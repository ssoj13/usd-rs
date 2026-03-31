//! HwBitangentNode — bitangent in object or world space (по рефу HwBitangentNode.cpp).

use crate::gen_hw::hw_constants::{block, get_node_space, space::*, token};
use crate::gen_shader::{
    Shader, ShaderImplContext, ShaderNode, ShaderNodeImpl, ShaderStage, add_stage_input,
    add_stage_output, add_stage_uniform, shader_stage, type_desc_types,
};

/// Bitangent node for hardware shaders. Uses hwImplicitBitangents=true (default).
#[derive(Debug, Default)]
pub struct HwBitangentNode {
    name: String,
    hash: u64,
}

impl HwBitangentNode {
    pub fn create() -> Box<dyn ShaderNodeImpl> {
        Box::new(Self::default())
    }
}

impl ShaderNodeImpl for HwBitangentNode {
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

    fn create_variables(
        &self,
        node_name: &str,
        context: &dyn ShaderImplContext,
        shader: &mut Shader,
    ) {
        let (space, type_desc) = {
            let node = match shader.get_graph().get_node(node_name) {
                Some(n) => n,
                None => return,
            };
            let output = match node.get_outputs().next() {
                Some(o) => o,
                None => return,
            };
            (
                get_node_space(node, OBJECT_SPACE),
                output.get_type().clone(),
            )
        };

        // C++: reads options.hwImplicitBitangents from GenOptions (default: true)
        let implicit = context.get_gen_options().hw_implicit_bitangents;

        if let Some(vs) = shader.get_stage_by_name_mut(shader_stage::VERTEX) {
            if implicit {
                add_stage_input(
                    block::VERTEX_INPUTS,
                    type_desc.clone(),
                    token::T_IN_NORMAL,
                    vs,
                    false,
                );
                add_stage_input(
                    block::VERTEX_INPUTS,
                    type_desc.clone(),
                    token::T_IN_TANGENT,
                    vs,
                    false,
                );
            } else {
                add_stage_input(
                    block::VERTEX_INPUTS,
                    type_desc.clone(),
                    token::T_IN_BITANGENT,
                    vs,
                    false,
                );
            }
            if space == WORLD_SPACE {
                add_stage_uniform(
                    block::PRIVATE_UNIFORMS,
                    type_desc_types::matrix44(),
                    token::T_WORLD_MATRIX,
                    vs,
                );
                if implicit {
                    add_stage_uniform(
                        block::PRIVATE_UNIFORMS,
                        type_desc_types::matrix44(),
                        token::T_WORLD_INVERSE_TRANSPOSE_MATRIX,
                        vs,
                    );
                    add_stage_output(
                        block::VERTEX_DATA,
                        type_desc.clone(),
                        token::T_NORMAL_WORLD,
                        vs,
                        false,
                    );
                    add_stage_output(
                        block::VERTEX_DATA,
                        type_desc.clone(),
                        token::T_TANGENT_WORLD,
                        vs,
                        false,
                    );
                }
                add_stage_output(
                    block::VERTEX_DATA,
                    type_desc.clone(),
                    token::T_BITANGENT_WORLD,
                    vs,
                    false,
                );
            } else {
                add_stage_output(
                    block::VERTEX_DATA,
                    type_desc.clone(),
                    token::T_BITANGENT_OBJECT,
                    vs,
                    false,
                );
            }
        }
        if let Some(ps) = shader.get_stage_by_name_mut(shader_stage::PIXEL) {
            let var_name = if space == WORLD_SPACE {
                token::T_BITANGENT_WORLD
            } else {
                token::T_BITANGENT_OBJECT
            };
            add_stage_input(block::VERTEX_DATA, type_desc, var_name, ps, false);
            if implicit && space == WORLD_SPACE {
                add_stage_input(
                    block::VERTEX_DATA,
                    type_desc_types::vector3(),
                    token::T_NORMAL_WORLD,
                    ps,
                    false,
                );
                add_stage_input(
                    block::VERTEX_DATA,
                    type_desc_types::vector3(),
                    token::T_TANGENT_WORLD,
                    ps,
                    false,
                );
            }
        }
    }

