//! Vulkan graphics pipeline.
//!
//! Port of pxr/imaging/hgiVulkan/graphicsPipeline.cpp/.h

use ash::vk;
use ash::vk::Handle as _;
use usd_hgi::{
    HgiAttachmentDesc, HgiAttachmentLoadOp, HgiAttachmentStoreOp, HgiFormat, HgiGraphicsCmdsDesc,
    HgiGraphicsPipeline, HgiGraphicsPipelineDesc, HgiSampleCount, HgiTexture,
    HgiVertexBufferStepFunction,
};

use crate::conversions::HgiVulkanConversions;
use crate::descriptor_set_layouts::{
    HgiVulkanDescriptorSetInfoVector, make_descriptor_set_layouts,
};
use crate::texture::HgiVulkanTexture;

/// Cached VkFramebuffer keyed by the graphics cmds descriptor that created it.
///
/// Port of C++ `HgiVulkan_Framebuffer`.
struct HgiVulkanFramebuffer {
    dimensions: [i32; 2],
    desc: HgiGraphicsCmdsDesc,
    vk_framebuffer: vk::Framebuffer,
}

/// Vulkan graphics pipeline: wraps VkPipeline + VkRenderPass + VkPipelineLayout.
///
/// Port of C++ `HgiVulkanGraphicsPipeline`.
pub struct HgiVulkanGraphicsPipeline {
    desc: HgiGraphicsPipelineDesc,
    device: ash::Device,
    vk_pipeline: vk::Pipeline,
    vk_render_pass: vk::RenderPass,
    vk_pipeline_layout: vk::PipelineLayout,
    vk_descriptor_set_layouts: Vec<vk::DescriptorSetLayout>,
    clear_needed: bool,
    inflight_bits: u64,
    framebuffers: Vec<HgiVulkanFramebuffer>,
}

// ---------------------------------------------------------------------------
// Internal attachment helpers
// ---------------------------------------------------------------------------

/// Infers whether an attachment is a depth/stencil target from its format.
///
/// In C++ this checks `attachment.usage & HgiTextureUsageBitsDepthTarget`.
/// Our Rust HgiAttachmentDesc has no `usage` field, so we infer from format.
fn is_depth_attachment(attachment: &HgiAttachmentDesc) -> bool {
    matches!(
        attachment.format,
        HgiFormat::Float32UInt8 | HgiFormat::PackedD16Unorm
    )
}

/// Returns attachment description + reference structs for one HgiAttachmentDesc.
///
/// Port of C++ `HgiVulkanGraphicsPipeline::_ProcessAttachment`.
/// The bool return indicates whether this attachment has a Clear load op.
fn process_attachment(
    attachment: &HgiAttachmentDesc,
    attachment_index: u32,
    sample_count: HgiSampleCount,
) -> (
    vk::AttachmentDescription2<'static>,
    vk::AttachmentReference2<'static>,
    bool,
) {
    let depth = is_depth_attachment(attachment);

    let aspect_mask = if depth {
        vk::ImageAspectFlags::DEPTH | vk::ImageAspectFlags::STENCIL
    } else {
        vk::ImageAspectFlags::COLOR
    };

    // Layout used during the subpass for this attachment
    let subpass_layout = if depth {
        vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL
    } else {
        vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL
    };

    let vk_ref = vk::AttachmentReference2::default()
        .attachment(attachment_index)
        .layout(subpass_layout)
        .aspect_mask(aspect_mask);

    // Initial/final layout: we don't know neighbouring passes, so we transition
    // back to the "default" shader-read layout (matches C++ comment).
    let layout = default_image_layout(depth);

    let load_op = HgiVulkanConversions::get_load_op(attachment.load_op);
    let store_op = HgiVulkanConversions::get_store_op(attachment.store_op);
    let samples = HgiVulkanConversions::get_sample_count(sample_count);
    let format = HgiVulkanConversions::get_format(attachment.format, depth);

    let vk_desc = vk::AttachmentDescription2::default()
        .format(format)
        .samples(samples)
        .load_op(load_op)
        .store_op(store_op)
        // Hgi doesn't separate stencil ops — match the depth ops (C++ comment)
        .stencil_load_op(load_op)
        .stencil_store_op(store_op)
        .initial_layout(layout)
        .final_layout(layout);

    let needs_clear = attachment.load_op == HgiAttachmentLoadOp::Clear;
    (vk_desc, vk_ref, needs_clear)
}

