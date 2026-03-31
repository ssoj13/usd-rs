//! Readers-writer spin lock for light contention scenarios.
//!
//! This module provides a readers-writer spin lock optimized for throughput
//! when there is light contention or moderate contention dominated by readers.
//! Like all spin locks, significant contention performs poorly.
//!
//! # Features
//!
//! - Multiple concurrent readers
//! - Exclusive writer access
//! - Upgrade from read to write lock
//! - Downgrade from write to read lock
//!
//! # Examples
//!
//! ```
//! use usd_tf::spin_rw_mutex::SpinRWMutex;
//!
//! let mutex = SpinRWMutex::new();
//!
//! // Multiple readers
//! {
//!     let _guard1 = mutex.read();
//!     let _guard2 = mutex.read(); // OK: multiple readers
//! }
//!
//! // Exclusive writer
//! {
//!     let _guard = mutex.write();
//!     // No other readers or writers allowed
//! }
//! ```

use std::cell::UnsafeCell;
use std::hint;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicI32, Ordering};
use std::thread;

/// Number of spin iterations before yielding to the OS scheduler.
const SPINS_BEFORE_BACKOFF: u32 = 32;

/// Reader count increment (bit 0 is reserved for writer flag).
const ONE_READER: i32 = 2;

/// Writer flag (bit 0).
const WRITER_FLAG: i32 = 1;

/// A readers-writer spin lock.
///
/// This lock allows multiple concurrent readers or one exclusive writer.
/// Best used when contention is light or dominated by readers.
///
/// # Lock State Encoding
///
/// The lock state is encoded in a single `i32`:
/// - Bit 0: Writer flag (1 = writer active or waiting)
/// - Bits 1+: Reader count (divided by 2)
///
/// # Examples
///
/// ```
/// use usd_tf::spin_rw_mutex::SpinRWMutex;
///
/// let mutex = SpinRWMutex::new();
///
/// // Read lock
/// {
///     let guard = mutex.read();
///     // ... read shared data ...
/// }
///
/// // Write lock
/// {
///     let guard = mutex.write();
///     // ... write shared data ...
/// }
/// ```
pub struct SpinRWMutex {
    /// Lock state: bit 0 = writer, bits 1+ = reader count * 2
    lock_state: AtomicI32,
}

impl SpinRWMutex {
    /// Creates a new unlocked readers-writer mutex.
    #[inline]
    pub const fn new() -> Self {
        Self {
            lock_state: AtomicI32::new(0),
        }
    }

