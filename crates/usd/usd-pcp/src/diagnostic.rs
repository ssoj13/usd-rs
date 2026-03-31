//! PCP diagnostic helpers.
//!
//! Provides debugging and diagnostic output for prim indexing and composition.
//! Includes functions for dumping prim indices, nodes, and generating DOT graphs.
//!
//! # C++ Parity
//!
//! This is a port of `pxr/usd/pcp/diagnostic.h` and `diagnostic.cpp`.

use std::fmt::Write as FmtWrite;
use std::io::Write;

use crate::{ArcType, LayerStackRefPtr, NodeRef, PrimIndex, Site};
use usd_sdf::Path;

// ============================================================================
// Public API - Dump Functions
// ============================================================================

/// Returns a debugging dump of the prim index.
///
/// # Arguments
///
/// * `prim_index` - The prim index to dump
/// * `include_inherit_origin_info` - Include origin info for inherits
/// * `include_maps` - Include map function details
pub fn dump_prim_index(
    prim_index: &PrimIndex,
    include_inherit_origin_info: bool,
    include_maps: bool,
) -> String {
    let mut result = String::new();

    writeln!(&mut result, "PrimIndex for {}", prim_index.path().as_str()).expect("fmt write");
    writeln!(&mut result, "========================================").expect("fmt write");

    if !prim_index.is_valid() {
        writeln!(&mut result, "  (invalid)").expect("fmt write");
        return result;
    }

    let root = prim_index.root_node();
    if root.is_valid() {
        dump_node_tree(
            &root,
            &mut result,
            0,
            include_inherit_origin_info,
            include_maps,
        );
    }

    result
}

/// Returns a debugging dump of the given node.
///
/// # Arguments
///
/// * `node` - The node to dump
/// * `include_inherit_origin_info` - Include origin info for inherits
/// * `include_maps` - Include map function details
pub fn dump_node(node: &NodeRef, include_inherit_origin_info: bool, include_maps: bool) -> String {
    let mut result = String::new();
    dump_node_tree(
        node,
        &mut result,
        0,
        include_inherit_origin_info,
        include_maps,
    );
    result
}

/// Writes a DOT graph representation of the prim index to a file.
///
/// # Arguments
///
/// * `prim_index` - The prim index to graph
/// * `filename` - Output filename for the .dot file
/// * `include_inherit_origin_info` - Include origin info for inherits
/// * `include_maps` - Include map function details
///
/// Matches C++ `PcpDumpDotGraph()` for PrimIndex.
pub fn dump_dot_graph_to_file(
    prim_index: &PrimIndex,
    filename: &str,
    include_inherit_origin_info: bool,
    include_maps: bool,
) -> std::io::Result<()> {
    let content = generate_dot_graph(prim_index, include_inherit_origin_info, include_maps);
    let mut file = std::fs::File::create(filename)?;
    file.write_all(content.as_bytes())?;
    Ok(())
}

/// Writes a DOT graph representation of the node to a file.
///
/// # Arguments
///
/// * `node` - The node to graph
/// * `filename` - Output filename for the .dot file
/// * `include_inherit_origin_info` - Include origin info for inherits
/// * `include_maps` - Include map function details
///
/// Matches C++ `PcpDumpDotGraph()` for NodeRef.
pub fn dump_dot_graph_node_to_file(
    node: &NodeRef,
    filename: &str,
    include_inherit_origin_info: bool,
    include_maps: bool,
) -> std::io::Result<()> {
    let content = generate_dot_graph_node(node, include_inherit_origin_info, include_maps);
    let mut file = std::fs::File::create(filename)?;
    file.write_all(content.as_bytes())?;
    Ok(())
}

/// Returns a DOT graph representation of the prim index.
pub fn generate_dot_graph(
    prim_index: &PrimIndex,
    include_inherit_origin_info: bool,
    include_maps: bool,
) -> String {
    let mut result = String::new();

    writeln!(&mut result, "digraph PrimIndex {{").expect("fmt write");
    writeln!(&mut result, "  rankdir=TB;").expect("fmt write");
    writeln!(&mut result, "  node [shape=box];").expect("fmt write");

    if prim_index.is_valid() {
        let root = prim_index.root_node();
        if root.is_valid() {
            generate_dot_nodes(
                &root,
                &mut result,
                include_inherit_origin_info,
                include_maps,
            );
        }
    }

    writeln!(&mut result, "}}").expect("fmt write");
    result
}

/// Returns a DOT graph representation of the node subtree.
pub fn generate_dot_graph_node(
    node: &NodeRef,
    include_inherit_origin_info: bool,
    include_maps: bool,
) -> String {
    let mut result = String::new();

    writeln!(&mut result, "digraph Node {{").expect("fmt write");
    writeln!(&mut result, "  rankdir=TB;").expect("fmt write");
    writeln!(&mut result, "  node [shape=box];").expect("fmt write");

    if node.is_valid() {
        generate_dot_nodes(node, &mut result, include_inherit_origin_info, include_maps);
    }

    writeln!(&mut result, "}}").expect("fmt write");
    result
}

