//! Vulkan resource bindings (descriptor sets).
//!
//! Port of pxr/imaging/hgiVulkan/resourceBindings.cpp/.h

#![allow(unsafe_code)]

use ash::vk::{self, Handle};
use usd_hgi::{HgiBindResourceType, HgiResourceBindings, HgiResourceBindingsDesc, HgiShaderStage};

use crate::conversions::HgiVulkanConversions;
use crate::diagnostic;

// Total number of HgiBindResourceType variants (static_assert matches C++).
const BIND_RESOURCE_TYPE_COUNT: usize = 7;

/// Vulkan resource bindings: descriptor pool + layout + set.
///
/// Each instance owns its own VkDescriptorPool so that pool reset and
/// multi-threaded allocation are trivially safe (mirrors C++ design comment).
pub struct HgiVulkanResourceBindings {
    desc: HgiResourceBindingsDesc,
    device: ash::Device,
    vk_descriptor_pool: vk::DescriptorPool,
    vk_descriptor_set_layout: vk::DescriptorSetLayout,
    vk_descriptor_set: vk::DescriptorSet,
    inflight_bits: u64,
}

impl std::fmt::Debug for HgiVulkanResourceBindings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HgiVulkanResourceBindings")
            .field("debug_name", &self.desc.debug_name)
            .field("inflight_bits", &self.inflight_bits)
            .finish_non_exhaustive()
    }
}

