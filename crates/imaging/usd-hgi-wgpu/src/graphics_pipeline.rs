//! wgpu graphics pipeline implementation for HGI.
//!
//! Maps HgiGraphicsPipelineDesc to wgpu::RenderPipeline, including vertex
//! state, fragment state, primitive topology, depth/stencil, multisample,
//! and color blend configuration.

use usd_hgi::graphics_pipeline::{HgiGraphicsPipeline, HgiGraphicsPipelineDesc};
use usd_hgi::shader_function::HgiShaderFunction;

use super::conversions;
use super::shader_function::WgpuShaderFunction;

/// Max push constants size in bytes (128 bytes = 8 vec4s, typical GPU minimum)
pub(crate) const MAX_PUSH_CONSTANTS_SIZE: u32 = 128;

/// wgpu-backed graphics (render) pipeline.
///
/// Created from HgiGraphicsPipelineDesc by translating all HGI state into
/// a wgpu::RenderPipeline. The pipeline is immutable after creation.
/// Stores the bind group layout extracted from shader reflection for
/// resource bindings compatibility.
pub struct WgpuGraphicsPipeline {
    desc: HgiGraphicsPipelineDesc,
    /// None when created via new_stub (HGI trait path without shader modules)
    pipeline: Option<wgpu::RenderPipeline>,
    /// Bind group layouts derived from shader via auto-layout reflection
    bind_group_layouts: Vec<wgpu::BindGroupLayout>,
}

