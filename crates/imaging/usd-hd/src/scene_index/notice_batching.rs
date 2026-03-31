
//! HdNoticeBatchingSceneIndex - Batches change notices for downstream scene indices.
//!
//! Port of pxr/imaging/hd/noticeBatchingSceneIndex.h/cpp
//!
//! By default, notices are forwarded immediately. When batching is enabled,
//! notices are queued and can be flushed explicitly.

use super::filtering::FilteringObserverTarget;
use super::observer::{AddedPrimEntry, DirtiedPrimEntry, RemovedPrimEntry, RenamedPrimEntry};
use super::{
    HdSceneIndexBase, HdSceneIndexHandle, HdSingleInputFilteringSceneIndexBase, SdfPathVector,
};
use crate::data_source::HdDataSourceBaseHandle;
use parking_lot::RwLock;
use std::sync::Arc;
use usd_sdf::Path;
use usd_tf::Token;

/// Scene index that batches change notices when enabled.
///
/// When batching is disabled (default), notices are forwarded immediately.
/// When enabled, notices are accumulated and can be flushed via `flush()`.
pub struct HdNoticeBatchingSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
    state: std::sync::Mutex<BatchingState>,
}

#[derive(Clone)]
enum NoticeBatch {
    PrimsAdded(Vec<AddedPrimEntry>),
    PrimsRemoved(Vec<RemovedPrimEntry>),
    PrimsDirtied(Vec<DirtiedPrimEntry>),
}

struct BatchingState {
    batching_enabled: bool,
    batches: Vec<NoticeBatch>,
}

impl HdNoticeBatchingSceneIndex {
    /// Creates a new notice batching scene index.
    ///
    /// Port of HdNoticeBatchingSceneIndex::New.
    pub fn new(input_scene: HdSceneIndexHandle) -> Arc<RwLock<Self>> {
        let input_clone = input_scene.clone();
        let result = Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(input_scene)),
            state: std::sync::Mutex::new(BatchingState {
                batching_enabled: false,
                batches: Vec::new(),
            }),
        }));
        super::filtering::wire_filter_to_input(&result, &input_clone);
        result
    }

    /// Returns whether batching is enabled.
    pub fn is_batching_enabled(&self) -> bool {
        let state = self.state.lock().expect("Lock poisoned");
        state.batching_enabled
    }

    /// Enables or disables batching without taking the outer `SceneRwLock`.
    ///
    /// This mirrors the rest of the scene-index observer stack, where callback
    /// paths use `RwLock::data_ptr()` to avoid Rust-only lock recursion hazards
    /// that do not exist in the C++ shared_ptr model.
    pub fn set_batching_enabled_unlocked(handle: &Arc<RwLock<Self>>, enabled: bool) {
        let inner = super::base::rwlock_data_ref(handle.as_ref());
        inner.set_batching_enabled(enabled);
    }

    /// Enables or disables batching.
    ///
    /// When disabling, any queued notices are flushed immediately.
    pub fn set_batching_enabled(&self, enabled: bool) {
        let should_flush = {
            let mut state = self.state.lock().expect("Lock poisoned");
            if state.batching_enabled == enabled {
                return;
            }
            state.batching_enabled = enabled;
            !enabled && !state.batches.is_empty()
        };
        if should_flush {
            self.flush();
        }
    }

    /// Flushes queued notices without taking the outer `SceneRwLock`.
    ///
    /// Holding the typed `Arc<RwLock<Self>>` write guard across observer
    /// callbacks can deadlock when downstream callbacks re-enter the same scene
    /// index chain. Using `data_ptr()` here matches the lock-free callback
    /// pattern already used elsewhere in the Hydra scene-index port.
    pub fn flush_unlocked(handle: &Arc<RwLock<Self>>) {
        let inner = super::base::rwlock_data_ref(handle.as_ref());
        inner.flush();
    }

    /// Forwards any queued notices to observers.
    pub fn flush(&self) {
        let batches = {
            let mut state = self.state.lock().expect("Lock poisoned");
            std::mem::take(&mut state.batches)
        };
        // Preserve the real scene-index sender contract during flush. Downstream
        // filters may call `sender.get_prim(...)` while processing dirties.
        // Using a placeholder sender breaks dependency rebuilds and diverges
        // from the C++ notice-batching behavior, which forwards `this`.
        let sender: &dyn HdSceneIndexBase = self;
        for batch in batches {
            match batch {
                NoticeBatch::PrimsAdded(entries) => {
                    self.base.base().send_prims_added(sender, &entries);
                }
                NoticeBatch::PrimsRemoved(entries) => {
                    self.base.base().send_prims_removed(sender, &entries);
                }
                NoticeBatch::PrimsDirtied(entries) => {
                    self.base.base().send_prims_dirtied(sender, &entries);
                }
            }
        }
    }
}

impl HdSceneIndexBase for HdNoticeBatchingSceneIndex {
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

    /// G2: SystemMessage recursion through input scene.
    fn get_input_scenes_for_system_message(&self) -> Vec<super::HdSceneIndexHandle> {
        self.base.get_input_scene().cloned().into_iter().collect()
    }

    fn get_display_name(&self) -> String {
        "HdNoticeBatchingSceneIndex".to_string()
    }
}

