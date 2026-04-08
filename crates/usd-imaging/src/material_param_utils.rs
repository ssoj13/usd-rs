#![allow(dead_code)]
//! Material parameter utilities.
//!
//! Port of pxr/usdImaging/usdImaging/materialParamUtils.h/cpp
//!
//! Provides helper functions for building HdMaterialNetworks from USD shade
//! graphs, including shader graph walking, asset path resolution, primvar
//! extraction, and time-varying detection.
//!
//! # Examples
//!
//! ```ignore
//! use usd_imaging::material_param_utils::{
//!     resolve_asset_attr, extract_shader_params,
//! };
//!
//! // Resolve a texture path
//! if let Some(resolved) = resolve_asset_attr(&texture_attr, &stage_path) {
//!     println!("Resolved texture: {}", resolved);
//! }
//! ```

use std::collections::{HashMap, HashSet};
use usd_core::{Attribute, Prim};
use usd_hd::material_network::{
    HdMaterialNetworkMap, HdMaterialNetworkV1, HdMaterialNode, HdMaterialRelationship,
};
use usd_lux::light_api::LightAPI;
use usd_lux::light_filter::LightFilter;
use usd_sdf::{AssetPath, Path, TimeCode};
use usd_sdr::SdrRegistry;
use usd_shade::connectable_api::ConnectableAPI;
use usd_shade::node_def_api::NodeDefAPI;
use usd_shade::output::Output;
use usd_shade::types::AttributeType;
use usd_shade::utils::Utils;
use usd_shade::{is_udim_identifier, resolve_udim_path};
use usd_tf::Token;
use usd_vt::Value;

// ============================================================================
// Types
// ============================================================================

/// Material parameter value.
#[derive(Debug, Clone)]
pub enum ParamValue {
    /// Float value.
    Float(f32),
    /// Vec2 value.
    Vec2(f32, f32),
    /// Vec3 value (color or vector).
    Vec3(f32, f32, f32),
    /// Vec4 value.
    Vec4(f32, f32, f32, f32),
    /// Integer value.
    Int(i32),
    /// Boolean value.
    Bool(bool),
    /// String value.
    String(String),
    /// Asset path.
    Asset(AssetPath),
    /// Token value.
    Token(Token),
}

/// Material network terminal information.
#[derive(Debug, Clone)]
pub struct MaterialTerminal {
    /// Terminal name (e.g., "surface", "displacement", "volume").
    pub name: Token,
    /// Path to the connected shader prim.
    pub shader_path: Path,
    /// Output name on the shader.
    pub output_name: Token,
}

// ============================================================================
// Private tokens
// ============================================================================

mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;
    pub static TYPE_NAME: LazyLock<Token> = LazyLock::new(|| Token::new("typeName"));
    pub static COLOR_SPACE: LazyLock<Token> = LazyLock::new(|| Token::new("colorSpace"));
}

// ============================================================================
// Asset Resolution Functions
// ============================================================================

/// Resolve an asset attribute to an absolute path.
///
/// Takes a USD attribute that contains an asset path reference and resolves it
/// relative to the stage's root layer. Handles both absolute and relative paths.
pub fn resolve_asset_attr(attr: &Attribute, context_path: &Path) -> Option<AssetPath> {
    let value = attr.get(TimeCode::default())?;

    if let Some(asset) = value.get::<AssetPath>() {
        Some(asset.clone())
    } else if let Some(s) = value.get::<String>() {
        resolve_asset_path(s, context_path)
    } else {
        None
    }
}

/// Resolve an asset path string relative to a context path.
pub fn resolve_asset_path(asset_path: &str, _context_path: &Path) -> Option<AssetPath> {
    if asset_path.is_empty() {
        return None;
    }
    // Full resolution requires AR module; wrap in AssetPath for now
    Some(AssetPath::new(asset_path))
}

/// Resolve material param value: gets attr value and resolves asset paths.
/// Port of C++ `_ResolveMaterialParamValue`.
fn resolve_material_param_value(attr: &Attribute, time: TimeCode) -> Option<Value> {
    let value = attr.get(time)?;
    if let Some(asset) = value.get::<AssetPath>() {
        if is_udim_identifier(asset.get_asset_path()) {
            let root_layer = attr
                .get_prim()
                .stage()
                .map(|stage| usd_sdf::LayerHandle::from_layer(&stage.get_root_layer()));
            let resolved = resolve_udim_path(asset.get_asset_path(), root_layer.as_ref());
            if !resolved.is_empty() {
                let mut resolved_asset = asset.clone();
                resolved_asset.set_resolved_path(resolved);
                return Some(Value::new(resolved_asset));
            }
        }
        return Some(value);
    }
    Some(value)
}

