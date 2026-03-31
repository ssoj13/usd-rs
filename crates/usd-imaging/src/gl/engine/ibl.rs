//! IBL (Image-Based Lighting) subsystem for the rendering engine.
//!
//! Contains:
//! - `IblGpuPipelines` — cached wgpu compute pipelines for GPU IBL prefiltering.
//! - `Engine::collect_dome_light_ibl` — find scene dome light and dispatch IBL computation.
//! - `Engine::compute_ibl_gpu` — GPU path: latlong->cubemap, irradiance, prefilter, BRDF LUT.
//! - CPU helper functions: `compute_ibl_cpu`, `upload_cubemap_texture`, `upload_2d_texture`,
//!   `generate_procedural_sky`, `cross`, `dot3`, `reflect_neg_v`.

use std::sync::Arc;
use parking_lot::RwLock;

#[cfg(feature = "wgpu")]
use usd_gf::Vec3i;
#[cfg(feature = "wgpu")]
use usd_hgi::hgi::Hgi;
#[cfg(feature = "wgpu")]
use usd_hgi::{
    enums::{HgiMipFilter, HgiSamplerAddressMode, HgiSamplerFilter, HgiTextureUsage},
    sampler::HgiSamplerDesc,
    texture::{HgiTextureDesc, HgiTextureHandle},
    types::HgiFormat,
};
use usd_hgi_wgpu::HgiWgpu;

use super::Engine;
#[cfg(feature = "wgpu")]
use super::texture::resolve_tex_path;

// =============================================================================
// IblGpuPipelines
// =============================================================================

/// Cached wgpu compute pipelines for GPU IBL prefiltering.
///
/// Created once on first dome light IBL dispatch, reused for subsequent
/// HDRI changes. Each pipeline corresponds to one dome light compute kernel.
#[cfg(feature = "wgpu")]
pub(super) struct IblGpuPipelines {
    /// Equirectangular latlong -> 6-face cubemap
    pub latlong_to_cubemap: wgpu::ComputePipeline,
    /// Cubemap -> diffuse irradiance cubemap
    pub irradiance_conv: wgpu::ComputePipeline,
    /// Cubemap -> GGX specular prefilter (per-roughness dispatch)
    pub prefilter_ggx: wgpu::ComputePipeline,
    /// BRDF split-sum integration LUT
    pub brdf_integration: wgpu::ComputePipeline,
    /// Mipmap blit: downsample one mip level of a 2D-array (cubemap) texture.
    /// C++ calls GenerateMipmaps() between latlong->cubemap and irradiance/prefilter.
    pub mipmap_blit: wgpu::RenderPipeline,
    /// BGL for the mipmap blit pass (sampled texture + sampler).
    pub mipmap_blit_bgl: wgpu::BindGroupLayout,
}

#[cfg(feature = "wgpu")]
impl IblGpuPipelines {
    /// Compile all 4 dome light compute pipelines from WGSL sources.
    pub fn new(device: &wgpu::Device) -> Option<Self> {
        use usd_hd_st::dome_light_computations::{BindingLayout, DomeLightCompType, wgsl_source};

        // Helper: compile a compute shader module from WGSL source.
        let compile_module = |comp: DomeLightCompType, entry: &str| -> Option<wgpu::ShaderModule> {
            let src = wgsl_source(comp);
            if src.is_empty() {
                return None;
            }

            device.push_error_scope(wgpu::ErrorFilter::Validation);
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(entry),
                source: wgpu::ShaderSource::Wgsl(src.into()),
            });
            let err = pollster::block_on(device.pop_error_scope());
            if let Some(e) = err {
                log::error!("[ibl_gpu] Shader compile error for {entry}: {e}");
                return None;
            }
            Some(module)
        };

        // Helper: create compute pipeline with auto-layout (no push constants).
        let create_auto = |module: &wgpu::ShaderModule, entry: &str| -> wgpu::ComputePipeline {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: None,
                module,
                entry_point: Some(entry),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            })
        };

        // Mipmap blit pipeline: fullscreen triangle that samples src mip -> writes dst mip.
        let blit_wgsl = r#"
