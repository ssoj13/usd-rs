//! HioGlslfxConfig - Shader configuration parser for .glslfx files.
//!
//! Port of pxr/imaging/hio/glslfxConfig.h/cpp
//!
//! Parses configuration sections of GLSLFX files, extracting parameters,
//! textures, attributes, metadata, and source key mappings per technique.

use std::collections::HashMap;
use usd_tf::Token;
use usd_vt::{Dictionary, Value};

// ============================================================================
// Role
// ============================================================================

/// Enumerates roles that parameters can have.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    /// No special role.
    None,
    /// Color role (vec3/vec4 color value).
    Color,
}

// ============================================================================
// Parameter
// ============================================================================

/// A shader parameter declaration from a GLSLFX config section.
#[derive(Debug, Clone)]
pub struct Parameter {
    /// Parameter name.
    pub name: String,
    /// Default value.
    pub default_value: Value,
    /// Documentation string.
    pub doc_string: String,
    /// Role (e.g., color).
    pub role: Role,
}

impl Parameter {
    /// Create a new parameter.
    pub fn new(name: &str, default_value: Value, doc_string: &str, role: Role) -> Self {
        Self {
            name: name.to_string(),
            default_value,
            doc_string: doc_string.to_string(),
            role,
        }
    }
}

/// A list of parameters.
pub type Parameters = Vec<Parameter>;

// ============================================================================
// Texture
// ============================================================================

/// A shader texture declaration from a GLSLFX config section.
#[derive(Debug, Clone)]
pub struct Texture {
    /// Texture name.
    pub name: String,
    /// Default value (e.g. default color).
    pub default_value: Value,
    /// Documentation string.
    pub doc_string: String,
}

impl Texture {
    /// Create a new texture.
    pub fn new(name: &str, default_value: Value, doc_string: &str) -> Self {
        Self {
            name: name.to_string(),
            default_value,
            doc_string: doc_string.to_string(),
        }
    }
}

/// A list of textures.
pub type Textures = Vec<Texture>;

// ============================================================================
// Attribute
// ============================================================================

/// A shader attribute declaration from a GLSLFX config section.
#[derive(Debug, Clone)]
pub struct Attribute {
    /// Attribute name.
    pub name: String,
    /// Default value.
    pub default_value: Value,
    /// Documentation string.
    pub doc_string: String,
}

impl Attribute {
    /// Create a new attribute.
    pub fn new(name: &str, default_value: Value, doc_string: &str) -> Self {
        Self {
            name: name.to_string(),
            default_value,
            doc_string: doc_string.to_string(),
        }
    }
}

/// A list of attributes.
pub type Attributes = Vec<Attribute>;

/// Source key list for a shader stage.
pub type SourceKeys = Vec<String>;

/// Metadata dictionary (maps string keys to values).
pub type MetadataDictionary = Dictionary;

// ============================================================================
// HioGlslfxConfig
// ============================================================================

/// Shader configuration parsed from a GLSLFX file's configuration section.
///
/// Matches C++ `HioGlslfxConfig`.
///
/// Holds parameters, textures, attributes, metadata, and source key
/// mappings (per shader stage) extracted from the JSON config block.
pub struct HioGlslfxConfig {
    technique: Token,
    params: Parameters,
    textures: Textures,
    attributes: Attributes,
    metadata: MetadataDictionary,
    source_key_map: HashMap<String, SourceKeys>,
}

impl HioGlslfxConfig {
    /// Parse a GLSLFX configuration from JSON input string.
    ///
    /// Matches C++ `HioGlslfxConfig::Read()`.
    pub fn read(
        technique: &Token,
        input: &str,
        _filename: &str,
        errors: &mut String,
    ) -> Option<Self> {
        let dict = parse_dict_from_input(input, errors)?;
        Some(Self::new(technique.clone(), &dict, errors))
    }

    /// Create from a pre-parsed dictionary.
    fn new(technique: Token, dict: &Dictionary, errors: &mut String) -> Self {
        let params = Self::parse_parameters(dict, errors);
        let textures = Self::parse_textures(dict, errors);
        let attributes = Self::parse_attributes(dict, errors);
        let metadata = Self::parse_metadata(dict, errors);
        let source_key_map = Self::parse_source_key_map(&technique, dict, errors);

        Self {
            technique,
            params,
            textures,
            attributes,
            metadata,
            source_key_map,
        }
    }

