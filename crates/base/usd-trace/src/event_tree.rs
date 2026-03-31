//! TraceEventTree - Tree representation of trace events.
//!
//! Port of pxr/base/trace/eventTree.h

use super::collection::Collection;
use super::counter_accumulator::CounterMap;
use super::event::EventType;
use super::event_node::{EventNode, TimeStamp};
use super::threads::ThreadId;
use std::collections::HashMap;
use std::sync::Arc;

/// Counter values over time: Vec<(timestamp_seconds, value)>.
pub type CounterValues = Vec<(TimeStamp, f64)>;

/// Map of counter name -> time-series values.
pub type CounterValuesMap = HashMap<String, CounterValues>;

/// Marker values: Vec<(timestamp_seconds, thread_id)>.
pub type MarkerValues = Vec<(TimeStamp, ThreadId)>;

/// Map of marker name -> occurrence list.
pub type MarkerValuesMap = HashMap<String, MarkerValues>;

/// Tree representation of trace events.
///
/// Provides a tree representation of the data in a TraceCollection.
/// Each thread's call stacks are represented as trees of EventNodes.
/// Also tracks counter time-series and marker events (matching C++ TraceEventTree).
#[derive(Debug, Default)]
pub struct EventTree {
    /// Root node of the tree (contains all threads as children).
    root: Arc<EventNode>,
    /// Counter values accumulated from the events.
    counters: CounterMap,
    /// Per-thread root nodes.
    thread_roots: HashMap<ThreadId, Arc<EventNode>>,
    /// Counter time-series values: name -> [(timestamp, value)].
    counter_values: CounterValuesMap,
    /// Marker events: name -> [(timestamp, thread_id)].
    markers: MarkerValuesMap,
}

impl EventTree {
    /// Creates a new empty event tree.
    pub fn new() -> Self {
        Self {
            root: Arc::new(EventNode::new("root")),
            counters: CounterMap::new(),
            thread_roots: HashMap::new(),
            counter_values: CounterValuesMap::new(),
            markers: MarkerValuesMap::new(),
        }
    }

    /// Creates an event tree from a collection.
    pub fn from_collection(collection: &Collection) -> Self {
        let mut builder = EventTreeBuilder::new();
        builder.build(collection)
    }

    /// Returns the root node.
    #[inline]
    pub fn root(&self) -> &Arc<EventNode> {
        &self.root
    }

    /// Returns the counter values.
    #[inline]
    pub fn counters(&self) -> &CounterMap {
        &self.counters
    }

    /// Returns the root node for a specific thread.
    pub fn thread_root(&self, thread_id: ThreadId) -> Option<&Arc<EventNode>> {
        self.thread_roots.get(&thread_id)
    }

    /// Returns all thread IDs in the tree.
    pub fn thread_ids(&self) -> impl Iterator<Item = &ThreadId> {
        self.thread_roots.keys()
    }

    /// Returns the total number of nodes in the tree.
    pub fn node_count(&self) -> usize {
        self.root.subtree_size()
    }

    /// Returns the counter time-series values.
    #[inline]
    pub fn counter_values(&self) -> &CounterValuesMap {
        &self.counter_values
    }

    /// Returns the marker events map.
    ///
    /// Matches C++ `TraceEventTree::GetMarkers()`.
    #[inline]
    pub fn markers(&self) -> &MarkerValuesMap {
        &self.markers
    }

    /// Returns the final value of each counter (last value in the time series).
    ///
    /// Matches C++ `TraceEventTree::GetFinalCounterValues()`.
    pub fn get_final_counter_values(&self) -> HashMap<String, f64> {
        let mut result = HashMap::new();
        for (name, values) in &self.counter_values {
            if let Some(last) = values.last() {
                result.insert(name.clone(), last.1);
            }
        }
        result
    }

    /// Returns true if the tree is empty.
    pub fn is_empty(&self) -> bool {
        self.thread_roots.is_empty()
    }

