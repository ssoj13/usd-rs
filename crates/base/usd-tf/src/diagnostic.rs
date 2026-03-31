//! Diagnostic utilities for errors, warnings, and status messages.
//!
//! This module provides a diagnostic system similar to OpenUSD's TF diagnostics,
//! offering macros for issuing errors, warnings, and status messages with
//! source location tracking.
//!
//! # Diagnostic Types
//!
//! - **Errors**: Indicate something went wrong. Use `tf_error!` or `tf_coding_error!`.
//! - **Warnings**: Indicate potential issues. Use `tf_warn!`.
//! - **Status**: Informational messages. Use `tf_status!`.
//! - **Fatal**: Unrecoverable errors that terminate. Use `tf_fatal_error!`.
//!
//! # Examples
//!
//! ```
//! use usd_tf::{tf_warn, tf_status, tf_coding_error};
//!
//! // Issue a warning
//! tf_warn!("Something might be wrong");
//!
//! // Issue a status message
//! tf_status!("Processing started");
//!
//! // Issue a coding error (continues execution)
//! tf_coding_error!("Invalid parameter value: {}", 42);
//! ```
//!
//! # Verification
//!
//! Use `tf_verify!` for runtime assertions that can recover:
//!
//! ```
//! use usd_tf::tf_verify;
//!
//! let value = 5;
//! if tf_verify!(value > 0, "Value must be positive, got {}", value) {
//!     // Proceed with valid value
//! }
//! ```
//!
//! # Thread Safety
//!
//! All diagnostic functions are thread-safe.

use std::fmt;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};

use super::CallContext;

/// Diagnostic type enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum DiagnosticType {
    /// Invalid/unset diagnostic type.
    #[default]
    Invalid = 0,
    /// Coding error (programmer mistake).
    CodingError,
    /// Fatal coding error (terminates program).
    FatalCodingError,
    /// Runtime error (external failure).
    RuntimeError,
    /// Fatal error (terminates program).
    FatalError,
    /// Non-fatal error (warning-level).
    NonfatalError,
    /// Warning message.
    Warning,
    /// Status/informational message.
    Status,
    /// Application exit request.
    ApplicationExit,
}

impl DiagnosticType {
    /// Returns true if this is a fatal diagnostic type.
    #[inline]
    #[must_use]
    pub const fn is_fatal(&self) -> bool {
        matches!(
            self,
            DiagnosticType::FatalCodingError
                | DiagnosticType::FatalError
                | DiagnosticType::ApplicationExit
        )
    }

    /// Returns true if this is a coding error type.
    #[inline]
    #[must_use]
    pub const fn is_coding_error(&self) -> bool {
        matches!(
            self,
            DiagnosticType::CodingError | DiagnosticType::FatalCodingError
        )
    }

    /// Returns true if this is an error type (not warning or status).
    #[inline]
    #[must_use]
    pub const fn is_error(&self) -> bool {
        matches!(
            self,
            DiagnosticType::CodingError
                | DiagnosticType::FatalCodingError
                | DiagnosticType::RuntimeError
                | DiagnosticType::FatalError
                | DiagnosticType::NonfatalError
        )
    }

    /// Returns the string representation for display.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            DiagnosticType::Invalid => "INVALID",
            DiagnosticType::CodingError => "CODING ERROR",
            DiagnosticType::FatalCodingError => "FATAL CODING ERROR",
            DiagnosticType::RuntimeError => "RUNTIME ERROR",
            DiagnosticType::FatalError => "FATAL ERROR",
            DiagnosticType::NonfatalError => "ERROR",
            DiagnosticType::Warning => "WARNING",
            DiagnosticType::Status => "STATUS",
            DiagnosticType::ApplicationExit => "APPLICATION EXIT",
        }
    }
}

impl fmt::Display for DiagnosticType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A diagnostic message with context.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// The type of diagnostic.
    pub diagnostic_type: DiagnosticType,
    /// Source location where the diagnostic was issued.
    pub context: CallContext,
    /// The diagnostic message.
    pub message: String,
    /// Whether this diagnostic was issued quietly.
    pub quiet: bool,
}

impl Diagnostic {
    /// Create a new diagnostic.
    #[must_use]
    pub fn new(
        diagnostic_type: DiagnosticType,
        context: CallContext,
        message: impl Into<String>,
    ) -> Self {
        Self {
            diagnostic_type,
            context,
            message: message.into(),
            quiet: false,
        }
    }

