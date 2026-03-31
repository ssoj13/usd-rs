//! Vulkan texture resource.
//!
//! Port of pxr/imaging/hgiVulkan/texture.cpp/.h

use std::sync::{Arc, Mutex};

use ash::vk;
use ash::vk::Handle as VkHandle;
use gpu_allocator::MemoryLocation;
use gpu_allocator::vulkan::{Allocation, AllocationCreateDesc, AllocationScheme, Allocator};

use usd_hgi::{
    HgiBuffer, HgiBufferDesc, HgiBufferUsage, HgiTexture, HgiTextureDesc, HgiTextureType,
    HgiTextureUsage, HgiTextureViewDesc, get_mip_infos,
};

use crate::buffer::HgiVulkanBuffer;
use crate::command_buffer::HgiVulkanCommandBuffer;
use crate::conversions::HgiVulkanConversions;

/// Marks no pending writes — equivalent to C++ `NO_PENDING_WRITES = 0`.
pub const NO_PENDING_WRITES: vk::AccessFlags = vk::AccessFlags::empty();

/// Allocation info returned by `get_allocation_info()`, mirroring C++ `VmaAllocationInfo2`.
///
/// `VmaAllocationInfo2` carries the same core fields plus VK_EXT_memory_budget stats;
/// here we expose what `gpu-allocator`'s `Allocation` provides.
pub struct TextureAllocationInfo {
    /// Backing `VkDeviceMemory` handle.
    pub device_memory: vk::DeviceMemory,
    /// Byte offset of the allocation within the `VkDeviceMemory`.
    pub offset: u64,
    /// Allocation size in bytes.
    pub size: u64,
    /// CPU-visible mapped pointer, if any.
    pub mapped_ptr: Option<*mut std::ffi::c_void>,
    /// Reported memory property flags for the allocation.
    pub memory_properties: vk::MemoryPropertyFlags,
}

/// Vulkan GPU texture resource.
///
/// Owns a `VkImage` + `VkImageView` pair and the gpu-allocator `Allocation`
/// backing them.  When constructed as a texture *view* (`new_view`) the image
/// handle is borrowed from the source texture and is **not** destroyed on drop.
pub struct HgiVulkanTexture {
    desc: HgiTextureDesc,
    /// Logical device handle (non-owning clone).  None in stub mode.
    device: Option<ash::Device>,
    /// The underlying Vulkan image.
    vk_image: vk::Image,
    /// Default image view covering all mips and layers.
    vk_image_view: vk::ImageView,
    /// Current image layout.  Updated by every layout barrier.
    vk_image_layout: vk::ImageLayout,
    /// gpu-allocator allocation backing the image (None for texture views or stubs).
    allocation: Option<Allocation>,
    /// Shared allocator — kept alive so we can free via it in Drop.
    allocator: Option<Arc<Mutex<Allocator>>>,
    /// Bit-mask tracking which in-flight command buffers reference this texture.
    inflight_bits: u64,
    /// Lazily-created staging buffer for CPU→GPU upload.
    staging_buffer: Option<Box<HgiVulkanBuffer>>,
    /// Mapped pointer into `staging_buffer` once it has been mapped.
    cpu_staging_address: Option<*mut u8>,
    /// True when this is an aliasing view — we do not own `vk_image`.
    is_texture_view: bool,
    /// Graphics queue used by `submit_layout_change` to issue one-shot layout barriers.
    /// `vk::Queue::null()` in stub mode.
    vk_graphics_queue: vk::Queue,
    /// Queue family index matching `vk_graphics_queue`; needed to create the transient pool.
    gfx_queue_family_index: u32,
}

// SAFETY: The raw pointer `cpu_staging_address` points into a VMA-managed
// staging buffer.  Access is guarded by the caller; we only expose it through
// &mut self, so Rust's borrow rules prevent concurrent mutation.
unsafe impl Send for HgiVulkanTexture {}
unsafe impl Sync for HgiVulkanTexture {}

impl HgiVulkanTexture {
    // -----------------------------------------------------------------------
    // Stub constructor (no device — used by the stub HgiVulkan impl)
    // -----------------------------------------------------------------------

