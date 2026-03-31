//! Light linking collection cache for UsdImaging delegate.
//!
//! Port of pxr/usdImaging/usdImaging/collectionCache.h/cpp
//!
//! Maps light prim collection paths (lightLink / shadowLink) to their
//! computed MembershipQuery objects, and provides fast lookup of which
//! collections contain a given prim path.
//!
//! # Design
//!
//! C++ `UsdImaging_CollectionCache` groups collections into equivalence classes
//! by their MembershipQuery hash. Here we store `collection_path → query`
//! and iterate over registered queries for path containment checks.
//! Trivial queries (include-all) are stored with `is_trivial = true` and skipped
//! in `compute_collections_containing_path` to match C++ empty-id behavior.

use std::collections::HashMap;
use std::sync::Mutex;
use usd_core::collection_membership_query::CollectionMembershipQuery;
use usd_sdf::Path;
use usd_tf::Token;

// ============================================================================
// LightLinkingCache
// ============================================================================

/// Registered collection entry.
struct CollectionEntry {
    /// Computed membership query for this collection.
    query: CollectionMembershipQuery,
    /// True when query includes everything (trivial / all-pass).
    /// Trivial collections are omitted from `compute_collections_containing_path`
    /// results — matches C++ behavior where trivial collections get empty id.
    is_trivial: bool,
    /// Human-readable id token (collection path string).
    id: Token,
}

/// Cache mapping light collection paths to their membership queries.
///
/// Matches C++ `UsdImaging_CollectionCache` used in `UsdImagingDelegate`.
///
/// Thread-safe: all mutations require `&self` via interior `Mutex`.
pub struct LightLinkingCache {
    /// collection_path → registered entry.
    entries: Mutex<HashMap<Path, CollectionEntry>>,
}

impl LightLinkingCache {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
        }
    }

    /// Register (or re-register) a collection by its pre-computed query.
    ///
    /// If the collection was previously registered its entry is replaced.
    /// Trivial queries (single `/ → expandPrims` rule) are stored but skipped
    /// in path containment checks. Matches C++ `UpdateCollection()`.
    ///
    /// # Arguments
    /// * `collection_path` — `SdfPath` of the collection property
    ///   (e.g. `/Lights/Sun.collection:lightLink`)
    /// * `query` — result of `CollectionAPI::compute_membership_query()`
    pub fn update_collection(&self, collection_path: Path, query: CollectionMembershipQuery) {
        let is_trivial = Self::query_is_trivial(&query);
        let id = Token::new(collection_path.as_str());
        let entry = CollectionEntry {
            query,
            is_trivial,
            id,
        };
        self.entries
            .lock()
            .expect("lock poisoned")
            .insert(collection_path, entry);
    }

    /// Remove a previously registered collection. Matches C++ `RemoveCollection()`.
    pub fn remove_collection(&self, collection_path: &Path) {
        self.entries
            .lock()
            .expect("lock poisoned")
            .remove(collection_path);
    }

    /// Clear all registered collections.
    pub fn clear(&self) {
        self.entries.lock().expect("lock poisoned").clear();
    }

    /// Return tokens of all non-trivial collections that include `path`.
    ///
    /// Matches C++ `ComputeCollectionsContainingPath()`. Trivial collections
    /// (those that include every prim) return empty id in C++ and are therefore
    /// excluded from results here.
    pub fn compute_collections_containing_path(&self, path: &Path) -> Vec<Token> {
        let guard = self.entries.lock().expect("lock poisoned");
        let mut result: Vec<Token> = guard
            .values()
            .filter(|e| !e.is_trivial && e.query.is_path_included(path, None))
            .map(|e| e.id.clone())
            .collect();
        // Stable sort for deterministic output (C++ iterates an unordered_map).
        result.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        result
    }

    /// Return the id token assigned to the collection at `collection_path`.
    ///
    /// Returns an empty `Token` for trivial (include-all) collections.
    /// Matches C++ `GetIdForCollection()`.
    pub fn get_id_for_collection(&self, collection_path: &Path) -> Token {
        let guard = self.entries.lock().expect("lock poisoned");
        match guard.get(collection_path) {
            Some(e) if !e.is_trivial => e.id.clone(),
            _ => Token::new(""),
        }
    }

    /// True when there are no registered collections.
    pub fn is_empty(&self) -> bool {
        self.entries.lock().expect("lock poisoned").is_empty()
    }

    // ---------------------------------------------------------------------- //
    // Helpers
    // ---------------------------------------------------------------------- //

    /// A query is trivial if it contains only one rule: `/ → expandPrims`.
    /// Such a collection includes every prim in the scene.
    /// Matches C++ `_IsQueryTrivial()`.
    fn query_is_trivial(query: &CollectionMembershipQuery) -> bool {
        let rule_map = query.get_as_path_expansion_rule_map();
        if rule_map.len() != 1 {
            return false;
        }
        let (path, rule) = rule_map.iter().next().unwrap();
        path.is_absolute_root_path() && rule == "expandPrims"
    }
}

impl Default for LightLinkingCache {
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
    use std::collections::HashSet;
    use usd_core::collection_membership_query::{CollectionMembershipQuery, PathExpansionRuleMap};
    use usd_sdf::Path;

    fn make_query_include(paths: &[&str]) -> CollectionMembershipQuery {
        let mut rule_map = PathExpansionRuleMap::new();
        for p in paths {
            rule_map.insert(
                Path::from_string(p).unwrap(),
                usd_tf::Token::new("expandPrims"),
            );
        }
        CollectionMembershipQuery::new_with_map(rule_map, HashSet::new())
    }

    fn make_trivial_query() -> CollectionMembershipQuery {
        // Single rule: / -> expandPrims  (include all)
        make_query_include(&["/"])
    }

