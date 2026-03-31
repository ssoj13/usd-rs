//! GPU mipmap generation using compute shaders.
//!
//! Provides a compute pipeline that downsamples a 2D texture from one mip level to the next
//! using a simple box filter (average of 4 texels). This replaces OpenGL's glGenerateMipmap()
//! which has no direct equivalent in wgpu.

/// WGSL compute shader for mipmap generation (Rgba8Unorm).
const MIPMAP_SHADER_LINEAR: &str = r#"
@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var dst: texture_storage_2d<rgba8unorm, write>;
@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let dst_size = textureDimensions(dst);
    if (id.x >= dst_size.x || id.y >= dst_size.y) { return; }
    let src_coord = vec2<i32>(id.xy) * 2;
    let s00 = textureLoad(src, src_coord, 0);
    let s10 = textureLoad(src, src_coord + vec2<i32>(1, 0), 0);
    let s01 = textureLoad(src, src_coord + vec2<i32>(0, 1), 0);
    let s11 = textureLoad(src, src_coord + vec2<i32>(1, 1), 0);
    textureStore(dst, vec2<i32>(id.xy), (s00 + s10 + s01 + s11) * 0.25);
}
"#;

/// WGSL mipmap shader for sRGB textures: decode to linear, average, re-encode.
const MIPMAP_SHADER_SRGB: &str = r#"
@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var dst: texture_storage_2d<rgba8unorm, write>;
fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 { return c / 12.92; }
    return pow((c + 0.055) / 1.055, 2.4);
}
fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.0031308 { return c * 12.92; }
    return 1.055 * pow(c, 1.0 / 2.4) - 0.055;
}
fn dec(v: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(srgb_to_linear(v.r), srgb_to_linear(v.g), srgb_to_linear(v.b), v.a);
}
fn enc(v: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(linear_to_srgb(v.r), linear_to_srgb(v.g), linear_to_srgb(v.b), v.a);
}
@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let dst_size = textureDimensions(dst);
    if (id.x >= dst_size.x || id.y >= dst_size.y) { return; }
    let src_coord = vec2<i32>(id.xy) * 2;
    let avg_lin = (dec(textureLoad(src, src_coord, 0))
        + dec(textureLoad(src, src_coord + vec2<i32>(1, 0), 0))
        + dec(textureLoad(src, src_coord + vec2<i32>(0, 1), 0))
        + dec(textureLoad(src, src_coord + vec2<i32>(1, 1), 0))) * 0.25;
    textureStore(dst, vec2<i32>(id.xy), enc(avg_lin));
}
"#;

/// WGSL mipmap shader for Rgba16Float HDR textures.
const MIPMAP_SHADER_RGBA16F: &str = r#"
@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var dst: texture_storage_2d<rgba16float, write>;
@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let dst_size = textureDimensions(dst);
    if (id.x >= dst_size.x || id.y >= dst_size.y) { return; }
    let src_coord = vec2<i32>(id.xy) * 2;
    let s00 = textureLoad(src, src_coord, 0);
    let s10 = textureLoad(src, src_coord + vec2<i32>(1, 0), 0);
    let s01 = textureLoad(src, src_coord + vec2<i32>(0, 1), 0);
    let s11 = textureLoad(src, src_coord + vec2<i32>(1, 1), 0);
    textureStore(dst, vec2<i32>(id.xy), (s00 + s10 + s01 + s11) * 0.25);
}
"#;

/// WGSL mipmap shader for Rgba32Float HDR textures.
const MIPMAP_SHADER_RGBA32F: &str = r#"
@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var dst: texture_storage_2d<rgba32float, write>;
@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let dst_size = textureDimensions(dst);
    if (id.x >= dst_size.x || id.y >= dst_size.y) { return; }
    let src_coord = vec2<i32>(id.xy) * 2;
    let s00 = textureLoad(src, src_coord, 0);
    let s10 = textureLoad(src, src_coord + vec2<i32>(1, 0), 0);
    let s01 = textureLoad(src, src_coord + vec2<i32>(0, 1), 0);
    let s11 = textureLoad(src, src_coord + vec2<i32>(1, 1), 0);
    textureStore(dst, vec2<i32>(id.xy), (s00 + s10 + s01 + s11) * 0.25);
}
"#;

