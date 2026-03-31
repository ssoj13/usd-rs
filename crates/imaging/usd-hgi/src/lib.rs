//! Hydra Graphics Interface (HGI)
//!
//! HGI is an abstract GPU interface layer that provides a graphics-API-agnostic way to
//! interact with GPU resources and commands. It serves as the foundation for Hydra's
//! rendering system.
//!
//! # Overview
//!
//! HGI provides:
//! - Abstract interfaces for GPU resources (buffers, textures, shaders, pipelines)
//! - Command buffer recording for rendering, compute, and copy operations
//! - Platform-independent graphics state management
//! - Resource lifetime management through handles
//!
//! # Architecture
//!
//! HGI is designed as a trait-based system where:
//! - `Hgi` is the main trait for device interaction
//! - Resources are represented by traits (e.g., `HgiBuffer`, `HgiTexture`)
//! - Handles (`HgiHandle<T>`) provide safe, reference-counted access to resources
//! - Descriptors (`*Desc` structs) define resource properties
//! - Command buffers (`HgiCmds` traits) record GPU commands
//!
//! # Thread Safety
//!
//! HGI supports multi-threaded command recording:
//! - Command buffers can be created and recorded on different threads
//! - Submission must happen on the main thread (for OpenGL compatibility)
//! - Resource creation/destruction should happen on the main thread
//!
//! # Example Usage
//!
//! ```rust,ignore
//! use usd_hgi::*;
//!
//! // Create a buffer
//! let buffer_desc = HgiBufferDesc::new()
//!     .with_usage(HgiBufferUsage::VERTEX)
//!     .with_byte_size(1024);
//! let buffer = hgi.create_buffer(&buffer_desc, None);
//!
//! // Create a texture
//! let texture_desc = HgiTextureDesc::new()
//!     .with_format(HgiFormat::UNorm8Vec4)
//!     .with_dimensions(Vec3i::new(512, 512, 1))
//!     .with_usage(HgiTextureUsage::COLOR_TARGET | HgiTextureUsage::SHADER_READ);
//! let texture = hgi.create_texture(&texture_desc, None);
//!
//! // Record rendering commands
//! let mut cmds = hgi.create_graphics_cmds();
//! cmds.bind_pipeline(&pipeline);
//! cmds.bind_resources(&bindings);
//! cmds.draw(&draw_op);
//!
//! // Submit commands
//! hgi.submit_cmds(cmds, HgiSubmitWaitType::NoWait);
//! ```

// Core types and enums
pub mod capabilities;
pub mod driver_handle;
pub mod enums;
pub mod handle;
pub mod tokens;
pub mod types;

// Resource descriptors and traits
pub mod attachment_desc;
pub mod buffer;
pub mod resource_bindings;
pub mod sampler;
pub mod shader_function;
pub mod shader_generator;
pub mod shader_program;
pub mod shader_section;
pub mod texture;

// Pipeline state
pub mod compute_pipeline;
pub mod graphics_pipeline;

// Command buffers
pub mod blit_cmds;
pub mod cmds;
pub mod compute_cmds;
pub mod compute_cmds_desc;
pub mod graphics_cmds;
pub mod graphics_cmds_desc;

// Indirect command encoder
pub mod indirect_command_encoder;

// Main HGI trait (same name as parent module - intentional API design)
#[allow(clippy::module_inception)]
pub mod hgi;

// Re-export commonly used types
pub use capabilities::HgiCapabilities;
pub use enums::*;
pub use handle::HgiHandle;
pub use types::*;

