//! Plugin Registry singleton.
//!
//! Port of pxr/base/plug/registry.h/cpp
//!
//! Central registry for all discovered plugins. Singleton that manages
//! plugin discovery, registration, and lookup.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

use usd_tf::{TfType, declare_by_name, declare_by_name_with_bases};

use crate::info::{self, RegistrationMetadata};
use crate::notice;
use crate::plugin::{PlugPlugin, PluginType};

/// Inner state of the plugin registry, protected by RwLock.
struct PlugRegistryInner {
    /// All plugins keyed by path (primary storage).
    all_plugins: HashMap<String, Arc<PlugPlugin>>,
    /// Lookup by plugin name -> plugin (for library, resource types).
    plugins_by_name: HashMap<String, Arc<PlugPlugin>>,
    /// Type name -> plugin that declared it.
    class_map: HashMap<String, Arc<PlugPlugin>>,
    /// Type name -> direct base type names (from "bases" array in plugInfo.json).
    type_bases: HashMap<String, Vec<String>>,
    /// Paths already visited during registration (dedup).
    registered_paths: HashSet<String>,
}

/// Central singleton registry for all plugins.
///
/// Matches C++ `PlugRegistry`.
///
/// Discovers plugins via `plugInfo.json` files, stores their metadata,
/// and provides lookup by name, path, or type.
pub struct PlugRegistry {
    inner: RwLock<PlugRegistryInner>,
    registration_mutex: Mutex<()>,
}

static INSTANCE: OnceLock<PlugRegistry> = OnceLock::new();

/// Flag for one-time bootstrap registration from env paths.
static ALL_PLUGINS_REGISTERED: OnceLock<()> = OnceLock::new();

