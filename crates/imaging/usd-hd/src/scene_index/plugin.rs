
//! Scene index plugin interface.

use super::base::{HdContainerDataSourceHandle, HdSceneIndexHandle};
use std::sync::Arc;
use usd_tf::Token as TfToken;

/// Plugin interface for scene indices.
///
/// Scene index plugins can be registered with the plugin registry
/// and instantiated on demand. This allows renderer-specific or
/// application-specific scene processing.
///
/// # Implementation
///
/// Implement this trait and register your plugin with
/// HdSceneIndexPluginRegistry.
pub trait HdSceneIndexPlugin: Send + Sync {
    /// Append scene indices to the chain.
    ///
    /// Given an input scene and optional arguments, create and return
    /// one or more scene indices. The return value should be the final
    /// scene in the chain, or the input scene if no processing is needed.
    ///
    /// # Arguments
    ///
    /// * `render_instance_id` - Identifier for the render instance (optional)
    /// * `input_scene` - The input scene to process
    /// * `input_args` - Optional configuration arguments
    fn append_scene_index(
        &self,
        _render_instance_id: &str,
        input_scene: HdSceneIndexHandle,
        _input_args: Option<HdContainerDataSourceHandle>,
    ) -> HdSceneIndexHandle {
        // Default: return input unchanged
        input_scene
    }

    /// Get the plugin name/identifier.
    fn get_name(&self) -> TfToken;
}

/// Strong reference to a scene index plugin.
pub type HdSceneIndexPluginHandle = Arc<dyn HdSceneIndexPlugin>;

/// Stub implementation for testing.
pub struct StubSceneIndexPlugin {
    name: TfToken,
}

impl StubSceneIndexPlugin {
    /// Create a new stub plugin.
    pub fn new(name: &str) -> Self {
        Self {
            name: TfToken::new(name),
        }
    }
}

impl HdSceneIndexPlugin for StubSceneIndexPlugin {
    fn get_name(&self) -> TfToken {
        self.name.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stub_plugin() {
        let plugin = StubSceneIndexPlugin::new("TestPlugin");
        assert_eq!(plugin.get_name().as_str(), "TestPlugin");
    }
}