    /// Create a descriptor-only stub, with no live Vulkan resources.
    ///
    /// Used by the `HgiVulkan` stub implementation until a real device is wired up.
    pub fn new_stub(desc: HgiTextureDesc) -> Self {
        Self {
            desc,
            device: None,
            vk_image: vk::Image::null(),
            vk_image_view: vk::ImageView::null(),
            vk_image_layout: vk::ImageLayout::UNDEFINED,
            allocation: None,
            allocator: None,
            inflight_bits: 0,
            staging_buffer: None,
            cpu_staging_address: None,
            is_texture_view: false,
            vk_graphics_queue: vk::Queue::null(),
            gfx_queue_family_index: 0,
        }
    }

    // -----------------------------------------------------------------------
    // Full constructor
    // -----------------------------------------------------------------------

    /// Create a new texture, allocating a fresh `VkImage` and binding memory.
    ///
    /// `optimal_tiling` selects `VK_IMAGE_TILING_OPTIMAL` vs LINEAR.
    /// Initial data upload is deferred to the caller — only the image and view
    /// are created here (matching the resource-creation split in C++ that calls
    /// `CopyBufferToTexture` / `LayoutBarrier` after construction).
    pub fn new(
        device: &ash::Device,
        allocator: Arc<Mutex<Allocator>>,
        desc: &HgiTextureDesc,
        optimal_tiling: bool,
        vk_graphics_queue: vk::Queue,
        gfx_queue_family_index: u32,
    ) -> Result<Self, String> {
        let is_depth = desc.usage.contains(HgiTextureUsage::DEPTH_TARGET);
        let vk_format = HgiVulkanConversions::get_format(desc.format, is_depth);
        let image_type = HgiVulkanConversions::get_texture_type(desc.texture_type);
        let usage_flags = HgiVulkanConversions::get_texture_usage(desc.usage);
        let sample_flags = HgiVulkanConversions::get_sample_count(desc.sample_count);

        // Cube maps require the CUBE_COMPATIBLE create flag.
        let create_flags = if desc.texture_type == HgiTextureType::Cubemap {
            vk::ImageCreateFlags::CUBE_COMPATIBLE
        } else {
            vk::ImageCreateFlags::empty()
        };

        let extent = vk::Extent3D {
            width: desc.dimensions[0] as u32,
            height: desc.dimensions[1] as u32,
            depth: desc.dimensions[2] as u32,
        };

        let image_info = vk::ImageCreateInfo::default()
            .image_type(image_type)
            .format(vk_format)
            .extent(extent)
            .mip_levels(desc.mip_levels as u32)
            .array_layers(desc.layer_count as u32)
            .samples(sample_flags)
            .tiling(if optimal_tiling {
                vk::ImageTiling::OPTIMAL
            } else {
                vk::ImageTiling::LINEAR
            })
            .usage(usage_flags)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .flags(create_flags);

        // SAFETY: device must be valid; image_info fields are all pod-safe.
        let vk_image = unsafe {
            device
                .create_image(&image_info, None)
                .map_err(|e| format!("vkCreateImage failed: {e}"))?
        };

        // Allocate and bind device-local memory via gpu-allocator.
        let requirements = unsafe { device.get_image_memory_requirements(vk_image) };

        let alloc_desc = AllocationCreateDesc {
            name: if desc.debug_name.is_empty() {
                "HgiVulkanTexture"
            } else {
                &desc.debug_name
            },
            requirements,
            location: MemoryLocation::GpuOnly,
            linear: !optimal_tiling,
            allocation_scheme: AllocationScheme::GpuAllocatorManaged,
        };

        let allocation = allocator
            .lock()
            .map_err(|e| {
                // SAFETY: image was just created above, no other owner.
                unsafe { device.destroy_image(vk_image, None) };
                format!("allocator lock poisoned: {e}")
            })?
            .allocate(&alloc_desc)
            .map_err(|e| {
                unsafe { device.destroy_image(vk_image, None) };
                format!("gpu-allocator allocate failed: {e}")
            })?;

        // SAFETY: image and memory must be from the same device; offset from allocator is valid.
        let bind_result =
            unsafe { device.bind_image_memory(vk_image, allocation.memory(), allocation.offset()) };
        if let Err(e) = bind_result {
            // Release resources in the correct order before propagating.
            allocator
                .lock()
                .ok()
                .and_then(|mut a| a.free(allocation).ok());
            unsafe { device.destroy_image(vk_image, None) };
            return Err(format!("vkBindImageMemory failed: {e}"));
        }

        // Build image view.
        let view_type = HgiVulkanConversions::get_texture_view_type(desc.texture_type);

        let aspect_mask = if desc.usage.contains(HgiTextureUsage::DEPTH_TARGET) {
            let mut mask = vk::ImageAspectFlags::DEPTH;
            if desc.usage.contains(HgiTextureUsage::STENCIL_TARGET) {
                mask |= vk::ImageAspectFlags::STENCIL;
            }
            mask
        } else {
            vk::ImageAspectFlags::COLOR
        };

        let components = vk::ComponentMapping {
            r: HgiVulkanConversions::get_component_swizzle(desc.component_mapping.r),
            g: HgiVulkanConversions::get_component_swizzle(desc.component_mapping.g),
            b: HgiVulkanConversions::get_component_swizzle(desc.component_mapping.b),
            a: HgiVulkanConversions::get_component_swizzle(desc.component_mapping.a),
        };

        let subresource_range = vk::ImageSubresourceRange {
            aspect_mask,
            base_mip_level: 0,
            level_count: desc.mip_levels as u32,
            base_array_layer: 0,
            layer_count: desc.layer_count as u32,
        };

        let view_info = vk::ImageViewCreateInfo::default()
            .image(vk_image)
            .view_type(view_type)
            .format(vk_format)
            .components(components)
            .subresource_range(subresource_range);

        let vk_image_view = unsafe { device.create_image_view(&view_info, None) }
            .map_err(|e| format!("vkCreateImageView failed: {e}"))?;

        // Debug naming via VK_EXT_debug_utils would go here if available;
        // skipped in the Rust port as the diagnostic module handles it separately.

        Ok(Self {
            desc: desc.clone(),
            device: Some(device.clone()),
            vk_image,
            vk_image_view,
            vk_image_layout: vk::ImageLayout::UNDEFINED,
            allocation: Some(allocation),
            allocator: Some(allocator),
            inflight_bits: 0,
            staging_buffer: None,
            cpu_staging_address: None,
            is_texture_view: false,
            vk_graphics_queue,
            gfx_queue_family_index,
        })
    }

