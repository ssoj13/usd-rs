//! Skydome background rendering methods for the UsdImagingGL Engine.
//!
//! Extracted from engine.rs. Contains:
//!   - skydome_wgsl()            — WGSL shader source (fullscreen env cubemap)
//!   - ensure_skydome_pipeline() — lazy pipeline creation
//!   - get_dome_light_inv_transform() — dome light xform from render index
//!   - render_skydome()          — execute the skydome render pass

use super::Engine;

#[cfg(feature = "wgpu")]
use usd_gf::Matrix4d;
#[cfg(feature = "wgpu")]
use usd_hd_st::light::HdStLight;
#[cfg(feature = "wgpu")]
use usd_hgi::texture::HgiTextureHandle;
#[cfg(feature = "wgpu")]
use usd_hgi_wgpu::conversions::to_wgpu_texture_format;
#[cfg(feature = "wgpu")]
use usd_tf::Token;

impl Engine {
    // -------------------------------------------------------------------------
    // Skydome background rendering
    // -------------------------------------------------------------------------

    /// WGSL source for the skydome fullscreen triangle shader.
    #[cfg(feature = "wgpu")]
    pub(super) fn skydome_wgsl() -> &'static str {
        r#"
// Skydome background: fullscreen triangle sampling environment cubemap.
// Vertex shader generates clip-space positions from vertex_index (0,1,2).
// Fragment shader reconstructs world-space ray direction from UV via
// inverse projection and inverse view matrices, then samples cubemap.

struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) uv: vec2f,
};

struct FragOutput {
    @location(0) color: vec4f,
    @builtin(frag_depth) depth: f32,
};

@vertex
fn vs_main(@builtin(vertex_index) id: u32) -> VertexOutput {
    // Oversize triangle covering entire screen (verts at -1..3)
    let uv = vec2f(f32((id << 1u) & 2u), f32(id & 2u));
    var out: VertexOutput;
    out.position = vec4f(uv * 2.0 - 1.0, 1.0, 1.0); // depth = 1.0 (far plane)
    out.uv = uv; // pass raw UV, no Y-flip (matches C++ skydome.glslfx)
    return out;
}

struct SkyUniforms {
    inv_proj: mat4x4f,
    inv_view: mat4x4f,
    light_transform: mat4x4f,
};

