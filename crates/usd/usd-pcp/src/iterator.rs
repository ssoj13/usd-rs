//! PCP Iterators - traversal over composition graphs and property stacks.
//!
//! This module provides iterators for traversing:
//! - Nodes in a prim index graph (strong-to-weak and weak-to-strong)
//! - Prim specs in a prim index (strong-to-weak)
//! - Property specs in a property index (strong-to-weak)
//!
//! # C++ Parity
//!
//! This is a port of `pxr/usd/pcp/iterator.h` and `iterator.cpp`.

use crate::{CompressedSdSite, NodeRef, PrimIndex, PrimIndexGraph, PropertyIndex};
use std::sync::Arc;
use usd_sdf::Site as SdfSite;

// ============================================================================
// Node Iterator
// ============================================================================

/// Iterator over nodes in a prim index graph in strong-to-weak order.
///
/// This iterator traverses all nodes in the composition graph starting
/// from the root (strongest) and proceeding to leaf nodes (weakest).
#[derive(Clone)]
pub struct NodeIterator {
    graph: Arc<PrimIndexGraph>,
    current: usize,
    end: usize,
}

impl NodeIterator {
    /// Creates a new node iterator starting at the given index.
    pub fn new(graph: Arc<PrimIndexGraph>, start: usize, end: usize) -> Self {
        Self {
            graph,
            current: start,
            end,
        }
    }

    /// Creates an iterator over all nodes in the graph.
    pub fn all(graph: Arc<PrimIndexGraph>) -> Self {
        let count = graph.node_count();
        Self::new(graph, 0, count)
    }

    /// Returns a compressed Sd site for internal use.
    pub fn compressed_sd_site(&self, layer_index: usize) -> CompressedSdSite {
        CompressedSdSite::new(self.current, layer_index)
    }

    /// Moves iterator to the next subtree (skips current subtree).
    ///
    /// If the current node has a direct sibling, moves to that node.
    /// Otherwise, moves to the next sibling of the nearest ancestor
    /// with siblings. If no such node exists, iterator becomes end.
    pub fn move_to_next_subtree(&mut self) {
        let node = NodeRef::new(self.graph.clone(), self.current);
        if node.is_valid() {
            let (_, subtree_end) = self.graph.get_node_subtree_range(&node);
            self.current = subtree_end;
        }
    }

    /// Returns the current position.
    pub fn position(&self) -> usize {
        self.current
    }

    /// Returns the graph being iterated.
    pub fn graph(&self) -> &Arc<PrimIndexGraph> {
        &self.graph
    }
}

impl Iterator for NodeIterator {
    type Item = NodeRef;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current >= self.end {
            return None;
        }
        let node = NodeRef::new(self.graph.clone(), self.current);
        self.current += 1;
        Some(node)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.end.saturating_sub(self.current);
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for NodeIterator {}

impl DoubleEndedIterator for NodeIterator {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.current >= self.end {
            return None;
        }
        self.end -= 1;
        Some(NodeRef::new(self.graph.clone(), self.end))
    }
}

// ============================================================================
// Node Reverse Iterator
// ============================================================================

/// Iterator over nodes in weak-to-strong order.
///
/// This is simply a reversed NodeIterator.
pub type NodeReverseIterator = std::iter::Rev<NodeIterator>;

// ============================================================================
// Prim Iterator
// ============================================================================

/// Iterator over prim specs in a prim index in strong-to-weak order.
///
/// Each item is an SdfSite representing where a prim spec exists.
pub struct PrimIterator<'a> {
    prim_index: &'a PrimIndex,
    current: usize,
    end: usize,
}

impl<'a> PrimIterator<'a> {
    /// Creates a new prim iterator.
    pub fn new(prim_index: &'a PrimIndex, start: usize) -> Self {
        let end = prim_index.prim_stack_len();
        Self {
            prim_index,
            current: start,
            end,
        }
    }

    /// Returns the node from which the current prim originated.
    pub fn current_node(&self) -> Option<NodeRef> {
        if self.current < self.end {
            self.prim_index.get_node_at_prim_stack_index(self.current)
        } else {
            None
        }
    }

