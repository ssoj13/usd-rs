//! USDZ package resolver.
//!
//! Port of pxr/usd/sdf/usdzResolver.h
//!
//! Package resolver responsible for resolving assets within .usdz files.
//! Also provides a thread-local scoped cache for zip file access.

use crate::zip_file::ZipFile;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// USDZ package resolver.
///
/// Resolves asset paths within .usdz (zip) packages. When given a package
/// path and a packaged path (the path within the archive), it locates and
/// opens the asset from the zip file.
pub struct UsdzResolver;

impl UsdzResolver {
    /// Creates a new USDZ resolver.
    pub fn new() -> Self {
        Self
    }

    /// Resolves a path within a USDZ package.
    ///
    /// Returns the resolved path string if the asset exists in the package.
    pub fn resolve(&self, package_path: &str, packaged_path: &str) -> Option<String> {
        let zip = ZipFile::open(package_path).ok()?;
        if zip.find(packaged_path).is_some() {
            Some(format!("{}[{}]", package_path, packaged_path))
        } else {
            None
        }
    }

    /// Opens an asset within a USDZ package.
    ///
    /// Returns the raw bytes of the asset as a newly allocated Vec.
    pub fn open_asset(&self, package_path: &str, packaged_path: &str) -> Option<Vec<u8>> {
        let zip = ZipFile::open(package_path).ok()?;
        zip.get_file_data(packaged_path).map(|data| data.to_vec())
    }
}

impl Default for UsdzResolver {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-local scoped cache for USDZ zip file access.
///
/// Caches opened zip files while a cache scope is active, avoiding
/// repeated I/O for the same .usdz package.
pub struct UsdzResolverCache {
    /// Cache of opened zip files keyed by package path.
    cache: Mutex<HashMap<String, Arc<ZipFile>>>,
    /// Scope nesting depth. Files are cached while > 0.
    active: Mutex<u32>,
}

impl UsdzResolverCache {
    /// Creates a new resolver cache.
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
            active: Mutex::new(0),
        }
    }

    /// Returns a globally shared instance.
    pub fn instance() -> &'static Self {
        use once_cell::sync::Lazy;
        static INSTANCE: Lazy<UsdzResolverCache> = Lazy::new(UsdzResolverCache::new);
        &INSTANCE
    }

    /// Finds or opens a zip file for the given package path.
    ///
    /// If a cache scope is active, the result is cached.
    pub fn find_or_open_zip_file(&self, package_path: &str) -> Option<Arc<ZipFile>> {
        let active = *self.active.lock().unwrap();
        if active > 0 {
            let mut cache = self.cache.lock().unwrap();
            if let Some(zip) = cache.get(package_path) {
                return Some(Arc::clone(zip));
            }
            let zip = Arc::new(ZipFile::open(package_path).ok()?);
            cache.insert(package_path.to_string(), Arc::clone(&zip));
            Some(zip)
        } else {
            Some(Arc::new(ZipFile::open(package_path).ok()?))
        }
    }

    /// Opens a cache scope. While active, zip files are cached.
    pub fn begin_cache_scope(&self) {
        let mut active = self.active.lock().unwrap();
        *active += 1;
    }

    /// Closes a cache scope. When all scopes are closed, cached
    /// zip files are dropped.
    pub fn end_cache_scope(&self) {
        let mut active = self.active.lock().unwrap();
        if *active > 0 {
            *active -= 1;
        }
        if *active == 0 {
            self.cache.lock().unwrap().clear();
        }
    }
}

impl Default for UsdzResolverCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolver_default() {
        let resolver = UsdzResolver::new();
        // Resolving a non-existent file should return None.
        assert!(resolver.resolve("nonexistent.usdz", "default.usda").is_none());
    }

    #[test]
    fn test_cache_scope() {
        let cache = UsdzResolverCache::new();
        cache.begin_cache_scope();
        cache.begin_cache_scope();
        cache.end_cache_scope();
        // Still one scope active, cache should not be cleared.
        cache.end_cache_scope();
        // All scopes closed now.
    }
}
