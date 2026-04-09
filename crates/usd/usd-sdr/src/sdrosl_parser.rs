//! SdrOsl Parser Plugin - Parses .sdrOsl JSON shader definition files.
//!
//! SdrOsl files are JSON-based shader definitions that can be generated
//! from OSL (Open Shading Language) shaders or authored directly.
//!
//! # Format
//!
//! ```json
//! {
//!     "name": "MyShader",
//!     "context": "pattern",
//!     "sourceType": "OSL",
//!     "help": "A custom shader",
//!     "inputs": [
//!         {
//!             "name": "diffuseColor",
//!             "type": "color",
//!             "default": [0.5, 0.5, 0.5],
//!             "help": "The diffuse color"
//!         }
//!     ],
//!     "outputs": [
//!         {
//!             "name": "out",
//!             "type": "color"
//!         }
//!     ],
//!     "metadata": {
//!         "category": "texture",
//!         "departments": ["lighting", "lookdev"]
//!     }
//! }
//! ```
//!
//! # Usage
//!
//! This format is useful for:
//! - Pre-parsed OSL shaders (avoiding OSL library dependency)
//! - Custom shader definitions without source files
//! - Testing and prototyping

use super::declare::{SdrOptionVec, SdrTokenMap, SdrTokenVec};
use super::discovery_result::SdrShaderNodeDiscoveryResult;
use super::parser_plugin::{SdrParserPlugin, get_invalid_shader_node};
use super::shader_node::{SdrShaderNode, SdrShaderNodeUniquePtr};
use super::shader_node_metadata::SdrShaderNodeMetadata;
use super::shader_property::SdrShaderProperty;
use super::shader_property_metadata::SdrShaderPropertyMetadata;
use super::tokens::tokens;
use usd_tf::Token;
use usd_vt::Value;

/// Source type identifier for SdrOsl shaders.
pub const SDROSL_SOURCE_TYPE: &str = "OSL";

/// Discovery type for SdrOsl files.
pub const SDROSL_DISCOVERY_TYPE: &str = "sdrOsl";

/// Alternative discovery type.
pub const SDROSL_DISCOVERY_TYPE_ALT: &str = "oso";

/// Parser plugin for .sdrOsl JSON files.
///
/// Parses JSON-based shader definitions that represent OSL shaders
/// without requiring the full OSL library.
pub struct SdrOslParserPlugin {
    discovery_types: SdrTokenVec,
    source_type: Token,
}

impl Default for SdrOslParserPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl SdrOslParserPlugin {
    /// Creates a new SdrOsl parser plugin.
    pub fn new() -> Self {
        Self {
            // Only JSON `.sdrOsl` — compiled `.oso` bytecode is handled by `osl_parser::OslParserPlugin`
            // (same as C++: one plugin per discovery type; "oso" must not map to JSON parsing).
            discovery_types: vec![Token::new(SDROSL_DISCOVERY_TYPE)],
            source_type: Token::new(SDROSL_SOURCE_TYPE),
        }
    }

