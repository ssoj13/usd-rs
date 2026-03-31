//! Diagnostic Manager - singleton for managing errors and diagnostics.
//!
//! This module provides the central diagnostic manager that handles
//! thread-local error lists, delegates, and error mark tracking.
//!
//! # Thread Safety
//!
//! The diagnostic manager uses thread-local storage for error lists,
//! ensuring thread-safe error handling without lock contention in
//! the common case.
//!
//! # Examples
//!
//! ```
//! use usd_tf::{DiagnosticMgr, tf_error};
//!
//! // Get the singleton instance
//! let mgr = DiagnosticMgr::instance();
//!
//! // Post an error
//! tf_error!("Something went wrong");
//!
//! // Check for errors (usually done via ErrorMark)
//! assert!(!mgr.has_active_error_mark());
//! ```

use std::cell::{Cell, RefCell};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock, RwLock};

use super::CallContext;
use super::diagnostic::{Diagnostic, DiagnosticType, issue_diagnostic};
use super::error::TfError;

/// Thread-local error list type.
type ErrorList = Vec<TfError>;

/// Thread-local data for each thread.
#[derive(Default)]
struct ThreadLocalData {
    /// Error list for this thread.
    errors: ErrorList,
    /// Count of active error marks on this thread.
    error_mark_count: usize,
}

thread_local! {
    static THREAD_DATA: RefCell<ThreadLocalData> = RefCell::new(ThreadLocalData::default());
    /// Prevents infinite recursion when a delegate callback posts a diagnostic.
    static REENTRANT_GUARD: Cell<bool> = Cell::new(false);
}

/// Diagnostic delegate trait.
///
/// Implement this trait to receive callbacks when diagnostics are issued.
pub trait DiagnosticDelegate: Send + Sync {
    /// Called when an error is posted.
    fn issue_error(&self, error: &TfError);

    /// Called when a fatal error is posted.
    fn issue_fatal_error(&self, context: &CallContext, msg: &str);

    /// Called when a status message is posted.
    fn issue_status(&self, diagnostic: &Diagnostic);

    /// Called when a warning is posted.
    fn issue_warning(&self, diagnostic: &Diagnostic);
}

/// Singleton diagnostic manager.
///
/// The diagnostic manager is the central point for all error and diagnostic
/// handling. It maintains thread-local error lists and dispatches diagnostics
/// to registered delegates.
pub struct DiagnosticMgr {
    /// Global serial number counter.
    next_serial: AtomicUsize,
    /// Registered delegates.
    delegates: RwLock<Vec<Arc<dyn DiagnosticDelegate>>>,
    /// Whether to suppress output.
    quiet: AtomicUsize, // Using usize as bool for AtomicBool compatibility
}

impl DiagnosticMgr {
    /// Create a new diagnostic manager.
    fn new() -> Self {
        Self {
            next_serial: AtomicUsize::new(1),
            delegates: RwLock::new(Vec::new()),
            quiet: AtomicUsize::new(0),
        }
    }

