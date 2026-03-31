//! JSON parsing error types.

use std::fmt;

/// A struct containing information about a JSON parsing error.
///
/// # Examples
///
/// ```
/// use usd_js::{parse_string, JsParseError};
///
/// let result = parse_string("{invalid}");
/// if let Err(err) = result {
///     println!("Parse error at line {}, column {}: {}",
///              err.line, err.column, err.reason);
/// }
/// ```
#[derive(Debug, Clone, Default)]
pub struct JsParseError {
    /// Line number where the error occurred (1-based).
    pub line: usize,
    /// Column number where the error occurred (1-based).
    pub column: usize,
    /// Description of the error.
    pub reason: String,
}

impl JsParseError {
    /// Creates a new parse error.
    #[must_use]
    pub fn new(line: usize, column: usize, reason: impl Into<String>) -> Self {
        Self {
            line,
            column,
            reason: reason.into(),
        }
    }
}

impl fmt::Display for JsParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line > 0 && self.column > 0 {
            write!(f, "{}:{}: {}", self.line, self.column, self.reason)
        } else if self.line > 0 {
            write!(f, "line {}: {}", self.line, self.reason)
        } else {
            write!(f, "{}", self.reason)
        }
    }
}

impl std::error::Error for JsParseError {}

impl From<serde_json::Error> for JsParseError {
    fn from(err: serde_json::Error) -> Self {
        Self {
            line: err.line(),
            column: err.column(),
            reason: err.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = JsParseError::new(10, 5, "unexpected token");
        assert_eq!(err.to_string(), "10:5: unexpected token");
    }

    #[test]
    fn test_error_display_no_position() {
        let err = JsParseError::new(0, 0, "unknown error");
        assert_eq!(err.to_string(), "unknown error");
    }

    #[test]
    fn test_from_serde_error() {
        let json_err = serde_json::from_str::<serde_json::Value>("{invalid}").unwrap_err();
        let err = JsParseError::from(json_err);

        assert!(err.line > 0);
        assert!(!err.reason.is_empty());
    }
}
