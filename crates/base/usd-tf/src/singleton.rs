//! Singleton pattern utilities.
//!
//! This module provides utilities for implementing the singleton pattern
//! in Rust. In Rust, the preferred way to create singletons is using
//! `OnceLock<T>` for lazy initialization.
//!
//! # Examples
//!
//! ```
//! use usd_tf::singleton::Singleton;
//! use std::sync::OnceLock;
//!
//! struct MyRegistry {
//!     data: Vec<String>,
//! }
//!
//! impl MyRegistry {
//!     fn new() -> Self {
//!         Self { data: Vec::new() }
//!     }
//! }
//!
//! impl Singleton for MyRegistry {
//!     fn instance() -> &'static Self {
//!         static INSTANCE: OnceLock<MyRegistry> = OnceLock::new();
//!         INSTANCE.get_or_init(MyRegistry::new)
//!     }
//! }
//!
//! // Access the singleton
//! let registry = MyRegistry::instance();
//! ```
//!
//! # Thread Safety
//!
//! All singleton instances are thread-safe. The `OnceLock` ensures that
//! initialization only happens once, even when multiple threads attempt
//! to access the singleton simultaneously.

use std::sync::OnceLock;

/// A trait for types that should have a single global instance.
///
/// Implementing this trait standardizes how singletons are accessed
/// throughout the codebase.
///
/// # Examples
///
/// ```
/// use usd_tf::singleton::Singleton;
/// use std::sync::OnceLock;
///
/// struct Config {
///     value: i32,
/// }
///
/// impl Singleton for Config {
///     fn instance() -> &'static Self {
///         static INSTANCE: OnceLock<Config> = OnceLock::new();
///         INSTANCE.get_or_init(|| Config { value: 42 })
///     }
/// }
///
/// assert_eq!(Config::instance().value, 42);
/// ```
pub trait Singleton: Sized {
    /// Returns a reference to the singleton instance.
    ///
    /// This method is guaranteed to return the same instance on every call.
    /// The instance is created lazily on first access.
    fn instance() -> &'static Self;
}

/// Check if a singleton of type T currently exists.
///
/// This is useful when you want to check if a singleton has been
/// initialized without actually creating it.
///
/// # Examples
///
/// ```
/// use usd_tf::singleton::{Singleton, singleton_exists};
/// use std::sync::OnceLock;
///
/// struct LazyRegistry;
///
/// static LAZY_REGISTRY: OnceLock<LazyRegistry> = OnceLock::new();
///
/// impl Singleton for LazyRegistry {
///     fn instance() -> &'static Self {
///         LAZY_REGISTRY.get_or_init(LazyRegistry::new)
///     }
/// }
///
/// impl LazyRegistry {
///     fn new() -> Self { LazyRegistry }
/// }
///
/// // Not yet initialized
/// assert!(!singleton_exists(&LAZY_REGISTRY));
///
/// // Force initialization
/// let _ = LazyRegistry::instance();
///
/// // Now it exists
/// assert!(singleton_exists(&LAZY_REGISTRY));
/// ```
#[must_use]
pub fn singleton_exists<T>(lock: &OnceLock<T>) -> bool {
    lock.get().is_some()
}

/// A helper macro to define a singleton type.
///
/// This macro generates the boilerplate code for implementing the
/// `Singleton` trait.
///
/// # Examples
///
/// ```
/// use usd_tf::singleton::Singleton;
/// use usd_tf::define_singleton;
///
/// struct MyService {
///     name: String,
/// }
///
/// impl MyService {
///     fn new() -> Self {
///         Self { name: "default".to_string() }
///     }
/// }
///
/// define_singleton!(MyService);
///
/// // Access the singleton
/// let service = MyService::instance();
/// assert_eq!(service.name, "default");
/// ```
#[macro_export]
macro_rules! define_singleton {
    ($type:ty) => {
        impl $crate::singleton::Singleton for $type {
            fn instance() -> &'static Self {
                static INSTANCE: std::sync::OnceLock<$type> = std::sync::OnceLock::new();
                INSTANCE.get_or_init(<$type>::new)
            }
        }
    };
    ($type:ty, $init:expr) => {
        impl $crate::singleton::Singleton for $type {
            fn instance() -> &'static Self {
                static INSTANCE: std::sync::OnceLock<$type> = std::sync::OnceLock::new();
                INSTANCE.get_or_init(|| $init)
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestSingleton {
        value: i32,
    }

    impl TestSingleton {
        fn new() -> Self {
            Self { value: 42 }
        }
    }

    impl Singleton for TestSingleton {
        fn instance() -> &'static Self {
            static INSTANCE: OnceLock<TestSingleton> = OnceLock::new();
            INSTANCE.get_or_init(TestSingleton::new)
        }
    }

    #[test]
    fn test_singleton_instance() {
        let s1 = TestSingleton::instance();
        let s2 = TestSingleton::instance();

        // Same instance
        assert!(std::ptr::eq(s1, s2));
        assert_eq!(s1.value, 42);
    }

    #[test]
    fn test_singleton_exists() {
        static TEST_LOCK: OnceLock<i32> = OnceLock::new();

        assert!(!singleton_exists(&TEST_LOCK));

        TEST_LOCK.get_or_init(|| 42);

        assert!(singleton_exists(&TEST_LOCK));
    }

    #[test]
    fn test_singleton_thread_safety() {
        use std::thread;

        struct Counter {
            count: std::sync::atomic::AtomicUsize,
        }

        impl Counter {
            fn new() -> Self {
                Self {
                    count: std::sync::atomic::AtomicUsize::new(0),
                }
            }

            fn increment(&self) {
                self.count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }

            fn get(&self) -> usize {
                self.count.load(std::sync::atomic::Ordering::SeqCst)
            }
        }

        impl Singleton for Counter {
            fn instance() -> &'static Self {
                static INSTANCE: OnceLock<Counter> = OnceLock::new();
                INSTANCE.get_or_init(Counter::new)
            }
        }

        let handles: Vec<_> = (0..10)
            .map(|_| {
                thread::spawn(|| {
                    Counter::instance().increment();
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("thread panicked");
        }

        assert_eq!(Counter::instance().get(), 10);
    }

    // Test the define_singleton! macro
    struct MacroTestSingleton {
        name: String,
    }

    impl MacroTestSingleton {
        fn new() -> Self {
            Self {
                name: "macro_test".to_string(),
            }
        }
    }

    define_singleton!(MacroTestSingleton);

    #[test]
    fn test_define_singleton_macro() {
        let s = MacroTestSingleton::instance();
        assert_eq!(s.name, "macro_test");

        // Same instance
        let s2 = MacroTestSingleton::instance();
        assert!(std::ptr::eq(s, s2));
    }

    // Test the define_singleton! macro with custom initializer
    struct CustomInitSingleton {
        value: i32,
    }

    define_singleton!(CustomInitSingleton, CustomInitSingleton { value: 100 });

    #[test]
    fn test_define_singleton_macro_custom_init() {
        let s = CustomInitSingleton::instance();
        assert_eq!(s.value, 100);
    }
}
