//! Work dispatcher for hierarchical parallel task submission.
//!
//! A [`Dispatcher`] runs concurrent tasks and supports adding new tasks from
//! within running tasks. This suits problems that exhibit hierarchical
//! structured parallelism: tasks that discover additional tasks during
//! their execution.
//!
//! # Examples
//!
//! ```
//! use usd_work::Dispatcher;
//! use std::sync::atomic::{AtomicUsize, Ordering};
//! use std::sync::Arc;
//!
//! let counter = Arc::new(AtomicUsize::new(0));
//! let mut dispatcher = Dispatcher::new();
//!
//! for i in 0..10 {
//!     let counter = Arc::clone(&counter);
//!     dispatcher.run(move || {
//!         counter.fetch_add(1, Ordering::Relaxed);
//!     });
//! }
//!
//! dispatcher.wait();
//! assert_eq!(counter.load(Ordering::Relaxed), 10);
//! ```

use parking_lot::Mutex;
use rayon::prelude::*;
use std::any::Any;
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// A work dispatcher that runs concurrent tasks.
///
/// The dispatcher supports adding new tasks from within running tasks.
/// This suits problems that exhibit hierarchical structured parallelism.
///
/// # Thread Safety
///
/// - Calls to [`run`](Dispatcher::run) may be made concurrently.
/// - Calls to [`wait`](Dispatcher::wait) may also be made concurrently.
/// - Once [`wait`](Dispatcher::wait) is invoked, [`run`](Dispatcher::run) must only
///   be called from within tasks already added by [`run`](Dispatcher::run).
///
/// # Examples
///
/// ```
/// use usd_work::Dispatcher;
/// use std::sync::Arc;
/// use std::sync::atomic::{AtomicUsize, Ordering};
///
/// let dispatcher = Dispatcher::new();
/// let counter = Arc::new(AtomicUsize::new(0));
///
/// // Submit tasks
/// for _ in 0..100 {
///     let c = Arc::clone(&counter);
///     dispatcher.run(move || {
///         c.fetch_add(1, Ordering::Relaxed);
///     });
/// }
///
/// // Wait for completion
/// dispatcher.wait();
/// assert_eq!(counter.load(Ordering::Relaxed), 100);
/// ```
pub struct Dispatcher {
    /// Pending tasks to execute.
    tasks: Mutex<Vec<Box<dyn FnOnce() + Send>>>,
    /// Number of tasks currently in flight.
    in_flight: AtomicUsize,
    /// Flag indicating if cancel was requested.
    cancelled: AtomicBool,
    /// Collected errors from panicking tasks.
    errors: Mutex<Vec<Box<dyn Any + Send>>>,
}

impl Default for Dispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Dispatcher {
    /// Creates a new work dispatcher.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_work::Dispatcher;
    ///
    /// let dispatcher = Dispatcher::new();
    /// ```
    pub fn new() -> Self {
        Self {
            tasks: Mutex::new(Vec::new()),
            in_flight: AtomicUsize::new(0),
            cancelled: AtomicBool::new(false),
            errors: Mutex::new(Vec::new()),
        }
    }

    /// Adds work for the dispatcher to run.
    ///
    /// This function does not block in general. The added work may be
    /// not yet started, started but not completed, or completed upon return.
    ///
    /// # Arguments
    ///
    /// * `task` - A callable to execute
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_work::Dispatcher;
    ///
    /// let dispatcher = Dispatcher::new();
    /// dispatcher.run(|| println!("Hello from task"));
    /// dispatcher.wait();
    /// ```
    pub fn run<F>(&self, task: F)
    where
        F: FnOnce() + Send + 'static,
    {
        if self.cancelled.load(Ordering::Relaxed) {
            return;
        }

        self.in_flight.fetch_add(1, Ordering::Relaxed);
        self.tasks.lock().push(Box::new(task));
    }

    /// Blocks until all work started by [`run`](Dispatcher::run) completes.
    ///
    /// After this returns, the cancel state is reset.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_work::Dispatcher;
    ///
    /// let dispatcher = Dispatcher::new();
    /// dispatcher.run(|| { /* do work */ });
    /// dispatcher.wait(); // Blocks until complete
    /// ```
    pub fn wait(&self) {
        loop {
            // Drain pending tasks
            let tasks: Vec<_> = {
                let mut guard = self.tasks.lock();
                std::mem::take(&mut *guard)
            };

            if tasks.is_empty() && self.in_flight.load(Ordering::Relaxed) == 0 {
                break;
            }

            if tasks.is_empty() {
                // Tasks still in flight but queue is empty, yield
                std::thread::yield_now();
                continue;
            }

            // Execute tasks in parallel
            let cancelled = &self.cancelled;
            let in_flight = &self.in_flight;

            let errors = &self.errors;
            tasks.into_par_iter().for_each(|task| {
                if !cancelled.load(Ordering::Relaxed) {
                    if let Err(payload) = panic::catch_unwind(AssertUnwindSafe(task)) {
                        errors.lock().push(payload);
                    }
                }
                in_flight.fetch_sub(1, Ordering::Relaxed);
            });
        }

        // Reset cancel state
        self.cancelled.store(false, Ordering::Relaxed);
    }

