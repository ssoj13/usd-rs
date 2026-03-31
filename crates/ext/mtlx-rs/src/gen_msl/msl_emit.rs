//! MSL emit -- Metal vertex/fragment stages (ref: MaterialXGenMsl MslShaderGenerator.cpp).

use crate::core::Document;
use crate::format::read_file;
use crate::gen_hw::{hw_block as block, hw_ident as ident, hw_token as token};
use crate::gen_shader::{
    GenContext, HwSpecularEnvironmentMethod, HwTransmissionRenderMethod, ResourceBindingContext,
    ShaderGraph, ShaderGraphCreateContext, ShaderImplContext, ShaderNodeClassification,
    ShaderNodeImpl, ShaderStage, VariableBlock,
};
use std::cell::RefCell;
use std::rc::Rc;

use super::msl_shader_generator::{MslShaderGenerator, MslShaderGraphContext, VERSION};
use super::msl_syntax::{FLAT_QUALIFIER, UNIFORM_QUALIFIER};

/// Stdlib mx_math.metal
const LIB_MX_MATH: &str = "stdlib/genmsl/lib/mx_math.metal";
/// Stdlib mx_texture.metal (MetalTexture, texture())
const LIB_MX_TEXTURE: &str = "stdlib/genmsl/lib/mx_texture.metal";
/// Stdlib mx_matscalaroperators.metal (ref: emitMathMatrixScalarMathOperators)
const LIB_MX_MATSCALAROPS: &str = "stdlib/genmsl/lib/mx_matscalaroperators.metal";

/// Token for file UV transform (ref: ShaderGenerator::T_FILE_TRANSFORM_UV).
const T_FILE_TRANSFORM_UV: &str = "$fileTransformUv";

/// Texture name helper: var + "_tex" (ref: C++ TEXTURE_NAME macro)
fn texture_name(var: &str) -> String {
    format!("{}_tex", var)
}

/// Sampler name helper: var + "_sampler" (ref: C++ SAMPLER_NAME macro)
fn sampler_name(var: &str) -> String {
    format!("{}_sampler", var)
}

fn msl_type_name(mtlx: &str) -> &'static str {
    match mtlx {
        "float" => "float",
        "integer" => "int",
        "boolean" => "bool",
        "vector2" => "float2",
        "vector3" => "float3",
        "vector4" => "float4",
        "color3" => "float3",
        "color4" => "float4",
        "matrix33" => "float3x3",
        "matrix44" => "float4x4",
        "filename" => "MetalTexture",
        "string" => "int",
        "surfaceshader" | "material" => "float4",
        _ => "float",
    }
}

/// MSL token substitutions (tokens from HwConstants -> MSL identifiers)
fn msl_token_substitutions() -> Vec<(&'static str, &'static str)> {
    vec![
        (token::T_IN_POSITION, ident::IN_POSITION),
        (token::T_IN_NORMAL, ident::IN_NORMAL),
        (token::T_IN_TANGENT, ident::IN_TANGENT),
        (token::T_IN_BITANGENT, ident::IN_BITANGENT),
        (token::T_IN_TEXCOORD, ident::IN_TEXCOORD),
        (token::T_IN_GEOMPROP, ident::IN_GEOMPROP),
        (token::T_IN_COLOR, ident::IN_COLOR),
        (token::T_POSITION_WORLD, ident::POSITION_WORLD),
        (token::T_NORMAL_WORLD, ident::NORMAL_WORLD),
        (token::T_NORMAL_OBJECT, ident::NORMAL_OBJECT),
        (token::T_TANGENT_WORLD, ident::TANGENT_WORLD),
        (token::T_TANGENT_OBJECT, ident::TANGENT_OBJECT),
        (token::T_BITANGENT_WORLD, ident::BITANGENT_WORLD),
        (token::T_BITANGENT_OBJECT, ident::BITANGENT_OBJECT),
        (token::T_POSITION_OBJECT, ident::POSITION_OBJECT),
        (token::T_TEXCOORD, ident::TEXCOORD),
        (token::T_COLOR, ident::COLOR),
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
        (token::T_VIEW_POSITION, ident::VIEW_POSITION),
        (
            token::T_VIEW_PROJECTION_MATRIX,
            ident::VIEW_PROJECTION_MATRIX,
        ),
        (token::T_FRAME, ident::FRAME),
        (token::T_TIME, ident::TIME),
        (token::T_GEOMPROP, ident::GEOMPROP),
        (token::T_ALPHA_THRESHOLD, ident::ALPHA_THRESHOLD),
        (token::T_AMB_OCC_MAP, ident::AMB_OCC_MAP),
        (token::T_AMB_OCC_GAIN, ident::AMB_OCC_GAIN),
        (token::T_SHADOW_MAP, ident::SHADOW_MAP),
        (token::T_SHADOW_MATRIX, ident::SHADOW_MATRIX),
        (token::T_VERTEX_DATA_INSTANCE, ident::VERTEX_DATA_INSTANCE),
        (token::T_LIGHT_DATA_INSTANCE, ident::LIGHT_DATA_INSTANCE),
        // MSL texture: MetalTexture struct (from mx_texture.metal)
        (token::T_TEX_SAMPLER_SAMPLER2D, ident::TEX_SAMPLER_SAMPLER2D),
        (token::T_TEX_SAMPLER_SIGNATURE, "MetalTexture tex_sampler"),
        // Closure data constructor (ref: MslShaderGenerator constructor)
        (
            token::T_CLOSURE_DATA_CONSTRUCTOR,
            "{closureType, L, V, N, P, occlusion}",
        ),
    ]
}

fn replace_tokens(stage: &mut ShaderStage, subs: &[(&str, &str)]) {
    stage.source_code = crate::core::replace_substrings(&stage.source_code, subs);
}

/// Check if character is allowed before a token in MetalizeGeneratedShader.
fn is_allowed_before_token(ch: u8) -> bool {
    ch.is_ascii_whitespace() || ch == b'(' || ch == b',' || ch == b'+' || ch == b'-'
}

/// Check if character is allowed after a token in MetalizeGeneratedShader.
fn is_allowed_after_token(ch: u8) -> bool {
    ch.is_ascii_whitespace() || ch == b'(' || ch == b')' || ch == b','
}

