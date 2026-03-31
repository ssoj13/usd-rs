//! Parallel prim index computation.
//!
//! Ports C++ `PcpCache::_ParallelIndexer` using rayon for level-based
//! parallelism. Paths are grouped by depth so that parent indexes are
//! guaranteed to be computed before their children.
//!
//! # C++ Reference
//!
//! See `pxr/usd/pcp/cache.cpp` `_ParallelIndexer` struct.

use std::collections::HashMap;
use std::sync::Mutex;

use rayon::prelude::*;

use crate::{
    ErrorType, LayerStackRefPtr, PrimIndex, PrimIndexInputs, PrimIndexOutputs, VariantFallbackMap,
    compute_prim_index,
};
use usd_sdf::Path;

/// Outputs from parallel index computation.
pub struct ParallelIndexerOutputs {
    /// Computed prim indices keyed by path.
    pub results: HashMap<Path, PrimIndex>,
    /// All errors from every path computation.
    pub all_errors: Vec<ErrorType>,
}

/// Parallel prim index computer.
///
/// Groups paths by depth and processes each level with rayon `par_iter`,
/// guaranteeing that parent indexes complete before children start.
/// This mirrors the C++ `_ParallelIndexer` approach but uses depth-level
/// parallelism instead of a TBB work-stealing dispatcher.
pub struct ParallelIndexer {
    layer_stack: LayerStackRefPtr,
    variant_fallbacks: VariantFallbackMap,
    is_usd: bool,
}

impl ParallelIndexer {
    /// Creates a new parallel indexer.
    pub fn new(
        layer_stack: LayerStackRefPtr,
        variant_fallbacks: VariantFallbackMap,
        is_usd: bool,
    ) -> Self {
        Self {
            layer_stack,
            variant_fallbacks,
            is_usd,
        }
    }

    /// Build shared PrimIndexInputs from our configuration.
    fn make_inputs(&self) -> PrimIndexInputs {
        PrimIndexInputs::new()
            .usd(self.is_usd)
            .variant_fallbacks(self.variant_fallbacks.clone())
    }

    /// Compute prim indexes for the given paths in parallel.
    ///
    /// Paths are grouped by depth (element count). Each depth level is
    /// processed in parallel; all parents are guaranteed done before children.
    pub fn compute_indexes(&self, paths: &[Path]) -> ParallelIndexerOutputs {
        if paths.is_empty() {
            return ParallelIndexerOutputs {
                results: HashMap::new(),
                all_errors: Vec::new(),
            };
        }

        // Group paths by depth (element count)
        let mut by_depth: Vec<Vec<&Path>> = Vec::new();
        for path in paths {
            let depth = path.get_path_element_count();
            if depth >= by_depth.len() {
                by_depth.resize_with(depth + 1, Vec::new);
            }
            by_depth[depth].push(path);
        }

        let results: Mutex<HashMap<Path, PrimIndex>> =
            Mutex::new(HashMap::with_capacity(paths.len()));
        let all_errors: Mutex<Vec<ErrorType>> = Mutex::new(Vec::new());

        // Process each depth level sequentially; within each level, compute in parallel
        for level_paths in &by_depth {
            if level_paths.is_empty() {
                continue;
            }

            let level_outputs: Vec<(Path, PrimIndexOutputs)> = level_paths
                .par_iter()
                .map(|path| {
                    let inputs = self.make_inputs();
                    let outputs = compute_prim_index(path, &self.layer_stack, &inputs);
                    ((*path).clone(), outputs)
                })
                .collect();

            // Publish results from this level before moving to the next
            let mut res = results.lock().expect("mutex poisoned");
            let mut errs = all_errors.lock().expect("mutex poisoned");
            for (path, outputs) in level_outputs {
                errs.extend(outputs.all_errors);
                res.insert(path, outputs.prim_index);
            }
        }

        ParallelIndexerOutputs {
            results: results.into_inner().expect("mutex poisoned"),
            all_errors: all_errors.into_inner().expect("mutex poisoned"),
        }
    }

