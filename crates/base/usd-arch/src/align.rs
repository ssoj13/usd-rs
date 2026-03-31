// SAFETY: This module provides FFI bindings to system APIs requiring unsafe
#![allow(unsafe_code)]

//! Memory alignment utilities.
//!
//! Provides functions for aligned memory allocation and size calculation.
//! Matches C++ `pxr/base/arch/align.h` API. On Unix uses `posix_memalign`+`free`;
//! on Windows uses `_aligned_malloc`+`_aligned_free` for C++ parity.

use std::alloc::{Layout, alloc, dealloc};
use std::ptr::NonNull;

/// Default alignment for memory allocations (8 bytes).
pub const DEFAULT_ALIGNMENT: usize = 8;

/// Maximum extra bytes that `align_memory_size` can add.
pub const MAX_ALIGNMENT_INCREASE: usize = 7;

/// Returns the size rounded up to the nearest 8-byte boundary.
///
/// This is useful for calculating how much memory will actually be consumed
/// by an allocation request.
///
/// # Examples
///
/// ```
/// use usd_arch::align_memory_size;
///
/// assert_eq!(align_memory_size(1), 8);
/// assert_eq!(align_memory_size(8), 8);
/// assert_eq!(align_memory_size(9), 16);
/// ```
#[inline]
#[must_use]
pub const fn align_memory_size(n_bytes: usize) -> usize {
    (n_bytes + 7) & !0x7
}

/// Aligns a pointer to the next 8-byte boundary.
///
/// # Safety
///
/// The returned pointer may point beyond the original allocation if the
/// original pointer was not aligned. Caller must ensure sufficient space.
#[inline]
#[must_use]
pub fn align_memory<T>(ptr: *mut T) -> *mut T {
    let offset = (ptr as *const u8).align_offset(DEFAULT_ALIGNMENT);
    if offset == usize::MAX {
        // Alignment not possible, fall back to manual calculation
        let addr = ptr as usize;
        let aligned = (addr + 7) & !0x7;
        aligned as *mut T
    } else {
        unsafe { (ptr as *mut u8).add(offset) as *mut T }
    }
}

/// Allocates memory with the specified alignment.
///
/// Uses `posix_memalign` on Unix and `_aligned_malloc` on Windows for C++ parity
/// with `ArchAlignedAlloc`. The returned pointer must be freed with `aligned_free`.
///
/// # Arguments
///
/// * `alignment` - The alignment requirement (must be a power of 2, >= sizeof(void*))
/// * `size` - The number of bytes to allocate
///
/// # Returns
///
/// Returns `Some(NonNull<u8>)` on success, `None` on failure.
///
/// # Safety
///
/// The returned memory must be deallocated using `aligned_free`.
#[must_use]
pub fn aligned_alloc(alignment: usize, size: usize) -> Option<NonNull<u8>> {
    aligned_alloc_impl(alignment, size)
}

#[cfg(unix)]
fn aligned_alloc_impl(alignment: usize, size: usize) -> Option<NonNull<u8>> {
    if size == 0 {
        return None;
    }
    let alignment = alignment.max(std::mem::align_of::<*const ()>());
    if !alignment.is_power_of_two() {
        return None;
    }
    let mut ptr: *mut libc::c_void = std::ptr::null_mut();
    let ret = unsafe { libc::posix_memalign(&mut ptr, alignment, size) };
    if ret == 0 && !ptr.is_null() {
        NonNull::new(ptr as *mut u8)
    } else {
        None
    }
}

#[cfg(windows)]
fn aligned_alloc_impl(alignment: usize, size: usize) -> Option<NonNull<u8>> {
    if size == 0 {
        return None;
    }
    let alignment = alignment.max(std::mem::align_of::<*const ()>());
    if !alignment.is_power_of_two() {
        return None;
    }
    unsafe extern "C" {
        fn _aligned_malloc(size: usize, alignment: usize) -> *mut std::ffi::c_void;
    }
    let ptr = unsafe { _aligned_malloc(size, alignment) };
    NonNull::new(ptr as *mut u8)
}

