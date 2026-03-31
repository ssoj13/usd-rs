//! HwPositionNode — position in object or world space (по рефу HwPositionNode.cpp).

use crate::gen_hw::hw_constants::{block, get_node_space, space::*, token};
use crate::gen_shader::{
    Shader, ShaderImplContext, ShaderNode, ShaderNodeImpl, ShaderStage, add_stage_input,
    add_stage_output, shader_stage,
};

/// Position node for hardware shaders.
#[derive(Debug, Default)]
pub struct HwPositionNode {
    name: String,
    hash: u64,
}

impl HwPositionNode {
    pub fn create() -> Box<dyn ShaderNodeImpl> {
        Box::new(Self::default())
    }
}

impl ShaderNodeImpl for HwPositionNode {
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
        let (type_desc, space) = {
            let node = match shader.get_graph().get_node(node_name) {
                Some(n) => n,
                None => return,
            };
            let output = match node.get_outputs().next() {
                Some(o) => o,
                None => return,
            };
            (
                output.get_type().clone(),
                get_node_space(node, OBJECT_SPACE),
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
            let var_name = if space == WORLD_SPACE {
                token::T_POSITION_WORLD
            } else {
                token::T_POSITION_OBJECT
            };
            add_stage_output(block::VERTEX_DATA, type_desc.clone(), var_name, vs, false);
        }
        if let Some(ps) = shader.get_stage_by_name_mut(shader_stage::PIXEL) {
            let var_name = if space == WORLD_SPACE {
                token::T_POSITION_WORLD
            } else {
                token::T_POSITION_OBJECT
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
            token::T_POSITION_WORLD
        } else {
            token::T_POSITION_OBJECT
        };

        if stage.get_name() == shader_stage::VERTEX {
            let (line, need_emit) = {
                let vertex_data = match stage.get_output_block_mut(block::VERTEX_DATA) {
                    Some(b) => b,
                    None => return,
                };
                if let Some(position) = vertex_data.find_mut(var_name) {
                    if !position.is_emitted() {
                        position.set_emitted(true);
                        let l = if space == WORLD_SPACE {
                            format!("vd.{} = hPositionWorld.xyz;", position.get_variable())
                        } else {
                            format!("vd.{} = {};", position.get_variable(), token::T_IN_POSITION)
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
            if let Some(position) = vertex_data.find(var_name) {
                let out_type = crate::gen_shader::source_code_node::glsl_type_name(
                    output.get_type().get_name(),
                );
                let line = format!(
                    "{} {} = vd.{};",
                    out_type,
                    output.port.get_variable(),
                    position.get_variable()
                );
                stage.append_line(&line);
            }
        }
    }
}
