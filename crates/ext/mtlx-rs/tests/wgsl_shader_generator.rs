//! WgslShaderGenerator tests — WGSL/WebGPU output validation.

use std::path::Path;

use mtlx_rs::core::Document;
use mtlx_rs::format::read_from_xml_file_path;
use mtlx_rs::gen_glsl::{
    WGSL_TARGET, WGSL_VERSION, WgslResourceBindingContext, WgslShaderGenerator,
};
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
fn wgsl_shader_generator_version_and_target() {
    let g = WgslShaderGenerator::create(None);
    assert_eq!(g.get_target(), WGSL_TARGET);
    assert_eq!(g.get_version(), WGSL_VERSION);
}

#[test]
fn wgsl_generates_version_450() {
    let doc = load_stdlib_doc();
    let g = WgslShaderGenerator::create(None);
    let mut ctx = GenContext::new(g);
    ctx.ensure_default_color_and_unit_systems();
    ctx.load_libraries_from_document(&doc);
    ctx.set_resource_binding_context(WgslResourceBindingContext::create_default());

    let node_graph = doc.get_node_graph("NG_tiledimage_float").expect("NG");
    let shader = ctx
        .get_shader_generator()
        .generate("wgsl_test", &node_graph, &ctx);

    let vs = shader.get_source_code("vertex");
    let ps = shader.get_source_code("pixel");

    assert!(
        vs.contains("#version 450"),
        "WGSL vertex stage must declare #version 450"
    );
    assert!(
        ps.contains("#version 450"),
        "WGSL pixel stage must declare #version 450"
    );
}

#[test]
fn wgsl_token_substitution_tex_sampler_split() {
    let doc = load_stdlib_doc();
    let g = WgslShaderGenerator::create(None);
    let mut ctx = GenContext::new(g);
    ctx.ensure_default_color_and_unit_systems();
    ctx.load_libraries_from_document(&doc);
    ctx.set_resource_binding_context(WgslResourceBindingContext::create_default());

    let node_graph = doc.get_node_graph("NG_tiledimage_float").expect("NG");
    let shader = ctx
        .get_shader_generator()
        .generate("wgsl_tokens", &node_graph, &ctx);

    let ps = shader.get_source_code("pixel");
    // WGSL token substitution: $texSamplerSignature -> texture2D tex_texture, sampler tex_sampler
    assert!(
        ps.contains("texture2D tex_texture") && ps.contains("sampler tex_sampler"),
        "WGSL must substitute texSamplerSignature with split texture+sampler"
    );
}

#[test]
fn wgsl_with_resource_binding_emits_texture_sampler_split() {
    let doc = load_stdlib_doc();
    let g = WgslShaderGenerator::create(None);
    let mut ctx = GenContext::new(g);
    ctx.ensure_default_color_and_unit_systems();
    ctx.load_libraries_from_document(&doc);
    ctx.set_resource_binding_context(WgslResourceBindingContext::create_default());

    let node_graph = doc.get_node_graph("NG_tiledimage_float").expect("NG");
    let shader = ctx
        .get_shader_generator()
        .generate("wgsl_split", &node_graph, &ctx);

    let ps = shader.get_source_code("pixel");
    assert!(
        ps.contains("_texture") && ps.contains("_sampler"),
        "WGSL with RBC should emit texture and sampler separately"
    );
}

// ===========================================================================
// NagaWgslShaderGenerator E2E tests (requires "wgsl-native" feature)
// ===========================================================================

#[cfg(feature = "wgsl-native")]
mod naga_e2e {
    use mtlx_rs::core::Document;
    use mtlx_rs::format::read_from_xml_file_path;
    use mtlx_rs::gen_shader::{GenContext, GenOptions, TypeSystem, shader_stage};
    use mtlx_rs::gen_wgsl::NagaWgslShaderGenerator;
    use std::path::Path;

