//! PCP Property Index - composition of property opinions.
//!
//! PcpPropertyIndex is an index of all sites in scene description that
//! contribute opinions to a specific property, under composition semantics.
//!
//! # C++ Parity
//!
//! This is a port of `pxr/usd/pcp/propertyIndex.h` and `propertyIndex.cpp`.

use super::iterator::PropertyIterator;
use crate::{ErrorType, NodeRef, PrimIndex};
use usd_sdf::Path;

// ============================================================================
// Property Info
// ============================================================================

/// Information about a property in the property stack.
///
/// Contains the property spec and the node that contributed it.
#[derive(Clone, Debug)]
pub struct PropertyInfo {
    /// The path to the property spec.
    pub property_path: Path,

    /// The node that originated this property opinion.
    pub originating_node: NodeRef,
}

impl PropertyInfo {
    /// Creates a new property info.
    pub fn new(property_path: Path, originating_node: NodeRef) -> Self {
        Self {
            property_path,
            originating_node,
        }
    }
}

// ============================================================================
// Property Index
// ============================================================================

/// Index of all sites that contribute opinions to a property.
///
/// PcpPropertyIndex is an index of all sites in scene description that
/// contribute opinions to a specific property, under composition semantics.
/// The property stack is ordered from strongest to weakest opinion.
#[derive(Clone, Debug, Default)]
pub struct PropertyIndex {
    /// The property stack (strong to weak order).
    property_stack: Vec<PropertyInfo>,

    /// Errors encountered during computation.
    local_errors: Vec<ErrorType>,
}

impl PropertyIndex {
    /// Creates an empty property index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Swaps contents with another property index.
    pub fn swap(&mut self, other: &mut PropertyIndex) {
        std::mem::swap(&mut self.property_stack, &mut other.property_stack);
        std::mem::swap(&mut self.local_errors, &mut other.local_errors);
    }

    /// Returns true if this property index contains no opinions.
    pub fn is_empty(&self) -> bool {
        self.property_stack.is_empty()
    }

    /// Returns the number of property specs in the stack.
    pub fn len(&self) -> usize {
        self.property_stack.len()
    }

    /// Returns the property stack (strong to weak).
    pub fn property_stack(&self) -> &[PropertyInfo] {
        &self.property_stack
    }

    /// Returns the list of local errors.
    pub fn local_errors(&self) -> &[ErrorType] {
        &self.local_errors
    }

    /// Returns the number of local specs.
    ///
    /// Local specs are those from nodes that are not due to ancestors.
    pub fn num_local_specs(&self) -> usize {
        self.property_stack
            .iter()
            .filter(|info| {
                info.originating_node.is_valid() && !info.originating_node.is_due_to_ancestor()
            })
            .count()
    }

    /// Returns an iterator over all property infos.
    pub fn iter(&self) -> impl Iterator<Item = &PropertyInfo> {
        self.property_stack.iter()
    }

    /// Returns an iterator over local property infos only.
    pub fn iter_local(&self) -> impl Iterator<Item = &PropertyInfo> {
        self.property_stack.iter().filter(|info| {
            info.originating_node.is_valid() && !info.originating_node.is_due_to_ancestor()
        })
    }

    /// Returns a range of iterators that encompasses properties in this index's property stack.
    ///
    /// By default, this returns a range encompassing all properties in the index.
    /// If `local_only` is specified, the range will only include properties from
    /// local nodes in its owning prim's graph.
    ///
    /// Matches C++ `GetPropertyRange(bool localOnly = false)`.
    pub fn get_property_range(
        &self,
        local_only: bool,
    ) -> (PropertyIterator<'_>, PropertyIterator<'_>) {
        if local_only {
            let local_start = self
                .property_stack
                .iter()
                .position(|info| {
                    info.originating_node.is_valid() && !info.originating_node.is_due_to_ancestor()
                })
                .unwrap_or(self.property_stack.len());
            let start_iter = PropertyIterator::new(self, local_start);
            let end_iter = PropertyIterator::new(self, self.property_stack.len());
            (start_iter, end_iter)
        } else {
            let start_iter = PropertyIterator::new(self, 0);
            let end_iter = PropertyIterator::new(self, self.property_stack.len());
            (start_iter, end_iter)
        }
    }

    /// Adds a property info to the stack.
    pub(crate) fn push(&mut self, info: PropertyInfo) {
        self.property_stack.push(info);
    }

    /// Adds an error.
    #[allow(dead_code)] // Internal API - used during property indexing
    pub(crate) fn add_error(&mut self, error: ErrorType) {
        self.local_errors.push(error);
    }
}

// ============================================================================
// Property Index Builder
// ============================================================================

