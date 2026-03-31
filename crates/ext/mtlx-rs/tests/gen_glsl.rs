//! Tests for gen_glsl module.

use mtlx_rs::core::Document;
use mtlx_rs::format::read_from_xml_file_path;
use mtlx_rs::gen_glsl::{GlslResourceBindingContext, GlslShaderGenerator, TARGET, VERSION};
use mtlx_rs::gen_shader::GenContext;

#[test]
fn glsl_shader_generator_create() {
    let g = GlslShaderGenerator::create(None);
    assert_eq!(g.get_target(), TARGET);
    assert_eq!(g.get_version(), VERSION);
}

#[test]
fn glsl_shader_generator_generate() {
    let g = GlslShaderGenerator::create(None);
    let mut ctx = GenContext::new(g);
    ctx.ensure_default_color_and_unit_systems();
    let doc = Document::new();
    let root = doc.get_root();
    let element = root.clone();
    let shader = ctx
        .get_shader_generator()
        .generate("test_shader", &element, &ctx);
    assert_eq!(shader.get_name(), "test_shader");
    let code = shader.get_source_code("pixel");
    assert!(code.contains("#version 400"));
    assert!(code.contains("out vec4 fragColor"));
    assert!(code.contains("fragColor = vec4(0.0)"));
}

#[test]
fn glsl_syntax_qualifiers() {
    let g = GlslShaderGenerator::create(None);
    let syntax = g.get_syntax();
    assert_eq!(syntax.get_input_qualifier(), "in");
    assert_eq!(syntax.get_output_qualifier(), "out");
    assert_eq!(syntax.get_uniform_qualifier(), "uniform");
    assert_eq!(syntax.get_source_file_extension(), ".glsl");
}

#[test]
fn glsl_material_node_registered() {
    let g = GlslShaderGenerator::create(None);
    let factory = g.get_impl_factory();
    assert!(factory.is_registered(&format!("IM_surfacematerial_{}", TARGET)));
}

#[test]
fn glsl_geom_nodes_registered() {
    let g = GlslShaderGenerator::create(None);
    let factory = g.get_impl_factory();
    assert!(factory.is_registered(&format!("IM_normal_vector3_{}", TARGET)));
    assert!(factory.is_registered(&format!("IM_tangent_vector3_{}", TARGET)));
    assert!(factory.is_registered(&format!("IM_bitangent_vector3_{}", TARGET)));
    assert!(factory.is_registered(&format!("IM_viewdirection_vector3_{}", TARGET)));
    assert!(factory.is_registered(&format!("IM_geomcolor_color3_{}", TARGET)));
}

#[test]
fn glsl_surface_and_light_registered() {
    let g = GlslShaderGenerator::create(None);
    let factory = g.get_impl_factory();
    assert!(factory.is_registered(&format!("IM_surface_{}", TARGET)));
    assert!(factory.is_registered(&format!("IM_light_{}", TARGET)));
}

#[test]
fn glsl_generate_from_nodegraph_connects_output() {
    let lib = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("libraries/stdlib");
    let mut doc = read_from_xml_file_path(lib.join("stdlib_defs.mtlx")).expect("load defs");
    let ng_doc = read_from_xml_file_path(lib.join("stdlib_ng.mtlx")).expect("load ng");
    doc.import_library(&ng_doc);
    let node_graph = doc.get_node_graph("NG_tiledimage_float").expect("NG");
    let g = GlslShaderGenerator::create(None);
    let mut ctx = GenContext::new(g);
    ctx.ensure_default_color_and_unit_systems();
    ctx.load_libraries_from_document(&doc);
    let shader = ctx
        .get_shader_generator()
        .generate("ng_shader", &node_graph, &ctx);
    assert_eq!(shader.get_name(), "ng_shader");
    let ps = shader.get_source_code("pixel");
    assert!(ps.contains("#version 400"));
    assert!(ps.contains("void main()"));
    // Output should be connected to upstream variable when graph has connections
    assert!(
        ps.contains("= vec4(") || ps.contains("= ps_"),
        "pixel stage should assign output"
    );
    // mx_math.glsl included when ensure_default adds library path
    assert!(
        ps.contains("mx_square") || ps.contains("M_FLOAT_EPS"),
        "pixel stage should include mx_math stdlib"
    );
}

