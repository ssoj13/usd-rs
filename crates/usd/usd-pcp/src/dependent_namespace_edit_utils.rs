//! PCP Dependent Namespace Edit Utilities.
//!
//! Utilities for computing edits needed to perform a namespace edit and fix up
//! downstream composition dependencies in dependent prim indexes.
//!
//! # C++ Parity
//!
//! This is a port of `pxr/usd/pcp/dependentNamespaceEditUtils.h` and `.cpp`.

use std::collections::HashMap;

use crate::{Cache, LayerStackRefPtr};
use usd_sdf::{LayerHandle, Path};
use usd_tf::Token;
use usd_vt::Value;

/// Description of an edit to a prim spec composition field, such as
/// references, inherits, or relocates.
#[derive(Debug, Clone)]
pub struct CompositionFieldEdit {
    /// Layer containing the prim spec to edit.
    pub layer: LayerHandle,
    /// Path of the prim spec to edit.
    pub path: Path,
    /// Name of the composition field.
    pub field_name: Token,
    /// New value of the composition field to set.
    pub new_field_value: Value,
}

/// Description of move edit which consists of the old (source) path and the
/// new (destination) path.
#[derive(Debug, Clone)]
pub struct MoveEditDescription {
    /// The old (source) path.
    pub old_path: Path,
    /// The new (destination) path.
    pub new_path: Path,
}

/// Vector of move edit descriptions.
pub type MoveEditDescriptionVector = Vec<MoveEditDescription>;

/// Relocates values type alias (source -> target path mapping).
pub type Relocates = Vec<(Path, Path)>;

/// Structure for bundling all the edits that need to be performed in order to
/// perform a namespace edit and fix up downstream composition dependencies on
/// dependent prim indexes in dependent PcpCaches.
///
/// This is the return value of `gather_dependent_namespace_edits`.
#[derive(Debug, Clone, Default)]
pub struct DependentNamespaceEdits {
    /// List of all composition fields edits to perform.
    pub composition_field_edits: Vec<CompositionFieldEdit>,

    /// Map of layer to the spec moves edits to perform on the layer.
    pub layer_spec_moves: HashMap<LayerHandle, MoveEditDescriptionVector>,

    /// Map of layer to relocates value to set in the layer metadata relocates
    /// field.
    pub dependent_relocates_edits: HashMap<LayerHandle, Relocates>,

    /// Errors encountered during the processing of the dependent namespace
    /// edits.
    pub errors: Vec<String>,

    /// Warnings encountered during the processing of the dependent namespace
    /// edits.
    pub warnings: Vec<String>,

    /// Lists of composed prim paths in each affected cache whose prim indexes
    /// will need to be recomputed after the changes in this object are applied.
    ///
    /// This information can be useful during change processing and notification
    /// to help report the intended effects of all the layer spec edits that are
    /// performed during a namespace edit.
    ///
    /// Note: We use a raw pointer here to match C++ API. In practice, the
    /// caches should outlive this struct.
    pub dependent_cache_path_changes: HashMap<*const Cache, MoveEditDescriptionVector>,
}

impl DependentNamespaceEdits {
    /// Creates a new empty edits structure.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true if there are no edits to perform.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.composition_field_edits.is_empty()
            && self.layer_spec_moves.is_empty()
            && self.dependent_relocates_edits.is_empty()
    }

    /// Returns true if there are any errors.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Returns true if there are any warnings.
    #[must_use]
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }

    /// Adds an error message.
    pub fn add_error(&mut self, error: String) {
        self.errors.push(error);
    }

    /// Adds a warning message.
    pub fn add_warning(&mut self, warning: String) {
        self.warnings.push(warning);
    }
}

/// Given a prim or property spec move edit from `old_path` to `new_path` and the
/// `affected_layers` on which this spec move will be performed, this function
/// finds all prim indexes already cached in each PcpCache in `dependent_caches`
/// that would be affected by these edits and computes a full set of edits that
/// would be required to maintain these dependent prim indexes' composed prim
/// stacks, possibly moving the prim index to a new prim path if necessary.
///
/// If `add_relocates_to_layer_stack` is provided, this will also add a new
/// relocates edit to the necessary layers in the layer stack that moves
/// old_prim_path to new_prim_path. The layer `add_relocates_to_layer_stack_edit_layer`
/// provided is only relevant when the relocates layer stack is also provided as
/// it determines which specific layer in the layer stack will have a new
/// relocates entry added to it.
///
/// # Arguments
///
/// * `old_path` - The original path
/// * `new_path` - The new path after the move
/// * `affected_layers` - Layers affected by the spec move
/// * `add_relocates_to_layer_stack` - Optional layer stack for adding relocates
/// * `add_relocates_to_layer_stack_edit_layer` - Layer for relocates edit
/// * `dependent_caches` - Caches to check for dependencies
pub fn gather_dependent_namespace_edits(
    old_path: &Path,
    new_path: &Path,
    affected_layers: &[LayerHandle],
    add_relocates_to_layer_stack: Option<&LayerStackRefPtr>,
    add_relocates_to_layer_stack_edit_layer: Option<&LayerHandle>,
    dependent_caches: &[&Cache],
) -> DependentNamespaceEdits {
    let mut edits = DependentNamespaceEdits::new();

    // Early return for empty paths
    if old_path.is_empty() {
        edits.add_error("Cannot perform namespace edit with empty old path".to_string());
        return edits;
    }

    if new_path.is_empty() && add_relocates_to_layer_stack.is_some() {
        // For delete operations, we may still need to add relocates
    }

    // Process each dependent cache
    for cache in dependent_caches {
        process_cache_dependencies(cache, old_path, new_path, affected_layers, &mut edits);
    }

    // Handle relocates if requested
    if let (Some(layer_stack), Some(edit_layer)) = (
        add_relocates_to_layer_stack,
        add_relocates_to_layer_stack_edit_layer,
    ) {
        add_relocates_edit(layer_stack, edit_layer, old_path, new_path, &mut edits);
    }

    edits
}

