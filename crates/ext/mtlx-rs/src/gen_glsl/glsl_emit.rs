//! GLSL emit -- emit vertex/pixel stages (by ref GlslShaderGenerator emitVertexStage, emitPixelStage).

use crate::core::Document;
use crate::format::read_file;
use crate::gen_hw::{hw_block as block, hw_ident as ident, hw_token as token};
use crate::gen_shader::{
    ShaderGraph, ShaderGraphCreateContext, ShaderImplContext, ShaderNodeImpl, ShaderStage,
};

use std::cell::RefCell;
use std::rc::Rc;

use super::GlslShaderGraphContext;
use super::glsl_shader_generator::GlslShaderGenerator;
use super::glsl_syntax::{
    CONSTANT_QUALIFIER, INPUT_QUALIFIER, OUTPUT_QUALIFIER, UNIFORM_QUALIFIER,
};
use crate::gen_shader::{GenContext, GenOptions, ResourceBindingContext};

/// Stdlib mx_math.glsl -- common math helpers (mx_square, mx_matrix_mul, etc.)
const LIB_MX_MATH: &str = "stdlib/genglsl/lib/mx_math.glsl";

/// Token for file UV transform (ref: ShaderGenerator::T_FILE_TRANSFORM_UV).
const T_FILE_TRANSFORM_UV: &str = "$fileTransformUv";

/// Specular environment method library files
const LIB_ENV_FIS: &str = "pbrlib/genglsl/lib/mx_environment_fis.glsl";
const LIB_ENV_PREFILTER: &str = "pbrlib/genglsl/lib/mx_environment_prefilter.glsl";
const LIB_ENV_NONE: &str = "pbrlib/genglsl/lib/mx_environment_none.glsl";

/// Transmission render method library files
const LIB_TRANSMISSION_REFRACT: &str = "pbrlib/genglsl/lib/mx_transmission_refract.glsl";
const LIB_TRANSMISSION_OPACITY: &str = "pbrlib/genglsl/lib/mx_transmission_opacity.glsl";

/// Shadow library files
const LIB_SHADOW: &str = "pbrlib/genglsl/lib/mx_shadow.glsl";
const LIB_SHADOW_PLATFORM: &str = "pbrlib/genglsl/lib/mx_shadow_platform.glsl";

/// Albedo table generation
const LIB_ALBEDO_TABLE: &str = "pbrlib/genglsl/lib/mx_generate_albedo_table.glsl";
/// Environment prefilter generation
const LIB_ENV_PREFILTER_GEN: &str = "pbrlib/genglsl/lib/mx_generate_prefilter_env.glsl";

/// Trait for GLSL/ESSL/Vk/WGSL generators -- provides version and syntax for emit.
pub trait GlslEmitter {
    fn get_version(&self) -> &str;
    fn get_syntax(&self) -> &super::GlslSyntax;
}

/// Emit behavior flags for different generator variants.
#[derive(Clone, Copy, Debug, Default)]
pub struct EmitFlags {
    /// ESSL mode: no interface blocks in pixel stage, precision mediump float
    pub essl_mode: bool,
    /// Vulkan mode: layout(location=N) for inputs/outputs
    pub vk_mode: bool,
    /// Vulkan vertex data location (for inter-stage binding)
    pub vk_vertex_data_location: i32,
}

fn glsl_type_name(mtlx: &str) -> &'static str {
    match mtlx {
        "float" => "float",
        "integer" => "int",
        "boolean" => "bool",
        "vector2" => "vec2",
        "vector3" => "vec3",
        "vector4" => "vec4",
        "color3" => "vec3",
        "color4" => "vec4",
        "matrix33" => "mat3",
        "matrix44" => "mat4",
        "filename" => "sampler2D",
        "string" => "int",
        "BSDF" => "BSDF",
        "EDF" => "EDF",
        "VDF" => "BSDF",
        "surfaceshader" => "surfaceshader",
        "volumeshader" => "volumeshader",
        "displacementshader" => "displacementshader",
        "lightshader" => "lightshader",
        "material" => "material",
        "floatarray" => "float",
        "integerarray" => "int",
        _ => "float",
    }
}

