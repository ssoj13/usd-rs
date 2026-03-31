//! GPU texture resources and descriptors

use super::enums::{HgiComponentSwizzle, HgiSampleCount, HgiTextureType, HgiTextureUsage};
use super::handle::HgiHandle;
use super::types::HgiFormat;
use usd_gf::Vec3i;

/// Describes color component mapping (swizzling)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HgiComponentMapping {
    /// What component is used for red channel
    pub r: HgiComponentSwizzle,
    /// What component is used for green channel
    pub g: HgiComponentSwizzle,
    /// What component is used for blue channel
    pub b: HgiComponentSwizzle,
    /// What component is used for alpha channel
    pub a: HgiComponentSwizzle,
}

impl Default for HgiComponentMapping {
    fn default() -> Self {
        Self {
            r: HgiComponentSwizzle::R,
            g: HgiComponentSwizzle::G,
            b: HgiComponentSwizzle::B,
            a: HgiComponentSwizzle::A,
        }
    }
}

impl HgiComponentMapping {
    /// Create identity component mapping (RGBA -> RGBA)
    pub fn identity() -> Self {
        Self::default()
    }

    /// Create a custom component mapping
    pub fn new(
        r: HgiComponentSwizzle,
        g: HgiComponentSwizzle,
        b: HgiComponentSwizzle,
        a: HgiComponentSwizzle,
    ) -> Self {
        Self { r, g, b, a }
    }
}

/// Describes the properties needed to create a GPU texture
#[derive(Debug, Clone)]
pub struct HgiTextureDesc {
    /// Debug label for GPU debugging
    pub debug_name: String,

    /// Describes how the texture is intended to be used
    pub usage: HgiTextureUsage,

    /// The format of the texture
    pub format: HgiFormat,

    /// The mapping of rgba components when accessing the texture
    pub component_mapping: HgiComponentMapping,

    /// The resolution of the texture (width, height, depth)
    pub dimensions: Vec3i,

    /// Type of texture (1D, 2D, 3D, Cube, etc.)
    pub texture_type: HgiTextureType,

    /// The number of layers (for texture arrays)
    pub layer_count: u16,

    /// The number of mip levels in texture
    pub mip_levels: u16,

    /// Samples per texel (multi-sampling)
    pub sample_count: HgiSampleCount,
    // Initial data is handled separately in Rust (not stored in descriptor)
    // Backend implementations will accept initial data during texture creation
}

impl Default for HgiTextureDesc {
    fn default() -> Self {
        Self {
            debug_name: String::new(),
            usage: HgiTextureUsage::empty(),
            format: HgiFormat::Invalid,
            component_mapping: HgiComponentMapping::identity(),
            dimensions: Vec3i::new(0, 0, 0),
            texture_type: HgiTextureType::Texture2D,
            layer_count: 1,
            mip_levels: 1,
            sample_count: HgiSampleCount::Count1,
        }
    }
}

impl HgiTextureDesc {
    /// Create a new texture descriptor
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the debug name
    pub fn with_debug_name(mut self, name: impl Into<String>) -> Self {
        self.debug_name = name.into();
        self
    }

    /// Set the usage flags
    pub fn with_usage(mut self, usage: HgiTextureUsage) -> Self {
        self.usage = usage;
        self
    }

    /// Set the format
    pub fn with_format(mut self, format: HgiFormat) -> Self {
        self.format = format;
        self
    }

    /// Set the component mapping
    pub fn with_component_mapping(mut self, mapping: HgiComponentMapping) -> Self {
        self.component_mapping = mapping;
        self
    }

    /// Set the dimensions
    pub fn with_dimensions(mut self, dimensions: Vec3i) -> Self {
        self.dimensions = dimensions;
        self
    }

    /// Set the texture type
    pub fn with_texture_type(mut self, texture_type: HgiTextureType) -> Self {
        self.texture_type = texture_type;
        self
    }

    /// Set the layer count
    pub fn with_layer_count(mut self, layer_count: u16) -> Self {
        self.layer_count = layer_count;
        self
    }

    /// Set the mip levels
    pub fn with_mip_levels(mut self, mip_levels: u16) -> Self {
        self.mip_levels = mip_levels;
        self
    }

    /// Set the sample count
    pub fn with_sample_count(mut self, sample_count: HgiSampleCount) -> Self {
        self.sample_count = sample_count;
        self
    }

    /// Check if this is a valid descriptor
    pub fn is_valid(&self) -> bool {
        self.format != HgiFormat::Invalid
            && !self.usage.is_empty()
            && self.dimensions[0] > 0
            && self.dimensions[1] > 0
    }
}

