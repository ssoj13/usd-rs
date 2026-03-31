//! Event tree builder.
//!
//! Port of pxr/base/trace/eventTreeBuilder.h/cpp
//!
//! This module provides the FullEventTreeBuilder class which creates
//! a tree of TraceEventNodes from TraceCollection instances.

use super::CategoryId;
use super::collection::Collection;
use super::counter_accumulator::{CounterAccumulator, CounterMap};
use super::event::{Event, EventType};
use super::event_node::{EventNode, TimeStamp};
use super::threads::ThreadId;
use std::collections::HashMap;
use std::sync::Arc;

// ============================================================================
// Marker Values
// ============================================================================

/// A marker value is a timestamp and thread ID pair.
pub type MarkerValue = (TimeStamp, ThreadId);

/// Map from marker key to list of marker values.
pub type MarkerValuesMap = HashMap<String, Vec<MarkerValue>>;

/// Attribute data stored in pending nodes.
#[derive(Debug, Clone)]
pub enum AttributeData {
    /// String attribute.
    String(String),
    /// Integer attribute.
    Int(i64),
    /// Float attribute.
    Float(f64),
    /// Boolean attribute.
    Bool(bool),
}

// ============================================================================
// Pending Event Node
// ============================================================================

/// Helper structure for building event graph.
///
/// Represents a node that hasn't been finalized yet.
struct PendingEventNode {
    /// The key/name of the event.
    key: String,
    /// The category ID.
    category: CategoryId,
    /// Start timestamp.
    start: TimeStamp,
    /// End timestamp.
    end: TimeStamp,
    /// Whether begin and end were separate events.
    separate_events: bool,
    /// Whether this node is complete (has both begin and end).
    is_complete: bool,
    /// Children nodes.
    children: Vec<Arc<EventNode>>,
    /// Attributes to attach to the node.
    attributes: Vec<PendingAttribute>,
}

/// Pending attribute data.
struct PendingAttribute {
    time: TimeStamp,
    key: String,
    data: AttributeData,
}

impl PendingEventNode {
    /// Creates a new pending event node.
    fn new(
        key: impl Into<String>,
        category: CategoryId,
        start: TimeStamp,
        end: TimeStamp,
        separate_events: bool,
        is_complete: bool,
    ) -> Self {
        Self {
            key: key.into(),
            category,
            start,
            end,
            separate_events,
            is_complete,
            children: Vec::new(),
            attributes: Vec::new(),
        }
    }

