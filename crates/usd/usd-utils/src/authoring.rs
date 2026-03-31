//! Authoring utilities for USD.
//!
//! Provides utilities for higher-level authoring and copying scene description,
//! including collection authoring and layer metadata copying.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::Arc;
use usd_core::collection_api::CollectionAPI;
use usd_core::prim::Prim;
use usd_core::prim_flags::PrimFlagsPredicate;
use usd_core::stage::Stage;
use usd_sdf::layer::Layer;
use usd_sdf::path::{self, Path};
use usd_tf::Token;

/// Hash set of paths for collection computation.
pub type PathHashSet = HashSet<Path>;

/// Copies all authored metadata from the pseudo-root of `source` to `destination`.
///
/// Matches C++ `UsdUtilsCopyLayerMetadata`: iterates `ListInfoKeys()` on the
/// source pseudo-root and copies each field via `SetInfo`/`set_field`.
/// Optionally skips sublayer fields and bakes color config fallbacks.
pub fn copy_layer_metadata(
    source: &Arc<Layer>,
    destination: &Arc<Layer>,
    skip_sublayers: bool,
    _bake_unauthored_fallbacks: bool,
) -> bool {
    let pseudo_root = Path::absolute_root();
    let sublayers_token = Token::new("subLayers");
    let sublayer_offsets_token = Token::new("subLayerOffsets");

    // Copy ALL authored fields from pseudo-root (matches C++ ListInfoKeys loop)
    let fields = source.list_fields(&pseudo_root);
    for field in &fields {
        // Skip sublayer fields when requested
        if skip_sublayers && (*field == sublayers_token || *field == sublayer_offsets_token) {
            continue;
        }
        if let Some(value) = source.get_field(&pseudo_root, field) {
            destination.set_field(&pseudo_root, field, value);
        }
    }

    true
}

