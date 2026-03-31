//! Vulkan logical device — physical device selection, logical device creation,
//! VMA allocator, command queue, and pipeline cache ownership.
//!
//! Port of pxr/imaging/hgiVulkan/device.cpp/.h

// All construction paths require unsafe Vulkan calls.
#![allow(unsafe_code)]

use std::ffi::CStr;
use std::sync::Arc;

use ash::vk;
use gpu_allocator::vulkan::{Allocator, AllocatorCreateDesc};

use crate::capabilities::HgiVulkanCapabilities;
use crate::command_queue::HgiVulkanCommandQueue;
use crate::instance::HgiVulkanInstance;
use crate::pipeline_cache::HgiVulkanPipelineCache;

// ---------------------------------------------------------------------------
// Preferred device type env-var (mirrors TF_DEFINE_ENV_SETTING)
// ---------------------------------------------------------------------------

/// Reads `HGIVULKAN_PREFERRED_DEVICE_TYPE` and maps integer string values to
/// `VkPhysicalDeviceType`. Defaults to `DISCRETE_GPU` (value 2) when unset or
/// unknown, matching C++ default `VK_PHYSICAL_DEVICE_TYPE_DISCRETE_GPU`.
fn preferred_device_type() -> vk::PhysicalDeviceType {
    match std::env::var("HGIVULKAN_PREFERRED_DEVICE_TYPE")
        .ok()
        .as_deref()
    {
        Some("0") | Some("OTHER") => vk::PhysicalDeviceType::OTHER,
        Some("1") | Some("INTEGRATED_GPU") => vk::PhysicalDeviceType::INTEGRATED_GPU,
        Some("3") | Some("VIRTUAL_GPU") => vk::PhysicalDeviceType::VIRTUAL_GPU,
        Some("4") | Some("CPU") => vk::PhysicalDeviceType::CPU,
        _ => vk::PhysicalDeviceType::DISCRETE_GPU,
    }
}

// ---------------------------------------------------------------------------
// Queue family helpers
// ---------------------------------------------------------------------------

/// Returns the index of the first queue family that supports GRAPHICS, or
/// `vk::QUEUE_FAMILY_IGNORED` when none is found.
///
/// Mirrors `_GetGraphicsQueueFamilyIndex()` from device.cpp.
fn get_graphics_queue_family_index(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
) -> u32 {
    let families = unsafe { instance.get_physical_device_queue_family_properties(physical_device) };

    for (i, family) in families.iter().enumerate() {
        if family.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
            return i as u32;
        }
    }

    vk::QUEUE_FAMILY_IGNORED
}

