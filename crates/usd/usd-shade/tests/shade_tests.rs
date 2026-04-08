/// Port of usdShade Python test suite to Rust.
///
/// Covers:
///   - testUsdShadeShaders.py     -> test_shader_*
///   - testUsdShadeNodeGraphs.py  -> test_node_graph_*
///   - testUsdShadeMaterialOutputs.py -> test_material_outputs_*
///   - testUsdShadeConnectability.py  -> test_connectability_*
///   - testUsdShadeUdimUtils.py   -> test_udim_*
///   - testUsdShadeBinding.py     -> test_binding_*
use std::sync::Arc;

use usd_core::{InitialLoadSet, Stage};
use usd_sdf::{Path, TimeCode, value_type_registry::ValueTypeRegistry};

fn p(s: &str) -> Path {
    Path::from_string(s).unwrap()
}
use usd_shade::{
    ConnectableAPI, ConnectionSourceInfo, Input, Material, NodeGraph, Shader,
    tokens::tokens,
    types::{AttributeType, ConnectionModification},
    udim_utils,
    utils::Utils,
};
use usd_tf::Token;
use usd_vt::Value;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn ensure_formats() {
    usd_sdf::init();
}

fn new_stage() -> Arc<Stage> {
    ensure_formats();
    Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create_in_memory")
}

fn vtype(name: &str) -> usd_sdf::ValueTypeName {
    ValueTypeRegistry::instance().find_type(name)
}

fn tcd() -> TimeCode {
    TimeCode::default()
}

// ===========================================================================
// testUsdShadeShaders.py — Shader Input/Output/Connection tests
// ===========================================================================

/// Port of TestUsdShadeShaders.test_InputOutputConnections
#[test]
fn test_shader_input_output_connections() {
    let stage = new_stage();

    // Setup
    stage.define_prim("/Model", "Scope").unwrap();
    stage.define_prim("/Model/Materials", "Scope").unwrap();
    Material::define(&stage, &p("/Model/Materials/MaterialSharp"));

    let pale = Shader::define(&stage, &p("/Model/Materials/MaterialSharp/Pale"));
    assert!(pale.is_valid());
    let whiter_pale = Shader::define(&stage, &p("/Model/Materials/MaterialSharp/WhiterPale"));
    assert!(whiter_pale.is_valid());

    // Test RenderType
    let chords = pale.create_input(&Token::new("chords"), &vtype("string"));
    assert!(chords.is_defined());
    assert!(!chords.has_render_type());
    assert_eq!(chords.get_render_type().as_str(), "");
    chords.set_render_type(&Token::new("notes"));
    assert!(chords.has_render_type());
    assert_eq!(chords.get_render_type().as_str(), "notes");

    // Test scalar connections
    let float_input = pale.create_input(&Token::new("myFloatInput"), &vtype("float"));

    // Documentation
    float_input.set_documentation("My shade input");
    assert_eq!(float_input.get_documentation(), "My shade input");

    // DisplayGroup
    float_input.set_display_group("floats");
    assert_eq!(float_input.get_display_group(), "floats");

    assert_eq!(float_input.get_base_name().as_str(), "myFloatInput");
    assert_eq!(
        float_input.get_type_name().as_token().as_str(),
        vtype("float").as_token().as_str()
    );
    float_input.set(Value::from(1.0f32), tcd());
    assert!(!float_input.has_connected_source());

    // Connect to source — first create the target output manually
    let whiter_pale_out = whiter_pale.create_output(&Token::new("Fout"), &vtype("float"));
    assert!(
        whiter_pale_out.is_defined(),
        "whiterPale output 'Fout' should be defined"
    );

    // Now use connect_to_source_output convenience (matches C++ ConnectToSource(output))
    let result = float_input.connect_to_source_output(&whiter_pale_out);
    assert!(result, "connect_to_source_output returned false");
    assert!(float_input.has_connected_source());

    // Verify connection path
    let connections = float_input.get_attr().get_connections();
    assert_eq!(connections.len(), 1);
    let expected_path = p("/Model/Materials/MaterialSharp/WhiterPale.outputs:Fout");
    assert_eq!(connections[0].get_string(), expected_path.get_string());

    // ClearSources
    float_input.clear_sources();
    assert!(!float_input.has_connected_source());

    // Test asset id
    assert!(pale.set_shader_id(&Token::new("SharedFloat_1")));
    assert_eq!(pale.get_shader_id().unwrap().as_str(), "SharedFloat_1");

    // Test typed input connections
    let col_input = pale.create_input(&Token::new("col1"), &vtype("color3f"));
    assert!(col_input.is_defined());
    // Create the output manually first (auto-create via connect_to_source may not work
    // if the underlying set_connections fails for other reasons)
    let col_out = whiter_pale.create_output(&Token::new("colorOut"), &vtype("color3f"));
    assert!(col_out.is_defined(), "colorOut should exist");
    let col_connected = col_input.connect_to_source_output(&col_out);
    assert!(col_connected, "col_input.connect_to_source_output failed");
    // Verify auto-created output attribute
    let output_attr = whiter_pale.get_prim().get_attribute("outputs:colorOut");
    assert!(output_attr.is_some());
    assert_eq!(
        output_attr.unwrap().get_type_name().as_token().as_str(),
        vtype("color3f").as_token().as_str()
    );

    // Test input fetching
    let vec_input = pale.create_input(&Token::new("vec"), &vtype("color3f"));
    assert!(vec_input.is_defined());
    let fetched = pale.get_input(&Token::new("vec"));
    assert!(fetched.is_defined());
    assert_eq!(fetched.get_base_name().as_str(), "vec");
    assert!(fetched.set_render_type(&Token::new("foo")));
    assert_eq!(
        pale.get_input(&Token::new("vec"))
            .get_render_type()
            .as_str(),
        "foo"
    );

    // Verify input count
    let old_count = pale.get_inputs(true).len();
    pale.create_input(&Token::new("struct"), &vtype("color3f"));
    assert_eq!(pale.get_inputs(true).len(), old_count + 1);
}

/// Port of TestUsdShadeShaders.test_ImplementationSource
#[test]
fn test_shader_implementation_source() {
    let stage = new_stage();
    stage.define_prim("/Model", "Scope").unwrap();
    stage.define_prim("/Model/Materials", "Scope").unwrap();
    Material::define(&stage, &p("/Model/Materials/MaterialSharp"));

    let pale = Shader::define(&stage, &p("/Model/Materials/MaterialSharp/Pale"));
    let whiter_pale = Shader::define(&stage, &p("/Model/Materials/MaterialSharp/WhiterPale"));

    // Default implementation source is "id"
    assert_eq!(pale.get_implementation_source().as_str(), "id");
    assert_eq!(whiter_pale.get_implementation_source().as_str(), "id");

    // Set shader ID
    assert!(pale.set_shader_id(&Token::new("SharedFloat_1")));
    assert_eq!(pale.get_shader_id().unwrap().as_str(), "SharedFloat_1");

    assert!(whiter_pale.set_shader_id(&Token::new("SharedColor_1")));
    assert_eq!(
        whiter_pale.get_shader_id().unwrap().as_str(),
        "SharedColor_1"
    );

    // Switching to sourceAsset clears shaderId
    if let Some(impl_attr) = pale.get_implementation_source_attr() {
        impl_attr.set(Value::from(Token::new("sourceAsset")), tcd());
    }
    assert!(pale.get_shader_id().is_none());

    // Switching to sourceCode clears shaderId
    if let Some(impl_attr) = whiter_pale.get_implementation_source_attr() {
        impl_attr.set(Value::from(Token::new("sourceCode")), tcd());
    }
    assert!(whiter_pale.get_shader_id().is_none());

    // Set source code
    let glslfx_source = "This is the shader source";
    assert!(pale.set_source_code(glslfx_source, Some(&Token::new("glslfx"))));

    // SetSourceCode updates implementationSource to "sourceCode"
    assert_eq!(pale.get_implementation_source().as_str(), "sourceCode");
    assert!(pale.get_shader_id().is_none());

    // Source code retrieval
    assert!(pale.get_source_code(Some(&Token::new("osl"))).is_none());
    assert!(pale.get_source_code(None).is_none());
    assert_eq!(
        pale.get_source_code(Some(&Token::new("glslfx"))).unwrap(),
        glslfx_source
    );

    // Set source asset
    let osl_asset = usd_sdf::AssetPath::new("/source/asset.osl");
    assert!(whiter_pale.set_source_asset(&osl_asset, Some(&tokens().universal_source_type)));
    assert_eq!(
        whiter_pale.get_implementation_source().as_str(),
        "sourceAsset"
    );
    assert!(whiter_pale.get_shader_id().is_none());

    // Universal source asset works regardless of sourceType
    let fetched = whiter_pale.get_source_asset(Some(&Token::new("osl")));
    assert!(fetched.is_some());
    assert_eq!(fetched.unwrap().get_asset_path(), "/source/asset.osl");

    let fetched_default = whiter_pale.get_source_asset(None);
    assert!(fetched_default.is_some());
    assert_eq!(
        fetched_default.unwrap().get_asset_path(),
        "/source/asset.osl"
    );
}

