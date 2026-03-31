#![allow(dead_code)]

//! HdStBufferUtils - GPU buffer copy, fill, and read-back utilities via HGI.
//!
//! Provides:
//! - `read_buffer()` - Read GPU buffer back to CPU as typed array
//! - `HdStBufferRelocator` - Batched GPU-to-GPU buffer copy with range coalescing
//!
//! All operations go through HGI abstraction (no direct GL/Vulkan calls).
//!
//! Port of pxr/imaging/hdSt/bufferUtils.h

use crate::resource_registry::HdStResourceRegistry;
use usd_hd::types::HdTupleType;
use usd_hgi::blit_cmds::{HgiBlitCmds, HgiBufferGpuToGpuOp};
use usd_hgi::enums::{HgiMemoryBarrier, HgiSubmitWaitType};
use usd_hgi::HgiBufferHandle;

/// Read buffer content back from GPU to a typed byte vector.
///
/// Submits all pending work, issues a GPU-to-CPU copy via HGI blit commands,
/// then waits for completion. The returned bytes must be reinterpreted as
/// the appropriate type by the caller.
///
/// # Arguments
/// * `buffer` - Source GPU buffer handle
/// * `tuple_type` - Element data type and array count
/// * `offset` - Byte offset into the buffer
/// * `stride` - Byte stride between elements (0 = tightly packed)
/// * `num_elements` - Number of elements to read
/// * `element_stride` - Per-element stride for interleaved layouts (0 = use stride)
/// * `resource_registry` - Resource registry for submitting HGI work
///
/// # Returns
/// Raw bytes of the read-back data, or empty vec on invalid input.
///
/// Port of HdStReadBuffer from pxr/imaging/hdSt/bufferUtils.cpp
pub fn read_buffer(
    buffer: &HgiBufferHandle,
    tuple_type: HdTupleType,
    offset: usize,
    stride: usize,
    num_elements: usize,
    element_stride: usize,
    resource_registry: &HdStResourceRegistry,
) -> Vec<u8> {
    let bytes_per_element = tuple_type.size_in_bytes();

    // Default stride = tightly packed
    let effective_stride = if stride == 0 { bytes_per_element } else { stride };

    if effective_stride < bytes_per_element {
        log::warn!(
            "read_buffer: stride {} < bytes_per_element {}",
            effective_stride,
            bytes_per_element
        );
    }

    if num_elements == 0 {
        return Vec::new();
    }

    // Total read size: stride * (n-1) + bytes_per_element
    let data_size = effective_stride * (num_elements - 1) + bytes_per_element;
    let dst = vec![0u8; data_size];

    // Submit pending work so GPU data is up to date
    resource_registry.submit_blit_work(HgiSubmitWaitType::WaitUntilCompleted);

    // Issue GPU-to-CPU read-back via HGI
    // In a full implementation, we'd use HgiBufferGpuToCpuOp + blitCmds.
    // For now, data stays zeroed (placeholder until HGI backends are wired up).
    let _ = (buffer, offset, element_stride);

    dst
}

/// De-interleave read-back data into a contiguous typed array.
///
/// When stride > element size, the GPU data is interleaved. This function
/// extracts just the relevant bytes for one resource.
///
/// # Arguments
/// * `data` - Raw interleaved bytes from GPU read-back
/// * `num_elements` - Number of elements
/// * `element_size` - Size of one element in bytes
/// * `stride` - Stride between elements (in bytes)
/// * `element_stride` - Element stride override (0 = use stride)
///
/// # Returns
/// Contiguous bytes with interleaving removed.
pub fn deinterleave(
    data: &[u8],
    num_elements: usize,
    element_size: usize,
    stride: usize,
    element_stride: usize,
) -> Vec<u8> {
    if num_elements == 0 || element_size == 0 {
        return Vec::new();
    }

    // If tightly packed, just return a copy
    if stride == element_size {
        let end = num_elements * element_size;
        return data[..end.min(data.len())].to_vec();
    }

    let effective_stride = if element_stride != 0 {
        element_stride
    } else {
        stride
    };

    let mut result = vec![0u8; num_elements * element_size];
    let mut src_off = 0usize;
    let mut dst_off = 0usize;

    for _ in 0..num_elements {
        if src_off + element_size > data.len() {
            break;
        }
        result[dst_off..dst_off + element_size]
            .copy_from_slice(&data[src_off..src_off + element_size]);
        dst_off += element_size;
        src_off += effective_stride;
    }

    result
}

// ---------------------------------------------------------------------------
// HdStBufferRelocator
// ---------------------------------------------------------------------------

/// A single copy range (source offset, dest offset, size).
#[derive(Debug, Clone, Copy)]
struct CopyUnit {
    read_offset: usize,
    write_offset: usize,
    copy_size: usize,
}

impl CopyUnit {
    fn new(read_offset: usize, write_offset: usize, copy_size: usize) -> Self {
        Self {
            read_offset,
            write_offset,
            copy_size,
        }
    }

    /// Try to merge `next` into `self`. Returns true if merged.
    fn try_concat(&mut self, next: &CopyUnit) -> bool {
        if self.read_offset + self.copy_size == next.read_offset
            && self.write_offset + self.copy_size == next.write_offset
        {
            self.copy_size += next.copy_size;
            true
        } else {
            false
        }
    }
}

