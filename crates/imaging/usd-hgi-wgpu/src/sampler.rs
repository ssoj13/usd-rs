//! wgpu sampler implementation for HGI.
//!
//! Implements HgiSampler trait using wgpu::Sampler.

use usd_hgi::{HgiMipFilter, HgiSampler, HgiSamplerDesc};

use super::conversions;

/// wgpu-backed GPU sampler resource.
///
/// Maps HgiSamplerDesc properties (filter modes, address modes, LOD,
/// anisotropy, comparison) to a wgpu::Sampler.
#[derive(Debug)]
#[allow(dead_code)] // fields used by pub(crate) accessors, consumed by hgi.rs
pub struct WgpuSampler {
    desc: HgiSamplerDesc,
    sampler: wgpu::Sampler,
}

impl WgpuSampler {
    /// Create a new wgpu sampler from an HGI descriptor.
    pub fn new(device: &wgpu::Device, desc: &HgiSamplerDesc) -> Self {
        let label = if desc.debug_name.is_empty() {
            None
        } else {
            Some(desc.debug_name.as_str())
        };

        // Map compare function only when comparison mode is enabled
        let compare = if desc.enable_compare {
            Some(conversions::to_wgpu_compare_fn(desc.compare_function))
        } else {
            None
        };

        // Filter modes
        let mag_filter = conversions::to_wgpu_filter_mode(desc.mag_filter);
        let min_filter = conversions::to_wgpu_filter_mode(desc.min_filter);
        let mipmap_filter = match desc.mip_filter {
            HgiMipFilter::NotMipmapped | HgiMipFilter::Nearest => wgpu::FilterMode::Nearest,
            HgiMipFilter::Linear => wgpu::FilterMode::Linear,
        };

        // C++ ref (hgiGL/sampler.cpp L68-82): disable anisotropy when any filter
        // is Nearest, to preserve exact nearest-neighbor sampling semantics.
        // wgpu also requires all filters to be Linear when anisotropy_clamp > 1.
        let any_nearest = min_filter == wgpu::FilterMode::Nearest
            || mag_filter == wgpu::FilterMode::Nearest
            || mipmap_filter == wgpu::FilterMode::Nearest;
        let anisotropy_clamp = if any_nearest {
            1
        } else {
            (desc.max_anisotropy as u16).clamp(1, 16)
        };

        // LOD clamp: wgpu uses lod_min_clamp / lod_max_clamp
        // For NotMipmapped, clamp max to 0 to force mip level 0
        let (lod_min, lod_max) = if desc.mip_filter == HgiMipFilter::NotMipmapped {
            (0.0, 0.0)
        } else {
            (desc.min_lod.max(0.0), desc.max_lod.max(0.0))
        };

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label,
            address_mode_u: conversions::to_wgpu_address_mode(desc.address_mode_u),
            address_mode_v: conversions::to_wgpu_address_mode(desc.address_mode_v),
            address_mode_w: conversions::to_wgpu_address_mode(desc.address_mode_w),
            mag_filter,
            min_filter,
            mipmap_filter,
            lod_min_clamp: lod_min,
            lod_max_clamp: lod_max,
            compare,
            anisotropy_clamp,
            border_color: Some(conversions::to_wgpu_border_color(desc.border_color)),
        });

        Self {
            desc: desc.clone(),
            sampler,
        }
    }

    /// Access the inner wgpu::Sampler for bind group creation.
    #[allow(dead_code)] // will be used by bind group creation
    pub(crate) fn wgpu_sampler(&self) -> &wgpu::Sampler {
        &self.sampler
    }
}

impl HgiSampler for WgpuSampler {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn descriptor(&self) -> &HgiSamplerDesc {
        &self.desc
    }

    /// wgpu does not expose raw native handles through its safe API.
    /// Returns 0; use wgpu_sampler() for internal access.
    fn raw_resource(&self) -> u64 {
        0
    }
}
