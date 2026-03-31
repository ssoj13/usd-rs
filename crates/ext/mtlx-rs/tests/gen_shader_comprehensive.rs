//! Comprehensive GenShader tests covering ShaderStage emit, Shader management,
//! ShaderGraph classification, ShaderTranslator, GenContext, VariableBlock,
//! token_substitution, and ShaderNode/ShaderPort internals.
//!
//! Ref: C++ MaterialXTest/MaterialXGenShader/GenShader.cpp

use std::sync::Arc;

use mtlx_rs::core::Document;
use mtlx_rs::format::FilePath;
use mtlx_rs::gen_shader::{
    BaseType, GenContext, GenOptions, ImplementationFactory, NopNode, Shader, ShaderGenerator,
    ShaderGraph, ShaderNode, ShaderNodeClassification, ShaderNodeImplCreator, ShaderPortFlag,
    ShaderStage, ShaderTranslator, TypeSystem, VariableBlock, add_stage_connector,
    add_stage_connector_block, add_stage_input, add_stage_output, add_stage_uniform,
    add_stage_uniform_with_value, hash_string, shader_stage, token_substitution, type_desc_types,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Minimal generator for unit tests (same as in gen_shader.rs).
struct TestGenerator {
    type_system: TypeSystem,
}

impl ShaderGenerator for TestGenerator {
    fn get_type_system(&self) -> &TypeSystem {
        &self.type_system
    }
    fn target(&self) -> &str {
        "test"
    }
}

// ===========================================================================
// 1. ShaderStage emit methods
// ===========================================================================

#[test]
fn stage_emit_line_basic() {
    let mut stage = ShaderStage::new("pixel");
    stage.emit_line("float x = 1.0", true);
    assert_eq!(stage.get_source_code(), "float x = 1.0;\n");
}

#[test]
fn stage_emit_line_no_semicolon() {
    let mut stage = ShaderStage::new("pixel");
    stage.emit_line("// comment line", false);
    assert_eq!(stage.get_source_code(), "// comment line\n");
}

#[test]
fn stage_emit_string_raw() {
    let mut stage = ShaderStage::new("pixel");
    stage.emit_string("hello ");
    stage.emit_string("world");
    assert_eq!(stage.get_source_code(), "hello world");
}

#[test]
fn stage_emit_comment() {
    let mut stage = ShaderStage::new("pixel");
    stage.emit_comment("This is a comment");
    assert_eq!(stage.get_source_code(), "// This is a comment\n");
}

#[test]
fn stage_emit_empty_line() {
    let mut stage = ShaderStage::new("pixel");
    stage.emit_line("int a = 0", true);
    stage.emit_empty_line();
    stage.emit_line("int b = 1", true);
    let code = stage.get_source_code();
    assert!(code.contains("int a = 0;\n\nint b = 1;\n"));
}

#[test]
fn stage_scope_begin_end_curly() {
    let mut stage = ShaderStage::new("pixel");
    stage.emit_line("void main()", false);
    stage.emit_scope_begin();
    stage.emit_line("float x = 0.0", true);
    stage.emit_scope_end(false, true);
    let code = stage.get_source_code();
    // Should have opening brace, indented line, closing brace
    assert!(code.contains("{\n"), "must have opening brace");
    assert!(
        code.contains("    float x = 0.0;\n"),
        "body must be indented 4 spaces"
    );
    assert!(code.contains("}\n"), "must have closing brace");
}

#[test]
fn stage_scope_nested() {
    let mut stage = ShaderStage::new("pixel");
    stage.emit_scope_begin(); // indent 1
    stage.emit_scope_begin(); // indent 2
    stage.emit_line("inner", false);
    stage.emit_scope_end(false, true); // indent 1
    stage.emit_scope_end(false, true); // indent 0
    let code = stage.get_source_code();
    // "inner" should be at 8 spaces (2 levels * 4)
    assert!(
        code.contains("        inner\n"),
        "nested inner must be 8 spaces deep"
    );
}

#[test]
fn stage_scope_end_with_semicolon() {
    let mut stage = ShaderStage::new("pixel");
    stage.emit_scope_begin();
    stage.emit_scope_end(true, true);
    let code = stage.get_source_code();
    assert!(code.contains("};\n"), "closing brace with semicolon");
}

#[test]
fn stage_scope_end_no_newline() {
    let mut stage = ShaderStage::new("pixel");
    stage.emit_scope_begin();
    stage.emit_scope_end(false, false);
    let code = stage.get_source_code();
    assert!(code.ends_with('}'), "closing brace without newline");
}

#[test]
fn stage_emit_variable_decl_full() {
    let mut stage = ShaderStage::new("pixel");
    stage.emit_variable_decl("vec3", "color", "uniform", Some("vec3(1.0)"));
    assert_eq!(stage.get_source_code(), "uniform vec3 color = vec3(1.0);\n");
}

#[test]
fn stage_emit_variable_decl_no_qualifier() {
    let mut stage = ShaderStage::new("pixel");
    stage.emit_variable_decl("float", "x", "", None);
    assert_eq!(stage.get_source_code(), "float x;\n");
}

#[test]
fn stage_emit_variable_decl_with_qualifier_no_value() {
    let mut stage = ShaderStage::new("pixel");
    stage.emit_variable_decl("int", "count", "varying", None);
    assert_eq!(stage.get_source_code(), "varying int count;\n");
}

#[test]
fn stage_emit_variable_decl_indented() {
    let mut stage = ShaderStage::new("pixel");
    stage.emit_scope_begin();
    stage.emit_variable_decl("float", "val", "", Some("0.0"));
    stage.emit_scope_end(false, true);
    let code = stage.get_source_code();
    assert!(
        code.contains("    float val = 0.0;\n"),
        "variable decl must be indented inside scope"
    );
}

#[test]
fn stage_indentation_manual_begin_end_line() {
    let mut stage = ShaderStage::new("pixel");
    stage.indentation = 2;
    stage.emit_line_begin();
    stage.emit_string("x = 1");
    stage.emit_line_end(true);
    let code = stage.get_source_code();
    assert_eq!(code, "        x = 1;\n"); // 2 * 4 = 8 spaces
}

#[test]
fn stage_add_indent_line() {
    let mut stage = ShaderStage::new("pixel");
    stage.indentation = 1;
    stage.add_indent_line("return result", true);
    assert_eq!(stage.get_source_code(), "    return result;\n");
}

// ===========================================================================
// 2. Shader creation and stage management
// ===========================================================================

#[test]
fn shader_create_stage() {
    let graph = ShaderGraph::new("test");
    let mut shader = Shader::new("TestShader", graph);
    assert_eq!(shader.get_name(), "TestShader");
    assert_eq!(shader.num_stages(), 0);
    assert!(!shader.has_stage("pixel"));

    shader.create_stage("pixel");
    assert!(shader.has_stage("pixel"));
    assert_eq!(shader.num_stages(), 1);

    // Creating same stage again returns existing
    shader.create_stage("pixel");
    assert_eq!(shader.num_stages(), 1);
}

#[test]
fn shader_new_hw_has_vertex_and_pixel() {
    let graph = ShaderGraph::new("hw_test");
    let shader = Shader::new_hw("HwShader", graph);
    assert!(shader.has_stage(shader_stage::VERTEX));
    assert!(shader.has_stage(shader_stage::PIXEL));
    assert_eq!(shader.num_stages(), 2);
}

#[test]
fn shader_get_stage_by_index_and_name() {
    let graph = ShaderGraph::new("test");
    let mut shader = Shader::new("S", graph);
    shader.create_stage("vertex");
    shader.create_stage("pixel");

    assert_eq!(shader.get_stage(0).unwrap().get_name(), "vertex");
    assert_eq!(shader.get_stage(1).unwrap().get_name(), "pixel");
    assert!(shader.get_stage(2).is_none());

    assert_eq!(
        shader.get_stage_by_name("vertex").unwrap().get_name(),
        "vertex"
    );
    assert!(shader.get_stage_by_name("geometry").is_none());
}

#[test]
fn shader_set_get_source_code() {
    let graph = ShaderGraph::new("g");
    let mut shader = Shader::new("S", graph);
    shader.create_stage("pixel");
    shader.set_source_code("void main() { }", "pixel");
    assert_eq!(shader.get_source_code("pixel"), "void main() { }");
    // Non-existent stage returns empty
    assert_eq!(shader.get_source_code("vertex"), "");
}

#[test]
fn shader_attributes() {
    use mtlx_rs::core::Value;

    let graph = ShaderGraph::new("g");
    let mut shader = Shader::new("S", graph);
    assert!(shader.get_attribute("transparent").is_none());

    shader.set_attribute("transparent", Value::Boolean(true));
    let val = shader.get_attribute("transparent").unwrap();
    match val {
        Value::Boolean(b) => assert!(*b),
        _ => panic!("expected bool attribute"),
    }
}

#[test]
fn shader_into_parts_and_from_parts() {
    let graph = ShaderGraph::new("g");
    let mut shader = Shader::new("S", graph);
    shader.create_stage("vertex");
    shader.create_stage("pixel");
    shader.set_source_code("vs_code", "vertex");
    shader.set_source_code("ps_code", "pixel");

    let (graph, stages) = shader.into_parts();
    assert_eq!(stages.len(), 2);

    let restored = Shader::from_parts("S", graph, stages);
    assert_eq!(restored.get_source_code("vertex"), "vs_code");
    assert_eq!(restored.get_source_code("pixel"), "ps_code");
}

// ===========================================================================
// 3. VariableBlock
// ===========================================================================

#[test]
fn variable_block_add_and_find() {
    let mut block = VariableBlock::new("uniforms", "u");
    assert!(block.is_empty());
    assert_eq!(block.size(), 0);
    assert_eq!(block.get_name(), "uniforms");
    assert_eq!(block.get_instance(), "u");

    block.add(type_desc_types::float(), "opacity", None, false);
    assert_eq!(block.size(), 1);
    assert!(!block.is_empty());

    let found = block.find("opacity").unwrap();
    assert_eq!(found.get_name(), "opacity");
    assert!(block.find("missing").is_none());
}

#[test]
fn variable_block_widening() {
    let mut block = VariableBlock::new("inputs", "i");
    // Add a float first
    block.add(type_desc_types::float(), "val", None, true);
    assert_eq!(block.find("val").unwrap().get_type().get_name(), "float");

    // Add same name with larger type and widening enabled
    block.add(type_desc_types::color3(), "val", None, true);
    assert_eq!(
        block.find("val").unwrap().get_type().get_name(),
        "color3",
        "should widen float to color3"
    );

    // Size stays 1 (same variable, just widened)
    assert_eq!(block.size(), 1);
}

#[test]
fn variable_block_no_widening_keeps_original() {
    let mut block = VariableBlock::new("inputs", "i");
    block.add(type_desc_types::float(), "val", None, false);
    // Add same name with larger type, widening disabled
    block.add(type_desc_types::color3(), "val", None, false);
    assert_eq!(
        block.find("val").unwrap().get_type().get_name(),
        "float",
        "should NOT widen when should_widen=false"
    );
}

#[test]
fn variable_block_get_by_index() {
    let mut block = VariableBlock::new("outputs", "o");
    block.add(type_desc_types::float(), "a", None, false);
    block.add(type_desc_types::color3(), "b", None, false);

    assert_eq!(block.get(0).unwrap().get_name(), "a");
    assert_eq!(block.get(1).unwrap().get_name(), "b");
    assert!(block.get(2).is_none());
}

#[test]
fn variable_block_order() {
    let mut block = VariableBlock::new("outputs", "o");
    block.add(type_desc_types::float(), "z_var", None, false);
    block.add(type_desc_types::float(), "a_var", None, false);
    block.add(type_desc_types::float(), "m_var", None, false);
    let order = block.get_variable_order();
    assert_eq!(order, &["z_var", "a_var", "m_var"]);
}

#[test]
fn variable_block_with_value() {
    use mtlx_rs::core::Value;

    let mut block = VariableBlock::new("uniforms", "u");
    block.add(
        type_desc_types::float(),
        "roughness",
        Some(Value::Float(0.5)),
        false,
    );
    let port = block.find("roughness").unwrap();
    match port.get_value().unwrap() {
        Value::Float(f) => assert!((*f - 0.5).abs() < f32::EPSILON),
        other => panic!("expected Float, got {:?}", other),
    }
}

// ===========================================================================
// 4. ShaderStage blocks and dependencies
// ===========================================================================

#[test]
fn stage_create_and_get_blocks() {
    let mut stage = ShaderStage::new("pixel");
    stage.create_uniform_block("PublicUniforms", "pub_u");
    stage.create_input_block("VertexData", "vd");
    stage.create_output_block("PixelOutputs", "po");

    assert!(stage.get_uniform_block("PublicUniforms").is_some());
    assert!(stage.get_input_block("VertexData").is_some());
    assert!(stage.get_output_block("PixelOutputs").is_some());
    assert!(stage.get_uniform_block("Missing").is_none());
}

#[test]
fn stage_source_dependencies_dedup() {
    let mut stage = ShaderStage::new("pixel");
    assert!(!stage.has_source_dependency("lib/mx_math.glsl"));

    stage.add_source_dependency("lib/mx_math.glsl");
    assert!(stage.has_source_dependency("lib/mx_math.glsl"));

    // Adding same dependency again is idempotent
    stage.add_source_dependency("lib/mx_math.glsl");
    assert!(stage.has_source_dependency("lib/mx_math.glsl"));
}

#[test]
fn stage_function_call_emitted_tracking() {
    let mut stage = ShaderStage::new("pixel");
    assert!(!stage.is_function_call_emitted("mx_image"));

    stage.add_function_call_emitted("mx_image");
    assert!(stage.is_function_call_emitted("mx_image"));
    assert!(!stage.is_function_call_emitted("mx_noise"));
}

#[test]
fn stage_function_definition_dedup_by_name() {
    let mut stage = ShaderStage::new("pixel");
    assert!(!stage.has_function_definition("mx_image_float"));

    let added = stage.add_function_definition("mx_image_float");
    assert!(added);
    assert!(stage.has_function_definition("mx_image_float"));

    // Second time returns false (duplicate)
    let added2 = stage.add_function_definition("mx_image_float");
    assert!(!added2);
}

#[test]
fn stage_function_definition_by_hash() {
    let mut stage = ShaderStage::new("pixel");
    let hash = hash_string("mx_image_float_body");

    let emitted = stage.add_function_definition_by_hash(hash, |s| {
        s.emit_line("float mx_image_float() { return 0.0; }", false);
    });
    assert!(emitted);
    assert!(stage.get_source_code().contains("mx_image_float"));

    // Duplicate should NOT emit
    let prev_len = stage.get_source_code().len();
    let emitted2 = stage.add_function_definition_by_hash(hash, |s| {
        s.emit_line("DUPLICATE", false);
    });
    assert!(!emitted2);
    assert_eq!(
        stage.get_source_code().len(),
        prev_len,
        "duplicate hash must not emit code"
    );
}

#[test]
fn stage_includes() {
    let mut stage = ShaderStage::new("pixel");
    stage.add_include("lib/mx_math.glsl");
    stage.add_include("lib/mx_transform.glsl");
    stage.add_include("lib/mx_math.glsl"); // duplicate
    assert_eq!(stage.includes.len(), 2);
}

#[test]
fn stage_function_name() {
    let mut stage = ShaderStage::new("pixel");
    assert!(stage.get_function_name().is_empty());
    stage.set_function_name("fragmentMain");
    assert_eq!(stage.get_function_name(), "fragmentMain");
}

// ===========================================================================
// 5. add_stage_* helper functions
// ===========================================================================

#[test]
fn add_stage_input_creates_block() {
    let mut stage = ShaderStage::new("vertex");
    add_stage_input(
        "VertexInputs",
        type_desc_types::vector3(),
        "position",
        &mut stage,
        false,
    );
    let block = stage.get_input_block("VertexInputs").unwrap();
    assert_eq!(block.size(), 1);
    assert!(block.find("position").is_some());
}

#[test]
fn add_stage_output_creates_block() {
    let mut stage = ShaderStage::new("pixel");
    add_stage_output(
        "PixelOutputs",
        type_desc_types::color4(),
        "out_color",
        &mut stage,
        false,
    );
    let block = stage.get_output_block("PixelOutputs").unwrap();
    assert!(block.find("out_color").is_some());
}

#[test]
fn add_stage_uniform_creates_block() {
    let mut stage = ShaderStage::new("pixel");
    add_stage_uniform(
        "PublicUniforms",
        type_desc_types::float(),
        "roughness",
        &mut stage,
    );
    let block = stage.get_uniform_block("PublicUniforms").unwrap();
    assert!(block.find("roughness").is_some());
}

#[test]
fn add_stage_uniform_with_value_stores_value() {
    use mtlx_rs::core::Value;

    let mut stage = ShaderStage::new("pixel");
    add_stage_uniform_with_value(
        "PublicUniforms",
        type_desc_types::float(),
        "metalness",
        &mut stage,
        Some(Value::Float(1.0)),
    );
    let block = stage.get_uniform_block("PublicUniforms").unwrap();
    let port = block.find("metalness").unwrap();
    match port.get_value().unwrap() {
        Value::Float(f) => assert!((*f - 1.0).abs() < f32::EPSILON),
        other => panic!("expected Float, got {:?}", other),
    }
}

#[test]
fn add_stage_connector_links_two_stages() {
    let mut vs = ShaderStage::new("vertex");
    let mut ps = ShaderStage::new("pixel");
    add_stage_connector(
        "VertexData",
        "vd",
        type_desc_types::vector3(),
        "normal",
        &mut vs,
        &mut ps,
        false,
    );
    // VS should have output block with the variable
    let vs_out = vs.get_output_block("VertexData").unwrap();
    assert!(vs_out.find("normal").is_some());
    // PS should have input block with the variable
    let ps_in = ps.get_input_block("VertexData").unwrap();
    assert!(ps_in.find("normal").is_some());
}

#[test]
fn add_stage_connector_block_creates_empty_blocks() {
    let mut vs = ShaderStage::new("vertex");
    let mut ps = ShaderStage::new("pixel");
    add_stage_connector_block("LightData", "ld", &mut vs, &mut ps);
    assert!(vs.get_output_block("LightData").unwrap().is_empty());
    assert!(ps.get_input_block("LightData").unwrap().is_empty());
}

// ===========================================================================
// 6. ShaderNode classification
// ===========================================================================

#[test]
fn shader_node_classification_flags() {
    let mut node = ShaderNode::new("my_bsdf");
    assert!(!node.has_classification(ShaderNodeClassification::BSDF));

    node.set_classification(ShaderNodeClassification::CLOSURE | ShaderNodeClassification::BSDF);
    assert!(node.has_classification(ShaderNodeClassification::BSDF));
    assert!(node.has_classification(ShaderNodeClassification::CLOSURE));
    assert!(!node.has_classification(ShaderNodeClassification::TEXTURE));

    // add_classification ORs additional flags
    node.add_classification(ShaderNodeClassification::BSDF_R);
    assert!(node.has_classification(ShaderNodeClassification::BSDF_R));
    assert!(node.has_classification(ShaderNodeClassification::BSDF)); // still set
}

#[test]
fn shader_node_impl_name() {
    let mut node = ShaderNode::new("n");
    assert!(node.get_impl_name().is_none());
    node.set_impl_name("IM_standard_surface_surfaceshader_genglsl");
    assert_eq!(
        node.get_impl_name().unwrap(),
        "IM_standard_surface_surfaceshader_genglsl"
    );
    node.clear_impl_name();
    assert!(node.get_impl_name().is_none());
}

#[test]
fn shader_node_input_output_by_index() {
    let mut node = ShaderNode::new("n");
    node.add_input("a", type_desc_types::float());
    node.add_input("b", type_desc_types::color3());
    node.add_output("out", type_desc_types::float());

    assert_eq!(node.get_input_at(0).unwrap().get_name(), "a");
    assert_eq!(node.get_input_at(1).unwrap().get_name(), "b");
    assert!(node.get_input_at(2).is_none());
    assert_eq!(node.get_output_at(0).unwrap().get_name(), "out");
}

// ===========================================================================
// 7. ShaderPort flags and properties
// ===========================================================================

#[test]
fn shader_port_flags() {
    let mut node = ShaderNode::new("n");
    let input = node.add_input("val", type_desc_types::float());
    assert!(!input.port().is_uniform());
    assert!(!input.port().is_emitted());

    input.port_mut().set_uniform(true);
    assert!(input.port().is_uniform());

    input.port_mut().set_emitted(true);
    assert!(input.port().is_emitted());

    // Test raw flag API
    input.port_mut().set_flag(ShaderPortFlag::BIND_INPUT, true);
    assert!(input.port().get_flag(ShaderPortFlag::BIND_INPUT));

    input.port_mut().set_flag(ShaderPortFlag::BIND_INPUT, false);
    assert!(!input.port().get_flag(ShaderPortFlag::BIND_INPUT));
}

#[test]
fn shader_port_value_and_authored() {
    use mtlx_rs::core::Value;

    let mut node = ShaderNode::new("n");
    let input = node.add_input("roughness", type_desc_types::float());
    assert!(input.port().get_value().is_none());

    input.port_mut().set_value(Some(Value::Float(0.3)), true);
    assert!(input.port().get_flag(ShaderPortFlag::AUTHORED_VALUE));
    assert_eq!(input.port().get_value_string(), "0.3");

    input.port_mut().set_value(Some(Value::Float(0.5)), false);
    assert!(!input.port().get_flag(ShaderPortFlag::AUTHORED_VALUE));
}

#[test]
fn shader_port_path_and_variable() {
    let mut node = ShaderNode::new("n");
    let input = node.add_input("color", type_desc_types::color3());
    assert_eq!(input.port().get_variable(), "color"); // default = name

    input.port_mut().set_variable("color_42");
    assert_eq!(input.port().get_variable(), "color_42");

    input.port_mut().set_path("/materials/mat1/base_color");
    assert_eq!(input.port().path, "/materials/mat1/base_color");
}

#[test]
fn shader_port_metadata() {
    let mut node = ShaderNode::new("n");
    let input = node.add_input("val", type_desc_types::float());
    assert!(input.port().get_metadata().is_empty());

    input.port_mut().add_metadata("uimin", "0.0");
    input.port_mut().add_metadata("uimax", "1.0");
    assert_eq!(input.port().get_metadata().len(), 2);
    assert_eq!(input.port().get_metadata()[0].name, "uimin");
    assert_eq!(input.port().get_metadata()[1].value, "1.0");
}

#[test]
fn shader_input_connection() {
    let mut node = ShaderNode::new("add");
    let input = node.add_input("in1", type_desc_types::float());
    assert!(!input.has_connection());
    assert!(input.get_connection().is_none());

    input.make_connection("upstream_node", "out");
    assert!(input.has_connection());
    let (node_name, output_name) = input.get_connection().unwrap();
    assert_eq!(node_name, "upstream_node");
    assert_eq!(output_name, "out");

    input.break_connection();
    assert!(!input.has_connection());
}

// ===========================================================================
// 8. ShaderGraph classification and connections
// ===========================================================================

#[test]
fn shader_graph_has_classification() {
    let mut graph = ShaderGraph::new("g");
    graph
        .node
        .set_classification(ShaderNodeClassification::SHADER | ShaderNodeClassification::SURFACE);
    assert!(graph.has_classification(ShaderNodeClassification::SHADER));
    assert!(graph.has_classification(ShaderNodeClassification::SURFACE));
    assert!(!graph.has_classification(ShaderNodeClassification::VOLUME));
}

#[test]
fn shader_graph_empty() {
    let graph = ShaderGraph::new("empty");
    assert_eq!(graph.num_input_sockets(), 0);
    assert_eq!(graph.num_output_sockets(), 0);
    assert!(graph.get_node("nonexistent").is_none());
}

#[test]
fn shader_graph_multiple_nodes() {
    let mut graph = ShaderGraph::new("g");
    let n1 = graph.create_node("noise");
    n1.add_output("out", type_desc_types::float());
    let n2 = graph.create_node("multiply");
    n2.add_input("in1", type_desc_types::float());
    n2.add_input("in2", type_desc_types::float());
    n2.add_output("out", type_desc_types::float());
    let n3 = graph.create_node("add");
    n3.add_input("in1", type_desc_types::float());
    n3.add_output("out", type_desc_types::float());

    assert!(graph.get_node("noise").is_some());
    assert!(graph.get_node("multiply").is_some());
    assert!(graph.get_node("add").is_some());

    // Connect: noise.out -> multiply.in1
    graph
        .make_connection("multiply", "in1", "noise", "out")
        .unwrap();
    // Connect: multiply.out -> add.in1
    graph
        .make_connection("add", "in1", "multiply", "out")
        .unwrap();

    // Topological sort should place noise before multiply before add
    graph.topological_sort();
    let order: Vec<&str> = graph.node_order.iter().map(|s| s.as_str()).collect();
    let pos_noise = order.iter().position(|&x| x == "noise").unwrap();
    let pos_mul = order.iter().position(|&x| x == "multiply").unwrap();
    let pos_add = order.iter().position(|&x| x == "add").unwrap();
    assert!(pos_noise < pos_mul);
    assert!(pos_mul < pos_add);
}

// ===========================================================================
// 9. ShaderTranslator
// ===========================================================================

#[test]
fn translator_same_category_error() {
    let mut doc = Document::new();
    let elem = doc
        .add_child_of_category("standard_surface", "SS_test")
        .unwrap();
    let mut translator = ShaderTranslator::new();
    // Translating to the same category should fail, matching the C++ contract.
    let result = translator.translate_shader(&elem, "standard_surface");
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("already"));
}

