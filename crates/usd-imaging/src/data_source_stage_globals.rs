//! Stage globals data source interface.

use std::sync::Arc;
use usd_core::TimeCode;
use usd_hd::HdDataSourceLocator;
use usd_sdf::Path;

/// Context object with global stage information passed to data sources.
///
/// This trait provides a pure virtual interface allowing different use cases
/// to override behaviors like getting time coordinates or tracking time-varying
/// attributes.
///
/// Data sources use this to:
/// - Get the current time code for sampling
/// - Flag attributes as time-varying for change tracking
/// - Flag prims as dependent on asset paths for reload tracking
pub trait DataSourceStageGlobals: Send + Sync {
    /// Returns the current time represented in this instance.
    fn get_time(&self) -> TimeCode;

    /// Flags the given Hydra path as time varying at the given locator.
    ///
    /// This is called by data sources when they detect that an attribute
    /// value changes over time. The scene index uses this to track which
    /// prims need to be dirtied on time changes.
    ///
    /// # Arguments
    ///
    /// * `hydra_path` - The Hydra prim path (may differ from USD path)
    /// * `locator` - The data source locator that varies over time
    fn flag_as_time_varying(&self, hydra_path: &Path, locator: &HdDataSourceLocator);

    /// Flags the object at USD path as dependent on an asset path.
    ///
    /// This is called when a prim or attribute has an asset path value
    /// (e.g., texture file paths). The scene index uses this to track
    /// which prims need to be invalidated when asset files change.
    ///
    /// # Arguments
    ///
    /// * `usd_path` - USD path to prim or attribute with asset dependency
    fn flag_as_asset_path_dependent(&self, usd_path: &Path);
}

/// Arc-wrapped stage globals for convenient sharing
pub type DataSourceStageGlobalsHandle = Arc<dyn DataSourceStageGlobals>;

/// Default no-op implementation for testing
#[derive(Debug, Clone)]
pub struct NoOpStageGlobals {
    time: TimeCode,
}

impl NoOpStageGlobals {
    /// Create new no-op globals with given time
    pub fn new(time: TimeCode) -> Self {
        Self { time }
    }
}

impl Default for NoOpStageGlobals {
    fn default() -> Self {
        Self::new(TimeCode::default_time())
    }
}

impl DataSourceStageGlobals for NoOpStageGlobals {
    fn get_time(&self) -> TimeCode {
        self.time
    }

    fn flag_as_time_varying(&self, _hydra_path: &Path, _locator: &HdDataSourceLocator) {
        // No-op
    }

    fn flag_as_asset_path_dependent(&self, _usd_path: &Path) {
        // No-op
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_noop_globals() {
        let globals = NoOpStageGlobals::default();
        assert_eq!(globals.get_time(), TimeCode::default_time());
    }

    #[test]
    fn test_noop_globals_custom_time() {
        let time = TimeCode::new(123.0);
        let globals = NoOpStageGlobals::new(time);
        assert_eq!(globals.get_time(), time);
    }

    #[test]
    fn test_noop_globals_flags() {
        let globals = NoOpStageGlobals::default();
        let path = Path::absolute_root();
        let locator = HdDataSourceLocator::empty();

        // Should not panic
        globals.flag_as_time_varying(&path, &locator);
        globals.flag_as_asset_path_dependent(&path);
    }

    #[test]
    fn test_stage_globals_handle() {
        let globals: DataSourceStageGlobalsHandle = Arc::new(NoOpStageGlobals::default());
        assert_eq!(globals.get_time(), TimeCode::default_time());
    }
}
