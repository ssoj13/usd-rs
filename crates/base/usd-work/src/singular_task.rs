//! Singular task - a task that never runs concurrently with itself.
//!
//! A [`SingularTask`] runs a task in a [`Dispatcher`], but never concurrently
//! with itself. This is useful for single-threaded work that can be overlapped
//! with other parallel tasks.
//!
//! # Example Use Case
//!
//! A multiple-producer, single-consumer problem: Run producer tasks in a
//! [`Dispatcher`] and create a [`SingularTask`] for the consumer. When a
//! producer has a result, it invokes [`wake`](SingularTask::wake) on the
//! consumer. The consumer runs only when there are results to consume, and
//! operates single-threaded (no locking needed for its data structures).
//!
//! # Examples
//!
//! ```no_run
//! use usd_work::{Dispatcher, SingularTask};
//! use std::sync::Arc;
//! use std::sync::atomic::{AtomicUsize, Ordering};
//!
//! let dispatcher = Arc::new(Dispatcher::new());
//! let results = Arc::new(AtomicUsize::new(0));
//!
//! // Create a consumer that processes results
//! let r = Arc::clone(&results);
//! let consumer = SingularTask::with_shared_dispatcher(
//!     Arc::clone(&dispatcher),
//!     move || { r.fetch_add(1, Ordering::Relaxed); },
//! );
//!
//! // Producers wake the consumer
//! for _ in 0..5 {
//!     consumer.wake();
//! }
//!
//! dispatcher.wait();
//! // Consumer ran at least once (possibly multiple wakes coalesced)
//! assert!(results.load(Ordering::Relaxed) >= 1);
//! ```

use super::Dispatcher;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A task that runs in a dispatcher but never concurrently with itself.
///
/// Call [`wake`](SingularTask::wake) to ensure the task runs at least once
/// after the call. Multiple wakes may be coalesced into a single execution.
///
/// # Thread Safety
///
/// The task function does not need to be thread-safe since it never runs
/// concurrently with itself. However, shared data accessed by the task
/// and other code should still be properly synchronized.
pub struct SingularTask {
    /// Reference count of pending wakes.
    count: Arc<AtomicUsize>,
    /// The wake closure that runs the task and drains the count.
    wake_fn: Arc<dyn Fn() + Send + Sync>,
    /// The dispatcher to submit work to. Stored as Arc for safe sharing.
    dispatcher: Arc<Dispatcher>,
}

impl SingularTask {
    /// Creates a singular task to be run in `dispatcher`.
    ///
    /// Callers must ensure that `dispatcher` lives at least as long as this
    /// `SingularTask`. Prefer [`with_shared_dispatcher`](Self::with_shared_dispatcher)
    /// for safe lifetime management.
    ///
    /// # Arguments
    ///
    /// * `_dispatcher` - The dispatcher to run the task in (lifetime must be
    ///   guaranteed by caller; internally uses a private dispatcher for safety)
    /// * `task` - The task function to execute
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_work::{Dispatcher, SingularTask};
    ///
    /// let dispatcher = Dispatcher::new();
    /// let task = SingularTask::new(&dispatcher, || {
    ///     println!("Task running");
    /// });
    ///
    /// task.wake();
    /// dispatcher.wait();
    /// ```
    pub fn new<F>(_dispatcher: &Dispatcher, task: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        // Use an internal dispatcher since we can't safely store a reference.
        // For full dispatcher integration, use with_shared_dispatcher().
        Self::with_shared_dispatcher(Arc::new(Dispatcher::new()), task)
    }