impl HgiVulkanResourceBindings {
    /// Creates descriptor pool, layout, and set from `desc`.
    ///
    /// Mirrors `HgiVulkanResourceBindings::HgiVulkanResourceBindings()`.
    pub fn new(
        device: &ash::Device,
        desc: &HgiResourceBindingsDesc,
        debug_utils: Option<&ash::ext::debug_utils::Device>,
    ) -> Result<Self, String> {
        // ----------------------------------------------------------------
        // Build per-type pool size counters (one slot per resource type).
        // Vulkan validation requires descriptorCount >= 1 per pool entry.
        // ----------------------------------------------------------------
        let mut pool_sizes = [vk::DescriptorPoolSize::default(); BIND_RESOURCE_TYPE_COUNT];
        for (i, pool_size) in pool_sizes.iter_mut().enumerate() {
            let rt = index_to_bind_resource_type(i);
            pool_size.ty = HgiVulkanConversions::get_descriptor_type(rt);
            pool_size.descriptor_count = 0;
        }

        // ----------------------------------------------------------------
        // Compute the texture binding index offset so texture bindings
        // start after the last buffer binding index (matching C++ Storm
        // convention: UBO/SSBO share one counter, textures have their own).
        // ----------------------------------------------------------------
        let mut texture_bind_index_start: u32 = 0;

        // Stage flags used when not compute-only (C++ overspecifies all
        // graphics stages so the layout matches spirv-reflect output).
        let buffer_stages = HgiVulkanConversions::get_shader_stages(
            HgiShaderStage::VERTEX
                | HgiShaderStage::TESSELLATION_CONTROL
                | HgiShaderStage::TESSELLATION_EVAL
                | HgiShaderStage::GEOMETRY
                | HgiShaderStage::FRAGMENT,
        );
        let texture_stages = HgiVulkanConversions::get_shader_stages(
            HgiShaderStage::GEOMETRY | HgiShaderStage::FRAGMENT,
        );

        // ----------------------------------------------------------------
        // Descriptor set layout bindings
        // ----------------------------------------------------------------
        let mut layout_bindings: Vec<vk::DescriptorSetLayoutBinding> = Vec::new();

        // Buffers
        for buf_desc in &desc.buffer_bindings {
            let rt_index = buf_desc.resource_type as usize;
            pool_sizes[rt_index].descriptor_count += 1;

            let stage_flags = if buf_desc.stage_usage == HgiShaderStage::COMPUTE {
                HgiVulkanConversions::get_shader_stages(buf_desc.stage_usage)
            } else {
                buffer_stages
            };

            layout_bindings.push(
                vk::DescriptorSetLayoutBinding::default()
                    .binding(buf_desc.binding_index)
                    .descriptor_type(HgiVulkanConversions::get_descriptor_type(
                        buf_desc.resource_type,
                    ))
                    .descriptor_count(buf_desc.buffers.len() as u32)
                    .stage_flags(stage_flags),
            );

            texture_bind_index_start = texture_bind_index_start.max(buf_desc.binding_index + 1);
        }

        // Textures
        for tex_desc in &desc.texture_bindings {
            let rt_index = tex_desc.resource_type as usize;
            pool_sizes[rt_index].descriptor_count += 1;

            let stage_flags = if tex_desc.stage_usage == HgiShaderStage::COMPUTE {
                HgiVulkanConversions::get_shader_stages(tex_desc.stage_usage)
            } else {
                texture_stages
            };

            layout_bindings.push(
                vk::DescriptorSetLayoutBinding::default()
                    .binding(texture_bind_index_start + tex_desc.binding_index)
                    .descriptor_type(HgiVulkanConversions::get_descriptor_type(
                        tex_desc.resource_type,
                    ))
                    .descriptor_count(tex_desc.textures.len() as u32)
                    .stage_flags(stage_flags),
            );
        }

        // ----------------------------------------------------------------
        // Create VkDescriptorSetLayout
        // ----------------------------------------------------------------
        let set_layout_info =
            vk::DescriptorSetLayoutCreateInfo::default().bindings(&layout_bindings);

        let vk_descriptor_set_layout = unsafe {
            device
                .create_descriptor_set_layout(&set_layout_info, None)
                .map_err(|e| format!("vkCreateDescriptorSetLayout failed: {e:?}"))?
        };

        if !desc.debug_name.is_empty() {
            diagnostic::set_debug_name(
                debug_utils,
                vk_descriptor_set_layout.as_raw(),
                vk::ObjectType::DESCRIPTOR_SET_LAYOUT,
                &format!("DescriptorSetLayout {}", desc.debug_name),
            );
        }

        // ----------------------------------------------------------------
        // Create VkDescriptorPool (one pool per resource bindings object).
        // Ensure every pool entry has descriptorCount >= 1 to avoid
        // Vulkan validation errors on otherwise-empty pools.
        // ----------------------------------------------------------------
        for pool_size in &mut pool_sizes {
            pool_size.descriptor_count = pool_size.descriptor_count.max(1);
        }

        let pool_info = vk::DescriptorPoolCreateInfo::default()
            .flags(vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET)
            .max_sets(1)
            .pool_sizes(&pool_sizes);

        let vk_descriptor_pool = unsafe {
            device
                .create_descriptor_pool(&pool_info, None)
                .map_err(|e| {
                    // Clean up layout on failure.
                    device.destroy_descriptor_set_layout(vk_descriptor_set_layout, None);
                    format!("vkCreateDescriptorPool failed: {e:?}")
                })?
        };

        if !desc.debug_name.is_empty() {
            diagnostic::set_debug_name(
                debug_utils,
                vk_descriptor_pool.as_raw(),
                vk::ObjectType::DESCRIPTOR_POOL,
                &format!("Descriptor Pool {}", desc.debug_name),
            );
        }

        // ----------------------------------------------------------------
        // Allocate VkDescriptorSet from the pool
        // ----------------------------------------------------------------
        let layouts = [vk_descriptor_set_layout];
        let alloc_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(vk_descriptor_pool)
            .set_layouts(&layouts);

        let vk_descriptor_set = unsafe {
            device
                .allocate_descriptor_sets(&alloc_info)
                .map_err(|e| {
                    device.destroy_descriptor_pool(vk_descriptor_pool, None);
                    device.destroy_descriptor_set_layout(vk_descriptor_set_layout, None);
                    format!("vkAllocateDescriptorSets failed: {e:?}")
                })?
                .into_iter()
                .next()
                .ok_or_else(|| "vkAllocateDescriptorSets returned empty list".to_string())?
        };

        if !desc.debug_name.is_empty() {
            diagnostic::set_debug_name(
                debug_utils,
                vk_descriptor_set.as_raw(),
                vk::ObjectType::DESCRIPTOR_SET,
                &format!("Descriptor Set Buffers {}", desc.debug_name),
            );
        }

        // ----------------------------------------------------------------
        // Write descriptor set
        // ----------------------------------------------------------------
        let mut write_sets: Vec<vk::WriteDescriptorSet> = Vec::new();

        // Collect buffer infos first so slices into them remain valid.
        let mut buffer_infos: Vec<vk::DescriptorBufferInfo> = Vec::new();
        for buf_desc in &desc.buffer_bindings {
            for (i, buf_handle) in buf_desc.buffers.iter().enumerate() {
                let offset = buf_desc.offsets.get(i).copied().unwrap_or(0) as u64;
                // raw_resource() returns the VkBuffer handle as u64.
                let vk_buffer = vk::Buffer::from_raw(buf_handle.raw_resource());
                buffer_infos.push(
                    vk::DescriptorBufferInfo::default()
                        .buffer(vk_buffer)
                        .offset(offset)
                        .range(vk::WHOLE_SIZE),
                );
            }
        }

        let mut buf_info_offset: usize = 0;
        for buf_desc in &desc.buffer_bindings {
            let count = buf_desc.buffers.len();
            let write = vk::WriteDescriptorSet::default()
                .dst_set(vk_descriptor_set)
                .dst_binding(buf_desc.binding_index)
                .dst_array_element(0)
                .descriptor_type(HgiVulkanConversions::get_descriptor_type(
                    buf_desc.resource_type,
                ))
                .buffer_info(&buffer_infos[buf_info_offset..buf_info_offset + count]);
            write_sets.push(write);
            buf_info_offset += count;
        }

        // Collect image infos.
        let mut image_infos: Vec<vk::DescriptorImageInfo> = Vec::new();
        for tex_desc in &desc.texture_bindings {
            for (i, tex_handle) in tex_desc.textures.iter().enumerate() {
                let sampler = if let Some(smp_handle) = tex_desc.samplers.get(i) {
                    vk::Sampler::from_raw(smp_handle.raw_resource())
                } else {
                    vk::Sampler::null()
                };
                // raw_resource() returns VkImageView as u64 for textures.
                let image_view = vk::ImageView::from_raw(tex_handle.raw_resource());
                // Use SHADER_READ_ONLY_OPTIMAL as the default layout; callers
                // that use storage images must transition themselves.
                let image_layout = if tex_desc.writable {
                    vk::ImageLayout::GENERAL
                } else {
                    vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL
                };
                image_infos.push(
                    vk::DescriptorImageInfo::default()
                        .sampler(sampler)
                        .image_view(image_view)
                        .image_layout(image_layout),
                );
            }
        }

        let mut tex_info_offset: usize = 0;
        for tex_desc in &desc.texture_bindings {
            let count = tex_desc.textures.len();
            let write = vk::WriteDescriptorSet::default()
                .dst_set(vk_descriptor_set)
                .dst_binding(texture_bind_index_start + tex_desc.binding_index)
                .dst_array_element(0)
                .descriptor_type(HgiVulkanConversions::get_descriptor_type(
                    tex_desc.resource_type,
                ))
                .image_info(&image_infos[tex_info_offset..tex_info_offset + count]);
            write_sets.push(write);
            tex_info_offset += count;
        }

        // Immediate update — must not happen while the set is in use on GPU.
        if !write_sets.is_empty() {
            unsafe {
                device.update_descriptor_sets(&write_sets, &[]);
            }
        }

        Ok(Self {
            desc: desc.clone(),
            device: device.clone(),
            vk_descriptor_pool,
            vk_descriptor_set_layout,
            vk_descriptor_set,
            inflight_bits: 0,
        })
    }

