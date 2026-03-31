//! HwTimeNode — time uniform (по рефу HwTimeNode.cpp).

use crate::gen_hw::hw_constants::block;
use crate::gen_hw::hw_constants::token;
use crate::gen_shader::{
    Shader, ShaderImplContext, ShaderNode, ShaderNodeImpl, ShaderStage, add_stage_uniform,
    shader_stage, type_desc_types,
};

/// Time uniform node.
#[derive(Debug, Default)]
pub struct HwTimeNode {
    name: String,
    hash: u64,
}

impl HwTimeNode {
    pub fn create() -> Box<dyn ShaderNodeImpl> {
        Box::new(Self::default())
    }
}

impl ShaderNodeImpl for HwTimeNode {
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
        _node_name: &str,
        _context: &dyn ShaderImplContext,
        shader: &mut Shader,
    ) {
        if let Some(ps) = shader.get_stage_by_name_mut(shader_stage::PIXEL) {
            add_stage_uniform(
                block::PRIVATE_UNIFORMS,
                type_desc_types::float(),
                token::T_TIME,
                ps,
            );
        }
    }

    fn emit_function_call(
        &self,
        node: &ShaderNode,
        _context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
    ) {
        if stage.get_name() != shader_stage::PIXEL {
            return;
        }
        let output = match node.get_outputs().next() {
            Some(o) => o,
            None => return,
        };
        let line = format!("{} = {}", output.port.get_variable(), token::T_TIME);
        stage.append_line(&line);
    }
}
