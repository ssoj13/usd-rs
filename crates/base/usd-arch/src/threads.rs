// SAFETY: This module provides FFI bindings to system APIs requiring unsafe
#![allow(unsafe_code)]

//! Thread utilities.
//!
//! Provides cross-platform thread identification and management.

use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::{self, ThreadId};

/// A unique identifier for a thread.
pub type ThreadIdType = u64;

// Counter for generating sequential thread IDs
static THREAD_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

// Thread-local storage for the sequential thread ID
thread_local! {
    static THREAD_SEQ_ID: u64 = THREAD_ID_COUNTER.fetch_add(1, Ordering::SeqCst);
}

/// OnceLock holding the main thread's ID.
///
/// Populated by `init_main_thread()` or lazily on first call to
/// `main_thread_id()`. If the first call happens from a spawned thread
/// the wrong ID will be recorded -- call `init_main_thread()` early in
/// `main()` to guarantee correctness.
static MAIN_THREAD_ID: std::sync::OnceLock<ThreadId> = std::sync::OnceLock::new();

/// Returns the main thread's ID.
fn main_thread_id() -> ThreadId {
    *MAIN_THREAD_ID.get_or_init(|| thread::current().id())
}

/// Explicitly capture the current thread as "main".
///
/// Call this once, early in `main()`, before spawning any threads that
/// might call `is_main_thread()`. If not called, the first thread to
/// invoke `main_thread_id()` will be recorded as main (which may be a
/// worker thread in some runtimes).
pub fn init_main_thread() {
    MAIN_THREAD_ID.get_or_init(|| thread::current().id());
}

/// Returns true if the current thread is the main thread.
///
/// # Examples
///
/// ```
/// use usd_arch::is_main_thread;
///
/// // In the main thread
/// assert!(is_main_thread());
/// ```
#[must_use]
pub fn is_main_thread() -> bool {
    thread::current().id() == main_thread_id()
}

/// Returns the `std::thread::id` for the thread arch considers "main".
///
/// C++ parity: `ArchGetMainThreadId()`.
#[must_use]
pub fn get_main_thread_id() -> ThreadId {
    main_thread_id()
}

/// Returns the current thread's ID as a numeric value.
///
/// This returns a unique identifier for the current thread that is stable
/// for the lifetime of the thread.
///
/// # Examples
///
/// ```
/// use usd_arch::get_current_thread_id;
///
/// let id = get_current_thread_id();
/// println!("Thread ID: {}", id);
/// ```
#[must_use]
pub fn get_current_thread_id() -> ThreadIdType {
    THREAD_SEQ_ID.with(|id| *id)
}

/// Returns the current thread's name, if it has one.
///
/// # Examples
///
/// ```
/// use usd_arch::get_current_thread_name;
///
/// if let Some(name) = get_current_thread_name() {
///     println!("Thread name: {}", name);
/// }
/// ```
#[must_use]
pub fn get_current_thread_name() -> Option<String> {
    thread::current().name().map(|s| s.to_string())
}

