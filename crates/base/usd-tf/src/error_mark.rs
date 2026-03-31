//! Error marks for tracking errors in a scope.
//!
//! `ErrorMark` provides RAII-style error tracking. When created, it records
//! the current position in the error list. You can then check if any new
//! errors were posted since the mark was created.
//!
//! # Examples
//!
//! ```ignore
//! use usd_tf::{ErrorMark, tf_error};
//!
//! let mark = ErrorMark::new();
//!
//! // Do something that might produce errors
//! tf_error!("Something went wrong");
//!
//! if !mark.is_clean() {
//!     // Handle errors
//!     for error in mark.errors() {
//!         println!("Error: {}", error.message());
//!     }
//!     mark.clear();
//! }
//! ```
//!
//! # Automatic Error Reporting
//!
//! If there are pending errors when the last `ErrorMark` on a thread is
//! dropped, those errors are automatically reported to stderr.

use super::diagnostic_mgr::DiagnosticMgr;
use super::error::TfError;
use super::error_transport::ErrorTransport;

/// RAII error mark for tracking errors in a scope.
///
/// When an `ErrorMark` is created, it records the current position in the
/// thread-local error list. The `is_clean()` method can be used to check
/// if any errors have been posted since the mark was set.
///
/// # Drop Behavior
///
/// When the last `ErrorMark` on a thread is dropped and there are pending
/// errors, those errors are automatically reported to stderr.
///
/// # Examples
///
/// ```
/// use usd_tf::{ErrorMark, tf_error};
///
/// fn do_something() -> bool {
///     let mark = ErrorMark::new();
///     
///     // ... operations that might error ...
///     tf_error!("Oops");
///     
///     mark.is_clean()
/// }
///
/// assert!(!do_something()); // Returns false because errors occurred
/// ```
#[derive(Debug)]
pub struct ErrorMark {
    mark: usize,
}

impl ErrorMark {
    /// Create a new error mark at the current position.
    ///
    /// The mark records the current serial number so that `is_clean()`
    /// can detect any errors posted after this point.
    #[must_use]
    pub fn new() -> Self {
        let mgr = DiagnosticMgr::instance();
        mgr.create_error_mark();
        Self {
            mark: mgr.current_serial(),
        }
    }

    /// Reset the mark to the current position.
    ///
    /// After calling this, `is_clean()` will only detect errors posted
    /// after this call.
    pub fn set_mark(&mut self) {
        self.mark = DiagnosticMgr::instance().current_serial();
    }

    /// Check if no errors have been posted since the mark was set.
    ///
    /// Returns `true` if no errors have been posted since the mark was
    /// created (or since `set_mark()` was last called).
    ///
    /// # Performance
    ///
    /// This is a fast operation in the common case where no errors exist.
    /// It only needs to check the thread-local error list when errors
    /// might be present.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        DiagnosticMgr::instance().is_clean_since(self.mark)
    }

    /// Clear all errors posted since the mark was set.
    ///
    /// Returns `true` if any errors were cleared, `false` if there were
    /// no errors to clear.
    ///
    /// After calling this, the errors are considered "handled" and will
    /// not be reported when the mark is dropped.
    pub fn clear(&self) -> bool {
        DiagnosticMgr::instance().clear_errors_since(self.mark) > 0
    }

    /// Get all errors posted since the mark was set.
    ///
    /// Returns a vector of errors. The errors remain in the error list
    /// and will be reported if not cleared.
    #[must_use]
    pub fn errors(&self) -> Vec<TfError> {
        DiagnosticMgr::instance().errors_since(self.mark)
    }

    /// Get the number of errors posted since the mark was set.
    #[must_use]
    pub fn error_count(&self) -> usize {
        self.errors().len()
    }

    /// Get the mark value (serial number).
    ///
    /// This is primarily useful for debugging or advanced use cases.
    #[must_use]
    pub fn mark_value(&self) -> usize {
        self.mark
    }

    /// Transport errors since the mark to another thread.
    ///
    /// This extracts all errors posted since the mark was set and returns
    /// them in an `ErrorTransport` that can be moved to another thread.
    /// The errors are removed from this thread's error list.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::{ErrorMark, tf_error};
    /// use std::thread;
    ///
    /// let transport = thread::spawn(|| {
    ///     let mark = ErrorMark::new();
    ///     tf_error!("Error in child thread");
    ///     mark.transport()
    /// }).join().unwrap();
    ///
    /// // Post errors to parent thread
    /// transport.post();
    /// ```
    #[must_use]
    pub fn transport(&self) -> ErrorTransport {
        let errors = DiagnosticMgr::instance().extract_errors_since(self.mark);
        ErrorTransport::from_errors(errors)
    }

    /// Transport errors to a specific `ErrorTransport`.
    ///
    /// This is useful when you want to accumulate errors from multiple
    /// sources into a single transport.
    pub fn transport_to(&self, transport: &mut ErrorTransport) {
        let mut new_transport = self.transport();
        transport.swap(&mut new_transport);
    }
}

