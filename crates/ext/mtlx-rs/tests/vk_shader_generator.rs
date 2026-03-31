//! VkShaderGenerator tests — Vulkan GLSL (#version 450) output validation.

use std::path::Path;

use mtlx_rs::core::Document;
use mtlx_rs::format::read_from_xml_file_path;
use mtlx_rs::gen_glsl::{GlslResourceBindingContext, VK_TARGET, VK_VERSION, VkShaderGenerator};
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
fn vk_shader_generator_version_and_target() {
    let g = VkShaderGenerator::create(None);
    assert_eq!(g.get_target(), VK_TARGET);
    assert_eq!(g.get_version(), VK_VERSION);
}

#[test]
fn vk_generates_version_450() {
    let doc = load_stdlib_doc();
    let g = VkShaderGenerator::create(None);
    let mut ctx = GenContext::new(g);
    ctx.ensure_default_color_and_unit_systems();
    ctx.load_libraries_from_document(&doc);

    let node_graph = doc.get_node_graph("NG_tiledimage_float").expect("NG");
    let shader = ctx
        .get_shader_generator()
        .generate("vk_test", &node_graph, &ctx);

    let vs = shader.get_source_code("vertex");
    let ps = shader.get_source_code("pixel");

    assert!(
        vs.contains("#version 450"),
        "Vulkan vertex stage must declare #version 450"
    );
    assert!(
        ps.contains("#version 450"),
        "Vulkan pixel stage must declare #version 450"
    );
}

#[test]
fn vk_with_resource_binding_context_emits_layout() {
    let doc = load_stdlib_doc();
    let g = VkShaderGenerator::create(None);
    let mut ctx = GenContext::new(g);
    ctx.ensure_default_color_and_unit_systems();
    ctx.load_libraries_from_document(&doc);
    ctx.set_resource_binding_context(GlslResourceBindingContext::create_default());

    let node_graph = doc.get_node_graph("NG_tiledimage_float").expect("NG");
    let shader = ctx
        .get_shader_generator()
        .generate("vk_rbc", &node_graph, &ctx);

    let ps = shader.get_source_code("pixel");
    assert!(
        ps.contains("layout") || ps.contains("GL_ARB_shading_language_420pack"),
        "Vulkan with RBC should emit layout or extension"
    );
}
