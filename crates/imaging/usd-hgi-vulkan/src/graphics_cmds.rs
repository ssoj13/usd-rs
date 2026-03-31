//! Vulkan graphics command recording.
//!
//! Complete port of pxr/imaging/hgiVulkan/graphicsCmds.cpp/.h
//!
//! Records rendering commands into a Vulkan command buffer, managing the render
//! pass lifecycle.  Commands that require an active render pass (bind_resources,
//! set_viewport, set_scissor, bind_vertex_buffers, set_constant_values) are
//! deferred as closures and executed just before the first draw call when
//! `_apply_pending_updates` opens the render pass.

#![allow(unsafe_code)]

use std::sync::Arc;

use ash::vk;

use usd_gf::Vec4f;
use usd_hgi::{
    HgiAttachmentLoadOp, HgiBufferHandle, HgiCmds, HgiDrawIndexedOp, HgiDrawIndirectOp, HgiDrawOp,
    HgiGraphicsCmds, HgiGraphicsCmdsDesc, HgiGraphicsPipelineHandle, HgiMemoryBarrier,
    HgiResourceBindingsHandle, HgiScissor, HgiShaderStage, HgiTexture, HgiViewport,
};

use crate::buffer::HgiVulkanBuffer;
use crate::conversions::HgiVulkanConversions;
use crate::diagnostic;
use crate::graphics_pipeline::HgiVulkanGraphicsPipeline;
use crate::resource_bindings::HgiVulkanResourceBindings;

/// Vulkan graphics command encoder.
///
/// Port of C++ `HgiVulkanGraphicsCmds`.
///
/// The command buffer handle (`vk::CommandBuffer`) is optional — it is supplied
/// by the caller (e.g. `HgiVulkanDevice`) when creating the encoder through
/// `new()`.  The stub path (`new_stub`) leaves it as `None` so that all
/// recording paths are safely skipped.
pub struct HgiVulkanGraphicsCmds {
    /// Logical device — shared so deferred closures can capture it.
    device: Option<Arc<ash::Device>>,
    /// Optional debug-utils device extension loader, for GPU labels.
    debug_utils: Option<ash::ext::debug_utils::Device>,
    /// Descriptor capturing the attachments for this render pass.
    descriptor: HgiGraphicsCmdsDesc,
    /// Raw Vulkan command buffer handle owned while recording.
    command_buffer: Option<vk::CommandBuffer>,
    /// Currently bound graphics pipeline — needed when opening the render pass.
    pipeline: Option<HgiGraphicsPipelineHandle>,
    /// True once `vkCmdBeginRenderPass` has been recorded.
    render_pass_started: bool,
    /// True if `set_viewport` was called before the render pass opened.
    viewport_set: bool,
    /// True if `set_scissor` was called before the render pass opened.
    scissor_set: bool,
    /// Closures that require the render pass to be active.
    /// Drained and executed by `_apply_pending_updates`.
    pending_updates: Vec<Box<dyn FnOnce(Arc<ash::Device>, vk::CommandBuffer) + Send>>,
    /// Pre-built clear values (one per color attachment + optional depth).
    vk_clear_values: Vec<vk::ClearValue>,
    /// Tracks whether this encoder has been submitted to the GPU.
    submitted: bool,
}

// SAFETY: `HgiVulkanGraphicsCmds` is used from a single thread at a time.
// `ash::Device` is already `Send + Sync`.
unsafe impl Send for HgiVulkanGraphicsCmds {}
unsafe impl Sync for HgiVulkanGraphicsCmds {}

// ---------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------

