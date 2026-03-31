//! Scalable reader-writer mutex for high read contention.
//!
//! This module provides a readers-writer mutex optimized for workloads with
//! many simultaneous readers and rare writers. It uses multiple cache-line
//! separated lock states to minimize hardware-level contention.
//!
//! # When to Use
//!
//! Use this mutex when:
//! - Many threads frequently acquire read locks concurrently
//! - Write locks are rare
//! - You can afford ~1KB of memory overhead
//!
//! # Performance Characteristics
//!
//! - Read locks: Very fast under contention (typically single cache line)
//! - Write locks: Expensive (must acquire all internal lock states)
//! - Memory: ~1KB (16 cache-line-sized lock states)
//!
//! # Examples
//!
//! ```
//! use usd_tf::big_rw_mutex::BigRWMutex;
//!
//! let mutex = BigRWMutex::new();
//!
//! // Many readers can proceed concurrently
//! {
//!     let _guard1 = mutex.read();
//!     let _guard2 = mutex.read();
//! }
//!
//! // Writer gets exclusive access
//! {
//!     let _guard = mutex.write();
//! }
//! ```

use crate::spin_rw_mutex::SpinRWMutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

/// Number of separate lock states (cache-line-separated).
const NUM_STATES: usize = 16;

/// Cache line size for padding.
const CACHE_LINE_SIZE: usize = 64;

/// A lock state padded to cache line size.
#[repr(C)]
struct LockState {
    mutex: SpinRWMutex,
    _padding: [u8; CACHE_LINE_SIZE - std::mem::size_of::<SpinRWMutex>()],
}

impl LockState {
    const fn new() -> Self {
        Self {
            mutex: SpinRWMutex::new(),
            _padding: [0u8; CACHE_LINE_SIZE - std::mem::size_of::<SpinRWMutex>()],
        }
    }
}

/// A scalable readers-writer mutex for high read contention.
///
/// This mutex trades memory (~1KB) for much better throughput under
/// read-heavy contention. It maintains 16 separate lock states, and
/// readers typically only contend on a single cache line.
///
/// # Design
///
/// - Read locks: Acquire one of 16 internal spin RW mutexes (based on hash)
/// - Write locks: Must acquire all 16 internal mutexes
///
/// # Examples
///
/// ```
/// use usd_tf::big_rw_mutex::BigRWMutex;
/// use std::sync::Arc;
/// use std::thread;
///
/// let mutex = Arc::new(BigRWMutex::new());
///
/// // Spawn multiple reader threads
/// let handles: Vec<_> = (0..4).map(|_| {
///     let m = Arc::clone(&mutex);
///     thread::spawn(move || {
///         for _ in 0..100 {
///             let _guard = m.read();
///             // ... read shared data ...
///         }
///     })
/// }).collect();
///
/// for h in handles {
///     h.join().unwrap();
/// }
/// ```
pub struct BigRWMutex {
    /// Array of cache-line-separated lock states.
    states: Box<[LockState; NUM_STATES]>,
    /// Flag indicating a writer is active or waiting.
    writer_active: AtomicBool,
}

impl BigRWMutex {
    /// Creates a new unlocked mutex.
    #[inline]
    pub fn new() -> Self {
        Self {
            states: Box::new([
                LockState::new(),
                LockState::new(),
                LockState::new(),
                LockState::new(),
                LockState::new(),
                LockState::new(),
                LockState::new(),
                LockState::new(),
                LockState::new(),
                LockState::new(),
                LockState::new(),
                LockState::new(),
                LockState::new(),
                LockState::new(),
                LockState::new(),
                LockState::new(),
            ]),
            writer_active: AtomicBool::new(false),
        }
    }

