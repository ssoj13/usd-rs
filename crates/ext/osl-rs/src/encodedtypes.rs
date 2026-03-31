//! Compact integer codes for OSL types in the batched/SIMD pathway.
//!
//! Ported from `encodedtypes.h`. `EncodedType` tags each argument in a blind
//! byte payload so `decode_message` can reconstruct typed values for
//! `fmtlib`-style formatting at runtime.

use crate::typedesc::{Aggregate, BaseType, TypeDesc, VecSemantics};

/// Compact type tag used to identify values in a blind byte payload.
///
/// Matches the C++ `enum class EncodedType : uint8_t` from
/// `OSL/encodedtypes.h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EncodedType {
    // OSL shaders can encode these types.
    UstringHash = 0,
    Int32 = 1,
    Float = 2,

    // OSL library functions / renderer services encode these additional types.
    Int64 = 3,
    Double = 4,
    UInt32 = 5,
    UInt64 = 6,
    Pointer = 7,
    TypeDesc = 8,
}

/// Total number of valid `EncodedType` variants (sentinel, not a real type).
pub const ENCODED_TYPE_COUNT: u8 = 9;

impl EncodedType {
    /// Size in bytes of the encoded value for this type tag.
    ///
    /// Matches `pvt::size_of_encoded_type` from C++.
    pub const fn size(self) -> usize {
        match self {
            // ustringhash is pointer-sized in C++; we store it as u64 for
            // cross-platform consistency with the batched pathway.
            EncodedType::UstringHash => 8,
            EncodedType::Int32 => 4,
            EncodedType::Float => 4,
            EncodedType::Int64 => 8,
            EncodedType::Double => 8,
            EncodedType::UInt32 => 4,
            EncodedType::UInt64 => 8,
            // Pointer is stored as u64 (8 bytes) even on 32-bit to keep the
            // payload layout fixed.
            EncodedType::Pointer => 8,
            // TypeDesc is bitcast to u64 (8 bytes).
            EncodedType::TypeDesc => 8,
        }
    }

    /// Try to convert a raw `u8` discriminant to an `EncodedType`.
    pub const fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(EncodedType::UstringHash),
            1 => Some(EncodedType::Int32),
            2 => Some(EncodedType::Float),
            3 => Some(EncodedType::Int64),
            4 => Some(EncodedType::Double),
            5 => Some(EncodedType::UInt32),
            6 => Some(EncodedType::UInt64),
            7 => Some(EncodedType::Pointer),
            8 => Some(EncodedType::TypeDesc),
            _ => None,
        }
    }
}

/// Encode a `TypeDesc` into its compact `EncodedType` tag.
///
/// Only scalar (non-array, aggregate=Scalar) base types have a direct
/// mapping. Returns `None` for compound / array types.
pub fn encode(td: &crate::typedesc::TypeDesc) -> Option<EncodedType> {
    // Arrays and non-scalar aggregates don't map to a single encoded tag.
    if td.arraylen != 0 || td.aggregate != Aggregate::Scalar as u8 {
        return None;
    }
    let bt = BaseType::from_u8(td.basetype);
    match bt {
        BaseType::UStringHash => Some(EncodedType::UstringHash),
        BaseType::Int32 => Some(EncodedType::Int32),
        BaseType::Float => Some(EncodedType::Float),
        BaseType::Int64 => Some(EncodedType::Int64),
        BaseType::Double => Some(EncodedType::Double),
        BaseType::UInt32 => Some(EncodedType::UInt32),
        BaseType::UInt64 => Some(EncodedType::UInt64),
        BaseType::Ptr => Some(EncodedType::Pointer),
        _ => None,
    }
}

