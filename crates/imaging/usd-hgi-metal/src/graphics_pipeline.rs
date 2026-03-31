//! Metal graphics pipeline. Port of pxr/imaging/hgiMetal/graphicsPipeline

use usd_hgi::{HgiGraphicsPipeline, HgiGraphicsPipelineDesc};

/// Metal graphics pipeline state.
/// Mirrors C++ HgiMetalGraphicsPipeline.
#[derive(Debug)]
pub struct HgiMetalGraphicsPipeline {
    desc: HgiGraphicsPipelineDesc,
    // On real Metal:
    // vertex_descriptor: MTLVertexDescriptor,
    // depth_stencil_state: id<MTLDepthStencilState>,
    // render_pipeline_state: id<MTLRenderPipelineState>,
    // constant_tess_factors: id<MTLBuffer>,
}

impl HgiMetalGraphicsPipeline {
    /// Creates a new Metal graphics pipeline from the given descriptor.
    /// On real Metal, this would:
    /// 1. _CreateVertexDescriptor() - build MTLVertexDescriptor from desc
    /// 2. _CreateDepthStencilState() - create MTLDepthStencilState
    /// 3. _CreateRenderPipelineState() - create MTLRenderPipelineState
    pub fn new(desc: HgiGraphicsPipelineDesc) -> Self {
        Self { desc }
    }

    /// Apply pipeline state to a render command encoder.
    /// Mirrors C++ BindPipeline(id<MTLRenderCommandEncoder>).
    /// Stub: requires Metal render command encoder.
    pub fn bind_pipeline(&self) {
        // Stub: on real Metal, calls:
        // [encoder setRenderPipelineState:_renderPipelineState]
        // [encoder setDepthStencilState:_depthStencilState]
        // [encoder setCullMode:...]
        // [encoder setTriangleFillMode:...]
        // [encoder setFrontFacingWinding:...]
    }
}

impl HgiGraphicsPipeline for HgiMetalGraphicsPipeline {
    fn descriptor(&self) -> &HgiGraphicsPipelineDesc {
        &self.desc
    }
    fn raw_resource(&self) -> u64 {
        0
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