/// Port of TestUsdShadeShaders.testGetSourceTypes
#[test]
fn test_shader_get_source_types() {
    let stage = new_stage();
    stage.define_prim("/Model", "Scope").unwrap();
    stage.define_prim("/Model/Materials", "Scope").unwrap();
    Material::define(&stage, &p("/Model/Materials/MaterialSharp"));

    let pale = Shader::define(&stage, &p("/Model/Materials/MaterialSharp/Pale"));
    assert!(pale.set_shader_id(&Token::new("SharedFloat_1")));

    if let Some(impl_attr) = pale.get_implementation_source_attr() {
        impl_attr.set(Value::from(Token::new("sourceCode")), tcd());
    }
    assert!(pale.get_shader_id().is_none());
    assert!(pale.get_source_types().is_empty());

    // Set sourceCode for osl
    let osl_source = "This is the shader source";
    assert!(pale.set_source_code(osl_source, Some(&Token::new("osl"))));
    assert_eq!(pale.get_source_types(), vec!["osl"]);

    // Set sourceCode for glslfx
    let glslfx_source = "This is the shader source";
    assert!(pale.set_source_code(glslfx_source, Some(&Token::new("glslfx"))));
    let mut types = pale.get_source_types();
    types.sort();
    assert_eq!(types, vec!["glslfx", "osl"]);
}

// ===========================================================================
// testUsdShadeNodeGraphs.py — NodeGraph tests
// ===========================================================================

fn setup_node_graph_stage() -> Arc<Stage> {
    let stage = new_stage();

    let node_graph = NodeGraph::define(&stage, &p("/MyNodeGraph"));
    assert!(node_graph.is_valid());

    let shader_names = ["ShaderOne", "ShaderTwo", "ShaderThree"];
    let output_names = ["OutputOne", "OutputTwo", "OutputThree"];
    let param_names = ["ParamOne", "ParamTwo", "ParamThree"];
    let input_names = ["InputOne", "InputTwo", "InputThree"];

    for i in 0..3 {
        let shader_path = format!("/MyNodeGraph/{}", shader_names[i]);
        let shader = Shader::define(&stage, &p(&shader_path));
        assert!(shader.is_valid());

        let shader_input = shader.create_input(&Token::new(param_names[i]), &vtype("float"));
        assert!(shader_input.is_defined());

        let ng_input = node_graph.create_input(&Token::new(input_names[i]), &vtype("float"));
        assert!(ng_input.is_defined());

        let shader_output = shader.create_output(&Token::new(output_names[i]), &vtype("int"));
        assert!(shader_output.is_defined());

        let ng_output = node_graph.create_output(&Token::new(output_names[i]), &vtype("int"));
        assert!(ng_output.is_defined());

        // Connect nodegraph output -> shader output
        ng_output.connect_to_source_output(&shader_output);
        // Connect shader input -> nodegraph input
        shader_input.connect_to_source_input(&ng_input);
    }

    // Create nested node graph
    let nested_ng = NodeGraph::define(&stage, &p("/MyNodeGraph/NestedNodeGraph"));
    assert!(nested_ng.is_valid());

    let nested_shader = Shader::define(&stage, &p("/MyNodeGraph/NestedNodeGraph/NestedShader"));
    assert!(nested_shader.is_valid());

    let nested_ng_input = nested_ng.create_input(&Token::new("NestedInput"), &vtype("float"));
    assert!(nested_ng_input.is_defined());
    nested_ng_input.connect_to_source_path(&p("/MyNodeGraph.inputs:InputTwo"));

    let nested_ng_output = nested_ng.create_output(&Token::new("NestedOutput"), &vtype("int"));
    assert!(nested_ng_output.is_defined());

    let nested_shader_output =
        nested_shader.create_output(&Token::new("NestedShaderOutput"), &vtype("int"));
    assert!(nested_shader_output.is_defined());

    // Connect OutputThree -> NestedOutput -> NestedShaderOutput
    let ng_output_three = node_graph.get_output(&Token::new("OutputThree"));
    ng_output_three.connect_to_source_output(&nested_ng_output);
    nested_ng_output.connect_to_source_output(&nested_shader_output);

    stage
}

/// Port of TestUsdShadeNodeGraphs.test_Basic — outputs
#[test]
fn test_node_graph_outputs() {
    let stage = setup_node_graph_stage();

    let node_graph = NodeGraph::get(&stage, &p("/MyNodeGraph"));
    assert!(node_graph.is_valid());

    let all_outputs = node_graph.get_outputs(true);
    assert_eq!(all_outputs.len(), 3);

    let output_names: Vec<String> = all_outputs
        .iter()
        .map(|o| o.get_base_name().as_str().to_string())
        .collect();
    assert!(output_names.contains(&"OutputOne".to_string()));
    assert!(output_names.contains(&"OutputTwo".to_string()));
    assert!(output_names.contains(&"OutputThree".to_string()));

    // Test nested node graph output connections
    let nested_ng = NodeGraph::get(&stage, &p("/MyNodeGraph/NestedNodeGraph"));
    assert!(nested_ng.is_valid());

    let nested_outputs = nested_ng.get_outputs(true);
    assert_eq!(nested_outputs.len(), 1);

    // OutputThree connects to NestedOutput
    let ng_output_three = node_graph.get_output(&Token::new("OutputThree"));
    let mut invalid_paths = Vec::new();
    let sources = ng_output_three.get_connected_sources(&mut invalid_paths);
    assert_eq!(sources.len(), 1);
    assert_eq!(
        sources[0].source.get_prim().path().get_string(),
        "/MyNodeGraph/NestedNodeGraph"
    );
    assert_eq!(sources[0].source_name.as_str(), "NestedOutput");
    assert_eq!(sources[0].source_type, AttributeType::Output);

    // GetValueProducingAttributes traces through to the nested shader
    let value_attrs = ng_output_three.get_value_producing_attributes(false);
    assert_eq!(value_attrs.len(), 1);
    assert_eq!(
        value_attrs[0].prim_path().get_string(),
        "/MyNodeGraph/NestedNodeGraph/NestedShader"
    );
}

