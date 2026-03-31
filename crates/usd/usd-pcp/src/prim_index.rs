//! PCP Prim Index - index of all sites contributing opinions to a prim.
//!
//! PcpPrimIndex is an index of all sites of scene description that contribute
//! opinions to a specific prim, under composition semantics.
//!
//! # C++ Parity
//!
//! This is a port of `pxr/usd/pcp/primIndex.h` (~458 lines).
//!
//! # Overview
//!
//! A prim index represents the computed composition structure for a single
//! prim. It contains a graph of nodes, where each node represents a site
//! (layer stack + path) that contributes opinions.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::{
    ErrorType, LayerStackIdentifier, LayerStackRefPtr, RangeType, Site, VariantFallbackMap,
};
use usd_sdf::{Layer, Path};
use usd_tf::Token;

use super::compose_site;
use super::node::NodeRef;
use super::prim_index_graph::{PrimIndexGraph, PrimIndexGraphRefPtr};
use crate::instancing::{prim_index_is_instanceable, traverse_instanceable_weak_to_strong};

// ============================================================================
// Prim Index
// ============================================================================

/// An index of all sites of scene description that contribute opinions
/// to a specific prim, under composition semantics.
///
/// PcpComputePrimIndex() builds an index ("indexes") the given prim site.
/// At any site there may be scene description values expressing arcs that
/// represent instructions to pull in further scene description.
/// PcpComputePrimIndex() recursively follows these arcs, building and
/// ordering the results.
///
/// # Examples
///
/// ```rust,ignore
/// use usd_pcp::PrimIndex;
///
/// let prim_index = cache.compute_prim_index(&path);
/// if prim_index.is_valid() {
///     let root = prim_index.root_node();
///     println!("Root path: {}", root.path().as_str());
/// }
#[derive(Clone)]
pub struct PrimIndex {
    /// The composition graph.
    graph: Option<PrimIndexGraphRefPtr>,
    /// Cached prim stack (specs in strength order).
    prim_stack: Vec<CompressedSdSite>,
    /// Local errors encountered during computation.
    local_errors: Vec<ErrorType>,
}

impl Default for PrimIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for PrimIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrimIndex")
            .field("path", &self.path().as_str())
            .field("is_valid", &self.is_valid())
            .field("num_nodes", &self.num_nodes())
            .field("local_errors", &self.local_errors.len())
            .finish()
    }
}

impl PrimIndex {
    /// Creates a new empty, invalid prim index.
    pub fn new() -> Self {
        Self {
            graph: None,
            prim_stack: Vec::new(),
            local_errors: Vec::new(),
        }
    }

    /// Creates a prim index from a graph.
    pub(crate) fn from_graph(graph: PrimIndexGraphRefPtr) -> Self {
        Self {
            graph: Some(graph),
            prim_stack: Vec::new(),
            local_errors: Vec::new(),
        }
    }

    // ========================================================================
    // Validity
    // ========================================================================