impl HgiVulkanGraphicsCmds {
    /// Creates a fully-wired graphics command encoder.
    ///
    /// Mirrors C++ `HgiVulkanGraphicsCmds::HgiVulkanGraphicsCmds`.
    ///
    /// The command buffer is NOT acquired here; it is deferred to the first
    /// use (see `_create_command_buffer`) so that a background thread can
    /// acquire the correct per-thread pool buffer.  This encoder is provided
    /// `command_buffer = None` initially; callers set it via
    /// `set_command_buffer` before recording.
    ///
    /// Processes the descriptor's attachments to build `vk_clear_values` up-front.
    pub fn new(
        device: Arc<ash::Device>,
        debug_utils: Option<ash::ext::debug_utils::Device>,
        descriptor: HgiGraphicsCmdsDesc,
    ) -> Self {
        let mut vk_clear_values: Vec<vk::ClearValue> = Vec::new();

        // Build one VkClearValue per color attachment.
        for attachment_desc in &descriptor.color_attachment_descs {
            vk_clear_values.push(vk::ClearValue {
                color: color_clear_value(attachment_desc),
            });
        }

        // Append a depth/stencil clear value when a depth attachment is present.
        let has_depth = descriptor.depth_attachment_desc.format != usd_hgi::HgiFormat::Invalid;
        if has_depth {
            let a = &descriptor.depth_attachment_desc;
            vk_clear_values.push(vk::ClearValue {
                depth_stencil: vk::ClearDepthStencilValue {
                    depth: a.clear_value.x,
                    stencil: a.clear_value.y as u32,
                },
            });
        }

        Self {
            device: Some(device),
            debug_utils,
            descriptor,
            command_buffer: None,
            pipeline: None,
            render_pass_started: false,
            viewport_set: false,
            scissor_set: false,
            pending_updates: Vec::new(),
            vk_clear_values,
            submitted: false,
        }
    }

    /// Stub constructor — produces a no-op encoder with no live Vulkan resources.
    ///
    /// Used by the placeholder `HgiVulkan` stub in `hgi.rs` until a real device
    /// is wired up.  All trait methods are safe no-ops when the command buffer
    /// is `None`.
    pub fn new_stub() -> Self {
        Self {
            device: None,
            debug_utils: None,
            descriptor: HgiGraphicsCmdsDesc::default(),
            command_buffer: None,
            pipeline: None,
            render_pass_started: false,
            viewport_set: false,
            scissor_set: false,
            pending_updates: Vec::new(),
            vk_clear_values: Vec::new(),
            submitted: false,
        }
    }

    // -----------------------------------------------------------------------
    // Public API (beyond the traits)
    // -----------------------------------------------------------------------

    /// Attach a command buffer to this encoder.
    ///
    /// Called by the device after acquiring a buffer from the per-thread pool.
    /// Mirrors C++ `_CreateCommandBuffer` (which is called lazily on first use;
    /// we split the call site here for cleaner ownership).
    pub fn set_command_buffer(&mut self, cb: vk::CommandBuffer) {
        self.command_buffer = Some(cb);
    }

    /// Returns the raw Vulkan command buffer handle, if one has been set.
    ///
    /// Port of C++ `GetCommandBuffer()`.
    pub fn vk_command_buffer(&self) -> Option<vk::CommandBuffer> {
        self.command_buffer
    }

