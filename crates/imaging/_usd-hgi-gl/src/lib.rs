//! HgiGL - OpenGL Backend for Hydra Graphics Interface
//!
//! This module provides an OpenGL implementation of the HGI trait system.
//! It allows Hydra to render using OpenGL 4.5+ on systems that support it.
//!
//! # Overview
//!
//! HgiGL implements the HGI trait to provide:
//! - GPU resource management (buffers, textures, shaders)
//! - Graphics and compute pipeline state
//! - Command buffer recording for deferred execution
//! - OpenGL state management and caching
//!
//! # Thread Safety
//!
//! HgiGL supports single-threaded command submission on the main thread.
//! Command recording can happen on worker threads, but submission requires
//! a valid OpenGL context on the calling thread.
//!
//! # OpenGL Context Management
//!
//! HgiGL expects an OpenGL context to be created and made current before use.
//! The context must remain valid for the lifetime of the HgiGL instance.
//!
//! # Implementation Status
//!
//! This is a STUB implementation. The actual OpenGL calls require:
//! - The `gl` crate for OpenGL bindings
//! - A valid OpenGL 4.5+ context
//! - Platform-specific context creation (glutin, winit, SDL2, etc.)

pub mod capabilities;
pub mod conversions;
pub mod diagnostic;

// Resource types
pub mod buffer;
pub mod resource_bindings;
pub mod sampler;
pub mod shader_function;
pub mod shader_program;
pub mod texture;

// Pipeline state
pub mod compute_pipeline;
pub mod graphics_pipeline;

// Command buffers
pub mod blit_cmds;
pub mod compute_cmds;
pub mod graphics_cmds;

// Main HgiGL implementation
pub mod hgi;

// GL state save/restore (P1-5: ScopedStateHolder)
pub mod scoped_state_holder;
// GLSL code generation from HgiShaderFunctionDesc (P1-6: ShaderGenerator)
pub mod shader_generator;

// Re-exports
pub use capabilities::HgiGLCapabilities;
pub use hgi::HgiGL;

pub use buffer::HgiGLBuffer;
pub use compute_pipeline::HgiGLComputePipeline;
pub use graphics_pipeline::HgiGLGraphicsPipeline;
pub use resource_bindings::HgiGLResourceBindings;
pub use sampler::HgiGLSampler;
pub use shader_function::HgiGLShaderFunction;
pub use shader_program::HgiGLShaderProgram;
pub use texture::HgiGLTexture;

pub use blit_cmds::HgiGLBlitCmds;
pub use compute_cmds::HgiGLComputeCmds;
pub use graphics_cmds::HgiGLGraphicsCmds;

pub use conversions::*;
pub use diagnostic::*;
pub use scoped_state_holder::HgiGLScopedStateHolder;
pub use shader_generator::HgiGLShaderGenerator;
