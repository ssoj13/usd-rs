//! Native instance prototype pruning scene index.
//!
//! Prunes (removes) prototype prims from the scene. After prototype data
//! has been propagated to instances, the prototype prims themselves are
//! no longer needed in the scene graph for rendering. This scene index
//! removes them to avoid redundant processing.
//!
//! # Why Prune Prototypes?
//!
//! - Prototypes are master copies, not meant to be rendered directly
//! - After propagation, instances have all the data they need
//! - Removing prototypes reduces scene graph size and traversal cost
//! - Render backends only see the actual instances
//!
//! # Pipeline Position
//!
//! Typically used after:
//! 1. Prototype scene index (identifies prototypes)
//! 2. Prototype propagating scene index (copies data to instances)
//! 3. This pruning index (removes prototype prims)
//!
//! # References
//!
//! OpenUSD: `pxr/usdImaging/usdImaging/niPrototypePruningSceneIndex.h`

use parking_lot::RwLock;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use usd_hd::HdDataSourceBaseHandle;
use usd_hd::scene_index::observer::{
    AddedPrimEntry, DirtiedPrimEntry, RemovedPrimEntry, RenamedPrimEntry,
};
use usd_hd::scene_index::{
    FilteringObserverTarget, HdContainerDataSourceHandle, HdSceneIndexBase, HdSceneIndexHandle,
    HdSceneIndexPrim, HdSingleInputFilteringSceneIndexBase, SdfPathVector, si_ref,
    wire_filter_to_input,
};
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;

/// Token names for prototype pruning.
#[allow(dead_code)]
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    /// Token for prototype prefix in paths.
    pub static PROTOTYPE_PREFIX: LazyLock<Token> = LazyLock::new(|| Token::new("__Prototype"));

    /// Token for pruning enabled configuration.
    pub static PRUNING_ENABLED: LazyLock<Token> = LazyLock::new(|| Token::new("pruningEnabled"));
}

/// Native instance prototype pruning scene index.
///
/// Removes prototype prims from the scene after their data has been
/// propagated to instances. This keeps the scene graph clean and
/// prevents redundant processing of prototypes.
///
/// # Fields
///
/// * `base` - Base filtering scene index functionality
/// * `pruned_paths` - Set of prototype paths that have been pruned
/// * `pruning_enabled` - Whether pruning is currently active
pub struct UsdImagingNiPrototypePruningSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
    state: Mutex<NiPrototypePruningState>,
}

struct NiPrototypePruningState {
    /// Set of paths that have been pruned (cached for performance)
    pruned_paths: HashSet<SdfPath>,
    /// Whether prototype pruning is enabled
    pruning_enabled: bool,
}

impl UsdImagingNiPrototypePruningSceneIndex {
    /// Creates a new prototype pruning scene index.
    ///
    /// This scene index filters out prototype prims from the scene,
    /// leaving only the actual instance prims for rendering.
    ///
    /// # Arguments
    ///
    /// * `input_scene` - The input scene index
    /// * `input_args` - Optional configuration data source
    ///
    /// # Returns
    ///
    /// Thread-safe shared reference to the new scene index
    pub fn new(
        input_scene: HdSceneIndexHandle,
        _input_args: Option<HdContainerDataSourceHandle>,
    ) -> Arc<RwLock<Self>> {
        let result = Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(input_scene.clone())),
            state: Mutex::new(NiPrototypePruningState {
                pruned_paths: HashSet::new(),
                pruning_enabled: true,
            }),
        }));
        wire_filter_to_input(&result, &input_scene);
        result
    }

    /// Enables or disables prototype pruning.
    ///
    /// When disabled, prototype prims will be visible in the scene.
    /// When enabled, they are filtered out.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether to enable pruning
    pub fn set_pruning_enabled(&mut self, enabled: bool) {
        let mut state = self.state.lock().expect("Lock poisoned");
        if state.pruning_enabled == enabled {
            return;
        }

        state.pruning_enabled = enabled;

        // Clear cache when toggling to force re-evaluation
        state.pruned_paths.clear();
    }

    /// Checks if pruning is currently enabled.
    ///
    /// # Returns
    ///
    /// `true` if prototype prims will be pruned
    pub fn is_pruning_enabled(&self) -> bool {
        self.state.lock().expect("Lock poisoned").pruning_enabled
    }

    /// Checks if a path should be pruned.
    ///
    /// Prototype paths typically have a special prefix like `__Prototype`.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to check
    ///
    /// # Returns
    ///
    /// `true` if this path should be pruned
    fn should_prune_path(&self, path: &SdfPath) -> bool {
        if !self.state.lock().expect("Lock poisoned").pruning_enabled {
            return false;
        }

        // Check if any path component starts with "__Prototype"
        path.get_text()
            .split('/')
            .any(|component| component.starts_with("__Prototype"))
    }

    /// Marks a path as pruned in the cache.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to mark as pruned
    fn mark_pruned(&self, path: SdfPath) {
        self.state
            .lock()
            .expect("Lock poisoned")
            .pruned_paths
            .insert(path);
    }

    /// Checks if a path was previously marked as pruned.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to check
    ///
    /// # Returns
    ///
    /// `true` if path is in the pruned set
    fn is_marked_pruned(&self, path: &SdfPath) -> bool {
        self.state
            .lock()
            .expect("Lock poisoned")
            .pruned_paths
            .contains(path)
    }

    /// Removes a path from the pruned set.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to unmark
    fn unmark_pruned(&self, path: &SdfPath) {
        self.state
            .lock()
            .expect("Lock poisoned")
            .pruned_paths
            .remove(path);
    }
}

