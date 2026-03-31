#![allow(dead_code)]

//! HdStTextureObject - GPU texture object hierarchy for Storm.
//!
//! Trait-based texture object system matching C++ HdStTextureObject hierarchy.
//! Manages HGI texture handle lifecycle with load/commit phases.
//!
//! Texture types:
//! - Uv: Standard 2D textures (most common)
//! - Field: 3D volume textures with bounding box
//! - Ptex: Ptex subdivision textures (texels + layout)
//! - Udim: UDIM tile sets (texels + layout)
//! - Cubemap: Cubemap environment textures
//!
//! Port of pxr/imaging/hdSt/textureObject.h

use super::texture_cpu_data::HdStTextureCpuData;
use super::texture_identifier::HdStTextureIdentifier;
use std::sync::Arc;
use usd_gf::{BBox3d, Matrix4d, Vec3i};
use usd_hd::enums::HdWrap;
use usd_hgi::{HgiDriverHandle, HgiFormat, HgiSampleCount, HgiTextureHandle, HgiTextureType};
use usd_tf::Token;

/// Storm texture type enum (mirrors HdStTextureType in C++).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextureType {
    /// Standard 2D UV texture
    Uv,
    /// 3D volume/field texture
    Field,
    /// Ptex subdivision texture
    Ptex,
    /// UDIM tiled texture
    Udim,
    /// Cubemap environment texture
    Cubemap,
}

impl TextureType {
    /// Convert to HGI texture type.
    pub fn to_hgi_type(&self) -> HgiTextureType {
        match self {
            TextureType::Uv => HgiTextureType::Texture2D,
            TextureType::Field => HgiTextureType::Texture3D,
            TextureType::Ptex => HgiTextureType::Texture2DArray,
            TextureType::Udim => HgiTextureType::Texture2DArray,
            TextureType::Cubemap => HgiTextureType::Cubemap,
        }
    }
}

impl Default for TextureType {
    fn default() -> Self {
        TextureType::Uv
    }
}

/// Base trait for all Storm texture objects.
///
/// Manages the load/commit lifecycle:
/// 1. `_load()` - Read texture data to CPU (thread-safe)
/// 2. `_commit()` - Upload CPU data to GPU via HGI (main thread only)
///
/// Port of HdStTextureObject from pxr/imaging/hdSt/textureObject.h
pub trait HdStTextureObjectTrait: std::fmt::Debug + Send + Sync {
    /// Get the texture identifier.
    fn identifier(&self) -> &HdStTextureIdentifier;

    /// Get the texture type.
    fn texture_type(&self) -> TextureType;

    /// Get target memory budget in bytes (0 = full resolution).
    fn target_memory(&self) -> usize;

    /// Set target memory. Texture will be downsampled on upload to fit.
    fn set_target_memory(&mut self, bytes: usize);

    /// Get GPU memory actually committed (valid after commit).
    fn committed_size(&self) -> usize;

    /// Is texture valid? Only correct after commit phase.
    fn is_valid(&self) -> bool;

    /// Get the primary HGI texture handle.
    fn texture_handle(&self) -> &HgiTextureHandle;
}

// ---------------------------------------------------------------------------
// HdStUvTextureObject - Standard 2D UV textures
// ---------------------------------------------------------------------------

/// UV (2D) texture object. Most common texture type.
///
/// Holds a single HGI texture with optional wrap parameter opinions
/// from the texture file itself (e.g. EXR metadata).
///
/// Port of HdStUvTextureObject / HdStAssetUvTextureObject
#[derive(Debug, Clone)]
pub struct HdStUvTextureObject {
    /// Texture identifier
    identifier: HdStTextureIdentifier,
    /// HGI texture handle
    gpu_texture: HgiTextureHandle,
    /// Texture dimensions
    dimensions: Vec3i,
    /// Pixel format
    format: HgiFormat,
    /// Mipmap levels
    mip_levels: u16,
    /// Samples per pixel (MSAA)
    sample_count: HgiSampleCount,
    /// GPU memory committed
    byte_size: usize,
    /// Target memory budget (0 = full resolution)
    target_memory: usize,
    /// Wrap mode opinions from the texture file
    wrap_params: (HdWrap, HdWrap),
    /// CPU data (between load and commit phases)
    cpu_data: Option<HdStTextureCpuData>,
    /// Validity flag
    valid: bool,
    /// Debug label
    debug_name: String,
}

