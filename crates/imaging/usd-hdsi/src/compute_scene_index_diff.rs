//! Compute scene index diff utilities.
//!
//! Port of pxr/imaging/hdsi/computeSceneIndexDiff.
//!
//! Utilities for computing differences between scene indices expressed as
//! observer entries (removed, added, renamed, dirtied).

use std::collections::HashSet;
use std::sync::Arc;
use usd_hd::data_source::HdDataSourceLocatorSet;
use usd_hd::scene_index::observer::{
    AddedPrimEntry, DirtiedPrimEntry, RemovedPrimEntry, RenamedPrimEntry,
};
use usd_hd::scene_index::{HdSceneIndexBase, HdSceneIndexHandle};
use usd_sdf::Path as SdfPath;

/// Function type for computing scene index diff.
///
/// Matches C++ `HdsiComputeSceneIndexDiff`. Fills the four entry vectors
/// with removed, added, renamed, and dirtied entries when switching from
/// si_a (old) to si_b (new). All output pointers must be non-null.
pub type ComputeSceneIndexDiffFn = Box<
    dyn Fn(
            Option<HdSceneIndexHandle>,
            Option<HdSceneIndexHandle>,
            &mut Vec<RemovedPrimEntry>,
            &mut Vec<AddedPrimEntry>,
            &mut Vec<RenamedPrimEntry>,
            &mut Vec<DirtiedPrimEntry>,
        ) + Send
        + Sync,
>;

/// Coarse diff: removes root if si_a exists, adds all prims from si_b.
///
/// Port of `HdsiComputeSceneIndexDiffRoot`. If si_a is not null, removes `/`.
/// If si_b is not null, adds all prims recursively from `/`.
pub fn compute_scene_index_diff_root(
    si_a: Option<HdSceneIndexHandle>,
    si_b: Option<HdSceneIndexHandle>,
    removed_entries: &mut Vec<RemovedPrimEntry>,
    added_entries: &mut Vec<AddedPrimEntry>,
    _renamed_entries: &mut Vec<RenamedPrimEntry>,
    _dirtied_entries: &mut Vec<DirtiedPrimEntry>,
) {
    if si_a.is_some() {
        removed_entries.push(RemovedPrimEntry::new(SdfPath::absolute_root()));
    }

    if let Some(ref si) = si_b {
        fill_added_child_entries(&*si.read(), &SdfPath::absolute_root(), added_entries);
    }
}

/// Recursively add prim and all descendants to added_entries.
fn fill_added_child_entries(
    scene: &dyn HdSceneIndexBase,
    path: &SdfPath,
    added_entries: &mut Vec<AddedPrimEntry>,
) {
    let prim = scene.get_prim(path);
    added_entries.push(AddedPrimEntry {
        prim_path: path.clone(),
        prim_type: prim.prim_type,
        data_source: prim.data_source,
    });
    for child_path in scene.get_child_prim_paths(path) {
        fill_added_child_entries(scene, &child_path, added_entries);
    }
}

/// Sparse delta diff: walks both scenes and computes prim-level delta.
///
/// Port of `HdsiComputeSceneIndexDiffDelta`. If either si_a or si_b is null,
/// falls back to `compute_scene_index_diff_root`.
pub fn compute_scene_index_diff_delta(
    si_a: Option<HdSceneIndexHandle>,
    si_b: Option<HdSceneIndexHandle>,
    removed_entries: &mut Vec<RemovedPrimEntry>,
    added_entries: &mut Vec<AddedPrimEntry>,
    _renamed_entries: &mut Vec<RenamedPrimEntry>,
    dirtied_entries: &mut Vec<DirtiedPrimEntry>,
) {
    let (a_handle, b_handle) = match (&si_a, &si_b) {
        (Some(a), Some(b)) => (a, b),
        _ => {
            compute_scene_index_diff_root(
                si_a.clone(),
                si_b.clone(),
                removed_entries,
                added_entries,
                _renamed_entries,
                dirtied_entries,
            );
            return;
        }
    };

    let guard_a = a_handle.read();
    let guard_b = b_handle.read();

    compute_delta_diff_helper(
        &*guard_a,
        &*guard_b,
        &SdfPath::absolute_root(),
        removed_entries,
        added_entries,
        dirtied_entries,
    );
}

