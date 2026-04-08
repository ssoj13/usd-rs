//! Plugin registry for managing Hydra plugins.
//!
//! Base registry system for discovering, loading, and managing plugins
//! using manual registration. Derived registries (e.g., render delegate registry)
//! specialize this for specific plugin types.

use super::plugin_base::HfPluginBase;
use super::plugin_desc::{HfPluginDesc, HfPluginDescVector};
use super::plugin_entry::{HfPluginEntry, PluginFactoryFn};
use std::any::TypeId;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};
use usd_tf::Token;

// ============================================================================
// Static plugin auto-discovery registry
// ============================================================================

/// Record of a self-registering plugin.
pub struct HfPluginAutoEntry {
    /// Type name used as plugin ID
    pub type_name: &'static str,
    /// Human-readable display name
    pub display_name: &'static str,
    /// Plugin priority
    pub priority: i32,
    /// Factory function to instantiate the plugin
    pub factory: fn() -> Box<dyn HfPluginBase>,
    /// Concrete TypeId captured at registration time (fixes P1-3 placeholder bug).
    pub type_id: TypeId,
}

/// Global list of auto-registered plugins (filled by `register_hf_plugin!` macro).
static AUTO_REGISTRY: OnceLock<RwLock<Vec<HfPluginAutoEntry>>> = OnceLock::new();

fn auto_registry() -> &'static RwLock<Vec<HfPluginAutoEntry>> {
    AUTO_REGISTRY.get_or_init(|| RwLock::new(Vec::new()))
}

/// Register a plugin for auto-discovery.
///
/// This is the Rust-idiomatic replacement for C++ TfType / plugInfo.json discovery.
/// Call this at startup or use the `register_hf_plugin!` macro.
pub fn register_hf_plugin_auto(
    type_name: &'static str,
    display_name: &'static str,
    priority: i32,
    factory: fn() -> Box<dyn HfPluginBase>,
    type_id: TypeId,
) {
    let mut reg = auto_registry().write().expect("AUTO_REGISTRY poisoned");
    if !reg.iter().any(|e| e.type_name == type_name) {
        reg.push(HfPluginAutoEntry {
            type_name,
            display_name,
            priority,
            factory,
            type_id,
        });
    }
}

/// Macro to register an `HfPluginBase` implementation for auto-discovery.
///
/// Usage:
/// ```rust,ignore
/// register_hf_plugin!(MyRenderDelegate, "My Render Delegate", 100);
/// ```
#[macro_export]
macro_rules! register_hf_plugin {
    ($t:ty, $display_name:expr, $priority:expr) => {
        $crate::plugin_registry::register_hf_plugin_auto(
            ::std::any::type_name::<$t>(),
            $display_name,
            $priority,
            || Box::new(<$t as Default>::default()),
            ::std::any::TypeId::of::<$t>(),
        );
    };
}

// ============================================================================
// Plugin registry trait
// ============================================================================

/// Base trait for plugin registries.
///
/// Provides plugin management functionality including registration,
/// discovery, and lifecycle management. Specific registries (e.g., for
/// render delegates) should extend this and provide type-safe accessors.
pub trait HfPluginRegistry: Send + Sync {
    /// Returns an ordered list of all registered plugins.
    ///
    /// Plugins are ordered by priority (descending), then alphabetically by ID.
    fn get_plugin_descs(&self) -> HfPluginDescVector;

    /// Returns the descriptor for a specific plugin by ID.
    ///
    /// Returns None if the plugin is not registered.
    fn get_plugin_desc(&self, plugin_id: &Token) -> Option<HfPluginDesc>;

    /// Checks if a plugin with the given ID is registered.
    fn is_registered(&self, plugin_id: &Token) -> bool;

    /// Returns the plugin ID for a given plugin instance.
    ///
    /// Returns None if the instance is not found in the registry.
    fn get_plugin_id(&self, plugin: &dyn HfPluginBase) -> Option<Token>;
}

/// Concrete implementation of the plugin registry.
///
/// This provides the actual storage and management of plugin entries.
/// Use this directly or wrap it in a specialized registry type.
pub struct HfPluginRegistryImpl {
    /// Map from plugin ID (type name) to entry index
    plugin_index: RwLock<HashMap<Token, usize>>,
    /// Ordered vector of plugin entries
    plugin_entries: RwLock<Vec<HfPluginEntry>>,
}