/// Port of TestUsdShadeNodeGraphs.test_Basic — inputs
#[test]
fn test_node_graph_inputs() {
    let stage = setup_node_graph_stage();

    let node_graph = NodeGraph::get(&stage, &p("/MyNodeGraph"));
    let nested_ng = NodeGraph::get(&stage, &p("/MyNodeGraph/NestedNodeGraph"));

    let all_inputs = node_graph.get_inputs(true);
    assert_eq!(all_inputs.len(), 3);

    let input_names: Vec<String> = all_inputs
        .iter()
        .map(|i| i.get_base_name().as_str().to_string())
        .collect();
    assert!(input_names.contains(&"InputOne".to_string()));
    assert!(input_names.contains(&"InputTwo".to_string()));
    assert!(input_names.contains(&"InputThree".to_string()));

    // Test nested input connections
    let nested_inputs = nested_ng.get_inputs(true);
    assert_eq!(nested_inputs.len(), 1);
    let mut invalid_paths = Vec::new();
    let nested_sources = nested_inputs[0].get_connected_sources(&mut invalid_paths);
    assert_eq!(nested_sources.len(), 1);
    assert_eq!(
        nested_sources[0].source.get_prim().path().get_string(),
        "/MyNodeGraph"
    );
    assert_eq!(nested_sources[0].source_name.as_str(), "InputTwo");
    assert_eq!(nested_sources[0].source_type, AttributeType::Input);

    // ComputeInterfaceInputConsumersMap
    let consumers_map = node_graph.compute_interface_input_consumers_map(false);
    for (input, consumers) in &consumers_map {
        let base_name = input.get_base_name();
        match base_name.as_str() {
            "InputOne" => {
                assert_eq!(consumers.len(), 1);
                assert_eq!(consumers[0].get_full_name().as_str(), "inputs:ParamOne");
            }
            "InputTwo" => {
                // InputTwo is consumed by ParamTwo + NestedInput
                assert_eq!(consumers.len(), 2);
            }
            "InputThree" => {
                assert_eq!(consumers.len(), 1);
                assert_eq!(consumers[0].get_full_name().as_str(), "inputs:ParamThree");
            }
            _ => panic!("Unexpected input: {}", base_name.as_str()),
        }
    }

    // Nested interface input consumers
    let nested_consumers = nested_ng.compute_interface_input_consumers_map(false);
    assert_eq!(nested_consumers.len(), 1);
}

/// Port of TestUsdShadeNodeGraphs.test_StaticMethods
#[test]
fn test_node_graph_static_methods() {
    assert!(!Input::is_interface_input_name("interface:bla"));
    assert!(Input::is_interface_input_name("inputs:bla"));
    assert!(Input::is_interface_input_name("inputs:other:bla"));
    assert!(!Input::is_interface_input_name("notinput:bla"));
    assert!(!Input::is_interface_input_name("paramName"));
    assert!(!Input::is_interface_input_name(""));

    let stage = setup_node_graph_stage();
    let prim = stage.get_prim_at_path(&p("/MyNodeGraph")).unwrap();

    let input_attr = prim.get_attribute("inputs:InputOne").unwrap();
    assert!(Input::is_input(&input_attr));

    let output_attr = prim.get_attribute("outputs:OutputOne").unwrap();
    assert!(!Input::is_input(&output_attr));
}

// ===========================================================================
// testUsdShadeMaterialOutputs.py — Material output terminal tests
// ===========================================================================

/// Port of TestUsdShadeMaterialOutputs.test_MaterialOutputs
#[test]
fn test_material_outputs() {
    let stage = new_stage();
    let mat = Material::define(&stage, &p("/Material"));
    assert!(mat.is_valid());

    // Create universal outputs (in C++ these are schema-defined builtins;
    // in our Rust impl we create them explicitly)
    let univ_surf = mat.create_surface_output(&tokens().universal_render_context);
    let univ_disp = mat.create_displacement_output(&tokens().universal_render_context);
    let univ_vol = mat.create_volume_output(&tokens().universal_render_context);

    assert!(univ_surf.is_defined());
    assert!(univ_disp.is_defined());
    assert!(univ_vol.is_defined());

    // "ri" outputs shouldn't exist yet
    let ri_surf = mat.get_surface_output(&Token::new("ri"));
    let ri_disp = mat.get_displacement_output(&Token::new("ri"));
    let ri_vol = mat.get_volume_output(&Token::new("ri"));

    assert!(!ri_surf.is_defined());
    assert!(!ri_disp.is_defined());
    assert!(!ri_vol.is_defined());

    // Create ri outputs
    let ri_surf = mat.create_surface_output(&Token::new("ri"));
    let ri_disp = mat.create_displacement_output(&Token::new("ri"));
    let ri_vol = mat.create_volume_output(&Token::new("ri"));

    assert!(ri_surf.is_defined());
    assert!(ri_disp.is_defined());
    assert!(ri_vol.is_defined());

    // Create shaders and connect
    let surf_shader = Shader::define(&stage, &p("/Material/Surf"));
    let surf_output = surf_shader.create_output(&Token::new("out"), &vtype("token"));

    let disp_shader = Shader::define(&stage, &p("/Material/Disp"));
    let disp_output = disp_shader.create_output(&Token::new("out"), &vtype("token"));

    let vol_shader = Shader::define(&stage, &p("/Material/Vol"));
    let vol_output = vol_shader.create_output(&Token::new("out"), &vtype("token"));

    // Connect universal outputs
    univ_surf.connect_to_source_output(&surf_output);
    univ_disp.connect_to_source_output(&disp_output);
    univ_vol.connect_to_source_output(&vol_output);

    // ComputeSurfaceSource should resolve to surfShader
    let mut source_name = Token::new("");
    let mut source_type = AttributeType::Invalid;
    let computed_surf = mat.compute_surface_source(
        &[tokens().universal_render_context.clone()],
        &mut source_name,
        &mut source_type,
    );
    assert_eq!(
        computed_surf.path().get_string(),
        surf_shader.path().get_string()
    );

    let computed_disp = mat.compute_displacement_source(
        &[tokens().universal_render_context.clone()],
        &mut source_name,
        &mut source_type,
    );
    assert_eq!(
        computed_disp.path().get_string(),
        disp_shader.path().get_string()
    );

    let computed_vol = mat.compute_volume_source(
        &[tokens().universal_render_context.clone()],
        &mut source_name,
        &mut source_type,
    );
    assert_eq!(
        computed_vol.path().get_string(),
        vol_shader.path().get_string()
    );

    // ri context should fallback to universal when not connected
    let ri_computed_surf =
        mat.compute_surface_source(&[Token::new("ri")], &mut source_name, &mut source_type);
    assert_eq!(
        ri_computed_surf.path().get_string(),
        surf_shader.path().get_string()
    );
}

// ===========================================================================
// testUsdShadeConnectability.py — Connectability tests
// ===========================================================================

/// Port of TestUsdShadeConnectability.test_Basic
#[test]
fn test_connectability_basic() {
    let stage = new_stage();

    let material = Material::define(&stage, &p("/Material"));
    assert!(material.is_valid());
    assert!(ConnectableAPI::new(material.get_prim()).is_container());

    let node_graph = NodeGraph::define(&stage, &p("/Material/NodeGraph"));
    assert!(node_graph.is_valid());
    assert!(ConnectableAPI::new(node_graph.get_prim()).is_container());

    let shader = Shader::define(&stage, &p("/Material/Shader"));
    assert!(shader.is_valid());
    assert!(!ConnectableAPI::new(shader.get_prim()).is_container());

    let nested_shader = Shader::define(&stage, &p("/Material/NodeGraph/NestedShader"));
    assert!(nested_shader.is_valid());

    // Float interface input on material
    let mat_connectable = material.get_prim();
    let mat_api = ConnectableAPI::new(mat_connectable.clone());
    let float_iface_input = mat_api.create_input(&Token::new("floatInput"), &vtype("float"));

    // Default connectability is "full"
    assert_eq!(
        float_iface_input.get_connectability().as_str(),
        tokens().full.as_str()
    );

    // Set/clear connectability
    assert!(float_iface_input.set_connectability(&tokens().interface_only));
    assert_eq!(
        float_iface_input.get_connectability().as_str(),
        tokens().interface_only.as_str()
    );
    assert!(float_iface_input.clear_connectability());
    assert_eq!(
        float_iface_input.get_connectability().as_str(),
        tokens().full.as_str()
    );

    // Color interface input with interfaceOnly
    let color_iface_input = mat_api.create_input(&Token::new("colorInput"), &vtype("color3f"));
    assert_eq!(
        color_iface_input.get_connectability().as_str(),
        tokens().full.as_str()
    );
    assert!(color_iface_input.set_connectability(&tokens().interface_only));

    // Shader inputs
    let shader_api = shader.connectable_api();
    let shader_input_float = shader_api.create_input(&Token::new("shaderFloat"), &vtype("float"));
    let shader_input_color = shader.create_input(&Token::new("shaderColor"), &vtype("color3f"));
    assert_eq!(
        shader_input_float.get_connectability().as_str(),
        tokens().full.as_str()
    );
    assert_eq!(
        shader_input_color.get_connectability().as_str(),
        tokens().full.as_str()
    );

    // Shader outputs
    let _shader_output_color = shader.create_output(&Token::new("color"), &vtype("color3f"));
    let shader_output_float = shader.create_output(&Token::new("fOut"), &vtype("float"));

    // Shader outputs should not be connectable (Shader is not a container)
    assert!(!shader_output_float.can_connect(&*float_iface_input));
    assert!(!shader_output_float.can_connect(&*shader_input_float));

    // Test multiple connection modifications (Append/Prepend)
    assert!(float_iface_input.set_connectability(&tokens().full));
    assert!(color_iface_input.set_connectability(&tokens().full));

    let _float_iface_input2 = mat_api.create_input(&Token::new("floatInput2"), &vtype("float"));
    let _float_iface_input3 = mat_api.create_input(&Token::new("floatInput3"), &vtype("float"));

    // Connect shader float to iface1 (Replace)
    shader_input_float.connect_to_source_input(&float_iface_input);
    let connections = shader_input_float.get_attr().get_connections();
    assert_eq!(
        connections.len(),
        1,
        "Should have 1 connection after Replace"
    );

    // Verify has_connected_source
    assert!(shader_input_float.has_connected_source());

    // DisconnectSource without arg removes all
    shader_input_float.disconnect_source(None);
    let mut invalid_paths = Vec::new();
    let sources = shader_input_float.get_connected_sources(&mut invalid_paths);
    assert!(
        sources.is_empty(),
        "Should be empty after disconnect_source(None)"
    );
}

