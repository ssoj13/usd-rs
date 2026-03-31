//! Metal texture resource. Port of pxr/imaging/hgiMetal/texture

use usd_hgi::{HgiFormat, HgiTexture, HgiTextureDesc, HgiTextureViewDesc};

/// Metal texture resource.
/// Mirrors C++ HgiMetalTexture.
#[derive(Debug)]
pub struct HgiMetalTexture {
    desc: HgiTextureDesc,
    // On real Metal: texture_id: id<MTLTexture>
}

impl HgiMetalTexture {
    /// Creates a new Metal texture from a texture descriptor.
    /// On real Metal, this would create via [device newTextureWithDescriptor:].
    pub fn new(desc: HgiTextureDesc) -> Self {
        Self { desc }
    }

    /// Creates a new Metal texture from a texture view descriptor.
    /// On real Metal, this would create a texture view via
    /// [sourceTexture newTextureViewWithPixelFormat:textureType:levels:slices:swizzle:].
    pub fn new_view(view_desc: &HgiTextureViewDesc) -> Self {
        // Use the source texture's descriptor if available, otherwise create a minimal one
        if let Some(source) = view_desc.source_texture.get() {
            let mut desc = source.descriptor().clone();
            if view_desc.format != HgiFormat::Invalid {
                desc.format = view_desc.format;
            }
            Self { desc }
        } else {
            Self {
                desc: HgiTextureDesc::new().with_format(view_desc.format),
            }
        }
    }

    /// Returns the Metal texture handle.
    /// Mirrors C++ GetTextureId().
    /// Stub: returns 0 (no real Metal texture).
    pub fn get_texture_id(&self) -> u64 {
        0
    }
}

impl HgiTexture for HgiMetalTexture {
    fn descriptor(&self) -> &HgiTextureDesc {
        &self.desc
    }
    fn byte_size_of_resource(&self) -> usize {
        // On real Metal, would query from MTLTexture
        0
    }
    fn raw_resource(&self) -> u64 {
        self.get_texture_id()
    }
    fn cpu_staging_address(&mut self) -> Option<*mut u8> {
        None
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
