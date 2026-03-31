//! TraceAggregateNode - Aggregated timing data for a trace key.
//!
//! Port of pxr/base/trace/aggregateNode.h

use std::collections::HashMap;
use std::sync::Arc;

/// Type for time values (in seconds).
///
/// Note: C++ TraceAggregateNode uses `TraceEvent::TimeStamp` which is a u64
/// tick count, but computations convert to seconds. Our Rust port uses f64
/// seconds throughout the trace pipeline (EventNode, AggregateNode) for
/// consistency and simpler arithmetic.
pub type TimeStamp = f64;

/// Aggregated timing data for a specific trace key.
///
/// This node type aggregates timing data from multiple calls to the same
/// traced scope, combining them across all occurrences.
#[derive(Debug, Clone)]
pub struct AggregateNode {
    /// The key/name of this aggregated scope.
    key: String,
    /// Unique ID for this node.
    id: u64,
    /// Total inclusive time across all calls.
    inclusive_time: TimeStamp,
    /// Total exclusive time across all calls.
    exclusive_time: TimeStamp,
    /// Number of times this scope was entered.
    count: u64,
    /// Exclusive count (calls not in recursive context).
    exclusive_count: u64,
    /// Recursive count.
    recursive_count: u64,
    /// Child nodes by key.
    children: HashMap<String, Arc<AggregateNode>>,
    /// Children in order of addition (for ordered iteration).
    children_order: Vec<String>,
    /// Whether timing was expanded (recursive call detected).
    expanded: bool,
    /// Parent node ID (0 for root).
    parent_id: u64,
    /// Whether this node is a recursion marker.
    is_recursion_marker: bool,
    /// Whether this node is the head of a recursive call tree.
    is_recursion_head: bool,
    /// Counter values: (index -> (inclusive, exclusive)).
    counter_values: HashMap<i32, (f64, f64)>,
}

impl AggregateNode {
    /// Creates a new aggregate node.
    pub fn new(key: impl Into<String>, id: u64) -> Self {
        Self {
            key: key.into(),
            id,
            inclusive_time: 0.0,
            exclusive_time: 0.0,
            count: 0,
            exclusive_count: 0,
            recursive_count: 0,
            children: HashMap::new(),
            children_order: Vec::new(),
            expanded: false,
            parent_id: 0,
            is_recursion_marker: false,
            is_recursion_head: false,
            counter_values: HashMap::new(),
        }
    }

    /// Creates a new aggregate node with initial timing data.
    ///
    /// Matches C++ `New(key, ts, count, exclusiveCount)`.
    pub fn new_with_time(
        key: impl Into<String>,
        id: u64,
        ts: TimeStamp,
        count: u64,
        exclusive_count: u64,
    ) -> Self {
        Self {
            key: key.into(),
            id,
            inclusive_time: ts,
            exclusive_time: ts,
            count,
            exclusive_count,
            recursive_count: count,
            children: HashMap::new(),
            children_order: Vec::new(),
            expanded: false,
            parent_id: 0,
            is_recursion_marker: false,
            is_recursion_head: false,
            counter_values: HashMap::new(),
        }
    }

    /// Returns the key/name.
    #[inline]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Returns the unique ID.
    #[inline]
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Returns total inclusive time.
    #[inline]
    pub fn inclusive_time(&self) -> TimeStamp {
        self.inclusive_time
    }

    /// Returns total exclusive time.
    #[inline]
    pub fn exclusive_time(&self) -> TimeStamp {
        self.exclusive_time
    }

    /// Returns the call count.
    #[inline]
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Returns average inclusive time per call.
    pub fn avg_inclusive_time(&self) -> TimeStamp {
        if self.count > 0 {
            self.inclusive_time / self.count as f64
        } else {
            0.0
        }
    }

    /// Returns average exclusive time per call.
    pub fn avg_exclusive_time(&self) -> TimeStamp {
        if self.count > 0 {
            self.exclusive_time / self.count as f64
        } else {
            0.0
        }
    }

    /// Returns whether this node was expanded (recursive).
    #[inline]
    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    /// Sets the expanded flag.
    #[inline]
    pub fn set_expanded(&mut self, expanded: bool) {
        self.expanded = expanded;
    }

    /// Returns the parent ID.
    #[inline]
    pub fn parent_id(&self) -> u64 {
        self.parent_id
    }

    /// Sets the parent ID.
    #[inline]
    pub fn set_parent_id(&mut self, id: u64) {
        self.parent_id = id;
    }

    /// Returns child nodes.
    pub fn children(&self) -> impl Iterator<Item = &Arc<AggregateNode>> {
        self.children_order
            .iter()
            .filter_map(|k| self.children.get(k))
    }

    /// Returns the number of children.
    #[inline]
    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    /// Gets a child by key (immutable).
    pub fn get_child(&self, key: &str) -> Option<&Arc<AggregateNode>> {
        self.children.get(key)
    }