// ============================================================================
// Internal Functions
// ============================================================================

/// Dumps a node and its children recursively.
fn dump_node_tree(
    node: &NodeRef,
    output: &mut String,
    depth: usize,
    include_inherit_origin_info: bool,
    include_maps: bool,
) {
    let indent = "  ".repeat(depth);

    // Node header
    let arc_type_str = format!("{:?}", node.arc_type());
    writeln!(
        output,
        "{}[{}] {}",
        indent,
        arc_type_str,
        node.path().as_str()
    )
    .expect("fmt write");

    // Site info
    let site = node.site();
    if site.is_valid() {
        writeln!(output, "{}  Site: {}", indent, format_site(&site)).expect("fmt write");
    }

    // Flags
    let mut flags = Vec::new();
    if node.is_culled() {
        flags.push("culled");
    }
    if node.is_inert() {
        flags.push("inert");
    }
    if node.is_restricted() {
        flags.push("restricted");
    }
    if node.has_specs() {
        flags.push("has_specs");
    }
    if node.is_due_to_ancestor() {
        flags.push("due_to_ancestor");
    }
    if !flags.is_empty() {
        writeln!(output, "{}  Flags: {}", indent, flags.join(", ")).expect("fmt write");
    }

    // Origin info for inherit/specialize arcs
    if include_inherit_origin_info && node.arc_type().is_class_based() {
        let origin = node.origin_node();
        if origin.is_valid() && origin != node.parent_node() {
            writeln!(output, "{}  Origin: {}", indent, origin.path().as_str()).expect("fmt write");
        }
    }

    // Map function
    if include_maps {
        let map_to_parent = node.map_to_parent();
        writeln!(
            output,
            "{}  MapToParent: {}",
            indent,
            map_to_parent.get_string()
        )
        .expect("fmt write");
    }

    // Recurse to children
    for child in node.children() {
        dump_node_tree(
            &child,
            output,
            depth + 1,
            include_inherit_origin_info,
            include_maps,
        );
    }
}

/// Generates DOT format nodes and edges.
fn generate_dot_nodes(
    node: &NodeRef,
    output: &mut String,
    include_inherit_origin_info: bool,
    include_maps: bool,
) {
    let node_id = format!("node_{}", node.unique_identifier());

    // Node label
    let arc_type_str = format!("{:?}", node.arc_type());
    let path_str = node.path().as_str().to_string();
    let mut label = format!("{}: {}", arc_type_str, path_str);

    // Add flags to label
    let mut flags = Vec::new();
    if node.is_culled() {
        flags.push("C");
    }
    if node.is_inert() {
        flags.push("I");
    }
    if node.has_specs() {
        flags.push("S");
    }
    if !flags.is_empty() {
        label.push_str(&format!(" [{}]", flags.join("")));
    }

    // Node styling based on arc type
    let color = match node.arc_type() {
        ArcType::Root => "lightblue",
        ArcType::Inherit => "lightgreen",
        ArcType::Specialize => "lightyellow",
        ArcType::Reference => "lightpink",
        ArcType::Payload => "orange",
        ArcType::Variant => "lightgray",
        ArcType::Relocate => "coral",
    };

    let _style = if node.is_culled() {
        ", style=dashed"
    } else {
        ""
    };

    writeln!(
        output,
        "  {} [label=\"{}\" fillcolor=\"{}\" style=\"filled{}\"];",
        node_id,
        label.replace('"', "\\\""),
        color,
        if node.is_culled() { ", dashed" } else { "" }
    )
    .expect("fmt write");

    // Add origin edge for class-based arcs
    if include_inherit_origin_info && node.arc_type().is_class_based() {
        let origin = node.origin_node();
        if origin.is_valid() && origin != node.parent_node() {
            let origin_id = format!("node_{}", origin.unique_identifier());
            writeln!(
                output,
                "  {} -> {} [style=dotted, color=purple, label=\"origin\"];",
                node_id, origin_id
            )
            .expect("fmt write");
        }
    }

    // Process children and add edges
    for child in node.children() {
        let child_id = format!("node_{}", child.unique_identifier());

        // Edge label (optional map info)
        let edge_label = if include_maps {
            let map = child.map_to_parent();
            if map.is_identity() {
                String::new()
            } else {
                format!(", label=\"{}\"", map.get_string().replace('"', "\\\""))
            }
        } else {
            String::new()
        };

        writeln!(output, "  {} -> {}{};", node_id, child_id, edge_label).expect("fmt write");

        // Recurse
        generate_dot_nodes(&child, output, include_inherit_origin_info, include_maps);
    }
}