@group(0) @binding(0) var<uniform> u: SkyUniforms;
@group(0) @binding(1) var env_map: texture_cube<f32>;
@group(0) @binding(2) var env_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> FragOutput {
    // UV -> NDC -> clip -> view-space ray (C++ skydome.glslfx:30-36)
    let ndc = in.uv * 2.0 - 1.0;
    let clip = vec4f(ndc.x, ndc.y, 1.0, 1.0);
    let view_pos = u.inv_proj * clip;
    let dir_view = normalize(view_pos.xyz);
    // Apply camera rotation + dome light transform (C++ skydome.glslfx:39-40)
    let dir_world = (u.light_transform * u.inv_view * vec4f(dir_view, 0.0)).xyz;
    let color = textureSample(env_map, env_sampler, dir_world).rgb;
    var out: FragOutput;
    out.color = vec4f(color, 1.0);
    out.depth = 1.0; // far plane (C++ gl_FragDepth = farPlane)
    return out;
}
"#
    }

    /// Lazily create the skydome render pipeline and uniform buffer.
    #[cfg(feature = "wgpu")]
    pub(super) fn ensure_skydome_pipeline(
        &mut self,
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
    ) {
        if self.skydome_pipeline.is_some()
            && self.skydome_pipeline_color_format == Some(color_format)
        {
            return;
        }

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("skydome_shader"),
            source: wgpu::ShaderSource::Wgsl(Self::skydome_wgsl().into()),
        });

        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("skydome_bgl"),
            entries: &[
                // binding 0: SkyUniforms (inv_proj + inv_view + light_transform)
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(192), // 3 * mat4x4f
                    },
                    count: None,
                },
                // binding 1: env cubemap texture
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::Cube,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 2: sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("skydome_pipeline_layout"),
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("skydome_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[], // No vertex buffers, positions from vertex_index
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // Fullscreen triangle, no culling
                ..Default::default()
            },
            // Write depth = 1.0 so geometry renders in front
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Always, // Always pass
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Uniform buffer: 3 * mat4x4<f32> = 192 bytes
        let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("skydome_uniforms"),
            size: 192,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        self.skydome_bind_group_layout = Some(bgl);
        self.skydome_pipeline = Some(pipeline);
        self.skydome_pipeline_color_format = Some(color_format);
        self.skydome_uniform_buf = Some(uniform_buf);
        log::info!("[engine] Skydome pipeline created for {:?}", color_format);
    }

    /// Get the inverse transform of the first dome light in the scene.
    /// Returns identity if no dome light found.
    /// Port of C++ skydomeTask.cpp:121-137.
    #[cfg(feature = "wgpu")]
    pub(super) fn get_dome_light_inv_transform(&self) -> Matrix4d {
        let index = match &self.render_index {
            Some(idx) => idx.clone(),
            None => return Matrix4d::identity(),
        };
        let guard = index.lock().expect("Mutex poisoned");
        let dome_tok = Token::new("domeLight");
        for id in guard.get_sprim_ids_for_type(&dome_tok) {
            if let Some(handle) = guard.get_sprim(&dome_tok, &id) {
                if let Some(hd_light) = handle.downcast_ref::<HdStLight>() {
                    if let Some(glf) = hd_light.get_simple_light() {
                        if glf.is_dome_light() {
                            let xform = glf.get_transform();
                            return xform.inverse().unwrap_or_else(Matrix4d::identity);
                        }
                    }
                }
            }
        }
        Matrix4d::identity()
    }

    /// Render the environment cubemap as a fullscreen background.
    ///
    /// Runs BEFORE the main geometry pass. Clears color + depth,
    /// draws a fullscreen triangle at depth=1.0 (far plane).
    /// Geometry pass afterwards uses Load ops to preserve this background.
    #[cfg(feature = "wgpu")]
    pub(super) fn render_skydome(
        &mut self,
        color_tex: &HgiTextureHandle,
        depth_tex: &HgiTextureHandle,
    ) -> bool {
        // Need IBL env cubemap
        let ibl = match self.render_pass_state.get_ibl_handles() {
            Some(h) => h.clone(),
            None => return false,
        };

        let hgi_arc = match &self.wgpu_hgi {
            Some(h) => h.clone(),
            None => return false,
        };

        let hgi_r = hgi_arc.read();
        let device = hgi_r.device().clone();
        let queue = hgi_r.queue().clone();
        drop(hgi_r);

        let color_format = color_tex
            .get()
            .map(|texture| to_wgpu_texture_format(texture.descriptor().format))
            .unwrap_or(wgpu::TextureFormat::Rgba8Unorm);

        // Ensure pipeline exists
        self.ensure_skydome_pipeline(&device, color_format);
        let pipeline = match &self.skydome_pipeline {
            Some(p) => p,
            None => return false,
        };
        let bgl = self.skydome_bind_group_layout.as_ref().unwrap();
        let uniform_buf = self.skydome_uniform_buf.as_ref().unwrap();

        // Compute inverse projection and inverse view matrices (f32)
        // Use effective matrices (accounts for camera_framing_override)
        let proj = self.render_pass_state.get_projection_matrix();
        let view = self.render_pass_state.get_world_to_view_matrix();
        let inv_proj = match proj.inverse() {
            Some(m) => m,
            None => Matrix4d::identity(),
        };
        let inv_view = match view.inverse() {
            Some(m) => m,
            None => Matrix4d::identity(),
        };

        // Get dome light inverse transform (C++ skydomeTask.cpp:121-137)
        let mut light_transform = self.get_dome_light_inv_transform();

        // When upAxis="Z" (Blender scenes), the cubemap expects Y-up sampling
        // directions. Apply Z-to-Y rotation so the environment map renders
        // with correct orientation. Rotation: (x,y,z) → (x,z,-y).
        if let Some(stage) = self
            .scene_indices
            .as_ref()
            .and_then(|indices| indices.stage_scene_index.get_stage())
        {
            let up = usd_geom::get_stage_up_axis(&stage);
            if up.as_str() == "Z" {
                // Row-major Z-up → Y-up rotation (USD row-vector convention).
                let z_to_y = Matrix4d::from_array([
                    [1.0, 0.0, 0.0, 0.0],
                    [0.0, 0.0, -1.0, 0.0],
                    [0.0, 1.0, 0.0, 0.0],
                    [0.0, 0.0, 0.0, 1.0],
                ]);
                light_transform = light_transform * z_to_y;
            }
        }

        // Pack 3 matrices as raw row-major f32 bytes.
        // C++ dumps GfMatrix4f (row-major) directly into GPU buffer.
        // GLSL/WGSL mat4 reads column-major, so it sees the transpose.
        // This is correct: USD row-vector convention (v*M) becomes
        // column-vector (M^T * v) in the shader. (skydomeTask.cpp:308-310)
        let mut uniforms = [0.0f32; 48];
        for r in 0..4usize {
            let ip_row = inv_proj.row(r);
            uniforms[r * 4] = ip_row.x as f32;
            uniforms[r * 4 + 1] = ip_row.y as f32;
            uniforms[r * 4 + 2] = ip_row.z as f32;
            uniforms[r * 4 + 3] = ip_row.w as f32;
            let iv_row = inv_view.row(r);
            uniforms[16 + r * 4] = iv_row.x as f32;
            uniforms[16 + r * 4 + 1] = iv_row.y as f32;
            uniforms[16 + r * 4 + 2] = iv_row.z as f32;
            uniforms[16 + r * 4 + 3] = iv_row.w as f32;
            let lt_row = light_transform.row(r);
            uniforms[32 + r * 4] = lt_row.x as f32;
            uniforms[32 + r * 4 + 1] = lt_row.y as f32;
            uniforms[32 + r * 4 + 2] = lt_row.z as f32;
            uniforms[32 + r * 4 + 3] = lt_row.w as f32;
        }
        let uniform_bytes: Vec<u8> = uniforms.iter().flat_map(|f| f.to_le_bytes()).collect();
        queue.write_buffer(uniform_buf, 0, &uniform_bytes);

        // Resolve wgpu texture views for render targets
        let color_view = match usd_hgi_wgpu::resolve_texture_view(color_tex) {
            Some(v) => v,
            None => return false,
        };
        let depth_view = match usd_hgi_wgpu::resolve_texture_view(depth_tex) {
            Some(v) => v,
            None => return false,
        };

        // Get env cubemap raw wgpu::Texture for Cube view creation.
        // The default HGI view is D2Array (6 layers), but the shader needs
        // TextureViewDimension::Cube.
        let env_raw_tex = match usd_hgi_wgpu::resolve_texture(&ibl.env_cubemap_tex) {
            Some(t) => t,
            None => return false,
        };
        let cube_view = env_raw_tex.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });
        let env_sampler = match usd_hgi_wgpu::resolve_sampler(&ibl.env_cubemap_smp) {
            Some(s) => s,
            None => return false,
        };

        // Create bind group
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("skydome_bg"),
            layout: bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&cube_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(env_sampler),
                },
            ],
        });

        // Render pass: Clear color + depth, draw fullscreen triangle
        let clear_color = self.render_pass_state.get_clear_color();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("skydome_encoder"),
        });
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("skydome_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: clear_color[0] as f64,
                            g: clear_color[1] as f64,
                            b: clear_color[2] as f64,
                            a: clear_color[3] as f64,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            let (vx, vy, vw, vh) = self.render_pass_state.get_viewport();
            rpass.set_viewport(vx, vy, vw, vh, 0.0, 1.0);
            rpass.set_pipeline(pipeline);
            rpass.set_bind_group(0, &bind_group, &[]);
            rpass.draw(0..3, 0..1); // Fullscreen triangle, 3 vertices
        }
        queue.submit(std::iter::once(encoder.finish()));
        log::trace!("[engine] Skydome background rendered");
        true
    }
}
