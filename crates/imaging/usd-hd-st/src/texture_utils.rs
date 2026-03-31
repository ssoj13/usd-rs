#![allow(dead_code)]

//! Texture utility functions for Storm.
//!
//! Helpers for format conversion, mip count calculation, texture type
//! detection, and memory size estimation. All through HGI abstraction.
//!
//! Port of pxr/imaging/hdSt/textureUtils.h

use super::hgi_conversions::HdFormat;
use super::texture_object::TextureType;
use usd_gf::Vec3i;
use usd_hgi::HgiFormat;

/// Calculate number of mipmap levels for given dimensions.
///
/// Returns floor(log2(max(w, h, d))) + 1.
pub fn calc_mip_levels(width: u32, height: u32, depth: u32) -> u16 {
    let max_dim = width.max(height).max(depth);
    if max_dim == 0 {
        return 0;
    }
    (32 - max_dim.leading_zeros()) as u16
}

/// Calculate mip levels from Vec3i dimensions.
pub fn calc_mip_levels_3i(dims: Vec3i) -> u16 {
    calc_mip_levels(
        dims[0].max(0) as u32,
        dims[1].max(0) as u32,
        dims[2].max(0) as u32,
    )
}

/// Get bytes per pixel for an HgiFormat.
///
/// Returns 0 for invalid formats. For compressed formats, returns
/// bytes per block (not per pixel).
pub fn format_byte_size(format: HgiFormat) -> usize {
    match format {
        // 1-component
        HgiFormat::UNorm8 | HgiFormat::SNorm8 => 1,
        HgiFormat::Float16 | HgiFormat::UInt16 | HgiFormat::Int16 => 2,
        HgiFormat::Float32 | HgiFormat::Int32 => 4,

        // 2-component
        HgiFormat::UNorm8Vec2 | HgiFormat::SNorm8Vec2 => 2,
        HgiFormat::Float16Vec2 | HgiFormat::UInt16Vec2 | HgiFormat::Int16Vec2 => 4,
        HgiFormat::Float32Vec2 | HgiFormat::Int32Vec2 => 8,

        // 3-component
        HgiFormat::Float16Vec3 | HgiFormat::UInt16Vec3 | HgiFormat::Int16Vec3 => 6,
        HgiFormat::Float32Vec3 | HgiFormat::Int32Vec3 => 12,

        // 4-component
        HgiFormat::UNorm8Vec4 | HgiFormat::SNorm8Vec4 | HgiFormat::UNorm8Vec4srgb => 4,
        HgiFormat::Float16Vec4 | HgiFormat::UInt16Vec4 | HgiFormat::Int16Vec4 => 8,
        HgiFormat::Float32Vec4 | HgiFormat::Int32Vec4 => 16,

        // Compressed (block size for 4x4 blocks)
        HgiFormat::BC1UNorm8Vec4 => 8,
        HgiFormat::BC3UNorm8Vec4 => 16,
        HgiFormat::BC6FloatVec3 | HgiFormat::BC6UFloatVec3 => 16,
        HgiFormat::BC7UNorm8Vec4 | HgiFormat::BC7UNorm8Vec4srgb => 16,

        // Depth/stencil
        HgiFormat::Float32UInt8 => 5,

        // Packed
        HgiFormat::PackedInt1010102 => 4,

        // 16-bit depth-only
        HgiFormat::PackedD16Unorm => 2,

        HgiFormat::Invalid => 0,
    }
}

/// Estimate total GPU memory for a texture including all mip levels.
pub fn calc_texture_memory(dims: Vec3i, format: HgiFormat, mip_levels: u16) -> usize {
    let bpp = format_byte_size(format);
    if bpp == 0 {
        return 0;
    }

    let compressed = is_compressed_format(format);
    let mut total = 0usize;
    let (mut w, mut h, mut d) = (
        dims[0].max(1) as usize,
        dims[1].max(1) as usize,
        dims[2].max(1) as usize,
    );

    for _ in 0..mip_levels.max(1) {
        if compressed {
            let bw = (w + 3) / 4;
            let bh = (h + 3) / 4;
            total += bw * bh * d * bpp;
        } else {
            total += w * h * d * bpp;
        }
        w = (w / 2).max(1);
        h = (h / 2).max(1);
        d = (d / 2).max(1);
    }

    total
}

/// Check if format is a compressed block format (BC1-BC7).
pub fn is_compressed_format(format: HgiFormat) -> bool {
    matches!(
        format,
        HgiFormat::BC1UNorm8Vec4
            | HgiFormat::BC3UNorm8Vec4
            | HgiFormat::BC6FloatVec3
            | HgiFormat::BC6UFloatVec3
            | HgiFormat::BC7UNorm8Vec4
            | HgiFormat::BC7UNorm8Vec4srgb
    )
}

