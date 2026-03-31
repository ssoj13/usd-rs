//! PCP Arc - represents an arc connecting two nodes in the prim index.
//!
//! An arc represents a composition relationship between two nodes,
//! such as a reference, inherit, or payload arc.
//!
//! # C++ Parity
//!
//! This is a port of `pxr/usd/pcp/arc.h` (~65 lines).

use crate::{ArcType, MapExpression, MapFunction};

// Forward declaration - will be implemented in node.rs
// For now we use indices instead of full PcpNodeRef to avoid circular dependency

/// Index type for referencing nodes in a graph.
/// Using usize for direct indexing into node vectors.
pub type NodeIndex = usize;

/// Invalid node index constant.
pub const INVALID_NODE_INDEX: NodeIndex = usize::MAX;

/// Represents an arc connecting two nodes in the prim index.
///
/// The arc is owned by a node (source) and points to its parent node (target)
/// in the index. It contains information about the type of composition arc,
/// the mapping function used to translate paths/values, and strength ordering.
///
/// # Examples
///
/// ```rust,ignore
/// use usd_pcp::{Arc, ArcType};
///
/// let arc = Arc::new(ArcType::Reference);
/// assert_eq!(arc.arc_type(), ArcType::Reference);
/// ```
#[derive(Clone, Debug)]
pub struct Arc {
    /// The type of this arc.
    arc_type: ArcType,

    /// The parent (or target) node index of this arc.
    /// If this arc's source node is a root node (type == Root),
    /// this will be INVALID_NODE_INDEX.
    parent_index: NodeIndex,

    /// The origin node index of this arc.
    /// This is the node that caused this arc's source node to be brought into
    /// the prim index. In most cases, this will be the same as the parent node.
    /// For implied inherits, this is the node from which this inherit arc was
    /// propagated. This affects strength ordering.
    origin_index: NodeIndex,

    /// The value-mapping expression from this arc's source node to its parent.
    ///
    /// Matches C++ `PcpArc::mapToParent` which is `PcpMapExpression`.
    /// Using `MapExpression` (not `MapFunction`) enables lazy evaluation
    /// and supports `AddRootIdentity` for class-based arcs (inherit/specialize)
    /// which need root identity mapping for implied class propagation.
    map_to_parent: MapExpression,

    /// Index among sibling arcs at origin; lower is stronger.
    sibling_num_at_origin: i32,

    /// Absolute depth in namespace of node that introduced this node.
    /// Note that this does *not* count any variant selections.
    namespace_depth: i32,
}

impl Default for Arc {
    fn default() -> Self {
        Self {
            arc_type: ArcType::Root,
            parent_index: INVALID_NODE_INDEX,
            origin_index: INVALID_NODE_INDEX,
            map_to_parent: MapExpression::null(),
            sibling_num_at_origin: 0,
            namespace_depth: 0,
        }
    }
}

impl Arc {
    /// Creates a new arc with the given type.
    pub fn new(arc_type: ArcType) -> Self {
        Self {
            arc_type,
            ..Default::default()
        }
    }

    /// Creates a root arc (no parent).
    pub fn root() -> Self {
        Self::new(ArcType::Root)
    }

    /// Returns the type of this arc.
    #[inline]
    pub fn arc_type(&self) -> ArcType {
        self.arc_type
    }

    /// Returns the parent (target) node index.
    #[inline]
    pub fn parent_index(&self) -> NodeIndex {
        self.parent_index
    }

    /// Returns the origin node index.
    #[inline]
    pub fn origin_index(&self) -> NodeIndex {
        self.origin_index
    }

    /// Returns the map expression to parent.
    #[inline]
    pub fn map_to_parent(&self) -> &MapExpression {
        &self.map_to_parent
    }

    /// Returns the sibling number at origin.
    /// Lower numbers are stronger.
    #[inline]
    pub fn sibling_num_at_origin(&self) -> i32 {
        self.sibling_num_at_origin
    }

    /// Returns the namespace depth.
    #[inline]
    pub fn namespace_depth(&self) -> i32 {
        self.namespace_depth
    }

    /// Sets the arc type.
    pub fn set_arc_type(&mut self, arc_type: ArcType) {
        self.arc_type = arc_type;
    }

    /// Sets the parent node index.
    pub fn set_parent_index(&mut self, index: NodeIndex) {
        self.parent_index = index;
    }

    /// Sets the origin node index.
    pub fn set_origin_index(&mut self, index: NodeIndex) {
        self.origin_index = index;
    }

    /// Sets the map expression to parent from a `MapFunction`.
    ///
    /// Wraps the function in `MapExpression::constant()`. Use this for
    /// simple arcs (variant, sublayer, relocate) that don't need
    /// expression-level operations like `add_root_identity`.
    pub fn set_map_to_parent(&mut self, map: MapFunction) {
        self.map_to_parent = MapExpression::constant(map);
    }

    /// Sets the map expression to parent directly.
    ///
    /// Use this for class-based arcs (inherit/specialize) where the
    /// expression needs `add_root_identity()`, or for internal
    /// references/payloads where root identity is added post-insert.
    pub fn set_map_to_parent_expr(&mut self, expr: MapExpression) {
        self.map_to_parent = expr;
    }

    /// Sets the sibling number at origin.
    pub fn set_sibling_num_at_origin(&mut self, num: i32) {
        self.sibling_num_at_origin = num;
    }

    /// Sets the namespace depth.
    pub fn set_namespace_depth(&mut self, depth: i32) {
        self.namespace_depth = depth;
    }

    /// Returns true if this is a root arc.
    #[inline]
    pub fn is_root(&self) -> bool {
        self.arc_type == ArcType::Root
    }

    /// Returns true if this arc has a valid parent.
    #[inline]
    pub fn has_parent(&self) -> bool {
        self.parent_index != INVALID_NODE_INDEX
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arc_default() {
        let arc = Arc::default();
        assert_eq!(arc.arc_type(), ArcType::Root);
        assert_eq!(arc.parent_index(), INVALID_NODE_INDEX);
        assert_eq!(arc.origin_index(), INVALID_NODE_INDEX);
        assert_eq!(arc.sibling_num_at_origin(), 0);
        assert_eq!(arc.namespace_depth(), 0);
        assert!(arc.is_root());
        assert!(!arc.has_parent());
    }

    #[test]
    fn test_arc_new() {
        let arc = Arc::new(ArcType::Reference);
        assert_eq!(arc.arc_type(), ArcType::Reference);
        assert!(!arc.is_root());
    }

    #[test]
    fn test_arc_root() {
        let arc = Arc::root();
        assert!(arc.is_root());
        assert_eq!(arc.arc_type(), ArcType::Root);
    }

    #[test]
    fn test_arc_setters() {
        let mut arc = Arc::new(ArcType::Reference);

        arc.set_parent_index(0);
        assert_eq!(arc.parent_index(), 0);
        assert!(arc.has_parent());

        arc.set_origin_index(1);
        assert_eq!(arc.origin_index(), 1);

        arc.set_sibling_num_at_origin(5);
        assert_eq!(arc.sibling_num_at_origin(), 5);

        arc.set_namespace_depth(3);
        assert_eq!(arc.namespace_depth(), 3);

        arc.set_arc_type(ArcType::Payload);
        assert_eq!(arc.arc_type(), ArcType::Payload);
    }

    #[test]
    fn test_arc_map_expression() {
        let mut arc = Arc::new(ArcType::Reference);
        assert!(arc.map_to_parent().is_null());

        let identity = MapFunction::identity().clone();
        arc.set_map_to_parent(identity);
        assert!(arc.map_to_parent().evaluate().is_identity());
    }
}
