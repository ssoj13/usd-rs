//! Graph traversal -- Edge, TreeIterator, GraphIterator, InheritanceIterator.
//!
//! Mirrors C++ MaterialXCore/Traversal.h:
//!   - Edge: downstream/connecting/upstream triple
//!   - TreeIterator: DFS element tree walk with pruning
//!   - GraphIterator: upstream dataflow graph DFS with cycle detection + pruning
//!   - InheritanceIterator: follows `inherit` chain with cycle detection
//!
//! Free helpers: get_upstream_edge, get_upstream_edge_count, traverse_tree,
//!               traverse_graph_iter, traverse_inheritance.

use std::collections::HashSet;
use std::sync::RwLock;

use crate::core::element::{Element, ElementPtr, category};

// ─────────────────────────────────────────────────────────────────────────────
// Edge
// ─────────────────────────────────────────────────────────────────────────────

/// An edge between connected elements: downstream ← (via connecting) ← upstream.
///
/// Matches C++ MaterialX::Edge. A valid edge has both downstream and upstream;
/// the connecting element (e.g. an Input) is optional.
#[derive(Clone, Debug)]
pub struct Edge {
    /// The downstream element (consumer).
    pub downstream: Option<ElementPtr>,
    /// The connecting element (e.g. the Input port), if any.
    pub connecting: Option<ElementPtr>,
    /// The upstream element (producer / source node).
    pub upstream: Option<ElementPtr>,
}

impl Edge {
    pub fn new(
        downstream: Option<ElementPtr>,
        connecting: Option<ElementPtr>,
        upstream: Option<ElementPtr>,
    ) -> Self {
        Self {
            downstream,
            connecting,
            upstream,
        }
    }

    /// True when the edge carries actual upstream data (non-null upstream).
    pub fn is_valid(&self) -> bool {
        self.upstream.is_some()
    }

    pub fn get_downstream_element(&self) -> Option<&ElementPtr> {
        self.downstream.as_ref()
    }
    pub fn get_connecting_element(&self) -> Option<&ElementPtr> {
        self.connecting.as_ref()
    }
    pub fn get_upstream_element(&self) -> Option<&ElementPtr> {
        self.upstream.as_ref()
    }

