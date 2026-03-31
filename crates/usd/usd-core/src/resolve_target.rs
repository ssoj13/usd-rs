//! Resolve Target - defines a subrange of nodes and layers for value resolution.
//!
//! Port of pxr/usd/usd/resolveTarget.h
//!
//! UsdResolveTarget defines a subrange of nodes and layers within a prim's prim index
//! to consider when performing value resolution for the prim's attributes.

use std::sync::Arc;

use usd_pcp::{NodeRef, PrimIndex};
use usd_sdf::LayerHandle;

/// Defines a subrange of nodes and layers within a prim's prim index to
/// consider when performing value resolution for the prim's attributes.
///
/// Matches C++ `UsdResolveTarget`.
#[derive(Debug, Clone, Default)]
pub struct ResolveTarget {
    /// The expanded prim index (held to keep it alive).
    expanded_prim_index: Option<Arc<PrimIndex>>,
    /// The node that value resolution will start at.
    start_node: Option<NodeRef>,
    /// The layer in the layer stack of the start node that value resolution will start at.
    start_layer: Option<LayerHandle>,
    /// The node that value resolution will stop at when the "stop at" layer is reached.
    stop_node: Option<NodeRef>,
    /// The layer in the layer stack of the stop node that value resolution will stop at.
    stop_layer: Option<LayerHandle>,
}

impl ResolveTarget {
    /// Creates a null resolve target.
    ///
    /// Matches C++ default constructor.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a resolve target with the given prim index, start node, and start layer.
    ///
    /// Matches C++ private constructor `UsdResolveTarget(const std::shared_ptr<PcpPrimIndex> &index, const PcpNodeRef &node, const SdfLayerHandle &layer)`.
    pub(crate) fn with_start(
        expanded_prim_index: Arc<PrimIndex>,
        start_node: NodeRef,
        start_layer: LayerHandle,
    ) -> Self {
        Self {
            expanded_prim_index: Some(expanded_prim_index),
            start_node: Some(start_node),
            start_layer: Some(start_layer),
            stop_node: None,
            stop_layer: None,
        }
    }

    /// Creates a resolve target with start and stop nodes/layers.
    ///
    /// Matches C++ private constructor `UsdResolveTarget(const std::shared_ptr<PcpPrimIndex> &index, const PcpNodeRef &node, const SdfLayerHandle &layer, const PcpNodeRef &stopNode, const SdfLayerHandle &stopLayer)`.
    pub(crate) fn with_start_and_stop(
        expanded_prim_index: Arc<PrimIndex>,
        start_node: NodeRef,
        start_layer: LayerHandle,
        stop_node: NodeRef,
        stop_layer: LayerHandle,
    ) -> Self {
        Self {
            expanded_prim_index: Some(expanded_prim_index),
            start_node: Some(start_node),
            start_layer: Some(start_layer),
            stop_node: Some(stop_node),
            stop_layer: Some(stop_layer),
        }
    }

    /// Get the prim index of the resolve target.
    ///
    /// Matches C++ `GetPrimIndex()`.
    pub fn prim_index(&self) -> Option<&PrimIndex> {
        self.expanded_prim_index.as_deref()
    }

    /// Returns the node that value resolution with this resolve target will start at.
    ///
    /// Matches C++ `GetStartNode()`.
    pub fn start_node(&self) -> Option<&NodeRef> {
        self.start_node.as_ref()
    }

    /// Returns the layer in the layer stack of the start node that value
    /// resolution with this resolve target will start at.
    ///
    /// Matches C++ `GetStartLayer()`.
    pub fn start_layer(&self) -> Option<&LayerHandle> {
        self.start_layer.as_ref()
    }

    /// Returns the node that value resolution with this resolve target will
    /// stop at when the "stop at" layer is reached.
    ///
    /// Matches C++ `GetStopNode()`.
    pub fn stop_node(&self) -> Option<&NodeRef> {
        self.stop_node.as_ref()
    }

    /// Returns the layer in the layer stack of the stop node that value
    /// resolution with this resolve target will stop at.
    ///
    /// Matches C++ `GetStopLayer()`.
    pub fn stop_layer(&self) -> Option<&LayerHandle> {
        self.stop_layer.as_ref()
    }

    /// Returns true if this is a null resolve target.
    ///
    /// Matches C++ `IsNull()`.
    pub fn is_null(&self) -> bool {
        self.expanded_prim_index.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_null() {
        let target = ResolveTarget::new();
        assert!(target.is_null());
        assert!(target.prim_index().is_none());
        assert!(target.start_node().is_none());
        assert!(target.start_layer().is_none());
    }
}
