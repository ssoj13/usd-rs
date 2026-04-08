//! HdSt_DependencyForwardingSceneIndex - Storm plugin for dependency forwarding.
//!
//! Storm-specific scene index plugin that wraps the core
//! `HdDependencyForwardingSceneIndex` from usd-hd. This plugin is registered
//! at insertion phase 1000 (late) to resolve dependencies introduced by
//! other scene indices in the Storm pipeline.
//!
//! Port of C++ `HdSt_DependencyForwardingSceneIndexPlugin`.

use parking_lot::RwLock;
use std::sync::Arc;
use usd_hd::scene_index::{HdDependencyForwardingSceneIndex, HdSceneIndexHandle};

/// Insertion phase for the dependency forwarding plugin.
///
/// Phase 1000 = late in the pipeline, after most other scene indices
/// have had a chance to introduce dependencies.
pub const INSERTION_PHASE: u32 = 1000;

/// Storm plugin display name (matches C++ `_pluginDisplayName`).
pub const PLUGIN_DISPLAY_NAME: &str = "GL";

/// Create a dependency forwarding scene index for Storm.
///
/// This is the Storm-specific factory that wraps the core
/// `HdDependencyForwardingSceneIndex`. In C++, this is done via the plugin
/// system; in Rust, we expose it as a direct factory function.
///
/// # Arguments
/// * `input_scene` - The input scene to observe for dependencies
///
/// # Returns
/// A new dependency forwarding scene index wrapped in Arc<RwLock<>>
pub fn create(
    input_scene: Option<HdSceneIndexHandle>,
) -> Arc<RwLock<HdDependencyForwardingSceneIndex>> {
    HdDependencyForwardingSceneIndex::new(input_scene)
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_hd::HdSceneIndexBase;

    #[test]
    fn test_create() {
        let si = create(None);
        let lock = si.read();
        assert_eq!(lock.get_display_name(), "HdDependencyForwardingSceneIndex");
    }

    #[test]
    fn test_constants() {
        assert_eq!(INSERTION_PHASE, 1000);
        assert_eq!(PLUGIN_DISPLAY_NAME, "GL");
    }
}
