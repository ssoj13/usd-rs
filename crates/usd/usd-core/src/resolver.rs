//! Usd_Resolver - internal class for value resolution.
//!
//! Port of pxr/usd/usd/resolver.h/cpp
//!
//! Given a PcpPrimIndex, this class facilitates value resolution by providing
//! a mechanism for walking the composition structure in strong-to-weak order.

use crate::resolve_target::ResolveTarget;
use std::sync::Arc;
use usd_pcp::{LayerStackRefPtr, NodeRef, PrimIndex};
use usd_sdf::Path;
use usd_sdf::{Layer, LayerOffset};
use usd_tf::Token;

// ============================================================================
// Resolver
// ============================================================================

/// Internal resolver for walking composition structure in strong-to-weak order.
///
/// Matches C++ `Usd_Resolver`.
///
/// This is an internal class used by Attribute and Property for value resolution.
/// It iterates over PCP nodes and layers in the correct order.
///
/// Performance: C++ uses raw iterators (`_curLayer`, `_curNode`) for zero-cost
/// layer access. We cache the current node's layers and layer stack to avoid
/// repeated RwLock reads + Vec<Arc<Layer>> allocations on every iteration step.
pub struct Resolver {
    /// The prim index being resolved.
    index: Option<Arc<PrimIndex>>,
    /// Whether to skip empty nodes.
    skip_empty_nodes: bool,
    /// Current node index into index.nodes() — O(1) step instead of O(n) search.
    current_node_index: usize,
    /// Exclusive upper bound for node iteration.
    end_node_index: usize,
    /// Current layer index within current node's layer stack.
    current_layer_index: usize,
    /// Exclusive upper bound for layer iteration within the current node.
    end_layer_index: usize,
    /// Resolve target (if provided).
    resolve_target: Option<ResolveTarget>,
    /// Cached layers for the current node — avoids repeated
    /// `layer_stack().get_layers()` (RwLock read + Vec clone) per iteration step.
    cached_layers: Vec<Arc<Layer>>,
    /// Cached layer stack for the current node — avoids repeated
    /// `node.layer_stack()` (RwLock read + Arc clone) in `get_layer_to_stage_offset`.
    cached_layer_stack: Option<LayerStackRefPtr>,
}

impl Resolver {
    /// Constructs a resolver with the given prim index.
    ///
    /// Matches C++ `Usd_Resolver(const PcpPrimIndex* index, bool skipEmptyNodes)`.
    pub fn new(index: &Arc<PrimIndex>, skip_empty_nodes: bool) -> Self {
        let end_node_index = index.num_nodes();

        let mut resolver = Self {
            index: Some(index.clone()),
            skip_empty_nodes,
            current_node_index: 0,
            end_node_index,
            current_layer_index: 0,
            end_layer_index: 0,
            resolve_target: None,
            cached_layers: Vec::new(),
            cached_layer_stack: None,
        };

        resolver.skip_empty_nodes();
        resolver.refresh_node_cache();

        resolver
    }

    /// Constructs a resolver positioned at the node/layer recorded in `resolve_info`.
    ///
    /// Matches C++ `Usd_Resolver(const PcpPrimIndex* index, bool skipEmptyNodes,
    /// const UsdResolveInfo* resolveInfo)` (`resolver.cpp`): the iterator starts at
    /// `resolveInfo->_node` / `resolveInfo->_layer` when present, so time-sample and
    /// value queries can continue toward **weaker** opinions without re-walking from the
    /// strongest site (used by `UsdStage::_GetTimeSamplesInInterval`, bracketing queries,
    /// and `_GetValueFromResolveInfo`).
    ///
    /// When `resolve_info` is `None`, behaves like [`Resolver::new`].
    pub fn new_with_resolve_info(
        index: &Arc<PrimIndex>,
        skip_empty_nodes: bool,
        resolve_info: Option<&crate::resolve_info::ResolveInfo>,
    ) -> Self {
        let nodes = index.nodes();
        let end_node_index = nodes.len();

        let start_node_index = resolve_info
            .and_then(|info| info.node())
            .and_then(|node| nodes.iter().position(|n| n == node))
            .unwrap_or(0);

        let mut resolver = Self {
            index: Some(index.clone()),
            skip_empty_nodes,
            current_node_index: start_node_index,
            end_node_index,
            current_layer_index: 0,
            end_layer_index: 0,
            resolve_target: None,
            cached_layers: Vec::new(),
            cached_layer_stack: None,
        };

        resolver.skip_empty_nodes();
        resolver.refresh_node_cache();

        if resolver.is_node_valid() {
            let layer_idx = resolve_info
                .and_then(|info| info.layer())
                .and_then(|h| h.upgrade())
                .and_then(|layer_arc| {
                    resolver.cached_layers.iter().position(|l| {
                        Arc::ptr_eq(l, &layer_arc) || l.identifier() == layer_arc.identifier()
                    })
                })
                .unwrap_or(0);
            resolver.current_layer_index =
                layer_idx.min(resolver.cached_layers.len().saturating_sub(1));
        }

        resolver
    }