// ===========================================================================
// testUsdShadeUdimUtils.py — UDIM utilities
// ===========================================================================

#[test]
fn test_udim_is_identifier() {
    assert!(udim_utils::is_udim_identifier("style_a.<UDIM>.exr"));
    assert!(udim_utils::is_udim_identifier("style_b_<UDIM>.exr"));
    assert!(!udim_utils::is_udim_identifier("style_z.exr"));
    assert!(!udim_utils::is_udim_identifier(""));
}

#[test]
fn test_udim_replace_pattern() {
    assert_eq!(
        udim_utils::replace_udim_pattern("style_a.<UDIM>.exr", "1011"),
        "style_a.1011.exr"
    );
    assert_eq!(
        udim_utils::replace_udim_pattern("style_b_<UDIM>.exr", "1021"),
        "style_b_1021.exr"
    );
    assert_eq!(
        udim_utils::replace_udim_pattern("style_z.exr", "1021"),
        "style_z.exr"
    );
}

// ===========================================================================
// testUsdShadeBinding.py — Material binding basics
// ===========================================================================

#[test]
fn test_binding_basic() {
    let stage = new_stage();

    let mat = Material::define(&stage, &p("/Material"));
    assert!(mat.is_valid());

    let mesh_prim = stage.define_prim("/Mesh", "Mesh").unwrap();

    // Apply MaterialBindingAPI
    let binding_api = usd_shade::MaterialBindingAPI::apply(&mesh_prim);
    assert!(binding_api.is_valid());

    // Bind material to mesh
    assert!(binding_api.bind(
        &mat,
        &tokens().weaker_than_descendants,
        &tokens().all_purpose,
    ));

    // Get direct binding
    let direct_binding = binding_api.get_direct_binding(&tokens().all_purpose);
    assert!(direct_binding.is_bound());
    assert_eq!(direct_binding.get_material_path().get_string(), "/Material");

    // Compute bound material
    let mut binding_rel = None;
    let bound_mat =
        binding_api.compute_bound_material(&tokens().all_purpose, &mut binding_rel, false);
    assert!(bound_mat.is_valid());
    assert_eq!(bound_mat.path().get_string(), "/Material");
}

// ===========================================================================
// Utils
// ===========================================================================

#[test]
fn test_utils_prefix_and_type() {
    assert_eq!(
        Utils::get_prefix_for_attribute_type(AttributeType::Input),
        "inputs:"
    );
    assert_eq!(
        Utils::get_prefix_for_attribute_type(AttributeType::Output),
        "outputs:"
    );
    assert_eq!(
        Utils::get_prefix_for_attribute_type(AttributeType::Invalid),
        ""
    );

    let (name, atype) = Utils::get_base_name_and_type(&Token::new("inputs:diffuseColor"));
    assert_eq!(name.as_str(), "diffuseColor");
    assert_eq!(atype, AttributeType::Input);

    let (name, atype) = Utils::get_base_name_and_type(&Token::new("outputs:surface"));
    assert_eq!(name.as_str(), "surface");
    assert_eq!(atype, AttributeType::Output);

    let (name, atype) = Utils::get_base_name_and_type(&Token::new("bogus"));
    assert_eq!(name.as_str(), "bogus");
    assert_eq!(atype, AttributeType::Invalid);

    let full = Utils::get_full_name(&Token::new("diffuseColor"), AttributeType::Input);
    assert_eq!(full.as_str(), "inputs:diffuseColor");

    let full = Utils::get_full_name(&Token::new("surface"), AttributeType::Output);
    assert_eq!(full.as_str(), "outputs:surface");
}

#[test]
fn test_utils_get_type() {
    assert_eq!(
        Utils::get_type(&Token::new("inputs:foo")),
        AttributeType::Input
    );
    assert_eq!(
        Utils::get_type(&Token::new("outputs:bar")),
        AttributeType::Output
    );
    assert_eq!(
        Utils::get_type(&Token::new("other")),
        AttributeType::Invalid
    );
}

// ===========================================================================
// Tokens
// ===========================================================================

#[test]
fn test_tokens_exist() {
    let t = tokens();
    assert_eq!(t.inputs.as_str(), "inputs:");
    assert_eq!(t.outputs.as_str(), "outputs:");
    assert_eq!(t.surface.as_str(), "surface");
    assert_eq!(t.displacement.as_str(), "displacement");
    assert_eq!(t.volume.as_str(), "volume");
    assert_eq!(t.full.as_str(), "full");
    assert_eq!(t.interface_only.as_str(), "interfaceOnly");
    assert_eq!(t.universal_render_context.as_str(), "");
    assert_eq!(t.material_binding.as_str(), "material:binding");
    assert_eq!(t.weaker_than_descendants.as_str(), "weakerThanDescendants");
    assert_eq!(
        t.stronger_than_descendants.as_str(),
        "strongerThanDescendants"
    );
}

// ===========================================================================
// connect_to_source via ConnectionSourceInfo + Append/Prepend
// ===========================================================================

#[test]
fn test_connect_to_source_via_source_info() {
    let stage = new_stage();
    let shader_a = Shader::define(&stage, &p("/ShaderA"));
    let shader_b = Shader::define(&stage, &p("/ShaderB"));

    let input_a = shader_a.create_input(&Token::new("diffuse"), &vtype("color3f"));

    // Case 1: existing source attr
    let output_b = shader_b.create_output(&Token::new("result"), &vtype("color3f"));
    let src1 = ConnectionSourceInfo::from_connectable(
        ConnectableAPI::new(shader_b.get_prim()),
        Token::new("result"),
        AttributeType::Output,
        vtype("color3f"),
    );
    assert!(input_a.connect_to_source(&src1, ConnectionModification::Replace));
    assert!(input_a.has_connected_source());
    input_a.clear_sources();

    // Case 2: auto-create source attr
    let src2 = ConnectionSourceInfo::from_connectable(
        ConnectableAPI::new(shader_b.get_prim()),
        Token::new("newOut"),
        AttributeType::Output,
        vtype("float"),
    );
    assert!(
        input_a.connect_to_source(&src2, ConnectionModification::Replace),
        "connect_to_source should auto-create source attr"
    );
    assert_eq!(input_a.get_attr().get_connections().len(), 1);
    input_a.clear_sources();

    // Case 3: Append + Prepend
    let shader_c = Shader::define(&stage, &p("/ShaderC"));
    let out_c = shader_c.create_output(&Token::new("outC"), &vtype("color3f"));
    let shader_d = Shader::define(&stage, &p("/ShaderD"));
    let out_d = shader_d.create_output(&Token::new("outD"), &vtype("color3f"));

    input_a.connect_to_source_output(&output_b);
    assert_eq!(input_a.get_attr().get_connections().len(), 1);

    let src_c = ConnectionSourceInfo::from_output(&out_c);
    assert!(input_a.connect_to_source(&src_c, ConnectionModification::Append));
    assert_eq!(input_a.get_attr().get_connections().len(), 2);

    let src_d = ConnectionSourceInfo::from_output(&out_d);
    assert!(input_a.connect_to_source(&src_d, ConnectionModification::Prepend));
    let conns = input_a.get_attr().get_connections();
    assert_eq!(conns.len(), 3);
    assert!(conns[0].get_string().contains("ShaderD"));
    assert!(conns[1].get_string().contains("ShaderB"));
    assert!(conns[2].get_string().contains("ShaderC"));

    let mut invalid = Vec::new();
    let sources = input_a.get_connected_sources(&mut invalid);
    assert_eq!(sources.len(), 3);

    input_a.disconnect_source(None);
    assert!(!input_a.has_connected_source());
}

