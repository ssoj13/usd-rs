//! Shader Metadata Helpers - Utilities for parsing shader metadata.
//!
//! Port of pxr/usd/sdr/shaderMetadataHelpers.h
//!
//! This module provides utilities for parsing metadata contained within shaders,
//! including boolean extraction, string/token parsing, option vectors, and
//! property role detection.

use super::declare::{SdrOptionVec, SdrStringVec, SdrTokenMap, SdrTokenVec};
use super::shader_property::SdrShaderProperty;
use super::shader_property_metadata::SdrShaderPropertyMetadata;
use super::tokens::tokens;
use usd_tf::Token;
use usd_vt::Value;

// Private tokens for widget/renderType detection
const WIDGET_FILENAME: &str = "filename";
const WIDGET_FILE_INPUT: &str = "fileInput";
const WIDGET_ASSET_ID_INPUT: &str = "assetIdInput";
const RENDER_TYPE_TERMINAL: &str = "terminal";

/// Determines if the given metadatum in the metadata dictionary has a truthy value.
///
/// All values are considered true except the following (case-insensitive):
/// '0', 'false', and 'f'. The absence of `key` in the metadata also evaluates to false.
///
/// # Deprecated
/// Prefer using SdrShaderNodeMetadata::get_* and SdrShaderPropertyMetadata::get_*
/// methods on bool metadata.
pub fn is_truthy(key: &Token, metadata: &SdrTokenMap) -> bool {
    match metadata.get(key) {
        None => false,
        Some(value) => {
            if value.is_empty() {
                // Presence without value implies true
                return true;
            }
            let lower = value.to_lowercase();
            !matches!(lower.as_str(), "0" | "false" | "f")
        }
    }
}

/// Extracts the string value from the given metadatum if it exists,
/// otherwise returns the default value.
///
/// # Deprecated
/// Prefer using SdrShaderNodeMetadata::get_* and SdrShaderPropertyMetadata::get_*
/// methods on string metadata.
pub fn string_val(key: &Token, metadata: &SdrTokenMap, default_value: &str) -> String {
    metadata
        .get(key)
        .cloned()
        .unwrap_or_else(|| default_value.to_string())
}

/// Extracts the tokenized value from the given metadatum if it exists,
/// otherwise returns the default value.
///
/// # Deprecated
/// Prefer using SdrShaderNodeMetadata::get_* and SdrShaderPropertyMetadata::get_*
/// methods on TfToken metadata.
pub fn token_val(key: &Token, metadata: &SdrTokenMap, default_value: &Token) -> Token {
    metadata
        .get(key)
        .map(|s| Token::new(s))
        .unwrap_or_else(|| default_value.clone())
}

/// Extracts the int value from the given metadatum if it exists and is a
/// valid integer value, otherwise returns the default value.
///
/// # Deprecated
/// Prefer using SdrShaderNodeMetadata::get_* and SdrShaderPropertyMetadata::get_*
/// methods on int metadata.
pub fn int_val(key: &Token, metadata: &SdrTokenMap, default_value: i32) -> i32 {
    metadata
        .get(key)
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(default_value)
}

/// Extracts a vector of strings from the given metadatum.
/// An empty vector is returned if the metadatum does not exist.
///
/// Values are split by the pipe character '|'.
///
/// # Deprecated
/// Prefer using SdrShaderNodeMetadata::get_* and SdrShaderPropertyMetadata::get_*
/// methods on SdrStringVec metadata.
pub fn string_vec_val(key: &Token, metadata: &SdrTokenMap) -> SdrStringVec {
    metadata
        .get(key)
        .map(|s| s.split('|').map(String::from).collect())
        .unwrap_or_default()
}

/// Extracts a vector of tokenized values from the given metadatum.
/// An empty vector is returned if the metadatum does not exist.
///
/// Values are split by the pipe character '|'.
///
/// # Deprecated
/// Prefer using SdrShaderNodeMetadata::get_* and SdrShaderPropertyMetadata::get_*
/// methods on SdrTokenVec metadata.
pub fn token_vec_val(key: &Token, metadata: &SdrTokenMap) -> SdrTokenVec {
    string_vec_val(key, metadata)
        .into_iter()
        .map(|s| Token::new(&s))
        .collect()
}

