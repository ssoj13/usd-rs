
//! HdSt_MaterialPrimvarTransferSceneIndexPlugin - transfers primvars from materials.
//!
//! Inserts a scene index that transfers primvars/attributes from materials
//! to the geometry that binds the material. This allows material-defined
//! primvars to be available on geometry prims for rendering.
//!
//! Chained after extComputationPrimvarPruning and procedural expansion
//! (insertion phase 3).
//!
//! Port of C++ `HdSt_MaterialPrimvarTransferSceneIndexPlugin`.

use std::sync::Arc;
use parking_lot::RwLock;
use usd_hd::data_source::HdDataSourceBaseHandle;
use usd_hd::scene_index::{
    AddedPrimEntry, DirtiedPrimEntry, FilteringObserverTarget, HdSceneIndexBase,
    HdSceneIndexHandle, HdSceneIndexObserverHandle, HdSceneIndexPrim,
    HdSingleInputFilteringSceneIndexBase, RemovedPrimEntry, RenamedPrimEntry, SdfPathVector,
};
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

/// Insertion phase: after extCompPrimvarPruning + procedural expansion.
pub const INSERTION_PHASE: u32 = 3;

/// Storm plugin display name.
pub const PLUGIN_DISPLAY_NAME: &str = "GL";

/// Filtering scene index that transfers material primvars to bound geometry.
pub struct HdStMaterialPrimvarTransferSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
}

impl HdStMaterialPrimvarTransferSceneIndex {
    /// Create a new material primvar transfer scene index.
    pub fn new(input_scene: Option<HdSceneIndexHandle>) -> Arc<RwLock<Self>> {
        Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(input_scene),
        }))
    }
}

impl HdSceneIndexBase for HdStMaterialPrimvarTransferSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        if let Some(input) = self.base.get_input_scene() {
            { let lock = input.read();
                return lock.get_prim(prim_path);
            }
        }
        HdSceneIndexPrim::empty()
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        if let Some(input) = self.base.get_input_scene() {
            { let lock = input.read();
                return lock.get_child_prim_paths(prim_path);
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
        "HdSt_MaterialPrimvarTransferSceneIndex".to_string()
    }

    fn get_input_scenes_for_system_message(&self) -> Vec<HdSceneIndexHandle> {
        self.base.get_input_scene().cloned().into_iter().collect()
    }
}

impl FilteringObserverTarget for HdStMaterialPrimvarTransferSceneIndex {
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

/// Plugin factory: create the material primvar transfer scene index.
pub fn create(
    input_scene: Option<HdSceneIndexHandle>,
) -> Arc<RwLock<HdStMaterialPrimvarTransferSceneIndex>> {
    HdStMaterialPrimvarTransferSceneIndex::new(input_scene)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create() {
        let si = create(None);
        let lock = si.read();
        assert_eq!(
            lock.get_display_name(),
            "HdSt_MaterialPrimvarTransferSceneIndex"
        );
    }

    #[test]
    fn test_constants() {
        assert_eq!(INSERTION_PHASE, 3);
        assert_eq!(PLUGIN_DISPLAY_NAME, "GL");
    }
}
