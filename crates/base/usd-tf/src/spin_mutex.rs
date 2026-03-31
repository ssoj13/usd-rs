//! Spin mutex for low-contention scenarios.
//!
//! This module provides a simple spin lock optimized for throughput when
//! there is little to no contention. Like all spin locks, any contention
//! performs poorly; consider a different synchronization strategy in that case.
//!
//! # Examples
//!
//! ```
//! use usd_tf::spin_mutex::SpinMutex;
//!
//! let mutex = SpinMutex::new();
//!
//! // Scoped locking
//! {
//!     let _guard = mutex.lock();
//!     // ... critical section ...
//! } // lock automatically released
//!
//! // Try-lock pattern
//! {
//!     let guard = mutex.try_lock();
//!     if guard.is_some() {
//!         // ... acquired lock ...
//!     }
//! }
//! ```
//!
//! # Performance Notes
//!
//! This mutex compiles to a minimal instruction sequence for uncontended
//! lock/unlock operations. For contended operations, it uses an out-of-line
//! spinning and backoff strategy with eventual thread yielding.

use std::cell::UnsafeCell;
use std::hint;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

/// Number of spin iterations before yielding to the OS scheduler.
const SPINS_BEFORE_BACKOFF: u32 = 32;

/// A simple spin lock mutex.
///
/// This mutex provides exclusive access to data, spinning while waiting
/// for the lock to become available. Best used when contention is rare
/// and lock hold times are very short.
///
/// # Examples
///
/// ```
/// use usd_tf::spin_mutex::SpinMutex;
///
/// let mutex = SpinMutex::new();
/// let guard = mutex.lock();
/// // Critical section - only one thread can be here
/// drop(guard);
/// ```
pub struct SpinMutex {
    /// The lock state: true = locked, false = unlocked.
    lock_state: AtomicBool,
}

impl SpinMutex {
    /// Creates a new unlocked spin mutex.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::spin_mutex::SpinMutex;
    ///
    /// let mutex = SpinMutex::new();
    /// assert!(mutex.try_lock().is_some());
    /// ```
    #[inline]
    pub const fn new() -> Self {
        Self {
            lock_state: AtomicBool::new(false),
        }
    }

    /// Attempts to acquire the lock without blocking.
    ///
    /// Returns `Some(SpinMutexGuard)` if the lock was acquired,
    /// or `None` if another thread holds the lock.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::spin_mutex::SpinMutex;
    ///
    /// let mutex = SpinMutex::new();
    /// {
    ///     let guard = mutex.try_lock();
    ///     assert!(guard.is_some());
    /// }
    /// ```
    #[inline]
    pub fn try_lock(&self) -> Option<SpinMutexGuard<'_>> {
        // Try to exchange false -> true atomically
        if !self.lock_state.swap(true, Ordering::Acquire) {
            Some(SpinMutexGuard { mutex: self })
        } else {
            None
        }
    }

    /// Acquires the lock, blocking until it becomes available.
    ///
    /// This method will spin while waiting for the lock. If contention
    /// persists, it will yield to allow other threads to run.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::spin_mutex::SpinMutex;
    ///
    /// let mutex = SpinMutex::new();
    /// let guard = mutex.lock();
    /// // ... critical section ...
    /// drop(guard);
    /// ```
    #[inline]
    pub fn lock(&self) -> SpinMutexGuard<'_> {
        // Fast path: uncontended lock
        if let Some(guard) = self.try_lock() {
            return guard;
        }
        // Slow path: contended lock with backoff
        self.lock_contended()
    }

    /// Slow path for contended lock acquisition with backoff strategy.
    #[cold]
    #[inline(never)]
    fn lock_contended(&self) -> SpinMutexGuard<'_> {
        wait_with_backoff(|| self.try_acquire_raw());
        SpinMutexGuard { mutex: self }
    }

    /// Raw try-acquire returning bool (for use in wait_with_backoff).
    #[inline]
    fn try_acquire_raw(&self) -> bool {
        !self.lock_state.swap(true, Ordering::Acquire)
    }

    /// Acquires the lock without returning a guard.
    ///
    /// Callers must ensure they call `release()` when done.
    /// Used by `SpinMutexData` to avoid the mem::forget anti-pattern.
    #[inline]
    pub(crate) fn acquire_raw(&self) {
        if self.try_acquire_raw() {
            return;
        }
        self.acquire_contended_raw();
    }

    /// Slow path for raw contended lock acquisition.
    #[cold]
    #[inline(never)]
    fn acquire_contended_raw(&self) {
        wait_with_backoff(|| self.try_acquire_raw());
    }

    /// Releases the lock.
    ///
    /// # Safety
    ///
    /// This should only be called by code that currently holds the lock.
    /// Normally you should use the `SpinMutexGuard` RAII type instead.
    #[inline]
    pub(crate) fn release(&self) {
        self.lock_state.store(false, Ordering::Release);
    }
}

