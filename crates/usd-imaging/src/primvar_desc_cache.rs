//! Primvar descriptor cache for UsdImaging.
//!
//! Port of pxr/usdImaging/usdImaging/primvarDescCache.h
//!
//! Provides caching for primvar descriptors to avoid redundant computation
//! when querying primvar metadata for prims at specific time codes.

use parking_lot::RwLock;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use usd_core::time_code::TimeCode;
use usd_hd::enums::HdInterpolation;
use usd_sdf::Path;
use usd_tf::Token;

// ============================================================================
// PrimvarDescriptor
// ============================================================================

/// Descriptor for a primvar (primitive variable).
///
/// Describes metadata about a primvar including its name, interpolation mode,
/// role, and whether it's indexed.
///
/// Matches relevant fields from C++ `HdPrimvarDescriptor`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PrimvarDescriptor {
    /// Name of the primvar
    pub name: Token,
    /// Interpolation mode (constant, vertex, varying, etc.)
    pub interpolation: HdInterpolation,
    /// Role hint for the primvar (e.g., "point", "color", "vector")
    pub role: Token,
    /// Whether the primvar uses indexed data
    pub indexed: bool,
}

impl PrimvarDescriptor {
    /// Creates a new primvar descriptor.
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the primvar
    /// * `interpolation` - Interpolation mode
    /// * `role` - Role hint (empty token for no specific role)
    /// * `indexed` - Whether the primvar uses indexed data
    pub fn new(name: Token, interpolation: HdInterpolation, role: Token, indexed: bool) -> Self {
        Self {
            name,
            interpolation,
            role,
            indexed,
        }
    }

    /// Creates a primvar descriptor with default values (non-indexed, no role).
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the primvar
    /// * `interpolation` - Interpolation mode
    pub fn with_name_and_interp(name: Token, interpolation: HdInterpolation) -> Self {
        Self {
            name,
            interpolation,
            role: Token::empty(),
            indexed: false,
        }
    }
}

// ============================================================================
// CacheKey
// ============================================================================

/// Key for primvar descriptor cache lookups.
///
/// Combines prim path and time code for unique identification of cached
/// primvar descriptors.
#[derive(Clone, Debug, PartialEq, Eq)]
struct CacheKey {
    /// Path to the prim
    prim_path: Path,
    /// Time code for the query
    time: TimeCode,
}

impl Hash for CacheKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.prim_path.hash(state);
        // Hash time as bits for consistent hashing
        self.time.value().to_bits().hash(state);
    }
}

// ============================================================================
// PrimvarDescCache
// ============================================================================

/// Cache for primvar descriptors.
///
/// Matches C++ `UsdImaging_PrimvarDescCache`.
///
/// This cache stores primvar descriptors for prims to avoid redundant
/// computation when querying primvar metadata. Primvar descriptors can be
/// time-varying, so the cache is keyed by prim path and time code.
///
/// The cache is thread-safe using RwLock for concurrent read access.
///
/// # Examples
///
/// ```
/// use usd_sdf::Path;
/// use usd_core::time_code::TimeCode;
/// use usd_imaging::{PrimvarDescCache, PrimvarDescriptor};
/// use usd_tf::Token;
/// use usd_hd::enums::HdInterpolation;
///
/// let cache = PrimvarDescCache::new();
/// let prim_path = Path::from_string("/World/Mesh").unwrap();
/// let time = TimeCode::default();
///
/// // Create primvar descriptors
/// let descriptors = vec![
///     PrimvarDescriptor::with_name_and_interp(
///         Token::new("points"),
///         HdInterpolation::Vertex
///     ),
///     PrimvarDescriptor::with_name_and_interp(
///         Token::new("normals"),
///         HdInterpolation::Vertex
///     ),
/// ];
///
/// // Store descriptors
/// cache.set(&prim_path, time, descriptors.clone());
///
/// // Retrieve descriptors
/// assert_eq!(cache.get(&prim_path, time), Some(descriptors));
///
/// // Clear the cache
/// cache.clear();
/// assert_eq!(cache.get(&prim_path, time), None);
/// ```
pub struct PrimvarDescCache {
    /// Internal cache storage
    cache: RwLock<HashMap<CacheKey, Vec<PrimvarDescriptor>>>,
}

impl PrimvarDescCache {
    /// Creates a new empty primvar descriptor cache.
    ///
    /// Matches C++ `UsdImaging_PrimvarDescCache()`.
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// Retrieves primvar descriptors for a prim at a specific time.
    ///
    /// Returns `Some(descriptors)` if cached, or `None` if not cached.
    ///
    /// # Arguments
    ///
    /// * `prim_path` - Path to the prim
    /// * `time` - Time code for the query
    pub fn get(&self, prim_path: &Path, time: TimeCode) -> Option<Vec<PrimvarDescriptor>> {
        let key = CacheKey {
            prim_path: prim_path.clone(),
            time,
        };

        let cache = self.cache.read();
        cache.get(&key).cloned()
    }