    /// Returns true if this index is valid.
    /// A default-constructed index is invalid.
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.graph.is_some()
    }

    /// Sets the graph for this index.
    pub fn set_graph(&mut self, graph: PrimIndexGraphRefPtr) {
        self.graph = Some(graph);
    }

    /// Returns the graph for this index.
    pub fn graph(&self) -> Option<&PrimIndexGraphRefPtr> {
        self.graph.as_ref()
    }

    // ========================================================================
    // Root Node
    // ========================================================================

    /// Returns the root node of the prim index graph.
    pub fn root_node(&self) -> NodeRef {
        if let Some(ref graph) = self.graph {
            NodeRef::new(graph.clone(), 0)
        } else {
            NodeRef::invalid()
        }
    }

    /// Returns the path of the prim whose opinions are represented by this index.
    pub fn path(&self) -> Path {
        self.root_node().path()
    }

    // ========================================================================
    // Query
    // ========================================================================

    /// Returns true if this prim index contains any scene description opinions.
    pub fn has_specs(&self) -> bool {
        if let Some(ref graph) = self.graph {
            // Check if any node has specs
            for i in 0..graph.num_nodes() {
                if graph.has_specs(i) {
                    return true;
                }
            }
        }
        false
    }

    /// Returns true if the prim has any authored payload arcs.
    pub fn has_any_payloads(&self) -> bool {
        self.graph.as_ref().is_some_and(|g| g.has_payloads())
    }

    /// Returns true if this prim index was composed in USD mode.
    pub fn is_usd(&self) -> bool {
        self.graph.as_ref().is_some_and(|g| g.is_usd())
    }

    /// Returns true if this prim index is instanceable.
    pub fn is_instanceable(&self) -> bool {
        self.graph.as_ref().is_some_and(|g| g.is_instanceable())
    }

    // ========================================================================
    // Iteration
    // ========================================================================

    /// Returns range of node indices for the given range type.
    pub fn get_node_range(&self, range_type: RangeType) -> (usize, usize) {
        if let Some(ref graph) = self.graph {
            let num_nodes = graph.num_nodes();
            match range_type {
                RangeType::All => (0, num_nodes),
                RangeType::Root => (0, 1.min(num_nodes)),
                _ => {
                    // Find nodes matching the arc type
                    if let Some(arc_type) = range_type.arc_type() {
                        let mut start = num_nodes;
                        let mut end = 0;
                        for i in 0..num_nodes {
                            if graph.get_arc_type(i) == arc_type {
                                start = start.min(i);
                                end = end.max(i + 1);
                            }
                        }
                        if start < end {
                            (start, end)
                        } else {
                            (num_nodes, num_nodes)
                        }
                    } else {
                        (0, num_nodes)
                    }
                }
            }
        } else {
            (0, 0)
        }
    }

    /// Returns the number of nodes in the prim index.
    pub fn num_nodes(&self) -> usize {
        if let Some(ref graph) = self.graph {
            graph.num_nodes()
        } else {
            0
        }
    }

    /// Returns all nodes in strength order.
    pub fn nodes(&self) -> Vec<NodeRef> {
        if let Some(ref graph) = self.graph {
            (0..graph.num_nodes())
                .map(|i| NodeRef::new(graph.clone(), i))
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Returns the node subtree range [start, end) for a given node.
    ///
    /// Matches C++ `PcpPrimIndex::GetNodeSubtreeRange` which delegates to
    /// `PcpPrimIndex_Graph::GetNodeIndexesForSubtreeRange`. The range
    /// includes the node itself and all of its recursive children.
    pub fn get_node_subtree_range(&self, node: &NodeRef) -> (usize, usize) {
        if !node.is_valid() {
            return (0, 0);
        }

        if let Some(ref graph) = self.graph {
            graph.get_node_subtree_range(node)
        } else {
            (0, 0)
        }
    }

    /// Returns the node iterator that points to the given node if the
    /// node is in the prim index graph.
    ///
    /// Returns an end iterator if the node is not contained in this prim index.
    pub fn get_node_iterator_at_node(&self, node: &NodeRef) -> super::iterator::NodeIterator {
        use super::iterator::NodeIterator;

        if !node.is_valid() || self.graph.is_none() {
            // Return end iterator
            if let Some(ref graph) = self.graph {
                let count = graph.num_nodes();
                return NodeIterator::new(graph.clone(), count, count);
            } else {
                // Invalid graph - return empty iterator
                let empty_site = Site::new(LayerStackIdentifier::new(""), Path::absolute_root());
                return NodeIterator::new(PrimIndexGraph::new(empty_site, false), 0, 0);
            }
        }

        let node_index = node.node_index();
        if let Some(ref graph) = self.graph {
            let count = graph.num_nodes();
            if node_index < count {
                NodeIterator::new(graph.clone(), node_index, count)
            } else {
                // Node index out of range - return end iterator
                NodeIterator::new(graph.clone(), count, count)
            }
        } else {
            // Should not happen due to check above
            let empty_site = Site::new(LayerStackIdentifier::new(""), Path::absolute_root());
            NodeIterator::new(PrimIndexGraph::new(empty_site, false), 0, 0)
        }
    }

    /// Returns range of iterators that encompasses all prims, in strong-to-weak order.
    pub fn get_prim_range(&self, range_type: RangeType) -> super::iterator::PrimRange<'_> {
        use super::iterator::PrimIterator;

        if !self.is_valid() {
            let start_iter = PrimIterator::new(self, 0);
            let end_iter = PrimIterator::new(self, 0);
            return (start_iter, end_iter);
        }

        let (start_idx, end_idx) = match range_type {
            RangeType::All => (0, self.prim_stack_len()),
            RangeType::Root => (0, 1.min(self.prim_stack_len())),
            _ => {
                // For arc type ranges, filter prim stack by nodes with matching arc type
                let mut start = self.prim_stack_len();
                let mut end = 0;
                if let Some(ref graph) = self.graph {
                    if let Some(arc_type) = range_type.arc_type() {
                        for (i, compressed) in self.prim_stack.iter().enumerate() {
                            let node = NodeRef::new(
                                self.graph.as_ref().expect("graph exists").clone(),
                                compressed.node_index,
                            );
                            if node.is_valid()
                                && graph.get_arc_type(compressed.node_index) == arc_type
                            {
                                start = start.min(i);
                                end = end.max(i + 1);
                            }
                        }
                    }
                }
                if start < end {
                    (start, end)
                } else {
                    (self.prim_stack_len(), self.prim_stack_len())
                }
            }
        };

        let start_iter = PrimIterator::new(self, start_idx);
        let end_iter = PrimIterator::new(self, end_idx);
        (start_iter, end_iter)
    }

    /// Returns range of iterators that encompasses all prims from the site of node.
    ///
    /// The node must belong to this prim index.
    pub fn get_prim_range_for_node(&self, node: &NodeRef) -> super::iterator::PrimRange<'_> {
        use super::iterator::PrimIterator;

        if !node.is_valid() || !self.is_valid() {
            let start_iter = PrimIterator::new(self, 0);
            let end_iter = PrimIterator::new(self, 0);
            return (start_iter, end_iter);
        }

        let node_index = node.node_index();
        let mut start_idx = self.prim_stack_len();
        let mut end_idx = 0;

        // Find prim stack entries that correspond to this node
        for (i, compressed) in self.prim_stack.iter().enumerate() {
            if compressed.node_index == node_index {
                start_idx = start_idx.min(i);
                end_idx = end_idx.max(i + 1);
            }
        }

        let start_iter = PrimIterator::new(self, start_idx);
        let end_iter = PrimIterator::new(self, end_idx);
        (start_iter, end_iter)
    }

    // ========================================================================
    // Lookup
    // ========================================================================

    /// Returns the node that brings opinions from the given layer and path.
    pub fn get_node_providing_spec(&self, layer: &Arc<Layer>, path: &Path) -> NodeRef {
        if let Some(ref graph) = self.graph {
            let layer_id = layer.identifier();
            for i in 0..graph.num_nodes() {
                if graph.get_site_path(i) == *path {
                    if let Some(ls) = graph.get_layer_stack(i) {
                        for l in ls.get_layers() {
                            if l.identifier() == layer_id {
                                return NodeRef::new(graph.clone(), i);
                            }
                        }
                    }
                }
            }
        }
        NodeRef::invalid()
    }

    /// Returns the node that brings opinions from the given prim spec.
    ///
    /// If no such node exists, returns an invalid NodeRef.
    pub fn get_node_providing_spec_from_prim_spec(&self, prim_spec: &usd_sdf::PrimSpec) -> NodeRef {
        let layer_handle = prim_spec.layer();
        let path = prim_spec.path();
        if let Some(layer) = layer_handle.upgrade() {
            self.get_node_providing_spec(&layer, &path)
        } else {
            NodeRef::invalid()
        }
    }

    // ========================================================================
    // Errors
    // ========================================================================

    /// Returns the list of errors local to this prim.
    pub fn local_errors(&self) -> &[ErrorType] {
        &self.local_errors
    }

    /// Returns true if there are local errors.
    pub fn has_local_errors(&self) -> bool {
        !self.local_errors.is_empty()
    }

    /// Adds a local error.
    #[allow(dead_code)] // Internal API - used during prim indexing
    pub(crate) fn add_error(&mut self, error: ErrorType) {
        self.local_errors.push(error);
    }

    // ========================================================================
    // Derived Computations
    // ========================================================================

    /// Computes the prim child names for the given path.
    ///
    /// Walks nodes strongest to weakest, collecting child names from each
    /// node's layer stack. Names from stronger nodes appear first; weaker
    /// nodes can only add new (unseen) names. Prohibited names come from
    /// restricted/culled nodes.
    pub fn compute_prim_child_names(&self) -> (Vec<Token>, Vec<Token>) {
        if let Some(ref graph) = self.graph {
            self.compose_child_names_for_range(graph, 0, graph.num_nodes())
        } else {
            (Vec::new(), Vec::new())
        }
    }

    /// Computes the prim child names for this prim when composed from only the
    /// subtree starting at subtree_root_node.
    pub fn compute_prim_child_names_in_subtree(
        &self,
        subtree_root_node: &NodeRef,
    ) -> (Vec<Token>, Vec<Token>) {
        if !subtree_root_node.is_valid() || !self.is_valid() {
            return (Vec::new(), Vec::new());
        }

        if let Some(ref graph) = self.graph {
            let (start, end) = graph.get_node_subtree_range(subtree_root_node);
            self.compose_child_names_for_range(graph, start, end)
        } else {
            (Vec::new(), Vec::new())
        }
    }

    /// Computes the prim property names.
    ///
    /// Walks all contributing nodes and collects unique property names
    /// from each node's layer stack. Properties use set-union semantics
    /// (no list-op ordering).
    pub fn compute_prim_property_names(&self) -> Vec<Token> {
        let mut names = Vec::new();
        let mut name_set = HashSet::new();

        if let Some(ref graph) = self.graph {
            let prop_children_field = Token::new("properties");
            let num_nodes = graph.num_nodes();

            for i in 0..num_nodes {
                let node = NodeRef::new(graph.clone(), i);
                if !node.can_contribute_specs() || !node.has_specs() {
                    continue;
                }

                let Some(layer_stack) = node.layer_stack() else {
                    continue;
                };
                let site_path = node.path();
                let layers = layer_stack.get_layers();

                for layer in &layers {
                    if let Some(prop_names) =
                        layer.get_field_as_token_vector(&site_path, &prop_children_field)
                    {
                        for name in prop_names {
                            if name_set.insert(name.clone()) {
                                names.push(name);
                            }
                        }
                    }
                }
            }
        }

        names
    }

    /// Composes the authored variant selections.
    ///
    /// Walks nodes strongest to weakest. Within each node, composes variant
    /// selections across the layer stack (first opinion wins). Across nodes,
    /// stronger nodes' selections take precedence.
    pub fn compose_authored_variant_selections(&self) -> HashMap<String, String> {
        let mut selections = HashMap::new();

        if let Some(ref graph) = self.graph {
            let num_nodes = graph.num_nodes();

            for i in 0..num_nodes {
                let node = NodeRef::new(graph.clone(), i);
                if !node.can_contribute_specs() || !node.has_specs() {
                    continue;
                }

                let Some(layer_stack) = node.layer_stack() else {
                    continue;
                };
                let site_path = node.path();

                // Compose variant selections from this node's layer stack
                let node_selections =
                    compose_site::compose_site_variant_selections(&layer_stack, &site_path);

                // Merge: strongest opinion wins (entry().or_insert)
                for (vset, selection) in node_selections {
                    selections.entry(vset).or_insert(selection);
                }
            }
        }

        selections
    }

    /// Internal: compose child names from a range of nodes.
    ///
    /// Iterates nodes in [start..end), collecting primChildren from each
    /// contributing node's layer stack. Uses compose_site_child_names for
    /// per-layer-stack composition, then merges across nodes (stronger first).
    fn compose_child_names_for_range(
        &self,
        graph: &PrimIndexGraphRefPtr,
        start: usize,
        _end: usize,
    ) -> (Vec<Token>, Vec<Token>) {
        let mut name_order = Vec::new();
        let mut name_set: HashSet<Token> = HashSet::new();
        let mut prohibited: HashSet<Token> = HashSet::new();

        if self.is_instanceable() && start == 0 {
            traverse_instanceable_weak_to_strong(self, |node, node_is_instanceable| {
                if node_is_instanceable {
                    Self::compose_child_names_at_node(
                        node,
                        &mut name_order,
                        &mut name_set,
                        &mut prohibited,
                    );
                }
            });
        } else {
            // C++ _ComposePrimChildNames: recursive weak-to-strong tree traversal
            let root_node = NodeRef::new(graph.clone(), start);
            Self::compose_child_names_recursive(
                &root_node,
                &mut name_order,
                &mut name_set,
                &mut prohibited,
            );
        }

        if !prohibited.is_empty() {
            name_order.retain(|n| !prohibited.contains(n));
        }
        let prohibited_vec: Vec<Token> = prohibited.into_iter().collect();
        (name_order, prohibited_vec)
    }

    /// C++ _ComposePrimChildNames: recursive weak-to-strong tree traversal.
    fn compose_child_names_recursive(
        node: &NodeRef,
        name_order: &mut Vec<Token>,
        name_set: &mut HashSet<Token>,
        prohibited: &mut HashSet<Token>,
    ) {
        if node.is_culled() {
            return;
        }

        // Reverse strength-order: children() = strongest-first, .rev() = weakest-first
        let children: Vec<NodeRef> = node.children().into_iter().collect();
        for child in children.iter().rev() {
            Self::compose_child_names_recursive(child, name_order, name_set, prohibited);
        }

        Self::compose_child_names_at_node(node, name_order, name_set, prohibited);
    }

    fn compose_child_names_at_node(
        node: &NodeRef,
        name_order: &mut Vec<Token>,
        name_set: &mut HashSet<Token>,
        prohibited: &mut HashSet<Token>,
    ) {
        // C++ _ComposePrimChildNamesAtNode
        if node.is_restricted() {
            let tk = Token::new("primChildren");
            if let Some(ls) = node.layer_stack() {
                let p = node.path();
                for layer in &ls.get_layers() {
                    if let Some(names) = layer.get_field_as_token_vector(&p, &tk) {
                        for name in names {
                            prohibited.insert(name);
                        }
                    }
                }
            }
            return;
        }
        if !node.can_contribute_specs() {
            return;
        }
        let Some(ls) = node.layer_stack() else {
            return;
        };

        let tk_children = Token::new("primChildren");
        let tk_order = Token::new("primOrder");
        let p = node.path();
        // C++ PcpComposeSiteChildNames: iterate layers weakest-to-strongest
        for layer in ls.get_layers().iter().rev() {
            if let Some(names) = layer.get_field_as_token_vector(&p, &tk_children) {
                for name in names {
                    if name_set.insert(name.clone()) {
                        name_order.push(name);
                    }
                }
            }
            if let Some(value) = layer.get_field(&p, &tk_order) {
                if let Some(order) = value.get::<usd_sdf::TokenListOp>() {
                    compose_site::apply_list_ordering(name_order, order.get_explicit_items());
                } else if let Some(order) = value.as_vec_clone::<Token>() {
                    compose_site::apply_list_ordering(name_order, &order);
                }
            }
        }
    }

    /// Returns the variant selection applied for the named variant set.
    pub fn get_selection_applied_for_variant_set(&self, variant_set: &str) -> Option<String> {
        // Would look up from computed variant selections
        self.compose_authored_variant_selections()
            .get(variant_set)
            .cloned()
    }

    // ========================================================================
    // Swap
    // ========================================================================

    /// Swaps the contents of this prim index with another.
    pub fn swap(&mut self, other: &mut PrimIndex) {
        std::mem::swap(&mut self.graph, &mut other.graph);
        std::mem::swap(&mut self.prim_stack, &mut other.prim_stack);
        std::mem::swap(&mut self.local_errors, &mut other.local_errors);
    }

    // ========================================================================
    // Prim Stack (for iterators)
    // ========================================================================

    /// Returns the length of the prim stack.
    pub fn prim_stack_len(&self) -> usize {
        self.prim_stack.len()
    }

    /// Returns the prim stack.
    pub fn prim_stack(&self) -> &[CompressedSdSite] {
        &self.prim_stack
    }

    /// Returns the site at the given prim stack index.
    pub fn get_site_at_prim_stack_index(&self, index: usize) -> Option<usd_sdf::Site> {
        if index >= self.prim_stack.len() {
            return None;
        }

        let compressed = &self.prim_stack[index];
        if let Some(ref graph) = self.graph {
            let node = NodeRef::new(graph.clone(), compressed.node_index);
            if node.is_valid() {
                if let Some(layer_stack) = node.layer_stack() {
                    let layers = layer_stack.get_layers();
                    if compressed.layer_index < layers.len() {
                        let layer = &layers[compressed.layer_index];
                        let path = node.path();
                        let handle = usd_sdf::LayerHandle::from_layer(layer);
                        return Some(usd_sdf::Site::new(handle, path));
                    }
                }
            }
        }
        None
    }

    /// Returns the node at the given prim stack index.
    pub fn get_node_at_prim_stack_index(&self, index: usize) -> Option<NodeRef> {
        if index >= self.prim_stack.len() {
            return None;
        }

        let compressed = &self.prim_stack[index];
        self.graph
            .as_ref()
            .map(|graph| NodeRef::new(graph.clone(), compressed.node_index))
    }

    /// Returns the compressed site at the given index.
    pub fn get_compressed_site_at_index(&self, index: usize) -> Option<CompressedSdSite> {
        if index < self.prim_stack.len() {
            Some(self.prim_stack[index])
        } else {
            None
        }
    }

    /// Builds the prim stack from the graph.
    ///
    /// Called after composition to populate the prim stack with all
    /// specs from nodes in strength order.
    pub fn build_prim_stack(&mut self) {
        self.prim_stack.clear();

        if let Some(ref graph) = self.graph {
            // Iterate nodes in strength order and collect specs
            for node_idx in 0..graph.num_nodes() {
                let node = NodeRef::new(graph.clone(), node_idx);
                if !node.is_valid() || node.is_culled() || node.is_inert() {
                    continue;
                }

                // Get layer stack for this node
                if let Some(layer_stack) = node.layer_stack() {
                    let layers = layer_stack.get_layers();
                    let node_path = node.path();
                    for (layer_idx, layer) in layers.iter().enumerate() {
                        // C++ checks per-layer

                        if layer.has_spec(&node_path) {
                            self.prim_stack
                                .push(CompressedSdSite::new(node_idx, layer_idx));
                        }
                    }
                }
            }
        }
    }

    // ========================================================================
    // Debug
    // ========================================================================

    /// Prints various statistics about this prim index.
    pub fn print_statistics(&self) {
        println!("PrimIndex Statistics:");
        println!("  Path: {}", self.path().as_str());
        println!("  Valid: {}", self.is_valid());
        if self.is_valid() {
            println!("  Nodes: {}", self.num_nodes());
            println!("  Has specs: {}", self.has_specs());
            println!("  Has payloads: {}", self.has_any_payloads());
            println!("  Is instanceable: {}", self.is_instanceable());
            println!("  Prim stack size: {}", self.prim_stack_len());
            println!("  Local errors: {}", self.local_errors().len());
        }
    }

    /// Dumps the prim index contents to a string.
    pub fn dump_to_string(&self, include_inherit_origin: bool, include_maps: bool) -> String {
        let mut result = String::new();

        result.push_str(&format!("PrimIndex for {}\n", self.path().as_str()));

        if !self.is_valid() {
            result.push_str("  (invalid)\n");
            return result;
        }

        result.push_str(&format!("  has_specs: {}\n", self.has_specs()));
        result.push_str(&format!("  has_payloads: {}\n", self.has_any_payloads()));
        result.push_str(&format!("  is_instanceable: {}\n", self.is_instanceable()));

        result.push_str("\nNodes:\n");
        for (i, node) in self.nodes().iter().enumerate() {
            result.push_str(&format!(
                "  [{}] {} arc={:?} path={}\n",
                i,
                if node.is_root_node() { "ROOT" } else { "    " },
                node.arc_type(),
                node.path().as_str()
            ));

            if include_maps {
                result.push_str(&format!(
                    "       mapToParent: {}\n",
                    node.map_to_parent().get_string()
                ));
                result.push_str(&format!(
                    "       mapToRoot: {}\n",
                    node.map_to_root().get_string()
                ));
            }

            if include_inherit_origin && node.arc_type().is_class_based() {
                let origin = node.origin_node();
                if origin.is_valid() {
                    result.push_str(&format!("       origin: node[{}]\n", origin.node_index()));
                }
            }
        }

        if self.has_local_errors() {
            result.push_str("\nErrors:\n");
            for error in &self.local_errors {
                result.push_str(&format!("  {:?}\n", error));
            }
        }

        result
    }
}

