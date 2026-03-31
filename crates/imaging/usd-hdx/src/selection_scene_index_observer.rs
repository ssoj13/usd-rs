
//! Selection scene index observer - observes scene index changes for selection.
//!
//! Watches scene index notifications (prims added/removed/dirtied) to keep
//! selection state synchronized. When selected prims are removed, their
//! selection entries are cleaned up.
//! Port of pxr/imaging/hdx/selectionSceneIndexObserver.h/cpp

use std::collections::HashSet;
use parking_lot::RwLock;
use usd_hd::scene_index::base::HdSceneIndexBase;
use usd_hd::scene_index::observer::{
    AddedPrimEntry, DirtiedPrimEntry, HdSceneIndexObserver, RemovedPrimEntry, RenamedPrimEntry,
};
use usd_sdf::Path;

use super::selection_tracker::{HdxSelectionTracker, SelectionTrackerExt};

/// Observer that watches scene index for selection-relevant changes.
///
/// Cleans up selection entries when prims are removed from the scene.
/// Optionally tracks dirtied prims for selection highlight updates.
///
/// Port of HdxSelectionSceneIndexObserver from pxr/imaging/hdx/selectionSceneIndexObserver.h
pub struct HdxSelectionSceneIndexObserver {
    /// Selection tracker to update.
    selection_tracker: HdxSelectionTracker,
    /// Set of paths that need selection highlight refresh.
    dirty_selection_paths: RwLock<HashSet<Path>>,
    /// Whether to track dirty paths for highlight refresh.
    track_dirty: bool,
}

impl HdxSelectionSceneIndexObserver {
    /// Create a new selection scene index observer.
    pub fn new(tracker: HdxSelectionTracker) -> Self {
        Self {
            selection_tracker: tracker,
            dirty_selection_paths: RwLock::new(HashSet::new()),
            track_dirty: true,
        }
    }

    /// Enable/disable tracking of dirty selection paths.
    pub fn set_track_dirty(&mut self, track: bool) {
        self.track_dirty = track;
    }

    /// Get and clear dirty selection paths since last call.
    pub fn take_dirty_paths(&self) -> HashSet<Path> {
        let mut paths = self.dirty_selection_paths.write();
        std::mem::take(&mut *paths)
    }

    /// Check if there are dirty selection paths pending.
    pub fn has_dirty_paths(&self) -> bool {
        let paths = self.dirty_selection_paths.read();
        !paths.is_empty()
    }

    /// Get a reference to the selection tracker.
    pub fn get_selection_tracker(&self) -> &HdxSelectionTracker {
        &self.selection_tracker
    }
}

impl HdSceneIndexObserver for HdxSelectionSceneIndexObserver {
    fn prims_added(&self, _sender: &dyn HdSceneIndexBase, _entries: &[AddedPrimEntry]) {
        // No action needed for added prims - they aren't selected yet
    }

    fn prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        // Remove selection entries for deleted prims
        for entry in entries {
            self.selection_tracker.deselect(&entry.prim_path);
        }
    }

    fn prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        if !self.track_dirty {
            return;
        }
        // Track dirtied prims that are currently selected for highlight refresh
        let mut dirty = self.dirty_selection_paths.write();
        for entry in entries {
            if self.selection_tracker.contains(&entry.prim_path) {
                dirty.insert(entry.prim_path.clone());
            }
        }
    }

    fn prims_renamed(&self, _sender: &dyn HdSceneIndexBase, _entries: &[RenamedPrimEntry]) {
        // In full implementation: update selection paths for renamed prims
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::selection_tracker::create_selection_tracker;

    #[test]
    fn test_observer_creation() {
        let tracker = create_selection_tracker();
        let observer = HdxSelectionSceneIndexObserver::new(tracker);
        assert!(!observer.has_dirty_paths());
    }

    #[test]
    fn test_prims_removed_cleans_selection() {
        let tracker = create_selection_tracker();
        let path = Path::from_string("/World/Mesh").unwrap();
        tracker.select(path.clone());
        assert!(tracker.contains(&path));

        let observer = HdxSelectionSceneIndexObserver::new(tracker.clone());

        // Create a no-op sender
        struct NoOp;
        impl HdSceneIndexBase for NoOp {
            fn get_prim(&self, _: &Path) -> usd_hd::scene_index::HdSceneIndexPrim {
                usd_hd::scene_index::HdSceneIndexPrim::empty()
            }
            fn get_child_prim_paths(&self, _: &Path) -> Vec<Path> {
                Vec::new()
            }
            fn add_observer(
                &self,
                _: usd_hd::scene_index::observer::HdSceneIndexObserverHandle,
            ) {
            }
            fn remove_observer(
                &self,
                _: &usd_hd::scene_index::observer::HdSceneIndexObserverHandle,
            ) {
            }
            fn get_display_name(&self) -> String {
                "NoOp".into()
            }
        }

        let sender = NoOp;
        observer.prims_removed(
            &sender,
            &[RemovedPrimEntry {
                prim_path: path.clone(),
            }],
        );

        assert!(!tracker.contains(&path));
    }
}
