//! Vulkan instance creation and management.
//!
//! Port of pxr/imaging/hgiVulkan/instance.cpp/.h

use ash::vk;
use std::ffi::{CStr, CString};

/// Returns true if HGIVULKAN_DEBUG=1 is set in the environment.
pub fn is_debug_enabled() -> bool {
    std::env::var("HGIVULKAN_DEBUG").as_deref() == Ok("1")
}

/// Returns true if HGIVULKAN_DEBUG_VERBOSE=1 is set in the environment.
pub fn is_verbose_debug_enabled() -> bool {
    std::env::var("HGIVULKAN_DEBUG_VERBOSE").as_deref() == Ok("1")
}

/// Validation messages intentionally suppressed in non-verbose mode.
///
/// These correspond to known, accepted warnings in USD rendering passes (OIT, shadow pass)
/// where attachment/shader I/O mismatches are by design.
const IGNORED_MESSAGE_PREFIXES: &[&str] = &[
    "Validation Warning: [ Undefined-Value-ShaderInputNotProduced ]",
    "Validation Warning: [ Undefined-Value-ShaderOutputNotConsumed ]",
];

/// Vulkan debug utils messenger callback — mirrors `_VulkanDebugCallback` from diagnostic.cpp.
///
/// Routes severity to `log::error!`, `log::warn!`, or `log::info!`.
///
/// # Safety
/// Called by the Vulkan driver; all pointer arguments are guaranteed valid for the call duration.
#[allow(clippy::unsafe_removed_from_name)] // extern "system" fn must be unsafe
unsafe extern "system" fn vulkan_debug_callback(
    severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    _message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _user_data: *mut std::ffi::c_void,
) -> vk::Bool32 {
    let message = if callback_data.is_null() {
        "<null callback data>".to_owned()
    } else {
        // SAFETY: Vulkan guarantees callback_data and p_message are valid for the call duration.
        let p_message = unsafe { (*callback_data).p_message };
        if p_message.is_null() {
            "<null message>".to_owned()
        } else {
            unsafe { CStr::from_ptr(p_message) }
                .to_string_lossy()
                .into_owned()
        }
    };

    // In non-verbose mode suppress known harmless warnings.
    if !is_verbose_debug_enabled() {
        for prefix in IGNORED_MESSAGE_PREFIXES {
            if message.starts_with(prefix) {
                return vk::FALSE;
            }
        }
    }

    if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::ERROR) {
        log::error!("VULKAN_ERROR: {}", message);
    } else if severity.contains(vk::DebugUtilsMessageSeverityFlagsEXT::WARNING) {
        log::warn!("VULKAN_WARNING: {}", message);
    } else {
        log::info!("VULKAN_MESSAGE: {}", message);
    }

    vk::FALSE
}

/// Filters `desired` down to only layers actually present on the system.
///
/// When debug is enabled and `VK_LAYER_KHRONOS_validation` is absent, logs an error
/// because the validation layer is expected to always be present in debug mode.
fn filter_layers(entry: &ash::Entry, desired: &[&str]) -> Vec<CString> {
    let available = unsafe {
        entry
            .enumerate_instance_layer_properties()
            .unwrap_or_default()
    };

    let mut result = Vec::new();
    for &name in desired {
        let found = available.iter().any(|p| {
            // SAFETY: Vulkan fills layer_name with a valid null-terminated C string.
            unsafe { CStr::from_ptr(p.layer_name.as_ptr()) }
                .to_str()
                .unwrap_or("")
                == name
        });

        if found {
            result.push(CString::new(name).expect("layer name contains interior nul"));
        } else if is_debug_enabled() && name == "VK_LAYER_KHRONOS_validation" {
            log::error!("Instance layer {} is not available, skipping it", name);
        } else {
            log::info!("Instance layer {} is not available, skipping it", name);
        }
    }
    result
}