fn get_sorted_child_paths(scene: &dyn HdSceneIndexBase, path: &SdfPath) -> Vec<SdfPath> {
    let mut paths = scene.get_child_prim_paths(path);
    paths.sort();
    paths
}

/// Merge two sorted path slices: outputs (both, a_only, b_only).
fn set_intersection_and_difference(
    a: &[SdfPath],
    b: &[SdfPath],
    both: &mut Vec<SdfPath>,
    a_only: &mut Vec<SdfPath>,
    b_only: &mut Vec<SdfPath>,
) {
    let mut i = 0;
    let mut j = 0;
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Less => {
                a_only.push(a[i].clone());
                i += 1;
            }
            std::cmp::Ordering::Greater => {
                b_only.push(b[j].clone());
                j += 1;
            }
            std::cmp::Ordering::Equal => {
                both.push(a[i].clone());
                i += 1;
                j += 1;
            }
        }
    }
    a_only.extend(a[i..].iter().cloned());
    b_only.extend(b[j..].iter().cloned());
}

fn compute_delta_diff_helper(
    si_a: &dyn HdSceneIndexBase,
    si_b: &dyn HdSceneIndexBase,
    common_path: &SdfPath,
    removed_entries: &mut Vec<RemovedPrimEntry>,
    added_entries: &mut Vec<AddedPrimEntry>,
    dirtied_entries: &mut Vec<DirtiedPrimEntry>,
) {
    let prim_a = si_a.get_prim(common_path);
    let prim_b = si_b.get_prim(common_path);

    if prim_a.prim_type == prim_b.prim_type {
        let data_source_changed = match (&prim_a.data_source, &prim_b.data_source) {
            (None, None) => false,
            (Some(a), Some(b)) => !Arc::ptr_eq(a, b),
            _ => true,
        };
        if data_source_changed {
            dirtied_entries.push(DirtiedPrimEntry::new(
                common_path.clone(),
                HdDataSourceLocatorSet::universal(),
            ));
        }
    } else {
        added_entries.push(AddedPrimEntry {
            prim_path: common_path.clone(),
            prim_type: prim_b.prim_type,
            data_source: prim_b.data_source.clone(),
        });
    }

    let a_paths = get_sorted_child_paths(si_a, common_path);
    let b_paths = get_sorted_child_paths(si_b, common_path);

    let mut shared_children = Vec::new();
    let mut a_only_paths = Vec::new();
    let mut b_only_paths = Vec::new();
    set_intersection_and_difference(
        &a_paths,
        &b_paths,
        &mut shared_children,
        &mut a_only_paths,
        &mut b_only_paths,
    );

    for a_path in &a_only_paths {
        removed_entries.push(RemovedPrimEntry::new(a_path.clone()));
    }

    for common_child in &shared_children {
        compute_delta_diff_helper(
            si_a,
            si_b,
            common_child,
            removed_entries,
            added_entries,
            dirtied_entries,
        );
    }

    for b_path in &b_only_paths {
        fill_added_child_entries(si_b, b_path, added_entries);
    }
}

/// Default compute diff function: sparse delta.
pub fn compute_scene_index_diff_delta_fn() -> ComputeSceneIndexDiffFn {
    Box::new(|si_a, si_b, removed, added, renamed, dirtied| {
        compute_scene_index_diff_delta(si_a, si_b, removed, added, renamed, dirtied);
    })
}

