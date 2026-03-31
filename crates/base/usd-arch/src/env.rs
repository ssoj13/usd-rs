// SAFETY: This module provides FFI bindings to system APIs requiring unsafe
#![allow(unsafe_code)]

//! Environment variable utilities.
//!
//! Provides cross-platform functions for reading and writing environment variables.

use std::env;
use std::ffi::OsStr;

/// Gets the value of an environment variable.
///
/// Returns `None` if the variable is not set or contains invalid UTF-8.
///
/// # Examples
///
/// ```
/// use usd_arch::get_env;
///
/// if let Some(path) = get_env("PATH") {
///     println!("PATH = {}", path);
/// }
/// ```
#[must_use]
pub fn get_env(name: &str) -> Option<String> {
    env::var(name).ok()
}

/// Gets the value of an environment variable, returning a default if not set.
///
/// # Examples
///
/// ```
/// use usd_arch::get_env_or;
///
/// let home = get_env_or("HOME", "/tmp");
/// ```
#[must_use]
pub fn get_env_or(name: &str, default: &str) -> String {
    env::var(name).unwrap_or_else(|_| default.to_string())
}

/// Sets an environment variable (always overwrites if present).
///
/// Equivalent to `ArchSetEnv(name, value, true)`.
///
/// # Platform Notes
///
/// On Unix, this affects only the current process and its children.
/// On Windows, this also only affects the current process.
///
/// # Examples
///
/// ```
/// use usd_arch::{set_env, get_env};
///
/// set_env("MY_VAR", "my_value");
/// assert_eq!(get_env("MY_VAR"), Some("my_value".to_string()));
/// ```
pub fn set_env(name: &str, value: &str) {
    let _ = set_env_with_overwrite(name, value, true);
}

/// Sets an environment variable with optional overwrite.
///
/// Equivalent to C++ `ArchSetEnv(name, value, overwrite)`.
/// Returns `true` on success, `false` on failure (e.g. if overwrite is false
/// and the variable already exists).
///
/// # Examples
///
/// ```
/// use usd_arch::set_env_with_overwrite;
///
/// // Only set if not already present
/// set_env_with_overwrite("MY_VAR", "value", false);
///
/// // Always overwrite
/// set_env_with_overwrite("MY_VAR", "new_value", true);
/// ```
#[must_use]
pub fn set_env_with_overwrite(name: &str, value: &str, overwrite: bool) -> bool {
    set_env_with_overwrite_impl(name, value, overwrite)
}

#[cfg(unix)]
fn set_env_with_overwrite_impl(name: &str, value: &str, overwrite: bool) -> bool {
    use std::ffi::CString;
    let name_c = match CString::new(name) {
        Ok(c) => c,
        Err(_) => return false,
    };
    let value_c = match CString::new(value) {
        Ok(c) => c,
        Err(_) => return false,
    };
    // setenv(name, value, overwrite): 1 = overwrite, 0 = do not overwrite
    unsafe { libc::setenv(name_c.as_ptr(), value_c.as_ptr(), overwrite as libc::c_int) == 0 }
}

#[cfg(windows)]
fn set_env_with_overwrite_impl(name: &str, value: &str, overwrite: bool) -> bool {
    use std::os::windows::ffi::OsStrExt;
    if !overwrite && has_env(name) {
        return true; // Already exists, "success" per C++ behavior
    }
    let name_wide: Vec<u16> = OsStr::new(name).encode_wide().chain(Some(0)).collect();
    let value_wide: Vec<u16> = OsStr::new(value).encode_wide().chain(Some(0)).collect();
    unsafe {
        windows_sys::Win32::System::Environment::SetEnvironmentVariableW(
            name_wide.as_ptr(),
            value_wide.as_ptr(),
        ) != 0
    }
}

/// Sets an environment variable with OsStr values.
///
/// This is useful when dealing with paths or other OS-specific strings.
pub fn set_env_os(name: &OsStr, value: &OsStr) {
    // SAFETY: Same as set_env - only used in initialization/test contexts.
    unsafe { env::set_var(name, value) };
}