impl HfPluginRegistryImpl {
    /// Creates a new empty plugin registry.
    pub fn new() -> Self {
        Self {
            plugin_index: RwLock::new(HashMap::new()),
            plugin_entries: RwLock::new(Vec::new()),
        }
    }

    /// Registers a plugin type with the registry.
    ///
    /// # Type Parameters
    ///
    /// * `T` - The plugin type to register (must implement HfPluginBase)
    ///
    /// # Arguments
    ///
    /// * `display_name` - Human-readable name for the plugin
    /// * `priority` - Plugin priority (higher = more preferred)
    /// * `factory` - Factory function to create plugin instances
    ///
    /// # Returns
    ///
    /// The Token ID for the registered plugin.
    pub fn register<T: HfPluginBase + 'static>(
        &self,
        display_name: &str,
        priority: i32,
        factory: PluginFactoryFn,
    ) -> Token {
        let type_id = TypeId::of::<T>();
        let type_name = std::any::type_name::<T>();

        let entry = HfPluginEntry::new(
            type_id,
            type_name.to_string(),
            display_name.to_string(),
            priority,
            factory,
        );

        let id = entry.id();
        let mut entries = self
            .plugin_entries
            .write()
            .expect("plugin_entries lock poisoned");
        let mut index = self
            .plugin_index
            .write()
            .expect("plugin_index lock poisoned");

        // Check for duplicate registration
        if index.contains_key(&id) {
            log::warn!("Plugin '{}' is already registered", id.as_str());
            return id;
        }

        let entry_idx = entries.len();
        entries.push(entry);
        index.insert(id.clone(), entry_idx);

        // Re-sort entries by priority
        entries.sort();

        // Rebuild index after sort
        index.clear();
        for (idx, entry) in entries.iter().enumerate() {
            index.insert(entry.id(), idx);
        }

        id
    }

    /// Gets a plugin instance by ID, incrementing its ref count.
    ///
    /// Creates the plugin instance if it doesn't exist yet.
    /// Caller is responsible for releasing the plugin via `release_plugin`.
    pub fn get_plugin(
        &self,
        plugin_id: &Token,
    ) -> Option<Arc<RwLock<Option<Box<dyn HfPluginBase>>>>> {
        let index = self
            .plugin_index
            .read()
            .expect("plugin_index lock poisoned");
        let entry_idx = *index.get(plugin_id)?;

        let entries = self
            .plugin_entries
            .read()
            .expect("plugin_entries lock poisoned");
        let entry = &entries[entry_idx];

        entry.inc_ref_count();
        entry.instance()
    }

    /// Increments the ref count for an existing plugin by ID.
    ///
    /// Mirrors C++ `HfPluginRegistry::AddPluginReference(HfPluginBase*)` which
    /// increments the ref count when sharing a plugin reference without calling
    /// `get_plugin()` again.
    pub fn add_plugin_reference(&self, plugin_id: &Token) {
        let index = self
            .plugin_index
            .read()
            .expect("plugin_index lock poisoned");
        if let Some(&entry_idx) = index.get(plugin_id) {
            let entries = self
                .plugin_entries
                .read()
                .expect("plugin_entries lock poisoned");
            entries[entry_idx].inc_ref_count();
        }
    }

    /// Releases a plugin instance, decrementing its ref count.
    ///
    /// When ref count reaches zero, the plugin instance is destroyed.
    pub fn release_plugin(&self, plugin_id: &Token) {
        let index = self
            .plugin_index
            .read()
            .expect("plugin_index lock poisoned");
        if let Some(&entry_idx) = index.get(plugin_id) {
            let entries = self
                .plugin_entries
                .read()
                .expect("plugin_entries lock poisoned");
            let entry = &entries[entry_idx];
            entry.dec_ref_count();
        }
    }

    /// Registers a plugin with an explicit plugin_id token (no generic T required).
    ///
    /// Used by higher-level registries (e.g. HdGpGenerativeProceduralPluginRegistry)
    /// that manage their own factory maps and just need descriptor storage.
    pub fn register_erased(&self, display_name: &str, priority: i32, id: Token) {
        let mut entries = self
            .plugin_entries
            .write()
            .expect("plugin_entries lock poisoned");
        let mut index = self
            .plugin_index
            .write()
            .expect("plugin_index lock poisoned");

        if index.contains_key(&id) {
            log::warn!("Plugin '{}' is already registered", id.as_str());
            return;
        }

        // Use a no-op factory since the caller manages construction externally.
        let factory: PluginFactoryFn = Box::new(|| panic!("use external factory"));
        let entry = HfPluginEntry::new(
            std::any::TypeId::of::<()>(),
            id.as_str().to_string(),
            display_name.to_string(),
            priority,
            factory,
        );

        let entry_idx = entries.len();
        entries.push(entry);
        index.insert(id, entry_idx);

        entries.sort();
        index.clear();
        for (idx, e) in entries.iter().enumerate() {
            index.insert(e.id(), idx);
        }
    }

    /// Discover and register all plugins that used `register_hf_plugin!` macro.
    ///
    /// Rust-idiomatic replacement for C++ `HfPluginRegistry::_DiscoverPlugins()`.
    /// Pulls from the global auto-registry and registers them into this instance.
    /// Safe to call multiple times — duplicates are silently ignored.
    pub fn discover_plugins(&self) {
        let auto_reg = auto_registry().read().expect("AUTO_REGISTRY poisoned");
        for entry in auto_reg.iter() {
            let token = Token::new(entry.type_name);
            // skip if already registered
            let already = self
                .plugin_index
                .read()
                .expect("plugin_index lock poisoned")
                .contains_key(&token);
            if already {
                continue;
            }
            // Wrap the fn pointer in a closure to match PluginFactoryFn
            let factory_fn: PluginFactoryFn = {
                let f = entry.factory;
                Box::new(move || f())
            };
            // Use the TypeId captured at registration time (fixes P1-3 placeholder bug).
            let type_id = entry.type_id;
            let e = HfPluginEntry::new(
                type_id,
                entry.type_name.to_string(),
                entry.display_name.to_string(),
                entry.priority,
                factory_fn,
            );
            let id = e.id();
            let mut entries = self
                .plugin_entries
                .write()
                .expect("plugin_entries lock poisoned");
            let mut index = self
                .plugin_index
                .write()
                .expect("plugin_index lock poisoned");
            let entry_idx = entries.len();
            entries.push(e);
            index.insert(id, entry_idx);
            // Re-sort
            entries.sort();
            index.clear();
            for (idx, ent) in entries.iter().enumerate() {
                index.insert(ent.id(), idx);
            }
        }
    }

    /// Gets the current ref count for a plugin (for testing/debugging).
    pub fn get_ref_count(&self, plugin_id: &Token) -> usize {
        let index = self
            .plugin_index
            .read()
            .expect("plugin_index lock poisoned");
        if let Some(&entry_idx) = index.get(plugin_id) {
            let entries = self
                .plugin_entries
                .read()
                .expect("plugin_entries lock poisoned");
            entries[entry_idx].ref_count()
        } else {
            0
        }
    }
}