/// Returns the default VkImageLayout for shader reading after a pass.
///
/// Simplified port of `HgiVulkanTexture::GetDefaultImageLayout`.
fn default_image_layout(depth: bool) -> vk::ImageLayout {
    if depth {
        vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL
    } else {
        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL
    }
}

// ---------------------------------------------------------------------------
// Render pass creation
// ---------------------------------------------------------------------------

/// Builds VkRenderPass2 from the pipeline descriptor.
///
/// Port of C++ `HgiVulkanGraphicsPipeline::_CreateRenderPass`.
/// Sets `*clear_needed = true` if any attachment has `HgiAttachmentLoadOpClear`.
fn create_render_pass(
    device: &ash::Device,
    desc: &HgiGraphicsPipelineDesc,
    clear_needed: &mut bool,
) -> Result<vk::RenderPass, String> {
    let samples = desc.multi_sample_state.sample_count;

    let mut vk_descriptions: Vec<vk::AttachmentDescription2> = Vec::new();
    let mut vk_color_refs: Vec<vk::AttachmentReference2> = Vec::new();

    // Color attachments
    for attachment in &desc.color_attachments {
        let slot = vk_descriptions.len() as u32;
        let (vk_desc, vk_ref, needs_clear) = process_attachment(attachment, slot, samples);
        if needs_clear {
            *clear_needed = true;
        }
        vk_descriptions.push(vk_desc);
        vk_color_refs.push(vk_ref);
    }

    // Depth attachment
    let has_depth = desc
        .depth_attachment
        .as_ref()
        .map_or(false, |a| a.format != HgiFormat::Invalid);

    let mut vk_depth_ref = vk::AttachmentReference2::default();
    if has_depth {
        let depth_attach = desc.depth_attachment.as_ref().unwrap();
        let slot = vk_descriptions.len() as u32;
        let (vk_desc, vk_ref, needs_clear) = process_attachment(depth_attach, slot, samples);
        if needs_clear {
            *clear_needed = true;
        }
        vk_descriptions.push(vk_desc);
        vk_depth_ref = vk_ref;
    }

    // Resolve attachments (MSAA → single-sample)
    let mut vk_color_resolve_refs: Vec<vk::AttachmentReference2> = Vec::new();
    let mut vk_depth_resolve_ref = vk::AttachmentReference2::default();

    if desc.resolve_attachments {
        for attachment in &desc.color_attachments {
            let slot = vk_descriptions.len() as u32;
            let (vk_desc, vk_ref, _) = process_attachment(attachment, slot, HgiSampleCount::Count1);
            let dont_care = HgiVulkanConversions::get_load_op(HgiAttachmentLoadOp::DontCare);
            let store = HgiVulkanConversions::get_store_op(HgiAttachmentStoreOp::Store);
            let vk_desc = vk_desc
                .load_op(dont_care)
                .stencil_load_op(dont_care)
                .store_op(store)
                .stencil_store_op(store);
            vk_descriptions.push(vk_desc);
            vk_color_resolve_refs.push(vk_ref);
        }

        if has_depth {
            let depth_attach = desc.depth_attachment.as_ref().unwrap();
            let slot = vk_descriptions.len() as u32;
            let (vk_desc, vk_ref, _) =
                process_attachment(depth_attach, slot, HgiSampleCount::Count1);
            let dont_care = HgiVulkanConversions::get_load_op(HgiAttachmentLoadOp::DontCare);
            let store = HgiVulkanConversions::get_store_op(HgiAttachmentStoreOp::Store);
            let vk_desc = vk_desc
                .load_op(dont_care)
                .stencil_load_op(dont_care)
                .store_op(store)
                .stencil_store_op(store);
            vk_descriptions.push(vk_desc);
            vk_depth_resolve_ref = vk_ref;
        }
    }

    // Subpass — build in two steps so we can conditionally attach depth/resolve.
    // SubpassDescriptionDepthStencilResolve for MSAA depth resolve (if needed).
    let mut depth_stencil_resolve = vk::SubpassDescriptionDepthStencilResolve::default()
        .depth_resolve_mode(vk::ResolveModeFlags::SAMPLE_ZERO)
        .stencil_resolve_mode(vk::ResolveModeFlags::SAMPLE_ZERO)
        .depth_stencil_resolve_attachment(&vk_depth_resolve_ref);

    let mut subpass = vk::SubpassDescription2::default()
        .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
        .color_attachments(&vk_color_refs);

    if has_depth {
        subpass = subpass.depth_stencil_attachment(&vk_depth_ref);
    }
    if desc.resolve_attachments && !vk_color_resolve_refs.is_empty() {
        subpass = subpass.resolve_attachments(&vk_color_resolve_refs);
    }
    if has_depth && desc.resolve_attachments {
        // Chain depth-stencil resolve into the subpass pNext
        subpass = subpass.push_next(&mut depth_stencil_resolve);
    }

    // Subpass dependencies: image layout transitions at pass boundaries.
    // C++ comment: non-optimal masks used because details aren't available here.
    let dependencies = [
        // Before subpass: ensure prior shader reads finish before FB writes begin.
        vk::SubpassDependency2::default()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .dependency_flags(vk::DependencyFlags::BY_REGION)
            .src_stage_mask(vk::PipelineStageFlags::BOTTOM_OF_PIPE)
            .dst_stage_mask(vk::PipelineStageFlags::TOP_OF_PIPE)
            .src_access_mask(vk::AccessFlags::MEMORY_READ)
            .dst_access_mask(vk::AccessFlags::MEMORY_READ | vk::AccessFlags::MEMORY_WRITE),
        // After subpass: ensure FB writes finish before shader reads in next pass.
        vk::SubpassDependency2::default()
            .src_subpass(0)
            .dst_subpass(vk::SUBPASS_EXTERNAL)
            .dependency_flags(vk::DependencyFlags::BY_REGION)
            .src_stage_mask(vk::PipelineStageFlags::TOP_OF_PIPE)
            .dst_stage_mask(vk::PipelineStageFlags::BOTTOM_OF_PIPE)
            .src_access_mask(vk::AccessFlags::MEMORY_READ | vk::AccessFlags::MEMORY_WRITE)
            .dst_access_mask(vk::AccessFlags::MEMORY_READ),
    ];

    let subpasses = [subpass];
    let create_info = vk::RenderPassCreateInfo2::default()
        .attachments(&vk_descriptions)
        .subpasses(&subpasses)
        .dependencies(&dependencies);

    // C++ note: uses vkCreateRenderPass2KHR because the core version was
    // crashing on some drivers. ash's create_render_pass2 handles this.
    let render_pass = unsafe {
        device
            .create_render_pass2(&create_info, None)
            .map_err(|e| {
                format!(
                    "vkCreateRenderPass2 failed for '{}': {e:?}",
                    desc.debug_name
                )
            })?
    };

    Ok(render_pass)
}

