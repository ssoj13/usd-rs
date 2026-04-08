//! Collection membership query cache for UsdImaging.
//!
//! Port of pxr/usdImaging/usdImaging/collectionCache.h
//!
//! Provides caching for expensive collection membership queries to avoid
//! redundant computation when querying whether a prim is a member of
//! a collection at specific time codes.

use parking_lot::RwLock;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use usd_core::time_code::TimeCode;
use usd_sdf::Path;

// ============================================================================
// CacheKey
// ============================================================================

/// Key for collection membership cache lookups.
///
/// Combines collection path, prim path, and time code for unique identification
/// of a cached query result.
#[derive(Clone, Debug, PartialEq, Eq)]
struct CacheKey {
    /// Path to the collection
    collection_path: Path,
    /// Path to the prim being queried
    prim_path: Path,
    /// Time code for the query
    time: TimeCode,
}

impl Hash for CacheKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.collection_path.hash(state);
        self.prim_path.hash(state);
        // Hash time as bits for consistent hashing
        self.time.value().to_bits().hash(state);
    }
}

// ============================================================================
// CollectionCache
// ============================================================================

/// Cache for collection membership queries.
///
/// Matches C++ `UsdImaging_CollectionCache`.
///
/// This cache stores the results of expensive collection membership queries
/// to avoid redundant computation. Collection membership can be time-varying,
/// so the cache is keyed by collection path, prim path, and time code.
///
/// The cache is thread-safe using RwLock for concurrent read access.
///
/// # Examples
///
/// ```
/// use usd_sdf::Path;
/// use usd_core::time_code::TimeCode;
/// use usd_imaging::CollectionCache;
///
/// let cache = CollectionCache::new();
/// let collection_path = Path::from_string("/Collections/MyCollection").unwrap();
/// let prim_path = Path::from_string("/World/Cube").unwrap();
/// let time = TimeCode::default();
///
/// // Insert a query result
/// cache.insert(&collection_path, &prim_path, time, true);
///
/// // Query the result
/// assert_eq!(cache.query(&collection_path, &prim_path, time), Some(true));
///
/// // Clear the cache
/// cache.clear();
/// assert_eq!(cache.query(&collection_path, &prim_path, time), None);
/// ```
pub struct CollectionCache {
    /// Internal cache storage
    cache: RwLock<HashMap<CacheKey, bool>>,
}

impl CollectionCache {
    /// Creates a new empty collection cache.
    ///
    /// Matches C++ `UsdImaging_CollectionCache()`.
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// Queries whether a prim is a member of a collection at a specific time.
    ///
    /// Returns `Some(true)` if cached as member, `Some(false)` if cached as non-member,
    /// or `None` if not cached.
    ///
    /// # Arguments
    ///
    /// * `collection_path` - Path to the collection
    /// * `prim_path` - Path to the prim being queried
    /// * `time` - Time code for the query
    pub fn query(&self, collection_path: &Path, prim_path: &Path, time: TimeCode) -> Option<bool> {
        let key = CacheKey {
            collection_path: collection_path.clone(),
            prim_path: prim_path.clone(),
            time,
        };

        let cache = self.cache.read();
        cache.get(&key).copied()
    }

    /// Inserts a collection membership query result into the cache.
    ///
    /// # Arguments
    ///
    /// * `collection_path` - Path to the collection
    /// * `prim_path` - Path to the prim being queried
    /// * `time` - Time code for the query
    /// * `is_member` - Whether the prim is a member of the collection
    pub fn insert(
        &self,
        collection_path: &Path,
        prim_path: &Path,
        time: TimeCode,
        is_member: bool,
    ) {
        let key = CacheKey {
            collection_path: collection_path.clone(),
            prim_path: prim_path.clone(),
            time,
        };

        let mut cache = self.cache.write();
        cache.insert(key, is_member);
    }

    /// Removes a specific cache entry.
    ///
    /// Returns `true` if an entry was removed, `false` if no entry existed.
    ///
    /// # Arguments
    ///
    /// * `collection_path` - Path to the collection
    /// * `prim_path` - Path to the prim being queried
    /// * `time` - Time code for the query
    pub fn remove(&self, collection_path: &Path, prim_path: &Path, time: TimeCode) -> bool {
        let key = CacheKey {
            collection_path: collection_path.clone(),
            prim_path: prim_path.clone(),
            time,
        };

        let mut cache = self.cache.write();
        cache.remove(&key).is_some()
    }

