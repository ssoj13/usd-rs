//! HdFlattenedOverlayDataSourceProvider - overlay semantics for data source flattening.
//!
//! A flattened data source provider that composes a prim's own data source
//! over the flattened parent data source (overlay semantics). The prim's
//! local values take precedence over inherited values.
//! Port of pxr/imaging/hd/flattenedOverlayDataSourceProvider.h/cpp

use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceLocatorSet, HdOverlayContainerDataSource,
};
use crate::flattened_data_source_provider::{
    HdFlattenedDataSourceProvider, HdFlattenedDataSourceProviderContext,
    HdFlattenedDataSourceProviderHandle,
};
use usd_tf::Token;

/// Flattened overlay data source provider.
///
/// Composes the prim's local data source over the parent's flattened data
/// source. This gives "overlay" semantics where local values win over inherited.
///
/// Port of HdFlattenedOverlayDataSourceProvider from
/// pxr/imaging/hd/flattenedOverlayDataSourceProvider.h
pub struct HdFlattenedOverlayDataSourceProvider;

impl HdFlattenedOverlayDataSourceProvider {
    /// Create a new overlay provider.
    pub fn new() -> Self {
        Self
    }

    /// Create as a handle for use in provider vectors.
    pub fn new_handle() -> HdFlattenedDataSourceProviderHandle {
        std::sync::Arc::new(Self::new())
    }
}

impl Default for HdFlattenedOverlayDataSourceProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl HdFlattenedDataSourceProvider for HdFlattenedOverlayDataSourceProvider {
    fn get_flattened_data_source(
        &self,
        ctx: &HdFlattenedDataSourceProviderContext<'_>,
    ) -> Option<HdContainerDataSourceHandle> {
        let local = ctx.get_input_data_source();
        let parent = ctx.get_flattened_data_source_from_parent_prim();

        match (local, parent) {
            (Some(local_ds), Some(parent_ds)) => {
                // Overlay local over parent (local has priority = first arg)
                Some(HdOverlayContainerDataSource::new_2(local_ds, parent_ds))
            }
            (Some(local_ds), None) => Some(local_ds),
            (None, Some(parent_ds)) => Some(parent_ds),
            (None, None) => None,
        }
    }

    fn compute_dirty_locators_for_descendants(&self, _locators: &mut HdDataSourceLocatorSet) {
        // Overlay changes propagate to all descendants as-is (no filtering)
    }
}

/// Create the default set of flattened data source providers.
///
/// Includes providers for xform, visibility, purpose, primvars, and
/// a generic overlay provider as fallback.
///
/// Port of HdMakeFlattenedDataSourceProviders from
/// pxr/imaging/hd/flattenedDataSourceProviders.h
pub fn make_flattened_data_source_providers() -> Vec<(Token, HdFlattenedDataSourceProviderHandle)> {
    use crate::flattened_primvars_data_source_provider::HdFlattenedPrimvarsDataSourceProvider;
    use crate::flattened_purpose_data_source_provider::HdFlattenedPurposeDataSourceProvider;
    use crate::flattened_visibility_data_source_provider::HdFlattenedVisibilityDataSourceProvider;
    use crate::flattened_xform_data_source_provider::HdFlattenedXformDataSourceProvider;

    vec![
        (
            Token::new("xform"),
            std::sync::Arc::new(HdFlattenedXformDataSourceProvider::new()),
        ),
        (
            Token::new("visibility"),
            std::sync::Arc::new(HdFlattenedVisibilityDataSourceProvider::new()),
        ),
        (
            Token::new("purpose"),
            std::sync::Arc::new(HdFlattenedPurposeDataSourceProvider::new()),
        ),
        (
            Token::new("primvars"),
            std::sync::Arc::new(HdFlattenedPrimvarsDataSourceProvider::new()),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_overlay_provider_creation() {
        let _provider = HdFlattenedOverlayDataSourceProvider::new();
        let _handle = HdFlattenedOverlayDataSourceProvider::new_handle();
    }

    #[test]
    fn test_default_providers() {
        let providers = make_flattened_data_source_providers();
        assert_eq!(providers.len(), 4);
        assert_eq!(providers[0].0.as_str(), "xform");
        assert_eq!(providers[1].0.as_str(), "visibility");
        assert_eq!(providers[2].0.as_str(), "purpose");
        assert_eq!(providers[3].0.as_str(), "primvars");
    }
}
