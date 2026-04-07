//! USD Shade ShaderDefUtils - utilities for shader definitions.
//!
//! Port of pxr/usd/usdShade/shaderDefUtils.h and shaderDefUtils.cpp
//!
//! This class contains a set of utility functions used for populating the
//! shader registry with shaders definitions specified using UsdShade schemas.

use super::connectable_api::ConnectableAPI;
use super::node_def_api::NodeDefAPI;
use super::shader::Shader;
use std::collections::HashMap;
use usd_sdf::AssetPath;
use usd_sdf::Path;
use usd_sdr::{
    SdrShaderNodeDiscoveryResult, SdrShaderNodeDiscoveryResultVec, declare::SdrVersion,
    filesystem_discovery_helpers::split_shader_identifier,
};
use usd_tf::Token;

/// Property information extracted from a shader definition.
#[derive(Debug, Clone)]
pub struct ShaderPropertyInfo {
    /// Property name.
    pub name: Token,
    /// Property type name (as string).
    pub type_name: String,
    /// Default value (if any).
    pub default_value: Option<usd_vt::Value>,
    /// Whether this is an output property.
    pub is_output: bool,
    /// Array size (0 if not an array).
    pub array_size: usize,
    /// Metadata for the property.
    pub metadata: HashMap<Token, String>,
}

/// Gets all input and output properties of the given shaderDef and
/// translates them into ShaderPropertyInfo objects.
///
/// Convert SDF value type name to SDR property type and array size.
///
/// Matches C++ `_GetShaderPropertyTypeAndArraySize()` from shaderDefUtils.cpp.
/// Maps SDF types (color3f, float, bool, token, etc.) to SDR types
/// (color, float, int, string, etc.) and determines the array size for
/// fixed-size vector types.
pub fn get_sdr_property_type_and_array_size(sdf_type_name: &str) -> (String, usize) {
    match sdf_type_name {
        // Int / Bool → "int"
        "int" | "int[]" | "bool" | "bool[]" => ("int".to_string(), 0),
        "int2" | "int2[]" => ("int".to_string(), 2),
        "int3" | "int3[]" => ("int".to_string(), 3),
        "int4" | "int4[]" => ("int".to_string(), 4),
        // String / Token / Asset → "string"
        "string" | "string[]" | "token" | "token[]" | "asset" | "asset[]" => {
            ("string".to_string(), 0)
        }
        // Float
        "float" | "float[]" | "double" | "double[]" | "half" | "half[]" => {
            ("float".to_string(), 0)
        }
        "float2" | "float2[]" | "double2" | "double2[]" | "half2" | "half2[]" => {
            ("float".to_string(), 2)
        }
        "float3" | "float3[]" | "double3" | "double3[]" | "half3" | "half3[]" => {
            ("float".to_string(), 3)
        }
        "float4" | "float4[]" | "double4" | "double4[]" | "half4" | "half4[]" => {
            ("float".to_string(), 4)
        }
        // Color3f → "color"
        "color3f" | "color3f[]" | "color3d" | "color3d[]" | "color3h" | "color3h[]" => {
            ("color".to_string(), 0)
        }
        // Color4f → "color4"
        "color4f" | "color4f[]" | "color4d" | "color4d[]" | "color4h" | "color4h[]" => {
            ("color4".to_string(), 0)
        }
        // Point3f → "point"
        "point3f" | "point3f[]" | "point3d" | "point3d[]" | "point3h" | "point3h[]" => {
            ("point".to_string(), 0)
        }
        // Vector3f → "vector"
        "vector3f" | "vector3f[]" | "vector3d" | "vector3d[]" | "vector3h" | "vector3h[]" => {
            ("vector".to_string(), 0)
        }
        // Normal3f → "normal"
        "normal3f" | "normal3f[]" | "normal3d" | "normal3d[]" | "normal3h" | "normal3h[]" => {
            ("normal".to_string(), 0)
        }
        // Matrix4d → "matrix"
        "matrix4d" | "matrix4d[]" | "matrix2d" | "matrix2d[]" | "matrix3d" | "matrix3d[]" => {
            ("matrix".to_string(), 0)
        }
        // Unknown → fall through with original name
        other => (other.to_string(), 0),
    }
}

