//! HdSt_DrawTargetSceneIndex - render-to-texture target management.
//!
//! Filtering scene index that manages draw targets (render-to-texture).
//! Draw targets allow rendering to offscreen textures that can then be
//! used as inputs to materials on other prims.

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

/// Draw target info tracked per prim.
#[derive(Clone, Debug)]
#[allow(dead_code)]
struct DrawTargetInfo {
    /// Resolution of the draw target texture
    pub resolution: [u32; 2],
    /// Whether this draw target is enabled
    pub enabled: bool,
    /// Camera path for the draw target
    pub camera: SdfPath,
    /// Collection of prims to render into this target
    pub collection: SdfPath,
}

/// Draw target scene index for Storm.
///
/// Tracks draw target prims and manages their state. Draw targets
/// are off-screen render targets used for render-to-texture effects
/// like reflection maps, shadow maps, and custom render passes.
///
/// Port of C++ `HdStDrawTarget` integration into scene index pattern.
pub struct HdStDrawTargetSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
    /// Tracked draw targets by path
    draw_targets: Mutex<HashMap<SdfPath, DrawTargetInfo>>,
}

impl HdStDrawTargetSceneIndex {
    /// Create a new draw target scene index.
    pub fn new(input_scene: Option<HdSceneIndexHandle>) -> Arc<RwLock<Self>> {
        Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(input_scene),
            draw_targets: Mutex::new(HashMap::new()),
        }))
    }

    /// Check if prim is a draw target type.
    fn is_draw_target(prim_type: &Token) -> bool {
        prim_type == "drawTarget" || prim_type == "DrawTarget"
    }

    /// Get all tracked draw target paths.
    pub fn get_draw_target_paths(&self) -> Vec<SdfPath> {
        self.draw_targets
            .lock()
            .expect("Lock poisoned")
            .keys()
            .cloned()
            .collect()
    }
}

impl HdSceneIndexBase for HdStDrawTargetSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        if let Some(input) = self.base.get_input_scene() {
            {
                let input_lock = input.read();
                return input_lock.get_prim(prim_path);
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
        "HdSt_DrawTargetSceneIndex".to_string()
    }

    fn get_input_scenes_for_system_message(&self) -> Vec<HdSceneIndexHandle> {
        self.base.get_input_scene().cloned().into_iter().collect()
    }
}

impl FilteringObserverTarget for HdStDrawTargetSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        // Track new draw target prims
        let mut draw_targets = self.draw_targets.lock().expect("Lock poisoned");
        for entry in entries {
            if Self::is_draw_target(&entry.prim_type) {
                draw_targets.insert(
                    entry.prim_path.clone(),
                    DrawTargetInfo {
                        resolution: [512, 512],
                        enabled: true,
                        camera: SdfPath::default(),
                        collection: SdfPath::default(),
                    },
                );
            }
        }
        drop(draw_targets);
        self.base.forward_prims_added(self, entries);
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        let mut draw_targets = self.draw_targets.lock().expect("Lock poisoned");
        for entry in entries {
            // Remove draw targets whose paths fall under the removed subtree
            draw_targets.retain(|path, _| !path.has_prefix(&entry.prim_path));
        }
        drop(draw_targets);
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
        let si = HdStDrawTargetSceneIndex::new(None);
        let lock = si.read();
        assert_eq!(lock.get_display_name(), "HdSt_DrawTargetSceneIndex");
        assert!(lock.get_draw_target_paths().is_empty());
    }
}
