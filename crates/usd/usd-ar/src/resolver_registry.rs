//! Resolver registry for built-in URI scheme resolvers.
//!
//! This module provides a registry for resolver implementations,
//! allowing different resolvers to be registered by URI scheme at runtime.
//! All resolvers are built-in (no plugins).
//!
//! # Overview
//!
//! In C++ OpenUSD, resolvers are discovered via the TfType plugin system.
//! In Rust, we use a built-in registry that allows:
//!
//! - Registration of custom resolver factories
//! - Discovery of registered resolvers by URI scheme
//! - Fallback to default resolver
//!
//! # Example
//!
//! ```ignore
//! use usd_ar::{ResolverRegistry, Resolver, ResolverFactory};
//!
//! // Define a custom resolver
//! struct MyResolver;
//! impl Resolver for MyResolver { ... }
//!
//! // Create a factory
//! struct MyResolverFactory;
//! impl ResolverFactory for MyResolverFactory {
//!     fn create(&self) -> Box<dyn Resolver> {
//!         Box::new(MyResolver)
//!     }
//! }
//!
//! // Register it
//! ResolverRegistry::register("myscheme", Box::new(MyResolverFactory));
//! ```

use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

use super::resolver::Resolver;

/// Factory trait for creating resolver instances.
pub trait ResolverFactory: Send + Sync {
    /// Creates a new resolver instance.
    fn create(&self) -> Box<dyn Resolver>;

    /// Returns the display name of this resolver.
    fn name(&self) -> &str;

    /// Returns a description of this resolver.
    fn description(&self) -> &str {
        ""
    }
}

/// Information about a registered resolver.
#[derive(Clone)]
pub struct ResolverInfo {
    /// The URI scheme this resolver handles.
    pub scheme: String,
    /// Display name of the resolver.
    pub name: String,
    /// Description of the resolver.
    pub description: String,
    /// Whether this is the primary (default) resolver.
    pub is_primary: bool,
}

/// Global resolver registry.
static REGISTRY: OnceLock<RwLock<ResolverRegistryInner>> = OnceLock::new();

/// Internal registry storage.
struct ResolverRegistryInner {
    /// Resolver factories by URI scheme.
    factories: HashMap<String, Arc<dyn ResolverFactory>>,
    /// Info about registered resolvers.
    info: HashMap<String, ResolverInfo>,
    /// Primary resolver scheme (empty string for default).
    primary_scheme: String,
}

impl Default for ResolverRegistryInner {
    fn default() -> Self {
        Self {
            factories: HashMap::new(),
            info: HashMap::new(),
            primary_scheme: String::new(),
        }
    }
}

/// Registry for built-in resolvers by URI scheme.
///
/// Allows resolver implementations to be registered for specific
/// URI schemes (e.g., "http", "s3") or as the default/primary resolver.
/// No plugins; all schemes are built-in.
pub struct ResolverRegistry;

