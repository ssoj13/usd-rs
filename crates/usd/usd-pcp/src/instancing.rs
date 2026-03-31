//! PCP Instancing - support for USD instancing functionality.
//!
//! This module provides utilities for determining whether prim indexes are
//! instanceable and for creating instance keys that identify shared compositions.
//!
//! # C++ Parity
//!
//! This is a port of `pxr/usd/pcp/instancing.h`, `instancing.cpp`,
//! `instanceKey.h`, and `instanceKey.cpp`.
//!
//! # Key Concepts
//!
//! - **Instanceable Node**: A node that represents a direct composition arc
//!   to a portion of scenegraph that could be shared with other prim indexes.
//!
//! - **Instance Key**: Identifies instanceable prim indexes that share the same
//!   set of opinions. Prim indexes with equal instance keys are guaranteed to
//!   have the same opinions for name children and properties.

use crate::{ArcType, NodeRef, PrimIndex, Site};
use std::fmt;
use std::hash::{Hash, Hasher};
use usd_sdf::LayerOffset;
use usd_tf::Token;

// ============================================================================
// Instance Key
// ============================================================================

/// Identifies instanceable prim indexes that share the same set of opinions.
///
/// Instanceable prim indexes with equal instance keys are guaranteed to have
/// the same opinions for name children and properties beneath those name
/// children. They are NOT guaranteed to have the same opinions for direct
/// properties of the prim indexes themselves.
#[derive(Clone, Debug, Default)]
pub struct InstanceKey {
    /// Arcs contributing to this instance.
    arcs: Vec<InstanceArc>,
    /// Variant selections affecting this instance.
    variant_selections: Vec<(String, String)>,
    /// Cached hash value.
    hash: u64,
}

/// Arc information for instance key comparison.
#[derive(Clone, Debug)]
struct InstanceArc {
    arc_type: ArcType,
    source_site: Site,
    time_offset: LayerOffset,
}

impl PartialEq for InstanceArc {
    fn eq(&self, other: &Self) -> bool {
        self.arc_type == other.arc_type
            && self.source_site == other.source_site
            && self.time_offset == other.time_offset
    }
}

impl Eq for InstanceArc {}

impl Hash for InstanceArc {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.arc_type.hash(state);
        self.source_site.hash(state);
        self.time_offset.offset().to_bits().hash(state);
        self.time_offset.scale().to_bits().hash(state);
    }
}

impl InstanceKey {
    /// Creates an empty instance key.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an instance key for the given prim index.
    pub fn from_prim_index(prim_index: &PrimIndex) -> Self {
        let mut key = Self::new();

        if !prim_index.is_valid() {
            return key;
        }

        // Collect arcs from instanceable nodes
        struct Collector {
            arcs: Vec<InstanceArc>,
            variant_selections: Vec<(String, String)>,
        }

        let mut collector = Collector {
            arcs: Vec::new(),
            variant_selections: Vec::new(),
        };

        // Traverse in strong-to-weak order
        traverse_instanceable_strong_to_weak(prim_index, |node, is_instanceable| {
            if is_instanceable {
                let site = node.site();
                if site.is_valid() {
                    let arc = InstanceArc {
                        arc_type: node.arc_type(),
                        source_site: site,
                        time_offset: node.map_to_root().time_offset(),
                    };
                    collector.arcs.push(arc);
                }
            }
            true // Continue traversal
        });

        // Collect variant selections
        let selections = prim_index.compose_authored_variant_selections();
        for (set_name, selection) in selections {
            collector.variant_selections.push((set_name, selection));
        }
        collector.variant_selections.sort();

        key.arcs = collector.arcs;
        key.variant_selections = collector.variant_selections;

        // Compute hash
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.arcs.hash(&mut hasher);
        key.variant_selections.hash(&mut hasher);
        key.hash = hasher.finish();

        key
    }

    /// Returns true if this instance key is empty (has no arcs or variant selections).
    pub fn is_empty(&self) -> bool {
        self.arcs.is_empty() && self.variant_selections.is_empty()
    }

