//! Expiry notification system for weak pointers.
//!
//! Provides a callback mechanism that is invoked when objects tracked by
//! weak pointers are destroyed. This is primarily used by the scripting
//! system to clean up resources associated with expired objects.
//!
//! # Overview
//!
//! Objects can request extra notification when they expire by registering
//! with the expiry notifier. When the object is destroyed, the registered
//! callback function is invoked with the object's unique identifier.
//!
//! # Examples
//!
//! ```
//! use usd_tf::expiry_notifier::ExpiryNotifier;
//!
//! // Set a notifier callback
//! ExpiryNotifier::set_notifier(|ptr| {
//!     println!("Object at {:?} expired", ptr);
//! });
//!
//! // Invoke the notifier (normally done automatically when objects expire)
//! ExpiryNotifier::invoke(std::ptr::null());
//!
//! // Clear the notifier
//! ExpiryNotifier::clear_notifier();
//! ```

use std::sync::{Mutex, OnceLock};

/// Type alias for the notifier callback function.
pub type NotifierFn = fn(*const std::ffi::c_void);

/// Global storage for the primary notifier function.
static NOTIFIER: OnceLock<Mutex<Option<NotifierFn>>> = OnceLock::new();

/// Global storage for the secondary notifier function.
static NOTIFIER2: OnceLock<Mutex<Option<NotifierFn>>> = OnceLock::new();

fn get_notifier() -> &'static Mutex<Option<NotifierFn>> {
    NOTIFIER.get_or_init(|| Mutex::new(None))
}

fn get_notifier2() -> &'static Mutex<Option<NotifierFn>> {
    NOTIFIER2.get_or_init(|| Mutex::new(None))
}

/// Expiry notifier for weak pointer cleanup.
///
/// This class provides a mechanism for receiving notifications when objects
/// tracked by weak pointers are destroyed. This is useful for cleanup of
/// associated resources (e.g., scripting bindings).
///
/// # Thread Safety
///
/// The notifier functions are stored behind a mutex and can be safely
/// set and invoked from multiple threads.
pub struct ExpiryNotifier;

impl ExpiryNotifier {
    /// Invoke the primary notifier function.
    ///
    /// If a notifier function has been set via [`set_notifier`], it will be
    /// called with the provided pointer. If no notifier is set, this is a no-op.
    ///
    /// # Parameters
    ///
    /// - `ptr`: The unique identifier of the expired object (typically the
    ///   address returned by `get_unique_identifier()`)
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::expiry_notifier::ExpiryNotifier;
    /// use std::sync::atomic::{AtomicUsize, Ordering};
    ///
    /// static CALL_COUNT: AtomicUsize = AtomicUsize::new(0);
    ///
    /// ExpiryNotifier::set_notifier(|_| {
    ///     CALL_COUNT.fetch_add(1, Ordering::Relaxed);
    /// });
    ///
    /// ExpiryNotifier::invoke(std::ptr::null());
    /// assert_eq!(CALL_COUNT.load(Ordering::Relaxed), 1);
    ///
    /// ExpiryNotifier::clear_notifier();
    /// ```
    pub fn invoke(ptr: *const std::ffi::c_void) {
        if let Ok(guard) = get_notifier().lock() {
            if let Some(func) = *guard {
                func(ptr);
            }
        }
    }

    /// Set the primary notifier function.
    ///
    /// This function sets the callback that will be invoked when objects
    /// expire. Only one notifier can be set at a time.
    ///
    /// # Parameters
    ///
    /// - `func`: The function to call when objects expire
    ///
    /// # Panics
    ///
    /// In the original C++ implementation, setting a new non-null notifier
    /// when one is already set causes a fatal error. This implementation
    /// matches that behavior by logging an error and returning without setting.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::expiry_notifier::ExpiryNotifier;
    ///
    /// ExpiryNotifier::set_notifier(|ptr| {
    ///     println!("Object expired: {:?}", ptr);
    /// });
    ///
    /// ExpiryNotifier::clear_notifier();
    /// ```
    pub fn set_notifier(func: NotifierFn) {
        if let Ok(mut guard) = get_notifier().lock() {
            if guard.is_some() {
                eprintln!("ExpiryNotifier: cannot override already installed notifier");
                return;
            }
            *guard = Some(func);
        }
    }

