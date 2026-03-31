//! HwSurfaceNode -- surface constructor (bsdf + edf + opacity -> surfaceshader).
//! Full C++ parity with MaterialX HwSurfaceNode.cpp.
//!
//! Reads bsdf, edf, opacity inputs and combines them with lighting:
//! - createVariables: position, normal inputs, world_inverse_transpose uniform,
//!   position_world/normal_world connectors, view_position uniform, lighting uniforms.
//! - emitFunctionCall vertex: position_world, normal_world, AO texcoord passthrough.
//! - emitFunctionCall pixel: full ClosureData system for reflection/indirect/emission/transmission.

use crate::gen_hw::hw_constants::{block, lighting, token};
use crate::gen_shader::{
    HwTransmissionRenderMethod, Shader, ShaderImplContext, ShaderNode, ShaderNodeClassification,
    ShaderNodeImpl, ShaderStage, add_stage_input, add_stage_output, add_stage_uniform,
    add_stage_uniform_with_value, shader_stage, type_desc_types,
};

const BSDF_INPUT: &str = "bsdf";
const EDF_INPUT: &str = "edf";
const OPACITY_INPUT: &str = "opacity";

/// Surface constructor -- combines bsdf, edf, opacity into surfaceshader.
/// Vertex stage: emits position_world, normal_world, AO texcoord writes.
/// Pixel stage: full ClosureData construction, light loop, indirect, emission, transmission.
#[derive(Debug, Default)]
pub struct HwSurfaceNode {
    name: String,
    hash: u64,
}

impl HwSurfaceNode {
    pub fn create() -> Box<dyn ShaderNodeImpl> {
        Box::new(Self::default())
    }
}

impl ShaderNodeImpl for HwSurfaceNode {
    fn get_name(&self) -> &str {
        &self.name
    }
    fn get_hash(&self) -> u64 {
        self.hash
    }
    fn initialize(&mut self, element: &crate::core::ElementPtr, _context: &dyn ShaderImplContext) {
        self.name = element.borrow().get_name().to_string();
        self.hash = crate::gen_shader::hash_string(&self.name);
    }

    /// Create variables needed by the surface shader.
    /// C++: adds position+normal vertex inputs, world_inverse_transpose uniform,
    /// position_world/normal_world connectors, view_position, and lighting uniforms.
    fn create_variables(
        &self,
        _node_name: &str,
        _context: &dyn ShaderImplContext,
        shader: &mut Shader,
    ) {
        // Vertex stage: position, normal inputs + world_inverse_transpose uniform
        if let Some(vs) = shader.get_stage_by_name_mut(shader_stage::VERTEX) {
            add_stage_input(
                block::VERTEX_INPUTS,
                type_desc_types::vector3(),
                token::T_IN_POSITION,
                vs,
                false,
            );
            add_stage_input(
                block::VERTEX_INPUTS,
                type_desc_types::vector3(),
                token::T_IN_NORMAL,
                vs,
                false,
            );
            add_stage_uniform(
                block::PRIVATE_UNIFORMS,
                type_desc_types::matrix44(),
                token::T_WORLD_INVERSE_TRANSPOSE_MATRIX,
                vs,
            );
            add_stage_output(
                block::VERTEX_DATA,
                type_desc_types::vector3(),
                token::T_POSITION_WORLD,
                vs,
                false,
            );
            add_stage_output(
                block::VERTEX_DATA,
                type_desc_types::vector3(),
                token::T_NORMAL_WORLD,
                vs,
                false,
            );
        }

        // Pixel stage: view_position uniform + vertex data inputs + lighting uniforms
        if let Some(ps) = shader.get_stage_by_name_mut(shader_stage::PIXEL) {
            add_stage_input(
                block::VERTEX_DATA,
                type_desc_types::vector3(),
                token::T_POSITION_WORLD,
                ps,
                false,
            );
            add_stage_input(
                block::VERTEX_DATA,
                type_desc_types::vector3(),
                token::T_NORMAL_WORLD,
                ps,
                false,
            );
            add_stage_uniform(
                block::PRIVATE_UNIFORMS,
                type_desc_types::vector3(),
                token::T_VIEW_POSITION,
                ps,
            );

            // C++: shadergen.addStageLightingUniforms(context, ps)
            add_stage_uniform_with_value(
                block::PRIVATE_UNIFORMS,
                type_desc_types::integer(),
                token::T_NUM_ACTIVE_LIGHT_SOURCES,
                ps,
                Some(crate::core::Value::Integer(0)),
            );
        }
    }

