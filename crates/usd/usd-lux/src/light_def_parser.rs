//! UsdLux_LightDefParserPlugin - light definition parser.
//!
//! Port of pxr/usd/usdLux/lightDefParser.cpp
//!
//! Parses shader definitions from generatedSchema.usda for the UsdLux
//! intrinsic concrete light types. Creates SdrShaderNode representations
//! for lights so they appear in the shader registry.

use std::collections::HashMap;
use std::sync::LazyLock;

use usd_core::{InitialLoadSet, Stage};
use usd_sdf::{Layer, Path, SpecType};
use usd_sdr::discovery_result::SdrShaderNodeDiscoveryResult;
use usd_sdr::parser_plugin::SdrParserPlugin;
use usd_sdr::shader_node::{SdrShaderNode, SdrShaderNodeUniquePtr};
use usd_sdr::shader_node_metadata::SdrShaderNodeMetadata;
use usd_sdr::shader_property::SdrShaderProperty;
use usd_sdr::shader_property_metadata::SdrShaderPropertyMetadata;
use usd_sdr::{SdrTokenMap, SdrTokenVec};
use usd_shade::{ConnectableAPI, get_properties};
use usd_tf::Token;

struct LightDefTokens {
    source_type: Token,
    discovery_type: Token,
    mesh_light: Token,
    mesh_light_api: Token,
    light_api: Token,
    shadow_api: Token,
    shaping_api: Token,
    volume_light: Token,
    volume_light_api: Token,
}

static TOKENS: LazyLock<LightDefTokens> = LazyLock::new(|| LightDefTokens {
    source_type: Token::new("USD"),
    discovery_type: Token::new("usd-schema-gen"),
    mesh_light: Token::new("MeshLight"),
    mesh_light_api: Token::new("MeshLightAPI"),
    light_api: Token::new("LightAPI"),
    shadow_api: Token::new("ShadowAPI"),
    shaping_api: Token::new("ShapingAPI"),
    volume_light: Token::new("VolumeLight"),
    volume_light_api: Token::new("VolumeLightAPI"),
});

type ShaderIdToApiTypeNameMap = HashMap<Token, Token>;

fn get_shader_id_to_api_type_name_map() -> &'static ShaderIdToApiTypeNameMap {
    static MAP: LazyLock<ShaderIdToApiTypeNameMap> = LazyLock::new(|| {
        let mut m = HashMap::new();
        m.insert(TOKENS.mesh_light.clone(), TOKENS.mesh_light_api.clone());
        m.insert(TOKENS.volume_light.clone(), TOKENS.volume_light_api.clone());
        m
    });
    &MAP
}

/// Open the generatedSchema.usda for UsdLux.
///
/// Matches C++ `_GetGeneratedSchema()`.
fn get_generated_schema() -> Option<std::sync::Arc<Layer>> {
    let schema_path = format!("{}/generatedSchema.usda", env!("CARGO_MANIFEST_DIR"));
    // init SDF file formats so we can read USDA
    usd_sdf::init();
    Layer::find_or_open(&schema_path).ok()
}

/// Copy properties from a schema prim in generatedSchema to a destination layer.
///
/// Matches C++ `_CopyPropertiesFromSchema()`.
fn copy_properties_from_schema(
    schema_layer: &std::sync::Arc<Layer>,
    schema_name: &Token,
    dest_layer: &std::sync::Arc<Layer>,
    dest_prim_name: &str,
) -> bool {
    let schema_path = Path::from(format!("/{}", schema_name.as_str()).as_str());
    let dest_path = Path::from(format!("/{}", dest_prim_name).as_str());

    if let Some(schema_prim) = schema_layer.get_prim_at_path(&schema_path) {
        for prop in schema_prim.properties() {
            let prop_name = prop.name();
            let src_prop_path = schema_path.append_property(prop_name.as_str());
            let dest_prop_path = dest_path.append_property(prop_name.as_str());
            if let (Some(src_p), Some(dest_p)) = (src_prop_path, dest_prop_path) {
                if !usd_sdf::copy_spec(schema_layer, &src_p, dest_layer, &dest_p) {
                    log::error!(
                        "Could not copy property '{}' from schema '{}'",
                        prop_name.as_str(),
                        schema_name.as_str()
                    );
                    return false;
                }
            }
        }
        true
    } else {
        log::warn!(
            "generatedSchema does not have prim spec for '{}'",
            schema_name.as_str()
        );
        false
    }
}

