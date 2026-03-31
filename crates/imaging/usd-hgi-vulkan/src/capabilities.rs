//! Vulkan device capabilities query.
//!
//! Port of pxr/imaging/hgiVulkan/capabilities.cpp/.h

// Vulkan API calls require raw pointer manipulation throughout this module.
#![allow(unsafe_code)]

use ash::vk;
use usd_hgi::{HgiCapabilities, HgiDeviceCapabilities};

// ---- env-var feature gates (mirror C++ TF_DEFINE_ENV_SETTING) ---------------

fn env_bool(name: &str, default: bool) -> bool {
    match std::env::var(name).as_deref() {
        Ok("1") | Ok("true") | Ok("TRUE") => true,
        Ok("0") | Ok("false") | Ok("FALSE") => false,
        _ => default,
    }
}

fn enable_multi_draw_indirect() -> bool {
    env_bool("HGIVULKAN_ENABLE_MULTI_DRAW_INDIRECT", true)
}
fn enable_builtin_barycentrics() -> bool {
    env_bool("HGIVULKAN_ENABLE_BUILTIN_BARYCENTRICS", false)
}
fn enable_native_interop() -> bool {
    env_bool("HGIVULKAN_ENABLE_NATIVE_INTEROP", true)
}
fn enable_uma() -> bool {
    env_bool("HGIVULKAN_ENABLE_UMA", true)
}
fn enable_rebar() -> bool {
    env_bool("HGIVULKAN_ENABLE_REBAR", false)
}

// ---- memory topology helpers ------------------------------------------------

const BAR_MAX_SIZE: u64 = 256 * 1024 * 1024;

/// Returns true if device memory is host-accessible (UMA or ReBAR).
///
/// Checks for a device-local heap > 256 MiB that also has a HOST_VISIBLE +
/// HOST_COHERENT memory type — the signature of UMA or ReBAR devices.
fn supports_host_accessible_device_memory(mem: &vk::PhysicalDeviceMemoryProperties) -> bool {
    for heap_idx in 0..mem.memory_heap_count as usize {
        let heap = &mem.memory_heaps[heap_idx];
        if !heap.flags.contains(vk::MemoryHeapFlags::DEVICE_LOCAL) || heap.size <= BAR_MAX_SIZE {
            continue;
        }
        let required = vk::MemoryPropertyFlags::DEVICE_LOCAL
            | vk::MemoryPropertyFlags::HOST_VISIBLE
            | vk::MemoryPropertyFlags::HOST_COHERENT;
        for type_idx in 0..mem.memory_type_count as usize {
            let mt = &mem.memory_types[type_idx];
            if mt.heap_index as usize == heap_idx && mt.property_flags.contains(required) {
                return true;
            }
        }
    }
    false
}

// ---- public types -----------------------------------------------------------

/// Vulkan format query result (mirrors `HgiVulkanFormatInfo`).
#[derive(Debug, Clone, Default)]
pub struct HgiVulkanFormatInfo {
    pub image_type: vk::ImageType,
    pub format: vk::Format,
    pub usage: vk::ImageUsageFlags,
    pub create_flags: vk::ImageCreateFlags,
    pub host_image_copy_optimal: bool,
}

/// Vulkan device capabilities.
///
/// Queries physical device properties and features during construction,
/// then populates `base` (HgiCapabilities) accordingly.
///
/// Port of C++ `HgiVulkanCapabilities`.
pub struct HgiVulkanCapabilities {
    /// Base HGI capabilities shared with the rest of the pipeline.
    pub base: HgiCapabilities,

    /// Whether the graphics queue supports timestamp queries.
    pub supports_timestamps: bool,
    /// Whether native GPU–CPU memory interop extensions are present (Win32/Linux).
    pub supports_native_interop: bool,
    /// Whether `VK_EXT_host_image_copy` is available and usable.
    pub supports_host_image_copy: bool,

    /// Full `VkPhysicalDeviceProperties2` including pNext-chained sub-structs.
    pub vk_device_properties2: vk::PhysicalDeviceProperties2<'static>,
    /// Memory heap / type layout.
    pub vk_memory_properties: vk::PhysicalDeviceMemoryProperties,
    /// Device UUID / LUID (for interop).
    pub vk_physical_device_id_properties: vk::PhysicalDeviceIDProperties<'static>,
    /// Max vertex attribute divisor.
    pub vk_vertex_attribute_divisor_properties:
        vk::PhysicalDeviceVertexAttributeDivisorPropertiesEXT<'static>,