impl WgpuGraphicsPipeline {
    /// Create a new wgpu render pipeline from an HGI descriptor.
    ///
    /// Uses a two-pass approach (same as compute pipeline):
    /// 1. Probe pipeline with auto-layout to extract bind group layouts via reflection.
    /// 2. Build explicit PipelineLayout with push_constant_ranges + extracted BGLs.
    /// 3. Create final pipeline with the explicit layout.
    ///
    /// This ensures push constants work and BGLs are always valid (not empty).
    /// Returns a pipeline with `pipeline: None` if creation fails.
    pub fn new(
        device: &wgpu::Device,
        desc: &HgiGraphicsPipelineDesc,
        vertex_module: &WgpuShaderFunction,
        fragment_module: Option<&WgpuShaderFunction>,
    ) -> Self {
        let label = if desc.debug_name.is_empty() {
            None
        } else {
            Some(desc.debug_name.as_str())
        };

        // -- Vertex buffers and attributes --
        let (vb_layouts, attr_storage) = build_vertex_state(desc);

        // Build wgpu VertexBufferLayout refs that borrow from attr_storage
        let vertex_buffers: Vec<wgpu::VertexBufferLayout<'_>> = vb_layouts
            .iter()
            .enumerate()
            .map(|(i, vbl)| wgpu::VertexBufferLayout {
                array_stride: vbl.array_stride,
                step_mode: vbl.step_mode,
                attributes: &attr_storage[i],
            })
            .collect();

        // -- Vertex entry point --
        let vs_entry = &vertex_module.descriptor().entry_point;

        // -- Fragment state --
        let color_targets = build_color_targets(desc);
        let color_target_opts: Vec<Option<wgpu::ColorTargetState>> =
            color_targets.into_iter().map(Some).collect();

        let mut fs_entry = String::new();
        let fragment_state = fragment_module.map(|fm| {
            fs_entry = fm.descriptor().entry_point.clone();
            wgpu::FragmentState {
                module: fm.wgpu_module(),
                entry_point: Some(&fs_entry),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &color_target_opts,
            }
        });

        // -- Primitive state --
        let primitive = wgpu::PrimitiveState {
            topology: conversions::to_wgpu_primitive_topology(desc.primitive_type),
            strip_index_format: None,
            front_face: conversions::to_wgpu_front_face(desc.rasterization_state.winding),
            cull_mode: conversions::to_wgpu_cull_mode(desc.rasterization_state.cull_mode),
            unclipped_depth: desc.rasterization_state.depth_clamp_enabled,
            polygon_mode: conversions::to_wgpu_polygon_mode(desc.rasterization_state.polygon_mode),
            conservative: desc.rasterization_state.conservative_raster,
        };

        // -- Depth/stencil state --
        let depth_stencil = build_depth_stencil(desc);

        // -- Multisample state --
        let multisample = wgpu::MultisampleState {
            count: conversions::to_wgpu_sample_count(desc.multi_sample_state.sample_count),
            mask: !0,
            alpha_to_coverage_enabled: desc.multi_sample_state.alpha_to_coverage_enable,
        };

        // Pass 1: probe pipeline with auto-layout to extract bind group layouts via reflection.
        // Use push_error_scope to suppress wgpu's fatal error handler during probe.
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let temp_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("temp_graphics_layout_probe"),
            layout: None, // auto-layout: wgpu infers BGLs from shader
            vertex: wgpu::VertexState {
                module: vertex_module.wgpu_module(),
                entry_point: Some(vs_entry.as_str()),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &vertex_buffers,
            },
            primitive,
            depth_stencil: depth_stencil.clone(),
            multisample,
            fragment: fragment_state.as_ref().map(|fs| wgpu::FragmentState {
                module: fs.module,
                entry_point: fs.entry_point,
                compilation_options: fs.compilation_options.clone(),
                targets: fs.targets,
            }),
            multiview: None,
            cache: None,
        });
        let probe_err = pollster::block_on(device.pop_error_scope());

        // Extract bind group layouts from the auto-layout pipeline.
        // Use the auto-layout pipeline itself — it already has the correct layout.
        // We just need to count how many groups the shader actually uses.
        // Collect bind group layouts from the auto-layout pipeline.
        // Use continue (not break) on error: groups may be sparse (e.g. 0, 2, 3 with no 1).
        // wgpu supports up to 8 bind groups; shaders may use group(4) for IBL textures.
        const MAX_BIND_GROUPS: u32 = 8;
        let mut bgls: Vec<Option<wgpu::BindGroupLayout>> = vec![None; MAX_BIND_GROUPS as usize];
        if probe_err.is_none() {
            for group_idx in 0..MAX_BIND_GROUPS {
                device.push_error_scope(wgpu::ErrorFilter::Validation);
                let bgl = temp_pipeline.get_bind_group_layout(group_idx);
                let err = pollster::block_on(device.pop_error_scope());
                if err.is_none() {
                    bgls[group_idx as usize] = Some(bgl);
                }
            }
        }
        // Preserve sparse BGL indices: wgpu pipeline layout bind groups must be
        // contiguous starting at 0, but shaders may use groups 0, 2 without group 1.
        // We fill gaps with an empty BGL so indices stay correct (P0-2 fix).
        let bgls: Vec<wgpu::BindGroupLayout> = {
            // Find the highest used group index.
            let max_used = bgls.iter().rposition(|b| b.is_some());
            match max_used {
                None => Vec::new(),
                Some(max_idx) => {
                    let mut out = Vec::with_capacity(max_idx + 1);
                    for bgl_opt in bgls.into_iter().take(max_idx + 1) {
                        match bgl_opt {
                            Some(bgl) => out.push(bgl),
                            None => {
                                // Fill gap with empty BGL for sparse group indices.
                                out.push(device.create_bind_group_layout(
                                    &wgpu::BindGroupLayoutDescriptor {
                                        label: Some("HgiWgpu empty BGL gap"),
                                        entries: &[],
                                    },
                                ));
                            }
                        }
                    }
                    out
                }
            }
        };

        // Build explicit pipeline layout with push constants + all extracted BGLs
        let push_constant_ranges = [wgpu::PushConstantRange {
            stages: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            range: 0..MAX_PUSH_CONSTANTS_SIZE,
        }];

        let bgl_refs: Vec<&wgpu::BindGroupLayout> = bgls.iter().collect();
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("HgiWgpu graphics pipeline layout"),
            bind_group_layouts: &bgl_refs,
            push_constant_ranges: &push_constant_ranges,
        });

        // Pass 2: create final pipeline with explicit layout (push constants now enabled).
        // Use push_error_scope to gracefully handle validation errors.
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let final_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: vertex_module.wgpu_module(),
                entry_point: Some(vs_entry.as_str()),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &vertex_buffers,
            },
            primitive,
            depth_stencil: depth_stencil.clone(),
            multisample,
            fragment: fragment_state.as_ref().map(|fs| wgpu::FragmentState {
                module: fs.module,
                entry_point: fs.entry_point,
                compilation_options: fs.compilation_options.clone(),
                targets: fs.targets,
            }),
            multiview: None,
            cache: None,
        });
        let pass2_err = pollster::block_on(device.pop_error_scope());

        let (pipeline, bind_group_layouts) = if let Some(e) = pass2_err {
            log::error!(
                "Failed to create graphics pipeline '{}': {}",
                desc.debug_name,
                e
            );
            (None, Vec::new())
        } else {
            (Some(final_pipeline), bgls)
        };

        Self {
            desc: desc.clone(),
            pipeline,
            bind_group_layouts,
        }
    }

    /// Access the inner wgpu::RenderPipeline for command encoding.
    pub(crate) fn wgpu_pipeline(&self) -> Option<&wgpu::RenderPipeline> {
        self.pipeline.as_ref()
    }

    /// Get bind group layouts derived from shader reflection.
    /// Resource bindings should use these layouts for compatibility.
    #[allow(dead_code)] // Used by HdSt integration
    pub(crate) fn bind_group_layouts(&self) -> &[wgpu::BindGroupLayout] {
        &self.bind_group_layouts
    }

    /// Get a specific bind group layout by index.
    #[allow(dead_code)]
    pub(crate) fn bind_group_layout(&self, index: usize) -> Option<&wgpu::BindGroupLayout> {
        self.bind_group_layouts.get(index)
    }
}