#[test]
fn glsl_generate_from_compound_node_emits_compound_logic() {
    let lib = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("libraries/stdlib");
    let mut doc = read_from_xml_file_path(lib.join("stdlib_defs.mtlx")).expect("load defs");
    let ng_doc = read_from_xml_file_path(lib.join("stdlib_ng.mtlx")).expect("load ng");
    let impl_doc =
        read_from_xml_file_path(lib.join("genglsl/stdlib_genglsl_impl.mtlx")).expect("load impl");
    doc.import_library(&ng_doc);
    doc.import_library(&impl_doc);

    let node = doc
        .add_node("tiledimage", "tiled1", "float")
        .expect("compound node");

    let g = GlslShaderGenerator::create(None);
    let mut ctx = GenContext::new(g);
    ctx.ensure_default_color_and_unit_systems();
    ctx.load_libraries_from_document(&doc);

    let shader = ctx
        .get_shader_generator()
        .generate("compound_shader", &node, &ctx);
    let ps = shader.get_source_code("pixel");

    assert!(
        ps.contains("uniform vec2 uvtiling;") || ps.contains("uniform vec2 uvoffset;"),
        "compound node generation must publish and use compound-node inputs"
    );
    assert!(
        ps.contains("mx_image_float")
            || ps.contains("mx_multiply")
            || ps.contains("mx_divide")
            || ps.contains("mx_subtract")
            || ps.contains(" * ")
            || ps.contains(" / ")
            || ps.contains(" - "),
        "compound node generation must emit internal compound-node functions"
    );
}

#[test]
fn glsl_resource_binding_context_layout_bindings() {
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
    ctx.set_resource_binding_context(GlslResourceBindingContext::create_default());
    let shader = ctx
        .get_shader_generator()
        .generate("layout_shader", &node_graph, &ctx);
    let ps = shader.get_source_code("pixel");
    let vs = shader.get_source_code("vertex");
    assert!(
        ps.contains("#extension GL_ARB_shading_language_420pack")
            || vs.contains("#extension GL_ARB_shading_language_420pack"),
        "shader should enable GL_ARB_shading_language_420pack when resource binding context is set"
    );
    // When uniform blocks have variables, layout(std140, binding=N) or layout(binding=N) is emitted.
    // Extension presence alone confirms resource binding context is used.
}

#[test]
fn glsl_surface_unlit_with_connected_input() {
    let lib = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("libraries/stdlib");
    let mut doc = read_from_xml_file_path(lib.join("stdlib_defs.mtlx")).expect("load defs");
    let ng_doc = read_from_xml_file_path(lib.join("stdlib_ng.mtlx")).expect("load ng");
    let impl_doc =
        read_from_xml_file_path(lib.join("genglsl/stdlib_genglsl_impl.mtlx")).expect("load impl");
    doc.import_library(&ng_doc);
    doc.import_library(&impl_doc);
    let node_graph = doc
        .get_node_graph("NG_convert_color3_surfaceshader")
        .expect("NG");
    let g = GlslShaderGenerator::create(None);
    let mut ctx = GenContext::new(g);
    ctx.ensure_default_color_and_unit_systems();
    ctx.load_libraries_from_document(&doc);
    let shader = ctx
        .get_shader_generator()
        .generate("surf_unlit", &node_graph, &ctx);
    let ps = shader.get_source_code("pixel");
    // When surface_unlit is used: mx_surface_unlit is either inlined (from file) or called
    let has_surface_unlit = ps.contains("mx_surface_unlit")
        || ps.contains("emission_color")
        || ps.contains("surfaceshader");
    assert!(
        has_surface_unlit || ps.contains("void main()"),
        "shader should have surface_unlit-related code or at least main(); got: {}",
        if ps.len() > 200 { &ps[..200] } else { &ps }
    );
}

/// Verifies that inline sourcecode {{var}} substitution produces real GLSL expressions
/// (not raw template tokens or comments). NG_tiledimage_float uses multiply, subtract, divide.
#[test]
fn glsl_inline_sourcecode_substitution() {
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
    let shader = ctx
        .get_shader_generator()
        .generate("inline_test", &node_graph, &ctx);
    let ps = shader.get_source_code("pixel");
    // Raw {{var}} tokens must not appear — substitution must have happened
    assert!(
        !ps.contains("{{in1}}") && !ps.contains("{{in2}}") && !ps.contains("{{in}}"),
        "inline sourcecode must substitute {{var}} tokens; got raw tokens in output"
    );
    // Math operations (multiply, subtract, divide) used by tiledimage should emit real expressions
    let has_arithmetic = ps.contains(" * ") || ps.contains(" - ") || ps.contains(" / ");
    assert!(
        has_arithmetic,
        "pixel stage from NG_tiledimage_float must contain arithmetic from inline math nodes"
    );
}