    /// Name of the edge = name of the connecting element, if present.
    pub fn get_name(&self) -> String {
        self.connecting
            .as_ref()
            .map(|e| e.borrow().get_name().to_string())
            .unwrap_or_default()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Upstream edge resolution (PortElement logic)
// ─────────────────────────────────────────────────────────────────────────────

/// Number of upstream edges available for `elem`.
///
/// For INPUT / OUTPUT elements that have a `nodename` or `nodegraph` attribute:
///   → 1 upstream edge (the named node/nodegraph).
/// For NODE-category elements:
///   → count of input children that have `nodename`/`nodegraph` set.
/// Everything else: 0.
pub fn get_upstream_edge_count(elem: &ElementPtr) -> usize {
    let cat = elem.borrow().get_category().to_string();
    match cat.as_str() {
        c if c == category::INPUT || c == category::OUTPUT => {
            let b = elem.borrow();
            if b.get_node_name().is_some() || b.get_node_graph_string().is_some() {
                1
            } else {
                0
            }
        }
        // Node-like elements: iterate connected input children
        _ => elem
            .borrow()
            .get_children()
            .iter()
            .filter(|ch| {
                let b = ch.borrow();
                b.get_category() == category::INPUT
                    && (b.get_node_name().is_some() || b.get_node_graph_string().is_some())
            })
            .count(),
    }
}

/// Return the Nth upstream edge for `elem`.
///
/// For INPUT/OUTPUT (index must be 0): resolves nodename → sibling node, or
/// nodegraph+output → NodeGraph's output element.
/// For node-like elements: the Nth connected input child is the connecting element,
/// the upstream is the node/nodegraph it points to.
pub fn get_upstream_edge(elem: &ElementPtr, index: usize) -> Edge {
    let null = Edge::new(None, None, None);
    let cat = elem.borrow().get_category().to_string();

    if cat == category::INPUT || cat == category::OUTPUT {
        // Port resolves to one upstream target
        if index != 0 {
            return null;
        }
        resolve_port_upstream(elem, None)
    } else {
        // Node-like: gather connected inputs in order, pick the Nth
        let inputs: Vec<ElementPtr> = elem
            .borrow()
            .get_children()
            .iter()
            .filter(|ch| {
                let b = ch.borrow();
                b.get_category() == category::INPUT
                    && (b.get_node_name().is_some() || b.get_node_graph_string().is_some())
            })
            .cloned()
            .collect();

        match inputs.get(index) {
            Some(inp) => resolve_port_upstream(inp, Some(elem.clone())),
            None => null,
        }
    }
}

/// Resolve an INPUT or OUTPUT port's upstream connection into an Edge.
/// `downstream_override` is used when the real downstream is the parent node,
/// not the port itself.
fn resolve_port_upstream(port: &ElementPtr, downstream_override: Option<ElementPtr>) -> Edge {
    let downstream = downstream_override.unwrap_or_else(|| port.clone());

    let (node_name, node_graph_name, _output_name) = {
        let b = port.borrow();
        (
            b.get_node_name().map(|s| s.to_string()),
            b.get_node_graph_string().map(|s| s.to_string()),
            b.get_output_string().map(|s| s.to_string()),
        )
    };

    // Find the graph scope: for ports on a node, we need the grandparent
    // (port.parent = node, node.parent = graph). For top-level ports the parent IS the graph.
    let port_parent = match port.borrow().get_parent() {
        Some(p) => p,
        None => return Edge::new(None, None, None),
    };
    // Determine scope: if port_parent is a node-like element (not a graph/document),
    // step up one more level to the containing graph.
    let port_parent_cat = port_parent.borrow().get_category().to_string();
    let scope = if port_parent_cat == category::NODE_GRAPH
        || port_parent_cat == category::MATERIAL
        || port_parent_cat == "materialx"
    {
        // Port is directly inside a graph/document — scope is port_parent
        port_parent.clone()
    } else {
        // Port is inside a node — scope is port_parent.parent (the containing graph)
        match port_parent.borrow().get_parent() {
            Some(gp) => gp,
            None => port_parent.clone(),
        }
    };

    if let Some(ng_name) = &node_graph_name {
        // Connecting to a NodeGraph: look up the graph in scope
        let graph = scope.borrow().get_child(ng_name);
        if let Some(graph) = graph {
            return Edge::new(Some(downstream), Some(port.clone()), Some(graph));
        }
        return Edge::new(None, None, None);
    }

    if let Some(nn) = &node_name {
        // Direct nodename: look for a sibling node in the graph scope
        let upstream_node = scope.borrow().get_child(nn);
        if let Some(upstream) = upstream_node {
            return Edge::new(Some(downstream), Some(port.clone()), Some(upstream));
        }
    }

    Edge::new(None, None, None)
}

// ─────────────────────────────────────────────────────────────────────────────
// TreeIterator
// ─────────────────────────────────────────────────────────────────────────────

/// Depth-first tree iterator over an element subtree.
///
/// Matches C++ TreeIterator. Supports `prune_subtree()` to skip children of
/// the current element. Returns `ElementPtr` items.
pub struct TreeIterator {
    /// Pending elements to visit (DFS order). Each entry is an element to yield next.
    pending: Vec<ElementPtr>,
    /// The element most recently yielded (children will be added on next call unless pruned).
    last_yielded: Option<ElementPtr>,
    /// When true, skip adding children of last_yielded on the next advance.
    prune: bool,
}

impl TreeIterator {
    pub fn new(root: ElementPtr) -> Self {
        Self {
            pending: vec![root],
            last_yielded: None,
            prune: false,
        }
    }

    /// Skip the subtree rooted at the last yielded element.
    /// Call immediately after `next()` returns, before calling `next()` again.
    pub fn prune_subtree(&mut self) {
        self.prune = true;
    }

    /// Depth of the last yielded element (root = 0).
    /// Returns 0 if nothing has been yielded yet.
    pub fn element_depth(&self) -> usize {
        self.last_yielded
            .as_ref()
            .map(|e| {
                // Count ancestors
                let mut depth = 0usize;
                let mut cur = e.borrow().get_parent();
                while let Some(p) = cur {
                    depth += 1;
                    cur = p.borrow().get_parent();
                }
                depth
            })
            .unwrap_or(0)
    }
}

impl Iterator for TreeIterator {
    type Item = ElementPtr;

    fn next(&mut self) -> Option<Self::Item> {
        // Expand children of last yielded element (unless pruned).
        if let Some(last) = self.last_yielded.take() {
            if !self.prune {
                // Push children in reverse order so first child is popped first (DFS).
                let children: Vec<ElementPtr> = last.borrow().get_children().to_vec();
                for child in children.into_iter().rev() {
                    self.pending.push(child);
                }
            }
            self.prune = false;
        }

        let elem = self.pending.pop()?;
        self.last_yielded = Some(elem.clone());
        Some(elem)
    }
}

/// Create a TreeIterator starting at `root`.
pub fn traverse_tree(root: ElementPtr) -> TreeIterator {
    TreeIterator::new(root)
}

// ─────────────────────────────────────────────────────────────────────────────
// GraphIterator
// ─────────────────────────────────────────────────────────────────────────────

/// Error produced by GraphIterator when a cycle is detected.
#[derive(Debug, Clone)]
pub struct CycleError(pub String);

impl std::fmt::Display for CycleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Cycle in graph at: {}", self.0)
    }
}
impl std::error::Error for CycleError {}