    // -----------------------------------------------------------------------
    // Texture view constructor
    // -----------------------------------------------------------------------

    /// Create an aliasing texture view over an existing texture.
    ///
    /// The view shares the source image — it creates a new `VkImageView` but
    /// does **not** allocate memory or own the `VkImage`.
    pub fn new_view(
        device: &ash::Device,
        allocator: Arc<Mutex<Allocator>>,
        view_desc: &HgiTextureViewDesc,
        source: &HgiVulkanTexture,
        vk_graphics_queue: vk::Queue,
        gfx_queue_family_index: u32,
    ) -> Result<Self, String> {
        let src_desc = source.descriptor();
        let is_depth = src_desc.usage.contains(HgiTextureUsage::DEPTH_TARGET);

        let view_type = HgiVulkanConversions::get_texture_view_type(src_desc.texture_type);
        let vk_format = HgiVulkanConversions::get_format(view_desc.format, is_depth);

        let aspect_mask = if is_depth {
            vk::ImageAspectFlags::DEPTH
        } else {
            vk::ImageAspectFlags::COLOR
        };

        let components = vk::ComponentMapping {
            r: HgiVulkanConversions::get_component_swizzle(src_desc.component_mapping.r),
            g: HgiVulkanConversions::get_component_swizzle(src_desc.component_mapping.g),
            b: HgiVulkanConversions::get_component_swizzle(src_desc.component_mapping.b),
            a: HgiVulkanConversions::get_component_swizzle(src_desc.component_mapping.a),
        };

        let subresource_range = vk::ImageSubresourceRange {
            aspect_mask,
            base_mip_level: view_desc.source_first_mip as u32,
            level_count: view_desc.source_mip_count as u32,
            base_array_layer: view_desc.source_first_layer as u32,
            layer_count: view_desc.source_layer_count as u32,
        };

        let view_info = vk::ImageViewCreateInfo::default()
            .image(source.vk_image)
            .view_type(view_type)
            .format(vk_format)
            .components(components)
            .subresource_range(subresource_range);

        let vk_image_view = unsafe { device.create_image_view(&view_info, None) }
            .map_err(|e| format!("vkCreateImageView (view) failed: {e}"))?;

        // Build a descriptor that reflects the view parameters over the source desc.
        let mut desc = src_desc.clone();
        desc.debug_name = view_desc.debug_name.clone();
        desc.format = view_desc.format;
        desc.layer_count = view_desc.source_layer_count;
        desc.mip_levels = view_desc.source_mip_count;

        Ok(Self {
            desc,
            device: Some(device.clone()),
            // Borrow the image handle — not owned.
            vk_image: source.vk_image,
            vk_image_view,
            vk_image_layout: source.vk_image_layout,
            allocation: None,
            allocator: Some(allocator),
            inflight_bits: 0,
            staging_buffer: None,
            cpu_staging_address: None,
            is_texture_view: true,
            vk_graphics_queue,
            gfx_queue_family_index,
        })
    }

