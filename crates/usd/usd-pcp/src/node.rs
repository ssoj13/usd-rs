//! PCP Node - represents a node in the prim index graph.
//!
//! A node represents the opinions from a particular site. In addition,
//! it may have child nodes, representing nested expressions that are
//! composited over/under this node.
//!
//! # C++ Parity
//!
//! This is a port of `pxr/usd/pcp/node.h` (~620 lines).
//!
//! # Overview
//!
//! PcpNodeRef is a lightweight reference to a node in a prim index graph.
//! It consists of a pointer to the graph and an index into the node pool.
//!
//! Child nodes are stored and composited in strength order.
//! Each node holds information about the arc to its parent.

use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::{ArcType, INVALID_INDEX, LayerStackRefPtr, MapExpression, Site};
use usd_sdf::Path;

use super::prim_index_graph::PrimIndexGraph;

/// Permission for node access. Re-uses the canonical definition from usd-sdf.
pub type Permission = usd_sdf::Permission;

/// A reference to a node in the prim index graph.
///
/// PcpNodeRef is a lightweight handle that refers to a node in a
/// PcpPrimIndex_Graph. It consists of a pointer to the graph and
/// an index into the node pool.
///
/// # Examples
///
/// ```rust,ignore
/// use usd_pcp::NodeRef;
///
/// // Nodes are typically obtained from a PrimIndex
/// let node = prim_index.root_node();
/// if node.is_valid() {
///     println!("Path: {}", node.path().as_str());
/// }
/// ```
#[derive(Clone)]
pub struct NodeRef {
    /// Pointer to the owning graph.
    graph: Option<Arc<PrimIndexGraph>>,
    /// Index into the node pool.
    pub(crate) node_idx: usize,
}

impl Default for NodeRef {
    fn default() -> Self {
        Self {
            graph: None,
            node_idx: INVALID_INDEX,
        }
    }
}

impl NodeRef {
    /// Creates an invalid node reference.
    pub fn invalid() -> Self {
        Self::default()
    }

    /// Creates a node reference from a graph and index.
    pub fn new(graph: Arc<PrimIndexGraph>, node_idx: usize) -> Self {
        Self {
            graph: Some(graph),
            node_idx,
        }
    }

    // ========================================================================
    // Validity
    // ========================================================================