#[test]
fn translator_different_category_error() {
    let mut doc = Document::new();
    let elem = doc
        .add_child_of_category("standard_surface", "SS_test")
        .unwrap();
    let mut translator = ShaderTranslator::new();
    // Different category without translation graph -> error
    let result = translator.translate_shader(&elem, "gltf_pbr");
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("standard_surface"));
    assert!(err_msg.contains("gltf_pbr"));
}

#[test]
fn translator_translate_all_materials_empty_doc() {
    let doc = Document::new();
    let mut translator = ShaderTranslator::new();
    let errors = translator.translate_all_materials(&doc, "UsdPreviewSurface");
    assert!(errors.is_empty(), "empty doc should produce no errors");
}

#[test]
fn translator_translate_all_materials_with_material() {
    let mut doc = Document::new();
    let mat = doc
        .add_child_of_category("surfacematerial", "MAT_test")
        .unwrap();
    // Add a surfaceshader input pointing to a shader node.
    let inp = mtlx_rs::core::add_child_of_category(&mat, "input", "surfaceshader").unwrap();
    inp.borrow_mut().set_type("surfaceshader");
    inp.borrow_mut().set_node_name("SR_test");
    // Add the shader node itself.
    let _shader = doc
        .add_child_of_category("standard_surface", "SR_test")
        .unwrap();
    let mut translator = ShaderTranslator::new();
    let errors = translator.translate_all_materials(&doc, "UsdPreviewSurface");
    // Shader exists but no translation nodedef -> error for that shader.
    assert!(!errors.is_empty());
}

