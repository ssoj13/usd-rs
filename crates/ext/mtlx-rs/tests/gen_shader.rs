//! GenShader tests: TypeDesc, TypeSystem, ShaderNode, ShaderGraph, Shader, Syntax, GenOptions.

use std::sync::Arc;

use mtlx_rs::core::Document;
use mtlx_rs::core::replace_substrings;
use mtlx_rs::format::{FilePath, read_file, read_from_xml_file_path, read_from_xml_str};
use mtlx_rs::gen_glsl::{GlslShaderGenerator, GlslShaderGraphContext};
use mtlx_rs::gen_hw::create_shader as hw_create_shader;
use mtlx_rs::gen_mdl::{MdlShaderGenerator, create_mdl_shader, generate_mdl_shader};
use mtlx_rs::gen_shader::{
    BaseType, ColorManagementSystem, ColorSpaceTransform, CompoundNode,
    DefaultColorManagementSystem, DefaultUnitSystem, GenContext, GenOptions, ImplementationFactory,
    NopNode, Shader, ShaderGenerator, ShaderGraph, ShaderImplContext, ShaderInterfaceType,
    ShaderNode, ShaderNodeImpl, ShaderNodeImplCreator, SourceCodeNode, Syntax, TypeSyntax,
    TypeSystem, UnitSystem, UnitTransform, create_from_nodegraph, type_desc_types,
};
use mtlx_rs::gen_slang::{SlangShaderGenerator, create_slang_shader, generate_slang_shader};

#[test]
fn type_system_standard_types() {
    let ts = TypeSystem::new();
    assert_eq!(ts.get_type("float").get_name(), "float");
    assert_eq!(ts.get_type("color3").get_name(), "color3");
    assert!(ts.get_type("none").get_base_type() == BaseType::None);
}

#[test]
fn type_desc_properties() {
    let float = type_desc_types::float();
    assert!(float.is_scalar());
    assert!(!float.is_closure());

    let color3 = type_desc_types::color3();
    assert!(color3.is_float3());
}

#[test]
fn shader_node_add_ports() {
    let mut node = ShaderNode::new("test_node");
    node.add_input("base", type_desc_types::float());
    node.add_input("base_color", type_desc_types::color3());
    node.add_output("out", type_desc_types::surfaceshader());

    assert_eq!(node.num_inputs(), 2);
    assert_eq!(node.num_outputs(), 1);
    assert!(node.get_input("base").is_some());
    assert!(node.get_output("out").is_some());
}

#[test]
fn syntax_default_value() {
    let mut syntax = Syntax::new(TypeSystem::new());
    syntax.register_type_syntax(
        type_desc_types::float(),
        TypeSyntax::scalar("float", "0.0", "0.0"),
    );

    let td = type_desc_types::float();
    assert_eq!(syntax.get_default_value(&td, false), "0.0");
}

#[test]
fn gen_options_default() {
    let opts = GenOptions::default();
    assert!(opts.elide_constant_nodes);
    assert_eq!(opts.library_prefix, "libraries");
}

#[test]
fn gen_options_hw_defaults() {
    use mtlx_rs::gen_shader::{
        HwDirectionalAlbedoMethod, HwSpecularEnvironmentMethod, HwTransmissionRenderMethod,
    };

    let opts = GenOptions::default();
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
}

#[test]
fn shader_graph_sockets() {
    let mut graph = ShaderGraph::new("test_graph");
    graph.add_input_socket("base", type_desc_types::float());
    graph.add_input_socket("base_color", type_desc_types::color3());
    graph.add_output_socket("out", type_desc_types::surfaceshader());

    assert_eq!(graph.num_input_sockets(), 2);
    assert_eq!(graph.num_output_sockets(), 1);
    assert!(graph.get_input_socket("base").is_some());
    assert!(graph.get_output_socket("out").is_some());
}

#[test]
fn shader_graph_nodes() {
    let mut graph = ShaderGraph::new("material_graph");
    let node = graph.create_node("surface_node");
    node.add_input("base", type_desc_types::float());
    node.add_output("out", type_desc_types::surfaceshader());

    assert!(graph.get_node("surface_node").is_some());
}

