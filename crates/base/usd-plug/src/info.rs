//! plugInfo.json discovery and parsing.
//!
//! Port of pxr/base/plug/info.h/cpp
//!
//! Discovers and reads plugInfo.json files recursively, supporting
//! glob wildcards and include directives.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use usd_js::{JsObject, JsValue};

use crate::plugin::PluginType;

/// Parsed plugin registration metadata from a plugInfo.json entry.
///
/// Matches C++ `Plug_RegistrationMetadata`.
#[derive(Debug, Clone)]
pub struct RegistrationMetadata {
    pub plugin_type: PluginType,
    pub plugin_name: String,
    pub plugin_path: String,
    pub plug_info: JsObject,
    pub library_path: String,
    pub resource_path: String,
}

/// Reads multiple plugInfo paths, discovering and parsing plugins.
///
/// Matches C++ `Plug_ReadPlugInfo(pathnames, pathsAreOrdered, addVisitedPath, addPlugin, taskArena)`.
///
/// If `paths_are_ordered` is true, each path is fully processed before the next
/// (first path wins for duplicate plugin names).
pub fn read_plug_info(pathnames: &[String], paths_are_ordered: bool) -> Vec<RegistrationMetadata> {
    let mut visited = HashSet::new();
    let mut plugins = Vec::new();

    for pathname in pathnames {
        if pathname.is_empty() {
            continue;
        }

        // For convenience: directories without trailing "/" are treated as directories
        let pathname = if !pathname.ends_with('/')
            && !pathname.contains('*')
            && Path::new(pathname).is_dir()
        {
            format!("{}/", pathname)
        } else {
            pathname.clone()
        };

        read_plug_info_with_wildcards(&pathname, &mut visited, &mut plugins);

        if paths_are_ordered {
            // Process each path fully before moving to next — C++ uses taskArena.Wait()
        }
    }

    plugins
}

/// Handle a path that may contain wildcards.
///
/// Matches C++ `_ReadPlugInfoWithWildcards`.
fn read_plug_info_with_wildcards(
    pathname: &str,
    visited: &mut HashSet<String>,
    plugins: &mut Vec<RegistrationMetadata>,
) {
    if pathname.is_empty() {
        return;
    }

    // Check for wildcards
    if !pathname.contains('*') {
        // No wildcards — direct read
        read_single_plug_info(pathname, visited, plugins);
        return;
    }

    // Single * (no **) — use glob
    if !pathname.contains("**") {
        log::debug!("Globbing plugin info path {}", pathname);
        if let Ok(entries) = glob::glob(pathname) {
            for entry in entries.flatten() {
                let path_str = entry.to_string_lossy().to_string();
                read_single_plug_info(&path_str, visited, plugins);
            }
        }
        return;
    }

    // ** — recursive directory walk with regex matching
    // Find longest non-wildcarded prefix directory
    let normalized = pathname.replace('\\', "/");
    let star_pos = normalized.find('*').unwrap_or(normalized.len());
    let slash_pos = normalized[..star_pos].rfind('/').unwrap_or(0);

    let dirname = &normalized[..slash_pos];
    let pattern_suffix = &normalized[slash_pos + 1..];

    // Translate wildcard pattern to regex
    let regex_pattern = translate_wildcard_to_regex(pattern_suffix);

    // Append implied filename if pattern ends with /
    let full_pattern = if regex_pattern.ends_with('/') || pattern_suffix.ends_with('/') {
        format!("{}/{}{}", dirname, regex_pattern, PLUG_INFO_NAME)
    } else {
        format!("{}/{}", dirname, regex_pattern)
    };

    let re = match regex::Regex::new(&full_pattern) {
        Ok(r) => r,
        Err(err) => {
            log::error!(
                "Failed to compile regex for {}: {} ({})",
                pathname,
                full_pattern,
                err
            );
            return;
        }
    };

    // Walk filesystem recursively
    traverse_directory(dirname, &re, visited, plugins);
}

