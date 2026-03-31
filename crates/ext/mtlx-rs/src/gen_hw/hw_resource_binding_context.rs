//! HwResourceBindingContext -- context for resource binding in HW shaders.
//! Based on MaterialX HwResourceBindingContext.h.
//!
//! Provides a trait for emitting resource binding directives, uniforms with
//! binding info (layout(binding=N)), and structured uniform blocks.
//! Concrete implementations (e.g. VkResourceBindingContext) implement this trait.

use crate::gen_shader::{ShaderStage, VariableBlock};

/// Context for resource binding in hardware shader generation (C++ HwResourceBindingContext).
///
/// Implementations provide target-specific (Vulkan, Metal, etc.) resource binding
/// directives and uniform emission with binding layout annotations.
pub trait HwResourceBindingContext {
    /// Initialize the context before code generation starts.
    fn initialize(&mut self);

    /// Emit directives required for binding support (e.g. #extension, #version).
    fn emit_directives(&self, stage: &mut ShaderStage);

    /// Emit uniforms with binding information (e.g. layout(binding=N) uniform ...).
    fn emit_resource_bindings(&self, uniforms: &VariableBlock, stage: &mut ShaderStage);

    /// Emit structured (struct) uniforms with binding information.
    ///
    /// `struct_instance_name` is the variable name for the struct instance.
    /// `array_suffix` is an optional array dimension suffix (e.g. "[MAX_LIGHT_SOURCES]").
    fn emit_structured_resource_bindings(
        &self,
        uniforms: &VariableBlock,
        stage: &mut ShaderStage,
        struct_instance_name: &str,
        array_suffix: &str,
    );
}