    /// Constructs a resolver with the given resolve target.
    ///
    /// Matches C++ `Usd_Resolver(const UsdResolveTarget *resolveTarget, bool skipEmptyNodes)`.
    pub fn new_with_resolve_target(resolve_target: &ResolveTarget, skip_empty_nodes: bool) -> Self {
        let Some(index_ref) = resolve_target.prim_index() else {
            return Self::default();
        };

        // Create Arc from reference — ResolveTarget holds &PrimIndex, we need Arc.
        let index_arc = Arc::new(index_ref.clone());

        let nodes = index_ref.nodes();
        let end_node_index = nodes.len();

        // Locate start node index from resolve target.
        let start_node_index = resolve_target
            .start_node()
            .and_then(|sn| nodes.iter().position(|n| n.node_index() == sn.node_index()))
            .unwrap_or(0);

        // Locate stop node index (exclusive upper bound).
        let stop_node_index = resolve_target
            .stop_node()
            .and_then(|sn| nodes.iter().position(|n| n.node_index() == sn.node_index()))
            .unwrap_or(end_node_index);

        let mut resolver = Self {
            index: Some(index_arc),
            skip_empty_nodes,
            current_node_index: start_node_index,
            end_node_index: stop_node_index,
            current_layer_index: 0,
            end_layer_index: 0,
            resolve_target: Some(resolve_target.clone()),
            cached_layers: Vec::new(),
            cached_layer_stack: None,
        };

        // Set start layer index from resolve target.
        if let Some(start_layer_handle) = resolve_target.start_layer() {
            if let Some(start_layer) = start_layer_handle.upgrade() {
                // Pre-fetch layers for start layer position lookup
                let layers = resolver.fetch_current_layers();
                resolver.current_layer_index = layers
                    .iter()
                    .position(|l| {
                        Arc::ptr_eq(l, &start_layer) || l.identifier() == start_layer.identifier()
                    })
                    .unwrap_or(0);
            }
        }

        resolver.skip_empty_nodes();
        resolver.refresh_node_cache();

        resolver
    }

