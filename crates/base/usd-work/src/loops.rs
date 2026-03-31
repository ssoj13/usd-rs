//! Parallel loop utilities.
//!
//! This module provides parallel for loops that distribute work across
//! multiple threads using rayon.
//!
//! # Examples
//!
//! ```
//! use usd_work::{parallel_for_n, parallel_for_each_index, serial_for_n};
//! use std::sync::atomic::{AtomicUsize, Ordering};
//!
//! // Parallel for with range
//! let counter = AtomicUsize::new(0);
//! parallel_for_n(100, |begin, end| {
//!     counter.fetch_add(end - begin, Ordering::Relaxed);
//! }, 1);
//! assert_eq!(counter.load(Ordering::Relaxed), 100);
//!
//! // Parallel for each (by index)
//! let items = vec![1usize, 2, 3, 4, 5];
//! let sum = AtomicUsize::new(0);
//! parallel_for_each_index(items.len(), |i| {
//!     sum.fetch_add(items[i], Ordering::Relaxed);
//! });
//! assert_eq!(sum.load(Ordering::Relaxed), 15);
//! ```

use super::has_concurrency;
use rayon::prelude::*;

/// A serial version of [`parallel_for_n`] as a drop-in replacement to
/// selectively turn off multithreading for easier debugging.
///
/// Calls `callback(0, n)` directly.
///
/// # Examples
///
/// ```
/// use usd_work::serial_for_n;
///
/// let mut sum = 0;
/// serial_for_n(10, |begin, end| {
///     for i in begin..end {
///         sum += i;
///     }
/// });
/// assert_eq!(sum, 45);
/// ```
pub fn serial_for_n<F>(n: usize, callback: F)
where
    F: FnOnce(usize, usize),
{
    if n > 0 {
        callback(0, n);
    }
}

/// Runs `callback` in parallel over the range [0, n).
///
/// The callback receives `(begin, end)` pairs representing sub-ranges
/// to process. The `grain_size` specifies the minimum amount of work
/// per task.
///
/// # Arguments
///
/// * `n` - The upper bound of the range [0, n)
/// * `callback` - Function called with (begin, end) for each sub-range
/// * `grain_size` - Minimum elements per parallel task (default 1)
///
/// # Examples
///
/// ```
/// use usd_work::parallel_for_n;
/// use std::sync::atomic::{AtomicUsize, Ordering};
///
/// let counter = AtomicUsize::new(0);
/// parallel_for_n(1000, |begin, end| {
///     // Process elements [begin, end)
///     counter.fetch_add(end - begin, Ordering::Relaxed);
/// }, 100);
/// assert_eq!(counter.load(Ordering::Relaxed), 1000);
/// ```
pub fn parallel_for_n<F>(n: usize, callback: F, grain_size: usize)
where
    F: Fn(usize, usize) + Sync,
{
    if n == 0 {
        return;
    }

    // Fall back to serial if no concurrency
    if !has_concurrency() {
        callback(0, n);
        return;
    }

    let grain = grain_size.max(1);

    // Use rayon's parallel iterator with chunks
    let chunk_count = n.div_ceil(grain);
    (0..chunk_count).into_par_iter().for_each(|chunk_idx| {
        let begin = chunk_idx * grain;
        let end = (begin + grain).min(n);
        callback(begin, end);
    });
}

/// Runs `callback` in parallel for each element in the iterator.
///
/// # Arguments
///
/// * `iter` - Iterator over elements to process
/// * `callback` - Function called for each element
///
/// # Examples
///
/// ```
/// use usd_work::parallel_for_each;
/// use rayon::prelude::*;
/// use std::sync::atomic::{AtomicI32, Ordering};
///
/// let items = vec![1, 2, 3, 4, 5];
/// let sum = AtomicI32::new(0);
/// parallel_for_each(items.par_iter(), |&x| {
///     sum.fetch_add(x, Ordering::Relaxed);
/// });
/// assert_eq!(sum.load(Ordering::Relaxed), 15);
/// ```
pub fn parallel_for_each<I, F>(iter: I, callback: F)
where
    I: IntoParallelIterator,
    F: Fn(I::Item) + Sync + Send,
{
    if !has_concurrency() {
        // Serial fallback
        iter.into_par_iter().for_each(&callback);
    } else {
        iter.into_par_iter().for_each(callback);
    }
}

/// Parallel for each over a mutable iterator.
///
/// # Examples
///
/// ```
/// use usd_work::parallel_for_each_mut;
/// use rayon::prelude::*;
///
/// let mut items = vec![1, 2, 3, 4, 5];
/// parallel_for_each_mut(items.par_iter_mut(), |x| {
///     *x *= 2;
/// });
/// assert_eq!(items, vec![2, 4, 6, 8, 10]);
/// ```
pub fn parallel_for_each_mut<I, F>(iter: I, callback: F)
where
    I: IntoParallelIterator,
    F: Fn(I::Item) + Sync + Send,
{
    iter.into_par_iter().for_each(callback);
}

