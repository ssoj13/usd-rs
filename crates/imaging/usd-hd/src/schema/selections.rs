//! Selections schema for Hydra.
//!
//! Vector schema containing multiple selection entries.

use super::HdSelectionSchema;
use crate::data_source::{HdDataSourceLocator, HdVectorDataSourceHandle, cast_to_container};
use std::sync::LazyLock;
use usd_tf::Token;

// Schema token
pub static SELECTIONS: LazyLock<Token> = LazyLock::new(|| Token::new("selections"));

/// Vector schema for selections.
///
/// Wraps a vector data source where each element is an HdSelectionSchema.
/// Used to represent multiple selection entries on a prim.
///
/// # Location
///
/// Default locator: `selections`
#[derive(Debug, Clone)]
pub struct HdSelectionsSchema {
    vector: Option<HdVectorDataSourceHandle>,
}

impl HdSelectionsSchema {
    /// Creates selections schema from vector data source.
    pub fn new(vector: HdVectorDataSourceHandle) -> Self {
        Self {
            vector: Some(vector),
        }
    }

    /// Creates empty selections schema.
    pub fn empty() -> Self {
        Self { vector: None }
    }

    /// Retrieves selections from parent container at "selections" locator.
    ///
    /// Returns empty schema if vector not found.
    pub fn get_from_parent(parent: &crate::data_source::HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&SELECTIONS) {
            let any = &child as &dyn std::any::Any;
            if let Some(vector) = any.downcast_ref::<HdVectorDataSourceHandle>() {
                return Self::new(vector.clone());
            }
        }
        Self::empty()
    }

    /// Checks if schema is defined (has valid vector).
    pub fn is_defined(&self) -> bool {
        self.vector.is_some()
    }

    /// Returns underlying vector data source.
    pub fn get_vector(&self) -> Option<&HdVectorDataSourceHandle> {
        self.vector.as_ref()
    }

    /// Returns number of selection elements.
    pub fn get_num_elements(&self) -> usize {
        self.vector
            .as_ref()
            .map(|v| v.get_num_elements())
            .unwrap_or(0)
    }

    /// Returns selection element at given index.
    ///
    /// # Arguments
    ///
    /// * `element` - Zero-based index into selections vector
    ///
    /// # Returns
    ///
    /// Selection schema wrapping element, or empty schema if index out of bounds
    pub fn get_element(&self, element: usize) -> HdSelectionSchema {
        if let Some(vector) = &self.vector {
            if let Some(child) = vector.get_element(element) {
                if let Some(container) = cast_to_container(&child) {
                    return HdSelectionSchema::new(container);
                }
            }
        }
        HdSelectionSchema::empty()
    }

    /// Returns the schema token (`selections`).
    pub fn get_schema_token() -> &'static LazyLock<Token> {
        &SELECTIONS
    }

    /// Returns the default locator for this schema.
    ///
    /// # Returns
    ///
    /// Locator with path `["selections"]`
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[SELECTIONS.clone()])
    }

    /// Builds retained vector from selection containers.
    ///
    /// # Arguments
    ///
    /// * `selections` - Slice of selection container data sources
    pub fn build_retained(
        selections: &[crate::data_source::HdContainerDataSourceHandle],
    ) -> HdVectorDataSourceHandle {
        use crate::data_source::{HdDataSourceBaseHandle, HdRetainedSmallVectorDataSource};

        let elements: Vec<HdDataSourceBaseHandle> = selections
            .iter()
            .map(|c| c.clone() as HdDataSourceBaseHandle)
            .collect();

        HdRetainedSmallVectorDataSource::new(&elements)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source::HdRetainedSmallVectorDataSource;

    #[test]
    fn test_empty_schema() {
        let schema = HdSelectionsSchema::empty();
        assert!(!schema.is_defined());
        assert_eq!(schema.get_num_elements(), 0);
    }

    #[test]
    fn test_build_retained() {
        let selections = vec![];
        let vector = HdSelectionsSchema::build_retained(&selections);
        let schema = HdSelectionsSchema::new(vector);

        assert!(schema.is_defined());
        assert_eq!(schema.get_num_elements(), 0);
    }

    #[test]
    fn test_get_element() {
        let vector = HdRetainedSmallVectorDataSource::new(&[]);
        let schema = HdSelectionsSchema::new(vector);

        let element = schema.get_element(0);
        assert!(!element.is_defined());
    }

    #[test]
    fn test_default_locator() {
        let locator = HdSelectionsSchema::get_default_locator();
        assert_eq!(locator.elements().len(), 1);
    }
}