/// Full token substitution map: token -> ident (by ref HwShaderGenerator constructor, ~60 entries).
/// Public for reuse by Slang, WGSL, ESSL, Vk emitters.
pub fn token_substitutions() -> Vec<(&'static str, &'static str)> {
    vec![
        // Vertex attributes
        (token::T_IN_POSITION, ident::IN_POSITION),
        (token::T_IN_NORMAL, ident::IN_NORMAL),
        (token::T_IN_TANGENT, ident::IN_TANGENT),
        (token::T_IN_BITANGENT, ident::IN_BITANGENT),
        (token::T_IN_TEXCOORD, ident::IN_TEXCOORD),
        (token::T_IN_GEOMPROP, ident::IN_GEOMPROP),
        (token::T_IN_COLOR, ident::IN_COLOR),
        // World-space interpolants
        (token::T_POSITION_WORLD, ident::POSITION_WORLD),
        (token::T_NORMAL_WORLD, ident::NORMAL_WORLD),
        (token::T_TANGENT_WORLD, ident::TANGENT_WORLD),
        (token::T_BITANGENT_WORLD, ident::BITANGENT_WORLD),
        // Object-space variants
        (token::T_POSITION_OBJECT, ident::POSITION_OBJECT),
        (token::T_NORMAL_OBJECT, ident::NORMAL_OBJECT),
        (token::T_TANGENT_OBJECT, ident::TANGENT_OBJECT),
        (token::T_BITANGENT_OBJECT, ident::BITANGENT_OBJECT),
        // Generic interpolants
        (token::T_TEXCOORD, ident::TEXCOORD),
        (token::T_COLOR, ident::COLOR),
        // World matrix family
        (token::T_WORLD_MATRIX, ident::WORLD_MATRIX),
        (token::T_WORLD_INVERSE_MATRIX, ident::WORLD_INVERSE_MATRIX),
        (
            token::T_WORLD_TRANSPOSE_MATRIX,
            ident::WORLD_TRANSPOSE_MATRIX,
        ),
        (
            token::T_WORLD_INVERSE_TRANSPOSE_MATRIX,
            ident::WORLD_INVERSE_TRANSPOSE_MATRIX,
        ),
        // View matrix family
        (token::T_VIEW_MATRIX, ident::VIEW_MATRIX),
        (token::T_VIEW_INVERSE_MATRIX, ident::VIEW_INVERSE_MATRIX),
        (token::T_VIEW_TRANSPOSE_MATRIX, ident::VIEW_TRANSPOSE_MATRIX),
        (
            token::T_VIEW_INVERSE_TRANSPOSE_MATRIX,
            ident::VIEW_INVERSE_TRANSPOSE_MATRIX,
        ),
        // Projection matrix family
        (token::T_PROJ_MATRIX, ident::PROJ_MATRIX),
        (token::T_PROJ_INVERSE_MATRIX, ident::PROJ_INVERSE_MATRIX),
        (token::T_PROJ_TRANSPOSE_MATRIX, ident::PROJ_TRANSPOSE_MATRIX),
        (
            token::T_PROJ_INVERSE_TRANSPOSE_MATRIX,
            ident::PROJ_INVERSE_TRANSPOSE_MATRIX,
        ),
        // Composite matrices
        (token::T_WORLD_VIEW_MATRIX, ident::WORLD_VIEW_MATRIX),
        (
            token::T_VIEW_PROJECTION_MATRIX,
            ident::VIEW_PROJECTION_MATRIX,
        ),
        (
            token::T_WORLD_VIEW_PROJECTION_MATRIX,
            ident::WORLD_VIEW_PROJECTION_MATRIX,
        ),
        // View/camera
        (token::T_VIEW_POSITION, ident::VIEW_POSITION),
        (token::T_VIEW_DIRECTION, ident::VIEW_DIRECTION),
        // Time/frame
        (token::T_FRAME, ident::FRAME),
        (token::T_TIME, ident::TIME),
        // Misc
        (token::T_GEOMPROP, ident::GEOMPROP),
        (token::T_ALPHA_THRESHOLD, ident::ALPHA_THRESHOLD),
        (
            token::T_NUM_ACTIVE_LIGHT_SOURCES,
            ident::NUM_ACTIVE_LIGHT_SOURCES,
        ),
        // Environment / IBL
        (token::T_ENV_MATRIX, ident::ENV_MATRIX),
        (token::T_ENV_RADIANCE, ident::ENV_RADIANCE),
        (
            token::T_ENV_RADIANCE_SAMPLER2D,
            ident::ENV_RADIANCE_SAMPLER2D,
        ),
        (token::T_ENV_RADIANCE_MIPS, ident::ENV_RADIANCE_MIPS),
        (token::T_ENV_RADIANCE_SAMPLES, ident::ENV_RADIANCE_SAMPLES),
        (token::T_ENV_IRRADIANCE, ident::ENV_IRRADIANCE),
        (
            token::T_ENV_IRRADIANCE_SAMPLER2D,
            ident::ENV_IRRADIANCE_SAMPLER2D,
        ),
        (token::T_ENV_LIGHT_INTENSITY, ident::ENV_LIGHT_INTENSITY),
        (token::T_REFRACTION_TWO_SIDED, ident::REFRACTION_TWO_SIDED),
        (token::T_ALBEDO_TABLE, ident::ALBEDO_TABLE),
        (token::T_ALBEDO_TABLE_SIZE, ident::ALBEDO_TABLE_SIZE),
        (token::T_ENV_PREFILTER_MIP, ident::ENV_PREFILTER_MIP),
        // Texture sampler helpers
        (token::T_TEX_SAMPLER_SAMPLER2D, ident::TEX_SAMPLER_SAMPLER2D),
        (token::T_TEX_SAMPLER_SIGNATURE, ident::TEX_SAMPLER_SIGNATURE),
        // Closure data constructor
        (
            token::T_CLOSURE_DATA_CONSTRUCTOR,
            crate::gen_hw::hw_lighting::CLOSURE_DATA_CONSTRUCTOR,
        ),
        // Shadow / AO
        (token::T_AMB_OCC_MAP, ident::AMB_OCC_MAP),
        (token::T_AMB_OCC_GAIN, ident::AMB_OCC_GAIN),
        (token::T_SHADOW_MAP, ident::SHADOW_MAP),
        (token::T_SHADOW_MATRIX, ident::SHADOW_MATRIX),
        // Stage data instances
        (token::T_VERTEX_DATA_INSTANCE, ident::VERTEX_DATA_INSTANCE),
        (token::T_LIGHT_DATA_INSTANCE, ident::LIGHT_DATA_INSTANCE),
    ]
}

/// WGSL token substitutions -- texture+sampler split for WebGPU (by ref WgslShaderGenerator.cpp).
fn wgsl_token_substitutions() -> Vec<(&'static str, &'static str)> {
    let wgsl_override_tokens: &[&str] = &[
        token::T_ENV_RADIANCE,
        token::T_ENV_RADIANCE_SAMPLER2D,
        token::T_ENV_IRRADIANCE,
        token::T_ENV_IRRADIANCE_SAMPLER2D,
        token::T_TEX_SAMPLER_SAMPLER2D,
        token::T_TEX_SAMPLER_SIGNATURE,
    ];
    let mut subs: Vec<_> = token_substitutions()
        .into_iter()
        .filter(|(tok, _)| !wgsl_override_tokens.contains(tok))
        .collect();
    // IMPORTANT: RBC appends _texture/_sampler suffixes to token names.
    // Replace suffixed variants FIRST to prevent $envRadiance matching inside $envRadiance_texture.
    subs.push(("$envRadiance_texture", "u_envRadiance_texture"));
    subs.push(("$envRadiance_sampler", "u_envRadiance_sampler"));
    subs.push(("$envIrradiance_texture", "u_envIrradiance_texture"));
    subs.push(("$envIrradiance_sampler", "u_envIrradiance_sampler"));
    subs.push((
        token::T_ENV_RADIANCE_SAMPLER2D,
        ident::ENV_RADIANCE_SAMPLER2D_SPLIT,
    ));
    subs.push((token::T_ENV_RADIANCE, ident::ENV_RADIANCE_SPLIT));
    subs.push((
        token::T_ENV_IRRADIANCE_SAMPLER2D,
        ident::ENV_IRRADIANCE_SAMPLER2D_SPLIT,
    ));
    subs.push((token::T_ENV_IRRADIANCE, ident::ENV_IRRADIANCE_SPLIT));
    subs.push((
        token::T_TEX_SAMPLER_SAMPLER2D,
        ident::TEX_SAMPLER_SAMPLER2D_SPLIT,
    ));
    subs.push((
        token::T_TEX_SAMPLER_SIGNATURE,
        ident::TEX_SAMPLER_SIGNATURE_SPLIT,
    ));
    subs
}