    /// Parses the JSON content of an sdrOsl file.
    fn parse_json(
        &self,
        content: &str,
        discovery_result: &SdrShaderNodeDiscoveryResult,
    ) -> Option<SdrShaderNodeUniquePtr> {
        // Simple JSON parsing (in production, use serde_json)
        let content = content.trim();

        if !content.starts_with('{') || !content.ends_with('}') {
            log::warn!("Invalid JSON format in sdrOsl file");
            return None;
        }

        let mut properties = Vec::new();
        let mut node_metadata = SdrShaderNodeMetadata::new();

        // Extract basic fields
        let name =
            extract_json_string(content, "name").unwrap_or_else(|| discovery_result.name.clone());

        let context = extract_json_string(content, "context")
            .map(|s| Token::new(&s))
            .unwrap_or_else(|| Token::new("pattern"));

        let source_type = extract_json_string(content, "sourceType")
            .map(|s| Token::new(&s))
            .unwrap_or_else(|| self.source_type.clone());

        // Extract help
        if let Some(help) = extract_json_string(content, "help") {
            node_metadata.set_help(&help);
        }

        // Extract category
        if let Some(category) = extract_json_string(content, "category") {
            node_metadata.set_category(&Token::new(&category));
        }

        // Extract label
        if let Some(label) = extract_json_string(content, "label") {
            node_metadata.set_label(&Token::new(&label));
        }

        // Parse inputs array
        if let Some(inputs_section) = extract_json_array(content, "inputs") {
            for input_json in split_json_array(&inputs_section) {
                if let Some(prop) = self.parse_property(&input_json, false) {
                    properties.push(Box::new(prop));
                }
            }
        }

        // Parse outputs array
        if let Some(outputs_section) = extract_json_array(content, "outputs") {
            for output_json in split_json_array(&outputs_section) {
                if let Some(prop) = self.parse_property(&output_json, true) {
                    properties.push(Box::new(prop));
                }
            }
        }

        // Create the shader node
        let node = SdrShaderNode::new(
            discovery_result.identifier.clone(),
            discovery_result.version,
            name,
            discovery_result.family.clone(),
            context,
            source_type,
            discovery_result.uri.clone(),
            discovery_result.resolved_uri.clone(),
            properties,
            node_metadata,
            String::new(),
        );

        Some(Box::new(node))
    }

    /// Parses a single property from JSON.
    fn parse_property(&self, json: &str, is_output: bool) -> Option<SdrShaderProperty> {
        let name = extract_json_string(json, "name")?;
        let type_str = extract_json_string(json, "type").unwrap_or_else(|| "float".to_string());

        // Convert type
        let sdr_type = json_type_to_sdr_type(&type_str);

        // Parse default value
        let default_value = if let Some(default_str) = extract_json_value(json, "default") {
            parse_json_default(&default_str, &type_str)
        } else {
            Value::default()
        };

        // Extract metadata
        let mut prop_metadata = SdrShaderPropertyMetadata::new();
        let hints = SdrTokenMap::new();

        if let Some(help) = extract_json_string(json, "help") {
            prop_metadata.set_help(&help);
        }

        if let Some(page) = extract_json_string(json, "page") {
            prop_metadata.set_page(&Token::new(&page));
        }

        if let Some(label) = extract_json_string(json, "label") {
            prop_metadata.set_label(&Token::new(&label));
        }

        if let Some(widget) = extract_json_string(json, "widget") {
            prop_metadata.set_widget(&Token::new(&widget));
        }

        // Parse options
        let options = if let Some(opts) = extract_json_array(json, "options") {
            parse_json_options(&opts)
        } else {
            Vec::new()
        };

        Some(SdrShaderProperty::new(
            Token::new(&name),
            sdr_type,
            default_value,
            is_output,
            0,
            prop_metadata,
            hints,
            options,
        ))
    }
}

impl SdrParserPlugin for SdrOslParserPlugin {
    fn parse_shader_node(
        &self,
        discovery_result: &SdrShaderNodeDiscoveryResult,
    ) -> Option<SdrShaderNodeUniquePtr> {
        let content = if !discovery_result.source_code.is_empty() {
            discovery_result.source_code.clone()
        } else if !discovery_result.resolved_uri.is_empty() {
            match std::fs::read_to_string(&discovery_result.resolved_uri) {
                Ok(c) => c,
                Err(e) => {
                    log::warn!(
                        "Failed to read sdrOsl file {}: {}",
                        discovery_result.resolved_uri,
                        e
                    );
                    return Some(get_invalid_shader_node(discovery_result));
                }
            }
        } else {
            log::warn!(
                "No source for sdrOsl file: {}",
                discovery_result.identifier.as_str()
            );
            return Some(get_invalid_shader_node(discovery_result));
        };

        self.parse_json(&content, discovery_result)
    }

    fn get_discovery_types(&self) -> SdrTokenVec {
        self.discovery_types.clone()
    }

    fn get_source_type(&self) -> Token {
        self.source_type.clone()
    }

    fn get_name(&self) -> &str {
        "SdrOslParserPlugin"
    }
}

// ============================================================================
// JSON Parsing Helpers (simple implementation)
// ============================================================================

