//! Args Parser Plugin - Parses RenderMan .args shader definition files.
//!
//! Port of the Args parsing functionality from OpenUSD.
//!
//! RenderMan Args files are XML documents that describe shader parameters:
//!
//! ```xml
//! <args format="1.0">
//!     <shaderType>
//!         <tag value="pattern"/>
//!     </shaderType>
//!     <param name="diffuseColor" type="color" default="0.5 0.5 0.5">
//!         <help>The diffuse color of the surface</help>
//!         <page>Basic</page>
//!     </param>
//!     <param name="roughness" type="float" default="0.5" min="0" max="1">
//!         <help>Surface roughness</help>
//!     </param>
//!     <output name="out" type="color"/>
//! </args>
//! ```
//!
//! # Supported Elements
//!
//! - `<param>` - Input parameter with name, type, default, min, max, help
//! - `<output>` - Output with name and type
//! - `<shaderType>` - Context (pattern, surface, displacement, etc.)
//! - `<help>` - Documentation string
//! - `<page>` - UI page grouping
//! - `<hintdict>` / `<hint>` - Metadata hints

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

/// Source type identifier for Args shaders.
pub const ARGS_SOURCE_TYPE: &str = "RmanCpp";

/// Discovery type for Args files.
pub const ARGS_DISCOVERY_TYPE: &str = "args";

/// Parser plugin for RenderMan .args files.
///
/// Parses XML-based shader definitions used by RenderMan and compatible renderers.
pub struct SdrArgsParserPlugin {
    discovery_types: SdrTokenVec,
    source_type: Token,
}

impl Default for SdrArgsParserPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl SdrArgsParserPlugin {
    /// Creates a new Args parser plugin.
    pub fn new() -> Self {
        Self {
            discovery_types: vec![Token::new(ARGS_DISCOVERY_TYPE)],
            source_type: Token::new(ARGS_SOURCE_TYPE),
        }
    }

    /// Parses the XML content of an args file.
    fn parse_args_xml(
        &self,
        content: &str,
        discovery_result: &SdrShaderNodeDiscoveryResult,
    ) -> Option<SdrShaderNodeUniquePtr> {
        let mut properties = Vec::new();
        let mut context = Token::new("pattern"); // Default context
        let mut node_metadata = SdrShaderNodeMetadata::new();
        let mut help_text = String::new();

        // Simple XML parsing (in production, use quick-xml or roxmltree)
        let content = content.trim();

        // Check for args root element
        if !content.contains("<args") {
            log::warn!("Args file missing <args> root element");
            return None;
        }

        // Extract shader type/context
        if let Some(shader_type) = extract_tag_value(content, "shaderType") {
            context = Token::new(&shader_type);
        }

        // Extract node-level help
        if let Some(args_help) = extract_element_text(content, "help") {
            help_text = args_help;
        }

        // Parse parameters
        for param in extract_elements(content, "param") {
            if let Some(prop) = self.parse_param(&param, false) {
                properties.push(Box::new(prop));
            }
        }

        // Parse outputs
        for output in extract_elements(content, "output") {
            if let Some(prop) = self.parse_param(&output, true) {
                properties.push(Box::new(prop));
            }
        }

        // Build metadata
        if !help_text.is_empty() {
            node_metadata.set_help(&help_text);
        }

        // Create the shader node
        let node = SdrShaderNode::new(
            discovery_result.identifier.clone(),
            discovery_result.version,
            discovery_result.name.clone(),
            discovery_result.family.clone(),
            context,
            self.source_type.clone(),
            discovery_result.uri.clone(),
            discovery_result.resolved_uri.clone(),
            properties,
            node_metadata,
            String::new(), // No source code for args files
        );

        Some(Box::new(node))
    }

    /// Parses a single param or output element.
    fn parse_param(&self, element: &str, is_output: bool) -> Option<SdrShaderProperty> {
        // Extract attributes
        let name = extract_attr(element, "name")?;
        let type_str = extract_attr(element, "type").unwrap_or_else(|| "float".to_string());
        let default_str = extract_attr(element, "default");

        // Convert type string to SDR type
        let sdr_type = args_type_to_sdr_type(&type_str);

        // Parse default value
        let default_value = if let Some(def) = default_str {
            parse_default_value(&def, &type_str)
        } else {
            Value::default()
        };

        // Extract metadata
        let mut prop_metadata = SdrShaderPropertyMetadata::new();
        let mut hints = SdrTokenMap::new();

        // Help text
        if let Some(help) = extract_element_text(element, "help") {
            prop_metadata.set_help(&help);
        }

        // Page
        if let Some(page) = extract_element_text(element, "page") {
            prop_metadata.set_page(&Token::new(&page));
        }

        // Widget hint
        if let Some(widget) = extract_attr(element, "widget") {
            prop_metadata.set_widget(&Token::new(&widget));
        }

        // Min/max hints
        if let Some(min) = extract_attr(element, "min") {
            hints.insert(Token::new("min"), min);
        }
        if let Some(max) = extract_attr(element, "max") {
            hints.insert(Token::new("max"), max);
        }

        // Connectable
        let connectable = extract_attr(element, "connectable")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(true);
        if !connectable {
            prop_metadata.set_connectable(false);
        }

        // Options (for enum-like parameters)
        let options = parse_options(element);

        Some(SdrShaderProperty::new(
            Token::new(&name),
            sdr_type,
            default_value,
            is_output,
            0, // array_size
            prop_metadata,
            hints,
            options,
        ))
    }
}