    /// Create a quiet diagnostic (won't print immediately).
    #[must_use]
    pub fn quiet(
        diagnostic_type: DiagnosticType,
        context: CallContext,
        message: impl Into<String>,
    ) -> Self {
        Self {
            diagnostic_type,
            context,
            message: message.into(),
            quiet: true,
        }
    }

    /// Returns true if this is a fatal diagnostic.
    #[inline]
    #[must_use]
    pub fn is_fatal(&self) -> bool {
        self.diagnostic_type.is_fatal()
    }

    /// Returns true if this is a coding error.
    #[inline]
    #[must_use]
    pub fn is_coding_error(&self) -> bool {
        self.diagnostic_type.is_coding_error()
    }

    /// Append to the diagnostic message.
    pub fn augment(&mut self, additional: &str) {
        if !self.message.is_empty() {
            self.message.push('\n');
        }
        self.message.push_str(additional);
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {} -- {}",
            self.diagnostic_type, self.message, self.context
        )
    }
}

/// Global flag to control whether TF_VERIFY failures are fatal.
static FATAL_VERIFY: AtomicBool = AtomicBool::new(false);

/// Set whether TF_VERIFY failures should be fatal.
///
/// By default, `tf_verify!` failures issue coding errors but continue.
/// Setting this to true makes them fatal (like `tf_axiom!`).
pub fn set_fatal_verify(fatal: bool) {
    FATAL_VERIFY.store(fatal, Ordering::Relaxed);
}

/// Returns whether TF_VERIFY failures are fatal.
#[must_use]
pub fn is_fatal_verify() -> bool {
    FATAL_VERIFY.load(Ordering::Relaxed)
}

/// Issue a diagnostic message.
///
/// This is the core function called by the diagnostic macros.
/// It formats and outputs the diagnostic to stderr.
pub fn issue_diagnostic(diagnostic: &Diagnostic) {
    if diagnostic.quiet {
        return;
    }

    let prefix = match diagnostic.diagnostic_type {
        DiagnosticType::Status => "",
        _ => "Error: ",
    };

    let output = if diagnostic.context.is_hidden() || !diagnostic.context.is_valid() {
        format!(
            "{}{}: {}",
            prefix, diagnostic.diagnostic_type, diagnostic.message
        )
    } else {
        format!(
            "{}{}: {} -- {}:{}",
            prefix,
            diagnostic.diagnostic_type,
            diagnostic.message,
            diagnostic.context.file(),
            diagnostic.context.line()
        )
    };

    // Write to stderr
    let _ = writeln!(std::io::stderr(), "{}", output);
}

/// Issue an error diagnostic.
///
/// This function routes through the DiagnosticMgr, which adds the error
/// to the thread-local error list and notifies delegates.
#[inline]
pub fn issue_error(context: CallContext, diagnostic_type: DiagnosticType, message: String) {
    let diag = Diagnostic::new(diagnostic_type, context, message);
    super::diagnostic_mgr::DiagnosticMgr::instance().post_error(diag);
}

/// Issue a warning diagnostic.
#[inline]
pub fn issue_warning(context: CallContext, message: String) {
    let diag = Diagnostic::new(DiagnosticType::Warning, context, message);
    super::diagnostic_mgr::DiagnosticMgr::instance().post_warning(diag);
}

/// Issue a status diagnostic.
#[inline]
pub fn issue_status(context: CallContext, message: String) {
    let diag = Diagnostic::new(DiagnosticType::Status, context, message);
    super::diagnostic_mgr::DiagnosticMgr::instance().post_status(diag);
}

/// Issue a fatal error and terminate the program.
#[inline]
pub fn issue_fatal_error(context: CallContext, message: String) -> ! {
    super::diagnostic_mgr::DiagnosticMgr::instance().post_fatal(
        context,
        DiagnosticType::FatalError,
        message,
    );
}

/// Handle a failed verify condition.
///
/// Returns false to allow conditional handling.
pub fn failed_verify(context: CallContext, condition: &str, message: Option<&str>) -> bool {
    let msg = match message {
        Some(m) if !m.is_empty() => format!("Failed verification: '{}' -- {}", condition, m),
        _ => format!("Failed verification: '{}'", condition),
    };

    if is_fatal_verify() {
        issue_fatal_error(context, msg);
    } else {
        issue_error(context, DiagnosticType::CodingError, msg);
    }
    false
}

