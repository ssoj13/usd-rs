//! RefPtr tracking for debugging reference counting issues.
//!
//! This module provides tracking of [`RefPtr`] objects to particular objects.
//! This is useful for debugging when a ref counted object has a ref count that
//! should have gone to zero but didn't.
//!
//! # Overview
//!
//! The tracker can tell you every [`RefPtr`] that's holding a [`RefBase`] and
//! a stack trace where it was created or last assigned to.
//!
//! # Examples
//!
//! ```
//! use usd_tf::{RefPtrTracker, TraceType};
//!
//! let tracker = RefPtrTracker::instance();
//!
//! // Get the current stack trace depth
//! let depth = tracker.stack_trace_max_depth();
//!
//! // Report all watched objects
//! let mut output = Vec::new();
//! tracker.report_all_watched_counts(&mut output);
//! ```
//!
//! # Enabling Tracking
//!
//! To enable tracking for a type, implement the `RefPtrTrackable` trait
//! and call the appropriate tracker methods in your `RefPtr` implementation.
//!
//! [`RefPtr`]: super::RefPtr
//! [`RefBase`]: super::RefBase

use std::collections::HashMap;
use std::io::Write;
use std::sync::{Mutex, OnceLock};

/// The type of trace event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TraceType {
    /// A new RefPtr was created pointing to the object.
    Add,
    /// An existing RefPtr was assigned to point to the object.
    Assign,
}

impl TraceType {
    /// Get the string representation.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            TraceType::Add => "Add",
            TraceType::Assign => "Assign",
        }
    }
}

impl std::fmt::Display for TraceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A single trace record.
#[derive(Debug, Clone)]
pub struct Trace {
    /// The stack trace when the RefPtr was created or assigned.
    /// Each element is an instruction pointer address.
    pub trace: Vec<usize>,
    /// The object being pointed to (as raw pointer address).
    pub obj: usize,
    /// Whether the RefPtr was created or assigned.
    pub trace_type: TraceType,
}

impl Trace {
    /// Create a new trace.
    #[must_use]
    pub fn new(trace: Vec<usize>, obj: usize, trace_type: TraceType) -> Self {
        Self {
            trace,
            obj,
            trace_type,
        }
    }
}

/// Maps a RefPtr address to the most recent trace for it.
pub type OwnerTraces = HashMap<usize, Trace>;

/// Maps a RefBase object pointer to the number of RefPtr objects using it.
pub type WatchedCounts = HashMap<usize, usize>;

/// Default maximum stack trace depth.
const DEFAULT_MAX_DEPTH: usize = 20;

/// Number of internal stack levels to skip when capturing traces.
const NUM_INTERNAL_STACK_LEVELS: usize = 2;

/// Singleton for tracking RefPtr objects for debugging.
///
/// This tracker maintains a list of "watched" objects and records stack traces
/// whenever a RefPtr is created or assigned to point to a watched object.
pub struct RefPtrTracker {
    /// Mutex protecting all mutable state.
    inner: Mutex<RefPtrTrackerInner>,
}

struct RefPtrTrackerInner {
    /// Maximum stack trace depth.
    max_depth: usize,
    /// Watched objects and their owner counts.
    watched: WatchedCounts,
    /// Traces for all owners.
    traces: OwnerTraces,
}

impl RefPtrTracker {
    /// Create a new tracker.
    fn new() -> Self {
        Self {
            inner: Mutex::new(RefPtrTrackerInner {
                max_depth: DEFAULT_MAX_DEPTH,
                watched: HashMap::new(),
                traces: HashMap::new(),
            }),
        }
    }