fn replace_tokens(stage: &mut ShaderStage, subs: &[(&str, &str)]) {
    stage.source_code = crate::core::replace_substrings(&stage.source_code, subs);
}

/// Emit library include -- resolve, read and append file content.
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
    // Use add_block_with_includes for recursive #include resolution (C++ addBlock)
    stage.add_block_with_includes(&content, &key, ctx);
    stage.append_line("");
    stage.add_source_dependency(key);
}

/// Emit type definitions for closure/shader types (BSDF, EDF, surfaceshader, etc.)
/// Matches C++ emitTypeDefinitions.
fn emit_type_definitions<G: GlslEmitter>(generator: &G, stage: &mut ShaderStage) {
    let syntax = generator.get_syntax().get_syntax();
    // Collect and sort by key for deterministic output (HashMap iteration is unordered)
    let mut entries: Vec<_> = syntax.iter_type_syntax().collect();
    entries.sort_by_key(|(name, _)| name.as_str());
    for (_name, ts) in entries {
        if !ts.type_definition.is_empty() {
            stage.append_line(&ts.type_definition);
        }
    }
    stage.append_line("");
}

/// Emit constants block (matches C++ emitConstants).
fn emit_constants(syntax: &crate::gen_shader::Syntax, stage: &mut ShaderStage) {
    let constants = stage.get_constant_block();
    if !constants.is_empty() {
        let type_system = &syntax.type_system;
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
        for (ty, var_name, def_val) in vars {
            let tn = glsl_type_name(type_system.get_type(&ty).get_name());
            // For compound GLSL types (vec*, mat*) wrap the scalar zero in a constructor.
            // e.g. vec2 -> vec2(0.0), mat3 -> mat3(0.0), float -> 0.0
            let default_owned: String;
            let default = if def_val.is_empty() {
                let needs_ctor = matches!(tn, "vec2" | "vec3" | "vec4" | "mat3" | "mat4");
                if needs_ctor {
                    default_owned = format!("{}(0.0)", tn);
                    &default_owned
                } else {
                    "0.0"
                }
            } else {
                &def_val
            };
            stage.append_line(&format!(
                "{} {} {} = {};",
                CONSTANT_QUALIFIER, tn, var_name, default
            ));
        }
        stage.append_line("");
    }
}

/// Emit uniforms (matches C++ emitUniforms). Skips LIGHT_DATA block.
fn emit_uniforms(
    syntax: &crate::gen_shader::Syntax,
    stage: &mut ShaderStage,
    resource_binding_context: Option<&Rc<RefCell<Box<dyn ResourceBindingContext>>>>,
    essl_mode: bool,
) {
    let type_system = &syntax.type_system;
    let mut uniform_blocks: Vec<_> = stage
        .get_uniform_blocks()
        .iter()
        .filter(|(_bn, b)| !b.is_empty() && b.get_name() != block::LIGHT_DATA)
        .map(|(bn, b)| (bn.clone(), b.clone()))
        .collect();
    uniform_blocks.sort_by(|(a, _), (b, _)| a.cmp(b));
    for (block_name, blk) in uniform_blocks {
        stage.append_line(&format!("// Uniform block: {}", block_name));
        // ESSL: never use resource binding context (flat declarations)
        if !essl_mode {
            if let Some(rbc) = resource_binding_context {
                rbc.borrow_mut().emit_resource_bindings(
                    &blk,
                    stage,
                    UNIFORM_QUALIFIER,
                    &glsl_type_name,
                );
                continue;
            }
        }
        // Flat declarations
        let vars: Vec<_> = blk
            .get_variable_order()
            .iter()
            .filter_map(|n| {
                blk.find(n).map(|v| {
                    (
                        v.get_type().get_name().to_string(),
                        v.get_variable().to_string(),
                    )
                })
            })
            .collect();
        for (ty, var_name) in vars {
            let tn = glsl_type_name(type_system.get_type(&ty).get_name());
            stage.append_line(&format!("{} {} {};", UNIFORM_QUALIFIER, tn, var_name));
        }
        stage.append_line("");
    }
}

/// Emit specular environment library includes (by ref emitSpecularEnvironment).
fn emit_specular_environment<C: ShaderGraphCreateContext + ShaderImplContext>(
    options: &GenOptions,
    ctx: &C,
    stage: &mut ShaderStage,
) {
    use crate::gen_shader::HwSpecularEnvironmentMethod;
    match options.hw_specular_environment_method {
        HwSpecularEnvironmentMethod::Fis => {
            emit_library_include(LIB_ENV_FIS, ctx, stage);
        }
        HwSpecularEnvironmentMethod::Prefilter => {
            emit_library_include(LIB_ENV_PREFILTER, ctx, stage);
        }
        HwSpecularEnvironmentMethod::None => {
            emit_library_include(LIB_ENV_NONE, ctx, stage);
        }
    }
    stage.append_line("");
}

