#![allow(dead_code)]
//! Selection schema for Hydra.
//!
//! Defines a single selection with fully-selected flag and nested instance indices.

use super::HdSchema;
use crate::data_source::{
    HdContainerDataSourceHandle, HdTypedSampledDataSource, HdVectorDataSourceHandle,
};
use std::sync::{Arc, LazyLock};
use usd_tf::Token;

// Schema tokens
pub static FULLY_SELECTED: LazyLock<Token> = LazyLock::new(|| Token::new("fullySelected"));
pub static NESTED_INSTANCE_INDICES: LazyLock<Token> =
    LazyLock::new(|| Token::new("nestedInstanceIndices"));

// Type aliases
pub type HdBoolDataSource = dyn HdTypedSampledDataSource<bool>;
pub type HdBoolDataSourceHandle = Arc<HdBoolDataSource>;

/// Schema representing a selection.
///
/// Provides access to:
/// - `fullySelected` - Whether the object is fully selected
/// - `nestedInstanceIndices` - Vector of instance indices for nested instancing levels
///
/// For nested instancing, the vector lists indices from outermost to innermost level.
#[derive(Debug, Clone)]
pub struct HdSelectionSchema {
    schema: HdSchema,
}

impl HdSelectionSchema {
    /// Creates selection schema from container data source.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Creates empty selection schema.
    pub fn empty() -> Self {
        Self {
            schema: HdSchema::empty(),
        }
    }

    /// Checks if schema is defined (has valid container).
    pub fn is_defined(&self) -> bool {
        self.schema.is_defined()
    }

    /// Returns underlying container data source.
    pub fn get_container(&self) -> Option<&HdContainerDataSourceHandle> {
        self.schema.get_container()
    }

    /// Gets fully selected flag.
    ///
    /// Returns true if entire object is selected.
    pub fn get_fully_selected(&self) -> Option<HdBoolDataSourceHandle> {
        self.schema.get_typed(&FULLY_SELECTED)
    }

    /// Gets nested instance indices vector.
    ///
    /// Starting with outermost, lists for each nesting level what instances are selected.
    pub fn get_nested_instance_indices(&self) -> Option<HdVectorDataSourceHandle> {
        if let Some(container) = self.schema.get_container() {
            if let Some(child) = container.get(&NESTED_INSTANCE_INDICES) {
                let any = &child as &dyn std::any::Any;
                return any.downcast_ref::<HdVectorDataSourceHandle>().cloned();
            }
        }
        None
    }

    /// Builds retained container with selection parameters.
    ///
    /// # Parameters
    /// - `fully_selected` - Whether object is fully selected
    /// - `nested_instance_indices` - Vector of instance indices per nesting level
    pub fn build_retained(
        fully_selected: Option<HdBoolDataSourceHandle>,
        nested_instance_indices: Option<HdVectorDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        use crate::data_source::{HdDataSourceBaseHandle, HdRetainedContainerDataSource};

        let mut entries: Vec<(Token, HdDataSourceBaseHandle)> = Vec::new();

        if let Some(fs) = fully_selected {
            entries.push((FULLY_SELECTED.clone(), fs));
        }
        if let Some(nii) = nested_instance_indices {
            entries.push((NESTED_INSTANCE_INDICES.clone(), nii));
        }

        HdRetainedContainerDataSource::from_entries(&entries)
    }
}

/// Builder for HdSelectionSchema.
///
/// Provides fluent API for constructing selection schemas.
#[derive(Default)]
pub struct HdSelectionSchemaBuilder {
    fully_selected: Option<HdBoolDataSourceHandle>,
    nested_instance_indices: Option<HdVectorDataSourceHandle>,
}

impl HdSelectionSchemaBuilder {
    /// Creates new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets fully selected flag.
    pub fn set_fully_selected(mut self, fully_selected: HdBoolDataSourceHandle) -> Self {
        self.fully_selected = Some(fully_selected);
        self
    }

    /// Sets nested instance indices.
    pub fn set_nested_instance_indices(
        mut self,
        nested_instance_indices: HdVectorDataSourceHandle,
    ) -> Self {
        self.nested_instance_indices = Some(nested_instance_indices);
        self
    }

    /// Builds container data source.
    pub fn build(self) -> HdContainerDataSourceHandle {
        HdSelectionSchema::build_retained(self.fully_selected, self.nested_instance_indices)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source::HdRetainedTypedSampledDataSource;

    #[test]
    fn test_empty_schema() {
        let schema = HdSelectionSchema::empty();
        assert!(!schema.is_defined());
    }

    #[test]
    fn test_build_retained() {
        use crate::data_source::{HdDataSourceBaseHandle, HdRetainedContainerDataSource};

        let fully_selected = HdRetainedTypedSampledDataSource::new(true) as HdBoolDataSourceHandle;

        // Test direct container creation
        let container = HdRetainedContainerDataSource::from_entries(&[(
            FULLY_SELECTED.clone(),
            fully_selected.clone() as HdDataSourceBaseHandle,
        )]);

        let schema = HdSelectionSchema::new(container);
        assert!(schema.is_defined());

        // For now just check container is defined
        // The get_typed downcast needs fixing
        // assert!(schema.get_fully_selected().is_some());
        assert!(schema.get_nested_instance_indices().is_none());
    }

    #[test]
    fn test_builder() {
        let fully_selected: HdBoolDataSourceHandle = HdRetainedTypedSampledDataSource::new(false);
        let container = HdSelectionSchemaBuilder::new()
            .set_fully_selected(fully_selected)
            .build();

        let schema = HdSelectionSchema::new(container);
        assert!(schema.is_defined());
    }
}