    /// Records `vkCmdBindDescriptorSets` into `command_buffer`.
    ///
    /// Slot 0 is always used — Hgi does not expose per-set slot selection.
    /// Mirrors `HgiVulkanResourceBindings::BindResources()`.
    pub fn bind_resources(
        &self,
        command_buffer: vk::CommandBuffer,
        bind_point: vk::PipelineBindPoint,
        layout: vk::PipelineLayout,
    ) {
        unsafe {
            self.device.cmd_bind_descriptor_sets(
                command_buffer,
                bind_point,
                layout,
                0, // firstSet — always 0
                &[self.vk_descriptor_set],
                &[], // no dynamic offsets
            );
        }
    }

    /// Returns the underlying `VkDescriptorSet` handle.
    pub fn vk_descriptor_set(&self) -> vk::DescriptorSet {
        self.vk_descriptor_set
    }

    /// Returns the in-flight command-buffer generation bits.
    pub fn inflight_bits(&self) -> u64 {
        self.inflight_bits
    }

    /// Sets the in-flight generation bits (used by the garbage collector).
    pub fn set_inflight_bits(&mut self, bits: u64) {
        self.inflight_bits = bits;
    }
}

impl Drop for HgiVulkanResourceBindings {
    fn drop(&mut self) {
        unsafe {
            // Layout first (matches C++ destructor order).
            self.device
                .destroy_descriptor_set_layout(self.vk_descriptor_set_layout, None);
            // Destroying the pool implicitly frees all sets allocated from it.
            self.device
                .destroy_descriptor_pool(self.vk_descriptor_pool, None);
        }
    }
}

impl HgiResourceBindings for HgiVulkanResourceBindings {
    fn descriptor(&self) -> &HgiResourceBindingsDesc {
        &self.desc
    }

    fn raw_resource(&self) -> u64 {
        self.vk_descriptor_set.as_raw()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Converts a 0-based index into the corresponding `HgiBindResourceType`.
///
/// Must stay in sync with the enum discriminant order.
fn index_to_bind_resource_type(index: usize) -> HgiBindResourceType {
    match index {
        0 => HgiBindResourceType::Sampler,
        1 => HgiBindResourceType::SampledImage,
        2 => HgiBindResourceType::CombinedSamplerImage,
        3 => HgiBindResourceType::StorageImage,
        4 => HgiBindResourceType::UniformBuffer,
        5 => HgiBindResourceType::StorageBuffer,
        6 => HgiBindResourceType::TessFactors,
        _ => {
            log::error!("index_to_bind_resource_type: out-of-range index {index}");
            HgiBindResourceType::UniformBuffer
        }
    }
}
