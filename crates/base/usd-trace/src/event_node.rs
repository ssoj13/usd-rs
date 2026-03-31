//! TraceEventNode - Node in an event call tree.
//!
//! Port of pxr/base/trace/eventNode.h

use std::collections::HashMap;
use std::sync::Arc;

/// Type for time values (in seconds).
pub type TimeStamp = f64;

/// A node in the call tree representing a traced scope.
#[derive(Debug, Clone)]
pub struct EventNode {
    /// The scope/event key.
    key: String,
    /// Category of this event.
    category: u32,
    /// Inclusive time (includes children).
    inclusive_time: TimeStamp,
    /// Exclusive time (excludes children).
    exclusive_time: TimeStamp,
    /// Number of times this scope was entered.
    count: u64,
    /// Child nodes.
    children: Vec<Arc<EventNode>>,
    /// Associated attributes/data.
    attributes: HashMap<String, String>,
}

impl EventNode {
    /// Creates a new event node.
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            category: 0,
            inclusive_time: 0.0,
            exclusive_time: 0.0,
            count: 0,
            children: Vec::new(),
            attributes: HashMap::new(),
        }
    }

    /// Creates a node with timing info.
    pub fn with_times(
        key: impl Into<String>,
        inclusive: TimeStamp,
        exclusive: TimeStamp,
        count: u64,
    ) -> Self {
        Self {
            key: key.into(),
            category: 0,
            inclusive_time: inclusive,
            exclusive_time: exclusive,
            count,
            children: Vec::new(),
            attributes: HashMap::new(),
        }
    }

    /// Returns the key/name of this node.
    #[inline]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Returns the name of this node (alias for key).
    #[inline]
    pub fn name(&self) -> &str {
        &self.key
    }

    /// Returns the category.
    #[inline]
    pub fn category(&self) -> u32 {
        self.category
    }

    /// Sets the category.
    #[inline]
    pub fn set_category(&mut self, category: u32) {
        self.category = category;
    }

    /// Returns inclusive time (including children).
    #[inline]
    pub fn inclusive_time(&self) -> TimeStamp {
        self.inclusive_time
    }

    /// Returns exclusive time (excluding children).
    #[inline]
    pub fn exclusive_time(&self) -> TimeStamp {
        self.exclusive_time
    }

    /// Returns the count (number of times entered).
    #[inline]
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Returns the child nodes.
    #[inline]
    pub fn children(&self) -> &[Arc<EventNode>] {
        &self.children
    }

    /// Returns the number of children.
    #[inline]
    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    /// Adds a child node.
    pub fn add_child(&mut self, child: Arc<EventNode>) {
        self.children.push(child);
    }

    /// Sets the inclusive time.
    #[inline]
    pub fn set_inclusive_time(&mut self, time: TimeStamp) {
        self.inclusive_time = time;
    }

    /// Sets the exclusive time.
    #[inline]
    pub fn set_exclusive_time(&mut self, time: TimeStamp) {
        self.exclusive_time = time;
    }

    /// Sets the count.
    #[inline]
    pub fn set_count(&mut self, count: u64) {
        self.count = count;
    }

    /// Adds to the inclusive time.
    #[inline]
    pub fn add_inclusive_time(&mut self, time: TimeStamp) {
        self.inclusive_time += time;
    }

    /// Adds to the exclusive time.
    #[inline]
    pub fn add_exclusive_time(&mut self, time: TimeStamp) {
        self.exclusive_time += time;
    }

    /// Increments the count.
    #[inline]
    pub fn increment_count(&mut self) {
        self.count += 1;
    }

    /// Returns the attributes.
    #[inline]
    pub fn attributes(&self) -> &HashMap<String, String> {
        &self.attributes
    }

    /// Sets an attribute.
    pub fn set_attribute(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.attributes.insert(key.into(), value.into());
    }

    /// Gets an attribute value.
    pub fn get_attribute(&self, key: &str) -> Option<&str> {
        self.attributes.get(key).map(String::as_str)
    }

    /// Returns true if this is a leaf node (no children).
    #[inline]
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    /// Calculates the total number of nodes in the subtree.
    pub fn subtree_size(&self) -> usize {
        1 + self
            .children
            .iter()
            .map(|c| c.subtree_size())
            .sum::<usize>()
    }

    /// Calculates total inclusive time of all children.
    pub fn children_time(&self) -> TimeStamp {
        self.children.iter().map(|c| c.inclusive_time).sum()
    }

    /// Sets the begin and end times based on children's times.
    ///
    /// This is used for incomplete nodes that didn't have explicit begin/end events.
    /// Sets the inclusive time to the sum of children's inclusive times.
    pub fn set_begin_and_end_from_children(&mut self) {
        if self.children.is_empty() {
            return;
        }

        let total_children_time = self.children_time();
        self.inclusive_time = total_children_time;
        self.exclusive_time = 0.0; // No exclusive time for incomplete nodes
    }
}

impl Default for EventNode {
    fn default() -> Self {
        Self::new("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_node_basic() {
        let node = EventNode::with_times("test", 1.5, 0.5, 3);

        assert_eq!(node.key(), "test");
        assert_eq!(node.inclusive_time(), 1.5);
        assert_eq!(node.exclusive_time(), 0.5);
        assert_eq!(node.count(), 3);
    }

    #[test]
    fn test_event_node_children() {
        let mut parent = EventNode::new("parent");
        let child1 = Arc::new(EventNode::with_times("child1", 0.5, 0.5, 1));
        let child2 = Arc::new(EventNode::with_times("child2", 0.3, 0.3, 1));

        parent.add_child(child1);
        parent.add_child(child2);

        assert_eq!(parent.child_count(), 2);
        assert!(!parent.is_leaf());
        assert_eq!(parent.children_time(), 0.8);
    }

    #[test]
    fn test_event_node_attributes() {
        let mut node = EventNode::new("test");
        node.set_attribute("color", "red");

        assert_eq!(node.get_attribute("color"), Some("red"));
        assert_eq!(node.get_attribute("size"), None);
    }

    #[test]
    fn test_subtree_size() {
        let mut root = EventNode::new("root");
        let mut child = EventNode::new("child");
        let grandchild = Arc::new(EventNode::new("grandchild"));

        child.add_child(grandchild);
        root.add_child(Arc::new(child));

        assert_eq!(root.subtree_size(), 3);
    }
}
