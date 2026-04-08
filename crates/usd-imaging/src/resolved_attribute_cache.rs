//! Resolved attribute value cache for UsdImaging.
//!
//! Port of pxr/usdImaging/usdImaging/resolvedAttributeCache.h
//!
//! Provides caching for resolved attribute values to avoid redundant queries
//! and value resolution when accessing prim attributes at specific time codes.

use parking_lot::RwLock;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use usd_core::time_code::TimeCode;
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Value;

// ============================================================================
// CacheKey
// ============================================================================

/// Key for resolved attribute cache lookups.
///
/// Combines prim path, attribute name, and time code for unique identification
/// of a cached attribute value.
#[derive(Clone, Debug, PartialEq, Eq)]
struct CacheKey {
    /// Path to the prim
    prim_path: Path,
    /// Name of the attribute
    attr_name: Token,
    /// Time code for the query
    time: TimeCode,
}

impl Hash for CacheKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Hash::hash(&self.prim_path, state);
        Hash::hash(&self.attr_name, state);
        // Hash time as bits for consistent hashing
        self.time.value().to_bits().hash(state);
    }
}

// ============================================================================
// ResolvedAttributeCache
// ============================================================================

/// Cache for resolved attribute values.
///
/// Matches C++ `UsdImaging_ResolvedAttributeCache`.
///
/// This cache stores resolved attribute values for prims to avoid redundant
/// queries and value resolution. Attribute values can be time-varying, so the
/// cache is keyed by prim path, attribute name, and time code.
///
/// The cache is thread-safe using RwLock for concurrent read access.
///
/// # Examples
///
/// ```
/// use usd_sdf::Path;
/// use usd_core::time_code::TimeCode;
/// use usd_imaging::ResolvedAttributeCache;
/// use usd_tf::Token;
/// use usd_vt::Value;
///
/// let cache = ResolvedAttributeCache::new();
/// let prim_path = Path::from_string("/World/Cube").unwrap();
/// let attr_name = Token::new("size");
/// let time = TimeCode::default();
///
/// // Store attribute value
/// let value = Value::from(1.0f64);
/// cache.set(&prim_path, &attr_name, time, value.clone());
///
/// // Retrieve attribute value
/// assert_eq!(cache.get(&prim_path, &attr_name, time), Some(value));
///
/// // Remove attribute value
/// assert!(cache.remove(&prim_path, &attr_name, time));
/// assert_eq!(cache.get(&prim_path, &attr_name, time), None);
/// ```
pub struct ResolvedAttributeCache {
    /// Internal cache storage
    cache: RwLock<HashMap<CacheKey, Value>>,
}

impl ResolvedAttributeCache {
    /// Creates a new empty resolved attribute cache.
    ///
    /// Matches C++ `UsdImaging_ResolvedAttributeCache()`.
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// Retrieves a resolved attribute value for a prim at a specific time.
    ///
    /// Returns `Some(value)` if cached, or `None` if not cached.
    ///
    /// # Arguments
    ///
    /// * `prim_path` - Path to the prim
    /// * `attr_name` - Name of the attribute
    /// * `time` - Time code for the query
    pub fn get(&self, prim_path: &Path, attr_name: &Token, time: TimeCode) -> Option<Value> {
        let key = CacheKey {
            prim_path: prim_path.clone(),
            attr_name: attr_name.clone(),
            time,
        };

        let cache = self.cache.read();
        cache.get(&key).cloned()
    }

    /// Stores a resolved attribute value for a prim at a specific time.
    ///
    /// # Arguments
    ///
    /// * `prim_path` - Path to the prim
    /// * `attr_name` - Name of the attribute
    /// * `time` - Time code for the query
    /// * `value` - Attribute value to cache
    pub fn set(&self, prim_path: &Path, attr_name: &Token, time: TimeCode, value: Value) {
        let key = CacheKey {
            prim_path: prim_path.clone(),
            attr_name: attr_name.clone(),
            time,
        };

        let mut cache = self.cache.write();
        cache.insert(key, value);
    }

