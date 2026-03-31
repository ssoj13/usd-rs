//! Built-in USD Shader Definitions for SDR.
//!
//! Port of pxr/usd/plugin/usdShaders
//!
//! This module provides discovery results for the standard USD preview shaders:
//! - UsdPreviewSurface - Physically-based surface shader
//! - UsdUVTexture - Texture sampling shader
//! - UsdPrimvarReader_* - Primvar reading shaders (float, float2, float3, etc.)
//! - UsdTransform2d - 2D transformation shader
//!
//! These shaders are built into USD and don't require external files.
//! The definitions match the official OpenUSD shaderDefs.usda.

use super::declare::{SdrTokenMap, SdrTokenVec, SdrVersion};
use super::discovery_plugin::{SdrDiscoveryPlugin, SdrDiscoveryPluginContext};
use super::discovery_result::{SdrShaderNodeDiscoveryResult, SdrShaderNodeDiscoveryResultVec};
use super::parser_plugin::SdrParserPlugin;
use super::shader_node::{SdrShaderNode, SdrShaderNodeUniquePtr};
use super::shader_node_metadata::SdrShaderNodeMetadata;
use super::shader_property::{SdrShaderProperty, SdrShaderPropertyUniquePtr};
use super::shader_property_metadata::SdrShaderPropertyMetadata;
use usd_tf::Token;
use usd_vt::Value;

/// Source type for USD preview shaders.
pub const USD_SOURCE_TYPE: &str = "glslfx";

/// Discovery type for USD preview shaders.
pub const USD_DISCOVERY_TYPE: &str = "usda";

/// Built-in USD shaders discovery plugin.
///
/// This plugin provides discovery results for the standard USD preview shaders
/// that are built into the system. No external files are required.
pub struct UsdShadersDiscoveryPlugin;

impl Default for UsdShadersDiscoveryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl UsdShadersDiscoveryPlugin {
    /// Creates a new USD shaders discovery plugin.
    pub fn new() -> Self {
        Self
    }

    /// Creates discovery result for UsdPreviewSurface shader.
    fn create_preview_surface() -> SdrShaderNodeDiscoveryResult {
        let mut metadata = SdrTokenMap::new();
        metadata.insert(Token::new("role"), "surface".to_string());

        SdrShaderNodeDiscoveryResult {
            identifier: Token::new("UsdPreviewSurface"),
            version: SdrVersion::default(),
            name: "UsdPreviewSurface".to_string(),
            family: Token::new("surface"),
            discovery_type: Token::new(USD_DISCOVERY_TYPE),
            source_type: Token::new(USD_SOURCE_TYPE),
            uri: "builtin://UsdPreviewSurface".to_string(),
            resolved_uri: "builtin://UsdPreviewSurface".to_string(),
            source_code: String::new(),
            metadata,
            blind_data: String::new(),
            sub_identifier: Token::default(),
        }
    }

    /// Creates discovery result for UsdUVTexture shader.
    fn create_uv_texture() -> SdrShaderNodeDiscoveryResult {
        let mut metadata = SdrTokenMap::new();
        metadata.insert(Token::new("role"), "texture".to_string());

        SdrShaderNodeDiscoveryResult {
            identifier: Token::new("UsdUVTexture"),
            version: SdrVersion::default(),
            name: "UsdUVTexture".to_string(),
            family: Token::new("texture"),
            discovery_type: Token::new(USD_DISCOVERY_TYPE),
            source_type: Token::new(USD_SOURCE_TYPE),
            uri: "builtin://UsdUVTexture".to_string(),
            resolved_uri: "builtin://UsdUVTexture".to_string(),
            source_code: String::new(),
            metadata,
            blind_data: String::new(),
            sub_identifier: Token::default(),
        }
    }

    /// Creates discovery result for UsdTransform2d shader.
    fn create_transform_2d() -> SdrShaderNodeDiscoveryResult {
        let mut metadata = SdrTokenMap::new();
        metadata.insert(Token::new("role"), "math".to_string());

        SdrShaderNodeDiscoveryResult {
            identifier: Token::new("UsdTransform2d"),
            version: SdrVersion::default(),
            name: "UsdTransform2d".to_string(),
            family: Token::new("math"),
            discovery_type: Token::new(USD_DISCOVERY_TYPE),
            source_type: Token::new(USD_SOURCE_TYPE),
            uri: "builtin://UsdTransform2d".to_string(),
            resolved_uri: "builtin://UsdTransform2d".to_string(),
            source_code: String::new(),
            metadata,
            blind_data: String::new(),
            sub_identifier: Token::default(),
        }
    }