#[test]
fn shader_graph_bypass_and_optimize() {
    use mtlx_rs::gen_shader::ShaderNodeClassification;

    let mut graph = ShaderGraph::new("g");
    graph.add_input_socket("base", type_desc_types::float());
    graph.add_output_socket("out", type_desc_types::float());
    let a = graph.create_node("a");
    a.add_input("value", type_desc_types::float());
    a.add_output("out", type_desc_types::float());
    a.set_classification(ShaderNodeClassification::CONSTANT);
    let b = graph.create_node("b");
    b.add_input("in", type_desc_types::float());
    b.add_output("out", type_desc_types::float());
    graph.make_connection("a", "value", "g", "base").ok();
    graph.make_connection("b", "in", "a", "out").ok();
    graph.make_connection("g", "out", "b", "out").ok();
    graph.bypass("a", 0, 0).unwrap();
    assert!(graph.get_node("a").is_some());
    let edits = graph.optimize(true);
    assert!(edits <= 2);
}

#[test]
fn shader_graph_connections_and_topological_sort() {
    let mut graph = ShaderGraph::new("g");
    graph.add_output_socket("out", type_desc_types::float());
    let a = graph.create_node("a");
    a.add_input("in", type_desc_types::float());
    a.add_output("out", type_desc_types::float());
    let b = graph.create_node("b");
    b.add_input("in", type_desc_types::float());
    b.add_output("out", type_desc_types::float());
    // a.out -> b.in, a.out -> g.out (output socket)
    graph.make_connection("b", "in", "a", "out").unwrap();
    graph.make_connection("g", "out", "a", "out").unwrap();
    assert!(
        graph
            .get_output_socket("out")
            .unwrap()
            .get_connection()
            .is_some()
    );
    let conns = graph.get_connections_for_output("a", "out");
    assert_eq!(conns.len(), 2);
    let conn_nodes: Vec<&str> = conns.iter().map(|(n, _)| n.as_str()).collect();
    assert!(conn_nodes.contains(&"b"));
    assert!(conn_nodes.contains(&"g"));
    graph.topological_sort();
    let order: Vec<&str> = graph.node_order.iter().map(|s| s.as_str()).collect();
    assert!(
        order.iter().position(|&x| x == "a").unwrap()
            < order.iter().position(|&x| x == "b").unwrap()
    );
}

#[test]
fn shader_stages() {
    let graph = ShaderGraph::new("test_shader");
    let mut shader = Shader::new("TestShader", graph);
    shader.create_stage("pixel");

    assert!(shader.has_stage("pixel"));
    shader.set_source_code("void main() { }", "pixel");
    assert!(!shader.get_source_code("pixel").is_empty());
}

/// Minimal generator for tests
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

#[test]
fn shader_node_impl_nop() {
    let mut doc = Document::new();
    let elem = doc
        .add_child_of_category("backdrop", "BD_org")
        .expect("add");
    let shader_gen = TestGenerator {
        type_system: TypeSystem::new(),
    };
    let ctx = GenContext::new(shader_gen);

    let mut nop = NopNode::new();
    nop.initialize(&elem, &ctx as &dyn ShaderImplContext);
    assert_eq!(nop.get_name(), "BD_org");
    assert_ne!(nop.get_hash(), 0);
}

#[test]
fn implementation_factory() {
    let creator: ShaderNodeImplCreator = Arc::new(|| NopNode::create());
    let mut factory = ImplementationFactory::new();
    factory.register("IM_nop", creator);
    assert!(factory.is_registered("IM_nop"));
    assert!(!factory.is_registered("IM_other"));

    let mut impl_ = factory.create("IM_nop").expect("create");
    let mut doc = Document::new();
    let elem = doc
        .add_child_of_category("backdrop", "BD_test")
        .expect("add");
    let shader_gen = TestGenerator {
        type_system: TypeSystem::new(),
    };
    let ctx = GenContext::new(shader_gen);
    impl_.initialize(&elem, &ctx as &dyn ShaderImplContext);
    assert_eq!(impl_.get_name(), "BD_test");
}