/// Returns the number of hardware threads available.
///
/// This typically corresponds to the number of CPU cores (including
/// hyperthreading/SMT threads).
///
/// # Examples
///
/// ```
/// use usd_arch::get_concurrency;
///
/// let cores = get_concurrency();
/// println!("Available threads: {}", cores);
/// ```
#[must_use]
pub fn get_concurrency() -> usize {
    thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

/// Returns the number of physical CPU cores.
///
/// Note: This may return the same value as `get_concurrency()` on systems
/// where the physical core count cannot be determined.
#[must_use]
pub fn get_physical_concurrency() -> usize {
    // In Rust, there's no standard way to distinguish physical from logical cores.
    // We return the same as get_concurrency().
    get_concurrency()
}

/// Yields execution to other threads.
///
/// This is a hint to the scheduler that the current thread is willing to
/// yield its time slice.
pub fn yield_processor() {
    thread::yield_now();
}

/// A more aggressive spin-wait hint for busy loops.
///
/// On x86, this emits a PAUSE instruction which reduces power consumption
/// and improves performance of spin locks.
#[inline]
pub fn spin_loop_hint() {
    std::hint::spin_loop();
}

/// CPU hint for spin-wait loops. Reduces power and improves performance
/// of spin loops by hinting to the processor.
///
/// Maps to `ARCH_SPIN_PAUSE` in C++:
/// - x86/x86_64: `_mm_pause()` (PAUSE instruction)
/// - aarch64: `__yield` (YIELD instruction)
/// - other: no-op
#[inline(always)]
pub fn spin_pause() {
    // _mm_pause() is safe in Rust (no unsafe needed)
    #[cfg(target_arch = "x86_64")]
    core::arch::x86_64::_mm_pause();
    #[cfg(target_arch = "x86")]
    core::arch::x86::_mm_pause();
    #[cfg(target_arch = "aarch64")]
    {
        std::hint::spin_loop();
    }
    // no-op on other architectures
}

/// Macro alias for `spin_pause()`. Matches the C++ `ARCH_SPIN_PAUSE` macro.
#[macro_export]
macro_rules! spin_pause {
    () => {
        $crate::spin_pause()
    };
}

/// Sets the name of the current thread.
///
/// # Platform Notes
///
/// - On Windows, uses `SetThreadDescription` (Windows 10 1607+)
/// - On Linux, uses `prctl(PR_SET_NAME)` (truncated to 15 bytes)
/// - On macOS, uses `pthread_setname_np`
pub fn set_current_thread_name(name: &str) {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Threading::{GetCurrentThread, SetThreadDescription};
        // Convert to UTF-16 null-terminated wide string
        let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            SetThreadDescription(GetCurrentThread(), wide.as_ptr());
        }
    }

    #[cfg(target_os = "linux")]
    {
        // prctl PR_SET_NAME: max 16 bytes including null terminator
        use std::ffi::CString;
        let truncated = if name.len() > 15 { &name[..15] } else { name };
        if let Ok(cname) = CString::new(truncated) {
            unsafe {
                libc::prctl(libc::PR_SET_NAME, cname.as_ptr() as libc::c_ulong, 0, 0, 0);
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        use std::ffi::CString;
        if let Ok(cname) = CString::new(name) {
            unsafe {
                libc::pthread_setname_np(cname.as_ptr());
            }
        }
    }

    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        let _ = name;
    }
}

/// A thread-local marker that tracks whether the thread is in a critical section.
#[derive(Debug)]
pub struct CriticalSectionMarker {
    depth: std::cell::Cell<u32>,
}

impl CriticalSectionMarker {
    /// Creates a new marker.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            depth: std::cell::Cell::new(0),
        }
    }

    /// Enters a critical section.
    pub fn enter(&self) {
        self.depth.set(self.depth.get() + 1);
    }

    /// Leaves a critical section.
    ///
    /// # Panics
    ///
    /// Panics if not currently in a critical section.
    pub fn leave(&self) {
        let depth = self.depth.get();
        assert!(depth > 0, "leave() called without matching enter()");
        self.depth.set(depth - 1);
    }

    /// Returns true if currently in a critical section.
    #[must_use]
    pub fn is_in_critical_section(&self) -> bool {
        self.depth.get() > 0
    }

    /// Returns the current nesting depth.
    #[must_use]
    pub fn depth(&self) -> u32 {
        self.depth.get()
    }
}

impl Default for CriticalSectionMarker {
    fn default() -> Self {
        Self::new()
    }
}

/// RAII guard for entering a critical section.
pub struct CriticalSectionGuard<'a> {
    marker: &'a CriticalSectionMarker,
}

impl<'a> CriticalSectionGuard<'a> {
    /// Enters the critical section.
    pub fn new(marker: &'a CriticalSectionMarker) -> Self {
        marker.enter();
        Self { marker }
    }
}

