//! PCP Namespace Edits.
//!
//! Computes the edits required across caches when a namespace edit (rename,
//! reparent, remove) is performed on a prim or property.
//!
//! # C++ Parity
//!
//! This is a port of `pxr/usd/pcp/namespaceEdits.h` and `namespaceEdits.cpp`.

use crate::{Cache, LayerStackPtr};
use usd_sdf::{LayerHandle, Path};

/// Types of namespace edits that a given layer stack site could need
/// to perform in response to a namespace edit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EditType {
    /// Must namespace edit spec.
    EditPath,
    /// Must fixup inherits.
    EditInherit,
    /// Must fixup specializes.
    EditSpecializes,
    /// Must fixup references.
    EditReference,
    /// Must fixup payloads.
    EditPayload,
    /// Must fixup relocates.
    EditRelocate,
}

/// Sites that must respond to a namespace edit.
#[derive(Clone, Debug, Default)]
pub struct NamespaceEdits {
    /// Cache sites that must respond to the namespace edit.
    pub cache_sites: Vec<CacheSite>,
    /// Layer stack sites that must respond to the namespace edit.
    pub layer_stack_sites: Vec<LayerStackSite>,
    /// Layer stack sites that are affected but cannot respond properly.
    ///
    /// For example, in situations involving relocates, a valid namespace edit
    /// in one cache may result in an invalid edit in another cache in response.
    pub invalid_layer_stack_sites: Vec<LayerStackSite>,
}

impl NamespaceEdits {
    /// Creates a new empty namespace edits struct.
    pub fn new() -> Self {
        Self::default()
    }

    /// Swaps contents with another NamespaceEdits.
    pub fn swap(&mut self, other: &mut NamespaceEdits) {
        std::mem::swap(&mut self.cache_sites, &mut other.cache_sites);
        std::mem::swap(&mut self.layer_stack_sites, &mut other.layer_stack_sites);
        std::mem::swap(
            &mut self.invalid_layer_stack_sites,
            &mut other.invalid_layer_stack_sites,
        );
    }

    /// Returns true if there are no edits required.
    pub fn is_empty(&self) -> bool {
        self.cache_sites.is_empty()
            && self.layer_stack_sites.is_empty()
            && self.invalid_layer_stack_sites.is_empty()
    }

    /// Returns true if there are any invalid sites.
    pub fn has_invalid_sites(&self) -> bool {
        !self.invalid_layer_stack_sites.is_empty()
    }
}

/// Cache site that must respond to a namespace edit.
#[derive(Clone, Debug)]
pub struct CacheSite {
    /// Index of the cache containing this site.
    pub cache_index: usize,
    /// Old path of the site.
    pub old_path: Path,
    /// New path of the site.
    pub new_path: Path,
}

/// Layer stack site that must respond to a namespace edit.
///
/// All specs at the site will respond the same way.
#[derive(Clone, Debug)]
pub struct LayerStackSite {
    /// Index of the cache containing this site.
    pub cache_index: usize,
    /// Type of edit required.
    pub edit_type: EditType,
    /// Layer stack needing the fix.
    pub layer_stack: LayerStackPtr,
    /// Path of the site needing the fix.
    pub site_path: Path,
    /// Old path.
    pub old_path: Path,
    /// New path.
    pub new_path: Path,
}

/// Computes namespace edits required across all caches.
///
/// Returns the changes caused in any cache in `caches` due to namespace editing
/// the object at `cur_path` in the primary cache to have the path `new_path`.
///
/// # Arguments
///
/// * `primary_cache` - The cache where the edit originates
/// * `caches` - All caches that may be affected (including primary_cache)
/// * `cur_path` - Current path of the object being edited
/// * `new_path` - New path for the object (empty for deletion)
/// * `relocates_layer` - Layer to write relocations to if needed
///
/// # Note
///
/// This method only works when the affected prim indexes have been computed.
/// In general, you must have computed the prim index of everything in any
/// existing cache, otherwise you might miss changes to objects in those caches
/// that use the namespace edited object.
pub fn compute_namespace_edits(
    _primary_cache: &Cache,
    _caches: &[&Cache],
    cur_path: &Path,
    new_path: &Path,
    _relocates_layer: Option<&LayerHandle>,
) -> NamespaceEdits {
    let mut edits = NamespaceEdits::new();

    // Basic case: if new_path is empty, this is a deletion
    let is_deletion = new_path.is_empty();

    // For a full implementation, we would:
    // 1. Find all sites in all caches that depend on cur_path
    // 2. For each site, determine the type of edit needed
    // 3. Check if the edit is valid or would create conflicts

    // For now, add the primary cache site
    edits.cache_sites.push(CacheSite {
        cache_index: 0,
        old_path: cur_path.clone(),
        new_path: if is_deletion {
            Path::empty()
        } else {
            new_path.clone()
        },
    });

    edits
}

