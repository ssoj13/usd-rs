//! Metal compute commands. Port of pxr/imaging/hgiMetal/computeCmds

use usd_hgi::*;

/// Metal compute command buffer.
/// Mirrors C++ HgiMetalComputeCmds.
#[derive(Debug)]
pub struct HgiMetalComputeCmds {
    submitted: std::sync::atomic::AtomicBool,
    dispatch_method: HgiComputeDispatch,
    // On real Metal:
    // hgi: *mut HgiMetal,
    // pipeline_state: *mut HgiMetalComputePipeline,
    // command_buffer: id<MTLCommandBuffer>,
    // argument_buffer: id<MTLBuffer>,
    // encoder: id<MTLComputeCommandEncoder>,
    // secondary_command_buffer: bool,
}

impl HgiMetalComputeCmds {
    /// Creates a new Metal compute command buffer.
    /// On real Metal, takes HgiMetal* and HgiComputeCmdsDesc.
    pub fn new() -> Self {
        Self {
            submitted: std::sync::atomic::AtomicBool::new(false),
            dispatch_method: HgiComputeDispatch::Serial,
        }
    }

    /// Creates with a specific dispatch method.
    pub fn with_dispatch(dispatch_method: HgiComputeDispatch) -> Self {
        Self {
            submitted: std::sync::atomic::AtomicBool::new(false),
            dispatch_method,
        }
    }

    /// Returns the compute command encoder.
    /// Mirrors C++ GetEncoder().
    /// Stub: returns 0 (no real encoder).
    pub fn get_encoder(&self) -> u64 {
        0
    }
}

impl Default for HgiMetalComputeCmds {
    fn default() -> Self {
        Self::new()
    }
}

impl HgiCmds for HgiMetalComputeCmds {
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

impl HgiComputeCmds for HgiMetalComputeCmds {
    fn bind_pipeline(&mut self, _pipeline: &HgiComputePipelineHandle) {
        // Stub: on real Metal, stores pipeline and calls BindPipeline on encoder
    }
    fn bind_resources(&mut self, _resources: &HgiResourceBindingsHandle) {
        // Stub: on real Metal, calls BindResources on the resource bindings
    }
    fn set_constant_values(
        &mut self,
        _pipeline: &HgiComputePipelineHandle,
        _bind_index: u32,
        _data: &[u8],
    ) {
        // Stub: on real Metal, writes constants into argument buffer
        // via HgiMetalResourceBindings::SetConstantValues
    }
    fn dispatch(&mut self, _op: &HgiComputeDispatchOp) {
        // Stub: on real Metal, calls [encoder dispatchThreadgroups:threadsPerThreadgroup:]
    }
    fn memory_barrier(&mut self, _barrier: HgiMemoryBarrier) {
        // Stub: on real Metal, calls [encoder memoryBarrierWithScope:]
    }
    fn get_dispatch_method(&self) -> HgiComputeDispatch {
        self.dispatch_method
    }
}
