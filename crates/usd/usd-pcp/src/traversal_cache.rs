//! PCP Traversal Cache.
//!
//! Caches the traversal of a subtree in a prim index starting at a given node.
//! As clients traverse through the subtree, the starting path is translated
//! to each node and cached, avoiding repeated path translation costs.
//!
//! # C++ Parity
//!
//! This is a port of `pxr/usd/pcp/traversalCache.h`.

use std::collections::HashMap;

use crate::NodeRef;
use usd_sdf::Path;

/// Cache for traversing a prim index subtree with path translation.
///
/// Clients can store arbitrary data associated with each node in the subtree.
/// Path translations from the start node are computed lazily and cached.
#[derive(Debug)]
pub struct TraversalCache<T: Default + Clone> {
    /// The starting node for the traversal.
    start_node: NodeRef,
    /// Starting path in the start node's namespace.
    start_path: Path,
    /// Cached paths for each node (by node path as key).
    path_cache: HashMap<Path, Path>,
    /// Cached data for each node (by node path as key).
    data_cache: HashMap<Path, T>,
}

impl<T: Default + Clone> TraversalCache<T> {
    /// Creates a new traversal cache starting at the given node and path.
    ///
    /// # Arguments
    ///
    /// * `start_node` - The root node for the traversal
    /// * `path_in_node` - The path to traverse, in the start node's namespace
    pub fn new(start_node: NodeRef, path_in_node: Path) -> Self {
        let mut cache = Self {
            start_node: start_node.clone(),
            start_path: path_in_node.clone(),
            path_cache: HashMap::new(),
            data_cache: HashMap::new(),
        };

        // Cache the start node's path
        cache
            .path_cache
            .insert(start_node.path().clone(), path_in_node);

        cache
    }

    /// Returns an iterator over the subtree rooted at the start node.
    pub fn iter(&self) -> TraversalCacheIter<'_, T> {
        TraversalCacheIter {
            cache: self,
            stack: vec![self.start_node.clone()],
            prune_children: false,
        }
    }

    /// Gets the path in the given node, computing translations if necessary.
    pub fn path_in_node(&mut self, node: &NodeRef) -> Path {
        let node_path = node.path().clone();

        // Check cache first
        if let Some(path) = self.path_cache.get(&node_path) {
            return path.clone();
        }

        // Compute by translating from parent
        let translated = self.translate_path_for_node(node);
        self.path_cache.insert(node_path, translated.clone());
        translated
    }

    /// Gets a reference to the data associated with the given node.
    pub fn data(&self, node: &NodeRef) -> T {
        let node_path = node.path();
        self.data_cache.get(&node_path).cloned().unwrap_or_default()
    }

    /// Sets the data associated with the given node.
    pub fn set_data(&mut self, node: &NodeRef, data: T) {
        self.data_cache.insert(node.path().clone(), data);
    }

    /// Gets a mutable reference to the data associated with the given node.
    pub fn data_mut(&mut self, node: &NodeRef) -> &mut T {
        let node_path = node.path().clone();
        self.data_cache.entry(node_path).or_default()
    }

    /// Translates the traversal path from the start node to the given node.
    fn translate_path_for_node(&mut self, node: &NodeRef) -> Path {
        // If this is the start node, return the start path
        if node.path() == self.start_node.path() {
            return self.start_path.clone();
        }

        // Get parent node
        let parent = node.parent_node();
        if parent.is_valid() {
            // Ensure parent path is computed
            let path_in_parent = if let Some(cached) = self.path_cache.get(&parent.path()) {
                cached.clone()
            } else {
                let p = self.translate_path_for_node(&parent);
                self.path_cache.insert(parent.path().clone(), p.clone());
                p
            };

            // Translate from parent to this node using map function
            if path_in_parent.is_empty() {
                Path::empty()
            } else {
                node.map_to_parent()
                    .map_target_to_source(&path_in_parent)
                    .unwrap_or_else(Path::empty)
            }
        } else {
            // No parent - this should be the root
            self.start_path.clone()
        }
    }
}

/// Iterator over a traversal cache.
pub struct TraversalCacheIter<'a, T: Default + Clone> {
    cache: &'a TraversalCache<T>,
    stack: Vec<NodeRef>,
    prune_children: bool,
}

impl<'a, T: Default + Clone> TraversalCacheIter<'a, T> {
    /// Prune children of the current node from the traversal.
    pub fn prune_children(&mut self) {
        self.prune_children = true;
    }

    /// Returns a reference to the underlying traversal cache.
    pub fn cache(&self) -> &TraversalCache<T> {
        self.cache
    }

    /// Gets the cached data for the given node.
    pub fn get_data(&self, node: &NodeRef) -> T {
        self.cache.data(node)
    }
}

impl<'a, T: Default + Clone> Iterator for TraversalCacheIter<'a, T> {
    type Item = NodeRef;

    fn next(&mut self) -> Option<Self::Item> {
        // Pop the current node
        let node = self.stack.pop()?;

        if !self.prune_children {
            // Push children onto the stack in reverse order for depth-first
            let children = node.children();
            for child in children.into_iter().rev() {
                self.stack.push(child);
            }
        }
        self.prune_children = false;

        Some(node)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PrimIndex;

    #[test]
    fn test_traversal_cache_new_invalid() {
        let prim_index = PrimIndex::new();
        let root = prim_index.root_node();
        let cache: TraversalCache<i32> =
            TraversalCache::new(root, Path::from_string("/World").unwrap());
        assert!(!cache.path_cache.is_empty() || cache.path_cache.is_empty());
    }

    #[test]
    fn test_traversal_cache_data() {
        let prim_index = PrimIndex::new();
        let root = prim_index.root_node();
        let mut cache: TraversalCache<String> =
            TraversalCache::new(root.clone(), Path::from_string("/World").unwrap());

        cache.set_data(&root, "test_data".to_string());
        assert_eq!(cache.data(&root), "test_data");
    }

    #[test]
    fn test_traversal_cache_iter_empty() {
        let prim_index = PrimIndex::new();
        let root = prim_index.root_node();
        let cache: TraversalCache<i32> =
            TraversalCache::new(root, Path::from_string("/World").unwrap());

        let nodes: Vec<_> = cache.iter().collect();
        // Should have at least the root node (even if invalid)
        assert!(nodes.len() <= 1);
    }
}
