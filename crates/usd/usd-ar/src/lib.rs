//! Asset Resolution (ar) module.
//!
//! The `ar` module provides the asset resolution system for USD. Asset resolution
//! is the process of converting logical asset paths to physical locations where
//! the asset data can be read.
//!
//! # Key Types
//!
//! - [`ResolvedPath`] - Represents a resolved (physical) asset path
//! - [`Resolver`] - Trait defining the asset resolution interface
//! - [`DefaultResolver`] - Default filesystem-based resolver implementation
//! - [`ResolverContext`] - Context for customizing resolution behavior
//! - [`ResolverContextBinder`] - RAII helper for binding contexts
//! - [`Asset`] - Trait for reading asset data
//! - [`WritableAsset`] - Trait for writing asset data
//!
//! # Architecture
//!
//! The asset resolution system follows these principles:
//!
//! 1. **Logical vs Physical Paths**: Logical paths are the paths that appear in
//!    USD files (e.g., `"./model.usd"`). Physical paths are the actual filesystem
//!    paths where data is stored (e.g., `"/assets/characters/model.usd"`).
//!
//! 2. **Resolver Registration**: Custom resolvers register via
//!    [`define_resolver::define_resolver`](define_resolver::define_resolver)
//!    (matches C++ `AR_DEFINE_RESOLVER`). Discovery uses [`TfType`](usd_tf::TfType).
//!
//! 3. **Context-Sensitive Resolution**: Resolution can be customized per-thread
//!    by binding [`ResolverContext`] objects containing context-specific data.
//!
//! # Examples
//!
//! ## Basic Resolution
//!
//! ```ignore
//! use usd_ar::{get_resolver, ResolvedPath};
//!
//! // Get the global resolver
//! let resolver = get_resolver();
//! let resolver = resolver.read().expect("rwlock poisoned");
//!
//! // Resolve an asset path
//! let path = resolver.resolve("/path/to/asset.usd");
//! if !path.is_empty() {
//!     println!("Resolved to: {}", path);
//! }
//! ```
//!
//! ## Using Contexts
//!
//! ```ignore
//! use usd_ar::{
//!     get_resolver, ResolverContext, ResolverContextBinder, DefaultResolverContext
//! };
//!
//! // Create a context with custom search paths
//! let ctx = ResolverContext::with_object(
//!     DefaultResolverContext::new(vec!["/assets/characters".into()])
//! );
//!
//! // Bind the context for this scope
//! let _binder = ResolverContextBinder::new(ctx);
//!
//! // Resolution will now search /assets/characters first
//! let resolver = get_resolver().read().expect("rwlock poisoned");
//! let path = resolver.resolve("model.usd");
//! ```
//!
//! ## Reading Assets
//!
//! ```ignore
//! use usd_ar::{get_resolver, Asset};
//!
//! let resolver = get_resolver().read().expect("rwlock poisoned");
//! let resolved = resolver.resolve("/path/to/asset.usd");
//!
//! if let Some(asset) = resolver.open_asset(&resolved) {
//!     let size = asset.size();
//!     let buffer = asset.get_buffer();
//!     // Process asset data...
//! }
//! ```

pub mod asset;
pub mod asset_info;
pub mod define_resolver;
pub mod dispatching_resolver;
pub mod filesystem_asset;
pub mod notice;
pub mod package_resolver;
pub mod package_utils;
pub mod resolved_path;
pub mod resolver;
pub mod resolver_context;
pub mod resolver_context_binder;
pub mod resolver_registry;
pub mod thread_local_scoped_cache;
pub mod timestamp;
pub mod writable_asset;

// Re-export main types
pub use asset::{Asset, AssetReader, InMemoryAsset, RawFileDescriptor};
pub use asset_info::AssetInfo;
pub use define_resolver::{ArResolver, ResolverMeta};
pub use dispatching_resolver::{DispatchingResolver, validate_uri_scheme};
pub use filesystem_asset::FilesystemAsset;
pub use notice::ResolverChangedNotice;
pub use package_resolver::{
    PackageResolver, PackageResolverRegistry, open_packaged_asset, register_package_resolver,
    resolve_packaged_path,
};
pub use package_utils::{
    PACKAGE_DELIMITER, PACKAGE_DELIMITER_CLOSE, escape_package_delimiter, is_package_relative_path,
    join_package_relative_path, join_package_relative_path_pair, split_package_relative_path_inner,
    split_package_relative_path_outer, unescape_package_delimiter,
};
pub use resolved_path::ResolvedPath;
pub use resolver::{
    DefaultResolver, Resolver, create_resolver, get_available_resolvers, get_resolver,
    set_preferred_resolver, set_resolver,
};
pub use resolver_context::{ContextObject, DefaultResolverContext, ResolverContext};
pub use resolver_context_binder::{ResolverContextBinder, ResolverScopedCache};
pub use resolver_registry::{ResolverFactory, ResolverInfo, ResolverRegistry};
pub use thread_local_scoped_cache::{CachePtr, ThreadLocalScopedCache};
pub use timestamp::Timestamp;
pub use writable_asset::{
    FilesystemWritableAsset, InMemoryWritableAsset, WritableAsset, WritableAssetWriter, WriteMode,
};

/// AR module version.
pub const AR_VERSION: u32 = 2;

// From<ResolverContext> lives here (usd-ar) to avoid the circular dep:
// usd-vt -> usd-ar -> usd-vt.
impl From<ResolverContext> for usd_vt::Value {
    #[inline]
    fn from(value: ResolverContext) -> Self {
        Self::new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_tf::TfType;

    #[test]
    fn test_ar_version() {
        assert_eq!(AR_VERSION, 2);
    }

    #[test]
    fn test_exports() {
        // Verify all exports are accessible
        let _path = ResolvedPath::empty();
        let _timestamp = Timestamp::invalid();
        let _info = AssetInfo::new();
        let _ctx = ResolverContext::new();
        let _default_ctx = DefaultResolverContext::empty();
        let _asset = InMemoryAsset::empty();
        let _writable = InMemoryWritableAsset::new();
        let _notice = ResolverChangedNotice::new();
        let _mode = WriteMode::Replace;
        let _registry = PackageResolverRegistry::new();
    }

    #[test]
    fn test_get_available_resolvers() {
        let types = get_available_resolvers();
        assert!(!types.is_empty());
        assert!(types.iter().any(|t| t.type_name() == "ArDefaultResolver"));
    }

    #[test]
    fn test_create_resolver() {
        let t = TfType::find_by_name("ArDefaultResolver");
        let resolver = create_resolver(t);
        let _ = resolver.resolve("model.usd"); // smoke test
    }
}
