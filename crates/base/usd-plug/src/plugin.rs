//! Plugin representation.
//!
//! Port of pxr/base/plug/plugin.h/cpp
//!
//! Each registered plugin is represented by a `PlugPlugin` instance
//! which provides access to metadata and supports lazy code loading.

use std::collections::HashSet;
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};
use usd_js::JsObject;

/// Plugin type, matching C++ `PlugPlugin::_Type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginType {
    /// Native shared library (cdylib).
    Library,
    /// Metadata-only plugin (no code loading).
    Resource,
}

/// A registered plugin.
///
/// Matches C++ `PlugPlugin`. Stores plugin metadata parsed from plugInfo.json
/// and supports lazy loading for library plugins.
pub struct PlugPlugin {
    name: String,
    path: String,
    resource_path: String,
    /// The "Info" dict from plugInfo.json
    dict: JsObject,
    plugin_type: PluginType,
    is_loaded: AtomicBool,
    /// Holds the loaded shared library handle for Library-type plugins.
    library_handle: Mutex<Option<libloading::Library>>,
}

impl PlugPlugin {
    /// Creates a new plugin. Only called by PlugRegistry during registration.
    ///
    /// Matches C++ `PlugPlugin::PlugPlugin(path, name, resourcePath, plugInfo, type)`.
    pub fn new(
        path: String,
        name: String,
        resource_path: String,
        plug_info: JsObject,
        plugin_type: PluginType,
    ) -> Self {
        let is_loaded = plugin_type == PluginType::Resource;
        Self {
            name,
            path,
            resource_path,
            dict: plug_info,
            plugin_type,
            is_loaded: AtomicBool::new(is_loaded),
            library_handle: Mutex::new(None),
        }
    }

    /// Returns the plugin's name.
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Returns the plugin's filesystem path.
    pub fn get_path(&self) -> &str {
        &self.path
    }

    /// Returns the plugin's resources filesystem path.
    pub fn get_resource_path(&self) -> &str {
        &self.resource_path
    }

    /// Returns true if the plugin is currently loaded.
    /// Resource plugins always report as loaded.
    pub fn is_loaded(&self) -> bool {
        self.is_loaded.load(Ordering::Acquire)
    }

    /// Returns true if the plugin is resource-only (no code).
    pub fn is_resource(&self) -> bool {
        self.plugin_type == PluginType::Resource
    }

    /// Returns the plugin type.
    pub fn get_type(&self) -> PluginType {
        self.plugin_type
    }

    /// Returns the full metadata dictionary ("Info" section from plugInfo.json).
    ///
    /// Matches C++ `PlugPlugin::GetMetadata()`.
    pub fn get_metadata(&self) -> &JsObject {
        &self.dict
    }

    /// Returns the metadata sub-dictionary for a particular type name.
    ///
    /// Looks up `type_name` in the "Types" dict within the plugin metadata.
    /// Matches C++ `PlugPlugin::GetMetadataForType(const TfType&)`.
    pub fn get_metadata_for_type(&self, type_name: &str) -> Option<JsObject> {
        let types = self.dict.get("Types")?;
        let types_obj = types.as_object()?;
        let entry = types_obj.get(type_name)?;
        entry.as_object().cloned()
    }

    /// Returns the dependencies dictionary ("PluginDependencies" key).
    ///
    /// Matches C++ `PlugPlugin::GetDependencies()`.
    pub fn get_dependencies(&self) -> JsObject {
        self.dict
            .get("PluginDependencies")
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default()
    }