    /// Creates discovery result for a primvar reader shader.
    fn create_primvar_reader(type_suffix: &str) -> SdrShaderNodeDiscoveryResult {
        let identifier = format!("UsdPrimvarReader_{}", type_suffix);
        let mut metadata = SdrTokenMap::new();
        metadata.insert(Token::new("role"), "primvar".to_string());

        SdrShaderNodeDiscoveryResult {
            identifier: Token::new(&identifier),
            version: SdrVersion::default(),
            name: identifier.clone(),
            family: Token::new("primvar"),
            discovery_type: Token::new(USD_DISCOVERY_TYPE),
            source_type: Token::new(USD_SOURCE_TYPE),
            uri: format!("builtin://{}", identifier),
            resolved_uri: format!("builtin://{}", identifier),
            source_code: String::new(),
            metadata,
            blind_data: String::new(),
            sub_identifier: Token::default(),
        }
    }
}

impl SdrDiscoveryPlugin for UsdShadersDiscoveryPlugin {
    fn discover_shader_nodes(
        &self,
        _context: &dyn SdrDiscoveryPluginContext,
    ) -> SdrShaderNodeDiscoveryResultVec {
        let mut results = Vec::new();

        // UsdPreviewSurface - main PBR surface shader
        results.push(Self::create_preview_surface());

        // UsdUVTexture - texture sampling
        results.push(Self::create_uv_texture());

        // UsdTransform2d - 2D coordinate transformation
        results.push(Self::create_transform_2d());

        // UsdPrimvarReader variants for different types
        let primvar_types = [
            "float", "float2", "float3", "float4", "int", "string", "normal", "point", "vector",
            "matrix",
        ];
        for type_suffix in primvar_types {
            results.push(Self::create_primvar_reader(type_suffix));
        }

        results
    }

    fn get_search_uris(&self) -> Vec<String> {
        // Built-in shaders don't have filesystem paths
        vec!["builtin://usdShaders".to_string()]
    }

    fn get_name(&self) -> &str {
        "UsdShadersDiscoveryPlugin"
    }
}

/// Registers the built-in USD shaders with the SDR registry.
///
/// Call this function to make UsdPreviewSurface, UsdUVTexture, and other
/// standard USD shaders available in the shader registry.
///
/// # Example
///
/// ```ignore
/// use usd_sdr::usd_shaders::register_usd_shaders;
/// use usd_sdr::SdrRegistry;
///
/// // Register built-in shaders
/// register_usd_shaders();
///
/// // Now they're available in the registry
/// let registry = SdrRegistry::get_instance();
/// let ids = registry.get_shader_node_identifiers(None, SdrVersionFilter::DefaultOnly);
/// assert!(ids.iter().any(|id| id.as_str() == "UsdPreviewSurface"));
/// ```
pub fn register_usd_shaders() {
    use super::discovery_plugin::DefaultDiscoveryPluginContext;
    use super::registry::SdrRegistry;

    let plugin = UsdShadersDiscoveryPlugin::new();
    let context = DefaultDiscoveryPluginContext;
    let results = plugin.discover_shader_nodes(&context);

    let registry = SdrRegistry::get_instance();
    for result in results {
        registry.add_discovery_result(result);
    }
}

// ============================================================================
// Parser Plugin - Creates full shader nodes with properties
// ============================================================================

/// Parser plugin for built-in USD shaders.
///
/// This parser creates full SdrShaderNode instances with all inputs and outputs
/// for the standard USD preview shaders.
pub struct UsdShadersParserPlugin;

impl Default for UsdShadersParserPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl UsdShadersParserPlugin {
    /// Creates a new USD shaders parser plugin.
    pub fn new() -> Self {
        Self
    }

    /// Creates the properties for UsdPreviewSurface.
    fn create_preview_surface_properties() -> Vec<SdrShaderPropertyUniquePtr> {
        let mut props = Vec::new();

        // Inputs
        props.push(Self::create_input(
            "diffuseColor",
            "color3f",
            Some("(0.18, 0.18, 0.18)"),
        ));
        props.push(Self::create_input(
            "emissiveColor",
            "color3f",
            Some("(0.0, 0.0, 0.0)"),
        ));
        props.push(Self::create_input("useSpecularWorkflow", "int", Some("0")));
        props.push(Self::create_input(
            "specularColor",
            "color3f",
            Some("(0.0, 0.0, 0.0)"),
        ));
        props.push(Self::create_input("metallic", "float", Some("0.0")));
        props.push(Self::create_input("roughness", "float", Some("0.5")));
        props.push(Self::create_input("clearcoat", "float", Some("0.0")));
        props.push(Self::create_input(
            "clearcoatRoughness",
            "float",
            Some("0.01"),
        ));
        props.push(Self::create_input("opacity", "float", Some("1.0")));
        props.push(Self::create_input("opacityThreshold", "float", Some("0.0")));
        props.push(Self::create_input("ior", "float", Some("1.5")));
        props.push(Self::create_input(
            "normal",
            "normal3f",
            Some("(0.0, 0.0, 1.0)"),
        ));
        props.push(Self::create_input("displacement", "float", Some("0.0")));
        props.push(Self::create_input("occlusion", "float", Some("1.0")));

        // Outputs
        props.push(Self::create_output("surface", "token"));
        props.push(Self::create_output("displacement", "token"));

        props
    }

