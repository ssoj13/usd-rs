//! MslShaderGenerator tests — Metal Shading Language output validation.

use std::path::Path;

use mtlx_rs::core::Document;
use mtlx_rs::format::read_from_xml_file_path;
use mtlx_rs::gen_msl::{MSL_TARGET, MSL_VERSION, MslResourceBindingContext, MslShaderGenerator};
use mtlx_rs::gen_shader::GenContext;

fn stdlib_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("libraries/stdlib")
}

fn load_stdlib_doc_genmsl() -> Document {
    let lib = stdlib_path();
    let mut doc = read_from_xml_file_path(lib.join("stdlib_defs.mtlx")).expect("load defs");
    let ng = read_from_xml_file_path(lib.join("stdlib_ng.mtlx")).expect("load ng");
    let impl_doc = read_from_xml_file_path(lib.join("genmsl/stdlib_genmsl_impl.mtlx"))
        .expect("load genmsl impl");
    doc.import_library(&ng);
    doc.import_library(&impl_doc);
    doc
}

#[test]
fn msl_shader_generator_version_and_target() {
    let g = MslShaderGenerator::create(None);
    assert_eq!(g.get_target(), MSL_TARGET);
    assert_eq!(g.get_version(), MSL_VERSION);
}

#[test]
fn msl_generates_metal_stdlib() {
    let doc = load_stdlib_doc_genmsl();
    let g = MslShaderGenerator::create(None);
    let mut ctx = GenContext::new(g);
    ctx.ensure_default_color_and_unit_systems();
    ctx.load_libraries_from_document(&doc);

    let node_graph = doc.get_node_graph("NG_tiledimage_float").expect("NG");
    let shader = ctx
        .get_shader_generator()
        .generate("msl_test", &node_graph, &ctx);

    let vs = shader.get_source_code("vertex");
    let ps = shader.get_source_code("pixel");

    assert!(
        vs.contains("#include <metal_stdlib>"),
        "MSL vertex stage must include metal_stdlib"
    );
    assert!(
        ps.contains("#include <metal_stdlib>"),
        "MSL pixel stage must include metal_stdlib"
    );
    assert!(
        ps.contains("using namespace metal"),
        "MSL pixel stage must use metal namespace"
    );
}

#[test]
fn msl_with_resource_binding_context_emits_struct() {
    let doc = load_stdlib_doc_genmsl();
    let g = MslShaderGenerator::create(None);
    let mut ctx = GenContext::new(g);
    ctx.ensure_default_color_and_unit_systems();
    ctx.load_libraries_from_document(&doc);
    ctx.set_resource_binding_context(MslResourceBindingContext::create_default());

    let node_graph = doc.get_node_graph("NG_tiledimage_float").expect("NG");
    let shader = ctx
        .get_shader_generator()
        .generate("msl_rbc", &node_graph, &ctx);

    let ps = shader.get_source_code("pixel");
    assert!(
        ps.contains("struct ") && ps.contains("PublicUniforms"),
        "MSL with RBC should emit struct for uniform blocks"
    );
}

#[test]
fn msl_generates_fragment_main() {
    let doc = load_stdlib_doc_genmsl();
    let g = MslShaderGenerator::create(None);
    let mut ctx = GenContext::new(g);
    ctx.ensure_default_color_and_unit_systems();
    ctx.load_libraries_from_document(&doc);

    let node_graph = doc.get_node_graph("NG_tiledimage_float").expect("NG");
    let shader = ctx
        .get_shader_generator()
        .generate("msl_frag", &node_graph, &ctx);

    let ps = shader.get_source_code("pixel");

    // C++ reference pattern: `fragment {OutputsStruct} FragmentMain(...)`
    // Entry point returns the outputs struct directly (not float4), matching vertex stage pattern.
    assert!(
        ps.contains("fragment ") && ps.contains("FragmentMain("),
        "MSL pixel stage must have 'fragment <OutputsStruct> FragmentMain(' entry point, got:\n{}",
        &ps[ps.find("fragment ").unwrap_or(0)..]
            .chars()
            .take(120)
            .collect::<String>()
    );
    assert!(
        ps.contains("return ctx.FragmentMain();"),
        "MSL pixel stage must return ctx.FragmentMain() directly (not a member extraction)"
    );
    // Sanity: must NOT use the old hack of extracting a .member from the result
    assert!(
        !ps.contains("ctx.FragmentMain()."),
        "MSL pixel stage must NOT extract a member from ctx.FragmentMain() — return the struct directly"
    );
}
