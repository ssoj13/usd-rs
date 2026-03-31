//! Trace data buffer.
//!
//! Port of pxr/base/trace/dataBuffer.h
//!
//! This module provides a buffer for storing copies of data associated
//! with TraceEvent instances.

use std::alloc::{Layout, alloc, dealloc};
use std::cell::UnsafeCell;
use std::ptr::NonNull;

// ============================================================================
// Constants
// ============================================================================

/// Default allocation block size.
pub const DEFAULT_ALLOC_SIZE: usize = 1024;

// ============================================================================
// Data Buffer
// ============================================================================

/// A buffer for storing copies of data associated with TraceEvent instances.
///
/// Data stored in the buffer must be Copy (for simplicity in Rust).
/// The buffer uses a bump allocator for efficient allocation without
/// individual deallocations.
pub struct DataBuffer {
    /// The internal allocator.
    alloc: UnsafeCell<Allocator>,
}

impl DataBuffer {
    /// Creates a new data buffer with the default allocation size.
    pub fn new() -> Self {
        Self::with_block_size(DEFAULT_ALLOC_SIZE)
    }

    /// Creates a new data buffer with the specified block size.
    pub fn with_block_size(block_size: usize) -> Self {
        Self {
            alloc: UnsafeCell::new(Allocator::new(block_size)),
        }
    }

    /// Gets mutable access to the internal allocator.
    ///
    /// # Safety
    /// - Caller must ensure no other references to the allocator exist
    /// - This is safe in practice because DataBuffer is !Sync
    #[allow(unsafe_code)]
    fn get_alloc_mut(&self) -> &mut Allocator {
        unsafe { &mut *self.alloc.get() }
    }

    /// Allocates raw bytes with the given alignment and size.
    fn allocate_bytes(&self, align: usize, size: usize) -> *mut u8 {
        self.get_alloc_mut().allocate(align, size)
    }

    /// Makes a copy of the value and returns a pointer to it.
    ///
    /// # Safety
    /// The returned pointer is valid for the lifetime of the DataBuffer.
    /// The type T must be Copy and have no drop glue.
    #[allow(unsafe_code)]
    pub fn store<T: Copy>(&self, value: T) -> *const T {
        let ptr = self.allocate_bytes(std::mem::align_of::<T>(), std::mem::size_of::<T>());
        unsafe {
            let typed_ptr = ptr as *mut T;
            typed_ptr.write(value);
            typed_ptr as *const T
        }
    }

    /// Makes a copy of the string and returns a pointer to it.
    #[allow(unsafe_code)]
    pub fn store_str(&self, s: &str) -> *const str {
        let bytes = s.as_bytes();
        let len = bytes.len();

        // Allocate space for the string bytes
        let ptr = self.allocate_bytes(1, len);
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, len);
            let slice = std::slice::from_raw_parts(ptr, len);
            std::str::from_utf8_unchecked(slice)
        }
    }

    /// Makes a copy of the C string and returns a pointer to it.
    #[allow(unsafe_code)]
    pub fn store_cstr(&self, s: &str) -> *const u8 {
        let bytes = s.as_bytes();
        let len = bytes.len() + 1; // +1 for null terminator

        let ptr = self.allocate_bytes(1, len);
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
            ptr.add(bytes.len()).write(0); // null terminator
            ptr
        }
    }
}

impl Default for DataBuffer {
    fn default() -> Self {
        Self::new()
    }
}

// DataBuffer is not Sync because it uses UnsafeCell
// But it can be Send if we're careful
#[allow(unsafe_code)]
unsafe impl Send for DataBuffer {}

// ============================================================================
// Block
// ============================================================================

/// A single memory block in the allocator.
struct Block {
    /// Pointer to the allocated memory.
    ptr: NonNull<u8>,
    /// Layout used for allocation.
    layout: Layout,
}

impl Block {
    /// Creates a new block with the given size.
    #[allow(unsafe_code)]
    fn new(size: usize) -> Option<Self> {
        let layout = Layout::from_size_align(size, 8).ok()?;
        let ptr = unsafe { alloc(layout) };
        NonNull::new(ptr).map(|ptr| Self { ptr, layout })
    }

    /// Returns a pointer to the start of the block.
    fn as_ptr(&self) -> *mut u8 {
        self.ptr.as_ptr()
    }

    /// Returns the size of the block.
    fn size(&self) -> usize {
        self.layout.size()
    }
}

impl Drop for Block {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        unsafe {
            dealloc(self.ptr.as_ptr(), self.layout);
        }
    }
}

// ============================================================================
// Allocator
// ============================================================================

