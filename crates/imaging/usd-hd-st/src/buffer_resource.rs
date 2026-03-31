#![allow(dead_code)]

//! HdStBufferResource - GPU buffer resource management.
//!
//! BufferResource represents a single named GPU buffer through HGI,
//! with offset, stride, and data type metadata.
//!
//! Port of pxr/imaging/hdSt/bufferResource.h

use std::sync::Arc;
use usd_hd::types::HdTupleType;
use usd_hgi::HgiBufferHandle;
use usd_tf::Token;

/// GPU buffer resource.
///
/// Wraps an HGI buffer handle with metadata describing the role,
/// type, offset, and stride of the data within. Multiple buffer
/// resources may share the same underlying HGI buffer at different
/// offsets (interleaved layout).
///
/// Port of HdStBufferResource from pxr/imaging/hdSt/bufferResource.h
#[derive(Debug, Clone)]
pub struct HdStBufferResource {
    /// HGI buffer handle
    handle: HgiBufferHandle,
    /// Buffer size in bytes
    size: usize,
    /// Role of the data (e.g., "points", "normals")
    role: Token,
    /// Data type and tuple count
    tuple_type: HdTupleType,
    /// Interleaved offset in bytes
    offset: i32,
    /// Stride in bytes between elements
    stride: i32,
}

impl HdStBufferResource {
    /// Create a new buffer resource.
    pub fn new(role: Token, tuple_type: HdTupleType, offset: i32, stride: i32) -> Self {
        Self {
            handle: HgiBufferHandle::default(),
            size: 0,
            role,
            tuple_type,
            offset,
            stride,
        }
    }

    /// Create with a pre-allocated size (no role metadata).
    pub fn with_size(size: usize) -> Self {
        Self {
            handle: HgiBufferHandle::default(),
            size,
            role: Token::new(""),
            tuple_type: HdTupleType::default(),
            offset: 0,
            stride: 0,
        }
    }

    /// Get the data role token.
    pub fn get_role(&self) -> &Token {
        &self.role
    }

    /// Get buffer size in bytes.
    pub fn get_size(&self) -> usize {
        self.size
    }

    /// Get the data type and tuple count.
    pub fn get_tuple_type(&self) -> HdTupleType {
        self.tuple_type
    }

    /// Get the interleaved offset in bytes.
    pub fn get_offset(&self) -> i32 {
        self.offset
    }

    /// Get the stride in bytes between elements.
    pub fn get_stride(&self) -> i32 {
        self.stride
    }

    /// Set the HGI buffer handle and allocation size.
    pub fn set_allocation(&mut self, handle: HgiBufferHandle, size: usize) {
        self.handle = handle;
        self.size = size;
    }

    /// Get the HGI buffer handle.
    pub fn get_handle(&self) -> &HgiBufferHandle {
        &self.handle
    }

    /// Get mutable reference to HGI buffer handle.
    pub fn get_handle_mut(&mut self) -> &mut HgiBufferHandle {
        &mut self.handle
    }

    /// Check if buffer has been allocated (has nonzero size).
    pub fn is_valid(&self) -> bool {
        self.size > 0
    }
}

/// Shared pointer to buffer resource.
pub type HdStBufferResourceSharedPtr = Arc<HdStBufferResource>;

/// Named pair of buffer resource.
pub type HdStBufferResourceNamedPair = (Token, HdStBufferResourceSharedPtr);

/// Named list of buffer resources.
pub type HdStBufferResourceNamedList = Vec<HdStBufferResourceNamedPair>;

/// Buffer array range - a view into a buffer resource.
///
/// Multiple prims can share the same buffer resource, each with
/// their own range (offset + count) into that buffer.
#[derive(Debug, Clone)]
pub struct HdStBufferArrayRange {
    /// Parent buffer resource
    buffer: Option<HdStBufferResourceSharedPtr>,
    /// Offset into buffer in bytes
    offset: usize,
    /// Size of this range in bytes
    size: usize,
    /// Byte size of positions data within packed vertex buffer (0 = unknown)
    positions_byte_size: usize,
    /// Byte size of normals data within packed vertex buffer (0 = not present)
    normals_byte_size: usize,
    /// Byte size of UV (texcoord) data within packed vertex buffer (0 = not present)
    uvs_byte_size: usize,
    /// Byte size of displayColor data within packed vertex buffer (0 = not present)
    colors_byte_size: usize,
}

impl HdStBufferArrayRange {
    /// Create a new buffer array range.
    pub fn new(buffer: HdStBufferResourceSharedPtr, offset: usize, size: usize) -> Self {
        Self {
            buffer: Some(buffer),
            offset,
            size,
            positions_byte_size: 0,
            normals_byte_size: 0,
            uvs_byte_size: 0,
            colors_byte_size: 0,
        }
    }