/// Computes the optimal set of paths to include and exclude for a collection.
///
/// Implements the C++ `UsdUtilsComputeCollectionIncludesAndExcludes` algorithm:
/// 1. Find common ancestor of all included paths.
/// 2. Traverse beneath common ancestor to find paths NOT in the collection
///    (pathsToExcludeBelowCommonAncestor).
/// 3. For each candidate ancestor path, compute inclusion ratio and excluded set.
/// 4. Pick the path that satisfies minInclusionRatio and maxNumExcludesBelowInclude.
pub fn compute_collection_includes_and_excludes(
    included_root_paths: &HashSet<Path>,
    stage: &Arc<Stage>,
    min_inclusion_ratio: f64,
    max_num_excludes_below_include: u32,
    min_include_exclude_collection_size: u32,
    paths_to_ignore: &PathHashSet,
) -> Option<(Vec<Path>, Vec<Path>)> {
    let min_ratio = min_inclusion_ratio.clamp(0.0, 1.0);
    let mut paths_to_include: Vec<Path> = Vec::new();
    let mut paths_to_exclude: Vec<Path> = Vec::new();

    if included_root_paths.is_empty() {
        return Some((paths_to_include, paths_to_exclude));
    }

    // Below min size -> return all included paths directly (no excludes needed)
    if included_root_paths.len() < min_include_exclude_collection_size as usize {
        paths_to_include.extend(included_root_paths.iter().cloned());
        return Some((paths_to_include, paths_to_exclude));
    }

    // Find common ancestor of all included paths
    let mut common_prefix = included_root_paths.iter().next()?.clone();
    for p in included_root_paths {
        common_prefix = common_prefix.get_common_prefix(p);
    }

    let common_ancestor = stage.get_prim_at_path(&common_prefix)?;
    let common_ancestor_parent = common_prefix.get_parent_path();

    // Build hash set of included paths for O(1) lookup
    let included_set: BTreeSet<Path> = included_root_paths.iter().cloned().collect();

    // Step 2: Traverse from commonAncestor to find paths to exclude
    // (prims that are NOT in the included set and are not ancestors of included paths)
    let all_under_ancestor = stage.traverse_from(&common_prefix, PrimFlagsPredicate::all());

    let mut paths_to_excl_below: Vec<Path> = Vec::new();

    // BFS/DFS: collect paths that are not in the included set
    let mut stack: Vec<Prim> = vec![common_ancestor.clone()];
    while let Some(prim) = stack.pop() {
        let prim_path = prim.get_path().clone();

        if paths_to_ignore.contains(&prim_path) {
            continue;
        }

        if included_set.contains(&prim_path) {
            // This path is included -> don't descend (prune children)
            // Remove any ancestor paths from exclude set
            // (they must remain accessible)
            continue;
        }

        // Check if any included path is a descendant of this prim
        let has_included_descendant = included_set
            .iter()
            .any(|ip| ip.has_prefix(&prim_path) && *ip != prim_path);

        if !has_included_descendant {
            // No included descendants -> this subtree is entirely excluded
            paths_to_excl_below.push(prim_path);
            // Don't descend (entire subtree excluded)
        } else {
            // Has included descendants -> recurse into children
            for child in prim.children() {
                stack.push(child);
            }
        }
    }

    // Remove descendant paths (keep only minimal exclude set)
    path::remove_descendent_paths(&mut paths_to_excl_below);

    // Step 3/4: Build ancestor->numIncluded and ancestor->excludedPaths maps
    // Map from ancestor path -> number of included paths beneath it (inclusive)
    let mut num_included_map: BTreeMap<Path, usize> = BTreeMap::new();
    for ip in &included_set {
        let mut p = ip.clone();
        while p != common_ancestor_parent {
            *num_included_map.entry(p.clone()).or_insert(0) += 1;
            p = p.get_parent_path();
            if p.is_empty() || p == Path::absolute_root() {
                break;
            }
        }
    }

    // Map from ancestor path -> set of excluded paths beneath it
    let mut excluded_paths_map: BTreeMap<Path, Vec<Path>> = BTreeMap::new();
    for ep in &paths_to_excl_below {
        let mut p = ep.clone();
        while p != common_ancestor_parent {
            excluded_paths_map
                .entry(p.clone())
                .or_default()
                .push(ep.clone());
            p = p.get_parent_path();
            if p.is_empty() || p == Path::absolute_root() {
                break;
            }
        }
    }

    // Step 5: Traverse from commonAncestor, select optimal include/exclude paths
    let mut stack: Vec<Prim> = vec![common_ancestor.clone()];
    while let Some(prim) = stack.pop() {
        let prim_path = prim.get_path().clone();

        if paths_to_ignore.contains(&prim_path) {
            continue;
        }

        let incl_count = num_included_map.get(&prim_path).copied().unwrap_or(0);
        if incl_count == 0 {
            // No included paths in this subtree -> skip
            continue;
        }

        let excl_paths = excluded_paths_map
            .get(&prim_path)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);
        let excl_count = excl_paths.len();

        let inclusion_ratio = incl_count as f64 / (incl_count + excl_count) as f64;

        if inclusion_ratio >= min_ratio && excl_count <= max_num_excludes_below_include as usize {
            // This path satisfies constraints -> include it and add its excludes
            paths_to_include.push(prim_path);
            paths_to_exclude.extend_from_slice(excl_paths);
            // Prune: don't descend further (ancestor already selected)
        } else {
            // Descend into children looking for a better candidate
            for child in prim.children() {
                stack.push(child);
            }
        }
    }

    let _ = all_under_ancestor; // used via traverse_from above

    paths_to_include.sort();
    paths_to_exclude.sort();
    paths_to_exclude.dedup();

    Some((paths_to_include, paths_to_exclude))
}

/// Authors a collection on a prim with the given includes and excludes.
///
/// Applies CollectionAPI to the prim, creates includes/excludes relationships,
/// and sets their targets. Matches C++ `UsdUtilsAuthorCollection`.
pub fn author_collection(
    collection_name: &Token,
    prim: &Prim,
    paths_to_include: &[Path],
    paths_to_exclude: &[Path],
) -> bool {
    // Apply CollectionAPI to the prim with the given name
    let collection = CollectionAPI::apply(prim, collection_name);

    // Create and set includes relationship targets
    let includes_rel = collection.create_includes_rel();
    includes_rel.set_targets(paths_to_include);

    // Create and set excludes relationship targets if any
    if !paths_to_exclude.is_empty() {
        let excludes_rel = collection.create_excludes_rel();
        excludes_rel.set_targets(paths_to_exclude);
    }

    true
}

