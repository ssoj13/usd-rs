//! Base class for effects shaders in Hydra extensions
//!
//! Provides functionality to create and manage a single HgiGraphicsPipeline
//! instance and issue draw calls to that instance. Primarily intended for
//! full-screen passes that perform screen-space effects.
//!
//! Port of pxr/imaging/hdx/effectsShader.h/.cpp

use parking_lot::RwLock;
use std::sync::Arc;
use usd_gf::Vec4i;
use usd_hgi::{Hgi, HgiBufferBindDesc};
use usd_hgi::{
    HgiAttachmentDesc, HgiBufferHandle, HgiDepthStencilState, HgiDrawIndexedOp, HgiDrawOp,
    HgiGraphicsCmds, HgiGraphicsCmdsDesc, HgiGraphicsPipelineDesc, HgiGraphicsPipelineHandle,
    HgiMultiSampleState, HgiPrimitiveType, HgiRasterizationState, HgiResourceBindingsDesc,
    HgiResourceBindingsHandle, HgiSampleCount, HgiShaderFunctionHandle, HgiShaderProgramHandle,
    HgiShaderStage, HgiSubmitWaitType, HgiTextureBindDesc, HgiTextureHandle, HgiVertexBufferDesc,
    HgiViewport,
};

/// Base class for effects shaders
///
/// This class provides functionality to create and manage a single
/// HgiGraphicsPipeline instance and to issue draw calls to that instance.
///
/// Sub-classes should define the actual interface for issuing the draw call
/// leveraging the common functionality this class provides to facilitate that.
///
/// It is primarily intended to be used for full screen passes that perform a
/// screen-space effect. As an example, the HdxFullscreenShader class inherits
/// from this class and makes use of the functions defined here to set up its
/// pipeline and issue draw commands.
pub struct HdxEffectsShader {
    /// Hgi instance for GPU resource management
    /// Uses std::sync::RwLock to match HgiDriverHandle's internal type.
    hgi: Arc<RwLock<dyn Hgi>>,

    /// Debug name for GPU debugging tools
    debug_name: String,

    /// Graphics pipeline descriptor
    pipeline_desc: HgiGraphicsPipelineDesc,

    /// Graphics pipeline handle
    pipeline: Option<HgiGraphicsPipelineHandle>,

    /// Shader constants data buffer
    constants_data: Vec<u8>,

    /// Resource bindings descriptor
    resource_bindings_desc: HgiResourceBindingsDesc,

    /// Resource bindings handle
    resource_bindings: Option<HgiResourceBindingsHandle>,

    /// Active graphics commands (during recording)
    gfx_cmds: Option<Box<dyn HgiGraphicsCmds>>,
}

impl HdxEffectsShader {
    /// Create a new effects shader
    ///
    /// # Arguments
    ///
    /// * `hgi` - Hgi instance to use to create any GPU resources
    /// * `debug_name` - Name used to tag GPU resources to aid in debugging
    pub fn new(hgi: Arc<RwLock<dyn Hgi>>, debug_name: String) -> Self {
        let name = if debug_name.is_empty() {
            "HdxEffectsShader".to_string()
        } else {
            debug_name
        };
        let mut pipeline_desc = HgiGraphicsPipelineDesc::default();
        pipeline_desc.debug_name = name.clone();
        let mut resource_bindings_desc = HgiResourceBindingsDesc::default();
        resource_bindings_desc.debug_name = name.clone();

        Self {
            hgi,
            debug_name: name,
            pipeline_desc,
            pipeline: None,
            constants_data: Vec::new(),
            resource_bindings_desc,
            resource_bindings: None,
            gfx_cmds: None,
        }
    }

    // -----------------------------------------------------------------------
    // Shader compilation error reporting
    // -----------------------------------------------------------------------

    /// Print shader compile errors for a shader function.
    ///
    /// Logs compilation errors if the shader function is invalid.
    pub fn print_compile_errors_fn(shader_fn: &HgiShaderFunctionHandle) {
        if let Some(f) = shader_fn.get() {
            if !f.is_valid() {
                eprintln!(
                    "HdxEffectsShader: shader function error: {}",
                    f.compile_errors()
                );
            }
        }
    }

