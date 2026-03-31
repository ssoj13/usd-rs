//! TfError - Error diagnostic type.
//!
//! Represents an object that contains error information.
//! This is the Rust equivalent of C++ TfError.

use super::CallContext;
use super::TfEnum;
use super::diagnostic::{Diagnostic, DiagnosticType};

/// Information that can be attached to a diagnostic.
#[allow(dead_code)] // C++ parity - diagnostic info type
pub type DiagnosticInfo = Option<Box<dyn std::any::Any + Send + Sync>>;

/// Error diagnostic message.
///
/// Represents an error that occurred during execution. Errors are
/// collected by the DiagnosticMgr and can be queried using ErrorMark.
#[derive(Debug, Clone)]
pub struct TfError {
    /// The underlying diagnostic.
    pub(crate) diagnostic: Diagnostic,
    /// Error code as TfEnum.
    code: TfEnum,
    /// Error code as string.
    code_string: String,
    /// Serial number for ordering.
    pub(crate) serial: usize,
}

impl TfError {
    /// Create a new TfError with full parameters.
    ///
    /// `diagnostic_type` and `quiet` are independent: the type comes from the
    /// caller's error code (C++ TfEnum), not from the quiet flag.
    #[allow(dead_code)] // C++ parity - error creation API
    pub(crate) fn new(
        code: TfEnum,
        code_string: &str,
        diagnostic_type: DiagnosticType,
        context: CallContext,
        commentary: String,
        quiet: bool,
    ) -> Self {
        let diagnostic = if quiet {
            Diagnostic::quiet(diagnostic_type, context, commentary)
        } else {
            Diagnostic::new(diagnostic_type, context, commentary)
        };

        Self {
            diagnostic,
            code,
            code_string: code_string.to_string(),
            serial: 0,
        }
    }

    /// Create TfError from an existing Diagnostic.
    /// Used internally by DiagnosticMgr.
    pub(crate) fn from_diagnostic(serial: usize, diagnostic: Diagnostic) -> Self {
        Self {
            diagnostic,
            code: TfEnum::default(),
            code_string: String::new(),
            serial,
        }
    }

    /// Return the error code posted.
    #[inline]
    pub fn error_code(&self) -> TfEnum {
        self.code
    }

    /// Return the diagnostic code posted as a string.
    #[inline]
    pub fn error_code_as_string(&self) -> &str {
        &self.code_string
    }

    /// Return the call context where the error was issued.
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

    /// Return true if the error was posted quietly.
    #[inline]
    pub fn is_quiet(&self) -> bool {
        self.diagnostic.quiet
    }

    /// Return true if this is a fatal error.
    #[inline]
    pub fn is_fatal(&self) -> bool {
        self.diagnostic.is_fatal()
    }

    /// Return true if this is a coding error.
    #[inline]
    pub fn is_coding_error(&self) -> bool {
        self.diagnostic.is_coding_error()
    }

    /// Get the serial number.
    #[inline]
    pub fn serial(&self) -> usize {
        self.serial
    }

    /// Set the serial number (used by DiagnosticMgr).
    #[allow(dead_code)] // C++ parity - serial number management
    pub(crate) fn set_serial(&mut self, serial: usize) {
        self.serial = serial;
    }

    /// Get the underlying diagnostic.
    #[inline]
    pub fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }
}

impl std::fmt::Display for TfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Error {}: {} -- {}:{}",
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
    fn test_tf_error_creation() {
        let ctx = CallContext::new("test.rs", "test_fn", 42);
        let code = TfEnum::from_int(1);
        let error = TfError::new(
            code,
            "TEST_ERROR",
            DiagnosticType::RuntimeError,
            ctx,
            "Something went wrong".to_string(),
            false,
        );

        assert_eq!(error.error_code_as_string(), "TEST_ERROR");
        assert_eq!(error.commentary(), "Something went wrong");
        assert_eq!(error.source_file_name(), "test.rs");
        assert_eq!(error.source_line_number(), 42);
        assert!(!error.is_quiet());
    }

    #[test]
    fn test_tf_error_quiet() {
        let ctx = CallContext::new("test.rs", "test_fn", 10);
        let code = TfEnum::from_int(2);
        let error = TfError::new(
            code,
            "QUIET_ERROR",
            DiagnosticType::RuntimeError,
            ctx,
            "Quiet error".to_string(),
            true,
        );

        assert!(error.is_quiet());
    }

    #[test]
    fn test_tf_error_augment() {
        let ctx = CallContext::new("test.rs", "test_fn", 10);
        let code = TfEnum::from_int(3);
        let mut error = TfError::new(
            code,
            "AUG_ERROR",
            DiagnosticType::RuntimeError,
            ctx,
            "First".to_string(),
            false,
        );

        error.augment_commentary("Second");
        assert!(error.commentary().contains("First"));
        assert!(error.commentary().contains("Second"));
    }
}
