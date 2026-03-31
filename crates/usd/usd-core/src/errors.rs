//! USD error types.
//!
//! Port of pxr/usd/usd/errors.h
//!
//! Defines specific error types thrown by USD when invalid operations
//! are attempted, such as accessing expired or null prims.

use std::fmt;

/// Error thrown when code attempts to access an invalid (expired or null) prim.
///
/// This corresponds to C++ UsdExpiredPrimAccessError. In Rust, this is
/// typically returned as a Result::Err rather than thrown as an exception.
#[derive(Debug, Clone)]
pub struct ExpiredPrimAccessError {
    /// Descriptive error message.
    pub message: String,
}

impl ExpiredPrimAccessError {
    /// Creates a new expired prim access error.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ExpiredPrimAccessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Expired prim access: {}", self.message)
    }
}

impl std::error::Error for ExpiredPrimAccessError {}

/// General USD error type encompassing various error conditions.
#[derive(Debug, Clone)]
pub enum UsdError {
    /// Attempted to access an expired or null prim.
    ExpiredPrimAccess(ExpiredPrimAccessError),
    /// Attempted an invalid edit operation.
    InvalidEdit(String),
    /// Composition error.
    CompositionError(String),
    /// Schema-related error.
    SchemaError(String),
    /// General error.
    Other(String),
}

impl fmt::Display for UsdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExpiredPrimAccess(e) => write!(f, "{}", e),
            Self::InvalidEdit(msg) => write!(f, "Invalid edit: {}", msg),
            Self::CompositionError(msg) => write!(f, "Composition error: {}", msg),
            Self::SchemaError(msg) => write!(f, "Schema error: {}", msg),
            Self::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for UsdError {}

impl From<ExpiredPrimAccessError> for UsdError {
    fn from(e: ExpiredPrimAccessError) -> Self {
        Self::ExpiredPrimAccess(e)
    }
}

impl From<String> for UsdError {
    fn from(s: String) -> Self {
        Self::Other(s)
    }
}

impl From<&str> for UsdError {
    fn from(s: &str) -> Self {
        Self::Other(s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expired_prim_error() {
        let err = ExpiredPrimAccessError::new("prim /World has expired");
        assert!(err.to_string().contains("expired"));
    }

    #[test]
    fn test_usd_error_from_string() {
        let err: UsdError = "something went wrong".into();
        assert!(err.to_string().contains("something went wrong"));
    }
}