impl HdSceneIndexBase for UsdImagingNiPrototypePruningSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        // If path should be pruned, return empty prim
        if self.should_prune_path(prim_path) {
            return HdSceneIndexPrim::default();
        }

        // Otherwise query from input
        if let Some(input) = self.base.get_input_scene() {
            return si_ref(&input).get_prim(prim_path);
        }

        HdSceneIndexPrim::default()
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        if let Some(input) = self.base.get_input_scene() {
            let children = si_ref(&input).get_child_prim_paths(prim_path);

            if !self.state.lock().expect("Lock poisoned").pruning_enabled {
                return children;
            }

            // Filter out prototype children
            return children
                .into_iter()
                .filter(|child| !self.should_prune_path(child))
                .collect();
        }
        Vec::new()
    }

    fn add_observer(&self, observer: usd_hd::scene_index::HdSceneIndexObserverHandle) {
        self.base.base().add_observer(observer);
    }

    fn remove_observer(&self, observer: &usd_hd::scene_index::HdSceneIndexObserverHandle) {
        self.base.base().remove_observer(observer);
    }

    fn _system_message(&self, _message_type: &TfToken, _args: Option<HdDataSourceBaseHandle>) {}

    fn get_display_name(&self) -> String {
        "UsdImagingNiPrototypePruningSceneIndex".to_string()
    }
}

