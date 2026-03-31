use std::collections::HashMap;

use bytemuck::{cast_slice, Pod, Zeroable};
use egui::TextureId;
use egui_wgpu::wgpu;
use usd_hgi_wgpu::resolve_texture_view;
use usd_imaging::gl::Engine;
use vfx_ocio::{GpuLanguage, GpuProcessor, GpuTextureType, Processor};

use crate::data_model::{ColorCorrectionMode, OcioSettings};

use super::color_correction::OcioCpuState;

const SRGB_SHADER: &str = r#"
@group(0) @binding(0) var input_texture: texture_2d<f32>;
@group(0) @binding(1) var output_texture: texture_storage_2d<rgba16float, write>;
@group(0) @binding(2) var tex_sampler: sampler;

fn linear_to_srgb(v: f32) -> f32 {
    if (v <= 0.0031308) {
        return v * 12.92;
    }
    return 1.055 * pow(v, 1.0 / 2.4) - 0.055;
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let dims = textureDimensions(input_texture);
    if (global_id.x >= dims.x || global_id.y >= dims.y) {
        return;
    }
    let uv =
        vec2<f32>(f32(global_id.x) + 0.5, f32(global_id.y) + 0.5) /
        vec2<f32>(f32(dims.x), f32(dims.y));
    let color = textureSampleLevel(input_texture, tex_sampler, uv, 0.0);
    textureStore(
        output_texture,
        vec2<i32>(global_id.xy),
        vec4<f32>(
            linear_to_srgb(color.r),
            linear_to_srgb(color.g),
            linear_to_srgb(color.b),
            color.a
        )
    );
}
"#;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct OcioUniforms {
    exposure_mult: f32,
    _pad: [f32; 3],
}

const GPU_OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

pub struct ViewportGpuState {
    render_state: Option<egui_wgpu::RenderState>,
    texture_id: Option<TextureId>,
    output_texture: Option<wgpu::Texture>,
    output_view: Option<wgpu::TextureView>,
    output_size: (u32, u32),
    srgb_pass: Option<SrgbPass>,
    ocio_pass: OcioGpuPass,
}

impl Default for ViewportGpuState {
    fn default() -> Self {
        Self::new()
    }
}

impl ViewportGpuState {
    pub fn new() -> Self {
        Self {
            render_state: None,
            texture_id: None,
            output_texture: None,
            output_view: None,
            output_size: (0, 0),
            srgb_pass: None,
            ocio_pass: OcioGpuPass::new(),
        }
    }

    pub fn set_render_state(&mut self, render_state: egui_wgpu::RenderState) {
        self.render_state = Some(render_state);
    }

    pub fn texture_id(&self) -> Option<TextureId> {
        self.texture_id
    }

    pub fn present_engine_color(
        &mut self,
        engine: &Engine,
        width: u32,
        height: u32,
        mode: ColorCorrectionMode,
        ocio_state: &mut OcioCpuState,
        ocio_settings: &OcioSettings,
    ) -> bool {
        let Some(render_state) = self.render_state.clone() else {
            log::debug!("[gpu_display] no render_state");
            return false;
        };
        let color_tex = engine.wgpu_color_texture();
        if color_tex.is_none() {
            log::debug!("[gpu_display] wgpu_color_texture() is None");
            return false;
        }
        let resolved = color_tex.and_then(resolve_texture_view);
        if resolved.is_none() {
            log::debug!("[gpu_display] resolve_texture_view() returned None");
            return false;
        }
        let source_view = resolved.cloned().unwrap();

        match mode {
            ColorCorrectionMode::Disabled => {
                // Always run sRGB for Rgba16Float → displayable conversion
                let Some(output_view) = self.ensure_output_view(&render_state, width, height)
                else {
                    return false;
                };
                let pass = self
                    .srgb_pass
                    .get_or_insert_with(|| SrgbPass::new(&render_state.device));
                pass.execute(
                    &render_state.device,
                    &render_state.queue,
                    &source_view,
                    &output_view,
                    width,
                    height,
                );
                self.bind_native_texture(&render_state, &output_view);
                true
            }
            ColorCorrectionMode::SRGB => {
                let Some(output_view) = self.ensure_output_view(&render_state, width, height)
                else {
                    return false;
                };
                let pass = self
                    .srgb_pass
                    .get_or_insert_with(|| SrgbPass::new(&render_state.device));
                pass.execute(
                    &render_state.device,
                    &render_state.queue,
                    &source_view,
                    &output_view,
                    width,
                    height,
                );
                self.bind_native_texture(&render_state, &output_view);
                true
            }
            ColorCorrectionMode::OpenColorIO => {
                ocio_state.load_config();
                ocio_state.ensure_processor(ocio_settings);
                let Some(processor) = ocio_state.processor() else {
                    return false;
                };
                let Some(output_view) = self.ensure_output_view(&render_state, width, height)
                else {
                    return false;
                };
                if self
                    .ocio_pass
                    .rebuild_if_needed(
                        &render_state.device,
                        &render_state.queue,
                        processor,
                        ocio_settings,
                    )
                    .is_err()
                {
                    return false;
                }
                if !self.ocio_pass.execute(
                    &render_state.device,
                    &render_state.queue,
                    &source_view,
                    &output_view,
                    width,
                    height,
                ) {
                    return false;
                }
                self.bind_native_texture(&render_state, &output_view);
                true
            }
        }
    }

