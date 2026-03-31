#![allow(dead_code)]
//! RAII-style context binding for asset resolver.
//!
//! The `ResolverContextBinder` provides automatic binding and unbinding
//! of resolver contexts using RAII semantics.
//!
//! Matches C++ `ArResolverContextBinder` from `resolverContextBinder.h`.

use usd_vt::Value;

use super::resolver::get_resolver;
use super::resolver_context::ResolverContext;

/// Helper object for managing resolver context binding.
///
/// Context binding and unbinding are thread-specific. If you bind a context
/// in a thread, that binding will only be visible to that thread.
///
/// When a `ResolverContextBinder` is created, it binds the given context
/// to the resolver. When it is dropped, the context is automatically unbound.
///
/// Matches C++ `ArResolverContextBinder`. In C++ the resolver pointer is stored
/// and reused in the destructor. In Rust, since the global resolver is a
/// `'static` singleton that never changes, `drop` always acquires the
/// global resolver for unbinding. `new_with_resolver` binds via the given
/// resolver but unbinds via the global one — this is correct because OpenUSD
/// only ever has a single active resolver.
///
/// # Examples
///
/// ```ignore
/// use usd_ar::{ResolverContext, ResolverContextBinder, DefaultResolverContext};
///
/// let ctx = ResolverContext::with_object(
///     DefaultResolverContext::new(vec!["/assets".into()])
/// );
///
/// {
///     let _binder = ResolverContextBinder::new(ctx);
///     // Context is now bound
///     // ... resolve assets using this context ...
/// } // Context is automatically unbound when _binder goes out of scope
/// ```
pub struct ResolverContextBinder {
    /// The bound context.
    context: ResolverContext,
    /// Binding data returned by bind_context.
    binding_data: Option<Value>,
}

impl ResolverContextBinder {
    /// Creates a new binder that binds the given context to the default resolver.
    ///
    /// The context will be automatically unbound when this binder is dropped.
    ///
    /// # Arguments
    ///
    /// * `context` - The context to bind
    pub fn new(context: ResolverContext) -> Self {
        let binding_data = if let Ok(resolver) = get_resolver().read() {
            resolver.bind_context(&context)
        } else {
            None
        };

        Self {
            context,
            binding_data,
        }
    }

    /// Creates a new binder that binds the given context to a specific resolver.
    ///
    /// Calls `resolver.bind_context()` on the given resolver. On drop,
    /// unbinding is performed via the global resolver (which in OpenUSD is
    /// always the same single resolver instance).
    ///
    /// Matches C++ `ArResolverContextBinder(ArResolver*, context)`.
    ///
    /// # Arguments
    ///
    /// * `resolver` - The resolver to bind the context to
    /// * `context` - The context to bind
    pub fn new_with_resolver(
        resolver: &dyn super::resolver::Resolver,
        context: ResolverContext,
    ) -> Self {
        let binding_data = resolver.bind_context(&context);

        Self {
            context,
            binding_data,
        }
    }

    /// Returns a reference to the bound context.
    pub fn context(&self) -> &ResolverContext {
        &self.context
    }
}

impl Drop for ResolverContextBinder {
    fn drop(&mut self) {
        // Unbind using the global resolver. This matches C++ behavior
        // because OpenUSD only ever has one active resolver instance.
        if let Ok(resolver) = get_resolver().read() {
            resolver.unbind_context(&self.context, self.binding_data.take());
        }
    }
}

/// RAII-style resolution cache scope.
///
/// When a `ResolverScopedCache` is created, it opens a resolution caching
/// scope. When it is dropped, the scope is closed. Resolution results
/// within the scope may be cached for consistency and performance.
///
/// # Examples
///
/// ```ignore
/// use usd_ar::ResolverScopedCache;
///
/// {
///     let _cache = ResolverScopedCache::new();
///     // Resolution results within this scope may be cached
///     // ... resolve multiple assets ...
/// } // Cache scope is closed
/// ```
pub struct ResolverScopedCache {
    /// Cache scope data returned by begin_cache_scope.
    cache_scope_data: Option<Value>,
}