#[test]
fn translator_requires_output_suffix() {
    let mut doc = Document::new();
    let shader = doc
        .add_child_of_category("standard_surface", "SS_test")
        .unwrap();
    shader.borrow_mut().set_type("surfaceshader");

    let nodedef = doc
        .add_node_def(
            "ND_standard_surface_to_gltf_pbr",
            "",
            "standard_surface_to_gltf_pbr",
        )
        .unwrap();
    let bad_output = mtlx_rs::core::add_child_of_category(&nodedef, "output", "base").unwrap();
    bad_output.borrow_mut().set_type("surfaceshader");

    let mut translator = ShaderTranslator::new();
    let result = translator.translate_shader(&shader, "gltf_pbr");
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("does not end with '_out'"));
}

#[test]
fn translator_copies_value_unit_and_colorspace_to_translation_input() {
    let mut doc = Document::new();
    let shader = doc
        .add_child_of_category("standard_surface", "SS_test")
        .unwrap();
    shader.borrow_mut().set_type("surfaceshader");

    let amount = mtlx_rs::core::add_child_of_category(&shader, "input", "amount").unwrap();
    amount.borrow_mut().set_type("float");
    amount.borrow_mut().set_value_string("0.25");
    amount.borrow_mut().set_color_space("lin_rec709");
    amount.borrow_mut().set_unit("meter");
    amount.borrow_mut().set_unit_type("distance");

    let nodedef = doc
        .add_node_def(
            "ND_standard_surface_to_gltf_pbr",
            "",
            "standard_surface_to_gltf_pbr",
        )
        .unwrap();
    let nd_input = mtlx_rs::core::add_child_of_category(&nodedef, "input", "amount").unwrap();
    nd_input.borrow_mut().set_type("float");
    let nd_output = mtlx_rs::core::add_child_of_category(&nodedef, "output", "amount_out").unwrap();
    nd_output.borrow_mut().set_type("float");

    let mut translator = ShaderTranslator::new();
    translator.translate_shader(&shader, "gltf_pbr").unwrap();

    let graphs = doc.get_node_graphs();
    assert_eq!(graphs.len(), 1);
    let graph = &graphs[0];
    let translation_node = graph
        .borrow()
        .get_child("standard_surface_to_gltf_pbr")
        .unwrap();
    let translation_input = translation_node.borrow().get_child("amount").unwrap();
    let input_ref = translation_input.borrow();
    assert_eq!(input_ref.get_value_string(), "0.25");
    assert_eq!(input_ref.get_unit().unwrap(), "meter");
    assert_eq!(input_ref.get_unit_type().unwrap(), "distance");
    drop(input_ref);
    assert_eq!(
        mtlx_rs::core::get_active_color_space(&translation_input),
        "lin_rec709"
    );
}

