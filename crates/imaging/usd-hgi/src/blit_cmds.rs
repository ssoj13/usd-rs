//! Blit (copy) command buffer interface

use super::buffer::HgiBufferHandle;
use super::cmds::HgiCmds;
use super::enums::HgiMemoryBarrier;
use super::texture::HgiTextureHandle;
use usd_gf::Vec3i;

/// Safe wrapper for raw pointers that indicates the pointer must remain
/// valid for the lifetime of the operation.
///
/// # Safety
///
/// The caller must ensure the pointed-to data remains valid and unchanged
/// for the duration of the operation (until command submission completes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawCpuBuffer(*const u8);

impl RawCpuBuffer {
    /// Create a new CPU buffer pointer.
    ///
    /// The caller must ensure the pointer remains valid and unchanged
    /// for the duration of any operation that uses this buffer.
    pub fn new(ptr: *const u8) -> Self {
        Self(ptr)
    }

    /// Get the underlying pointer.
    pub fn as_ptr(&self) -> *const u8 {
        self.0
    }

    /// Create a null buffer.
    pub fn null() -> Self {
        Self(std::ptr::null())
    }
}

// SAFETY: The GPU copy operations that use these pointers are single-threaded
// in the OpenGL context. The data is only read synchronously during command execution.
// Caller ensures buffer validity for operation duration.
#[allow(unsafe_code)]
unsafe impl Send for RawCpuBuffer {}
#[allow(unsafe_code)]
unsafe impl Sync for RawCpuBuffer {}

/// Safe wrapper for mutable raw pointers (for GPU->CPU copies).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawCpuBufferMut(*mut u8);

impl RawCpuBufferMut {
    /// Create a new mutable CPU buffer pointer.
    ///
    /// # Safety
    ///
    /// The caller must ensure the pointer remains valid for the duration
    /// of any operation that uses this buffer.
    #[allow(unsafe_code)] // SAFETY: Wrapper for GPU download pointer
    pub unsafe fn new(ptr: *mut u8) -> Self {
        Self(ptr)
    }

    /// Get the underlying pointer.
    pub fn as_ptr(&self) -> *mut u8 {
        self.0
    }

    /// Create a null buffer.
    pub fn null() -> Self {
        Self(std::ptr::null_mut())
    }
}

// SAFETY: Same as RawCpuBuffer - GPU operations are synchronized
#[allow(unsafe_code)]
unsafe impl Send for RawCpuBufferMut {}
#[allow(unsafe_code)]
unsafe impl Sync for RawCpuBufferMut {}

/// Region for buffer copy operations (C++ HgiBufferCpuToGpuOp)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HgiBufferCpuToGpuOp {
    /// CPU source data pointer (must remain valid until command submission)
    pub cpu_source_buffer: RawCpuBuffer,

    /// Byte offset into the CPU source buffer where copying starts
    /// (C++ `sourceByteOffset`)
    pub source_byte_offset: usize,

    /// GPU destination buffer
    pub gpu_destination_buffer: HgiBufferHandle,

    /// Offset into destination buffer (C++ `destinationByteOffset`)
    pub destination_byte_offset: usize,

    /// Size of data to copy in bytes (C++ `byteSize`)
    pub byte_size: usize,
}

/// GPU buffer to CPU buffer copy operation (read-back)
#[derive(Debug, Clone, PartialEq)]
pub struct HgiBufferGpuToCpuOp {
    /// Source GPU buffer
    pub gpu_source_buffer: HgiBufferHandle,

    /// Offset into source buffer
    pub source_byte_offset: usize,

    /// CPU destination buffer pointer (must remain valid until command submission)
    pub cpu_destination_buffer: RawCpuBufferMut,

    /// Size of data to copy in bytes
    pub byte_size: usize,
}

/// Buffer to buffer copy operation
#[derive(Debug, Clone, PartialEq)]
pub struct HgiBufferGpuToGpuOp {
    /// Source buffer
    pub gpu_source_buffer: HgiBufferHandle,