/// Removes an environment variable.
///
/// Equivalent to C++ `ArchRemoveEnv`. Returns `true` on success.
///
/// # Examples
///
/// ```
/// use usd_arch::{set_env, unset_env, get_env};
///
/// set_env("MY_VAR", "value");
/// unset_env("MY_VAR");
/// assert_eq!(get_env("MY_VAR"), None);
/// ```
#[must_use]
pub fn unset_env(name: &str) -> bool {
    unset_env_impl(name)
}

#[cfg(unix)]
fn unset_env_impl(name: &str) -> bool {
    use std::ffi::CString;
    let name_c = match CString::new(name) {
        Ok(c) => c,
        Err(_) => return false,
    };
    unsafe { libc::unsetenv(name_c.as_ptr()) == 0 }
}

#[cfg(windows)]
fn unset_env_impl(name: &str) -> bool {
    use std::os::windows::ffi::OsStrExt;
    let name_wide: Vec<u16> = OsStr::new(name).encode_wide().chain(Some(0)).collect();
    unsafe {
        windows_sys::Win32::System::Environment::SetEnvironmentVariableW(
            name_wide.as_ptr(),
            std::ptr::null::<u16>(),
        ) != 0
    }
}

/// Checks if an environment variable is set.
///
/// # Examples
///
/// ```
/// use usd_arch::has_env;
///
/// if has_env("DEBUG") {
///     println!("Debug mode enabled");
/// }
/// ```
#[must_use]
pub fn has_env(name: &str) -> bool {
    env::var_os(name).is_some()
}

/// Gets an environment variable as a boolean.
///
/// Returns `true` if the variable is set to "1", "true", "yes", or "on" (case-insensitive).
/// Returns `false` otherwise (including if the variable is not set).
///
/// # Examples
///
/// ```
/// use usd_arch::{set_env, get_env_bool};
///
/// set_env("DEBUG", "true");
/// assert!(get_env_bool("DEBUG"));
///
/// set_env("DEBUG", "0");
/// assert!(!get_env_bool("DEBUG"));
/// ```
#[must_use]
pub fn get_env_bool(name: &str) -> bool {
    match env::var(name) {
        Ok(val) => {
            let val = val.to_lowercase();
            val == "1" || val == "true" || val == "yes" || val == "on"
        }
        Err(_) => false,
    }
}

/// Gets an environment variable as an integer.
///
/// Returns `None` if the variable is not set or cannot be parsed as an integer.
///
/// # Examples
///
/// ```
/// use usd_arch::{set_env, get_env_int};
///
/// set_env("PORT", "8080");
/// assert_eq!(get_env_int("PORT"), Some(8080));
///
/// set_env("PORT", "invalid");
/// assert_eq!(get_env_int("PORT"), None);
/// ```
#[must_use]
pub fn get_env_int(name: &str) -> Option<i64> {
    env::var(name).ok()?.parse().ok()
}

/// Gets an environment variable as an unsigned integer.
///
/// Returns `None` if the variable is not set or cannot be parsed.
#[must_use]
pub fn get_env_uint(name: &str) -> Option<u64> {
    env::var(name).ok()?.parse().ok()
}

/// Gets an environment variable as a floating-point number.
///
/// Returns `None` if the variable is not set or cannot be parsed.
#[must_use]
pub fn get_env_float(name: &str) -> Option<f64> {
    env::var(name).ok()?.parse().ok()
}

