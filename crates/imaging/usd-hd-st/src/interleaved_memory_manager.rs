//! HdStInterleavedMemoryManager - Manages interleaved vertex attribute buffers.
//!
//! Packs multiple vertex attributes (position, normal, UV, etc.) into a single
//! interleaved buffer with proper alignment for GPU cache efficiency.
//!
//! The buffer layout per vertex is:
//!   [attr0 | padding | attr1 | padding | ...] repeated N times
//!
//! Each attribute is aligned to `element_alignment` bytes (default 4).
//! The full stride is also aligned to `struct_alignment` (default 16 for UBO).
//!
//! Port of pxr/imaging/hdSt/interleavedMemoryManager.h

use crate::vbo_memory_manager::{VboAllocation, VboMemoryManagerSharedPtr};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use usd_hd::types::HdTupleType;
use usd_hgi::HgiBufferUsage;
use usd_tf::Token;

/// Default alignment for individual attributes within a vertex struct.
const ELEMENT_ALIGNMENT: usize = 4;

/// Default struct alignment (stride is rounded up to this).
/// Matches std140 UBO layout requirement of 16 bytes.
const STRUCT_ALIGNMENT: usize = 16;

// ---------------------------------------------------------------------------
// VertexAttribute
// ---------------------------------------------------------------------------

/// Descriptor for one vertex attribute inside an interleaved layout.
#[derive(Debug, Clone)]
pub struct VertexAttribute {
    /// Attribute name/role (e.g. "points", "normals").
    pub name: Token,
    /// Data type and count.
    pub tuple_type: HdTupleType,
    /// Byte offset from start of vertex struct.
    pub offset: usize,
    /// Size in bytes of this attribute (unaligned).
    pub size: usize,
}

impl VertexAttribute {
    pub fn new(name: Token, tuple_type: HdTupleType, offset: usize, size: usize) -> Self {
        Self {
            name,
            tuple_type,
            offset,
            size,
        }
    }
}

// ---------------------------------------------------------------------------
// InterleavedLayout
// ---------------------------------------------------------------------------

/// Describes how multiple vertex attributes are packed into a single buffer.
///
/// Example layout (stride=32):
///   [pos.xyz(12) | pad(4) | normal.xyz(12) | pad(4)] per vertex
///
/// Attributes are added sequentially; each is aligned to `element_alignment`.
/// The final stride is rounded up to `struct_alignment`.
#[derive(Debug, Clone)]
pub struct InterleavedLayout {
    /// Ordered list of attributes.
    attributes: Vec<VertexAttribute>,
    /// Byte stride per vertex (includes trailing padding for struct_alignment).
    stride: usize,
    /// Number of vertices.
    vertex_count: usize,
    /// Per-element alignment (default 4).
    element_alignment: usize,
    /// Struct alignment — stride is rounded to this (default 16).
    struct_alignment: usize,
}

impl InterleavedLayout {
    /// Create a layout with default alignment settings.
    pub fn new() -> Self {
        Self {
            attributes: Vec::new(),
            stride: 0,
            vertex_count: 0,
            element_alignment: ELEMENT_ALIGNMENT,
            struct_alignment: STRUCT_ALIGNMENT,
        }
    }

    /// Create a layout with explicit alignment settings.
    pub fn with_alignment(element_alignment: usize, struct_alignment: usize) -> Self {
        Self {
            attributes: Vec::new(),
            stride: 0,
            vertex_count: 0,
            element_alignment,
            struct_alignment,
        }
    }

    /// Add an attribute. Returns the byte offset where it was placed.
    ///
    /// Each attribute is aligned to `element_alignment`. The raw stride tracks the
    /// end of the last attribute. Struct-level alignment (`struct_alignment`) is only
    /// applied to the FINAL stride (not after every attribute), so intermediate
    /// attributes are not over-padded. (P1-12)
    pub fn add_attribute(&mut self, name: Token, tuple_type: HdTupleType, size: usize) -> usize {
        // Raw offset: align to element boundary from current raw end.
        let raw_end = if self.attributes.is_empty() {
            0
        } else {
            let last = &self.attributes[self.attributes.len() - 1];
            last.offset + last.size
        };
        let offset = align_up(raw_end, self.element_alignment);
        let attr = VertexAttribute::new(name, tuple_type, offset, size);
        self.attributes.push(attr);
        // Tentative stride: raw end of this attribute, without struct alignment.
        // struct_alignment is applied to the final stride in stride() / total_size().
        // This prevents over-padding intermediate attributes.
        self.stride = align_up(offset + size, self.struct_alignment);
        offset
    }