/// Emit transmission render library includes (by ref emitTransmissionRender).
fn emit_transmission_render<C: ShaderGraphCreateContext + ShaderImplContext>(
    options: &GenOptions,
    ctx: &C,
    stage: &mut ShaderStage,
) {
    use crate::gen_shader::HwTransmissionRenderMethod;
    match options.hw_transmission_render_method {
        HwTransmissionRenderMethod::Refraction => {
            emit_library_include(LIB_TRANSMISSION_REFRACT, ctx, stage);
        }
        HwTransmissionRenderMethod::Opacity => {
            emit_library_include(LIB_TRANSMISSION_OPACITY, ctx, stage);
        }
    }
    stage.append_line("");
}

/// Emit light data struct + uniform block (by ref emitLightData).
fn emit_light_data(
    stage: &mut ShaderStage,
    resource_binding_context: Option<&Rc<RefCell<Box<dyn ResourceBindingContext>>>>,
    max_lights: u32,
) {
    // Clone light data block to avoid borrow issues with stage
    let light_data = match stage.get_uniform_block(block::LIGHT_DATA) {
        Some(ld) if !ld.is_empty() => ld.clone(),
        _ => return,
    };
    let struct_array_suffix = format!("[{}]", max_lights.max(1));
    let struct_instance = light_data.get_instance().to_string();

    if let Some(rbc) = resource_binding_context {
        rbc.borrow_mut().emit_structured_resource_bindings(
            &light_data,
            stage,
            &struct_instance,
            &struct_array_suffix,
            UNIFORM_QUALIFIER,
            &glsl_type_name,
        );
    } else {
        // Flat struct + uniform array declaration
        stage.append_line(&format!("struct {}", light_data.get_name()));
        stage.append_line("{");
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
        for (ty, var_name) in vars {
            let tn = glsl_type_name(&ty);
            stage.append_line(&format!("    {} {};", tn, var_name));
        }
        stage.append_line("};");
        stage.append_line("");
        stage.append_line(&format!(
            "uniform {} {}{};",
            light_data.get_name(),
            struct_instance,
            struct_array_suffix
        ));
    }
    stage.append_line("");
}

/// Emit vertex inputs (matches C++ emitInputs for vertex stage).
fn emit_vertex_inputs(
    stage: &mut ShaderStage,
    syntax: &crate::gen_shader::Syntax,
    flags: &EmitFlags,
) {
    if let Some(vertex_inputs) = stage.get_input_block(block::VERTEX_INPUTS) {
        if !vertex_inputs.is_empty() {
            let type_system = &syntax.type_system;
            let vars: Vec<_> = vertex_inputs
                .get_variable_order()
                .iter()
                .filter_map(|n| {
                    vertex_inputs.find(n).map(|v| {
                        (
                            v.get_type().get_name().to_string(),
                            v.get_variable().to_string(),
                        )
                    })
                })
                .collect();
            stage.append_line(&format!("// Inputs block: {}", block::VERTEX_INPUTS));
            for (i, (ty, var_name)) in vars.iter().enumerate() {
                let tn = glsl_type_name(type_system.get_type(ty).get_name());
                if flags.vk_mode {
                    // Vulkan: layout(location=N)
                    stage.append_line(&format!(
                        "layout (location = {}) {} {} {};",
                        i, INPUT_QUALIFIER, tn, var_name
                    ));
                } else {
                    stage.append_line(&format!("{} {} {};", INPUT_QUALIFIER, tn, var_name));
                }
            }
            stage.append_line("");
        }
    }
}

/// Emit vertex data output block (matches C++ emitOutputs for vertex stage).
fn emit_vertex_data_output(
    stage: &mut ShaderStage,
    syntax: &crate::gen_shader::Syntax,
    flags: &EmitFlags,
) {
    if let Some(vertex_data) = stage.get_output_block(block::VERTEX_DATA) {
        if !vertex_data.is_empty() {
            let type_system = &syntax.type_system;
            let vars: Vec<_> = vertex_data
                .get_variable_order()
                .iter()
                .filter_map(|n| {
                    vertex_data.find(n).map(|v| {
                        (
                            v.get_type().get_name().to_string(),
                            v.get_variable().to_string(),
                        )
                    })
                })
                .collect();
            if flags.essl_mode {
                // ESSL: flat varying declarations (no interface block)
                for (ty, var_name) in &vars {
                    let tn = glsl_type_name(type_system.get_type(ty).get_name());
                    stage.append_line(&format!("{} {} {};", OUTPUT_QUALIFIER, tn, var_name));
                }
            } else if flags.vk_mode {
                // Vulkan: layout(location=N) out VertexData { ... } instance;
                stage.append_line(&format!(
                    "layout (location = {}) {} {} ",
                    flags.vk_vertex_data_location,
                    OUTPUT_QUALIFIER,
                    block::VERTEX_DATA
                ));
                stage.append_line("{");
                for (ty, var_name) in &vars {
                    let tn = glsl_type_name(type_system.get_type(ty).get_name());
                    stage.append_line(&format!("    {} {};", tn, var_name));
                }
                stage.append_line(&format!("}} {};", ident::VERTEX_DATA_INSTANCE));
            } else {
                // GLSL: out VertexData { ... } instance;
                stage.append_line(&format!("out {} ", block::VERTEX_DATA));
                stage.append_line("{");
                for (ty, var_name) in &vars {
                    let tn = glsl_type_name(type_system.get_type(ty).get_name());
                    stage.append_line(&format!("    {} {};", tn, var_name));
                }
                stage.append_line(&format!("}} {};", ident::VERTEX_DATA_INSTANCE));
            }
            stage.append_line("");
        }
    }
}