/// Expands environment variables in a string.
///
/// Supports `$VAR` and `${VAR}` syntax on Unix, and `%VAR%` on Windows.
///
/// # Examples
///
/// ```
/// use usd_arch::{set_env, expand_env_vars};
///
/// set_env("USER", "alice");
/// let expanded = expand_env_vars("Hello, $USER!");
/// assert!(expanded.contains("alice") || expanded.contains("$USER"));
/// ```
#[must_use]
pub fn expand_env_vars(s: &str) -> String {
    let mut result = s.to_string();

    // Handle ${VAR} syntax
    while let Some(start) = result.find("${") {
        if let Some(end) = result[start..].find('}') {
            let var_name = &result[start + 2..start + end];
            let value = env::var(var_name).unwrap_or_default();
            result = format!(
                "{}{}{}",
                &result[..start],
                value,
                &result[start + end + 1..]
            );
        } else {
            break;
        }
    }

    // Handle $VAR syntax (Unix-style)
    let mut i = 0;
    while i < result.len() {
        if result.as_bytes()[i] == b'$' && i + 1 < result.len() && result.as_bytes()[i + 1] != b'{'
        {
            // Find the end of the variable name
            let start = i + 1;
            let mut end = start;
            while end < result.len() {
                let c = result.as_bytes()[end];
                if c.is_ascii_alphanumeric() || c == b'_' {
                    end += 1;
                } else {
                    break;
                }
            }
            if end > start {
                let var_name = &result[start..end];
                let value = env::var(var_name).unwrap_or_default();
                result = format!("{}{}{}", &result[..i], value, &result[end..]);
                i += value.len();
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    // Handle %VAR% syntax (Windows-style)
    #[cfg(target_os = "windows")]
    {
        while let Some(start) = result.find('%') {
            if let Some(end) = result[start + 1..].find('%') {
                let var_name = &result[start + 1..start + 1 + end];
                if !var_name.is_empty() {
                    let value = env::var(var_name).unwrap_or_default();
                    result = format!(
                        "{}{}{}",
                        &result[..start],
                        value,
                        &result[start + end + 2..]
                    );
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }

    result
}

/// Returns an iterator over all environment variables.
///
/// # Examples
///
/// ```
/// use usd_arch::env_vars;
///
/// for (key, value) in env_vars() {
///     println!("{} = {}", key, value);
/// }
/// ```
pub fn env_vars() -> impl Iterator<Item = (String, String)> {
    env::vars()
}

/// Expands environment variable references in `s`, mirroring `ArchExpandEnvironmentVariables`.
///
/// On Windows (matching C++ behavior) expands `%VAR%` patterns.
/// On all platforms also expands `${VAR}` and `$VAR` patterns.
/// Variables that are not set are replaced with an empty string.
///
/// # Examples
///
/// ```
/// use usd_arch::{set_env, expand_environment_variables};
///
/// set_env("MY_LIB", "/usr/lib");
/// let s = expand_environment_variables("path=${MY_LIB}/foo");
/// assert_eq!(s, "path=/usr/lib/foo");
/// ```
#[must_use]
pub fn expand_environment_variables(s: &str) -> String {
    let mut result = s.to_string();

    // --- Windows: %VAR% (primary C++ behavior on Windows) ---
    #[cfg(windows)]
    {
        loop {
            // Find the first '%'
            let Some(start) = result.find('%') else { break };
            // Find the closing '%' after it
            let Some(end_rel) = result[start + 1..].find('%') else {
                break;
            };
            let end = start + 1 + end_rel;
            let var_name = &result[start + 1..end];
            if var_name.is_empty() {
                // %% — not a variable, stop to avoid infinite loop
                break;
            }
            let value = env::var(var_name).unwrap_or_default();
            result = format!("{}{}{}", &result[..start], value, &result[end + 1..]);
        }
    }

    // --- ${VAR} syntax (Unix primary / cross-platform secondary) ---
    loop {
        let Some(start) = result.find("${") else {
            break;
        };
        let Some(end_rel) = result[start..].find('}') else {
            break;
        };
        let end = start + end_rel; // index of '}'
        let var_name = &result[start + 2..end];
        let value = env::var(var_name).unwrap_or_default();
        result = format!("{}{}{}", &result[..start], value, &result[end + 1..]);
    }

    // --- $VAR syntax: Windows-only (C++ Unix only expands ${VAR}) ---
    #[cfg(windows)]
    {
        let bytes = result.as_bytes();
        let mut i = 0;
        let mut out = String::with_capacity(result.len());
        while i < bytes.len() {
            let next = if i + 1 < bytes.len() { bytes[i + 1] } else { 0 };
            if bytes[i] == b'$' && (next.is_ascii_alphabetic() || next == b'_') {
                let name_start = i + 1;
                let mut name_end = name_start;
                while name_end < bytes.len()
                    && (bytes[name_end].is_ascii_alphanumeric() || bytes[name_end] == b'_')
                {
                    name_end += 1;
                }
                if name_end > name_start {
                    // SAFETY: bytes are ASCII-validated above; result is valid UTF-8
                    let var_name =
                        unsafe { std::str::from_utf8_unchecked(&bytes[name_start..name_end]) };
                    let value = env::var(var_name).unwrap_or_default();
                    out.push_str(&value);
                    i = name_end;
                } else {
                    out.push('$');
                    i += 1;
                }
            } else {
                out.push(bytes[i] as char);
                i += 1;
            }
        }
        result = out;
    }

    result
}

/// Returns all current environment variables as a `Vec` of `(key, value)` pairs.
///
/// Mirrors `ArchEnviron()` which returns the raw `environ` pointer; here we
/// return an owned collection of valid UTF-8 pairs (non-UTF-8 names/values are
/// silently skipped, matching the behavior callers expect on modern systems).
///
/// # Examples
///
/// ```
/// use usd_arch::environ;
///
/// let vars = environ();
/// assert!(!vars.is_empty());
/// ```
#[must_use]
pub fn environ() -> Vec<(String, String)> {
    env::vars().collect()
}

#[cfg(test)]
#[allow(unused_must_use)]
mod tests {
    use super::*;

    #[test]
    fn test_get_set_env() {
        let var_name = "USD_RS_TEST_VAR";
        set_env(var_name, "test_value");
        assert_eq!(get_env(var_name), Some("test_value".to_string()));
        unset_env(var_name);
        assert_eq!(get_env(var_name), None);
    }

    #[test]
    fn test_get_env_or() {
        let var_name = "USD_RS_TEST_VAR_DEFAULT";
        unset_env(var_name);
        assert_eq!(get_env_or(var_name, "default"), "default");
        set_env(var_name, "actual");
        assert_eq!(get_env_or(var_name, "default"), "actual");
        unset_env(var_name);
    }

    #[test]
    fn test_has_env() {
        let var_name = "USD_RS_TEST_HAS_VAR";
        unset_env(var_name);
        assert!(!has_env(var_name));
        set_env(var_name, "");
        assert!(has_env(var_name));
        unset_env(var_name);
    }

    #[test]
    fn test_get_env_bool() {
        let var_name = "USD_RS_TEST_BOOL";

        for (value, expected) in &[
            ("1", true),
            ("true", true),
            ("TRUE", true),
            ("yes", true),
            ("YES", true),
            ("on", true),
            ("ON", true),
            ("0", false),
            ("false", false),
            ("no", false),
            ("off", false),
            ("", false),
            ("invalid", false),
        ] {
            set_env(var_name, value);
            assert_eq!(
                get_env_bool(var_name),
                *expected,
                "Failed for value: {}",
                value
            );
        }

        unset_env(var_name);
        assert!(!get_env_bool(var_name));
    }

    #[test]
    fn test_get_env_int() {
        let var_name = "USD_RS_TEST_INT";

        set_env(var_name, "42");
        assert_eq!(get_env_int(var_name), Some(42));

        set_env(var_name, "-100");
        assert_eq!(get_env_int(var_name), Some(-100));

        set_env(var_name, "invalid");
        assert_eq!(get_env_int(var_name), None);

        unset_env(var_name);
        assert_eq!(get_env_int(var_name), None);
    }

    #[test]
    fn test_expand_env_vars() {
        let var_name = "USD_RS_TEST_EXPAND";
        set_env(var_name, "world");

        let expanded = expand_env_vars("Hello, ${USD_RS_TEST_EXPAND}!");
        assert_eq!(expanded, "Hello, world!");

        let expanded2 = expand_env_vars("Hello, $USD_RS_TEST_EXPAND!");
        assert_eq!(expanded2, "Hello, world!");

        unset_env(var_name);
    }

    // --- expand_environment_variables tests ---

    #[test]
    fn test_expand_evars_braced() {
        set_env("USD_RS_EEV_A", "hello");
        assert_eq!(expand_environment_variables("${USD_RS_EEV_A}"), "hello");
        assert_eq!(
            expand_environment_variables("pre_${USD_RS_EEV_A}_suf"),
            "pre_hello_suf"
        );
        unset_env("USD_RS_EEV_A");
    }

    #[cfg(windows)]
    #[test]
    fn test_expand_evars_bare_dollar() {
        // Bare $VAR expansion is Windows-only (C++ Unix only expands ${VAR})
        set_env("USD_RS_EEV_B", "world");
        assert_eq!(expand_environment_variables("$USD_RS_EEV_B"), "world");
        assert_eq!(
            expand_environment_variables("say $USD_RS_EEV_B!"),
            "say world!"
        );
        unset_env("USD_RS_EEV_B");
    }

    #[cfg(not(windows))]
    #[test]
    fn test_expand_evars_bare_dollar_unix() {
        // On Unix, bare $VAR is NOT expanded (C++ only expands ${VAR})
        set_env("USD_RS_EEV_B", "world");
        assert_eq!(
            expand_environment_variables("$USD_RS_EEV_B"),
            "$USD_RS_EEV_B"
        );
        unset_env("USD_RS_EEV_B");
    }

    #[test]
    fn test_expand_evars_unset_replaced_with_empty() {
        // Unset var must become empty string for ${VAR}
        unset_env("USD_RS_EEV_UNSET_XYZ");
        assert_eq!(expand_environment_variables("${USD_RS_EEV_UNSET_XYZ}"), "");
        // Bare $VAR: only expanded on Windows
        #[cfg(windows)]
        assert_eq!(expand_environment_variables("$USD_RS_EEV_UNSET_XYZ"), "");
        #[cfg(not(windows))]
        assert_eq!(
            expand_environment_variables("$USD_RS_EEV_UNSET_XYZ"),
            "$USD_RS_EEV_UNSET_XYZ"
        );
    }

    #[test]
    fn test_expand_evars_multiple() {
        set_env("USD_RS_EEV_X", "foo");
        set_env("USD_RS_EEV_Y", "bar");
        assert_eq!(
            expand_environment_variables("${USD_RS_EEV_X}/${USD_RS_EEV_Y}"),
            "foo/bar"
        );
        // Mixed braced + bare: bare $VAR only expanded on Windows
        #[cfg(windows)]
        assert_eq!(
            expand_environment_variables("${USD_RS_EEV_X}/$USD_RS_EEV_Y"),
            "foo/bar"
        );
        #[cfg(not(windows))]
        assert_eq!(
            expand_environment_variables("${USD_RS_EEV_X}/$USD_RS_EEV_Y"),
            "foo/$USD_RS_EEV_Y"
        );
        unset_env("USD_RS_EEV_X");
        unset_env("USD_RS_EEV_Y");
    }

    #[test]
    fn test_expand_evars_lone_dollar_passthrough() {
        // '$' followed by a digit or at end -> pass through on all platforms
        assert_eq!(expand_environment_variables("cost: $5"), "cost: $5");
        assert_eq!(expand_environment_variables("end$"), "end$");
        // $b: expanded on Windows only
        #[cfg(windows)]
        assert_eq!(expand_environment_variables("a$b c"), "a c");
        #[cfg(not(windows))]
        assert_eq!(expand_environment_variables("a$b c"), "a$b c");
    }

    #[cfg(windows)]
    #[test]
    fn test_expand_evars_percent_windows() {
        set_env("USD_RS_EEV_W", "winval");
        assert_eq!(expand_environment_variables("%USD_RS_EEV_W%"), "winval");
        assert_eq!(expand_environment_variables("x=%USD_RS_EEV_W%"), "x=winval");
        unset_env("USD_RS_EEV_W");
    }

    // --- environ tests ---

    #[test]
    fn test_environ_nonempty() {
        let vars = environ();
        assert!(!vars.is_empty(), "environ() must return at least one entry");
    }

    #[test]
    fn test_environ_contains_set_var() {
        set_env("USD_RS_EEV_ENV", "sentinel");
        let vars = environ();
        let found = vars
            .iter()
            .any(|(k, v)| k == "USD_RS_EEV_ENV" && v == "sentinel");
        assert!(found, "environ() must include freshly-set variable");
        unset_env("USD_RS_EEV_ENV");
    }
}