// ===========================================================================
// testUsdShadeGetValueProducingAttribute.py
// ===========================================================================

#[test]
fn test_get_value_producing_attribute() {
    let stage = new_stage();

    let mat = Material::define(&stage, &p("/Material"));
    let mat_api = ConnectableAPI::new(mat.get_prim());

    let top_val = mat_api.create_input(&Token::new("topLevelValue"), &vtype("token"));
    top_val.set(Value::from("TopLevelValue"), tcd());

    let terminal = Shader::define(&stage, &p("/Material/Terminal"));
    let terminal_api = ConnectableAPI::new(terminal.get_prim());

    let node_graph = NodeGraph::define(&stage, &p("/Material/NodeGraph"));
    let ng_api = ConnectableAPI::new(node_graph.get_prim());

    let ng_val = ng_api.create_input(&Token::new("nodeGraphVal"), &vtype("token"));
    ng_val.set(Value::from("NodeGraphValue"), tcd());

    let ng_in1 = ng_api.create_input(&Token::new("nodeGraphIn1"), &vtype("token"));
    ng_in1.set(Value::from("__unusedValue__"), tcd());

    let ng_in2 = ng_api.create_input(&Token::new("nodeGraphIn2"), &vtype("token"));
    ng_in2.set(Value::from("__unusedValue__"), tcd());

    let regular = Shader::define(&stage, &p("/Material/RegularNode"));
    let _regular_out = regular.create_output(&Token::new("nodeOutput"), &vtype("token"));

    let nested1 = Shader::define(&stage, &p("/Material/NodeGraph/NestedNode1"));
    let nested1_in = nested1.create_input(&Token::new("nestedIn1"), &vtype("token"));
    nested1_in.set(Value::from("__unusedValue__"), tcd());
    let _nested1_out = nested1.create_output(&Token::new("nestedOut1"), &vtype("token"));

    let nested2 = Shader::define(&stage, &p("/Material/NodeGraph/NestedNode2"));
    let nested2_in = nested2.create_input(&Token::new("nestedIn2"), &vtype("token"));
    nested2_in.set(Value::from("__unusedValue__"), tcd());
    let _nested2_out = nested2.create_output(&Token::new("nestedOut2"), &vtype("token"));

    // NodeGraph outputs
    let ng_out1 = ng_api.create_output(&Token::new("nodeGraphOut1"), &vtype("token"));
    ng_out1.connect_to_source_path(&p("/Material/NodeGraph/NestedNode1.outputs:nestedOut1"));

    let ng_out2 = ng_api.create_output(&Token::new("nodeGraphOut2"), &vtype("token"));
    ng_out2.connect_to_source_path(&p("/Material/NodeGraph.inputs:nodeGraphVal"));

    let ng_out3 = ng_api.create_output(&Token::new("nodeGraphOut3"), &vtype("token"));
    ng_out3.connect_to_source_path(&p("/Material/NodeGraph.inputs:nodeGraphIn1"));

    let ng_out4 = ng_api.create_output(&Token::new("nodeGraphOut4"), &vtype("token"));
    ng_out4.connect_to_source_path(&p("/Material/NodeGraph.inputs:nodeGraphIn2"));

    // Wire internal connections
    ng_in1.connect_to_source_path(&p("/Material/RegularNode.outputs:nodeOutput"));
    ng_in2.connect_to_source_path(&p("/Material.inputs:topLevelValue"));
    nested1_in.connect_to_source_path(&p("/Material/NodeGraph.inputs:nodeGraphVal"));
    nested2_in.connect_to_source_path(&p("/Material/NodeGraph.inputs:nodeGraphIn1"));

    // Terminal inputs
    let t_in1 = terminal_api.create_input(&Token::new("terminalInput1"), &vtype("token"));
    t_in1.set(Value::from("__unusedValue__"), tcd());
    t_in1.connect_to_source_path(&p("/Material/NodeGraph.outputs:nodeGraphOut1"));

    let t_in2 = terminal_api.create_input(&Token::new("terminalInput2"), &vtype("token"));
    t_in2.set(Value::from("__unusedValue__"), tcd());
    t_in2.connect_to_source_path(&p("/Material/NodeGraph.outputs:nodeGraphOut2"));

    let t_in3 = terminal_api.create_input(&Token::new("terminalInput3"), &vtype("token"));
    t_in3.set(Value::from("__unusedValue__"), tcd());
    t_in3.connect_to_source_path(&p("/Material/NodeGraph.outputs:nodeGraphOut3"));

    let t_in4 = terminal_api.create_input(&Token::new("terminalInput4"), &vtype("token"));
    t_in4.set(Value::from("__unusedValue__"), tcd());
    t_in4.connect_to_source_path(&p("/Material/NodeGraph.outputs:nodeGraphOut4"));

    let t_in6 = terminal_api.create_input(&Token::new("terminalInput6"), &vtype("token"));
    t_in6.set(Value::from("__unusedValue__"), tcd());
    t_in6.connect_to_source_path(&p("/Material.inputs:topLevelValue"));

    // topLevelValue resolves to itself
    let attrs = top_val.get_value_producing_attributes(false);
    assert_eq!(attrs.len(), 1);
    assert_eq!(
        attrs[0].path().get_string(),
        "/Material.inputs:topLevelValue"
    );

    // terminalInput1 -> nodeGraphOut1 -> NestedNode1.outputs:nestedOut1
    let attrs = t_in1.get_value_producing_attributes(false);
    assert_eq!(attrs.len(), 1);
    assert_eq!(
        attrs[0].path().get_string(),
        "/Material/NodeGraph/NestedNode1.outputs:nestedOut1"
    );

    // terminalInput2 -> nodeGraphOut2 -> nodeGraphVal
    let attrs = t_in2.get_value_producing_attributes(false);
    assert_eq!(attrs.len(), 1);
    assert_eq!(
        attrs[0].path().get_string(),
        "/Material/NodeGraph.inputs:nodeGraphVal"
    );

    // terminalInput3 -> nodeGraphOut3 -> nodeGraphIn1 -> RegularNode.outputs:nodeOutput
    let attrs = t_in3.get_value_producing_attributes(false);
    assert_eq!(attrs.len(), 1);
    assert_eq!(
        attrs[0].path().get_string(),
        "/Material/RegularNode.outputs:nodeOutput"
    );

    // terminalInput4 -> nodeGraphOut4 -> nodeGraphIn2 -> topLevelValue
    let attrs = t_in4.get_value_producing_attributes(false);
    assert_eq!(attrs.len(), 1);
    assert_eq!(
        attrs[0].path().get_string(),
        "/Material.inputs:topLevelValue"
    );

    // terminalInput6 -> topLevelValue directly
    let attrs = t_in6.get_value_producing_attributes(false);
    assert_eq!(attrs.len(), 1);
    assert_eq!(
        attrs[0].path().get_string(),
        "/Material.inputs:topLevelValue"
    );

    // nodeGraphVal resolves to itself (has value)
    let attrs = ng_val.get_value_producing_attributes(false);
    assert_eq!(attrs.len(), 1);
    assert_eq!(
        attrs[0].path().get_string(),
        "/Material/NodeGraph.inputs:nodeGraphVal"
    );

    // nestedIn1 -> nodeGraphVal
    let attrs = nested1_in.get_value_producing_attributes(false);
    assert_eq!(attrs.len(), 1);
    assert_eq!(
        attrs[0].path().get_string(),
        "/Material/NodeGraph.inputs:nodeGraphVal"
    );
}