    /// Get the singleton instance.
    #[must_use]
    pub fn instance() -> &'static Self {
        static INSTANCE: OnceLock<RefPtrTracker> = OnceLock::new();
        INSTANCE.get_or_init(RefPtrTracker::new)
    }

    /// Returns the maximum stack trace depth.
    #[must_use]
    pub fn stack_trace_max_depth(&self) -> usize {
        self.inner
            .lock()
            .expect("RefPtrTracker lock poisoned")
            .max_depth
    }

    /// Sets the maximum stack trace depth.
    pub fn set_stack_trace_max_depth(&self, depth: usize) {
        self.inner
            .lock()
            .expect("RefPtrTracker lock poisoned")
            .max_depth = depth;
    }

    /// Returns the watched objects and the number of owners of each.
    ///
    /// Returns a copy for thread safety.
    #[must_use]
    pub fn watched_counts(&self) -> WatchedCounts {
        self.inner
            .lock()
            .expect("RefPtrTracker lock poisoned")
            .watched
            .clone()
    }

    /// Returns traces for all owners.
    ///
    /// Returns a copy for thread safety.
    #[must_use]
    pub fn all_traces(&self) -> OwnerTraces {
        self.inner
            .lock()
            .expect("RefPtrTracker lock poisoned")
            .traces
            .clone()
    }

    /// Writes all watched objects and the number of owners of each to `writer`.
    pub fn report_all_watched_counts<W: Write>(&self, writer: &mut W) {
        let inner = self.inner.lock().expect("RefPtrTracker lock poisoned");

        let _ = writeln!(writer, "TfRefPtrTracker watched counts:");
        for (obj, count) in &inner.watched {
            let _ = writeln!(writer, "  {:#x}: {} owners", obj, count);
        }
    }

    /// Writes all traces to `writer`.
    pub fn report_all_traces<W: Write>(&self, writer: &mut W) {
        let inner = self.inner.lock().expect("RefPtrTracker lock poisoned");

        let _ = writeln!(writer, "TfRefPtrTracker traces:");
        for (owner, trace) in &inner.traces {
            let _ = writeln!(
                writer,
                "  Owner: {:#x} {} {:#x}:",
                owner, trace.trace_type, trace.obj
            );
            let _ = writeln!(
                writer,
                "=============================================================="
            );
            for (i, addr) in trace.trace.iter().enumerate() {
                let _ = writeln!(writer, "    #{}: {:#x}", i, addr);
            }
            let _ = writeln!(writer);
        }
    }

    /// Writes traces for all owners of `watched` to `writer`.
    pub fn report_traces_for_watched<W: Write>(&self, writer: &mut W, watched: usize) {
        let inner = self.inner.lock().expect("RefPtrTracker lock poisoned");

        // Report if not watched
        if !inner.watched.contains_key(&watched) {
            let _ = writeln!(
                writer,
                "TfRefPtrTracker traces for {:#x}: not watched",
                watched
            );
            return;
        }

        let _ = writeln!(writer, "TfRefPtrTracker traces for {:#x}:", watched);

        // Report traces for this watched object
        for (owner, trace) in &inner.traces {
            if trace.obj == watched {
                let _ = writeln!(writer, "  Owner: {:#x} {}:", owner, trace.trace_type);
                let _ = writeln!(
                    writer,
                    "=============================================================="
                );
                for (i, addr) in trace.trace.iter().enumerate() {
                    let _ = writeln!(writer, "    #{}: {:#x}", i, addr);
                }
                let _ = writeln!(writer);
            }
        }

        let _ = writeln!(
            writer,
            "=============================================================="
        );
    }

    /// Start watching an object. Only watched objects are traced.
    pub fn watch(&self, obj: usize) {
        let mut inner = self.inner.lock().expect("RefPtrTracker lock poisoned");
        inner.watched.insert(obj, 0);
    }

    /// Stop watching an object. Existing traces for the object are kept.
    pub fn unwatch(&self, obj: usize) {
        let mut inner = self.inner.lock().expect("RefPtrTracker lock poisoned");
        inner.watched.remove(&obj);
    }

    /// Add a trace for a new owner of an object if the object is being watched.
    pub fn add_trace(&self, owner: usize, obj: usize, trace_type: TraceType) {
        let mut inner = self.inner.lock().expect("RefPtrTracker lock poisoned");

        // Decrement count for previous object if owner was tracking something
        let old_obj = inner.traces.get(&owner).map(|t| t.obj);
        if let Some(old_obj_addr) = old_obj {
            if let Some(count) = inner.watched.get_mut(&old_obj_addr) {
                *count = count.saturating_sub(1);
            }
        }

        // Check if new object is being watched
        if let Some(count) = inner.watched.get_mut(&obj) {
            *count += 1;

            // Capture stack trace
            let trace_addrs = capture_stack_trace(inner.max_depth, NUM_INTERNAL_STACK_LEVELS);

            inner
                .traces
                .insert(owner, Trace::new(trace_addrs, obj, trace_type));
        } else if inner.traces.contains_key(&owner) {
            // Owner assigned to unwatched object, remove its trace
            inner.traces.remove(&owner);
        }
    }

    /// Remove traces for an owner.
    pub fn remove_traces(&self, owner: usize) {
        let mut inner = self.inner.lock().expect("RefPtrTracker lock poisoned");

        if let Some(trace) = inner.traces.remove(&owner) {
            // Decrement the count for the object it was pointing to
            if let Some(count) = inner.watched.get_mut(&trace.obj) {
                *count = count.saturating_sub(1);
            }
        }
    }

    /// Check if an object is being watched.
    #[must_use]
    pub fn is_watched(&self, obj: usize) -> bool {
        self.inner
            .lock()
            .expect("RefPtrTracker lock poisoned")
            .watched
            .contains_key(&obj)
    }

    /// Get the number of watched objects.
    #[must_use]
    pub fn watched_count(&self) -> usize {
        self.inner
            .lock()
            .expect("RefPtrTracker lock poisoned")
            .watched
            .len()
    }

    /// Get the number of traces.
    #[must_use]
    pub fn trace_count(&self) -> usize {
        self.inner
            .lock()
            .expect("RefPtrTracker lock poisoned")
            .traces
            .len()
    }

    /// Clear all watches and traces.
    pub fn clear(&self) {
        let mut inner = self.inner.lock().expect("RefPtrTracker lock poisoned");
        inner.watched.clear();
        inner.traces.clear();
    }
}