struct VsOut { @builtin(position) pos: vec4f, @location(0) uv: vec2f };
@vertex fn vs(@builtin(vertex_index) id: u32) -> VsOut {
    let uv = vec2f(f32((id << 1u) & 2u), f32(id & 2u));
    return VsOut(vec4f(uv * 2.0 - 1.0, 0.0, 1.0), uv);
}
@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var src_smp: sampler;
@fragment fn fs(in: VsOut) -> @location(0) vec4f {
    return textureSampleLevel(src_tex, src_smp, in.uv, 0.0);
}
"#;
        let blit_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mipmap_blit"),
            source: wgpu::ShaderSource::Wgsl(blit_wgsl.into()),
        });
        let blit_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mipmap_blit_bgl"),
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
        });
        let blit_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mipmap_blit_layout"),
            bind_group_layouts: &[&blit_bgl],
            push_constant_ranges: &[],
        });
        let blit_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mipmap_blit"),
            layout: Some(&blit_layout),
            vertex: wgpu::VertexState {
                module: &blit_module,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &blit_module,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba16Float,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Compile shader modules
        let latlong_mod =
            compile_module(DomeLightCompType::EquirectToCubemap, "latlong_to_cubemap")?;
        let irr_mod = compile_module(DomeLightCompType::Irradiance, "irradiance_conv")?;
        let prefilter_mod =
            compile_module(DomeLightCompType::PrefilteredSpecular, "prefilter_ggx")?;
        let brdf_mod = compile_module(DomeLightCompType::BrdfLut, "brdf_integration")?;

        let build_compute_bgl = |layout: BindingLayout, label: &'static str| {
            let mut entries = Vec::new();
            let mut binding = 0u32;
            if layout.src_texture {
                entries.push(wgpu::BindGroupLayoutEntry {
                    binding,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        // The authored IBL WGSL currently samples cubemaps through
                        // `texture_2d_array`, not `texture_cube`.
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                });
                binding += 1;
            }
            if layout.src_sampler {
                entries.push(wgpu::BindGroupLayoutEntry {
                    binding,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                });
                binding += 1;
            }
            if layout.dst_texture_array {
                entries.push(wgpu::BindGroupLayoutEntry {
                    binding,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                    },
                    count: None,
                });
                binding += 1;
            }
            if layout.dst_texture_2d {
                entries.push(wgpu::BindGroupLayoutEntry {
                    binding,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rg16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                });
            }
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some(label),
                entries: &entries,
            })
        };

        // prefilter_ggx is the only IBL compute kernel that uses push constants.
        // Build its bind-group layout directly from the authored shader contract
        // so we do not need an invalid auto-layout probe pipeline at all.
        let prefilter_bgl = build_compute_bgl(
            BindingLayout {
                src_texture: true,
                src_sampler: true,
                dst_texture_array: true,
                dst_texture_2d: false,
                push_constant_roughness: true,
            },
            "prefilter_ggx_bgl",
        );
        let prefilter_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("prefilter_ggx_layout"),
            bind_group_layouts: &[&prefilter_bgl],
            push_constant_ranges: &[wgpu::PushConstantRange {
                stages: wgpu::ShaderStages::COMPUTE,
                range: 0..4, // f32 roughness
            }],
        });
        let prefilter_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("prefilter_ggx"),
            layout: Some(&prefilter_layout),
            module: &prefilter_mod,
            entry_point: Some("prefilter_ggx"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        Some(Self {
            latlong_to_cubemap: create_auto(&latlong_mod, "latlong_to_cubemap"),
            irradiance_conv: create_auto(&irr_mod, "irradiance_conv"),
            prefilter_ggx: prefilter_pipeline,
            brdf_integration: create_auto(&brdf_mod, "brdf_integration"),
            mipmap_blit: blit_pipeline,
            mipmap_blit_bgl: blit_bgl,
        })
    }
}

// =============================================================================
// Engine IBL methods
// =============================================================================

impl Engine {
    /// Find the first scene dome light with an HDRI texture, load/generate the
    /// IBL textures (GPU path first, CPU fallback), and push them to render pass state.
    pub(super) fn collect_dome_light_ibl(&mut self) {
        use usd_hd_st::render_pass_state::IblHandles;
        use usd_hgi::sampler::HgiSamplerDesc;
        use usd_hio::SourceColorSpace;
        use usd_hio::image_reader::read_image_data;

        // 1. Find first scene dome light with a texture file
        let scene_hdri_path = {
            let index = match &self.render_index {
                Some(i) => i.clone(),
                None => return,
            };
            let guard = index.lock().expect("Mutex poisoned");
            let dome_tok = usd_tf::Token::new("domeLight");
            let mut found_path: Option<String> = None;

            for id in guard.get_sprim_ids_for_type(&dome_tok) {
                if let Some(handle) = guard.get_sprim(&dome_tok, &id) {
                    if let Some(light) = handle.downcast_ref::<usd_hd_st::light::HdStLight>() {
                        if let Some(glf) = light.get_simple_light() {
                            let asset = glf.get_dome_light_texture_file();
                            let p = asset.get_resolved_path();
                            if p.is_empty() {
                                let up = asset.get_asset_path();
                                if !up.is_empty() {
                                    // Asset path is raw authored (e.g. "./sky.hdr") -- resolve
                                    // relative to the stage root layer directory.
                                    let resolved = self
                                        .scene_indices
                                        .as_ref()
                                        .and_then(|indices| {
                                            indices.stage_scene_index.get_stage()
                                        })
                                        .and_then(|s| resolve_tex_path(up, &s))
                                        .unwrap_or_else(|| up.to_string());
                                    found_path = Some(resolved);
                                    break;
                                }
                            } else {
                                found_path = Some(p.to_string());
                                break;
                            }
                        }
                    }
                }
            }
            found_path
        };

        // 1b. Validate inputs:texture:format -- only latlong/automatic supported.
        //     Angular and mirroredBall formats would produce wrong results if processed
        //     as equirectangular, so we warn and skip non-supported formats.
        if scene_hdri_path.is_some() {
            if let Some(stage) = self
                .scene_indices
                .as_ref()
                .and_then(|indices| indices.stage_scene_index.get_stage())
            {
                'fmt_check: for prim in stage.traverse() {
                    if prim.get_type_name() == "DomeLight" {
                        if let Some(attr) = prim.get_attribute("inputs:texture:format") {
                            if let Some(val) = attr.get(usd_sdf::TimeCode::default_time()) {
                                let fmt_str = format!("{val:?}");
                                // Token values stringify as Token("latlong") etc.
                                // Accepted: "latlong" and "automatic" (equirectangular).
                                let is_latlong = fmt_str.contains("latlong")
                                    || fmt_str.contains("automatic")
                                    || fmt_str.contains("equirectangular");
                                if !is_latlong {
                                    log::warn!(
                                        "[engine] DomeLight inputs:texture:format = {} is not \
                                         supported (only latlong/automatic). \
                                         Skipping IBL load to avoid incorrect result.",
                                        fmt_str
                                    );
                                    return;
                                }
                            }
                        }
                        break 'fmt_check;
                    }
                }
            }
        }

        // 2. Determine effective HDRI source: scene > fallback file > procedural
        //    Use a sentinel key for procedural sky so cache invalidation works.
        const PROCEDURAL_SKY_KEY: &str = "__procedural_sky__";