    /// Creates the properties for UsdUVTexture.
    fn create_uv_texture_properties() -> Vec<SdrShaderPropertyUniquePtr> {
        let mut props = Vec::new();

        // Inputs
        props.push(Self::create_input("file", "asset", None));
        props.push(Self::create_input("st", "float2", Some("(0.0, 0.0)")));
        props.push(Self::create_input("wrapS", "token", Some("useMetadata")));
        props.push(Self::create_input("wrapT", "token", Some("useMetadata")));
        props.push(Self::create_input(
            "fallback",
            "float4",
            Some("(0.0, 0.0, 0.0, 1.0)"),
        ));
        props.push(Self::create_input(
            "scale",
            "float4",
            Some("(1.0, 1.0, 1.0, 1.0)"),
        ));
        props.push(Self::create_input(
            "bias",
            "float4",
            Some("(0.0, 0.0, 0.0, 0.0)"),
        ));
        props.push(Self::create_input(
            "sourceColorSpace",
            "token",
            Some("auto"),
        ));

        // Outputs
        props.push(Self::create_output("r", "float"));
        props.push(Self::create_output("g", "float"));
        props.push(Self::create_output("b", "float"));
        props.push(Self::create_output("a", "float"));
        props.push(Self::create_output("rgb", "float3"));

        props
    }

    /// Creates the properties for UsdTransform2d.
    fn create_transform_2d_properties() -> Vec<SdrShaderPropertyUniquePtr> {
        let mut props = Vec::new();

        // Inputs
        props.push(Self::create_input("in", "float2", Some("(0.0, 0.0)")));
        props.push(Self::create_input("rotation", "float", Some("0.0")));
        props.push(Self::create_input("scale", "float2", Some("(1.0, 1.0)")));
        props.push(Self::create_input(
            "translation",
            "float2",
            Some("(0.0, 0.0)"),
        ));

        // Outputs
        props.push(Self::create_output("result", "float2"));

        props
    }

    /// Creates the properties for a primvar reader.
    fn create_primvar_reader_properties(
        result_type: &str,
        default_value: Option<&str>,
    ) -> Vec<SdrShaderPropertyUniquePtr> {
        let mut props = Vec::new();

        // Inputs
        props.push(Self::create_input("varname", "string", Some("")));
        props.push(Self::create_input("fallback", result_type, default_value));

        // Outputs
        props.push(Self::create_output("result", result_type));

        props
    }

    /// Helper to create an input property.
    fn create_input(
        name: &str,
        type_name: &str,
        default_value: Option<&str>,
    ) -> SdrShaderPropertyUniquePtr {
        let mut legacy_metadata = SdrTokenMap::new();
        if let Some(val) = default_value {
            legacy_metadata.insert(Token::new("default"), val.to_string());
        }
        let metadata = SdrShaderPropertyMetadata::from_token_map(&legacy_metadata);

        // Convert default value string to Value
        let value = default_value
            .map(|s| Value::from(s.to_string()))
            .unwrap_or_default();

        Box::new(SdrShaderProperty::new(
            Token::new(name),
            Token::new(type_name),
            value,
            false, // is_output
            0,     // array_size
            metadata,
            SdrTokenMap::new(), // hints
            Vec::new(),         // options
        ))
    }

    /// Helper to create an output property.
    fn create_output(name: &str, type_name: &str) -> SdrShaderPropertyUniquePtr {
        let metadata = SdrShaderPropertyMetadata::new();

        Box::new(SdrShaderProperty::new(
            Token::new(name),
            Token::new(type_name),
            Value::default(),
            true, // is_output
            0,    // array_size
            metadata,
            SdrTokenMap::new(),
            Vec::new(),
        ))
    }

