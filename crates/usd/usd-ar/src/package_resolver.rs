//! Package resolver interface for handling assets within package files.
//!
//! The `PackageResolver` trait defines the interface for resolving and opening
//! assets that are stored within package files (archives, bundles, etc.).

use std::sync::{Arc, OnceLock, RwLock};

use usd_vt::Value;

use super::asset::Asset;

/// Interface for resolving assets within package assets.
///
/// A package resolver is responsible for processing particular package asset
/// formats and resolving information about assets stored within that package.
///
/// Each package resolver is associated with particular file formats and is
/// invoked by asset resolution when handling package-relative paths involving
/// those formats. `PackageResolver` instances are only used internally by Ar
/// and are not directly exposed to clients.
///
/// # Implementing a Package Resolver
///
/// To implement a package resolver, create a type that implements this trait
/// and register it with the plugin system. The resolver should be associated
/// with specific file extensions in the plugin metadata.
///
/// # Examples
///
/// ```ignore
/// use usd_ar::{PackageResolver, Asset, ResolvedPath};
/// use std::sync::Arc;
///
/// struct CustomPackageResolver {
///     // resolver state
/// }
///
/// impl PackageResolver for CustomPackageResolver {
///     fn resolve(
///         &self,
///         resolved_package_path: &str,
///         packaged_path: &str,
///     ) -> String {
///         // Custom resolution logic
///         todo!()
///     }
///
///     fn open_asset(
///         &self,
///         resolved_package_path: &str,
///         resolved_packaged_path: &str,
///     ) -> Option<Arc<dyn Asset>> {
///         // Custom asset opening logic
///         todo!()
///     }
///
///     fn begin_cache_scope(&self, cache_scope_data: &mut Value) {
///         // Optional: initialize cache scope
///     }
///
///     fn end_cache_scope(&self, cache_scope_data: &mut Value) {
///         // Optional: cleanup cache scope
///     }
/// }
/// ```
pub trait PackageResolver: Send + Sync {
    // -------------------------------------------------------------------------
    // Packaged Path Resolution Operations
    // -------------------------------------------------------------------------

    /// Returns the resolved path for the asset located at `packaged_path`
    /// in the package specified by `resolved_package_path` if it exists.
    ///
    /// If the asset does not exist in the package, returns an empty string.
    ///
    /// When `Resolver::resolve` is invoked on a package-relative path, the
    /// path will be parsed into the outermost package path and the inner
    /// packaged path. The outermost package path will be resolved by the
    /// primary resolver. `PackageResolver::resolve` will then be called on
    /// the corresponding package resolver with that resolved path and the
    /// inner packaged path. If the inner packaged path is itself a
    /// package-relative path, this process recurses until all paths have been
    /// resolved.
    ///
    /// # Arguments
    ///
    /// * `resolved_package_path` - The resolved path to the package file
    /// * `packaged_path` - The path to the asset within the package
    ///
    /// # Returns
    ///
    /// The resolved path to the packaged asset, or an empty string if not found
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let resolver: Box<dyn PackageResolver> = get_package_resolver(".zip");
    /// let resolved = resolver.resolve(
    ///     "/path/to/package.zip",
    ///     "assets/model.usd"
    /// );
    /// if !resolved.is_empty() {
    ///     println!("Found asset at: {}", resolved);
    /// }
    /// ```
    fn resolve(&self, resolved_package_path: &str, packaged_path: &str) -> String;

    // -------------------------------------------------------------------------
    // Asset-specific Operations
    // -------------------------------------------------------------------------

    /// Returns an `Asset` object for the asset at `resolved_packaged_path`
    /// located in the package asset at `resolved_package_path`.
    ///
    /// Returns `None` if the asset could not be opened.
    ///
    /// # Arguments
    ///
    /// * `resolved_package_path` - The resolved path to the package file
    /// * `resolved_packaged_path` - The resolved path to the asset within the package
    ///
    /// # Returns
    ///
    /// An `Asset` object for reading the packaged asset, or `None` on failure
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let resolver: Box<dyn PackageResolver> = get_package_resolver(".zip");
    /// if let Some(asset) = resolver.open_asset(
    ///     "/path/to/package.zip",
    ///     "/path/to/package.zip[assets/model.usd]"
    /// ) {
    ///     let data = asset.get_buffer();
    ///     // Process asset data...
    /// }
    /// ```
    fn open_asset(
        &self,
        resolved_package_path: &str,
        resolved_packaged_path: &str,
    ) -> Option<Arc<dyn Asset>>;

