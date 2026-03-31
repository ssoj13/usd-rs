
//! HdLegacyPrimSceneIndex - Retained scene index with legacy prim support.
//!
//! Extends HdRetainedSceneIndex to instantiate and dirty HdDataSourceLegacyPrim
//! data sources. During emulation of legacy HdSceneDelegates, HdRenderIndex
//! forwards prim insertion calls here to produce a comparable HdSceneIndex
//! representation.
//!
//! Corresponds to pxr/imaging/hd/legacyPrimSceneIndex.h/cpp.

use super::base::{HdSceneIndexBase, SdfPathVector};
use super::observer::{DirtiedPrimEntry, HdSceneIndexObserverHandle, RemovedPrimEntry};
use super::prim::HdSceneIndexPrim;
use super::retained::{HdRetainedSceneIndex, RetainedAddedPrimEntry};
use crate::data_source::HdDataSourceLegacyPrim;
use crate::data_source::HdRetainedContainerDataSource;
use crate::data_source::{HdDataSourceLegacyTaskPrim, HdLegacyTaskFactoryHandle};
use crate::prim::HdSceneDelegate;
use parking_lot::RwLock;
use std::sync::Arc;
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;

/// Scene index that instantiates HdDataSourceLegacyPrim for legacy delegate emulation.
///
/// This is the bridge from pull-based (HdSceneDelegate) to push-based (HdSceneIndex).
/// HdRenderIndex calls AddLegacyPrim for each prim from a legacy scene delegate,
/// and this scene index creates HdDataSourceLegacyPrim data sources that lazily
/// query the delegate when the scene index pipeline reads them.
///
/// Corresponds to C++ `HdLegacyPrimSceneIndex`.
pub struct HdLegacyPrimSceneIndex {
    retained: Arc<RwLock<HdRetainedSceneIndex>>,
}

impl HdLegacyPrimSceneIndex {
    /// Create new legacy prim scene index.
    pub fn new() -> Arc<RwLock<Self>> {
        Arc::new(RwLock::new(Self {
            retained: HdRetainedSceneIndex::new(),
        }))
    }

    /// Add legacy prim (called by HdRenderIndex during legacy delegate population).
    ///
    /// Creates an HdDataSourceLegacyPrim wrapping the scene delegate, which will
    /// lazily call through to delegate methods when queried.
    pub fn add_legacy_prim(
        &mut self,
        id: SdfPath,
        prim_type: TfToken,
        scene_delegate: Option<Arc<dyn HdSceneDelegate + Send + Sync>>,
    ) {
        // Create the legacy prim data source that wraps the delegate
        let ds = HdDataSourceLegacyPrim::new(id.clone(), prim_type.clone(), scene_delegate);
        let entry = RetainedAddedPrimEntry::new(id, prim_type, Some(ds));
        self.retained.write().add_prims(&[entry]);
    }

    /// Add legacy task (called by HdRenderIndex::InsertTask during legacy delegate population).
    pub fn add_legacy_task(
        &mut self,
        id: SdfPath,
        scene_delegate: Option<Arc<dyn HdSceneDelegate + Send + Sync>>,
        factory: Option<HdLegacyTaskFactoryHandle>,
    ) {
        let ds = HdDataSourceLegacyTaskPrim::new(id.clone(), scene_delegate, factory);
        let entry = RetainedAddedPrimEntry::new(id, TfToken::new("task"), Some(ds));
        self.retained.write().add_prims(&[entry]);
    }

    /// Remove only the prim at `id` without affecting children.
    ///
    /// If `id` has children, it is replaced by an entry with no type and an
    /// empty data source (preserving the parent slot). If `id` does not have
    /// children, it is fully removed from the retained scene index.
    ///
    /// This emulates the original HdRenderIndex Remove{B,R,S}Prim behavior
    /// which did not remove children.
    pub fn remove_prim(&mut self, id: SdfPath) {
        let retained = self.retained.read();
        let children = retained.get_child_prim_paths(&id);
        drop(retained);

        if !children.is_empty() {
            // Has children: replace with empty entry (preserve parent slot)
            let empty_ds = HdRetainedContainerDataSource::new_empty();
            let entry = RetainedAddedPrimEntry::new(id, TfToken::new(""), Some(empty_ds));
            self.retained.write().add_prims(&[entry]);
        } else {
            // No children: fully remove
            let entry = RemovedPrimEntry::new(id);
            let entries = vec![entry];
            self.retained.write().remove_prims(&entries);
        }
    }

    /// Dirty prims - extends HdRetainedSceneIndex::DirtyPrims to also call
    /// PrimDirtied on HdDataSourceLegacyPrim data sources.
    ///
    /// For each entry, if the dirty locators intersect cached locators, we
    /// find the HdDataSourceLegacyPrim and invalidate its caches. Then we
    /// forward the dirty notification to the retained scene index.
    pub fn dirty_prims(&mut self, entries: &[DirtiedPrimEntry]) {
        let cached_locators = HdDataSourceLegacyPrim::get_cached_locators();

        // First pass: invalidate caches on legacy data sources
        {
            let retained = self.retained.read();
            for entry in entries {
                if !entry.dirty_locators.intersects(cached_locators) {
                    // None of the locators are cached by the data source,
                    // so PrimDirtied would be a no-op - skip the lookup.
                    continue;
                }

                let prim = retained.get_prim(&entry.prim_path);
                if let Some(ds) = &prim.data_source {
                    // Try to downcast to HdDataSourceLegacyPrim
                    if let Some(legacy_ds) = ds.as_any().downcast_ref::<HdDataSourceLegacyPrim>() {
                        legacy_ds.prim_dirtied(&entry.dirty_locators);
                    }
                }
            }
        }

        // Second pass: forward to retained scene index for observer notification
        let entries_vec: Vec<_> = entries.to_vec();
        self.retained.write().dirty_prims(&entries_vec);
    }
}

impl HdSceneIndexBase for HdLegacyPrimSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        let inner = super::base::rwlock_data_ref(self.retained.as_ref());
        inner.get_prim(prim_path)
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        let inner = super::base::rwlock_data_ref(self.retained.as_ref());
        inner.get_child_prim_paths(prim_path)
    }

    fn add_observer(&self, observer: HdSceneIndexObserverHandle) {
        self.retained.read().add_observer(observer);
    }

    fn remove_observer(&self, observer: &HdSceneIndexObserverHandle) {
        self.retained.read().remove_observer(observer);
    }
}
