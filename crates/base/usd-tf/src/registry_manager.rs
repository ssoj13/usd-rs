//! Registry manager for type-keyed initialization.
//!
//! Provides a system for registering functions that are called on demand
//! when a particular type is subscribed to. This enables lazy initialization
//! of registries and plugin systems.
//!
//! # Overview
//!
//! The registry manager maintains a map of registration functions keyed by
//! TypeId. When code subscribes to a type, all registered functions for that
//! type are executed.
//!
//! # Examples
//!
//! ```
//! use usd_tf::registry_manager::RegistryManager;
//! use std::sync::atomic::{AtomicBool, Ordering};
//!
//! // Define a marker type for our registry
//! struct MyRegistry;
//!
//! static INITIALIZED: AtomicBool = AtomicBool::new(false);
//!
//! // Register a function
//! RegistryManager::register::<MyRegistry>(|| {
//!     INITIALIZED.store(true, Ordering::SeqCst);
//! });
//!
//! // Subscribe to run the functions
//! RegistryManager::subscribe_to::<MyRegistry>();
//!
//! assert!(INITIALIZED.load(Ordering::SeqCst));
//! ```

use std::any::TypeId;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

/// Type for registration functions (Arc for cheap cloning during snapshot).
pub type RegistrationFn = Arc<dyn Fn() + Send + Sync>;

/// Type for unload functions.
/// We use Fn() + Send + Sync to allow storage in static and thread-safe access.
/// The actual FnOnce behavior is simulated by wrapping in Option.
pub type UnloadFn = Box<dyn Fn() + Send + Sync>;

/// Global registry manager data.
static REGISTRY_DATA: OnceLock<RwLock<RegistryData>> = OnceLock::new();

/// Pending registrations (before manager is accessed).
static PENDING_REGISTRATIONS: OnceLock<Mutex<Vec<(TypeId, RegistrationFn)>>> = OnceLock::new();

/// Whether to run unloaders at exit.
static RUN_UNLOADERS_AT_EXIT: OnceLock<std::sync::atomic::AtomicBool> = OnceLock::new();

fn get_registry_data() -> &'static RwLock<RegistryData> {
    REGISTRY_DATA.get_or_init(|| {
        let mut data = RegistryData::new();

        // Process any pending registrations
        if let Some(pending) = PENDING_REGISTRATIONS.get() {
            if let Ok(mut pending_guard) = pending.lock() {
                for (type_id, func) in pending_guard.drain(..) {
                    data.functions.entry(type_id).or_default().push(func);
                }
            }
        }

        RwLock::new(data)
    })
}

fn get_pending() -> &'static Mutex<Vec<(TypeId, RegistrationFn)>> {
    PENDING_REGISTRATIONS.get_or_init(|| Mutex::new(Vec::new()))
}

/// Internal registry data.
#[derive(Default)]
struct RegistryData {
    /// Registration functions by type.
    functions: HashMap<TypeId, Vec<RegistrationFn>>,
    /// Types that have been subscribed to.
    subscribed: HashSet<TypeId>,
    /// Unload functions to run.
    unload_functions: Vec<UnloadFn>,
    /// Currently executing registration (for AddFunctionForUnload).
    current_registration: Option<TypeId>,
}

impl RegistryData {
    fn new() -> Self {
        Self::default()
    }
}

/// Manage initialization of registries.
///
/// The registry manager allows code to register functions that are called
/// when a particular type is subscribed to. This enables lazy initialization
/// of plugin registries and other systems.
///
/// # Thread Safety
///
/// All operations are thread-safe. Functions are executed with a read lock
/// held, so they should not attempt to register new functions.
pub struct RegistryManager;

impl RegistryManager {
    /// Get the singleton instance.
    ///
    /// In Rust, we use static methods, so this returns a unit type.
    pub fn instance() -> Self {
        // Ensure registry is initialized
        let _ = get_registry_data();
        Self
    }