    /// Clears all entries from the cache.
    ///
    /// Matches C++ `Clear()`.
    pub fn clear(&self) {
        let mut cache = self.cache.write();
        cache.clear();
    }

    /// Invalidates all cached entries for a specific collection.
    ///
    /// Removes all cache entries where the collection path matches.
    ///
    /// # Arguments
    ///
    /// * `collection_path` - Path to the collection to invalidate
    pub fn invalidate_collection(&self, collection_path: &Path) {
        let mut cache = self.cache.write();
        cache.retain(|key, _| &key.collection_path != collection_path);
    }

    /// Invalidates all cached entries for a specific prim.
    ///
    /// Removes all cache entries where the prim path matches.
    ///
    /// # Arguments
    ///
    /// * `prim_path` - Path to the prim to invalidate
    pub fn invalidate_prim(&self, prim_path: &Path) {
        let mut cache = self.cache.write();
        cache.retain(|key, _| &key.prim_path != prim_path);
    }

    /// Invalidates all cached entries within a time range.
    ///
    /// Removes all cache entries where the time code falls within the specified range.
    ///
    /// # Arguments
    ///
    /// * `start_time` - Start of the time range (inclusive)
    /// * `end_time` - End of the time range (inclusive)
    pub fn invalidate_time_range(&self, start_time: TimeCode, end_time: TimeCode) {
        let start_val = start_time.value();
        let end_val = end_time.value();

        let mut cache = self.cache.write();
        cache.retain(|key, _| {
            let time_val = key.time.value();
            time_val < start_val || time_val > end_val
        });
    }

    /// Returns the number of entries in the cache.
    pub fn size(&self) -> usize {
        let cache = self.cache.read();
        cache.len()
    }

    /// Returns whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        let cache = self.cache.read();
        cache.is_empty()
    }

    /// Compute all collections that contain the given prim path.
    ///
    /// Returns a list of collection path tokens for light linking / category
    /// filtering. Matches C++ `ComputeCollectionsContainingPath()`.
    pub fn compute_collections_containing_path(&self, prim_path: &Path) -> Vec<usd_tf::Token> {
        let cache = self.cache.read();
        let mut result = Vec::new();

        for (key, &is_member) in cache.iter() {
            if &key.prim_path == prim_path && is_member {
                result.push(usd_tf::Token::new(key.collection_path.as_str()));
            }
        }

        result.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        result
    }
}

