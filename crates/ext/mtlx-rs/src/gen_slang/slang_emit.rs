//! Slang emit -- emit vertex/pixel stages with Slang syntax (ref: MaterialXGenSlang).
//! Reuses GLSL structure, applies Slang type substitutions (vec*->float*, etc.).

use crate::core::Document;
use crate::format::read_file;
use crate::gen_hw::{hw_block as block, hw_ident as ident, hw_token as token};
use crate::gen_shader::{
    GenOptions, HwDirectionalAlbedoMethod, HwSpecularEnvironmentMethod, HwTransmissionRenderMethod,
    Shader, ShaderGraph, ShaderGraphCreateContext, ShaderImplContext, ShaderNodeClassification,
    ShaderPort, ShaderStage, shader_stage,
};

use super::SlangShaderGraphContext;
use super::slang_shader_generator::SlangShaderGenerator;
use super::slang_syntax::{FLAT_QUALIFIER, UNIFORM_QUALIFIER};
use crate::gen_glsl::{emit_function_calls, emit_function_definitions, token_substitutions};

const LIB_MX_MATH: &str = "stdlib/genslang/lib/mx_math.slang";
const LIB_MX_TEXTURE: &str = "stdlib/genslang/lib/mx_texture.slang";

// Library paths for specular environment (ref: GlslShaderGenerator::emitSpecularEnvironment)
const LIB_ENV_FIS: &str = "pbrlib/genglsl/lib/mx_environment_fis.glsl";
const LIB_ENV_PREFILTER: &str = "pbrlib/genglsl/lib/mx_environment_prefilter.glsl";
const LIB_ENV_NONE: &str = "pbrlib/genglsl/lib/mx_environment_none.glsl";

// Library paths for transmission render (ref: GlslShaderGenerator::emitTransmissionRender)
const LIB_TRANSMISSION_REFRACT: &str = "pbrlib/genglsl/lib/mx_transmission_refract.glsl";
const LIB_TRANSMISSION_OPACITY: &str = "pbrlib/genglsl/lib/mx_transmission_opacity.glsl";

// Library paths for shadow, albedo table, env prefilter
const LIB_SHADOW: &str = "pbrlib/genglsl/lib/mx_shadow.glsl";
const LIB_SHADOW_PLATFORM: &str = "pbrlib/genglsl/lib/mx_shadow_platform.glsl";
const LIB_ALBEDO_TABLE: &str = "pbrlib/genglsl/lib/mx_generate_albedo_table.glsl";
const LIB_ENV_PREFILTER_GEN: &str = "pbrlib/genglsl/lib/mx_generate_prefilter_env.glsl";

/// Token for file UV transform (ref: ShaderGenerator::T_FILE_TRANSFORM_UV).
const T_FILE_TRANSFORM_UV: &str = "$fileTransformUv";

/// Resolve Slang type name from TypeSystem (uses registered TypeSyntax).
fn slang_type_name(mtlx: &str) -> &'static str {
    match mtlx {
        "float" => "float",
        "integer" | "boolean" => "int",
        "vector2" => "float2",
        "vector3" => "float3",
        "vector4" => "float4",
        "color3" => "float3",
        "color4" => "float4",
        "matrix33" => "float3x3",
        "matrix44" => "float4x4",
        "filename" => "SamplerTexture2D",
        "string" => "int",
        _ => "float",
    }
}

/// Check if char is allowed before a token boundary (ref: isAllowedBeforeToken).
fn is_allowed_before(ch: u8) -> bool {
    ch.is_ascii_whitespace() || ch == b'(' || ch == b',' || ch == b'-' || ch == b'+'
}

/// Check if char is allowed after a token boundary (ref: isAllowedAfterToken).
fn is_allowed_after(ch: u8) -> bool {
    ch.is_ascii_whitespace() || ch == b'(' || ch == b')' || ch == b','
}

/// Word-boundary-aware replacement (ref: SlangShaderGenerator::SlangSyntaxFromGlsl).
fn replace_word_boundary(source: &mut String, from: &str, to: &str) {
    let from_len = from.len();
    let mut pos = 0;
    while let Some(idx) = source[pos..].find(from) {
        let abs_pos = pos + idx;
        let before_ok = abs_pos == 0 || is_allowed_before(source.as_bytes()[abs_pos - 1]);
        let after_pos = abs_pos + from_len;
        let after_ok = after_pos >= source.len() || is_allowed_after(source.as_bytes()[after_pos]);
        if before_ok && after_ok {
            source.replace_range(abs_pos..after_pos, to);
            pos = abs_pos + to.len();
        } else {
            pos = abs_pos + from_len;
        }
    }
}