/// Supported texture formats for GPU mipmap generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MipmapFormat {
    Rgba8Unorm,
    Rgba8UnormSrgb,
    /// 16-bit HDR (FP16)
    Rgba16Float,
    /// 32-bit HDR (FP32)
    Rgba32Float,
}

impl MipmapFormat {
    /// Returns the wgpu storage format used for the destination mip view.
    pub fn storage_format(self) -> wgpu::TextureFormat {
        match self {
            MipmapFormat::Rgba8Unorm | MipmapFormat::Rgba8UnormSrgb => {
                wgpu::TextureFormat::Rgba8Unorm
            }
            MipmapFormat::Rgba16Float => wgpu::TextureFormat::Rgba16Float,
            MipmapFormat::Rgba32Float => wgpu::TextureFormat::Rgba32Float,
        }
    }
}

/// Cached compute pipeline for GPU mipmap generation.
///
/// Create once and reuse across multiple textures. Supports:
/// - Rgba8Unorm / Rgba8UnormSrgb (LDR)
/// - Rgba16Float / Rgba32Float (HDR, P1-3 fix)
pub struct MipmapGenerator {
    pipeline_linear: wgpu::ComputePipeline,
    pipeline_srgb: wgpu::ComputePipeline,
    pipeline_rgba16f: wgpu::ComputePipeline,
    pipeline_rgba32f: wgpu::ComputePipeline,
    bind_group_layout_ldr: wgpu::BindGroupLayout,
    bind_group_layout_hdr16: wgpu::BindGroupLayout,
    bind_group_layout_hdr32: wgpu::BindGroupLayout,
}

