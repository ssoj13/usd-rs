//! Render Variable schema.
//!
//! A RenderVar describes a custom data variable for a render to produce.
//! The name of the RenderVar prim drives the name of the data variable.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdRender/var.h`

use std::sync::Arc;

use usd_core::attribute::Variability;
use usd_core::{Attribute, Prim, Stage};
use usd_sdf::{Path, ValueTypeRegistry};
use usd_tf::Token;

use super::tokens::USD_RENDER_TOKENS;

/// Render variable schema.
///
/// Describes a custom data variable for a render to produce.
#[derive(Debug, Clone)]
pub struct RenderVar {
    prim: Prim,
}

impl RenderVar {
    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "RenderVar";

    /// Construct a RenderVar on the given prim.
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Construct from another prim.
    pub fn from_prim(prim: &Prim) -> Self {
        Self::new(prim.clone())
    }

    /// Return a RenderVar holding the prim at `path` on `stage`.
    ///
    /// Matches C++ `UsdRenderVar::Get()` — no type check performed.
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage.get_prim_at_path(path)?;
        Some(Self::new(prim))
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
        self.prim.is_valid()
    }

    /// Get the wrapped prim.
    pub fn get_prim(&self) -> &Prim {
        &self.prim
    }

    // =========================================================================
    // DataType Attribute
    // =========================================================================

    /// Get the dataType attribute.
    pub fn get_data_type_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_RENDER_TOKENS.data_type.as_str())
    }

    /// Creates the dataType attribute.
    pub fn create_data_type_attr(&self, _default_value: Option<Token>) -> Attribute {
        if !self.prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let token_type = registry.find_type_by_token(&Token::new("token"));

        let attr = self
            .prim
            .create_attribute(
                USD_RENDER_TOKENS.data_type.as_str(),
                &token_type,
                false,
                Some(Variability::Uniform),
            )
            .unwrap_or_else(Attribute::invalid);

        // Note: Default value setting omitted - use attr.set() with TimeCode if needed
        attr
    }

    // =========================================================================
    // SourceName Attribute
    // =========================================================================

    /// Get the sourceName attribute.
    pub fn get_source_name_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_RENDER_TOKENS.source_name.as_str())
    }

    /// Creates the sourceName attribute.
    pub fn create_source_name_attr(&self, _default_value: Option<String>) -> Attribute {
        if !self.prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let string_type = registry.find_type_by_token(&Token::new("string"));

        let attr = self
            .prim
            .create_attribute(
                USD_RENDER_TOKENS.source_name.as_str(),
                &string_type,
                false,
                Some(Variability::Uniform),
            )
            .unwrap_or_else(Attribute::invalid);

        // Note: Default value setting omitted - use attr.set() with TimeCode if needed
        attr
    }

    // =========================================================================
    // SourceType Attribute
    // =========================================================================

    /// Get the sourceType attribute.
    pub fn get_source_type_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_RENDER_TOKENS.source_type.as_str())
    }

    /// Creates the sourceType attribute.
    pub fn create_source_type_attr(&self, _default_value: Option<Token>) -> Attribute {
        if !self.prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let token_type = registry.find_type_by_token(&Token::new("token"));

        let attr = self
            .prim
            .create_attribute(
                USD_RENDER_TOKENS.source_type.as_str(),
                &token_type,
                false,
                Some(Variability::Uniform),
            )
            .unwrap_or_else(Attribute::invalid);

        // Note: Default value setting omitted - use attr.set() with TimeCode if needed
        attr
    }

    // =========================================================================
    // Schema attribute names
    // =========================================================================

    /// Returns all pre-declared attributes for this schema class.
    ///
    /// Inherits from UsdTyped (no attributes), so inherited == local.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        let local = vec![
            USD_RENDER_TOKENS.data_type.clone(),
            USD_RENDER_TOKENS.source_name.clone(),
            USD_RENDER_TOKENS.source_type.clone(),
        ];
        if include_inherited {
            // UsdTyped has no attributes, so allNames == localNames
            local
        } else {
            local
        }
    }
}

impl From<Prim> for RenderVar {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

impl From<RenderVar> for Prim {
    fn from(var: RenderVar) -> Self {
        var.prim
    }
}

impl AsRef<Prim> for RenderVar {
    fn as_ref(&self) -> &Prim {
        &self.prim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_type_name() {
        assert_eq!(RenderVar::SCHEMA_TYPE_NAME, "RenderVar");
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = RenderVar::get_schema_attribute_names(false);
        assert!(names.iter().any(|n| n.get_text() == "dataType"));
        assert!(names.iter().any(|n| n.get_text() == "sourceName"));
        assert!(names.iter().any(|n| n.get_text() == "sourceType"));
    }
}
