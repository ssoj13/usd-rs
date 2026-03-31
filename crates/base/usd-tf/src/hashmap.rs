//! Hash map type alias for USD compatibility.
//!
//! In USD C++, `TfHashMap` wraps `std::unordered_map` or GNU `hash_map`.
//! In Rust, we simply use `std::collections::HashMap`.
//!
//! # Examples
//!
//! ```
//! use usd_tf::hashmap::TfHashMap;
//!
//! let mut map: TfHashMap<String, i32> = TfHashMap::new();
//! map.insert("one".to_string(), 1);
//! map.insert("two".to_string(), 2);
//!
//! assert_eq!(map.get("one"), Some(&1));
//! ```

use std::collections::HashMap;
use std::hash::{BuildHasher, Hash};

/// Hash map type matching USD's TfHashMap.
///
/// This is a type alias for `std::collections::HashMap` using the default hasher.
/// For custom hashers, use `TfHashMapWith`.
///
/// # Examples
///
/// ```
/// use usd_tf::hashmap::TfHashMap;
///
/// let mut map = TfHashMap::new();
/// map.insert("key", 42);
/// assert_eq!(map.get("key"), Some(&42));
/// ```
pub type TfHashMap<K, V> = HashMap<K, V>;

/// Hash map with custom hasher.
///
/// # Type Parameters
///
/// * `K` - Key type (must implement `Hash` and `Eq`)
/// * `V` - Value type
/// * `S` - Hasher builder type
pub type TfHashMapWith<K, V, S> = HashMap<K, V, S>;

/// Creates a new empty hash map.
///
/// # Examples
///
/// ```
/// use usd_tf::hashmap::new_hashmap;
///
/// let map: std::collections::HashMap<i32, &str> = new_hashmap();
/// assert!(map.is_empty());
/// ```
#[inline]
pub fn new_hashmap<K, V>() -> TfHashMap<K, V>
where
    K: Hash + Eq,
{
    HashMap::new()
}

/// Creates a hash map with the specified capacity.
///
/// # Examples
///
/// ```
/// use usd_tf::hashmap::with_capacity;
///
/// let map: std::collections::HashMap<i32, i32> = with_capacity(100);
/// assert!(map.capacity() >= 100);
/// ```
#[inline]
pub fn with_capacity<K, V>(capacity: usize) -> TfHashMap<K, V>
where
    K: Hash + Eq,
{
    HashMap::with_capacity(capacity)
}

/// Creates a hash map with a custom hasher.
///
/// # Examples
///
/// ```
/// use usd_tf::hashmap::with_hasher;
/// use std::collections::hash_map::RandomState;
///
/// let map: std::collections::HashMap<i32, i32, _> = with_hasher(RandomState::new());
/// assert!(map.is_empty());
/// ```
#[inline]
pub fn with_hasher<K, V, S>(hash_builder: S) -> TfHashMapWith<K, V, S>
where
    K: Hash + Eq,
    S: BuildHasher,
{
    HashMap::with_hasher(hash_builder)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tfhashmap_basic() {
        let mut map: TfHashMap<String, i32> = TfHashMap::new();
        map.insert("one".to_string(), 1);
        map.insert("two".to_string(), 2);
        map.insert("three".to_string(), 3);

        assert_eq!(map.len(), 3);
        assert_eq!(map.get("one"), Some(&1));
        assert_eq!(map.get("two"), Some(&2));
        assert_eq!(map.get("three"), Some(&3));
        assert_eq!(map.get("four"), None);
    }

    #[test]
    fn test_new_hashmap() {
        let map: TfHashMap<i32, &str> = new_hashmap();
        assert!(map.is_empty());
    }

    #[test]
    fn test_with_capacity() {
        let map: TfHashMap<i32, i32> = with_capacity(100);
        assert!(map.capacity() >= 100);
    }

    #[test]
    fn test_hashmap_operations() {
        let mut map: TfHashMap<i32, String> = new_hashmap();

        // Insert
        map.insert(1, "one".to_string());
        map.insert(2, "two".to_string());

        // Contains
        assert!(map.contains_key(&1));
        assert!(!map.contains_key(&3));

        // Remove
        assert_eq!(map.remove(&1), Some("one".to_string()));
        assert!(!map.contains_key(&1));

        // Entry API
        map.entry(3).or_insert("three".to_string());
        assert_eq!(map.get(&3), Some(&"three".to_string()));
    }

    #[test]
    fn test_hashmap_iteration() {
        let mut map: TfHashMap<i32, i32> = new_hashmap();
        map.insert(1, 10);
        map.insert(2, 20);
        map.insert(3, 30);

        let sum: i32 = map.values().sum();
        assert_eq!(sum, 60);
    }
}
