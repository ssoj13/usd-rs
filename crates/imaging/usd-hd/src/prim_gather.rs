//! HdPrimGather - Filter paths by include/exclude prefixes.
//!
//! Corresponds to pxr/imaging/hd/primGather.h.
//! Paths must be pre-sorted. Supports Filter, PredicatedFilter, Subtree.

use std::cmp::Ordering;
use usd_sdf::Path as SdfPath;

/// Path vector type.
pub type SdfPathVector = Vec<SdfPath>;

/// Filter paths by include/exclude prefix rules.
///
/// Corresponds to C++ `HdPrimGather`.
/// - Path is included if prefixed by at least one include path
/// - Path is excluded if prefixed by an exclude path with more elements
///   than the best matching include prefix
#[derive(Debug, Default)]
pub struct HdPrimGather;

impl HdPrimGather {
    /// Filter paths. Paths must be pre-sorted. Results may be unsorted.
    pub fn filter(
        paths: &[SdfPath],
        include_paths: &[SdfPath],
        exclude_paths: &[SdfPath],
        results: &mut SdfPathVector,
    ) {
        results.clear();
        if paths.is_empty() {
            return;
        }
        for path in paths {
            if Self::passes_filter(path, include_paths, exclude_paths) {
                results.push(path.clone());
            }
        }
    }

    /// Filter with predicate. Predicate must be thread-safe.
    pub fn predicated_filter<F>(
        paths: &[SdfPath],
        include_paths: &[SdfPath],
        exclude_paths: &[SdfPath],
        predicate: F,
        results: &mut SdfPathVector,
    ) where
        F: Fn(&SdfPath) -> bool,
    {
        results.clear();
        if paths.is_empty() {
            return;
        }
        for path in paths {
            if Self::passes_filter(path, include_paths, exclude_paths) && predicate(path) {
                results.push(path.clone());
            }
        }
    }

    /// Subtree: gather paths under root_path. Results maintain sorted order.
    pub fn subtree(paths: &[SdfPath], root_path: &SdfPath, results: &mut SdfPathVector) {
        results.clear();
        if paths.is_empty() {
            return;
        }
        let (start, end) = match Self::subtree_range(paths, root_path) {
            Some(r) => r,
            None => return,
        };
        results.extend(paths[start..=end].iter().cloned());
    }

    /// Subtree as range. Returns (start, end) inclusive, or None.
    pub fn subtree_as_range(paths: &[SdfPath], root_path: &SdfPath) -> Option<(usize, usize)> {
        if paths.is_empty() {
            return None;
        }
        Self::subtree_range(paths, root_path)
    }

    fn passes_filter(path: &SdfPath, include_paths: &[SdfPath], exclude_paths: &[SdfPath]) -> bool {
        let best_include = include_paths
            .iter()
            .filter(|inc| path.has_prefix(inc))
            .max_by(|a, b| a.get_path_element_count().cmp(&b.get_path_element_count()));
        let best_include_len = best_include
            .map(|p| p.get_path_element_count())
            .unwrap_or(0);
        let excluded = exclude_paths
            .iter()
            .any(|exc| path.has_prefix(exc) && exc.get_path_element_count() > best_include_len);
        best_include.is_some() && !excluded
    }

    fn subtree_range(paths: &[SdfPath], root_path: &SdfPath) -> Option<(usize, usize)> {
        let start = Self::find_lower_bound(paths, 0, paths.len(), root_path);
        if start >= paths.len() || !paths[start].has_prefix(root_path) {
            return None;
        }
        let end = Self::find_upper_bound(paths, start, paths.len() - 1, root_path);
        Some((start, end))
    }

    fn find_lower_bound(paths: &[SdfPath], start: usize, end: usize, path: &SdfPath) -> usize {
        let mut low = start;
        let mut size = end - start;
        while size > 0 {
            let mid = low + size / 2;
            match paths[mid].cmp(path) {
                Ordering::Less => {
                    low = mid + 1;
                    size -= size / 2 + 1;
                }
                _ => size = size / 2,
            }
        }
        low
    }

    fn find_upper_bound(paths: &[SdfPath], start: usize, end: usize, path: &SdfPath) -> usize {
        if paths[end].has_prefix(path) {
            return end;
        }
        let mut low = start;
        let mut size = end - start;
        while size > 0 {
            let mid = low + size / 2;
            if paths[mid].has_prefix(path) {
                low = mid + 1;
                size -= size / 2 + 1;
            } else {
                size = size / 2;
            }
        }
        low.saturating_sub(1)
    }
}
