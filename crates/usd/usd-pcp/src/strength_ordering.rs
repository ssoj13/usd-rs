//! Node strength ordering for composition.
//!
//! Provides comparison functions for determining the relative strength
//! of nodes in a prim index graph, following LIVRPS ordering rules.
//!
//! # C++ Parity
//!
//! This is a port of `pxr/usd/pcp/strengthOrdering.h` and `strengthOrdering.cpp`.
//!
//! # LIVRPS Ordering
//!
//! The strength order for composition arcs is:
//! - **L** - Local (direct opinions in the layer stack)
//! - **I** - Inherits (class-based arcs)
//! - **V** - Variants (variant selections)
//! - **R** - References (external references)
//! - **P** - Payloads (deferred references)
//! - **S** - Specializes (weaker class-based arcs)
//!
//! Within each arc type category, additional rules apply:
//! - Higher namespace depth = stronger (deeper opinions are stronger)
//! - Lower sibling arc number = stronger (earlier arcs are stronger)
//! - Origin strength matters for implied arcs

use crate::utils::{find_starting_node_of_class_hierarchy, is_propagated_specializes_node};
use crate::{ArcType, NodeRef};

/// Compares the strength of sibling nodes.
///
/// The nodes must have the same parent; it is an error if they don't.
///
/// # Returns
///
/// * `-1` if `a` is stronger than `b`
/// * `0` if `a` is equivalent to `b`
/// * `1` if `a` is weaker than `b`
pub fn compare_sibling_node_strength(a: &NodeRef, b: &NodeRef) -> i32 {
    // Matches C++ TF_CODING_ERROR: log and return safe default instead of crashing
    if a.parent_node() != b.parent_node() {
        tracing::error!("PcpCompareNodeStrength: nodes are not siblings");
        return 0;
    }

    // Same node?
    if a == b {
        return 0;
    }

    // Compare arc types - relies on enum values being in strength order
    let a_type = a.arc_type();
    let b_type = b.arc_type();
    if a_type < b_type {
        return -1;
    }
    if a_type > b_type {
        return 1;
    }

    // Same arc type - apply additional rules
    if a_type.is_specialize() {
        // Specializes arcs need special handling
        compare_specialize_siblings(a, b)
    } else {
        // Standard comparison for other arc types
        compare_standard_siblings(a, b)
    }
}

/// Compares standard sibling nodes (non-specialize).
fn compare_standard_siblings(a: &NodeRef, b: &NodeRef) -> i32 {
    // Origin namespace depth - higher values (deeper) are stronger
    if a.namespace_depth() > b.namespace_depth() {
        return -1;
    }
    if a.namespace_depth() < b.namespace_depth() {
        return 1;
    }

    // Origin strength - compare origins if different
    let a_origin = a.origin_node();
    let b_origin = b.origin_node();

    if a_origin != b_origin {
        // Walk the expression tree to find which origin comes first
        let root = a.root_node();
        if root.is_valid() {
            let result = origin_is_stronger(&root, &a_origin, &b_origin);
            if result < 0 {
                return -1;
            } else if result > 0 {
                return 1;
            }
        }
    }

    // Origin sibling arc number - lower numbers are stronger
    if a.sibling_num_at_origin() < b.sibling_num_at_origin() {
        return -1;
    }
    if a.sibling_num_at_origin() > b.sibling_num_at_origin() {
        return 1;
    }

    0
}

/// Returns the namespace depth of the "instance" node at the start of the class hierarchy.
///
/// P0-5 FIX: Implements C++ `_GetNamespaceDepthForClassHierarchy`.
/// Walks up past Relocate nodes (which are just namespace placeholders) to find
/// the real instance node, then returns its namespace depth.
fn get_namespace_depth_for_class_hierarchy(n: &NodeRef) -> usize {
    if !n.is_valid() {
        return 0;
    }
    let (mut instance_node, _class_node) = find_starting_node_of_class_hierarchy(n);
    // Skip relocate nodes — they're placeholders, not real instances.
    while instance_node.arc_type() == ArcType::Relocate {
        let parent = instance_node.parent_node();
        if parent.is_valid() {
            instance_node = parent;
        } else {
            break;
        }
    }
    instance_node.namespace_depth() as usize
}

