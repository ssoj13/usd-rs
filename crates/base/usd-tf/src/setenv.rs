//! Environment variable setting utilities.
//!
//! Provides functions for setting and unsetting environment variables.
//!
//! # Examples
//!
//! ```
//! use usd_tf::setenv::{tf_setenv, tf_unsetenv};
//!
//! // Set an environment variable
//! tf_setenv("MY_VAR", "value");
//!
//! // Unset an environment variable
//! tf_unsetenv("MY_VAR");
//! ```

use std::env;

/// Set an environment variable.
///
/// Sets the environment variable `env_name` to `value`.
///
/// # Returns
///
/// Returns `true` on success, `false` on failure.
///
/// # Examples
///
/// ```
/// use usd_tf::setenv::tf_setenv;
///
/// tf_setenv("MY_VAR", "my_value");
/// ```
#[inline]
pub fn tf_setenv(env_name: &str, value: &str) -> bool {
    // In Rust, set_var doesn't return an error, so we always return true
    // unless we have an invalid name (empty or contains '=')
    if env_name.is_empty() || env_name.contains('=') {
        return false;
    }
    // SAFETY: set_var is unsafe in Rust 2024 because environment mutation is
    // not thread-safe w.r.t. C library calls. This mirrors the C++ TfSetenv
    // which also has the same caveat. Caller is responsible for synchronization.
    #[allow(unsafe_code)]
    unsafe {
        env::set_var(env_name, value)
    };
    true
}

/// Unset (remove) an environment variable.
///
/// Removes the environment variable `env_name` from the environment.
///
/// # Returns
///
/// Returns `true` on success, `false` on failure.
///
/// # Examples
///
/// ```
/// use usd_tf::setenv::tf_unsetenv;
///
/// tf_unsetenv("MY_VAR");
/// ```
#[inline]
pub fn tf_unsetenv(env_name: &str) -> bool {
    if env_name.is_empty() || env_name.contains('=') {
        return false;
    }
    // SAFETY: Same as tf_setenv - caller responsible for synchronization.
    #[allow(unsafe_code)]
    unsafe {
        env::remove_var(env_name)
    };
    true
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::getenv::tf_getenv;

    #[test]
    fn test_tf_setenv() {
        let var_name = "TF_TEST_SETENV_VAR";

        // Set the variable
        assert!(tf_setenv(var_name, "test_value"));

        // Verify it was set
        assert_eq!(tf_getenv(var_name, ""), "test_value");

        // Clean up
        tf_unsetenv(var_name);
    }

    #[test]
    fn test_tf_unsetenv() {
        let var_name = "TF_TEST_UNSETENV_VAR";

        // Set then unset
        tf_setenv(var_name, "temp_value");
        assert!(tf_unsetenv(var_name));

        // Verify it's gone
        assert_eq!(tf_getenv(var_name, "default"), "default");
    }

    #[test]
    fn test_tf_setenv_invalid_name() {
        // Empty name should fail
        assert!(!tf_setenv("", "value"));

        // Name with '=' should fail
        assert!(!tf_setenv("NAME=VALUE", "value"));
    }

    #[test]
    fn test_tf_unsetenv_invalid_name() {
        // Empty name should fail
        assert!(!tf_unsetenv(""));

        // Name with '=' should fail
        assert!(!tf_unsetenv("NAME=VALUE"));
    }

    #[test]
    fn test_tf_setenv_overwrite() {
        let var_name = "TF_TEST_OVERWRITE_VAR";

        // Set initial value
        tf_setenv(var_name, "initial");
        assert_eq!(tf_getenv(var_name, ""), "initial");

        // Overwrite
        tf_setenv(var_name, "updated");
        assert_eq!(tf_getenv(var_name, ""), "updated");

        // Clean up
        tf_unsetenv(var_name);
    }
}
