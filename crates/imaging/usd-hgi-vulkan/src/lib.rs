//! HgiVulkan - Vulkan backend for the Hydra Graphics Interface (HGI).
//!
//! Port of `pxr/imaging/hgiVulkan`.
//!
//! Provides a complete Vulkan implementation of the HGI trait system,
//! enabling Hydra rendering via Vulkan on supporting platforms.
//!
//! # Feature
//!
//! Enable with the `vulkan` feature in `Cargo.toml`.

// Core infrastructure
pub mod capabilities;
pub mod conversions;
pub mod device;
pub mod diagnostic;
pub mod garbage_collector;
pub mod instance;

// Command management
pub mod command_buffer;
pub mod command_queue;
pub mod pipeline_cache;

// Shader system
pub mod descriptor_set_layouts;
pub mod shader_compiler;
pub mod shader_generator;
pub mod shader_section;

// Resources
pub mod buffer;
pub mod resource_bindings;
pub mod sampler;
pub mod shader_function;
pub mod shader_program;
pub mod texture;

// Pipelines
pub mod compute_pipeline;
pub mod graphics_pipeline;

// Command encoders
pub mod blit_cmds;
pub mod compute_cmds;
pub mod graphics_cmds;

// Main entry point
pub mod hgi;

// --- Re-exports ---

pub use capabilities::HgiVulkanCapabilities;
pub use device::HgiVulkanDevice;
pub use garbage_collector::{GarbageItem, HgiVulkanGarbageCollector};
pub use hgi::HgiVulkan;
pub use instance::HgiVulkanInstance;

pub use command_buffer::{HgiVulkanCommandBuffer, HgiVulkanCompletedHandler, InFlightUpdateResult};
pub use command_queue::{HgiVulkanCommandPool, HgiVulkanCommandQueue};
pub use pipeline_cache::HgiVulkanPipelineCache;

pub use descriptor_set_layouts::{HgiVulkanDescriptorSetInfo, HgiVulkanDescriptorSetInfoVector};
pub use shader_section::{
    HgiVulkanBlockShaderSection, HgiVulkanBufferShaderSection,
    HgiVulkanInterstageBlockShaderSection, HgiVulkanKeywordShaderSection,
    HgiVulkanMacroShaderSection, HgiVulkanMemberShaderSection, HgiVulkanShaderSection,
    HgiVulkanTextureShaderSection,
};

pub use buffer::HgiVulkanBuffer;
pub use resource_bindings::HgiVulkanResourceBindings;
pub use sampler::HgiVulkanSampler;
pub use shader_function::HgiVulkanShaderFunction;
pub use shader_program::HgiVulkanShaderProgram;
pub use texture::HgiVulkanTexture;

pub use compute_pipeline::HgiVulkanComputePipeline;
pub use graphics_pipeline::HgiVulkanGraphicsPipeline;

pub use blit_cmds::HgiVulkanBlitCmds;
pub use compute_cmds::HgiVulkanComputeCmds;
pub use graphics_cmds::HgiVulkanGraphicsCmds;
