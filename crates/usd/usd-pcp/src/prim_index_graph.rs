//! PCP Prim Index Graph - internal representation of the composition graph.
//!
//! The graph stores all nodes in a prim index and their relationships.
//!
//! # C++ Parity
//!
//! This is a port of `pxr/usd/pcp/primIndex_Graph.h` (~460 lines).

use std::sync::{Arc, RwLock};

use crate::{
    ArcType, INVALID_INDEX, LayerStackRefPtr, MapExpression, Site, compare_sibling_node_strength,
};
use usd_sdf::Path;

use super::arc::{Arc as PcpArc, INVALID_NODE_INDEX, NodeIndex};
use super::node::{NodeRef, Permission};

/// Internal node data in the graph.
#[derive(Clone)]
pub struct GraphNode {
    /// The layer stack for this node.
    pub layer_stack: Option<LayerStackRefPtr>,
    /// Mapping function to root node.
    pub map_to_root: MapExpression,
    /// Mapping function to parent node.
    pub map_to_parent: MapExpression,

    // Arc info
    /// Arc type connecting to parent.
    pub arc_type: ArcType,
    /// Parent node index.
    pub parent_index: NodeIndex,
    /// Origin node index.
    pub origin_index: NodeIndex,
    /// Sibling number at origin.
    pub sibling_num_at_origin: i32,
    /// Namespace depth.
    pub namespace_depth: i32,

    // Child/sibling indices for tree traversal
    /// First child node index.
    pub first_child_index: NodeIndex,
    /// Last child node index.
    pub last_child_index: NodeIndex,
    /// Previous sibling index.
    pub prev_sibling_index: NodeIndex,
    /// Next sibling index.
    pub next_sibling_index: NodeIndex,

    // Flags (packed in C++, separate here for clarity)
    /// Permission level.
    pub permission: Permission,
    /// Whether node has symmetry info.
    pub has_symmetry: bool,
    /// Whether node has value clips.
    pub has_value_clips: bool,
    /// Whether node is inert.
    pub inert: bool,
    /// Whether node is permission denied.
    pub permission_denied: bool,
}

impl Default for GraphNode {
    fn default() -> Self {
        Self {
            layer_stack: None,
            map_to_root: MapExpression::null(),
            map_to_parent: MapExpression::null(),
            arc_type: ArcType::Root,
            parent_index: INVALID_NODE_INDEX,
            origin_index: INVALID_NODE_INDEX,
            sibling_num_at_origin: 0,
            namespace_depth: 0,
            first_child_index: INVALID_NODE_INDEX,
            last_child_index: INVALID_NODE_INDEX,
            prev_sibling_index: INVALID_NODE_INDEX,
            next_sibling_index: INVALID_NODE_INDEX,
            permission: Permission::Public,
            has_symmetry: false,
            has_value_clips: false,
            inert: false,
            permission_denied: false,
        }
    }
}

impl GraphNode {
    /// Creates a new node from an arc.
    pub fn from_arc(arc: &PcpArc) -> Self {
        let mut node = Self::default();
        node.arc_type = arc.arc_type();
        node.parent_index = arc.parent_index();
        node.origin_index = arc.origin_index();
        node.sibling_num_at_origin = arc.sibling_num_at_origin();
        node.namespace_depth = arc.namespace_depth();
        node.map_to_parent = arc.map_to_parent().clone();
        node
    }
}

/// Unshared node data (varies per graph instance even with shared node pool).
#[derive(Clone, Default)]
pub(crate) struct UnsharedNodeData {
    /// The site path for this node.
    pub site_path: Path,
    /// Restriction depth.
    pub restriction_depth: u16,
    /// Whether node has specs.
    pub has_specs: bool,
    /// Whether node is culled.
    pub culled: bool,
    /// Whether node is due to ancestor.
    pub is_due_to_ancestor: bool,
    /// Whether node has transitive direct arc.
    pub has_transitive_direct_arc: bool,
    /// Whether node has transitive ancestral arc.
    pub has_transitive_ancestral_arc: bool,
}

/// Write guard for the shared node pool implementing COW semantics.
///
/// Dereferences to `Vec<GraphNode>` via `Arc::make_mut`, cloning the
/// pool only if other graphs still share it.
pub(crate) struct NodePoolWriteGuard<'a>(std::sync::RwLockWriteGuard<'a, Arc<Vec<GraphNode>>>);

impl std::ops::Deref for NodePoolWriteGuard<'_> {
    type Target = Vec<GraphNode>;
    fn deref(&self) -> &Vec<GraphNode> {
        &*self.0
    }
}

impl std::ops::DerefMut for NodePoolWriteGuard<'_> {
    fn deref_mut(&mut self) -> &mut Vec<GraphNode> {
        Arc::make_mut(&mut *self.0)
    }
}

/// The prim index graph - internal representation of composition structure.
///
/// This graph stores all nodes representing sites that contribute opinions
/// to a prim, along with the arcs connecting them.
pub struct PrimIndexGraph {
    /// The shared node pool (copy-on-write via Arc).
    ///
    /// Parent and child prim index graphs share this pool via `Arc::clone`.
    /// Write access calls `Arc::make_mut` to detach only when needed.
    pub(crate) shared_nodes: RwLock<Arc<Vec<GraphNode>>>,
    /// Unshared per-node data (always unique per graph instance).
    unshared: RwLock<Vec<UnsharedNodeData>>,