    /// Return the parameters.
    pub fn get_parameters(&self) -> &Parameters {
        &self.params
    }

    /// Return the textures.
    pub fn get_textures(&self) -> &Textures {
        &self.textures
    }

    /// Return the attributes.
    pub fn get_attributes(&self) -> &Attributes {
        &self.attributes
    }

    /// Return the metadata dictionary.
    pub fn get_metadata(&self) -> &MetadataDictionary {
        &self.metadata
    }

    /// Return the technique token.
    pub fn technique(&self) -> &Token {
        &self.technique
    }

    /// Return source keys for a shader stage.
    pub fn get_source_keys(&self, shader_stage_key: &Token) -> SourceKeys {
        self.source_key_map
            .get(shader_stage_key.as_str())
            .cloned()
            .unwrap_or_default()
    }

    // ========================================================================
    // Private parsing methods
    // ========================================================================

    /// Parse parameters from the config dictionary.
    fn parse_parameters(dict: &Dictionary, errors: &mut String) -> Parameters {
        let mut result = Parameters::new();

        let params_dict = match dict.get("parameters") {
            Some(v) => match v.get::<Dictionary>() {
                Some(d) => d.clone(),
                None => {
                    errors.push_str("parameters declaration expects a dictionary value\n");
                    return result;
                }
            },
            None => return result,
        };

        // Check for parameter order
        let mut param_order: Vec<String> = Vec::new();
        if let Some(order_val) = dict.get("parameterOrder") {
            if let Some(order_list) = order_val.get::<Vec<String>>() {
                for name in order_list {
                    if !param_order.contains(name) {
                        param_order.push(name.clone());
                    }
                }
            }
        }

        // Add any params not in the order list
        for key in params_dict.keys() {
            if !param_order.contains(key) {
                param_order.push(key.clone());
            }
        }

        for param_name in &param_order {
            let param_data = match params_dict.get(param_name) {
                Some(v) => v,
                None => continue,
            };

            let param_dict = match param_data.get::<Dictionary>() {
                Some(d) => d,
                None => {
                    errors.push_str(&format!(
                        "parameters declaration for {} expects a dictionary value\n",
                        param_name
                    ));
                    return result;
                }
            };

            // Get default value (required)
            let def_val = match param_dict.get("default") {
                Some(v) => v.clone(),
                None => {
                    errors.push_str(&format!(
                        "parameters declaration for {} must specify a default value\n",
                        param_name
                    ));
                    return result;
                }
            };

            // Optional documentation
            let doc_string = param_dict
                .get("documentation")
                .and_then(|v| v.get::<String>().cloned())
                .unwrap_or_default();

            // Optional role
            let role = param_dict
                .get("role")
                .and_then(|v| v.get::<String>())
                .map(|s| match s.as_str() {
                    "color" => Role::Color,
                    _ => Role::None,
                })
                .unwrap_or(Role::None);

            result.push(Parameter::new(param_name, def_val, &doc_string, role));
        }

        result
    }

    /// Parse textures from the config dictionary.
    fn parse_textures(dict: &Dictionary, errors: &mut String) -> Textures {
        let mut result = Textures::new();

        let textures_dict = match dict.get("textures") {
            Some(v) => match v.get::<Dictionary>() {
                Some(d) => d.clone(),
                None => {
                    errors.push_str("textures declaration expects a dictionary value\n");
                    return result;
                }
            },
            None => return result,
        };

        for (tex_name, tex_data) in textures_dict.iter() {
            let tex_dict = match tex_data.get::<Dictionary>() {
                Some(d) => d,
                None => {
                    errors.push_str(&format!(
                        "textures declaration for {} expects a dictionary value\n",
                        tex_name
                    ));
                    return result;
                }
            };

            let def_val = tex_dict
                .get("default")
                .cloned()
                .unwrap_or_else(|| Value::from(0.0f64));

            let doc_string = tex_dict
                .get("documentation")
                .and_then(|v| v.get::<String>().cloned())
                .unwrap_or_default();

            result.push(Texture::new(tex_name, def_val, &doc_string));
        }

        result
    }

