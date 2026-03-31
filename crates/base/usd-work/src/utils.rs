//! Work utilities.
//!
//! This module provides utility functions for asynchronous operations,
//! including async destruction of large objects.
//!
//! # Examples
//!
//! ```
//! use usd_work::{swap_destroy_async, move_destroy_async};
//!
//! // Swap a large vector with an empty one and destroy the old one async
//! let mut big_vec: Vec<u8> = vec![0; 1_000_000];
//! swap_destroy_async(&mut big_vec);
//! assert!(big_vec.is_empty()); // big_vec is now empty, old data being destroyed async
//!
//! // Move from an object and destroy async
//! let mut big_string = "x".repeat(1_000_000);
//! move_destroy_async(&mut big_string);
//! assert!(big_string.is_empty()); // big_string is now in moved-from state
//! ```

use super::run_detached_task;
use std::sync::atomic::{AtomicBool, Ordering};

/// Global flag to force synchronous destruction (for testing/debugging).
static SYNCHRONIZE_ASYNC_DESTROY: AtomicBool = AtomicBool::new(false);

/// Returns true if async destroy calls should be forced to run synchronously.
///
/// This is useful for debugging memory issues or ensuring deterministic
/// destruction order in tests.
pub fn should_synchronize_async_destroy() -> bool {
    SYNCHRONIZE_ASYNC_DESTROY.load(Ordering::Relaxed)
}

/// Sets whether async destroy calls should be forced to run synchronously.
///
/// When set to `true`, [`swap_destroy_async`] and [`move_destroy_async`]
/// will destroy objects synchronously instead of on a background thread.
///
/// # Examples
///
/// ```
/// use usd_work::{set_synchronize_async_destroy, swap_destroy_async};
///
/// // Force synchronous destruction for testing
/// set_synchronize_async_destroy(true);
///
/// let mut data = vec![1, 2, 3];
/// swap_destroy_async(&mut data); // Now destroys synchronously
///
/// // Reset to async behavior
/// set_synchronize_async_destroy(false);
/// ```
pub fn set_synchronize_async_destroy(sync: bool) {
    SYNCHRONIZE_ASYNC_DESTROY.store(sync, Ordering::Relaxed);
}

/// Swaps `obj` with a default-constructed instance and arranges for the
/// swapped-out instance to be destroyed asynchronously.
///
/// After this call, `obj` will be in its default state, and the original
/// contents will be destroyed on a background thread (unless
/// [`set_synchronize_async_destroy`] has been called with `true`).
///
/// # Safety Considerations
///
/// Any code that the destructor might invoke must be safe to run:
/// - Concurrently with other code
/// - At any point in the future
///
/// This might NOT be safe if the destructor tries to update data structures
/// that could be destroyed by the time the destruction occurs.
///
/// # Examples
///
/// ```
/// use usd_work::swap_destroy_async;
///
/// let mut large_vec = vec![0u8; 1_000_000];
/// swap_destroy_async(&mut large_vec);
///
/// // large_vec is now empty, original data being destroyed async
/// assert!(large_vec.is_empty());
/// ```
pub fn swap_destroy_async<T: Default + Send + 'static>(obj: &mut T) {
    let mut temp = T::default();
    std::mem::swap(obj, &mut temp);

    if should_synchronize_async_destroy() {
        drop(temp); // Synchronous destruction
    } else {
        run_detached_task(move || {
            drop(temp); // Async destruction
        });
    }
}

/// Moves from `obj`, leaving it in a moved-from state, and arranges for
/// the moved-out value to be destroyed asynchronously.
///
/// Unlike [`swap_destroy_async`], this leaves `obj` in an unspecified but
/// valid moved-from state rather than a default-constructed state.
///
/// # Safety Considerations
///
/// Same as [`swap_destroy_async`] - the destructor must be safe to run
/// concurrently and at any future time.
///
/// # Examples
///
/// ```
/// use usd_work::move_destroy_async;
///
/// let mut large_string = "x".repeat(1_000_000);
/// move_destroy_async(&mut large_string);
///
/// // large_string is now in moved-from state (empty for String)
/// assert!(large_string.is_empty());
/// ```
pub fn move_destroy_async<T: Default + Send + 'static>(obj: &mut T) {
    let temp = std::mem::take(obj);

    if should_synchronize_async_destroy() {
        drop(temp);
    } else {
        run_detached_task(move || {
            drop(temp);
        });
    }
}