/// Extracts an "options" vector from the given string.
///
/// The input string should be formatted as one of the following:
/// - list:   "option1|option2|option3|..."
/// - mapper: "key1:value1|key2:value2|..."
///
/// If it's a mapper, returns the result as a list of key-value tuples to
/// preserve order.
pub fn option_vec_val(option_str: &str) -> SdrOptionVec {
    let mut options = SdrOptionVec::new();

    for token in option_str.split('|') {
        if let Some(colon_pos) = token.find(':') {
            // Mapper format: key:value
            let key = &token[..colon_pos];
            let value = &token[colon_pos + 1..];
            options.push((Token::new(key), Token::new(value)));
        } else {
            // List format: just the option
            options.push((Token::new(token), Token::default()));
        }
    }

    options
}

/// Serializes a vector of strings into a string using the pipe character
/// as the delimiter.
///
/// # Deprecated
/// Prefer using SdrShaderNodeMetadata::set_* and SdrShaderPropertyMetadata::set_*
/// methods on SdrStringVec metadata.
pub fn create_string_from_string_vec(string_vec: &SdrStringVec) -> String {
    string_vec.join("|")
}

/// Determines if the specified property metadata has a widget that
/// indicates the property is an asset identifier.
///
/// # Deprecated
/// Prefer using SdrShaderPropertyMetadata::get_is_asset_identifier().
pub fn is_property_an_asset_identifier(metadata: &SdrTokenMap) -> bool {
    if let Some(widget) = metadata.get(&tokens().property_metadata.widget) {
        let widget_lower = widget.to_lowercase();
        return widget_lower == WIDGET_FILENAME
            || widget_lower == WIDGET_FILE_INPUT.to_lowercase()
            || widget_lower == WIDGET_ASSET_ID_INPUT.to_lowercase();
    }
    false
}

/// Determines if the specified property metadata has a 'renderType' that
/// indicates the property should be a Terminal.
pub fn is_property_a_terminal(metadata: &SdrTokenMap) -> bool {
    if let Some(render_type) = metadata.get(&tokens().property_metadata.render_type) {
        // If the property is a Terminal, then the renderType value will be
        // "terminal <terminalName>", where <terminalName> is the specific
        // kind of terminal.
        return render_type.to_lowercase().starts_with(RENDER_TYPE_TERMINAL);
    }
    false
}

/// Gets the "role" from metadata if one is provided.
///
/// Only returns a value if it's a valid role as defined by SdrPropertyRole tokens.
pub fn get_role_from_metadata(metadata: &SdrShaderPropertyMetadata) -> Token {
    if metadata.has_role() {
        let role = metadata.get_role();
        // Check if role is valid (in SdrPropertyRole->allTokens)
        // For now, return the role if present
        if !role.is_empty() {
            // Valid roles: "none", "color", "normal", "point", "vector", "textureCoordinate"
            let valid_roles = [
                "none",
                "color",
                "normal",
                "point",
                "vector",
                "textureCoordinate",
            ];
            if valid_roles.contains(&role.as_str()) {
                return Token::new(&role);
            }
        }
    }
    Token::default()
}

/// Parses the VtValue from the given valueStr according to the sdf type
/// expressed by the given property via two steps.
///
/// 1. valueStr is preprocessed into a suitable input for the sdf value parser.
/// 2. sdf value parsing is performed on the preprocessed result.
///
/// If parsing fails, returns None and populates the error string.
pub fn parse_sdf_value(
    value_str: &str,
    property: &SdrShaderProperty,
    err: &mut String,
) -> Option<Value> {
    let _indicator = property.get_type_as_sdf_type();
    let sdr_type = property.get_type();

    // Normalize the string based on property type
    let normalized = normalize_value_string(value_str, property);

    // Try to parse the normalized value
    // Note: Full implementation would use Sdf value parsing
    // For now, we handle basic types
    parse_value_from_string(&normalized, sdr_type, err)
}

/// Normalizes a value string for parsing based on property type.
fn normalize_value_string(value_str: &str, property: &SdrShaderProperty) -> String {
    let trimmed = value_str.trim();
    let sdr_type = property.get_type();
    let sdr_type_str = sdr_type.as_str();

    // Check if array-like or tuple-like
    if property.is_dynamic_array() || sdr_type_str == "vector" {
        format!("[{}]", trimmed)
    } else if property.is_array()
        || sdr_type_str == "color"
        || sdr_type_str == "color4"
        || sdr_type_str == "point"
        || sdr_type_str == "normal"
    {
        format!("({})", trimmed)
    } else {
        // For string/token/asset, would quote appropriately
        trimmed.to_string()
    }
}

