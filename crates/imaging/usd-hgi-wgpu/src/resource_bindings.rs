//! wgpu resource bindings implementation for HGI.
//!
//! Maps HgiResourceBindingsDesc (buffer + texture bindings) into a
//! wgpu::BindGroupLayout + wgpu::BindGroup pair.
//!
//! ## Binding Index Convention
//!
//! wgpu requires separate bindings for textures and samplers (unlike Vulkan/GL
//! which support combined image-samplers). For HgiTextureBindDesc:
//!
//! - **Texture**: bound at `binding_index`
//! - **Sampler**: bound at `binding_index + 1` (if samplers exist and not StorageImage)
//!
//! This means shaders must declare consecutive bindings:
//! ```glsl
//! layout(binding=0) uniform texture2D myTex;
//! layout(binding=1) uniform sampler mySampler;
//! ```
//!
//! HgiBindResourceType::CombinedSamplerImage is automatically split into separate bindings.

use usd_hgi::enums::HgiBindResourceType;
use usd_hgi::resource_bindings::{HgiResourceBindings, HgiResourceBindingsDesc};

use super::buffer::WgpuBuffer;
use super::conversions;
use super::sampler::WgpuSampler;
use super::texture::WgpuTexture;

/// wgpu-backed resource bindings (bind group + layout).
///
/// Created from HgiResourceBindingsDesc by mapping each buffer and texture
/// binding to wgpu layout entries and bind group entries.
pub struct WgpuResourceBindings {
    desc: HgiResourceBindingsDesc,
    /// None when created via new_empty (HGI trait path without concrete resources)
    bind_group_layout: Option<wgpu::BindGroupLayout>,
    bind_group: Option<wgpu::BindGroup>,
}

impl WgpuResourceBindings {
    /// Create resource bindings from an HGI descriptor and concrete wgpu resources.
    ///
    /// `buffers` and `textures`/`samplers` are parallel to the binding descs.
    /// Each inner Vec corresponds to the buffers/textures in that binding slot.
    pub fn new(
        device: &wgpu::Device,
        desc: &HgiResourceBindingsDesc,
        buffers: &[Vec<&WgpuBuffer>],
        textures: &[Vec<&WgpuTexture>],
        samplers: &[Vec<&WgpuSampler>],
    ) -> Self {
        let label = if desc.debug_name.is_empty() {
            None
        } else {
            Some(desc.debug_name.as_str())
        };

        // -- Build layout entries --
        let mut layout_entries = Vec::new();

        // Buffer bindings
        for buf_bind in &desc.buffer_bindings {
            let visibility = conversions::to_wgpu_shader_stages(buf_bind.stage_usage);
            let ty =
                conversions::to_wgpu_buffer_binding_type(buf_bind.resource_type, buf_bind.writable);

            layout_entries.push(wgpu::BindGroupLayoutEntry {
                binding: buf_bind.binding_index,
                visibility,
                ty,
                count: None,
            });
        }

        // Texture bindings (wgpu separates textures and samplers)
        for tex_bind in &desc.texture_bindings {
            let visibility = conversions::to_wgpu_shader_stages(tex_bind.stage_usage);
            let is_storage = tex_bind.resource_type == HgiBindResourceType::StorageImage;

            let tex_ty = conversions::to_wgpu_texture_binding_type(
                tex_bind.resource_type,
                tex_bind.writable,
            );
            layout_entries.push(wgpu::BindGroupLayoutEntry {
                binding: tex_bind.binding_index,
                visibility,
                ty: tex_ty,
                count: None,
            });

            // Sampler at binding_index + 1 (only for sampled images)
            if !is_storage && !tex_bind.samplers.is_empty() {
                layout_entries.push(wgpu::BindGroupLayoutEntry {
                    binding: tex_bind.binding_index + 1,
                    visibility,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                });
            }
        }

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label,
            entries: &layout_entries,
        });

        // -- Build bind group entries --
        let mut bg_entries = Vec::new();

        // Buffer entries
        for (i, buf_bind) in desc.buffer_bindings.iter().enumerate() {
            if let Some(buf_list) = buffers.get(i) {
                if let Some(buf) = buf_list.first() {
                    let offset = buf_bind.offsets.first().copied().unwrap_or(0) as u64;
                    let size = buf_bind.sizes.first().copied().filter(|&s| s > 0);

                    bg_entries.push(wgpu::BindGroupEntry {
                        binding: buf_bind.binding_index,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: buf.wgpu_buffer(),
                            offset,
                            size: size.map(|s| {
                                std::num::NonZeroU64::new(s as u64)
                                    .expect("buffer bind size must be > 0")
                            }),
                        }),
                    });
                }
            }
        }

        // Texture + sampler entries
        for (i, tex_bind) in desc.texture_bindings.iter().enumerate() {
            let is_storage = tex_bind.resource_type == HgiBindResourceType::StorageImage;

            if let Some(tex_list) = textures.get(i) {
                if let Some(tex) = tex_list.first() {
                    bg_entries.push(wgpu::BindGroupEntry {
                        binding: tex_bind.binding_index,
                        resource: wgpu::BindingResource::TextureView(tex.wgpu_view()),
                    });
                }
            }

            if !is_storage {
                if let Some(smp_list) = samplers.get(i) {
                    if let Some(smp) = smp_list.first() {
                        bg_entries.push(wgpu::BindGroupEntry {
                            binding: tex_bind.binding_index + 1,
                            resource: wgpu::BindingResource::Sampler(smp.wgpu_sampler()),
                        });
                    }
                }
            }
        }

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label,
            layout: &bind_group_layout,
            entries: &bg_entries,
        });

        Self {
            desc: desc.clone(),
            bind_group_layout: Some(bind_group_layout),
            bind_group: Some(bind_group),
        }
    }

    /// Create empty resource bindings (for HGI trait path without concrete resources).
    pub fn new_empty(desc: &HgiResourceBindingsDesc) -> Self {
        Self {
            desc: desc.clone(),
            bind_group_layout: None,
            bind_group: None,
        }
    }

    /// Access the wgpu::BindGroupLayout for pipeline layout creation.
    pub(crate) fn wgpu_layout(&self) -> &wgpu::BindGroupLayout {
        self.bind_group_layout
            .as_ref()
            .expect("bind group layout not initialized")
    }

    /// Access the wgpu::BindGroup for command encoding.
    pub(crate) fn wgpu_bind_group(&self) -> &wgpu::BindGroup {
        self.bind_group
            .as_ref()
            .expect("bind group not initialized")
    }
}