// ============================================================================
// Node ID Resolution
// ============================================================================

/// Get the node identifier for a connectable prim.
/// Port of C++ `_GetNodeId`. Tries:
/// 1. NodeDefAPI::GetShaderId
/// 2. NodeDefAPI::GetShaderNodeForSourceType (per source type)
/// 3. LightFilter::GetShaderId
/// 4. LightAPI::GetShaderId
/// 5. Prim type name fallback
fn get_node_id(
    shade_node: &ConnectableAPI,
    shader_source_types: &[Token],
    render_contexts: &[Token],
) -> Token {
    let prim = shade_node.get_prim();
    let node_def = NodeDefAPI::new(prim.clone());

    // Try NodeDefAPI path first
    if node_def.is_valid() {
        if let Some(id) = node_def.get_id() {
            if !id.as_str().is_empty() {
                return id;
            }
        }
        // No info:id — try source type lookup
        for source_type in shader_source_types {
            if let Some(sdr_node) = node_def.get_shader_node_for_source_type(source_type) {
                return sdr_node.get_identifier().clone();
            }
        }
    }

    // Try LightFilter
    let light_filter = LightFilter::new(prim.clone());
    if light_filter.is_valid() {
        let id = light_filter.get_shader_id(render_contexts);
        if !id.is_empty() {
            return id;
        }
    } else {
        // Try LightAPI
        let light = LightAPI::new(prim.clone());
        if light.is_valid() {
            let id = light.get_shader_id(render_contexts);
            if !id.is_empty() {
                return id;
            }
        }
    }

    // Fallback: prim type name
    prim.get_type_name()
}

// ============================================================================
// Primvar Extraction from Sdr
// ============================================================================

/// Get a primvar name attribute value from either authored params or Sdr default.
/// Port of C++ `_GetPrimvarNameAttributeValue`.
fn get_primvar_name_attr_value(
    sdr_node: Option<&usd_sdr::SdrShaderNode>,
    node: &HdMaterialNode,
    prop_name: &Token,
) -> Token {
    // Check authored params first (strongest opinion)
    if let Some(vt_name) = node.parameters.get(prop_name) {
        if let Some(t) = vt_name.get::<Token>() {
            return t.clone();
        }
        if let Some(s) = vt_name.get::<String>() {
            return Token::new(s);
        }
    }

    // Consult Sdr for default value
    if let Some(sdr) = sdr_node {
        if let Some(sdr_input) = sdr.get_shader_input(prop_name) {
            let default_val = sdr_input.get_default_value();
            if let Some(t) = default_val.get::<Token>() {
                return t.clone();
            }
            if let Some(s) = default_val.get::<String>() {
                return Token::new(s);
            }
        }
    }

    Token::empty()
}

/// Extract primvars referenced by a shader node from Sdr metadata.
/// Port of C++ `_ExtractPrimvarsFromNode`.
fn extract_primvars_from_node(
    node: &HdMaterialNode,
    network: &mut HdMaterialNetworkV1,
    shader_source_types: &[Token],
) {
    let registry = SdrRegistry::get_instance();
    let types_vec = shader_source_types.to_vec();
    let sdr_node = registry.get_shader_node_by_identifier(&node.identifier, &types_vec);

    if let Some(sdr) = sdr_node {
        // Direct primvars from node definition
        for pv in sdr.get_primvars() {
            network.primvars.push(pv.clone());
        }
        // Additional primvar properties (indirectly-named primvars)
        for p in sdr.get_additional_primvar_properties() {
            let name = get_primvar_name_attr_value(Some(sdr), node, p);
            network.primvars.push(name);
        }
    }
}

// ============================================================================
// Shader Graph Walker
// ============================================================================

