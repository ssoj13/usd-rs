//! Cleanup enabler for automatic spec cleanup.
//!
//! Port of pxr/usd/sdf/cleanupEnabler.h
//!
//! Provides an RAII guard that enables automatic cleanup of specs
//! that no longer contribute to the scene.

use std::cell::Cell;

thread_local! {
    /// Depth counter for cleanup enablers on this thread.
    static CLEANUP_DEPTH: Cell<u32> = const { Cell::new(0) };
}

/// An RAII class which, when alive, enables scheduling of automatic cleanup.
///
/// Any affected specs which no longer contribute to the scene will be removed
/// when the last `CleanupEnabler` instance goes out of scope.
///
/// For property specs, they are removed if they have only required fields,
/// but only if the property spec itself was affected by an edit that left
/// it with only required fields. This has the effect of uninstantiating
/// on-demand attributes.
///
/// # Examples
///
/// ```ignore
/// use usd_sdf::CleanupEnabler;
///
/// {
///     let _enabler = CleanupEnabler::new();
///     
///     // Perform any action that might leave inert specs around,
///     // such as removing info from properties or prims.
///     prim_spec.clear_info("default");
///
///     // When enabler goes out of scope, prim_spec will be removed
///     // if it has been left as an empty over.
/// }
/// ```
pub struct CleanupEnabler {
    /// Marker to prevent Send/Sync (thread-local state).
    _marker: std::marker::PhantomData<*const ()>,
}

impl CleanupEnabler {
    /// Creates a new cleanup enabler, incrementing the cleanup depth.
    pub fn new() -> Self {
        CLEANUP_DEPTH.with(|depth| {
            depth.set(depth.get() + 1);
        });
        Self {
            _marker: std::marker::PhantomData,
        }
    }

    /// Returns whether cleanup is currently being scheduled.
    ///
    /// This is true when at least one `CleanupEnabler` is in scope.
    pub fn is_cleanup_enabled() -> bool {
        CLEANUP_DEPTH.with(|depth| depth.get() > 0)
    }

    /// Returns the current nesting depth of cleanup enablers.
    pub fn depth() -> u32 {
        CLEANUP_DEPTH.with(|depth| depth.get())
    }
}

impl Default for CleanupEnabler {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for CleanupEnabler {
    fn drop(&mut self) {
        CLEANUP_DEPTH.with(|depth| {
            let current = depth.get();
            if current > 0 {
                depth.set(current - 1);
            }

            // When the last enabler goes out of scope, trigger cleanup
            if current == 1 {
                // In a full implementation, this would trigger the
                // cleanup tracker to process pending cleanups
                trigger_cleanup();
            }
        });
    }
}

/// Triggers the cleanup of specs that no longer contribute to the scene.
///
/// This is called automatically when the last CleanupEnabler goes out of scope.
fn trigger_cleanup() {
    // In a full implementation, this would:
    // 1. Get the cleanup tracker
    // 2. For each spec in the tracker:
    //    - Check if it's inert (has only required fields)
    //    - If inert, remove it from the layer
    // For now, this is a placeholder that can be extended
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cleanup_enabler_basic() {
        assert!(!CleanupEnabler::is_cleanup_enabled());
        assert_eq!(CleanupEnabler::depth(), 0);

        {
            let _enabler = CleanupEnabler::new();
            assert!(CleanupEnabler::is_cleanup_enabled());
            assert_eq!(CleanupEnabler::depth(), 1);

            {
                let _nested = CleanupEnabler::new();
                assert!(CleanupEnabler::is_cleanup_enabled());
                assert_eq!(CleanupEnabler::depth(), 2);
            }

            assert!(CleanupEnabler::is_cleanup_enabled());
            assert_eq!(CleanupEnabler::depth(), 1);
        }

        assert!(!CleanupEnabler::is_cleanup_enabled());
        assert_eq!(CleanupEnabler::depth(), 0);
    }

    #[test]
    fn test_cleanup_enabler_default() {
        let _enabler = CleanupEnabler::default();
        assert!(CleanupEnabler::is_cleanup_enabled());
    }
}
