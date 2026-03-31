//! GLSL comparison tests — полная валидация сгенерированного GLSL вывода.
//! Проверяет структуру vertex/pixel stages, обязательные паттерны, golden-файлы.

use std::path::Path;

use mtlx_rs::core::Document;
use mtlx_rs::format::read_from_xml_file_path;
use mtlx_rs::gen_glsl::{GlslResourceBindingContext, GlslShaderGenerator, TARGET, VERSION};
use mtlx_rs::gen_shader::GenContext;

/// Путь к библиотекам stdlib
fn stdlib_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("libraries/stdlib")
}

/// Загрузить doc с stdlib_defs, stdlib_ng, stdlib_genglsl_impl
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

/// Создать GenContext с настроенным генератором
fn create_context() -> GenContext<GlslShaderGenerator> {
    let g = GlslShaderGenerator::create(None);
    let mut ctx = GenContext::new(g);
    ctx.ensure_default_color_and_unit_systems();
    ctx
}

/// Нормализовать строку для сравнения (CRLF -> LF, trim trailing whitespace per line)
fn normalize_for_comparison(s: &str) -> String {
    let normalized: String = s
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .lines()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n");
    if normalized.is_empty() {
        normalized
    } else if s.ends_with('\n') || s.ends_with("\r\n") {
        normalized + "\n"
    } else {
        normalized
    }
}

#[test]
fn glsl_comparison_vertex_stage_required_elements() {
    let doc = load_stdlib_doc();
    let mut ctx = create_context();
    ctx.load_libraries_from_document(&doc);
    let node_graph = doc.get_node_graph("NG_tiledimage_float").expect("NG");
    let shader = ctx
        .get_shader_generator()
        .generate("vs_test", &node_graph, &ctx);

    let vs = shader.get_source_code("vertex");
    assert!(!vs.is_empty(), "Vertex stage must not be empty");

    assert!(
        vs.contains("#version 400"),
        "Vertex stage must declare #version 400"
    );
    assert!(
        vs.contains("u_worldMatrix") || vs.contains("$worldMatrix"),
        "Vertex stage must have world matrix uniform"
    );
    assert!(
        vs.contains("u_viewProjectionMatrix") || vs.contains("$viewProjectionMatrix"),
        "Vertex stage must have view-projection matrix uniform"
    );
    assert!(
        vs.contains("i_position") || vs.contains("$inPosition"),
        "Vertex stage must have position input"
    );
    // VertexData output block is emitted when non-empty (graphs using geom interpolation)
    assert!(
        vs.contains("mx_square") || vs.contains("M_FLOAT_EPS") || vs.contains("mx_mod"),
        "Vertex stage must include mx_math stdlib"
    );
    assert!(vs.contains("void main()"), "Vertex stage must have main()");
    assert!(
        vs.contains("gl_Position"),
        "Vertex stage must assign gl_Position"
    );
}

#[test]
fn glsl_comparison_pixel_stage_required_elements() {
    let doc = load_stdlib_doc();
    let mut ctx = create_context();
    ctx.load_libraries_from_document(&doc);
    let node_graph = doc.get_node_graph("NG_tiledimage_float").expect("NG");
    let shader = ctx
        .get_shader_generator()
        .generate("ps_test", &node_graph, &ctx);

    let ps = shader.get_source_code("pixel");
    assert!(!ps.is_empty(), "Pixel stage must not be empty");

    assert!(
        ps.contains("#version 400"),
        "Pixel stage must declare #version 400"
    );
    // VertexData input is present when vertex stage emits it (geom-interpolated data)
    assert!(ps.contains("void main()"), "Pixel stage must have main()");
    assert!(
        ps.contains("mx_square") || ps.contains("M_FLOAT_EPS") || ps.contains("mx_mod"),
        "Pixel stage must include mx_math stdlib"
    );
}

#[test]
fn glsl_comparison_tiledimage_float_full() {
    let doc = load_stdlib_doc();
    let mut ctx = create_context();
    ctx.load_libraries_from_document(&doc);
    let node_graph = doc.get_node_graph("NG_tiledimage_float").expect("NG");
    let shader = ctx
        .get_shader_generator()
        .generate("tiledimage", &node_graph, &ctx);

    let vs = shader.get_source_code("vertex");
    let ps = shader.get_source_code("pixel");

    assert_eq!(shader.get_name(), "tiledimage");
    assert!(shader.has_stage("vertex"));
    assert!(shader.has_stage("pixel"));

    // Vertex: transforms and gl_Position
    assert!(vs.contains("vec4"));
    assert!(vs.contains("mat4"));

    // Pixel: image lookup or graph output
    assert!(
        ps.contains("mx_image_float")
            || ps.contains("texture")
            || ps.contains("NG_tiledimage_float"),
        "Pixel stage should have image sampling or graph output"
    );
    // Output assignment
    assert!(ps.contains("= vec4(") || ps.contains("= ps_") || ps.contains("= o_"));
}