impl ResolverScopedCache {
    /// Creates a new scoped cache.
    ///
    /// This opens a resolution caching scope. The scope is closed when
    /// this object is dropped.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_ar::ResolverScopedCache;
    ///
    /// let cache = ResolverScopedCache::new();
    /// ```
    pub fn new() -> Self {
        let cache_scope_data = if let Ok(resolver) = get_resolver().read() {
            resolver.begin_cache_scope()
        } else {
            None
        };

        Self { cache_scope_data }
    }

    /// Creates a new scoped cache sharing a parent's cache data.
    ///
    /// Matches C++ `ArResolverScopedCache(const ArResolverScopedCache* parent)`.
    /// The parent's cache data is copied, then `begin_cache_scope_with_parent`
    /// is called so the resolver can share the parent's cache in the new scope.
    ///
    /// # Arguments
    ///
    /// * `parent_cache_data` - Cache data from a parent scope to share
    pub fn with_data(parent_cache_data: Option<Value>) -> Self {
        let cache_scope_data = if let Ok(resolver) = get_resolver().read() {
            match parent_cache_data {
                Some(ref parent_data) => resolver.begin_cache_scope_with_parent(parent_data),
                None => resolver.begin_cache_scope(),
            }
        } else {
            None
        };
        Self { cache_scope_data }
    }

    /// Returns a reference to the cache scope data.
    pub fn cache_data(&self) -> Option<&Value> {
        self.cache_scope_data.as_ref()
    }
}

impl Default for ResolverScopedCache {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ResolverScopedCache {
    fn drop(&mut self) {
        if let Ok(resolver) = get_resolver().read() {
            resolver.end_cache_scope(self.cache_scope_data.take());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::resolver_context::DefaultResolverContext;
    use super::*;

    #[test]
    fn test_resolver_context_binder_new() {
        let ctx = ResolverContext::new();
        let _binder = ResolverContextBinder::new(ctx);
        // Should not panic
    }

    #[test]
    fn test_resolver_context_binder_with_context() {
        let ctx =
            ResolverContext::with_object(DefaultResolverContext::new(vec!["/test/path".into()]));
        let binder = ResolverContextBinder::new(ctx.clone());

        assert_eq!(binder.context(), &ctx);
    }

    #[test]
    fn test_resolver_context_binder_drop() {
        let ctx =
            ResolverContext::with_object(DefaultResolverContext::new(vec!["/test/path".into()]));

        {
            let _binder = ResolverContextBinder::new(ctx);
            // Context should be bound here
        }
        // Context should be unbound after binder is dropped
    }

    #[test]
    fn test_resolver_scoped_cache_new() {
        let _cache = ResolverScopedCache::new();
        // Should not panic
    }

    #[test]
    fn test_resolver_scoped_cache_default() {
        let _cache = ResolverScopedCache::default();
        // Should not panic
    }

    #[test]
    fn test_resolver_scoped_cache_with_data() {
        let data = Some(Value::from(42));
        let cache = ResolverScopedCache::with_data(data);
        assert!(cache.cache_data().is_some());
    }

    #[test]
    fn test_resolver_scoped_cache_drop() {
        {
            let _cache = ResolverScopedCache::new();
            // Cache scope should be open here
        }
        // Cache scope should be closed after cache is dropped
    }

    #[test]
    fn test_nested_binders() {
        let ctx1 = ResolverContext::with_object(DefaultResolverContext::new(vec!["/path1".into()]));
        let ctx2 = ResolverContext::with_object(DefaultResolverContext::new(vec!["/path2".into()]));

        {
            let _binder1 = ResolverContextBinder::new(ctx1);
            {
                let _binder2 = ResolverContextBinder::new(ctx2);
                // Inner context should be active
            }
            // Outer context should be restored
        }
        // All contexts should be unbound
    }
}