impl HgiGraphicsPipeline for WgpuGraphicsPipeline {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn descriptor(&self) -> &HgiGraphicsPipelineDesc {
        &self.desc
    }

    fn raw_resource(&self) -> u64 {
        // wgpu 24 removed global_id(); return 0 as placeholder
        0
    }
}

impl WgpuGraphicsPipeline {
    /// Create a stub pipeline without shaders (for HGI trait path).
    ///
    /// The HGI trait create_graphics_pipeline() doesn't pass concrete shader
    /// modules, so we store just the descriptor. Actual pipeline creation
    /// requires calling new() with shader modules.
    pub fn new_stub(desc: &HgiGraphicsPipelineDesc) -> Self {
        Self {
            desc: desc.clone(),
            pipeline: None,
            bind_group_layouts: Vec::new(),
        }
    }
}

// -- Internal helpers --

/// Intermediate vertex buffer layout (owns nothing, used to build descriptors).
struct VbLayout {
    array_stride: wgpu::BufferAddress,
    step_mode: wgpu::VertexStepMode,
}

/// Build vertex buffer layouts and attribute arrays from the HGI descriptor.
///
/// Returns a parallel pair: one VbLayout per vertex buffer, and one Vec of
/// wgpu::VertexAttribute per vertex buffer.
fn build_vertex_state(
    desc: &HgiGraphicsPipelineDesc,
) -> (Vec<VbLayout>, Vec<Vec<wgpu::VertexAttribute>>) {
    let mut vb_layouts = Vec::with_capacity(desc.vertex_buffers.len());
    let mut attr_storage = Vec::with_capacity(desc.vertex_buffers.len());

    for vb in &desc.vertex_buffers {
        let step_mode = conversions::to_wgpu_step_mode(vb.step_function);

        let attrs: Vec<wgpu::VertexAttribute> = vb
            .vertex_attributes
            .iter()
            .map(|attr| wgpu::VertexAttribute {
                format: conversions::to_wgpu_vertex_format(attr.format),
                offset: attr.offset as u64,
                shader_location: attr.shader_binding_location,
            })
            .collect();

        vb_layouts.push(VbLayout {
            array_stride: vb.vertex_stride as wgpu::BufferAddress,
            step_mode,
        });
        attr_storage.push(attrs);
    }

    (vb_layouts, attr_storage)
}