// ============================================================================
// Compressed Site
// ============================================================================

/// A compressed representation of an SdfSite.
/// Uses indices rather than pointers for compactness.
#[derive(Clone, Copy, Debug, Default)]
pub struct CompressedSdSite {
    /// Index of the node in the graph.
    pub node_index: usize,
    /// Index of the layer in the layer stack.
    pub layer_index: usize,
}

impl CompressedSdSite {
    /// Creates a new compressed site.
    pub fn new(node_index: usize, layer_index: usize) -> Self {
        Self {
            node_index,
            layer_index,
        }
    }
}

// ============================================================================
// Prim Index Outputs
// ============================================================================

/// Outputs of the prim indexing procedure.
#[derive(Default)]
pub struct PrimIndexOutputs {
    /// The computed prim index.
    pub prim_index: PrimIndex,
    /// All errors encountered during indexing.
    pub all_errors: Vec<ErrorType>,
    /// Payload state.
    pub payload_state: PayloadState,
    /// Culled dependencies for nodes removed during optimization.
    pub culled_dependencies: Vec<super::CulledDependency>,
    /// Dynamic file format dependency data.
    pub dynamic_file_format_dependency_data: super::DynamicFileFormatDependencyData,
    /// Expression variables dependency data.
    pub expression_variables_dependency_data: super::ExpressionVariablesDependencyData,
}