    fn libraries_dir() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("libraries")
    }

    /// Load full MaterialX standard library (defs + nodegraphs + GLSL impls + bxdf + pbrlib).
    fn load_full_stdlib() -> Document {
        let lib = libraries_dir();
        let mut doc =
            read_from_xml_file_path(lib.join("stdlib/stdlib_defs.mtlx")).expect("stdlib_defs.mtlx");
        let ng =
            read_from_xml_file_path(lib.join("stdlib/stdlib_ng.mtlx")).expect("stdlib_ng.mtlx");
        let impl_doc = read_from_xml_file_path(lib.join("stdlib/genglsl/stdlib_genglsl_impl.mtlx"))
            .expect("stdlib_genglsl_impl.mtlx");
        doc.import_library(&ng);
        doc.import_library(&impl_doc);

        // Load pbrlib (contains standard_surface, gltf_pbr, etc.)
        if let Ok(pbr_defs) = read_from_xml_file_path(lib.join("pbrlib/pbrlib_defs.mtlx")) {
            doc.import_library(&pbr_defs);
        }
        if let Ok(pbr_ng) = read_from_xml_file_path(lib.join("pbrlib/pbrlib_ng.mtlx")) {
            doc.import_library(&pbr_ng);
        }
        if let Ok(pbr_impl) =
            read_from_xml_file_path(lib.join("pbrlib/genglsl/pbrlib_genglsl_impl.mtlx"))
        {
            doc.import_library(&pbr_impl);
        }

        // Load bxdf (standard_surface, gltf_pbr surface shaders)
        if let Ok(bxdf_defs) = read_from_xml_file_path(lib.join("bxdf/standard_surface.mtlx")) {
            doc.import_library(&bxdf_defs);
        }
        if let Ok(gltf) = read_from_xml_file_path(lib.join("bxdf/gltf_pbr.mtlx")) {
            doc.import_library(&gltf);
        }

        doc
    }

    /// Helper: generate WGSL from a nodegraph via NagaWgslShaderGenerator and validate both stages.
    fn assert_wgsl_from_nodegraph(ng_name: &str) {
        let doc = load_full_stdlib();
        let node_graph = doc
            .get_node_graph(ng_name)
            .unwrap_or_else(|| panic!("nodegraph '{}' not found in stdlib", ng_name));

        let generator = NagaWgslShaderGenerator::new(TypeSystem::new());
        let shader = generator.generate(
            &format!("test_{}", ng_name),
            &node_graph,
            &GenOptions::default(),
        );
        assert_wgsl_stages(&shader, ng_name);
    }

    /// Helper: generate WGSL for pbrlib nodegraphs that need full library loading.
    /// Uses WgslShaderGenerator + manual preprocess + naga transpile (same E2E pipeline).
    fn assert_wgsl_from_pbrlib_nodegraph(ng_name: &str) {
        use mtlx_rs::gen_glsl::{WgslResourceBindingContext, WgslShaderGenerator};
        use mtlx_rs::gen_wgsl::{ShaderStage, glsl_to_wgsl, preprocess_mtlx_glsl};

        let doc = load_full_stdlib();
        let node_graph = doc
            .get_node_graph(ng_name)
            .unwrap_or_else(|| panic!("nodegraph '{}' not found in stdlib", ng_name));

        let wgsl_gen = WgslShaderGenerator::new(TypeSystem::new());
        let mut ctx = GenContext::new(wgsl_gen);
        ctx.ensure_default_color_and_unit_systems();
        ctx.load_libraries_from_document(&doc);
        ctx.set_resource_binding_context(Box::new(WgslResourceBindingContext::new(0)));

        let mut shader =
            ctx.get_shader_generator()
                .generate(&format!("test_{}", ng_name), &node_graph, &ctx);

        // Transpile each stage GLSL -> WGSL via naga
        for stage in &mut shader.stages {
            let glsl = stage.get_source_code();
            if glsl.is_empty() {
                continue;
            }
            let naga_stage = if stage.name == shader_stage::VERTEX {
                ShaderStage::Vertex
            } else {
                ShaderStage::Fragment
            };
            let pp = preprocess_mtlx_glsl(glsl);
            match glsl_to_wgsl(&pp, naga_stage) {
                Ok(wgsl) => stage.set_source_code(wgsl),
                Err(e) => {
                    panic!(
                        "{}: naga transpile failed for stage '{}' ({} bytes): {:?}",
                        ng_name,
                        stage.name,
                        pp.len(),
                        e
                    );
                }
            }
        }
        assert_wgsl_stages(&shader, ng_name);
    }

    /// Validate that both stages contain valid WGSL (no GLSL artifacts).
    fn assert_wgsl_stages(shader: &mtlx_rs::gen_shader::Shader, ng_name: &str) {
        let ps = shader
            .get_stage_by_name(shader_stage::PIXEL)
            .expect("pixel stage");
        let ps_src = ps.get_source_code();
        assert!(
            !ps_src.contains("#version"),
            "{}: WGSL pixel should not contain #version",
            ng_name
        );
        assert!(
            !ps_src.contains("#define"),
            "{}: WGSL pixel should not contain #define",
            ng_name
        );
        assert!(
            ps_src.contains("fn main") || ps_src.contains("@fragment"),
            "{}: pixel stage should contain WGSL entry point, got {}B:\n{}",
            ng_name,
            ps_src.len(),
            &ps_src[..ps_src.len().min(300)]
        );

        let vs = shader
            .get_stage_by_name(shader_stage::VERTEX)
            .expect("vertex stage");
        let vs_src = vs.get_source_code();
        assert!(
            !vs_src.contains("#version"),
            "{}: WGSL vertex should not contain #version",
            ng_name
        );
        assert!(
            vs_src.contains("fn main") || vs_src.contains("@vertex"),
            "{}: vertex stage should contain WGSL entry point, got {}B:\n{}",
            ng_name,
            vs_src.len(),
            &vs_src[..vs_src.len().min(300)]
        );
    }

    #[test]
    fn naga_e2e_tiledimage_nodegraph_produces_wgsl() {
        assert_wgsl_from_nodegraph("NG_tiledimage_float");
    }

    #[test]
    fn naga_e2e_tiledimage_color3() {
        assert_wgsl_from_nodegraph("NG_tiledimage_color3");
    }

    #[test]
    fn naga_e2e_tiledimage_vector2() {
        assert_wgsl_from_nodegraph("NG_tiledimage_vector2");
    }

    #[test]
    fn naga_e2e_triplanarprojection_float() {
        assert_wgsl_from_nodegraph("NG_triplanarprojection_float");
    }

    #[test]
    fn naga_e2e_ramp4_color3() {
        assert_wgsl_from_nodegraph("NG_ramp4_color3");
    }

    // ── pbrlib / bxdf nodegraphs (closure types: BSDF/EDF/VDF) ─────────────

    #[test]
    fn naga_e2e_standard_surface() {
        assert_wgsl_from_pbrlib_nodegraph("NG_standard_surface_surfaceshader_100");
    }

    // ── stdlib: additional tiledimage variants ────────────────────────────────

    #[test]
    fn naga_e2e_tiledimage_color4() {
        assert_wgsl_from_nodegraph("NG_tiledimage_color4");
    }

    #[test]
    fn naga_e2e_tiledimage_vector3() {
        assert_wgsl_from_nodegraph("NG_tiledimage_vector3");
    }

    #[test]
    fn naga_e2e_tiledimage_vector4() {
        assert_wgsl_from_nodegraph("NG_tiledimage_vector4");
    }

    // ── stdlib: triplanarprojection variants ──────────────────────────────────

    #[test]
    fn naga_e2e_triplanarprojection_color3() {
        assert_wgsl_from_nodegraph("NG_triplanarprojection_color3");
    }

    #[test]
    fn naga_e2e_triplanarprojection_color4() {
        assert_wgsl_from_nodegraph("NG_triplanarprojection_color4");
    }

    #[test]
    fn naga_e2e_triplanarprojection_vector2() {
        assert_wgsl_from_nodegraph("NG_triplanarprojection_vector2");
    }

    #[test]
    fn naga_e2e_triplanarprojection_vector3() {
        assert_wgsl_from_nodegraph("NG_triplanarprojection_vector3");
    }

    #[test]
    fn naga_e2e_triplanarprojection_vector4() {
        assert_wgsl_from_nodegraph("NG_triplanarprojection_vector4");
    }

    // ── stdlib: ramp4 variants ────────────────────────────────────────────────

    #[test]
    fn naga_e2e_ramp4_float() {
        assert_wgsl_from_nodegraph("NG_ramp4_float");
    }

    #[test]
    fn naga_e2e_ramp4_color4() {
        assert_wgsl_from_nodegraph("NG_ramp4_color4");
    }

    #[test]
    fn naga_e2e_ramp4_vector2() {
        assert_wgsl_from_nodegraph("NG_ramp4_vector2");
    }

    #[test]
    fn naga_e2e_ramp4_vector3() {
        assert_wgsl_from_nodegraph("NG_ramp4_vector3");
    }

    #[test]
    fn naga_e2e_ramp4_vector4() {
        assert_wgsl_from_nodegraph("NG_ramp4_vector4");
    }

    // ── stdlib: noise nodegraphs ──────────────────────────────────────────────

    #[test]
    fn naga_e2e_noise2d_color3() {
        assert_wgsl_from_nodegraph("NG_noise2d_color3");
    }

    #[test]
    fn naga_e2e_noise3d_color3() {
        assert_wgsl_from_nodegraph("NG_noise3d_color3");
    }

    // ── stdlib: pattern nodegraphs ────────────────────────────────────────────

    #[test]
    fn naga_e2e_checkerboard_color3() {
        assert_wgsl_from_nodegraph("NG_checkerboard_color3");
    }

    // ── stdlib: math nodegraphs ───────────────────────────────────────────────

    #[test]
    fn naga_e2e_place2d_vector2() {
        assert_wgsl_from_nodegraph("NG_place2d_vector2");
    }

    #[test]
    fn naga_e2e_smoothstep_color3() {
        assert_wgsl_from_nodegraph("NG_smoothstep_color3");
    }
}
