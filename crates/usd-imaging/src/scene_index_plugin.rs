//! UsdImagingSceneIndexPlugin - Trait and registry for scene index plugins.
//!
//! Port of pxr/usdImaging/usdImaging/sceneIndexPlugin.h and sceneIndexPlugin.cpp
//!
//! Base trait for plugins that insert filtering scene indices into
//! UsdImaging pipeline (e.g. UsdSkelImaging).

use parking_lot::RwLock;
use std::sync::{Arc, LazyLock};
use usd_hd::data_source::HdDataSourceBaseHandle;
use usd_hd::data_source::HdRetainedContainerDataSource;
use usd_hd::scene_index::{HdContainerDataSourceHandle, HdSceneIndexHandle};
use usd_tf::Token;

/// Trait for UsdImaging scene index plugins.
///
/// Plugins can insert filtering scene indices into the UsdImaging pipeline.
/// Used by UsdSkelImaging and similar to add skeletal skinning, etc.
pub trait UsdImagingSceneIndexPlugin: Send + Sync {
    /// Append this plugin's scene index to the input scene chain.
    ///
    /// The plugin wraps the input scene with one or more filtering scene indices
    /// and returns the final scene index in the chain.
    fn append_scene_index(&self, input_scene: HdSceneIndexHandle) -> HdSceneIndexHandle;

    /// Register additional HdFlattenedDataSourceProvider's for the flattening scene index.
    ///
    /// Returns empty container by default. Override to add providers.
    fn flattened_data_source_providers(&self) -> HdContainerDataSourceHandle {
        HdRetainedContainerDataSource::new_empty()
    }

    /// Register names for instance aggregation.
    ///
    /// Additional data source names that the native instance aggregation
    /// scene index should consider when grouping instances.
    fn instance_data_source_names(&self) -> Vec<Token> {
        vec![]
    }

    /// Register names for proxy path translation.
    ///
    /// Prim-level data source names that should receive path translation
    /// for path-valued data sources pointing at instance proxies.
    fn proxy_path_translation_data_source_names(&self) -> Vec<Token> {
        vec![]
    }
}

/// Handle to a UsdImaging scene index plugin.
pub type UsdImagingSceneIndexPluginHandle = Arc<dyn UsdImagingSceneIndexPlugin>;

// ============================================================================
// UsdImagingSceneIndexPluginRegistry
// ============================================================================

/// Registry for UsdImaging scene index plugins.
///
/// Port of UsdImagingSceneIndexPlugin::GetAllSceneIndexPlugins().
/// In C++ plugins are discovered via TfType/PlugRegistry; in Rust we use
/// explicit registration. Built-in plugins (e.g. UsdSkelImaging) are
/// auto-registered on first access.
pub struct UsdImagingSceneIndexPluginRegistry {
    plugins: Vec<UsdImagingSceneIndexPluginHandle>,
}

impl UsdImagingSceneIndexPluginRegistry {
    fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    /// Register a plugin. Call from plugin init or application startup.
    pub fn register_plugin(&mut self, plugin: UsdImagingSceneIndexPluginHandle) {
        self.plugins.push(plugin);
    }

    /// Register built-in UsdImaging plugins (UsdSkelImaging, etc.).
    /// Called automatically on first get_all_plugins() if empty.
    fn register_builtin_plugins(&mut self) {
        use crate::skel::ResolvingSceneIndexPlugin;
        self.plugins.push(Arc::new(ResolvingSceneIndexPlugin));
    }

    /// Get all registered plugins. Auto-registers builtins on first call.
    pub fn get_all_plugins(&mut self) -> Vec<UsdImagingSceneIndexPluginHandle> {
        if self.plugins.is_empty() {
            self.register_builtin_plugins();
        }
        self.plugins.clone()
    }
}

static PLUGIN_REGISTRY: LazyLock<RwLock<UsdImagingSceneIndexPluginRegistry>> =
    LazyLock::new(|| RwLock::new(UsdImagingSceneIndexPluginRegistry::new()));

impl UsdImagingSceneIndexPluginRegistry {
    /// Get the global plugin registry.
    pub fn get_instance() -> &'static LazyLock<RwLock<UsdImagingSceneIndexPluginRegistry>> {
        &PLUGIN_REGISTRY
    }

    /// Append plugin scene indices to the given scene. Port of _AddPluginSceneIndices.
    pub fn add_plugin_scene_indices(scene: HdSceneIndexHandle) -> HdSceneIndexHandle {
        let mut registry_guard = PLUGIN_REGISTRY.write();
        let plugins = registry_guard.get_all_plugins();
        drop(registry_guard);

        let mut current = scene;
        for plugin in plugins {
            current = plugin.append_scene_index(current);
        }
        current
    }

    /// Collect instance data source names from all plugins.
    pub fn instance_data_source_names() -> Vec<Token> {
        let mut registry_guard = PLUGIN_REGISTRY.write();
        let plugins = registry_guard.get_all_plugins();
        drop(registry_guard);

        let mut result = Vec::new();
        for plugin in plugins {
            result.extend(plugin.instance_data_source_names());
        }
        result
    }

    /// Collect proxy path translation data source names from all plugins.
    pub fn proxy_path_translation_data_source_names() -> Vec<Token> {
        let mut registry_guard = PLUGIN_REGISTRY.write();
        let plugins = registry_guard.get_all_plugins();
        drop(registry_guard);

        let mut result = Vec::new();
        for plugin in plugins {
            result.extend(plugin.proxy_path_translation_data_source_names());
        }
        result
    }

    /// Merge flattened data source providers from all plugins.
    pub fn flattened_data_source_providers_from_plugins() -> HdContainerDataSourceHandle {
        let mut registry_guard = PLUGIN_REGISTRY.write();
        let plugins = registry_guard.get_all_plugins();
        drop(registry_guard);

        let mut all_entries = std::collections::HashMap::<Token, HdDataSourceBaseHandle>::new();
        for plugin in plugins.iter() {
            let ds = plugin.flattened_data_source_providers();
            for name in ds.get_names() {
                if let Some(child) = ds.get(&name) {
                    all_entries.insert(name, child);
                }
            }
        }
        HdRetainedContainerDataSource::new(all_entries)
    }
}