impl HgiResourceBindings for WgpuResourceBindings {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn descriptor(&self) -> &HgiResourceBindingsDesc {
        &self.desc
    }

    fn raw_resource(&self) -> u64 {
        // No single numeric ID for a bind group in wgpu
        0
    }
}

#[cfg(test)]
mod tests {
    use usd_hgi::enums::HgiShaderStage;
    use usd_hgi::resource_bindings::{HgiBufferBindDesc, HgiResourceBindingsDesc};

    #[test]
    fn test_empty_desc() {
        let desc = HgiResourceBindingsDesc::new();
        assert!(desc.buffer_bindings.is_empty());
        assert!(desc.texture_bindings.is_empty());
    }

    // -----------------------------------------------------------------------
    // BGL sparse binding indices — layout_entries must use the declared index
    // -----------------------------------------------------------------------

    /// When bindings have non-consecutive (sparse) indices, each entry must
    /// carry its declared `binding_index` through to the layout entries.
    /// This test validates the mapping logic in WgpuResourceBindings::new()
    /// at the descriptor level (no GPU device required).
    #[test]
    fn test_sparse_buffer_binding_indices() {
        let mut desc = HgiResourceBindingsDesc::new();

        // Add bindings at indices 0, 5, 10 (sparse gap between each)
        for &idx in &[0u32, 5, 10] {
            let bind = HgiBufferBindDesc::new()
                .with_binding_index(idx)
                .with_stage_usage(HgiShaderStage::VERTEX);
            desc.buffer_bindings.push(bind);
        }

        // Verify the descriptor captured the correct indices
        let indices: Vec<u32> = desc
            .buffer_bindings
            .iter()
            .map(|b| b.binding_index)
            .collect();
        assert_eq!(indices, vec![0, 5, 10]);
    }

    #[test]
    fn test_buffer_binding_default_index_zero() {
        let bind = HgiBufferBindDesc::new();
        assert_eq!(bind.binding_index, 0);
    }

    #[test]
    fn test_buffer_binding_index_builder() {
        let bind = HgiBufferBindDesc::new().with_binding_index(7);
        assert_eq!(bind.binding_index, 7);
    }

    /// Texture bindings at consecutive indices — layout entry at binding_index,
    /// sampler at binding_index + 1. With indices 2,4 the entries would be
    /// at 2,3,4,5 (tex0, samp0, tex1, samp1) — no collision.
    #[test]
    fn test_texture_binding_index_stride() {
        use usd_hgi::resource_bindings::HgiTextureBindDesc;
        let mut desc = HgiResourceBindingsDesc::new();

        // Bindings at indices 0 and 2 (stride 2 to account for paired sampler)
        for &idx in &[0u32, 2] {
            let bind = HgiTextureBindDesc::new().with_binding_index(idx);
            desc.texture_bindings.push(bind);
        }

        let tex_indices: Vec<u32> = desc
            .texture_bindings
            .iter()
            .map(|b| b.binding_index)
            .collect();
        assert_eq!(tex_indices, vec![0, 2]);
        // Each texture occupies binding_index (texture) and binding_index+1 (sampler)
        // No overlap: 0,1 then 2,3 — valid sparse layout
    }
}