    /// Clear the primary notifier function.
    ///
    /// After calling this, [`invoke`] will be a no-op until a new notifier is set.
    pub fn clear_notifier() {
        if let Ok(mut guard) = get_notifier().lock() {
            *guard = None;
        }
    }

    /// Check if a primary notifier is currently set.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::expiry_notifier::ExpiryNotifier;
    ///
    /// assert!(!ExpiryNotifier::has_notifier());
    ///
    /// ExpiryNotifier::set_notifier(|_| {});
    /// assert!(ExpiryNotifier::has_notifier());
    ///
    /// ExpiryNotifier::clear_notifier();
    /// assert!(!ExpiryNotifier::has_notifier());
    /// ```
    pub fn has_notifier() -> bool {
        get_notifier().lock().map(|g| g.is_some()).unwrap_or(false)
    }

    /// Invoke the secondary notifier function.
    ///
    /// This is a separate notification channel that can be used independently
    /// of the primary notifier.
    pub fn invoke2(ptr: *const std::ffi::c_void) {
        if let Ok(guard) = get_notifier2().lock() {
            if let Some(func) = *guard {
                func(ptr);
            }
        }
    }

    /// Set the secondary notifier function.
    ///
    /// # Parameters
    ///
    /// - `func`: The function to call when objects expire (secondary channel)
    pub fn set_notifier2(func: NotifierFn) {
        if let Ok(mut guard) = get_notifier2().lock() {
            *guard = Some(func);
        }
    }

    /// Clear the secondary notifier function.
    pub fn clear_notifier2() {
        if let Ok(mut guard) = get_notifier2().lock() {
            *guard = None;
        }
    }