        let (effective_key, hdri_source) = if let Some(ref path) = scene_hdri_path {
            (path.clone(), IblSource::File(path.clone()))
        } else if self.dome_light_enabled {
            if let Some(ref fallback) = self.dome_light_texture_path {
                (fallback.clone(), IblSource::File(fallback.clone()))
            } else {
                (PROCEDURAL_SKY_KEY.to_string(), IblSource::ProceduralSky)
            }
        } else {
            // Dome light disabled and no scene dome light: clear IBL
            if self.ibl_hdri_path.is_some() {
                self.ibl_hdri_path = None;
                self.render_pass_state.clear_ibl_handles();
                log::debug!("[engine] IBL cleared: no dome light");
            }
            return;
        };

        // Skip reload if source unchanged
        if self.ibl_hdri_path.as_deref() == Some(effective_key.as_str()) {
            return;
        }

        let hgi_arc = match &self.wgpu_hgi {
            Some(h) => h.clone(),
            None => return,
        };

        // Resolve source pixels (width, height, f32 RGBA, is_hdr)
        let (src_pixels, src_w, src_h, is_hdr) = match hdri_source {
            IblSource::File(ref path) => {
                log::info!("[engine] Loading dome light HDRI: {}", path);
                let img = match read_image_data(path, SourceColorSpace::Raw, false, false) {
                    Some(i) => i,
                    None => {
                        log::warn!("[engine] Failed to load HDRI: {}", path);
                        return;
                    }
                };
                let w = img.width as u32;
                let h = img.height as u32;
                let hdr = img.format == usd_hio::types::HioFormat::Float32Vec4;
                (img.pixels, w, h, hdr)
            }
            IblSource::ProceduralSky => {
                log::info!("[engine] Generating procedural sky IBL");
                let (w, h) = (512u32, 256u32);
                let pixels = generate_procedural_sky(w, h);
                // Pixels are already f32 RGBA, convert to bytes
                let bytes: Vec<u8> = pixels.iter().flat_map(|f| f.to_le_bytes()).collect();
                (bytes, w, h, true)
            }
        };

        // Compute cubemap face dimension
        let face_dim = {
            let d = usd_hd_st::dome_light_computations::compute_cubemap_width(src_w, src_h, 256, 8);
            usd_hd_st::dome_light_computations::next_pow2(d)
                .max(64)
                .min(512)
        };

        let irr_dim = (face_dim / 8).max(16);
        let prefilter_mips: u32 = 5;
        let brdf_dim: u32 = 256;

        // Try GPU compute path first, fall back to CPU
        let gpu_result = self.compute_ibl_gpu(
            &src_pixels,
            src_w,
            src_h,
            is_hdr,
            face_dim,
            irr_dim,
            brdf_dim,
            &hgi_arc,
        );

        let (env_tex, env_smp, irr_tex, irr_smp, pf_tex, pf_smp, brdf_tex, brdf_smp) =
            if let Some(handles) = gpu_result {
                handles
            } else {
                log::info!("[engine] GPU IBL unavailable, using CPU fallback");

                let (env_pixels, irradiance_pixels, prefilter_pixels, brdf_pixels) =
                    compute_ibl_cpu(
                        &src_pixels,
                        src_w,
                        src_h,
                        is_hdr,
                        face_dim,
                        irr_dim,
                        prefilter_mips,
                        brdf_dim,
                    );

                let mut hgi = hgi_arc.write();

                let env_tex = upload_cubemap_texture(
                    &mut *hgi,
                    &env_pixels,
                    face_dim,
                    HgiFormat::Float32Vec4,
                );
                let env_smp = hgi.create_sampler(&HgiSamplerDesc {
                    address_mode_u: HgiSamplerAddressMode::ClampToEdge,
                    address_mode_v: HgiSamplerAddressMode::ClampToEdge,
                    address_mode_w: HgiSamplerAddressMode::ClampToEdge,
                    mag_filter: HgiSamplerFilter::Linear,
                    min_filter: HgiSamplerFilter::Linear,
                    mip_filter: HgiMipFilter::Linear,
                    ..Default::default()
                });

                let irr_tex = upload_cubemap_texture(
                    &mut *hgi,
                    &irradiance_pixels,
                    irr_dim,
                    HgiFormat::Float32Vec4,
                );
                let irr_smp = hgi.create_sampler(&HgiSamplerDesc {
                    address_mode_u: HgiSamplerAddressMode::ClampToEdge,
                    address_mode_v: HgiSamplerAddressMode::ClampToEdge,
                    address_mode_w: HgiSamplerAddressMode::ClampToEdge,
                    mag_filter: HgiSamplerFilter::Linear,
                    min_filter: HgiSamplerFilter::Linear,
                    mip_filter: HgiMipFilter::Linear,
                    ..Default::default()
                });

                let pf_tex = upload_cubemap_texture(
                    &mut *hgi,
                    &prefilter_pixels,
                    face_dim,
                    HgiFormat::Float32Vec4,
                );
                let pf_smp = hgi.create_sampler(&HgiSamplerDesc {
                    address_mode_u: HgiSamplerAddressMode::ClampToEdge,
                    address_mode_v: HgiSamplerAddressMode::ClampToEdge,
                    address_mode_w: HgiSamplerAddressMode::ClampToEdge,
                    mag_filter: HgiSamplerFilter::Linear,
                    min_filter: HgiSamplerFilter::Linear,
                    mip_filter: HgiMipFilter::Linear,
                    ..Default::default()
                });

                let brdf_tex = upload_2d_texture(
                    &mut *hgi,
                    &brdf_pixels,
                    brdf_dim,
                    brdf_dim,
                    HgiFormat::Float32Vec2,
                );
                let brdf_smp = hgi.create_sampler(&HgiSamplerDesc {
                    address_mode_u: HgiSamplerAddressMode::ClampToEdge,
                    address_mode_v: HgiSamplerAddressMode::ClampToEdge,
                    address_mode_w: HgiSamplerAddressMode::ClampToEdge,
                    mag_filter: HgiSamplerFilter::Linear,
                    min_filter: HgiSamplerFilter::Linear,
                    mip_filter: HgiMipFilter::NotMipmapped,
                    ..Default::default()
                });

                drop(hgi);
                (
                    env_tex, env_smp, irr_tex, irr_smp, pf_tex, pf_smp, brdf_tex, brdf_smp,
                )
            };