impl ResolverRegistry {
    /// Gets the global registry instance.
    fn instance() -> &'static RwLock<ResolverRegistryInner> {
        REGISTRY.get_or_init(|| RwLock::new(ResolverRegistryInner::default()))
    }

    /// Registers a resolver factory for the given URI scheme.
    ///
    /// # Arguments
    ///
    /// * `scheme` - The URI scheme this resolver handles (e.g., "http", "s3")
    /// * `factory` - The factory that creates resolver instances
    ///
    /// # Returns
    ///
    /// Returns true if registration succeeded, false if a resolver was
    /// already registered for this scheme.
    pub fn register(scheme: &str, factory: Box<dyn ResolverFactory>) -> bool {
        let registry = Self::instance();
        let mut guard = registry.write().expect("registry poisoned");

        if guard.factories.contains_key(scheme) {
            return false;
        }

        let info = ResolverInfo {
            scheme: scheme.to_string(),
            name: factory.name().to_string(),
            description: factory.description().to_string(),
            is_primary: false,
        };

        guard.info.insert(scheme.to_string(), info);
        guard
            .factories
            .insert(scheme.to_string(), Arc::from(factory));
        true
    }

    /// Registers a resolver as the primary (default) resolver.
    ///
    /// The primary resolver is used when no scheme-specific resolver is found.
    ///
    /// # Arguments
    ///
    /// * `factory` - The factory that creates resolver instances
    ///
    /// # Returns
    ///
    /// Returns true if registration succeeded, false if a primary resolver
    /// was already registered.
    pub fn register_primary(factory: Box<dyn ResolverFactory>) -> bool {
        let registry = Self::instance();
        let mut guard = registry.write().expect("registry poisoned");

        if !guard.primary_scheme.is_empty() {
            return false;
        }

        let scheme = "__primary__";
        let info = ResolverInfo {
            scheme: scheme.to_string(),
            name: factory.name().to_string(),
            description: factory.description().to_string(),
            is_primary: true,
        };

        guard.info.insert(scheme.to_string(), info);
        guard
            .factories
            .insert(scheme.to_string(), Arc::from(factory));
        guard.primary_scheme = scheme.to_string();
        true
    }

    /// Creates a resolver for the given URI scheme.
    ///
    /// Returns None if no resolver is registered for the scheme.
    pub fn create_resolver(scheme: &str) -> Option<Box<dyn Resolver>> {
        let registry = Self::instance();
        let guard = registry.read().expect("registry poisoned");

        guard.factories.get(scheme).map(|f| f.create())
    }

    /// Creates the primary resolver.
    ///
    /// Returns None if no primary resolver is registered.
    pub fn create_primary_resolver() -> Option<Box<dyn Resolver>> {
        let registry = Self::instance();
        let guard = registry.read().expect("registry poisoned");

        if guard.primary_scheme.is_empty() {
            return None;
        }

        guard
            .factories
            .get(&guard.primary_scheme)
            .map(|f| f.create())
    }

    /// Returns information about all registered resolvers.
    pub fn get_all_resolvers() -> Vec<ResolverInfo> {
        let registry = Self::instance();
        let guard = registry.read().expect("registry poisoned");
        guard.info.values().cloned().collect()
    }

    /// Returns information about the resolver for a given scheme.
    pub fn get_resolver_info(scheme: &str) -> Option<ResolverInfo> {
        let registry = Self::instance();
        let guard = registry.read().expect("registry poisoned");
        guard.info.get(scheme).cloned()
    }

    /// Returns all registered URI schemes.
    pub fn get_registered_schemes() -> Vec<String> {
        let registry = Self::instance();
        let guard = registry.read().expect("registry poisoned");
        guard
            .factories
            .keys()
            .filter(|k| *k != "__primary__")
            .cloned()
            .collect()
    }

    /// Checks if a resolver is registered for the given scheme.
    pub fn has_resolver(scheme: &str) -> bool {
        let registry = Self::instance();
        let guard = registry.read().expect("registry poisoned");
        guard.factories.contains_key(scheme)
    }

    /// Unregisters a resolver for the given scheme.
    ///
    /// Returns true if a resolver was unregistered.
    pub fn unregister(scheme: &str) -> bool {
        let registry = Self::instance();
        let mut guard = registry.write().expect("registry poisoned");

        guard.info.remove(scheme);
        guard.factories.remove(scheme).is_some()
    }

    /// Clears all registered resolvers.
    pub fn clear() {
        let registry = Self::instance();
        let mut guard = registry.write().expect("registry poisoned");

        guard.factories.clear();
        guard.info.clear();
        guard.primary_scheme.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset::Asset;
    use crate::asset_info::AssetInfo;
    use crate::resolved_path::ResolvedPath;
    use crate::resolver_context::ResolverContext;
    use crate::timestamp::Timestamp;
    use crate::writable_asset::{WritableAsset, WriteMode};
    use std::sync::{Arc, Mutex};
    use usd_vt::Value;

    // Global lock to serialize tests that modify registry state
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    // Mock resolver for testing
    #[allow(dead_code)]
    struct MockResolver {
        name: String, // Reserved for debug identification
    }

    impl Resolver for MockResolver {
        fn create_identifier(&self, asset_path: &str, _anchor: Option<&ResolvedPath>) -> String {
            asset_path.to_string()
        }

        fn create_identifier_for_new_asset(
            &self,
            asset_path: &str,
            _anchor: Option<&ResolvedPath>,
        ) -> String {
            asset_path.to_string()
        }

        fn resolve(&self, asset_path: &str) -> ResolvedPath {
            ResolvedPath::new(asset_path)
        }

        fn resolve_for_new_asset(&self, asset_path: &str) -> ResolvedPath {
            ResolvedPath::new(asset_path)
        }

        fn bind_context(&self, _context: &ResolverContext) -> Option<Value> {
            None
        }

        fn unbind_context(&self, _context: &ResolverContext, _binding_data: Option<Value>) {}

        fn create_default_context(&self) -> ResolverContext {
            ResolverContext::default()
        }

        fn create_default_context_for_asset(&self, _asset_path: &str) -> ResolverContext {
            ResolverContext::default()
        }

        fn create_context_from_string(&self, _context_str: &str) -> ResolverContext {
            ResolverContext::default()
        }

        fn create_context_from_string_with_scheme(
            &self,
            _uri_scheme: &str,
            _context_str: &str,
        ) -> ResolverContext {
            ResolverContext::default()
        }

        fn create_context_from_strings(
            &self,
            _context_pairs: &[(String, String)],
        ) -> ResolverContext {
            ResolverContext::default()
        }

        fn refresh_context(&self, _context: &ResolverContext) {}

        fn get_current_context(&self) -> ResolverContext {
            ResolverContext::default()
        }

        fn is_context_dependent_path(&self, _asset_path: &str) -> bool {
            false
        }

        fn get_extension(&self, asset_path: &str) -> String {
            std::path::Path::new(asset_path)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_string()
        }

        fn get_asset_info(&self, _asset_path: &str, _resolved_path: &ResolvedPath) -> AssetInfo {
            AssetInfo::default()
        }

        fn get_modification_timestamp(
            &self,
            _asset_path: &str,
            _resolved_path: &ResolvedPath,
        ) -> Timestamp {
            Timestamp::invalid()
        }

        fn open_asset(&self, _resolved_path: &ResolvedPath) -> Option<Arc<dyn Asset>> {
            None
        }

        fn open_asset_for_write(
            &self,
            _resolved_path: &ResolvedPath,
            _mode: WriteMode,
        ) -> Option<Arc<dyn WritableAsset + Send + Sync>> {
            None
        }

        fn can_write_asset_to_path(
            &self,
            _resolved_path: &ResolvedPath,
            _why_not: Option<&mut String>,
        ) -> bool {
            false
        }

        fn begin_cache_scope(&self) -> Option<Value> {
            None
        }

        fn end_cache_scope(&self, _cache_scope_data: Option<Value>) {}

        fn is_repository_path(&self, _path: &str) -> bool {
            false
        }
    }

    struct MockResolverFactory {
        name: String,
    }

    impl ResolverFactory for MockResolverFactory {
        fn create(&self) -> Box<dyn Resolver> {
            Box::new(MockResolver {
                name: self.name.clone(),
            })
        }

        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            "A mock resolver for testing"
        }
    }

    #[test]
    fn test_register_and_create() {
        let _lock = TEST_LOCK.lock().unwrap();
        ResolverRegistry::clear();

        let factory = Box::new(MockResolverFactory {
            name: "TestResolver".to_string(),
        });

        assert!(ResolverRegistry::register("test", factory));
        assert!(ResolverRegistry::has_resolver("test"));

        let resolver = ResolverRegistry::create_resolver("test");
        assert!(resolver.is_some());

        ResolverRegistry::clear();
    }

    #[test]
    fn test_duplicate_registration() {
        let _lock = TEST_LOCK.lock().unwrap();
        ResolverRegistry::clear();

        let factory1 = Box::new(MockResolverFactory {
            name: "Resolver1".to_string(),
        });
        let factory2 = Box::new(MockResolverFactory {
            name: "Resolver2".to_string(),
        });

        assert!(ResolverRegistry::register("dup", factory1));
        assert!(!ResolverRegistry::register("dup", factory2)); // Should fail

        ResolverRegistry::clear();
    }

    #[test]
    fn test_get_registered_schemes() {
        let _lock = TEST_LOCK.lock().unwrap();
        ResolverRegistry::clear();

        ResolverRegistry::register(
            "scheme1",
            Box::new(MockResolverFactory {
                name: "R1".to_string(),
            }),
        );
        ResolverRegistry::register(
            "scheme2",
            Box::new(MockResolverFactory {
                name: "R2".to_string(),
            }),
        );

        let schemes = ResolverRegistry::get_registered_schemes();
        assert_eq!(schemes.len(), 2);
        assert!(schemes.contains(&"scheme1".to_string()));
        assert!(schemes.contains(&"scheme2".to_string()));

        ResolverRegistry::clear();
    }

    #[test]
    fn test_resolver_info() {
        let _lock = TEST_LOCK.lock().unwrap();
        ResolverRegistry::clear();

        ResolverRegistry::register(
            "info_test",
            Box::new(MockResolverFactory {
                name: "InfoResolver".to_string(),
            }),
        );

        let info = ResolverRegistry::get_resolver_info("info_test");
        assert!(info.is_some());
        let info = info.unwrap();
        assert_eq!(info.scheme, "info_test");
        assert_eq!(info.name, "InfoResolver");
        assert!(!info.is_primary);

        ResolverRegistry::clear();
    }

    #[test]
    fn test_unregister() {
        let _lock = TEST_LOCK.lock().unwrap();
        ResolverRegistry::clear();

        ResolverRegistry::register(
            "unreg",
            Box::new(MockResolverFactory {
                name: "ToUnregister".to_string(),
            }),
        );

        assert!(ResolverRegistry::has_resolver("unreg"));
        assert!(ResolverRegistry::unregister("unreg"));
        assert!(!ResolverRegistry::has_resolver("unreg"));

        ResolverRegistry::clear();
    }
}
