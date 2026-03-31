//! Trace serialization utilities.
//!
//! Port of pxr/base/trace/serialization.h and jsonSerialization.h
//!
//! Provides JSON (Chrome trace format) serialization and deserialization
//! for trace collections.

use super::aggregate_node::AggregateNode;
use super::aggregate_tree::AggregateTree;
use super::collection::Collection;
use super::event::{Event, EventType};
use super::event_list::EventList;
use super::threads::ThreadId;
use std::collections::HashMap;
use std::fmt::Write;
use std::sync::Arc;

/// Chrome trace format output.
///
/// Writes trace data in the Chrome Tracing Format (JSON) that can be
/// viewed in chrome://tracing or similar tools.
pub struct ChromeTraceWriter {
    output: String,
    first_event: bool,
}

impl ChromeTraceWriter {
    /// Creates a new Chrome trace writer.
    pub fn new() -> Self {
        Self {
            output: String::new(),
            first_event: true,
        }
    }

    /// Begins writing the trace.
    pub fn begin(&mut self) {
        self.output.push_str("{\"traceEvents\":[");
        self.first_event = true;
    }

    /// Ends writing the trace.
    pub fn end(&mut self) {
        self.output.push_str("]}");
    }

    /// Writes an event.
    pub fn write_event(
        &mut self,
        name: &str,
        category: &str,
        phase: char,
        timestamp_us: u64,
        pid: u64,
        tid: u64,
    ) {
        if !self.first_event {
            self.output.push(',');
        }
        self.first_event = false;

        write!(
            self.output,
            "{{\"name\":\"{}\",\"cat\":\"{}\",\"ph\":\"{}\",\"ts\":{},\"pid\":{},\"tid\":{}}}",
            escape_json(name),
            escape_json(category),
            phase,
            timestamp_us,
            pid,
            tid
        )
        .expect("fmt write");
    }

    /// Writes a duration event (complete event with duration).
    pub fn write_duration(
        &mut self,
        name: &str,
        category: &str,
        timestamp_us: u64,
        duration_us: u64,
        pid: u64,
        tid: u64,
    ) {
        if !self.first_event {
            self.output.push(',');
        }
        self.first_event = false;

        write!(
            self.output,
            "{{\"name\":\"{}\",\"cat\":\"{}\",\"ph\":\"X\",\"ts\":{},\"dur\":{},\"pid\":{},\"tid\":{}}}",
            escape_json(name),
            escape_json(category),
            timestamp_us,
            duration_us,
            pid,
            tid
        )
        .expect("fmt write");
    }

    /// Writes a counter event.
    pub fn write_counter(&mut self, name: &str, value: f64, timestamp_us: u64, pid: u64) {
        if !self.first_event {
            self.output.push(',');
        }
        self.first_event = false;

        write!(
            self.output,
            "{{\"name\":\"{}\",\"ph\":\"C\",\"ts\":{},\"pid\":{},\"args\":{{\"value\":{}}}}}",
            escape_json(name),
            timestamp_us,
            pid,
            value
        )
        .expect("fmt write");
    }

    /// Returns the output string.
    pub fn finish(self) -> String {
        self.output
    }
}

impl Default for ChromeTraceWriter {
    fn default() -> Self {
        Self::new()
    }
}

/// Escapes a string for JSON output.
fn escape_json(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            c if c.is_control() => {
                write!(result, "\\u{:04x}", c as u32).expect("fmt write");
            }
            c => result.push(c),
        }
    }
    result
}

/// Writes multiple collections to Chrome trace format.
pub fn write_collections_to_json(collections: &[Arc<Collection>]) -> String {
    let mut writer = ChromeTraceWriter::new();
    writer.begin();

    let mut thread_id_map: HashMap<String, u64> = HashMap::new();
    let mut next_tid: u64 = 1;

    for collection in collections {
        for (thread_id, event_list) in collection.iter() {
            let tid = *thread_id_map
                .entry(thread_id.as_str().to_string())
                .or_insert_with(|| {
                    let id = next_tid;
                    next_tid += 1;
                    id
                });

            for event in event_list.iter() {
                let timestamp_us = event.timestamp() / 1000;
                match &event.event_type {
                    EventType::Begin => {
                        writer.write_event(event.key(), "trace", 'B', timestamp_us, 1, tid);
                    }
                    EventType::End => {
                        writer.write_event(event.key(), "trace", 'E', timestamp_us, 1, tid);
                    }
                    EventType::Timespan(duration_ns) => {
                        let duration_us = duration_ns / 1000;
                        writer.write_duration(
                            event.key(),
                            "trace",
                            timestamp_us,
                            duration_us,
                            1,
                            tid,
                        );
                    }
                    EventType::Marker => {
                        writer.write_event(event.key(), "trace", 'i', timestamp_us, 1, tid);
                    }
                    EventType::CounterDelta(v) | EventType::CounterValue(v) => {
                        writer.write_counter(event.key(), *v, timestamp_us, 1);
                    }
                    EventType::Data(_) | EventType::ScopeData(_) => {}
                }
            }
        }
    }

    writer.end();
    writer.finish()
}

