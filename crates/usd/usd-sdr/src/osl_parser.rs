//! SdrOsl Parser Plugin — parses compiled `.oso` OSL shaders via osl-rs.
//!
//! Port of `_ref/OpenUSD/pxr/usd/plugin/sdrOsl/oslParser.cpp`.
//!
//! Uses `osl_rs::OslQuery` (our pure-Rust port of `OSL::OSLQuery`) to read
//! compiled OSL bytecode (`.oso`) files and extract shader parameters,
//! metadata, and types — converting them to SDR shader node descriptions.
//!
//! Implementation differences from C++:
//! - Uses Cranelift JIT backend instead of LLVM (via osl-rs)
//! - Pure Rust, no C/C++ dependencies
//! - Interfaces and API match the C++ `SdrOslParserPlugin` exactly

use super::declare::{SdrOptionVec, SdrTokenMap, SdrTokenVec};
use super::discovery_result::SdrShaderNodeDiscoveryResult;
use super::parser_plugin::{SdrParserPlugin, get_invalid_shader_node};
use super::shader_metadata_helpers::{
    is_property_a_terminal, is_property_an_asset_identifier, is_truthy, option_vec_val,
};
use super::shader_node::{SdrShaderNode, SdrShaderNodeUniquePtr};
use super::shader_property::SdrShaderProperty;
use super::tokens::tokens;
use usd_gf::matrix4::Matrix4d;
use usd_gf::vec3::Vec3f;
use usd_tf::Token;
use usd_vt::Value;

use osl_rs::oslquery::{OslQuery, Parameter as OslParameter};

// ---------------------------------------------------------------------------
// Private tokens (matches C++ TF_DEFINE_PRIVATE_TOKENS)
// ---------------------------------------------------------------------------

/// Tokens local to the OSL parser plugin.
/// Matches `_tokens` in the C++ `oslParser.cpp`.
#[allow(dead_code)] // C++ parity: all fields present; page_open_str/open_str reserved for future use
struct OslParserTokens {
    array_size: &'static str,
    page_str: &'static str,
    page_open_str: &'static str,
    open_str: &'static str,
    osl_page_delimiter: &'static str,
    vstruct_member: &'static str,
    sdr_definition_name: &'static str,
    discovery_type: &'static str,
    source_type: &'static str,
    usd_schema_def_prefix: &'static str,
    sdr_global_config_prefix: &'static str,
    sdr_definition_name_fallback_prefix: &'static str,
    schema_base: &'static str,
}

const TOKENS: OslParserTokens = OslParserTokens {
    array_size: "arraySize",
    page_str: "page",
    page_open_str: "page_open",
    open_str: "open",
    osl_page_delimiter: ".",
    vstruct_member: "vstructmember",
    sdr_definition_name: "sdrDefinitionName",
    discovery_type: "oso",
    source_type: "OSL",
    usd_schema_def_prefix: "usdSchemaDef_",
    sdr_global_config_prefix: "sdrGlobalConfig_",
    sdr_definition_name_fallback_prefix: "sdrDefinitionNameFallbackPrefix",
    schema_base: "schemaBase",
};

/// Source type identifier for OSL shaders.
/// Matches C++ `SdrOslParserPlugin::_sourceType = "OSL"`.
pub const OSL_SOURCE_TYPE: &str = "OSL";

/// Discovery type for compiled `.oso` files.
/// C++ only registers "oso" (not "osl").
pub const OSO_DISCOVERY_TYPE: &str = "oso";

/// Kept for backward compatibility but C++ does NOT register "osl" as discovery type.
pub const OSL_DISCOVERY_TYPE: &str = "osl";

// ---------------------------------------------------------------------------
// SdrOslParserPlugin (the real one, backed by osl-rs)
// ---------------------------------------------------------------------------

/// Parser plugin for compiled `.oso` OSL shader files.
///
/// Uses `osl_rs::OslQuery` to parse `.oso` bytecode and extract the full
/// shader interface. Matches C++ `SdrOslParserPlugin`.
pub struct OslParserPlugin {
    discovery_types: SdrTokenVec,
    source_type: Token,
}