    /// Destination buffer
    pub gpu_destination_buffer: HgiBufferHandle,

    /// Offset into source buffer
    pub source_byte_offset: usize,

    /// Offset into destination buffer
    pub destination_byte_offset: usize,

    /// Size of data to copy in bytes
    pub byte_size: usize,
}

/// Texture copy region
#[derive(Debug, Clone, PartialEq)]
pub struct HgiTextureCpuToGpuOp {
    /// CPU source data pointer
    pub cpu_source_buffer: RawCpuBuffer,

    /// Size of source data in bytes
    pub buffer_byte_size: usize,

    /// Destination texture
    pub gpu_destination_texture: HgiTextureHandle,

    /// Destination region offset
    pub destination_texel_offset: Vec3i,

    /// Mip level to copy to
    pub mip_level: u32,

    /// Array layer to copy to (for texture arrays)
    pub destination_layer: u32,
}

/// GPU texture to GPU texture copy operation
#[derive(Debug, Clone, PartialEq)]
pub struct HgiTextureGpuToGpuOp {
    /// Source texture
    pub gpu_source_texture: HgiTextureHandle,

    /// Destination texture
    pub gpu_destination_texture: HgiTextureHandle,

    /// Source region offset
    pub source_texel_offset: Vec3i,

    /// Destination region offset
    pub destination_texel_offset: Vec3i,

    /// Size of region to copy
    pub copy_size: Vec3i,

    /// Source mip level
    pub source_mip_level: u32,

    /// Destination mip level
    pub destination_mip_level: u32,

    /// Source layer (for texture arrays)
    pub source_layer: u32,

    /// Destination layer (for texture arrays)
    pub destination_layer: u32,
}

/// GPU texture to CPU buffer copy operation (C++ HgiTextureGpuToCpuOp)
#[derive(Debug, Clone, PartialEq)]
pub struct HgiTextureGpuToCpuOp {
    /// Source texture (C++ `gpuSourceTexture`)
    pub gpu_source_texture: HgiTextureHandle,

    /// Source region offset (C++ `sourceTexelOffset`)
    pub source_texel_offset: Vec3i,

    /// Mip level to read from (C++ `mipLevel`)
    pub mip_level: u32,

    /// CPU destination buffer pointer (C++ `cpuDestinationBuffer`)
    pub cpu_destination_buffer: RawCpuBufferMut,

    /// Byte offset into the destination CPU buffer where data is written
    /// (C++ `destinationByteOffset`)
    pub destination_byte_offset: usize,

    /// Size of destination buffer in bytes (C++ `destinationBufferByteSize`)
    pub destination_buffer_byte_size: usize,

    /// Size of the region to copy
    pub copy_size: Vec3i,

    /// Source layer (for texture arrays)
    pub source_layer: u32,
}

/// Buffer to texture copy operation
#[derive(Debug, Clone, PartialEq)]
pub struct HgiBufferToTextureOp {
    /// Source buffer
    pub gpu_source_buffer: HgiBufferHandle,

    /// Offset into source buffer
    pub source_byte_offset: usize,

    /// Destination texture
    pub gpu_destination_texture: HgiTextureHandle,

    /// Destination region offset
    pub destination_texel_offset: Vec3i,

    /// Size of region to copy
    pub copy_size: Vec3i,

    /// Destination mip level
    pub destination_mip_level: u32,

    /// Destination layer
    pub destination_layer: u32,
}

/// Texture to buffer copy operation
#[derive(Debug, Clone, PartialEq)]
pub struct HgiTextureToBufferOp {
    /// Source texture
    pub gpu_source_texture: HgiTextureHandle,

    /// Source region offset
    pub source_texel_offset: Vec3i,

    /// Source mip level
    pub mip_level: u32,

    /// Source layer
    pub source_layer: u32,