/// Translate GLSL syntax to MSL with word-boundary-aware replacement.
/// (ref: MslShaderGenerator::MetalizeGeneratedShader in MslShaderGenerator.cpp)
fn metalize_generated_shader(source: &str) -> String {
    let mut s = source.to_string();
    // Phase 1: Convert out/inout parameters to thread references
    // (ref: C++ lines 183-236)
    for keyword in &["out", "inout"] {
        let mut pos = 0usize;
        loop {
            let search_start = pos;
            let found = match s[search_start..].find(keyword) {
                Some(f) => f,
                None => break,
            };
            let abs_pos = search_start + found;

            if abs_pos == 0 {
                pos = abs_pos + keyword.len();
                continue;
            }
            let sb = s.as_bytes();
            if abs_pos + keyword.len() >= sb.len() {
                break;
            }
            let preceding = sb[abs_pos - 1];
            let succeeding = sb[abs_pos + keyword.len()];

            let is_out_keyword =
                (preceding == b'(' || preceding == b',' || preceding.is_ascii_whitespace())
                    && succeeding.is_ascii_whitespace()
                    && succeeding != b'\n';

            if !is_out_keyword {
                pos = abs_pos + keyword.len();
                continue;
            }

            // Parse type name after keyword
            let mut tp = abs_pos + keyword.len();
            let sb = s.as_bytes();
            while tp < sb.len() && sb[tp].is_ascii_whitespace() {
                tp += 1;
            }
            let typename_beg = tp;
            while tp < sb.len() && !sb[tp].is_ascii_whitespace() {
                tp += 1;
            }
            let typename_end = tp;
            let type_name = s[typename_beg..typename_end].to_string();

            // Parse variable name
            while tp < sb.len() && sb[tp].is_ascii_whitespace() {
                tp += 1;
            }
            let varname_beg = tp;
            while tp < sb.len() {
                let ch = sb[tp];
                if ch.is_ascii_whitespace() || ch == b'\n' || ch == b',' || ch == b')' {
                    break;
                }
                tp += 1;
            }
            let var_name = s[varname_beg..tp].to_string();

            // Check if var is array (no & for arrays)
            let replacement = if var_name.contains('[') {
                format!("thread {}", type_name)
            } else {
                format!("thread {} &", type_name)
            };

            // Replace from keyword position through end of type name
            s.replace_range(abs_pos..typename_end, &replacement);
            pos = abs_pos + replacement.len();
        }
    }

    // Phase 2: Word-boundary-aware token replacement for type/function renames
    let replace_tokens: &[(&str, &str)] = &[
        ("sampler2D", "MetalTexture"),
        ("dFdy", "dfdy"),
        ("dFdx", "dfdx"),
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

    for &(from, to) in replace_tokens {
        let mut pos = 0;
        loop {
            let found = match s[pos..].find(from) {
                Some(f) => f,
                None => break,
            };
            let abs_pos = pos + found;
            let sb = s.as_bytes();

            let before_ok = if abs_pos == 0 {
                true
            } else {
                is_allowed_before_token(sb[abs_pos - 1])
            };

            let after_pos = abs_pos + from.len();
            let after_ok = if after_pos >= sb.len() {
                true
            } else {
                is_allowed_after_token(sb[after_pos])
            };

            if before_ok && after_ok {
                s.replace_range(abs_pos..after_pos, to);
                pos = abs_pos + to.len();
            } else {
                pos = abs_pos + from.len();
            }
        }
    }

    s
}

fn emit_library_include<C: ShaderGraphCreateContext + ShaderImplContext>(
    filename: &str,
    ctx: &C,
    stage: &mut ShaderStage,
    translate: bool,
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
    let mut content = read_file(&resolved);
    if content.is_empty() {
        return;
    }
    if translate {
        content = metalize_generated_shader(&content);
    }
    stage.append_source_code(&content);
    stage.append_line("");
    stage.add_source_dependency(key);
}

fn emit_function_definitions<C: ShaderGraphCreateContext + ShaderImplContext>(
    graph: &ShaderGraph,
    doc: &Document,
    ctx: &C,
    stage: &mut ShaderStage,
    translate_source: bool,
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
    if translate_source {
        stage.source_code = metalize_generated_shader(&stage.source_code);
    }
}

fn emit_function_calls<C: ShaderGraphCreateContext + ShaderImplContext>(
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

/// Emit function calls for nodes matching a given classification mask.
fn emit_function_calls_classified<C: ShaderGraphCreateContext + ShaderImplContext>(
    graph: &ShaderGraph,
    doc: &Document,
    ctx: &C,
    stage: &mut ShaderStage,
    classification: u32,
) {
    let target = ctx.get_implementation_target();
    for node_name in &graph.node_order {
        let node = match graph.get_node(node_name) {
            Some(n) => n,
            None => continue,
        };
        if !node.has_classification(classification) {
            continue;
        }
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

/// Emit a single node's function call by name.
fn emit_single_function_call<C: ShaderGraphCreateContext + ShaderImplContext>(
    graph: &ShaderGraph,
    doc: &Document,
    ctx: &C,
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
    if let Some(impl_) = ctx.get_implementation_for_nodedef(doc, node_def_name, target) {
        impl_.emit_function_call(node, ctx, stage);
    }
}

// ---- Directives ---------------------------------------------------------

/// Emit Metal directives (ref: MslShaderGenerator::emitDirectives).
fn emit_directives(stage: &mut ShaderStage) {
    stage.append_line(&format!("//Metal Shading Language version {}", VERSION));
    stage.append_line("#define __METAL__ ");
    stage.append_line("#include <metal_stdlib>");
    stage.append_line("#include <simd/simd.h>");
    stage.append_line("using namespace metal;");
    stage.append_line("");
}

// ---- Constants ----------------------------------------------------------

/// Emit constant variable declarations (ref: MslShaderGenerator::emitConstants).
fn emit_constants(stage: &mut ShaderStage) {
    let constants = stage.get_constant_block();
    if constants.is_empty() {
        return;
    }
    // Clone data to avoid borrow conflict
    let vars: Vec<(String, String, String)> = constants
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
    for (ty, var_name, def_val) in &vars {
        let tn = msl_type_name(ty);
        let default = if def_val.is_empty() {
            "0.0"
        } else {
            def_val.as_str()
        };
        stage.append_line(&format!(
            "{} {} {} = {};",
            UNIFORM_QUALIFIER, tn, var_name, default
        ));
    }
    stage.append_line("");
}

// ---- Constant Buffer Declarations ----------------------------------------

/// Emit uniform block struct declarations (ref: MslShaderGenerator::emitConstantBufferDeclarations).
fn emit_constant_buffer_declarations(
    stage: &mut ShaderStage,
    resource_binding_context: Option<&Rc<RefCell<Box<dyn ResourceBindingContext>>>>,
) {
    // Clone block data to avoid borrow conflict
    let uniform_blocks: Vec<(String, VariableBlock)> = stage
        .get_uniform_blocks()
        .iter()
        .filter(|(bn, b)| bn.as_str() != block::LIGHT_DATA && !b.is_empty())
        .map(|(bn, b)| (bn.clone(), b.clone()))
        .collect();

    for (block_name, blk) in &uniform_blocks {
        stage.append_line(&format!("// Uniform block: {}", block_name));
        if let Some(rbc) = resource_binding_context {
            rbc.borrow_mut()
                .emit_resource_bindings(blk, stage, UNIFORM_QUALIFIER, &msl_type_name);
        } else {
            let vars: Vec<(String, String)> = blk
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
            for (ty, var_name) in &vars {
                let tn = msl_type_name(ty);
                stage.append_line(&format!("{} {} {};", UNIFORM_QUALIFIER, tn, var_name));
            }
            stage.append_line("");
        }
    }
}

// ---- Inputs -------------------------------------------------------------

/// Collected variable info for rendering input/output structs.
struct VarInfo {
    type_name: String,
    var_name: String,
    port_name: String,
}

/// Emit an input struct with proper MSL attributes.
fn emit_inputs_block(
    stage: &mut ShaderStage,
    block_name: &str,
    vars: &[VarInfo],
    is_vertex_stage: bool,
) {
    stage.append_line(&format!("// Inputs block: {}", block_name));
    stage.append_line(&format!("struct {}", block_name));
    stage.append_line("{");

    if !is_vertex_stage {
        stage.append_line("    float4 pos [[position]];");
    }

    for (i, v) in vars.iter().enumerate() {
        let ty = msl_type_name(&v.type_name);
        let mut line = format!("    {} {}", ty, v.port_name);
        if is_vertex_stage {
            line.push_str(&format!(" [[attribute({})]]", i));
        }
        line.push(';');
        stage.append_line(&line);
    }

    stage.append_line("};");
    stage.append_line("");
}

/// Emit inputs for the current stage.
fn emit_inputs(stage: &mut ShaderStage, is_vertex_stage: bool) {
    let block_key = if is_vertex_stage {
        block::VERTEX_INPUTS
    } else {
        block::VERTEX_DATA
    };
    // Collect data before mutating stage
    let data: Option<(String, Vec<VarInfo>)> = stage.get_input_block(block_key).and_then(|blk| {
        if blk.is_empty() {
            return None;
        }
        let name = blk.get_name().to_string();
        let vars: Vec<VarInfo> = blk
            .get_variable_order()
            .iter()
            .filter_map(|n| {
                blk.find(n).map(|v| VarInfo {
                    type_name: v.get_type().get_name().to_string(),
                    var_name: v.get_variable().to_string(),
                    port_name: v.get_name().to_string(),
                })
            })
            .collect();
        Some((name, vars))
    });

    if let Some((name, vars)) = data {
        emit_inputs_block(stage, &name, &vars, is_vertex_stage);
    }
}

// ---- Outputs ------------------------------------------------------------

/// Emit output struct (ref: MslShaderGenerator::emitOutputs).
fn emit_outputs(stage: &mut ShaderStage, is_vertex_stage: bool) {
    let block_key = if is_vertex_stage {
        block::VERTEX_DATA
    } else {
        block::PIXEL_OUTPUTS
    };

    // Clone data before mutating
    let data: Option<(String, Vec<VarInfo>)> = if is_vertex_stage {
        stage.get_output_block(block_key)
    } else {
        stage.get_output_block(block_key)
    }
    .map(|blk| {
        let name = blk.get_name().to_string();
        let vars: Vec<VarInfo> = blk
            .get_variable_order()
            .iter()
            .filter_map(|n| {
                blk.find(n).map(|v| VarInfo {
                    type_name: v.get_type().get_name().to_string(),
                    var_name: v.get_variable().to_string(),
                    port_name: v.get_name().to_string(),
                })
            })
            .collect();
        (name, vars)
    });

    if !is_vertex_stage {
        stage.append_line("// Pixel shader outputs");
    }

    if let Some((name, vars)) = data {
        if !vars.is_empty() {
            stage.append_line(&format!("struct {}", name));
            stage.append_line("{");
            if is_vertex_stage {
                stage.append_line("    float4 pos [[position]];");
            }
            for v in &vars {
                let ty = msl_type_name(&v.type_name);
                let flat_suffix =
                    if v.type_name == "integer" && v.port_name.starts_with("$inGeomprop") {
                        format!(" [[ {} ]]", FLAT_QUALIFIER)
                    } else {
                        String::new()
                    };
                stage.append_line(&format!("    {} {}{};", ty, v.var_name, flat_suffix));
            }
            stage.append_line("};");
            stage.append_line("");
        } else if is_vertex_stage {
            stage.append_line("struct VertexData");
            stage.append_line("{");
            stage.append_line("    float4 pos [[position]];");
            stage.append_line("};");
            stage.append_line("");
        }
    } else if is_vertex_stage {
        stage.append_line("struct VertexData");
        stage.append_line("{");
        stage.append_line("    float4 pos [[position]];");
        stage.append_line("};");
        stage.append_line("");
    }
}

// ---- Variable Declaration ------------------------------------------------

/// Emit a variable declaration with MSL-specific handling.
/// (ref: MslShaderGenerator::emitVariableDeclaration)
fn emit_variable_declaration(
    type_name: &str,
    var_name: &str,
    semantic: &str,
    port_name: &str,
    qualifier: &str,
    assign_value: bool,
    value_str: &str,
) -> String {
    // FILENAME type -> MetalTexture
    if type_name == "filename" {
        let q = if qualifier.is_empty() {
            String::new()
        } else {
            format!("{} ", qualifier)
        };
        return format!("{}MetalTexture {}", q, var_name);
    }

    let mut s = if qualifier.is_empty() {
        String::new()
    } else {
        format!("{} ", qualifier)
    };

    let tn = msl_type_name(type_name);
    s.push_str(&format!("{} {}", tn, var_name));

    // Semantic
    if !semantic.is_empty() {
        s.push_str(&format!(" : {}", semantic));
    }

    // Flat qualifier for integer geomprop outputs
    if qualifier.is_empty()
        && type_name == "integer"
        && !assign_value
        && port_name.starts_with("$inGeomprop")
    {
        s.push_str(&format!(" [[ {} ]]", FLAT_QUALIFIER));
    }

    // Value assignment
    if assign_value {
        if !value_str.is_empty() {
            s.push_str(&format!(" = {}", value_str));
        } else {
            let default = match type_name {
                "float" => "0.0",
                "integer" | "boolean" => "0",
                "vector2" => "float2(0.0)",
                "vector3" | "color3" => "float3(0.0)",
                "vector4" | "color4" => "float4(0.0)",
                "matrix33" => "float3x3(1.0)",
                "matrix44" => "float4x4(1.0)",
                _ => "",
            };
            if !default.is_empty() {
                s.push_str(&format!(" = {}", default));
            }
        }
    }

    s
}

// ---- Light Data ---------------------------------------------------------

/// Emit light data struct and uniform block (ref: MslShaderGenerator::emitLightData).
fn emit_light_data(
    stage: &mut ShaderStage,
    resource_binding_context: Option<&Rc<RefCell<Box<dyn ResourceBindingContext>>>>,
) {
    // Clone the block data to avoid borrow conflicts
    let light_data_opt: Option<VariableBlock> = stage.get_uniform_block(block::LIGHT_DATA).cloned();

    let light_data = match light_data_opt {
        Some(ld) if !ld.is_empty() => ld,
        _ => return,
    };

    let struct_array_suffix = format!("[{}]", ident::LIGHT_DATA_MAX_LIGHT_SOURCES);
    let struct_name = light_data.get_instance().to_string();

    if let Some(rbc) = resource_binding_context {
        rbc.borrow_mut().emit_structured_resource_bindings(
            &light_data,
            stage,
            &struct_name,
            &struct_array_suffix,
            UNIFORM_QUALIFIER,
            &msl_type_name,
        );
    } else {
        let block_name = light_data.get_name().to_string();
        stage.append_line(&format!("struct {}", block_name));
        stage.append_line("{");
        for name in light_data.get_variable_order() {
            if let Some(v) = light_data.find(name) {
                let ty = msl_type_name(v.get_type().get_name());
                stage.append_line(&format!("    {} {};", ty, v.get_variable()));
            }
        }
        stage.append_line("};");
        stage.append_line("");
        stage.append_line(&format!(
            "uniform {} {} {};",
            block_name, struct_name, struct_array_suffix
        ));
    }
    stage.append_line("");
}

// ---- Specular Environment ------------------------------------------------

fn emit_specular_environment<C: ShaderGraphCreateContext + ShaderImplContext>(
    ctx: &C,
    stage: &mut ShaderStage,
) {
    match ctx.get_options().hw_specular_environment_method {
        HwSpecularEnvironmentMethod::Fis => {
            emit_library_include(
                "pbrlib/genglsl/lib/mx_environment_fis.glsl",
                ctx,
                stage,
                true,
            );
        }
        HwSpecularEnvironmentMethod::Prefilter => {
            emit_library_include(
                "pbrlib/genglsl/lib/mx_environment_prefilter.glsl",
                ctx,
                stage,
                true,
            );
        }
        HwSpecularEnvironmentMethod::None => {
            emit_library_include(
                "pbrlib/genglsl/lib/mx_environment_none.glsl",
                ctx,
                stage,
                true,
            );
        }
    }
    stage.append_line("");
}

// ---- Transmission Render ------------------------------------------------

fn emit_transmission_render<C: ShaderGraphCreateContext + ShaderImplContext>(
    ctx: &C,
    stage: &mut ShaderStage,
) {
    match ctx.get_options().hw_transmission_render_method {
        HwTransmissionRenderMethod::Refraction => {
            emit_library_include(
                "pbrlib/genglsl/lib/mx_transmission_refract.glsl",
                ctx,
                stage,
                true,
            );
        }
        HwTransmissionRenderMethod::Opacity => {
            emit_library_include(
                "pbrlib/genglsl/lib/mx_transmission_opacity.glsl",
                ctx,
                stage,
                true,
            );
        }
    }
    stage.append_line("");
}

// ---- Light Function Definitions -----------------------------------------

fn emit_light_function_definitions<C: ShaderGraphCreateContext + ShaderImplContext>(
    graph: &ShaderGraph,
    doc: &Document,
    ctx: &C,
    stage: &mut ShaderStage,
) {
    let requires_lighting = graph.has_classification(ShaderNodeClassification::SHADER)
        || graph.has_classification(ShaderNodeClassification::CLOSURE);

    if !requires_lighting || ctx.get_options().hw_max_active_light_sources == 0 {
        return;
    }

    if !graph
        .has_classification(ShaderNodeClassification::SHADER | ShaderNodeClassification::SURFACE)
    {
        return;
    }

    let target = ctx.get_implementation_target();
    for node_name in &graph.node_order {
        let node = match graph.get_node(node_name) {
            Some(n) => n,
            None => continue,
        };
        if node.has_classification(ShaderNodeClassification::SHADER)
            && !node.has_classification(ShaderNodeClassification::SURFACE)
        {
            let node_def_name = match graph.get_node_def(node_name) {
                Some(nd) => nd,
                None => continue,
            };
            if let Some(impl_) = ctx.get_implementation_for_nodedef(doc, node_def_name, target) {
                impl_.emit_function_definition(node, ctx, stage);
            }
        }
    }
}

// ---- Type Definitions ---------------------------------------------------

fn emit_type_definitions(stage: &mut ShaderStage) {
    let type_defs = [
        "struct BSDF { float3 response; float3 throughput; };",
        "#define EDF float3",
        "struct surfaceshader { float3 color; float3 transparency; };",
        "struct volumeshader { float3 color; float3 transparency; };",
        "struct displacementshader { float3 offset; float scale; };",
        "struct lightshader { float3 intensity; float3 direction; };",
        "#define material surfaceshader",
    ];
    for def in type_defs {
        stage.append_line(def);
    }
    stage.append_line("");
}

// ---- GlobalContext emission modes ----------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum EmitGlobalScope {
    EntryFunctionResources,
    MemberInit,
    MemberDecl,
    ConstructorArgs,
    ConstructorInit,
}

/// Collected uniform variable info for global context emission.
struct UniformVarInfo {
    type_name: String,
    var_name: String,
    port_name: String,
    is_filename: bool,
}

/// Collected uniform block info.
struct UniformBlockInfo {
    name: String,
    instance: String,
    is_light_data: bool,
    vars: Vec<UniformVarInfo>,
}

/// Collected vertex input info.
struct VertexInputInfo {
    block_name: String,
    instance: String,
    vars: Vec<UniformVarInfo>,
}

/// Helper to convert output type to float4 if needed.
fn to_vec4(type_name: &str, var: &str) -> String {
    match type_name {
        "color4" | "vector4" => var.to_string(),
        // float3-based types (including BSDF, EDF which are float3 aliases)
        "color3" | "vector3" | "BSDF" | "EDF" => format!("float4({}, 1.0)", var),
        "vector2" => format!("float4({}, 0.0, 1.0)", var),
        "float" | "integer" | "boolean" => {
            format!("float4({}, {}, {}, 1.0)", var, var, var)
        }
        // Unknown type: output black (ref: C++ HwShaderGenerator::toVec4 fallback)
        _ => "float4(0.0, 0.0, 0.0, 1.0)".to_string(),
    }
}

/// Pre-collect all data needed for emit_global_variables to avoid borrow conflicts.
fn collect_global_data(
    stage: &ShaderStage,
    is_vertex_shader: bool,
) -> (
    Option<VertexInputInfo>,
    Vec<UniformBlockInfo>,
    Vec<UniformVarInfo>,
) {
    // Vertex/pixel inputs
    let input_block_name = if is_vertex_shader {
        block::VERTEX_INPUTS
    } else {
        block::VERTEX_DATA
    };
    let vertex_input = stage
        .get_input_block(input_block_name)
        .map(|blk| VertexInputInfo {
            block_name: blk.get_name().to_string(),
            instance: blk.get_instance().to_string(),
            vars: blk
                .get_variable_order()
                .iter()
                .filter_map(|n| {
                    blk.find(n).map(|v| UniformVarInfo {
                        type_name: v.get_type().get_name().to_string(),
                        var_name: v.get_variable().to_string(),
                        port_name: v.get_name().to_string(),
                        is_filename: v.get_type().get_name() == "filename",
                    })
                })
                .collect(),
        });

    // Uniform blocks
    let uniform_blocks: Vec<UniformBlockInfo> = stage
        .get_uniform_blocks()
        .iter()
        .map(|(bn, b)| UniformBlockInfo {
            name: b.get_name().to_string(),
            instance: b.get_instance().to_string(),
            is_light_data: bn.as_str() == block::LIGHT_DATA,
            vars: b
                .get_variable_order()
                .iter()
                .filter_map(|n| {
                    b.find(n).map(|v| UniformVarInfo {
                        type_name: v.get_type().get_name().to_string(),
                        var_name: v.get_variable().to_string(),
                        port_name: v.get_name().to_string(),
                        is_filename: v.get_type().get_name() == "filename",
                    })
                })
                .collect(),
        })
        .collect();

    // Pixel outputs
    let pixel_outputs: Vec<UniformVarInfo> = if !is_vertex_shader {
        stage
            .get_output_block(block::PIXEL_OUTPUTS)
            .map(|blk| {
                blk.get_variable_order()
                    .iter()
                    .filter_map(|n| {
                        blk.find(n).map(|v| UniformVarInfo {
                            type_name: v.get_type().get_name().to_string(),
                            var_name: v.get_variable().to_string(),
                            port_name: v.get_name().to_string(),
                            is_filename: false,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    } else {
        vec![]
    };

    (vertex_input, uniform_blocks, pixel_outputs)
}

fn emit_global_variables(
    stage: &mut ShaderStage,
    situation: EmitGlobalScope,
    is_vertex_shader: bool,
    needs_light_data: bool,
    max_lights: u32,
) {
    let (vertex_input, uniform_blocks, pixel_outputs) =
        collect_global_data(stage, is_vertex_shader);

    let mut buffer_slot: usize = if is_vertex_shader {
        vertex_input
            .as_ref()
            .map(|vi| vi.vars.len().max(0))
            .unwrap_or(0)
    } else {
        0
    };
    let mut tex_slot: usize = 0;

    let entry = situation == EmitGlobalScope::EntryFunctionResources;
    let init = situation == EmitGlobalScope::MemberInit;
    let members = situation == EmitGlobalScope::MemberDecl;
    let ctor_args = situation == EmitGlobalScope::ConstructorArgs;
    let ctor_init = situation == EmitGlobalScope::ConstructorInit;

    let mut separator = String::new();

    // Pixel stage: gl_FragCoord
    if !is_vertex_shader {
        if members {
            stage.append_line("float4 gl_FragCoord;");
        }
        if ctor_init {
            if let Some(vi) = &vertex_input {
                stage.append_source_code(&format!("gl_FragCoord({}.pos)", vi.instance));
                separator = ",".to_string();
            }
        }
    }

    // Vertex inputs / vertex data
    if let Some(vi) = &vertex_input {
        if !entry {
            if is_vertex_shader {
                for v in &vi.vars {
                    stage.append_source_code(&separator);
                    if init {
                        stage.append_source_code(&format!("{}.{}", vi.instance, v.port_name));
                    } else if members || ctor_args {
                        let ty = msl_type_name(&v.type_name);
                        let decl = format!("{} {}", ty, v.port_name);
                        if ctor_args {
                            stage.append_source_code(&decl);
                        } else {
                            stage.append_line(&format!("{};", decl));
                        }
                    } else if ctor_init {
                        stage.append_source_code(&format!("{}({})", v.port_name, v.port_name));
                    }
                    if init || ctor_args || ctor_init {
                        separator = ", ".to_string();
                    } else if members {
                        separator = "\n".to_string();
                    }
                }
            } else {
                // Pixel stage: vertex data as single block
                stage.append_source_code(&separator);
                if init {
                    stage.append_source_code(&vi.instance);
                    separator = ", ".to_string();
                } else if members || ctor_args {
                    let decl = format!("{} {}", vi.block_name, vi.instance);
                    if ctor_args {
                        stage.append_source_code(&decl);
                        separator = ", ".to_string();
                    } else {
                        stage.append_line(&format!("{};", decl));
                        separator = "\n".to_string();
                    }
                } else if ctor_init {
                    stage.append_source_code(&format!("{}({})", vi.instance, vi.instance));
                    separator = ", ".to_string();
                }
            }
        } else {
            // Entry function: [[stage_in]]
            stage.append_source_code(&format!("{} {} [[ stage_in ]]", vi.block_name, vi.instance));
            separator = ", ".to_string();
        }
    }

    // Uniform blocks
    for ub in &uniform_blocks {
        if !needs_light_data && ub.is_light_data {
            continue;
        }

        if ub.is_light_data {
            stage.append_source_code(&separator);
            if entry {
                stage.append_source_code(&format!(
                    "{} {}_{}& {} [[ buffer({}) ]]",
                    UNIFORM_QUALIFIER,
                    ub.name,
                    stage.get_name(),
                    ub.instance,
                    buffer_slot
                ));
                buffer_slot += 1;
            } else if init {
                stage.append_source_code(&format!("{}.{}", ub.instance, ub.instance));
            } else if members || ctor_args {
                let struct_suffix = format!("[{}]", ident::LIGHT_DATA_MAX_LIGHT_SOURCES);
                let q = if ctor_args {
                    format!("{} ", UNIFORM_QUALIFIER)
                } else {
                    String::new()
                };
                let decl = format!("{}{} {}{}", q, ub.name, ub.instance, struct_suffix);
                if ctor_args {
                    stage.append_source_code(&decl);
                } else {
                    stage.append_line(&format!("{};", decl));
                }
            } else if ctor_init {
                let ml = max_lights.max(1);
                stage.append_source_code(&ub.instance);
                stage.append_source_code("{");
                for l in 0..ml {
                    if l > 0 {
                        stage.append_source_code(", ");
                    }
                    stage.append_source_code(&format!("{}[{}]", ub.instance, l));
                }
                stage.append_source_code("}");
            }
        } else {
            // Regular uniform blocks
            if !entry {
                if !ub.vars.is_empty() {
                    for v in &ub.vars {
                        if !v.is_filename {
                            stage.append_source_code(&separator);
                            if init {
                                stage
                                    .append_source_code(&format!("{}.{}", ub.instance, v.var_name));
                            } else if members || ctor_args {
                                let ty = msl_type_name(&v.type_name);
                                let decl = format!("{} {}", ty, v.var_name);
                                if ctor_args {
                                    stage.append_source_code(&decl);
                                } else {
                                    stage.append_line(&format!("{};", decl));
                                }
                            } else if ctor_init {
                                stage
                                    .append_source_code(&format!("{}({})", v.var_name, v.var_name));
                            }
                        } else {
                            // FILENAME type
                            if init {
                                stage.append_source_code(&separator);
                                stage.append_source_code("MetalTexture");
                                stage.append_source_code("{");
                                stage.append_source_code(&texture_name(&v.var_name));
                                stage.append_source_code(", ");
                                stage.append_source_code(&sampler_name(&v.var_name));
                                stage.append_source_code("}");
                            } else if members || ctor_args {
                                stage.append_source_code(&separator);
                                let decl = emit_variable_declaration(
                                    &v.type_name,
                                    &v.var_name,
                                    "",
                                    &v.port_name,
                                    "",
                                    false,
                                    "",
                                );
                                stage.append_source_code(&decl);
                                if !ctor_args {
                                    stage.append_source_code(";");
                                }
                            } else if ctor_init {
                                stage.append_source_code(&separator);
                                stage
                                    .append_source_code(&format!("{}({})", v.var_name, v.var_name));
                            }
                        }

                        if init || ctor_args || ctor_init {
                            separator = ", ".to_string();
                        } else if members {
                            separator = "\n".to_string();
                        }
                    }
                }
            } else {
                // Entry function args: texture/sampler bindings + buffer bindings
                if !ub.vars.is_empty() {
                    let mut has_uniforms = false;
                    for v in &ub.vars {
                        if v.is_filename {
                            stage.append_source_code(&separator);
                            stage.append_source_code(&format!(
                                "texture2d<float> {} [[texture({})]], sampler {} [[sampler({})]]",
                                texture_name(&v.var_name),
                                tex_slot,
                                sampler_name(&v.var_name),
                                tex_slot,
                            ));
                            tex_slot += 1;
                        } else {
                            has_uniforms = true;
                        }
                    }
                    if has_uniforms {
                        stage.append_source_code(&separator);
                        stage.append_source_code(&format!(
                            "{} {}& {} [[ buffer({}) ]]",
                            UNIFORM_QUALIFIER, ub.name, ub.instance, buffer_slot
                        ));
                        buffer_slot += 1;
                    }
                }
            }
        }

        if init || entry || ctor_args || ctor_init {
            separator = ", ".to_string();
        } else {
            separator = "\n".to_string();
        }
    }

    // Pixel output member declarations
    if !is_vertex_shader && members {
        for v in &pixel_outputs {
            let ty = msl_type_name(&v.type_name);
            stage.append_line(&format!("{} {};", ty, v.var_name));
        }
    }
}

// ---- Vertex Stage -------------------------------------------------------

fn emit_vertex_stage(
    graph: &ShaderGraph,
    doc: &Document,
    ctx: &MslShaderGraphContext<'_>,
    _generator: &MslShaderGenerator,
    stage: &mut ShaderStage,
    resource_binding_context: Option<&Rc<RefCell<Box<dyn ResourceBindingContext>>>>,
) {
    emit_directives(stage);
    if let Some(rbc) = resource_binding_context {
        rbc.borrow().emit_directives(stage);
    }
    stage.append_line("");

    emit_constant_buffer_declarations(stage, resource_binding_context);
    emit_inputs(stage, true);
    emit_outputs(stage, true);

    // Pre-collect vertex data info
    let vd_name = stage
        .get_output_block(block::VERTEX_DATA)
        .map(|b| b.get_name().to_string())
        .unwrap_or_else(|| "VertexData".to_string());
    let vd_instance = stage
        .get_output_block(block::VERTEX_DATA)
        .map(|b| b.get_instance().to_string())
        .unwrap_or_else(|| "vd".to_string());

    // GlobalContext struct
    stage.append_line("struct GlobalContext");
    stage.append_line("{");
    {
        stage.append_source_code("GlobalContext(");
        emit_global_variables(stage, EmitGlobalScope::ConstructorArgs, true, false, 0);
        stage.append_source_code(") : ");
        emit_global_variables(stage, EmitGlobalScope::ConstructorInit, true, false, 0);
        stage.append_line("{}");

        emit_library_include(LIB_MX_MATH, ctx, stage, false);
        stage.append_line("");

        emit_global_variables(stage, EmitGlobalScope::MemberDecl, true, false, 0);

        emit_function_definitions(graph, doc, ctx, stage, true);

        stage.append_line(&format!("{} VertexMain()", vd_name));
        stage.append_line("{");
        stage.append_line(&format!("    {} {};", vd_name, vd_instance));
        stage.append_line(&format!(
            "    float4 hPositionWorld = {} * float4({}, 1.0);",
            token::T_WORLD_MATRIX,
            token::T_IN_POSITION
        ));
        stage.append_line(&format!(
            "    {}.pos = {} * hPositionWorld;",
            vd_instance,
            token::T_VIEW_PROJECTION_MATRIX
        ));

        emit_function_calls(graph, doc, ctx, stage);
        stage.append_line("");
        stage.append_line(&format!("    return {};", vd_instance));

        // Dual function call pass (ref: C++ per-node emitFunctionCall)
        for node_name in &graph.node_order {
            emit_single_function_call(graph, doc, ctx, stage, node_name);
        }

        stage.append_line("}");
    }
    stage.append_line("};");
    stage.append_line("");

    // Entry point
    stage.append_source_code(&format!("vertex {} VertexMain(", vd_name));
    emit_global_variables(
        stage,
        EmitGlobalScope::EntryFunctionResources,
        true,
        false,
        0,
    );
    stage.append_line(")");
    stage.append_line("{");
    stage.append_source_code("\tGlobalContext ctx {");
    emit_global_variables(stage, EmitGlobalScope::MemberInit, true, false, 0);
    stage.append_line("};");
    stage.append_line(&format!("    {} outVertex = ctx.VertexMain();", vd_name));
    stage.append_line("    outVertex.pos.y = -outVertex.pos.y;");
    stage.append_line("    return outVertex;");
    stage.append_line("}");
    stage.append_line("");
}

// ---- Pixel Stage --------------------------------------------------------

fn emit_pixel_stage(
    graph: &ShaderGraph,
    doc: &Document,
    ctx: &MslShaderGraphContext<'_>,
    _generator: &MslShaderGenerator,
    stage: &mut ShaderStage,
    resource_binding_context: Option<&Rc<RefCell<Box<dyn ResourceBindingContext>>>>,
) {
    let options = ctx.ctx.get_options();

    emit_directives(stage);
    if let Some(rbc) = resource_binding_context {
        rbc.borrow().emit_directives(stage);
    }
    stage.append_line("");

    emit_library_include(LIB_MX_TEXTURE, ctx, stage, false);
    emit_type_definitions(stage);
    emit_constant_buffer_declarations(stage, resource_binding_context);
    emit_constants(stage);
    emit_inputs(stage, false);
    emit_outputs(stage, false);

    let lighting = graph.has_classification(ShaderNodeClassification::SHADER)
        || graph.has_classification(ShaderNodeClassification::CLOSURE);

    if lighting || options.hw_write_albedo_table {
        stage.append_line(&format!(
            "#define DIRECTIONAL_ALBEDO_METHOD {}",
            options.hw_directional_albedo_method as i32
        ));
        stage.append_line("");
    }

    stage.append_line(&format!(
        "#define AIRY_FRESNEL_ITERATIONS {}",
        options.hw_airy_fresnel_iterations
    ));
    stage.append_line("");

    if lighting {
        let max_lights = options.hw_max_active_light_sources;
        if max_lights > 0 {
            let ml = max_lights.max(1);
            stage.append_line(&format!(
                "#define {} {}",
                ident::LIGHT_DATA_MAX_LIGHT_SOURCES,
                ml
            ));
        }
        if max_lights > 0 {
            emit_light_data(stage, resource_binding_context);
        }
    }

    let needs_light_buffer = lighting && options.hw_max_active_light_sources > 0;
    let max_lights = options.hw_max_active_light_sources.max(1);

    emit_library_include(LIB_MX_MATSCALAROPS, ctx, stage, false);

    // Pre-collect outputs info
    let outputs_name = stage
        .get_output_block(block::PIXEL_OUTPUTS)
        .map(|b| b.get_name().to_string())
        .unwrap_or_else(|| "PixelOutputs".to_string());
    let output_var_names: Vec<String> = stage
        .get_output_block(block::PIXEL_OUTPUTS)
        .map(|b| {
            b.get_variable_order()
                .iter()
                .filter_map(|n| b.find(n).map(|v| v.get_variable().to_string()))
                .collect()
        })
        .unwrap_or_default();

    // GlobalContext struct
    stage.append_line("struct GlobalContext");
    stage.append_line("{");
    {
        stage.append_source_code("GlobalContext(");
        emit_global_variables(
            stage,
            EmitGlobalScope::ConstructorArgs,
            false,
            needs_light_buffer,
            max_lights,
        );
        stage.append_source_code(") : ");
        emit_global_variables(
            stage,
            EmitGlobalScope::ConstructorInit,
            false,
            needs_light_buffer,
            max_lights,
        );
        stage.append_line("{}");

        emit_library_include(LIB_MX_MATH, ctx, stage, false);
        stage.append_line("");

        if lighting {
            emit_specular_environment(ctx, stage);
            emit_transmission_render(ctx, stage);
        }

        emit_global_variables(
            stage,
            EmitGlobalScope::MemberDecl,
            false,
            needs_light_buffer,
            max_lights,
        );

        // Shadow support
        let shadowing = (lighting && options.hw_shadow_map) || options.hw_write_depth_moments;
        if shadowing {
            emit_library_include("pbrlib/genglsl/lib/mx_shadow.glsl", ctx, stage, true);
            emit_library_include(
                "pbrlib/genmsl/lib/mx_shadow_platform.metal",
                ctx,
                stage,
                false,
            );
        }

        if options.hw_write_albedo_table {
            emit_library_include(
                "pbrlib/genglsl/lib/mx_generate_albedo_table.glsl",
                ctx,
                stage,
                true,
            );
            stage.append_line("");
        }

        if options.hw_write_env_prefilter {
            emit_library_include(
                "pbrlib/genglsl/lib/mx_generate_prefilter_env.glsl",
                ctx,
                stage,
                true,
            );
            stage.append_line("");
        }

        // UV transform token
        let file_transform_uv = if options.file_texture_vertical_flip {
            "mx_transform_uv_vflip.glsl"
        } else {
            "mx_transform_uv.glsl"
        };
        stage.source_code = stage
            .source_code
            .replace(T_FILE_TRANSFORM_UV, file_transform_uv);

        emit_light_function_definitions(graph, doc, ctx, stage);
        emit_function_definitions(graph, doc, ctx, stage, true);

        // FragmentMain method
        stage.append_line(&format!("{} FragmentMain()", outputs_name));
        stage.append_line("{");

        // Get output socket info
        let output_socket_info: Option<(String, String, Option<(String, String)>)> =
            graph.get_output_socket_at(0).map(|sock| {
                let var = sock.port.get_variable().to_string();
                let type_name = sock.port.get_type().get_name().to_string();
                let conn = sock
                    .get_connection()
                    .map(|(n, o)| (n.to_string(), o.to_string()));
                (var, type_name, conn)
            });

        let is_closure_only = graph.has_classification(ShaderNodeClassification::CLOSURE)
            && !graph.has_classification(ShaderNodeClassification::SHADER);

        if is_closure_only {
            if let Some((var, _, _)) = &output_socket_info {
                stage.append_line(&format!("    {} = float4(0.0, 0.0, 0.0, 1.0);", var));
            }
        } else if options.hw_write_depth_moments {
            if let Some((var, _, _)) = &output_socket_info {
                stage.append_line(&format!(
                    "    {} = float4(mx_compute_depth_moments(), 0.0, 1.0);",
                    var
                ));
            }
        } else if options.hw_write_albedo_table {
            if let Some((var, _, _)) = &output_socket_info {
                stage.append_line(&format!(
                    "    {} = float4(mx_generate_dir_albedo_table(), 1.0);",
                    var
                ));
            }
        } else if options.hw_write_env_prefilter {
            if let Some((var, _, _)) = &output_socket_info {
                stage.append_line(&format!(
                    "    {} = float4(mx_generate_prefilter_env(), 1.0);",
                    var
                ));
            }
        } else {
            let is_surface_shader = graph.has_classification(
                ShaderNodeClassification::SHADER | ShaderNodeClassification::SURFACE,
            );

            if is_surface_shader {
                emit_function_calls_classified(
                    graph,
                    doc,
                    ctx,
                    stage,
                    ShaderNodeClassification::TEXTURE,
                );

                if let Some(sock) = graph.get_output_socket_at(0) {
                    if let Some((up_node, _up_output)) = sock.get_connection() {
                        if let Some(upstream) = graph.get_node(up_node) {
                            if upstream.has_classification(ShaderNodeClassification::CLOSURE)
                                || upstream.has_classification(ShaderNodeClassification::SHADER)
                            {
                                emit_single_function_call(graph, doc, ctx, stage, up_node);
                            }
                        }
                    }
                }
            } else {
                emit_function_calls(graph, doc, ctx, stage);
            }

            // Emit final output
            if let Some((out_var, out_type, conn)) = &output_socket_info {
                if let Some((up_node, up_output)) = conn {
                    let final_output = graph
                        .get_connection_variable(up_node, up_output)
                        .unwrap_or_else(|| "float4(0.0)".to_string());

                    if graph.has_classification(ShaderNodeClassification::SURFACE) {
                        if options.hw_transparency {
                            stage.append_line(&format!(
                                "    float outAlpha = clamp(1.0 - dot({}.transparency, float3(0.3333)), 0.0, 1.0);",
                                final_output
                            ));
                            stage.append_line(&format!(
                                "    {} = float4({}.color, outAlpha);",
                                out_var, final_output
                            ));
                            stage.append_line(&format!(
                                "    if (outAlpha < {})",
                                token::T_ALPHA_THRESHOLD
                            ));
                            stage.append_line("    {");
                            stage.append_line("        discard_fragment();");
                            stage.append_line("    }");
                        } else {
                            stage.append_line(&format!(
                                "    {} = float4({}.color, 1.0);",
                                out_var, final_output
                            ));
                        }
                    } else {
                        let converted = if out_type != "color4" && out_type != "vector4" {
                            to_vec4(out_type, &final_output)
                        } else {
                            final_output.clone()
                        };
                        stage.append_line(&format!("    {} = {};", out_var, converted));
                    }
                } else {
                    // No connection: use default value from syntax
                    let default_val = match out_type.as_str() {
                        "color4" | "vector4" => "float4(0.0)",
                        "color3" | "vector3" => "float3(0.0)",
                        "vector2" => "float2(0.0)",
                        "float" => "0.0",
                        _ => "float4(0.0)",
                    };

                    if out_type != "color4" && out_type != "vector4" {
                        let tmp_var = format!("{}_tmp", out_var);
                        let ty = msl_type_name(out_type);
                        stage.append_line(&format!("    {} {} = {};", ty, tmp_var, default_val));
                        let converted = to_vec4(out_type, &tmp_var);
                        stage.append_line(&format!("    {} = {};", out_var, converted));
                    } else {
                        stage.append_line(&format!("    {} = {};", out_var, default_val));
                    }
                }
            }
        }

        // Return struct
        stage.append_line(&format!(
            "    return {}{{{}}};",
            outputs_name,
            output_var_names.join(", ")
        ));

        stage.append_line("}");
    }
    stage.append_line("};");
    stage.append_line("");

    // Entry point — matches C++ reference: `fragment {PixelOutputs} FragmentMain(...)`
    // Returns the outputs struct directly, identical to how the vertex entry point works.
    stage.append_source_code(&format!("fragment {} FragmentMain(", outputs_name));
    emit_global_variables(
        stage,
        EmitGlobalScope::EntryFunctionResources,
        false,
        needs_light_buffer,
        max_lights,
    );
    stage.append_line(")");
    stage.append_line("{");
    stage.append_source_code("\tGlobalContext ctx {");
    emit_global_variables(
        stage,
        EmitGlobalScope::MemberInit,
        false,
        needs_light_buffer,
        max_lights,
    );
    stage.append_line("};");
    stage.append_line("    return ctx.FragmentMain();");
    stage.append_line("}");
    stage.append_line("");
}

// ---- Public entry point -------------------------------------------------

/// Emit shader for Metal target.
pub fn emit_shader_msl(
    name: &str,
    graph: ShaderGraph,
    mut stages: Vec<ShaderStage>,
    doc: &Document,
    context: &GenContext<MslShaderGenerator>,
    generator: &MslShaderGenerator,
) -> crate::gen_shader::Shader {
    let subs = msl_token_substitutions();
    let emit_ctx = MslShaderGraphContext::with_graph(context, &graph);
    let rbc = context.get_resource_binding_context();

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
        );
        replace_tokens(&mut stages[idx], &subs);
        stages[idx].source_code = metalize_generated_shader(&stages[idx].source_code);
    }
    if let Some(idx) = ps_idx {
        emit_pixel_stage(
            &graph,
            doc,
            &emit_ctx,
            generator,
            &mut stages[idx],
            rbc.as_ref(),
        );
        replace_tokens(&mut stages[idx], &subs);
        stages[idx].source_code = metalize_generated_shader(&stages[idx].source_code);
    }

    crate::gen_shader::Shader::from_parts(name, graph, stages)
}