#[test]
fn translator_translate_graph_same_model_noop() {
    let doc = Document::new();
    let mut graph = ShaderGraph::new("g");
    let mut translator = ShaderTranslator::new();
    let result =
        translator.translate_graph(&mut graph, &doc, "standard_surface", "standard_surface");
    assert!(result.is_ok());
}

#[test]
fn translator_translate_graph_different_model_error() {
    let doc = Document::new();
    let mut graph = ShaderGraph::new("g");
    let mut translator = ShaderTranslator::new();
    let result = translator.translate_graph(&mut graph, &doc, "standard_surface", "gltf_pbr");
    assert!(result.is_err());
}

// ===========================================================================
// 10. GenContext: impl cache, options, source resolution
// ===========================================================================

#[test]
fn gen_context_impl_cache() {
    let test_gen = TestGenerator {
        type_system: TypeSystem::new(),
    };
    let mut ctx = GenContext::new(test_gen);

    assert!(ctx.get_cached_impl("ND_image_float").is_none());
    ctx.cache_impl("ND_image_float", "IM_image_float_genglsl");
    assert_eq!(
        ctx.get_cached_impl("ND_image_float").unwrap(),
        "IM_image_float_genglsl"
    );

    // Override
    ctx.cache_impl("ND_image_float", "IM_image_float_genglsl_v2");
    assert_eq!(
        ctx.get_cached_impl("ND_image_float").unwrap(),
        "IM_image_float_genglsl_v2"
    );

    ctx.clear_impl_cache();
    assert!(ctx.get_cached_impl("ND_image_float").is_none());
}

