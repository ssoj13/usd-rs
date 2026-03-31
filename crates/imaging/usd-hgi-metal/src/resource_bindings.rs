//! Metal resource bindings. Port of pxr/imaging/hgiMetal/resourceBindings

use usd_hgi::{HgiResourceBindings, HgiResourceBindingsDesc, HgiShaderStage};

/// Fixed argument buffer indices for Metal resource types.
/// Chosen at the top of the range to not interfere with vertex attributes.
/// Mirrors C++ HgiMetalArgumentIndex.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HgiMetalArgumentIndex {
    Icb = 26,
    Constants = 27,
    Samplers = 28,
    Textures = 29,
    Buffers = 30,
}

/// Byte offsets within the argument buffer for different resource/stage combos.
/// Mirrors C++ HgiMetalArgumentOffset.
/// Note: C++ uses plain enum with duplicated values for VS/CS overlap.
/// In Rust we use constants instead since enums can't have duplicate discriminants.
pub mod argument_offset {
    pub const BUFFER_VS: u32 = 0;
    pub const BUFFER_FS: u32 = 512;
    pub const SAMPLER_VS: u32 = 1024;
    pub const SAMPLER_FS: u32 = 1536;
    pub const TEXTURE_VS: u32 = 2048;
    pub const TEXTURE_FS: u32 = 2560;

    pub const BUFFER_CS: u32 = 0;
    pub const SAMPLER_CS: u32 = 1024;
    pub const TEXTURE_CS: u32 = 2048;

    pub const CONSTANTS: u32 = 3072;

    pub const SIZE: u32 = 4096;
}

/// Re-export for backward compat.
/// Mirrors C++ HgiMetalArgumentOffset enum.
#[allow(non_camel_case_types)]
pub type HgiMetalArgumentOffset = u32;

/// Total size of the Metal argument buffer in bytes.
pub const HGI_METAL_ARGUMENT_OFFSET_SIZE: u32 = 4096;

/// Metal resource bindings (descriptor set equivalent).
#[derive(Debug)]
pub struct HgiMetalResourceBindings {
    desc: HgiResourceBindingsDesc,
}

impl HgiMetalResourceBindings {
    /// Creates a new Metal resource bindings from the given descriptor.
    pub fn new(desc: HgiResourceBindingsDesc) -> Self {
        Self { desc }
    }

    /// Bind resources to a render command encoder.
    /// Stub: requires Metal render command encoder and argument buffer.
    pub fn bind_resources_render(&self, _arg_buffer_offset: u64) {
        // Stub: on real Metal, would iterate buffer/texture/sampler bindings
        // and encode them into the argument buffer at the appropriate offsets
    }

    /// Bind resources to a compute command encoder.
    /// Stub: requires Metal compute command encoder and argument buffer.
    pub fn bind_resources_compute(&self, _arg_buffer_offset: u64) {
        // Stub: similar to render but using CS offsets
    }

    /// Set constant values into the argument buffer.
    /// Mirrors C++ static SetConstantValues().
    pub fn set_constant_values(
        _stages: HgiShaderStage,
        _bind_index: u32,
        _byte_size: u32,
        _data: &[u8],
    ) {
        // Stub: on real Metal, would memcpy data into argument buffer
        // at HgiMetalArgumentOffset::Constants + bind_index * byte_size
    }
}

impl HgiResourceBindings for HgiMetalResourceBindings {
    fn descriptor(&self) -> &HgiResourceBindingsDesc {
        &self.desc
    }
    fn raw_resource(&self) -> u64 {
        0
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
