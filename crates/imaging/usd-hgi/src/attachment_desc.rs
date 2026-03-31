//! Render target attachment descriptors

use super::enums::{HgiAttachmentLoadOp, HgiAttachmentStoreOp, HgiSampleCount};
use super::texture::HgiTextureHandle;
use super::types::HgiFormat;
use usd_gf::Vec4f;

/// Describes the properties of a render target attachment
///
/// This descriptor is used to specify how a texture should be used as an
/// attachment (color, depth, or stencil) in a render pass.
#[derive(Debug, Clone)]
pub struct HgiAttachmentDesc {
    /// Format of the attachment
    pub format: HgiFormat,

    /// What to do with attachment pixel data prior to rendering
    pub load_op: HgiAttachmentLoadOp,

    /// What to do with attachment pixel data after rendering
    pub store_op: HgiAttachmentStoreOp,

    /// Clear color (if load_op is Clear)
    pub clear_value: Vec4f,

    /// The texture to use as attachment
    pub texture: HgiTextureHandle,

    /// For texture arrays, which layer to use (default: 0)
    pub layer_index: u32,

    /// Which mip level to use (default: 0)
    pub mip_level: u32,

    /// Sample count for multi-sampling
    pub sample_count: HgiSampleCount,

    /// Blend enabled for this attachment (color attachments only)
    pub blend_enabled: bool,
}

impl Default for HgiAttachmentDesc {
    fn default() -> Self {
        // C++: format(Invalid), usage(0), loadOp(Load), storeOp(Store),
        // clearValue(0), colorMask(RGBA), blendEnabled(false),
        // srcColor/dstColor/srcAlpha/dstAlpha BlendFactor(Zero), blendOp(Add),
        // blendConstantColor(0,0,0,0)
        Self {
            format: HgiFormat::Invalid,
            load_op: HgiAttachmentLoadOp::Load,
            store_op: HgiAttachmentStoreOp::Store,
            clear_value: Vec4f::new(0.0, 0.0, 0.0, 0.0),
            texture: HgiTextureHandle::null(),
            layer_index: 0,
            mip_level: 0,
            sample_count: HgiSampleCount::Count1,
            blend_enabled: false,
        }
    }
}

impl HgiAttachmentDesc {
    /// Create a new attachment descriptor with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the format
    pub fn with_format(mut self, format: HgiFormat) -> Self {
        self.format = format;
        self
    }

    /// Set the load operation
    pub fn with_load_op(mut self, load_op: HgiAttachmentLoadOp) -> Self {
        self.load_op = load_op;
        self
    }

    /// Set the store operation
    pub fn with_store_op(mut self, store_op: HgiAttachmentStoreOp) -> Self {
        self.store_op = store_op;
        self
    }

    /// Set the clear value
    pub fn with_clear_value(mut self, clear_value: Vec4f) -> Self {
        self.clear_value = clear_value;
        self
    }

    /// Set the texture
    pub fn with_texture(mut self, texture: HgiTextureHandle) -> Self {
        self.texture = texture;
        self
    }

    /// Set the layer index
    pub fn with_layer_index(mut self, layer_index: u32) -> Self {
        self.layer_index = layer_index;
        self
    }

    /// Set the mip level
    pub fn with_mip_level(mut self, mip_level: u32) -> Self {
        self.mip_level = mip_level;
        self
    }

    /// Set the sample count
    pub fn with_sample_count(mut self, sample_count: HgiSampleCount) -> Self {
        self.sample_count = sample_count;
        self
    }

    /// Enable/disable blending
    pub fn with_blend_enabled(mut self, enabled: bool) -> Self {
        self.blend_enabled = enabled;
        self
    }

    /// Check if this is a valid attachment descriptor
    pub fn is_valid(&self) -> bool {
        self.format != HgiFormat::Invalid && self.texture.is_valid()
    }
}

impl PartialEq for HgiAttachmentDesc {
    fn eq(&self, other: &Self) -> bool {
        self.format == other.format
            && self.load_op == other.load_op
            && self.store_op == other.store_op
            && self.clear_value == other.clear_value
            && self.texture == other.texture
            && self.layer_index == other.layer_index
            && self.mip_level == other.mip_level
            && self.sample_count == other.sample_count
            && self.blend_enabled == other.blend_enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_attachment() {
        let desc = HgiAttachmentDesc::default();
        assert_eq!(desc.format, HgiFormat::Invalid);
        // C++ defaults: Load/Store (not DontCare)
        assert_eq!(desc.load_op, HgiAttachmentLoadOp::Load);
        assert_eq!(desc.store_op, HgiAttachmentStoreOp::Store);
        // clearValue is (0,0,0,0) per C++
        assert_eq!(desc.clear_value, usd_gf::Vec4f::new(0.0, 0.0, 0.0, 0.0));
        assert!(!desc.is_valid());
    }

    #[test]
    fn test_builder_pattern() {
        let desc = HgiAttachmentDesc::new()
            .with_format(HgiFormat::UNorm8Vec4)
            .with_load_op(HgiAttachmentLoadOp::Clear)
            .with_store_op(HgiAttachmentStoreOp::Store)
            .with_clear_value(Vec4f::new(0.2, 0.3, 0.4, 1.0))
            .with_sample_count(HgiSampleCount::Count4)
            .with_blend_enabled(true);

        assert_eq!(desc.format, HgiFormat::UNorm8Vec4);
        assert_eq!(desc.load_op, HgiAttachmentLoadOp::Clear);
        assert_eq!(desc.store_op, HgiAttachmentStoreOp::Store);
        assert_eq!(desc.sample_count, HgiSampleCount::Count4);
        assert!(desc.blend_enabled);
    }
}
