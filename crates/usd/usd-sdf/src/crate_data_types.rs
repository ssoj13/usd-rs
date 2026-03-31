//! Crate file data type enumeration.
//!
//! Port of pxr/usd/sdf/crateDataTypes.h
//!
//! Defines the numeric type codes used in the .usdc binary file format
//! to identify stored value types. Adding new types is backwards-compatible;
//! changing existing numeric values is not.

/// Type code for values stored in .usdc (crate) files.
///
/// These numeric values MUST remain stable for backwards compatibility.
/// Value 0 is reserved for Invalid. New types can only be appended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum CrateTypeId {
    /// Invalid / uninitialized.
    Invalid = 0,

    // -- Array-capable types --
    /// bool
    Bool = 1,
    /// uint8_t
    UChar = 2,
    /// int32_t
    Int = 3,
    /// uint32_t
    UInt = 4,
    /// int64_t
    Int64 = 5,
    /// uint64_t
    UInt64 = 6,
    /// half-float (16-bit)
    Half = 7,
    /// float (32-bit)
    Float = 8,
    /// double (64-bit)
    Double = 9,
    /// std::string
    String = 10,
    /// TfToken
    Token = 11,
    /// SdfAssetPath
    AssetPath = 12,
    /// GfMatrix2d
    Matrix2d = 13,
    /// GfMatrix3d
    Matrix3d = 14,
    /// GfMatrix4d
    Matrix4d = 15,
    /// GfQuatd
    Quatd = 16,
    /// GfQuatf
    Quatf = 17,
    /// GfQuath
    Quath = 18,
    /// GfVec2d
    Vec2d = 19,
    /// GfVec2f
    Vec2f = 20,
    /// GfVec2h
    Vec2h = 21,
    /// GfVec2i
    Vec2i = 22,
    /// GfVec3d
    Vec3d = 23,
    /// GfVec3f
    Vec3f = 24,
    /// GfVec3h
    Vec3h = 25,
    /// GfVec3i
    Vec3i = 26,
    /// GfVec4d
    Vec4d = 27,
    /// GfVec4f
    Vec4f = 28,
    /// GfVec4h
    Vec4h = 29,
    /// GfVec4i
    Vec4i = 30,

    // -- Non-array types --
    /// VtDictionary
    Dictionary = 31,
    /// SdfTokenListOp
    TokenListOp = 32,
    /// SdfStringListOp
    StringListOp = 33,
    /// SdfPathListOp
    PathListOp = 34,
    /// SdfReferenceListOp
    ReferenceListOp = 35,
    /// SdfIntListOp
    IntListOp = 36,
    /// SdfInt64ListOp
    Int64ListOp = 37,
    /// SdfUIntListOp
    UIntListOp = 38,
    /// SdfUInt64ListOp
    UInt64ListOp = 39,
    /// SdfPathVector (Vec<Path>)
    PathVector = 40,
    /// Vec<Token>
    TokenVector = 41,
    /// SdfSpecifier
    Specifier = 42,
    /// SdfPermission
    Permission = 43,
    /// SdfVariability
    Variability = 44,
    /// SdfVariantSelectionMap
    VariantSelectionMap = 45,
    /// TimeSamples
    TimeSamples = 46,
    /// SdfPayload
    Payload = 47,
    /// Vec<f64>
    DoubleVector = 48,
    /// Vec<SdfLayerOffset>
    LayerOffsetVector = 49,
    /// Vec<String>
    StringVector = 50,
    /// SdfValueBlock
    ValueBlock = 51,
    /// VtValue
    Value = 52,
    /// SdfUnregisteredValue
    UnregisteredValue = 53,
    /// SdfUnregisteredValueListOp
    UnregisteredValueListOp = 54,
    /// SdfPayloadListOp
    PayloadListOp = 55,

    // -- Later additions (array-capable) --
    /// SdfTimeCode
    TimeCode = 56,
    /// SdfPathExpression
    PathExpression = 57,

    // -- Later additions (non-array) --
    /// SdfRelocates
    Relocates = 58,
    /// TsSpline
    Spline = 59,
    /// SdfAnimationBlock
    AnimationBlock = 60,
}

impl CrateTypeId {
    /// Returns true if values of this type can be stored as arrays.
    pub fn supports_array(&self) -> bool {
        matches!(
            self,
            Self::Bool
                | Self::UChar
                | Self::Int
                | Self::UInt
                | Self::Int64
                | Self::UInt64
                | Self::Half
                | Self::Float
                | Self::Double
                | Self::String
                | Self::Token
                | Self::AssetPath
                | Self::Matrix2d
                | Self::Matrix3d
                | Self::Matrix4d
                | Self::Quatd
                | Self::Quatf
                | Self::Quath
                | Self::Vec2d
                | Self::Vec2f
                | Self::Vec2h
                | Self::Vec2i
                | Self::Vec3d
                | Self::Vec3f
                | Self::Vec3h
                | Self::Vec3i
                | Self::Vec4d
                | Self::Vec4f
                | Self::Vec4h
                | Self::Vec4i
                | Self::TimeCode
                | Self::PathExpression
        )
    }

