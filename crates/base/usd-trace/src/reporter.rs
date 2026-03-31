//! Trace reporter for outputting collected trace data.
//!
//! The reporter provides utilities for formatting and outputting trace data
//! in various formats including text, JSON, and Chrome trace format.
//! Supports overhead adjustment, recursive call folding, and per-thread grouping.

use std::collections::HashMap;
use std::io::Write;
use std::sync::Arc;

use super::aggregate_node::AggregateNode;
use super::aggregate_tree::AggregateTree;
use super::collection::Collection;
use super::collector::Collector;
use super::event::{Event, EventType};
use super::event_node::EventNode;
use super::event_tree::EventTree;
use usd_tf::Token;

/// Output format for trace reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportFormat {
    /// Human-readable text format.
    Text,
    /// JSON format.
    Json,
    /// Chrome tracing format (for chrome://tracing).
    Chrome,
}

/// Configuration for trace reports.
#[derive(Debug, Clone)]
pub struct ReportConfig {
    /// The output format.
    pub format: ReportFormat,
    /// Whether to include thread information.
    pub include_threads: bool,
    /// Whether to include counter values.
    pub include_counters: bool,
    /// Minimum duration (in nanoseconds) to include an event.
    pub min_duration_ns: u64,
}

impl Default for ReportConfig {
    fn default() -> Self {
        Self {
            format: ReportFormat::Text,
            include_threads: true,
            include_counters: true,
            min_duration_ns: 0,
        }
    }
}

/// Aggregated timing information for a scope.
#[derive(Debug, Clone, Default)]
pub struct ScopeStats {
    /// Total time spent in this scope (nanoseconds).
    pub total_ns: u64,
    /// Number of times this scope was entered.
    pub count: u64,
    /// Minimum duration (nanoseconds).
    pub min_ns: u64,
    /// Maximum duration (nanoseconds).
    pub max_ns: u64,
}

impl ScopeStats {
    /// Returns the average duration in nanoseconds.
    pub fn avg_ns(&self) -> u64 {
        if self.count > 0 {
            self.total_ns / self.count
        } else {
            0
        }
    }

    /// Returns the average duration in milliseconds.
    pub fn avg_ms(&self) -> f64 {
        self.avg_ns() as f64 / 1_000_000.0
    }

    /// Returns the total duration in milliseconds.
    pub fn total_ms(&self) -> f64 {
        self.total_ns as f64 / 1_000_000.0
    }
}

/// Counter map type (token -> total value).
pub type CounterMap = HashMap<Token, f64>;

/// Parsed tree from a report file.
#[derive(Debug)]
pub struct ParsedTree {
    /// The aggregate tree.
    pub tree: AggregateTree,
    /// Iteration count used when generating the report.
    pub iteration_count: i32,
}

/// The trace reporter.
///
/// Converts streams of TraceEvent objects into call trees which can then
/// be used as a data source to a GUI or written out to a file.
/// Supports overhead adjustment and recursive call folding.
pub struct Reporter {
    /// Label for this reporter.
    label: String,
    /// Report configuration.
    config: ReportConfig,
    /// Group events by function (affects stack trace reporting).
    group_by_function: bool,
    /// Fold recursive calls in output.
    fold_recursive_calls: bool,
    /// Adjust scope times for overhead and noise.
    should_adjust_for_overhead_and_noise: bool,
    /// Cached aggregate tree.
    aggregate_tree: Option<AggregateTree>,
    /// Cached event tree.
    event_tree: Option<EventTree>,
    /// Counter values (key -> total value).
    counters: CounterMap,
    /// Counter indices (key -> index).
    counter_indices: HashMap<Token, i32>,
    /// Next counter index.
    next_counter_index: i32,
}

impl Reporter {
    /// Creates a new reporter with the given label.
    pub fn new(label: &str) -> Self {
        Self {
            label: label.to_string(),
            config: ReportConfig::default(),
            group_by_function: true,
            fold_recursive_calls: false,
            should_adjust_for_overhead_and_noise: true,
            aggregate_tree: None,
            event_tree: None,
            counters: HashMap::new(),
            counter_indices: HashMap::new(),
            next_counter_index: 0,
        }
    }

