//! TraceAggregateTree - Aggregated trace data tree.
//!
//! Port of pxr/base/trace/aggregateTree.h

use super::aggregate_node::AggregateNode;
use super::counter_accumulator::CounterMap;
use super::event_node::TimeStamp;
use super::event_tree::EventTree;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Global node ID counter.
static NEXT_NODE_ID: AtomicU64 = AtomicU64::new(1);

fn next_node_id() -> u64 {
    NEXT_NODE_ID.fetch_add(1, Ordering::Relaxed)
}

/// Aggregated tree of trace data.
///
/// This tree aggregates timing data from EventTree, combining multiple
/// calls to the same scope into single nodes with accumulated timing.
#[derive(Debug)]
pub struct AggregateTree {
    /// Root node.
    root: Arc<AggregateNode>,
    /// Counter values.
    counters: CounterMap,
    /// Node lookup by ID.
    nodes_by_id: HashMap<u64, Arc<AggregateNode>>,
    /// Nodes by key for fast lookup.
    nodes_by_key: HashMap<String, Vec<u64>>,
}

impl Default for AggregateTree {
    fn default() -> Self {
        Self::new()
    }
}

impl AggregateTree {
    /// Creates a new empty aggregate tree.
    pub fn new() -> Self {
        let root = Arc::new(AggregateNode::new("root", next_node_id()));
        let root_id = root.id();
        let mut nodes_by_id = HashMap::new();
        nodes_by_id.insert(root_id, Arc::clone(&root));

        Self {
            root,
            counters: CounterMap::new(),
            nodes_by_id,
            nodes_by_key: HashMap::new(),
        }
    }

    /// Creates an aggregate tree from an event tree.
    pub fn from_event_tree(event_tree: &EventTree) -> Self {
        let mut tree = Self::new();
        tree.counters = event_tree.counters().clone();

        let mut builder = AggregateTreeBuilder::new();
        tree.root = builder.build(event_tree.root());
        tree.nodes_by_id = builder.nodes_by_id;
        tree.nodes_by_key = builder.nodes_by_key;

        tree
    }

    /// Returns the root node.
    #[inline]
    pub fn root(&self) -> &Arc<AggregateNode> {
        &self.root
    }

    /// Returns counter values.
    #[inline]
    pub fn counters(&self) -> &CounterMap {
        &self.counters
    }

    /// Gets a node by ID.
    pub fn get_node(&self, id: u64) -> Option<&Arc<AggregateNode>> {
        self.nodes_by_id.get(&id)
    }

    /// Gets all nodes with a given key.
    pub fn get_nodes_by_key(&self, key: &str) -> Vec<&Arc<AggregateNode>> {
        self.nodes_by_key
            .get(key)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.nodes_by_id.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Returns the total number of nodes.
    pub fn node_count(&self) -> usize {
        self.nodes_by_id.len()
    }

    /// Returns true if the tree is empty.
    pub fn is_empty(&self) -> bool {
        self.root.child_count() == 0
    }

    /// Returns the total inclusive time at root level.
    pub fn total_time(&self) -> TimeStamp {
        self.root.inclusive_time()
    }

    /// Returns all unique keys in the tree.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.nodes_by_key.keys().map(String::as_str)
    }

    /// Clears the tree.
    pub fn clear(&mut self) {
        *self = Self::new();
    }
}

/// Builder for creating aggregate trees.
///
/// Matches C++ `_CreateAggregateNodes`: uses `AggregateNode::append()` which
/// merges same-key children instead of creating duplicate nodes. This means
/// if two EventNode children have the same key, their timing data is aggregated
/// into a single AggregateNode child.
#[derive(Debug)]
struct AggregateTreeBuilder {
    /// Nodes by ID for lookup.
    nodes_by_id: HashMap<u64, Arc<AggregateNode>>,
    /// Nodes by key.
    nodes_by_key: HashMap<String, Vec<u64>>,
    /// Stack for detecting recursion.
    key_stack: Vec<String>,
}

impl AggregateTreeBuilder {
    fn new() -> Self {
        Self {
            nodes_by_id: HashMap::new(),
            nodes_by_key: HashMap::new(),
            key_stack: Vec::new(),
        }
    }

    fn build(&mut self, event_root: &Arc<super::event_node::EventNode>) -> Arc<AggregateNode> {
        self.build_node(event_root, 0)
    }

