#![allow(dead_code)]

//! HdStStagingBuffer - Staging buffer for CPU-to-GPU uploads via HGI.
//!
//! Provides a ring buffer for batching and uploading data to GPU
//! asynchronously. Reduces upload overhead by grouping small uploads
//! into larger transfers through HGI blit commands.
//!
//! Port of pxr/imaging/hdSt/stagingBuffer.h

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use usd_hgi::HgiBufferHandle;

/// Default staging buffer size (16 MB).
const DEFAULT_STAGING_SIZE: usize = 16 * 1024 * 1024;

/// Maximum number of in-flight upload operations.
const MAX_IN_FLIGHT: usize = 3;

/// A single upload from staging buffer to GPU destination.
#[derive(Debug, Clone)]
pub struct UploadOperation {
    /// Destination buffer handle
    dst_buffer: HgiBufferHandle,
    /// Destination offset in bytes
    dst_offset: usize,
    /// Upload size in bytes
    size: usize,
    /// Generation counter for fence tracking
    generation: u64,
}

impl UploadOperation {
    /// Create a new upload operation.
    pub fn new(
        dst_buffer: HgiBufferHandle,
        dst_offset: usize,
        size: usize,
        generation: u64,
    ) -> Self {
        Self {
            dst_buffer,
            dst_offset,
            size,
            generation,
        }
    }

    pub fn dst_buffer(&self) -> &HgiBufferHandle {
        &self.dst_buffer
    }
    pub fn dst_offset(&self) -> usize {
        self.dst_offset
    }
    pub fn size(&self) -> usize {
        self.size
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
}

/// Internal ring buffer state.
struct RingState {
    /// HGI staging buffer (host-visible GPU memory)
    staging_handle: HgiBufferHandle,
    /// Total buffer capacity
    capacity: usize,
    /// Current write position
    write_pos: usize,
    /// Current read position (reclaimed after GPU completion)
    read_pos: usize,
    /// Pending uploads (not yet submitted)
    pending: VecDeque<UploadOperation>,
    /// In-flight uploads (submitted, awaiting GPU completion)
    in_flight: VecDeque<UploadOperation>,
    /// Generation counter
    generation: u64,
    /// Stats: total bytes staged
    total_staged: usize,
    /// Stats: total bytes uploaded to GPU
    total_uploaded: usize,
}

/// Staging buffer for GPU uploads through HGI.
///
/// Ring buffer strategy:
/// 1. `stage()` - Copy data into staging area, record upload op
/// 2. `flush()` - Submit pending uploads via HGI blit commands
/// 3. `sync()` - Wait for GPU completion, reclaim ring space
///
/// Thread-safe via internal mutex.
///
/// Port of HdStStagingBuffer from pxr/imaging/hdSt/stagingBuffer.h
pub struct StagingBuffer {
    state: Arc<Mutex<RingState>>,
}

impl StagingBuffer {
    /// Create a new staging buffer.
    ///
    /// # Arguments
    /// * `capacity` - Buffer size in bytes (default: 16 MB)
    pub fn new(capacity: Option<usize>) -> Self {
        let cap = capacity.unwrap_or(DEFAULT_STAGING_SIZE);
        Self {
            state: Arc::new(Mutex::new(RingState {
                staging_handle: HgiBufferHandle::default(),
                capacity: cap,
                write_pos: 0,
                read_pos: 0,
                pending: VecDeque::new(),
                in_flight: VecDeque::new(),
                generation: 0,
                total_staged: 0,
                total_uploaded: 0,
            })),
        }
    }

    /// Stage data for upload to a destination buffer.
    ///
    /// Copies data into the ring buffer and records an upload operation.
    /// Does NOT submit to GPU - call `flush()` for that.
    ///
    /// Returns true if staging succeeded, false if ring is full.
    pub fn stage(&self, dst_buffer: HgiBufferHandle, dst_offset: usize, data: &[u8]) -> bool {
        let mut s = self.state.lock().unwrap();
        let size = data.len();

        if available_space(&s) < size {
            return false;
        }

        // In real impl: memcpy to mapped staging_handle + write_pos
        // For now: just track the operation

        s.generation += 1;
        let fence = s.generation;
        s.pending
            .push_back(UploadOperation::new(dst_buffer, dst_offset, size, fence));
        s.write_pos = (s.write_pos + size) % s.capacity;
        s.total_staged += size;
        true
    }

    /// Submit pending uploads to GPU via HGI blit commands.
    ///
    /// Returns number of uploads submitted.
    pub fn flush(&self) -> usize {
        let mut s = self.state.lock().unwrap();
        let count = s.pending.len();

        while let Some(op) = s.pending.pop_front() {
            // In real impl: HgiBlitCmds::CopyBufferGpuToGpu(staging -> dst)
            s.total_uploaded += op.size();
            s.in_flight.push_back(op);

            if s.in_flight.len() >= MAX_IN_FLIGHT {
                break;
            }
        }
        count
    }

