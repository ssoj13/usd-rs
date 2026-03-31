#![allow(dead_code)]
//! UsdPrimInfoSchema - Hydra schema for USD prim information.
//!
//! Port of pxr/usdImaging/usdImaging/usdPrimInfoSchema.h
//!
//! Provides data source schema for USD prim metadata including specifier,
//! type name, loaded state, API schemas, kind, and prototype information.

use usd_hd::data_source::cast_to_container;
use usd_hd::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdDataSourceLocator,
    HdRetainedContainerDataSource,
};
use usd_tf::Token;

// Token constants
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static USD_PRIM_INFO: LazyLock<Token> = LazyLock::new(|| Token::new("__usdPrimInfo"));
    pub static SPECIFIER: LazyLock<Token> = LazyLock::new(|| Token::new("specifier"));
    pub static TYPE_NAME: LazyLock<Token> = LazyLock::new(|| Token::new("typeName"));
    pub static IS_LOADED: LazyLock<Token> = LazyLock::new(|| Token::new("isLoaded"));
    pub static API_SCHEMAS: LazyLock<Token> = LazyLock::new(|| Token::new("apiSchemas"));
    pub static KIND: LazyLock<Token> = LazyLock::new(|| Token::new("kind"));
    pub static NI_PROTOTYPE_PATH: LazyLock<Token> = LazyLock::new(|| Token::new("niPrototypePath"));
    pub static IS_NI_PROTOTYPE: LazyLock<Token> = LazyLock::new(|| Token::new("isNiPrototype"));
    pub static PI_PROPAGATED_PROTOTYPES: LazyLock<Token> =
        LazyLock::new(|| Token::new("piPropagatedPrototypes"));

    // Specifier values
    pub static DEF: LazyLock<Token> = LazyLock::new(|| Token::new("def"));
    pub static OVER: LazyLock<Token> = LazyLock::new(|| Token::new("over"));
    pub static CLASS: LazyLock<Token> = LazyLock::new(|| Token::new("class"));
}

// ============================================================================
// UsdPrimInfoSchema
// ============================================================================

/// Schema for USD prim information.
///
/// Contains USD-specific prim metadata including specifier (def/over/class),
/// type name, loaded state, applied API schemas, kind, and prototype information
/// for native and point instancing.
#[derive(Debug, Clone)]
pub struct UsdPrimInfoSchema {
    container: Option<HdContainerDataSourceHandle>,
}

impl UsdPrimInfoSchema {
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
        tokens::USD_PRIM_INFO.clone()
    }

    /// Get the default locator for this schema.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_token(tokens::USD_PRIM_INFO.clone())
    }

    /// Get the specifier locator.
    pub fn get_specifier_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(tokens::USD_PRIM_INFO.clone(), tokens::SPECIFIER.clone())
    }

    /// Get the type name locator.
    pub fn get_type_name_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(tokens::USD_PRIM_INFO.clone(), tokens::TYPE_NAME.clone())
    }

    /// Get the is loaded locator.
    pub fn get_is_loaded_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(tokens::USD_PRIM_INFO.clone(), tokens::IS_LOADED.clone())
    }

    /// Get the API schemas locator.
    pub fn get_api_schemas_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::USD_PRIM_INFO.clone(),
            tokens::API_SCHEMAS.clone(),
        )
    }

    /// Get the kind locator.
    pub fn get_kind_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(tokens::USD_PRIM_INFO.clone(), tokens::KIND.clone())
    }

    /// Get the native instancing prototype path locator.
    pub fn get_ni_prototype_path_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::USD_PRIM_INFO.clone(),
            tokens::NI_PROTOTYPE_PATH.clone(),
        )
    }

    /// Get the is native instancing prototype locator.
    pub fn get_is_ni_prototype_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::USD_PRIM_INFO.clone(),
            tokens::IS_NI_PROTOTYPE.clone(),
        )
    }

    /// Get the point instancer propagated prototypes locator.
    pub fn get_pi_propagated_prototypes_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::USD_PRIM_INFO.clone(),
            tokens::PI_PROPAGATED_PROTOTYPES.clone(),
        )
    }

    /// Get schema from parent container.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Option<Self> {
        let child = parent.get(&tokens::USD_PRIM_INFO)?;
        let container = cast_to_container(&child)?;
        Some(Self::new(Some(container)))
    }
}

// ============================================================================
// UsdPrimInfoSchemaBuilder
// ============================================================================