/// Simple bump allocator that only supports allocations, not frees.
///
/// Allocated memory is tied to the lifetime of the allocator object.
struct Allocator {
    /// Current position in the current block.
    next: *mut u8,
    /// End of the current block.
    block_end: *mut u8,
    /// All allocated blocks.
    blocks: Vec<Block>,
    /// Desired block size for new allocations.
    desired_block_size: usize,
}

impl Allocator {
    /// Creates a new allocator with the given block size.
    fn new(block_size: usize) -> Self {
        Self {
            next: std::ptr::null_mut(),
            block_end: std::ptr::null_mut(),
            blocks: Vec::new(),
            desired_block_size: block_size.max(64),
        }
    }

    /// Allocates memory with the given alignment and size.
    #[allow(unsafe_code)]
    fn allocate(&mut self, align: usize, size: usize) -> *mut u8 {
        let aligned_next = Self::align_pointer(self.next, align);
        let end = unsafe { aligned_next.add(size) };

        if end > self.block_end || self.next.is_null() {
            self.allocate_block(align, size);
            let aligned_next = Self::align_pointer(self.next, align);
            self.next = unsafe { aligned_next.add(size) };
            return aligned_next;
        }

        self.next = end;
        aligned_next
    }

    /// Allocates a new block.
    #[allow(unsafe_code)]
    fn allocate_block(&mut self, align: usize, desired_size: usize) {
        // Calculate block size: max of desired block size and required size
        let required_size = desired_size + align - 1; // extra for alignment
        let block_size = self.desired_block_size.max(required_size);

        if let Some(block) = Block::new(block_size) {
            self.next = block.as_ptr();
            self.block_end = unsafe { block.as_ptr().add(block.size()) };
            self.blocks.push(block);
        } else {
            panic!("Failed to allocate memory block of size {}", block_size);
        }
    }

    /// Aligns a pointer to the given alignment.
    #[allow(unsafe_code)]
    fn align_pointer(ptr: *mut u8, align: usize) -> *mut u8 {
        let offset = ptr.align_offset(align);
        if offset == usize::MAX {
            // Alignment not possible, fall back to manual calculation
            let addr = ptr as usize;
            let align_mask = align - 1;
            let aligned = (addr + align_mask) & !align_mask;
            aligned as *mut u8
        } else {
            unsafe { ptr.add(offset) }
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_primitive() {
        let buffer = DataBuffer::new();

        let ptr1 = buffer.store(42i32);
        let ptr2 = buffer.store(3.14f64);
        let ptr3 = buffer.store(true);

        unsafe {
            assert_eq!(*ptr1, 42);
            assert!(((*ptr2) - 3.14).abs() < 1e-10);
            assert!(*ptr3);
        }
    }

    #[test]
    fn test_store_many() {
        let buffer = DataBuffer::with_block_size(64);

        // Store more data than fits in one block
        let ptrs: Vec<_> = (0..100).map(|i| buffer.store(i as u64)).collect();

        for (i, ptr) in ptrs.iter().enumerate() {
            unsafe {
                assert_eq!(**ptr, i as u64);
            }
        }
    }

    #[test]
    fn test_store_str() {
        let buffer = DataBuffer::new();

        let ptr = buffer.store_str("Hello, World!");
        unsafe {
            assert_eq!(&*ptr, "Hello, World!");
        }
    }

    #[test]
    fn test_store_cstr() {
        let buffer = DataBuffer::new();

        let ptr = buffer.store_cstr("test");
        unsafe {
            // Check null terminator
            assert_eq!(*ptr.add(4), 0);
            // Check content
            let bytes = std::slice::from_raw_parts(ptr, 4);
            assert_eq!(bytes, b"test");
        }
    }

    #[test]
    fn test_alignment() {
        let buffer = DataBuffer::new();

        // Store items that require different alignments
        buffer.store(1u8);
        let ptr64 = buffer.store(42u64);
        buffer.store(2u8);
        let ptr32 = buffer.store(100i32);

        // Check alignment
        assert_eq!((ptr64 as usize) % std::mem::align_of::<u64>(), 0);
        assert_eq!((ptr32 as usize) % std::mem::align_of::<i32>(), 0);

        unsafe {
            assert_eq!(*ptr64, 42u64);
            assert_eq!(*ptr32, 100i32);
        }
    }

    #[test]
    fn test_struct() {
        #[derive(Copy, Clone, Debug, PartialEq)]
        struct Point {
            x: f64,
            y: f64,
        }

        let buffer = DataBuffer::new();
        let pt = Point { x: 1.0, y: 2.0 };
        let ptr = buffer.store(pt);

        unsafe {
            assert_eq!(*ptr, pt);
        }
    }
}