    /// Check if a secondary notifier is currently set.
    pub fn has_notifier2() -> bool {
        get_notifier2().lock().map(|g| g.is_some()).unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // Global mutex to serialize tests that modify global notifier state.
    // This prevents race conditions when tests run in parallel.
    static TEST_MUTEX: Mutex<()> = Mutex::new(());

    // Reset notifiers between tests
    fn reset_notifiers() {
        ExpiryNotifier::clear_notifier();
        ExpiryNotifier::clear_notifier2();
    }

    #[test]
    fn test_invoke_without_notifier() {
        let _guard = TEST_MUTEX.lock().unwrap();
        reset_notifiers();

        // Should not panic when no notifier is set
        ExpiryNotifier::invoke(std::ptr::null());
        ExpiryNotifier::invoke2(std::ptr::null());
    }

    #[test]
    fn test_set_and_invoke_notifier() {
        let _guard = TEST_MUTEX.lock().unwrap();
        reset_notifiers();

        static CALL_COUNT: AtomicUsize = AtomicUsize::new(0);
        CALL_COUNT.store(0, Ordering::SeqCst);

        ExpiryNotifier::set_notifier(|_| {
            CALL_COUNT.fetch_add(1, Ordering::SeqCst);
        });

        assert!(ExpiryNotifier::has_notifier());

        ExpiryNotifier::invoke(std::ptr::null());
        assert_eq!(CALL_COUNT.load(Ordering::SeqCst), 1);

        ExpiryNotifier::invoke(std::ptr::null());
        assert_eq!(CALL_COUNT.load(Ordering::SeqCst), 2);

        reset_notifiers();
    }

    #[test]
    fn test_clear_notifier() {
        let _guard = TEST_MUTEX.lock().unwrap();
        reset_notifiers();

        static CALL_COUNT: AtomicUsize = AtomicUsize::new(0);
        CALL_COUNT.store(0, Ordering::SeqCst);

        ExpiryNotifier::set_notifier(|_| {
            CALL_COUNT.fetch_add(1, Ordering::SeqCst);
        });

        ExpiryNotifier::invoke(std::ptr::null());
        assert_eq!(CALL_COUNT.load(Ordering::SeqCst), 1);

        ExpiryNotifier::clear_notifier();
        assert!(!ExpiryNotifier::has_notifier());

        ExpiryNotifier::invoke(std::ptr::null());
        assert_eq!(CALL_COUNT.load(Ordering::SeqCst), 1); // No increase

        reset_notifiers();
    }

    #[test]
    fn test_notifier2() {
        let _guard = TEST_MUTEX.lock().unwrap();
        reset_notifiers();

        static CALL_COUNT: AtomicUsize = AtomicUsize::new(0);
        CALL_COUNT.store(0, Ordering::SeqCst);

        ExpiryNotifier::set_notifier2(|_| {
            CALL_COUNT.fetch_add(10, Ordering::SeqCst);
        });

        assert!(ExpiryNotifier::has_notifier2());

        ExpiryNotifier::invoke2(std::ptr::null());
        assert_eq!(CALL_COUNT.load(Ordering::SeqCst), 10);

        ExpiryNotifier::clear_notifier2();
        assert!(!ExpiryNotifier::has_notifier2());

        reset_notifiers();
    }

    #[test]
    fn test_both_notifiers_independent() {
        let _guard = TEST_MUTEX.lock().unwrap();
        reset_notifiers();

        static COUNT1: AtomicUsize = AtomicUsize::new(0);
        static COUNT2: AtomicUsize = AtomicUsize::new(0);
        COUNT1.store(0, Ordering::SeqCst);
        COUNT2.store(0, Ordering::SeqCst);

        ExpiryNotifier::set_notifier(|_| {
            COUNT1.fetch_add(1, Ordering::SeqCst);
        });

        ExpiryNotifier::set_notifier2(|_| {
            COUNT2.fetch_add(1, Ordering::SeqCst);
        });

        ExpiryNotifier::invoke(std::ptr::null());
        assert_eq!(COUNT1.load(Ordering::SeqCst), 1);
        assert_eq!(COUNT2.load(Ordering::SeqCst), 0);

        ExpiryNotifier::invoke2(std::ptr::null());
        assert_eq!(COUNT1.load(Ordering::SeqCst), 1);
        assert_eq!(COUNT2.load(Ordering::SeqCst), 1);

        reset_notifiers();
    }

    #[test]
    fn test_invoke_with_pointer() {
        let _guard = TEST_MUTEX.lock().unwrap();
        reset_notifiers();

        static RECEIVED_PTR: AtomicUsize = AtomicUsize::new(0);

        ExpiryNotifier::set_notifier(|ptr| {
            RECEIVED_PTR.store(ptr as usize, Ordering::SeqCst);
        });

        let test_value = 42i32;
        let ptr = &test_value as *const i32 as *const std::ffi::c_void;

        ExpiryNotifier::invoke(ptr);
        assert_eq!(RECEIVED_PTR.load(Ordering::SeqCst), ptr as usize);

        reset_notifiers();
    }

    #[test]
    fn test_replace_notifier() {
        let _guard = TEST_MUTEX.lock().unwrap();
        reset_notifiers();

        static COUNT: AtomicUsize = AtomicUsize::new(0);
        COUNT.store(0, Ordering::SeqCst);

        ExpiryNotifier::set_notifier(|_| {
            COUNT.fetch_add(1, Ordering::SeqCst);
        });

        ExpiryNotifier::invoke(std::ptr::null());
        assert_eq!(COUNT.load(Ordering::SeqCst), 1);

        // Clear before setting a second notifier (double-set is now an error)
        ExpiryNotifier::clear_notifier();
        ExpiryNotifier::set_notifier(|_| {
            COUNT.fetch_add(10, Ordering::SeqCst);
        });

        ExpiryNotifier::invoke(std::ptr::null());
        assert_eq!(COUNT.load(Ordering::SeqCst), 11);

        reset_notifiers();
    }

    #[test]
    fn test_has_notifier_initially_false() {
        let _guard = TEST_MUTEX.lock().unwrap();
        // Note: With TEST_MUTEX, tests are now properly isolated
        let had_notifier = ExpiryNotifier::has_notifier();
        let _had_notifier2 = ExpiryNotifier::has_notifier2();

        reset_notifiers();

        // After reset, should be false
        assert!(!ExpiryNotifier::has_notifier());
        assert!(!ExpiryNotifier::has_notifier2());

        // Restore original state if needed
        if had_notifier {
            // Can't restore, but tests should be isolated anyway
        }
    }
}
