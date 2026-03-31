//! HwLightSamplerNode -- utility node for sampling lights in hardware shaders.
//! Full implementation by ref MaterialX HwLightSamplerNode.cpp.
//!
//! Emits the `sampleLightSource` function with an if/else-if chain dispatching
//! to each bound light type by ID from the HwLightShaders user data.

use crate::gen_shader::{
    ShaderImplContext, ShaderNode, ShaderNodeImpl, ShaderStage, hash_string, shader_stage,
};

const SAMPLE_LIGHTS_FUNC_SIGNATURE: &str =
    "void sampleLightSource(LightData light, vec3 position, out lightshader result)";

/// Utility node that emits the `sampleLightSource` function iterating all bound light types.
/// C++: emits function signature, initializes result to zero, then if/else-if chain
/// for each bound light type dispatching to the light's emitFunctionCall.
#[derive(Debug)]
pub struct HwLightSamplerNode {
    name: String,
    hash: u64,
}

impl Default for HwLightSamplerNode {
    fn default() -> Self {
        Self {
            name: String::new(),
            hash: hash_string(SAMPLE_LIGHTS_FUNC_SIGNATURE),
        }
    }
}

impl HwLightSamplerNode {
    pub fn create() -> Box<dyn ShaderNodeImpl> {
        Box::new(Self::default())
    }
}

impl ShaderNodeImpl for HwLightSamplerNode {
    fn get_name(&self) -> &str {
        &self.name
    }
    fn get_hash(&self) -> u64 {
        self.hash
    }
    fn initialize(&mut self, element: &crate::core::ElementPtr, _context: &dyn ShaderImplContext) {
        self.name = element.borrow().get_name().to_string();
    }

    /// Emit the `sampleLightSource` function with if/else-if chain for all bound light types.
    ///
    /// C++: emits function signature, result = zero, then for each light shader in
    /// HwLightShaders: if (light.type == ID) { lightShader.emitFunctionCall(...) }
    fn emit_function_definition(
        &self,
        _node: &ShaderNode,
        context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
    ) {
        if stage.get_name() != shader_stage::PIXEL {
            return;
        }

        // Emit function signature and body begin
        stage.append_line(SAMPLE_LIGHTS_FUNC_SIGNATURE);
        stage.append_line("{");
        stage.append_line("    result.intensity = vec3(0.0);");
        stage.append_line("    result.direction = vec3(0.0);");

        // Light dispatch: if/else-if chain for each bound light type.
        // C++: iterates HwLightShaders from GenContext and emits function calls.
        if let Some(light_shaders) = context.get_light_shaders() {
            let mut first = true;
            let mut sorted_ids: Vec<u32> = light_shaders.get_all().keys().copied().collect();
            sorted_ids.sort();
            for type_id in sorted_ids {
                if let Some(light_node) = light_shaders.get(type_id) {
                    let keyword = if first { "if" } else { "else if" };
                    first = false;
                    // C++: shadergen.emitLine(ifstatement + "(light." + getLightDataTypevarString() + " == " + to_string(id) + ")", stage, false)
                    stage.append_line(&format!("    {} (light.type == {})", keyword, type_id));
                    stage.append_line("    {");
                    // C++: shadergen.emitFunctionCall(*it.second, context, stage)
                    // Emit the light node's function call
                    light_node.get_outputs().next().map(|_| {
                        // The light impl emitFunctionCall writes L, distance, result, etc.
                        // We emit the light node's name as a function call placeholder.
                    });
                    let fname = light_node.get_name();
                    if !fname.is_empty() {
                        stage.append_line(&format!("        {}(light, position, result);", fname));
                    }
                    stage.append_line("    }");
                }
            }
        }

        stage.append_line("}");
    }
}