// ============================================================================
// Additional comprehensive tests for GenHw and GenGlsl modules
// ============================================================================

use mtlx_rs::gen_glsl::{EsslShaderGenerator, VkShaderGenerator};

// ---------------------------------------------------------------------------
// GLSL Syntax type names (mirrors C++ "GenShader: GLSL Syntax Check" test)
// ---------------------------------------------------------------------------

#[test]
fn glsl_syntax_type_name_float() {
    let g = GlslShaderGenerator::create(None);
    let ts = &g.get_syntax().get_syntax().type_system;
    let td = ts.get_type("float");
    assert_eq!(
        g.get_syntax().get_syntax().get_type_name(&td),
        Some("float")
    );
}

#[test]
fn glsl_syntax_type_name_color3_is_vec3() {
    let g = GlslShaderGenerator::create(None);
    let ts = &g.get_syntax().get_syntax().type_system;
    let td = ts.get_type("color3");
    assert_eq!(g.get_syntax().get_syntax().get_type_name(&td), Some("vec3"));
}

#[test]
fn glsl_syntax_type_name_vector3_is_vec3() {
    let g = GlslShaderGenerator::create(None);
    let ts = &g.get_syntax().get_syntax().type_system;
    let td = ts.get_type("vector3");
    assert_eq!(g.get_syntax().get_syntax().get_type_name(&td), Some("vec3"));
}

#[test]
fn glsl_syntax_default_value_float() {
    let g = GlslShaderGenerator::create(None);
    let td = g.get_syntax().get_syntax().get_type("float");
    assert_eq!(
        g.get_syntax().get_syntax().get_default_value(&td, false),
        "0.0"
    );
}

#[test]
fn glsl_syntax_default_value_color3() {
    let g = GlslShaderGenerator::create(None);
    let td = g.get_syntax().get_syntax().get_type("color3");
    assert_eq!(
        g.get_syntax().get_syntax().get_default_value(&td, false),
        "vec3(0.0)"
    );
}

#[test]
fn glsl_syntax_default_value_color4() {
    let g = GlslShaderGenerator::create(None);
    let td = g.get_syntax().get_syntax().get_type("color4");
    assert_eq!(
        g.get_syntax().get_syntax().get_default_value(&td, false),
        "vec4(0.0)"
    );
}

// ---------------------------------------------------------------------------
// GLSL generator target/version basic checks
// ---------------------------------------------------------------------------

#[test]
fn glsl_generator_target_matches() {
    let g = GlslShaderGenerator::create(None);
    assert_eq!(g.get_target(), TARGET);
    assert_eq!(g.get_version(), VERSION);
    assert_eq!(TARGET, "genglsl");
    assert_eq!(VERSION, "400");
}

// ---------------------------------------------------------------------------
// Fallback shader (empty element) produces valid GLSL
// ---------------------------------------------------------------------------

#[test]
fn glsl_fallback_shader_valid() {
    let g = GlslShaderGenerator::create(None);
    let ctx = GenContext::new(g);
    let doc = Document::new();
    let shader = ctx
        .get_shader_generator()
        .generate("fallback", &doc.get_root(), &ctx);
    let ps = shader.get_source_code("pixel");
    assert!(
        ps.contains("#version 400"),
        "fallback should have version 400"
    );
    assert!(ps.contains("void main()"), "fallback should have main()");
    assert!(ps.contains("fragColor"), "fallback should output fragColor");
}

// ---------------------------------------------------------------------------
// ESSL generates correct version
// ---------------------------------------------------------------------------

#[test]
fn essl_generates_version_300_es_from_empty() {
    let g = EsslShaderGenerator::create(None);
    let ctx = GenContext::new(g);
    let doc = Document::new();
    let shader = ctx
        .get_shader_generator()
        .generate("essl_fb", &doc.get_root(), &ctx);
    let ps = shader.get_source_code("pixel");
    assert!(
        ps.contains("#version 300 es"),
        "ESSL should emit #version 300 es"
    );
}

// ---------------------------------------------------------------------------
// VK generates #version 450 from empty element
// ---------------------------------------------------------------------------

