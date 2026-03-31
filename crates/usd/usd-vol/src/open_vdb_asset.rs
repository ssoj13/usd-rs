//! OpenVDB Asset schema.
//!
//! Concrete field primitive for OpenVDB format files.
//! The filePath attribute must specify a file in OpenVDB format.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdVol/openVDBAsset.h`

use std::sync::Arc;

use usd_core::attribute::Variability;
use usd_core::{Attribute, Prim, Stage};
use usd_sdf::{Path, ValueTypeRegistry};
use usd_tf::Token;

use super::field_asset::FieldAsset;
use super::tokens::USD_VOL_TOKENS;

/// OpenVDB field primitive.
///
/// The FieldAsset filePath attribute must specify a file in OpenVDB format.
///
/// # Additional Attributes
///
/// - `fieldDataType` - Override with OpenVDB-specific allowed values
/// - `fieldClass` - Grid class (levelSet, fogVolume, staggered, unknown)
///
/// # Schema Kind
///
/// This is a concrete typed schema (ConcreteTyped).
#[derive(Debug, Clone)]
pub struct OpenVDBAsset {
    field_asset: FieldAsset,
}

impl OpenVDBAsset {
    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "OpenVDBAsset";

    /// Construct an OpenVDBAsset on the given prim.
    pub fn new(prim: Prim) -> Self {
        Self {
            field_asset: FieldAsset::new(prim),
        }
    }

    /// Construct from another prim.
    pub fn from_prim(prim: &Prim) -> Self {
        Self::new(prim.clone())
    }

    /// Return an OpenVDBAsset holding the prim at `path` on `stage`.
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage.get_prim_at_path(path)?;
        if prim.is_a(&USD_VOL_TOKENS.open_vdb_asset) {
            Some(Self::new(prim))
        } else {
            None
        }
    }

    /// Attempt to ensure a prim adhering to this schema at `path` is defined.
    pub fn define(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage
            .define_prim(path.as_str(), Self::SCHEMA_TYPE_NAME)
            .ok()?;
        Some(Self::new(prim))
    }

    /// Returns true if this schema is valid.
    pub fn is_valid(&self) -> bool {
        self.field_asset.is_valid()
    }

    /// Get the wrapped prim.
    pub fn get_prim(&self) -> &Prim {
        self.field_asset.get_prim()
    }

    /// Access the underlying FieldAsset.
    pub fn field_asset(&self) -> &FieldAsset {
        &self.field_asset
    }

    // =========================================================================
    // FieldDataType Attribute (Override)
    // =========================================================================

    /// Get the fieldDataType attribute.
    ///
    /// For OpenVDB, allowed values are: half, float, double, int, uint, int64,
    /// half2, float2, double2, int2, half3, float3, double3, int3,
    /// matrix3d, matrix4d, quatd, bool, mask, string.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `token fieldDataType` |
    /// | C++ Type | TfToken |
    pub fn get_field_data_type_attr(&self) -> Option<Attribute> {
        self.get_prim()
            .get_attribute(USD_VOL_TOKENS.field_data_type.as_str())
    }

    /// Creates the fieldDataType attribute.
    pub fn create_field_data_type_attr(&self) -> Attribute {
        if !self.is_valid() {
            return Attribute::invalid();
        }

        if let Some(attr) = self
            .get_prim()
            .get_attribute(USD_VOL_TOKENS.field_data_type.as_str())
        {
            return attr;
        }

        let registry = ValueTypeRegistry::instance();
        let token_type = registry.find_type_by_token(&Token::new("token"));

        self.get_prim()
            .create_attribute(
                USD_VOL_TOKENS.field_data_type.as_str(),
                &token_type,
                false,
                Some(Variability::Varying),
            )
            .unwrap_or_else(Attribute::invalid)
    }

    // =========================================================================
    // FieldClass Attribute
    // =========================================================================

    /// Get the fieldClass attribute.
    ///
    /// Indicates the class of the OpenVDB grid (levelSet, fogVolume, etc.).
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `token fieldClass` |
    /// | C++ Type | TfToken |
    /// | Allowed Values | levelSet, fogVolume, staggered, unknown |
    pub fn get_field_class_attr(&self) -> Option<Attribute> {
        self.get_prim()
            .get_attribute(USD_VOL_TOKENS.field_class.as_str())
    }

    /// Creates the fieldClass attribute.
    pub fn create_field_class_attr(&self) -> Attribute {
        if !self.is_valid() {
            return Attribute::invalid();
        }

        if let Some(attr) = self
            .get_prim()
            .get_attribute(USD_VOL_TOKENS.field_class.as_str())
        {
            return attr;
        }

        let registry = ValueTypeRegistry::instance();
        let token_type = registry.find_type_by_token(&Token::new("token"));

        self.get_prim()
            .create_attribute(
                USD_VOL_TOKENS.field_class.as_str(),
                &token_type,
                false,
                Some(Variability::Varying),
            )
            .unwrap_or_else(Attribute::invalid)
    }

    // =========================================================================
    // Schema attribute names
    // =========================================================================

    /// Returns all pre-declared attributes for this schema class.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        let mut names = if include_inherited {
            FieldAsset::get_schema_attribute_names(true)
        } else {
            Vec::new()
        };

        names.extend([
            USD_VOL_TOKENS.field_data_type.clone(),
            USD_VOL_TOKENS.field_class.clone(),
        ]);

        names
    }
}

// Delegate to FieldAsset
impl std::ops::Deref for OpenVDBAsset {
    type Target = FieldAsset;

    fn deref(&self) -> &Self::Target {
        &self.field_asset
    }
}

impl From<Prim> for OpenVDBAsset {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

impl From<OpenVDBAsset> for Prim {
    fn from(asset: OpenVDBAsset) -> Self {
        asset.field_asset.into()
    }
}

impl AsRef<Prim> for OpenVDBAsset {
    fn as_ref(&self) -> &Prim {
        self.field_asset.get_prim()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_type_name() {
        assert_eq!(OpenVDBAsset::SCHEMA_TYPE_NAME, "OpenVDBAsset");
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = OpenVDBAsset::get_schema_attribute_names(false);
        assert!(names.iter().any(|n| n == "fieldDataType"));
        assert!(names.iter().any(|n| n == "fieldClass"));
    }
}
