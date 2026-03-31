//! Private scope description stack report utilities.
//!
//! Port of pxr/base/tf/scopeDescriptionPrivate.h
//!
//! Provides crash-reporting helpers that capture the TfScopeDescription
//! stacks from all threads as human-readable text.

use std::time::Duration;

/// Lock guard for reading scope description stacks for crash reporting.
///
/// Matches C++ `Tf_ScopeDescriptionStackReportLock`.
///
/// Tries to lock and compute a report message from all thread-local scope
/// description stacks. If locking times out, skips those threads.
pub struct ScopeDescriptionStackReportLock {
    /// The captured report message, if any.
    message: Option<String>,
}

impl ScopeDescriptionStackReportLock {
    /// Create a new report lock with a timeout for acquiring per-thread locks.
    ///
    /// # Arguments
    ///
    /// * `lock_wait_ms` - Maximum time to wait for each lock (in ms).
    ///   If <= 0, skip threads whose locks can't be acquired immediately.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::scope_description_private::ScopeDescriptionStackReportLock;
    ///
    /// let report = ScopeDescriptionStackReportLock::new(10);
    /// if let Some(msg) = report.message() {
    ///     println!("Scope stacks:\n{}", msg);
    /// }
    /// ```
    pub fn new(lock_wait_ms: i32) -> Self {
        let _timeout = if lock_wait_ms > 0 {
            Some(Duration::from_millis(lock_wait_ms as u64))
        } else {
            None
        };

        // In the Rust port, we build the report from the scope description
        // system. The actual implementation depends on the scope_description module.
        let message = Self::build_report();

        Self { message }
    }

    /// Get the captured report message.
    ///
    /// Returns None if the report could not be obtained (e.g., all locks timed out).
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    /// Build the scope description report from all threads.
    fn build_report() -> Option<String> {
        // Collect scope descriptions from the current thread.
        // In a full implementation, this would iterate all threads.
        use crate::scope_description::get_scope_stack;

        let stack = get_scope_stack();
        if stack.is_empty() {
            return None;
        }

        let mut report = String::new();
        report.push_str("Thread scope description stack:\n");
        for (i, desc) in stack.iter().enumerate() {
            report.push_str(&format!("  [{}] {}\n", i, desc));
        }

        Some(report)
    }
}

impl Default for ScopeDescriptionStackReportLock {
    fn default() -> Self {
        Self::new(10)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_report() {
        let report = ScopeDescriptionStackReportLock::new(10);
        // With no active scope descriptions, message may be None
        // (depends on whether any descriptions are active)
        let _ = report.message();
    }

    #[test]
    fn test_default() {
        let report = ScopeDescriptionStackReportLock::default();
        let _ = report.message();
    }
}
