//! SurfaceFactoryCache — cache of reusable irregular patch representations.
//!
//! Mirrors `Bfr::SurfaceFactoryCache` from `surfaceFactoryCache.h/cpp`.

use std::collections::HashMap;
use std::sync::RwLock;

use super::irregular_patch_type::IrregularPatchSharedPtr;

/// Key type used to look up cached patch trees.
pub type CacheKey = u64;

/// Value type stored in the cache (shared ref-counted patch tree).
pub type CacheData = IrregularPatchSharedPtr;

// ---------------------------------------------------------------------------
// SurfaceFactoryCache
// ---------------------------------------------------------------------------

/// Cache for reusable irregular patch representations.
///
/// Mirrors `Bfr::SurfaceFactoryCache` (single-threaded variant).
///
/// Access is restricted to `SurfaceFactory` and its builders. Public
/// construction is available so that an external cache can be shared
/// between multiple factories.
pub struct SurfaceFactoryCache {
    map: HashMap<CacheKey, CacheData>,
}

impl Default for SurfaceFactoryCache {
    fn default() -> Self {
        SurfaceFactoryCache { map: HashMap::new() }
    }
}

impl SurfaceFactoryCache {
    pub fn new() -> Self { Self::default() }

    /// Number of entries in the cache.
    pub fn size(&self) -> usize { self.map.len() }

    /// Look up `key` in the cache.  Returns `None` when not found.
    pub fn find(&self, key: CacheKey) -> Option<CacheData> {
        self.map.get(&key).cloned()
    }

    /// Insert `data` for `key`.  If an entry already exists (inserted
    /// concurrently by another thread), the existing entry is returned
    /// instead of inserting the new one.
    pub fn add(&mut self, key: CacheKey, data: CacheData) -> CacheData {
        self.map.entry(key).or_insert(data).clone()
    }
}

// ---------------------------------------------------------------------------
// SurfaceFactoryCacheThreaded
// ---------------------------------------------------------------------------

/// Thread-safe variant of `SurfaceFactoryCache` backed by an `RwLock`.
///
/// Mirrors `Bfr::SurfaceFactoryCacheThreaded<std::shared_mutex, ...>`.
pub struct SurfaceFactoryCacheThreaded {
    inner: RwLock<SurfaceFactoryCache>,
}

impl Default for SurfaceFactoryCacheThreaded {
    fn default() -> Self {
        SurfaceFactoryCacheThreaded {
            inner: RwLock::new(SurfaceFactoryCache::new()),
        }
    }
}

impl SurfaceFactoryCacheThreaded {
    pub fn new() -> Self { Self::default() }

    pub fn size(&self) -> usize {
        self.inner.read().unwrap().size()
    }

    /// Thread-safe lookup.
    pub fn find(&self, key: CacheKey) -> Option<CacheData> {
        self.inner.read().unwrap().find(key)
    }

    /// Thread-safe insertion.
    pub fn add(&self, key: CacheKey, data: CacheData) -> CacheData {
        self.inner.write().unwrap().add(key, data)
    }
}

// ---------------------------------------------------------------------------
// Trait for unified access
// ---------------------------------------------------------------------------

/// Trait allowing both `SurfaceFactoryCache` and `SurfaceFactoryCacheThreaded`
/// to be used polymorphically from `SurfaceFactory`.
pub trait SurfaceFactoryCacheTrait: Send + Sync {
    fn find(&self, key: CacheKey) -> Option<CacheData>;
    fn add(&self, key: CacheKey, data: CacheData) -> CacheData;
}

// Wrap the non-threaded version behind an RwLock for the trait impl:
impl SurfaceFactoryCacheTrait for RwLock<SurfaceFactoryCache> {
    fn find(&self, key: CacheKey) -> Option<CacheData> {
        self.read().unwrap().find(key)
    }
    fn add(&self, key: CacheKey, data: CacheData) -> CacheData {
        self.write().unwrap().add(key, data)
    }
}

impl SurfaceFactoryCacheTrait for SurfaceFactoryCacheThreaded {
    fn find(&self, key: CacheKey) -> Option<CacheData> {
        SurfaceFactoryCacheThreaded::find(self, key)
    }
    fn add(&self, key: CacheKey, data: CacheData) -> CacheData {
        SurfaceFactoryCacheThreaded::add(self, key, data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::bfr::patch_tree::PatchTree;

    fn make_patch() -> CacheData {
        Arc::new(PatchTree::new())
    }

    #[test]
    fn cache_find_miss() {
        let cache = SurfaceFactoryCache::new();
        assert!(cache.find(42).is_none());
    }

    #[test]
    fn cache_add_and_find() {
        let mut cache = SurfaceFactoryCache::new();
        let data = make_patch();
        cache.add(1u64, data.clone());
        assert!(cache.find(1).is_some());
        assert!(cache.find(2).is_none());
        assert_eq!(cache.size(), 1);
    }

    #[test]
    fn threaded_cache_find_miss() {
        let cache = SurfaceFactoryCacheThreaded::new();
        assert!(cache.find(42).is_none());
    }

    #[test]
    fn threaded_cache_add_and_find() {
        let cache = SurfaceFactoryCacheThreaded::new();
        let data  = make_patch();
        cache.add(7u64, data);
        assert!(cache.find(7).is_some());
    }
}