    /// Returns the hash value.
    pub fn hash_value(&self) -> u64 {
        self.hash
    }

    /// Returns a string representation for debugging.
    pub fn get_string(&self) -> String {
        let mut result = String::new();
        result.push_str("InstanceKey {\n");
        result.push_str("  arcs: [\n");
        for arc in &self.arcs {
            result.push_str(&format!(
                "    {{ type: {:?}, site: {}, offset: ({}, {}) }}\n",
                arc.arc_type,
                arc.source_site.path.as_str(),
                arc.time_offset.offset(),
                arc.time_offset.scale()
            ));
        }
        result.push_str("  ]\n");
        result.push_str("  variants: [\n");
        for (set, sel) in &self.variant_selections {
            result.push_str(&format!("    {}: {}\n", set, sel));
        }
        result.push_str("  ]\n");
        result.push_str("}\n");
        result
    }
}

impl PartialEq for InstanceKey {
    fn eq(&self, other: &Self) -> bool {
        self.arcs == other.arcs && self.variant_selections == other.variant_selections
    }
}

impl Eq for InstanceKey {}

impl Hash for InstanceKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hash.hash(state);
    }
}

impl fmt::Display for InstanceKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.get_string())
    }
}

// ============================================================================
// Instancing Helper Functions
// ============================================================================

/// Determines whether the given prim index is instanceable.
///
/// An instanceable prim index must have instanceable nodes and must have
/// been tagged so that the composed value of the 'instanceable' metadata
/// field is true.
pub fn prim_index_is_instanceable(prim_index: &PrimIndex) -> bool {
    if !prim_index.is_usd() {
        return false;
    }

    let mut has_instanceable_data = false;
    traverse_instanceable_strong_to_weak(prim_index, |_node, node_is_instanceable| {
        if node_is_instanceable {
            has_instanceable_data = true;
            return false;
        }
        true
    });
    if !has_instanceable_data {
        return false;
    }

    compose_instanceable_opinion(prim_index.root_node()).unwrap_or(false)
}

fn compose_instanceable_opinion(root_node: NodeRef) -> Option<bool> {
    if !root_node.is_valid() {
        return None;
    }

    let field = Token::new("instanceable");
    compose_instanceable_opinion_from_node(&root_node, &field)
}

fn compose_instanceable_opinion_from_node(node: &NodeRef, field: &Token) -> Option<bool> {
    if node.can_contribute_specs() {
        if let Some(layer_stack) = node.layer_stack() {
            let path = node.path();
            for layer in layer_stack.get_layers() {
                let mut value = false;
                if layer.has_field_typed(&path, field, Some(&mut value)) {
                    return Some(value);
                }
            }
        }
    }

    for child in node.children() {
        if let Some(value) = compose_instanceable_opinion_from_node(&child, field) {
            return Some(value);
        }
    }

    None
}

/// Checks if a child node is instanceable.
///
/// A node is instanceable if:
/// - It has a transitive direct dependency (non-ancestral or in direct arc subtree)
/// - It can contribute specs
/// - It has specs
///
/// Non-ancestral nodes represent direct composition arcs to portions of
/// scenegraph that could be shared with other prim indexes.
pub fn child_node_is_instanceable(node: &NodeRef) -> bool {
    node.has_transitive_direct_dependency() && node.can_contribute_specs() && node.has_specs()
}

/// Checks if a child node is direct or in a direct arc subtree.
pub fn child_node_is_direct_or_in_direct_arc_subtree(node: &NodeRef) -> bool {
    node.is_root_node() || node.has_transitive_direct_dependency()
}

/// Checks if a node's instanceable status has changed (specs existence changed).
pub fn child_node_instanceable_changed(node: &NodeRef) -> bool {
    if !child_node_is_direct_or_in_direct_arc_subtree(node) {
        return false;
    }

    // Check if composed site has specs vs node's cached has_specs
    if let Some(layer_stack) = node.layer_stack() {
        let path = node.path();
        let composed_has_specs = crate::compose_site_has_specs(&layer_stack, &path);
        composed_has_specs != node.has_specs()
    } else {
        false
    }
}