/// Upstream dataflow graph iterator.
///
/// Mirrors C++ GraphIterator. Performs a DFS upstream from a starting element,
/// yielding `Result<Edge, CycleError>` at each step. Call `prune_subgraph()`
/// on the iterator after receiving an edge to skip that branch.
pub struct GraphIterator {
    /// Current upstream element
    upstream_elem: Option<ElementPtr>,
    /// Connecting element for the current edge
    connecting_elem: Option<ElementPtr>,
    /// Elements currently on the DFS path (for cycle detection)
    path_elems: HashSet<*const RwLock<Element>>,
    /// DFS stack: (parent-elem, current-child-index)
    stack: Vec<(ElementPtr, usize)>,
    /// Edges already visited (prevent revisiting shared nodes)
    visited_edges: std::collections::BTreeSet<EdgeKey>,
    /// If true, skip upstream traversal from current element
    prune: bool,
    /// Internal: whether begin() was called (first edge already generated)
    started: bool,
    /// Count of Node-category elements currently on the DFS path.
    /// Replaces the unsafe raw-pointer iteration in node_depth().
    node_depth_counter: usize,
}

/// Stable identity key for an edge (uses raw pointer addresses).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct EdgeKey {
    down: usize,
    connect: usize,
    up: usize,
}

impl EdgeKey {
    fn from_edge(e: &Edge) -> Self {
        EdgeKey {
            down: e
                .downstream
                .as_ref()
                .map(|p| p.as_raw_ptr() as usize)
                .unwrap_or(0),
            connect: e
                .connecting
                .as_ref()
                .map(|p| p.as_raw_ptr() as usize)
                .unwrap_or(0),
            up: e
                .upstream
                .as_ref()
                .map(|p| p.as_raw_ptr() as usize)
                .unwrap_or(0),
        }
    }
}

impl GraphIterator {
    /// Create a new iterator starting at `elem`. Call `begin()` or use directly.
    pub fn new(elem: ElementPtr) -> Self {
        let ptr = elem.as_raw_ptr();
        let mut path_elems = HashSet::new();
        path_elems.insert(ptr);
        let initial_is_node = elem.borrow().get_category() == category::NODE;
        Self {
            upstream_elem: Some(elem),
            connecting_elem: None,
            path_elems,
            stack: Vec::new(),
            visited_edges: std::collections::BTreeSet::new(),
            prune: false,
            started: false,
            node_depth_counter: if initial_is_node { 1 } else { 0 },
        }
    }

    /// Build the current edge from iterator state.
    pub fn current_edge(&self) -> Edge {
        Edge::new(
            self.get_downstream_element(),
            self.connecting_elem.clone(),
            self.upstream_elem.clone(),
        )
    }

    /// Downstream element: top of stack (parent that pushed us).
    pub fn get_downstream_element(&self) -> Option<ElementPtr> {
        self.stack.last().map(|(e, _)| e.clone())
    }

    pub fn get_connecting_element(&self) -> Option<ElementPtr> {
        self.connecting_elem.clone()
    }

    pub fn get_upstream_element(&self) -> Option<ElementPtr> {
        self.upstream_elem.clone()
    }

    /// Index of the current edge among the upstream edges of the downstream element.
    pub fn get_upstream_index(&self) -> usize {
        self.stack.last().map(|(_, idx)| *idx).unwrap_or(0)
    }

    /// Element depth: number of edges traversed from the root.
    pub fn element_depth(&self) -> usize {
        self.stack.len()
    }

    /// Node depth: number of Node-category elements on the current path.
    /// Maintained incrementally in extend_path/retract_path to avoid unsafe pointer deref.
    pub fn node_depth(&self) -> usize {
        self.node_depth_counter
    }

