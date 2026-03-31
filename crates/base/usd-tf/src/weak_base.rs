//! Weak pointer base class.
//!
//! Provides a base class that enables weak pointers via [`WeakPtr`].
//! Objects inheriting from `WeakBase` can be referenced by weak pointers
//! that don't prevent destruction.
//!
//! # Overview
//!
//! The weak pointer system uses a "remnant" object that persists after
//! the original object is destroyed. Weak pointers hold a reference to
//! the remnant, which tracks whether the original object is still alive.
//!
//! # Examples
//!
//! ```
//! use usd_tf::weak_base::{WeakBase, Remnant};
//! use std::sync::Arc;
//!
//! struct MyObject {
//!     weak_base: WeakBase,
//!     data: String,
//! }
//!
//! impl MyObject {
//!     fn new(data: String) -> Self {
//!         Self {
//!             weak_base: WeakBase::new(),
//!             data,
//!         }
//!     }
//!
//!     fn get_remnant(&self) -> Arc<Remnant> {
//!         self.weak_base.register()
//!     }
//! }
//! ```

use crate::expiry_notifier::ExpiryNotifier;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

/// A remnant is a persistent memory of an object's address.
///
/// When the original object is destroyed, the remnant marks itself as
/// "dead" but continues to exist as long as weak pointers reference it.
/// This allows weak pointers to safely detect when their target is gone.
///
/// # Thread Safety
///
/// All operations on Remnant are thread-safe.
#[derive(Debug)]
pub struct Remnant {
    /// Whether the original object is still alive.
    alive: AtomicBool,
    /// Whether to notify via primary expiry notifier.
    notify: AtomicBool,
    /// Whether to notify via secondary expiry notifier.
    notify2: AtomicBool,
    /// Unique identifier (typically the original object's address).
    unique_id: AtomicPtr<std::ffi::c_void>,
}

impl Default for Remnant {
    fn default() -> Self {
        Self::new()
    }
}

impl Remnant {
    /// Create a new remnant.
    pub fn new() -> Self {
        Self {
            alive: AtomicBool::new(true),
            notify: AtomicBool::new(false),
            notify2: AtomicBool::new(false),
            unique_id: AtomicPtr::new(std::ptr::null_mut()),
        }
    }

    /// Create a remnant with a specific unique identifier.
    pub fn with_id(id: *const std::ffi::c_void) -> Self {
        Self {
            alive: AtomicBool::new(true),
            notify: AtomicBool::new(false),
            notify2: AtomicBool::new(false),
            unique_id: AtomicPtr::new(id as *mut _),
        }
    }

    /// Mark this remnant as dead (original object destroyed).
    ///
    /// This is called when the original object's destructor runs.
    pub fn forget(&self) {
        self.alive.store(false, Ordering::Release);

        // Invoke expiry notifiers if enabled
        if self.notify.load(Ordering::Acquire) {
            ExpiryNotifier::invoke(self.get_unique_identifier());
        }
        if self.notify2.load(Ordering::Acquire) {
            ExpiryNotifier::invoke2(self.get_unique_identifier());
        }
    }

    /// Check if the original object is still alive.
    ///
    /// Note: A `true` result may become stale immediately in a
    /// multi-threaded context. A `false` result is definitive.
    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Acquire)
    }

    /// Get the unique identifier for this remnant.
    ///
    /// This is typically the address of the original object.
    pub fn get_unique_identifier(&self) -> *const std::ffi::c_void {
        let id = self.unique_id.load(Ordering::Relaxed);
        if id.is_null() {
            // Default to self address if no ID was set
            self as *const Self as *const std::ffi::c_void
        } else {
            id
        }
    }

    /// Set the unique identifier.
    pub fn set_unique_identifier(&self, id: *const std::ffi::c_void) {
        self.unique_id.store(id as *mut _, Ordering::Relaxed);
    }

    /// Enable primary expiry notification.
    ///
    /// When enabled, the expiry notifier callback will be invoked
    /// when this remnant's original object is destroyed.
    pub fn enable_notification(&self) {
        self.notify.store(true, Ordering::Release);
    }

    /// Enable secondary expiry notification.
    pub fn enable_notification2(&self) {
        self.notify2.store(true, Ordering::Release);
    }

    /// Check if primary notification is enabled.
    pub fn has_notification(&self) -> bool {
        self.notify.load(Ordering::Acquire)
    }

    /// Check if secondary notification is enabled.
    pub fn has_notification2(&self) -> bool {
        self.notify2.load(Ordering::Acquire)
    }
}