    fn ensure_output_view(
        &mut self,
        render_state: &egui_wgpu::RenderState,
        width: u32,
        height: u32,
    ) -> Option<wgpu::TextureView> {
        if width == 0 || height == 0 {
            return None;
        }
        if self.output_size != (width, height) || self.output_view.is_none() {
            let texture = render_state
                .device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("usd-view viewport output"),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: GPU_OUTPUT_FORMAT,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::STORAGE_BINDING
                        | wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            self.output_texture = Some(texture);
            self.output_view = Some(view);
            self.output_size = (width, height);
        }
        self.output_view.as_ref().cloned()
    }

    fn bind_native_texture(
        &mut self,
        render_state: &egui_wgpu::RenderState,
        texture_view: &wgpu::TextureView,
    ) {
        let mut renderer = render_state.renderer.write();
        if let Some(texture_id) = self.texture_id {
            renderer.update_egui_texture_from_wgpu_texture(
                &render_state.device,
                texture_view,
                wgpu::FilterMode::Nearest,
                texture_id,
            );
        } else {
            self.texture_id = Some(renderer.register_native_texture(
                &render_state.device,
                texture_view,
                wgpu::FilterMode::Nearest,
            ));
        }
    }
}

struct SrgbPass {
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::ComputePipeline,
    sampler: wgpu::Sampler,
}