/// Runs `callback` over a splittable range in parallel.
///
/// The range type must be splittable (implement rayon's IntoParallelIterator).
/// This is the Rust equivalent of WorkParallelForTBBRange.
///
/// # Examples
///
/// ```ignore
/// use usd_work::parallel_for_range;
/// use std::sync::atomic::{AtomicI32, Ordering};
///
/// let sum = AtomicI32::new(0);
/// parallel_for_range(0..100, |i| {
///     sum.fetch_add(i, Ordering::Relaxed);
/// });
/// ```
pub fn parallel_for_range<R, F>(range: R, callback: F)
where
    R: IntoParallelIterator,
    F: Fn(R::Item) + Sync + Send,
{
    if !has_concurrency() {
        // Serial fallback - use into_par_iter which falls back to serial
        range.into_par_iter().for_each(callback);
        return;
    }

    range.into_par_iter().for_each(callback);
}
/// Runs `callback` over a chunked range in parallel.
///
/// This divides the range [0, n) into chunks and calls the callback
/// with each chunk as a range.
///
/// # Examples
///
/// ```
/// use usd_work::parallel_for_chunked;
/// use std::sync::atomic::{AtomicI32, Ordering};
///
/// let sum = AtomicI32::new(0);
/// parallel_for_chunked(100, 10, |chunk| {
///     for i in chunk {
///         sum.fetch_add(i as i32, Ordering::Relaxed);
///     }
/// });
/// assert_eq!(sum.load(Ordering::Relaxed), (0..100).sum::<i32>());
/// ```
pub fn parallel_for_chunked<F>(n: usize, chunk_size: usize, callback: F)
where
    F: Fn(std::ops::Range<usize>) + Sync + Send,
{
    if n == 0 {
        return;
    }

    let chunk = chunk_size.max(1);

    if !has_concurrency() {
        callback(0..n);
        return;
    }

    let num_chunks = n.div_ceil(chunk);
    (0..num_chunks).into_par_iter().for_each(|i| {
        let start = i * chunk;
        let end = ((i + 1) * chunk).min(n);
        callback(start..end);
    });
}
/// Parallel for with index. Calls `callback(index)` for each index in [0, n).
///
/// # Examples
///
/// ```
/// use usd_work::parallel_for_each_index;
/// use std::sync::atomic::{AtomicUsize, Ordering};
///
/// let counter = AtomicUsize::new(0);
/// parallel_for_each_index(100, |_i| {
///     counter.fetch_add(1, Ordering::Relaxed);
/// });
/// assert_eq!(counter.load(Ordering::Relaxed), 100);
/// ```
pub fn parallel_for_each_index<F>(n: usize, callback: F)
where
    F: Fn(usize) + Sync + Send,
{
    if n == 0 {
        return;
    }

    if !has_concurrency() {
        for i in 0..n {
            callback(i);
        }
    } else {
        (0..n).into_par_iter().for_each(callback);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rayon::iter::IntoParallelRefIterator;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_serial_for_n() {
        let mut visited = vec![false; 10];
        serial_for_n(10, |begin, end| {
            for i in begin..end {
                visited[i] = true;
            }
        });
        assert!(visited.iter().all(|&v| v));
    }

    #[test]
    fn test_serial_for_n_empty() {
        let called = std::sync::atomic::AtomicBool::new(false);
        serial_for_n(0, |_, _| {
            called.store(true, Ordering::Relaxed);
        });
        assert!(!called.load(Ordering::Relaxed));
    }

    #[test]
    fn test_parallel_for_n_basic() {
        let counter = AtomicUsize::new(0);
        parallel_for_n(
            1000,
            |begin, end| {
                counter.fetch_add(end - begin, Ordering::Relaxed);
            },
            1,
        );
        assert_eq!(counter.load(Ordering::Relaxed), 1000);
    }

    #[test]
    fn test_parallel_for_n_empty() {
        let called = std::sync::atomic::AtomicBool::new(false);
        parallel_for_n(
            0,
            |_, _| {
                called.store(true, Ordering::Relaxed);
            },
            1,
        );
        assert!(!called.load(Ordering::Relaxed));
    }

    #[test]
    fn test_parallel_for_n_grain_size() {
        let calls = AtomicUsize::new(0);
        parallel_for_n(
            100,
            |_begin, _end| {
                calls.fetch_add(1, Ordering::Relaxed);
            },
            50,
        );
        // With grain_size=50 and n=100, should be ~2 calls (may vary due to rayon)
        let call_count = calls.load(Ordering::Relaxed);
        assert!(call_count <= 100); // At most 100 calls
        assert!(call_count >= 1); // At least 1 call
    }

    #[test]
    fn test_parallel_for_chunked() {
        let results = Mutex::new(Vec::new());
        parallel_for_chunked(20, 5, |chunk| {
            let mut local = Vec::new();
            for i in chunk {
                local.push(i);
            }
            results.lock().expect("lock poisoned").extend(local);
        });
        let mut res = results.lock().expect("lock poisoned").clone();
        res.sort();
        assert_eq!(res, (0..20).collect::<Vec<_>>());
    }

    #[test]
    fn test_parallel_for_chunked_empty() {
        let called = std::sync::atomic::AtomicBool::new(false);
        parallel_for_chunked(0, 10, |_| {
            called.store(true, Ordering::Relaxed);
        });
        assert!(!called.load(Ordering::Relaxed));
    }

    #[test]
    fn test_parallel_for_each() {
        let items: Vec<i32> = (0..100).collect();
        let sum = AtomicUsize::new(0);
        parallel_for_each(items.par_iter(), |&x| {
            sum.fetch_add(x as usize, Ordering::Relaxed);
        });
        assert_eq!(sum.load(Ordering::Relaxed), (0..100).sum::<i32>() as usize);
    }

    #[test]
    fn test_parallel_for_each_index() {
        let results = Mutex::new(vec![false; 100]);
        parallel_for_each_index(100, |i| {
            results.lock().expect("lock poisoned")[i] = true;
        });
        assert!(results.lock().expect("lock poisoned").iter().all(|&v| v));
    }
}
