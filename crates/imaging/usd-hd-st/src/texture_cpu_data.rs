#![allow(dead_code)]

//! HdStTextureCpuData - CPU-side texture data for GPU upload.
//!
//! Holds pixel data, format, dimensions, and mip levels on the CPU side.
//! Used during the texture load phase before committing to GPU via HGI.
//!
//! Port of pxr/imaging/hdSt/textureCpuData.h

use usd_gf::Vec3i;
use usd_hgi::{HgiFormat, HgiTextureDesc, HgiTextureType, HgiTextureUsage};

/// CPU-side texture data ready for GPU upload.
///
/// Contains all metadata and pixel data needed to create an HGI texture.
/// The texture descriptor includes a pointer to the CPU data as `initial_data`.
///
/// # Lifecycle
/// 1. Created during texture _Load() phase (thread-safe)
/// 2. Consumed during _Commit() phase to create GPU texture
/// 3. Dropped after commit (CPU memory freed)
///
/// Port of HdStTextureCpuData from pxr/imaging/hdSt/textureCpuData.h
#[derive(Clone)]
pub struct HdStTextureCpuData {
    /// HGI texture descriptor (dimensions, format, usage, etc.)
    texture_desc: HgiTextureDesc,

    /// CPU pixel data (owned). The descriptor's initial_data points here.
    pixel_data: Vec<u8>,

    /// Whether GPU should generate mipmaps from mip level 0
    generate_mipmaps: bool,

    /// Data validity flag (false if file not found, decode failed, etc.)
    valid: bool,
}

impl HdStTextureCpuData {
    /// Create new CPU texture data from raw pixels.
    ///
    /// # Arguments
    /// * `pixel_data` - Raw pixel bytes
    /// * `dimensions` - Width, height, depth
    /// * `format` - Pixel format
    /// * `texture_type` - 2D, 3D, Cube, etc.
    /// * `mip_levels` - Number of mip levels in pixel_data (1 = base only)
    /// * `generate_mipmaps` - Ask GPU to generate remaining mips
    pub fn new(
        pixel_data: Vec<u8>,
        dimensions: Vec3i,
        format: HgiFormat,
        texture_type: HgiTextureType,
        mip_levels: u16,
        generate_mipmaps: bool,
    ) -> Self {
        let mut desc = HgiTextureDesc::new();
        desc.dimensions = dimensions;
        desc.format = format;
        desc.texture_type = texture_type;
        desc.mip_levels = mip_levels;
        desc.usage = HgiTextureUsage::SHADER_READ;

        Self {
            texture_desc: desc,
            pixel_data,
            generate_mipmaps,
            valid: true,
        }
    }

    /// Create a 2D texture CPU data (most common).
    pub fn new_2d(
        pixel_data: Vec<u8>,
        width: i32,
        height: i32,
        format: HgiFormat,
        generate_mipmaps: bool,
    ) -> Self {
        let mip_levels = if generate_mipmaps { 1 } else { 1 };
        Self::new(
            pixel_data,
            Vec3i::new(width, height, 1),
            format,
            HgiTextureType::Texture2D,
            mip_levels,
            generate_mipmaps,
        )
    }

    /// Create a 3D volume texture CPU data.
    pub fn new_3d(
        pixel_data: Vec<u8>,
        width: i32,
        height: i32,
        depth: i32,
        format: HgiFormat,
    ) -> Self {
        Self::new(
            pixel_data,
            Vec3i::new(width, height, depth),
            format,
            HgiTextureType::Texture3D,
            1,
            false,
        )
    }

    /// Create an invalid (empty) CPU data placeholder.
    pub fn invalid() -> Self {
        Self {
            texture_desc: HgiTextureDesc::new(),
            pixel_data: Vec::new(),
            generate_mipmaps: false,
            valid: false,
        }
    }

    /// Get the HGI texture descriptor for GPU resource creation.
    pub fn texture_desc(&self) -> &HgiTextureDesc {
        &self.texture_desc
    }

    /// Get mutable texture descriptor.
    pub fn texture_desc_mut(&mut self) -> &mut HgiTextureDesc {
        &mut self.texture_desc
    }

    /// Get the raw pixel data bytes.
    pub fn pixel_data(&self) -> &[u8] {
        &self.pixel_data
    }

    /// Take ownership of pixel data (consuming).
    pub fn take_pixel_data(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.pixel_data)
    }

    /// Whether the GPU should generate mipmaps from level 0.
    pub fn generate_mipmaps(&self) -> bool {
        self.generate_mipmaps
    }

    /// Whether data is valid (file loaded, decoded successfully).
    pub fn is_valid(&self) -> bool {
        self.valid && !self.pixel_data.is_empty()
    }

    /// Get texture dimensions.
    pub fn dimensions(&self) -> Vec3i {
        self.texture_desc.dimensions
    }

    /// Get pixel format.
    pub fn format(&self) -> HgiFormat {
        self.texture_desc.format
    }

    /// Get total byte size of pixel data.
    pub fn byte_size(&self) -> usize {
        self.pixel_data.len()
    }
}

impl std::fmt::Debug for HdStTextureCpuData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HdStTextureCpuData")
            .field("dimensions", &self.texture_desc.dimensions)
            .field("format", &self.texture_desc.format)
            .field("byte_size", &self.pixel_data.len())
            .field("generate_mipmaps", &self.generate_mipmaps)
            .field("valid", &self.valid)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_2d_texture_data() {
        // 4x4 RGBA8 texture = 64 bytes
        let pixels = vec![0u8; 64];
        let data = HdStTextureCpuData::new_2d(pixels, 4, 4, HgiFormat::UNorm8Vec4, false);

        assert!(data.is_valid());
        assert_eq!(data.dimensions(), Vec3i::new(4, 4, 1));
        assert_eq!(data.format(), HgiFormat::UNorm8Vec4);
        assert_eq!(data.byte_size(), 64);
        assert!(!data.generate_mipmaps());
    }

    #[test]
    fn test_3d_texture_data() {
        let pixels = vec![0u8; 512]; // 8x8x8 single-channel
        let data = HdStTextureCpuData::new_3d(pixels, 8, 8, 8, HgiFormat::UNorm8);

        assert!(data.is_valid());
        assert_eq!(data.dimensions(), Vec3i::new(8, 8, 8));
    }

    #[test]
    fn test_invalid_data() {
        let data = HdStTextureCpuData::invalid();
        assert!(!data.is_valid());
    }

    #[test]
    fn test_take_pixel_data() {
        let pixels = vec![42u8; 16];
        let mut data = HdStTextureCpuData::new_2d(pixels, 2, 2, HgiFormat::UNorm8Vec4, false);

        let taken = data.take_pixel_data();
        assert_eq!(taken.len(), 16);
        assert_eq!(taken[0], 42);
        assert!(data.pixel_data().is_empty());
    }

    #[test]
    fn test_mipmap_generation() {
        let pixels = vec![0u8; 256];
        let data = HdStTextureCpuData::new_2d(pixels, 8, 8, HgiFormat::UNorm8Vec4, true);

        assert!(data.generate_mipmaps());
    }
}