impl HdStUvTextureObject {
    /// Create a new UV texture object.
    pub fn new(identifier: HdStTextureIdentifier) -> Self {
        Self {
            identifier,
            gpu_texture: HgiTextureHandle::default(),
            dimensions: Vec3i::new(0, 0, 0),
            format: HgiFormat::Invalid,
            mip_levels: 1,
            sample_count: HgiSampleCount::Count1,
            byte_size: 0,
            target_memory: 0,
            wrap_params: (HdWrap::NoOpinion, HdWrap::NoOpinion),
            cpu_data: None,
            valid: false,
            debug_name: String::new(),
        }
    }

    /// Get texture dimensions.
    pub fn dimensions(&self) -> Vec3i {
        self.dimensions
    }

    /// Get pixel format.
    pub fn format(&self) -> HgiFormat {
        self.format
    }

    /// Get mipmap levels.
    pub fn mip_levels(&self) -> u16 {
        self.mip_levels
    }

    /// Get wrap parameter opinions from the texture file.
    ///
    /// Returns `(wrapS, wrapT)`. Either may be `HdWrap::NoOpinion`.
    pub fn wrap_params(&self) -> &(HdWrap, HdWrap) {
        &self.wrap_params
    }

    /// Set wrap parameters (typically from texture file metadata).
    pub fn set_wrap_params(&mut self, wrap_s: HdWrap, wrap_t: HdWrap) {
        self.wrap_params = (wrap_s, wrap_t);
    }

    /// Set CPU data (from load phase).
    pub fn set_cpu_data(&mut self, cpu_data: HdStTextureCpuData) {
        self.dimensions = cpu_data.dimensions();
        self.format = cpu_data.format();
        self.valid = cpu_data.is_valid();
        self.cpu_data = Some(cpu_data);
    }

    /// Set GPU texture handle (from commit phase).
    pub fn set_gpu_texture(&mut self, handle: HgiTextureHandle, byte_size: usize) {
        self.gpu_texture = handle;
        self.byte_size = byte_size;
    }

    /// Set debug name.
    pub fn set_debug_name(&mut self, name: impl Into<String>) {
        self.debug_name = name.into();
    }

    /// Set dimensions directly.
    pub fn set_dimensions(&mut self, dims: Vec3i) {
        self.dimensions = dims;
    }

    /// Set format directly.
    pub fn set_format(&mut self, format: HgiFormat) {
        self.format = format;
    }

    /// Set mip levels.
    pub fn set_mip_levels(&mut self, levels: u16) {
        self.mip_levels = levels;
    }

    /// Commit CPU data to GPU via HGI, creating the actual GPU texture.
    ///
    /// Consumes `cpu_data` (drops CPU memory after upload).
    /// Mirrors C++ HdStAssetUvTextureObject::_Commit().
    ///
    /// # Arguments
    /// * `hgi` - HGI driver handle (from resource registry)
    pub fn commit_to_gpu(&mut self, hgi: &HgiDriverHandle) {
        let Some(cpu_data) = self.cpu_data.take() else {
            return; // Nothing to upload
        };

        if !cpu_data.is_valid() {
            log::warn!(
                "HdStUvTextureObject::commit_to_gpu: invalid CPU data for {}",
                self.debug_name
            );
            return;
        }

        let desc = cpu_data.texture_desc().clone();
        let pixel_data = cpu_data.pixel_data();

        // Calculate byte size of the GPU texture
        let byte_size = pixel_data.len();

        // Create GPU texture via HGI, uploading pixel data in one shot
        let handle = hgi.with_write(|h| h.create_texture(&desc, Some(pixel_data)));

        self.gpu_texture = handle;
        self.byte_size = byte_size;
        self.valid = true;
        log::debug!(
            "HdStUvTextureObject: committed {}x{}x{} {} texture ({} bytes)",
            desc.dimensions.x,
            desc.dimensions.y,
            desc.dimensions.z,
            self.debug_name,
            byte_size,
        );
    }
}

impl HdStTextureObjectTrait for HdStUvTextureObject {
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
    fn is_valid(&self) -> bool {
        self.valid && self.gpu_texture.is_valid()
    }
    fn texture_handle(&self) -> &HgiTextureHandle {
        &self.gpu_texture
    }
}

// ---------------------------------------------------------------------------
// HdStFieldTextureObject - 3D volume/field textures
// ---------------------------------------------------------------------------