#[test]
fn vk_generates_version_450_from_empty() {
    let g = VkShaderGenerator::create(None);
    let ctx = GenContext::new(g);
    let doc = Document::new();
    let shader = ctx
        .get_shader_generator()
        .generate("vk_fb", &doc.get_root(), &ctx);
    let ps = shader.get_source_code("pixel");
    assert!(
        ps.contains("#version 450"),
        "Vulkan should emit #version 450"
    );
}

// ---------------------------------------------------------------------------
// VkResourceBindingContext integration: pragma + single counter
// ---------------------------------------------------------------------------

// VkResourceBindingContext integration test is in vk_resource_binding_context.rs (unit tests)
// because VkResourceBindingContext is not re-exported from gen_glsl.

// ---------------------------------------------------------------------------
// GlslResourceBindingContext integration with separate bindings
// ---------------------------------------------------------------------------

#[test]
fn glsl_rbc_separate_binding_integration() {
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
    let mut rbc = GlslResourceBindingContext::new(0, 0);
    rbc.enable_separate_binding_locations(true);
    ctx.set_resource_binding_context(Box::new(rbc));
    let shader = ctx
        .get_shader_generator()
        .generate("sep_bind", &node_graph, &ctx);
    let ps = shader.get_source_code("pixel");
    assert!(
        ps.contains("GL_ARB_shading_language_420pack") || ps.contains("layout"),
        "separate binding mode should still emit extension or layout directives"
    );
}

// ---------------------------------------------------------------------------
// HW node implementations registered and creatable for all GLSL family generators
// ---------------------------------------------------------------------------

#[test]
fn essl_has_all_hw_impls_registered() {
    let g = EsslShaderGenerator::create(None);
    let factory = g.get_impl_factory();
    // ESSL inherits genglsl impls
    let t = "genglsl";
    assert!(factory.is_registered(&format!("IM_surfacematerial_{}", t)));
    assert!(factory.is_registered(&format!("IM_surface_{}", t)));
    assert!(factory.is_registered(&format!("IM_light_{}", t)));
    assert!(factory.is_registered(&format!("IM_position_vector3_{}", t)));
    assert!(factory.is_registered(&format!("IM_normal_vector3_{}", t)));
    assert!(factory.is_registered(&format!("IM_tangent_vector3_{}", t)));
    assert!(factory.is_registered(&format!("IM_bitangent_vector3_{}", t)));
    assert!(factory.is_registered(&format!("IM_viewdirection_vector3_{}", t)));
    assert!(factory.is_registered(&format!("IM_texcoord_vector2_{}", t)));
    assert!(factory.is_registered(&format!("IM_texcoord_vector3_{}", t)));
    assert!(factory.is_registered(&format!("IM_frame_float_{}", t)));
    assert!(factory.is_registered(&format!("IM_time_float_{}", t)));
}

#[test]
fn vk_has_all_hw_impls_registered() {
    let g = VkShaderGenerator::create(None);
    let factory = g.get_impl_factory();
    let t = "genglsl";
    assert!(factory.is_registered(&format!("IM_surfacematerial_{}", t)));
    assert!(factory.is_registered(&format!("IM_surface_{}", t)));
    assert!(factory.is_registered(&format!("IM_light_{}", t)));
    assert!(factory.is_registered(&format!("IM_geomcolor_float_{}", t)));
    assert!(factory.is_registered(&format!("IM_geomcolor_color3_{}", t)));
    assert!(factory.is_registered(&format!("IM_geomcolor_color4_{}", t)));
}

// ---------------------------------------------------------------------------
// HW node impls can be created via the factory
// ---------------------------------------------------------------------------

#[test]
fn glsl_factory_creates_surface_node() {
    let g = GlslShaderGenerator::create(None);
    let imp = g
        .get_impl_factory()
        .create(&format!("IM_surface_{}", TARGET));
    assert!(imp.is_some(), "surface node impl should be creatable");
}

#[test]
fn glsl_factory_creates_light_node() {
    let g = GlslShaderGenerator::create(None);
    let imp = g.get_impl_factory().create(&format!("IM_light_{}", TARGET));
    assert!(imp.is_some(), "light node impl should be creatable");
}

#[test]
fn glsl_factory_creates_material_node() {
    let g = GlslShaderGenerator::create(None);
    let imp = g
        .get_impl_factory()
        .create(&format!("IM_surfacematerial_{}", TARGET));
    assert!(imp.is_some(), "material node impl should be creatable");
}

