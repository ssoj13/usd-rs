#![allow(dead_code)]

//! HdFormat <-> HioFormat conversions for Storm.
//!
//! Provides bidirectional conversion between Hydra render buffer formats
//! (HdFormat) and Hio image I/O formats (HioFormat) used when loading
//! texture data through the Hio image subsystem.
//!
//! Port of pxr/imaging/hdSt/hioConversions.h

use super::render_buffer::HdFormat;

/// Hio (Hydra Image I/O) pixel format.
///
/// Represents pixel formats used by the Hio image loading subsystem.
/// Subset of formats relevant to Storm texture loading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HioFormat {
    /// 8-bit unsigned normalized, 1 channel
    UNorm8,
    /// 8-bit unsigned normalized, 2 channels
    UNorm8Vec2,
    /// 8-bit unsigned normalized, 3 channels
    UNorm8Vec3,
    /// 8-bit unsigned normalized, 4 channels
    UNorm8Vec4,
    /// 16-bit float, 1 channel
    Float16,
    /// 16-bit float, 2 channels
    Float16Vec2,
    /// 16-bit float, 3 channels
    Float16Vec3,
    /// 16-bit float, 4 channels
    Float16Vec4,
    /// 32-bit float, 1 channel
    Float32,
    /// 32-bit float, 2 channels
    Float32Vec2,
    /// 32-bit float, 3 channels
    Float32Vec3,
    /// 32-bit float, 4 channels
    Float32Vec4,
    /// 32-bit integer, 1 channel
    Int32,
    /// Invalid / unknown format
    Invalid,
}

impl Default for HioFormat {
    fn default() -> Self {
        Self::Invalid
    }
}

/// Convert HdFormat to the corresponding HioFormat.
pub fn hd_to_hio(hd_format: HdFormat) -> HioFormat {
    match hd_format {
        HdFormat::UNorm8Vec4 => HioFormat::UNorm8Vec4,
        HdFormat::UNorm8Vec3 => HioFormat::UNorm8Vec3,
        HdFormat::Float16Vec4 => HioFormat::Float16Vec4,
        HdFormat::Float32Vec4 => HioFormat::Float32Vec4,
        HdFormat::Float32 => HioFormat::Float32,
        HdFormat::Int32 => HioFormat::Int32,
    }
}

/// Convert HioFormat to the corresponding HdFormat.
///
/// Returns `None` if the HioFormat has no direct HdFormat equivalent
/// (e.g., 1- or 2-channel formats that HdFormat doesn't represent).
pub fn hio_to_hd(hio_format: HioFormat) -> Option<HdFormat> {
    match hio_format {
        HioFormat::UNorm8Vec4 => Some(HdFormat::UNorm8Vec4),
        HioFormat::UNorm8Vec3 => Some(HdFormat::UNorm8Vec3),
        HioFormat::Float16Vec4 => Some(HdFormat::Float16Vec4),
        HioFormat::Float32Vec4 => Some(HdFormat::Float32Vec4),
        HioFormat::Float32 => Some(HdFormat::Float32),
        HioFormat::Int32 => Some(HdFormat::Int32),
        // Formats without direct HdFormat mapping
        _ => None,
    }
}

/// Convenience struct matching the C++ static-method class pattern.
///
/// Port of HdStHioConversions
pub struct HioConversions;

impl HioConversions {
    /// Get HioFormat from HdFormat.
    pub fn get_hio_format(hd_format: HdFormat) -> HioFormat {
        hd_to_hio(hd_format)
    }

    /// Get HdFormat from HioFormat. Returns None if no mapping exists.
    pub fn get_hd_format(hio_format: HioFormat) -> Option<HdFormat> {
        hio_to_hd(hio_format)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let formats = [
            HdFormat::UNorm8Vec4,
            HdFormat::UNorm8Vec3,
            HdFormat::Float16Vec4,
            HdFormat::Float32Vec4,
            HdFormat::Float32,
            HdFormat::Int32,
        ];
        for fmt in formats {
            let hio = hd_to_hio(fmt);
            let back = hio_to_hd(hio);
            assert_eq!(back, Some(fmt), "roundtrip failed for {:?}", fmt);
        }
    }

    #[test]
    fn test_no_mapping() {
        // Formats that don't map back
        assert_eq!(hio_to_hd(HioFormat::UNorm8), None);
        assert_eq!(hio_to_hd(HioFormat::Float32Vec2), None);
        assert_eq!(hio_to_hd(HioFormat::Invalid), None);
    }

    #[test]
    fn test_class_style_api() {
        let hio = HioConversions::get_hio_format(HdFormat::Float32);
        assert_eq!(hio, HioFormat::Float32);
        assert_eq!(HioConversions::get_hd_format(hio), Some(HdFormat::Float32));
    }
}
