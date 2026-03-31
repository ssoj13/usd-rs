//! Thread-local scoped cache for resolver implementations.
//!
//! Port of pxr/usd/ar/threadLocalScopedCache.h
//!
//! Utility class for custom resolver implementations. This wraps up
//! a common pattern for implementing thread-local scoped caches for
//! `Resolver::begin_cache_scope` and `Resolver::end_cache_scope`.
//!
//! # Example
//!
//! ```rust,ignore
//! use usd_ar::ThreadLocalScopedCache;
//! use std::collections::HashMap;
//!
//! type ResolveCache = HashMap<String, String>;
//!
//! struct MyResolver {
//!     cache: ThreadLocalScopedCache<ResolveCache>,
//! }
//!
//! impl MyResolver {
//!     fn begin_cache_scope(&self) -> Option<usd_vt::Value> {
//!         self.cache.begin_cache_scope(None)
//!     }
//!
//!     fn end_cache_scope(&self, data: Option<usd_vt::Value>) {
//!         self.cache.end_cache_scope();
//!     }
//!
//!     fn resolve(&self, path: &str) -> String {
//!         if let Some(cache) = self.cache.get_current_cache() {
//!             let cache = cache.read();
//!             if let Some(resolved) = cache.get(path) {
//!                 return resolved.clone();
//!             }
//!         }
//!         // ... resolve without cache ...
//!         String::new()
//!     }
//! }
//! ```

use std::cell::RefCell;
use std::sync::{Arc, RwLock};

/// A shared pointer to a cached value.
pub type CachePtr<T> = Arc<RwLock<T>>;

/// Thread-local scoped cache utility for resolver implementations.
///
/// Manages a thread-local stack of cache scopes. When a cache scope is
/// opened, a new or shared cache is pushed onto the stack. When the scope
/// is closed, the stack is popped. Nested scopes share the same cache
/// by default, unless explicit cache data is provided.
///
/// This mirrors C++ `ArThreadLocalScopedCache<CachedType>`.
pub struct ThreadLocalScopedCache<T: Default + Send + Sync + 'static> {
    /// Thread-local stack of cache pointers (field reserved for future
    /// per-instance configuration; actual storage is in thread-local map).
    #[allow(dead_code)]
    thread_cache_stack: ThreadLocal<RefCell<Vec<CachePtr<T>>>>,
}

/// Wrapper for thread-local storage that works with `Send + Sync`.
struct ThreadLocal<T: 'static> {
    _phantom: std::marker::PhantomData<T>,
}

impl<T: 'static> ThreadLocal<RefCell<Vec<CachePtr<T>>>> {
    fn new() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }
}

// We use std::thread_local! for actual thread-local storage.
// Since ThreadLocalScopedCache is generic, we use a different approach:
// store the stacks in a thread-local map keyed by a unique ID per instance.

use std::any::Any;
use std::collections::HashMap;
use std::sync::atomic::AtomicU64;

/// Counter for generating unique cache IDs (reserved for future use).
#[allow(dead_code)]
static NEXT_CACHE_ID: AtomicU64 = AtomicU64::new(0);

thread_local! {
    static CACHE_STACKS: RefCell<HashMap<u64, Box<dyn Any>>> = RefCell::new(HashMap::new());
}

impl<T: Default + Send + Sync + 'static> ThreadLocalScopedCache<T> {
    /// Creates a new thread-local scoped cache.
    pub fn new() -> Self {
        Self {
            thread_cache_stack: ThreadLocal::new(),
        }
    }

    /// Unique ID for this cache instance (used for thread-local storage).
    fn id(&self) -> u64 {
        // Use the address of self as a unique key. This is safe because
        // ThreadLocalScopedCache instances are typically stored in structs
        // with stable addresses (e.g., in a resolver).
        self as *const _ as u64
    }

    /// Gets (or creates) the thread-local cache stack for this instance.
    fn with_stack<R, F: FnOnce(&mut Vec<CachePtr<T>>) -> R>(&self, f: F) -> R {
        let id = self.id();
        CACHE_STACKS.with(|stacks| {
            let mut stacks = stacks.borrow_mut();
            let stack = stacks
                .entry(id)
                .or_insert_with(|| Box::new(Vec::<CachePtr<T>>::new()));
            let stack = stack
                .downcast_mut::<Vec<CachePtr<T>>>()
                .expect("cache stack type mismatch");
            f(stack)
        })
    }

    /// Marks the start of a resolution caching scope.
    ///
    /// If `existing_cache` is provided, it will be pushed onto the stack
    /// (parent-scope sharing). Otherwise, a new cache is created or the
    /// existing top cache is shared.
    ///
    /// Returns the cache pointer that was pushed, which can be passed to
    /// child scopes for sharing.
    pub fn begin_cache_scope(&self, existing_cache: Option<CachePtr<T>>) -> CachePtr<T> {
        self.with_stack(|stack| {
            let cache = if let Some(existing) = existing_cache {
                // Reuse provided cache (parent-scope sharing)
                existing
            } else if let Some(current) = stack.last() {
                // Share with current top scope
                current.clone()
            } else {
                // Create new cache
                Arc::new(RwLock::new(T::default()))
            };
            stack.push(cache.clone());
            cache
        })
    }

    /// Marks the end of a resolution caching scope.
    ///
    /// Pops the most recent cache from the stack.
    pub fn end_cache_scope(&self) {
        self.with_stack(|stack| {
            if stack.is_empty() {
                // TF_CODING_ERROR equivalent
                eprintln!(
                    "ThreadLocalScopedCache: EndCacheScope called without matching BeginCacheScope"
                );
            } else {
                stack.pop();
            }
        });
    }

    /// Returns the current cache, if any cache scope is active.
    ///
    /// Returns `None` if no cache scope is currently active on this thread.
    pub fn get_current_cache(&self) -> Option<CachePtr<T>> {
        self.with_stack(|stack| stack.last().cloned())
    }

    /// Returns true if a cache scope is currently active on this thread.
    pub fn is_cache_active(&self) -> bool {
        self.with_stack(|stack| !stack.is_empty())
    }
}