        self.render_pass_state.set_ibl_handles(IblHandles {
            env_cubemap_tex: env_tex,
            env_cubemap_smp: env_smp,
            irradiance_tex: irr_tex,
            irradiance_smp: irr_smp,
            prefilter_tex: pf_tex,
            prefilter_smp: pf_smp,
            brdf_lut_tex: brdf_tex,
            brdf_lut_smp: brdf_smp,
        });
        log::info!(
            "[engine] IBL ready: src={}x{} hdr={} face_dim={} irr_dim={} brdf={}x{} source={}",
            src_w,
            src_h,
            is_hdr,
            face_dim,
            irr_dim,
            brdf_dim,
            brdf_dim,
            &effective_key
        );
        self.ibl_hdri_path = Some(effective_key);
    }

    /// GPU IBL precomputation: latlong->cubemap, mipmap generation, irradiance,
    /// GGX prefilter, and BRDF LUT — all via wgpu compute/render passes.
    ///
    /// Returns None if GPU compute is unavailable (pipelines fail to compile).
    #[cfg(feature = "wgpu")]
    #[allow(clippy::too_many_arguments)]
    pub(super) fn compute_ibl_gpu(
        &mut self,
        src_pixels: &[u8],
        src_w: u32,
        src_h: u32,
        is_hdr: bool,
        face_dim: u32,
        irr_dim: u32,
        brdf_dim: u32,
        hgi_arc: &Arc<RwLock<HgiWgpu>>,
    ) -> Option<(
        HgiTextureHandle,
        usd_hgi::sampler::HgiSamplerHandle, // env cubemap
        HgiTextureHandle,
        usd_hgi::sampler::HgiSamplerHandle, // irradiance
        HgiTextureHandle,
        usd_hgi::sampler::HgiSamplerHandle, // prefilter
        HgiTextureHandle,
        usd_hgi::sampler::HgiSamplerHandle, // brdf LUT
    )> {
        let hgi_r = hgi_arc.read();
        let device = hgi_r.device().clone();
        let queue = hgi_r.queue().clone();
        drop(hgi_r);

        // Lazily create compute pipelines on first use
        if self.ibl_gpu_pipelines.is_none() {
            self.ibl_gpu_pipelines = IblGpuPipelines::new(&device);
            if self.ibl_gpu_pipelines.is_none() {
                log::warn!("[ibl_gpu] Failed to create compute pipelines");
                return None;
            }
        }
        let pipelines = self.ibl_gpu_pipelines.as_ref().unwrap();

        // Decode HDRI source into f32 RGBA
        let hdri_f32: Vec<f32> = if is_hdr {
            src_pixels
                .chunks(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect()
        } else {
            src_pixels
                .iter()
                .map(|&b| (b as f32 / 255.0).powf(2.2))
                .collect()
        };
        let hdri_bytes: Vec<u8> = hdri_f32.iter().flat_map(|f| f.to_le_bytes()).collect();

        // Upload HDRI as 2D texture (sampled)
        let src_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("ibl_src_latlong"),
            size: wgpu::Extent3d {
                width: src_w,
                height: src_h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            src_tex.as_image_copy(),
            &hdri_bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(src_w * 16),
                rows_per_image: Some(src_h),
            },
            wgpu::Extent3d {
                width: src_w,
                height: src_h,
                depth_or_array_layers: 1,
            },
        );
        let src_view = src_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("ibl_linear_sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Create HGI textures with SHADER_READ | SHADER_WRITE so they're usable
        // both as compute storage outputs and as fragment shader sampled inputs.
        let mut hgi_w = hgi_arc.write();

        // C++ generates mipmaps on the cubemap between latlong->cubemap and
        // irradiance/prefilter passes (domeLightComputations.cpp:477).
        // Allocate enough mip levels so irradiance/prefilter can sample blurred mips.
        let cubemap_mip_count = {
            let mut m = 1u16;
            let mut s = face_dim;
            while s > 1 {
                s >>= 1;
                m += 1;
            }
            m
        };
        let cubemap_desc = HgiTextureDesc {
            debug_name: "ibl_cubemap".to_string(),
            dimensions: Vec3i::new(face_dim as i32, face_dim as i32, 1),
            layer_count: 6,
            mip_levels: cubemap_mip_count,
            format: HgiFormat::Float16Vec4,
            usage: HgiTextureUsage::SHADER_READ
                | HgiTextureUsage::SHADER_WRITE
                | HgiTextureUsage::COLOR_TARGET, // RENDER_ATTACHMENT for mipmap blit
            ..Default::default()
        };
        let cubemap_handle = hgi_w.create_texture(&cubemap_desc, None);

        let irr_desc = HgiTextureDesc {
            debug_name: "ibl_irradiance".to_string(),
            dimensions: Vec3i::new(irr_dim as i32, irr_dim as i32, 1),
            layer_count: 6,
            mip_levels: 1,
            format: HgiFormat::Float16Vec4,
            usage: HgiTextureUsage::SHADER_READ | HgiTextureUsage::SHADER_WRITE,
            ..Default::default()
        };
        let irr_handle = hgi_w.create_texture(&irr_desc, None);

        // One mip level per roughness step (0.0 .. 1.0)
        let prefilter_mips: u16 = 5;
        let pf_desc = HgiTextureDesc {
            debug_name: "ibl_prefilter".to_string(),
            dimensions: Vec3i::new(face_dim as i32, face_dim as i32, 1),
            layer_count: 6,
            mip_levels: prefilter_mips,
            format: HgiFormat::Float16Vec4,
            usage: HgiTextureUsage::SHADER_READ | HgiTextureUsage::SHADER_WRITE,
            ..Default::default()
        };
        let pf_handle = hgi_w.create_texture(&pf_desc, None);

        let brdf_desc = HgiTextureDesc {
            debug_name: "ibl_brdf_lut".to_string(),
            dimensions: Vec3i::new(brdf_dim as i32, brdf_dim as i32, 1),
            layer_count: 1,
            mip_levels: 1,
            format: HgiFormat::Float16Vec4,
            usage: HgiTextureUsage::SHADER_READ | HgiTextureUsage::SHADER_WRITE,
            ..Default::default()
        };
        let brdf_handle = hgi_w.create_texture(&brdf_desc, None);

        // Resolve HGI handles to raw wgpu texture views for bind groups.
        // Storage texture views MUST have exactly 1 mip level (WebGPU spec).
        // Sampled texture views can cover all mips for textureSampleLevel() reads.
        let cubemap_raw_tex = usd_hgi_wgpu::resolve_texture(&cubemap_handle)?;
        // Mip-0 only view for storage writes (dispatch 1)
        let cubemap_storage_view = cubemap_raw_tex.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            base_mip_level: 0,
            mip_level_count: Some(1),
            ..Default::default()
        });
        // All-mips view for sampled reads (dispatch 2, 3)
        let cubemap_sampled_view = cubemap_raw_tex.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let irr_view = usd_hgi_wgpu::resolve_texture_view(&irr_handle)?;
        // Prefilter texture has 5 mip levels — need mip-0-only view for storage writes
        let pf_raw_tex = usd_hgi_wgpu::resolve_texture(&pf_handle)?;
        let pf_storage_view = pf_raw_tex.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            base_mip_level: 0,
            mip_level_count: Some(1),
            ..Default::default()
        });
        let brdf_view = usd_hgi_wgpu::resolve_texture_view(&brdf_handle)?;

        // Create HGI samplers for the output textures
        let mut make_cube_sampler = || {
            hgi_w.create_sampler(&HgiSamplerDesc {
                address_mode_u: HgiSamplerAddressMode::ClampToEdge,
                address_mode_v: HgiSamplerAddressMode::ClampToEdge,
                address_mode_w: HgiSamplerAddressMode::ClampToEdge,
                mag_filter: HgiSamplerFilter::Linear,
                min_filter: HgiSamplerFilter::Linear,
                mip_filter: HgiMipFilter::Linear,
                ..Default::default()
            })
        };
        let env_smp = make_cube_sampler();
        let irr_smp = make_cube_sampler();
        let pf_smp = make_cube_sampler();
        let brdf_smp = hgi_w.create_sampler(&HgiSamplerDesc {
            address_mode_u: HgiSamplerAddressMode::ClampToEdge,
            address_mode_v: HgiSamplerAddressMode::ClampToEdge,
            address_mode_w: HgiSamplerAddressMode::ClampToEdge,
            mag_filter: HgiSamplerFilter::Linear,
            min_filter: HgiSamplerFilter::Linear,
            mip_filter: HgiMipFilter::NotMipmapped,
            ..Default::default()
        });
        drop(hgi_w);

        // Dispatch 1: latlong_to_cubemap
        let wg = 8u32;
        {
            let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ibl_latlong_bg"),
                layout: &pipelines.latlong_to_cubemap.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&src_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&linear_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&cubemap_storage_view),
                    },
                ],
            });
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("ibl_latlong"),
            });
            {
                let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("latlong_to_cubemap"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&pipelines.latlong_to_cubemap);
                pass.set_bind_group(0, &bg, &[]);
                let g = (face_dim + wg - 1) / wg;
                pass.dispatch_workgroups(g, g, 6);
            }
            queue.submit(std::iter::once(enc.finish()));
        }

        // Generate cubemap mipmaps -- C++ domeLightComputations.cpp:477
        // This MUST happen between latlong->cubemap and irradiance/prefilter dispatches.
        // Irradiance and prefilter shaders sample higher mip levels for quality.
        {
            let mip_count = cubemap_mip_count as u32;
            for face in 0..6u32 {
                for mip in 1..mip_count {
                    // Source: previous mip of this face
                    let src_mip_view = cubemap_raw_tex.create_view(&wgpu::TextureViewDescriptor {
                        dimension: Some(wgpu::TextureViewDimension::D2),
                        base_mip_level: mip - 1,
                        mip_level_count: Some(1),
                        base_array_layer: face,
                        array_layer_count: Some(1),
                        ..Default::default()
                    });
                    // Destination: current mip of this face (render target)
                    let dst_mip_view = cubemap_raw_tex.create_view(&wgpu::TextureViewDescriptor {
                        dimension: Some(wgpu::TextureViewDimension::D2),
                        base_mip_level: mip,
                        mip_level_count: Some(1),
                        base_array_layer: face,
                        array_layer_count: Some(1),
                        ..Default::default()
                    });
                    let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("mipmap_blit_bg"),
                        layout: &pipelines.mipmap_blit_bgl,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::TextureView(&src_mip_view),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::Sampler(&linear_sampler),
                            },
                        ],
                    });
                    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("mipmap_blit_enc"),
                    });
                    {
                        let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("mipmap_blit_pass"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &dst_mip_view,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                    store: wgpu::StoreOp::Store,
                                },
                                depth_slice: None,
                            })],
                            depth_stencil_attachment: None,
                            ..Default::default()
                        });
                        rp.set_pipeline(&pipelines.mipmap_blit);
                        rp.set_bind_group(0, &bg, &[]);
                        rp.draw(0..3, 0..1);
                    }
                    queue.submit(std::iter::once(enc.finish()));
                }
            }
            log::debug!(
                "[ibl_gpu] Generated {} mip levels for cubemap ({face_dim}px)",
                mip_count
            );
        }

        // Dispatch 2: irradiance_conv
        {
            let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ibl_irradiance_bg"),
                layout: &pipelines.irradiance_conv.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&cubemap_sampled_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&linear_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(irr_view),
                    },
                ],
            });
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("ibl_irradiance"),
            });
            {
                let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("irradiance_conv"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&pipelines.irradiance_conv);
                pass.set_bind_group(0, &bg, &[]);
                let g = (irr_dim + wg - 1) / wg;
                pass.dispatch_workgroups(g, g, 6);
            }
            queue.submit(std::iter::once(enc.finish()));
        }

        // Dispatch 3: prefilter_ggx (roughness=0 for mip level 0)
        {
            let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ibl_prefilter_bg"),
                layout: &pipelines.prefilter_ggx.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&cubemap_sampled_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&linear_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&pf_storage_view),
                    },
                ],
            });
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("ibl_prefilter"),
            });
            {
                let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("prefilter_ggx"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&pipelines.prefilter_ggx);
                pass.set_bind_group(0, &bg, &[]);
                pass.set_push_constants(0, &0.0f32.to_le_bytes());
                let g = (face_dim + wg - 1) / wg;
                pass.dispatch_workgroups(g, g, 6);
            }
            queue.submit(std::iter::once(enc.finish()));
        }

        // Dispatch 4: brdf_integration
        {
            let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ibl_brdf_bg"),
                layout: &pipelines.brdf_integration.get_bind_group_layout(0),
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(brdf_view),
                }],
            });
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("ibl_brdf"),
            });
            {
                let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("brdf_integration"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&pipelines.brdf_integration);
                pass.set_bind_group(0, &bg, &[]);
                let g = (brdf_dim + wg - 1) / wg;
                pass.dispatch_workgroups(g, g, 1);
            }
            queue.submit(std::iter::once(enc.finish()));
        }

        log::info!("[engine] IBL computed on GPU: face={face_dim} irr={irr_dim} brdf={brdf_dim}");
        Some((
            cubemap_handle,
            env_smp,
            irr_handle,
            irr_smp,
            pf_handle,
            pf_smp,
            brdf_handle,
            brdf_smp,
        ))
    }
}