    /// Print shader compile errors for a shader program and all its functions.
    pub fn print_compile_errors_program(shader_program: &HgiShaderProgramHandle) {
        if let Some(prog) = shader_program.get() {
            // Check each shader function linked into the program
            for fn_handle in &prog.descriptor().shader_functions {
                Self::print_compile_errors_fn(fn_handle);
            }
            if !prog.is_valid() {
                eprintln!(
                    "HdxEffectsShader: shader program error: {}",
                    prog.link_errors()
                );
            }
        }
    }

    // -----------------------------------------------------------------------
    // Pipeline descriptor setters — invalidate pipeline on change
    // -----------------------------------------------------------------------

    /// Set color attachment descriptors.
    ///
    /// Destroys the existing pipeline if the attachments changed.
    pub fn set_color_attachments(&mut self, color_attachment_descs: Vec<HgiAttachmentDesc>) {
        // Compare ignoring format/usage (those come from textures at pipeline creation time)
        if !attachments_match(
            &self.pipeline_desc.color_attachments,
            &color_attachment_descs,
        ) {
            self.destroy_pipeline();
        }
        self.pipeline_desc.color_attachments = color_attachment_descs;
    }

    /// Set depth attachment descriptor.
    ///
    /// Destroys the existing pipeline if the attachment changed.
    pub fn set_depth_attachment(&mut self, depth_attachment_desc: HgiAttachmentDesc) {
        let matches = self
            .pipeline_desc
            .depth_attachment
            .as_ref()
            .map(|d| attachment_match(d, &depth_attachment_desc))
            .unwrap_or(false);
        if !matches {
            self.destroy_pipeline();
        }
        self.pipeline_desc.depth_attachment = Some(depth_attachment_desc);
    }

    /// Set primitive topology type.
    pub fn set_primitive_type(&mut self, primitive_type: HgiPrimitiveType) {
        if self.pipeline_desc.primitive_type != primitive_type {
            self.destroy_pipeline();
        }
        self.pipeline_desc.primitive_type = primitive_type;
    }

    /// Set shader program.
    pub fn set_shader_program(&mut self, shader_program: HgiShaderProgramHandle) {
        if self.pipeline_desc.shader_program != shader_program {
            self.destroy_pipeline();
        }
        self.pipeline_desc.shader_program = shader_program;
    }

    /// Set vertex buffer descriptors.
    pub fn set_vertex_buffer_descs(&mut self, vertex_buffer_descs: Vec<HgiVertexBufferDesc>) {
        if self.pipeline_desc.vertex_buffers != vertex_buffer_descs {
            self.destroy_pipeline();
        }
        self.pipeline_desc.vertex_buffers = vertex_buffer_descs;
    }

    /// Set depth stencil state.
    pub fn set_depth_stencil_state(&mut self, depth_stencil_state: HgiDepthStencilState) {
        if self.pipeline_desc.depth_stencil_state != depth_stencil_state {
            self.destroy_pipeline();
        }
        self.pipeline_desc.depth_stencil_state = depth_stencil_state;
    }

    /// Set multi-sample state.
    ///
    /// Only destroys the pipeline when the parts of multi-sample state that do
    /// NOT come from the texture descriptors have changed (mirrors C++ behaviour).
    pub fn set_multi_sample_state(&mut self, multi_sample_state: HgiMultiSampleState) {
        // Compare only the fields that are independent of the attached textures
        let old_partial = partial_multisample_copy(&self.pipeline_desc.multi_sample_state);
        let new_partial = partial_multisample_copy(&multi_sample_state);
        if old_partial != new_partial {
            self.destroy_pipeline();
        }
        self.pipeline_desc.multi_sample_state = multi_sample_state;
    }

    /// Set rasterization state.
    pub fn set_rasterization_state(&mut self, rasterization_state: HgiRasterizationState) {
        if self.pipeline_desc.rasterization_state != rasterization_state {
            self.destroy_pipeline();
        }
        self.pipeline_desc.rasterization_state = rasterization_state;
    }

    /// Set shader constants (push constants / function constants).
    ///
    /// Destroys the existing pipeline when the size or stage usage changes,
    /// because those are baked into the pipeline descriptor.
    pub fn set_shader_constants(
        &mut self,
        byte_size: u32,
        data: &[u8],
        stage_usage: HgiShaderStage,
    ) {
        // If byte size or stage changed we need a new pipeline
        let size_changed = byte_size as usize != self.constants_data.len();
        let stage_changed = stage_usage != self.pipeline_desc.shader_constants_desc.stage_usage;
        if size_changed || stage_changed {
            self.destroy_pipeline();
            self.pipeline_desc.shader_constants_desc.byte_size = byte_size;
            self.pipeline_desc.shader_constants_desc.stage_usage = stage_usage;
        }
        // Always capture the new data (even if layout didn't change)
        let end = (byte_size as usize).min(data.len());
        self.constants_data.clear();
        self.constants_data.extend_from_slice(&data[..end]);
    }