impl Drop for CriticalSectionGuard<'_> {
    fn drop(&mut self) {
        self.marker.leave();
    }
}

/// Thread-local storage wrapper with lazy initialization.
///
/// This provides a convenient way to create thread-local values that are
/// initialized on first access.
#[macro_export]
macro_rules! thread_local_lazy {
    ($name:ident : $ty:ty = $init:expr) => {
        thread_local! {
            static $name: std::cell::RefCell<Option<$ty>> = const { std::cell::RefCell::new(None) };
        }

        impl $name {
            #[allow(dead_code)]
            fn with<F, R>(f: F) -> R
            where
                F: FnOnce(&$ty) -> R,
            {
                $name.with(|cell| {
                    let mut borrow = cell.borrow_mut();
                    if borrow.is_none() {
                        *borrow = Some($init);
                    }
                    f(borrow.as_ref().unwrap())
                })
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_is_main_thread() {
        // Note: In Rust's test framework, tests run in separate threads,
        // so this test checks the relative behavior rather than absolute.
        // The first thread to call is_main_thread() will be considered "main".
        let current_is_main = is_main_thread();

        // Spawn a new thread and verify it's different from current thread's status
        let handle = thread::spawn(move || {
            // If current thread was "main", spawned should not be
            // If current thread was not "main" (test runner), spawned is also not main
            let spawned_is_main = is_main_thread();
            // The spawned thread should not be the main thread
            // if the current thread was registered as main
            (current_is_main, spawned_is_main)
        });

        let (current, spawned) = handle.join().expect("Thread panicked");

        // Either current is main and spawned is not, or neither is main
        // (depending on test execution order)
        if current {
            assert!(
                !spawned,
                "Spawned thread should not be main when caller is main"
            );
        }
    }

    #[test]
    fn test_get_current_thread_id() {
        let main_id = get_current_thread_id();

        let handles: Vec<_> = (0..4)
            .map(|_| thread::spawn(|| get_current_thread_id()))
            .collect();

        let ids: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        // All IDs should be unique
        let mut all_ids = vec![main_id];
        all_ids.extend(ids);
        all_ids.sort();
        all_ids.dedup();
        assert_eq!(all_ids.len(), 5);
    }

    #[test]
    fn test_get_concurrency() {
        let cores = get_concurrency();
        assert!(cores >= 1);
    }

    #[test]
    fn test_critical_section_marker() {
        let marker = CriticalSectionMarker::new();
        assert!(!marker.is_in_critical_section());
        assert_eq!(marker.depth(), 0);

        marker.enter();
        assert!(marker.is_in_critical_section());
        assert_eq!(marker.depth(), 1);

        marker.enter();
        assert_eq!(marker.depth(), 2);

        marker.leave();
        assert_eq!(marker.depth(), 1);

        marker.leave();
        assert!(!marker.is_in_critical_section());
    }

    #[test]
    fn test_critical_section_guard() {
        let marker = CriticalSectionMarker::new();

        {
            let _guard = CriticalSectionGuard::new(&marker);
            assert!(marker.is_in_critical_section());
        }

        assert!(!marker.is_in_critical_section());
    }

    #[test]
    fn test_set_current_thread_name() {
        // Should not panic on any platform
        set_current_thread_name("test_thread");
        set_current_thread_name("a_very_long_thread_name_that_exceeds_limits");
        set_current_thread_name("");
    }

    #[test]
    fn test_thread_id_uniqueness_across_threads() {
        let ids = Arc::new(std::sync::Mutex::new(Vec::new()));

        let handles: Vec<_> = (0..10)
            .map(|_| {
                let ids = Arc::clone(&ids);
                thread::spawn(move || {
                    let id = get_current_thread_id();
                    ids.lock().expect("lock poisoned").push(id);
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        let mut ids = ids.lock().expect("lock poisoned").clone();
        let original_len = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), original_len, "Thread IDs should be unique");
    }
}
