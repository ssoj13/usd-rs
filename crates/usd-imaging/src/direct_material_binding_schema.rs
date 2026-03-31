//! DirectMaterialBindingSchema - Hydra schema for direct material bindings.
//!
//! Port of pxr/usdImaging/usdImaging/directMaterialBindingSchema.h
//!
//! Provides data source schema for direct material bindings in Hydra.
//! Represents material bindings applied directly to prims without collections.

use usd_hd::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdDataSourceLocator, cast_to_container,
};
use usd_tf::Token;

// Token constants
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static DIRECT_MATERIAL_BINDING: LazyLock<Token> =
        LazyLock::new(|| Token::new("directMaterialBinding"));
    pub static MATERIAL_PATH: LazyLock<Token> = LazyLock::new(|| Token::new("materialPath"));
    pub static BINDING_STRENGTH: LazyLock<Token> = LazyLock::new(|| Token::new("bindingStrength"));
}

// ============================================================================
// DirectMaterialBindingSchema
// ============================================================================

/// Schema for direct material bindings in Hydra.
///
/// Represents material bindings applied directly to prims without using
/// collections. Contains the material path and binding strength.
#[derive(Debug, Clone)]
pub struct DirectMaterialBindingSchema {
    container: Option<HdContainerDataSourceHandle>,
}

impl DirectMaterialBindingSchema {
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
        tokens::DIRECT_MATERIAL_BINDING.clone()
    }

    /// Get the default locator for this schema.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_token(tokens::DIRECT_MATERIAL_BINDING.clone())
    }

    /// Get the material path locator.
    pub fn get_material_path_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::DIRECT_MATERIAL_BINDING.clone(),
            tokens::MATERIAL_PATH.clone(),
        )
    }

    /// Get the binding strength locator.
    pub fn get_binding_strength_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::DIRECT_MATERIAL_BINDING.clone(),
            tokens::BINDING_STRENGTH.clone(),
        )
    }

    /// Get schema from parent container.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Option<Self> {
        let ds = parent.get(&tokens::DIRECT_MATERIAL_BINDING)?;
        let container = cast_to_container(&ds)?;
        Some(Self {
            container: Some(container),
        })
    }
}

// ============================================================================
// DirectMaterialBindingSchemaBuilder
// ============================================================================

/// Builder for DirectMaterialBindingSchema data sources.
#[derive(Debug, Default)]
pub struct DirectMaterialBindingSchemaBuilder {
    material_path: Option<HdDataSourceBaseHandle>,
    binding_strength: Option<HdDataSourceBaseHandle>,
}

impl DirectMaterialBindingSchemaBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the material path data source.
    pub fn set_material_path(mut self, path: HdDataSourceBaseHandle) -> Self {
        self.material_path = Some(path);
        self
    }

    /// Set the binding strength data source.
    pub fn set_binding_strength(mut self, strength: HdDataSourceBaseHandle) -> Self {
        self.binding_strength = Some(strength);
        self
    }

    /// Build the container data source from set fields.
    ///
    /// Matches C++ BuildRetained: only includes non-None fields.
    pub fn build(self) -> HdContainerDataSourceHandle {
        let mut entries: Vec<(Token, HdDataSourceBaseHandle)> = Vec::with_capacity(2);
        if let Some(v) = self.material_path {
            entries.push((tokens::MATERIAL_PATH.clone(), v));
        }
        if let Some(v) = self.binding_strength {
            entries.push((tokens::BINDING_STRENGTH.clone(), v));
        }
        usd_hd::HdRetainedContainerDataSource::from_entries(&entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_token() {
        assert_eq!(
            DirectMaterialBindingSchema::get_schema_token().as_str(),
            "directMaterialBinding"
        );
    }

    #[test]
    fn test_material_path_locator() {
        let locator = DirectMaterialBindingSchema::get_material_path_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_binding_strength_locator() {
        let locator = DirectMaterialBindingSchema::get_binding_strength_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_builder() {
        let _schema = DirectMaterialBindingSchemaBuilder::new().build();
    }

    #[test]
    fn test_is_defined() {
        let schema = DirectMaterialBindingSchema::new(None);
        assert!(!schema.is_defined());
    }
}
