//! Span utilities for contiguous element ranges.
//!
//! In Rust, the standard library already provides excellent support for
//! contiguous ranges through slices (`&[T]` and `&mut [T]`). This module
//! provides helper functions that mirror the USD TfSpan API for familiarity.
//!
//! # Rust Slices vs TfSpan
//!
//! USD's `TfSpan<T>` is essentially equivalent to Rust's `&[T]` (for const)
//! or `&mut [T]` (for mutable). The main differences:
//!
//! | TfSpan (C++) | Rust |
//! |--------------|------|
//! | `TfSpan<T>` | `&[T]` or `&mut [T]` |
//! | `TfSpan<const T>` | `&[T]` |
//! | `span.data()` | `slice.as_ptr()` |
//! | `span.size()` | `slice.len()` |
//! | `span[i]` | `slice[i]` |
//! | `span.front()` | `slice.first().unwrap()` |
//! | `span.back()` | `slice.last().unwrap()` |
//! | `span.subspan(off, cnt)` | `&slice[off..off+cnt]` |
//! | `span.first(n)` | `&slice[..n]` |
//! | `span.last(n)` | `&slice[slice.len()-n..]` |
//!
//! # Examples
//!
//! ```
//! use usd_tf::span::{make_span, make_const_span, Span, SpanMut};
//!
//! let mut data = vec![1, 2, 3, 4, 5];
//!
//! // Create a mutable span (equivalent to TfMakeSpan)
//! let span: SpanMut<i32> = make_span(&mut data);
//! span[0] = 10;
//!
//! // Create an immutable span (equivalent to TfMakeConstSpan)
//! let const_span: Span<i32> = make_const_span(&data);
//! assert_eq!(const_span[0], 10);
//!
//! // Subspan operations
//! let sub = &const_span[1..4]; // [2, 3, 4]
//! assert_eq!(sub.len(), 3);
//! ```

/// Type alias for an immutable span (equivalent to `TfSpan<const T>`).
pub type Span<'a, T> = &'a [T];

/// Type alias for a mutable span (equivalent to `TfSpan<T>`).
pub type SpanMut<'a, T> = &'a mut [T];

/// Creates a mutable span from a slice-like container.
///
/// This is equivalent to `TfMakeSpan` in USD.
///
/// # Examples
///
/// ```
/// use usd_tf::span::make_span;
///
/// let mut vec = vec![1, 2, 3];
/// let span = make_span(&mut vec);
/// span[0] = 10;
/// assert_eq!(vec[0], 10);
/// ```
#[inline]
pub fn make_span<T>(slice: &mut [T]) -> SpanMut<'_, T> {
    slice
}

/// Creates an immutable span from a slice-like container.
///
/// This is equivalent to `TfMakeConstSpan` in USD.
///
/// # Examples
///
/// ```
/// use usd_tf::span::make_const_span;
///
/// let vec = vec![1, 2, 3];
/// let span = make_const_span(&vec);
/// assert_eq!(span.len(), 3);
/// ```
#[inline]
pub fn make_const_span<T>(slice: &[T]) -> Span<'_, T> {
    slice
}

/// Extension trait providing TfSpan-like methods on slices.
///
/// These methods provide a familiar API for users coming from USD C++.
pub trait SpanExt<T> {
    /// Returns a pointer to the first element (equivalent to `data()`).
    fn data_ptr(&self) -> *const T;

    /// Returns a subspan starting at `offset` with optional `count`.
    ///
    /// If `count` is `None`, returns from offset to the end.
    ///
    /// # Panics
    ///
    /// Panics if the range is out of bounds.
    fn subspan(&self, offset: usize, count: Option<usize>) -> &[T];

    /// Returns a span of the first `n` elements.
    ///
    /// # Panics
    ///
    /// Panics if `n > len()`.
    fn first_n(&self, n: usize) -> &[T];

    /// Returns a span of the last `n` elements.
    ///
    /// # Panics
    ///
    /// Panics if `n > len()`.
    fn last_n(&self, n: usize) -> &[T];
}

impl<T> SpanExt<T> for [T] {
    #[inline]
    fn data_ptr(&self) -> *const T {
        self.as_ptr()
    }

    #[inline]
    fn subspan(&self, offset: usize, count: Option<usize>) -> &[T] {
        match count {
            Some(n) => &self[offset..offset + n],
            None => &self[offset..],
        }
    }

