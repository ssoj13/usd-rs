//! Trace collection notices.
//!
//! Port of pxr/base/trace/collectionNotice.h
//!
//! Provides notification support for when TraceCollections become available.

use super::Collection;
use std::sync::Arc;

// ============================================================================
// Collection Available Notice
// ============================================================================

/// A notice that is sent when the TraceCollector creates a TraceCollection.
///
/// This can potentially be sent from multiple threads. Listeners must be
/// thread safe.
#[derive(Debug, Clone)]
pub struct CollectionAvailable {
    /// The collection that was produced.
    collection: Arc<Collection>,
}

impl CollectionAvailable {
    /// Creates a new CollectionAvailable notice.
    pub fn new(collection: Arc<Collection>) -> Self {
        Self { collection }
    }

    /// Returns the TraceCollection which was produced.
    pub fn collection(&self) -> &Arc<Collection> {
        &self.collection
    }

    /// Consumes the notice and returns the collection.
    pub fn into_collection(self) -> Arc<Collection> {
        self.collection
    }
}

// ============================================================================
// Notice Listener Trait
// ============================================================================

/// Trait for objects that can receive collection available notices.
///
/// Implementations must be thread-safe.
pub trait CollectionListener: Send + Sync {
    /// Called when a new collection becomes available.
    fn on_collection_available(&self, notice: &CollectionAvailable);
}

// ============================================================================
// Notice Registry
// ============================================================================

use parking_lot::RwLock;
use std::sync::Weak;

/// Registry for collection notice listeners.
///
/// Thread-safe registry that allows listeners to subscribe to and
/// receive collection available notices.
pub struct CollectionNoticeRegistry {
    /// Registered listeners (weak references to allow cleanup).
    listeners: RwLock<Vec<Weak<dyn CollectionListener>>>,
}

impl CollectionNoticeRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        Self {
            listeners: RwLock::new(Vec::new()),
        }
    }

    /// Registers a listener to receive collection notices.
    pub fn register(&self, listener: &Arc<dyn CollectionListener>) {
        let weak = Arc::downgrade(listener);
        let mut listeners = self.listeners.write();
        listeners.push(weak);
    }

    /// Sends a notice to all registered listeners.
    pub fn send(&self, notice: &CollectionAvailable) {
        let listeners = self.listeners.read();
        for weak in listeners.iter() {
            if let Some(listener) = weak.upgrade() {
                listener.on_collection_available(notice);
            }
        }
    }

    /// Removes dead listeners from the registry.
    pub fn cleanup(&self) {
        let mut listeners = self.listeners.write();
        listeners.retain(|weak| weak.strong_count() > 0);
    }

    /// Returns the number of active listeners.
    pub fn listener_count(&self) -> usize {
        let listeners = self.listeners.read();
        listeners.iter().filter(|w| w.strong_count() > 0).count()
    }
}

impl Default for CollectionNoticeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Global Registry
// ============================================================================

use once_cell::sync::Lazy;

/// Global collection notice registry.
static GLOBAL_REGISTRY: Lazy<CollectionNoticeRegistry> = Lazy::new(CollectionNoticeRegistry::new);

/// Returns the global collection notice registry.
pub fn global_registry() -> &'static CollectionNoticeRegistry {
    &GLOBAL_REGISTRY
}

/// Sends a collection available notice to all registered listeners.
pub fn send_collection_available(collection: Arc<Collection>) {
    let notice = CollectionAvailable::new(collection);
    global_registry().send(&notice);
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TestListener {
        call_count: AtomicUsize,
    }

    impl TestListener {
        fn new() -> Self {
            Self {
                call_count: AtomicUsize::new(0),
            }
        }

        #[allow(dead_code)]
        fn count(&self) -> usize {
            self.call_count.load(Ordering::SeqCst)
        }
    }

    impl CollectionListener for TestListener {
        fn on_collection_available(&self, _notice: &CollectionAvailable) {
            self.call_count.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn test_collection_available_notice() {
        let collection = Arc::new(Collection::new());
        let notice = CollectionAvailable::new(collection.clone());
        assert!(Arc::ptr_eq(notice.collection(), &collection));
    }

    #[test]
    fn test_registry_send() {
        let registry = CollectionNoticeRegistry::new();
        let listener: Arc<dyn CollectionListener> = Arc::new(TestListener::new());

        registry.register(&listener);

        let collection = Arc::new(Collection::new());
        let notice = CollectionAvailable::new(collection);
        registry.send(&notice);

        // Check listener count
        assert_eq!(registry.listener_count(), 1);
    }

    #[test]
    fn test_registry_cleanup() {
        let registry = CollectionNoticeRegistry::new();

        {
            let listener: Arc<dyn CollectionListener> = Arc::new(TestListener::new());
            registry.register(&listener);
            assert_eq!(registry.listener_count(), 1);
        }
        // listener dropped here

        registry.cleanup();
        assert_eq!(registry.listener_count(), 0);
    }
}
