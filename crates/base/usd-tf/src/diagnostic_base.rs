//! Base class for diagnostic messages.
//!
//! This module provides the base type for all diagnostic messages (errors,
//! warnings, status). It associates a diagnostic with a call context and
//! allows for commentary augmentation.

use std::any::Any;
use std::fmt;

use super::{CallContext, DiagnosticType};

/// Type-erased diagnostic information that can be attached to diagnostics.
pub type TfDiagnosticInfo = Box<dyn Any + Send + Sync>;

/// Base class for all diagnostic messages.
///
/// This represents a diagnostic message with associated context and commentary.
/// It's used as the base for specific diagnostic types (errors, warnings, status).
#[derive(Clone)]
pub struct TfDiagnosticBase {
    /// The call context where the diagnostic was issued
    context: CallContext,
    /// The diagnostic type
    diagnostic_type: DiagnosticType,
    /// The commentary string describing the diagnostic
    commentary: String,
    /// Optional additional diagnostic information
    info: Option<String>,
}

impl TfDiagnosticBase {
    /// Creates a new diagnostic base with the given context and type.
    #[must_use]
    pub fn new(context: CallContext, diagnostic_type: DiagnosticType) -> Self {
        Self {
            context,
            diagnostic_type,
            commentary: String::new(),
            info: None,
        }
    }

    /// Creates a new diagnostic with context, type, and commentary.
    #[must_use]
    pub fn with_commentary(
        context: CallContext,
        diagnostic_type: DiagnosticType,
        commentary: String,
    ) -> Self {
        Self {
            context,
            diagnostic_type,
            commentary,
            info: None,
        }
    }

    /// Returns the call context where the message was issued.
    #[must_use]
    pub const fn get_context(&self) -> &CallContext {
        &self.context
    }

    /// Returns the diagnostic type.
    #[must_use]
    pub const fn get_diagnostic_type(&self) -> DiagnosticType {
        self.diagnostic_type
    }

    /// Returns the source file name that the diagnostic was posted from.
    #[must_use]
    pub fn get_source_file_name(&self) -> &str {
        self.context.file()
    }

    /// Returns the source line number that the diagnostic was posted from.
    #[must_use]
    pub const fn get_source_line_number(&self) -> u32 {
        self.context.line()
    }

    /// Returns the source function that the diagnostic was posted from.
    #[must_use]
    pub fn get_source_function(&self) -> &str {
        self.context.function()
    }

    /// Returns the commentary string describing this diagnostic.
    #[must_use]
    pub fn get_commentary(&self) -> &str {
        &self.commentary
    }

    /// Sets the commentary string.
    pub fn set_commentary(&mut self, commentary: String) {
        self.commentary = commentary;
    }

    /// Adds to the commentary string describing this diagnostic.
    ///
    /// Each string added to the commentary is separated from the previous one
    /// with a newline. The string should not end with a newline.
    pub fn augment_commentary(&mut self, additional: &str) {
        if self.commentary.is_empty() {
            self.commentary = additional.to_string();
        } else {
            self.commentary.push('\n');
            self.commentary.push_str(additional);
        }
    }

    /// Sets the diagnostic info.
    pub fn set_info(&mut self, info: String) {
        self.info = Some(info);
    }

    /// Gets the diagnostic info if set.
    #[must_use]
    pub fn get_info(&self) -> Option<&str> {
        self.info.as_deref()
    }

    /// Returns true if this diagnostic has info attached.
    #[must_use]
    pub fn has_info(&self) -> bool {
        self.info.is_some()
    }

    /// Clears the diagnostic info.
    pub fn clear_info(&mut self) {
        self.info = None;
    }
}

impl fmt::Debug for TfDiagnosticBase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TfDiagnosticBase")
            .field("type", &self.diagnostic_type)
            .field("file", &self.context.file())
            .field("line", &self.context.line())
            .field("function", &self.context.function())
            .field("commentary", &self.commentary)
            .field("has_info", &self.info.is_some())
            .finish()
    }
}

impl fmt::Display for TfDiagnosticBase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{:?}] {}:{} in {}: {}",
            self.diagnostic_type,
            self.context.file(),
            self.context.line(),
            self.context.function(),
            self.commentary
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_diagnostic_base() {
        let ctx = CallContext::new("test.rs", "test_func", 42);
        let diag = TfDiagnosticBase::new(ctx, DiagnosticType::Warning);

        assert_eq!(diag.get_source_file_name(), "test.rs");
        assert_eq!(diag.get_source_line_number(), 42);
        assert_eq!(diag.get_source_function(), "test_func");
        assert_eq!(diag.get_diagnostic_type(), DiagnosticType::Warning);
    }

    #[test]
    fn test_augment_commentary() {
        let ctx = CallContext::new("test.rs", "test_func", 42);
        let mut diag = TfDiagnosticBase::with_commentary(
            ctx,
            DiagnosticType::CodingError,
            "Initial message".to_string(),
        );

        assert_eq!(diag.get_commentary(), "Initial message");

        diag.augment_commentary("Additional info");
        assert_eq!(diag.get_commentary(), "Initial message\nAdditional info");

        diag.augment_commentary("More info");
        assert_eq!(
            diag.get_commentary(),
            "Initial message\nAdditional info\nMore info"
        );
    }

    #[test]
    fn test_diagnostic_info() {
        let ctx = CallContext::new("test.rs", "test_func", 42);
        let mut diag = TfDiagnosticBase::new(ctx, DiagnosticType::RuntimeError);

        assert!(!diag.has_info());
        assert!(diag.get_info().is_none());

        diag.set_info("Extra information".to_string());
        assert!(diag.has_info());
        assert_eq!(diag.get_info(), Some("Extra information"));

        diag.clear_info();
        assert!(!diag.has_info());
    }

    #[test]
    fn test_display() {
        let ctx = CallContext::new("myfile.rs", "my_function", 123);
        let diag = TfDiagnosticBase::with_commentary(
            ctx,
            DiagnosticType::Warning,
            "Something suspicious".to_string(),
        );

        let output = format!("{}", diag);
        assert!(output.contains("myfile.rs"));
        assert!(output.contains("123"));
        assert!(output.contains("my_function"));
        assert!(output.contains("Something suspicious"));
    }
}