    #[inline]
    fn first_n(&self, n: usize) -> &[T] {
        &self[..n]
    }

    #[inline]
    fn last_n(&self, n: usize) -> &[T] {
        &self[self.len() - n..]
    }
}

/// Extension trait providing mutable TfSpan-like methods on slices.
pub trait SpanMutExt<T>: SpanExt<T> {
    /// Returns a mutable pointer to the first element.
    fn data_ptr_mut(&mut self) -> *mut T;

    /// Returns a mutable subspan starting at `offset` with optional `count`.
    fn subspan_mut(&mut self, offset: usize, count: Option<usize>) -> &mut [T];

    /// Returns a mutable span of the first `n` elements.
    fn first_n_mut(&mut self, n: usize) -> &mut [T];

    /// Returns a mutable span of the last `n` elements.
    fn last_n_mut(&mut self, n: usize) -> &mut [T];
}

impl<T> SpanMutExt<T> for [T] {
    #[inline]
    fn data_ptr_mut(&mut self) -> *mut T {
        self.as_mut_ptr()
    }

    #[inline]
    fn subspan_mut(&mut self, offset: usize, count: Option<usize>) -> &mut [T] {
        match count {
            Some(n) => &mut self[offset..offset + n],
            None => &mut self[offset..],
        }
    }

    #[inline]
    fn first_n_mut(&mut self, n: usize) -> &mut [T] {
        &mut self[..n]
    }

    #[inline]
    fn last_n_mut(&mut self, n: usize) -> &mut [T] {
        let len = self.len();
        &mut self[len - n..]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_span() {
        let mut data = vec![1, 2, 3, 4, 5];
        let span = make_span(&mut data);
        span[0] = 10;
        assert_eq!(data[0], 10);
    }

    #[test]
    fn test_make_const_span() {
        let data = vec![1, 2, 3, 4, 5];
        let span = make_const_span(&data);
        assert_eq!(span.len(), 5);
        assert_eq!(span[2], 3);
    }

    #[test]
    fn test_data_ptr() {
        let data = [1, 2, 3];
        assert_eq!(data.data_ptr(), data.as_ptr());
    }

    #[test]
    fn test_subspan() {
        let data = [1, 2, 3, 4, 5];

        // Subspan with count
        let sub = data.subspan(1, Some(3));
        assert_eq!(sub, &[2, 3, 4]);

        // Subspan to end
        let sub = data.subspan(2, None);
        assert_eq!(sub, &[3, 4, 5]);
    }

    #[test]
    fn test_first_n() {
        let data = [1, 2, 3, 4, 5];
        assert_eq!(data.first_n(3), &[1, 2, 3]);
        assert_eq!(data.first_n(0), &[] as &[i32]);
        assert_eq!(data.first_n(5), &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_last_n() {
        let data = [1, 2, 3, 4, 5];
        assert_eq!(data.last_n(3), &[3, 4, 5]);
        assert_eq!(data.last_n(0), &[] as &[i32]);
        assert_eq!(data.last_n(5), &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_subspan_mut() {
        let mut data = [1, 2, 3, 4, 5];
        let sub = data.subspan_mut(1, Some(2));
        sub[0] = 20;
        sub[1] = 30;
        assert_eq!(data, [1, 20, 30, 4, 5]);
    }

    #[test]
    fn test_first_n_mut() {
        let mut data = [1, 2, 3, 4, 5];
        let first = data.first_n_mut(2);
        first[0] = 10;
        first[1] = 20;
        assert_eq!(data, [10, 20, 3, 4, 5]);
    }

    #[test]
    fn test_last_n_mut() {
        let mut data = [1, 2, 3, 4, 5];
        let last = data.last_n_mut(2);
        last[0] = 40;
        last[1] = 50;
        assert_eq!(data, [1, 2, 3, 40, 50]);
    }

    #[test]
    fn test_empty_slice() {
        let data: [i32; 0] = [];
        assert_eq!(data.first_n(0), &[] as &[i32]);
        assert_eq!(data.last_n(0), &[] as &[i32]);
        assert_eq!(data.subspan(0, Some(0)), &[] as &[i32]);
    }

    #[test]
    fn test_with_vec() {
        let mut vec = vec![1, 2, 3];
        let span = make_span(&mut vec);
        assert_eq!(span.len(), 3);

        let const_span = make_const_span(&vec);
        assert_eq!(const_span.first_n(2), &[1, 2]);
    }
}
