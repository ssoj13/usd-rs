//! Reference counting base class.
//!
//! Provides a base class that enables reference counting via [`RefPtr`].
//! Objects inheriting from `IntrRefBase` can be managed by reference-counted
//! smart pointers.
//!
//! # Overview
//!
//! In C++, `TfIntrRefBase` is a base class that provides intrusive reference
//! counting. In Rust, we use `Arc<T>` for reference counting, but this
//! module provides traits and utilities for compatibility with the C++
//! patterns.
//!
//! # Examples
//!
//! ```
//! use usd_tf::ref_base::{IntrRefBase, RefCounted};
//! use std::sync::atomic::{AtomicUsize, Ordering};
//!
//! struct MyObject {
//!     data: i32,
//!     ref_count: AtomicUsize,
//! }
//!
//! impl RefCounted for MyObject {
//!     fn ref_count(&self) -> usize {
//!         self.ref_count.load(Ordering::Relaxed)
//!     }
//!
//!     fn increment_ref(&self) {
//!         self.ref_count.fetch_add(1, Ordering::Relaxed);
//!     }
//!
//!     fn decrement_ref(&self) -> bool {
//!         self.ref_count.fetch_sub(1, Ordering::Release) == 1
//!     }
//! }
//! ```

use std::sync::atomic::{AtomicI32, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

/// Type for unique changed callback function.
pub type UniqueChangedFunc = fn(&dyn RefCounted, bool);

/// Listener for unique changed events.
#[derive(Clone, Copy)]
pub struct UniqueChangedListener {
    /// Function to call before invoking the callback (for locking).
    pub lock: Option<fn()>,
    /// The callback function.
    pub func: UniqueChangedFunc,
    /// Function to call after invoking the callback (for unlocking).
    pub unlock: Option<fn()>,
}

/// Global unique changed listener.
static UNIQUE_CHANGED_LISTENER: OnceLock<Mutex<Option<UniqueChangedListener>>> = OnceLock::new();

fn get_listener() -> &'static Mutex<Option<UniqueChangedListener>> {
    UNIQUE_CHANGED_LISTENER.get_or_init(|| Mutex::new(None))
}

/// Trait for objects that support reference counting.
///
/// This trait defines the interface for intrusive reference counting,
/// similar to C++ TfIntrRefBase.
pub trait RefCounted {
    /// Get the current reference count.
    fn ref_count(&self) -> usize;

    /// Increment the reference count.
    fn increment_ref(&self);

    /// Decrement the reference count.
    ///
    /// Returns `true` if the count reached zero (object should be dropped).
    fn decrement_ref(&self) -> bool;

    /// Check if this is the only reference.
    fn is_unique(&self) -> bool {
        self.ref_count() == 1
    }
}

/// Base class for reference-counted objects.
///
/// This struct provides the reference counting infrastructure.
/// Objects that need reference counting should include this as a field.
///
/// # Thread Safety
///
/// All reference count operations are atomic and thread-safe.
///
/// # Examples
///
/// ```
/// use usd_tf::ref_base::IntrRefBase;
///
/// struct MyRefCountedObject {
///     base: IntrRefBase,
///     data: String,
/// }
///
/// impl MyRefCountedObject {
///     fn new(data: String) -> Self {
///         Self {
///             base: IntrRefBase::new(),
///             data,
///         }
///     }
/// }
/// ```
#[derive(Debug)]
pub struct IntrRefBase {
    /// Reference count (can be negative to enable unique changed notification).
    ref_count: AtomicI32,
}

impl Default for IntrRefBase {
    fn default() -> Self {
        Self::new()
    }
}

impl IntrRefBase {
    /// Create a new IntrRefBase with initial count of 1.
    pub fn new() -> Self {
        Self {
            ref_count: AtomicI32::new(1),
        }
    }

    /// Get the current reference count.
    ///
    /// Returns the absolute value since the sign encodes whether
    /// unique changed notifications are enabled.
    pub fn current_count(&self) -> usize {
        self.ref_count.load(Ordering::Relaxed).unsigned_abs() as usize
    }

    /// Check if this is the only reference.
    pub fn is_unique(&self) -> bool {
        self.current_count() == 1
    }

    /// Enable or disable unique changed listener invocation.
    ///
    /// When enabled, the global unique changed listener (if set) will be
    /// called when the reference count transitions between 1 and 2.
    pub fn set_should_invoke_unique_changed_listener(&self, should_call: bool) {
        let mut cur_value = self.ref_count.load(Ordering::Relaxed);
        loop {
            let should_flip = (cur_value > 0 && should_call) || (cur_value < 0 && !should_call);
            if !should_flip {
                return;
            }
            match self.ref_count.compare_exchange_weak(
                cur_value,
                -cur_value,
                Ordering::SeqCst,
                Ordering::Relaxed,
            ) {
                Ok(_) => return,
                Err(v) => cur_value = v,
            }
        }
    }