/// Builds a property index for a given property path.
///
/// This walks the prim index and collects all property specs at the
/// given property path, ordered from strongest to weakest.
///
/// # Arguments
///
/// * `property_path` - The path to the property
/// * `prim_index` - The owning prim's index
///
/// # Returns
///
/// A PropertyIndex containing all opinions for this property.
pub fn build_prim_property_index(
    property_path: &Path,
    prim_index: &PrimIndex,
) -> (PropertyIndex, Vec<ErrorType>) {
    let mut index = PropertyIndex::new();
    let errors = Vec::new();

    // Validate the property path
    if property_path.is_empty() || !property_path.is_property_path() {
        return (index, errors);
    }

    // Get the property name
    let prop_name = property_path.get_name();
    if prop_name.is_empty() {
        return (index, errors);
    }

    // Walk the prim index nodes in strength order
    // Start from root and traverse children recursively
    let root = prim_index.root_node();
    if root.is_valid() {
        collect_property_specs(&root, prop_name, &mut index);
    }

    (index, errors)
}

/// Recursively collects property specs from a node and its children.
fn collect_property_specs(node: &NodeRef, prop_name: &str, index: &mut PropertyIndex) {
    // Skip invalid nodes
    if !node.is_valid() {
        return;
    }

    // Add property info if node has specs and can contribute
    if node.has_specs() && !node.is_inert() && !node.is_culled() {
        let node_path = node.path();
        if let Some(local_prop_path) = node_path.append_property(prop_name) {
            let info = PropertyInfo::new(local_prop_path, node.clone());
            index.push(info);
        }
    }

    // Recurse to children
    for child in node.children() {
        collect_property_specs(&child, prop_name, index);
    }
}

/// Builds a property index, computing the prim index if needed.
///
/// This is a higher-level function that can compute the owning
/// prim index if it's not already cached.
pub fn build_property_index(
    property_path: &Path,
    prim_index: &PrimIndex,
) -> (PropertyIndex, Vec<ErrorType>) {
    // For now, delegate to build_prim_property_index
    // In full implementation, this would check cache and compute if needed
    build_prim_property_index(property_path, prim_index)
}

/// Builds a property index using a PcpCache to auto-compute the owning prim index.
///
/// Matches C++ `PcpBuildPropertyIndex(cache, propertyPath, ...)` overload.
/// Extracts the prim path from the property path, looks up (or computes) the
/// prim index in the cache, then delegates to `build_prim_property_index`.
pub fn build_property_index_from_cache(
    property_path: &Path,
    cache: &crate::Cache,
) -> (PropertyIndex, Vec<ErrorType>) {
    if property_path.is_empty() || !property_path.is_property_path() {
        return (PropertyIndex::new(), Vec::new());
    }

    // Get the owning prim path
    let prim_path = property_path.get_prim_path();
    if prim_path.is_empty() {
        return (PropertyIndex::new(), Vec::new());
    }

    // Compute (or find) the prim index in the cache
    let (prim_index, errors) = cache.compute_prim_index(&prim_path);

    let (index, mut prop_errors) = build_prim_property_index(property_path, &prim_index);
    prop_errors.extend(errors);

    (index, prop_errors)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_property_index() {
        let index = PropertyIndex::new();
        assert!(index.is_empty());
        assert_eq!(index.len(), 0);
        assert_eq!(index.num_local_specs(), 0);
    }

    #[test]
    fn test_property_index_swap() {
        let mut index1 = PropertyIndex::new();
        let mut index2 = PropertyIndex::new();

        let path = Path::from_string("/World.attr").unwrap();
        index1.push(PropertyInfo::new(path, NodeRef::invalid()));

        assert!(!index1.is_empty());
        assert!(index2.is_empty());

        index1.swap(&mut index2);

        assert!(index1.is_empty());
        assert!(!index2.is_empty());
    }

    #[test]
    fn test_property_index_errors() {
        let mut index = PropertyIndex::new();
        assert!(index.local_errors().is_empty());

        index.add_error(ErrorType::ArcCycle);
        assert_eq!(index.local_errors().len(), 1);
    }

    #[test]
    fn test_build_property_index_from_cache_empty_path() {
        let cache = crate::Cache::new(crate::LayerStackIdentifier::default(), true);
        let (index, errors) = build_property_index_from_cache(&Path::empty(), &cache);
        assert!(index.is_empty());
        assert!(errors.is_empty());
    }

    #[test]
    fn test_build_property_index_from_cache_non_property_path() {
        let cache = crate::Cache::new(crate::LayerStackIdentifier::default(), true);
        let prim_path = Path::from_string("/World").unwrap();
        // Prim path is not a property path
        let (index, _errors) = build_property_index_from_cache(&prim_path, &cache);
        assert!(index.is_empty());
    }
}