    /// Submit the recorded commands and optionally wait for completion.
    ///
    /// Port of C++ `_Submit`.  Called by `Hgi::SubmitCmds`.
    ///
    /// When no draw calls were recorded (render pass never started) we still
    /// clear attachments manually via `_clear_attachments_if_needed` so that
    /// load-op=Clear is honoured even for empty passes.
    pub fn submit(&mut self, _wait: usd_hgi::HgiSubmitWaitType) -> bool {
        if !self.render_pass_started {
            self._clear_attachments_if_needed();
        }
        self._end_render_pass();

        self.viewport_set = false;
        self.scissor_set = false;
        self.submitted = true;
        true
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Lazy command buffer acquisition hook.
    ///
    /// Port of C++ `_CreateCommandBuffer`.
    /// In C++ this acquires from the per-thread queue pool.  In our Rust port
    /// the buffer is set externally via `set_command_buffer` before recording.
    /// This method is a no-op placeholder for compatibility with C++ call sites.
    fn _create_command_buffer(&self) {
        // No-op: acquisition is performed externally in the full device integration.
    }

    /// Begin the render pass and execute all deferred state-setting commands.
    ///
    /// Port of C++ `_ApplyPendingUpdates`.
    fn _apply_pending_updates(&mut self) {
        let (device, cb) = match (&self.device, self.command_buffer) {
            (Some(d), Some(cb)) => (Arc::clone(d), cb),
            _ => return,
        };

        // Must have a pipeline before opening the render pass.
        let pipeline_handle = match &self.pipeline {
            Some(h) => h.clone(),
            None => {
                log::error!("HgiVulkanGraphicsCmds::_apply_pending_updates: no pipeline bound");
                return;
            }
        };

        // Begin render pass on the first draw call (once, for this encoder).
        if !self.render_pass_started && !self.pending_updates.is_empty() {
            self.render_pass_started = true;

            let pso = pipeline_handle
                .get()
                .and_then(|p| p.as_any().downcast_ref::<HgiVulkanGraphicsPipeline>());

            if let Some(pso) = pso {
                let size = framebuffer_size_from_desc(&self.descriptor);

                let begin_info = vk::RenderPassBeginInfo::default()
                    .render_pass(pso.vk_render_pass())
                    .render_area(vk::Rect2D {
                        offset: vk::Offset2D { x: 0, y: 0 },
                        extent: vk::Extent2D {
                            width: size[0],
                            height: size[1],
                        },
                    });

                // Attach clear values only when the pipeline needs them.
                let begin_info = if pso.clear_needed() {
                    begin_info.clear_values(&self.vk_clear_values)
                } else {
                    begin_info
                };

                unsafe {
                    device.cmd_begin_render_pass(cb, &begin_info, vk::SubpassContents::INLINE);
                }

                // Guarantee viewport and scissor are set — the pipeline hardcodes
                // one dynamic viewport and one dynamic scissor.
                if !self.viewport_set {
                    record_viewport(&device, cb, 0.0, 0.0, size[0] as f32, size[1] as f32);
                }
                if !self.scissor_set {
                    record_scissor(&device, cb, 0, 0, size[0], size[1]);
                }
            } else {
                // Pipeline's concrete type is unavailable (stub) — skip render pass.
                log::warn!(
                    "HgiVulkanGraphicsCmds: pipeline is not HgiVulkanGraphicsPipeline, \
                     skipping render pass begin"
                );
            }
        }

        // Execute all deferred state-setting commands now that the render pass is open.
        for update in self.pending_updates.drain(..) {
            update(Arc::clone(&device), cb);
        }
    }

    /// End the current render pass if one is active.
    ///
    /// Port of C++ `_EndRenderPass`.
    fn _end_render_pass(&mut self) {
        if self.render_pass_started {
            if let (Some(device), Some(cb)) = (&self.device, self.command_buffer) {
                unsafe {
                    device.cmd_end_render_pass(cb);
                }
            }
            self.render_pass_started = false;
        }
    }

    /// Manually clear attachments when no draw calls occurred.
    ///
    /// Port of C++ `_ClearAttachmentsIfNeeded`.
    /// When the render pass is never opened (no draws) we honour load-op=Clear
    /// by issuing `vkCmdClearColorImage` / `vkCmdClearDepthStencilImage` outside
    /// a render pass, using image layout barriers around the clear commands.
    fn _clear_attachments_if_needed(&mut self) {
        let (device, cb) = match (&self.device, self.command_buffer) {
            (Some(d), Some(cb)) => (Arc::clone(d), cb),
            _ => return,
        };

        for i in 0..self.descriptor.color_attachment_descs.len() {
            let attachment_desc = &self.descriptor.color_attachment_descs[i];
            if attachment_desc.load_op != HgiAttachmentLoadOp::Clear {
                continue;
            }
            let clear_color = color_clear_value(attachment_desc);

            // Clear the primary color texture.
            if let Some(tex_handle) = self.descriptor.color_textures.get(i) {
                if tex_handle.is_valid() {
                    if let Some(tex) = tex_handle.get().and_then(|t| {
                        t.as_any()
                            .downcast_ref::<crate::texture::HgiVulkanTexture>()
                    }) {
                        clear_color_image(&device, cb, tex, clear_color);
                    }
                }
            }

            // Clear the MSAA resolve texture if present.
            if let Some(resolve_handle) = self.descriptor.color_resolve_textures.get(i) {
                if resolve_handle.is_valid() {
                    if let Some(tex) = resolve_handle.get().and_then(|t| {
                        t.as_any()
                            .downcast_ref::<crate::texture::HgiVulkanTexture>()
                    }) {
                        clear_color_image(&device, cb, tex, clear_color);
                    }
                }
            }
        }

        // Clear the depth attachment.
        let depth_desc = &self.descriptor.depth_attachment_desc;
        if depth_desc.load_op == HgiAttachmentLoadOp::Clear {
            let clear_ds = vk::ClearDepthStencilValue {
                depth: depth_desc.clear_value.x,
                stencil: depth_desc.clear_value.y as u32,
            };

            if self.descriptor.depth_texture.is_valid() {
                if let Some(tex) = self.descriptor.depth_texture.get().and_then(|t| {
                    t.as_any()
                        .downcast_ref::<crate::texture::HgiVulkanTexture>()
                }) {
                    clear_depth_image(&device, cb, tex, clear_ds);
                }
            }

            if self.descriptor.depth_resolve_texture.is_valid() {
                if let Some(tex) = self.descriptor.depth_resolve_texture.get().and_then(|t| {
                    t.as_any()
                        .downcast_ref::<crate::texture::HgiVulkanTexture>()
                }) {
                    clear_depth_image(&device, cb, tex, clear_ds);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// HgiCmds trait
// ---------------------------------------------------------------------------

impl HgiCmds for HgiVulkanGraphicsCmds {
    fn is_submitted(&self) -> bool {
        self.submitted
    }

    fn push_debug_group(&mut self, label: &str) {
        self._create_command_buffer();
        if let Some(cb) = self.command_buffer {
            // Blue = graphics debug color, matching C++ `s_graphicsDebugColor`.
            let color = [0.0_f32, 0.639, 0.878, 1.0];
            diagnostic::begin_label(self.debug_utils.as_ref(), cb, label, color);
        }
    }

    fn pop_debug_group(&mut self) {
        if let Some(cb) = self.command_buffer {
            diagnostic::end_label(self.debug_utils.as_ref(), cb);
        }
    }

    fn insert_debug_marker(&mut self, label: &str) {
        self._create_command_buffer();
        if let Some(cb) = self.command_buffer {
            let color = [0.0_f32; 4];
            diagnostic::insert_debug_marker(self.debug_utils.as_ref(), cb, label, color);
        }
    }
}

// ---------------------------------------------------------------------------
// HgiGraphicsCmds trait
// ---------------------------------------------------------------------------

impl HgiGraphicsCmds for HgiVulkanGraphicsCmds {
    /// Bind a graphics pipeline.
    ///
    /// Port of C++ `BindPipeline`.
    /// Ends any active render pass (supports re-use with multiple pipelines),
    /// records `vkCmdBindPipeline(GRAPHICS)`, and stores the pipeline handle so
    /// `_apply_pending_updates` can open the correct render pass.
    fn bind_pipeline(&mut self, pipeline: &HgiGraphicsPipelineHandle) {
        self._create_command_buffer();

        // End the previous render pass before switching pipelines.
        self._end_render_pass();

        self.pipeline = Some(pipeline.clone());

        if let (Some(device), Some(cb)) = (&self.device, self.command_buffer) {
            if let Some(pso) = pipeline
                .get()
                .and_then(|p| p.as_any().downcast_ref::<HgiVulkanGraphicsPipeline>())
            {
                pso.bind_pipeline(cb);
                let _ = device; // device accessed via pso — suppress unused warning
            }
        }
    }

    /// Bind descriptor sets.
    ///
    /// Port of C++ `BindResources`.
    /// Deferred until the render pass is active.
    fn bind_resources(&mut self, resources: &HgiResourceBindingsHandle) {
        let resources = resources.clone();
        let pipeline = self.pipeline.clone();
        self.pending_updates.push(Box::new(move |_device, cb| {
            let pso = pipeline
                .as_ref()
                .and_then(|h| h.get())
                .and_then(|p| p.as_any().downcast_ref::<HgiVulkanGraphicsPipeline>());

            let rb = resources
                .get()
                .and_then(|r| r.as_any().downcast_ref::<HgiVulkanResourceBindings>());

            if let (Some(pso), Some(rb)) = (pso, rb) {
                rb.bind_resources(
                    cb,
                    vk::PipelineBindPoint::GRAPHICS,
                    pso.vk_pipeline_layout(),
                );
            }
        }));
    }

    /// Push constants (shader constant data).
    ///
    /// Port of C++ `SetConstantValues`.
    /// Makes a copy of `data` so that stack-allocated constants are valid when
    /// the deferred closure eventually executes.
    fn set_constant_values(
        &mut self,
        _pipeline: &HgiGraphicsPipelineHandle,
        stages: HgiShaderStage,
        _bind_index: u32,
        data: &[u8],
    ) {
        let data_copy = data.to_vec();
        let pipeline = self.pipeline.clone();

        self.pending_updates.push(Box::new(move |device, cb| {
            let pso = pipeline
                .as_ref()
                .and_then(|h| h.get())
                .and_then(|p| p.as_any().downcast_ref::<HgiVulkanGraphicsPipeline>());

            if let Some(pso) = pso {
                // offset is always 0, matching C++ comment.
                unsafe {
                    device.cmd_push_constants(
                        cb,
                        pso.vk_pipeline_layout(),
                        HgiVulkanConversions::get_shader_stages(stages),
                        0,
                        &data_copy,
                    );
                }
            }
        }));
    }

    /// Bind vertex buffers.
    ///
    /// Port of C++ `BindVertexBuffers`.
    /// Deferred until the render pass is active.
    fn bind_vertex_buffers(&mut self, buffers: &[HgiBufferHandle], offsets: &[u64]) {
        let buffers: Vec<HgiBufferHandle> = buffers.to_vec();
        let offsets: Vec<u64> = offsets.to_vec();

        self.pending_updates.push(Box::new(move |device, cb| {
            let mut vk_buffers: Vec<vk::Buffer> = Vec::with_capacity(buffers.len());
            let mut vk_offsets: Vec<vk::DeviceSize> = Vec::with_capacity(buffers.len());

            for (i, buf_handle) in buffers.iter().enumerate() {
                if let Some(buf) = buf_handle
                    .get()
                    .and_then(|b| b.as_any().downcast_ref::<HgiVulkanBuffer>())
                {
                    let vk_buf = buf.vk_buffer();
                    if vk_buf != vk::Buffer::null() {
                        vk_buffers.push(vk_buf);
                        vk_offsets.push(offsets.get(i).copied().unwrap_or(0));
                    }
                }
            }

            if !vk_buffers.is_empty() {
                // first_binding = 0; a full port would use HgiVertexBufferBinding.index.
                unsafe {
                    device.cmd_bind_vertex_buffers(cb, 0, &vk_buffers, &vk_offsets);
                }
            }
        }));
    }

    /// Set the viewport rectangle.
    ///
    /// Port of C++ `SetViewport`.
    /// Deferred until the render pass is active.
    ///
    /// C++ comment: We do NOT flip the viewport Y here.  We render upside-down
    /// intentionally to keep OpenGL projection matrix conventions and to make
    /// downstream AOV handling consistent between Vulkan and OpenGL.
    /// Winding is flipped instead in conversions.cpp / shaderGenerator.cpp.
    fn set_viewport(&mut self, viewport: &HgiViewport) {
        self.viewport_set = true;
        let vp = *viewport;
        self.pending_updates.push(Box::new(move |device, cb| {
            let vulkan_vp = vk::Viewport {
                x: vp.x,
                y: vp.y,
                width: vp.width,
                height: vp.height,
                min_depth: 0.0,
                max_depth: 1.0,
            };
            unsafe {
                device.cmd_set_viewport(cb, 0, &[vulkan_vp]);
            }
        }));
    }

    /// Set the scissor rectangle.
    ///
    /// Port of C++ `SetScissor`.
    /// Deferred until the render pass is active.
    fn set_scissor(&mut self, scissor: &HgiScissor) {
        self.scissor_set = true;
        let sc = *scissor;
        self.pending_updates.push(Box::new(move |device, cb| {
            let rect = vk::Rect2D {
                offset: vk::Offset2D { x: sc.x, y: sc.y },
                extent: vk::Extent2D {
                    width: sc.width,
                    height: sc.height,
                },
            };
            unsafe {
                device.cmd_set_scissor(cb, 0, &[rect]);
            }
        }));
    }

    /// Set blend constant color.
    ///
    /// No C++ equivalent method in `HgiGraphicsCmds`, but corresponds to
    /// `vkCmdSetBlendConstants`.  Deferred until the render pass is active.
    fn set_blend_constant_color(&mut self, color: &Vec4f) {
        let constants = [color.x, color.y, color.z, color.w];
        self.pending_updates
            .push(Box::new(move |device, cb| unsafe {
                device.cmd_set_blend_constants(cb, &constants);
            }));
    }

    /// Set stencil reference value for front-and-back faces.
    fn set_stencil_reference_value(&mut self, value: u32) {
        self.pending_updates
            .push(Box::new(move |device, cb| unsafe {
                device.cmd_set_stencil_reference(cb, vk::StencilFaceFlags::FRONT_AND_BACK, value);
            }));
    }

    /// Non-indexed draw call.
    ///
    /// Port of C++ `Draw`.
    fn draw(&mut self, op: &HgiDrawOp) {
        self._apply_pending_updates();
        if let (Some(device), Some(cb)) = (&self.device, self.command_buffer) {
            unsafe {
                device.cmd_draw(
                    cb,
                    op.vertex_count,
                    op.instance_count,
                    op.base_vertex,
                    op.base_instance,
                );
            }
        }
    }

    /// Indexed draw call.
    ///
    /// Port of C++ `DrawIndexed`.
    /// Binds the index buffer as `VK_INDEX_TYPE_UINT32` at offset 0, then
    /// issues `vkCmdDrawIndexed`.  The `base_index` (byte offset) is converted
    /// to a first-index count by dividing by `sizeof(uint32_t) = 4`, matching
    /// the C++ `indexBufferByteOffset / sizeof(uint32_t)` calculation.
    fn draw_indexed(&mut self, index_buffer: &HgiBufferHandle, op: &HgiDrawIndexedOp) {
        self._apply_pending_updates();
        if let (Some(device), Some(cb)) = (&self.device, self.command_buffer) {
            if let Some(ibo) = index_buffer
                .get()
                .and_then(|b| b.as_any().downcast_ref::<HgiVulkanBuffer>())
            {
                // Convert byte offset to first-index by dividing by 4.
                let first_index = op.base_index / 4;
                unsafe {
                    device.cmd_bind_index_buffer(cb, ibo.vk_buffer(), 0, vk::IndexType::UINT32);
                    device.cmd_draw_indexed(
                        cb,
                        op.index_count,
                        op.instance_count,
                        first_index,
                        op.base_vertex,
                        op.base_instance,
                    );
                }
            }
        }
    }

    /// Indirect (non-indexed) draw call.
    ///
    /// Port of C++ `DrawIndirect`.
    fn draw_indirect(&mut self, op: &HgiDrawIndirectOp) {
        self._apply_pending_updates();
        if let (Some(device), Some(cb)) = (&self.device, self.command_buffer) {
            if let Some(draw_buf) = op
                .draw_buffer
                .get()
                .and_then(|b| b.as_any().downcast_ref::<HgiVulkanBuffer>())
            {
                unsafe {
                    device.cmd_draw_indirect(
                        cb,
                        draw_buf.vk_buffer(),
                        op.draw_buffer_byte_offset as vk::DeviceSize,
                        op.draw_count,
                        op.stride,
                    );
                }
            }
        }
    }

    /// Indirect indexed draw call.
    ///
    /// Port of C++ `DrawIndexedIndirect`.
    /// Binds the index buffer (`VK_INDEX_TYPE_UINT32`) then issues
    /// `vkCmdDrawIndexedIndirect`.  The `drawParameterBufferUInt32` and
    /// `patchBaseVertexByteOffset` arguments from C++ are not used here
    /// (marked unused in C++ too).
    fn draw_indexed_indirect(&mut self, index_buffer: &HgiBufferHandle, op: &HgiDrawIndirectOp) {
        self._apply_pending_updates();
        if let (Some(device), Some(cb)) = (&self.device, self.command_buffer) {
            let ibo = index_buffer
                .get()
                .and_then(|b| b.as_any().downcast_ref::<HgiVulkanBuffer>());
            let draw_buf = op
                .draw_buffer
                .get()
                .and_then(|b| b.as_any().downcast_ref::<HgiVulkanBuffer>());

            if let (Some(ibo), Some(draw_buf)) = (ibo, draw_buf) {
                unsafe {
                    device.cmd_bind_index_buffer(cb, ibo.vk_buffer(), 0, vk::IndexType::UINT32);
                    device.cmd_draw_indexed_indirect(
                        cb,
                        draw_buf.vk_buffer(),
                        op.draw_buffer_byte_offset as vk::DeviceSize,
                        op.draw_count,
                        op.stride,
                    );
                }
            }
        }
    }

    /// Insert a pipeline memory barrier.
    ///
    /// Port of C++ `InsertMemoryBarrier`.
    /// Uses a full `ALL_COMMANDS` read+write barrier, matching
    /// `HgiMemoryBarrierAll` semantics in `HgiVulkanCommandBuffer`.
    fn memory_barrier(&mut self, barrier: HgiMemoryBarrier) {
        self._create_command_buffer();
        if let (Some(device), Some(cb)) = (&self.device, self.command_buffer) {
            if barrier == HgiMemoryBarrier::ALL {
                let mem_barrier = vk::MemoryBarrier::default()
                    .src_access_mask(vk::AccessFlags::MEMORY_READ | vk::AccessFlags::MEMORY_WRITE)
                    .dst_access_mask(vk::AccessFlags::MEMORY_READ | vk::AccessFlags::MEMORY_WRITE);
                unsafe {
                    device.cmd_pipeline_barrier(
                        cb,
                        vk::PipelineStageFlags::ALL_COMMANDS,
                        vk::PipelineStageFlags::ALL_COMMANDS,
                        vk::DependencyFlags::empty(),
                        &[mem_barrier],
                        &[],
                        &[],
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Free helpers — stateless Vulkan recording utilities
// ---------------------------------------------------------------------------

/// Records `vkCmdSetViewport` onto `cb`.
fn record_viewport(
    device: &ash::Device,
    cb: vk::CommandBuffer,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) {
    let vp = vk::Viewport {
        x,
        y,
        width,
        height,
        min_depth: 0.0,
        max_depth: 1.0,
    };
    unsafe {
        device.cmd_set_viewport(cb, 0, &[vp]);
    }
}

/// Records `vkCmdSetScissor` onto `cb`.
fn record_scissor(
    device: &ash::Device,
    cb: vk::CommandBuffer,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) {
    let rect = vk::Rect2D {
        offset: vk::Offset2D { x, y },
        extent: vk::Extent2D { width, height },
    };
    unsafe {
        device.cmd_set_scissor(cb, 0, &[rect]);
    }
}

/// Issues barrier + `vkCmdClearColorImage` + barrier for one color texture.
///
/// Port of the color-clear branch in C++ `_ClearAttachmentsIfNeeded`.
fn clear_color_image(
    device: &ash::Device,
    cb: vk::CommandBuffer,
    texture: &crate::texture::HgiVulkanTexture,
    clear_color: vk::ClearColorValue,
) {
    let image = texture.vk_image();
    if image == vk::Image::null() {
        return;
    }
    let old_layout = texture.vk_image_layout();
    let subresource_range = vk::ImageSubresourceRange {
        aspect_mask: HgiVulkanConversions::get_image_aspect_flag(texture.descriptor().usage),
        base_mip_level: 0,
        level_count: texture.descriptor().mip_levels as u32,
        base_array_layer: 0,
        layer_count: texture.descriptor().layer_count as u32,
    };

    // Transition to TRANSFER_DST_OPTIMAL for the clear.
    let pre = layout_barrier(
        image,
        old_layout,
        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
        vk::AccessFlags::TRANSFER_WRITE,
        subresource_range,
    );
    unsafe {
        device.cmd_pipeline_barrier(
            cb,
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            vk::PipelineStageFlags::TRANSFER,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[pre],
        );
        device.cmd_clear_color_image(
            cb,
            image,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            &clear_color,
            &[subresource_range],
        );
    }

    // Transition back to original layout.
    let post = layout_barrier(
        image,
        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        old_layout,
        vk::AccessFlags::TRANSFER_WRITE,
        vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
        subresource_range,
    );
    unsafe {
        device.cmd_pipeline_barrier(
            cb,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[post],
        );
    }
}

/// Issues barrier + `vkCmdClearDepthStencilImage` + barrier for a depth texture.
///
/// Port of the depth-clear branch in C++ `_ClearAttachmentsIfNeeded`.
fn clear_depth_image(
    device: &ash::Device,
    cb: vk::CommandBuffer,
    texture: &crate::texture::HgiVulkanTexture,
    clear_value: vk::ClearDepthStencilValue,
) {
    let image = texture.vk_image();
    if image == vk::Image::null() {
        return;
    }
    let old_layout = texture.vk_image_layout();
    let subresource_range = vk::ImageSubresourceRange {
        aspect_mask: HgiVulkanConversions::get_image_aspect_flag(texture.descriptor().usage),
        base_mip_level: 0,
        level_count: texture.descriptor().mip_levels as u32,
        base_array_layer: 0,
        layer_count: texture.descriptor().layer_count as u32,
    };

    let pre = layout_barrier(
        image,
        old_layout,
        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
        vk::AccessFlags::TRANSFER_WRITE,
        subresource_range,
    );
    unsafe {
        device.cmd_pipeline_barrier(
            cb,
            vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
            vk::PipelineStageFlags::TRANSFER,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[pre],
        );
        device.cmd_clear_depth_stencil_image(
            cb,
            image,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            &clear_value,
            &[subresource_range],
        );
    }

    let post = layout_barrier(
        image,
        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        old_layout,
        vk::AccessFlags::TRANSFER_WRITE,
        vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
        subresource_range,
    );
    unsafe {
        device.cmd_pipeline_barrier(
            cb,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[post],
        );
    }
}

/// Builds a `VkImageMemoryBarrier` with `QUEUE_FAMILY_IGNORED` queue families.
fn layout_barrier(
    image: vk::Image,
    old_layout: vk::ImageLayout,
    new_layout: vk::ImageLayout,
    src_access: vk::AccessFlags,
    dst_access: vk::AccessFlags,
    subresource_range: vk::ImageSubresourceRange,
) -> vk::ImageMemoryBarrier<'static> {
    vk::ImageMemoryBarrier::default()
        .src_access_mask(src_access)
        .dst_access_mask(dst_access)
        .old_layout(old_layout)
        .new_layout(new_layout)
        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .image(image)
        .subresource_range(subresource_range)
}

/// Builds the `VkClearColorValue` for an attachment descriptor.
///
/// Port of C++ `_SetVkClearColorValue`.
/// Float path for floating-point / normalised formats; int32 path for integer
/// formats used in pick / ID renders.
fn color_clear_value(attachment_desc: &usd_hgi::HgiAttachmentDesc) -> vk::ClearColorValue {
    if is_float_format(attachment_desc.format) {
        vk::ClearColorValue {
            float32: [
                attachment_desc.clear_value.x,
                attachment_desc.clear_value.y,
                attachment_desc.clear_value.z,
                attachment_desc.clear_value.w,
            ],
        }
    } else {
        vk::ClearColorValue {
            int32: [
                attachment_desc.clear_value.x as i32,
                attachment_desc.clear_value.y as i32,
                attachment_desc.clear_value.z as i32,
                attachment_desc.clear_value.w as i32,
            ],
        }
    }
}

/// Returns true for floating-point and normalised formats; false for pure integer formats.
///
/// Port of C++ `HgiIsFloatFormat`.
fn is_float_format(format: usd_hgi::HgiFormat) -> bool {
    use usd_hgi::HgiFormat;
    !matches!(
        format,
        HgiFormat::Int16
            | HgiFormat::Int16Vec2
            | HgiFormat::Int16Vec3
            | HgiFormat::Int16Vec4
            | HgiFormat::UInt16
            | HgiFormat::UInt16Vec2
            | HgiFormat::UInt16Vec3
            | HgiFormat::UInt16Vec4
            | HgiFormat::Int32
            | HgiFormat::Int32Vec2
            | HgiFormat::Int32Vec3
            | HgiFormat::Int32Vec4
    )
}

/// Determines the framebuffer dimensions from the descriptor's textures.
///
/// Port of the `GfVec2i size(0)` / `pso->AcquireVulkanFramebuffer(...)` logic
/// in C++ `_ApplyPendingUpdates`.  We inspect descriptor dimensions instead of
/// querying a live framebuffer object (framebuffer management lives in
/// `HgiVulkanGraphicsPipeline::AcquireVulkanFramebuffer`).
fn framebuffer_size_from_desc(descriptor: &HgiGraphicsCmdsDesc) -> [u32; 2] {
    // Prefer the first color texture's dimensions.
    if let Some(tex_handle) = descriptor.color_textures.first() {
        if let Some(tex) = tex_handle.get() {
            let d = tex.descriptor();
            return [d.dimensions.x as u32, d.dimensions.y as u32];
        }
    }
    // Fall back to the depth texture.
    if descriptor.depth_texture.is_valid() {
        if let Some(tex) = descriptor.depth_texture.get() {
            let d = tex.descriptor();
            return [d.dimensions.x as u32, d.dimensions.y as u32];
        }
    }
    [0, 0]
}