    /// Core Vulkan 1.0 features.
    pub vk_device_features2: vk::PhysicalDeviceFeatures2<'static>,
    /// Vulkan 1.1 features (shaderDrawParameters, etc.).
    pub vk_vulkan11_features: vk::PhysicalDeviceVulkan11Features<'static>,
    /// Vulkan 1.2 features (timelineSemaphore, etc.).
    pub vk_vulkan12_features: vk::PhysicalDeviceVulkan12Features<'static>,
    /// Vulkan 1.3 features (synchronization2, dynamicRendering, etc.).
    pub vk_vulkan13_features: vk::PhysicalDeviceVulkan13Features<'static>,
    /// Vertex attribute divisor feature flag.
    pub vk_vertex_attribute_divisor_features:
        vk::PhysicalDeviceVertexAttributeDivisorFeaturesEXT<'static>,
    /// Fragment shader barycentric feature flag (KHR).
    pub vk_barycentric_features: vk::PhysicalDeviceFragmentShaderBarycentricFeaturesKHR<'static>,
    /// Line rasterization feature flag (KHR).
    pub vk_line_rasterization_features: vk::PhysicalDeviceLineRasterizationFeaturesKHR<'static>,
}

impl HgiVulkanCapabilities {
    /// Query all physical device properties and features, then populate
    /// `base` (HgiCapabilities) with matching values.
    ///
    /// `gfx_queue_family_index` — the graphics queue family that will be used
    /// for rendering (needed to check timestamp support).
    ///
    /// `supported_extensions` — list of device extension names that were enabled
    /// (mirrors `HgiVulkanDevice::IsSupportedExtension` calls in C++).
    ///
    /// # Safety
    /// `instance` and `physical_device` must remain valid for the duration of
    /// this call. The returned struct owns no Vulkan handles.
    pub fn new(
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
        gfx_queue_family_index: u32,
        supported_extensions: &[&str],
    ) -> Self {
        // ---- allocate all pNext-chain structs with default sType/zero rest ----
        let mut vk_device_properties2 = vk::PhysicalDeviceProperties2::default();
        let mut vk_physical_device_id_properties = vk::PhysicalDeviceIDProperties::default();
        let mut vk_vertex_attribute_divisor_properties =
            vk::PhysicalDeviceVertexAttributeDivisorPropertiesEXT::default();

        let mut vk_device_features2 = vk::PhysicalDeviceFeatures2::default();
        let mut vk_vulkan11_features = vk::PhysicalDeviceVulkan11Features::default();
        let mut vk_vulkan12_features = vk::PhysicalDeviceVulkan12Features::default();
        let mut vk_vulkan13_features = vk::PhysicalDeviceVulkan13Features::default();
        let mut vk_vertex_attribute_divisor_features =
            vk::PhysicalDeviceVertexAttributeDivisorFeaturesEXT::default();
        let mut vk_barycentric_features =
            vk::PhysicalDeviceFragmentShaderBarycentricFeaturesKHR::default();
        let mut vk_line_rasterization_features =
            vk::PhysicalDeviceLineRasterizationFeaturesKHR::default();

        let has_ext = |name: &str| supported_extensions.contains(&name);

        // ---- timestamp support via queue family properties -------------------
        let supports_timestamps = unsafe {
            let queue_families =
                instance.get_physical_device_queue_family_properties(physical_device);
            let idx = gfx_queue_family_index as usize;
            if idx < queue_families.len() {
                queue_families[idx].timestamp_valid_bits > 0
            } else {
                log::warn!(
                    "gfx_queue_family_index {} out of range ({}), no timestamps",
                    gfx_queue_family_index,
                    queue_families.len()
                );
                false
            }
        };

        // ---- query properties (pNext chain) ---------------------------------
        // Chain: vkDeviceProperties2 -> vkPhysicalDeviceIdProperties ->
        //        vkVertexAttributeDivisorProperties
        unsafe {
            // Build the chain manually via raw pNext pointers so we avoid
            // ash builder lifetime limitations with local structs.
            vk_vertex_attribute_divisor_properties.p_next = std::ptr::null_mut();
            vk_physical_device_id_properties.p_next =
                &mut vk_vertex_attribute_divisor_properties as *mut _ as *mut std::ffi::c_void;
            vk_device_properties2.p_next =
                &mut vk_physical_device_id_properties as *mut _ as *mut std::ffi::c_void;

            instance.get_physical_device_properties2(physical_device, &mut vk_device_properties2);
        }

        let vk_memory_properties =
            unsafe { instance.get_physical_device_memory_properties(physical_device) };

        // ---- query features (pNext chain) -----------------------------------
        // Order mirrors C++: 1.1 -> 1.2 -> 1.3 -> vertex divisor -> barycentric -> line raster
        let barycentric_ext_supported = has_ext("VK_KHR_fragment_shader_barycentric");
        let line_rasterization_ext_supported = has_ext("VK_KHR_line_rasterization");

        unsafe {
            vk_vulkan11_features.p_next = std::ptr::null_mut();
            vk_vulkan12_features.p_next =
                &mut vk_vulkan11_features as *mut _ as *mut std::ffi::c_void;
            vk_vulkan13_features.p_next =
                &mut vk_vulkan12_features as *mut _ as *mut std::ffi::c_void;
            vk_vertex_attribute_divisor_features.p_next =
                &mut vk_vulkan13_features as *mut _ as *mut std::ffi::c_void;

            let mut tail: *mut std::ffi::c_void =
                &mut vk_vertex_attribute_divisor_features as *mut _ as *mut std::ffi::c_void;

            if barycentric_ext_supported {
                vk_barycentric_features.p_next = tail;
                tail = &mut vk_barycentric_features as *mut _ as *mut std::ffi::c_void;
            }
            if line_rasterization_ext_supported {
                vk_line_rasterization_features.p_next = tail;
                tail = &mut vk_line_rasterization_features as *mut _ as *mut std::ffi::c_void;
            }

            vk_device_features2.p_next = tail;
            instance.get_physical_device_features2(physical_device, &mut vk_device_features2);
        }

        // ---- native interop detection (mirrors platform #ifdefs) ------------
        let supports_native_interop = {
            let base_ok = enable_native_interop()
                && has_ext("VK_KHR_external_memory")
                && has_ext("VK_KHR_external_semaphore");
            #[cfg(target_os = "windows")]
            {
                base_ok
                    && has_ext("VK_KHR_external_memory_win32")
                    && has_ext("VK_KHR_external_semaphore_win32")
            }
            #[cfg(all(unix, not(target_os = "macos")))]
            {
                base_ok
                    && has_ext("VK_KHR_external_memory_fd")
                    && has_ext("VK_KHR_external_semaphore_fd")
            }
            #[cfg(not(any(target_os = "windows", all(unix, not(target_os = "macos")))))]
            {
                let _ = base_ok;
                false
            }
        };

        // Host image copy is reported by device extension; we keep a simple flag here.
        // Full host-image-copy support requires checking
        // VkPhysicalDeviceHostImageCopyFeaturesEXT.hostImageCopy and
        // identicalMemoryTypeRequirements — that requires the extension loader.
        // For now we record availability; callers may query via ash extensions as needed.
        let supports_host_image_copy = has_ext("VK_EXT_host_image_copy");

        // ---- UMA / ReBAR detection ------------------------------------------
        let host_accessible = supports_host_accessible_device_memory(&vk_memory_properties);
        let props = &vk_device_properties2.properties;
        let is_integrated_or_cpu = matches!(
            props.device_type,
            vk::PhysicalDeviceType::INTEGRATED_GPU | vk::PhysicalDeviceType::CPU
        );
        let is_uma = host_accessible && is_integrated_or_cpu;
        let is_rebar = host_accessible && !is_uma;
        let unified_memory = (is_uma && enable_uma()) || (is_rebar && enable_rebar());

        // ---- capability flags -----------------------------------------------
        let conservative_raster = has_ext("VK_EXT_conservative_rasterization");
        let shader_draw_parameters = vk_vulkan11_features.shader_draw_parameters == vk::TRUE;
        let multi_draw_indirect = enable_multi_draw_indirect();
        let builtin_barycentrics = barycentric_ext_supported
            && vk_barycentric_features.fragment_shader_barycentric == vk::TRUE
            && enable_builtin_barycentrics();

        let limits = &props.limits;

        let mut base = HgiCapabilities::default();
        base.api_version = props.api_version as i32;
        // GetShaderVersion() always returns 450 (GLSL 4.50 target).
        base.shader_version = 450;
        base.max_uniform_block_size = limits.max_uniform_buffer_range as usize;
        base.max_storage_block_size = limits.max_storage_buffer_range as usize;
        base.uniform_buffer_offset_alignment = limits.min_uniform_buffer_offset_alignment as usize;
        base.max_clip_distances = limits.max_clip_distances as usize;
        base.max_texture_dimension_2d = limits.max_image_dimension2_d as i32;
        base.max_texture_dimension_3d = limits.max_image_dimension3_d as i32;
        base.max_texture_layers = limits.max_image_array_layers as i32;
        base.max_vertex_attributes = limits.max_vertex_input_attributes as i32;
        base.max_color_attachments = limits.max_color_attachments as i32;
        base.max_compute_work_group_size = limits.max_compute_work_group_size;
        base.max_compute_work_group_invocations = limits.max_compute_work_group_invocations;
        base.max_cull_distances = limits.max_cull_distances as i32;
        base.max_combined_clip_and_cull_distances =
            limits.max_combined_clip_and_cull_distances as i32;
        base.uses_unified_memory = unified_memory;

        if unified_memory {
            base.enable(HgiDeviceCapabilities::UNIFIED_MEMORY);
        }
        // Vulkan uses [0,1] depth range, not [-1,1].
        // No flag set for DepthRangeMinusOneToOne.
        base.enable(HgiDeviceCapabilities::STENCIL_READBACK);
        base.enable(HgiDeviceCapabilities::SHADER_DOUBLE_PRECISION);
        base.enable(HgiDeviceCapabilities::SINGLE_SLOT_RESOURCE_ARRAYS);
        if conservative_raster {
            base.enable(HgiDeviceCapabilities::CONSERVATIVE_RASTER);
        }
        if builtin_barycentrics {
            base.enable(HgiDeviceCapabilities::BUILTIN_BARYCENTRICS);
        }
        if shader_draw_parameters {
            base.enable(HgiDeviceCapabilities::SHADER_DRAW_PARAMETERS);
        }
        if multi_draw_indirect {
            base.enable(HgiDeviceCapabilities::MULTI_DRAW_INDIRECT);
        }

        if crate::instance::is_debug_enabled() {
            let mem_tag = if unified_memory {
                if is_rebar { " (ReBAR)" } else { " (UMA)" }
            } else {
                ""
            };
            let name = unsafe {
                std::ffi::CStr::from_ptr(props.device_name.as_ptr())
                    .to_string_lossy()
                    .into_owned()
            };
            log::info!("Selected GPU: \"{}\"{}", name, mem_tag);
        }

        Self {
            base,
            supports_timestamps,
            supports_native_interop,
            supports_host_image_copy,
            vk_device_properties2,
            vk_memory_properties,
            vk_physical_device_id_properties,
            vk_vertex_attribute_divisor_properties,
            vk_device_features2,
            vk_vulkan11_features,
            vk_vulkan12_features,
            vk_vulkan13_features,
            vk_vertex_attribute_divisor_features,
            vk_barycentric_features,
            vk_line_rasterization_features,
        }
    }

