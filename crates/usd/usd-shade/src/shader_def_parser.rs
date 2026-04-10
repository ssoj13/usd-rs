//! USD Shade ShaderDefParser - parser plugin for shader definitions.
//!
//! Port of pxr/usd/usdShade/shaderDefParser.h and shaderDefParser.cpp
//!
//! Parses shader definitions represented using USD scene description using the
//! schemas provided by UsdShade. Opens the USD layer, finds the shader def prim
//! by identifier, extracts properties and metadata.

use super::node_def_api::NodeDefAPI;
use super::shader::Shader;
use super::shader_def_utils;
use std::sync::Arc;
use usd_core::{InitialLoadSet, Stage};
use usd_sdf::Path;
use usd_sdr::declare::SdrTokenMap;
use usd_tf::Token;

/// Discovery types supported by the shader definition parser.
///
/// Matches C++ `GetDiscoveryTypes()`.
pub fn get_discovery_types() -> Vec<Token> {
    vec![Token::new("usda"), Token::new("usdc"), Token::new("usd")]
}

/// Source type for this parser plugin.
///
/// Matches C++ `GetSourceType()`.
/// Empty because this parser can generate nodes of any sourceType.
pub fn get_source_type() -> Token {
    Token::new("")
}

/// Result of parsing a shader definition from a USD file.
///
/// Contains all information needed to construct an SdrShaderNode.
#[derive(Debug, Clone)]
pub struct ShaderDefParseResult {
    /// Identifier of the shader node.
    pub identifier: Token,
    /// Extracted shader properties (inputs and outputs).
    pub properties: Vec<shader_def_utils::ShaderPropertyInfo>,
    /// Primvar names metadata string.
    pub metadata_str: String,
    /// Flattened `sdrMetadata` (plus `primvars`) for constructing [`usd_sdr::SdrShaderNode`].
    pub shader_node_metadata: SdrTokenMap,
    /// Source type for the shader node.
    pub source_type: Token,
    /// Resolved URI of the implementation (the actual shader source file).
    pub resolved_impl_uri: String,
    /// Path to the root USD layer that contains the shader definition.
    pub root_layer_path: String,
}

/// Parse a shader node from a discovery result.
///
/// Opens the USD file at `resolved_uri`, locates the shader def prim by
/// `identifier` (falling back to `sub_identifier`), extracts properties and
/// metadata, and returns a `ShaderDefParseResult`.
///
/// Matches C++ `UsdShadeShaderDefParserPlugin::ParseShaderNode()`.
pub fn parse_shader_node(
    identifier: &Token,
    sub_identifier: &Token,
    resolved_uri: &str,
    source_type: &Token,
) -> Result<ShaderDefParseResult, String> {
    // Open the USD stage from the layer on disk.
    let stage = Stage::open(resolved_uri, InitialLoadSet::LoadAll)
        .map_err(|e| format!("Failed to open stage '{}': {}", resolved_uri, e))?;

    // Find the shader def prim: try identifier first, then sub_identifier.
    let shader_def = find_shader_def(&stage, identifier)
        .or_else(|| {
            if !sub_identifier.is_empty() && sub_identifier != identifier {
                find_shader_def(&stage, sub_identifier)
            } else {
                None
            }
        })
        .ok_or_else(|| {
            format!(
                "No shader def prim found for identifier '{}' in '{}'",
                identifier.as_str(),
                resolved_uri
            )
        })?;

    // Get the source asset (resolved implementation URI).
    let node_def = NodeDefAPI::new(shader_def.get_prim());
    let asset_path = node_def
        .get_source_asset(Some(source_type))
        .ok_or_else(|| {
            format!(
                "No sourceAsset for sourceType '{}' in shader '{}'",
                source_type.as_str(),
                identifier.as_str()
            )
        })?;

    let resolved_impl_uri = asset_path.get_resolved_path().to_string();
    if resolved_impl_uri.is_empty() {
        return Err(format!(
            "Unable to resolve path '{}' in shader definition file '{}'",
            asset_path.get_asset_path(),
            resolved_uri
        ));
    }

    // Extract properties from the connectable API.
    let connectable = shader_def.connectable_api();
    let properties = shader_def_utils::get_properties(&connectable);

    // Build metadata: shader sdr metadata + primvar names string.
    let sdr_metadata = shader_def.get_sdr_metadata();
    let metadata_str = shader_def_utils::get_primvar_names_metadata_string(
        &sdr_metadata
            .iter()
            .map(|(k, v)| (Token::new(k.as_str()), v.clone()))
            .collect(),
        &connectable,
    );

    let mut shader_node_metadata: SdrTokenMap = sdr_metadata
        .iter()
        .map(|(k, v)| (Token::new(k.as_str()), v.clone()))
        .collect();
    if !metadata_str.is_empty() {
        shader_node_metadata.insert(Token::new("primvars"), metadata_str.clone());
    }

    Ok(ShaderDefParseResult {
        identifier: identifier.clone(),
        properties,
        metadata_str,
        shader_node_metadata,
        source_type: source_type.clone(),
        resolved_impl_uri,
        root_layer_path: resolved_uri.to_string(),
    })
}

/// Find the shader def prim at `/<identifier>` on the stage.
fn find_shader_def(stage: &Arc<Stage>, identifier: &Token) -> Option<Shader> {
    if identifier.is_empty() {
        return None;
    }
    // Shader defs are authored at the absolute root child path /<Identifier>.
    let path = Path::absolute_root().append_child(identifier.as_str())?;
    let prim = stage.get_prim_at_path(&path)?;
    let shader = Shader::new(prim);
    if shader.is_valid() {
        Some(shader)
    } else {
        None
    }
}
