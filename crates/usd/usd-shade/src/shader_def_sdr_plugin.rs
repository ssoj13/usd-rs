//! [`SdrParserPlugin`] for shader definitions authored as USD prims (`UsdShadeShader`).
//!
//! Matches C++ `UsdShadeShaderDefParserPlugin`.

use crate::shader_def_parser::parse_shader_node;
use crate::shader_def_utils::{self, ShaderPropertyInfo};
use std::collections::HashMap;
use usd_sdr::declare::{SdrOptionVec, SdrTokenMap};
use usd_sdr::discovery_result::SdrShaderNodeDiscoveryResult;
use usd_sdr::parser_plugin::SdrParserPlugin;
use usd_sdr::shader_node::SdrShaderNode;
use usd_sdr::shader_node_metadata::SdrShaderNodeMetadata;
use usd_sdr::shader_property::SdrShaderProperty;
use usd_sdr::shader_property_metadata::SdrShaderPropertyMetadata;
use usd_sdr::{SdrShaderNodeUniquePtr, SdrShaderPropertyUniquePtrVec, SdrTokenVec};
use usd_tf::Token;

fn shader_property_info_to_sdr(info: &ShaderPropertyInfo) -> SdrShaderProperty {
    let (sdr_type, array_size) =
        shader_def_utils::get_sdr_property_type_and_array_size(&info.type_name);
    let prop_type = Token::new(sdr_type.as_str());
    let meta = SdrShaderPropertyMetadata::from_token_map(&info.metadata);
    let default = info.default_value.clone().unwrap_or_default();
    let hints: SdrTokenMap = HashMap::new();
    let options: SdrOptionVec = Vec::new();
    SdrShaderProperty::new(
        info.name.clone(),
        prop_type,
        default,
        info.is_output,
        array_size,
        meta,
        hints,
        options,
    )
}

fn build_sdr_shader_node(
    dr: &SdrShaderNodeDiscoveryResult,
    parsed: crate::shader_def_parser::ShaderDefParseResult,
) -> SdrShaderNodeUniquePtr {
    let node_meta = SdrShaderNodeMetadata::from_token_map(&parsed.shader_node_metadata);
    let context = Token::new(
        parsed
            .shader_node_metadata
            .get(&Token::new("context"))
            .map(|s| s.as_str())
            .unwrap_or(""),
    );
    let properties: SdrShaderPropertyUniquePtrVec = parsed
        .properties
        .iter()
        .map(|p| Box::new(shader_property_info_to_sdr(p)))
        .collect();
    let source_code = std::fs::read_to_string(&parsed.resolved_impl_uri).unwrap_or_default();
    Box::new(SdrShaderNode::new(
        dr.identifier.clone(),
        dr.version,
        dr.name.clone(),
        dr.family.clone(),
        context,
        dr.source_type.clone(),
        dr.resolved_uri.clone(),
        parsed.resolved_impl_uri,
        properties,
        node_meta,
        source_code,
    ))
}

/// Parser plugin that turns `UsdShadeShader` discovery results into [`SdrShaderNode`].
pub struct UsdShadeShaderDefParserPlugin;

impl Default for UsdShadeShaderDefParserPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl UsdShadeShaderDefParserPlugin {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl SdrParserPlugin for UsdShadeShaderDefParserPlugin {
    fn parse_shader_node(
        &self,
        discovery_result: &SdrShaderNodeDiscoveryResult,
    ) -> Option<SdrShaderNodeUniquePtr> {
        let parsed = parse_shader_node(
            &discovery_result.identifier,
            &discovery_result.sub_identifier,
            discovery_result.resolved_uri.as_str(),
            &discovery_result.source_type,
        )
        .ok()?;
        Some(build_sdr_shader_node(discovery_result, parsed))
    }

    fn get_discovery_types(&self) -> SdrTokenVec {
        crate::shader_def_parser::get_discovery_types()
    }

    fn get_source_type(&self) -> Token {
        crate::shader_def_parser::get_source_type()
    }

    fn get_name(&self) -> &str {
        "UsdShadeShaderDefParserPlugin"
    }
}