/// Matches C++ `GetProperties(const UsdShadeConnectableAPI &shaderDef)`.
/// Note: This is a simplified version that doesn't use Sdr types.
pub fn get_properties(shader_def: &ConnectableAPI) -> Vec<ShaderPropertyInfo> {
    let mut result = Vec::new();

    // Get inputs
    let inputs = shader_def.get_inputs(false);
    for input in inputs {
        let default_value = input.get(usd_sdf::TimeCode::default());
        let metadata = input.get_sdr_metadata();

        let type_name = input.get_type_name();
        let type_name_str = type_name.name().as_str().to_string();

        // Determine array size
        let array_size = if type_name_str.contains("Array") {
            // Try to get array size from default value if available
            if let Some(ref _val) = default_value {
                // Simplified - would need proper array size detection
                0 // Will be determined by actual array size if available
            } else {
                0
            }
        } else {
            0
        };

        result.push(ShaderPropertyInfo {
            name: input.get_base_name(),
            type_name: type_name_str,
            default_value: default_value.clone(),
            is_output: false,
            array_size,
            metadata: metadata.clone(),
        });
    }

    // Get outputs
    let outputs = shader_def.get_outputs(false);
    for output in outputs {
        let metadata = output.get_sdr_metadata();
        let type_name = output.get_type_name();
        let type_name_str = type_name.name().as_str().to_string();

        // Determine array size
        let array_size = if type_name_str.contains("Array") {
            0 // Will be determined by actual array size if available
        } else {
            0
        };

        result.push(ShaderPropertyInfo {
            name: output.get_base_name(),
            type_name: type_name_str,
            default_value: None,
            is_output: true,
            array_size,
            metadata: metadata.clone(),
        });
    }

    result
}

/// Collects all the names of valid primvar inputs of the given metadata
/// and the given shaderDef and returns the string used to represent
/// them in shader node metadata.
///
/// Matches C++ `GetPrimvarNamesMetadataString(const SdrTokenMap metadata, const UsdShadeConnectableAPI &shaderDef)`.
pub fn get_primvar_names_metadata_string(
    metadata: &HashMap<Token, String>,
    shader_def: &ConnectableAPI,
) -> String {
    // If there's an existing value in the definition, we must append to it.
    let mut primvar_names = Vec::new();

    if let Some(existing_value) = metadata.get(&Token::new("primvars")) {
        // Only append if it's non-empty
        if !existing_value.is_empty() {
            primvar_names.push(existing_value.clone());
        }
    }

    // Check for primvarProperty metadata on inputs
    let primvar_property_token = Token::new("primvarProperty");

    for input in shader_def.get_inputs(false) {
        if input.has_sdr_metadata_by_key(&primvar_property_token) {
            // Check if the input is string-valued
            let type_name = input.get_type_name();
            let type_name_token = type_name.name();
            let type_name_str = type_name_token.as_str();

            // Simplified check - in full implementation would check SdrPropertyTypes
            if type_name_str.contains("String") || type_name_str.contains("Token") {
                let base_name = input.get_base_name();
                primvar_names.push(format!("${}", base_name.as_str()));
            } else {
                eprintln!(
                    "Shader input <{}> is tagged as a primvarProperty, but isn't string-valued.",
                    input.as_attribute().path()
                );
            }
        }
    }

    primvar_names.join("|")
}

/// Gets the source asset path for a shader definition.
///
/// Simplified version of discovery result generation.
/// Matches part of C++ `GetDiscoveryResults(const UsdShadeShader &shaderDef, const std::string &sourceUri)`.
pub fn get_source_asset(shader_def: &Shader, source_type: &Token) -> Option<AssetPath> {
    let node_def_api = NodeDefAPI::new(shader_def.get_prim());
    node_def_api.get_source_asset(Some(source_type))
}