/// Compares specialize sibling nodes.
///
/// Specializes arcs need special handling because of how they're
/// copied/propagated throughout the graph for strength ordering.
fn compare_specialize_siblings(a: &NodeRef, b: &NodeRef) -> i32 {
    let (a_origin_root, a_origin_depth) = get_origin_root_node(a);
    let (b_origin_root, b_origin_depth) = get_origin_root_node(b);

    // Check if origins are nested
    let origins_nested = origins_are_nested_arcs(&a_origin_root, &b_origin_root);

    // Origin namespace depth - higher values are stronger, unless nested
    if !origins_nested {
        if a.namespace_depth() > b.namespace_depth() {
            return -1;
        }
        if a.namespace_depth() < b.namespace_depth() {
            return 1;
        }
    }

    // Origin strength
    let a_origin = a.origin_node();
    let b_origin = b.origin_node();

    let a_is_authored = a_origin == a.parent_node();
    let b_is_authored = b_origin == b.parent_node();

    if a_origin == b_origin {
        // Same origin - check if both are authored or both are implied/propagated
        if !a_is_authored && !b_is_authored {
            // Handle implied vs propagated case
            let a_is_implied = a.site() != a_origin.site();
            let b_is_implied = b.site() != b_origin.site();

            if a_is_implied && !b_is_implied {
                return -1;
            } else if !a_is_implied && b_is_implied {
                return 1;
            }
        }
    } else {
        // Different origins
        if a_origin_root != b_origin_root {
            // Different origin roots - use origin root strength
            let root = a.root_node();
            if root.is_valid() {
                let result = origin_is_stronger(&root, &a_origin_root, &b_origin_root);
                if result != 0 {
                    return result;
                }
            }
        }

        // P0-5 FIX: Step 1 — namespace depth of the instance node that inherits/specializes
        // the class hierarchy (C++ _GetNamespaceDepthForClassHierarchy).
        // This handles SpecializesAndAncestralArcs2 test case.
        let a_depth = if a_is_authored {
            0
        } else {
            get_namespace_depth_for_class_hierarchy(&a_origin)
        };
        let b_depth = if b_is_authored {
            0
        } else {
            get_namespace_depth_for_class_hierarchy(&b_origin)
        };
        if a_depth < b_depth {
            return -1;
        } else if b_depth < a_depth {
            return 1;
        }

        // Step 2 — origin chain depth (longer chain = more implied = closer to root = stronger).
        if a_origin_depth > b_origin_depth {
            return -1;
        }
        if b_origin_depth > a_origin_depth {
            return 1;
        }

        // P0-5 FIX: Step 3 — implied vs propagated within the root layer stack
        // (C++ lines 251-263, TrickySpecializesAndInherits3 test case).
        let a_layer_stack = a.layer_stack();
        let b_layer_stack = b.layer_stack();
        let a_root_layer_stack = a.root_node().layer_stack();
        let b_root_layer_stack = b.root_node().layer_stack();
        let a_in_root_ls = a_layer_stack.is_some()
            && a_root_layer_stack.is_some()
            && a_layer_stack.as_ref().map(|s| s.identifier())
                == a_root_layer_stack.as_ref().map(|s| s.identifier());
        let b_in_root_ls = b_layer_stack.is_some()
            && b_root_layer_stack.is_some()
            && b_layer_stack.as_ref().map(|s| s.identifier())
                == b_root_layer_stack.as_ref().map(|s| s.identifier());

        if a_in_root_ls && b_in_root_ls && !a_is_authored && !b_is_authored {
            let a_is_implied = a.site() != a_origin.site();
            let b_is_implied = b.site() != b_origin.site();
            if a_is_implied && !b_is_implied {
                return -1;
            } else if !a_is_implied && b_is_implied {
                return 1;
            }
        }

        // Step 4 — traverse graph to find stronger origin.
        let root = a.root_node();
        if root.is_valid() {
            let result = origin_is_stronger(&root, &a_origin, &b_origin);
            if result != 0 {
                return result;
            }
        }
    }

    // Sibling arc number
    if a.sibling_num_at_origin() < b.sibling_num_at_origin() {
        return -1;
    }
    if a.sibling_num_at_origin() > b.sibling_num_at_origin() {
        return 1;
    }

    0
}