impl Default for HfPluginRegistryImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl HfPluginRegistry for HfPluginRegistryImpl {
    fn get_plugin_descs(&self) -> HfPluginDescVector {
        let entries = self
            .plugin_entries
            .read()
            .expect("plugin_entries lock poisoned");
        entries.iter().map(|entry| entry.get_desc()).collect()
    }

    fn get_plugin_desc(&self, plugin_id: &Token) -> Option<HfPluginDesc> {
        let index = self
            .plugin_index
            .read()
            .expect("plugin_index lock poisoned");
        let entry_idx = *index.get(plugin_id)?;

        let entries = self
            .plugin_entries
            .read()
            .expect("plugin_entries lock poisoned");
        Some(entries[entry_idx].get_desc())
    }

    fn is_registered(&self, plugin_id: &Token) -> bool {
        let index = self
            .plugin_index
            .read()
            .expect("plugin_index lock poisoned");
        index.contains_key(plugin_id)
    }

    fn get_plugin_id(&self, plugin: &dyn HfPluginBase) -> Option<Token> {
        let type_name = plugin.type_name();
        let token = Token::new(type_name);

        if self.is_registered(&token) {
            Some(token)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::any::Any;

    struct MockPlugin {
        _name: String,
    }

    impl MockPlugin {
        fn new(name: &str) -> Self {
            Self {
                _name: name.to_string(),
            }
        }
    }

    impl HfPluginBase for MockPlugin {
        fn type_name(&self) -> &'static str {
            std::any::type_name::<Self>()
        }

        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    }

    struct AnotherPlugin;

    impl HfPluginBase for AnotherPlugin {
        fn type_name(&self) -> &'static str {
            std::any::type_name::<Self>()
        }

        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    }

