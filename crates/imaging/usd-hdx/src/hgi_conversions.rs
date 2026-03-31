
//! HGI type conversions for Hydra extensions.
//!
//! Converts between `HdFormat` (Hydra's abstract format enum) and `HgiFormat`
//! (the low-level Graphics Interface format) using a bidirectional lookup table.
//!
//! The mapping follows the C++ `FORMAT_DESC` table in `hgiConversions.cpp`.
//!
//! Port of pxr/imaging/hdx/hgiConversions.h/cpp

use usd_hgi::HgiFormat;

/// Hydra format enumeration.
///
/// This is a distinct enum from `HgiFormat` — it represents the abstract
/// Hydra-layer format identifiers. Some entries have no direct HgiFormat
/// equivalent (e.g. `UNorm8Vec3`, which HGI does not support).
///
/// The integer values match the C++ `HdFormat` enum ordinals exactly.
/// C++ `HdFormatCount == 29` validates the table stays in sync.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum HdFormat {
    Invalid = -1,

    UNorm8 = 0,
    UNorm8Vec2 = 1,
    UNorm8Vec3 = 2, // No HgiFormat equivalent
    UNorm8Vec4 = 3,

    SNorm8 = 4,
    SNorm8Vec2 = 5,
    SNorm8Vec3 = 6, // No HgiFormat equivalent
    SNorm8Vec4 = 7,

    Float16 = 8,
    Float16Vec2 = 9,
    Float16Vec3 = 10,
    Float16Vec4 = 11,

    Float32 = 12,
    Float32Vec2 = 13,
    Float32Vec3 = 14,
    Float32Vec4 = 15,

    Int16 = 16,
    Int16Vec2 = 17,
    Int16Vec3 = 18,
    Int16Vec4 = 19,

    UInt16 = 20,
    UInt16Vec2 = 21,
    UInt16Vec3 = 22,
    UInt16Vec4 = 23,

    Int32 = 24,
    Int32Vec2 = 25,
    Int32Vec3 = 26,
    Int32Vec4 = 27,

    Float32UInt8 = 28,

    Count = 29,
}

/// Bidirectional lookup table entry mirroring C++ `_FormatDesc`.
struct FormatDesc {
    hd: HdFormat,
    hgi: HgiFormat,
}

/// Format conversion table matching C++ `FORMAT_DESC[]` in `hgiConversions.cpp`.
///
/// `HgiFormatInvalid` marks entries where no HGI equivalent exists.
static FORMAT_DESC: &[FormatDesc] = &[
    FormatDesc {
        hd: HdFormat::UNorm8,
        hgi: HgiFormat::UNorm8,
    },
    FormatDesc {
        hd: HdFormat::UNorm8Vec2,
        hgi: HgiFormat::UNorm8Vec2,
    },
    FormatDesc {
        hd: HdFormat::UNorm8Vec3,
        hgi: HgiFormat::Invalid,
    }, // Unsupported by HGI
    FormatDesc {
        hd: HdFormat::UNorm8Vec4,
        hgi: HgiFormat::UNorm8Vec4,
    },
    FormatDesc {
        hd: HdFormat::SNorm8,
        hgi: HgiFormat::SNorm8,
    },
    FormatDesc {
        hd: HdFormat::SNorm8Vec2,
        hgi: HgiFormat::SNorm8Vec2,
    },
    FormatDesc {
        hd: HdFormat::SNorm8Vec3,
        hgi: HgiFormat::Invalid,
    }, // Unsupported by HGI
    FormatDesc {
        hd: HdFormat::SNorm8Vec4,
        hgi: HgiFormat::SNorm8Vec4,
    },
    FormatDesc {
        hd: HdFormat::Float16,
        hgi: HgiFormat::Float16,
    },
    FormatDesc {
        hd: HdFormat::Float16Vec2,
        hgi: HgiFormat::Float16Vec2,
    },
    FormatDesc {
        hd: HdFormat::Float16Vec3,
        hgi: HgiFormat::Float16Vec3,
    },
    FormatDesc {
        hd: HdFormat::Float16Vec4,
        hgi: HgiFormat::Float16Vec4,
    },
    FormatDesc {
        hd: HdFormat::Float32,
        hgi: HgiFormat::Float32,
    },
    FormatDesc {
        hd: HdFormat::Float32Vec2,
        hgi: HgiFormat::Float32Vec2,
    },
    FormatDesc {
        hd: HdFormat::Float32Vec3,
        hgi: HgiFormat::Float32Vec3,
    },
    FormatDesc {
        hd: HdFormat::Float32Vec4,
        hgi: HgiFormat::Float32Vec4,
    },
    FormatDesc {
        hd: HdFormat::Int16,
        hgi: HgiFormat::Int16,
    },
    FormatDesc {
        hd: HdFormat::Int16Vec2,
        hgi: HgiFormat::Int16Vec2,
    },
    FormatDesc {
        hd: HdFormat::Int16Vec3,
        hgi: HgiFormat::Int16Vec3,
    },
    FormatDesc {
        hd: HdFormat::Int16Vec4,
        hgi: HgiFormat::Int16Vec4,
    },
    FormatDesc {
        hd: HdFormat::UInt16,
        hgi: HgiFormat::UInt16,
    },
    FormatDesc {
        hd: HdFormat::UInt16Vec2,
        hgi: HgiFormat::UInt16Vec2,
    },
    FormatDesc {
        hd: HdFormat::UInt16Vec3,
        hgi: HgiFormat::UInt16Vec3,
    },
    FormatDesc {
        hd: HdFormat::UInt16Vec4,
        hgi: HgiFormat::UInt16Vec4,
    },
    FormatDesc {
        hd: HdFormat::Int32,
        hgi: HgiFormat::Int32,
    },
    FormatDesc {
        hd: HdFormat::Int32Vec2,
        hgi: HgiFormat::Int32Vec2,
    },
    FormatDesc {
        hd: HdFormat::Int32Vec3,
        hgi: HgiFormat::Int32Vec3,
    },
    FormatDesc {
        hd: HdFormat::Int32Vec4,
        hgi: HgiFormat::Int32Vec4,
    },
    FormatDesc {
        hd: HdFormat::Float32UInt8,
        hgi: HgiFormat::Float32UInt8,
    },
];

