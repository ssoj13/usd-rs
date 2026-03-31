//! HGI type conversions for Storm.
//!
//! Ported from C++ hgiConversions.h/cpp. Converts Hydra types (HdFormat, HdType)
//! to HGI types (HgiFormat) for resource creation.

use usd_hd::types::HdType;
use usd_hgi::HgiFormat;

/// Hydra texture/buffer format (matches HdFormat in C++).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HdFormat {
    Invalid,
    UNorm8,
    UNorm8Vec2,
    UNorm8Vec3,
    UNorm8Vec4,
    SNorm8,
    SNorm8Vec2,
    SNorm8Vec3,
    SNorm8Vec4,
    Float16,
    Float16Vec2,
    Float16Vec3,
    Float16Vec4,
    Float32,
    Float32Vec2,
    Float32Vec3,
    Float32Vec4,
    UInt16,
    UInt16Vec2,
    UInt16Vec3,
    UInt16Vec4,
    UInt32,
    UInt32Vec2,
    UInt32Vec3,
    UInt32Vec4,
    Int16,
    Int16Vec2,
    Int16Vec3,
    Int16Vec4,
    Int32,
    Int32Vec2,
    Int32Vec3,
    Int32Vec4,
}

/// Convert HdType to HgiFormat for vertex attribute declarations.
///
/// Note: HgiFormat has no unsigned 32-bit integer variants.
/// UInt32 types are mapped to Int32 equivalents (same byte layout).
pub fn hd_type_to_hgi(ty: HdType) -> HgiFormat {
    match ty {
        HdType::Int32 | HdType::UInt32 => HgiFormat::Int32,
        HdType::HalfFloat => HgiFormat::Float16,
        HdType::Float => HgiFormat::Float32,
        HdType::Int32Vec2 | HdType::UInt32Vec2 => HgiFormat::Int32Vec2,
        HdType::HalfFloatVec2 => HgiFormat::Float16Vec2,
        HdType::FloatVec2 => HgiFormat::Float32Vec2,
        HdType::Int32Vec3 | HdType::UInt32Vec3 => HgiFormat::Int32Vec3,
        HdType::HalfFloatVec3 => HgiFormat::Float16Vec3,
        HdType::FloatVec3 => HgiFormat::Float32Vec3,
        HdType::Int32Vec4 | HdType::UInt32Vec4 => HgiFormat::Int32Vec4,
        HdType::HalfFloatVec4 => HgiFormat::Float16Vec4,
        HdType::FloatVec4 => HgiFormat::Float32Vec4,
        HdType::Int16 => HgiFormat::Int16,
        HdType::UInt16 => HgiFormat::UInt16,
        // Note: HdType has no Int16Vec2/3/4 or UInt16Vec2/3/4 variants.
        // HgiFormat does, but they're only reachable via HdFormat conversion.
        _ => HgiFormat::Invalid,
    }
}

/// Convert HdFormat to HgiFormat for texture/buffer creation.
pub fn hd_format_to_hgi(fmt: HdFormat) -> HgiFormat {
    match fmt {
        HdFormat::Invalid => HgiFormat::Invalid,
        HdFormat::UNorm8 => HgiFormat::UNorm8,
        HdFormat::UNorm8Vec2 => HgiFormat::UNorm8Vec2,
        HdFormat::UNorm8Vec3 => HgiFormat::UNorm8Vec4, // vec3 unsupported in Metal, promote
        HdFormat::UNorm8Vec4 => HgiFormat::UNorm8Vec4,
        HdFormat::SNorm8 => HgiFormat::SNorm8,
        HdFormat::SNorm8Vec2 => HgiFormat::SNorm8Vec2,
        HdFormat::SNorm8Vec3 => HgiFormat::SNorm8Vec4, // promote
        HdFormat::SNorm8Vec4 => HgiFormat::SNorm8Vec4,
        HdFormat::Float16 => HgiFormat::Float16,
        HdFormat::Float16Vec2 => HgiFormat::Float16Vec2,
        HdFormat::Float16Vec3 => HgiFormat::Float16Vec3,
        HdFormat::Float16Vec4 => HgiFormat::Float16Vec4,
        HdFormat::Float32 => HgiFormat::Float32,
        HdFormat::Float32Vec2 => HgiFormat::Float32Vec2,
        HdFormat::Float32Vec3 => HgiFormat::Float32Vec3,
        HdFormat::Float32Vec4 => HgiFormat::Float32Vec4,
        HdFormat::UInt16 => HgiFormat::UInt16,
        HdFormat::UInt16Vec2 => HgiFormat::UInt16Vec2,
        HdFormat::UInt16Vec3 => HgiFormat::UInt16Vec3,
        HdFormat::UInt16Vec4 => HgiFormat::UInt16Vec4,
        // UInt32 has no HgiFormat equiv, map to Int32 (same byte size)
        HdFormat::UInt32 => HgiFormat::Int32,
        HdFormat::UInt32Vec2 => HgiFormat::Int32Vec2,
        HdFormat::UInt32Vec3 => HgiFormat::Int32Vec3,
        HdFormat::UInt32Vec4 => HgiFormat::Int32Vec4,
        HdFormat::Int16 => HgiFormat::Int16,
        HdFormat::Int16Vec2 => HgiFormat::Int16Vec2,
        HdFormat::Int16Vec3 => HgiFormat::Int16Vec3,
        HdFormat::Int16Vec4 => HgiFormat::Int16Vec4,
        HdFormat::Int32 => HgiFormat::Int32,
        HdFormat::Int32Vec2 => HgiFormat::Int32Vec2,
        HdFormat::Int32Vec3 => HgiFormat::Int32Vec3,
        HdFormat::Int32Vec4 => HgiFormat::Int32Vec4,
    }
}