    fn emit_function_call(
        &self,
        node: &ShaderNode,
        context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
    ) {
        match stage.get_name() {
            shader_stage::VERTEX => self.emit_vertex(node, context, stage),
            shader_stage::PIXEL => self.emit_pixel(node, context, stage),
            _ => {}
        }
    }

    fn emit_output_variables(
        &self,
        node: &ShaderNode,
        _context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
    ) {
        if stage.get_name() != shader_stage::PIXEL {
            return;
        }
        for output in node.get_outputs() {
            let var = output.port.get_variable();
            stage.append_line(&format!(
                "surfaceshader {} = surfaceshader(vec3(0.0), vec3(0.0));",
                var
            ));
        }
    }
}

impl HwSurfaceNode {
    /// Vertex stage: write world-space position and normal into vertex data block.
    /// Also passes through ambient occlusion texcoord if enabled (C++ line 74-82).
    fn emit_vertex(
        &self,
        _node: &ShaderNode,
        context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
    ) {
        let prefix = "vd.";

        // Position world (C++ line 62-67)
        {
            let (line, need_emit) = {
                let vertex_data = match stage.get_output_block_mut(block::VERTEX_DATA) {
                    Some(b) => b,
                    None => return,
                };
                if let Some(position) = vertex_data.find_mut(token::T_POSITION_WORLD) {
                    if !position.is_emitted() {
                        position.set_emitted(true);
                        let l = format!(
                            "{}{}  = hPositionWorld.xyz;",
                            prefix,
                            position.get_variable()
                        );
                        (l, true)
                    } else {
                        (String::new(), false)
                    }
                } else {
                    (String::new(), false)
                }
            };
            if need_emit {
                stage.append_line(&line);
            }
        }

        // Normal world (C++ line 68-73)
        {
            let (line, need_emit) = {
                let vertex_data = match stage.get_output_block_mut(block::VERTEX_DATA) {
                    Some(b) => b,
                    None => return,
                };
                if let Some(normal) = vertex_data.find_mut(token::T_NORMAL_WORLD) {
                    if !normal.is_emitted() {
                        normal.set_emitted(true);
                        let l = format!(
                            "{}{} = normalize(mx_matrix_mul({}, vec4({}, 0)).xyz);",
                            prefix,
                            normal.get_variable(),
                            token::T_WORLD_INVERSE_TRANSPOSE_MATRIX,
                            token::T_IN_NORMAL
                        );
                        (l, true)
                    } else {
                        (String::new(), false)
                    }
                } else {
                    (String::new(), false)
                }
            };
            if need_emit {
                stage.append_line(&line);
            }
        }

        // Ambient occlusion texcoord passthrough (C++ line 74-82)
        // C++: if (context.getOptions().hwAmbientOcclusion) — we check if connector exists.
        let texcoord_var = format!("{}_0", token::T_TEXCOORD);
        let (line, need_emit) = {
            let vertex_data = match stage.get_output_block_mut(block::VERTEX_DATA) {
                Some(b) => b,
                None => return,
            };
            if let Some(texcoord) = vertex_data.find_mut(&texcoord_var) {
                if !texcoord.is_emitted() {
                    texcoord.set_emitted(true);
                    let l = format!(
                        "{}{} = {}_0;",
                        prefix,
                        texcoord.get_variable(),
                        token::T_IN_TEXCOORD
                    );
                    (l, true)
                } else {
                    (String::new(), false)
                }
            } else {
                (String::new(), false)
            }
        };
        if need_emit {
            stage.append_line(&line);
        }
        let _ = context;
    }

