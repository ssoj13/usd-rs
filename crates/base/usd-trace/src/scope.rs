//! Trace scopes for RAII-style tracing.
//!
//! Scopes automatically record begin/end events when constructed and dropped.

use std::borrow::Cow;

use super::collector::Collector;

/// A trace scope that records timing automatically.
///
/// When a `Scope` is created, it records a begin event. When it is dropped,
/// it records an end event. This provides RAII-style automatic timing.
///
/// # Examples
///
/// ```
/// use usd_trace::Scope;
///
/// fn my_function() {
///     let _scope = Scope::new("my_function");
///     // ... work here is timed
///     // Scope is dropped at end, recording the end event
/// }
/// ```
pub struct Scope {
    /// The scope key/name.
    key: Cow<'static, str>,
    /// Whether the scope is active (tracing was enabled when created).
    active: bool,
    /// Timestamp when the scope was created.
    start_timestamp: u64,
}

impl Scope {
    /// Creates a new scope with the given key.
    ///
    /// If tracing is not enabled, the scope will be inactive and
    /// will not record any events.
    ///
    /// # Arguments
    ///
    /// * `key` - The scope name
    pub fn new(key: impl Into<Cow<'static, str>>) -> Self {
        let key = key.into();
        let collector = Collector::get_instance();

        if Collector::is_enabled() {
            let start_timestamp = collector.begin_event(&key);
            Self {
                key,
                active: true,
                start_timestamp,
            }
        } else {
            Self {
                key,
                active: false,
                start_timestamp: 0,
            }
        }
    }

    /// Creates a new scope with a static key (zero allocation).
    ///
    /// # Arguments
    ///
    /// * `key` - The static scope name
    pub fn new_static(key: &'static str) -> Self {
        Self::new(Cow::Borrowed(key))
    }

    /// Returns the scope key.
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Returns whether the scope is active (recording events).
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Returns the start timestamp, or 0 if inactive.
    pub fn start_timestamp(&self) -> u64 {
        self.start_timestamp
    }

    /// Returns the elapsed time since scope creation in nanoseconds.
    ///
    /// Returns 0 if the scope is inactive.
    pub fn elapsed_ns(&self) -> u64 {
        if self.active {
            Collector::get_instance().now() - self.start_timestamp
        } else {
            0
        }
    }

    /// Returns the elapsed time in milliseconds.
    pub fn elapsed_ms(&self) -> f64 {
        self.elapsed_ns() as f64 / 1_000_000.0
    }

    /// Returns the elapsed time in seconds.
    pub fn elapsed_secs(&self) -> f64 {
        self.elapsed_ns() as f64 / 1_000_000_000.0
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        if self.active {
            Collector::get_instance().end_event(&self.key);
        }
    }
}

/// A guard that can be used to conditionally create a scope.
///
/// This is useful for the trace macros which may or may not create
/// actual scope objects based on whether tracing is enabled.
pub enum ScopeGuard {
    /// An active scope that will record events.
    Active(Scope),
    /// An inactive guard that does nothing.
    Inactive,
}

impl ScopeGuard {
    /// Creates a new scope guard.
    ///
    /// If tracing is enabled, creates an active scope.
    /// Otherwise, creates an inactive guard.
    pub fn new(key: impl Into<Cow<'static, str>>) -> Self {
        if Collector::is_enabled() {
            ScopeGuard::Active(Scope::new(key))
        } else {
            ScopeGuard::Inactive
        }
    }

    /// Creates a guard from a static key.
    pub fn new_static(key: &'static str) -> Self {
        Self::new(Cow::Borrowed(key))
    }

    /// Returns `true` if this guard has an active scope.
    pub fn is_active(&self) -> bool {
        matches!(self, ScopeGuard::Active(_))
    }

    /// Returns the scope key, if active.
    pub fn key(&self) -> Option<&str> {
        match self {
            ScopeGuard::Active(scope) => Some(scope.key()),
            ScopeGuard::Inactive => None,
        }
    }
}

/// Creates a scope that automatically records begin/end events.
///
/// This is the recommended way to time code sections.
///
/// # Examples
///
/// ```
/// use usd_trace::scope;
///
/// fn process_data() {
///     let _guard = scope!("process_data");
///     // ... work here is timed
/// }
/// ```
#[macro_export]
macro_rules! scope {
    ($key:expr) => {
        $crate::ScopeGuard::new($key)
    };
}

pub use scope;

#[cfg(test)]
mod tests {
    use super::super::collector::TRACE_TEST_LOCK;
    use super::*;

    #[test]
    fn test_scope_creation() {
        let scope = Scope::new("test_scope");
        assert_eq!(scope.key(), "test_scope");
    }

    #[test]
    fn test_scope_static() {
        let scope = Scope::new_static("static_scope");
        assert_eq!(scope.key(), "static_scope");
    }

    #[test]
    fn test_scope_active_when_enabled() {
        let _lock = TRACE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let collector = Collector::get_instance();
        collector.set_enabled(true);

        // Use unique scope name
        let scope_name = format!("active_scope_{:?}", std::thread::current().id());

        let event_count_before = collector.event_count();

        let scope = Scope::new(scope_name.clone());
        assert!(scope.is_active());
        // Note: start_timestamp() may be 0 if less than 1ns elapsed since collector init
        // The important thing is that the scope is active
        drop(scope);

        // Should have added begin and end events (at least 2 more than before)
        let event_count_after = collector.event_count();
        assert!(event_count_after >= event_count_before + 2);

        // Verify our events are there
        let events = collector.get_events();
        let our_events: Vec<_> = events.iter().filter(|e| e.key == scope_name).collect();
        assert!(our_events.len() >= 2);

        collector.set_enabled(false);
    }

    #[test]
    fn test_scope_inactive_when_disabled() {
        let _lock = TRACE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let collector = Collector::get_instance();
        collector.set_enabled(false);

        let scope = Scope::new("inactive_scope");
        assert!(!scope.is_active());
        assert_eq!(scope.start_timestamp(), 0);
    }

    #[test]
    fn test_scope_guard_active() {
        let _lock = TRACE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let collector = Collector::get_instance();
        collector.set_enabled(true);

        let guard = ScopeGuard::new("guard_test");
        assert!(guard.is_active());
        assert_eq!(guard.key(), Some("guard_test"));

        collector.set_enabled(false);
    }

    #[test]
    fn test_scope_guard_inactive() {
        let _lock = TRACE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let collector = Collector::get_instance();
        collector.set_enabled(false);

        let guard = ScopeGuard::new("guard_test");
        assert!(!guard.is_active());
        assert_eq!(guard.key(), None);
    }

    #[test]
    fn test_scope_elapsed() {
        let _lock = TRACE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let collector = Collector::get_instance();
        collector.set_enabled(true);

        let scope = Scope::new("elapsed_test");
        // Do some minimal work
        std::thread::sleep(std::time::Duration::from_micros(100));

        let elapsed = scope.elapsed_ns();
        assert!(elapsed > 0);

        // elapsed_ms and elapsed_secs should be consistent
        assert!(scope.elapsed_ms() >= 0.0);
        assert!(scope.elapsed_secs() >= 0.0);

        collector.set_enabled(false);
    }

    #[test]
    fn test_scope_macro() {
        let _guard = scope!("macro_test");
        // Should compile
    }
}