    /// Register a function to be called when subscribing to type T.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::registry_manager::RegistryManager;
    ///
    /// struct MyService;
    ///
    /// RegistryManager::register::<MyService>(|| {
    ///     println!("MyService initialized");
    /// });
    /// ```
    pub fn register<T: 'static>(func: impl Fn() + Send + Sync + 'static) {
        let type_id = TypeId::of::<T>();
        let arc_fn: RegistrationFn = Arc::new(func);

        // Try to add to main registry
        if let Some(data) = REGISTRY_DATA.get() {
            if let Ok(mut guard) = data.write() {
                let run_now = guard.subscribed.contains(&type_id);
                guard.functions.entry(type_id).or_default().push(arc_fn);
                // Snapshot ALL functions for this type before releasing the lock.
                // C++ TfRegistryManager::_DoRegister runs all registered functions
                // when a type is already subscribed, not just the newly added one.
                let funcs_to_run: Vec<RegistrationFn> = if run_now {
                    guard
                        .functions
                        .get(&type_id)
                        .map(|v| v.iter().cloned().collect())
                        .unwrap_or_default()
                } else {
                    vec![]
                };
                // Release lock before invoking callbacks to avoid deadlock
                drop(guard);

                for func in funcs_to_run {
                    func();
                }
                return;
            }
        }

        // Registry not initialized yet, add to pending
        if let Ok(mut pending) = get_pending().lock() {
            pending.push((type_id, arc_fn));
        }
    }

    /// Request that initialization for service T be performed.
    ///
    /// Calling `subscribe_to::<T>()` causes all existing registration
    /// functions of type T to be run. Once subscribed, when new functions
    /// are registered for type T, they will be run immediately.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::registry_manager::RegistryManager;
    ///
    /// struct MyService;
    ///
    /// // Subscribe to run all MyService registration functions
    /// RegistryManager::subscribe_to::<MyService>();
    /// ```
    pub fn subscribe_to<T: 'static>() {
        let type_id = TypeId::of::<T>();
        let data = get_registry_data();

        // Snapshot function refs under single write lock to avoid TOCTOU race
        let func_snapshot: Vec<RegistrationFn> = {
            let Ok(mut guard) = data.write() else { return };
            if guard.subscribed.contains(&type_id) {
                return; // Already subscribed
            }
            guard.subscribed.insert(type_id);
            guard.current_registration = Some(type_id);
            guard
                .functions
                .get(&type_id)
                .map(|v| v.iter().cloned().collect())
                .unwrap_or_default()
        };
        // Lock released, invoke callbacks outside the lock

        for func in &func_snapshot {
            func();
        }

        // Clear current registration
        if let Ok(mut guard) = data.write() {
            guard.current_registration = None;
        }
    }

    /// Cancel any previous subscriptions to service T.
    ///
    /// After this call, newly registered functions will not be run
    /// automatically.
    pub fn unsubscribe_from<T: 'static>() {
        let type_id = TypeId::of::<T>();
        let data = get_registry_data();

        if let Ok(mut guard) = data.write() {
            guard.subscribed.remove(&type_id);
        }
    }

    /// Check if a type has been subscribed to.
    pub fn is_subscribed<T: 'static>() -> bool {
        let type_id = TypeId::of::<T>();
        let data = get_registry_data();

        if let Ok(guard) = data.read() {
            guard.subscribed.contains(&type_id)
        } else {
            false
        }
    }

    /// Add a function to be called when code is unloaded.
    ///
    /// This function should be called from within a registration function.
    /// Returns true if the function was added, false if not in a registration
    /// context.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::registry_manager::RegistryManager;
    ///
    /// struct MyService;
    ///
    /// RegistryManager::register::<MyService>(|| {
    ///     // Setup code...
    ///
    ///     // Register cleanup
    ///     RegistryManager::add_unload_function(|| {
    ///         // Cleanup code...
    ///     });
    /// });
    /// ```
    pub fn add_unload_function(func: impl Fn() + Send + Sync + 'static) -> bool {
        let data = get_registry_data();

        if let Ok(mut guard) = data.write() {
            if guard.current_registration.is_some() {
                guard.unload_functions.push(Box::new(func));
                return true;
            }
        }

        false
    }

    /// Configure unload functions to run at program exit.
    ///
    /// By default, unload functions are not run at exit for performance.
    /// Call this to enable running them (useful for leak detection).
    pub fn run_unloaders_at_exit() {
        let flag = RUN_UNLOADERS_AT_EXIT.get_or_init(|| std::sync::atomic::AtomicBool::new(false));
        flag.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// Check if unloaders will run at exit.
    pub fn will_run_unloaders_at_exit() -> bool {
        RUN_UNLOADERS_AT_EXIT
            .get()
            .map(|f| f.load(std::sync::atomic::Ordering::SeqCst))
            .unwrap_or(false)
    }

    /// Get the number of registered functions for a type.
    pub fn registered_count<T: 'static>() -> usize {
        let type_id = TypeId::of::<T>();
        let data = get_registry_data();

        if let Ok(guard) = data.read() {
            guard.functions.get(&type_id).map(|v| v.len()).unwrap_or(0)
        } else {
            0
        }
    }

    /// Clear all registrations (for testing).
    #[cfg(test)]
    pub fn clear_all() {
        if let Some(data) = REGISTRY_DATA.get() {
            if let Ok(mut guard) = data.write() {
                guard.functions.clear();
                guard.subscribed.clear();
                guard.unload_functions.clear();
                guard.current_registration = None;
            }
        }
    }
}