    /// Stores primvar descriptors for a prim at a specific time.
    ///
    /// # Arguments
    ///
    /// * `prim_path` - Path to the prim
    /// * `time` - Time code for the query
    /// * `descriptors` - Vector of primvar descriptors to cache
    pub fn set(&self, prim_path: &Path, time: TimeCode, descriptors: Vec<PrimvarDescriptor>) {
        let key = CacheKey {
            prim_path: prim_path.clone(),
            time,
        };

        let mut cache = self.cache.write();
        cache.insert(key, descriptors);
    }

    /// Removes cached descriptors for a prim at a specific time.
    ///
    /// Returns `true` if an entry was removed, `false` if no entry existed.
    ///
    /// # Arguments
    ///
    /// * `prim_path` - Path to the prim
    /// * `time` - Time code for the query
    pub fn remove(&self, prim_path: &Path, time: TimeCode) -> bool {
        let key = CacheKey {
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
}

impl Default for PrimvarDescCache {
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

    fn create_test_descriptors() -> Vec<PrimvarDescriptor> {
        vec![
            PrimvarDescriptor::with_name_and_interp(Token::new("points"), HdInterpolation::Vertex),
            PrimvarDescriptor::with_name_and_interp(Token::new("normals"), HdInterpolation::Vertex),
            PrimvarDescriptor::new(
                Token::new("displayColor"),
                HdInterpolation::Constant,
                Token::new("color"),
                false,
            ),
        ]
    }

    #[test]
    fn test_new_cache_is_empty() {
        let cache = PrimvarDescCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.size(), 0);
    }

    #[test]
    fn test_set_and_get() {
        let cache = PrimvarDescCache::new();
        let prim = Path::from_string("/World/Mesh").unwrap();
        let time = TimeCode::default();
        let descriptors = create_test_descriptors();

        // Should return None before insertion
        assert_eq!(cache.get(&prim, time), None);

        // Set and get
        cache.set(&prim, time, descriptors.clone());
        assert_eq!(cache.get(&prim, time), Some(descriptors));
        assert_eq!(cache.size(), 1);
    }

    #[test]
    fn test_different_times() {
        let cache = PrimvarDescCache::new();
        let prim = Path::from_string("/World/Mesh").unwrap();

        let time0 = TimeCode::new(0.0);
        let time1 = TimeCode::new(1.0);

        let desc0 = vec![PrimvarDescriptor::with_name_and_interp(
            Token::new("points"),
            HdInterpolation::Vertex,
        )];

        let desc1 = vec![
            PrimvarDescriptor::with_name_and_interp(Token::new("points"), HdInterpolation::Vertex),
            PrimvarDescriptor::with_name_and_interp(
                Token::new("velocities"),
                HdInterpolation::Vertex,
            ),
        ];

        cache.set(&prim, time0, desc0.clone());
        cache.set(&prim, time1, desc1.clone());

        assert_eq!(cache.get(&prim, time0), Some(desc0));
        assert_eq!(cache.get(&prim, time1), Some(desc1));
        assert_eq!(cache.size(), 2);
    }

    #[test]
    fn test_remove() {
        let cache = PrimvarDescCache::new();
        let prim = Path::from_string("/World/Mesh").unwrap();
        let time = TimeCode::default();
        let descriptors = create_test_descriptors();

        cache.set(&prim, time, descriptors);
        assert_eq!(cache.size(), 1);

        // Remove existing entry
        assert!(cache.remove(&prim, time));
        assert_eq!(cache.get(&prim, time), None);
        assert_eq!(cache.size(), 0);

        // Remove non-existing entry
        assert!(!cache.remove(&prim, time));
    }

    #[test]
    fn test_clear() {
        let cache = PrimvarDescCache::new();
        let prim1 = Path::from_string("/World/Mesh1").unwrap();
        let prim2 = Path::from_string("/World/Mesh2").unwrap();
        let time = TimeCode::default();
        let descriptors = create_test_descriptors();

        cache.set(&prim1, time, descriptors.clone());
        cache.set(&prim2, time, descriptors.clone());
        assert_eq!(cache.size(), 2);

        cache.clear();
        assert!(cache.is_empty());
        assert_eq!(cache.get(&prim1, time), None);
        assert_eq!(cache.get(&prim2, time), None);
    }

    #[test]
    fn test_invalidate_prim() {
        let cache = PrimvarDescCache::new();
        let prim1 = Path::from_string("/World/Mesh1").unwrap();
        let prim2 = Path::from_string("/World/Mesh2").unwrap();
        let time = TimeCode::default();
        let descriptors = create_test_descriptors();

        cache.set(&prim1, time, descriptors.clone());
        cache.set(&prim2, time, descriptors.clone());
        assert_eq!(cache.size(), 2);

        cache.invalidate_prim(&prim1);
        assert_eq!(cache.size(), 1);
        assert_eq!(cache.get(&prim1, time), None);
        assert_eq!(cache.get(&prim2, time), Some(descriptors));
    }

    #[test]
    fn test_invalidate_subtree() {
        let cache = PrimvarDescCache::new();
        let root = Path::from_string("/World").unwrap();
        let prim1 = Path::from_string("/World/Mesh1").unwrap();
        let prim2 = Path::from_string("/World/Mesh2").unwrap();
        let prim3 = Path::from_string("/Other/Mesh").unwrap();
        let time = TimeCode::default();
        let descriptors = create_test_descriptors();

        cache.set(&prim1, time, descriptors.clone());
        cache.set(&prim2, time, descriptors.clone());
        cache.set(&prim3, time, descriptors.clone());
        assert_eq!(cache.size(), 3);

        cache.invalidate_subtree(&root);
        assert_eq!(cache.size(), 1);
        assert_eq!(cache.get(&prim1, time), None);
        assert_eq!(cache.get(&prim2, time), None);
        assert_eq!(cache.get(&prim3, time), Some(descriptors));
    }

    #[test]
    fn test_invalidate_time_range() {
        let cache = PrimvarDescCache::new();
        let prim = Path::from_string("/World/Mesh").unwrap();
        let descriptors = create_test_descriptors();

        cache.set(&prim, TimeCode::new(0.0), descriptors.clone());
        cache.set(&prim, TimeCode::new(1.0), descriptors.clone());
        cache.set(&prim, TimeCode::new(2.0), descriptors.clone());
        cache.set(&prim, TimeCode::new(3.0), descriptors.clone());
        assert_eq!(cache.size(), 4);

        // Invalidate times 1.0 to 2.0 (inclusive)
        cache.invalidate_time_range(TimeCode::new(1.0), TimeCode::new(2.0));
        assert_eq!(cache.size(), 2);

        // Times 0.0 and 3.0 should remain
        assert!(cache.get(&prim, TimeCode::new(0.0)).is_some());
        assert!(cache.get(&prim, TimeCode::new(1.0)).is_none());
        assert!(cache.get(&prim, TimeCode::new(2.0)).is_none());
        assert!(cache.get(&prim, TimeCode::new(3.0)).is_some());
    }

    #[test]
    fn test_primvar_descriptor_equality() {
        let desc1 =
            PrimvarDescriptor::with_name_and_interp(Token::new("points"), HdInterpolation::Vertex);

        let desc2 =
            PrimvarDescriptor::with_name_and_interp(Token::new("points"), HdInterpolation::Vertex);

        let desc3 =
            PrimvarDescriptor::with_name_and_interp(Token::new("normals"), HdInterpolation::Vertex);

        assert_eq!(desc1, desc2);
        assert_ne!(desc1, desc3);
    }

    #[test]
    fn test_primvar_descriptor_with_role() {
        let desc = PrimvarDescriptor::new(
            Token::new("displayColor"),
            HdInterpolation::Constant,
            Token::new("color"),
            true,
        );

        assert_eq!(desc.name, Token::new("displayColor"));
        assert_eq!(desc.interpolation, HdInterpolation::Constant);
        assert_eq!(desc.role, Token::new("color"));
        assert!(desc.indexed);
    }

    #[test]
    fn test_thread_safety() {
        use std::sync::Arc;
        use std::thread;

        let cache = Arc::new(PrimvarDescCache::new());
        let prim = Path::from_string("/World/Mesh").unwrap();

        // Spawn multiple threads to test concurrent access
        let mut handles = vec![];

        for i in 0..10 {
            let cache_clone = Arc::clone(&cache);
            let prim_clone = prim.clone();

            let handle = thread::spawn(move || {
                let time = TimeCode::new(i as f64);
                let descriptors = vec![PrimvarDescriptor::with_name_and_interp(
                    Token::new(&format!("primvar_{}", i)),
                    HdInterpolation::Vertex,
                )];

                cache_clone.set(&prim_clone, time, descriptors.clone());

                // Read back the value
                let result = cache_clone.get(&prim_clone, time);
                assert_eq!(result, Some(descriptors));
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
