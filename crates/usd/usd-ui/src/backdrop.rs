//! Backdrop schema.
//!
//! Provides a visual 'group-box' for node graph organization.
//! Backdrops are organizational and don't affect rendering.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdUI/backdrop.h`

use std::sync::Arc;

use usd_core::attribute::Variability;
use usd_core::{Attribute, Prim, Stage};
use usd_sdf::{Path, ValueTypeRegistry};
use usd_tf::Token;
use usd_vt::Value;

use super::tokens::USD_UI_TOKENS;

/// Visual backdrop for node graph organization.
///
/// Unlike containers, backdrops do not store nodes inside them.
/// A node is considered part of a backdrop when its bounding box
/// fits inside the backdrop.
///
/// # Schema Kind
///
/// This is a concrete typed schema (ConcreteTyped).
///
/// # Attributes
///
/// - `ui:description` - Text label displayed on the backdrop
#[derive(Debug, Clone)]
pub struct Backdrop {
    prim: Prim,
}

impl Backdrop {
    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "Backdrop";

    /// Construct a Backdrop on the given prim.
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Construct from another prim.
    pub fn from_prim(prim: &Prim) -> Self {
        Self::new(prim.clone())
    }

    /// Return a Backdrop holding the prim at `path` on `stage`.
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage.get_prim_at_path(path)?;
        if prim.is_a(&USD_UI_TOKENS.backdrop) {
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
        self.prim.is_valid()
    }

    /// Get the wrapped prim.
    pub fn get_prim(&self) -> &Prim {
        &self.prim
    }

    // =========================================================================
    // Description Attribute
    // =========================================================================

    /// Get the ui:description attribute.
    ///
    /// The text label displayed on the backdrop.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `uniform token ui:description` |
    /// | C++ Type | TfToken |
    pub fn get_description_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_UI_TOKENS.ui_description.as_str())
    }

    /// Creates the ui:description attribute.
    pub fn create_description_attr(
        &self,
        default_value: Option<&Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        if !self.prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let token_type = registry.find_type_by_token(&Token::new("token"));

        let attr = self
            .prim
            .create_attribute(
                USD_UI_TOKENS.ui_description.as_str(),
                &token_type,
                false,
                Some(Variability::Uniform),
            )
            .unwrap_or_else(Attribute::invalid);
        if let Some(val) = default_value {
            attr.set(val.clone(), usd_sdf::TimeCode::default());
        }
        attr
    }

    // =========================================================================
    // Schema attribute names
    // =========================================================================

    /// Returns all pre-declared attributes for this schema class.
    pub fn get_schema_attribute_names(_include_inherited: bool) -> Vec<Token> {
        vec![USD_UI_TOKENS.ui_description.clone()]
    }
}

impl From<Prim> for Backdrop {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

impl From<Backdrop> for Prim {
    fn from(backdrop: Backdrop) -> Self {
        backdrop.prim
    }
}

impl AsRef<Prim> for Backdrop {
    fn as_ref(&self) -> &Prim {
        &self.prim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_type_name() {
        assert_eq!(Backdrop::SCHEMA_TYPE_NAME, "Backdrop");
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = Backdrop::get_schema_attribute_names(false);
        assert!(names.iter().any(|n| n == "ui:description"));
    }
}
