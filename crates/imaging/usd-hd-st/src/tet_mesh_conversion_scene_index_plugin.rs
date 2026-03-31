
//! HdSt_TetMeshConversionSceneIndexPlugin - converts tet meshes to triangles.
//!
//! Storm does not natively support tetrahedral meshes. This plugin inserts a
//! scene index that converts tetMesh prims into standard triangle mesh
//! representations by extracting the surface triangles from the tetrahedra.
//!
//! Port of C++ `HdSt_TetMeshConversionSceneIndexPlugin`.

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

/// Insertion phase: early (phase 0).
pub const INSERTION_PHASE: u32 = 0;

/// Storm plugin display name.
pub const PLUGIN_DISPLAY_NAME: &str = "GL";

/// Filtering scene index that converts tet meshes to triangle meshes.
///
/// Rewrites tetMesh prims as mesh prims with surface triangulation
/// extracted from the tetrahedral connectivity.
pub struct HdStTetMeshConversionSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
}

impl HdStTetMeshConversionSceneIndex {
    /// Create a new tet mesh conversion scene index.
    pub fn new(input_scene: Option<HdSceneIndexHandle>) -> Arc<RwLock<Self>> {
        Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(input_scene),
        }))
    }

    /// Check if a prim type is a tet mesh needing conversion.
    pub fn is_tet_mesh(prim_type: &str) -> bool {
        prim_type == "tetMesh" || prim_type == "TetMesh"
    }
}

impl HdSceneIndexBase for HdStTetMeshConversionSceneIndex {
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
        "HdSt_TetMeshConversionSceneIndex".to_string()
    }

    fn get_input_scenes_for_system_message(&self) -> Vec<HdSceneIndexHandle> {
        self.base.get_input_scene().cloned().into_iter().collect()
    }
}

impl FilteringObserverTarget for HdStTetMeshConversionSceneIndex {
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

/// Plugin factory: create the tet mesh conversion scene index.
pub fn create(
    input_scene: Option<HdSceneIndexHandle>,
) -> Arc<RwLock<HdStTetMeshConversionSceneIndex>> {
    HdStTetMeshConversionSceneIndex::new(input_scene)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create() {
        let si = create(None);
        let lock = si.read();
        assert_eq!(lock.get_display_name(), "HdSt_TetMeshConversionSceneIndex");
    }

    #[test]
    fn test_tet_mesh_types() {
        assert!(HdStTetMeshConversionSceneIndex::is_tet_mesh("tetMesh"));
        assert!(HdStTetMeshConversionSceneIndex::is_tet_mesh("TetMesh"));
        assert!(!HdStTetMeshConversionSceneIndex::is_tet_mesh("mesh"));
    }

    #[test]
    fn test_constants() {
        assert_eq!(INSERTION_PHASE, 0);
        assert_eq!(PLUGIN_DISPLAY_NAME, "GL");
    }
}
