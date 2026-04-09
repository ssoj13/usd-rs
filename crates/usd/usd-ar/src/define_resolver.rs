//! Resolver type registration (AR_DEFINE_RESOLVER).
//!
//! Matches C++ `defineResolver.h`: TfType-based registration for resolver discovery.
//!
//! # Custom resolvers
//!
//! To register a custom resolver so it can be discovered by [`get_available_resolvers`](super::get_available_resolvers)
//! and created via [`create_resolver`](super::create_resolver), call [`define_resolver`] in your resolver's module:
//!
//! ```ignore
//! use usd_ar::define_resolver::define_resolver;
//! use usd_ar::{Resolver};
//!
//! define_resolver::<MyCustomResolver>("MyCustomResolver");
//! ```
//!
//! For URI resolvers, use [`define_resolver_with_meta`] to specify URI schemes
//! and optional metadata (matches C++ plugInfo.json `"uriSchemes"`,
//! `"implementsContexts"`, `"implementsScopedCaches"`):
//!
//! ```ignore
//! use usd_ar::define_resolver::{define_resolver_with_meta, ResolverMeta};
//!
//! define_resolver_with_meta::<MyURIResolver>("MyURIResolver", ResolverMeta {
//!     uri_schemes: vec!["myscheme".into()],
//!     implements_contexts: true,
//!     implements_scoped_caches: false,
//! });
//! ```
//!
//! # Reference
//!
//! C++ equivalent: `AR_DEFINE_RESOLVER(ResolverClass, ArResolver)` in defineResolver.h

use std::any::TypeId;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use usd_tf::TfType;

use super::resolver::Resolver;

/// Marker type for the ArResolver base in the TfType registry.
///
/// All concrete resolvers (`DefaultResolver`, custom implementations) are
/// registered as deriving from this type.
///
/// Matches C++ `TfType::Define<ArResolver>()` in resolver.cpp.
#[derive(Debug, Clone, Copy)]
pub struct ArResolver;

/// Optional metadata for resolver registration.
///
/// Matches the plugInfo.json metadata keys used by C++ `_GetAvailableResolvers`.
#[derive(Clone, Debug, Default)]
pub struct ResolverMeta {
    /// URI/IRI schemes handled by this resolver (e.g. `["http", "https"]`).
    /// Matches C++ plugInfo.json `"uriSchemes"` key.
    pub uri_schemes: Vec<String>,
    /// Whether this resolver implements context-related operations.
    /// Matches C++ plugInfo.json `"implementsContexts"` key.
    pub implements_contexts: bool,
    /// Whether this resolver implements scoped-cache operations.
    /// Matches C++ plugInfo.json `"implementsScopedCaches"` key.
    pub implements_scoped_caches: bool,
}

/// Type name -> constructor map.
static RESOLVER_CONSTRUCTORS: OnceLock<Mutex<HashMap<String, fn() -> Box<dyn Resolver>>>> =
    OnceLock::new();

/// Type name -> metadata map.
static RESOLVER_META: OnceLock<Mutex<HashMap<String, ResolverMeta>>> = OnceLock::new();

fn constructors() -> &'static Mutex<HashMap<String, fn() -> Box<dyn Resolver>>> {
    RESOLVER_CONSTRUCTORS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn metadata_map() -> &'static Mutex<HashMap<String, ResolverMeta>> {
    RESOLVER_META.get_or_init(|| Mutex::new(HashMap::new()))
}

fn ensure_base() {
    TfType::declare::<ArResolver>("ArResolver");
}

/// Registers a resolver type with TfType and the constructor registry.
///
/// Performs the same role as C++ `AR_DEFINE_RESOLVER(ResolverClass, ArResolver)`.
/// The type becomes discoverable via [`get_available_resolvers`] and instantiable
/// via [`create_resolver`].
///
/// # Arguments
///
/// * `name` - Type name for lookup (e.g. `"ArDefaultResolver"`). Must be unique.
///
/// # Type requirements
///
/// `R` must implement [`Resolver`] and [`Default`].
pub fn define_resolver<R: Resolver + Default + 'static>(name: &str) {
    define_resolver_with_meta::<R>(name, ResolverMeta::default());
}

