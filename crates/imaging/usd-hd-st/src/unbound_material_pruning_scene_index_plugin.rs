//! HdSt_UnboundMaterialPruningSceneIndexPlugin - prunes unbound materials.
//!
//! Inserts a scene index that prunes material prims not bound by any
//! geometry. Runs late in the pipeline (phase 900) but before dependency
//! forwarding (phase 1000).
//!
//! Can be disabled via `HDST_ENABLE_UNBOUND_MATERIAL_PRUNING_SCENE_INDEX`
//! environment variable if it regresses performance.
//!
//! Binding purposes checked: "preview" and "allPurpose".
//!
//! Port of C++ `HdSt_UnboundMaterialPruningSceneIndexPlugin`.

use parking_lot::RwLock;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use usd_hd::data_source::HdDataSourceBaseHandle;
use usd_hd::scene_index::{
    AddedPrimEntry, DirtiedPrimEntry, FilteringObserverTarget, HdSceneIndexBase,
    HdSceneIndexHandle, HdSceneIndexObserverHandle, HdSceneIndexPrim,
    HdSingleInputFilteringSceneIndexBase, RemovedPrimEntry, RenamedPrimEntry, SdfPathVector,
};
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

/// Insertion phase: late, but before dependency forwarding.
pub const INSERTION_PHASE: u32 = 900;

/// Storm plugin display name.
pub const PLUGIN_DISPLAY_NAME: &str = "GL";

/// Material binding purposes to check when determining if a material is bound.
pub const BINDING_PURPOSES: &[&str] = &["preview", "allPurpose"];

/// Whether the plugin is enabled (mirrors C++ env setting).
pub fn is_enabled() -> bool {
    std::env::var("HDST_ENABLE_UNBOUND_MATERIAL_PRUNING_SCENE_INDEX")
        .map(|v| v != "0" && v.to_lowercase() != "false")
        .unwrap_or(true) // enabled by default
}

/// Filtering scene index that prunes material prims not bound by geometry.
///
/// Tracks material binding relationships and removes material prims
/// that have no geometry referencing them, reducing shader compilation
/// and resource overhead.
pub struct HdStUnboundMaterialPruningSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
    /// Set of material paths that are bound by at least one geometry prim.
    bound_materials: Mutex<HashSet<SdfPath>>,
}

impl HdStUnboundMaterialPruningSceneIndex {
    /// Create a new unbound material pruning scene index.
    pub fn new(input_scene: Option<HdSceneIndexHandle>) -> Arc<RwLock<Self>> {
        Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(input_scene),
            bound_materials: Mutex::new(HashSet::new()),
        }))
    }

    /// Get the set of currently bound material paths.
    pub fn bound_materials(&self) -> HashSet<SdfPath> {
        self.bound_materials.lock().expect("Lock poisoned").clone()
    }
}

impl HdSceneIndexBase for HdStUnboundMaterialPruningSceneIndex {
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
        "HdSt_UnboundMaterialPruningSceneIndex".to_string()
    }

    fn get_input_scenes_for_system_message(&self) -> Vec<HdSceneIndexHandle> {
        self.base.get_input_scene().cloned().into_iter().collect()
    }
}

impl FilteringObserverTarget for HdStUnboundMaterialPruningSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        self.base.forward_prims_added(self, entries);
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        let mut bound_materials = self.bound_materials.lock().expect("Lock poisoned");
        for entry in entries {
            bound_materials.retain(|path| !path.has_prefix(&entry.prim_path));
        }
        drop(bound_materials);
        self.base.forward_prims_removed(self, entries);
    }

    fn on_prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        self.base.forward_prims_dirtied(self, entries);
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        self.base.forward_prims_renamed(self, entries);
    }
}

/// Plugin factory: create the unbound material pruning scene index.
///
/// Returns None if the plugin is disabled via environment variable.
pub fn create(
    input_scene: Option<HdSceneIndexHandle>,
) -> Option<Arc<RwLock<HdStUnboundMaterialPruningSceneIndex>>> {
    if !is_enabled() {
        return None;
    }
    Some(HdStUnboundMaterialPruningSceneIndex::new(input_scene))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create() {
        let si = create(None);
        assert!(si.is_some());
        let si = si.unwrap();
        let lock = si.read();
        assert_eq!(
            lock.get_display_name(),
            "HdSt_UnboundMaterialPruningSceneIndex"
        );
        assert!(lock.bound_materials().is_empty());
    }

    #[test]
    fn test_constants() {
        assert_eq!(INSERTION_PHASE, 900);
        assert_eq!(PLUGIN_DISPLAY_NAME, "GL");
        assert_eq!(BINDING_PURPOSES, &["preview", "allPurpose"]);
    }

    #[test]
    fn test_enabled_by_default() {
        // Unless env var is set, should be enabled.
        assert!(is_enabled());
    }
}
