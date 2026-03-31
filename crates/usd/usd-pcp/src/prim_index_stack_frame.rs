//! Prim Index Stack Frame - internal helper for tracking recursive prim indexing.
//!
//! Port of pxr/usd/pcp/primIndex_StackFrame.h
//!
//! This module provides internal helper classes for tracking recursive invocations
//! of the prim indexing algorithm.

use crate::{Arc as PcpArc, NodeRef, Site};

use super::prim_index::PrimIndex;

/// Internal helper class for tracking recursive invocations of
/// the prim indexing algorithm.
///
/// Matches C++ `PcpPrimIndex_StackFrame`.
#[derive(Debug, Clone)]
pub struct PrimIndexStackFrame {
    /// Link to the previous recursive invocation.
    pub previous_frame: Option<Box<PrimIndexStackFrame>>,
    /// The site of the prim index being built by this recursive call.
    pub requested_site: Site,
    /// The node in the parent graph that will be the parent of the prim index
    /// being built by this recursive call.
    pub parent_node: NodeRef,
    /// The arc connecting the prim index being built by this recursive
    /// call to the parent node in the previous stack frame.
    pub arc_to_parent: PcpArc,
    /// The outer-most index whose computation originated this recursive chain.
    /// This is meant for debugging support.
    pub originating_index: Option<*const PrimIndex>,
    /// Whether the prim index being built by this recursive call should
    /// skip adding nodes if another node exists with the same site.
    pub skip_duplicate_nodes: bool,
}

impl PrimIndexStackFrame {
    /// Creates a new stack frame.
    ///
    /// Matches C++ constructor:
    /// ```cpp
    /// PcpPrimIndex_StackFrame(
    ///     PcpLayerStackSite const &requestedSite,
    ///     PcpNodeRef const &parentNode,
    ///     PcpArc *arcToParent,
    ///     PcpPrimIndex_StackFrame *previousFrame,
    ///     PcpPrimIndex const *originatingIndex,
    ///     bool skipDuplicateNodes)
    /// ```
    pub fn new(
        requested_site: Site,
        parent_node: NodeRef,
        arc_to_parent: PcpArc,
        previous_frame: Option<Box<PrimIndexStackFrame>>,
        originating_index: Option<*const PrimIndex>,
        skip_duplicate_nodes: bool,
    ) -> Self {
        Self {
            previous_frame,
            requested_site,
            parent_node,
            arc_to_parent,
            originating_index,
            skip_duplicate_nodes,
        }
    }
}

/// Iterator for walking up a node's ancestors while potentially crossing
/// stack frames.
///
/// Matches C++ `PcpPrimIndex_StackFrameIterator`.
pub struct PrimIndexStackFrameIterator {
    /// Current node being iterated.
    pub node: NodeRef,
    /// Previous stack frame.
    pub previous_frame: Option<Box<PrimIndexStackFrame>>,
}

impl PrimIndexStackFrameIterator {
    /// Creates a new iterator.
    ///
    /// Matches C++ constructor:
    /// ```cpp
    /// PcpPrimIndex_StackFrameIterator(
    ///     const PcpNodeRef& n, PcpPrimIndex_StackFrame* f)
    /// ```
    pub fn new(node: NodeRef, previous_frame: Option<Box<PrimIndexStackFrame>>) -> Self {
        Self {
            node,
            previous_frame,
        }
    }

    /// Step to the next parent node.
    ///
    /// Matches C++ `Next()` method.
    pub fn next(&mut self) {
        use crate::ArcType;

        if self.node.arc_type() != ArcType::Root {
            // Step to the next parent within this graph.
            let parent = self.node.parent_node();
            if parent.is_valid() {
                self.node = parent;
            } else {
                self.node = NodeRef::invalid();
            }
        } else if let Some(ref mut frame) = self.previous_frame {
            // No more parents in this graph, but there is an outer
            // prim index that this node will become part of.
            // Step to the (eventual) parent in that graph.
            self.node = frame.parent_node.clone();
            // Note: In C++, previous_frame is a pointer, so we can't clone it directly.
            // We need to take ownership or use a different approach.
            // For now, we'll just take the frame and extract what we need.
            let parent_frame = frame.previous_frame.take();
            self.previous_frame = parent_frame;
        } else {
            // No more parents.
            self.node = NodeRef::invalid();
        }
    }

    /// Step to the first parent node in the next recursive call.
    ///
    /// Matches C++ `NextFrame()` method.
    pub fn next_frame(&mut self) {
        if let Some(frame) = self.previous_frame.take() {
            self.node = frame.parent_node;
            self.previous_frame = frame.previous_frame;
        } else {
            self.node = NodeRef::invalid();
        }
    }

    /// Get the type of arc connecting the current node with its parent.
    ///
    /// Matches C++ `GetArcType()` method.
    pub fn arc_type(&self) -> crate::ArcType {
        use crate::ArcType;

        if self.node.arc_type() != ArcType::Root {
            // Use the current node's arc type.
            self.node.arc_type()
        } else if let Some(ref frame) = self.previous_frame {
            // No more parents in this graph, but there is an outer
            // prim index, so consult arcToParent.
            frame.arc_to_parent.arc_type()
        } else {
            // No more parents; this must be the absolute final root.
            ArcType::Root
        }
    }
}
