//! Dense hash map - space efficient for small collections.
//!
//! This container uses a vector for storage when small, switching to a
//! hash map when the size exceeds a threshold. This provides cache-efficient
//! iteration for small collections while maintaining O(1) lookup for larger ones.
//!
//! # Examples
//!
//! ```
//! use usd_tf::dense_hashmap::DenseHashMap;
//!
//! let mut map: DenseHashMap<String, i32> = DenseHashMap::new();
//! map.insert("one".to_string(), 1);
//! map.insert("two".to_string(), 2);
//!
//! assert_eq!(map.get("one"), Some(&1));
//! ```

use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::Hash;

/// Default threshold for switching from vector to hashmap storage.
const DEFAULT_THRESHOLD: usize = 128;

/// A space-efficient map that uses vector storage for small sizes.
///
/// When the map has fewer than `THRESHOLD` elements, it uses a `Vec`
/// for storage (O(n) lookup but cache-efficient). Above the threshold,
/// it creates a `HashMap` index for O(1) lookups.
///
/// # Type Parameters
///
/// * `K` - Key type
/// * `V` - Value type
/// * `THRESHOLD` - Size at which to switch to hashmap (default 128)
///
/// # Examples
///
/// ```
/// use usd_tf::dense_hashmap::DenseHashMap;
///
/// let mut map = DenseHashMap::<&str, i32>::new();
/// map.insert("a", 1);
/// map.insert("b", 2);
///
/// assert_eq!(map.len(), 2);
/// assert_eq!(map.get(&"a"), Some(&1));
/// ```
pub struct DenseHashMap<K, V, const THRESHOLD: usize = DEFAULT_THRESHOLD> {
    /// Vector storage for key-value pairs (always used).
    vec: Vec<(K, V)>,
    /// HashMap index (only allocated when size > threshold).
    index: Option<HashMap<K, usize>>,
}

impl<K, V> DenseHashMap<K, V, DEFAULT_THRESHOLD>
where
    K: Hash + Eq + Clone,
{
    /// Creates a new empty dense hash map.
    #[inline]
    pub fn new() -> Self {
        Self {
            vec: Vec::new(),
            index: None,
        }
    }

    /// Creates a dense hash map with the specified capacity.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            vec: Vec::with_capacity(capacity),
            index: None,
        }
    }
}

impl<K, V, const THRESHOLD: usize> DenseHashMap<K, V, THRESHOLD>
where
    K: Hash + Eq + Clone,
{
    /// Creates a new empty dense hash map with custom threshold.
    #[inline]
    pub fn new_with_threshold() -> Self {
        Self {
            vec: Vec::new(),
            index: None,
        }
    }

    /// Returns the number of elements in the map.
    #[inline]
    pub fn len(&self) -> usize {
        self.vec.len()
    }

    /// Returns true if the map is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.vec.is_empty()
    }

    /// Clears the map.
    #[inline]
    pub fn clear(&mut self) {
        self.vec.clear();
        self.index = None;
    }

    /// Inserts a key-value pair into the map.
    ///
    /// Returns the old value if the key was present.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        // C++ insert() is insert-if-absent: do not overwrite an existing key.
        if self.find_index(&key).is_some() {
            return None;
        }

        // Insert new element
        self.vec.push((key.clone(), value));

        // Update or create index if at or above threshold (matches C++ size() >= Threshold).
        if self.vec.len() >= THRESHOLD {
            if let Some(ref mut index) = self.index {
                index.insert(key, self.vec.len() - 1);
            } else {
                self.rebuild_index();
            }
        }

        None
    }

    /// Returns a reference to the value for the given key.
    #[inline]
    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.find_index_q(key).map(|idx| &self.vec[idx].1)
    }

    /// Returns a mutable reference to the value for the given key.
    #[inline]
    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.find_index_q(key).map(|idx| &mut self.vec[idx].1)
    }

    /// Returns true if the map contains the given key.
    #[inline]
    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.find_index_q(key).is_some()
    }

    /// Removes a key from the map, returning the value if present.
    pub fn remove<Q>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let idx = self.find_index_q(key)?;
        let (removed_key, value) = self.vec.swap_remove(idx);

        // Update index
        if let Some(ref mut index) = self.index {
            // Remove the old key from index (use turbofish to avoid Q shadowing)
            index.remove::<K>(&removed_key);
            // If we swapped, update the moved element's index
            if idx < self.vec.len() {
                let moved_key = &self.vec[idx].0;
                index.insert(moved_key.clone(), idx);
            }
        }

        Some(value)
    }

    /// Returns an iterator over the key-value pairs.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.vec.iter().map(|(k, v)| (k, v))
    }

    /// Returns a mutable iterator over the key-value pairs.
    #[inline]
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&K, &mut V)> {
        self.vec.iter_mut().map(|(k, v)| (&*k, v))
    }

    /// Returns an iterator over the keys.
    #[inline]
    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.vec.iter().map(|(k, _)| k)
    }

    /// Returns an iterator over the values.
    #[inline]
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.vec.iter().map(|(_, v)| v)
    }

    /// Returns a mutable iterator over the values.
    #[inline]
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut V> {
        self.vec.iter_mut().map(|(_, v)| v)
    }

    /// Finds the index of a key in the vector.
    fn find_index(&self, key: &K) -> Option<usize> {
        if let Some(ref index) = self.index {
            index.get(key).copied()
        } else {
            self.vec.iter().position(|(k, _)| k == key)
        }
    }

    /// Finds the index of a key using Borrow trait.
    fn find_index_q<Q>(&self, key: &Q) -> Option<usize>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        if let Some(ref index) = self.index {
            index.get(key).copied()
        } else {
            self.vec.iter().position(|(k, _)| k.borrow() == key)
        }
    }

    /// Rebuilds the index from the vector.
    fn rebuild_index(&mut self) {
        let mut index = HashMap::with_capacity(self.vec.len());
        for (i, (k, _)) in self.vec.iter().enumerate() {
            index.insert(k.clone(), i);
        }
        self.index = Some(index);
    }
}