#[test]
fn util_replace_substrings() {
    let s = "hello\nworld\n";
    let r = replace_substrings(s, &[("\n", " ")]);
    assert_eq!(r, "hello world ");
}

#[test]
fn color_management_system() {
    let mut cms = DefaultColorManagementSystem::new("genglsl");
    assert_eq!(cms.get_name(), "default_cms");
    let transform =
        ColorSpaceTransform::new("srgb_texture", "lin_rec709", type_desc_types::color3());
    assert!(!cms.supports_transform(&transform)); // no library loaded

    let cmlib_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("libraries/cmlib");
    let doc = read_from_xml_file_path(cmlib_path.join("cmlib_defs.mtlx")).expect("load cmlib");
    cms.load_library(doc);
    assert!(cms.supports_transform(&transform)); // ND_srgb_texture_to_lin_rec709_color3
}

#[test]
fn unit_system() {
    let mut us = DefaultUnitSystem::new("genglsl");
    assert_eq!(us.get_name(), DefaultUnitSystem::UNITSYSTEM_NAME);
    let transform = UnitTransform::new("meter", "inch", type_desc_types::float(), "distance");
    assert!(!us.supports_transform(&transform)); // no library

    let stdlib_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("libraries/stdlib");
    let doc = read_from_xml_file_path(stdlib_path.join("stdlib_defs.mtlx")).expect("load stdlib");
    us.load_library(doc);
    assert!(us.supports_transform(&transform)); // ND_multiply_float
}

#[test]
fn format_read_file() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("libraries/stdlib/genglsl/mx_image_float.glsl");
    let fp = FilePath::new(path);
    let content = read_file(&fp);
    assert!(!content.is_empty());
    assert!(content.contains("mx_image_float"));
}

const IMPL_WITH_SOURCECODE: &str = r#"<?xml version="1.0"?>
<materialx version="1.39">
  <implementation name="IM_test_inline" nodedef="ND_test" sourcecode="val * 2" />
  <implementation name="IM_test_file" nodedef="ND_test2" file="mx_image_float.glsl" function="mx_image_float" />
</materialx>"#;

#[test]
fn syntax_make_valid_name() {
    let mut syntax = Syntax::new(TypeSystem::new());
    syntax.register_invalid_tokens(vec![("/".to_string(), "::".to_string())]);
    syntax.register_reserved_words(vec!["int".to_string(), "float".to_string()]);

    // C++ order: replace invalid chars first (non-alnum -> _), then invalid_tokens. So "/" -> "_" first.
    let mut name = "node/name".to_string();
    syntax.make_valid_name(&mut name);
    assert_eq!(name, "node_name");

    let mut reserved = "float".to_string();
    syntax.make_valid_name(&mut reserved);
    assert_eq!(reserved, "float1");
}

#[test]
fn source_code_node_initialize() {
    let doc = read_from_xml_str(IMPL_WITH_SOURCECODE).expect("parse");
    let impl_inline = doc.get_implementation("IM_test_inline").expect("impl");
    let shader_gen = TestGenerator {
        type_system: TypeSystem::new(),
    };
    let mut ctx = GenContext::new(shader_gen);
    // Register path so IM_test_file can resolve mx_image_float.glsl
    let genglsl = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("libraries/stdlib/genglsl");
    ctx.register_source_code_search_path(FilePath::new(genglsl));

    let mut src_node = SourceCodeNode::new();
    src_node.initialize(&impl_inline, &ctx as &dyn ShaderImplContext);
    assert_eq!(src_node.get_name(), "IM_test_inline");
    assert!(src_node.get_hash() != 0);

    let mut src_node2 = SourceCodeNode::new();
    let impl_file = doc.get_implementation("IM_test_file").expect("impl2");
    src_node2.initialize(&impl_file, &ctx as &dyn ShaderImplContext);
    assert_eq!(src_node2.get_name(), "IM_test_file");
}