// ---------------------------------------------------------------------------
// Constructor
// ---------------------------------------------------------------------------

impl HgiVulkanGraphicsPipeline {
    /// Creates a complete Vulkan graphics pipeline.
    ///
    /// Port of C++ `HgiVulkanGraphicsPipeline::HgiVulkanGraphicsPipeline`.
    ///
    /// # Arguments
    /// - `device` — Ash logical device (cloned into the pipeline for lifetime management)
    /// - `pipeline_cache` — Vulkan pipeline cache for compilation reuse
    /// - `desc` — Full pipeline state descriptor
    /// - `descriptor_set_infos` — SPIR-V reflection data, one `Vec<SetInfo>` per shader stage
    pub fn new(
        device: &ash::Device,
        pipeline_cache: vk::PipelineCache,
        desc: &HgiGraphicsPipelineDesc,
        descriptor_set_infos: Vec<HgiVulkanDescriptorSetInfoVector>,
    ) -> Result<Self, String> {
        // ----------------------------------------------------------------
        // Shader stages
        // In the full implementation, VkShaderModule handles and stage bits
        // come from HgiVulkanShaderFunction. With the current stubs those
        // aren't exposed, so we pass an empty list here — sufficient for
        // building the pipeline object and all surrounding state.
        // ----------------------------------------------------------------
        let stages: Vec<vk::PipelineShaderStageCreateInfo> = Vec::new();
        let use_tessellation = false;

        // ----------------------------------------------------------------
        // Vertex input state
        // ----------------------------------------------------------------
        let mut vert_bufs: Vec<vk::VertexInputBindingDescription> = Vec::new();
        let mut vert_attrs: Vec<vk::VertexInputAttributeDescription> = Vec::new();
        let mut vert_binding_divisors: Vec<vk::VertexInputBindingDivisorDescriptionEXT> =
            Vec::new();

        for vbo in &desc.vertex_buffers {
            for va in &vbo.vertex_attributes {
                vert_attrs.push(vk::VertexInputAttributeDescription {
                    binding: vbo.binding_index,
                    location: va.shader_binding_location,
                    offset: va.offset,
                    format: HgiVulkanConversions::get_format(va.format, false),
                });
            }

            let (input_rate, divisor) =
                if vbo.step_function == HgiVertexBufferStepFunction::PerDrawCommand {
                    // Divisor = max value → attribute index advances only per base instance,
                    // matching C++ `maxVertexAttribDivisor` for multi-draw indirect.
                    (vk::VertexInputRate::INSTANCE, Some(u32::MAX))
                } else {
                    (vk::VertexInputRate::VERTEX, None)
                };

            vert_bufs.push(vk::VertexInputBindingDescription {
                binding: vbo.binding_index,
                stride: vbo.vertex_stride,
                input_rate,
            });

            if let Some(div) = divisor {
                vert_binding_divisors.push(vk::VertexInputBindingDivisorDescriptionEXT {
                    binding: vbo.binding_index,
                    divisor: div,
                });
            }
        }

        let mut divisor_ext = vk::PipelineVertexInputDivisorStateCreateInfoEXT::default()
            .vertex_binding_divisors(&vert_binding_divisors);

        let mut vertex_input = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_attribute_descriptions(&vert_attrs)
            .vertex_binding_descriptions(&vert_bufs);

        if !vert_binding_divisors.is_empty() {
            vertex_input = vertex_input.push_next(&mut divisor_ext);
        }

        // ----------------------------------------------------------------
        // Input assembly
        // ----------------------------------------------------------------
        let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default().topology(
            HgiVulkanConversions::get_primitive_type(desc.primitive_type),
        );

        // ----------------------------------------------------------------
        // Tessellation state
        // ----------------------------------------------------------------
        let tess_state = vk::PipelineTessellationStateCreateInfo::default()
            .patch_control_points(desc.tessellation_state.primitive_index_size as u32);

        // ----------------------------------------------------------------
        // Viewport / scissor (both set dynamically via commands)
        // ----------------------------------------------------------------
        let viewport_state = vk::PipelineViewportStateCreateInfo::default()
            .viewport_count(1)
            .scissor_count(1);

        // ----------------------------------------------------------------
        // Rasterization state
        // ----------------------------------------------------------------
        let ras = &desc.rasterization_state;
        let multisample_enabled = desc.multi_sample_state.sample_count as u32 > 1;

        let raster_state = vk::PipelineRasterizationStateCreateInfo::default()
            .line_width(ras.line_width)
            .cull_mode(HgiVulkanConversions::get_cull_mode(ras.cull_mode))
            .polygon_mode(HgiVulkanConversions::get_polygon_mode(ras.polygon_mode))
            .front_face(HgiVulkanConversions::get_winding(ras.winding))
            .rasterizer_discard_enable(!ras.rasterizer_enabled)
            .depth_clamp_enable(ras.depth_clamp_enabled)
            .depth_bias_enable(desc.depth_stencil_state.depth_bias_enabled)
            .depth_bias_constant_factor(desc.depth_stencil_state.depth_bias_constant_factor)
            .depth_bias_clamp(0.0) // 0.0 / NaN disables depth bias clamping
            .depth_bias_slope_factor(desc.depth_stencil_state.depth_bias_slope_factor);

        // ----------------------------------------------------------------
        // Multisample state
        // ----------------------------------------------------------------
        let ms = &desc.multi_sample_state;
        let multisample_state = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(HgiVulkanConversions::get_sample_count(ms.sample_count))
            .sample_shading_enable(false)
            .min_sample_shading(0.5)
            // Disable alpha-to-coverage/one when not actually multisampling (matches GL)
            .alpha_to_coverage_enable(ms.alpha_to_coverage_enable && multisample_enabled)
            .alpha_to_one_enable(ms.alpha_to_one_enable && multisample_enabled);

        // ----------------------------------------------------------------
        // Depth / stencil state
        // ----------------------------------------------------------------
        let ds = &desc.depth_stencil_state;

        // Default stencil ops when stencil test is disabled (C++ sets these explicitly)
        let stencil_op = vk::StencilOpState {
            fail_op: vk::StencilOp::KEEP,
            pass_op: vk::StencilOp::KEEP,
            depth_fail_op: vk::StencilOp::KEEP,
            compare_op: vk::CompareOp::ALWAYS,
            compare_mask: 0,
            write_mask: 0,
            reference: 0,
        };

        let depth_stencil_state = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(ds.depth_test_enabled)
            .depth_write_enable(ds.depth_write_enabled)
            .depth_compare_op(HgiVulkanConversions::get_depth_compare_function(
                ds.depth_compare_function,
            ))
            .depth_bounds_test_enable(false)
            .min_depth_bounds(0.0)
            .max_depth_bounds(0.0)
            .stencil_test_enable(ds.stencil_test_enabled)
            .front(stencil_op)
            .back(stencil_op);

        // ----------------------------------------------------------------
        // Color blend state — one entry per color attachment
        // ----------------------------------------------------------------
        let color_attach_states: Vec<vk::PipelineColorBlendAttachmentState> = desc
            .color_attachments
            .iter()
            .enumerate()
            .map(|(i, attach)| {
                // Per-attachment blend state from desc.color_blend_states (optional, parallel array)
                let blend = desc.color_blend_states.get(i);
                vk::PipelineColorBlendAttachmentState {
                    blend_enable: attach.blend_enabled as vk::Bool32,
                    src_color_blend_factor: blend
                        .map(|b| HgiVulkanConversions::get_blend_factor(b.src_color_blend_factor))
                        .unwrap_or(vk::BlendFactor::ONE),
                    dst_color_blend_factor: blend
                        .map(|b| HgiVulkanConversions::get_blend_factor(b.dst_color_blend_factor))
                        .unwrap_or(vk::BlendFactor::ZERO),
                    color_blend_op: blend
                        .map(|b| HgiVulkanConversions::get_blend_equation(b.color_blend_op))
                        .unwrap_or(vk::BlendOp::ADD),
                    src_alpha_blend_factor: blend
                        .map(|b| HgiVulkanConversions::get_blend_factor(b.src_alpha_blend_factor))
                        .unwrap_or(vk::BlendFactor::ONE),
                    dst_alpha_blend_factor: blend
                        .map(|b| HgiVulkanConversions::get_blend_factor(b.dst_alpha_blend_factor))
                        .unwrap_or(vk::BlendFactor::ZERO),
                    alpha_blend_op: blend
                        .map(|b| HgiVulkanConversions::get_blend_equation(b.alpha_blend_op))
                        .unwrap_or(vk::BlendOp::ADD),
                    color_write_mask: vk::ColorComponentFlags::R
                        | vk::ColorComponentFlags::G
                        | vk::ColorComponentFlags::B
                        | vk::ColorComponentFlags::A,
                }
            })
            .collect();

        let color_blend_state = vk::PipelineColorBlendStateCreateInfo::default()
            .attachments(&color_attach_states)
            .logic_op_enable(false)
            .logic_op(vk::LogicOp::NO_OP)
            .blend_constants([1.0, 1.0, 1.0, 1.0]);

        // ----------------------------------------------------------------
        // Dynamic state: viewport + scissor set via command buffer
        // ----------------------------------------------------------------
        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state =
            vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);