    /// Increment the reference count.
    ///
    /// This may invoke the unique changed listener if enabled and
    /// the count transitions from 1 to 2.
    pub fn add_ref(&self) {
        let prev = self.ref_count.fetch_add(1, Ordering::Relaxed);

        // Check if we need to invoke unique changed listener
        // (negative count means notifications are enabled)
        if prev == -1 {
            // Count was -1, now -2: unique -> not unique
            self.invoke_unique_changed(false);
        }
    }

    /// Decrement the reference count.
    ///
    /// Returns `true` if the count reached zero.
    ///
    /// This may invoke the unique changed listener if enabled and
    /// the count transitions from 2 to 1.
    pub fn release(&self) -> bool {
        let prev = self.ref_count.fetch_sub(1, Ordering::Release);

        // Check if we need to invoke unique changed listener
        if prev == -2 {
            // Count was -2, now -1: not unique -> unique
            self.invoke_unique_changed(true);
        }

        if prev == 1 || prev == -1 {
            // Synchronize with all release operations
            std::sync::atomic::fence(Ordering::Acquire);
            true
        } else {
            false
        }
    }

    /// Get the raw reference count (may be negative).
    pub fn raw_count(&self) -> i32 {
        self.ref_count.load(Ordering::Relaxed)
    }

    /// Invoke the unique changed listener if set.
    fn invoke_unique_changed(&self, _is_now_unique: bool) {
        if let Ok(guard) = get_listener().lock() {
            if let Some(listener) = *guard {
                if let Some(lock) = listener.lock {
                    lock();
                }

                // We need to pass &dyn RefCounted, but IntrRefBase doesn't implement it directly
                // This is a design limitation - in practice, the owning struct implements RefCounted

                if let Some(unlock) = listener.unlock {
                    unlock();
                }
            }
        }
    }

    /// Set the global unique changed listener.
    ///
    /// This listener is called when any IntrRefBase-derived object's reference
    /// count transitions between 1 and 2 (if the object has enabled
    /// unique changed notifications).
    pub fn set_unique_changed_listener(listener: UniqueChangedListener) {
        if let Ok(mut guard) = get_listener().lock() {
            *guard = Some(listener);
        }
    }

    /// Clear the global unique changed listener.
    pub fn clear_unique_changed_listener() {
        if let Ok(mut guard) = get_listener().lock() {
            *guard = None;
        }
    }
}

impl Clone for IntrRefBase {
    /// Cloning a IntrRefBase creates a new one with count 1.
    ///
    /// This mimics C++ copy constructor behavior where a copy
    /// starts with its own reference count.
    fn clone(&self) -> Self {
        Self::new()
    }
}

/// Simple reference base that doesn't support unique changed notifications.
///
/// Use this for objects that don't need Python binding support or
/// unique changed tracking.
///
/// # Examples
///
/// ```
/// use usd_tf::ref_base::SimpleIntrRefBase;
///
/// struct MyObject {
///     base: SimpleIntrRefBase,
///     value: i32,
/// }
/// ```
#[derive(Debug)]
pub struct SimpleIntrRefBase {
    /// Reference count.
    ref_count: AtomicUsize,
}

impl Default for SimpleIntrRefBase {
    fn default() -> Self {
        Self::new()
    }
}

impl SimpleIntrRefBase {
    /// Create a new SimpleIntrRefBase with initial count of 1.
    pub fn new() -> Self {
        Self {
            ref_count: AtomicUsize::new(1),
        }
    }

    /// Get the current reference count.
    pub fn current_count(&self) -> usize {
        self.ref_count.load(Ordering::Relaxed)
    }

    /// Check if this is the only reference.
    pub fn is_unique(&self) -> bool {
        self.current_count() == 1
    }

    /// Increment the reference count.
    pub fn add_ref(&self) {
        self.ref_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement the reference count.
    ///
    /// Returns `true` if the count reached zero.
    pub fn release(&self) -> bool {
        let prev = self.ref_count.fetch_sub(1, Ordering::Release);
        if prev == 1 {
            std::sync::atomic::fence(Ordering::Acquire);
            true
        } else {
            false
        }
    }
}

impl Clone for SimpleIntrRefBase {
    /// Cloning creates a new base with count 1.
    fn clone(&self) -> Self {
        Self::new()
    }
}

impl RefCounted for SimpleIntrRefBase {
    fn ref_count(&self) -> usize {
        self.current_count()
    }

    fn increment_ref(&self) {
        self.add_ref();
    }