impl Default for ErrorMark {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ErrorMark {
    fn drop(&mut self) {
        let mgr = DiagnosticMgr::instance();
        let is_last = mgr.destroy_error_mark();

        // If this is the last mark and there are pending errors, report them
        if is_last && !self.is_clean() {
            mgr.report_pending_errors(self.mark);
        }
    }
}

/// Convenience macro to check if errors occurred during an expression.
///
/// This macro evaluates the expression and returns `true` if any errors
/// were posted during evaluation.
///
/// # Examples
///
/// ```
/// use usd_tf::{ErrorMark, tf_has_errors, tf_error};
///
/// let mut mark = ErrorMark::new();
///
/// if tf_has_errors!(mark, {
///     tf_error!("Something failed");
///     42 // expression result is discarded
/// }) {
///     println!("Errors occurred!");
///     mark.clear();
/// }
/// ```
#[macro_export]
macro_rules! tf_has_errors {
    ($marker:expr, $expr:expr) => {{
        $marker.set_mark();
        let _ = $expr;
        !$marker.is_clean()
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CallContext;
    use crate::diagnostic::{Diagnostic, DiagnosticType};

    fn post_test_error(msg: &str) {
        let mgr = DiagnosticMgr::instance();
        let ctx = CallContext::new("test.rs", "test", 1);
        let diag = Diagnostic::new(DiagnosticType::RuntimeError, ctx, msg);
        mgr.post_error(diag);
    }

    #[test]
    fn test_error_mark_new() {
        let _mark = ErrorMark::new();
        assert!(DiagnosticMgr::instance().has_active_error_mark());
    }

    #[test]
    fn test_error_mark_is_clean() {
        DiagnosticMgr::instance().set_quiet(true);

        let mark = ErrorMark::new();
        assert!(mark.is_clean());

        post_test_error("test error");
        assert!(!mark.is_clean());

        mark.clear();
        assert!(mark.is_clean());

        DiagnosticMgr::instance().set_quiet(false);
    }

    #[test]
    fn test_error_mark_clear() {
        DiagnosticMgr::instance().set_quiet(true);

        let mark = ErrorMark::new();

        // No errors to clear
        assert!(!mark.clear());

        post_test_error("test error 1");
        post_test_error("test error 2");

        // Should clear both errors
        assert!(mark.clear());
        assert!(mark.is_clean());

        DiagnosticMgr::instance().set_quiet(false);
    }

    #[test]
    fn test_error_mark_errors() {
        DiagnosticMgr::instance().set_quiet(true);

        let mark = ErrorMark::new();
        assert!(mark.errors().is_empty());

        post_test_error("error 1");
        post_test_error("error 2");

        let errors = mark.errors();
        assert_eq!(errors.len(), 2);
        assert_eq!(errors[0].commentary(), "error 1");
        assert_eq!(errors[1].commentary(), "error 2");

        mark.clear();
        DiagnosticMgr::instance().set_quiet(false);
    }

    #[test]
    fn test_error_mark_set_mark() {
        DiagnosticMgr::instance().set_quiet(true);

        let mut mark = ErrorMark::new();

        post_test_error("error before reset");
        assert!(!mark.is_clean());

        mark.set_mark();
        // Old errors are now before the mark
        assert!(mark.is_clean());

        post_test_error("error after reset");
        assert!(!mark.is_clean());

        // Clear only clears errors after the mark
        let errors = mark.errors();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].commentary(), "error after reset");

        // Clean up all errors
        DiagnosticMgr::instance().clear_all_errors();
        DiagnosticMgr::instance().set_quiet(false);
    }

    #[test]
    fn test_error_mark_nested() {
        DiagnosticMgr::instance().set_quiet(true);

        let outer = ErrorMark::new();
        assert!(outer.is_clean());

        {
            let inner = ErrorMark::new();
            post_test_error("inner error");

            assert!(!inner.is_clean());
            assert!(!outer.is_clean());

            inner.clear();
            assert!(inner.is_clean());
            assert!(outer.is_clean());
        }

        // After inner drops, outer should still be clean
        assert!(outer.is_clean());

        DiagnosticMgr::instance().set_quiet(false);
    }

    #[test]
    fn test_error_mark_default() {
        let mark: ErrorMark = Default::default();
        assert!(mark.is_clean());
    }

    #[test]
    fn test_error_mark_error_count() {
        DiagnosticMgr::instance().set_quiet(true);

        let mark = ErrorMark::new();
        assert_eq!(mark.error_count(), 0);

        post_test_error("error 1");
        assert_eq!(mark.error_count(), 1);

        post_test_error("error 2");
        assert_eq!(mark.error_count(), 2);

        mark.clear();
        DiagnosticMgr::instance().set_quiet(false);
    }

    #[test]
    fn test_tf_has_errors_macro() {
        DiagnosticMgr::instance().set_quiet(true);

        let mut mark = ErrorMark::new();

        // No errors
        let had_errors = tf_has_errors!(mark, { 1 + 1 });
        assert!(!had_errors);

        // With errors
        let had_errors = tf_has_errors!(mark, {
            post_test_error("macro error");
        });
        assert!(had_errors);

        mark.clear();
        DiagnosticMgr::instance().set_quiet(false);
    }

    #[test]
    fn test_error_mark_value() {
        let mark = ErrorMark::new();
        let value = mark.mark_value();
        assert!(value > 0);
    }
}