/// Handy function to pass as a predicate - no objects will be watched.
///
/// Use this when you want to track derived types but not the base type itself.
#[inline]
#[must_use]
pub fn watch_none<T>(_: &T) -> bool {
    false
}

/// Handy function to pass as a predicate - all objects will be watched.
#[inline]
#[must_use]
pub fn watch_all<T>(_: &T) -> bool {
    true
}

/// Trait for types that can be tracked by `RefPtrTracker`.
pub trait RefPtrTrackable {
    /// Returns true if this instance should be watched.
    fn should_watch(&self) -> bool;
}

/// Capture a stack trace.
///
/// Returns a vector of instruction pointer addresses.
/// Note: Stack trace capture is a placeholder. For full stack traces,
/// integrate with the backtrace crate or platform-specific APIs.
fn capture_stack_trace(_max_depth: usize, _skip: usize) -> Vec<usize> {
    // Stack trace capture is optional and platform-dependent.
    // Return empty for now - actual implementation would use
    // platform-specific APIs or the backtrace crate.
    Vec::new()
}

/// Utility struct for RefPtr tracking operations.
///
/// This provides static methods that wrap the singleton tracker.
pub struct RefPtrTrackerUtil;

impl RefPtrTrackerUtil {
    /// Start watching an object.
    #[inline]
    pub fn watch(obj: usize) {
        RefPtrTracker::instance().watch(obj);
    }

    /// Stop watching an object.
    #[inline]
    pub fn unwatch(obj: usize) {
        RefPtrTracker::instance().unwatch(obj);
    }

    /// Add a trace for a new owner.
    #[inline]
    pub fn add_trace(owner: usize, obj: usize, trace_type: TraceType) {
        RefPtrTracker::instance().add_trace(owner, obj, trace_type);
    }

    /// Remove traces for an owner.
    #[inline]
    pub fn remove_traces(owner: usize) {
        RefPtrTracker::instance().remove_traces(owner);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_singleton() {
        let t1 = RefPtrTracker::instance();
        let t2 = RefPtrTracker::instance();
        assert!(std::ptr::eq(t1, t2));
    }

    #[test]
    fn test_max_depth() {
        let tracker = RefPtrTracker::instance();
        let original = tracker.stack_trace_max_depth();

        tracker.set_stack_trace_max_depth(50);
        assert_eq!(tracker.stack_trace_max_depth(), 50);

        // Restore
        tracker.set_stack_trace_max_depth(original);
    }

    #[test]
    fn test_watch_unwatch() {
        let tracker = RefPtrTracker::instance();
        let obj_addr: usize = 0x1234_5678;

        // Initially not watched
        assert!(!tracker.is_watched(obj_addr));

        // Watch it
        tracker.watch(obj_addr);
        assert!(tracker.is_watched(obj_addr));

        // Check counts
        let counts = tracker.watched_counts();
        assert_eq!(counts.get(&obj_addr), Some(&0));

        // Unwatch it
        tracker.unwatch(obj_addr);
        assert!(!tracker.is_watched(obj_addr));
    }

    #[test]
    fn test_add_remove_traces() {
        let tracker = RefPtrTracker::instance();
        let obj_addr: usize = 0xABCD_0001;
        let owner_addr: usize = 0xDEAD_BEEF;

        // Watch the object
        tracker.watch(obj_addr);

        // Add a trace
        tracker.add_trace(owner_addr, obj_addr, TraceType::Add);

        // Check the count increased
        let counts = tracker.watched_counts();
        assert_eq!(counts.get(&obj_addr), Some(&1));

        // Check trace exists
        let traces = tracker.all_traces();
        assert!(traces.contains_key(&owner_addr));
        assert_eq!(traces[&owner_addr].obj, obj_addr);
        assert_eq!(traces[&owner_addr].trace_type, TraceType::Add);

        // Remove the trace
        tracker.remove_traces(owner_addr);

        // Check count decreased
        let counts = tracker.watched_counts();
        assert_eq!(counts.get(&obj_addr), Some(&0));

        // Clean up
        tracker.unwatch(obj_addr);
    }

    #[test]
    fn test_trace_type_display() {
        assert_eq!(TraceType::Add.as_str(), "Add");
        assert_eq!(TraceType::Assign.as_str(), "Assign");
        assert_eq!(format!("{}", TraceType::Add), "Add");
        assert_eq!(format!("{}", TraceType::Assign), "Assign");
    }

    #[test]
    fn test_report_watched_counts() {
        let tracker = RefPtrTracker::instance();
        let obj_addr: usize = 0xABCD_0002;

        tracker.watch(obj_addr);

        let mut output = Vec::new();
        tracker.report_all_watched_counts(&mut output);

        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.contains("TfRefPtrTracker watched counts:"));

        tracker.unwatch(obj_addr);
    }