    // ---- accessors mirroring C++ virtuals -----------------------------------

    /// Returns the raw packed Vulkan API version from device properties.
    ///
    /// Mirrors C++ `HgiVulkanCapabilities::GetAPIVersion()`.
    pub fn get_api_version(&self) -> u32 {
        self.vk_device_properties2.properties.api_version
    }

    /// Returns the GLSL shader language version target (always 450).
    ///
    /// Mirrors C++ `HgiVulkanCapabilities::GetShaderVersion()` which returns 450
    /// for compatibility — this is the GLSL version, not the Vulkan API version.
    pub fn get_shader_version(&self) -> i32 {
        450
    }

    /// Returns a reference to the physical device properties struct.
    pub fn device_properties(&self) -> &vk::PhysicalDeviceProperties {
        &self.vk_device_properties2.properties
    }

    /// Returns a reference to the physical device memory properties.
    pub fn memory_properties(&self) -> &vk::PhysicalDeviceMemoryProperties {
        &self.vk_memory_properties
    }

    /// Returns a reference to base HGI capabilities.
    pub fn base_capabilities(&self) -> &HgiCapabilities {
        &self.base
    }

    /// Convenience: whether a device extension was reported as supported.
    ///
    /// In the full implementation the extension list is owned by HgiVulkanDevice;
    /// here the caller already filtered it through `supported_extensions` at
    /// construction time, so individual flags are stored on the struct.
    pub fn supports_conservative_raster(&self) -> bool {
        self.base
            .supports(HgiDeviceCapabilities::CONSERVATIVE_RASTER)
    }
}