/// Builder for UsdPrimInfoSchema data sources.
#[derive(Debug, Default)]
pub struct UsdPrimInfoSchemaBuilder {
    specifier: Option<HdDataSourceBaseHandle>,
    type_name: Option<HdDataSourceBaseHandle>,
    is_loaded: Option<HdDataSourceBaseHandle>,
    api_schemas: Option<HdDataSourceBaseHandle>,
    kind: Option<HdDataSourceBaseHandle>,
    ni_prototype_path: Option<HdDataSourceBaseHandle>,
    is_ni_prototype: Option<HdDataSourceBaseHandle>,
    pi_propagated_prototypes: Option<HdDataSourceBaseHandle>,
}

impl UsdPrimInfoSchemaBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the specifier data source.
    pub fn set_specifier(mut self, specifier: HdDataSourceBaseHandle) -> Self {
        self.specifier = Some(specifier);
        self
    }

    /// Set the type name data source.
    pub fn set_type_name(mut self, type_name: HdDataSourceBaseHandle) -> Self {
        self.type_name = Some(type_name);
        self
    }

    /// Set the is loaded data source.
    pub fn set_is_loaded(mut self, is_loaded: HdDataSourceBaseHandle) -> Self {
        self.is_loaded = Some(is_loaded);
        self
    }

    /// Set the API schemas data source.
    pub fn set_api_schemas(mut self, api_schemas: HdDataSourceBaseHandle) -> Self {
        self.api_schemas = Some(api_schemas);
        self
    }

    /// Set the kind data source.
    pub fn set_kind(mut self, kind: HdDataSourceBaseHandle) -> Self {
        self.kind = Some(kind);
        self
    }

    /// Set the native instancing prototype path data source.
    pub fn set_ni_prototype_path(mut self, path: HdDataSourceBaseHandle) -> Self {
        self.ni_prototype_path = Some(path);
        self
    }

    /// Set the is native instancing prototype data source.
    pub fn set_is_ni_prototype(mut self, is_prototype: HdDataSourceBaseHandle) -> Self {
        self.is_ni_prototype = Some(is_prototype);
        self
    }

    /// Set the point instancer propagated prototypes data source.
    pub fn set_pi_propagated_prototypes(mut self, prototypes: HdDataSourceBaseHandle) -> Self {
        self.pi_propagated_prototypes = Some(prototypes);
        self
    }

    /// Build the container data source.
    pub fn build(self) -> HdContainerDataSourceHandle {
        let mut entries: Vec<(Token, HdDataSourceBaseHandle)> = Vec::new();
        if let Some(v) = self.specifier {
            entries.push((tokens::SPECIFIER.clone(), v));
        }
        if let Some(v) = self.type_name {
            entries.push((tokens::TYPE_NAME.clone(), v));
        }
        if let Some(v) = self.is_loaded {
            entries.push((tokens::IS_LOADED.clone(), v));
        }
        if let Some(v) = self.api_schemas {
            entries.push((tokens::API_SCHEMAS.clone(), v));
        }
        if let Some(v) = self.kind {
            entries.push((tokens::KIND.clone(), v));
        }
        if let Some(v) = self.ni_prototype_path {
            entries.push((tokens::NI_PROTOTYPE_PATH.clone(), v));
        }
        if let Some(v) = self.is_ni_prototype {
            entries.push((tokens::IS_NI_PROTOTYPE.clone(), v));
        }
        if let Some(v) = self.pi_propagated_prototypes {
            entries.push((tokens::PI_PROPAGATED_PROTOTYPES.clone(), v));
        }
        HdRetainedContainerDataSource::from_entries(&entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_token() {
        assert_eq!(
            UsdPrimInfoSchema::get_schema_token().as_str(),
            "__usdPrimInfo"
        );
    }

    #[test]
    fn test_specifier_locator() {
        let locator = UsdPrimInfoSchema::get_specifier_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_type_name_locator() {
        let locator = UsdPrimInfoSchema::get_type_name_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_is_loaded_locator() {
        let locator = UsdPrimInfoSchema::get_is_loaded_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_api_schemas_locator() {
        let locator = UsdPrimInfoSchema::get_api_schemas_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_kind_locator() {
        let locator = UsdPrimInfoSchema::get_kind_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_ni_prototype_path_locator() {
        let locator = UsdPrimInfoSchema::get_ni_prototype_path_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_is_ni_prototype_locator() {
        let locator = UsdPrimInfoSchema::get_is_ni_prototype_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_pi_propagated_prototypes_locator() {
        let locator = UsdPrimInfoSchema::get_pi_propagated_prototypes_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_specifier_tokens() {
        assert_eq!(tokens::DEF.as_str(), "def");
        assert_eq!(tokens::OVER.as_str(), "over");
        assert_eq!(tokens::CLASS.as_str(), "class");
    }

    #[test]
    fn test_builder() {
        let _schema = UsdPrimInfoSchemaBuilder::new().build();
    }

    #[test]
    fn test_schema_is_defined() {
        let schema = UsdPrimInfoSchema::new(None);
        assert!(!schema.is_defined());
    }
}