/// Describes the payload state of a prim index.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PayloadState {
    /// No payload arcs.
    #[default]
    NoPayload,
    /// Included by include set.
    IncludedByIncludeSet,
    /// Excluded by include set.
    ExcludedByIncludeSet,
    /// Included by predicate.
    IncludedByPredicate,
    /// Excluded by predicate.
    ExcludedByPredicate,
}

// ============================================================================
// Prim Index Inputs
// ============================================================================

/// Inputs for the prim indexing procedure.
pub struct PrimIndexInputs {
    /// Variant fallbacks.
    pub variant_fallbacks: Option<VariantFallbackMap>,
    /// Paths to include payloads for.
    /// None = never include payloads (C++ nullptr).
    /// Some(set) = check set when no predicate is provided.
    pub included_payloads: Option<Vec<Path>>,
    /// Predicate for payload inclusion. If present, this is the sole authority
    /// (included_payloads set is NOT checked). C++ `includePayloadPredicate`.
    /// Matches C++ UsdStage::_IncludePayloadsPredicate (calls loadRules.IsLoaded).
    pub include_payload_predicate: Option<Arc<dyn Fn(&Path) -> bool + Send + Sync>>,
    /// Whether to cull nodes without opinions.
    pub cull: bool,
    /// Whether in USD mode.
    pub usd: bool,
    /// File format target.
    pub file_format_target: String,
}

/// Controls which parts of prim-index evaluation should run for a build.
///
/// Recursive `_AddArc(... includeAncestralOpinions=true)` calls intentionally
/// defer some work until after their subgraph is merged into the final graph.
#[derive(Clone, Copy, Debug)]
pub struct PrimIndexBuildOptions {
    pub evaluate_implied_specializes: bool,
    pub evaluate_variants_and_dynamic_payloads: bool,
    pub root_node_should_contribute_specs: bool,
}

impl Default for PrimIndexBuildOptions {
    fn default() -> Self {
        Self {
            evaluate_implied_specializes: true,
            evaluate_variants_and_dynamic_payloads: true,
            root_node_should_contribute_specs: true,
        }
    }
}

impl Default for PrimIndexInputs {
    fn default() -> Self {
        Self {
            variant_fallbacks: None,
            included_payloads: None,
            include_payload_predicate: None,
            cull: true,
            usd: false,
            file_format_target: String::new(),
        }
    }
}

impl PrimIndexInputs {
    /// Creates new inputs with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets variant fallbacks.
    pub fn variant_fallbacks(mut self, fallbacks: VariantFallbackMap) -> Self {
        self.variant_fallbacks = Some(fallbacks);
        self
    }

    /// Sets included payloads.
    pub fn included_payloads(mut self, payloads: Vec<Path>) -> Self {
        self.included_payloads = Some(payloads);
        self
    }

    /// Sets the include payload predicate.
    /// C++ UsdStage passes `_IncludePayloadsPredicate` = `loadRules.IsLoaded(path)`.
    pub fn include_payload_predicate(
        mut self,
        pred: Arc<dyn Fn(&Path) -> bool + Send + Sync>,
    ) -> Self {
        self.include_payload_predicate = Some(pred);
        self
    }

    /// Sets culling flag.
    pub fn cull(mut self, do_cull: bool) -> Self {
        self.cull = do_cull;
        self
    }

    /// Sets USD mode flag.
    pub fn usd(mut self, is_usd: bool) -> Self {
        self.usd = is_usd;
        self
    }