/// Extracts a string value from JSON.
fn extract_json_string(json: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\"", key);
    if let Some(start) = json.find(&pattern) {
        let after_key = &json[start + pattern.len()..];
        // Skip whitespace and colon
        let trimmed = after_key.trim_start();
        if let Some(rest) = trimmed.strip_prefix(':') {
            let rest = rest.trim_start();
            if let Some(rest) = rest.strip_prefix('"') {
                // String value
                if let Some(end) = rest.find('"') {
                    return Some(rest[..end].to_string());
                }
            }
        }
    }
    None
}

/// Extracts a raw value (could be string, number, array, etc.).
fn extract_json_value(json: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\"", key);
    if let Some(start) = json.find(&pattern) {
        let after_key = &json[start + pattern.len()..];
        let trimmed = after_key.trim_start();
        if let Some(rest) = trimmed.strip_prefix(':') {
            let rest = rest.trim_start();

            // Find the end of the value
            if let Some(rest) = rest.strip_prefix('"') {
                // String
                if let Some(end) = rest.find('"') {
                    return Some(rest[..end].to_string());
                }
            } else if rest.starts_with('[') {
                // Array - find matching bracket
                let mut depth = 0;
                for (byte_pos, c) in rest.char_indices() {
                    match c {
                        '[' => depth += 1,
                        ']' => {
                            depth -= 1;
                            if depth == 0 {
                                return Some(rest[..=byte_pos].to_string());
                            }
                        }
                        _ => {}
                    }
                }
            } else {
                // Number or other primitive - read until comma or closing brace
                let end = rest.find([',', '}', ']']).unwrap_or(rest.len());
                return Some(rest[..end].trim().to_string());
            }
        }
    }
    None
}

