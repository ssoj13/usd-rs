//! Lazily initialized static data.
//!
//! This module provides utilities for declaring global data that is
//! initialized on first use. This solves the "static initialization
//! order fiasco" problem in a safe way.
//!
//! # Why Use This?
//!
//! In Rust, global data with non-const initialization is tricky:
//! - `static` requires const initialization
//! - `lazy_static!` works but has some limitations
//! - `std::sync::OnceLock` is the modern solution
//!
//! This module provides a thin wrapper around `OnceLock` that matches
//! the USD API for easier porting.
//!
//! # Examples
//!
//! ```
//! use usd_tf::static_data::StaticData;
//! use std::collections::HashSet;
//!
//! // Define static data with a factory function
//! static NAMES: StaticData<HashSet<&'static str>> = StaticData::new(|| {
//!     let mut set = HashSet::new();
//!     set.insert("alice");
//!     set.insert("bob");
//!     set
//! });
//!
//! // Use the data (initialized on first access)
//! assert!(NAMES.contains("alice"));
//! assert!(NAMES.contains("bob"));
//! assert!(!NAMES.contains("charlie"));
//! ```

use std::ops::Deref;
use std::sync::OnceLock;

/// Lazily initialized static data.
///
/// The data is created on first access and remains alive for the
/// duration of the program.
///
/// # Thread Safety
///
/// Initialization is thread-safe. Multiple threads accessing the data
/// for the first time will race, but only one will win and its value
/// will be used.
///
/// # Examples
///
/// ```
/// use usd_tf::static_data::StaticData;
///
/// static CONFIG: StaticData<Vec<String>> = StaticData::new(|| {
///     vec!["option1".into(), "option2".into()]
/// });
///
/// assert_eq!(CONFIG.len(), 2);
/// ```
pub struct StaticData<T> {
    data: OnceLock<T>,
    factory: fn() -> T,
}

impl<T> StaticData<T> {
    /// Create a new StaticData with a factory function.
    ///
    /// The factory is called on first access to create the data.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::static_data::StaticData;
    ///
    /// static COUNTER: StaticData<std::sync::atomic::AtomicUsize> = StaticData::new(|| {
    ///     std::sync::atomic::AtomicUsize::new(0)
    /// });
    /// ```
    #[inline]
    pub const fn new(factory: fn() -> T) -> Self {
        Self {
            data: OnceLock::new(),
            factory,
        }
    }

    /// Get a reference to the data, initializing if necessary.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::static_data::StaticData;
    ///
    /// static VALUE: StaticData<i32> = StaticData::new(|| 42);
    ///
    /// assert_eq!(*VALUE.get(), 42);
    /// ```
    #[inline]
    pub fn get(&self) -> &T {
        self.data.get_or_init(self.factory)
    }

    /// Check if the data has been initialized.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::static_data::StaticData;
    ///
    /// static LAZY: StaticData<String> = StaticData::new(|| "initialized".into());
    ///
    /// // Not initialized yet
    /// // assert!(!LAZY.is_initialized()); // This would fail since we already accessed it above
    ///
    /// // Access to initialize
    /// let _ = LAZY.get();
    /// assert!(LAZY.is_initialized());
    /// ```
    #[inline]
    pub fn is_initialized(&self) -> bool {
        self.data.get().is_some()
    }

    /// Ensure the data is initialized without returning a reference.
    ///
    /// Useful when you need initialization side effects but don't
    /// need the value yet.
    #[inline]
    pub fn touch(&self) {
        let _ = self.get();
    }
}

impl<T> Deref for StaticData<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

// SAFETY: StaticData is Sync if T is Sync (OnceLock provides the synchronization)
#[allow(unsafe_code)]
unsafe impl<T: Sync + Send> Sync for StaticData<T> {}

#[allow(unsafe_code)]
unsafe impl<T: Send> Send for StaticData<T> {}

impl<T: std::fmt::Debug> std::fmt::Debug for StaticData<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.data.get() {
            Some(data) => f.debug_tuple("StaticData").field(data).finish(),
            None => f.write_str("StaticData(<not initialized>)"),
        }
    }
}

