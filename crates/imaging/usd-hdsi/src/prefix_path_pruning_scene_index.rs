//! Prefix path pruning scene index.
//!
//! Port of pxr/imaging/hdsi/prefixPathPruningSceneIndex.
//!
//! Prunes prims at or below the list of provided path prefixes.
//! The list may be set at construction via input args (excludePathPrefixes)
//! or updated via `set_exclude_path_prefixes`.

use crate::tokens::PREFIX_PATH_PRUNING_SCENE_INDEX_TOKENS;
use crate::utils;
use parking_lot::RwLock;
use std::sync::Arc;
use usd_hd::data_source::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdRetainedTypedSampledDataSource,
    HdTypedSampledDataSource,
};
use usd_hd::scene_index::filtering::FilteringSceneIndexObserver;
use usd_hd::scene_index::observer::*;
use usd_hd::scene_index::*;
use usd_sdf::Path as SdfPath;
use usd_sdf::path;
use usd_tf::Token as TfToken;

/// Scene index that prunes prims at or below the list of exclude path prefixes.
///
/// "Pruning" here **modifies topology**: pruned subtrees are removed from
/// the scene graph. Notices are filtered to exclude entries for pruned paths.
///
/// Differs from PrimTypePruningSceneIndex and PrimTypeAndPathPruningSceneIndex:
/// those return empty prim type for "pruned" prims but keep topology.
pub struct HdsiPrefixPathPruningSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
    /// Sorted exclude paths (descendent paths removed).
    sorted_exclude_paths: Vec<SdfPath>,
}

/// Check if prim_path is pruned by any exclude path.
///
/// A path is pruned if it has one of the sorted exclude paths as a prefix
/// (i.e. the prim is at or under an excluded path).
fn is_pruned_impl(prim_path: &SdfPath, sorted_exclude_paths: &[SdfPath]) -> bool {
    if sorted_exclude_paths.is_empty() {
        return false;
    }
    // Find first exclude path that is <= prim_path (using reverse order).
    // If such a path exists and prim_path has it as prefix, we're pruned.
    for exclude in sorted_exclude_paths.iter().rev() {
        if *exclude <= *prim_path && prim_path.has_prefix(exclude) {
            return true;
        }
        if *exclude > *prim_path {
            break;
        }
    }
    false
}

/// Sanitize exclude paths: remove duplicates and descendants.
fn get_sanitized_exclude_paths(input_args: Option<&HdContainerDataSourceHandle>) -> Vec<SdfPath> {
    let mut path_vector = get_exclude_path_prefixes_from_args(input_args);
    path::remove_descendent_paths(&mut path_vector);
    path_vector
}

/// Read excludePathPrefixes from input args container.
///
/// Expects a HdTypedSampledDataSource<Vec<SdfPath>> at "excludePathPrefixes".
/// Returns empty vec if not present or wrong type.
fn get_exclude_path_prefixes_from_args(
    input_args: Option<&HdContainerDataSourceHandle>,
) -> Vec<SdfPath> {
    let container = match input_args {
        Some(c) => c,
        None => return Vec::new(),
    };
    let ds = match container.get(&PREFIX_PATH_PRUNING_SCENE_INDEX_TOKENS.exclude_path_prefixes) {
        Some(d) => d,
        None => return Vec::new(),
    };
    // Try to extract Vec<SdfPath> from value if it's a retained typed data source
    if let Some(retained) = (ds.as_ref() as &dyn usd_hd::data_source::HdDataSourceBase)
        .as_any()
        .downcast_ref::<HdRetainedTypedSampledDataSource<Vec<SdfPath>>>()
    {
        return retained.get_typed_value(0.0);
    }
    Vec::new()
}

/// Compute prefixes in `a` that are not covered by any prefix in `b`.
fn compute_uncovered_prefixes(a: &[SdfPath], b: &[SdfPath]) -> Vec<SdfPath> {
    a.iter()
        .filter(|path| !b.iter().any(|prefix| path.has_prefix(prefix)))
        .cloned()
        .collect()
}