    /// Sets file format target.
    pub fn file_format_target(mut self, target: String) -> Self {
        self.file_format_target = target;
        self
    }
}

// ============================================================================
// Composition Helpers (C++ primIndex.cpp)
// ============================================================================

/// C++ _ElideSubtree: marks node+descendants as culled/inert with restricted depth.
/// Iterative to avoid stack overflow on deep graphs.
fn elide_subtree(node: &NodeRef, cull: bool) {
    let mut stack = vec![node.clone()];
    while let Some(n) = stack.pop() {
        if cull {
            n.set_culled(true);
        } else {
            n.set_inert(true);
        }
        n.set_spec_contribution_restricted_depth(1);
        stack.extend(n.children());
    }
}

/// C++ _ComposeIsProhibitedPrimChild: traverses the graph checking if any
/// non-inert node's path is a relocation source ("salted earth" policy).
fn compose_is_prohibited_prim_child(graph: &Arc<PrimIndexGraph>) -> bool {
    let num_nodes = graph.num_nodes();
    for ni in 0..num_nodes {
        let node = NodeRef::new(graph.clone(), ni);
        if node.is_culled() || node.is_inert() {
            continue;
        }
        if let Some(ls) = node.layer_stack() {
            if !ls.has_relocates() {
                continue;
            }
            let reloc_s2t = ls.incremental_relocates_source_to_target();
            if reloc_s2t.contains_key(&node.path()) {
                return true;
            }
        }
    }
    false
}

/// C++ _ElidePrimIndexIfProhibited: if any node is a relocation source,
/// set root inert and force-cull all children.
fn elide_prim_index_if_prohibited(graph: &Arc<PrimIndexGraph>) -> bool {
    if compose_is_prohibited_prim_child(graph) {
        let root = NodeRef::new(graph.clone(), 0);
        root.set_inert(true);
        for child in root.children() {
            elide_subtree(&child, true);
        }
        return true;
    }
    false
}

/// C++ _NodeCanBeCulled: determines if a node can be removed from the graph
/// because it contributes no opinions and is not structurally required.
fn node_can_be_culled(node: &NodeRef, root_layer_stack: &LayerStackRefPtr) -> bool {
    // Already culled (ancestrally)
    if node.is_culled() {
        return true;
    }
    // Root node never culled
    if node.is_root_node() {
        return false;
    }
    // Nodes at depth_below_introduction==0 introduce arcs — keep them
    if node.depth_below_introduction() == 0 {
        return false;
    }
    // Keep nodes with symmetry
    if node.has_symmetry() {
        return false;
    }
    // Keep nodes with value clips
    if node.has_value_clips() {
        return false;
    }
    // Keep subroot inherit nodes in root layer stack (for GetBases)
    if node.arc_type() == crate::ArcType::Inherit {
        if let Some(ls) = node.layer_stack() {
            if Arc::ptr_eq(&ls, root_layer_stack) {
                let origin_node =
                    if node.origin_node().node_index() == node.parent_node().node_index() {
                        node.clone()
                    } else {
                        node.origin_root_node()
                    };
                if !origin_node.path_at_introduction().is_root_prim_path() {
                    return false;
                }
            }
        }
    }
    // If any child is not culled, we can't cull this node
    for child in node.children() {
        if !child.is_culled() {
            return false;
        }
    }
    // If node contributes opinions, keep it
    if node.has_specs() && node.can_contribute_specs() {
        return false;
    }
    true
}

/// C++ _CullSubtreesWithNoOpinions: post-build pass to mark nodes that
/// don't contribute opinions as culled.
fn cull_subtrees_with_no_opinions(
    graph: &Arc<PrimIndexGraph>,
    root_layer_stack: &LayerStackRefPtr,
) {
    let root = NodeRef::new(graph.clone(), 0);
    for child in root.children() {
        cull_subtrees_helper(&child, root_layer_stack);
    }
}

fn cull_subtrees_helper(node: &NodeRef, root_layer_stack: &LayerStackRefPtr) {
    // Recurse children first (bottom-up)
    for child in node.children() {
        cull_subtrees_helper(&child, root_layer_stack);
    }
    if node_can_be_culled(node, root_layer_stack) {
        node.set_culled(true);
    }
}

// ============================================================================
// Compute Function
// ============================================================================

/// Computes a prim index for the given path.
///
/// This is the main entry point for prim index computation.
/// Uses the PrimIndexer to process composition tasks in LIVRPS order.
pub fn compute_prim_index(
    prim_path: &Path,
    layer_stack: &LayerStackRefPtr,
    inputs: &PrimIndexInputs,
) -> PrimIndexOutputs {
    compute_prim_index_with_frame(
        prim_path,
        layer_stack,
        inputs,
        PrimIndexBuildOptions::default(),
        None,
    )
}