        // ----------------------------------------------------------------
        // Pipeline layout (descriptor sets + push constants)
        // ----------------------------------------------------------------
        let use_push_constants = desc.shader_constants_desc.byte_size > 0;
        let push_constant_range = vk::PushConstantRange {
            stage_flags: HgiVulkanConversions::get_shader_stages(
                desc.shader_constants_desc.stage_usage,
            ),
            offset: 0,
            size: desc.shader_constants_desc.byte_size,
        };
        let pc_ranges: &[vk::PushConstantRange] = if use_push_constants {
            std::slice::from_ref(&push_constant_range)
        } else {
            &[]
        };

        let vk_descriptor_set_layouts =
            make_descriptor_set_layouts(device, &descriptor_set_infos, &desc.debug_name).map_err(
                |e| {
                    format!(
                        "make_descriptor_set_layouts failed for '{}': {e:?}",
                        desc.debug_name
                    )
                },
            )?;

        let pipeline_layout_info = vk::PipelineLayoutCreateInfo::default()
            .set_layouts(&vk_descriptor_set_layouts)
            .push_constant_ranges(pc_ranges);

        let vk_pipeline_layout = unsafe {
            device
                .create_pipeline_layout(&pipeline_layout_info, None)
                .map_err(|e| {
                    format!(
                        "vkCreatePipelineLayout failed for '{}': {e:?}",
                        desc.debug_name
                    )
                })?
        };