/// Base class for objects that support weak pointers.
///
/// Include this as a field in any struct that needs to be referenced
/// by weak pointers.
///
/// # Thread Safety
///
/// Registration of remnants is thread-safe and uses lock-free atomics.
///
/// # Examples
///
/// ```
/// use usd_tf::weak_base::WeakBase;
/// use std::sync::Arc;
///
/// struct MyObject {
///     weak_base: WeakBase,
///     value: i32,
/// }
///
/// impl MyObject {
///     fn new(value: i32) -> Self {
///         Self {
///             weak_base: WeakBase::new(),
///             value,
///         }
///     }
/// }
///
/// impl Drop for MyObject {
///     fn drop(&mut self) {
///         // Mark any remnant as dead
///         self.weak_base.invalidate();
///     }
/// }
/// ```
#[derive(Debug)]
pub struct WeakBase {
    /// Pointer to the remnant (if registered).
    remnant_ptr: AtomicPtr<Remnant>,
}

impl Default for WeakBase {
    fn default() -> Self {
        Self::new()
    }
}

impl WeakBase {
    /// Create a new WeakBase with no remnant.
    pub fn new() -> Self {
        Self {
            remnant_ptr: AtomicPtr::new(std::ptr::null_mut()),
        }
    }

    /// Clones Arc from raw pointer, incrementing ref count.
    ///
    /// # Safety
    /// - ptr must be a valid Arc<Remnant> raw pointer
    /// - ptr must not be null
    #[allow(unsafe_code)]
    fn clone_arc_from_raw(ptr: *mut Remnant) -> Arc<Remnant> {
        unsafe {
            Arc::increment_strong_count(ptr);
            Arc::from_raw(ptr)
        }
    }

    /// Register this object and get a remnant.
    ///
    /// If a remnant already exists, returns a new Arc to it.
    /// Otherwise, creates a new remnant.
    ///
    /// # Thread Safety
    ///
    /// This method is thread-safe. Multiple threads calling register
    /// will all receive the same remnant.
    #[allow(unsafe_code)]
    pub fn register(&self) -> Arc<Remnant> {
        // Try to load existing remnant
        let existing = self.remnant_ptr.load(Ordering::Acquire);
        if !existing.is_null() {
            return Self::clone_arc_from_raw(existing);
        }

        // Create a new remnant
        let new_remnant = Arc::new(Remnant::new());
        let new_ptr = Arc::into_raw(new_remnant.clone()) as *mut Remnant;

        // Try to register it
        match self.remnant_ptr.compare_exchange(
            std::ptr::null_mut(),
            new_ptr,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => {
                // Successfully registered
                new_remnant
            }
            Err(existing) => {
                // Someone else registered first, drop our candidate
                drop(unsafe { Arc::from_raw(new_ptr) });
                // Return a reference to the existing one
                Self::clone_arc_from_raw(existing)
            }
        }
    }

    /// Register with a custom remnant.
    ///
    /// This allows derived classes to provide specialized remnants.
    #[allow(unsafe_code)]
    pub fn register_custom(&self, candidate: Arc<Remnant>) -> Arc<Remnant> {
        let new_ptr = Arc::into_raw(candidate.clone()) as *mut Remnant;

        match self.remnant_ptr.compare_exchange(
            std::ptr::null_mut(),
            new_ptr,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => candidate,
            Err(existing) => {
                // Drop the candidate
                drop(unsafe { Arc::from_raw(new_ptr) });
                // Return the existing one
                Self::clone_arc_from_raw(existing)
            }
        }
    }