const PLUG_INFO_NAME: &str = "plugInfo.json";

/// Read a single plugInfo.json file (or directory path).
///
/// Matches C++ `_ReadPlugInfo`.
fn read_single_plug_info(
    pathname: &str,
    visited: &mut HashSet<String>,
    plugins: &mut Vec<RegistrationMetadata>,
) {
    if pathname.is_empty() {
        return;
    }

    // Append default filename if path ends with /
    let pathname = if pathname.ends_with('/') {
        format!("{}{}", pathname, PLUG_INFO_NAME)
    } else {
        pathname.to_string()
    };

    // Prevent redundant reads and infinite recursion
    if !visited.insert(pathname.clone()) {
        log::debug!("Ignoring already read plugin info {}", pathname);
        return;
    }

    log::debug!("Will read plugin info {}", pathname);

    // Read and parse
    let top = match read_plug_info_object(&pathname) {
        Some(obj) => obj,
        None => return,
    };

    // Process "Plugins" array
    if let Some(plugins_val) = top.get("Plugins") {
        if let Some(arr) = plugins_val.as_array() {
            for (index, entry) in arr.iter().enumerate() {
                if let Some(metadata) = parse_plugin_entry(entry, &pathname, "Plugins", index) {
                    plugins.push(metadata);
                }
            }
        } else {
            log::error!(
                "Plugin info file {} key 'Plugins' doesn't hold an array",
                pathname
            );
        }
    }

    // Process "Includes" array
    if let Some(includes_val) = top.get("Includes") {
        if let Some(arr) = includes_val.as_array() {
            for (index, entry) in arr.iter().enumerate() {
                if let Some(include_path) = entry.as_string() {
                    let resolved = merge_paths(&pathname, include_path, true);
                    read_plug_info_with_wildcards(&resolved, visited, plugins);
                } else {
                    log::error!(
                        "Plugin info file {} key 'Includes' index {} doesn't hold a string",
                        pathname,
                        index
                    );
                }
            }
        } else {
            log::error!(
                "Plugin info file {} key 'Includes' doesn't hold an array",
                pathname
            );
        }
    }

    // Warn about unexpected top-level keys
    for key in top.keys() {
        if key != "Plugins" && key != "Includes" {
            log::error!("Plugin info file {} has unknown key {}", pathname, key);
        }
    }
}

/// Read and parse a plugInfo.json file, stripping # comments.
///
/// Matches C++ `_ReadPlugInfoObject`.
fn read_plug_info_object(pathname: &str) -> Option<JsObject> {
    let content = match std::fs::read_to_string(pathname) {
        Ok(c) => c,
        Err(_) => {
            log::debug!("Failed to open plugin info {}", pathname);
            return None;
        }
    };

    // Strip # comments: C++ clears entire lines where # appears before
    // any non-whitespace/non-# content. It does NOT strip inline comments.
    // Matches C++ info.cpp lines 159-165:
    //   if (line.find('#') < line.find_first_not_of(" \t#"))
    //       line.clear();
    let filtered: Vec<&str> = content
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();
            if trimmed.starts_with('#') { "" } else { line }
        })
        .collect();

    let json_str = filtered.join("\n");

    match usd_js::parse_string(&json_str) {
        Ok(value) => {
            if let Some(obj) = value.as_object() {
                // C++ special case: if no "Includes" or "Plugins" keys,
                // treat the whole object as if it were in a "Plugins" array
                if !obj.contains_key("Includes") && !obj.contains_key("Plugins") {
                    let mut wrapper = JsObject::new();
                    wrapper.insert(
                        "Plugins".to_string(),
                        JsValue::Array(vec![JsValue::Object(obj.clone())]),
                    );
                    Some(wrapper)
                } else {
                    Some(obj.clone())
                }
            } else {
                log::error!(
                    "Plugin info file {} did not contain a JSON object",
                    pathname
                );
                None
            }
        }
        Err(err) => {
            log::error!(
                "Plugin info file {} couldn't be read (line {}, col {}): {}",
                pathname,
                err.line,
                err.column,
                err.reason
            );
            None
        }
    }
}

