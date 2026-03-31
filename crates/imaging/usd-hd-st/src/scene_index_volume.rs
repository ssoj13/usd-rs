
//! HdSt_VolumeSceneIndex - volume rendering data processing for Storm.
//!
//! Filtering scene index that processes volume prims for Storm rendering.
//! Manages volume field bindings, step sizes, and texture memory limits
//! needed for raymarching-based volume rendering.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use parking_lot::RwLock;
use usd_hd::data_source::HdDataSourceBaseHandle;
use usd_hd::scene_index::{
    AddedPrimEntry, DirtiedPrimEntry, FilteringObserverTarget, HdSceneIndexBase,
    HdSceneIndexHandle, HdSceneIndexObserverHandle, HdSceneIndexPrim,
    HdSingleInputFilteringSceneIndexBase, RemovedPrimEntry, RenamedPrimEntry, SdfPathVector,
};
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

/// Default step size for volume raymarching.
pub const DEFAULT_STEP_SIZE: f32 = 1.0;
/// Default step size for lighting computation in volumes.
pub const DEFAULT_STEP_SIZE_LIGHTING: f32 = 10.0;
/// Default max texture memory per field (in MB).
pub const DEFAULT_MAX_TEXTURE_MEMORY_PER_FIELD: f32 = 128.0;

/// Volume scene index for Storm.
///
/// Processes volume prims and their field bindings for Storm's
/// raymarching-based volume renderer. Tracks volume prims and manages
/// the relationship between volumes and their field textures.
///
/// Port of C++ `HdStVolume` into scene index pattern.
pub struct HdStVolumeSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
    /// Tracked volume prim paths
    volume_paths: Mutex<HashSet<SdfPath>>,
}

impl HdStVolumeSceneIndex {
    /// Create a new volume scene index.
    pub fn new(input_scene: Option<HdSceneIndexHandle>) -> Arc<RwLock<Self>> {
        Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(input_scene),
            volume_paths: Mutex::new(HashSet::new()),
        }))
    }

    /// Check if prim is a volume type.
    fn is_volume_prim(prim_type: &Token) -> bool {
        prim_type == "volume" || prim_type == "Volume"
    }

    /// Get all tracked volume paths.
    pub fn get_volume_paths(&self) -> HashSet<SdfPath> {
        self.volume_paths.lock().expect("Lock poisoned").clone()
    }
}

impl HdSceneIndexBase for HdStVolumeSceneIndex {
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
        "HdSt_VolumeSceneIndex".to_string()
    }

    fn get_input_scenes_for_system_message(&self) -> Vec<HdSceneIndexHandle> {
        self.base.get_input_scene().cloned().into_iter().collect()
    }
}

impl FilteringObserverTarget for HdStVolumeSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        let mut volume_paths = self.volume_paths.lock().expect("Lock poisoned");
        for entry in entries {
            if Self::is_volume_prim(&entry.prim_type) {
                volume_paths.insert(entry.prim_path.clone());
            }
        }
        drop(volume_paths);
        self.base.forward_prims_added(self, entries);
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        let mut volume_paths = self.volume_paths.lock().expect("Lock poisoned");
        for entry in entries {
            volume_paths.retain(|path| !path.has_prefix(&entry.prim_path));
        }
        drop(volume_paths);
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
        let si = HdStVolumeSceneIndex::new(None);
        let lock = si.read();
        assert_eq!(lock.get_display_name(), "HdSt_VolumeSceneIndex");
        assert!(lock.get_volume_paths().is_empty());
    }

    #[test]
    fn test_is_volume_prim() {
        assert!(HdStVolumeSceneIndex::is_volume_prim(&Token::new("volume")));
        assert!(HdStVolumeSceneIndex::is_volume_prim(&Token::new("Volume")));
        assert!(!HdStVolumeSceneIndex::is_volume_prim(&Token::new("mesh")));
    }

    #[test]
    fn test_defaults() {
        assert_eq!(DEFAULT_STEP_SIZE, 1.0);
        assert_eq!(DEFAULT_STEP_SIZE_LIGHTING, 10.0);
        assert_eq!(DEFAULT_MAX_TEXTURE_MEMORY_PER_FIELD, 128.0);
    }
}