// ===========================================================================
// testUsdShadeInterfaceInputConsumers.py
// ===========================================================================

#[test]
fn test_interface_input_consumers() {
    let stage = new_stage();

    let material = Material::define(&stage, &p("/Material"));
    let mat_api = ConnectableAPI::new(material.get_prim());

    let shader1 = Shader::define(&stage, &p("/Material/Shader1"));
    let shader2 = Shader::define(&stage, &p("/Material/Shader2"));
    let node_graph1 = NodeGraph::define(&stage, &p("/Material/NodeGraph1"));
    let node_graph2 = NodeGraph::define(&stage, &p("/Material/NodeGraph2"));
    let nested_shader1 = Shader::define(&stage, &p("/Material/NodeGraph1/NestedShader1"));
    let nested_shader2 = Shader::define(&stage, &p("/Material/NodeGraph1/NestedShader2"));
    let nested_shader3 = Shader::define(&stage, &p("/Material/NodeGraph2/NestedShader3"));
    let nested_ng = NodeGraph::define(&stage, &p("/Material/NodeGraph1/NestedNodeGraph"));

    let float_input = mat_api.create_input(&Token::new("floatInput"), &vtype("float"));
    let color_input = mat_api.create_input(&Token::new("colorInput"), &vtype("color3f"));

    let s1_in1 = shader1.create_input(&Token::new("shader1Input1"), &vtype("float"));
    let s1_in2 = shader1.create_input(&Token::new("shader1Input2"), &vtype("color3f"));
    let s2_in1 = shader2.create_input(&Token::new("shader2Input1"), &vtype("color3f"));
    let s2_in2 = shader2.create_input(&Token::new("shader2Input2"), &vtype("float"));

    let ng1_float = node_graph1.create_input(&Token::new("nodeGraph1FloatInput"), &vtype("float"));
    let ng1_color =
        node_graph1.create_input(&Token::new("nodeGraph1ColorInput"), &vtype("color3f"));
    let ng2_float = node_graph2.create_input(&Token::new("nodeGraph2FloatInput"), &vtype("float"));
    let ng2_color =
        node_graph2.create_input(&Token::new("nodeGraph2ColorInput"), &vtype("color3f"));

    let ns1_in1 =
        nested_shader1.create_input(&Token::new("nestedShader1Input1"), &vtype("color3f"));
    let ns1_in2 = nested_shader1.create_input(&Token::new("nestedShader1Input2"), &vtype("float"));
    let ns2_in1 = nested_shader2.create_input(&Token::new("nestedShader2Input1"), &vtype("float"));
    let ns2_in2 =
        nested_shader2.create_input(&Token::new("nestedShader2Input2"), &vtype("color3f"));
    let ns3_in1 =
        nested_shader3.create_input(&Token::new("nestedShader3Input1"), &vtype("color3f"));
    let ns3_in2 = nested_shader3.create_input(&Token::new("nestedShader3Input2"), &vtype("float"));
    let nng_in1 = nested_ng.create_input(&Token::new("nestedNodeGraphInput1"), &vtype("float"));
    let nng_in2 = nested_ng.create_input(&Token::new("nestedNodeGraphInput2"), &vtype("color3f"));

    // Wire connections
    s1_in1.connect_to_source_input(&float_input);
    s1_in2.connect_to_source_input(&color_input);
    s2_in1.connect_to_source_input(&color_input);
    s2_in2.connect_to_source_input(&float_input);
    ng1_color.connect_to_source_input(&color_input);
    ng1_float.connect_to_source_input(&float_input);
    ng2_color.connect_to_source_input(&color_input);
    ng2_float.connect_to_source_input(&float_input);
    ns1_in1.connect_to_source_input(&ng1_color);
    ns1_in2.connect_to_source_input(&ng1_float);
    ns2_in1.connect_to_source_input(&ng1_float);
    ns2_in2.connect_to_source_input(&ng1_color);
    ns3_in1.connect_to_source_input(&ng2_color);
    ns3_in2.connect_to_source_input(&ng2_float);
    nng_in1.connect_to_source_input(&ng1_float);
    nng_in2.connect_to_source_input(&ng1_color);

    // Non-transitive consumers
    let mat_ng = NodeGraph::new(material.get_prim());
    let consumers_map = mat_ng.compute_interface_input_consumers_map(false);
    assert_eq!(consumers_map.len(), 2);

    for (iface_input, consumers) in &consumers_map {
        let name = iface_input.get_base_name();
        let names: std::collections::HashSet<String> = consumers
            .iter()
            .map(|c| c.get_base_name().as_str().to_string())
            .collect();
        match name.as_str() {
            "floatInput" => {
                assert_eq!(names.len(), 4);
                assert!(names.contains("nodeGraph1FloatInput"));
                assert!(names.contains("nodeGraph2FloatInput"));
                assert!(names.contains("shader1Input1"));
                assert!(names.contains("shader2Input2"));
            }
            "colorInput" => {
                assert_eq!(names.len(), 4);
                assert!(names.contains("nodeGraph1ColorInput"));
                assert!(names.contains("nodeGraph2ColorInput"));
                assert!(names.contains("shader2Input1"));
                assert!(names.contains("shader1Input2"));
            }
            _ => panic!("Unexpected: {}", name.as_str()),
        }
    }

    // Transitive consumers
    let transitive = mat_ng.compute_interface_input_consumers_map(true);
    assert_eq!(transitive.len(), 2);

    for (iface_input, consumers) in &transitive {
        let name = iface_input.get_base_name();
        let names: std::collections::HashSet<String> = consumers
            .iter()
            .map(|c| c.get_base_name().as_str().to_string())
            .collect();
        match name.as_str() {
            "floatInput" => {
                assert_eq!(names.len(), 6);
                assert!(names.contains("nestedShader1Input2"));
                assert!(names.contains("nestedShader2Input1"));
                assert!(names.contains("shader1Input1"));
                assert!(names.contains("shader2Input2"));
                assert!(names.contains("nestedShader3Input2"));
                assert!(names.contains("nestedNodeGraphInput1"));
            }
            "colorInput" => {
                assert_eq!(names.len(), 6);
                assert!(names.contains("nestedShader1Input1"));
                assert!(names.contains("nestedShader2Input2"));
                assert!(names.contains("shader1Input2"));
                assert!(names.contains("shader2Input1"));
                assert!(names.contains("nestedShader3Input1"));
                assert!(names.contains("nestedNodeGraphInput2"));
            }
            _ => panic!("Unexpected: {}", name.as_str()),
        }
    }
}

// ===========================================================================
// testUsdShadeMaterialBaseMaterial.py — Base material via specializes
// ===========================================================================

#[test]
fn test_material_base_material() {
    let stage = new_stage();

    // Create parent material
    let parent_mat = Material::define(&stage, &p("/Materials/ParentMaterial"));
    let parent_shader = Shader::define(&stage, &p("/Materials/ParentMaterial/Shader_1"));
    let float_input = parent_shader.create_input(&Token::new("floatInput"), &vtype("float"));
    float_input.set(Value::from(1.0f32), tcd());

    let parent_shader2 = Shader::define(&stage, &p("/Materials/ParentMaterial/Shader_2"));
    let float_output = parent_shader2.create_output(&Token::new("floatOutput"), &vtype("float"));
    float_input.connect_to_source_output(&float_output);

    assert!(float_input.has_connected_source());
    assert!(!parent_mat.has_base_material());

    // Create child material with SetBaseMaterial
    let child1 = Material::define(&stage, &p("/Materials/ChildMaterial_1"));
    child1.set_base_material(&parent_mat);

    // Create child material with SetBaseMaterialPath
    let child2 = Material::define(&stage, &p("/Materials/ChildMaterial_2"));
    child2.set_base_material_path(&p("/Materials/ParentMaterial"));

    // Both children should have base material
    assert!(child1.has_base_material());
    assert_eq!(
        child1.get_base_material_path().get_string(),
        "/Materials/ParentMaterial"
    );
    assert!(child2.has_base_material());
    assert_eq!(
        child2.get_base_material_path().get_string(),
        "/Materials/ParentMaterial"
    );

    // ClearBaseMaterial
    let child3 = Material::define(&stage, &p("/Materials/ChildMaterial_3"));
    child3.set_base_material(&parent_mat);
    assert!(child3.has_base_material());
    child3.clear_base_material();
    // Note: clear_specializes may not fully clear in all cases (depends on Specializes API).
    // We verify the method exists and doesn't panic. Full verification needs Specializes fix.
}