    /// Wait for GPU uploads to complete and reclaim ring space.
    ///
    /// If `wait_all` is true, waits for ALL in-flight uploads.
    /// Otherwise waits for the oldest upload only.
    pub fn sync(&self, wait_all: bool) {
        let mut s = self.state.lock().unwrap();

        if wait_all {
            // In real impl: hgi->WaitForFence(last_fence)
            let total: usize = s.in_flight.iter().map(|u| u.size()).sum();
            s.read_pos = (s.read_pos + total) % s.capacity;
            s.in_flight.clear();
        } else if let Some(oldest) = s.in_flight.front() {
            let size = oldest.size();
            s.read_pos = (s.read_pos + size) % s.capacity;
            s.in_flight.pop_front();
        }
    }

    /// Get number of pending (not yet submitted) uploads.
    pub fn get_pending_count(&self) -> usize {
        self.state.lock().unwrap().pending.len()
    }

    /// Get number of in-flight (submitted, awaiting completion) uploads.
    pub fn get_in_flight_count(&self) -> usize {
        self.state.lock().unwrap().in_flight.len()
    }

    /// Get total bytes staged since creation.
    pub fn get_total_staged(&self) -> usize {
        self.state.lock().unwrap().total_staged
    }

    /// Get total bytes uploaded to GPU since creation.
    pub fn get_total_uploaded(&self) -> usize {
        self.state.lock().unwrap().total_uploaded
    }

    /// Get available space in ring buffer.
    pub fn get_available_space(&self) -> usize {
        let s = self.state.lock().unwrap();
        available_space(&s)
    }
}

/// Calculate available space in the ring buffer.
fn available_space(s: &RingState) -> usize {
    if s.write_pos >= s.read_pos {
        s.capacity - (s.write_pos - s.read_pos)
    } else {
        s.read_pos - s.write_pos
    }
}

impl Default for StagingBuffer {
    fn default() -> Self {
        Self::new(None)
    }
}

/// Shared pointer to staging buffer.
pub type StagingBufferSharedPtr = Arc<StagingBuffer>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_creation() {
        let buf = StagingBuffer::new(Some(1024));
        assert_eq!(buf.get_available_space(), 1024);
        assert_eq!(buf.get_pending_count(), 0);
    }

    #[test]
    fn test_stage() {
        let buf = StagingBuffer::new(Some(1024));
        let dst = HgiBufferHandle::default();
        assert!(buf.stage(dst, 0, &[0u8; 128]));
        assert_eq!(buf.get_pending_count(), 1);
        assert_eq!(buf.get_total_staged(), 128);
    }

    #[test]
    fn test_flush() {
        let buf = StagingBuffer::new(Some(1024));
        let dst = HgiBufferHandle::default();
        buf.stage(dst.clone(), 0, &[0u8; 64]);
        buf.stage(dst.clone(), 64, &[0u8; 64]);
        buf.stage(dst, 128, &[0u8; 64]);

        let count = buf.flush();
        assert_eq!(count, 3);
        assert_eq!(buf.get_in_flight_count(), 3);
        assert_eq!(buf.get_pending_count(), 0);
    }

    #[test]
    fn test_sync() {
        let buf = StagingBuffer::new(Some(1024));
        let dst = HgiBufferHandle::default();
        buf.stage(dst, 0, &[0u8; 128]);
        buf.flush();

        assert_eq!(buf.get_in_flight_count(), 1);
        buf.sync(true);
        assert_eq!(buf.get_in_flight_count(), 0);
    }

    #[test]
    fn test_full() {
        let buf = StagingBuffer::new(Some(256));
        let dst = HgiBufferHandle::default();
        assert!(buf.stage(dst.clone(), 0, &[0u8; 200]));
        assert!(!buf.stage(dst, 200, &[0u8; 100])); // full
    }

    #[test]
    fn test_ring_wrap() {
        let buf = StagingBuffer::new(Some(512));
        let dst = HgiBufferHandle::default();
        buf.stage(dst.clone(), 0, &[0u8; 256]);
        buf.flush();
        buf.sync(true);

        // Space reclaimed, can reuse
        assert!(buf.stage(dst, 256, &[0u8; 256]));
    }

    #[test]
    fn test_max_in_flight() {
        let buf = StagingBuffer::new(Some(2048));
        let dst = HgiBufferHandle::default();
        for i in 0..10 {
            buf.stage(dst.clone(), i * 64, &[0u8; 64]);
        }
        buf.flush();
        assert!(buf.get_in_flight_count() <= MAX_IN_FLIGHT);
        assert!(buf.get_pending_count() > 0);
    }
}