    /// Parse attributes from the config dictionary.
    fn parse_attributes(dict: &Dictionary, errors: &mut String) -> Attributes {
        let mut result = Attributes::new();

        let attrs_dict = match dict.get("attributes") {
            Some(v) => match v.get::<Dictionary>() {
                Some(d) => d.clone(),
                None => {
                    errors.push_str("attributes declaration expects a dictionary value\n");
                    return result;
                }
            },
            None => return result,
        };

        for (attr_name, attr_data) in attrs_dict.iter() {
            let attr_dict = match attr_data.get::<Dictionary>() {
                Some(d) => d,
                None => {
                    errors.push_str(&format!(
                        "attributes declaration for {} expects a dictionary value\n",
                        attr_name
                    ));
                    return result;
                }
            };

            let def_val = get_default_value(attr_name, attr_dict, errors);

            let doc_string = attr_dict
                .get("documentation")
                .and_then(|v| v.get::<String>().cloned())
                .unwrap_or_default();

            result.push(Attribute::new(attr_name, def_val, &doc_string));
        }

        result
    }

    /// Parse metadata from the config dictionary.
    fn parse_metadata(dict: &Dictionary, _errors: &mut String) -> MetadataDictionary {
        match dict.get("metadata") {
            Some(v) => match v.get::<Dictionary>() {
                Some(d) => d.clone(),
                None => Dictionary::new(),
            },
            None => Dictionary::new(),
        }
    }

    /// Parse source key map from the techniques section.
    fn parse_source_key_map(
        technique: &Token,
        dict: &Dictionary,
        errors: &mut String,
    ) -> HashMap<String, SourceKeys> {
        let mut result = HashMap::new();

        let techniques_dict = match dict.get("techniques") {
            Some(v) => match v.get::<Dictionary>() {
                Some(d) => d.clone(),
                None => {
                    errors.push_str("techniques declaration expects a dictionary value\n");
                    return result;
                }
            },
            None => {
                errors.push_str("Configuration does not specify techniques\n");
                return result;
            }
        };

        if techniques_dict.is_empty() {
            errors.push_str("No techniques specified\n");
            return result;
        }

        let technique_name = technique.as_str();
        let spec_dict = match techniques_dict.get(technique_name) {
            Some(v) => match v.get::<Dictionary>() {
                Some(d) => d.clone(),
                None => {
                    errors.push_str(&format!(
                        "techniques spec for {} expects a dictionary value\n",
                        technique_name
                    ));
                    return result;
                }
            },
            None => {
                errors.push_str(&format!("No entry for techniques: {}\n", technique_name));
                return result;
            }
        };

        // Parse each shader stage
        for (stage_key, stage_spec) in spec_dict.iter() {
            let stage_dict = match stage_spec.get::<Dictionary>() {
                Some(d) => d,
                None => {
                    errors.push_str(&format!(
                        "{} spec for {} expects a dictionary value\n",
                        technique_name, stage_key
                    ));
                    return result;
                }
            };

            let source = match stage_dict.get("source") {
                Some(v) => v,
                None => {
                    errors.push_str(&format!(
                        "{} spec doesn't define source for {}\n",
                        technique_name, stage_key
                    ));
                    return result;
                }
            };

            // Source should be a list of strings
            if let Some(source_list) = source.get::<Vec<String>>() {
                result.insert(stage_key.clone(), source_list.clone());
            } else {
                errors.push_str(&format!(
                    "source of {} for spec {} expects a list\n",
                    stage_key, technique_name
                ));
                return result;
            }
        }

        result
    }
}

// ============================================================================
// Helper: default value extraction
// ============================================================================

/// Extract a default value from an attribute dictionary, considering type info.
///
/// Matches C++ `_GetDefaultValue()`.
/// BUG-8: Check if default value type validation is enabled.
///
/// Reads `HIO_GLSLFX_DEFAULT_VALUE_VALIDATION` env var.
/// Validation is ON by default (matches C++ behavior).
fn glslfx_default_value_validation() -> bool {
    std::env::var("HIO_GLSLFX_DEFAULT_VALUE_VALIDATION")
        .map(|v| !matches!(v.to_lowercase().as_str(), "0" | "false" | "no"))
        .unwrap_or(true)
}

/// Return true if `v` holds a float or double scalar.
fn is_float_or_double(v: &Value) -> bool {
    v.get::<f32>().is_some() || v.get::<f64>().is_some()
}

/// Return true if `v` holds any integer scalar (i32 / u32 / i64 / u64).
fn is_int_val(v: &Value) -> bool {
    v.get::<i32>().is_some()
        || v.get::<u32>().is_some()
        || v.get::<i64>().is_some()
        || v.get::<u64>().is_some()
}