    /// Skip further upstream traversal from the current element.
    pub fn prune_subgraph(&mut self) {
        self.prune = true;
    }

    // ── internal helpers ──────────────────────────────────────────────────

    /// Push `upstream` onto the DFS path. Returns Err if a cycle is detected.
    fn extend_path(
        &mut self,
        upstream: ElementPtr,
        connecting: Option<ElementPtr>,
    ) -> Result<(), CycleError> {
        let ptr = upstream.as_raw_ptr();
        if self.path_elems.contains(&ptr) {
            let name = upstream.borrow().as_string();
            return Err(CycleError(name));
        }
        if upstream.borrow().get_category() == category::NODE {
            self.node_depth_counter += 1;
        }
        self.path_elems.insert(ptr);
        self.upstream_elem = Some(upstream);
        self.connecting_elem = connecting;
        Ok(())
    }

    /// Remove `upstream` from the path when backtracking.
    fn retract_path(&mut self, upstream: &ElementPtr) {
        let ptr = upstream.as_raw_ptr();
        self.path_elems.remove(&ptr);
        if upstream.borrow().get_category() == category::NODE {
            self.node_depth_counter = self.node_depth_counter.saturating_sub(1);
        }
        self.upstream_elem = None;
        self.connecting_elem = None;
    }

    /// Returns `true` if the edge was already visited (and marks it visited).
    fn skip_or_mark(&mut self, edge: &Edge) -> bool {
        let key = EdgeKey::from_edge(edge);
        !self.visited_edges.insert(key)
    }

    /// Try to advance to the next valid edge. Returns Ok(true) if successful,
    /// Ok(false) if exhausted, Err on cycle.
    fn advance(&mut self) -> Result<bool, CycleError> {
        // Attempt to go deeper from current upstream element
        if !self.prune {
            if let Some(ref up) = self.upstream_elem.clone() {
                let count = get_upstream_edge_count(up);
                if count > 0 {
                    self.stack.push((up.clone(), 0));
                    let candidate = get_upstream_edge(up, 0);
                    if candidate.is_valid() {
                        if !self.skip_or_mark(&candidate) {
                            let upstream = candidate.upstream.clone().unwrap();
                            let connecting = candidate.connecting.clone();
                            self.extend_path(upstream, connecting)?;
                            return Ok(true);
                        }
                    }
                    // Edge invalid or already visited; fall through to sibling search
                }
            }
        }
        self.prune = false;

        // Backtrack and look for unvisited siblings / parent's siblings
        loop {
            if let Some(ref up) = self.upstream_elem.clone() {
                self.retract_path(up);
            }

            let frame = match self.stack.last_mut() {
                None => return Ok(false), // exhausted
                Some(f) => f,
            };

            let parent = frame.0.clone();
            let next_idx = frame.1 + 1;
            let total = get_upstream_edge_count(&parent);

            if next_idx < total {
                frame.1 = next_idx;
                let candidate = get_upstream_edge(&parent, next_idx);
                if candidate.is_valid() && !self.skip_or_mark(&candidate) {
                    let upstream = candidate.upstream.clone().unwrap();
                    let connecting = candidate.connecting.clone();
                    self.extend_path(upstream, connecting)?;
                    return Ok(true);
                }
                // Invalid/visited; try next sibling in next loop iteration
                // (keep frame with updated idx, re-enter loop)
                //
                // Actually: re-enter with upstream_elem = None so retract_path is skipped
                self.upstream_elem = None;
                continue;
            }

            // No more siblings; pop frame and go up
            self.stack.pop();
            // Retract parent from path on next iteration
            self.upstream_elem = Some(parent);
        }
    }
}

impl Iterator for GraphIterator {
    type Item = Result<Edge, CycleError>;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.started {
            // First call: generate the very first edge by advancing once
            self.started = true;
            match self.advance() {
                Ok(true) => Some(Ok(self.current_edge())),
                Ok(false) => None,
                Err(e) => Some(Err(e)),
            }
        } else {
            // Subsequent calls: advance then return new edge
            match self.advance() {
                Ok(true) => Some(Ok(self.current_edge())),
                Ok(false) => None,
                Err(e) => Some(Err(e)),
            }
        }
    }
}

/// Create a GraphIterator starting at `elem`.
pub fn traverse_graph_iter(root: ElementPtr) -> GraphIterator {
    GraphIterator::new(root)
}

// ─────────────────────────────────────────────────────────────────────────────
// InheritanceIterator
// ─────────────────────────────────────────────────────────────────────────────