/// Emit vertex data input block in pixel stage (matches C++ emitInputs pixel stage).
fn emit_vertex_data_input(
    stage: &mut ShaderStage,
    syntax: &crate::gen_shader::Syntax,
    flags: &EmitFlags,
) {
    if let Some(vertex_data) = stage.get_input_block(block::VERTEX_DATA) {
        if !vertex_data.is_empty() {
            let type_system = &syntax.type_system;
            let vars: Vec<_> = vertex_data
                .get_variable_order()
                .iter()
                .filter_map(|n| {
                    vertex_data.find(n).map(|v| {
                        (
                            v.get_type().get_name().to_string(),
                            v.get_variable().to_string(),
                        )
                    })
                })
                .collect();
            if flags.essl_mode {
                // ESSL: flat varying declarations (no interface block)
                for (ty, var_name) in &vars {
                    let tn = glsl_type_name(type_system.get_type(ty).get_name());
                    stage.append_line(&format!("{} {} {};", INPUT_QUALIFIER, tn, var_name));
                }
            } else if flags.vk_mode {
                // Vulkan: layout(location=N) in VertexData { ... } instance;
                stage.append_line(&format!(
                    "layout (location = {}) {} {} ",
                    flags.vk_vertex_data_location,
                    INPUT_QUALIFIER,
                    block::VERTEX_DATA
                ));
                stage.append_line("{");
                for (ty, var_name) in &vars {
                    let tn = glsl_type_name(type_system.get_type(ty).get_name());
                    stage.append_line(&format!("    {} {};", tn, var_name));
                }
                stage.append_line(&format!("}} {};", ident::VERTEX_DATA_INSTANCE));
            } else {
                // GLSL: in VertexData { ... } instance;
                stage.append_line(&format!("in {} ", block::VERTEX_DATA));
                stage.append_line("{");
                for (ty, var_name) in &vars {
                    let tn = glsl_type_name(type_system.get_type(ty).get_name());
                    stage.append_line(&format!("    {} {};", tn, var_name));
                }
                stage.append_line(&format!("}} {};", ident::VERTEX_DATA_INSTANCE));
            }
            stage.append_line("");
        }
    }
}

/// Emit pixel shader outputs (matches C++ emitOutputs pixel stage).
fn emit_pixel_outputs(
    stage: &mut ShaderStage,
    syntax: &crate::gen_shader::Syntax,
    flags: &EmitFlags,
) {
    if let Some(outputs) = stage.get_output_block(block::PIXEL_OUTPUTS) {
        if !outputs.is_empty() {
            let type_system = &syntax.type_system;
            let vars: Vec<_> = outputs
                .get_variable_order()
                .iter()
                .filter_map(|n| {
                    outputs.find(n).map(|v| {
                        (
                            v.get_type().get_name().to_string(),
                            v.get_variable().to_string(),
                        )
                    })
                })
                .collect();
            stage.append_line("// Pixel shader outputs");
            for (i, (ty, var_name)) in vars.iter().enumerate() {
                let tn = glsl_type_name(type_system.get_type(ty).get_name());
                if flags.vk_mode {
                    // Vulkan: layout(location=N) out type name;
                    stage.append_line(&format!(
                        "layout (location = {}) {} {} {};",
                        i, OUTPUT_QUALIFIER, tn, var_name
                    ));
                } else {
                    stage.append_line(&format!("{} {} {};", OUTPUT_QUALIFIER, tn, var_name));
                }
            }
            stage.append_line("");
        }
    }
}

fn emit_vertex_stage<C: ShaderGraphCreateContext + ShaderImplContext, G: GlslEmitter>(
    graph: &ShaderGraph,
    doc: &Document,
    ctx: &C,
    generator: &G,
    stage: &mut ShaderStage,
    resource_binding_context: Option<&Rc<RefCell<Box<dyn ResourceBindingContext>>>>,
    flags: &EmitFlags,
) {
    let syntax = generator.get_syntax().get_syntax();

    // Directives
    stage.append_line(&format!("#version {}", generator.get_version()));
    stage.append_line("");
    if flags.essl_mode {
        stage.append_line("precision mediump float;");
        stage.append_line("");
    }
    if let Some(rbc) = resource_binding_context {
        rbc.borrow().emit_directives(stage);
    }

    // Constants
    emit_constants(syntax, stage);

    // Uniforms
    emit_uniforms(syntax, stage, resource_binding_context, flags.essl_mode);

    // Vertex inputs
    emit_vertex_inputs(stage, syntax, flags);

    // Vertex data outputs
    emit_vertex_data_output(stage, syntax, flags);

    // Common math
    emit_library_include(LIB_MX_MATH, ctx, stage);

    // Function definitions
    emit_function_definitions(graph, doc, ctx, stage);

    // Main
    stage.append_line("void main()");
    stage.append_line("{");
    stage.append_line(&format!(
        "    vec4 hPositionWorld = {} * vec4({}, 1.0);",
        token::T_WORLD_MATRIX,
        token::T_IN_POSITION
    ));
    stage.append_line(&format!(
        "    gl_Position = {} * hPositionWorld;",
        token::T_VIEW_PROJECTION_MATRIX
    ));

    emit_function_calls(graph, doc, ctx, stage);

    stage.append_line("}");
}

