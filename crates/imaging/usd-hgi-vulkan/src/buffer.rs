//! Vulkan buffer resource.
//!
//! Port of pxr/imaging/hgiVulkan/buffer.cpp/.h

// All methods here call into unsafe Vulkan and gpu-allocator FFI.
#![allow(unsafe_code)]

use std::sync::{Arc, Mutex};

use ash::vk;
use ash::vk::Handle as VkHandle;
use gpu_allocator::MemoryLocation;
use gpu_allocator::vulkan::{Allocation, AllocationCreateDesc, AllocationScheme, Allocator};
use usd_hgi::{HgiBuffer, HgiBufferDesc, HgiBufferUsage};

use crate::conversions::HgiVulkanConversions;

/// Vulkan buffer resource.
///
/// Owns a `VkBuffer` and its backing `gpu_allocator::Allocation`.
/// On UMA / upload buffers the allocation is host-visible and can be mapped
/// directly; on discrete GPU buffers a CPU-side staging buffer is created
/// lazily on first call to `cpu_staging_address`.
pub struct HgiVulkanBuffer {
    desc: HgiBufferDesc,
    /// Raw Vulkan device handle — used for `vkDestroyBuffer`.
    device: ash::Device,
    /// Shared allocator — needed in `Drop` to free the allocation.
    allocator: Arc<Mutex<Allocator>>,
    vk_buffer: vk::Buffer,
    allocation: Option<Allocation>,
    /// Lazily created staging buffer (GPU-only path).
    staging_buffer: Option<Box<HgiVulkanBuffer>>,
    /// Mapped host pointer — either the allocation itself (mappable path) or
    /// the staging buffer's mapped memory (GPU-only path).
    cpu_staging_address: Option<*mut u8>,
    /// True when the backing allocation is host-visible (upload or UMA).
    mappable: bool,
    /// Bitmask tracking which in-flight command buffers reference this buffer,
    /// used by the garbage collector to defer destruction safely.
    inflight_bits: u64,
}

// SAFETY: The raw pointers stored here (`cpu_staging_address`) are only ever
// produced by gpu-allocator's mapped memory, which is valid for the lifetime
// of the allocation.  Callers must not use the pointer after the buffer is
// destroyed.  The buffer itself is otherwise Send+Sync because all mutable
// access goes through `&mut self` or the `Arc<Mutex<Allocator>>`.
unsafe impl Send for HgiVulkanBuffer {}
unsafe impl Sync for HgiVulkanBuffer {}