/// Field (3D volume) texture object.
///
/// Wraps a 3D texture with bounding box and sampling transform for
/// volume rendering (e.g. OpenVDB grids).
///
/// Port of HdStFieldTextureObject
#[derive(Debug, Clone)]
pub struct HdStFieldTextureObject {
    identifier: HdStTextureIdentifier,
    gpu_texture: HgiTextureHandle,
    dimensions: Vec3i,
    format: HgiFormat,
    byte_size: usize,
    target_memory: usize,
    /// Bounding box the field fills in object space
    bbox: BBox3d,
    /// Transform from object space to texture [0,1]^3 space
    sampling_transform: Matrix4d,
    cpu_data: Option<HdStTextureCpuData>,
    valid: bool,
}

impl HdStFieldTextureObject {
    /// Create a new field texture object.
    pub fn new(identifier: HdStTextureIdentifier) -> Self {
        Self {
            identifier,
            gpu_texture: HgiTextureHandle::default(),
            dimensions: Vec3i::new(0, 0, 0),
            format: HgiFormat::Invalid,
            byte_size: 0,
            target_memory: 0,
            bbox: BBox3d::default(),
            sampling_transform: Matrix4d::identity(),
            cpu_data: None,
            valid: false,
        }
    }

    /// Get the bounding box. Valid after commit.
    pub fn bbox(&self) -> &BBox3d {
        &self.bbox
    }

    /// Get the sampling transform. Valid after commit.
    pub fn sampling_transform(&self) -> &Matrix4d {
        &self.sampling_transform
    }

    /// Set bounding box and sampling transform.
    pub fn set_volume_data(&mut self, bbox: BBox3d, sampling_transform: Matrix4d) {
        self.bbox = bbox;
        self.sampling_transform = sampling_transform;
    }

    /// Set CPU data.
    pub fn set_cpu_data(&mut self, cpu_data: HdStTextureCpuData) {
        self.dimensions = cpu_data.dimensions();
        self.format = cpu_data.format();
        self.valid = cpu_data.is_valid();
        self.cpu_data = Some(cpu_data);
    }

    /// Set GPU texture handle.
    pub fn set_gpu_texture(&mut self, handle: HgiTextureHandle, byte_size: usize) {
        self.gpu_texture = handle;
        self.byte_size = byte_size;
    }

    /// Get dimensions.
    pub fn dimensions(&self) -> Vec3i {
        self.dimensions
    }

    /// Commit 3D volume texture CPU data to GPU.
    pub fn commit_to_gpu(&mut self, hgi: &HgiDriverHandle) {
        let Some(cpu_data) = self.cpu_data.take() else {
            return;
        };
        if !cpu_data.is_valid() {
            return;
        }

        let desc = cpu_data.texture_desc().clone();
        let byte_size = cpu_data.pixel_data().len();

        let handle = hgi.with_write(|h| h.create_texture(&desc, Some(cpu_data.pixel_data())));
        self.gpu_texture = handle;
        self.byte_size = byte_size;
        self.valid = true;
    }
}

impl HdStTextureObjectTrait for HdStFieldTextureObject {
    fn identifier(&self) -> &HdStTextureIdentifier {
        &self.identifier
    }
    fn texture_type(&self) -> TextureType {
        TextureType::Field
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
    fn is_valid(&self) -> bool {
        self.valid && self.gpu_texture.is_valid()
    }
    fn texture_handle(&self) -> &HgiTextureHandle {
        &self.gpu_texture
    }
}

// ---------------------------------------------------------------------------
// HdStPtexTextureObject - Ptex subdivision textures
// ---------------------------------------------------------------------------

/// Ptex texture object.
///
/// Ptex textures have two GPU resources: texels (the actual pixel data)
/// and layout (per-face metadata for subdivision lookup).
///
/// Port of HdStPtexTextureObject
#[derive(Debug, Clone)]
pub struct HdStPtexTextureObject {
    identifier: HdStTextureIdentifier,
    /// Texel data (face textures packed into 2D array)
    texels_texture: HgiTextureHandle,
    /// Layout metadata (per-face adjacency and resolution)
    layout_texture: HgiTextureHandle,
    byte_size: usize,
    target_memory: usize,
    valid: bool,
}

impl HdStPtexTextureObject {
    /// Create a new Ptex texture object.
    pub fn new(identifier: HdStTextureIdentifier) -> Self {
        Self {
            identifier,
            texels_texture: HgiTextureHandle::default(),
            layout_texture: HgiTextureHandle::default(),
            byte_size: 0,
            target_memory: 0,
            valid: false,
        }
    }

