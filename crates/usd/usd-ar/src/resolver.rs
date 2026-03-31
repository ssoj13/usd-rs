//! Asset resolver interface and default implementation.
//!
//! The resolver is responsible for resolving asset paths to physical locations.

use std::cell::RefCell;
use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};

use usd_tf::TfType;
use usd_tf::notice::send;
use usd_vt::Value;

use super::asset::Asset;
use super::asset_info::AssetInfo;
use super::filesystem_asset::FilesystemAsset;
use super::notice::ResolverChangedNotice;
use super::package_resolver::{open_packaged_asset, resolve_packaged_path};
use super::package_utils::{
    is_package_relative_path, join_package_relative_path_pair, split_package_relative_path_outer,
};
use super::resolved_path::ResolvedPath;
use super::resolver_context::{DefaultResolverContext, ResolverContext};
use super::timestamp::Timestamp;
use super::writable_asset::{FilesystemWritableAsset as FsWritableAsset, WritableAsset, WriteMode};

/// Global resolver singleton.
static RESOLVER: OnceLock<RwLock<Box<dyn Resolver>>> = OnceLock::new();

/// Preferred resolver type name (ArSetPreferredResolver). Must be set before first get_resolver.
static PREFERRED_RESOLVER: OnceLock<String> = OnceLock::new();

/// Environment variable for default search path.
const PXR_AR_DEFAULT_SEARCH_PATH: &str = "PXR_AR_DEFAULT_SEARCH_PATH";

/// Interface for asset resolution.
///
/// An asset resolver is responsible for resolving asset information
/// (including the asset's physical path) from a logical path.
///
/// # Thread Safety
///
/// Resolver implementations must be thread-safe. All methods can be
/// called from multiple threads concurrently.
pub trait Resolver: Send + Sync {
    // -------------------------------------------------------------------------
    // Identifier Operations
    // -------------------------------------------------------------------------

    /// Returns an identifier for the asset specified by `asset_path`.
    ///
    /// If `anchor_asset_path` is not empty, it is the resolved asset path
    /// that `asset_path` should be anchored to if it is a relative path.
    ///
    /// # Arguments
    ///
    /// * `asset_path` - The asset path to create an identifier for
    /// * `anchor_asset_path` - Optional anchor for relative paths
    fn create_identifier(
        &self,
        asset_path: &str,
        anchor_asset_path: Option<&ResolvedPath>,
    ) -> String;

    /// Returns an identifier for a new asset at the given path.
    ///
    /// Similar to `create_identifier` but for assets that may not exist yet.
    fn create_identifier_for_new_asset(
        &self,
        asset_path: &str,
        anchor_asset_path: Option<&ResolvedPath>,
    ) -> String;

    // -------------------------------------------------------------------------
    // Resolution Operations
    // -------------------------------------------------------------------------

    /// Returns the resolved path for the asset identified by `asset_path`.
    ///
    /// Returns an empty `ResolvedPath` if the asset does not exist.
    ///
    /// # Arguments
    ///
    /// * `asset_path` - The asset path to resolve
    fn resolve(&self, asset_path: &str) -> ResolvedPath;

    /// Returns the resolved path for a new asset.
    ///
    /// Note that an asset might or might not already exist at the returned path.
    fn resolve_for_new_asset(&self, asset_path: &str) -> ResolvedPath;

    // -------------------------------------------------------------------------
    // Context Operations
    // -------------------------------------------------------------------------

    /// Binds the given context to this resolver.
    ///
    /// Returns binding data that should be passed to `unbind_context`.
    fn bind_context(&self, context: &ResolverContext) -> Option<Value>;

    /// Unbinds the given context from this resolver.
    fn unbind_context(&self, context: &ResolverContext, binding_data: Option<Value>);

    /// Returns a default context that may be bound to this resolver.
    fn create_default_context(&self) -> ResolverContext;

    /// Returns a context for resolving the asset at the given path.
    fn create_default_context_for_asset(&self, asset_path: &str) -> ResolverContext;

    /// Creates a context from the given string.
    fn create_context_from_string(&self, context_str: &str) -> ResolverContext;

    /// Creates a context from the given string using the resolver registered
    /// for the given URI/IRI scheme.
    ///
    /// If `uri_scheme` is empty, uses the primary resolver (equivalent to
    /// `create_context_from_string(context_str)`).
    ///
    /// If no resolver is registered for `uri_scheme`, returns an empty context.
    fn create_context_from_string_with_scheme(
        &self,
        uri_scheme: &str,
        context_str: &str,
    ) -> ResolverContext;

    /// Creates a context by combining contexts created from multiple
    /// URI/IRI scheme and context string pairs.
    ///
    /// Each entry in `context_strings` is a pair of `(uri_scheme, context_str)`.
    /// An empty `uri_scheme` indicates the primary resolver.
    ///
    /// # Arguments
    ///
    /// * `context_strings` - Vector of `(uri_scheme, context_str)` pairs
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::{Resolver, get_resolver};
    ///
    /// let resolver = get_resolver().read().expect("rwlock poisoned");
    /// let contexts = vec![
    ///     ("".to_string(), "path1:path2".to_string()),
    ///     ("my-scheme".to_string(), "path3:path4".to_string()),
    /// ];
    /// let ctx = resolver.create_context_from_strings(&contexts);
    /// ```
    fn create_context_from_strings(&self, context_strings: &[(String, String)]) -> ResolverContext;

    /// Refreshes caches associated with the given context.
    fn refresh_context(&self, context: &ResolverContext);

    /// Returns the currently bound context.
    fn get_current_context(&self) -> ResolverContext;

    /// Returns true if the asset path is context-dependent.
    fn is_context_dependent_path(&self, asset_path: &str) -> bool;

    // -------------------------------------------------------------------------
    // File/Asset Operations
    // -------------------------------------------------------------------------

    /// Returns the file extension for the given asset path.
    fn get_extension(&self, asset_path: &str) -> String;

    /// Returns additional metadata about the asset.
    fn get_asset_info(&self, asset_path: &str, resolved_path: &ResolvedPath) -> AssetInfo;