    /// Pixel stage: full surface shader evaluation with ClosureData system.
    /// C++ HwSurfaceNode::emitFunctionCall pixel branch (lines 85-234).
    fn emit_pixel(
        &self,
        node: &ShaderNode,
        context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
    ) {
        let output = match node.get_outputs().next() {
            Some(o) => o,
            None => return,
        };
        let out_var = output.port.get_variable().to_string();
        let prefix = "vd.";

        // Declare output variable (C++ lines 91-94)
        stage.append_line(&format!(
            "surfaceshader {} = surfaceshader(vec3(0.0), vec3(0.0));",
            out_var
        ));
        stage.append_line("{");

        // Setup shading locals N, V, P, L, occlusion (C++ lines 98-103)
        stage.append_line(&format!(
            "    vec3 {N} = normalize({pf}{nw});",
            N = lighting::DIR_N,
            pf = prefix,
            nw = token::T_NORMAL_WORLD,
        ));
        stage.append_line(&format!(
            "    vec3 {V} = normalize({vp} - {pf}{pw});",
            V = lighting::DIR_V,
            vp = token::T_VIEW_POSITION,
            pf = prefix,
            pw = token::T_POSITION_WORLD,
        ));
        stage.append_line(&format!(
            "    vec3 {P} = {pf}{pw};",
            P = lighting::WORLD_POSITION,
            pf = prefix,
            pw = token::T_POSITION_WORLD,
        ));
        stage.append_line(&format!("    vec3 {} = vec3(0.0);", lighting::DIR_L));
        stage.append_line(&format!("    float {} = 1.0;", lighting::OCCLUSION));
        stage.append_line("");

        let out_color = format!("{}.color", out_var);
        let out_transparency = format!("{}.transparency", out_var);

        // Gather BSDF connection info (C++ line 108-109)
        let bsdf_input = node.get_input(BSDF_INPUT);
        let has_bsdf = bsdf_input.map(|i| i.has_connection()).unwrap_or(false);
        let bsdf_node_name: Option<&str> =
            bsdf_input.and_then(|i| i.get_connection()).map(|(n, _)| n);

        // Gather node classifications from graph
        let bsdf_is_bsdf_r = bsdf_node_name
            .map(|name| {
                context
                    .get_graph()
                    .and_then(|g| g.get_node(name))
                    .map(|n| n.has_classification(ShaderNodeClassification::BSDF_R))
                    .unwrap_or(false)
            })
            .unwrap_or(false);

        let bsdf_is_bsdf_t = bsdf_node_name
            .map(|name| {
                context
                    .get_graph()
                    .and_then(|g| g.get_node(name))
                    .map(|n| n.has_classification(ShaderNodeClassification::BSDF_T))
                    .unwrap_or(false)
            })
            .unwrap_or(false);

        let bsdf_is_vdf = bsdf_node_name
            .map(|name| {
                context
                    .get_graph()
                    .and_then(|g| g.get_node(name))
                    .map(|n| n.has_classification(ShaderNodeClassification::VDF))
                    .unwrap_or(false)
            })
            .unwrap_or(false);

        // BSDF output variable name (the variable holding the BSDF struct)
        let bsdf_out_var: Option<String> = bsdf_node_name.map(|name| {
            context
                .get_graph()
                .and_then(|g| g.get_node(name))
                .and_then(|n| n.get_outputs().next())
                .map(|o| o.port.get_variable().to_string())
                .unwrap_or_else(|| format!("{}_out", name))
        });

        if has_bsdf {
            // Emit surfaceOpacity: C++ line 111-115 uses emitInput() which is connection-aware.
            // If opacity is connected, emit the upstream variable; else literal value.
            let opacity_str = resolve_connected_input(node, OPACITY_INPUT, context, "1.0");
            stage.append_line(&format!("    float surfaceOpacity = {};", opacity_str));
            stage.append_line("");

            // === Handle direct lighting (C++ lines 117-127) ===
            stage.append_line("    // Shadow occlusion");
            // C++: if hwShadowMap: occlusion = mx_shadow_occlusion(...)
            if context.get_gen_options().hw_shadow_map {
                stage.append_line(&format!(
                    "    occlusion = mx_shadow_occlusion({}, {}, {}{});",
                    token::T_SHADOW_MAP,
                    token::T_SHADOW_MATRIX,
                    prefix,
                    token::T_POSITION_WORLD,
                ));
            }
            stage.append_line("");

            // Light loop (C++ line 127: emitLightLoop)
            self.emit_light_loop(
                node,
                context,
                stage,
                &out_color,
                &bsdf_out_var,
                bsdf_is_bsdf_r,
                bsdf_node_name,
            );

            // === Handle indirect lighting (C++ lines 129-168) ===
            stage.append_line("    // Ambient occlusion");
            if context.get_gen_options().hw_ambient_occlusion {
                // C++ lines 135-141: read texcoord from vertex data, sample AO map
                stage.append_line(&format!(
                    "    vec2 ambOccUv = {}{};",
                    prefix,
                    format!("{}_0", token::T_TEXCOORD),
                ));
                if context.get_gen_options().file_texture_vertical_flip {
                    stage.append_line("    ambOccUv = vec2(ambOccUv.x, 1.0 - ambOccUv.y);");
                }
                stage.append_line(&format!(
                    "    occlusion = mix(1.0, texture({}, ambOccUv).x, {});",
                    token::T_AMB_OCC_MAP,
                    token::T_AMB_OCC_GAIN,
                ));
            } else {
                stage.append_line("    occlusion = 1.0;");
            }
            stage.append_line("");

            stage.append_line("    // Add environment contribution");
            stage.append_line("    {");

            // Indirect BSDF evaluation (C++ lines 153-167)
            if bsdf_is_bsdf_r {
                stage.append_line("        ClosureData closureData = makeClosureData(CLOSURE_TYPE_INDIRECT, L, V, N, P, occlusion);");
                // C++ line 156: shadergen.emitFunctionCall(*bsdf, context, stage)
                if let Some(name) = bsdf_node_name {
                    context.emit_node_function_call(name, stage);
                }
            } else if let Some(bov) = bsdf_out_var.as_deref() {
                // Non-BSDF_R: just declare the output type (C++ lines 160-162)
                stage.append_line(&format!(
                    "        BSDF {} = BSDF(vec3(0.0), vec3(1.0));",
                    bov
                ));
            }

            stage.append_line("");
            if let Some(bov) = bsdf_out_var.as_deref() {
                // C++ line 166: outColor += occlusion * bsdf.response
                stage.append_line(&format!(
                    "        {} += occlusion * {}.response;",
                    out_color, bov
                ));
            }
            stage.append_line("    }");
            stage.append_line("");
        }

        // === Handle surface emission (C++ lines 171-195) ===
        let edf_input = node.get_input(EDF_INPUT);
        let has_edf = edf_input.map(|i| i.has_connection()).unwrap_or(false);

        if has_edf {
            let edf_node_name: Option<&str> =
                edf_input.and_then(|i| i.get_connection()).map(|(n, _)| n);

            let edf_is_edf = edf_node_name
                .map(|name| {
                    context
                        .get_graph()
                        .and_then(|g| g.get_node(name))
                        .map(|n| n.has_classification(ShaderNodeClassification::EDF))
                        .unwrap_or(false)
                })
                .unwrap_or(false);

            let edf_out_var: Option<String> = edf_node_name.map(|name| {
                context
                    .get_graph()
                    .and_then(|g| g.get_node(name))
                    .and_then(|n| n.get_outputs().next())
                    .map(|o| o.port.get_variable().to_string())
                    .unwrap_or_else(|| format!("{}_out", name))
            });

            stage.append_line("    // Add surface emission");
            stage.append_line("    {");

            if edf_is_edf {
                // C++ line 182-183
                stage.append_line("        ClosureData closureData = makeClosureData(CLOSURE_TYPE_EMISSION, L, V, N, P, occlusion);");
                // C++ line 183: shadergen.emitFunctionCall(*edf, context, stage)
                if let Some(name) = edf_node_name {
                    context.emit_node_function_call(name, stage);
                }
            } else if let Some(eov) = edf_out_var.as_deref() {
                // C++ lines 187-189
                stage.append_line(&format!("        EDF {} = EDF(0.0);", eov));
            }

            if let Some(eov) = edf_out_var.as_deref() {
                // C++ line 192: outColor += edf->getOutput()->getVariable() (no .response for EDF)
                stage.append_line(&format!("        {} += {};", out_color, eov));
            }

            stage.append_line("    }");
            stage.append_line("");
        }

        // === Handle surface transmission and opacity (C++ lines 197-230) ===
        if has_bsdf {
            stage.append_line("    // Calculate the BSDF transmission for viewing direction");

            if bsdf_is_bsdf_t || bsdf_is_vdf {
                // C++ line 205-206
                stage.append_line("    ClosureData closureData = makeClosureData(CLOSURE_TYPE_TRANSMISSION, L, V, N, P, occlusion);");
                // C++ line 206: shadergen.emitFunctionCall(*bsdf, context, stage)
                if let Some(name) = bsdf_node_name {
                    context.emit_node_function_call(name, stage);
                }
            } else if let Some(bov) = bsdf_out_var.as_deref() {
                // C++ lines 210-212: declare default output
                stage.append_line(&format!("    BSDF {} = BSDF(vec3(0.0), vec3(1.0));", bov));
            }

            if let Some(bov) = bsdf_out_var.as_deref() {
                // C++ lines 215-222: transmission render method switch
                // Refraction: add to outColor; Opacity: add to outTransparency.
                if context.get_gen_options().hw_transmission_render_method
                    == HwTransmissionRenderMethod::Refraction
                {
                    stage.append_line(&format!("    {} += {}.response;", out_color, bov));
                } else {
                    stage.append_line(&format!("    {} += {}.response;", out_transparency, bov));
                }
            }

            stage.append_line("");
            stage.append_line("    // Compute and apply surface opacity");
            stage.append_line("    {");
            stage.append_line(&format!("        {} *= surfaceOpacity;", out_color));
            stage.append_line(&format!(
                "        {} = mix(vec3(1.0), {}, surfaceOpacity);",
                out_transparency, out_transparency
            ));
            stage.append_line("    }");
        }

        stage.append_line("}");
        stage.append_line("");
    }