    /// Check if a remnant has been registered.
    pub fn has_remnant(&self) -> bool {
        !self.remnant_ptr.load(Ordering::Relaxed).is_null()
    }

    /// Get the unique identifier for this object.
    ///
    /// If a remnant exists, returns its unique ID.
    /// Otherwise returns the address of this WeakBase.
    #[allow(unsafe_code)]
    pub fn get_unique_identifier(&self) -> *const std::ffi::c_void {
        let remnant = self.remnant_ptr.load(Ordering::Acquire);
        if !remnant.is_null() {
            unsafe { (*remnant).get_unique_identifier() }
        } else {
            self as *const Self as *const std::ffi::c_void
        }
    }

    /// Enable secondary notification on the remnant.
    ///
    /// This is used internally for Python binding support.
    pub fn enable_notification2(&self) {
        let remnant = self.register();
        remnant.enable_notification2();
    }

    /// Invalidate any existing remnant.
    ///
    /// This should be called from the destructor of objects
    /// containing WeakBase.
    #[allow(unsafe_code)]
    pub fn invalidate(&self) {
        let remnant = self.remnant_ptr.load(Ordering::Acquire);
        if !remnant.is_null() {
            unsafe {
                (*remnant).forget();
                // Drop our reference
                drop(Arc::from_raw(remnant));
            }
            self.remnant_ptr
                .store(std::ptr::null_mut(), Ordering::Release);
        }
    }
}

impl Clone for WeakBase {
    /// Cloning a WeakBase creates a new one without a remnant.
    ///
    /// A newly copied object doesn't inherit the original's weak pointers.
    fn clone(&self) -> Self {
        Self::new()
    }
}

impl Drop for WeakBase {
    fn drop(&mut self) {
        self.invalidate();
    }
}

/// Access helper for WeakBase internals.
///
/// This is used by WeakPtr and related types.
pub struct WeakBaseAccess;

impl WeakBaseAccess {
    /// Get the remnant from a WeakBase.
    pub fn get_remnant(wb: &WeakBase) -> Arc<Remnant> {
        wb.register()
    }
}

/// Trait for types that support weak pointers.
///
/// Types implementing this trait can be referenced by WeakPtr.
pub trait WeakPointable {
    /// Get the WeakBase for this object.
    fn weak_base(&self) -> &WeakBase;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remnant_new() {
        let remnant = Remnant::new();
        assert!(remnant.is_alive());
        assert!(!remnant.has_notification());
        assert!(!remnant.has_notification2());
    }

    #[test]
    fn test_remnant_forget() {
        let remnant = Remnant::new();
        assert!(remnant.is_alive());

        remnant.forget();
        assert!(!remnant.is_alive());
    }

    #[test]
    fn test_remnant_notification() {
        let remnant = Remnant::new();

        remnant.enable_notification();
        assert!(remnant.has_notification());

        remnant.enable_notification2();
        assert!(remnant.has_notification2());
    }

    #[test]
    fn test_remnant_unique_id() {
        let remnant = Remnant::new();

        // Default ID is self address
        let default_id = remnant.get_unique_identifier();
        assert!(!default_id.is_null());

        // Set custom ID
        let custom_id = 0x12345678 as *const std::ffi::c_void;
        remnant.set_unique_identifier(custom_id);
        assert_eq!(remnant.get_unique_identifier(), custom_id);
    }

    #[test]
    fn test_remnant_with_id() {
        let id = 0xDEADBEEF as *const std::ffi::c_void;
        let remnant = Remnant::with_id(id);
        assert_eq!(remnant.get_unique_identifier(), id);
    }