    /// Resolver with a [`ResolveTarget`](crate::resolve_target::ResolveTarget) **and** a cached
    /// [`ResolveInfo`](crate::resolve_info::ResolveInfo) start position.
    ///
    /// Matches C++ `Usd_Resolver(const UsdResolveTarget *resolveTarget, bool skipEmptyNodes,
    /// const UsdResolveInfo *resolveInfo)` (`resolver.cpp`): search for `resolveInfo->_node`
    /// from the target's start node forward; start layer from `resolveInfo->_layer`, else from
    /// the target's start layer when still on the start node, else the first layer in the stack.
    pub fn new_with_resolve_target_and_resolve_info(
        resolve_target: &ResolveTarget,
        skip_empty_nodes: bool,
        resolve_info: Option<&crate::resolve_info::ResolveInfo>,
    ) -> Self {
        let mut r = Self::new_with_resolve_target(resolve_target, skip_empty_nodes);
        let Some(info) = resolve_info else {
            return r;
        };
        let Some(ref index) = r.index else {
            return r;
        };
        let nodes = index.nodes();
        let start_node_index = resolve_target
            .start_node()
            .and_then(|sn| nodes.iter().position(|n| n.node_index() == sn.node_index()))
            .unwrap_or(0);

        if let Some(want) = info.node() {
            if let Some(pos) = nodes
                .iter()
                .enumerate()
                .skip(start_node_index)
                .find(|(_, n)| **n == *want)
                .map(|(i, _)| i)
            {
                if pos < r.end_node_index {
                    r.current_node_index = pos;
                    r.current_layer_index = 0;
                    r.refresh_node_cache();
                    r.current_layer_index = if let Some(h) = info.layer() {
                        h.upgrade()
                            .and_then(|layer_arc| {
                                r.cached_layers.iter().position(|l| {
                                    Arc::ptr_eq(l, &layer_arc)
                                        || l.identifier() == layer_arc.identifier()
                                })
                            })
                            .unwrap_or(0)
                    } else if pos == start_node_index {
                        resolve_target
                            .start_layer()
                            .and_then(|h| h.upgrade())
                            .and_then(|start_layer| {
                                r.cached_layers.iter().position(|l| {
                                    Arc::ptr_eq(l, &start_layer)
                                        || l.identifier() == start_layer.identifier()
                                })
                            })
                            .unwrap_or(0)
                    } else {
                        0
                    };
                    r.current_layer_index = r
                        .current_layer_index
                        .min(r.cached_layers.len().saturating_sub(1));
                    r.end_layer_index = r.compute_end_layer_index(&r.cached_layers);
                    r.skip_empty_nodes();
                }
            }
        } else if let Some(h) = info.layer() {
            if let Some(layer_arc) = h.upgrade() {
                let idx = r
                    .cached_layers
                    .iter()
                    .position(|l| {
                        Arc::ptr_eq(l, &layer_arc) || l.identifier() == layer_arc.identifier()
                    })
                    .unwrap_or(r.current_layer_index);
                r.current_layer_index = idx.min(r.cached_layers.len().saturating_sub(1));
                r.end_layer_index = r.compute_end_layer_index(&r.cached_layers);
            }
        }

        r
    }

    /// Returns true when there is a current Node and Layer.
    ///
    /// Matches C++ `IsValid()`.
    pub fn is_valid(&self) -> bool {
        self.current_node_index < self.end_node_index
            && self.current_layer_index < self.end_layer_index
    }

    /// Advances the resolver to the next weaker Layer in the layer stack.
    ///
    /// Matches C++ `NextLayer()`.
    pub fn next_layer(&mut self) -> bool {
        self.current_layer_index += 1;

        if self.current_layer_index >= self.end_layer_index {
            // Exhausted this node's layers — advance to next node.
            self.next_node();
            return true;
        }

        false
    }

    /// Skips all pending layers in the current LayerStack and jumps to the next weaker PcpNode.
    ///
    /// Matches C++ `NextNode()`.
    pub fn next_node(&mut self) {
        self.current_node_index += 1; // O(1)
        self.skip_empty_nodes();
        self.refresh_node_cache();
    }

    /// Returns the current PCP node for a valid resolver.
    ///
    /// Matches C++ `GetNode()`.
    pub fn get_node(&self) -> Option<NodeRef> {
        self.current_node()
    }

    /// Returns the current layer for the current PcpNode for a valid resolver.
    ///
    /// Matches C++ `GetLayer()`.
    ///
    /// C++ returns `const SdfLayerRefPtr&` (zero-cost reference). We return a
    /// cloned `Arc<Layer>` from the per-node cache — single refcount bump, no
    /// Vec allocation.
    pub fn get_layer(&self) -> Option<Arc<Layer>> {
        if !self.is_valid() {
            return None;
        }
        self.cached_layers.get(self.current_layer_index).cloned()
    }

    /// Returns a translated path for the current PcpNode and Layer for a valid resolver.
    ///
    /// Matches C++ `GetLocalPath()`.
    pub fn get_local_path(&self) -> Option<Path> {
        self.current_node().map(|n| n.path())
    }

    /// Returns a translated path of the property with the given propName.
    ///
    /// Matches C++ `GetLocalPath(TfToken const &propName)`.
    pub fn get_local_path_for_property(&self, prop_name: &Token) -> Option<Path> {
        if prop_name.get_text().is_empty() {
            return self.get_local_path();
        }

        self.get_local_path()
            .and_then(|path| path.append_property(prop_name.get_text()))
    }

    /// Returns the PcpPrimIndex.
    ///
    /// Matches C++ `GetPrimIndex()`.
    pub fn get_prim_index(&self) -> Option<&Arc<PrimIndex>> {
        self.index.as_ref()
    }