    // Graph flags
    /// Whether this graph was created in USD mode.
    is_usd: bool,
    /// Whether graph has payloads.
    has_payloads: RwLock<bool>,
    /// Whether graph has new nodes.
    has_new_nodes: RwLock<bool>,
    /// Whether graph is instanceable.
    is_instanceable: RwLock<bool>,
    /// Whether graph is finalized.
    is_finalized: RwLock<bool>,
}

impl PrimIndexGraph {
    // ========================================================================
    // Construction
    // ========================================================================

    /// Creates a new graph with a root node for the given site.
    pub fn new(root_site: Site, is_usd: bool) -> Arc<Self> {
        let mut root_node = GraphNode::default();
        root_node.arc_type = ArcType::Root;
        root_node.map_to_root = MapExpression::identity();
        root_node.map_to_parent = MapExpression::identity();

        let mut root_unshared = UnsharedNodeData::default();
        root_unshared.site_path = root_site.path;

        let graph = Self {
            shared_nodes: RwLock::new(Arc::new(vec![root_node])),
            unshared: RwLock::new(vec![root_unshared]),
            is_usd,
            has_payloads: RwLock::new(false),
            has_new_nodes: RwLock::new(false),
            is_instanceable: RwLock::new(false),
            is_finalized: RwLock::new(false),
        };

        // Set layer stack on root node (need to do separately due to borrow)
        // In a full implementation, would set root_site.layer_stack here

        Arc::new(graph)
    }

    /// Creates a new graph sharing the node pool with another (copy-on-write).
    ///
    /// The shared node pool is reference-counted via `Arc`. The clone shares
    /// the same node data until either graph mutates it, at which point
    /// `Arc::make_mut` triggers an actual deep clone. Unshared data (site paths,
    /// has_specs, culled flags) is always deeply cloned since it varies per graph.
    pub fn clone_graph(other: &Arc<Self>) -> Arc<Self> {
        Arc::new(Self {
            // COW: share the node pool via Arc clone (cheap)
            shared_nodes: RwLock::new(Arc::clone(
                &other.shared_nodes.read().expect("rwlock poisoned"),
            )),
            // Unshared data is always deep-cloned
            unshared: RwLock::new(other.unshared.read().expect("rwlock poisoned").clone()),
            is_usd: other.is_usd,
            has_payloads: RwLock::new(*other.has_payloads.read().expect("rwlock poisoned")),
            has_new_nodes: RwLock::new(*other.has_new_nodes.read().expect("rwlock poisoned")),
            is_instanceable: RwLock::new(*other.is_instanceable.read().expect("rwlock poisoned")),
            is_finalized: RwLock::new(*other.is_finalized.read().expect("rwlock poisoned")),
        })
    }

    // ========================================================================
    // Node Pool Access (COW helpers)
    // ========================================================================