#[test]
fn gen_context_clear_node_implementations_alias() {
    let test_gen = TestGenerator {
        type_system: TypeSystem::new(),
    };
    let mut ctx = GenContext::new(test_gen);
    ctx.cache_impl("ND_add_float", "IM_add_float_genglsl");
    ctx.clear_node_implementations();
    assert!(ctx.get_cached_impl("ND_add_float").is_none());
}

#[test]
fn gen_context_options_mutation() {
    let test_gen = TestGenerator {
        type_system: TypeSystem::new(),
    };
    let mut ctx = GenContext::new(test_gen);

    assert!(ctx.get_options().elide_constant_nodes);
    ctx.get_options_mut().elide_constant_nodes = false;
    assert!(!ctx.get_options().elide_constant_nodes);

    ctx.get_options_mut().hw_transparency = true;
    assert!(ctx.get_options().hw_transparency);
}

#[test]
fn gen_context_source_code_search_path() {
    let test_gen = TestGenerator {
        type_system: TypeSystem::new(),
    };
    let mut ctx = GenContext::new(test_gen);

    let lib_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("libraries/stdlib/genglsl");
    ctx.register_source_code_search_path(FilePath::new(&lib_path));

    // Should be able to resolve a file that exists in that path
    let resolved = ctx.resolve_source_file("mx_image_float.glsl", None);
    assert!(
        resolved.is_some(),
        "should resolve mx_image_float.glsl from registered path"
    );
}

