//! Dispatching resolver — routes calls to primary, URI, and package resolvers.
//!
//! Strict port of C++ `_DispatchingResolver` from `resolver.cpp`.
//!
//! This is the actual resolver singleton returned by `get_resolver()`. It owns
//! the primary resolver, any URI-scheme resolvers, and delegates every
//! `Resolver` trait method to the appropriate sub-resolver based on the
//! asset path's URI scheme.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use usd_plug::PlugRegistry;
use usd_tf::TfType;
use usd_vt::Value;

use crate::asset::Asset;
use crate::asset_info::AssetInfo;
use crate::define_resolver;
use crate::package_resolver;
use crate::package_utils::{
    is_package_relative_path, join_package_relative_path_pair, split_package_relative_path_inner,
    split_package_relative_path_outer,
};
use crate::resolved_path::ResolvedPath;
use crate::resolver::{DefaultResolver, Resolver};
use crate::resolver_context::ResolverContext;
use crate::timestamp::Timestamp;
use crate::writable_asset::{WritableAsset, WriteMode};

// ── Environment variables (C++ TF_DEFINE_ENV_SETTING) ──────────────────────

const ENV_DISABLE_PLUGIN_RESOLVER: &str = "PXR_AR_DISABLE_PLUGIN_RESOLVER";
const ENV_DISABLE_PLUGIN_URI_RESOLVERS: &str = "PXR_AR_DISABLE_PLUGIN_URI_RESOLVERS";