    /// Returns a read guard to the shared node pool.
    #[inline]
    fn nodes_read(&self) -> std::sync::RwLockReadGuard<'_, Arc<Vec<GraphNode>>> {
        self.shared_nodes.read().expect("rwlock poisoned")
    }

    /// Returns a write guard to the node pool.
    ///
    /// Uses `Arc::make_mut` to detach the shared pool if needed (COW).
    /// After this call, the guard holds a unique `Vec<GraphNode>`.
    #[inline]
    fn nodes_write(&self) -> NodePoolWriteGuard<'_> {
        let guard = self.shared_nodes.write().expect("rwlock poisoned");
        NodePoolWriteGuard(guard)
    }

    /// Returns true if the node pool is shared with another graph.
    pub fn is_node_pool_shared(&self) -> bool {
        Arc::strong_count(&*self.shared_nodes.read().expect("rwlock poisoned")) > 1
    }

    // ========================================================================
    // Graph Properties
    // ========================================================================

    /// Returns true if this graph was created in USD mode.
    #[inline]
    pub fn is_usd(&self) -> bool {
        self.is_usd
    }

    /// Returns true if the graph has payloads.
    pub fn has_payloads(&self) -> bool {
        *self.has_payloads.read().expect("rwlock poisoned")
    }

    /// Sets whether the graph has payloads.
    pub fn set_has_payloads(&self, value: bool) {
        *self.has_payloads.write().expect("rwlock poisoned") = value;
    }

    /// Returns true if the graph has new nodes.
    pub fn has_new_nodes(&self) -> bool {
        *self.has_new_nodes.read().expect("rwlock poisoned")
    }

    /// Sets whether the graph has new nodes.
    pub fn set_has_new_nodes(&self, value: bool) {
        *self.has_new_nodes.write().expect("rwlock poisoned") = value;
    }

    /// Returns true if the graph is instanceable.
    pub fn is_instanceable(&self) -> bool {
        *self.is_instanceable.read().expect("rwlock poisoned")
    }

    /// Sets whether the graph is instanceable.
    pub fn set_is_instanceable(&self, value: bool) {
        *self.is_instanceable.write().expect("rwlock poisoned") = value;
    }

    /// Returns true if the graph is finalized.
    pub fn is_finalized(&self) -> bool {
        *self.is_finalized.read().expect("rwlock poisoned")
    }

    /// Finalizes the graph, optimizing internal data structures.
    pub fn finalize(&self) {
        if *self.is_finalized.read().expect("rwlock poisoned") {
            return;
        }
        // C++ Finalize(): reorder nodes to strength order (pre-order DFS)
        let mapping = self.compute_strength_order_mapping();
        let needs_reorder = mapping.iter().enumerate().any(|(i, &m)| i != m);
        if needs_reorder {
            self.apply_node_index_mapping(&mapping);
        }
        // Erase culled nodes
        let cull_mapping = self.compute_cull_mapping();
        if cull_mapping.iter().any(|&m| m == INVALID_INDEX) {
            self.apply_node_index_mapping(&cull_mapping);
        }
        *self.is_finalized.write().expect("rwlock poisoned") = true;
    }

    fn compute_strength_order_mapping(&self) -> Vec<usize> {
        let nodes = self.nodes_read();
        let mut mapping = vec![0usize; nodes.len()];
        let mut si = 0usize;
        Self::strength_recurse(&nodes, 0, &mut si, &mut mapping);
        mapping
    }

    fn strength_recurse(nodes: &[GraphNode], idx: usize, si: &mut usize, map: &mut [usize]) {
        if idx >= nodes.len() {
            return;
        }
        map[idx] = *si;
        let fc = nodes[idx].first_child_index;
        if fc != INVALID_INDEX && fc < nodes.len() {
            *si += 1;
            Self::strength_recurse(nodes, fc, si, map);
        }
        let ns = nodes[idx].next_sibling_index;
        if ns != INVALID_INDEX && ns < nodes.len() {
            *si += 1;
            Self::strength_recurse(nodes, ns, si, map);
        }
    }

    fn compute_cull_mapping(&self) -> Vec<usize> {
        let nodes = self.nodes_read();
        let unshared = self.unshared.read().expect("rwlock poisoned");
        let mut mapping = Vec::with_capacity(nodes.len());
        let mut ni = 0;
        for (idx, _node) in nodes.iter().enumerate() {
            let is_culled = unshared.get(idx).map_or(false, |u| u.culled);
            if is_culled {
                mapping.push(INVALID_INDEX);
            } else {
                mapping.push(ni);
                ni += 1;
            }
        }
        mapping
    }

    fn apply_node_index_mapping(&self, mapping: &[usize]) {
        let mut nodes_guard = self.shared_nodes.write().expect("rwlock poisoned");
        let mut unshared_guard = self.unshared.write().expect("rwlock poisoned");
        let old_nodes = nodes_guard.as_ref().clone();
        let old_unshared = unshared_guard.clone();
        let new_count = mapping.iter().filter(|&&m| m != INVALID_INDEX).count();
        let remap = |idx: usize| -> usize {
            if idx == INVALID_INDEX || idx >= mapping.len() {
                INVALID_INDEX
            } else {
                mapping[idx]
            }
        };
        let mut nn = Vec::with_capacity(new_count);
        nn.resize_with(new_count, || old_nodes[0].clone());
        let mut nu: Vec<UnsharedNodeData> = vec![UnsharedNodeData::default(); new_count];
        for (oi, node) in old_nodes.iter().enumerate() {
            let ni = mapping[oi];
            if ni == INVALID_INDEX {
                continue;
            }
            let mut n = node.clone();
            n.parent_index = remap(n.parent_index);
            n.origin_index = remap(n.origin_index);
            n.first_child_index = remap(n.first_child_index);
            n.last_child_index = remap(n.last_child_index);
            n.next_sibling_index = remap(n.next_sibling_index);
            n.prev_sibling_index = remap(n.prev_sibling_index);
            nn[ni] = n;
            if oi < old_unshared.len() {
                nu[ni] = old_unshared[oi].clone();
            }
        }
        *nodes_guard = Arc::new(nn);
        *unshared_guard = nu;
    }

    // ========================================================================
    // Node Access
    // ========================================================================

    /// Returns the number of nodes in the graph.
    pub fn num_nodes(&self) -> usize {
        self.nodes_read().len()
    }

    /// Alias for num_nodes.
    #[inline]
    pub fn node_count(&self) -> usize {
        self.num_nodes()
    }

    /// Returns a read guard to the shared node pool.
    ///
    /// This provides access to the nodes for iteration.
    /// The Arc wrapper is transparent via Deref.
    pub fn nodes(&self) -> std::sync::RwLockReadGuard<'_, Arc<Vec<GraphNode>>> {
        self.nodes_read()
    }

    /// Returns the subtree range [start, end) for a node.
    ///
    /// Matches C++ `PcpPrimIndex_Graph::GetNodeIndexesForSubtreeRange`:
    /// starts at the subtree root, then follows `last_child_index` to
    /// find the deepest last descendant. The range spans [root, last+1).
    pub fn get_node_subtree_range(&self, node: &super::NodeRef) -> (usize, usize) {
        if !node.is_valid() {
            let n = self.num_nodes();
            return (n, n);
        }

        let start = node.node_index();
        let nodes = self.nodes_read();

        // Walk last_child_index to find the deepest last descendant.
        // C++ algorithm: while node has children, follow lastChildIndex.
        let mut last = start;
        loop {
            let last_child = nodes[last].last_child_index;
            if last_child == INVALID_INDEX || last_child >= nodes.len() {
                break;
            }
            last = last_child;
        }

        (start, last + 1)
    }

    /// Returns index range for nodes in subtree (convenience method).
    pub fn get_node_indexes_for_subtree_range(&self, node: &super::NodeRef) -> (usize, usize) {
        self.get_node_subtree_range(node)
    }

    /// Returns the root node index (always 0).
    #[inline]
    pub fn root_node_index(&self) -> usize {
        0
    }

    /// Returns this graph's root node. This should always return a valid node.
    pub fn root_node(self: &Arc<Self>) -> super::node::NodeRef {
        use super::node::NodeRef;
        NodeRef::new(Arc::clone(self), 0)
    }

    /// Returns the indexes of the nodes that encompass all direct child
    /// nodes in the specified range as well as their descendants, in
    /// strong-to-weak order.
    ///
    /// By default, this returns a range encompassing the entire graph.
    pub fn get_node_indexes_for_range(
        &self,
        range_type: super::types::RangeType,
    ) -> (usize, usize) {
        let num_nodes = self.num_nodes();

        match range_type {
            super::types::RangeType::All => (0, num_nodes),
            super::types::RangeType::Root => (0, 1.min(num_nodes)),
            _ => {
                // Find nodes matching the arc type
                if let Some(arc_type) = range_type.arc_type() {
                    let nodes = self.nodes_read();
                    let mut start = num_nodes;
                    let mut end = 0;
                    for i in 0..num_nodes {
                        if let Some(node) = nodes.get(i) {
                            if node.arc_type == arc_type {
                                start = start.min(i);
                                end = end.max(i + 1);
                            }
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
    }

    /// Returns the node index of the given node in this graph.
    ///
    /// If the node is not in this graph, this returns the end index of the graph.
    pub fn get_node_index_for_node(&self, node: &super::node::NodeRef) -> usize {
        if !node.is_valid() {
            return self.num_nodes();
        }

        // Check if the node's graph matches this graph
        // In Rust, we compare by checking if the node's index is valid for this graph
        let node_idx = node.node_index();
        if node_idx < self.num_nodes() {
            // Additional check: verify the node's graph is the same
            // For now, just return the index if it's in range
            node_idx
        } else {
            self.num_nodes()
        }
    }

    /// Returns the arc type for a node.
    pub fn get_arc_type(&self, node_idx: usize) -> ArcType {
        self.nodes_read()
            .get(node_idx)
            .map(|n| n.arc_type)
            .unwrap_or(ArcType::Root)
    }

    /// Returns the parent index for a node.
    pub fn get_parent_index(&self, node_idx: usize) -> usize {
        self.nodes_read()
            .get(node_idx)
            .map(|n| n.parent_index)
            .unwrap_or(INVALID_INDEX)
    }

    /// Returns the origin index for a node.
    pub fn get_origin_index(&self, node_idx: usize) -> usize {
        self.nodes_read()
            .get(node_idx)
            .map(|n| n.origin_index)
            .unwrap_or(INVALID_INDEX)
    }

    /// Returns the map expression to parent for a node.
    pub fn get_map_to_parent(&self, node_idx: usize) -> MapExpression {
        self.nodes_read()
            .get(node_idx)
            .map(|n| n.map_to_parent.clone())
            .unwrap_or_else(MapExpression::null)
    }

    /// Returns the map expression to root for a node.
    pub fn get_map_to_root(&self, node_idx: usize) -> MapExpression {
        self.nodes_read()
            .get(node_idx)
            .map(|n| n.map_to_root.clone())
            .unwrap_or_else(MapExpression::null)
    }

    /// Returns the sibling number at origin.
    pub fn get_sibling_num_at_origin(&self, node_idx: usize) -> i32 {
        self.nodes_read()
            .get(node_idx)
            .map(|n| n.sibling_num_at_origin)
            .unwrap_or(0)
    }

    /// Returns the namespace depth.
    pub fn get_namespace_depth(&self, node_idx: usize) -> i32 {
        self.nodes_read()
            .get(node_idx)
            .map(|n| n.namespace_depth)
            .unwrap_or(0)
    }

    /// Returns whether node has a transitive direct arc.
    pub fn get_has_transitive_direct_arc(&self, node_idx: usize) -> bool {
        self.unshared
            .read()
            .expect("rwlock poisoned")
            .get(node_idx)
            .map(|n| n.has_transitive_direct_arc)
            .unwrap_or(false)
    }

    /// Returns whether node has a transitive ancestral arc.
    pub fn get_has_transitive_ancestral_arc(&self, node_idx: usize) -> bool {
        self.unshared
            .read()
            .expect("rwlock poisoned")
            .get(node_idx)
            .map(|n| n.has_transitive_ancestral_arc)
            .unwrap_or(false)
    }

    /// Sets whether node has a transitive direct arc.
    pub fn set_has_transitive_direct_arc(&self, node_idx: usize, value: bool) {
        if let Some(data) = self
            .unshared
            .write()
            .expect("rwlock poisoned")
            .get_mut(node_idx)
        {
            data.has_transitive_direct_arc = value;
        }
    }

    /// Sets whether node has a transitive ancestral arc.
    pub fn set_has_transitive_ancestral_arc(&self, node_idx: usize, value: bool) {
        if let Some(data) = self
            .unshared
            .write()
            .expect("rwlock poisoned")
            .get_mut(node_idx)
        {
            data.has_transitive_ancestral_arc = value;
        }
    }

    /// Returns the restriction depth for a node.
    pub fn get_restriction_depth(&self, node_idx: usize) -> usize {
        self.unshared
            .read()
            .expect("rwlock poisoned")
            .get(node_idx)
            .map(|d| d.restriction_depth as usize)
            .unwrap_or(0)
    }

    /// Sets the restriction depth for a node.
    pub fn set_restriction_depth(&self, node_idx: usize, depth: usize) {
        if let Some(data) = self
            .unshared
            .write()
            .expect("rwlock poisoned")
            .get_mut(node_idx)
        {
            data.restriction_depth = depth.min(u16::MAX as usize) as u16;
        }
    }

    /// Returns the site for a node.
    pub fn get_site(&self, node_idx: usize) -> Option<Site> {
        let nodes = self.nodes_read();
        let unshared = self.unshared.read().expect("rwlock poisoned");

        let node = nodes.get(node_idx)?;
        let data = unshared.get(node_idx)?;

        Some(Site {
            layer_stack_identifier: node
                .layer_stack
                .as_ref()
                .map(|ls| ls.identifier().clone())
                .unwrap_or_default(),
            path: data.site_path.clone(),
        })
    }

    /// Returns the site path for a node.
    pub fn get_site_path(&self, node_idx: usize) -> Path {
        self.unshared
            .read()
            .expect("rwlock poisoned")
            .get(node_idx)
            .map(|d| d.site_path.clone())
            .unwrap_or_else(Path::empty)
    }

    /// Returns the layer stack for a node.
    pub fn get_layer_stack(&self, node_idx: usize) -> Option<LayerStackRefPtr> {
        self.nodes_read()
            .get(node_idx)
            .and_then(|n| n.layer_stack.clone())
    }

    /// Returns children indices for a node.
    pub fn get_children_indices(&self, node_idx: usize) -> Vec<usize> {
        let nodes = self.nodes_read();
        let mut result = Vec::new();

        if let Some(node) = nodes.get(node_idx) {
            let mut child_idx = node.first_child_index;
            while child_idx != INVALID_NODE_INDEX {
                result.push(child_idx);
                if let Some(child) = nodes.get(child_idx) {
                    child_idx = child.next_sibling_index;
                } else {
                    break;
                }
            }
        }

        result
    }

    // ========================================================================
    // Node Flags
    // ========================================================================

    /// Returns whether node is due to ancestor.
    pub fn is_due_to_ancestor(&self, node_idx: usize) -> bool {
        self.unshared
            .read()
            .expect("rwlock poisoned")
            .get(node_idx)
            .map(|d| d.is_due_to_ancestor)
            .unwrap_or(false)
    }

    /// Sets whether node is due to ancestor.
    pub fn set_is_due_to_ancestor(&self, node_idx: usize, value: bool) {
        if let Some(data) = self
            .unshared
            .write()
            .expect("rwlock poisoned")
            .get_mut(node_idx)
        {
            data.is_due_to_ancestor = value;
        }
    }

    /// Returns whether node has symmetry.
    pub fn has_symmetry(&self, node_idx: usize) -> bool {
        self.nodes_read()
            .get(node_idx)
            .map(|n| n.has_symmetry)
            .unwrap_or(false)
    }

    /// Sets whether node has symmetry.
    pub fn set_has_symmetry(&self, node_idx: usize, value: bool) {
        if let Some(node) = self.nodes_write().get_mut(node_idx) {
            node.has_symmetry = value;
        }
    }

    /// Returns the permission for a node.
    pub fn get_permission(&self, node_idx: usize) -> Permission {
        self.nodes_read()
            .get(node_idx)
            .map(|n| n.permission)
            .unwrap_or(Permission::Public)
    }

    /// Sets the permission for a node.
    pub fn set_permission(&self, node_idx: usize, perm: Permission) {
        if let Some(node) = self.nodes_write().get_mut(node_idx) {
            node.permission = perm;
        }
    }

    /// Returns whether node is inert.
    pub fn is_inert(&self, node_idx: usize) -> bool {
        self.nodes_read()
            .get(node_idx)
            .map(|n| n.inert)
            .unwrap_or(false)
    }

    /// Sets whether node is inert.
    pub fn set_inert(&self, node_idx: usize, value: bool) {
        if let Some(node) = self.nodes_write().get_mut(node_idx) {
            node.inert = value;
        }
    }

    /// Returns whether node is culled.
    pub fn is_culled(&self, node_idx: usize) -> bool {
        self.unshared
            .read()
            .expect("rwlock poisoned")
            .get(node_idx)
            .map(|d| d.culled)
            .unwrap_or(false)
    }

    /// Sets whether node is culled.
    pub fn set_culled(&self, node_idx: usize, value: bool) {
        if let Some(data) = self
            .unshared
            .write()
            .expect("rwlock poisoned")
            .get_mut(node_idx)
        {
            data.culled = value;
        }
    }

    /// Returns whether node is restricted.
    pub fn is_restricted(&self, node_idx: usize) -> bool {
        self.nodes_read()
            .get(node_idx)
            .map(|n| n.permission_denied)
            .unwrap_or(false)
    }

    /// Sets whether node is restricted.
    pub fn set_restricted(&self, node_idx: usize, value: bool) {
        if let Some(node) = self.nodes_write().get_mut(node_idx) {
            node.permission_denied = value;
        }
    }

    /// Returns whether node has specs.
    pub fn has_specs(&self, node_idx: usize) -> bool {
        self.unshared
            .read()
            .expect("rwlock poisoned")
            .get(node_idx)
            .map(|d| d.has_specs)
            .unwrap_or(false)
    }

    /// Sets whether node has specs.
    pub fn set_has_specs(&self, node_idx: usize, value: bool) {
        if let Some(data) = self
            .unshared
            .write()
            .expect("rwlock poisoned")
            .get_mut(node_idx)
        {
            data.has_specs = value;
        }
    }

    /// Returns whether node has value clips.
    pub fn has_value_clips(&self, node_idx: usize) -> bool {
        self.nodes_read()
            .get(node_idx)
            .map(|n| n.has_value_clips)
            .unwrap_or(false)
    }

    /// Sets whether node has value clips.
    pub fn set_has_value_clips(&self, node_idx: usize, value: bool) {
        if let Some(node) = self.nodes_write().get_mut(node_idx) {
            node.has_value_clips = value;
        }
    }

    // ========================================================================
    // Node Insertion
    // ========================================================================

    /// Inserts a new child node.
    ///
    /// Returns the index of the new node, or INVALID_INDEX on failure.
    pub fn insert_child_node(
        self: &Arc<Self>,
        parent_idx: usize,
        site: Site,
        arc: &PcpArc,
        layer_stack: Option<LayerStackRefPtr>,
    ) -> usize {
        let new_idx = {
            let mut nodes = self.nodes_write();
            let mut unshared = self.unshared.write().expect("rwlock poisoned");

            // Create the new node
            let new_idx = nodes.len();
            let mut new_node = GraphNode::from_arc(arc);
            new_node.parent_index = parent_idx;
            new_node.layer_stack = layer_stack;

            // Compute map to root by composing with parent's map
            if let Some(parent) = nodes.get(parent_idx) {
                let parent_map = parent.map_to_root.clone();
                new_node.map_to_root = parent_map.compose(&new_node.map_to_parent);
            }

            // Add node to pool
            nodes.push(new_node);

            // Add unshared data
            let mut new_unshared = UnsharedNodeData::default();
            new_unshared.site_path = site.path;
            unshared.push(new_unshared);

            new_idx
        };

        let (insert_after, insert_before) = self.find_child_insertion_point(parent_idx, new_idx);

        let mut nodes = self.nodes_write();
        self.link_child_to_parent(
            &mut *nodes,
            parent_idx,
            new_idx,
            insert_after,
            insert_before,
        );

        new_idx
    }

    fn find_child_insertion_point(
        self: &Arc<Self>,
        parent_idx: usize,
        child_idx: usize,
    ) -> (Option<usize>, Option<usize>) {
        let parent = NodeRef::new(Arc::clone(self), parent_idx);
        let child = NodeRef::new(Arc::clone(self), child_idx);
        if !parent.is_valid() || !child.is_valid() {
            return (None, None);
        }

        let mut insert_after = None;
        for existing in parent.children() {
            if compare_sibling_node_strength(&child, &existing) < 0 {
                return (insert_after, Some(existing.node_index()));
            }
            insert_after = Some(existing.node_index());
        }

        (insert_after, None)
    }

    /// Links a child node to its parent's child list at the requested position.
    fn link_child_to_parent(
        &self,
        nodes: &mut Vec<GraphNode>,
        parent_idx: usize,
        child_idx: usize,
        insert_after: Option<usize>,
        insert_before: Option<usize>,
    ) {
        if nodes
            .get(parent_idx)
            .is_none_or(|parent| parent.first_child_index == INVALID_NODE_INDEX)
        {
            nodes[parent_idx].first_child_index = child_idx;
            nodes[parent_idx].last_child_index = child_idx;
            return;
        }

        match (insert_after, insert_before) {
            (None, Some(next_idx)) => {
                nodes[child_idx].next_sibling_index = next_idx;
                nodes[next_idx].prev_sibling_index = child_idx;
                nodes[parent_idx].first_child_index = child_idx;
            }
            (Some(prev_idx), None) => {
                nodes[child_idx].prev_sibling_index = prev_idx;
                nodes[prev_idx].next_sibling_index = child_idx;
                nodes[parent_idx].last_child_index = child_idx;
            }
            (Some(prev_idx), Some(next_idx)) => {
                nodes[child_idx].prev_sibling_index = prev_idx;
                nodes[child_idx].next_sibling_index = next_idx;
                nodes[prev_idx].next_sibling_index = child_idx;
                nodes[next_idx].prev_sibling_index = child_idx;
            }
            (None, None) => {
                nodes[parent_idx].first_child_index = child_idx;
                nodes[parent_idx].last_child_index = child_idx;
            }
        }
    }

    /// Returns the node using the given site, if one exists.
    ///
    /// Returns a NodeRef if a node using the site is found, otherwise returns an invalid NodeRef.
    pub fn get_node_using_site(self: &Arc<Self>, site: &Site) -> super::node::NodeRef {
        use super::node::NodeRef;

        if let Some(idx) = self.get_node_using_site_index(site) {
            NodeRef::new(Arc::clone(self), idx)
        } else {
            NodeRef::invalid()
        }
    }

    /// Returns the node index using the given site, if one exists.
    pub fn get_node_using_site_index(&self, site: &Site) -> Option<usize> {
        let nodes = self.nodes_read();
        let unshared = self.unshared.read().expect("rwlock poisoned");

        for (idx, (node, data)) in nodes.iter().zip(unshared.iter()).enumerate() {
            if node.inert || data.culled {
                continue;
            }

            // Check path match
            if data.site_path != site.path {
                continue;
            }

            // Check layer stack match if available
            if let Some(ref ls) = node.layer_stack {
                if ls.identifier() == &site.layer_stack_identifier {
                    return Some(idx);
                }
            } else if !site.layer_stack_identifier.is_valid() {
                // Both have empty/default identifiers
                return Some(idx);
            }
        }

        None
    }

    /// Sets the origin index for a node.
    pub fn set_origin_index(&self, node_idx: usize, origin_idx: usize) {
        if let Some(node) = self.nodes_write().get_mut(node_idx) {
            node.origin_index = origin_idx;
        }
    }

    /// Sets the sibling number at origin for a node.
    pub fn set_sibling_num_at_origin(&self, node_idx: usize, value: i32) {
        if let Some(node) = self.nodes_write().get_mut(node_idx) {
            node.sibling_num_at_origin = value;
        }
    }

    /// Sets the namespace depth for a node.
    pub fn set_namespace_depth(&self, node_idx: usize, value: i32) {
        if let Some(node) = self.nodes_write().get_mut(node_idx) {
            node.namespace_depth = value;
        }
    }

    /// Sets the layer stack for a node.
    pub fn set_layer_stack(&self, node_idx: usize, layer_stack: LayerStackRefPtr) {
        if let Some(node) = self.nodes_write().get_mut(node_idx) {
            node.layer_stack = Some(layer_stack);
        }
    }

    /// Sets the arc type for a node.
    pub fn set_arc_type(&self, node_idx: usize, arc_type: ArcType) {
        if let Some(node) = self.nodes_write().get_mut(node_idx) {
            node.arc_type = arc_type;
        }
    }

    /// Sets the map to parent for a node.
    pub fn set_map_to_parent(&self, node_idx: usize, map: MapExpression) {
        if let Some(node) = self.nodes_write().get_mut(node_idx) {
            node.map_to_parent = map;
        }
    }

    /// Appends the final element of childPath to each node's site path.
    pub fn append_child_name_to_all_sites(&self, child_path: &Path) {
        let name = child_path.get_name();
        if name.is_empty() {
            return;
        }

        let mut unshared = self.unshared.write().expect("rwlock poisoned");
        for data in unshared.iter_mut() {
            if let Some(new_path) = data.site_path.append_child(name) {
                data.site_path = new_path;
            }
        }
    }

    /// Inserts a subgraph as a child of parent_node.
    ///
    /// The root node of the subgraph will be an immediate child of parent_node,
    /// connected via arc.
    ///
    /// Returns the root node of the newly-added subgraph.
    /// If the new nodes would exceed the graph capacity, an invalid NodeRef is returned.
    pub fn insert_child_subgraph(
        self: &Arc<Self>,
        parent_node: &super::node::NodeRef,
        subgraph: Arc<PrimIndexGraph>,
        arc: &PcpArc,
        error: Option<&mut Vec<super::errors::ErrorType>>,
    ) -> super::node::NodeRef {
        use super::node::NodeRef;

        if !parent_node.is_valid() {
            if let Some(err) = error {
                err.push(super::errors::ErrorType::InvalidPrimPath);
            }
            return NodeRef::invalid();
        }

        let parent_idx = parent_node.node_index();
        let mut nodes = self.nodes_write();
        let mut unshared = self.unshared.write().expect("rwlock poisoned");
        let parent_map_to_root = nodes
            .get(parent_idx)
            .map(|node| node.map_to_root.clone())
            .unwrap_or_else(MapExpression::null);
        let subgraph_root_map_to_root = parent_map_to_root.compose(arc.map_to_parent());

        let subgraph_nodes = subgraph.shared_nodes.read().expect("rwlock poisoned");
        let subgraph_unshared = subgraph.unshared.read().expect("rwlock poisoned");

        // Check capacity (simplified - would need actual capacity check)
        let start_idx = nodes.len();

        // Copy nodes from subgraph
        for (sub_idx, sub_node) in subgraph_nodes.iter().enumerate() {
            let mut new_node = sub_node.clone();

            // Adjust parent index for root node of subgraph
            if sub_idx == 0 {
                new_node.parent_index = parent_idx;
                new_node.origin_index = arc.origin_index();
                new_node.arc_type = arc.arc_type();
                new_node.map_to_parent = arc.map_to_parent().clone();
                new_node.sibling_num_at_origin = arc.sibling_num_at_origin();
                new_node.namespace_depth = arc.namespace_depth();
            } else {
                // Adjust parent indices for other nodes
                if new_node.parent_index != INVALID_INDEX {
                    new_node.parent_index += start_idx;
                }
                if new_node.origin_index != INVALID_INDEX {
                    new_node.origin_index += start_idx;
                }
            }

            new_node.map_to_root = subgraph_root_map_to_root.compose(&sub_node.map_to_root);

            // Adjust child/sibling indices
            if new_node.first_child_index != INVALID_INDEX {
                new_node.first_child_index += start_idx;
            }
            if new_node.last_child_index != INVALID_INDEX {
                new_node.last_child_index += start_idx;
            }
            if new_node.prev_sibling_index != INVALID_INDEX {
                new_node.prev_sibling_index += start_idx;
            }
            if new_node.next_sibling_index != INVALID_INDEX {
                new_node.next_sibling_index += start_idx;
            }

            nodes.push(new_node);
        }

        // Copy unshared data
        for sub_data in subgraph_unshared.iter() {
            unshared.push(sub_data.clone());
        }

        // Link root of subgraph to parent
        let root_idx = start_idx;
        drop(nodes);
        drop(unshared);

        let (insert_after, insert_before) = self.find_child_insertion_point(parent_idx, root_idx);
        self.link_child_to_parent(
            &mut *self.nodes_write(),
            parent_idx,
            root_idx,
            insert_after,
            insert_before,
        );

        NodeRef::new(Arc::clone(self), root_idx)
    }

    /// Gets the SdfSite from a compressed site.
    pub fn get_sd_site(
        &self,
        compressed: &super::prim_index::CompressedSdSite,
    ) -> Option<usd_sdf::Site> {
        let nodes = self.nodes_read();
        let unshared = self.unshared.read().expect("rwlock poisoned");

        let node = nodes.get(compressed.node_index)?;
        let data = unshared.get(compressed.node_index)?;

        let layer_stack = node.layer_stack.as_ref()?;
        let layers = layer_stack.get_layers();
        let layer = layers.get(compressed.layer_index)?;

        Some(usd_sdf::Site::new(
            usd_sdf::LayerHandle::from_layer(layer),
            data.site_path.clone(),
        ))
    }

    /// Gets a node reference from a compressed site.
    pub fn get_node(
        self: &Arc<Self>,
        compressed: &super::prim_index::CompressedSdSite,
    ) -> super::node::NodeRef {
        use super::node::NodeRef;

        if compressed.node_index < self.num_nodes() {
            NodeRef::new(Arc::clone(self), compressed.node_index)
        } else {
            NodeRef::invalid()
        }
    }
}

// ============================================================================
// Types
// ============================================================================

/// Reference-counted pointer to a prim index graph.
pub type PrimIndexGraphRefPtr = Arc<PrimIndexGraph>;

/// Weak pointer to a prim index graph.
pub type PrimIndexGraphPtr = std::sync::Weak<PrimIndexGraph>;

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LayerStackIdentifier, MapFunction};

    #[test]
    fn test_graph_creation() {
        let site = Site::new(LayerStackIdentifier::default(), Path::absolute_root());
        let graph = PrimIndexGraph::new(site, true);

        assert!(graph.is_usd());
        assert_eq!(graph.num_nodes(), 1);
        assert!(!graph.is_finalized());
        assert!(!graph.has_payloads());
    }

    #[test]
    fn test_root_node() {
        let site = Site::new(
            LayerStackIdentifier::default(),
            Path::from_string("/World").expect("valid path"),
        );
        let graph = PrimIndexGraph::new(site, true);

        assert_eq!(graph.get_arc_type(0), ArcType::Root);
        assert_eq!(graph.get_parent_index(0), INVALID_INDEX);
        assert_eq!(graph.get_site_path(0).as_str(), "/World");
    }

    #[test]
    fn test_finalize() {
        let site = Site::new(LayerStackIdentifier::default(), Path::absolute_root());
        let graph = PrimIndexGraph::new(site, true);

        assert!(!graph.is_finalized());
        graph.finalize();
        assert!(graph.is_finalized());
    }

    #[test]
    fn test_insert_child() {
        let root_site = Site::new(
            LayerStackIdentifier::default(),
            Path::from_string("/World").expect("valid path"),
        );
        let graph = PrimIndexGraph::new(root_site, true);

        let child_site = Site::new(
            LayerStackIdentifier::default(),
            Path::from_string("/Model").expect("valid path"),
        );

        let mut arc = PcpArc::new(ArcType::Reference);
        arc.set_parent_index(0);
        arc.set_map_to_parent(MapFunction::identity().clone());

        let child_idx = graph.insert_child_node(0, child_site, &arc, None);

        assert_eq!(graph.num_nodes(), 2);
        assert_eq!(child_idx, 1);
        assert_eq!(graph.get_arc_type(child_idx), ArcType::Reference);
        assert_eq!(graph.get_parent_index(child_idx), 0);
    }

    #[test]
    fn test_children_indices() {
        let root_site = Site::new(
            LayerStackIdentifier::default(),
            Path::from_string("/World").expect("valid path"),
        );
        let graph = PrimIndexGraph::new(root_site, true);

        // Add two children
        let mut arc = PcpArc::new(ArcType::Reference);
        arc.set_parent_index(0);
        arc.set_map_to_parent(MapFunction::identity().clone());

        let child1_site = Site::new(
            LayerStackIdentifier::default(),
            Path::from_string("/Model1").expect("valid path"),
        );
        let child1_idx = graph.insert_child_node(0, child1_site, &arc, None);

        let child2_site = Site::new(
            LayerStackIdentifier::default(),
            Path::from_string("/Model2").expect("valid path"),
        );
        let child2_idx = graph.insert_child_node(0, child2_site, &arc, None);

        let children = graph.get_children_indices(0);
        assert_eq!(children.len(), 2);
        assert_eq!(children[0], child1_idx);
        assert_eq!(children[1], child2_idx);
    }

    #[test]
    fn test_cow_clone_shares_node_pool() {
        let site = Site::new(
            LayerStackIdentifier::default(),
            Path::from_string("/World").expect("valid path"),
        );
        let graph = PrimIndexGraph::new(site, true);

        // Clone shares the Arc node pool
        let cloned = PrimIndexGraph::clone_graph(&graph);

        assert!(graph.is_node_pool_shared());
        assert!(cloned.is_node_pool_shared());
        assert_eq!(graph.num_nodes(), cloned.num_nodes());
    }

    #[test]
    fn test_cow_write_detaches() {
        let site = Site::new(
            LayerStackIdentifier::default(),
            Path::from_string("/World").expect("valid path"),
        );
        let graph = PrimIndexGraph::new(site, true);

        // Clone shares the Arc node pool
        let cloned = PrimIndexGraph::clone_graph(&graph);
        assert!(graph.is_node_pool_shared());

        // Mutating the clone detaches via Arc::make_mut
        cloned.set_inert(0, true);
        assert!(cloned.is_inert(0));
        // Original is unaffected
        assert!(!graph.is_inert(0));
        // After detach, neither shares
        assert!(!cloned.is_node_pool_shared());
    }

    #[test]
    fn test_cow_read_does_not_detach() {
        let site = Site::new(
            LayerStackIdentifier::default(),
            Path::from_string("/World").expect("valid path"),
        );
        let graph = PrimIndexGraph::new(site, true);
        let cloned = PrimIndexGraph::clone_graph(&graph);

        // Reading does NOT detach
        let _ = cloned.get_arc_type(0);
        let _ = cloned.get_parent_index(0);
        let _ = cloned.is_inert(0);

        // Still shared
        assert!(graph.is_node_pool_shared());
        assert!(cloned.is_node_pool_shared());
    }
}