// =============================================================================
// IBL source type
// =============================================================================

/// IBL source type for collect_dome_light_ibl.
#[cfg(feature = "wgpu")]
pub(super) enum IblSource {
    /// Load from an HDRI file (scene dome light or fallback path).
    File(String),
    /// Generate a procedural sky gradient (no file needed).
    ProceduralSky,
}

// =============================================================================
// Free functions: procedural sky + CPU helpers
// =============================================================================

/// Generate a procedural sky gradient as equirectangular f32 RGBA pixels.
///
/// Produces a simple sky dome with:
/// - Blue-white gradient for sky hemisphere
/// - Warm horizon band
/// - Dark ground hemisphere
///
/// Output: w*h*4 f32 values in RGBA order (linear HDR).
#[cfg(feature = "wgpu")]
pub(super) fn generate_procedural_sky(w: u32, h: u32) -> Vec<f32> {
    let mut pixels = Vec::with_capacity((w * h * 4) as usize);

    for y in 0..h {
        // v=0 is top (north pole), v=1 is bottom (south pole)
        let v = (y as f32 + 0.5) / h as f32;
        // Elevation: +1 at zenith, 0 at horizon, -1 at nadir
        let elevation = 1.0 - 2.0 * v;

        // Sky colors (linear HDR)
        let (r, g, b) = if elevation > 0.0 {
            // Upper hemisphere: sky gradient from horizon to zenith
            let t = elevation; // 0 at horizon, 1 at zenith
            // Horizon: warm white-blue (0.7, 0.75, 0.85)
            // Zenith: deep sky blue (0.15, 0.3, 0.8)
            let r = 0.7 * (1.0 - t) + 0.15 * t;
            let g = 0.75 * (1.0 - t) + 0.3 * t;
            let b = 0.85 * (1.0 - t) + 0.8 * t;
            // Slight HDR boost near horizon for natural glow
            let horizon_boost = (1.0 - t).powf(4.0) * 0.3;
            (
                r + horizon_boost,
                g + horizon_boost,
                b + horizon_boost * 0.5,
            )
        } else {
            // Lower hemisphere: ground darkens from horizon
            let t = (-elevation).min(1.0);
            // Horizon: warm grey (0.4, 0.38, 0.35)
            // Nadir: dark ground (0.05, 0.05, 0.04)
            let r = 0.4 * (1.0 - t) + 0.05 * t;
            let g = 0.38 * (1.0 - t) + 0.05 * t;
            let b = 0.35 * (1.0 - t) + 0.04 * t;
            (r, g, b)
        };

        for _x in 0..w {
            pixels.push(r);
            pixels.push(g);
            pixels.push(b);
            pixels.push(1.0);
        }
    }

    pixels
}

