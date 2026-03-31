
//! HdFlattenedVisibilityDataSourceProvider - Flattens visibility inheritance.
//!
//! Port of pxr/imaging/hd/flattenedVisibilityDataSourceProvider.cpp.
//!
//! If the prim has visibility authored, use it. Otherwise inherit from
//! the parent. If neither has it, default to visible (true).

use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceLocatorSet, HdRetainedTypedSampledDataSource,
};
use crate::flattened_data_source_provider::{
    HdFlattenedDataSourceProvider, HdFlattenedDataSourceProviderContext,
};
use crate::schema::HdVisibilitySchema;

/// Provider that flattens visibility by inheriting from parent if not authored locally.
///
/// Corresponds to C++ HdFlattenedVisibilityDataSourceProvider.
#[derive(Debug, Default)]
pub struct HdFlattenedVisibilityDataSourceProvider;

impl HdFlattenedVisibilityDataSourceProvider {
    /// Create new visibility flattening provider.
    pub fn new() -> Self {
        Self
    }
}

impl HdFlattenedDataSourceProvider for HdFlattenedVisibilityDataSourceProvider {
    fn get_flattened_data_source(
        &self,
        ctx: &HdFlattenedDataSourceProviderContext<'_>,
    ) -> Option<HdContainerDataSourceHandle> {
        // If prim has visibility authored, use it.
        if let Some(input_container) = ctx.get_input_data_source() {
            let input_vis = HdVisibilitySchema::new(input_container);
            if input_vis.get_visibility().is_some() {
                return input_vis.get_container().cloned();
            }
        }

        // Inherit from parent if available.
        if let Some(parent_container) = ctx.get_flattened_data_source_from_parent_prim() {
            let parent_vis = HdVisibilitySchema::new(parent_container);
            if parent_vis.get_visibility().is_some() {
                return parent_vis.get_container().cloned();
            }
        }

        // Default: visible.
        Some(HdVisibilitySchema::build_retained(Some(
            HdRetainedTypedSampledDataSource::new(true),
        )))
    }

    fn compute_dirty_locators_for_descendants(&self, locators: &mut HdDataSourceLocatorSet) {
        // Visibility changes affect all descendants.
        *locators = HdDataSourceLocatorSet::universal();
    }
}