    /// Destination buffer
    pub gpu_destination_buffer: HgiBufferHandle,

    /// Offset into destination buffer
    pub destination_byte_offset: usize,

    /// Size of region to copy
    pub copy_size: Vec3i,
}

/// Blit command buffer for copy operations
///
/// Used for CPU<->GPU and GPU<->GPU data transfers.
pub trait HgiBlitCmds: HgiCmds {
    /// Copy data from CPU to GPU buffer
    fn copy_buffer_cpu_to_gpu(&mut self, op: &HgiBufferCpuToGpuOp);

    /// Copy data from GPU buffer to CPU buffer (read-back)
    ///
    /// Synchronization between GPU writes and CPU reads must be managed by
    /// the client by supplying the correct 'wait' flags in SubmitCmds.
    fn copy_buffer_gpu_to_cpu(&mut self, _op: &HgiBufferGpuToCpuOp) {
        // Default: no-op. Backends override for GPU->CPU copy support.
    }

    /// Copy data from GPU buffer to GPU buffer
    fn copy_buffer_gpu_to_gpu(&mut self, op: &HgiBufferGpuToGpuOp);

    /// Copy data from CPU to GPU texture
    fn copy_texture_cpu_to_gpu(&mut self, op: &HgiTextureCpuToGpuOp);

    /// Copy data from GPU texture to GPU texture
    fn copy_texture_gpu_to_gpu(&mut self, op: &HgiTextureGpuToGpuOp);

    /// Copy data from GPU texture to CPU buffer
    fn copy_texture_gpu_to_cpu(&mut self, op: &HgiTextureGpuToCpuOp);

    /// Copy data from GPU buffer to GPU texture
    fn copy_buffer_to_texture(&mut self, op: &HgiBufferToTextureOp);

    /// Copy data from GPU texture to GPU buffer
    fn copy_texture_to_buffer(&mut self, op: &HgiTextureToBufferOp);

    /// Generate mipmaps for a texture
    fn generate_mipmap(&mut self, texture: &HgiTextureHandle);

    /// Fill an entire buffer with a constant byte value (C++ `FillBuffer(buffer, uint8_t)`)
    fn fill_buffer(&mut self, buffer: &HgiBufferHandle, value: u8);

    /// Insert a memory barrier in blit commands
    ///
    /// Ensures that data written to memory by commands before the barrier
    /// is available to commands after the barrier.
    fn memory_barrier(&mut self, _barrier: HgiMemoryBarrier) {
        // Default: no-op (some backends handle barriers automatically)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_cpu_to_gpu_op() {
        let data: Vec<u8> = vec![1, 2, 3, 4];
        let op = HgiBufferCpuToGpuOp {
            // SAFETY: data outlives op within this test scope
            cpu_source_buffer: RawCpuBuffer::new(data.as_ptr()),
            source_byte_offset: 0,
            gpu_destination_buffer: HgiBufferHandle::null(),
            destination_byte_offset: 0,
            byte_size: 4,
        };

        assert_eq!(op.byte_size, 4);
        assert_eq!(op.source_byte_offset, 0);
        assert_eq!(op.destination_byte_offset, 0);
    }

    #[test]
    fn test_texture_gpu_to_gpu_op() {
        let op = HgiTextureGpuToGpuOp {
            gpu_source_texture: HgiTextureHandle::null(),
            gpu_destination_texture: HgiTextureHandle::null(),
            source_texel_offset: Vec3i::new(0, 0, 0),
            destination_texel_offset: Vec3i::new(0, 0, 0),
            copy_size: Vec3i::new(256, 256, 1),
            source_mip_level: 0,
            destination_mip_level: 0,
            source_layer: 0,
            destination_layer: 0,
        };

        assert_eq!(op.copy_size, Vec3i::new(256, 256, 1));
        assert_eq!(op.source_mip_level, 0);
    }
}