    /// Creates a new reporter with the given label and configuration.
    pub fn with_config(label: &str, config: ReportConfig) -> Self {
        Self {
            label: label.to_string(),
            config,
            group_by_function: true,
            fold_recursive_calls: false,
            should_adjust_for_overhead_and_noise: true,
            aggregate_tree: None,
            event_tree: None,
            counters: HashMap::new(),
            counter_indices: HashMap::new(),
            next_counter_index: 0,
        }
    }

    /// Returns the label associated with this reporter.
    pub fn get_label(&self) -> &str {
        &self.label
    }

    /// Sets the output format.
    pub fn set_format(&mut self, format: ReportFormat) {
        self.config.format = format;
    }

    /// Returns the current configuration.
    pub fn config(&self) -> &ReportConfig {
        &self.config
    }

    /// Generates a human-readable text report with call tree, thread grouping,
    /// overhead adjustment, and recursive folding support.
    pub fn generate_text_report(&self) -> String {
        let collector = Collector::get_instance();
        let events = collector.get_events();
        let counters = collector.get_counters();

        let mut output = String::new();
        output.push_str("=== Trace Report ===\n\n");

        // Calculate scope statistics (with overhead adjustment and folding)
        let stats = self.calculate_stats(&events);

        // Group events by thread for tree output
        if self.config.include_threads {
            let tree = self.build_call_tree(&events);
            if !tree.is_empty() {
                output.push_str("Call Tree:\n");
                output.push_str("---------\n");
                for (thread_name, entries) in &tree {
                    output.push_str(&format!("Thread: {}\n", thread_name));
                    for entry in entries {
                        output.push_str(&format!(
                            "{}{}: {:.3}ms\n",
                            "  ".repeat(entry.depth),
                            entry.key,
                            entry.duration_ns as f64 / 1_000_000.0,
                        ));
                    }
                    output.push('\n');
                }
            }
        }

        // Output scope statistics
        if !stats.is_empty() {
            output.push_str("Scope Statistics:\n");
            output.push_str("-----------------\n");

            let mut sorted_stats: Vec<_> = stats.iter().collect();
            sorted_stats.sort_by(|a, b| b.1.total_ns.cmp(&a.1.total_ns));

            for (name, stat) in sorted_stats {
                output.push_str(&format!(
                    "{}: count={}, total={:.3}ms, avg={:.3}ms, min={:.3}ms, max={:.3}ms\n",
                    name,
                    stat.count,
                    stat.total_ms(),
                    stat.avg_ms(),
                    stat.min_ns as f64 / 1_000_000.0,
                    stat.max_ns as f64 / 1_000_000.0,
                ));
            }
            output.push('\n');
        }

        // Output counters
        if self.config.include_counters && !counters.is_empty() {
            output.push_str("Counters:\n");
            output.push_str("---------\n");

            let mut sorted_counters: Vec<_> = counters.iter().collect();
            sorted_counters.sort_by(|a, b| a.0.cmp(b.0));

            for (name, value) in sorted_counters {
                output.push_str(&format!("{}: {}\n", name, value));
            }
            output.push('\n');
        }

        // Output event count
        output.push_str(&format!("Total events: {}\n", events.len()));

        output
    }

    /// Generates a JSON report from the collected events.
    pub fn generate_json_report(&self) -> String {
        let collector = Collector::get_instance();
        let events = collector.get_events();
        let counters = collector.get_counters();
        let stats = self.calculate_stats(&events);

        let mut output = String::new();
        output.push_str("{\n");

        // Scope statistics
        output.push_str("  \"scopes\": {\n");
        let stats_vec: Vec<_> = stats.iter().collect();
        for (i, (name, stat)) in stats_vec.iter().enumerate() {
            output.push_str(&format!(
                "    \"{}\": {{\"count\": {}, \"total_ms\": {:.6}, \"avg_ms\": {:.6}, \"min_ms\": {:.6}, \"max_ms\": {:.6}}}",
                name,
                stat.count,
                stat.total_ms(),
                stat.avg_ms(),
                stat.min_ns as f64 / 1_000_000.0,
                stat.max_ns as f64 / 1_000_000.0,
            ));
            if i < stats_vec.len() - 1 {
                output.push(',');
            }
            output.push('\n');
        }
        output.push_str("  },\n");

        // Counters
        output.push_str("  \"counters\": {\n");
        let counters_vec: Vec<_> = counters.iter().collect();
        for (i, (name, value)) in counters_vec.iter().enumerate() {
            output.push_str(&format!("    \"{}\": {}", name, value));
            if i < counters_vec.len() - 1 {
                output.push(',');
            }
            output.push('\n');
        }
        output.push_str("  },\n");

        output.push_str(&format!("  \"total_events\": {}\n", events.len()));
        output.push_str("}\n");

        output
    }