    /// Set texture bindings. Destroys resource bindings when they change.
    pub fn set_texture_bindings(&mut self, textures: Vec<HgiTextureBindDesc>) {
        if self.resource_bindings_desc.texture_bindings != textures {
            self.destroy_resource_bindings();
        }
        self.resource_bindings_desc.texture_bindings = textures;
    }

    /// Set buffer bindings. Destroys resource bindings when they change.
    pub fn set_buffer_bindings(&mut self, buffers: Vec<HgiBufferBindDesc>) {
        if self.resource_bindings_desc.buffer_bindings != buffers {
            self.destroy_resource_bindings();
        }
        self.resource_bindings_desc.buffer_bindings = buffers;
    }

    // -----------------------------------------------------------------------
    // Main render dispatch
    // -----------------------------------------------------------------------

    /// Create graphics commands, record draw commands, and submit.
    ///
    /// This is the main entry point for rendering. It:
    /// 1. Ensures the pipeline is created and attachment formats are up to date
    /// 2. Ensures resource bindings are created
    /// 3. Creates a HgiGraphicsCmds object
    /// 4. Binds the pipeline, viewport, and resource bindings
    /// 5. Pushes any shader constants
    /// 6. Calls `record_draw_cmds()` so sub-classes can issue draw calls
    /// 7. Submits the command buffer to the GPU
    pub fn create_and_submit_graphics_cmds(
        &mut self,
        color_textures: &[HgiTextureHandle],
        color_resolve_textures: &[HgiTextureHandle],
        depth_texture: &HgiTextureHandle,
        depth_resolve_texture: &HgiTextureHandle,
        viewport: &Vec4i,
    ) {
        // Ensure pipeline is ready with correct attachment formats/sample count
        self.create_pipeline(
            color_textures,
            color_resolve_textures,
            depth_texture,
            depth_resolve_texture,
        );

        // Ensure resource bindings are ready
        self.create_resource_bindings();

        // Build the HgiGraphicsCmds descriptor from the pipeline's attachment info
        let mut gfx_desc = HgiGraphicsCmdsDesc::new();
        gfx_desc.color_attachment_descs = self.pipeline_desc.color_attachments.clone();
        gfx_desc.depth_attachment_desc = self
            .pipeline_desc
            .depth_attachment
            .clone()
            .unwrap_or_default();
        gfx_desc.color_textures = color_textures.to_vec();
        gfx_desc.color_resolve_textures = color_resolve_textures.to_vec();
        gfx_desc.depth_texture = depth_texture.clone();
        gfx_desc.depth_resolve_texture = depth_resolve_texture.clone();

        // Create the graphics command encoder
        let mut cmds = {
            let mut hgi = self.hgi.write();
            hgi.create_graphics_cmds(&gfx_desc)
        };

        // Viewport converted from Vec4i(x, y, w, h)
        let vp = HgiViewport::new(
            viewport[0] as f32,
            viewport[1] as f32,
            viewport[2] as f32,
            viewport[3] as f32,
        );

        cmds.push_debug_group(&self.debug_name);

        // Bind pipeline
        if let Some(ref pipeline) = self.pipeline {
            cmds.bind_pipeline(pipeline);
        }

        // Set viewport
        cmds.set_viewport(&vp);

        // Bind resource bindings
        if let Some(ref bindings) = self.resource_bindings {
            cmds.bind_resources(bindings);
        }

        // Push shader constants if any
        if !self.constants_data.is_empty() {
            if let Some(ref pipeline) = self.pipeline {
                cmds.set_constant_values(
                    pipeline,
                    self.pipeline_desc.shader_constants_desc.stage_usage,
                    0,
                    &self.constants_data,
                );
            }
        }

        // Store cmds so record_draw_cmds() sub-class calls can use them
        self.gfx_cmds = Some(cmds);

        // Let the sub-class record its draw calls
        self.record_draw_cmds();

        // Pop debug group and submit
        if let Some(mut cmds) = self.gfx_cmds.take() {
            cmds.pop_debug_group();
            let mut hgi = self.hgi.write();
            hgi.submit_cmds(cmds, HgiSubmitWaitType::NoWait);
        }
    }