/// Filters `desired` down to only extensions actually supported by the instance.
fn filter_extensions(entry: &ash::Entry, desired: &[&str]) -> Vec<CString> {
    let available = unsafe {
        entry
            .enumerate_instance_extension_properties(None)
            .unwrap_or_default()
    };

    let mut result = Vec::new();
    for &name in desired {
        let found = available.iter().any(|p| {
            // SAFETY: Vulkan fills extension_name with a valid null-terminated C string.
            unsafe { CStr::from_ptr(p.extension_name.as_ptr()) }
                .to_str()
                .unwrap_or("")
                == name
        });

        if found {
            result.push(CString::new(name).expect("extension name contains interior nul"));
        } else {
            log::info!("Instance extension {} is not available, skipping it", name);
        }
    }
    result
}

/// Vulkan instance — wraps `ash::Entry` + `ash::Instance` with optional debug messenger.
///
/// Port of `HgiVulkanInstance` from pxr/imaging/hgiVulkan/instance.h.
pub struct HgiVulkanInstance {
    /// Vulkan loader entry points.
    entry: ash::Entry,
    /// Live Vulkan instance.
    instance: ash::Instance,
    /// Active debug messenger handle, present only when HGIVULKAN_DEBUG=1.
    debug_messenger: Option<vk::DebugUtilsMessengerEXT>,
    /// Extension function pointers for debug utils, present only when debug is active.
    debug_utils: Option<ash::ext::debug_utils::Instance>,
    /// Whether presentation extensions (VK_KHR_surface) are available.
    has_presentation: bool,
}