    #[test]
    fn test_report_all_traces() {
        let tracker = RefPtrTracker::instance();

        let mut output = Vec::new();
        tracker.report_all_traces(&mut output);

        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.contains("TfRefPtrTracker traces:"));
    }

    #[test]
    fn test_report_traces_for_unwatched() {
        let tracker = RefPtrTracker::instance();
        let obj_addr: usize = 0xFFFF_FFFF;

        let mut output = Vec::new();
        tracker.report_traces_for_watched(&mut output, obj_addr);

        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.contains("not watched"));
    }

    #[test]
    fn test_watch_none() {
        assert!(!watch_none(&42));
        assert!(!watch_none(&"hello"));
    }

    #[test]
    fn test_watch_all() {
        assert!(watch_all(&42));
        assert!(watch_all(&"hello"));
    }

    #[test]
    fn test_util_methods() {
        let obj_addr: usize = 0xABCD_0003;
        let owner_addr: usize = 0xDEAD_0001;

        RefPtrTrackerUtil::watch(obj_addr);
        assert!(RefPtrTracker::instance().is_watched(obj_addr));

        RefPtrTrackerUtil::add_trace(owner_addr, obj_addr, TraceType::Add);
        RefPtrTrackerUtil::remove_traces(owner_addr);

        RefPtrTrackerUtil::unwatch(obj_addr);
        assert!(!RefPtrTracker::instance().is_watched(obj_addr));
    }

    #[test]
    fn test_assign_trace_type() {
        let tracker = RefPtrTracker::instance();
        let obj_addr: usize = 0xABCD_0004;
        let owner_addr: usize = 0xDEAD_0002;

        tracker.watch(obj_addr);
        tracker.add_trace(owner_addr, obj_addr, TraceType::Assign);

        let traces = tracker.all_traces();
        assert_eq!(traces[&owner_addr].trace_type, TraceType::Assign);

        tracker.remove_traces(owner_addr);
        tracker.unwatch(obj_addr);
    }

    #[test]
    fn test_reassign_owner() {
        let tracker = RefPtrTracker::instance();
        let obj1: usize = 0xABCD_0005;
        let obj2: usize = 0xABCD_0006;
        let owner: usize = 0xDEAD_0003;

        tracker.watch(obj1);
        tracker.watch(obj2);

        // Add trace to obj1
        tracker.add_trace(owner, obj1, TraceType::Add);
        assert_eq!(tracker.watched_counts().get(&obj1), Some(&1));
        assert_eq!(tracker.watched_counts().get(&obj2), Some(&0));

        // Reassign to obj2
        tracker.add_trace(owner, obj2, TraceType::Assign);
        assert_eq!(tracker.watched_counts().get(&obj1), Some(&0));
        assert_eq!(tracker.watched_counts().get(&obj2), Some(&1));

        // Clean up
        tracker.remove_traces(owner);
        tracker.unwatch(obj1);
        tracker.unwatch(obj2);
    }
}