    /// Returns the compressed site ref at current position.
    pub fn compressed_site(&self) -> Option<CompressedSdSite> {
        if self.current < self.end {
            self.prim_index.get_compressed_site_at_index(self.current)
        } else {
            None
        }
    }
}

impl<'a> Iterator for PrimIterator<'a> {
    type Item = SdfSite;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current >= self.end {
            return None;
        }
        let site = self.prim_index.get_site_at_prim_stack_index(self.current);
        self.current += 1;
        site
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.end.saturating_sub(self.current);
        (remaining, Some(remaining))
    }
}

impl<'a> ExactSizeIterator for PrimIterator<'a> {}

impl<'a> DoubleEndedIterator for PrimIterator<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.current >= self.end {
            return None;
        }
        self.end -= 1;
        self.prim_index.get_site_at_prim_stack_index(self.end)
    }
}

// ============================================================================
// Prim Reverse Iterator
// ============================================================================

/// Iterator over prim specs in weak-to-strong order.
///
/// Wraps `Rev<PrimIterator>` and provides additional methods matching
/// C++ `PcpPrimReverseIterator` (GetNode, _GetSiteRef).
pub struct PrimReverseIterator<'a> {
    inner: std::iter::Rev<PrimIterator<'a>>,
    /// Tracks the "base" position for GetNode/_GetSiteRef (like C++ base()-1).
    prim_index: &'a PrimIndex,
    /// Current position tracking (end-relative). Updated on each next().
    current_pos: Option<usize>,
}

impl<'a> PrimReverseIterator<'a> {
    /// Creates a new reverse prim iterator from a forward iterator.
    pub fn new(fwd: PrimIterator<'a>) -> Self {
        let prim_index = fwd.prim_index;
        let end = fwd.end;
        Self {
            inner: fwd.rev(),
            prim_index,
            current_pos: if end > 0 { Some(end - 1) } else { None },
        }
    }

    /// Returns the node from which the current prim originated.
    ///
    /// Matches C++ `PcpPrimReverseIterator::GetNode()`.
    pub fn current_node(&self) -> Option<NodeRef> {
        self.current_pos
            .and_then(|pos| self.prim_index.get_node_at_prim_stack_index(pos))
    }

    /// Returns the compressed site ref at the current position.
    ///
    /// Matches C++ `PcpPrimReverseIterator::_GetSiteRef()`.
    pub fn compressed_site(&self) -> Option<CompressedSdSite> {
        self.current_pos
            .and_then(|pos| self.prim_index.get_compressed_site_at_index(pos))
    }
}

impl<'a> Iterator for PrimReverseIterator<'a> {
    type Item = SdfSite;

    fn next(&mut self) -> Option<Self::Item> {
        let result = self.inner.next();
        // Update current_pos: the Rev iterator walks backwards from end
        if result.is_some() {
            self.current_pos = self.current_pos.and_then(|p| p.checked_sub(1));
        }
        result
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<'a> ExactSizeIterator for PrimReverseIterator<'a> {}

// ============================================================================
// Property Iterator
// ============================================================================

/// Iterator over property specs in a property index in strong-to-weak order.
pub struct PropertyIterator<'a> {
    property_index: &'a PropertyIndex,
    current: usize,
    end: usize,
}

impl<'a> PropertyIterator<'a> {
    /// Creates a new property iterator.
    pub fn new(property_index: &'a PropertyIndex, start: usize) -> Self {
        let end = property_index.len();
        Self {
            property_index,
            current: start,
            end,
        }
    }

    /// Returns the node from which the current property originated.
    pub fn current_node(&self) -> Option<NodeRef> {
        if self.current < self.end {
            let stack = self.property_index.property_stack();
            Some(stack[self.current].originating_node.clone())
        } else {
            None
        }
    }

