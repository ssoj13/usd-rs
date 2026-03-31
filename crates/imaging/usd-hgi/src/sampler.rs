//! GPU sampler resources and descriptors

use super::enums::{
    HgiBorderColor, HgiCompareFunction, HgiMipFilter, HgiSamplerAddressMode, HgiSamplerFilter,
};
use super::handle::HgiHandle;

/// Describes the properties needed to create a GPU sampler
#[derive(Debug, Clone, PartialEq)]
pub struct HgiSamplerDesc {
    /// Debug label for GPU debugging
    pub debug_name: String,

    /// Magnification filter
    pub mag_filter: HgiSamplerFilter,

    /// Minification filter
    pub min_filter: HgiSamplerFilter,

    /// Mipmap filter
    pub mip_filter: HgiMipFilter,

    /// Addressing mode for U coordinate
    pub address_mode_u: HgiSamplerAddressMode,

    /// Addressing mode for V coordinate
    pub address_mode_v: HgiSamplerAddressMode,

    /// Addressing mode for W coordinate
    pub address_mode_w: HgiSamplerAddressMode,

    /// Border color for clamped texture values
    pub border_color: HgiBorderColor,

    /// Maximum anisotropy ratio (1 disables anisotropic filtering, 16 is max).
    ///
    /// Matches C++ `HgiSamplerDesc::maxAnisotropy`. Default is 16 per C++.
    /// Value of 1 effectively disables anisotropic sampling.
    pub max_anisotropy: u32,

    /// Enable comparison mode (for shadow sampling)
    pub enable_compare: bool,

    /// Comparison function (for shadow sampling)
    pub compare_function: HgiCompareFunction,

    /// Minimum LOD level
    pub min_lod: f32,

    /// Maximum LOD level
    pub max_lod: f32,

    /// LOD bias
    pub lod_bias: f32,
}

impl Default for HgiSamplerDesc {
    fn default() -> Self {
        // Defaults match C++ HgiSamplerDesc constructor:
        // magFilter=Nearest, minFilter=Nearest, mipFilter=NotMipmapped,
        // addressMode*=ClampToEdge, borderColor=TransparentBlack,
        // enableCompare=false, compareFunction=Never, maxAnisotropy=16
        Self {
            debug_name: String::new(),
            mag_filter: HgiSamplerFilter::Nearest,
            min_filter: HgiSamplerFilter::Nearest,
            mip_filter: HgiMipFilter::NotMipmapped,
            address_mode_u: HgiSamplerAddressMode::ClampToEdge,
            address_mode_v: HgiSamplerAddressMode::ClampToEdge,
            address_mode_w: HgiSamplerAddressMode::ClampToEdge,
            border_color: HgiBorderColor::TransparentBlack,
            max_anisotropy: 16,
            enable_compare: false,
            compare_function: HgiCompareFunction::Never,
            min_lod: -1000.0,
            max_lod: 1000.0,
            lod_bias: 0.0,
        }
    }
}

impl HgiSamplerDesc {
    /// Create a new sampler descriptor with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the debug name
    pub fn with_debug_name(mut self, name: impl Into<String>) -> Self {
        self.debug_name = name.into();
        self
    }

    /// Set the magnification filter
    pub fn with_mag_filter(mut self, filter: HgiSamplerFilter) -> Self {
        self.mag_filter = filter;
        self
    }

    /// Set the minification filter
    pub fn with_min_filter(mut self, filter: HgiSamplerFilter) -> Self {
        self.min_filter = filter;
        self
    }

    /// Set the mipmap filter
    pub fn with_mip_filter(mut self, filter: HgiMipFilter) -> Self {
        self.mip_filter = filter;
        self
    }

    /// Set all address modes at once
    pub fn with_address_mode(mut self, mode: HgiSamplerAddressMode) -> Self {
        self.address_mode_u = mode;
        self.address_mode_v = mode;
        self.address_mode_w = mode;
        self
    }

    /// Set U address mode
    pub fn with_address_mode_u(mut self, mode: HgiSamplerAddressMode) -> Self {
        self.address_mode_u = mode;
        self
    }

    /// Set V address mode
    pub fn with_address_mode_v(mut self, mode: HgiSamplerAddressMode) -> Self {
        self.address_mode_v = mode;
        self
    }

    /// Set W address mode
    pub fn with_address_mode_w(mut self, mode: HgiSamplerAddressMode) -> Self {
        self.address_mode_w = mode;
        self
    }

    /// Set border color
    pub fn with_border_color(mut self, color: HgiBorderColor) -> Self {
        self.border_color = color;
        self
    }

