//! Scope descriptions for debugging and crash reports.
//!
//! This module provides utilities for annotating scopes with human-readable
//! descriptions. These descriptions are pushed onto a thread-local stack
//! and can be retrieved for debugging, logging, or crash reports.
//!
//! # Examples
//!
//! ```
//! use usd_tf::scope_description::{ScopeDescription, get_scope_stack};
//! use usd_tf::tf_describe_scope;
//!
//! fn process_files(files: &[&str]) {
//!     tf_describe_scope!("Processing {} files", files.len());
//!
//!     for file in files {
//!         tf_describe_scope!("Processing file: {}", file);
//!         // ... expensive file operations ...
//!     }
//! }
//!
//! // Get the current scope stack for debugging
//! let stack = get_scope_stack();
//! ```

use crate::CallContext;
use std::cell::RefCell;

thread_local! {
    /// Thread-local stack of scope descriptions.
    static SCOPE_STACK: RefCell<Vec<ScopeDescriptionEntry>> = const { RefCell::new(Vec::new()) };
}

/// Entry in the scope description stack.
#[derive(Clone, Debug)]
struct ScopeDescriptionEntry {
    /// The description text.
    description: String,
    /// The call context where the scope was created.
    context: CallContext,
}

/// A scope description that automatically pushes/pops from the thread-local stack.
///
/// Create this at the start of a scope to annotate it with a description.
/// The description is automatically removed when this object is dropped.
///
/// # Thread Safety
///
/// Each thread has its own independent description stack.
///
/// # Examples
///
/// ```
/// use usd_tf::scope_description::ScopeDescription;
///
/// fn load_asset(path: &str) {
///     let _desc = ScopeDescription::new(format!("Loading asset: {}", path));
///     // ... loading code ...
/// } // Description automatically removed here
/// ```
pub struct ScopeDescription {
    /// Index in the stack (for verification on drop).
    _index: usize,
}

impl ScopeDescription {
    /// Create a new scope description with the given text.
    ///
    /// The description is pushed onto the thread-local stack.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::scope_description::ScopeDescription;
    ///
    /// let _desc = ScopeDescription::new("Processing data");
    /// ```
    pub fn new(description: impl Into<String>) -> Self {
        Self::with_context(description, CallContext::default())
    }

    /// Create a new scope description with the given text and call context.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::scope_description::ScopeDescription;
    /// use usd_tf::CallContext;
    ///
    /// let ctx = CallContext::new("my_file.rs", "my_function", 42);
    /// let _desc = ScopeDescription::with_context("Processing", ctx);
    /// ```
    pub fn with_context(description: impl Into<String>, context: CallContext) -> Self {
        let index = SCOPE_STACK.with(|stack| {
            let mut stack = stack.borrow_mut();
            let index = stack.len();
            stack.push(ScopeDescriptionEntry {
                description: description.into(),
                context,
            });
            index
        });
        Self { _index: index }
    }

    /// Update the description text for this scope.
    ///
    /// This is useful when more information becomes available during execution.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::scope_description::ScopeDescription;
    ///
    /// let desc = ScopeDescription::new("Loading...");
    /// // ... load some data ...
    /// desc.set_description("Loaded 100 items");
    /// ```
    pub fn set_description(&self, description: impl Into<String>) {
        SCOPE_STACK.with(|stack| {
            let mut stack = stack.borrow_mut();
            // Use the stored index so nested scopes don't clobber each other.
            if let Some(entry) = stack.get_mut(self._index) {
                entry.description = description.into();
            }
        });
    }
}

impl Drop for ScopeDescription {
    fn drop(&mut self) {
        SCOPE_STACK.with(|stack| {
            let mut stack = stack.borrow_mut();
            // Always pop the last entry (LIFO order)
            stack.pop();
        });
    }
}

/// Get a copy of the current scope description stack for this thread.
///
/// The most recently pushed description is at the end of the vector.
///
/// # Examples
///
/// ```
/// use usd_tf::scope_description::{ScopeDescription, get_scope_stack};
///
/// let _outer = ScopeDescription::new("Outer scope");
/// let _inner = ScopeDescription::new("Inner scope");
///
/// let stack = get_scope_stack();
/// assert_eq!(stack.len(), 2);
/// assert_eq!(stack[0], "Outer scope");
/// assert_eq!(stack[1], "Inner scope");
/// ```
pub fn get_scope_stack() -> Vec<String> {
    SCOPE_STACK.with(|stack| {
        stack
            .borrow()
            .iter()
            .map(|entry| entry.description.clone())
            .collect()
    })
}

/// Get the current scope depth (number of descriptions on the stack).
///
/// # Examples
///
/// ```
/// use usd_tf::scope_description::{ScopeDescription, scope_depth};
///
/// assert_eq!(scope_depth(), 0);
/// {
///     let _desc = ScopeDescription::new("test");
///     assert_eq!(scope_depth(), 1);
/// }
/// assert_eq!(scope_depth(), 0);
/// ```
pub fn scope_depth() -> usize {
    SCOPE_STACK.with(|stack| stack.borrow().len())
}