    /// Gets a mutable reference to a child Arc by key.
    ///
    /// Allows callers to use `Arc::get_mut` on the result if they hold
    /// the only reference.
    pub fn get_child_arc_mut(&mut self, key: &str) -> Option<&mut Arc<AggregateNode>> {
        self.children.get_mut(key)
    }

    /// Adds timing data.
    pub fn add_time(&mut self, inclusive: TimeStamp, exclusive: TimeStamp, count: u64) {
        self.inclusive_time += inclusive;
        self.exclusive_time += exclusive;
        self.count += count;
    }

    /// Adds a child node.
    pub fn add_child(&mut self, node: Arc<AggregateNode>) {
        let key = node.key.clone();
        if !self.children.contains_key(&key) {
            self.children_order.push(key.clone());
        }
        self.children.insert(key, node);
    }

    /// Merges another node's data into this one.
    pub fn merge(&mut self, other: &AggregateNode) {
        self.inclusive_time += other.inclusive_time;
        self.exclusive_time += other.exclusive_time;
        self.count += other.count;
    }

    /// Returns total number of nodes in subtree.
    pub fn subtree_size(&self) -> usize {
        1 + self
            .children
            .values()
            .map(|c| c.subtree_size())
            .sum::<usize>()
    }

    /// Returns true if this is a leaf node.
    #[inline]
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    // =========================================================================
    // Counter Value Methods (C++ parity)
    // =========================================================================

    /// Appends an inclusive counter value.
    ///
    /// Matches C++ `AppendInclusiveCounterValue(int index, double value)`.
    pub fn append_inclusive_counter_value(&mut self, index: i32, value: f64) {
        let entry = self.counter_values.entry(index).or_insert((0.0, 0.0));
        entry.0 += value;
    }

    /// Returns the inclusive counter value for the given index.
    ///
    /// Matches C++ `GetInclusiveCounterValue(int index)`.
    pub fn get_inclusive_counter_value(&self, index: i32) -> f64 {
        self.counter_values.get(&index).map(|v| v.0).unwrap_or(0.0)
    }

    /// Appends an exclusive counter value.
    ///
    /// Matches C++ `AppendExclusiveCounterValue(int index, double value)`.
    pub fn append_exclusive_counter_value(&mut self, index: i32, value: f64) {
        let entry = self.counter_values.entry(index).or_insert((0.0, 0.0));
        entry.1 += value;
    }

    /// Returns the exclusive counter value for the given index.
    ///
    /// Matches C++ `GetExclusiveCounterValue(int index)`.
    pub fn get_exclusive_counter_value(&self, index: i32) -> f64 {
        self.counter_values.get(&index).map(|v| v.1).unwrap_or(0.0)
    }

    /// Recursively calculates the inclusive counter values from the inclusive
    /// and exclusive counts of child nodes.
    ///
    /// Matches C++ `CalculateInclusiveCounterValues()`.
    pub fn calculate_inclusive_counter_values(&mut self) {
        // First, recursively calculate for children
        // Note: We need to collect keys first to avoid borrow issues
        let child_keys: Vec<String> = self.children_order.clone();

        for key in &child_keys {
            if let Some(child) = self.children.get_mut(key) {
                // We need to get mutable access to child
                if let Some(child_mut) = Arc::get_mut(child) {
                    child_mut.calculate_inclusive_counter_values();
                }
            }
        }

        // Then sum up child inclusive values into our inclusive values
        for child in self.children.values() {
            for (&index, &(child_inclusive, _)) in &child.counter_values {
                let entry = self.counter_values.entry(index).or_insert((0.0, 0.0));
                entry.0 += child_inclusive;
            }
        }
    }

    // =========================================================================
    // Recursion Detection Methods (C++ parity)
    // =========================================================================

    /// Returns true if this node is simply a marker for a merged recursive subtree.
    ///
    /// Matches C++ `IsRecursionMarker()`.
    #[inline]
    pub fn is_recursion_marker(&self) -> bool {
        self.is_recursion_marker
    }

    /// Returns true if this node is the head of a recursive call tree.
    ///
    /// Matches C++ `IsRecursionHead()`.
    #[inline]
    pub fn is_recursion_head(&self) -> bool {
        self.is_recursion_head
    }

    /// Returns the count, optionally including recursive calls.
    ///
    /// Matches C++ `GetCount(bool recursive)`.
    pub fn get_count(&self, recursive: bool) -> u64 {
        if recursive {
            self.recursive_count
        } else {
            self.count
        }
    }

    /// Returns the exclusive count.
    ///
    /// Matches C++ `GetExclusiveCount()`.
    #[inline]
    pub fn get_exclusive_count(&self) -> u64 {
        self.exclusive_count
    }

    /// Scans the tree for recursive calls and updates the recursive counts.
    ///
    /// Matches C++ `MarkRecursiveChildren()`.
    pub fn mark_recursive_children(&mut self) {
        self.mark_recursive_children_impl(&mut std::collections::HashSet::new());
    }

