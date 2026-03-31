//! CollectionMaterialBindingSchema - Hydra schema for collection-based material bindings.
//!
//! Port of pxr/usdImaging/usdImaging/collectionMaterialBindingSchema.h
//!
//! Provides data source schema for collection-based material bindings in Hydra.
//! Represents material bindings applied via collections with binding strength.

use usd_hd::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdDataSourceLocator, cast_to_container,
};
use usd_tf::Token;

// Token constants
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static COLLECTION_MATERIAL_BINDING: LazyLock<Token> =
        LazyLock::new(|| Token::new("collectionMaterialBinding"));
    pub static COLLECTION_PRIM_PATH: LazyLock<Token> =
        LazyLock::new(|| Token::new("collectionPrimPath"));
    pub static COLLECTION_NAME: LazyLock<Token> = LazyLock::new(|| Token::new("collectionName"));
    pub static MATERIAL_PATH: LazyLock<Token> = LazyLock::new(|| Token::new("materialPath"));
    pub static BINDING_STRENGTH: LazyLock<Token> = LazyLock::new(|| Token::new("bindingStrength"));
}

// ============================================================================
// CollectionMaterialBindingSchema
// ============================================================================

/// Schema for collection-based material bindings in Hydra.
///
/// Represents material bindings applied via collections. Contains the
/// collection prim path, collection name, material path, and binding strength.
#[derive(Debug, Clone)]
pub struct CollectionMaterialBindingSchema {
    container: Option<HdContainerDataSourceHandle>,
}

impl CollectionMaterialBindingSchema {
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
        tokens::COLLECTION_MATERIAL_BINDING.clone()
    }

    /// Get the default locator for this schema.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_token(tokens::COLLECTION_MATERIAL_BINDING.clone())
    }

    /// Get the collection prim path locator.
    pub fn get_collection_prim_path_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::COLLECTION_MATERIAL_BINDING.clone(),
            tokens::COLLECTION_PRIM_PATH.clone(),
        )
    }

    /// Get the collection name locator.
    pub fn get_collection_name_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::COLLECTION_MATERIAL_BINDING.clone(),
            tokens::COLLECTION_NAME.clone(),
        )
    }

    /// Get the material path locator.
    pub fn get_material_path_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::COLLECTION_MATERIAL_BINDING.clone(),
            tokens::MATERIAL_PATH.clone(),
        )
    }

    /// Get the binding strength locator.
    pub fn get_binding_strength_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::COLLECTION_MATERIAL_BINDING.clone(),
            tokens::BINDING_STRENGTH.clone(),
        )
    }

    /// Get schema from parent container.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Option<Self> {
        let ds = parent.get(&tokens::COLLECTION_MATERIAL_BINDING)?;
        let container = cast_to_container(&ds)?;
        Some(Self {
            container: Some(container),
        })
    }
}

// ============================================================================
// CollectionMaterialBindingSchemaBuilder
// ============================================================================

/// Builder for CollectionMaterialBindingSchema data sources.
#[derive(Debug, Default)]
pub struct CollectionMaterialBindingSchemaBuilder {
    collection_prim_path: Option<HdDataSourceBaseHandle>,
    collection_name: Option<HdDataSourceBaseHandle>,
    material_path: Option<HdDataSourceBaseHandle>,
    binding_strength: Option<HdDataSourceBaseHandle>,
}

impl CollectionMaterialBindingSchemaBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the collection prim path data source.
    pub fn set_collection_prim_path(mut self, path: HdDataSourceBaseHandle) -> Self {
        self.collection_prim_path = Some(path);
        self
    }

    /// Set the collection name data source.
    pub fn set_collection_name(mut self, name: HdDataSourceBaseHandle) -> Self {
        self.collection_name = Some(name);
        self
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
        let mut entries: Vec<(Token, HdDataSourceBaseHandle)> = Vec::with_capacity(4);
        if let Some(v) = self.collection_prim_path {
            entries.push((tokens::COLLECTION_PRIM_PATH.clone(), v));
        }
        if let Some(v) = self.collection_name {
            entries.push((tokens::COLLECTION_NAME.clone(), v));
        }
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
            CollectionMaterialBindingSchema::get_schema_token().as_str(),
            "collectionMaterialBinding"
        );
    }

    #[test]
    fn test_collection_prim_path_locator() {
        let locator = CollectionMaterialBindingSchema::get_collection_prim_path_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_collection_name_locator() {
        let locator = CollectionMaterialBindingSchema::get_collection_name_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_material_path_locator() {
        let locator = CollectionMaterialBindingSchema::get_material_path_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_binding_strength_locator() {
        let locator = CollectionMaterialBindingSchema::get_binding_strength_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_builder() {
        let _schema = CollectionMaterialBindingSchemaBuilder::new().build();
    }

    #[test]
    fn test_is_defined() {
        let schema = CollectionMaterialBindingSchema::new(None);
        assert!(!schema.is_defined());
    }
}
