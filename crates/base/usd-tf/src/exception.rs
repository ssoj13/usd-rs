//! Exception handling utilities.
//!
//! Provides a base exception type with call context and stack trace capture.
//!
//! In Rust, we use the standard Error trait, but this module provides
//! additional functionality like capturing the throw point's call context
//! and stack trace.
//!
//! # Examples
//!
//! ```
//! use usd_tf::exception::TfException;
//! use usd_tf::call_context;
//!
//! let error = TfException::new("Something went wrong", call_context!());
//! assert!(error.message().contains("wrong"));
//! ```

use std::error::Error;
use std::fmt;

use crate::CallContext;

/// Number of stack frames to skip when capturing.
#[derive(Debug, Clone, Copy, Default)]
pub struct SkipCallerFrames {
    /// Number of frames to skip.
    pub num_to_skip: usize,
}

impl SkipCallerFrames {
    /// Create with explicit frame count to skip.
    pub fn new(n: usize) -> Self {
        Self { num_to_skip: n }
    }
}

impl From<usize> for SkipCallerFrames {
    fn from(n: usize) -> Self {
        Self::new(n)
    }
}

/// The base exception type for Tf exceptions.
///
/// Provides message, call context, and optional stack trace capture.
/// Implements std::error::Error for compatibility with Rust's error handling.
#[derive(Debug, Clone)]
pub struct TfException {
    /// Error message.
    message: String,
    /// Call context from throw point.
    call_context: CallContext,
    /// Stack frame addresses from throw point.
    throw_stack: Vec<usize>,
}

impl TfException {
    /// Create a new TfException with the given message and call context.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::exception::TfException;
    /// use usd_tf::call_context;
    ///
    /// let exc = TfException::new("error message", call_context!());
    /// assert_eq!(exc.message(), "error message");
    /// ```
    pub fn new(message: impl Into<String>, call_context: CallContext) -> Self {
        Self {
            message: message.into(),
            call_context,
            throw_stack: Vec::new(),
        }
    }

    /// Create a new TfException with stack capture.
    ///
    /// Note: Stack capture is a placeholder. Full stack capture would require
    /// the `backtrace` crate.
    ///
    /// # Arguments
    ///
    /// * `message` - Error message
    /// * `call_context` - Call context from throw point
    /// * `_skip_frames` - Number of additional frames to skip (currently unused)
    pub fn with_stack(
        message: impl Into<String>,
        call_context: CallContext,
        _skip_frames: SkipCallerFrames,
    ) -> Self {
        // Stack capture is a placeholder - full implementation would use backtrace crate
        Self::new(message, call_context)
    }

    /// Returns the error message.
    #[inline]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the call context from the throw point.
    #[inline]
    pub fn throw_context(&self) -> &CallContext {
        &self.call_context
    }

    /// Returns the stack frame addresses from the throw point.
    #[inline]
    pub fn throw_stack(&self) -> &[usize] {
        &self.throw_stack
    }

    /// Move the stack frame addresses out of this exception.
    pub fn take_throw_stack(&mut self) -> Vec<usize> {
        std::mem::take(&mut self.throw_stack)
    }

    /// Set the throw stack.
    pub fn set_throw_stack(&mut self, stack: Vec<usize>) {
        self.throw_stack = stack;
    }

    /// Check if TF_FATAL_THROW is enabled.
    ///
    /// When enabled, exceptions cause fatal errors instead of being thrown.
    pub fn is_fatal_throw_enabled() -> bool {
        std::env::var("TF_FATAL_THROW")
            .ok()
            .and_then(|v| v.parse::<bool>().ok())
            .unwrap_or(false)
    }
}

impl fmt::Display for TfException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)?;
        if !self.call_context.file().is_empty() {
            write!(
                f,
                " (at {}:{} in {})",
                self.call_context.file(),
                self.call_context.line(),
                self.call_context.function()
            )?;
        }
        Ok(())
    }
}

impl Error for TfException {}

