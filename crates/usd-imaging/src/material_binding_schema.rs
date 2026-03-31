//! MaterialBindingSchema - Hydra schema for single material binding.
//!
//! Port of pxr/usdImaging/usdImaging/materialBindingSchema.h
//!
//! Provides data source schema for a prim's material binding for a particular purpose.
//! Note that only one direct binding but any number of collection-based bindings may
//! be declared for a given purpose.

use usd_hd::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdDataSourceLocator,
    HdVectorDataSourceHandle, cast_to_container, cast_to_vector,
};
use usd_tf::Token;

// Token constants
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static USD_MATERIAL_BINDING: LazyLock<Token> =
        LazyLock::new(|| Token::new("usdMaterialBinding"));
    pub static DIRECT_MATERIAL_BINDING: LazyLock<Token> =
        LazyLock::new(|| Token::new("directMaterialBinding"));
    pub static COLLECTION_MATERIAL_BINDINGS: LazyLock<Token> =
        LazyLock::new(|| Token::new("collectionMaterialBindings"));
}

// ============================================================================
// MaterialBindingSchema
// ============================================================================

/// Schema for a single material binding per purpose.
///
/// Specifies a container for a prim's material bindings for a particular purpose.
/// Contains one direct binding and any number of collection-based bindings.
#[derive(Debug, Clone)]
pub struct MaterialBindingSchema {
    container: Option<HdContainerDataSourceHandle>,
}

impl MaterialBindingSchema {
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
        tokens::USD_MATERIAL_BINDING.clone()
    }

    /// Get the default locator for this schema.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_token(tokens::USD_MATERIAL_BINDING.clone())
    }

    /// Get the direct material binding locator.
    pub fn get_direct_material_binding_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::USD_MATERIAL_BINDING.clone(),
            tokens::DIRECT_MATERIAL_BINDING.clone(),
        )
    }

    /// Get the collection material bindings locator.
    pub fn get_collection_material_bindings_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::USD_MATERIAL_BINDING.clone(),
            tokens::COLLECTION_MATERIAL_BINDINGS.clone(),
        )
    }

    /// Get schema from parent container.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Option<Self> {
        let ds = parent.get(&tokens::USD_MATERIAL_BINDING)?;
        let container = cast_to_container(&ds)?;
        Some(Self {
            container: Some(container),
        })
    }

    /// Get the direct material binding container.
    pub fn get_direct_material_binding(&self) -> Option<HdContainerDataSourceHandle> {
        let container = self.container.as_ref()?;
        let ds = container.get(&tokens::DIRECT_MATERIAL_BINDING)?;
        cast_to_container(&ds)
    }

    /// Get the collection material bindings vector.
    pub fn get_collection_material_bindings(&self) -> Option<HdVectorDataSourceHandle> {
        let container = self.container.as_ref()?;
        let ds = container.get(&tokens::COLLECTION_MATERIAL_BINDINGS)?;
        cast_to_vector(&ds)
    }
}

// ============================================================================
// MaterialBindingSchemaBuilder
// ============================================================================

/// Builder for MaterialBindingSchema data sources.
#[derive(Debug, Default)]
pub struct MaterialBindingSchemaBuilder {
    direct_material_binding: Option<HdDataSourceBaseHandle>,
    collection_material_bindings: Option<HdDataSourceBaseHandle>,
}

impl MaterialBindingSchemaBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the direct material binding data source.
    pub fn set_direct_material_binding(mut self, binding: HdDataSourceBaseHandle) -> Self {
        self.direct_material_binding = Some(binding);
        self
    }

    /// Set the collection material bindings data source.
    pub fn set_collection_material_bindings(mut self, bindings: HdDataSourceBaseHandle) -> Self {
        self.collection_material_bindings = Some(bindings);
        self
    }

    /// Build the container data source from set fields.
    ///
    /// Matches C++ BuildRetained: only includes non-None fields.
    pub fn build(self) -> HdContainerDataSourceHandle {
        let mut entries: Vec<(Token, HdDataSourceBaseHandle)> = Vec::with_capacity(2);
        if let Some(v) = self.direct_material_binding {
            entries.push((tokens::DIRECT_MATERIAL_BINDING.clone(), v));
        }
        if let Some(v) = self.collection_material_bindings {
            entries.push((tokens::COLLECTION_MATERIAL_BINDINGS.clone(), v));
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
            MaterialBindingSchema::get_schema_token().as_str(),
            "usdMaterialBinding"
        );
    }

    #[test]
    fn test_direct_material_binding_locator() {
        let locator = MaterialBindingSchema::get_direct_material_binding_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_collection_material_bindings_locator() {
        let locator = MaterialBindingSchema::get_collection_material_bindings_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_builder() {
        let _schema = MaterialBindingSchemaBuilder::new().build();
    }

    #[test]
    fn test_schema_is_defined() {
        let schema = MaterialBindingSchema::new(None);
        assert!(!schema.is_defined());
    }
}