/// Return true if `v` holds a Vec<f32> of length `n`.
fn is_vec_n(v: &Value, n: usize) -> bool {
    if let Some(arr) = v.get::<Vec<f32>>() {
        arr.len() == n
    } else if let Some(arr) = v.get::<Vec<f64>>() {
        arr.len() == n
    } else {
        false
    }
}

fn get_default_value(name: &str, dict: &Dictionary, errors: &mut String) -> Value {
    let has_default = dict.get("default");
    let type_name = dict.get("type").and_then(|v| v.get::<String>().cloned());

    // No type declared: return default as-is (or zero vec if no default either)
    let Some(type_str) = type_name.as_deref() else {
        if let Some(def) = has_default {
            return def.clone();
        }
        errors.push_str(&format!("No type or default value for {}\n", name));
        return Value::from(vec![0.0f32, 0.0f32, 0.0f32, 0.0f32]);
    };

    // Table: (type_name, predicate, zero_default)
    // BUG-2: validate that the declared type matches the default value type
    struct TypeInfo {
        type_name: &'static str,
        check: fn(&Value) -> bool,
        zero: fn() -> Value,
    }

    let table: &[TypeInfo] = &[
        TypeInfo {
            type_name: "float",
            check: is_float_or_double,
            zero: || Value::from(0.0f32),
        },
        TypeInfo {
            type_name: "double",
            check: is_float_or_double,
            zero: || Value::from(0.0f64),
        },
        TypeInfo {
            type_name: "int",
            check: is_int_val,
            zero: || Value::from(0i32),
        },
        TypeInfo {
            type_name: "vec2",
            check: |v| is_vec_n(v, 2),
            zero: || Value::from(vec![0.0f32, 0.0f32]),
        },
        TypeInfo {
            type_name: "vec3",
            check: |v| is_vec_n(v, 3),
            zero: || Value::from(vec![0.0f32, 0.0f32, 0.0f32]),
        },
        TypeInfo {
            type_name: "vec4",
            check: |v| is_vec_n(v, 4),
            zero: || Value::from(vec![0.0f32, 0.0f32, 0.0f32, 0.0f32]),
        },
    ];

    if let Some(entry) = table.iter().find(|e| e.type_name == type_str) {
        if let Some(def) = has_default {
            // BUG-2: validate type if validation is enabled
            if glslfx_default_value_validation() && !(entry.check)(def) {
                errors.push_str(&format!(
                    "Default value type mismatch for param \"{}\": expected {} but got different type\n",
                    name, type_str
                ));
                return (entry.zero)();
            }
            return def.clone();
        }
        return (entry.zero)();
    }

    // Unknown type
    errors.push_str(&format!("Invalid type {} for {}\n", type_str, name));
    has_default
        .cloned()
        .unwrap_or_else(|| Value::from(vec![0.0f32, 0.0f32, 0.0f32, 0.0f32]))
}

// ============================================================================
// Helper: JSON dictionary parsing
// ============================================================================

/// Parse a JSON-like input string into a Dictionary.
///
/// Matches C++ `Hio_GetDictionaryFromInput()`.
///
/// Strips comment lines (lines starting with #) before parsing.
pub fn parse_dict_from_input(input: &str, errors: &mut String) -> Option<Dictionary> {
    if input.is_empty() {
        errors.push_str("Cannot create Dictionary from empty string\n");
        return None;
    }

    // Strip comment lines (lines starting with #)
    let filtered: String = input
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();
            if trimmed.starts_with('#') { "" } else { line }
        })
        .collect::<Vec<&str>>()
        .join("\n");

    // Replace Python-style True/False with JSON true/false
    let json_str = filtered
        .replace("True", "true")
        .replace("False", "false")
        // Handle single-quoted strings -> double-quoted (common in GLSLFX)
        .replace('\'', "\"");

    match serde_json::from_str::<serde_json::Value>(&json_str) {
        Ok(json_val) => {
            let dict = json_to_dictionary(&json_val);
            Some(dict)
        }
        Err(e) => {
            errors.push_str(&format!(
                "Failed to parse JSON (line {}, col {}): {}\n",
                e.line(),
                e.column(),
                e
            ));
            None
        }
    }
}