impl<K, V, const THRESHOLD: usize> Default for DenseHashMap<K, V, THRESHOLD>
where
    K: Hash + Eq + Clone,
{
    fn default() -> Self {
        Self {
            vec: Vec::new(),
            index: None,
        }
    }
}

impl<K, V, const THRESHOLD: usize> FromIterator<(K, V)> for DenseHashMap<K, V, THRESHOLD>
where
    K: Hash + Eq + Clone,
{
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let mut map = Self::default();
        for (k, v) in iter {
            map.insert(k, v);
        }
        map
    }
}

impl<K, V, const THRESHOLD: usize> IntoIterator for DenseHashMap<K, V, THRESHOLD> {
    type Item = (K, V);
    type IntoIter = std::vec::IntoIter<(K, V)>;

    fn into_iter(self) -> Self::IntoIter {
        self.vec.into_iter()
    }
}

impl<'a, K, V, const THRESHOLD: usize> IntoIterator for &'a DenseHashMap<K, V, THRESHOLD>
where
    K: Hash + Eq + Clone,
{
    type Item = (&'a K, &'a V);
    type IntoIter = std::iter::Map<std::slice::Iter<'a, (K, V)>, fn(&'a (K, V)) -> (&'a K, &'a V)>;

    fn into_iter(self) -> Self::IntoIter {
        self.vec.iter().map(|(k, v)| (k, v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_operations() {
        let mut map: DenseHashMap<String, i32> = DenseHashMap::new();

        // Insert
        assert_eq!(map.insert("one".to_string(), 1), None);
        assert_eq!(map.insert("two".to_string(), 2), None);
        assert_eq!(map.insert("three".to_string(), 3), None);

        assert_eq!(map.len(), 3);

        // Get
        assert_eq!(map.get("one"), Some(&1));
        assert_eq!(map.get("two"), Some(&2));
        assert_eq!(map.get("four"), None);

        // Re-insert existing key: C++ insert-if-absent returns None and does not overwrite.
        assert_eq!(map.insert("one".to_string(), 10), None);
        assert_eq!(map.get("one"), Some(&1));

        // Contains
        assert!(map.contains_key("one"));
        assert!(!map.contains_key("four"));
    }

    #[test]
    fn test_remove() {
        let mut map: DenseHashMap<i32, &str> = DenseHashMap::new();
        map.insert(1, "one");
        map.insert(2, "two");
        map.insert(3, "three");

        assert_eq!(map.remove(&2), Some("two"));
        assert_eq!(map.len(), 2);
        assert!(!map.contains_key(&2));

        assert_eq!(map.remove(&5), None);
    }

    #[test]
    fn test_clear() {
        let mut map: DenseHashMap<i32, i32> = DenseHashMap::new();
        map.insert(1, 1);
        map.insert(2, 2);

        map.clear();
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
    }

    #[test]
    fn test_iteration() {
        let mut map: DenseHashMap<i32, i32> = DenseHashMap::new();
        map.insert(1, 10);
        map.insert(2, 20);
        map.insert(3, 30);

        let sum: i32 = map.values().sum();
        assert_eq!(sum, 60);

        let key_sum: i32 = map.keys().sum();
        assert_eq!(key_sum, 6);
    }

    #[test]
    fn test_get_mut() {
        let mut map: DenseHashMap<&str, i32> = DenseHashMap::new();
        map.insert("key", 1);

        if let Some(v) = map.get_mut(&"key") {
            *v = 100;
        }

        assert_eq!(map.get(&"key"), Some(&100));
    }

    #[test]
    fn test_threshold_switch() {
        // Use a small threshold for testing
        let mut map: DenseHashMap<i32, i32, 4> = DenseHashMap::new_with_threshold();

        // Below threshold - no index
        for i in 0..4 {
            map.insert(i, i * 10);
        }

        // Above threshold - index created
        map.insert(4, 40);
        map.insert(5, 50);

        // Lookups should still work
        assert_eq!(map.get(&0), Some(&0));
        assert_eq!(map.get(&5), Some(&50));
    }

    #[test]
    fn test_from_iterator() {
        let pairs = vec![("a", 1), ("b", 2), ("c", 3)];
        let map: DenseHashMap<&str, i32> = pairs.into_iter().collect();

        assert_eq!(map.len(), 3);
        assert_eq!(map.get(&"b"), Some(&2));
    }

    #[test]
    fn test_into_iterator() {
        let mut map: DenseHashMap<i32, i32> = DenseHashMap::new();
        map.insert(1, 10);
        map.insert(2, 20);

        let vec: Vec<_> = map.into_iter().collect();
        assert_eq!(vec.len(), 2);
    }

    #[test]
    fn test_with_capacity() {
        let map: DenseHashMap<i32, i32> = DenseHashMap::with_capacity(100);
        assert!(map.is_empty());
    }
}