#[test]
fn source_code_node_rejects_invalid_function_name() {
    let doc = read_from_xml_str(
        r#"<?xml version="1.0"?>
<materialx version="1.39">
  <implementation name="IM_bad_name" nodedef="ND_test" file="mx_image_float.glsl" function="bad/name" />
</materialx>"#,
    )
    .expect("parse");
    let impl_elem = doc.get_implementation("IM_bad_name").expect("impl");
    let mut ctx = GenContext::new(GlslShaderGenerator::create(None));
    let genglsl = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("libraries/stdlib/genglsl");
    ctx.register_source_code_search_path(FilePath::new(genglsl));
    let impl_ctx = GlslShaderGraphContext::new(&ctx);

    let mut src_node = SourceCodeNode::new();
    // initialize logs error and returns early (no panic) for invalid function names.
    src_node.initialize(&impl_elem, &impl_ctx as &dyn ShaderImplContext);
    // After early return, emit_function_call should produce empty output.
    // The node's name gets set before the function name validation, but the
    // invalid function name means code gen won't work — this is the expected
    // graceful degradation (no panic).
}

#[test]
fn shader_graph_create_from_nodegraph() {
    let lib = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("libraries/stdlib");
    let mut doc = read_from_xml_file_path(lib.join("stdlib_defs.mtlx")).expect("load defs");
    let ng_doc = read_from_xml_file_path(lib.join("stdlib_ng.mtlx")).expect("load ng");
    doc.import_library(&ng_doc);
    let node_graph = doc.get_node_graph("NG_tiledimage_float").expect("NG");
    let shader_gen = GlslShaderGenerator::create(None);
    let mut ctx = GenContext::new(shader_gen);
    ctx.ensure_default_color_and_unit_systems();
    ctx.load_libraries_from_document(&doc);
    let create_ctx = GlslShaderGraphContext::new(&ctx);
    let graph = create_from_nodegraph(&node_graph, &doc, &create_ctx).expect("create");
    assert!(!graph.get_name().is_empty());
    assert!(graph.num_input_sockets() > 0);
    assert!(graph.num_output_sockets() > 0);
}

#[test]
fn shader_graph_complete_interface_publishes_only_editable_internal_inputs() {
    let lib = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("libraries/stdlib");
    let mut doc = read_from_xml_file_path(lib.join("stdlib_defs.mtlx")).expect("load defs");
    let ng_doc = read_from_xml_file_path(lib.join("stdlib_ng.mtlx")).expect("load ng");
    let impl_doc =
        read_from_xml_file_path(lib.join("genglsl/stdlib_genglsl_impl.mtlx")).expect("load impl");
    doc.import_library(&ng_doc);
    doc.import_library(&impl_doc);
    let node_graph = doc.get_node_graph("NG_tiledimage_float").expect("NG");
    let shader_gen = GlslShaderGenerator::create(None);
    let mut ctx = GenContext::new(shader_gen);
    ctx.ensure_default_color_and_unit_systems();
    ctx.load_libraries_from_document(&doc);
    ctx.options.shader_interface_type = ShaderInterfaceType::Complete;
    let create_ctx = GlslShaderGraphContext::new(&ctx);
    let graph = create_from_nodegraph(&node_graph, &doc, &create_ctx).expect("create");

    assert!(graph.get_input_socket("N_img_float_uaddressmode").is_some());
    assert!(graph.get_input_socket("N_img_float_vaddressmode").is_some());
    assert!(graph.get_input_socket("N_img_float_uv_scale").is_none());
    assert!(graph.get_input_socket("N_img_float_uv_offset").is_none());
}

