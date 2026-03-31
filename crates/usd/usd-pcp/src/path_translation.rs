//! Path translation between nodes in a composition graph.
//!
//! This module provides functions for translating paths between the namespace
//! of a prim index node and the root node, applying all necessary namespace
//! translations from the composition arcs.
//!
//! # C++ Parity
//!
//! This is a port of `pxr/usd/pcp/pathTranslation.h` and `pathTranslation.cpp`.
//!
//! # Key Functions
//!
//! - `translate_path_from_node_to_root` - Forward translation from node to root
//! - `translate_path_from_root_to_node` - Reverse translation from root to node
//! - `translate_target_path_from_root_to_node` - For relationship/connection targets

use crate::{MapExpression, MapFunction, NodeRef};
use usd_sdf::Path;

// ============================================================================
// Internal Translation
// ============================================================================

/// Internal path translation using a map function.
///
/// Template parameter `NODE_TO_ROOT`:
/// - true: translate from node namespace to root namespace (forward)
/// - false: translate from root namespace to node namespace (reverse)
fn translate_path_impl<F>(map_to_root: &F, path: &Path, node_to_root: bool) -> (Path, bool)
where
    F: PathMapper,
{
    // Handle null/invalid map
    if map_to_root.is_null() {
        return (Path::empty(), false);
    }

    // Empty path translates to itself
    if path.is_empty() {
        return (path.clone(), true);
    }

    // Path must be absolute
    if !path.is_absolute_path() {
        return (Path::empty(), false);
    }

    // Path must not contain variant selections (map functions don't handle them)
    if path.contains_prim_variant_selection() {
        return (Path::empty(), false);
    }

    // Identity mapping returns the path unchanged
    if map_to_root.is_identity() {
        return (path.clone(), true);
    }

    // Translate the path using the map function
    let translated_path = if node_to_root {
        map_to_root.map_source_to_target(path)
    } else {
        map_to_root.map_target_to_source(path)
    };

    // Check if translation succeeded
    let translated_path = match translated_path {
        Some(p) if !p.is_empty() => p,
        _ => return (Path::empty(), false),
    };

    // Translate any target paths embedded in the path
    // (e.g., in relationship targets like /Prim.rel[/Target])
    let target_paths = translated_path.get_all_target_paths_recursively();

    let mut result_path = translated_path;
    for target_path in target_paths {
        let translated_target = if node_to_root {
            map_to_root.map_source_to_target(&target_path)
        } else {
            map_to_root.map_target_to_source(&target_path)
        };

        // If any target path fails to translate, the whole translation fails
        let translated_target = match translated_target {
            Some(p) if !p.is_empty() => p,
            _ => return (Path::empty(), false),
        };

        // Replace the target path in the result
        if let Some(replaced) = result_path.replace_prefix(&target_path, &translated_target) {
            result_path = replaced;
        }
    }

    (result_path, true)
}

/// Trait for path mapping functionality.
trait PathMapper {
    fn is_null(&self) -> bool;
    fn is_identity(&self) -> bool;
    fn map_source_to_target(&self, path: &Path) -> Option<Path>;
    fn map_target_to_source(&self, path: &Path) -> Option<Path>;
}

impl PathMapper for MapFunction {
    fn is_null(&self) -> bool {
        self.is_null()
    }

    fn is_identity(&self) -> bool {
        self.is_identity()
    }

    fn map_source_to_target(&self, path: &Path) -> Option<Path> {
        self.map_source_to_target(path)
    }

    fn map_target_to_source(&self, path: &Path) -> Option<Path> {
        self.map_target_to_source(path)
    }
}

impl PathMapper for MapExpression {
    fn is_null(&self) -> bool {
        self.is_null()
    }

    fn is_identity(&self) -> bool {
        self.is_identity()
    }

    fn map_source_to_target(&self, path: &Path) -> Option<Path> {
        self.map_source_to_target(path)
    }

    fn map_target_to_source(&self, path: &Path) -> Option<Path> {
        self.map_target_to_source(path)
    }
}

// ============================================================================
// Public API
// ============================================================================

/// Translates a path from the namespace of a prim index node to the root namespace.
///
/// This applies all necessary namespace translations from the composition arcs
/// between the source node and the root.
///
/// # Arguments
///
/// * `source_node` - The node whose namespace contains the path
/// * `path_in_node_namespace` - The path to translate
///
/// # Returns
///
/// A tuple of (translated_path, was_translated). If translation fails,
/// returns (empty_path, false).
///
/// # Example
///
/// ```ignore
/// let (translated, success) = translate_path_from_node_to_root(&node, &path);
/// if success {
///     println!("Translated: {}", translated);
/// }
/// ```
pub fn translate_path_from_node_to_root(
    source_node: &NodeRef,
    path_in_node_namespace: &Path,
) -> (Path, bool) {
    if !source_node.is_valid() {
        return (Path::empty(), false);
    }

    // Strip variant selections - map functions don't handle them
    let path_to_translate = path_in_node_namespace.strip_all_variant_selections();

    let map_to_root = source_node.map_to_root();
    translate_path_impl(&map_to_root, &path_to_translate, true)
}

