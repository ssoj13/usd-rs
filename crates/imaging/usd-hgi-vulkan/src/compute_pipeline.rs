//! Vulkan compute pipeline.
//!
//! Port of pxr/imaging/hgiVulkan/computePipeline.cpp/.h

#![allow(unsafe_code)]

use std::ffi::CString;

use ash::vk;
use ash::vk::Handle;
use usd_hgi::{HgiComputePipeline, HgiComputePipelineDesc};

use crate::descriptor_set_layouts::make_descriptor_set_layouts;
use crate::shader_function::HgiVulkanShaderFunction;

/// Vulkan compute pipeline — owns the VkPipeline, VkPipelineLayout, and
/// all VkDescriptorSetLayouts created for it.
///
/// Port of C++ `HgiVulkanComputePipeline`.
pub struct HgiVulkanComputePipeline {
    desc: HgiComputePipelineDesc,
    /// Logical device that owns all Vulkan objects stored here.
    device: ash::Device,
    vk_pipeline: vk::Pipeline,
    vk_pipeline_layout: vk::PipelineLayout,
    /// One layout per descriptor set declared by the compute shader.
    vk_descriptor_set_layouts: Vec<vk::DescriptorSetLayout>,
    /// Tracks which in-flight command buffer submissions reference this
    /// pipeline; used by the garbage collector.
    inflight_bits: u64,
}

impl HgiVulkanComputePipeline {
    /// Creates a Vulkan compute pipeline from `desc`.
    ///
    /// Steps (matching C++ constructor):
    /// 1. Obtain the first shader function from the program.
    /// 2. Downcast to `HgiVulkanShaderFunction` to get the VkShaderModule
    ///    and descriptor-set reflection data.
    /// 3. Build descriptor set layouts via `make_descriptor_set_layouts`.
    /// 4. Optionally build a push-constant range when `shader_constants_desc`
    ///    has a non-zero `byte_size`.
    /// 5. Create `VkPipelineLayout`.
    /// 6. Create `VkComputePipelineCreateInfo` and call
    ///    `vkCreateComputePipelines`.
    ///
    /// # Errors
    /// Returns a `String` describing the failure when any Vulkan call fails or
    /// the descriptor is missing required data.
    pub fn new(
        device: &ash::Device,
        pipeline_cache: vk::PipelineCache,
        desc: &HgiComputePipelineDesc,
    ) -> Result<Self, String> {
        // --- 1. Obtain compute shader function ---
        let program = desc
            .shader_program
            .get()
            .ok_or_else(|| "HgiVulkanComputePipeline: null shader program".to_string())?;

        let shader_functions = &program.descriptor().shader_functions;
        if shader_functions.is_empty() {
            return Err("HgiVulkanComputePipeline: shader program has no functions".to_string());
        }

        // --- 2. Downcast to HgiVulkanShaderFunction ---
        let vk_shader_fn = shader_functions[0]
            .get()
            .and_then(|f| f.as_any().downcast_ref::<HgiVulkanShaderFunction>())
            .ok_or_else(|| {
                "HgiVulkanComputePipeline: shader function is not HgiVulkanShaderFunction"
                    .to_string()
            })?;

        let shader_module = vk_shader_fn.vk_shader_module();
        // Build a CString for the entry point name; always "main" in practice.
        let entry_name = CString::new(vk_shader_fn.shader_function_name()).map_err(|e| {
            format!(
                "HgiVulkanComputePipeline: entry point name contains interior nul: {}",
                e
            )
        })?;

        // --- 3. Build descriptor set layouts from shader reflection data ---
        let set_info = vk_shader_fn.descriptor_set_info();
        let vk_descriptor_set_layouts =
            make_descriptor_set_layouts(device, &[set_info.to_vec()], &desc.debug_name).map_err(
                |e| {
                    format!(
                        "HgiVulkanComputePipeline: vkCreateDescriptorSetLayout failed: {:?}",
                        e
                    )
                },
            )?;

        // --- 4. Optional push-constant range ---
        let use_push_constants = desc.shader_constants_desc.byte_size > 0;
        let pc_range = vk::PushConstantRange {
            stage_flags: vk::ShaderStageFlags::COMPUTE,
            offset: 0,
            size: desc.shader_constants_desc.byte_size,
        };

        // --- 5. Create VkPipelineLayout ---
        // Collect push-constant ranges into a local slice so the borrow lives
        // long enough to pass to the create_info builder.
        let pc_ranges: &[vk::PushConstantRange] = if use_push_constants {
            std::slice::from_ref(&pc_range)
        } else {
            &[]
        };

        let layout_create_info = vk::PipelineLayoutCreateInfo::default()
            .set_layouts(&vk_descriptor_set_layouts)
            .push_constant_ranges(pc_ranges);

        // SAFETY: layout_create_info, vk_descriptor_set_layouts, and pc_ranges
        // are all stack-allocated and valid for the duration of this call.
        let vk_pipeline_layout = unsafe {
            device
                .create_pipeline_layout(&layout_create_info, None)
                .map_err(|e| {
                    format!(
                        "HgiVulkanComputePipeline: vkCreatePipelineLayout failed: {:?}",
                        e
                    )
                })?
        };

        if !desc.debug_name.is_empty() {
            log::debug!(
                "PipelineLayout {}: created {:?}",
                desc.debug_name,
                vk_pipeline_layout
            );
        }

        // --- 6. Create VkComputePipeline ---
        let stage_info = vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::COMPUTE)
            .module(shader_module)
            .name(&entry_name);