/// GLSL->Slang token substitutions (ref: SlangShaderGenerator::SlangSyntaxFromGlsl).
/// Word-boundary-aware replacement + static const fixups.
fn slang_syntax_from_glsl(source: &str) -> String {
    let mut result = source.to_string();
    // Word-boundary-aware replacements
    let word_replacements: &[(&str, &str)] = &[
        ("sampler2D", "SamplerTexture2D"),
        ("dFdy", "ddy"),
        ("dFdx", "ddx"),
        ("mix", "lerp"),
        ("fract", "frac"),
        ("vec2", "float2"),
        ("vec3", "float3"),
        ("vec4", "float4"),
        ("ivec2", "int2"),
        ("ivec3", "int3"),
        ("ivec4", "int4"),
        ("uvec2", "uint2"),
        ("uvec3", "uint3"),
        ("uvec4", "uint4"),
        ("bvec2", "bool2"),
        ("bvec3", "bool3"),
        ("bvec4", "bool4"),
        ("mat2", "float2x2"),
        ("mat3", "float3x3"),
        ("mat4", "float4x4"),
    ];
    for (from, to) in word_replacements {
        replace_word_boundary(&mut result, from, to);
    }
    // Literal replacements (gl_FragCoord, gl_Position)
    let literal: &[(&str, &str)] = &[
        ("gl_FragCoord.xy", "vd.SV_Position.xy"),
        ("gl_FragCoord.z", "vd.SV_Position.z"),
        ("gl_Position", "vd.SV_Position"),
    ];
    for (from, to) in literal {
        result = result.replace(from, to);
    }
    // Static const fixups (ref: microfacet_diffuse, microfacet_specular, blackbody)
    let static_fixups: &[(&str, &str)] = &[
        (
            "const float FUJII_CONSTANT_1",
            "static const float FUJII_CONSTANT_1",
        ),
        (
            "const float FUJII_CONSTANT_2",
            "static const float FUJII_CONSTANT_2",
        ),
        (
            "const int FRESNEL_MODEL_DIELECTRIC",
            "static const int FRESNEL_MODEL_DIELECTRIC",
        ),
        (
            "const int FRESNEL_MODEL_CONDUCTOR",
            "static const int FRESNEL_MODEL_CONDUCTOR",
        ),
        (
            "const int FRESNEL_MODEL_SCHLICK",
            "static const int FRESNEL_MODEL_SCHLICK",
        ),
        (
            "const float3x3 XYZ_to_RGB",
            "static const float3x3 XYZ_to_RGB",
        ),
    ];
    for (from, to) in static_fixups {
        result = result.replace(from, to);
    }

    // Static const fixups (ref: microfacet_diffuse, microfacet_specular, blackbody)
    let static_const_fixups: &[(&str, &str)] = &[
        (
            "const float FUJII_CONSTANT_1",
            "static const float FUJII_CONSTANT_1",
        ),
        (
            "const float FUJII_CONSTANT_2",
            "static const float FUJII_CONSTANT_2",
        ),
        (
            "const int FRESNEL_MODEL_DIELECTRIC",
            "static const int FRESNEL_MODEL_DIELECTRIC",
        ),
        (
            "const int FRESNEL_MODEL_CONDUCTOR",
            "static const int FRESNEL_MODEL_CONDUCTOR",
        ),
        (
            "const int FRESNEL_MODEL_SCHLICK",
            "static const int FRESNEL_MODEL_SCHLICK",
        ),
        (
            "const float3x3 XYZ_to_RGB",
            "static const float3x3 XYZ_to_RGB",
        ),
    ];
    for (from, to) in static_const_fixups {
        result = result.replace(from, to);
    }

    result
}

fn emit_library_include<C: ShaderGraphCreateContext + ShaderImplContext>(
    filename: &str,
    ctx: &C,
    stage: &mut ShaderStage,
) {
    let full_path = {
        let prefix = ctx.get_options().library_prefix.trim();
        if prefix.is_empty() {
            filename.to_string()
        } else {
            format!("{}/{}", prefix, filename)
        }
    };
    let resolved = match ctx.resolve_source_file(&full_path, None) {
        Some(p) => p,
        None => return,
    };
    let key = resolved.as_str().to_string();
    if stage.has_source_dependency(&key) {
        return;
    }
    let content = read_file(&resolved);
    if content.is_empty() {
        return;
    }
    stage.append_source_code(&content);
    stage.append_line("");
    stage.add_source_dependency(key);
}

/// Check if graph requires lighting (ref: SlangShaderGenerator::requiresLighting).
fn requires_lighting(graph: &ShaderGraph) -> bool {
    let is_bsdf = graph.has_classification(ShaderNodeClassification::BSDF);
    let is_lit_surface = graph.has_classification(ShaderNodeClassification::SHADER)
        && graph.has_classification(ShaderNodeClassification::SURFACE)
        && !graph.has_classification(ShaderNodeClassification::UNLIT);
    is_bsdf || is_lit_surface
}

/// Convert variable to float4 (ref: SlangShaderGenerator::toVec4).
fn to_vec4(type_name: &str, variable: &str) -> String {
    match type_name {
        "color3" | "vector3" => format!("float4({}, 1.0)", variable),
        "color4" | "vector4" => variable.to_string(),
        "vector2" => format!("float4({}, 0.0, 1.0)", variable),
        "float" | "integer" => format!("float4({v}, {v}, {v}, 1.0)", v = variable),
        // BSDF/EDF are float3 in Slang
        "BSDF" | "EDF" => format!("float4({}, 1.0)", variable),
        _ => "float4(0.0, 0.0, 0.0, 1.0)".to_string(),
    }
}

