//! HdLegacyGeomSubsetSceneIndex - Converts legacy geom subsets to geomSubset prims.
//!
//! Corresponds to pxr/imaging/hd/legacyGeomSubsetSceneIndex.h.

use super::base::{HdSceneIndexBase, HdSceneIndexHandle, SdfPathVector};
use super::filtering::{FilteringObserverTarget, HdSingleInputFilteringSceneIndexBase};
use super::observer::{
    AddedPrimEntry, DirtiedPrimEntry, HdSceneIndexObserverHandle, RemovedPrimEntry,
    RenamedPrimEntry,
};
use super::prim::HdSceneIndexPrim;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use usd_sdf::Path as SdfPath;

/// Converts legacy mesh/basisCurves geom subsets into Hydra geomSubset prims.
///
/// Corresponds to C++ `HdLegacyGeomSubsetSceneIndex`.
pub struct HdLegacyGeomSubsetSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
    parent_prims: RwLock<HashMap<SdfPath, Vec<SdfPath>>>,
}

impl HdLegacyGeomSubsetSceneIndex {
    /// Create new legacy geom subset scene index.
    ///
    /// C++ parity: constructor calls `_inputSceneIndex->AddObserver(this)`.
    pub fn new(input_scene: Option<HdSceneIndexHandle>) -> Arc<RwLock<Self>> {
        let input_clone = input_scene.clone();
        let result = Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(input_scene),
            parent_prims: RwLock::new(HashMap::new()),
        }));
        if let Some(input) = input_clone {
            super::filtering::wire_filter_to_input(&result, &input);
        }
        result
    }
}

impl HdSceneIndexBase for HdLegacyGeomSubsetSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        if let Some(input) = self.base.get_input_scene() {
            {
                let guard = input.read();
                return guard.get_prim(prim_path);
            }
        }
        HdSceneIndexPrim::empty()
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        if let Some(input) = self.base.get_input_scene() {
            {
                let guard = input.read();
                let input_children = guard.get_child_prim_paths(prim_path);
                {
                    let parents = self.parent_prims.read();
                    if let Some(subsets) = parents.get(prim_path) {
                        let mut result = input_children;
                        result.extend(subsets.iter().cloned());
                        return result;
                    }
                }
                return input_children;
            }
        }
        Vec::new()
    }

    fn add_observer(&self, observer: HdSceneIndexObserverHandle) {
        self.base.base().add_observer(observer);
    }

    fn remove_observer(&self, observer: &HdSceneIndexObserverHandle) {
        self.base.base().remove_observer(observer);
    }
}

impl FilteringObserverTarget for HdLegacyGeomSubsetSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        self.base.forward_prims_added(self, entries);
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        self.base.forward_prims_removed(self, entries);
    }

    fn on_prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        self.base.forward_prims_dirtied(self, entries);
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        self.base.forward_prims_renamed(self, entries);
    }
}