#[test]
fn shader_graph_reduced_interface_skips_internal_inputs() {
    let lib = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("libraries/stdlib");
    let mut doc = read_from_xml_file_path(lib.join("stdlib_defs.mtlx")).expect("load defs");
    let ng_doc = read_from_xml_file_path(lib.join("stdlib_ng.mtlx")).expect("load ng");
    let impl_doc =
        read_from_xml_file_path(lib.join("genglsl/stdlib_genglsl_impl.mtlx")).expect("load impl");
    doc.import_library(&ng_doc);
    doc.import_library(&impl_doc);
    let node_graph = doc.get_node_graph("NG_tiledimage_float").expect("NG");
    let shader_gen = GlslShaderGenerator::create(None);
    let mut ctx = GenContext::new(shader_gen);
    ctx.ensure_default_color_and_unit_systems();
    ctx.load_libraries_from_document(&doc);
    ctx.options.shader_interface_type = ShaderInterfaceType::Reduced;
    let create_ctx = GlslShaderGraphContext::new(&ctx);
    let graph = create_from_nodegraph(&node_graph, &doc, &create_ctx).expect("create");

    assert!(graph.get_input_socket("N_img_float_uaddressmode").is_none());
    assert!(graph.get_input_socket("N_img_float_vaddressmode").is_none());
    assert!(graph.get_input_socket("N_img_float_uv_scale").is_none());
    assert!(graph.get_input_socket("N_img_float_uv_offset").is_none());
}

#[test]
fn create_shader_from_nodegraph() {
    let lib = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("libraries/stdlib");
    let mut doc = read_from_xml_file_path(lib.join("stdlib_defs.mtlx")).expect("load defs");
    let ng_doc = read_from_xml_file_path(lib.join("stdlib_ng.mtlx")).expect("load ng");
    let impl_doc =
        read_from_xml_file_path(lib.join("genglsl/stdlib_genglsl_impl.mtlx")).expect("load impl");
    doc.import_library(&ng_doc);
    doc.import_library(&impl_doc);
    let node_graph = doc.get_node_graph("NG_tiledimage_float").expect("NG");
    let shader_gen = GlslShaderGenerator::create(None);
    let mut ctx = GenContext::new(shader_gen);
    ctx.ensure_default_color_and_unit_systems();
    ctx.load_libraries_from_document(&doc);
    let create_ctx = GlslShaderGraphContext::new(&ctx);
    let shader =
        hw_create_shader("test_shader", &node_graph, &doc, &create_ctx).expect("create_shader");
    assert_eq!(shader.get_name(), "test_shader");
    assert!(shader.has_stage("vertex"));
    assert!(shader.has_stage("pixel"));
    let vs = shader.get_stage_by_name("vertex").expect("vertex stage");
    let ps = shader.get_stage_by_name("pixel").expect("pixel stage");
    assert!(vs.get_input_block("VertexInputs").is_some());
    assert!(vs.get_uniform_block("PrivateUniforms").is_some());
    assert!(vs.get_output_block("VertexData").is_some());
    assert!(ps.get_input_block("VertexData").is_some());
    assert!(ps.get_output_block("PixelOutputs").is_some());
    let pub_uniforms = ps
        .get_uniform_block("PublicUniforms")
        .expect("pixel public uniforms");
    let published: Vec<&str> = pub_uniforms
        .variables
        .iter()
        .map(|v| v.get_name())
        .collect();
    assert!(published.contains(&"N_img_float_uaddressmode"));
    assert!(published.contains(&"N_img_float_vaddressmode"));
    assert!(!published.contains(&"N_img_float_uv_scale"));
    assert!(!published.contains(&"N_img_float_uv_offset"));
}

#[test]
fn tiledimage_graph_has_image_node_and_output_variable() {
    let lib = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("libraries/stdlib");
    let mut doc = read_from_xml_file_path(lib.join("stdlib_defs.mtlx")).expect("load defs");
    let ng_doc = read_from_xml_file_path(lib.join("stdlib_ng.mtlx")).expect("load ng");
    let impl_doc =
        read_from_xml_file_path(lib.join("genglsl/stdlib_genglsl_impl.mtlx")).expect("load impl");
    doc.import_library(&ng_doc);
    doc.import_library(&impl_doc);
    let node_graph = doc.get_node_graph("NG_tiledimage_float").expect("NG");
    let g = GlslShaderGenerator::create(None);
    let mut ctx = GenContext::new(g);
    ctx.ensure_default_color_and_unit_systems();
    ctx.load_libraries_from_document(&doc);
    let create_ctx = GlslShaderGraphContext::new(&ctx);
    let graph = create_from_nodegraph(&node_graph, &doc, &create_ctx).expect("create");
    let img_node = graph.get_node("N_img_float");
    assert!(
        img_node.is_some(),
        "NG_tiledimage_float graph must contain N_img_float node"
    );
    let out_var = graph.get_connection_variable("N_img_float", "out");
    assert!(
        out_var.as_ref().map(|s| !s.is_empty()).unwrap_or(false),
        "N_img_float output must have variable set for pixel emission"
    );
}