/// Convert a serde_json::Value into a usd_vt::Dictionary.
fn json_to_dictionary(json: &serde_json::Value) -> Dictionary {
    let mut dict = Dictionary::new();
    if let serde_json::Value::Object(map) = json {
        for (key, val) in map {
            dict.insert(key.clone(), json_to_value(val));
        }
    }
    dict
}

/// Convert a serde_json::Value into a usd_vt::Value.
///
/// JSON arrays are converted to concrete Rust types:
/// - Array of numbers -> Vec<f32>
/// - Array of strings -> Vec<String>
/// - Object -> Dictionary
fn json_to_value(json: &serde_json::Value) -> Value {
    match json {
        serde_json::Value::Null => Value::default(),
        serde_json::Value::Bool(b) => Value::from(*b),
        serde_json::Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                Value::from(f)
            } else {
                Value::default()
            }
        }
        serde_json::Value::String(s) => Value::from(s.clone()),
        serde_json::Value::Array(arr) => {
            // Detect array element type
            if arr.is_empty() {
                return Value::from(Vec::<String>::new());
            }
            // All numbers -> Vec<f32>
            if arr.iter().all(|v| v.is_number()) {
                let floats: Vec<f32> = arr
                    .iter()
                    .filter_map(|v| v.as_f64().map(|f| f as f32))
                    .collect();
                return Value::from(floats);
            }
            // All strings -> Vec<String>
            if arr.iter().all(|v| v.is_string()) {
                let strings: Vec<String> = arr
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect();
                return Value::from(strings);
            }
            // Mixed/complex arrays -> store as Vec<String> (serialized)
            // This is a simplification; in practice GLSLFX arrays
            // are either all-numeric or all-string
            let strings: Vec<String> = arr.iter().map(|v| v.to_string()).collect();
            Value::from(strings)
        }
        serde_json::Value::Object(_) => {
            let dict = json_to_dictionary(json);
            Value::from(dict)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty_config() {
        let mut errors = String::new();
        let config = HioGlslfxConfig::read(
            &Token::new("default"),
            r#"{"techniques": {"default": {"fragmentShader": {"source": ["MyFrag"]}}}}"#,
            "test.glslfx",
            &mut errors,
        );
        assert!(config.is_some(), "errors: {}", errors);
        let config = config.unwrap();
        assert!(config.get_parameters().is_empty());
        assert!(config.get_textures().is_empty());
    }

    #[test]
    fn test_parse_parameters() {
        let mut errors = String::new();
        let input = r#"{
            "parameters": {
                "diffuseColor": {
                    "default": [0.18, 0.18, 0.18],
                    "role": "color",
                    "documentation": "Base diffuse color"
                },
                "roughness": {
                    "default": 0.5
                }
            },
            "techniques": {
                "default": {
                    "fragmentShader": {
                        "source": ["MyFrag"]
                    }
                }
            }
        }"#;

        let config =
            HioGlslfxConfig::read(&Token::new("default"), input, "test.glslfx", &mut errors);
        assert!(config.is_some(), "errors: {}", errors);
        let config = config.unwrap();
        let params = config.get_parameters();
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn test_parse_source_keys() {
        let mut errors = String::new();
        let input = r#"{
            "techniques": {
                "default": {
                    "vertexShader": {
                        "source": ["MyVert"]
                    },
                    "fragmentShader": {
                        "source": ["Preamble", "MyFrag"]
                    }
                }
            }
        }"#;

        let config =
            HioGlslfxConfig::read(&Token::new("default"), input, "test.glslfx", &mut errors);
        assert!(config.is_some(), "errors: {}", errors);
        let config = config.unwrap();

        let vert_keys = config.get_source_keys(&Token::new("vertexShader"));
        assert_eq!(vert_keys, vec!["MyVert"]);

        let frag_keys = config.get_source_keys(&Token::new("fragmentShader"));
        assert_eq!(frag_keys, vec!["Preamble", "MyFrag"]);
    }

    #[test]
    fn test_role_parsing() {
        let mut errors = String::new();
        let input = r#"{
            "parameters": {
                "color": {
                    "default": [1.0, 0.0, 0.0],
                    "role": "color"
                }
            },
            "techniques": {"default": {"fragmentShader": {"source": ["F"]}}}
        }"#;

        let config =
            HioGlslfxConfig::read(&Token::new("default"), input, "test.glslfx", &mut errors);
        assert!(config.is_some());
        let config = config.unwrap();
        assert_eq!(config.get_parameters()[0].role, Role::Color);
    }
}
