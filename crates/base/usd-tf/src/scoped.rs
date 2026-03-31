//! Scoped utilities for RAII-style cleanup.
//!
//! This module provides utilities for executing code when a scope exits,
//! regardless of how it exits (normal flow, early return, panic).
//!
//! # Examples
//!
//! ```
//! use usd_tf::scoped::{Scoped, ScopedVar};
//!
//! // Execute cleanup on scope exit
//! {
//!     let _guard = Scoped::new(|| println!("Cleaning up!"));
//!     // ... do work ...
//! } // "Cleaning up!" printed here
//!
//! // Temporarily change a value
//! let mut value = 10;
//! {
//!     let guard = ScopedVar::new(&mut value, 42);
//!     assert_eq!(*guard.get(), 42); // Access via guard while borrowed
//! }
//! assert_eq!(value, 10); // restored
//! ```

use std::mem::ManuallyDrop;

/// Execute code when this guard is dropped.
///
/// This is useful for cleanup that should happen regardless of how
/// a scope is exited.
///
/// # Examples
///
/// ```
/// use usd_tf::scoped::Scoped;
///
/// let mut cleaned = false;
/// {
///     let cleaned_ref = &mut cleaned;
///     let _guard = Scoped::new(move || {
///         // In real code this would do cleanup
///     });
///     // ... do work ...
/// }
/// ```
pub struct Scoped<F: FnOnce()> {
    on_exit: ManuallyDrop<F>,
}

impl<F: FnOnce()> Scoped<F> {
    /// Create a new scoped guard that executes `on_exit` when dropped.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::scoped::Scoped;
    ///
    /// let _guard = Scoped::new(|| {
    ///     println!("Scope exited");
    /// });
    /// ```
    #[inline]
    pub fn new(on_exit: F) -> Self {
        Self {
            on_exit: ManuallyDrop::new(on_exit),
        }
    }

    /// Dismiss the guard without running the cleanup function.
    ///
    /// This is useful when cleanup should only happen on error paths.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::scoped::Scoped;
    ///
    /// let guard = Scoped::new(|| panic!("Should not run"));
    /// guard.dismiss(); // Cleanup won't run
    /// ```
    #[inline]
    pub fn dismiss(self) {
        // Don't run the closure - just forget self without running drop
        std::mem::forget(self);
    }
}

impl<F: FnOnce()> Drop for Scoped<F> {
    fn drop(&mut self) {
        // SAFETY: on_exit is only taken once during drop, never accessed again
        #[allow(unsafe_code)]
        let on_exit = unsafe { ManuallyDrop::take(&mut self.on_exit) };
        on_exit();
    }
}

/// Temporarily change a variable's value, restoring it when dropped.
///
/// This is useful when you need to temporarily modify state.
///
/// # Examples
///
/// ```
/// use usd_tf::scoped::ScopedVar;
///
/// let mut debug_mode = false;
/// {
///     let guard = ScopedVar::new(&mut debug_mode, true);
///     assert!(*guard.get()); // Access via guard
/// }
/// assert!(!debug_mode); // Restored to original value
/// ```
pub struct ScopedVar<'a, T> {
    var: &'a mut T,
    old_value: T,
}

impl<'a, T> ScopedVar<'a, T> {
    /// Set `var` to `new_value` and restore the old value when dropped.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::scoped::ScopedVar;
    ///
    /// let mut x = 1;
    /// {
    ///     let guard = ScopedVar::new(&mut x, 100);
    ///     assert_eq!(*guard.get(), 100);
    /// }
    /// assert_eq!(x, 1);
    /// ```
    #[inline]
    pub fn new(var: &'a mut T, new_value: T) -> Self {
        let old_value = std::mem::replace(var, new_value);
        Self { var, old_value }
    }

    /// Get a reference to the current value.
    #[inline]
    pub fn get(&self) -> &T {
        self.var
    }

    /// Get a mutable reference to the current value.
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        self.var
    }

    /// Get the original value that will be restored.
    #[inline]
    pub fn original(&self) -> &T {
        &self.old_value
    }
}

impl<T> Drop for ScopedVar<'_, T> {
    fn drop(&mut self) {
        // Swap the old value back, dropping the temporary value
        std::mem::swap(self.var, &mut self.old_value);
    }
}

/// Temporarily increment a counter, decrementing when dropped.
///
/// Useful for tracking nested scopes or recursive depths.
///
/// # Examples
///
/// ```
/// use usd_tf::scoped::ScopedIncrement;
///
/// let mut depth = 0;
/// {
///     let guard = ScopedIncrement::new(&mut depth);
///     assert_eq!(guard.get(), 1); // Access via guard.get()
/// }
/// assert_eq!(depth, 0); // Back to original after guard dropped
/// ```
pub struct ScopedIncrement<'a, T>
where
    T: std::ops::AddAssign<T> + std::ops::SubAssign<T> + From<u8> + Copy,
{
    var: &'a mut T,
}

