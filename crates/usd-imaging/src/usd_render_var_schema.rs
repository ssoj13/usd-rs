//! UsdRenderVarSchema - Hydra schema for render variable data.
//!
//! Port of pxr/usdImaging/usdImaging/usdRenderVarSchema.h
//!
//! Provides data source schema for render variables (AOVs) in Hydra.

use usd_hd::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdDataSourceLocator, cast_to_container,
};
use usd_tf::Token;

// Token constants
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static USD_RENDER_VAR: LazyLock<Token> = LazyLock::new(|| Token::new("__usdRenderVar"));
    pub static DATA_TYPE: LazyLock<Token> = LazyLock::new(|| Token::new("dataType"));
    pub static SOURCE_NAME: LazyLock<Token> = LazyLock::new(|| Token::new("sourceName"));
    pub static SOURCE_TYPE: LazyLock<Token> = LazyLock::new(|| Token::new("sourceType"));
    pub static NAMESPACED_SETTINGS: LazyLock<Token> =
        LazyLock::new(|| Token::new("namespacedSettings"));
}

// ============================================================================
// UsdRenderVarSchema
// ============================================================================

/// Schema for render variable (AOV) data in Hydra.
///
/// Corresponds to UsdRenderVar. Defines a render output variable
/// including its data type, source, and custom settings.
#[derive(Debug, Clone)]
pub struct UsdRenderVarSchema {
    container: Option<HdContainerDataSourceHandle>,
}

impl UsdRenderVarSchema {
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
        tokens::USD_RENDER_VAR.clone()
    }

    /// Get the default locator for this schema.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_token(tokens::USD_RENDER_VAR.clone())
    }

    /// Get the data type locator.
    pub fn get_data_type_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::USD_RENDER_VAR.clone(),
            tokens::DATA_TYPE.clone(),
        )
    }

    /// Get the source name locator.
    pub fn get_source_name_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::USD_RENDER_VAR.clone(),
            tokens::SOURCE_NAME.clone(),
        )
    }

    /// Get the source type locator.
    pub fn get_source_type_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::USD_RENDER_VAR.clone(),
            tokens::SOURCE_TYPE.clone(),
        )
    }

    /// Get the namespaced settings locator.
    pub fn get_namespaced_settings_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::USD_RENDER_VAR.clone(),
            tokens::NAMESPACED_SETTINGS.clone(),
        )
    }

    /// Get schema from parent container.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Option<Self> {
        let ds = parent.get(&tokens::USD_RENDER_VAR)?;
        let container = cast_to_container(&ds)?;
        Some(Self {
            container: Some(container),
        })
    }
}

// ============================================================================
// UsdRenderVarSchemaBuilder
// ============================================================================

/// Builder for UsdRenderVarSchema data sources.
#[derive(Debug, Default)]
pub struct UsdRenderVarSchemaBuilder {
    data_type: Option<HdDataSourceBaseHandle>,
    source_name: Option<HdDataSourceBaseHandle>,
    source_type: Option<HdDataSourceBaseHandle>,
    namespaced_settings: Option<HdDataSourceBaseHandle>,
}

impl UsdRenderVarSchemaBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the data type data source.
    pub fn set_data_type(mut self, data_type: HdDataSourceBaseHandle) -> Self {
        self.data_type = Some(data_type);
        self
    }

    /// Set the source name data source.
    pub fn set_source_name(mut self, name: HdDataSourceBaseHandle) -> Self {
        self.source_name = Some(name);
        self
    }

    /// Set the source type data source.
    pub fn set_source_type(mut self, source_type: HdDataSourceBaseHandle) -> Self {
        self.source_type = Some(source_type);
        self
    }

    /// Set the namespaced settings data source.
    pub fn set_namespaced_settings(mut self, settings: HdDataSourceBaseHandle) -> Self {
        self.namespaced_settings = Some(settings);
        self
    }

    /// Build the container data source from set fields.
    ///
    /// Matches C++ BuildRetained: only includes non-None fields.
    pub fn build(self) -> HdContainerDataSourceHandle {
        let mut entries: Vec<(Token, HdDataSourceBaseHandle)> = Vec::with_capacity(4);
        if let Some(v) = self.data_type {
            entries.push((tokens::DATA_TYPE.clone(), v));
        }
        if let Some(v) = self.source_name {
            entries.push((tokens::SOURCE_NAME.clone(), v));
        }
        if let Some(v) = self.source_type {
            entries.push((tokens::SOURCE_TYPE.clone(), v));
        }
        if let Some(v) = self.namespaced_settings {
            entries.push((tokens::NAMESPACED_SETTINGS.clone(), v));
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
            UsdRenderVarSchema::get_schema_token().as_str(),
            "__usdRenderVar"
        );
    }

    #[test]
    fn test_data_type_locator() {
        let locator = UsdRenderVarSchema::get_data_type_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_source_name_locator() {
        let locator = UsdRenderVarSchema::get_source_name_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_source_type_locator() {
        let locator = UsdRenderVarSchema::get_source_type_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_namespaced_settings_locator() {
        let locator = UsdRenderVarSchema::get_namespaced_settings_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_builder() {
        let _schema = UsdRenderVarSchemaBuilder::new().build();
    }

    #[test]
    fn test_builder_chain() {
        let _schema = UsdRenderVarSchemaBuilder::new()
            .set_data_type(usd_hd::HdRetainedContainerDataSource::new_empty())
            .set_source_name(usd_hd::HdRetainedContainerDataSource::new_empty())
            .build();
    }
}