/// Check if format has an alpha channel.
pub fn has_alpha(format: HgiFormat) -> bool {
    matches!(
        format,
        HgiFormat::UNorm8Vec4
            | HgiFormat::SNorm8Vec4
            | HgiFormat::Float16Vec4
            | HgiFormat::Float32Vec4
            | HgiFormat::Int32Vec4
            | HgiFormat::UNorm8Vec4srgb
            | HgiFormat::BC1UNorm8Vec4
            | HgiFormat::BC3UNorm8Vec4
            | HgiFormat::BC7UNorm8Vec4
            | HgiFormat::BC7UNorm8Vec4srgb
    )
}

/// Get number of components for an HgiFormat (1-4).
pub fn component_count(format: HgiFormat) -> u8 {
    match format {
        HgiFormat::UNorm8
        | HgiFormat::SNorm8
        | HgiFormat::Float16
        | HgiFormat::Float32
        | HgiFormat::Int32
        | HgiFormat::UInt16
        | HgiFormat::Int16 => 1,

        HgiFormat::UNorm8Vec2
        | HgiFormat::SNorm8Vec2
        | HgiFormat::Float16Vec2
        | HgiFormat::Float32Vec2
        | HgiFormat::Int32Vec2
        | HgiFormat::UInt16Vec2
        | HgiFormat::Int16Vec2 => 2,

        HgiFormat::Float16Vec3
        | HgiFormat::Float32Vec3
        | HgiFormat::Int32Vec3
        | HgiFormat::BC6FloatVec3
        | HgiFormat::BC6UFloatVec3
        | HgiFormat::UInt16Vec3
        | HgiFormat::Int16Vec3 => 3,

        HgiFormat::UNorm8Vec4
        | HgiFormat::SNorm8Vec4
        | HgiFormat::Float16Vec4
        | HgiFormat::Float32Vec4
        | HgiFormat::Int32Vec4
        | HgiFormat::UNorm8Vec4srgb
        | HgiFormat::BC1UNorm8Vec4
        | HgiFormat::BC3UNorm8Vec4
        | HgiFormat::BC7UNorm8Vec4
        | HgiFormat::BC7UNorm8Vec4srgb
        | HgiFormat::UInt16Vec4
        | HgiFormat::Int16Vec4
        | HgiFormat::PackedInt1010102 => 4,

        HgiFormat::Float32UInt8 => 2, // depth + stencil

        // 16-bit depth-only: single component
        HgiFormat::PackedD16Unorm => 1,

        HgiFormat::Invalid => 0,
    }
}

/// Convert HdFormat to HgiFormat.
pub fn hd_format_to_hgi(hd_format: HdFormat) -> HgiFormat {
    match hd_format {
        HdFormat::UNorm8 => HgiFormat::UNorm8,
        HdFormat::UNorm8Vec2 => HgiFormat::UNorm8Vec2,
        HdFormat::UNorm8Vec3 => HgiFormat::UNorm8Vec4, // No 3-channel UNorm8 in HGI
        HdFormat::UNorm8Vec4 => HgiFormat::UNorm8Vec4,
        HdFormat::SNorm8 => HgiFormat::SNorm8,
        HdFormat::SNorm8Vec2 => HgiFormat::SNorm8Vec2,
        HdFormat::SNorm8Vec3 => HgiFormat::SNorm8Vec4,
        HdFormat::SNorm8Vec4 => HgiFormat::SNorm8Vec4,
        HdFormat::Float16 => HgiFormat::Float16,
        HdFormat::Float16Vec2 => HgiFormat::Float16Vec2,
        HdFormat::Float16Vec3 => HgiFormat::Float16Vec3,
        HdFormat::Float16Vec4 => HgiFormat::Float16Vec4,
        HdFormat::Float32 => HgiFormat::Float32,
        HdFormat::Float32Vec2 => HgiFormat::Float32Vec2,
        HdFormat::Float32Vec3 => HgiFormat::Float32Vec3,
        HdFormat::Float32Vec4 => HgiFormat::Float32Vec4,
        HdFormat::Int32 => HgiFormat::Int32,
        HdFormat::Int32Vec2 => HgiFormat::Int32Vec2,
        HdFormat::Int32Vec3 => HgiFormat::Int32Vec3,
        HdFormat::Int32Vec4 => HgiFormat::Int32Vec4,
        HdFormat::Invalid => HgiFormat::Invalid,
        _ => HgiFormat::Invalid,
    }
}