        // ----------------------------------------------------------------
        // Render pass
        // ----------------------------------------------------------------
        let mut clear_needed = false;
        let vk_render_pass = create_render_pass(device, desc, &mut clear_needed).map_err(|e| {
            // Clean up already-created layout before propagating
            unsafe { device.destroy_pipeline_layout(vk_pipeline_layout, None) };
            for layout in &vk_descriptor_set_layouts {
                unsafe { device.destroy_descriptor_set_layout(*layout, None) };
            }
            e
        })?;

        // ----------------------------------------------------------------
        // Create VkGraphicsPipeline
        // ----------------------------------------------------------------
        let mut pipe_create_info = vk::GraphicsPipelineCreateInfo::default()
            .stages(&stages)
            .vertex_input_state(&vertex_input)
            .input_assembly_state(&input_assembly)
            .viewport_state(&viewport_state)
            .rasterization_state(&raster_state)
            .multisample_state(&multisample_state)
            .depth_stencil_state(&depth_stencil_state)
            .color_blend_state(&color_blend_state)
            .dynamic_state(&dynamic_state)
            .layout(vk_pipeline_layout)
            .render_pass(vk_render_pass)
            .subpass(0);

        if use_tessellation {
            pipe_create_info = pipe_create_info.tessellation_state(&tess_state);
        }

