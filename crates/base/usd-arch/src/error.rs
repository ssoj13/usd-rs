//! Low-level fatal error reporting and diagnostics.
//!
//! This module provides functions and macros for reporting fatal errors and warnings,
//! capturing stack traces, and aborting the program in a controlled manner.
//!
//! # Examples
//!
//! ```ignore
//! use usd_arch::{arch_error, arch_warning, arch_axiom};
//!
//! // Print warning
//! arch_warning!("Something unexpected happened");
//!
//! // Check invariant
//! let some_value = 10;
//! arch_axiom!(some_value > 0);
//!
//! // Fatal error (does not return)
//! arch_error!("Critical failure");
//! ```

use std::backtrace::{Backtrace, BacktraceStatus};
use std::sync::atomic::{AtomicBool, Ordering};

/// Global flag controlling whether fatal errors should log stack traces.
///
/// When enabled, fatal errors will capture and print a stack trace before aborting.
/// This can be controlled via [`arch_set_fatal_error_logging`].
static FATAL_ERROR_LOGGING_ENABLED: AtomicBool = AtomicBool::new(true);

/// Sets whether fatal errors should log stack traces.
///
/// When enabled (default), fatal errors will capture and print stack traces
/// before aborting the program. Disabling this can be useful in production
/// environments or when debugging with external tools.
///
/// # Arguments
///
/// * `enable` - Whether to enable fatal error logging
///
/// # Examples
///
/// ```
/// use usd_arch::arch_set_fatal_error_logging;
///
/// // Disable stack trace logging
/// arch_set_fatal_error_logging(false);
///
/// // Re-enable it
/// arch_set_fatal_error_logging(true);
/// ```
#[inline]
pub fn arch_set_fatal_error_logging(enable: bool) {
    FATAL_ERROR_LOGGING_ENABLED.store(enable, Ordering::Relaxed);
}

/// Returns whether fatal error logging is currently enabled.
///
/// # Examples
///
/// ```
/// use usd_arch::{arch_is_fatal_error_logging_enabled, arch_set_fatal_error_logging};
///
/// arch_set_fatal_error_logging(true);
/// assert!(arch_is_fatal_error_logging_enabled());
///
/// arch_set_fatal_error_logging(false);
/// assert!(!arch_is_fatal_error_logging_enabled());
/// ```
#[inline]
#[must_use]
pub fn arch_is_fatal_error_logging_enabled() -> bool {
    FATAL_ERROR_LOGGING_ENABLED.load(Ordering::Relaxed)
}

/// Prints error information to stderr.
///
/// This is the internal implementation used by the [`arch_error!`] macro.
///
/// # Arguments
///
/// * `msg` - Error message
/// * `func_name` - Name of the function where the error occurred
/// * `line` - Line number where the error occurred
/// * `file` - File name where the error occurred
#[doc(hidden)]
#[cold]
#[inline(never)]
pub fn arch_error_impl(msg: &str, func_name: &str, line: u32, file: &str) -> ! {
    eprintln!(" ArchError: {}", msg);
    eprintln!("  Function: {}", func_name);
    eprintln!("      File: {}", file);
    eprintln!("      Line: {}", line);

    if arch_is_fatal_error_logging_enabled() {
        let backtrace = Backtrace::capture();
        if backtrace.status() == BacktraceStatus::Captured {
            eprintln!("\nStack trace:");
            eprintln!("{}", backtrace);
        }
    }

    super::debugger::arch_abort(true);
}

/// Prints warning information to stderr.
///
/// This is the internal implementation used by the [`arch_warning!`] macro.
///
/// # Arguments
///
/// * `msg` - Warning message
/// * `func_name` - Name of the function where the warning occurred
/// * `line` - Line number where the warning occurred
/// * `file` - File name where the warning occurred
#[doc(hidden)]
#[cold]
#[inline(never)]
pub fn arch_warning_impl(msg: &str, func_name: &str, line: u32, file: &str) {
    eprintln!(" ArchWarn: {}", msg);
    eprintln!(" Function: {}", func_name);
    eprintln!("     File: {}", file);
    eprintln!("     Line: {}", line);
}

// Note: arch_abort is defined in debugger.rs with a logging parameter.
// Use `arch_abort(true)` for default behavior or `arch_abort(false)` to skip logging.

/// Logs a fatal error with a custom message and aborts the program.
///
/// This function captures and prints the current stack trace (if logging is enabled),
/// prints the error message, and then aborts the program.
///
/// # Arguments
///
/// * `msg` - The fatal error message
///
/// # Examples
///
/// ```no_run
/// use usd_arch::arch_log_fatal_error;
///
/// arch_log_fatal_error("Critical system failure");
/// ```
#[cold]
#[inline(never)]
pub fn arch_log_fatal_error(msg: &str) -> ! {
    eprintln!("FATAL ERROR: {}", msg);

    if arch_is_fatal_error_logging_enabled() {
        let backtrace = Backtrace::capture();
        if backtrace.status() == BacktraceStatus::Captured {
            eprintln!("\nStack trace:");
            eprintln!("{}", backtrace);
        }
    }

    super::debugger::arch_abort(true);
}

