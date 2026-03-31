//! Metal graphics commands. Port of pxr/imaging/hgiMetal/graphicsCmds

use usd_gf::Vec4f;
use usd_hgi::*;

/// Cached encoder state for Metal graphics commands.
/// Mirrors C++ HgiMetalGraphicsCmds::CachedEncoderState.
#[derive(Debug, Default)]
pub struct CachedEncoderState {
    pub viewport_set: bool,
    pub scissor_set: bool,
    pub viewport: [f64; 4],
    pub scissor: [u32; 4],
}

impl CachedEncoderState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.viewport_set = false;
        self.scissor_set = false;
        self.viewport = [0.0; 4];
        self.scissor = [0; 4];
    }
}

/// Metal graphics command buffer.
/// Mirrors C++ HgiMetalGraphicsCmds.
#[derive(Debug)]
pub struct HgiMetalGraphicsCmds {
    submitted: std::sync::atomic::AtomicBool,
    cached_state: CachedEncoderState,
    primitive_type: HgiPrimitiveType,
    enable_parallel_encoder: bool,
    max_num_encoders: u32,
    draw_buffer_binding_index: u32,
    // On real Metal:
    // hgi: *mut HgiMetal,
    // render_pass_descriptor: MTLRenderPassDescriptor,
    // parallel_encoder: id<MTLParallelRenderCommandEncoder>,
    // encoders: Vec<id<MTLRenderCommandEncoder>>,
    // argument_buffer: id<MTLBuffer>,
    // descriptor: HgiGraphicsCmdsDesc,
    // step_functions: HgiMetalStepFunctions,
}

impl HgiMetalGraphicsCmds {
    /// Creates a new Metal graphics command buffer.
    /// On real Metal, takes HgiMetal* and HgiGraphicsCmdsDesc.
    pub fn new() -> Self {
        Self {
            submitted: std::sync::atomic::AtomicBool::new(false),
            cached_state: CachedEncoderState::new(),
            primitive_type: HgiPrimitiveType::TriangleList,
            enable_parallel_encoder: false,
            max_num_encoders: 1,
            draw_buffer_binding_index: 0,
        }
    }

    /// Enable or disable parallel render command encoder.
    /// Mirrors C++ EnableParallelEncoder(bool).
    pub fn enable_parallel_encoder(&mut self, enable: bool) {
        self.enable_parallel_encoder = enable;
    }

    /// Get the render command encoder at the given index.
    /// Mirrors C++ GetEncoder(uint32_t encoderIndex).
    /// Stub: returns 0 (no real encoder).
    pub fn get_encoder(&self, _encoder_index: u32) -> u64 {
        0
    }
}

impl Default for HgiMetalGraphicsCmds {
    fn default() -> Self {
        Self::new()
    }
}

impl HgiCmds for HgiMetalGraphicsCmds {
    fn is_submitted(&self) -> bool {
        self.submitted.load(std::sync::atomic::Ordering::SeqCst)
    }
    fn push_debug_group(&mut self, _label: &str) {
        // Stub: on real Metal, calls [encoder pushDebugGroup:label]
    }
    fn pop_debug_group(&mut self) {
        // Stub: on real Metal, calls [encoder popDebugGroup]
    }
    fn insert_debug_marker(&mut self, _label: &str) {
        // Stub: on real Metal, calls [encoder insertDebugSignpost:label]
    }
}

impl HgiGraphicsCmds for HgiMetalGraphicsCmds {
    fn bind_pipeline(&mut self, _pipeline: &HgiGraphicsPipelineHandle) {
        // Stub: on real Metal, stores pipeline, calls BindPipeline on encoder
    }
    fn bind_resources(&mut self, _resources: &HgiResourceBindingsHandle) {
        // Stub: on real Metal, calls BindResources on the resource bindings
    }
    fn set_constant_values(
        &mut self,
        _pipeline: &HgiGraphicsPipelineHandle,
        _stages: HgiShaderStage,
        _bind_index: u32,
        _data: &[u8],
    ) {
        // Stub: on real Metal, writes constants into argument buffer
        // via HgiMetalResourceBindings::SetConstantValues
    }
    fn bind_vertex_buffers(&mut self, _buffers: &[HgiBufferHandle], _offsets: &[u64]) {
        // Stub: on real Metal, calls [encoder setVertexBuffer:offset:atIndex:]
        // and updates step functions
    }
    fn set_viewport(&mut self, viewport: &HgiViewport) {
        self.cached_state.viewport_set = true;
        self.cached_state.viewport = [
            viewport.x as f64,
            viewport.y as f64,
            viewport.width as f64,
            viewport.height as f64,
        ];
    }
    fn set_scissor(&mut self, scissor: &HgiScissor) {
        self.cached_state.scissor_set = true;
        self.cached_state.scissor = [
            scissor.x as u32,
            scissor.y as u32,
            scissor.width,
            scissor.height,
        ];
    }
    fn set_blend_constant_color(&mut self, _color: &Vec4f) {
        // Stub: on real Metal, calls [encoder setBlendColorRed:green:blue:alpha:]
    }
    fn set_stencil_reference_value(&mut self, _value: u32) {
        // Stub: on real Metal, calls [encoder setStencilReferenceValue:]
    }
    fn draw(&mut self, _op: &HgiDrawOp) {
        // Stub: on real Metal, calls [encoder drawPrimitives:vertexStart:vertexCount:instanceCount:baseInstance:]
    }
    fn draw_indexed(&mut self, _index_buffer: &HgiBufferHandle, _op: &HgiDrawIndexedOp) {
        // Stub: on real Metal, calls [encoder drawIndexedPrimitives:...]
    }
    fn draw_indirect(&mut self, _op: &HgiDrawIndirectOp) {
        // Stub: on real Metal, iterates draw count and calls
        // [encoder drawPrimitives:indirectBuffer:indirectBufferOffset:]
    }
    fn draw_indexed_indirect(&mut self, _index_buffer: &HgiBufferHandle, _op: &HgiDrawIndirectOp) {
        // Stub: on real Metal, iterates draw count and calls
        // [encoder drawIndexedPrimitives:indexBuffer:...indirectBuffer:...]
    }
    fn memory_barrier(&mut self, _barrier: HgiMemoryBarrier) {
        // Stub: on real Metal, calls [encoder memoryBarrierWithScope:...]
    }
}
