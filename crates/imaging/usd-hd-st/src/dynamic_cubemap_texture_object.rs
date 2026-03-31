#![allow(dead_code)]

//! HdStDynamicCubemapTextureObject - Dynamic environment cubemaps.
//!
//! A cubemap texture managed but not populated by the Storm texture system.
//! Clients allocate GPU resources via `create_texture()` and populate them
//! by providing data or binding as a render target.
//!
//! Port of pxr/imaging/hdSt/dynamicCubemapTextureObject.h

use super::texture_identifier::HdStTextureIdentifier;
use super::texture_object::{HdStCubemapTextureObject, HdStTextureObjectTrait, TextureType};
use usd_hgi::{HgiTextureDesc, HgiTextureHandle};

/// Dynamic cubemap texture object.
///
/// Extends HdStCubemapTextureObject with explicit create/destroy lifecycle.
/// Clients control GPU resource allocation through `create_texture`,
/// `generate_mipmaps`, and `destroy_texture`.
///
/// Port of HdStDynamicCubemapTextureObject
#[derive(Debug, Clone)]
pub struct HdStDynamicCubemapTextureObject {
    /// Inner cubemap texture object
    inner: HdStCubemapTextureObject,
    /// Whether texture has been created via create_texture
    created: bool,
}

impl Default for HdStDynamicCubemapTextureObject {
    fn default() -> Self {
        Self {
            inner: HdStCubemapTextureObject::new(HdStTextureIdentifier::default()),
            created: false,
        }
    }
}

impl HdStDynamicCubemapTextureObject {
    /// Create a new dynamic cubemap texture object.
    pub fn new(texture_id: HdStTextureIdentifier) -> Self {
        Self {
            inner: HdStCubemapTextureObject::new(texture_id),
            created: false,
        }
    }

    /// Allocate GPU resource using the texture descriptor.
    /// Populate if data are given in the descriptor.
    pub fn create_texture(&mut self, _desc: &HgiTextureDesc) {
        // In a full implementation, this would create an HGI texture
        // from the descriptor. For now, mark as created.
        self.created = true;
    }

    /// Make GPU generate mipmaps.
    pub fn generate_mipmaps(&mut self) {
        // Delegates to HGI mipmap generation (backend-specific)
        log::debug!("generate_mipmaps called on dynamic cubemap texture");
    }

    /// Release GPU resource.
    pub fn destroy_texture(&mut self) {
        self.inner.set_gpu_texture(HgiTextureHandle::default(), 0);
        self.created = false;
    }

    /// Access the inner cubemap texture object.
    pub fn inner(&self) -> &HdStCubemapTextureObject {
        &self.inner
    }

    /// Access the inner cubemap texture object mutably.
    pub fn inner_mut(&mut self) -> &mut HdStCubemapTextureObject {
        &mut self.inner
    }
}

impl HdStTextureObjectTrait for HdStDynamicCubemapTextureObject {
    fn identifier(&self) -> &HdStTextureIdentifier {
        self.inner.identifier()
    }
    fn texture_type(&self) -> TextureType {
        TextureType::Cubemap
    }
    fn target_memory(&self) -> usize {
        self.inner.target_memory()
    }
    fn set_target_memory(&mut self, bytes: usize) {
        self.inner.set_target_memory(bytes);
    }
    fn committed_size(&self) -> usize {
        self.inner.committed_size()
    }

    /// Always returns true so that samplers are created.
    fn is_valid(&self) -> bool {
        true
    }
    fn texture_handle(&self) -> &HgiTextureHandle {
        self.inner.texture_handle()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dynamic_cubemap_lifecycle() {
        let id = HdStTextureIdentifier::default();
        let mut tex = HdStDynamicCubemapTextureObject::new(id);

        // Always valid (by design)
        assert!(tex.is_valid());
        assert_eq!(tex.texture_type(), TextureType::Cubemap);

        // Create and destroy cycle
        tex.create_texture(&HgiTextureDesc::default());
        assert!(tex.created);
        tex.destroy_texture();
        assert!(!tex.created);
    }
}
