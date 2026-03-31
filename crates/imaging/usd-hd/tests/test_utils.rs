// Port of pxr/imaging/hd/testenv/testHdUtils.cpp
//
// C++ test exercises HdUtils::ConvertHdMaterialNetworkToHdMaterialSchema,
// which converts a v1 HdMaterialNetworkMap into an HdContainerDataSource
// via Hydra's scene-index material schema.
//
// In Rust we have hd_convert_to_material_network2 (v1 -> v2 network), but
// ConvertHdMaterialNetworkToHdMaterialSchema (v1 -> schema container DS) is
// not yet ported.  We test the v2 conversion path that _is_ available, and
// mark the schema-level test ignored until the function is ported.

use usd_hd::data_source::cast_to_container;
use usd_hd::material_network::{
    HdMaterialNetworkMap, HdMaterialNetworkV1, HdMaterialNode, HdMaterialRelationship,
    hd_convert_to_material_network2,
};
use usd_hd::utils::convert_hd_material_network_to_hd_material_schema;
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

/// Build the same material network that C++ BasicTest builds:
///
///   Texture_5 ──resultRGB──► MaterialLayer_3 ──pbsMaterialOut──► PbsNetworkMaterialStandIn_3
fn build_test_network_map() -> HdMaterialNetworkMap {
    let material_path = SdfPath::from("/Asset/Looks/Material");
    let tex_path = SdfPath::from("/Asset/Looks/Material/Texture");
    let layer_path = SdfPath::from("/Asset/Looks/Material/MaterialLayer");
    let standin_path = SdfPath::from("/Asset/Looks/Material/StandIn");

    let texture_node = HdMaterialNode {
        path: tex_path.clone(),
        identifier: Token::new("Texture_5"),
        parameters: {
            let mut p = std::collections::BTreeMap::new();
            p.insert(
                Token::new("inputs:filename"),
                usd_vt::Value::from("studio/patterns/checkerboard/checkerboard.tex"),
            );
            p
        },
    };

    let layer_node = HdMaterialNode {
        path: layer_path.clone(),
        identifier: Token::new("MaterialLayer_3"),
        parameters: std::collections::BTreeMap::new(),
    };

    let standin_node = HdMaterialNode {
        path: standin_path.clone(),
        identifier: Token::new("PbsNetworkMaterialStandIn_3"),
        parameters: std::collections::BTreeMap::new(),
    };

    let tex_to_layer = HdMaterialRelationship {
        input_id: tex_path.clone(),
        input_name: Token::new("resultRGB"),
        output_id: layer_path.clone(),
        output_name: Token::new("albedo"),
    };

    let layer_to_standin = HdMaterialRelationship {
        input_id: layer_path.clone(),
        input_name: Token::new("pbsMaterialOut"),
        output_id: standin_path.clone(),
        output_name: Token::new("multiMaterialIn"),
    };

    let network = HdMaterialNetworkV1 {
        nodes: vec![texture_node, layer_node, standin_node],
        relationships: vec![tex_to_layer, layer_to_standin],
        primvars: vec![],
    };

    let mut map = HdMaterialNetworkMap::default();
    map.map.insert(Token::new("surface"), network);

    let _ = material_path; // used for context, matches C++ SdfPath materialPath
    map
}