/// Emit a single variable declaration with Slang-specific handling
/// (ref: SlangShaderGenerator::emitVariableDeclaration).
///
/// Handles:
/// - FILENAME type -> SamplerTexture2D
/// - FLAT_QUALIFIER (nointerpolation) for integer geomprops
/// - Array variable suffix
/// - Semantic annotation (: SEMANTIC)
/// - Conditional value assignment (skip for uniforms)
fn emit_var_decl(
    port: &ShaderPort,
    qualifier: &str,
    type_system: &crate::gen_shader::TypeSystem,
    assign_value: bool,
) -> String {
    let type_name = port.get_type().get_name();

    // FILENAME special handling: use SamplerTexture2D (ref: C++ line 803-807)
    if type_name == "filename" {
        let q = if qualifier.is_empty() {
            String::new()
        } else {
            format!("{} ", qualifier)
        };
        return format!("{}SamplerTexture2D {}", q, port.get_variable());
    }

    let mut str = if qualifier.is_empty() {
        String::new()
    } else {
        format!("{} ", qualifier)
    };

    // FLAT_QUALIFIER for integer geomprops (ref: C++ line 813-816)
    // Varying parameters of type int must be flat qualified on output from vertex stage
    // and input to pixel stage. The only way to get these is with geompropvalue_integer nodes.
    if qualifier.is_empty()
        && type_name == "integer"
        && !assign_value
        && port.get_name().starts_with(ident::IN_GEOMPROP)
    {
        str.push_str(FLAT_QUALIFIER);
        str.push(' ');
    }

    let tn = slang_type_name(type_system.get_type(type_name).get_name());
    str.push_str(tn);
    str.push(' ');
    str.push_str(port.get_variable());

    // Array variable suffix (ref: C++ line 820-823)
    if port.get_type().is_array() {
        if let Some(val) = &port.value {
            let suffix = match val {
                crate::core::Value::FloatArray(arr) => format!("[{}]", arr.len()),
                crate::core::Value::IntegerArray(arr) => format!("[{}]", arr.len()),
                _ => {
                    let s = val.get_value_string();
                    if s.is_empty() {
                        String::new()
                    } else {
                        let count = s.split(',').count();
                        format!("[{}]", count)
                    }
                }
            };
            str.push_str(&suffix);
        }
    }

    // Semantic annotation (ref: C++ line 825-828)
    if !port.semantic.is_empty() {
        str.push_str(" : ");
        str.push_str(&port.semantic);
    }

    // Value assignment (ref: C++ line 830-834)
    // Skip for uniforms (Slang cannot initialize uniforms from code)
    if assign_value && qualifier != UNIFORM_QUALIFIER {
        let val_str = port.get_value_string();
        if !val_str.is_empty() {
            str.push_str(" = ");
            str.push_str(&val_str);
        }
    }

    str
}

/// Emit type definitions from syntax (ref: emitTypeDefinitions).
/// Iterates all registered type syntaxes and emits non-empty type_definition strings.
fn emit_type_definitions(syntax: &super::slang_syntax::SlangSyntax, stage: &mut ShaderStage) {
    let syn = syntax.get_syntax();
    // Sort by key for deterministic output (HashMap iteration is unordered)
    let mut entries: Vec<_> = syn.iter_type_syntax().collect();
    entries.sort_by_key(|(name, _)| name.as_str());
    for (_, type_syn) in entries {
        if !type_syn.type_definition.is_empty() {
            stage.append_line(&type_syn.type_definition);
        }
    }
}

/// Emit constants with "static const" qualifier (ref: SlangShaderGenerator::emitConstants).
fn emit_constants(stage: &mut ShaderStage, type_system: &crate::gen_shader::TypeSystem) {
    let constants = stage.get_constant_block();
    if constants.is_empty() {
        return;
    }
    let vars: Vec<_> = constants
        .get_variable_order()
        .iter()
        .filter_map(|n| {
            constants.find(n).map(|v| {
                (
                    v.get_type().get_name().to_string(),
                    v.get_variable().to_string(),
                    v.get_value_string(),
                )
            })
        })
        .collect();
    for (ty, var, val) in &vars {
        let tn = slang_type_name(type_system.get_type(ty).get_name());
        if val.is_empty() {
            stage.append_line(&format!("static const {} {};", tn, var));
        } else {
            stage.append_line(&format!("static const {} {} = {};", tn, var, val));
        }
    }
    stage.append_line("");
}

/// Emit uniforms with light data struct + cbuffer (ref: SlangShaderGenerator::emitUniforms).
/// Emits ALL uniform blocks inside cbuffer, not just PRIVATE_UNIFORMS.
fn emit_uniforms(
    stage: &mut ShaderStage,
    opts: &GenOptions,
    emit_lighting: bool,
    type_system: &crate::gen_shader::TypeSystem,
) {
    // Light data struct and MAX_LIGHT_SOURCES define
    if emit_lighting {
        let max_lights = opts.hw_max_active_light_sources.max(1);
        stage.append_line(&format!(
            "#define {} {}",
            ident::LIGHT_DATA_MAX_LIGHT_SOURCES,
            max_lights
        ));
        stage.append_line("");

        // Emit LightData struct
        if let Some(light_data) = stage.get_uniform_block(block::LIGHT_DATA) {
            let struct_name = light_data.get_name().to_string();
            let vars: Vec<_> = light_data
                .get_variable_order()
                .iter()
                .filter_map(|n| {
                    light_data.find(n).map(|v| {
                        (
                            v.get_type().get_name().to_string(),
                            v.get_variable().to_string(),
                        )
                    })
                })
                .collect();
            stage.append_line(&format!("struct {}", struct_name));
            stage.append_line("{");
            for (ty, var) in &vars {
                let tn = slang_type_name(type_system.get_type(ty).get_name());
                stage.append_line(&format!("    {} {};", tn, var));
            }
            stage.append_line("};");
            stage.append_line("");
        }
    }

    // Emit cbuffer with ALL uniform blocks (ref: SlangShaderGenerator::emitUniforms)
    stage.append_line(&format!("cbuffer {}CB", stage.name));
    stage.append_line("{");

    // Collect block names to avoid borrow issues
    let block_names: Vec<String> = stage
        .get_uniform_blocks()
        .iter()
        .filter(|(_, b)| !b.is_empty() && b.get_name() != block::LIGHT_DATA)
        .map(|(k, _)| k.clone())
        .collect();

    for block_name in &block_names {
        if let Some(uniforms) = stage.get_uniform_block(block_name) {
            let uname = uniforms.get_name().to_string();
            let vars: Vec<_> = uniforms
                .get_variable_order()
                .iter()
                .filter_map(|n| {
                    uniforms.find(n).map(|v| {
                        (
                            v.get_type().get_name().to_string(),
                            v.get_variable().to_string(),
                        )
                    })
                })
                .collect();
            stage.append_line(&format!("    // Uniform block: {}", uname));
            for (ty, var) in &vars {
                let tn = slang_type_name(type_system.get_type(ty).get_name());
                stage.append_line(&format!("    uniform {} {};", tn, var));
            }
            stage.append_line("");
        }
    }

    // Emit light data array inside cbuffer
    if emit_lighting {
        if let Some(light_data) = stage.get_uniform_block(block::LIGHT_DATA) {
            let struct_name = light_data.get_name().to_string();
            let instance_name = light_data.get_instance().to_string();
            stage.append_line(&format!(
                "    uniform {} {}[{}];",
                struct_name,
                instance_name,
                ident::LIGHT_DATA_MAX_LIGHT_SOURCES
            ));
        }
    }

    stage.append_line("}");
    stage.append_line("");
}