impl HgiVulkanBuffer {
    /// Creates a GPU buffer described by `desc`.
    ///
    /// If `initial_data` is provided and the allocation is host-visible the
    /// data is copied immediately.  Otherwise a one-shot staging buffer is
    /// returned via `staging_buffer()` and the caller is responsible for
    /// recording a `vkCmdCopyBuffer` on a command buffer and scheduling the
    /// staging buffer for deletion.
    pub fn new(
        device: &ash::Device,
        allocator: Arc<Mutex<Allocator>>,
        desc: &HgiBufferDesc,
        initial_data: Option<&[u8]>,
    ) -> Result<Self, String> {
        if desc.byte_size == 0 {
            return Err(format!(
                "HgiVulkanBuffer: byte_size is zero for buffer '{}'",
                desc.debug_name
            ));
        }

        let is_upload = desc.usage.contains(HgiBufferUsage::UPLOAD);

        // Always add TRANSFER_SRC/DST so we can use this buffer as staging
        // source or copy destination without recreating it.
        let usage_flags = HgiVulkanConversions::get_buffer_usage(desc.usage)
            | vk::BufferUsageFlags::TRANSFER_SRC
            | vk::BufferUsageFlags::TRANSFER_DST;

        let buffer_info = vk::BufferCreateInfo::default()
            .size(desc.byte_size as u64)
            .usage(usage_flags)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        // SAFETY: valid device handle; create_buffer is an FFI call.
        let vk_buffer = unsafe {
            device
                .create_buffer(&buffer_info, None)
                .map_err(|e| format!("vkCreateBuffer failed: {e:?}"))?
        };

        // SAFETY: valid buffer handle.
        let mem_reqs = unsafe { device.get_buffer_memory_requirements(vk_buffer) };

        let memory_location = if is_upload {
            // Upload / staging buffers live in host-visible memory.
            MemoryLocation::CpuToGpu
        } else {
            // Regular buffers prefer device-local memory.
            MemoryLocation::GpuOnly
        };

        let alloc_desc = AllocationCreateDesc {
            name: desc.debug_name.as_str(),
            requirements: mem_reqs,
            location: memory_location,
            linear: true, // buffers are always linear resources
            allocation_scheme: AllocationScheme::GpuAllocatorManaged,
        };

        let allocation = {
            let mut alloc_guard = allocator
                .lock()
                .map_err(|e| format!("allocator mutex poisoned: {e}"))?;
            alloc_guard
                .allocate(&alloc_desc)
                .map_err(|e| format!("gpu_allocator::allocate failed: {e}"))?
        };

        // Bind buffer to its memory.
        // SAFETY: valid device/buffer/memory handles.
        unsafe {
            device
                .bind_buffer_memory(vk_buffer, allocation.memory(), allocation.offset())
                .map_err(|e| format!("vkBindBufferMemory failed: {e:?}"))?;
        }

        // Whether this buffer can be mapped directly.
        let mappable = is_upload || allocation.mapped_ptr().is_some();

        let mut buffer = Self {
            desc: desc.clone(),
            device: device.clone(),
            allocator,
            vk_buffer,
            allocation: Some(allocation),
            staging_buffer: None,
            cpu_staging_address: None,
            mappable,
            inflight_bits: 0,
        };

        // Upload initial data if provided.
        if let Some(data) = initial_data {
            if buffer.mappable {
                // Host-visible: write directly.
                buffer.write_bytes(data)?;
            } else {
                // GPU-only: create staging buffer, memcpy into it.
                // The caller must record vkCmdCopyBuffer before submitting.
                let mut staging_desc = desc.clone();
                staging_desc.usage = HgiBufferUsage::UPLOAD;
                if !staging_desc.debug_name.is_empty() {
                    staging_desc.debug_name =
                        format!("Staging Buffer for {}", staging_desc.debug_name);
                }

                let mut staging = Self::create_staging_buffer(
                    &buffer.device,
                    Arc::clone(&buffer.allocator),
                    &staging_desc,
                )?;
                staging.write_bytes(data)?;
                buffer.staging_buffer = Some(Box::new(staging));
            }
        }

        Ok(buffer)
    }

    /// Creates a CPU-visible (CpuToGpu) staging buffer.
    pub fn create_staging_buffer(
        device: &ash::Device,
        allocator: Arc<Mutex<Allocator>>,
        desc: &HgiBufferDesc,
    ) -> Result<Self, String> {
        let mut staging_desc = desc.clone();
        staging_desc.usage = HgiBufferUsage::UPLOAD;
        Self::new(device, allocator, &staging_desc, None)
    }