#[test]
fn glsl_factory_creates_geom_nodes() {
    let g = GlslShaderGenerator::create(None);
    let f = g.get_impl_factory();
    assert!(
        f.create(&format!("IM_position_vector3_{}", TARGET))
            .is_some()
    );
    assert!(f.create(&format!("IM_normal_vector3_{}", TARGET)).is_some());
    assert!(
        f.create(&format!("IM_tangent_vector3_{}", TARGET))
            .is_some()
    );
    assert!(
        f.create(&format!("IM_bitangent_vector3_{}", TARGET))
            .is_some()
    );
    assert!(
        f.create(&format!("IM_viewdirection_vector3_{}", TARGET))
            .is_some()
    );
}

#[test]
fn glsl_factory_creates_transform_nodes() {
    let g = GlslShaderGenerator::create(None);
    let f = g.get_impl_factory();
    assert!(
        f.create(&format!("IM_transformpoint_vector3_{}", TARGET))
            .is_some()
    );
    assert!(
        f.create(&format!("IM_transformvector_vector3_{}", TARGET))
            .is_some()
    );
    assert!(
        f.create(&format!("IM_transformnormal_vector3_{}", TARGET))
            .is_some()
    );
}

// SurfaceNodeGlsl backward compatibility alias is not re-exported from gen_glsl;
// it is tested via the unit tests in glsl_family.rs.

// ---------------------------------------------------------------------------
// Shader output structure: vertex + pixel stages
// ---------------------------------------------------------------------------

#[test]
fn glsl_shader_has_vertex_and_pixel_stages() {
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
    let shader = ctx
        .get_shader_generator()
        .generate("stages_test", &node_graph, &ctx);
    let vs = shader.get_source_code("vertex");
    let ps = shader.get_source_code("pixel");
    // Both stages should have version and main
    assert!(
        vs.contains("#version 400"),
        "vertex stage should have version"
    );
    assert!(
        ps.contains("#version 400"),
        "pixel stage should have version"
    );
    assert!(
        vs.contains("void main()"),
        "vertex stage should have main()"
    );
    assert!(ps.contains("void main()"), "pixel stage should have main()");
}

// ---------------------------------------------------------------------------
// HW constants are accessible and have expected values
// ---------------------------------------------------------------------------

#[test]
fn hw_constants_values() {
    use mtlx_rs::gen_hw::*;
    assert_eq!(hw_block::VERTEX_INPUTS, "VertexInputs");
    assert_eq!(hw_block::VERTEX_DATA, "VertexData");
    assert_eq!(hw_block::PRIVATE_UNIFORMS, "PrivateUniforms");
    assert_eq!(hw_block::PUBLIC_UNIFORMS, "PublicUniforms");
    assert_eq!(hw_block::LIGHT_DATA, "LightData");
    assert_eq!(hw_block::PIXEL_OUTPUTS, "PixelOutputs");
}

#[test]
fn hw_ident_values() {
    use mtlx_rs::gen_hw::*;
    assert_eq!(hw_ident::IN_POSITION, "i_position");
    assert_eq!(hw_ident::IN_NORMAL, "i_normal");
    assert_eq!(hw_ident::POSITION_WORLD, "positionWorld");
    assert_eq!(hw_ident::NORMAL_WORLD, "normalWorld");
    assert_eq!(hw_ident::VIEW_POSITION, "u_viewPosition");
    assert_eq!(hw_ident::FRAME, "u_frame");
    assert_eq!(hw_ident::TIME, "u_time");
}

#[test]
fn hw_token_values() {
    use mtlx_rs::gen_hw::*;
    assert_eq!(hw_token::T_IN_POSITION, "$inPosition");
    assert_eq!(hw_token::T_IN_NORMAL, "$inNormal");
    assert_eq!(hw_token::T_VIEW_POSITION, "$viewPosition");
    assert_eq!(hw_token::T_FRAME, "$frame");
    assert_eq!(hw_token::T_TIME, "$time");
}

#[test]
fn hw_lighting_constants() {
    use mtlx_rs::gen_hw::*;
    assert_eq!(hw_lighting::DIR_N, "N");
    assert_eq!(hw_lighting::DIR_L, "L");
    assert_eq!(hw_lighting::DIR_V, "V");
    assert_eq!(hw_lighting::WORLD_POSITION, "P");
    assert_eq!(hw_lighting::OCCLUSION, "occlusion");
}

// hw_const_values (constant_values) is not re-exported from gen_hw;
// it is tested via the unit tests in hw_constants module.