    /// Returns true if current property is local to the owning layer stack.
    pub fn is_local(&self) -> bool {
        self.current < self.property_index.num_local_specs()
    }
}

impl<'a> Iterator for PropertyIterator<'a> {
    type Item = &'a crate::PropertyInfo;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current >= self.end {
            return None;
        }
        let stack = self.property_index.property_stack();
        let info = &stack[self.current];
        self.current += 1;
        Some(info)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.end.saturating_sub(self.current);
        (remaining, Some(remaining))
    }
}

impl<'a> ExactSizeIterator for PropertyIterator<'a> {}

impl<'a> DoubleEndedIterator for PropertyIterator<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.current >= self.end {
            return None;
        }
        self.end -= 1;
        let stack = self.property_index.property_stack();
        Some(&stack[self.end])
    }
}

// ============================================================================
// Property Reverse Iterator
// ============================================================================

/// Iterator over property specs in weak-to-strong order.
///
/// Wraps `Rev<PropertyIterator>` and provides additional methods matching
/// C++ `PcpPropertyReverseIterator` (GetNode, IsLocal).
pub struct PropertyReverseIterator<'a> {
    inner: std::iter::Rev<PropertyIterator<'a>>,
    /// Reference to the property index for GetNode/IsLocal.
    property_index: &'a PropertyIndex,
    /// Current position tracking (end-relative).
    current_pos: Option<usize>,
}

impl<'a> PropertyReverseIterator<'a> {
    /// Creates a new reverse property iterator from a forward iterator.
    pub fn new(fwd: PropertyIterator<'a>) -> Self {
        let property_index = fwd.property_index;
        let end = fwd.end;
        Self {
            inner: fwd.rev(),
            property_index,
            current_pos: if end > 0 { Some(end - 1) } else { None },
        }
    }

    /// Returns the node from which the current property originated.
    ///
    /// Matches C++ `PcpPropertyReverseIterator::GetNode()`.
    pub fn current_node(&self) -> Option<NodeRef> {
        self.current_pos.map(|pos| {
            let stack = self.property_index.property_stack();
            stack[pos].originating_node.clone()
        })
    }

    /// Returns true if the current property is local to the owning layer stack.
    ///
    /// Matches C++ `PcpPropertyReverseIterator::IsLocal()`.
    pub fn is_local(&self) -> bool {
        self.current_pos
            .map(|pos| pos < self.property_index.num_local_specs())
            .unwrap_or(false)
    }
}

impl<'a> Iterator for PropertyReverseIterator<'a> {
    type Item = &'a crate::PropertyInfo;

    fn next(&mut self) -> Option<Self::Item> {
        let result = self.inner.next();
        if result.is_some() {
            self.current_pos = self.current_pos.and_then(|p| p.checked_sub(1));
        }
        result
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<'a> ExactSizeIterator for PropertyReverseIterator<'a> {}

// ============================================================================
// Range Types
// ============================================================================

/// A range of nodes (start, end iterators).
pub type NodeRange = (NodeIterator, NodeIterator);

/// A range of prim specs.
pub type PrimRange<'a> = (PrimIterator<'a>, PrimIterator<'a>);

/// A range of property specs.
pub type PropertyRange<'a> = (PropertyIterator<'a>, PropertyIterator<'a>);

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_iterator_empty() {
        // Test with empty graph requires PrimIndexGraph construction
        // which needs more infrastructure - skip for now
    }

    #[test]
    fn test_property_iterator_empty() {
        let index = PropertyIndex::new();
        let iter = PropertyIterator::new(&index, 0);
        assert_eq!(iter.count(), 0);
    }

    #[test]
    fn test_iterator_traits() {
        // Verify iterator traits are properly implemented
        fn assert_iterator<T: Iterator>() {}
        fn assert_double_ended<T: DoubleEndedIterator>() {}
        fn assert_exact_size<T: ExactSizeIterator>() {}

        assert_iterator::<NodeIterator>();
        assert_double_ended::<NodeIterator>();
        assert_exact_size::<NodeIterator>();

        assert_iterator::<PropertyIterator>();
        assert_double_ended::<PropertyIterator>();
        assert_exact_size::<PropertyIterator>();
    }
}