impl PartialEq for HgiTextureDesc {
    fn eq(&self, other: &Self) -> bool {
        self.debug_name == other.debug_name
            && self.usage == other.usage
            && self.format == other.format
            && self.component_mapping == other.component_mapping
            && self.dimensions == other.dimensions
            && self.texture_type == other.texture_type
            && self.layer_count == other.layer_count
            && self.mip_levels == other.mip_levels
            && self.sample_count == other.sample_count
    }
}

/// GPU texture resource (abstract interface)
///
/// Represents a graphics platform independent GPU texture resource.
/// Textures should be created via Hgi::create_texture().
pub trait HgiTexture: Send + Sync {
    /// Downcast to concrete type (for backend-specific operations)
    fn as_any(&self) -> &dyn std::any::Any;

    /// Get the descriptor that was used to create this texture
    fn descriptor(&self) -> &HgiTextureDesc;

    /// Returns the byte size of the GPU texture
    ///
    /// This can be helpful if the application wishes to tally up memory usage.
    fn byte_size_of_resource(&self) -> usize;

    /// Returns the backend's raw GPU resource handle
    ///
    /// Platform-specific return values:
    /// - OpenGL: returns the GLuint resource name
    /// - Metal: returns the id<MTLTexture> as u64
    /// - Vulkan: returns the VkImage as u64
    /// - DX12: returns the ID3D12Resource pointer as u64
    fn raw_resource(&self) -> u64;

    /// Returns a CPU staging address for uploading data
    ///
    /// Some implementations (e.g. Metal) may have built-in support for
    /// queueing up CPU->GPU copies. Those implementations can return the
    /// CPU pointer to the texture's content directly.
    ///
    /// Returns None if CPU staging is not supported by the backend.
    fn cpu_staging_address(&mut self) -> Option<*mut u8>;

    /// Submit a layout transition for this texture.
    ///
    /// Some backends (e.g. Vulkan) require explicit image layout transitions.
    /// The new_layout usage flags describe the intended usage after the transition.
    /// Returns the previous layout (C++ `SubmitLayoutChange` returns `HgiTextureUsage`).
    fn submit_layout_change(&mut self, _new_layout: HgiTextureUsage) -> HgiTextureUsage {
        // Default: no-op. Returns empty (no previous layout known).
        // Vulkan overrides to perform VkImageLayout transitions and return prior layout.
        HgiTextureUsage::empty()
    }
}

/// Type alias for texture handle
pub type HgiTextureHandle = HgiHandle<dyn HgiTexture>;

/// Vector of texture handles
pub type HgiTextureHandleVector = Vec<HgiTextureHandle>;

/// Describes the properties needed to create a GPU texture view from an existing texture
///
/// A texture view aliases the data of another texture, providing a different format
/// or accessing a subset of layers/mips.
#[derive(Debug, Clone)]
pub struct HgiTextureViewDesc {
    /// Debug label for GPU debugging
    pub debug_name: String,

    /// Handle to the source texture to be aliased
    pub source_texture: HgiTextureHandle,

    /// The format of the texture view (must be compatible with source texture)
    ///
    /// Generally: All 8-, 16-, 32-, 64-, and 128-bit color formats are compatible
    /// with other formats with the same bit length.
    /// For example HgiFormat::Float32Vec4 and HgiFormat::Int32Vec4 are compatible.
    pub format: HgiFormat,

    /// The layer index to use from the source texture as the first layer of the view
    pub source_first_layer: u16,

    /// The number of layers (texture-arrays)
    pub source_layer_count: u16,

    /// The mip index to use from the source texture as the first mip of the view
    pub source_first_mip: u16,

    /// The number of mip levels in the view
    pub source_mip_count: u16,
}

impl Default for HgiTextureViewDesc {
    fn default() -> Self {
        Self {
            debug_name: String::new(),
            source_texture: HgiTextureHandle::null(),
            format: HgiFormat::Invalid,
            source_first_layer: 0,
            source_layer_count: 1,
            source_first_mip: 0,
            source_mip_count: 1,
        }
    }
}

impl HgiTextureViewDesc {
    /// Create a new texture view descriptor
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the debug name
    pub fn with_debug_name(mut self, name: impl Into<String>) -> Self {
        self.debug_name = name.into();
        self
    }

    /// Set the source texture
    pub fn with_source_texture(mut self, source: HgiTextureHandle) -> Self {
        self.source_texture = source;
        self
    }