/// Upload RGBA f32 cubemap as a 6-layer texture_2d_array.
///
/// `pixels` must contain 6 * face_dim^2 RGBA f32 texels (face-major).
#[cfg(feature = "wgpu")]
pub(super) fn upload_cubemap_texture(
    hgi: &mut dyn usd_hgi::hgi::Hgi,
    pixels: &[f32],
    face_dim: u32,
    format: usd_hgi::types::HgiFormat,
) -> usd_hgi::texture::HgiTextureHandle {
    use usd_hgi::enums::HgiTextureUsage;
    use usd_hgi::texture::HgiTextureDesc;

    let bytes_per_channel = match format {
        HgiFormat::Float32Vec4 => 4 * 4,
        HgiFormat::Float32Vec2 => 2 * 4,
        _ => 4 * 4,
    };
    let face_bytes = face_dim as usize * face_dim as usize * bytes_per_channel;
    let total_bytes = face_bytes * 6;

    // Convert f32 slice to bytes
    let byte_data: Vec<u8> = pixels
        .iter()
        .flat_map(|f| f.to_le_bytes())
        .take(total_bytes)
        .collect();
    let padded = if byte_data.len() < total_bytes {
        let mut v = byte_data;
        v.resize(total_bytes, 0);
        v
    } else {
        byte_data
    };

    let desc = HgiTextureDesc {
        debug_name: "ibl_cubemap".to_string(),
        dimensions: Vec3i::new(face_dim as i32, face_dim as i32, 1),
        layer_count: 6,
        mip_levels: 1,
        format,
        usage: HgiTextureUsage::SHADER_READ,
        ..Default::default()
    };
    hgi.create_texture(&desc, Some(&padded))
}