/// Iterator that follows the inheritance chain of an element.
///
/// Mirrors C++ InheritanceIterator. Yields the starting element first, then
/// each inherited ancestor in order (following `inherit` attributes). Stops
/// at None or a category mismatch. Returns `Err(CycleError)` on a cycle.
pub struct InheritanceIterator {
    /// Current element (None = exhausted)
    elem: Option<ElementPtr>,
    /// Pointer set for cycle detection
    visited: HashSet<*const RwLock<Element>>,
}

impl InheritanceIterator {
    pub fn new(root: ElementPtr) -> Self {
        let ptr = root.as_raw_ptr();
        let mut visited = HashSet::new();
        visited.insert(ptr);
        Self {
            elem: Some(root),
            visited,
        }
    }
}

impl Iterator for InheritanceIterator {
    /// Yields the current element, or a CycleError if a cycle is detected.
    type Item = Result<ElementPtr, CycleError>;

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.elem.take()?;
        let result = current.clone();

        // Resolve next: find the element named by `inherit` attribute
        let inherit_name = match current.borrow().get_inherits_from() {
            Some(s) => s.to_string(),
            None => return Some(Ok(result)), // no inheritance, final element
        };
        if inherit_name.is_empty() {
            return Some(Ok(result));
        }

        // Category must match (C++ InheritanceIterator checks this)
        let current_cat = current.borrow().get_category().to_string();

        // Resolve by name: look in parent scope (siblings) or document root
        let next_elem = resolve_inherit(&current, &inherit_name);

        match next_elem {
            None => {
                // Named element not found; chain ends
                Some(Ok(result))
            }
            Some(next) => {
                // Category mismatch: stop
                if next.borrow().get_category() != current_cat {
                    return Some(Ok(result));
                }
                // Cycle check
                let ptr = next.as_raw_ptr();
                if self.visited.contains(&ptr) {
                    self.elem = None;
                    return Some(Err(CycleError(next.borrow().as_string())));
                }
                self.visited.insert(ptr);
                self.elem = Some(next);
                Some(Ok(result))
            }
        }
    }
}

/// Resolve the `inherit` name relative to `elem`'s scope.
///
/// MaterialX inheritance is always between same-scope siblings (e.g. NodeDefs
/// at document level). We walk up to find the parent and look for a child
/// with the given name.
fn resolve_inherit(elem: &ElementPtr, name: &str) -> Option<ElementPtr> {
    let parent = elem.borrow().get_parent()?;
    parent.borrow().get_child(name)
}

/// Create an InheritanceIterator starting at `root`.
pub fn traverse_inheritance(root: ElementPtr) -> InheritanceIterator {
    InheritanceIterator::new(root)
}

// ─────────────────────────────────────────────────────────────────────────────
// Legacy free function (kept for compat with previous code)
// ─────────────────────────────────────────────────────────────────────────────

