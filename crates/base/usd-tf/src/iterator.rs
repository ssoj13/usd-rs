//! Iterator utilities for USD-style container traversal.
//!
//! This module provides iterator adapters that match the USD TfIterator API
//! for users familiar with the C++ codebase. In Rust, standard iterators are
//! typically preferred, but these types provide a bridge for ported code.
//!
//! # USD C++ vs Rust Iterators
//!
//! | TfIterator (C++) | Rust |
//! |------------------|------|
//! | `TfIterator<T> i(container)` | `container.iter()` |
//! | `while (i) { ... ++i }` | `for item in container.iter() { ... }` |
//! | `TF_FOR_ALL(i, c)` | `for i in &c` |
//! | `TF_REVERSE_FOR_ALL(i, c)` | `for i in c.iter().rev()` |
//!
//! # Examples
//!
//! ```
//! use usd_tf::iterator::{TfIterator, tf_for_all};
//!
//! let data = vec![1, 2, 3, 4, 5];
//!
//! // USD-style iteration
//! let mut iter = TfIterator::new(&data);
//! while iter.is_valid() {
//!     let _val = iter.next();
//! }
//!
//! // Using the macro (preferred for familiarity)
//! tf_for_all!(i, data, {
//!     println!("{}", i);
//! });
//! ```

use std::iter::{DoubleEndedIterator, ExactSizeIterator, FusedIterator};
use std::marker::PhantomData;

/// Iterator adapter providing USD TfIterator-like semantics.
///
/// This wraps a standard Rust iterator and provides methods that mirror
/// the TfIterator C++ API for code porting familiarity.
///
/// # Type Parameters
///
/// * `I` - The underlying iterator type
///
/// # Examples
///
/// ```
/// use usd_tf::iterator::TfIterator;
///
/// let vec = vec![10, 20, 30];
/// let mut iter = TfIterator::new(&vec);
///
/// assert!(iter.is_valid());
/// assert_eq!(iter.get(), Some(&10));
///
/// iter.advance();
/// assert_eq!(iter.get(), Some(&20));
/// ```
pub struct TfIterator<I>
where
    I: Iterator,
{
    /// The underlying iterator.
    iter: I,
    /// Current cached item (for peeking semantics).
    current: Option<I::Item>,
    /// Whether we've exhausted the iterator.
    exhausted: bool,
}

impl<I> TfIterator<I>
where
    I: Iterator,
{
    /// Creates a new TfIterator from any iterator.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::iterator::TfIterator;
    ///
    /// let data = vec![1, 2, 3];
    /// let iter = TfIterator::wrap(data.iter());
    /// assert!(iter.is_valid());
    /// ```
    #[inline]
    pub fn wrap(mut iter: I) -> Self {
        let current = iter.next();
        let exhausted = current.is_none();
        Self {
            iter,
            current,
            exhausted,
        }
    }

    /// Returns true if the iterator has more elements.
    ///
    /// This is equivalent to the C++ `operator bool()` conversion.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::iterator::TfIterator;
    ///
    /// let data = vec![1];
    /// let mut iter = TfIterator::new(&data);
    /// assert!(iter.is_valid());
    /// iter.advance();
    /// assert!(!iter.is_valid());
    /// ```
    #[inline]
    pub fn is_valid(&self) -> bool {
        !self.exhausted
    }

    /// Returns true if the iterator is exhausted.
    ///
    /// This is equivalent to the C++ `operator!()`.
    #[inline]
    pub fn is_exhausted(&self) -> bool {
        self.exhausted
    }

    /// Advances the iterator to the next element.
    ///
    /// This is equivalent to the C++ `operator++()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::iterator::TfIterator;
    ///
    /// let data = vec![1, 2, 3];
    /// let mut iter = TfIterator::new(&data);
    /// assert_eq!(iter.get(), Some(&1));
    /// iter.advance();
    /// assert_eq!(iter.get(), Some(&2));
    /// ```
    #[inline]
    pub fn advance(&mut self) {
        if self.exhausted {
            return;
        }
        self.current = self.iter.next();
        self.exhausted = self.current.is_none();
    }

    /// Returns the current element without advancing.
    ///
    /// Returns `None` if the iterator is exhausted.
    /// For slice iterators, this returns `&T` (not `&&T`).
    #[inline]
    pub fn get(&self) -> Option<I::Item>
    where
        I::Item: Copy,
    {
        self.current
    }

    /// Returns a reference to the current element without advancing.
    ///
    /// Returns `None` if the iterator is exhausted.
    #[inline]
    pub fn get_ref(&self) -> Option<&I::Item> {
        self.current.as_ref()
    }

    /// Takes the current element and advances to the next.
    ///
    /// This is similar to the standard `Iterator::next()` but matches
    /// the post-increment semantics of C++ TfIterator.
    #[inline]
    pub fn next_item(&mut self) -> Option<I::Item>
    where
        I::Item: Clone,
    {
        if self.exhausted {
            return None;
        }
        let result = self.current.clone();
        self.advance();
        result
    }

    /// Consumes and returns the current element, advancing to the next.
    ///
    /// Unlike `next_item()`, this takes ownership without cloning.
    #[inline]
    pub fn take_current(&mut self) -> Option<I::Item> {
        if self.exhausted {
            return None;
        }
        let result = self.current.take();
        self.current = self.iter.next();
        self.exhausted = self.current.is_none();
        result
    }
}