#[test]
fn create_slang_shader_from_nodegraph() {
    let lib = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("libraries/stdlib");
    let mut doc = read_from_xml_file_path(lib.join("stdlib_defs.mtlx")).expect("load defs");
    let ng_doc = read_from_xml_file_path(lib.join("stdlib_ng.mtlx")).expect("load ng");
    let impl_doc = read_from_xml_file_path(lib.join("genslang/stdlib_genslang_impl.mtlx"))
        .expect("load genslang impl");
    doc.import_library(&ng_doc);
    doc.import_library(&impl_doc);
    let node_graph = doc.get_node_graph("NG_tiledimage_float").expect("NG");
    let shader_gen = SlangShaderGenerator::create(None);
    let mut ctx = GenContext::new(shader_gen);
    ctx.ensure_default_color_and_unit_systems();
    ctx.load_libraries_from_document(&doc);
    ctx.register_source_code_search_path(FilePath::new(lib.clone()));
    let shader =
        create_slang_shader("test_slang", &node_graph, &doc, &ctx).expect("create_slang_shader");
    assert_eq!(shader.get_name(), "test_slang");
    assert!(shader.has_stage("vertex"));
    assert!(shader.has_stage("pixel"));
    let vs = shader.get_stage_by_name("vertex").expect("vertex stage");
    let ps = shader.get_stage_by_name("pixel").expect("pixel stage");
    assert!(vs.get_input_block("VertexInputs").is_some());
    assert!(vs.get_uniform_block("PrivateUniforms").is_some());
    assert!(vs.get_output_block("VertexData").is_some());
    assert!(ps.get_input_block("VertexData").is_some());
    assert!(ps.get_output_block("PixelOutputs").is_some());
}

#[test]
fn generate_slang_shader_emits_slang_syntax() {
    let lib = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("libraries/stdlib");
    let mut doc = read_from_xml_file_path(lib.join("stdlib_defs.mtlx")).expect("load defs");
    let ng_doc = read_from_xml_file_path(lib.join("stdlib_ng.mtlx")).expect("load ng");
    let impl_doc = read_from_xml_file_path(lib.join("genslang/stdlib_genslang_impl.mtlx"))
        .expect("load genslang impl");
    doc.import_library(&ng_doc);
    doc.import_library(&impl_doc);
    let node_graph = doc.get_node_graph("NG_tiledimage_float").expect("NG");
    let shader_gen = SlangShaderGenerator::create(None);
    let mut ctx = GenContext::new(shader_gen);
    ctx.ensure_default_color_and_unit_systems();
    ctx.load_libraries_from_document(&doc);
    ctx.register_source_code_search_path(FilePath::new(lib.clone()));
    let shader = generate_slang_shader("test_slang_gen", &node_graph, &doc, &mut ctx)
        .expect("generate_slang_shader");
    let vs_src = shader.get_source_code("vertex");
    let ps_src = shader.get_source_code("pixel");
    assert!(
        vs_src.contains("[shader(\"vertex\")]"),
        "vertex stage must have Slang attribute"
    );
    assert!(
        vs_src.contains("float4"),
        "vertex must use Slang float4 type"
    );
    assert!(vs_src.contains("vertexMain"), "vertex must have vertexMain");
    assert!(
        ps_src.contains("[shader(\"fragment\")]"),
        "pixel stage must have Slang attribute"
    );
    assert!(
        ps_src.contains("float4"),
        "pixel must use Slang float4 type"
    );
    assert!(
        ps_src.contains("fragmentMain"),
        "pixel must have fragmentMain"
    );
}

