//! Trace events.
//!
//! This module defines the event types that are recorded by the trace collector.

use std::hash::{Hash, Hasher};
use std::thread::ThreadId;

use super::category::{CategoryId, DEFAULT_CATEGORY};
use super::event_data::EventData;

// ============================================================================
// TimeStamp
// ============================================================================

/// A timestamp for trace events.
///
/// Stores time as ticks (nanoseconds) for high precision.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TimeStamp {
    ticks: u64,
}

impl TimeStamp {
    /// Creates a new timestamp from ticks (nanoseconds).
    pub const fn from_ticks(ticks: u64) -> Self {
        Self { ticks }
    }

    /// Creates a new timestamp from seconds.
    pub fn from_seconds(seconds: f64) -> Self {
        Self {
            ticks: (seconds * 1_000_000_000.0) as u64,
        }
    }

    /// Returns the timestamp as ticks (nanoseconds).
    pub const fn as_ticks(&self) -> u64 {
        self.ticks
    }

    /// Returns the timestamp as seconds.
    pub fn as_seconds(&self) -> f64 {
        self.ticks as f64 / 1_000_000_000.0
    }

    /// Returns the timestamp as milliseconds.
    pub fn as_millis(&self) -> f64 {
        self.ticks as f64 / 1_000_000.0
    }

    /// Returns the timestamp as microseconds.
    pub fn as_micros(&self) -> f64 {
        self.ticks as f64 / 1_000.0
    }

    /// Returns a zero timestamp.
    pub const fn zero() -> Self {
        Self { ticks: 0 }
    }
}

impl std::ops::Add for TimeStamp {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            ticks: self.ticks + rhs.ticks,
        }
    }
}

impl std::ops::Sub for TimeStamp {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            ticks: self.ticks.saturating_sub(rhs.ticks),
        }
    }
}

impl From<u64> for TimeStamp {
    fn from(ticks: u64) -> Self {
        Self::from_ticks(ticks)
    }
}

impl From<TimeStamp> for u64 {
    fn from(ts: TimeStamp) -> Self {
        ts.ticks
    }
}

// ============================================================================
// TraceKey
// ============================================================================

/// A trace key that can be either static or dynamic.
#[derive(Debug, Clone)]
pub struct Key {
    name: String,
}

impl Key {
    /// Creates a new key from a string.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }

    /// Creates a key from a static string.
    pub const fn from_static(name: &'static str) -> StaticKeyRef {
        StaticKeyRef { name }
    }

    /// Returns the key name.
    pub fn text(&self) -> &str {
        &self.name
    }
}

impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for Key {}

impl Hash for Key {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl From<&str> for Key {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for Key {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

/// A reference to a static key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StaticKeyRef {
    name: &'static str,
}

impl StaticKeyRef {
    /// Returns the key name.
    pub const fn text(&self) -> &'static str {
        self.name
    }
}

// ============================================================================
// EventType
// ============================================================================

/// The type of a trace event.
#[derive(Debug, Clone, PartialEq)]
pub enum EventType {
    /// Begin of a scope/span.
    Begin,
    /// End of a scope/span.
    End,
    /// A marker event (point in time).
    Marker,
    /// A counter delta event.
    CounterDelta(f64),
    /// A counter absolute value event.
    CounterValue(f64),
    /// A timespan event with duration in nanoseconds.
    Timespan(u64),
    /// Custom data event (string only, for backward compat).
    Data(String),
    /// Typed scope data event (matches C++ ScopeData with DataType).
    ScopeData(EventData),
}

impl EventType {
    /// Returns the name of the event type.
    pub fn name(&self) -> &'static str {
        match self {
            EventType::Begin => "begin",
            EventType::End => "end",
            EventType::Marker => "marker",
            EventType::CounterDelta(_) => "counter_delta",
            EventType::CounterValue(_) => "counter_value",
            EventType::Timespan(_) => "timespan",
            EventType::Data(_) => "data",
            EventType::ScopeData(_) => "scope_data",
        }
    }

    /// Returns `true` if this is a begin event.
    pub fn is_begin(&self) -> bool {
        matches!(self, EventType::Begin)
    }

    /// Returns `true` if this is an end event.
    pub fn is_end(&self) -> bool {
        matches!(self, EventType::End)
    }

    /// Returns `true` if this is a marker event.
    pub fn is_marker(&self) -> bool {
        matches!(self, EventType::Marker)
    }

    /// Returns `true` if this is a counter event.
    pub fn is_counter(&self) -> bool {
        matches!(
            self,
            EventType::CounterDelta(_) | EventType::CounterValue(_)
        )
    }

    /// Returns `true` if this is a data event (either Data or ScopeData).
    pub fn is_data(&self) -> bool {
        matches!(self, EventType::Data(_) | EventType::ScopeData(_))
    }
}

