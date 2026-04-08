//! Switching scene index.
//!
//! Port of pxr/imaging/hdsi/switchingSceneIndex.
//!
//! Switches between multiple input scene indices. When the index changes,
//! computes diff and sends appropriate notices to observers.

use crate::compute_scene_index_diff::ComputeSceneIndexDiffFn;
use parking_lot::RwLock;
use std::sync::{Arc, Weak};
use usd_hd::data_source::HdDataSourceBaseHandle;
use usd_hd::scene_index::base::{HdSceneIndexBaseImpl, SceneIndexDelegate};
use usd_hd::scene_index::filtering::HdFilteringSceneIndexBase;
use usd_hd::scene_index::observer::*;
use usd_hd::scene_index::*;
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;

/// Observer that forwards notices from the current input to the switching scene index.
struct SwitchingSceneIndexObserver {
    owner: Weak<RwLock<HdsiSwitchingSceneIndex>>,
}

impl SwitchingSceneIndexObserver {
    fn new_handle(owner: Weak<RwLock<HdsiSwitchingSceneIndex>>) -> HdSceneIndexObserverHandle {
        Arc::new(Self { owner })
    }
}

impl HdSceneIndexObserver for SwitchingSceneIndexObserver {
    fn prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        if let Some(owner_arc) = self.owner.upgrade() {
            let owner = usd_hd::scene_index::base::rwlock_data_ref(owner_arc.as_ref());
            let owner_sender: &dyn HdSceneIndexBase = owner;
            owner.base.send_prims_added(owner_sender, entries);
        }
    }

    fn prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        if let Some(owner_arc) = self.owner.upgrade() {
            let owner = usd_hd::scene_index::base::rwlock_data_ref(owner_arc.as_ref());
            let owner_sender: &dyn HdSceneIndexBase = owner;
            owner.base.send_prims_removed(owner_sender, entries);
        }
    }

    fn prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        if let Some(owner_arc) = self.owner.upgrade() {
            let owner = usd_hd::scene_index::base::rwlock_data_ref(owner_arc.as_ref());
            let owner_sender: &dyn HdSceneIndexBase = owner;
            owner.base.send_prims_dirtied(owner_sender, entries);
        }
    }

    fn prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        if let Some(owner_arc) = self.owner.upgrade() {
            let owner = usd_hd::scene_index::base::rwlock_data_ref(owner_arc.as_ref());
            let owner_sender: &dyn HdSceneIndexBase = owner;
            owner.base.send_prims_renamed(owner_sender, entries);
        }
    }
}

/// Switching scene index.
///
/// Can switch between multiple inputs (fixed at construction).
/// Uses [`ComputeSceneIndexDiffFn`] to compute notices when switching.
pub struct HdsiSwitchingSceneIndex {
    /// Base for observer management.
    base: HdSceneIndexBaseImpl,
    /// Our observer (forwards from current input). Stored so we can add/remove from inputs.
    observer_handle: HdSceneIndexObserverHandle,
    /// Input scene indices.
    inputs: Vec<HdSceneIndexHandle>,
    /// Current active index.
    index: usize,
    /// Currently active input (at inputs[index]).
    current_scene_index: Option<HdSceneIndexHandle>,
    /// Function to compute diff when switching.
    compute_diff_fn: ComputeSceneIndexDiffFn,
    /// Weak self for observer.
    self_weak: Weak<RwLock<Self>>,
}