    /// Writes a JSON object representing the data in Chrome Trace format.
    ///
    /// Matches C++ `TraceEventTree::WriteChromeTraceObject()`.
    /// The output can be loaded by Chrome's chrome://tracing viewer.
    pub fn write_chrome_trace_object(
        &self,
        writer: &mut dyn std::io::Write,
    ) -> std::io::Result<()> {
        let pid = 0;

        write!(writer, "{{\"traceEvents\":[")?;
        let mut first = true;

        // Write call tree events per thread.
        for (thread_id, thread_node) in &self.thread_roots {
            for child in thread_node.children() {
                self.write_node_chrome(writer, child, pid, thread_id, &mut first)?;
            }
        }

        // Write counter events.
        for (name, values) in &self.counter_values {
            for (ts, value) in values {
                if !first {
                    write!(writer, ",")?;
                }
                first = false;
                // Chrome counter event (ph: "C").
                // Timestamp in microseconds.
                let ts_us = ts * 1_000_000.0;
                write!(
                    writer,
                    "{{\"cat\":\"\",\"tid\":0,\"pid\":{pid},\"name\":{name_json},\"ph\":\"C\",\"ts\":{ts_us},\"args\":{{\"value\":{value}}}}}",
                    pid = pid,
                    name_json = json_string(name),
                    ts_us = ts_us,
                    value = value,
                )?;
            }
        }

        // Write marker events as instant events.
        for (name, values) in &self.markers {
            for (ts, thread_id) in values {
                if !first {
                    write!(writer, ",")?;
                }
                first = false;
                let ts_us = ts * 1_000_000.0;
                write!(
                    writer,
                    "{{\"cat\":\"\",\"tid\":{tid},\"pid\":{pid},\"name\":{name_json},\"ph\":\"I\",\"s\":\"t\",\"ts\":{ts_us}}}",
                    tid = json_string(&thread_id.to_string()),
                    pid = pid,
                    name_json = json_string(name),
                    ts_us = ts_us,
                )?;
            }
        }

        write!(writer, "]}}")?;
        Ok(())
    }

    /// Writes a single EventNode as Chrome Trace JSON (recursive).
    fn write_node_chrome(
        &self,
        writer: &mut dyn std::io::Write,
        node: &EventNode,
        pid: i32,
        thread_id: &ThreadId,
        first: &mut bool,
    ) -> std::io::Result<()> {
        if !*first {
            write!(writer, ",")?;
        }
        *first = false;

        let ts_us = node.inclusive_time() * 1_000_000.0; // placeholder start
        let dur_us = node.inclusive_time() * 1_000_000.0;

        // Write as complete event (ph: "X").
        write!(
            writer,
            "{{\"cat\":\"\",\"pid\":{pid},\"tid\":{tid},\"name\":{name},\"ph\":\"X\",\"ts\":{ts},\"dur\":{dur}}}",
            pid = pid,
            tid = json_string(&thread_id.to_string()),
            name = json_string(node.key()),
            ts = ts_us,
            dur = dur_us,
        )?;

        // Recurse into children.
        for child in node.children() {
            self.write_node_chrome(writer, child, pid, thread_id, first)?;
        }
        Ok(())
    }

    /// Merges another tree into this one.
    ///
    /// Matches C++ `TraceEventTree::Merge()`: merges counters, counter
    /// time-series, markers, and thread roots.
    pub fn merge(&mut self, other: EventTree) {
        // Merge counters
        for (name, value) in other.counters {
            self.counters
                .entry(name)
                .and_modify(|v| {
                    if v.is_delta {
                        v.value += value.value;
                    } else {
                        v.value = value.value;
                    }
                })
                .or_insert(value);
        }

        // Merge counter time-series (sorted merge by timestamp).
        for (name, mut values) in other.counter_values {
            let entry = self.counter_values.entry(name).or_default();
            let orig_len = entry.len();
            entry.append(&mut values);
            // In-place merge to keep sorted by timestamp.
            if orig_len > 0 && entry.len() > orig_len {
                entry[..]
                    .sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            }
        }

        // Merge marker values (sorted merge by timestamp).
        for (name, mut values) in other.markers {
            let entry = self.markers.entry(name).or_default();
            let orig_len = entry.len();
            entry.append(&mut values);
            if orig_len > 0 && entry.len() > orig_len {
                entry[..]
                    .sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            }
        }

        // Merge thread roots
        for (thread_id, node) in other.thread_roots {
            self.thread_roots.insert(thread_id, node);
        }

        // Rebuild root children
        self.rebuild_root();
    }

    fn rebuild_root(&mut self) {
        let mut new_root = EventNode::new("root");
        for node in self.thread_roots.values() {
            new_root.add_child(Arc::clone(node));
        }
        self.root = Arc::new(new_root);
    }
}