impl Default for CollectionCache {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_cache_is_empty() {
        let cache = CollectionCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.size(), 0);
    }

    #[test]
    fn test_insert_and_query() {
        let cache = CollectionCache::new();
        let collection = Path::from_string("/Collections/Set1").unwrap();
        let prim = Path::from_string("/World/Cube").unwrap();
        let time = TimeCode::default();

        // Should return None before insertion
        assert_eq!(cache.query(&collection, &prim, time), None);

        // Insert and query
        cache.insert(&collection, &prim, time, true);
        assert_eq!(cache.query(&collection, &prim, time), Some(true));
        assert_eq!(cache.size(), 1);

        // Insert false value
        cache.insert(&collection, &prim, time, false);
        assert_eq!(cache.query(&collection, &prim, time), Some(false));
        assert_eq!(cache.size(), 1); // Should update, not add new entry
    }

    #[test]
    fn test_different_times() {
        let cache = CollectionCache::new();
        let collection = Path::from_string("/Collections/Set1").unwrap();
        let prim = Path::from_string("/World/Cube").unwrap();

        let time0 = TimeCode::new(0.0);
        let time1 = TimeCode::new(1.0);

        cache.insert(&collection, &prim, time0, true);
        cache.insert(&collection, &prim, time1, false);

        assert_eq!(cache.query(&collection, &prim, time0), Some(true));
        assert_eq!(cache.query(&collection, &prim, time1), Some(false));
        assert_eq!(cache.size(), 2);
    }

    #[test]
    fn test_remove() {
        let cache = CollectionCache::new();
        let collection = Path::from_string("/Collections/Set1").unwrap();
        let prim = Path::from_string("/World/Cube").unwrap();
        let time = TimeCode::default();

        cache.insert(&collection, &prim, time, true);
        assert_eq!(cache.size(), 1);

        // Remove existing entry
        assert!(cache.remove(&collection, &prim, time));
        assert_eq!(cache.query(&collection, &prim, time), None);
        assert_eq!(cache.size(), 0);

        // Remove non-existing entry
        assert!(!cache.remove(&collection, &prim, time));
    }

    #[test]
    fn test_clear() {
        let cache = CollectionCache::new();
        let collection = Path::from_string("/Collections/Set1").unwrap();
        let prim1 = Path::from_string("/World/Cube").unwrap();
        let prim2 = Path::from_string("/World/Sphere").unwrap();
        let time = TimeCode::default();

        cache.insert(&collection, &prim1, time, true);
        cache.insert(&collection, &prim2, time, false);
        assert_eq!(cache.size(), 2);

        cache.clear();
        assert!(cache.is_empty());
        assert_eq!(cache.query(&collection, &prim1, time), None);
        assert_eq!(cache.query(&collection, &prim2, time), None);
    }

    #[test]
    fn test_invalidate_collection() {
        let cache = CollectionCache::new();
        let collection1 = Path::from_string("/Collections/Set1").unwrap();
        let collection2 = Path::from_string("/Collections/Set2").unwrap();
        let prim = Path::from_string("/World/Cube").unwrap();
        let time = TimeCode::default();

        cache.insert(&collection1, &prim, time, true);
        cache.insert(&collection2, &prim, time, false);
        assert_eq!(cache.size(), 2);

        cache.invalidate_collection(&collection1);
        assert_eq!(cache.size(), 1);
        assert_eq!(cache.query(&collection1, &prim, time), None);
        assert_eq!(cache.query(&collection2, &prim, time), Some(false));
    }

    #[test]
    fn test_invalidate_prim() {
        let cache = CollectionCache::new();
        let collection = Path::from_string("/Collections/Set1").unwrap();
        let prim1 = Path::from_string("/World/Cube").unwrap();
        let prim2 = Path::from_string("/World/Sphere").unwrap();
        let time = TimeCode::default();

        cache.insert(&collection, &prim1, time, true);
        cache.insert(&collection, &prim2, time, false);
        assert_eq!(cache.size(), 2);

        cache.invalidate_prim(&prim1);
        assert_eq!(cache.size(), 1);
        assert_eq!(cache.query(&collection, &prim1, time), None);
        assert_eq!(cache.query(&collection, &prim2, time), Some(false));
    }

    #[test]
    fn test_invalidate_time_range() {
        let cache = CollectionCache::new();
        let collection = Path::from_string("/Collections/Set1").unwrap();
        let prim = Path::from_string("/World/Cube").unwrap();

        cache.insert(&collection, &prim, TimeCode::new(0.0), true);
        cache.insert(&collection, &prim, TimeCode::new(1.0), true);
        cache.insert(&collection, &prim, TimeCode::new(2.0), false);
        cache.insert(&collection, &prim, TimeCode::new(3.0), false);
        assert_eq!(cache.size(), 4);

        // Invalidate times 1.0 to 2.0 (inclusive)
        cache.invalidate_time_range(TimeCode::new(1.0), TimeCode::new(2.0));
        assert_eq!(cache.size(), 2);

        // Times 0.0 and 3.0 should remain
        assert_eq!(
            cache.query(&collection, &prim, TimeCode::new(0.0)),
            Some(true)
        );
        assert_eq!(cache.query(&collection, &prim, TimeCode::new(1.0)), None);
        assert_eq!(cache.query(&collection, &prim, TimeCode::new(2.0)), None);
        assert_eq!(
            cache.query(&collection, &prim, TimeCode::new(3.0)),
            Some(false)
        );
    }

    #[test]
    fn test_thread_safety() {
        use std::sync::Arc;
        use std::thread;

        let cache = Arc::new(CollectionCache::new());
        let collection = Path::from_string("/Collections/Set1").unwrap();
        let prim = Path::from_string("/World/Cube").unwrap();

        // Spawn multiple threads to test concurrent access
        let mut handles = vec![];

        for i in 0..10 {
            let cache_clone = Arc::clone(&cache);
            let collection_clone = collection.clone();
            let prim_clone = prim.clone();

            let handle = thread::spawn(move || {
                let time = TimeCode::new(i as f64);
                cache_clone.insert(&collection_clone, &prim_clone, time, i % 2 == 0);

                // Read back the value
                let result = cache_clone.query(&collection_clone, &prim_clone, time);
                assert_eq!(result, Some(i % 2 == 0));
            });

            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(cache.size(), 10);
    }
}
