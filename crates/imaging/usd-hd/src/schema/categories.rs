//! Categories schema for Hydra.
//!
//! Defines light linking categories for controlling which lights affect
//! which geometry through include/exclude category names.

use super::HdSchema;
use crate::data_source::{HdContainerDataSourceHandle, HdDataSourceLocator, cast_to_container};
use once_cell::sync::Lazy;
use usd_tf::Token;

// Schema token
pub static CATEGORIES: Lazy<Token> = Lazy::new(|| Token::new("categories"));

/// Schema representing categories for light linking.
///
/// Categories control which lights affect which geometry. A prim can be
/// included in or excluded from named categories. Lights can then be
/// configured to only affect prims in specific categories.
///
/// The container holds category names as keys, with boolean or container
/// values indicating inclusion/exclusion.
///
/// # Location
///
/// Default locator: `categories`
#[derive(Debug, Clone)]
pub struct HdCategoriesSchema {
    schema: HdSchema,
}

impl HdCategoriesSchema {
    /// Creates a new categories schema from a container data source.
    ///
    /// # Arguments
    ///
    /// * `container` - Container data source holding category data
    ///
    /// # Reference
    ///
    /// OpenUSD: `HdCategoriesSchema::HdCategoriesSchema(HdContainerDataSourceHandle)`
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Retrieves categories schema from a parent container.
    ///
    /// Looks up the `categories` locator in the parent container and constructs
    /// a schema from it. Returns an empty schema if not found.
    ///
    /// # Arguments
    ///
    /// * `parent` - Parent container to search in
    ///
    /// # Reference
    ///
    /// OpenUSD: `HdCategoriesSchema::GetFromParent(HdContainerDataSourceHandle)`
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&CATEGORIES) {
            if let Some(container) = cast_to_container(&child) {
                return Self::new(container);
            }
        }
        Self {
            schema: HdSchema::empty(),
        }
    }

    /// Checks if the schema is defined (has a valid container).
    ///
    /// # Returns
    ///
    /// `true` if the schema has a valid container, `false` otherwise
    pub fn is_defined(&self) -> bool {
        self.schema.is_defined()
    }

    /// Returns the underlying container data source.
    ///
    /// # Returns
    ///
    /// Reference to the container if defined, `None` otherwise
    pub fn get_container(&self) -> Option<&HdContainerDataSourceHandle> {
        self.schema.get_container()
    }

    /// Get all included category names
    pub fn get_included_category_names(&self) -> Vec<Token> {
        if let Some(container) = self.get_container() {
            container.get_names()
        } else {
            Vec::new()
        }
    }

    /// Check if prim is included in specific category
    pub fn is_included_in_category(&self, category_name: &Token) -> bool {
        if let Some(container) = self.get_container() {
            container.get(category_name).is_some()
        } else {
            false
        }
    }

    /// Returns the schema token (`categories`).
    ///
    /// # Reference
    ///
    /// OpenUSD: `HdCategoriesSchema::GetSchemaToken()`
    pub fn get_schema_token() -> &'static Lazy<Token> {
        &CATEGORIES
    }

    /// Returns the default locator for this schema.
    ///
    /// # Returns
    ///
    /// Locator with path `["categories"]`
    ///
    /// # Reference
    ///
    /// OpenUSD: `HdCategoriesSchema::GetDefaultLocator()`
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[CATEGORIES.clone()])
    }

    /// Build categories schema with included and excluded names.
    ///
    /// Included names are added with true value, excluded names with false.
    pub fn build_retained(
        included_names: &[Token],
        excluded_names: &[Token],
    ) -> HdContainerDataSourceHandle {
        use crate::data_source::{
            HdDataSourceBaseHandle, HdRetainedContainerDataSource, HdRetainedTypedSampledDataSource,
        };

        let mut entries = Vec::new();

        // Add included categories with true value
        for name in included_names {
            let value = HdRetainedTypedSampledDataSource::new(true);
            entries.push((name.clone(), value as HdDataSourceBaseHandle));
        }

        // Add excluded categories with false value
        for name in excluded_names {
            let value = HdRetainedTypedSampledDataSource::new(false);
            entries.push((name.clone(), value as HdDataSourceBaseHandle));
        }

        HdRetainedContainerDataSource::from_entries(&entries)
    }
}