/// Parses a value from string based on type.
fn parse_value_from_string(value_str: &str, sdr_type: &Token, err: &mut String) -> Option<Value> {
    let type_str = sdr_type.as_str();

    match type_str {
        "int" => value_str
            .trim()
            .parse::<i32>()
            .map(Value::from)
            .map_err(|e| *err = format!("Failed to parse int: {}", e))
            .ok(),
        "float" => value_str
            .trim()
            .parse::<f64>()
            .map(Value::from)
            .map_err(|e| *err = format!("Failed to parse float: {}", e))
            .ok(),
        "string" | "token" => Some(Value::from(value_str.to_string())),
        "bool" => {
            let lower = value_str.to_lowercase();
            match lower.as_str() {
                "true" | "1" | "yes" => Some(Value::from(true)),
                "false" | "0" | "no" => Some(Value::from(false)),
                _ => {
                    *err = format!("Failed to parse bool: {}", value_str);
                    None
                }
            }
        }
        _ => {
            // For complex types, return string representation
            // Full implementation would parse vectors, colors, etc.
            Some(Value::from(value_str.to_string()))
        }
    }
}

/// Synthesizes a "shownIf" expression from conditional visibility metadata
/// in the property, expressed according to Katana's "args" format.
///
/// The sibling properties should be provided in `all_properties` and will
/// be referenced when resolving relative paths and when parsing embedded
/// property values.
pub fn compute_shown_if_from_property_metadata(
    property: &SdrShaderProperty,
    all_properties: &[Box<SdrShaderProperty>],
    shader_uri: &str,
) -> String {
    let metadata = property.get_metadata();
    let base_path = build_base_path(property);

    extract_expression(
        metadata,
        "conditionalVis",
        &base_path,
        all_properties,
        shader_uri,
    )
}

/// Synthesizes a "shownIf" expression from conditional visibility metadata
/// associated with a page name.
pub fn compute_shown_if_from_page_metadata(
    metadata: &SdrTokenMap,
    page_name: &str,
    properties: &[Box<SdrShaderProperty>],
    shader_uri: &str,
) -> String {
    let base_path = page_name.replace(':', "/");
    extract_expression(
        metadata,
        "conditionalVis",
        &base_path,
        properties,
        shader_uri,
    )
}

/// Builds the base path from a property's page and implementation name.
fn build_base_path(property: &SdrShaderProperty) -> String {
    let page = property.get_page();
    let mut base_path = String::new();

    if !page.is_empty() {
        // Convert page from namespaced identifier to path
        let page_parts: Vec<&str> = page.as_str().split(':').collect();
        base_path = page_parts.join("/");
        base_path.push('/');
    }

    base_path.push_str(property.get_implementation_name().as_str());
    base_path
}

/// Extracts an expression from metadata at the given prefix.
fn extract_expression(
    metadata: &SdrTokenMap,
    prefix: &str,
    base_path: &str,
    _all_properties: &[Box<SdrShaderProperty>],
    _shader_uri: &str,
) -> String {
    // Look for the operator
    let op_key = Token::new(&format!("{}Op", prefix));
    let op_value = match metadata.get(&op_key) {
        Some(v) => v.clone(),
        None => return String::new(),
    };

    // Check if it's a boolean operator (and/or)
    match op_value.to_lowercase().as_str() {
        "and" | "or" => {
            // Boolean operator - get left and right branches
            let left_key = Token::new(&format!("{}Left", prefix));
            let right_key = Token::new(&format!("{}Right", prefix));

            let left_prefix = metadata.get(&left_key).cloned().unwrap_or_default();
            let right_prefix = metadata.get(&right_key).cloned().unwrap_or_default();

            if left_prefix.is_empty() || right_prefix.is_empty() {
                return String::new();
            }

            let lhs = extract_expression(
                metadata,
                &left_prefix,
                base_path,
                _all_properties,
                _shader_uri,
            );
            let rhs = extract_expression(
                metadata,
                &right_prefix,
                base_path,
                _all_properties,
                _shader_uri,
            );

            if lhs.is_empty() || rhs.is_empty() {
                return String::new();
            }

            // Match OpenUSD `SdfBooleanExpression` text (`&&` / `||`), not Katana op names.
            let joiner = if op_value.to_lowercase() == "or" {
                " || "
            } else {
                " && "
            };
            format!("{lhs}{joiner}{rhs}")
        }
        "equalto"
        | "notequalto"
        | "greaterthan"
        | "lessthan"
        | "greaterthanorequalto"
        | "lessthanorequalto" => {
            // Comparison operator - get path and value
            let path_key = Token::new(&format!("{}Path", prefix));
            let value_key = Token::new(&format!("{}Value", prefix));

            let path = metadata.get(&path_key).cloned().unwrap_or_default();
            let value = metadata.get(&value_key).cloned().unwrap_or_default();

            if path.is_empty() || value.is_empty() {
                return String::new();
            }

            // Map operator to symbol
            let op_symbol = match op_value.to_lowercase().as_str() {
                "equalto" => "==",
                "notequalto" => "!=",
                "greaterthan" => ">",
                "lessthan" => "<",
                "greaterthanorequalto" => ">=",
                "lessthanorequalto" => "<=",
                _ => "==",
            };

            // Resolve the path to a property name
            // For now, use the path directly
            let resolved_name = resolve_property_path(&path, base_path);
            format!("{} {} {}", resolved_name, op_symbol, value)
        }
        _ => String::new(),
    }
}