/// Walk the chain of origins and return the root of that chain,
/// along with the number of origin nodes encountered.
fn get_origin_root_node(node: &NodeRef) -> (NodeRef, usize) {
    let mut current = node.clone();
    let mut depth = 0;

    while current.origin_node() != current.parent_node() {
        current = current.origin_node();
        depth += 1;
    }

    (current, depth)
}

/// Check if node a is a descendant of node b or vice-versa.
fn origins_are_nested_arcs(a: &NodeRef, b: &NodeRef) -> bool {
    is_nested_under(a, b) || is_nested_under(b, a)
}

/// Check if node x is nested under node y.
///
/// P0-3 FIX: C++ `isNestedUnder` follows origin ONLY for propagated specializes nodes
/// (3-condition check from utils), not for any specialize with origin != parent.
fn is_nested_under(x: &NodeRef, y: &NodeRef) -> bool {
    let mut current = x.clone();
    loop {
        if &current == y {
            return true;
        }

        // Follow origin only for *propagated* specializes (parent==root && site==origin.site).
        // C++ uses Pcp_IsPropagatedSpecializesNode which requires all 3 conditions.
        let next = if is_propagated_specializes_node(&current) {
            current.origin_node()
        } else {
            current.parent_node()
        };

        if next.is_valid() {
            current = next;
        } else {
            break;
        }
    }
    false
}

/// Walk the expression tree to find which origin comes first (stronger).
///
/// Returns:
/// * `-1` if `a` is stronger
/// * `0` if neither found
/// * `1` if `b` is stronger
fn origin_is_stronger(node: &NodeRef, a: &NodeRef, b: &NodeRef) -> i32 {
    if node == a {
        return -1;
    }
    if node == b {
        return 1;
    }

    // Recursively check children
    for child in node.children() {
        let result = origin_is_stronger(&child, a, b);
        if result != 0 {
            return result;
        }
    }

    0
}

/// Compares the strength of any two nodes in the same graph.
///
/// The nodes must be part of the same graph (have the same root);
/// it is an error if they don't.
///
/// # Returns
///
/// * `-1` if `a` is stronger than `b`
/// * `0` if `a` is equivalent to `b`
/// * `1` if `a` is weaker than `b`
pub fn compare_node_strength(a: &NodeRef, b: &NodeRef) -> i32 {
    // Matches C++ TF_CODING_ERROR: log and return safe default instead of crashing
    if a.root_node() != b.root_node() {
        tracing::error!("PcpCompareNodeStrength: nodes are not part of the same prim index");
        return 0;
    }

    if a == b {
        return 0;
    }

    // Collect path from each node to root
    let a_nodes = collect_nodes_from_node_to_root(a);
    let b_nodes = collect_nodes_from_node_to_root(b);

    compare_node_strength_internal(a, &a_nodes, b, &b_nodes)
}

/// Collect all nodes from the given node to the root.
fn collect_nodes_from_node_to_root(node: &NodeRef) -> Vec<NodeRef> {
    let mut nodes = Vec::new();
    let mut current = node.clone();

    while current.is_valid() {
        nodes.push(current.clone());
        current = current.parent_node();
    }

    nodes
}

/// Internal comparison using pre-collected node paths.
fn compare_node_strength_internal(
    a: &NodeRef,
    a_nodes: &[NodeRef],
    b: &NodeRef,
    b_nodes: &[NodeRef],
) -> i32 {
    // Ensure a_nodes is the shorter or equal path
    if b_nodes.len() < a_nodes.len() {
        return -compare_node_strength_internal(b, b_nodes, a, a_nodes);
    }

    // Search for the lowest common parent and siblings beneath it
    // Iterate from root (end of vectors) toward the nodes
    let mut a_iter = a_nodes.iter().rev();
    let mut b_iter = b_nodes.iter().rev();

    let mut last_common_a = None;
    let mut last_common_b = None;

    loop {
        match (a_iter.next(), b_iter.next()) {
            (Some(a_node), Some(b_node)) => {
                if a_node == b_node {
                    // Still on common path
                    continue;
                } else {
                    // Found divergence point - these are the siblings
                    last_common_a = Some(a_node.clone());
                    last_common_b = Some(b_node.clone());
                    break;
                }
            }
            (None, Some(_)) => {
                // a_nodes is a subset of b_nodes - a is above b, so a is stronger
                return -1;
            }
            _ => break,
        }
    }

    // Compare the sibling nodes
    if let (Some(ref a_sibling), Some(ref b_sibling)) = (last_common_a, last_common_b) {
        compare_sibling_node_strength(a_sibling, b_sibling)
    } else {
        0
    }
}