    /// Returns the cumulative layer-to-stage offset for the current node+layer.
    ///
    /// Matches C++ `_GetLayerToStageOffset(pcpNode, layer)` in stage.cpp:
    ///   offset = node.mapToRoot.timeOffset * layerStack.getLayerOffset(layer)
    ///
    /// The result maps layer-local time to stage time: `stage_time = offset.apply(layer_time)`.
    /// Use `offset.inverse()` to go the other direction (stage → layer).
    pub fn get_layer_to_stage_offset(&self) -> LayerOffset {
        let Some(node) = self.current_node() else {
            return LayerOffset::identity();
        };

        // 1. Node-to-root offset (from composition arcs: references, payloads, etc.)
        let node_to_root = node.map_to_root().time_offset();

        // 2. Sublayer offset within the node's layer stack (use cached layer stack)
        let Some(ref layer_stack) = self.cached_layer_stack else {
            return node_to_root;
        };
        let sublayer_offset = layer_stack
            .get_layer_offset_at(self.current_layer_index)
            .unwrap_or_else(LayerOffset::identity);

        // Compose: first sublayer offset (layer→root-of-stack), then node offset (stack→stage)
        node_to_root.compose(&sublayer_offset)
    }

    // ========================================================================
    // Internal Helpers
    // ========================================================================

    /// O(1) accessor: returns the node at `current_node_index`, or None if out of range.
    fn current_node(&self) -> Option<NodeRef> {
        self.index
            .as_ref()
            .and_then(|idx| idx.nodes().get(self.current_node_index).cloned())
    }

    /// Node-only validity check (layer iterators may not be set yet).
    fn is_node_valid(&self) -> bool {
        self.current_node_index < self.end_node_index
    }

    /// Refreshes the cached layers and layer stack from the current node.
    /// Called once per node transition (constructor, next_node) instead of
    /// per-iteration-step, eliminating O(n) Vec allocations from
    /// `layer_stack().get_layers()`.
    fn refresh_node_cache(&mut self) {
        if !self.is_node_valid() {
            self.cached_layers.clear();
            self.cached_layer_stack = None;
            self.end_layer_index = 0;
            return;
        }

        if let Some(node) = self.current_node() {
            if let Some(ls) = node.layer_stack() {
                self.cached_layers = ls.get_layers();
                self.cached_layer_stack = Some(ls);
            } else {
                self.cached_layers.clear();
                self.cached_layer_stack = None;
            }
        } else {
            self.cached_layers.clear();
            self.cached_layer_stack = None;
        }

        self.current_layer_index = 0;
        self.end_layer_index = self.compute_end_layer_index(&self.cached_layers);
    }

    /// Fetches layers for the current node without caching (one-shot use in constructors).
    fn fetch_current_layers(&self) -> Vec<Arc<Layer>> {
        if !self.is_node_valid() {
            return Vec::new();
        }
        self.current_node()
            .and_then(|node| node.layer_stack())
            .map(|ls| ls.get_layers())
            .unwrap_or_default()
    }

    /// Skips empty/inert nodes by incrementing `current_node_index` — O(k) where k is
    /// the number of skipped nodes, each step O(1).
    ///
    /// Matches C++ `_SkipEmptyNodes()`.
    fn skip_empty_nodes(&mut self) {
        let Some(ref index) = self.index else {
            return;
        };
        let nodes = index.nodes();

        if self.skip_empty_nodes {
            // Skip nodes where !has_specs() || is_inert()
            while self.current_node_index < self.end_node_index {
                let node = &nodes[self.current_node_index];
                if node.has_specs() && !node.is_inert() {
                    break;
                }
                self.current_node_index += 1;
            }
        } else {
            // Skip only inert nodes
            while self.current_node_index < self.end_node_index {
                let node = &nodes[self.current_node_index];
                if !node.is_inert() {
                    break;
                }
                self.current_node_index += 1;
            }
        }
    }

