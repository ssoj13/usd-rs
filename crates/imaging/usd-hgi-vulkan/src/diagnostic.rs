//! Vulkan debug utilities — free functions.
//!
//! Port of pxr/imaging/hgiVulkan/diagnostic.cpp/.h

// All functions in this module require unsafe Vulkan FFI calls by necessity.
#![allow(unsafe_code)]

use std::ffi::CString;

use ash::vk;

// ---------------------------------------------------------------------------
// Environment checks
// ---------------------------------------------------------------------------

/// Returns true if `HGIVULKAN_DEBUG=1` is set in the environment.
pub fn is_debug_enabled() -> bool {
    static DEBUG: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *DEBUG.get_or_init(|| std::env::var("HGIVULKAN_DEBUG").as_deref() == Ok("1"))
}

/// Returns true if `HGIVULKAN_DEBUG_VERBOSE=1` is set in the environment.
pub fn is_verbose_debug_enabled() -> bool {
    static VERBOSE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *VERBOSE.get_or_init(|| std::env::var("HGIVULKAN_DEBUG_VERBOSE").as_deref() == Ok("1"))
}

// ---------------------------------------------------------------------------
// Debug messenger callback
// ---------------------------------------------------------------------------

/// Validation layer messages suppressed in non-verbose mode.
///
/// Mirrors the `ignoredMessages` list in the C++ `_VulkanDebugCallback`.
const IGNORED_MESSAGES: &[&str] = &[
    // Render passes like OIT/volume do not write to all pipeline attachments.
    "Validation Warning: [ Undefined-Value-ShaderInputNotProduced ]",
    // Shadow pass writes a shader output with no corresponding attachment.
    "Validation Warning: [ Undefined-Value-ShaderOutputNotConsumed ]",
];