/// Parse a single plugin entry from the "Plugins" array.
///
/// Matches C++ `Plug_RegistrationMetadata::Plug_RegistrationMetadata(JsValue, ...)`.
fn parse_plugin_entry(
    value: &JsValue,
    pathname: &str,
    key: &str,
    index: usize,
) -> Option<RegistrationMetadata> {
    let location = format!("file {} {}[{}]", pathname, key, index);
    let top_info = match value.as_object() {
        Some(obj) => obj,
        None => {
            log::error!(
                "Plugin info {} doesn't hold an object; plugin ignored",
                location
            );
            return None;
        }
    };

    // Parse Type (required)
    let plugin_type = match top_info.get("Type").and_then(|v| v.as_string()) {
        Some("library") => PluginType::Library,
        Some("resource") => PluginType::Resource,
        Some(other) => {
            log::error!(
                "Plugin info {} key 'Type' has invalid value '{}'; plugin ignored",
                location,
                other
            );
            return None;
        }
        None => {
            log::error!(
                "Plugin info {} key 'Type' is missing or not a string; plugin ignored",
                location
            );
            return None;
        }
    };

    // Parse Name (required)
    let plugin_name = match top_info.get("Name").and_then(|v| v.as_string()) {
        Some(name) if !name.is_empty() => name.to_string(),
        _ => {
            log::error!(
                "Plugin info {} key 'Name' is missing or empty; plugin ignored",
                location
            );
            return None;
        }
    };

    // Parse Root (optional, defaults to dirname of plugInfo.json)
    let plugin_path = match top_info.get("Root").and_then(|v| v.as_string()) {
        Some(root) => {
            let merged = merge_paths(pathname, root, false);
            if merged.is_empty() {
                log::error!(
                    "Plugin info {} key 'Root' doesn't hold a valid path; plugin ignored",
                    location
                );
                return None;
            }
            merged
        }
        None => get_dirname(pathname).to_string(),
    };

    // Parse LibraryPath (required for library type, relative to plugin_path)
    let library_path = match top_info.get("LibraryPath").and_then(|v| v.as_string()) {
        Some(lp) if !lp.is_empty() => append_to_root_path(&plugin_path, lp),
        Some(_) => String::new(), // empty string is valid (monolithic/static builds)
        None => {
            if plugin_type == PluginType::Library {
                log::error!(
                    "Plugin info {} key 'LibraryPath' is missing; plugin ignored",
                    location
                );
                return None;
            }
            String::new()
        }
    };

    // Parse ResourcePath (optional, relative to plugin_path)
    let resource_path = match top_info.get("ResourcePath").and_then(|v| v.as_string()) {
        Some(rp) => {
            let resolved = append_to_root_path(&plugin_path, rp);
            if resolved.is_empty() {
                log::error!(
                    "Plugin info {} key 'ResourcePath' doesn't hold a valid path; plugin ignored",
                    location
                );
                return None;
            }
            resolved
        }
        None => get_dirname(&plugin_path).to_string(),
    };

    // Parse Info (required)
    let plug_info = match top_info.get("Info").and_then(|v| v.as_object()) {
        Some(obj) => obj.clone(),
        None => {
            log::error!(
                "Plugin info {} key 'Info' is missing or not an object; plugin ignored",
                location
            );
            return None;
        }
    };

    // Warn about unexpected keys
    for sub_key in top_info.keys() {
        if !matches!(
            sub_key.as_str(),
            "Type" | "Name" | "Info" | "Root" | "LibraryPath" | "ResourcePath"
        ) {
            log::error!(
                "Plugin info {}: ignoring unknown key '{}'",
                location,
                sub_key
            );
        }
    }

    Some(RegistrationMetadata {
        plugin_type,
        plugin_name,
        plugin_path,
        plug_info,
        library_path,
        resource_path,
    })
}