fn emit_pixel_stage<C: ShaderGraphCreateContext + ShaderImplContext, G: GlslEmitter>(
    graph: &ShaderGraph,
    doc: &Document,
    ctx: &C,
    generator: &G,
    stage: &mut ShaderStage,
    resource_binding_context: Option<&Rc<RefCell<Box<dyn ResourceBindingContext>>>>,
    flags: &EmitFlags,
    options: &GenOptions,
) {
    let syntax = generator.get_syntax().get_syntax();

    // Directives
    stage.append_line(&format!("#version {}", generator.get_version()));
    stage.append_line("");
    if flags.essl_mode {
        stage.append_line("precision mediump float;");
        stage.append_line("");
    }
    if let Some(rbc) = resource_binding_context {
        rbc.borrow().emit_directives(stage);
    }

    // Type definitions (struct BSDF, EDF, surfaceshader, etc.)
    emit_type_definitions(generator, stage);

    // Constants
    emit_constants(syntax, stage);

    // Uniforms (skips LIGHT_DATA)
    emit_uniforms(syntax, stage, resource_binding_context, flags.essl_mode);

    // Vertex data inputs
    emit_vertex_data_input(stage, syntax, flags);

    // Pixel outputs
    emit_pixel_outputs(stage, syntax, flags);

    // Common math
    emit_library_include(LIB_MX_MATH, ctx, stage);
    stage.append_line("");

    // Determine lighting
    let lighting = graph.requires_lighting();

    // Directional albedo method #define
    if lighting || options.hw_write_albedo_table || options.hw_write_env_prefilter {
        stage.append_line(&format!(
            "#define DIRECTIONAL_ALBEDO_METHOD {}",
            options.hw_directional_albedo_method as i32
        ));
        stage.append_line("");
    }

    // Airy Fresnel iterations #define
    stage.append_line(&format!(
        "#define AIRY_FRESNEL_ITERATIONS {}",
        options.hw_airy_fresnel_iterations
    ));
    stage.append_line("");

    // Lighting support
    if lighting {
        if options.hw_max_active_light_sources > 0 {
            let max_lights = options.hw_max_active_light_sources.max(1);
            stage.append_line(&format!(
                "#define {} {}",
                ident::LIGHT_DATA_MAX_LIGHT_SOURCES,
                max_lights
            ));
        }
        emit_specular_environment(options, ctx, stage);
        emit_transmission_render(options, ctx, stage);

        if options.hw_max_active_light_sources > 0 {
            emit_light_data(
                stage,
                resource_binding_context,
                options.hw_max_active_light_sources,
            );
        }
    }

    // Shadowing support
    let shadowing = (lighting && options.hw_shadow_map) || options.hw_write_depth_moments;
    if shadowing {
        emit_library_include(LIB_SHADOW, ctx, stage);
        emit_library_include(LIB_SHADOW_PLATFORM, ctx, stage);
    }

    // Albedo table generation
    if options.hw_write_albedo_table {
        emit_library_include(LIB_ALBEDO_TABLE, ctx, stage);
        stage.append_line("");
    }

    // Environment prefilter generation
    if options.hw_write_env_prefilter {
        emit_library_include(LIB_ENV_PREFILTER_GEN, ctx, stage);
        stage.append_line("");
    }

    // File texture UV transform token
    // (set by generator, replaced in token substitution)

    // Light function definitions
    emit_light_function_definitions(graph, doc, ctx, stage);

    // Function definitions for all nodes
    emit_function_definitions(graph, doc, ctx, stage);

    // Main function
    stage.append_line("void main()");
    stage.append_line("{");

    // Check for special output modes
    if graph.has_classification_closure() && !graph.has_classification_shader() {
        // Direct closure: output black
        if let Some(output_socket) = graph.get_output_socket_at(0) {
            let out_var = output_socket.port.get_variable();
            stage.append_line(&format!("    {} = vec4(0.0, 0.0, 0.0, 1.0);", out_var));
        }
    } else if options.hw_write_depth_moments {
        if let Some(output_socket) = graph.get_output_socket_at(0) {
            let out_var = output_socket.port.get_variable();
            stage.append_line(&format!(
                "    {} = vec4(mx_compute_depth_moments(), 0.0, 1.0);",
                out_var
            ));
        }
    } else if options.hw_write_albedo_table {
        if let Some(output_socket) = graph.get_output_socket_at(0) {
            let out_var = output_socket.port.get_variable();
            stage.append_line(&format!(
                "    {} = vec4(mx_generate_dir_albedo_table(), 1.0);",
                out_var
            ));
        }
    } else if options.hw_write_env_prefilter {
        if let Some(output_socket) = graph.get_output_socket_at(0) {
            let out_var = output_socket.port.get_variable();
            stage.append_line(&format!(
                "    {} = vec4(mx_generate_prefilter_env(), 1.0);",
                out_var
            ));
        }
    } else {
        // Normal output path
        // Pre-declare closureData for BSDF/EDF/VDF function calls.
        // C++ declares it inside HwSurfaceNode's closure scopes; we pre-declare
        // at function scope so top-level closure function calls can reference it.
        if graph.has_classification_closure() {
            stage.append_line("    ClosureData closureData;");
        }
        emit_function_calls(graph, doc, ctx, stage);

        if let Some(output_socket) = graph.get_output_socket_at(0) {
            let out_var = output_socket.port.get_variable();
            let rhs = if let Some((up_node, up_output)) = output_socket.get_connection() {
                if let Some(up_var) = graph.get_connection_variable(up_node, up_output) {
                    let type_name = output_socket.port.get_type().get_name();
                    if graph.has_classification_surface() {
                        // Surface shader: color + transparency handling
                        let out_color = format!("{}.color", up_var);
                        let out_transparency = format!("{}.transparency", up_var);
                        let final_color = if options.hw_srgb_encode_output {
                            format!("mx_srgb_encode({})", out_color)
                        } else {
                            out_color.clone()
                        };
                        if options.hw_transparency {
                            stage.append_line(&format!(
                                "    float outAlpha = clamp(1.0 - dot({}, vec3(0.3333)), 0.0, 1.0);",
                                out_transparency
                            ));
                            stage.append_line(&format!(
                                "    {} = vec4({}, outAlpha);",
                                out_var, final_color
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
                                "    {} = vec4({}, 1.0);",
                                out_var, final_color
                            ));
                        }
                        // Already emitted, skip default rhs path
                        stage.append_line("}");
                        return;
                    }
                    // Non-surface shader output
                    let out_value = if options.hw_srgb_encode_output
                        && output_socket.port.get_type().is_float3()
                    {
                        format!("mx_srgb_encode({})", up_var)
                    } else {
                        up_var.clone()
                    };
                    match type_name {
                        "color4" | "vector4" => out_value,
                        "color3" | "vector3" => format!("vec4({}, 1.0)", out_value),
                        "vector2" => format!("vec4({}, 0.0, 1.0)", out_value),
                        "float" | "integer" | "boolean" => {
                            format!("vec4({}, 0.0, 0.0, 1.0)", out_value)
                        }
                        "surfaceshader" | "material" => out_value,
                        _ => format!("vec4({}, 0.0, 0.0, 1.0)", out_value),
                    }
                } else {
                    "vec4(0.0, 0.0, 0.0, 1.0)".to_string()
                }
            } else {
                "vec4(0.0, 0.0, 0.0, 1.0)".to_string()
            };
            stage.append_line(&format!("    {} = {};", out_var, rhs));
        }
    }
    stage.append_line("}");
}

