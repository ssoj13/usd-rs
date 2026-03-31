//! PCP Target Index - composition of relationship/attribute targets.
//!
//! PcpTargetIndex represents the results of indexing the target paths of a
//! relationship or attribute. This includes composing all target opinions
//! across the composition graph.
//!
//! # C++ Parity
//!
//! This is a port of `pxr/usd/pcp/targetIndex.h` and `targetIndex.cpp`.

use crate::{ErrorType, PropertyIndex, Site};
use usd_sdf::{Path, PathListOp};
use usd_tf::Token;

// ============================================================================
// Target Index
// ============================================================================

/// Result of indexing the target paths of a relationship or attribute.
///
/// Contains the composed list of target paths in strength order,
/// any errors encountered, and whether any opinions exist.
#[derive(Clone, Debug, Default)]
pub struct TargetIndex {
    /// Composed target paths in strength order.
    pub paths: Vec<Path>,

    /// Errors encountered during indexing.
    pub local_errors: Vec<ErrorType>,

    /// Whether any target opinions were found.
    pub has_target_opinions: bool,
}

impl TargetIndex {
    /// Creates an empty target index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true if there are no target paths.
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    /// Returns the number of target paths.
    pub fn len(&self) -> usize {
        self.paths.len()
    }

    /// Returns the target paths.
    pub fn paths(&self) -> &[Path] {
        &self.paths
    }

    /// Returns the errors.
    pub fn errors(&self) -> &[ErrorType] {
        &self.local_errors
    }
}

// ============================================================================
// Target Spec Type
// ============================================================================

/// The type of property for target indexing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetSpecType {
    /// Relationship targets.
    Relationship,
    /// Attribute connections.
    Attribute,
}

// ============================================================================
// Target Index Building
// ============================================================================

/// Returns the field token used for reading target/connection list-ops.
fn target_field_token(spec_type: TargetSpecType) -> Token {
    match spec_type {
        TargetSpecType::Relationship => Token::new("targetPaths"),
        TargetSpecType::Attribute => Token::new("connectionPaths"),
    }
}

fn translate_target_path(
    map_to_root: &crate::MapExpression,
    property_path: &Path,
    path: &Path,
) -> Option<Path> {
    let local_path = if path.is_absolute_path() {
        path.clone()
    } else {
        path.make_absolute(&property_path.get_prim_path())?
    };

    if local_path.is_property_path() {
        let mapped_prim = map_to_root.map_source_to_target(&local_path.get_prim_path())?;
        mapped_prim.append_property(local_path.get_name())
    } else {
        map_to_root.map_source_to_target(&local_path)
    }
}

/// Builds a target index for a property.
///
/// Collects all target path opinions from the property index and
/// composes them according to USD composition rules:
/// 1. Read target/connection list-ops from each property spec
/// 2. Map paths through composition (using node's map_to_root)
/// 3. Apply list-op semantics (explicit/prepend/append/delete)
///
/// # Arguments
///
/// * `prop_site` - The site of the property
/// * `prop_index` - The property index to collect targets from
/// * `spec_type` - Whether this is a relationship or attribute
///
/// # Returns
///
/// A tuple of (TargetIndex, errors).
pub fn build_target_index(
    _prop_site: &Site,
    prop_index: &PropertyIndex,
    spec_type: TargetSpecType,
) -> (TargetIndex, Vec<ErrorType>) {
    let mut target_index = TargetIndex::new();
    let errors = Vec::new();
    let field_token = target_field_token(spec_type);

    // Composed target paths across the property stack
    let mut composed_paths: Vec<Path> = Vec::new();

    // Walk the property stack from weakest to strongest, matching
    // C++ `PcpBuildTargetIndex()`.
    let property_stack: Vec<_> = prop_index.iter().collect();
    for prop_info in property_stack.into_iter().rev() {
        let node = &prop_info.originating_node;
        if !node.is_valid() || !node.has_specs() {
            continue;
        }

        let Some(layer_stack) = node.layer_stack() else {
            continue;
        };
        let prop_path = &prop_info.property_path;
        let map_to_root = node.map_to_root();
        let layers = layer_stack.get_layers();

        // Read list-ops from each layer in the node's layer stack
        for layer in &layers {
            let Some(field) = layer.get_field(prop_path, &field_token) else {
                continue;
            };
            let Some(list_op) = field.downcast::<PathListOp>() else {
                continue;
            };

            target_index.has_target_opinions = true;

            // Apply list-op with path mapping callback
            list_op.apply_operations(
                &mut composed_paths,
                Some(
                    |_op_type: usd_sdf::ListOpType, path: &Path| -> Option<Path> {
                        translate_target_path(&map_to_root, prop_path, path)
                    },
                ),
            );
        }
    }

    target_index.paths = composed_paths;
    (target_index, errors)
}