    // -------------------------------------------------------------------------
    // Scoped Resolution Cache
    // -------------------------------------------------------------------------

    /// Marks the start of a resolution caching scope.
    ///
    /// This method is called when scoped resolution caches are enabled.
    /// The `cache_scope_data` parameter can be used to store resolver-specific
    /// cache data that will be passed to `end_cache_scope`.
    ///
    /// # Arguments
    ///
    /// * `cache_scope_data` - Mutable reference to cache scope data
    ///
    /// # Examples
    ///
    /// ```ignore
    /// fn begin_cache_scope(&self, cache_scope_data: &mut Value) {
    ///     // Initialize cache for this scope
    ///     *cache_scope_data = Value::from(HashMap::<String, String>::new());
    /// }
    /// ```
    fn begin_cache_scope(&self, cache_scope_data: &mut Value);

    /// Marks the end of a resolution caching scope.
    ///
    /// This method is called when a scoped resolution cache goes out of scope.
    /// Implementations should clean up any resources associated with the cache.
    ///
    /// # Arguments
    ///
    /// * `cache_scope_data` - Mutable reference to cache scope data
    ///
    /// # Examples
    ///
    /// ```ignore
    /// fn end_cache_scope(&self, cache_scope_data: &mut Value) {
    ///     // Clear cache data
    ///     *cache_scope_data = Value::empty();
    /// }
    /// ```
    fn end_cache_scope(&self, cache_scope_data: &mut Value);
}

/// Registry for package resolvers associated with file extensions.
///
/// This type manages the mapping between file extensions and their
/// corresponding package resolver implementations.
#[derive(Default)]
pub struct PackageResolverRegistry {
    /// Map from file extension to resolver factory
    resolvers:
        std::collections::HashMap<String, Box<dyn Fn() -> Box<dyn PackageResolver> + Send + Sync>>,
}

impl PackageResolverRegistry {
    /// Creates a new empty package resolver registry.
    pub fn new() -> Self {
        Self {
            resolvers: std::collections::HashMap::new(),
        }
    }

    /// Registers a package resolver for the given file extension.
    ///
    /// # Arguments
    ///
    /// * `extension` - File extension (e.g., "zip", "tar")
    /// * `factory` - Factory function that creates resolver instances
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let mut registry = PackageResolverRegistry::new();
    /// registry.register("zip", || Box::new(ZipPackageResolver::new()));
    /// ```
    pub fn register<F>(&mut self, extension: &str, factory: F)
    where
        F: Fn() -> Box<dyn PackageResolver> + Send + Sync + 'static,
    {
        self.resolvers
            .insert(extension.to_string(), Box::new(factory));
    }

    /// Gets a package resolver for the given file extension.
    ///
    /// # Arguments
    ///
    /// * `extension` - File extension to look up
    ///
    /// # Returns
    ///
    /// A new instance of the registered resolver, or `None` if not found
    pub fn get(&self, extension: &str) -> Option<Box<dyn PackageResolver>> {
        self.resolvers.get(extension).map(|factory| factory())
    }

    /// Checks if a resolver is registered for the given extension.
    pub fn has_resolver(&self, extension: &str) -> bool {
        self.resolvers.contains_key(extension)
    }

    /// Returns all registered file extensions.
    pub fn extensions(&self) -> Vec<String> {
        self.resolvers.keys().cloned().collect()
    }
}

// ── Global singleton registry ──────────────────────────────────────────────

/// Global package resolver registry instance.
static GLOBAL_REGISTRY: OnceLock<RwLock<PackageResolverRegistry>> = OnceLock::new();