/// Computes a prim index with an optional stack frame for recursive composition.
///
/// C++ `Pcp_BuildPrimIndex` (primIndex.cpp:5164-5340).
/// When called from `_AddArc` with `includeAncestralOpinions=true`, a
/// `PrimIndexStackFrame` links the recursive call to the parent indexer
/// for cross-graph cycle detection.
pub fn compute_prim_index_with_frame(
    prim_path: &Path,
    layer_stack: &LayerStackRefPtr,
    inputs: &PrimIndexInputs,
    build_options: PrimIndexBuildOptions,
    previous_frame: Option<Box<super::prim_index_stack_frame::PrimIndexStackFrame>>,
) -> PrimIndexOutputs {
    let mut outputs = PrimIndexOutputs::default();
    let path_depth = prim_path.get_path_element_count();

    // C++ Pcp_BuildPrimIndex: pseudo-root (pathElementCount==0) is base case
    if path_depth == 0 {
        let site = Site::new(layer_stack.identifier().clone(), prim_path.clone());
        let g = PrimIndexGraph::new(site, inputs.usd);
        g.set_layer_stack(0, layer_stack.clone());
        let node = NodeRef::new(g.clone(), 0);
        node.set_has_specs(compose_site::compose_site_has_specs(layer_stack, prim_path));
        outputs.prim_index = PrimIndex::from_graph(g);
        return outputs;
    }

    // Variant selection: single node, no ancestor recursion
    if prim_path.is_prim_variant_selection_path() {
        let site = Site::new(layer_stack.identifier().clone(), prim_path.clone());
        let graph = PrimIndexGraph::new(site.clone(), inputs.usd);
        graph.set_layer_stack(0, layer_stack.clone());
        return run_indexer_for_path(
            graph,
            site,
            prim_path,
            layer_stack,
            inputs,
            build_options,
            false,
            true,
            None,
        );
    }

    // C++ _BuildInitialPrimIndexFromAncestor (iterative):
    // Build indexes from shallowest ancestor to prim_path. Each level inherits
    // the parent's fully-resolved graph. Uses iteration instead of recursion
    // to avoid stack overflow on deep hierarchies (caldera: 20+ levels).
    let mut ancestors = Vec::with_capacity(path_depth);
    let mut p = prim_path.clone();
    while p.get_path_element_count() > 0 {
        ancestors.push(p.clone());
        p = p.get_parent_path();
    }
    ancestors.reverse(); // shallowest first: [/A, /A/B, /A/B/C, ...]

    let mut prev_graph: Option<std::sync::Arc<PrimIndexGraph>> = None;
    let last_idx = ancestors.len() - 1;

    for (i, ancestor_path) in ancestors.iter().enumerate() {
        let ancestor_site = Site::new(layer_stack.identifier().clone(), ancestor_path.clone());

        let graph = if let Some(ref pg) = prev_graph {
            // Clone parent's graph and adjust for child namespace
            let g = PrimIndexGraph::clone_graph(pg);
            g.append_child_name_to_all_sites(ancestor_path);
            g.set_has_payloads(false);
            // C++ 5069: Reset 'has new nodes' — we haven't added nodes at this level yet
            g.set_has_new_nodes(false);

            // C++ _ConvertNodeForChild (primIndex.cpp:4763-4812):
            // re-check has_specs, propagate value clips, set ancestor flags
            let num_nodes = g.num_nodes();
            for ni in 0..num_nodes {
                let node = NodeRef::new(g.clone(), ni);
                if node.has_specs() {
                    let has = if let Some(ls) = node.layer_stack() {
                        compose_site::compose_site_has_specs(&ls, &node.path())
                    } else {
                        false
                    };
                    node.set_has_specs(has);
                }
                // C++ 4777-4784: value clips propagation in USD mode
                if !node.is_inert() && node.has_specs() && inputs.usd {
                    if !node.has_value_clips() {
                        if let Some(ls) = node.layer_stack() {
                            node.set_has_value_clips(compose_site::compose_site_has_value_clips(
                                &ls,
                                &node.path(),
                            ));
                        }
                    }
                }
                // C++ 4805-4809: non-root inherited nodes get all three ancestor flags
                if ni != 0 {
                    node.set_is_due_to_ancestor(true);
                    node.set_has_transitive_direct_dependency(false);
                    node.set_has_transitive_ancestral_dependency(true);
                }
            }
            g
        } else {
            // Depth==1: fresh graph, no parent to inherit from
            let g = PrimIndexGraph::new(ancestor_site.clone(), inputs.usd);
            g.set_layer_stack(0, layer_stack.clone());
            g
        };

        let has_ancestors = prev_graph.is_some();
        if i == last_idx {
            // Target prim — return full outputs with post-build ops
            return run_indexer_for_path(
                graph,
                ancestor_site,
                ancestor_path,
                layer_stack,
                inputs,
                build_options,
                has_ancestors,
                true,
                previous_frame,
            );
        } else {
            // Intermediate ancestor — run indexer and save graph for next level.
            // C++ _BuildInitialPrimIndexFromAncestor (line 5030-5035) passes
            // previousFrame to each recursive Pcp_BuildPrimIndex call so that
            // cross-frame duplicate detection works at every ancestor level.
            let result = run_indexer_for_path(
                graph,
                ancestor_site,
                ancestor_path,
                layer_stack,
                inputs,
                build_options,
                has_ancestors,
                false,
                previous_frame.as_ref().map(|f| f.clone()),
            );

            // C++ 5037-5047: if ancestor is instanceable, mark non-instanceable
            // nodes as inert to restrict opinions in restricted locations.
            let ancestor_instanceable = prim_index_is_instanceable(&result.prim_index);
            if ancestor_instanceable {
                crate::instancing::traverse_instanceable_strong_to_weak(
                    &result.prim_index,
                    |node, is_instanceable| {
                        if !is_instanceable {
                            node.set_inert(true);
                            return true; // continue traversal
                        }
                        false // stop descending into instanceable subtrees
                    },
                );
            }

            prev_graph = result.prim_index.graph().cloned();
        }
    }

    outputs
}