    /// Closes this pending node and creates a finalized EventNode.
    fn close(mut self) -> Arc<EventNode> {
        // Reverse children and attributes (built in reverse order)
        self.children.reverse();
        self.attributes.reverse();

        // Sort attributes by time for deterministic ordering
        self.attributes.sort_by(|a, b| {
            a.time
                .partial_cmp(&b.time)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut node = EventNode::with_times(
            &self.key,
            self.end - self.start, // inclusive = duration
            self.end - self.start, // exclusive starts equal, adjusted later
            1,
        );

        // Set category from pending node
        node.set_category(self.category);

        // Add children
        for child in self.children {
            node.add_child(child);
        }

        // Add attributes with timestamp annotation
        for attr in self.attributes {
            let value = format!("{}@{:.6}", format!("{:?}", attr.data), attr.time);
            node.set_attribute(&attr.key, value);
        }

        Arc::new(node)
    }
}

// ============================================================================
// Event Tree Builder
// ============================================================================

/// Creates a tree of TraceEventNodes from TraceCollection instances.
///
/// This builder processes events in reverse order to efficiently
/// match begin/end pairs and build the tree structure.
pub struct FullEventTreeBuilder {
    /// Root node of the tree.
    root: Arc<EventNode>,
    /// Per-thread stacks of pending nodes.
    thread_stacks: HashMap<ThreadId, Vec<PendingEventNode>>,
    /// Counter accumulator.
    counter_accum: CounterAccumulator,
    /// Marker values map.
    markers_map: MarkerValuesMap,
}

impl FullEventTreeBuilder {
    /// Creates a new event tree builder.
    pub fn new() -> Self {
        Self {
            root: Arc::new(EventNode::new("root")),
            thread_stacks: HashMap::new(),
            counter_accum: CounterAccumulator::new(),
            markers_map: HashMap::new(),
        }
    }

    /// Returns the built tree root.
    pub fn root(&self) -> &Arc<EventNode> {
        &self.root
    }

    /// Returns the counter values.
    pub fn counters(&self) -> &CounterMap {
        self.counter_accum.counters()
    }

    /// Returns the markers map.
    pub fn markers(&self) -> &MarkerValuesMap {
        &self.markers_map
    }

    /// Sets initial counter values.
    pub fn set_counter_values(&mut self, values: CounterMap) {
        self.counter_accum.set_current_values(values);
    }

    /// Creates an event tree from a collection.
    ///
    /// Processes the collection's events in reverse order.
    pub fn create_tree(&mut self, collection: &Collection) {
        self.on_begin_collection();

        for (thread_id, event_list) in collection.iter() {
            self.on_begin_thread(thread_id);

            // Process events in reverse order
            for event in event_list.iter_rev() {
                self.on_event(thread_id, event);
            }

            self.on_end_thread(thread_id);
        }

        self.on_end_collection();

        // Update counters
        self.counter_accum.update(collection);
    }

    /// Called at the start of collection processing.
    fn on_begin_collection(&mut self) {
        // Nothing to do
    }

    /// Called at the end of collection processing.
    fn on_end_collection(&mut self) {
        self.thread_stacks.clear();

        // Sort marker timestamps for each key
        for markers in self.markers_map.values_mut() {
            markers.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        }
    }

    /// Called at the start of a thread's events.
    fn on_begin_thread(&mut self, thread_id: &ThreadId) {
        let mut stack = Vec::new();
        // Push thread root node
        stack.push(PendingEventNode::new(
            thread_id.to_string(),
            CategoryId::default(),
            0.0,
            0.0,
            false,
            true,
        ));
        self.thread_stacks.insert(thread_id.clone(), stack);
    }

    /// Called at the end of a thread's events.
    fn on_end_thread(&mut self, thread_id: &ThreadId) {
        if let Some(mut stack) = self.thread_stacks.remove(thread_id) {
            let mut first_node: Option<Arc<EventNode>> = None;

            // Close any incomplete nodes
            while let Some(back_node) = stack.pop() {
                let was_complete = back_node.is_complete;
                let mut node = back_node.close();

                // If incomplete, set times from children
                if !was_complete {
                    if let Some(n) = Arc::get_mut(&mut node) {
                        n.set_begin_and_end_from_children();
                    }
                }

                if !stack.is_empty() {
                    stack
                        .last_mut()
                        .expect("stack not empty")
                        .children
                        .push(node.clone());
                }
                first_node = Some(node);
            }

            // Set times on thread root from children
            if let Some(ref mut node) = first_node {
                if let Some(n) = Arc::get_mut(node) {
                    n.set_begin_and_end_from_children();
                }
            }

            // Append thread root to global root
            if let Some(thread_node) = first_node {
                if let Some(root) = Arc::get_mut(&mut self.root) {
                    root.add_child(thread_node);
                }
            }
        }
    }

    /// Process a single event.
    fn on_event(&mut self, thread_id: &ThreadId, event: &Event) {
        match &event.event_type {
            EventType::Begin => self.on_begin(thread_id, event),
            EventType::End => self.on_end(thread_id, event),
            EventType::Timespan(duration) => self.on_timespan(thread_id, event, *duration),
            EventType::Marker => self.on_marker(thread_id, event),
            EventType::Data(data) => self.on_data(thread_id, event, data),
            EventType::ScopeData(data) => {
                // Typed data - format for display and store same as Data
                self.on_data(thread_id, event, &data.to_string())
            }
            EventType::CounterDelta(_) | EventType::CounterValue(_) => {
                // Handled by counter accumulator
            }
        }
    }

    /// Process a Begin event (working in reverse, so this is actually "end" in time).
    fn on_begin(&mut self, thread_id: &ThreadId, event: &Event) {
        let stack = match self.thread_stacks.get_mut(thread_id) {
            Some(s) => s,
            None => return,
        };

        let key = event.key();

        // Find matching End event on the stack
        let mut index = stack.len().saturating_sub(1);

        while index > 0 {
            let node = &stack[index];
            if !node.is_complete && node.key == key {
                break;
            }
            if node.is_complete {
                // Pop and close completed nodes
                let closed = stack.remove(index).close();
                if index > 0 {
                    stack[index - 1].children.push(closed);
                }
                index = stack.len().saturating_sub(1);
            } else {
                index -= 1;
            }
        }

        let stack = self
            .thread_stacks
            .get_mut(thread_id)
            .expect("thread stack exists");

        // Check if we found a match
        if !stack.is_empty() && (index > 0 || stack[index].key == key) && !stack[index].is_complete
        {
            let node = &mut stack[index];
            node.start = event.timestamp_seconds();
            node.separate_events = true;
            node.is_complete = true;
        } else {
            // No matching End - this is an incomplete scope
            // Create a node with children taken from current top
            let mut pending =
                PendingEventNode::new(key, CategoryId::default(), 0.0, 0.0, true, false);

            if !stack.is_empty() {
                std::mem::swap(
                    &mut pending.children,
                    &mut stack.last_mut().expect("stack not empty").children,
                );
                std::mem::swap(
                    &mut pending.attributes,
                    &mut stack.last_mut().expect("stack not empty").attributes,
                );
            }

            let mut node = pending.close();
            if let Some(n) = Arc::get_mut(&mut node) {
                n.set_begin_and_end_from_children();
            }

            if !stack.is_empty() {
                stack
                    .last_mut()
                    .expect("stack not empty")
                    .children
                    .push(node);
            }
        }
    }

    /// Process an End event (working in reverse, so this is actually "begin" in time).
    fn on_end(&mut self, thread_id: &ThreadId, event: &Event) {
        let key = event.key().to_string();

        let stack = match self.thread_stacks.get_mut(thread_id) {
            Some(s) => s,
            None => return,
        };

        // Pop completed nodes that this End cannot be a child of
        while stack.len() > 1 {
            let prev = stack.last().expect("stack not empty");
            if prev.is_complete && event.timestamp_seconds() <= prev.start {
                let closed = stack.pop().expect("stack not empty").close();
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(closed);
                }
                continue;
            }
            break;
        }

        // Push new pending node for this End
        stack.push(PendingEventNode::new(
            key,
            CategoryId::default(),
            0.0, // temporary, will be set by matching Begin
            event.timestamp_seconds(),
            true,
            false,
        ));
    }

    /// Process a Timespan event.
    fn on_timespan(&mut self, thread_id: &ThreadId, event: &Event, duration_ns: u64) {
        let start = event.timestamp_seconds() - (duration_ns as f64 / 1_000_000_000.0);
        let end = event.timestamp_seconds();

        let key = event.key().to_string();

        let stack = match self.thread_stacks.get_mut(thread_id) {
            Some(s) => s,
            None => return,
        };

        // Pop nodes that this timespan is not a child of
        while stack.len() > 1 {
            let prev = stack.last().expect("stack not empty");
            if start < prev.start || end > prev.end {
                let closed = stack.pop().expect("stack not empty").close();
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(closed);
                }
                continue;
            }
            break;
        }

        // Add this timespan as a complete node
        stack.push(PendingEventNode::new(
            key,
            CategoryId::default(),
            start,
            end,
            false,
            true,
        ));
    }

