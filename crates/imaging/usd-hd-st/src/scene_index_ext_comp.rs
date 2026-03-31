
//! HdSt_ExtCompSceneIndex - external computation data processing.
//!
//! Filtering scene index that processes external computation (ExtComputation)
//! data for Storm. ExtComputations allow GPU or CPU computations to produce
//! primvar data consumed by other prims during rendering.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use parking_lot::RwLock;
use usd_hd::data_source::{HdDataSourceBaseHandle, HdDataSourceLocator};
use usd_hd::scene_index::{
    AddedPrimEntry, DirtiedPrimEntry, FilteringObserverTarget, HdSceneIndexBase,
    HdSceneIndexHandle, HdSceneIndexObserverHandle, HdSceneIndexPrim,
    HdSingleInputFilteringSceneIndexBase, RemovedPrimEntry, RenamedPrimEntry, SdfPathVector,
};
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

/// External computation scene index for Storm.
///
/// Processes ExtComputation prims, managing the flow of computed data
/// between computation prims and the prims that consume their outputs.
/// Handles both CPU and GPU computation scheduling.
///
/// Port of C++ `HdStExtComputation` integration into scene index pattern.
pub struct HdStExtCompSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
    /// Tracked computation prim paths
    computation_paths: Mutex<HashSet<SdfPath>>,
}

impl HdStExtCompSceneIndex {
    /// Create a new ext computation scene index.
    pub fn new(input_scene: Option<HdSceneIndexHandle>) -> Arc<RwLock<Self>> {
        Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(input_scene),
            computation_paths: Mutex::new(HashSet::new()),
        }))
    }

    /// Check if prim is an ext computation type.
    fn is_computation(prim_type: &Token) -> bool {
        prim_type == "extComputation" || prim_type == "ExtComputation"
    }

    /// Get all tracked computation paths.
    pub fn get_computation_paths(&self) -> HashSet<SdfPath> {
        self.computation_paths
            .lock()
            .expect("Lock poisoned")
            .clone()
    }
}

impl HdSceneIndexBase for HdStExtCompSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        if let Some(input) = self.base.get_input_scene() {
            { let input_lock = input.read();
                return input_lock.get_prim(prim_path);
            }
        }
        HdSceneIndexPrim::empty()
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        if let Some(input) = self.base.get_input_scene() {
            { let input_lock = input.read();
                return input_lock.get_child_prim_paths(prim_path);
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

    fn _system_message(&self, _msg: &Token, _args: Option<HdDataSourceBaseHandle>) {}

    fn get_display_name(&self) -> String {
        "HdSt_ExtCompSceneIndex".to_string()
    }

    fn get_input_scenes_for_system_message(&self) -> Vec<HdSceneIndexHandle> {
        self.base.get_input_scene().cloned().into_iter().collect()
    }
}

impl FilteringObserverTarget for HdStExtCompSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        let mut computation_paths = self.computation_paths.lock().expect("Lock poisoned");
        for entry in entries {
            if Self::is_computation(&entry.prim_type) {
                computation_paths.insert(entry.prim_path.clone());
            }
        }
        drop(computation_paths);
        self.base.forward_prims_added(self, entries);
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        let mut computation_paths = self.computation_paths.lock().expect("Lock poisoned");
        for entry in entries {
            computation_paths.retain(|path| !path.has_prefix(&entry.prim_path));
        }
        drop(computation_paths);
        self.base.forward_prims_removed(self, entries);
    }

    fn on_prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        let comp_locator = HdDataSourceLocator::from_token(Token::new("extComputation"));
        let mut augmented = Vec::new();

        for entry in entries {
            if self
                .computation_paths
                .lock()
                .expect("Lock poisoned")
                .contains(&entry.prim_path)
                && entry.dirty_locators.intersects_locator(&comp_locator)
            {
                // ExtComputation data changed - mark outputs as dirty
                let mut locs = entry.dirty_locators.clone();
                locs.insert(HdDataSourceLocator::from_token(Token::new(
                    "extComputationOutputs",
                )));
                augmented.push(DirtiedPrimEntry::new(entry.prim_path.clone(), locs));
            } else {
                augmented.push(entry.clone());
            }
        }

        self.base.forward_prims_dirtied(self, &augmented);
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        self.base.forward_prims_renamed(self, entries);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create() {
        let si = HdStExtCompSceneIndex::new(None);
        let lock = si.read();
        assert_eq!(lock.get_display_name(), "HdSt_ExtCompSceneIndex");
        assert!(lock.get_computation_paths().is_empty());
    }
}
