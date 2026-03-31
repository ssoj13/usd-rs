#![allow(dead_code)]

//! HdSt_FieldTextureCpuData - CPU-side 3D field texture loading.
//!
//! Implements texture CPU data by reading a 3D volume field from a
//! file (e.g. OpenVDB). Handles format conversion and optional alpha
//! premultiplication.
//!
//! Port of pxr/imaging/hdSt/fieldTextureCpuData.h

use super::texture_cpu_data::HdStTextureCpuData;
use usd_gf::Vec3i;
use usd_hgi::{HgiFormat, HgiTextureDesc, HgiTextureType, HgiTextureUsage};

/// CPU-side 3D field texture data.
///
/// Loaded from volume field files (OpenVDB, Field3D, etc.).
/// Converts field data to a 3D HGI texture format suitable for
/// GPU upload and volume rendering.
///
/// Port of HdSt_FieldTextureCpuData
#[derive(Debug, Clone)]
pub struct HdStFieldTextureCpuData {
    /// HGI texture descriptor for 3D texture creation
    texture_desc: HgiTextureDesc,
    /// Whether GPU should generate mipmaps
    generate_mipmaps: bool,
    /// Converted pixel data buffer (if format conversion was needed)
    converted_data: Option<Vec<u8>>,
    /// Original field data reference (raw bytes)
    raw_data: Vec<u8>,
    /// Debug name for diagnostics
    debug_name: String,
    /// Data validity flag
    valid: bool,
}

impl HdStFieldTextureCpuData {
    /// Create from raw field data.
    ///
    /// # Arguments
    /// * `data` - Raw voxel data bytes
    /// * `dimensions` - 3D grid dimensions (width, height, depth)
    /// * `format` - Voxel data format
    /// * `debug_name` - Name for diagnostics
    /// * `premultiply_alpha` - Whether to premultiply RGB by alpha
    pub fn new(
        data: Vec<u8>,
        dimensions: Vec3i,
        format: HgiFormat,
        debug_name: String,
        premultiply_alpha: bool,
    ) -> Self {
        let mut texture_desc = HgiTextureDesc::new();
        texture_desc.dimensions = dimensions;
        texture_desc.format = format;
        texture_desc.texture_type = HgiTextureType::Texture3D;
        texture_desc.mip_levels = 1;
        texture_desc.usage = HgiTextureUsage::SHADER_READ;
        texture_desc.debug_name = debug_name.clone();

        let valid = !data.is_empty()
            && dimensions[0] > 0
            && dimensions[1] > 0
            && dimensions[2] > 0;

        // Apply premultiply alpha if needed (for RGBA formats)
        let converted_data = if premultiply_alpha && Self::is_rgba_format(format) {
            Some(Self::premultiply(data.clone(), format))
        } else {
            None
        };

        Self {
            texture_desc,
            generate_mipmaps: false,
            converted_data,
            raw_data: data,
            debug_name,
            valid,
        }
    }

    /// Create an invalid (empty) placeholder.
    pub fn invalid(debug_name: String) -> Self {
        Self {
            texture_desc: HgiTextureDesc::new(),
            generate_mipmaps: false,
            converted_data: None,
            raw_data: Vec::new(),
            debug_name,
            valid: false,
        }
    }

    /// Get the texture descriptor.
    pub fn texture_desc(&self) -> &HgiTextureDesc {
        &self.texture_desc
    }

    /// Whether GPU should generate mipmaps.
    pub fn generate_mipmaps(&self) -> bool {
        self.generate_mipmaps
    }

    /// Whether data is valid.
    pub fn is_valid(&self) -> bool {
        self.valid
    }

    /// Get the active data buffer (converted or original).
    pub fn data(&self) -> &[u8] {
        self.converted_data.as_deref().unwrap_or(&self.raw_data)
    }

    /// Get dimensions.
    pub fn dimensions(&self) -> Vec3i {
        self.texture_desc.dimensions
    }

    /// Get format.
    pub fn format(&self) -> HgiFormat {
        self.texture_desc.format
    }

    /// Get debug name.
    pub fn debug_name(&self) -> &str {
        &self.debug_name
    }

    /// Convert to a general HdStTextureCpuData.
    pub fn to_cpu_data(&self) -> HdStTextureCpuData {
        if self.valid {
            HdStTextureCpuData::new(
                self.data().to_vec(),
                self.texture_desc.dimensions,
                self.texture_desc.format,
                HgiTextureType::Texture3D,
                1,
                false,
            )
        } else {
            HdStTextureCpuData::invalid()
        }
    }

    /// Check if format has an alpha channel.
    fn is_rgba_format(format: HgiFormat) -> bool {
        matches!(
            format,
            HgiFormat::UNorm8Vec4
                | HgiFormat::SNorm8Vec4
                | HgiFormat::Float16Vec4
                | HgiFormat::Float32Vec4
        )
    }

    /// Premultiply RGB by alpha in-place.
    fn premultiply(mut data: Vec<u8>, format: HgiFormat) -> Vec<u8> {
        match format {
            HgiFormat::UNorm8Vec4 => {
                for chunk in data.chunks_exact_mut(4) {
                    let a = chunk[3] as f32 / 255.0;
                    chunk[0] = (chunk[0] as f32 * a) as u8;
                    chunk[1] = (chunk[1] as f32 * a) as u8;
                    chunk[2] = (chunk[2] as f32 * a) as u8;
                }
            }
            // Other formats: no-op for now
            _ => {}
        }
        data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_cpu_data_creation() {
        // 4x4x4 single-channel float32 = 256 bytes
        let data = vec![0u8; 256];
        let cpu = HdStFieldTextureCpuData::new(
            data,
            Vec3i::new(4, 4, 4),
            HgiFormat::Float32,
            "density".to_string(),
            false,
        );

        assert!(cpu.is_valid());
        assert_eq!(cpu.dimensions(), Vec3i::new(4, 4, 4));
        assert_eq!(cpu.format(), HgiFormat::Float32);
        assert_eq!(cpu.debug_name(), "density");
        assert!(!cpu.generate_mipmaps());
    }

    #[test]
    fn test_field_invalid() {
        let cpu = HdStFieldTextureCpuData::invalid("missing.vdb".to_string());
        assert!(!cpu.is_valid());
    }

    #[test]
    fn test_field_premultiply() {
        // 1x1x1 RGBA8 = 4 bytes
        let data = vec![200u8, 100, 50, 128]; // ~50% alpha
        let cpu = HdStFieldTextureCpuData::new(
            data,
            Vec3i::new(1, 1, 1),
            HgiFormat::UNorm8Vec4,
            "test".to_string(),
            true,
        );

        assert!(cpu.is_valid());
        let result = cpu.data();
        // Alpha should be preserved, RGB should be premultiplied
        assert_eq!(result[3], 128);
        // 200 * (128/255) ~= 100
        assert!(result[0] < 200);
    }

    #[test]
    fn test_to_cpu_data() {
        let data = vec![1u8; 64]; // 4x4x1 single-channel
        let field = HdStFieldTextureCpuData::new(
            data,
            Vec3i::new(4, 4, 1),
            HgiFormat::UNorm8,
            "test_field".to_string(),
            false,
        );

        let cpu = field.to_cpu_data();
        assert!(cpu.is_valid());
        assert_eq!(cpu.dimensions(), Vec3i::new(4, 4, 1));
    }
}