    /// Returns the modification timestamp for the asset.
    fn get_modification_timestamp(
        &self,
        asset_path: &str,
        resolved_path: &ResolvedPath,
    ) -> Timestamp;

    /// Opens the asset at the given resolved path for reading.
    fn open_asset(&self, resolved_path: &ResolvedPath) -> Option<Arc<dyn Asset>>;

    /// Opens the asset at the given resolved path for writing.
    fn open_asset_for_write(
        &self,
        resolved_path: &ResolvedPath,
        write_mode: WriteMode,
    ) -> Option<Arc<dyn WritableAsset + Send + Sync>>;

    /// Returns true if an asset may be written to the given path.
    ///
    /// If this function returns false and `why_not` is not None, it may be
    /// filled with an explanation.
    fn can_write_asset_to_path(
        &self,
        resolved_path: &ResolvedPath,
        why_not: Option<&mut String>,
    ) -> bool;

    // -------------------------------------------------------------------------
    // Cache Operations
    // -------------------------------------------------------------------------

    /// Marks the start of a resolution caching scope.
    fn begin_cache_scope(&self) -> Option<Value>;

    /// Marks the start of a resolution caching scope, sharing parent data.
    ///
    /// Matches C++ `ArResolver::BeginCacheScope(ArResolverCacheData*)` when
    /// called from `ArResolverScopedCache(const ArResolverScopedCache* parent)`.
    /// The parent's cache scope data is passed in so the resolver can share
    /// the parent's cache for the new scope.
    ///
    /// Default implementation delegates to `begin_cache_scope()` ignoring
    /// the parent data.
    fn begin_cache_scope_with_parent(&self, _parent_data: &Value) -> Option<Value> {
        self.begin_cache_scope()
    }

    /// Marks the end of a resolution caching scope.
    fn end_cache_scope(&self, cache_scope_data: Option<Value>);

    // -------------------------------------------------------------------------
    // Deprecated APIs
    // -------------------------------------------------------------------------

    /// \deprecated
    /// Returns true if the given path is a repository path.
    fn is_repository_path(&self, path: &str) -> bool;
}

/// Returns the configured asset resolver.
///
/// Matches C++ `ArGetResolver()`. When first called, the resolver is chosen as follows:
///
/// 1. If [`set_preferred_resolver`] was called, that type is used (falls back to
///    [`DefaultResolver`] if the type cannot be instantiated).
/// 2. Otherwise, resolvers are discovered via the TfType registry. The list is
///    sorted by type name; the first available resolver is selected.
/// 3. On error, a [`DefaultResolver`] is constructed.
///
/// The resolver is cached and used for all subsequent calls.
///
/// # Examples
///
/// ```
/// use usd_ar::get_resolver;
///
/// let resolver = get_resolver();
/// let path = resolver.read().expect("rwlock poisoned").resolve("/path/to/asset.usd");
/// ```
pub fn get_resolver() -> &'static RwLock<Box<dyn Resolver>> {
    RESOLVER.get_or_init(|| {
        // C++ ArGetResolver() creates a _DispatchingResolver singleton.
        // The DispatchingResolver discovers primary + URI resolvers via
        // TfType/PlugRegistry and dispatches all calls to the appropriate one.
        let resolver: Box<dyn Resolver> =
            Box::new(super::dispatching_resolver::DispatchingResolver::new());
        RwLock::new(resolver)
    })
}

/// Sets the preferred resolver type used by [`get_resolver`].
///
/// Matches C++ `ArSetPreferredResolver`. Overrides the default resolver discovery
/// and forces use of the specified type (e.g. `"ArDefaultResolver"`).
///
/// If the type cannot be found or instantiated, [`get_resolver`] will fall back
/// to [`DefaultResolver`].
///
/// Returns an error string if called after [`get_resolver`] has already been called,
/// matching C++ behavior where `TF_CODING_ERROR` is issued but execution continues.
///
/// # Example
///
/// Must be called before any call to [`get_resolver`]:
///
/// ```ignore
/// use usd_ar::{get_resolver, set_preferred_resolver};
///
/// set_preferred_resolver("ArDefaultResolver").expect("must call before get_resolver");
/// let resolver = get_resolver().read();
/// ```
pub fn set_preferred_resolver(resolver_type_name: &str) -> Result<(), String> {
    // Matches C++ TF_CODING_ERROR: report error but don't crash
    if RESOLVER.get().is_some() {
        return Err(
            "ArSetPreferredResolver: cannot set preferred resolver after \
             get_resolver has been called"
                .to_string(),
        );
    }
    let _ = PREFERRED_RESOLVER.set(resolver_type_name.to_string());
    Ok(())
}

/// Directly injects a resolver instance.
///
/// Use for testing or when providing a custom resolver without TfType registration.
/// Not present in C++ Ar API; equivalent to bypassing plugin discovery.
///
/// Returns an error string if called after [`get_resolver`] has already been called,
/// matching C++ behavior where `TF_CODING_ERROR` is issued but execution continues.
pub fn set_resolver(resolver: Box<dyn Resolver>) -> Result<(), String> {
    // Matches C++ TF_CODING_ERROR: report error but don't crash
    if RESOLVER.get().is_some() {
        return Err("ArSetResolver: cannot set resolver after initialization".to_string());
    }
    let _ = RESOLVER.set(RwLock::new(resolver));
    Ok(())
}

/// Returns the preferred resolver type name, if set via `set_preferred_resolver`.
/// Used internally by `DispatchingResolver::init_primary`.
pub(crate) fn get_preferred_resolver_name() -> Option<String> {
    PREFERRED_RESOLVER.get().cloned()
}

/// Returns the underlying resolver instance used by [`get_resolver`].
///
/// Matches C++ `ArGetUnderlyingResolver`. Returns `Some` only after [`get_resolver`]
/// has been called at least once. Due to Rust's ownership model, this returns the
/// resolver's `RwLock` directly rather than a reference to the inner `Box<dyn Resolver>`.
///
/// # Warning
///
/// Typically not needed. Use [`get_resolver`] for asset resolution.
pub fn get_underlying_resolver() -> Option<&'static RwLock<Box<dyn Resolver>>> {
    RESOLVER.get()
}