/// Translates a path from the root namespace to the namespace of a prim index node.
///
/// This applies all necessary namespace translations from the composition arcs
/// between the root and the destination node.
///
/// # Arguments
///
/// * `dest_node` - The destination node whose namespace to translate to
/// * `path_in_root_namespace` - The path to translate (in root namespace)
///
/// # Returns
///
/// A tuple of (translated_path, was_translated). If translation fails,
/// returns (empty_path, false).
pub fn translate_path_from_root_to_node(
    dest_node: &NodeRef,
    path_in_root_namespace: &Path,
) -> (Path, bool) {
    if !dest_node.is_valid() {
        return (Path::empty(), false);
    }

    let map_to_root = dest_node.map_to_root();
    let (mut translated_path, was_translated) =
        translate_path_impl(&map_to_root, path_in_root_namespace, false);

    // Apply variant selections from the destination node's path
    // Map functions don't include variant selections, so we need to
    // add them back by prefix replacement
    if was_translated {
        let site_path = dest_node.path();
        let stripped_site_path = site_path.strip_all_variant_selections();

        if let Some(replaced) = translated_path.replace_prefix(&stripped_site_path, &site_path) {
            translated_path = replaced;
        }
    }

    (translated_path, was_translated)
}

/// Translates a target path from the root namespace to a node namespace.
///
/// This is similar to `translate_path_from_root_to_node` but is specifically
/// for attribute connections and relationship targets. The key difference is
/// that variant selections are never included in the result.
///
/// # Arguments
///
/// * `dest_node` - The destination node whose namespace to translate to
/// * `path_in_root_namespace` - The target path to translate
///
/// # Returns
///
/// A tuple of (translated_path, was_translated).
pub fn translate_target_path_from_root_to_node(
    dest_node: &NodeRef,
    path_in_root_namespace: &Path,
) -> (Path, bool) {
    if !dest_node.is_valid() {
        return (Path::empty(), false);
    }

    let map_to_root = dest_node.map_to_root();
    translate_path_impl(&map_to_root, path_in_root_namespace, false)
}

/// Translates a path from root to node using a map function directly.
///
/// This is a convenience function when you have the map function
/// but not the node reference.
pub fn translate_path_from_root_to_node_using_function(
    map_to_root: &MapFunction,
    path_in_root_namespace: &Path,
) -> (Path, bool) {
    translate_path_impl(map_to_root, path_in_root_namespace, false)
}

/// Translates a path from node to root using a map function directly.
///
/// This is a convenience function when you have the map function
/// but not the node reference.
pub fn translate_path_from_node_to_root_using_function(
    map_to_root: &MapFunction,
    path_in_node_namespace: &Path,
) -> (Path, bool) {
    // Strip variant selections - map functions don't handle them
    let path_to_translate = path_in_node_namespace.strip_all_variant_selections();
    translate_path_impl(map_to_root, &path_to_translate, true)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_translate_empty_path() {
        let map = MapFunction::identity().clone();
        let (result, success) =
            translate_path_from_node_to_root_using_function(&map, &Path::empty());
        assert!(success);
        assert!(result.is_empty());
    }

    #[test]
    fn test_translate_identity() {
        let map = MapFunction::identity().clone();
        let path = Path::from_string("/World/Prim").unwrap();

        let (result, success) = translate_path_from_node_to_root_using_function(&map, &path);
        assert!(success);
        assert_eq!(result, path);
    }

    #[test]
    fn test_translate_with_invalid_node() {
        let invalid_node = NodeRef::invalid();
        let path = Path::from_string("/World").unwrap();

        let (result, success) = translate_path_from_node_to_root(&invalid_node, &path);
        assert!(!success);
        assert!(result.is_empty());
    }

    #[test]
    fn test_translate_relative_path_fails() {
        let map = MapFunction::identity().clone();
        let path = Path::from_string("Relative/Path").unwrap_or(Path::empty());

        // Relative paths should fail (but Path::from_string may not allow them)
        if !path.is_empty() && !path.is_absolute_path() {
            let (_, success) = translate_path_from_node_to_root_using_function(&map, &path);
            assert!(!success);
        }
    }
}
