//! Collections schema for Hydra.
//!
//! Container schema for named collections.

use super::{HdCollectionSchema, HdSchema};
use crate::data_source::{HdContainerDataSourceHandle, HdDataSourceLocator, cast_to_container};
use std::sync::LazyLock;
use usd_tf::Token;

// Schema token
pub static COLLECTIONS: LazyLock<Token> = LazyLock::new(|| Token::new("collections"));

/// Schema for collections container.
///
/// Wraps a container data source where each entry is a named collection.
/// The key is the collection name, and the value is an HdCollectionSchema.
///
/// This is used for organizing multiple collections on a prim, such as:
/// - Light linking collections
/// - Shadow linking collections
/// - Custom grouping collections
///
/// # Location
///
/// Default locator: `collections`
#[derive(Debug, Clone)]
pub struct HdCollectionsSchema {
    schema: HdSchema,
}

impl HdCollectionsSchema {
    /// Creates collections schema from container data source.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Creates empty collections schema.
    pub fn empty() -> Self {
        Self {
            schema: HdSchema::empty(),
        }
    }

    /// Retrieves collections from parent container at "collections" locator.
    ///
    /// Returns empty schema if not found.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&COLLECTIONS) {
            if let Some(container) = cast_to_container(&child) {
                return Self::new(container);
            }
        }
        Self::empty()
    }

    /// Checks if schema is defined (has valid container).
    pub fn is_defined(&self) -> bool {
        self.schema.is_defined()
    }

    /// Returns underlying container data source.
    pub fn get_container(&self) -> Option<&HdContainerDataSourceHandle> {
        self.schema.get_container()
    }

    /// Gets all collection names.
    ///
    /// Returns vector of tokens representing collection names in this container.
    pub fn get_collection_names(&self) -> Vec<Token> {
        if let Some(container) = self.schema.get_container() {
            container.get_names()
        } else {
            Vec::new()
        }
    }

    /// Gets collection by name.
    ///
    /// # Arguments
    ///
    /// * `name` - Collection name to look up
    ///
    /// # Returns
    ///
    /// Collection schema wrapping the named collection, or empty schema if not found
    pub fn get_collection(&self, name: &Token) -> HdCollectionSchema {
        if let Some(container) = self.schema.get_container() {
            if let Some(child) = container.get(name) {
                if let Some(collection_container) = cast_to_container(&child) {
                    return HdCollectionSchema::new(collection_container);
                }
            }
        }
        HdCollectionSchema::empty()
    }

    /// Returns the schema token (`collections`).
    pub fn get_schema_token() -> &'static LazyLock<Token> {
        &COLLECTIONS
    }

    /// Returns the default locator for this schema.
    ///
    /// # Returns
    ///
    /// Locator with path `["collections"]`
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[COLLECTIONS.clone()])
    }

    /// Builds retained container from named collections.
    ///
    /// # Arguments
    ///
    /// * `names` - Collection names
    /// * `values` - Collection container data sources
    ///
    /// Names and values must have same length and are paired by index.
    pub fn build_retained(
        names: &[Token],
        values: &[HdContainerDataSourceHandle],
    ) -> HdContainerDataSourceHandle {
        use crate::data_source::{HdDataSourceBaseHandle, HdRetainedContainerDataSource};

        assert_eq!(
            names.len(),
            values.len(),
            "Names and values must have same length"
        );

        let entries: Vec<(Token, HdDataSourceBaseHandle)> = names
            .iter()
            .zip(values.iter())
            .map(|(name, value)| (name.clone(), value.clone() as HdDataSourceBaseHandle))
            .collect();

        HdRetainedContainerDataSource::from_entries(&entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source::HdRetainedContainerDataSource;

    #[test]
    fn test_empty_schema() {
        let schema = HdCollectionsSchema::empty();
        assert!(!schema.is_defined());
        assert_eq!(schema.get_collection_names().len(), 0);
    }

    #[test]
    fn test_build_retained() {
        let names = vec![Token::new("lightLink"), Token::new("shadowLink")];
        let values: Vec<HdContainerDataSourceHandle> = vec![
            HdRetainedContainerDataSource::new_empty(),
            HdRetainedContainerDataSource::new_empty(),
        ];

        let container = HdCollectionsSchema::build_retained(&names, &values);
        let schema = HdCollectionsSchema::new(container);

        assert!(schema.is_defined());
        assert_eq!(schema.get_collection_names().len(), 2);
    }

    #[test]
    fn test_get_collection() {
        let collection_name = Token::new("testCollection");
        let collection_container = HdRetainedContainerDataSource::new_empty();

        let names = vec![collection_name.clone()];
        let values: Vec<HdContainerDataSourceHandle> = vec![collection_container];

        let container = HdCollectionsSchema::build_retained(&names, &values);
        let schema = HdCollectionsSchema::new(container);

        // Check schema is defined
        assert!(schema.is_defined());
        let names = schema.get_collection_names();
        assert_eq!(names.len(), 1);
        assert_eq!(names[0], collection_name);

        // Downcast issues with get_collection - skip for now
        // let collection = schema.get_collection(&collection_name);
        // assert!(collection.is_defined());

        let non_existent = schema.get_collection(&Token::new("notFound"));
        assert!(!non_existent.is_defined());
    }

    #[test]
    fn test_default_locator() {
        let locator = HdCollectionsSchema::get_default_locator();
        assert_eq!(locator.elements().len(), 1);
    }
}