    /// Returns true if this is a valid node reference.
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.graph.is_some() && self.node_idx != INVALID_INDEX
    }

    /// Returns a unique identifier for this node.
    pub fn unique_identifier(&self) -> usize {
        if self.is_valid() {
            // Combine graph address and node index
            let graph_addr = self
                .graph
                .as_ref()
                .map(|g| Arc::as_ptr(g) as usize)
                .unwrap_or(0);
            graph_addr ^ (self.node_idx << 16)
        } else {
            0
        }
    }

    /// Returns the node index.
    ///
    /// Matches C++ access to node index.
    pub fn node_index(&self) -> usize {
        self.node_idx
    }

    // ========================================================================
    // Arc Information
    // ========================================================================

    /// Returns the type of arc connecting this node to its parent node.
    pub fn arc_type(&self) -> ArcType {
        if let Some(ref graph) = self.graph {
            graph.get_arc_type(self.node_idx)
        } else {
            ArcType::Root
        }
    }

    /// Returns this node's immediate parent node.
    /// Returns an invalid NodeRef if this is a root node.
    pub fn parent_node(&self) -> NodeRef {
        if let Some(ref graph) = self.graph {
            let parent_idx = graph.get_parent_index(self.node_idx);
            if parent_idx != INVALID_INDEX {
                NodeRef::new(graph.clone(), parent_idx)
            } else {
                NodeRef::invalid()
            }
        } else {
            NodeRef::invalid()
        }
    }

    /// Returns the origin node of this arc.
    /// For most nodes, this is the same as the parent.
    /// For implied inherits, this is the node from which the inherit was propagated.
    pub fn origin_node(&self) -> NodeRef {
        if let Some(ref graph) = self.graph {
            let origin_idx = graph.get_origin_index(self.node_idx);
            if origin_idx != INVALID_INDEX {
                NodeRef::new(graph.clone(), origin_idx)
            } else {
                NodeRef::invalid()
            }
        } else {
            NodeRef::invalid()
        }
    }

    /// Walks up to the root origin node for this node.
    pub fn origin_root_node(&self) -> NodeRef {
        let mut current = self.clone();
        loop {
            let origin = current.origin_node();
            if !origin.is_valid() || origin == current {
                break;
            }
            current = origin;
        }
        current
    }

    /// Walks up to the root node of this expression.
    pub fn root_node(&self) -> NodeRef {
        if let Some(ref graph) = self.graph {
            NodeRef::new(graph.clone(), 0) // Root is always at index 0
        } else {
            NodeRef::invalid()
        }
    }

    /// Returns the mapping function from this node to its parent.
    pub fn map_to_parent(&self) -> MapExpression {
        if let Some(ref graph) = self.graph {
            graph.get_map_to_parent(self.node_idx)
        } else {
            MapExpression::null()
        }
    }

    /// Sets the mapping expression from this node to its parent.
    ///
    /// Used by _AddArc to apply AddRootIdentity for internal references.
    pub fn set_map_to_parent_expr(&self, map: MapExpression) {
        if let Some(ref graph) = self.graph {
            graph.set_map_to_parent(self.node_idx, map);
        }
    }

    /// Returns the mapping function from this node directly to the root node.
    pub fn map_to_root(&self) -> MapExpression {
        if let Some(ref graph) = self.graph {
            graph.get_map_to_root(self.node_idx)
        } else {
            MapExpression::null()
        }
    }

    /// Returns this node's index among siblings with the same arc type.
    pub fn sibling_num_at_origin(&self) -> i32 {
        if let Some(ref graph) = self.graph {
            graph.get_sibling_num_at_origin(self.node_idx)
        } else {
            0
        }
    }

    /// Returns the absolute namespace depth of the node that introduced this node.
    pub fn namespace_depth(&self) -> i32 {
        if let Some(ref graph) = self.graph {
            graph.get_namespace_depth(self.node_idx)
        } else {
            0
        }
    }

    /// Returns how many levels below the introduction point this node is.
    ///
    /// Matches C++ `PcpNodeRef::GetDepthBelowIntroduction()`: uses the **parent**
    /// node's path element count (not this node's). If no parent, returns 0.
    pub fn depth_below_introduction(&self) -> i32 {
        let parent = self.parent_node();
        if !parent.is_valid() {
            return 0;
        }
        let parent_path = parent.path();
        let ns_depth = self.namespace_depth() as usize;

        // Count non-variant path elements in parent's path (mirrors C++).
        let path_depth = count_non_variant_path_elements(&parent_path);

        (path_depth as i32) - (ns_depth as i32)
    }

    /// Returns the path for this node's site when it was introduced.
    ///
    /// Matches C++ `_GetPathAtIntroDepth(GetPath(), GetDepthBelowIntroduction())`.
    /// Strips `depth_below_introduction` path elements from the end, skipping
    /// variant-selection path segments (which don't count as namespace depth).
    pub fn path_at_introduction(&self) -> Path {
        get_path_at_intro_depth(self.path(), self.depth_below_introduction())
    }

    /// Returns the path that introduced this node.
    ///
    /// Matches C++ `GetIntroPath()`: uses the **parent's** current path
    /// stripped by **this** node's depth_below_introduction (not the parent's).
    pub fn intro_path(&self) -> Path {
        let parent = self.parent_node();
        if !parent.is_valid() {
            return Path::absolute_root();
        }
        get_path_at_intro_depth(parent.path(), self.depth_below_introduction())
    }

    /// Returns the node's path at the same level of namespace as its origin
    /// root node was when it was added as a child.
    ///
    /// Matches C++ `GetPathAtOriginRootIntroduction()`:
    /// `_GetPathAtIntroDepth(GetPath(), GetOriginRootNode().GetDepthBelowIntroduction())`
    ///
    /// Uses **this** node's path but **origin root's** depth_below_introduction.
    pub fn path_at_origin_root_introduction(&self) -> Path {
        let origin_root = self.origin_root_node();
        get_path_at_intro_depth(self.path(), origin_root.depth_below_introduction())
    }

    // ========================================================================
    // Node Information
    // ========================================================================

    /// Returns the site this node represents.
    ///
    /// Returns a default (empty) Site if the node is invalid.
    /// Matches C++ `PcpNodeRef::GetSite()` which returns `PcpLayerStackSite`
    /// directly (not optional).
    pub fn site(&self) -> Site {
        if let Some(ref graph) = self.graph {
            graph.get_site(self.node_idx).unwrap_or_default()
        } else {
            Site::default()
        }
    }

    /// Returns the path for the site this node represents.
    pub fn path(&self) -> Path {
        if let Some(ref graph) = self.graph {
            graph.get_site_path(self.node_idx)
        } else {
            Path::empty()
        }
    }

    /// Returns the layer stack for the site this node represents.
    pub fn layer_stack(&self) -> Option<LayerStackRefPtr> {
        if let Some(ref graph) = self.graph {
            graph.get_layer_stack(self.node_idx)
        } else {
            None
        }
    }

    /// Returns true if this node is the root node of the prim index graph.
    pub fn is_root_node(&self) -> bool {
        self.is_valid() && self.node_idx == 0
    }

    // ========================================================================
    // Flags
    // ========================================================================

    /// Returns true if this node was introduced by copying from namespace ancestor.
    pub fn is_due_to_ancestor(&self) -> bool {
        if let Some(ref graph) = self.graph {
            graph.is_due_to_ancestor(self.node_idx)
        } else {
            false
        }
    }

    /// Sets whether this node was introduced by ancestor.
    pub fn set_is_due_to_ancestor(&self, value: bool) {
        if let Some(ref graph) = self.graph {
            graph.set_is_due_to_ancestor(self.node_idx, value);
        }
    }

    /// Returns true if this node contributes symmetry information.
    pub fn has_symmetry(&self) -> bool {
        if let Some(ref graph) = self.graph {
            graph.has_symmetry(self.node_idx)
        } else {
            false
        }
    }

    /// Sets whether this node has symmetry.
    pub fn set_has_symmetry(&self, value: bool) {
        if let Some(ref graph) = self.graph {
            graph.set_has_symmetry(self.node_idx, value);
        }
    }

    /// Returns the permission for this node.
    pub fn permission(&self) -> Permission {
        if let Some(ref graph) = self.graph {
            graph.get_permission(self.node_idx)
        } else {
            Permission::Public
        }
    }

    /// Sets the permission for this node.
    pub fn set_permission(&self, perm: Permission) {
        if let Some(ref graph) = self.graph {
            graph.set_permission(self.node_idx, perm);
        }
    }

    /// Returns true if this node is inert (never contributes opinions).
    pub fn is_inert(&self) -> bool {
        if let Some(ref graph) = self.graph {
            graph.is_inert(self.node_idx)
        } else {
            false
        }
    }

    /// Sets whether this node is inert.
    pub fn set_inert(&self, inert: bool) {
        if let Some(ref graph) = self.graph {
            graph.set_inert(self.node_idx, inert);
        }
    }

    /// Returns true if this node is culled.
    pub fn is_culled(&self) -> bool {
        if let Some(ref graph) = self.graph {
            graph.is_culled(self.node_idx)
        } else {
            false
        }
    }

    /// Sets whether this node is culled.
    pub fn set_culled(&self, culled: bool) {
        if let Some(ref graph) = self.graph {
            graph.set_culled(self.node_idx, culled);
        }
    }

    /// Returns true if this node is restricted due to permissions.
    pub fn is_restricted(&self) -> bool {
        if let Some(ref graph) = self.graph {
            graph.is_restricted(self.node_idx)
        } else {
            false
        }
    }

    /// Sets whether this node is restricted.
    pub fn set_restricted(&self, restricted: bool) {
        if let Some(ref graph) = self.graph {
            graph.set_restricted(self.node_idx, restricted);
        }
    }

    /// Returns true if this node can contribute specs for composition.
    ///
    /// Matches C++ `PcpNodeRef::CanContributeSpecs()`: the logic is
    ///     `!(inert || culled) && (!permissionDenied || isUsd)`
    /// In USD mode permissions are ignored, so a permission-denied node
    /// can still contribute specs.
    pub fn can_contribute_specs(&self) -> bool {
        if self.is_inert() || self.is_culled() {
            return false;
        }
        // In USD mode, skip restriction/permission check (no permissions).
        if let Some(ref graph) = self.graph {
            if graph.is_usd() {
                return true;
            }
        }
        !self.is_restricted()
    }

    /// Returns true if this node has specs to contribute.
    pub fn has_specs(&self) -> bool {
        if let Some(ref graph) = self.graph {
            graph.has_specs(self.node_idx)
        } else {
            false
        }
    }

    /// Sets whether this node has specs.
    pub fn set_has_specs(&self, has_specs: bool) {
        if let Some(ref graph) = self.graph {
            graph.set_has_specs(self.node_idx, has_specs);
        }
    }

    /// Returns true if this node has authored value clips.
    pub fn has_value_clips(&self) -> bool {
        if let Some(ref graph) = self.graph {
            graph.has_value_clips(self.node_idx)
        } else {
            false
        }
    }

    /// Sets whether this node has value clips.
    pub fn set_has_value_clips(&self, has_clips: bool) {
        if let Some(ref graph) = self.graph {
            graph.set_has_value_clips(self.node_idx, has_clips);
        }
    }

    /// Returns true if this node has a transitive direct dependency.
    ///
    /// A node has a transitive direct dependency if it was brought in via
    /// a direct (non-ancestral) arc or is in a subtree of such an arc.
    pub fn has_transitive_direct_dependency(&self) -> bool {
        if let Some(ref graph) = self.graph {
            graph.get_has_transitive_direct_arc(self.node_idx)
        } else {
            false
        }
    }

    /// Returns true if this node has a transitive ancestral dependency.
    pub fn has_transitive_ancestral_dependency(&self) -> bool {
        if let Some(ref graph) = self.graph {
            graph.get_has_transitive_ancestral_arc(self.node_idx)
        } else {
            false
        }
    }

    /// Sets whether this node has a transitive direct dependency.
    pub fn set_has_transitive_direct_dependency(&self, has_dep: bool) {
        if let Some(ref graph) = self.graph {
            graph.set_has_transitive_direct_arc(self.node_idx, has_dep);
        }
    }

    /// Sets whether this node has a transitive ancestral dependency.
    pub fn set_has_transitive_ancestral_dependency(&self, has_dep: bool) {
        if let Some(ref graph) = self.graph {
            graph.set_has_transitive_ancestral_arc(self.node_idx, has_dep);
        }
    }

    /// Returns the namespace depth of this node's path when it was restricted
    /// from contributing opinions for composition.
    ///
    /// If this node has no such restriction, returns 0.
    /// Note that unlike namespace_depth(), this value *does* include variant selections.
    pub fn spec_contribution_restricted_depth(&self) -> usize {
        if let Some(ref graph) = self.graph {
            graph.get_restriction_depth(self.node_idx)
        } else {
            0
        }
    }

    /// Sets this node's contribution restriction depth.
    ///
    /// Note that this function typically does not need to be called,
    /// since functions that restrict contributions (e.g., set_inert)
    /// automatically set the restriction depth appropriately.
    pub fn set_spec_contribution_restricted_depth(&self, depth: usize) {
        if let Some(ref graph) = self.graph {
            graph.set_restriction_depth(self.node_idx, depth);
        }
    }

    // ========================================================================
    // Children
    // ========================================================================

    /// Returns the indices of child nodes.
    pub fn children_indices(&self) -> Vec<usize> {
        if let Some(ref graph) = self.graph {
            graph.get_children_indices(self.node_idx)
        } else {
            Vec::new()
        }
    }

    /// Returns child nodes in strength order.
    pub fn children(&self) -> Vec<NodeRef> {
        self.children_indices()
            .into_iter()
            .filter_map(|idx| self.graph.as_ref().map(|g| NodeRef::new(g.clone(), idx)))
            .collect()
    }

    /// Returns an iterator range over child nodes in strongest to weakest order.
    pub fn children_range(&self) -> impl Iterator<Item = NodeRef> + '_ {
        self.children_indices()
            .into_iter()
            .filter_map(move |idx| self.graph.as_ref().map(|g| NodeRef::new(g.clone(), idx)))
    }

    /// Returns an iterator range over child nodes in weakest to strongest order.
    pub fn children_reverse_range(&self) -> impl DoubleEndedIterator<Item = NodeRef> + '_ {
        let mut children: Vec<NodeRef> = self.children();
        children.reverse();
        children.into_iter()
    }

    /// Returns the owning graph.
    pub fn owning_graph(&self) -> Option<Arc<PrimIndexGraph>> {
        self.graph.clone()
    }

    // ========================================================================
    // Node Modification
    // ========================================================================

    /// Inserts a child node at the given site with the specified arc.
    ///
    /// Returns the new child node, or invalid node on failure.
    pub fn insert_child(
        &self,
        site: &crate::Site,
        arc: &super::arc::Arc,
        layer_stack: Option<crate::LayerStackRefPtr>,
    ) -> NodeRef {
        if let Some(ref graph) = self.graph {
            let child_idx = graph.insert_child_node(self.node_idx, site.clone(), arc, layer_stack);
            if child_idx != INVALID_INDEX {
                NodeRef::new(graph.clone(), child_idx)
            } else {
                NodeRef::invalid()
            }
        } else {
            NodeRef::invalid()
        }
    }

    /// Inserts a subgraph as a child of this node.
    ///
    /// The root node of the subgraph will be connected to this node via arc.
    /// Returns the root node of the newly-added subgraph, or invalid node on failure.
    pub fn insert_child_subgraph(
        &self,
        subgraph: Arc<PrimIndexGraph>,
        arc: &super::arc::Arc,
        error: Option<&mut Vec<super::errors::ErrorType>>,
    ) -> NodeRef {
        if let Some(ref graph) = self.graph {
            // Call with Arc::clone to match signature: (self: &Arc<Self>, parent_node: &NodeRef, subgraph, arc, error)
            PrimIndexGraph::insert_child_subgraph(graph, self, subgraph, arc, error)
        } else {
            NodeRef::invalid()
        }
    }

    /// Sets the origin node index.
    pub fn set_origin_index(&self, origin_idx: usize) {
        if let Some(ref graph) = self.graph {
            graph.set_origin_index(self.node_idx, origin_idx);
        }
    }

    /// Sets the sibling number at origin.
    pub fn set_sibling_num_at_origin(&self, value: i32) {
        if let Some(ref graph) = self.graph {
            graph.set_sibling_num_at_origin(self.node_idx, value);
        }
    }

    /// Sets the namespace depth.
    pub fn set_namespace_depth(&self, value: i32) {
        if let Some(ref graph) = self.graph {
            graph.set_namespace_depth(self.node_idx, value);
        }
    }
}

