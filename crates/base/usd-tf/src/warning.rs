//! TfWarning - Warning diagnostic type.
//!
//! Represents an object that contains warning information.
//! This is the Rust equivalent of C++ TfWarning.

use super::CallContext;
use super::TfEnum;
use super::diagnostic::{Diagnostic, DiagnosticType};

/// Warning diagnostic message.
///
/// Represents a warning that occurred during execution.
/// Warnings indicate potential issues that don't prevent execution.
#[derive(Debug, Clone)]
pub struct TfWarning {
    /// The underlying diagnostic.
    diagnostic: Diagnostic,
    /// Warning code as TfEnum.
    code: TfEnum,
    /// Warning code as string.
    code_string: String,
}

impl TfWarning {
    /// Create a new TfWarning.
    #[allow(dead_code)] // C++ parity - diagnostic creation
    pub(crate) fn new(
        code: TfEnum,
        code_string: &str,
        context: CallContext,
        commentary: String,
        quiet: bool,
    ) -> Self {
        let diagnostic = if quiet {
            Diagnostic::quiet(DiagnosticType::Warning, context, commentary)
        } else {
            Diagnostic::new(DiagnosticType::Warning, context, commentary)
        };

        Self {
            diagnostic,
            code,
            code_string: code_string.to_string(),
        }
    }

    /// Return the warning code posted.
    #[inline]
    pub fn warning_code(&self) -> TfEnum {
        self.code
    }

    /// Return the diagnostic code posted as a string.
    #[inline]
    pub fn warning_code_as_string(&self) -> &str {
        &self.code_string
    }

    /// Return the call context where the warning was issued.
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

    /// Return true if the warning was posted quietly.
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

impl std::fmt::Display for TfWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Warning {}: {} -- {}:{}",
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
    fn test_tf_warning_creation() {
        let ctx = CallContext::new("test.rs", "test_fn", 42);
        let code = TfEnum::from_int(1);
        let warning = TfWarning::new(
            code,
            "TEST_WARNING",
            ctx,
            "Something might be wrong".to_string(),
            false,
        );

        assert_eq!(warning.warning_code_as_string(), "TEST_WARNING");
        assert_eq!(warning.commentary(), "Something might be wrong");
        assert_eq!(warning.source_file_name(), "test.rs");
        assert_eq!(warning.source_line_number(), 42);
        assert!(!warning.is_quiet());
    }

    #[test]
    fn test_tf_warning_quiet() {
        let ctx = CallContext::new("test.rs", "test_fn", 10);
        let code = TfEnum::from_int(2);
        let warning = TfWarning::new(
            code,
            "QUIET_WARNING",
            ctx,
            "Quiet warning".to_string(),
            true,
        );

        assert!(warning.is_quiet());
    }
}