impl FilteringObserverTarget for UsdImagingNiPrototypePruningSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        if !self.state.lock().expect("Lock poisoned").pruning_enabled {
            self.base.forward_prims_added(self, entries);
            return;
        }

        // Filter out prototype prims from added entries
        let mut filtered_entries = Vec::new();

        for entry in entries {
            if self.should_prune_path(&entry.prim_path) {
                self.mark_pruned(entry.prim_path.clone());
            } else {
                filtered_entries.push(entry.clone());
            }
        }

        if !filtered_entries.is_empty() {
            self.base.forward_prims_added(self, &filtered_entries);
        }
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        // Clean up pruned path tracking
        for entry in entries {
            self.state
                .lock()
                .expect("Lock poisoned")
                .pruned_paths
                .retain(|p| !p.has_prefix(&entry.prim_path));
        }

        if !self.state.lock().expect("Lock poisoned").pruning_enabled {
            self.base.forward_prims_removed(self, entries);
            return;
        }

        // Filter out prototype paths from removal notices
        let filtered_entries: Vec<_> = entries
            .iter()
            .filter(|entry| !self.should_prune_path(&entry.prim_path))
            .cloned()
            .collect();

        if !filtered_entries.is_empty() {
            self.base.forward_prims_removed(self, &filtered_entries);
        }
    }

    fn on_prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        if !self.state.lock().expect("Lock poisoned").pruning_enabled {
            self.base.forward_prims_dirtied(self, entries);
            return;
        }

        // Filter out prototype paths from dirty notices
        let filtered_entries: Vec<_> = entries
            .iter()
            .filter(|entry| !self.should_prune_path(&entry.prim_path))
            .cloned()
            .collect();

        if !filtered_entries.is_empty() {
            self.base.forward_prims_dirtied(self, &filtered_entries);
        }
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        if !self.state.lock().expect("Lock poisoned").pruning_enabled {
            self.base.forward_prims_renamed(self, entries);
            return;
        }

        // Update pruned path tracking for renamed prims
        for entry in entries {
            if self.is_marked_pruned(&entry.old_prim_path) {
                self.unmark_pruned(&entry.old_prim_path);
                self.mark_pruned(entry.new_prim_path.clone());
            }
        }

        // Filter out prototype paths from rename notices
        let filtered_entries: Vec<_> = entries
            .iter()
            .filter(|entry| {
                !self.should_prune_path(&entry.old_prim_path)
                    && !self.should_prune_path(&entry.new_prim_path)
            })
            .cloned()
            .collect();

        if !filtered_entries.is_empty() {
            self.base.forward_prims_renamed(self, &filtered_entries);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pruning_enabled() {
        let mut scene_index = UsdImagingNiPrototypePruningSceneIndex {
            base: HdSingleInputFilteringSceneIndexBase::new(None),
            state: Mutex::new(NiPrototypePruningState {
                pruned_paths: HashSet::new(),
                pruning_enabled: true,
            }),
        };

        assert!(scene_index.is_pruning_enabled());

        scene_index.set_pruning_enabled(false);
        assert!(!scene_index.is_pruning_enabled());
    }

    #[test]
    fn test_should_prune_prototype_paths() {
        let scene_index = UsdImagingNiPrototypePruningSceneIndex {
            base: HdSingleInputFilteringSceneIndexBase::new(None),
            state: Mutex::new(NiPrototypePruningState {
                pruned_paths: HashSet::new(),
                pruning_enabled: true,
            }),
        };

        let prototype_path = SdfPath::from_string("/__Prototype_1").unwrap();
        let regular_path = SdfPath::from_string("/World/Cube").unwrap();

        assert!(scene_index.should_prune_path(&prototype_path));
        assert!(!scene_index.should_prune_path(&regular_path));
    }

    #[test]
    fn test_pruning_disabled() {
        let scene_index = UsdImagingNiPrototypePruningSceneIndex {
            base: HdSingleInputFilteringSceneIndexBase::new(None),
            state: Mutex::new(NiPrototypePruningState {
                pruned_paths: HashSet::new(),
                pruning_enabled: false,
            }),
        };

        let prototype_path = SdfPath::from_string("/__Prototype_1").unwrap();

        assert!(!scene_index.should_prune_path(&prototype_path));
    }

    #[test]
    fn test_pruned_path_tracking() {
        let scene_index = UsdImagingNiPrototypePruningSceneIndex {
            base: HdSingleInputFilteringSceneIndexBase::new(None),
            state: Mutex::new(NiPrototypePruningState {
                pruned_paths: HashSet::new(),
                pruning_enabled: true,
            }),
        };

        let path = SdfPath::from_string("/__Prototype_1").unwrap();

        assert!(!scene_index.is_marked_pruned(&path));

        scene_index.mark_pruned(path.clone());
        assert!(scene_index.is_marked_pruned(&path));

        scene_index.unmark_pruned(&path);
        assert!(!scene_index.is_marked_pruned(&path));
    }

    #[test]
    fn test_cache_cleared_on_toggle() {
        let mut scene_index = UsdImagingNiPrototypePruningSceneIndex {
            base: HdSingleInputFilteringSceneIndexBase::new(None),
            state: Mutex::new(NiPrototypePruningState {
                pruned_paths: HashSet::new(),
                pruning_enabled: true,
            }),
        };

        let path = SdfPath::from_string("/__Prototype_1").unwrap();
        scene_index.mark_pruned(path.clone());
        assert_eq!(
            scene_index
                .state
                .lock()
                .expect("Lock poisoned")
                .pruned_paths
                .len(),
            1
        );

        scene_index.set_pruning_enabled(false);
        assert_eq!(
            scene_index
                .state
                .lock()
                .expect("Lock poisoned")
                .pruned_paths
                .len(),
            0
        );
    }
}