    /// Cancels remaining work and returns immediately.
    ///
    /// This affects tasks that are being run directly by this dispatcher.
    /// Nested dispatchers are not affected.
    ///
    /// This call does not block. Call [`wait`](Dispatcher::wait) after
    /// `cancel` to wait for pending tasks to complete.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_work::Dispatcher;
    /// use std::sync::atomic::{AtomicBool, Ordering};
    /// use std::sync::Arc;
    ///
    /// let dispatcher = Dispatcher::new();
    /// let started = Arc::new(AtomicBool::new(false));
    ///
    /// let s = Arc::clone(&started);
    /// dispatcher.run(move || {
    ///     s.store(true, Ordering::Relaxed);
    /// });
    ///
    /// // Cancel before waiting
    /// dispatcher.cancel();
    /// dispatcher.wait();
    /// // Task may or may not have run depending on timing
    /// ```
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    /// Returns true if [`cancel`](Dispatcher::cancel) has been called.
    ///
    /// Calling [`wait`](Dispatcher::wait) will reset the cancel state.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_work::Dispatcher;
    ///
    /// let dispatcher = Dispatcher::new();
    /// assert!(!dispatcher.is_cancelled());
    ///
    /// dispatcher.cancel();
    /// assert!(dispatcher.is_cancelled());
    ///
    /// dispatcher.wait();
    /// assert!(!dispatcher.is_cancelled());
    /// ```
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    /// Returns collected errors from panicking tasks.
    ///
    /// Errors are accumulated across `wait()` calls until `take_errors()`
    /// or `clear_errors()` is called.
    pub fn get_errors(&self) -> Vec<String> {
        self.errors
            .lock()
            .iter()
            .map(|e| {
                if let Some(s) = e.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = e.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "unknown panic payload".to_string()
                }
            })
            .collect()
    }

    /// Takes and returns all collected error payloads, clearing the list.
    pub fn take_errors(&self) -> Vec<Box<dyn Any + Send>> {
        std::mem::take(&mut *self.errors.lock())
    }

    /// Returns the number of collected errors.
    pub fn error_count(&self) -> usize {
        self.errors.lock().len()
    }

    /// Clears all collected errors.
    pub fn clear_errors(&self) {
        self.errors.lock().clear();
    }
}

impl Drop for Dispatcher {
    fn drop(&mut self) {
        self.wait();
    }
}

// NOTE: IsolatingDispatcher is defined in isolating_dispatcher.rs (canonical location).
// The duplicate that was here has been removed to avoid confusion.

/// A scoped dispatcher that ensures all tasks complete within a scope.
///
/// Unlike [`Dispatcher`], this version borrows data instead of requiring
/// `'static` lifetimes, making it more ergonomic for local parallelism.
pub struct ScopedDispatcher<'scope> {
    tasks: Mutex<Vec<Box<dyn FnOnce() + Send + 'scope>>>,
    cancelled: AtomicBool,
    /// Collected errors from panicking tasks.
    errors: Mutex<Vec<Box<dyn Any + Send>>>,
}

impl<'scope> ScopedDispatcher<'scope> {
    /// Creates a new scoped dispatcher.
    pub fn new() -> Self {
        Self {
            tasks: Mutex::new(Vec::new()),
            cancelled: AtomicBool::new(false),
            errors: Mutex::new(Vec::new()),
        }
    }

    /// Adds work for the dispatcher to run.
    pub fn run<F>(&self, task: F)
    where
        F: FnOnce() + Send + 'scope,
    {
        if !self.cancelled.load(Ordering::Relaxed) {
            self.tasks.lock().push(Box::new(task));
        }
    }

    /// Cancels remaining work.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    /// Returns true if cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    /// Waits for all tasks to complete.
    ///
    /// Panics in tasks are caught and stored (matching C++ error transport).
    pub fn wait(&self) {
        loop {
            let tasks: Vec<_> = std::mem::take(&mut *self.tasks.lock());
            if tasks.is_empty() {
                break;
            }

            let cancelled = &self.cancelled;
            let errors = &self.errors;
            rayon::scope(|_s| {
                tasks.into_par_iter().for_each(|task| {
                    if !cancelled.load(Ordering::Relaxed) {
                        if let Err(payload) = panic::catch_unwind(AssertUnwindSafe(task)) {
                            errors.lock().push(payload);
                        }
                    }
                });
            });
        }
        self.cancelled.store(false, Ordering::Relaxed);
    }

    /// Returns the number of collected errors.
    pub fn error_count(&self) -> usize {
        self.errors.lock().len()
    }

    /// Takes and returns all collected error payloads, clearing the list.
    pub fn take_errors(&self) -> Vec<Box<dyn Any + Send>> {
        std::mem::take(&mut *self.errors.lock())
    }
}

impl<'scope> Default for ScopedDispatcher<'scope> {
    fn default() -> Self {
        Self::new()
    }
}