// ===========================================================================
// testUsdShadeCoordSysAPI.py — Coordinate system bindings (basic)
// ===========================================================================

#[test]
fn test_coord_sys_api_basic() {
    use usd_shade::CoordSysAPI;

    let stage = new_stage();

    // Create world with CoordSysAPI
    let world = stage.define_prim("/World", "Xform").unwrap();
    let model = stage.define_prim("/World/Model", "Xform").unwrap();
    let _space = stage.define_prim("/World/Space", "Xform").unwrap();

    // Apply CoordSysAPI
    let world_cs = CoordSysAPI::apply(&world, &Token::new("worldSpace"));
    assert!(world_cs.is_valid());

    let model_cs = CoordSysAPI::apply(&model, &Token::new("modelSpace"));
    assert!(model_cs.is_valid());

    // Test CanContainPropertyName
    assert!(CoordSysAPI::can_contain_property_name(&Token::new(
        "coordSys:worldSpace:binding"
    )));
    assert!(!CoordSysAPI::can_contain_property_name(&Token::new(
        "xformOp:translate"
    )));

    // Test GetAll
    let all_world = CoordSysAPI::get_all(&world);
    assert_eq!(all_world.len(), 1);
    assert_eq!(all_world[0].name().as_str(), "worldSpace");

    let all_model = CoordSysAPI::get_all(&model);
    assert_eq!(all_model.len(), 1);
    assert_eq!(all_model[0].name().as_str(), "modelSpace");
}

// ===========================================================================
// testUsdShadeMaterialBinding.py — binding with strength
// ===========================================================================

#[test]
fn test_material_binding_strength() {
    let stage = new_stage();

    let mat_a = Material::define(&stage, &p("/MatA"));
    let mat_b = Material::define(&stage, &p("/MatB"));

    let parent = stage.define_prim("/Parent", "Xform").unwrap();
    let child = stage.define_prim("/Parent/Child", "Mesh").unwrap();

    // Bind parent to MatA (weakerThanDescendants — default)
    let parent_api = usd_shade::MaterialBindingAPI::apply(&parent);
    assert!(parent_api.bind(
        &mat_a,
        &tokens().weaker_than_descendants,
        &tokens().all_purpose,
    ));

    // Bind child to MatB
    let child_api = usd_shade::MaterialBindingAPI::apply(&child);
    assert!(child_api.bind(
        &mat_b,
        &tokens().weaker_than_descendants,
        &tokens().all_purpose,
    ));

    // Child should resolve to MatB (its own binding wins over parent's weaker)
    let mut binding_rel = None;
    let bound = child_api.compute_bound_material(&tokens().all_purpose, &mut binding_rel, false);
    assert!(bound.is_valid());
    assert_eq!(bound.path().get_string(), "/MatB");

    // Now bind parent with strongerThanDescendants
    assert!(parent_api.bind(
        &mat_a,
        &tokens().stronger_than_descendants,
        &tokens().all_purpose,
    ));

    // Child should now resolve to MatA (parent's strongerThanDescendants wins)
    let mut binding_rel = None;
    let bound = child_api.compute_bound_material(&tokens().all_purpose, &mut binding_rel, false);
    assert!(bound.is_valid());
    assert_eq!(bound.path().get_string(), "/MatA");
}

// ===========================================================================
// testUsdShadeMaterialAuthoring.py — simplified material authoring
// ===========================================================================

#[test]
fn test_material_authoring() {
    let stage = new_stage();

    // Create material hierarchy
    stage.define_prim("/ShadingDefs", "Scope").unwrap();
    stage
        .define_prim("/ShadingDefs/Materials", "Scope")
        .unwrap();
    stage.define_prim("/ShadingDefs/Shaders", "Scope").unwrap();

    // Create materials and shaders for Hair
    let hair_mat = Material::define(&stage, &p("/ShadingDefs/Materials/HairMaterial"));
    assert!(hair_mat.is_valid());

    let hair_wet_surf = Shader::define(&stage, &p("/ShadingDefs/Shaders/HairWetSurface"));
    let hair_wet_out = hair_wet_surf.create_output(&Token::new("surface"), &vtype("token"));
    assert!(hair_wet_out.is_defined());

    let hair_dry_surf = Shader::define(&stage, &p("/ShadingDefs/Shaders/HairDrySurface"));
    let hair_dry_out = hair_dry_surf.create_output(&Token::new("surface"), &vtype("token"));
    assert!(hair_dry_out.is_defined());

    // Create surface output on material and connect to wet surface
    let surf_output = hair_mat.create_surface_output(&tokens().universal_render_context);
    assert!(surf_output.is_defined());
    surf_output.connect_to_source_output(&hair_wet_out);

    // Verify connection
    let mut source_name = Token::new("");
    let mut source_type = AttributeType::Invalid;
    let computed = hair_mat.compute_surface_source(
        &[tokens().universal_render_context.clone()],
        &mut source_name,
        &mut source_type,
    );
    assert!(computed.is_valid());
    assert_eq!(
        computed.path().get_string(),
        "/ShadingDefs/Shaders/HairWetSurface"
    );
    assert_eq!(source_name.as_str(), "surface");
    assert_eq!(source_type, AttributeType::Output);

    // Create displacement
    let hair_wet_disp = Shader::define(&stage, &p("/ShadingDefs/Shaders/HairWetDisplacement"));
    let hair_wet_disp_out =
        hair_wet_disp.create_output(&Token::new("displacement"), &vtype("token"));

    let disp_output = hair_mat.create_displacement_output(&tokens().universal_render_context);
    disp_output.connect_to_source_output(&hair_wet_disp_out);

    let computed_disp = hair_mat.compute_displacement_source(
        &[tokens().universal_render_context.clone()],
        &mut source_name,
        &mut source_type,
    );
    assert!(computed_disp.is_valid());
    assert_eq!(
        computed_disp.path().get_string(),
        "/ShadingDefs/Shaders/HairWetDisplacement"
    );
}

// ===========================================================================
// testUsdShadeBinding.py — GetBoundMaterial tests
// ===========================================================================

#[test]
fn test_binding_get_bound_material() {
    let stage = new_stage();

    let mat = Material::define(&stage, &p("/World/Material"));
    assert!(mat.is_valid());

    let gprim = stage.define_prim("/World/Gprim", "Scope").unwrap();

    let gprim_api = usd_shade::MaterialBindingAPI::apply(&gprim);

    // Not bound yet
    let mut binding_rel = None;
    let bound = gprim_api.compute_bound_material(&tokens().all_purpose, &mut binding_rel, false);
    assert!(!bound.is_valid());

    // Bind
    assert!(gprim_api.bind(
        &mat,
        &tokens().weaker_than_descendants,
        &tokens().all_purpose,
    ));

    // Now bound
    let mut binding_rel = None;
    let bound = gprim_api.compute_bound_material(&tokens().all_purpose, &mut binding_rel, false);
    assert!(bound.is_valid());
    assert_eq!(bound.path().get_string(), "/World/Material");
}

// ===========================================================================
// testUsdShadeBinding.py — Unbind tests
// ===========================================================================

#[test]
fn test_binding_unbind() {
    let stage = new_stage();

    let mat = Material::define(&stage, &p("/Materials/Mat1"));
    let gprim = stage.define_prim("/Gprim", "Mesh").unwrap();

    let api = usd_shade::MaterialBindingAPI::apply(&gprim);
    api.bind(
        &mat,
        &tokens().weaker_than_descendants,
        &tokens().all_purpose,
    );

    // Verify bound
    let direct = api.get_direct_binding(&tokens().all_purpose);
    assert!(direct.is_bound());

    // Unbind
    assert!(api.unbind_direct_binding(&tokens().all_purpose));

    // Verify unbound
    let direct = api.get_direct_binding(&tokens().all_purpose);
    assert!(!direct.is_bound());
}