    /// Generates a Chrome trace format JSON report for chrome://tracing.
    pub fn generate_chrome_report(&self) -> String {
        let collector = Collector::get_instance();
        let events = collector.get_events();

        let mut output = String::new();
        output.push_str("{\"traceEvents\": [\n");

        let mut first = true;
        for event in &events {
            if !first {
                output.push_str(",\n");
            }
            first = false;

            let phase = match &event.event_type {
                EventType::Begin => "B",
                EventType::End => "E",
                EventType::Marker => "i",
                EventType::CounterDelta(_) | EventType::CounterValue(_) => "C",
                EventType::Timespan(_) => "X",
                EventType::Data(_) | EventType::ScopeData(_) => "i",
            };

            let tid = format!("{:?}", event.thread_id);
            let ts = event.timestamp / 1000; // Convert to microseconds

            match &event.event_type {
                EventType::CounterDelta(v) | EventType::CounterValue(v) => {
                    output.push_str(&format!(
                        "  {{\"name\": \"{}\", \"ph\": \"{}\", \"ts\": {}, \"pid\": 1, \"tid\": \"{}\", \"args\": {{\"value\": {}}}}}",
                        event.key, phase, ts, tid, v
                    ));
                }
                EventType::Timespan(dur) => {
                    output.push_str(&format!(
                        "  {{\"name\": \"{}\", \"ph\": \"{}\", \"ts\": {}, \"dur\": {}, \"pid\": 1, \"tid\": \"{}\"}}",
                        event.key, phase, ts, dur / 1000, tid
                    ));
                }
                _ => {
                    output.push_str(&format!(
                        "  {{\"name\": \"{}\", \"ph\": \"{}\", \"ts\": {}, \"pid\": 1, \"tid\": \"{}\"}}",
                        event.key, phase, ts, tid
                    ));
                }
            }
        }

        output.push_str("\n]}\n");
        output
    }

