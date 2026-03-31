//! MaterialXGenWgsl — native WGSL shader generation via naga transpilation.
//!
//! This module provides a real WGSL shader generator by delegating MaterialX
//! codegen to the existing VkShaderGenerator (Vulkan GLSL 450) and then
//! transpiling the result to native WGSL using naga.
//!
//! The old gen_glsl::WgslShaderGenerator remains available for Vulkan-GLSL
//! workflows. This module is the preferred path for wgpu/WebGPU rendering.

mod naga_transpiler;
mod naga_wgsl_shader_generator;

pub use naga_transpiler::{ShaderStage, TranspileError, glsl_to_wgsl, preprocess_mtlx_glsl};
pub use naga_wgsl_shader_generator::{
    NagaWgslShaderGenerator, TARGET as WGSL_NATIVE_TARGET, VERSION as WGSL_NATIVE_VERSION,
};