        let pipe_create_info = vk::ComputePipelineCreateInfo::default()
            .stage(stage_info)
            .layout(vk_pipeline_layout);

        // SAFETY: pipe_create_info, stage_info, and entry_name are all valid
        // and live past this call.
        let pipelines = unsafe {
            device
                .create_compute_pipelines(
                    pipeline_cache,
                    std::slice::from_ref(&pipe_create_info),
                    None,
                )
                .map_err(|(_, e)| {
                    format!(
                        "HgiVulkanComputePipeline: vkCreateComputePipelines failed: {:?}",
                        e
                    )
                })?
        };
        let vk_pipeline = pipelines[0];

        if !desc.debug_name.is_empty() {
            log::debug!("Pipeline {}: created {:?}", desc.debug_name, vk_pipeline);
        }

        Ok(Self {
            desc: desc.clone(),
            device: device.clone(),
            vk_pipeline,
            vk_pipeline_layout,
            vk_descriptor_set_layouts,
            inflight_bits: 0,
        })
    }

    /// Records `vkCmdBindPipeline(COMPUTE_BIT)` into `command_buffer`.
    ///
    /// Mirrors `HgiVulkanComputePipeline::BindPipeline`.
    pub fn bind_pipeline(&self, command_buffer: vk::CommandBuffer) {
        // SAFETY: command_buffer is in recording state; vk_pipeline is valid.
        unsafe {
            self.device.cmd_bind_pipeline(
                command_buffer,
                vk::PipelineBindPoint::COMPUTE,
                self.vk_pipeline,
            );
        }
    }

    /// Returns the `VkPipelineLayout` for this pipeline.
    ///
    /// Used by `HgiVulkanResourceBindings` to bind descriptor sets.
    pub fn vk_pipeline_layout(&self) -> vk::PipelineLayout {
        self.vk_pipeline_layout
    }

    /// Returns the underlying `VkPipeline` handle.
    pub fn vk_pipeline(&self) -> vk::Pipeline {
        self.vk_pipeline
    }

    /// Returns the current in-flight bits tracked by the garbage collector.
    pub fn inflight_bits(&self) -> u64 {
        self.inflight_bits
    }

    /// Sets the in-flight bits (called by the garbage collector).
    pub fn set_inflight_bits(&mut self, bits: u64) {
        self.inflight_bits = bits;
    }
}

impl Drop for HgiVulkanComputePipeline {
    fn drop(&mut self) {
        // Destroy in reverse creation order — descriptor set layouts first,
        // then the pipeline layout, then the pipeline.  Mirrors C++ destructor.
        //
        // SAFETY: all handles were created by `self.device` and have not been
        // destroyed yet.
        unsafe {
            for &layout in &self.vk_descriptor_set_layouts {
                self.device.destroy_descriptor_set_layout(layout, None);
            }
            self.device
                .destroy_pipeline_layout(self.vk_pipeline_layout, None);
            self.device.destroy_pipeline(self.vk_pipeline, None);
        }
    }
}

impl HgiComputePipeline for HgiVulkanComputePipeline {
    fn descriptor(&self) -> &HgiComputePipelineDesc {
        &self.desc
    }

    /// Returns the `VkPipeline` handle cast to `u64`, matching the C++
    /// `GetRawResource()` contract for Vulkan backends.
    fn raw_resource(&self) -> u64 {
        self.vk_pipeline.as_raw()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
