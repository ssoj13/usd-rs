//! Metal buffer resource. Port of pxr/imaging/hgiMetal/buffer

use usd_hgi::{HgiBuffer, HgiBufferDesc};

/// Metal-backed GPU buffer resource.
/// Mirrors C++ HgiMetalBuffer.
#[derive(Debug)]
pub struct HgiMetalBuffer {
    desc: HgiBufferDesc,
    // On real Metal: buffer_id: id<MTLBuffer>
}

impl HgiMetalBuffer {
    /// Creates a new Metal buffer from the given descriptor.
    /// On real Metal, this would allocate via [device newBufferWithLength:options:].
    pub fn new(desc: HgiBufferDesc) -> Self {
        Self { desc }
    }

    /// Returns the Metal buffer handle.
    /// Mirrors C++ GetBufferId().
    /// Stub: returns 0 (no real Metal buffer).
    pub fn get_buffer_id(&self) -> u64 {
        0
    }
}

impl HgiBuffer for HgiMetalBuffer {
    fn descriptor(&self) -> &HgiBufferDesc {
        &self.desc
    }
    fn byte_size_of_resource(&self) -> usize {
        self.desc.byte_size
    }
    fn raw_resource(&self) -> u64 {
        self.get_buffer_id()
    }
    fn cpu_staging_address(&mut self) -> Option<*mut u8> {
        // On real Metal with shared storage mode, would return buffer.contents
        None
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
