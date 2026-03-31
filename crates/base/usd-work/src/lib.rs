#![allow(unsafe_code)]
//! Work - Threading and Parallelism.
//!
//! This module provides abstractions for parallel execution, built on top
//! of the `rayon` crate. It matches the OpenUSD work library API.
//!
//! # Key Components
//!
//! - Thread limits: Control concurrency with [`get_concurrency_limit`], [`set_concurrency_limit`]
//! - Parallel loops: [`parallel_for_n`], [`parallel_for_each`]
//! - Parallel reduce: [`parallel_reduce_n`]
//! - Parallel sort: [`parallel_sort`], [`parallel_sort_by`]
//! - Dispatcher: [`Dispatcher`] for hierarchical parallel task submission
//! - Singular task: [`SingularTask`] for non-concurrent task execution
//! - Detached task: [`run_detached_task`] for fire-and-forget execution
//!
//! # Examples
//!
//! ```
//! use usd_work::{parallel_for_n, parallel_for_each_index, parallel_reduce_n};
//!
//! // Parallel for with range
//! let results = std::sync::Mutex::new(Vec::new());
//! parallel_for_n(10, |begin, end| {
//!     for i in begin..end {
//!         results.lock().expect("lock poisoned").push(i);
//!     }
//! }, 1);
//!
//! // Parallel for each (by index)
//! let items = vec![1, 2, 3, 4, 5];
//! let sum = std::sync::atomic::AtomicI32::new(0);
//! parallel_for_each_index(items.len(), |i| {
//!     sum.fetch_add(items[i], std::sync::atomic::Ordering::Relaxed);
//! });
//!
//! // Parallel reduce
//! let total: i32 = parallel_reduce_n(
//!     0i32,
//!     100,
//!     |begin, end, _identity| {
//!         (begin..end).map(|i| i as i32).sum::<i32>()
//!     },
//!     |a, b| a + b,
//!     1,
//! );
//! assert_eq!(total, (0..100).sum::<i32>());
//! ```

pub mod detached_task;
pub mod dispatcher;
pub mod isolating_dispatcher;
pub mod loops;
pub mod reduce;
pub mod scoped_parallelism;
pub mod singular_task;
pub mod sort;
pub mod task_graph;
pub mod thread_limits;
pub mod utils;
pub mod zero_allocator;

pub use detached_task::*;
pub use dispatcher::*;
pub use isolating_dispatcher::IsolatingDispatcher;
pub use loops::{
    parallel_for_chunked, parallel_for_each, parallel_for_each_index, parallel_for_each_mut,
    parallel_for_n, parallel_for_range, serial_for_n,
};
pub use reduce::{
    parallel_reduce, parallel_reduce_n, parallel_reduce_n_auto, parallel_reduce_transform_n,
    parallel_transform_n, parallel_transform_n_auto,
};
pub use scoped_parallelism::{
    with_scoped_dispatcher, with_scoped_dispatcher_mut, with_scoped_parallelism,
};
pub use singular_task::*;
pub use sort::*;
pub use task_graph::{
    BaseTask, ChainedTask, FnTask, RawTask, RefCountedTask, SimpleTask, TaskBase, TaskGraph,
    TaskList, allocate_child,
};
pub use thread_limits::*; // includes get_physical_core_count
pub use utils::*;
pub use zero_allocator::{
    CACHE_LINE_SIZE, CacheAlignedVec, alloc_zeroed_raw, zeroed_boxed_slice, zeroed_vec,
};

#[cfg(test)]
mod tests {
    use super::*;
    use rayon::iter::IntoParallelRefIterator;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_parallel_for_n_basic() {
        let counter = AtomicUsize::new(0);
        parallel_for_n(
            100,
            |begin, end| {
                counter.fetch_add(end - begin, Ordering::Relaxed);
            },
            1,
        );
        assert_eq!(counter.load(Ordering::Relaxed), 100);
    }

    #[test]
    fn test_parallel_for_each_basic() {
        let items: Vec<i32> = (0..100).collect();
        let sum = AtomicUsize::new(0);
        parallel_for_each(items.par_iter(), |&x| {
            sum.fetch_add(x as usize, Ordering::Relaxed);
        });
        assert_eq!(sum.load(Ordering::Relaxed), (0..100).sum::<i32>() as usize);
    }

    #[test]
    fn test_parallel_reduce() {
        let result: i32 = parallel_reduce_n(
            0,
            1000,
            |begin, end, _| (begin..end).map(|i| i as i32).sum(),
            |a, b| a + b,
            10,
        );
        assert_eq!(result, (0..1000).sum::<i32>());
    }

    #[test]
    fn test_parallel_sort() {
        let mut data: Vec<i32> = (0..1000).rev().collect();
        parallel_sort(&mut data);
        assert!(data.windows(2).all(|w| w[0] <= w[1]));
    }

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
}