/// Handle a failed axiom condition.
///
/// This always terminates the program.
pub fn failed_axiom(context: CallContext, condition: &str) -> ! {
    let msg = format!("Failed axiom: '{}'", condition);
    issue_fatal_error(context, msg);
}

/// Sets program name for reporting errors.
///
/// This function simply calls to `arch_set_program_name_for_errors()`.
pub fn set_program_name_for_errors(program_name: &str) {
    usd_arch::arch_set_program_name_for_errors(program_name);
}

/// Returns currently set program name for errors.
///
/// This function simply calls to `arch_get_program_name_for_errors()`.
#[must_use]
pub fn get_program_name_for_errors() -> String {
    usd_arch::arch_get_program_name_for_errors()
}

/// (Re)install Tf's crash handler.
///
/// This should not generally need to be called since Tf does this itself when loaded.
/// However, when run in 3rd party environments that install their own signal handlers,
/// possibly overriding Tf's, this provides a way to reinstall them.
///
/// This calls std::panic::set_hook() and installs signal handlers for SIGSEGV,
/// SIGBUS, SIGFPE, and SIGABRT.
pub fn install_terminate_and_crash_handlers() {
    // Install panic hook for stack traces
    std::panic::set_hook(Box::new(|panic_info| {
        eprintln!("Fatal error: {:?}", panic_info);
        if let Some(location) = panic_info.location() {
            eprintln!(
                "Location: {}:{}:{}",
                location.file(),
                location.line(),
                location.column()
            );
        }
        eprintln!("Stack trace:");
        usd_arch::arch_print_stack_trace_stderr();
    }));

    // Note: Signal handlers would need to be set up here if we had signal support
    // For now, Rust's panic system handles most cases
}

// ============================================================================
// Macros
// ============================================================================

/// Issue a warning message.
///
/// # Examples
///
/// ```
/// use usd_tf::tf_warn;
///
/// tf_warn!("Simple warning");
/// tf_warn!("Warning with value: {}", 42);
/// ```
#[macro_export]
macro_rules! tf_warn {
    ($($arg:tt)*) => {{
        $crate::issue_warning(
            $crate::call_context!(),
            format!($($arg)*),
        );
    }};
}

/// Issue a status message.
///
/// # Examples
///
/// ```
/// use usd_tf::tf_status;
///
/// tf_status!("Processing started");
/// tf_status!("Processed {} items", 100);
/// ```
#[macro_export]
macro_rules! tf_status {
    ($($arg:tt)*) => {{
        $crate::issue_status(
            $crate::call_context!(),
            format!($($arg)*),
        );
    }};
}

/// Issue a coding error (programmer mistake).
///
/// This indicates a bug in the code but continues execution.
///
/// # Examples
///
/// ```
/// use usd_tf::tf_coding_error;
///
/// tf_coding_error!("Invalid state");
/// tf_coding_error!("Index {} out of bounds", 5);
/// ```
#[macro_export]
macro_rules! tf_coding_error {
    ($($arg:tt)*) => {{
        $crate::issue_error(
            $crate::call_context!(),
            $crate::DiagnosticType::CodingError,
            format!($($arg)*),
        );
    }};
}

/// Issue a runtime error (external failure).
///
/// This indicates a failure from external conditions (file not found, etc.).
///
/// # Examples
///
/// ```
/// use usd_tf::tf_runtime_error;
///
/// tf_runtime_error!("File not found");
/// tf_runtime_error!("Failed to connect to {}", "server");
/// ```
#[macro_export]
macro_rules! tf_runtime_error {
    ($($arg:tt)*) => {{
        $crate::issue_error(
            $crate::call_context!(),
            $crate::DiagnosticType::RuntimeError,
            format!($($arg)*),
        );
    }};
}

/// Issue a generic error.
///
/// Supports an optional error code (DiagnosticType) as first argument
/// using the `@code` prefix:
///
/// # Examples
///
/// ```
/// use usd_tf::{tf_error, DiagnosticType};
///
/// // Without error code (defaults to NonfatalError)
/// tf_error!("Something went wrong");
///
/// // With explicit error code using @code prefix
/// tf_error!(@code DiagnosticType::RuntimeError, "Failed to open file: {}", "foo.usda");
/// ```
#[macro_export]
macro_rules! tf_error {
    // With explicit error code using @code prefix for disambiguation
    (@code $code:expr, $($arg:tt)*) => {{
        $crate::issue_error(
            $crate::call_context!(),
            $code,
            format!($($arg)*),
        );
    }};
    // Without error code (default to NonfatalError)
    ($($arg:tt)*) => {{
        $crate::issue_error(
            $crate::call_context!(),
            $crate::DiagnosticType::NonfatalError,
            format!($($arg)*),
        );
    }};
}