/// Upload a plain 2D float texture (e.g. BRDF LUT, RG format).
#[cfg(feature = "wgpu")]
pub(super) fn upload_2d_texture(
    hgi: &mut dyn usd_hgi::hgi::Hgi,
    pixels: &[f32],
    width: u32,
    height: u32,
    format: usd_hgi::types::HgiFormat,
) -> usd_hgi::texture::HgiTextureHandle {
    use usd_hgi::enums::HgiTextureUsage;
    use usd_hgi::texture::HgiTextureDesc;

    let byte_data: Vec<u8> = pixels.iter().flat_map(|f| f.to_le_bytes()).collect();

    let desc = HgiTextureDesc {
        debug_name: "ibl_brdf_lut".to_string(),
        dimensions: Vec3i::new(width as i32, height as i32, 1),
        layer_count: 1,
        mip_levels: 1,
        format,
        usage: HgiTextureUsage::SHADER_READ,
        ..Default::default()
    };
    hgi.create_texture(&desc, Some(&byte_data))
}

/// CPU-side IBL precomputation from an equirectangular HDRI.
///
/// Returns (env_cubemap, irradiance, prefilter, brdf_lut):
/// - env_cubemap: 6 * face_dim^2 * 4 f32 values (RGBA)
/// - irradiance:  6 * irr_dim^2 * 4 f32 values (RGBA)
/// - prefilter:   6 * face_dim^2 * 4 f32 values (RGBA, single mip)
/// - brdf_lut:    brdf_dim^2 * 2 f32 values (RG)
///
/// Quality is acceptable for preview rendering; the full GPU compute path
/// (dome_light_computations.rs) should replace this for production.
#[cfg(feature = "wgpu")]
pub(super) fn compute_ibl_cpu(
    src_pixels: &[u8],
    src_w: u32,
    src_h: u32,
    is_hdr: bool,
    face_dim: u32,
    irr_dim: u32,
    _prefilter_mips: u32,
    brdf_dim: u32,
) -> (Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>) {
    // Returns: (env_cubemap, irradiance, prefilter, brdf_lut)
    // Decode HDRI to f32 RGBA
    let hdri: Vec<f32> = if is_hdr {
        // Already f32 bytes from HIO
        src_pixels
            .chunks(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect()
    } else {
        // LDR u8 -> linear f32
        src_pixels
            .iter()
            .map(|&b| (b as f32 / 255.0).powf(2.2))
            .collect()
    };

    // Sample equirectangular latlong at a normalized direction
    let sample_latlong = |dir: [f32; 3]| -> [f32; 4] {
        let (dx, dy, dz) = (dir[0], dir[1], dir[2]);
        let phi = dz.atan2(dx);
        let theta = dy
            .asin()
            .clamp(-std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2);
        let u = (phi / (2.0 * std::f32::consts::PI) + 0.5)
            .fract()
            .max(0.0)
            .min(1.0 - 1e-6);
        let v = (theta / std::f32::consts::PI + 0.5).clamp(0.0, 1.0 - 1e-6);
        let px = (u * src_w as f32) as usize;
        let py = (v * src_h as f32) as usize;
        let idx = (py * src_w as usize + px) * 4;
        if idx + 3 < hdri.len() {
            [hdri[idx], hdri[idx + 1], hdri[idx + 2], 1.0]
        } else {
            [0.0, 0.0, 0.0, 1.0]
        }
    };

    // Direction for cubemap face pixel
    let cube_dir = |face: u32, u: f32, v: f32| -> [f32; 3] {
        let uc = 2.0 * u - 1.0;
        let vc = 2.0 * v - 1.0;
        let d = match face {
            0 => [1.0, -vc, -uc],  // +X
            1 => [-1.0, -vc, uc],  // -X
            2 => [uc, 1.0, vc],    // +Y
            3 => [uc, -1.0, -vc],  // -Y
            4 => [uc, -vc, 1.0],   // +Z
            5 => [-uc, -vc, -1.0], // -Z
            _ => [0.0, 0.0, 1.0],
        };
        let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1e-9);
        [d[0] / len, d[1] / len, d[2] / len]
    };

    // Irradiance map: cosine-weighted hemisphere integral (cheap MC approximation)
    let samples = 32u32;
    let irr_pixels = irr_dim as usize;
    let mut irradiance = vec![0.0f32; 6 * irr_pixels * irr_pixels * 4];
    for face in 0u32..6 {
        for y in 0..irr_pixels {
            for x in 0..irr_pixels {
                let u = (x as f32 + 0.5) / irr_pixels as f32;
                let v = (y as f32 + 0.5) / irr_pixels as f32;
                let n = cube_dir(face, u, v);
                let mut acc = [0.0f32; 3];
                // Simple stratified sampling over hemisphere
                for si in 0..samples {
                    for sj in 0..samples {
                        let phi = 2.0 * std::f32::consts::PI * si as f32 / samples as f32;
                        let cos_th = 1.0 - (sj as f32 + 0.5) / samples as f32;
                        let sin_th = (1.0 - cos_th * cos_th).sqrt();
                        // Tangent-space sample
                        let ls = [sin_th * phi.cos(), cos_th, sin_th * phi.sin()];
                        // Rotate to world space around n
                        let up = if n[1].abs() < 0.99 {
                            [0.0f32, 1.0, 0.0]
                        } else {
                            [1.0, 0.0, 0.0]
                        };
                        let t = cross(up, n);
                        let t_len = (t[0] * t[0] + t[1] * t[1] + t[2] * t[2]).sqrt().max(1e-9);
                        let t = [t[0] / t_len, t[1] / t_len, t[2] / t_len];
                        let b = cross(n, t);
                        let wd = [
                            t[0] * ls[0] + n[0] * ls[1] + b[0] * ls[2],
                            t[1] * ls[0] + n[1] * ls[1] + b[1] * ls[2],
                            t[2] * ls[0] + n[2] * ls[1] + b[2] * ls[2],
                        ];
                        let s = sample_latlong(wd);
                        acc[0] += s[0] * cos_th;
                        acc[1] += s[1] * cos_th;
                        acc[2] += s[2] * cos_th;
                    }
                }
                let scale = std::f32::consts::PI / (samples * samples) as f32;
                let idx = (face as usize * irr_pixels * irr_pixels + y * irr_pixels + x) * 4;
                irradiance[idx] = acc[0] * scale;
                irradiance[idx + 1] = acc[1] * scale;
                irradiance[idx + 2] = acc[2] * scale;
                irradiance[idx + 3] = 1.0;
            }
        }
    }

    // Raw environment cubemap: direct latlong -> cubemap conversion
    let cm_pixels = face_dim as usize;
    let mut env_cubemap = vec![0.0f32; 6 * cm_pixels * cm_pixels * 4];
    for face in 0u32..6 {
        for y in 0..cm_pixels {
            for x in 0..cm_pixels {
                let u = (x as f32 + 0.5) / cm_pixels as f32;
                let v = (y as f32 + 0.5) / cm_pixels as f32;
                let d = cube_dir(face, u, v);
                let s = sample_latlong(d);
                let idx = (face as usize * cm_pixels * cm_pixels + y * cm_pixels + x) * 4;
                env_cubemap[idx] = s[0];
                env_cubemap[idx + 1] = s[1];
                env_cubemap[idx + 2] = s[2];
                env_cubemap[idx + 3] = 1.0;
            }
        }
    }

    // Prefilter: for simplicity, use roughness=0 (mirror) = just the latlong cubemap
    // A proper prefilter requires GGX importance sampling per mip level.
    let pf_pixels = face_dim as usize;
    let mut prefilter = vec![0.0f32; 6 * pf_pixels * pf_pixels * 4];
    for face in 0u32..6 {
        for y in 0..pf_pixels {
            for x in 0..pf_pixels {
                let u = (x as f32 + 0.5) / pf_pixels as f32;
                let v = (y as f32 + 0.5) / pf_pixels as f32;
                let d = cube_dir(face, u, v);
                let s = sample_latlong(d);
                let idx = (face as usize * pf_pixels * pf_pixels + y * pf_pixels + x) * 4;
                prefilter[idx] = s[0];
                prefilter[idx + 1] = s[1];
                prefilter[idx + 2] = s[2];
                prefilter[idx + 3] = 1.0;
            }
        }
    }

    // BRDF LUT: pre-integrate split-sum (simple Schlick approximation)
    let bsz = brdf_dim as usize;
    let mut brdf_lut = vec![0.0f32; bsz * bsz * 2];
    let lut_samples = 256u32;
    for yi in 0..bsz {
        let roughness = (yi as f32 + 0.5) / bsz as f32;
        let alpha = roughness * roughness;
        for xi in 0..bsz {
            let n_dot_v = (xi as f32 + 0.5) / bsz as f32;
            let v = [(1.0 - n_dot_v * n_dot_v).sqrt(), n_dot_v, 0.0f32];
            let mut scale = 0.0f32;
            let mut bias = 0.0f32;
            for si in 0..lut_samples {
                let xi_h = usd_hd_st::dome_light_computations::radical_inverse_vdc(si);
                let phi = 2.0 * std::f32::consts::PI * si as f32 / lut_samples as f32;
                let cos_th = ((1.0 - xi_h) / (1.0 + (alpha * alpha - 1.0) * xi_h)).sqrt();
                let sin_th = (1.0 - cos_th * cos_th).sqrt();
                let h = [sin_th * phi.cos(), cos_th, sin_th * phi.sin()];
                let l = reflect_neg_v(v, h);
                let n_dot_l = l[1].max(0.0);
                if n_dot_l > 0.0 {
                    let n_dot_h = h[1].max(0.0);
                    let v_dot_h = dot3(v, h).max(0.0);
                    let g = usd_hd_st::dome_light_computations::geometry_schlick_smith(
                        n_dot_l, n_dot_v, roughness,
                    );
                    let g_vis = g * v_dot_h / (n_dot_h * n_dot_v).max(1e-6);
                    let fc = (1.0 - v_dot_h).powf(5.0);
                    scale += (1.0 - fc) * g_vis;
                    bias += fc * g_vis;
                }
            }
            let idx = (yi * bsz + xi) * 2;
            brdf_lut[idx] = (scale / lut_samples as f32).clamp(0.0, 1.0);
            brdf_lut[idx + 1] = (bias / lut_samples as f32).clamp(0.0, 1.0);
        }
    }

    (env_cubemap, irradiance, prefilter, brdf_lut)
}

/// Cross product of two 3-vectors.
#[cfg(feature = "wgpu")]
#[inline]
pub(super) fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Dot product of two 3-vectors.
#[cfg(feature = "wgpu")]
#[inline]
pub(super) fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// reflect(-v, h) = 2*dot(h,v)*h - v  (note: v is view, not -v)
#[cfg(feature = "wgpu")]
#[inline]
pub(super) fn reflect_neg_v(v: [f32; 3], h: [f32; 3]) -> [f32; 3] {
    let d = 2.0 * dot3(v, h);
    [d * h[0] - v[0], d * h[1] - v[1], d * h[2] - v[2]]
}
