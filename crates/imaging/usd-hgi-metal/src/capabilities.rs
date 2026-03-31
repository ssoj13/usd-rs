//! Metal device capabilities.
//! Port of pxr/imaging/hgiMetal/capabilities

use usd_hgi::{HgiCapabilities, HgiDeviceCapabilities};

/// Metal API version constants.
/// Mirrors C++ APIVersion_Metal* enum.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetalApiVersion {
    Metal1_0 = 0,
    Metal2_0 = 1,
    Metal3_0 = 2,
}

/// Metal resource storage mode (mirrors MTLResourceOptions).
/// Determines CPU/GPU memory sharing strategy.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetalStorageMode {
    /// MTLResourceStorageModeShared — CPU and GPU share memory
    Shared = 0,
    /// MTLResourceStorageModeManaged — CPU/GPU have separate copies, synced explicitly
    Managed = 16,
    /// MTLResourceStorageModePrivate — GPU-only memory
    Private = 32,
}

/// Metal-specific capabilities.
/// Mirrors C++ HgiMetalCapabilities with all Metal-specific fields.
#[derive(Debug, Clone)]
pub struct HgiMetalCapabilities {
    /// Base HGI capabilities
    pub base: HgiCapabilities,
    /// Default storage mode for buffer/texture allocation.
    /// MTLResourceStorageModeShared on unified memory, Managed on discrete.
    /// Mirrors C++ defaultStorageMode field.
    pub default_storage_mode: MetalStorageMode,
    /// Whether the device has vertex memory barrier support
    pub has_vertex_memory_barrier: bool,
    /// Whether to use parallel render command encoder
    pub use_parallel_encoder: bool,
    /// Whether indirect draw fix is required (Vega GPUs pre-macOS 12.2)
    pub requires_indirect_draw_fix: bool,
    /// Whether return-after-discard is required in fragment shaders
    pub requires_return_after_discard: bool,
}

impl HgiMetalCapabilities {
    /// Creates a new Metal capabilities instance with default values.
    /// On real Metal, this would take an id<MTLDevice> and query capabilities.
    pub fn new() -> Self {
        let mut base = HgiCapabilities::new();

        // Set Metal-specific capability flags matching C++ defaults
        base.enable(HgiDeviceCapabilities::CPP_SHADER_PADDING);
        base.enable(HgiDeviceCapabilities::METAL_TESSELLATION);
        base.enable(HgiDeviceCapabilities::MULTI_DRAW_INDIRECT);
        base.enable(HgiDeviceCapabilities::SINGLE_SLOT_RESOURCE_ARRAYS);

        // Set Metal limits matching C++ constructor
        base.max_uniform_block_size = 64 * 1024;
        base.max_storage_block_size = 1024 * 1024 * 1024;
        base.uniform_buffer_offset_alignment = 16;
        base.max_clip_distances = 8;
        base.page_size_alignment = 4096;

        // Shader version: 450 for GLSL compatibility (matches C++)
        base.shader_version = 450;

        Self {
            base,
            default_storage_mode: MetalStorageMode::Shared,
            has_vertex_memory_barrier: true,
            use_parallel_encoder: true,
            requires_indirect_draw_fix: false,
            requires_return_after_discard: true,
        }
    }

    /// Returns the base HGI capabilities.
    pub fn base_capabilities(&self) -> &HgiCapabilities {
        &self.base
    }

    /// Returns the Metal API version.
    /// Mirrors C++ GetAPIVersion().
    pub fn get_api_version(&self) -> MetalApiVersion {
        // Stub: assume Metal 3.0 (macOS 10.15+)
        MetalApiVersion::Metal3_0
    }

    /// Returns the shader language version.
    /// Mirrors C++ GetShaderVersion().
    /// Returns 450 for GLSL compatibility.
    pub fn get_shader_version(&self) -> i32 {
        self.base.shader_version
    }

    /// Set a device capability flag.
    pub fn set_flag(&mut self, flag: HgiDeviceCapabilities, enabled: bool) {
        if enabled {
            self.base.enable(flag);
        } else {
            self.base.disable(flag);
        }
    }
}

impl Default for HgiMetalCapabilities {
    fn default() -> Self {
        Self::new()
    }
}