    // -----------------------------------------------------------------------
    // Accessors
    // -----------------------------------------------------------------------

    /// Returns the underlying `VkImage` handle.
    pub fn vk_image(&self) -> vk::Image {
        self.vk_image
    }

    /// Returns the default `VkImageView` covering all mips and layers.
    pub fn vk_image_view(&self) -> vk::ImageView {
        self.vk_image_view
    }

    /// Returns the current image layout.
    pub fn vk_image_layout(&self) -> vk::ImageLayout {
        self.vk_image_layout
    }

    /// Returns the current in-flight bits (bitmask of command buffer slots).
    pub fn inflight_bits(&self) -> u64 {
        self.inflight_bits
    }

    /// Sets the in-flight bits.
    pub fn set_inflight_bits(&mut self, bits: u64) {
        self.inflight_bits = bits;
    }

    /// Returns true if this is a texture view (aliasing another texture's image).
    pub fn is_texture_view(&self) -> bool {
        self.is_texture_view
    }

    /// Returns a clone of the logical device handle, if available.
    pub fn device_clone(&self) -> Option<ash::Device> {
        self.device.clone()
    }

    /// Returns the VMA-equivalent allocation info for this texture.
    ///
    /// Mirrors C++ `GetAllocationInfo()` → `VmaAllocationInfo2`.
    /// Returns `None` for stub textures or texture views (which don't own memory).
    pub fn get_allocation_info(&self) -> Option<TextureAllocationInfo> {
        let allocation = self.allocation.as_ref()?;
        // SAFETY: `allocation.memory()` is valid for the lifetime of the allocation;
        // the allocation is kept alive by `self`.
        let device_memory = unsafe { allocation.memory() };
        Some(TextureAllocationInfo {
            device_memory,
            offset: allocation.offset(),
            size: allocation.size(),
            mapped_ptr: allocation
                .mapped_ptr()
                .map(|p| p.as_ptr() as *mut std::ffi::c_void),
            memory_properties: allocation.memory_properties(),
        })
    }

    // -----------------------------------------------------------------------
    // Staging
    // -----------------------------------------------------------------------

    /// Returns (and lazily creates) the CPU staging buffer.
    ///
    /// Callers memcpy into this address then schedule a `copy_texture_cpu_to_gpu`
    /// blit command.  Equivalent to C++ `GetCPUStagingAddress`.
    pub fn get_cpu_staging_address(&mut self) -> Option<*mut u8> {
        if self.staging_buffer.is_none() {
            let (Some(device), Some(allocator)) = (self.device.as_ref(), self.allocator.as_ref())
            else {
                // Stub mode — no device available to create a real staging buffer.
                return None;
            };
            let byte_size = self.byte_size_of_resource();
            let mut buf_desc = HgiBufferDesc::default();
            buf_desc.usage = HgiBufferUsage::UPLOAD;
            buf_desc.byte_size = byte_size;
            buf_desc.debug_name = format!("Staging Buffer for {}", self.desc.debug_name);
            match HgiVulkanBuffer::new(device, allocator.clone(), &buf_desc, None) {
                Ok(buf) => self.staging_buffer = Some(Box::new(buf)),
                Err(e) => {
                    log::error!("HgiVulkanTexture: failed to create staging buffer: {e}");
                    return None;
                }
            }
        }

        // The actual VMA map call would go here once buffer.rs is fully implemented.
        // For now we return whatever the staging buffer exposes via the HgiBuffer trait.
        if self.cpu_staging_address.is_none() {
            self.cpu_staging_address = self
                .staging_buffer
                .as_mut()
                .and_then(|b| b.cpu_staging_address());
        }

        self.cpu_staging_address
    }

    /// Returns true when `address` matches the mapped staging buffer pointer.
    pub fn is_cpu_staging_address(&self, address: *const u8) -> bool {
        self.cpu_staging_address
            .map(|p| p as *const u8 == address)
            .unwrap_or(false)
    }

    /// Returns a reference to the staging buffer, if one has been created.
    pub fn staging_buffer(&self) -> Option<&HgiVulkanBuffer> {
        self.staging_buffer.as_deref()
    }