impl SrgbPass {
    fn new(device: &wgpu::Device) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("usd-view sRGB layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: GPU_OUTPUT_FORMAT,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("usd-view sRGB shader"),
            source: wgpu::ShaderSource::Wgsl(SRGB_SHADER.into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("usd-view sRGB pipeline layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("usd-view sRGB pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("usd-view sRGB sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        Self {
            layout,
            pipeline,
            sampler,
        }
    }

    fn execute(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        input_view: &wgpu::TextureView,
        output_view: &wgpu::TextureView,
        width: u32,
        height: u32,
    ) {
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("usd-view sRGB bind group"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(input_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(output_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("usd-view sRGB encoder"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("usd-view sRGB pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(width.div_ceil(8), height.div_ceil(8), 1);
        }
        queue.submit(Some(encoder.finish()));
    }
}

struct OcioGpuPass {
    settings_key: String,
    io_layout: Option<wgpu::BindGroupLayout>,
    uniform_layout: Option<wgpu::BindGroupLayout>,
    lut_layout: Option<wgpu::BindGroupLayout>,
    pipeline: Option<wgpu::ComputePipeline>,
    io_sampler: Option<wgpu::Sampler>,
    uniform_buf: Option<wgpu::Buffer>,
    uniform_bg: Option<wgpu::BindGroup>,
    gpu_processor: Option<GpuProcessor>,
    lut_textures: HashMap<String, (wgpu::Texture, wgpu::TextureView)>,
    lut_samplers: HashMap<String, wgpu::Sampler>,
}

impl OcioGpuPass {
    fn new() -> Self {
        Self {
            settings_key: String::new(),
            io_layout: None,
            uniform_layout: None,
            lut_layout: None,
            pipeline: None,
            io_sampler: None,
            uniform_buf: None,
            uniform_bg: None,
            gpu_processor: None,
            lut_textures: HashMap::new(),
            lut_samplers: HashMap::new(),
        }
    }

    fn rebuild_if_needed(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        processor: &Processor,
        settings: &OcioSettings,
    ) -> Result<(), ()> {
        let settings_key = format!(
            "{}/{}/{}/{}",
            settings.display, settings.view, settings.color_space, settings.looks
        );
        if settings_key == self.settings_key && self.pipeline.is_some() {
            return Ok(());
        }

        let gpu_processor = GpuProcessor::from_processor_wgsl(processor).map_err(|_| ())?;
        let raw_wgsl = gpu_processor.generate_shader(GpuLanguage::Wgsl);
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("usd-view ocio shader"),
            source: wgpu::ShaderSource::Wgsl(wrap_ocio_wgsl(raw_wgsl.fragment_code()).into()),
        });

        let io_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("usd-view ocio io layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: GPU_OUTPUT_FORMAT,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("usd-view ocio uniform layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let mut lut_entries = Vec::new();
        for desc in gpu_processor.batch_texture_descriptors() {
            lut_entries.push(wgpu::BindGroupLayoutEntry {
                binding: desc.binding_index as u32 * 2,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: match desc.texture_type {
                        GpuTextureType::Texture1D | GpuTextureType::Texture2D => {
                            wgpu::TextureViewDimension::D2
                        }
                        GpuTextureType::Texture3D => wgpu::TextureViewDimension::D3,
                    },
                    multisampled: false,
                },
                count: None,
            });
            lut_entries.push(wgpu::BindGroupLayoutEntry {
                binding: desc.binding_index as u32 * 2 + 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            });
        }
        let lut_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("usd-view ocio lut layout"),
            entries: &lut_entries,
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("usd-view ocio pipeline layout"),
            bind_group_layouts: &[&io_layout, &uniform_layout, &lut_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("usd-view ocio pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader_module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("usd-view ocio uniform buffer"),
            size: std::mem::size_of::<OcioUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(
            &uniform_buf,
            0,
            bytemuck::bytes_of(&OcioUniforms {
                exposure_mult: 1.0,
                _pad: [0.0; 3],
            }),
        );
        let uniform_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("usd-view ocio uniform bind group"),
            layout: &uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buf.as_entire_binding(),
            }],
        });
        let io_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("usd-view ocio io sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        self.upload_luts(device, queue, &gpu_processor);
        self.settings_key = settings_key;
        self.io_layout = Some(io_layout);
        self.uniform_layout = Some(uniform_layout);
        self.lut_layout = Some(lut_layout);
        self.pipeline = Some(pipeline);
        self.io_sampler = Some(io_sampler);
        self.uniform_buf = Some(uniform_buf);
        self.uniform_bg = Some(uniform_bg);
        self.gpu_processor = Some(gpu_processor);
        Ok(())
    }

    fn execute(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        input_view: &wgpu::TextureView,
        output_view: &wgpu::TextureView,
        width: u32,
        height: u32,
    ) -> bool {
        let (Some(io_layout), Some(uniform_bg), Some(pipeline), Some(io_sampler), Some(lut_layout)) = (
            self.io_layout.as_ref(),
            self.uniform_bg.as_ref(),
            self.pipeline.as_ref(),
            self.io_sampler.as_ref(),
            self.lut_layout.as_ref(),
        ) else {
            return false;
        };
        let io_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("usd-view ocio io bind group"),
            layout: io_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(input_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(output_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(io_sampler),
                },
            ],
        });
        let Some(lut_bg) = self.create_lut_bind_group(device, lut_layout) else {
            return false;
        };
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("usd-view ocio encoder"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("usd-view ocio pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &io_bg, &[]);
            pass.set_bind_group(1, uniform_bg, &[]);
            pass.set_bind_group(2, &lut_bg, &[]);
            pass.dispatch_workgroups(width.div_ceil(8), height.div_ceil(8), 1);
        }
        queue.submit(Some(encoder.finish()));
        true
    }

    fn upload_luts(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        gpu_processor: &GpuProcessor,
    ) {
        self.lut_textures.clear();
        self.lut_samplers.clear();

        for texture in gpu_processor.textures() {
            let (wgpu_texture, wgpu_view) = match texture.texture_type {
                GpuTextureType::Texture1D => {
                    let tex = device.create_texture(&wgpu::TextureDescriptor {
                        label: Some(&texture.name),
                        size: wgpu::Extent3d {
                            width: texture.width,
                            height: 1,
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
                        wgpu::TexelCopyTextureInfo {
                            texture: &tex,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        cast_slice(&texture.data),
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(texture.width * 16),
                            rows_per_image: Some(1),
                        },
                        wgpu::Extent3d {
                            width: texture.width,
                            height: 1,
                            depth_or_array_layers: 1,
                        },
                    );
                    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
                    (tex, view)
                }
                GpuTextureType::Texture2D => {
                    let tex = device.create_texture(&wgpu::TextureDescriptor {
                        label: Some(&texture.name),
                        size: wgpu::Extent3d {
                            width: texture.width,
                            height: texture.height,
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
                        wgpu::TexelCopyTextureInfo {
                            texture: &tex,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        cast_slice(&texture.data),
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(texture.width * 16),
                            rows_per_image: Some(texture.height),
                        },
                        wgpu::Extent3d {
                            width: texture.width,
                            height: texture.height,
                            depth_or_array_layers: 1,
                        },
                    );
                    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
                    (tex, view)
                }
                GpuTextureType::Texture3D => {
                    let tex = device.create_texture(&wgpu::TextureDescriptor {
                        label: Some(&texture.name),
                        size: wgpu::Extent3d {
                            width: texture.width,
                            height: texture.height,
                            depth_or_array_layers: texture.depth,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D3,
                        format: wgpu::TextureFormat::Rgba32Float,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                        view_formats: &[],
                    });
                    queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: &tex,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        cast_slice(&texture.data),
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(texture.width * 16),
                            rows_per_image: Some(texture.height),
                        },
                        wgpu::Extent3d {
                            width: texture.width,
                            height: texture.height,
                            depth_or_array_layers: texture.depth,
                        },
                    );
                    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
                    (tex, view)
                }
            };
            let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some(&format!("{} sampler", texture.name)),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Nearest,
                ..Default::default()
            });
            self.lut_textures
                .insert(texture.name.clone(), (wgpu_texture, wgpu_view));
            self.lut_samplers.insert(texture.name.clone(), sampler);
        }
    }

    fn create_lut_bind_group(
        &self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
    ) -> Option<wgpu::BindGroup> {
        let gpu_processor = self.gpu_processor.as_ref()?;
        let descriptors = gpu_processor.batch_texture_descriptors();
        if descriptors.is_empty() {
            return Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("usd-view ocio empty lut bind group"),
                layout,
                entries: &[],
            }));
        }

        let mut entries = Vec::new();
        for desc in descriptors {
            let (_, view) = self.lut_textures.get(&desc.name)?;
            let sampler = self.lut_samplers.get(&desc.name)?;
            entries.push(wgpu::BindGroupEntry {
                binding: desc.binding_index as u32 * 2,
                resource: wgpu::BindingResource::TextureView(view),
            });
            entries.push(wgpu::BindGroupEntry {
                binding: desc.binding_index as u32 * 2 + 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            });
        }
        Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("usd-view ocio lut bind group"),
            layout,
            entries: &entries,
        }))
    }
}

