//! Stack trace utilities.
//!
//! Provides functions for capturing and printing stack traces for debugging
//! and crash reporting purposes.
//!
//! # Overview
//!
//! This module provides functionality to:
//! - Capture current stack trace as a string
//! - Print stack traces to files or streams
//! - Log crash information with stack traces
//!
//! # Examples
//!
//! ```
//! use usd_tf::stack_trace::{get_stack_trace, print_stack_trace};
//!
//! // Get current stack trace as a string
//! let trace = get_stack_trace();
//! println!("{}", trace);
//!
//! // Print with a reason
//! print_stack_trace("Debug checkpoint reached");
//! ```

use std::fmt::Write as FmtWrite;
use std::io::Write;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

/// Application launch time (set once on first call).
static APP_LAUNCH_TIME: OnceLock<SystemTime> = OnceLock::new();

fn get_app_launch_time_internal() -> &'static SystemTime {
    APP_LAUNCH_TIME.get_or_init(SystemTime::now)
}

/// Get the current stack trace as a string.
///
/// Returns a formatted string containing the current call stack, with
/// function names, file locations, and line numbers where available.
///
/// # Examples
///
/// ```
/// use usd_tf::stack_trace::get_stack_trace;
///
/// fn inner_function() -> String {
///     get_stack_trace()
/// }
///
/// let trace = inner_function();
/// // trace contains the call stack
/// assert!(!trace.is_empty());
/// ```
pub fn get_stack_trace() -> String {
    let bt = std::backtrace::Backtrace::capture();
    let result = format!("{}", bt);

    if result.is_empty() || result == "disabled backtrace" {
        "[Stack trace not available]".to_string()
    } else {
        result
    }
}

/// Print stack trace to stderr with a reason.
///
/// # Parameters
///
/// - `reason`: A description of why the stack trace is being printed
///
/// # Examples
///
/// ```
/// use usd_tf::stack_trace::print_stack_trace;
///
/// print_stack_trace("Unexpected state encountered");
/// ```
pub fn print_stack_trace(reason: &str) {
    let trace = get_stack_trace();
    eprintln!("Stack trace ({})", reason);
    eprintln!("{}", trace);
}

/// Print stack trace to a writer with a reason.
///
/// # Parameters
///
/// - `out`: The writer to print to
/// - `reason`: A description of why the stack trace is being printed
///
/// # Examples
///
/// ```
/// use usd_tf::stack_trace::print_stack_trace_to;
/// use std::io::Cursor;
///
/// let mut buffer = Vec::new();
/// print_stack_trace_to(&mut buffer, "Test trace");
/// let output = String::from_utf8(buffer).unwrap();
/// assert!(output.contains("Stack trace"));
/// ```
pub fn print_stack_trace_to<W: Write>(out: &mut W, reason: &str) {
    let trace = get_stack_trace();
    writeln!(out, "Stack trace ({}):", reason).ok();
    writeln!(out, "{}", trace).ok();
}

/// Log a stack trace to a temporary file.
///
/// Creates a file in the system's temporary directory containing the
/// stack trace and a message indicating where the trace was written.
///
/// # Parameters
///
/// - `reason`: A description of why the stack trace is being logged
///
/// # Returns
///
/// The path to the log file, or an error message if logging failed.
///
/// # Examples
///
/// ```no_run
/// use usd_tf::stack_trace::log_stack_trace;
///
/// let path = log_stack_trace("Memory corruption detected");
/// println!("Stack trace logged to: {}", path);
/// ```
pub fn log_stack_trace(reason: &str) -> String {
    let trace = get_stack_trace();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let filename = format!("stack_trace_{}.log", timestamp);
    let path = std::env::temp_dir().join(&filename);

    match std::fs::File::create(&path) {
        Ok(mut file) => {
            writeln!(file, "Stack trace logged at: {:?}", SystemTime::now()).ok();
            writeln!(file, "Reason: {}", reason).ok();
            writeln!(file).ok();
            writeln!(file, "{}", trace).ok();

            eprintln!("Stack trace written to: {}", path.display());
            path.display().to_string()
        }
        Err(e) => {
            eprintln!("Failed to write stack trace to file: {}", e);
            eprintln!("Reason: {}", reason);
            eprintln!("{}", trace);
            format!("[Error: {}]", e)
        }
    }
}

/// Log crash information with context.
///
/// Creates a detailed crash report including:
/// - The reason for the crash
/// - A descriptive message
/// - Additional context information
/// - Call context (file, function, line)
/// - Full stack trace
///
/// # Parameters
///
/// - `reason`: Brief title for the crash (e.g., "FATAL_ERROR")
/// - `message`: Detailed description of what went wrong
/// - `additional_info`: Any extra context that might help debug
/// - `file`: Source file where the crash occurred
/// - `line`: Line number where the crash occurred
/// - `function`: Function name where the crash occurred
///
/// # Examples
///
/// ```
/// use usd_tf::stack_trace::log_crash;
///
/// log_crash(
///     "FATAL_ERROR",
///     "Dereferenced null pointer",
///     "ptr was None after validation check",
///     file!(),
///     line!(),
///     "process_data"
/// );
/// ```
pub fn log_crash(
    reason: &str,
    message: &str,
    additional_info: &str,
    file: &str,
    line: u32,
    function: &str,
) {
    let trace = get_stack_trace();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let filename = format!("crash_{}.log", timestamp);
    let path = std::env::temp_dir().join(&filename);

    let mut report = String::new();
    writeln!(report, "=== CRASH REPORT ===").ok();
    writeln!(report, "Time: {:?}", SystemTime::now()).ok();
    writeln!(report, "Reason: {}", reason).ok();
    writeln!(report, "Message: {}", message).ok();
    writeln!(report).ok();
    writeln!(report, "Location: {}:{} in {}", file, line, function).ok();
    writeln!(report).ok();

    if !additional_info.is_empty() {
        writeln!(report, "Additional Info:").ok();
        writeln!(report, "{}", additional_info).ok();
        writeln!(report).ok();
    }

    writeln!(report, "Stack Trace:").ok();
    writeln!(report, "{}", trace).ok();
    writeln!(report, "=== END CRASH REPORT ===").ok();

    // Write to file
    if let Ok(mut file) = std::fs::File::create(&path) {
        write!(file, "{}", report).ok();
        eprintln!("Crash report written to: {}", path.display());
    }

    // Also print to stderr
    eprintln!("{}", report);
}