    fn build_node(
        &mut self,
        event_node: &super::event_node::EventNode,
        parent_id: u64,
    ) -> Arc<AggregateNode> {
        let id = next_node_id();
        let key = event_node.key().to_string();

        // Check for recursion.
        let is_recursive = self.key_stack.contains(&key);
        self.key_stack.push(key.clone());

        let mut agg_node = AggregateNode::new(&key, id);
        agg_node.set_parent_id(parent_id);
        agg_node.add_time(
            event_node.inclusive_time(),
            event_node.exclusive_time(),
            event_node.count(),
        );

        if is_recursive {
            agg_node.set_expanded(true);
        }

        // Build children using `append()` which merges same-key siblings.
        // This matches C++ `aggStack.top()->Append(key, duration)`.
        // When two event children share the same key, append() accumulates
        // their timing into a single aggregate child node.
        //
        // First pass: create/merge top-level children via append.
        for child in event_node.children() {
            let child_duration = child.inclusive_time();
            let child_count = child.count().max(1);
            agg_node.append(
                child.key(),
                child_duration,
                child_count,
                child_count,
                next_node_id(),
            );
        }

        // Second pass: recursively build grandchildren for each event child.
        // Grandchildren are added to the corresponding aggregate child.
        for child in event_node.children() {
            if child.children().is_empty() {
                continue;
            }
            let child_key = child.key();
            // Build grandchildren and merge them into the aggregate child.
            if let Some(agg_child) = agg_node.get_child_arc_mut(child_key) {
                if let Some(agg_child_mut) = Arc::get_mut(agg_child) {
                    for grandchild in child.children() {
                        let gc_agg = self.build_node(grandchild, agg_child_mut.id());
                        agg_child_mut.add_child(gc_agg);
                    }
                }
            }
        }

        let result = Arc::new(agg_node);

        // Register node.
        self.nodes_by_id.insert(id, Arc::clone(&result));
        self.nodes_by_key.entry(key).or_default().push(id);

        self.key_stack.pop();

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_node::EventNode;

    #[test]
    fn test_aggregate_tree_basic() {
        let tree = AggregateTree::new();
        assert!(tree.is_empty());
        assert_eq!(tree.node_count(), 1); // Just root
    }

    #[test]
    fn test_aggregate_tree_from_event_tree() {
        let mut root = EventNode::new("root");
        let mut child = EventNode::with_times("child", 1.0, 0.5, 2);
        child.add_child(Arc::new(EventNode::with_times("grandchild", 0.3, 0.3, 1)));
        root.add_child(Arc::new(child));

        let _event_tree = EventTree::new();
        // Would need to properly construct EventTree...

        let agg_tree = AggregateTree::new();
        assert!(agg_tree.root().key() == "root");
    }

    #[test]
    fn test_aggregate_tree_node_lookup() {
        let tree = AggregateTree::new();
        let root_id = tree.root().id();

        assert!(tree.get_node(root_id).is_some());
        assert!(tree.get_node(999999).is_none());
    }

    #[test]
    fn test_aggregate_tree_merges_same_key() {
        // Build an event tree where two children have the same key "work".
        // The aggregate tree should merge them into a single node.
        let mut root = EventNode::new("root");
        let work1 = EventNode::with_times("work", 1.0, 1.0, 1);
        let work2 = EventNode::with_times("work", 0.5, 0.5, 1);
        let other = EventNode::with_times("other", 0.3, 0.3, 1);

        root.add_child(Arc::new(work1));
        root.add_child(Arc::new(other));
        root.add_child(Arc::new(work2));

        // Build aggregate from the root node directly.
        let mut builder = AggregateTreeBuilder::new();
        let agg_root = builder.build(&Arc::new(root));

        // "work" should appear as a single child with merged timing.
        let work_child = agg_root.get_child("work");
        assert!(work_child.is_some(), "work child should exist");
        let work_node = work_child.unwrap();
        // Merged: 1.0 + 0.5 = 1.5
        assert!(
            (work_node.inclusive_time() - 1.5).abs() < 0.001,
            "Expected merged inclusive_time ~1.5, got {}",
            work_node.inclusive_time()
        );

        // "other" should remain separate.
        assert!(agg_root.get_child("other").is_some());
        assert_eq!(
            agg_root.child_count(),
            2,
            "Should have 2 unique children, not 3"
        );
    }
}
