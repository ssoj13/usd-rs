//! Descriptor for creating graphics command buffers

use super::attachment_desc::HgiAttachmentDesc;
use super::texture::HgiTextureHandle;

/// Describes the properties to begin a HgiGraphicsCmds.
///
/// Specifies color/depth attachments and resolve targets for a render pass.
#[derive(Debug, Clone, Default)]
pub struct HgiGraphicsCmdsDesc {
    /// Color attachment descriptors (load/store ops, clear values, format)
    pub color_attachment_descs: Vec<HgiAttachmentDesc>,

    /// Depth attachment descriptor (optional)
    pub depth_attachment_desc: HgiAttachmentDesc,

    /// Color attachment render target textures
    pub color_textures: Vec<HgiTextureHandle>,

    /// Optional MSAA resolve targets for color attachments
    pub color_resolve_textures: Vec<HgiTextureHandle>,

    /// Depth attachment render target (optional)
    pub depth_texture: HgiTextureHandle,

    /// Optional MSAA resolve target for depth attachment
    pub depth_resolve_texture: HgiTextureHandle,
}

impl HgiGraphicsCmdsDesc {
    /// Create a new empty descriptor.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true if any color or depth attachments are configured.
    pub fn has_attachments(&self) -> bool {
        !self.color_attachment_descs.is_empty() || self.depth_texture.is_valid()
    }
}

impl PartialEq for HgiGraphicsCmdsDesc {
    fn eq(&self, other: &Self) -> bool {
        self.color_attachment_descs == other.color_attachment_descs
            && self.depth_attachment_desc == other.depth_attachment_desc
            && self.color_textures == other.color_textures
            && self.color_resolve_textures == other.color_resolve_textures
            && self.depth_texture == other.depth_texture
            && self.depth_resolve_texture == other.depth_resolve_texture
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_desc() {
        let desc = HgiGraphicsCmdsDesc::new();
        assert!(!desc.has_attachments());
        assert!(desc.color_attachment_descs.is_empty());
        assert!(desc.color_textures.is_empty());
    }
}