/// Build color targets from the descriptor's color attachments and blend states.
fn build_color_targets(desc: &HgiGraphicsPipelineDesc) -> Vec<wgpu::ColorTargetState> {
    desc.color_attachments
        .iter()
        .enumerate()
        .map(|(i, attachment)| {
            let blend = desc.color_blend_states.get(i).map(|bs| {
                if !bs.blend_enabled {
                    return None;
                }
                Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: conversions::to_wgpu_blend_factor(bs.src_color_blend_factor),
                        dst_factor: conversions::to_wgpu_blend_factor(bs.dst_color_blend_factor),
                        operation: conversions::to_wgpu_blend_op(bs.color_blend_op),
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: conversions::to_wgpu_blend_factor(bs.src_alpha_blend_factor),
                        dst_factor: conversions::to_wgpu_blend_factor(bs.dst_alpha_blend_factor),
                        operation: conversions::to_wgpu_blend_op(bs.alpha_blend_op),
                    },
                })
            });

            let write_mask = desc
                .color_blend_states
                .get(i)
                .map(|bs| conversions::to_wgpu_color_writes(bs.color_mask))
                .unwrap_or(wgpu::ColorWrites::ALL);

            wgpu::ColorTargetState {
                format: conversions::to_wgpu_texture_format(attachment.format),
                blend: blend.flatten(),
                write_mask,
            }
        })
        .collect()
}

/// Build optional depth/stencil state from the descriptor.
fn build_depth_stencil(desc: &HgiGraphicsPipelineDesc) -> Option<wgpu::DepthStencilState> {
    let depth_attachment = desc.depth_attachment.as_ref()?;

    // Determine depth format from the attachment
    let format = conversions::to_wgpu_depth_format(depth_attachment.format)
        .unwrap_or(wgpu::TextureFormat::Depth32Float);

    let ds = &desc.depth_stencil_state;

    let depth_compare = if ds.depth_test_enabled {
        conversions::to_wgpu_compare_fn(ds.depth_compare_function)
    } else {
        wgpu::CompareFunction::Always
    };

    let stencil = if ds.stencil_test_enabled {
        wgpu::StencilState {
            front: wgpu::StencilFaceState {
                compare: conversions::to_wgpu_compare_fn(ds.stencil_front.compare_function),
                fail_op: conversions::to_wgpu_stencil_op(ds.stencil_front.stencil_fail_op),
                depth_fail_op: conversions::to_wgpu_stencil_op(ds.stencil_front.depth_fail_op),
                pass_op: conversions::to_wgpu_stencil_op(ds.stencil_front.depth_stencil_pass_op),
            },
            back: wgpu::StencilFaceState {
                compare: conversions::to_wgpu_compare_fn(ds.stencil_back.compare_function),
                fail_op: conversions::to_wgpu_stencil_op(ds.stencil_back.stencil_fail_op),
                depth_fail_op: conversions::to_wgpu_stencil_op(ds.stencil_back.depth_fail_op),
                pass_op: conversions::to_wgpu_stencil_op(ds.stencil_back.depth_stencil_pass_op),
            },
            read_mask: ds.stencil_front.read_mask,
            write_mask: ds.stencil_front.write_mask,
        }
    } else {
        wgpu::StencilState::default()
    };

    Some(wgpu::DepthStencilState {
        format,
        depth_write_enabled: ds.depth_write_enabled,
        depth_compare,
        stencil,
        bias: wgpu::DepthBiasState {
            // Apply polygon offset when depth bias is enabled
            constant: if ds.depth_bias_enabled {
                ds.depth_bias_constant_factor as i32
            } else {
                0
            },
            slope_scale: if ds.depth_bias_enabled {
                ds.depth_bias_slope_factor
            } else {
                0.0
            },
            clamp: 0.0,
        },
    })
}