fn env_is_true(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

// ── _ResolverInfo (C++ struct _ResolverInfo) ───────────────────────────────

/// Metadata about a discovered resolver. Matches C++ `_ResolverInfo`.
#[derive(Clone, Debug)]
pub struct ResolverInfo {
    pub type_name: String,
    pub uri_schemes: Vec<String>,
    pub can_be_primary: bool,
    pub implements_contexts: bool,
    pub implements_scoped_caches: bool,
}

// ── URI scheme validation (C++ _ValidateResourceIdentifierScheme) ──────────

/// Validates that `scheme` conforms to RFC 3986 sec 3.1 / RFC 3987 sec 2.2.
/// Scheme must start with ASCII a-z, followed by a-z 0-9 - . +
pub fn validate_uri_scheme(scheme: &str) -> Result<(), String> {
    if scheme.is_empty() {
        return Err("Scheme cannot be empty".into());
    }
    let bytes = scheme.as_bytes();
    if !(bytes[0] >= b'a' && bytes[0] <= b'z') {
        return Err("Scheme must start with ASCII 'a-z'".into());
    }
    for &b in &bytes[1..] {
        let valid = (b >= b'a' && b <= b'z')
            || (b >= b'0' && b <= b'9')
            || b == b'-'
            || b == b'.'
            || b == b'+';
        if !valid {
            return Err(format!(
                "'{}' not allowed in scheme. Characters must be ASCII 'a-z', '-', '+', or '.'",
                char::from(b)
            ));
        }
    }
    Ok(())
}

// ── _GetAvailableResolvers (C++ resolver.cpp:208-285) ──────────────────────

/// Collect all available resolvers from TfType registry + PlugRegistry metadata.
fn get_available_resolvers() -> Vec<ResolverInfo> {
    define_resolver::ensure_resolvers_registered();

    let base = TfType::find::<define_resolver::ArResolver>();
    let mut derived: Vec<TfType> = base.get_all_derived_types().into_iter().collect();
    // Sort by typename for stable order (C++ resolver.cpp:227-231)
    derived.sort_by(|a, b| a.type_name().cmp(&b.type_name()));

    let plug_reg = PlugRegistry::get_instance();
    let mut result = Vec::with_capacity(derived.len());

    for tf_type in derived {
        let type_name = tf_type.type_name();

        // 1. Programmatic metadata (from define_resolver_with_meta)
        let prog_meta = define_resolver::get_resolver_meta(&type_name);

        // 2. PlugRegistry metadata (from plugInfo.json)
        let plug_meta = plug_reg
            .get_plugin_for_type(&type_name)
            .and_then(|p| p.get_metadata_for_type(&type_name));

        // Merge: programmatic takes priority, plug overlays missing fields
        let mut uri_schemes: Vec<String> = prog_meta
            .as_ref()
            .map(|m| m.uri_schemes.clone())
            .unwrap_or_default();

        let mut implements_contexts = prog_meta
            .as_ref()
            .map(|m| m.implements_contexts)
            .unwrap_or(false);

        let mut implements_scoped_caches = prog_meta
            .as_ref()
            .map(|m| m.implements_scoped_caches)
            .unwrap_or(false);

        if let Some(ref pobj) = plug_meta {
            // uriSchemes from plugInfo
            if uri_schemes.is_empty() {
                if let Some(arr) = pobj.get("uriSchemes").and_then(|v| v.as_array()) {
                    uri_schemes = arr
                        .iter()
                        .filter_map(|v| v.as_string().map(String::from))
                        .collect();
                }
            }
            // implementsContexts — walk base types (C++ _FindMetadataValueOnTypeOrBase)
            if !implements_contexts {
                implements_contexts = find_bool_on_type_or_base("implementsContexts", &type_name);
            }
            // implementsScopedCaches — walk base types
            if !implements_scoped_caches {
                implements_scoped_caches =
                    find_bool_on_type_or_base("implementsScopedCaches", &type_name);
            }
        }

        // C++ resolver.cpp:273: canBePrimaryResolver = uriSchemes.empty()
        let can_be_primary = uri_schemes.is_empty();

        result.push(ResolverInfo {
            type_name,
            uri_schemes,
            can_be_primary,
            implements_contexts,
            implements_scoped_caches,
        });
    }

    result
}

/// Walk type hierarchy for a boolean metadata key.
/// C++ `_FindMetadataValueOnTypeOrBase<bool>`.
fn find_bool_on_type_or_base(key: &str, type_name: &str) -> bool {
    let plug_reg = PlugRegistry::get_instance();
    if let Some(plugin) = plug_reg.get_plugin_for_type(type_name) {
        if let Some(meta) = plugin.get_metadata_for_type(type_name) {
            if let Some(b) = meta.get(key).and_then(|v| v.as_bool()) {
                return b;
            }
        }
    }
    // Walk base types
    let tf = TfType::find_by_name(type_name);
    for base in tf.base_types() {
        let bn = base.type_name();
        if bn.is_empty() || bn == "ArResolver" {
            continue;
        }
        if find_bool_on_type_or_base(key, &bn) {
            return true;
        }
    }
    false
}

// ── _GetAvailablePrimaryResolvers (C++ resolver.cpp:287-334) ───────────────

fn get_available_primary_resolvers(all: &[ResolverInfo]) -> Vec<ResolverInfo> {
    let disable = env_is_true(ENV_DISABLE_PLUGIN_RESOLVER);

    let mut primaries: Vec<ResolverInfo> = if disable {
        Vec::new()
    } else {
        all.iter()
            .filter(|r| r.can_be_primary && r.type_name != "ArDefaultResolver")
            .cloned()
            .collect()
    };

    // DefaultResolver always last (C++ resolver.cpp:325-331)
    if let Some(def) = all.iter().find(|r| r.type_name == "ArDefaultResolver") {
        primaries.push(def.clone());
    }

    primaries
}

// ── ResolverEntry — resolver instance + its info ───────────────────────────

struct ResolverEntry {
    info: ResolverInfo,
    resolver: Box<dyn Resolver>,
}

// ── Thread-local context stack (C++ _DispatchingResolver::_threadContextStack)

thread_local! {
    static DISPATCHING_CONTEXT_STACK: RefCell<Vec<ResolverContext>> =
        const { RefCell::new(Vec::new()) };
}

// ── DispatchingResolver (C++ class _DispatchingResolver) ───────────────────

/// Top-level resolver singleton. Routes calls to primary / URI / package
/// resolvers.  Matches C++ `_DispatchingResolver` in `resolver.cpp`.
pub struct DispatchingResolver {
    primary: ResolverEntry,
    uri_resolvers: HashMap<String, ResolverEntry>,
    max_uri_scheme_len: usize,
}

impl DispatchingResolver {
    /// Construct and initialize. C++ `_DispatchingResolver::_DispatchingResolver()`.
    pub fn new() -> Self {
        let available = get_available_resolvers();
        let primary = Self::init_primary(&available);
        let (uri_resolvers, max_len) = Self::init_uri_resolvers(&available);
        Self::init_package_resolvers();
        Self {
            primary,
            uri_resolvers,
            max_uri_scheme_len: max_len,
        }
    }

    // ── _InitializePrimaryResolver (C++ resolver.cpp:1046-1127) ────────

    fn init_primary(available: &[ResolverInfo]) -> ResolverEntry {
        let primaries = get_available_primary_resolvers(available);

        log::debug!(
            "ArGetResolver(): Found primary asset resolver types: [{}]",
            primaries
                .iter()
                .map(|r| r.type_name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );

        let chosen = if env_is_true(ENV_DISABLE_PLUGIN_RESOLVER) {
            log::debug!(
                "ArGetResolver(): Plugin asset resolver disabled via PXR_AR_DISABLE_PLUGIN_RESOLVER."
            );
            "ArDefaultResolver".to_string()
        } else if let Some(pref) = crate::resolver::get_preferred_resolver_name() {
            // C++ resolver.cpp:1064-1085
            let tf = TfType::find_by_name(&pref);
            if tf.is_unknown() {
                log::warn!(
                    "ArGetResolver(): Preferred resolver {} not found. Using default resolver.",
                    pref
                );
                "ArDefaultResolver".to_string()
            } else {
                log::debug!("ArGetResolver(): Using preferred resolver {}", pref);
                pref
            }
        } else if !primaries.is_empty() {
            // C++ resolver.cpp:1086-1098: first non-default, or default
            let chosen = &primaries[0].type_name;
            if primaries.len() > 2 {
                log::debug!(
                    "ArGetResolver(): Found multiple primary asset resolvers, using {}",
                    chosen
                );
            }
            chosen.clone()
        } else {
            "ArDefaultResolver".to_string()
        };

        // Find info for chosen type
        let info = available
            .iter()
            .find(|r| r.type_name == chosen)
            .cloned()
            .unwrap_or(ResolverInfo {
                type_name: "ArDefaultResolver".into(),
                uri_schemes: Vec::new(),
                can_be_primary: true,
                implements_contexts: false,
                implements_scoped_caches: false,
            });

        let resolver =
            define_resolver::create_resolver_by_type(TfType::find_by_name(&info.type_name))
                .unwrap_or_else(|| {
                    log::debug!("ArGetResolver(): Using default asset resolver ArDefaultResolver");
                    Box::new(DefaultResolver::new())
                });

        log::debug!("ArGetResolver(): {} for primary resolver", info.type_name);
        ResolverEntry { info, resolver }
    }

    // ── _InitializeURIResolvers (C++ resolver.cpp:1129-1205) ───────────

    fn init_uri_resolvers(available: &[ResolverInfo]) -> (HashMap<String, ResolverEntry>, usize) {
        if env_is_true(ENV_DISABLE_PLUGIN_URI_RESOLVERS) {
            log::debug!(
                "ArGetResolver(): Plugin URI asset resolvers disabled via PXR_AR_DISABLE_PLUGIN_URI_RESOLVERS."
            );
            return (HashMap::new(), 0);
        }

        let mut map = HashMap::new();
        let mut max_len = 0usize;

        for info in available.iter().filter(|r| !r.uri_schemes.is_empty()) {
            log::debug!("ArGetResolver(): Found URI resolver {}", info.type_name);

            let mut valid_schemes = Vec::new();
            for scheme in &info.uri_schemes {
                // RFC 3986 sec 3.1: case-insensitive
                let lower = scheme.to_ascii_lowercase();

                if map.contains_key(&lower) {
                    log::warn!(
                        "ArGetResolver(): {} registered to handle scheme '{}' which is already handled. Ignoring.",
                        info.type_name,
                        lower
                    );
                    continue;
                }
                if let Err(msg) = validate_uri_scheme(&lower) {
                    log::warn!(
                        "ArGetResolver(): '{}' for '{}' is not a valid resource identifier scheme: {}. \
                        Paths with this prefix will be handled by other resolvers.",
                        lower,
                        info.type_name,
                        msg
                    );
                    continue;
                }
                valid_schemes.push(lower);
            }

            if valid_schemes.is_empty() {
                continue;
            }

            log::debug!(
                "ArGetResolver(): Using {} for URI scheme(s) [\"{}\"]",
                info.type_name,
                valid_schemes.join("\", \"")
            );

            // Create one resolver instance per scheme (C++ shares via shared_ptr;
            // Rust creates separate instances since Box<dyn Resolver> isn't Clone).
            for scheme in &valid_schemes {
                max_len = max_len.max(scheme.len());
                if let Some(r) =
                    define_resolver::create_resolver_by_type(TfType::find_by_name(&info.type_name))
                {
                    map.insert(
                        scheme.clone(),
                        ResolverEntry {
                            info: info.clone(),
                            resolver: r,
                        },
                    );
                }
            }
        }

        (map, max_len)
    }

    // ── _InitializePackageResolvers (C++ resolver.cpp:1207-1267) ───────

    fn init_package_resolvers() {
        // Discover package resolver types via PlugRegistry
        let plug_reg = PlugRegistry::get_instance();
        let pkg_types = plug_reg.get_all_derived_types("ArPackageResolver");

        for type_name in &pkg_types {
            log::debug!("ArGetResolver(): Found package resolver {}", type_name);

            let plugin = match plug_reg.get_plugin_for_type(type_name) {
                Some(p) => p,
                None => {
                    log::error!("Could not find plugin for package resolver {}", type_name);
                    continue;
                }
            };

            let meta = match plugin.get_metadata_for_type(type_name) {
                Some(m) => m,
                None => continue,
            };

            let extensions = match meta.get("extensions").and_then(|v| v.as_array()) {
                Some(arr) => arr
                    .iter()
                    .filter_map(|v| v.as_string().map(String::from))
                    .collect::<Vec<_>>(),
                None => {
                    log::error!(
                        "No package formats specified in 'extensions' metadata for '{}'",
                        type_name
                    );
                    continue;
                }
            };

            for ext in &extensions {
                if ext.is_empty() {
                    continue;
                }
                let lower = ext.to_ascii_lowercase();
                log::debug!(
                    "ArGetResolver(): Using package resolver {} for {} from plugin {}",
                    type_name,
                    ext,
                    plugin.get_name()
                );
                // Registration of package resolvers happens in the providing crate
                // (e.g. usd-sdf registers .usdz). We just log discovery here.
                let _ = lower;
            }
        }
    }

    // ── _GetResolver (C++ resolver.cpp:1269-1283) ──────────────────────

    /// Get resolver + info for a path. URI scheme check first, then primary.
    fn get_resolver_for_path(&self, path: &str) -> (&dyn Resolver, &ResolverInfo) {
        if let Some(entry) = self.get_uri_resolver(path) {
            return (entry.resolver.as_ref(), &entry.info);
        }
        (self.primary.resolver.as_ref(), &self.primary.info)
    }

    // ── _GetURIResolver (C++ resolver.cpp:1285-1308) ───────────────────

    fn get_uri_resolver(&self, path: &str) -> Option<&ResolverEntry> {
        if self.uri_resolvers.is_empty() {
            return None;
        }
        let search_len = path.len().min(self.max_uri_scheme_len + 1);
        let colon_pos = path[..search_len].find(':')?;
        self.get_uri_resolver_for_scheme(&path[..colon_pos])
    }

    // ── _GetURIResolverForScheme (C++ resolver.cpp:1310-1329) ──────────

    fn get_uri_resolver_for_scheme(&self, scheme: &str) -> Option<&ResolverEntry> {
        self.uri_resolvers.get(&scheme.to_ascii_lowercase())
    }

    /// Returns all registered URI schemes, sorted. C++ `GetURISchemes`.
    pub fn get_uri_schemes(&self) -> Vec<String> {
        let mut schemes: Vec<String> = self.uri_resolvers.keys().cloned().collect();
        schemes.sort();
        schemes
    }

    /// Returns primary resolver reference. C++ `GetPrimaryResolver`.
    pub fn get_primary_resolver(&self) -> &dyn Resolver {
        self.primary.resolver.as_ref()
    }

    // ── _ResolveHelper (C++ resolver.cpp:1347-1396) ────────────────────

    fn resolve_helper<F>(&self, path: &str, resolve_fn: &F) -> ResolvedPath
    where
        F: Fn(&str) -> ResolvedPath,
    {
        if is_package_relative_path(path) {
            let (pkg_path, packaged_path) = split_package_relative_path_outer(path);
            let resolved_pkg = resolve_fn(&pkg_path);
            if resolved_pkg.is_empty() {
                return ResolvedPath::empty();
            }

            let mut current = resolved_pkg.as_str().to_string();
            let mut remaining = packaged_path;

            while !remaining.is_empty() {
                let (inner_pkg, inner_rest) = if is_package_relative_path(&remaining) {
                    split_package_relative_path_outer(&remaining)
                } else {
                    (remaining.clone(), String::new())
                };

                let resolved_inner = package_resolver::resolve_packaged_path(&current, &inner_pkg);
                if resolved_inner.is_empty() {
                    return ResolvedPath::empty();
                }

                current = join_package_relative_path_pair(&current, &resolved_inner);
                remaining = inner_rest;
            }

            return ResolvedPath::new(current);
        }

        resolve_fn(path)
    }
}

impl Default for DispatchingResolver {
    fn default() -> Self {
        Self::new()
    }
}

// ── Resolver trait (C++ _DispatchingResolver virtual method overrides) ──────

impl Resolver for DispatchingResolver {
    // ── _CreateIdentifier (C++ resolver.cpp:516-582) ───────────────────

    fn create_identifier(
        &self,
        asset_path: &str,
        anchor_asset_path: Option<&ResolvedPath>,
    ) -> String {
        if asset_path.is_empty() {
            return String::new();
        }

        // If assetPath has a recognized URI scheme, use that resolver.
        // Otherwise use the resolver for the anchor path.
        // C++ resolver.cpp:556-559
        let resolver: &dyn Resolver = if let Some(entry) = self.get_uri_resolver(asset_path) {
            entry.resolver.as_ref()
        } else {
            let anchor_str = anchor_asset_path.map(|a| a.as_str()).unwrap_or("");
            if let Some(entry) = self.get_uri_resolver(anchor_str) {
                entry.resolver.as_ref()
            } else {
                self.primary.resolver.as_ref()
            }
        };

        // C++ resolver.cpp:569-570: strip outer package from anchor
        let effective_anchor = anchor_asset_path.map(|a| {
            if is_package_relative_path(a.as_str()) {
                ResolvedPath::new(split_package_relative_path_outer(a.as_str()).0)
            } else {
                a.clone()
            }
        });

        // C++ resolver.cpp:572-581: package-relative path handling
        if is_package_relative_path(asset_path) {
            let (outer, inner) = split_package_relative_path_outer(asset_path);
            let outer_id = resolver.create_identifier(&outer, effective_anchor.as_ref());
            return join_package_relative_path_pair(&outer_id, &inner);
        }

        resolver.create_identifier(asset_path, effective_anchor.as_ref())
    }

    // ── _CreateIdentifierForNewAsset (C++ resolver.cpp:528-539) ────────

    fn create_identifier_for_new_asset(
        &self,
        asset_path: &str,
        anchor_asset_path: Option<&ResolvedPath>,
    ) -> String {
        if asset_path.is_empty() {
            return String::new();
        }

        let resolver: &dyn Resolver = if let Some(entry) = self.get_uri_resolver(asset_path) {
            entry.resolver.as_ref()
        } else {
            let anchor_str = anchor_asset_path.map(|a| a.as_str()).unwrap_or("");
            if let Some(entry) = self.get_uri_resolver(anchor_str) {
                entry.resolver.as_ref()
            } else {
                self.primary.resolver.as_ref()
            }
        };

        let effective_anchor = anchor_asset_path.map(|a| {
            if is_package_relative_path(a.as_str()) {
                ResolvedPath::new(split_package_relative_path_outer(a.as_str()).0)
            } else {
                a.clone()
            }
        });

        if is_package_relative_path(asset_path) {
            let (outer, inner) = split_package_relative_path_outer(asset_path);
            let outer_id =
                resolver.create_identifier_for_new_asset(&outer, effective_anchor.as_ref());
            return join_package_relative_path_pair(&outer_id, &inner);
        }

        resolver.create_identifier_for_new_asset(asset_path, effective_anchor.as_ref())
    }

    // ── _Resolve (C++ resolver.cpp:820-842) ────────────────────────────

    fn resolve(&self, asset_path: &str) -> ResolvedPath {
        if asset_path.is_empty() {
            return ResolvedPath::empty();
        }

        let resolve_fn = |path: &str| -> ResolvedPath {
            let (resolver, _info) = self.get_resolver_for_path(path);
            // C++ resolver.cpp:827-836: scoped cache check for non-implementsScopedCaches
            // (our DefaultResolver handles its own cache internally)
            resolver.resolve(path)
        };

        self.resolve_helper(asset_path, &resolve_fn)
    }

    // ── _ResolveForNewAsset (C++ resolver.cpp:844-855) ─────────────────

    fn resolve_for_new_asset(&self, asset_path: &str) -> ResolvedPath {
        let (resolver, _) = self.get_resolver_for_path(asset_path);
        if is_package_relative_path(asset_path) {
            let (outer, inner) = split_package_relative_path_outer(asset_path);
            let resolved_outer = resolver.resolve_for_new_asset(&outer);
            if resolved_outer.is_empty() {
                return ResolvedPath::empty();
            }
            return ResolvedPath::new(join_package_relative_path_pair(
                resolved_outer.as_str(),
                &inner,
            ));
        }
        resolver.resolve_for_new_asset(asset_path)
    }

    // ── _BindContext (C++ resolver.cpp:638-666) ────────────────────────

    fn bind_context(&self, context: &ResolverContext) -> Option<Value> {
        // C++ calls sub-resolvers that implement contexts
        if self.primary.info.implements_contexts {
            self.primary.resolver.bind_context(context);
        }
        for entry in self.uri_resolvers.values() {
            if entry.info.implements_contexts {
                entry.resolver.bind_context(context);
            }
        }

        // C++ resolver.cpp:664-665: push to thread-local context stack
        DISPATCHING_CONTEXT_STACK.with(|stack| {
            stack.borrow_mut().push(context.clone());
        });

        None
    }

    // ── _UnbindContext (C++ resolver.cpp:668-708) ──────────────────────

    fn unbind_context(&self, context: &ResolverContext, _binding_data: Option<Value>) {
        if self.primary.info.implements_contexts {
            self.primary.resolver.unbind_context(context, None);
        }
        for entry in self.uri_resolvers.values() {
            if entry.info.implements_contexts {
                entry.resolver.unbind_context(context, None);
            }
        }

        // C++ resolver.cpp:699-707: pop from thread-local context stack
        DISPATCHING_CONTEXT_STACK.with(|stack| {
            let mut s = stack.borrow_mut();
            if s.is_empty() {
                log::error!(
                    "No context was bound, cannot unbind context: {:?}",
                    context.debug_string()
                );
            } else {
                s.pop();
            }
        });
    }

    // ── _CreateDefaultContext (C++ resolver.cpp:710-729) ────────────────

    fn create_default_context(&self) -> ResolverContext {
        let mut contexts = Vec::new();

        if self.primary.info.implements_contexts {
            contexts.push(self.primary.resolver.create_default_context());
        }
        for entry in self.uri_resolvers.values() {
            if !entry.info.implements_contexts {
                continue;
            }
            contexts.push(entry.resolver.create_default_context());
        }

        ResolverContext::from_contexts(contexts)
    }

    // ── _CreateDefaultContextForAsset (C++ resolver.cpp:741-768) ───────

    fn create_default_context_for_asset(&self, asset_path: &str) -> ResolverContext {
        // C++ resolver.cpp:744-747: strip package-relative outer
        let effective = if is_package_relative_path(asset_path) {
            split_package_relative_path_outer(asset_path).0
        } else {
            asset_path.to_string()
        };

        let mut contexts = Vec::new();

        if self.primary.info.implements_contexts {
            contexts.push(
                self.primary
                    .resolver
                    .create_default_context_for_asset(&effective),
            );
        }
        for entry in self.uri_resolvers.values() {
            if !entry.info.implements_contexts {
                continue;
            }
            contexts.push(entry.resolver.create_default_context_for_asset(&effective));
        }

        ResolverContext::from_contexts(contexts)
    }

    // ── _CreateContextFromString (C++ resolver.cpp:731-739) ────────────

    fn create_context_from_string(&self, context_str: &str) -> ResolverContext {
        // C++ resolver.cpp:734: only if primary implements contexts
        if !self.primary.info.implements_contexts {
            return ResolverContext::new();
        }
        self.primary
            .resolver
            .create_context_from_string(context_str)
    }

    // ── CreateContextFromString(scheme, str) (C++ resolver.cpp:489-497)

    fn create_context_from_string_with_scheme(
        &self,
        uri_scheme: &str,
        context_str: &str,
    ) -> ResolverContext {
        if uri_scheme.is_empty() {
            return self
                .primary
                .resolver
                .create_context_from_string(context_str);
        }
        if let Some(entry) = self.get_uri_resolver_for_scheme(uri_scheme) {
            entry.resolver.create_context_from_string(context_str)
        } else {
            ResolverContext::new()
        }
    }

    // ── CreateContextFromStrings (C++ resolver.cpp:499-514) ────────────

    fn create_context_from_strings(&self, context_strings: &[(String, String)]) -> ResolverContext {
        let mut contexts = Vec::new();
        for (scheme, str) in context_strings {
            let ctx = self.create_context_from_string_with_scheme(scheme, str);
            if !ctx.is_empty() {
                contexts.push(ctx);
            }
        }
        ResolverContext::from_contexts(contexts)
    }

    // ── _RefreshContext (C++ resolver.cpp:770-785) ─────────────────────

    fn refresh_context(&self, context: &ResolverContext) {
        if self.primary.info.implements_contexts {
            self.primary.resolver.refresh_context(context);
        }
        for entry in self.uri_resolvers.values() {
            if entry.info.implements_contexts {
                entry.resolver.refresh_context(context);
            }
        }
    }

    // ── _GetCurrentContext (C++ resolver.cpp:787-818) ──────────────────

    fn get_current_context(&self) -> ResolverContext {
        let mut contexts = Vec::new();

        // C++ collects from all resolvers + internal stack
        if self.primary.info.implements_contexts {
            let ctx = self.primary.resolver.get_current_context();
            if !ctx.is_empty() {
                contexts.push(ctx);
            }
        }
        for entry in self.uri_resolvers.values() {
            if !entry.info.implements_contexts {
                continue;
            }
            let ctx = entry.resolver.get_current_context();
            if !ctx.is_empty() {
                contexts.push(ctx);
            }
        }

        // Internal stack (C++ resolver.cpp:812-815)
        DISPATCHING_CONTEXT_STACK.with(|stack| {
            let s = stack.borrow();
            if let Some(ctx) = s.last() {
                contexts.push(ctx.clone());
            }
        });

        if contexts.len() == 1 {
            return contexts.into_iter().next().unwrap_or_default();
        }
        ResolverContext::from_contexts(contexts)
    }

    // ── _IsContextDependentPath (C++ resolver.cpp:584-599) ─────────────

    fn is_context_dependent_path(&self, asset_path: &str) -> bool {
        let (resolver, info) = self.get_resolver_for_path(asset_path);
        if !info.implements_contexts {
            return false;
        }
        if is_package_relative_path(asset_path) {
            return resolver
                .is_context_dependent_path(&split_package_relative_path_outer(asset_path).0);
        }
        resolver.is_context_dependent_path(asset_path)
    }

    // ── _GetExtension (C++ resolver.cpp:611-629) ───────────────────────

    fn get_extension(&self, asset_path: &str) -> String {
        let (resolver, _) = self.get_resolver_for_path(asset_path);
        if is_package_relative_path(asset_path) {
            // C++ resolver.cpp:624-626: innermost packaged path
            let (_, inner) = split_package_relative_path_inner(asset_path);
            return resolver.get_extension(&inner);
        }
        resolver.get_extension(asset_path)
    }

    // ── _GetAssetInfo (C++ resolver.cpp:857-885) ───────────────────────

    fn get_asset_info(&self, asset_path: &str, resolved_path: &ResolvedPath) -> AssetInfo {
        let (resolver, _) = self.get_resolver_for_path(asset_path);
        if is_package_relative_path(asset_path) {
            let (outer_asset, _) = split_package_relative_path_outer(asset_path);
            let (outer_resolved, _) = split_package_relative_path_outer(resolved_path.as_str());
            return resolver.get_asset_info(&outer_asset, &ResolvedPath::new(outer_resolved));
        }
        resolver.get_asset_info(asset_path, resolved_path)
    }

    // ── _GetModificationTimestamp (C++ resolver.cpp:887-899) ───────────

    fn get_modification_timestamp(
        &self,
        asset_path: &str,
        resolved_path: &ResolvedPath,
    ) -> Timestamp {
        let (resolver, _) = self.get_resolver_for_path(asset_path);
        if is_package_relative_path(asset_path) {
            let (outer_asset, _) = split_package_relative_path_outer(asset_path);
            let (outer_resolved, _) = split_package_relative_path_outer(resolved_path.as_str());
            return resolver
                .get_modification_timestamp(&outer_asset, &ResolvedPath::new(outer_resolved));
        }
        resolver.get_modification_timestamp(asset_path, resolved_path)
    }

    // ── _OpenAsset (C++ resolver.cpp:901-918) ──────────────────────────

    fn open_asset(&self, resolved_path: &ResolvedPath) -> Option<Arc<dyn Asset>> {
        let (resolver, _) = self.get_resolver_for_path(resolved_path.as_str());
        if is_package_relative_path(resolved_path.as_str()) {
            return package_resolver::open_packaged_asset(resolved_path.as_str());
        }
        resolver.open_asset(resolved_path)
    }

    // ── _OpenAssetForWrite (C++ resolver.cpp:920-930) ──────────────────

    fn open_asset_for_write(
        &self,
        resolved_path: &ResolvedPath,
        write_mode: WriteMode,
    ) -> Option<Arc<dyn WritableAsset + Send + Sync>> {
        if is_package_relative_path(resolved_path.as_str()) {
            log::error!("Cannot open package-relative paths for write");
            return None;
        }
        let (resolver, _) = self.get_resolver_for_path(resolved_path.as_str());
        resolver.open_asset_for_write(resolved_path, write_mode)
    }

    // ── _CanWriteAssetToPath (C++ resolver.cpp:932-944) ────────────────

    fn can_write_asset_to_path(
        &self,
        resolved_path: &ResolvedPath,
        why_not: Option<&mut String>,
    ) -> bool {
        if is_package_relative_path(resolved_path.as_str()) {
            if let Some(reason) = why_not {
                *reason = "Cannot open package-relative paths for write".into();
            }
            return false;
        }
        let (resolver, _) = self.get_resolver_for_path(resolved_path.as_str());
        resolver.can_write_asset_to_path(resolved_path, why_not)
    }

    // ── _BeginCacheScope (C++ resolver.cpp:953-1001) ───────────────────

    fn begin_cache_scope(&self) -> Option<Value> {
        if self.primary.info.implements_scoped_caches {
            self.primary.resolver.begin_cache_scope();
        }
        for entry in self.uri_resolvers.values() {
            if entry.info.implements_scoped_caches {
                entry.resolver.begin_cache_scope();
            }
        }
        // Primary always gets begin_cache_scope for its built-in cache
        self.primary.resolver.begin_cache_scope()
    }

    // ── _EndCacheScope (C++ resolver.cpp:1003-1043) ────────────────────

    fn end_cache_scope(&self, cache_scope_data: Option<Value>) {
        if self.primary.info.implements_scoped_caches {
            self.primary.resolver.end_cache_scope(None);
        }
        for entry in self.uri_resolvers.values() {
            if entry.info.implements_scoped_caches {
                entry.resolver.end_cache_scope(None);
            }
        }
        self.primary.resolver.end_cache_scope(cache_scope_data);
    }

    // ── _IsRepositoryPath (C++ resolver.cpp:601-609) ───────────────────

    fn is_repository_path(&self, path: &str) -> bool {
        let (resolver, _) = self.get_resolver_for_path(path);
        if is_package_relative_path(path) {
            return resolver.is_repository_path(&split_package_relative_path_outer(path).0);
        }
        resolver.is_repository_path(path)
    }
}

// ── Internally-managed current context ─────────────────────────────────────

/// Returns the context from the dispatching resolver's thread-local stack.
/// Matches C++ `_DispatchingResolver::GetInternallyManagedCurrentContext`.
pub fn get_internally_managed_current_context() -> Option<ResolverContext> {
    DISPATCHING_CONTEXT_STACK.with(|stack| stack.borrow().last().cloned())
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_uri_scheme_valid() {
        assert!(validate_uri_scheme("http").is_ok());
        assert!(validate_uri_scheme("https").is_ok());
        assert!(validate_uri_scheme("test").is_ok());
        assert!(validate_uri_scheme("test-other").is_ok());
        assert!(validate_uri_scheme("a1b2").is_ok());
        assert!(validate_uri_scheme("x+y.z-w").is_ok());
    }

    #[test]
    fn test_validate_uri_scheme_invalid() {
        assert!(validate_uri_scheme("").is_err());
        assert!(validate_uri_scheme("1test").is_err());
        assert!(validate_uri_scheme("Test").is_err());
        assert!(validate_uri_scheme("test_other").is_err());
        assert!(validate_uri_scheme("test:other").is_err());
    }

    #[test]
    fn test_dispatching_resolver_creates() {
        let dr = DispatchingResolver::new();
        let result = dr.resolve("nonexistent_file.usd");
        assert!(result.is_empty());
    }

    #[test]
    fn test_primary_is_default() {
        let dr = DispatchingResolver::new();
        let (_, info) = dr.get_resolver_for_path("/some/path.usd");
        assert_eq!(info.type_name, "ArDefaultResolver");
    }

    #[test]
    fn test_no_uri_resolvers_by_default() {
        let dr = DispatchingResolver::new();
        assert!(dr.get_uri_schemes().is_empty());
    }

    #[test]
    fn test_unknown_uri_goes_to_primary() {
        let dr = DispatchingResolver::new();
        let (_, info) = dr.get_resolver_for_path("unknown://foo");
        assert_eq!(info.type_name, "ArDefaultResolver");
    }

    #[test]
    fn test_get_available_resolvers_includes_default() {
        let resolvers = get_available_resolvers();
        assert!(resolvers.iter().any(|r| r.type_name == "ArDefaultResolver"));
    }
}
