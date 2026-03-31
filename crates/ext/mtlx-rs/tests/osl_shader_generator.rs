//! Tests for OSL shader generator (MaterialXGenOsl).

use mtlx_rs::core::Value;
use mtlx_rs::format::read_from_xml_file_path;
use mtlx_rs::gen_osl::{
    OslNetworkShaderGenerator, OslShaderGenerator, OslSyntax, TARGET, create_osl_network_shader,
    create_osl_shader, generate_osl_network, generate_osl_shader, register_osl_shader_metadata,
};
use mtlx_rs::gen_shader::GenContext;
use mtlx_rs::gen_shader::TypeSystem;

#[test]
fn osl_syntax_types() {
    let ts = TypeSystem::new();
    let syntax = OslSyntax::create(ts);
    let syn = syntax.get_syntax();

    assert_eq!(
        syn.get_type_name(&syn.type_system.get_type("float"))
            .unwrap(),
        "float"
    );
    assert_eq!(
        syn.get_type_name(&syn.type_system.get_type("color3"))
            .unwrap(),
        "color"
    );
    assert_eq!(
        syn.get_type_name(&syn.type_system.get_type("vector3"))
            .unwrap(),
        "vector"
    );
    assert_eq!(
        syn.get_type_name(&syn.type_system.get_type("floatarray"))
            .unwrap(),
        "float"
    );
    assert_eq!(
        syn.get_type_name(&syn.type_system.get_type("integerarray"))
            .unwrap(),
        "int"
    );
    assert_eq!(
        syn.get_type_name(&syn.type_system.get_type("BSDF"))
            .unwrap(),
        "BSDF"
    );
    assert_eq!(syntax.get_output_type_name("BSDF"), "output BSDF");

    assert_eq!(
        syn.get_default_value(&syn.type_system.get_type("float"), false),
        "0.0"
    );
    assert_eq!(
        syn.get_default_value(&syn.type_system.get_type("color3"), false),
        "color(0.0)"
    );
    assert_eq!(
        syn.get_default_value(&syn.type_system.get_type("color3"), true),
        "color(0.0)"
    );
    assert_eq!(
        syn.get_default_value(&syn.type_system.get_type("color4"), false),
        "color4(color(0.0), 0.0)"
    );
    assert_eq!(
        syn.get_default_value(&syn.type_system.get_type("color4"), true),
        "{color(0.0), 0.0}"
    );
    assert_eq!(
        syn.get_default_value(&syn.type_system.get_type("floatarray"), true),
        ""
    );
    assert_eq!(
        syn.get_default_value(&syn.type_system.get_type("integerarray"), true),
        ""
    );

    let fv = Value::Float(42.0);
    let fv_str = syn.get_value(&syn.type_system.get_type("float"), &fv, false);
    assert!(
        fv_str == "42.0" || fv_str == "42",
        "float value: {}",
        fv_str
    );
}

#[test]
fn osl_shader_generator_target() {
    let generator = OslShaderGenerator::create(None);
    assert_eq!(generator.get_target(), TARGET);
}

#[test]
fn osl_create_shader_from_tiledimage() {
    let lib = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("libraries/stdlib");
    let mut doc = read_from_xml_file_path(lib.join("stdlib_defs.mtlx")).expect("load defs");
    let ng_doc = read_from_xml_file_path(lib.join("stdlib_ng.mtlx")).expect("load ng");
    let genosl_impl = read_from_xml_file_path(lib.join("genosl/stdlib_genosl_impl.mtlx"))
        .expect("load genosl impl");
    doc.import_library(&ng_doc);
    doc.import_library(&genosl_impl);

    let node_graph = doc.get_node_graph("NG_tiledimage_float").expect("NG");
    let mut ctx = GenContext::new(OslShaderGenerator::create(None));
    ctx.ensure_default_color_and_unit_systems();

    let shader =
        create_osl_shader("tiledimage_float", &node_graph, &doc, &ctx).expect("create shader");
    assert_eq!(shader.get_name(), "tiledimage_float");
    assert_eq!(shader.num_stages(), 1);

    let stage = shader.get_stage_by_name("pixel").expect("pixel stage");
    assert!(stage.get_uniform_block("u").is_some());
    assert!(stage.get_output_block("o").is_some());
}

