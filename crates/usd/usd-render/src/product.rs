//! Render Product schema.
//!
//! A RenderProduct describes an image or other file-like artifact produced
//! by a render.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdRender/product.h`

use std::sync::Arc;

use usd_core::attribute::Variability;
use usd_core::{Attribute, Prim, Relationship, Stage};
use usd_sdf::{Path, ValueTypeRegistry};
use usd_tf::Token;
use usd_vt::Value;

use super::settings_base::RenderSettingsBase;
use super::tokens::USD_RENDER_TOKENS;

/// Render product schema.
#[derive(Debug, Clone)]
pub struct RenderProduct {
    prim: Prim,
}

impl RenderProduct {
    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "RenderProduct";

    /// Construct a RenderProduct on the given prim.
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Construct from another prim.
    pub fn from_prim(prim: &Prim) -> Self {
        Self::new(prim.clone())
    }

    /// Return a RenderProduct holding the prim at `path` on `stage`.
    ///
    /// Matches C++ `UsdRenderProduct::Get()` — no type check performed.
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

    /// Get as base class for accessing inherited attributes.
    pub fn as_settings_base(&self) -> RenderSettingsBase {
        RenderSettingsBase::new(self.prim.clone())
    }

    // =========================================================================
    // ProductType Attribute
    // =========================================================================

    /// Get the productType attribute.
    pub fn get_product_type_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_RENDER_TOKENS.product_type.as_str())
    }

    /// Creates the productType attribute.
    ///
    /// Matches C++ `CreateProductTypeAttr(VtValue, bool)`.
    pub fn create_product_type_attr(
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
                USD_RENDER_TOKENS.product_type.as_str(),
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
    // ProductName Attribute
    // =========================================================================

    /// Get the productName attribute.
    pub fn get_product_name_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_RENDER_TOKENS.product_name.as_str())
    }

    /// Creates the productName attribute.
    ///
    /// Matches C++ `CreateProductNameAttr(VtValue, bool)`.
    pub fn create_product_name_attr(
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
                USD_RENDER_TOKENS.product_name.as_str(),
                &token_type,
                false,
                Some(Variability::Varying),
            )
            .unwrap_or_else(Attribute::invalid);

        if let Some(val) = default_value {
            attr.set(val.clone(), usd_sdf::TimeCode::default());
        }

        attr
    }

    // =========================================================================
    // OrderedVars Relationship
    // =========================================================================

    /// Get the orderedVars relationship.
    pub fn get_ordered_vars_rel(&self) -> Option<Relationship> {
        self.prim
            .get_relationship(USD_RENDER_TOKENS.ordered_vars.as_str())
    }

    /// Creates the orderedVars relationship.
    pub fn create_ordered_vars_rel(&self) -> Option<Relationship> {
        self.prim
            .create_relationship(USD_RENDER_TOKENS.ordered_vars.as_str(), false)
    }

    // =========================================================================
    // Schema attribute names
    // =========================================================================

    /// Returns all pre-declared attributes for this schema class.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        let mut names = if include_inherited {
            RenderSettingsBase::get_schema_attribute_names(true)
        } else {
            Vec::new()
        };

        names.extend([
            USD_RENDER_TOKENS.product_type.clone(),
            USD_RENDER_TOKENS.product_name.clone(),
        ]);

        names
    }
}

impl From<Prim> for RenderProduct {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

impl From<RenderProduct> for Prim {
    fn from(product: RenderProduct) -> Self {
        product.prim
    }
}

impl AsRef<Prim> for RenderProduct {
    fn as_ref(&self) -> &Prim {
        &self.prim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_type_name() {
        assert_eq!(RenderProduct::SCHEMA_TYPE_NAME, "RenderProduct");
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = RenderProduct::get_schema_attribute_names(false);
        assert!(names.iter().any(|n| n.get_text() == "productType"));
    }
}