    /// Get the texels texture handle.
    pub fn texels_texture(&self) -> &HgiTextureHandle {
        &self.texels_texture
    }

    /// Get the layout texture handle.
    pub fn layout_texture(&self) -> &HgiTextureHandle {
        &self.layout_texture
    }

    /// Set GPU textures.
    pub fn set_gpu_textures(
        &mut self,
        texels: HgiTextureHandle,
        layout: HgiTextureHandle,
        byte_size: usize,
    ) {
        self.texels_texture = texels;
        self.layout_texture = layout;
        self.byte_size = byte_size;
        self.valid = true;
    }
}

impl HdStTextureObjectTrait for HdStPtexTextureObject {
    fn identifier(&self) -> &HdStTextureIdentifier {
        &self.identifier
    }
    fn texture_type(&self) -> TextureType {
        TextureType::Ptex
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
    fn is_valid(&self) -> bool {
        self.valid
    }
    fn texture_handle(&self) -> &HgiTextureHandle {
        &self.texels_texture
    }
}

// ---------------------------------------------------------------------------
// HdStUdimTextureObject - UDIM tile sets
// ---------------------------------------------------------------------------

/// UDIM texture object.
///
/// UDIM textures pack multiple UV tiles into a single 2D array texture.
/// Has two GPU resources: texels (packed tile data) and layout (tile lookup).
///
/// Port of HdStUdimTextureObject
#[derive(Debug, Clone)]
pub struct HdStUdimTextureObject {
    identifier: HdStTextureIdentifier,
    /// Packed tile texels
    texels_texture: HgiTextureHandle,
    /// Tile layout lookup texture
    layout_texture: HgiTextureHandle,
    byte_size: usize,
    target_memory: usize,
    valid: bool,
}

impl HdStUdimTextureObject {
    /// Create a new UDIM texture object.
    pub fn new(identifier: HdStTextureIdentifier) -> Self {
        Self {
            identifier,
            texels_texture: HgiTextureHandle::default(),
            layout_texture: HgiTextureHandle::default(),
            byte_size: 0,
            target_memory: 0,
            valid: false,
        }
    }

    /// Get the texels texture handle.
    pub fn texels_texture(&self) -> &HgiTextureHandle {
        &self.texels_texture
    }

    /// Get the layout texture handle.
    pub fn layout_texture(&self) -> &HgiTextureHandle {
        &self.layout_texture
    }

    /// Set GPU textures.
    pub fn set_gpu_textures(
        &mut self,
        texels: HgiTextureHandle,
        layout: HgiTextureHandle,
        byte_size: usize,
    ) {
        self.texels_texture = texels;
        self.layout_texture = layout;
        self.byte_size = byte_size;
        self.valid = true;
    }
}

impl HdStTextureObjectTrait for HdStUdimTextureObject {
    fn identifier(&self) -> &HdStTextureIdentifier {
        &self.identifier
    }
    fn texture_type(&self) -> TextureType {
        TextureType::Udim
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
    fn is_valid(&self) -> bool {
        self.valid
    }
    fn texture_handle(&self) -> &HgiTextureHandle {
        &self.texels_texture
    }
}

// ---------------------------------------------------------------------------
// HdStCubemapTextureObject - Cubemap environment textures
// ---------------------------------------------------------------------------

/// Cubemap texture object for environment maps.
///
/// Port of HdStCubemapTextureObject
#[derive(Debug, Clone)]
pub struct HdStCubemapTextureObject {
    identifier: HdStTextureIdentifier,
    gpu_texture: HgiTextureHandle,
    byte_size: usize,
    target_memory: usize,
    cpu_data: Option<HdStTextureCpuData>,
    valid: bool,
}

impl HdStCubemapTextureObject {
    /// Create a new cubemap texture object.
    pub fn new(identifier: HdStTextureIdentifier) -> Self {
        Self {
            identifier,
            gpu_texture: HgiTextureHandle::default(),
            byte_size: 0,
            target_memory: 0,
            cpu_data: None,
            valid: false,
        }
    }