    /// Writes the report to a writer.
    pub fn write_report<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        let report = match self.config.format {
            ReportFormat::Text => self.generate_text_report(),
            ReportFormat::Json => self.generate_json_report(),
            ReportFormat::Chrome => self.generate_chrome_report(),
        };
        writer.write_all(report.as_bytes())
    }

    /// Writes the report to a file.
    pub fn write_to_file(&self, path: &std::path::Path) -> std::io::Result<()> {
        let mut file = std::fs::File::create(path)?;
        self.write_report(&mut file)
    }

    /// Generates a report, dividing all times by `iteration_count`.
    pub fn report(&self, iteration_count: u32) -> String {
        let collector = Collector::get_instance();
        let events = collector.get_events();
        let stats = self.calculate_stats(&events);

        let mut output = String::new();
        output.push_str(&format!(
            "=== Trace Report (iterations: {}) ===\n\n",
            iteration_count
        ));

        let divisor = iteration_count.max(1) as f64;

        if !stats.is_empty() {
            output.push_str("Scope Statistics (per iteration):\n");
            output.push_str("----------------------------------\n");

            let mut sorted_stats: Vec<_> = stats.iter().collect();
            sorted_stats.sort_by(|a, b| b.1.total_ns.cmp(&a.1.total_ns));

            for (name, stat) in sorted_stats {
                output.push_str(&format!(
                    "{}: count={:.1}, total={:.3}ms, avg={:.3}ms\n",
                    name,
                    stat.count as f64 / divisor,
                    stat.total_ms() / divisor,
                    stat.avg_ms(),
                ));
            }
        }

        output
    }

    /// Generates a timing-only report.
    pub fn report_times(&self) -> String {
        let collector = Collector::get_instance();
        let events = collector.get_events();
        let stats = self.calculate_stats(&events);

        let mut output = String::new();
        output.push_str("=== Trace Times ===\n\n");

        let mut sorted_stats: Vec<_> = stats.iter().collect();
        sorted_stats.sort_by(|a, b| b.1.total_ns.cmp(&a.1.total_ns));

        for (name, stat) in sorted_stats {
            output.push_str(&format!(
                "{}: {:.3}ms ({} calls)\n",
                name,
                stat.total_ms(),
                stat.count,
            ));
        }

        output
    }

    /// Generates a Chrome tracing format report (alias for API compatibility).
    pub fn report_chrome_tracing(&self) -> String {
        self.generate_chrome_report()
    }

    /// Load a report from a JSON file.
    pub fn load_report(path: &std::path::Path) -> std::io::Result<HashMap<String, ScopeStats>> {
        use std::io::Read;
        let mut file = std::fs::File::open(path)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        Self::parse_json_report(&content)
    }

    /// Parse a JSON report string.
    fn parse_json_report(content: &str) -> std::io::Result<HashMap<String, ScopeStats>> {
        let mut stats = HashMap::new();

        if let Some(scopes_start) = content.find("\"scopes\"") {
            if let Some(brace_start) = content[scopes_start..].find('{') {
                let scopes_content = &content[scopes_start + brace_start..];

                let mut pos = 0;
                while let Some(name_start) = scopes_content[pos..].find('"') {
                    let name_start = pos + name_start + 1;
                    if let Some(name_end) = scopes_content[name_start..].find('"') {
                        let name_end = name_start + name_end;
                        let name = scopes_content[name_start..name_end].to_string();

                        if let Some(obj_start) = scopes_content[name_end..].find('{') {
                            let obj_start = name_end + obj_start;
                            if let Some(obj_end) = scopes_content[obj_start..].find('}') {
                                let obj_end = obj_start + obj_end + 1;
                                let obj_content = &scopes_content[obj_start..obj_end];

                                let mut scope_stat = ScopeStats::default();

                                if let Some(count) = Self::extract_json_number(obj_content, "count")
                                {
                                    scope_stat.count = count as u64;
                                }
                                if let Some(total_ms) =
                                    Self::extract_json_float(obj_content, "total_ms")
                                {
                                    scope_stat.total_ns = (total_ms * 1_000_000.0) as u64;
                                }
                                if let Some(min_ms) =
                                    Self::extract_json_float(obj_content, "min_ms")
                                {
                                    scope_stat.min_ns = (min_ms * 1_000_000.0) as u64;
                                }
                                if let Some(max_ms) =
                                    Self::extract_json_float(obj_content, "max_ms")
                                {
                                    scope_stat.max_ns = (max_ms * 1_000_000.0) as u64;
                                }

                                stats.insert(name, scope_stat);
                                pos = obj_end;
                                continue;
                            }
                        }
                    }
                    break;
                }
            }
        }

        Ok(stats)
    }

    fn extract_json_number(content: &str, key: &str) -> Option<i64> {
        let pattern = format!("\"{}\":", key);
        if let Some(start) = content.find(&pattern) {
            let value_start = start + pattern.len();
            let value_str: String = content[value_start..]
                .chars()
                .skip_while(|c| c.is_whitespace())
                .take_while(|c| c.is_ascii_digit() || *c == '-')
                .collect();
            value_str.parse().ok()
        } else {
            None
        }
    }

    fn extract_json_float(content: &str, key: &str) -> Option<f64> {
        let pattern = format!("\"{}\":", key);
        if let Some(start) = content.find(&pattern) {
            let value_start = start + pattern.len();
            let value_str: String = content[value_start..]
                .chars()
                .skip_while(|c| c.is_whitespace())
                .take_while(|c| {
                    c.is_ascii_digit() || *c == '.' || *c == '-' || *c == 'e' || *c == 'E'
                })
                .collect();
            value_str.parse().ok()
        } else {
            None
        }
    }

    // ========== Tree Accessors ==========

    /// Returns the root node of the aggregated call tree.
    pub fn get_aggregate_tree_root(&self) -> Option<&Arc<AggregateNode>> {
        self.aggregate_tree.as_ref().map(|t| t.root())
    }

    /// Returns the root node of the call tree.
    pub fn get_event_root(&self) -> Option<&Arc<EventNode>> {
        self.event_tree.as_ref().map(|t| t.root())
    }

    /// Returns the event call tree.
    pub fn get_event_tree(&self) -> Option<&EventTree> {
        self.event_tree.as_ref()
    }

    /// Returns the aggregate tree.
    pub fn get_aggregate_tree(&self) -> Option<&AggregateTree> {
        self.aggregate_tree.as_ref()
    }

    // ========== Counter Management ==========

    /// Returns a map of counters with their total accumulated values.
    pub fn get_counters(&self) -> &CounterMap {
        &self.counters
    }

    /// Returns the numeric index associated with a counter key (-1 if missing).
    pub fn get_counter_index(&self, key: &Token) -> i32 {
        self.counter_indices.get(key).copied().unwrap_or(-1)
    }

    /// Add a counter to the reporter. Returns false if key or index already exists.
    pub fn add_counter(&mut self, key: &Token, index: i32, total_value: f64) -> bool {
        if self.counter_indices.contains_key(key) {
            return false;
        }
        if self.counter_indices.values().any(|&i| i == index) {
            return false;
        }

        self.counters.insert(key.clone(), total_value);
        self.counter_indices.insert(key.clone(), index);
        if index >= self.next_counter_index {
            self.next_counter_index = index + 1;
        }
        true
    }

    // ========== Tree Management ==========

    /// Fully re-builds the event and aggregate trees from the current collection.
    pub fn update_trace_trees(&mut self) {
        let collector = Collector::get_instance();
        let events = collector.get_events();

        let mut collection = Collection::new();
        for event in events {
            let thread_id = super::threads::ThreadId::current();
            collection.add_to_collection(
                thread_id,
                super::event_list::EventList::from_iter(std::iter::once(event)),
            );
        }

        let event_tree = EventTree::from_collection(&collection);
        let aggregate_tree = AggregateTree::from_event_tree(&event_tree);

        for (key, counter_value) in event_tree.counters() {
            self.counters.insert(Token::new(key), counter_value.value);
            if !self.counter_indices.contains_key(&Token::new(key)) {
                self.counter_indices
                    .insert(Token::new(key), self.next_counter_index);
                self.next_counter_index += 1;
            }
        }

        self.event_tree = Some(event_tree);
        self.aggregate_tree = Some(aggregate_tree);
    }

    /// Clears event tree and counters.
    pub fn clear_tree(&mut self) {
        self.aggregate_tree = None;
        self.event_tree = None;
        self.counters.clear();
        self.counter_indices.clear();
        self.next_counter_index = 0;
    }

    // ========== Report Options ==========

    /// Sets whether events in a function are grouped together.
    pub fn set_group_by_function(&mut self, value: bool) {
        self.group_by_function = value;
    }

    /// Returns the current group-by-function state.
    pub fn get_group_by_function(&self) -> bool {
        self.group_by_function
    }

    /// Sets whether recursive calls are folded in output.
    pub fn set_fold_recursive_calls(&mut self, value: bool) {
        self.fold_recursive_calls = value;
    }

    /// Returns the current setting for recursion folding.
    pub fn get_fold_recursive_calls(&self) -> bool {
        self.fold_recursive_calls
    }

    /// Set whether to adjust scope times for overhead and noise.
    pub fn set_should_adjust_for_overhead_and_noise(&mut self, adjust: bool) {
        self.should_adjust_for_overhead_and_noise = adjust;
    }

    /// Returns the current setting for overhead adjustment.
    pub fn should_adjust_for_overhead_and_noise(&self) -> bool {
        self.should_adjust_for_overhead_and_noise
    }

    // ========== Internal Methods ==========

    /// Calculates statistics from events with overhead adjustment and recursive folding.
    fn calculate_stats(&self, events: &[Event]) -> HashMap<String, ScopeStats> {
        let mut stats: HashMap<String, ScopeStats> = HashMap::new();

        // Stack-based tracking per thread: Vec<(key, begin_timestamp)>
        let mut begin_stacks: HashMap<std::thread::ThreadId, Vec<(String, u64)>> = HashMap::new();

        let overhead_ns = if self.should_adjust_for_overhead_and_noise {
            Collector::get_instance().get_scope_overhead()
        } else {
            0
        };

        for event in events {
            match &event.event_type {
                EventType::Begin => {
                    begin_stacks
                        .entry(event.thread_id)
                        .or_default()
                        .push((event.key.clone(), event.timestamp));
                }
                EventType::End => {
                    let stack = begin_stacks.entry(event.thread_id).or_default();

                    // Find and pop the matching begin from the stack (LIFO)
                    let begin_ts =
                        if let Some(pos) = stack.iter().rposition(|(k, _)| k == &event.key) {
                            let (_, ts) = stack.remove(pos);
                            Some(ts)
                        } else {
                            None
                        };

                    if let Some(start) = begin_ts {
                        // Recursive folding: if this key is still on the stack,
                        // this is a nested recursive exit - skip counting
                        if self.fold_recursive_calls && stack.iter().any(|(k, _)| k == &event.key) {
                            continue;
                        }

                        let raw_duration = event.timestamp.saturating_sub(start);
                        let duration = raw_duration.saturating_sub(overhead_ns);

                        if duration >= self.config.min_duration_ns {
                            let stat = stats.entry(event.key.clone()).or_default();
                            stat.total_ns += duration;
                            stat.count += 1;
                            if stat.min_ns == 0 || duration < stat.min_ns {
                                stat.min_ns = duration;
                            }
                            if duration > stat.max_ns {
                                stat.max_ns = duration;
                            }
                        }
                    }
                }
                EventType::Timespan(duration) => {
                    let adjusted = duration.saturating_sub(overhead_ns);
                    if adjusted >= self.config.min_duration_ns {
                        let stat = stats.entry(event.key.clone()).or_default();
                        stat.total_ns += adjusted;
                        stat.count += 1;
                        if stat.min_ns == 0 || adjusted < stat.min_ns {
                            stat.min_ns = adjusted;
                        }
                        if adjusted > stat.max_ns {
                            stat.max_ns = adjusted;
                        }
                    }
                }
                _ => {}
            }
        }

        stats
    }

    /// Builds a call tree grouped by thread from raw events.
    fn build_call_tree(&self, events: &[Event]) -> Vec<(String, Vec<CallTreeEntry>)> {
        // Group events by thread
        let mut by_thread: HashMap<std::thread::ThreadId, Vec<&Event>> = HashMap::new();
        for event in events {
            by_thread.entry(event.thread_id).or_default().push(event);
        }

        let overhead_ns = if self.should_adjust_for_overhead_and_noise {
            Collector::get_instance().get_scope_overhead()
        } else {
            0
        };

        let mut result = Vec::new();
        for (tid, thread_events) in &by_thread {
            let mut entries = Vec::new();
            let mut stack: Vec<(&str, u64)> = Vec::new(); // (key, begin_timestamp)

            for event in thread_events {
                match &event.event_type {
                    EventType::Begin => {
                        stack.push((&event.key, event.timestamp));
                    }
                    EventType::End => {
                        if let Some((key, start)) = stack.pop() {
                            if key == event.key.as_str() {
                                let raw = event.timestamp.saturating_sub(start);
                                let dur = raw.saturating_sub(overhead_ns);
                                entries.push(CallTreeEntry {
                                    key: key.to_string(),
                                    depth: stack.len(),
                                    duration_ns: dur,
                                });
                            }
                        }
                    }
                    _ => {}
                }
            }

            let thread_name = format!("{:?}", tid);
            result.push((thread_name, entries));
        }

        result
    }
}

