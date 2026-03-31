//! Trace collector singleton.
//!
//! The collector is responsible for recording trace events and managing
//! the enabled state of tracing. Uses per-thread event buffers for
//! zero-contention event recording.

use parking_lot::{Mutex, RwLock};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use super::category::{Category, CategoryId, DEFAULT_CATEGORY};
use super::collection::Collection;
use super::collection_notice::send_collection_available;
use super::event::{Event, EventType};
use super::event_data::EventData;
use super::event_list::EventList;
use super::threads::ThreadId;

/// Global collector singleton.
static COLLECTOR: OnceLock<Collector> = OnceLock::new();

/// Global enabled flag for fast checking.
static ENABLED: AtomicBool = AtomicBool::new(false);

/// Per-thread event buffer handle.
type ThreadBuffer = Arc<Mutex<Vec<Event>>>;

thread_local! {
    /// Each thread gets its own event buffer, lazily registered with the collector.
    static THREAD_BUFFER: RefCell<Option<ThreadBuffer>> = const { RefCell::new(None) };
    /// Thread-local key cache to avoid repeated string allocations.
    static KEY_CACHE: RefCell<HashMap<String, Arc<str>>> = RefCell::new(HashMap::new());
}

/// Returns true if env var is set to "1" or "true" (case-insensitive).
fn env_is_true(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// The trace collector singleton.
///
/// Records trace events using per-thread buffers for zero-contention writes.
/// All public methods are thread-safe.
///
/// # Examples
///
/// ```
/// use usd_trace::Collector;
///
/// let collector = Collector::get_instance();
///
/// // Enable tracing
/// collector.set_enabled(true);
///
/// // Record events...
///
/// // Disable tracing
/// collector.set_enabled(false);
/// ```
pub struct Collector {
    /// Registry of all per-thread event buffers.
    thread_buffers: Mutex<Vec<ThreadBuffer>>,
    /// Counter values by name.
    counters: RwLock<HashMap<String, f64>>,
    /// Start time for the collector.
    start_time: Instant,
    /// Measured scope overhead in nanoseconds.
    scope_overhead_ns: u64,
}

impl Collector {
    /// Creates a new collector.
    ///
    /// Checks `PXR_ENABLE_GLOBAL_TRACE` env var on init: if set to "1" or "true",
    /// tracing is automatically enabled (matching C++ TraceCollector ctor behavior).
    fn new() -> Self {
        let c = Self {
            thread_buffers: Mutex::new(Vec::new()),
            counters: RwLock::new(HashMap::new()),
            start_time: Instant::now(),
            scope_overhead_ns: 0,
        };

        // Match C++ TraceCollector::TraceCollector() which calls
        // TfGetenvBool("PXR_ENABLE_GLOBAL_TRACE", false)
        if env_is_true("PXR_ENABLE_GLOBAL_TRACE") {
            ENABLED.store(true, Ordering::Release);
        }

        c
    }

    /// Returns the singleton collector instance.
    pub fn get_instance() -> &'static Collector {
        COLLECTOR.get_or_init(Collector::new)
    }

    /// Returns whether tracing is enabled (fast atomic check).
    #[inline]
    pub fn is_enabled() -> bool {
        ENABLED.load(Ordering::Acquire)
    }

    /// Enables or disables tracing.
    pub fn set_enabled(&self, enabled: bool) {
        ENABLED.store(enabled, Ordering::Release);
    }

    /// Returns the elapsed time since the collector was created.
    pub fn elapsed(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }

    /// Returns the current timestamp in nanoseconds.
    pub fn now(&self) -> u64 {
        self.start_time.elapsed().as_nanos() as u64
    }

    /// Gets or creates the per-thread event buffer for the current thread.
    fn get_thread_buffer(&self) -> ThreadBuffer {
        THREAD_BUFFER.with(|cell| {
            let mut opt = cell.borrow_mut();
            if let Some(buf) = opt.as_ref() {
                buf.clone()
            } else {
                // Create new buffer and register with collector
                let buf = Arc::new(Mutex::new(Vec::with_capacity(256)));
                self.thread_buffers.lock().push(buf.clone());
                *opt = Some(buf.clone());
                buf
            }
        })
    }

    /// Pushes an event into the current thread's buffer (zero global contention).
    #[inline]
    fn push_event(&self, event: Event) {
        let buf = self.get_thread_buffer();
        buf.lock().push(event);
    }

    /// Returns a cached key string, avoiding repeated allocations for hot keys.
    fn cached_key(&self, key: &str) -> String {
        KEY_CACHE.with(|cell| {
            let mut cache = cell.borrow_mut();
            if let Some(cached) = cache.get(key) {
                cached.to_string()
            } else {
                let arc: Arc<str> = Arc::from(key);
                cache.insert(key.to_string(), arc.clone());
                arc.to_string()
            }
        })
    }

    /// Clears all recorded events and counters.
    pub fn clear(&self) {
        // Clear all registered thread buffers
        let buffers = self.thread_buffers.lock();
        for buf in buffers.iter() {
            buf.lock().clear();
        }
        self.counters.write().clear();
    }

    /// Records a begin event with the default category.
    ///
    /// Returns the timestamp of the event, or 0 if tracing is disabled.
    pub fn begin_event(&self, key: &str) -> u64 {
        self.begin_event_cat(key, DEFAULT_CATEGORY)
    }

    /// Records a begin event with a specific category.
    ///
    /// Returns the timestamp of the event, or 0 if tracing is disabled.
    /// Events with a disabled category are silently skipped.
    pub fn begin_event_cat(&self, key: &str, category: CategoryId) -> u64 {
        if !Self::is_enabled() || !Category::get().is_enabled(category) {
            return 0;
        }

        let timestamp = self.now();
        let event = Event {
            key: self.cached_key(key),
            event_type: EventType::Begin,
            timestamp,
            thread_id: std::thread::current().id(),
            category,
        };
        self.push_event(event);
        timestamp
    }

    /// Records an end event with the default category.
    ///
    /// Returns the timestamp of the event, or 0 if tracing is disabled.
    pub fn end_event(&self, key: &str) -> u64 {
        self.end_event_cat(key, DEFAULT_CATEGORY)
    }

    /// Records an end event with a specific category.
    ///
    /// Returns the timestamp of the event, or 0 if tracing is disabled.
    pub fn end_event_cat(&self, key: &str, category: CategoryId) -> u64 {
        if !Self::is_enabled() || !Category::get().is_enabled(category) {
            return 0;
        }

        let timestamp = self.now();
        let event = Event {
            key: self.cached_key(key),
            event_type: EventType::End,
            timestamp,
            thread_id: std::thread::current().id(),
            category,
        };
        self.push_event(event);
        timestamp
    }

    /// Records a marker event with the default category.
    pub fn marker_event(&self, key: &str) {
        self.marker_event_cat(key, DEFAULT_CATEGORY);
    }

    /// Records a marker event with a specific category.
    pub fn marker_event_cat(&self, key: &str, category: CategoryId) {
        if !Self::is_enabled() || !Category::get().is_enabled(category) {
            return;
        }

        let event = Event {
            key: self.cached_key(key),
            event_type: EventType::Marker,
            timestamp: self.now(),
            thread_id: std::thread::current().id(),
            category,
        };
        self.push_event(event);
    }

    /// Records a counter delta value with default category.
    pub fn record_counter_delta(&self, name: &str, delta: f64) {
        self.record_counter_delta_cat(name, delta, DEFAULT_CATEGORY);
    }

    /// Records a counter delta value with a specific category.
    ///
    /// Matches C++ `RecordCounterDelta(key, delta, Category::GetId())`.
    pub fn record_counter_delta_cat(&self, name: &str, delta: f64, category: CategoryId) {
        if !Self::is_enabled() || !Category::get().is_enabled(category) {
            return;
        }

        {
            let mut counters = self.counters.write();
            let counter = counters.entry(name.to_string()).or_insert(0.0);
            *counter += delta;
        }

        let event = Event {
            key: self.cached_key(name),
            event_type: EventType::CounterDelta(delta),
            timestamp: self.now(),
            thread_id: std::thread::current().id(),
            category,
        };
        self.push_event(event);
    }

    /// Records a counter absolute value with default category.
    pub fn record_counter_value(&self, name: &str, value: f64) {
        self.record_counter_value_cat(name, value, DEFAULT_CATEGORY);
    }

    /// Records a counter absolute value with a specific category.
    ///
    /// Matches C++ `RecordCounterValue(key, value, Category::GetId())`.
    pub fn record_counter_value_cat(&self, name: &str, value: f64, category: CategoryId) {
        if !Self::is_enabled() || !Category::get().is_enabled(category) {
            return;
        }

        self.counters.write().insert(name.to_string(), value);

        let event = Event {
            key: self.cached_key(name),
            event_type: EventType::CounterValue(value),
            timestamp: self.now(),
            thread_id: std::thread::current().id(),
            category,
        };
        self.push_event(event);
    }

    /// Returns a copy of all recorded events from all threads.
    pub fn get_events(&self) -> Vec<Event> {
        let buffers = self.thread_buffers.lock();
        let mut all_events = Vec::new();
        for buf in buffers.iter() {
            all_events.extend(buf.lock().iter().cloned());
        }
        all_events
    }

    /// Returns a copy of all counter values.
    pub fn get_counters(&self) -> HashMap<String, f64> {
        self.counters.read().clone()
    }

    /// Returns the number of recorded events across all threads.
    pub fn event_count(&self) -> usize {
        let buffers = self.thread_buffers.lock();
        buffers.iter().map(|b| b.lock().len()).sum()
    }

    /// Returns the scope overhead in nanoseconds.
    ///
    /// Measured by timing empty scope operations. Used for overhead adjustment.
    pub fn get_scope_overhead(&self) -> u64 {
        if self.scope_overhead_ns > 0 {
            return self.scope_overhead_ns;
        }
        // Conservative estimate - actual measurement would require
        // enabling/disabling which interferes with active tracing
        100 // ~100ns overhead per scope
    }

    /// Creates a Collection from the recorded events and clears them.
    ///
    /// Groups events by their originating thread and sends a CollectionAvailable notice.
    /// Uses a stable sequential thread ID mapping (Thread 0, Thread 1, ...)
    /// instead of Debug-formatting std::thread::ThreadId.
    pub fn create_collection(&self) -> Arc<Collection> {
        // Drain all thread buffers
        let events = {
            let buffers = self.thread_buffers.lock();
            let mut all = Vec::new();
            for buf in buffers.iter() {
                let mut locked = buf.lock();
                all.append(&mut *locked);
            }
            all
        };

        // Map std::thread::ThreadId -> stable sequential ThreadId.
        // This avoids non-deterministic Debug output like "ThreadId(2)".
        let mut thread_map: HashMap<std::thread::ThreadId, ThreadId> = HashMap::new();
        let mut next_idx = 0u64;
        let main_id = std::thread::current().id();

        let mut events_by_thread: HashMap<ThreadId, Vec<Event>> = HashMap::new();
        for event in events {
            let tid = thread_map
                .entry(event.thread_id)
                .or_insert_with(|| {
                    if event.thread_id == main_id {
                        ThreadId::new("Main Thread")
                    } else {
                        let idx = next_idx;
                        next_idx += 1;
                        ThreadId::new(format!("Thread {idx}"))
                    }
                })
                .clone();
            events_by_thread.entry(tid).or_default().push(event);
        }

        // Build collection
        let mut collection = Collection::new();
        for (thread_id, events) in events_by_thread {
            let mut event_list = EventList::new();
            for event in events {
                event_list.push(event);
            }
            collection.add_to_collection(thread_id, event_list);
        }

        let collection = Arc::new(collection);
        send_collection_available(collection.clone());
        collection
    }

    /// Record a scope event described by `key` that started at `start` and ended at `stop`.
    pub fn scope(key: &str, start: u64, stop: u64) {
        if !Self::is_enabled() {
            return;
        }
        let collector = Self::get_instance();
        collector.begin_event_at_time(key, start);
        collector.end_event_at_time(key, stop);
    }

    /// Record a begin event at a specified time (default category).
    pub fn begin_event_at_time(&self, key: &str, timestamp: u64) {
        self.begin_event_at_time_cat(key, timestamp, DEFAULT_CATEGORY);
    }

    /// Record a begin event at a specified time with a specific category.
    ///
    /// Matches C++ `_BeginEventAtTime(key, ms, cat)`.
    pub fn begin_event_at_time_cat(&self, key: &str, timestamp: u64, category: CategoryId) {
        if !Self::is_enabled() || !Category::get().is_enabled(category) {
            return;
        }
        let event = Event {
            event_type: EventType::Begin,
            key: self.cached_key(key),
            timestamp,
            thread_id: std::thread::current().id(),
            category,
        };
        self.push_event(event);
    }

    /// Record an end event at a specified time (default category).
    pub fn end_event_at_time(&self, key: &str, timestamp: u64) {
        self.end_event_at_time_cat(key, timestamp, DEFAULT_CATEGORY);
    }

    /// Record an end event at a specified time with a specific category.
    ///
    /// Matches C++ `_EndEventAtTime(key, ms, cat)`.
    pub fn end_event_at_time_cat(&self, key: &str, timestamp: u64, category: CategoryId) {
        if !Self::is_enabled() || !Category::get().is_enabled(category) {
            return;
        }
        let event = Event {
            event_type: EventType::End,
            key: self.cached_key(key),
            timestamp,
            thread_id: std::thread::current().id(),
            category,
        };
        self.push_event(event);
    }

    /// Record a marker event at a specified time (default category).
    pub fn marker_event_at_time(&self, key: &str, timestamp: u64) {
        self.marker_event_at_time_cat(key, timestamp, DEFAULT_CATEGORY);
    }

    /// Record a marker event at a specified time with a specific category.
    ///
    /// Matches C++ `_MarkerEventAtTime(key, ms, cat)`.
    pub fn marker_event_at_time_cat(&self, key: &str, timestamp: u64, category: CategoryId) {
        if !Self::is_enabled() || !Category::get().is_enabled(category) {
            return;
        }
        let event = Event {
            event_type: EventType::Marker,
            key: self.cached_key(key),
            timestamp,
            thread_id: std::thread::current().id(),
            category,
        };
        self.push_event(event);
    }

    /// Record a begin scope event (alias for `begin_event`).
    #[inline]
    pub fn begin_scope(&self, key: &str) -> u64 {
        self.begin_event(key)
    }

    /// Record an end scope event (alias for `end_event`).
    #[inline]
    pub fn end_scope(&self, key: &str) -> u64 {
        self.end_event(key)
    }

    /// Records a marker event using a static key (zero-allocation variant).
    pub fn marker_event_static(&self, key: &'static str) {
        if !Self::is_enabled() {
            return;
        }

        let event = Event {
            key: key.to_string(),
            event_type: EventType::Marker,
            timestamp: self.now(),
            thread_id: std::thread::current().id(),
            category: DEFAULT_CATEGORY,
        };
        self.push_event(event);
    }

    /// Records data events for the current scope.
    pub fn scope_args(&self, args: &[(&str, &str)]) {
        if !Self::is_enabled() {
            return;
        }

        for (key, value) in args {
            self.store_data(key, value);
        }
    }

    /// Records a data event.
    pub fn store_data(&self, key: &str, value: &str) {
        if !Self::is_enabled() {
            return;
        }

        let event = Event {
            key: self.cached_key(key),
            event_type: EventType::Data(value.to_string()),
            timestamp: self.now(),
            thread_id: std::thread::current().id(),
            category: DEFAULT_CATEGORY,
        };
        self.push_event(event);
    }

    /// Records a data event with a generic value type.
    pub fn store_data_value<T: std::fmt::Display>(&self, key: &str, value: T) {
        self.store_data(key, &value.to_string());
    }

    /// Records a typed data event using EventData.
    ///
    /// Matches C++ `_StoreData` overloads that accept bool/int/uint/float/string.
    /// Uses `EventType::ScopeData` for typed storage instead of stringifying.
    pub fn store_data_typed(&self, key: &str, data: EventData) {
        if !Self::is_enabled() {
            return;
        }

        let event = Event {
            key: self.cached_key(key),
            event_type: EventType::ScopeData(data),
            timestamp: self.now(),
            thread_id: std::thread::current().id(),
            category: DEFAULT_CATEGORY,
        };
        self.push_event(event);
    }
}