/// Produces a JSON-escaped string with surrounding quotes.
fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < '\x20' => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Builder for constructing event trees from collections.
///
/// Tracks markers separately (matching C++ `_markersMap`), and records
/// counter time-series into the tree.
#[derive(Debug)]
pub struct EventTreeBuilder {
    /// Stack of nodes being built for current thread.
    stack: Vec<EventNode>,
    /// Start times for matching begin/end.
    start_times: Vec<TimeStamp>,
    /// Accumulated marker events: key -> [(timestamp, thread_id)].
    markers: MarkerValuesMap,
}

impl EventTreeBuilder {
    /// Creates a new builder.
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            start_times: Vec::new(),
            markers: MarkerValuesMap::new(),
        }
    }

    /// Builds an event tree from a collection.
    pub fn build(&mut self, collection: &Collection) -> EventTree {
        let mut tree = EventTree::new();
        self.markers.clear();

        for (thread_id, event_list) in collection.iter() {
            self.stack.clear();
            self.start_times.clear();

            // Push root for this thread
            self.stack
                .push(EventNode::new(format!("Thread {}", thread_id)));

            for event in event_list.iter() {
                match &event.event_type {
                    EventType::Begin => {
                        self.start_times.push(event.timestamp_seconds());
                        self.stack.push(EventNode::new(event.key()));
                    }
                    EventType::End => {
                        if let (Some(start), Some(mut node)) =
                            (self.start_times.pop(), self.stack.pop())
                        {
                            let duration = event.timestamp_seconds() - start;
                            node.set_inclusive_time(duration);
                            node.set_exclusive_time(duration - node.children_time());
                            node.increment_count();

                            if let Some(parent) = self.stack.last_mut() {
                                parent.add_child(Arc::new(node));
                            }
                        }
                    }
                    EventType::Timespan(duration_ns) => {
                        let mut node = EventNode::new(event.key());
                        let duration = *duration_ns as f64 / 1_000_000_000.0;
                        node.set_inclusive_time(duration);
                        node.set_exclusive_time(duration);
                        node.increment_count();

                        if let Some(parent) = self.stack.last_mut() {
                            parent.add_child(Arc::new(node));
                        }
                    }
                    EventType::CounterDelta(delta) => {
                        // Update summary counter.
                        let counter = tree.counters.entry(event.key().to_string()).or_default();
                        counter.value += delta;
                        counter.is_delta = true;
                        // Record time-series data point.
                        let ts_sec = event.timestamp_seconds();
                        tree.counter_values
                            .entry(event.key().to_string())
                            .or_default()
                            .push((ts_sec, counter.value));
                    }
                    EventType::CounterValue(value) => {
                        let counter = tree.counters.entry(event.key().to_string()).or_default();
                        counter.value = *value;
                        counter.is_delta = false;
                        // Record time-series data point.
                        let ts_sec = event.timestamp_seconds();
                        tree.counter_values
                            .entry(event.key().to_string())
                            .or_default()
                            .push((ts_sec, *value));
                    }
                    EventType::Marker => {
                        // Store marker in the markers map (matches C++ _OnMarker).
                        let ts_sec = event.timestamp_seconds();
                        self.markers
                            .entry(event.key().to_string())
                            .or_default()
                            .push((ts_sec, thread_id.clone()));

                        // Also set as attribute on current node for tree traversal.
                        if let Some(current) = self.stack.last_mut() {
                            current.set_attribute(event.key(), "Marker");
                        }
                    }
                    EventType::Data(_) | EventType::ScopeData(_) => {
                        // Data events are attributes on current node.
                        if let Some(current) = self.stack.last_mut() {
                            current.set_attribute(event.key(), format!("{:?}", event.event_type));
                        }
                    }
                }
            }

            // Pop remaining stack (unclosed scopes)
            while self.stack.len() > 1 {
                if let Some(node) = self.stack.pop() {
                    if let Some(parent) = self.stack.last_mut() {
                        parent.add_child(Arc::new(node));
                    }
                }
            }

            // Get thread root
            if let Some(thread_root) = self.stack.pop() {
                tree.thread_roots
                    .insert(thread_id.clone(), Arc::new(thread_root));
            }
        }

        // Sort marker timestamps (matches C++ OnEndCollection).
        for values in self.markers.values_mut() {
            values.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        }

        // Move markers into the tree.
        tree.markers = std::mem::take(&mut self.markers);

        tree.rebuild_root();
        tree
    }
}