/// Diff result between two scene indices.
#[derive(Debug, Default, Clone)]
pub struct SceneIndexDiff {
    /// Paths added in new scene index
    pub added: Vec<SdfPath>,
    /// Paths removed from old scene index
    pub removed: Vec<SdfPath>,
    /// Paths that exist in both but may have changes
    pub modified: Vec<SdfPath>,
}

impl SceneIndexDiff {
    /// Returns true if there are no differences.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.modified.is_empty()
    }

    /// Returns total number of differences.
    pub fn len(&self) -> usize {
        self.added.len() + self.removed.len() + self.modified.len()
    }
}

/// Compute diff between two scene indices.
///
/// Compares the scene graphs and identifies added, removed, and modified prims.
/// Starts from the absolute root and traverses the entire scene.
pub fn compute_scene_index_diff(
    old_scene: &dyn HdSceneIndexBase,
    new_scene: &dyn HdSceneIndexBase,
) -> SceneIndexDiff {
    compute_scene_index_diff_at_path(old_scene, new_scene, &SdfPath::absolute_root())
}

/// Compute diff at a specific subtree.
///
/// Only compares prims at or under the given root path.
pub fn compute_scene_index_diff_at_path(
    old_scene: &dyn HdSceneIndexBase,
    new_scene: &dyn HdSceneIndexBase,
    root_path: &SdfPath,
) -> SceneIndexDiff {
    let mut diff = SceneIndexDiff::default();

    // Collect all paths from both scenes
    let old_paths = collect_all_paths(old_scene, root_path);
    let new_paths = collect_all_paths(new_scene, root_path);

    // Convert to sets for efficient comparison
    let old_set: HashSet<_> = old_paths.iter().collect();
    let new_set: HashSet<_> = new_paths.iter().collect();

    // Find added paths (in new but not in old)
    for path in &new_paths {
        if !old_set.contains(path) {
            diff.added.push(path.clone());
        }
    }

    // Find removed paths (in old but not in new)
    for path in &old_paths {
        if !new_set.contains(path) {
            diff.removed.push(path.clone());
        }
    }

    // Find modified paths (in both, check if prim changed)
    for path in &old_paths {
        if new_set.contains(path) {
            // Check if prim type changed
            let old_prim = old_scene.get_prim(path);
            let new_prim = new_scene.get_prim(path);

            if old_prim.prim_type != new_prim.prim_type {
                diff.modified.push(path.clone());
            }
            // Note: Full implementation would also compare data sources
            // using HdDataSourceLocator-based comparison
        }
    }

    diff
}

/// Recursively collect all prim paths from a scene index starting at root.
fn collect_all_paths(scene: &dyn HdSceneIndexBase, root: &SdfPath) -> Vec<SdfPath> {
    let mut paths = Vec::new();
    collect_paths_recursive(scene, root, &mut paths);
    paths
}

/// Helper to recursively collect paths.
fn collect_paths_recursive(
    scene: &dyn HdSceneIndexBase,
    path: &SdfPath,
    result: &mut Vec<SdfPath>,
) {
    // Add current path if it's not the pseudo-root
    let prim = scene.get_prim(path);
    if !prim.prim_type.is_empty() {
        result.push(path.clone());
    }

    // Recurse into children
    for child_path in scene.get_child_prim_paths(path) {
        collect_paths_recursive(scene, &child_path, result);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scene_index_diff_default() {
        let diff = SceneIndexDiff::default();
        assert!(diff.added.is_empty());
        assert!(diff.removed.is_empty());
        assert!(diff.modified.is_empty());
        assert!(diff.is_empty());
        assert_eq!(diff.len(), 0);
    }

    #[test]
    fn test_scene_index_diff_len() {
        let mut diff = SceneIndexDiff::default();
        diff.added.push(SdfPath::from_string("/A").unwrap());
        diff.removed.push(SdfPath::from_string("/B").unwrap());
        diff.modified.push(SdfPath::from_string("/C").unwrap());

        assert!(!diff.is_empty());
        assert_eq!(diff.len(), 3);
    }
}
