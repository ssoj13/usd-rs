//! ExtentsHintSchema - Hydra schema for extents hints.
//!
//! Port of pxr/usdImaging/usdImaging/extentsHintSchema.h
//!
//! Provides data source schema for extents hints in Hydra.
//! Contains extent information for different purposes (render, proxy, etc).

use usd_hd::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdDataSourceLocator, cast_to_container,
};
use usd_tf::Token;

// Token constants
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static EXTENTS_HINT: LazyLock<Token> = LazyLock::new(|| Token::new("extentsHint"));
}

// ============================================================================
// ExtentsHintSchema
// ============================================================================

/// Schema for extents hints in Hydra.
///
/// Contains extent information for different purposes (render, proxy, guide).
/// Each purpose maps to an HdExtentSchema with min/max bounds.
#[derive(Debug, Clone)]
pub struct ExtentsHintSchema {
    container: Option<HdContainerDataSourceHandle>,
}

impl ExtentsHintSchema {
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
        tokens::EXTENTS_HINT.clone()
    }

    /// Get the default locator for this schema.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_token(tokens::EXTENTS_HINT.clone())
    }

    /// Get extent for a specific purpose token.
    ///
    /// Looks up a child container keyed by `purpose` and wraps it as HdExtentSchema.
    pub fn get_extent(&self, purpose: &Token) -> Option<usd_hd::HdExtentSchema> {
        let container = self.container.as_ref()?;
        let ds = container.get(purpose)?;
        let child = cast_to_container(&ds)?;
        Some(usd_hd::HdExtentSchema::new(child))
    }

    /// Get schema from parent container.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Option<Self> {
        let ds = parent.get(&tokens::EXTENTS_HINT)?;
        let container = cast_to_container(&ds)?;
        Some(Self {
            container: Some(container),
        })
    }

    /// Build retained container with extent hints.
    ///
    /// Mirrors C++ BuildRetained(count, names, values).
    ///
    /// # Arguments
    /// * `names` - Purpose tokens (render, proxy, guide, etc)
    /// * `values` - Extent data sources for each purpose
    pub fn build_retained(
        names: &[Token],
        values: &[HdDataSourceBaseHandle],
    ) -> HdContainerDataSourceHandle {
        usd_hd::HdRetainedContainerDataSource::from_arrays(names, values)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_token() {
        assert_eq!(
            ExtentsHintSchema::get_schema_token().as_str(),
            "extentsHint"
        );
    }

    #[test]
    fn test_default_locator() {
        let locator = ExtentsHintSchema::get_default_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_is_defined() {
        let schema = ExtentsHintSchema::new(None);
        assert!(!schema.is_defined());
    }

    #[test]
    fn test_build_retained() {
        let _container = ExtentsHintSchema::build_retained(&[], &[]);
    }
}