/// A trace event.
///
/// Events represent points in time or spans of time that are recorded
/// by the trace collector for profiling purposes.
#[derive(Debug, Clone)]
pub struct Event {
    /// The event key/name.
    pub key: String,
    /// The type of event.
    pub event_type: EventType,
    /// Timestamp in nanoseconds since collector start.
    pub timestamp: u64,
    /// The thread that recorded this event.
    pub thread_id: ThreadId,
    /// Category for filtering (matches C++ TraceCategoryId).
    pub category: CategoryId,
}

impl Event {
    /// Creates a new event with the default category.
    pub fn new(key: impl Into<String>, event_type: EventType, timestamp: u64) -> Self {
        Self {
            key: key.into(),
            event_type,
            timestamp,
            thread_id: std::thread::current().id(),
            category: DEFAULT_CATEGORY,
        }
    }

    /// Creates a new event with a specific category.
    pub fn with_category(
        key: impl Into<String>,
        event_type: EventType,
        timestamp: u64,
        category: CategoryId,
    ) -> Self {
        Self {
            key: key.into(),
            event_type,
            timestamp,
            thread_id: std::thread::current().id(),
            category,
        }
    }

    /// Creates a begin event.
    pub fn begin(key: impl Into<String>, timestamp: u64) -> Self {
        Self::new(key, EventType::Begin, timestamp)
    }

    /// Creates an end event.
    pub fn end(key: impl Into<String>, timestamp: u64) -> Self {
        Self::new(key, EventType::End, timestamp)
    }

    /// Creates a marker event.
    pub fn marker(key: impl Into<String>, timestamp: u64) -> Self {
        Self::new(key, EventType::Marker, timestamp)
    }

    /// Creates a timespan event.
    pub fn timespan(key: impl Into<String>, timestamp: u64, duration_ns: u64) -> Self {
        Self::new(key, EventType::Timespan(duration_ns), timestamp)
    }

    /// Returns the event key.
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Returns the event type.
    pub fn event_type(&self) -> &EventType {
        &self.event_type
    }

    /// Returns the timestamp.
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    /// Returns the timestamp in seconds.
    pub fn timestamp_seconds(&self) -> f64 {
        self.timestamp as f64 / 1_000_000_000.0
    }

    /// Returns the timestamp in milliseconds.
    pub fn timestamp_millis(&self) -> f64 {
        self.timestamp as f64 / 1_000_000.0
    }

    /// Returns the thread ID.
    pub fn thread_id(&self) -> ThreadId {
        self.thread_id
    }

    /// Returns the event category.
    pub fn category(&self) -> CategoryId {
        self.category
    }

    /// Returns the counter value if this is a counter event.
    ///
    /// Returns `None` if this is not a counter event.
    pub fn get_counter_value(&self) -> Option<f64> {
        match &self.event_type {
            EventType::CounterDelta(v) | EventType::CounterValue(v) => Some(*v),
            _ => None,
        }
    }

    /// Returns the start timestamp for a timespan event.
    ///
    /// For timespan events, returns the stored start time.
    /// For begin events, returns the timestamp.
    /// Returns `None` for other event types.
    pub fn get_start_timestamp(&self) -> Option<u64> {
        match &self.event_type {
            EventType::Begin => Some(self.timestamp),
            EventType::Timespan(_) => Some(self.timestamp), // Start time is stored as timestamp
            _ => None,
        }
    }

    /// Returns the end timestamp for a timespan event.
    ///
    /// For timespan events, computes end from start + duration.
    /// For end events, returns the timestamp.
    /// Returns `None` for other event types.
    pub fn get_end_timestamp(&self) -> Option<u64> {
        match &self.event_type {
            EventType::End => Some(self.timestamp),
            EventType::Timespan(duration) => Some(self.timestamp + duration),
            _ => None,
        }
    }