/// Light definition parser plugin.
///
/// Matches C++ `UsdLux_LightDefParserPlugin`.
pub struct LightDefParserPlugin;

impl LightDefParserPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LightDefParserPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl SdrParserPlugin for LightDefParserPlugin {
    /// Parse a shader node from a discovery result.
    ///
    /// Matches C++ `UsdLux_LightDefParserPlugin::ParseShaderNode()`.
    fn parse_shader_node(
        &self,
        discovery_result: &SdrShaderNodeDiscoveryResult,
    ) -> Option<SdrShaderNodeUniquePtr> {
        let api_map = get_shader_id_to_api_type_name_map();

        // Resolve prim type name for API schemas
        let prim_type_name = api_map
            .get(&discovery_result.identifier)
            .cloned()
            .unwrap_or_else(|| discovery_result.identifier.clone());

        // Open generatedSchema.usda
        let schema_layer = get_generated_schema()?;

        // Create anonymous layer with a prim to compose properties into
        let layer = Layer::create_anonymous(Some("lightDef.usd"));
        let prim_name = prim_type_name.as_str();
        let prim_path = Path::from(format!("/{}", prim_name).as_str());
        layer.create_spec(&prim_path, SpecType::Prim);

        // Copy properties from each schema (order matters)
        let schemas = [
            TOKENS.light_api.clone(),
            prim_type_name.clone(),
            TOKENS.shadow_api.clone(),
            TOKENS.shaping_api.clone(),
        ];
        for schema_name in &schemas {
            copy_properties_from_schema(&schema_layer, schema_name, &layer, prim_name);
        }

        // Open as stage and get ConnectableAPI
        let stage = Stage::open_with_root_layer(layer, InitialLoadSet::LoadAll).ok()?;
        let prim = stage.get_prim_at_path(&prim_path)?;
        let connectable = ConnectableAPI::new(prim);

        // Get shader properties
        let property_infos = get_properties(&connectable);

        // Convert to SdrShaderProperty
        let sdr_properties: Vec<Box<SdrShaderProperty>> = property_infos
            .iter()
            .map(|info| {
                let default_value = info.default_value.clone().unwrap_or_default();
                let sdr_metadata = SdrShaderPropertyMetadata::from_token_map(
                    &info
                        .metadata
                        .iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect::<SdrTokenMap>(),
                );

                Box::new(SdrShaderProperty::new(
                    info.name.clone(),
                    Token::new(&info.type_name),
                    default_value,
                    info.is_output,
                    info.array_size,
                    sdr_metadata,
                    SdrTokenMap::new(),
                    Vec::new(),
                ))
            })
            .collect();

        // Build metadata
        let mut metadata_map = discovery_result.metadata.clone();
        metadata_map.insert(
            Token::new("help"),
            format!(
                "Fallback shader node generated from the USD {} schema",
                prim_type_name.as_str()
            ),
        );
        let metadata = SdrShaderNodeMetadata::from_token_map(&metadata_map);

        Some(Box::new(SdrShaderNode::new(
            discovery_result.identifier.clone(),
            discovery_result.version,
            discovery_result.name.clone(),
            discovery_result.family.clone(),
            Token::new("light"),
            discovery_result.source_type.clone(),
            String::new(),
            String::new(),
            sdr_properties,
            metadata,
            discovery_result.source_code.clone(),
        )))
    }

    fn get_discovery_types(&self) -> SdrTokenVec {
        vec![TOKENS.discovery_type.clone()]
    }

    fn get_source_type(&self) -> Token {
        TOKENS.source_type.clone()
    }

    fn get_name(&self) -> &str {
        "UsdLux_LightDefParserPlugin"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_type() {
        let parser = LightDefParserPlugin::new();
        assert_eq!(parser.get_source_type().as_str(), "USD");
    }

    #[test]
    fn test_discovery_types() {
        let parser = LightDefParserPlugin::new();
        let types = parser.get_discovery_types();
        assert_eq!(types.len(), 1);
        assert_eq!(types[0].as_str(), "usd-schema-gen");
    }

    #[test]
    fn test_shader_id_to_api_map() {
        let map = get_shader_id_to_api_type_name_map();
        assert_eq!(
            map.get(&Token::new("MeshLight")).map(|t| t.as_str()),
            Some("MeshLightAPI")
        );
        assert_eq!(
            map.get(&Token::new("VolumeLight")).map(|t| t.as_str()),
            Some("VolumeLightAPI")
        );
        assert!(map.get(&Token::new("RectLight")).is_none());
    }
}