    fn mark_recursive_children_impl(&mut self, seen_keys: &mut std::collections::HashSet<String>) {
        // Check if we've seen this key before (recursive call)
        if seen_keys.contains(&self.key) {
            self.is_recursion_marker = true;
            return;
        }

        // Mark this key as seen
        seen_keys.insert(self.key.clone());

        // Check if any child has the same key (making us a recursion head)
        for child in self.children.values() {
            if child.key == self.key {
                // Can't modify self while iterating, mark after
            }
        }

        // Check children
        let child_keys: Vec<String> = self.children_order.clone();
        for key in &child_keys {
            if let Some(child) = self.children.get_mut(key) {
                if let Some(child_mut) = Arc::get_mut(child) {
                    child_mut.mark_recursive_children_impl(seen_keys);
                    if child_mut.key == self.key {
                        self.is_recursion_head = true;
                    }
                }
            }
        }

        // Remove this key from seen set when leaving
        seen_keys.remove(&self.key);
    }

    /// Appends a child node with the given key and timing data.
    ///
    /// Matches C++ `Append(key, ts, count, exclusiveCount)`.
    pub fn append(
        &mut self,
        key: impl Into<String>,
        ts: TimeStamp,
        count: u64,
        exclusive_count: u64,
        id: u64,
    ) -> Arc<AggregateNode> {
        let key = key.into();

        if let Some(existing) = self.children.get_mut(&key) {
            // Merge into existing child
            if let Some(child_mut) = Arc::get_mut(existing) {
                child_mut.inclusive_time += ts;
                child_mut.exclusive_time += ts;
                child_mut.count += count;
                child_mut.exclusive_count += exclusive_count;
            }
            return existing.clone();
        }

        // Create new child
        let child = Arc::new(AggregateNode::new_with_time(
            &key,
            id,
            ts,
            count,
            exclusive_count,
        ));
        self.children_order.push(key.clone());
        self.children.insert(key, child.clone());
        child
    }

    /// Adjust for overhead and noise.
    ///
    /// Subtracts scope overhead cost times the number of descendant nodes
    /// from the inclusive time of each node.
    ///
    /// Matches C++ `AdjustForOverheadAndNoise(scopeOverhead, timerQuantum, numDescendantNodes)`.
    pub fn adjust_for_overhead_and_noise(
        &mut self,
        scope_overhead: TimeStamp,
        timer_quantum: TimeStamp,
    ) -> u64 {
        let mut num_descendants: u64 = 0;

        // Recursively adjust children first
        let child_keys: Vec<String> = self.children_order.clone();
        for key in &child_keys {
            if let Some(child) = self.children.get_mut(key) {
                if let Some(child_mut) = Arc::get_mut(child) {
                    num_descendants +=
                        1 + child_mut.adjust_for_overhead_and_noise(scope_overhead, timer_quantum);
                }
            }
        }

        // Subtract overhead from our inclusive time
        let overhead = scope_overhead * num_descendants as f64;
        self.inclusive_time = (self.inclusive_time - overhead).max(0.0);

        // If we're too noisy relative to the timer quantum, zero out our time
        if timer_quantum > 0.0 && self.inclusive_time < timer_quantum * 2.0 && num_descendants > 0 {
            self.inclusive_time = 0.0;
            self.exclusive_time = 0.0;
        }

        num_descendants
    }
}

impl Default for AggregateNode {
    fn default() -> Self {
        Self::new("", 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aggregate_node_basic() {
        let mut node = AggregateNode::new("test", 1);

        node.add_time(1.0, 0.5, 2);
        node.add_time(0.5, 0.25, 1);

        assert_eq!(node.inclusive_time(), 1.5);
        assert_eq!(node.exclusive_time(), 0.75);
        assert_eq!(node.count(), 3);
        assert_eq!(node.avg_inclusive_time(), 0.5);
        assert_eq!(node.avg_exclusive_time(), 0.25);
    }

    #[test]
    fn test_aggregate_node_children() {
        let mut parent = AggregateNode::new("parent", 1);
        let child1 = Arc::new(AggregateNode::new("child1", 2));
        let child2 = Arc::new(AggregateNode::new("child2", 3));

        parent.add_child(child1);
        parent.add_child(child2);

        assert_eq!(parent.child_count(), 2);
        assert!(parent.get_child("child1").is_some());
        assert!(parent.get_child("child2").is_some());
    }

    #[test]
    fn test_aggregate_node_merge() {
        let mut node1 = AggregateNode::new("test", 1);
        node1.add_time(1.0, 0.5, 2);

        let mut node2 = AggregateNode::new("test", 2);
        node2.add_time(0.5, 0.25, 1);

        node1.merge(&node2);

        assert_eq!(node1.inclusive_time(), 1.5);
        assert_eq!(node1.exclusive_time(), 0.75);
        assert_eq!(node1.count(), 3);
    }
}