impl SdrParserPlugin for SdrArgsParserPlugin {
    fn parse_shader_node(
        &self,
        discovery_result: &SdrShaderNodeDiscoveryResult,
    ) -> Option<SdrShaderNodeUniquePtr> {
        // Read file content
        let content = if !discovery_result.source_code.is_empty() {
            discovery_result.source_code.clone()
        } else if !discovery_result.resolved_uri.is_empty() {
            match std::fs::read_to_string(&discovery_result.resolved_uri) {
                Ok(c) => c,
                Err(e) => {
                    log::warn!(
                        "Failed to read args file {}: {}",
                        discovery_result.resolved_uri,
                        e
                    );
                    return Some(get_invalid_shader_node(discovery_result));
                }
            }
        } else {
            log::warn!(
                "No source for args file: {}",
                discovery_result.identifier.as_str()
            );
            return Some(get_invalid_shader_node(discovery_result));
        };

        self.parse_args_xml(&content, discovery_result)
    }

    fn get_discovery_types(&self) -> SdrTokenVec {
        self.discovery_types.clone()
    }

    fn get_source_type(&self) -> Token {
        self.source_type.clone()
    }

    fn get_name(&self) -> &str {
        "SdrArgsParserPlugin"
    }
}

// ============================================================================
// XML Parsing Helpers (simple implementation)
// ============================================================================

/// Extracts an attribute value from an XML element.
fn extract_attr(element: &str, attr_name: &str) -> Option<String> {
    // Match: attr_name="value" or attr_name='value'
    let patterns = [format!("{}=\"", attr_name), format!("{}='", attr_name)];

    for pattern in &patterns {
        if let Some(start) = element.find(pattern) {
            let value_start = start + pattern.len();
            let quote_char = if pattern.ends_with('"') { '"' } else { '\'' };
            if let Some(end) = element[value_start..].find(quote_char) {
                return Some(element[value_start..value_start + end].to_string());
            }
        }
    }
    None
}

/// Extracts the text content of a child element.
fn extract_element_text(parent: &str, tag_name: &str) -> Option<String> {
    let open_tag = format!("<{}", tag_name);
    let close_tag = format!("</{}>", tag_name);

    if let Some(start) = parent.find(&open_tag) {
        let after_open = &parent[start..];
        // Find the end of opening tag
        if let Some(tag_end) = after_open.find('>') {
            let content_start = tag_end + 1;
            if let Some(close_pos) = after_open.find(&close_tag) {
                let text = &after_open[content_start..close_pos];
                return Some(text.trim().to_string());
            }
        }
    }
    None
}

/// Extracts the value attribute from a tag element.
fn extract_tag_value(content: &str, parent_tag: &str) -> Option<String> {
    let open_tag = format!("<{}", parent_tag);
    if let Some(start) = content.find(&open_tag) {
        let section = &content[start..];
        // Look for <tag value="..."/>
        if let Some(tag_start) = section.find("<tag") {
            let tag_section = &section[tag_start..];
            if let Some(end) = tag_section.find("/>").or_else(|| tag_section.find('>')) {
                let tag_element = &tag_section[..end];
                return extract_attr(tag_element, "value");
            }
        }
    }
    None
}

/// Extracts all elements with a given tag name.
fn extract_elements(content: &str, tag_name: &str) -> Vec<String> {
    let mut elements = Vec::new();
    let open_tag = format!("<{}", tag_name);

    let mut search_from = 0;
    while let Some(start) = content[search_from..].find(&open_tag) {
        let abs_start = search_from + start;
        let after_start = &content[abs_start..];

        // Find the end of this element (either /> or </tag>)
        let close_tag = format!("</{}>", tag_name);

        if let Some(self_close) = after_start.find("/>") {
            if let Some(full_close) = after_start.find(&close_tag) {
                // Use whichever comes first
                let end = if self_close < full_close {
                    self_close + 2
                } else {
                    full_close + close_tag.len()
                };
                elements.push(after_start[..end].to_string());
                search_from = abs_start + end;
            } else {
                // Self-closing only
                elements.push(after_start[..self_close + 2].to_string());
                search_from = abs_start + self_close + 2;
            }
        } else if let Some(full_close) = after_start.find(&close_tag) {
            elements.push(after_start[..full_close + close_tag.len()].to_string());
            search_from = abs_start + full_close + close_tag.len();
        } else {
            // Malformed, skip
            search_from = abs_start + open_tag.len();
        }
    }

    elements
}