    /// Attempts to acquire a read lock without blocking.
    ///
    /// Returns `Some(ReadGuard)` if acquired, `None` if a writer is active.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::spin_rw_mutex::SpinRWMutex;
    ///
    /// let mutex = SpinRWMutex::new();
    /// {
    ///     let guard = mutex.try_read();
    ///     assert!(guard.is_some());
    /// }
    /// ```
    #[inline]
    pub fn try_read(&self) -> Option<ReadGuard<'_>> {
        // Optimistically increment reader count
        if self.lock_state.fetch_add(ONE_READER, Ordering::Acquire) & WRITER_FLAG == 0 {
            // No writer activity, we have a read lock
            Some(ReadGuard { mutex: self })
        } else {
            // Writer active, undo increment
            self.lock_state.fetch_sub(ONE_READER, Ordering::Release);
            None
        }
    }

    /// Acquires a read lock, blocking until no writer is active.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::spin_rw_mutex::SpinRWMutex;
    ///
    /// let mutex = SpinRWMutex::new();
    /// let guard = mutex.read();
    /// // ... read data ...
    /// ```
    #[inline]
    pub fn read(&self) -> ReadGuard<'_> {
        loop {
            if let Some(guard) = self.try_read() {
                return guard;
            }
            // Wait for writer to clear
            self.wait_for_writer();
        }
    }

    /// Releases a read lock.
    ///
    /// # Safety
    ///
    /// Caller must hold a read lock. Normally use `ReadGuard` RAII instead.
    #[inline]
    pub fn release_read(&self) {
        self.lock_state.fetch_sub(ONE_READER, Ordering::Release);
    }

    /// Stage 1 of staged write acquisition: atomically set the writer flag.
    ///
    /// Returns `true` if this thread claimed the flag (no other writer was
    /// present), `false` if another writer already owns it.
    ///
    /// After `try_acquire_write_flag` returns `true`, the caller must still
    /// wait for existing readers to drain before the write lock is fully held.
    /// Use `readers_have_drained` to poll for completion, then call
    /// `release_write` to release when done.
    #[inline]
    pub fn try_acquire_write_flag(&self) -> bool {
        self.lock_state.fetch_or(WRITER_FLAG, Ordering::Acquire) & WRITER_FLAG == 0
    }

    /// Returns `true` once all pre-existing readers have drained (only the
    /// writer flag remains in the lock state).
    ///
    /// Only meaningful after `try_acquire_write_flag` returned `true`.
    #[inline]
    pub fn readers_have_drained(&self) -> bool {
        self.lock_state.load(Ordering::Acquire) == WRITER_FLAG
    }

    /// Attempts to acquire a write lock without blocking.
    ///
    /// Returns `Some(WriteGuard)` if acquired, `None` if another writer
    /// is active. Note: this will wait for existing readers to finish.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::spin_rw_mutex::SpinRWMutex;
    ///
    /// let mutex = SpinRWMutex::new();
    /// {
    ///     let guard = mutex.try_write();
    ///     assert!(guard.is_some());
    /// }
    /// ```
    #[inline]
    pub fn try_write(&self) -> Option<WriteGuard<'_>> {
        let state = self.lock_state.fetch_or(WRITER_FLAG, Ordering::Acquire);
        if state & WRITER_FLAG == 0 {
            // We set the flag, wait for existing readers
            if state != 0 {
                self.wait_for_readers();
            }
            Some(WriteGuard { mutex: self })
        } else {
            // Another writer is active
            None
        }
    }

    /// Acquires a write lock, blocking until available.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::spin_rw_mutex::SpinRWMutex;
    ///
    /// let mutex = SpinRWMutex::new();
    /// let guard = mutex.write();
    /// // ... exclusive access ...
    /// ```
    #[inline]
    pub fn write(&self) -> WriteGuard<'_> {
        loop {
            if let Some(guard) = self.try_write() {
                return guard;
            }
            // Wait for other writer to clear
            self.wait_for_writer();
        }
    }

    /// Releases a write lock.
    ///
    /// # Safety
    ///
    /// Caller must hold a write lock. Normally use `WriteGuard` RAII instead.
    #[inline]
    pub fn release_write(&self) {
        self.lock_state.fetch_and(!WRITER_FLAG, Ordering::Release);
    }

    /// Upgrades a read lock to a write lock.
    ///
    /// Returns `true` if the upgrade was atomic (no intervening writer),
    /// `false` if another writer may have acquired the lock in between.
    ///
    /// # Safety
    ///
    /// Caller must hold a read lock. This is typically managed by
    /// `ReadGuard::upgrade()`.
    #[inline]
    fn upgrade_to_writer(&self) -> bool {
        let mut atomic = true;
        loop {
            let state = self.lock_state.fetch_or(WRITER_FLAG, Ordering::Acquire);
            if state & WRITER_FLAG == 0 {
                // We set the flag, release our reader count and wait for others
                if self.lock_state.fetch_sub(ONE_READER, Ordering::AcqRel)
                    != (ONE_READER | WRITER_FLAG)
                {
                    self.wait_for_readers();
                }
                return atomic;
            }
            // Other writer activity, wait and retry
            atomic = false;
            self.wait_for_writer();
        }
    }

    /// Downgrades a write lock to a read lock.
    ///
    /// Returns `true` (always atomic in this implementation).
    ///
    /// # Safety
    ///
    /// Caller must hold a write lock. This is typically managed by
    /// `WriteGuard::downgrade()`.
    #[inline]
    fn downgrade_to_reader(&self) -> bool {
        // Add reader count and clear writer flag atomically
        // Adding (ONE_READER - 1) = (2 - 1) = 1 clears bit 0 and adds reader
        self.lock_state.fetch_add(ONE_READER - 1, Ordering::Release);
        true
    }

    /// Wait for the writer flag to be cleared.
    #[cold]
    fn wait_for_writer(&self) {
        wait_with_backoff(|| self.lock_state.load(Ordering::Relaxed) & WRITER_FLAG == 0);
    }

    /// Wait for all readers to finish (only writer flag remains).
    #[cold]
    fn wait_for_readers(&self) {
        wait_with_backoff(|| self.lock_state.load(Ordering::Relaxed) == WRITER_FLAG);
    }
}