    /// Record draw commands (override in derived classes).
    ///
    /// Sub-classes should override this method and invoke one or more calls to
    /// `draw_non_indexed` or `draw_indexed`.
    pub fn record_draw_cmds(&mut self) {
        // Base class no-op — derived types override via composition/delegation
    }

    // -----------------------------------------------------------------------
    // Draw calls (called from record_draw_cmds)
    // -----------------------------------------------------------------------

    /// Issue a non-indexed draw call.
    ///
    /// Binds the vertex buffer and calls HgiGraphicsCmds::draw.
    pub fn draw_non_indexed(
        &mut self,
        vertex_buffer: &HgiBufferHandle,
        vertex_count: u32,
        base_vertex: u32,
        instance_count: u32,
        base_instance: u32,
    ) {
        if let Some(ref mut cmds) = self.gfx_cmds {
            cmds.bind_vertex_buffers(&[vertex_buffer.clone()], &[0u64]);
            cmds.draw(&HgiDrawOp {
                vertex_count,
                base_vertex,
                instance_count,
                base_instance,
            });
        }
    }

    /// Issue an indexed draw call.
    ///
    /// Binds the vertex buffer and calls HgiGraphicsCmds::draw_indexed.
    pub fn draw_indexed(
        &mut self,
        vertex_buffer: &HgiBufferHandle,
        index_buffer: &HgiBufferHandle,
        index_count: u32,
        index_buffer_byte_offset: u32,
        base_vertex: u32,
        instance_count: u32,
        base_instance: u32,
    ) {
        if let Some(ref mut cmds) = self.gfx_cmds {
            cmds.bind_vertex_buffers(&[vertex_buffer.clone()], &[0u64]);
            cmds.draw_indexed(
                index_buffer,
                &HgiDrawIndexedOp {
                    index_count,
                    base_index: index_buffer_byte_offset,
                    base_vertex: base_vertex as i32,
                    instance_count,
                    base_instance,
                },
            );
        }
    }

    // -----------------------------------------------------------------------
    // Resource management helpers
    // -----------------------------------------------------------------------

    /// Get the Hgi instance.
    pub fn get_hgi(&self) -> Arc<RwLock<dyn Hgi>> {
        self.hgi.clone()
    }

    /// Destroy a shader program and all its shader functions.
    ///
    /// Mirrors C++ `_DestroyShaderProgram`.
    pub fn destroy_shader_program(&mut self, shader_program: &mut Option<HgiShaderProgramHandle>) {
        if let Some(program) = shader_program.take() {
            let mut hgi = self.hgi.write();
            // Destroy each shader function first
            if let Some(prog) = program.get() {
                for fn_handle in &prog.descriptor().shader_functions {
                    hgi.destroy_shader_function(fn_handle);
                }
            }
            hgi.destroy_shader_program(&program);
        }
    }

    /// Get the debug name.
    pub fn get_debug_name(&self) -> &str {
        &self.debug_name
    }

