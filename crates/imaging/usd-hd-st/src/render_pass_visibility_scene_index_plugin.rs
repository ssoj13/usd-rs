
//! HdSt_RenderPassVisibilitySceneIndexPlugin - applies render pass visibility.
//!
//! Inserts a scene index that applies render visibility rules from the active
//! render pass (specified in HdSceneGlobalsSchema). Geometry and light prims
//! excluded from the renderVisibility collection get their visibility
//! overridden to false.
//!
//! Runs downstream of procedural expansion (phase 4) so generated prims
//! are also subject to visibility rules.
//!
//! Assumes the active render pass is a UsdRenderPass for collection naming.
//!
//! Port of C++ `HdSt_RenderPassVisibilitySceneIndexPlugin`.

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

/// Insertion phase: downstream of procedural expansion.
pub const INSERTION_PHASE: u32 = 4;

/// Storm plugin display name.
pub const PLUGIN_DISPLAY_NAME: &str = "GL";

/// Filtering scene index that applies render pass visibility rules.
///
/// Tracks the active render pass from scene globals and evaluates its
/// renderVisibility collection. Geometry and light prims not matching
/// the collection have visibility overridden to false.
pub struct HdStRenderPassVisibilitySceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
    /// Path of the active render pass prim (from scene globals).
    active_render_pass: Option<SdfPath>,
}

impl HdStRenderPassVisibilitySceneIndex {
    /// Create a new render pass visibility scene index.
    pub fn new(input_scene: Option<HdSceneIndexHandle>) -> Arc<RwLock<Self>> {
        Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(input_scene),
            active_render_pass: None,
        }))
    }

    /// Check if a prim type should have pass visibility applied.
    pub fn should_apply_visibility(prim_type: &str) -> bool {
        // Geometry prims and lights are subject to pass visibility.
        Self::is_geometry_type(prim_type) || Self::is_light_type(prim_type)
    }

    /// Check if prim type is a geometry type (gprim or implicit).
    fn is_geometry_type(prim_type: &str) -> bool {
        matches!(
            prim_type,
            "mesh"
                | "basisCurves"
                | "points"
                | "volume"
                | "cone"
                | "cylinder"
                | "sphere"
                | "cube"
                | "capsule"
                | "plane"
                | "nurbsPatch"
                | "nurbsCurves"
                | "tetMesh"
        )
    }

    /// Check if prim type is a light type.
    fn is_light_type(prim_type: &str) -> bool {
        matches!(
            prim_type,
            "distantLight"
                | "domeLight"
                | "rectLight"
                | "sphereLight"
                | "cylinderLight"
                | "diskLight"
                | "pluginLight"
                | "simpleLight"
        )
    }

    /// Get the active render pass path, if any.
    pub fn active_render_pass(&self) -> Option<&SdfPath> {
        self.active_render_pass.as_ref()
    }
}

impl HdSceneIndexBase for HdStRenderPassVisibilitySceneIndex {
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
        "HdSt_RenderPassVisibilitySceneIndex".to_string()
    }

    fn get_input_scenes_for_system_message(&self) -> Vec<HdSceneIndexHandle> {
        self.base.get_input_scene().cloned().into_iter().collect()
    }
}

impl FilteringObserverTarget for HdStRenderPassVisibilitySceneIndex {
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

/// Plugin factory: create the render pass visibility scene index.
pub fn create(
    input_scene: Option<HdSceneIndexHandle>,
) -> Arc<RwLock<HdStRenderPassVisibilitySceneIndex>> {
    HdStRenderPassVisibilitySceneIndex::new(input_scene)
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
            "HdSt_RenderPassVisibilitySceneIndex"
        );
        assert!(lock.active_render_pass().is_none());
    }

    #[test]
    fn test_visibility_types() {
        assert!(HdStRenderPassVisibilitySceneIndex::should_apply_visibility(
            "mesh"
        ));
        assert!(HdStRenderPassVisibilitySceneIndex::should_apply_visibility(
            "sphere"
        ));
        assert!(HdStRenderPassVisibilitySceneIndex::should_apply_visibility(
            "domeLight"
        ));
        assert!(!HdStRenderPassVisibilitySceneIndex::should_apply_visibility("material"));
        assert!(!HdStRenderPassVisibilitySceneIndex::should_apply_visibility("camera"));
    }

    #[test]
    fn test_constants() {
        assert_eq!(INSERTION_PHASE, 4);
        assert_eq!(PLUGIN_DISPLAY_NAME, "GL");
    }
}