    /// Acquires a read lock.
    ///
    /// This selects one of the internal lock states based on the calling
    /// thread's characteristics to minimize contention.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::big_rw_mutex::BigRWMutex;
    ///
    /// let mutex = BigRWMutex::new();
    /// let guard = mutex.read();
    /// // ... read shared data ...
    /// ```
    #[inline]
    pub fn read(&self) -> BigReadGuard<'_> {
        let state_index = self.get_state_index();
        self.acquire_read(state_index);
        BigReadGuard {
            mutex: self,
            state_index,
        }
    }

    /// Acquires a write lock.
    ///
    /// This acquires all internal lock states for exclusive access.
    /// It's expensive but guarantees exclusive access.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::big_rw_mutex::BigRWMutex;
    ///
    /// let mutex = BigRWMutex::new();
    /// let guard = mutex.write();
    /// // ... exclusive access ...
    /// ```
    #[inline]
    pub fn write(&self) -> BigWriteGuard<'_> {
        self.acquire_write();
        BigWriteGuard { mutex: self }
    }

    /// Tries to acquire a read lock without blocking on writers.
    #[inline]
    pub fn try_read(&self) -> Option<BigReadGuard<'_>> {
        if self.writer_active.load(Ordering::Acquire) {
            return None;
        }

        let state_index = self.get_state_index();
        if let Some(guard) = self.states[state_index].mutex.try_read() {
            // Don't drop the guard - we manage releases manually
            std::mem::forget(guard);
            Some(BigReadGuard {
                mutex: self,
                state_index,
            })
        } else {
            None
        }
    }

    /// Tries to acquire a write lock without waiting.
    #[inline]
    pub fn try_write(&self) -> Option<BigWriteGuard<'_>> {
        // Try to set writer active flag
        if self.writer_active.swap(true, Ordering::Acquire) {
            // Another writer is active
            return None;
        }

        // Try to acquire all states
        let mut acquired = 0;
        for i in 0..NUM_STATES {
            if let Some(guard) = self.states[i].mutex.try_write() {
                std::mem::forget(guard);
                acquired += 1;
            } else {
                // Release what we acquired
                for j in 0..acquired {
                    self.states[j].mutex.release_write();
                }
                self.writer_active.store(false, Ordering::Release);
                return None;
            }
        }

        Some(BigWriteGuard { mutex: self })
    }

    /// Gets a state index based on the calling stack frame address.
    ///
    /// A local variable's address changes per call site, giving cheap and
    /// well-distributed bucketing without any TLS lookup.
    #[inline]
    fn get_state_index(&self) -> usize {
        let local = 0u8;
        let addr = &local as *const u8 as usize;
        // Mix the address bits so nearby stack frames don't all land in bucket 0
        let mixed = addr ^ (addr >> 7);
        mixed % NUM_STATES
    }

    /// Acquires a read lock on the given state.
    #[inline]
    fn acquire_read(&self, state_index: usize) {
        // Fast path: no writer, try to acquire
        if !self.writer_active.load(Ordering::Acquire) {
            if let Some(guard) = self.states[state_index].mutex.try_read() {
                std::mem::forget(guard);
                return;
            }
        }
        // Slow path: contended
        self.acquire_read_contended(state_index);
    }

    /// Slow path for contended read acquisition.
    #[cold]
    fn acquire_read_contended(&self, state_index: usize) {
        loop {
            if self.writer_active.load(Ordering::Acquire) {
                thread::yield_now();
            } else if let Some(guard) = self.states[state_index].mutex.try_read() {
                std::mem::forget(guard);
                return;
            }
        }
    }

    /// Releases a read lock on the given state.
    #[inline]
    fn release_read(&self, state_index: usize) {
        self.states[state_index].mutex.release_read();
    }

    /// Acquires write lock on all states using a staged approach.
    ///
    /// Phase 1: Set the writer flag on every sub-mutex concurrently (non-blocking).
    /// Phase 2: Poll each sub-mutex until its readers have fully drained.
    ///
    /// This mirrors the C++ `_StagedAcquireWriteStep` pattern: instead of
    /// blocking on sub-mutex 0 until all its readers leave before even touching
    /// sub-mutex 1, we claim the writer flag on all 16 first so that readers on
    /// every bucket start draining in parallel.
    fn acquire_write(&self) {
        // Wait until no other writer is active, then claim the global flag.
        while self.writer_active.swap(true, Ordering::Acquire) {
            loop {
                thread::yield_now();
                if !self.writer_active.load(Ordering::Acquire) {
                    break;
                }
            }
        }

        // Phase 1: claim the writer flag on every sub-mutex without blocking.
        // Tracks which sub-mutexes we still need to fully drain.
        let mut pending = [false; NUM_STATES];
        for i in 0..NUM_STATES {
            if self.states[i].mutex.try_acquire_write_flag() {
                // Flag claimed; check immediately whether readers are already gone.
                pending[i] = !self.states[i].mutex.readers_have_drained();
            } else {
                // Another writer on this sub-mutex — impossible here because the
                // global writer_active flag serialises writers, but handle it
                // defensively by falling back to blocking acquire.
                let guard = self.states[i].mutex.write();
                std::mem::forget(guard);
                pending[i] = false;
            }
        }

        // Phase 2: spin until every sub-mutex reports its readers drained.
        // We poll all pending entries in a round-robin fashion so draining
        // across all 16 buckets happens in parallel rather than sequentially.
        let mut all_done = false;
        let mut spins: u32 = 0;
        while !all_done {
            all_done = true;
            for i in 0..NUM_STATES {
                if pending[i] {
                    if self.states[i].mutex.readers_have_drained() {
                        pending[i] = false;
                    } else {
                        all_done = false;
                    }
                }
            }
            if !all_done {
                spins += 1;
                if spins < 32 {
                    std::hint::spin_loop();
                } else {
                    thread::yield_now();
                    spins = 0;
                }
            }
        }
    }

    /// Releases write lock on all states.
    fn release_write(&self) {
        // Clear writer active flag first
        self.writer_active.store(false, Ordering::Release);

        // Release all write locks
        for i in 0..NUM_STATES {
            self.states[i].mutex.release_write();
        }
    }
}