    /// Compute indexes for an entire subtree rooted at `root`.
    ///
    /// First computes the root index, then discovers children by examining
    /// the composed namespace, and recursively computes child indexes
    /// in parallel at each depth level.
    pub fn compute_subtree(&self, root: &Path) -> ParallelIndexerOutputs {
        let mut results: HashMap<Path, PrimIndex> = HashMap::new();
        let mut all_errors: Vec<ErrorType> = Vec::new();

        // Compute root index first
        let inputs = self.make_inputs();
        let root_outputs = compute_prim_index(root, &self.layer_stack, &inputs);
        all_errors.extend(root_outputs.all_errors);

        let root_index = root_outputs.prim_index;
        if !root_index.is_valid() {
            results.insert(root.clone(), root_index);
            return ParallelIndexerOutputs {
                results,
                all_errors,
            };
        }
        results.insert(root.clone(), root_index);

        // BFS: discover children at each level, compute them in parallel
        let mut current_paths = vec![root.clone()];

        while !current_paths.is_empty() {
            // Discover child paths from current level
            let child_paths: Vec<Path> = current_paths
                .iter()
                .flat_map(|parent_path| {
                    if let Some(idx) = results.get(parent_path) {
                        let (child_names, _prohibited) = idx.compute_prim_child_names();
                        child_names
                            .into_iter()
                            .filter_map(|name| parent_path.append_child(name.as_str()))
                            .collect::<Vec<_>>()
                    } else {
                        Vec::new()
                    }
                })
                .collect();

            if child_paths.is_empty() {
                break;
            }

            // Compute all children at this depth in parallel
            let level_outputs: Vec<(Path, PrimIndexOutputs)> = child_paths
                .par_iter()
                .map(|path| {
                    let inputs = self.make_inputs();
                    let outputs = compute_prim_index(path, &self.layer_stack, &inputs);
                    (path.clone(), outputs)
                })
                .collect();

            current_paths = Vec::with_capacity(level_outputs.len());
            for (path, outputs) in level_outputs {
                all_errors.extend(outputs.all_errors);
                if outputs.prim_index.is_valid() {
                    current_paths.push(path.clone());
                }
                results.insert(path, outputs.prim_index);
            }
        }

        ParallelIndexerOutputs {
            results,
            all_errors,
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LayerStackIdentifier;
    use crate::layer_stack::LayerStack;

    fn make_layer_stack() -> LayerStackRefPtr {
        let id = LayerStackIdentifier::new("test.usda");
        LayerStack::new(id)
    }

    #[test]
    fn test_parallel_empty_paths() {
        let ls = make_layer_stack();
        let indexer = ParallelIndexer::new(ls, VariantFallbackMap::new(), true);
        let out = indexer.compute_indexes(&[]);
        assert!(out.results.is_empty());
        assert!(out.all_errors.is_empty());
    }

    #[test]
    fn test_parallel_single_path() {
        let ls = make_layer_stack();
        let indexer = ParallelIndexer::new(ls, VariantFallbackMap::new(), true);
        let path = Path::from_string("/World").unwrap();
        let out = indexer.compute_indexes(&[path.clone()]);
        assert_eq!(out.results.len(), 1);
        assert!(out.results.contains_key(&path));
        assert!(out.results[&path].is_valid());
    }

    #[test]
    fn test_parallel_matches_sequential() {
        let ls = make_layer_stack();
        let paths: Vec<Path> = ["/A", "/B", "/A/C", "/A/D", "/B/E", "/A/C/F"]
            .iter()
            .filter_map(|s| Path::from_string(s))
            .collect();

        // Compute sequentially
        let mut seq_results: HashMap<Path, PrimIndex> = HashMap::new();
        for p in &paths {
            let inputs = PrimIndexInputs::new().usd(true);
            let outputs = compute_prim_index(p, &ls, &inputs);
            seq_results.insert(p.clone(), outputs.prim_index);
        }

        // Compute in parallel
        let indexer = ParallelIndexer::new(ls, VariantFallbackMap::new(), true);
        let par_out = indexer.compute_indexes(&paths);

        // Same keys, all valid
        assert_eq!(seq_results.len(), par_out.results.len());
        for (path, seq_idx) in &seq_results {
            assert!(
                par_out.results.contains_key(path),
                "missing {}",
                path.as_str()
            );
            assert_eq!(seq_idx.is_valid(), par_out.results[path].is_valid());
        }
    }

    #[test]
    fn test_parallel_depth_ordering() {
        // Verify that paths at different depths are all computed
        let ls = make_layer_stack();
        let paths: Vec<Path> = ["/Root", "/Root/Child", "/Root/Child/GrandChild"]
            .iter()
            .filter_map(|s| Path::from_string(s))
            .collect();

        let indexer = ParallelIndexer::new(ls, VariantFallbackMap::new(), true);
        let out = indexer.compute_indexes(&paths);

        assert_eq!(out.results.len(), 3);
        for p in &paths {
            assert!(out.results[p].is_valid(), "{} should be valid", p.as_str());
        }
    }

    #[test]
    fn test_parallel_subtree() {
        let ls = make_layer_stack();
        let root = Path::from_string("/World").unwrap();
        let indexer = ParallelIndexer::new(ls, VariantFallbackMap::new(), true);
        let out = indexer.compute_subtree(&root);

        // At minimum the root should be in results
        assert!(out.results.contains_key(&root));
        assert!(out.results[&root].is_valid());
    }

    #[test]
    fn test_parallel_many_paths() {
        // Test with a larger set to exercise rayon parallelism
        let ls = make_layer_stack();
        let mut paths = Vec::new();
        for i in 0..50 {
            let p = Path::from_string(&format!("/Prim{}", i)).unwrap();
            paths.push(p);
        }

        let indexer = ParallelIndexer::new(ls, VariantFallbackMap::new(), true);
        let out = indexer.compute_indexes(&paths);

        assert_eq!(out.results.len(), 50);
        for p in &paths {
            assert!(out.results[p].is_valid());
        }
    }
}
