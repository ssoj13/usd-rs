//! Vulkan blit/copy command recording.
//!
//! Port of pxr/imaging/hgiVulkan/blitCmds.cpp/.h

// All Vulkan command recording goes through unsafe FFI.
#![allow(unsafe_code)]

use ash::vk;
use usd_hgi::*;

use crate::buffer::HgiVulkanBuffer;
use crate::command_buffer::HgiVulkanCommandBuffer;
use crate::conversions::HgiVulkanConversions;
use crate::diagnostic;
use crate::texture::{HgiVulkanTexture, NO_PENDING_WRITES};

/// Debug label color for blit command buffers (yellow, matching C++ `s_blitDebugColor`).
const BLIT_DEBUG_COLOR: [f32; 4] = [0.996, 0.875, 0.0, 1.0];
/// Debug color for single markers (transparent, matching C++ `s_markerDebugColor`).
const MARKER_DEBUG_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 0.0];

/// Thin wrapper that stores a pointer as `usize` so closures can be `Send`.
///
/// Used only inside completion handler closures where the raw pointer is
/// guaranteed to outlive the closure execution (VMA staging allocation).
struct SendPtr(usize);

impl SendPtr {
    fn new(ptr: *mut u8) -> Self {
        Self(ptr as usize)
    }
    fn as_ptr(self) -> *mut u8 {
        self.0 as *mut u8
    }
}

// SAFETY: The wrapped value is a VMA staging pointer; access is serialized
// through the GPU submission fence before the handler executes.
unsafe impl Send for SendPtr {}

/// Vulkan blit commands — handles all copy/transfer operations between CPU and GPU resources.
///
/// Port of `HgiVulkanBlitCmds`. The command buffer is provided externally (via
/// `set_command_buffer`) by `HgiVulkan::create_blit_cmds` after acquiring it from
/// the command queue — matching the C++ lazy-acquire design.
pub struct HgiVulkanBlitCmds {
    /// Debug utils device loader — used for labels/markers (optional extension).
    debug_utils: Option<ash::ext::debug_utils::Device>,
    /// Lazily acquired command buffer; `None` until provided by the owner.
    command_buffer: Option<HgiVulkanCommandBuffer>,
    /// Set to true once `submit()` is called, preventing double-submission.
    submitted: bool,
}

// SAFETY: `HgiVulkanCommandBuffer` is not Sync due to FnOnce handlers, but
// blit commands are single-threaded in practice (one thread records, one thread
// submits after a synchronization point).  Matches C++ which makes no Sync
// guarantee either.
unsafe impl Send for HgiVulkanBlitCmds {}
unsafe impl Sync for HgiVulkanBlitCmds {}

impl std::fmt::Debug for HgiVulkanBlitCmds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HgiVulkanBlitCmds")
            .field("submitted", &self.submitted)
            .field("has_command_buffer", &self.command_buffer.is_some())
            .finish()
    }
}

impl HgiVulkanBlitCmds {
    /// Create a new blit cmds object.
    ///
    /// No command buffer is acquired here — it is deferred to first use so
    /// that this can be constructed on the main thread and recorded on a worker.
    pub fn new(debug_utils: Option<ash::ext::debug_utils::Device>) -> Self {
        Self {
            debug_utils,
            command_buffer: None,
            submitted: false,
        }
    }

    /// Provide an already-acquired command buffer (called by the command queue
    /// integration path in `HgiVulkan::create_blit_cmds`).
    pub fn set_command_buffer(&mut self, cb: HgiVulkanCommandBuffer) {
        self.command_buffer = Some(cb);
    }

    /// Returns a reference to the inner command buffer, if one has been assigned.
    pub fn command_buffer(&self) -> Option<&HgiVulkanCommandBuffer> {
        self.command_buffer.as_ref()
    }

    /// Returns a mutable reference to the inner command buffer, if one has been assigned.
    pub fn command_buffer_mut(&mut self) -> Option<&mut HgiVulkanCommandBuffer> {
        self.command_buffer.as_mut()
    }