/// Converts between `HdFormat` and `HgiFormat`.
///
/// Static conversion utilities matching `HdxHgiConversions` in C++.
pub struct HdxHgiConversions;

impl HdxHgiConversions {
    /// Convert `HdFormat` to the corresponding `HgiFormat`.
    ///
    /// Returns `HgiFormat::Invalid` if the format is out of range or has
    /// no HGI equivalent (e.g. `UNorm8Vec3`).
    pub fn get_hgi_format(hd_format: HdFormat) -> HgiFormat {
        let idx = hd_format as i32;
        if idx < 0 || idx >= HdFormat::Count as i32 {
            return HgiFormat::Invalid;
        }
        FORMAT_DESC[idx as usize].hgi
    }

    /// Convert `HgiFormat` back to `HdFormat`.
    ///
    /// Performs a linear scan of the table (matching C++ behaviour).
    /// Returns `HdFormat::Invalid` if no mapping exists.
    pub fn get_hd_format(hgi_format: HgiFormat) -> HdFormat {
        if hgi_format == HgiFormat::Invalid {
            return HdFormat::Invalid;
        }
        for desc in FORMAT_DESC {
            if desc.hgi == hgi_format {
                return desc.hd;
            }
        }
        HdFormat::Invalid
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_table_entry_count() {
        // C++ static_assert: HdFormatCount == 29
        assert_eq!(FORMAT_DESC.len(), 29);
    }

    #[test]
    fn test_float32vec4_roundtrip() {
        let hd = HdFormat::Float32Vec4;
        let hgi = HdxHgiConversions::get_hgi_format(hd);
        assert_eq!(hgi, HgiFormat::Float32Vec4);
        let back = HdxHgiConversions::get_hd_format(hgi);
        assert_eq!(back, HdFormat::Float32Vec4);
    }

    #[test]
    fn test_unorm8vec3_has_no_hgi() {
        // UNorm8Vec3 has no HGI equivalent
        let hgi = HdxHgiConversions::get_hgi_format(HdFormat::UNorm8Vec3);
        assert_eq!(hgi, HgiFormat::Invalid);
    }

    #[test]
    fn test_float32uint8() {
        // Float32UInt8 is the last entry
        let hgi = HdxHgiConversions::get_hgi_format(HdFormat::Float32UInt8);
        assert_eq!(hgi, HgiFormat::Float32UInt8);
        let back = HdxHgiConversions::get_hd_format(HgiFormat::Float32UInt8);
        assert_eq!(back, HdFormat::Float32UInt8);
    }

    #[test]
    fn test_invalid_hd_format() {
        let hgi = HdxHgiConversions::get_hgi_format(HdFormat::Invalid);
        assert_eq!(hgi, HgiFormat::Invalid);
    }

    #[test]
    fn test_invalid_hgi_format() {
        let hd = HdxHgiConversions::get_hd_format(HgiFormat::Invalid);
        assert_eq!(hd, HdFormat::Invalid);
    }

    #[test]
    fn test_all_hd_formats_roundtrip() {
        // All formats with valid HGI mappings should round-trip
        for desc in FORMAT_DESC {
            if desc.hgi != HgiFormat::Invalid {
                let back = HdxHgiConversions::get_hd_format(desc.hgi);
                assert_eq!(back, desc.hd, "Roundtrip failed for {:?}", desc.hd);
            }
        }
    }
}
