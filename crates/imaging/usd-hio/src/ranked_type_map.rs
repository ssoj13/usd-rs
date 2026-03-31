//! HioRankedTypeMap - Token-to-factory map with precedence support.
//!
//! Port of pxr/imaging/hio/rankedTypeMap.h
//!
//! Maps token keys (e.g., file extensions) to factory functions with
//! precedence-based conflict resolution. Higher precedence wins.

use std::any::TypeId;
use std::collections::HashMap;
use usd_tf::Token;

/// Entry in the ranked type map: factory + precedence.
struct RankedEntry {
    /// Type ID of the registered factory/handler type
    type_id: TypeId,
    /// Precedence value; higher wins on conflict
    precedence: i32,
}

/// Maps token keys to type IDs with precedence-based conflict resolution.
///
/// In C++ this maps TfToken -> TfType using plugin metadata.
/// In Rust we map Token -> TypeId since we don't have TfType.
///
/// # Usage
///
/// ```
/// use usd_hio::HioRankedTypeMap;
/// use usd_tf::Token;
///
/// let mut map = HioRankedTypeMap::new();
///
/// struct PngHandler;
/// struct BetterPngHandler;
///
/// map.add::<PngHandler>(&Token::new("png"), 1);
/// map.add::<BetterPngHandler>(&Token::new("png"), 10);
///
/// // BetterPngHandler wins due to higher precedence
/// let found = map.find(&Token::new("png"));
/// assert!(found.is_some());
/// assert_eq!(found.unwrap(), std::any::TypeId::of::<BetterPngHandler>());
/// ```
pub struct HioRankedTypeMap {
    entries: HashMap<Token, RankedEntry>,
}

impl HioRankedTypeMap {
    /// Create a new empty ranked type map.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Add a type for the given key with the given precedence.
    ///
    /// If the key already exists, the new entry replaces it only if
    /// the new precedence is strictly greater than the existing one.
    pub fn add<T: 'static>(&mut self, key: &Token, precedence: i32) {
        self.add_type_id(key, TypeId::of::<T>(), precedence);
    }

    /// Add a TypeId for the given key with the given precedence.
    pub fn add_type_id(&mut self, key: &Token, type_id: TypeId, precedence: i32) {
        match self.entries.get(key) {
            Some(existing) if existing.precedence >= precedence => {
                // Existing entry has equal or higher precedence, skip
            }
            _ => {
                self.entries.insert(
                    key.clone(),
                    RankedEntry {
                        type_id,
                        precedence,
                    },
                );
            }
        }
    }

    /// Find the highest-precedence TypeId for the given key.
    ///
    /// Returns None if the key was never added.
    pub fn find(&self, key: &Token) -> Option<TypeId> {
        self.entries.get(key).map(|e| e.type_id)
    }

    /// Check if the map contains the given key.
    pub fn contains(&self, key: &Token) -> bool {
        self.entries.contains_key(key)
    }

    /// Returns the number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the map is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get all registered keys.
    pub fn keys(&self) -> Vec<Token> {
        self.entries.keys().cloned().collect()
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl Default for HioRankedTypeMap {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct HandlerA;
    struct HandlerB;

    #[test]
    fn test_add_and_find() {
        let mut map = HioRankedTypeMap::new();
        map.add::<HandlerA>(&Token::new("png"), 1);

        let found = map.find(&Token::new("png"));
        assert!(found.is_some());
        assert_eq!(found.unwrap(), TypeId::of::<HandlerA>());
    }

    #[test]
    fn test_precedence_override() {
        let mut map = HioRankedTypeMap::new();
        map.add::<HandlerA>(&Token::new("png"), 1);
        map.add::<HandlerB>(&Token::new("png"), 10);

        // HandlerB wins (higher precedence)
        let found = map.find(&Token::new("png")).unwrap();
        assert_eq!(found, TypeId::of::<HandlerB>());
    }

    #[test]
    fn test_precedence_no_override() {
        let mut map = HioRankedTypeMap::new();
        map.add::<HandlerA>(&Token::new("png"), 10);
        map.add::<HandlerB>(&Token::new("png"), 1);

        // HandlerA stays (higher precedence)
        let found = map.find(&Token::new("png")).unwrap();
        assert_eq!(found, TypeId::of::<HandlerA>());
    }

    #[test]
    fn test_find_missing() {
        let map = HioRankedTypeMap::new();
        assert!(map.find(&Token::new("missing")).is_none());
    }

    #[test]
    fn test_multiple_keys() {
        let mut map = HioRankedTypeMap::new();
        map.add::<HandlerA>(&Token::new("png"), 1);
        map.add::<HandlerB>(&Token::new("jpg"), 1);

        assert_eq!(map.len(), 2);
        assert!(map.contains(&Token::new("png")));
        assert!(map.contains(&Token::new("jpg")));
        assert!(!map.contains(&Token::new("exr")));
    }

    #[test]
    fn test_clear() {
        let mut map = HioRankedTypeMap::new();
        map.add::<HandlerA>(&Token::new("png"), 1);
        assert!(!map.is_empty());

        map.clear();
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
    }
}