/// Detect texture type from file extension.
pub fn detect_texture_type(file_path: &str) -> TextureType {
    let lower = file_path.to_ascii_lowercase();

    if lower.contains("<udim>") {
        return TextureType::Udim;
    }

    let ext = lower.rsplit('.').next().unwrap_or("");
    match ext {
        "vdb" | "openvdb" | "nvdb" | "field3d" => TextureType::Field,
        "ptx" | "ptex" => TextureType::Ptex,
        _ => TextureType::Uv,
    }
}

/// Compute dimensions that fit within a target memory budget.
///
/// Repeatedly halves dimensions until estimated memory fits.
/// Returns (adjusted_dims, mip_level_used).
pub fn fit_dimensions_to_memory(
    dims: Vec3i,
    format: HgiFormat,
    target_memory: usize,
) -> (Vec3i, u16) {
    if target_memory == 0 {
        return (dims, 0);
    }

    let bpp = format_byte_size(format);
    if bpp == 0 {
        return (dims, 0);
    }

    let mut w = dims[0].max(1);
    let mut h = dims[1].max(1);
    let mut d = dims[2].max(1);
    let mut mip = 0u16;

    loop {
        let mem = (w as usize) * (h as usize) * (d as usize) * bpp;
        if mem <= target_memory || (w <= 1 && h <= 1 && d <= 1) {
            break;
        }
        w = (w / 2).max(1);
        h = (h / 2).max(1);
        d = (d / 2).max(1);
        mip += 1;
    }

    (Vec3i::new(w, h, d), mip)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mip_levels() {
        assert_eq!(calc_mip_levels(1, 1, 1), 1);
        assert_eq!(calc_mip_levels(2, 2, 1), 2);
        assert_eq!(calc_mip_levels(4, 4, 1), 3);
        assert_eq!(calc_mip_levels(256, 256, 1), 9);
        assert_eq!(calc_mip_levels(1024, 512, 1), 11);
        assert_eq!(calc_mip_levels(0, 0, 0), 0);
    }

    #[test]
    fn test_format_byte_size() {
        assert_eq!(format_byte_size(HgiFormat::UNorm8), 1);
        assert_eq!(format_byte_size(HgiFormat::UNorm8Vec4), 4);
        assert_eq!(format_byte_size(HgiFormat::Float32Vec4), 16);
        assert_eq!(format_byte_size(HgiFormat::Float16Vec3), 6);
        assert_eq!(format_byte_size(HgiFormat::Invalid), 0);
    }

    #[test]
    fn test_texture_memory() {
        let mem = calc_texture_memory(Vec3i::new(256, 256, 1), HgiFormat::UNorm8Vec4, 1);
        assert_eq!(mem, 262144); // 256*256*4

        let mem_mips = calc_texture_memory(Vec3i::new(256, 256, 1), HgiFormat::UNorm8Vec4, 9);
        assert!(mem_mips > mem);
    }

    #[test]
    fn test_compressed_format() {
        assert!(is_compressed_format(HgiFormat::BC7UNorm8Vec4));
        assert!(!is_compressed_format(HgiFormat::UNorm8Vec4));
    }

    #[test]
    fn test_has_alpha() {
        assert!(has_alpha(HgiFormat::UNorm8Vec4));
        assert!(has_alpha(HgiFormat::Float32Vec4));
        assert!(!has_alpha(HgiFormat::Float32Vec3));
        assert!(!has_alpha(HgiFormat::UNorm8));
    }

    #[test]
    fn test_component_count() {
        assert_eq!(component_count(HgiFormat::UNorm8), 1);
        assert_eq!(component_count(HgiFormat::Float32Vec2), 2);
        assert_eq!(component_count(HgiFormat::Float32Vec3), 3);
        assert_eq!(component_count(HgiFormat::UNorm8Vec4), 4);
    }

    #[test]
    fn test_detect_type() {
        assert_eq!(detect_texture_type("tex/diffuse.png"), TextureType::Uv);
        assert_eq!(detect_texture_type("vol/density.vdb"), TextureType::Field);
        assert_eq!(detect_texture_type("mesh.ptx"), TextureType::Ptex);
        assert_eq!(
            detect_texture_type("tex/color.<udim>.exr"),
            TextureType::Udim
        );
    }

    #[test]
    fn test_fit_dimensions() {
        let dims = Vec3i::new(1024, 1024, 1);
        let target = 256 * 256 * 4;
        let (fitted, mip) = fit_dimensions_to_memory(dims, HgiFormat::UNorm8Vec4, target);
        assert_eq!(fitted, Vec3i::new(256, 256, 1));
        assert_eq!(mip, 2);
    }
}
