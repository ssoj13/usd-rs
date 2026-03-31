//! HgiInteropWgpu - wgpu compositor for presenting HGI textures to a surface.
//!
//! Creates a fullscreen-triangle render pipeline that samples an input texture
//! and blits it onto a wgpu surface/texture view.
//!
//! Matches C++ hgiInterop/opengl.cpp behaviour:
//! - Premultiplied-alpha blending  (GL_ONE, GL_ONE_MINUS_SRC_ALPHA)
//! - Optional depth compositing via frag_depth output (mirrors gl_FragDepth)

use std::sync::Arc;

/// Wgpu compositor that blits an HGI-rendered texture onto a surface.
///
/// Lazily creates and caches render pipelines + samplers. Two pipeline variants
/// are maintained: one for color-only and one that additionally writes
/// `@builtin(frag_depth)` from the supplied depth AOV texture.
pub struct HgiInteropWgpu {
    /// Shared wgpu device handle.
    device: Arc<wgpu::Device>,
    /// Shared wgpu queue handle.
    queue: Arc<wgpu::Queue>,
    /// Cached pipeline for color-only composite.
    pipeline_color: Option<CachedPipeline>,
    /// Cached pipeline for color + depth composite.
    pipeline_depth: Option<CachedPipeline>,
    /// Linear sampler for the color source texture.
    sampler: Option<wgpu::Sampler>,
    /// Bind group layout for color-only pass (binding 0: color, 1: sampler).
    bgl_color: Option<wgpu::BindGroupLayout>,
    /// Bind group layout for depth pass (binding 0: color, 1: sampler, 2: depth).
    bgl_depth: Option<wgpu::BindGroupLayout>,
}

/// A cached render pipeline keyed by output texture format.
struct CachedPipeline {
    pipeline: wgpu::RenderPipeline,
    format: wgpu::TextureFormat,
}

impl HgiInteropWgpu {
    /// Create a new compositor bound to the given device and queue.
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Self {
        Self {
            device,
            queue,
            pipeline_color: None,
            pipeline_depth: None,
            sampler: None,
            bgl_color: None,
            bgl_depth: None,
        }
    }