    /// Converts from a raw u32 value.
    pub fn from_u32(val: u32) -> Option<Self> {
        // We validate against known range rather than using transmute.
        if val == 0 || val > 60 {
            return None;
        }
        // SAFETY: We've validated the value is in the valid enum range.
        // However some values in the range may not be defined (there are no
        // gaps currently). We use a match for safety.
        match val {
            0 => Some(Self::Invalid),
            1 => Some(Self::Bool),
            2 => Some(Self::UChar),
            3 => Some(Self::Int),
            4 => Some(Self::UInt),
            5 => Some(Self::Int64),
            6 => Some(Self::UInt64),
            7 => Some(Self::Half),
            8 => Some(Self::Float),
            9 => Some(Self::Double),
            10 => Some(Self::String),
            11 => Some(Self::Token),
            12 => Some(Self::AssetPath),
            13 => Some(Self::Matrix2d),
            14 => Some(Self::Matrix3d),
            15 => Some(Self::Matrix4d),
            16 => Some(Self::Quatd),
            17 => Some(Self::Quatf),
            18 => Some(Self::Quath),
            19 => Some(Self::Vec2d),
            20 => Some(Self::Vec2f),
            21 => Some(Self::Vec2h),
            22 => Some(Self::Vec2i),
            23 => Some(Self::Vec3d),
            24 => Some(Self::Vec3f),
            25 => Some(Self::Vec3h),
            26 => Some(Self::Vec3i),
            27 => Some(Self::Vec4d),
            28 => Some(Self::Vec4f),
            29 => Some(Self::Vec4h),
            30 => Some(Self::Vec4i),
            31 => Some(Self::Dictionary),
            32 => Some(Self::TokenListOp),
            33 => Some(Self::StringListOp),
            34 => Some(Self::PathListOp),
            35 => Some(Self::ReferenceListOp),
            36 => Some(Self::IntListOp),
            37 => Some(Self::Int64ListOp),
            38 => Some(Self::UIntListOp),
            39 => Some(Self::UInt64ListOp),
            40 => Some(Self::PathVector),
            41 => Some(Self::TokenVector),
            42 => Some(Self::Specifier),
            43 => Some(Self::Permission),
            44 => Some(Self::Variability),
            45 => Some(Self::VariantSelectionMap),
            46 => Some(Self::TimeSamples),
            47 => Some(Self::Payload),
            48 => Some(Self::DoubleVector),
            49 => Some(Self::LayerOffsetVector),
            50 => Some(Self::StringVector),
            51 => Some(Self::ValueBlock),
            52 => Some(Self::Value),
            53 => Some(Self::UnregisteredValue),
            54 => Some(Self::UnregisteredValueListOp),
            55 => Some(Self::PayloadListOp),
            56 => Some(Self::TimeCode),
            57 => Some(Self::PathExpression),
            58 => Some(Self::Relocates),
            59 => Some(Self::Spline),
            60 => Some(Self::AnimationBlock),
            _ => None,
        }
    }

    /// Returns the display name for this type.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Invalid => "Invalid",
            Self::Bool => "Bool",
            Self::UChar => "UChar",
            Self::Int => "Int",
            Self::UInt => "UInt",
            Self::Int64 => "Int64",
            Self::UInt64 => "UInt64",
            Self::Half => "Half",
            Self::Float => "Float",
            Self::Double => "Double",
            Self::String => "String",
            Self::Token => "Token",
            Self::AssetPath => "AssetPath",
            Self::Matrix2d => "Matrix2d",
            Self::Matrix3d => "Matrix3d",
            Self::Matrix4d => "Matrix4d",
            Self::Quatd => "Quatd",
            Self::Quatf => "Quatf",
            Self::Quath => "Quath",
            Self::Vec2d => "Vec2d",
            Self::Vec2f => "Vec2f",
            Self::Vec2h => "Vec2h",
            Self::Vec2i => "Vec2i",
            Self::Vec3d => "Vec3d",
            Self::Vec3f => "Vec3f",
            Self::Vec3h => "Vec3h",
            Self::Vec3i => "Vec3i",
            Self::Vec4d => "Vec4d",
            Self::Vec4f => "Vec4f",
            Self::Vec4h => "Vec4h",
            Self::Vec4i => "Vec4i",
            Self::Dictionary => "Dictionary",
            Self::TokenListOp => "TokenListOp",
            Self::StringListOp => "StringListOp",
            Self::PathListOp => "PathListOp",
            Self::ReferenceListOp => "ReferenceListOp",
            Self::IntListOp => "IntListOp",
            Self::Int64ListOp => "Int64ListOp",
            Self::UIntListOp => "UIntListOp",
            Self::UInt64ListOp => "UInt64ListOp",
            Self::PathVector => "PathVector",
            Self::TokenVector => "TokenVector",
            Self::Specifier => "Specifier",
            Self::Permission => "Permission",
            Self::Variability => "Variability",
            Self::VariantSelectionMap => "VariantSelectionMap",
            Self::TimeSamples => "TimeSamples",
            Self::Payload => "Payload",
            Self::DoubleVector => "DoubleVector",
            Self::LayerOffsetVector => "LayerOffsetVector",
            Self::StringVector => "StringVector",
            Self::ValueBlock => "ValueBlock",
            Self::Value => "Value",
            Self::UnregisteredValue => "UnregisteredValue",
            Self::UnregisteredValueListOp => "UnregisteredValueListOp",
            Self::PayloadListOp => "PayloadListOp",
            Self::TimeCode => "TimeCode",
            Self::PathExpression => "PathExpression",
            Self::Relocates => "Relocates",
            Self::Spline => "Spline",
            Self::AnimationBlock => "AnimationBlock",
        }
    }
}

impl std::fmt::Display for CrateTypeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round_trip() {
        for val in 1..=60u32 {
            let type_id = CrateTypeId::from_u32(val).unwrap();
            assert_eq!(type_id as u32, val);
        }
    }

    #[test]
    fn test_invalid() {
        assert_eq!(CrateTypeId::from_u32(0), None);
        assert_eq!(CrateTypeId::from_u32(999), None);
    }

    #[test]
    fn test_array_support() {
        assert!(CrateTypeId::Bool.supports_array());
        assert!(CrateTypeId::Vec3f.supports_array());
        assert!(!CrateTypeId::Dictionary.supports_array());
        assert!(!CrateTypeId::Specifier.supports_array());
    }
}