impl Default for OslParserPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl OslParserPlugin {
    /// Creates a new OSL parser plugin.
    /// Matches C++ `SdrOslParserPlugin::SdrOslParserPlugin()`.
    pub fn new() -> Self {
        Self {
            // C++ only registers "oso"
            discovery_types: vec![Token::new(TOKENS.discovery_type)],
            source_type: Token::new(TOKENS.source_type),
        }
    }

    // -----------------------------------------------------------------------
    // _getSdrContextFromSchemaBase
    // -----------------------------------------------------------------------

    /// Determines the SDR context for a shader from the `schemaBase` metadata.
    /// Matches C++ `_getSdrContextFromSchemaBase`.
    fn get_sdr_context_from_schema_base(&self, metadata: &SdrTokenMap) -> Token {
        let schema_base = match metadata.get(&Token::new(TOKENS.schema_base)) {
            Some(val) => val.to_lowercase(),
            None => return self.source_type.clone(),
        };

        // Order matters: "lightfilter" must be checked before "light"
        let context_mapping: &[(&str, &str)] = &[
            ("displayfilter", "displayFilter"),
            ("lightfilter", "lightFilter"),
            ("samplefilter", "sampleFilter"),
            ("integrator", "integrator"),
            ("light", "light"),
            ("projection", "projection"),
        ];

        for (key, context) in context_mapping {
            if schema_base.contains(key) {
                return Token::new(context);
            }
        }

        // Fallback to source type as default context
        self.source_type.clone()
    }

    // -----------------------------------------------------------------------
    // _getNodeProperties
    // -----------------------------------------------------------------------

    /// Gets a vector of properties from the OSL query object.
    /// Matches C++ `_getNodeProperties`.
    fn get_node_properties(
        &self,
        query: &OslQuery,
        discovery_result: &SdrShaderNodeDiscoveryResult,
        fallback_prefix: &str,
    ) -> Vec<Box<SdrShaderProperty>> {
        let mut properties = Vec::new();
        let prop_metadata_tokens = &tokens().node_metadata;
        let _ = prop_metadata_tokens;

        for param in query.parameters() {
            let mut prop_name = param.name.as_str().to_string();

            // Struct members are not supported (names containing '.')
            if prop_name.contains('.') {
                continue;
            }

            // Extract metadata
            let mut metadata = self.get_property_metadata(param, discovery_result);

            // Get type name, and determine the size of the array (if an array)
            let (type_name, array_size) = self.get_type_name(param, &metadata);

            self.inject_parser_metadata(&mut metadata, &type_name);

            // Non-standard properties in the metadata are considered hints
            let mut hints = SdrTokenMap::new();
            let mut definition_name = String::new();
            let standard_keys = get_standard_property_metadata_keys();

            let mut keys_to_remove = Vec::new();
            for (key, val) in &metadata {
                if standard_keys.contains(&key.as_str().to_string()) {
                    continue;
                }
                if key == TOKENS.sdr_definition_name {
                    definition_name = val.clone();
                    keys_to_remove.push(key.clone());
                    continue;
                }
                if key == TOKENS.array_size {
                    // The metadata sometimes incorrectly specifies array size;
                    // this value is not respected (matches C++ behavior)
                    keys_to_remove.push(key.clone());
                    continue;
                }
                hints.insert(key.clone(), val.clone());
            }
            for key in &keys_to_remove {
                metadata.remove(key);
            }
            // Remove hint keys from metadata too
            for (key, _) in &hints {
                metadata.remove(key);
            }

            // If we found 'definitionName' metadata, rename the property
            // using the OSL parameter name as the ImplementationName.
            // Matches C++ behavior exactly.
            if !definition_name.is_empty() {
                metadata.insert(Token::new("__SDR__implementationName"), prop_name.clone());
                prop_name = definition_name;
            } else if !fallback_prefix.is_empty() {
                metadata.insert(Token::new("__SDR__implementationName"), prop_name.clone());
                // SdfPath::JoinIdentifier equivalent: prefix:name
                prop_name = format!("{}:{}", fallback_prefix, prop_name);
            }

            // Extract options
            let options: SdrOptionVec = if let Some(opts_str) = metadata.get(&Token::new("options"))
            {
                option_vec_val(opts_str)
            } else {
                Vec::new()
            };

            let default_value =
                self.get_default_value(param, &type_name.as_str(), array_size, &metadata);

            let prop_metadata =
                super::shader_property_metadata::SdrShaderPropertyMetadata::from_token_map(
                    &metadata,
                );

            properties.push(Box::new(SdrShaderProperty::new(
                Token::new(&prop_name),
                type_name,
                default_value,
                param.is_output,
                array_size,
                prop_metadata,
                hints,
                options,
            )));
        }

        properties
    }

