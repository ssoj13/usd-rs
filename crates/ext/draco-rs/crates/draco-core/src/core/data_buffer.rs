//! Data buffer utilities.
//! Reference: `_ref/draco/src/draco/core/data_buffer.h` + `.cc`.

use std::io::Write;

/// Buffer descriptor serving as a unique identifier of a buffer.
#[derive(Debug, Clone, Copy, Default)]
pub struct DataBufferDescriptor {
    /// Id of the data buffer.
    pub buffer_id: i64,
    /// The number of times the buffer content was updated.
    pub buffer_update_count: i64,
}

/// Class used for storing raw buffer data.
#[derive(Debug, Default, Clone)]
pub struct DataBuffer {
    data: Vec<u8>,
    descriptor: DataBufferDescriptor,
}

impl DataBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, data: Option<&[u8]>, size: i64) -> bool {
        self.update_with_offset(data, size, 0)
    }

    pub fn update_with_offset(&mut self, data: Option<&[u8]>, size: i64, offset: i64) -> bool {
        if data.is_none() {
            if size + offset < 0 {
                return false;
            }
            let new_size = (size + offset) as usize;
            self.data.resize(new_size, 0);
        } else {
            if size < 0 {
                return false;
            }
            let size_u = size as usize;
            let offset_u = if offset < 0 {
                return false;
            } else {
                offset as usize
            };
            let src = data.unwrap();
            if src.len() < size_u {
                return false;
            }
            let required_size = offset_u.saturating_add(size_u);
            if required_size > self.data.len() {
                self.data.resize(required_size, 0);
            }
            self.data[offset_u..offset_u + size_u].copy_from_slice(&src[..size_u]);
        }
        self.descriptor.buffer_update_count += 1;
        true
    }

    /// Reallocate the buffer storage to a new size keeping the data unchanged.
    pub fn resize(&mut self, new_size: i64) {
        if new_size < 0 {
            return;
        }
        self.data.resize(new_size as usize, 0);
        self.descriptor.buffer_update_count += 1;
    }

    pub fn write_data_to_stream<W: Write>(&self, stream: &mut W) -> std::io::Result<()> {
        if self.data.is_empty() {
            return Ok(());
        }
        stream.write_all(&self.data)
    }

    /// Reads data from the buffer. Caller must ensure bounds are valid.
    pub fn read(&self, byte_pos: i64, out_data: &mut [u8]) {
        if byte_pos < 0 {
            return;
        }
        let offset = byte_pos as usize;
        let end = offset.saturating_add(out_data.len());
        if end > self.data.len() {
            return;
        }
        out_data.copy_from_slice(&self.data[offset..end]);
    }

    /// Writes data to the buffer. Caller must ensure bounds are valid.
    pub fn write(&mut self, byte_pos: i64, in_data: &[u8]) {
        if byte_pos < 0 {
            return;
        }
        let offset = byte_pos as usize;
        let end = offset.saturating_add(in_data.len());
        if end > self.data.len() {
            return;
        }
        self.data[offset..end].copy_from_slice(in_data);
    }

    /// In-buffer copy. Handles overlapping ranges correctly (like Vec::copy_within).
    /// Use for compaction/removal where src and dst may alias.
    #[inline]
    pub fn copy_within(&mut self, src: std::ops::Range<usize>, dst: usize) {
        self.data.copy_within(src, dst);
    }

    /// Copies data from another buffer to this buffer.
    pub fn copy_from(&mut self, dst_offset: i64, src_buf: &DataBuffer, src_offset: i64, size: i64) {
        if dst_offset < 0 || src_offset < 0 || size < 0 {
            return;
        }
        let dst_off = dst_offset as usize;
        let src_off = src_offset as usize;
        let size_u = size as usize;
        let dst_end = dst_off.saturating_add(size_u);
        let src_end = src_off.saturating_add(size_u);
        if dst_end > self.data.len() || src_end > src_buf.data.len() {
            return;
        }
        let src_slice = &src_buf.data[src_off..src_end];
        self.data[dst_off..dst_end].copy_from_slice(src_slice);
    }

    pub fn set_update_count(&mut self, buffer_update_count: i64) {
        self.descriptor.buffer_update_count = buffer_update_count;
    }

    pub fn update_count(&self) -> i64 {
        self.descriptor.buffer_update_count
    }

    pub fn data_size(&self) -> usize {
        self.data.len()
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    pub fn buffer_id(&self) -> i64 {
        self.descriptor.buffer_id
    }

    pub fn set_buffer_id(&mut self, buffer_id: i64) {
        self.descriptor.buffer_id = buffer_id;
    }
}
