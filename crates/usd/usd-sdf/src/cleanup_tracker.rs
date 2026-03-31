//! SdfCleanupTracker - tracks specs for automatic cleanup.
//!
//! Port of pxr/usd/sdf/cleanupTracker.h
//!
//! A singleton that tracks specs edited within a CleanupEnabler scope.
//! When the last CleanupEnabler goes out of scope, the tracked specs are
//! removed from the layer if they are inert (have no meaningful content).

use crate::{CleanupEnabler, Layer, Path, SpecType};
use std::sync::{Arc, Mutex, OnceLock, Weak};

/// Entry for a tracked spec.
#[derive(Clone)]
struct TrackedSpec {
    /// Weak reference to the layer.
    layer: Weak<Layer>,
    /// Path to the spec.
    path: Path,
    /// Type of the spec.
    spec_type: SpecType,
}

/// Singleton that tracks specs edited within a CleanupEnabler scope.
///
/// When cleanup is triggered (the last CleanupEnabler goes out of scope),
/// all tracked specs that are "inert" (have no meaningful content) are
/// automatically removed from their layers.
pub struct CleanupTracker {
    /// Tracked specs pending cleanup.
    specs: Mutex<Vec<TrackedSpec>>,
}

impl CleanupTracker {
    /// Gets the singleton instance.
    pub fn instance() -> &'static CleanupTracker {
        static INSTANCE: OnceLock<CleanupTracker> = OnceLock::new();
        INSTANCE.get_or_init(CleanupTracker::new)
    }

    /// Creates a new cleanup tracker.
    fn new() -> Self {
        Self {
            specs: Mutex::new(Vec::new()),
        }
    }

    /// Adds a spec to tracking if cleanup is currently enabled.
    ///
    /// Call this when a spec is edited and may need cleanup later.
    pub fn add_spec_if_tracking(&self, layer: &Arc<Layer>, path: &Path, spec_type: SpecType) {
        if !CleanupEnabler::is_cleanup_enabled() {
            return;
        }

        let entry = TrackedSpec {
            layer: Arc::downgrade(layer),
            path: path.clone(),
            spec_type,
        };

        let mut specs = self.specs.lock().expect("lock poisoned");
        specs.push(entry);
    }

    /// Cleans up all tracked specs that are inert.
    ///
    /// This is called automatically when the last CleanupEnabler goes out of
    /// scope. Inert specs (those with no meaningful content) are removed from
    /// their layers.
    pub fn cleanup_specs(&self) {
        let specs = {
            let mut guard = self.specs.lock().expect("lock poisoned");
            std::mem::take(&mut *guard)
        };

        // Process in reverse order (clean up children before parents)
        for entry in specs.into_iter().rev() {
            if let Some(layer) = entry.layer.upgrade() {
                if Self::is_spec_inert(&layer, &entry.path, entry.spec_type) {
                    Self::remove_spec(&layer, &entry.path, entry.spec_type);
                }
            }
        }
    }

    /// Checks if a spec is inert (has no meaningful content).
    fn is_spec_inert(layer: &Arc<Layer>, path: &Path, _spec_type: SpecType) -> bool {
        // A spec is inert if it has no fields
        layer.list_fields(path).is_empty()
    }

    /// Removes a spec from a layer.
    ///
    /// Matches C++ cleanup tracker behavior - uses layer's delete_spec.
    fn remove_spec(layer: &Arc<Layer>, path: &Path, _spec_type: SpecType) {
        layer.delete_spec(path);
    }

    /// Clears all tracked specs without processing them.
    pub fn clear(&self) {
        let mut specs = self.specs.lock().expect("lock poisoned");
        specs.clear();
    }

    /// Returns the number of tracked specs.
    pub fn tracked_count(&self) -> usize {
        self.specs.lock().expect("lock poisoned").len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_singleton() {
        let t1 = CleanupTracker::instance();
        let t2 = CleanupTracker::instance();
        assert!(std::ptr::eq(t1, t2));
    }

    #[test]
    fn test_tracking_disabled() {
        let tracker = CleanupTracker::instance();
        let initial_count = tracker.tracked_count();

        let layer = Layer::create_anonymous(Some("test"));
        let path = Path::from_string("/Test").unwrap();

        // Without CleanupEnabler, nothing is tracked
        tracker.add_spec_if_tracking(&layer, &path, SpecType::Prim);
        assert_eq!(tracker.tracked_count(), initial_count);
    }

    #[test]
    fn test_clear() {
        let tracker = CleanupTracker::instance();
        tracker.clear();
        assert_eq!(tracker.tracked_count(), 0);
    }
}
