//! TraceCounterAccumulator - Accumulates counter values.
//!
//! Port of pxr/base/trace/counterAccumulator.h

use std::collections::HashMap;

/// Accumulated counter data.
#[derive(Debug, Clone)]
pub struct CounterValue {
    /// Current accumulated value.
    pub value: f64,
    /// Whether this counter uses delta semantics.
    pub is_delta: bool,
}

impl Default for CounterValue {
    fn default() -> Self {
        Self {
            value: 0.0,
            is_delta: false,
        }
    }
}

/// Accumulates counter sample values over time.
///
/// This class is used to accumulate counter values from trace events,
/// tracking both absolute values and deltas.
#[derive(Debug, Default)]
pub struct CounterAccumulator {
    /// Counter values by name.
    counters: HashMap<String, CounterValue>,
}

impl CounterAccumulator {
    /// Creates a new empty accumulator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a delta counter value.
    pub fn record_delta(&mut self, name: &str, delta: f64) {
        let entry = self
            .counters
            .entry(name.to_string())
            .or_insert(CounterValue {
                value: 0.0,
                is_delta: true,
            });
        entry.value += delta;
        entry.is_delta = true;
    }

    /// Records an absolute counter value.
    pub fn record_value(&mut self, name: &str, value: f64) {
        let entry = self.counters.entry(name.to_string()).or_default();
        entry.value = value;
        entry.is_delta = false;
    }

    /// Returns the current value of a counter.
    pub fn get_value(&self, name: &str) -> Option<f64> {
        self.counters.get(name).map(|v| v.value)
    }

    /// Returns whether a counter uses delta semantics.
    pub fn is_delta(&self, name: &str) -> Option<bool> {
        self.counters.get(name).map(|v| v.is_delta)
    }

    /// Returns all counter values.
    pub fn get_all(&self) -> &HashMap<String, CounterValue> {
        &self.counters
    }

    /// Returns a reference to the counters map.
    pub fn counters(&self) -> &CounterMap {
        &self.counters
    }

    /// Sets the current counter values.
    pub fn set_current_values(&mut self, values: CounterMap) {
        self.counters = values;
    }

    /// Updates the accumulator from a collection.
    pub fn update(&mut self, collection: &super::Collection) {
        for (_thread_id, event_list) in collection.iter() {
            for event in event_list.iter() {
                match &event.event_type {
                    super::EventType::CounterDelta(delta) => {
                        self.record_delta(event.key(), *delta);
                    }
                    super::EventType::CounterValue(value) => {
                        self.record_value(event.key(), *value);
                    }
                    _ => {}
                }
            }
        }
    }

    /// Returns an iterator over counter names.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.counters.keys().map(String::as_str)
    }

    /// Returns the number of tracked counters.
    pub fn len(&self) -> usize {
        self.counters.len()
    }

    /// Returns true if no counters are being tracked.
    pub fn is_empty(&self) -> bool {
        self.counters.is_empty()
    }

    /// Clears all accumulated counter values.
    pub fn clear(&mut self) {
        self.counters.clear();
    }

    /// Resets all counters to their initial values without removing them.
    pub fn reset(&mut self) {
        for value in self.counters.values_mut() {
            value.value = 0.0;
        }
    }
}

/// Counter map type alias.
pub type CounterMap = HashMap<String, CounterValue>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_counter_delta() {
        let mut acc = CounterAccumulator::new();

        acc.record_delta("items", 5.0);
        assert_eq!(acc.get_value("items"), Some(5.0));
        assert_eq!(acc.is_delta("items"), Some(true));

        acc.record_delta("items", 3.0);
        assert_eq!(acc.get_value("items"), Some(8.0));
    }

    #[test]
    fn test_counter_value() {
        let mut acc = CounterAccumulator::new();

        acc.record_value("cache_size", 100.0);
        assert_eq!(acc.get_value("cache_size"), Some(100.0));
        assert_eq!(acc.is_delta("cache_size"), Some(false));

        acc.record_value("cache_size", 150.0);
        assert_eq!(acc.get_value("cache_size"), Some(150.0));
    }

    #[test]
    fn test_counter_reset() {
        let mut acc = CounterAccumulator::new();

        acc.record_delta("a", 10.0);
        acc.record_value("b", 20.0);

        acc.reset();

        assert_eq!(acc.get_value("a"), Some(0.0));
        assert_eq!(acc.get_value("b"), Some(0.0));
        assert_eq!(acc.len(), 2);
    }

    #[test]
    fn test_counter_clear() {
        let mut acc = CounterAccumulator::new();

        acc.record_delta("a", 10.0);
        acc.record_value("b", 20.0);

        acc.clear();

        assert!(acc.is_empty());
        assert_eq!(acc.get_value("a"), None);
    }
}