/// Byte size of an HgiFormat element.
pub fn hgi_format_byte_size(fmt: HgiFormat) -> usize {
    match fmt {
        HgiFormat::UNorm8 | HgiFormat::SNorm8 => 1,
        HgiFormat::UNorm8Vec2 | HgiFormat::SNorm8Vec2 => 2,
        HgiFormat::UNorm8Vec4 | HgiFormat::SNorm8Vec4 => 4,
        HgiFormat::Float16 | HgiFormat::Int16 | HgiFormat::UInt16 => 2,
        HgiFormat::Float16Vec2 | HgiFormat::Int16Vec2 | HgiFormat::UInt16Vec2 => 4,
        HgiFormat::Float16Vec3 | HgiFormat::Int16Vec3 | HgiFormat::UInt16Vec3 => 6,
        HgiFormat::Float16Vec4 | HgiFormat::Int16Vec4 | HgiFormat::UInt16Vec4 => 8,
        HgiFormat::Float32 | HgiFormat::Int32 => 4,
        HgiFormat::Float32Vec2 | HgiFormat::Int32Vec2 => 8,
        HgiFormat::Float32Vec3 | HgiFormat::Int32Vec3 => 12,
        HgiFormat::Float32Vec4 | HgiFormat::Int32Vec4 => 16,
        _ => 0,
    }
}

/// Get WGSL type name for an HgiFormat (for shader codegen).
pub fn hgi_format_to_wgsl(fmt: HgiFormat) -> &'static str {
    match fmt {
        HgiFormat::Float32 => "f32",
        HgiFormat::Float32Vec2 => "vec2<f32>",
        HgiFormat::Float32Vec3 => "vec3<f32>",
        HgiFormat::Float32Vec4 => "vec4<f32>",
        HgiFormat::Int32 => "i32",
        HgiFormat::Int32Vec2 => "vec2<i32>",
        HgiFormat::Int32Vec3 => "vec3<i32>",
        HgiFormat::Int32Vec4 => "vec4<i32>",
        HgiFormat::Float16 => "f16",
        HgiFormat::Float16Vec2 => "vec2<f16>",
        HgiFormat::Float16Vec3 => "vec3<f16>",
        HgiFormat::Float16Vec4 => "vec4<f16>",
        HgiFormat::UNorm8Vec4 | HgiFormat::SNorm8Vec4 => "vec4<f32>",
        _ => "f32", // fallback
    }
}

/// Number of components in an HgiFormat.
pub fn hgi_format_components(fmt: HgiFormat) -> u32 {
    match fmt {
        HgiFormat::Float32
        | HgiFormat::Int32
        | HgiFormat::Float16
        | HgiFormat::Int16
        | HgiFormat::UInt16
        | HgiFormat::UNorm8
        | HgiFormat::SNorm8 => 1,
        HgiFormat::Float32Vec2
        | HgiFormat::Int32Vec2
        | HgiFormat::Float16Vec2
        | HgiFormat::Int16Vec2
        | HgiFormat::UInt16Vec2
        | HgiFormat::UNorm8Vec2
        | HgiFormat::SNorm8Vec2 => 2,
        HgiFormat::Float32Vec3
        | HgiFormat::Int32Vec3
        | HgiFormat::Float16Vec3
        | HgiFormat::Int16Vec3
        | HgiFormat::UInt16Vec3 => 3,
        HgiFormat::Float32Vec4
        | HgiFormat::Int32Vec4
        | HgiFormat::Float16Vec4
        | HgiFormat::Int16Vec4
        | HgiFormat::UInt16Vec4
        | HgiFormat::UNorm8Vec4
        | HgiFormat::SNorm8Vec4 => 4,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hd_type_to_hgi() {
        assert_eq!(hd_type_to_hgi(HdType::FloatVec3), HgiFormat::Float32Vec3);
        assert_eq!(hd_type_to_hgi(HdType::Int32Vec4), HgiFormat::Int32Vec4);
        assert_eq!(hd_type_to_hgi(HdType::Double), HgiFormat::Invalid);
    }

    #[test]
    fn test_uint32_maps_to_int32() {
        assert_eq!(hd_type_to_hgi(HdType::UInt32), HgiFormat::Int32);
        assert_eq!(hd_type_to_hgi(HdType::UInt32Vec3), HgiFormat::Int32Vec3);
        assert_eq!(hd_format_to_hgi(HdFormat::UInt32Vec4), HgiFormat::Int32Vec4);
    }

    #[test]
    fn test_hd_format_vec3_promotion() {
        assert_eq!(
            hd_format_to_hgi(HdFormat::UNorm8Vec3),
            HgiFormat::UNorm8Vec4
        );
        assert_eq!(
            hd_format_to_hgi(HdFormat::SNorm8Vec3),
            HgiFormat::SNorm8Vec4
        );
    }

    #[test]
    fn test_byte_sizes() {
        assert_eq!(hgi_format_byte_size(HgiFormat::Float32Vec3), 12);
        assert_eq!(hgi_format_byte_size(HgiFormat::Float32Vec4), 16);
        assert_eq!(hgi_format_byte_size(HgiFormat::UNorm8Vec4), 4);
    }

    #[test]
    fn test_wgsl_names() {
        assert_eq!(hgi_format_to_wgsl(HgiFormat::Float32Vec3), "vec3<f32>");
        assert_eq!(hgi_format_to_wgsl(HgiFormat::Float32Vec4), "vec4<f32>");
    }
}