#[test]
fn glsl_comparison_convert_color3_surfaceshader_full() {
    let doc = load_stdlib_doc();
    let mut ctx = create_context();
    ctx.load_libraries_from_document(&doc);
    let node_graph = doc
        .get_node_graph("NG_convert_color3_surfaceshader")
        .expect("NG");
    let shader = ctx
        .get_shader_generator()
        .generate("surf_color3", &node_graph, &ctx);

    let ps = shader.get_source_code("pixel");

    assert!(shader.has_stage("pixel"));

    assert!(
        ps.contains("mx_surface_unlit")
            || ps.contains("emission_color")
            || ps.contains("surfaceshader"),
        "Surface shader must have mx_surface_unlit or emission"
    );
}

#[test]
fn glsl_comparison_noise2d_color3_full() {
    let doc = load_stdlib_doc();
    let mut ctx = create_context();
    ctx.load_libraries_from_document(&doc);
    let node_graph = doc.get_node_graph("NG_noise2d_color3").expect("NG");
    let shader = ctx
        .get_shader_generator()
        .generate("noise2d", &node_graph, &ctx);

    let vs = shader.get_source_code("vertex");
    let ps = shader.get_source_code("pixel");

    assert!(!vs.is_empty());
    assert!(!ps.is_empty());

    assert!(
        ps.contains("mx_noise2d") || ps.contains("noise") || ps.contains("NG_noise2d"),
        "Noise graph should emit noise-related code"
    );
}

#[test]
fn glsl_comparison_token_substitution_vertex() {
    let doc = load_stdlib_doc();
    let mut ctx = create_context();
    ctx.load_libraries_from_document(&doc);
    let node_graph = doc.get_node_graph("NG_tiledimage_float").expect("NG");
    let shader = ctx
        .get_shader_generator()
        .generate("tokens", &node_graph, &ctx);

    let vs = shader.get_source_code("vertex");

    // Токены должны быть заменены на идентификаторы (не $)
    assert!(
        !vs.contains("$inPosition"),
        "Token $inPosition should be substituted"
    );
    assert!(
        vs.contains("i_position"),
        "Should use i_position identifier"
    );
    assert!(
        !vs.contains("$worldMatrix"),
        "Token $worldMatrix should be substituted"
    );
    assert!(
        vs.contains("u_worldMatrix"),
        "Should use u_worldMatrix identifier"
    );
}

#[test]
fn glsl_comparison_token_substitution_pixel() {
    let doc = load_stdlib_doc();
    let mut ctx = create_context();
    ctx.load_libraries_from_document(&doc);
    let node_graph = doc.get_node_graph("NG_tiledimage_float").expect("NG");
    let shader = ctx
        .get_shader_generator()
        .generate("tokens_ps", &node_graph, &ctx);

    let ps = shader.get_source_code("pixel");

    assert!(
        !ps.contains("$vd"),
        "Token $vd should be substituted in pixel"
    );
}

#[test]
fn glsl_comparison_resource_binding_context_output() {
    let doc = load_stdlib_doc();
    let mut ctx = create_context();
    ctx.load_libraries_from_document(&doc);
    ctx.set_resource_binding_context(GlslResourceBindingContext::create_default());
    let node_graph = doc.get_node_graph("NG_tiledimage_float").expect("NG");
    let shader = ctx
        .get_shader_generator()
        .generate("rbc_shader", &node_graph, &ctx);

    let vs = shader.get_source_code("vertex");
    let ps = shader.get_source_code("pixel");

    assert!(
        vs.contains("#extension GL_ARB_shading_language_420pack")
            || ps.contains("#extension GL_ARB_shading_language_420pack"),
        "Resource binding context must emit GL_ARB_shading_language_420pack"
    );
}

#[test]
fn glsl_comparison_version_and_target() {
    let g = GlslShaderGenerator::create(None);
    assert_eq!(g.get_version(), VERSION);
    assert_eq!(g.get_version(), "400");
    assert_eq!(g.get_target(), TARGET);
    assert_eq!(g.get_target(), "genglsl");
}

#[test]
fn glsl_comparison_multiple_graphs_deterministic() {
    let doc = load_stdlib_doc();
    let mut ctx = create_context();
    ctx.load_libraries_from_document(&doc);

    let ng1 = doc.get_node_graph("NG_tiledimage_float").expect("NG1");
    let shader1 = ctx.get_shader_generator().generate("a", &ng1, &ctx);

    let ng2 = doc.get_node_graph("NG_tiledimage_float").expect("NG2");
    let mut ctx2 = create_context();
    ctx2.load_libraries_from_document(&doc);
    let shader2 = ctx2.get_shader_generator().generate("a", &ng2, &ctx2);

    let ps1 = shader1.get_source_code("pixel");
    let ps2 = shader2.get_source_code("pixel");

    assert_eq!(ps1, ps2, "Same graph should produce identical output");
}

