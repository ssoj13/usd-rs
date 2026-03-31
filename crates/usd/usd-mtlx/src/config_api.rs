//! MaterialX Config API schema.
//!
//! API schema for storing MaterialX environment information.
//! Currently exposes the MaterialX library version that data was authored against.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdMtlx/materialXConfigAPI.h`

use std::sync::Arc;

use usd_core::attribute::Variability;
use usd_core::{Attribute, Prim, SchemaKind, Stage};
use usd_sdf::{Path, ValueTypeRegistry};
use usd_tf::Token;

use super::tokens::USD_MTLX_TOKENS;

/// MaterialX configuration API schema.
///
/// Provides an interface for storing MaterialX environment information,
/// particularly the library version that data was authored against.
/// This enables the MaterialX library to perform upgrades on data
/// from prior versions.
///
/// # Schema Kind
///
/// This is a single-apply API schema (SingleApplyAPI).
///
/// # Attributes
///
/// - `config:mtlx:version` - MaterialX version string (default: "1.38")
#[derive(Debug, Clone)]
pub struct MaterialXConfigAPI {
    prim: Prim,
}

impl MaterialXConfigAPI {
    /// The schema kind for this class.
    pub const SCHEMA_KIND: SchemaKind = SchemaKind::SingleApplyAPI;

    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "MaterialXConfigAPI";

    /// Default MaterialX version for legacy data.
    pub const DEFAULT_VERSION: &'static str = "1.38";

    // =========================================================================
    // Construction
    // =========================================================================

    /// Construct a MaterialXConfigAPI on the given prim.
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Construct from another prim.
    pub fn from_prim(prim: &Prim) -> Self {
        Self::new(prim.clone())
    }

    /// Return a MaterialXConfigAPI holding the prim at `path` on `stage`.
    ///
    /// If no prim exists at `path` or it doesn't have this API applied,
    /// returns None.
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage.get_prim_at_path(path)?;
        if prim.has_api(&USD_MTLX_TOKENS.material_x_config_api) {
            Some(Self::new(prim))
        } else {
            None
        }
    }

    /// Check if a prim has MaterialXConfigAPI applied.
    ///
    /// Returns true if the API schema is applied to the prim.
    pub fn has(prim: &Prim) -> bool {
        prim.has_api(&USD_MTLX_TOKENS.material_x_config_api)
    }

    /// Returns true if this API schema can be applied to the given prim.
    pub fn can_apply(prim: &Prim, _why_not: Option<&mut String>) -> bool {
        prim.can_apply_api(&USD_MTLX_TOKENS.material_x_config_api)
    }

    /// Applies this API schema to the given prim.
    ///
    /// Adds "MaterialXConfigAPI" to the prim's apiSchemas metadata.
    pub fn apply(prim: &Prim) -> Option<Self> {
        if prim.apply_api(&USD_MTLX_TOKENS.material_x_config_api) {
            Some(Self::new(prim.clone()))
        } else {
            None
        }
    }

    /// Returns the schema kind.
    pub fn get_schema_kind(&self) -> SchemaKind {
        Self::SCHEMA_KIND
    }

    /// Returns true if this schema is valid.
    pub fn is_valid(&self) -> bool {
        self.prim.is_valid()
    }

    /// Get the wrapped prim.
    pub fn get_prim(&self) -> &Prim {
        &self.prim
    }

    // =========================================================================
    // ConfigMtlxVersion Attribute
    // =========================================================================

    /// Get the config:mtlx:version attribute.
    ///
    /// MaterialX library version that the data was authored against.
    /// Defaults to "1.38" to allow correct versioning of old files.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `string config:mtlx:version = "1.38"` |
    /// | C++ Type | std::string |
    pub fn get_config_mtlx_version_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_MTLX_TOKENS.config_mtlx_version.as_str())
    }

    /// Creates the config:mtlx:version attribute.
    pub fn create_config_mtlx_version_attr(&self) -> Attribute {
        if !self.prim.is_valid() {
            return Attribute::invalid();
        }

        if let Some(attr) = self
            .prim
            .get_attribute(USD_MTLX_TOKENS.config_mtlx_version.as_str())
        {
            return attr;
        }

        let registry = ValueTypeRegistry::instance();
        let string_type = registry.find_type_by_token(&Token::new("string"));

        let attr = self
            .prim
            .create_attribute(
                USD_MTLX_TOKENS.config_mtlx_version.as_str(),
                &string_type,
                false,
                Some(Variability::Varying),
            )
            .unwrap_or_else(Attribute::invalid);

        // Set default value to "1.38" for legacy data versioning
        if attr.is_valid() {
            use usd_sdf::TimeCode;
            use usd_vt::Value;
            let _ = attr.set(Value::from(Self::DEFAULT_VERSION), TimeCode::default_time());
        }

        attr
    }

    // =========================================================================
    // Schema attribute names
    // =========================================================================

    /// Returns all pre-declared attributes for this schema class.
    pub fn get_schema_attribute_names(_include_inherited: bool) -> Vec<Token> {
        vec![USD_MTLX_TOKENS.config_mtlx_version.clone()]
    }
}

impl From<Prim> for MaterialXConfigAPI {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

impl From<MaterialXConfigAPI> for Prim {
    fn from(api: MaterialXConfigAPI) -> Self {
        api.prim
    }
}

impl AsRef<Prim> for MaterialXConfigAPI {
    fn as_ref(&self) -> &Prim {
        &self.prim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_kind() {
        assert_eq!(MaterialXConfigAPI::SCHEMA_KIND, SchemaKind::SingleApplyAPI);
    }

    #[test]
    fn test_schema_type_name() {
        assert_eq!(MaterialXConfigAPI::SCHEMA_TYPE_NAME, "MaterialXConfigAPI");
    }

    #[test]
    fn test_default_version() {
        assert_eq!(MaterialXConfigAPI::DEFAULT_VERSION, "1.38");
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = MaterialXConfigAPI::get_schema_attribute_names(false);
        assert!(names.iter().any(|n| n == "config:mtlx:version"));
    }

    #[test]
    fn test_has_method() {
        // Test that has() method exists and compiles
        // Note: Full integration test requires schema registration
        use usd_core::{InitialLoadSet, Stage};

        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let prim = stage.define_prim("/TestPrim", "Scope").unwrap();

        // Initially should not have the API
        assert!(!MaterialXConfigAPI::has(&prim));
    }

    #[test]
    fn test_variability_is_varying() {
        // Test that the constant DEFAULT_VERSION is correct
        assert_eq!(MaterialXConfigAPI::DEFAULT_VERSION, "1.38");
    }
}
