//! Vulkan sampler resource.
//!
//! Port of pxr/imaging/hgiVulkan/sampler.cpp/.h

#![allow(unsafe_code)]

use ash::vk;
use ash::vk::Handle as VkHandle;
use usd_hgi::{HgiMipFilter, HgiSampler, HgiSamplerDesc, HgiSamplerFilter};

use crate::capabilities::HgiVulkanCapabilities;
use crate::conversions::HgiVulkanConversions;

/// Vulkan sampler resource.
///
/// Wraps a `VkSampler` created from an `HgiSamplerDesc`.
/// When constructed via `new_stub` (no live device), the sampler handle is
/// null and `drop` skips destruction.
pub struct HgiVulkanSampler {
    desc: HgiSamplerDesc,
    /// Logical device used for destruction; `None` in stub mode.
    device: Option<ash::Device>,
    vk_sampler: vk::Sampler,
    inflight_bits: u64,
}

impl HgiVulkanSampler {
    /// Creates a descriptor-only stub sampler without a live Vulkan device.
    ///
    /// Used by `HgiVulkan` (stub backend) where no `ash::Device` is available.
    /// `raw_resource()` returns 0 and `Drop` skips `vkDestroySampler`.
    pub fn new_stub(desc: HgiSamplerDesc) -> Self {
        Self {
            desc,
            device: None,
            vk_sampler: vk::Sampler::null(),
            inflight_bits: 0,
        }
    }

    /// Creates a `VkSampler` from the descriptor using the given device.
    ///
    /// Mirrors `HgiVulkanSampler::HgiVulkanSampler(device, desc)`.
    pub fn new(
        device: &ash::Device,
        capabilities: &HgiVulkanCapabilities,
        desc: &HgiSamplerDesc,
    ) -> Result<Self, vk::Result> {
        let vk_sampler = create_vk_sampler(device, capabilities, desc)?;
        Ok(Self {
            desc: desc.clone(),
            // ash::Device is Clone — it shares the underlying dispatch table.
            device: Some(device.clone()),
            vk_sampler,
            inflight_bits: 0,
        })
    }

    /// Returns the underlying `VkSampler` handle.
    pub fn vk_sampler(&self) -> vk::Sampler {
        self.vk_sampler
    }

    /// Returns the inflight-bits bitmask used by the garbage collector.
    pub fn inflight_bits(&self) -> u64 {
        self.inflight_bits
    }

    /// Sets the inflight-bits bitmask.
    pub fn set_inflight_bits(&mut self, bits: u64) {
        self.inflight_bits = bits;
    }
}

impl HgiSampler for HgiVulkanSampler {
    fn descriptor(&self) -> &HgiSamplerDesc {
        &self.desc
    }

    fn raw_resource(&self) -> u64 {
        self.vk_sampler.as_raw()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl Drop for HgiVulkanSampler {
    fn drop(&mut self) {
        // Stub samplers (no device) have a null handle — nothing to destroy.
        if let Some(device) = &self.device {
            // SAFETY: vk_sampler was created by this device and is no longer in use.
            unsafe {
                device.destroy_sampler(self.vk_sampler, None);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Internal construction helper
// ---------------------------------------------------------------------------

/// Builds `VkSamplerCreateInfo` and calls `vkCreateSampler`.
///
/// Mirrors the constructor body of `HgiVulkanSampler` in C++.
fn create_vk_sampler(
    device: &ash::Device,
    capabilities: &HgiVulkanCapabilities,
    desc: &HgiSamplerDesc,
) -> Result<vk::Sampler, vk::Result> {
    // Anisotropy is enabled only when both min and mag filters are not Nearest,
    // or when the mip filter is Linear — matching the C++ condition:
    //   if ((desc.minFilter != Nearest || mipFilter == Linear) && magFilter != Nearest)
    let wants_anisotropy = (desc.min_filter != HgiSamplerFilter::Nearest
        || desc.mip_filter == HgiMipFilter::Linear)
        && desc.mag_filter != HgiSamplerFilter::Nearest;

    let (anisotropy_enable, max_anisotropy) = if wants_anisotropy {
        let supported = capabilities.vk_device_features2.features.sampler_anisotropy == vk::TRUE;
        if supported {
            let device_max = capabilities
                .vk_device_properties2
                .properties
                .limits
                .max_sampler_anisotropy;
            // Clamp to hardware limit; descriptor value acts as an upper bound.
            let clamped = device_max.min(desc.max_anisotropy as f32);
            (vk::TRUE, clamped)
        } else {
            (vk::FALSE, 1.0_f32)
        }
    } else {
        (vk::FALSE, 1.0_f32)
    };

    // maxLod: 0.25 when not mipmapped (emulates OpenGL behaviour per Vulkan spec
    // recommendation), VK_LOD_CLAMP_NONE otherwise.
    let max_lod = if desc.mip_filter == HgiMipFilter::NotMipmapped {
        0.25_f32
    } else {
        vk::LOD_CLAMP_NONE
    };

    let create_info = vk::SamplerCreateInfo {
        mag_filter: HgiVulkanConversions::get_min_mag_filter(desc.mag_filter),
        min_filter: HgiVulkanConversions::get_min_mag_filter(desc.min_filter),
        mipmap_mode: HgiVulkanConversions::get_mip_filter(desc.mip_filter),
        address_mode_u: HgiVulkanConversions::get_sampler_address_mode(desc.address_mode_u),
        address_mode_v: HgiVulkanConversions::get_sampler_address_mode(desc.address_mode_v),
        address_mode_w: HgiVulkanConversions::get_sampler_address_mode(desc.address_mode_w),
        mip_lod_bias: 0.0,
        anisotropy_enable,
        max_anisotropy,
        // Percentage-closer filtering (shadow sampling).
        compare_enable: if desc.enable_compare {
            vk::TRUE
        } else {
            vk::FALSE
        },
        compare_op: HgiVulkanConversions::get_depth_compare_function(desc.compare_function),
        min_lod: 0.0,
        max_lod,
        border_color: HgiVulkanConversions::get_border_color(desc.border_color),
        // Default fills in s_type, p_next, flags, unnormalized_coordinates.
        ..Default::default()
    };

    // SAFETY: device is valid, create_info is correctly initialised.
    unsafe { device.create_sampler(&create_info, None) }
}