#[test]
fn glsl_comparison_golden_vertex_tiledimage() {
    let doc = load_stdlib_doc();
    let mut ctx = create_context();
    ctx.load_libraries_from_document(&doc);
    let node_graph = doc.get_node_graph("NG_tiledimage_float").expect("NG");
    let shader = ctx
        .get_shader_generator()
        .generate("golden_tiledimage", &node_graph, &ctx);

    let generated = normalize_for_comparison(shader.get_source_code("vertex"));
    let fixtures_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/glsl_expected");
    let golden_path = fixtures_dir.join("tiledimage_float_vertex.glsl");

    if std::env::var("GLSL_UPDATE_GOLDEN").is_ok() {
        std::fs::create_dir_all(&fixtures_dir).expect("create fixtures dir");
        std::fs::write(&golden_path, &generated).expect("write golden");
        return;
    }

    let expected = std::fs::read_to_string(&golden_path).unwrap_or_else(|_| {
        panic!(
            "Golden file not found: {}. Run with GLSL_UPDATE_GOLDEN=1 to create.",
            golden_path.display()
        )
    });
    let expected_norm = normalize_for_comparison(&expected);

    assert_eq!(
        generated, expected_norm,
        "Vertex stage output differs from golden. Run with GLSL_UPDATE_GOLDEN=1 to update."
    );
}

#[test]
fn glsl_comparison_golden_pixel_tiledimage() {
    let doc = load_stdlib_doc();
    let mut ctx = create_context();
    ctx.load_libraries_from_document(&doc);
    let node_graph = doc.get_node_graph("NG_tiledimage_float").expect("NG");
    let shader = ctx
        .get_shader_generator()
        .generate("golden_tiledimage", &node_graph, &ctx);

    let generated = normalize_for_comparison(shader.get_source_code("pixel"));
    let fixtures_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/glsl_expected");
    let golden_path = fixtures_dir.join("tiledimage_float_pixel.glsl");

    if std::env::var("GLSL_UPDATE_GOLDEN").is_ok() {
        std::fs::create_dir_all(&fixtures_dir).expect("create fixtures dir");
        std::fs::write(&golden_path, &generated).expect("write golden");
        return;
    }

    let expected = std::fs::read_to_string(&golden_path).unwrap_or_else(|_| {
        panic!(
            "Golden file not found: {}. Run with GLSL_UPDATE_GOLDEN=1 to create.",
            golden_path.display()
        )
    });
    let expected_norm = normalize_for_comparison(&expected);

    assert_eq!(
        generated, expected_norm,
        "Pixel stage output differs from golden. Run with GLSL_UPDATE_GOLDEN=1 to update."
    );
}

#[test]
fn glsl_comparison_golden_surface_unlit() {
    let doc = load_stdlib_doc();
    let mut ctx = create_context();
    ctx.load_libraries_from_document(&doc);
    let node_graph = doc
        .get_node_graph("NG_convert_color3_surfaceshader")
        .expect("NG");
    let shader = ctx
        .get_shader_generator()
        .generate("golden_surface", &node_graph, &ctx);

    let vs = normalize_for_comparison(shader.get_source_code("vertex"));
    let ps = normalize_for_comparison(shader.get_source_code("pixel"));

    let fixtures_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/glsl_expected");

    if std::env::var("GLSL_UPDATE_GOLDEN").is_ok() {
        std::fs::create_dir_all(&fixtures_dir).expect("create fixtures dir");
        std::fs::write(fixtures_dir.join("surface_unlit_vertex.glsl"), &vs).expect("write vs");
        std::fs::write(fixtures_dir.join("surface_unlit_pixel.glsl"), &ps).expect("write ps");
        return;
    }

    let expected_vs = std::fs::read_to_string(fixtures_dir.join("surface_unlit_vertex.glsl")).unwrap_or_else(|_| {
        panic!(
            "Golden file surface_unlit_vertex.glsl not found. Run with GLSL_UPDATE_GOLDEN=1 to create."
        )
    });
    let expected_ps = std::fs::read_to_string(fixtures_dir.join("surface_unlit_pixel.glsl")).unwrap_or_else(|_| {
        panic!(
            "Golden file surface_unlit_pixel.glsl not found. Run with GLSL_UPDATE_GOLDEN=1 to create."
        )
    });

    assert_eq!(
        vs,
        normalize_for_comparison(&expected_vs),
        "Surface vertex differs from golden"
    );
    assert_eq!(
        ps,
        normalize_for_comparison(&expected_ps),
        "Surface pixel differs from golden"
    );
}