#[test]
fn gen_context_type_desc() {
    let test_gen = TestGenerator {
        type_system: TypeSystem::new(),
    };
    let ctx = GenContext::new(test_gen);
    let float_td = ctx.get_type_desc("float");
    assert_eq!(float_td.get_base_type(), BaseType::Float);
}

// ===========================================================================
// 11. token_substitution (C++ parity tests from GenShader.cpp)
// ===========================================================================

#[test]
fn token_substitution_cpp_parity_threeheaded_monkey() {
    // C++ test: "Look behind you, a $threeheaded $monkey!"
    let source = "Look behind you, a $threeheaded $monkey!";
    let subs = vec![
        ("$threeheaded".to_string(), "mighty".to_string()),
        ("$monkey".to_string(), "pirate".to_string()),
    ];
    let result = token_substitution(source, &subs);
    assert_eq!(result, "Look behind you, a mighty pirate!");
}

#[test]
fn token_substitution_uniform_name() {
    // C++ test: substituting $T_ENV_RADIANCE -> u_envRadiance (HW constant pattern)
    let source = "uniform vec3 $T_ENV_RADIANCE;";
    let subs = vec![("$T_ENV_RADIANCE".to_string(), "u_envRadiance".to_string())];
    let result = token_substitution(source, &subs);
    assert_eq!(result, "uniform vec3 u_envRadiance;");
}