pub use attachment_desc::HgiAttachmentDesc;
pub use buffer::{HgiBuffer, HgiBufferDesc, HgiBufferHandle, HgiBufferHandleVector};
pub use resource_bindings::{
    HgiBufferBindDesc, HgiResourceBindings, HgiResourceBindingsDesc, HgiResourceBindingsHandle,
    HgiTextureBindDesc,
};
pub use sampler::{HgiSampler, HgiSamplerDesc, HgiSamplerHandle, HgiSamplerHandleVector};
pub use shader_function::{
    HgiShaderFunction, HgiShaderFunctionBufferDesc, HgiShaderFunctionComputeDesc,
    HgiShaderFunctionDesc, HgiShaderFunctionFragmentDesc, HgiShaderFunctionGeometryDesc,
    HgiShaderFunctionHandle, HgiShaderFunctionHandleVector, HgiShaderFunctionParamBlockDesc,
    HgiShaderFunctionParamBlockMember, HgiShaderFunctionParamDesc,
    HgiShaderFunctionTessellationDesc, HgiShaderFunctionTextureDesc,
};
pub use shader_generator::{HgiShaderGenerator, HgiShaderGeneratorBase};
pub use shader_program::{
    HgiShaderProgram, HgiShaderProgramDesc, HgiShaderProgramHandle, HgiShaderProgramHandleVector,
};
pub use shader_section::{HgiShaderSection, HgiShaderSectionAttribute};
pub use texture::{
    HgiComponentMapping, HgiTexture, HgiTextureDesc, HgiTextureHandle, HgiTextureHandleVector,
    HgiTextureViewDesc, HgiTextureViewHandle,
};

pub use compute_pipeline::{
    HgiComputePipeline, HgiComputePipelineDesc, HgiComputePipelineHandle,
    HgiComputePipelineHandleVector, HgiComputeShaderConstantsDesc,
};
pub use graphics_pipeline::{
    HgiColorBlendState, HgiDepthStencilState, HgiGraphicsPipeline, HgiGraphicsPipelineDesc,
    HgiGraphicsPipelineHandle, HgiMultiSampleState, HgiRasterizationState, HgiStencilState,
    HgiVertexAttributeDesc, HgiVertexBufferDesc,
};

pub use blit_cmds::{
    HgiBlitCmds, HgiBufferCpuToGpuOp, HgiBufferGpuToCpuOp, HgiBufferGpuToGpuOp,
    HgiBufferToTextureOp, HgiTextureCpuToGpuOp, HgiTextureGpuToCpuOp, HgiTextureGpuToGpuOp,
    HgiTextureToBufferOp,
};
pub use cmds::{HgiCmds, HgiCmdsSubmit};
pub use compute_cmds::{HgiComputeCmds, HgiComputeDispatchOp};
pub use compute_cmds_desc::HgiComputeCmdsDesc;
pub use graphics_cmds::{
    HgiDrawIndexedOp, HgiDrawIndirectOp, HgiDrawOp, HgiGraphicsCmds, HgiScissor, HgiViewport,
};

pub use driver_handle::HgiDriverHandle;
pub use graphics_cmds_desc::HgiGraphicsCmdsDesc;
pub use hgi::{
    Hgi, HgiBackendEntry, create_named_hgi, create_platform_default_hgi,
    get_platform_default_hgi_name, is_hgi_supported, register_hgi_backend,
};
pub use indirect_command_encoder::{
    HgiIndirectCommandEncoder, HgiIndirectCommands, HgiVertexBufferBinding,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // Test that key types are accessible
        let _format = HgiFormat::Float32Vec4;
        let _usage = HgiTextureUsage::COLOR_TARGET;
        let _stage = HgiShaderStage::VERTEX;

        // Test descriptor creation
        let _buffer_desc = HgiBufferDesc::new();
        let _texture_desc = HgiTextureDesc::new();
        let _sampler_desc = HgiSamplerDesc::new();

        // Test handle creation
        let _buffer_handle: HgiBufferHandle = HgiHandle::null();
        let _texture_handle: HgiTextureHandle = HgiHandle::null();
    }

    #[test]
    fn test_capabilities() {
        let caps = HgiCapabilities::new();
        // C++ defaults: all limits are 0, page_size_alignment is 1
        assert_eq!(caps.max_texture_dimension_2d, 0);
        assert_eq!(caps.max_uniform_block_size, 0);
        assert_eq!(caps.page_size_alignment, 1);
        assert_eq!(caps.api_version, 0);
        assert_eq!(caps.shader_version, 0);
    }
}