impl<T: Default + Send + Sync + 'static> Default for ThreadLocalScopedCache<T> {
    fn default() -> Self {
        Self::new()
    }
}

// SAFETY: ThreadLocalScopedCache must be Send + Sync since it's stored in resolvers
// that are shared across threads. The actual data is thread-local (stored in
// thread_local! static), so there's no shared mutable state. Each thread accesses
// its own independent stack via CACHE_STACKS thread-local storage.
#[allow(unsafe_code)]
unsafe impl<T: Default + Send + Sync + 'static> Send for ThreadLocalScopedCache<T> {}

#[allow(unsafe_code)]
unsafe impl<T: Default + Send + Sync + 'static> Sync for ThreadLocalScopedCache<T> {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    type TestCache = HashMap<String, String>;

    #[test]
    fn test_basic_scope() {
        let cache = ThreadLocalScopedCache::<TestCache>::new();

        // No active cache initially
        assert!(!cache.is_cache_active());
        assert!(cache.get_current_cache().is_none());

        // Begin scope
        let ptr = cache.begin_cache_scope(None);
        assert!(cache.is_cache_active());
        assert!(cache.get_current_cache().is_some());

        // Write to cache
        {
            let mut c = ptr.write().expect("lock poisoned");
            c.insert("key".to_string(), "value".to_string());
        }

        // Read from cache
        {
            let current = cache.get_current_cache().unwrap();
            let c = current.read().expect("lock poisoned");
            assert_eq!(c.get("key"), Some(&"value".to_string()));
        }

        // End scope
        cache.end_cache_scope();
        assert!(!cache.is_cache_active());
    }

    #[test]
    fn test_nested_scopes_share_cache() {
        let cache = ThreadLocalScopedCache::<TestCache>::new();

        // Open outer scope
        let outer_ptr = cache.begin_cache_scope(None);
        {
            let mut c = outer_ptr.write().expect("lock poisoned");
            c.insert("outer".to_string(), "data".to_string());
        }

        // Open inner scope (should share same cache)
        let inner_ptr = cache.begin_cache_scope(None);
        {
            let c = inner_ptr.read().expect("lock poisoned");
            assert_eq!(c.get("outer"), Some(&"data".to_string()));
        }

        // Close inner
        cache.end_cache_scope();
        assert!(cache.is_cache_active());

        // Close outer
        cache.end_cache_scope();
        assert!(!cache.is_cache_active());
    }

    #[test]
    fn test_explicit_cache_sharing() {
        let cache = ThreadLocalScopedCache::<TestCache>::new();

        // Create first scope
        let ptr1 = cache.begin_cache_scope(None);
        {
            let mut c = ptr1.write().expect("lock poisoned");
            c.insert("shared".to_string(), "yes".to_string());
        }
        cache.end_cache_scope();

        // Create second scope reusing the first cache
        let _ptr2 = cache.begin_cache_scope(Some(ptr1));
        {
            let current = cache.get_current_cache().unwrap();
            let c = current.read().expect("lock poisoned");
            assert_eq!(c.get("shared"), Some(&"yes".to_string()));
        }
        cache.end_cache_scope();
    }

    #[test]
    fn test_thread_isolation() {
        let cache = Arc::new(ThreadLocalScopedCache::<TestCache>::new());

        let cache2 = cache.clone();
        let handle = std::thread::spawn(move || {
            // This thread should have no active cache
            assert!(!cache2.is_cache_active());

            // Begin scope in this thread
            let ptr = cache2.begin_cache_scope(None);
            {
                let mut c = ptr.write().expect("lock poisoned");
                c.insert("thread".to_string(), "local".to_string());
            }
            assert!(cache2.is_cache_active());
            cache2.end_cache_scope();
        });

        // Main thread should be unaffected
        assert!(!cache.is_cache_active());

        handle.join().unwrap();
    }
}