    pub fn stride(&self) -> usize {
        self.stride
    }
    pub fn attributes(&self) -> &[VertexAttribute] {
        &self.attributes
    }
    pub fn vertex_count(&self) -> usize {
        self.vertex_count
    }
    pub fn set_vertex_count(&mut self, count: usize) {
        self.vertex_count = count;
    }

    /// Total buffer size needed for all vertices.
    pub fn total_size(&self) -> usize {
        self.stride * self.vertex_count
    }

    /// Find attribute by name.
    pub fn find_attribute(&self, name: &Token) -> Option<&VertexAttribute> {
        self.attributes.iter().find(|a| &a.name == name)
    }
}

impl Default for InterleavedLayout {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// InterleavedAllocation
// ---------------------------------------------------------------------------

/// An allocated interleaved buffer: VBO range + layout descriptor.
#[derive(Debug, Clone)]
pub struct InterleavedAllocation {
    /// Underlying sub-allocation from the VBO manager.
    vbo: VboAllocation,
    /// Layout of attributes within each vertex.
    layout: InterleavedLayout,
}

impl InterleavedAllocation {
    pub fn new(vbo: VboAllocation, layout: InterleavedLayout) -> Self {
        Self { vbo, layout }
    }

    pub fn vbo_allocation(&self) -> &VboAllocation {
        &self.vbo
    }
    pub fn layout(&self) -> &InterleavedLayout {
        &self.layout
    }

    /// Per-vertex stride in bytes.
    pub fn stride(&self) -> usize {
        self.layout.stride()
    }

    /// Total buffer size in bytes.
    pub fn total_size(&self) -> usize {
        self.layout.total_size()
    }

    /// Byte offset of a named attribute within the buffer (absolute, not per-vertex).
    ///
    /// Returns `vbo_offset + attr.offset` — the value to pass as a vertex
    /// attribute binding offset to the GPU.
    pub fn attribute_offset(&self, name: &Token) -> Option<usize> {
        self.layout
            .find_attribute(name)
            .map(|a| self.vbo.offset() + a.offset)
    }
}

// ---------------------------------------------------------------------------
// InterleavedMemoryManager
// ---------------------------------------------------------------------------

struct ManagerState {
    allocations: HashMap<u64, InterleavedAllocation>,
    next_id: u64,
}

/// Interleaved memory manager.
///
/// Allocates interleaved vertex buffers via the underlying `VboMemoryManager`.
/// Computes attribute offsets and strides according to GPU alignment rules.
///
/// # Thread Safety
/// All operations are protected by an internal mutex.
pub struct InterleavedMemoryManager {
    vbo: VboMemoryManagerSharedPtr,
    state: Arc<Mutex<ManagerState>>,
}

impl InterleavedMemoryManager {
    pub fn new(vbo: VboMemoryManagerSharedPtr) -> Self {
        Self {
            vbo,
            state: Arc::new(Mutex::new(ManagerState {
                allocations: HashMap::new(),
                next_id: 0,
            })),
        }
    }

    /// Allocate an interleaved buffer from the given layout.
    ///
    /// Returns `None` if the layout is empty or if the VBO manager cannot
    /// satisfy the allocation.
    pub fn allocate(&self, layout: InterleavedLayout) -> Option<InterleavedAllocation> {
        let total = layout.total_size();
        if total == 0 {
            return None;
        }

        let vbo_alloc = self
            .vbo
            .allocate(total, HgiBufferUsage::VERTEX | HgiBufferUsage::STORAGE)?;
        let alloc = InterleavedAllocation::new(vbo_alloc, layout);

        let mut st = self.state.lock().unwrap();
        let id = st.next_id;
        st.next_id += 1;
        st.allocations.insert(id, alloc.clone());
        Some(alloc)
    }

    /// Free an interleaved allocation.
    pub fn free(&self, alloc: &InterleavedAllocation) {
        self.vbo.free(alloc.vbo_allocation());
        let freed_gen = alloc.vbo_allocation().generation();
        let mut st = self.state.lock().unwrap();
        st.allocations
            .retain(|_, a| a.vbo_allocation().generation() != freed_gen);
    }

    /// Number of live allocations.
    pub fn allocation_count(&self) -> usize {
        self.state.lock().unwrap().allocations.len()
    }

