
//! HdFlattenedDataSourceProvider - Provider for flattening hierarchical data.
//!
//! Corresponds to pxr/imaging/hd/flattenedDataSourceProvider.h.
//!
//! Given to HdFlatteningSceneIndex to determine how to compute the flattened
//! data source at each prim from inherited state.

use crate::data_source::{HdContainerDataSourceHandle, HdDataSourceLocatorSet, cast_to_container};
use crate::scene_index::flattening::HdFlatteningSceneIndex;
use crate::scene_index::base::HdSceneIndexBase;
use crate::scene_index::prim::HdSceneIndexPrim;
use parking_lot::RwLock;
use std::sync::{Arc, Weak};
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

/// Handle to a flattened data source provider.
pub type HdFlattenedDataSourceProviderHandle = Arc<dyn HdFlattenedDataSourceProvider + Send + Sync>;

/// Vector of provider handles.
pub type HdFlattenedDataSourceProviderVector = Vec<HdFlattenedDataSourceProviderHandle>;

/// Context passed to the provider when computing flattened data.
///
/// Corresponds to C++ `HdFlattenedDataSourceProvider::Context`.
pub struct HdFlattenedDataSourceProviderContext<'a> {
    /// The flattening scene index (for hierarchy traversal).
    pub flattening_scene_index: &'a dyn HdSceneIndexBase,
    /// Weak handle to the concrete flattening scene index.
    pub flattening_scene_index_weak: Weak<RwLock<HdFlatteningSceneIndex>>,
    /// Path of the prim being flattened.
    pub prim_path: &'a SdfPath,
    /// Name of the data source being flattened (e.g. "xform", "visibility").
    pub name: &'a Token,
    /// Input prim from the input scene index.
    pub input_prim: &'a HdSceneIndexPrim,
}

impl<'a> HdFlattenedDataSourceProviderContext<'a> {
    /// Get the type of the prim from the input scene index.
    pub fn get_input_prim_type(&self) -> &Token {
        &self.input_prim.prim_type
    }

    /// Get input data source for this locator from the prim.
    pub fn get_input_data_source(&self) -> Option<HdContainerDataSourceHandle> {
        let ds: &HdContainerDataSourceHandle = self.input_prim.data_source.as_ref()?;
        let child = ds.get(self.name)?;
        cast_to_container(&child)
    }

    /// Get flattened data source from the parent prim.
    ///
    /// Used when composing inherited state (e.g. parent transform for xform).
    pub fn get_flattened_data_source_from_parent_prim(
        &self,
    ) -> Option<HdContainerDataSourceHandle> {
        let parent_path = self.prim_path.get_parent_path();
        if parent_path == *self.prim_path {
            return None;
        }
        let parent_prim = self.flattening_scene_index.get_prim(&parent_path);
        if !parent_prim.is_defined() {
            return None;
        }
        let ds: &HdContainerDataSourceHandle = parent_prim.data_source.as_ref()?;
        let child = ds.get(self.name)?;
        cast_to_container(&child)
    }

    /// Get the flattened parent prim container itself.
    ///
    /// Callers that need live parent state across invalidation should prefer
    /// this over caching a specific child container snapshot.
    pub fn get_flattened_parent_prim_container(&self) -> Option<HdContainerDataSourceHandle> {
        let parent_path = self.prim_path.get_parent_path();
        if parent_path == *self.prim_path {
            return None;
        }
        let parent_prim = self.flattening_scene_index.get_prim(&parent_path);
        parent_prim.data_source
    }

    /// Returns a weak handle to the concrete flattening scene index.
    pub fn get_flattening_scene_index_weak(&self) -> Weak<RwLock<HdFlatteningSceneIndex>> {
        self.flattening_scene_index_weak.clone()
    }
}

/// Provider that computes flattened (composed) data sources for a prim.
///
/// Implementations include:
/// - HdFlattenedXformDataSourceProvider
/// - HdFlattenedVisibilityDataSourceProvider
/// - HdFlattenedPurposeDataSourceProvider
/// - HdFlattenedPrimvarsDataSourceProvider
pub trait HdFlattenedDataSourceProvider: Send + Sync {
    /// Returns the flattened data source for the given context.
    fn get_flattened_data_source(
        &self,
        ctx: &HdFlattenedDataSourceProviderContext<'_>,
    ) -> Option<HdContainerDataSourceHandle>;

    /// Compute dirty locators for descendants when input is dirtied.
    ///
    /// When a prim is dirtied, this extracts locators relevant to this provider
    /// (relative to the input data source) and may expand or filter them.
    /// The result is used to invalidate flattened data in descendants.
    fn compute_dirty_locators_for_descendants(&self, locators: &mut HdDataSourceLocatorSet);
}