/// Destroys a value asynchronously.
///
/// Takes ownership of the value and destroys it on a background thread.
///
/// # Examples
///
/// ```
/// use usd_work::destroy_async;
///
/// let large_vec = vec![0u8; 1_000_000];
/// destroy_async(large_vec);
/// // large_vec is now being destroyed async (ownership transferred)
/// ```
pub fn destroy_async<T: Send + 'static>(obj: T) {
    if should_synchronize_async_destroy() {
        drop(obj);
    } else {
        run_detached_task(move || {
            drop(obj);
        });
    }
}

/// RAII guard that destroys a value asynchronously when dropped.
///
/// # Examples
///
/// ```
/// use usd_work::AsyncDestroyGuard;
///
/// {
///     let _guard = AsyncDestroyGuard::new(vec![0u8; 1_000_000]);
///     // ... use the vector via _guard ...
/// } // Vector destroyed async when guard drops
/// ```
pub struct AsyncDestroyGuard<T: Send + 'static> {
    value: Option<T>,
}

impl<T: Send + 'static> AsyncDestroyGuard<T> {
    /// Creates a new async destroy guard wrapping `value`.
    pub fn new(value: T) -> Self {
        Self { value: Some(value) }
    }

    /// Returns a reference to the wrapped value.
    pub fn get(&self) -> &T {
        self.value.as_ref().expect("Value already taken")
    }

    /// Returns a mutable reference to the wrapped value.
    pub fn get_mut(&mut self) -> &mut T {
        self.value.as_mut().expect("Value already taken")
    }

    /// Takes the value out, preventing async destruction.
    pub fn take(mut self) -> T {
        self.value.take().expect("Value already taken")
    }
}

impl<T: Send + 'static> std::ops::Deref for AsyncDestroyGuard<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

impl<T: Send + 'static> std::ops::DerefMut for AsyncDestroyGuard<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.get_mut()
    }
}

impl<T: Send + 'static> Drop for AsyncDestroyGuard<T> {
    fn drop(&mut self) {
        if let Some(value) = self.value.take() {
            destroy_async(value);
        }
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
    fn test_swap_destroy_async() {
        let mut data = vec![1, 2, 3];
        swap_destroy_async(&mut data);
        assert!(data.is_empty());
    }

    #[test]
    fn test_move_destroy_async() {
        let mut data = String::from("hello");
        move_destroy_async(&mut data);
        assert!(data.is_empty());
    }

    #[test]
    fn test_destroy_async() {
        let counter = Arc::new(AtomicUsize::new(0));

        struct Counter(Arc<AtomicUsize>);
        impl Drop for Counter {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::Relaxed);
            }
        }

        let c = Counter(Arc::clone(&counter));
        destroy_async(c);

        // Wait for async destruction
        thread::sleep(Duration::from_millis(50));
        assert_eq!(counter.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_synchronize_async_destroy() {
        set_synchronize_async_destroy(true);

        let counter = Arc::new(AtomicUsize::new(0));

        struct Counter(Arc<AtomicUsize>);
        impl Drop for Counter {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::Relaxed);
            }
        }

        let c = Counter(Arc::clone(&counter));
        destroy_async(c);

        // With sync mode, destruction is immediate
        assert_eq!(counter.load(Ordering::Relaxed), 1);

        set_synchronize_async_destroy(false);
    }

    #[test]
    fn test_async_destroy_guard() {
        let guard = AsyncDestroyGuard::new(vec![1, 2, 3]);
        assert_eq!(guard.len(), 3);
        assert_eq!(guard[0], 1);
    }

    #[test]
    fn test_async_destroy_guard_take() {
        let guard = AsyncDestroyGuard::new(vec![1, 2, 3]);
        let vec = guard.take();
        assert_eq!(vec, vec![1, 2, 3]);
    }

    #[test]
    fn test_async_destroy_guard_deref_mut() {
        let mut guard = AsyncDestroyGuard::new(vec![1, 2, 3]);
        guard.push(4);
        assert_eq!(guard.len(), 4);
    }
}
