//! HwTangentNode — tangent in object or world space (по рефу HwTangentNode.cpp).

use crate::gen_hw::hw_constants::{block, get_node_space, space::*, token};
use crate::gen_shader::{
    Shader, ShaderImplContext, ShaderNode, ShaderNodeImpl, ShaderStage, add_stage_input,
    add_stage_output, add_stage_uniform, shader_stage, type_desc_types,
};

/// Tangent node for hardware shaders.
#[derive(Debug, Default)]
pub struct HwTangentNode {
    name: String,
    hash: u64,
}

impl HwTangentNode {
    pub fn create() -> Box<dyn ShaderNodeImpl> {
        Box::new(Self::default())
    }
}

impl ShaderNodeImpl for HwTangentNode {
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
        _context: &dyn ShaderImplContext,
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

        if let Some(vs) = shader.get_stage_by_name_mut(shader_stage::VERTEX) {
            add_stage_input(
                block::VERTEX_INPUTS,
                type_desc.clone(),
                token::T_IN_TANGENT,
                vs,
                false,
            );
            if space == WORLD_SPACE {
                add_stage_uniform(
                    block::PRIVATE_UNIFORMS,
                    type_desc_types::matrix44(),
                    token::T_WORLD_MATRIX,
                    vs,
                );
                add_stage_output(
                    block::VERTEX_DATA,
                    type_desc.clone(),
                    token::T_TANGENT_WORLD,
                    vs,
                    false,
                );
            } else {
                add_stage_output(
                    block::VERTEX_DATA,
                    type_desc.clone(),
                    token::T_TANGENT_OBJECT,
                    vs,
                    false,
                );
            }
        }
        if let Some(ps) = shader.get_stage_by_name_mut(shader_stage::PIXEL) {
            let var_name = if space == WORLD_SPACE {
                token::T_TANGENT_WORLD
            } else {
                token::T_TANGENT_OBJECT
            };
            add_stage_input(block::VERTEX_DATA, type_desc, var_name, ps, false);
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
            token::T_TANGENT_WORLD
        } else {
            token::T_TANGENT_OBJECT
        };

        if stage.get_name() == shader_stage::VERTEX {
            let (line, need_emit) = {
                let vertex_data = match stage.get_output_block_mut(block::VERTEX_DATA) {
                    Some(b) => b,
                    None => return,
                };
                if let Some(tangent) = vertex_data.find_mut(var_name) {
                    if !tangent.is_emitted() {
                        tangent.set_emitted(true);
                        let l = if space == WORLD_SPACE {
                            format!(
                                "vd.{} = normalize(mx_matrix_mul({}, vec4({}, 0.0)).xyz);",
                                tangent.get_variable(),
                                token::T_WORLD_MATRIX,
                                token::T_IN_TANGENT
                            )
                        } else {
                            format!("vd.{} = {};", tangent.get_variable(), token::T_IN_TANGENT)
                        };
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
        } else if stage.get_name() == shader_stage::PIXEL {
            let vertex_data = match stage.get_input_block(block::VERTEX_DATA) {
                Some(b) => b,
                None => return,
            };
            if let Some(tangent) = vertex_data.find(var_name) {
                // C++ emitOutput(node.getOutput(), true, false, ...) — includeDeclaration=true
                let out_type = crate::gen_shader::source_code_node::glsl_type_name(
                    output.get_type().get_name(),
                );
                let line = format!(
                    "{} {} = normalize(vd.{});",
                    out_type,
                    output.port.get_variable(),
                    tangent.get_variable()
                );
                stage.append_line(&line);
            }
        }
    }
}
