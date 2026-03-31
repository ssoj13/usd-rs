//! GPU device capabilities query interface

use super::enums::HgiDeviceCapabilities;

/// GPU device capabilities
///
/// Mirrors C++ HgiCapabilities base class. The core 5 limit fields plus device
/// capability flags have C++-matching defaults. Extended fields (texture dims,
/// compute limits, etc.) are Rust additions populated by each backend.
#[derive(Debug, Clone)]
pub struct HgiCapabilities {
    /// Supported device capability flags (C++ `_flags`)
    pub device_capabilities: HgiDeviceCapabilities,

    /// Maximum uniform buffer binding size in bytes
    /// (C++ `_maxUniformBlockSize`, default 0)
    pub max_uniform_block_size: usize,

    /// Maximum shader storage buffer binding size in bytes
    /// (C++ `_maxShaderStorageBlockSize`, default 0)
    pub max_storage_block_size: usize,

    /// Required alignment for uniform buffer offsets in bytes
    /// (C++ `_uniformBufferOffsetAlignment`, default 0)
    pub uniform_buffer_offset_alignment: usize,

    /// Maximum clip distances
    /// (C++ `_maxClipDistances`, default 0)
    pub max_clip_distances: usize,

    /// Page size for buffer/texture memory alignment in bytes
    /// (C++ `_pageSizeAlignment`, default 1)
    pub page_size_alignment: usize,

    /// Graphics API version integer (e.g. 460 for GL 4.6).
    /// Corresponds to C++ virtual `GetAPIVersion()`. 0 until backend sets it.
    pub api_version: i32,

    /// Shader language version integer (e.g. 460 for GLSL 4.60).
    /// Corresponds to C++ virtual `GetShaderVersion()`. 0 until backend sets it.
    pub shader_version: i32,

    // -- Extended fields (not in C++ base, populated by backends) --
    /// Maximum texture dimension for 1D and 2D textures
    pub max_texture_dimension_2d: i32,

    /// Maximum texture dimension for 3D textures
    pub max_texture_dimension_3d: i32,

    /// Maximum number of texture layers in an array texture
    pub max_texture_layers: i32,

    /// Maximum number of vertex attributes
    pub max_vertex_attributes: i32,

    /// Maximum number of color attachments in a framebuffer
    pub max_color_attachments: i32,

    /// Maximum work group size for compute shaders (x, y, z)
    pub max_compute_work_group_size: [u32; 3],

    /// Maximum work group invocations for compute shaders
    pub max_compute_work_group_invocations: u32,

    /// Maximum cull distances
    pub max_cull_distances: i32,

    /// Maximum combined clip and cull distances
    pub max_combined_clip_and_cull_distances: i32,

    /// Whether the backend uses unified memory (CPU & GPU share memory)
    pub uses_unified_memory: bool,

    /// Whether bindless resources are supported
    pub supports_bindless: bool,
}

impl Default for HgiCapabilities {
    fn default() -> Self {
        // Core fields match C++ HgiCapabilities() constructor:
        //   _maxUniformBlockSize(0), _maxShaderStorageBlockSize(0),
        //   _uniformBufferOffsetAlignment(0), _maxClipDistances(0), _pageSizeAlignment(1)
        // Extended fields start at 0 — backends fill them in during init.
        Self {
            device_capabilities: HgiDeviceCapabilities::empty(),
            max_uniform_block_size: 0,
            max_storage_block_size: 0,
            uniform_buffer_offset_alignment: 0,
            max_clip_distances: 0,
            page_size_alignment: 1,
            api_version: 0,
            shader_version: 0,
            max_texture_dimension_2d: 0,
            max_texture_dimension_3d: 0,
            max_texture_layers: 0,
            max_vertex_attributes: 0,
            max_color_attachments: 0,
            max_compute_work_group_size: [0, 0, 0],
            max_compute_work_group_invocations: 0,
            max_cull_distances: 0,
            max_combined_clip_and_cull_distances: 0,
            uses_unified_memory: false,
            supports_bindless: false,
        }
    }
}

impl HgiCapabilities {
    /// Create new capabilities with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if a specific capability is supported
    pub fn supports(&self, capability: HgiDeviceCapabilities) -> bool {
        self.device_capabilities.contains(capability)
    }

    /// Enable a specific capability
    pub fn enable(&mut self, capability: HgiDeviceCapabilities) {
        self.device_capabilities.insert(capability);
    }

    /// Disable a specific capability
    pub fn disable(&mut self, capability: HgiDeviceCapabilities) {
        self.device_capabilities.remove(capability);
    }

    /// Get a human-readable description of the capabilities
    pub fn description(&self) -> String {
        format!(
            "HgiCapabilities:\n\
             - Max Texture 2D: {}x{}\n\
             - Max Texture 3D: {}x{}x{}\n\
             - Max Texture Layers: {}\n\
             - Max Vertex Attributes: {}\n\
             - Max Color Attachments: {}\n\
             - Max Uniform Block: {} bytes\n\
             - Max Storage Block: {} bytes\n\
             - Unified Memory: {}\n\
             - Bindless: {}\n\
             - API Version: {}\n\
             - Shader Version: {}",
            self.max_texture_dimension_2d,
            self.max_texture_dimension_2d,
            self.max_texture_dimension_3d,
            self.max_texture_dimension_3d,
            self.max_texture_dimension_3d,
            self.max_texture_layers,
            self.max_vertex_attributes,
            self.max_color_attachments,
            self.max_uniform_block_size,
            self.max_storage_block_size,
            self.uses_unified_memory,
            self.supports_bindless,
            self.api_version,
            self.shader_version,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_capabilities() {
        let caps = HgiCapabilities::default();
        // Core fields match C++ HgiCapabilities() defaults
        assert_eq!(caps.max_uniform_block_size, 0);
        assert_eq!(caps.max_storage_block_size, 0);
        assert_eq!(caps.uniform_buffer_offset_alignment, 0);
        assert_eq!(caps.max_clip_distances, 0);
        assert_eq!(caps.page_size_alignment, 1);
        assert_eq!(caps.api_version, 0);
        assert_eq!(caps.shader_version, 0);
        assert!(!caps.supports_bindless);
    }

    #[test]
    fn test_capability_flags() {
        let mut caps = HgiCapabilities::new();

        assert!(!caps.supports(HgiDeviceCapabilities::UNIFIED_MEMORY));

        caps.enable(HgiDeviceCapabilities::UNIFIED_MEMORY);
        assert!(caps.supports(HgiDeviceCapabilities::UNIFIED_MEMORY));

        caps.disable(HgiDeviceCapabilities::UNIFIED_MEMORY);
        assert!(!caps.supports(HgiDeviceCapabilities::UNIFIED_MEMORY));
    }

    #[test]
    fn test_description() {
        let caps = HgiCapabilities::new();
        let desc = caps.description();
        assert!(desc.contains("HgiCapabilities"));
        assert!(desc.contains("Max Texture 2D"));
        assert!(desc.contains("API Version"));
    }
}
