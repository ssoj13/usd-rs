
//! Scene index plugin registry.

use super::base::HdSceneIndexHandle;
use super::plugin::HdSceneIndexPluginHandle;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use usd_tf::Token as TfToken;

/// Registry for scene index plugins.
///
/// This is a global singleton that manages plugin registration and
/// instantiation. Plugins can be registered for specific renderers
/// or globally for all renderers.
///
/// # Thread Safety
///
/// The registry is thread-safe and can be accessed from multiple threads.
pub struct HdSceneIndexPluginRegistry {
    /// Registered plugins by name
    plugins: HashMap<TfToken, HdSceneIndexPluginHandle>,
    /// Plugins registered for specific renderers
    renderer_plugins: HashMap<String, Vec<TfToken>>,
}

impl HdSceneIndexPluginRegistry {
    /// Create a new plugin registry.
    fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            renderer_plugins: HashMap::new(),
        }
    }

    /// Get the global singleton instance.
    pub fn get_instance() -> Arc<RwLock<Self>> {
        static REGISTRY: std::sync::LazyLock<Arc<RwLock<HdSceneIndexPluginRegistry>>> =
            std::sync::LazyLock::new(|| Arc::new(RwLock::new(HdSceneIndexPluginRegistry::new())));
        REGISTRY.clone()
    }

    /// Register a scene index plugin.
    ///
    /// # Arguments
    ///
    /// * `plugin` - The plugin to register
    pub fn register_plugin(&mut self, plugin: HdSceneIndexPluginHandle) {
        let name = plugin.get_name();
        self.plugins.insert(name, plugin);
    }

    /// Register a scene index plugin for a specific renderer.
    ///
    /// # Arguments
    ///
    /// * `renderer_name` - The renderer display name
    /// * `plugin_id` - The plugin identifier (must be registered)
    pub fn register_scene_index_for_renderer(&mut self, renderer_name: &str, plugin_id: TfToken) {
        self.renderer_plugins
            .entry(renderer_name.to_string())
            .or_default()
            .push(plugin_id);
    }

    /// Get a registered plugin by name.
    pub fn get_plugin(&self, plugin_id: &TfToken) -> Option<HdSceneIndexPluginHandle> {
        self.plugins.get(plugin_id).cloned()
    }

    /// Append scene indices for a specific renderer.
    ///
    /// Applies all registered plugins for the given renderer to the input scene.
    ///
    /// # Arguments
    ///
    /// * `renderer_name` - The renderer display name
    /// * `input_scene` - The input scene
    /// * `render_instance_id` - Optional render instance identifier
    ///
    /// # Returns
    ///
    /// The final scene after all plugins have been applied, or the input
    /// scene if no plugins are registered.
    pub fn append_scene_indices_for_renderer(
        &self,
        renderer_name: &str,
        input_scene: HdSceneIndexHandle,
        render_instance_id: &str,
    ) -> HdSceneIndexHandle {
        // Get plugins for this renderer
        let Some(plugin_ids) = self.renderer_plugins.get(renderer_name) else {
            return input_scene;
        };

        // Apply each plugin in order
        let mut current_scene = input_scene;
        for plugin_id in plugin_ids {
            if let Some(plugin) = self.plugins.get(plugin_id) {
                current_scene = plugin.append_scene_index(render_instance_id, current_scene, None);
            }
        }

        current_scene
    }

    /// Get all registered plugin IDs.
    pub fn get_registered_plugin_ids(&self) -> Vec<TfToken> {
        self.plugins.keys().cloned().collect()
    }

    /// Get plugins registered for a specific renderer.
    pub fn get_renderer_plugin_ids(&self, renderer_name: &str) -> Vec<TfToken> {
        self.renderer_plugins
            .get(renderer_name)
            .cloned()
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::super::plugin::{HdSceneIndexPlugin, StubSceneIndexPlugin};
    use super::*;

    #[test]
    fn test_registry_singleton() {
        let registry1 = HdSceneIndexPluginRegistry::get_instance();
        let registry2 = HdSceneIndexPluginRegistry::get_instance();

        // Should be the same instance
        assert!(Arc::ptr_eq(&registry1, &registry2));
    }

    #[test]
    fn test_register_plugin() {
        let registry = HdSceneIndexPluginRegistry::get_instance();
        let mut registry_lock = registry.write();

        let plugin = Arc::new(StubSceneIndexPlugin::new("TestPlugin"));
        let plugin_id = (*plugin).get_name();

        registry_lock.register_plugin(plugin);

        let retrieved = registry_lock.get_plugin(&plugin_id);
        assert!(retrieved.is_some());
    }

    #[test]
    fn test_register_for_renderer() {
        let registry = HdSceneIndexPluginRegistry::get_instance();
        let mut registry_lock = registry.write();

        let plugin = Arc::new(StubSceneIndexPlugin::new("RendererPlugin"));
        let plugin_id = (*plugin).get_name();

        registry_lock.register_plugin(plugin);
        registry_lock.register_scene_index_for_renderer("TestRenderer", plugin_id.clone());

        let plugins = registry_lock.get_renderer_plugin_ids("TestRenderer");
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0], plugin_id);
    }

    #[test]
    fn test_get_registered_ids() {
        let registry = HdSceneIndexPluginRegistry::get_instance();
        let registry_lock = registry.read();

        // Should have at least the plugins registered in other tests
        let ids = registry_lock.get_registered_plugin_ids();
        // Just verify it doesn't crash
        let _ = ids.len();
    }
}