    // -----------------------------------------------------------------------
    // _getPropertyMetadata
    // -----------------------------------------------------------------------

    /// Gets all metadata for the specified OSL parameter.
    /// Matches C++ `_getPropertyMetadata`.
    fn get_property_metadata(
        &self,
        param: &OslParameter,
        _discovery_result: &SdrShaderNodeDiscoveryResult,
    ) -> SdrTokenMap {
        let mut metadata = SdrTokenMap::new();

        for meta_param in &param.metadata {
            let entry_name = meta_param.name.as_str();

            if entry_name == TOKENS.vstruct_member {
                // Vstruct metadata needs to be specially parsed
                let vstruct = get_param_as_string(meta_param);
                if !vstruct.is_empty() {
                    if let Some(dot_pos) = vstruct.find('.') {
                        metadata.insert(
                            Token::new("vstructMemberOf"),
                            vstruct[..dot_pos].to_string(),
                        );
                        metadata.insert(
                            Token::new("vstructMemberName"),
                            vstruct[dot_pos + 1..].to_string(),
                        );
                    }
                }
            } else if entry_name == TOKENS.page_str {
                // Replace OSL page delimiter "." with SDR page delimiter ":"
                let page_value = get_param_as_string(meta_param);
                let replaced = page_value.replace(
                    TOKENS.osl_page_delimiter,
                    tokens().property_tokens.page_delimiter.as_str(),
                );
                metadata.insert(Token::new(entry_name), replaced);
            } else {
                metadata.insert(Token::new(entry_name), get_param_as_string(meta_param));
            }
        }

        metadata
    }

    // -----------------------------------------------------------------------
    // _injectParserMetadata
    // -----------------------------------------------------------------------

    /// Injects any metadata that is generated by the parser.
    /// Matches C++ `_injectParserMetadata`.
    fn inject_parser_metadata(&self, metadata: &mut SdrTokenMap, type_name: &Token) {
        let prop_types = &tokens().property_types;
        if *type_name == prop_types.string {
            if is_property_an_asset_identifier(metadata) {
                metadata.insert(Token::new("__SDR__isAssetIdentifier"), String::new());
            }
        }
    }

    // -----------------------------------------------------------------------
    // _getNodeMetadata
    // -----------------------------------------------------------------------

    /// Gets all metadata for the node from the OSL query.
    /// Matches C++ `_getNodeMetadata`.
    fn get_node_metadata(&self, query: &OslQuery, base_metadata: &SdrTokenMap) -> SdrTokenMap {
        let mut node_metadata = base_metadata.clone();

        for meta_param in query.metadata() {
            let entry_name = meta_param.name.as_str();

            // Check for usdSchemaDef_ prefix
            if let Some(suffix) = entry_name.strip_prefix(TOKENS.usd_schema_def_prefix) {
                node_metadata.insert(Token::new(suffix), get_param_as_string(meta_param));
            }
            // Check for sdrGlobalConfig_ prefix
            else if let Some(suffix) = entry_name.strip_prefix(TOKENS.sdr_global_config_prefix) {
                node_metadata.insert(Token::new(suffix), get_param_as_string(meta_param));
            } else {
                node_metadata.insert(Token::new(entry_name), get_param_as_string(meta_param));
            }
        }

        node_metadata
    }

