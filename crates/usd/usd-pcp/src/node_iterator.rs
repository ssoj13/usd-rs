//! Node Iterator - iterators for traversing nodes in the prim index graph.
//!
//! Port of pxr/usd/pcp/node_Iterator.h
//!
//! These classes exist because we want to optimize the iteration of a
//! node's children while not exposing the PrimIndexGraph implementation
//! detail outside of Pcp.

use std::sync::Arc;

use crate::{INVALID_INDEX, NodeRef};

use super::prim_index_graph::PrimIndexGraph;

/// Object used to iterate over child nodes (not all descendant nodes) of a
/// node in the prim index graph in strong-to-weak order.
///
/// Matches C++ `PcpNodeRef_PrivateChildrenConstIterator`.
pub struct NodeRefPrivateChildrenConstIterator {
    /// Current node this iterator is pointing at.
    node: NodeRef,
    /// Reference to the graph for safe node access.
    graph: Option<Arc<PrimIndexGraph>>,
}

impl NodeRefPrivateChildrenConstIterator {
    /// Constructs an iterator pointing to node's first or past its last child.
    ///
    /// Matches C++ constructor:
    /// ```cpp
    /// PcpNodeRef_PrivateChildrenConstIterator(const PcpNodeRef& node, bool end = false)
    /// ```
    pub fn new(node: NodeRef, end: bool) -> Self {
        let graph = node.owning_graph();

        let node_idx = if end {
            INVALID_INDEX
        } else {
            // Get first child index from the node
            if let Some(ref g) = graph {
                let nodes = g.nodes();
                if let Some(graph_node) = nodes.get(node.node_index()) {
                    graph_node.first_child_index
                } else {
                    INVALID_INDEX
                }
            } else {
                INVALID_INDEX
            }
        };

        // Create a new NodeRef with the child index
        let iter_node = if node_idx != INVALID_INDEX {
            if let Some(ref g) = graph {
                NodeRef::new(g.clone(), node_idx)
            } else {
                NodeRef::invalid()
            }
        } else {
            NodeRef::invalid()
        };

        Self {
            node: iter_node,
            graph,
        }
    }

    fn increment(&mut self) {
        if self.node.node_index() == INVALID_INDEX {
            return;
        }

        // Get next sibling index from graph
        let next_idx = if let Some(ref graph) = self.graph {
            let nodes = graph.nodes();
            nodes
                .get(self.node.node_index())
                .map(|n| n.next_sibling_index)
                .unwrap_or(INVALID_INDEX)
        } else {
            INVALID_INDEX
        };

        if next_idx != INVALID_INDEX {
            if let Some(ref graph) = self.graph {
                self.node = NodeRef::new(graph.clone(), next_idx);
            } else {
                self.node = NodeRef::invalid();
            }
        } else {
            self.node = NodeRef::invalid();
        }
    }

    fn equal(&self, other: &Self) -> bool {
        self.node == other.node
    }

    /// C++ iterator pattern - Rust uses Iterator::next() instead.
    #[allow(dead_code)]
    fn dereference(&self) -> &NodeRef {
        &self.node
    }
}

impl Iterator for NodeRefPrivateChildrenConstIterator {
    type Item = NodeRef;

    fn next(&mut self) -> Option<Self::Item> {
        if self.node.node_index() == INVALID_INDEX {
            return None;
        }

        let result = self.node.clone();
        self.increment();
        Some(result)
    }
}

impl PartialEq for NodeRefPrivateChildrenConstIterator {
    fn eq(&self, other: &Self) -> bool {
        self.equal(other)
    }
}

impl Eq for NodeRefPrivateChildrenConstIterator {}

/// Object used to iterate over child nodes (not all descendant nodes) of a
/// node in the prim index graph in weak-to-strong order.
///
/// Matches C++ `PcpNodeRef_PrivateChildrenConstReverseIterator`.
pub struct NodeRefPrivateChildrenConstReverseIterator {
    /// Current node this iterator is pointing at.
    node: NodeRef,
    /// Reference to the graph for safe node access.
    graph: Option<Arc<PrimIndexGraph>>,
}