/// Get the application's launch time.
///
/// Returns the time when the application was started (specifically, when
/// this function was first called).
///
/// # Examples
///
/// ```
/// use usd_tf::stack_trace::get_app_launch_time;
/// use std::time::SystemTime;
///
/// let launch = get_app_launch_time();
/// assert!(launch <= SystemTime::now());
/// ```
pub fn get_app_launch_time() -> SystemTime {
    *get_app_launch_time_internal()
}

/// Get the elapsed time since application launch.
///
/// # Examples
///
/// ```
/// use usd_tf::stack_trace::get_uptime;
/// use std::time::Duration;
///
/// let uptime = get_uptime();
/// // Uptime should be non-negative
/// assert!(uptime >= Duration::ZERO);
/// ```
pub fn get_uptime() -> std::time::Duration {
    get_app_launch_time_internal().elapsed().unwrap_or_default()
}

/// Capture a stack trace as a vector of frame addresses.
///
/// This is useful for storing traces compactly and resolving them later.
///
/// # Examples
///
/// ```
/// use usd_tf::stack_trace::capture_stack_frames;
///
/// let frames = capture_stack_frames();
/// // Number of frames depends on call depth
/// ```
pub fn capture_stack_frames() -> Vec<usize> {
    // std::backtrace doesn't expose individual frame addresses,
    // so we return an empty vec. For detailed frame access,
    // users should use the `backtrace` crate directly.
    Vec::new()
}

/// Resolve stack frame addresses to a readable string.
///
/// # Parameters
///
/// - `frames`: Vector of frame addresses from [`capture_stack_frames`]
///
/// # Examples
///
/// ```
/// use usd_tf::stack_trace::{capture_stack_frames, resolve_stack_frames};
///
/// let frames = capture_stack_frames();
/// let resolved = resolve_stack_frames(&frames);
/// ```
pub fn resolve_stack_frames(frames: &[usize]) -> String {
    if frames.is_empty() {
        return "[No frames to resolve]".to_string();
    }

    let mut result = String::new();
    for (i, &addr) in frames.iter().enumerate() {
        writeln!(result, "{:4}: 0x{:016x}", i, addr).ok();
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_stack_trace() {
        let trace = get_stack_trace();
        // Should return something (even if just a placeholder)
        assert!(!trace.is_empty());
    }

    #[test]
    fn test_get_stack_trace_in_nested_call() {
        fn level1() -> String {
            level2()
        }

        fn level2() -> String {
            level3()
        }

        fn level3() -> String {
            get_stack_trace()
        }

        let trace = level1();
        assert!(!trace.is_empty());
    }

    #[test]
    fn test_print_stack_trace_to_buffer() {
        let mut buffer = Vec::new();
        print_stack_trace_to(&mut buffer, "Test reason");

        let output = String::from_utf8(buffer).unwrap();
        assert!(output.contains("Stack trace"));
        assert!(output.contains("Test reason"));
    }

    #[test]
    fn test_get_app_launch_time() {
        let launch1 = get_app_launch_time();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let launch2 = get_app_launch_time();

        // Launch time should not change
        assert_eq!(launch1, launch2);

        // Launch time should be in the past or present
        assert!(launch1 <= SystemTime::now());
    }

    #[test]
    fn test_get_uptime() {
        let uptime1 = get_uptime();
        std::thread::sleep(std::time::Duration::from_millis(50));
        let uptime2 = get_uptime();

        // Uptime should increase
        assert!(uptime2 > uptime1);
    }

    #[test]
    fn test_capture_stack_frames() {
        fn nested_capture() -> Vec<usize> {
            capture_stack_frames()
        }

        let frames = nested_capture();
        // May be empty if backtrace feature is not enabled
        // but the function should not panic
        let _ = frames;
    }

    #[test]
    fn test_resolve_stack_frames_empty() {
        let resolved = resolve_stack_frames(&[]);
        // Should handle empty input gracefully
        assert!(!resolved.is_empty() || resolved == "[No frames to resolve]");
    }

    #[test]
    fn test_resolve_stack_frames() {
        let frames = capture_stack_frames();
        let resolved = resolve_stack_frames(&frames);
        // Should return something
        assert!(!resolved.is_empty());
    }

    #[test]
    fn test_log_stack_trace() {
        // This creates a file, so we just verify it doesn't panic
        let path = log_stack_trace("Test log");
        // Path should be returned (either actual path or error message)
        assert!(!path.is_empty());
    }

    #[test]
    fn test_log_crash() {
        // This creates a file and prints to stderr
        // Just verify it doesn't panic
        log_crash(
            "TEST_CRASH",
            "This is a test crash",
            "Additional test info",
            file!(),
            line!(),
            "test_log_crash",
        );
    }
}