/// Emit specular environment library include (ref: GlslShaderGenerator::emitSpecularEnvironment).
fn emit_specular_environment<C: ShaderGraphCreateContext + ShaderImplContext>(
    ctx: &C,
    stage: &mut ShaderStage,
) {
    let method = ctx.get_options().hw_specular_environment_method;
    match method {
        HwSpecularEnvironmentMethod::Fis => emit_library_include(LIB_ENV_FIS, ctx, stage),
        HwSpecularEnvironmentMethod::Prefilter => {
            emit_library_include(LIB_ENV_PREFILTER, ctx, stage)
        }
        HwSpecularEnvironmentMethod::None => emit_library_include(LIB_ENV_NONE, ctx, stage),
    }
    stage.append_line("");
}

/// Emit transmission render library include (ref: GlslShaderGenerator::emitTransmissionRender).
fn emit_transmission_render<C: ShaderGraphCreateContext + ShaderImplContext>(
    ctx: &C,
    stage: &mut ShaderStage,
) {
    let method = ctx.get_options().hw_transmission_render_method;
    match method {
        HwTransmissionRenderMethod::Refraction => {
            emit_library_include(LIB_TRANSMISSION_REFRACT, ctx, stage)
        }
        HwTransmissionRenderMethod::Opacity => {
            emit_library_include(LIB_TRANSMISSION_OPACITY, ctx, stage)
        }
    }
    stage.append_line("");
}

/// Emit function calls filtered by classification (ref: emitFunctionCalls with classification).
fn emit_function_calls_filtered(
    graph: &ShaderGraph,
    doc: &Document,
    ctx: &SlangShaderGraphContext<'_>,
    stage: &mut ShaderStage,
    classification: u32,
) {
    let target = ctx.get_implementation_target();
    for node_name in &graph.node_order {
        let node = match graph.get_node(node_name) {
            Some(n) => n,
            None => continue,
        };
        // Filter by classification
        if !node.has_classification(classification) {
            continue;
        }
        let node_def_name = match graph.get_node_def(node_name) {
            Some(nd) => nd,
            None => continue,
        };
        let impl_ = match ctx.get_implementation_for_nodedef(doc, node_def_name, target) {
            Some(i) => i,
            None => continue,
        };
        impl_.emit_function_call(node, ctx, stage);
    }
}

/// Emit a single function call for a named node (ref: emitFunctionCall(*upstream, ...)).
fn emit_function_call_for_node(
    graph: &ShaderGraph,
    doc: &Document,
    ctx: &SlangShaderGraphContext<'_>,
    stage: &mut ShaderStage,
    node_name: &str,
) {
    let target = ctx.get_implementation_target();
    let node = match graph.get_node(node_name) {
        Some(n) => n,
        None => return,
    };
    let node_def_name = match graph.get_node_def(node_name) {
        Some(nd) => nd,
        None => return,
    };
    let impl_ = match ctx.get_implementation_for_nodedef(doc, node_def_name, target) {
        Some(i) => i,
        None => return,
    };
    impl_.emit_function_call(node, ctx, stage);
}

/// Emit light function definitions (ref: SlangShaderGenerator::emitLightFunctionDefinitions).
/// Iterates HwLightShaders user data (bound light shader definitions) and _lightSamplingNodes.
fn emit_light_fn_defs(
    graph: &ShaderGraph,
    doc: &Document,
    ctx: &SlangShaderGraphContext<'_>,
    stage: &mut ShaderStage,
) {
    // Only emit if lighting is required and max light sources > 0
    if !requires_lighting(graph) || ctx.get_options().hw_max_active_light_sources == 0 {
        return;
    }
    // Only for surface shaders (ref: C++ checks SHADER | SURFACE)
    if !graph
        .has_classification(ShaderNodeClassification::SHADER | ShaderNodeClassification::SURFACE)
    {
        return;
    }

    // Emit functions for all bound light shaders (ref: HwLightShaders user data iteration)
    // In Rust, we iterate graph nodes with LIGHT classification as an approximation
    // since the full HwLightShaders user data system is not yet implemented.
    let target = ctx.get_implementation_target();
    for node_name in &graph.node_order {
        let node = match graph.get_node(node_name) {
            Some(n) => n,
            None => continue,
        };
        if !node.has_classification(ShaderNodeClassification::LIGHT) {
            continue;
        }
        let node_def_name = match graph.get_node_def(node_name) {
            Some(nd) => nd,
            None => continue,
        };
        if let Some(impl_) = ctx.get_implementation_for_nodedef(doc, node_def_name, target) {
            impl_.emit_function_definition(node, ctx, stage);
        }
    }

    // Emit functions for light sampling nodes (ref: _lightSamplingNodes iteration)
    // numActiveLightSources and sampleLightSource are emitted via their node implementations
    emit_light_sampling_functions(ctx, doc, stage);
}