    // -----------------------------------------------------------------------
    // Buffer → Texture copy
    // -----------------------------------------------------------------------

    /// Schedule a copy from `src_buffer` into this texture via `cb`.
    ///
    /// When `mip_level` is -1 all mip levels present in the buffer are copied.
    /// Mirrors C++ `CopyBufferToTexture`.
    pub fn copy_buffer_to_texture(
        &mut self,
        cb: &HgiVulkanCommandBuffer,
        src_buffer: vk::Buffer,
        src_buffer_byte_size: usize,
        dst_texel_offset: [i32; 3],
        mip_level: i32,
    ) {
        let Some(device) = self.device.clone() else {
            log::warn!("HgiVulkanTexture::copy_buffer_to_texture: no device (stub mode)");
            return;
        };

        let mip_infos = get_mip_infos(
            self.desc.format,
            &self.desc.dimensions,
            self.desc.layer_count as usize,
            Some(src_buffer_byte_size),
        );

        let base_mip = if mip_level > -1 { mip_level as u32 } else { 0 };
        let mip_count = if mip_level > -1 {
            1u32
        } else {
            (mip_infos.len() as u32).min(self.desc.mip_levels as u32)
        };

        let aspect_mask = HgiVulkanConversions::get_image_aspect_flag(self.desc.usage);

        let copy_regions: Vec<vk::BufferImageCopy> = (base_mip..(base_mip + mip_count))
            .map(|mip| {
                let info = &mip_infos[mip as usize];
                vk::BufferImageCopy {
                    buffer_offset: info.byte_offset as u64,
                    buffer_row_length: 0,
                    buffer_image_height: 0,
                    image_subresource: vk::ImageSubresourceLayers {
                        aspect_mask,
                        mip_level: mip,
                        base_array_layer: 0,
                        layer_count: self.desc.layer_count as u32,
                    },
                    image_offset: vk::Offset3D {
                        x: dst_texel_offset[0],
                        y: dst_texel_offset[1],
                        z: dst_texel_offset[2],
                    },
                    image_extent: vk::Extent3D {
                        width: info.dimensions[0] as u32,
                        height: info.dimensions[1] as u32,
                        depth: info.dimensions[2] as u32,
                    },
                }
            })
            .collect();

        // Transition to transfer destination layout.
        self.layout_barrier(
            cb,
            self.vk_image_layout,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            NO_PENDING_WRITES,
            vk::AccessFlags::TRANSFER_WRITE,
            vk::PipelineStageFlags::HOST,
            vk::PipelineStageFlags::TRANSFER,
            -1,
        );

        // Record the copy command.
        // SAFETY: cb and src_buffer must both be valid for the duration of this call.
        unsafe {
            device.cmd_copy_buffer_to_image(
                cb.vk_command_buffer(),
                src_buffer,
                self.vk_image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &copy_regions,
            );
        }

        // Transition to the default layout for this texture's usage.
        let final_layout = Self::get_default_image_layout(self.desc.usage);
        let final_access = Self::get_default_access_flags(self.desc.usage);

        self.layout_barrier(
            cb,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            final_layout,
            vk::AccessFlags::TRANSFER_WRITE,
            final_access,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::ALL_GRAPHICS,
            -1,
        );
    }

    // -----------------------------------------------------------------------
    // Layout barrier
    // -----------------------------------------------------------------------