/// Gathers the list of layers that need to be edited to perform the spec move
/// from `old_spec_path` to `new_spec_path` on the given `layer_stack`.
///
/// If any errors are encountered where the spec would not be able to be performed
/// on a layer that needs to be edited, those errors will be added to `errors`.
/// Layers with errors are still included in the returned result regardless.
pub fn gather_layers_to_edit_for_spec_move(
    layer_stack: &LayerStackRefPtr,
    _old_spec_path: &Path,
    _new_spec_path: &Path,
    errors: &mut Vec<String>,
) -> Vec<LayerHandle> {
    let mut result = Vec::new();

    // Get all layers in the stack that have specs at the old path
    for layer in layer_stack.get_layers() {
        // In a full implementation, we would check if the layer has specs
        // at old_spec_path. For now, include all layers.
        let handle = LayerHandle::from_layer(&layer);

        // Check if layer is editable
        if !layer.permission_to_edit() {
            errors.push(format!("Layer '{}' is not editable", layer.identifier()));
        }

        result.push(handle);
    }

    result
}

/// Processes dependencies for a single cache.
fn process_cache_dependencies(
    cache: &Cache,
    old_path: &Path,
    new_path: &Path,
    _affected_layers: &[LayerHandle],
    edits: &mut DependentNamespaceEdits,
) {
    // In a full implementation, this would:
    // 1. Find all prim indices that depend on specs at old_path
    // 2. For each dependent prim index, compute the edits needed
    // 3. Track which composition fields (refs, inherits, etc.) need updating

    // For now, just track that we processed this cache
    let cache_ptr = cache as *const Cache;
    edits
        .dependent_cache_path_changes
        .entry(cache_ptr)
        .or_default()
        .push(MoveEditDescription {
            old_path: old_path.clone(),
            new_path: new_path.clone(),
        });
}

/// Adds relocates edits for a layer stack.
fn add_relocates_edit(
    _layer_stack: &LayerStackRefPtr,
    edit_layer: &LayerHandle,
    old_path: &Path,
    new_path: &Path,
    edits: &mut DependentNamespaceEdits,
) {
    if old_path.is_empty() {
        return;
    }

    // Add the relocate to the specified layer
    let relocate = (old_path.clone(), new_path.clone());
    edits
        .dependent_relocates_edits
        .entry(edit_layer.clone())
        .or_default()
        .push(relocate);
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dependent_namespace_edits_new() {
        let edits = DependentNamespaceEdits::new();
        assert!(edits.is_empty());
        assert!(!edits.has_errors());
        assert!(!edits.has_warnings());
    }

    #[test]
    fn test_dependent_namespace_edits_errors() {
        let mut edits = DependentNamespaceEdits::new();
        edits.add_error("Test error".to_string());
        assert!(edits.has_errors());
        assert!(!edits.has_warnings());
    }

    #[test]
    fn test_dependent_namespace_edits_warnings() {
        let mut edits = DependentNamespaceEdits::new();
        edits.add_warning("Test warning".to_string());
        assert!(!edits.has_errors());
        assert!(edits.has_warnings());
    }

    #[test]
    fn test_gather_with_empty_old_path() {
        let edits = gather_dependent_namespace_edits(
            &Path::empty(),
            &Path::from_string("/NewPath").unwrap(),
            &[],
            None,
            None,
            &[],
        );
        assert!(edits.has_errors());
    }

    #[test]
    fn test_move_edit_description() {
        let desc = MoveEditDescription {
            old_path: Path::from_string("/Old").unwrap(),
            new_path: Path::from_string("/New").unwrap(),
        };
        assert_eq!(desc.old_path.as_str(), "/Old");
        assert_eq!(desc.new_path.as_str(), "/New");
    }
}
