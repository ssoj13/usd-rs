#![allow(dead_code)]
//! Collection schema for Hydra.
//!
//! Defines a collection with membership expression for grouping prims.

use super::HdSchema;
use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdDataSourceLocator, cast_to_container,
};
use std::sync::LazyLock;
use usd_tf::Token;

// Schema tokens
pub static COLLECTION: LazyLock<Token> = LazyLock::new(|| Token::new("collection"));
pub static MEMBERSHIP_EXPRESSION: LazyLock<Token> =
    LazyLock::new(|| Token::new("membershipExpression"));

// Note: HdPathExpressionDataSource would need to be defined in data_source module
// For now we use base data source handle - proper path expression typing to be added later
pub type HdPathExpressionDataSourceHandle = HdDataSourceBaseHandle;

/// Schema representing a collection.
///
/// Collections define groups of prims using path expressions. They're used for:
/// - Light linking (which lights affect which geometry)
/// - Shadow linking (which lights cast shadows on which geometry)  
/// - Matte and phantom settings
/// - General prim grouping
///
/// Provides access to:
/// - `membershipExpression` - Path expression defining collection membership
///
/// # Location
///
/// Default locator: `collection`
#[derive(Debug, Clone)]
pub struct HdCollectionSchema {
    schema: HdSchema,
}

impl HdCollectionSchema {
    /// Creates collection schema from container data source.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Creates empty collection schema.
    pub fn empty() -> Self {
        Self {
            schema: HdSchema::empty(),
        }
    }

    /// Retrieves collection from parent container at "collection" locator.
    ///
    /// Returns empty schema if not found.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&COLLECTION) {
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

    /// Gets membership expression.
    ///
    /// The path expression defines which prims belong to this collection.
    pub fn get_membership_expression(&self) -> Option<HdPathExpressionDataSourceHandle> {
        if let Some(container) = self.schema.get_container() {
            container.get(&MEMBERSHIP_EXPRESSION)
        } else {
            None
        }
    }

    /// Returns the schema token (`collection`).
    pub fn get_schema_token() -> &'static LazyLock<Token> {
        &COLLECTION
    }

    /// Returns the default locator for this schema.
    ///
    /// # Returns
    ///
    /// Locator with path `["collection"]`
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[COLLECTION.clone()])
    }

    /// Builds retained container with collection parameters.
    ///
    /// # Parameters
    ///
    /// * `membership_expression` - Path expression defining collection membership
    pub fn build_retained(
        membership_expression: Option<HdPathExpressionDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        use crate::data_source::HdRetainedContainerDataSource;

        let mut entries = Vec::new();

        if let Some(me) = membership_expression {
            entries.push((MEMBERSHIP_EXPRESSION.clone(), me));
        }

        HdRetainedContainerDataSource::from_entries(&entries)
    }
}

/// Builder for HdCollectionSchema.
///
/// Provides fluent API for constructing collection schemas.
#[derive(Default)]
pub struct HdCollectionSchemaBuilder {
    membership_expression: Option<HdPathExpressionDataSourceHandle>,
}

impl HdCollectionSchemaBuilder {
    /// Creates new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets membership expression.
    pub fn set_membership_expression(
        mut self,
        membership_expression: HdPathExpressionDataSourceHandle,
    ) -> Self {
        self.membership_expression = Some(membership_expression);
        self
    }

    /// Builds container data source.
    pub fn build(self) -> HdContainerDataSourceHandle {
        HdCollectionSchema::build_retained(self.membership_expression)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_schema() {
        let schema = HdCollectionSchema::empty();
        assert!(!schema.is_defined());
    }

    #[test]
    fn test_build_retained() {
        let container = HdCollectionSchema::build_retained(None);
        let schema = HdCollectionSchema::new(container);
        assert!(schema.is_defined());
        assert!(schema.get_membership_expression().is_none());
    }

    #[test]
    fn test_default_locator() {
        let locator = HdCollectionSchema::get_default_locator();
        assert_eq!(locator.elements().len(), 1);
    }
}