    /// Emit the light loop. C++ HwSurfaceNode::emitLightLoop (lines 237-288).
    /// Iterates active lights, samples each, evaluates BSDF with REFLECTION ClosureData,
    /// accumulates lightShader.intensity * bsdf.response.
    fn emit_light_loop(
        &self,
        _node: &ShaderNode,
        context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
        out_color: &str,
        bsdf_out_var: &Option<String>,
        bsdf_is_bsdf_r: bool,
        bsdf_node_name: Option<&str>,
    ) {
        // C++ line 242: if hwMaxActiveLightSources > 0
        if context.get_gen_options().hw_max_active_light_sources == 0 {
            return;
        }

        stage.append_line("    // Light loop");
        stage.append_line("    int numLights = numActiveLightSources();");
        stage.append_line("    lightshader lightShader;");
        stage.append_line(
            "    for (int activeLightIndex = 0; activeLightIndex < numLights; ++activeLightIndex)",
        );
        stage.append_line("    {");
        stage.append_line(&format!(
            "        sampleLightSource({}[activeLightIndex], {}{}, lightShader);",
            token::T_LIGHT_DATA_INSTANCE,
            "vd.",
            token::T_POSITION_WORLD,
        ));
        stage.append_line("        L = lightShader.direction;");
        stage.append_line("");

        // C++ lines 262-273: evaluate BSDF for this light
        stage.append_line("        // Calculate the BSDF response for this light source");
        if bsdf_is_bsdf_r {
            // C++ line 265-266
            stage.append_line("        ClosureData closureData = makeClosureData(CLOSURE_TYPE_REFLECTION, L, V, N, P, occlusion);");
            // C++ line 266: shadergen.emitFunctionCall(*bsdf, context, stage)
            if let Some(name) = bsdf_node_name {
                context.emit_node_function_call(name, stage);
            }
        } else if let Some(bov) = bsdf_out_var.as_deref() {
            // C++ lines 270-272: declare default BSDF output
            stage.append_line(&format!(
                "        BSDF {} = BSDF(vec3(0.0), vec3(1.0));",
                bov
            ));
        }
        stage.append_line("");

        // C++ line 278: outColor += lightShader.intensity * bsdf.response
        stage.append_line("        // Accumulate the light's contribution");
        if let Some(bov) = bsdf_out_var.as_deref() {
            stage.append_line(&format!(
                "        {} += lightShader.intensity * {}.response;",
                out_color, bov
            ));
        }
        stage.append_line("");

        // C++ line 283: occlusion = 1.0 (reset for next light's shadow)
        stage.append_line("        // Clear shadow factor for next light");
        stage.append_line("        occlusion = 1.0;");
        stage.append_line("    }");
        stage.append_line("");
    }
}

/// Resolve input value for emit -- connection-aware, mirrors C++ emitInput().
/// If the input has an upstream connection, returns the connected variable name.
/// Otherwise returns the literal value string or `default`.
fn resolve_connected_input(
    node: &ShaderNode,
    input_name: &str,
    context: &dyn ShaderImplContext,
    default: &str,
) -> String {
    let input = match node.get_input(input_name) {
        Some(i) => i,
        None => return default.to_string(),
    };
    // Check for upstream connection first (C++ emitInput behavior)
    if let Some((up_node, up_output)) = input.get_connection() {
        if let Some(graph) = context.get_graph() {
            if let Some(var) = graph.get_connection_variable(up_node, up_output) {
                return var;
            }
        }
    }
    // Fallback: literal value
    let val_str = input.port.get_value_string();
    if val_str.is_empty() {
        return default.to_string();
    }
    if let Ok(f) = val_str.parse::<f32>() {
        return format!("{:.1}", f);
    }
    default.to_string()
}