    /// Parse a specific shader by identifier.
    fn parse_shader(
        &self,
        identifier: &str,
        dr: &SdrShaderNodeDiscoveryResult,
    ) -> Option<SdrShaderNodeUniquePtr> {
        let properties = match identifier {
            "UsdPreviewSurface" => Self::create_preview_surface_properties(),
            "UsdUVTexture" => Self::create_uv_texture_properties(),
            "UsdTransform2d" => Self::create_transform_2d_properties(),
            id if id.starts_with("UsdPrimvarReader_") => {
                let type_suffix = &id["UsdPrimvarReader_".len()..];
                let (result_type, default) = match type_suffix {
                    "float" => ("float", Some("0.0")),
                    "float2" => ("float2", Some("(0.0, 0.0)")),
                    "float3" => ("float3", Some("(0.0, 0.0, 0.0)")),
                    "float4" => ("float4", Some("(0.0, 0.0, 0.0, 0.0)")),
                    "int" => ("int", Some("0")),
                    "string" => ("string", Some("")),
                    "normal" => ("normal3f", Some("(0.0, 0.0, 0.0)")),
                    "point" => ("point3f", Some("(0.0, 0.0, 0.0)")),
                    "vector" => ("vector3f", Some("(0.0, 0.0, 0.0)")),
                    "matrix" => (
                        "matrix4d",
                        Some("((1,0,0,0),(0,1,0,0),(0,0,1,0),(0,0,0,1))"),
                    ),
                    _ => return None,
                };
                Self::create_primvar_reader_properties(result_type, default)
            }
            _ => return None,
        };

        // Determine context based on shader type
        let context = match identifier {
            "UsdPreviewSurface" => Token::new("surface"),
            "UsdUVTexture" => Token::new("texture"),
            "UsdTransform2d" => Token::new("math"),
            id if id.starts_with("UsdPrimvarReader_") => Token::new("primvar"),
            _ => Token::default(),
        };

        let metadata = SdrShaderNodeMetadata::from_token_map(&dr.metadata);

        let node = SdrShaderNode::new(
            dr.identifier.clone(),
            dr.version,
            dr.name.clone(),
            dr.family.clone(),
            context,
            dr.source_type.clone(),
            dr.uri.clone(),
            dr.resolved_uri.clone(),
            properties,
            metadata,
            dr.source_code.clone(),
        );

        Some(Box::new(node))
    }
}

impl SdrParserPlugin for UsdShadersParserPlugin {
    fn parse_shader_node(
        &self,
        discovery_result: &SdrShaderNodeDiscoveryResult,
    ) -> Option<SdrShaderNodeUniquePtr> {
        // Only handle builtin:// URIs
        if !discovery_result.uri.starts_with("builtin://") {
            return None;
        }

        self.parse_shader(discovery_result.identifier.as_str(), discovery_result)
    }

    fn get_discovery_types(&self) -> SdrTokenVec {
        vec![Token::new(USD_DISCOVERY_TYPE)]
    }

    fn get_source_type(&self) -> Token {
        Token::new(USD_SOURCE_TYPE)
    }

    fn get_name(&self) -> &str {
        "UsdShadersParserPlugin"
    }
}

/// Registers the built-in USD shaders parser plugin with the SDR registry.
///
/// This enables the registry to parse builtin:// shader URIs into full nodes.
pub fn register_usd_shaders_parser() {
    use super::registry::SdrRegistry;

    let registry = SdrRegistry::get_instance();
    registry.register_parser_plugin(Box::new(UsdShadersParserPlugin::new()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discovery_plugin() {
        let plugin = UsdShadersDiscoveryPlugin::new();
        let context = super::super::discovery_plugin::DefaultDiscoveryPluginContext;
        let results = plugin.discover_shader_nodes(&context);

        // Should have 13 shaders: 1 surface + 1 texture + 1 transform + 10 primvar readers
        assert_eq!(results.len(), 13);

        // Check UsdPreviewSurface
        let preview_surface = results.iter().find(|r| r.identifier == "UsdPreviewSurface");
        assert!(preview_surface.is_some());
        let ps = preview_surface.unwrap();
        assert_eq!(ps.family.as_str(), "surface");
        assert_eq!(ps.source_type.as_str(), USD_SOURCE_TYPE);

        // Check UsdUVTexture
        let uv_texture = results.iter().find(|r| r.identifier == "UsdUVTexture");
        assert!(uv_texture.is_some());

        // Check primvar readers
        let primvar_float = results
            .iter()
            .find(|r| r.identifier == "UsdPrimvarReader_float");
        assert!(primvar_float.is_some());

        let primvar_matrix = results
            .iter()
            .find(|r| r.identifier == "UsdPrimvarReader_matrix");
        assert!(primvar_matrix.is_some());
    }

    #[test]
    fn test_search_uris() {
        let plugin = UsdShadersDiscoveryPlugin::new();
        let uris = plugin.get_search_uris();
        assert_eq!(uris.len(), 1);
        assert!(uris[0].starts_with("builtin://"));
    }
}