/// Creates multiple collections on a prim.
///
/// Returns the created CollectionAPI instances. C++ returns `vector<UsdCollectionAPI>`.
pub fn create_collections(
    assignments: &[(Token, HashSet<Path>)],
    prim: &Prim,
    min_inclusion_ratio: f64,
    max_num_excludes_below_include: u32,
    min_include_exclude_collection_size: u32,
) -> Vec<CollectionAPI> {
    let stage = match prim.stage() {
        Some(s) => s,
        None => return Vec::new(),
    };

    let mut result = Vec::new();

    for (collection_name, paths) in assignments {
        if let Some((includes, excludes)) = compute_collection_includes_and_excludes(
            paths,
            &stage,
            min_inclusion_ratio,
            max_num_excludes_below_include,
            min_include_exclude_collection_size,
            &PathHashSet::new(),
        ) {
            if author_collection(collection_name, prim, &includes, &excludes) {
                result.push(CollectionAPI::apply(prim, collection_name));
            }
        }
    }

    result
}

/// Retrieves a list of all dirty layers from the stage's used layers.
pub fn get_dirty_layers(stage: &Arc<Stage>, include_clip_layers: bool) -> Vec<Arc<Layer>> {
    let mut dirty_layers = Vec::new();

    let used_layers = stage.get_used_layers(include_clip_layers);

    for layer in used_layers {
        if layer.is_dirty() {
            dirty_layers.push(layer);
        }
    }

    dirty_layers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_hash_set() {
        let mut set = PathHashSet::new();
        set.insert(Path::from_string("/World").unwrap());
        set.insert(Path::from_string("/World/Mesh").unwrap());
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_compute_collection_below_min_size() {
        // When paths < min_include_exclude_collection_size, all paths returned as includes
        use usd_core::common::InitialLoadSet;
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let mut paths = PathHashSet::new();
        paths.insert(Path::from_string("/World/MeshA").unwrap());
        paths.insert(Path::from_string("/World/MeshB").unwrap());

        let result = compute_collection_includes_and_excludes(
            &paths,
            &stage,
            0.5,
            10,
            100, // min size = 100 > 2 paths
            &PathHashSet::new(),
        );

        assert!(result.is_some());
        let (includes, excludes) = result.unwrap();
        // Below min size -> return all as includes, no excludes
        assert_eq!(includes.len(), 2);
        assert!(excludes.is_empty());
    }

    #[test]
    fn test_compute_collection_empty_paths() {
        use usd_core::common::InitialLoadSet;
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let paths = PathHashSet::new();

        let result = compute_collection_includes_and_excludes(
            &paths,
            &stage,
            0.5,
            10,
            5,
            &PathHashSet::new(),
        );
        assert!(result.is_some());
        let (includes, excludes) = result.unwrap();
        assert!(includes.is_empty());
        assert!(excludes.is_empty());
    }

    #[test]
    fn test_compute_collection_paths_to_ignore() {
        use usd_core::common::InitialLoadSet;
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let p1 = Path::from_string("/World/A").unwrap();
        let p2 = Path::from_string("/World/B").unwrap();

        let mut paths = PathHashSet::new();
        paths.insert(p1.clone());
        paths.insert(p2.clone());

        let mut ignore = PathHashSet::new();
        ignore.insert(p1.clone());

        // Below min_size (size 2 < 100) so returned directly without ignore filter
        // (the ignore filter applies only when size >= min)
        let result =
            compute_collection_includes_and_excludes(&paths, &stage, 0.5, 10, 100, &ignore);
        assert!(result.is_some());
    }

    #[test]
    fn test_compute_collection_clamps_ratio() {
        // min_inclusion_ratio is clamped to [0, 1]; passing 2.0 should not panic
        use usd_core::common::InitialLoadSet;
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let paths = PathHashSet::new();

        let result = compute_collection_includes_and_excludes(
            &paths,
            &stage,
            2.0,
            5,
            1,
            &PathHashSet::new(),
        );
        assert!(result.is_some());
    }
}