/// Issue a fatal error and terminate the program.
///
/// # Examples
///
/// ```no_run
/// use usd_tf::tf_fatal_error;
///
/// tf_fatal_error!("Unrecoverable error");
/// // Program terminates here
/// ```
#[macro_export]
macro_rules! tf_fatal_error {
    ($($arg:tt)*) => {{
        $crate::issue_fatal_error(
            $crate::call_context!(),
            format!($($arg)*),
        );
    }};
}

/// Verify a condition, issuing a coding error if false.
///
/// Returns the boolean result of the condition, allowing conditional handling.
///
/// # Examples
///
/// ```
/// use usd_tf::tf_verify;
///
/// let x = 5;
/// if tf_verify!(x > 0) {
///     // x is positive
/// }
///
/// // With a message
/// tf_verify!(x < 10, "x must be less than 10, got {}", x);
/// ```
#[macro_export]
macro_rules! tf_verify {
    ($cond:expr) => {{
        let result = $cond;
        if !result {
            $crate::failed_verify(
                $crate::call_context!(),
                stringify!($cond),
                None,
            );
        }
        result
    }};
    ($cond:expr, $($arg:tt)*) => {{
        let result = $cond;
        if !result {
            let msg = format!($($arg)*);
            $crate::failed_verify(
                $crate::call_context!(),
                stringify!($cond),
                Some(&msg),
            );
        }
        result
    }};
}

/// Assert a condition, terminating the program if false.
///
/// Unlike `tf_verify!`, this always terminates on failure.
///
/// # Examples
///
/// ```
/// use usd_tf::tf_axiom;
///
/// let x = 5;
/// tf_axiom!(x > 0);
/// ```
#[macro_export]
macro_rules! tf_axiom {
    ($cond:expr) => {{
        if !$cond {
            $crate::failed_axiom($crate::call_context!(), stringify!($cond));
        }
    }};
}

/// Assert a condition only in debug builds.
///
/// In release builds, this is a no-op.
///
/// # Examples
///
/// ```
/// use usd_tf::tf_dev_axiom;
///
/// let x = 5;
/// tf_dev_axiom!(x > 0);
/// ```
#[macro_export]
macro_rules! tf_dev_axiom {
    ($cond:expr) => {{
        #[cfg(debug_assertions)]
        {
            if !$cond {
                $crate::failed_axiom($crate::call_context!(), stringify!($cond));
            }
        }
    }};
}