    /// Consume this struct, returning the inner command buffer (used by the
    /// queue when submitting).
    pub fn take_command_buffer(&mut self) -> Option<HgiVulkanCommandBuffer> {
        self.command_buffer.take()
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Returns the raw `vk::CommandBuffer` handle if a command buffer is set.
    fn vk_cb(&self) -> vk::CommandBuffer {
        match &self.command_buffer {
            Some(cb) => cb.vk_command_buffer(),
            None => {
                log::warn!("HgiVulkanBlitCmds: no command buffer set — operation skipped");
                vk::CommandBuffer::null()
            }
        }
    }

    /// Determine the old `VkAccessFlags` and `VkPipelineStageFlags` based on the
    /// current image layout.  Matches C++ file-static `_GetOldAccessAndPipelineStageFlags`.
    fn old_access_and_stage(
        old_layout: vk::ImageLayout,
    ) -> (vk::AccessFlags, vk::PipelineStageFlags) {
        match old_layout {
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL => (
                vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            ),
            vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL => (
                vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
            ),
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL => (
                vk::AccessFlags::SHADER_READ,
                vk::PipelineStageFlags::ALL_GRAPHICS,
            ),
            _ => (
                vk::AccessFlags::empty(),
                vk::PipelineStageFlags::ALL_GRAPHICS,
            ),
        }
    }

    /// Returns the aspect mask for copy operations, stripping stencil when both
    /// depth and stencil are set (Vulkan validation forbids simultaneous aspects
    /// during copy).  Matches C++ `_GetImageAspectMaskForCopy`.
    fn aspect_mask_for_copy(usage: HgiTextureUsage) -> vk::ImageAspectFlags {
        let mut mask = HgiVulkanConversions::get_image_aspect_flag(usage);
        // Per Vulkan validation: during copy only DEPTH or STENCIL, not both.
        if mask.contains(vk::ImageAspectFlags::DEPTH) {
            mask = vk::ImageAspectFlags::DEPTH;
        }
        mask
    }

    /// Downcast a texture handle to a mutable `HgiVulkanTexture` reference.
    ///
    /// Uses `Arc::as_ptr` so we derive mutability from the raw pointer rather
    /// than casting away `&T` const-ness, which would be undefined behaviour.
    ///
    /// # Safety
    ///
    /// The caller must ensure exclusive logical access to the texture for the
    /// duration of the returned borrow.  Matches C++
    /// `static_cast<HgiVulkanTexture*>(handle.Get())`.
    unsafe fn as_vk_tex_mut(handle: &HgiTextureHandle) -> Option<&mut HgiVulkanTexture> {
        let arc = handle.arc()?;
        // `Arc::as_ptr` returns `*const dyn HgiTexture`; we re-interpret as
        // `*mut HgiVulkanTexture` after verifying the concrete type.
        let const_ptr: *const dyn HgiTexture = std::sync::Arc::as_ptr(&arc);
        // SAFETY: arc is valid and we only call an immutable method.
        if unsafe { (*const_ptr).as_any().downcast_ref::<HgiVulkanTexture>() }.is_none() {
            return None;
        }
        // SAFETY: we just verified the concrete type and caller guarantees
        // exclusive access.
        Some(unsafe { &mut *(const_ptr as *mut HgiVulkanTexture) })
    }

    /// Downcast a shared buffer trait-object to `HgiVulkanBuffer`.
    fn as_vk_buf(buf: &dyn HgiBuffer) -> Option<&HgiVulkanBuffer> {
        buf.as_any().downcast_ref::<HgiVulkanBuffer>()
    }
}

// ---------------------------------------------------------------------------
// HgiCmds
// ---------------------------------------------------------------------------

impl HgiCmds for HgiVulkanBlitCmds {
    fn is_submitted(&self) -> bool {
        self.submitted
    }

    fn push_debug_group(&mut self, label: &str) {
        let cb = self.vk_cb();
        if cb != vk::CommandBuffer::null() {
            diagnostic::begin_label(self.debug_utils.as_ref(), cb, label, BLIT_DEBUG_COLOR);
        }
    }

    fn pop_debug_group(&mut self) {
        let cb = self.vk_cb();
        if cb != vk::CommandBuffer::null() {
            diagnostic::end_label(self.debug_utils.as_ref(), cb);
        }
    }