/// Compares the strength of a payload node with a sibling node.
///
/// This is used when the payload node hasn't been added to the graph yet,
/// so we use the parent node and arc number instead.
///
/// # Arguments
///
/// * `payload_parent` - The parent node of the payload
/// * `payload_arc_num` - The sibling arc number for the payload
/// * `sibling_node` - The sibling node to compare against
///
/// # Returns
///
/// * `-1` if the payload node is stronger
/// * `0` if equivalent
/// * `1` if the payload node is weaker
pub fn compare_sibling_payload_node_strength(
    payload_parent: &NodeRef,
    payload_arc_num: i32,
    sibling_node: &NodeRef,
) -> i32 {
    // Matches C++ TF_CODING_ERROR: log and return safe default instead of crashing
    if *payload_parent != sibling_node.parent_node() {
        tracing::error!("PcpCompareNodeStrength: payload node and sibling are not siblings");
        return 0;
    }

    // Compare arc types
    if ArcType::Payload < sibling_node.arc_type() {
        return -1;
    }
    if ArcType::Payload > sibling_node.arc_type() {
        return 1;
    }

    // Both are payloads - compare namespace depth
    if payload_parent.namespace_depth() > sibling_node.namespace_depth() {
        return -1;
    }
    if payload_parent.namespace_depth() < sibling_node.namespace_depth() {
        return 1;
    }

    // Compare sibling arc numbers
    if payload_arc_num < sibling_node.sibling_num_at_origin() {
        return -1;
    }
    if payload_arc_num > sibling_node.sibling_num_at_origin() {
        return 1;
    }

    0
}

// P0-4 FIX: Removed duplicate `is_propagated_specializes_node` with wrong logic (only 2 conditions).
// The correct 3-condition version is in utils.rs and imported at the top of this file.
// find_starting_node_of_class_hierarchy is also imported from utils.rs.

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LayerStackIdentifier, PrimIndexGraph, Site};
    use usd_sdf::Path;

    /// Helper to create a test graph and get root NodeRef
    fn create_test_graph_and_root(path_str: &str) -> (std::sync::Arc<PrimIndexGraph>, NodeRef) {
        let id = LayerStackIdentifier::new("test.usda");
        let path = Path::from_string(path_str).unwrap();
        let site = Site::new(id, path);
        let graph = PrimIndexGraph::new(site, true);
        let root = NodeRef::new(graph.clone(), 0);
        (graph, root)
    }

    #[test]
    fn test_arc_type_strength_order() {
        // Verify that ArcType comparison works for strength ordering
        assert!(ArcType::Root < ArcType::Inherit);
        assert!(ArcType::Inherit < ArcType::Variant);
        assert!(ArcType::Variant < ArcType::Relocate);
        assert!(ArcType::Relocate < ArcType::Reference);
        assert!(ArcType::Reference < ArcType::Payload);
        assert!(ArcType::Payload < ArcType::Specialize);
    }

    #[test]
    fn test_collect_nodes_to_root() {
        let (_graph, root) = create_test_graph_and_root("/World");

        // Root node path to root should just contain itself
        let nodes = collect_nodes_from_node_to_root(&root);
        assert_eq!(nodes.len(), 1);
    }

    #[test]
    fn test_compare_same_node() {
        let (_graph, root) = create_test_graph_and_root("/World");

        // Same node should be equivalent
        assert_eq!(compare_node_strength(&root, &root), 0);
    }

    #[test]
    fn test_origin_is_stronger_not_found() {
        let (_graph1, root) = create_test_graph_and_root("/World");
        let (_graph2, other_root) = create_test_graph_and_root("/Other");

        // Neither should be found in root's tree
        let result = origin_is_stronger(&root, &other_root, &other_root);
        assert_eq!(result, 0);
    }

    #[test]
    fn test_is_propagated_specializes_node_false() {
        let (_graph, root) = create_test_graph_and_root("/World");

        // Root node is not a propagated specializes
        assert!(!is_propagated_specializes_node(&root));
    }
}
