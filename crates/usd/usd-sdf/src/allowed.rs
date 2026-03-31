//! Operation permission result type.
//!
//! `Allowed` indicates if an operation is permitted and, if not, why not.

use std::fmt;

/// Indicates if an operation is allowed and, if not, why not.
///
/// An `Allowed` either evaluates to `true` in a boolean context
/// or evaluates to `false` and has a string annotation explaining why.
///
/// This is similar to `Result<(), String>` but with a more ergonomic API
/// for the common pattern of checking if something is allowed.
///
/// # Examples
///
/// ```ignore
/// use usd_sdf::Allowed;
///
/// // Allowed operation
/// let ok = Allowed::yes();
/// assert!(ok.is_allowed());
///
/// // Disallowed operation with reason
/// let no = Allowed::no("Cannot modify read-only layer");
/// assert!(!no.is_allowed());
/// assert_eq!(no.why_not(), "Cannot modify read-only layer");
///
/// // Conditional construction
/// let allowed = Allowed::with_condition(true, "Would fail");
/// assert!(allowed.is_allowed());
/// ```
#[derive(Clone, Debug)]
pub struct Allowed {
    /// None means allowed, Some(reason) means not allowed.
    why_not: Option<String>,
}

impl Default for Allowed {
    fn default() -> Self {
        Self::yes()
    }
}

impl Allowed {
    /// Creates an allowed result.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Allowed;
    ///
    /// let allowed = Allowed::yes();
    /// assert!(allowed.is_allowed());
    /// ```
    pub fn yes() -> Self {
        Self { why_not: None }
    }

    /// Creates a disallowed result with the given reason.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Allowed;
    ///
    /// let not_allowed = Allowed::no("Operation not permitted");
    /// assert!(!not_allowed.is_allowed());
    /// ```
    pub fn no(why_not: impl Into<String>) -> Self {
        Self {
            why_not: Some(why_not.into()),
        }
    }

    /// Creates an allowed/disallowed result based on condition.
    ///
    /// # Arguments
    ///
    /// * `condition` - If true, creates allowed result; if false, creates disallowed
    /// * `why_not` - The reason if disallowed
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Allowed;
    ///
    /// let allowed = Allowed::with_condition(true, "Would fail");
    /// assert!(allowed.is_allowed());
    ///
    /// let not_allowed = Allowed::with_condition(false, "Permission denied");
    /// assert!(!not_allowed.is_allowed());
    /// assert_eq!(not_allowed.why_not(), "Permission denied");
    /// ```
    pub fn with_condition(condition: bool, why_not: impl Into<String>) -> Self {
        if condition {
            Self::yes()
        } else {
            Self::no(why_not)
        }
    }

    /// Returns true if the operation is allowed.
    pub fn is_allowed(&self) -> bool {
        self.why_not.is_none()
    }

    /// Returns the reason why the operation is not allowed.
    ///
    /// Returns an empty string if the operation is allowed.
    pub fn why_not(&self) -> &str {
        self.why_not.as_deref().unwrap_or("")
    }

    /// Returns true if allowed, otherwise fills the provided string with the reason.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Allowed;
    ///
    /// let not_allowed = Allowed::no("Read-only");
    /// let mut reason = String::new();
    /// if !not_allowed.is_allowed_with_reason(&mut reason) {
    ///     assert_eq!(reason, "Read-only");
    /// }
    /// ```
    pub fn is_allowed_with_reason(&self, reason: &mut String) -> bool {
        if let Some(ref why) = self.why_not {
            *reason = why.clone();
            false
        } else {
            true
        }
    }

    /// Converts to a Result type.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Allowed;
    ///
    /// let allowed = Allowed::yes();
    /// assert!(allowed.to_result().is_ok());
    ///
    /// let not_allowed = Allowed::no("Error");
    /// assert!(not_allowed.to_result().is_err());
    /// ```
    pub fn to_result(&self) -> Result<(), String> {
        match &self.why_not {
            None => Ok(()),
            Some(reason) => Err(reason.clone()),
        }
    }

    /// Converts into a Result type, consuming self.
    pub fn into_result(self) -> Result<(), String> {
        match self.why_not {
            None => Ok(()),
            Some(reason) => Err(reason),
        }
    }
}

impl From<bool> for Allowed {
    fn from(allowed: bool) -> Self {
        if allowed { Self::yes() } else { Self::no("") }
    }
}