/// Default (stub) capabilities — zeroed out, no Vulkan device queried.
///
/// Used by `HgiVulkan::new()` before a real device is selected.
impl Default for HgiVulkanCapabilities {
    fn default() -> Self {
        Self {
            base: HgiCapabilities::default(),
            supports_timestamps: false,
            supports_native_interop: false,
            supports_host_image_copy: false,
            vk_device_properties2: vk::PhysicalDeviceProperties2::default(),
            vk_memory_properties: vk::PhysicalDeviceMemoryProperties::default(),
            vk_physical_device_id_properties: vk::PhysicalDeviceIDProperties::default(),
            vk_vertex_attribute_divisor_properties:
                vk::PhysicalDeviceVertexAttributeDivisorPropertiesEXT::default(),
            vk_device_features2: vk::PhysicalDeviceFeatures2::default(),
            vk_vulkan11_features: vk::PhysicalDeviceVulkan11Features::default(),
            vk_vulkan12_features: vk::PhysicalDeviceVulkan12Features::default(),
            vk_vulkan13_features: vk::PhysicalDeviceVulkan13Features::default(),
            vk_vertex_attribute_divisor_features:
                vk::PhysicalDeviceVertexAttributeDivisorFeaturesEXT::default(),
            vk_barycentric_features:
                vk::PhysicalDeviceFragmentShaderBarycentricFeaturesKHR::default(),
            vk_line_rasterization_features: vk::PhysicalDeviceLineRasterizationFeaturesKHR::default(
            ),
        }
    }
}
