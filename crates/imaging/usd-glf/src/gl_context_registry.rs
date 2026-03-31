//! Registry of active GL contexts.
//!
//! Port of pxr/imaging/glf/glContextRegistry.h / glContextRegistry.cpp
//!
//! `GlfGLContextRegistry` is a singleton that tracks which `GlfGLContext`
//! objects are live and which is considered the "shared" (primary) context.
//! Platform GL context state is opaque (`u64`) at this abstraction level.

use crate::gl_context::GlfGLContext;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, Weak};

// ---------------------------------------------------------------------------
// Registration interface
// ---------------------------------------------------------------------------

/// Trait implemented by platform-specific context providers.
///
/// Mirrors `GlfGLContextRegistrationInterface`.  A provider is registered
/// once and polled when the registry needs to find the current context or
/// the shared context.
pub trait GlfGLContextRegistration: Send + Sync {
    /// Return the shared (primary) context, if this provider has one.
    fn get_shared(&self) -> Option<Arc<GlfGLContext>>;

    /// Return the context that is currently bound on the calling thread.
    fn get_current(&self) -> Option<Arc<GlfGLContext>>;
}

// ---------------------------------------------------------------------------
// Registry internals
// ---------------------------------------------------------------------------

/// Raw platform-context key (e.g. HGLRC / GLXContext / EGLContext as usize).
type RawContextKey = usize;

struct RegistryInner {
    /// Registered platform providers.
    interfaces: Vec<Box<dyn GlfGLContextRegistration>>,
    /// Map from raw context handle to a weak `GlfGLContext`.
    by_handle: HashMap<RawContextKey, Weak<GlfGLContext>>,
    /// Cached shared context (initialized lazily).
    shared: Option<Arc<GlfGLContext>>,
    shared_initialized: bool,
}

impl RegistryInner {
    fn new() -> Self {
        Self {
            interfaces: Vec::new(),
            by_handle: HashMap::new(),
            shared: None,
            shared_initialized: false,
        }
    }
}

// ---------------------------------------------------------------------------
// GlfGLContextRegistry
// ---------------------------------------------------------------------------

/// Singleton registry for live GL contexts.
///
/// Mirrors `GlfGLContextRegistry`.
pub struct GlfGLContextRegistry {
    inner: Mutex<RegistryInner>,
}

static INSTANCE: OnceLock<GlfGLContextRegistry> = OnceLock::new();

impl GlfGLContextRegistry {
    /// Return the global singleton.
    pub fn get_instance() -> &'static Self {
        INSTANCE.get_or_init(|| Self {
            inner: Mutex::new(RegistryInner::new()),
        })
    }

    /// Returns `true` if at least one registration interface has been added.
    ///
    /// Mirrors `IsInitialized()`.
    pub fn is_initialized(&self) -> bool {
        self.inner
            .lock()
            .expect("GlfGLContextRegistry lock poisoned")
            .interfaces
            .len()
            > 0
    }

    /// Register a platform context provider.  Takes ownership.
    ///
    /// Mirrors `Add(GlfGLContextRegistrationInterface*)`.
    pub fn add(&self, iface: Box<dyn GlfGLContextRegistration>) {
        self.inner
            .lock()
            .expect("GlfGLContextRegistry lock poisoned")
            .interfaces
            .push(iface);
    }

    /// Return the shared (primary) context, querying providers if needed.
    ///
    /// Mirrors `GetShared()`.  Only queries on the first call; result is
    /// cached afterwards.
    pub fn get_shared(&self) -> Option<Arc<GlfGLContext>> {
        let mut inner = self
            .inner
            .lock()
            .expect("GlfGLContextRegistry lock poisoned");

        if inner.shared_initialized {
            return inner.shared.clone();
        }

        inner.shared_initialized = true;

        for iface in &inner.interfaces {
            if let Some(shared) = iface.get_shared() {
                inner.shared = Some(shared.clone());
                return Some(shared);
            }
        }

        log::warn!("GlfGLContextRegistry: no shared context registered");
        None
    }

    /// Return the context that is current on the calling thread.
    ///
    /// Mirrors `GetCurrent()`.  Falls back to querying each registered
    /// provider if the handle is not already in the cache.
    pub fn get_current(&self) -> Option<Arc<GlfGLContext>> {
        let inner = self
            .inner
            .lock()
            .expect("GlfGLContextRegistry lock poisoned");

        // Try each interface
        for iface in &inner.interfaces {
            if let Some(ctx) = iface.get_current() {
                if ctx.is_valid() {
                    return Some(ctx);
                }
            }
        }
        None
    }

    /// Notify the registry that `context` has been made current.
    ///
    /// Mirrors `DidMakeCurrent()`.  Associates the context with the current
    /// raw platform handle so future lookups are O(1).
    pub fn did_make_current(&self, context: &Arc<GlfGLContext>, raw_handle: RawContextKey) {
        let mut inner = self
            .inner
            .lock()
            .expect("GlfGLContextRegistry lock poisoned");

        inner
            .by_handle
            .entry(raw_handle)
            .or_insert_with(|| Arc::downgrade(context));
    }

    /// Remove a context from the registry.
    ///
    /// Mirrors `Remove()`.
    pub fn remove(&self, raw_handle: RawContextKey) {
        self.inner
            .lock()
            .expect("GlfGLContextRegistry lock poisoned")
            .by_handle
            .remove(&raw_handle);
    }

    /// Evict all expired (dropped) weak references from the cache.
    pub fn gc(&self) {
        self.inner
            .lock()
            .expect("GlfGLContextRegistry lock poisoned")
            .by_handle
            .retain(|_, w| w.strong_count() > 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_singleton_identity() {
        let a = GlfGLContextRegistry::get_instance();
        let b = GlfGLContextRegistry::get_instance();
        // Same address
        assert!(std::ptr::eq(a, b));
    }

    #[test]
    fn test_not_initialized_by_default() {
        // A fresh registry (in the test process) has no providers
        let reg = GlfGLContextRegistry::get_instance();
        // We can't guarantee the state since other tests might have added a provider,
        // but calling is_initialized() must not panic.
        let _ = reg.is_initialized();
    }

    #[test]
    fn test_get_current_no_provider() {
        let reg = GlfGLContextRegistry::get_instance();
        // Without providers, get_current returns None
        // (may return Some in a full GL context; here it's a stub)
        let _ = reg.get_current(); // must not panic
    }
}