impl From<(bool, &str)> for Allowed {
    fn from((condition, why_not): (bool, &str)) -> Self {
        Self::with_condition(condition, why_not)
    }
}

impl From<(bool, String)> for Allowed {
    fn from((condition, why_not): (bool, String)) -> Self {
        Self::with_condition(condition, why_not)
    }
}

impl From<Result<(), String>> for Allowed {
    fn from(result: Result<(), String>) -> Self {
        match result {
            Ok(()) => Self::yes(),
            Err(reason) => Self::no(reason),
        }
    }
}

impl From<Allowed> for bool {
    fn from(allowed: Allowed) -> Self {
        allowed.is_allowed()
    }
}

impl PartialEq for Allowed {
    fn eq(&self, other: &Self) -> bool {
        self.why_not == other.why_not
    }
}

impl Eq for Allowed {}

impl fmt::Display for Allowed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_allowed() {
            write!(f, "Allowed")
        } else {
            write!(f, "Not allowed: {}", self.why_not())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_yes() {
        let allowed = Allowed::yes();
        assert!(allowed.is_allowed());
        assert_eq!(allowed.why_not(), "");
    }

    #[test]
    fn test_no() {
        let not_allowed = Allowed::no("Test reason");
        assert!(!not_allowed.is_allowed());
        assert_eq!(not_allowed.why_not(), "Test reason");
    }

    #[test]
    fn test_with_condition() {
        let allowed = Allowed::with_condition(true, "Would fail");
        assert!(allowed.is_allowed());

        let not_allowed = Allowed::with_condition(false, "Failed");
        assert!(!not_allowed.is_allowed());
        assert_eq!(not_allowed.why_not(), "Failed");
    }

    #[test]
    fn test_default() {
        let allowed = Allowed::default();
        assert!(allowed.is_allowed());
    }

    #[test]
    fn test_is_allowed_with_reason() {
        let allowed = Allowed::yes();
        let mut reason = String::new();
        assert!(allowed.is_allowed_with_reason(&mut reason));
        assert!(reason.is_empty());

        let not_allowed = Allowed::no("Error message");
        assert!(!not_allowed.is_allowed_with_reason(&mut reason));
        assert_eq!(reason, "Error message");
    }

    #[test]
    fn test_to_result() {
        let allowed = Allowed::yes();
        assert!(allowed.to_result().is_ok());

        let not_allowed = Allowed::no("Error");
        assert_eq!(not_allowed.to_result(), Err("Error".to_string()));
    }

    #[test]
    fn test_into_result() {
        let allowed = Allowed::yes();
        assert!(allowed.into_result().is_ok());

        let not_allowed = Allowed::no("Error");
        assert_eq!(not_allowed.into_result(), Err("Error".to_string()));
    }

    #[test]
    fn test_from_bool() {
        let allowed: Allowed = true.into();
        assert!(allowed.is_allowed());

        let not_allowed: Allowed = false.into();
        assert!(!not_allowed.is_allowed());
    }

    #[test]
    fn test_from_tuple() {
        let allowed: Allowed = (true, "reason").into();
        assert!(allowed.is_allowed());

        let not_allowed: Allowed = (false, "reason").into();
        assert!(!not_allowed.is_allowed());
        assert_eq!(not_allowed.why_not(), "reason");
    }

    #[test]
    fn test_from_result() {
        let allowed: Allowed = Ok::<(), String>(()).into();
        assert!(allowed.is_allowed());

        let not_allowed: Allowed = Err("Error".to_string()).into();
        assert!(!not_allowed.is_allowed());
        assert_eq!(not_allowed.why_not(), "Error");
    }

    #[test]
    fn test_into_bool() {
        let allowed = Allowed::yes();
        assert!(bool::from(allowed));

        let not_allowed = Allowed::no("Error");
        assert!(!bool::from(not_allowed));
    }

    #[test]
    fn test_equality() {
        assert_eq!(Allowed::yes(), Allowed::yes());
        assert_eq!(Allowed::no("a"), Allowed::no("a"));
        assert_ne!(Allowed::yes(), Allowed::no("a"));
        assert_ne!(Allowed::no("a"), Allowed::no("b"));
    }

    #[test]
    fn test_display() {
        let allowed = Allowed::yes();
        assert_eq!(format!("{}", allowed), "Allowed");

        let not_allowed = Allowed::no("Test error");
        assert!(format!("{}", not_allowed).contains("Test error"));
    }
}