/// Writes a collection to Chrome trace format.
pub fn write_chrome_trace(collection: &Collection) -> String {
    let mut writer = ChromeTraceWriter::new();
    writer.begin();

    // Map thread names to numeric IDs for Chrome trace format
    let mut thread_id_map: HashMap<String, u64> = HashMap::new();
    let mut next_tid: u64 = 1;

    for (thread_id, event_list) in collection.iter() {
        let tid = *thread_id_map
            .entry(thread_id.as_str().to_string())
            .or_insert_with(|| {
                let id = next_tid;
                next_tid += 1;
                id
            });

        let mut stack: Vec<(&str, u64)> = Vec::new();

        for event in event_list.iter() {
            let timestamp_us = event.timestamp() / 1000; // ns to us

            match &event.event_type {
                EventType::Begin => {
                    writer.write_event(event.key(), "trace", 'B', timestamp_us, 1, tid);
                    stack.push((event.key(), timestamp_us));
                }
                EventType::End => {
                    writer.write_event(event.key(), "trace", 'E', timestamp_us, 1, tid);
                    stack.pop();
                }
                EventType::Timespan(duration_ns) => {
                    let duration_us = duration_ns / 1000;
                    writer.write_duration(event.key(), "trace", timestamp_us, duration_us, 1, tid);
                }
                EventType::Marker => {
                    writer.write_event(event.key(), "trace", 'i', timestamp_us, 1, tid);
                }
                EventType::CounterDelta(v) => {
                    writer.write_counter(event.key(), *v, timestamp_us, 1);
                }
                EventType::CounterValue(v) => {
                    writer.write_counter(event.key(), *v, timestamp_us, 1);
                }
                EventType::Data(_) | EventType::ScopeData(_) => {}
            }
        }
    }

    writer.end();
    writer.finish()
}

/// Converts microseconds to nanosecond ticks.
fn microseconds_to_ticks(us: f64) -> u64 {
    (us * 1000.0) as u64
}

/// Parses a Chrome trace JSON and creates a Collection.
///
/// Supports both Chrome trace format and extended libTrace format.
pub fn collection_from_json(json: &str) -> Option<Collection> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;

    let trace_events = if let Some(obj) = value.as_object() {
        obj.get("traceEvents")?.as_array()?
    } else if let Some(arr) = value.as_array() {
        arr
    } else {
        return None;
    };

    // Group events by thread ID
    let mut events_by_thread: HashMap<String, Vec<Event>> = HashMap::new();

    for event in trace_events {
        let obj = event.as_object()?;

        // Get thread ID (can be string or integer)
        let tid = if let Some(tid_str) = obj.get("tid").and_then(|v| v.as_str()) {
            tid_str.to_string()
        } else if let Some(tid_num) = obj.get("tid").and_then(|v| v.as_u64()) {
            tid_num.to_string()
        } else {
            continue;
        };

        // Get timestamp (can be float or integer)
        let ts = if let Some(ts_f) = obj.get("ts").and_then(|v| v.as_f64()) {
            microseconds_to_ticks(ts_f)
        } else if let Some(ts_i) = obj.get("ts").and_then(|v| v.as_u64()) {
            ts_i * 1000 // us to ns
        } else {
            continue;
        };

        let name = obj.get("name").and_then(|v| v.as_str())?;
        let ph = obj.get("ph").and_then(|v| v.as_str())?;

        let event = match ph {
            "B" => Event::new(name, EventType::Begin, ts),
            "E" => Event::new(name, EventType::End, ts),
            "i" | "I" | "R" => Event::new(name, EventType::Marker, ts),
            "X" => {
                // Complete event with duration
                let dur = if let Some(dur_f) = obj.get("dur").and_then(|v| v.as_f64()) {
                    microseconds_to_ticks(dur_f)
                } else if let Some(dur_i) = obj.get("dur").and_then(|v| v.as_u64()) {
                    dur_i * 1000
                } else if let Some(tdur_f) = obj.get("tdur").and_then(|v| v.as_f64()) {
                    microseconds_to_ticks(tdur_f)
                } else if let Some(tdur_i) = obj.get("tdur").and_then(|v| v.as_u64()) {
                    tdur_i * 1000
                } else {
                    continue;
                };
                Event::new(name, EventType::Timespan(dur), ts)
            }
            "C" => {
                // Counter event
                let args = obj.get("args").and_then(|v| v.as_object())?;
                let value = args.get("value").and_then(|v| v.as_f64())?;
                Event::new(name, EventType::CounterValue(value), ts)
            }
            _ => continue,
        };

        events_by_thread.entry(tid).or_default().push(event);
    }

    // Sort events by timestamp and build collection
    let mut collection = Collection::new();

    for (tid, mut events) in events_by_thread {
        events.sort_by_key(|e| e.timestamp());

        let mut event_list = EventList::new();
        for event in events {
            event_list.push(event);
        }

        collection.add_to_collection(ThreadId::new(&tid), event_list);
    }

    Some(collection)
}