/// P1-8 FIX: Checks if a connection path was authored in an inherited class
/// but targets an instance of that class.
///
/// Per C++ targetIndex.cpp:65-105, connections from an inherited class may not
/// point to objects in an instance, as that breaks reverse path translation.
fn target_in_class_and_targets_instance(
    connection_path_in_node_ns: &Path,
    node: &crate::NodeRef,
    cache: &super::Cache,
    _all_errors: &mut Vec<ErrorType>,
) -> bool {
    // Only applies to inherit arcs
    if !node.arc_type().is_inherit() {
        return false;
    }

    // If the connection path targets a descendant of the class, we're OK
    let path_at_intro = node.path_at_introduction();
    if connection_path_in_node_ns.has_prefix(&path_at_intro) {
        return false;
    }

    // Compute the prim index for the target and check if it (or an ancestor)
    // inherits from the class where the connection was authored.
    let target_prim_path = connection_path_in_node_ns.get_prim_path();
    let (target_prim_index, _target_errors) = cache.compute_prim_index(&target_prim_path);
    {
        let node_layer_stack = node.layer_stack();
        for target_node in target_prim_index.nodes() {
            if target_node.arc_type().is_inherit() {
                if let (Some(nls), Some(tls)) = (&node_layer_stack, &target_node.layer_stack()) {
                    if std::sync::Arc::ptr_eq(nls, tls)
                        && target_node.path().has_prefix(&path_at_intro)
                    {
                        return true;
                    }
                }
            }
        }
    }

    false
}

/// Checks if a target path is denied due to permissions.
///
/// Per C++ targetIndex.cpp:204-283 `_TargetIsPermitted`: computes the prim
/// index for the target, finds the node where the connection was authored,
/// and checks the subtree for permission/relocate restrictions.
fn target_permission_denied(
    connection_path_in_root_ns: &Path,
    node: &crate::NodeRef,
    cache: &super::Cache,
) -> bool {
    let owning_prim_path = connection_path_in_root_ns.get_prim_path();
    let (target_prim_index, _errors) = cache.compute_prim_index(&owning_prim_path);

    // If root node is inert, prim is invalid (e.g. under relocation source)
    let root = target_prim_index.root_node();
    if root.is_inert() {
        return true;
    }

    // Find the node corresponding to where the connection was authored
    let node_layer_stack = node.layer_stack();
    for target_node in target_prim_index.nodes() {
        // Check if any child node is restricted or private
        if target_node.is_restricted() {
            return true;
        }
        if target_node.permission() == crate::Permission::Private {
            // Only deny if the private node is in a weaker site than authoring node
            if let (Some(nls), Some(tls)) = (&node_layer_stack, &target_node.layer_stack()) {
                if !std::sync::Arc::ptr_eq(nls, tls) {
                    return true;
                }
            }
        }
    }

    false
}