    // -----------------------------------------------------------------------
    // _getTypeName
    // -----------------------------------------------------------------------

    /// Gets a common type + array size (if array) from the OSL parameter.
    /// Matches C++ `_getTypeName`.
    fn get_type_name(&self, param: &OslParameter, metadata: &SdrTokenMap) -> (Token, usize) {
        let prop_types = &tokens().property_types;

        // Exit early if this param is known to be a struct
        if param.is_struct {
            return (prop_types.struct_type.clone(), 0);
        }

        // Exit early if the param's metadata indicates a terminal type
        if is_property_a_terminal(metadata) {
            return (prop_types.terminal.clone(), 0);
        }

        // Get the OSL type string (e.g., "color", "float", "int[3]")
        let mut type_name = param.type_desc.to_string();
        let mut array_size: usize = 0;

        // Check for array syntax: "color[3]"
        if let Some(bracket_pos) = type_name.find('[') {
            // Try to parse the array size
            let after_bracket = &type_name[bracket_pos + 1..];
            if let Some(end_bracket) = after_bracket.find(']') {
                let size_str = &after_bracket[..end_bracket];
                array_size = size_str.parse().unwrap_or(0);
                // Dynamic arrays like "color[]" will have array_size == 0;
                // they NEED isDynamicArray metadata set to 1
            }
            type_name = type_name[..bracket_pos].to_string();
        }

        (Token::new(&type_name), array_size)
    }

    // -----------------------------------------------------------------------
    // _getDefaultValue
    // -----------------------------------------------------------------------

    /// Gets the default value of the specified parameter.
    /// Matches C++ `_getDefaultValue` exactly.
    fn get_default_value(
        &self,
        param: &OslParameter,
        osl_type: &str,
        array_size: usize,
        metadata: &SdrTokenMap,
    ) -> Value {
        let prop_types = &tokens().property_types;
        let is_dynamic_array = is_truthy(&Token::new("__SDR__isDynamicArray"), metadata);
        let is_array = array_size > 0 || is_dynamic_array;

        // INT and INT ARRAY
        if osl_type == prop_types.int.as_str() {
            if !is_array && param.idefault.len() == 1 {
                return Value::new(param.idefault[0]);
            }
            // Return as array
            return Value::new(param.idefault.clone());
        }

        // STRING and STRING ARRAY
        if osl_type == prop_types.string.as_str() {
            if !is_array && param.sdefault.len() == 1 {
                return Value::new(param.sdefault[0].as_str().to_string());
            }
            let strings: Vec<String> = param
                .sdefault
                .iter()
                .map(|u| u.as_str().to_string())
                .collect();
            return Value::new(strings);
        }

        // FLOAT and FLOAT ARRAY
        if osl_type == prop_types.float.as_str() {
            if !is_array && param.fdefault.len() == 1 {
                return Value::from(param.fdefault[0]);
            }
            return Value::from_no_hash(param.fdefault.clone());
        }

        // VECTOR TYPES: color, point, normal, vector — stored as GfVec3f
        if osl_type == prop_types.color.as_str()
            || osl_type == prop_types.point.as_str()
            || osl_type == prop_types.normal.as_str()
            || osl_type == prop_types.vector.as_str()
        {
            if !is_array && param.fdefault.len() == 3 {
                return Value::from(Vec3f::new(
                    param.fdefault[0],
                    param.fdefault[1],
                    param.fdefault[2],
                ));
            } else if is_array && param.fdefault.len() % 3 == 0 {
                let num_elements = param.fdefault.len() / 3;
                let mut array = Vec::with_capacity(num_elements);
                for i in 0..num_elements {
                    array.push(Vec3f::new(
                        param.fdefault[3 * i],
                        param.fdefault[3 * i + 1],
                        param.fdefault[3 * i + 2],
                    ));
                }
                return Value::from_no_hash(array);
            }
        }

        // MATRIX — stored as GfMatrix4d
        if osl_type == prop_types.matrix.as_str() {
            // No matrix array support (matches C++)
            if !is_array && param.fdefault.len() == 16 {
                let f = &param.fdefault;
                let data: [[f64; 4]; 4] = [
                    [f[0] as f64, f[1] as f64, f[2] as f64, f[3] as f64],
                    [f[4] as f64, f[5] as f64, f[6] as f64, f[7] as f64],
                    [f[8] as f64, f[9] as f64, f[10] as f64, f[11] as f64],
                    [f[12] as f64, f[13] as f64, f[14] as f64, f[15] as f64],
                ];
                return Value::from(Matrix4d::from_array(data));
            }
        }

        // STRUCT, TERMINAL, VSTRUCT — return empty value
        if osl_type == prop_types.struct_type.as_str()
            || osl_type == prop_types.terminal.as_str()
            || osl_type == prop_types.vstruct.as_str()
        {
            return Value::default();
        }

        // Didn't find a supported type
        Value::default()
    }
}

