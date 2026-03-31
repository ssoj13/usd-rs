//! Parallel sort utilities.
//!
//! This module provides parallel sorting functions using rayon.
//!
//! # Examples
//!
//! ```
//! use usd_work::{parallel_sort, parallel_sort_by};
//!
//! // Sort in ascending order
//! let mut data: Vec<i32> = vec![5, 2, 8, 1, 9, 3];
//! parallel_sort(&mut data);
//! assert_eq!(data, vec![1, 2, 3, 5, 8, 9]);
//!
//! // Sort with custom comparator (descending)
//! let mut data: Vec<i32> = vec![5, 2, 8, 1, 9, 3];
//! parallel_sort_by(&mut data, |a, b| b.cmp(a));
//! assert_eq!(data, vec![9, 8, 5, 3, 2, 1]);
//! ```

use super::has_concurrency;
use rayon::prelude::*;
use std::cmp::Ordering;

/// Sorts a slice in-place in parallel using the natural ordering.
///
/// Uses rayon's parallel sort which is a parallel merge sort.
/// Falls back to standard sort if concurrency is disabled.
///
/// # Examples
///
/// ```
/// use usd_work::parallel_sort;
///
/// let mut data: Vec<i32> = (0..1000).rev().collect();
/// parallel_sort(&mut data);
/// assert!(data.windows(2).all(|w| w[0] <= w[1]));
/// ```
pub fn parallel_sort<T>(slice: &mut [T])
where
    T: Ord + Send,
{
    if !has_concurrency() {
        slice.sort();
    } else {
        slice.par_sort();
    }
}

/// Sorts a slice in-place in parallel using a custom comparison function.
///
/// # Examples
///
/// ```
/// use usd_work::parallel_sort_by;
///
/// // Sort descending
/// let mut data: Vec<i32> = (0..1000).collect();
/// parallel_sort_by(&mut data, |a, b| b.cmp(a));
/// assert!(data.windows(2).all(|w| w[0] >= w[1]));
/// ```
pub fn parallel_sort_by<T, F>(slice: &mut [T], compare: F)
where
    T: Send,
    F: Fn(&T, &T) -> Ordering + Sync,
{
    if !has_concurrency() {
        slice.sort_by(compare);
    } else {
        slice.par_sort_by(compare);
    }
}

/// Sorts a slice in-place in parallel using a key extraction function.
///
/// # Examples
///
/// ```
/// use usd_work::parallel_sort_by_key;
///
/// #[derive(Debug, PartialEq)]
/// struct Person { name: String, age: u32 }
///
/// let mut people = vec![
///     Person { name: "Alice".into(), age: 30 },
///     Person { name: "Bob".into(), age: 25 },
///     Person { name: "Charlie".into(), age: 35 },
/// ];
///
/// parallel_sort_by_key(&mut people, |p| p.age);
/// assert_eq!(people[0].name, "Bob");
/// assert_eq!(people[1].name, "Alice");
/// assert_eq!(people[2].name, "Charlie");
/// ```
pub fn parallel_sort_by_key<T, K, F>(slice: &mut [T], key: F)
where
    T: Send,
    K: Ord,
    F: Fn(&T) -> K + Sync,
{
    if !has_concurrency() {
        slice.sort_by_key(key);
    } else {
        slice.par_sort_by_key(key);
    }
}

/// Sorts a slice in-place in parallel, but might not preserve the order of
/// equal elements (unstable sort).
///
/// This is typically faster than [`parallel_sort`] but may not preserve
/// the relative order of equal elements.
///
/// # Examples
///
/// ```
/// use usd_work::parallel_sort_unstable;
///
/// let mut data: Vec<i32> = (0..1000).rev().collect();
/// parallel_sort_unstable(&mut data);
/// assert!(data.windows(2).all(|w| w[0] <= w[1]));
/// ```
pub fn parallel_sort_unstable<T>(slice: &mut [T])
where
    T: Ord + Send,
{
    if !has_concurrency() {
        slice.sort_unstable();
    } else {
        slice.par_sort_unstable();
    }
}

/// Unstable sort with custom comparison function.
///
/// # Examples
///
/// ```
/// use usd_work::parallel_sort_unstable_by;
///
/// let mut data: Vec<i32> = (0..1000).collect();
/// parallel_sort_unstable_by(&mut data, |a, b| b.cmp(a));
/// assert!(data.windows(2).all(|w| w[0] >= w[1]));
/// ```
pub fn parallel_sort_unstable_by<T, F>(slice: &mut [T], compare: F)
where
    T: Send,
    F: Fn(&T, &T) -> Ordering + Sync,
{
    if !has_concurrency() {
        slice.sort_unstable_by(compare);
    } else {
        slice.par_sort_unstable_by(compare);
    }
}

/// Unstable sort with key extraction function.
///
/// # Examples
///
/// ```
/// use usd_work::parallel_sort_unstable_by_key;
///
/// let mut data: Vec<(i32, i32)> = vec![(3, 1), (1, 2), (2, 3)];
/// parallel_sort_unstable_by_key(&mut data, |&(a, _)| a);
/// assert_eq!(data, vec![(1, 2), (2, 3), (3, 1)]);
/// ```
pub fn parallel_sort_unstable_by_key<T, K, F>(slice: &mut [T], key: F)
where
    T: Send,
    K: Ord,
    F: Fn(&T) -> K + Sync,
{
    if !has_concurrency() {
        slice.sort_unstable_by_key(key);
    } else {
        slice.par_sort_unstable_by_key(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parallel_sort() {
        let mut data: Vec<i32> = (0..10000).rev().collect();
        parallel_sort(&mut data);
        assert!(data.windows(2).all(|w| w[0] <= w[1]));
    }

    #[test]
    fn test_parallel_sort_small() {
        let mut data = vec![5, 2, 8, 1, 9, 3];
        parallel_sort(&mut data);
        assert_eq!(data, vec![1, 2, 3, 5, 8, 9]);
    }

    #[test]
    fn test_parallel_sort_by() {
        let mut data: Vec<i32> = (0..10000).collect();
        parallel_sort_by(&mut data, |a, b| b.cmp(a));
        assert!(data.windows(2).all(|w| w[0] >= w[1]));
    }

    #[test]
    fn test_parallel_sort_by_key() {
        let mut data: Vec<(i32, &str)> = vec![(3, "c"), (1, "a"), (2, "b")];
        parallel_sort_by_key(&mut data, |&(k, _)| k);
        assert_eq!(data, vec![(1, "a"), (2, "b"), (3, "c")]);
    }

    #[test]
    fn test_parallel_sort_unstable() {
        let mut data: Vec<i32> = (0..10000).rev().collect();
        parallel_sort_unstable(&mut data);
        assert!(data.windows(2).all(|w| w[0] <= w[1]));
    }

    #[test]
    fn test_parallel_sort_empty() {
        let mut data: Vec<i32> = vec![];
        parallel_sort(&mut data);
        assert!(data.is_empty());
    }

    #[test]
    fn test_parallel_sort_single() {
        let mut data = vec![42];
        parallel_sort(&mut data);
        assert_eq!(data, vec![42]);
    }
}