/// Frees memory allocated by `aligned_alloc`.
///
/// Uses `free` on Unix and `_aligned_free` on Windows. Matches C++ `ArchAlignedFree`.
///
/// # Safety
///
/// - `ptr` must have been allocated by `aligned_alloc`
/// - `ptr` must not have been freed already
pub unsafe fn aligned_free(ptr: NonNull<u8>) {
    unsafe {
        aligned_free_impl(ptr);
    }
}

#[cfg(unix)]
unsafe fn aligned_free_impl(ptr: NonNull<u8>) {
    unsafe {
        libc::free(ptr.as_ptr() as *mut libc::c_void);
    }
}

#[cfg(windows)]
unsafe fn aligned_free_impl(ptr: NonNull<u8>) {
    unsafe extern "C" {
        fn _aligned_free(ptr: *mut std::ffi::c_void);
    }
    unsafe {
        _aligned_free(ptr.as_ptr() as *mut std::ffi::c_void);
    }
}

/// A wrapper around aligned memory allocation that tracks the layout.
#[derive(Debug)]
pub struct AlignedBox {
    ptr: NonNull<u8>,
    layout: Layout,
}

impl AlignedBox {
    /// Allocates a new aligned memory block.
    ///
    /// # Arguments
    ///
    /// * `alignment` - The alignment requirement (must be a power of 2)
    /// * `size` - The number of bytes to allocate
    ///
    /// # Returns
    ///
    /// Returns `Some(AlignedBox)` on success, `None` on failure.
    #[must_use]
    pub fn new(alignment: usize, size: usize) -> Option<Self> {
        if size == 0 {
            return None;
        }

        let alignment = alignment.max(std::mem::align_of::<*const ()>());
        if !alignment.is_power_of_two() {
            return None;
        }

        let layout = Layout::from_size_align(size, alignment).ok()?;

        // SAFETY: Layout is valid
        let ptr = unsafe { alloc(layout) };
        let ptr = NonNull::new(ptr)?;

        Some(Self { ptr, layout })
    }

    /// Returns a pointer to the allocated memory.
    #[inline]
    #[must_use]
    pub fn as_ptr(&self) -> *mut u8 {
        self.ptr.as_ptr()
    }

    /// Returns the size of the allocation.
    #[inline]
    #[must_use]
    pub fn size(&self) -> usize {
        self.layout.size()
    }

    /// Returns the alignment of the allocation.
    #[inline]
    #[must_use]
    pub fn alignment(&self) -> usize {
        self.layout.align()
    }
}

impl Drop for AlignedBox {
    fn drop(&mut self) {
        // SAFETY: ptr was allocated with this layout
        unsafe {
            dealloc(self.ptr.as_ptr(), self.layout);
        }
    }
}

// SAFETY: The raw pointer is uniquely owned
unsafe impl Send for AlignedBox {}
unsafe impl Sync for AlignedBox {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_align_memory_size() {
        assert_eq!(align_memory_size(0), 0);
        assert_eq!(align_memory_size(1), 8);
        assert_eq!(align_memory_size(7), 8);
        assert_eq!(align_memory_size(8), 8);
        assert_eq!(align_memory_size(9), 16);
        assert_eq!(align_memory_size(16), 16);
        assert_eq!(align_memory_size(17), 24);
    }

    #[test]
    fn test_aligned_box() {
        let aligned = AlignedBox::new(64, 1024).expect("allocation failed");
        assert_eq!(aligned.size(), 1024);
        assert_eq!(aligned.alignment(), 64);
        assert_eq!(aligned.as_ptr() as usize % 64, 0);
    }

    #[test]
    fn test_aligned_box_zero_size() {
        assert!(AlignedBox::new(64, 0).is_none());
    }

    #[test]
    fn test_aligned_alloc() {
        if let Some(ptr) = aligned_alloc(64, 1024) {
            assert_eq!(ptr.as_ptr() as usize % 64, 0);
            unsafe { aligned_free(ptr) };
        }
    }
}