/// Returns discovery results for a shader definition.
///
/// This function inspects a UsdShadeShader prim and generates discovery results
/// that can be registered with the Sdr shader registry.
///
/// Matches C++ `GetDiscoveryResults(const UsdShadeShader &shaderDef, const std::string &sourceUri)`.
///
/// # Arguments
/// * `shader_def` - The shader definition prim to inspect
/// * `source_uri` - The source URI of the shader definition file
///
/// # Returns
/// A vector of discovery results for each source asset found in the shader definition.
/// Returns empty if implementation source is not "sourceAsset".
pub fn get_discovery_results(
    shader_def: &Shader,
    source_uri: &str,
) -> SdrShaderNodeDiscoveryResultVec {
    let mut result = SdrShaderNodeDiscoveryResultVec::new();

    // Implementation source must be sourceAsset for the shader to represent
    // nodes in Sdr.
    let impl_source = shader_def.get_implementation_source();
    if impl_source != "sourceAsset" {
        return result;
    }

    let shader_def_prim = shader_def.get_prim();
    let identifier = shader_def_prim.name();

    // Get the family name, shader name and version information from the
    // identifier.
    let mut family = Token::default();
    let mut name = Token::default();
    let mut version = SdrVersion::default();

    if !split_shader_identifier(&identifier, &mut family, &mut name, &mut version) {
        // A warning has already been issued by split_shader_identifier.
        return result;
    }

    const INFO_NAMESPACE: &str = "info:";
    const BASE_SOURCE_ASSET: &str = ":sourceAsset";

    // This vector will contain all the info:*:sourceAsset properties.
    let source_asset_properties: Vec<_> = shader_def_prim
        .get_authored_properties()
        .into_iter()
        .filter(|prop| {
            let property_name = prop.name();
            let property_name_str = property_name.as_str();
            property_name_str.starts_with(INFO_NAMESPACE)
                && property_name_str.ends_with(BASE_SOURCE_ASSET)
        })
        .collect();

    // Get discovery type from source URI extension using ArGetResolver().GetExtension()
    // equivalent in Rust.
    let discovery_type = Token::new(
        std::path::Path::new(source_uri)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or(""),
    );

    for prop in source_asset_properties {
        // Convert property to attribute
        let Some(attr) = prop.as_attribute() else {
            continue;
        };

        // Get the asset path value
        let Some(value) = attr.get(usd_sdf::TimeCode::default()) else {
            continue;
        };

        // Try to extract AssetPath from the value
        let source_asset_path = if let Some(ap) = value.downcast::<AssetPath>() {
            ap.clone()
        } else {
            continue;
        };

        if source_asset_path.get_asset_path().is_empty() {
            continue;
        }

        // Tokenize the attribute name: "info:sourceType:sourceAsset" -> ["info", "sourceType", "sourceAsset"]
        let name_tokens = Path::tokenize_identifier_as_tokens(attr.name().as_str());
        if name_tokens.len() != 3 {
            continue;
        }

        let resolved_uri = source_asset_path.get_resolved_path();

        // Create a discoveryResult only if the referenced sourceAsset
        // can be resolved.
        // XXX: Should we do this regardless and expect the parser to be
        // able to resolve the unresolved asset path?
        if !resolved_uri.is_empty() {
            let source_type = name_tokens[1].clone();

            // Use the prim name as the identifier since it is
            // guaranteed to be unique in the file.
            // Use the shader id as the name of the shader.
            result.push(SdrShaderNodeDiscoveryResult::new(
                identifier.clone(),
                version.as_default(),
                name.as_str().to_string(),
                family.clone(),
                discovery_type.clone(),
                source_type,
                /* uri */ source_uri.to_string(),
                /* resolved_uri */ source_uri.to_string(),
                /* source_code */ String::new(),
                /* metadata */ usd_sdr::declare::SdrTokenMap::new(),
                /* blind_data */ String::new(),
                /* sub_identifier */ Token::default(),
            ));
        } else {
            eprintln!(
                "Unable to resolve info:sourceAsset <{}> with value @{}@.",
                attr.path(),
                source_asset_path.get_asset_path()
            );
        }
    }

    result
}
