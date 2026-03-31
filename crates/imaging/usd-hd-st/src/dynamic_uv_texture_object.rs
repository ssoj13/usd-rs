#![allow(dead_code)]

//! HdStDynamicUvTextureObject - Client-managed dynamic UV textures.
//!
//! A UV texture that is managed but not populated by the Storm texture
//! system. Clients allocate GPU resources via `create_texture()` and
//! populate them by providing data in the descriptor or by binding the
//! texture as a render target.
//!
//! Used for AOVs, procedural textures, and render-to-texture scenarios.
//!
//! Port of pxr/imaging/hdSt/dynamicUvTextureObject.h

use super::texture_cpu_data::HdStTextureCpuData;
use super::texture_identifier::HdStTextureIdentifier;
use super::texture_object::{HdStTextureObjectTrait, TextureType};
use usd_gf::Vec3i;
use usd_hd::enums::HdWrap;
use usd_hgi::{HgiFormat, HgiTextureDesc, HgiTextureHandle};

/// Dynamic UV texture object managed by external clients.
///
/// Unlike asset-backed textures, the Storm texture system does not load
/// these from files. Instead, clients call `create_texture()` to allocate
/// GPU resources and populate data externally.
///
/// # Lifecycle
/// 1. Client creates via registry with a DynamicUvSubtextureIdentifier
/// 2. Client calls `create_texture(desc)` to allocate GPU resource
/// 3. Client populates texture (via desc.initial_data or render target)
/// 4. Optional: `generate_mipmaps()` for mip generation
/// 5. Client calls `destroy_texture()` when done
///
/// Port of HdStDynamicUvTextureObject
#[derive(Debug, Clone)]
pub struct HdStDynamicUvTextureObject {
    /// Texture identifier
    identifier: HdStTextureIdentifier,
    /// GPU texture handle
    gpu_texture: HgiTextureHandle,
    /// Texture dimensions
    dimensions: Vec3i,
    /// Pixel format
    format: HgiFormat,
    /// GPU memory committed
    byte_size: usize,
    /// Target memory budget
    target_memory: usize,
    /// Wrap mode hints from texture file metadata
    wrap_params: (HdWrap, HdWrap),
    /// CPU data (between load and commit phases)
    cpu_data: Option<HdStTextureCpuData>,
    /// Whether texture has been created
    created: bool,
}

impl HdStDynamicUvTextureObject {
    /// Create a new dynamic UV texture object.
    pub fn new(identifier: HdStTextureIdentifier) -> Self {
        Self {
            identifier,
            gpu_texture: HgiTextureHandle::default(),
            dimensions: Vec3i::new(0, 0, 0),
            format: HgiFormat::Invalid,
            byte_size: 0,
            target_memory: 0,
            wrap_params: (HdWrap::NoOpinion, HdWrap::NoOpinion),
            cpu_data: None,
            created: false,
        }
    }

    /// Allocate GPU resource from a texture descriptor.
    ///
    /// If the descriptor contains initial data, the texture is also populated.
    /// Must be called before the texture commit phase finishes for bindless
    /// sampler handles to be created correctly.
    pub fn create_texture(&mut self, desc: &HgiTextureDesc) {
        self.dimensions = desc.dimensions;
        self.format = desc.format;
        // Actual GPU allocation deferred to HGI
        self.gpu_texture = HgiTextureHandle::default();
        self.created = true;
    }

    /// Request GPU mipmap generation.
    ///
    /// Only valid after `create_texture()` has been called.
    pub fn generate_mipmaps(&self) {
        if !self.created {
            log::warn!("generate_mipmaps called on uncreated dynamic texture");
        }
        // Actual mipmap generation deferred to HGI backend
    }

    /// Release GPU resources.
    pub fn destroy_texture(&mut self) {
        self.gpu_texture = HgiTextureHandle::default();
        self.byte_size = 0;
        self.created = false;
    }

    /// Set wrap mode hints (typically from file metadata or client).
    pub fn set_wrap_params(&mut self, wrap_s: HdWrap, wrap_t: HdWrap) {
        self.wrap_params = (wrap_s, wrap_t);
    }