// ============================================================================
// Site Formatting
// ============================================================================

/// Formats a site for display.
pub fn format_site(site: &Site) -> String {
    format!("@{}", site.path.as_str())
}

/// Formats a layer stack site (layer stack + path) for display.
pub fn format_layer_stack_site(layer_stack: &LayerStackRefPtr, path: &Path) -> String {
    if let Some(root_layer) = layer_stack.root_layer() {
        format!("@{}::{}", root_layer.identifier(), path.as_str())
    } else {
        format!("@<no-layer>::{}", path.as_str())
    }
}

// ============================================================================
// Consistency Checking (Debug Mode)
// ============================================================================

/// Checks the consistency of a prim index (debug validation).
///
/// In debug builds, this performs various consistency checks on the prim index
/// structure. In release builds, this is a no-op.
#[cfg(debug_assertions)]
pub fn check_consistency(prim_index: &PrimIndex) {
    if !prim_index.is_valid() {
        return;
    }

    let root = prim_index.root_node();
    if !root.is_valid() {
        return;
    }

    // Check that root has Root arc type
    assert!(
        root.arc_type() == ArcType::Root,
        "Root node should have Root arc type"
    );

    // Check that root is not culled
    assert!(!root.is_culled(), "Root node should not be culled");

    // Check all nodes
    check_node_consistency(&root);
}

/// Checks PrimIndex consistency (no-op in release builds).
#[cfg(not(debug_assertions))]
pub fn check_consistency(_prim_index: &PrimIndex) {
    // No-op in release builds
}

#[cfg(debug_assertions)]
fn check_node_consistency(node: &NodeRef) {
    // Parent relationship
    if !node.is_root_node() {
        let parent = node.parent_node();
        assert!(parent.is_valid(), "Non-root node must have valid parent");
    }

    // Check children
    for child in node.children() {
        check_node_consistency(&child);
    }
}

// ============================================================================
// Indexing Phase Debugging
// ============================================================================

/// Scope guard for indexing phase debugging.
///
/// This is used to trace the phases of prim indexing when debug output is enabled.
#[derive(Default)]
pub struct IndexingPhaseScope {
    #[cfg(debug_assertions)]
    message: Option<String>,
}

impl IndexingPhaseScope {
    /// Creates a new indexing phase scope with the given message.
    pub fn new(_prim_index: &PrimIndex, _node: &NodeRef, message: String) -> Self {
        #[cfg(debug_assertions)]
        {
            // Could log the start of the phase here
            tracing::debug!("PCP Indexing Phase: {}", message);
            Self {
                message: Some(message),
            }
        }
        #[cfg(not(debug_assertions))]
        {
            let _ = message;
            Self {}
        }
    }
}

impl Drop for IndexingPhaseScope {
    fn drop(&mut self) {
        #[cfg(debug_assertions)]
        if let Some(ref msg) = self.message {
            tracing::debug!("PCP Indexing Phase Complete: {}", msg);
        }
    }
}

/// Logs an indexing update message.
pub fn indexing_update(_prim_index: &PrimIndex, _node: &NodeRef, message: &str) {
    #[cfg(debug_assertions)]
    tracing::debug!("PCP Indexing Update: {}", message);
    #[cfg(not(debug_assertions))]
    let _ = message;
}

/// Logs an indexing message.
pub fn indexing_msg(_prim_index: &PrimIndex, message: &str) {
    #[cfg(debug_assertions)]
    tracing::debug!("PCP: {}", message);
    #[cfg(not(debug_assertions))]
    let _ = message;
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use crate::{LayerStackIdentifier, PrimIndexGraph};

    #[test]
    fn test_format_site() {
        let id = LayerStackIdentifier::new("test.usda");
        let path = Path::from_string("/World").expect("valid path");
        let site = Site::new(id, path);
        let formatted = format_site(&site);
        assert!(formatted.contains("/World"));
    }

    #[test]
    fn test_dump_invalid_prim_index() {
        let prim_index = PrimIndex::new();
        let dump = dump_prim_index(&prim_index, false, false);
        assert!(dump.contains("(invalid)"));
    }

    #[test]
    fn test_generate_dot_graph_invalid() {
        let prim_index = PrimIndex::new();
        let dot = generate_dot_graph(&prim_index, false, false);
        assert!(dot.contains("digraph"));
        assert!(dot.contains("}"));
    }

    #[test]
    fn test_indexing_phase_scope() {
        let prim_index = PrimIndex::new();
        let node = NodeRef::invalid();
        let _scope = IndexingPhaseScope::new(&prim_index, &node, "test phase".to_string());
    }

    #[test]
    fn test_check_consistency_invalid() {
        let prim_index = PrimIndex::new();
        check_consistency(&prim_index); // Should not panic
    }
}