    fn emit_function_call(
        &self,
        node: &ShaderNode,
        _context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
    ) {
        let space = get_node_space(node, OBJECT_SPACE);
        let output = match node.get_outputs().next() {
            Some(o) => o,
            None => return,
        };
        let var_name = if space == WORLD_SPACE {
            token::T_BITANGENT_WORLD
        } else {
            token::T_BITANGENT_OBJECT
        };

        // C++: reads options.hwImplicitBitangents from GenOptions
        let implicit = _context.get_gen_options().hw_implicit_bitangents;

        if stage.get_name() == shader_stage::VERTEX {
            let mut lines_to_emit: Vec<String> = Vec::new();
            {
                let vertex_data = match stage.get_output_block_mut(block::VERTEX_DATA) {
                    Some(b) => b,
                    None => return,
                };
                if let Some(bitangent) = vertex_data.find_mut(var_name) {
                    if bitangent.is_emitted() {
                        return;
                    }
                    bitangent.set_emitted(true);
                    let bvar = bitangent.get_variable().to_string();

                    if space == WORLD_SPACE {
                        if implicit {
                            if let Some(normal) = vertex_data.find_mut(token::T_NORMAL_WORLD) {
                                if !normal.is_emitted() {
                                    normal.set_emitted(true);
                                    lines_to_emit.push(format!(
                                        "vd.{} = normalize(mx_matrix_mul({}, vec4({}, 0.0)).xyz);",
                                        normal.get_variable(),
                                        token::T_WORLD_INVERSE_TRANSPOSE_MATRIX,
                                        token::T_IN_NORMAL
                                    ));
                                }
                            }
                            if let Some(tangent) = vertex_data.find_mut(token::T_TANGENT_WORLD) {
                                if !tangent.is_emitted() {
                                    tangent.set_emitted(true);
                                    lines_to_emit.push(format!(
                                        "vd.{} = normalize(mx_matrix_mul({}, vec4({}, 0.0)).xyz);",
                                        tangent.get_variable(),
                                        token::T_WORLD_MATRIX,
                                        token::T_IN_TANGENT
                                    ));
                                }
                            }
                            lines_to_emit.push(format!(
                                "vd.{} = cross(vd.{}, vd.{});",
                                bvar,
                                token::T_NORMAL_WORLD,
                                token::T_TANGENT_WORLD
                            ));
                        } else {
                            lines_to_emit.push(format!(
                                "vd.{} = normalize(mx_matrix_mul({}, vec4({}, 0.0)).xyz);",
                                bvar,
                                token::T_WORLD_MATRIX,
                                token::T_IN_BITANGENT
                            ));
                        }
                    } else {
                        if implicit {
                            lines_to_emit.push(format!(
                                "vd.{} = cross({}, {});",
                                bvar,
                                token::T_IN_NORMAL,
                                token::T_IN_TANGENT
                            ));
                        } else {
                            lines_to_emit.push(format!("vd.{} = {};", bvar, token::T_IN_BITANGENT));
                        }
                    }
                }
            }
            for line in lines_to_emit {
                stage.append_line(&line);
            }
        } else if stage.get_name() == shader_stage::PIXEL {
            let vertex_data = match stage.get_input_block(block::VERTEX_DATA) {
                Some(b) => b,
                None => return,
            };
            if let Some(bitangent) = vertex_data.find(var_name) {
                let out_type = crate::gen_shader::source_code_node::glsl_type_name(
                    output.get_type().get_name(),
                );
                let line = format!(
                    "{} {} = normalize(vd.{});",
                    out_type,
                    output.port.get_variable(),
                    bitangent.get_variable()
                );
                stage.append_line(&line);
            }
        }
    }
}