    /// Record an image memory barrier transitioning from `old_layout` to `new_layout`.
    ///
    /// `producer_access = NO_PENDING_WRITES` means invalidation-only (read-only resource).
    /// When `mip_level >= 0` only that single mip level is transitioned.
    ///
    /// Mirrors C++ `LayoutBarrier`.
    #[allow(clippy::too_many_arguments)]
    pub fn layout_barrier(
        &mut self,
        cb: &HgiVulkanCommandBuffer,
        old_layout: vk::ImageLayout,
        new_layout: vk::ImageLayout,
        producer_access: vk::AccessFlags,
        consumer_access: vk::AccessFlags,
        producer_stage: vk::PipelineStageFlags,
        consumer_stage: vk::PipelineStageFlags,
        mip_level: i32,
    ) {
        let Some(device) = self.device.clone() else {
            log::warn!("HgiVulkanTexture::layout_barrier: no device (stub mode)");
            return;
        };

        let first_mip = if mip_level < 0 { 0 } else { mip_level as u32 };
        let mip_count = if mip_level < 0 {
            vk::REMAINING_MIP_LEVELS
        } else {
            1
        };

        let aspect_mask = HgiVulkanConversions::get_image_aspect_flag(self.desc.usage);

        let barrier = vk::ImageMemoryBarrier::default()
            .src_access_mask(producer_access)
            .dst_access_mask(consumer_access)
            .old_layout(old_layout)
            .new_layout(new_layout)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(self.vk_image)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask,
                base_mip_level: first_mip,
                level_count: mip_count,
                base_array_layer: 0,
                layer_count: self.desc.layer_count as u32,
            });

        // SAFETY: cb must be in the recording state.
        unsafe {
            device.cmd_pipeline_barrier(
                cb.vk_command_buffer(),
                producer_stage,
                consumer_stage,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier],
            );
        }

        self.vk_image_layout = new_layout;
    }

    // -----------------------------------------------------------------------
    // One-shot layout barrier (used by submit_layout_change)
    // -----------------------------------------------------------------------

    /// Create a transient command pool, record a single pipeline barrier, and
    /// immediately submit + wait for it on `vk_graphics_queue`.
    ///
    /// This matches C++ `SubmitLayoutChange` which records into the resource
    /// command buffer (submitted at frame end).  Since the Rust texture can't
    /// access the frame cycle directly, we submit immediately — safe because
    /// `submit_layout_change` is only called outside of active render passes.
    #[allow(clippy::too_many_arguments)]
    fn record_and_submit_layout_barrier(
        &mut self,
        device: &ash::Device,
        old_layout: vk::ImageLayout,
        new_layout: vk::ImageLayout,
        src_access: vk::AccessFlags,
        dst_access: vk::AccessFlags,
        src_stage: vk::PipelineStageFlags,
        dst_stage: vk::PipelineStageFlags,
    ) {
        let pool_info = vk::CommandPoolCreateInfo::default()
            .flags(vk::CommandPoolCreateFlags::TRANSIENT)
            .queue_family_index(self.gfx_queue_family_index);

        // SAFETY: device is valid; pool_info fields are all pod-safe.
        let pool = match unsafe { device.create_command_pool(&pool_info, None) } {
            Ok(p) => p,
            Err(e) => {
                log::error!("submit_layout_change: vkCreateCommandPool failed: {:?}", e);
                return;
            }
        };

        let alloc_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);

        let cb = match unsafe { device.allocate_command_buffers(&alloc_info) } {
            Ok(cbs) => cbs[0],
            Err(e) => {
                log::error!(
                    "submit_layout_change: vkAllocateCommandBuffers failed: {:?}",
                    e
                );
                // SAFETY: pool was just created above.
                unsafe { device.destroy_command_pool(pool, None) };
                return;
            }
        };

        let begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

        if let Err(e) = unsafe { device.begin_command_buffer(cb, &begin_info) } {
            log::error!("submit_layout_change: vkBeginCommandBuffer failed: {:?}", e);
            unsafe { device.destroy_command_pool(pool, None) };
            return;
        }

        let aspect_mask = HgiVulkanConversions::get_image_aspect_flag(self.desc.usage);
        let barrier = vk::ImageMemoryBarrier::default()
            .src_access_mask(src_access)
            .dst_access_mask(dst_access)
            .old_layout(old_layout)
            .new_layout(new_layout)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(self.vk_image)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask,
                base_mip_level: 0,
                level_count: vk::REMAINING_MIP_LEVELS,
                base_array_layer: 0,
                layer_count: self.desc.layer_count as u32,
            });

        // SAFETY: cb is in recording state, all handles are valid.
        unsafe {
            device.cmd_pipeline_barrier(
                cb,
                src_stage,
                dst_stage,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[barrier],
            );
        }

        if let Err(e) = unsafe { device.end_command_buffer(cb) } {
            log::error!("submit_layout_change: vkEndCommandBuffer failed: {:?}", e);
            unsafe { device.destroy_command_pool(pool, None) };
            return;
        }

        let submit_info = vk::SubmitInfo::default().command_buffers(std::slice::from_ref(&cb));

        // SAFETY: queue and cb are valid; no other submits racing on this queue at this point.
        let submit_result = unsafe {
            device.queue_submit(self.vk_graphics_queue, &[submit_info], vk::Fence::null())
        };
        if let Err(e) = submit_result {
            log::error!("submit_layout_change: vkQueueSubmit failed: {:?}", e);
        } else {
            // Wait for the single barrier command to complete before returning.
            if let Err(e) = unsafe { device.queue_wait_idle(self.vk_graphics_queue) } {
                log::error!("submit_layout_change: vkQueueWaitIdle failed: {:?}", e);
            }
        }

        // Free the transient pool (and its command buffer) immediately.
        // SAFETY: GPU is idle after queue_wait_idle, pool and cb are safe to destroy.
        unsafe { device.destroy_command_pool(pool, None) };
    }

    // -----------------------------------------------------------------------
    // Static helpers
    // -----------------------------------------------------------------------

    /// Returns the default `VkImageLayout` for the given texture usage flags.
    ///
    /// Priority order (matches C++):
    /// `SHADER_WRITE` → GENERAL, `COLOR_TARGET` → COLOR_ATTACHMENT_OPTIMAL,
    /// depth/stencil → DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
    /// `SHADER_READ` → SHADER_READ_ONLY_OPTIMAL.
    pub fn get_default_image_layout(usage: HgiTextureUsage) -> vk::ImageLayout {
        if usage.is_empty() {
            log::error!("HgiVulkanTexture::get_default_image_layout: invalid (empty) usage");
        }

        if usage.contains(HgiTextureUsage::SHADER_WRITE) {
            return vk::ImageLayout::GENERAL;
        }
        if usage.contains(HgiTextureUsage::COLOR_TARGET) {
            return vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;
        }
        if usage.contains(HgiTextureUsage::DEPTH_TARGET)
            || usage.contains(HgiTextureUsage::STENCIL_TARGET)
        {
            return vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL;
        }
        // Default: shader-read (covers SHADER_READ and any unrecognised usage).
        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL
    }

    /// Returns the default `VkAccessFlags` for the given texture usage flags.
    ///
    /// Mirrors C++ `GetDefaultAccessFlags`.
    pub fn get_default_access_flags(usage: HgiTextureUsage) -> vk::AccessFlags {
        if usage.is_empty() {
            log::error!("HgiVulkanTexture::get_default_access_flags: invalid (empty) usage");
        }

        let mut flags = vk::AccessFlags::empty();
        if usage.contains(HgiTextureUsage::SHADER_READ) {
            flags |= vk::AccessFlags::SHADER_READ;
        }
        if usage.contains(HgiTextureUsage::DEPTH_TARGET) {
            flags |= vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE;
        } else if usage.contains(HgiTextureUsage::COLOR_TARGET) {
            flags |= vk::AccessFlags::COLOR_ATTACHMENT_WRITE;
        }
        flags
    }

    /// Convert a `VkImageLayout` back to the corresponding `HgiTextureUsage` bits.
    ///
    /// Used by `submit_layout_change` to return the *previous* layout as usage.
    /// Mirrors the file-static `_VkImageLayoutToHgiTextureUsage` in C++.
    fn vk_layout_to_hgi_usage(layout: vk::ImageLayout) -> HgiTextureUsage {
        match layout {
            vk::ImageLayout::GENERAL => HgiTextureUsage::SHADER_WRITE,
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL => HgiTextureUsage::COLOR_TARGET,
            vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL => HgiTextureUsage::DEPTH_TARGET,
            vk::ImageLayout::STENCIL_ATTACHMENT_OPTIMAL => HgiTextureUsage::STENCIL_TARGET,
            vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL => {
                HgiTextureUsage::DEPTH_TARGET | HgiTextureUsage::STENCIL_TARGET
            }
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL => HgiTextureUsage::SHADER_READ,
            _ => {
                log::error!(
                    "HgiVulkanTexture::vk_layout_to_hgi_usage: unsupported layout {:?}",
                    layout
                );
                HgiTextureUsage::empty()
            }
        }
    }
}