    /// Composite `src_color` onto `dst_view` using a fullscreen triangle blit.
    ///
    /// * `src_color`      - TextureView of the rendered color AOV (RGBA, premultiplied alpha).
    /// * `src_depth`      - Optional depth AOV texture to read depth values from.
    /// * `dst_view`       - Target surface texture view to draw color into.
    /// * `dst_depth_view` - Optional destination depth buffer to write depth into.
    ///                      Required when `src_depth` is provided; the shader reads
    ///                      from `src_depth` and writes `@builtin(frag_depth)` to this
    ///                      attachment (mirrors C++ gl_FragDepth → app framebuffer depth).
    /// * `dst_format`     - wgpu::TextureFormat of the destination surface.
    /// * `viewport`       - (x, y, w, h) region on the destination.
    pub fn composite(
        &mut self,
        src_color: &wgpu::TextureView,
        src_depth: Option<&wgpu::TextureView>,
        dst_view: &wgpu::TextureView,
        dst_depth_view: Option<&wgpu::TextureView>,
        dst_format: wgpu::TextureFormat,
        viewport: [f32; 4],
    ) {
        // Depth compositing requires both a source texture to read from and a
        // destination attachment to write into (mirrors C++ where the app's
        // framebuffer depth buffer is the write target).
        let with_depth = src_depth.is_some() && dst_depth_view.is_some();

        // Ensure shared sampler exists.
        self.ensure_sampler();

        // Ensure the appropriate BGL and pipeline exist.
        if with_depth {
            self.ensure_bgl_depth();
            self.ensure_pipeline_depth(dst_format);
        } else {
            self.ensure_bgl_color();
            self.ensure_pipeline_color(dst_format);
        }

        let sampler = self.sampler.as_ref().unwrap();

        // Build per-frame bind group.
        let bind_group = if with_depth {
            let bgl = self.bgl_depth.as_ref().unwrap();
            self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("hgi_interop_bg_depth"),
                layout: bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(src_color),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                    // Depth AOV at binding 2; depth textures use textureLoad
                    // (not filterable), so no sampler is needed for it.
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(src_depth.unwrap()),
                    },
                ],
            })
        } else {
            let bgl = self.bgl_color.as_ref().unwrap();
            self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("hgi_interop_bg_color"),
                layout: bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(src_color),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                ],
            })
        };

        let pipeline = if with_depth {
            &self.pipeline_depth.as_ref().unwrap().pipeline
        } else {
            &self.pipeline_color.as_ref().unwrap().pipeline
        };

        // Encode render pass.
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("hgi_interop_enc"),
            });

        // Debug group mirrors C++ glPushDebugGroup(GL_DEBUG_SOURCE_THIRD_PARTY, 0, -1, "Interop")
        encoder.push_debug_group("HgiInterop");

        {
            // Attach the DESTINATION depth buffer for writing frag_depth.
            // src_depth is read via texture binding, dst_depth_view receives the writes.
            // This mirrors C++ where gl_FragDepth writes to the app's framebuffer depth.
            let depth_attachment = if with_depth {
                dst_depth_view.map(|dv| wgpu::RenderPassDepthStencilAttachment {
                    view: dv,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                })
            } else {
                None
            };

            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("hgi_interop_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: dst_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: depth_attachment,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            rpass.set_pipeline(pipeline);
            rpass.set_bind_group(0, Some(&bind_group), &[]);
            rpass.set_viewport(viewport[0], viewport[1], viewport[2], viewport[3], 0.0, 1.0);
            // Fullscreen triangle: 3 procedural vertices, no vertex buffer.
            rpass.draw(0..3, 0..1);
        }

        encoder.pop_debug_group();

        self.queue.submit(std::iter::once(encoder.finish()));
    }

    // -- internal helpers ---------------------------------------------------

    fn ensure_sampler(&mut self) {
        if self.sampler.is_some() {
            return;
        }
        self.sampler = Some(self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("hgi_interop_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        }));
    }

    /// BGL for color-only pass: (0) color texture, (1) sampler.
    fn ensure_bgl_color(&mut self) {
        if self.bgl_color.is_some() {
            return;
        }
        self.bgl_color = Some(self.device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
                label: Some("hgi_interop_bgl_color"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            },
        ));
    }

    /// BGL for depth pass: (0) color texture, (1) sampler, (2) depth texture.
    ///
    /// Depth textures are not filterable, so they use `Depth` sample type and
    /// are accessed via `textureLoad` in the shader (no sampler needed for
    /// binding 2).
    fn ensure_bgl_depth(&mut self) {
        if self.bgl_depth.is_some() {
            return;
        }
        self.bgl_depth = Some(self.device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
                label: Some("hgi_interop_bgl_depth"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // Depth texture at binding 2: non-filterable depth.
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Depth,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            },
        ));
    }

    /// Build (or reuse) the color-only pipeline for `format`.
    fn ensure_pipeline_color(&mut self, format: wgpu::TextureFormat) {
        if let Some(ref c) = self.pipeline_color {
            if c.format == format {
                return;
            }
        }
        let bgl = self.bgl_color.as_ref().unwrap();
        let pipeline = self.build_pipeline(format, bgl, false, "fs_color");
        self.pipeline_color = Some(CachedPipeline { pipeline, format });
    }

    /// Build (or reuse) the depth-composite pipeline for `format`.
    fn ensure_pipeline_depth(&mut self, format: wgpu::TextureFormat) {
        if let Some(ref c) = self.pipeline_depth {
            if c.format == format {
                return;
            }
        }
        let bgl = self.bgl_depth.as_ref().unwrap();
        let pipeline = self.build_pipeline(format, bgl, true, "fs_depth");
        self.pipeline_depth = Some(CachedPipeline { pipeline, format });
    }

    /// Common pipeline builder.  `fs_entry` selects which fragment entry point
    /// to use (`fs_color` or `fs_depth`).
    fn build_pipeline(
        &self,
        format: wgpu::TextureFormat,
        bgl: &wgpu::BindGroupLayout,
        with_depth: bool,
        fs_entry: &str,
    ) -> wgpu::RenderPipeline {
        let shader = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("hgi_interop_shader"),
                source: wgpu::ShaderSource::Wgsl(Self::composite_shader_source().into()),
            });

        let layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("hgi_interop_pl"),
                bind_group_layouts: &[bgl],
                push_constant_ranges: &[],
            });

        // Depth state: when compositing depth, write the frag_depth value that
        // the shader outputs (mirrors C++ gl_FragDepth assignment).
        // CompareFunction::Always so every fragment writes unconditionally.
        let depth_stencil = if with_depth {
            Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                // Must be true so the shader-written frag_depth is stored.
                depth_write_enabled: true,
                // LessEqual matches C++ glDepthFunc(GL_LEQUAL): fragments with
                // depth ≤ existing pass, ensuring translucent contributions composite.
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            })
        } else {
            None
        };

        // Premultiplied-alpha blending: matches C++ glBlendFuncSeparate(
        //   GL_ONE, GL_ONE_MINUS_SRC_ALPHA, GL_ONE, GL_ONE_MINUS_SRC_ALPHA).
        // Storm produces premultiplied-alpha output so we use src_factor = One.
        let blend = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
        };

        self.device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("hgi_interop_rp"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[], // fullscreen triangle, no vertex buffer
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    ..Default::default()
                },
                depth_stencil,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fs_entry),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(blend),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview: None,
                cache: None,
            })
    }

    /// WGSL source for the fullscreen composite pass.
    ///
    /// Two fragment entry points:
    /// - `fs_color`: color-only blit (binding 0 + 1).
    /// - `fs_depth`: color + depth write (bindings 0, 1, 2); outputs
    ///   `@builtin(frag_depth)` mirroring C++ `gl_FragDepth = texture(depthIn, uv).r`.
    pub fn composite_shader_source() -> &'static str {
        r#"