/// Decode an `EncodedType` tag back into the corresponding scalar `TypeDesc`.
///
/// The `TypeDesc` variant is special: it returns a `TypeDesc` whose basetype
/// is `UInt64` because C++ stores `TypeDesc` as a bitcast `u64`.
pub fn decode(et: EncodedType) -> crate::typedesc::TypeDesc {
    let bt = match et {
        EncodedType::UstringHash => BaseType::UStringHash,
        EncodedType::Int32 => BaseType::Int32,
        EncodedType::Float => BaseType::Float,
        EncodedType::Int64 => BaseType::Int64,
        EncodedType::Double => BaseType::Double,
        EncodedType::UInt32 => BaseType::UInt32,
        EncodedType::UInt64 => BaseType::UInt64,
        EncodedType::Pointer => BaseType::Ptr,
        // TypeDesc is encoded as u64 in the payload.
        EncodedType::TypeDesc => BaseType::UInt64,
    };
    TypeDesc {
        basetype: bt as u8,
        aggregate: Aggregate::Scalar as u8,
        vecsemantics: VecSemantics::NoXform as u8,
        reserved: 0,
        arraylen: 0,
    }
}

/// Decode an encoded message from format hash + typed arg payload into a String.
///
/// Matches C++ `OSL::decode_message()` from `encodedtypes.h`.
/// Interprets each `EncodedType` in `arg_types` to read values from the
/// `arg_values` byte buffer, then formats them using the format string
/// identified by `format_hash`.
///
/// Returns the decoded message string. If format resolution fails,
/// returns a debug representation of the arguments.
pub fn decode_message(
    _format_hash: u64,
    arg_count: i32,
    arg_types: &[EncodedType],
    arg_values: &[u8],
) -> String {
    // In a full implementation, format_hash would be resolved to a
    // ustringhash -> format string via the global string table.
    // For now, we extract typed values and produce a debug-style output
    // that includes all argument values (matching the C++ fallback behavior).
    let mut result = String::new();
    let mut offset = 0usize;
    let count = arg_count as usize;

    for i in 0..count {
        if i >= arg_types.len() {
            break;
        }
        let et = arg_types[i];
        let sz = et.size();
        if offset + sz > arg_values.len() {
            break;
        }

        if i > 0 {
            result.push(' ');
        }

        match et {
            EncodedType::UstringHash => {
                let val = read_u64_le(&arg_values[offset..]);
                // UStringHash value -- in production would resolve via string table
                result.push_str(&format!("<str:{:#x}>", val));
            }
            EncodedType::Int32 => {
                let val = read_i32_le(&arg_values[offset..]);
                result.push_str(&format!("{}", val));
            }
            EncodedType::Float => {
                let val = read_f32_le(&arg_values[offset..]);
                result.push_str(&format!("{}", val));
            }
            EncodedType::Int64 => {
                let val = read_i64_le(&arg_values[offset..]);
                result.push_str(&format!("{}", val));
            }
            EncodedType::Double => {
                let val = read_f64_le(&arg_values[offset..]);
                result.push_str(&format!("{}", val));
            }
            EncodedType::UInt32 => {
                let val = read_u32_le(&arg_values[offset..]);
                result.push_str(&format!("{}", val));
            }
            EncodedType::UInt64 => {
                let val = read_u64_le(&arg_values[offset..]);
                result.push_str(&format!("{}", val));
            }
            EncodedType::Pointer => {
                let val = read_u64_le(&arg_values[offset..]);
                result.push_str(&format!("{:#x}", val));
            }
            EncodedType::TypeDesc => {
                let val = read_u64_le(&arg_values[offset..]);
                result.push_str(&format!("<TypeDesc:{:#x}>", val));
            }
        }
        offset += sz;
    }
    result
}

// Little-endian helpers for reading typed values from byte slices.

fn read_u64_le(buf: &[u8]) -> u64 {
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&buf[..8]);
    u64::from_le_bytes(bytes)
}

fn read_i64_le(buf: &[u8]) -> i64 {
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&buf[..8]);
    i64::from_le_bytes(bytes)
}

fn read_u32_le(buf: &[u8]) -> u32 {
    let mut bytes = [0u8; 4];
    bytes.copy_from_slice(&buf[..4]);
    u32::from_le_bytes(bytes)
}