/// Text report format for aggregate data.
pub struct TextReportWriter {
    output: String,
    indent_level: usize,
}

impl TextReportWriter {
    /// Creates a new text report writer.
    pub fn new() -> Self {
        Self {
            output: String::new(),
            indent_level: 0,
        }
    }

    /// Writes a header line.
    pub fn write_header(&mut self, title: &str) {
        writeln!(self.output, "=== {} ===", title).expect("fmt write");
        writeln!(self.output).expect("fmt write");
    }

    /// Writes a separator line.
    pub fn write_separator(&mut self) {
        writeln!(self.output, "{}", "-".repeat(80)).expect("fmt write");
    }

    /// Writes an aggregate node.
    pub fn write_node(&mut self, node: &AggregateNode) {
        let indent = "  ".repeat(self.indent_level);
        writeln!(
            self.output,
            "{}{}: {:.3}ms ({:.3}ms exclusive) x{}",
            indent,
            node.key(),
            node.inclusive_time() * 1000.0,
            node.exclusive_time() * 1000.0,
            node.count()
        )
        .expect("fmt write");

        self.indent_level += 1;
        for child in node.children() {
            self.write_node(child);
        }
        self.indent_level -= 1;
    }

    /// Writes an aggregate tree.
    pub fn write_tree(&mut self, tree: &AggregateTree) {
        self.write_header("Trace Report");

        writeln!(
            self.output,
            "Total time: {:.3}ms",
            tree.total_time() * 1000.0
        )
        .expect("fmt write");
        writeln!(self.output).expect("fmt write");

        self.write_separator();
        writeln!(self.output, "Call Tree:").expect("fmt write");
        self.write_separator();

        for child in tree.root().children() {
            self.write_node(child);
        }

        if !tree.counters().is_empty() {
            writeln!(self.output).expect("fmt write");
            self.write_separator();
            writeln!(self.output, "Counters:").expect("fmt write");
            self.write_separator();

            for (name, value) in tree.counters() {
                writeln!(self.output, "  {}: {:.3}", name, value.value).expect("fmt write");
            }
        }
    }

    /// Returns the output.
    pub fn finish(self) -> String {
        self.output
    }
}

impl Default for TextReportWriter {
    fn default() -> Self {
        Self::new()
    }
}

/// Writes an aggregate tree to text format.
pub fn write_text_report(tree: &AggregateTree) -> String {
    let mut writer = TextReportWriter::new();
    writer.write_tree(tree);
    writer.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_json() {
        assert_eq!(escape_json("hello"), "hello");
        assert_eq!(escape_json("hello\"world"), "hello\\\"world");
        assert_eq!(escape_json("line1\nline2"), "line1\\nline2");
    }

    #[test]
    fn test_chrome_trace_writer() {
        let mut writer = ChromeTraceWriter::new();
        writer.begin();
        writer.write_event("test", "cat", 'B', 1000, 1, 1);
        writer.write_event("test", "cat", 'E', 2000, 1, 1);
        writer.end();

        let output = writer.finish();
        assert!(output.contains("traceEvents"));
        assert!(output.contains("\"name\":\"test\""));
    }

    #[test]
    fn test_text_report_writer() {
        let mut writer = TextReportWriter::new();
        writer.write_header("Test");
        let output = writer.finish();
        assert!(output.contains("=== Test ==="));
    }
}