/// Extracts a JSON array as a string.
fn extract_json_array(json: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\"", key);
    if let Some(start) = json.find(&pattern) {
        let after_key = &json[start + pattern.len()..];
        let trimmed = after_key.trim_start();
        if let Some(rest) = trimmed.strip_prefix(':') {
            let rest = rest.trim_start();
            if rest.starts_with('[') {
                let mut depth = 0;
                for (byte_pos, c) in rest.char_indices() {
                    match c {
                        '[' => depth += 1,
                        ']' => {
                            depth -= 1;
                            if depth == 0 {
                                // Extract content between [ and ], using byte positions
                                if let Some(content) = rest.get(1..byte_pos) {
                                    return Some(content.to_string());
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    None
}

/// Splits a JSON array content into individual objects.
fn split_json_array(array_content: &str) -> Vec<String> {
    let mut objects = Vec::new();
    let mut depth = 0;
    let mut byte_start = 0;
    let mut in_object = false;

    for (byte_pos, c) in array_content.char_indices() {
        match c {
            '{' => {
                if depth == 0 {
                    byte_start = byte_pos;
                    in_object = true;
                }
                depth += 1;
            }
            '}' => {
                depth -= 1;
                if depth == 0 && in_object {
                    objects.push(array_content[byte_start..=byte_pos].to_string());
                    in_object = false;
                }
            }
            _ => {}
        }
    }

    objects
}

/// Parses JSON options array.
fn parse_json_options(options_content: &str) -> SdrOptionVec {
    let mut options = Vec::new();

    // Options can be simple strings or objects with value/label
    for item in split_json_array(options_content) {
        if let Some(value) = extract_json_string(&item, "value") {
            let label = extract_json_string(&item, "label").unwrap_or_default();
            options.push((Token::new(&value), Token::new(&label)));
        }
    }

    // Also handle simple string arrays
    let trimmed = options_content.trim();
    if !trimmed.starts_with('{') {
        // Simple string array: ["opt1", "opt2"]
        for part in trimmed.split(',') {
            let part = part.trim().trim_matches('"');
            if !part.is_empty() && !part.starts_with('{') {
                options.push((Token::new(part), Token::default()));
            }
        }
    }

    options
}

// ============================================================================
// Type Conversion
// ============================================================================

/// Converts JSON type string to SDR property type token.
fn json_type_to_sdr_type(type_str: &str) -> Token {
    let prop_types = &tokens().property_types;

    match type_str.to_lowercase().as_str() {
        "int" | "integer" => prop_types.int.clone(),
        "float" => prop_types.float.clone(),
        "string" => prop_types.string.clone(),
        "color" | "color3" | "color3f" => prop_types.color.clone(),
        "color4" | "color4f" => prop_types.color4.clone(),
        "point" | "point3" | "point3f" => prop_types.point.clone(),
        "vector" | "vector3" | "vector3f" => prop_types.vector.clone(),
        "normal" | "normal3" | "normal3f" => prop_types.normal.clone(),
        "matrix" | "matrix4" | "matrix4d" => prop_types.matrix.clone(),
        "bool" | "boolean" => prop_types.int.clone(),
        "asset" => prop_types.string.clone(), // Asset paths are strings
        _ => prop_types.unknown.clone(),
    }
}

/// Parses a JSON default value.
fn parse_json_default(value_str: &str, type_str: &str) -> Value {
    let type_lower = type_str.to_lowercase();

    // Handle array values like [0.5, 0.5, 0.5]
    if value_str.starts_with('[') && value_str.ends_with(']') {
        // Note: Stored as string; GfVec requires Value extension.
        return Value::new(value_str.to_string());
    }

    // Handle scalar values
    match type_lower.as_str() {
        "int" | "integer" => value_str.parse::<i32>().map(Value::new).unwrap_or_default(),
        "float" => {
            // Use f32 since f64 doesn't implement Hash
            value_str
                .parse::<f32>()
                .map(Value::from)
                .unwrap_or_default()
        }
        "bool" | "boolean" => {
            let b = value_str == "true" || value_str == "1";
            Value::new(if b { 1i32 } else { 0i32 })
        }
        "string" | "asset" => Value::new(value_str.to_string()),
        _ => Value::new(value_str.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::declare::SdrVersion;

    #[test]
    fn test_extract_json_string() {
        let json = r#"{"name": "TestShader", "type": "float"}"#;
        assert_eq!(
            extract_json_string(json, "name"),
            Some("TestShader".to_string())
        );
        assert_eq!(extract_json_string(json, "type"), Some("float".to_string()));
    }

    #[test]
    fn test_extract_json_array() {
        let json = r#"{"inputs": [{"name": "a"}, {"name": "b"}]}"#;
        let arr = extract_json_array(json, "inputs");
        assert!(arr.is_some());
    }

    #[test]
    fn test_parse_json_default() {
        assert_eq!(parse_json_default("42", "int").get::<i32>(), Some(&42));
        // Note: We use f32 since f64 doesn't implement Hash
        assert_eq!(
            parse_json_default("3.14", "float").get::<f32>(),
            Some(&3.14f32)
        );

        let color = parse_json_default("[0.5, 0.5, 0.5]", "color");
        // Colors are stored as String for now (until proper GfVec support)
        assert!(color.get::<String>().is_some());
        assert_eq!(color.get::<String>().unwrap(), "[0.5, 0.5, 0.5]");
    }

    #[test]
    fn test_parse_simple_sdrosl() {
        let parser = SdrOslParserPlugin::new();

        let content = r#"{
            "name": "TestShader",
            "context": "pattern",
            "inputs": [
                {"name": "diffuseColor", "type": "color", "default": [0.5, 0.5, 0.5]}
            ],
            "outputs": [
                {"name": "out", "type": "color"}
            ]
        }"#;

        let dr = SdrShaderNodeDiscoveryResult::minimal(
            Token::new("TestShader"),
            SdrVersion::new(1, 0),
            "TestShader".to_string(),
            Token::new("sdrOsl"),
            Token::new(SDROSL_SOURCE_TYPE),
            "/test/TestShader.sdrOsl".to_string(),
            "/test/TestShader.sdrOsl".to_string(),
        );

        let node = parser.parse_json(content, &dr);
        assert!(node.is_some());

        let node = node.unwrap();
        assert_eq!(node.get_name(), "TestShader");
        assert_eq!(node.get_shader_input_names().len(), 1);
        assert_eq!(node.get_shader_output_names().len(), 1);
    }
}
