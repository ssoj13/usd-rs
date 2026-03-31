//! Instance categories schema for Hydra.
//!
//! Provides access to category values for instances.

use super::HdSchema;
use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceLocator, HdVectorDataSourceHandle, cast_to_container,
};
use once_cell::sync::Lazy;
use usd_tf::Token;

// Schema tokens

/// Schema token for instance categories
pub static INSTANCE_CATEGORIES: Lazy<Token> = Lazy::new(|| Token::new("instanceCategories"));
/// Schema token for categories values
pub static CATEGORIES_VALUES: Lazy<Token> = Lazy::new(|| Token::new("categoriesValues"));

/// Schema representing instance categories.
///
/// Provides access to:
/// - `categoriesValues` - Vector data source containing category values
///
/// # Location
///
/// Default locator: `instanceCategories`
#[derive(Debug, Clone)]
pub struct HdInstanceCategoriesSchema {
    schema: HdSchema,
}

impl HdInstanceCategoriesSchema {
    /// Create schema from container data source
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Extract instance categories schema from parent container
    ///
    /// Returns empty schema if not found
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&INSTANCE_CATEGORIES) {
            if let Some(container) = cast_to_container(&child) {
                return Self::new(container);
            }
        }
        Self {
            schema: HdSchema::empty(),
        }
    }

    /// Check if schema is defined (has valid container)
    pub fn is_defined(&self) -> bool {
        self.schema.is_defined()
    }

    /// Get underlying container data source
    pub fn get_container(&self) -> Option<&HdContainerDataSourceHandle> {
        self.schema.get_container()
    }

    /// Get categories values vector data source
    pub fn get_categories_values(&self) -> Option<HdVectorDataSourceHandle> {
        if let Some(container) = self.get_container() {
            if let Some(child) = container.get(&CATEGORIES_VALUES) {
                let any = &child as &dyn std::any::Any;
                return any.downcast_ref::<HdVectorDataSourceHandle>().cloned();
            }
        }
        None
    }

    /// Get schema token for instance categories
    pub fn get_schema_token() -> &'static Lazy<Token> {
        &INSTANCE_CATEGORIES
    }

    /// Get default data source locator for instance categories
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[INSTANCE_CATEGORIES.clone()])
    }

    /// Build retained container with instance categories data
    ///
    /// # Arguments
    ///
    /// * `categories_values` - Vector data source with category values
    pub fn build_retained(
        categories_values: Option<HdVectorDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        use crate::data_source::{HdDataSourceBaseHandle, HdRetainedContainerDataSource};

        let mut entries = Vec::new();

        if let Some(v) = categories_values {
            entries.push((CATEGORIES_VALUES.clone(), v as HdDataSourceBaseHandle));
        }

        HdRetainedContainerDataSource::from_entries(&entries)
    }
}

/// Builder for HdInstanceCategoriesSchema
///
/// Provides fluent API for constructing instance categories schemas.
#[allow(dead_code)]
#[derive(Default)]
pub struct HdInstanceCategoriesSchemaBuilder {
    /// Categories values
    categories_values: Option<HdVectorDataSourceHandle>,
}

#[allow(dead_code)]
impl HdInstanceCategoriesSchemaBuilder {
    /// Create new builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set categories values
    pub fn set_categories_values(mut self, v: HdVectorDataSourceHandle) -> Self {
        self.categories_values = Some(v);
        self
    }

    /// Build container data source with configured values
    pub fn build(self) -> HdContainerDataSourceHandle {
        HdInstanceCategoriesSchema::build_retained(self.categories_values)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_token() {
        let token = HdInstanceCategoriesSchema::get_schema_token();
        assert_eq!(token.as_str(), "instanceCategories");
    }

    #[test]
    fn test_default_locator() {
        let locator = HdInstanceCategoriesSchema::get_default_locator();
        assert!(!locator.is_empty());
    }

    #[test]
    fn test_empty_schema() {
        let schema = HdInstanceCategoriesSchema {
            schema: HdSchema::empty(),
        };
        assert!(!schema.is_defined());
        assert!(schema.get_container().is_none());
    }

    #[test]
    fn test_build_retained() {
        let container = HdInstanceCategoriesSchema::build_retained(None);
        assert!(container.get_names().is_empty());
    }

    #[test]
    fn test_builder() {
        let builder = HdInstanceCategoriesSchemaBuilder::new();
        let container = builder.build();
        assert!(container.get_names().is_empty());
    }
}
