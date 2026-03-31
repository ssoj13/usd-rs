//! Zero-initialized memory allocation.
//!
//! Provides utilities for allocating zero-initialized memory,
//! particularly useful for parallel algorithms that require
//! pre-zeroed buffers.
//!
//! # Examples
//!
//! ```
//! use usd_work::{zeroed_vec, zeroed_boxed_slice};
//!
//! // Create a zero-initialized vector
//! let v: Vec<i32> = zeroed_vec(100);
//! assert!(v.iter().all(|&x| x == 0));
//!
//! // Create a zero-initialized boxed slice
//! let s: Box<[f64]> = zeroed_boxed_slice(50);
//! assert!(s.iter().all(|&x| x == 0.0));
//! ```

use std::alloc::{Layout, alloc_zeroed};

/// Create a zero-initialized vector of the given size.
///
/// All elements will be initialized to their zero value.
///
/// # Examples
///
/// ```
/// use usd_work::zeroed_vec;
///
/// let v: Vec<u8> = zeroed_vec(1024);
/// assert_eq!(v.len(), 1024);
/// assert!(v.iter().all(|&x| x == 0));
/// ```
#[must_use]
pub fn zeroed_vec<T: Copy + Default>(len: usize) -> Vec<T> {
    vec![T::default(); len]
}

/// Create a zero-initialized boxed slice.
///
/// More efficient than `zeroed_vec` for fixed-size allocations
/// as it doesn't need capacity tracking.
///
/// # Examples
///
/// ```
/// use usd_work::zeroed_boxed_slice;
///
/// let s: Box<[i32]> = zeroed_boxed_slice(100);
/// assert_eq!(s.len(), 100);
/// ```
#[must_use]
pub fn zeroed_boxed_slice<T: Copy + Default>(len: usize) -> Box<[T]> {
    vec![T::default(); len].into_boxed_slice()
}

/// Allocate raw zero-initialized memory.
///
/// Returns a pointer to `count` zero-initialized elements of type `T`.
///
/// # Safety
///
/// The caller must ensure proper deallocation using the matching layout.
///
/// # Examples
///
/// ```
/// use usd_work::alloc_zeroed_raw;
/// use std::alloc::{dealloc, Layout};
///
/// unsafe {
///     let ptr: *mut u64 = alloc_zeroed_raw(10);
///     assert!(!ptr.is_null());
///     
///     // Check it's zeroed
///     for i in 0..10 {
///         assert_eq!(*ptr.add(i), 0);
///     }
///     
///     // Deallocate
///     let layout = Layout::array::<u64>(10).unwrap();
///     dealloc(ptr as *mut u8, layout);
/// }
/// ```
///
/// # Panics
///
/// Panics if the layout is invalid or allocation fails.
#[must_use]
#[allow(unsafe_code)] // SAFETY: Low-level allocator for zero-initialized memory
pub unsafe fn alloc_zeroed_raw<T>(count: usize) -> *mut T {
    unsafe {
        if count == 0 {
            return std::ptr::NonNull::dangling().as_ptr();
        }

        let layout = Layout::array::<T>(count).expect("Invalid layout");
        let ptr = alloc_zeroed(layout);

        if ptr.is_null() {
            std::alloc::handle_alloc_error(layout);
        }

        ptr as *mut T
    }
}

/// Cache line size for alignment (64 bytes on most modern CPUs).
pub const CACHE_LINE_SIZE: usize = 64;

/// Allocate cache-aligned, zero-initialized memory.
///
/// Returns memory aligned to cache line boundaries for optimal
/// performance in parallel algorithms.
///
/// # Safety
///
/// The caller must ensure proper deallocation.
#[must_use]
#[allow(unsafe_code)] // SAFETY: Cache-aligned allocator for concurrent algorithms
pub unsafe fn alloc_cache_aligned_zeroed<T>(count: usize) -> *mut T {
    unsafe {
        if count == 0 {
            return std::ptr::NonNull::dangling().as_ptr();
        }

        let size = std::mem::size_of::<T>() * count;
        let align = CACHE_LINE_SIZE.max(std::mem::align_of::<T>());

        let layout = Layout::from_size_align(size, align).expect("Invalid layout");
        let ptr = alloc_zeroed(layout);

        if ptr.is_null() {
            std::alloc::handle_alloc_error(layout);
        }

        ptr as *mut T
    }
}

/// A Vec-like container with cache-aligned, zero-initialized storage.
///
/// Useful for parallel algorithms where false sharing needs to be avoided.
#[derive(Debug)]
pub struct CacheAlignedVec<T> {
    ptr: *mut T,
    len: usize,
    capacity: usize,
}

