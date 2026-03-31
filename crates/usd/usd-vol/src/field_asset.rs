//! Field Asset schema.
//!
//! Abstract base class for field primitives defined by an external file.
//! Subclasses specify the file format (OpenVDB, Field3D, etc.).
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdVol/fieldAsset.h`

use std::sync::Arc;

use usd_core::attribute::Variability;
use usd_core::{Attribute, Prim, Stage};
use usd_sdf::{Path, ValueTypeRegistry};
use usd_tf::Token;

use super::field_base::FieldBase;
use super::tokens::USD_VOL_TOKENS;

/// Abstract base class for asset-backed field primitives.
///
/// FieldAsset provides common attributes for fields defined by external files:
/// - `filePath` - Path to the volume file on disk
/// - `fieldName` - Name of the field within the file
/// - `fieldIndex` - Index to disambiguate fields with same name
/// - `fieldDataType` - Data type token (float, double, etc.)
/// - `vectorDataRoleHint` - Role hint for vector fields
///
/// # Schema Kind
///
/// This is an abstract typed schema (AbstractTyped).
#[derive(Debug, Clone)]
pub struct FieldAsset {
    field_base: FieldBase,
}

impl FieldAsset {
    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "FieldAsset";

    /// Construct a FieldAsset on the given prim.
    pub fn new(prim: Prim) -> Self {
        Self {
            field_base: FieldBase::new(prim),
        }
    }

    /// Construct from another prim.
    pub fn from_prim(prim: &Prim) -> Self {
        Self::new(prim.clone())
    }

    /// Return a FieldAsset holding the prim at `path` on `stage`.
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage.get_prim_at_path(path)?;
        if prim.is_a(&USD_VOL_TOKENS.field_asset) {
            Some(Self::new(prim))
        } else {
            None
        }
    }

    /// Returns true if this schema is valid.
    pub fn is_valid(&self) -> bool {
        self.field_base.is_valid()
    }

    /// Get the wrapped prim.
    pub fn get_prim(&self) -> &Prim {
        self.field_base.get_prim()
    }

    /// Access the underlying FieldBase.
    pub fn field_base(&self) -> &FieldBase {
        &self.field_base
    }

    // =========================================================================
    // FilePath Attribute
    // =========================================================================

    /// Get the filePath attribute.
    ///
    /// An asset path pointing to a volume file on disk.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `asset filePath` |
    /// | C++ Type | SdfAssetPath |
    pub fn get_file_path_attr(&self) -> Option<Attribute> {
        self.get_prim()
            .get_attribute(USD_VOL_TOKENS.file_path.as_str())
    }

    /// Creates the filePath attribute.
    pub fn create_file_path_attr(&self) -> Attribute {
        if !self.is_valid() {
            return Attribute::invalid();
        }

        if let Some(attr) = self
            .get_prim()
            .get_attribute(USD_VOL_TOKENS.file_path.as_str())
        {
            return attr;
        }

        let registry = ValueTypeRegistry::instance();
        let asset_type = registry.find_type_by_token(&Token::new("asset"));

        self.get_prim()
            .create_attribute(
                USD_VOL_TOKENS.file_path.as_str(),
                &asset_type,
                false,
                Some(Variability::Varying),
            )
            .unwrap_or_else(Attribute::invalid)
    }

    // =========================================================================
    // FieldName Attribute
    // =========================================================================

    /// Get the fieldName attribute.
    ///
    /// Name of an individual field within the file.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `token fieldName` |
    /// | C++ Type | TfToken |
    pub fn get_field_name_attr(&self) -> Option<Attribute> {
        self.get_prim()
            .get_attribute(USD_VOL_TOKENS.field_name.as_str())
    }

    /// Creates the fieldName attribute.
    pub fn create_field_name_attr(&self) -> Attribute {
        if !self.is_valid() {
            return Attribute::invalid();
        }

        if let Some(attr) = self
            .get_prim()
            .get_attribute(USD_VOL_TOKENS.field_name.as_str())
        {
            return attr;
        }

        let registry = ValueTypeRegistry::instance();
        let token_type = registry.find_type_by_token(&Token::new("token"));

        self.get_prim()
            .create_attribute(
                USD_VOL_TOKENS.field_name.as_str(),
                &token_type,
                false,
                Some(Variability::Varying),
            )
            .unwrap_or_else(Attribute::invalid)
    }

    // =========================================================================
    // FieldIndex Attribute
    // =========================================================================

    /// Get the fieldIndex attribute.
    ///
    /// Index to disambiguate between fields with the same name.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `int fieldIndex` |
    /// | C++ Type | int |
    pub fn get_field_index_attr(&self) -> Option<Attribute> {
        self.get_prim()
            .get_attribute(USD_VOL_TOKENS.field_index.as_str())
    }

    /// Creates the fieldIndex attribute.
    pub fn create_field_index_attr(&self) -> Attribute {
        if !self.is_valid() {
            return Attribute::invalid();
        }

        if let Some(attr) = self
            .get_prim()
            .get_attribute(USD_VOL_TOKENS.field_index.as_str())
        {
            return attr;
        }

        let registry = ValueTypeRegistry::instance();
        let int_type = registry.find_type_by_token(&Token::new("int"));

        self.get_prim()
            .create_attribute(
                USD_VOL_TOKENS.field_index.as_str(),
                &int_type,
                false,
                Some(Variability::Varying),
            )
            .unwrap_or_else(Attribute::invalid)
    }

    // =========================================================================
    // FieldDataType Attribute
    // =========================================================================

    /// Get the fieldDataType attribute.
    ///
    /// Token indicating the data type of the field (float, double, etc.).
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
    // VectorDataRoleHint Attribute
    // =========================================================================

    /// Get the vectorDataRoleHint attribute.
    ///
    /// Optional token for vector field roles (Point, Normal, Vector, Color).
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `token vectorDataRoleHint = "None"` |
    /// | C++ Type | TfToken |
    /// | Allowed Values | None, Point, Normal, Vector, Color |
    pub fn get_vector_data_role_hint_attr(&self) -> Option<Attribute> {
        self.get_prim()
            .get_attribute(USD_VOL_TOKENS.vector_data_role_hint.as_str())
    }

    /// Creates the vectorDataRoleHint attribute.
    pub fn create_vector_data_role_hint_attr(&self) -> Attribute {
        if !self.is_valid() {
            return Attribute::invalid();
        }

        if let Some(attr) = self
            .get_prim()
            .get_attribute(USD_VOL_TOKENS.vector_data_role_hint.as_str())
        {
            return attr;
        }

        let registry = ValueTypeRegistry::instance();
        let token_type = registry.find_type_by_token(&Token::new("token"));

        // C++ uses SdfVariabilityVarying for vectorDataRoleHint
        self.get_prim()
            .create_attribute(
                USD_VOL_TOKENS.vector_data_role_hint.as_str(),
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
            FieldBase::get_schema_attribute_names(true)
        } else {
            Vec::new()
        };

        names.extend([
            USD_VOL_TOKENS.file_path.clone(),
            USD_VOL_TOKENS.field_name.clone(),
            USD_VOL_TOKENS.field_index.clone(),
            USD_VOL_TOKENS.field_data_type.clone(),
            USD_VOL_TOKENS.vector_data_role_hint.clone(),
        ]);

        names
    }
}

// Delegate to FieldBase
impl std::ops::Deref for FieldAsset {
    type Target = FieldBase;

    fn deref(&self) -> &Self::Target {
        &self.field_base
    }
}

impl From<Prim> for FieldAsset {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

impl From<FieldAsset> for Prim {
    fn from(asset: FieldAsset) -> Self {
        asset.field_base.into()
    }
}

impl AsRef<Prim> for FieldAsset {
    fn as_ref(&self) -> &Prim {
        self.field_base.get_prim()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_type_name() {
        assert_eq!(FieldAsset::SCHEMA_TYPE_NAME, "FieldAsset");
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = FieldAsset::get_schema_attribute_names(false);
        assert!(names.iter().any(|n| n == "filePath"));
        assert!(names.iter().any(|n| n == "fieldName"));
        assert!(names.iter().any(|n| n == "fieldDataType"));
    }
}