/// Resolves a relative property path to an absolute property name.
fn resolve_property_path(path: &str, base_path: &str) -> String {
    // Construct full path
    let full_path = format!("{}/{}", base_path, path);

    // Normalize (handle ../)
    let parts: Vec<&str> = full_path.split('/').collect();
    let mut normalized = Vec::new();

    for part in parts {
        match part {
            ".." => {
                normalized.pop();
            }
            "." | "" => {}
            _ => normalized.push(part),
        }
    }

    // Return the last component as the property name
    normalized.last().map(|s| s.to_string()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_truthy() {
        let key = Token::new("enabled");
        let mut metadata = SdrTokenMap::new();

        // Absent key is false
        assert!(!is_truthy(&key, &metadata));

        // Present with no value is true
        metadata.insert(key.clone(), String::new());
        assert!(is_truthy(&key, &metadata));

        // False values
        metadata.insert(key.clone(), "0".to_string());
        assert!(!is_truthy(&key, &metadata));

        metadata.insert(key.clone(), "false".to_string());
        assert!(!is_truthy(&key, &metadata));

        metadata.insert(key.clone(), "FALSE".to_string());
        assert!(!is_truthy(&key, &metadata));

        metadata.insert(key.clone(), "f".to_string());
        assert!(!is_truthy(&key, &metadata));

        // True values
        metadata.insert(key.clone(), "1".to_string());
        assert!(is_truthy(&key, &metadata));

        metadata.insert(key.clone(), "true".to_string());
        assert!(is_truthy(&key, &metadata));

        metadata.insert(key.clone(), "yes".to_string());
        assert!(is_truthy(&key, &metadata));
    }

    #[test]
    fn test_option_vec_val() {
        // List format
        let options = option_vec_val("a|b|c");
        assert_eq!(options.len(), 3);
        assert_eq!(options[0].0.as_str(), "a");
        assert!(options[0].1.is_empty());

        // Mapper format
        let options = option_vec_val("key1:value1|key2:value2");
        assert_eq!(options.len(), 2);
        assert_eq!(options[0].0.as_str(), "key1");
        assert_eq!(options[0].1.as_str(), "value1");
        assert_eq!(options[1].0.as_str(), "key2");
        assert_eq!(options[1].1.as_str(), "value2");
    }

    #[test]
    fn test_string_vec_val() {
        let key = Token::new("tags");
        let mut metadata = SdrTokenMap::new();
        metadata.insert(key.clone(), "a|b|c".to_string());

        let vec = string_vec_val(&key, &metadata);
        assert_eq!(vec.len(), 3);
        assert_eq!(vec[0], "a");
        assert_eq!(vec[1], "b");
        assert_eq!(vec[2], "c");
    }

    #[test]
    fn test_create_string_from_string_vec() {
        let vec = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        assert_eq!(create_string_from_string_vec(&vec), "a|b|c");
    }

    #[test]
    fn test_resolve_property_path() {
        assert_eq!(
            resolve_property_path("../../Advanced/traceLightPaths", "Shadows/enableShadows"),
            "traceLightPaths"
        );
    }
}