impl<'a, T> TfIterator<std::slice::Iter<'a, T>> {
    /// Creates a TfIterator over a slice or collection.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::iterator::TfIterator;
    ///
    /// let vec = vec![1, 2, 3];
    /// let iter = TfIterator::new(&vec);
    /// assert!(iter.is_valid());
    /// ```
    #[inline]
    pub fn new<C: AsRef<[T]> + ?Sized>(container: &'a C) -> Self {
        Self::wrap(container.as_ref().iter())
    }
}

impl<'a, T> TfIterator<std::slice::IterMut<'a, T>> {
    /// Creates a mutable TfIterator over a slice or collection.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::iterator::TfIterator;
    ///
    /// let mut vec = vec![1, 2, 3];
    /// let mut iter = TfIterator::new_mut(&mut vec);
    /// if let Some(val) = iter.get_mut() {
    ///     *val = 10;
    /// }
    /// assert_eq!(vec[0], 10);
    /// ```
    #[inline]
    pub fn new_mut(container: &'a mut [T]) -> Self {
        Self::wrap(container.iter_mut())
    }

    /// Returns a mutable reference to the current element.
    #[inline]
    pub fn get_mut(&mut self) -> Option<&mut T> {
        self.current.as_deref_mut()
    }
}

impl<I> Iterator for TfIterator<I>
where
    I: Iterator,
    I::Item: Clone,
{
    type Item = I::Item;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.next_item()
    }
}

/// Reverse iterator adapter matching TfIterator API.
///
/// Provides the same interface as [`TfIterator`] but iterates in reverse.
///
/// # Examples
///
/// ```
/// use usd_tf::iterator::TfReverseIterator;
///
/// let data = vec![1, 2, 3];
/// let mut iter = TfReverseIterator::new(&data);
///
/// assert_eq!(iter.get(), Some(&3));
/// iter.advance();
/// assert_eq!(iter.get(), Some(&2));
/// ```
pub struct TfReverseIterator<I>
where
    I: DoubleEndedIterator,
{
    /// The underlying reversed iterator.
    iter: std::iter::Rev<I>,
    /// Current cached item.
    current: Option<I::Item>,
    /// Whether exhausted.
    exhausted: bool,
}