    /// Create with explicit positions byte size (for packed pos+normals buffers).
    pub fn with_positions_size(
        buffer: HdStBufferResourceSharedPtr,
        offset: usize,
        size: usize,
        positions_byte_size: usize,
    ) -> Self {
        Self {
            buffer: Some(buffer),
            offset,
            size,
            positions_byte_size,
            normals_byte_size: 0,
            uvs_byte_size: 0,
            colors_byte_size: 0,
        }
    }

    /// Create with explicit per-stream sizes (pos + normals + uvs + colors packed in one buffer).
    pub fn with_stream_sizes(
        buffer: HdStBufferResourceSharedPtr,
        offset: usize,
        size: usize,
        positions_byte_size: usize,
        normals_byte_size: usize,
        uvs_byte_size: usize,
        colors_byte_size: usize,
    ) -> Self {
        Self {
            buffer: Some(buffer),
            offset,
            size,
            positions_byte_size,
            normals_byte_size,
            uvs_byte_size,
            colors_byte_size,
        }
    }

    /// Get parent buffer.
    pub fn get_buffer(&self) -> Option<&HdStBufferResourceSharedPtr> {
        self.buffer.as_ref()
    }

    /// Get offset into buffer.
    pub fn get_offset(&self) -> usize {
        self.offset
    }

    /// Get size of range.
    pub fn get_size(&self) -> usize {
        self.size
    }

    /// Get positions byte size (0 = use size/2 fallback).
    pub fn get_positions_byte_size(&self) -> usize {
        self.positions_byte_size
    }

    /// Get normals byte size (0 = not present).
    pub fn get_normals_byte_size(&self) -> usize {
        self.normals_byte_size
    }

    /// Get UV byte size (0 = not present).
    pub fn get_uvs_byte_size(&self) -> usize {
        self.uvs_byte_size
    }

    /// Get displayColor byte size (0 = not present).
    pub fn get_colors_byte_size(&self) -> usize {
        self.colors_byte_size
    }

    /// Check if range is valid.
    pub fn is_valid(&self) -> bool {
        self.buffer.as_ref().map(|b| b.is_valid()).unwrap_or(false)
    }
}

// Implement the HdBufferArrayRange trait from draw_item.rs
impl crate::draw_item::HdBufferArrayRange for HdStBufferArrayRange {
    fn is_valid(&self) -> bool {
        HdStBufferArrayRange::is_valid(self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_resource_creation() {
        let buffer = HdStBufferResource::new(Token::new("points"), HdTupleType::default(), 0, 12);
        assert_eq!(buffer.get_role().as_str(), "points");
        assert_eq!(buffer.get_stride(), 12);
        assert!(!buffer.is_valid());
    }

    #[test]
    fn test_buffer_allocation() {
        let mut buffer =
            HdStBufferResource::new(Token::new("normals"), HdTupleType::default(), 0, 12);
        buffer.set_allocation(HgiBufferHandle::default(), 1024);
        assert_eq!(buffer.get_size(), 1024);
        assert!(buffer.is_valid());
    }

    #[test]
    fn test_buffer_array_range() {
        let buffer = Arc::new(HdStBufferResource::with_size(1024));
        let range = HdStBufferArrayRange::new(buffer, 256, 512);
        assert_eq!(range.get_offset(), 256);
        assert_eq!(range.get_size(), 512);
        assert!(range.is_valid());
        // Default: no stream sizes set
        assert_eq!(range.get_positions_byte_size(), 0);
        assert_eq!(range.get_normals_byte_size(), 0);
        assert_eq!(range.get_uvs_byte_size(), 0);
    }

    #[test]
    fn test_buffer_array_range_with_stream_sizes() {
        // Simulate mesh with pos (36B) + normals (36B) + uvs (24B)
        let total = 36 + 36 + 24;
        let buffer = Arc::new(HdStBufferResource::with_size(total));
        let range = HdStBufferArrayRange::with_stream_sizes(buffer, 0, total, 36, 36, 24, 0);
        assert_eq!(range.get_positions_byte_size(), 36);
        assert_eq!(range.get_normals_byte_size(), 36);
        assert_eq!(range.get_uvs_byte_size(), 24);
        // Verify offsets: normals start at 36, uvs start at 72
        let nrm_offset = (range.get_positions_byte_size() + 3) & !3;
        assert_eq!(nrm_offset, 36, "normals offset 36B aligned");
        let uv_offset = (nrm_offset + range.get_normals_byte_size() + 3) & !3;
        assert_eq!(uv_offset, 72, "uv offset 72B aligned");
    }

    #[test]
    fn test_buffer_array_range_with_positions_size_backward_compat() {
        // with_positions_size must leave normals and uvs at 0
        let buf = Arc::new(HdStBufferResource::with_size(100));
        let range = HdStBufferArrayRange::with_positions_size(buf, 0, 100, 60);
        assert_eq!(range.get_positions_byte_size(), 60);
        assert_eq!(
            range.get_normals_byte_size(),
            0,
            "normals size should default to 0"
        );
        assert_eq!(range.get_uvs_byte_size(), 0, "uvs size should default to 0");
    }
}