/// Traverse the dataflow graph upstream from `elem`, calling `f` for each edge.
/// Legacy callback-style API. Prefer `traverse_graph_iter` for iterator usage.
pub fn traverse_graph<F>(elem: &ElementPtr, f: &mut F)
where
    F: FnMut(&Edge),
{
    for result in GraphIterator::new(elem.clone()) {
        if let Ok(edge) = result {
            f(&edge);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::document::create_document;
    use crate::core::element::add_child_of_category;

    // Helper: create a minimal document with a nodegraph.
    // Returns (doc_root, ng, img_node, mul_node) as ElementPtr.
    fn make_doc_with_graph() -> (ElementPtr, ElementPtr, ElementPtr, ElementPtr) {
        let doc = create_document();
        let root = doc.get_root();
        // nodegraph
        let ng = add_child_of_category(&root, category::NODE_GRAPH, "ng").unwrap();
        // two nodes: image -> multiply
        let img = add_child_of_category(&ng, "image", "img").unwrap();
        let mul = add_child_of_category(&ng, "multiply", "mul").unwrap();
        // input on mul that connects to img
        let inp = add_child_of_category(&mul, category::INPUT, "in1").unwrap();
        inp.borrow_mut().set_node_name("img");
        (root, ng, img, mul)
    }

    #[test]
    fn test_edge_valid() {
        let (_, _, img, mul) = make_doc_with_graph();
        let edge = Edge::new(Some(mul.clone()), None, Some(img.clone()));
        assert!(edge.is_valid());
        assert_eq!(
            edge.get_upstream_element().unwrap().borrow().get_name(),
            "img"
        );
    }

    #[test]
    fn test_tree_iterator_order() {
        let doc = create_document();
        let root = doc.get_root();
        let ng = add_child_of_category(&root, category::NODE_GRAPH, "ng").unwrap();
        let _n1 = add_child_of_category(&ng, "image", "n1").unwrap();
        let _n2 = add_child_of_category(&ng, "multiply", "n2").unwrap();

        let names: Vec<String> = traverse_tree(root.clone())
            .map(|e| e.borrow().get_name().to_string())
            .collect();

        // root ("") -> ng -> n1 -> n2
        assert!(names.contains(&"ng".to_string()));
        assert!(names.contains(&"n1".to_string()));
        assert!(names.contains(&"n2".to_string()));
        // ng must come before n1 and n2
        let pos_ng = names.iter().position(|s| s == "ng").unwrap();
        let pos_n1 = names.iter().position(|s| s == "n1").unwrap();
        assert!(pos_ng < pos_n1);
    }

    #[test]
    fn test_tree_iterator_prune() {
        let doc = create_document();
        let root = doc.get_root();
        let ng = add_child_of_category(&root, category::NODE_GRAPH, "ng").unwrap();
        let _n1 = add_child_of_category(&ng, "image", "n1").unwrap();

        let mut it = TreeIterator::new(root.clone());
        let first = it.next().unwrap(); // document root
        assert_eq!(first.borrow().get_category(), "materialx");
        it.prune_subtree(); // skip all children of document
        assert!(it.next().is_none());
    }

    #[test]
    fn test_upstream_edge_count() {
        // Keep _root and _ng alive so Weak parent pointers from nodes remain valid.
        let (_root, _ng, img, mul) = make_doc_with_graph();
        // mul has one connected input ("in1" -> img)
        assert_eq!(get_upstream_edge_count(&mul), 1);
        // img has no connected inputs
        assert_eq!(get_upstream_edge_count(&img), 0);
    }

    #[test]
    fn test_upstream_edge_resolve() {
        let (_root, _ng, _img, mul) = make_doc_with_graph();
        let edge = get_upstream_edge(&mul, 0);
        assert!(edge.is_valid());
        let up = edge.get_upstream_element().unwrap();
        assert_eq!(up.borrow().get_name(), "img");
    }

    #[test]
    fn test_graph_iterator_basic() {
        let (_root, _ng, _img, mul) = make_doc_with_graph();
        let edges: Vec<Edge> = GraphIterator::new(mul).filter_map(|r| r.ok()).collect();
        // Should find exactly one edge: mul <- img
        assert_eq!(edges.len(), 1);
        let e = &edges[0];
        assert_eq!(e.get_upstream_element().unwrap().borrow().get_name(), "img");
    }

    #[test]
    fn test_inheritance_iterator_no_inherit() {
        let doc = create_document();
        let root = doc.get_root();
        let nd = add_child_of_category(&root, "nodedef", "ND_foo").unwrap();
        let items: Vec<String> = traverse_inheritance(nd)
            .filter_map(|r| r.ok())
            .map(|e| e.borrow().get_name().to_string())
            .collect();
        assert_eq!(items, vec!["ND_foo"]);
    }

    #[test]
    fn test_inheritance_iterator_chain() {
        let doc = create_document();
        let root = doc.get_root();
        let _nd_base = add_child_of_category(&root, "nodedef", "ND_base").unwrap();
        let nd_child = add_child_of_category(&root, "nodedef", "ND_child").unwrap();
        nd_child.borrow_mut().set_inherit_string("ND_base");

        let items: Vec<String> = traverse_inheritance(nd_child)
            .filter_map(|r| r.ok())
            .map(|e| e.borrow().get_name().to_string())
            .collect();
        // child first, then base
        assert_eq!(items, vec!["ND_child", "ND_base"]);
    }

    #[test]
    fn test_inheritance_iterator_cycle() {
        let doc = create_document();
        let root = doc.get_root();
        let nd_a = add_child_of_category(&root, "nodedef", "ND_a").unwrap();
        let nd_b = add_child_of_category(&root, "nodedef", "ND_b").unwrap();
        nd_a.borrow_mut().set_inherit_string("ND_b");
        nd_b.borrow_mut().set_inherit_string("ND_a");

        let results: Vec<_> = traverse_inheritance(nd_a).collect();
        // Should end with a CycleError
        let has_err = results.iter().any(|r| r.is_err());
        assert!(has_err, "Expected cycle detection");
    }
}
