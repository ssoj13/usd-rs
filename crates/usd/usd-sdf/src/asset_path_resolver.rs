//! Asset path resolution utilities.
//!
//! Port of pxr/usd/sdf/assetPathResolver.h
//!
//! Provides functions for resolving asset paths, managing layer identifiers,
//! and working with anonymous layers.

use crate::Layer;
use std::collections::HashMap;

/// Container for layer asset information.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AssetInfo {
    /// The layer identifier.
    pub identifier: String,
    /// The resolved filesystem path.
    pub resolved_path: String,
    /// Additional asset info metadata.
    pub asset_info: HashMap<String, String>,
}

/// Separator used to delimit file format arguments in identifiers.
pub const FORMAT_ARGS_SEPARATOR: &str = ":SDF_FORMAT_ARGS:";

/// Template prefix for anonymous layer identifiers.
const ANON_LAYER_PREFIX: &str = "anon:";

/// Returns true if `identifier` can be used to create a new layer.
pub fn can_create_new_layer(identifier: &str) -> Result<bool, String> {
    if identifier.is_empty() {
        return Err("Empty identifier".to_string());
    }
    if is_anon_layer_identifier(identifier) {
        return Err("Cannot create layer with anonymous identifier".to_string());
    }
    Ok(true)
}

/// Resolves a layer path to a filesystem path.
///
/// Returns the resolved path if an asset exists at that path, with
/// file format arguments stripped. Returns None if no asset exists.
pub fn resolve_path(layer_path: &str) -> Option<String> {
    let stripped = strip_format_args(layer_path);
    let path = std::path::Path::new(&stripped);
    if path.exists() { Some(stripped) } else { None }
}

/// Computes a file path for creating a new layer.
///
/// Returns a path where the new layer should be created.
pub fn compute_file_path(layer_path: &str) -> Option<String> {
    let stripped = strip_format_args(layer_path);
    if stripped.is_empty() {
        return None;
    }
    Some(stripped)
}

/// Returns true if a layer can be written to the given resolved path.
pub fn can_write_layer_to_path(resolved_path: &str) -> bool {
    if resolved_path.is_empty() {
        return false;
    }
    let path = std::path::Path::new(resolved_path);
    // Check if parent directory exists and is writable.
    if let Some(parent) = path.parent() {
        parent.exists()
    } else {
        false
    }
}

/// Returns true if `identifier` is an anonymous layer identifier.
pub fn is_anon_layer_identifier(identifier: &str) -> bool {
    identifier.starts_with(ANON_LAYER_PREFIX)
}

/// Returns the display name portion of an anonymous layer identifier.
///
/// This is the identifier tag, if present, or the empty string.
pub fn get_anon_layer_display_name(identifier: &str) -> String {
    if !is_anon_layer_identifier(identifier) {
        return String::new();
    }
    // Format: "anon:<hex_addr>:<tag>" or "anon:<hex_addr>"
    let after_prefix = &identifier[ANON_LAYER_PREFIX.len()..];
    if let Some(colon_pos) = after_prefix.find(':') {
        after_prefix[colon_pos + 1..].to_string()
    } else {
        String::new()
    }
}

/// Returns the anonymous layer identifier template for the given tag.
pub fn get_anon_layer_identifier_template(tag: &str) -> String {
    if tag.is_empty() {
        format!("{}%p", ANON_LAYER_PREFIX)
    } else {
        format!("{}%p:{}", ANON_LAYER_PREFIX, tag)
    }
}

/// Computes an anonymous layer identifier from a template.
///
/// Replaces %p with the layer's pointer address.
pub fn compute_anon_layer_identifier(template: &str, layer: &Layer) -> String {
    let addr = std::ptr::addr_of!(*layer) as usize;
    template.replace("%p", &format!("{:016X}", addr))
}

/// If `identifier` contains file format arguments, strips them and
/// returns the stripped identifier. Otherwise returns None.
pub fn strip_identifier_arguments(identifier: &str) -> Option<String> {
    if let Some(pos) = identifier.find(FORMAT_ARGS_SEPARATOR) {
        Some(identifier[..pos].to_string())
    } else {
        None
    }
}

/// Splits an identifier into a layer path and an arguments string.
///
/// Returns (layer_path, arguments) or None if no arguments present.
pub fn split_identifier(identifier: &str) -> (String, String) {
    if let Some(pos) = identifier.find(FORMAT_ARGS_SEPARATOR) {
        (identifier[..pos].to_string(), identifier[pos..].to_string())
    } else {
        (identifier.to_string(), String::new())
    }
}

