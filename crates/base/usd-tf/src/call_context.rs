//! Call context for capturing source location information.
//!
//! Provides structures and macros for recording where in source code
//! a function was called from. This is primarily used by diagnostic
//! macros to report error locations.
//!
//! # Examples
//!
//! ```
//! use usd_tf::{CallContext, call_context};
//!
//! let ctx = call_context!();
//! println!("Called from {}:{}", ctx.file(), ctx.line());
//! ```

use std::fmt;

/// A structure that captures source code location information.
///
/// `CallContext` stores the file name, function name, and line number
/// where it was created. This is typically done via the `call_context!`
/// macro rather than by constructing directly.
#[derive(Clone, Copy)]
pub struct CallContext {
    file: &'static str,
    function: &'static str,
    line: u32,
    hidden: bool,
}

impl CallContext {
    /// Create a new call context.
    ///
    /// Typically you should use the `call_context!` macro instead of
    /// calling this directly.
    #[inline]
    #[must_use]
    pub const fn new(file: &'static str, function: &'static str, line: u32) -> Self {
        Self {
            file,
            function,
            line,
            hidden: false,
        }
    }

    /// Create an empty/invalid call context.
    #[inline]
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            file: "",
            function: "",
            line: 0,
            hidden: false,
        }
    }

    /// Returns the source file name.
    #[inline]
    #[must_use]
    pub const fn file(&self) -> &'static str {
        self.file
    }

    /// Returns the function name.
    #[inline]
    #[must_use]
    pub const fn function(&self) -> &'static str {
        self.function
    }

    /// Returns the line number.
    #[inline]
    #[must_use]
    pub const fn line(&self) -> u32 {
        self.line
    }

    /// Returns whether this context is hidden.
    ///
    /// Hidden contexts are used for internal diagnostics that
    /// should not be displayed to users.
    #[inline]
    #[must_use]
    pub const fn is_hidden(&self) -> bool {
        self.hidden
    }

    /// Mark this context as hidden.
    #[inline]
    #[must_use]
    pub const fn hide(self) -> Self {
        Self {
            file: self.file,
            function: self.function,
            line: self.line,
            hidden: true,
        }
    }

    /// Returns true if this context has valid location information.
    #[inline]
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        !self.file.is_empty() && !self.function.is_empty()
    }
}

impl Default for CallContext {
    fn default() -> Self {
        Self::empty()
    }
}

impl fmt::Debug for CallContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CallContext")
            .field("file", &self.file)
            .field("function", &self.function)
            .field("line", &self.line)
            .field("hidden", &self.hidden)
            .finish()
    }
}

impl fmt::Display for CallContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_valid() {
            write!(f, "{}:{} in {}", self.file, self.line, self.function)
        } else {
            write!(f, "<unknown location>")
        }
    }
}

/// Create a `CallContext` capturing the current source location.
///
/// This macro captures the file, function name, and line number
/// where it is invoked.
///
/// # Examples
///
/// ```
/// use usd_tf::call_context;
///
/// fn my_function() {
///     let ctx = call_context!();
///     assert!(!ctx.file().is_empty());
///     assert!(ctx.line() > 0);
/// }
/// my_function();
/// ```
#[macro_export]
macro_rules! call_context {
    () => {
        $crate::CallContext::new(file!(), module_path!(), line!())
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_call_context_new() {
        let ctx = CallContext::new("test.rs", "test_fn", 42);
        assert_eq!(ctx.file(), "test.rs");
        assert_eq!(ctx.function(), "test_fn");
        assert_eq!(ctx.line(), 42);
        assert!(!ctx.is_hidden());
        assert!(ctx.is_valid());
    }

    #[test]
    fn test_call_context_empty() {
        let ctx = CallContext::empty();
        assert_eq!(ctx.file(), "");
        assert_eq!(ctx.function(), "");
        assert_eq!(ctx.line(), 0);
        assert!(!ctx.is_valid());
    }

    #[test]
    fn test_call_context_hide() {
        let ctx = CallContext::new("test.rs", "test_fn", 42);
        assert!(!ctx.is_hidden());

        let hidden = ctx.hide();
        assert!(hidden.is_hidden());
        assert_eq!(hidden.file(), ctx.file());
        assert_eq!(hidden.line(), ctx.line());
    }

    #[test]
    fn test_call_context_display() {
        let ctx = CallContext::new("test.rs", "my_function", 42);
        let s = format!("{}", ctx);
        assert!(s.contains("test.rs"));
        assert!(s.contains("42"));
        assert!(s.contains("my_function"));
    }

    #[test]
    fn test_call_context_display_invalid() {
        let ctx = CallContext::empty();
        let s = format!("{}", ctx);
        assert!(s.contains("unknown"));
    }

    #[test]
    fn test_call_context_macro() {
        let ctx = call_context!();
        assert!(!ctx.file().is_empty());
        assert!(!ctx.function().is_empty());
        assert!(ctx.line() > 0);
        assert!(ctx.is_valid());
    }

    #[test]
    fn test_call_context_default() {
        let ctx: CallContext = Default::default();
        assert!(!ctx.is_valid());
    }
}