impl HdsiPrefixPathPruningSceneIndex {
    /// Creates a new prefix path pruning scene index.
    ///
    /// # Arguments
    /// * `input_scene` - The scene index to filter
    /// * `input_args` - Optional container with "excludePathPrefixes" (SdfPathVector)
    pub fn new(
        input_scene: HdSceneIndexHandle,
        input_args: Option<HdContainerDataSourceHandle>,
    ) -> Arc<RwLock<Self>> {
        let sorted_exclude_paths = get_sanitized_exclude_paths(input_args.as_ref());
        let observer = Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(input_scene.clone())),
            sorted_exclude_paths,
        }));
        let filtering_observer = FilteringSceneIndexObserver::new(
            Arc::downgrade(&observer) as std::sync::Weak<RwLock<dyn FilteringObserverTarget>>
        );
        {
            input_scene
                .read()
                .add_observer(Arc::new(filtering_observer));
        }
        observer
    }

    /// Updates the exclude path prefixes.
    ///
    /// When observed, sends PrimsAdded for paths no longer pruned and
    /// PrimsRemoved for newly pruned prefixes.
    pub fn set_exclude_path_prefixes(this: &Arc<RwLock<Self>>, paths: Vec<SdfPath>) {
        let mut new_prefixes = paths;
        path::remove_descendent_paths(&mut new_prefixes);

        let (old_prefixes, needs_notice, input_handle) = {
            let mut guard = this.write();
            if new_prefixes == guard.sorted_exclude_paths {
                return;
            }
            let old = std::mem::replace(&mut guard.sorted_exclude_paths, new_prefixes.clone());
            let observed = guard.base.base().is_observed();
            let input = guard.base.get_input_scene().cloned();
            (old, observed, input)
        };

        if !needs_notice {
            return;
        }

        let input_si = match input_handle {
            Some(h) => h,
            None => return,
        };

        let no_longer_pruned = compute_uncovered_prefixes(&old_prefixes, &new_prefixes);
        let newly_pruned: Vec<SdfPath> = new_prefixes
            .iter()
            .filter(|p| !old_prefixes.contains(p))
            .cloned()
            .collect();

        let mut added_entries = Vec::new();
        for prefix in &no_longer_pruned {
            for prim_path in utils::collect_prim_paths(&input_si, prefix) {
                {
                    let prim = si_ref(&input_si).get_prim(&prim_path);
                    if prim.is_defined() {
                        added_entries.push(AddedPrimEntry {
                            prim_path,
                            prim_type: prim.prim_type,
                            data_source: prim.data_source,
                        });
                    }
                }
            }
        }

        let removed_entries: Vec<RemovedPrimEntry> = newly_pruned
            .into_iter()
            .map(RemovedPrimEntry::new)
            .collect();

        {
            let mut guard = this.write();
            let delegate = usd_hd::scene_index::base::SceneIndexDelegate(Arc::clone(this));
            let sender = &delegate as &dyn HdSceneIndexBase;
            if !added_entries.is_empty() {
                guard
                    .base
                    .base_mut()
                    .send_prims_added(sender, &added_entries);
            }
            if !removed_entries.is_empty() {
                guard
                    .base
                    .base_mut()
                    .send_prims_removed(sender, &removed_entries);
            }
        }
    }

    /// Returns current exclude path prefixes.
    pub fn get_exclude_path_prefixes(&self) -> &[SdfPath] {
        &self.sorted_exclude_paths
    }

    fn is_pruned(&self, prim_path: &SdfPath) -> bool {
        is_pruned_impl(prim_path, &self.sorted_exclude_paths)
    }

    fn remove_pruned_children(&self, child_paths: &mut Vec<SdfPath>) {
        child_paths.retain(|p| !self.is_pruned(p));
    }
}

impl HdSceneIndexBase for HdsiPrefixPathPruningSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        if self.sorted_exclude_paths.is_empty() || !self.is_pruned(prim_path) {
            if let Some(input) = self.base.get_input_scene() {
                return si_ref(&input).get_prim(prim_path);
            }
        }
        HdSceneIndexPrim::default()
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        let have_exclude_paths = !self.sorted_exclude_paths.is_empty();
        if have_exclude_paths && self.is_pruned(prim_path) {
            return Vec::new();
        }

        let mut child_paths = if let Some(input) = self.base.get_input_scene() {
            si_ref(&input).get_child_prim_paths(prim_path)
        } else {
            Vec::new()
        };

        if have_exclude_paths {
            self.remove_pruned_children(&mut child_paths);
        }
        child_paths
    }

    fn add_observer(&self, observer: HdSceneIndexObserverHandle) {
        self.base.base().add_observer(observer);
    }

    fn remove_observer(&self, observer: &HdSceneIndexObserverHandle) {
        self.base.base().remove_observer(observer);
    }

    fn _system_message(&self, _message_type: &TfToken, _args: Option<HdDataSourceBaseHandle>) {}

    fn get_display_name(&self) -> String {
        "HdsiPrefixPathPruningSceneIndex".to_string()
    }
}

impl FilteringObserverTarget for HdsiPrefixPathPruningSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        if !self.base.base().is_observed() {
            return;
        }
        if self.sorted_exclude_paths.is_empty() {
            self.base.forward_prims_added(self, entries);
            return;
        }
        let filtered: Vec<AddedPrimEntry> = entries
            .iter()
            .filter(|e| !is_pruned_impl(&e.prim_path, &self.sorted_exclude_paths))
            .cloned()
            .collect();
        if !filtered.is_empty() {
            self.base.forward_prims_added(self, &filtered);
        }
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        if !self.base.base().is_observed() {
            return;
        }
        if self.sorted_exclude_paths.is_empty() {
            self.base.forward_prims_removed(self, entries);
            return;
        }
        let filtered: Vec<RemovedPrimEntry> = entries
            .iter()
            .filter(|e| !is_pruned_impl(&e.prim_path, &self.sorted_exclude_paths))
            .cloned()
            .collect();
        if !filtered.is_empty() {
            self.base.forward_prims_removed(self, &filtered);
        }
    }

    fn on_prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        if !self.base.base().is_observed() {
            return;
        }
        if self.sorted_exclude_paths.is_empty() {
            self.base.forward_prims_dirtied(self, entries);
            return;
        }
        let filtered: Vec<DirtiedPrimEntry> = entries
            .iter()
            .filter(|e| !is_pruned_impl(&e.prim_path, &self.sorted_exclude_paths))
            .cloned()
            .collect();
        if !filtered.is_empty() {
            self.base.forward_prims_dirtied(self, &filtered);
        }
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        // Renamed entries need filtering too if we filter others
        if !self.base.base().is_observed() {
            return;
        }
        self.base.forward_prims_renamed(self, entries);
    }
}