impl<I> TfReverseIterator<I>
where
    I: DoubleEndedIterator,
{
    /// Creates a new reverse iterator from a double-ended iterator.
    #[inline]
    pub fn wrap(iter: I) -> Self {
        let mut rev = iter.rev();
        let current = rev.next();
        let exhausted = current.is_none();
        Self {
            iter: rev,
            current,
            exhausted,
        }
    }

    /// Returns true if the iterator has more elements.
    #[inline]
    pub fn is_valid(&self) -> bool {
        !self.exhausted
    }

    /// Returns true if the iterator is exhausted.
    #[inline]
    pub fn is_exhausted(&self) -> bool {
        self.exhausted
    }

    /// Advances the iterator to the next (previous in original order) element.
    #[inline]
    pub fn advance(&mut self) {
        if self.exhausted {
            return;
        }
        self.current = self.iter.next();
        self.exhausted = self.current.is_none();
    }

    /// Returns the current element.
    #[inline]
    pub fn get(&self) -> Option<I::Item>
    where
        I::Item: Copy,
    {
        self.current
    }

    /// Returns a reference to the current element.
    #[inline]
    pub fn get_ref(&self) -> Option<&I::Item> {
        self.current.as_ref()
    }

    /// Takes the current element and advances.
    #[inline]
    pub fn next_item(&mut self) -> Option<I::Item>
    where
        I::Item: Clone,
    {
        if self.exhausted {
            return None;
        }
        let result = self.current.clone();
        self.advance();
        result
    }

    /// Takes ownership of current element and advances.
    #[inline]
    pub fn take(&mut self) -> Option<I::Item> {
        if self.exhausted {
            return None;
        }
        let result = self.current.take();
        self.current = self.iter.next();
        self.exhausted = self.current.is_none();
        result
    }
}

impl<'a, T> TfReverseIterator<std::slice::Iter<'a, T>> {
    /// Creates a reverse TfIterator over a slice.
    #[inline]
    pub fn new<C: AsRef<[T]> + ?Sized>(container: &'a C) -> Self {
        Self::wrap(container.as_ref().iter())
    }
}

impl<'a, T> TfReverseIterator<std::slice::IterMut<'a, T>> {
    /// Creates a mutable reverse TfIterator over a slice.
    #[inline]
    pub fn new_mut(container: &'a mut [T]) -> Self {
        Self::wrap(container.iter_mut())
    }

    /// Returns a mutable reference to the current element.
    #[inline]
    pub fn get_mut(&mut self) -> Option<&mut T> {
        self.current.as_deref_mut()
    }
}

impl<I> Iterator for TfReverseIterator<I>
where
    I: DoubleEndedIterator,
    I::Item: Clone,
{
    type Item = I::Item;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.next_item()
    }
}

/// Creates a forward TfIterator from a container reference.
///
/// This is equivalent to `TfMakeIterator` in C++.
///
/// # Examples
///
/// ```
/// use usd_tf::iterator::make_iterator;
///
/// let data = vec![1, 2, 3];
/// let iter = make_iterator(&data);
/// assert!(iter.is_valid());
/// ```
#[inline]
pub fn make_iterator<'a, T>(container: &'a [T]) -> TfIterator<std::slice::Iter<'a, T>> {
    TfIterator::new(container)
}

/// Creates a reverse TfIterator from a container reference.
///
/// This is equivalent to `TfMakeReverseIterator` in C++.
///
/// # Examples
///
/// ```
/// use usd_tf::iterator::make_reverse_iterator;
///
/// let data = vec![1, 2, 3];
/// let mut iter = make_reverse_iterator(&data);
/// assert_eq!(iter.get(), Some(&3));
/// ```
#[inline]
pub fn make_reverse_iterator<'a, T>(
    container: &'a [T],
) -> TfReverseIterator<std::slice::Iter<'a, T>> {
    TfReverseIterator::new(container)
}

/// Returns the number of elements in a statically sized array.
///
/// This is equivalent to `TfArraySize` in C++ (and C++17's `std::size()`).
///
/// # Examples
///
/// ```
/// use usd_tf::iterator::array_size;
///
/// let arr = [1, 2, 3, 4, 5];
/// assert_eq!(array_size(&arr), 5);
///
/// let empty: [i32; 0] = [];
/// assert_eq!(array_size(&empty), 0);
/// ```
#[inline]
pub const fn array_size<T, const N: usize>(_array: &[T; N]) -> usize {
    N
}

/// Macro for iterating over a container in USD style.
///
/// This provides a familiar interface for code ported from C++ USD.
///
/// # Syntax
///
/// ```text
/// tf_for_all!(item, container, { body });
/// ```
///
/// # Examples
///
/// ```
/// use usd_tf::iterator::tf_for_all;
///
/// let data = vec![1, 2, 3];
/// let mut sum = 0;
///
/// tf_for_all!(i, data, {
///     sum += i;
/// });
///
/// assert_eq!(sum, 6);
/// ```
#[macro_export]
macro_rules! tf_for_all {
    ($item:ident, $container:expr, $body:block) => {
        for $item in &$container $body
    };
}

