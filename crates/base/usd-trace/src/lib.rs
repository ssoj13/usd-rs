#![allow(unsafe_code)]
//! Trace - Performance Tracing System.
//!
//! This module provides performance tracing utilities for profiling USD code.
//! It wraps the `tracing` crate to provide USD-like APIs while integrating
//! with the Rust ecosystem.
//!
//! # Key Components
//!
//! - [`Collector`] - Singleton trace collector
//! - [`Scope`] - RAII scope guard for timing
//! - [`Reporter`] - Trace reporting utilities
//! - [`Category`] - Category management for filtering events
//! - Macros: `trace_function!`, `trace_scope!`, `trace_marker!`
//!
//! # Examples
//!
//! ```
//! use usd_trace::{trace_function, trace_scope, Collector};
//!
//! fn my_function() {
//!     trace_function!();
//!     // ... work here is timed
//!     
//!     {
//!         trace_scope!("inner_work");
//!         // ... more timed work
//!     }
//! }
//! ```
//!
//! # Backend
//!
//! This module uses the `tracing` crate as its backend, providing:
//! - Structured logging and tracing
//! - Integration with tracing-subscriber for output
//! - Zero-cost when tracing is disabled
//! - Async support

pub mod aggregate_node;
pub mod aggregate_tree;
pub mod category;
pub mod collection;
pub mod collection_notice;
pub mod collector;
pub mod concurrent_list;
pub mod counter_accumulator;
pub mod counter_holder;
pub mod data_buffer;
pub mod dynamic_key;
pub mod event;
pub mod event_container;
pub mod event_data;
pub mod event_list;
pub mod event_node;
pub mod event_tree;
pub mod event_tree_builder;
pub mod key;
pub mod profiling;
pub mod reporter;
pub mod reporter_data_source;
pub mod scope;
pub mod serialization;
pub mod static_key_data;
pub mod string_hash;
pub mod threads;
pub mod trace_auto;

pub use aggregate_node::AggregateNode;
pub use aggregate_tree::AggregateTree;
pub use category::{Category, CategoryId, DEFAULT_CATEGORY, create_category_id};
pub use collection::{Collection, Visitor as CollectionVisitor};
pub use collection_notice::{
    CollectionAvailable, CollectionListener, CollectionNoticeRegistry,
    global_registry as collection_notice_registry, send_collection_available,
};
pub use collector::Collector;
pub use concurrent_list::ConcurrentList;
pub use counter_accumulator::{CounterAccumulator, CounterMap, CounterValue};
pub use counter_holder::CounterHolder;
pub use data_buffer::DataBuffer;
pub use dynamic_key::{DynamicKey as TraceDynamicKey, StaticKeyData as DynamicStaticKeyData};
pub use event::{Event, EventType, Key as TraceKey, TimeStamp};
pub use event_container::EventContainer;
pub use event_data::{DataType, EventData};
pub use event_list::EventList;
pub use event_node::EventNode;
pub use event_tree::{EventTree, EventTreeBuilder};
pub use event_tree_builder::{FullEventTreeBuilder, MarkerValue, MarkerValuesMap};
pub use key::{DynamicKey, Key, StaticKey};
pub use reporter::{ReportConfig, ReportFormat, Reporter, ScopeStats};
pub use reporter_data_source::{
    CollectionDataSource, CollectorDataSource, ReporterBase, ReporterDataSource,
};
pub use scope::{Scope, ScopeGuard};
pub use serialization::{
    ChromeTraceWriter, TextReportWriter, collection_from_json, write_chrome_trace,
    write_collections_to_json, write_text_report,
};
pub use static_key_data::StaticKeyData;
pub use string_hash::StringHash;
pub use threads::{ThreadId, get_thread_id};
pub use trace_auto::{TraceAuto, TraceScopeAuto};

/// Re-export tracing for users who want direct access.
/// Note: Our trace macros (trace_function!, trace_scope!, trace_marker!)
/// use our Collector directly, matching C++ TRACE_* macro behavior.
pub use tracing;

/// Records a timestamp when constructed and a timespan event when destructed,
/// using the name of the function as the key.
///
/// Uses our Collector directly (matching C++ TRACE_FUNCTION macro behavior).
///
/// # Examples
///
/// ```
/// use usd_trace::trace_function;
///
/// fn my_expensive_function() {
///     trace_function!();
///     // ... work here is timed
/// }
/// ```
#[macro_export]
macro_rules! trace_function {
    () => {
        let _trace_guard = if $crate::Collector::is_enabled() {
            let _c = $crate::Collector::get_instance();
            let _key = ::std::any::type_name::<fn()>();
            let _ts = _c.begin_event(_key);
            Some((_key, _ts))
        } else {
            None
        };
        // Use a drop guard to record end event
        struct _TraceFunctionGuard {
            key: &'static str,
            active: bool,
        }
        impl Drop for _TraceFunctionGuard {
            fn drop(&mut self) {
                if self.active {
                    $crate::Collector::get_instance().end_event(self.key);
                }
            }
        }
        let _trace_fn_guard = _TraceFunctionGuard {
            key: ::std::any::type_name::<fn()>(),
            active: _trace_guard.is_some(),
        };
    };
}