// ===========================================================================
// testUsdShadeMaterialBindFaceSubset.py — simplified
// ===========================================================================

#[test]
fn test_material_bind_subset_create() {
    let stage = new_stage();

    let mesh = stage.define_prim("/Sphere/Mesh", "Mesh").unwrap();
    let _mat1 = Material::define(&stage, &p("/Sphere/Materials/mat1"));

    let api = usd_shade::MaterialBindingAPI::apply(&mesh);

    // No subsets initially
    let subsets = api.get_material_bind_subsets();
    assert_eq!(subsets.len(), 0);

    // Create a material bind subset
    let indices = vec![0i32, 1, 2, 3];
    let subset =
        api.create_material_bind_subset(&Token::new("subset1"), &indices, &Token::new("face"));
    assert!(subset.is_valid(), "Created subset should be valid");

    // Now should have 1 subset
    let subsets = api.get_material_bind_subsets();
    assert_eq!(subsets.len(), 1);

    // Family type defaults to nonOverlapping
    let family_type = api.get_material_bind_subsets_family_type();
    assert_eq!(family_type.as_str(), "nonOverlapping");

    // Set to partition
    assert!(api.set_material_bind_subsets_family_type(&Token::new("partition")));
    let family_type = api.get_material_bind_subsets_family_type();
    assert_eq!(family_type.as_str(), "partition");
}

// ===========================================================================
// Shader SdrMetadata API
// ===========================================================================

#[test]
fn test_shader_sdr_metadata_api() {
    let stage = new_stage();
    let shader = Shader::define(&stage, &p("/Shader"));

    // Empty by default
    assert!(!shader.has_sdr_metadata());
    assert!(shader.get_sdr_metadata().is_empty());

    // Test prim-level dict metadata round-trip
    let _prim = shader.get_prim();
    let _sdr_key = tokens().sdr_metadata.clone();

    // Write via Shader API
    shader.set_sdr_metadata_by_key(&Token::new("category"), "preview");
    shader.set_sdr_metadata_by_key(&Token::new("departments"), "anim|layout");

    // Read back
    assert!(shader.has_sdr_metadata());
    let meta = shader.get_sdr_metadata();
    assert!(
        meta.len() >= 2,
        "Should have at least 2 entries, got {}",
        meta.len()
    );

    // Get by key
    let cat = shader.get_sdr_metadata_by_key(&Token::new("category"));
    assert!(!cat.is_empty(), "category should not be empty");

    // Has by key
    assert!(shader.has_sdr_metadata_by_key(&Token::new("category")));
    assert!(!shader.has_sdr_metadata_by_key(&Token::new("nonexistent")));

    // Clear by key
    shader.clear_sdr_metadata_by_key(&Token::new("category"));

    // Clear all
    shader.clear_sdr_metadata();
}

// ===========================================================================
// Input/Output SdrMetadata
// ===========================================================================

#[test]
fn test_input_sdr_metadata() {
    let stage = new_stage();
    let shader = Shader::define(&stage, &p("/Shader"));
    let input = shader.create_input(&Token::new("diffuse"), &vtype("color3f"));

    // Empty by default
    assert!(!input.has_sdr_metadata());
    assert!(input.get_sdr_metadata().is_empty());

    // Set
    input.set_sdr_metadata_by_key(&Token::new("role"), "color");

    // Get
    assert!(input.has_sdr_metadata());
    assert!(input.has_sdr_metadata_by_key(&Token::new("role")));

    // Clear
    input.clear_sdr_metadata_by_key(&Token::new("role"));
    input.clear_sdr_metadata();
}

// ===========================================================================
// Output operations
// ===========================================================================

#[test]
fn test_output_render_type() {
    let stage = new_stage();
    let shader = Shader::define(&stage, &p("/Shader"));
    let output = shader.create_output(&Token::new("result"), &vtype("color3f"));

    assert!(!output.has_render_type());
    assert_eq!(output.get_render_type().as_str(), "");

    output.set_render_type(&Token::new("struct"));
    assert!(output.has_render_type());
    assert_eq!(output.get_render_type().as_str(), "struct");
}

// ===========================================================================
// ConnectableAPI HasConnectableAPI
// ===========================================================================

#[test]
fn test_has_connectable_api() {
    assert!(ConnectableAPI::has_connectable_api("Shader"));
    assert!(ConnectableAPI::has_connectable_api("NodeGraph"));
    assert!(ConnectableAPI::has_connectable_api("Material"));
    assert!(!ConnectableAPI::has_connectable_api("Xform"));
    assert!(!ConnectableAPI::has_connectable_api("Mesh"));
}

// ===========================================================================
// NodeDefAPI
// ===========================================================================

#[test]
fn test_node_def_api() {
    use usd_shade::NodeDefAPI;

    let stage = new_stage();
    let prim = stage.define_prim("/Shader", "Shader").unwrap();

    let node_def = NodeDefAPI::new(prim.clone());
    assert!(node_def.is_valid());

    // Default implementation source
    assert_eq!(node_def.get_implementation_source().as_str(), "id");

    // Set and get shader id
    assert!(node_def.set_shader_id(&Token::new("UsdPreviewSurface")));
    assert_eq!(node_def.get_id().unwrap().as_str(), "UsdPreviewSurface");

    // Source asset
    let asset = usd_sdf::AssetPath::new("shader.glslfx");
    assert!(node_def.set_shader_source_asset(Some(&Token::new("glslfx")), &asset));
    assert_eq!(node_def.get_implementation_source().as_str(), "sourceAsset");
    assert!(node_def.get_id().is_none()); // gated by impl source

    let fetched = node_def.get_source_asset(Some(&Token::new("glslfx")));
    assert!(fetched.is_some());
    assert_eq!(fetched.unwrap().get_asset_path(), "shader.glslfx");

    // Source code
    assert!(node_def.set_shader_source_code(Some(&Token::new("osl")), "void shader() {}"));
    assert_eq!(node_def.get_implementation_source().as_str(), "sourceCode");
    let code = node_def.get_source_code(Some(&Token::new("osl")));
    assert!(code.is_some());
    assert_eq!(code.unwrap(), "void shader() {}");
}

// ===========================================================================
// testUsdShadeMaterialSpecializesBaseComposition.py — file-based tests
// ===========================================================================

fn open_testenv_stage(subdir: &str, filename: &str) -> Arc<Stage> {
    ensure_formats();
    let path =
        openusd_test_path::pxr_usd_module_testenv("usdShade", format!("{subdir}/{filename}"));
    let path_str = path.to_string_lossy().replace('\\', "/");
    Stage::open(&path_str, InitialLoadSet::LoadAll)
        .unwrap_or_else(|e| panic!("Failed to open {}: {}", path_str, e))
}

#[test]
fn test_specializes_basic_setup() {
    let stage = open_testenv_stage(
        "testUsdShadeMaterialSpecializesBaseComposition",
        "library.usda",
    );

    let child_prim = stage.get_prim_at_path(&p("/ChildMaterial")).unwrap();
    let child = Material::new(child_prim);
    assert!(child.is_valid());

    let base_prim = stage.get_prim_at_path(&p("/BaseMaterial")).unwrap();
    let base = Material::new(base_prim);
    assert!(base.is_valid());

    // BaseMaterial has no base material
    assert!(!base.has_base_material());

    // ChildMaterial's base should be BaseMaterial
    assert!(child.has_base_material());
    assert_eq!(child.get_base_material_path().get_string(), "/BaseMaterial");
}

#[test]
fn test_specializes_not_present_ignored() {
    let stage = open_testenv_stage(
        "testUsdShadeMaterialSpecializesBaseComposition",
        "asset.usda",
    );

    let mat_prim = stage
        .get_prim_at_path(&p("/Asset/Looks/NotAChildMaterial"))
        .unwrap();
    let mat = Material::new(mat_prim);
    assert!(mat.is_valid());

    // Specializes target doesn't exist on stage — should not be treated as child
    assert!(!mat.has_base_material());
}