#[test]
fn token_substitution_empty_source() {
    let result = token_substitution("", &[("$X".to_string(), "y".to_string())]);
    assert_eq!(result, "");
}

#[test]
fn token_substitution_no_tokens_in_source() {
    let result = token_substitution("plain text", &[("$X".to_string(), "y".to_string())]);
    assert_eq!(result, "plain text");
}

#[test]
fn token_substitution_dollar_at_end() {
    let result = token_substitution("cost is $", &[]);
    assert_eq!(result, "cost is $");
}

#[test]
fn token_substitution_consecutive_tokens() {
    let source = "$A$B$C";
    let subs = vec![
        ("$A".to_string(), "1".to_string()),
        ("$B".to_string(), "2".to_string()),
        ("$C".to_string(), "3".to_string()),
    ];
    assert_eq!(token_substitution(source, &subs), "123");
}

#[test]
fn token_substitution_partial_match() {
    // $VAR_NAME should match as one token, not $VAR + _NAME
    let source = "$VAR_NAME";
    let subs = vec![
        ("$VAR".to_string(), "x".to_string()),
        ("$VAR_NAME".to_string(), "full_match".to_string()),
    ];
    // The tokenizer reads the whole alphanumeric+_ sequence, so it should match $VAR_NAME
    assert_eq!(token_substitution(source, &subs), "full_match");
}

// ===========================================================================
// 12. hash_string utility
// ===========================================================================

#[test]
fn hash_string_deterministic() {
    let h1 = hash_string("mx_image_float");
    let h2 = hash_string("mx_image_float");
    assert_eq!(h1, h2, "same string must hash identically");
}

#[test]
fn hash_string_different_for_different_strings() {
    let h1 = hash_string("mx_image_float");
    let h2 = hash_string("mx_image_color3");
    assert_ne!(
        h1, h2,
        "different strings should (very likely) hash differently"
    );
}

// ===========================================================================
// 13. GenOptions full coverage
// ===========================================================================

#[test]
fn gen_options_all_defaults() {
    use mtlx_rs::gen_shader::{
        HwDirectionalAlbedoMethod, HwSpecularEnvironmentMethod, HwTransmissionRenderMethod,
        ShaderInterfaceType,
    };

    let opts = GenOptions::default();
    assert!(opts.elide_constant_nodes);
    assert_eq!(opts.library_prefix, "libraries");
    assert!(!opts.hw_transparency);
    assert!(!opts.hw_shadow_map);
    assert!(!opts.hw_ambient_occlusion);
    assert_eq!(opts.hw_max_active_light_sources, 3);
    assert!(opts.hw_implicit_bitangents);
    assert_eq!(
        opts.hw_specular_environment_method,
        HwSpecularEnvironmentMethod::Fis
    );
    assert_eq!(
        opts.hw_directional_albedo_method,
        HwDirectionalAlbedoMethod::Analytic
    );
    assert_eq!(
        opts.hw_transmission_render_method,
        HwTransmissionRenderMethod::Refraction
    );
    assert_eq!(opts.shader_interface_type, ShaderInterfaceType::Complete);
    assert!(!opts.file_texture_vertical_flip);
}

