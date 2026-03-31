//! Automatic scope tracing helpers.
//!
//! Port of TraceAuto and TraceScopeAuto from pxr/base/trace/trace.h

use super::collector::Collector;
use super::static_key_data::StaticKeyData;
use std::sync::atomic::{Ordering, fence};

/// RAII scope guard using static key data.
///
/// This is the Rust equivalent of C++ `TraceScopeAuto`.
/// It records a scope event with precise timing using static keys.
///
/// # Examples
///
/// ```
/// use usd_trace::{StaticKeyData, TraceScopeAuto};
///
/// const KEY: StaticKeyData = StaticKeyData::new("my_scope");
///
/// fn my_function() {
///     let _scope = TraceScopeAuto::new(&KEY);
///     // ... work here is timed
/// }
/// ```
pub struct TraceScopeAuto {
    /// Static key for the scope.
    key: &'static StaticKeyData,
    /// Start timestamp.
    start_ticks: u64,
    /// Whether the scope is active.
    started: bool,
}

impl TraceScopeAuto {
    /// Creates a new automatic scope.
    ///
    /// Records begin timestamp if tracing is enabled.
    ///
    /// # Arguments
    ///
    /// * `key` - Static key data for the scope
    pub fn new(key: &'static StaticKeyData) -> Self {
        let started = Collector::is_enabled();
        let start_ticks = if started {
            Collector::get_instance().now()
        } else {
            0
        };

        Self {
            key,
            start_ticks,
            started,
        }
    }

    /// Creates a scope with additional data arguments.
    ///
    /// # Arguments
    ///
    /// * `key` - Static key data
    /// * `args` - Key-value pairs to store with the scope
    pub fn with_args(key: &'static StaticKeyData, args: &[(&str, &str)]) -> Self {
        let started = Collector::is_enabled();
        let start_ticks = if started {
            let collector = Collector::get_instance();
            let ts = collector.now();
            collector.scope_args(args);
            ts
        } else {
            0
        };

        Self {
            key,
            start_ticks,
            started,
        }
    }

    /// Returns whether the scope is active.
    pub fn is_started(&self) -> bool {
        self.started
    }

    /// Returns the start timestamp.
    pub fn start_ticks(&self) -> u64 {
        self.start_ticks
    }
}

impl Drop for TraceScopeAuto {
    fn drop(&mut self) {
        if self.started {
            let stop_ticks = Collector::get_instance().now();
            let key_str = self.key.get_string();
            Collector::scope(&key_str, self.start_ticks, stop_ticks);
        }
    }
}

/// Dynamic scope tracing helper.
///
/// This is the Rust equivalent of C++ `TraceAuto`.
/// Unlike `TraceScopeAuto`, this uses dynamic keys (runtime strings).
///
/// # Examples
///
/// ```
/// use usd_trace::TraceAuto;
///
/// fn my_function(name: &str) {
///     let _scope = TraceAuto::new(name);
///     // ... work here is timed
/// }
/// ```
pub struct TraceAuto {
    /// Dynamic key.
    key: String,
    /// Collector reference.
    collector: &'static Collector,
}

impl TraceAuto {
    /// Creates a new dynamic scope from a string.
    ///
    /// Records begin event if tracing is enabled.
    pub fn new(key: impl Into<String>) -> Self {
        fence(Ordering::SeqCst);
        let collector = Collector::get_instance();
        let key = key.into();
        collector.begin_event(&key);
        fence(Ordering::SeqCst);

        Self { key, collector }
    }

    /// Creates a scope for a function with a custom suffix.
    ///
    /// Builds key as "function_name [suffix]".
    ///
    /// # Arguments
    ///
    /// * `func_name` - Function name
    /// * `pretty_func_name` - Pretty function name (with signature)
    /// * `suffix` - Custom scope suffix
    pub fn new_function(_func_name: &str, pretty_func_name: &str, suffix: &str) -> Self {
        let key = format!("{} [{}]", pretty_func_name, suffix);
        Self::new(key)
    }

    /// Returns the scope key.
    pub fn key(&self) -> &str {
        &self.key
    }
}

impl Drop for TraceAuto {
    fn drop(&mut self) {
        fence(Ordering::SeqCst);
        self.collector.end_event(&self.key);
        fence(Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::super::collector::TRACE_TEST_LOCK;
    use super::*;

    #[test]
    fn test_trace_scope_auto() {
        let _lock = TRACE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        const KEY: StaticKeyData = StaticKeyData::new("test_scope_auto");

        let collector = Collector::get_instance();
        collector.clear();
        collector.set_enabled(true);

        {
            let _scope = TraceScopeAuto::new(&KEY);
            assert!(_scope.is_started());
        }

        let count = collector
            .get_events()
            .iter()
            .filter(|e| e.key == KEY.name())
            .count();
        assert!(
            count >= 2,
            "Expected events for key '{}', got {}",
            KEY.name(),
            count
        );

        collector.set_enabled(false);
    }

    #[test]
    fn test_trace_scope_auto_with_args() {
        let _lock = TRACE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        const KEY: StaticKeyData = StaticKeyData::new("test_with_args");

        let collector = Collector::get_instance();
        collector.clear();
        collector.set_enabled(true);

        {
            let _scope = TraceScopeAuto::with_args(&KEY, &[("size", "1024"), ("type", "buffer")]);
            assert!(_scope.is_started());
        }

        let count = collector
            .get_events()
            .iter()
            .filter(|e| e.key == KEY.name())
            .count();
        // Should have scope events (begin + end)
        assert!(count >= 2, "Expected at least 2 events, got {}", count);

        collector.set_enabled(false);
    }

    #[test]
    fn test_trace_auto() {
        let _lock = TRACE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let collector = Collector::get_instance();
        collector.clear();
        collector.set_enabled(true);

        {
            let _scope = TraceAuto::new("dynamic_scope");
            assert_eq!(_scope.key(), "dynamic_scope");
        }

        let count = collector
            .get_events()
            .iter()
            .filter(|e| e.key == "dynamic_scope")
            .count();
        assert!(
            count >= 2,
            "Expected events for 'dynamic_scope', got {}",
            count
        );

        collector.set_enabled(false);
    }

    #[test]
    fn test_trace_auto_function() {
        let _lock = TRACE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let collector = Collector::get_instance();
        collector.clear();
        collector.set_enabled(true);

        let expected_key_part = "void my_fn()";

        {
            let _scope = TraceAuto::new_function("my_fn", "void my_fn()", "phase1");
            assert!((_scope.key()).contains("void my_fn()"));
            assert!((_scope.key()).contains("phase1"));
        }

        let count = collector
            .get_events()
            .iter()
            .filter(|e| e.key.contains(expected_key_part))
            .count();
        assert!(
            count >= 2,
            "Expected events containing '{}', got {}",
            expected_key_part,
            count
        );

        collector.set_enabled(false);
    }

    #[test]
    fn test_trace_scope_auto_disabled() {
        let _lock = TRACE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        const KEY: StaticKeyData = StaticKeyData::new("disabled_scope");

        let collector = Collector::get_instance();
        collector.set_enabled(false);

        let scope = TraceScopeAuto::new(&KEY);
        assert!(!scope.is_started());
        assert_eq!(scope.start_ticks(), 0);
    }
}