/// Traverses a prim index in strong-to-weak order while identifying instanceable nodes.
///
/// The visitor function receives each node and a boolean indicating whether
/// that node is instanceable. If the visitor returns false, traversal is
/// pruned at that node.
pub fn traverse_instanceable_strong_to_weak<F>(prim_index: &PrimIndex, mut visitor: F)
where
    F: FnMut(&NodeRef, bool) -> bool,
{
    let root_node = prim_index.root_node();
    if !root_node.is_valid() {
        return;
    }

    // Root node is never instanceable
    if !visitor(&root_node, false) {
        return;
    }

    // Traverse children
    for child in root_node.children() {
        traverse_instanceable_helper_strong(&child, &mut visitor);
    }
}

fn traverse_instanceable_helper_strong<F>(node: &NodeRef, visitor: &mut F)
where
    F: FnMut(&NodeRef, bool) -> bool,
{
    // If culled, skip entire subtree
    if node.is_culled() {
        return;
    }

    let is_instanceable = child_node_is_instanceable(node);
    if !visitor(node, is_instanceable) {
        return;
    }

    // Continue to children
    for child in node.children() {
        traverse_instanceable_helper_strong(&child, visitor);
    }
}

/// Traverses a prim index in weak-to-strong order while identifying instanceable nodes.
///
/// The visitor function receives each node and a boolean indicating whether
/// that node is instanceable.
pub fn traverse_instanceable_weak_to_strong<F>(prim_index: &PrimIndex, mut visitor: F)
where
    F: FnMut(&NodeRef, bool),
{
    let root_node = prim_index.root_node();
    if !root_node.is_valid() {
        return;
    }

    // Traverse children in reverse order
    let children: Vec<_> = root_node.children();
    for child in children.into_iter().rev() {
        traverse_instanceable_helper_weak(&child, &mut visitor);
    }

    // Root node is never instanceable
    visitor(&root_node, false);
}

fn traverse_instanceable_helper_weak<F>(node: &NodeRef, visitor: &mut F)
where
    F: FnMut(&NodeRef, bool),
{
    // If culled, skip entire subtree
    if node.is_culled() {
        return;
    }

    // First traverse children in reverse order
    let children: Vec<_> = node.children();
    for child in children.into_iter().rev() {
        traverse_instanceable_helper_weak(&child, visitor);
    }

    // Then visit this node
    let is_instanceable = child_node_is_instanceable(node);
    visitor(node, is_instanceable);
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_instance_key() {
        let key = InstanceKey::new();
        assert!(key.arcs.is_empty());
        assert!(key.variant_selections.is_empty());
        assert!(key.is_empty());
    }

    #[test]
    fn test_instance_key_equality() {
        let key1 = InstanceKey::new();
        let key2 = InstanceKey::new();
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_invalid_prim_index() {
        let prim_index = PrimIndex::new();
        assert!(!prim_index_is_instanceable(&prim_index));
    }

    #[test]
    fn test_instance_arc_equality() {
        use crate::LayerStackIdentifier;

        let site1 = Site::new(
            LayerStackIdentifier::default(),
            usd_sdf::Path::from_string("/World").unwrap(),
        );
        let arc1 = InstanceArc {
            arc_type: ArcType::Reference,
            source_site: site1.clone(),
            time_offset: LayerOffset::identity(),
        };
        let arc2 = InstanceArc {
            arc_type: ArcType::Reference,
            source_site: site1,
            time_offset: LayerOffset::identity(),
        };
        assert_eq!(arc1, arc2);
    }

    #[test]
    fn test_instance_key_display_empty() {
        let key = InstanceKey::new();
        let display = format!("{}", key);
        assert!(display.contains("InstanceKey"));
        assert!(display.contains("arcs"));
        assert!(display.contains("variants"));
    }

    #[test]
    fn test_instance_key_get_string() {
        let key = InstanceKey::new();
        let s = key.get_string();
        assert!(s.starts_with("InstanceKey {"));
        assert!(s.contains("arcs: ["));
        assert!(s.contains("variants: ["));
    }
}