impl FilteringObserverTarget for HdNoticeBatchingSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        let mut state = self.state.lock().expect("Lock poisoned");
        if state.batching_enabled {
            if let Some(NoticeBatch::PrimsAdded(batch)) = state.batches.last_mut() {
                batch.extend(entries.iter().cloned());
                return;
            }
            state
                .batches
                .push(NoticeBatch::PrimsAdded(entries.to_vec()));
        } else {
            drop(state);
            self.base.forward_prims_added(self, entries);
        }
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        let mut state = self.state.lock().expect("Lock poisoned");
        if state.batching_enabled {
            if let Some(NoticeBatch::PrimsRemoved(batch)) = state.batches.last_mut() {
                batch.extend(entries.iter().cloned());
                return;
            }
            state
                .batches
                .push(NoticeBatch::PrimsRemoved(entries.to_vec()));
        } else {
            drop(state);
            self.base.forward_prims_removed(self, entries);
        }
    }

    fn on_prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        let mut state = self.state.lock().expect("Lock poisoned");
        if state.batching_enabled {
            if let Some(NoticeBatch::PrimsDirtied(batch)) = state.batches.last_mut() {
                batch.extend(entries.iter().cloned());
                return;
            }
            state
                .batches
                .push(NoticeBatch::PrimsDirtied(entries.to_vec()));
        } else {
            drop(state);
            self.base.forward_prims_dirtied(self, entries);
        }
    }

    /// G21: PrimsRenamed converts to removed+added, then batches those.
    ///
    /// Port of C++ _PrimsRenamed behavior: never batches renames directly,
    /// instead decomposes into removed+added and batches the components.
    fn on_prims_renamed(&self, sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        let (removed, added) =
            super::observer::convert_prims_renamed_to_removed_and_added(sender, entries);

        if !removed.is_empty() {
            self.on_prims_removed(sender, &removed);
        }
        if !added.is_empty() {
            self.on_prims_added(sender, &added);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene_index::base::scene_index_to_handle;
    use crate::scene_index::observer::HdSceneIndexObserver;
    use crate::scene_index::retained::{HdRetainedSceneIndex, RetainedAddedPrimEntry};
    use crate::{HdContainerDataSourceHandle, HdRetainedContainerDataSource};
    use std::sync::atomic::{AtomicBool, Ordering};
    use usd_tf::Token;

    struct ReentrantObserver {
        batching: Arc<RwLock<HdNoticeBatchingSceneIndex>>,
        callback_ran: Arc<AtomicBool>,
        reentered: Arc<AtomicBool>,
    }

    impl HdSceneIndexObserver for ReentrantObserver {
        fn prims_added(&self, _sender: &dyn HdSceneIndexBase, _entries: &[AddedPrimEntry]) {}

        fn prims_removed(&self, _sender: &dyn HdSceneIndexBase, _entries: &[RemovedPrimEntry]) {}

        fn prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, _entries: &[DirtiedPrimEntry]) {
            self.callback_ran.store(true, Ordering::SeqCst);
            self.reentered
                .store(self.batching.try_read().is_some(), Ordering::SeqCst);
        }

        fn prims_renamed(&self, _sender: &dyn HdSceneIndexBase, _entries: &[RenamedPrimEntry]) {}
    }

    #[test]
    fn test_flush_unlocked_allows_reentrant_read_from_observer_callback() {
        let retained = HdRetainedSceneIndex::new();
        let retained_handle = scene_index_to_handle(retained.clone());
        let batching = HdNoticeBatchingSceneIndex::new(retained_handle);
        let callback_ran = Arc::new(AtomicBool::new(false));
        let reentered = Arc::new(AtomicBool::new(false));
        let observer = Arc::new(ReentrantObserver {
            batching: batching.clone(),
            callback_ran: callback_ran.clone(),
            reentered: reentered.clone(),
        }) as super::super::observer::HdSceneIndexObserverHandle;

        batching.read().add_observer(observer);
        HdNoticeBatchingSceneIndex::set_batching_enabled_unlocked(&batching, true);

        let prim_path = Path::from_string("/Test").unwrap();
        let prim_data: HdContainerDataSourceHandle = HdRetainedContainerDataSource::new_empty();
        retained.write().add_prims(&[RetainedAddedPrimEntry::new(
            prim_path.clone(),
            Token::new("Mesh"),
            Some(prim_data),
        )]);

        retained.write().dirty_prims(&[DirtiedPrimEntry::new(
            prim_path,
            crate::data_source::HdDataSourceLocatorSet::new(),
        )]);

        HdNoticeBatchingSceneIndex::flush_unlocked(&batching);

        assert!(callback_ran.load(Ordering::SeqCst));
        assert!(reentered.load(Ordering::SeqCst));
    }

    #[test]
    fn test_set_batching_enabled_unlocked_flushes_without_outer_write_guard() {
        let retained = HdRetainedSceneIndex::new();
        let retained_handle = scene_index_to_handle(retained.clone());
        let batching = HdNoticeBatchingSceneIndex::new(retained_handle);
        let callback_ran = Arc::new(AtomicBool::new(false));
        let reentered = Arc::new(AtomicBool::new(false));
        let observer = Arc::new(ReentrantObserver {
            batching: batching.clone(),
            callback_ran: callback_ran.clone(),
            reentered: reentered.clone(),
        }) as super::super::observer::HdSceneIndexObserverHandle;

        batching.read().add_observer(observer);
        HdNoticeBatchingSceneIndex::set_batching_enabled_unlocked(&batching, true);

        let prim_path = Path::from_string("/Test").unwrap();
        let prim_data: HdContainerDataSourceHandle = HdRetainedContainerDataSource::new_empty();
        retained.write().add_prims(&[RetainedAddedPrimEntry::new(
            prim_path.clone(),
            Token::new("Mesh"),
            Some(prim_data),
        )]);

        retained.write().dirty_prims(&[DirtiedPrimEntry::new(
            prim_path,
            crate::data_source::HdDataSourceLocatorSet::new(),
        )]);

        HdNoticeBatchingSceneIndex::set_batching_enabled_unlocked(&batching, false);

        assert!(callback_ran.load(Ordering::SeqCst));
        assert!(reentered.load(Ordering::SeqCst));
    }
}
