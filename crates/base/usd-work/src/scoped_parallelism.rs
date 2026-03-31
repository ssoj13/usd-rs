//! Scoped parallelism utilities.
//!
//! Functions for executing code in an isolated parallel context where
//! wait operations only take tasks created within the current scope.
//!
//! # Problem
//!
//! When a thread waits on concurrent work, it normally helps by executing
//! other available tasks. This can be problematic when holding locks, as
//! the waiting thread might pick up unrelated tasks that try to acquire
//! the same lock, causing deadlock.
//!
//! # Solution
//!
//! [`with_scoped_parallelism`] ensures that wait operations only process
//! tasks created within the function's scope.
//!
//! # Examples
//!
//! ```
//! use usd_work::{with_scoped_parallelism, Dispatcher};
//!
//! fn get_resource() {
//!     // Safe: waits only take tasks from this scope
//!     with_scoped_parallelism(|| {
//!         let dispatcher = Dispatcher::new();
//!         dispatcher.run(|| println!("Task 1"));
//!         dispatcher.run(|| println!("Task 2"));
//!         dispatcher.wait();
//!     });
//! }
//! ```

use super::Dispatcher;

/// Execute a function with scoped parallelism.
///
/// All wait operations on concurrent constructs within `fn` will only
/// take tasks created within this scope. This prevents deadlocks when
/// waiting while holding locks.
///
/// # Examples
///
/// ```
/// use usd_work::with_scoped_parallelism;
///
/// with_scoped_parallelism(|| {
///     // Parallel work here is isolated
///     println!("Scoped work");
/// });
/// ```
pub fn with_scoped_parallelism<F, R>(f: F) -> R
where
    F: FnOnce() -> R + Send,
    R: Send,
{
    // Use rayon's scope for isolation
    let mut result = None;
    rayon::scope(|_| {
        result = Some(f());
    });
    result.expect("Scoped parallelism function panicked")
}

/// Execute a function with a scoped dispatcher.
///
/// Creates a [`Dispatcher`] and passes it to the function. After the
/// function returns, waits for all dispatcher tasks to complete before
/// the scoped parallelism ends.
///
/// # Examples
///
/// ```
/// use usd_work::with_scoped_dispatcher;
///
/// with_scoped_dispatcher(|dispatcher| {
///     dispatcher.run(|| println!("Task 1"));
///     dispatcher.run(|| println!("Task 2"));
///     // Dispatcher automatically waits when scope ends
/// });
/// ```
pub fn with_scoped_dispatcher<F, R>(f: F) -> R
where
    F: FnOnce(&Dispatcher) -> R + Send,
    R: Send,
{
    with_scoped_parallelism(|| {
        let dispatcher = Dispatcher::new();
        let result = f(&dispatcher);
        dispatcher.wait();
        result
    })
}

/// Execute a function with scoped parallelism, taking a mutable dispatcher.
///
/// Similar to [`with_scoped_dispatcher`] but provides mutable access to
/// the dispatcher.
pub fn with_scoped_dispatcher_mut<F, R>(f: F) -> R
where
    F: FnOnce(&mut Dispatcher) -> R + Send,
    R: Send,
{
    with_scoped_parallelism(|| {
        let mut dispatcher = Dispatcher::new();
        let result = f(&mut dispatcher);
        dispatcher.wait();
        result
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicI32, Ordering};

    #[test]
    fn test_with_scoped_parallelism() {
        let counter = Arc::new(AtomicI32::new(0));

        let c = Arc::clone(&counter);
        with_scoped_parallelism(move || {
            c.fetch_add(42, Ordering::SeqCst);
        });

        assert_eq!(counter.load(Ordering::SeqCst), 42);
    }

    #[test]
    fn test_with_scoped_parallelism_return() {
        let result = with_scoped_parallelism(|| 42 + 1);
        assert_eq!(result, 43);
    }

    #[test]
    fn test_with_scoped_dispatcher() {
        let counter = Arc::new(AtomicI32::new(0));

        let c = Arc::clone(&counter);
        with_scoped_dispatcher(move |d| {
            for _ in 0..10 {
                let cc = Arc::clone(&c);
                d.run(move || {
                    cc.fetch_add(1, Ordering::SeqCst);
                });
            }
        });

        assert_eq!(counter.load(Ordering::SeqCst), 10);
    }

    #[test]
    fn test_with_scoped_dispatcher_return() {
        let result = with_scoped_dispatcher(|d| {
            d.run(|| {});
            "done"
        });
        assert_eq!(result, "done");
    }

    #[test]
    fn test_nested_scoped_parallelism() {
        let counter = Arc::new(AtomicI32::new(0));

        let c = Arc::clone(&counter);
        with_scoped_parallelism(move || {
            c.fetch_add(1, Ordering::SeqCst);

            let cc = Arc::clone(&c);
            with_scoped_parallelism(move || {
                cc.fetch_add(10, Ordering::SeqCst);
            });

            c.fetch_add(100, Ordering::SeqCst);
        });

        assert_eq!(counter.load(Ordering::SeqCst), 111);
    }
}