fn read_i32_le(buf: &[u8]) -> i32 {
    let mut bytes = [0u8; 4];
    bytes.copy_from_slice(&buf[..4]);
    i32::from_le_bytes(bytes)
}

fn read_f32_le(buf: &[u8]) -> f32 {
    let mut bytes = [0u8; 4];
    bytes.copy_from_slice(&buf[..4]);
    f32::from_le_bytes(bytes)
}

fn read_f64_le(buf: &[u8]) -> f64 {
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&buf[..8]);
    f64::from_le_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typedesc::{Aggregate, BaseType, TypeDesc, VecSemantics};

    /// Helper: build a scalar TypeDesc from a BaseType.
    fn scalar(bt: BaseType) -> TypeDesc {
        TypeDesc {
            basetype: bt as u8,
            aggregate: Aggregate::Scalar as u8,
            vecsemantics: VecSemantics::NoXform as u8,
            reserved: 0,
            arraylen: 0,
        }
    }

    #[test]
    fn roundtrip_all_scalar_types() {
        let pairs = [
            (BaseType::UStringHash, EncodedType::UstringHash),
            (BaseType::Int32, EncodedType::Int32),
            (BaseType::Float, EncodedType::Float),
            (BaseType::Int64, EncodedType::Int64),
            (BaseType::Double, EncodedType::Double),
            (BaseType::UInt32, EncodedType::UInt32),
            (BaseType::UInt64, EncodedType::UInt64),
            (BaseType::Ptr, EncodedType::Pointer),
        ];
        for (bt, expected_et) in pairs {
            let td = scalar(bt);
            let et = encode(&td).expect("encode should succeed");
            assert_eq!(et, expected_et, "encode mismatch for {:?}", bt);
            let decoded = decode(et);
            assert_eq!(decoded, td, "round-trip mismatch for {:?}", bt);
        }
    }

    #[test]
    fn encode_typedesc_variant_not_in_basetype() {
        // There is no BaseType for "TypeDesc itself"; encode should return None
        // for unsupported base types. decode(TypeDesc) returns UInt64 scalar.
        let decoded = decode(EncodedType::TypeDesc);
        assert_eq!(decoded, scalar(BaseType::UInt64));
    }

    #[test]
    fn encode_rejects_arrays() {
        let mut td = scalar(BaseType::Float);
        td.arraylen = 3;
        assert!(encode(&td).is_none(), "arrays should not encode");
    }

    #[test]
    fn encode_rejects_aggregates() {
        let td = TypeDesc {
            basetype: BaseType::Float as u8,
            aggregate: Aggregate::Vec3 as u8,
            vecsemantics: VecSemantics::NoXform as u8,
            reserved: 0,
            arraylen: 0,
        };
        assert!(
            encode(&td).is_none(),
            "non-scalar aggregates should not encode"
        );
    }

    #[test]
    fn encode_rejects_unknown() {
        let td = scalar(BaseType::Unknown);
        assert!(encode(&td).is_none());
    }

    #[test]
    fn from_u8_valid_range() {
        for v in 0..ENCODED_TYPE_COUNT {
            assert!(EncodedType::from_u8(v).is_some(), "v={}", v);
        }
    }

    #[test]
    fn from_u8_out_of_range() {
        assert!(EncodedType::from_u8(ENCODED_TYPE_COUNT).is_none());
        assert!(EncodedType::from_u8(255).is_none());
    }

    #[test]
    fn size_matches_expected() {
        assert_eq!(EncodedType::UstringHash.size(), 8);
        assert_eq!(EncodedType::Int32.size(), 4);
        assert_eq!(EncodedType::Float.size(), 4);
        assert_eq!(EncodedType::Int64.size(), 8);
        assert_eq!(EncodedType::Double.size(), 8);
        assert_eq!(EncodedType::UInt32.size(), 4);
        assert_eq!(EncodedType::UInt64.size(), 8);
        assert_eq!(EncodedType::Pointer.size(), 8);
        assert_eq!(EncodedType::TypeDesc.size(), 8);
    }
}
