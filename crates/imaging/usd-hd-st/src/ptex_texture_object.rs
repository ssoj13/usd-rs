#![allow(dead_code)]

//! HdStPtexTextureObject - Extended Ptex texture loading with CPU data.
//!
//! Provides full Ptex texture loading with CPU-side data management,
//! load/commit lifecycle, and texel+layout GPU resource allocation.
//! Extends the basic HdStPtexTextureObject in texture_object.rs with
//! actual load/commit implementation details.
//!
//! Ptex textures have two GPU resources:
//! - **Texels**: Face textures packed into a 2D array texture
//! - **Layout**: Per-face adjacency and resolution metadata
//!
//! Port of pxr/imaging/hdSt/ptexTextureObject.h

use super::texture_identifier::HdStTextureIdentifier;
use super::texture_object::{HdStTextureObjectTrait, TextureType};
use usd_gf::{Vec2i, Vec3i};
use usd_hgi::{HgiFormat, HgiTextureHandle};

/// Extended Ptex texture object with CPU data and load/commit phases.
///
/// Manages the full lifecycle of a Ptex texture from file loading
/// through GPU upload. CPU data is held between load and commit phases.
///
/// Port of HdStPtexTextureObject (full implementation)
#[derive(Debug, Clone)]
pub struct HdStPtexTextureObjectFull {
    /// Texture identifier
    identifier: HdStTextureIdentifier,
    /// Pixel format of texel data
    format: HgiFormat,
    /// Texel texture dimensions (width, height, layers)
    texel_dimensions: Vec3i,
    /// Number of layers in the texel array texture
    texel_layers: i32,
    /// Raw texel data size in bytes
    texel_data_size: usize,
    /// Layout texture dimensions (width, height)
    layout_dimensions: Vec2i,
    /// Raw layout data size in bytes
    layout_data_size: usize,
    /// CPU texel data (between load and commit)
    texel_data: Option<Vec<u8>>,
    /// CPU layout data (between load and commit)
    layout_data: Option<Vec<u8>>,
    /// GPU texel texture handle
    texel_texture: HgiTextureHandle,
    /// GPU layout texture handle
    layout_texture: HgiTextureHandle,
    /// Total GPU memory committed
    byte_size: usize,
    /// Target memory budget
    target_memory: usize,
    /// Validity flag
    valid: bool,
}

impl HdStPtexTextureObjectFull {
    /// Create a new Ptex texture object.
    pub fn new(identifier: HdStTextureIdentifier) -> Self {
        Self {
            identifier,
            format: HgiFormat::Invalid,
            texel_dimensions: Vec3i::new(0, 0, 0),
            texel_layers: 0,
            texel_data_size: 0,
            layout_dimensions: Vec2i::new(0, 0),
            layout_data_size: 0,
            texel_data: None,
            layout_data: None,
            texel_texture: HgiTextureHandle::default(),
            layout_texture: HgiTextureHandle::default(),
            byte_size: 0,
            target_memory: 0,
            valid: false,
        }
    }

    /// Get the GPU texel texture handle (valid after commit).
    pub fn texel_texture(&self) -> &HgiTextureHandle {
        &self.texel_texture
    }

    /// Get the GPU layout texture handle (valid after commit).
    pub fn layout_texture(&self) -> &HgiTextureHandle {
        &self.layout_texture
    }

    /// Get texel dimensions.
    pub fn texel_dimensions(&self) -> Vec3i {
        self.texel_dimensions
    }

    /// Get number of texel layers.
    pub fn texel_layers(&self) -> i32 {
        self.texel_layers
    }

    /// Get pixel format.
    pub fn format(&self) -> HgiFormat {
        self.format
    }

    /// Get layout dimensions.
    pub fn layout_dimensions(&self) -> Vec2i {
        self.layout_dimensions
    }