fn wrap_ocio_wgsl(raw_wgsl: &str) -> String {
    let split_idx = raw_wgsl.find("@group(0)").unwrap_or(raw_wgsl.len());
    let ocio_fns = &raw_wgsl[..split_idx];
    format!(
        r#"
struct OcioUniforms {{
    exposure_mult: f32,
    _pad1: f32,
    _pad2: f32,
    _pad3: f32,
}}
@group(1) @binding(0) var<uniform> u_ocio: OcioUniforms;

@group(0) @binding(0) var input_texture: texture_2d<f32>;
@group(0) @binding(1) var output_texture: texture_storage_2d<rgba16float, write>;
@group(0) @binding(2) var tex_sampler: sampler;

{ocio_fns}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {{
    let dims = textureDimensions(input_texture);
    if (global_id.x >= dims.x || global_id.y >= dims.y) {{
        return;
    }}
    let uv =
        vec2<f32>(f32(global_id.x) + 0.5, f32(global_id.y) + 0.5) /
        vec2<f32>(f32(dims.x), f32(dims.y));
    let color = textureSampleLevel(input_texture, tex_sampler, uv, 0.0);
    let exposed = vec4<f32>(color.rgb * u_ocio.exposure_mult, color.a);
    let result = ocio_transform(exposed);
    textureStore(output_texture, vec2<i32>(global_id.xy), result);
}}
"#
    )
}