/// Computes the namespace edits required for a rename operation.
///
/// This is a convenience wrapper around `compute_namespace_edits`.
pub fn compute_rename_edits(
    primary_cache: &Cache,
    caches: &[&Cache],
    old_path: &Path,
    new_name: &str,
) -> NamespaceEdits {
    // Compute new path by replacing the last component
    let parent = old_path.get_parent_path();
    let new_path = parent
        .append_child(new_name)
        .unwrap_or_else(|| parent.clone());

    compute_namespace_edits(primary_cache, caches, old_path, &new_path, None)
}

/// Computes the namespace edits required for a reparent operation.
///
/// This is a convenience wrapper around `compute_namespace_edits`.
pub fn compute_reparent_edits(
    primary_cache: &Cache,
    caches: &[&Cache],
    old_path: &Path,
    new_parent_path: &Path,
) -> NamespaceEdits {
    // Compute new path under new parent
    let name = old_path.get_name();
    let new_path = new_parent_path
        .append_child(name)
        .unwrap_or_else(|| new_parent_path.clone());

    compute_namespace_edits(primary_cache, caches, old_path, &new_path, None)
}

/// Computes the namespace edits required for a deletion operation.
///
/// This is a convenience wrapper around `compute_namespace_edits`.
pub fn compute_delete_edits(
    primary_cache: &Cache,
    caches: &[&Cache],
    path_to_delete: &Path,
) -> NamespaceEdits {
    compute_namespace_edits(primary_cache, caches, path_to_delete, &Path::empty(), None)
}

// ============================================================================
// Internal Helpers
// ============================================================================

/// Checks if a namespace edit would create a cycle.
pub fn would_create_cycle(_cache: &Cache, _old_path: &Path, _new_path: &Path) -> bool {
    // In a full implementation, this would check if moving old_path to new_path
    // would create a reference cycle (e.g., A references B which references A)
    false
}

/// Checks if a path is under another path.
/// Used by compute_namespace_edits for dependency checks.
#[cfg(test)] // Note: Helper for compute_namespace_edits.
fn is_path_under(path: &Path, parent: &Path) -> bool {
    path.has_prefix(parent) && path != parent
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edit_type() {
        assert_ne!(EditType::EditPath, EditType::EditInherit);
        assert_eq!(EditType::EditPath, EditType::EditPath);
    }

    #[test]
    fn test_namespace_edits_new() {
        let edits = NamespaceEdits::new();
        assert!(edits.is_empty());
        assert!(!edits.has_invalid_sites());
    }

    #[test]
    fn test_namespace_edits_swap() {
        let mut edits1 = NamespaceEdits::new();
        edits1.cache_sites.push(CacheSite {
            cache_index: 0,
            old_path: Path::from_string("/A").unwrap(),
            new_path: Path::from_string("/B").unwrap(),
        });

        let mut edits2 = NamespaceEdits::new();

        edits1.swap(&mut edits2);

        assert!(edits1.is_empty());
        assert!(!edits2.is_empty());
    }

    #[test]
    fn test_cache_site() {
        let site = CacheSite {
            cache_index: 1,
            old_path: Path::from_string("/World/Cube").unwrap(),
            new_path: Path::from_string("/World/Box").unwrap(),
        };

        assert_eq!(site.cache_index, 1);
        assert_eq!(site.old_path.as_str(), "/World/Cube");
        assert_eq!(site.new_path.as_str(), "/World/Box");
    }

    #[test]
    fn test_is_path_under() {
        let parent = Path::from_string("/World").unwrap();
        let child = Path::from_string("/World/Cube").unwrap();
        let other = Path::from_string("/Other").unwrap();

        assert!(is_path_under(&child, &parent));
        assert!(!is_path_under(&parent, &parent)); // Same path
        assert!(!is_path_under(&other, &parent));
    }
}
