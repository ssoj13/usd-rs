//! ModelSchema - Hydra schema for model-level data.
//!
//! Port of pxr/usdImaging/usdImaging/modelSchema.h
//!
//! Provides data source schema for model-level data including model path,
//! asset identifier, name, and version information.

use usd_hd::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdDataSourceLocator, cast_to_container,
};
use usd_tf::Token;

// Token constants
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static MODEL: LazyLock<Token> = LazyLock::new(|| Token::new("model"));
    pub static MODEL_PATH: LazyLock<Token> = LazyLock::new(|| Token::new("modelPath"));
    pub static ASSET_IDENTIFIER: LazyLock<Token> = LazyLock::new(|| Token::new("assetIdentifier"));
    pub static ASSET_NAME: LazyLock<Token> = LazyLock::new(|| Token::new("assetName"));
    pub static ASSET_VERSION: LazyLock<Token> = LazyLock::new(|| Token::new("assetVersion"));
}

// ============================================================================
// ModelSchema
// ============================================================================

/// Schema for model-level data.
///
/// Contains model path, asset identifier, asset name, and asset version
/// information for model prims.
#[derive(Debug, Clone)]
pub struct ModelSchema {
    container: Option<HdContainerDataSourceHandle>,
}

impl ModelSchema {
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
        tokens::MODEL.clone()
    }

    /// Get the default locator for this schema.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_token(tokens::MODEL.clone())
    }

    /// Get the model path locator.
    pub fn get_model_path_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(tokens::MODEL.clone(), tokens::MODEL_PATH.clone())
    }

    /// Get the asset identifier locator.
    pub fn get_asset_identifier_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(tokens::MODEL.clone(), tokens::ASSET_IDENTIFIER.clone())
    }

    /// Get the asset name locator.
    pub fn get_asset_name_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(tokens::MODEL.clone(), tokens::ASSET_NAME.clone())
    }

    /// Get the asset version locator.
    pub fn get_asset_version_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(tokens::MODEL.clone(), tokens::ASSET_VERSION.clone())
    }

    /// Get schema from parent container.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Option<Self> {
        let ds = parent.get(&tokens::MODEL)?;
        let container = cast_to_container(&ds)?;
        Some(Self {
            container: Some(container),
        })
    }
}

// ============================================================================
// ModelSchemaBuilder
// ============================================================================

/// Builder for ModelSchema data sources.
#[derive(Debug, Default)]
pub struct ModelSchemaBuilder {
    model_path: Option<HdDataSourceBaseHandle>,
    asset_identifier: Option<HdDataSourceBaseHandle>,
    asset_name: Option<HdDataSourceBaseHandle>,
    asset_version: Option<HdDataSourceBaseHandle>,
}

impl ModelSchemaBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the model path data source.
    pub fn set_model_path(mut self, path: HdDataSourceBaseHandle) -> Self {
        self.model_path = Some(path);
        self
    }

    /// Set the asset identifier data source.
    pub fn set_asset_identifier(mut self, identifier: HdDataSourceBaseHandle) -> Self {
        self.asset_identifier = Some(identifier);
        self
    }

    /// Set the asset name data source.
    pub fn set_asset_name(mut self, name: HdDataSourceBaseHandle) -> Self {
        self.asset_name = Some(name);
        self
    }

    /// Set the asset version data source.
    pub fn set_asset_version(mut self, version: HdDataSourceBaseHandle) -> Self {
        self.asset_version = Some(version);
        self
    }

    /// Build the container data source from set fields.
    ///
    /// Matches C++ BuildRetained: only includes non-None fields.
    pub fn build(self) -> HdContainerDataSourceHandle {
        let mut entries: Vec<(Token, HdDataSourceBaseHandle)> = Vec::with_capacity(4);
        if let Some(v) = self.model_path {
            entries.push((tokens::MODEL_PATH.clone(), v));
        }
        if let Some(v) = self.asset_identifier {
            entries.push((tokens::ASSET_IDENTIFIER.clone(), v));
        }
        if let Some(v) = self.asset_name {
            entries.push((tokens::ASSET_NAME.clone(), v));
        }
        if let Some(v) = self.asset_version {
            entries.push((tokens::ASSET_VERSION.clone(), v));
        }
        usd_hd::HdRetainedContainerDataSource::from_entries(&entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_token() {
        assert_eq!(ModelSchema::get_schema_token().as_str(), "model");
    }

    #[test]
    fn test_model_path_locator() {
        let locator = ModelSchema::get_model_path_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_asset_identifier_locator() {
        let locator = ModelSchema::get_asset_identifier_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_asset_name_locator() {
        let locator = ModelSchema::get_asset_name_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_asset_version_locator() {
        let locator = ModelSchema::get_asset_version_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_builder() {
        let _schema = ModelSchemaBuilder::new().build();
    }

    #[test]
    fn test_schema_is_defined() {
        let schema = ModelSchema::new(None);
        assert!(!schema.is_defined());
    }
}