/// Emit light function definitions (by ref emitLightFunctionDefinitions).
/// In C++, HwNumLightsNode and HwLightSamplerNode are "internal" nodes created
/// by HwShaderGenerator::generate(). Their function definitions are emitted here.
fn emit_light_function_definitions<C: ShaderGraphCreateContext + ShaderImplContext>(
    graph: &ShaderGraph,
    _doc: &Document,
    ctx: &C,
    stage: &mut ShaderStage,
) {
    if !graph.requires_lighting() {
        return;
    }
    let options = ctx.get_options();
    if options.hw_max_active_light_sources == 0 {
        return;
    }
    if !graph.has_classification_surface() {
        return;
    }

    // numActiveLightSources() — wraps the uniform with a max clamp.
    // C++: HwNumLightsNode::emitFunctionDefinition
    stage.append_line("int numActiveLightSources()");
    stage.append_line("{");
    stage.append_line(&format!(
        "    return min({}, {});",
        ident::NUM_ACTIVE_LIGHT_SOURCES,
        ident::LIGHT_DATA_MAX_LIGHT_SOURCES,
    ));
    stage.append_line("}");
    stage.append_line("");

    // sampleLightSource() — dispatches to bound light type implementations.
    // C++: HwLightSamplerNode::emitFunctionDefinition builds an if/else chain
    // for each bound light type. With no lights bound, the body is empty.
    stage.append_line(
        "void sampleLightSource(LightData light, vec3 position, out lightshader result)",
    );
    stage.append_line("{");
    stage.append_line("    result.intensity = vec3(0.0);");
    stage.append_line("    result.direction = vec3(0.0);");
    stage.append_line("}");
    stage.append_line("");
}

/// Emit function definitions for all nodes. Public for reuse by Slang emit.
pub fn emit_function_definitions<C: ShaderGraphCreateContext + ShaderImplContext>(
    graph: &ShaderGraph,
    doc: &Document,
    ctx: &C,
    stage: &mut ShaderStage,
) {
    let target = ctx.get_implementation_target();
    for node_name in &graph.node_order {
        let node = match graph.get_node(node_name) {
            Some(n) => n,
            None => continue,
        };
        let node_def_name = match graph.get_node_def(node_name) {
            Some(nd) => nd,
            None => continue,
        };
        let impl_: Box<dyn ShaderNodeImpl> =
            match ctx.get_implementation_for_nodedef(doc, node_def_name, target) {
                Some(i) => i,
                None => continue,
            };
        impl_.emit_function_definition(node, ctx, stage);
    }
}

/// Emit function calls for all nodes. Public for reuse by Slang emit.
pub fn emit_function_calls<C: ShaderGraphCreateContext + ShaderImplContext>(
    graph: &ShaderGraph,
    doc: &Document,
    ctx: &C,
    stage: &mut ShaderStage,
) {
    let target = ctx.get_implementation_target();
    for node_name in &graph.node_order {
        let node = match graph.get_node(node_name) {
            Some(n) => n,
            None => continue,
        };
        let node_def_name = match graph.get_node_def(node_name) {
            Some(nd) => nd,
            None => continue,
        };
        let impl_: Box<dyn ShaderNodeImpl> =
            match ctx.get_implementation_for_nodedef(doc, node_def_name, target) {
                Some(i) => i,
                None => continue,
            };
        impl_.emit_function_call(node, ctx, stage);
    }
}

// ============================================================================
// Generator-specific emit_shader functions
// ============================================================================

pub fn emit_shader(
    name: &str,
    graph: ShaderGraph,
    mut stages: Vec<ShaderStage>,
    doc: &Document,
    context: &GenContext<GlslShaderGenerator>,
    generator: &GlslShaderGenerator,
) -> crate::gen_shader::Shader {
    let mut subs = token_substitutions();
    let emit_ctx = GlslShaderGraphContext::with_graph_and_doc(context, &graph, doc);
    let rbc = context.get_resource_binding_context();
    let options = context.get_options();
    let flags = EmitFlags::default();

    // File texture UV transform token substitution
    let file_transform_uv = if options.file_texture_vertical_flip {
        "mx_transform_uv_vflip.glsl"
    } else {
        "mx_transform_uv.glsl"
    };
    subs.push((T_FILE_TRANSFORM_UV, file_transform_uv));

    let vs_idx = stages
        .iter()
        .position(|s| s.name == crate::gen_shader::shader_stage::VERTEX);
    let ps_idx = stages
        .iter()
        .position(|s| s.name == crate::gen_shader::shader_stage::PIXEL);

    if let Some(idx) = vs_idx {
        emit_vertex_stage(
            &graph,
            doc,
            &emit_ctx,
            generator,
            &mut stages[idx],
            rbc.as_ref(),
            &flags,
        );
        replace_tokens(&mut stages[idx], &subs);
    }
    if let Some(idx) = ps_idx {
        emit_pixel_stage(
            &graph,
            doc,
            &emit_ctx,
            generator,
            &mut stages[idx],
            rbc.as_ref(),
            &flags,
            options,
        );
        replace_tokens(&mut stages[idx], &subs);
    }

    crate::gen_shader::Shader::from_parts(name, graph, stages)
}

impl GlslEmitter for GlslShaderGenerator {
    fn get_version(&self) -> &str {
        super::glsl_shader_generator::VERSION
    }
    fn get_syntax(&self) -> &super::GlslSyntax {
        self.get_syntax()
    }
}