    /// Removes a cached attribute value for a prim at a specific time.
    ///
    /// Returns `true` if an entry was removed, `false` if no entry existed.
    ///
    /// # Arguments
    ///
    /// * `prim_path` - Path to the prim
    /// * `attr_name` - Name of the attribute
    /// * `time` - Time code for the query
    pub fn remove(&self, prim_path: &Path, attr_name: &Token, time: TimeCode) -> bool {
        let key = CacheKey {
            prim_path: prim_path.clone(),
            attr_name: attr_name.clone(),
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

    /// Invalidates all cached entries for prims under a specific path.
    ///
    /// Removes all cache entries where the prim path is at or under the specified path.
    ///
    /// # Arguments
    ///
    /// * `root_path` - Root path to invalidate
    pub fn invalidate_subtree(&self, root_path: &Path) {
        let mut cache = self.cache.write();
        cache.retain(|key, _| !key.prim_path.has_prefix(root_path));
    }

    /// Invalidates all cached entries for a specific attribute across all prims.
    ///
    /// Removes all cache entries where the attribute name matches.
    ///
    /// # Arguments
    ///
    /// * `attr_name` - Name of the attribute to invalidate
    pub fn invalidate_attribute(&self, attr_name: &Token) {
        let mut cache = self.cache.write();
        cache.retain(|key, _| &key.attr_name != attr_name);
    }

    /// Invalidates all cached entries for a specific attribute on a specific prim.
    ///
    /// Removes all cache entries where both the prim path and attribute name match.
    ///
    /// # Arguments
    ///
    /// * `prim_path` - Path to the prim
    /// * `attr_name` - Name of the attribute
    pub fn invalidate_prim_attribute(&self, prim_path: &Path, attr_name: &Token) {
        let mut cache = self.cache.write();
        cache.retain(|key, _| &key.prim_path != prim_path || &key.attr_name != attr_name);
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

    /// Gets all cached attribute names for a specific prim at a specific time.
    ///
    /// Returns a vector of attribute names that have cached values for the
    /// specified prim and time.
    ///
    /// # Arguments
    ///
    /// * `prim_path` - Path to the prim
    /// * `time` - Time code for the query
    pub fn get_cached_attrs(&self, prim_path: &Path, time: TimeCode) -> Vec<Token> {
        let cache = self.cache.read();
        cache
            .keys()
            .filter(|key| &key.prim_path == prim_path && key.time == time)
            .map(|key| key.attr_name.clone())
            .collect()
    }
}

impl Default for ResolvedAttributeCache {
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
        let cache = ResolvedAttributeCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.size(), 0);
    }

    #[test]
    fn test_set_and_get() {
        let cache = ResolvedAttributeCache::new();
        let prim = Path::from_string("/World/Cube").unwrap();
        let attr = Token::new("size");
        let time = TimeCode::default();
        let value = Value::from(1.0f64);

        // Should return None before insertion
        assert_eq!(cache.get(&prim, &attr, time), None);

        // Set and get
        cache.set(&prim, &attr, time, value.clone());
        assert_eq!(cache.get(&prim, &attr, time), Some(value));
        assert_eq!(cache.size(), 1);
    }

    #[test]
    fn test_different_attributes() {
        let cache = ResolvedAttributeCache::new();
        let prim = Path::from_string("/World/Cube").unwrap();
        let time = TimeCode::default();

        let size_attr = Token::new("size");
        let color_attr = Token::new("displayColor");

        let size_val = Value::from(2.0f64);
        let color_val = Value::from(1.0f32);

        cache.set(&prim, &size_attr, time, size_val.clone());
        cache.set(&prim, &color_attr, time, color_val.clone());

        assert_eq!(cache.get(&prim, &size_attr, time), Some(size_val));
        assert_eq!(cache.get(&prim, &color_attr, time), Some(color_val));
        assert_eq!(cache.size(), 2);
    }

    #[test]
    fn test_different_times() {
        let cache = ResolvedAttributeCache::new();
        let prim = Path::from_string("/World/Cube").unwrap();
        let attr = Token::new("size");

        let time0 = TimeCode::new(0.0);
        let time1 = TimeCode::new(1.0);

        let val0 = Value::from(1.0f64);
        let val1 = Value::from(2.0f64);

        cache.set(&prim, &attr, time0, val0.clone());
        cache.set(&prim, &attr, time1, val1.clone());

        assert_eq!(cache.get(&prim, &attr, time0), Some(val0));
        assert_eq!(cache.get(&prim, &attr, time1), Some(val1));
        assert_eq!(cache.size(), 2);
    }

    #[test]
    fn test_remove() {
        let cache = ResolvedAttributeCache::new();
        let prim = Path::from_string("/World/Cube").unwrap();
        let attr = Token::new("size");
        let time = TimeCode::default();
        let value = Value::from(1.0f64);

        cache.set(&prim, &attr, time, value);
        assert_eq!(cache.size(), 1);

        // Remove existing entry
        assert!(cache.remove(&prim, &attr, time));
        assert_eq!(cache.get(&prim, &attr, time), None);
        assert_eq!(cache.size(), 0);

        // Remove non-existing entry
        assert!(!cache.remove(&prim, &attr, time));
    }

    #[test]
    fn test_clear() {
        let cache = ResolvedAttributeCache::new();
        let prim1 = Path::from_string("/World/Cube1").unwrap();
        let prim2 = Path::from_string("/World/Cube2").unwrap();
        let attr = Token::new("size");
        let time = TimeCode::default();
        let value = Value::from(1.0f64);

        cache.set(&prim1, &attr, time, value.clone());
        cache.set(&prim2, &attr, time, value.clone());
        assert_eq!(cache.size(), 2);

        cache.clear();
        assert!(cache.is_empty());
        assert_eq!(cache.get(&prim1, &attr, time), None);
        assert_eq!(cache.get(&prim2, &attr, time), None);
    }

    #[test]
    fn test_invalidate_prim() {
        let cache = ResolvedAttributeCache::new();
        let prim1 = Path::from_string("/World/Cube1").unwrap();
        let prim2 = Path::from_string("/World/Cube2").unwrap();
        let attr = Token::new("size");
        let time = TimeCode::default();
        let value = Value::from(1.0f64);

        cache.set(&prim1, &attr, time, value.clone());
        cache.set(&prim2, &attr, time, value.clone());
        assert_eq!(cache.size(), 2);

        cache.invalidate_prim(&prim1);
        assert_eq!(cache.size(), 1);
        assert_eq!(cache.get(&prim1, &attr, time), None);
        assert_eq!(cache.get(&prim2, &attr, time), Some(value));
    }

    #[test]
    fn test_invalidate_subtree() {
        let cache = ResolvedAttributeCache::new();
        let root = Path::from_string("/World").unwrap();
        let prim1 = Path::from_string("/World/Cube1").unwrap();
        let prim2 = Path::from_string("/World/Cube2").unwrap();
        let prim3 = Path::from_string("/Other/Cube").unwrap();
        let attr = Token::new("size");
        let time = TimeCode::default();
        let value = Value::from(1.0f64);

        cache.set(&prim1, &attr, time, value.clone());
        cache.set(&prim2, &attr, time, value.clone());
        cache.set(&prim3, &attr, time, value.clone());
        assert_eq!(cache.size(), 3);

        cache.invalidate_subtree(&root);
        assert_eq!(cache.size(), 1);
        assert_eq!(cache.get(&prim1, &attr, time), None);
        assert_eq!(cache.get(&prim2, &attr, time), None);
        assert_eq!(cache.get(&prim3, &attr, time), Some(value));
    }

    #[test]
    fn test_invalidate_attribute() {
        let cache = ResolvedAttributeCache::new();
        let prim = Path::from_string("/World/Cube").unwrap();
        let size_attr = Token::new("size");
        let color_attr = Token::new("displayColor");
        let time = TimeCode::default();

        cache.set(&prim, &size_attr, time, Value::from(1.0f64));
        cache.set(&prim, &color_attr, time, Value::from(1.0f32));
        assert_eq!(cache.size(), 2);

        cache.invalidate_attribute(&size_attr);
        assert_eq!(cache.size(), 1);
        assert_eq!(cache.get(&prim, &size_attr, time), None);
        assert!(cache.get(&prim, &color_attr, time).is_some());
    }

    #[test]
    fn test_invalidate_prim_attribute() {
        let cache = ResolvedAttributeCache::new();
        let prim1 = Path::from_string("/World/Cube1").unwrap();
        let prim2 = Path::from_string("/World/Cube2").unwrap();
        let attr = Token::new("size");
        let time = TimeCode::default();
        let value = Value::from(1.0f64);

        cache.set(&prim1, &attr, time, value.clone());
        cache.set(&prim2, &attr, time, value.clone());
        assert_eq!(cache.size(), 2);

        cache.invalidate_prim_attribute(&prim1, &attr);
        assert_eq!(cache.size(), 1);
        assert_eq!(cache.get(&prim1, &attr, time), None);
        assert_eq!(cache.get(&prim2, &attr, time), Some(value));
    }

    #[test]
    fn test_invalidate_time_range() {
        let cache = ResolvedAttributeCache::new();
        let prim = Path::from_string("/World/Cube").unwrap();
        let attr = Token::new("size");

        cache.set(&prim, &attr, TimeCode::new(0.0), Value::from(1.0f64));
        cache.set(&prim, &attr, TimeCode::new(1.0), Value::from(2.0f64));
        cache.set(&prim, &attr, TimeCode::new(2.0), Value::from(3.0f64));
        cache.set(&prim, &attr, TimeCode::new(3.0), Value::from(4.0f64));
        assert_eq!(cache.size(), 4);

        // Invalidate times 1.0 to 2.0 (inclusive)
        cache.invalidate_time_range(TimeCode::new(1.0), TimeCode::new(2.0));
        assert_eq!(cache.size(), 2);

        // Times 0.0 and 3.0 should remain
        assert!(cache.get(&prim, &attr, TimeCode::new(0.0)).is_some());
        assert!(cache.get(&prim, &attr, TimeCode::new(1.0)).is_none());
        assert!(cache.get(&prim, &attr, TimeCode::new(2.0)).is_none());
        assert!(cache.get(&prim, &attr, TimeCode::new(3.0)).is_some());
    }

    #[test]
    fn test_get_cached_attrs() {
        let cache = ResolvedAttributeCache::new();
        let prim = Path::from_string("/World/Cube").unwrap();
        let time = TimeCode::default();

        let size_attr = Token::new("size");
        let color_attr = Token::new("displayColor");
        let vis_attr = Token::new("visibility");

        cache.set(&prim, &size_attr, time, Value::from(1.0f64));
        cache.set(&prim, &color_attr, time, Value::from(1.0f32));
        cache.set(&prim, &vis_attr, time, Value::from(true));

        let cached_attrs = cache.get_cached_attrs(&prim, time);
        assert_eq!(cached_attrs.len(), 3);
        assert!(cached_attrs.contains(&size_attr));
        assert!(cached_attrs.contains(&color_attr));
        assert!(cached_attrs.contains(&vis_attr));
    }

    #[test]
    fn test_thread_safety() {
        use std::sync::Arc;
        use std::thread;

        let cache = Arc::new(ResolvedAttributeCache::new());
        let prim = Path::from_string("/World/Cube").unwrap();
        let attr = Token::new("size");

        // Spawn multiple threads to test concurrent access
        let mut handles = vec![];

        for i in 0..10 {
            let cache_clone = Arc::clone(&cache);
            let prim_clone = prim.clone();
            let attr_clone = attr.clone();

            let handle = thread::spawn(move || {
                let time = TimeCode::new(i as f64);
                let value = Value::from(i as f64);

                cache_clone.set(&prim_clone, &attr_clone, time, value.clone());

                // Read back the value
                let result = cache_clone.get(&prim_clone, &attr_clone, time);
                assert_eq!(result, Some(value));
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