/// Join dirname(owner_pathname) with sub_pathname.
///
/// Matches C++ `_MergePaths`.
fn merge_paths(owner_pathname: &str, sub_pathname: &str, keep_trailing_slash: bool) -> String {
    if sub_pathname.is_empty() {
        return String::new();
    }

    // Return absolute path as-is
    if Path::new(sub_pathname).is_absolute() {
        return sub_pathname.to_string();
    }

    // Join dirname of owner with sub
    let dir = get_dirname(owner_pathname);
    let mut result = PathBuf::from(dir);
    result.push(sub_pathname);

    let mut result_str = result.to_string_lossy().replace('\\', "/");

    // Retain trailing slash if requested
    if keep_trailing_slash && sub_pathname.ends_with('/') && !result_str.ends_with('/') {
        result_str.push('/');
    }

    result_str
}

/// Join root_pathname with sub_pathname. Returns absolute sub_pathname as-is.
///
/// Matches C++ `_AppendToRootPath`.
fn append_to_root_path(root_pathname: &str, sub_pathname: &str) -> String {
    if sub_pathname.is_empty() {
        return root_pathname.to_string();
    }

    if Path::new(sub_pathname).is_absolute() {
        return sub_pathname.to_string();
    }

    let mut result = PathBuf::from(root_pathname);
    result.push(sub_pathname);
    result.to_string_lossy().replace('\\', "/")
}

/// Get directory part of a path.
fn get_dirname(path: &str) -> &str {
    match path.rfind('/').or_else(|| path.rfind('\\')) {
        Some(pos) => &path[..pos],
        None => ".",
    }
}

/// Translate wildcard pattern to regex.
///
/// Matches C++ `_TranslateWildcardToRegex`.
fn translate_wildcard_to_regex(wildcard: &str) -> String {
    let mut result = String::with_capacity(wildcard.len() * 2);
    let chars: Vec<char> = wildcard.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        match chars[i] {
            '.' | '[' | ']' => {
                result.push('\\');
                result.push(chars[i]);
            }
            '*' => {
                if i + 1 < chars.len() && chars[i + 1] == '*' {
                    // ** => match anything including /
                    result.push_str(".*");
                    i += 1; // skip next *
                } else {
                    // * => match anything except /
                    result.push_str("[^/]*");
                }
            }
            c => result.push(c),
        }
        i += 1;
    }

    result
}