impl Default for SpinRWMutex {
    fn default() -> Self {
        Self::new()
    }
}

// SAFETY: SpinRWMutex can be safely shared between threads - uses atomic operations
#[allow(unsafe_code)]
unsafe impl Send for SpinRWMutex {}

#[allow(unsafe_code)]
unsafe impl Sync for SpinRWMutex {}

/// RAII guard for a read lock on [`SpinRWMutex`].
pub struct ReadGuard<'a> {
    mutex: &'a SpinRWMutex,
}

impl<'a> ReadGuard<'a> {
    /// Upgrades this read lock to a write lock.
    ///
    /// Returns a `WriteGuard` and a boolean indicating if the upgrade
    /// was atomic (no intervening writers).
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::spin_rw_mutex::SpinRWMutex;
    ///
    /// let mutex = SpinRWMutex::new();
    /// let read_guard = mutex.read();
    /// let (write_guard, was_atomic) = read_guard.upgrade();
    /// // Now have exclusive write access
    /// ```
    pub fn upgrade(self) -> (WriteGuard<'a>, bool) {
        let atomic = self.mutex.upgrade_to_writer();
        let mutex = self.mutex;
        // Don't run Drop (which would release read lock)
        std::mem::forget(self);
        (WriteGuard { mutex }, atomic)
    }
}

impl<'a> Drop for ReadGuard<'a> {
    fn drop(&mut self) {
        self.mutex.release_read();
    }
}

// SAFETY: ReadGuard can be sent/synced - holds shared read lock
#[allow(unsafe_code)]
unsafe impl<'a> Send for ReadGuard<'a> {}

#[allow(unsafe_code)]
unsafe impl<'a> Sync for ReadGuard<'a> {}

/// RAII guard for a write lock on [`SpinRWMutex`].
pub struct WriteGuard<'a> {
    mutex: &'a SpinRWMutex,
}

impl<'a> WriteGuard<'a> {
    /// Downgrades this write lock to a read lock.
    ///
    /// Returns a `ReadGuard` and a boolean indicating if the downgrade
    /// was atomic (always true in this implementation).
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::spin_rw_mutex::SpinRWMutex;
    ///
    /// let mutex = SpinRWMutex::new();
    /// let write_guard = mutex.write();
    /// let (read_guard, was_atomic) = write_guard.downgrade();
    /// assert!(was_atomic);
    /// // Now have shared read access
    /// ```
    pub fn downgrade(self) -> (ReadGuard<'a>, bool) {
        let atomic = self.mutex.downgrade_to_reader();
        let mutex = self.mutex;
        // Don't run Drop (which would release write lock)
        std::mem::forget(self);
        (ReadGuard { mutex }, atomic)
    }
}

impl<'a> Drop for WriteGuard<'a> {
    fn drop(&mut self) {
        self.mutex.release_write();
    }
}

// SAFETY: WriteGuard can be sent/synced - holds exclusive write lock
#[allow(unsafe_code)]
unsafe impl<'a> Send for WriteGuard<'a> {}

#[allow(unsafe_code)]
unsafe impl<'a> Sync for WriteGuard<'a> {}

/// A readers-writer mutex that also guards data.
///
/// Similar to `std::sync::RwLock` but using spin locks.
///
/// # Examples
///
/// ```
/// use usd_tf::spin_rw_mutex::SpinRWMutexData;
///
/// let data = SpinRWMutexData::new(vec![1, 2, 3]);
///
/// // Multiple readers
/// {
///     let guard = data.read();
///     assert_eq!(guard.len(), 3);
/// }
///
/// // Exclusive writer
/// {
///     let mut guard = data.write();
///     guard.push(4);
/// }
///
/// assert_eq!(data.read().len(), 4);
/// ```
pub struct SpinRWMutexData<T> {
    mutex: SpinRWMutex,
    data: UnsafeCell<T>,
}

impl<T> SpinRWMutexData<T> {
    /// Creates a new mutex protecting the given data.
    #[inline]
    pub const fn new(data: T) -> Self {
        Self {
            mutex: SpinRWMutex::new(),
            data: UnsafeCell::new(data),
        }
    }

