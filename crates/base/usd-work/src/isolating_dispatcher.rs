//! Isolating work dispatcher.
//!
//! [`IsolatingDispatcher`] is a specialization of [`WorkDispatcher`](super::Dispatcher)
//! that prevents work stealing between tasks. Tasks added to this dispatcher
//! will only be run by this dispatcher or nested non-isolating dispatchers.
//!
//! Uses a dedicated rayon::ThreadPool to enforce isolation: tasks submitted
//! here cannot be stolen by unrelated parallel constructs.
//!
//! # Performance Note
//!
//! Enforcing isolation carries additional cost. Use [`Dispatcher`](super::Dispatcher)
//! unless isolation is specifically required.
//!
//! # Examples
//!
//! ```
//! use usd_work::IsolatingDispatcher;
//!
//! let dispatcher = IsolatingDispatcher::new();
//! dispatcher.run(|| println!("Isolated task 1"));
//! dispatcher.run(|| println!("Isolated task 2"));
//! dispatcher.wait();
//! ```

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// Type alias for task storage.
type TaskBox = Box<dyn FnOnce() + Send>;

/// An isolating work dispatcher.
///
/// Unlike [`Dispatcher`](super::Dispatcher), tasks submitted to this dispatcher
/// are isolated from work stealing by unrelated parallel constructs.
/// Achieves isolation by using a dedicated rayon::ThreadPool, so tasks
/// are executed in parallel within the pool but isolated from the global pool.
///
/// This matches C++ WorkSingularTask isolation semantics where tasks
/// run on a separate thread pool.
pub struct IsolatingDispatcher {
    /// Pending tasks.
    tasks: Arc<Mutex<Vec<TaskBox>>>,
    /// Count of pending tasks.
    pending: Arc<AtomicUsize>,
    /// Dedicated thread pool for isolation from global rayon pool.
    pool: rayon::ThreadPool,
}

impl IsolatingDispatcher {
    /// Create a new isolating dispatcher.
    #[must_use]
    pub fn new() -> Self {
        Self::with_num_threads(0)
    }

    /// Create a new isolating dispatcher with a specific number of threads.
    ///
    /// If `num_threads` is 0, uses the number of available CPUs.
    #[must_use]
    pub fn with_num_threads(num_threads: usize) -> Self {
        let mut builder = rayon::ThreadPoolBuilder::new();
        if num_threads > 0 {
            builder = builder.num_threads(num_threads);
        }
        let pool = builder
            .build()
            .expect("Failed to create isolated thread pool");
        Self {
            tasks: Arc::new(Mutex::new(Vec::new())),
            pending: Arc::new(AtomicUsize::new(0)),
            pool,
        }
    }

    /// Submit a task for isolated execution.
    ///
    /// The task will only be executed by this dispatcher's dedicated
    /// thread pool, isolated from unrelated parallel constructs.
    pub fn run<F>(&self, task: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.pending.fetch_add(1, Ordering::SeqCst);
        if let Ok(mut tasks) = self.tasks.lock() {
            tasks.push(Box::new(task));
        }
    }

    /// Wait for all submitted tasks to complete.
    ///
    /// Drains the task queue and executes tasks in parallel on the
    /// dedicated thread pool. Tasks cannot be stolen by the global pool.
    pub fn wait(&self) {
        // Drain all tasks from the queue.
        let tasks_to_run: Vec<TaskBox> = {
            let mut tasks = match self.tasks.lock() {
                Ok(t) => t,
                Err(_) => return,
            };
            std::mem::take(&mut *tasks)
        };

        if tasks_to_run.is_empty() {
            return;
        }

        let pending = &self.pending;

        // Execute tasks in parallel on the isolated pool using rayon::scope.
        self.pool.scope(|s| {
            for task in tasks_to_run {
                s.spawn(|_| {
                    task();
                    pending.fetch_sub(1, Ordering::SeqCst);
                });
            }
        });
    }

    /// Cancel all pending tasks.
    ///
    /// Tasks that have already started will complete, but pending
    /// tasks will be discarded.
    pub fn cancel(&self) {
        if let Ok(mut tasks) = self.tasks.lock() {
            let count = tasks.len();
            tasks.clear();
            self.pending.fetch_sub(count, Ordering::SeqCst);
        }
    }

    /// Returns the number of pending tasks.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.pending.load(Ordering::SeqCst)
    }
}

impl Default for IsolatingDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicI32;

    #[test]
    fn test_new() {
        let d = IsolatingDispatcher::new();
        assert_eq!(d.pending_count(), 0);
    }

    #[test]
    fn test_single_task() {
        let counter = Arc::new(AtomicI32::new(0));
        let d = IsolatingDispatcher::new();

        let c = Arc::clone(&counter);
        d.run(move || {
            c.fetch_add(1, Ordering::SeqCst);
        });

        d.wait();

        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_multiple_tasks() {
        let counter = Arc::new(AtomicI32::new(0));
        let d = IsolatingDispatcher::new();

        for _ in 0..10 {
            let c = Arc::clone(&counter);
            d.run(move || {
                c.fetch_add(1, Ordering::SeqCst);
            });
        }

        d.wait();

        assert_eq!(counter.load(Ordering::SeqCst), 10);
    }

    #[test]
    fn test_cancel() {
        let d = IsolatingDispatcher::new();

        // Add tasks but don't wait
        for _ in 0..5 {
            d.run(|| {
                std::thread::sleep(std::time::Duration::from_secs(10));
            });
        }

        assert_eq!(d.pending_count(), 5);

        d.cancel();

        assert_eq!(d.pending_count(), 0);
    }

    #[test]
    fn test_default() {
        let d = IsolatingDispatcher::default();
        assert_eq!(d.pending_count(), 0);
    }

    #[test]
    fn test_parallel_execution() {
        // Verify tasks run in parallel by checking thread IDs.
        use std::collections::HashSet;
        let thread_ids = Arc::new(Mutex::new(HashSet::new()));
        let d = IsolatingDispatcher::with_num_threads(4);

        for _ in 0..20 {
            let tids = Arc::clone(&thread_ids);
            d.run(move || {
                tids.lock().unwrap().insert(std::thread::current().id());
                // Small sleep to ensure threads overlap
                std::thread::sleep(std::time::Duration::from_millis(5));
            });
        }

        d.wait();

        let unique_threads = thread_ids.lock().unwrap().len();
        // With 4 threads and 20 tasks, we should see more than 1 thread
        assert!(
            unique_threads > 1,
            "Expected parallel execution across >1 thread, got {}",
            unique_threads
        );
    }

    #[test]
    fn test_with_num_threads() {
        let d = IsolatingDispatcher::with_num_threads(2);
        let counter = Arc::new(AtomicI32::new(0));

        for _ in 0..5 {
            let c = Arc::clone(&counter);
            d.run(move || {
                c.fetch_add(1, Ordering::SeqCst);
            });
        }

        d.wait();
        assert_eq!(counter.load(Ordering::SeqCst), 5);
    }
}
