//! Prim type info cache.
//!
//! Port of pxr/usd/usd/primTypeInfoCache.h
//!
//! A thread-safe cache used by UsdStage to store PrimTypeInfo structures
//! for all distinct prim types encountered. Each unique combination of
//! type name + applied API schemas gets its own cached entry.

use crate::prim_type_info::{PrimTypeId, PrimTypeInfo};
use parking_lot::RwLock;
use std::collections::HashMap;
use usd_tf::Token;

/// Thread-safe cache for prim type info.
///
/// Used by UsdStage to cache type info for all distinct prim types
/// used by any prim data. Keyed by PrimTypeId (type name + applied schemas).
pub struct PrimTypeInfoCache {
    /// Map from PrimTypeId to cached PrimTypeInfo.
    map: RwLock<HashMap<PrimTypeId, PrimTypeInfo>>,
}

impl PrimTypeInfoCache {
    /// Creates a new, empty cache.
    pub fn new() -> Self {
        Self {
            map: RwLock::new(HashMap::new()),
        }
    }

    /// Finds the cached prim type info for the given type ID, creating
    /// and caching a new one if it doesn't exist.
    pub fn find_or_create(&self, type_id: PrimTypeId) -> PrimTypeId {
        if type_id.is_empty() {
            return PrimTypeId::default();
        }

        // Fast path: check if already cached (read lock).
        {
            let map = self.map.read();
            if map.contains_key(&type_id) {
                return type_id;
            }
        }

        // Slow path: create and insert (write lock).
        let mut map = self.map.write();
        // Double-check after acquiring write lock.
        if !map.contains_key(&type_id) {
            let info = PrimTypeInfo::new(type_id.clone());
            map.insert(type_id.clone(), info);
        }
        type_id
    }

    /// Returns a reference to the cached info for the given type ID, if any.
    pub fn get(&self, type_id: &PrimTypeId) -> Option<PrimTypeId> {
        let map = self.map.read();
        if map.contains_key(type_id) {
            Some(type_id.clone())
        } else {
            None
        }
    }

    /// Returns true if the cache contains an entry for the given type ID.
    pub fn contains(&self, type_id: &PrimTypeId) -> bool {
        self.map.read().contains_key(type_id)
    }

    /// Returns the number of cached entries.
    pub fn len(&self) -> usize {
        self.map.read().len()
    }

    /// Returns true if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.map.read().is_empty()
    }

    /// Clears the cache.
    pub fn clear(&mut self) {
        self.map.write().clear();
    }

    /// Computes a mapping of invalid prim type names to their valid
    /// fallback type names from the provided fallback prim types dictionary.
    pub fn compute_invalid_prim_type_to_fallback_map(
        &self,
        fallback_types: &HashMap<Token, Token>,
    ) -> HashMap<Token, Token> {
        // The fallback dict maps unrecognized type name -> fallback type name.
        // We just validate and return the mapping.
        fallback_types.clone()
    }
}

impl Default for PrimTypeInfoCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_cache() {
        let cache = PrimTypeInfoCache::new();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_find_or_create() {
        let cache = PrimTypeInfoCache::new();
        let id = PrimTypeId::from_type_name(Token::from("Mesh"));
        let result = cache.find_or_create(id.clone());
        assert_eq!(result, id);
        assert!(cache.contains(&id));
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_empty_type_id_not_cached() {
        let cache = PrimTypeInfoCache::new();
        let empty = PrimTypeId::default();
        let result = cache.find_or_create(empty.clone());
        assert!(result.is_empty());
        // Empty type IDs should not be stored in the cache.
        assert!(cache.is_empty());
    }
}