impl Default for EventTreeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Event, EventList, ThreadId};

    #[test]
    fn test_event_tree_from_collection() {
        let mut collection = Collection::new();

        let mut events = EventList::new();
        events.push(Event::begin("outer", 0));
        events.push(Event::begin("inner", 100_000_000)); // 0.1s
        events.push(Event::end("inner", 200_000_000)); // 0.2s
        events.push(Event::end("outer", 500_000_000)); // 0.5s

        collection.add_to_collection(ThreadId::new("thread1"), events);

        let tree = EventTree::from_collection(&collection);

        assert!(!tree.is_empty());
        assert_eq!(tree.thread_ids().count(), 1);
    }

    #[test]
    fn test_event_tree_counters() {
        let mut collection = Collection::new();

        let mut events = EventList::new();
        events.push(Event::new("items", EventType::CounterDelta(5.0), 100));
        events.push(Event::new("items", EventType::CounterDelta(3.0), 200));

        collection.add_to_collection(ThreadId::new("thread1"), events);

        let tree = EventTree::from_collection(&collection);

        assert!(tree.counters().contains_key("items"));
        assert_eq!(tree.counters()["items"].value, 8.0);

        // Counter time-series should have 2 data points.
        let cv = tree.counter_values().get("items").unwrap();
        assert_eq!(cv.len(), 2);
        // First point: 5.0, second point: 5.0 + 3.0 = 8.0
        assert_eq!(cv[0].1, 5.0);
        assert_eq!(cv[1].1, 8.0);
    }

    #[test]
    fn test_event_tree_markers() {
        let mut collection = Collection::new();

        let mut events = EventList::new();
        events.push(Event::new("mark1", EventType::Marker, 100_000_000));
        events.push(Event::new("mark1", EventType::Marker, 300_000_000));
        events.push(Event::new("mark2", EventType::Marker, 200_000_000));

        collection.add_to_collection(ThreadId::new("thread1"), events);

        let tree = EventTree::from_collection(&collection);

        // Should track markers by name.
        let markers = tree.markers();
        assert!(markers.contains_key("mark1"));
        assert!(markers.contains_key("mark2"));
        assert_eq!(markers["mark1"].len(), 2);
        assert_eq!(markers["mark2"].len(), 1);

        // Markers should be sorted by timestamp.
        assert!(markers["mark1"][0].0 <= markers["mark1"][1].0);
    }

    #[test]
    fn test_event_tree_final_counter_values() {
        let mut collection = Collection::new();

        let mut events = EventList::new();
        events.push(Event::new("items", EventType::CounterDelta(5.0), 100));
        events.push(Event::new("items", EventType::CounterDelta(3.0), 200));
        events.push(Event::new("score", EventType::CounterValue(42.0), 300));

        collection.add_to_collection(ThreadId::new("thread1"), events);

        let tree = EventTree::from_collection(&collection);

        let final_vals = tree.get_final_counter_values();
        assert_eq!(final_vals["items"], 8.0);
        assert_eq!(final_vals["score"], 42.0);
    }

    #[test]
    fn test_write_chrome_trace_object() {
        let mut collection = Collection::new();

        let mut events = EventList::new();
        events.push(Event::begin("outer", 0));
        events.push(Event::begin("inner", 100_000_000));
        events.push(Event::end("inner", 200_000_000));
        events.push(Event::end("outer", 500_000_000));
        events.push(Event::new("marker_test", EventType::Marker, 600_000_000));

        collection.add_to_collection(ThreadId::new("thread1"), events);

        let tree = EventTree::from_collection(&collection);

        let mut buf = Vec::new();
        tree.write_chrome_trace_object(&mut buf).unwrap();
        let json = String::from_utf8(buf).unwrap();

        // Basic structural checks.
        assert!(json.starts_with("{\"traceEvents\":["));
        assert!(json.ends_with("]}"));
        assert!(json.contains("\"ph\":\"X\"")); // Complete events
        assert!(json.contains("\"ph\":\"I\"")); // Marker instant event
        assert!(json.contains("marker_test"));
    }
}
