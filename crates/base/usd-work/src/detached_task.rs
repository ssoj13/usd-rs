//! Detached task - fire-and-forget async execution.
//!
//! This module provides functions to run tasks asynchronously without
//! waiting for their completion. Errors produced by detached tasks are
//! discarded.
//!
//! # Warning
//!
//! Use detached tasks carefully. Since there's no way to wait for completion
//! or retrieve errors, they should only be used for truly independent work
//! where:
//! - The result is not needed
//! - Errors can be safely ignored
//! - The task's lifetime doesn't depend on any scoped data
//!
//! # Examples
//!
//! ```
//! use usd_work::run_detached_task;
//! use std::sync::atomic::{AtomicBool, Ordering};
//! use std::sync::Arc;
//!
//! let completed = Arc::new(AtomicBool::new(false));
//! let c = Arc::clone(&completed);
//!
//! run_detached_task(move || {
//!     // Do some work asynchronously
//!     c.store(true, Ordering::Relaxed);
//! });
//!
//! // Task may or may not have completed yet
//! // We have no way to wait for it
//! ```

use super::has_concurrency;

/// Invokes `task` asynchronously, discards any errors it produces, and
/// provides no way to wait for it to complete.
///
/// If concurrency is not available (single-threaded mode), the task is
/// executed synchronously.
///
/// # Arguments
///
/// * `task` - The task to execute asynchronously
///
/// # Examples
///
/// ```
/// use usd_work::run_detached_task;
///
/// run_detached_task(|| {
///     println!("Running in background");
/// });
/// ```
///
/// # Warning
///
/// Detached tasks cannot be waited on and their errors are silently discarded.
/// Use only when you truly don't care about the result or completion status.
pub fn run_detached_task<F>(task: F)
where
    F: FnOnce() + Send + 'static,
{
    if has_concurrency() {
        // Spawn on rayon's thread pool
        rayon::spawn(task);
    } else {
        // Execute synchronously
        task();
    }
}

/// Invokes `task` asynchronously, returning a handle that can be used to
/// check completion (but not wait for it).
///
/// This is a slightly safer alternative to [`run_detached_task`] that at
/// least allows checking if the task has completed.
///
/// # Examples
///
/// ```
/// use usd_work::spawn_detached_task;
/// use std::sync::atomic::{AtomicBool, Ordering};
/// use std::sync::Arc;
///
/// let done = Arc::new(AtomicBool::new(false));
/// let d = Arc::clone(&done);
///
/// let handle = spawn_detached_task(move || {
///     d.store(true, Ordering::Relaxed);
/// });
///
/// // Can check if completed (non-blocking)
/// // handle.is_completed() - would return true when done
/// ```
pub fn spawn_detached_task<F>(task: F) -> DetachedTaskHandle
where
    F: FnOnce() + Send + 'static,
{
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let completed = Arc::new(AtomicBool::new(false));
    let completed_clone = Arc::clone(&completed);

    let wrapped_task = move || {
        task();
        completed_clone.store(true, Ordering::Release);
    };

    if has_concurrency() {
        rayon::spawn(wrapped_task);
    } else {
        wrapped_task();
    }

    DetachedTaskHandle { completed }
}

/// A handle to a detached task that allows checking completion status.
///
/// Note: This handle does NOT provide a way to wait for completion.
/// It only allows polling the completion status.
#[derive(Clone)]
pub struct DetachedTaskHandle {
    completed: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl DetachedTaskHandle {
    /// Returns `true` if the task has completed.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_work::spawn_detached_task;
    /// use std::thread;
    /// use std::time::Duration;
    ///
    /// let handle = spawn_detached_task(|| {
    ///     // Quick task
    /// });
    ///
    /// // Give it time to complete
    /// thread::sleep(Duration::from_millis(10));
    ///
    /// // Check if done
    /// if handle.is_completed() {
    ///     println!("Task finished");
    /// }
    /// ```
    pub fn is_completed(&self) -> bool {
        self.completed.load(std::sync::atomic::Ordering::Acquire)
    }
}

/// Runs multiple tasks in parallel, fire-and-forget style.
///
/// # Examples
///
/// ```
/// use usd_work::run_detached_tasks;
///
/// let tasks: Vec<Box<dyn FnOnce() + Send>> = vec![
///     Box::new(|| println!("Task 1")),
///     Box::new(|| println!("Task 2")),
///     Box::new(|| println!("Task 3")),
/// ];
///
/// run_detached_tasks(tasks);
/// ```
pub fn run_detached_tasks<F>(tasks: impl IntoIterator<Item = F>)
where
    F: FnOnce() + Send + 'static,
{
    for task in tasks {
        run_detached_task(task);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_run_detached_task() {
        let counter = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&counter);

        run_detached_task(move || {
            c.fetch_add(1, Ordering::Relaxed);
        });

        // Wait a bit for the task to complete
        thread::sleep(Duration::from_millis(50));

        // Task should have run
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_spawn_detached_task() {
        let handle = spawn_detached_task(|| {
            // Quick task
        });

        // Wait a bit
        thread::sleep(Duration::from_millis(50));

        assert!(handle.is_completed());
    }

    #[test]
    fn test_run_detached_tasks() {
        let counter = Arc::new(AtomicUsize::new(0));

        let tasks: Vec<Box<dyn FnOnce() + Send>> = (0..10)
            .map(|_| {
                let c = Arc::clone(&counter);
                Box::new(move || {
                    c.fetch_add(1, Ordering::Relaxed);
                }) as Box<dyn FnOnce() + Send>
            })
            .collect();

        run_detached_tasks(tasks);

        // Wait for tasks
        thread::sleep(Duration::from_millis(100));

        assert_eq!(counter.load(Ordering::Relaxed), 10);
    }

    #[test]
    fn test_detached_task_handle_clone() {
        let handle = spawn_detached_task(|| {});
        let handle2 = handle.clone();

        thread::sleep(Duration::from_millis(50));

        assert!(handle.is_completed());
        assert!(handle2.is_completed());
    }
}
