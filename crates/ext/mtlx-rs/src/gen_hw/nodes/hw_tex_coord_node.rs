//! HwTexCoordNode — texture coordinate node (по рефу HwTexCoordNode.cpp).
use crate::gen_hw::hw_constants::block;
use crate::gen_hw::hw_constants::token;
use crate::gen_shader::{
    Shader, ShaderImplContext, ShaderNode, ShaderNodeImpl, ShaderStage, add_stage_input,
    add_stage_output, shader_stage,
};

const INDEX: &str = "index";

/// TexCoord node for hardware shaders.
#[derive(Debug, Default)]
pub struct HwTexCoordNode {
    name: String,
    hash: u64,
}

impl HwTexCoordNode {
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

impl ShaderNodeImpl for HwTexCoordNode {
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
        let (type_desc, index) = {
            let node = match shader.get_graph().get_node(node_name) {
                Some(n) => n,
                None => return,
            };
            let output = match node.get_outputs().next() {
                Some(o) => o,
                None => return,
            };
            (output.get_type().clone(), Self::get_index(node))
        };
        let in_name = format!("{}_{}", token::T_IN_TEXCOORD, index);
        let coord_name = format!("{}_{}", token::T_TEXCOORD, index);

        if let Some(vs) = shader.get_stage_by_name_mut(shader_stage::VERTEX) {
            add_stage_input(block::VERTEX_INPUTS, type_desc.clone(), &in_name, vs, true);
            add_stage_output(block::VERTEX_DATA, type_desc.clone(), &coord_name, vs, true);
        }
        if let Some(ps) = shader.get_stage_by_name_mut(shader_stage::PIXEL) {
            add_stage_input(block::VERTEX_DATA, type_desc, &coord_name, ps, true);
        }
    }

    fn emit_function_call(
        &self,
        node: &ShaderNode,
        _context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
    ) {
        let index = Self::get_index(node);
        let variable = format!("{}_{}", token::T_TEXCOORD, index);
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
                let texcoord = match vertex_data.find_mut(&variable) {
                    Some(p) => p,
                    None => return,
                };
                if texcoord.is_emitted() {
                    (String::new(), false)
                } else {
                    texcoord.set_emitted(true);
                    let l = format!(
                        "vd.{} = {}_{};",
                        texcoord.get_variable(),
                        token::T_IN_TEXCOORD,
                        index
                    );
                    (l, true)
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
            let texcoord = match vertex_data.find(&variable) {
                Some(p) => p,
                None => return,
            };
            let type_name = output.get_type().get_name();
            let suffix = if type_name == "vector2" {
                ".xy"
            } else if type_name == "vector3" {
                ".xyz"
            } else {
                ""
            };
            let out_var = output.port.get_variable();
            // C++ emitOutput(node.getOutput(), true, false, ...) — includeDeclaration=true
            let out_type = crate::gen_shader::source_code_node::glsl_type_name(type_name);
            let line = format!(
                "{} {} = vd.{}{};",
                out_type,
                out_var,
                texcoord.get_variable(),
                suffix
            );
            stage.append_line(&line);
        }
    }
}