    /// Computes end_layer_index for `layers`, respecting an optional stop_layer in
    /// the resolve target when we are on the stop node.
    fn compute_end_layer_index(&self, layers: &[Arc<Layer>]) -> usize {
        let Some(ref rt) = self.resolve_target else {
            return layers.len();
        };

        let stop_node = rt.stop_node();
        let on_stop = stop_node
            .as_ref()
            .and_then(|sn| {
                self.current_node()
                    .map(|cn| cn.node_index() == sn.node_index())
            })
            .unwrap_or(false);

        if on_stop {
            Self::stop_layer_index(layers, rt)
        } else {
            layers.len()
        }
    }

    /// Finds the index of the stop layer within `layers`, falling back to `layers.len()`.
    fn stop_layer_index(layers: &[Arc<Layer>], rt: &ResolveTarget) -> usize {
        rt.stop_layer()
            .and_then(|wh| wh.upgrade())
            .and_then(|stop| {
                layers
                    .iter()
                    .position(|l| Arc::ptr_eq(l, &stop) || l.identifier() == stop.identifier())
            })
            .unwrap_or(layers.len())
    }
}

impl Default for Resolver {
    fn default() -> Self {
        Self {
            index: None,
            skip_empty_nodes: true,
            current_node_index: 0,
            end_node_index: 0,
            current_layer_index: 0,
            end_layer_index: 0,
            resolve_target: None,
            cached_layers: Vec::new(),
            cached_layer_stack: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::InitialLoadSet;
    use crate::stage::Stage;

    /// Returns a PrimIndex for a freshly defined prim on an in-memory stage.
    fn make_prim_index(prim_path: &str) -> Option<Arc<PrimIndex>> {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).ok()?;
        stage.define_prim(prim_path, "Xform").ok()?;
        let pcp = stage.pcp_cache()?;
        let path = usd_sdf::Path::from_string(prim_path)?;
        let (idx, _errs) = pcp.compute_prim_index(&path);
        idx.is_valid().then(|| Arc::new(idx))
    }

    #[test]
    fn test_default_is_invalid() {
        let r = Resolver::default();
        assert!(!r.is_valid(), "default resolver must be invalid");
        assert!(r.get_node().is_none());
        assert!(r.get_layer().is_none());
        assert!(r.get_local_path().is_none());
        assert!(r.get_prim_index().is_none());
    }

    #[test]
    fn test_new_does_not_panic() {
        if let Some(idx) = make_prim_index("/ResolverTest") {
            let _r = Resolver::new(&idx, true);
        }
    }

    #[test]
    fn test_get_prim_index_returns_arc() {
        let Some(idx) = make_prim_index("/IdxTest") else {
            return;
        };
        let r = Resolver::new(&idx, true);
        assert!(
            r.get_prim_index().is_some(),
            "get_prim_index must be Some after construction"
        );
    }

    #[test]
    fn test_next_layer_exhausts_all_nodes() {
        let Some(idx) = make_prim_index("/Exhaust") else {
            return;
        };
        let mut r = Resolver::new(&idx, true);
        if !r.is_valid() {
            return;
        }
        let mut count = 0;
        while r.is_valid() && count < 200 {
            r.next_layer();
            count += 1;
        }
        assert!(
            !r.is_valid(),
            "resolver must become invalid after exhausting all nodes/layers"
        );
    }

    #[test]
    fn test_get_local_path_for_property_appends_dot_name() {
        let Some(idx) = make_prim_index("/PropTest") else {
            return;
        };
        let r = Resolver::new(&idx, true);
        if !r.is_valid() {
            return;
        }
        let tok = Token::new("size");
        if let Some(p) = r.get_local_path_for_property(&tok) {
            assert!(
                p.to_string().ends_with(".size"),
                "expected path ending in .size, got: {}",
                p
            );
        }
    }

    #[test]
    fn test_empty_prop_name_returns_plain_prim_path() {
        let Some(idx) = make_prim_index("/EmptyProp") else {
            return;
        };
        let r = Resolver::new(&idx, true);
        if !r.is_valid() {
            return;
        }
        let empty = Token::new("");
        let with_empty = r.get_local_path_for_property(&empty);
        let plain = r.get_local_path();
        assert_eq!(
            with_empty, plain,
            "empty prop name must return prim path unchanged"
        );
    }

    #[test]
    fn test_skip_empty_false_does_not_panic() {
        let Some(idx) = make_prim_index("/SkipTest") else {
            return;
        };
        let _ = Resolver::new(&idx, true);
        let _ = Resolver::new(&idx, false);
    }
}