    /// Set maximum anisotropy level (1 disables, 16 is max)
    pub fn with_anisotropy(mut self, max_anisotropy: u32) -> Self {
        self.max_anisotropy = max_anisotropy;
        self
    }

    /// Enable comparison mode for shadow sampling
    pub fn with_compare(mut self, compare_fn: HgiCompareFunction) -> Self {
        self.enable_compare = true;
        self.compare_function = compare_fn;
        self
    }

    /// Set LOD range
    pub fn with_lod_range(mut self, min_lod: f32, max_lod: f32) -> Self {
        self.min_lod = min_lod;
        self.max_lod = max_lod;
        self
    }

    /// Set LOD bias
    pub fn with_lod_bias(mut self, bias: f32) -> Self {
        self.lod_bias = bias;
        self
    }
}

/// GPU sampler resource (abstract interface)
///
/// Represents a graphics platform independent GPU sampler resource.
/// Samplers should be created via Hgi::create_sampler().
pub trait HgiSampler: Send + Sync {
    /// Downcast to concrete type (for backend-specific operations)
    fn as_any(&self) -> &dyn std::any::Any;

    /// Get the descriptor that was used to create this sampler
    fn descriptor(&self) -> &HgiSamplerDesc;

    /// Returns the backend's raw GPU resource handle
    ///
    /// Platform-specific return values:
    /// - OpenGL: returns the GLuint sampler object
    /// - Metal: returns the id<MTLSamplerState> as u64
    /// - Vulkan: returns the VkSampler as u64
    /// - DX12: returns the D3D12_CPU_DESCRIPTOR_HANDLE as u64
    fn raw_resource(&self) -> u64;
}

/// Type alias for sampler handle
pub type HgiSamplerHandle = HgiHandle<dyn HgiSampler>;

/// Vector of sampler handles
pub type HgiSamplerHandleVector = Vec<HgiSamplerHandle>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sampler_desc_default() {
        let desc = HgiSamplerDesc::default();
        // C++ defaults: Nearest/Nearest/NotMipmapped, maxAnisotropy=16
        assert_eq!(desc.mag_filter, HgiSamplerFilter::Nearest);
        assert_eq!(desc.min_filter, HgiSamplerFilter::Nearest);
        assert_eq!(desc.mip_filter, HgiMipFilter::NotMipmapped);
        assert_eq!(desc.max_anisotropy, 16);
        assert!(!desc.enable_compare);
    }

    #[test]
    fn test_sampler_desc_builder() {
        let desc = HgiSamplerDesc::new()
            .with_debug_name("MySampler")
            .with_mag_filter(HgiSamplerFilter::Nearest)
            .with_min_filter(HgiSamplerFilter::Nearest)
            .with_mip_filter(HgiMipFilter::NotMipmapped)
            .with_address_mode(HgiSamplerAddressMode::Repeat)
            .with_anisotropy(16)
            .with_lod_range(0.0, 10.0);

        assert_eq!(desc.debug_name, "MySampler");
        assert_eq!(desc.mag_filter, HgiSamplerFilter::Nearest);
        assert_eq!(desc.address_mode_u, HgiSamplerAddressMode::Repeat);
        assert_eq!(desc.address_mode_v, HgiSamplerAddressMode::Repeat);
        assert_eq!(desc.address_mode_w, HgiSamplerAddressMode::Repeat);
        assert_eq!(desc.max_anisotropy, 16);
        assert_eq!(desc.min_lod, 0.0);
        assert_eq!(desc.max_lod, 10.0);
    }

    #[test]
    fn test_shadow_sampler() {
        let desc = HgiSamplerDesc::new()
            .with_compare(HgiCompareFunction::LEqual)
            .with_address_mode(HgiSamplerAddressMode::ClampToBorderColor)
            .with_border_color(HgiBorderColor::OpaqueWhite);

        assert!(desc.enable_compare);
        assert_eq!(desc.compare_function, HgiCompareFunction::LEqual);
        assert_eq!(desc.border_color, HgiBorderColor::OpaqueWhite);
    }

    // Mock implementation for testing
    struct MockSampler {
        desc: HgiSamplerDesc,
    }

    impl HgiSampler for MockSampler {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn descriptor(&self) -> &HgiSamplerDesc {
            &self.desc
        }

        fn raw_resource(&self) -> u64 {
            0
        }
    }

    #[test]
    fn test_sampler_trait() {
        let desc = HgiSamplerDesc::new().with_mag_filter(HgiSamplerFilter::Linear);

        let sampler = MockSampler { desc: desc.clone() };

        assert_eq!(sampler.descriptor().mag_filter, HgiSamplerFilter::Linear);
    }
}