impl NodeRefPrivateChildrenConstReverseIterator {
    /// Constructs an iterator pointing to node's first or past its last child.
    ///
    /// Matches C++ constructor:
    /// ```cpp
    /// PcpNodeRef_PrivateChildrenConstReverseIterator(const PcpNodeRef& node, bool end = false)
    /// ```
    pub fn new(node: NodeRef, end: bool) -> Self {
        let graph = node.owning_graph();

        let node_idx = if end {
            INVALID_INDEX
        } else {
            // Get last child index from the node
            if let Some(ref g) = graph {
                let nodes = g.nodes();
                if let Some(graph_node) = nodes.get(node.node_index()) {
                    graph_node.last_child_index
                } else {
                    INVALID_INDEX
                }
            } else {
                INVALID_INDEX
            }
        };

        // Create a new NodeRef with the child index
        let iter_node = if node_idx != INVALID_INDEX {
            if let Some(ref g) = graph {
                NodeRef::new(g.clone(), node_idx)
            } else {
                NodeRef::invalid()
            }
        } else {
            NodeRef::invalid()
        };

        Self {
            node: iter_node,
            graph,
        }
    }

    fn increment(&mut self) {
        if self.node.node_index() == INVALID_INDEX {
            return;
        }

        // Get previous sibling index from graph
        let prev_idx = if let Some(ref graph) = self.graph {
            let nodes = graph.nodes();
            nodes
                .get(self.node.node_index())
                .map(|n| n.prev_sibling_index)
                .unwrap_or(INVALID_INDEX)
        } else {
            INVALID_INDEX
        };

        if prev_idx != INVALID_INDEX {
            if let Some(ref graph) = self.graph {
                self.node = NodeRef::new(graph.clone(), prev_idx);
            } else {
                self.node = NodeRef::invalid();
            }
        } else {
            self.node = NodeRef::invalid();
        }
    }

    fn equal(&self, other: &Self) -> bool {
        self.node == other.node
    }

    /// C++ iterator pattern - Rust uses Iterator::next() instead.
    #[allow(dead_code)]
    fn dereference(&self) -> &NodeRef {
        &self.node
    }
}

impl Iterator for NodeRefPrivateChildrenConstReverseIterator {
    type Item = NodeRef;

    fn next(&mut self) -> Option<Self::Item> {
        if self.node.node_index() == INVALID_INDEX {
            return None;
        }

        let result = self.node.clone();
        self.increment();
        Some(result)
    }
}

impl PartialEq for NodeRefPrivateChildrenConstReverseIterator {
    fn eq(&self, other: &Self) -> bool {
        self.equal(other)
    }
}

impl Eq for NodeRefPrivateChildrenConstReverseIterator {}

/// Wrapper type for range-based iteration.
///
/// Matches C++ `PcpNodeRef_PrivateChildrenConstRange`.
pub struct NodeRefPrivateChildrenConstRange {
    node: NodeRef,
}

impl NodeRefPrivateChildrenConstRange {
    /// Creates a new range for the given node.
    pub fn new(node: NodeRef) -> Self {
        Self { node }
    }

    /// Returns an iterator over children in strong-to-weak order.
    pub fn iter(&self) -> NodeRefPrivateChildrenConstIterator {
        NodeRefPrivateChildrenConstIterator::new(self.node.clone(), false)
    }

    /// Returns an iterator over children in weak-to-strong order.
    pub fn iter_rev(&self) -> NodeRefPrivateChildrenConstReverseIterator {
        NodeRefPrivateChildrenConstReverseIterator::new(self.node.clone(), false)
    }
}

impl IntoIterator for NodeRefPrivateChildrenConstRange {
    type Item = NodeRef;
    type IntoIter = NodeRefPrivateChildrenConstIterator;

    fn into_iter(self) -> Self::IntoIter {
        NodeRefPrivateChildrenConstIterator::new(self.node, false)
    }
}

/// Return node range for children of the given node.
///
/// Matches C++ `Pcp_GetChildrenRange(const PcpNodeRef& node)`.
pub fn get_children_range(node: &NodeRef) -> NodeRefPrivateChildrenConstRange {
    NodeRefPrivateChildrenConstRange::new(node.clone())
}

/// Return all of a node's children, strong-to-weak.
///
/// Matches C++ `Pcp_GetChildren(const PcpNodeRef& node)`.
pub fn get_children(node: &NodeRef) -> Vec<NodeRef> {
    get_children_range(node).iter().collect()
}

/// Object used to iterate over all nodes in a subtree rooted at a
/// given node in the prim index graph in strong-to-weak order.
///
/// Matches C++ `PcpNodeRef_PrivateSubtreeConstIterator`.
pub struct NodeRefPrivateSubtreeConstIterator {
    /// Current node this iterator is pointing at.
    node: NodeRef,
    /// Reference to the graph for safe node access.
    graph: Option<Arc<PrimIndexGraph>>,
    /// Whether to prune children on next increment.
    prune_children: bool,
}