    /// Get the singleton instance.
    #[must_use]
    pub fn instance() -> &'static Self {
        static INSTANCE: OnceLock<DiagnosticMgr> = OnceLock::new();
        INSTANCE.get_or_init(DiagnosticMgr::new)
    }

    /// Get the current serial number (for error marks).
    #[must_use]
    pub fn current_serial(&self) -> usize {
        self.next_serial.load(Ordering::Acquire)
    }

    /// Allocate a new serial number.
    fn alloc_serial(&self) -> usize {
        self.next_serial.fetch_add(1, Ordering::AcqRel)
    }

    /// Add a delegate.
    pub fn add_delegate(&self, delegate: Arc<dyn DiagnosticDelegate>) {
        let mut delegates = self.delegates.write().expect("delegates lock poisoned");
        delegates.push(delegate);
    }

    /// Remove a delegate.
    pub fn remove_delegate(&self, delegate: &Arc<dyn DiagnosticDelegate>) {
        let mut delegates = self.delegates.write().expect("delegates lock poisoned");
        delegates.retain(|d| !Arc::ptr_eq(d, delegate));
    }

    /// Set quiet mode (suppress output).
    pub fn set_quiet(&self, quiet: bool) {
        self.quiet
            .store(if quiet { 1 } else { 0 }, Ordering::Relaxed);
    }

    /// Check if quiet mode is enabled.
    #[must_use]
    pub fn is_quiet(&self) -> bool {
        self.quiet.load(Ordering::Relaxed) != 0
    }

    /// Post an error.
    ///
    /// The error is added to the thread-local error list. Delegates are only
    /// dispatched when there is no active ErrorMark, matching C++ AppendError
    /// behavior. Reentrancy from delegate callbacks is short-circuited.
    pub fn post_error(&self, diagnostic: Diagnostic) {
        // Guard against infinite recursion from delegate callbacks.
        if REENTRANT_GUARD.with(|g| g.get()) {
            issue_diagnostic(&diagnostic);
            return;
        }

        let serial = self.alloc_serial();
        let error = TfError::from_diagnostic(serial, diagnostic);

        // Always accumulate in the thread-local error list.
        THREAD_DATA.with(|data| {
            data.borrow_mut().errors.push(error.clone());
        });

        // Only dispatch to delegates (and stderr fallback) when there is no
        // active ErrorMark. With an active mark errors are silently held.
        if !self.has_active_error_mark() {
            let delegates = self.delegates.read().expect("delegates lock poisoned");
            REENTRANT_GUARD.with(|g| g.set(true));
            for delegate in delegates.iter() {
                delegate.issue_error(&error);
            }
            REENTRANT_GUARD.with(|g| g.set(false));

            if delegates.is_empty() && !self.is_quiet() {
                issue_diagnostic(&error.diagnostic);
            }
        }
    }

    /// Post a warning.
    ///
    /// Dispatches to delegates; falls back to stderr only when no delegates
    /// handled it and quiet mode is off. Reentrancy from delegate callbacks
    /// is short-circuited.
    pub fn post_warning(&self, diagnostic: Diagnostic) {
        // Guard against infinite recursion from delegate callbacks.
        if REENTRANT_GUARD.with(|g| g.get()) {
            issue_diagnostic(&diagnostic);
            return;
        }

        let delegates = self.delegates.read().expect("delegates lock poisoned");
        let dispatched = !delegates.is_empty();
        REENTRANT_GUARD.with(|g| g.set(true));
        for delegate in delegates.iter() {
            delegate.issue_warning(&diagnostic);
        }
        REENTRANT_GUARD.with(|g| g.set(false));
        drop(delegates);

        // Only fall back to stderr when no delegate handled it.
        if !dispatched && !self.is_quiet() {
            issue_diagnostic(&diagnostic);
        }
    }

    /// Post a status message.
    ///
    /// Dispatches to delegates; falls back to stderr only when no delegates
    /// handled it and quiet mode is off. Reentrancy from delegate callbacks
    /// is short-circuited.
    pub fn post_status(&self, diagnostic: Diagnostic) {
        // Guard against infinite recursion from delegate callbacks.
        if REENTRANT_GUARD.with(|g| g.get()) {
            issue_diagnostic(&diagnostic);
            return;
        }

        let delegates = self.delegates.read().expect("delegates lock poisoned");
        let dispatched = !delegates.is_empty();
        REENTRANT_GUARD.with(|g| g.set(true));
        for delegate in delegates.iter() {
            delegate.issue_status(&diagnostic);
        }
        REENTRANT_GUARD.with(|g| g.set(false));
        drop(delegates);

        // Only fall back to stderr when no delegate handled it.
        if !dispatched && !self.is_quiet() {
            issue_diagnostic(&diagnostic);
        }
    }

    /// Post a fatal error.
    ///
    /// This function does not return. Reentrancy from delegate callbacks
    /// is short-circuited to avoid infinite recursion before abort.
    ///
    /// Matches C++ PostFatal: RuntimeError exits cleanly via exit(1); coding
    /// errors and other fatal types abort via Tf_UnhandledAbort.
    pub fn post_fatal(
        &self,
        context: CallContext,
        diagnostic_type: DiagnosticType,
        msg: String,
    ) -> ! {
        if !REENTRANT_GUARD.with(|g| g.get()) {
            let delegates = self.delegates.read().expect("delegates lock poisoned");
            REENTRANT_GUARD.with(|g| g.set(true));
            for delegate in delegates.iter() {
                delegate.issue_fatal_error(&context, &msg);
            }
            REENTRANT_GUARD.with(|g| g.set(false));
        }

        // Always print fatal errors regardless of delegates or quiet mode.
        let diagnostic = Diagnostic::new(diagnostic_type, context, msg);
        issue_diagnostic(&diagnostic);
        match diagnostic_type {
            // C++: RUNTIME_ERROR branch calls exit(1) for a clean shutdown.
            DiagnosticType::RuntimeError => std::process::exit(1),
            // C++: CODING_ERROR and all other fatal types call Tf_UnhandledAbort.
            _ => std::process::abort(),
        }
    }

    /// Check if there are any active error marks on this thread.
    #[must_use]
    pub fn has_active_error_mark(&self) -> bool {
        THREAD_DATA.with(|data| data.borrow().error_mark_count > 0)
    }

    /// Create an error mark (called by ErrorMark constructor).
    pub(crate) fn create_error_mark(&self) {
        THREAD_DATA.with(|data| {
            data.borrow_mut().error_mark_count += 1;
        });
    }

    /// Destroy an error mark (called by ErrorMark destructor).
    ///
    /// Returns true if this was the last error mark on this thread.
    pub(crate) fn destroy_error_mark(&self) -> bool {
        THREAD_DATA.with(|data| {
            let mut data = data.borrow_mut();
            data.error_mark_count = data.error_mark_count.saturating_sub(1);
            data.error_mark_count == 0
        })
    }

    /// Get the number of errors in this thread's error list.
    #[must_use]
    pub fn error_count(&self) -> usize {
        THREAD_DATA.with(|data| data.borrow().errors.len())
    }

    /// Check if any errors exist with serial >= mark.
    #[must_use]
    pub fn is_clean_since(&self, mark: usize) -> bool {
        // Fast path: if our mark is >= current serial, no errors could exist
        if mark >= self.current_serial() {
            return true;
        }

        THREAD_DATA.with(|data| {
            let data = data.borrow();
            if data.errors.is_empty() {
                return true;
            }
            // Check if the last error has serial < mark
            data.errors.last().is_none_or(|e| e.serial < mark)
        })
    }

    /// Get errors with serial >= mark.
    #[must_use]
    pub fn errors_since(&self, mark: usize) -> Vec<TfError> {
        THREAD_DATA.with(|data| {
            let data = data.borrow();
            data.errors
                .iter()
                .filter(|e| e.serial >= mark)
                .cloned()
                .collect()
        })
    }

    /// Clear errors with serial >= mark.
    ///
    /// Returns the number of errors cleared.
    pub fn clear_errors_since(&self, mark: usize) -> usize {
        THREAD_DATA.with(|data| {
            let mut data = data.borrow_mut();
            let original_len = data.errors.len();
            data.errors.retain(|e| e.serial < mark);
            original_len - data.errors.len()
        })
    }

    /// Get all errors on this thread.
    #[must_use]
    pub fn all_errors(&self) -> Vec<TfError> {
        THREAD_DATA.with(|data| data.borrow().errors.clone())
    }

    /// Clear all errors on this thread.
    pub fn clear_all_errors(&self) {
        THREAD_DATA.with(|data| {
            data.borrow_mut().errors.clear();
        });
    }

    /// Returns the name of the given diagnostic code.
    #[must_use]
    pub fn get_code_name(code: DiagnosticType) -> &'static str {
        match code {
            DiagnosticType::Invalid => "Invalid",
            DiagnosticType::CodingError => "CodingError",
            DiagnosticType::FatalCodingError => "FatalCodingError",
            DiagnosticType::RuntimeError => "RuntimeError",
            DiagnosticType::FatalError => "FatalError",
            DiagnosticType::NonfatalError => "NonfatalError",
            DiagnosticType::Warning => "Warning",
            DiagnosticType::Status => "Status",
            DiagnosticType::ApplicationExit => "ApplicationExit",
        }
    }

    /// Format a diagnostic message in the standard format.
    #[must_use]
    pub fn format_diagnostic(diagnostic: &Diagnostic) -> String {
        let type_name = Self::get_code_name(diagnostic.diagnostic_type);
        format!(
            "{} [{}:{}] {}: {}",
            type_name,
            diagnostic.context.file(),
            diagnostic.context.line(),
            diagnostic.context.function(),
            diagnostic.message
        )
    }

    /// Append an error to the error list.
    ///
    /// This is used by systems that need to inject errors from external sources
    /// (e.g., Python exception translation).
    pub fn append_error(&self, error: TfError) {
        THREAD_DATA.with(|data| {
            data.borrow_mut().errors.push(error.clone());
        });

        // Dispatch to delegates
        let delegates = self.delegates.read().expect("delegates lock poisoned");
        for delegate in delegates.iter() {
            delegate.issue_error(&error);
        }
    }

    /// Erase a specific error by index.
    ///
    /// Returns true if an error was erased.
    pub fn erase_error(&self, index: usize) -> bool {
        THREAD_DATA.with(|data| {
            let mut data = data.borrow_mut();
            if index < data.errors.len() {
                data.errors.remove(index);
                true
            } else {
                false
            }
        })
    }

    /// Erase errors in a range [start, end).
    ///
    /// Returns the number of errors erased.
    pub fn erase_range(&self, start: usize, end: usize) -> usize {
        THREAD_DATA.with(|data| {
            let mut data = data.borrow_mut();
            let actual_end = end.min(data.errors.len());
            let actual_start = start.min(actual_end);
            let count = actual_end - actual_start;
            if count > 0 {
                data.errors.drain(actual_start..actual_end);
            }
            count
        })
    }

    /// Get number of registered delegates.
    #[must_use]
    pub fn delegate_count(&self) -> usize {
        self.delegates
            .read()
            .expect("delegates lock poisoned")
            .len()
    }

    /// Report pending errors (called when last ErrorMark is destroyed).
    pub(crate) fn report_pending_errors(&self, mark: usize) {
        let errors = self.errors_since(mark);
        for error in &errors {
            issue_diagnostic(&error.diagnostic);
        }
        self.clear_errors_since(mark);
    }

    /// Splice errors from another list into the current thread's error list.
    ///
    /// This is used by `ErrorTransport` to inject errors from another thread.
    /// Transported errors receive fresh serial numbers so ErrorMark ordering
    /// is correct on the receiving thread (matches C++ _SpliceErrors).
    pub(crate) fn splice_errors(&self, errors: Vec<TfError>) {
        if errors.is_empty() {
            return;
        }

        // Atomically reserve a contiguous block of serials for the incoming
        // batch, then stamp each error before appending.
        let base_serial = self.next_serial.fetch_add(errors.len(), Ordering::AcqRel);
        THREAD_DATA.with(|data| {
            let mut data = data.borrow_mut();
            for (i, mut error) in errors.into_iter().enumerate() {
                error.serial = base_serial + i;
                data.errors.push(error);
            }
        });
    }

    /// Extract errors since mark and return them, removing from the error list.
    ///
    /// This is used by `ErrorMark::transport()` to move errors out.
    pub(crate) fn extract_errors_since(&self, mark: usize) -> Vec<TfError> {
        THREAD_DATA.with(|data| {
            let mut data = data.borrow_mut();
            let drain_start = data.errors.iter().position(|e| e.serial >= mark);
            match drain_start {
                Some(idx) => data.errors.drain(idx..).collect(),
                None => Vec::new(),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_singleton() {
        let mgr1 = DiagnosticMgr::instance();
        let mgr2 = DiagnosticMgr::instance();
        assert!(std::ptr::eq(mgr1, mgr2));
    }

    #[test]
    fn test_serial_allocation() {
        let mgr = DiagnosticMgr::instance();
        let s1 = mgr.current_serial();
        let s2 = mgr.alloc_serial();
        let s3 = mgr.current_serial();
        assert_eq!(s1, s2);
        assert_eq!(s3, s2 + 1);
    }

    #[test]
    fn test_quiet_mode() {
        let mgr = DiagnosticMgr::instance();
        let was_quiet = mgr.is_quiet();

        mgr.set_quiet(true);
        assert!(mgr.is_quiet());

        mgr.set_quiet(false);
        assert!(!mgr.is_quiet());

        // Restore original state
        mgr.set_quiet(was_quiet);
    }

    #[test]
    fn test_error_mark_count() {
        let mgr = DiagnosticMgr::instance();

        // Initially no error marks
        // Note: Other tests might have marks, so just check the operations work
        mgr.create_error_mark();
        assert!(mgr.has_active_error_mark());

        let is_last = mgr.destroy_error_mark();
        // is_last depends on whether other marks exist
        let _ = is_last;
    }

    #[test]
    fn test_post_error() {
        let mgr = DiagnosticMgr::instance();
        mgr.set_quiet(true);

        let initial_count = mgr.error_count();
        let mark = mgr.current_serial();

        // Create an error mark to prevent immediate output
        mgr.create_error_mark();

        let ctx = CallContext::new("test.rs", "test_fn", 42);
        let diag = Diagnostic::new(DiagnosticType::RuntimeError, ctx, "Test error");
        mgr.post_error(diag);

        assert_eq!(mgr.error_count(), initial_count + 1);
        assert!(!mgr.is_clean_since(mark));

        // Clean up
        mgr.clear_errors_since(mark);
        mgr.destroy_error_mark();
        mgr.set_quiet(false);
    }

    #[test]
    fn test_errors_since() {
        let mgr = DiagnosticMgr::instance();
        mgr.set_quiet(true);
        mgr.create_error_mark();

        let mark = mgr.current_serial();

        let ctx = CallContext::new("test.rs", "test_fn", 42);
        let diag = Diagnostic::new(DiagnosticType::RuntimeError, ctx, "Error 1");
        mgr.post_error(diag);

        let ctx = CallContext::new("test.rs", "test_fn", 43);
        let diag = Diagnostic::new(DiagnosticType::RuntimeError, ctx, "Error 2");
        mgr.post_error(diag);

        let errors = mgr.errors_since(mark);
        assert_eq!(errors.len(), 2);

        // Clean up
        let cleared = mgr.clear_errors_since(mark);
        assert_eq!(cleared, 2);

        mgr.destroy_error_mark();
        mgr.set_quiet(false);
    }

    #[test]
    fn test_is_clean_since() {
        let mgr = DiagnosticMgr::instance();
        mgr.set_quiet(true);
        mgr.create_error_mark();

        let mark = mgr.current_serial();
        assert!(mgr.is_clean_since(mark));

        let ctx = CallContext::new("test.rs", "test_fn", 42);
        let diag = Diagnostic::new(DiagnosticType::RuntimeError, ctx, "Test");
        mgr.post_error(diag);

        assert!(!mgr.is_clean_since(mark));

        // Clean up
        mgr.clear_errors_since(mark);
        mgr.destroy_error_mark();
        mgr.set_quiet(false);
    }
}