    /// Returns the data stored in a string data event.
    ///
    /// Returns `None` if this is not a Data event.
    pub fn get_data(&self) -> Option<&str> {
        match &self.event_type {
            EventType::Data(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Returns the typed data stored in a ScopeData event.
    ///
    /// Returns `None` if this is not a ScopeData event.
    pub fn get_scope_data(&self) -> Option<&EventData> {
        match &self.event_type {
            EventType::ScopeData(d) => Some(d),
            _ => None,
        }
    }

    /// Returns the duration for timespan events.
    ///
    /// Returns `None` for non-timespan events.
    pub fn get_duration(&self) -> Option<u64> {
        match &self.event_type {
            EventType::Timespan(duration) => Some(*duration),
            _ => None,
        }
    }

    /// Sets the timestamp of the event.
    pub fn set_timestamp(&mut self, time: u64) {
        self.timestamp = time;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_type_names() {
        assert_eq!(EventType::Begin.name(), "begin");
        assert_eq!(EventType::End.name(), "end");
        assert_eq!(EventType::Marker.name(), "marker");
        assert_eq!(EventType::CounterDelta(1.0).name(), "counter_delta");
        assert_eq!(EventType::CounterValue(1.0).name(), "counter_value");
        assert_eq!(EventType::Timespan(100).name(), "timespan");
    }

    #[test]
    fn test_event_type_checks() {
        assert!(EventType::Begin.is_begin());
        assert!(!EventType::Begin.is_end());

        assert!(EventType::End.is_end());
        assert!(!EventType::End.is_begin());

        assert!(EventType::Marker.is_marker());

        assert!(EventType::CounterDelta(1.0).is_counter());
        assert!(EventType::CounterValue(1.0).is_counter());
    }

    #[test]
    fn test_event_creation() {
        let event = Event::begin("test", 1000);
        assert_eq!(event.key(), "test");
        assert!(event.event_type().is_begin());
        assert_eq!(event.timestamp(), 1000);
    }

    #[test]
    fn test_event_timestamps() {
        let event = Event::new("test", EventType::Marker, 1_000_000_000);
        assert_eq!(event.timestamp_seconds(), 1.0);
        assert_eq!(event.timestamp_millis(), 1000.0);
    }

    #[test]
    fn test_timespan_event() {
        let event = Event::timespan("test", 1000, 500);
        match event.event_type() {
            EventType::Timespan(duration) => assert_eq!(*duration, 500),
            _ => panic!("Expected timespan event"),
        }
    }

    #[test]
    fn test_timestamp_from_ticks() {
        let ts = TimeStamp::from_ticks(1_000_000_000);
        assert_eq!(ts.as_ticks(), 1_000_000_000);
        assert!((ts.as_seconds() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_timestamp_from_seconds() {
        let ts = TimeStamp::from_seconds(1.5);
        assert_eq!(ts.as_ticks(), 1_500_000_000);
    }

    #[test]
    fn test_timestamp_arithmetic() {
        let t1 = TimeStamp::from_ticks(1000);
        let t2 = TimeStamp::from_ticks(500);

        let sum = t1 + t2;
        assert_eq!(sum.as_ticks(), 1500);

        let diff = t1 - t2;
        assert_eq!(diff.as_ticks(), 500);
    }

    #[test]
    fn test_key() {
        let key = Key::new("test");
        assert_eq!(key.text(), "test");
    }

    #[test]
    fn test_counter_value() {
        let event = Event::new("counter", EventType::CounterDelta(5.5), 1000);
        assert_eq!(event.get_counter_value(), Some(5.5));

        let event = Event::new("counter", EventType::CounterValue(10.0), 1000);
        assert_eq!(event.get_counter_value(), Some(10.0));

        let event = Event::begin("test", 1000);
        assert_eq!(event.get_counter_value(), None);
    }

    #[test]
    fn test_timespan_timestamps() {
        let event = Event::timespan("test", 1000, 500);
        assert_eq!(event.get_start_timestamp(), Some(1000));
        assert_eq!(event.get_end_timestamp(), Some(1500));
        assert_eq!(event.get_duration(), Some(500));
    }

    #[test]
    fn test_data_event() {
        let event = Event::new("data", EventType::Data("hello".to_string()), 1000);
        assert_eq!(event.get_data(), Some("hello"));
    }

    #[test]
    fn test_set_timestamp() {
        let mut event = Event::begin("test", 1000);
        event.set_timestamp(2000);
        assert_eq!(event.timestamp(), 2000);
    }

    #[test]
    fn test_scope_data_variant() {
        let sd = EventType::ScopeData(EventData::Bool(true));
        assert_eq!(sd.name(), "scope_data");
        assert!(sd.is_data());
        assert!(!sd.is_begin());
        assert!(!sd.is_counter());

        let sd_int = EventType::ScopeData(EventData::Int(42));
        assert!(sd_int.is_data());
    }

    #[test]
    fn test_event_category() {
        let event = Event::new("test", EventType::Begin, 1000);
        assert_eq!(event.category(), DEFAULT_CATEGORY);

        let cat = crate::create_category_id("CustomEvt");
        let event = Event::with_category("test", EventType::Begin, 1000, cat);
        assert_eq!(event.category(), cat);
    }

    #[test]
    fn test_get_scope_data() {
        let event = Event::new("data", EventType::ScopeData(EventData::Float(2.71)), 1000);
        let sd = event.get_scope_data();
        assert!(sd.is_some());
        assert_eq!(sd.unwrap().get_float(), Some(2.71));

        // Non-ScopeData event returns None
        let event = Event::begin("test", 1000);
        assert!(event.get_scope_data().is_none());
    }
}
