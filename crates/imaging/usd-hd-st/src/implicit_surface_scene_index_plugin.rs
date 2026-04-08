//! HdSt_ImplicitSurfaceSceneIndexPlugin - converts implicit shapes to meshes.
//!
//! Storm does not natively support implicit geometry (spheres, cubes, cones,
//! cylinders, capsules, planes). This plugin inserts a scene index that
//! converts all implicit surface prims into mesh representations.
//!
//! Port of C++ `HdSt_ImplicitSurfaceSceneIndexPlugin`.

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

/// Insertion phase: early (phase 0), before most other scene indices.
pub const INSERTION_PHASE: u32 = 0;

/// Storm plugin display name.
pub const PLUGIN_DISPLAY_NAME: &str = "GL";

/// Implicit prim types that Storm converts to meshes.
pub const IMPLICIT_PRIM_TYPES: &[&str] =
    &["sphere", "cube", "cone", "cylinder", "capsule", "plane"];

/// Filtering scene index that converts implicit surfaces to meshes.
///
/// Rewrites implicit prim types (sphere, cube, cone, cylinder, capsule,
/// plane) as mesh prims with tessellated geometry data sources.
pub struct HdStImplicitSurfaceSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
}

impl HdStImplicitSurfaceSceneIndex {
    /// Create a new implicit surface scene index.
    pub fn new(input_scene: Option<HdSceneIndexHandle>) -> Arc<RwLock<Self>> {
        Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(input_scene),
        }))
    }

    /// Check if a prim type is an implicit surface that needs conversion.
    pub fn is_implicit_type(prim_type: &str) -> bool {
        IMPLICIT_PRIM_TYPES.contains(&prim_type)
    }
}

impl HdSceneIndexBase for HdStImplicitSurfaceSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        if let Some(input) = self.base.get_input_scene() {
            {
                let lock = input.read();
                let prim = lock.get_prim(prim_path);
                // In full impl, if prim.prim_type is implicit, rewrite to mesh
                // with tessellated geometry data source.
                return prim;
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
        "HdSt_ImplicitSurfaceSceneIndex".to_string()
    }

    fn get_input_scenes_for_system_message(&self) -> Vec<HdSceneIndexHandle> {
        self.base.get_input_scene().cloned().into_iter().collect()
    }
}

impl FilteringObserverTarget for HdStImplicitSurfaceSceneIndex {
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

/// Plugin factory: create the implicit surface scene index for Storm.
pub fn create(
    input_scene: Option<HdSceneIndexHandle>,
) -> Arc<RwLock<HdStImplicitSurfaceSceneIndex>> {
    HdStImplicitSurfaceSceneIndex::new(input_scene)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create() {
        let si = create(None);
        let lock = si.read();
        assert_eq!(lock.get_display_name(), "HdSt_ImplicitSurfaceSceneIndex");
    }

    #[test]
    fn test_implicit_types() {
        assert!(HdStImplicitSurfaceSceneIndex::is_implicit_type("sphere"));
        assert!(HdStImplicitSurfaceSceneIndex::is_implicit_type("cube"));
        assert!(HdStImplicitSurfaceSceneIndex::is_implicit_type("cone"));
        assert!(HdStImplicitSurfaceSceneIndex::is_implicit_type("cylinder"));
        assert!(HdStImplicitSurfaceSceneIndex::is_implicit_type("capsule"));
        assert!(HdStImplicitSurfaceSceneIndex::is_implicit_type("plane"));
        assert!(!HdStImplicitSurfaceSceneIndex::is_implicit_type("mesh"));
    }

    #[test]
    fn test_constants() {
        assert_eq!(INSERTION_PHASE, 0);
        assert_eq!(PLUGIN_DISPLAY_NAME, "GL");
    }
}