/// Run the indexer for a single prim path level and return outputs.
/// `has_ancestor_nodes`: true when graph was cloned from parent (ancestor recursion).
/// `is_final`: true for the target prim (top-level call), triggers post-build ops.
fn run_indexer_for_path(
    graph: std::sync::Arc<PrimIndexGraph>,
    site: Site,
    prim_path: &Path,
    layer_stack: &LayerStackRefPtr,
    inputs: &PrimIndexInputs,
    build_options: PrimIndexBuildOptions,
    has_ancestor_nodes: bool,
    is_final: bool,
    previous_frame: Option<Box<super::prim_index_stack_frame::PrimIndexStackFrame>>,
) -> PrimIndexOutputs {
    let mut outputs = PrimIndexOutputs::default();

    let mut indexer =
        super::indexer::PrimIndexer::new(graph, site, layer_stack.clone(), inputs.usd);
    if previous_frame.is_some() {
        indexer.set_previous_frame(previous_frame);
    }

    if let Some(ref fallbacks) = inputs.variant_fallbacks {
        indexer.set_variant_fallbacks(fallbacks.clone());
    }
    indexer.set_included_payloads(inputs.included_payloads.clone());
    if let Some(ref pred) = inputs.include_payload_predicate {
        indexer.set_include_payload_predicate(pred.clone());
    }
    indexer.set_evaluate_implied_specializes(build_options.evaluate_implied_specializes);
    indexer.set_evaluate_variants_and_dynamic_payloads(
        build_options.evaluate_variants_and_dynamic_payloads,
    );

    // Set root has_specs BEFORE task addition
    let root_node = NodeRef::new(indexer.graph().clone(), 0);
    root_node.set_has_specs(compose_site::compose_site_has_specs(layer_stack, prim_path));

    // C++ Pcp_BuildPrimIndex: check salted-earth relocation policy after
    // ancestor graph is set up but BEFORE adding composition tasks.
    if has_ancestor_nodes {
        if elide_prim_index_if_prohibited(indexer.graph()) {
            // Prohibited — all nodes are inert/culled, no tasks to run
            outputs.prim_index = PrimIndex::from_graph(indexer.into_graph());
            return outputs;
        }
    }

    if !build_options.root_node_should_contribute_specs {
        root_node.set_inert(true);
    }

    // Add tasks for all nodes (C++ AddTasksForRootNode -> _AddTasksForNodeRecursively)
    indexer.add_tasks_for_root_node(&root_node);

    indexer.run();

    outputs.payload_state = indexer.payload_state();
    // Collect errors from the indexer BEFORE consuming it
    outputs.all_errors = indexer.take_errors();
    let final_graph = indexer.into_graph();

    // C++ PcpComputePrimIndex post-build operations (only at top level)
    if is_final {
        // Cull subtrees with no opinions
        if inputs.cull {
            cull_subtrees_with_no_opinions(&final_graph, layer_stack);
        }
        // Set instanceable flag
        let temp_index = PrimIndex::from_graph(final_graph.clone());
        final_graph.set_is_instanceable(prim_index_is_instanceable(&temp_index));
        // Finalize the graph (removes culled nodes, compacts)
        final_graph.finalize();
    }

    let mut prim_idx = PrimIndex::from_graph(final_graph);
    prim_idx.build_prim_stack();
    outputs.prim_index = prim_idx;
    outputs
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ArcType, LayerStackIdentifier};

    #[test]
    fn test_invalid_prim_index() {
        let index = PrimIndex::new();
        assert!(!index.is_valid());
        assert!(index.path().is_empty());
        assert!(!index.has_specs());
    }

    #[test]
    fn test_prim_index_from_graph() {
        let site = Site::new(
            LayerStackIdentifier::default(),
            Path::from_string("/World").unwrap(),
        );
        let graph = PrimIndexGraph::new(site, true);
        let index = PrimIndex::from_graph(graph);

        assert!(index.is_valid());
        assert_eq!(index.path().as_str(), "/World");
        assert!(index.is_usd());
    }

    #[test]
    fn test_root_node() {
        let site = Site::new(
            LayerStackIdentifier::default(),
            Path::from_string("/World").unwrap(),
        );
        let graph = PrimIndexGraph::new(site, true);
        let index = PrimIndex::from_graph(graph);

        let root = index.root_node();
        assert!(root.is_valid());
        assert!(root.is_root_node());
        assert_eq!(root.arc_type(), ArcType::Root);
    }

    #[test]
    fn test_node_range() {
        let site = Site::new(
            LayerStackIdentifier::default(),
            Path::from_string("/World").unwrap(),
        );
        let graph = PrimIndexGraph::new(site, true);
        let index = PrimIndex::from_graph(graph);

        let (start, end) = index.get_node_range(RangeType::All);
        assert_eq!(start, 0);
        assert_eq!(end, 1);

        let (start, end) = index.get_node_range(RangeType::Root);
        assert_eq!(start, 0);
        assert_eq!(end, 1);
    }

    #[test]
    fn test_swap() {
        let site1 = Site::new(
            LayerStackIdentifier::default(),
            Path::from_string("/A").unwrap(),
        );
        let graph1 = PrimIndexGraph::new(site1, true);
        let mut index1 = PrimIndex::from_graph(graph1);

        let mut index2 = PrimIndex::new();

        assert!(index1.is_valid());
        assert!(!index2.is_valid());

        index1.swap(&mut index2);

        assert!(!index1.is_valid());
        assert!(index2.is_valid());
    }

    #[test]
    fn test_inputs_builder() {
        let inputs = PrimIndexInputs::new()
            .usd(true)
            .cull(false)
            .file_format_target("usd".to_string());

        assert!(inputs.usd);
        assert!(!inputs.cull);
        assert_eq!(inputs.file_format_target, "usd");
    }

    #[test]
    fn test_payload_state() {
        assert_eq!(PayloadState::default(), PayloadState::NoPayload);
    }

    // ====================================================================
    // Composition algorithm tests
    // ====================================================================

    /// Helper: creates a graph with a root node backed by a real layer stack.
    fn make_index_with_layer(prim_path: &str, layer: std::sync::Arc<usd_sdf::Layer>) -> PrimIndex {
        use crate::LayerStack;

        let layer_stack = LayerStack::from_root_layer(layer);
        let path = Path::from_string(prim_path).unwrap();
        let site = Site::new(layer_stack.identifier().clone(), path.clone());
        let graph = PrimIndexGraph::new(site, true);

        // Set layer stack on root node and mark it as having specs
        graph.set_layer_stack(0, layer_stack);
        graph.set_has_specs(0, true);

        PrimIndex::from_graph(graph)
    }

    #[test]
    fn test_compute_child_names_empty() {
        // No graph -> empty results
        let index = PrimIndex::new();
        let (names, prohibited) = index.compute_prim_child_names();
        assert!(names.is_empty());
        assert!(prohibited.is_empty());
    }

    #[test]
    fn test_compute_child_names_no_specs() {
        // Graph exists but has no specs -> empty results
        let site = Site::new(
            LayerStackIdentifier::default(),
            Path::from_string("/World").unwrap(),
        );
        let graph = PrimIndexGraph::new(site, true);
        let index = PrimIndex::from_graph(graph);

        let (names, prohibited) = index.compute_prim_child_names();
        assert!(names.is_empty());
        assert!(prohibited.is_empty());
    }

    #[test]
    fn test_compute_child_names_with_layer() {
        let layer = usd_sdf::Layer::create_anonymous(Some("test"));

        // Create prim /World with children A, B, C
        let world_path = Path::from_string("/World").unwrap();
        layer.create_prim_spec(&world_path, usd_sdf::Specifier::Def, "");

        // create_prim_spec for children adds them to primChildren of /World
        let child_a = Path::from_string("/World/A").unwrap();
        let child_b = Path::from_string("/World/B").unwrap();
        let child_c = Path::from_string("/World/C").unwrap();
        layer.create_prim_spec(&child_a, usd_sdf::Specifier::Def, "");
        layer.create_prim_spec(&child_b, usd_sdf::Specifier::Def, "");
        layer.create_prim_spec(&child_c, usd_sdf::Specifier::Def, "");

        let index = make_index_with_layer("/World", layer);
        let (names, prohibited) = index.compute_prim_child_names();

        assert_eq!(names.len(), 3, "Expected 3 child names");
        assert_eq!(names[0].as_str(), "A");
        assert_eq!(names[1].as_str(), "B");
        assert_eq!(names[2].as_str(), "C");
        assert!(prohibited.is_empty());
    }

    #[test]
    fn test_compute_property_names_empty() {
        let index = PrimIndex::new();
        let names = index.compute_prim_property_names();
        assert!(names.is_empty());
    }

    #[test]
    fn test_compute_property_names_with_layer() {
        let layer = usd_sdf::Layer::create_anonymous(Some("test"));
        let world_path = Path::from_string("/World").unwrap();
        layer.create_prim_spec(&world_path, usd_sdf::Specifier::Def, "");

        // Set propertyChildren field on /World
        let prop_children_field = Token::new("properties");
        let prop_names = vec![Token::from("xformOp:translate"), Token::from("visibility")];
        layer.set_field(
            &world_path,
            &prop_children_field,
            usd_sdf::abstract_data::Value::new(prop_names),
        );

        let index = make_index_with_layer("/World", layer);
        let names = index.compute_prim_property_names();

        assert_eq!(names.len(), 2);
        assert_eq!(names[0].as_str(), "xformOp:translate");
        assert_eq!(names[1].as_str(), "visibility");
    }

    #[test]
    fn test_variant_selections_empty() {
        let index = PrimIndex::new();
        let selections = index.compose_authored_variant_selections();
        assert!(selections.is_empty());
    }

    #[test]
    fn test_variant_selections_with_layer() {
        let layer = usd_sdf::Layer::create_anonymous(Some("test"));
        let world_path = Path::from_string("/World").unwrap();
        layer.create_prim_spec(&world_path, usd_sdf::Specifier::Def, "");

        // Set variant selections
        let vsel_token = Token::new("variantSelection");
        let mut vsel_map = HashMap::new();
        vsel_map.insert("modelingVariant".to_string(), "high".to_string());
        vsel_map.insert("shadingVariant".to_string(), "metal".to_string());
        layer.set_field(
            &world_path,
            &vsel_token,
            usd_sdf::abstract_data::Value::from_no_hash(vsel_map),
        );

        let index = make_index_with_layer("/World", layer);
        let selections = index.compose_authored_variant_selections();

        assert_eq!(selections.len(), 2);
        assert_eq!(selections.get("modelingVariant").unwrap(), "high");
        assert_eq!(selections.get("shadingVariant").unwrap(), "metal");
    }

    #[test]
    fn test_get_selection_applied_for_variant_set() {
        let layer = usd_sdf::Layer::create_anonymous(Some("test"));
        let world_path = Path::from_string("/World").unwrap();
        layer.create_prim_spec(&world_path, usd_sdf::Specifier::Def, "");

        let vsel_token = Token::new("variantSelection");
        let mut vsel_map = HashMap::new();
        vsel_map.insert("lod".to_string(), "high".to_string());
        layer.set_field(
            &world_path,
            &vsel_token,
            usd_sdf::abstract_data::Value::from_no_hash(vsel_map),
        );

        let index = make_index_with_layer("/World", layer);

        assert_eq!(
            index.get_selection_applied_for_variant_set("lod"),
            Some("high".to_string())
        );
        assert_eq!(
            index.get_selection_applied_for_variant_set("nonexistent"),
            None
        );
    }

    #[test]
    fn test_compute_child_names_dedup() {
        // If the same child name appears in multiple layers in the same stack,
        // it should be deduplicated
        let layer = usd_sdf::Layer::create_anonymous(Some("test"));
        let world_path = Path::from_string("/World").unwrap();
        layer.create_prim_spec(&world_path, usd_sdf::Specifier::Def, "");

        // Manually set primChildren with duplicates won't happen through
        // create_prim_spec, but we can verify deduplicated behavior
        let child_a = Path::from_string("/World/A").unwrap();
        let child_b = Path::from_string("/World/B").unwrap();
        layer.create_prim_spec(&child_a, usd_sdf::Specifier::Def, "");
        layer.create_prim_spec(&child_b, usd_sdf::Specifier::Def, "");

        let index = make_index_with_layer("/World", layer);
        let (names, _) = index.compute_prim_child_names();

        // Names should be unique
        let name_set: HashSet<String> = names.iter().map(|n| n.as_str().to_string()).collect();
        assert_eq!(name_set.len(), names.len(), "Child names should be unique");
    }

    #[test]
    fn test_compute_child_names_in_subtree() {
        // Subtree range with invalid node -> empty
        let site = Site::new(
            LayerStackIdentifier::default(),
            Path::from_string("/World").unwrap(),
        );
        let graph = PrimIndexGraph::new(site, true);
        let index = PrimIndex::from_graph(graph);

        let invalid_node = NodeRef::invalid();
        let (names, prohibited) = index.compute_prim_child_names_in_subtree(&invalid_node);
        assert!(names.is_empty());
        assert!(prohibited.is_empty());
    }

    #[test]
    fn test_compute_child_names_in_subtree_root() {
        // Subtree from root should be same as full computation
        let layer = usd_sdf::Layer::create_anonymous(Some("test"));
        let world_path = Path::from_string("/World").unwrap();
        layer.create_prim_spec(&world_path, usd_sdf::Specifier::Def, "");
        let child_a = Path::from_string("/World/X").unwrap();
        layer.create_prim_spec(&child_a, usd_sdf::Specifier::Def, "");

        let index = make_index_with_layer("/World", layer);
        let root_node = index.root_node();

        let (full_names, _) = index.compute_prim_child_names();
        let (subtree_names, _) = index.compute_prim_child_names_in_subtree(&root_node);

        assert_eq!(full_names, subtree_names);
    }

    // ====================================================================
    // Payload composition tests
    // ====================================================================

    /// Helper: build a layer with a payload on the given prim.
    fn make_layer_with_payload(
        prim_path: &str,
        payload_asset: &str,
    ) -> std::sync::Arc<usd_sdf::Layer> {
        let layer = usd_sdf::Layer::create_anonymous(Some("payload_test"));
        let path = Path::from_string(prim_path).unwrap();
        layer.create_prim_spec(&path, usd_sdf::Specifier::Def, "");

        // Set payload field as PayloadListOp
        let payload = usd_sdf::Payload::new(payload_asset, "");
        let list_op = usd_sdf::PayloadListOp::create_explicit(vec![payload]);
        let field_token = Token::new("payload");
        layer.set_field(
            &path,
            &field_token,
            usd_sdf::abstract_data::Value::new(list_op),
        );
        layer
    }

    /// Helper: run compute_prim_index with given inputs.
    fn run_compute(
        prim_path: &str,
        layer: std::sync::Arc<usd_sdf::Layer>,
        inputs: PrimIndexInputs,
    ) -> PrimIndexOutputs {
        use crate::LayerStack;
        let layer_stack = LayerStack::from_root_layer(layer);
        let path = Path::from_string(prim_path).unwrap();
        compute_prim_index(&path, &layer_stack, &inputs)
    }

    #[test]
    fn test_payload_predicate_include() {
        // Predicate returns true → payload should be included
        let layer = make_layer_with_payload("/Model", "payload.usda");
        let mut inputs = PrimIndexInputs::new().usd(true);
        inputs.included_payloads = Some(vec![]);
        inputs.include_payload_predicate = Some(std::sync::Arc::new(|_| true));

        let outputs = run_compute("/Model", layer, inputs);
        assert_eq!(outputs.payload_state, PayloadState::IncludedByPredicate);
    }

    #[test]
    fn test_payload_predicate_exclude() {
        // Predicate returns false → excluded by predicate
        let layer = make_layer_with_payload("/Model", "payload.usda");
        let mut inputs = PrimIndexInputs::new().usd(true);
        inputs.included_payloads = Some(vec![]);
        inputs.include_payload_predicate = Some(std::sync::Arc::new(|_| false));

        let outputs = run_compute("/Model", layer, inputs);
        assert_eq!(outputs.payload_state, PayloadState::ExcludedByPredicate);
    }

    #[test]
    fn test_payload_set_include() {
        // No predicate, path in set → included by set
        let layer = make_layer_with_payload("/Model", "payload.usda");
        let model_path = Path::from_string("/Model").unwrap();
        let mut inputs = PrimIndexInputs::new().usd(true);
        inputs.included_payloads = Some(vec![model_path]);
        // No predicate — should fall to set check

        let outputs = run_compute("/Model", layer, inputs);
        assert_eq!(outputs.payload_state, PayloadState::IncludedByIncludeSet);
    }

    #[test]
    fn test_payload_set_exclude() {
        // No predicate, path NOT in set → excluded by set
        let layer = make_layer_with_payload("/Model", "payload.usda");
        let mut inputs = PrimIndexInputs::new().usd(true);
        inputs.included_payloads = Some(vec![]); // empty set
        // No predicate

        let outputs = run_compute("/Model", layer, inputs);
        assert_eq!(outputs.payload_state, PayloadState::ExcludedByIncludeSet);
    }

    #[test]
    fn test_payload_none_set_excludes_all() {
        // included_payloads = None (C++ nullptr) → never include
        let layer = make_layer_with_payload("/Model", "payload.usda");
        let inputs = PrimIndexInputs::new().usd(true);
        // included_payloads defaults to None

        let outputs = run_compute("/Model", layer, inputs);
        assert_eq!(outputs.payload_state, PayloadState::ExcludedByIncludeSet);
    }

    #[test]
    fn test_payload_predicate_overrides_set() {
        // C++ behavior: if predicate exists, set is NOT checked.
        // Predicate says false, but path IS in set → should be EXCLUDED by predicate.
        let layer = make_layer_with_payload("/Model", "payload.usda");
        let model_path = Path::from_string("/Model").unwrap();
        let mut inputs = PrimIndexInputs::new().usd(true);
        inputs.included_payloads = Some(vec![model_path]);
        inputs.include_payload_predicate = Some(std::sync::Arc::new(|_| false));

        let outputs = run_compute("/Model", layer, inputs);
        // Predicate takes priority over set — excluded by predicate, not included by set
        assert_eq!(outputs.payload_state, PayloadState::ExcludedByPredicate);
    }

    #[test]
    fn test_payload_no_payload_state() {
        // Prim with no payload → NoPayload state
        let layer = usd_sdf::Layer::create_anonymous(Some("no_payload"));
        let path = Path::from_string("/World").unwrap();
        layer.create_prim_spec(&path, usd_sdf::Specifier::Def, "");

        let mut inputs = PrimIndexInputs::new().usd(true);
        inputs.included_payloads = Some(vec![]);
        inputs.include_payload_predicate = Some(std::sync::Arc::new(|_| true));

        let outputs = run_compute("/World", layer, inputs);
        assert_eq!(outputs.payload_state, PayloadState::NoPayload);
    }
}
