//! wgpu device capabilities for HGI.
//!
//! Queries the wgpu::Adapter for features and limits, then maps them
//! into HgiCapabilities fields.

use usd_hgi::capabilities::HgiCapabilities;
use usd_hgi::enums::HgiDeviceCapabilities;

/// wgpu-specific device capabilities.
///
/// Wraps HgiCapabilities with wgpu adapter features and limits.
/// Created by querying the wgpu::Adapter at HGI initialization time.
pub struct WgpuCapabilities {
    /// Base HGI capabilities populated from wgpu limits
    pub base: HgiCapabilities,
    /// wgpu feature flags reported by the adapter
    pub features: wgpu::Features,
    /// wgpu device limits reported by the adapter
    pub limits: wgpu::Limits,
    /// Adapter info (driver name, backend, etc.)
    pub adapter_info: wgpu::AdapterInfo,
}

impl WgpuCapabilities {
    /// Query capabilities from a wgpu adapter.
    pub fn new(adapter: &wgpu::Adapter) -> Self {
        let features = adapter.features();
        let limits = adapter.limits();
        let adapter_info = adapter.get_info();

        let mut base = HgiCapabilities::default();

        // -- Map wgpu limits to HGI capabilities --
        base.max_uniform_block_size = limits.max_uniform_buffer_binding_size as usize;
        base.max_storage_block_size = limits.max_storage_buffer_binding_size as usize;
        base.max_texture_dimension_2d = limits.max_texture_dimension_2d as i32;
        base.max_texture_dimension_3d = limits.max_texture_dimension_3d as i32;
        base.max_texture_layers = limits.max_texture_array_layers as i32;
        base.max_vertex_attributes = limits.max_vertex_attributes as i32;
        base.max_color_attachments = limits.max_color_attachments as i32;

        base.max_compute_work_group_size = [
            limits.max_compute_workgroup_size_x,
            limits.max_compute_workgroup_size_y,
            limits.max_compute_workgroup_size_z,
        ];
        base.max_compute_work_group_invocations = limits.max_compute_invocations_per_workgroup;

        // Clip/cull distances (wgpu doesn't expose these limits via API).
        // Set reasonable defaults; max_clip_distances is usize (C++ type), others are i32 extensions.
        base.max_clip_distances = 8_usize;
        base.max_cull_distances = 8;
        base.max_combined_clip_and_cull_distances = 8;

        // Uniform buffer offset alignment (e.g., 256 on most GPUs)
        base.uniform_buffer_offset_alignment = limits.min_uniform_buffer_offset_alignment as usize;
        // Page size alignment: use the same value as UBO alignment for wgpu
        base.page_size_alignment = limits.min_uniform_buffer_offset_alignment as usize;

        // -- Map wgpu features to HGI capability flags --
        if features.contains(wgpu::Features::INDIRECT_FIRST_INSTANCE) {
            base.device_capabilities
                .insert(HgiDeviceCapabilities::MULTI_DRAW_INDIRECT);
        }

        if features.contains(wgpu::Features::DEPTH_CLIP_CONTROL) {
            base.device_capabilities
                .insert(HgiDeviceCapabilities::CUSTOM_DEPTH_RANGE);
        }

        if features.contains(wgpu::Features::CONSERVATIVE_RASTERIZATION) {
            base.device_capabilities
                .insert(HgiDeviceCapabilities::CONSERVATIVE_RASTER);
        }

        // wgpu depth range is [0, 1] by default (not [-1, 1] like OpenGL)
        // so we do NOT set DEPTH_RANGE_MINUS_ONE_TO_ONE

        // Unified memory detection from adapter type
        if adapter_info.device_type == wgpu::DeviceType::IntegratedGpu {
            base.uses_unified_memory = true;
            base.device_capabilities
                .insert(HgiDeviceCapabilities::UNIFIED_MEMORY);
        }

        // wgpu doesn't expose bindless textures/buffers via standard API
        base.supports_bindless = false;

        // Presentation is always supported (wgpu can create surfaces)
        base.device_capabilities
            .insert(HgiDeviceCapabilities::PRESENTATION);

        // Concurrent dispatch is supported in wgpu
        base.device_capabilities
            .insert(HgiDeviceCapabilities::CONCURRENT_DISPATCH);

        Self {
            base,
            features,
            limits,
            adapter_info,
        }
    }

    /// Create capabilities with default/fallback values (no adapter).
    pub fn default_caps() -> Self {
        Self {
            base: HgiCapabilities::default(),
            features: wgpu::Features::empty(),
            limits: wgpu::Limits::default(),
            adapter_info: wgpu::AdapterInfo {
                name: "Unknown".to_string(),
                vendor: 0,
                device: 0,
                device_type: wgpu::DeviceType::Other,
                driver: String::new(),
                driver_info: String::new(),
                backend: wgpu::Backend::Vulkan,
            },
        }
    }

    /// Get the wgpu backend name as a string (e.g., "Vulkan", "Metal", "DirectX 12").
    pub fn backend_name(&self) -> &str {
        match self.adapter_info.backend {
            wgpu::Backend::Vulkan => "Vulkan",
            wgpu::Backend::Metal => "Metal",
            wgpu::Backend::Dx12 => "DirectX 12",
            wgpu::Backend::Gl => "OpenGL",
            wgpu::Backend::BrowserWebGpu => "WebGPU",
            _ => "Unknown",
        }
    }

    /// Get API version string from driver info (if available).
    pub fn api_version(&self) -> String {
        if !self.adapter_info.driver_info.is_empty() {
            format!(
                "{} ({})",
                self.backend_name(),
                self.adapter_info.driver_info
            )
        } else {
            self.backend_name().to_string()
        }
    }

    /// Get shader language version string (WGSL for wgpu).
    pub fn shader_version(&self) -> &str {
        "WGSL 1.0"
    }

    /// Get adapter device name.
    pub fn device_name(&self) -> &str {
        &self.adapter_info.name
    }

    /// Get the base HGI capabilities (matching GL API pattern).
    pub fn base_capabilities(&self) -> &HgiCapabilities {
        &self.base
    }

    /// Check if a specific wgpu feature is available.
    pub fn has_feature(&self, feature: wgpu::Features) -> bool {
        self.features.contains(feature)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_caps() {
        let caps = WgpuCapabilities::default_caps();
        // After fix: HgiCapabilities::default() has max_texture_dimension_2d = 0 (C++ parity)
        assert_eq!(caps.base.max_texture_dimension_2d, 0);
        // default_caps uses wgpu::Backend::Vulkan as a fallback sentinel
        assert_eq!(caps.backend_name(), "Vulkan");
        // After fix: HgiCapabilities::default() has uniform_buffer_offset_alignment = 0 (C++ parity)
        assert_eq!(caps.base.uniform_buffer_offset_alignment, 0);
    }
}