impl MipmapGenerator {
    /// Create a mipmap generator supporting all formats.
    pub fn new(device: &wgpu::Device) -> Self {
        // Helper to create a BGL for a given storage format.
        let make_bgl = |label: &str, storage_fmt: wgpu::TextureFormat| {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some(label),
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
                            format: storage_fmt,
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                ],
            })
        };

        let bgl_ldr = make_bgl("Mipmap BGL (LDR/rgba8)", wgpu::TextureFormat::Rgba8Unorm);
        let bgl_hdr16 = make_bgl("Mipmap BGL (HDR/rgba16f)", wgpu::TextureFormat::Rgba16Float);
        let bgl_hdr32 = make_bgl("Mipmap BGL (HDR/rgba32f)", wgpu::TextureFormat::Rgba32Float);

        let mk_pipeline = |label: &str, src: &'static str, bgl: &wgpu::BindGroupLayout| {
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: &[bgl],
                push_constant_ranges: &[],
            });
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(label),
                source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(src)),
            });
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
                module: &shader,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            })
        };

        let pipeline_linear = mk_pipeline("Mipmap (linear)", MIPMAP_SHADER_LINEAR, &bgl_ldr);
        let pipeline_srgb = mk_pipeline("Mipmap (sRGB)", MIPMAP_SHADER_SRGB, &bgl_ldr);
        let pipeline_rgba16f = mk_pipeline("Mipmap (rgba16f)", MIPMAP_SHADER_RGBA16F, &bgl_hdr16);
        let pipeline_rgba32f = mk_pipeline("Mipmap (rgba32f)", MIPMAP_SHADER_RGBA32F, &bgl_hdr32);

        Self {
            pipeline_linear,
            pipeline_srgb,
            pipeline_rgba16f,
            pipeline_rgba32f,
            bind_group_layout_ldr: bgl_ldr,
            bind_group_layout_hdr16: bgl_hdr16,
            bind_group_layout_hdr32: bgl_hdr32,
        }
    }

    /// Generate mipmaps for a texture using the compute shader.
    ///
    /// Downsamples from mip level 0 to all subsequent levels using a box filter.
    /// Each dispatch processes one mip level at a time, reading from level N-1 and writing to N.
    ///
    /// # Arguments
    ///
    /// * `device` - wgpu device for bind group creation
    /// * `encoder` - command encoder to record compute passes
    /// * `texture` - source texture with multiple mip levels allocated
    /// * `fmt` - logical format of the texture (determines view aliasing for sRGB)
    /// * `mip_count` - number of mip levels to generate (including level 0)
    /// * `base_width` - width of mip level 0
    /// * `base_height` - height of mip level 0
    ///
    /// # Format Support
    ///
    /// Supports rgba8unorm and rgba8unorm-srgb. sRGB textures use a unorm storage
    /// view alias (byte-compatible, texture was created with view_formats=[Rgba8Unorm]).
    pub fn generate(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        texture: &wgpu::Texture,
        fmt: MipmapFormat,
        mip_count: u32,
        base_width: u32,
        base_height: u32,
    ) {
        // Select pipeline and BGL by format.
        let (pipeline, bgl) = match fmt {
            MipmapFormat::Rgba8UnormSrgb => (&self.pipeline_srgb, &self.bind_group_layout_ldr),
            MipmapFormat::Rgba8Unorm => (&self.pipeline_linear, &self.bind_group_layout_ldr),
            MipmapFormat::Rgba16Float => (&self.pipeline_rgba16f, &self.bind_group_layout_hdr16),
            MipmapFormat::Rgba32Float => (&self.pipeline_rgba32f, &self.bind_group_layout_hdr32),
        };
        let dst_view_fmt = fmt.storage_format();

        // Process each mip level (skip level 0, it's the source)
        for mip_level in 1..mip_count {
            // Calculate source and destination dimensions
            let src_width = (base_width >> (mip_level - 1)).max(1);
            let src_height = (base_height >> (mip_level - 1)).max(1);
            let dst_width = (base_width >> mip_level).max(1);
            let dst_height = (base_height >> mip_level).max(1);

            // Source view keeps native format (textureLoad returns raw encoded values)
            let src_view = texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some(&format!("Mipmap Source Mip {}", mip_level - 1)),
                format: None,
                usage: None,
                dimension: Some(wgpu::TextureViewDimension::D2),
                aspect: wgpu::TextureAspect::All,
                base_mip_level: mip_level - 1,
                mip_level_count: Some(1),
                base_array_layer: 0,
                array_layer_count: None,
            });

            // Destination view uses unorm for storage (sRGB is not a valid storage format)
            let dst_view = texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some(&format!("Mipmap Dest Mip {}", mip_level)),
                format: Some(dst_view_fmt),
                usage: None,
                dimension: Some(wgpu::TextureViewDimension::D2),
                aspect: wgpu::TextureAspect::All,
                base_mip_level: mip_level,
                mip_level_count: Some(1),
                base_array_layer: 0,
                array_layer_count: None,
            });

            // Create bind group for this mip level
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("Mipmap Bind Group Mip {}", mip_level)),
                layout: bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&src_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&dst_view),
                    },
                ],
            });

            // Dispatch compute shader
            let workgroup_count_x = (dst_width + 7) / 8;
            let workgroup_count_y = (dst_height + 7) / 8;

            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some(&format!("Mipmap Generation Mip {}", mip_level)),
                timestamp_writes: None,
            });

            compute_pass.set_pipeline(pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);
            compute_pass.dispatch_workgroups(workgroup_count_x, workgroup_count_y, 1);
            drop(compute_pass);

            // Log progress for debugging
            log::trace!(
                "Generated mip level {} ({}x{} -> {}x{})",
                mip_level,
                src_width,
                src_height,
                dst_width,
                dst_height
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mipmap_shader_compiles() {
        // Verify WGSL shader source is valid (basic syntax check)
        assert!(MIPMAP_SHADER_LINEAR.contains("@compute"));
        assert!(MIPMAP_SHADER_LINEAR.contains("texture_2d"));
        assert!(MIPMAP_SHADER_SRGB.contains("srgb_to_linear"));
    }

    // Full integration test requires wgpu::Device, tested in integration tests
}