/// Splits an identifier into a layer path and a map of arguments.
pub fn split_identifier_args(identifier: &str) -> (String, HashMap<String, String>) {
    let (path, args_str) = split_identifier(identifier);
    let mut args = HashMap::new();

    if !args_str.is_empty() {
        // Format: ":SDF_FORMAT_ARGS:key=value&key2=value2"
        let pairs = &args_str[FORMAT_ARGS_SEPARATOR.len()..];
        for pair in pairs.split('&') {
            if let Some(eq_pos) = pair.find('=') {
                let key = pair[..eq_pos].to_string();
                let value = pair[eq_pos + 1..].to_string();
                args.insert(key, value);
            }
        }
    }

    (path, args)
}

/// Joins a layer path and arguments string into an identifier.
pub fn create_identifier(layer_path: &str, arguments: &str) -> String {
    if arguments.is_empty() {
        layer_path.to_string()
    } else {
        format!("{}{}", layer_path, arguments)
    }
}

/// Joins a layer path and arguments map into an identifier.
pub fn create_identifier_from_args(layer_path: &str, args: &HashMap<String, String>) -> String {
    if args.is_empty() {
        return layer_path.to_string();
    }
    // Sort keys for deterministic output matching C++ std::map ordering
    let mut sorted_keys: Vec<&String> = args.keys().collect();
    sorted_keys.sort();
    let args_str: String = sorted_keys
        .iter()
        .map(|k| format!("{}={}", k, args[k.as_str()]))
        .collect::<Vec<_>>()
        .join("&");
    format!("{}{}{}", layer_path, FORMAT_ARGS_SEPARATOR, args_str)
}

/// Returns true if the identifier contains file format arguments.
pub fn identifier_contains_arguments(identifier: &str) -> bool {
    identifier.contains(FORMAT_ARGS_SEPARATOR)
}

/// Returns the display name for a layer.
///
/// For anonymous layers, returns the anonymous display name.
/// For regular layers, returns the filename portion.
pub fn get_layer_display_name(identifier: &str) -> String {
    if is_anon_layer_identifier(identifier) {
        return get_anon_layer_display_name(identifier);
    }
    let stripped = strip_format_args(identifier);
    std::path::Path::new(&stripped)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&stripped)
        .to_string()
}

/// Returns the extension of the given identifier, used to identify
/// the associated file format.
pub fn get_extension(identifier: &str) -> String {
    let stripped = strip_format_args(identifier);
    std::path::Path::new(&stripped)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_string()
}

/// Returns true if the given layer is a package layer or packaged
/// within a package layer.
pub fn is_package_or_packaged_layer(layer: &Layer) -> bool {
    let identifier = layer.identifier();
    identifier.ends_with(".usdz") || identifier.contains(".usdz[") || identifier.contains(".usdz/")
}

/// Strips file format arguments from an identifier.
fn strip_format_args(identifier: &str) -> String {
    if let Some(pos) = identifier.find(FORMAT_ARGS_SEPARATOR) {
        identifier[..pos].to_string()
    } else {
        identifier.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_anon_layer_identifier() {
        assert!(is_anon_layer_identifier("anon:0x1234"));
        assert!(is_anon_layer_identifier("anon:0x1234:myTag"));
        assert!(!is_anon_layer_identifier("layer.usda"));
    }

    #[test]
    fn test_get_anon_display_name() {
        assert_eq!(get_anon_layer_display_name("anon:0x1234:myTag"), "myTag");
        assert_eq!(get_anon_layer_display_name("anon:0x1234"), "");
    }

    #[test]
    fn test_split_identifier() {
        let (path, args) = split_identifier("foo.usda:SDF_FORMAT_ARGS:a=b&c=d");
        assert_eq!(path, "foo.usda");
        assert_eq!(args, ":SDF_FORMAT_ARGS:a=b&c=d");
    }

    #[test]
    fn test_split_identifier_args() {
        let (path, args) = split_identifier_args("foo.usda:SDF_FORMAT_ARGS:a=b&c=d");
        assert_eq!(path, "foo.usda");
        assert_eq!(args.get("a"), Some(&"b".to_string()));
        assert_eq!(args.get("c"), Some(&"d".to_string()));
    }

    #[test]
    fn test_identifier_contains_arguments() {
        assert!(identifier_contains_arguments(
            "foo.usda:SDF_FORMAT_ARGS:a=b"
        ));
        assert!(!identifier_contains_arguments("foo.usda"));
    }

    #[test]
    fn test_get_extension() {
        assert_eq!(get_extension("model.usda"), "usda");
        assert_eq!(get_extension("model.usdc:SDF_FORMAT_ARGS:a=b"), "usdc");
    }

    #[test]
    fn test_get_layer_display_name() {
        assert_eq!(get_layer_display_name("/path/to/model.usda"), "model.usda");
        assert_eq!(get_layer_display_name("anon:0x1234:myLayer"), "myLayer");
    }
}