    /// Attempts to acquire a read lock without blocking.
    #[inline]
    pub fn try_read(&self) -> Option<ReadDataGuard<'_, T>> {
        self.mutex.try_read().map(|guard| {
            std::mem::forget(guard);
            ReadDataGuard {
                mutex: &self.mutex,
                data: &self.data,
            }
        })
    }

    /// Acquires a read lock.
    #[inline]
    pub fn read(&self) -> ReadDataGuard<'_, T> {
        let guard = self.mutex.read();
        std::mem::forget(guard);
        ReadDataGuard {
            mutex: &self.mutex,
            data: &self.data,
        }
    }

    /// Attempts to acquire a write lock without blocking.
    #[inline]
    pub fn try_write(&self) -> Option<WriteDataGuard<'_, T>> {
        self.mutex.try_write().map(|guard| {
            std::mem::forget(guard);
            WriteDataGuard {
                mutex: &self.mutex,
                data: &self.data,
            }
        })
    }

    /// Acquires a write lock.
    #[inline]
    pub fn write(&self) -> WriteDataGuard<'_, T> {
        let guard = self.mutex.write();
        std::mem::forget(guard);
        WriteDataGuard {
            mutex: &self.mutex,
            data: &self.data,
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

impl<T: Default> Default for SpinRWMutexData<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

// SAFETY: Send/Sync if T is Send - mutex provides synchronization
#[allow(unsafe_code)]
unsafe impl<T: Send> Send for SpinRWMutexData<T> {}

#[allow(unsafe_code)]
unsafe impl<T: Send + Sync> Sync for SpinRWMutexData<T> {}

/// RAII read guard for [`SpinRWMutexData`].
pub struct ReadDataGuard<'a, T> {
    mutex: &'a SpinRWMutex,
    data: &'a UnsafeCell<T>,
}

impl<'a, T> Deref for ReadDataGuard<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        // SAFETY: we hold a read lock
        #[allow(unsafe_code)]
        unsafe {
            &*self.data.get()
        }
    }
}

impl<'a, T> Drop for ReadDataGuard<'a, T> {
    fn drop(&mut self) {
        self.mutex.release_read();
    }
}

// SAFETY: ReadDataGuard holds shared read access
#[allow(unsafe_code)]
unsafe impl<'a, T: Sync> Send for ReadDataGuard<'a, T> {}

#[allow(unsafe_code)]
unsafe impl<'a, T: Sync> Sync for ReadDataGuard<'a, T> {}

/// RAII write guard for [`SpinRWMutexData`].
pub struct WriteDataGuard<'a, T> {
    mutex: &'a SpinRWMutex,
    data: &'a UnsafeCell<T>,
}

impl<'a, T> Deref for WriteDataGuard<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        // SAFETY: we hold a write lock
        #[allow(unsafe_code)]
        unsafe {
            &*self.data.get()
        }
    }
}

impl<'a, T> DerefMut for WriteDataGuard<'a, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: we hold an exclusive write lock
        #[allow(unsafe_code)]
        unsafe {
            &mut *self.data.get()
        }
    }
}

impl<'a, T> Drop for WriteDataGuard<'a, T> {
    fn drop(&mut self) {
        self.mutex.release_write();
    }
}

// SAFETY: WriteDataGuard holds exclusive write access
#[allow(unsafe_code)]
unsafe impl<'a, T: Send> Send for WriteDataGuard<'a, T> {}

#[allow(unsafe_code)]
unsafe impl<'a, T: Send + Sync> Sync for WriteDataGuard<'a, T> {}

