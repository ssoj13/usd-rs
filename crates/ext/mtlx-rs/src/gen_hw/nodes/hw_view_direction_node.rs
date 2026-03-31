//! HwViewDirectionNode — view direction vector (по рефу HwViewDirectionNode.cpp).

use crate::gen_hw::hw_constants::{block, get_node_space, space::*, token};
use crate::gen_shader::{
    Shader, ShaderImplContext, ShaderNode, ShaderNodeImpl, ShaderStage, add_stage_input,
    add_stage_output, add_stage_uniform, shader_stage, type_desc_types,
};

/// ViewDirection node for hardware shaders.
#[derive(Debug, Default)]
pub struct HwViewDirectionNode {
    name: String,
    hash: u64,
}

impl HwViewDirectionNode {
    pub fn create() -> Box<dyn ShaderNodeImpl> {
        Box::new(Self::default())
    }
}

impl ShaderNodeImpl for HwViewDirectionNode {
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
                token::T_IN_POSITION,
                vs,
                false,
            );
            add_stage_output(
                block::VERTEX_DATA,
                type_desc.clone(),
                token::T_POSITION_WORLD,
                vs,
                false,
            );
        }
        if let Some(ps) = shader.get_stage_by_name_mut(shader_stage::PIXEL) {
            add_stage_uniform(
                block::PRIVATE_UNIFORMS,
                type_desc_types::vector3(),
                token::T_VIEW_POSITION,
                ps,
            );
            add_stage_input(
                block::VERTEX_DATA,
                type_desc.clone(),
                token::T_POSITION_WORLD,
                ps,
                false,
            );
            if space != WORLD_SPACE {
                add_stage_uniform(
                    block::PRIVATE_UNIFORMS,
                    type_desc_types::matrix44(),
                    token::T_WORLD_INVERSE_TRANSPOSE_MATRIX,
                    ps,
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

        if stage.get_name() == shader_stage::VERTEX {
            let (line, need_emit) = {
                let vertex_data = match stage.get_output_block_mut(block::VERTEX_DATA) {
                    Some(b) => b,
                    None => return,
                };
                if let Some(position) = vertex_data.find_mut(token::T_POSITION_WORLD) {
                    if !position.is_emitted() {
                        position.set_emitted(true);
                        let l = format!("vd.{} = hPositionWorld.xyz;", position.get_variable());
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
            if let Some(position) = vertex_data.find(token::T_POSITION_WORLD) {
                let out_type = crate::gen_shader::source_code_node::glsl_type_name(
                    output.get_type().get_name(),
                );
                let line = if space == WORLD_SPACE {
                    format!(
                        "{} {} = normalize(vd.{} - {});",
                        out_type,
                        output.port.get_variable(),
                        position.get_variable(),
                        token::T_VIEW_POSITION
                    )
                } else {
                    format!(
                        "{} {} = normalize(mx_matrix_mul({}, vec4(vd.{} - {}, 0.0)).xyz);",
                        out_type,
                        output.port.get_variable(),
                        token::T_WORLD_INVERSE_TRANSPOSE_MATRIX,
                        position.get_variable(),
                        token::T_VIEW_POSITION
                    )
                };
                stage.append_line(&line);
            }
        }
    }
}