/// Recursively traverse directory tree looking for paths matching regex.
///
/// Matches C++ `_TraverseDirectory`.
fn traverse_directory(
    dirname: &str,
    regex: &regex::Regex,
    visited: &mut HashSet<String>,
    plugins: &mut Vec<RegistrationMetadata>,
) {
    let entries = match std::fs::read_dir(dirname) {
        Ok(e) => e,
        Err(_) => return,
    };

    let mut subdirs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let path_str = path.to_string_lossy().replace('\\', "/");

        if path.is_file() && regex.is_match(&path_str) {
            read_single_plug_info(&path_str, visited, plugins);
            // C++ terminates recursion for this directory on first match
            return;
        }

        if path.is_dir() {
            subdirs.push(path_str);
        }
    }

    for subdir in subdirs {
        traverse_directory(&subdir, regex, visited, plugins);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_paths_relative() {
        assert_eq!(
            merge_paths("/a/b/plugInfo.json", "subdir/other.json", false),
            "/a/b/subdir/other.json"
        );
    }

    #[test]
    fn test_merge_paths_absolute() {
        assert_eq!(
            merge_paths("/a/b/plugInfo.json", "/absolute/path.json", false),
            "/absolute/path.json"
        );
    }

    #[test]
    fn test_merge_paths_trailing_slash() {
        let result = merge_paths("/a/b/plugInfo.json", "subdir/", true);
        assert!(result.ends_with('/'));
    }

    #[test]
    fn test_append_to_root_path() {
        assert_eq!(
            append_to_root_path("/root/dir", "lib/plugin.so"),
            "/root/dir/lib/plugin.so"
        );
        assert_eq!(
            append_to_root_path("/root/dir", "/absolute/plugin.so"),
            "/absolute/plugin.so"
        );
        assert_eq!(append_to_root_path("/root/dir", ""), "/root/dir");
    }

    #[test]
    fn test_get_dirname() {
        assert_eq!(get_dirname("/a/b/file.json"), "/a/b");
        assert_eq!(get_dirname("file.json"), ".");
        assert_eq!(get_dirname("/single"), "");
    }

    #[test]
    fn test_translate_wildcard_to_regex() {
        assert_eq!(translate_wildcard_to_regex("*.json"), "[^/]*\\.json");
        assert_eq!(
            translate_wildcard_to_regex("**/plugInfo.json"),
            ".*/plugInfo\\.json"
        );
        assert_eq!(translate_wildcard_to_regex("a/b/c"), "a/b/c");
    }

    #[test]
    fn test_strip_comments() {
        // Verify the comment stripping works via read_plug_info_object
        // by testing the helper behavior
        let json_with_comments = r#"{
            # This is a comment
            "Plugins": [
                {
                    "Type": "resource",
                    "Name": "test",
                    "Info": {
                        # Another comment
                        "key": "value"
                    }
                }
            ]
        }"#;

        // Write to temp file and read
        let dir = std::env::temp_dir().join("usd_plug_test_comments");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("plugInfo.json");
        std::fs::write(&path, json_with_comments).unwrap();

        let result = read_plug_info_object(&path.to_string_lossy());
        assert!(result.is_some());
        let obj = result.unwrap();
        assert!(obj.contains_key("Plugins"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_parse_plugin_entry_resource() {
        let json_str = r#"{
            "Type": "resource",
            "Name": "testPlugin",
            "Info": {
                "Kinds": {
                    "myKind": { "baseKind": "model" }
                }
            }
        }"#;
        let value = usd_js::parse_string(json_str).unwrap();
        let result = parse_plugin_entry(&value, "/test/plugInfo.json", "Plugins", 0);
        assert!(result.is_some());
        let meta = result.unwrap();
        assert_eq!(meta.plugin_name, "testPlugin");
        assert_eq!(meta.plugin_type, PluginType::Resource);
        assert!(meta.plug_info.contains_key("Kinds"));
    }

    #[test]
    fn test_parse_plugin_entry_missing_name() {
        let json_str = r#"{ "Type": "resource", "Info": {} }"#;
        let value = usd_js::parse_string(json_str).unwrap();
        let result = parse_plugin_entry(&value, "/test/plugInfo.json", "Plugins", 0);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_plugin_entry_missing_info() {
        let json_str = r#"{ "Type": "resource", "Name": "test" }"#;
        let value = usd_js::parse_string(json_str).unwrap();
        let result = parse_plugin_entry(&value, "/test/plugInfo.json", "Plugins", 0);
        assert!(result.is_none());
    }

    #[test]
    fn test_read_plug_info_from_dir() {
        let dir = std::env::temp_dir().join("usd_plug_test_read");
        std::fs::create_dir_all(&dir).unwrap();

        let plug_info = r#"{
            "Plugins": [
                {
                    "Type": "resource",
                    "Name": "testFromDir",
                    "Info": { "Kinds": { "custom_kind": { "baseKind": "model" } } }
                }
            ]
        }"#;
        std::fs::write(dir.join("plugInfo.json"), plug_info).unwrap();

        let dir_str = dir.to_string_lossy().to_string();
        let results = read_plug_info(&[dir_str], true);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].plugin_name, "testFromDir");

        std::fs::remove_dir_all(&dir).ok();
    }
}
