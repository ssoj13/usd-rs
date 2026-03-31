//! Profiling utilities for performance analysis.
//!
//! Provides a simple init/shutdown API that enables the Collector,
//! and on shutdown writes a Chrome trace JSON file and prints a text summary.
//!
//! # Usage
//!
//! ```ignore
//! // At app start:
//! usd_trace::profiling::init();
//!
//! // ... run application ...
//!
//! // At app exit:
//! usd_trace::profiling::shutdown("trace.json");
//! ```

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::Collector;
use crate::aggregate_tree::AggregateTree;
use crate::event_tree::EventTree;
use crate::serialization::{write_chrome_trace, write_text_report};

/// Whether profiling has been initialized.
static PROFILING_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Initialize profiling: enables the trace Collector.
pub fn init() {
    let collector = Collector::get_instance();
    collector.set_enabled(true);
    PROFILING_ACTIVE.store(true, Ordering::Release);
    eprintln!("[profile] Trace collection enabled");
}

/// Returns true if profiling is currently active.
pub fn is_active() -> bool {
    PROFILING_ACTIVE.load(Ordering::Acquire)
}

/// Shutdown profiling: collect events, write chrome trace JSON, print text summary.
///
/// `output_path` is the path for the Chrome trace JSON file (e.g. "trace.json").
/// The file can be loaded in chrome://tracing or https://ui.perfetto.dev
pub fn shutdown(output_path: &str) {
    if !PROFILING_ACTIVE.load(Ordering::Acquire) {
        return;
    }

    let collector = Collector::get_instance();
    collector.set_enabled(false);
    PROFILING_ACTIVE.store(false, Ordering::Release);

    // Create collection from recorded events
    let collection = collector.create_collection();

    // Write Chrome trace JSON
    let json = write_chrome_trace(&collection);
    match std::fs::write(output_path, &json) {
        Ok(()) => {
            let size_kb = json.len() / 1024;
            eprintln!(
                "[profile] Trace written to {} ({} KB, {} events)",
                output_path,
                size_kb,
                collector.event_count()
            );
        }
        Err(e) => {
            eprintln!("[profile] Failed to write {}: {}", output_path, e);
        }
    }

    // Build aggregate tree and print text summary
    let event_tree = EventTree::from_collection(&collection);
    let tree = AggregateTree::from_event_tree(&event_tree);
    let report = write_text_report(&tree);
    if !report.is_empty() {
        eprintln!("\n{}", report);
    }
}

/// Shutdown profiling and write to a specific path.
/// Convenience wrapper that accepts a Path.
pub fn shutdown_to(output_path: &Path) {
    shutdown(&output_path.to_string_lossy());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_shutdown() {
        // Should not panic even if called multiple times
        init();
        assert!(is_active());
        shutdown("test_trace.json");
        assert!(!is_active());
        // Cleanup
        let _ = std::fs::remove_file("test_trace.json");
    }
}