/// Get the name of the current function as a string.
///
/// This macro will return the name of the current function, nicely
/// formatted, as a string. This is meant primarily for diagnostics.
/// Code should not rely on a specific format, because it may change
/// in the future or vary across architectures.
///
/// # Examples
///
/// ```ignore
/// use usd_tf::tf_func_name;
///
/// fn my_function() {
///     println!("Debugging info about function {}", tf_func_name!());
/// }
/// ```
#[macro_export]
macro_rules! tf_func_name {
    () => {{
        usd_arch::function::arch_get_prettier_function_name(
            usd_arch::function_lite::arch_function!(),
            usd_arch::function_lite::arch_pretty_function!(),
        )
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnostic_type_is_fatal() {
        assert!(!DiagnosticType::Invalid.is_fatal());
        assert!(!DiagnosticType::CodingError.is_fatal());
        assert!(DiagnosticType::FatalCodingError.is_fatal());
        assert!(!DiagnosticType::RuntimeError.is_fatal());
        assert!(DiagnosticType::FatalError.is_fatal());
        assert!(!DiagnosticType::Warning.is_fatal());
        assert!(!DiagnosticType::Status.is_fatal());
    }

    #[test]
    fn test_diagnostic_type_is_coding_error() {
        assert!(!DiagnosticType::Invalid.is_coding_error());
        assert!(DiagnosticType::CodingError.is_coding_error());
        assert!(DiagnosticType::FatalCodingError.is_coding_error());
        assert!(!DiagnosticType::RuntimeError.is_coding_error());
        assert!(!DiagnosticType::Warning.is_coding_error());
    }

    #[test]
    fn test_diagnostic_type_is_error() {
        assert!(!DiagnosticType::Invalid.is_error());
        assert!(DiagnosticType::CodingError.is_error());
        assert!(DiagnosticType::FatalCodingError.is_error());
        assert!(DiagnosticType::RuntimeError.is_error());
        assert!(DiagnosticType::FatalError.is_error());
        assert!(DiagnosticType::NonfatalError.is_error());
        assert!(!DiagnosticType::Warning.is_error());
        assert!(!DiagnosticType::Status.is_error());
    }

    #[test]
    fn test_diagnostic_type_as_str() {
        assert_eq!(DiagnosticType::Warning.as_str(), "WARNING");
        assert_eq!(DiagnosticType::Status.as_str(), "STATUS");
        assert_eq!(DiagnosticType::CodingError.as_str(), "CODING ERROR");
    }

    #[test]
    fn test_diagnostic_type_display() {
        let s = format!("{}", DiagnosticType::Warning);
        assert_eq!(s, "WARNING");
    }

    #[test]
    fn test_diagnostic_new() {
        let ctx = CallContext::new("test.rs", "test_fn", 42);
        let diag = Diagnostic::new(DiagnosticType::Warning, ctx, "Test message");

        assert_eq!(diag.diagnostic_type, DiagnosticType::Warning);
        assert_eq!(diag.message, "Test message");
        assert!(!diag.quiet);
        assert!(!diag.is_fatal());
        assert!(!diag.is_coding_error());
    }

    #[test]
    fn test_diagnostic_quiet() {
        let ctx = CallContext::new("test.rs", "test_fn", 42);
        let diag = Diagnostic::quiet(DiagnosticType::Warning, ctx, "Quiet message");

        assert!(diag.quiet);
    }

    #[test]
    fn test_diagnostic_augment() {
        let ctx = CallContext::new("test.rs", "test_fn", 42);
        let mut diag = Diagnostic::new(DiagnosticType::Warning, ctx, "First");

        diag.augment("Second");
        assert_eq!(diag.message, "First\nSecond");

        diag.augment("Third");
        assert_eq!(diag.message, "First\nSecond\nThird");
    }

    #[test]
    fn test_diagnostic_display() {
        let ctx = CallContext::new("test.rs", "test_fn", 42);
        let diag = Diagnostic::new(DiagnosticType::Warning, ctx, "Test");

        let s = format!("{}", diag);
        assert!(s.contains("WARNING"));
        assert!(s.contains("Test"));
        assert!(s.contains("test.rs"));
    }

    #[test]
    fn test_fatal_verify_flag() {
        // Default should be false
        set_fatal_verify(false);
        assert!(!is_fatal_verify());

        set_fatal_verify(true);
        assert!(is_fatal_verify());

        // Reset for other tests
        set_fatal_verify(false);
    }

    #[test]
    fn test_tf_verify_success() {
        assert!(tf_verify!(true));
        assert!(tf_verify!(1 + 1 == 2));
        assert!(tf_verify!(1 < 2, "1 should be less than 2"));
    }

    #[test]
    fn test_tf_verify_failure() {
        // This should return false but not panic
        set_fatal_verify(false);
        assert!(!tf_verify!(false));
        assert!(!tf_verify!(1 > 2, "This should fail"));
    }

    #[test]
    fn test_tf_axiom_success() {
        tf_axiom!(true);
        tf_axiom!(1 + 1 == 2);
    }

    #[test]
    fn test_tf_dev_axiom_success() {
        tf_dev_axiom!(true);
        tf_dev_axiom!(1 + 1 == 2);
    }

    #[test]
    fn test_diagnostic_macros_compile() {
        // Just verify these compile and don't panic
        tf_warn!("Test warning");
        tf_warn!("Warning with arg: {}", 42);

        tf_status!("Test status");
        tf_status!("Status with arg: {}", "hello");

        tf_coding_error!("Test coding error");
        tf_coding_error!("Coding error with arg: {}", 123);

        tf_runtime_error!("Test runtime error");
        tf_runtime_error!("Runtime error with arg: {}", "test");

        tf_error!("Test error");
        tf_error!("Error with arg: {}", true);
    }

    #[test]
    fn test_diagnostic_type_default() {
        let dt: DiagnosticType = Default::default();
        assert_eq!(dt, DiagnosticType::Invalid);
    }

    #[test]
    fn test_tf_error_with_error_code() {
        // Test tf_error! macro with explicit error code via @code
        tf_error!(@code DiagnosticType::RuntimeError, "File not found: {}", "test.usd");
        tf_error!(@code DiagnosticType::CodingError, "Bad state");

        // Default (NonfatalError) still works
        tf_error!("Generic error: {}", 42);
    }
}
