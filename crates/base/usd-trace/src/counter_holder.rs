//! Counter holder for trace counter macros.
//!
//! Port of TraceCounterHolder from pxr/base/trace/trace.h

use super::collector::Collector;
use super::static_key_data::StaticKeyData;

/// Holds a counter key and provides fast recording methods.
///
/// This is used by the trace counter macros to efficiently record
/// counter values and deltas.
///
/// # Examples
///
/// ```
/// use usd_trace::{CounterHolder, StaticKeyData};
///
/// const KEY: StaticKeyData = StaticKeyData::new("items_processed");
/// let holder = CounterHolder::new(&KEY);
///
/// if holder.is_enabled() {
///     holder.record(5.0, true); // delta
/// }
/// ```
pub struct CounterHolder {
    /// The static key data.
    key: &'static StaticKeyData,
}

impl CounterHolder {
    /// Creates a new counter holder.
    ///
    /// # Arguments
    ///
    /// * `key` - Static key data for the counter
    pub const fn new(key: &'static StaticKeyData) -> Self {
        Self { key }
    }

    /// Returns whether tracing is enabled.
    ///
    /// This is a fast check using atomic operations.
    #[inline]
    pub fn is_enabled(&self) -> bool {
        Collector::is_enabled()
    }

    /// Records a counter value or delta.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to record
    /// * `is_delta` - If true, treat as delta; if false, treat as absolute value
    pub fn record(&self, value: f64, is_delta: bool) {
        if !self.is_enabled() {
            return;
        }

        let collector = Collector::get_instance();
        let key_str = self.key.get_string();

        if is_delta {
            collector.record_counter_delta(&key_str, value);
        } else {
            collector.record_counter_value(&key_str, value);
        }
    }

    /// Records a counter delta.
    ///
    /// # Arguments
    ///
    /// * `delta` - The delta value (can be positive or negative)
    #[inline]
    pub fn record_delta(&self, delta: f64) {
        self.record(delta, true);
    }

    /// Records an absolute counter value.
    ///
    /// # Arguments
    ///
    /// * `value` - The absolute value
    #[inline]
    pub fn record_value(&self, value: f64) {
        self.record(value, false);
    }

    /// Returns the counter key.
    pub fn key(&self) -> &'static StaticKeyData {
        self.key
    }
}

#[cfg(test)]
mod tests {
    use super::super::collector::TRACE_TEST_LOCK;
    use super::*;

    #[test]
    fn test_counter_holder_creation() {
        const KEY: StaticKeyData = StaticKeyData::new("test_counter");
        let holder = CounterHolder::new(&KEY);
        assert_eq!(holder.key().name(), "test_counter");
    }

    #[test]
    fn test_counter_holder_enabled() {
        let _lock = TRACE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        const KEY: StaticKeyData = StaticKeyData::new("enabled_test");
        let holder = CounterHolder::new(&KEY);

        let collector = Collector::get_instance();
        collector.set_enabled(true);

        assert!(holder.is_enabled());

        collector.set_enabled(false);
        assert!(!holder.is_enabled());
    }

    #[test]
    fn test_counter_holder_record_delta() {
        let _lock = TRACE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        const KEY: StaticKeyData = StaticKeyData::new("delta_counter");
        let holder = CounterHolder::new(&KEY);

        let collector = Collector::get_instance();
        collector.set_enabled(true);

        // Get baseline (may be non-zero from parallel tests)
        let baseline = collector
            .get_counters()
            .get("delta_counter")
            .copied()
            .unwrap_or(0.0);

        holder.record_delta(5.0);
        holder.record_delta(3.0);

        let counters = collector.get_counters();
        let value = counters.get("delta_counter").copied().unwrap_or(0.0);
        // Value should have increased by at least 8.0 from our deltas
        assert!(
            value >= baseline + 8.0,
            "Counter should have increased by at least 8.0: baseline={}, value={}",
            baseline,
            value
        );

        collector.set_enabled(false);
    }

    #[test]
    fn test_counter_holder_record_value() {
        let _lock = TRACE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        const KEY: StaticKeyData = StaticKeyData::new("value_counter");
        let holder = CounterHolder::new(&KEY);

        let collector = Collector::get_instance();
        collector.set_enabled(true);
        collector.clear();

        holder.record_value(100.0);

        let counters = collector.get_counters();
        assert_eq!(counters.get("value_counter").copied(), Some(100.0));

        collector.set_enabled(false);
        collector.clear();
    }

    #[test]
    fn test_counter_holder_disabled() {
        let _lock = TRACE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        const KEY: StaticKeyData = StaticKeyData::new("disabled_counter");
        let holder = CounterHolder::new(&KEY);

        let collector = Collector::get_instance();
        collector.set_enabled(false);
        collector.clear();

        holder.record_delta(10.0);

        // Should not record when disabled
        let counters = collector.get_counters();
        assert_eq!(counters.get("disabled_counter"), None);
    }
}