/// Macro for iterating over a container in reverse, USD style.
///
/// # Examples
///
/// ```
/// use usd_tf::iterator::tf_reverse_for_all;
///
/// let data = vec![1, 2, 3];
/// let mut result = Vec::new();
///
/// tf_reverse_for_all!(i, data, {
///     result.push(*i);
/// });
///
/// assert_eq!(result, vec![3, 2, 1]);
/// ```
#[macro_export]
macro_rules! tf_reverse_for_all {
    ($item:ident, $container:expr, $body:block) => {
        for $item in $container.iter().rev() $body
    };
}

// Re-export macros at module level
pub use tf_for_all;
pub use tf_reverse_for_all;

/// Trait for types that should be copied when iterated with TfIterator.
///
/// This mirrors `Tf_ShouldIterateOverCopy` from C++. Types implementing
/// this trait indicate that TfIterator should copy them before iteration
/// (useful for proxy types that may become invalid).
///
/// By default, types are NOT copied during iteration.
pub trait ShouldIterateOverCopy {
    /// Returns true if the type should be copied before iteration.
    const SHOULD_COPY: bool = false;
}

/// Blanket implementation: by default, types are not copied.
impl<T> ShouldIterateOverCopy for T {}

/// Iterator that can enumerate key-value pairs with index.
///
/// Useful for associative container iteration patterns from C++.
///
/// # Examples
///
/// ```
/// use usd_tf::iterator::enumerate;
///
/// let data = vec!["a", "b", "c"];
/// for (idx, val) in enumerate(&data) {
///     println!("{}: {}", idx, val);
/// }
/// ```
#[inline]
pub fn enumerate<T>(
    container: &[T],
) -> impl ExactSizeIterator<Item = (usize, &T)> + DoubleEndedIterator + FusedIterator {
    container.iter().enumerate()
}

/// Iterator that can enumerate mutable key-value pairs with index.
#[inline]
pub fn enumerate_mut<T>(
    container: &mut [T],
) -> impl ExactSizeIterator<Item = (usize, &mut T)> + DoubleEndedIterator + FusedIterator {
    container.iter_mut().enumerate()
}

/// Zips two iterators together, like C++ ranges::views::zip.
///
/// # Examples
///
/// ```
/// use usd_tf::iterator::zip;
///
/// let a = vec![1, 2, 3];
/// let b = vec!["a", "b", "c"];
///
/// for (num, letter) in zip(&a, &b) {
///     println!("{}: {}", num, letter);
/// }
/// ```
#[inline]
pub fn zip<'a, T, U>(
    first: &'a [T],
    second: &'a [U],
) -> impl ExactSizeIterator<Item = (&'a T, &'a U)> + 'a {
    first.iter().zip(second.iter())
}

/// Marker type for proxy reference reverse iterator.
///
/// This is a placeholder for `Tf_ProxyReferenceReverseIterator` from C++.
/// In Rust, proxy references are less common due to the ownership model,
/// but this type exists for API compatibility if needed.
#[doc(hidden)]
pub struct ProxyReferenceReverseIterator<I: Iterator> {
    _marker: PhantomData<I>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tf_iterator_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let mut iter = TfIterator::new(&data);

        assert!(iter.is_valid());
        assert!(!iter.is_exhausted());

        assert_eq!(iter.get(), Some(&1));
        iter.advance();
        assert_eq!(iter.get(), Some(&2));
        iter.advance();
        assert_eq!(iter.get(), Some(&3));
        iter.advance();
        assert_eq!(iter.get(), Some(&4));
        iter.advance();
        assert_eq!(iter.get(), Some(&5));
        iter.advance();

