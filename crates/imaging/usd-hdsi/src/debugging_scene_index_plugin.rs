//! Debugging scene index plugin.
//!
//! Registers debugging scene index if env var
//! HDSI_DEBUGGING_SCENE_INDEX_INSERTION_PHASE is an integer.

use super::debugging_scene_index::HdsiDebuggingSceneIndex;
use std::env;
use std::sync::Arc;
use usd_hd::data_source::HdContainerDataSourceHandle;
use usd_hd::scene_index::base::{HdSceneIndexHandle, scene_index_to_handle};
use usd_hd::scene_index::plugin::HdSceneIndexPlugin;
use usd_tf::Token as TfToken;

const PLUGIN_NAME: &str = "HdsiDebuggingSceneIndexPlugin";
const ENV_INSERTION_PHASE: &str = "HDSI_DEBUGGING_SCENE_INDEX_INSERTION_PHASE";

fn get_insertion_phase() -> Option<i32> {
    let value = env::var(ENV_INSERTION_PHASE).ok()?;
    if value.is_empty() {
        return None;
    }
    value.parse().ok()
}

/// Plugin that adds the debugging scene index when env var is set.
///
/// Call `register_if_enabled()` at startup to conditionally register
/// with HdSceneIndexPluginRegistry.
pub struct HdsiDebuggingSceneIndexPlugin;

impl HdsiDebuggingSceneIndexPlugin {
    /// Registers this plugin with the registry if HDSI_DEBUGGING_SCENE_INDEX_INSERTION_PHASE
    /// is set to an integer. Call during initialization.
    pub fn register_if_enabled() {
        if get_insertion_phase().is_none() {
            return;
        }
        let registry = usd_hd::scene_index::HdSceneIndexPluginRegistry::get_instance();
        {
            let mut reg = registry.write();
            reg.register_plugin(Arc::new(Self));
            reg.register_scene_index_for_renderer("", TfToken::new(PLUGIN_NAME));
        }
    }
}

impl HdSceneIndexPlugin for HdsiDebuggingSceneIndexPlugin {
    fn append_scene_index(
        &self,
        _render_instance_id: &str,
        input_scene: HdSceneIndexHandle,
        input_args: Option<HdContainerDataSourceHandle>,
    ) -> HdSceneIndexHandle {
        scene_index_to_handle(HdsiDebuggingSceneIndex::new(input_scene, input_args))
    }

    fn get_name(&self) -> TfToken {
        TfToken::new(PLUGIN_NAME)
    }
}