    /// Process a Marker event.
    fn on_marker(&mut self, thread_id: &ThreadId, event: &Event) {
        self.markers_map
            .entry(event.key().to_string())
            .or_default()
            .push((event.timestamp_seconds(), thread_id.clone()));
    }

    /// Process a Data event.
    fn on_data(&mut self, thread_id: &ThreadId, event: &Event, data: &str) {
        let key = event.key().to_string();

        let stack = match self.thread_stacks.get_mut(thread_id) {
            Some(s) => s,
            None => return,
        };

        if stack.is_empty() {
            return;
        }

        // Find the node this data belongs to
        while stack.len() > 1 {
            let prev = stack.last().expect("stack not empty");
            let ts = event.timestamp_seconds();
            if ts < prev.start || ts > prev.end {
                let closed = stack.pop().expect("stack not empty").close();
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(closed);
                }
                continue;
            }
            break;
        }

        // Add attribute to current node
        if let Some(current) = stack.last_mut() {
            current.attributes.push(PendingAttribute {
                time: event.timestamp_seconds(),
                key,
                data: AttributeData::String(data.to_string()),
            });
        }
    }
}

impl Default for FullEventTreeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EventList;

    #[test]
    fn test_builder_empty() {
        let builder = FullEventTreeBuilder::new();
        assert_eq!(builder.root().name(), "root");
    }

    #[test]
    fn test_builder_with_events() {
        let mut collection = Collection::new();
        let mut events = EventList::new();

        events.push(Event::begin("outer", 0));
        events.push(Event::begin("inner", 100_000_000));
        events.push(Event::end("inner", 200_000_000));
        events.push(Event::end("outer", 500_000_000));

        collection.add_to_collection(ThreadId::new("thread1"), events);

        let mut builder = FullEventTreeBuilder::new();
        builder.create_tree(&collection);

        // Should have one thread child
        assert!(!builder.root().children().is_empty());
    }

    #[test]
    fn test_builder_markers() {
        let mut collection = Collection::new();
        let mut events = EventList::new();

        events.push(Event::new("marker1", EventType::Marker, 100_000_000));
        events.push(Event::new("marker1", EventType::Marker, 200_000_000));
        events.push(Event::new("marker2", EventType::Marker, 300_000_000));

        collection.add_to_collection(ThreadId::new("thread1"), events);

        let mut builder = FullEventTreeBuilder::new();
        builder.create_tree(&collection);

        assert_eq!(builder.markers().len(), 2);
        assert_eq!(builder.markers()["marker1"].len(), 2);
        assert_eq!(builder.markers()["marker2"].len(), 1);
    }
}
