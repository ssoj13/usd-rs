//! UsdVol tokens for volumetric data schemas.
//!
//! These tokens are used for attribute names, data types, and allowed values
//! in the UsdVol schema module.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdVol/tokens.h`

use std::sync::LazyLock;

use usd_tf::Token;

/// All tokens for UsdVol schemas.
pub struct UsdVolTokensType {
    // Data type tokens (allowed values)
    /// "bool" - Field data type
    pub bool_: Token,
    /// "double" - Field data type
    pub double_: Token,
    /// "double2" - Field data type
    pub double2: Token,
    /// "double3" - Field data type
    pub double3: Token,
    /// "float" - Field data type
    pub float_: Token,
    /// "float2" - Field data type
    pub float2: Token,
    /// "float3" - Field data type
    pub float3: Token,
    /// "half" - Field data type
    pub half: Token,
    /// "half2" - Field data type
    pub half2: Token,
    /// "half3" - Field data type
    pub half3: Token,
    /// "int" - Field data type
    pub int_: Token,
    /// "int2" - Field data type
    pub int2: Token,
    /// "int3" - Field data type
    pub int3: Token,
    /// "int64" - Field data type
    pub int64: Token,
    /// "uint" - Field data type
    pub uint: Token,
    /// "mask" - Field data type
    pub mask: Token,
    /// "matrix3d" - Field data type
    pub matrix3d: Token,
    /// "matrix4d" - Field data type
    pub matrix4d: Token,
    /// "quatd" - Field data type
    pub quatd: Token,
    /// "string" - Field data type
    pub string: Token,

    // Field class tokens (OpenVDB grid class)
    /// "fogVolume" - OpenVDB GRID_FOG_VOLUME
    pub fog_volume: Token,
    /// "levelSet" - OpenVDB GRID_LEVEL_SET
    pub level_set: Token,
    /// "staggered" - OpenVDB GRID_STAGGERED
    pub staggered: Token,
    /// "unknown" - OpenVDB GRID_UNKNOWN
    pub unknown: Token,

    // Vector data role hint tokens
    /// "None" - No role hint
    pub none_: Token,
    /// "Color" - Color role
    pub color: Token,
    /// "Normal" - Normal role
    pub normal: Token,
    /// "Point" - Point role
    pub point: Token,
    /// "Vector" - Vector role
    pub vector: Token,

    // Attribute name tokens
    /// "field" - Namespace prefix for field relationships
    pub field: Token,
    /// "fieldClass" - OpenVDB field class attribute
    pub field_class: Token,
    /// "fieldDataType" - Field data type attribute
    pub field_data_type: Token,
    /// "fieldIndex" - Field index attribute
    pub field_index: Token,
    /// "fieldName" - Field name attribute
    pub field_name: Token,
    /// "fieldPurpose" - Field3D purpose attribute
    pub field_purpose: Token,
    /// "filePath" - File path attribute
    pub file_path: Token,
    /// "vectorDataRoleHint" - Vector role hint attribute
    pub vector_data_role_hint: Token,

    // Schema type names
    /// "Field3DAsset" - Schema identifier
    pub field_3d_asset: Token,
    /// "FieldAsset" - Schema identifier
    pub field_asset: Token,
    /// "FieldBase" - Schema identifier
    pub field_base: Token,
    /// "OpenVDBAsset" - Schema identifier
    pub open_vdb_asset: Token,
    /// "Volume" - Schema identifier
    pub volume: Token,
}

impl UsdVolTokensType {
    /// Returns all tokens as a vector.
    /// Matches C++ `UsdVolTokensType::allTokens`.
    pub fn all_tokens(&self) -> Vec<Token> {
        vec![
            self.bool_.clone(),
            self.color.clone(),
            self.double2.clone(),
            self.double3.clone(),
            self.double_.clone(),
            self.field.clone(),
            self.field_class.clone(),
            self.field_data_type.clone(),
            self.field_index.clone(),
            self.field_name.clone(),
            self.field_purpose.clone(),
            self.file_path.clone(),
            self.float2.clone(),
            self.float3.clone(),
            self.float_.clone(),
            self.fog_volume.clone(),
            self.half.clone(),
            self.half2.clone(),
            self.half3.clone(),
            self.int2.clone(),
            self.int3.clone(),
            self.int64.clone(),
            self.int_.clone(),
            self.level_set.clone(),
            self.mask.clone(),
            self.matrix3d.clone(),
            self.matrix4d.clone(),
            self.none_.clone(),
            self.normal.clone(),
            self.point.clone(),
            self.quatd.clone(),
            self.staggered.clone(),
            self.string.clone(),
            self.uint.clone(),
            self.unknown.clone(),
            self.vector.clone(),
            self.vector_data_role_hint.clone(),
            self.field_3d_asset.clone(),
            self.field_asset.clone(),
            self.field_base.clone(),
            self.open_vdb_asset.clone(),
            self.volume.clone(),
        ]
    }
}

impl UsdVolTokensType {
    fn new() -> Self {
        Self {
            // Data types
            bool_: Token::new("bool"),
            double_: Token::new("double"),
            double2: Token::new("double2"),
            double3: Token::new("double3"),
            float_: Token::new("float"),
            float2: Token::new("float2"),
            float3: Token::new("float3"),
            half: Token::new("half"),
            half2: Token::new("half2"),
            half3: Token::new("half3"),
            int_: Token::new("int"),
            int2: Token::new("int2"),
            int3: Token::new("int3"),
            int64: Token::new("int64"),
            uint: Token::new("uint"),
            mask: Token::new("mask"),
            matrix3d: Token::new("matrix3d"),
            matrix4d: Token::new("matrix4d"),
            quatd: Token::new("quatd"),
            string: Token::new("string"),

            // Field classes
            fog_volume: Token::new("fogVolume"),
            level_set: Token::new("levelSet"),
            staggered: Token::new("staggered"),
            unknown: Token::new("unknown"),

            // Vector roles
            none_: Token::new("None"),
            color: Token::new("Color"),
            normal: Token::new("Normal"),
            point: Token::new("Point"),
            vector: Token::new("Vector"),

            // Attribute names
            field: Token::new("field"),
            field_class: Token::new("fieldClass"),
            field_data_type: Token::new("fieldDataType"),
            field_index: Token::new("fieldIndex"),
            field_name: Token::new("fieldName"),
            field_purpose: Token::new("fieldPurpose"),
            file_path: Token::new("filePath"),
            vector_data_role_hint: Token::new("vectorDataRoleHint"),

            // Schema types
            field_3d_asset: Token::new("Field3DAsset"),
            field_asset: Token::new("FieldAsset"),
            field_base: Token::new("FieldBase"),
            open_vdb_asset: Token::new("OpenVDBAsset"),
            volume: Token::new("Volume"),
        }
    }
}

/// Global tokens instance for UsdVol schemas.
pub static USD_VOL_TOKENS: LazyLock<UsdVolTokensType> = LazyLock::new(UsdVolTokensType::new);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokens() {
        assert_eq!(USD_VOL_TOKENS.field.as_str(), "field");
        assert_eq!(USD_VOL_TOKENS.file_path.as_str(), "filePath");
        assert_eq!(USD_VOL_TOKENS.fog_volume.as_str(), "fogVolume");
        assert_eq!(USD_VOL_TOKENS.volume.as_str(), "Volume");
    }
}