    #[test]
    fn test_weak_base_new() {
        let wb = WeakBase::new();
        assert!(!wb.has_remnant());
    }

    #[test]
    fn test_weak_base_register() {
        let wb = WeakBase::new();
        assert!(!wb.has_remnant());

        let remnant = wb.register();
        assert!(wb.has_remnant());
        assert!(remnant.is_alive());
    }

    #[test]
    fn test_weak_base_register_same_remnant() {
        let wb = WeakBase::new();

        let r1 = wb.register();
        let r2 = wb.register();

        // Both should point to the same remnant
        assert!(Arc::ptr_eq(&r1, &r2));
    }

    #[test]
    fn test_weak_base_invalidate() {
        let wb = WeakBase::new();
        let remnant = wb.register();
        assert!(remnant.is_alive());

        wb.invalidate();
        assert!(!remnant.is_alive());
        assert!(!wb.has_remnant());
    }

    #[test]
    fn test_weak_base_drop() {
        let remnant;
        {
            let wb = WeakBase::new();
            remnant = wb.register();
            assert!(remnant.is_alive());
        } // wb dropped here

        // Remnant should be marked as dead
        assert!(!remnant.is_alive());
    }

    #[test]
    fn test_weak_base_clone() {
        let wb1 = WeakBase::new();
        let _r1 = wb1.register();
        assert!(wb1.has_remnant());

        let wb2 = wb1.clone();
        assert!(!wb2.has_remnant()); // Clone doesn't inherit remnant
    }

    #[test]
    fn test_weak_base_unique_identifier() {
        let wb = WeakBase::new();

        // Without remnant, uses self address
        let id1 = wb.get_unique_identifier();
        assert!(!id1.is_null());

        // With remnant, uses remnant's ID
        let _r = wb.register();
        let id2 = wb.get_unique_identifier();
        assert!(!id2.is_null());
    }

    #[test]
    fn test_weak_base_enable_notification2() {
        let wb = WeakBase::new();
        wb.enable_notification2();

        let remnant = wb.register();
        assert!(remnant.has_notification2());
    }

    #[test]
    fn test_weak_base_register_custom() {
        let wb = WeakBase::new();

        let custom_id = 0x42424242 as *const std::ffi::c_void;
        let custom = Arc::new(Remnant::with_id(custom_id));

        let remnant = wb.register_custom(custom);
        assert_eq!(remnant.get_unique_identifier(), custom_id);
    }

    #[test]
    fn test_weak_base_thread_safety() {
        use std::thread;

        let wb = Arc::new(WeakBase::new());
        let mut handles = vec![];

        // Multiple threads registering simultaneously
        for _ in 0..10 {
            let wb_clone = Arc::clone(&wb);
            handles.push(thread::spawn(move || {
                let r = wb_clone.register();
                assert!(r.is_alive());
                r
            }));
        }

        let remnants: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        // All should be the same remnant
        for i in 1..remnants.len() {
            assert!(Arc::ptr_eq(&remnants[0], &remnants[i]));
        }
    }

    #[test]
    fn test_weak_base_access() {
        let wb = WeakBase::new();
        let remnant = WeakBaseAccess::get_remnant(&wb);
        assert!(remnant.is_alive());
    }

    struct TestObject {
        weak_base: WeakBase,
        _data: i32,
    }

    impl WeakPointable for TestObject {
        fn weak_base(&self) -> &WeakBase {
            &self.weak_base
        }
    }

    #[test]
    fn test_weak_pointable_trait() {
        let obj = TestObject {
            weak_base: WeakBase::new(),
            _data: 42,
        };

        let remnant = obj.weak_base().register();
        assert!(remnant.is_alive());
    }

    #[test]
    fn test_weak_base_default() {
        let wb = WeakBase::default();
        assert!(!wb.has_remnant());
    }

    #[test]
    fn test_remnant_default() {
        let r = Remnant::default();
        assert!(r.is_alive());
    }
}