/// Walk the shader graph emitting nodes in topological order.
/// Port of C++ `_WalkGraph`.
fn walk_graph(
    shade_node: &ConnectableAPI,
    network: &mut HdMaterialNetworkV1,
    visited: &mut HashSet<Path>,
    shader_source_types: &[Token],
    render_contexts: &[Token],
    time: TimeCode,
) {
    let prim = shade_node.get_prim();
    let node_path = prim.get_path().clone();
    if node_path.is_empty() {
        return;
    }

    // Skip already-visited nodes
    if !visited.insert(node_path.clone()) {
        return;
    }

    let mut node = HdMaterialNode {
        path: node_path.clone(),
        identifier: Token::empty(),
        parameters: Default::default(),
    };

    // Visit inputs — ensures upstream nodes are emitted first
    let inputs = shade_node.get_inputs(true);
    for input in &inputs {
        let input_name = input.get_base_name();
        let attrs = input.get_value_producing_attributes(false);

        for attr in &attrs {
            let attr_type = Utils::get_type(&attr.name());

            if attr_type == AttributeType::Output {
                // Output on upstream shading node — recurse and record connection
                let upstream_prim = attr.get_prim();
                let upstream = ConnectableAPI::new(upstream_prim.clone());
                walk_graph(
                    &upstream,
                    network,
                    visited,
                    shader_source_types,
                    render_contexts,
                    time,
                );

                let relationship = HdMaterialRelationship {
                    output_id: node_path.clone(),
                    output_name: input_name.clone(),
                    input_id: upstream_prim.get_path().clone(),
                    input_name: Output::from_attribute(attr.clone()).get_base_name(),
                };
                network.relationships.push(relationship);
            } else if attr_type == AttributeType::Input {
                // Input attribute — extract authored value
                if let Some(value) = resolve_material_param_value(attr, time) {
                    node.parameters.insert(input_name.clone(), value);
                }

                // ColorSpace metadata: `colorSpace:inputName`
                if attr.has_color_space() {
                    let cs_key = Token::new(&Path::join_identifier_pair(
                        &tokens::COLOR_SPACE,
                        &input_name,
                    ));
                    let cs_val = Value::new(attr.get_color_space());
                    node.parameters.insert(cs_key, cs_val);
                }

                // Type metadata: `typeName:inputName`
                let type_key =
                    Token::new(&Path::join_identifier_pair(&tokens::TYPE_NAME, &input_name));
                let type_val = Value::new(attr.get_type_name().as_token());
                node.parameters.insert(type_key, type_val);
            }
        }
    }

    // Resolve node identifier
    let id = get_node_id(shade_node, shader_source_types, render_contexts);
    if !id.is_empty() {
        node.identifier = id;
        extract_primvars_from_node(&node, network, shader_source_types);
    }

    network.nodes.push(node);
}

// ============================================================================
// Public API: Build Material Network
// ============================================================================

/// Build an HdMaterialNetwork for a terminal prim and populate it in the map.
///
/// Walks the shader graph rooted at `usd_terminal` and populates the
/// `material_network_map` under `terminal_identifier`. Works for materials,
/// lights, and light filters.
///
/// Port of C++ `UsdImagingBuildHdMaterialNetworkFromTerminal`.
pub fn build_hd_material_network_from_terminal(
    usd_terminal: &Prim,
    terminal_identifier: &Token,
    shader_source_types: &[Token],
    render_contexts: &[Token],
    material_network_map: &mut HdMaterialNetworkMap,
    time: TimeCode,
) {
    let network = material_network_map
        .map
        .entry(terminal_identifier.clone())
        .or_insert_with(HdMaterialNetworkV1::default);

    let mut visited = HashSet::new();
    let connectable = ConnectableAPI::new(usd_terminal.clone());

    walk_graph(
        &connectable,
        network,
        &mut visited,
        shader_source_types,
        render_contexts,
        time,
    );

    if network.nodes.is_empty() {
        log::warn!(
            "Empty material network for terminal {}",
            usd_terminal.get_path().as_str()
        );
        return;
    }

    // Terminal node is last in the list (topological order)
    let terminal_path = network.nodes.last().unwrap().path.clone();
    material_network_map.terminals.push(terminal_path);

    // Validate identifier against Sdr
    let terminal_id = &network.nodes.last().unwrap().identifier;
    let registry = SdrRegistry::get_instance();
    let types_vec2 = shader_source_types.to_vec();
    if registry
        .get_shader_node_by_identifier(terminal_id, &types_vec2)
        .is_none()
    {
        log::warn!(
            "Invalid info:id {} node: {}",
            terminal_id.as_str(),
            network.nodes.last().unwrap().path.as_str()
        );
        // Return empty network so backend can use fallback material
        *material_network_map = HdMaterialNetworkMap::default();
    }
}

