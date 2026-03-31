//! Metal compute pipeline. Port of pxr/imaging/hgiMetal/computePipeline

use usd_hgi::{HgiComputePipeline, HgiComputePipelineDesc};

/// Metal compute pipeline state.
/// Mirrors C++ HgiMetalComputePipeline.
#[derive(Debug)]
pub struct HgiMetalComputePipeline {
    desc: HgiComputePipelineDesc,
    // On real Metal: compute_pipeline_state: id<MTLComputePipelineState>
}

impl HgiMetalComputePipeline {
    /// Creates a new Metal compute pipeline from the given descriptor.
    /// On real Metal, this would create via [device newComputePipelineStateWithFunction:error:].
    pub fn new(desc: HgiComputePipelineDesc) -> Self {
        Self { desc }
    }

    /// Apply pipeline state to a compute command encoder.
    /// Mirrors C++ BindPipeline(id<MTLComputeCommandEncoder>).
    /// Stub: requires Metal compute command encoder.
    pub fn bind_pipeline(&self) {
        // Stub: on real Metal, calls [encoder setComputePipelineState:_computePipelineState]
    }

    /// Returns the Metal compute pipeline state.
    /// Mirrors C++ GetMetalPipelineState().
    /// Stub: returns 0.
    pub fn get_metal_pipeline_state(&self) -> u64 {
        0
    }
}

impl HgiComputePipeline for HgiMetalComputePipeline {
    fn descriptor(&self) -> &HgiComputePipelineDesc {
        &self.desc
    }
    fn raw_resource(&self) -> u64 {
        self.get_metal_pipeline_state()
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