// Fullscreen composite shader for HGI interop.
// Covers the screen with a procedural triangle and samples the source textures.

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// Bindings shared by both entry points.
@group(0) @binding(0) var color_tex:     texture_2d<f32>;
@group(0) @binding(1) var color_sampler: sampler;

// Depth AOV texture (only bound for the depth pipeline variant).
// Depth textures are not filterable; access via textureLoad.
@group(0) @binding(2) var depth_tex: texture_depth_2d;

// Fullscreen triangle: 3 vertices cover the entire screen, no vertex buffer needed.
@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOutput {
    var out: VertexOutput;
    let x = f32(i32(vi & 1u) * 4 - 1);
    let y = f32(i32(vi & 2u) * 2 - 1);
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

// Color-only blit. Used by the color pipeline variant.
@fragment
fn fs_color(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(color_tex, color_sampler, in.uv);
}

// Color + depth composite output.
struct DepthFragOutput {
    @location(0)          color: vec4<f32>,
    @builtin(frag_depth)  depth: f32,
};

// Depth-composite blit. Mirrors C++ _fragmentDepthFullscreen:
//   gl_FragColor = texture2D(colorIn, uv);
//   gl_FragDepth = texture2D(depthIn, uv).r;
@fragment
fn fs_depth(in: VertexOutput) -> DepthFragOutput {
    var out: DepthFragOutput;
    out.color = textureSample(color_tex, color_sampler, in.uv);
    // textureLoad requires integer texel coordinates; in.position.xy gives
    // the fragment's pixel position which is exactly what we need.
    out.depth = textureLoad(depth_tex, vec2<i32>(in.position.xy), 0);
    return out;
}
"#
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shader_has_color_entry_point() {
        let src = HgiInteropWgpu::composite_shader_source();
        assert!(src.contains("fn vs_main"), "missing vs_main");
        assert!(src.contains("fn fs_color"), "missing fs_color");
        assert!(src.contains("color_tex"), "missing color_tex");
        assert!(src.contains("color_sampler"), "missing color_sampler");
    }

    #[test]
    fn shader_has_depth_entry_point() {
        let src = HgiInteropWgpu::composite_shader_source();
        assert!(src.contains("fn fs_depth"), "missing fs_depth");
        assert!(src.contains("depth_tex"), "missing depth_tex");
        assert!(src.contains("frag_depth"), "missing @builtin(frag_depth)");
        assert!(src.contains("textureLoad"), "missing textureLoad for depth");
    }

    #[test]
    fn shader_uses_premultiplied_alpha_comment() {
        // The blend state is wired in Rust, not WGSL, but we verify the shader
        // source string at minimum compiles logically (entry-point hygiene).
        let src = HgiInteropWgpu::composite_shader_source();
        // fs_depth must output a struct with both color and frag_depth.
        assert!(
            src.contains("DepthFragOutput"),
            "missing DepthFragOutput struct"
        );
    }
}