/// Vulkan validation layer message callback — routes severity to the `log` crate.
///
/// Mirrors `_VulkanDebugCallback` from diagnostic.cpp.
pub unsafe extern "system" fn debug_callback(
    severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    _message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _user_data: *mut std::ffi::c_void,
) -> vk::Bool32 {
    let message = if callback_data.is_null() {
        "<null callback data>"
    } else {
        let p_message = unsafe { (*callback_data).p_message };
        if p_message.is_null() {
            "<null message>"
        } else {
            // SAFETY: Vulkan guarantees p_message is a valid null-terminated string.
            unsafe { std::ffi::CStr::from_ptr(p_message) }
                .to_str()
                .unwrap_or("<invalid utf-8>")
        }
    };

    // Suppress known-noisy messages when not in verbose mode.
    if !is_verbose_debug_enabled() {
        for ignored in IGNORED_MESSAGES {
            if message.starts_with(ignored) {
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

// ---------------------------------------------------------------------------
// Instance-level debug messenger lifecycle
// ---------------------------------------------------------------------------

/// Creates the `VkDebugUtilsMessengerEXT` for the given instance.
///
/// Takes `entry` and `instance` separately so the caller (e.g. `HgiVulkanInstance`)
/// can supply its already-loaded `ash::Entry` without a redundant loader call.
///
/// Returns `None` when debug is disabled.  The caller must pass the returned
/// pair to [`destroy_debug`] before destroying the instance.
///
/// Mirrors `HgiVulkanCreateDebug()` from diagnostic.cpp.
pub fn create_debug(
    entry: &ash::Entry,
    instance: &ash::Instance,
) -> Option<(ash::ext::debug_utils::Instance, vk::DebugUtilsMessengerEXT)> {
    if !is_debug_enabled() {
        return None;
    }

    let debug_utils = ash::ext::debug_utils::Instance::new(entry, instance);

    let mut severity = vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
        | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR;

    if is_verbose_debug_enabled() {
        severity |= vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE
            | vk::DebugUtilsMessageSeverityFlagsEXT::INFO;
    }

    let create_info = vk::DebugUtilsMessengerCreateInfoEXT::default()
        .message_severity(severity)
        .message_type(
            vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
        )
        .pfn_user_callback(Some(debug_callback));

    // SAFETY: create_info is fully initialised; no allocator (matches C++).
    let messenger = unsafe {
        debug_utils
            .create_debug_utils_messenger(&create_info, None)
            .map_err(|e| log::error!("vkCreateDebugUtilsMessengerEXT failed: {:?}", e))
            .ok()?
    };

    Some((debug_utils, messenger))
}

/// Destroys the debug messenger created by [`create_debug`].
///
/// Mirrors `HgiVulkanDestroyDebug()`.
pub fn destroy_debug(
    debug_utils: &ash::ext::debug_utils::Instance,
    messenger: vk::DebugUtilsMessengerEXT,
) {
    // SAFETY: messenger was created by this loader and has not been destroyed.
    unsafe { debug_utils.destroy_debug_utils_messenger(messenger, None) };
}

// ---------------------------------------------------------------------------
// Object naming
// ---------------------------------------------------------------------------

/// Attaches a human-readable debug name to any Vulkan object handle.
///
/// No-op when debug is disabled, `name` is empty, or the device extension
/// loader is unavailable.
///
/// Note: unlike the C++ `HgiVulkanSetDebugName(HgiVulkanDevice*, ...)` which
/// stored raw fn pointers on the device, `ash::ext::debug_utils::Device`
/// already holds the device handle internally, so no separate `ash::Device`
/// argument is needed.
///
/// Mirrors `HgiVulkanSetDebugName()`.
pub fn set_debug_name(
    debug_utils_device: Option<&ash::ext::debug_utils::Device>,
    object: u64,
    object_type: vk::ObjectType,
    name: &str,
) {
    if !is_debug_enabled() || name.is_empty() {
        return;
    }
    let Some(du) = debug_utils_device else { return };

    let Ok(cname) = CString::new(name) else {
        log::warn!(
            "set_debug_name: name contains interior nul byte: {:?}",
            name
        );
        return;
    };

    // Build the info struct directly: the typed builder encodes object_type
    // from the Handle trait, but we receive a raw u64 to match the C++ API
    // (uint64_t vulkanObject).  Set fields manually.
    let name_info = vk::DebugUtilsObjectNameInfoEXT {
        object_type,
        object_handle: object,
        p_object_name: cname.as_ptr(),
        ..Default::default()
    };

    // SAFETY: name_info and cname live past the call; device handle is baked
    // into the `du` loader at construction time.
    if let Err(e) = unsafe { du.set_debug_utils_object_name(&name_info) } {
        log::warn!("set_debug_name failed for {:?}: {:?}", name, e);
    }
}

// ---------------------------------------------------------------------------
// Command-buffer labels
// ---------------------------------------------------------------------------

/// Opens a named, coloured label region in a command buffer for GPU profilers.
///
/// Mirrors `HgiVulkanBeginLabel()`.
pub fn begin_label(
    debug_utils_device: Option<&ash::ext::debug_utils::Device>,
    command_buffer: vk::CommandBuffer,
    label: &str,
    color: [f32; 4],
) {
    if !is_debug_enabled() || label.is_empty() {
        return;
    }
    let Some(du) = debug_utils_device else { return };
    let Ok(clabel) = CString::new(label) else {
        return;
    };

    let label_info = vk::DebugUtilsLabelEXT::default()
        .label_name(&clabel)
        .color(color);

    // SAFETY: command_buffer is valid and in recording state.
    unsafe { du.cmd_begin_debug_utils_label(command_buffer, &label_info) };
}

/// Closes the most recently opened label region in a command buffer.
///
/// Mirrors `HgiVulkanEndLabel()`.
pub fn end_label(
    debug_utils_device: Option<&ash::ext::debug_utils::Device>,
    command_buffer: vk::CommandBuffer,
) {
    if !is_debug_enabled() {
        return;
    }
    let Some(du) = debug_utils_device else { return };

    // SAFETY: command_buffer is valid and has a matching begin_label.
    unsafe { du.cmd_end_debug_utils_label(command_buffer) };
}

/// Inserts a single-point debug marker into a command buffer.
///
/// Mirrors `HgiVulkanInsertDebugMarker()`.
pub fn insert_debug_marker(
    debug_utils_device: Option<&ash::ext::debug_utils::Device>,
    command_buffer: vk::CommandBuffer,
    label: &str,
    color: [f32; 4],
) {
    if !is_debug_enabled() || label.is_empty() {
        return;
    }
    let Some(du) = debug_utils_device else { return };
    let Ok(clabel) = CString::new(label) else {
        return;
    };

    let label_info = vk::DebugUtilsLabelEXT::default()
        .label_name(&clabel)
        .color(color);

    // SAFETY: command_buffer is valid and in recording state.
    unsafe { du.cmd_insert_debug_utils_label(command_buffer, &label_info) };
}

// ---------------------------------------------------------------------------
// Queue labels
// ---------------------------------------------------------------------------

/// Opens a named label region on a Vulkan queue.
///
/// Mirrors `HgiVulkanBeginQueueLabel()`.
pub fn begin_queue_label(
    debug_utils_device: Option<&ash::ext::debug_utils::Device>,
    queue: vk::Queue,
    label: &str,
) {
    if !is_debug_enabled() || label.is_empty() {
        return;
    }
    let Some(du) = debug_utils_device else { return };
    let Ok(clabel) = CString::new(label) else {
        return;
    };

    let label_info = vk::DebugUtilsLabelEXT::default()
        .label_name(&clabel)
        .color([0.0; 4]);

    // SAFETY: queue is a valid Vulkan queue handle.
    unsafe { du.queue_begin_debug_utils_label(queue, &label_info) };
}

/// Closes the most recently opened label region on a Vulkan queue.
///
/// Mirrors `HgiVulkanEndQueueLabel()`.
pub fn end_queue_label(
    debug_utils_device: Option<&ash::ext::debug_utils::Device>,
    queue: vk::Queue,
) {
    if !is_debug_enabled() {
        return;
    }
    let Some(du) = debug_utils_device else { return };

    // SAFETY: queue is valid and has a matching begin_queue_label.
    unsafe { du.queue_end_debug_utils_label(queue) };
}

// ---------------------------------------------------------------------------
// VkResult to string
// ---------------------------------------------------------------------------

/// Returns a human-readable static string for a `VkResult` value.
///
/// Covers all result codes defined by the Vulkan 1.3 core spec plus common
/// extensions.  Mirrors `HgiVulkanResultString()` / `string_VkResult()`.
pub fn vk_result_string(result: vk::Result) -> &'static str {
    match result {
        vk::Result::SUCCESS => "VK_SUCCESS",
        vk::Result::NOT_READY => "VK_NOT_READY",
        vk::Result::TIMEOUT => "VK_TIMEOUT",
        vk::Result::EVENT_SET => "VK_EVENT_SET",
        vk::Result::EVENT_RESET => "VK_EVENT_RESET",
        vk::Result::INCOMPLETE => "VK_INCOMPLETE",
        vk::Result::ERROR_OUT_OF_HOST_MEMORY => "VK_ERROR_OUT_OF_HOST_MEMORY",
        vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => "VK_ERROR_OUT_OF_DEVICE_MEMORY",
        vk::Result::ERROR_INITIALIZATION_FAILED => "VK_ERROR_INITIALIZATION_FAILED",
        vk::Result::ERROR_DEVICE_LOST => "VK_ERROR_DEVICE_LOST",
        vk::Result::ERROR_MEMORY_MAP_FAILED => "VK_ERROR_MEMORY_MAP_FAILED",
        vk::Result::ERROR_LAYER_NOT_PRESENT => "VK_ERROR_LAYER_NOT_PRESENT",
        vk::Result::ERROR_EXTENSION_NOT_PRESENT => "VK_ERROR_EXTENSION_NOT_PRESENT",
        vk::Result::ERROR_FEATURE_NOT_PRESENT => "VK_ERROR_FEATURE_NOT_PRESENT",
        vk::Result::ERROR_INCOMPATIBLE_DRIVER => "VK_ERROR_INCOMPATIBLE_DRIVER",
        vk::Result::ERROR_TOO_MANY_OBJECTS => "VK_ERROR_TOO_MANY_OBJECTS",
        vk::Result::ERROR_FORMAT_NOT_SUPPORTED => "VK_ERROR_FORMAT_NOT_SUPPORTED",
        vk::Result::ERROR_FRAGMENTED_POOL => "VK_ERROR_FRAGMENTED_POOL",
        vk::Result::ERROR_UNKNOWN => "VK_ERROR_UNKNOWN",
        vk::Result::ERROR_OUT_OF_POOL_MEMORY => "VK_ERROR_OUT_OF_POOL_MEMORY",
        vk::Result::ERROR_INVALID_EXTERNAL_HANDLE => "VK_ERROR_INVALID_EXTERNAL_HANDLE",
        vk::Result::ERROR_FRAGMENTATION => "VK_ERROR_FRAGMENTATION",
        vk::Result::ERROR_INVALID_OPAQUE_CAPTURE_ADDRESS => {
            "VK_ERROR_INVALID_OPAQUE_CAPTURE_ADDRESS"
        }
        vk::Result::PIPELINE_COMPILE_REQUIRED => "VK_PIPELINE_COMPILE_REQUIRED",
        vk::Result::ERROR_SURFACE_LOST_KHR => "VK_ERROR_SURFACE_LOST_KHR",
        vk::Result::ERROR_NATIVE_WINDOW_IN_USE_KHR => "VK_ERROR_NATIVE_WINDOW_IN_USE_KHR",
        vk::Result::SUBOPTIMAL_KHR => "VK_SUBOPTIMAL_KHR",
        vk::Result::ERROR_OUT_OF_DATE_KHR => "VK_ERROR_OUT_OF_DATE_KHR",
        vk::Result::ERROR_INCOMPATIBLE_DISPLAY_KHR => "VK_ERROR_INCOMPATIBLE_DISPLAY_KHR",
        vk::Result::ERROR_VALIDATION_FAILED_EXT => "VK_ERROR_VALIDATION_FAILED_EXT",
        vk::Result::ERROR_INVALID_SHADER_NV => "VK_ERROR_INVALID_SHADER_NV",
        vk::Result::ERROR_IMAGE_USAGE_NOT_SUPPORTED_KHR => "VK_ERROR_IMAGE_USAGE_NOT_SUPPORTED_KHR",
        vk::Result::ERROR_VIDEO_PICTURE_LAYOUT_NOT_SUPPORTED_KHR => {
            "VK_ERROR_VIDEO_PICTURE_LAYOUT_NOT_SUPPORTED_KHR"
        }
        vk::Result::ERROR_VIDEO_PROFILE_OPERATION_NOT_SUPPORTED_KHR => {
            "VK_ERROR_VIDEO_PROFILE_OPERATION_NOT_SUPPORTED_KHR"
        }
        vk::Result::ERROR_VIDEO_PROFILE_FORMAT_NOT_SUPPORTED_KHR => {
            "VK_ERROR_VIDEO_PROFILE_FORMAT_NOT_SUPPORTED_KHR"
        }
        vk::Result::ERROR_VIDEO_PROFILE_CODEC_NOT_SUPPORTED_KHR => {
            "VK_ERROR_VIDEO_PROFILE_CODEC_NOT_SUPPORTED_KHR"
        }
        vk::Result::ERROR_VIDEO_STD_VERSION_NOT_SUPPORTED_KHR => {
            "VK_ERROR_VIDEO_STD_VERSION_NOT_SUPPORTED_KHR"
        }
        vk::Result::ERROR_INVALID_DRM_FORMAT_MODIFIER_PLANE_LAYOUT_EXT => {
            "VK_ERROR_INVALID_DRM_FORMAT_MODIFIER_PLANE_LAYOUT_EXT"
        }
        vk::Result::ERROR_NOT_PERMITTED_KHR => "VK_ERROR_NOT_PERMITTED_KHR",
        vk::Result::ERROR_FULL_SCREEN_EXCLUSIVE_MODE_LOST_EXT => {
            "VK_ERROR_FULL_SCREEN_EXCLUSIVE_MODE_LOST_EXT"
        }
        vk::Result::THREAD_IDLE_KHR => "VK_THREAD_IDLE_KHR",
        vk::Result::THREAD_DONE_KHR => "VK_THREAD_DONE_KHR",
        vk::Result::OPERATION_DEFERRED_KHR => "VK_OPERATION_DEFERRED_KHR",
        vk::Result::OPERATION_NOT_DEFERRED_KHR => "VK_OPERATION_NOT_DEFERRED_KHR",
        vk::Result::ERROR_COMPRESSION_EXHAUSTED_EXT => "VK_ERROR_COMPRESSION_EXHAUSTED_EXT",
        _ => "VK_UNKNOWN_RESULT",
    }
}
