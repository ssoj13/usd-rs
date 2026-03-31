//! ResourceBindingContext — trait for emitting uniforms with layout bindings.
//! По рефу MaterialX HwResourceBindingContext.h

use super::{ShaderStage, VariableBlock};

/// Context for emitting resource bindings (layout(binding=N)) for hardware shaders.
pub trait ResourceBindingContext: Send + Sync {
    /// Reset binding counters before generation.
    fn initialize(&mut self);

    /// Emit required directives (#extension etc.) for the stage.
    fn emit_directives(&self, stage: &mut ShaderStage);

    /// Emit uniforms: value uniforms in layout(std140) block, samplers separately.
    fn emit_resource_bindings(
        &mut self,
        uniforms: &VariableBlock,
        stage: &mut ShaderStage,
        uniform_qualifier: &str,
        glsl_type_fn: &dyn Fn(&str) -> &'static str,
    );

    /// Emit structured uniform block (e.g. LightData) with std140.
    fn emit_structured_resource_bindings(
        &mut self,
        uniforms: &VariableBlock,
        stage: &mut ShaderStage,
        struct_instance_name: &str,
        array_suffix: &str,
        uniform_qualifier: &str,
        glsl_type_fn: &dyn Fn(&str) -> &'static str,
    );
}