impl Default for SpinMutex {
    fn default() -> Self {
        Self::new()
    }
}

// SAFETY: SpinMutex can be safely shared between threads - uses atomic operations
#[allow(unsafe_code)]
unsafe impl Send for SpinMutex {}

#[allow(unsafe_code)]
unsafe impl Sync for SpinMutex {}

/// RAII guard for a [`SpinMutex`].
///
/// When this guard is dropped, the lock is automatically released.
///
/// # Examples
///
/// ```
/// use usd_tf::spin_mutex::SpinMutex;
///
/// let mutex = SpinMutex::new();
///
/// {
///     let guard = mutex.lock();
///     // ... critical section ...
/// } // guard dropped, lock released
/// ```
pub struct SpinMutexGuard<'a> {
    mutex: &'a SpinMutex,
}

impl<'a> Drop for SpinMutexGuard<'a> {
    #[inline]
    fn drop(&mut self) {
        self.mutex.release();
    }
}

// SAFETY: SpinMutexGuard can be sent between threads (it owns the lock)
#[allow(unsafe_code)]
unsafe impl<'a> Send for SpinMutexGuard<'a> {}

#[allow(unsafe_code)]
unsafe impl<'a> Sync for SpinMutexGuard<'a> {}

/// A spin mutex that also guards data, similar to `std::sync::Mutex`.
///
/// This provides the same API as [`SpinMutex`] but also stores and
/// provides access to the protected data.
///
/// # Examples
///
/// ```
/// use usd_tf::spin_mutex::SpinMutexData;
///
/// let mutex = SpinMutexData::new(42);
///
/// {
///     let mut guard = mutex.lock();
///     *guard += 1;
/// }
///
/// assert_eq!(*mutex.lock(), 43);
/// ```
pub struct SpinMutexData<T> {
    /// The lock itself.
    mutex: SpinMutex,
    /// The protected data.
    data: UnsafeCell<T>,
}

impl<T> SpinMutexData<T> {
    /// Creates a new mutex protecting the given data.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::spin_mutex::SpinMutexData;
    ///
    /// let mutex = SpinMutexData::new(vec![1, 2, 3]);
    /// ```
    #[inline]
    pub const fn new(data: T) -> Self {
        Self {
            mutex: SpinMutex::new(),
            data: UnsafeCell::new(data),
        }
    }

    /// Attempts to acquire the lock without blocking.
    ///
    /// Returns `Some(guard)` if the lock was acquired, giving access
    /// to the protected data. Returns `None` if the lock is held.
    #[inline]
    pub fn try_lock(&self) -> Option<SpinMutexDataGuard<'_, T>> {
        if self.mutex.try_acquire_raw() {
            Some(SpinMutexDataGuard {
                data: &self.data,
                mutex: &self.mutex,
            })
        } else {
            None
        }
    }

    /// Acquires the lock, blocking until available.
    ///
    /// Returns a guard that provides access to the protected data.
    #[inline]
    pub fn lock(&self) -> SpinMutexDataGuard<'_, T> {
        // Acquire raw lock directly, avoiding SpinMutexGuard creation/forget.
        self.mutex.acquire_raw();
        SpinMutexDataGuard {
            data: &self.data,
            mutex: &self.mutex,
        }
    }

    /// Returns a mutable reference to the underlying data.
    ///
    /// Since this requires `&mut self`, no locking is needed.
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        self.data.get_mut()
    }

    /// Consumes the mutex and returns the inner data.
    #[inline]
    pub fn into_inner(self) -> T {
        self.data.into_inner()
    }
}

impl<T: Default> Default for SpinMutexData<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

// SAFETY: SpinMutexData is Send/Sync if T is Send - mutex provides synchronization
#[allow(unsafe_code)]
unsafe impl<T: Send> Send for SpinMutexData<T> {}

#[allow(unsafe_code)]
unsafe impl<T: Send> Sync for SpinMutexData<T> {}

/// RAII guard for [`SpinMutexData`].
///
/// Provides mutable access to the protected data. The lock is
/// released when this guard is dropped.
pub struct SpinMutexDataGuard<'a, T> {
    data: &'a UnsafeCell<T>,
    mutex: &'a SpinMutex,
}

