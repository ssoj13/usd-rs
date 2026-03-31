
//! HdSt_DrawItemsCache - Caching mechanism for filtered draw items.
//!
//! Provides a cache of draw items per (collection, renderTags) query.
//! Owned by the Storm render delegate; queried by render passes.
//! Ported from drawItemsCache.h.
//!
//! # Performance
//!
//! Caching is useful when multiple tasks use the same query:
//! - Multiple viewers with similar Hydra task sets
//! - Shadow map generation reusing the same shadow caster prims

use crate::draw_item::HdStDrawItemSharedPtr;
use crate::render_param::HdStRenderParam;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use usd_hd::render::{HdRenderIndex, HdRprimCollection};
use usd_tf::Token;

/// Shared vector of draw item pointers.
pub type HdDrawItemVecSharedPtr = Arc<Vec<HdStDrawItemSharedPtr>>;

/// Cached entry with version tracking for staleness detection.
#[derive(Debug)]
struct CacheValue {
    /// Cached draw items
    draw_items: HdDrawItemVecSharedPtr,
    /// Collection version when cached (matches HdChangeTracker u32)
    collection_version: u32,
    /// Render tags version when cached (matches HdChangeTracker u32)
    render_tags_version: u32,
    /// Material tags version from HdStRenderParam
    material_tags_version: usize,
    /// Geom subset draw items version from HdStRenderParam
    geom_subset_draw_items_version: usize,
}

impl Default for CacheValue {
    fn default() -> Self {
        Self {
            draw_items: Arc::new(Vec::new()),
            collection_version: 0,
            render_tags_version: 0,
            material_tags_version: 0,
            geom_subset_draw_items_version: 0,
        }
    }
}

/// Draw items cache for Storm render passes.
///
/// Caches filtered draw item vectors keyed by (collection, render tags).
/// Render passes call `get_draw_items()` to obtain shared pointers to
/// up-to-date draw item vectors.
#[derive(Debug, Default)]
pub struct HdStDrawItemsCache {
    cache: HashMap<u64, CacheValue>,
}

/// Pointer type for the draw items cache.
pub type HdStDrawItemsCachePtr = *mut HdStDrawItemsCache;

impl HdStDrawItemsCache {
    /// Create a new empty cache.
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    /// Get cached draw items for the given collection and render tags.
    ///
    /// Get cached draw items, or update cache with freshly queried items.
    ///
    /// Checks staleness via change tracker + render param version counters.
    /// `fetch_draw_items` closure is called only on cache miss or stale entry;
    /// caller queries render index and downcasts results to HdStDrawItem.
    /// Matches C++ HdSt_DrawItemsCache::GetDrawItems.
    pub fn get_draw_items(
        &mut self,
        collection: &HdRprimCollection,
        render_tags: &[Token],
        render_index: &HdRenderIndex,
        render_param: Option<&HdStRenderParam>,
        fetch_draw_items: impl FnOnce() -> Vec<HdStDrawItemSharedPtr>,
    ) -> HdDrawItemVecSharedPtr {
        let key = Self::make_key(collection, render_tags);

        let cache_miss = !self.cache.contains_key(&key);
        if cache_miss {
            self.cache.insert(key, CacheValue::default());
        }

        let stale_entry = !cache_miss && {
            let val = self.cache.get(&key).unwrap();
            Self::is_cache_entry_stale(val, &collection.name, render_index, render_param)
        };

        if cache_miss || stale_entry {
            let items = fetch_draw_items();
            Self::update_cache_entry(
                self.cache.get_mut(&key).unwrap(),
                collection,
                render_index,
                render_param,
                items,
            );
        }

        Arc::clone(&self.cache.get(&key).unwrap().draw_items)
    }

    /// Check if a cache entry is stale by comparing version counters
    /// against the change tracker and render param.
    fn is_cache_entry_stale(
        val: &CacheValue,
        collection_name: &Token,
        render_index: &HdRenderIndex,
        render_param: Option<&HdStRenderParam>,
    ) -> bool {
        let tracker = render_index.get_change_tracker();
        if val.collection_version != tracker.get_collection_version(&collection_name) {
            return true;
        }
        if val.render_tags_version != tracker.get_render_tag_version() {
            return true;
        }
        if let Some(rp) = render_param {
            if val.material_tags_version != rp.get_material_tags_version() {
                return true;
            }
            if val.geom_subset_draw_items_version != rp.get_geom_subset_draw_items_version() {
                return true;
            }
        }
        false
    }

    /// Re-query the render index and update a cache entry with fresh
    /// draw items and version stamps. Caller provides the new draw items
    /// (already downcast from render index results).
    fn update_cache_entry(
        val: &mut CacheValue,
        collection: &HdRprimCollection,
        render_index: &HdRenderIndex,
        render_param: Option<&HdStRenderParam>,
        draw_items: Vec<HdStDrawItemSharedPtr>,
    ) {
        let tracker = render_index.get_change_tracker();
        val.collection_version = tracker.get_collection_version(&collection.name);
        val.render_tags_version = tracker.get_render_tag_version();
        if let Some(rp) = render_param {
            val.material_tags_version = rp.get_material_tags_version();
            val.geom_subset_draw_items_version = rp.get_geom_subset_draw_items_version();
        }
        val.draw_items = Arc::new(draw_items);
    }

    /// Remove cache entries no longer referenced by any render pass.
    ///
    /// Called by the render delegate during CommitResources.
    pub fn garbage_collect(&mut self) {
        // Remove entries where the draw items Arc has only 1 strong ref (ours)
        self.cache
            .retain(|_, v| Arc::strong_count(&v.draw_items) > 1);
    }

    /// Clear the entire cache.
    pub fn clear(&mut self) {
        self.cache.clear();
    }

    /// Number of cached entries.
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Whether cache is empty.
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    // Compute a hash key from collection + render tags.
    fn make_key(collection: &HdRprimCollection, render_tags: &[Token]) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        collection.hash(&mut hasher);
        render_tags.hash(&mut hasher);
        hasher.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_new_empty() {
        let cache = HdStDrawItemsCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_garbage_collect_empty() {
        let mut cache = HdStDrawItemsCache::new();
        cache.garbage_collect(); // should not panic
        assert!(cache.is_empty());
    }

    #[test]
    fn test_clear() {
        let mut cache = HdStDrawItemsCache::new();
        cache.clear();
        assert!(cache.is_empty());
    }
}