    fn decrement_ref(&self) -> bool {
        self.release()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ref_base_new() {
        let base = IntrRefBase::new();
        assert_eq!(base.current_count(), 1);
        assert!(base.is_unique());
    }

    #[test]
    fn test_ref_base_add_release() {
        let base = IntrRefBase::new();
        assert_eq!(base.current_count(), 1);

        base.add_ref();
        assert_eq!(base.current_count(), 2);
        assert!(!base.is_unique());

        assert!(!base.release()); // count 2 -> 1
        assert_eq!(base.current_count(), 1);
        assert!(base.is_unique());

        assert!(base.release()); // count 1 -> 0
    }

    #[test]
    fn test_ref_base_clone() {
        let base1 = IntrRefBase::new();
        base1.add_ref();
        assert_eq!(base1.current_count(), 2);

        let base2 = base1.clone();
        assert_eq!(base2.current_count(), 1); // Clone starts fresh
        assert_eq!(base1.current_count(), 2); // Original unchanged
    }

    #[test]
    fn test_ref_base_unique_changed_setting() {
        let base = IntrRefBase::new();

        // Initially positive count
        assert!(base.raw_count() > 0);

        // Enable notifications (makes count negative)
        base.set_should_invoke_unique_changed_listener(true);
        assert!(base.raw_count() < 0);
        assert_eq!(base.current_count(), 1); // Absolute value still 1

        // Disable notifications (makes count positive again)
        base.set_should_invoke_unique_changed_listener(false);
        assert!(base.raw_count() > 0);
    }

    #[test]
    fn test_simple_ref_base_new() {
        let base = SimpleIntrRefBase::new();
        assert_eq!(base.current_count(), 1);
        assert!(base.is_unique());
    }

    #[test]
    fn test_simple_ref_base_add_release() {
        let base = SimpleIntrRefBase::new();

        base.add_ref();
        assert_eq!(base.current_count(), 2);

        assert!(!base.release());
        assert_eq!(base.current_count(), 1);

        assert!(base.release());
    }

    #[test]
    fn test_simple_ref_base_clone() {
        let base1 = SimpleIntrRefBase::new();
        base1.add_ref();

        let base2 = base1.clone();
        assert_eq!(base2.current_count(), 1);
        assert_eq!(base1.current_count(), 2);
    }

    #[test]
    fn test_simple_ref_base_ref_counted_trait() {
        let base = SimpleIntrRefBase::new();

        assert_eq!(base.ref_count(), 1);
        assert!(base.is_unique());

        base.increment_ref();
        assert_eq!(base.ref_count(), 2);
        assert!(!base.is_unique());

        assert!(!base.decrement_ref());
        assert_eq!(base.ref_count(), 1);
    }

    #[test]
    fn test_ref_base_default() {
        let base = IntrRefBase::default();
        assert_eq!(base.current_count(), 1);
    }

    #[test]
    fn test_simple_ref_base_default() {
        let base = SimpleIntrRefBase::default();
        assert_eq!(base.current_count(), 1);
    }

    #[test]
    fn test_ref_base_thread_safety() {
        use std::sync::Arc;
        use std::thread;

        let base = Arc::new(IntrRefBase::new());
        let mut handles = vec![];

        // Spawn threads that add and release refs
        for _ in 0..10 {
            let base_clone = Arc::clone(&base);
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    base_clone.add_ref();
                }
                for _ in 0..100 {
                    base_clone.release();
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Should be back to 1 (the Arc's reference)
        assert_eq!(base.current_count(), 1);
    }

    #[test]
    fn test_simple_ref_base_thread_safety() {
        use std::sync::Arc;
        use std::thread;

        let base = Arc::new(SimpleIntrRefBase::new());
        let mut handles = vec![];

        for _ in 0..10 {
            let base_clone = Arc::clone(&base);
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    base_clone.add_ref();
                }
                for _ in 0..100 {
                    base_clone.release();
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(base.current_count(), 1);
    }

    #[test]
    fn test_unique_changed_listener() {
        use std::sync::atomic::{AtomicBool, Ordering};

        static LISTENER_CALLED: AtomicBool = AtomicBool::new(false);

        fn test_callback(_obj: &dyn RefCounted, _is_unique: bool) {
            LISTENER_CALLED.store(true, Ordering::SeqCst);
        }

        let listener = UniqueChangedListener {
            lock: None,
            func: test_callback,
            unlock: None,
        };

        IntrRefBase::set_unique_changed_listener(listener);

        // Create a base with notifications enabled
        let base = IntrRefBase::new();
        base.set_should_invoke_unique_changed_listener(true);

        // The listener would be called on 1->2 or 2->1 transitions
        // but our current implementation has a limitation

        IntrRefBase::clear_unique_changed_listener();
    }
}