impl NodeRefPrivateSubtreeConstIterator {
    /// Creates an iterator representing the beginning or end of the subtree.
    ///
    /// Matches C++ constructor:
    /// ```cpp
    /// PcpNodeRef_PrivateSubtreeConstIterator(const PcpNodeRef& node, bool end)
    /// ```
    pub fn new(node: NodeRef, end: bool) -> Self {
        let graph = node.owning_graph();
        let iter_node = node.clone();

        let mut iter = Self {
            node: iter_node,
            graph,
            prune_children: false,
        };

        if end {
            iter.move_to_next();
        }

        iter
    }

    /// Causes the next increment of this iterator to ignore
    /// descendants of the current node.
    ///
    /// Matches C++ `PruneChildren()` method.
    pub fn prune_children(&mut self) {
        self.prune_children = true;
    }

    fn move_to_first_child(&mut self) -> bool {
        if self.node.node_index() == INVALID_INDEX {
            return false;
        }

        // Get first child index from graph
        let first_child_idx = if let Some(ref graph) = self.graph {
            let nodes = graph.nodes();
            nodes
                .get(self.node.node_index())
                .map(|n| n.first_child_index)
                .unwrap_or(INVALID_INDEX)
        } else {
            INVALID_INDEX
        };

        if first_child_idx != INVALID_INDEX {
            if let Some(ref graph) = self.graph {
                self.node = NodeRef::new(graph.clone(), first_child_idx);
                return true;
            }
        }

        false
    }

    fn move_to_next(&mut self) {
        if self.node.node_index() == INVALID_INDEX {
            return;
        }

        let mut cur_idx = self.node.node_index();

        while cur_idx != INVALID_INDEX {
            // Get next sibling and parent indices
            let (next_sibling, parent) = if let Some(ref graph) = self.graph {
                let nodes = graph.nodes();
                nodes
                    .get(cur_idx)
                    .map(|n| (n.next_sibling_index, n.parent_index))
                    .unwrap_or((INVALID_INDEX, INVALID_INDEX))
            } else {
                (INVALID_INDEX, INVALID_INDEX)
            };

            // See if we can move to the current node's next sibling.
            if next_sibling != INVALID_INDEX {
                cur_idx = next_sibling;
                break;
            }

            // If we can't, move to the current node's parent and try again.
            cur_idx = parent;
        }

        if cur_idx != INVALID_INDEX {
            if let Some(ref graph) = self.graph {
                self.node = NodeRef::new(graph.clone(), cur_idx);
            } else {
                self.node = NodeRef::invalid();
            }
        } else {
            self.node = NodeRef::invalid();
        }
    }
}

impl Iterator for NodeRefPrivateSubtreeConstIterator {
    type Item = NodeRef;

    fn next(&mut self) -> Option<Self::Item> {
        if self.node.node_index() == INVALID_INDEX {
            return None;
        }

        let result = self.node.clone();

        if self.prune_children || !self.move_to_first_child() {
            self.move_to_next();
        }
        self.prune_children = false;

        Some(result)
    }
}

/// Wrapper type for range-based for loops.
///
/// Matches C++ `PcpNodeRef_PrivateSubtreeConstRange`.
pub struct NodeRefPrivateSubtreeConstRange {
    begin: NodeRefPrivateSubtreeConstIterator,
    /// End iterator kept for C++ parity. Rust iterator uses internal termination.
    _end: NodeRefPrivateSubtreeConstIterator,
}

impl NodeRefPrivateSubtreeConstRange {
    /// Creates a new range for the subtree rooted at the given node.
    pub fn new(node: NodeRef) -> Self {
        Self {
            begin: NodeRefPrivateSubtreeConstIterator::new(node.clone(), false),
            _end: NodeRefPrivateSubtreeConstIterator::new(node, true),
        }
    }
}

impl IntoIterator for NodeRefPrivateSubtreeConstRange {
    type Item = NodeRef;
    type IntoIter = NodeRefPrivateSubtreeConstIterator;

    fn into_iter(self) -> Self::IntoIter {
        self.begin
    }
}

/// Return node range for subtree rooted at the given node.
///
/// Matches C++ `Pcp_GetSubtreeRange(const PcpNodeRef& node)`.
pub fn get_subtree_range(node: &NodeRef) -> NodeRefPrivateSubtreeConstRange {
    NodeRefPrivateSubtreeConstRange::new(node.clone())
}