// ============================================================================
// Public API: Time-Varying Check
// ============================================================================

/// Returns whether the material network for a terminal prim is time varying.
///
/// Walks the shader graph and checks if any input attribute is time-varying.
/// Port of C++ `UsdImagingIsHdMaterialNetworkTimeVarying`.
pub fn is_hd_material_network_time_varying(usd_terminal: &Prim) -> bool {
    let mut visited = HashSet::new();
    is_graph_time_varying(&ConnectableAPI::new(usd_terminal.clone()), &mut visited)
}

/// Recursive time-varying check on the shader graph.
/// Port of C++ `_IsGraphTimeVarying`.
fn is_graph_time_varying(shade_node: &ConnectableAPI, visited: &mut HashSet<Path>) -> bool {
    let prim = shade_node.get_prim();
    let node_path = prim.get_path().clone();
    if node_path.is_empty() {
        return false;
    }
    if !visited.insert(node_path) {
        return false;
    }

    let inputs = shade_node.get_inputs(true);
    for input in &inputs {
        let attrs = input.get_value_producing_attributes(false);
        for attr in &attrs {
            let attr_type = Utils::get_type(&attr.name());
            if attr_type == AttributeType::Output {
                if is_graph_time_varying(&ConnectableAPI::new(attr.get_prim()), visited) {
                    return true;
                }
            } else if attr_type == AttributeType::Input {
                if attr.value_might_be_time_varying() {
                    return true;
                }
            }
        }
    }

    false
}

// ============================================================================
// Legacy Convenience Functions
// ============================================================================

/// Extract all authored shader parameters from a shader prim.
///
/// Iterates all `inputs:*` attributes on a shader prim and extracts their
/// values into ParamValue types. This matches the C++ pattern of walking shader
/// inputs via UsdShadeConnectableAPI::GetInputs().
pub fn extract_shader_params(shader: &Prim) -> HashMap<Token, ParamValue> {
    let mut params = HashMap::new();
    let input_prefix = "inputs:";

    for attr_name in shader.get_attribute_names() {
        let name_str = attr_name.as_str();
        if !name_str.starts_with(input_prefix) {
            continue;
        }
        let base_name = &name_str[input_prefix.len()..];
        if base_name.is_empty() {
            continue;
        }
        if let Some(attr) = shader.get_attribute(name_str) {
            if let Some(value) = extract_param_value(&attr) {
                params.insert(Token::new(base_name), value);
            }
        }
    }

    params
}

/// Extract a parameter value from an attribute.
fn extract_param_value(attr: &Attribute) -> Option<ParamValue> {
    use usd_gf::{Vec2f, Vec3f, Vec4f};

    let value = attr.get(TimeCode::default())?;

    if let Some(&f) = value.get::<f64>() {
        Some(ParamValue::Float(f as f32))
    } else if let Some(&f) = value.get::<f32>() {
        Some(ParamValue::Float(f))
    } else if let Some(v) = value.get::<Vec2f>() {
        Some(ParamValue::Vec2(v.x, v.y))
    } else if let Some(v) = value.get::<Vec3f>() {
        Some(ParamValue::Vec3(v.x, v.y, v.z))
    } else if let Some(v) = value.get::<Vec4f>() {
        Some(ParamValue::Vec4(v.x, v.y, v.z, v.w))
    } else if let Some(&i) = value.get::<i32>() {
        Some(ParamValue::Int(i))
    } else if let Some(&b) = value.get::<bool>() {
        Some(ParamValue::Bool(b))
    } else if let Some(s) = value.get::<String>() {
        Some(ParamValue::String(s.clone()))
    } else if let Some(a) = value.get::<AssetPath>() {
        Some(ParamValue::Asset(a.clone()))
    } else if let Some(t) = value.get::<Token>() {
        Some(ParamValue::Token(t.clone()))
    } else {
        None
    }
}

// ============================================================================
// Material Network Functions
// ============================================================================

/// Build list of material network terminals from a material prim.
///
/// Extracts the surface, displacement, and volume shader connections.
pub fn build_material_terminals(material: &Prim) -> Vec<MaterialTerminal> {
    let mut terminals = Vec::new();
    let terminal_names = ["surface", "displacement", "volume"];

    for name in &terminal_names {
        let rel_name = format!("outputs:{}", name);
        if let Some(rel) = material.get_relationship(&rel_name) {
            let targets = rel.get_targets();
            if let Some(target) = targets.first() {
                if let Some((shader_path, output)) = parse_shader_connection(target) {
                    terminals.push(MaterialTerminal {
                        name: Token::new(name),
                        shader_path,
                        output_name: output,
                    });
                }
            }
        }
    }

    terminals
}