/// Returns a reference to the global package resolver registry lock.
fn global_registry() -> &'static RwLock<PackageResolverRegistry> {
    GLOBAL_REGISTRY.get_or_init(|| RwLock::new(PackageResolverRegistry::new()))
}

/// Registers a package resolver factory for a file extension in the global registry.
///
/// Called by format-specific crates (e.g., usd-sdf for ".usdz") at startup.
/// Subsequent calls for the same extension are no-ops (first registration wins).
///
/// # Arguments
/// * `extension` - File extension without dot, e.g. `"usdz"`
/// * `factory` - Factory closure that creates a boxed `PackageResolver`
pub fn register_package_resolver<F>(extension: &str, factory: F)
where
    F: Fn() -> Box<dyn PackageResolver> + Send + Sync + 'static,
{
    let mut guard = global_registry()
        .write()
        .expect("global pkg registry poisoned");
    if !guard.has_resolver(extension) {
        guard.register(extension, factory);
    }
}

/// Opens an asset from a package-relative resolved path using the global registry.
///
/// Expects `resolved_path` to be a package-relative path like
/// `/abs/path/to/archive.usdz[inner/texture.png]`.
///
/// Returns `None` if no resolver is registered for the package extension or
/// if the asset is not found within the package.
pub fn open_packaged_asset(resolved_path: &str) -> Option<Arc<dyn Asset>> {
    use super::package_utils::{is_package_relative_path, split_package_relative_path_inner};

    if !is_package_relative_path(resolved_path) {
        return None;
    }

    // Recursively unwrap nested packages: archive.usdz[sub.usdz[asset.png]]
    // split_inner gives us (archive.usdz[sub.usdz], asset.png)
    let (package_path, packaged_path) = split_package_relative_path_inner(resolved_path);

    // Get the extension of the innermost package to find the right resolver
    let ext = std::path::Path::new(&package_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let resolver = {
        let guard = global_registry()
            .read()
            .expect("global pkg registry poisoned");
        guard.get(&ext)?
    };

    resolver.open_asset(&package_path, &packaged_path)
}

/// Resolves a packaged path within a package using the global registry.
///
/// Returns resolved path string (e.g. `archive.usdz[inner/texture.png]`)
/// or empty string if not found.
pub fn resolve_packaged_path(package_path: &str, packaged_path: &str) -> String {
    let ext = std::path::Path::new(package_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let guard = global_registry()
        .read()
        .expect("global pkg registry poisoned");
    if let Some(resolver) = guard.get(&ext) {
        resolver.resolve(package_path, packaged_path)
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestPackageResolver;

    impl PackageResolver for TestPackageResolver {
        fn resolve(&self, _resolved_package_path: &str, _packaged_path: &str) -> String {
            String::new()
        }

        fn open_asset(
            &self,
            _resolved_package_path: &str,
            _resolved_packaged_path: &str,
        ) -> Option<Arc<dyn Asset>> {
            None
        }

        fn begin_cache_scope(&self, _cache_scope_data: &mut Value) {}

        fn end_cache_scope(&self, _cache_scope_data: &mut Value) {}
    }

    #[test]
    fn test_registry_new() {
        let registry = PackageResolverRegistry::new();
        assert_eq!(registry.extensions().len(), 0);
    }

    #[test]
    fn test_registry_register() {
        let mut registry = PackageResolverRegistry::new();
        registry.register("test", || Box::new(TestPackageResolver));

        assert!(registry.has_resolver("test"));
        assert!(!registry.has_resolver("other"));
    }

    #[test]
    fn test_registry_get() {
        let mut registry = PackageResolverRegistry::new();
        registry.register("test", || Box::new(TestPackageResolver));

        let resolver = registry.get("test");
        assert!(resolver.is_some());

        let resolver = registry.get("other");
        assert!(resolver.is_none());
    }

    #[test]
    fn test_registry_extensions() {
        let mut registry = PackageResolverRegistry::new();
        registry.register("zip", || Box::new(TestPackageResolver));
        registry.register("tar", || Box::new(TestPackageResolver));

        let mut exts = registry.extensions();
        exts.sort();
        assert_eq!(exts, vec!["tar", "zip"]);
    }
}