// ---------------------------------------------------------------------------
// SdrParserPlugin trait implementation
// ---------------------------------------------------------------------------

impl SdrParserPlugin for OslParserPlugin {
    /// Parses a shader node from the given discovery result.
    /// Matches C++ `SdrOslParserPlugin::ParseShaderNode`.
    fn parse_shader_node(
        &self,
        discovery_result: &SdrShaderNodeDiscoveryResult,
    ) -> Option<SdrShaderNodeUniquePtr> {
        let mut osl_query = OslQuery::new();
        let parse_successful;

        if !discovery_result.uri.is_empty() {
            // Attempt to parse the node
            if std::path::Path::new(&discovery_result.resolved_uri).is_file() {
                parse_successful = osl_query.open(&discovery_result.resolved_uri, "");
            } else if !discovery_result.source_code.is_empty() {
                parse_successful = osl_query.open_bytecode(&discovery_result.source_code);
            } else {
                log::warn!(
                    "Could not open the OSL at URI [{}] ({}). \
                     An invalid Sdr node definition will be created.",
                    discovery_result.uri,
                    discovery_result.resolved_uri
                );
                return Some(get_invalid_shader_node(discovery_result));
            }
        } else if !discovery_result.source_code.is_empty() {
            parse_successful = osl_query.open_bytecode(&discovery_result.source_code);
        } else {
            log::warn!(
                "Invalid SdrShaderNodeDiscoveryResult with identifier {}: \
                 both uri and sourceCode are empty.",
                discovery_result.identifier.as_str()
            );
            return Some(get_invalid_shader_node(discovery_result));
        }

        let errors = osl_query.get_error();
        if !parse_successful || !errors.is_empty() {
            log::warn!(
                "Could not parse OSL shader at URI [{}]. \
                 An invalid Sdr node definition will be created. {}{}",
                discovery_result.uri,
                if errors.is_empty() {
                    ""
                } else {
                    "Errors from OSL parser: "
                },
                errors.replace('\n', "; ")
            );
            return Some(get_invalid_shader_node(discovery_result));
        }

        // The sdrDefinitionNameFallbackPrefix is found in the node metadata.
        let metadata = self.get_node_metadata(&osl_query, &discovery_result.metadata);
        let fallback_prefix = metadata
            .get(&Token::new(TOKENS.sdr_definition_name_fallback_prefix))
            .cloned()
            .unwrap_or_default();

        // Generate properties
        let properties = self.get_node_properties(&osl_query, discovery_result, &fallback_prefix);

        // Determine context
        let context = self.get_sdr_context_from_schema_base(&metadata);

        let node_metadata =
            super::shader_node_metadata::SdrShaderNodeMetadata::from_token_map(&metadata);

        Some(Box::new(SdrShaderNode::new(
            discovery_result.identifier.clone(),
            discovery_result.version,
            discovery_result.name.clone(),
            discovery_result.family.clone(),
            context,
            self.source_type.clone(),
            discovery_result.resolved_uri.clone(),
            discovery_result.resolved_uri.clone(), // Definitive: implementation == definition
            properties,
            node_metadata,
            discovery_result.source_code.clone(),
        )))
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

// ---------------------------------------------------------------------------
// Helpers (free functions matching C++ static helpers)
// ---------------------------------------------------------------------------

/// Gets the specified parameter's value as a string.
/// Matches C++ `_getParamAsString`.
fn get_param_as_string(param: &OslParameter) -> String {
    if param.sdefault.len() == 1 {
        return param.sdefault[0].as_str().to_string();
    } else if param.idefault.len() == 1 {
        return param.idefault[0].to_string();
    } else if param.fdefault.len() == 1 {
        return param.fdefault[0].to_string();
    }
    String::new()
}

/// Returns the set of standard SDR property metadata keys.
/// Used to filter non-standard keys into hints.
fn get_standard_property_metadata_keys() -> Vec<String> {
    // Matches SdrPropertyMetadata->allTokens from C++
    vec![
        "options".into(),
        "page".into(),
        "help".into(),
        "label".into(),
        "widget".into(),
        "connectable".into(),
        "isDynamicArray".into(),
        "vstructMemberOf".into(),
        "vstructMemberName".into(),
        "vstructConditionalExpr".into(),
        "validConnectionTypes".into(),
        "__SDR__isAssetIdentifier".into(),
        "__SDR__implementationName".into(),
        "__SDR__defaultinput".into(),
        "__SDR__target".into(),
        "__SDR__colorspace".into(),
        "sdrUsdDefinitionType".into(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_osl_parser_plugin_creation() {
        let plugin = OslParserPlugin::new();
        assert_eq!(plugin.get_name(), "SdrOslParserPlugin");
        assert_eq!(plugin.get_source_type().as_str(), "OSL");

        let types = plugin.get_discovery_types();
        assert_eq!(types.len(), 1);
        assert_eq!(types[0].as_str(), "oso");
    }

    #[test]
    fn test_get_param_as_string() {
        let mut param = OslParameter {
            name: osl_rs::UString::new("test"),
            type_desc: osl_rs::TypeDesc::STRING,
            is_output: false,
            valid_default: true,
            varlen_array: false,
            is_struct: false,
            is_closure: false,
            idefault: vec![],
            fdefault: vec![],
            sdefault: vec![osl_rs::UString::new("hello")],
            spacename: vec![],
            fields: vec![],
            structname: osl_rs::UString::default(),
            metadata: vec![],
            data: vec![],
        };
        assert_eq!(get_param_as_string(&param), "hello");

        param.sdefault.clear();
        param.idefault = vec![42];
        assert_eq!(get_param_as_string(&param), "42");

        param.idefault.clear();
        param.fdefault = vec![3.14];
        assert_eq!(get_param_as_string(&param), "3.14");
    }

    #[test]
    fn test_context_from_schema_base() {
        let plugin = OslParserPlugin::new();

        let mut meta = SdrTokenMap::new();
        meta.insert(
            Token::new("schemaBase"),
            "PxrDisplayFilterPluginBase".to_string(),
        );
        assert_eq!(
            plugin.get_sdr_context_from_schema_base(&meta).as_str(),
            "displayFilter"
        );

        meta.insert(
            Token::new("schemaBase"),
            "PxrLightFilterPluginBase".to_string(),
        );
        assert_eq!(
            plugin.get_sdr_context_from_schema_base(&meta).as_str(),
            "lightFilter"
        );

        meta.insert(Token::new("schemaBase"), "RectLight".to_string());
        assert_eq!(
            plugin.get_sdr_context_from_schema_base(&meta).as_str(),
            "light"
        );

        // No schemaBase → fallback to source type
        let empty_meta = SdrTokenMap::new();
        assert_eq!(
            plugin
                .get_sdr_context_from_schema_base(&empty_meta)
                .as_str(),
            "OSL"
        );
    }
}
