//! wgpu backend for USD Hydra Graphics Interface (HGI).
//!
//! Provides GPU resource implementations (buffer, texture, sampler, pipeline,
//! shader, command buffer) using wgpu as the graphics backend.
//! Covers Vulkan, Metal, DX12, and OpenGL through a single safe Rust API --
//! no unsafe code required.

pub mod blit_cmds;
pub mod buffer;
pub mod capabilities;
pub mod compute_cmds;
pub mod compute_pipeline;
pub mod conversions;
pub mod gpu_timer;
pub mod graphics_cmds;
pub mod graphics_pipeline;
pub mod hgi;
pub mod mipmap;
pub mod resolve;
pub mod resource_bindings;
pub mod sampler;
pub mod shader_function;
pub mod shader_program;
pub mod surface;
pub mod texture;

pub use blit_cmds::WgpuBlitCmds;
pub use buffer::WgpuBuffer;
pub use capabilities::WgpuCapabilities;
pub use compute_cmds::WgpuComputeCmds;
pub use compute_pipeline::WgpuComputePipeline;
pub use gpu_timer::GpuTimer;
pub use graphics_cmds::WgpuGraphicsCmds;
pub use graphics_pipeline::WgpuGraphicsPipeline;
pub use hgi::{HgiWgpu, create_hgi_wgpu};
pub use mipmap::MipmapGenerator;
pub use resolve::*;
pub use resource_bindings::WgpuResourceBindings;
pub use sampler::WgpuSampler;
pub use shader_function::WgpuShaderFunction;
pub use shader_program::WgpuShaderProgram;
pub use surface::{StagingReadback, create_presentation_texture, write_pixels_to_texture};
pub use texture::WgpuTexture;