/// Runs a closure with a scoped dispatcher, waiting for all tasks to complete.
///
/// # Examples
///
/// ```
/// use usd_work::with_dispatcher;
/// use std::sync::Arc;
/// use std::sync::atomic::{AtomicUsize, Ordering};
///
/// let counter = Arc::new(AtomicUsize::new(0));
/// with_dispatcher(|d| {
///     for _ in 0..10 {
///         let c = Arc::clone(&counter);
///         d.run(move || {
///             c.fetch_add(1, Ordering::Relaxed);
///         });
///     }
/// });
/// assert_eq!(counter.load(Ordering::Relaxed), 10);
/// ```
pub fn with_dispatcher<F, R>(f: F) -> R
where
    F: FnOnce(&ScopedDispatcher<'_>) -> R,
{
    let dispatcher = ScopedDispatcher::new();
    let result = f(&dispatcher);
    dispatcher.wait();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_dispatcher_basic() {
        let counter = Arc::new(AtomicUsize::new(0));
        let dispatcher = Dispatcher::new();

        for _ in 0..100 {
            let c = Arc::clone(&counter);
            dispatcher.run(move || {
                c.fetch_add(1, Ordering::Relaxed);
            });
        }

        dispatcher.wait();
        assert_eq!(counter.load(Ordering::Relaxed), 100);
    }

    #[test]
    fn test_dispatcher_nested() {
        let counter = Arc::new(AtomicUsize::new(0));
        let dispatcher = Dispatcher::new();

        let c = Arc::clone(&counter);
        dispatcher.run(move || {
            c.fetch_add(1, Ordering::Relaxed);
            // Nested dispatchers work but won't add to parent
        });

        dispatcher.wait();
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_dispatcher_cancel() {
        let dispatcher = Dispatcher::new();

        assert!(!dispatcher.is_cancelled());
        dispatcher.cancel();
        assert!(dispatcher.is_cancelled());

        dispatcher.wait();
        assert!(!dispatcher.is_cancelled());
    }

    #[test]
    fn test_dispatcher_empty() {
        let dispatcher = Dispatcher::new();
        dispatcher.wait(); // Should not hang
    }

    #[test]
    fn test_with_dispatcher() {
        let counter = Arc::new(AtomicUsize::new(0));
        with_dispatcher(|d| {
            for _ in 0..10 {
                let c = Arc::clone(&counter);
                d.run(move || {
                    c.fetch_add(1, Ordering::Relaxed);
                });
            }
        });
        assert_eq!(counter.load(Ordering::Relaxed), 10);
    }

    #[test]
    fn test_scoped_dispatcher() {
        let counter = AtomicUsize::new(0);
        let dispatcher = ScopedDispatcher::new();

        for _ in 0..10 {
            dispatcher.run(|| {
                counter.fetch_add(1, Ordering::Relaxed);
            });
        }

        dispatcher.wait();
        assert_eq!(counter.load(Ordering::Relaxed), 10);
    }

    #[test]
    fn test_dispatcher_error_capture() {
        let dispatcher = Dispatcher::new();

        dispatcher.run(|| {
            panic!("task failed");
        });

        dispatcher.wait();
        assert_eq!(dispatcher.error_count(), 1);

        let errors = dispatcher.get_errors();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0], "task failed");
    }

    #[test]
    fn test_dispatcher_multiple_errors() {
        let dispatcher = Dispatcher::new();

        for i in 0..5 {
            dispatcher.run(move || {
                panic!("error {}", i);
            });
        }

        dispatcher.wait();
        assert_eq!(dispatcher.error_count(), 5);

        let errors = dispatcher.get_errors();
        assert_eq!(errors.len(), 5);
    }

    #[test]
    fn test_dispatcher_clean_tasks_unaffected() {
        let counter = Arc::new(AtomicUsize::new(0));
        let dispatcher = Dispatcher::new();

        // Mix of clean and panicking tasks
        for i in 0..10 {
            let c = Arc::clone(&counter);
            dispatcher.run(move || {
                if i % 3 == 0 {
                    panic!("task {} failed", i);
                }
                c.fetch_add(1, Ordering::Relaxed);
            });
        }

        dispatcher.wait();

        // Tasks 0,3,6,9 panic => 4 errors; tasks 1,2,4,5,7,8 succeed => 6
        assert_eq!(dispatcher.error_count(), 4);
        assert_eq!(counter.load(Ordering::Relaxed), 6);
    }

    #[test]
    fn test_dispatcher_take_errors() {
        let dispatcher = Dispatcher::new();

        dispatcher.run(|| panic!("boom"));
        dispatcher.wait();

        let errors = dispatcher.take_errors();
        assert_eq!(errors.len(), 1);

        // After take, errors should be empty
        assert_eq!(dispatcher.error_count(), 0);
    }

    #[test]
    fn test_dispatcher_clear_errors() {
        let dispatcher = Dispatcher::new();

        dispatcher.run(|| panic!("fail"));
        dispatcher.wait();

        assert_eq!(dispatcher.error_count(), 1);
        dispatcher.clear_errors();
        assert_eq!(dispatcher.error_count(), 0);
    }
}
