//! Environment variable utilities — port of MaterialXFormat/Environ.h.

/// Get environment variable value.
/// Returns `""` when the variable is not set (matches C++ `getEnviron`).
pub fn get_environ(name: &str) -> String {
    std::env::var(name).unwrap_or_default()
}

/// Get environment variable as Option (Rust-idiomatic variant).
pub fn get_environ_opt(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

/// Set environment variable. Returns `true` on success (matches C++ `setEnviron`).
pub fn set_environ(name: &str, value: &str) -> bool {
    // SAFETY: single-threaded usage; no async signal handlers
    unsafe { std::env::set_var(name, value) };
    true
}

/// Remove environment variable. Returns `true` on success (matches C++ `removeEnviron`).
pub fn remove_environ(name: &str) -> bool {
    // SAFETY: single-threaded usage
    unsafe { std::env::remove_var(name) };
    true
}

pub const MATERIALX_SEARCH_PATH_ENV_VAR: &str = "MATERIALX_SEARCH_PATH";