    fn make_exclude_query(include: &[&str], exclude: &[&str]) -> CollectionMembershipQuery {
        let mut rule_map = PathExpansionRuleMap::new();
        for p in include {
            rule_map.insert(
                Path::from_string(p).unwrap(),
                usd_tf::Token::new("expandPrims"),
            );
        }
        for p in exclude {
            rule_map.insert(Path::from_string(p).unwrap(), usd_tf::Token::new("exclude"));
        }
        CollectionMembershipQuery::new_with_map(rule_map, HashSet::new())
    }

    #[test]
    fn test_empty_cache() {
        let cache = LightLinkingCache::new();
        assert!(cache.is_empty());
        let prim = Path::from_string("/World/Sphere").unwrap();
        assert!(cache.compute_collections_containing_path(&prim).is_empty());
    }

    #[test]
    fn test_trivial_excluded_from_results() {
        let cache = LightLinkingCache::new();
        let col_path = Path::from_string("/Lights/Sun.collection:lightLink").unwrap();
        cache.update_collection(col_path.clone(), make_trivial_query());
        assert!(!cache.is_empty());

        // Trivial collections must NOT appear in results (matches C++ empty-id behavior).
        let prim = Path::from_string("/World/Mesh").unwrap();
        let result = cache.compute_collections_containing_path(&prim);
        assert!(
            result.is_empty(),
            "trivial collection should not appear in results"
        );

        // GetIdForCollection also returns empty token for trivial collections.
        assert_eq!(cache.get_id_for_collection(&col_path).as_str(), "");
    }

    #[test]
    fn test_non_trivial_include() {
        let cache = LightLinkingCache::new();
        let col_path = Path::from_string("/Lights/Key.collection:lightLink").unwrap();
        // Include /World subtree
        cache.update_collection(col_path.clone(), make_query_include(&["/World"]));

        let inside = Path::from_string("/World/Sphere").unwrap();
        let outside = Path::from_string("/Props/Table").unwrap();

        let result_inside = cache.compute_collections_containing_path(&inside);
        assert_eq!(result_inside.len(), 1);
        assert_eq!(
            result_inside[0].as_str(),
            "/Lights/Key.collection:lightLink"
        );

        let result_outside = cache.compute_collections_containing_path(&outside);
        assert!(result_outside.is_empty());
    }

    #[test]
    fn test_multiple_collections() {
        let cache = LightLinkingCache::new();
        let col_a = Path::from_string("/Lights/A.collection:lightLink").unwrap();
        let col_b = Path::from_string("/Lights/B.collection:shadowLink").unwrap();

        // A includes /World, B includes /World/Sphere specifically
        cache.update_collection(col_a.clone(), make_query_include(&["/World"]));
        cache.update_collection(col_b.clone(), make_query_include(&["/World/Sphere"]));

        let sphere = Path::from_string("/World/Sphere").unwrap();
        let cube = Path::from_string("/World/Cube").unwrap();

        let sphere_result = cache.compute_collections_containing_path(&sphere);
        // Both A and B include /World/Sphere, sorted alphabetically
        assert_eq!(sphere_result.len(), 2);

        let cube_result = cache.compute_collections_containing_path(&cube);
        // Only A includes /World/Cube
        assert_eq!(cube_result.len(), 1);
        assert_eq!(cube_result[0].as_str(), "/Lights/A.collection:lightLink");
    }

    #[test]
    fn test_remove_collection() {
        let cache = LightLinkingCache::new();
        let col_path = Path::from_string("/Lights/Key.collection:lightLink").unwrap();
        cache.update_collection(col_path.clone(), make_query_include(&["/World"]));

        let sphere = Path::from_string("/World/Sphere").unwrap();
        assert_eq!(cache.compute_collections_containing_path(&sphere).len(), 1);

        cache.remove_collection(&col_path);
        assert!(
            cache
                .compute_collections_containing_path(&sphere)
                .is_empty()
        );
        assert!(cache.is_empty());
    }

    #[test]
    fn test_update_replaces_entry() {
        let cache = LightLinkingCache::new();
        let col_path = Path::from_string("/Lights/Key.collection:lightLink").unwrap();

        // First register: includes /World
        cache.update_collection(col_path.clone(), make_query_include(&["/World"]));

        // Re-register with exclude — /World/Cube excluded
        cache.update_collection(
            col_path.clone(),
            make_exclude_query(&["/World"], &["/World/Cube"]),
        );

        let sphere = Path::from_string("/World/Sphere").unwrap();
        let cube = Path::from_string("/World/Cube").unwrap();

        assert_eq!(cache.compute_collections_containing_path(&sphere).len(), 1);
        // Cube is excluded so not in result
        assert!(cache.compute_collections_containing_path(&cube).is_empty());
    }

    #[test]
    fn test_get_id_for_collection() {
        let cache = LightLinkingCache::new();
        let col_path = Path::from_string("/Lights/Sun.collection:lightLink").unwrap();
        cache.update_collection(col_path.clone(), make_query_include(&["/World"]));
        assert_eq!(
            cache.get_id_for_collection(&col_path).as_str(),
            "/Lights/Sun.collection:lightLink"
        );
    }

    #[test]
    fn test_clear() {
        let cache = LightLinkingCache::new();
        let col_a = Path::from_string("/Lights/A.collection:lightLink").unwrap();
        let col_b = Path::from_string("/Lights/B.collection:shadowLink").unwrap();
        cache.update_collection(col_a, make_query_include(&["/World"]));
        cache.update_collection(col_b, make_query_include(&["/Props"]));
        assert!(!cache.is_empty());
        cache.clear();
        assert!(cache.is_empty());
    }
}