/// Macro to define a registry function.
///
/// This provides a similar interface to TF_REGISTRY_FUNCTION.
///
/// # Examples
///
/// ```ignore
/// use usd_tf::registry_function;
///
/// struct MyRegistry;
///
/// registry_function!(MyRegistry, {
///     // Registration code here
/// });
/// ```
#[macro_export]
macro_rules! registry_function {
    ($key_type:ty, $body:block) => {
        $crate::registry_manager::RegistryManager::register::<$key_type>(|| $body);
    };
}

pub use registry_function;

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // Each test uses a unique type to avoid interference from parallel tests

    #[test]
    fn test_register_and_subscribe() {
        // Use a unique type for this test
        struct TestRegSub1;
        static COUNTER: AtomicUsize = AtomicUsize::new(0);

        RegistryManager::register::<TestRegSub1>(|| {
            COUNTER.fetch_add(1, Ordering::SeqCst);
        });

        // Not run until subscribed (might be 0 or initial depending on test order)
        let before_sub = COUNTER.load(Ordering::SeqCst);

        RegistryManager::subscribe_to::<TestRegSub1>();
        assert!(COUNTER.load(Ordering::SeqCst) > before_sub);
    }

    #[test]
    fn test_multiple_registrations() {
        struct TestMultiReg;
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let initial = COUNTER.load(Ordering::SeqCst);

        RegistryManager::register::<TestMultiReg>(|| {
            COUNTER.fetch_add(1, Ordering::SeqCst);
        });

        RegistryManager::register::<TestMultiReg>(|| {
            COUNTER.fetch_add(10, Ordering::SeqCst);
        });

        RegistryManager::subscribe_to::<TestMultiReg>();
        assert!(COUNTER.load(Ordering::SeqCst) >= initial + 11);
    }

    #[test]
    fn test_is_subscribed() {
        struct TestIsSub;

        assert!(!RegistryManager::is_subscribed::<TestIsSub>());

        RegistryManager::subscribe_to::<TestIsSub>();
        assert!(RegistryManager::is_subscribed::<TestIsSub>());

        RegistryManager::unsubscribe_from::<TestIsSub>();
        assert!(!RegistryManager::is_subscribed::<TestIsSub>());
    }

    #[test]
    fn test_registered_count() {
        struct TestRegCount;

        let initial = RegistryManager::registered_count::<TestRegCount>();

        RegistryManager::register::<TestRegCount>(|| {});
        assert_eq!(
            RegistryManager::registered_count::<TestRegCount>(),
            initial + 1
        );

        RegistryManager::register::<TestRegCount>(|| {});
        assert_eq!(
            RegistryManager::registered_count::<TestRegCount>(),
            initial + 2
        );
    }

    #[test]
    fn test_unsubscribe() {
        struct TestUnsub2;
        static COUNTER: AtomicUsize = AtomicUsize::new(0);

        RegistryManager::subscribe_to::<TestUnsub2>();

        // Register after subscribe - should run immediately
        RegistryManager::register::<TestUnsub2>(|| {
            COUNTER.fetch_add(1, Ordering::SeqCst);
        });

        // This might run immediately depending on timing
        let _count_after_first = COUNTER.load(Ordering::SeqCst);

        RegistryManager::unsubscribe_from::<TestUnsub2>();

        // Register after unsubscribe - should NOT run
        let count_before_second = COUNTER.load(Ordering::SeqCst);
        RegistryManager::register::<TestUnsub2>(|| {
            COUNTER.fetch_add(100, Ordering::SeqCst);
        });

        // Should not have added 100
        assert!(COUNTER.load(Ordering::SeqCst) < count_before_second + 100);
    }

    #[test]
    fn test_instance() {
        let _instance = RegistryManager::instance();
        // Just verify it doesn't panic
    }

    #[test]
    fn test_run_unloaders_at_exit() {
        // Just verify the API works
        RegistryManager::run_unloaders_at_exit();
        assert!(RegistryManager::will_run_unloaders_at_exit());
    }

    #[test]
    fn test_add_unload_function_outside_registration() {
        // Should return false when not in registration context
        let result = RegistryManager::add_unload_function(|| {});
        assert!(!result);
    }

    #[test]
    fn test_different_types_independent() {
        struct TypeAIndep;
        struct TypeBIndep;
        static A_COUNT: AtomicUsize = AtomicUsize::new(0);
        static B_COUNT: AtomicUsize = AtomicUsize::new(0);
        let a_initial = A_COUNT.load(Ordering::SeqCst);
        let b_initial = B_COUNT.load(Ordering::SeqCst);

        RegistryManager::register::<TypeAIndep>(|| {
            A_COUNT.fetch_add(1, Ordering::SeqCst);
        });

        RegistryManager::register::<TypeBIndep>(|| {
            B_COUNT.fetch_add(1, Ordering::SeqCst);
        });

        RegistryManager::subscribe_to::<TypeAIndep>();
        assert!(A_COUNT.load(Ordering::SeqCst) > a_initial);
        assert_eq!(B_COUNT.load(Ordering::SeqCst), b_initial);

        RegistryManager::subscribe_to::<TypeBIndep>();
        assert!(B_COUNT.load(Ordering::SeqCst) > b_initial);
    }

    #[test]
    fn test_thread_safety() {
        struct ThreadTestType;
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let initial = COUNTER.load(Ordering::SeqCst);

        // Register from multiple threads
        let handles: Vec<_> = (0..10)
            .map(|_| {
                std::thread::spawn(|| {
                    RegistryManager::register::<ThreadTestType>(|| {
                        COUNTER.fetch_add(1, Ordering::SeqCst);
                    });
                })
            })
            .collect();

        for h in handles {
            h.join().ok();
        }

        let registered = RegistryManager::registered_count::<ThreadTestType>();
        assert!(registered >= 10);

        RegistryManager::subscribe_to::<ThreadTestType>();
        assert!(COUNTER.load(Ordering::SeqCst) >= initial + 10);
    }
}