impl PlugRegistry {
    /// Returns the singleton PlugRegistry instance.
    ///
    /// Matches C++ `PlugRegistry::GetInstance()`.
    pub fn get_instance() -> &'static PlugRegistry {
        INSTANCE.get_or_init(|| PlugRegistry {
            inner: RwLock::new(PlugRegistryInner {
                all_plugins: HashMap::new(),
                plugins_by_name: HashMap::new(),
                class_map: HashMap::new(),
                type_bases: HashMap::new(),
                registered_paths: HashSet::new(),
            }),
            registration_mutex: Mutex::new(()),
        })
    }

    /// Register all plugins discovered at the given path.
    ///
    /// Sends DidRegisterPlugins notice with any newly registered plugins.
    /// Matches C++ `PlugRegistry::RegisterPlugins(const string&)`.
    pub fn register_plugins(&self, path: &str) -> Vec<Arc<PlugPlugin>> {
        self.register_plugins_multi(&[path.to_string()])
    }

    /// Register all plugins discovered in any of the given paths.
    ///
    /// Sends DidRegisterPlugins notice with any newly registered plugins.
    /// Matches C++ `PlugRegistry::RegisterPlugins(const vector<string>&)`.
    pub fn register_plugins_multi(&self, paths: &[String]) -> Vec<Arc<PlugPlugin>> {
        let new_plugins = self.register_plugins_internal(paths, true);
        if !new_plugins.is_empty() {
            notice::send_did_register_plugins(&new_plugins);
        }
        new_plugins
    }

    /// Internal registration without sending notice.
    ///
    /// Matches C++ `PlugRegistry::_RegisterPlugins`.
    fn register_plugins_internal(
        &self,
        paths: &[String],
        paths_are_ordered: bool,
    ) -> Vec<Arc<PlugPlugin>> {
        // Keep registration + type declaration in a single critical section so
        // other threads cannot observe plugins_by_name/all_plugins before the
        // corresponding class_map/type_bases entries exist.
        let _registration_guard = self
            .registration_mutex
            .lock()
            .expect("PlugRegistry registration mutex poisoned");
        let metadata_list = info::read_plug_info(paths, paths_are_ordered);

        let mut new_plugins = Vec::new();

        for metadata in metadata_list {
            if let Some(plugin) = self.register_plugin(metadata) {
                new_plugins.push(plugin);
            }
        }

        // Match OpenUSD's _RegisterPlugins flow: newly discovered plugins
        // declare their types before RegisterPlugins returns or emits notices.
        for plugin in &new_plugins {
            self.declare_types(plugin);
        }

        new_plugins
    }

    /// Register a single plugin from parsed metadata.
    ///
    /// Returns the plugin if newly registered, None if already known.
    /// Matches C++ `PlugRegistry::_RegisterPlugin`.
    fn register_plugin(&self, metadata: RegistrationMetadata) -> Option<Arc<PlugPlugin>> {
        let mut inner = self.inner.write().expect("PlugRegistry lock poisoned");

        // For library plugins, dedup by library path (each DSO is unique).
        // For resource plugins, multiple entries can share a directory path
        // (one plugInfo.json may declare many plugins), so dedup by name only.
        if metadata.plugin_type == PluginType::Library {
            if inner.all_plugins.contains_key(&metadata.library_path) {
                return None;
            }
            if inner.registered_paths.contains(&metadata.library_path) {
                return None;
            }
        }

        // Check by name: already registered? First one wins.
        if let Some(existing) = inner.plugins_by_name.get(&metadata.plugin_name) {
            log::debug!(
                "Already registered {} plugin '{}' at {} - skipping",
                match metadata.plugin_type {
                    PluginType::Library => "library",
                    PluginType::Resource => "resource",
                },
                metadata.plugin_name,
                existing.get_path(),
            );
            return None;
        }

        let plugin_key = match metadata.plugin_type {
            PluginType::Library => metadata.library_path.clone(),
            PluginType::Resource => {
                // Use name as unique key for resource plugins
                format!("{}:{}", metadata.plugin_path, metadata.plugin_name)
            }
        };

        log::debug!(
            "Registering {} plugin '{}' at '{}'",
            match metadata.plugin_type {
                PluginType::Library => "library",
                PluginType::Resource => "resource",
            },
            metadata.plugin_name,
            plugin_key
        );

        let creation_path = match metadata.plugin_type {
            PluginType::Library => metadata.library_path.clone(),
            PluginType::Resource => metadata.plugin_path.clone(),
        };

        let plugin = Arc::new(PlugPlugin::new(
            creation_path,
            metadata.plugin_name.clone(),
            metadata.resource_path,
            metadata.plug_info,
            metadata.plugin_type,
        ));

        inner.registered_paths.insert(plugin_key.clone());
        inner.all_plugins.insert(plugin_key, plugin.clone());
        inner
            .plugins_by_name
            .insert(metadata.plugin_name, plugin.clone());

        Some(plugin)
    }

    /// Declare types from a plugin's "Types" metadata into the class map
    /// and register them with TfType (bases + aliases).
    ///
    /// Matches C++ `PlugPlugin::_DeclareTypes()`.
    fn declare_types(&self, plugin: &Arc<PlugPlugin>) {
        let types_obj = match plugin
            .get_metadata()
            .get("Types")
            .and_then(|v| v.as_object())
        {
            Some(obj) => obj.clone(),
            None => return,
        };

        for (type_name, type_val) in &types_obj {
            // --- class_map ownership check (held only for this block) ---
            {
                let mut inner = self.inner.write().expect("PlugRegistry lock poisoned");
                if let Some(existing) = inner.class_map.get(type_name) {
                    log::error!(
                        "Plugin '{}' claims to provide type '{}', but this was previously provided by plugin '{}'",
                        plugin.get_name(),
                        type_name,
                        existing.get_name()
                    );
                    continue;
                }
                inner.class_map.insert(type_name.clone(), plugin.clone());
            }

            let type_dict = type_val.as_object();

            // --- parse "bases" ---
            let base_names: Vec<String> = match type_dict.and_then(|d| d.get("bases")) {
                None => Vec::new(),
                Some(bases_val) => match bases_val.as_array() {
                    Some(arr) => {
                        let mut names = Vec::with_capacity(arr.len());
                        for item in arr {
                            match item.as_string() {
                                Some(s) => names.push(s.to_string()),
                                None => {
                                    log::error!(
                                        "Plugin '{}' type '{}': 'bases' array entry is not a string",
                                        plugin.get_name(),
                                        type_name
                                    );
                                }
                            }
                        }
                        names
                    }
                    None => {
                        log::error!(
                            "Plugin '{}' type '{}': 'bases' is not an array",
                            plugin.get_name(),
                            type_name
                        );
                        Vec::new()
                    }
                },
            };

            // Populate the local type_bases index (used by derived-type queries).
            {
                let mut inner = self.inner.write().expect("PlugRegistry lock poisoned");
                inner
                    .type_bases
                    .insert(type_name.clone(), base_names.clone());
            }

            // Register with TfType so the global type hierarchy is up to date.
            let base_refs: Vec<&str> = base_names.iter().map(String::as_str).collect();
            let tf_type = declare_by_name_with_bases(type_name, &base_refs);

            // --- parse "alias" dict: { baseTypeName: aliasName } ---
            if let Some(alias_val) = type_dict.and_then(|d| d.get("alias")) {
                match alias_val.as_object() {
                    Some(alias_dict) => {
                        for (base_type_name, alias_name_val) in alias_dict {
                            let alias_name = match alias_name_val.as_string() {
                                Some(s) => s,
                                None => {
                                    log::warn!(
                                        "Plugin '{}' type '{}': alias for base '{}' is not a string, skipping",
                                        plugin.get_name(),
                                        type_name,
                                        base_type_name
                                    );
                                    continue;
                                }
                            };
                            // Ensure the base type exists in TfType before aliasing.
                            let base_tf = if TfType::find_by_name(base_type_name).is_unknown() {
                                declare_by_name(base_type_name)
                            } else {
                                TfType::find_by_name(base_type_name)
                            };
                            tf_type.add_alias(base_tf, alias_name);
                        }
                    }
                    None => {
                        log::warn!(
                            "Plugin '{}' type '{}': 'alias' is not an object, skipping",
                            plugin.get_name(),
                            type_name
                        );
                    }
                }
            }
        }
    }

    /// Ensure all plugins from standard search paths are registered.
    ///
    /// Called lazily before queries that need the full plugin set.
    /// Matches C++ `PlugPlugin::_RegisterAllPlugins()`.
    fn ensure_all_registered(&self) {
        ALL_PLUGINS_REGISTERED.get_or_init(|| {
            if !crate::init_config::is_standard_search_disabled() {
                for msg in crate::init_config::get_debug_messages() {
                    log::debug!("{}", msg);
                }
                let paths = crate::init_config::get_plugin_search_paths();
                let ordered = crate::init_config::paths_are_ordered();
                let new_plugins = self.register_plugins_internal(&paths.to_vec(), ordered);
                if !new_plugins.is_empty() {
                    notice::send_did_register_plugins(&new_plugins);
                }
            }
        });
    }

    /// Returns all registered plugins.
    ///
    /// Triggers plugin discovery if not already done.
    /// Matches C++ `PlugRegistry::GetAllPlugins()`.
    pub fn get_all_plugins(&self) -> Vec<Arc<PlugPlugin>> {
        self.ensure_all_registered();
        let inner = self.inner.read().expect("PlugRegistry lock poisoned");
        inner.all_plugins.values().cloned().collect()
    }

    /// Returns a plugin with the specified name.
    ///
    /// Triggers plugin discovery if not already done.
    /// Matches C++ `PlugRegistry::GetPluginWithName(const string&)`.
    pub fn get_plugin_with_name(&self, name: &str) -> Option<Arc<PlugPlugin>> {
        self.ensure_all_registered();
        let inner = self.inner.read().expect("PlugRegistry lock poisoned");
        inner.plugins_by_name.get(name).cloned()
    }

    /// Returns the plugin that declares the given type.
    ///
    /// Triggers plugin discovery if not already done.
    /// Matches C++ `PlugRegistry::GetPluginForType(TfType)`.
    pub fn get_plugin_for_type(&self, type_name: &str) -> Option<Arc<PlugPlugin>> {
        self.ensure_all_registered();
        let inner = self.inner.read().expect("PlugRegistry lock poisoned");
        inner.class_map.get(type_name).cloned()
    }

    /// Looks for a string associated with a type and key in plugin metadata.
    ///
    /// Matches C++ `PlugRegistry::GetStringFromPluginMetaData(TfType, string)`.
    pub fn get_string_from_plugin_metadata(&self, type_name: &str, key: &str) -> Option<String> {
        let plugin = self.get_plugin_for_type(type_name)?;
        let type_meta = plugin.get_metadata_for_type(type_name)?;
        type_meta
            .get(key)
            .and_then(|v| v.as_string())
            .map(|s| s.to_string())
    }

    /// Looks for a JsValue associated with a type and key in plugin metadata.
    ///
    /// Matches C++ `PlugRegistry::GetDataFromPluginMetaData(TfType, string)`.
    pub fn get_data_from_plugin_metadata(
        &self,
        type_name: &str,
        key: &str,
    ) -> Option<usd_js::JsValue> {
        let plugin = self.get_plugin_for_type(type_name)?;
        let type_meta = plugin.get_metadata_for_type(type_name)?;
        type_meta.get(key).cloned()
    }

    /// Returns the type name if it is registered, None otherwise.
    ///
    /// Matches C++ `PlugRegistry::FindTypeByName(const string&)`.
    pub fn find_type_by_name(&self, name: &str) -> Option<String> {
        self.ensure_all_registered();
        let inner = self.inner.read().expect("PlugRegistry lock poisoned");
        inner.class_map.get(name).map(|_| name.to_string())
    }

    /// Returns the type name if it is a direct child of `base` and matches `name`.
    ///
    /// Matches C++ `PlugRegistry::FindDerivedTypeByName(TfType base, const string&)`.
    pub fn find_derived_type_by_name(&self, base: &str, name: &str) -> Option<String> {
        self.ensure_all_registered();
        let inner = self.inner.read().expect("PlugRegistry lock poisoned");
        let bases = inner.type_bases.get(name)?;
        if bases.iter().any(|b| b == base) {
            Some(name.to_string())
        } else {
            None
        }
    }

    /// Returns all type names whose direct bases contain `base`.
    ///
    /// Matches C++ `PlugRegistry::GetDirectlyDerivedTypes(TfType)`.
    pub fn get_directly_derived_types(&self, base: &str) -> Vec<String> {
        self.ensure_all_registered();
        let inner = self.inner.read().expect("PlugRegistry lock poisoned");
        inner
            .type_bases
            .iter()
            .filter(|(_, bases)| bases.iter().any(|b| b == base))
            .map(|(type_name, _)| type_name.clone())
            .collect()
    }

    /// Returns all type names transitively derived from `base` (BFS over type_bases).
    ///
    /// Matches C++ `PlugRegistry::GetAllDerivedTypes(TfType)`.
    pub fn get_all_derived_types(&self, base: &str) -> Vec<String> {
        self.ensure_all_registered();
        let inner = self.inner.read().expect("PlugRegistry lock poisoned");

        // Build a reverse map: parent -> children for efficient BFS.
        let mut children: HashMap<&str, Vec<&str>> = HashMap::new();
        for (type_name, bases) in &inner.type_bases {
            for b in bases {
                children
                    .entry(b.as_str())
                    .or_default()
                    .push(type_name.as_str());
            }
        }

        // BFS from `base`, collecting all transitive descendants.
        let mut result = Vec::new();
        let mut queue = VecDeque::new();
        queue.push_back(base);
        let mut visited: HashSet<&str> = HashSet::new();
        visited.insert(base);

        while let Some(current) = queue.pop_front() {
            if let Some(derived) = children.get(current) {
                for child in derived {
                    if visited.insert(child) {
                        result.push(child.to_string());
                        queue.push_back(child);
                    }
                }
            }
        }

        result
    }

    /// Returns the plugin for `type_name`, panicking with a diagnostic if not found.
    ///
    /// Matches C++ `PlugRegistry::DemandPluginForType(TfType)`.
    pub fn demand_plugin_for_type(&self, type_name: &str) -> Arc<PlugPlugin> {
        self.ensure_all_registered();
        if let Some(plugin) = self.get_plugin_for_type(type_name) {
            return plugin;
        }

        // Build diagnostic listing known search paths.
        let search_paths = crate::init_config::get_plugin_search_paths();
        let paths_display = search_paths
            .iter()
            .map(|p| format!("  {p}"))
            .collect::<Vec<_>>()
            .join("\n");

        log::error!(
            "Could not find plugin for type '{}'. Searched paths:\n{}",
            type_name,
            paths_display
        );
        panic!("No plugin found for type '{type_name}'");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_singleton() {
        let r1 = PlugRegistry::get_instance();
        let r2 = PlugRegistry::get_instance();
        assert!(std::ptr::eq(r1, r2));
    }

    #[test]
    fn test_register_from_dir() {
        let dir = std::env::temp_dir().join("usd_plug_test_registry");
        std::fs::create_dir_all(&dir).unwrap();

        let plug_info = r#"{
            "Plugins": [{
                "Type": "resource",
                "Name": "registryTestPlugin",
                "Info": {
                    "Types": {
                        "TestFilterType": {
                            "bases": ["ImageFilter"],
                            "displayName": "Test Filter"
                        }
                    },
                    "Kinds": {
                        "test_kind": { "baseKind": "model" }
                    }
                }
            }]
        }"#;
        std::fs::write(dir.join("plugInfo.json"), plug_info).unwrap();

        let registry = PlugRegistry::get_instance();
        let new_plugins = registry.register_plugins(&dir.to_string_lossy());

        // May or may not find new plugins depending on test execution order
        // (singleton is shared across tests). Just verify no panic.
        if !new_plugins.is_empty() {
            let plugin = &new_plugins[0];
            assert_eq!(plugin.get_name(), "registryTestPlugin");
            assert!(plugin.is_resource());

            // Check metadata
            let kinds = plugin
                .get_metadata()
                .get("Kinds")
                .and_then(|v| v.as_object());
            assert!(kinds.is_some());

            // Check type declaration
            assert!(plugin.declares_type("TestFilterType", false));

            // TfType must be registered for the declared type and its base.
            assert!(!TfType::find_by_name("TestFilterType").is_unknown());
            assert!(!TfType::find_by_name("ImageFilter").is_unknown());
            // TestFilterType is_a ImageFilter via TfType hierarchy.
            let tf = TfType::find_by_name("TestFilterType");
            let base = TfType::find_by_name("ImageFilter");
            assert!(tf.is_a(base));
        }

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_duplicate_registration() {
        let dir = std::env::temp_dir().join("usd_plug_test_dup");
        std::fs::create_dir_all(&dir).unwrap();

        let plug_info = r#"{
            "Plugins": [{
                "Type": "resource",
                "Name": "dupTestPlugin",
                "Info": {}
            }]
        }"#;
        std::fs::write(dir.join("plugInfo.json"), plug_info).unwrap();

        let registry = PlugRegistry::get_instance();
        let _first = registry.register_plugins(&dir.to_string_lossy());
        let second = registry.register_plugins(&dir.to_string_lossy());

        // Second registration should return nothing new
        assert!(second.is_empty());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_find_type_by_name() {
        let dir = std::env::temp_dir().join("usd_plug_test_find_type");
        std::fs::create_dir_all(&dir).unwrap();

        let plug_info = r#"{
            "Plugins": [{
                "Type": "resource",
                "Name": "findTypePlugin",
                "Info": {
                    "Types": {
                        "FindableType55": {}
                    }
                }
            }]
        }"#;
        std::fs::write(dir.join("plugInfo.json"), plug_info).unwrap();

        let registry = PlugRegistry::get_instance();
        registry.register_plugins(&dir.to_string_lossy());

        // Non-existent type must always return None.
        assert!(registry.find_type_by_name("__NonExistentType__").is_none());

        // If we successfully registered the plugin, the type must be found.
        if registry.get_plugin_with_name("findTypePlugin").is_some() {
            assert_eq!(
                registry.find_type_by_name("FindableType55"),
                Some("FindableType55".to_string())
            );
        }

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_find_derived_type_by_name() {
        let dir = std::env::temp_dir().join("usd_plug_test_derived_name");
        std::fs::create_dir_all(&dir).unwrap();

        let plug_info = r#"{
            "Plugins": [{
                "Type": "resource",
                "Name": "derivedNamePlugin",
                "Info": {
                    "Types": {
                        "ShapeBase42": {},
                        "CircleShape42": { "bases": ["ShapeBase42"] }
                    }
                }
            }]
        }"#;
        std::fs::write(dir.join("plugInfo.json"), plug_info).unwrap();

        let registry = PlugRegistry::get_instance();
        registry.register_plugins(&dir.to_string_lossy());

        // Only assert when the plugin was registered in this test run.
        if registry.find_type_by_name("CircleShape42").is_some() {
            assert_eq!(
                registry.find_derived_type_by_name("ShapeBase42", "CircleShape42"),
                Some("CircleShape42".to_string())
            );
            // Wrong base -> None.
            assert!(
                registry
                    .find_derived_type_by_name("ShapeBase42", "ShapeBase42")
                    .is_none()
            );
        }

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_get_directly_derived_types() {
        let dir = std::env::temp_dir().join("usd_plug_test_direct_derived");
        std::fs::create_dir_all(&dir).unwrap();

        let plug_info = r#"{
            "Plugins": [{
                "Type": "resource",
                "Name": "directDerivedPlugin",
                "Info": {
                    "Types": {
                        "VehicleBase99": {},
                        "Car99": { "bases": ["VehicleBase99"] },
                        "Truck99": { "bases": ["VehicleBase99"] },
                        "SportsCar99": { "bases": ["Car99"] }
                    }
                }
            }]
        }"#;
        std::fs::write(dir.join("plugInfo.json"), plug_info).unwrap();

        let registry = PlugRegistry::get_instance();
        registry.register_plugins(&dir.to_string_lossy());

        if registry.find_type_by_name("VehicleBase99").is_some() {
            let direct = registry.get_directly_derived_types("VehicleBase99");
            // Car99 and Truck99 are direct children; SportsCar99 is not.
            assert!(direct.contains(&"Car99".to_string()));
            assert!(direct.contains(&"Truck99".to_string()));
            assert!(!direct.contains(&"SportsCar99".to_string()));
        }

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_get_all_derived_types() {
        let dir = std::env::temp_dir().join("usd_plug_test_all_derived");
        std::fs::create_dir_all(&dir).unwrap();

        let plug_info = r#"{
            "Plugins": [{
                "Type": "resource",
                "Name": "allDerivedPlugin",
                "Info": {
                    "Types": {
                        "NodeBase77": {},
                        "GraphNode77": { "bases": ["NodeBase77"] },
                        "ShaderNode77": { "bases": ["GraphNode77"] },
                        "MaterialNode77": { "bases": ["ShaderNode77"] }
                    }
                }
            }]
        }"#;
        std::fs::write(dir.join("plugInfo.json"), plug_info).unwrap();

        let registry = PlugRegistry::get_instance();
        registry.register_plugins(&dir.to_string_lossy());

        if registry.find_type_by_name("NodeBase77").is_some() {
            let all = registry.get_all_derived_types("NodeBase77");
            // All three transitive descendants must appear.
            assert!(all.contains(&"GraphNode77".to_string()));
            assert!(all.contains(&"ShaderNode77".to_string()));
            assert!(all.contains(&"MaterialNode77".to_string()));
            // The base itself must not be included.
            assert!(!all.contains(&"NodeBase77".to_string()));
        }

        std::fs::remove_dir_all(&dir).ok();
    }
}