/// Returns the list of available resolver TfTypes.
///
/// Matches C++ `ArGetAvailableResolvers`. Resolvers are discovered via the TfType
/// registry (types deriving from `ArResolver`). Sorted by type name.
///
/// # Warning
///
/// Advanced API. Use [`get_resolver`] for normal asset resolution.
pub fn get_available_resolvers() -> Vec<TfType> {
    super::define_resolver::get_all_resolver_types()
}

/// Returns the list of URI schemes for which a resolver has been registered.
///
/// Matches C++ `ArGetRegisteredURISchemes`. Schemes are lower-case and sorted.
/// Reads from built-in [`ResolverRegistry`]; no plugins.
///
/// [`ResolverRegistry`]: super::ResolverRegistry
pub fn get_registered_uri_schemes() -> Vec<String> {
    let mut schemes = super::resolver_registry::ResolverRegistry::get_registered_schemes();
    schemes.iter_mut().for_each(|s| *s = s.to_lowercase());
    schemes.sort();
    schemes
}

/// Constructs a new resolver instance of the given TfType.
///
/// Matches C++ `ArCreateResolver`. Loads the type from the registry and constructs
/// a new instance. If the type is unknown or construction fails, returns a
/// [`DefaultResolver`].
///
/// Does *not* change the resolver used by [`get_resolver`].
///
/// # Warning
///
/// Advanced API. Use [`get_resolver`] for normal asset resolution.
///
/// # Example
///
/// ```
/// use usd_ar::create_resolver;
/// use usd_tf::TfType;
///
/// let t = TfType::find_by_name("ArDefaultResolver");
/// let resolver = create_resolver(t);
/// ```
pub fn create_resolver(resolver_type: TfType) -> Box<dyn Resolver> {
    super::define_resolver::create_resolver_by_type(resolver_type)
        .unwrap_or_else(|| Box::new(DefaultResolver::new()))
}

// Thread-local cache stack for resolution caching.
thread_local! {
    static CACHE_STACK: RefCell<Vec<Arc<RwLock<HashMap<String, ResolvedPath>>>>> = const { RefCell::new(Vec::new()) };
}

// Thread-local context stack for resolver contexts.
// Matches C++ ArResolver::GetCurrentContext() which uses thread-local stack.
thread_local! {
    static CONTEXT_STACK: RefCell<Vec<ResolverContext>> = const { RefCell::new(Vec::new()) };
}

/// Default asset resolver implementation.
///
/// Matches C++ `ArDefaultResolver`. Provides file-based resolution with search paths:
/// current working directory, [`DefaultResolverContext`] paths, environment variable
/// `PXR_AR_DEFAULT_SEARCH_PATH`, and [`set_default_search_path_static`].
///
/// [`DefaultResolverContext`]: super::DefaultResolverContext
/// [`set_default_search_path_static`]: DefaultResolver::set_default_search_path_static
pub struct DefaultResolver {
    /// Default search paths.
    default_search_path: RwLock<Vec<String>>,
}

/// Global default search path (static, shared across all DefaultResolver instances)
static GLOBAL_DEFAULT_SEARCH_PATH: OnceLock<RwLock<Vec<String>>> = OnceLock::new();

impl DefaultResolver {
    /// Creates a new default resolver.
    ///
    /// The initial search path is read from the `PXR_AR_DEFAULT_SEARCH_PATH`
    /// environment variable if set.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::DefaultResolver;
    ///
    /// let resolver = DefaultResolver::new();
    /// ```
    pub fn new() -> Self {
        let search_path = Self::read_search_path_from_env();
        // Initialize global search path if not already set
        GLOBAL_DEFAULT_SEARCH_PATH.get_or_init(|| RwLock::new(search_path.clone()));
        Self {
            default_search_path: RwLock::new(search_path),
        }
    }

    /// Sets the default search path for asset resolution (instance method).
    ///
    /// This overwrites any path specified via the environment variable.
    ///
    /// # Arguments
    ///
    /// * `search_path` - List of directories to search
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::DefaultResolver;
    ///
    /// let resolver = DefaultResolver::new();
    /// resolver.set_default_search_path(vec!["/assets".into(), "/backup".into()]);
    /// ```
    pub fn set_default_search_path(&self, search_path: Vec<String>) {
        if let Ok(mut paths) = self.default_search_path.write() {
            *paths = search_path;
        }
    }

    /// Sets the default search path that will be used during asset resolution (static method).
    ///
    /// Calling this function will trigger a ResolverChanged notification to be sent
    /// if the search path differs from the currently set default value.
    ///
    /// The initial search path may be specified using via the environment
    /// variable PXR_AR_DEFAULT_SEARCH_PATH. Calling this function will
    /// override any path specified in this manner.
    ///
    /// This function is not thread-safe and should not be called concurrently
    /// with any other ArResolver operations.
    ///
    /// # Arguments
    ///
    /// * `search_path` - List of directories to search
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::DefaultResolver;
    ///
    /// DefaultResolver::set_default_search_path_static(vec!["/assets".into(), "/backup".into()]);
    /// ```
    pub fn set_default_search_path_static(search_path: Vec<String>) {
        let global_path = GLOBAL_DEFAULT_SEARCH_PATH.get_or_init(|| RwLock::new(Vec::new()));

        // Check if the path differs from current
        let mut changed = false;
        if let Ok(current) = global_path.read() {
            changed = *current != search_path;
        }

        // Update global search path
        if let Ok(mut paths) = global_path.write() {
            *paths = search_path;
        }

        // Send notice if changed
        if changed {
            let notice = ResolverChangedNotice::with_filter(|ctx| {
                ctx.get::<DefaultResolverContext>().is_some()
            });
            send(&notice);
        }
    }

    /// Returns the current default search path.
    pub fn get_default_search_path(&self) -> Vec<String> {
        self.default_search_path
            .read()
            .map(|p| p.clone())
            .unwrap_or_default()
    }