impl<'a, T> ScopedIncrement<'a, T>
where
    T: std::ops::AddAssign<T> + std::ops::SubAssign<T> + From<u8> + Copy,
{
    /// Increment `var` by 1, decrementing when dropped.
    #[inline]
    pub fn new(var: &'a mut T) -> Self {
        *var += T::from(1);
        Self { var }
    }

    /// Get the current value.
    #[inline]
    pub fn get(&self) -> T {
        *self.var
    }
}

impl<T> Drop for ScopedIncrement<'_, T>
where
    T: std::ops::AddAssign<T> + std::ops::SubAssign<T> + From<u8> + Copy,
{
    fn drop(&mut self) {
        *self.var -= T::from(1);
    }
}

/// A macro to create a scoped guard inline.
///
/// # Examples
///
/// ```
/// use usd_tf::scoped;
///
/// let mut cleaned = false;
/// {
///     let cleaned_ref = &mut cleaned;
///     scoped!(|| *cleaned_ref = true);
/// }
/// assert!(cleaned);
/// ```
#[macro_export]
macro_rules! scoped {
    ($cleanup:expr) => {
        let _guard = $crate::scoped::Scoped::new($cleanup);
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scoped_basic() {
        use std::cell::Cell;
        let cleaned = Cell::new(false);
        {
            let _guard = Scoped::new(|| cleaned.set(true));
            assert!(!cleaned.get());
        }
        assert!(cleaned.get());
    }

    #[test]
    fn test_scoped_dismiss() {
        use std::cell::Cell;
        let cleaned = Cell::new(false);
        {
            let guard = Scoped::new(|| cleaned.set(true));
            guard.dismiss();
        }
        assert!(!cleaned.get());
    }

    #[test]
    fn test_scoped_var_basic() {
        let mut value = 10;
        {
            let guard = ScopedVar::new(&mut value, 42);
            assert_eq!(*guard.get(), 42);
        }
        assert_eq!(value, 10);
    }

    #[test]
    fn test_scoped_var_sequential() {
        // Test that restoration works correctly with sequential guards
        let mut value = 1;
        {
            let guard1 = ScopedVar::new(&mut value, 2);
            assert_eq!(*guard1.get(), 2);
            assert_eq!(*guard1.original(), 1);
        }
        assert_eq!(value, 1);

        {
            let guard2 = ScopedVar::new(&mut value, 3);
            assert_eq!(*guard2.get(), 3);
        }
        assert_eq!(value, 1);
    }

    #[test]
    fn test_scoped_var_accessors() {
        let mut value = "hello".to_string();
        {
            let mut guard = ScopedVar::new(&mut value, "world".to_string());
            assert_eq!(guard.get(), "world");
            assert_eq!(guard.original(), "hello");
            guard.get_mut().push_str("!");
            assert_eq!(*guard.get(), "world!");
        }
        assert_eq!(value, "hello");
    }

    #[test]
    fn test_scoped_increment() {
        let mut counter = 0i32;
        {
            let g1 = ScopedIncrement::new(&mut counter);
            assert_eq!(g1.get(), 1);
            drop(g1);
            // After g1 is dropped, counter is back to 0

            // Create a new guard - counter goes from 0 to 1
            let mut counter2 = counter;
            {
                let g2 = ScopedIncrement::new(&mut counter2);
                assert_eq!(g2.get(), 1); // It's 1, not 2 because we started from 0
            }
            assert_eq!(counter2, 0);
        }
        assert_eq!(counter, 0);
    }

    #[test]
    fn test_scoped_increment_usize() {
        let mut depth: usize = 0;
        {
            let g = ScopedIncrement::new(&mut depth);
            assert_eq!(g.get(), 1);
        }
        assert_eq!(depth, 0);
    }

    #[test]
    fn test_scoped_var_bool() {
        let mut flag = false;
        {
            let guard = ScopedVar::new(&mut flag, true);
            assert!(*guard.get());
        }
        assert!(!flag);
    }

    #[test]
    fn test_scoped_multiple_cleanup() {
        use std::cell::RefCell;
        let order = RefCell::new(Vec::new());
        {
            let _g1 = Scoped::new(|| order.borrow_mut().push(1));
            let _g2 = Scoped::new(|| order.borrow_mut().push(2));
            let _g3 = Scoped::new(|| order.borrow_mut().push(3));
        }
        // Drop in reverse order
        assert_eq!(*order.borrow(), vec![3, 2, 1]);
    }
}
