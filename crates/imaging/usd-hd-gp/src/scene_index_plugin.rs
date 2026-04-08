//! Scene index plugin integration for generative procedurals.

use super::generative_procedural_resolving_scene_index::HdGpGenerativeProceduralResolvingSceneIndex;
use usd_hd::data_source::HdContainerDataSourceHandle;
use usd_hd::scene_index::{HdSceneIndexHandle, HdSceneIndexPlugin};
use usd_tf::Token as TfToken;

/// Token key for reading proceduralPrimTypeName from inputArgs.
static PROCEDURAL_PRIM_TYPE_NAME: once_cell::sync::Lazy<TfToken> =
    once_cell::sync::Lazy::new(|| TfToken::new("proceduralPrimTypeName"));

/// Insertion phase for the generative procedural scene index plugin.
///
/// Returns 2 to allow other plugins to run before and after this plugin.
/// (Avoids using 0 for better ordering flexibility)
pub const INSERTION_PHASE: i32 = 2;

/// Scene index plugin for generative procedurals.
///
/// Provides HdSceneIndexPluginRegistry access to instantiate
/// HdGpGenerativeProceduralResolvingSceneIndex either directly or
/// automatically via RegisterSceneIndexForRenderer.
///
/// # Example
///
/// ```ignore
/// use usd_hd_gp::*;
///
/// // Register the plugin
/// let plugin = HdGpSceneIndexPlugin::new();
/// HdSceneIndexPluginRegistry::register_plugin(plugin);
///
/// // Plugin will be automatically inserted in the scene index chain
/// ```
pub struct HdGpSceneIndexPlugin {
    name: TfToken,
}

impl HdGpSceneIndexPlugin {
    /// Create a new scene index plugin instance.
    pub fn new() -> Self {
        Self {
            name: TfToken::new("HdGpSceneIndexPlugin"),
        }
    }

    /// Get the insertion phase for this plugin.
    pub fn get_insertion_phase() -> i32 {
        INSERTION_PHASE
    }
}

impl Default for HdGpSceneIndexPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl HdSceneIndexPlugin for HdGpSceneIndexPlugin {
    fn get_name(&self) -> TfToken {
        self.name.clone()
    }

    fn append_scene_index(
        &self,
        _render_instance_id: &str,
        input_scene: HdSceneIndexHandle,
        input_args: Option<HdContainerDataSourceHandle>,
    ) -> HdSceneIndexHandle {
        // Check inputArgs for a proceduralPrimTypeName token.
        // Matches C++ _AppendSceneIndex which reads from inputArgs.
        if let Some(ref args) = input_args {
            if let Some(type_ds) = args.get(&PROCEDURAL_PRIM_TYPE_NAME) {
                if let Some(sampled) = type_ds.as_sampled() {
                    let value = sampled.get_value(0.0);
                    if let Some(token) = value.get::<TfToken>() {
                        return HdGpGenerativeProceduralResolvingSceneIndex::new_with_type(
                            input_scene,
                            token.clone(),
                        ) as HdSceneIndexHandle;
                    }
                }
            }
        }

        HdGpGenerativeProceduralResolvingSceneIndex::new(input_scene) as HdSceneIndexHandle
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_creation() {
        let plugin = HdGpSceneIndexPlugin::new();
        assert_eq!(plugin.get_name().as_str(), "HdGpSceneIndexPlugin");
    }

    #[test]
    fn test_insertion_phase() {
        assert_eq!(HdGpSceneIndexPlugin::get_insertion_phase(), 2);
        assert_eq!(INSERTION_PHASE, 2);
    }

    #[test]
    fn test_default() {
        let plugin = HdGpSceneIndexPlugin::default();
        assert_eq!(plugin.get_name().as_str(), "HdGpSceneIndexPlugin");
    }
}
