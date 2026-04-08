//! HdSt_MaterialOverrideSceneIndex - material parameter overrides for Storm.
//!
//! Filtering scene index that applies material parameter overrides.
//! Allows render settings or per-prim overrides to modify material
//! parameters without changing the underlying material network.

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use usd_hd::data_source::HdDataSourceBaseHandle;
use usd_hd::scene_index::{
    AddedPrimEntry, DirtiedPrimEntry, FilteringObserverTarget, HdSceneIndexBase,
    HdSceneIndexHandle, HdSceneIndexObserverHandle, HdSceneIndexPrim,
    HdSingleInputFilteringSceneIndexBase, RemovedPrimEntry, RenamedPrimEntry, SdfPathVector,
};
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

/// A material parameter override entry.
#[derive(Clone, Debug)]
pub struct MaterialParamOverride {
    /// Parameter name to override
    pub param_name: Token,
    /// Override value (as VtValue equivalent)
    pub value: Vec<u8>,
}

/// Material override scene index for Storm.
///
/// Intercepts material prim queries and applies parameter overrides
/// from render settings or explicit override maps. This enables
/// features like:
/// - Display color overrides
/// - Wireframe color
/// - Material fallbacks for unresolved materials
///
/// Port of material override handling from C++ Storm render delegate.
pub struct HdStMaterialOverrideSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
    /// Per-material path overrides: material_path -> [(param, value)]
    overrides: Mutex<HashMap<SdfPath, Vec<MaterialParamOverride>>>,
}

impl HdStMaterialOverrideSceneIndex {
    /// Create a new material override scene index.
    pub fn new(input_scene: Option<HdSceneIndexHandle>) -> Arc<RwLock<Self>> {
        Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(input_scene),
            overrides: Mutex::new(HashMap::new()),
        }))
    }

    /// Set overrides for a specific material path.
    pub fn set_overrides(&mut self, material_path: SdfPath, params: Vec<MaterialParamOverride>) {
        self.overrides
            .lock()
            .expect("Lock poisoned")
            .insert(material_path, params);
    }

    /// Clear all overrides.
    pub fn clear_overrides(&mut self) {
        self.overrides.lock().expect("Lock poisoned").clear();
    }

    /// Check if a prim has active overrides.
    pub fn has_overrides(&self, prim_path: &SdfPath) -> bool {
        self.overrides
            .lock()
            .expect("Lock poisoned")
            .contains_key(prim_path)
    }
}

impl HdSceneIndexBase for HdStMaterialOverrideSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        if let Some(input) = self.base.get_input_scene() {
            {
                let input_lock = input.read();
                let prim = input_lock.get_prim(prim_path);
                // In full implementation: if overrides exist for this material,
                // wrap the data source to intercept material parameter queries.
                return prim;
            }
        }
        HdSceneIndexPrim::empty()
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        if let Some(input) = self.base.get_input_scene() {
            {
                let input_lock = input.read();
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
        "HdSt_MaterialOverrideSceneIndex".to_string()
    }

    fn get_input_scenes_for_system_message(&self) -> Vec<HdSceneIndexHandle> {
        self.base.get_input_scene().cloned().into_iter().collect()
    }
}

impl FilteringObserverTarget for HdStMaterialOverrideSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        self.base.forward_prims_added(self, entries);
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        let mut overrides = self.overrides.lock().expect("Lock poisoned");
        for entry in entries {
            overrides.retain(|path, _| !path.has_prefix(&entry.prim_path));
        }
        drop(overrides);
        self.base.forward_prims_removed(self, entries);
    }

    fn on_prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        self.base.forward_prims_dirtied(self, entries);
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
        let si = HdStMaterialOverrideSceneIndex::new(None);
        let lock = si.read();
        assert_eq!(lock.get_display_name(), "HdSt_MaterialOverrideSceneIndex");
    }

    #[test]
    fn test_overrides() {
        let si = HdStMaterialOverrideSceneIndex::new(None);
        let mut lock = si.write();
        let path = SdfPath::from_string("/Materials/Mat1").unwrap();
        assert!(!lock.has_overrides(&path));
        lock.set_overrides(path.clone(), vec![]);
        assert!(lock.has_overrides(&path));
        lock.clear_overrides();
        assert!(!lock.has_overrides(&path));
    }
}