/// Registers a resolver type with metadata (URI schemes, context/cache flags).
///
/// Extended version of [`define_resolver`] that accepts metadata matching the
/// C++ plugInfo.json keys `"uriSchemes"`, `"implementsContexts"`,
/// `"implementsScopedCaches"`.
pub fn define_resolver_with_meta<R: Resolver + Default + 'static>(name: &str, meta: ResolverMeta) {
    ensure_base();
    TfType::declare_with_bases::<R>(name, &[TypeId::of::<ArResolver>()]);
    constructors()
        .lock()
        .expect("resolver constructor registry lock poisoned")
        .insert(name.to_string(), || Box::new(R::default()));
    metadata_map()
        .lock()
        .expect("resolver metadata lock poisoned")
        .insert(name.to_string(), meta);
}

/// Registers a resolver type with explicit base types (for type hierarchy).
///
/// Like C++ `AR_DEFINE_RESOLVER(Derived, Base)` where Base != ArResolver.
pub fn define_resolver_with_bases<R: Resolver + Default + 'static>(
    name: &str,
    base_names: &[&str],
    meta: ResolverMeta,
) {
    ensure_base();
    // Register TfType with specific bases
    let base_ids: Vec<TypeId> = base_names
        .iter()
        .map(|_| TypeId::of::<ArResolver>()) // All ultimately derive from ArResolver
        .collect();
    TfType::declare_with_bases::<R>(name, &base_ids);

    // Also register with TfType by name for base-type linkage
    for base_name in base_names {
        usd_tf::declare_by_name_with_bases(name, &[base_name]);
    }

    constructors()
        .lock()
        .expect("resolver constructor registry lock poisoned")
        .insert(name.to_string(), || Box::new(R::default()));
    metadata_map()
        .lock()
        .expect("resolver metadata lock poisoned")
        .insert(name.to_string(), meta);
}

/// Returns metadata for a resolver type, if registered programmatically.
pub fn get_resolver_meta(type_name: &str) -> Option<ResolverMeta> {
    metadata_map()
        .lock()
        .expect("resolver metadata lock poisoned")
        .get(type_name)
        .cloned()
}

/// Ensures built-in resolvers (ArDefaultResolver) are registered.
/// Called lazily on first resolver API access.
pub(crate) fn ensure_resolvers_registered() {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        define_resolver_with_meta::<super::resolver::DefaultResolver>(
            "ArDefaultResolver",
            ResolverMeta {
                uri_schemes: Vec::new(),
                implements_contexts: true,
                implements_scoped_caches: true,
            },
        );
    });
}

/// Returns all TfTypes for resolver types sorted by name.
/// Matches C++ `_GetAvailableResolvers` — returns ALL derived types, not just primaries.
pub(crate) fn get_all_resolver_types() -> Vec<TfType> {
    ensure_resolvers_registered();
    let default_type = TfType::find::<super::resolver::DefaultResolver>();
    let base = TfType::find::<ArResolver>();
    let mut types: Vec<TfType> = base
        .get_all_derived_types()
        .into_iter()
        .filter(|t| *t != default_type)
        .collect();
    types.sort_by(|a, b| a.type_name().cmp(&b.type_name()));
    types.push(default_type); // DefaultResolver always last (C++ fallback order)
    types
}

/// Creates a new resolver instance for the given TfType.
/// Returns `None` if the type is unknown or has no registered constructor.
/// Matches C++ `_CreateResolver` / `ArCreateResolver`.
pub(crate) fn create_resolver_by_type(tf_type: TfType) -> Option<Box<dyn Resolver>> {
    ensure_resolvers_registered();
    let name = tf_type.type_name();
    if name.is_empty() {
        return None;
    }
    constructors()
        .lock()
        .expect("resolver constructor registry lock poisoned")
        .get(&name)
        .map(|f| f())
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_tf::TfType;

    #[test]
    fn test_define_resolver() {
        let types = get_all_resolver_types();
        assert!(
            types.iter().any(|t| t.type_name() == "ArDefaultResolver"),
            "ArDefaultResolver should appear"
        );

        let tf_type = TfType::find_by_name("ArDefaultResolver");
        let resolver = create_resolver_by_type(tf_type);
        assert!(resolver.is_some());
        let resolver = resolver.unwrap();
        let _ = resolver.resolve("model.usd");
    }

    #[test]
    fn test_resolver_meta_default() {
        ensure_resolvers_registered();
        let meta = get_resolver_meta("ArDefaultResolver");
        assert!(meta.is_some());
        let meta = meta.unwrap();
        assert!(meta.uri_schemes.is_empty());
        assert!(meta.implements_contexts);
        assert!(meta.implements_scoped_caches);
    }
}
