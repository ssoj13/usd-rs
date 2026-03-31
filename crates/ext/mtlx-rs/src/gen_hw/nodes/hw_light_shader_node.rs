//! HwLightShaderNode -- source-code-backed individual light type implementation.
//! Based on MaterialX HwLightShaderNode.h/cpp.
//!
//! Handles point/directional/spot light implementations backed by a GLSL/MSL source file.
//! Creates light uniforms from the NodeDef inputs and emits `lightFunc(light, position, result)`.

use crate::core::ElementPtr;
use crate::gen_hw::hw_constants::{block, token};
use crate::gen_shader::{
    Shader, ShaderImplContext, ShaderNode, ShaderNodeImpl, ShaderStage, SourceCodeNode,
    VariableBlock, add_stage_uniform, add_stage_uniform_with_value, shader_stage, type_desc_types,
};

/// Source-code-backed light shader node -- handles individual light type implementations.
///
/// Extends SourceCodeNode: uses the GLSL/MSL source file from the Implementation element,
/// but also collects all NodeDef inputs as `LightData` block uniforms and adds lighting
/// uniforms (u_numActiveLightSources, MAX_LIGHT_SOURCES) to the pixel stage.
pub struct HwLightShaderNode {
    /// Delegate to SourceCodeNode for file/function-name loading.
    inner: Box<dyn ShaderNodeImpl>,
    /// Light-specific uniforms collected from NodeDef inputs during initialize.
    /// Mirrors C++ `_lightUniforms` (VariableBlock with block name = HW::LIGHT_DATA).
    light_uniforms: VariableBlock,
    /// Cached function name for emit_function_call (e.g. "mx_point_light").
    function_name: String,
}

impl std::fmt::Debug for HwLightShaderNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HwLightShaderNode")
            .field("name", &self.inner.get_name())
            .field("light_uniforms_count", &self.light_uniforms.size())
            .finish()
    }
}

impl HwLightShaderNode {
    pub fn create() -> Box<dyn ShaderNodeImpl> {
        Box::new(Self {
            inner: SourceCodeNode::create(),
            light_uniforms: VariableBlock::new(block::LIGHT_DATA, ""),
            function_name: String::new(),
        })
    }
}

impl ShaderNodeImpl for HwLightShaderNode {
    fn get_name(&self) -> &str {
        self.inner.get_name()
    }

    fn get_hash(&self) -> u64 {
        self.inner.get_hash()
    }

    /// Initialize: delegates to SourceCodeNode, then collects light uniforms from NodeDef inputs.
    ///
    /// C++ validates element is Implementation and not inlined. Here we delegate and capture
    /// the function name for later emit. Collects NodeDef inputs as light uniforms.
    fn initialize(&mut self, element: &ElementPtr, context: &dyn ShaderImplContext) {
        // SourceCodeNode::initialize reads 'file', 'function', 'sourcecode' attributes.
        self.inner.initialize(element, context);

        // Cache function name for emit_function_call (C++ uses _functionName from SourceCodeNode).
        self.function_name = element
            .borrow()
            .get_attribute_or_empty("function")
            .to_string();

        // Collect light uniforms from NodeDef inputs.
        // C++: impl.getNodeDef()->getActiveInputs() -> _lightUniforms.add(type, name, value)
        if let Some(node_def) = crate::core::get_declaration(element) {
            let type_system = context.get_type_system();
            for input in crate::core::get_active_inputs(&node_def) {
                let inp = input.borrow();
                let name = inp.get_name().to_string();
                let ty_str = inp
                    .get_type()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "float".to_string());
                let type_desc = type_system.get_type(&ty_str);
                // C++ passes input->getValue() as well; parse value string if present.
                let value = if inp.has_value_string() {
                    crate::core::Value::from_strings(&inp.get_value_string(), &ty_str)
                } else {
                    None
                };
                self.light_uniforms.add(type_desc, name, value, false);
            }
        }
    }

    /// Register light uniforms into the LightData block of the pixel stage.
    ///
    /// C++ iterates _lightUniforms and adds each to ps.getUniformBlock(HW::LIGHT_DATA).
    /// Also calls shadergen.addStageLightingUniforms() to add numActiveLightSources etc.
    fn create_variables(
        &self,
        node_name: &str,
        context: &dyn ShaderImplContext,
        shader: &mut Shader,
    ) {
        // Delegate inner create_variables first (SourceCodeNode has no-op, but keep for subclass correctness).
        self.inner.create_variables(node_name, context, shader);

        let ps = match shader.get_stage_by_name_mut(shader_stage::PIXEL) {
            Some(s) => s,
            None => return,
        };

        // Add each collected light uniform to the LightData block.
        // C++: for each u in _lightUniforms -> lightData.add(u->getType(), u->getName())
        for i in 0..self.light_uniforms.size() {
            if let Some(port) = self.light_uniforms.get(i) {
                let ty = port.get_type().clone();
                let name = port.get_name().to_string();
                add_stage_uniform(block::LIGHT_DATA, ty, &name, ps);
            }
        }

        // C++: shadergen.addStageLightingUniforms(context, ps)
        // Adds u_numActiveLightSources integer uniform to PrivateUniforms block.
        let port = add_stage_uniform_with_value(
            block::PRIVATE_UNIFORMS,
            type_desc_types::integer(),
            token::T_NUM_ACTIVE_LIGHT_SOURCES,
            ps,
            Some(crate::core::Value::Integer(0)),
        );
        port.set_value(Some(crate::core::Value::Integer(0)), false);
    }

    /// Emit the light function definition -- delegates to SourceCodeNode (emits the GLSL source file).
    fn emit_function_definition(
        &self,
        node: &ShaderNode,
        context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
    ) {
        self.inner.emit_function_definition(node, context, stage);
    }

    /// Emit the light function call: `lightFunc(light, position, result);`
    ///
    /// C++: shadergen.emitLine(_functionName + "(light, position, result)", stage)
    /// Only emits for the pixel stage.
    fn emit_function_call(
        &self,
        _node: &ShaderNode,
        _context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
    ) {
        if stage.get_name() != shader_stage::PIXEL {
            return;
        }
        let fname = if self.function_name.is_empty() {
            self.inner.get_name()
        } else {
            &self.function_name
        };
        stage.append_line(&format!("{}(light, position, result);", fname));
    }

    fn emit_output_variables(
        &self,
        node: &ShaderNode,
        context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
    ) {
        self.inner.emit_output_variables(node, context, stage);
    }
}