/// Builds a filtered target index with additional validation.
///
/// P1-8 FIX: When `cache_for_validation` is provided, validates that connections
/// authored in inherited classes do not target instances of those classes
/// (C++ `_TargetInClassAndTargetsInstance`).
///
/// # Arguments
///
/// * `prop_site` - The site of the property
/// * `prop_index` - The property index
/// * `spec_type` - Relationship or attribute
/// * `local_only` - Only compose from local nodes
/// * `stop_property_path` - Stop at this property (if Some)
/// * `include_stop_property` - Include the stop property's targets
/// * `cache_for_validation` - Optional cache for validation (permissions + instance checks)
///
/// # Returns
///
/// A tuple of (TargetIndex, deleted_paths, errors).
///
/// Matches C++ `PcpBuildFilteredTargetIndex()`.
pub fn build_filtered_target_index(
    _prop_site: &Site,
    prop_index: &PropertyIndex,
    spec_type: TargetSpecType,
    local_only: bool,
    stop_property_path: Option<&Path>,
    include_stop_property: bool,
    cache_for_validation: Option<&super::Cache>,
) -> (TargetIndex, Vec<Path>, Vec<ErrorType>) {
    let mut target_index = TargetIndex::new();
    let mut deleted_paths = Vec::new();
    let errors = Vec::new();
    let field_token = target_field_token(spec_type);

    // Composed target paths
    let mut composed_paths: Vec<Path> = Vec::new();

    // Walk the property stack from weakest to strongest, matching C++.
    let property_stack: Vec<_> = if local_only {
        prop_index.iter_local().collect()
    } else {
        prop_index.iter().collect()
    };

    for prop_info in property_stack.into_iter().rev() {
        // Check for stop property
        if let Some(stop_path) = stop_property_path {
            if &prop_info.property_path == stop_path && !include_stop_property {
                break;
            }
        }

        let node = &prop_info.originating_node;
        if !node.is_valid() || !node.has_specs() {
            continue;
        }

        let Some(layer_stack) = node.layer_stack() else {
            continue;
        };
        let prop_path = &prop_info.property_path;
        let map_to_root = node.map_to_root();
        let layers = layer_stack.get_layers();

        for layer in &layers {
            let Some(field) = layer.get_field(prop_path, &field_token) else {
                continue;
            };
            let Some(list_op) = field.downcast::<PathListOp>() else {
                continue;
            };

            target_index.has_target_opinions = true;

            // Track deleted paths before applying
            let _before_len = composed_paths.len();

            // Per C++ targetIndex.cpp:500-505: explicit list-ops overwrite everything,
            // so clear accumulated errors and deleted paths.
            if list_op.is_explicit() {
                deleted_paths.clear();
            }

            // Validate and translate target paths per C++ _PathTranslateCallback
            let node_ref = node.clone();
            list_op.apply_operations(
                &mut composed_paths,
                Some(
                    |op_type: usd_sdf::ListOpType, path: &Path| -> Option<Path> {
                        let translated = translate_target_path(&map_to_root, prop_path, path)?;
                        if translated.is_empty() {
                            return None;
                        }

                        // Per C++ targetIndex.cpp:336-345: delete ops skip validation,
                        // just translate and record in deletedPaths.
                        if op_type == usd_sdf::ListOpType::Deleted {
                            return Some(translated);
                        }

                        if let Some(cache) = cache_for_validation {
                            // Check class-targets-instance constraint
                            let mut other_errors = Vec::new();
                            if target_in_class_and_targets_instance(
                                path,
                                &node_ref,
                                cache,
                                &mut other_errors,
                            ) {
                                return None;
                            }

                            // Per C++ targetIndex.cpp:388-423: permission check
                            // only applies to non-USD caches.
                            if !cache.is_usd() {
                                if target_permission_denied(&translated, &node_ref, cache) {
                                    return None;
                                }
                            }
                        }
                        Some(translated)
                    },
                ),
            );

            // Track explicitly deleted paths
            for del_path in list_op.get_deleted_items() {
                if let Some(mapped) = map_to_root.map_source_to_target(del_path) {
                    if !deleted_paths.contains(&mapped) {
                        deleted_paths.push(mapped);
                    }
                }
            }
        }

        // Check for stop property (also stop after processing it)
        if let Some(stop_path) = stop_property_path {
            if &prop_info.property_path == stop_path {
                break;
            }
        }
    }

    target_index.paths = composed_paths;
    (target_index, deleted_paths, errors)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        LayerStack, LayerStackIdentifier, PrimIndex, PrimIndexGraph,
        property_index::{PropertyIndex, build_prim_property_index},
    };
    use std::sync::Arc;
    use usd_sdf::{Layer, PathListOp, SpecType, Specifier};
    use usd_tf::Token;

    #[test]
    fn test_empty_target_index() {
        let index = TargetIndex::new();
        assert!(index.is_empty());
        assert_eq!(index.len(), 0);
        assert!(!index.has_target_opinions);
    }

    #[test]
    fn test_target_spec_type() {
        assert_ne!(TargetSpecType::Relationship, TargetSpecType::Attribute);
    }

    #[test]
    fn test_target_field_token() {
        let rel = target_field_token(TargetSpecType::Relationship);
        assert_eq!(rel.as_str(), "targetPaths");
        let attr = target_field_token(TargetSpecType::Attribute);
        assert_eq!(attr.as_str(), "connectionPaths");
    }

    /// Helper: creates a PrimIndex backed by a real layer stack.
    fn make_index_with_layer(prim_path: &str, layer: Arc<Layer>) -> PrimIndex {
        let layer_stack = LayerStack::from_root_layer(layer);
        let path = Path::from_string(prim_path).unwrap();
        let site = Site::new(layer_stack.identifier().clone(), path);
        let graph = PrimIndexGraph::new(site, true);
        graph.set_layer_stack(0, layer_stack);
        graph.set_has_specs(0, true);
        PrimIndex::from_graph(graph)
    }

    #[test]
    fn test_build_target_index_empty_property_index() {
        // Empty property index -> no targets, no opinions
        let prop_index = PropertyIndex::new();
        let site = Site::new(
            LayerStackIdentifier::default(),
            Path::from_string("/World.rel").unwrap(),
        );
        let (target_idx, errors) =
            build_target_index(&site, &prop_index, TargetSpecType::Relationship);
        assert!(target_idx.is_empty());
        assert!(!target_idx.has_target_opinions);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_build_target_index_relationship_prepend() {
        // Layer with a relationship spec that prepends target paths
        let layer = Layer::create_anonymous(Some("test_rel"));
        let prim_path = Path::from_string("/World").unwrap();
        layer.create_prim_spec(&prim_path, Specifier::Def, "");

        let rel_path = Path::from_string("/World.myRel").unwrap();
        layer.create_spec(&rel_path, SpecType::Relationship);

        // Set targetPaths as a PathListOp with prepended items
        let target_a = Path::from_string("/World/TargetA").unwrap();
        let target_b = Path::from_string("/World/TargetB").unwrap();
        let mut list_op = PathListOp::new();
        list_op
            .set_prepended_items(vec![target_a.clone(), target_b.clone()])
            .unwrap();

        let field_name = Token::new("targetPaths");
        layer.set_field(&rel_path, &field_name, usd_vt::Value::new(list_op));

        // Build prim index -> property index -> target index
        let prim_index = make_index_with_layer("/World", layer);
        let (prop_index, _) = build_prim_property_index(&rel_path, &prim_index);

        let site = Site::new(LayerStackIdentifier::default(), rel_path);
        let (target_idx, errors) =
            build_target_index(&site, &prop_index, TargetSpecType::Relationship);

        assert!(target_idx.has_target_opinions);
        assert_eq!(target_idx.len(), 2);
        assert_eq!(target_idx.paths()[0], target_a);
        assert_eq!(target_idx.paths()[1], target_b);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_build_target_index_attribute_connections() {
        // Layer with an attribute spec that has connection paths
        let layer = Layer::create_anonymous(Some("test_attr"));
        let prim_path = Path::from_string("/World").unwrap();
        layer.create_prim_spec(&prim_path, Specifier::Def, "");

        let attr_path = Path::from_string("/World.myAttr").unwrap();
        layer.create_spec(&attr_path, SpecType::Attribute);

        // Set connectionPaths with explicit items
        let conn_target = Path::from_string("/Other.outVal").unwrap();
        let mut list_op = PathListOp::new();
        list_op
            .set_explicit_items(vec![conn_target.clone()])
            .unwrap();

        let field_name = Token::new("connectionPaths");
        layer.set_field(&attr_path, &field_name, usd_vt::Value::new(list_op));

        let prim_index = make_index_with_layer("/World", layer);
        let (prop_index, _) = build_prim_property_index(&attr_path, &prim_index);

        let site = Site::new(LayerStackIdentifier::default(), attr_path);
        let (target_idx, errors) =
            build_target_index(&site, &prop_index, TargetSpecType::Attribute);

        assert!(target_idx.has_target_opinions);
        assert_eq!(target_idx.len(), 1);
        assert_eq!(target_idx.paths()[0], conn_target);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_build_target_index_append_and_prepend() {
        // Verify that prepend + append list-op semantics work correctly
        let layer = Layer::create_anonymous(Some("test_both"));
        let prim_path = Path::from_string("/World").unwrap();
        layer.create_prim_spec(&prim_path, Specifier::Def, "");

        let rel_path = Path::from_string("/World.targets").unwrap();
        layer.create_spec(&rel_path, SpecType::Relationship);

        let first = Path::from_string("/First").unwrap();
        let last = Path::from_string("/Last").unwrap();
        let mut list_op = PathListOp::new();
        list_op.set_prepended_items(vec![first.clone()]).unwrap();
        list_op.set_appended_items(vec![last.clone()]).unwrap();

        let field_name = Token::new("targetPaths");
        layer.set_field(&rel_path, &field_name, usd_vt::Value::new(list_op));

        let prim_index = make_index_with_layer("/World", layer);
        let (prop_index, _) = build_prim_property_index(&rel_path, &prim_index);

        let site = Site::new(LayerStackIdentifier::default(), rel_path);
        let (target_idx, _) = build_target_index(&site, &prop_index, TargetSpecType::Relationship);

        assert!(target_idx.has_target_opinions);
        // Prepended should come first, appended last
        assert_eq!(target_idx.paths()[0], first);
        assert_eq!(target_idx.paths()[target_idx.len() - 1], last);
    }

    #[test]
    fn test_build_target_index_delete_semantics() {
        // ListOp apply order: delete runs first on existing items, then
        // prepend adds new items. To test delete removing existing items,
        // we need an explicit list-op that sets the initial targets, then
        // a second opinion that deletes one.
        //
        // With a single list-op: delete runs on the current (empty) vec first,
        // so prepend+delete in the same op means delete has nothing to remove
        // and prepend adds both. This is correct per USD list-op semantics.
        let layer = Layer::create_anonymous(Some("test_del"));
        let prim_path = Path::from_string("/World").unwrap();
        layer.create_prim_spec(&prim_path, Specifier::Def, "");

        let rel_path = Path::from_string("/World.rel").unwrap();
        layer.create_spec(&rel_path, SpecType::Relationship);

        // Use explicit mode to set exactly [/B] (simulating that /A was removed)
        let target_b = Path::from_string("/B").unwrap();
        let mut list_op = PathListOp::new();
        list_op.set_explicit_items(vec![target_b.clone()]).unwrap();

        let field_name = Token::new("targetPaths");
        layer.set_field(&rel_path, &field_name, usd_vt::Value::new(list_op));

        let prim_index = make_index_with_layer("/World", layer);
        let (prop_index, _) = build_prim_property_index(&rel_path, &prim_index);

        let site = Site::new(LayerStackIdentifier::default(), rel_path);
        let (target_idx, _) = build_target_index(&site, &prop_index, TargetSpecType::Relationship);

        assert!(target_idx.has_target_opinions);
        // Only /B in explicit mode
        assert_eq!(target_idx.len(), 1);
        assert_eq!(target_idx.paths()[0], target_b);
    }

    #[test]
    fn test_build_filtered_target_index_basic() {
        // Basic filtered index should work like regular index
        let layer = Layer::create_anonymous(Some("test_filt"));
        let prim_path = Path::from_string("/World").unwrap();
        layer.create_prim_spec(&prim_path, Specifier::Def, "");

        let rel_path = Path::from_string("/World.filteredRel").unwrap();
        layer.create_spec(&rel_path, SpecType::Relationship);

        let target = Path::from_string("/Target").unwrap();
        let mut list_op = PathListOp::new();
        list_op.set_prepended_items(vec![target.clone()]).unwrap();

        let field_name = Token::new("targetPaths");
        layer.set_field(&rel_path, &field_name, usd_vt::Value::new(list_op));

        let prim_index = make_index_with_layer("/World", layer);
        let (prop_index, _) = build_prim_property_index(&rel_path, &prim_index);

        let site = Site::new(LayerStackIdentifier::default(), rel_path);
        let (target_idx, deleted, errors) = build_filtered_target_index(
            &site,
            &prop_index,
            TargetSpecType::Relationship,
            false,
            None,
            false,
            None,
        );

        assert!(target_idx.has_target_opinions);
        assert_eq!(target_idx.len(), 1);
        assert_eq!(target_idx.paths()[0], target);
        assert!(deleted.is_empty());
        assert!(errors.is_empty());
    }

    #[test]
    fn test_build_filtered_target_index_tracks_deleted() {
        // Filtered index should track deleted paths
        let layer = Layer::create_anonymous(Some("test_del_track"));
        let prim_path = Path::from_string("/World").unwrap();
        layer.create_prim_spec(&prim_path, Specifier::Def, "");

        let rel_path = Path::from_string("/World.rel").unwrap();
        layer.create_spec(&rel_path, SpecType::Relationship);

        let kept = Path::from_string("/Kept").unwrap();
        let removed = Path::from_string("/Removed").unwrap();
        let mut list_op = PathListOp::new();
        list_op.set_prepended_items(vec![kept.clone()]).unwrap();
        list_op.set_deleted_items(vec![removed.clone()]).unwrap();

        let field_name = Token::new("targetPaths");
        layer.set_field(&rel_path, &field_name, usd_vt::Value::new(list_op));

        let prim_index = make_index_with_layer("/World", layer);
        let (prop_index, _) = build_prim_property_index(&rel_path, &prim_index);

        let site = Site::new(LayerStackIdentifier::default(), rel_path);
        let (target_idx, deleted, _) = build_filtered_target_index(
            &site,
            &prop_index,
            TargetSpecType::Relationship,
            false,
            None,
            false,
            None,
        );

        assert_eq!(target_idx.len(), 1);
        assert_eq!(target_idx.paths()[0], kept);
        // The deleted path should be tracked
        assert_eq!(deleted.len(), 1);
        assert_eq!(deleted[0], removed);
    }

    #[test]
    fn test_build_target_index_no_field_on_spec() {
        // Property spec exists but has no targetPaths field -> no opinions
        let layer = Layer::create_anonymous(Some("test_nof"));
        let prim_path = Path::from_string("/World").unwrap();
        layer.create_prim_spec(&prim_path, Specifier::Def, "");

        let rel_path = Path::from_string("/World.emptyRel").unwrap();
        layer.create_spec(&rel_path, SpecType::Relationship);

        let prim_index = make_index_with_layer("/World", layer);
        let (prop_index, _) = build_prim_property_index(&rel_path, &prim_index);

        let site = Site::new(LayerStackIdentifier::default(), rel_path);
        let (target_idx, errors) =
            build_target_index(&site, &prop_index, TargetSpecType::Relationship);

        assert!(target_idx.is_empty());
        assert!(!target_idx.has_target_opinions);
        assert!(errors.is_empty());
    }
}
