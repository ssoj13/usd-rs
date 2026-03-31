
//! HdEncapsulatingSceneIndex - Pass-through wrapper for scene debugger introspection.
//!
//! Port of pxr/imaging/hd/sceneIndexUtil.h HdMakeEncapsulatingSceneIndex.
//!
//! When HD_USE_ENCAPSULATING_SCENE_INDICES is true, the scene index graph
//! uses these wrappers so the Hydra Scene Debugger can understand structure.

use super::filtering::FilteringObserverTarget;
use super::observer::{AddedPrimEntry, DirtiedPrimEntry, RemovedPrimEntry, RenamedPrimEntry};
use super::{
    HdSceneIndexBase, HdSceneIndexHandle, HdSingleInputFilteringSceneIndexBase, SdfPathVector,
    si_ref,
};
use crate::data_source::HdDataSourceBaseHandle;
use parking_lot::RwLock;
use std::sync::Arc;
use usd_sdf::Path;
use usd_tf::Token;
use usd_tf::getenv::tf_getenv_bool;

/// Environment variable to enable encapsulating scene indices.
///
/// Port of HD_USE_ENCAPSULATING_SCENE_INDICES (default: false).
pub fn hd_use_encapsulating_scene_indices() -> bool {
    tf_getenv_bool("HD_USE_ENCAPSULATING_SCENE_INDICES", false)
}

/// Pass-through scene index that forwards to an encapsulated scene.
///
/// Used when HD_USE_ENCAPSULATING_SCENE_INDICES is enabled for scene debugger.
pub struct HdEncapsulatingSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
}

impl HdEncapsulatingSceneIndex {
    /// Creates a new encapsulating scene index.
    ///
    /// Port of HdMakeEncapsulatingSceneIndex with empty inputScenes.
    /// C++ parity: constructor calls `_inputSceneIndex->AddObserver(this)`.
    pub fn new(encapsulated_scene: HdSceneIndexHandle) -> Arc<RwLock<Self>> {
        let input_clone = encapsulated_scene.clone();
        let result = Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(encapsulated_scene)),
        }));
        super::filtering::wire_filter_to_input(&result, &input_clone);
        result
    }
}

impl HdSceneIndexBase for HdEncapsulatingSceneIndex {
    fn get_prim(&self, prim_path: &Path) -> super::HdSceneIndexPrim {
        if let Some(input) = self.base.get_input_scene() {
            {
                let input_locked = input.read();
                return input_locked.get_prim(prim_path);
            }
        }
        super::HdSceneIndexPrim::default()
    }

    fn get_child_prim_paths(&self, prim_path: &Path) -> SdfPathVector {
        if let Some(input) = self.base.get_input_scene() {
            {
                let input_locked = input.read();
                return input_locked.get_child_prim_paths(prim_path);
            }
        }
        Vec::new()
    }

    fn add_observer(&self, observer: super::HdSceneIndexObserverHandle) {
        self.base.base().add_observer(observer);
    }

    fn remove_observer(&self, observer: &super::HdSceneIndexObserverHandle) {
        self.base.base().remove_observer(observer);
    }

    fn _system_message(&self, _message_type: &Token, _args: Option<HdDataSourceBaseHandle>) {}

    fn get_display_name(&self) -> String {
        "HdEncapsulatingSceneIndex".to_string()
    }
}

impl FilteringObserverTarget for HdEncapsulatingSceneIndex {
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

/// Wraps the scene index. Port of HdMakeEncapsulatingSceneIndex.
///
/// - If `input_scenes` is empty: returns HdEncapsulatingSceneIndex (single-input).
/// - Otherwise: returns HdFilteringEncapsulatingSceneIndex with GetInputScenes.
pub fn hd_make_encapsulating_scene_index(
    input_scenes: &[HdSceneIndexHandle],
    encapsulated_scene: HdSceneIndexHandle,
) -> HdSceneIndexHandle {
    if input_scenes.is_empty() {
        super::scene_index_to_handle(HdEncapsulatingSceneIndex::new(encapsulated_scene))
    } else {
        super::scene_index_to_handle(HdFilteringEncapsulatingSceneIndex::new(
            input_scenes.to_vec(),
            encapsulated_scene,
        ))
    }
}

/// Encapsulating scene index with multiple input scenes (for scene debugger).
pub struct HdFilteringEncapsulatingSceneIndex {
    input_scenes: Vec<HdSceneIndexHandle>,
    encapsulated_scene: HdSceneIndexHandle,
    base: super::base::HdSceneIndexBaseImpl,
}

impl HdFilteringEncapsulatingSceneIndex {
    fn new(
        input_scenes: Vec<HdSceneIndexHandle>,
        encapsulated_scene: HdSceneIndexHandle,
    ) -> Arc<RwLock<Self>> {
        Arc::new(RwLock::new(Self {
            input_scenes: input_scenes.clone(),
            encapsulated_scene: encapsulated_scene.clone(),
            base: super::base::HdSceneIndexBaseImpl::new(),
        }))
    }
}

impl HdSceneIndexBase for HdFilteringEncapsulatingSceneIndex {
    fn get_prim(&self, prim_path: &Path) -> super::HdSceneIndexPrim {
        si_ref(&self.encapsulated_scene).get_prim(prim_path)
    }

    fn get_child_prim_paths(&self, prim_path: &Path) -> SdfPathVector {
        si_ref(&self.encapsulated_scene).get_child_prim_paths(prim_path)
    }

    fn add_observer(&self, observer: super::observer::HdSceneIndexObserverHandle) {
        self.base.add_observer(observer);
    }

    fn remove_observer(&self, observer: &super::observer::HdSceneIndexObserverHandle) {
        self.base.remove_observer(observer);
    }

    fn _system_message(&self, _message_type: &Token, _args: Option<HdDataSourceBaseHandle>) {}

    fn get_display_name(&self) -> String {
        "HdFilteringEncapsulatingSceneIndex".to_string()
    }
}

impl super::filtering::HdFilteringSceneIndexBase for HdFilteringEncapsulatingSceneIndex {
    fn get_input_scenes(&self) -> Vec<HdSceneIndexHandle> {
        self.input_scenes.clone()
    }
}
