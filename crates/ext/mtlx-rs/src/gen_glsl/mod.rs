//! MaterialXGenGlsl — GLSL shader generation.

mod essl_shader_generator;
mod glsl_emit;
pub mod glsl_family;
pub use glsl_emit::{emit_function_calls, emit_function_definitions, token_substitutions};
mod glsl_resource_binding_context;
mod glsl_shader_generator;
mod glsl_syntax;
mod vk_resource_binding_context;
mod vk_shader_generator;
mod wgsl_resource_binding_context;
mod wgsl_shader_generator;
mod wgsl_syntax;

pub use essl_shader_generator::{
    EsslShaderGenerator, EsslShaderGraphContext, TARGET as ESSL_TARGET, VERSION as ESSL_VERSION,
};
pub use glsl_resource_binding_context::GlslResourceBindingContext;
pub use glsl_shader_generator::{GlslShaderGenerator, GlslShaderGraphContext, TARGET, VERSION};
pub use glsl_syntax::GlslSyntax;
pub use vk_resource_binding_context::VkResourceBindingContext;
pub use vk_shader_generator::{
    TARGET as VK_TARGET, VERSION as VK_VERSION, VkShaderGenerator, VkShaderGraphContext,
};
pub use wgsl_resource_binding_context::WgslResourceBindingContext;
pub use wgsl_shader_generator::{
    TARGET as WGSL_TARGET, VERSION as WGSL_VERSION, WgslShaderGenerator, WgslShaderGraphContext,
};
pub use wgsl_syntax::create_wgsl_syntax;