/// Parses options from hintdict or option elements.
fn parse_options(element: &str) -> SdrOptionVec {
    let mut options = Vec::new();

    // Look for <hintdict name="options"> or <option> elements
    for hint in extract_elements(element, "hint") {
        if let Some(name) = extract_attr(&hint, "name") {
            if let Some(value) = extract_attr(&hint, "value") {
                options.push((Token::new(&name), Token::new(&value)));
            }
        }
    }

    // Also check for <options> element with string value
    if let Some(opts_str) = extract_attr(element, "options") {
        // Format: "option1|option2|option3"
        for opt in opts_str.split('|') {
            let opt = opt.trim();
            if !opt.is_empty() {
                options.push((Token::new(opt), Token::default()));
            }
        }
    }

    options
}

// ============================================================================
// Type Conversion
// ============================================================================

/// Converts Args type string to SDR property type token.
fn args_type_to_sdr_type(type_str: &str) -> Token {
    let prop_types = &tokens().property_types;

    match type_str.to_lowercase().as_str() {
        "int" | "integer" => prop_types.int.clone(),
        "float" => prop_types.float.clone(),
        "double" => prop_types.float.clone(), // SDR has no double
        "string" => prop_types.string.clone(),
        "color" | "color3" => prop_types.color.clone(),
        "color4" => prop_types.color4.clone(),
        "point" | "point3" => prop_types.point.clone(),
        "vector" | "vector3" => prop_types.vector.clone(),
        "normal" | "normal3" => prop_types.normal.clone(),
        "matrix" | "matrix4" => prop_types.matrix.clone(),
        "bool" | "boolean" => prop_types.int.clone(), // SDR has no bool
        "struct" => prop_types.vstruct.clone(),
        _ => prop_types.unknown.clone(),
    }
}

/// Parses a default value string into a Value.
fn parse_default_value(default_str: &str, type_str: &str) -> Value {
    let type_lower = type_str.to_lowercase();

    match type_lower.as_str() {
        "int" | "integer" => default_str
            .parse::<i32>()
            .map(Value::new)
            .unwrap_or_default(),
        "float" | "double" => {
            // Use f32 since f64 doesn't implement Hash
            default_str
                .parse::<f32>()
                .map(Value::from)
                .unwrap_or_default()
        }
        "bool" | "boolean" => {
            let b = default_str == "true" || default_str == "1";
            Value::new(if b { 1i32 } else { 0i32 })
        }
        "string" => Value::new(default_str.to_string()),
        "color" | "color3" | "point" | "point3" | "vector" | "vector3" | "normal" | "normal3" => {
            // Note: Stored as string; GfVec3f requires Value extension.
            Value::new(default_str.to_string())
        }
        "color4" => {
            // Store as string representation for now
            Value::new(default_str.to_string())
        }
        _ => Value::new(default_str.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::declare::SdrVersion;

    #[test]
    fn test_extract_attr() {
        let element = r#"<param name="test" type="float" default="1.0"/>"#;
        assert_eq!(extract_attr(element, "name"), Some("test".to_string()));
        assert_eq!(extract_attr(element, "type"), Some("float".to_string()));
        assert_eq!(extract_attr(element, "default"), Some("1.0".to_string()));
        assert_eq!(extract_attr(element, "missing"), None);
    }

    #[test]
    fn test_extract_element_text() {
        let element = r#"<param name="test"><help>Some help text</help></param>"#;
        assert_eq!(
            extract_element_text(element, "help"),
            Some("Some help text".to_string())
        );
    }

    #[test]
    fn test_args_type_to_sdr_type() {
        let prop_types = &tokens().property_types;
        assert_eq!(args_type_to_sdr_type("float"), prop_types.float);
        assert_eq!(args_type_to_sdr_type("int"), prop_types.int);
        assert_eq!(args_type_to_sdr_type("color"), prop_types.color);
        assert_eq!(args_type_to_sdr_type("string"), prop_types.string);
    }

    #[test]
    fn test_parse_default_value() {
        assert_eq!(parse_default_value("42", "int").get::<i32>(), Some(&42));
        // Note: We use f32 since f64 doesn't implement Hash
        assert_eq!(
            parse_default_value("3.14", "float").get::<f32>(),
            Some(&3.14f32)
        );
    }

    #[test]
    fn test_parse_simple_args() {
        let parser = SdrArgsParserPlugin::new();

        let content = r#"
        <args format="1.0">
            <param name="inputA" type="float" default="0.0"/>
            <output name="out" type="color"/>
        </args>
        "#;

        let dr = SdrShaderNodeDiscoveryResult::minimal(
            Token::new("TestShader"),
            SdrVersion::new(1, 0),
            "TestShader".to_string(),
            Token::new("args"),
            Token::new(ARGS_SOURCE_TYPE),
            "/test/TestShader.args".to_string(),
            "/test/TestShader.args".to_string(),
        );

        // Set source code for testing
        let mut dr = dr;
        dr.source_code = content.to_string();

        let node = parser.parse_args_xml(content, &dr);
        assert!(node.is_some());

        let node = node.unwrap();
        assert_eq!(node.get_name(), "TestShader");
        assert_eq!(node.get_shader_input_names().len(), 1);
        assert_eq!(node.get_shader_output_names().len(), 1);
    }
}
