//! TfStatus - Status diagnostic type.
//!
//! Represents an object that contains status message information.
//! This is the Rust equivalent of C++ TfStatus.

use super::CallContext;
use super::TfEnum;
use super::diagnostic::{Diagnostic, DiagnosticType};

/// Status diagnostic message.
///
/// Represents a status/informational message.
/// Status messages are purely informational and don't indicate problems.
#[derive(Debug, Clone)]
pub struct TfStatus {
    /// The underlying diagnostic.
    diagnostic: Diagnostic,
    /// Status code as TfEnum.
    code: TfEnum,
    /// Status code as string.
    code_string: String,
}

impl TfStatus {
    /// Create a new TfStatus.
    #[allow(dead_code)] // C++ parity - diagnostic creation
    pub(crate) fn new(
        code: TfEnum,
        code_string: &str,
        context: CallContext,
        commentary: String,
        quiet: bool,
    ) -> Self {
        let diagnostic = if quiet {
            Diagnostic::quiet(DiagnosticType::Status, context, commentary)
        } else {
            Diagnostic::new(DiagnosticType::Status, context, commentary)
        };

        Self {
            diagnostic,
            code,
            code_string: code_string.to_string(),
        }
    }

    /// Return the status code posted.
    #[inline]
    pub fn status_code(&self) -> TfEnum {
        self.code
    }

    /// Return the diagnostic code posted as a string.
    #[inline]
    pub fn status_code_as_string(&self) -> &str {
        &self.code_string
    }

    /// Return the call context where the status was issued.
    #[inline]
    pub fn context(&self) -> &CallContext {
        &self.diagnostic.context
    }

    /// Return the source file name.
    #[inline]
    pub fn source_file_name(&self) -> &str {
        self.diagnostic.context.file()
    }

    /// Return the source line number.
    #[inline]
    pub fn source_line_number(&self) -> u32 {
        self.diagnostic.context.line()
    }

    /// Return the commentary string.
    #[inline]
    pub fn commentary(&self) -> &str {
        &self.diagnostic.message
    }

    /// Return the source function name.
    #[inline]
    pub fn source_function(&self) -> &str {
        self.diagnostic.context.function()
    }

    /// Add to the commentary string.
    pub fn augment_commentary(&mut self, s: &str) {
        self.diagnostic.augment(s);
    }

    /// Return true if the status was posted quietly.
    #[inline]
    pub fn is_quiet(&self) -> bool {
        self.diagnostic.quiet
    }

    /// Get the underlying diagnostic.
    #[inline]
    pub fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }
}

impl std::fmt::Display for TfStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Status {}: {} -- {}:{}",
            self.code_string,
            self.diagnostic.message,
            self.diagnostic.context.file(),
            self.diagnostic.context.line()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tf_status_creation() {
        let ctx = CallContext::new("test.rs", "test_fn", 42);
        let code = TfEnum::from_int(1);
        let status = TfStatus::new(
            code,
            "TEST_STATUS",
            ctx,
            "Processing started".to_string(),
            false,
        );

        assert_eq!(status.status_code_as_string(), "TEST_STATUS");
        assert_eq!(status.commentary(), "Processing started");
        assert_eq!(status.source_file_name(), "test.rs");
        assert_eq!(status.source_line_number(), 42);
        assert!(!status.is_quiet());
    }

    #[test]
    fn test_tf_status_quiet() {
        let ctx = CallContext::new("test.rs", "test_fn", 10);
        let code = TfEnum::from_int(2);
        let status = TfStatus::new(code, "QUIET_STATUS", ctx, "Quiet status".to_string(), true);

        assert!(status.is_quiet());
    }
}