/// Records a timestamp when constructed and a timespan event when destructed,
/// using the provided name as the key.
///
/// Uses our Collector directly (matching C++ TRACE_SCOPE macro behavior).
///
/// # Examples
///
/// ```
/// use usd_trace::trace_scope;
///
/// fn process_data() {
///     {
///         trace_scope!("loading");
///         // ... loading work
///     }
///     {
///         trace_scope!("processing");
///         // ... processing work
///     }
/// }
/// ```
#[macro_export]
macro_rules! trace_scope {
    ($name:expr) => {
        let _trace_scope_active = $crate::Collector::is_enabled();
        if _trace_scope_active {
            $crate::Collector::get_instance().begin_event($name);
        }
        struct _TraceScopeGuard {
            name: &'static str,
            active: bool,
        }
        impl Drop for _TraceScopeGuard {
            fn drop(&mut self) {
                if self.active {
                    $crate::Collector::get_instance().end_event(self.name);
                }
            }
        }
        let _trace_scope_guard = _TraceScopeGuard {
            name: $name,
            active: _trace_scope_active,
        };
    };
}

/// Records a marker event at the current time.
///
/// Uses our Collector directly (matching C++ TRACE_MARKER_SCOPED macro behavior).
///
/// # Examples
///
/// ```
/// use usd_trace::trace_marker;
///
/// fn process() {
///     trace_marker!("start_processing");
///     // ... work
///     trace_marker!("checkpoint_1");
///     // ... more work
///     trace_marker!("end_processing");
/// }
/// ```
#[macro_export]
macro_rules! trace_marker {
    ($name:expr) => {
        if $crate::Collector::is_enabled() {
            $crate::Collector::get_instance().marker_event($name);
        }
    };
}

/// Records a counter delta value.
///
/// # Examples
///
/// ```
/// use usd_trace::trace_counter_delta;
///
/// fn process_items(items: &[Item]) {
///     for item in items {
///         trace_counter_delta!("items_processed", 1.0);
///         process_item(item);
///     }
/// }
/// # struct Item;
/// # fn process_item(_: &Item) {}
/// ```
#[macro_export]
macro_rules! trace_counter_delta {
    ($name:expr, $delta:expr) => {
        if $crate::Collector::is_enabled() {
            $crate::Collector::get_instance().record_counter_delta($name, $delta);
        }
    };
}

/// Records a counter absolute value.
///
/// # Examples
///
/// ```
/// use usd_trace::trace_counter_value;
///
/// fn update_cache_size(size: usize) {
///     trace_counter_value!("cache_size", size as f64);
/// }
/// ```
#[macro_export]
macro_rules! trace_counter_value {
    ($name:expr, $value:expr) => {
        if $crate::Collector::is_enabled() {
            $crate::Collector::get_instance().record_counter_value($name, $value);
        }
    };
}

// Macros are automatically available via #[macro_export] — no re-export needed.

#[cfg(test)]
mod tests {
    use super::collector::TRACE_TEST_LOCK;
    use super::*;

    #[test]
    fn test_collector_singleton() {
        let c1 = Collector::get_instance();
        let c2 = Collector::get_instance();
        assert!(std::ptr::eq(c1, c2));
    }

    #[test]
    fn test_collector_enabled() {
        let _lock = TRACE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let collector = Collector::get_instance();

        collector.set_enabled(true);
        assert!(Collector::is_enabled());

        collector.set_enabled(false);
        assert!(!Collector::is_enabled());
    }

    #[test]
    fn test_trace_scope_macro() {
        trace_scope!("test_scope");
        // Should compile and not panic
    }

    #[test]
    fn test_trace_marker_macro() {
        trace_marker!("test_marker");
        // Should compile and not panic
    }

    #[test]
    fn test_trace_function_macro() {
        trace_function!();
        // Should compile and not panic
    }

    #[test]
    fn test_category_singleton() {
        let c1 = Category::get();
        let c2 = Category::get();
        assert!(std::ptr::eq(c1, c2));
    }

    #[test]
    fn test_create_category_id() {
        const ID1: CategoryId = create_category_id("Test");
        const ID2: CategoryId = create_category_id("Test");
        assert_eq!(ID1, ID2);
    }
}
