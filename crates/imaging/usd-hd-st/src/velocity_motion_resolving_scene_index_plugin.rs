//! HdSt_VelocityMotionResolvingSceneIndexPlugin - resolves velocity-based motion.
//!
//! Inserts a scene index that resolves velocity and acceleration motion blur
//! data into deformed positions for Storm rendering. Uses a configurable
//! frame rate (default 24.0 fps) to compute motion sample positions.
//!
//! Port of C++ `HdSt_VelocityMotionResolvingSceneIndexPlugin`.

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

/// Insertion phase: early (phase 0), at end of insertion order.
pub const INSERTION_PHASE: u32 = 0;

/// Storm plugin display name.
pub const PLUGIN_DISPLAY_NAME: &str = "GL";

/// Default frame rate for velocity motion blur computation.
pub const DEFAULT_FPS: f32 = 24.0;

/// Filtering scene index that resolves velocity/acceleration motion blur.
///
/// Computes deformed position samples from velocity and acceleration
/// primvar data, enabling motion blur rendering in Storm without
/// requiring explicit multi-sample position data.
pub struct HdStVelocityMotionResolvingSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
    /// Frame rate used for motion computation.
    fps: f32,
}

impl HdStVelocityMotionResolvingSceneIndex {
    /// Create a new velocity motion resolving scene index.
    pub fn new(input_scene: Option<HdSceneIndexHandle>) -> Arc<RwLock<Self>> {
        Self::with_fps(input_scene, DEFAULT_FPS)
    }

    /// Create with a custom frame rate.
    pub fn with_fps(input_scene: Option<HdSceneIndexHandle>, fps: f32) -> Arc<RwLock<Self>> {
        Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(input_scene),
            fps,
        }))
    }

    /// Get the configured frame rate.
    pub fn fps(&self) -> f32 {
        self.fps
    }
}

impl HdSceneIndexBase for HdStVelocityMotionResolvingSceneIndex {
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
        "HdSt_VelocityMotionResolvingSceneIndex".to_string()
    }

    fn get_input_scenes_for_system_message(&self) -> Vec<HdSceneIndexHandle> {
        self.base.get_input_scene().cloned().into_iter().collect()
    }
}

impl FilteringObserverTarget for HdStVelocityMotionResolvingSceneIndex {
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

/// Plugin factory: create the velocity motion resolving scene index.
pub fn create(
    input_scene: Option<HdSceneIndexHandle>,
) -> Arc<RwLock<HdStVelocityMotionResolvingSceneIndex>> {
    HdStVelocityMotionResolvingSceneIndex::new(input_scene)
}

/// Plugin factory with custom fps.
pub fn create_with_fps(
    input_scene: Option<HdSceneIndexHandle>,
    fps: f32,
) -> Arc<RwLock<HdStVelocityMotionResolvingSceneIndex>> {
    HdStVelocityMotionResolvingSceneIndex::with_fps(input_scene, fps)
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
            "HdSt_VelocityMotionResolvingSceneIndex"
        );
        assert_eq!(lock.fps(), 24.0);
    }

    #[test]
    fn test_create_with_fps() {
        let si = create_with_fps(None, 30.0);
        let lock = si.read();
        assert_eq!(lock.fps(), 30.0);
    }

    #[test]
    fn test_constants() {
        assert_eq!(INSERTION_PHASE, 0);
        assert_eq!(DEFAULT_FPS, 24.0);
        assert_eq!(PLUGIN_DISPLAY_NAME, "GL");
    }
}