/// Get the current (most recent) scope description.
///
/// Returns `None` if the stack is empty.
///
/// # Examples
///
/// ```
/// use usd_tf::scope_description::{ScopeDescription, current_scope};
///
/// assert!(current_scope().is_none());
/// let _desc = ScopeDescription::new("Current operation");
/// assert_eq!(current_scope(), Some("Current operation".to_string()));
/// ```
pub fn current_scope() -> Option<String> {
    SCOPE_STACK.with(|stack| stack.borrow().last().map(|entry| entry.description.clone()))
}

/// Get the full scope stack with call contexts.
///
/// Returns a vector of (description, file, function, line) tuples.
pub fn get_scope_stack_with_context() -> Vec<(String, String, String, u32)> {
    SCOPE_STACK.with(|stack| {
        stack
            .borrow()
            .iter()
            .map(|entry| {
                (
                    entry.description.clone(),
                    entry.context.file().to_string(),
                    entry.context.function().to_string(),
                    entry.context.line(),
                )
            })
            .collect()
    })
}

/// Macro to annotate the current scope with a description.
///
/// This macro creates a [`ScopeDescription`] that automatically captures
/// the current file, function, and line number.
///
/// # Examples
///
/// ```
/// use usd_tf::tf_describe_scope;
/// use usd_tf::scope_description::get_scope_stack;
///
/// fn process_data() {
///     tf_describe_scope!("Processing data");
///     // ... processing code ...
/// }
///
/// process_data();
/// ```
///
/// With formatting:
///
/// ```
/// use usd_tf::tf_describe_scope;
///
/// fn load_file(path: &str) {
///     tf_describe_scope!("Loading file: {}", path);
///     // ... loading code ...
/// }
/// ```
#[macro_export]
macro_rules! tf_describe_scope {
    ($msg:literal) => {
        let __scope_desc__ = $crate::scope_description::ScopeDescription::with_context(
            $msg,
            $crate::CallContext::new(file!(), "", line!()),
        );
    };
    ($fmt:literal, $($arg:tt)*) => {
        let __scope_desc__ = $crate::scope_description::ScopeDescription::with_context(
            format!($fmt, $($arg)*),
            $crate::CallContext::new(file!(), "", line!()),
        );
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_scope() {
        assert_eq!(scope_depth(), 0);
        assert!(current_scope().is_none());

        {
            let _desc = ScopeDescription::new("Test scope");
            assert_eq!(scope_depth(), 1);
            assert_eq!(current_scope(), Some("Test scope".to_string()));
        }

        assert_eq!(scope_depth(), 0);
        assert!(current_scope().is_none());
    }

    #[test]
    fn test_nested_scopes() {
        {
            let _outer = ScopeDescription::new("Outer");
            assert_eq!(current_scope(), Some("Outer".to_string()));

            {
                let _inner = ScopeDescription::new("Inner");
                assert_eq!(current_scope(), Some("Inner".to_string()));
                assert_eq!(scope_depth(), 2);

                let stack = get_scope_stack();
                assert_eq!(stack, vec!["Outer", "Inner"]);
            }

            assert_eq!(current_scope(), Some("Outer".to_string()));
            assert_eq!(scope_depth(), 1);
        }

        assert_eq!(scope_depth(), 0);
    }

    #[test]
    fn test_set_description() {
        let desc = ScopeDescription::new("Initial");
        assert_eq!(current_scope(), Some("Initial".to_string()));

        desc.set_description("Updated");
        assert_eq!(current_scope(), Some("Updated".to_string()));
    }

    #[test]
    fn test_set_description_outer_with_nested() {
        // Calling set_description on the outer scope must not touch the inner scope.
        let outer = ScopeDescription::new("Outer initial");
        let _inner = ScopeDescription::new("Inner");

        outer.set_description("Outer updated");

        let stack = get_scope_stack();
        assert_eq!(stack[0], "Outer updated");
        assert_eq!(stack[1], "Inner");
    }

    #[test]
    fn test_with_context() {
        let ctx = CallContext::new("test.rs", "test_fn", 42);
        let _desc = ScopeDescription::with_context("Test", ctx);

        let stack = get_scope_stack_with_context();
        assert_eq!(stack.len(), 1);
        assert_eq!(stack[0].0, "Test");
        assert_eq!(stack[0].1, "test.rs");
        assert_eq!(stack[0].2, "test_fn");
        assert_eq!(stack[0].3, 42);
    }

    #[test]
    fn test_multiple_levels() {
        let _a = ScopeDescription::new("Level A");
        let _b = ScopeDescription::new("Level B");
        let _c = ScopeDescription::new("Level C");

        assert_eq!(scope_depth(), 3);
        let stack = get_scope_stack();
        assert_eq!(stack, vec!["Level A", "Level B", "Level C"]);
    }

    #[test]
    fn test_empty_stack() {
        assert!(get_scope_stack().is_empty());
        assert!(get_scope_stack_with_context().is_empty());
    }
}