impl<'a, T> Deref for SpinMutexDataGuard<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        // SAFETY: we hold the lock
        #[allow(unsafe_code)]
        unsafe {
            &*self.data.get()
        }
    }
}

impl<'a, T> DerefMut for SpinMutexDataGuard<'a, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: we hold the lock exclusively
        #[allow(unsafe_code)]
        unsafe {
            &mut *self.data.get()
        }
    }
}

impl<'a, T> Drop for SpinMutexDataGuard<'a, T> {
    #[inline]
    fn drop(&mut self) {
        self.mutex.release();
    }
}

// SAFETY: Guard can be sent if T is Send - owns exclusive lock
#[allow(unsafe_code)]
unsafe impl<'a, T: Send> Send for SpinMutexDataGuard<'a, T> {}

#[allow(unsafe_code)]
unsafe impl<'a, T: Send + Sync> Sync for SpinMutexDataGuard<'a, T> {}

/// Wait with exponential backoff strategy.
///
/// Matches C++ WaitWithBackoff: (1) hopeful first try, (2) spin with pause
/// hints for SPINS_BEFORE_BACKOFF iterations, (3) yield loop.
#[inline]
fn wait_with_backoff<F>(mut try_fn: F)
where
    F: FnMut() -> bool,
{
    // Hope for the best - one extra try before spinning
    if try_fn() {
        return;
    }
    // Spin for a bit with pause hints
    for _ in 0..SPINS_BEFORE_BACKOFF {
        hint::spin_loop();
        if try_fn() {
            return;
        }
    }
    // Keep trying but yield to let other threads run
    loop {
        thread::yield_now();
        if try_fn() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_spin_mutex_basic() {
        let mutex = SpinMutex::new();

        // Can acquire lock
        let guard = mutex.lock();
        drop(guard);

        // Can acquire again
        let _guard = mutex.lock();
    }

    #[test]
    fn test_spin_mutex_try_lock() {
        let mutex = SpinMutex::new();

        // First try_lock succeeds
        let guard = mutex.try_lock();
        assert!(guard.is_some());

        // Second try_lock fails while first is held
        let guard2 = mutex.try_lock();
        assert!(guard2.is_none());

        // After dropping first, try_lock succeeds
        drop(guard);
        let guard3 = mutex.try_lock();
        assert!(guard3.is_some());
    }

    #[test]
    fn test_spin_mutex_data_basic() {
        let mutex = SpinMutexData::new(0);

        {
            let mut guard = mutex.lock();
            *guard = 42;
        }

        assert_eq!(*mutex.lock(), 42);
    }

    #[test]
    fn test_spin_mutex_data_try_lock() {
        let mutex = SpinMutexData::new(100);

        let guard = mutex.try_lock();
        assert!(guard.is_some());
        assert_eq!(*guard.expect("should have guard"), 100);
    }

    #[test]
    fn test_spin_mutex_data_get_mut() {
        let mut mutex = SpinMutexData::new(5);
        *mutex.get_mut() = 10;
        assert_eq!(*mutex.lock(), 10);
    }

    #[test]
    fn test_spin_mutex_data_into_inner() {
        let mutex = SpinMutexData::new(String::from("hello"));
        let value = mutex.into_inner();
        assert_eq!(value, "hello");
    }

    #[test]
    fn test_spin_mutex_concurrent() {
        let mutex = Arc::new(SpinMutexData::new(0));
        let mut handles = vec![];

        for _ in 0..4 {
            let mutex_clone = Arc::clone(&mutex);
            handles.push(thread::spawn(move || {
                for _ in 0..1000 {
                    let mut guard = mutex_clone.lock();
                    *guard += 1;
                }
            }));
        }

        for handle in handles {
            handle.join().expect("thread panicked");
        }

        assert_eq!(*mutex.lock(), 4000);
    }

    #[test]
    fn test_spin_mutex_default() {
        let mutex: SpinMutex = Default::default();
        assert!(mutex.try_lock().is_some());
    }

    #[test]
    fn test_spin_mutex_data_default() {
        let mutex: SpinMutexData<i32> = Default::default();
        assert_eq!(*mutex.lock(), 0);
    }

    #[test]
    fn test_spin_mutex_guard_dropped() {
        let mutex = SpinMutex::new();

        {
            let _guard = mutex.lock();
        } // guard dropped here

        // Should be able to acquire again
        let guard = mutex.try_lock();
        assert!(guard.is_some());
    }
}