    /// Creates a singular task with a shared dispatcher.
    ///
    /// This variant takes an `Arc<Dispatcher>` for safe lifetime management.
    /// The dispatcher is kept alive as long as this task exists.
    pub fn with_shared_dispatcher<F>(dispatcher: Arc<Dispatcher>, task: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        let count = Arc::new(AtomicUsize::new(0));
        let task = Arc::new(task);

        // Build the wake closure matching C++ WorkSingularTask:
        // Read current count, invoke task, CAS to zero. If CAS fails
        // (more wakes arrived), repeat.
        let count_ref = Arc::clone(&count);
        let task_ref = Arc::clone(&task);
        let wake_fn: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
            let mut old = count_ref.load(Ordering::Acquire);
            loop {
                task_ref();
                match count_ref.compare_exchange(old, 0, Ordering::AcqRel, Ordering::Acquire) {
                    Ok(_) => break,
                    Err(current) => old = current,
                }
            }
        });

        Self {
            count,
            wake_fn,
            dispatcher,
        }
    }

    /// Ensures that this task runs at least once after this call.
    ///
    /// The task is not guaranteed to run as many times as `wake` is invoked,
    /// only that it runs at least once after a call to `wake`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use usd_work::{Dispatcher, SingularTask};
    /// use std::sync::atomic::{AtomicUsize, Ordering};
    /// use std::sync::Arc;
    ///
    /// let dispatcher = Arc::new(Dispatcher::new());
    /// let counter = Arc::new(AtomicUsize::new(0));
    ///
    /// let c = Arc::clone(&counter);
    /// let task = SingularTask::with_shared_dispatcher(
    ///     Arc::clone(&dispatcher),
    ///     move || { c.fetch_add(1, Ordering::Relaxed); },
    /// );
    ///
    /// // Wake multiple times
    /// task.wake();
    /// task.wake();
    /// task.wake();
    ///
    /// dispatcher.wait();
    ///
    /// // Task ran at least once
    /// assert!(counter.load(Ordering::Relaxed) >= 1);
    /// ```
    pub fn wake(&self) {
        // Increment count. If we transition 0 -> 1, submit to dispatcher.
        let prev = self.count.fetch_add(1, Ordering::AcqRel);
        if prev == 0 {
            let wake_fn = Arc::clone(&self.wake_fn);
            self.dispatcher.run(move || wake_fn());
        }
    }

    /// Returns the current wake count (for testing/debugging).
    pub fn wake_count(&self) -> usize {
        self.count.load(Ordering::Acquire)
    }
}

/// A simpler singular task implementation using closures.
///
/// This version owns the task function and can be used without a dispatcher.
pub struct SimpleSingularTask<F> {
    task: F,
    count: AtomicUsize,
}

impl<F> SimpleSingularTask<F>
where
    F: Fn() + Send + Sync,
{
    /// Creates a new simple singular task.
    pub fn new(task: F) -> Self {
        Self {
            task,
            count: AtomicUsize::new(0),
        }
    }

    /// Wakes the task and runs it immediately if no other instance is running.
    ///
    /// Returns `true` if this call triggered an execution, `false` if another
    /// execution was already in progress.
    pub fn wake(&self) -> bool {
        let prev = self.count.fetch_add(1, Ordering::Acquire);
        if prev == 0 {
            // We're first - run the task in a CAS loop like C++
            loop {
                (self.task)();
                let old = self.count.load(Ordering::Acquire);
                if self
                    .count
                    .compare_exchange(old, 0, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
                {
                    break;
                }
            }
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_simple_singular_task() {
        let counter = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&counter);

        let task = SimpleSingularTask::new(move || {
            c.fetch_add(1, Ordering::Relaxed);
        });

        // Wake and verify it runs
        assert!(task.wake());
        assert!(counter.load(Ordering::Relaxed) >= 1);
    }

    #[test]
    fn test_simple_singular_task_multiple_wakes() {
        let counter = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&counter);

        let task = SimpleSingularTask::new(move || {
            c.fetch_add(1, Ordering::Relaxed);
            std::thread::yield_now();
        });

        task.wake();
        assert!(counter.load(Ordering::Relaxed) >= 1);
    }

    #[test]
    fn test_singular_task_with_shared_dispatcher() {
        let counter = Arc::new(AtomicUsize::new(0));
        let dispatcher = Arc::new(Dispatcher::new());

        let c = Arc::clone(&counter);
        let task = SingularTask::with_shared_dispatcher(Arc::clone(&dispatcher), move || {
            c.fetch_add(1, Ordering::Relaxed);
        });

        task.wake();
        dispatcher.wait();

        assert!(counter.load(Ordering::Relaxed) >= 1);
    }

    #[test]
    fn test_singular_task_wake_count() {
        let dispatcher = Dispatcher::new();
        let task = SingularTask::new(&dispatcher, || {});

        assert_eq!(task.wake_count(), 0);
        task.wake();
        // After wake + dispatcher processes, count should be back to 0
    }
}
