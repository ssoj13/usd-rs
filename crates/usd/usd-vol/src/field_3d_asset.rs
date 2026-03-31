//! Field3D Asset schema.
//!
//! Concrete field primitive for Field3D format files.
//! The filePath attribute must specify a file in Field3D format.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdVol/field3DAsset.h`

use std::sync::Arc;

use usd_core::attribute::Variability;
use usd_core::{Attribute, Prim, Stage};
use usd_sdf::{Path, ValueTypeRegistry};
use usd_tf::Token;

use super::field_asset::FieldAsset;
use super::tokens::USD_VOL_TOKENS;

/// Field3D field primitive.
///
/// The FieldAsset filePath attribute must specify a file in Field3D format.
///
/// # Additional Attributes
///
/// - `fieldDataType` - Override with Field3D-specific allowed values
/// - `fieldPurpose` - Purpose/grouping of the field (maps to Field3D field name)
///
/// # Schema Kind
///
/// This is a concrete typed schema (ConcreteTyped).
#[derive(Debug, Clone)]
pub struct Field3DAsset {
    field_asset: FieldAsset,
}

impl Field3DAsset {
    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "Field3DAsset";

    /// Construct a Field3DAsset on the given prim.
    pub fn new(prim: Prim) -> Self {
        Self {
            field_asset: FieldAsset::new(prim),
        }
    }

    /// Construct from another prim.
    pub fn from_prim(prim: &Prim) -> Self {
        Self::new(prim.clone())
    }

    /// Return a Field3DAsset holding the prim at `path` on `stage`.
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage.get_prim_at_path(path)?;
        if prim.is_a(&USD_VOL_TOKENS.field_3d_asset) {
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
    /// For Field3D, allowed values are: half, float, double, half3, float3, double3.
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
    // FieldPurpose Attribute
    // =========================================================================

    /// Get the fieldPurpose attribute.
    ///
    /// Purpose or grouping of the field. Clients should treat this as the
    /// Field3D field name.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `token fieldPurpose` |
    /// | C++ Type | TfToken |
    pub fn get_field_purpose_attr(&self) -> Option<Attribute> {
        self.get_prim()
            .get_attribute(USD_VOL_TOKENS.field_purpose.as_str())
    }

    /// Creates the fieldPurpose attribute.
    pub fn create_field_purpose_attr(&self) -> Attribute {
        if !self.is_valid() {
            return Attribute::invalid();
        }

        if let Some(attr) = self
            .get_prim()
            .get_attribute(USD_VOL_TOKENS.field_purpose.as_str())
        {
            return attr;
        }

        let registry = ValueTypeRegistry::instance();
        let token_type = registry.find_type_by_token(&Token::new("token"));

        self.get_prim()
            .create_attribute(
                USD_VOL_TOKENS.field_purpose.as_str(),
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
            USD_VOL_TOKENS.field_purpose.clone(),
        ]);

        names
    }
}

// Delegate to FieldAsset
impl std::ops::Deref for Field3DAsset {
    type Target = FieldAsset;

    fn deref(&self) -> &Self::Target {
        &self.field_asset
    }
}

impl From<Prim> for Field3DAsset {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

impl From<Field3DAsset> for Prim {
    fn from(asset: Field3DAsset) -> Self {
        asset.field_asset.into()
    }
}

impl AsRef<Prim> for Field3DAsset {
    fn as_ref(&self) -> &Prim {
        self.field_asset.get_prim()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_type_name() {
        assert_eq!(Field3DAsset::SCHEMA_TYPE_NAME, "Field3DAsset");
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = Field3DAsset::get_schema_attribute_names(false);
        assert!(names.iter().any(|n| n == "fieldDataType"));
        assert!(names.iter().any(|n| n == "fieldPurpose"));
    }
}
