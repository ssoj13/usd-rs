//! HwGeomColorNode — vertex color (по рефу HwGeomColorNode.cpp).

use crate::gen_hw::hw_constants::{block, token};
use crate::gen_shader::type_desc_types;
use crate::gen_shader::{
    Shader, ShaderImplContext, ShaderNode, ShaderNodeImpl, ShaderStage, add_stage_input,
    add_stage_output, shader_stage,
};

const INDEX: &str = "index";

/// GeomColor (vertex color) node for hardware shaders.
#[derive(Debug, Default)]
pub struct HwGeomColorNode {
    name: String,
    hash: u64,
}

impl HwGeomColorNode {
    pub fn create() -> Box<dyn ShaderNodeImpl> {
        Box::new(Self::default())
    }

    fn get_index(node: &ShaderNode) -> String {
        node.get_input(INDEX)
            .and_then(|i| i.port.get_value())
            .map(|v| v.get_value_string())
            .unwrap_or_else(|| "0".to_string())
    }
}

impl ShaderNodeImpl for HwGeomColorNode {
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
        let (index, type_desc) = {
            let node = match shader.get_graph().get_node(node_name) {
                Some(n) => n,
                None => return,
            };
            let output = match node.get_outputs().next() {
                Some(o) => o,
                None => return,
            };
            (Self::get_index(node), output.get_type().clone())
        };
        let _ = type_desc; // C++: always uses COLOR4 for both input and connector
        let in_name = format!("{}_{}", token::T_IN_COLOR, index);
        let color_name = format!("{}_{}", token::T_COLOR, index);

        if let Some(vs) = shader.get_stage_by_name_mut(shader_stage::VERTEX) {
            add_stage_input(
                block::VERTEX_INPUTS,
                type_desc_types::color4(),
                &in_name,
                vs,
                false,
            );
            // C++: always uses Type::COLOR4 for connector, regardless of output type
            add_stage_output(
                block::VERTEX_DATA,
                type_desc_types::color4(),
                &color_name,
                vs,
                false,
            );
        }
        if let Some(ps) = shader.get_stage_by_name_mut(shader_stage::PIXEL) {
            // C++: always uses Type::COLOR4 for pixel stage input connector
            add_stage_input(
                block::VERTEX_DATA,
                type_desc_types::color4(),
                &color_name,
                ps,
                false,
            );
        }
    }

    fn emit_function_call(
        &self,
        node: &ShaderNode,
        _context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
    ) {
        let index = Self::get_index(node);
        let variable = format!("{}_{}", token::T_COLOR, index);
        let output = match node.get_outputs().next() {
            Some(o) => o,
            None => return,
        };
        let type_name = output.get_type().get_name();
        let suffix = match type_name {
            "float" => ".r",
            "color3" => ".rgb",
            _ => "",
        };

        if stage.get_name() == shader_stage::VERTEX {
            let (line, need_emit) = {
                let vertex_data = match stage.get_output_block_mut(block::VERTEX_DATA) {
                    Some(b) => b,
                    None => return,
                };
                if let Some(color) = vertex_data.find_mut(&variable) {
                    if !color.is_emitted() {
                        color.set_emitted(true);
                        let in_name = format!("{}_{}", token::T_IN_COLOR, index);
                        let l = format!("vd.{} = {};", color.get_variable(), in_name);
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
            if let Some(color) = vertex_data.find(&variable) {
                let out_type = crate::gen_shader::source_code_node::glsl_type_name(
                    output.get_type().get_name(),
                );
                let line = format!(
                    "{} {} = vd.{}{};",
                    out_type,
                    output.port.get_variable(),
                    color.get_variable(),
                    suffix
                );
                stage.append_line(&line);
            }
        }
    }
}