    /// Set up a shared fullscreen pass configuration.
    ///
    /// Configures the effects shader as a fullscreen triangle pass with:
    /// - No vertex buffers (3 vertices generated in vertex shader from gl_VertexID)
    /// - Triangle list primitive type
    /// - Depth test disabled, depth write disabled
    /// - No backface culling (fullscreen quad is always front-facing)
    /// - Color attachment as output
    pub fn setup_fullscreen_pass(
        &mut self,
        color_attachments: Vec<HgiAttachmentDesc>,
        depth_attachment: Option<HgiAttachmentDesc>,
    ) {
        // Fullscreen triangle: no vertex input needed
        self.pipeline_desc.vertex_buffers = Vec::new();
        self.pipeline_desc.primitive_type = HgiPrimitiveType::TriangleList;

        self.pipeline_desc.color_attachments = color_attachments;
        self.pipeline_desc.depth_attachment = depth_attachment;

        // Depth: off for fullscreen passes (they composite, not occlude)
        let mut depth_state = HgiDepthStencilState::default();
        depth_state.depth_test_enabled = false;
        depth_state.depth_write_enabled = false;
        self.pipeline_desc.depth_stencil_state = depth_state;

        // Rasterization: no culling for fullscreen triangle
        let mut raster_state = HgiRasterizationState::default();
        raster_state.cull_mode = usd_hgi::HgiCullMode::None;
        self.pipeline_desc.rasterization_state = raster_state;
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Create graphics pipeline if needed, updating attachment formats from textures.
    ///
    /// Mirrors C++ `_CreatePipeline`.
    fn create_pipeline(
        &mut self,
        color_textures: &[HgiTextureHandle],
        color_resolve_textures: &[HgiTextureHandle],
        depth_texture: &HgiTextureHandle,
        depth_resolve_texture: &HgiTextureHandle,
    ) {
        // Check if existing pipeline already matches the textures' format + sample count
        if self.pipeline.is_some() {
            let sc = self.pipeline_desc.multi_sample_state.sample_count;
            let color_match =
                formats_match(color_textures, &self.pipeline_desc.color_attachments, sc);
            let resolve_match = formats_match(
                color_resolve_textures,
                &self.pipeline_desc.color_attachments,
                HgiSampleCount::Count1,
            );
            let depth_match = format_matches_attachment(
                depth_texture,
                self.pipeline_desc.depth_attachment.as_ref(),
                sc,
            );
            let depth_resolve_match = format_matches_attachment(
                depth_resolve_texture,
                self.pipeline_desc.depth_attachment.as_ref(),
                HgiSampleCount::Count1,
            );
            if color_match && resolve_match && depth_match && depth_resolve_match {
                return; // still valid
            }
            self.destroy_pipeline();
        }

        // Update sample count from the actual textures
        let sample_count = if !color_textures.is_empty() {
            color_textures[0]
                .get()
                .map(|t| t.descriptor().sample_count)
                .unwrap_or(HgiSampleCount::Count1)
        } else {
            depth_texture
                .get()
                .map(|t| t.descriptor().sample_count)
                .unwrap_or(HgiSampleCount::Count1)
        };
        self.pipeline_desc.multi_sample_state.sample_count = sample_count;
        self.pipeline_desc.multi_sample_state.multi_sample_enable =
            sample_count != HgiSampleCount::Count1;

        // Update attachment format/usage from the actual color textures
        update_attachment_formats(color_textures, &mut self.pipeline_desc.color_attachments);

        // Update depth attachment format/usage from the depth texture
        if let Some(ref mut depth_att) = self.pipeline_desc.depth_attachment {
            update_single_attachment(depth_texture, depth_att);
        }

        // Mark resolve attachments when resolve textures are provided
        let has_resolve = (!color_resolve_textures.is_empty()
            && color_resolve_textures[0].is_valid())
            || depth_resolve_texture.is_valid();
        self.pipeline_desc.resolve_attachments = has_resolve;

        // Create the pipeline
        let pipeline = {
            let mut hgi = self.hgi.write();
            hgi.create_graphics_pipeline(&self.pipeline_desc)
        };
        self.pipeline = Some(pipeline);
    }

    /// Destroy the graphics pipeline via HGI.
    fn destroy_pipeline(&mut self) {
        if let Some(pipeline) = self.pipeline.take() {
            let mut hgi = self.hgi.write();
            hgi.destroy_graphics_pipeline(&pipeline);
        }
    }

    /// Create resource bindings if not already created.
    ///
    /// Mirrors C++ `_CreateResourceBindings`.
    fn create_resource_bindings(&mut self) {
        if self.resource_bindings.is_some() {
            return;
        }
        let bindings = {
            let mut hgi = self.hgi.write();
            hgi.create_resource_bindings(&self.resource_bindings_desc)
        };
        self.resource_bindings = Some(bindings);
    }

    /// Destroy resource bindings via HGI.
    fn destroy_resource_bindings(&mut self) {
        if let Some(bindings) = self.resource_bindings.take() {
            let mut hgi = self.hgi.write();
            hgi.destroy_resource_bindings(&bindings);
        }
    }
}

impl Drop for HdxEffectsShader {
    fn drop(&mut self) {
        self.destroy_pipeline();
        self.destroy_resource_bindings();
    }
}

// ---------------------------------------------------------------------------
// Attachment comparison helpers (mirrors C++ static helpers)
// ---------------------------------------------------------------------------

/// Copy an attachment descriptor stripping format (which comes from the texture).
fn partial_attachment_copy(desc: &HgiAttachmentDesc) -> HgiAttachmentDesc {
    use usd_hgi::HgiFormat;
    let mut out = desc.clone();
    // format is texture-derived — ignore it for comparison purposes
    out.format = HgiFormat::Invalid;
    out
}

/// Compare two attachment descriptors ignoring texture-derived fields.
fn attachment_match(a: &HgiAttachmentDesc, b: &HgiAttachmentDesc) -> bool {
    partial_attachment_copy(a) == partial_attachment_copy(b)
}

/// Compare two attachment descriptor slices ignoring texture-derived fields.
fn attachments_match(a: &[HgiAttachmentDesc], b: &[HgiAttachmentDesc]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b.iter()).all(|(x, y)| attachment_match(x, y))
}