    /// Reads search path from environment variable.
    fn read_search_path_from_env() -> Vec<String> {
        env::var(PXR_AR_DEFAULT_SEARCH_PATH)
            .ok()
            .map(|path| {
                let sep = if cfg!(windows) { ';' } else { ':' };
                path.split(sep)
                    .filter(|s| !s.is_empty())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Resolves an asset path using the search paths.
    fn resolve_with_search_paths(&self, asset_path: &str) -> Option<PathBuf> {
        let path = Path::new(asset_path);

        // If absolute, check if it exists
        if !is_relative_path(asset_path) {
            return if path.exists() {
                Some(path.to_path_buf())
            } else {
                None
            };
        }

        // 1. Try cwd first (C++ defaultResolver.cpp:196-200)
        if let Ok(cwd) = env::current_dir() {
            let full = cwd.join(path);
            if full.exists() {
                return Some(full);
            }
        }

        // 2. Search paths only for search-style paths (not ./foo or ../bar)
        // C++ defaultResolver.cpp:202: if (_IsSearchPath(path))
        if !is_search_path(asset_path) {
            return None;
        }

        // 3. Context search paths (C++ defaultResolver.cpp:204-213)
        let ctx = self.get_current_context();
        if let Some(default_ctx) = ctx.get::<DefaultResolverContext>() {
            for p in default_ctx.search_paths() {
                let full = PathBuf::from(p).join(path);
                if full.exists() {
                    return Some(full);
                }
            }
        }

        // 4. Default search paths (C++ defaultResolver.cpp:214-222)
        if let Some(global_paths) = GLOBAL_DEFAULT_SEARCH_PATH.get() {
            if let Ok(paths) = global_paths.read() {
                for p in paths.iter() {
                    let full = PathBuf::from(p).join(path);
                    if full.exists() {
                        return Some(full);
                    }
                }
            }
        }

        // 5. Instance-specific default search paths
        if let Ok(paths) = self.default_search_path.read() {
            for p in paths.iter() {
                let full = PathBuf::from(p).join(path);
                if full.exists() {
                    return Some(full);
                }
            }
        }

        None
    }

    /// Creates an identifier by anchoring a relative path.
    /// Matches C++ `_AnchorRelativePath` in defaultResolver.cpp.
    fn anchor_relative_path(&self, asset_path: &str, anchor: Option<&ResolvedPath>) -> String {
        let path = Path::new(asset_path);

        // If absolute (not relative), normalize and return.
        // Uses is_relative_path which handles Unix-style '/' on Windows.
        if !is_relative_path(asset_path) {
            return normalize_path(path);
        }

        // If we have an anchor, use it
        if let Some(anchor) = anchor {
            if !anchor.is_empty() {
                let anchor_path = Path::new(anchor.as_str());
                if let Some(parent) = anchor_path.parent() {
                    let anchored = parent.join(path);
                    return normalize_path(&anchored);
                }
            }
        }

        // Otherwise, try to make it absolute from cwd
        if let Ok(cwd) = env::current_dir() {
            let full = cwd.join(path);
            return normalize_path(&full);
        }

        asset_path.to_string()
    }

    /// Resolve a packaged path chain through nested package resolvers.
    ///
    /// Given a resolved outer package path (e.g. `/abs/archive.usdz`) and an
    /// inner path that may itself be package-relative (e.g. `sub.usdz[asset.png]`),
    /// iterates through each nesting level using the registered package resolvers.
    /// Returns the fully-resolved package-relative path or empty string on failure.
    fn resolve_packaged_path_chain(&self, resolved_package: &str, packaged_path: &str) -> String {
        let mut current_package = resolved_package.to_string();
        let mut remaining = packaged_path.to_string();

        loop {
            // Split off the outermost packaged segment
            let (inner_pkg, inner_rest) = if is_package_relative_path(&remaining) {
                split_package_relative_path_outer(&remaining)
            } else {
                (remaining.clone(), String::new())
            };

            // Use the registered package resolver for current_package's extension
            let resolved_inner = resolve_packaged_path(&current_package, &inner_pkg);
            if resolved_inner.is_empty() {
                return String::new();
            }

            // Build the new package-relative path: current_package[resolved_inner]
            let joined = join_package_relative_path_pair(&current_package, &resolved_inner);

            if inner_rest.is_empty() {
                return joined;
            }

            // Descend: the new current package is the joined path so far
            current_package = joined;
            remaining = inner_rest;
        }
    }
}

impl Default for DefaultResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl Resolver for DefaultResolver {
    /// C++ resolver.cpp:541-582 `_CreateIdentifierHelper`:
    /// Handles package-relative paths by splitting outer/inner, creating
    /// identifier for outer only, then rejoining.
    fn create_identifier(
        &self,
        asset_path: &str,
        anchor_asset_path: Option<&ResolvedPath>,
    ) -> String {
        if asset_path.is_empty() {
            return asset_path.to_string();
        }

        // Handle package-relative asset paths
        if crate::package_utils::is_package_relative_path(asset_path) {
            let (outer, inner) =
                crate::package_utils::split_package_relative_path_inner(asset_path);
            let outer_id = self.create_identifier(&outer, anchor_asset_path);
            return format!("{}[{}]", outer_id, inner);
        }

        // No anchor: just normalize the path
        let anchor = match anchor_asset_path {
            Some(a) if !a.is_empty() => a,
            _ => return normalize_path(Path::new(asset_path)),
        };

        // C++ resolver.cpp:569-570: extract outer package from anchor if package-relative
        let effective_anchor = if crate::package_utils::is_package_relative_path(anchor.as_str()) {
            let (outer, _) =
                crate::package_utils::split_package_relative_path_inner(anchor.as_str());
            ResolvedPath::new(outer)
        } else {
            anchor.clone()
        };

        let anchored = self.anchor_relative_path(asset_path, Some(&effective_anchor));

        // Look-here-first semantics for search paths:
        // If assetPath is a search path, we want to preserve it as the identifier
        // (instead of the anchored absolute path) so that the search-path mechanism
        // is always invoked on re-resolution. But first, check if the anchored path
        // actually resolves - if it does, use the anchored path; otherwise keep the
        // search path as-is.
        if is_search_path(asset_path) {
            let resolved = self.resolve(&anchored);
            if resolved.is_empty() {
                return normalize_path(Path::new(asset_path));
            }
        }

        normalize_path(Path::new(&anchored))
    }

    /// C++ resolver.cpp:528-539: same as create_identifier but for new assets.
    fn create_identifier_for_new_asset(
        &self,
        asset_path: &str,
        anchor_asset_path: Option<&ResolvedPath>,
    ) -> String {
        if asset_path.is_empty() {
            return asset_path.to_string();
        }

        // Handle package-relative asset paths
        if crate::package_utils::is_package_relative_path(asset_path) {
            let (outer, inner) =
                crate::package_utils::split_package_relative_path_inner(asset_path);
            let outer_id = self.create_identifier_for_new_asset(&outer, anchor_asset_path);
            return format!("{}[{}]", outer_id, inner);
        }

        let path = Path::new(asset_path);
        if is_relative_path(asset_path) {
            // For new assets, always anchor relative paths
            if let Some(anchor) = anchor_asset_path {
                if !anchor.is_empty() {
                    return normalize_path(
                        &Path::new(anchor.as_str())
                            .parent()
                            .unwrap_or(Path::new(""))
                            .join(path),
                    );
                }
            }
            // No anchor - make absolute from cwd
            if let Ok(cwd) = env::current_dir() {
                return normalize_path(&cwd.join(path));
            }
        }

        normalize_path(path)
    }

    fn resolve(&self, asset_path: &str) -> ResolvedPath {
        // Handle package-relative paths (e.g. "archive.usdz[inner/texture.png]").
        // Resolve the outermost package path first via the primary resolver,
        // then use registered package resolvers for each nesting level.
        // Matches C++ _ResolveHelper logic in resolver.cpp.
        if is_package_relative_path(asset_path) {
            let (pkg_path, packaged_path) = split_package_relative_path_outer(asset_path);

            // Resolve the outer package path (the .usdz file on disk)
            let resolved_pkg = self.resolve(&pkg_path);
            if resolved_pkg.is_empty() {
                return ResolvedPath::empty();
            }

            // Recursively resolve inner packaged path segments via package resolvers
            let resolved_inner =
                self.resolve_packaged_path_chain(resolved_pkg.as_str(), &packaged_path);
            if resolved_inner.is_empty() {
                return ResolvedPath::empty();
            }

            return ResolvedPath::new(resolved_inner);
        }

        // Check cache if caching is active
        let cached = CACHE_STACK.with(|stack| {
            let stack_ref = stack.borrow();
            if let Some(cache) = stack_ref.last() {
                if let Ok(cache_map) = cache.read() {
                    cache_map.get(asset_path).cloned()
                } else {
                    None
                }
            } else {
                None
            }
        });

        if let Some(cached_path) = cached {
            return cached_path;
        }

        // Resolve and cache result
        let resolved = self
            .resolve_with_search_paths(asset_path)
            .map(|p| ResolvedPath::new(p.to_string_lossy().into_owned()))
            .unwrap_or_else(ResolvedPath::empty);

        // Store in cache if caching is active
        CACHE_STACK.with(|stack| {
            let stack_ref = stack.borrow();
            if let Some(cache) = stack_ref.last() {
                if let Ok(mut cache_map) = cache.write() {
                    cache_map.insert(asset_path.to_string(), resolved.clone());
                }
            }
        });

        resolved
    }

    /// C++ resolver.cpp:844-855 `_ResolveForNewAsset`:
    /// Handles package-relative paths by splitting outer/inner, resolving
    /// only the outer part, then rejoining. Without this, a path like
    /// "archive.usdz[inner.usd]" would be treated as a single filesystem path.
    fn resolve_for_new_asset(&self, asset_path: &str) -> ResolvedPath {
        // Handle package-relative paths: split, resolve outer, rejoin
        if crate::package_utils::is_package_relative_path(asset_path) {
            let (outer, inner) =
                crate::package_utils::split_package_relative_path_inner(asset_path);
            let resolved_outer = self.resolve_for_new_asset(&outer);
            if resolved_outer.is_empty() {
                return ResolvedPath::empty();
            }
            return ResolvedPath::new(format!("{}[{}]", resolved_outer.as_str(), inner));
        }

        let path = Path::new(asset_path);

        if !is_relative_path(asset_path) {
            return ResolvedPath::new(normalize_path(path));
        }

        if let Ok(cwd) = env::current_dir() {
            let full = cwd.join(path);
            return ResolvedPath::new(normalize_path(&full));
        }

        ResolvedPath::empty()
    }

    fn bind_context(&self, context: &ResolverContext) -> Option<Value> {
        // Push context onto thread-local stack
        CONTEXT_STACK.with(|stack| {
            stack.borrow_mut().push(context.clone());
        });
        None
    }

    fn unbind_context(&self, _context: &ResolverContext, _binding_data: Option<Value>) {
        // Pop context from thread-local stack
        CONTEXT_STACK.with(|stack| {
            let mut stack_ref = stack.borrow_mut();
            if !stack_ref.is_empty() {
                stack_ref.pop();
            }
        });
    }

    fn create_default_context(&self) -> ResolverContext {
        // Use global static search path if available, otherwise instance-specific
        let search_paths = if let Some(global_paths) = GLOBAL_DEFAULT_SEARCH_PATH.get() {
            if let Ok(paths) = global_paths.read() {
                paths.clone()
            } else {
                self.get_default_search_path()
            }
        } else {
            self.get_default_search_path()
        };

        ResolverContext::with_object(DefaultResolverContext::new(search_paths))
    }

    fn create_default_context_for_asset(&self, asset_path: &str) -> ResolverContext {
        // C++ defaultResolver.cpp:276-288: context contains ONLY asset directory
        if asset_path.is_empty() {
            return ResolverContext::with_object(DefaultResolverContext::empty());
        }

        let path = Path::new(asset_path);
        let abs_path = if path.is_relative() {
            env::current_dir()
                .map(|cwd| cwd.join(path))
                .unwrap_or_else(|_| path.to_path_buf())
        } else {
            path.to_path_buf()
        };

        let mut search_paths = Vec::new();
        if let Some(parent) = abs_path.parent() {
            let parent_str = parent.to_string_lossy().into_owned();
            if !parent_str.is_empty() {
                search_paths.push(parent_str);
            }
        }

        ResolverContext::with_object(DefaultResolverContext::new(search_paths))
    }

    fn create_context_from_string(&self, context_str: &str) -> ResolverContext {
        let sep = if cfg!(windows) { ';' } else { ':' };
        let search_paths: Vec<String> = context_str
            .split(sep)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();

        ResolverContext::with_object(DefaultResolverContext::new(search_paths))
    }

    fn create_context_from_string_with_scheme(
        &self,
        uri_scheme: &str,
        context_str: &str,
    ) -> ResolverContext {
        // Scheme-aware context creation
        // For empty scheme (primary resolver), use default resolver
        if uri_scheme.is_empty() {
            self.create_context_from_string(context_str)
        } else {
            // Note: Full impl would look up URI scheme resolver via plugin registry
            // and create scheme-specific context. Returns empty context without plugins.
            ResolverContext::new()
        }
    }

    fn create_context_from_strings(&self, context_strings: &[(String, String)]) -> ResolverContext {
        // Match C++ CreateContextFromStrings implementation:
        // Creates a vector of ArResolverContext objects and passes them to
        // ArResolverContext constructor (matches resolver.cpp:499-514)
        let mut contexts = Vec::new();

        for (uri_scheme, context_str) in context_strings {
            let ctx = self.create_context_from_string_with_scheme(uri_scheme, context_str);
            if !ctx.is_empty() {
                contexts.push(ctx);
            }
        }

        // Match C++: return ArResolverContext(contexts)
        ResolverContext::from_contexts(contexts)
    }

    fn refresh_context(&self, _context: &ResolverContext) {
        // C++ default _RefreshContext is a noop.
        // Only custom resolver subclasses send notices when context changes.
    }

    fn get_current_context(&self) -> ResolverContext {
        // Get top context from thread-local stack
        CONTEXT_STACK.with(|stack| {
            let stack_ref = stack.borrow();
            stack_ref.last().cloned().unwrap_or_default()
        })
    }

    fn is_context_dependent_path(&self, asset_path: &str) -> bool {
        // Only search-path style relative paths are context-dependent.
        // File-relative paths (./ or ../) are NOT context-dependent because
        // they always resolve relative to their anchor, not via search paths.
        // This matches C++ ArDefaultResolver::_IsContextDependentPath which
        // calls _IsSearchPath(assetPath).
        is_search_path(asset_path)
    }

    fn get_extension(&self, asset_path: &str) -> String {
        Path::new(asset_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_string()
    }

    fn get_asset_info(&self, _asset_path: &str, _resolved_path: &ResolvedPath) -> AssetInfo {
        AssetInfo::new()
    }

    fn get_modification_timestamp(
        &self,
        _asset_path: &str,
        resolved_path: &ResolvedPath,
    ) -> Timestamp {
        if resolved_path.is_empty() {
            return Timestamp::invalid();
        }

        let path = Path::new(resolved_path.as_str());
        path.metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .map(Timestamp::from_system_time)
            .unwrap_or_else(Timestamp::invalid)
    }

    fn open_asset(&self, resolved_path: &ResolvedPath) -> Option<Arc<dyn Asset>> {
        if resolved_path.is_empty() {
            return None;
        }

        let path_str = resolved_path.as_str();

        // Package-relative path (e.g. `/abs/archive.usdz[inner/texture.png]`):
        // delegate to the registered package resolver. Matches C++ _OpenAsset
        // which calls packageResolver->OpenAsset(outerPkg, innerPath).
        if is_package_relative_path(path_str) {
            return open_packaged_asset(path_str);
        }

        // Plain filesystem path: use FilesystemAsset for streaming reads.
        // Matches C++ ArFilesystemAsset::Open(resolvedPath).
        FilesystemAsset::open_resolved(resolved_path).map(|asset| Arc::new(asset) as Arc<dyn Asset>)
    }

    fn open_asset_for_write(
        &self,
        resolved_path: &ResolvedPath,
        write_mode: WriteMode,
    ) -> Option<Arc<dyn WritableAsset + Send + Sync>> {
        if resolved_path.is_empty() {
            return None;
        }

        // Use the dedicated FilesystemWritableAsset from ar::writable_asset.
        // This matches C++ ArDefaultResolver::_OpenAssetForWrite which
        // returns ArFilesystemWritableAsset::Create(resolvedPath, mode).
        let asset = FsWritableAsset::create(resolved_path, write_mode)?;

        // FsWritableAsset::create returns Arc<Mutex<FsWritableAsset>>.
        // We need Arc<dyn WritableAsset + Send + Sync>.
        // Wrap in an adapter.
        Some(Arc::new(MutexWritableAssetAdapter(asset)))
    }

    fn can_write_asset_to_path(
        &self,
        resolved_path: &ResolvedPath,
        why_not: Option<&mut String>,
    ) -> bool {
        // C++ default _CanWriteAssetToPath just returns true.
        // No filesystem side effects.
        if resolved_path.is_empty() {
            if let Some(why) = why_not {
                *why = "Empty resolved path".to_string();
            }
            return false;
        }
        true
    }

    fn is_repository_path(&self, _path: &str) -> bool {
        // Default implementation returns false (deprecated API)
        false
    }

    fn begin_cache_scope(&self) -> Option<Value> {
        // Push a new cache onto the thread-local cache stack
        CACHE_STACK.with(|stack| {
            let mut stack_ref = stack.borrow_mut();
            let cache = Arc::new(RwLock::new(HashMap::new()));
            stack_ref.push(cache);
            // Return cache scope depth as Value
            Some(Value::from(stack_ref.len() as i64))
        })
    }

    fn end_cache_scope(&self, _cache_scope_data: Option<Value>) {
        CACHE_STACK.with(|stack| {
            let mut stack_ref = stack.borrow_mut();
            if !stack_ref.is_empty() {
                stack_ref.pop();
            }
        });
    }
}

/// Adapter that wraps `Arc<Mutex<dyn WritableAsset>>` to impl `WritableAsset`
/// directly, allowing it to be stored as `Arc<dyn WritableAsset + Send + Sync>`.
struct MutexWritableAssetAdapter<W: WritableAsset + Send>(Arc<std::sync::Mutex<W>>);

// SAFETY: MutexWritableAssetAdapter is Sync because it wraps an Arc<Mutex<W>>.
// Arc is Sync when T: Send + Sync, and Mutex<W> is Sync when W: Send.
// The WritableAsset trait requires Send, so this is safe.
#[allow(unsafe_code)]
unsafe impl<W: WritableAsset + Send> Sync for MutexWritableAssetAdapter<W> {}

impl<W: WritableAsset + Send> WritableAsset for MutexWritableAssetAdapter<W> {
    fn close(&mut self) -> bool {
        if let Ok(mut inner) = self.0.lock() {
            inner.close()
        } else {
            false
        }
    }

    fn write(&mut self, buffer: &[u8], offset: usize) -> usize {
        if let Ok(mut inner) = self.0.lock() {
            inner.write(buffer, offset)
        } else {
            0
        }
    }
}

/// Returns true if the path is "file-relative", i.e. starts with "./" or "../".
///
/// File-relative paths are resolved relative to their anchor, NOT via search paths.
/// This matches the C++ `_IsFileRelative` function in defaultResolver.cpp.
#[inline]
fn is_file_relative(path: &str) -> bool {
    path.starts_with("./")
        || path.starts_with("../")
        || path.starts_with(".\\")
        || path.starts_with("..\\")
}

/// Returns true if path is non-empty and relative.
///
/// Matches C++ `_IsRelativePath` in defaultResolver.cpp:
///   `!path.empty() && TfIsRelativePath(path)`
///
/// On Windows, `TfIsRelativePath` treats paths starting with '/' or '\' as
/// absolute even though Rust's `Path::is_absolute()` considers them relative
/// (since they lack a drive letter).
#[inline]
fn is_relative_path(path: &str) -> bool {
    if path.is_empty() {
        return false;
    }
    // On all platforms, treat leading '/' or '\' as absolute (matches C++ behavior).
    let first = path.as_bytes()[0];
    if first == b'/' || first == b'\\' {
        return false;
    }
    !Path::new(path).is_absolute()
}

/// Returns true if the path is a "search path" - a relative path that is NOT
/// file-relative (i.e. does not start with "./" or "../").
///
/// Search paths are resolved by searching through configured search directories.
/// This matches the C++ `_IsSearchPath` function in defaultResolver.cpp:
///   `_IsRelativePath(path) && !_IsFileRelative(path)`
#[inline]
fn is_search_path(path: &str) -> bool {
    is_relative_path(path) && !is_file_relative(path)
}

/// Normalizes a path by resolving `.` and `..` components without
/// resolving symlinks or adding OS-specific prefixes.
///
/// Unlike `std::fs::canonicalize`, this does NOT:
/// - Resolve symlinks
/// - Add `\\?\` prefix on Windows
/// - Check whether the path actually exists
///
/// This matches C++ `TfNormPath` / `ArDefaultResolver::_CreateIdentifier` behavior.
fn normalize_path(path: &Path) -> String {
    // Delegate to usd_arch::norm_path via usd_tf for consistent normalization.
    usd_tf::path_utils::norm_path(&path.to_string_lossy())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_default_resolver_new() {
        let resolver = DefaultResolver::new();
        // Should not panic
        let _ = resolver.get_default_search_path();
    }

    #[test]
    fn test_default_resolver_set_search_path() {
        let resolver = DefaultResolver::new();
        resolver.set_default_search_path(vec!["/path1".into(), "/path2".into()]);

        let paths = resolver.get_default_search_path();
        assert_eq!(paths, vec!["/path1", "/path2"]);
    }

    #[test]
    fn test_resolve_absolute_path() {
        let resolver = DefaultResolver::new();

        // Create a temp file
        let dir = tempdir().expect("should create temp dir");
        let file_path = dir.path().join("test.usd");
        std::fs::write(&file_path, b"test").expect("should write file");

        let resolved = resolver.resolve(file_path.to_str().expect("valid path"));
        assert!(!resolved.is_empty());
        assert_eq!(resolved.as_str(), file_path.to_str().expect("valid path"));
    }

    #[test]
    fn test_resolve_nonexistent() {
        let resolver = DefaultResolver::new();
        let resolved = resolver.resolve("/nonexistent/path/to/file.usd");
        assert!(resolved.is_empty());
    }

    #[test]
    fn test_get_extension() {
        let resolver = DefaultResolver::new();
        assert_eq!(resolver.get_extension("/path/to/file.usd"), "usd");
        assert_eq!(resolver.get_extension("/path/to/file.usda"), "usda");
        assert_eq!(resolver.get_extension("/path/to/file"), "");
        assert_eq!(resolver.get_extension(".hidden"), "");
        assert_eq!(resolver.get_extension(".hidden.txt"), "txt");
    }

    #[test]
    fn test_is_context_dependent_path() {
        let resolver = DefaultResolver::new();

        // Search-path style relative paths ARE context-dependent
        assert!(resolver.is_context_dependent_path("relative/path.usd"));
        assert!(resolver.is_context_dependent_path("models/char.usd"));

        // File-relative paths (./ and ../) are NOT context-dependent
        // (they resolve relative to their anchor, not via search paths)
        assert!(!resolver.is_context_dependent_path("./local.usd"));
        assert!(!resolver.is_context_dependent_path("../parent.usd"));

        // Absolute paths are NOT context-dependent
        #[cfg(windows)]
        assert!(!resolver.is_context_dependent_path("C:\\absolute\\path.usd"));
        #[cfg(not(windows))]
        assert!(!resolver.is_context_dependent_path("/absolute/path.usd"));

        // Empty path is NOT context-dependent
        assert!(!resolver.is_context_dependent_path(""));
    }

    #[test]
    fn test_is_search_path() {
        assert!(is_search_path("models/char.usd"));
        assert!(is_search_path("char.usd"));
        assert!(!is_search_path("./char.usd"));
        assert!(!is_search_path("../char.usd"));
        assert!(!is_search_path("/absolute/char.usd"));
        assert!(!is_search_path(""));
    }

    #[test]
    fn test_is_file_relative() {
        assert!(is_file_relative("./local.usd"));
        assert!(is_file_relative("../parent.usd"));
        assert!(!is_file_relative("models/char.usd"));
        assert!(!is_file_relative("/absolute/path.usd"));
        assert!(!is_file_relative(""));
    }

    #[test]
    fn test_create_identifier() {
        let resolver = DefaultResolver::new();

        // Absolute path stays absolute (matches C++ _CreateIdentifier)
        let id = resolver.create_identifier("/absolute/path.usd", None);
        assert!(
            id.contains("absolute"),
            "Absolute path should be preserved: got '{}'",
            id
        );

        // No anchor → normalize only
        let id = resolver.create_identifier("search_path.usd", None);
        assert!(
            id.contains("search_path"),
            "Without anchor, search path should be preserved: got '{}'",
            id
        );

        // Relative path with anchor — uses look-here-first semantics:
        // Since the anchored path doesn't actually exist on disk,
        // search paths fall through to returning the bare search path.
        // File-relative paths always anchor regardless.
        // (Matches C++ _CreateIdentifier behavior)
        let anchor = ResolvedPath::new("/base/dir/anchor.usd");
        let id = resolver.create_identifier("./relative.usd", Some(&anchor));
        // File-relative (./) always anchors to the anchor directory
        assert!(
            id.contains("base") || id.contains("dir") || id.contains("relative"),
            "File-relative path should anchor: got '{}'",
            id
        );

        // Create a real file to test anchored resolution
        let dir = tempdir().expect("tempdir");
        let anchor_path = dir.path().join("anchor.usd");
        let target_path = dir.path().join("sibling.usd");
        std::fs::write(&anchor_path, "").unwrap();
        std::fs::write(&target_path, "").unwrap();

        let anchor = ResolvedPath::new(&*anchor_path.to_string_lossy());
        let id = resolver.create_identifier("sibling.usd", Some(&anchor));
        // Since the anchored path exists, look-here-first returns anchored path
        assert!(
            id.contains("sibling"),
            "Anchored identifier should contain the asset name: got '{}'",
            id
        );
    }

    #[test]
    fn test_create_context() {
        let resolver = DefaultResolver::new();

        let ctx = resolver.create_default_context();
        assert!(ctx.contains::<DefaultResolverContext>());

        let ctx = resolver.create_context_from_string("/path1:/path2");
        let default_ctx: &DefaultResolverContext = ctx.get().expect("should have context");
        assert!(default_ctx.search_paths().len() >= 1);
    }

    #[test]
    fn test_bind_unbind_context() {
        let resolver = DefaultResolver::new();

        let ctx =
            ResolverContext::with_object(DefaultResolverContext::new(vec!["/test/path".into()]));

        let binding = resolver.bind_context(&ctx);
        let current = resolver.get_current_context();
        assert!(current.contains::<DefaultResolverContext>());

        resolver.unbind_context(&ctx, binding);
        let current = resolver.get_current_context();
        assert!(current.is_empty());
    }

    #[test]
    fn test_get_modification_timestamp() {
        let resolver = DefaultResolver::new();

        // Create a temp file
        let dir = tempdir().expect("should create temp dir");
        let file_path = dir.path().join("test.usd");
        std::fs::write(&file_path, b"test").expect("should write file");

        let resolved = ResolvedPath::new(file_path.to_string_lossy().into_owned());
        let timestamp = resolver.get_modification_timestamp("", &resolved);

        assert!(timestamp.is_valid());
    }

    #[test]
    fn test_open_asset() {
        let resolver = DefaultResolver::new();

        // Create a temp file
        let dir = tempdir().expect("should create temp dir");
        let file_path = dir.path().join("test.usd");
        std::fs::write(&file_path, b"test content").expect("should write file");

        let resolved = ResolvedPath::new(file_path.to_string_lossy().into_owned());
        let asset = resolver.open_asset(&resolved).expect("should open asset");

        assert_eq!(asset.size(), 12);

        let buffer = asset.get_buffer().expect("should get buffer");
        assert_eq!(&*buffer, b"test content");
    }

    #[test]
    fn test_open_asset_for_write() {
        let resolver = DefaultResolver::new();

        let dir = tempdir().expect("should create temp dir");
        let file_path = dir.path().join("new_file.usd");

        let resolved = ResolvedPath::new(file_path.to_string_lossy().into_owned());

        let _asset = resolver
            .open_asset_for_write(&resolved, WriteMode::Replace)
            .expect("should open for write");

        // Note: FilesystemWritableAsset requires &mut self for write/close
        // This is a limitation of the current design
    }

    #[test]
    fn test_can_write_asset_to_path() {
        let resolver = DefaultResolver::new();

        let dir = tempdir().expect("should create temp dir");
        let file_path = dir.path().join("writable.usd");

        let resolved = ResolvedPath::new(file_path.to_string_lossy().into_owned());
        assert!(resolver.can_write_asset_to_path(&resolved, None));

        let empty = ResolvedPath::empty();
        let mut why_not = String::new();
        assert!(!resolver.can_write_asset_to_path(&empty, Some(&mut why_not)));
        assert!(!why_not.is_empty());
    }

    #[test]
    fn test_cache_scope() {
        let resolver = DefaultResolver::new();

        // begin_cache_scope returns cache scope data
        let data = resolver.begin_cache_scope();
        assert!(data.is_some());

        resolver.end_cache_scope(data);
    }

    #[test]
    fn test_normalize_path() {
        // Test with existing path
        let dir = tempdir().expect("should create temp dir");
        let result = normalize_path(dir.path());
        assert!(!result.is_empty());

        // Test with non-existing path containing ..
        let path = Path::new("/some/path/../other");
        let result = normalize_path(path);
        assert!(!result.contains(".."));
    }
}