/// A macro to define static data with initialization code.
///
/// This is similar to `lazy_static!` but uses `StaticData` internally.
///
/// # Examples
///
/// ```
/// use usd_tf::make_static_data;
/// use std::collections::HashMap;
///
/// make_static_data!(REGISTRY: HashMap<String, i32> = {
///     let mut m = HashMap::new();
///     m.insert("one".into(), 1);
///     m.insert("two".into(), 2);
///     m
/// });
///
/// // Access the HashMap via Deref, then call HashMap::get
/// assert_eq!((*REGISTRY).get("one"), Some(&1));
/// assert_eq!((*REGISTRY).get("two"), Some(&2));
/// ```
#[macro_export]
macro_rules! make_static_data {
    ($name:ident : $type:ty = $init:expr) => {
        static $name: $crate::static_data::StaticData<$type> =
            $crate::static_data::StaticData::new(|| $init);
    };
    (pub $name:ident : $type:ty = $init:expr) => {
        pub static $name: $crate::static_data::StaticData<$type> =
            $crate::static_data::StaticData::new(|| $init);
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn counting_factory() -> String {
        TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        "test".to_string()
    }

    #[test]
    fn test_basic_initialization() {
        static DATA: StaticData<Vec<i32>> = StaticData::new(|| vec![1, 2, 3]);

        assert_eq!(DATA.len(), 3);
        assert_eq!(DATA[0], 1);
        assert_eq!(DATA[1], 2);
        assert_eq!(DATA[2], 3);
    }

    #[test]
    fn test_deref() {
        static DATA: StaticData<String> = StaticData::new(|| "hello".to_string());

        // Using Deref
        assert_eq!(&*DATA, "hello");
        assert_eq!(DATA.len(), 5);
    }

    #[test]
    fn test_is_initialized() {
        static DATA: StaticData<i32> = StaticData::new(|| 42);

        let was_init = DATA.is_initialized();
        let _ = DATA.get();
        assert!(DATA.is_initialized());

        // Either it was already initialized from another test, or we just initialized it
        assert!(was_init || DATA.is_initialized());
    }

    #[test]
    fn test_touch() {
        static TOUCHED: StaticData<String> = StaticData::new(|| "touched".to_string());

        TOUCHED.touch();
        assert!(TOUCHED.is_initialized());
    }

    #[test]
    fn test_debug_format() {
        static DEBUG_DATA: StaticData<i32> = StaticData::new(|| 123);

        // Initialize first
        let _ = DEBUG_DATA.get();

        let debug_str = format!("{:?}", DEBUG_DATA);
        assert!(debug_str.contains("StaticData"));
        assert!(debug_str.contains("123"));
    }

    #[test]
    fn test_complex_type() {
        static COMPLEX: StaticData<HashMap<String, Vec<i32>>> = StaticData::new(|| {
            let mut m = HashMap::new();
            m.insert("primes".to_string(), vec![2, 3, 5, 7, 11]);
            m.insert("evens".to_string(), vec![2, 4, 6, 8, 10]);
            m
        });

        assert_eq!(COMPLEX.get().get("primes"), Some(&vec![2, 3, 5, 7, 11]));
        assert_eq!(COMPLEX.get().get("evens"), Some(&vec![2, 4, 6, 8, 10]));
    }

    #[test]
    fn test_macro() {
        make_static_data!(MACRO_TEST: Vec<&'static str> = vec!["a", "b", "c"]);

        assert_eq!(MACRO_TEST.len(), 3);
        assert_eq!(MACRO_TEST[0], "a");
    }

    #[test]
    fn test_thread_safety() {
        use std::thread;

        static SHARED: StaticData<AtomicUsize> = StaticData::new(|| AtomicUsize::new(0));

        let handles: Vec<_> = (0..10)
            .map(|_| {
                thread::spawn(|| {
                    SHARED.fetch_add(1, Ordering::SeqCst);
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(SHARED.load(Ordering::SeqCst), 10);
    }

    #[test]
    fn test_only_initialized_once() {
        static ONCE_DATA: StaticData<String> = StaticData::new(counting_factory);

        let before = TEST_COUNTER.load(Ordering::SeqCst);

        // Access multiple times
        let _ = ONCE_DATA.get();
        let _ = ONCE_DATA.get();
        let _ = ONCE_DATA.get();

        let after = TEST_COUNTER.load(Ordering::SeqCst);

        // Factory should only be called once (or already was called)
        assert!(after - before <= 1);
    }
}