    /// Writes `data` into the host-visible allocation backing this buffer.
    ///
    /// Panics if the buffer is not mappable.
    fn write_bytes(&mut self, data: &[u8]) -> Result<(), String> {
        let alloc = self
            .allocation
            .as_ref()
            .ok_or_else(|| "HgiVulkanBuffer: allocation already freed".to_string())?;

        let mapped = alloc.mapped_ptr().ok_or_else(|| {
            "HgiVulkanBuffer::write_bytes: allocation is not host-visible".to_string()
        })?;

        let len = data.len().min(self.desc.byte_size);
        // SAFETY: mapped_ptr is valid for at least `byte_size` bytes while the
        // allocation is alive.
        unsafe {
            std::ptr::copy_nonoverlapping(data.as_ptr(), mapped.as_ptr() as *mut u8, len);
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Accessors (mirror HgiVulkanBuffer C++ API)
    // -----------------------------------------------------------------------

    /// Returns the underlying `VkBuffer`.
    pub fn vk_buffer(&self) -> vk::Buffer {
        self.vk_buffer
    }

    /// Returns a reference to the GPU memory allocation.
    pub fn allocation(&self) -> Option<&Allocation> {
        self.allocation.as_ref()
    }

    /// Returns the staging buffer created during construction (GPU-only path).
    pub fn staging_buffer(&self) -> Option<&HgiVulkanBuffer> {
        self.staging_buffer.as_deref()
    }

    /// Returns the in-flight bitmask used by the garbage collector.
    pub fn inflight_bits(&self) -> u64 {
        self.inflight_bits
    }

    /// Sets the in-flight bitmask.
    pub fn set_inflight_bits(&mut self, bits: u64) {
        self.inflight_bits = bits;
    }

    /// Returns a mutable reference to the in-flight bitmask (mirrors C++
    /// `GetInflightBits()` which returns a `uint64_t&`).
    pub fn inflight_bits_mut(&mut self) -> &mut u64 {
        &mut self.inflight_bits
    }

    /// Returns true when `addr` equals the CPU staging address held by this
    /// buffer (mirrors `IsCPUStagingAddress`).
    pub fn is_cpu_staging_address(&self, addr: *const u8) -> bool {
        match self.cpu_staging_address {
            Some(ptr) => std::ptr::eq(ptr, addr),
            None => false,
        }
    }

    /// Returns the cached CPU staging pointer without requiring `&mut self`.
    ///
    /// Returns `None` if the staging address has not been initialised yet
    /// (call `cpu_staging_address()` first to ensure it is mapped).
    pub fn cpu_staging_address_raw(&self) -> Option<*mut u8> {
        self.cpu_staging_address
    }

    /// Returns a reference to the logical device used to create this buffer.
    pub fn device(&self) -> &ash::Device {
        &self.device
    }
}

impl HgiBuffer for HgiVulkanBuffer {
    fn descriptor(&self) -> &HgiBufferDesc {
        &self.desc
    }

    fn byte_size_of_resource(&self) -> usize {
        // Return the actual allocated size when available; fall back to the
        // requested size from the descriptor.
        self.allocation
            .as_ref()
            .map(|a| a.size() as usize)
            .unwrap_or(self.desc.byte_size)
    }

    fn raw_resource(&self) -> u64 {
        self.vk_buffer.as_raw()
    }

    /// Returns a host-visible pointer suitable for CPU→GPU data transfer.
    ///
    /// On mappable (upload / UMA) buffers the allocation is mapped directly.
    /// On GPU-only buffers a staging `HgiVulkanBuffer` is created lazily and
    /// the caller must later submit a copy command via `BlitCmds`.
    fn cpu_staging_address(&mut self) -> Option<*mut u8> {
        if self.cpu_staging_address.is_some() {
            return self.cpu_staging_address;
        }

        if self.mappable {
            // Map the allocation directly — for upload buffers the allocator
            // already keeps the mapping alive for the allocation lifetime.
            let ptr = self
                .allocation
                .as_ref()
                .and_then(|a| a.mapped_ptr())
                .map(|p| p.as_ptr() as *mut u8);
            self.cpu_staging_address = ptr;
        } else {
            // GPU-only: create a staging buffer and expose its mapped pointer.
            let mut staging_desc = self.desc.clone();
            staging_desc.usage = HgiBufferUsage::UPLOAD;
            if !staging_desc.debug_name.is_empty() {
                staging_desc.debug_name =
                    format!("Staging Buffer for: {}", staging_desc.debug_name);
            }
            staging_desc.byte_size = self.desc.byte_size;

            match Self::create_staging_buffer(
                &self.device,
                Arc::clone(&self.allocator),
                &staging_desc,
            ) {
                Ok(staging) => {
                    let ptr = staging
                        .allocation
                        .as_ref()
                        .and_then(|a| a.mapped_ptr())
                        .map(|p| p.as_ptr() as *mut u8);
                    self.staging_buffer = Some(Box::new(staging));
                    self.cpu_staging_address = ptr;
                }
                Err(e) => {
                    log::error!("HgiVulkanBuffer::cpu_staging_address: {e}");
                }
            }
        }

        self.cpu_staging_address
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl Drop for HgiVulkanBuffer {
    fn drop(&mut self) {
        // Drop the staging buffer first so its own Drop runs before we
        // invalidate `self.device`.
        self.staging_buffer = None;
        self.cpu_staging_address = None;

        if let Some(allocation) = self.allocation.take() {
            match self.allocator.lock() {
                Ok(mut guard) => {
                    if let Err(e) = guard.free(allocation) {
                        log::error!("HgiVulkanBuffer: gpu_allocator::free failed: {e}");
                    }
                }
                Err(e) => {
                    log::error!("HgiVulkanBuffer: allocator mutex poisoned on drop: {e}");
                }
            }
        }

        if self.vk_buffer != vk::Buffer::null() {
            // SAFETY: the device is still valid (we hold a clone of its
            // internal Arc), and this buffer has not been destroyed yet.
            unsafe { self.device.destroy_buffer(self.vk_buffer, None) };
        }
    }
}