impl HdsiSwitchingSceneIndex {
    /// Creates a new switching scene index.
    ///
    /// # Arguments
    /// * `inputs` - Input scene indices to switch between
    /// * `initial_index` - Index of the initially active input (default 0)
    /// * `compute_diff_fn` - Function to compute diff when switching. Use
    ///   [`compute_scene_index_diff_delta_fn`] for default.
    pub fn new(
        inputs: Vec<HdSceneIndexHandle>,
        initial_index: usize,
        compute_diff_fn: ComputeSceneIndexDiffFn,
    ) -> Arc<RwLock<Self>> {
        let scene = Arc::new(RwLock::new(Self {
            base: HdSceneIndexBaseImpl::new(),
            observer_handle: SwitchingSceneIndexObserver::new_handle(Weak::new()),
            inputs: inputs.clone(),
            index: 0,
            current_scene_index: None,
            compute_diff_fn,
            self_weak: Weak::new(),
        }));

        {
            let mut guard = scene.write();
            guard.self_weak = Arc::downgrade(&scene);
            guard.observer_handle = SwitchingSceneIndexObserver::new_handle(Arc::downgrade(&scene));
            guard.update_current_scene_index(&scene, initial_index);
        }

        scene
    }

    /// Creates a new switching scene index with default delta diff function.
    pub fn new_with_default_diff(
        inputs: Vec<HdSceneIndexHandle>,
        initial_index: usize,
    ) -> Arc<RwLock<Self>> {
        Self::new(
            inputs,
            initial_index,
            crate::compute_scene_index_diff_delta_fn(),
        )
    }

    /// Returns the current index.
    pub fn get_index(&self) -> usize {
        self.index
    }

    /// Sets the active input index.
    ///
    /// Index must be in `[0, get_input_scenes().len())`.
    pub fn set_index(this: &Arc<RwLock<Self>>, index: usize) {
        let mut guard = this.write();
        guard.update_current_scene_index(this, index);
    }

    fn update_current_scene_index(&mut self, this: &Arc<RwLock<Self>>, index: usize) {
        let prev_input = self.current_scene_index.take();
        self.index = index;

        self.current_scene_index = if index < self.inputs.len() {
            Some(self.inputs[index].clone())
        } else {
            None
        };

        if let Some(ref prev) = prev_input {
            prev.read().remove_observer(&self.observer_handle);
        }

        if self.base.is_observed() {
            let mut removed = Vec::new();
            let mut added = Vec::new();
            let mut renamed = Vec::new();
            let mut dirtied = Vec::new();
            (self.compute_diff_fn)(
                prev_input,
                self.current_scene_index.clone(),
                &mut removed,
                &mut added,
                &mut renamed,
                &mut dirtied,
            );

            let delegate = SceneIndexDelegate(Arc::clone(this));
            let sender = &delegate as &dyn HdSceneIndexBase;

            if !removed.is_empty() {
                self.base.send_prims_removed(sender, &removed);
            }
            if !added.is_empty() {
                self.base.send_prims_added(sender, &added);
            }
            if !renamed.is_empty() {
                self.base.send_prims_renamed(sender, &renamed);
            }
            if !dirtied.is_empty() {
                self.base.send_prims_dirtied(sender, &dirtied);
            }
        }

        if let Some(ref curr) = self.current_scene_index {
            curr.read().add_observer(Arc::clone(&self.observer_handle));
        }
    }
}

impl HdSceneIndexBase for HdsiSwitchingSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        if let Some(ref curr) = self.current_scene_index {
            return si_ref(&curr).get_prim(prim_path);
        }
        HdSceneIndexPrim::default()
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        if let Some(ref curr) = self.current_scene_index {
            return si_ref(&curr).get_child_prim_paths(prim_path);
        }
        Vec::new()
    }

    fn add_observer(&self, observer: HdSceneIndexObserverHandle) {
        self.base.add_observer(observer);
    }

    fn remove_observer(&self, observer: &HdSceneIndexObserverHandle) {
        self.base.remove_observer(observer);
    }

    fn _system_message(&self, _message_type: &TfToken, _args: Option<HdDataSourceBaseHandle>) {}

    fn get_display_name(&self) -> String {
        "HdsiSwitchingSceneIndex".to_string()
    }
}

impl HdFilteringSceneIndexBase for HdsiSwitchingSceneIndex {
    fn get_input_scenes(&self) -> Vec<HdSceneIndexHandle> {
        self.inputs.clone()
    }
}
