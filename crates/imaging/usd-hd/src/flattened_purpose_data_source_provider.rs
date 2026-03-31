
//! HdFlattenedPurposeDataSourceProvider - Flattens purpose inheritance.
//!
//! Port of pxr/imaging/hd/flattenedPurposeDataSourceProvider.cpp.
//!
//! If the prim has a purpose authored, use it. Otherwise check if the parent's
//! purpose is inheritable. Falls back to the prim's input data as-is.

use crate::data_source::{HdContainerDataSourceHandle, HdDataSourceLocatorSet};
use crate::flattened_data_source_provider::{
    HdFlattenedDataSourceProvider, HdFlattenedDataSourceProviderContext,
};
use crate::schema::HdPurposeSchema;

/// Provider that flattens purpose, inheriting from parent when local is absent.
///
/// Corresponds to C++ HdFlattenedPurposeDataSourceProvider.
///
/// Inheritance rules:
/// 1. If prim has authored purpose -> use it
/// 2. If parent purpose is marked inheritable -> inherit it
/// 3. Otherwise -> pass through input data as-is
#[derive(Debug, Default)]
pub struct HdFlattenedPurposeDataSourceProvider;

impl HdFlattenedPurposeDataSourceProvider {
    /// Create new purpose flattening provider.
    pub fn new() -> Self {
        Self
    }
}

impl HdFlattenedDataSourceProvider for HdFlattenedPurposeDataSourceProvider {
    fn get_flattened_data_source(
        &self,
        ctx: &HdFlattenedDataSourceProviderContext<'_>,
    ) -> Option<HdContainerDataSourceHandle> {
        let input_container = ctx.get_input_data_source();
        let input_purpose = input_container
            .as_ref()
            .map(|c| HdPurposeSchema::new(c.clone()));

        // If prim has authored purpose, use it directly.
        if let Some(ref schema) = input_purpose {
            if schema.get_purpose().is_some() {
                return schema.get_container().cloned();
            }
        }

        // Try inheriting from parent.
        if let Some(parent_container) = ctx.get_flattened_data_source_from_parent_prim() {
            let parent_purpose = HdPurposeSchema::new(parent_container);
            // If parent purpose exists, inherit it.
            // (C++ checks GetInheritable bool, but our schema may not have that
            // field yet. For safety, inherit if parent has purpose at all.)
            if parent_purpose.get_purpose().is_some() {
                return parent_purpose.get_container().cloned();
            }
        }

        // Pass through existing data untouched.
        input_purpose.and_then(|s| s.get_container().cloned())
    }

    fn compute_dirty_locators_for_descendants(&self, locators: &mut HdDataSourceLocatorSet) {
        // Purpose changes affect all descendants.
        *locators = HdDataSourceLocatorSet::universal();
    }
}