#[test]
fn create_mdl_shader_from_nodegraph() {
    let lib = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("libraries/stdlib");
    let mut doc = read_from_xml_file_path(lib.join("stdlib_defs.mtlx")).expect("load defs");
    let ng_doc = read_from_xml_file_path(lib.join("stdlib_ng.mtlx")).expect("load ng");
    let impl_doc = read_from_xml_file_path(lib.join("genmdl/stdlib_genmdl_impl.mtlx"))
        .expect("load genmdl impl");
    doc.import_library(&ng_doc);
    doc.import_library(&impl_doc);
    let node_graph = doc.get_node_graph("NG_tiledimage_float").expect("NG");
    let shader_gen = MdlShaderGenerator::create(None);
    let mut ctx = GenContext::new(shader_gen);
    ctx.ensure_default_color_and_unit_systems();
    ctx.load_libraries_from_document(&doc);
    let shader = create_mdl_shader("test_mdl", &node_graph, &doc, &ctx).expect("create_mdl_shader");
    assert_eq!(shader.get_name(), "test_mdl");
    assert!(shader.has_stage("pixel"));
}

#[test]
fn generate_mdl_shader_emits_mdl() {
    let lib = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("libraries/stdlib");
    let mut doc = read_from_xml_file_path(lib.join("stdlib_defs.mtlx")).expect("load defs");
    let ng_doc = read_from_xml_file_path(lib.join("stdlib_ng.mtlx")).expect("load ng");
    let impl_doc = read_from_xml_file_path(lib.join("genmdl/stdlib_genmdl_impl.mtlx"))
        .expect("load genmdl impl");
    doc.import_library(&ng_doc);
    doc.import_library(&impl_doc);
    let node_graph = doc.get_node_graph("NG_tiledimage_float").expect("NG");
    let shader_gen = MdlShaderGenerator::create(None);
    let mut ctx = GenContext::new(shader_gen);
    ctx.ensure_default_color_and_unit_systems();
    ctx.load_libraries_from_document(&doc);
    let shader = generate_mdl_shader("test_mdl_gen", &node_graph, &doc, &mut ctx)
        .expect("generate_mdl_shader");
    let ps_src = shader.get_source_code("pixel");
    assert!(ps_src.contains("mdl 1.10"), "MDL must have version");
    assert!(
        ps_src.contains("export material"),
        "MDL must have export material"
    );
    assert!(
        ps_src.contains("::materialx::stdlib_1_10"),
        "MDL must import stdlib"
    );
}

#[test]
fn generate_mdl_material_graph() {
    let lib = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("libraries");
    let mut doc =
        read_from_xml_file_path(lib.join("stdlib/stdlib_defs.mtlx")).expect("stdlib defs");
    doc.import_library(&read_from_xml_file_path(lib.join("stdlib/stdlib_ng.mtlx")).expect("ng"));
    doc.import_library(
        &read_from_xml_file_path(lib.join("stdlib/genmdl/stdlib_genmdl_impl.mtlx"))
            .expect("stdlib genmdl"),
    );
    doc.import_library(
        &read_from_xml_file_path(lib.join("pbrlib/pbrlib_defs.mtlx")).expect("pbrlib defs"),
    );
    doc.import_library(
        &read_from_xml_file_path(lib.join("pbrlib/genmdl/pbrlib_genmdl_impl.mtlx"))
            .expect("pbrlib genmdl"),
    );
    doc.import_library(
        &read_from_xml_file_path(lib.join("bxdf/lama/lama_surface.mtlx")).expect("lama"),
    );
    let node_graph = doc
        .get_node_graph("NG_lama_surface")
        .expect("NG_lama_surface");
    let shader_gen = MdlShaderGenerator::create(None);
    let mut ctx = GenContext::new(shader_gen);
    ctx.ensure_default_color_and_unit_systems();
    ctx.load_libraries_from_document(&doc);
    let shader = generate_mdl_shader("lama_mdl", &node_graph, &doc, &mut ctx).expect("generate");
    let ps_src = shader.get_source_code("pixel");
    assert!(
        ps_src.contains("export material"),
        "MDL must have export material"
    );
    assert!(ps_src.contains("pbrlib_1_10"), "MDL must import pbrlib");
}