    /// Set CPU data.
    pub fn set_cpu_data(&mut self, cpu_data: HdStTextureCpuData) {
        self.valid = cpu_data.is_valid();
        self.cpu_data = Some(cpu_data);
    }

    /// Set GPU texture handle.
    pub fn set_gpu_texture(&mut self, handle: HgiTextureHandle, byte_size: usize) {
        self.gpu_texture = handle;
        self.byte_size = byte_size;
    }
}

impl HdStTextureObjectTrait for HdStCubemapTextureObject {
    fn identifier(&self) -> &HdStTextureIdentifier {
        &self.identifier
    }
    fn texture_type(&self) -> TextureType {
        TextureType::Cubemap
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
    fn is_valid(&self) -> bool {
        self.valid && self.gpu_texture.is_valid()
    }
    fn texture_handle(&self) -> &HgiTextureHandle {
        &self.gpu_texture
    }
}

// ---------------------------------------------------------------------------
// Convenience type: a concrete "any" texture object wrapping all variants
// ---------------------------------------------------------------------------

/// Concrete texture object enum wrapping all texture type variants.
///
/// Allows storing heterogeneous texture objects in a single collection.
#[derive(Debug, Clone)]
pub enum HdStTextureObject {
    Uv(HdStUvTextureObject),
    Field(HdStFieldTextureObject),
    Ptex(HdStPtexTextureObject),
    Udim(HdStUdimTextureObject),
    Cubemap(HdStCubemapTextureObject),
}

impl HdStTextureObject {
    /// Create a 2D UV texture (convenience).
    pub fn new_2d(identifier: HdStTextureIdentifier) -> Self {
        Self::Uv(HdStUvTextureObject::new(identifier))
    }

    /// Get the texture identifier.
    pub fn identifier(&self) -> &HdStTextureIdentifier {
        match self {
            Self::Uv(t) => t.identifier(),
            Self::Field(t) => t.identifier(),
            Self::Ptex(t) => t.identifier(),
            Self::Udim(t) => t.identifier(),
            Self::Cubemap(t) => t.identifier(),
        }
    }

    /// Get texture type.
    pub fn texture_type(&self) -> TextureType {
        match self {
            Self::Uv(_) => TextureType::Uv,
            Self::Field(_) => TextureType::Field,
            Self::Ptex(_) => TextureType::Ptex,
            Self::Udim(_) => TextureType::Udim,
            Self::Cubemap(_) => TextureType::Cubemap,
        }
    }

    /// Get the primary HGI texture handle.
    pub fn texture_handle(&self) -> &HgiTextureHandle {
        match self {
            Self::Uv(t) => t.texture_handle(),
            Self::Field(t) => t.texture_handle(),
            Self::Ptex(t) => t.texture_handle(),
            Self::Udim(t) => t.texture_handle(),
            Self::Cubemap(t) => t.texture_handle(),
        }
    }

    /// Is texture valid? Only correct after commit phase.
    pub fn is_valid(&self) -> bool {
        match self {
            Self::Uv(t) => t.is_valid(),
            Self::Field(t) => t.is_valid(),
            Self::Ptex(t) => t.is_valid(),
            Self::Udim(t) => t.is_valid(),
            Self::Cubemap(t) => t.is_valid(),
        }
    }

    /// Get committed GPU memory size.
    pub fn committed_size(&self) -> usize {
        match self {
            Self::Uv(t) => t.committed_size(),
            Self::Field(t) => t.committed_size(),
            Self::Ptex(t) => t.committed_size(),
            Self::Udim(t) => t.committed_size(),
            Self::Cubemap(t) => t.committed_size(),
        }
    }

    /// Try to get as UV texture.
    pub fn as_uv(&self) -> Option<&HdStUvTextureObject> {
        match self {
            Self::Uv(t) => Some(t),
            _ => None,
        }
    }

    /// Try to get as UV texture (mutable).
    pub fn as_uv_mut(&mut self) -> Option<&mut HdStUvTextureObject> {
        match self {
            Self::Uv(t) => Some(t),
            _ => None,
        }
    }

    /// Try to get as field texture.
    pub fn as_field(&self) -> Option<&HdStFieldTextureObject> {
        match self {
            Self::Field(t) => Some(t),
            _ => None,
        }
    }

    /// Try to get as field texture (mutable).
    pub fn as_field_mut(&mut self) -> Option<&mut HdStFieldTextureObject> {
        match self {
            Self::Field(t) => Some(t),
            _ => None,
        }
    }

