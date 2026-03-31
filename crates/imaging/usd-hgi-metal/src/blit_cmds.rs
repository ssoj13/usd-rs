//! Metal blit/copy commands. Port of pxr/imaging/hgiMetal/blitCmds

use usd_hgi::*;

/// Metal blit/copy command buffer.
/// Mirrors C++ HgiMetalBlitCmds.
#[derive(Debug)]
pub struct HgiMetalBlitCmds {
    submitted: std::sync::atomic::AtomicBool,
    // On real Metal:
    // hgi: *mut HgiMetal,
    // command_buffer: id<MTLCommandBuffer>,
    // blit_encoder: id<MTLBlitCommandEncoder>,
    // label: String,
    // secondary_command_buffer: bool,
}

impl HgiMetalBlitCmds {
    /// Creates a new Metal blit command buffer.
    /// On real Metal, takes HgiMetal* to access device/queue.
    pub fn new() -> Self {
        Self {
            submitted: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

impl Default for HgiMetalBlitCmds {
    fn default() -> Self {
        Self::new()
    }
}

impl HgiCmds for HgiMetalBlitCmds {
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

impl HgiBlitCmds for HgiMetalBlitCmds {
    fn copy_buffer_cpu_to_gpu(&mut self, _op: &HgiBufferCpuToGpuOp) {
        // Stub: on real Metal, creates blit encoder if needed,
        // copies CPU data to MTLBuffer via memcpy or blit
    }
    fn copy_buffer_gpu_to_gpu(&mut self, _op: &HgiBufferGpuToGpuOp) {
        // Stub: on real Metal, uses [blitEncoder copyFromBuffer:toBuffer:]
    }
    fn copy_buffer_gpu_to_cpu(&mut self, _op: &HgiBufferGpuToCpuOp) {
        // Stub: on real Metal, syncs managed buffer and reads back
    }
    fn copy_texture_cpu_to_gpu(&mut self, _op: &HgiTextureCpuToGpuOp) {
        // Stub: on real Metal, uses [texture replaceRegion:...] or blit
    }
    fn copy_texture_gpu_to_gpu(&mut self, _op: &HgiTextureGpuToGpuOp) {
        // Stub: on real Metal, uses [blitEncoder copyFromTexture:toTexture:]
    }
    fn copy_texture_gpu_to_cpu(&mut self, _op: &HgiTextureGpuToCpuOp) {
        // Stub: on real Metal, uses [blitEncoder copyFromTexture:toBuffer:]
    }
    fn copy_buffer_to_texture(&mut self, _op: &HgiBufferToTextureOp) {
        // Stub: on real Metal, uses [blitEncoder copyFromBuffer:toTexture:]
    }
    fn copy_texture_to_buffer(&mut self, _op: &HgiTextureToBufferOp) {
        // Stub: on real Metal, uses [blitEncoder copyFromTexture:toBuffer:]
    }
    fn generate_mipmap(&mut self, _texture: &HgiTextureHandle) {
        // Stub: on real Metal, uses [blitEncoder generateMipmapsForTexture:]
    }
    fn fill_buffer(&mut self, _buffer: &HgiBufferHandle, _value: u8) {
        // Stub: on real Metal, uses [blitEncoder fillBuffer:range:value:]
    }
}
