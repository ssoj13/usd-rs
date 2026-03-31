//! EsslShaderGenerator tests — ESSL (WebGL 2) output validation.

use std::path::Path;

use mtlx_rs::core::Document;
use mtlx_rs::format::read_from_xml_file_path;
use mtlx_rs::gen_glsl::{ESSL_TARGET, ESSL_VERSION, EsslShaderGenerator};
use mtlx_rs::gen_shader::GenContext;

fn stdlib_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("libraries/stdlib")
}

fn load_stdlib_doc() -> Document {
    let lib = stdlib_path();
    let mut doc = read_from_xml_file_path(lib.join("stdlib_defs.mtlx")).expect("load defs");
    let ng = read_from_xml_file_path(lib.join("stdlib_ng.mtlx")).expect("load ng");
    let impl_doc =
        read_from_xml_file_path(lib.join("genglsl/stdlib_genglsl_impl.mtlx")).expect("load impl");
    doc.import_library(&ng);
    doc.import_library(&impl_doc);
    doc
}

#[test]
fn essl_shader_generator_version_and_target() {
    let g = EsslShaderGenerator::create(None);
    assert_eq!(g.get_target(), ESSL_TARGET);
    assert_eq!(g.get_version(), ESSL_VERSION);
}

#[test]
fn essl_generates_version_300_es() {
    let doc = load_stdlib_doc();
    let g = EsslShaderGenerator::create(None);
    let mut ctx = GenContext::new(g);
    ctx.ensure_default_color_and_unit_systems();
    ctx.load_libraries_from_document(&doc);

    let node_graph = doc.get_node_graph("NG_tiledimage_float").expect("NG");
    let shader = ctx
        .get_shader_generator()
        .generate("essl_test", &node_graph, &ctx);

    let vs = shader.get_source_code("vertex");
    let ps = shader.get_source_code("pixel");

    assert!(
        vs.contains("#version 300 es"),
        "ESSL vertex stage must declare #version 300 es"
    );
    assert!(
        ps.contains("#version 300 es"),
        "ESSL pixel stage must declare #version 300 es"
    );
}