/// Logs the current process state for debugging purposes.
///
/// Unlike [`arch_log_fatal_error`], this function does not abort the program.
/// It's useful for logging diagnostic information during runtime without
/// terminating execution.
///
/// # Arguments
///
/// * `msg` - Contextual message describing why the state is being logged
///
/// # Examples
///
/// ```
/// use usd_arch::arch_log_current_process_state;
///
/// // Log state without aborting
/// arch_log_current_process_state("Checkpoint reached");
/// ```
#[cold]
#[inline(never)]
pub fn arch_log_current_process_state(msg: &str) {
    eprintln!("PROCESS STATE: {}", msg);

    if arch_is_fatal_error_logging_enabled() {
        let backtrace = Backtrace::capture();
        if backtrace.status() == BacktraceStatus::Captured {
            eprintln!("\nStack trace:");
            eprintln!("{}", backtrace);
        }
    }
}

/// Prints an error message and aborts the program.
///
/// This macro captures file, line, and function information automatically
/// and formats a detailed error message before aborting.
///
/// # Examples
///
/// ```no_run
/// # use usd_arch::arch_error;
/// arch_error!("Failed to initialize subsystem");
/// ```
#[macro_export]
macro_rules! arch_error {
    ($msg:expr) => {{ $crate::arch_error_impl($msg, $crate::function_name!(), line!(), file!()) }};
}

/// Prints a warning message without aborting.
///
/// This macro captures file, line, and function information automatically
/// and formats a detailed warning message.
///
/// # Examples
///
/// ```
/// # use usd_arch::arch_warning;
/// arch_warning!("Deprecated API used");
/// ```
#[macro_export]
macro_rules! arch_warning {
    ($msg:expr) => {{ $crate::arch_warning_impl($msg, $crate::function_name!(), line!(), file!()) }};
}

/// Asserts that a condition holds, aborting if it doesn't.
///
/// This is similar to `assert!` but uses the Arch error reporting infrastructure
/// to provide detailed diagnostic information.
///
/// # Examples
///
/// ```
/// # use usd_arch::arch_axiom;
/// let value = 42;
/// arch_axiom!(value > 0);
/// ```
///
/// Failing axiom (cannot be tested with #[should_panic] as it calls abort):
/// ```no_run
/// # use usd_arch::arch_axiom;
/// let value = -1;
/// arch_axiom!(value > 0); // This will abort
/// ```
#[macro_export]
macro_rules! arch_axiom {
    ($cond:expr) => {{
        if !($cond) {
            $crate::arch_error_impl(
                concat!("[", stringify!($cond), "] axiom failed"),
                $crate::function_name!(),
                line!(),
                file!(),
            )
        }
    }};
}

/// Helper macro to get the current function name.
///
/// This is used internally by error reporting macros.
#[doc(hidden)]
#[macro_export]
macro_rules! function_name {
    () => {{
        fn f() {}
        fn type_name_of<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }
        let name = type_name_of(f);
        // Strip the trailing "::f" from the name
        &name[..name.len() - 3]
    }};
}

// Re-export macros used via crate::error:: path
pub use crate::{arch_error, arch_warning};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fatal_error_logging_flag() {
        // Test default state
        let initial_state = arch_is_fatal_error_logging_enabled();

        // Test enabling
        arch_set_fatal_error_logging(true);
        assert!(arch_is_fatal_error_logging_enabled());

        // Test disabling
        arch_set_fatal_error_logging(false);
        assert!(!arch_is_fatal_error_logging_enabled());

        // Restore initial state
        arch_set_fatal_error_logging(initial_state);
    }

    #[test]
    fn test_warning_does_not_panic() {
        // Warnings should not cause panic
        arch_warning!("This is a test warning");
    }

    #[test]
    fn test_log_current_process_state_does_not_panic() {
        // Logging process state should not cause panic
        arch_log_current_process_state("Test checkpoint");
    }

    #[test]
    fn test_arch_axiom_success() {
        // This should not panic
        arch_axiom!(true);
        arch_axiom!(1 + 1 == 2);
        arch_axiom!(42 > 0);
    }

    // Note: Cannot test arch_axiom!(false) or arch_error!() in unit tests
    // because they call std::process::abort() which cannot be caught by
    // #[should_panic]. These need to be tested in integration tests or
    // manually verified.

    #[test]
    fn test_backtrace_capture() {
        // Test that we can capture backtraces
        arch_set_fatal_error_logging(true);

        let backtrace = Backtrace::capture();
        // On some platforms or configurations, backtraces might not be available
        // So we just check that the capture doesn't crash
        let _ = backtrace.status();
    }

    #[test]
    fn test_function_name_macro() {
        fn test_func() -> &'static str {
            function_name!()
        }

        let name = test_func();
        assert!(
            name.contains("test_func"),
            "Function name should contain 'test_func', got: {}",
            name
        );
    }
}