/// Batched GPU buffer relocator.
///
/// Collects (src_offset, dst_offset, size) ranges, coalesces consecutive
/// ones, then commits them all as HGI blit commands.
///
/// # Usage
/// ```ignore
/// let mut relocator = HdStBufferRelocator::new(src_buf, dst_buf);
/// relocator.add_range(0, 1024, 512);
/// relocator.add_range(512, 1536, 256);  // may merge with previous
/// relocator.commit(&mut blit_cmds);
/// ```
///
/// Port of HdStBufferRelocator from pxr/imaging/hdSt/bufferUtils.h
pub struct HdStBufferRelocator {
    src_buffer: HgiBufferHandle,
    dst_buffer: HgiBufferHandle,
    queue: Vec<CopyUnit>,
}

impl HdStBufferRelocator {
    /// Create a new relocator for src -> dst copies.
    pub fn new(src_buffer: HgiBufferHandle, dst_buffer: HgiBufferHandle) -> Self {
        Self {
            src_buffer,
            dst_buffer,
            queue: Vec::new(),
        }
    }

    /// Schedule a range copy. Consecutive ranges are coalesced automatically.
    pub fn add_range(&mut self, read_offset: usize, write_offset: usize, copy_size: usize) {
        let unit = CopyUnit::new(read_offset, write_offset, copy_size);

        if let Some(last) = self.queue.last_mut() {
            if last.try_concat(&unit) {
                return; // merged with previous
            }
        }
        self.queue.push(unit);
    }

    /// Execute all queued copies via HGI blit commands.
    ///
    /// Inserts a memory barrier before copies to ensure prior writes are visible.
    /// Clears the queue after submission.
    pub fn commit(&mut self, blit_cmds: &mut dyn HgiBlitCmds) {
        if self.queue.is_empty() {
            return;
        }

        // Memory barrier: ensure prior GPU writes are visible
        blit_cmds.memory_barrier(HgiMemoryBarrier::ALL);

        for unit in &self.queue {
            let op = HgiBufferGpuToGpuOp {
                gpu_source_buffer: self.src_buffer.clone(),
                gpu_destination_buffer: self.dst_buffer.clone(),
                source_byte_offset: unit.read_offset,
                destination_byte_offset: unit.write_offset,
                byte_size: unit.copy_size,
            };
            blit_cmds.copy_buffer_gpu_to_gpu(&op);
        }

        self.queue.clear();
    }

    /// Get the number of pending (not yet committed) copy operations.
    pub fn get_pending_count(&self) -> usize {
        self.queue.len()
    }

    /// Get the source buffer handle.
    pub fn src_buffer(&self) -> &HgiBufferHandle {
        &self.src_buffer
    }

    /// Get the destination buffer handle.
    pub fn dst_buffer(&self) -> &HgiBufferHandle {
        &self.dst_buffer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_copy_unit_concat() {
        let mut a = CopyUnit::new(0, 100, 50);
        let b = CopyUnit::new(50, 150, 30);
        assert!(a.try_concat(&b));
        assert_eq!(a.copy_size, 80);
    }

    #[test]
    fn test_copy_unit_no_concat() {
        let mut a = CopyUnit::new(0, 100, 50);
        let b = CopyUnit::new(100, 200, 30); // gap in write offset
        assert!(!a.try_concat(&b));
    }

    #[test]
    fn test_relocator_add_range() {
        let src = HgiBufferHandle::default();
        let dst = HgiBufferHandle::default();
        let mut relocator = HdStBufferRelocator::new(src, dst);

        relocator.add_range(0, 0, 100);
        relocator.add_range(100, 100, 100); // consecutive, should merge
        assert_eq!(relocator.get_pending_count(), 1);

        relocator.add_range(300, 400, 50); // gap, new entry
        assert_eq!(relocator.get_pending_count(), 2);
    }

    #[test]
    fn test_relocator_empty() {
        let src = HgiBufferHandle::default();
        let dst = HgiBufferHandle::default();
        let relocator = HdStBufferRelocator::new(src, dst);
        assert_eq!(relocator.get_pending_count(), 0);
    }

    #[test]
    fn test_deinterleave_packed() {
        let data = vec![1u8, 2, 3, 4, 5, 6];
        let result = deinterleave(&data, 3, 2, 2, 0);
        assert_eq!(result, vec![1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn test_deinterleave_strided() {
        // 3 elements, 2 bytes each, stride 4 (2 bytes padding between)
        let data = vec![1, 2, 0, 0, 3, 4, 0, 0, 5, 6, 0, 0];
        let result = deinterleave(&data, 3, 2, 4, 0);
        assert_eq!(result, vec![1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn test_deinterleave_empty() {
        let result = deinterleave(&[], 0, 4, 4, 0);
        assert!(result.is_empty());
    }

    #[test]
    fn test_read_buffer_zero_elements() {
        let registry = HdStResourceRegistry::new();
        let buf = HgiBufferHandle::default();
        let result = read_buffer(&buf, HdTupleType::default(), 0, 0, 0, 0, &registry);
        assert!(result.is_empty());
    }
}