    /// Try to get as Ptex texture.
    pub fn as_ptex(&self) -> Option<&HdStPtexTextureObject> {
        match self {
            Self::Ptex(t) => Some(t),
            _ => None,
        }
    }

    /// Try to get as UDIM texture.
    pub fn as_udim(&self) -> Option<&HdStUdimTextureObject> {
        match self {
            Self::Udim(t) => Some(t),
            _ => None,
        }
    }

    /// Try to get as cubemap texture.
    pub fn as_cubemap(&self) -> Option<&HdStCubemapTextureObject> {
        match self {
            Self::Cubemap(t) => Some(t),
            _ => None,
        }
    }

    /// Commit all loaded CPU data to GPU via HGI.
    ///
    /// Must be called on the main thread after `set_cpu_data()`.
    /// Drops CPU memory after successful upload.
    ///
    /// No-op for Ptex/Udim (they manage their own GPU resources).
    pub fn commit_to_gpu(&mut self, hgi: &HgiDriverHandle) {
        match self {
            Self::Uv(t) => t.commit_to_gpu(hgi),
            Self::Field(t) => t.commit_to_gpu(hgi),
            Self::Ptex(_) | Self::Udim(_) | Self::Cubemap(_) => {
                // Ptex/Udim/Cubemap have custom loaders — no-op here
            }
        }
    }
}

/// Shared pointer to texture object.
pub type HdStTextureObjectSharedPtr = Arc<HdStTextureObject>;

/// Named pair of texture object.
pub type HdStTextureObjectNamedPair = (Token, HdStTextureObjectSharedPtr);

/// Named list of texture objects.
pub type HdStTextureObjectNamedList = Vec<HdStTextureObjectNamedPair>;

#[cfg(test)]
mod tests {
    use super::*;
    use usd_sdf::AssetPath;

    #[test]
    fn test_texture_type_to_hgi() {
        assert_eq!(TextureType::Uv.to_hgi_type(), HgiTextureType::Texture2D);
        assert_eq!(TextureType::Field.to_hgi_type(), HgiTextureType::Texture3D);
        assert_eq!(TextureType::Cubemap.to_hgi_type(), HgiTextureType::Cubemap);
    }

    #[test]
    fn test_uv_texture_object() {
        let id = HdStTextureIdentifier::from_path(AssetPath::new("diffuse.png"));
        let tex = HdStUvTextureObject::new(id);

        assert_eq!(tex.texture_type(), TextureType::Uv);
        assert!(!tex.is_valid());
        assert_eq!(tex.committed_size(), 0);
        assert_eq!(tex.wrap_params(), &(HdWrap::NoOpinion, HdWrap::NoOpinion));
    }

    #[test]
    fn test_field_texture_object() {
        let id = HdStTextureIdentifier::from_path(AssetPath::new("density.vdb"));
        let tex = HdStFieldTextureObject::new(id);

        assert_eq!(tex.texture_type(), TextureType::Field);
        assert!(!tex.is_valid());
    }

    #[test]
    fn test_ptex_texture_object() {
        let id = HdStTextureIdentifier::from_path(AssetPath::new("model.ptx"));
        let mut tex = HdStPtexTextureObject::new(id);

        assert_eq!(tex.texture_type(), TextureType::Ptex);
        assert!(!tex.is_valid());

        tex.set_gpu_textures(
            HgiTextureHandle::default(),
            HgiTextureHandle::default(),
            4096,
        );
        assert_eq!(tex.committed_size(), 4096);
    }

    #[test]
    fn test_enum_variant_accessors() {
        let id = HdStTextureIdentifier::from_path(AssetPath::new("test.png"));
        let obj = HdStTextureObject::new_2d(id);

        assert_eq!(obj.texture_type(), TextureType::Uv);
        assert!(obj.as_uv().is_some());
        assert!(obj.as_field().is_none());
    }

    #[test]
    fn test_shared_ptr() {
        let id = HdStTextureIdentifier::from_path(AssetPath::new("shared.png"));
        let tex = HdStTextureObject::new_2d(id);
        let shared: HdStTextureObjectSharedPtr = Arc::new(tex);
        let clone = shared.clone();

        assert_eq!(Arc::strong_count(&shared), 2);
        assert_eq!(shared.texture_type(), clone.texture_type());
    }
}