/// Copy multi-sample state stripping the texture-derived fields.
fn partial_multisample_copy(s: &HgiMultiSampleState) -> HgiMultiSampleState {
    let mut out = s.clone();
    out.multi_sample_enable = false;
    out.sample_count = HgiSampleCount::Count1;
    out
}

// ---------------------------------------------------------------------------
// Format/sample-count helpers for pipeline caching
// ---------------------------------------------------------------------------

/// Check whether a set of textures matches the formats/sample count in their
/// corresponding attachment descriptors.
fn formats_match(
    textures: &[HgiTextureHandle],
    attachments: &[HgiAttachmentDesc],
    sample_count: HgiSampleCount,
) -> bool {
    if textures.len() != attachments.len() {
        return false;
    }
    for (tex, att) in textures.iter().zip(attachments.iter()) {
        if !format_matches_attachment(tex, Some(att), sample_count) {
            return false;
        }
    }
    true
}

/// Check whether a single texture matches a single attachment descriptor.
fn format_matches_attachment(
    texture: &HgiTextureHandle,
    attachment: Option<&HgiAttachmentDesc>,
    sample_count: HgiSampleCount,
) -> bool {
    use usd_hgi::HgiFormat;
    match (texture.get(), attachment) {
        (Some(t), Some(att)) => {
            let td = t.descriptor();
            att.format == td.format && sample_count == td.sample_count
        }
        (None, Some(att)) => att.format == HgiFormat::Invalid,
        (None, None) => true,
        (Some(_), None) => false,
    }
}

/// Update attachment descriptors' format/usage from actual textures.
fn update_attachment_formats(textures: &[HgiTextureHandle], descs: &mut Vec<HgiAttachmentDesc>) {
    for (tex, desc) in textures.iter().zip(descs.iter_mut()) {
        update_single_attachment(tex, desc);
    }
}

