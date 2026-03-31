#![allow(dead_code)]
//! MaterialBindingsSchema - Hydra schema for all material bindings.
//!
//! Port of pxr/usdImaging/usdImaging/materialBindingsSchema.h
//!
//! Provides data source schema for all material bindings declared on a prim.
//! The material binding purpose serves as the key, with the value being a vector
//! of MaterialBindingSchema elements to model the inheritance semantics of
//! UsdShadeMaterialBindingAPI.

use usd_hd::{HdContainerDataSourceHandle, HdDataSourceLocator, cast_to_container};
use usd_tf::Token;

// Token constants
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static USD_MATERIAL_BINDINGS: LazyLock<Token> =
        LazyLock::new(|| Token::new("usdMaterialBindings"));
    pub static ALL_PURPOSE: LazyLock<Token> = LazyLock::new(|| Token::new(""));
}

// ============================================================================
// MaterialBindingsSchema
// ============================================================================

/// Schema for all material bindings on a prim.
///
/// Specifies a container for all material bindings declared on a prim.
/// Material binding purpose serves as the key, with values being vectors
/// of MaterialBindingSchema for aggregating ancestor bindings to model
/// UsdShadeMaterialBindingAPI inheritance semantics.
#[derive(Debug, Clone)]
pub struct MaterialBindingsSchema {
    container: Option<HdContainerDataSourceHandle>,
}

impl MaterialBindingsSchema {
    /// Create schema from container.
    pub fn new(container: Option<HdContainerDataSourceHandle>) -> Self {
        Self { container }
    }

    /// Check if this schema is defined.
    pub fn is_defined(&self) -> bool {
        self.container.is_some()
    }

    /// Get the schema token.
    pub fn get_schema_token() -> Token {
        tokens::USD_MATERIAL_BINDINGS.clone()
    }

    /// Get the default locator for this schema.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_token(tokens::USD_MATERIAL_BINDINGS.clone())
    }

    /// Get schema from parent container.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Option<Self> {
        let ds = parent.get(&tokens::USD_MATERIAL_BINDINGS)?;
        let container = cast_to_container(&ds)?;
        Some(Self::new(Some(container)))
    }

    /// Returns the purposes for which bindings may be available.
    /// Note: Preferable to calling schema.container.get_names().
    pub fn get_purposes(&self) -> Vec<Token> {
        self.container
            .as_ref()
            .map_or_else(Vec::new, |container| container.get_names())
    }

    /// Returns the bindings for 'allPurpose' (empty string).
    pub fn get_material_bindings(&self) -> Option<HdContainerDataSourceHandle> {
        self.get_material_bindings_for_purpose(&tokens::ALL_PURPOSE)
    }

    /// Returns the bindings for the given purpose.
    pub fn get_material_bindings_for_purpose(
        &self,
        purpose: &Token,
    ) -> Option<HdContainerDataSourceHandle> {
        let container = self.container.as_ref()?;
        let ds = container.get(purpose)?;
        cast_to_container(&ds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_token() {
        assert_eq!(
            MaterialBindingsSchema::get_schema_token().as_str(),
            "usdMaterialBindings"
        );
    }

    #[test]
    fn test_all_purpose_token() {
        assert_eq!(tokens::ALL_PURPOSE.as_str(), "");
    }

    #[test]
    fn test_default_locator() {
        let locator = MaterialBindingsSchema::get_default_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_schema_is_defined() {
        let schema = MaterialBindingsSchema::new(None);
        assert!(!schema.is_defined());
    }

    #[test]
    fn test_get_purposes_empty() {
        let schema = MaterialBindingsSchema::new(None);
        assert_eq!(schema.get_purposes().len(), 0);
    }
}