impl HgiVulkanInstance {
    /// Creates a Vulkan instance, setting up validation layers and debug messenger
    /// when `HGIVULKAN_DEBUG=1`.
    ///
    /// Mirrors `HgiVulkanInstance::HgiVulkanInstance()` + `HgiVulkanCreateDebug()`.
    pub fn new() -> Result<Self, String> {
        // `Entry::load()` requires the `loaded` feature — dynamically loads vulkan-1.dll/.so.
        let entry = unsafe {
            ash::Entry::load().map_err(|e| format!("Failed to load Vulkan loader: {e}"))?
        };

        // Build desired extension list; platform-specific surface extension included.
        let mut desired_extensions: Vec<&str> = vec![
            "VK_KHR_surface",
            #[cfg(target_os = "windows")]
            "VK_KHR_win32_surface",
            #[cfg(all(unix, not(target_os = "macos")))]
            "VK_KHR_xlib_surface",
            #[cfg(target_os = "macos")]
            "VK_EXT_metal_surface",
            #[cfg(target_os = "macos")]
            "VK_KHR_portability_enumeration",
            // External resource interop (OpenGL/semaphore sharing)
            "VK_KHR_external_memory_capabilities",
            "VK_KHR_external_semaphore_capabilities",
            "VK_KHR_get_physical_device_properties2",
        ];

        let mut desired_layers: Vec<&str> = vec![];

        if is_debug_enabled() {
            desired_layers.push("VK_LAYER_KHRONOS_validation");
            desired_extensions.push("VK_EXT_debug_utils");
        }

        let layers = filter_layers(&entry, &desired_layers);
        let extensions = filter_extensions(&entry, &desired_extensions);

        // Presentation requires VK_KHR_surface to have survived filtering.
        let has_presentation = extensions.iter().any(|e| e.as_c_str() == c"VK_KHR_surface");

        // Raw pointers for Vulkan create info structs (lifetimes bound to this scope).
        let layer_ptrs: Vec<*const i8> = layers.iter().map(|s| s.as_ptr()).collect();
        let extension_ptrs: Vec<*const i8> = extensions.iter().map(|s| s.as_ptr()).collect();

        let app_info = vk::ApplicationInfo::default()
            .application_name(c"HgiVulkan")
            .application_version(vk::make_api_version(0, 1, 0, 0))
            .engine_name(c"HgiVulkan")
            .engine_version(vk::make_api_version(0, 1, 0, 0))
            .api_version(vk::API_VERSION_1_2);

        let mut create_info = vk::InstanceCreateInfo::default()
            .application_info(&app_info)
            .enabled_layer_names(&layer_ptrs)
            .enabled_extension_names(&extension_ptrs);

        // Synchronization validation layer settings (validate_sync=true).
        // Both `layer_settings_info` and `layer_setting` must outlive `create_info`.
        // `vk::Bool32` is u32; `LayerSettingEXT::values()` takes `&[u8]`, so we view it as bytes.
        let sync_val: vk::Bool32 = vk::TRUE;
        let sync_val_bytes: &[u8] = unsafe {
            std::slice::from_raw_parts(
                std::ptr::addr_of!(sync_val).cast::<u8>(),
                std::mem::size_of::<vk::Bool32>(),
            )
        };
        let layer_setting = vk::LayerSettingEXT::default()
            .layer_name(c"VK_LAYER_KHRONOS_validation")
            .setting_name(c"validate_sync")
            .ty(vk::LayerSettingTypeEXT::BOOL32)
            .values(sync_val_bytes);
        let mut layer_settings_info = vk::LayerSettingsCreateInfoEXT::default()
            .settings(std::slice::from_ref(&layer_setting));

        if is_debug_enabled() {
            create_info = create_info.push_next(&mut layer_settings_info);
        }

        // On MoltenVK / macOS the portability enumeration flag is required.
        #[cfg(target_os = "macos")]
        {
            if extensions
                .iter()
                .any(|e| e.as_c_str() == c"VK_KHR_portability_enumeration")
            {
                create_info.flags |= vk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR;
            }
        }

        let instance = unsafe {
            entry
                .create_instance(&create_info, None)
                .map_err(|e| format!("vkCreateInstance failed: {e}"))?
        };

        // Setup debug messenger when requested.
        let (debug_messenger, debug_utils) = if is_debug_enabled() {
            let debug_utils_loader = ash::ext::debug_utils::Instance::new(&entry, &instance);

            let msg_severity = vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                | if is_verbose_debug_enabled() {
                    vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE
                        | vk::DebugUtilsMessageSeverityFlagsEXT::INFO
                } else {
                    vk::DebugUtilsMessageSeverityFlagsEXT::empty()
                };

            let msg_type = vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE;

            let messenger_info = vk::DebugUtilsMessengerCreateInfoEXT::default()
                .message_severity(msg_severity)
                .message_type(msg_type)
                .pfn_user_callback(Some(vulkan_debug_callback));

            let messenger = unsafe {
                debug_utils_loader
                    .create_debug_utils_messenger(&messenger_info, None)
                    .map_err(|e| format!("vkCreateDebugUtilsMessengerEXT failed: {e}"))?
            };

            (Some(messenger), Some(debug_utils_loader))
        } else {
            (None, None)
        };

        Ok(Self {
            entry,
            instance,
            debug_messenger,
            debug_utils,
            has_presentation,
        })
    }

    /// Returns the underlying `ash::Instance`.
    pub fn vk_instance(&self) -> &ash::Instance {
        &self.instance
    }

    /// Returns the Vulkan loader entry points.
    pub fn entry(&self) -> &ash::Entry {
        &self.entry
    }

    /// Whether VK_KHR_surface was available — mirrors `HgiVulkanInstance::HasPresentation()`.
    pub fn has_presentation(&self) -> bool {
        self.has_presentation
    }

    /// Debug utils extension loader, present only when HGIVULKAN_DEBUG=1.
    pub fn debug_utils(&self) -> Option<&ash::ext::debug_utils::Instance> {
        self.debug_utils.as_ref()
    }
}

impl Drop for HgiVulkanInstance {
    /// Mirrors `HgiVulkanDestroyDebug()` + `vkDestroyInstance()`.
    fn drop(&mut self) {
        // SAFETY: We own both the messenger and the instance; no other code holds references
        // to them at drop time (this struct is not Clone/Copy).
        unsafe {
            if let (Some(messenger), Some(loader)) =
                (self.debug_messenger.take(), self.debug_utils.as_ref())
            {
                loader.destroy_debug_utils_messenger(messenger, None);
            }
            self.instance.destroy_instance(None);
        }
    }
}