/// Update a single attachment descriptor's format from a texture.
fn update_single_attachment(texture: &HgiTextureHandle, desc: &mut HgiAttachmentDesc) {
    use usd_hgi::HgiFormat;
    if let Some(t) = texture.get() {
        desc.format = t.descriptor().format;
    } else {
        desc.format = HgiFormat::Invalid;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use usd_hgi::*;

    // Minimal mock HGI for unit tests

    struct MockGraphicsCmds;
    impl HgiCmds for MockGraphicsCmds {
        fn is_submitted(&self) -> bool {
            false
        }
        fn push_debug_group(&mut self, _: &str) {}
        fn pop_debug_group(&mut self) {}
        fn insert_debug_marker(&mut self, _: &str) {}
    }
    impl HgiGraphicsCmds for MockGraphicsCmds {
        fn bind_pipeline(&mut self, _: &HgiGraphicsPipelineHandle) {}
        fn bind_resources(&mut self, _: &HgiResourceBindingsHandle) {}
        fn bind_vertex_buffers(&mut self, _: &[HgiBufferHandle], _: &[u64]) {}
        fn set_viewport(&mut self, _: &HgiViewport) {}
        fn set_scissor(&mut self, _: &HgiScissor) {}
        fn set_blend_constant_color(&mut self, _: &usd_gf::Vec4f) {}
        fn set_stencil_reference_value(&mut self, _: u32) {}
        fn draw(&mut self, _: &HgiDrawOp) {}
        fn draw_indexed(&mut self, _: &HgiBufferHandle, _: &HgiDrawIndexedOp) {}
        fn draw_indirect(&mut self, _: &HgiDrawIndirectOp) {}
        fn draw_indexed_indirect(&mut self, _: &HgiBufferHandle, _: &HgiDrawIndirectOp) {}
        fn memory_barrier(&mut self, _: HgiMemoryBarrier) {}
    }

    struct MockBlitCmds;
    impl HgiCmds for MockBlitCmds {
        fn is_submitted(&self) -> bool {
            false
        }
        fn push_debug_group(&mut self, _: &str) {}
        fn pop_debug_group(&mut self) {}
        fn insert_debug_marker(&mut self, _: &str) {}
    }
    impl HgiBlitCmds for MockBlitCmds {
        fn copy_buffer_cpu_to_gpu(&mut self, _: &HgiBufferCpuToGpuOp) {}
        fn copy_buffer_gpu_to_gpu(&mut self, _: &HgiBufferGpuToGpuOp) {}
        fn copy_texture_cpu_to_gpu(&mut self, _: &HgiTextureCpuToGpuOp) {}
        fn copy_texture_gpu_to_gpu(&mut self, _: &HgiTextureGpuToGpuOp) {}
        fn copy_texture_gpu_to_cpu(&mut self, _: &HgiTextureGpuToCpuOp) {}
        fn copy_buffer_to_texture(&mut self, _: &HgiBufferToTextureOp) {}
        fn copy_texture_to_buffer(&mut self, _: &HgiTextureToBufferOp) {}
        fn generate_mipmap(&mut self, _: &HgiTextureHandle) {}
        fn fill_buffer(&mut self, _: &HgiBufferHandle, _: u8) {}
    }

    struct MockComputeCmds;
    impl HgiCmds for MockComputeCmds {
        fn is_submitted(&self) -> bool {
            false
        }
        fn push_debug_group(&mut self, _: &str) {}
        fn pop_debug_group(&mut self) {}
        fn insert_debug_marker(&mut self, _: &str) {}
    }
    impl HgiComputeCmds for MockComputeCmds {
        fn bind_pipeline(&mut self, _: &HgiComputePipelineHandle) {}
        fn bind_resources(&mut self, _: &HgiResourceBindingsHandle) {}
        fn dispatch(&mut self, _: &HgiComputeDispatchOp) {}
        fn memory_barrier(&mut self, _: HgiMemoryBarrier) {}
    }

    struct MockHgi {
        counter: AtomicU64,
        caps: HgiCapabilities,
    }
    impl MockHgi {
        fn new() -> Arc<RwLock<dyn Hgi>> {
            Arc::new(RwLock::new(Self {
                counter: AtomicU64::new(1),
                caps: HgiCapabilities::default(),
            }))
        }
    }
    impl Hgi for MockHgi {
        fn is_backend_supported(&self) -> bool {
            true
        }
        fn capabilities(&self) -> &HgiCapabilities {
            &self.caps
        }
        fn create_buffer(&mut self, _: &HgiBufferDesc, _: Option<&[u8]>) -> HgiBufferHandle {
            HgiHandle::null()
        }
        fn create_texture(&mut self, _: &HgiTextureDesc, _: Option<&[u8]>) -> HgiTextureHandle {
            HgiHandle::null()
        }
        fn create_sampler(&mut self, _: &HgiSamplerDesc) -> HgiSamplerHandle {
            HgiHandle::null()
        }
        fn create_shader_function(&mut self, _: &HgiShaderFunctionDesc) -> HgiShaderFunctionHandle {
            HgiHandle::null()
        }
        fn create_shader_program(&mut self, _: &HgiShaderProgramDesc) -> HgiShaderProgramHandle {
            HgiHandle::null()
        }
        fn create_resource_bindings(
            &mut self,
            _: &HgiResourceBindingsDesc,
        ) -> HgiResourceBindingsHandle {
            HgiHandle::null()
        }
        fn create_graphics_pipeline(
            &mut self,
            _: &HgiGraphicsPipelineDesc,
        ) -> HgiGraphicsPipelineHandle {
            HgiHandle::null()
        }
        fn create_compute_pipeline(
            &mut self,
            _: &HgiComputePipelineDesc,
        ) -> HgiComputePipelineHandle {
            HgiHandle::null()
        }
        fn destroy_buffer(&mut self, _: &HgiBufferHandle) {}
        fn destroy_texture(&mut self, _: &HgiTextureHandle) {}
        fn destroy_sampler(&mut self, _: &HgiSamplerHandle) {}
        fn destroy_shader_function(&mut self, _: &HgiShaderFunctionHandle) {}
        fn destroy_shader_program(&mut self, _: &HgiShaderProgramHandle) {}
        fn destroy_resource_bindings(&mut self, _: &HgiResourceBindingsHandle) {}
        fn destroy_graphics_pipeline(&mut self, _: &HgiGraphicsPipelineHandle) {}
        fn destroy_compute_pipeline(&mut self, _: &HgiComputePipelineHandle) {}
        fn create_blit_cmds(&mut self) -> Box<dyn HgiBlitCmds> {
            Box::new(MockBlitCmds)
        }
        fn create_graphics_cmds(&mut self, _: &HgiGraphicsCmdsDesc) -> Box<dyn HgiGraphicsCmds> {
            Box::new(MockGraphicsCmds)
        }
        fn create_compute_cmds(&mut self, _: &HgiComputeCmdsDesc) -> Box<dyn HgiComputeCmds> {
            Box::new(MockComputeCmds)
        }
        fn submit_cmds(&mut self, _: Box<dyn HgiCmds>, _: HgiSubmitWaitType) {}
        fn unique_id(&mut self) -> u64 {
            self.counter.fetch_add(1, Ordering::SeqCst)
        }
        fn wait_for_idle(&mut self) {}
        fn get_api_name(&self) -> &str {
            "Mock"
        }
        fn start_frame(&mut self) {}
        fn end_frame(&mut self) {}
        fn garbage_collect(&mut self) {}
    }

    #[test]
    fn test_resource_bindings_desc() {
        let desc = HgiResourceBindingsDesc::default();
        assert!(desc.buffer_bindings.is_empty());
        assert!(desc.texture_bindings.is_empty());
    }

    #[test]
    fn test_pipeline_desc() {
        let desc = HgiGraphicsPipelineDesc::default();
        assert_eq!(desc.primitive_type, HgiPrimitiveType::TriangleList);
    }

    #[test]
    fn test_new_sets_debug_name() {
        let hgi = MockHgi::new();
        let s = HdxEffectsShader::new(hgi, "TestShader".to_string());
        assert_eq!(s.get_debug_name(), "TestShader");
    }

    #[test]
    fn test_new_empty_debug_name_uses_default() {
        let hgi = MockHgi::new();
        let s = HdxEffectsShader::new(hgi, String::new());
        assert_eq!(s.get_debug_name(), "HdxEffectsShader");
    }

    #[test]
    fn test_set_shader_constants_stores_data() {
        let hgi = MockHgi::new();
        let mut s = HdxEffectsShader::new(hgi, "Test".to_string());
        let data = [1u8, 2, 3, 4];
        s.set_shader_constants(4, &data, HgiShaderStage::FRAGMENT);
        assert_eq!(s.constants_data, &[1, 2, 3, 4]);
        assert_eq!(s.pipeline_desc.shader_constants_desc.byte_size, 4);
        assert_eq!(
            s.pipeline_desc.shader_constants_desc.stage_usage,
            HgiShaderStage::FRAGMENT
        );
    }

    #[test]
    fn test_set_shader_constants_pipeline_invalidated_on_size_change() {
        let hgi = MockHgi::new();
        let mut s = HdxEffectsShader::new(hgi, "Test".to_string());
        let data = [0u8; 8];
        s.set_shader_constants(4, &data, HgiShaderStage::VERTEX);
        // Change size: pipeline should be invalidated (pipeline is None so no crash)
        s.set_shader_constants(8, &data, HgiShaderStage::VERTEX);
        assert_eq!(s.constants_data.len(), 8);
    }

    #[test]
    fn test_setup_fullscreen_pass() {
        let hgi = MockHgi::new();
        let mut s = HdxEffectsShader::new(hgi, "FS".to_string());
        s.setup_fullscreen_pass(vec![], None);
        assert_eq!(
            s.pipeline_desc.primitive_type,
            HgiPrimitiveType::TriangleList
        );
        assert!(!s.pipeline_desc.depth_stencil_state.depth_test_enabled);
        assert!(!s.pipeline_desc.depth_stencil_state.depth_write_enabled);
    }

    #[test]
    fn test_create_and_submit_with_null_textures() {
        // Should not panic even with null (invalid) texture handles
        let hgi = MockHgi::new();
        let mut s = HdxEffectsShader::new(hgi, "Submit".to_string());
        let null = HgiTextureHandle::null();
        let vp = Vec4i::new(0, 0, 1920, 1080);
        s.create_and_submit_graphics_cmds(&[], &[], &null, &null, &vp);
    }
}