// ---------------------------------------------------------------------------
// HgiTexture trait
// ---------------------------------------------------------------------------

impl HgiTexture for HgiVulkanTexture {
    fn descriptor(&self) -> &HgiTextureDesc {
        &self.desc
    }

    /// Computes the byte size across all mip levels × layers.
    ///
    /// Mirrors C++ `GetByteSizeOfResource` → `_GetByteSizeOfResource`.
    fn byte_size_of_resource(&self) -> usize {
        let mip_infos = get_mip_infos(
            self.desc.format,
            &self.desc.dimensions,
            self.desc.layer_count as usize,
            None,
        );

        // Sum over the mip levels present in the descriptor.
        let mip_count = (self.desc.mip_levels as usize).min(mip_infos.len());
        mip_infos[..mip_count]
            .iter()
            .map(|m| m.byte_size_per_layer * self.desc.layer_count as usize)
            .sum()
    }

    /// Returns the `VkImage` handle cast to `u64`.
    fn raw_resource(&self) -> u64 {
        self.vk_image.as_raw()
    }

    /// Returns the mapped staging buffer pointer, creating the staging buffer on first call.
    fn cpu_staging_address(&mut self) -> Option<*mut u8> {
        self.get_cpu_staging_address()
    }

    /// Transitions the image layout and returns the previous layout as usage bits.
    ///
    /// Mirrors C++ `SubmitLayoutChange`.
    fn submit_layout_change(&mut self, new_usage: HgiTextureUsage) -> HgiTextureUsage {
        let old_vk_layout = self.vk_image_layout;
        let new_vk_layout = Self::get_default_image_layout(new_usage);

        if old_vk_layout == new_vk_layout {
            return Self::vk_layout_to_hgi_usage(old_vk_layout);
        }

        // Determine source access mask and stage from the old layout.
        let (src_access, src_stage) = match old_vk_layout {
            vk::ImageLayout::PREINITIALIZED => (
                vk::AccessFlags::HOST_WRITE | vk::AccessFlags::TRANSFER_WRITE,
                vk::PipelineStageFlags::ALL_GRAPHICS,
            ),
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL => (
                vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            ),
            vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL => (
                vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
            ),
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL => (
                vk::AccessFlags::TRANSFER_READ,
                vk::PipelineStageFlags::TRANSFER,
            ),
            vk::ImageLayout::TRANSFER_DST_OPTIMAL => (
                vk::AccessFlags::TRANSFER_WRITE,
                vk::PipelineStageFlags::TRANSFER,
            ),
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL => (
                vk::AccessFlags::SHADER_READ,
                vk::PipelineStageFlags::ALL_GRAPHICS,
            ),
            _ => (
                vk::AccessFlags::empty(),
                vk::PipelineStageFlags::ALL_GRAPHICS,
            ),
        };

        // Determine destination access mask and stage from the new layout.
        let (dst_access, dst_stage) = match new_vk_layout {
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
        };

        // C++ acquires the resource command buffer from the device command queue.
        // In the Rust port the texture stores the raw queue handle and creates a
        // transient one-shot command pool so it can issue the barrier independently.
        // Clone the Arc<Device> first to release the immutable borrow before the &mut self call.
        let device_clone = self.device.clone();
        if let Some(device) = device_clone {
            if self.vk_graphics_queue != vk::Queue::null() {
                self.record_and_submit_layout_barrier(
                    &device,
                    old_vk_layout,
                    new_vk_layout,
                    src_access,
                    dst_access,
                    src_stage,
                    dst_stage,
                );
            } else {
                // Stub or interop path with no queue — update tracked layout only.
                log::debug!(
                    "HgiVulkanTexture::submit_layout_change: no queue handle; \
                     layout state updated from {:?} to {:?} without GPU barrier",
                    old_vk_layout,
                    new_vk_layout
                );
            }
        }

        self.vk_image_layout = new_vk_layout;
        Self::vk_layout_to_hgi_usage(old_vk_layout)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// ---------------------------------------------------------------------------
// Drop — destroy Vulkan objects in the correct order
// ---------------------------------------------------------------------------

impl Drop for HgiVulkanTexture {
    fn drop(&mut self) {
        // Release mapped staging pointer (unmap is handled when buffer.rs is fully implemented).
        self.cpu_staging_address = None;
        self.staging_buffer = None;

        let Some(device) = self.device.take() else {
            // Stub mode — no real Vulkan objects to destroy.
            return;
        };

        // Always destroy our image view.
        if self.vk_image_view != vk::ImageView::null() {
            // SAFETY: image view was created by us from `device`.
            unsafe {
                device.destroy_image_view(self.vk_image_view, None);
            }
            self.vk_image_view = vk::ImageView::null();
        }

        // Only destroy the image when we own it (i.e. not a texture view).
        if !self.is_texture_view && self.vk_image != vk::Image::null() {
            if let Some(allocation) = self.allocation.take() {
                if let Some(ref allocator) = self.allocator {
                    if let Ok(mut guard) = allocator.lock() {
                        guard.free(allocation).unwrap_or_else(|e| {
                            log::error!("HgiVulkanTexture: failed to free allocation: {e}");
                        });
                    }
                }
            }
            // SAFETY: image was created by us from `device`.
            unsafe {
                device.destroy_image(self.vk_image, None);
            }
            self.vk_image = vk::Image::null();
        }
    }
}