    #[test]
    fn test_registry_creation() {
        let registry = HfPluginRegistryImpl::new();
        assert_eq!(registry.get_plugin_descs().len(), 0);
    }

    #[test]
    fn test_plugin_registration() {
        let registry = HfPluginRegistryImpl::new();

        let id = registry.register::<MockPlugin>(
            "Mock Plugin",
            100,
            Box::new(|| Box::new(MockPlugin::new("test"))),
        );

        assert!(registry.is_registered(&id));

        let desc = registry.get_plugin_desc(&id).unwrap();
        assert_eq!(desc.display_name, "Mock Plugin");
        assert_eq!(desc.priority, 100);
    }

    #[test]
    fn test_multiple_plugin_registration() {
        let registry = HfPluginRegistryImpl::new();

        let id1 = registry.register::<MockPlugin>(
            "Mock Plugin",
            50,
            Box::new(|| Box::new(MockPlugin::new("test1"))),
        );

        let id2 = registry.register::<AnotherPlugin>(
            "Another Plugin",
            100,
            Box::new(|| Box::new(AnotherPlugin)),
        );

        assert!(registry.is_registered(&id1));
        assert!(registry.is_registered(&id2));

        let descs = registry.get_plugin_descs();
        assert_eq!(descs.len(), 2);

        // C++ sorts ascending: lower numeric priority = sorts first (index 0).
        // priority=50 (Mock) < priority=100 (Another), so Mock comes first.
        assert_eq!(descs[0].priority, 50);
        assert_eq!(descs[1].priority, 100);
    }

    #[test]
    fn test_plugin_retrieval() {
        let registry = HfPluginRegistryImpl::new();

        let id = registry.register::<MockPlugin>(
            "Mock Plugin",
            100,
            Box::new(|| Box::new(MockPlugin::new("test"))),
        );

        // Get plugin instance
        let plugin = registry.get_plugin(&id);
        assert!(plugin.is_some());

        // Ref count should be 1
        assert_eq!(registry.get_ref_count(&id), 1);

        // Release plugin
        registry.release_plugin(&id);
        assert_eq!(registry.get_ref_count(&id), 0);
    }

    #[test]
    fn test_plugin_ref_counting() {
        let registry = HfPluginRegistryImpl::new();

        let id = registry.register::<MockPlugin>(
            "Mock Plugin",
            100,
            Box::new(|| Box::new(MockPlugin::new("test"))),
        );

        // Get plugin multiple times
        let _p1 = registry.get_plugin(&id);
        let _p2 = registry.get_plugin(&id);
        assert_eq!(registry.get_ref_count(&id), 2);

        // Release once
        registry.release_plugin(&id);
        assert_eq!(registry.get_ref_count(&id), 1);

        // Release again
        registry.release_plugin(&id);
        assert_eq!(registry.get_ref_count(&id), 0);
    }

    #[test]
    fn test_plugin_not_found() {
        let registry = HfPluginRegistryImpl::new();
        let fake_id = Token::new("NonExistent");

        assert!(!registry.is_registered(&fake_id));
        assert!(registry.get_plugin_desc(&fake_id).is_none());
        assert!(registry.get_plugin(&fake_id).is_none());
    }

    #[test]
    fn test_get_plugin_id_from_instance() {
        let registry = HfPluginRegistryImpl::new();

        let id = registry.register::<MockPlugin>(
            "Mock Plugin",
            100,
            Box::new(|| Box::new(MockPlugin::new("test"))),
        );

        let plugin_lock = registry.get_plugin(&id).unwrap();
        let plugin_guard = plugin_lock.read().expect("plugin lock poisoned");
        let plugin_ref = plugin_guard.as_ref().unwrap();

        let found_id = registry.get_plugin_id(plugin_ref.as_ref());
        assert_eq!(found_id, Some(id));
    }
}