impl PartialEq for NodeRef {
    fn eq(&self, other: &Self) -> bool {
        // Compare by graph address and node index
        let same_graph = match (&self.graph, &other.graph) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        same_graph && self.node_idx == other.node_idx
    }
}

impl Eq for NodeRef {}

impl Hash for NodeRef {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.unique_identifier().hash(state);
    }
}

impl std::fmt::Debug for NodeRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_valid() {
            write!(
                f,
                "NodeRef {{ idx: {}, path: {}, arc: {:?} }}",
                self.node_idx,
                self.path().as_str(),
                self.arc_type()
            )
        } else {
            write!(f, "NodeRef {{ invalid }}")
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Counts path elements excluding variant selections.
///
/// This is the Rust equivalent of C++ `PcpNode_GetNonVariantPathElementCount`.
/// Used to compute namespace depth for nodes whose paths may contain
/// variant selections (e.g., `/World{modelVariant=default}/Child`).
pub fn count_non_variant_path_elements(path: &Path) -> usize {
    let path_str = path.get_string();
    if path_str == "/" {
        return 0;
    }

    // Count slashes that aren't part of variant selections
    let mut count = 0;
    let mut in_variant = false;

    for ch in path_str.chars() {
        if ch == '{' {
            in_variant = true;
        } else if ch == '}' {
            in_variant = false;
        } else if ch == '/' && !in_variant {
            count += 1;
        }
    }

    count
}

/// Strips `depth_below_intro` non-variant path elements from the end of `path`.
///
/// Matches C++ `_GetPathAtIntroDepth()`: for each level to strip, skips any
/// leading variant-selection segments (which do not count as namespace depth),
/// then calls `GetParentPath()` once. This implements the fixed "proper depth
/// loop" that handles `depth > 1` correctly.
pub(crate) fn get_path_at_intro_depth(mut path: Path, mut depth_below_intro: i32) -> Path {
    while depth_below_intro > 0 {
        // Skip over variant-selection segments (they don't count as depth).
        while path.is_prim_variant_selection_path() {
            path = path.get_parent_path();
        }
        // Strip one real namespace level.
        path = path.get_parent_path();
        depth_below_intro -= 1;
    }
    path
}

/// A vector of node references.
pub type NodeRefVector = Vec<NodeRef>;

/// A hash set of node references.
pub type NodeRefHashSet = std::collections::HashSet<NodeRef>;

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_node() {
        let node = NodeRef::invalid();
        assert!(!node.is_valid());
        assert!(!node.is_root_node());
        assert!(node.path().is_empty());
    }

    #[test]
    fn test_node_equality() {
        let node1 = NodeRef::invalid();
        let node2 = NodeRef::invalid();
        assert_eq!(node1, node2);
    }

    #[test]
    fn test_permission_default() {
        assert_eq!(Permission::default(), Permission::Public);
    }

    #[test]
    fn test_count_non_variant_path_elements() {
        assert_eq!(count_non_variant_path_elements(&Path::absolute_root()), 0);
        assert_eq!(
            count_non_variant_path_elements(&Path::from_string("/World").unwrap()),
            1
        );
        assert_eq!(
            count_non_variant_path_elements(&Path::from_string("/World/Cube").unwrap()),
            2
        );
    }

    #[test]
    fn test_count_non_variant_with_variant_selections() {
        // Variant selections are enclosed in braces and should not be counted
        // as path elements. E.g., /World{variant=default}/Child has 2 non-variant
        // path elements: "World" and "Child".
        assert_eq!(
            count_non_variant_path_elements(
                &Path::from_string("/World{variant=default}/Child").unwrap()
            ),
            2
        );
        assert_eq!(
            count_non_variant_path_elements(&Path::from_string("/A{v=x}/B{w=y}/C").unwrap()),
            3
        );
    }

    // =========================================================================
    // Tests for path_at_introduction / depth_below_introduction
    // =========================================================================

    /// Root node has no parent so depth_below_introduction() must always be 0.
    /// C++ PcpNodeRef::GetDepthBelowIntroduction() returns 0 when parent is null.
    #[test]
    fn test_depth_below_introduction_root_is_zero() {
        use crate::{LayerStackIdentifier, PrimIndexGraph, Site};

        let root_site = Site::new(
            LayerStackIdentifier::new("test.usda"),
            Path::from_string("/World").unwrap(),
        );
        let graph = PrimIndexGraph::new(root_site, true);
        let root = graph.root_node();
        assert!(root.is_valid());
        assert!(!root.parent_node().is_valid());

        // Root has no parent -> depth_below_introduction = 0 (C++ parity)
        assert_eq!(root.depth_below_introduction(), 0);
    }

    /// Child node: parent path = /World (1 element), child ns_depth = 1 -> depth = 0.
    /// path_at_introduction = strip 0 levels from child path -> child path itself.
    #[test]
    fn test_path_at_introduction_child_depth_zero() {
        use crate::{
            Arc as PcpArc, ArcType, LayerStackIdentifier, MapFunction, PrimIndexGraph, Site,
        };

        // Root at /World
        let root_site = Site::new(
            LayerStackIdentifier::new("test.usda"),
            Path::from_string("/World").unwrap(),
        );
        let graph = PrimIndexGraph::new(root_site, true);
        let root = graph.root_node();
        root.set_namespace_depth(0);

        // Child at /Model, namespace_depth = 1 (introduced at parent's depth 1)
        let child_site = Site::new(
            LayerStackIdentifier::new("ref.usda"),
            Path::from_string("/Model").unwrap(),
        );
        let mut arc = PcpArc::new(ArcType::Reference);
        arc.set_parent_index(root.node_index());
        arc.set_origin_index(root.node_index());
        arc.set_namespace_depth(1);
        arc.set_map_to_parent(MapFunction::identity().clone());
        let child = root.insert_child(&child_site, &arc, None);
        assert!(child.is_valid());

        // parent.path = /World (1 element), child ns_depth = 1 -> depth_below = 0
        assert_eq!(count_non_variant_path_elements(&root.path()), 1);
        assert_eq!(child.namespace_depth(), 1);
        assert_eq!(child.depth_below_introduction(), 0);

        // path_at_introduction: strip 0 levels from /Model -> /Model
        let pai = child.path_at_introduction();
        assert_eq!(
            pai.get_string(),
            "/Model",
            "child introduced at same depth as parent has path_at_introduction = own path"
        );
    }

    /// Child node: parent path = /World (1 element), child ns_depth = 0
    /// -> depth_below_introduction = 1 - 0 = 1.
    /// path_at_introduction = strip 1 level from /Model -> /.
    #[test]
    fn test_path_at_introduction_depth_one() {
        use crate::{
            Arc as PcpArc, ArcType, LayerStackIdentifier, MapFunction, PrimIndexGraph, Site,
        };

        let root_site = Site::new(
            LayerStackIdentifier::new("test.usda"),
            Path::from_string("/World").unwrap(),
        );
        let graph = PrimIndexGraph::new(root_site, true);
        let root = graph.root_node();

        let child_site = Site::new(
            LayerStackIdentifier::new("ref.usda"),
            Path::from_string("/Model").unwrap(),
        );
        let mut arc = PcpArc::new(ArcType::Reference);
        arc.set_parent_index(root.node_index());
        arc.set_origin_index(root.node_index());
        arc.set_namespace_depth(0); // introduced at depth 0
        arc.set_map_to_parent(MapFunction::identity().clone());
        let child = root.insert_child(&child_site, &arc, None);
        assert!(child.is_valid());

        // parent.path = /World (1 element), ns_depth = 0 -> depth_below = 1
        assert_eq!(child.namespace_depth(), 0);
        assert_eq!(child.depth_below_introduction(), 1);

        // path_at_introduction: strip 1 level from /Model -> /
        let pai = child.path_at_introduction();
        assert_eq!(
            pai,
            Path::absolute_root(),
            "depth_below=1 must strip one level from child's path"
        );
    }

    /// Child node: parent path = /A/B (2 elements), child ns_depth = 1
    /// -> depth_below_introduction = 2 - 1 = 1.
    /// path_at_introduction of /A/B/C strips 1 -> /A/B.
    #[test]
    fn test_path_at_introduction_depth_greater_than_one() {
        use crate::{
            Arc as PcpArc, ArcType, LayerStackIdentifier, MapFunction, PrimIndexGraph, Site,
        };

        let root_site = Site::new(
            LayerStackIdentifier::new("test.usda"),
            Path::from_string("/A/B").unwrap(),
        );
        let graph = PrimIndexGraph::new(root_site, true);
        let root = graph.root_node();

        let child_site = Site::new(
            LayerStackIdentifier::new("ref.usda"),
            Path::from_string("/A/B/C").unwrap(),
        );
        let mut arc = PcpArc::new(ArcType::Reference);
        arc.set_parent_index(root.node_index());
        arc.set_origin_index(root.node_index());
        arc.set_namespace_depth(1);
        arc.set_map_to_parent(MapFunction::identity().clone());
        let child = root.insert_child(&child_site, &arc, None);
        assert!(child.is_valid());

        // parent.path = /A/B (2 elements), ns_depth = 1 -> depth_below = 1
        assert_eq!(count_non_variant_path_elements(&root.path()), 2);
        assert_eq!(child.namespace_depth(), 1);
        assert_eq!(child.depth_below_introduction(), 1);

        // path_at_introduction: strip 1 level from /A/B/C -> /A/B
        let pai = child.path_at_introduction();
        assert_eq!(
            pai.get_string(),
            "/A/B",
            "depth_below=1 on /A/B/C strips to /A/B"
        );
    }

    /// Variant selections in parent path are excluded from depth count.
    #[test]
    fn test_depth_below_introduction_excludes_variant_elements() {
        use crate::{
            Arc as PcpArc, ArcType, LayerStackIdentifier, MapFunction, PrimIndexGraph, Site,
        };

        // Parent at /World{v=default}/Mesh: non-variant count = 2
        let root_site = Site::new(
            LayerStackIdentifier::new("test.usda"),
            Path::from_string("/World{v=default}/Mesh").unwrap(),
        );
        let graph = PrimIndexGraph::new(root_site, true);
        let root = graph.root_node();

        let child_site = Site::new(
            LayerStackIdentifier::new("ref.usda"),
            Path::from_string("/Model").unwrap(),
        );
        let mut arc = PcpArc::new(ArcType::Reference);
        arc.set_parent_index(root.node_index());
        arc.set_origin_index(root.node_index());
        arc.set_namespace_depth(1); // introduced at /World level
        arc.set_map_to_parent(MapFunction::identity().clone());
        let child = root.insert_child(&child_site, &arc, None);
        assert!(child.is_valid());

        // parent path non-variant count = 2, ns_depth = 1 -> depth_below = 1
        assert_eq!(child.depth_below_introduction(), 1);
    }
}