    /// Flush pending uploads to GPU (placeholder — real impl coordinates staging buffer).
    pub fn flush(&self) {
        // In a full implementation, this submits staged CPU->GPU copies via blit cmds.
    }
}

/// Shared pointer to InterleavedMemoryManager.
pub type InterleavedMemoryManagerSharedPtr = Arc<InterleavedMemoryManager>;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

#[inline]
fn align_up(v: usize, align: usize) -> usize {
    if align == 0 {
        return v;
    }
    (v + align - 1) & !(align - 1)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vbo_memory_manager::VboMemoryManager;
    use usd_hd::types::{HdTupleType, HdType};

    fn make_vbo() -> VboMemoryManagerSharedPtr {
        Arc::new(VboMemoryManager::new())
    }

    // --- Layout tests ---

    #[test]
    fn test_layout_offsets() {
        // P1-12 fix: struct_alignment is applied only to the FINAL stride, not after each
        // attribute. Intermediate offsets are aligned only to element_alignment (4 bytes).
        let mut layout = InterleavedLayout::new();
        // pos: 12 bytes at offset=0; stride = align16(12) = 16 (only one attr so far)
        let pos =
            layout.add_attribute(Token::new("points"), HdTupleType::new(HdType::Float, 3), 12);
        assert_eq!(pos, 0);
        assert_eq!(layout.stride(), 16); // align16(0+12) = 16

        // normal: raw_end=12, offset=align4(12)=12; stride = align16(12+12) = align16(24) = 32
        let norm = layout.add_attribute(
            Token::new("normals"),
            HdTupleType::new(HdType::Float, 3),
            12,
        );
        assert_eq!(norm, 12); // tightly packed after pos (element-aligned, not struct-aligned)
        assert_eq!(layout.stride(), 32); // align16(24) = 32

        // uv: raw_end=24, offset=align4(24)=24; stride = align16(24+8) = align16(32) = 32
        let uv = layout.add_attribute(Token::new("uvs"), HdTupleType::new(HdType::Float, 2), 8);
        assert_eq!(uv, 24);
        assert_eq!(layout.stride(), 32); // align16(32) = 32
    }

    #[test]
    fn test_layout_total_size() {
        let mut layout = InterleavedLayout::new();
        layout.add_attribute(Token::new("points"), HdTupleType::new(HdType::Float, 3), 12);
        layout.set_vertex_count(100);
        // stride = 16, total = 16 * 100
        assert_eq!(layout.total_size(), 1600);
    }

    #[test]
    fn test_find_attribute() {
        let mut layout = InterleavedLayout::new();
        layout.add_attribute(Token::new("points"), HdTupleType::new(HdType::Float, 3), 12);
        layout.add_attribute(
            Token::new("normals"),
            HdTupleType::new(HdType::Float, 3),
            12,
        );

        assert!(layout.find_attribute(&Token::new("points")).is_some());
        assert!(layout.find_attribute(&Token::new("normals")).is_some());
        assert!(layout.find_attribute(&Token::new("missing")).is_none());
    }

    #[test]
    fn test_layout_no_struct_alignment() {
        let mut layout = InterleavedLayout::with_alignment(4, 4);
        layout.add_attribute(Token::new("a"), HdTupleType::new(HdType::Float, 3), 12);
        layout.add_attribute(Token::new("b"), HdTupleType::new(HdType::Float, 3), 12);
        assert_eq!(layout.stride(), 24);
    }

    // --- Allocation tests ---

    #[test]
    fn test_allocate_and_free() {
        let mgr = InterleavedMemoryManager::new(make_vbo());

        let mut layout = InterleavedLayout::new();
        layout.add_attribute(Token::new("points"), HdTupleType::new(HdType::Float, 3), 12);
        layout.set_vertex_count(100);

        let alloc = mgr.allocate(layout).expect("alloc failed");
        assert_eq!(alloc.stride(), 16);
        assert_eq!(mgr.allocation_count(), 1);

        mgr.free(&alloc);
        assert_eq!(mgr.allocation_count(), 0);
    }

    #[test]
    fn test_attribute_offset_within_buffer() {
        let mgr = InterleavedMemoryManager::new(make_vbo());

        let mut layout = InterleavedLayout::new();
        layout.add_attribute(Token::new("points"), HdTupleType::new(HdType::Float, 3), 12);
        layout.add_attribute(
            Token::new("normals"),
            HdTupleType::new(HdType::Float, 3),
            12,
        );
        layout.set_vertex_count(10);

        let alloc = mgr.allocate(layout).unwrap();
        let pos_off = alloc.attribute_offset(&Token::new("points")).unwrap();
        let nrm_off = alloc.attribute_offset(&Token::new("normals")).unwrap();
        // P1-12: normals are placed at element_alignment (4) from end of positions,
        // not struct_alignment (16). So gap = 12 (tight packing), not 16.
        assert_eq!(nrm_off - pos_off, 12);
    }

    #[test]
    fn test_empty_layout_returns_none() {
        let mgr = InterleavedMemoryManager::new(make_vbo());
        let layout = InterleavedLayout::new();
        assert!(mgr.allocate(layout).is_none());
    }

    #[test]
    fn test_flush_no_panic() {
        let mgr = InterleavedMemoryManager::new(make_vbo());
        mgr.flush();
    }
}