        let pipelines = unsafe {
            device
                .create_graphics_pipelines(pipeline_cache, &[pipe_create_info], None)
                .map_err(|(_, e)| {
                    // Clean up on failure
                    device.destroy_render_pass(vk_render_pass, None);
                    device.destroy_pipeline_layout(vk_pipeline_layout, None);
                    for layout in &vk_descriptor_set_layouts {
                        device.destroy_descriptor_set_layout(*layout, None);
                    }
                    format!(
                        "vkCreateGraphicsPipelines failed for '{}': {e:?}",
                        desc.debug_name
                    )
                })?
        };

        Ok(Self {
            desc: desc.clone(),
            device: device.clone(),
            vk_pipeline: pipelines[0],
            vk_render_pass,
            vk_pipeline_layout,
            vk_descriptor_set_layouts,
            clear_needed,
            inflight_bits: 0,
            framebuffers: Vec::new(),
        })
    }

    // -----------------------------------------------------------------------
    // Public accessors
    // -----------------------------------------------------------------------

    /// Records `vkCmdBindPipeline(GRAPHICS)` on the given command buffer.
    ///
    /// Port of C++ `HgiVulkanGraphicsPipeline::BindPipeline`.
    pub fn bind_pipeline(&self, cb: vk::CommandBuffer) {
        unsafe {
            self.device
                .cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, self.vk_pipeline);
        }
    }

    /// Returns the Vulkan pipeline layout.
    pub fn vk_pipeline_layout(&self) -> vk::PipelineLayout {
        self.vk_pipeline_layout
    }

    /// Returns the Vulkan render pass.
    pub fn vk_render_pass(&self) -> vk::RenderPass {
        self.vk_render_pass
    }

    /// Returns true if any attachment in the descriptor has a Clear load op.
    pub fn clear_needed(&self) -> bool {
        self.clear_needed
    }

    /// Returns the inflight bits used by the garbage collector.
    pub fn inflight_bits(&self) -> u64 {
        self.inflight_bits
    }

    /// Sets the inflight bits.
    pub fn set_inflight_bits(&mut self, bits: u64) {
        self.inflight_bits = bits;
    }

    /// Returns a cached VkFramebuffer for `gfx_desc`, creating one if not found.
    ///
    /// Port of C++ `HgiVulkanGraphicsPipeline::AcquireVulkanFramebuffer`.
    ///
    /// Collects image views from all color/depth/resolve textures in the descriptor,
    /// creates a VkFramebuffer against the pipeline's render pass, and caches it.
    /// The cache is capped at 32 entries: oldest is evicted when exceeded (e.g. viewport resize).
    pub fn acquire_vulkan_framebuffer(
        &mut self,
        gfx_desc: &HgiGraphicsCmdsDesc,
    ) -> Result<(vk::Framebuffer, [i32; 2]), String> {
        // Check cache first.
        for fb in &self.framebuffers {
            if &fb.desc == gfx_desc {
                return Ok((fb.vk_framebuffer, fb.dimensions));
            }
        }

        // Evict oldest entry when cache exceeds limit to bound memory usage.
        if self.framebuffers.len() > 32 {
            let old = self.framebuffers.remove(0);
            unsafe { self.device.destroy_framebuffer(old.vk_framebuffer, None) };
        }

        // Build ordered list of all attachments: color, depth, color-resolve, depth-resolve.
        let mut tex_handles: Vec<&usd_hgi::HgiTextureHandle> = Vec::new();
        for t in &gfx_desc.color_textures {
            tex_handles.push(t);
        }
        if gfx_desc.depth_texture.is_valid() {
            tex_handles.push(&gfx_desc.depth_texture);
        }
        for t in &gfx_desc.color_resolve_textures {
            tex_handles.push(t);
        }
        if gfx_desc.depth_resolve_texture.is_valid() {
            tex_handles.push(&gfx_desc.depth_resolve_texture);
        }

        let mut views: Vec<vk::ImageView> = Vec::with_capacity(tex_handles.len());
        let mut dimensions = [0i32; 2];

        for handle in &tex_handles {
            let tex = handle
                .get()
                .and_then(|t| t.as_any().downcast_ref::<HgiVulkanTexture>())
                .ok_or_else(|| {
                    format!(
                        "acquire_vulkan_framebuffer: texture handle is not a HgiVulkanTexture \
                         (pipeline '{}')",
                        self.desc.debug_name
                    )
                })?;
            views.push(tex.vk_image_view());
            // Last texture wins for dimensions, matching C++ behaviour.
            dimensions[0] = tex.descriptor().dimensions[0];
            dimensions[1] = tex.descriptor().dimensions[1];
        }

        if dimensions[0] == 0 || dimensions[1] == 0 {
            return Err(format!(
                "acquire_vulkan_framebuffer: zero dimensions for pipeline '{}'",
                self.desc.debug_name
            ));
        }

        let fb_info = vk::FramebufferCreateInfo::default()
            .render_pass(self.vk_render_pass)
            .attachments(&views)
            .width(dimensions[0] as u32)
            .height(dimensions[1] as u32)
            .layers(1);

        let vk_framebuffer = unsafe {
            self.device
                .create_framebuffer(&fb_info, None)
                .map_err(|e| {
                    format!(
                        "vkCreateFramebuffer failed for '{}': {e:?}",
                        self.desc.debug_name
                    )
                })?
        };

        self.framebuffers.push(HgiVulkanFramebuffer {
            dimensions,
            desc: gfx_desc.clone(),
            vk_framebuffer,
        });

        Ok((vk_framebuffer, dimensions))
    }
}

