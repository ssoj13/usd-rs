//! WgslSyntax — WGSL syntax (по рефу MaterialXGenGlsl WgslSyntax).
//! Vulkan GLSL types + WGSL reserved words. Output: #version 450, layout(binding=N).
//! Texture+sampler emitted separately for WebGPU/WGSL transpilation.

use crate::gen_shader::TypeSystem;

use super::glsl_syntax::GlslSyntax;

/// Create WGSL syntax (GlslSyntax + WGSL reserved words).
pub fn create_wgsl_syntax(type_system: TypeSystem) -> GlslSyntax {
    GlslSyntax::create_wgsl(type_system)
}