        assert!(!iter.is_valid());
        assert!(iter.is_exhausted());
        assert_eq!(iter.get(), None);
    }

    #[test]
    fn test_tf_iterator_next() {
        let data = vec![10, 20, 30];
        let mut iter = TfIterator::new(&data);

        assert_eq!(iter.next(), Some(&10));
        assert_eq!(iter.next(), Some(&20));
        assert_eq!(iter.next(), Some(&30));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_tf_iterator_empty() {
        let data: Vec<i32> = vec![];
        let iter = TfIterator::new(&data);

        assert!(!iter.is_valid());
        assert!(iter.is_exhausted());
        assert_eq!(iter.get(), None);
    }

    #[test]
    fn test_tf_reverse_iterator() {
        let data = vec![1, 2, 3, 4, 5];
        let mut iter = TfReverseIterator::new(&data);

        assert!(iter.is_valid());
        assert_eq!(iter.get(), Some(&5));
        iter.advance();
        assert_eq!(iter.get(), Some(&4));
        iter.advance();
        assert_eq!(iter.get(), Some(&3));
        iter.advance();
        assert_eq!(iter.get(), Some(&2));
        iter.advance();
        assert_eq!(iter.get(), Some(&1));
        iter.advance();

        assert!(!iter.is_valid());
    }

    #[test]
    fn test_make_iterator() {
        let data = vec![1, 2, 3];
        let iter = make_iterator(&data);
        assert!(iter.is_valid());
        assert_eq!(iter.get(), Some(&1));
    }

    #[test]
    fn test_make_reverse_iterator() {
        let data = vec![1, 2, 3];
        let iter = make_reverse_iterator(&data);
        assert!(iter.is_valid());
        assert_eq!(iter.get(), Some(&3));
    }

    #[test]
    fn test_array_size() {
        let arr5 = [1, 2, 3, 4, 5];
        assert_eq!(array_size(&arr5), 5);

        let arr3 = ["a", "b", "c"];
        assert_eq!(array_size(&arr3), 3);

        let empty: [i32; 0] = [];
        assert_eq!(array_size(&empty), 0);
    }

    #[test]
    fn test_tf_for_all_macro() {
        let data = vec![1, 2, 3, 4, 5];
        let mut sum = 0;

        tf_for_all!(i, data, {
            sum += i;
        });

        assert_eq!(sum, 15);
    }

    #[test]
    fn test_tf_reverse_for_all_macro() {
        let data = vec![1, 2, 3];
        let mut result = Vec::new();

        tf_reverse_for_all!(i, data, {
            result.push(*i);
        });

        assert_eq!(result, vec![3, 2, 1]);
    }

    #[test]
    fn test_enumerate() {
        let data = vec!["a", "b", "c"];
        let pairs: Vec<_> = enumerate(&data).collect();
        assert_eq!(pairs, vec![(0, &"a"), (1, &"b"), (2, &"c")]);
    }

    #[test]
    fn test_enumerate_mut() {
        let mut data = vec![1, 2, 3];
        for (idx, val) in enumerate_mut(&mut data) {
            *val = (idx + 1) * 10;
        }
        assert_eq!(data, vec![10, 20, 30]);
    }

    #[test]
    fn test_zip() {
        let a = vec![1, 2, 3];
        let b = vec!["a", "b", "c"];
        let pairs: Vec<_> = zip(&a, &b).collect();
        assert_eq!(pairs, vec![(&1, &"a"), (&2, &"b"), (&3, &"c")]);
    }

    #[test]
    fn test_iterator_mutable() {
        let mut data = vec![1, 2, 3];
        let mut iter = TfIterator::new_mut(&mut data);

        if let Some(val) = iter.get_mut() {
            *val = 10;
        }
        iter.advance();
        if let Some(val) = iter.get_mut() {
            *val = 20;
        }

        drop(iter);
        assert_eq!(data, vec![10, 20, 3]);
    }

    #[test]
    fn test_iterator_as_std_iterator() {
        let data = vec![1, 2, 3];
        let iter = TfIterator::new(&data);

        // Can use as a standard iterator
        let collected: Vec<_> = iter.collect();
        assert_eq!(collected, vec![&1, &2, &3]);
    }

    #[test]
    fn test_iterator_take_current() {
        let data = vec![String::from("a"), String::from("b")];
        let mut iter = TfIterator::wrap(data.into_iter());

        let first = iter.take_current();
        assert_eq!(first, Some(String::from("a")));

        let second = iter.take_current();
        assert_eq!(second, Some(String::from("b")));

        let third = iter.take_current();
        assert_eq!(third, None);
    }
}