    /// Set CPU texel data (from load phase).
    ///
    /// Stores raw texel bytes for later GPU upload during commit.
    pub fn set_texel_data(
        &mut self,
        data: Vec<u8>,
        dimensions: Vec3i,
        layers: i32,
        format: HgiFormat,
    ) {
        self.texel_data_size = data.len();
        self.texel_data = Some(data);
        self.texel_dimensions = dimensions;
        self.texel_layers = layers;
        self.format = format;
    }

    /// Set CPU layout data (from load phase).
    pub fn set_layout_data(&mut self, data: Vec<u8>, dimensions: Vec2i) {
        self.layout_data_size = data.len();
        self.layout_data = Some(data);
        self.layout_dimensions = dimensions;
    }

    /// Set GPU textures after commit.
    pub fn set_gpu_textures(
        &mut self,
        texels: HgiTextureHandle,
        layout: HgiTextureHandle,
        byte_size: usize,
    ) {
        self.texel_texture = texels;
        self.layout_texture = layout;
        self.byte_size = byte_size;
        self.valid = true;
        // Free CPU data after GPU upload
        self.texel_data = None;
        self.layout_data = None;
    }

    /// Destroy GPU textures and free resources.
    pub fn destroy_textures(&mut self) {
        self.texel_texture = HgiTextureHandle::default();
        self.layout_texture = HgiTextureHandle::default();
        self.byte_size = 0;
        self.valid = false;
    }

    /// Whether CPU data is loaded and ready for commit.
    pub fn has_cpu_data(&self) -> bool {
        self.texel_data.is_some() && self.layout_data.is_some()
    }

    /// Get CPU texel data bytes (if loaded).
    pub fn texel_data(&self) -> Option<&[u8]> {
        self.texel_data.as_deref()
    }

    /// Get CPU layout data bytes (if loaded).
    pub fn layout_data(&self) -> Option<&[u8]> {
        self.layout_data.as_deref()
    }
}

impl HdStTextureObjectTrait for HdStPtexTextureObjectFull {
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
        &self.texel_texture
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_sdf::AssetPath;

    #[test]
    fn test_ptex_creation() {
        let id = HdStTextureIdentifier::from_path(AssetPath::new("model.ptex"));
        let obj = HdStPtexTextureObjectFull::new(id);

        assert_eq!(obj.texture_type(), TextureType::Ptex);
        assert!(!obj.is_valid());
        assert!(!obj.has_cpu_data());
        assert_eq!(obj.committed_size(), 0);
    }

    #[test]
    fn test_ptex_cpu_data_lifecycle() {
        let id = HdStTextureIdentifier::from_path(AssetPath::new("model.ptex"));
        let mut obj = HdStPtexTextureObjectFull::new(id);

        // Load phase: set CPU data
        obj.set_texel_data(
            vec![0u8; 4096],
            Vec3i::new(64, 64, 1),
            4,
            HgiFormat::UNorm8Vec4,
        );
        obj.set_layout_data(vec![0u8; 256], Vec2i::new(16, 16));

        assert!(obj.has_cpu_data());
        assert_eq!(obj.texel_dimensions(), Vec3i::new(64, 64, 1));
        assert_eq!(obj.texel_layers(), 4);
        assert_eq!(obj.format(), HgiFormat::UNorm8Vec4);
        assert_eq!(obj.layout_dimensions(), Vec2i::new(16, 16));

        // Commit phase: set GPU handles
        obj.set_gpu_textures(
            HgiTextureHandle::default(),
            HgiTextureHandle::default(),
            4352,
        );

        assert!(obj.is_valid());
        assert_eq!(obj.committed_size(), 4352);
        // CPU data freed after commit
        assert!(!obj.has_cpu_data());
    }

    #[test]
    fn test_ptex_destroy() {
        let id = HdStTextureIdentifier::from_path(AssetPath::new("model.ptex"));
        let mut obj = HdStPtexTextureObjectFull::new(id);

        obj.set_gpu_textures(
            HgiTextureHandle::default(),
            HgiTextureHandle::default(),
            1024,
        );
        assert!(obj.is_valid());

        obj.destroy_textures();
        assert!(!obj.is_valid());
        assert_eq!(obj.committed_size(), 0);
    }
}
