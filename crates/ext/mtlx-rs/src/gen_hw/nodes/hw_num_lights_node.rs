//! HwNumLightsNode — returns the number of active light sources.
//! Based on MaterialX HwNumLightsNode.cpp.
//! Emits a function `int numActiveLightSources()` that returns the uniform clamped to max.

use crate::gen_hw::hw_constants::{block, ident, token};
use crate::gen_shader::{
    Shader, ShaderImplContext, ShaderNode, ShaderNodeImpl, ShaderStage, add_stage_uniform,
    shader_stage, type_desc_types,
};

const NUM_LIGHTS_FUNC_SIGNATURE: &str = "int numActiveLightSources()";

/// Returns the number of active HW light sources as a uniform-backed function.
#[derive(Debug, Default)]
pub struct HwNumLightsNode {
    name: String,
    hash: u64,
}

impl HwNumLightsNode {
    pub fn create() -> Box<dyn ShaderNodeImpl> {
        Box::new(Self {
            name: String::new(),
            // Hash based on function signature — matches C++ constructor
            hash: crate::gen_shader::hash_string(NUM_LIGHTS_FUNC_SIGNATURE),
        })
    }
}

impl ShaderNodeImpl for HwNumLightsNode {
    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_hash(&self) -> u64 {
        self.hash
    }

    fn initialize(&mut self, element: &crate::core::ElementPtr, _context: &dyn ShaderImplContext) {
        self.name = element.borrow().get_name().to_string();
        // Keep signature-based hash — do NOT overwrite with name hash.
        // C++ sets _hash = hash(NUM_LIGHTS_FUNC_SIGNATURE) in constructor.
    }

    /// Register the `u_numActiveLightSources` uniform in the pixel stage.
    fn create_variables(
        &self,
        _node_name: &str,
        _context: &dyn ShaderImplContext,
        shader: &mut Shader,
    ) {
        if let Some(ps) = shader.get_stage_by_name_mut(shader_stage::PIXEL) {
            let port = add_stage_uniform(
                block::PRIVATE_UNIFORMS,
                type_desc_types::integer(),
                token::T_NUM_ACTIVE_LIGHT_SOURCES,
                ps,
            );
            // Default: zero active lights
            port.set_value(Some(crate::core::Value::Integer(0)), false);
        }
    }

    /// Emit `int numActiveLightSources() { return min($numActiveLightSources, MAX_LIGHT_SOURCES); }`
    fn emit_function_definition(
        &self,
        _node: &ShaderNode,
        _context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
    ) {
        if stage.get_name() != shader_stage::PIXEL {
            return;
        }
        stage.append_line(NUM_LIGHTS_FUNC_SIGNATURE);
        stage.append_line("{");
        stage.append_line(&format!(
            "    return min({}, {});",
            token::T_NUM_ACTIVE_LIGHT_SOURCES,
            ident::LIGHT_DATA_MAX_LIGHT_SOURCES,
        ));
        stage.append_line("}");
    }

    /// Emit the call: assign output = numActiveLightSources()
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
        stage.append_line(&format!(
            "{} = numActiveLightSources();",
            output.port.get_variable()
        ));
    }
}
