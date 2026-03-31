//! Diagnostic helper functions for posting errors, warnings, and status messages.
//!
//! Port of pxr/base/tf/diagnosticHelper.h
//!
//! Provides the internal helper functions used by TF_ERROR, TF_WARN, TF_STATUS
//! macros. In Rust, these wrap the `issue_error`, `issue_warning`, `issue_status`
//! functions from `crate::diagnostic` with additional overloads for diagnostic info.

use std::any::Any;

use crate::call_context::CallContext;
use crate::diagnostic::{Diagnostic, DiagnosticType, issue_error, issue_status, issue_warning};

/// Diagnostic info type (type-erased auxiliary data attached to a diagnostic).
///
/// Matches C++ `TfDiagnosticInfo = std::any`.
pub type DiagnosticInfo = Box<dyn Any + Send + Sync>;

// --- Error helpers ---

/// Post an error with a message string and diagnostic type code.
pub fn post_error_helper(context: CallContext, code: DiagnosticType, msg: &str) {
    issue_error(context, code, msg.to_string());
}

/// Post an error with diagnostic info attached.
///
/// The info is currently not stored (would require extending Diagnostic struct).
pub fn post_error_with_info(
    context: CallContext,
    _info: DiagnosticInfo,
    code: DiagnosticType,
    msg: &str,
) {
    issue_error(context, code, msg.to_string());
}

/// Post an error quietly (suppresses console output, only stored for programmatic access).
///
/// Uses per-error quiet flag on the Diagnostic itself rather than toggling
/// the global quiet flag, which would affect concurrent threads.
pub fn post_quietly_error_helper(context: CallContext, code: DiagnosticType, msg: &str) {
    use crate::diagnostic_mgr::DiagnosticMgr;
    let diag = Diagnostic::quiet(code, context, msg.to_string());
    DiagnosticMgr::instance().post_error(diag);
}

/// Post a quiet error with diagnostic info.
pub fn post_quietly_error_with_info(
    context: CallContext,
    _info: DiagnosticInfo,
    code: DiagnosticType,
    msg: &str,
) {
    post_quietly_error_helper(context, code, msg);
}

// --- Warning helpers ---

/// Post a warning with a message string.
pub fn post_warning_helper(context: CallContext, msg: &str) {
    issue_warning(context, msg.to_string());
}

/// Post a warning with a diagnostic type code.
pub fn post_warning_with_code(context: CallContext, _code: DiagnosticType, msg: &str) {
    issue_warning(context, msg.to_string());
}

/// Post a warning with diagnostic info attached.
pub fn post_warning_with_info(
    context: CallContext,
    _info: DiagnosticInfo,
    _code: DiagnosticType,
    msg: &str,
) {
    issue_warning(context, msg.to_string());
}

// --- Status helpers ---

/// Post a status message.
pub fn post_status_helper(context: CallContext, msg: &str) {
    issue_status(context, msg.to_string());
}

/// Post a status message with a diagnostic type code.
pub fn post_status_with_code(context: CallContext, _code: DiagnosticType, msg: &str) {
    issue_status(context, msg.to_string());
}

/// Post a status message with diagnostic info attached.
pub fn post_status_with_info(
    context: CallContext,
    _info: DiagnosticInfo,
    _code: DiagnosticType,
    msg: &str,
) {
    issue_status(context, msg.to_string());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_post_helpers_no_panic() {
        let ctx = CallContext::new("test_file.rs", "test_fn", 42);
        post_error_helper(ctx.clone(), DiagnosticType::RuntimeError, "test error");
        post_warning_helper(ctx.clone(), "test warning");
        post_status_helper(ctx, "test status");
    }

    #[test]
    fn test_post_quietly_no_panic() {
        let ctx = CallContext::new("test_file.rs", "test_fn", 42);
        post_quietly_error_helper(ctx, DiagnosticType::RuntimeError, "quiet error");
    }

    #[test]
    fn test_post_with_info_no_panic() {
        let ctx = CallContext::new("test_file.rs", "test_fn", 42);
        let info: DiagnosticInfo = Box::new(42i32);
        post_error_with_info(ctx, info, DiagnosticType::RuntimeError, "error with info");
    }
}