    /// Returns true if the given type_name is declared by this plugin.
    ///
    /// If `include_subclasses` is true, also checks if any type in the
    /// plugin's "Types" dict lists `type_name` in its "bases".
    ///
    /// Matches C++ `PlugPlugin::DeclaresType(TfType, bool)`.
    pub fn declares_type(&self, type_name: &str, include_subclasses: bool) -> bool {
        let types_obj = match self.dict.get("Types").and_then(|v| v.as_object()) {
            Some(obj) => obj,
            None => return false,
        };

        for (declared_name, value) in types_obj {
            if !include_subclasses {
                if declared_name == type_name {
                    return true;
                }
            } else {
                // Check if this type or any of its bases match
                if declared_name == type_name {
                    return true;
                }
                if let Some(dict) = value.as_object() {
                    if let Some(bases) = dict.get("bases").and_then(|b| b.as_array()) {
                        for base in bases {
                            if base.as_string() == Some(type_name) {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        false
    }

    /// Build a plugin resource path by returning an absolute path as-is or
    /// combining the plugin's resource path with a relative path.
    ///
    /// Matches C++ `PlugPlugin::MakeResourcePath(const std::string&)`.
    pub fn make_resource_path(&self, path: &str) -> String {
        if path.is_empty() {
            return String::new();
        }
        // Check absolute: both platform-native and Unix-style /
        if std::path::Path::new(path).is_absolute() || path.starts_with('/') {
            return path.to_string();
        }
        let mut result = self.resource_path.clone();
        if !result.ends_with('/') && !result.ends_with('\\') {
            result.push('/');
        }
        result.push_str(path);
        result
    }

    /// Find a plugin resource by absolute or relative path, optionally
    /// verifying that the file exists.
    ///
    /// Matches C++ `PlugPlugin::FindPluginResource(const std::string&, bool)`.
    pub fn find_plugin_resource(&self, path: &str, verify: bool) -> String {
        let result = self.make_resource_path(path);
        if verify && !std::path::Path::new(&result).exists() {
            return String::new();
        }
        result
    }

    /// Returns all type names declared in the "Types" dict.
    pub fn get_declared_types(&self) -> Vec<String> {
        self.dict
            .get("Types")
            .and_then(|v| v.as_object())
            .map(|obj| obj.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Marks the plugin as loaded. Used internally after successful code loading.
    pub fn set_loaded(&self) {
        self.is_loaded.store(true, Ordering::Release);
    }

    /// Loads this plugin and all its dependencies.
    ///
    /// Idempotent -- returns Ok immediately if already loaded.
    /// Matches C++ `PlugPlugin::Load()` which uses `static recursive_mutex loadMutex`.
    ///
    /// The Rust port serializes only the actual library loading (`_load()`),
    /// not the dependency walk. This avoids the need for a recursive mutex
    /// and eliminates potential deadlock with the registry RwLock.
    pub fn load(&self) -> Result<(), String> {
        if self.is_loaded() {
            return Ok(());
        }
        let mut seen = HashSet::new();
        self.load_with_dependents(&mut seen)
    }

    /// Recursively loads dependencies then loads this plugin.
    ///
    /// `seen_plugins` tracks visited names to detect cycles.
    /// Matches C++ `PlugPlugin::_LoadWithDependents`.
    pub fn load_with_dependents(&self, seen_plugins: &mut HashSet<String>) -> Result<(), String> {
        if self.is_loaded() {
            return Ok(());
        }

        if seen_plugins.contains(&self.name) {
            return Err(format!("cyclic dependency on plugin '{}'", self.name));
        }
        seen_plugins.insert(self.name.clone());

        // The "PluginDependencies" dict maps base-type names to arrays of
        // dependent type names. For each dependent type, find its plugin and
        // load it first.
        let deps = self.get_dependencies();
        for (base_type_name, dep_list_val) in &deps {
            // C++ validates base type exists: TfType::FindByName(baseTypeName).IsUnknown()
            let base_tf = usd_tf::TfType::find_by_name(base_type_name);
            if base_tf.is_unknown() {
                return Err(format!(
                    "unknown base class '{}' in dependencies of plugin '{}'",
                    base_type_name, self.name
                ));
            }
            let dep_names = match dep_list_val.as_array() {
                Some(arr) => arr,
                None => {
                    return Err(format!(
                        "dependency list for '{}' in plugin '{}' is not an array",
                        base_type_name, self.name
                    ));
                }
            };
            for dep_val in dep_names {
                let dep_name = match dep_val.as_string() {
                    Some(s) => s,
                    None => {
                        return Err(format!(
                            "dependency entry in plugin '{}' is not a string",
                            self.name
                        ));
                    }
                };
                let dep_plugin = crate::registry::PlugRegistry::get_instance()
                    .get_plugin_for_type(dep_name)
                    .ok_or_else(|| {
                        format!(
                            "unknown dependent type '{}' required by plugin '{}'",
                            dep_name, self.name
                        )
                    })?;
                dep_plugin
                    .load_with_dependents(seen_plugins)
                    .map_err(|e| format!("failed to load dependency '{}': {}", dep_name, e))?;
            }
        }

        self._load()
    }

    /// Performs the actual shared-library load for Library-type plugins.
    ///
    /// Resource plugins carry no code and are already considered loaded.
    /// An empty path means a monolithic/static build -- treated as success.
    /// Uses a process-wide mutex to serialize library loading across threads.
    /// Matches C++ `PlugPlugin::_Load()`.
    fn _load(&self) -> Result<(), String> {
        if self.plugin_type == PluginType::Resource {
            return Ok(());
        }

        static LOAD_MUTEX: Mutex<()> = Mutex::new(());
        let _guard = LOAD_MUTEX.lock().map_err(|e| e.to_string())?;

        if self.path.is_empty() {
            // Monolithic or static build -- library already linked in.
            log::debug!(
                "No library path for plugin '{}'; assuming statically linked.",
                self.name
            );
            self.set_loaded();
            return Ok(());
        }

        #[allow(unsafe_code)]
        // SAFETY: loading a C-ABI shared library by filesystem path.
        // The handle is stored inside the plugin and dropped only when the
        // plugin itself is dropped.
        let lib = unsafe { libloading::Library::new(&self.path) }.map_err(|e| {
            let msg = format!(
                "failed to load library '{}' for plugin '{}': {}",
                self.path, self.name, e
            );
            log::error!("{}", msg);
            msg
        })?;

        *self.library_handle.lock().map_err(|e| e.to_string())? = Some(lib);

        // Set loaded only after the handle is safely stored, so other threads
        // waiting on is_loaded() see a fully-initialized plugin.
        self.set_loaded();
        log::debug!("Loaded plugin '{}'.", self.name);
        Ok(())
    }
}

impl std::fmt::Debug for PlugPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlugPlugin")
            .field("name", &self.name)
            .field("path", &self.path)
            .field("type", &self.plugin_type)
            .field("is_loaded", &self.is_loaded())
            .finish()
    }
}

/// Find a plugin's resource by absolute or relative path, optionally
/// verifying that the file exists. Returns empty string if plugin is None.
///
/// Matches C++ `PlugFindPluginResource(PlugPluginPtr, string, bool)`.
pub fn find_plugin_resource(plugin: Option<&PlugPlugin>, path: &str, verify: bool) -> String {
    match plugin {
        Some(p) => p.find_plugin_resource(path, verify),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_js::JsValue;

    fn make_test_plugin() -> PlugPlugin {
        let mut info = JsObject::new();

        let mut types = JsObject::new();
        let mut my_type = JsObject::new();
        my_type.insert(
            "bases".to_string(),
            JsValue::Array(vec![JsValue::from("BaseType")]),
        );
        my_type.insert("displayName".to_string(), JsValue::from("My Type"));
        types.insert("MyType".to_string(), JsValue::Object(my_type));
        info.insert("Types".to_string(), JsValue::Object(types));

        PlugPlugin::new(
            "/path/to/plugin".to_string(),
            "testPlugin".to_string(),
            "/path/to/resources".to_string(),
            info,
            PluginType::Resource,
        )
    }

    #[test]
    fn test_plugin_basic_accessors() {
        let plugin = make_test_plugin();
        assert_eq!(plugin.get_name(), "testPlugin");
        assert_eq!(plugin.get_path(), "/path/to/plugin");
        assert_eq!(plugin.get_resource_path(), "/path/to/resources");
        assert!(plugin.is_resource());
        assert!(plugin.is_loaded()); // resource plugins are always loaded
    }

    #[test]
    fn test_plugin_metadata() {
        let plugin = make_test_plugin();
        let meta = plugin.get_metadata();
        assert!(meta.contains_key("Types"));
    }

    #[test]
    fn test_plugin_metadata_for_type() {
        let plugin = make_test_plugin();
        let meta = plugin.get_metadata_for_type("MyType");
        assert!(meta.is_some());
        let dict = meta.unwrap();
        assert_eq!(
            dict.get("displayName").and_then(|v| v.as_string()),
            Some("My Type")
        );

        assert!(plugin.get_metadata_for_type("NonExistent").is_none());
    }

    #[test]
    fn test_plugin_declares_type() {
        let plugin = make_test_plugin();
        assert!(plugin.declares_type("MyType", false));
        assert!(!plugin.declares_type("BaseType", false));
        // With subclasses: BaseType appears in MyType's bases
        assert!(plugin.declares_type("BaseType", true));
    }

    #[test]
    fn test_plugin_get_declared_types() {
        let plugin = make_test_plugin();
        let types = plugin.get_declared_types();
        assert_eq!(types, vec!["MyType"]);
    }

    #[test]
    fn test_plugin_make_resource_path() {
        let plugin = make_test_plugin();
        assert_eq!(
            plugin.make_resource_path("shaders/test.glsl"),
            "/path/to/resources/shaders/test.glsl"
        );
        assert_eq!(
            plugin.make_resource_path("/absolute/path.glsl"),
            "/absolute/path.glsl"
        );
        assert_eq!(plugin.make_resource_path(""), "");
    }

    #[test]
    fn test_plugin_library_not_loaded() {
        let plugin = PlugPlugin::new(
            "/lib/path".to_string(),
            "libPlugin".to_string(),
            "/lib/resources".to_string(),
            JsObject::new(),
            PluginType::Library,
        );
        assert!(!plugin.is_loaded());
        assert!(!plugin.is_resource());
        plugin.set_loaded();
        assert!(plugin.is_loaded());
    }

    #[test]
    fn test_plugin_dependencies() {
        let mut info = JsObject::new();
        let mut deps = JsObject::new();
        deps.insert(
            "ImageFilter".to_string(),
            JsValue::Array(vec![JsValue::from("MyFilter")]),
        );
        info.insert("PluginDependencies".to_string(), JsValue::Object(deps));

        let plugin = PlugPlugin::new(
            "".to_string(),
            "test".to_string(),
            "".to_string(),
            info,
            PluginType::Resource,
        );

        let deps = plugin.get_dependencies();
        assert!(deps.contains_key("ImageFilter"));
    }

    #[test]
    fn test_load_resource_plugin() {
        // Resource plugins are immediately loaded at construction; load() is a no-op.
        let plugin = make_test_plugin(); // PluginType::Resource
        assert!(plugin.is_loaded());
        assert!(plugin.load().is_ok());
        assert!(plugin.is_loaded());
    }

    #[test]
    fn test_load_cycle_detection() {
        // Simulate a cyclic dependency by pre-seeding the seen set with our
        // own name before calling load_with_dependents.
        let plugin = PlugPlugin::new(
            "".to_string(),
            "cyclePlugin".to_string(),
            "".to_string(),
            JsObject::new(),
            PluginType::Library,
        );

        let mut seen = HashSet::new();
        seen.insert("cyclePlugin".to_string());

        let result = plugin.load_with_dependents(&mut seen);
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(msg.contains("cyclic"), "expected cycle error, got: {}", msg);
    }

    #[test]
    fn test_load_nonexistent_library() {
        // A Library plugin with a bogus path must return an error, not panic.
        let plugin = PlugPlugin::new(
            "/nonexistent/path/libfake.so".to_string(),
            "fakeLibPlugin".to_string(),
            "".to_string(),
            JsObject::new(),
            PluginType::Library,
        );
        assert!(!plugin.is_loaded());
        let result = plugin.load();
        assert!(result.is_err());
        assert!(!plugin.is_loaded());
    }
}