#[test]
fn osl_metadata_exported() {
    let lib = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("libraries/stdlib");
    let mut doc = read_from_xml_file_path(lib.join("stdlib_defs.mtlx")).expect("load defs");
    let ng_doc = read_from_xml_file_path(lib.join("stdlib_ng.mtlx")).expect("load ng");
    let genosl_impl = read_from_xml_file_path(lib.join("genosl/stdlib_genosl_impl.mtlx"))
        .expect("load genosl impl");
    doc.import_library(&ng_doc);
    doc.import_library(&genosl_impl);

    let node_graph = doc.get_node_graph("NG_tiledimage_float").expect("NG");
    let mut ctx = GenContext::new(OslShaderGenerator::create(None));
    ctx.ensure_default_color_and_unit_systems();
    register_osl_shader_metadata(&mut ctx);

    let shader =
        generate_osl_shader("tiledimage_float", &node_graph, &doc, &mut ctx).expect("generate");
    let src = shader.get_source_code("pixel");
    assert!(!src.is_empty());
    assert!(
        src.contains("string widget = ") && src.contains("[[ "),
        "OSL source should have metadata (widget, etc.): {}",
        &src[..src.len().min(1500)]
    );
}

#[test]
fn osl_generate_produces_source() {
    let lib = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("libraries/stdlib");
    let mut doc = read_from_xml_file_path(lib.join("stdlib_defs.mtlx")).expect("load defs");
    let ng_doc = read_from_xml_file_path(lib.join("stdlib_ng.mtlx")).expect("load ng");
    let genosl_impl = read_from_xml_file_path(lib.join("genosl/stdlib_genosl_impl.mtlx"))
        .expect("load genosl impl");
    doc.import_library(&ng_doc);
    doc.import_library(&genosl_impl);

    let node_graph = doc.get_node_graph("NG_tiledimage_float").expect("NG");
    let mut ctx = GenContext::new(OslShaderGenerator::create(None));
    ctx.ensure_default_color_and_unit_systems();

    // По рефу GenOsl: registerShaderMetadata before generate
    register_osl_shader_metadata(&mut ctx);

    let shader =
        generate_osl_shader("tiledimage_float", &node_graph, &doc, &mut ctx).expect("generate");
    assert_eq!(shader.get_name(), "tiledimage_float");

    let src = shader.get_source_code("pixel");
    assert!(!src.is_empty());
    assert!(src.contains("#include"), "OSL source should have includes");
    assert!(
        src.contains("surface ") || src.contains("shader "),
        "OSL source should declare shader type"
    );
    assert!(
        src.contains("tiledimage_float"),
        "OSL source should have shader name"
    );
}

#[test]
fn osl_network_generate_produces_param_connect_shader() {
    let lib = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("libraries/stdlib");
    let mut doc = read_from_xml_file_path(lib.join("stdlib_defs.mtlx")).expect("load defs");
    let ng_doc = read_from_xml_file_path(lib.join("stdlib_ng.mtlx")).expect("load ng");
    let genosl_impl = read_from_xml_file_path(lib.join("genosl/stdlib_genosl_impl.mtlx"))
        .expect("load genosl impl");
    doc.import_library(&ng_doc);
    doc.import_library(&genosl_impl);

    let node_graph = doc.get_node_graph("NG_tiledimage_float").expect("NG");
    let mut ctx = GenContext::new(OslNetworkShaderGenerator::create(None));
    ctx.ensure_default_color_and_unit_systems();

    let shader =
        generate_osl_network("tiledimage_float", &node_graph, &doc, &mut ctx).expect("generate");
    assert_eq!(shader.get_name(), "tiledimage_float");

    let src = shader.get_source_code("pixel");
    assert!(!src.is_empty());
    assert!(
        src.contains("shader "),
        "OSL network should have shader declarations"
    );
    assert!(
        src.contains("param ") || src.contains("connect "),
        "OSL network should have param or connect: {}",
        &src[..src.len().min(500)]
    );
}

#[test]
fn osl_network_create_shader() {
    let lib = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("libraries/stdlib");
    let mut doc = read_from_xml_file_path(lib.join("stdlib_defs.mtlx")).expect("load defs");
    let ng_doc = read_from_xml_file_path(lib.join("stdlib_ng.mtlx")).expect("load ng");
    let genosl_impl = read_from_xml_file_path(lib.join("genosl/stdlib_genosl_impl.mtlx"))
        .expect("load genosl impl");
    doc.import_library(&ng_doc);
    doc.import_library(&genosl_impl);

    let node_graph = doc.get_node_graph("NG_tiledimage_float").expect("NG");
    let mut ctx = GenContext::new(OslNetworkShaderGenerator::create(None));
    ctx.ensure_default_color_and_unit_systems();

    let shader =
        create_osl_network_shader("tiledimage_float", &node_graph, &doc, &ctx).expect("create");
    assert_eq!(shader.get_name(), "tiledimage_float");
    assert_eq!(shader.num_stages(), 1);
}