/// Emit shader for ESSL target (WebGL 2).
pub fn emit_shader_essl(
    name: &str,
    graph: ShaderGraph,
    mut stages: Vec<ShaderStage>,
    doc: &Document,
    context: &GenContext<super::EsslShaderGenerator>,
    generator: &super::EsslShaderGenerator,
) -> crate::gen_shader::Shader {
    let mut subs = token_substitutions();
    let emit_ctx = super::EsslShaderGraphContext::with_graph_and_doc(context, &graph, doc);
    // ESSL does not support resource binding context (C++ throws ExceptionShaderGenError)
    if context.get_resource_binding_context().is_some() {
        eprintln!("Warning: The EsslShaderGenerator does not support resource binding.");
    }
    let options = context.get_options();
    let flags = EmitFlags {
        essl_mode: true,
        ..Default::default()
    };

    let file_transform_uv = if options.file_texture_vertical_flip {
        "mx_transform_uv_vflip.glsl"
    } else {
        "mx_transform_uv.glsl"
    };
    subs.push((T_FILE_TRANSFORM_UV, file_transform_uv));

    let vs_idx = stages
        .iter()
        .position(|s| s.name == crate::gen_shader::shader_stage::VERTEX);
    let ps_idx = stages
        .iter()
        .position(|s| s.name == crate::gen_shader::shader_stage::PIXEL);

    // ESSL: never pass resource binding context
    if let Some(idx) = vs_idx {
        emit_vertex_stage(
            &graph,
            doc,
            &emit_ctx,
            generator,
            &mut stages[idx],
            None,
            &flags,
        );
        replace_tokens(&mut stages[idx], &subs);
    }
    if let Some(idx) = ps_idx {
        emit_pixel_stage(
            &graph,
            doc,
            &emit_ctx,
            generator,
            &mut stages[idx],
            None,
            &flags,
            options,
        );
        replace_tokens(&mut stages[idx], &subs);
    }

    crate::gen_shader::Shader::from_parts(name, graph, stages)
}

impl GlslEmitter for super::EsslShaderGenerator {
    fn get_version(&self) -> &str {
        super::essl_shader_generator::VERSION
    }
    fn get_syntax(&self) -> &super::GlslSyntax {
        self.get_syntax()
    }
}

/// Emit shader for Vulkan GLSL target (#version 450).
pub fn emit_shader_vk(
    name: &str,
    graph: ShaderGraph,
    mut stages: Vec<ShaderStage>,
    doc: &Document,
    context: &GenContext<super::VkShaderGenerator>,
    generator: &super::VkShaderGenerator,
) -> crate::gen_shader::Shader {
    let mut subs = token_substitutions();
    let emit_ctx = super::VkShaderGraphContext::with_graph_and_doc(context, &graph, doc);
    let rbc = generator.get_resource_binding_context_internal(context);
    let options = context.get_options();
    let flags = EmitFlags {
        vk_mode: true,
        vk_vertex_data_location: generator.get_vertex_data_location(),
        ..Default::default()
    };

    let file_transform_uv = if options.file_texture_vertical_flip {
        "mx_transform_uv_vflip.glsl"
    } else {
        "mx_transform_uv.glsl"
    };
    subs.push((T_FILE_TRANSFORM_UV, file_transform_uv));

    let vs_idx = stages
        .iter()
        .position(|s| s.name == crate::gen_shader::shader_stage::VERTEX);
    let ps_idx = stages
        .iter()
        .position(|s| s.name == crate::gen_shader::shader_stage::PIXEL);

    if let Some(idx) = vs_idx {
        emit_vertex_stage(
            &graph,
            doc,
            &emit_ctx,
            generator,
            &mut stages[idx],
            rbc.as_ref(),
            &flags,
        );
        replace_tokens(&mut stages[idx], &subs);
    }
    if let Some(idx) = ps_idx {
        emit_pixel_stage(
            &graph,
            doc,
            &emit_ctx,
            generator,
            &mut stages[idx],
            rbc.as_ref(),
            &flags,
            options,
        );
        replace_tokens(&mut stages[idx], &subs);
    }

    crate::gen_shader::Shader::from_parts(name, graph, stages)
}

impl GlslEmitter for super::VkShaderGenerator {
    fn get_version(&self) -> &str {
        super::vk_shader_generator::VERSION
    }
    fn get_syntax(&self) -> &super::GlslSyntax {
        self.get_syntax()
    }
}

/// Emit shader for WGSL target (#version 450, texture+sampler split for WebGPU).
pub fn emit_shader_wgsl(
    name: &str,
    graph: ShaderGraph,
    mut stages: Vec<ShaderStage>,
    doc: &Document,
    context: &GenContext<super::WgslShaderGenerator>,
    generator: &super::WgslShaderGenerator,
) -> crate::gen_shader::Shader {
    let mut subs = wgsl_token_substitutions();
    let emit_ctx = super::wgsl_shader_generator::WgslShaderGraphContext::with_graph_and_doc(
        context, &graph, doc,
    );
    let rbc = context.get_resource_binding_context();
    let options = context.get_options();
    let flags = EmitFlags {
        vk_mode: true,
        ..Default::default()
    };

    let file_transform_uv = if options.file_texture_vertical_flip {
        "mx_transform_uv_vflip.glsl"
    } else {
        "mx_transform_uv.glsl"
    };
    subs.push((T_FILE_TRANSFORM_UV, file_transform_uv));

    let vs_idx = stages
        .iter()
        .position(|s| s.name == crate::gen_shader::shader_stage::VERTEX);
    let ps_idx = stages
        .iter()
        .position(|s| s.name == crate::gen_shader::shader_stage::PIXEL);

    if let Some(idx) = vs_idx {
        emit_vertex_stage(
            &graph,
            doc,
            &emit_ctx,
            generator,
            &mut stages[idx],
            rbc.as_ref(),
            &flags,
        );
        replace_tokens(&mut stages[idx], &subs);
    }
    if let Some(idx) = ps_idx {
        emit_pixel_stage(
            &graph,
            doc,
            &emit_ctx,
            generator,
            &mut stages[idx],
            rbc.as_ref(),
            &flags,
            options,
        );
        replace_tokens(&mut stages[idx], &subs);
    }

    crate::gen_shader::Shader::from_parts(name, graph, stages)
}

impl GlslEmitter for super::WgslShaderGenerator {
    fn get_version(&self) -> &str {
        super::wgsl_shader_generator::VERSION
    }
    fn get_syntax(&self) -> &super::GlslSyntax {
        self.get_syntax()
    }
}
