//! Lightweight diagnostic utilities.
//!
//! This module provides stripped-down diagnostic macros that don't require
//! string formatting infrastructure. These are designed for use in
//! performance-critical or minimal-dependency contexts.
//!
//! Unlike the full diagnostic system, these utilities use static strings
//! for efficiency and minimal binary size impact.

use super::{CallContext, DiagnosticType};
use crate::diagnostic::{issue_error, issue_fatal_error, issue_status, issue_warning};

/// Helper for lightweight diagnostic issuing.
///
/// This is used internally by the lite diagnostic macros to post diagnostics
/// without the overhead of the full diagnostic system.
pub struct DiagnosticLiteHelper {
    context: CallContext,
    diagnostic_type: DiagnosticType,
}

impl DiagnosticLiteHelper {
    /// Creates a new lite helper with the given context and type.
    #[must_use]
    pub const fn new(context: CallContext, diagnostic_type: DiagnosticType) -> Self {
        Self {
            context,
            diagnostic_type,
        }
    }

    /// Issues an error with a static message.
    pub fn issue_error(&self, msg: &str) {
        issue_error(self.context.clone(), self.diagnostic_type, msg.to_string());
    }

    /// Issues a fatal error, notifies delegates, and aborts.
    #[cold]
    pub fn issue_fatal_error(&self, msg: &str) -> ! {
        issue_fatal_error(self.context.clone(), msg.to_string());
    }

    /// Issues a warning with a static message.
    pub fn issue_warning(&self, msg: &str) {
        issue_warning(self.context.clone(), msg.to_string());
    }

    /// Issues a status message.
    pub fn issue_status(&self, msg: &str) {
        issue_status(self.context.clone(), msg.to_string());
    }

    /// Returns the context.
    #[must_use]
    pub const fn context(&self) -> &CallContext {
        &self.context
    }

    /// Returns the diagnostic type.
    #[must_use]
    pub const fn diagnostic_type(&self) -> DiagnosticType {
        self.diagnostic_type
    }
}

/// Creates a lite coding error helper.
///
/// This is a low-level macro for creating diagnostic helpers in
/// performance-critical code paths.
#[macro_export]
macro_rules! tf_lite_coding_error {
    () => {
        $crate::DiagnosticLiteHelper::new(
            $crate::call_context!(),
            $crate::DiagnosticType::CodingError,
        )
    };
}

/// Creates a lite runtime error helper.
#[macro_export]
macro_rules! tf_lite_runtime_error {
    () => {
        $crate::DiagnosticLiteHelper::new(
            $crate::call_context!(),
            $crate::DiagnosticType::RuntimeError,
        )
    };
}

/// Creates a lite fatal error helper.
#[macro_export]
macro_rules! tf_lite_fatal_error {
    () => {
        $crate::DiagnosticLiteHelper::new(
            $crate::call_context!(),
            $crate::DiagnosticType::FatalError,
        )
    };
}

/// Creates a lite warning helper.
#[macro_export]
macro_rules! tf_lite_warning {
    () => {
        $crate::DiagnosticLiteHelper::new($crate::call_context!(), $crate::DiagnosticType::Warning)
    };
}

/// Creates a lite status helper.
#[macro_export]
macro_rules! tf_lite_status {
    () => {
        $crate::DiagnosticLiteHelper::new($crate::call_context!(), $crate::DiagnosticType::Status)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lite_helper_creation() {
        let ctx = CallContext::new("test.rs", "test_func", 42);
        let helper = DiagnosticLiteHelper::new(ctx, DiagnosticType::Warning);

        assert_eq!(helper.context().file(), "test.rs");
        assert_eq!(helper.context().line(), 42);
        assert_eq!(helper.diagnostic_type(), DiagnosticType::Warning);
    }

    #[test]
    fn test_issue_error() {
        let ctx = CallContext::new("test.rs", "test_func", 42);
        let helper = DiagnosticLiteHelper::new(ctx, DiagnosticType::CodingError);
        helper.issue_error("Test error message");
    }

    #[test]
    fn test_issue_warning() {
        let ctx = CallContext::new("test.rs", "test_func", 42);
        let helper = DiagnosticLiteHelper::new(ctx, DiagnosticType::Warning);
        helper.issue_warning("Test warning message");
    }

    #[test]
    fn test_issue_status() {
        let ctx = CallContext::new("test.rs", "test_func", 42);
        let helper = DiagnosticLiteHelper::new(ctx, DiagnosticType::Status);
        helper.issue_status("Test status message");
    }

    #[test]
    #[ignore = "calls process::abort which kills test harness"]
    fn test_issue_fatal() {
        let ctx = CallContext::new("test.rs", "test_func", 42);
        let helper = DiagnosticLiteHelper::new(ctx, DiagnosticType::FatalError);
        helper.issue_fatal_error("Test fatal error");
    }
}