/// Create a TfException at the current location.
///
/// # Examples
///
/// ```
/// use usd_tf::exception::TfException;
/// use usd_tf::tf_exception;
///
/// let exc = tf_exception!("error: {}", 42);
/// assert!(exc.message().contains("42"));
/// ```
#[macro_export]
macro_rules! tf_exception {
    ($msg:expr) => {
        $crate::exception::TfException::new($msg, $crate::call_context!())
    };
    ($fmt:expr, $($arg:tt)*) => {
        $crate::exception::TfException::new(
            format!($fmt, $($arg)*),
            $crate::call_context!()
        )
    };
}

/// Throw a TfException (panic in Rust).
///
/// If TF_FATAL_THROW is enabled, this will cause a fatal error.
/// Otherwise, it will panic with the exception message.
///
/// # Examples
///
/// ```should_panic
/// use usd_tf::tf_throw;
///
/// tf_throw!("Something went wrong!");
/// ```
#[macro_export]
macro_rules! tf_throw {
    ($msg:expr) => {{
        let exc = $crate::tf_exception!($msg);
        if $crate::exception::TfException::is_fatal_throw_enabled() {
            $crate::tf_fatal_error!("{}", exc);
        } else {
            panic!("{}", exc);
        }
    }};
    ($fmt:expr, $($arg:tt)*) => {{
        let exc = $crate::tf_exception!($fmt, $($arg)*);
        if $crate::exception::TfException::is_fatal_throw_enabled() {
            $crate::tf_fatal_error!("{}", exc);
        } else {
            panic!("{}", exc);
        }
    }};
}

/// Result type alias using TfException as the error type.
pub type TfResult<T> = Result<T, TfException>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::call_context;

    #[test]
    fn test_new() {
        let exc = TfException::new("test error", call_context!());
        assert_eq!(exc.message(), "test error");
        assert!(!exc.throw_context().file().is_empty());
    }

    #[test]
    fn test_display() {
        let exc = TfException::new("test error", call_context!());
        let display = format!("{}", exc);
        assert!(display.contains("test error"));
        assert!(display.contains("exception.rs"));
    }

    #[test]
    fn test_skip_caller_frames() {
        let skip = SkipCallerFrames::new(5);
        assert_eq!(skip.num_to_skip, 5);

        let skip: SkipCallerFrames = 3.into();
        assert_eq!(skip.num_to_skip, 3);

        let skip = SkipCallerFrames::default();
        assert_eq!(skip.num_to_skip, 0);
    }

    #[test]
    fn test_throw_stack() {
        let mut exc = TfException::new("test", call_context!());
        assert!(exc.throw_stack().is_empty());

        exc.set_throw_stack(vec![1, 2, 3]);
        assert_eq!(exc.throw_stack().len(), 3);

        let stack = exc.take_throw_stack();
        assert_eq!(stack, vec![1, 2, 3]);
        assert!(exc.throw_stack().is_empty());
    }

    #[test]
    fn test_tf_exception_macro() {
        let exc = tf_exception!("simple error");
        assert_eq!(exc.message(), "simple error");

        let exc = tf_exception!("formatted error: {}", 42);
        assert_eq!(exc.message(), "formatted error: 42");
    }

    #[test]
    fn test_is_error() {
        let exc = TfException::new("error", call_context!());
        let error: &dyn Error = &exc;
        assert!(error.to_string().contains("error"));
    }

    #[test]
    fn test_clone() {
        let exc1 = TfException::new("test", call_context!());
        let exc2 = exc1.clone();
        assert_eq!(exc1.message(), exc2.message());
    }

    #[test]
    fn test_debug() {
        let exc = TfException::new("test", call_context!());
        let debug = format!("{:?}", exc);
        assert!(debug.contains("TfException"));
        assert!(debug.contains("test"));
    }

    #[test]
    fn test_tf_result() {
        fn might_fail(succeed: bool) -> TfResult<i32> {
            if succeed {
                Ok(42)
            } else {
                Err(tf_exception!("operation failed"))
            }
        }

        assert_eq!(might_fail(true).unwrap(), 42);
        assert!(might_fail(false).is_err());
    }

    #[test]
    fn test_is_fatal_throw_disabled() {
        // By default, fatal throw is disabled
        unsafe {
            std::env::remove_var("TF_FATAL_THROW");
        }
        assert!(!TfException::is_fatal_throw_enabled());
    }
}