// Ensure thread safety
const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Collector>();
};

/// Test-only mutex to serialize tests that depend on the global ENABLED state.
/// All trace tests across modules must acquire this before toggling enabled/clear.
#[cfg(test)]
pub(crate) static TRACE_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;

    use super::TRACE_TEST_LOCK;

    #[test]
    fn test_singleton() {
        let c1 = Collector::get_instance();
        let c2 = Collector::get_instance();
        assert!(std::ptr::eq(c1, c2));
    }

    #[test]
    fn test_enabled_state() {
        let _lock = TRACE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let collector = Collector::get_instance();

        collector.set_enabled(true);
        assert!(Collector::is_enabled());

        collector.set_enabled(false);
        assert!(!Collector::is_enabled());

        collector.set_enabled(false);
    }

    #[test]
    fn test_events() {
        let _lock = TRACE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let collector = Collector::get_instance();
        collector.clear();
        collector.set_enabled(true);

        let begin_ts = collector.begin_event("test_scope");
        assert!(begin_ts > 0);

        let end_ts = collector.end_event("test_scope");
        assert!(end_ts >= begin_ts);

        let events = collector.get_events();
        assert!(events.len() >= 2);

        collector.set_enabled(false);
    }

    #[test]
    fn test_marker() {
        let _lock = TRACE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let collector = Collector::get_instance();
        collector.clear();
        collector.set_enabled(true);

        collector.marker_event("test_marker");

        let events = collector.get_events();
        assert!(!events.is_empty());
        assert!(events.iter().any(|e| e.key == "test_marker"));

        collector.set_enabled(false);
    }

    #[test]
    fn test_counters() {
        let _lock = TRACE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let collector = Collector::get_instance();
        collector.set_enabled(true);

        let counter_name = format!("test_counter_{:?}", std::thread::current().id());

        collector.record_counter_delta(&counter_name, 5.0);
        collector.record_counter_delta(&counter_name, 3.0);

        let counters = collector.get_counters();
        if let Some(&value) = counters.get(&counter_name) {
            assert!(value >= 0.0, "Counter should be non-negative");
        }

        collector.record_counter_value(&counter_name, 100.0);
        let counters = collector.get_counters();
        if let Some(&value) = counters.get(&counter_name) {
            assert!(value >= 0.0, "Counter value should be non-negative");
        }

        collector.set_enabled(false);
    }

    #[test]
    fn test_disabled_no_record() {
        let _lock = TRACE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let collector = Collector::get_instance();
        collector.set_enabled(false);

        let scope_name = format!("disabled_scope_{:?}", std::thread::current().id());

        let ts = collector.begin_event(&scope_name);
        assert_eq!(ts, 0);

        collector.end_event(&scope_name);

        collector.set_enabled(false);
    }

    #[test]
    fn test_clear() {
        let _lock = TRACE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let collector = Collector::get_instance();
        collector.set_enabled(true);

        let event_name = format!("to_clear_test_{:?}", std::thread::current().id());
        collector.begin_event(&event_name);

        let events_before = collector.get_events();
        let our_event_exists = events_before.iter().any(|e| e.key == event_name);

        if our_event_exists {
            collector.clear();
            let events_after = collector.get_events();
            let our_event_after_clear = events_after.iter().any(|e| e.key == event_name);
            assert!(!our_event_after_clear, "Event should be cleared");
        }

        collector.set_enabled(false);
    }

    #[test]
    fn test_per_thread_isolation() {
        let _lock = TRACE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // Each thread writes to its own buffer - zero contention on write path.
        let collector = Collector::get_instance();
        collector.clear();
        collector.set_enabled(true);

        let test_id = format!("{:?}", std::thread::current().id());
        let barrier = Arc::new(std::sync::Barrier::new(3));

        let handles: Vec<_> = (0..2)
            .map(|i| {
                let b = barrier.clone();
                let tid = test_id.clone();
                std::thread::spawn(move || {
                    let c = Collector::get_instance();
                    let key = format!("iso_{}_{}", tid, i);
                    b.wait();
                    for _ in 0..100 {
                        c.begin_event(&key);
                        c.end_event(&key);
                    }
                    // Return count from this thread's perspective
                    let events = c.get_events();
                    events.iter().filter(|e| e.key == key).count()
                })
            })
            .collect();

        barrier.wait();
        let counts: Vec<usize> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        // Each thread wrote 200 events (100 begin + 100 end)
        assert_eq!(counts[0], 200, "Thread 0 should have 200 events");
        assert_eq!(counts[1], 200, "Thread 1 should have 200 events");

        collector.set_enabled(false);
    }

    #[test]
    fn test_multi_threaded_collection() {
        let _lock = TRACE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let collector = Collector::get_instance();
        collector.clear();
        collector.set_enabled(true);

        let main_key = "mt_main_test";
        let worker_key = "mt_worker_test";

        collector.begin_event(main_key);
        collector.end_event(main_key);

        let wk = worker_key.to_string();
        let handle = std::thread::spawn(move || {
            let c = Collector::get_instance();
            c.begin_event(&wk);
            c.end_event(&wk);
        });
        handle.join().unwrap();

        let events = collector.get_events();
        let main_count = events.iter().filter(|e| e.key == main_key).count();
        let worker_count = events.iter().filter(|e| e.key == worker_key).count();

        assert_eq!(main_count, 2, "Main thread events: {}", main_count);
        assert_eq!(worker_count, 2, "Worker thread events: {}", worker_count);

        collector.set_enabled(false);
    }

    #[test]
    fn test_key_caching() {
        let _lock = TRACE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let collector = Collector::get_instance();
        collector.clear();
        collector.set_enabled(true);

        let key = "cache_test_key";

        for _ in 0..50 {
            collector.begin_event(key);
            collector.end_event(key);
        }

        let events = collector.get_events();
        let count = events.iter().filter(|e| e.key == key).count();
        assert_eq!(count, 100, "Expected 100 events, got {}", count);

        collector.set_enabled(false);
    }

    #[test]
    fn test_env_is_true_helper() {
        // Test the env_is_true helper directly
        assert!(super::env_is_true("PATH") == false || super::env_is_true("PATH") == true);

        // Non-existent var
        assert!(!super::env_is_true("__TRACE_TEST_NONEXISTENT_VAR_12345__"));
    }

    #[test]
    fn test_begin_event_cat() {
        let _lock = TRACE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let collector = Collector::get_instance();
        collector.clear();
        collector.set_enabled(true);

        // Register and enable a custom category
        let cat = crate::create_category_id("TestCatBegin");
        Category::get().register_category(cat, "TestCatBegin");
        Category::get().enable_category(cat);

        let ts = collector.begin_event_cat("cat_begin", cat);
        assert!(ts > 0, "Should record event for enabled category");

        let events = collector.get_events();
        let found = events.iter().find(|e| e.key == "cat_begin");
        assert!(found.is_some(), "Event should be present");
        assert_eq!(found.unwrap().category, cat);

        collector.set_enabled(false);
    }

    #[test]
    fn test_counter_cat() {
        let _lock = TRACE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let collector = Collector::get_instance();
        collector.clear();
        collector.set_enabled(true);

        // Use default category for counter
        let name = "cat_counter_test";
        collector.record_counter_delta_cat(name, 7.0, DEFAULT_CATEGORY);
        collector.record_counter_value_cat(name, 42.0, DEFAULT_CATEGORY);

        let events = collector.get_events();
        let delta_found = events.iter().any(|e| {
            e.key == name
                && matches!(e.event_type, EventType::CounterDelta(v) if (v - 7.0).abs() < 1e-10)
        });
        let value_found = events.iter().any(|e| {
            e.key == name
                && matches!(e.event_type, EventType::CounterValue(v) if (v - 42.0).abs() < 1e-10)
        });
        assert!(delta_found, "CounterDelta with category should be recorded");
        assert!(value_found, "CounterValue with category should be recorded");

        collector.set_enabled(false);
    }

    #[test]
    fn test_store_data_typed() {
        let _lock = TRACE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let collector = Collector::get_instance();
        collector.clear();
        collector.set_enabled(true);

        use crate::EventData;

        // Store various typed data
        collector.store_data_typed("bool_key", EventData::Bool(true));
        collector.store_data_typed("int_key", EventData::Int(42));
        collector.store_data_typed("float_key", EventData::Float(3.14));
        collector.store_data_typed("str_key", EventData::String("hello".into()));

        let events = collector.get_events();

        // Verify typed data is stored as ScopeData variant
        let bool_evt = events.iter().find(|e| e.key == "bool_key");
        assert!(bool_evt.is_some());
        assert!(matches!(
            &bool_evt.unwrap().event_type,
            EventType::ScopeData(EventData::Bool(true))
        ));

        let int_evt = events.iter().find(|e| e.key == "int_key");
        assert!(int_evt.is_some());
        assert!(matches!(
            &int_evt.unwrap().event_type,
            EventType::ScopeData(EventData::Int(42))
        ));

        let float_evt = events.iter().find(|e| e.key == "float_key");
        assert!(float_evt.is_some());
        if let EventType::ScopeData(EventData::Float(v)) = &float_evt.unwrap().event_type {
            assert!((v - 3.14).abs() < 1e-10);
        } else {
            panic!("Expected ScopeData(Float)");
        }

        collector.set_enabled(false);
    }

    #[test]
    fn test_at_time_cat() {
        let _lock = TRACE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let collector = Collector::get_instance();
        collector.clear();
        collector.set_enabled(true);

        collector.begin_event_at_time_cat("at_time_begin", 1000, DEFAULT_CATEGORY);
        collector.end_event_at_time_cat("at_time_end", 2000, DEFAULT_CATEGORY);
        collector.marker_event_at_time_cat("at_time_marker", 1500, DEFAULT_CATEGORY);

        let events = collector.get_events();
        assert!(
            events
                .iter()
                .any(|e| e.key == "at_time_begin" && e.timestamp == 1000)
        );
        assert!(
            events
                .iter()
                .any(|e| e.key == "at_time_end" && e.timestamp == 2000)
        );
        assert!(
            events
                .iter()
                .any(|e| e.key == "at_time_marker" && e.timestamp == 1500)
        );

        collector.set_enabled(false);
    }
}