/// Emit light sampling node function definitions (ref: _lightSamplingNodes).
/// These are numActiveLightSources and sampleLightSource, created in the C++ constructor.
fn emit_light_sampling_functions(
    ctx: &SlangShaderGraphContext<'_>,
    _doc: &Document,
    stage: &mut ShaderStage,
) {
    let target = ctx.get_implementation_target();

    // Try to find and emit numActiveLightSources implementation
    let num_lights_impl_name = format!("IM_numActiveLightSources_{}", target);
    if let Some(impl_) = ctx
        .ctx
        .get_shader_generator()
        .get_impl_factory()
        .create(&num_lights_impl_name)
    {
        // Create a minimal ShaderNode for the function definition
        let node = crate::gen_shader::ShaderNode::new_minimal("numActiveLightSources", "integer");
        impl_.emit_function_definition(&node, ctx, stage);
    }

    // Try to find and emit sampleLightSource implementation
    let sample_light_impl_name = format!("IM_sampleLightSource_{}", target);
    if let Some(impl_) = ctx
        .ctx
        .get_shader_generator()
        .get_impl_factory()
        .create(&sample_light_impl_name)
    {
        let node = crate::gen_shader::ShaderNode::new_minimal("sampleLightSource", "lightshader");
        impl_.emit_function_definition(&node, ctx, stage);
    }
}

