//! Environment variable access utilities.
//!
//! Provides functions for reading environment variables with default values,
//! type conversion, and consistent behavior across platforms.
//!
//! # Examples
//!
//! ```
//! use usd_tf::getenv::{tf_getenv, tf_getenv_int, tf_getenv_bool, tf_getenv_double};
//!
//! // Get a string environment variable
//! let path = tf_getenv("MY_PATH", "/default/path");
//!
//! // Get an integer environment variable
//! let count = tf_getenv_int("MY_COUNT", 10);
//!
//! // Get a boolean environment variable
//! let enabled = tf_getenv_bool("MY_FEATURE_ENABLED", false);
//!
//! // Get a double environment variable
//! let threshold = tf_getenv_double("MY_THRESHOLD", 0.5);
//! ```

use std::env;

/// Resolve an env variable: check real env first, then file overrides.
fn resolve_env(name: &str) -> Option<String> {
    // Real env takes priority (matches C++ overwrite=false semantics)
    if let Ok(val) = env::var(name) {
        if !val.is_empty() {
            return Some(val);
        }
    }
    // Fall back to file overrides from PIXAR_TF_ENV_SETTING_FILE
    crate::env_setting::get_file_override(name).map(|s| s.to_string())
}

/// Return an environment variable as a string.
///
/// Returns the value of the environment variable `env_name` as a string.
/// If the variable is unset or empty, returns `default_value`.
///
/// # Examples
///
/// ```
/// use usd_tf::getenv::tf_getenv;
///
/// let home = tf_getenv("HOME", "/home/default");
/// ```
#[inline]
pub fn tf_getenv(env_name: &str, default_value: &str) -> String {
    resolve_env(env_name).unwrap_or_else(|| default_value.to_string())
}

/// Return an environment variable as an integer.
///
/// Returns the value of the environment variable `env_name` as an integer.
/// If the variable is unset, empty, or cannot be parsed as an integer,
/// returns `default_value`.
///
/// # Examples
///
/// ```
/// use usd_tf::getenv::tf_getenv_int;
///
/// let port = tf_getenv_int("MY_PORT", 8080);
/// ```
#[inline]
pub fn tf_getenv_int(env_name: &str, default_value: i32) -> i32 {
    resolve_env(env_name)
        .and_then(|v| v.parse().ok())
        .unwrap_or(default_value)
}

/// Return an environment variable as a boolean.
///
/// Returns the value of the environment variable `env_name` as a boolean.
/// If the variable is unset or empty, returns `default_value`.
///
/// A value of `true` is returned if the environment variable is any of:
/// "true", "yes", "on", or "1" (case-insensitive).
/// All other values yield `false`.
///
/// # Examples
///
/// ```
/// use usd_tf::getenv::tf_getenv_bool;
///
/// let debug = tf_getenv_bool("DEBUG_MODE", false);
/// ```
#[inline]
pub fn tf_getenv_bool(env_name: &str, default_value: bool) -> bool {
    match resolve_env(env_name) {
        Some(val) => {
            let lower = val.to_lowercase();
            matches!(lower.as_str(), "true" | "yes" | "on" | "1")
        }
        None => default_value,
    }
}

/// Return an environment variable as a double.
///
/// Returns the value of the environment variable `env_name` as a double.
/// If the variable is unset, empty, or cannot be parsed as a double,
/// returns `default_value`.
///
/// # Examples
///
/// ```
/// use usd_tf::getenv::tf_getenv_double;
///
/// let scale = tf_getenv_double("SCALE_FACTOR", 1.0);
/// ```
#[inline]
pub fn tf_getenv_double(env_name: &str, default_value: f64) -> f64 {
    resolve_env(env_name)
        .and_then(|v| v.parse().ok())
        .unwrap_or(default_value)
}

/// Check if an environment variable is set (non-empty).
///
/// Returns `true` if the variable exists and has a non-empty value.
#[inline]
pub fn tf_has_env(env_name: &str) -> bool {
    resolve_env(env_name).is_some()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tf_getenv_default() {
        // Use a variable that definitely doesn't exist
        let result = tf_getenv("TF_TEST_NONEXISTENT_VAR_12345", "default_val");
        assert_eq!(result, "default_val");
    }

    #[test]
    fn test_tf_getenv_existing() {
        // PATH should exist on all systems
        let result = tf_getenv("PATH", "");
        assert!(!result.is_empty());
    }

    #[test]
    fn test_tf_getenv_int_default() {
        let result = tf_getenv_int("TF_TEST_NONEXISTENT_VAR_12345", 42);
        assert_eq!(result, 42);
    }

    #[test]
    fn test_tf_getenv_bool_default() {
        let result = tf_getenv_bool("TF_TEST_NONEXISTENT_VAR_12345", true);
        assert!(result);

        let result = tf_getenv_bool("TF_TEST_NONEXISTENT_VAR_12345", false);
        assert!(!result);
    }

    #[test]
    fn test_tf_getenv_double_default() {
        let result = tf_getenv_double("TF_TEST_NONEXISTENT_VAR_12345", 3.14);
        assert!((result - 3.14).abs() < f64::EPSILON);
    }

    #[test]
    fn test_tf_has_env() {
        // PATH should exist
        assert!(tf_has_env("PATH"));
        // This shouldn't exist
        assert!(!tf_has_env("TF_TEST_NONEXISTENT_VAR_12345"));
    }
}
