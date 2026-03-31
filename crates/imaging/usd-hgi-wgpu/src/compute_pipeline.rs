//! wgpu compute pipeline implementation for HGI.
//!
//! Maps HgiComputePipelineDesc to wgpu::ComputePipeline.
//! The compute shader module is extracted from the shader program.

use usd_hgi::compute_pipeline::{HgiComputePipeline, HgiComputePipelineDesc};
use usd_hgi::shader_function::HgiShaderFunction;

use super::shader_function::WgpuShaderFunction;

use super::graphics_pipeline::MAX_PUSH_CONSTANTS_SIZE;

/// wgpu-backed compute pipeline.
///
/// Wraps a wgpu::ComputePipeline created from a single compute shader
/// module and bind group layouts for all bind groups used by the shader.
pub struct WgpuComputePipeline {
    desc: HgiComputePipelineDesc,
    /// None when created via new_stub (HGI trait path without shader module)
    pipeline: Option<wgpu::ComputePipeline>,
    /// Bind group layouts for all groups used by this compute shader (P1-2 fix).
    #[allow(dead_code)]
    bind_group_layouts: Vec<wgpu::BindGroupLayout>,
}

impl WgpuComputePipeline {
    /// Create a new wgpu compute pipeline.
    ///
    /// `compute_module` must be the compute shader function from the
    /// shader program referenced in `desc`.
    /// Uses wgpu auto-layout (layout: None) to infer bind group layout from shader.
    ///
    /// Returns a pipeline with `pipeline: None` if creation fails (invalid shader, etc.)
    pub fn new(
        device: &wgpu::Device,
        desc: &HgiComputePipelineDesc,
        compute_module: &WgpuShaderFunction,
    ) -> Self {
        let label = if desc.debug_name.is_empty() {
            None
        } else {
            Some(desc.debug_name.as_str())
        };

        let entry_point = &compute_module.descriptor().entry_point;

        // Two-pass: auto-layout probe -> extract bind group layout -> explicit layout with push constants
        // Use push_error_scope to suppress Vulkan validation noise from probe pipeline
        // (auto-layout doesn't know about push constants, so validation may warn)
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let temp_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("temp_compute_layout_probe"),
            layout: None,
            module: compute_module.wgpu_module(),
            entry_point: Some(entry_point.as_str()),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
        let probe_err = pollster::block_on(device.pop_error_scope());
        let temp_result: Result<wgpu::ComputePipeline, ()> = if probe_err.is_some() {
            Err(())
        } else {
            Ok(temp_pipeline)
        };

        // Extract bind group layouts for all groups (not just group 0) — P1-2 fix.
        // Also preserve sparse group indices by filling gaps with empty BGLs.
        let bind_group_layouts: Vec<wgpu::BindGroupLayout> = {
            let mut bgls: Vec<Option<wgpu::BindGroupLayout>> = vec![None, None, None, None];
            if let Ok(ref temp) = temp_result {
                for group_idx in 0..4u32 {
                    device.push_error_scope(wgpu::ErrorFilter::Validation);
                    let bgl = temp.get_bind_group_layout(group_idx);
                    let err = pollster::block_on(device.pop_error_scope());
                    if err.is_none() {
                        bgls[group_idx as usize] = Some(bgl);
                    }
                }
            }
            // Preserve sparse indices: fill gaps with empty BGLs.
            let max_used = bgls.iter().rposition(|b| b.is_some());
            match max_used {
                None => Vec::new(),
                Some(max_idx) => {
                    let mut out = Vec::with_capacity(max_idx + 1);
                    for bgl_opt in bgls.into_iter().take(max_idx + 1) {
                        match bgl_opt {
                            Some(bgl) => out.push(bgl),
                            None => out.push(device.create_bind_group_layout(
                                &wgpu::BindGroupLayoutDescriptor {
                                    label: Some("HgiWgpu empty compute BGL gap"),
                                    entries: &[],
                                },
                            )),
                        }
                    }
                    out
                }
            }
        };

        // Build explicit pipeline layout with push constants
        let push_constant_ranges = [wgpu::PushConstantRange {
            stages: wgpu::ShaderStages::COMPUTE,
            range: 0..MAX_PUSH_CONSTANTS_SIZE,
        }];

        let bgl_refs: Vec<&wgpu::BindGroupLayout> = bind_group_layouts.iter().collect();
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("HgiWgpu compute pipeline layout"),
            bind_group_layouts: &bgl_refs,
            push_constant_ranges: &push_constant_ranges,
        });

        // Create real pipeline with explicit layout (push constants now enabled)
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let final_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label,
            layout: Some(&pipeline_layout),
            module: compute_module.wgpu_module(),
            entry_point: Some(entry_point.as_str()),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
        let pass2_err = pollster::block_on(device.pop_error_scope());

        let pipeline = if let Some(e) = pass2_err {
            log::error!(
                "Failed to create compute pipeline '{}': {}",
                desc.debug_name,
                e
            );
            None
        } else {
            Some(final_pipeline)
        };

        Self {
            desc: desc.clone(),
            pipeline,
            bind_group_layouts: bind_group_layouts,
        }
    }

    /// Create a stub pipeline without a shader (for HGI trait path).
    pub fn new_stub(desc: &HgiComputePipelineDesc) -> Self {
        Self {
            desc: desc.clone(),
            pipeline: None,
            bind_group_layouts: Vec::new(),
        }
    }

    /// Access the inner wgpu::ComputePipeline for command encoding.
    ///
    /// Returns None for stub pipelines (created with new_stub() when no shader is available).
    pub(crate) fn wgpu_pipeline(&self) -> Option<&wgpu::ComputePipeline> {
        self.pipeline.as_ref()
    }

    /// Get the first bind group layout (group 0) derived from shader reflection.
    #[allow(dead_code)] // Used by HdSt integration
    pub(crate) fn bind_group_layout(&self) -> Option<&wgpu::BindGroupLayout> {
        self.bind_group_layouts.first()
    }

    /// Get all bind group layouts (all groups) derived from shader reflection.
    #[allow(dead_code)]
    pub(crate) fn bind_group_layouts(&self) -> &[wgpu::BindGroupLayout] {
        &self.bind_group_layouts
    }
}

impl HgiComputePipeline for WgpuComputePipeline {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn descriptor(&self) -> &HgiComputePipelineDesc {
        &self.desc
    }

    fn raw_resource(&self) -> u64 {
        // wgpu 24 removed global_id(); return 0 as placeholder
        0
    }
}
