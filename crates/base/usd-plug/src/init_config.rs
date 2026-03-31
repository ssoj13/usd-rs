//! Bootstrap plugin search path configuration.
//!
//! Port of pxr/base/plug/initConfig.cpp
//!
//! Reads PXR_PLUGINPATH_NAME environment variable and builds
//! the ordered list of paths to search for plugInfo.json files.

use std::sync::OnceLock;

/// Environment variable name for plugin search paths.
///
/// Semicolon-separated on Windows, colon-separated on Unix.
const PLUGIN_PATH_ENV_VAR: &str = "PXR_PLUGINPATH_NAME";

/// Environment variable to disable standard plugin search.
const DISABLE_SEARCH_ENV_VAR: &str = "PXR_DISABLE_STANDARD_PLUG_SEARCH_PATH";

/// Stored plugin search paths and debug info.
struct PathsInfo {
    paths: Vec<String>,
    debug_messages: Vec<String>,
    paths_are_ordered: bool,
}

static PATHS_INFO: OnceLock<PathsInfo> = OnceLock::new();

/// Initialize plugin search paths from environment.
///
/// Called once at startup. Reads `PXR_PLUGINPATH_NAME` env var
/// and builds the search path list.
///
/// Matches C++ `Plug_InitConfig` (ARCH_CONSTRUCTOR).
fn init_paths() -> PathsInfo {
    let mut paths = Vec::new();
    let mut debug_messages = Vec::new();

    // Get binary path for resolving relative paths
    let binary_path = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_string_lossy().to_string()))
        .unwrap_or_default();

    debug_messages.push(format!(
        "Plug will search for plug infos under '{}'\n",
        binary_path
    ));

    // Read PXR_PLUGINPATH_NAME
    if let Ok(env_paths) = std::env::var(PLUGIN_PATH_ENV_VAR) {
        append_path_list(&mut paths, &env_paths, &binary_path);
    }

    PathsInfo {
        paths,
        debug_messages,
        paths_are_ordered: true,
    }
}

/// Split a path list string and resolve relative paths against base_path.
///
/// Matches C++ `_AppendPathList`.
fn append_path_list(result: &mut Vec<String>, paths_str: &str, base_path: &str) {
    let separator = if cfg!(windows) { ';' } else { ':' };

    for path in paths_str.split(separator) {
        let path = path.trim();
        if path.is_empty() {
            continue;
        }

        if std::path::Path::new(path).is_relative() {
            // Anchor relative paths to the binary path
            let mut full = std::path::PathBuf::from(base_path);
            full.push(path);
            let mut resolved = full.to_string_lossy().to_string();
            // Retain trailing / if present
            if path.ends_with('/') && !resolved.ends_with('/') {
                resolved.push('/');
            }
            result.push(resolved);
        } else {
            result.push(path.to_string());
        }
    }
}

/// Returns the configured plugin search paths.
///
/// Lazily initializes from environment on first call.
pub fn get_plugin_search_paths() -> &'static [String] {
    &PATHS_INFO.get_or_init(init_paths).paths
}

/// Returns true if standard plugin search is disabled.
///
/// Checks `PXR_DISABLE_STANDARD_PLUG_SEARCH_PATH` env var.
pub fn is_standard_search_disabled() -> bool {
    std::env::var(DISABLE_SEARCH_ENV_VAR)
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Returns debug messages from path initialization.
pub fn get_debug_messages() -> &'static [String] {
    &PATHS_INFO.get_or_init(init_paths).debug_messages
}

/// Returns whether search paths should be processed in order.
pub fn paths_are_ordered() -> bool {
    PATHS_INFO.get_or_init(init_paths).paths_are_ordered
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_append_path_list_absolute() {
        let mut paths = Vec::new();
        if cfg!(windows) {
            append_path_list(&mut paths, "C:/plugins;D:/more_plugins", "C:/app");
            assert_eq!(paths, vec!["C:/plugins", "D:/more_plugins"]);
        } else {
            append_path_list(&mut paths, "/usr/plugins:/opt/plugins", "/app");
            assert_eq!(paths, vec!["/usr/plugins", "/opt/plugins"]);
        }
    }

    #[test]
    fn test_append_path_list_relative() {
        let mut paths = Vec::new();
        append_path_list(&mut paths, "plugins", "/base/path");
        assert_eq!(paths.len(), 1);
        // Should be anchored to base_path
        assert!(paths[0].contains("plugins"));
        assert!(paths[0].starts_with("/base/path") || paths[0].contains("base"));
    }

    #[test]
    fn test_append_path_list_empty() {
        let mut paths = Vec::new();
        append_path_list(&mut paths, "", "/base");
        assert!(paths.is_empty());
    }
}