impl Default for BigRWMutex {
    fn default() -> Self {
        Self::new()
    }
}

// SAFETY: BigRWMutex can be shared between threads - it uses atomic operations
// and internally synchronized SpinRWMutex for all state access
#[allow(unsafe_code)]
unsafe impl Send for BigRWMutex {}

#[allow(unsafe_code)]
unsafe impl Sync for BigRWMutex {}

/// RAII guard for a read lock on [`BigRWMutex`].
pub struct BigReadGuard<'a> {
    mutex: &'a BigRWMutex,
    state_index: usize,
}

impl<'a> Drop for BigReadGuard<'a> {
    fn drop(&mut self) {
        self.mutex.release_read(self.state_index);
    }
}

// SAFETY: BigReadGuard represents shared read access to BigRWMutex
#[allow(unsafe_code)]
unsafe impl<'a> Send for BigReadGuard<'a> {}

#[allow(unsafe_code)]
unsafe impl<'a> Sync for BigReadGuard<'a> {}

/// RAII guard for a write lock on [`BigRWMutex`].
pub struct BigWriteGuard<'a> {
    mutex: &'a BigRWMutex,
}

impl<'a> Drop for BigWriteGuard<'a> {
    fn drop(&mut self) {
        self.mutex.release_write();
    }
}

// SAFETY: BigWriteGuard represents exclusive write access to BigRWMutex
#[allow(unsafe_code)]
unsafe impl<'a> Send for BigWriteGuard<'a> {}

#[allow(unsafe_code)]
unsafe impl<'a> Sync for BigWriteGuard<'a> {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_basic_read() {
        let mutex = BigRWMutex::new();
        let _guard = mutex.read();
    }

    #[test]
    fn test_basic_write() {
        let mutex = BigRWMutex::new();
        let _guard = mutex.write();
    }

    #[test]
    fn test_multiple_readers() {
        let mutex = BigRWMutex::new();
        let _g1 = mutex.read();
        let _g2 = mutex.read();
        let _g3 = mutex.read();
    }

    #[test]
    fn test_try_read_success() {
        let mutex = BigRWMutex::new();
        assert!(mutex.try_read().is_some());
    }

    #[test]
    fn test_try_write_success() {
        let mutex = BigRWMutex::new();
        assert!(mutex.try_write().is_some());
    }

    #[test]
    fn test_concurrent_readers() {
        let mutex = Arc::new(BigRWMutex::new());
        let mut handles = vec![];

        for _ in 0..8 {
            let m = Arc::clone(&mutex);
            handles.push(thread::spawn(move || {
                for _ in 0..1000 {
                    let _guard = m.read();
                    // Simulate some work
                    std::hint::black_box(42);
                }
            }));
        }

        for h in handles {
            h.join().expect("thread panicked");
        }
    }

    #[test]
    fn test_read_write_interleaved() {
        let mutex = Arc::new(BigRWMutex::new());
        let counter = Arc::new(std::sync::atomic::AtomicI32::new(0));

        let mut handles = vec![];

        // Reader threads
        for _ in 0..4 {
            let m = Arc::clone(&mutex);
            let c = Arc::clone(&counter);
            handles.push(thread::spawn(move || {
                for _ in 0..500 {
                    let _guard = m.read();
                    let val = c.load(Ordering::Relaxed);
                    assert!(val >= 0);
                }
            }));
        }

        // Writer threads
        for _ in 0..2 {
            let m = Arc::clone(&mutex);
            let c = Arc::clone(&counter);
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    let _guard = m.write();
                    c.fetch_add(1, Ordering::Relaxed);
                }
            }));
        }

        for h in handles {
            h.join().expect("thread panicked");
        }

        assert_eq!(counter.load(Ordering::Relaxed), 200);
    }

    #[test]
    fn test_guard_drop() {
        let mutex = BigRWMutex::new();

        {
            let _guard = mutex.read();
        }
        // Should be able to acquire write after read is dropped
        let _guard = mutex.write();
    }
}