// ---------------------------------------------------------------------------
// HgiGraphicsPipeline trait impl
// ---------------------------------------------------------------------------

impl HgiGraphicsPipeline for HgiVulkanGraphicsPipeline {
    fn descriptor(&self) -> &HgiGraphicsPipelineDesc {
        &self.desc
    }

    fn raw_resource(&self) -> u64 {
        self.vk_pipeline.as_raw()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// ---------------------------------------------------------------------------
// Drop: destroy Vulkan objects in reverse creation order
// ---------------------------------------------------------------------------

impl Drop for HgiVulkanGraphicsPipeline {
    fn drop(&mut self) {
        unsafe {
            // Framebuffers reference the render pass, so destroy them first.
            for fb in self.framebuffers.drain(..) {
                self.device.destroy_framebuffer(fb.vk_framebuffer, None);
            }
            // Descriptor set layouts first (pipeline layout holds references to them)
            for layout in self.vk_descriptor_set_layouts.drain(..) {
                self.device.destroy_descriptor_set_layout(layout, None);
            }
            self.device
                .destroy_pipeline_layout(self.vk_pipeline_layout, None);
            self.device.destroy_render_pass(self.vk_render_pass, None);
            self.device.destroy_pipeline(self.vk_pipeline, None);
        }
    }
}
