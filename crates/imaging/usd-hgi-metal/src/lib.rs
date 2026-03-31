#![allow(dead_code)]
//! HgiMetal - Metal Backend for Hydra Graphics Interface
//!
//! Port of pxr/imaging/hgiMetal
//!
//! This module provides a Metal implementation of the HGI trait system.
//! It allows Hydra to render using Metal on macOS/iOS.
//!
//! # Platform
//!
//! macOS and iOS only. On other platforms, this crate compiles but
//! all Metal operations are stubs.

// Diagnostics and debug utilities
pub mod diagnostic;

// Type conversions
pub mod capabilities;
pub mod conversions;

// Resource types
pub mod buffer;
pub mod resource_bindings;
pub mod sampler;
pub mod texture;

// Shader code generation
pub mod shader_function;
pub mod shader_generator;
pub mod shader_program;
pub mod shader_section;

// Pipeline state
pub mod compute_pipeline;
pub mod graphics_pipeline;

// Command buffers
pub mod blit_cmds;
pub mod compute_cmds;
pub mod graphics_cmds;

// Step functions for multi-draw indirect
pub mod step_functions;

// Indirect command encoder
pub mod indirect_command_encoder;

// Main HgiMetal implementation
pub mod hgi;

// Re-exports
pub use blit_cmds::HgiMetalBlitCmds;
pub use buffer::HgiMetalBuffer;
pub use capabilities::{HgiMetalCapabilities, MetalApiVersion};
pub use compute_cmds::HgiMetalComputeCmds;
pub use compute_pipeline::HgiMetalComputePipeline;
pub use conversions::HgiMetalConversions;
pub use graphics_cmds::HgiMetalGraphicsCmds;
pub use graphics_pipeline::HgiMetalGraphicsPipeline;
pub use hgi::{CommitCommandBufferWaitType, HgiMetal};
pub use indirect_command_encoder::HgiMetalIndirectCommandEncoder;
pub use resource_bindings::{
    HGI_METAL_ARGUMENT_OFFSET_SIZE, HgiMetalArgumentIndex, HgiMetalArgumentOffset,
    HgiMetalResourceBindings,
};
pub use sampler::HgiMetalSampler;
pub use shader_function::HgiMetalShaderFunction;
pub use shader_generator::HgiMetalShaderGenerator;
pub use shader_program::HgiMetalShaderProgram;
pub use step_functions::{HgiMetalStepFunctionDesc, HgiMetalStepFunctions};
pub use texture::HgiMetalTexture;