#[test]
fn gen_options_modify_and_check() {
    use mtlx_rs::gen_shader::HwSpecularEnvironmentMethod;

    let mut opts = GenOptions::default();
    opts.hw_transparency = true;
    opts.hw_shadow_map = true;
    opts.hw_max_active_light_sources = 8;
    opts.hw_specular_environment_method = HwSpecularEnvironmentMethod::Prefilter;
    opts.elide_constant_nodes = false;

    assert!(opts.hw_transparency);
    assert!(opts.hw_shadow_map);
    assert_eq!(opts.hw_max_active_light_sources, 8);
    assert_eq!(
        opts.hw_specular_environment_method,
        HwSpecularEnvironmentMethod::Prefilter
    );
    assert!(!opts.elide_constant_nodes);
}

// ===========================================================================
// 14. TypeSystem additional coverage
// ===========================================================================

#[test]
fn type_system_register_custom_type() {
    let mut ts = TypeSystem::new();
    // Standard types exist
    assert_eq!(ts.get_type("float").get_base_type(), BaseType::Float);
    assert_eq!(ts.get_type("integer").get_base_type(), BaseType::Integer);

    // Register custom type
    ts.register_type_custom(
        "custom5",
        BaseType::Float,
        mtlx_rs::gen_shader::type_desc_types::float().get_semantic(),
        5,
        None,
    );
    let custom = ts.get_type("custom5");
    assert_eq!(custom.get_name(), "custom5");
    assert_eq!(custom.get_base_type(), BaseType::Float);
    assert_eq!(custom.get_size(), 5);

    // Unknown type returns NONE
    let none = ts.get_type("nonexistent");
    assert_eq!(none.get_base_type(), BaseType::None);
}

// ===========================================================================
// 15. Shader classification through graph
// ===========================================================================

#[test]
fn shader_has_classification_delegates_to_graph() {
    let mut graph = ShaderGraph::new("g");
    graph
        .node
        .set_classification(ShaderNodeClassification::SHADER | ShaderNodeClassification::SURFACE);
    let shader = Shader::new("S", graph);
    assert!(shader.has_classification(ShaderNodeClassification::SHADER));
    assert!(shader.has_classification(ShaderNodeClassification::SURFACE));
    assert!(!shader.has_classification(ShaderNodeClassification::MATERIAL));
}

// ===========================================================================
// 16. ShaderStage constant block
// ===========================================================================

#[test]
fn stage_constant_block() {
    let mut stage = ShaderStage::new("pixel");
    assert!(stage.get_constant_block().is_empty());

    stage.get_constant_block_mut().add(
        type_desc_types::float(),
        "PI",
        Some(mtlx_rs::core::Value::Float(3.14159)),
        false,
    );
    assert_eq!(stage.get_constant_block().size(), 1);
    assert!(stage.get_constant_block().find("PI").is_some());
}

// ===========================================================================
// 17. ImplementationFactory additional coverage
// ===========================================================================

#[test]
fn implementation_factory_create_not_registered() {
    let factory = ImplementationFactory::new();
    let result = factory.create("IM_nonexistent");
    assert!(result.is_none());
}

#[test]
fn implementation_factory_multiple_registrations() {
    let creator: ShaderNodeImplCreator = Arc::new(|| NopNode::create());
    let mut factory = ImplementationFactory::new();
    factory.register("IM_a", creator.clone());
    factory.register("IM_b", creator.clone());
    assert!(factory.is_registered("IM_a"));
    assert!(factory.is_registered("IM_b"));
    assert!(!factory.is_registered("IM_c"));
}

// ===========================================================================
// 18. Emit pattern: multi-line function with scoping (integration)
// ===========================================================================

#[test]
fn emit_integration_function_pattern() {
    let mut stage = ShaderStage::new("pixel");

    // Emit a typical GLSL function pattern
    stage.emit_line("vec4 myFunction(vec3 color, float alpha)", false);
    stage.emit_scope_begin();
    stage.emit_variable_decl("vec4", "result", "", Some("vec4(0.0)"));
    stage.emit_line("result.rgb = color", true);
    stage.emit_line("result.a = alpha", true);
    stage.emit_line("return result", true);
    stage.emit_scope_end(false, true);

    let code = stage.get_source_code();
    assert!(code.contains("vec4 myFunction(vec3 color, float alpha)\n"));
    assert!(code.contains("{\n"));
    assert!(code.contains("    vec4 result = vec4(0.0);\n"));
    assert!(code.contains("    result.rgb = color;\n"));
    assert!(code.contains("    result.a = alpha;\n"));
    assert!(code.contains("    return result;\n"));
    assert!(code.contains("}\n"));
}

#[test]
fn emit_integration_nested_if_else() {
    let mut stage = ShaderStage::new("pixel");

    stage.emit_line("if (condition)", false);
    stage.emit_scope_begin();
    stage.emit_line("doA()", true);
    stage.emit_scope_end(false, true);
    stage.emit_line("else", false);
    stage.emit_scope_begin();
    stage.emit_line("doB()", true);
    stage.emit_scope_end(false, true);

    let code = stage.get_source_code();
    assert!(code.contains("if (condition)\n"));
    assert!(code.contains("    doA();\n"));
    assert!(code.contains("else\n"));
    assert!(code.contains("    doB();\n"));
}