impl<T: Copy + Default> CacheAlignedVec<T> {
    /// Create a new cache-aligned vector with zero-initialized elements.
    #[must_use]
    pub fn new(len: usize) -> Self {
        if len == 0 {
            return Self {
                ptr: std::ptr::NonNull::dangling().as_ptr(),
                len: 0,
                capacity: 0,
            };
        }

        #[allow(unsafe_code)] // SAFETY: Allocating cache-aligned buffer
        let ptr = unsafe { alloc_cache_aligned_zeroed::<T>(len) };

        Self {
            ptr,
            len,
            capacity: len,
        }
    }

    /// Returns the number of elements.
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns true if empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns a slice of the elements.
    #[must_use]
    pub fn as_slice(&self) -> &[T] {
        if self.len == 0 {
            &[]
        } else {
            #[allow(unsafe_code)] // SAFETY: ptr is valid for len elements
            unsafe {
                std::slice::from_raw_parts(self.ptr, self.len)
            }
        }
    }

    /// Returns a mutable slice of the elements.
    #[must_use]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        if self.len == 0 {
            &mut []
        } else {
            #[allow(unsafe_code)] // SAFETY: ptr is valid for len elements, exclusive access
            unsafe {
                std::slice::from_raw_parts_mut(self.ptr, self.len)
            }
        }
    }

    /// Get element at index.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&T> {
        if index < self.len {
            #[allow(unsafe_code)] // SAFETY: index < len, ptr valid
            Some(unsafe { &*self.ptr.add(index) })
        } else {
            None
        }
    }

    /// Get mutable element at index.
    #[must_use]
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        if index < self.len {
            #[allow(unsafe_code)] // SAFETY: index < len, exclusive access
            Some(unsafe { &mut *self.ptr.add(index) })
        } else {
            None
        }
    }
}

impl<T> Drop for CacheAlignedVec<T> {
    fn drop(&mut self) {
        if self.capacity > 0 {
            let size = std::mem::size_of::<T>() * self.capacity;
            let align = CACHE_LINE_SIZE.max(std::mem::align_of::<T>());
            let layout =
                Layout::from_size_align(size, align).expect("Invalid layout for CacheAlignedVec");
            #[allow(unsafe_code)] // SAFETY: Deallocating buffer with matching layout
            unsafe {
                std::alloc::dealloc(self.ptr as *mut u8, layout);
            }
        }
    }
}

impl<T: Copy + Default> std::ops::Index<usize> for CacheAlignedVec<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        self.get(index).expect("Index out of bounds")
    }
}

impl<T: Copy + Default> std::ops::IndexMut<usize> for CacheAlignedVec<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.get_mut(index).expect("Index out of bounds")
    }
}

// SAFETY: CacheAlignedVec owns its data exclusively; Send/Sync if T is Send/Sync.
// The raw pointer is managed correctly (allocated/deallocated) within the type.
#[allow(unsafe_code)]
unsafe impl<T: Send> Send for CacheAlignedVec<T> {}
#[allow(unsafe_code)]
unsafe impl<T: Sync> Sync for CacheAlignedVec<T> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zeroed_vec() {
        let v: Vec<i32> = zeroed_vec(100);
        assert_eq!(v.len(), 100);
        assert!(v.iter().all(|&x| x == 0));
    }

    #[test]
    fn test_zeroed_vec_floats() {
        let v: Vec<f64> = zeroed_vec(50);
        assert_eq!(v.len(), 50);
        assert!(v.iter().all(|&x| x == 0.0));
    }

    #[test]
    fn test_zeroed_boxed_slice() {
        let s: Box<[u8]> = zeroed_boxed_slice(256);
        assert_eq!(s.len(), 256);
        assert!(s.iter().all(|&x| x == 0));
    }

    #[test]
    fn test_alloc_zeroed_raw() {
        unsafe {
            let ptr: *mut u32 = alloc_zeroed_raw(10);
            assert!(!ptr.is_null());

            for i in 0..10 {
                assert_eq!(*ptr.add(i), 0);
            }

            let layout = Layout::array::<u32>(10).unwrap();
            std::alloc::dealloc(ptr as *mut u8, layout);
        }
    }

    #[test]
    fn test_cache_aligned_vec() {
        let v: CacheAlignedVec<i32> = CacheAlignedVec::new(100);
        assert_eq!(v.len(), 100);
        assert!(v.as_slice().iter().all(|&x| x == 0));
    }

    #[test]
    fn test_cache_aligned_vec_mutation() {
        let mut v: CacheAlignedVec<i32> = CacheAlignedVec::new(10);
        v[0] = 42;
        v[9] = 100;

        assert_eq!(v[0], 42);
        assert_eq!(v[9], 100);
        assert_eq!(v[5], 0);
    }

    #[test]
    fn test_cache_aligned_vec_empty() {
        let v: CacheAlignedVec<i32> = CacheAlignedVec::new(0);
        assert!(v.is_empty());
        assert_eq!(v.len(), 0);
    }

    #[test]
    fn test_zeroed_vec_empty() {
        let v: Vec<i32> = zeroed_vec(0);
        assert!(v.is_empty());
    }
}