/// Port of C++ BasicTest — verifies the material network topology is preserved
/// through v1 -> v2 conversion.  The C++ test calls
/// HdUtils::ConvertHdMaterialNetworkToHdMaterialSchema (not yet ported), then
/// debug-prints the resulting container data source.  We test the equivalent
/// v2 network conversion which IS ported.
#[test]
fn basic_material_network_v1_to_v2_conversion() {
    let map = build_test_network_map();

    let (net2, is_volume) = hd_convert_to_material_network2(&map);

    // Not a volume network.
    assert!(!is_volume);

    // All 3 nodes must be present.
    assert_eq!(net2.nodes.len(), 3, "expected 3 nodes after conversion");

    // Terminal "surface" must point at the last node (StandIn).
    let surface_token = Token::new("surface");
    assert!(
        net2.terminals.contains_key(&surface_token),
        "surface terminal missing"
    );
    let terminal = &net2.terminals[&surface_token];
    assert_eq!(
        terminal.upstream_node,
        SdfPath::from("/Asset/Looks/Material/StandIn"),
        "terminal must point at StandIn (last node)"
    );

    // StandIn must have an input connection on "multiMaterialIn".
    let standin = &net2.nodes[&SdfPath::from("/Asset/Looks/Material/StandIn")];
    let multi_in = Token::new("multiMaterialIn");
    assert!(
        standin.input_connections.contains_key(&multi_in),
        "StandIn missing multiMaterialIn connection"
    );

    // MaterialLayer must have an input connection on "albedo".
    let layer = &net2.nodes[&SdfPath::from("/Asset/Looks/Material/MaterialLayer")];
    let albedo = Token::new("albedo");
    assert!(
        layer.input_connections.contains_key(&albedo),
        "MaterialLayer missing albedo connection"
    );

    // Texture node must have the filename parameter.
    let texture = &net2.nodes[&SdfPath::from("/Asset/Looks/Material/Texture")];
    let filename_key = Token::new("inputs:filename");
    assert!(
        texture.parameters.contains_key(&filename_key),
        "Texture node missing inputs:filename parameter"
    );
}

/// Verify identifiers (node_type_id) are preserved through conversion.
#[test]
fn node_identifiers_preserved() {
    let map = build_test_network_map();
    let (net2, _) = hd_convert_to_material_network2(&map);

    let tex = &net2.nodes[&SdfPath::from("/Asset/Looks/Material/Texture")];
    assert_eq!(tex.node_type_id, Token::new("Texture_5"));

    let layer = &net2.nodes[&SdfPath::from("/Asset/Looks/Material/MaterialLayer")];
    assert_eq!(layer.node_type_id, Token::new("MaterialLayer_3"));

    let standin = &net2.nodes[&SdfPath::from("/Asset/Looks/Material/StandIn")];
    assert_eq!(
        standin.node_type_id,
        Token::new("PbsNetworkMaterialStandIn_3")
    );
}

/// Empty network map must produce an empty v2 network without panicking.
#[test]
fn empty_network_map_is_safe() {
    let map = HdMaterialNetworkMap::default();
    let (net2, is_vol) = hd_convert_to_material_network2(&map);
    assert!(!is_vol);
    assert!(net2.nodes.is_empty());
    assert!(net2.terminals.is_empty());
}

/// Port of C++ ConvertHdMaterialNetworkToHdMaterialSchema / BasicTest.
/// Verifies the v1 network is correctly lifted into the HdMaterial schema DS hierarchy.
#[test]
fn convert_to_hd_material_schema_container_ds() {
    let map = build_test_network_map();
    let ds = convert_hd_material_network_to_hd_material_schema(&map);

    // Top-level container must exist and expose the universal render context (empty token).
    let universal = Token::new("");
    let network_base = ds
        .get(&universal)
        .expect("universalRenderContext child missing");
    let network_ds = cast_to_container(&network_base).expect("network must be a container");

    // Must have a "nodes" child container.
    let nodes_token = Token::new("nodes");
    let nodes_base = network_ds
        .get(&nodes_token)
        .expect("nodes container missing");
    let nodes_ds = cast_to_container(&nodes_base).expect("nodes must be a container");

    // All three nodes must be present (keyed by path string).
    let tex_key = Token::new("/Asset/Looks/Material/Texture");
    let layer_key = Token::new("/Asset/Looks/Material/MaterialLayer");
    let standin_key = Token::new("/Asset/Looks/Material/StandIn");
    assert!(nodes_ds.get(&tex_key).is_some(), "Texture node missing");
    assert!(
        nodes_ds.get(&layer_key).is_some(),
        "MaterialLayer node missing"
    );
    assert!(nodes_ds.get(&standin_key).is_some(), "StandIn node missing");

    // Must have a "terminals" child container with a "surface" entry.
    let terminals_token = Token::new("terminals");
    let terminals_base = network_ds
        .get(&terminals_token)
        .expect("terminals container missing");
    let terminals_ds = cast_to_container(&terminals_base).expect("terminals must be a container");
    let surface_token = Token::new("surface");
    assert!(
        terminals_ds.get(&surface_token).is_some(),
        "surface terminal missing"
    );
}