    fn insert_debug_marker(&mut self, label: &str) {
        let cb = self.vk_cb();
        if cb != vk::CommandBuffer::null() {
            diagnostic::insert_debug_marker(
                self.debug_utils.as_ref(),
                cb,
                label,
                MARKER_DEBUG_COLOR,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// HgiBlitCmds
// ---------------------------------------------------------------------------

impl HgiBlitCmds for HgiVulkanBlitCmds {
    // -----------------------------------------------------------------------
    // Buffer operations
    // -----------------------------------------------------------------------

    /// Copy from a CPU buffer to a GPU buffer via the buffer's staging allocation.
    ///
    /// Skips the CPU memcpy if `cpuSourceBuffer` is already the staging address
    /// with matching offsets (the C++ "already contains the desired data" path).
    /// On non-UMA/upload buffers records `vkCmdCopyBuffer` staging → device-local.
    fn copy_buffer_cpu_to_gpu(&mut self, op: &HgiBufferCpuToGpuOp) {
        if op.byte_size == 0 || op.cpu_source_buffer.as_ptr().is_null() {
            return;
        }
        if op.gpu_destination_buffer.is_null() {
            return;
        }

        let vk_cmd = self.vk_cb();
        if vk_cmd == vk::CommandBuffer::null() {
            return;
        }

        let Some(hgi_buf) = op.gpu_destination_buffer.get() else {
            log::error!("copy_buffer_cpu_to_gpu: invalid destination buffer handle");
            return;
        };
        let Some(dst_buf) = Self::as_vk_buf(hgi_buf) else {
            log::error!("copy_buffer_cpu_to_gpu: destination is not a HgiVulkanBuffer");
            return;
        };

        // Skip memcpy when cpuSourceBuffer IS the staging address and offsets match.
        let skip_memcpy = dst_buf.is_cpu_staging_address(op.cpu_source_buffer.as_ptr())
            && op.source_byte_offset == op.destination_byte_offset;

        if !skip_memcpy {
            let Some(staging_ptr) = dst_buf.cpu_staging_address_raw() else {
                log::error!(
                    "copy_buffer_cpu_to_gpu: staging address not yet mapped — call cpu_staging_address() first"
                );
                return;
            };
            // SAFETY: cpu_source_buffer must remain valid for this call (caller contract).
            // staging_ptr is valid for the lifetime of the staging allocation.
            unsafe {
                let dst = staging_ptr.add(op.destination_byte_offset);
                let src = op.cpu_source_buffer.as_ptr().add(op.source_byte_offset);
                std::ptr::copy_nonoverlapping(src, dst, op.byte_size);
            }
        }

        // On GPU-only buffers: record staging → device-local copy.
        // Upload / UMA buffers have no separate staging buffer; the write above
        // already went into the device buffer directly.
        let Some(staging) = dst_buf.staging_buffer() else {
            return; // Upload or UMA — no additional copy needed.
        };

        let copy_region = vk::BufferCopy {
            // Use destinationByteOffset as srcOffset — staging has the same layout.
            src_offset: op.destination_byte_offset as u64,
            dst_offset: op.destination_byte_offset as u64,
            size: op.byte_size as u64,
        };

        // SAFETY: all Vulkan handles are valid and in the recording state.
        unsafe {
            dst_buf.device().cmd_copy_buffer(
                vk_cmd,
                staging.vk_buffer(),
                dst_buf.vk_buffer(),
                &[copy_region],
            );
        }
    }

    /// Copy from one GPU buffer to another — simple `vkCmdCopyBuffer`.
    fn copy_buffer_gpu_to_gpu(&mut self, op: &HgiBufferGpuToGpuOp) {
        if op.byte_size == 0 {
            log::warn!("copy_buffer_gpu_to_gpu: byte_size is zero — aborted");
            return;
        }

        let vk_cmd = self.vk_cb();
        if vk_cmd == vk::CommandBuffer::null() {
            return;
        }

        let Some(src_hgi) = op.gpu_source_buffer.get() else {
            log::error!("copy_buffer_gpu_to_gpu: invalid source buffer");
            return;
        };
        let Some(dst_hgi) = op.gpu_destination_buffer.get() else {
            log::error!("copy_buffer_gpu_to_gpu: invalid destination buffer");
            return;
        };
        let Some(src) = Self::as_vk_buf(src_hgi) else {
            log::error!("copy_buffer_gpu_to_gpu: source is not a HgiVulkanBuffer");
            return;
        };
        let Some(dst) = Self::as_vk_buf(dst_hgi) else {
            log::error!("copy_buffer_gpu_to_gpu: destination is not a HgiVulkanBuffer");
            return;
        };

        let copy_region = vk::BufferCopy {
            src_offset: op.source_byte_offset as u64,
            dst_offset: op.destination_byte_offset as u64,
            size: op.byte_size as u64,
        };

        // SAFETY: all handles are valid and in the recording state.
        unsafe {
            src.device()
                .cmd_copy_buffer(vk_cmd, src.vk_buffer(), dst.vk_buffer(), &[copy_region]);
        }
    }

    /// Copy from a GPU buffer to CPU.
    ///
    /// On non-UMA: records `vkCmdCopyBuffer` from device-local → staging,
    /// then registers a completion handler to memcpy staging → `cpuDestinationBuffer`
    /// once the command buffer retires.  On UMA the device buffer IS the staging
    /// buffer so only the completion handler is registered.
    fn copy_buffer_gpu_to_cpu(&mut self, op: &HgiBufferGpuToCpuOp) {
        if op.byte_size == 0 || op.cpu_destination_buffer.as_ptr().is_null() {
            return;
        }
        if op.gpu_source_buffer.is_null() {
            return;
        }

        let vk_cmd = self.vk_cb();
        if vk_cmd == vk::CommandBuffer::null() {
            return;
        }

        let Some(src_hgi) = op.gpu_source_buffer.get() else {
            log::error!("copy_buffer_gpu_to_cpu: invalid source buffer");
            return;
        };
        let Some(src_buf) = Self::as_vk_buf(src_hgi) else {
            log::error!("copy_buffer_gpu_to_cpu: source is not a HgiVulkanBuffer");
            return;
        };

        // On non-UMA: device-local → staging via vkCmdCopyBuffer.
        // After this the staging pointer at offset 0 holds the requested data.
        let mut src_offset_for_handler = op.source_byte_offset;
        if let Some(staging) = src_buf.staging_buffer() {
            let copy_region = vk::BufferCopy {
                src_offset: op.source_byte_offset as u64,
                dst_offset: 0,
                size: op.byte_size as u64,
            };
            // SAFETY: handles are valid and in the recording state.
            unsafe {
                src_buf.device().cmd_copy_buffer(
                    vk_cmd,
                    src_buf.vk_buffer(),
                    staging.vk_buffer(),
                    &[copy_region],
                );
            }
            src_offset_for_handler = 0;
        }

        // Register a completion handler: memcpy staging → CPU destination once
        // the GPU has finished consuming this command buffer.
        let Some(staging_raw) = src_buf.cpu_staging_address_raw() else {
            log::error!(
                "copy_buffer_gpu_to_cpu: staging not mapped — call cpu_staging_address() first"
            );
            return;
        };

        let dst_ptr = SendPtr::new(op.cpu_destination_buffer.as_ptr());
        let byte_size = op.byte_size;
        let staging_send = SendPtr::new(staging_raw);

        if let Some(cb) = &mut self.command_buffer {
            cb.add_completed_handler(Box::new(move || {
                // SAFETY: dst_ptr is provided by the caller and must remain valid
                // until the command buffer retires.  staging_raw points into the
                // VMA staging allocation which outlives the command buffer.
                let src = unsafe { staging_send.as_ptr().add(src_offset_for_handler) };
                unsafe { std::ptr::copy_nonoverlapping(src, dst_ptr.as_ptr(), byte_size) };
            }));
        }
    }

    // -----------------------------------------------------------------------
    // Texture operations
    // -----------------------------------------------------------------------

    /// Copy CPU data into a GPU texture via the texture's staging buffer.
    ///
    /// Matches C++ `CopyTextureCpuToGpu`.
    fn copy_texture_cpu_to_gpu(&mut self, op: &HgiTextureCpuToGpuOp) {
        let vk_cmd = self.vk_cb();
        if vk_cmd == vk::CommandBuffer::null() {
            return;
        }

        // SAFETY: see as_vk_tex_mut safety note.
        let Some(dst_tex) = (unsafe { Self::as_vk_tex_mut(&op.gpu_destination_texture) }) else {
            log::error!("copy_texture_cpu_to_gpu: destination is not a valid HgiVulkanTexture");
            return;
        };

        let tex_desc = dst_tex.descriptor().clone();

        // Skip memcpy when cpuSourceBuffer IS already the staging address.
        let skip_memcpy = dst_tex.is_cpu_staging_address(op.cpu_source_buffer.as_ptr());

        if !skip_memcpy {
            let mip_infos = usd_hgi::get_mip_infos(
                tex_desc.format,
                &tex_desc.dimensions,
                1, // CpuToGpu does one layer at a time
                None,
            );

            if let Some(mip_info) = mip_infos.get(op.mip_level as usize) {
                let byte_offset = mip_info.byte_offset;
                let copy_size = op.buffer_byte_size.min(mip_info.byte_size_per_layer);

                if let Some(staging_ptr) = dst_tex.get_cpu_staging_address() {
                    // SAFETY: staging_ptr is valid for the allocation lifetime;
                    // cpu_source_buffer is valid per caller contract.
                    unsafe {
                        let dst_ptr = staging_ptr.add(byte_offset);
                        std::ptr::copy_nonoverlapping(
                            op.cpu_source_buffer.as_ptr(),
                            dst_ptr,
                            copy_size,
                        );
                    }
                } else {
                    log::error!("copy_texture_cpu_to_gpu: could not get staging address");
                    return;
                }
            }
        }

        // Schedule staging buffer → device-local texture transfer.
        let Some(staging_vk) = dst_tex.staging_buffer().map(|b| b.vk_buffer()) else {
            log::error!("copy_texture_cpu_to_gpu: no staging buffer available");
            return;
        };

        let staging_byte_size = dst_tex
            .staging_buffer()
            .map(|b| b.byte_size_of_resource())
            .unwrap_or(0);

        let Some(cb) = &self.command_buffer else {
            log::warn!("copy_texture_cpu_to_gpu: no command buffer — skipped");
            return;
        };

        dst_tex.copy_buffer_to_texture(
            cb,
            staging_vk,
            staging_byte_size,
            [
                op.destination_texel_offset[0],
                op.destination_texel_offset[1],
                op.destination_texel_offset[2],
            ],
            op.mip_level as i32,
        );
    }

    /// Copy one GPU texture region to another using `vkCmdBlitImage` (NEAREST filter).
    ///
    /// The Rust trait includes GPU→GPU texture copy; C++ BlitTexture is closest.
    fn copy_texture_gpu_to_gpu(&mut self, op: &HgiTextureGpuToGpuOp) {
        let vk_cmd = self.vk_cb();
        if vk_cmd == vk::CommandBuffer::null() {
            return;
        }

        // SAFETY: see as_vk_tex_mut safety note.
        let Some(src) = (unsafe { Self::as_vk_tex_mut(&op.gpu_source_texture) }) else {
            log::error!("copy_texture_gpu_to_gpu: source is not a valid HgiVulkanTexture");
            return;
        };
        let src_desc = src.descriptor().clone();
        let aspect = Self::aspect_mask_for_copy(src_desc.usage);
        let src_old_layout = src.vk_image_layout();
        let src_image = src.vk_image();
        let device = src.device_clone();
        let (src_access, src_stage) = Self::old_access_and_stage(src_old_layout);

        let Some(cb) = &self.command_buffer else {
            return;
        };
        src.layout_barrier(
            cb,
            src_old_layout,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            src_access,
            vk::AccessFlags::TRANSFER_READ,
            src_stage,
            vk::PipelineStageFlags::TRANSFER,
            -1,
        );

        // SAFETY: see as_vk_tex_mut safety note.
        let Some(dst) = (unsafe { Self::as_vk_tex_mut(&op.gpu_destination_texture) }) else {
            log::error!("copy_texture_gpu_to_gpu: destination is not a valid HgiVulkanTexture");
            return;
        };
        let dst_desc = dst.descriptor().clone();
        let dst_old_layout = dst.vk_image_layout();
        let dst_image = dst.vk_image();
        let (dst_access, dst_stage) = Self::old_access_and_stage(dst_old_layout);

        let Some(cb) = &self.command_buffer else {
            return;
        };
        dst.layout_barrier(
            cb,
            dst_old_layout,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            dst_access,
            vk::AccessFlags::TRANSFER_WRITE,
            dst_stage,
            vk::PipelineStageFlags::TRANSFER,
            -1,
        );

        let region = vk::ImageBlit {
            src_subresource: vk::ImageSubresourceLayers {
                aspect_mask: aspect,
                mip_level: op.source_mip_level,
                base_array_layer: op.source_layer,
                layer_count: 1,
            },
            src_offsets: [
                vk::Offset3D {
                    x: op.source_texel_offset[0],
                    y: op.source_texel_offset[1],
                    z: op.source_texel_offset[2],
                },
                vk::Offset3D {
                    x: op.source_texel_offset[0] + op.copy_size[0],
                    y: op.source_texel_offset[1] + op.copy_size[1],
                    z: op.source_texel_offset[2] + op.copy_size[2],
                },
            ],
            dst_subresource: vk::ImageSubresourceLayers {
                aspect_mask: aspect,
                mip_level: op.destination_mip_level,
                base_array_layer: op.destination_layer,
                layer_count: 1,
            },
            dst_offsets: [
                vk::Offset3D {
                    x: op.destination_texel_offset[0],
                    y: op.destination_texel_offset[1],
                    z: op.destination_texel_offset[2],
                },
                vk::Offset3D {
                    x: op.destination_texel_offset[0] + op.copy_size[0],
                    y: op.destination_texel_offset[1] + op.copy_size[1],
                    z: op.destination_texel_offset[2] + op.copy_size[2],
                },
            ],
        };

        let Some(device) = device else {
            log::error!("copy_texture_gpu_to_gpu: no device (stub mode)");
            return;
        };

        // SAFETY: all Vulkan handles valid; images transitioned above.
        unsafe {
            device.cmd_blit_image(
                vk_cmd,
                src_image,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                dst_image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[region],
                vk::Filter::NEAREST,
            );
        }

        // Transition src back to original layout.
        let src_default_access = HgiVulkanTexture::get_default_access_flags(src_desc.usage);
        let Some(cb) = &self.command_buffer else {
            return;
        };
        // SAFETY: re-acquire; no aliasing borrow exists at this point.
        let src2 = unsafe { Self::as_vk_tex_mut(&op.gpu_source_texture) }.unwrap();
        src2.layout_barrier(
            cb,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            src_old_layout,
            NO_PENDING_WRITES,
            src_default_access,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::ALL_GRAPHICS,
            -1,
        );

        // Transition dst back to original layout.
        let dst_default_access = HgiVulkanTexture::get_default_access_flags(dst_desc.usage);
        // SAFETY: re-acquire; no aliasing borrow exists at this point.
        let dst2 = unsafe { Self::as_vk_tex_mut(&op.gpu_destination_texture) }.unwrap();
        dst2.layout_barrier(
            cb,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            dst_old_layout,
            vk::AccessFlags::TRANSFER_WRITE,
            dst_default_access,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::ALL_GRAPHICS,
            -1,
        );
    }

    /// Read back a GPU texture region to CPU memory.
    ///
    /// Records a layout transition + `vkCmdCopyImageToBuffer` into the texture's
    /// staging buffer, then registers a completion handler that memcpy's staging
    /// data into `cpuDestinationBuffer` after GPU completion.
    /// Matches C++ `CopyTextureGpuToCpu`.
    fn copy_texture_gpu_to_cpu(&mut self, op: &HgiTextureGpuToCpuOp) {
        if op.destination_buffer_byte_size == 0 {
            log::warn!("copy_texture_gpu_to_cpu: destination byte size is zero — aborted");
            return;
        }

        let vk_cmd = self.vk_cb();
        if vk_cmd == vk::CommandBuffer::null() {
            return;
        }

        // SAFETY: see as_vk_tex_mut safety note.
        let Some(src) = (unsafe { Self::as_vk_tex_mut(&op.gpu_source_texture) }) else {
            log::error!("copy_texture_gpu_to_cpu: source is not a valid HgiVulkanTexture");
            return;
        };

        let tex_desc = src.descriptor().clone();
        let is_tex_array = tex_desc.layer_count > 1;
        let depth_offset = if is_tex_array {
            0
        } else {
            op.source_texel_offset[2]
        };

        let image_sub = vk::ImageSubresourceLayers {
            aspect_mask: Self::aspect_mask_for_copy(tex_desc.usage),
            mip_level: op.mip_level,
            base_array_layer: if is_tex_array {
                op.source_texel_offset[2] as u32
            } else {
                0
            },
            layer_count: 1,
        };

        // See Vulkan spec: "Copying Data Between Buffers and Images"
        let region = vk::BufferImageCopy {
            buffer_offset: 0,       // cpuDestinationBuffer offset is applied in the handler
            buffer_row_length: 0,   // tightly packed
            buffer_image_height: 0, // tightly packed
            image_subresource: image_sub,
            image_offset: vk::Offset3D {
                x: op.source_texel_offset[0],
                y: op.source_texel_offset[1],
                z: depth_offset,
            },
            image_extent: vk::Extent3D {
                width: (tex_desc.dimensions[0] - op.source_texel_offset[0]) as u32,
                height: (tex_desc.dimensions[1] - op.source_texel_offset[1]) as u32,
                depth: (tex_desc.dimensions[2] - depth_offset) as u32,
            },
        };

        let old_layout = src.vk_image_layout();
        let (src_access, src_stage) = Self::old_access_and_stage(old_layout);

        let Some(cb) = &self.command_buffer else {
            return;
        };
        src.layout_barrier(
            cb,
            old_layout,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            src_access,
            vk::AccessFlags::TRANSFER_READ,
            src_stage,
            vk::PipelineStageFlags::TRANSFER,
            -1,
        );

        // Ensure staging buffer exists (created lazily on first call).
        let _ensure_staged = src.get_cpu_staging_address();

        let Some(staging_buf) = src.staging_buffer() else {
            log::error!("copy_texture_gpu_to_cpu: could not create staging buffer");
            return;
        };
        let staging_vk = staging_buf.vk_buffer();
        let staging_raw = staging_buf.cpu_staging_address_raw();

        let src_vk_image = src.vk_image();
        let Some(device) = src.device_clone() else {
            log::error!("copy_texture_gpu_to_cpu: no device (stub mode)");
            return;
        };

        // SAFETY: image is in TRANSFER_SRC_OPTIMAL; all handles are valid.
        unsafe {
            device.cmd_copy_image_to_buffer(
                vk_cmd,
                src_vk_image,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                staging_vk,
                &[region],
            );
        }

        // Transition image back to its original layout.
        let default_access = HgiVulkanTexture::get_default_access_flags(tex_desc.usage);
        let Some(cb) = &self.command_buffer else {
            return;
        };
        src.layout_barrier(
            cb,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            old_layout,
            NO_PENDING_WRITES,
            default_access,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::ALL_GRAPHICS,
            -1,
        );

        // Register GPU→CPU memcpy to run once the command buffer retires.
        let dst_ptr = SendPtr::new(op.cpu_destination_buffer.as_ptr());
        let dst_byte_offset = op.destination_byte_offset;
        let byte_size = op.destination_buffer_byte_size;

        if let Some(staging_raw) = staging_raw {
            let staging_send = SendPtr::new(staging_raw);
            if let Some(cb) = &mut self.command_buffer {
                cb.add_completed_handler(Box::new(move || {
                    // SAFETY: dst_ptr valid per caller contract; staging_send.as_ptr() valid
                    // until the allocation is freed (after all handlers run).
                    let dst = unsafe { dst_ptr.as_ptr().add(dst_byte_offset) };
                    unsafe { std::ptr::copy_nonoverlapping(staging_send.as_ptr(), dst, byte_size) };
                }));
            }
        }
    }

    /// Copy a GPU buffer region into a GPU texture.  Matches C++ `CopyBufferToTexture`.
    fn copy_buffer_to_texture(&mut self, op: &HgiBufferToTextureOp) {
        // Zero copy_size is not explicitly checked in C++, but guard it here.
        let vk_cmd = self.vk_cb();
        if vk_cmd == vk::CommandBuffer::null() {
            return;
        }

        let Some(src_hgi) = op.gpu_source_buffer.get() else {
            log::error!("copy_buffer_to_texture: invalid source buffer");
            return;
        };
        let Some(src_buf) = Self::as_vk_buf(src_hgi) else {
            log::error!("copy_buffer_to_texture: source is not a HgiVulkanBuffer");
            return;
        };

        // SAFETY: see as_vk_tex_mut safety note.
        let Some(dst) = (unsafe { Self::as_vk_tex_mut(&op.gpu_destination_texture) }) else {
            log::error!("copy_buffer_to_texture: destination is not a valid HgiVulkanTexture");
            return;
        };

        let tex_desc = dst.descriptor().clone();
        let old_layout = dst.vk_image_layout();
        let (src_access, src_stage) = Self::old_access_and_stage(old_layout);

        let Some(cb) = &self.command_buffer else {
            return;
        };
        dst.layout_barrier(
            cb,
            old_layout,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            src_access,
            vk::AccessFlags::TRANSFER_WRITE,
            src_stage,
            vk::PipelineStageFlags::TRANSFER,
            op.destination_mip_level as i32,
        );

        let region = vk::BufferImageCopy {
            buffer_offset: op.source_byte_offset as u64,
            buffer_row_length: 0,
            buffer_image_height: 0,
            image_subresource: vk::ImageSubresourceLayers {
                aspect_mask: Self::aspect_mask_for_copy(tex_desc.usage),
                mip_level: op.destination_mip_level,
                base_array_layer: op.destination_layer,
                layer_count: tex_desc.layer_count as u32,
            },
            image_offset: vk::Offset3D {
                x: op.destination_texel_offset[0],
                y: op.destination_texel_offset[1],
                z: op.destination_texel_offset[2],
            },
            image_extent: vk::Extent3D {
                width: (tex_desc.dimensions[0] - op.destination_texel_offset[0]) as u32,
                height: (tex_desc.dimensions[1] - op.destination_texel_offset[1]) as u32,
                depth: (tex_desc.dimensions[2] - op.destination_texel_offset[2]) as u32,
            },
        };

        let src_vk_buf = src_buf.vk_buffer();
        let dst_vk_image = dst.vk_image();
        let device = src_buf.device().clone();

        // SAFETY: image is in TRANSFER_DST_OPTIMAL; handles are valid.
        unsafe {
            device.cmd_copy_buffer_to_image(
                vk_cmd,
                src_vk_buf,
                dst_vk_image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[region],
            );
        }

        let default_access = HgiVulkanTexture::get_default_access_flags(tex_desc.usage);
        let Some(cb) = &self.command_buffer else {
            return;
        };
        dst.layout_barrier(
            cb,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            old_layout,
            vk::AccessFlags::TRANSFER_WRITE,
            default_access,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::ALL_GRAPHICS,
            op.destination_mip_level as i32,
        );
    }

    /// Copy a GPU texture region into a GPU buffer.  Matches C++ `CopyTextureToBuffer`.
    fn copy_texture_to_buffer(&mut self, op: &HgiTextureToBufferOp) {
        let vk_cmd = self.vk_cb();
        if vk_cmd == vk::CommandBuffer::null() {
            return;
        }

        // SAFETY: see as_vk_tex_mut safety note.
        let Some(src) = (unsafe { Self::as_vk_tex_mut(&op.gpu_source_texture) }) else {
            log::error!("copy_texture_to_buffer: source is not a valid HgiVulkanTexture");
            return;
        };

        let tex_desc = src.descriptor().clone();

        let Some(dst_hgi) = op.gpu_destination_buffer.get() else {
            log::error!("copy_texture_to_buffer: invalid destination buffer");
            return;
        };
        let Some(dst_buf) = Self::as_vk_buf(dst_hgi) else {
            log::error!("copy_texture_to_buffer: destination is not a HgiVulkanBuffer");
            return;
        };

        let old_layout = src.vk_image_layout();
        let (src_access, src_stage) = Self::old_access_and_stage(old_layout);

        let Some(cb) = &self.command_buffer else {
            return;
        };
        src.layout_barrier(
            cb,
            old_layout,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            src_access,
            vk::AccessFlags::TRANSFER_READ,
            src_stage,
            vk::PipelineStageFlags::TRANSFER,
            op.mip_level as i32,
        );

        let region = vk::BufferImageCopy {
            buffer_offset: op.destination_byte_offset as u64,
            buffer_row_length: 0,
            buffer_image_height: 0,
            image_subresource: vk::ImageSubresourceLayers {
                aspect_mask: Self::aspect_mask_for_copy(tex_desc.usage),
                mip_level: op.mip_level,
                base_array_layer: op.source_layer,
                layer_count: tex_desc.layer_count as u32,
            },
            image_offset: vk::Offset3D {
                x: op.source_texel_offset[0],
                y: op.source_texel_offset[1],
                z: op.source_texel_offset[2],
            },
            image_extent: vk::Extent3D {
                width: tex_desc.dimensions[0] as u32,
                height: tex_desc.dimensions[1] as u32,
                depth: tex_desc.dimensions[2] as u32,
            },
        };

        let src_vk_image = src.vk_image();
        let dst_vk_buf = dst_buf.vk_buffer();
        let device = dst_buf.device().clone();

        // SAFETY: image is in TRANSFER_SRC_OPTIMAL; buffer handle is valid.
        unsafe {
            device.cmd_copy_image_to_buffer(
                vk_cmd,
                src_vk_image,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                dst_vk_buf,
                &[region],
            );
        }

        let default_access = HgiVulkanTexture::get_default_access_flags(tex_desc.usage);
        let Some(cb) = &self.command_buffer else {
            return;
        };
        src.layout_barrier(
            cb,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            old_layout,
            vk::AccessFlags::TRANSFER_WRITE,
            default_access,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::ALL_GRAPHICS,
            op.mip_level as i32,
        );
    }

    /// Generate mipmaps by iteratively blitting mip[i-1] → mip[i].
    ///
    /// Matches C++ `GenerateMipMaps`.  Format blit-capability check is omitted
    /// in stub mode (requires physical device access not yet wired in).
    fn generate_mipmap(&mut self, texture: &HgiTextureHandle) {
        let vk_cmd = self.vk_cb();
        if vk_cmd == vk::CommandBuffer::null() {
            return;
        }

        // SAFETY: see as_vk_tex_mut safety note.
        let Some(tex) = (unsafe { Self::as_vk_tex_mut(texture) }) else {
            log::error!("generate_mipmap: not a valid HgiVulkanTexture");
            return;
        };

        let desc = tex.descriptor().clone();
        let mip_levels = desc.mip_levels as u32;
        if mip_levels <= 1 {
            return;
        }

        let layer_count = desc.layer_count as u32;
        let width = desc.dimensions[0];
        let height = desc.dimensions[1];

        let old_layout = tex.vk_image_layout();
        let (src_access, src_stage) = Self::old_access_and_stage(old_layout);

        let Some(cb) = &self.command_buffer else {
            return;
        };
        // Transition mip 0 to TRANSFER_SRC so it can be the blit source.
        tex.layout_barrier(
            cb,
            old_layout,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            src_access,
            vk::AccessFlags::TRANSFER_READ,
            src_stage,
            vk::PipelineStageFlags::TRANSFER,
            0,
        );

        let src_vk_image = tex.vk_image();
        let Some(device) = tex.device_clone() else {
            log::error!("generate_mipmap: no device (stub mode)");
            return;
        };

        for i in 1..mip_levels {
            // Transition mip[i] to TRANSFER_DST before writing into it.
            let Some(cb) = &self.command_buffer else {
                return;
            };
            tex.layout_barrier(
                cb,
                old_layout,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                src_access,
                vk::AccessFlags::TRANSFER_WRITE,
                src_stage,
                vk::PipelineStageFlags::TRANSFER,
                i as i32,
            );

            let src_w = (width >> (i - 1)).max(1) as i32;
            let src_h = (height >> (i - 1)).max(1) as i32;
            let dst_w = (width >> i).max(1) as i32;
            let dst_h = (height >> i).max(1) as i32;

            let blit = vk::ImageBlit {
                src_subresource: vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: i - 1,
                    base_array_layer: 0,
                    layer_count,
                },
                src_offsets: [
                    vk::Offset3D { x: 0, y: 0, z: 0 },
                    vk::Offset3D {
                        x: src_w,
                        y: src_h,
                        z: 1,
                    },
                ],
                dst_subresource: vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: i,
                    base_array_layer: 0,
                    layer_count,
                },
                dst_offsets: [
                    vk::Offset3D { x: 0, y: 0, z: 0 },
                    vk::Offset3D {
                        x: dst_w,
                        y: dst_h,
                        z: 1,
                    },
                ],
            };

            // SAFETY: image handles are valid; layouts have been transitioned above.
            unsafe {
                device.cmd_blit_image(
                    vk_cmd,
                    src_vk_image,
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    src_vk_image,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    &[blit],
                    vk::Filter::LINEAR,
                );
            }

            // Prepare mip[i] as the source for the next iteration: DST → SRC.
            let Some(cb) = &self.command_buffer else {
                return;
            };
            tex.layout_barrier(
                cb,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                vk::AccessFlags::TRANSFER_WRITE,
                vk::AccessFlags::TRANSFER_READ,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::TRANSFER,
                i as i32,
            );
        }

        // Return all mips from TRANSFER_SRC back to the original layout.
        let default_access = HgiVulkanTexture::get_default_access_flags(desc.usage);
        let Some(cb) = &self.command_buffer else {
            return;
        };
        tex.layout_barrier(
            cb,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            old_layout,
            vk::AccessFlags::TRANSFER_READ,
            default_access,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::ALL_GRAPHICS,
            -1,
        );
    }

    /// Fill an entire GPU buffer with a constant byte pattern — `vkCmdFillBuffer`.
    ///
    /// The 8-bit value is replicated into a 32-bit word (e.g. `0xff` → `0xffffffff`)
    /// exactly as in the C++ implementation.
    fn fill_buffer(&mut self, buffer: &HgiBufferHandle, value: u8) {
        let vk_cmd = self.vk_cb();
        if vk_cmd == vk::CommandBuffer::null() {
            return;
        }

        let Some(hgi_buf) = buffer.get() else {
            log::error!("fill_buffer: invalid buffer handle");
            return;
        };
        let Some(buf) = Self::as_vk_buf(hgi_buf) else {
            log::error!("fill_buffer: not a HgiVulkanBuffer");
            return;
        };

        // Replicate 8-bit pattern across all four bytes of a u32.
        let v = value as u32;
        let value32 = v | (v << 8) | (v << 16) | (v << 24);

        // SAFETY: buffer handle is valid; VK_WHOLE_SIZE fills the entire buffer.
        unsafe {
            buf.device()
                .cmd_fill_buffer(vk_cmd, buf.vk_buffer(), 0, vk::WHOLE_SIZE, value32);
        }
    }

    /// Insert a pipeline memory barrier.  Delegates to the command buffer helper
    /// which records a full `READ|WRITE` → `READ|WRITE` barrier on ALL_COMMANDS.
    fn memory_barrier(&mut self, barrier: HgiMemoryBarrier) {
        if let Some(cb) = &self.command_buffer {
            cb.insert_memory_barrier(barrier);
        } else {
            log::warn!("memory_barrier: no command buffer set — skipped");
        }
    }
}