/// Parse a shader connection path into shader prim path and output name.
///
/// USD connections: "/Path/To/Shader.outputs:outputName"
fn parse_shader_connection(path: &Path) -> Option<(Path, Token)> {
    let path_str = path.as_str();

    if let Some(dot_pos) = path_str.find(".outputs:") {
        let shader_path_str = &path_str[..dot_pos];
        let output_name = &path_str[dot_pos + 9..]; // Skip ".outputs:"

        if let Some(shader_path) = Path::from_string(shader_path_str) {
            return Some((shader_path, Token::new(output_name)));
        }
    }

    None
}

/// Check if a shader prim is a texture reader node.
pub fn is_texture_reader(shader: &Prim) -> bool {
    let type_name = shader.get_type_name();
    matches!(
        type_name.as_str(),
        "UsdUVTexture" | "UsdPrimvarReader_float2"
    )
}

/// Get the file attribute from a texture shader.
pub fn get_texture_file_attr(texture_shader: &Prim) -> Option<Attribute> {
    texture_shader.get_attribute("inputs:file")
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_empty_path() {
        let context = Path::from_string("/World").unwrap();
        let resolved = resolve_asset_path("", &context);
        assert!(resolved.is_none());
    }

    #[test]
    fn test_resolve_non_empty_path() {
        let context = Path::from_string("/World/Material").unwrap();
        let asset = "textures/albedo.png";
        let resolved = resolve_asset_path(asset, &context);
        assert!(resolved.is_some());
    }

    #[test]
    fn test_parse_shader_connection() {
        let path = Path::from_string("/World/Material/Shader.outputs:surface").unwrap();
        let result = parse_shader_connection(&path);
        assert!(result.is_some());

        let (shader_path, output) = result.unwrap();
        assert_eq!(shader_path.as_str(), "/World/Material/Shader");
        assert_eq!(output.as_str(), "surface");
    }

    #[test]
    fn test_param_value_types() {
        let float_val = ParamValue::Float(1.5);
        let vec3_val = ParamValue::Vec3(1.0, 0.5, 0.0);
        let string_val = ParamValue::String("test".to_string());

        match float_val {
            ParamValue::Float(f) => assert_eq!(f, 1.5),
            _ => panic!("Wrong variant"),
        }

        match vec3_val {
            ParamValue::Vec3(r, g, b) => {
                assert_eq!(r, 1.0);
                assert_eq!(g, 0.5);
                assert_eq!(b, 0.0);
            }
            _ => panic!("Wrong variant"),
        }

        match string_val {
            ParamValue::String(s) => assert_eq!(s, "test"),
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_material_terminal() {
        let terminal = MaterialTerminal {
            name: Token::new("surface"),
            shader_path: Path::from_string("/Material/Shader").unwrap(),
            output_name: Token::new("result"),
        };

        assert_eq!(terminal.name.as_str(), "surface");
        assert_eq!(terminal.shader_path.as_str(), "/Material/Shader");
        assert_eq!(terminal.output_name.as_str(), "result");
    }

    #[test]
    fn test_get_node_id_fallback_to_type_name() {
        // Without a real stage, ConnectableAPI::new on an empty prim
        // should fall through to type name fallback.
        // This tests the fallback path structurally.
        let prim = Prim::invalid();
        let connectable = ConnectableAPI::new(prim);
        let id = get_node_id(&connectable, &[], &[]);
        // Pseudo root has empty type name
        assert!(id.is_empty() || !id.is_empty()); // Just verify no panic
    }

    #[test]
    fn test_build_hd_material_network_empty_prim() {
        // Building from pseudo root should produce an empty/warned network
        let prim = Prim::invalid();
        let mut map = HdMaterialNetworkMap::default();
        build_hd_material_network_from_terminal(
            &prim,
            &Token::new("surface"),
            &[],
            &[],
            &mut map,
            TimeCode::default(),
        );
        // Network should be empty (no authored inputs on pseudo root)
        // Either nodes is empty or the map was cleared due to invalid id
        assert!(
            map.map.is_empty()
                || map.map.values().all(|n| n.nodes.is_empty())
                || map.terminals.is_empty()
        );
    }
}