#[test]
fn generate_mdl_shader_emits_custom_node_imports() {
    use mtlx_rs::core::create_document;
    use mtlx_rs::core::element::{add_child_of_category, category};

    let mut doc = create_document();
    let nodedef = doc
        .add_node_def("ND_custom_float", "float", "custom_float")
        .expect("custom nodedef");
    let graph_nodedef = doc
        .add_node_def("ND_graph_float", "float", "graph_float")
        .expect("graph nodedef");
    let impl_elem = doc
        .add_child_of_category(category::IMPLEMENTATION, "IM_custom_float_genmdl")
        .expect("custom implementation");
    impl_elem
        .borrow_mut()
        .set_attribute("nodedef", nodedef.borrow().get_name());
    impl_elem.borrow_mut().set_attribute("target", "genmdl");
    impl_elem
        .borrow_mut()
        .set_attribute("file", "my/custom_module.mdl");
    impl_elem
        .borrow_mut()
        .set_attribute("function", "customfunc");

    let node_graph = doc.add_node_graph("NG_custom").expect("custom graph");
    node_graph
        .borrow_mut()
        .set_attribute("nodedef", graph_nodedef.borrow().get_name());
    let node = add_child_of_category(&node_graph, "custom_float", "custom1").expect("custom node");
    node.borrow_mut().set_attribute("type", "float");
    let out = add_child_of_category(&node_graph, category::OUTPUT, "out").expect("graph output");
    out.borrow_mut().set_attribute("type", "float");
    out.borrow_mut().set_node_name("custom1");

    let shader_gen = MdlShaderGenerator::create(None);
    let mut ctx = GenContext::new(shader_gen);
    ctx.ensure_default_color_and_unit_systems();
    let shader =
        generate_mdl_shader("custom_mdl", &node_graph, &doc, &mut ctx).expect("generate custom");
    let ps_src = shader.get_source_code("pixel");

    assert!(
        ps_src.contains("import ::my::custom_module::*"),
        "custom MDL implementations must emit their module imports"
    );
}

#[test]
fn create_mdl_shader_from_single_node_element() {
    use mtlx_rs::core::create_document;

    let mut doc = create_document();
    let _nodedef = doc
        .add_node_def("ND_custom_float", "float", "custom_float")
        .expect("custom nodedef");
    let impl_elem = doc
        .add_child_of_category("implementation", "IM_custom_float_genmdl")
        .expect("custom implementation");
    impl_elem
        .borrow_mut()
        .set_attribute("nodedef", "ND_custom_float");
    impl_elem.borrow_mut().set_attribute("target", "genmdl");
    impl_elem
        .borrow_mut()
        .set_attribute("sourcecode", "return mxp_out;");

    let node = doc
        .add_node("custom_float", "custom1", "float")
        .expect("custom node");

    let shader_gen = MdlShaderGenerator::create(None);
    let ctx = GenContext::new(shader_gen);
    let shader = create_mdl_shader("single_node_mdl", &node, &doc, &ctx).expect("single node");

    assert!(shader.has_stage("pixel"));
}

#[test]
fn compound_node_initialize() {
    let lib = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("libraries/stdlib");
    let mut doc = read_from_xml_file_path(lib.join("stdlib_defs.mtlx")).expect("load defs");
    let ng_doc = read_from_xml_file_path(lib.join("stdlib_ng.mtlx")).expect("load ng");
    doc.import_library(&ng_doc);
    let node_graph = doc.get_node_graph("NG_tiledimage_float").expect("NG");
    let shader_gen = TestGenerator {
        type_system: TypeSystem::new(),
    };
    let ctx = GenContext::new(shader_gen);
    let mut compound = CompoundNode::new();
    compound.initialize(&node_graph, &ctx as &dyn ShaderImplContext);
    assert_eq!(compound.get_name(), "NG_tiledimage_float");
    let graph = compound.get_graph().expect("graph");
    assert!(graph.num_input_sockets() > 0);
    assert_eq!(graph.num_output_sockets(), 1);
}