/// Emit vertex stage (ref: SlangShaderGenerator::emitVertexStage).
fn emit_vertex_stage_slang(
    graph: &ShaderGraph,
    doc: &Document,
    ctx: &SlangShaderGraphContext<'_>,
    stage: &mut ShaderStage,
    _syntax: &super::slang_syntax::SlangSyntax,
) {
    let type_system = ctx.get_type_system();

    // emitDirectives -- no-op for Slang (ref: C++ emitDirectives is empty)
    stage.append_line("");

    // emitConstants (ref: C++ emitVertexStage calls emitConstants)
    emit_constants(stage, type_system);

    // emitUniforms with ALL blocks (ref: C++ emitUniforms(context, stage, false))
    emit_uniforms(stage, ctx.get_options(), false, type_system);

    // emitInputs for VERTEX stage (ref: SlangShaderGenerator::emitInputs VERTEX)
    // Collect vertex input vars
    let vertex_input_vars: Vec<_> = stage
        .get_input_block(block::VERTEX_INPUTS)
        .map(|b| {
            b.get_variable_order()
                .iter()
                .filter_map(|n| {
                    b.find(n).map(|v| {
                        (
                            v.get_type().get_name().to_string(),
                            v.get_variable().to_string(),
                            v.semantic.clone(),
                            v.get_name().to_string(),
                        )
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if !vertex_input_vars.is_empty() {
        // Emit the input struct with semantic annotations
        stage.append_line(&format!("struct {}", block::VERTEX_INPUTS));
        stage.append_line("{");
        // Use emitVariableDeclarations with input qualifier (ref: C++ line 374)
        if let Some(input_block) = stage.get_input_block(block::VERTEX_INPUTS) {
            let decl_lines: Vec<String> = input_block
                .get_variable_order()
                .iter()
                .filter_map(|n| input_block.find(n))
                .map(|port| format!("    {};", emit_var_decl(port, "", type_system, false)))
                .collect();
            for line in decl_lines {
                stage.append_line(&line);
            }
        }
        stage.append_line("};");
        stage.append_line("");

        // Static variable declarations for vertex inputs (ref: C++ emitInputs line 378-387)
        // These are global mirrors of the input struct fields, without semantics
        for (ty, var, _sem, _name) in &vertex_input_vars {
            let tn = slang_type_name(type_system.get_type(ty).get_name());
            stage.append_line(&format!("static {} {};", tn, var));
        }
    }

    // emitOutputs for VERTEX stage (ref: SlangShaderGenerator::emitOutputs VERTEX)
    stage.append_line(&format!("struct {}", block::VERTEX_DATA));
    stage.append_line("{");
    stage.append_line("    float4 SV_Position : SV_Position;");
    // Use emitVariableDeclarations for vertex data outputs
    if let Some(output_block) = stage.get_output_block(block::VERTEX_DATA) {
        let decl_lines: Vec<String> = output_block
            .get_variable_order()
            .iter()
            .filter_map(|n| output_block.find(n))
            .map(|port| format!("    {};", emit_var_decl(port, "", type_system, false)))
            .collect();
        for line in decl_lines {
            stage.append_line(&line);
        }
    }
    stage.append_line("};");
    stage.append_line("");
    stage.append_line("");

    // Add common math functions
    emit_library_include(LIB_MX_MATH, ctx, stage);
    stage.append_line("");

    // emitFunctionDefinitions
    emit_function_definitions(graph, doc, ctx, stage);

    // Main function
    stage.append_line("[shader(\"vertex\")]");
    stage.append_line(&format!(
        "{} vertexMain({} vi)",
        block::VERTEX_DATA,
        block::VERTEX_INPUTS
    ));
    stage.append_line("{");

    // Assign input variables from `vi` struct to static globals (ref: C++ line 252-253)
    stage.append_line("    // Variable input block");
    for (_, var, _, _) in &vertex_input_vars {
        stage.append_line(&format!("    {} = vi.{};", var, var));
    }

    stage.append_line(&format!(
        "    float4 hPositionWorld = mul(float4({}, 1.0), {});",
        token::T_IN_POSITION,
        token::T_WORLD_MATRIX
    ));
    stage.append_line(&format!(
        "    {} {};",
        block::VERTEX_DATA,
        ident::VERTEX_DATA_INSTANCE
    ));
    stage.append_line(&format!(
        "    {}.SV_Position = mul(hPositionWorld, {});",
        ident::VERTEX_DATA_INSTANCE,
        token::T_VIEW_PROJECTION_MATRIX
    ));

    // Emit all function calls in order
    emit_function_calls(graph, doc, ctx, stage);

    stage.append_line(&format!("    return {};", ident::VERTEX_DATA_INSTANCE));
    stage.append_line("}");
}

/// Emit pixel stage (ref: SlangShaderGenerator::emitPixelStage).
fn emit_pixel_stage_slang(
    graph: &ShaderGraph,
    doc: &Document,
    ctx: &SlangShaderGraphContext<'_>,
    stage: &mut ShaderStage,
    syntax: &super::slang_syntax::SlangSyntax,
) {
    let type_system = ctx.get_type_system();
    let opts = ctx.get_options().clone();

    // emitDirectives -- no-op
    stage.append_line("");

    // Add textures
    emit_library_include(LIB_MX_TEXTURE, ctx, stage);

    // emitTypeDefinitions (ref: C++ emitPixelStage calls emitTypeDefinitions)
    emit_type_definitions(syntax, stage);

    // Add all constants with "static const" qualifier
    emit_constants(stage, type_system);

    // Determine whether lighting is required
    let lighting = requires_lighting(graph);
    let emit_light_uniforms = lighting && opts.hw_max_active_light_sources > 0;

    // Add all uniforms (light data struct + cbuffer with ALL uniform blocks)
    emit_uniforms(stage, &opts, emit_light_uniforms, type_system);

    // Add vertex data inputs block (ref: emitInputs for PIXEL stage)
    stage.append_line(&format!("struct {}", block::VERTEX_DATA));
    stage.append_line("{");
    stage.append_line("    float4 SV_Position : SV_Position;");
    if let Some(vd_block) = stage.get_input_block(block::VERTEX_DATA) {
        let decl_lines: Vec<String> = vd_block
            .get_variable_order()
            .iter()
            .filter_map(|n| vd_block.find(n))
            .map(|port| format!("    {};", emit_var_decl(port, "", type_system, false)))
            .collect();
        for line in decl_lines {
            stage.append_line(&line);
        }
    }
    stage.append_line("}");
    stage.append_line("");
    stage.append_line(&format!("static {} vd;", block::VERTEX_DATA));
    stage.append_line("");

    // Add common math functions
    emit_library_include(LIB_MX_MATH, ctx, stage);
    stage.append_line("");

    // Define directional albedo approach
    if lighting || opts.hw_write_albedo_table || opts.hw_write_env_prefilter {
        let method_val = match opts.hw_directional_albedo_method {
            HwDirectionalAlbedoMethod::Analytic => 0,
            HwDirectionalAlbedoMethod::Table => 1,
            HwDirectionalAlbedoMethod::MonteCarlo => 2,
        };
        stage.append_line(&format!("#define DIRECTIONAL_ALBEDO_METHOD {}", method_val));
        stage.append_line("");
    }

    // Define Airy Fresnel iterations
    stage.append_line(&format!(
        "#define AIRY_FRESNEL_ITERATIONS {}",
        opts.hw_airy_fresnel_iterations
    ));
    stage.append_line("");

    // Add lighting support
    if lighting {
        emit_specular_environment(ctx, stage);
        emit_transmission_render(ctx, stage);
    }

    // Add shadowing support
    let shadowing = (lighting && opts.hw_shadow_map) || opts.hw_write_depth_moments;
    if shadowing {
        emit_library_include(LIB_SHADOW, ctx, stage);
        emit_library_include(LIB_SHADOW_PLATFORM, ctx, stage);
    }

    // Emit directional albedo table code
    if opts.hw_write_albedo_table {
        emit_library_include(LIB_ALBEDO_TABLE, ctx, stage);
        stage.append_line("");
    }

    // Emit environment prefiltering code
    if opts.hw_write_env_prefilter {
        emit_library_include(LIB_ENV_PREFILTER_GEN, ctx, stage);
        stage.append_line("");
    }

    // Emit light function definitions
    emit_light_fn_defs(graph, doc, ctx, stage);

    // Emit function definitions for all nodes in the graph
    emit_function_definitions(graph, doc, ctx, stage);

    // Get output socket info before entering the main function
    let output_socket_info = graph.get_output_socket_at(0).map(|s| {
        (
            s.port.get_variable().to_string(),
            s.port.get_type().get_name().to_string(),
            s.get_connection()
                .map(|(n, o)| (n.to_string(), o.to_string())),
        )
    });

    // Collect all output sockets for surface shader emit
    let all_output_sockets: Vec<_> = graph
        .node
        .input_order
        .iter()
        .filter_map(|name| {
            graph
                .node
                .inputs
                .get(name)
                .and_then(|socket| socket.get_connection().map(|(n, _o)| n.to_string()))
        })
        .collect();

    // Pixel shader outputs (ref: emitOutputs for PIXEL stage)
    let pixel_output_vars: Vec<_> = stage
        .get_output_block(block::PIXEL_OUTPUTS)
        .map(|b| {
            b.get_variable_order()
                .iter()
                .filter_map(|n| {
                    b.find(n).map(|v| {
                        (
                            v.get_type().get_name().to_string(),
                            v.get_variable().to_string(),
                        )
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    // Add main function
    stage.append_line("[shader(\"fragment\")]");
    stage.append_line(&format!(
        "float4 fragmentMain({} _vd) : SV_Target",
        block::VERTEX_DATA
    ));
    stage.append_line("{");
    stage.append_line("");

    // Fragment shader inputs
    stage.append_line("    // Fragment shader inputs");
    stage.append_line("    vd = _vd;");
    stage.append_line("");

    // Pixel shader outputs (ref: emitOutputs)
    if !pixel_output_vars.is_empty() {
        stage.append_line("    // Pixel shader outputs");
        for (ty, var) in &pixel_output_vars {
            let tn = slang_type_name(type_system.get_type(ty).get_name());
            stage.append_line(&format!("    {} {};", tn, var));
        }
        stage.append_line("");
    }

    // Emit output based on graph classification
    let is_closure_only = graph.has_classification(ShaderNodeClassification::CLOSURE)
        && !graph.has_classification(ShaderNodeClassification::SHADER);
    let is_surface = graph.has_classification(ShaderNodeClassification::SURFACE);
    let is_surface_shader = graph
        .has_classification(ShaderNodeClassification::SHADER | ShaderNodeClassification::SURFACE);

    if let Some((out_var, out_type, connection)) = &output_socket_info {
        if is_closure_only {
            // Direct closure -- output black
            stage.append_line(&format!("    {} = float4(0.0, 0.0, 0.0, 1.0);", out_var));
        } else if opts.hw_write_depth_moments {
            stage.append_line(&format!(
                "    {} = float4(mx_compute_depth_moments(), 0.0, 1.0);",
                out_var
            ));
        } else if opts.hw_write_albedo_table {
            stage.append_line(&format!(
                "    {} = float4(mx_generate_dir_albedo_table(), 1.0);",
                out_var
            ));
        } else if opts.hw_write_env_prefilter {
            stage.append_line(&format!(
                "    {} = float4(mx_generate_prefilter_env(), 1.0);",
                out_var
            ));
        } else {
            // Normal rendering path -- emit function calls
            if is_surface_shader {
                // Emit all texturing nodes first
                emit_function_calls_filtered(
                    graph,
                    doc,
                    ctx,
                    stage,
                    ShaderNodeClassification::TEXTURE,
                );

                // Emit function calls for root closure/shader nodes
                for upstream_name in &all_output_sockets {
                    if let Some(upstream) = graph.get_node(upstream_name) {
                        if upstream.has_classification(ShaderNodeClassification::CLOSURE)
                            || upstream.has_classification(ShaderNodeClassification::SHADER)
                        {
                            emit_function_call_for_node(graph, doc, ctx, stage, upstream_name);
                        }
                    }
                }
            } else {
                // Non-surface: emit all function calls in order
                emit_function_calls(graph, doc, ctx, stage);
            }

            // Emit final output
            if let Some((up_node, up_output)) = connection {
                if let Some(up_var) = graph.get_connection_variable(up_node, up_output) {
                    if is_surface {
                        // Surface shader output: .color and .transparency
                        let mut out_color = format!("{}.color", up_var);
                        let out_transparency = format!("{}.transparency", up_var);

                        if opts.hw_srgb_encode_output {
                            out_color = format!("mx_srgb_encode({})", out_color);
                        }

                        if opts.hw_transparency {
                            stage.append_line(&format!(
                                "    float outAlpha = clamp(1.0 - dot({}, float3(0.3333)), 0.0, 1.0);",
                                out_transparency
                            ));
                            stage.append_line(&format!(
                                "    {} = float4({}, outAlpha);",
                                out_var, out_color
                            ));
                            stage.append_line(&format!(
                                "    if (outAlpha < {})",
                                token::T_ALPHA_THRESHOLD
                            ));
                            stage.append_line("    {");
                            stage.append_line("        discard;");
                            stage.append_line("    }");
                        } else {
                            stage.append_line(&format!(
                                "    {} = float4({}, 1.0);",
                                out_var, out_color
                            ));
                        }
                    } else {
                        // Non-surface output
                        let mut out_value = up_var.clone();

                        if opts.hw_srgb_encode_output
                            && matches!(out_type.as_str(), "color3" | "vector3")
                        {
                            out_value = format!("mx_srgb_encode({})", out_value);
                        }

                        if !matches!(out_type.as_str(), "color4" | "vector4") {
                            out_value = to_vec4(out_type, &out_value);
                        }

                        stage.append_line(&format!("    {} = {};", out_var, out_value));
                    }
                } else {
                    stage.append_line(&format!("    {} = float4(0.0, 0.0, 0.0, 1.0);", out_var));
                }
            } else {
                // No connection -- use default value
                let out_value_str = "float4(0.0, 0.0, 0.0, 1.0)".to_string();
                if !matches!(out_type.as_str(), "color4" | "vector4") {
                    let final_output = format!("{}_tmp", out_var);
                    let default_val = match out_type.as_str() {
                        "float" => "0.0",
                        "integer" => "0",
                        "color3" | "vector3" => "float3(0.0, 0.0, 0.0)",
                        "color4" | "vector4" => "float4(0.0, 0.0, 0.0, 0.0)",
                        "vector2" => "float2(0.0, 0.0)",
                        _ => "0.0",
                    };
                    let tn = slang_type_name(type_system.get_type(out_type).get_name());
                    stage.append_line(&format!("    {} {} = {};", tn, final_output, default_val));
                    let converted = to_vec4(out_type, &final_output);
                    stage.append_line(&format!("    {} = {};", out_var, converted));
                } else {
                    stage.append_line(&format!("    {} = {};", out_var, out_value_str));
                }
            }
        }

        stage.append_line(&format!("    return {};", out_var));
    } else {
        // No output socket -- return black
        stage.append_line("    return float4(0.0, 0.0, 0.0, 1.0);");
    }

    stage.append_line("}");
}

/// Apply Slang semantic annotations to vertex input ports (ref: SlangShaderGenerator::generate).
/// Maps HW token names (e.g. "$inPosition") to HLSL/Slang semantics (e.g. "POSITION").
/// Indexed variants like "$inTexcoord_0" map to "TEXCOORD0".
/// Non-matching names fall back to the port name itself as semantic.
fn assign_vertex_input_semantics(stage: &mut ShaderStage) {
    // Indexed substitutions: prefix match, index appended (e.g. "$inTexcoord_" -> "TEXCOORD")
    let indexed: &[(&str, &str)] = &[
        (token::T_IN_POSITION, "POSITION"),
        (token::T_IN_NORMAL, "NORMAL"),
        (token::T_IN_BITANGENT, "BINORMAL"),
        (token::T_IN_TANGENT, "TANGENT"),
        (token::T_IN_TEXCOORD, "TEXCOORD"),
        (token::T_IN_COLOR, "COLOR"),
    ];

    let input_block = match stage.get_input_block_mut(block::VERTEX_INPUTS) {
        Some(b) => b,
        None => return,
    };

    for port in input_block.variables.iter_mut() {
        if !port.semantic.is_empty() {
            continue;
        }
        let name = port.name.clone();
        let mut semantic = name.clone();

        // Check indexed form first: "$inTexcoord_0" -> "TEXCOORD0"
        let mut matched = false;
        for (token, sem) in indexed {
            let indexed_prefix = format!("{}_", token);
            if let Some(suffix) = name.strip_prefix(indexed_prefix.as_str()) {
                semantic = format!("{}{}", sem, suffix);
                matched = true;
                break;
            }
        }
        // Then check exact match: "$inPosition" -> "POSITION"
        if !matched {
            for (token, sem) in indexed {
                if name == *token {
                    semantic = sem.to_string();
                    break;
                }
            }
        }

        port.semantic = semantic;
    }
}

/// Set inter-stage semantics on VERTEX_DATA output ports: use port name if semantic is empty.
fn assign_vertex_data_semantics(stage: &mut ShaderStage) {
    let output_block = match stage.get_output_block_mut(block::VERTEX_DATA) {
        Some(b) => b,
        None => return,
    };
    for port in output_block.variables.iter_mut() {
        if port.semantic.is_empty() {
            port.semantic = port.name.clone();
        }
    }
}

/// Set inter-stage semantics on VERTEX_DATA input ports in pixel stage.
fn assign_pixel_vertex_data_semantics(stage: &mut ShaderStage) {
    let input_block = match stage.get_input_block_mut(block::VERTEX_DATA) {
        Some(b) => b,
        None => return,
    };
    for port in input_block.variables.iter_mut() {
        if port.semantic.is_empty() {
            port.semantic = port.name.clone();
        }
    }
}

/// Build Slang-specific token substitutions (on top of GLSL base).
fn slang_token_substitutions() -> Vec<(&'static str, &'static str)> {
    let mut subs = token_substitutions();

    // T_TEX_SAMPLER_SIGNATURE -> Slang signature
    subs.push((
        token::T_TEX_SAMPLER_SIGNATURE,
        "SamplerTexture2D tex_sampler",
    ));

    subs
}

/// Emit Slang shader -- vertex/pixel with Slang syntax, GLSL->Slang token substitution.
pub fn emit_shader(
    name: &str,
    graph: ShaderGraph,
    mut stages: Vec<ShaderStage>,
    doc: &Document,
    context: &crate::gen_shader::GenContext<SlangShaderGenerator>,
    generator: &SlangShaderGenerator,
) -> Shader {
    let emit_ctx = SlangShaderGraphContext::with_graph(context, &graph);
    let vs_idx = stages.iter().position(|s| s.name == shader_stage::VERTEX);
    let ps_idx = stages.iter().position(|s| s.name == shader_stage::PIXEL);
    let syntax = generator.get_syntax();

    // Assign Slang semantics before emit (ref: SlangShaderGenerator::generate).
    if let Some(idx) = vs_idx {
        assign_vertex_input_semantics(&mut stages[idx]);
        assign_vertex_data_semantics(&mut stages[idx]);
    }
    if let Some(idx) = ps_idx {
        assign_pixel_vertex_data_semantics(&mut stages[idx]);
    }

    // T_FILE_TRANSFORM_UV token substitution
    let opts = context.get_options();
    let file_transform_uv = if opts.file_texture_vertical_flip {
        "mx_transform_uv_vflip.glsl"
    } else {
        "mx_transform_uv.glsl"
    };

    let subs = slang_token_substitutions();

    // ScopedFloatFormatting equivalent: set fixed-point formatting flag
    // (ref: C++ ScopedFloatFormatting fmt(Value::FloatFormatFixed))
    // In Rust, float formatting in Value::get_value_string() already produces
    // consistent fixed-point notation. This matches the C++ behavior.

    if let Some(idx) = vs_idx {
        emit_vertex_stage_slang(&graph, doc, &emit_ctx, &mut stages[idx], syntax);
        stages[idx].source_code = crate::core::replace_substrings(&stages[idx].source_code, &subs);
        stages[idx].source_code = stages[idx]
            .source_code
            .replace(T_FILE_TRANSFORM_UV, file_transform_uv);
        stages[idx].source_code = slang_syntax_from_glsl(&stages[idx].source_code);
    }
    if let Some(idx) = ps_idx {
        emit_pixel_stage_slang(&graph, doc, &emit_ctx, &mut stages[idx], syntax);
        stages[idx].source_code = crate::core::replace_substrings(&stages[idx].source_code, &subs);
        stages[idx].source_code = stages[idx]
            .source_code
            .replace(T_FILE_TRANSFORM_UV, file_transform_uv);
        stages[idx].source_code = slang_syntax_from_glsl(&stages[idx].source_code);
    }

    Shader::from_parts(name, graph, stages)
}