/// Wait with exponential backoff strategy.
#[inline]
fn wait_with_backoff<F>(mut try_fn: F)
where
    F: FnMut() -> bool,
{
    // Try once
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

    // Yield to scheduler
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
    fn test_basic_read() {
        let mutex = SpinRWMutex::new();
        let _guard = mutex.read();
    }

    #[test]
    fn test_basic_write() {
        let mutex = SpinRWMutex::new();
        let _guard = mutex.write();
    }

    #[test]
    fn test_multiple_readers() {
        let mutex = SpinRWMutex::new();
        let _guard1 = mutex.read();
        let _guard2 = mutex.read();
        let _guard3 = mutex.read();
    }

    #[test]
    fn test_try_read() {
        let mutex = SpinRWMutex::new();
        {
            let guard = mutex.try_read();
            assert!(guard.is_some());
        }
    }

    #[test]
    fn test_try_write() {
        let mutex = SpinRWMutex::new();
        {
            let guard = mutex.try_write();
            assert!(guard.is_some());
        }
    }

    #[test]
    fn test_try_write_blocked_by_writer() {
        let mutex = SpinRWMutex::new();
        let _write_guard = mutex.write();
        // Another try_write should fail
        assert!(mutex.try_write().is_none());
    }

    #[test]
    fn test_try_read_blocked_by_writer() {
        let mutex = SpinRWMutex::new();
        let _write_guard = mutex.write();
        // try_read should fail while writer holds lock
        assert!(mutex.try_read().is_none());
    }

    #[test]
    fn test_upgrade() {
        let mutex = SpinRWMutex::new();
        let read_guard = mutex.read();
        let (write_guard, was_atomic) = read_guard.upgrade();
        assert!(was_atomic);
        drop(write_guard);
    }

    #[test]
    fn test_downgrade() {
        let mutex = SpinRWMutex::new();
        let write_guard = mutex.write();
        let (read_guard, was_atomic) = write_guard.downgrade();
        assert!(was_atomic);
        // Can get another read lock now
        let _read_guard2 = mutex.read();
        drop(read_guard);
    }

    #[test]
    fn test_data_read() {
        let data = SpinRWMutexData::new(42);
        assert_eq!(*data.read(), 42);
    }

    #[test]
    fn test_data_write() {
        let data = SpinRWMutexData::new(0);
        *data.write() = 100;
        assert_eq!(*data.read(), 100);
    }

    #[test]
    fn test_data_multiple_readers() {
        let data = SpinRWMutexData::new(vec![1, 2, 3]);
        let guard1 = data.read();
        let guard2 = data.read();
        assert_eq!(guard1.len(), 3);
        assert_eq!(guard2.len(), 3);
    }

    #[test]
    fn test_data_get_mut() {
        let mut data = SpinRWMutexData::new(5);
        *data.get_mut() = 10;
        assert_eq!(*data.read(), 10);
    }

    #[test]
    fn test_data_into_inner() {
        let data = SpinRWMutexData::new(String::from("hello"));
        let s = data.into_inner();
        assert_eq!(s, "hello");
    }

    #[test]
    fn test_concurrent_readers() {
        let data = Arc::new(SpinRWMutexData::new(42));
        let mut handles = vec![];

        for _ in 0..4 {
            let data_clone = Arc::clone(&data);
            handles.push(thread::spawn(move || {
                for _ in 0..1000 {
                    let guard = data_clone.read();
                    assert_eq!(*guard, 42);
                }
            }));
        }

        for handle in handles {
            handle.join().expect("thread panicked");
        }
    }

    #[test]
    fn test_concurrent_writers() {
        let data = Arc::new(SpinRWMutexData::new(0));
        let mut handles = vec![];

        for _ in 0..4 {
            let data_clone = Arc::clone(&data);
            handles.push(thread::spawn(move || {
                for _ in 0..1000 {
                    let mut guard = data_clone.write();
                    *guard += 1;
                }
            }));
        }

        for handle in handles {
            handle.join().expect("thread panicked");
        }

        assert_eq!(*data.read(), 4000);
    }

    #[test]
    fn test_concurrent_read_write() {
        let data = Arc::new(SpinRWMutexData::new(0i64));
        let mut handles = vec![];

        // Writers
        for _ in 0..2 {
            let data_clone = Arc::clone(&data);
            handles.push(thread::spawn(move || {
                for _ in 0..500 {
                    let mut guard = data_clone.write();
                    *guard += 1;
                }
            }));
        }

        // Readers
        for _ in 0..2 {
            let data_clone = Arc::clone(&data);
            handles.push(thread::spawn(move || {
                for _ in 0..500 {
                    let guard = data_clone.read();
                    // Value should always be non-negative
                    assert!(*guard >= 0);
                }
            }));
        }

        for handle in handles {
            handle.join().expect("thread panicked");
        }

        assert_eq!(*data.read(), 1000);
    }
}