    /// Set the view format
    pub fn with_format(mut self, format: HgiFormat) -> Self {
        self.format = format;
        self
    }

    /// Set the source layer range
    pub fn with_source_layers(mut self, first_layer: u16, layer_count: u16) -> Self {
        self.source_first_layer = first_layer;
        self.source_layer_count = layer_count;
        self
    }

    /// Set the source mip range
    pub fn with_source_mips(mut self, first_mip: u16, mip_count: u16) -> Self {
        self.source_first_mip = first_mip;
        self.source_mip_count = mip_count;
        self
    }
}

impl PartialEq for HgiTextureViewDesc {
    fn eq(&self, other: &Self) -> bool {
        self.debug_name == other.debug_name
            && self.source_texture == other.source_texture
            && self.format == other.format
            && self.source_first_layer == other.source_first_layer
            && self.source_layer_count == other.source_layer_count
            && self.source_first_mip == other.source_first_mip
            && self.source_mip_count == other.source_mip_count
    }
}

/// Type alias for texture view handle
pub type HgiTextureViewHandle = HgiHandle<dyn HgiTexture>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_component_mapping() {
        let mapping = HgiComponentMapping::identity();
        assert_eq!(mapping.r, HgiComponentSwizzle::R);
        assert_eq!(mapping.g, HgiComponentSwizzle::G);
        assert_eq!(mapping.b, HgiComponentSwizzle::B);
        assert_eq!(mapping.a, HgiComponentSwizzle::A);

        let custom = HgiComponentMapping::new(
            HgiComponentSwizzle::One,
            HgiComponentSwizzle::One,
            HgiComponentSwizzle::One,
            HgiComponentSwizzle::R,
        );
        assert_eq!(custom.r, HgiComponentSwizzle::One);
        assert_eq!(custom.a, HgiComponentSwizzle::R);
    }

    #[test]
    fn test_texture_desc_default() {
        let desc = HgiTextureDesc::default();
        assert_eq!(desc.format, HgiFormat::Invalid);
        assert_eq!(desc.texture_type, HgiTextureType::Texture2D);
        assert_eq!(desc.layer_count, 1);
        assert_eq!(desc.mip_levels, 1);
        assert!(!desc.is_valid());
    }

    #[test]
    fn test_texture_desc_builder() {
        let desc = HgiTextureDesc::new()
            .with_debug_name("MyTexture")
            .with_format(HgiFormat::UNorm8Vec4)
            .with_usage(HgiTextureUsage::COLOR_TARGET | HgiTextureUsage::SHADER_READ)
            .with_dimensions(Vec3i::new(1024, 768, 1))
            .with_texture_type(HgiTextureType::Texture2D)
            .with_mip_levels(4)
            .with_sample_count(HgiSampleCount::Count4);

        assert_eq!(desc.debug_name, "MyTexture");
        assert_eq!(desc.format, HgiFormat::UNorm8Vec4);
        assert!(desc.usage.contains(HgiTextureUsage::COLOR_TARGET));
        assert!(desc.usage.contains(HgiTextureUsage::SHADER_READ));
        assert_eq!(desc.dimensions, Vec3i::new(1024, 768, 1));
        assert_eq!(desc.mip_levels, 4);
        assert_eq!(desc.sample_count, HgiSampleCount::Count4);
        assert!(desc.is_valid());
    }

    // Mock implementation for testing
    struct MockTexture {
        desc: HgiTextureDesc,
    }

    impl HgiTexture for MockTexture {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn descriptor(&self) -> &HgiTextureDesc {
            &self.desc
        }

        fn byte_size_of_resource(&self) -> usize {
            let dims = &self.desc.dimensions;
            (dims[0] as usize) * (dims[1] as usize) * (dims[2] as usize) * 4
        }

        fn raw_resource(&self) -> u64 {
            0
        }

        fn cpu_staging_address(&mut self) -> Option<*mut u8> {
            None
        }
    }

    #[test]
    fn test_texture_trait() {
        let desc = HgiTextureDesc::new()
            .with_format(HgiFormat::UNorm8Vec4)
            .with_dimensions(Vec3i::new(256, 256, 1));

        let texture = MockTexture { desc: desc.clone() };

        assert_eq!(texture.descriptor().format, HgiFormat::UNorm8Vec4);
        assert_eq!(texture.descriptor().dimensions, Vec3i::new(256, 256, 1));
        assert_eq!(texture.byte_size_of_resource(), 256 * 256 * 1 * 4);
    }
}