/// Entry in the call tree output.
struct CallTreeEntry {
    key: String,
    depth: usize,
    duration_ns: u64,
}

impl Default for Reporter {
    fn default() -> Self {
        Self::new("default")
    }
}

#[cfg(test)]
mod tests {
    use super::super::collector::TRACE_TEST_LOCK;
    use super::*;

    #[test]
    fn test_reporter_creation() {
        let reporter = Reporter::new("test");
        assert_eq!(reporter.config().format, ReportFormat::Text);
        assert_eq!(reporter.get_label(), "test");
    }

    #[test]
    fn test_reporter_config() {
        let config = ReportConfig {
            format: ReportFormat::Json,
            include_threads: false,
            include_counters: true,
            min_duration_ns: 1000,
        };
        let reporter = Reporter::with_config("test", config);
        assert_eq!(reporter.config().format, ReportFormat::Json);
        assert!(!reporter.config().include_threads);
        assert_eq!(reporter.get_label(), "test");
    }

    #[test]
    fn test_scope_stats() {
        let mut stats = ScopeStats::default();
        stats.total_ns = 1_000_000; // 1ms
        stats.count = 2;
        stats.min_ns = 400_000;
        stats.max_ns = 600_000;

        assert_eq!(stats.avg_ns(), 500_000);
        assert!((stats.avg_ms() - 0.5).abs() < 0.001);
        assert!((stats.total_ms() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_text_report() {
        let _lock = TRACE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let collector = Collector::get_instance();
        collector.clear();
        collector.set_enabled(true);

        collector.begin_event("test_scope");
        collector.end_event("test_scope");
        collector.record_counter_value("test_counter", 42.0);

        let reporter = Reporter::new("test");
        let report = reporter.generate_text_report();

        assert!(report.contains("Trace Report"));
        assert!(report.contains("test_scope"));
        assert!(report.contains("test_counter"));

        collector.set_enabled(false);
        collector.clear();
    }

    #[test]
    fn test_json_report() {
        let _lock = TRACE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let collector = Collector::get_instance();
        collector.clear();
        collector.set_enabled(true);

        collector.begin_event("json_test");
        collector.end_event("json_test");

        let reporter = Reporter::with_config(
            "test",
            ReportConfig {
                format: ReportFormat::Json,
                ..Default::default()
            },
        );
        let report = reporter.generate_json_report();

        assert!(report.contains("\"scopes\""));
        assert!(report.contains("\"counters\""));
        assert!(report.contains("\"total_events\""));

        collector.set_enabled(false);
        collector.clear();
    }

    #[test]
    fn test_chrome_report() {
        let _lock = TRACE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let collector = Collector::get_instance();
        collector.clear();
        collector.set_enabled(true);

        collector.begin_event("chrome_test");
        collector.end_event("chrome_test");

        let reporter = Reporter::with_config(
            "test",
            ReportConfig {
                format: ReportFormat::Chrome,
                ..Default::default()
            },
        );
        let report = reporter.generate_chrome_report();

        assert!(report.contains("traceEvents"));
        assert!(report.contains("chrome_test"));

        collector.set_enabled(false);
        collector.clear();
    }

    #[test]
    fn test_set_format() {
        let mut reporter = Reporter::new("test");
        assert_eq!(reporter.config().format, ReportFormat::Text);

        reporter.set_format(ReportFormat::Json);
        assert_eq!(reporter.config().format, ReportFormat::Json);
    }

    #[test]
    fn test_counter_management() {
        let mut reporter = Reporter::new("test");

        let key1 = Token::new("counter1");
        let key2 = Token::new("counter2");

        assert!(reporter.add_counter(&key1, 0, 10.0));
        assert!(reporter.add_counter(&key2, 1, 20.0));

        // Duplicate key should fail
        assert!(!reporter.add_counter(&key1, 2, 30.0));

        // Duplicate index should fail
        assert!(!reporter.add_counter(&Token::new("counter3"), 0, 40.0));

        assert_eq!(reporter.get_counter_index(&key1), 0);
        assert_eq!(reporter.get_counter_index(&key2), 1);
        assert_eq!(reporter.get_counter_index(&Token::new("nonexistent")), -1);

        let counters = reporter.get_counters();
        assert_eq!(counters.get(&key1), Some(&10.0));
        assert_eq!(counters.get(&key2), Some(&20.0));
    }

    #[test]
    fn test_report_options() {
        let mut reporter = Reporter::new("test");

        assert!(reporter.get_group_by_function());
        assert!(!reporter.get_fold_recursive_calls());
        assert!(reporter.should_adjust_for_overhead_and_noise());

        reporter.set_group_by_function(false);
        reporter.set_fold_recursive_calls(true);
        reporter.set_should_adjust_for_overhead_and_noise(false);

        assert!(!reporter.get_group_by_function());
        assert!(reporter.get_fold_recursive_calls());
        assert!(!reporter.should_adjust_for_overhead_and_noise());
    }

    #[test]
    fn test_clear_tree() {
        let mut reporter = Reporter::new("test");

        reporter.add_counter(&Token::new("counter"), 0, 42.0);
        assert!(!reporter.get_counters().is_empty());

        reporter.clear_tree();
        assert!(reporter.get_counters().is_empty());
        assert!(reporter.get_event_tree().is_none());
        assert!(reporter.get_aggregate_tree().is_none());
    }

    #[test]
    fn test_text_format_output() {
        let _lock = TRACE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // Test that text format includes call tree and proper formatting
        let collector = Collector::get_instance();
        collector.clear();
        collector.set_enabled(true);

        collector.begin_event("outer");
        collector.begin_event("inner");
        collector.end_event("inner");
        collector.end_event("outer");

        let reporter = Reporter::new("text_format_test");
        let report = reporter.generate_text_report();

        // Should contain tree structure
        assert!(report.contains("Call Tree:"));
        assert!(report.contains("outer"));
        assert!(report.contains("inner"));

        // Should contain stats
        assert!(report.contains("Scope Statistics:"));
        assert!(report.contains("Total events:"));

        collector.set_enabled(false);
        collector.clear();
    }

    #[test]
    fn test_overhead_adjustment() {
        // Test that overhead adjustment reduces reported durations
        let mut reporter_adjusted = Reporter::new("adjusted");
        reporter_adjusted.set_should_adjust_for_overhead_and_noise(true);

        let mut reporter_raw = Reporter::new("raw");
        reporter_raw.set_should_adjust_for_overhead_and_noise(false);

        // Create test events with known timestamps
        let events = vec![
            Event {
                key: "test_fn".to_string(),
                event_type: EventType::Begin,
                timestamp: 1000,
                thread_id: std::thread::current().id(),
                category: 0,
            },
            Event {
                key: "test_fn".to_string(),
                event_type: EventType::End,
                timestamp: 2000,
                thread_id: std::thread::current().id(),
                category: 0,
            },
        ];

        let stats_adjusted = reporter_adjusted.calculate_stats(&events);
        let stats_raw = reporter_raw.calculate_stats(&events);

        let adj = stats_adjusted.get("test_fn").unwrap();
        let raw = stats_raw.get("test_fn").unwrap();

        // Raw should be 1000ns, adjusted should be 1000 - overhead
        assert_eq!(raw.total_ns, 1000);
        // Adjusted should be less than or equal to raw
        assert!(adj.total_ns <= raw.total_ns);
        assert_eq!(adj.count, 1);
        assert_eq!(raw.count, 1);
    }

    #[test]
    fn test_recursive_folding() {
        // Test that recursive calls are folded when enabled
        let tid = std::thread::current().id();

        let events = vec![
            Event {
                key: "recursive_fn".to_string(),
                event_type: EventType::Begin,
                timestamp: 1000,
                thread_id: tid,
                category: 0,
            },
            Event {
                key: "recursive_fn".to_string(),
                event_type: EventType::Begin,
                timestamp: 1100,
                thread_id: tid,
                category: 0,
            },
            Event {
                key: "recursive_fn".to_string(),
                event_type: EventType::Begin,
                timestamp: 1200,
                thread_id: tid,
                category: 0,
            },
            Event {
                key: "recursive_fn".to_string(),
                event_type: EventType::End,
                timestamp: 1300,
                thread_id: tid,
                category: 0,
            },
            Event {
                key: "recursive_fn".to_string(),
                event_type: EventType::End,
                timestamp: 1400,
                thread_id: tid,
                category: 0,
            },
            Event {
                key: "recursive_fn".to_string(),
                event_type: EventType::End,
                timestamp: 1500,
                thread_id: tid,
                category: 0,
            },
        ];

        // Without folding: should count all 3 calls
        let mut reporter_no_fold = Reporter::new("no_fold");
        reporter_no_fold.set_fold_recursive_calls(false);
        reporter_no_fold.set_should_adjust_for_overhead_and_noise(false);
        let stats_no_fold = reporter_no_fold.calculate_stats(&events);
        let no_fold = stats_no_fold.get("recursive_fn").unwrap();
        assert_eq!(no_fold.count, 3, "Without folding: 3 calls");

        // With folding: should count only the outermost call
        let mut reporter_fold = Reporter::new("fold");
        reporter_fold.set_fold_recursive_calls(true);
        reporter_fold.set_should_adjust_for_overhead_and_noise(false);
        let stats_fold = reporter_fold.calculate_stats(&events);
        let folded = stats_fold.get("recursive_fn").unwrap();
        assert_eq!(folded.count, 1, "With folding: only 1 outermost call");
    }
}
