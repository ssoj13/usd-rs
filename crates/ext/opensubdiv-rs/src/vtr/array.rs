// Copyright 2014 DreamWorks Animation LLC.
// Ported to Rust from OpenSubdiv 3.7.0 vtr/array.h

use std::ops::{Index, IndexMut};

/// Immutable non-owning view into a contiguous slice of `T`.
///
/// Mirrors C++ `Vtr::ConstArray<TYPE>`.  Lifetime `'a` ties the view to the
/// underlying allocation — typically a `Vec` field inside a `Level`.
#[derive(Clone, Copy)]
pub struct ConstArray<'a, T> {
    data: &'a [T],
}

impl<'a, T> ConstArray<'a, T> {
    /// Construct from a raw slice.
    #[inline]
    pub fn new(data: &'a [T]) -> Self {
        Self { data }
    }

    /// Number of elements (i32 to match C++ `size_type`).
    #[inline]
    pub fn size(&self) -> i32 {
        self.data.len() as i32
    }

    /// Returns `true` when there are no elements.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Raw slice access.
    #[inline]
    pub fn as_slice(&self) -> &'a [T] {
        self.data
    }

    /// Iterator over elements.
    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'a, T> {
        self.data.iter()
    }

    /// Search the first four elements for `value`, returning the local index (0-3).
    ///
    /// Mirrors C++ `ConstArray::FindIndexIn4Tuple` which uses a debug `assert`.
    /// In debug builds this panics when not found; in release builds it is
    /// undefined behaviour to call this with a value not present in the tuple
    /// (matching C++ semantics — the caller must guarantee the value exists).
    pub fn find_index_in_4_tuple(&self, value: T) -> i32
    where
        T: PartialEq,
    {
        debug_assert!(
            self.data.len() >= 4,
            "find_index_in_4_tuple: slice too short"
        );
        if self.data[0] == value {
            return 0;
        }
        if self.data[1] == value {
            return 1;
        }
        if self.data[2] == value {
            return 2;
        }
        if self.data[3] == value {
            return 3;
        }
        // SAFETY: caller guarantees value exists in the tuple (debug_assert above).
        debug_assert!(false, "find_index_in_4_tuple: value not found");
        -1 // unreachable in correct usage
    }

    /// Linear search, returning the first matching index or -1.
    pub fn find_index(&self, value: T) -> i32
    where
        T: PartialEq,
    {
        for (i, v) in self.data.iter().enumerate() {
            if *v == value {
                return i as i32;
            }
        }
        -1
    }
}

impl<'a, T> Index<usize> for ConstArray<'a, T> {
    type Output = T;
    #[inline]
    fn index(&self, i: usize) -> &T {
        &self.data[i]
    }
}

impl<'a, T> Index<i32> for ConstArray<'a, T> {
    type Output = T;
    #[inline]
    fn index(&self, i: i32) -> &T {
        &self.data[i as usize]
    }
}

impl<'a, T> IntoIterator for ConstArray<'a, T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.data.iter()
    }
}

// ---------------------------------------------------------------------------

/// Mutable non-owning view into a contiguous slice of `T`.
///
/// Mirrors C++ `Vtr::Array<TYPE>`.  Extends `ConstArray` with write access.
pub struct Array<'a, T> {
    data: &'a mut [T],
}

impl<'a, T> Array<'a, T> {
    /// Construct from a mutable raw slice.
    #[inline]
    pub fn new(data: &'a mut [T]) -> Self {
        Self { data }
    }

    /// Number of elements.
    #[inline]
    pub fn size(&self) -> i32 {
        self.data.len() as i32
    }

    /// Returns `true` when there are no elements.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Immutable slice.
    #[inline]
    pub fn as_slice(&self) -> &[T] {
        self.data
    }

    /// Mutable slice.
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        self.data
    }

    /// Reborrow as a `ConstArray`.
    #[inline]
    pub fn as_const(&self) -> ConstArray<'_, T> {
        ConstArray::new(self.data)
    }

    /// Iterator over elements.
    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.data.iter()
    }

    /// Mutable iterator over elements.
    #[inline]
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, T> {
        self.data.iter_mut()
    }

    /// Linear search, returning the first matching index or -1.
    pub fn find_index(&self, value: T) -> i32
    where
        T: PartialEq,
    {
        for (i, v) in self.data.iter().enumerate() {
            if *v == value {
                return i as i32;
            }
        }
        -1
    }
}

impl<'a, T> Index<usize> for Array<'a, T> {
    type Output = T;
    #[inline]
    fn index(&self, i: usize) -> &T {
        &self.data[i]
    }
}

impl<'a, T> IndexMut<usize> for Array<'a, T> {
    #[inline]
    fn index_mut(&mut self, i: usize) -> &mut T {
        &mut self.data[i]
    }
}

impl<'a, T> Index<i32> for Array<'a, T> {
    type Output = T;
    #[inline]
    fn index(&self, i: i32) -> &T {
        &self.data[i as usize]
    }
}

impl<'a, T> IndexMut<i32> for Array<'a, T> {
    #[inline]
    fn index_mut(&mut self, i: i32) -> &mut T {
        &mut self.data[i as usize]
    }
}

impl<'a, T> IntoIterator for Array<'a, T> {
    type Item = &'a mut T;
    type IntoIter = std::slice::IterMut<'a, T>;
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.data.iter_mut()
    }
}
