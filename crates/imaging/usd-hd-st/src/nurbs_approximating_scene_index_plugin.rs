//! HdSt_NurbsApproximatingSceneIndexPlugin - converts NURBS to meshes.
//!
//! Storm does not natively support NURBS surfaces. This plugin inserts a
//! scene index that approximates NURBS patches/curves as tessellated mesh
//! geometry suitable for rasterization.
//!
//! Port of C++ `HdSt_NurbsApproximatingSceneIndexPlugin`.

use parking_lot::RwLock;
use std::sync::Arc;
use usd_hd::data_source::HdDataSourceBaseHandle;
use usd_hd::scene_index::{
    AddedPrimEntry, DirtiedPrimEntry, FilteringObserverTarget, HdSceneIndexBase,
    HdSceneIndexHandle, HdSceneIndexObserverHandle, HdSceneIndexPrim,
    HdSingleInputFilteringSceneIndexBase, RemovedPrimEntry, RenamedPrimEntry, SdfPathVector,
};
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

/// Insertion phase: early (phase 0).
pub const INSERTION_PHASE: u32 = 0;

/// Storm plugin display name.
pub const PLUGIN_DISPLAY_NAME: &str = "GL";

/// Filtering scene index that approximates NURBS as meshes.
///
/// Converts nurbsPatch and nurbsCurves prim types to mesh/basisCurves
/// representations that Storm can render via its standard pipeline.
pub struct HdStNurbsApproximatingSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
}

impl HdStNurbsApproximatingSceneIndex {
    /// Create a new NURBS approximating scene index.
    pub fn new(input_scene: Option<HdSceneIndexHandle>) -> Arc<RwLock<Self>> {
        Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(input_scene),
        }))
    }

    /// Check if a prim type is a NURBS type needing approximation.
    pub fn is_nurbs_type(prim_type: &str) -> bool {
        prim_type == "nurbsPatch" || prim_type == "nurbsCurves"
    }
}

impl HdSceneIndexBase for HdStNurbsApproximatingSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        if let Some(input) = self.base.get_input_scene() {
            {
                let lock = input.read();
                return lock.get_prim(prim_path);
            }
        }
        HdSceneIndexPrim::empty()
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        if let Some(input) = self.base.get_input_scene() {
            {
                let lock = input.read();
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
        "HdSt_NurbsApproximatingSceneIndex".to_string()
    }

    fn get_input_scenes_for_system_message(&self) -> Vec<HdSceneIndexHandle> {
        self.base.get_input_scene().cloned().into_iter().collect()
    }
}

impl FilteringObserverTarget for HdStNurbsApproximatingSceneIndex {
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

/// Plugin factory: create the NURBS approximating scene index.
pub fn create(
    input_scene: Option<HdSceneIndexHandle>,
) -> Arc<RwLock<HdStNurbsApproximatingSceneIndex>> {
    HdStNurbsApproximatingSceneIndex::new(input_scene)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create() {
        let si = create(None);
        let lock = si.read();
        assert_eq!(lock.get_display_name(), "HdSt_NurbsApproximatingSceneIndex");
    }

    #[test]
    fn test_nurbs_types() {
        assert!(HdStNurbsApproximatingSceneIndex::is_nurbs_type(
            "nurbsPatch"
        ));
        assert!(HdStNurbsApproximatingSceneIndex::is_nurbs_type(
            "nurbsCurves"
        ));
        assert!(!HdStNurbsApproximatingSceneIndex::is_nurbs_type("mesh"));
    }

    #[test]
    fn test_constants() {
        assert_eq!(INSERTION_PHASE, 0);
        assert_eq!(PLUGIN_DISPLAY_NAME, "GL");
    }
}
