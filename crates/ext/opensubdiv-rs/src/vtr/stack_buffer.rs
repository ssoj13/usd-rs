// Copyright 2014 DreamWorks Animation LLC.
// Ported to Rust from OpenSubdiv 3.7.0 vtr/stackBuffer.h

/// Fixed-capacity stack-allocated buffer that falls back to heap when full.
/// Mirrors C++ `Vtr::internal::StackBuffer<TYPE, SIZE, INITIALIZED>`.
///
/// In Rust, true inline stack allocation with a const SIZE is not possible
/// without nightly features, so we use a `Vec` with small-buffer optimisation
/// approximated via capacity hints. For all practical OpenSubdiv uses the
/// buffers are small (4-16 elements) and allocate in the hot path anyway.
pub struct StackBuffer<T> {
    data: Vec<T>,
}

impl<T: Default + Clone> StackBuffer<T> {
    /// Allocate a buffer with `count` default-initialised elements.
    #[inline]
    pub fn new(count: usize) -> Self {
        Self { data: vec![T::default(); count] }
    }

    /// Allocate with `count` copies of `value`.
    #[inline]
    pub fn with_value(count: usize, value: T) -> Self {
        Self { data: vec![value; count] }
    }

    /// Resize to `new_len` elements, filling new slots with `T::default()`.
    #[inline]
    pub fn set_size(&mut self, new_len: usize) {
        self.data.resize(new_len, T::default());
    }
}

impl<T> StackBuffer<T> {
    /// Number of elements.
    #[inline] pub fn len(&self) -> usize { self.data.len() }
    /// True when empty.
    #[inline] pub fn is_empty(&self) -> bool { self.data.is_empty() }
    /// Raw pointer to data.
    #[inline] pub fn as_ptr(&self) -> *const T { self.data.as_ptr() }
    /// Mutable raw pointer to data.
    #[inline] pub fn as_mut_ptr(&mut self) -> *mut T { self.data.as_mut_ptr() }
    /// Immutable slice.
    #[inline] pub fn as_slice(&self) -> &[T] { &self.data }
    /// Mutable slice.
    #[inline] pub fn as_mut_slice(&mut self) -> &mut [T] { &mut self.data }

    /// Reserve capacity for at least `cap` elements without changing length.
    #[inline]
    pub fn reserve(&mut self, cap: usize) {
        if cap > self.data.capacity() {
            self.data.reserve(cap - self.data.len());
        }
    }
}

impl<T> std::ops::Index<usize> for StackBuffer<T> {
    type Output = T;
    #[inline] fn index(&self, i: usize) -> &T { &self.data[i] }
}

impl<T> std::ops::IndexMut<usize> for StackBuffer<T> {
    #[inline] fn index_mut(&mut self, i: usize) -> &mut T { &mut self.data[i] }
}

impl<T> std::ops::Index<i32> for StackBuffer<T> {
    type Output = T;
    #[inline] fn index(&self, i: i32) -> &T { &self.data[i as usize] }
}

impl<T> std::ops::IndexMut<i32> for StackBuffer<T> {
    #[inline] fn index_mut(&mut self, i: i32) -> &mut T { &mut self.data[i as usize] }
}