    /// Get wrap parameter opinions.
    pub fn wrap_params(&self) -> &(HdWrap, HdWrap) {
        &self.wrap_params
    }

    /// Store CPU data for upload during commit phase.
    pub fn set_cpu_data(&mut self, cpu_data: HdStTextureCpuData) {
        self.dimensions = cpu_data.dimensions();
        self.format = cpu_data.format();
        self.cpu_data = Some(cpu_data);
    }

    /// Get CPU data if set.
    pub fn cpu_data(&self) -> Option<&HdStTextureCpuData> {
        self.cpu_data.as_ref()
    }

    /// Take ownership of CPU data (clears internal reference).
    pub fn take_cpu_data(&mut self) -> Option<HdStTextureCpuData> {
        self.cpu_data.take()
    }

    /// Whether texture has been created.
    pub fn is_created(&self) -> bool {
        self.created
    }

    /// Get texture dimensions.
    pub fn dimensions(&self) -> Vec3i {
        self.dimensions
    }

    /// Get pixel format.
    pub fn format(&self) -> HgiFormat {
        self.format
    }

    /// Set GPU texture handle (from commit phase).
    pub fn set_gpu_texture(&mut self, handle: HgiTextureHandle, byte_size: usize) {
        self.gpu_texture = handle;
        self.byte_size = byte_size;
    }
}

impl HdStTextureObjectTrait for HdStDynamicUvTextureObject {
    fn identifier(&self) -> &HdStTextureIdentifier {
        &self.identifier
    }

    fn texture_type(&self) -> TextureType {
        TextureType::Uv
    }

    fn target_memory(&self) -> usize {
        self.target_memory
    }

    fn set_target_memory(&mut self, bytes: usize) {
        self.target_memory = bytes;
    }

    fn committed_size(&self) -> usize {
        self.byte_size
    }

    /// Dynamic textures are always valid (so samplers are created).
    fn is_valid(&self) -> bool {
        true
    }

    fn texture_handle(&self) -> &HgiTextureHandle {
        &self.gpu_texture
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_sdf::AssetPath;

    #[test]
    fn test_dynamic_texture_creation() {
        let id = HdStTextureIdentifier::from_path(AssetPath::new("dynamic://aov"));
        let obj = HdStDynamicUvTextureObject::new(id);

        assert!(!obj.is_created());
        // Dynamic textures are always valid
        assert!(obj.is_valid());
        assert_eq!(obj.texture_type(), TextureType::Uv);
    }

    #[test]
    fn test_create_and_destroy() {
        let id = HdStTextureIdentifier::from_path(AssetPath::new("dynamic://test"));
        let mut obj = HdStDynamicUvTextureObject::new(id);

        let mut desc = HgiTextureDesc::new();
        desc.dimensions = Vec3i::new(256, 256, 1);
        desc.format = HgiFormat::UNorm8Vec4;

        obj.create_texture(&desc);
        assert!(obj.is_created());
        assert_eq!(obj.dimensions(), Vec3i::new(256, 256, 1));

        obj.destroy_texture();
        assert!(!obj.is_created());
    }

    #[test]
    fn test_cpu_data_lifecycle() {
        let id = HdStTextureIdentifier::from_path(AssetPath::new("dynamic://proc"));
        let mut obj = HdStDynamicUvTextureObject::new(id);

        let data = HdStTextureCpuData::new_2d(vec![0u8; 64], 4, 4, HgiFormat::UNorm8Vec4, false);

        obj.set_cpu_data(data);
        assert!(obj.cpu_data().is_some());
        assert_eq!(obj.dimensions(), Vec3i::new(4, 4, 1));

        let taken = obj.take_cpu_data();
        assert!(taken.is_some());
        assert!(obj.cpu_data().is_none());
    }

    #[test]
    fn test_wrap_params() {
        let id = HdStTextureIdentifier::from_path(AssetPath::new("dynamic://wrap"));
        let mut obj = HdStDynamicUvTextureObject::new(id);

        obj.set_wrap_params(HdWrap::Repeat, HdWrap::Clamp);
        assert_eq!(obj.wrap_params(), &(HdWrap::Repeat, HdWrap::Clamp));
    }
}