/// Returns true when the given queue family supports presentation on the
/// current platform.
///
/// Mirrors `_SupportsPresentation()` from device.cpp. On Windows we check
/// Win32 presentation support via the `VK_KHR_win32_surface` extension loader
/// (requires both `Entry` and `Instance`). On Linux/macOS we return true
/// unconditionally, matching the C++ Metal branch and the typical headless
/// Xlib path where opening a display is impractical at device-selection time.
fn supports_presentation(
    hgi_instance: &HgiVulkanInstance,
    physical_device: vk::PhysicalDevice,
    family_index: u32,
) -> bool {
    #[cfg(target_os = "windows")]
    {
        let win32_surface = ash::khr::win32_surface::Instance::new(
            hgi_instance.entry(),
            hgi_instance.vk_instance(),
        );
        // SAFETY: physical_device and family_index are valid for this instance.
        unsafe {
            win32_surface
                .get_physical_device_win32_presentation_support(physical_device, family_index)
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        // Suppress unused-variable warnings on non-Windows platforms.
        let _ = (hgi_instance, physical_device, family_index);
        // Mirrors C++ Metal branch: "Presentation currently always supported".
        true
    }
}

// ---------------------------------------------------------------------------
// HgiVulkanDevice
// ---------------------------------------------------------------------------

/// Vulkan logical device — owns the `VkDevice`, GPU allocator, command queue,
/// capabilities, and pipeline cache.
///
/// Port of C++ `HgiVulkanDevice`.
pub struct HgiVulkanDevice {
    // Drop order: Rust drops fields top-to-bottom in declaration order.
    // All objects that hold internal Vulkan handles referencing the VkDevice
    // must appear BEFORE `device` so they are destroyed first.
    /// Command queue — manages pools, submission, timeline semaphore.
    /// Holds an `Arc<ash::Device>` clone; must drop before `device`.
    command_queue: HgiVulkanCommandQueue,
    /// GPU memory allocator (`gpu-allocator` VMA equivalent).
    /// Stores a clone of `ash::Device`; must drop before `device`.
    allocator: Allocator,
    /// Pipeline cache stub (VK_NULL_HANDLE until upstream wires it up).
    pipeline_cache: HgiVulkanPipelineCache,
    /// Device-level debug utils loader — present only when HGIVULKAN_DEBUG=1.
    debug_utils_device: Option<ash::ext::debug_utils::Device>,
    /// Extension names advertised by the physical device (used by `is_supported_extension`).
    supported_extensions: Vec<String>,
    /// Physical device capabilities and feature flags.
    capabilities: HgiVulkanCapabilities,
    /// Graphics queue family index.
    gfx_queue_family_index: u32,
    /// Physical device handle (never null after successful construction).
    physical_device: vk::PhysicalDevice,
    /// Raw ash logical device — dropped last so all dependents above are clean.
    /// Wrapped in `Arc` so `HgiVulkanCommandQueue` can hold a clone.
    device: Arc<ash::Device>,
}

impl HgiVulkanDevice {
    /// Creates a logical device from the best available physical device.
    ///
    /// Selection order mirrors C++:
    /// 1. Enumerate physical devices.
    /// 2. Skip devices without a graphics queue or without Vulkan 1.3 support.
    /// 3. Prefer `HGIVULKAN_PREFERRED_DEVICE_TYPE` (default: `DISCRETE_GPU`).
    /// 4. Fall back to the first acceptable device if none matches the preference.
    ///
    /// # Errors
    /// Returns a `String` describing the first Vulkan error or configuration
    /// problem encountered.
    pub fn new(instance: &HgiVulkanInstance) -> Result<Self, String> {
        let vk_instance = instance.vk_instance();

        // ------------------------------------------------------------------
        // 1. Physical device selection
        // ------------------------------------------------------------------
        let physical_devices = unsafe {
            vk_instance
                .enumerate_physical_devices()
                .map_err(|e| format!("vkEnumeratePhysicalDevices failed: {e}"))?
        };

        if physical_devices.is_empty() {
            return Err("VULKAN_ERROR: No physical devices found".to_owned());
        }

        let preferred_type = preferred_device_type();

        let mut selected_device: Option<vk::PhysicalDevice> = None;
        let mut selected_family_index: u32 = 0;

        for &pd in &physical_devices {
            let props = unsafe { vk_instance.get_physical_device_properties(pd) };

            // Require Vulkan 1.3+, matching `if (props.apiVersion < VK_API_VERSION_1_3)`.
            if props.api_version < vk::API_VERSION_1_3 {
                continue;
            }

            let family_index = get_graphics_queue_family_index(vk_instance, pd);
            if family_index == vk::QUEUE_FAMILY_IGNORED {
                continue;
            }

            // When the instance has presentation extensions, require that
            // the queue family supports presenting to the OS window system.
            if instance.has_presentation() && !supports_presentation(instance, pd, family_index) {
                continue;
            }

            if props.device_type == preferred_type {
                // Preferred type found — use it immediately.
                selected_device = Some(pd);
                selected_family_index = family_index;
                break;
            }

            // Keep the first acceptable device as fallback.
            if selected_device.is_none() {
                selected_device = Some(pd);
                selected_family_index = family_index;
            }
        }

        let physical_device =
            selected_device.ok_or("VULKAN_ERROR: Unable to determine physical device")?;

        // ------------------------------------------------------------------
        // 2. Enumerate device extensions
        // ------------------------------------------------------------------
        let ext_props = unsafe {
            vk_instance
                .enumerate_device_extension_properties(physical_device)
                .map_err(|e| format!("vkEnumerateDeviceExtensionProperties failed: {e}"))?
        };

        // Convert to owned Strings for lifetime-free storage.
        let supported_extensions: Vec<String> = ext_props
            .iter()
            .map(|p| {
                // SAFETY: Vulkan fills extension_name with a valid C string.
                unsafe { CStr::from_ptr(p.extension_name.as_ptr()) }
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();

        let has_ext = |name: &str| -> bool { supported_extensions.iter().any(|e| e == name) };

        // ------------------------------------------------------------------
        // 3. Capabilities — queried before device creation to populate the
        //    pNext feature chains that are passed into vkCreateDevice.
        // ------------------------------------------------------------------

        // Gather the extension slice already known to be supported; this is
        // what HgiVulkanCapabilities needs to gate optional feature queries.
        let ext_slice: Vec<&str> = supported_extensions.iter().map(String::as_str).collect();

        let capabilities = HgiVulkanCapabilities::new(
            vk_instance,
            physical_device,
            selected_family_index,
            &ext_slice,
        );

        // ------------------------------------------------------------------
        // 4. Build the enabled-extension list (mirrors C++ extension block)
        // ------------------------------------------------------------------
        let mut extensions: Vec<*const i8> = Vec::new();
        // Use CStr literals for all extension names — they are 'static so the
        // raw pointers remain valid for the duration of vkCreateDevice.

        macro_rules! push_if_supported {
            ($name:expr) => {
                if has_ext($name) {
                    extensions.push(c_str_ptr($name));
                }
            };
        }

        // Swapchain — optional for surfaceless builds (e.g. Lavapipe).
        push_if_supported!("VK_KHR_swapchain");

        // Dedicated allocations — VMA can use these for performance.
        let dedicated_allocations =
            has_ext("VK_KHR_get_memory_requirements2") && has_ext("VK_KHR_dedicated_allocation");
        if dedicated_allocations {
            extensions.push(c_str_ptr("VK_KHR_get_memory_requirements2"));
            extensions.push(c_str_ptr("VK_KHR_dedicated_allocation"));
        }

        // Platform-specific GL/interop extensions.
        #[cfg(target_os = "windows")]
        {
            if has_ext("VK_KHR_external_memory")
                && has_ext("VK_KHR_external_semaphore")
                && has_ext("VK_KHR_external_memory_win32")
                && has_ext("VK_KHR_external_semaphore_win32")
            {
                extensions.push(c_str_ptr("VK_KHR_external_semaphore"));
                extensions.push(c_str_ptr("VK_KHR_external_memory"));
                extensions.push(c_str_ptr("VK_KHR_external_memory_win32"));
                extensions.push(c_str_ptr("VK_KHR_external_semaphore_win32"));
            }
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            if has_ext("VK_KHR_external_memory")
                && has_ext("VK_KHR_external_semaphore")
                && has_ext("VK_KHR_external_memory_fd")
                && has_ext("VK_KHR_external_semaphore_fd")
            {
                extensions.push(c_str_ptr("VK_KHR_external_semaphore"));
                extensions.push(c_str_ptr("VK_KHR_external_memory"));
                extensions.push(c_str_ptr("VK_KHR_external_memory_fd"));
                extensions.push(c_str_ptr("VK_KHR_external_semaphore_fd"));
            }
        }

        // Memory budget query.
        let supports_mem_budget = has_ext("VK_EXT_memory_budget");
        if supports_mem_budget {
            extensions.push(c_str_ptr("VK_EXT_memory_budget"));
        }

        // Depth/stencil resolve during render pass — requires a chain of deps.
        if has_ext("VK_KHR_depth_stencil_resolve") {
            extensions.push(c_str_ptr("VK_KHR_depth_stencil_resolve"));
            extensions.push(c_str_ptr("VK_KHR_create_renderpass2"));
            extensions.push(c_str_ptr("VK_KHR_multiview"));
            extensions.push(c_str_ptr("VK_KHR_maintenance2"));
        }

        // Scalar block layout (shared C++/GLSL struct layout via `scalar` qualifier).
        if has_ext("VK_EXT_scalar_block_layout") {
            extensions.push(c_str_ptr("VK_EXT_scalar_block_layout"));
        } else {
            log::warn!("Unsupported VK_EXT_scalar_block_layout. Update gfx driver?");
        }

        push_if_supported!("VK_EXT_conservative_rasterization");
        push_if_supported!("VK_KHR_fragment_shader_barycentric");
        push_if_supported!("VK_KHR_shader_draw_parameters");
        push_if_supported!("VK_EXT_vertex_attribute_divisor");
        push_if_supported!("VK_KHR_line_rasterization");
        push_if_supported!("VK_EXT_host_image_copy");

        // Negative-Y viewport flip (required to match OpenGL convention).
        // Promoted to core in Vulkan 1.1, but we still push the extension
        // name for compat with older drivers that expose it separately.
        if has_ext("VK_KHR_maintenance1") {
            extensions.push(c_str_ptr("VK_KHR_maintenance1"));
        }

        #[cfg(target_os = "macos")]
        push_if_supported!("VK_KHR_portability_subset");

        // ------------------------------------------------------------------
        // 5. Build pNext feature chain (mirrors C++ VkPhysicalDeviceFeatures2
        //    chain with Vulkan 1.1/1.2/1.3 + optional extension features).
        //    We copy the values queried in HgiVulkanCapabilities to avoid
        //    enabling features the device doesn't actually have.
        // ------------------------------------------------------------------
        let cap = &capabilities;

        let mut features2 = vk::PhysicalDeviceFeatures2::default();
        {
            let src = &cap.vk_device_features2.features;
            let dst = &mut features2.features;
            dst.multi_draw_indirect = src.multi_draw_indirect;
            dst.sampler_anisotropy = src.sampler_anisotropy;
            dst.shader_sampled_image_array_dynamic_indexing =
                src.shader_sampled_image_array_dynamic_indexing;
            dst.shader_storage_image_array_dynamic_indexing =
                src.shader_storage_image_array_dynamic_indexing;
            dst.sample_rate_shading = src.sample_rate_shading;
            dst.shader_clip_distance = src.shader_clip_distance;
            dst.tessellation_shader = src.tessellation_shader;
            dst.depth_clamp = src.depth_clamp;
            dst.shader_float64 = src.shader_float64;
            dst.fill_mode_non_solid = src.fill_mode_non_solid;
            dst.alpha_to_one = src.alpha_to_one;
            dst.vertex_pipeline_stores_and_atomics = src.vertex_pipeline_stores_and_atomics;
            dst.fragment_stores_and_atomics = src.fragment_stores_and_atomics;
            dst.shader_int64 = src.shader_int64;
            dst.geometry_shader = src.geometry_shader;
        }

        let mut vk11 = vk::PhysicalDeviceVulkan11Features::default();
        vk11.shader_draw_parameters = cap.vk_vulkan11_features.shader_draw_parameters;

        let mut vk12 = vk::PhysicalDeviceVulkan12Features::default();
        vk12.timeline_semaphore = cap.vk_vulkan12_features.timeline_semaphore;

        let mut vk13 = vk::PhysicalDeviceVulkan13Features::default();
        vk13.shader_demote_to_helper_invocation =
            cap.vk_vulkan13_features.shader_demote_to_helper_invocation;

        let mut vertex_attr_div = vk::PhysicalDeviceVertexAttributeDivisorFeaturesEXT::default();
        vertex_attr_div.vertex_attribute_instance_rate_divisor = cap
            .vk_vertex_attribute_divisor_features
            .vertex_attribute_instance_rate_divisor;

        let mut barycentric = vk::PhysicalDeviceFragmentShaderBarycentricFeaturesKHR::default();
        let use_barycentric = has_ext("VK_KHR_fragment_shader_barycentric");
        if use_barycentric {
            barycentric.fragment_shader_barycentric =
                cap.vk_barycentric_features.fragment_shader_barycentric;
        }

        let mut line_raster = vk::PhysicalDeviceLineRasterizationFeaturesKHR::default();
        let use_line_raster = has_ext("VK_KHR_line_rasterization");
        if use_line_raster {
            line_raster.bresenham_lines = cap.vk_line_rasterization_features.bresenham_lines;
        }

        let mut host_image_copy = vk::PhysicalDeviceHostImageCopyFeaturesEXT::default();
        let use_host_image_copy = has_ext("VK_EXT_host_image_copy");
        if use_host_image_copy {
            host_image_copy.host_image_copy = if cap.supports_host_image_copy {
                vk::TRUE
            } else {
                vk::FALSE
            };
        }

        // Build the pNext chain manually via raw pointer assignment, mirroring C++.
        // Order (innermost first): vk11 -> vk12 -> vk13 -> vertex_attr_div
        //                          -> [barycentric] -> [line_raster]
        //                          -> [host_image_copy] -> features2
        //
        // All structs are local variables that live until vkCreateDevice returns.
        // Raw pointer assignment itself is safe; the pointers are only read by
        // the Vulkan driver during vkCreateDevice (no unsafe Rust dereference).
        vk11.p_next = std::ptr::null_mut();
        vk12.p_next = &mut vk11 as *mut _ as *mut std::ffi::c_void;
        vk13.p_next = &mut vk12 as *mut _ as *mut std::ffi::c_void;
        vertex_attr_div.p_next = &mut vk13 as *mut _ as *mut std::ffi::c_void;

        let mut tail: *mut std::ffi::c_void =
            &mut vertex_attr_div as *mut _ as *mut std::ffi::c_void;

        if use_barycentric {
            barycentric.p_next = tail;
            tail = &mut barycentric as *mut _ as *mut std::ffi::c_void;
        }
        if use_line_raster {
            line_raster.p_next = tail;
            tail = &mut line_raster as *mut _ as *mut std::ffi::c_void;
        }
        if use_host_image_copy {
            host_image_copy.p_next = tail;
            tail = &mut host_image_copy as *mut _ as *mut std::ffi::c_void;
        }

        features2.p_next = tail;

        // ------------------------------------------------------------------
        // 6. Create the logical device
        // ------------------------------------------------------------------
        let queue_priority = 1.0_f32;
        let queue_create_info = vk::DeviceQueueCreateInfo::default()
            .queue_family_index(selected_family_index)
            .queue_priorities(std::slice::from_ref(&queue_priority));

        // Build DeviceCreateInfo with a pNext chain that points to features2.
        // We cannot use `.push_next()` here because ash's push_next overwrites
        // features2.p_next, destroying the manually-built Vulkan 1.1/1.2/1.3
        // feature chain. Instead we set p_next directly on the raw struct.
        let mut device_create_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(std::slice::from_ref(&queue_create_info))
            .enabled_extension_names(&extensions);
        // features2.p_next already points to the complete vk11->vk12->vk13->...
        // chain built above; we just wire device_create_info.p_next to features2.
        device_create_info.p_next = &features2 as *const _ as *const std::ffi::c_void;

        let raw_device = unsafe {
            vk_instance
                .create_device(physical_device, &device_create_info, None)
                .map_err(|e| format!("vkCreateDevice failed: {e}"))?
        };

        let device = Arc::new(raw_device);

        // ------------------------------------------------------------------
        // 7. Debug utils device extension loader
        // ------------------------------------------------------------------
        let debug_utils_device = if crate::diagnostic::is_debug_enabled() {
            Some(ash::ext::debug_utils::Device::new(vk_instance, &device))
        } else {
            None
        };

        // ------------------------------------------------------------------
        // 8. GPU memory allocator (VMA equivalent via gpu-allocator)
        // ------------------------------------------------------------------
        let allocator = Allocator::new(&AllocatorCreateDesc {
            instance: vk_instance.clone(),
            device: (*device).clone(),
            physical_device,
            debug_settings: Default::default(),
            // buffer_device_address requires VkPhysicalDeviceBufferDeviceAddressFeatures;
            // we do not enable that feature in this port, so keep it false.
            buffer_device_address: false,
            allocation_sizes: Default::default(),
        })
        .map_err(|e| format!("gpu-allocator Allocator::new failed: {e}"))?;

        // ------------------------------------------------------------------
        // 9. Command queue and pipeline cache
        // ------------------------------------------------------------------
        let command_queue = HgiVulkanCommandQueue::new(Arc::clone(&device), selected_family_index)
            .map_err(|e| format!("HgiVulkanCommandQueue::new failed: {e}"))?;

        let pipeline_cache = HgiVulkanPipelineCache::new();

        Ok(Self {
            command_queue,
            allocator,
            pipeline_cache,
            debug_utils_device,
            supported_extensions,
            capabilities,
            gfx_queue_family_index: selected_family_index,
            physical_device,
            device,
        })
    }

    // ------------------------------------------------------------------
    // Accessors — mirror C++ Get* methods
    // ------------------------------------------------------------------

    /// Returns a reference to the `ash` logical device.
    ///
    /// Mirrors `GetVulkanDevice()`.
    pub fn vk_device(&self) -> &ash::Device {
        &self.device
    }

    /// Returns a reference to the GPU memory allocator.
    ///
    /// Mirrors `GetVulkanMemoryAllocator()`.
    pub fn allocator(&self) -> &Allocator {
        &self.allocator
    }

    /// Returns a mutable reference to the GPU memory allocator.
    pub fn allocator_mut(&mut self) -> &mut Allocator {
        &mut self.allocator
    }

    /// Returns a reference to the command queue.
    ///
    /// Mirrors `GetCommandQueue()`.
    pub fn command_queue(&self) -> &HgiVulkanCommandQueue {
        &self.command_queue
    }

    /// Returns a mutable reference to the command queue.
    pub fn command_queue_mut(&mut self) -> &mut HgiVulkanCommandQueue {
        &mut self.command_queue
    }

    /// Returns a reference to the device capabilities.
    ///
    /// Mirrors `GetDeviceCapabilities()`.
    pub fn capabilities(&self) -> &HgiVulkanCapabilities {
        &self.capabilities
    }

    /// Returns the graphics queue family index.
    ///
    /// Mirrors `GetGfxQueueFamilyIndex()`.
    pub fn gfx_queue_family_index(&self) -> u32 {
        self.gfx_queue_family_index
    }

    /// Returns the raw `VkPhysicalDevice` handle.
    ///
    /// Mirrors `GetVulkanPhysicalDevice()`.
    pub fn physical_device(&self) -> vk::PhysicalDevice {
        self.physical_device
    }

    /// Returns a reference to the pipeline cache.
    ///
    /// Mirrors `GetPipelineCache()`.
    pub fn pipeline_cache(&self) -> &HgiVulkanPipelineCache {
        &self.pipeline_cache
    }

    /// Blocks until all GPU work has finished.
    ///
    /// Mirrors `WaitForIdle()`. Use sparingly — this is a full device stall.
    pub fn wait_for_idle(&self) {
        // SAFETY: device handle is valid for the lifetime of this struct.
        if let Err(e) = unsafe { self.device.device_wait_idle() } {
            log::error!("vkDeviceWaitIdle failed: {:?}", e);
        }
    }

    /// Returns true when `name` is in the list of supported device extensions.
    ///
    /// Mirrors `IsSupportedExtension()`.
    pub fn is_supported_extension(&self, name: &str) -> bool {
        self.supported_extensions.iter().any(|e| e == name)
    }

    /// Returns the debug utils device extension loader when debug is enabled.
    pub fn debug_utils_device(&self) -> Option<&ash::ext::debug_utils::Device> {
        self.debug_utils_device.as_ref()
    }
}

impl Drop for HgiVulkanDevice {
    fn drop(&mut self) {
        // Stall the GPU before any Vulkan objects are destroyed.
        // Mirrors C++ `if (_vkDevice) { vkDeviceWaitIdle(_vkDevice); }`.
        self.wait_for_idle();

        // After this `drop()` body returns, Rust automatically drops all
        // struct fields in top-to-bottom declaration order:
        //
        //   1. command_queue  — destroys semaphore + command pools
        //   2. allocator      — releases gpu-allocator internal state
        //   3. pipeline_cache — no-op (null handle)
        //   4. debug_utils_device, supported_extensions, capabilities — trivial
        //   5. gfx_queue_family_index, physical_device — plain values
        //   6. device (Arc)   — vkDestroyDevice fires when refcount hits 0
        //
        // The struct field order was chosen so that all objects holding
        // internal VkDevice references are destroyed before the device itself.
    }
}

// ---------------------------------------------------------------------------
// Internal helper: stable &'static CStr pointer from a &str literal.
// ---------------------------------------------------------------------------

/// Returns a raw `*const i8` suitable for `ppEnabledExtensionNames` from a
/// UTF-8 string that is known to be a null-terminated ASCII extension name.
///
/// # Safety
/// The caller must ensure `s` ends with `\0` or that the returned pointer is
/// only used while a matching `CString`/`CStr` is alive. Here we rely on
/// the Vulkan extension name constants being `'static` string slices that
/// Vulkan guarantees are already null-terminated when coming from C headers.
///
/// Because we construct these from Rust `&str` literals that do NOT include
/// a trailing `\0`, we use a thread-local `CString` cache instead.
fn c_str_ptr(s: &'static str) -> *const i8 {
    // We build a CString inline. The pointer is valid for the duration of
    // the `vkCreateDevice` call because we collect all pointers into the
    // `extensions` Vec before the call, and the CStrings are pushed into a
    // parallel `owned` Vec that lives until the end of `new()`.
    //
    // IMPORTANT: This function is only called from within `new()` where the
    // `owned` Vec is kept alive by the caller. See usage site.
    //
    // Actually, since the extensions Vec stores raw pointers we must keep the
    // backing CStrings alive. The approach below leaks them into a thread-local
    // storage that is valid for the program lifetime — acceptable because
    // extension name strings are fixed ASCII constants.
    use std::collections::HashMap;
    use std::sync::Mutex;

    static CACHE: std::sync::OnceLock<Mutex<HashMap<&'static str, std::ffi::CString>>> =
        std::sync::OnceLock::new();

    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = cache.lock().unwrap();
    let cstr = map
        .entry(s)
        .or_insert_with(|| std::ffi::CString::new(s).expect("extension name must not contain NUL"));
    cstr.as_ptr()
}
