//! Map keyed by type info or string aliases.
//!
//! Provides a specialized map that can store values under keys that are
//! either [`TypeId`] or string aliases. This is useful for runtime type
//! systems where types can be looked up by their RTTI or by name.
//!
//! # Features
//!
//! - Primary key is a `TypeId` (from `std::any::TypeId`)
//! - String aliases can be created for any entry
//! - Fast `TypeId` lookup with fallback to string lookup
//! - Automatic aliasing of type names
//!
//! # Examples
//!
//! ```
//! use usd_tf::type_info_map::TypeInfoMap;
//! use std::any::TypeId;
//!
//! let mut map: TypeInfoMap<i32> = TypeInfoMap::new();
//!
//! // Set by type
//! map.set_by_type::<String>(42);
//!
//! // Lookup by type
//! assert_eq!(map.find_by_type::<String>(), Some(&42));
//!
//! // Create string alias
//! map.create_alias_for_type::<String>("MyString");
//! assert_eq!(map.find_by_name("MyString"), Some(&42));
//! ```

use std::any::TypeId;
use std::collections::HashMap;

/// A map that can be keyed by [`TypeId`] or string aliases.
///
/// Each entry has a primary `TypeId` key and can have multiple string aliases.
/// The string name derived from `std::any::type_name` is automatically added
/// as an alias when setting a value by type.
///
/// # Type Parameters
///
/// - `V` - The value type stored in the map
///
/// # Examples
///
/// ```
/// use usd_tf::type_info_map::TypeInfoMap;
///
/// let mut map: TypeInfoMap<String> = TypeInfoMap::new();
///
/// // Store values by type
/// map.set_by_type::<i32>("integer".to_string());
/// map.set_by_type::<f64>("float".to_string());
///
/// // Lookup by type
/// assert_eq!(map.find_by_type::<i32>(), Some(&"integer".to_string()));
/// assert_eq!(map.find_by_type::<f64>(), Some(&"float".to_string()));
///
/// // Create aliases
/// map.create_alias_for_type::<i32>("int");
/// assert_eq!(map.find_by_name("int"), Some(&"integer".to_string()));
/// ```
pub struct TypeInfoMap<V> {
    /// Primary storage: TypeId -> Entry index
    type_id_map: HashMap<TypeId, usize>,
    /// String alias -> Entry index
    string_map: HashMap<String, usize>,
    /// All entries (actual data storage)
    entries: Vec<Entry<V>>,
}

/// Internal entry storing the value and its aliases.
struct Entry<V> {
    /// The primary type ID for this entry (if any)
    type_id: Option<TypeId>,
    /// The primary string key for this entry
    primary_key: String,
    /// All string aliases including primary key
    string_aliases: Vec<String>,
    /// The stored value
    value: V,
}

impl<V> TypeInfoMap<V> {
    /// Create a new empty `TypeInfoMap`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::type_info_map::TypeInfoMap;
    ///
    /// let map: TypeInfoMap<i32> = TypeInfoMap::new();
    /// assert!(map.is_empty());
    /// ```
    pub fn new() -> Self {
        Self {
            type_id_map: HashMap::new(),
            string_map: HashMap::new(),
            entries: Vec::new(),
        }
    }

    /// Check if the map is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::type_info_map::TypeInfoMap;
    ///
    /// let mut map: TypeInfoMap<i32> = TypeInfoMap::new();
    /// assert!(map.is_empty());
    ///
    /// map.set_by_name("test", 42);
    /// assert!(!map.is_empty());
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get the number of entries in the map.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::type_info_map::TypeInfoMap;
    ///
    /// let mut map: TypeInfoMap<i32> = TypeInfoMap::new();
    /// assert_eq!(map.len(), 0);
    ///
    /// map.set_by_name("a", 1);
    /// map.set_by_name("b", 2);
    /// assert_eq!(map.len(), 2);
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if a type exists in the map.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::type_info_map::TypeInfoMap;
    ///
    /// let mut map: TypeInfoMap<i32> = TypeInfoMap::new();
    /// map.set_by_type::<String>(42);
    ///
    /// assert!(map.exists_by_type::<String>());
    /// assert!(!map.exists_by_type::<i32>());
    /// ```
    #[inline]
    pub fn exists_by_type<T: 'static>(&self) -> bool {
        self.find_by_type::<T>().is_some()
    }

    /// Check if a string key exists in the map.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::type_info_map::TypeInfoMap;
    ///
    /// let mut map: TypeInfoMap<i32> = TypeInfoMap::new();
    /// map.set_by_name("test", 42);
    ///
    /// assert!(map.exists_by_name("test"));
    /// assert!(!map.exists_by_name("other"));
    /// ```
    #[inline]
    pub fn exists_by_name(&self, key: &str) -> bool {
        self.find_by_name(key).is_some()
    }

    /// Find a value by type.
    ///
    /// First looks up by `TypeId`, then falls back to looking up by
    /// the type's name string.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::type_info_map::TypeInfoMap;
    ///
    /// let mut map: TypeInfoMap<i32> = TypeInfoMap::new();
    /// map.set_by_type::<String>(42);
    ///
    /// assert_eq!(map.find_by_type::<String>(), Some(&42));
    /// assert_eq!(map.find_by_type::<i32>(), None);
    /// ```
    pub fn find_by_type<T: 'static>(&self) -> Option<&V> {
        let type_id = TypeId::of::<T>();

        // First try direct TypeId lookup
        if let Some(&idx) = self.type_id_map.get(&type_id) {
            return Some(&self.entries[idx].value);
        }

        // Fall back to type name lookup
        let type_name = std::any::type_name::<T>();
        self.find_by_name(type_name)
    }

    /// Find a mutable value by type.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::type_info_map::TypeInfoMap;
    ///
    /// let mut map: TypeInfoMap<i32> = TypeInfoMap::new();
    /// map.set_by_type::<String>(42);
    ///
    /// if let Some(v) = map.find_by_type_mut::<String>() {
    ///     *v = 100;
    /// }
    /// assert_eq!(map.find_by_type::<String>(), Some(&100));
    /// ```
    pub fn find_by_type_mut<T: 'static>(&mut self) -> Option<&mut V> {
        let type_id = TypeId::of::<T>();

        // First try direct TypeId lookup
        if let Some(&idx) = self.type_id_map.get(&type_id) {
            return Some(&mut self.entries[idx].value);
        }

        // Fall back to type name lookup
        let type_name = std::any::type_name::<T>();
        self.find_by_name_mut(type_name)
    }

    /// Find a value by string key.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::type_info_map::TypeInfoMap;
    ///
    /// let mut map: TypeInfoMap<i32> = TypeInfoMap::new();
    /// map.set_by_name("test", 42);
    ///
    /// assert_eq!(map.find_by_name("test"), Some(&42));
    /// assert_eq!(map.find_by_name("other"), None);
    /// ```
    pub fn find_by_name(&self, key: &str) -> Option<&V> {
        self.string_map
            .get(key)
            .map(|&idx| &self.entries[idx].value)
    }

    /// Find a mutable value by string key.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::type_info_map::TypeInfoMap;
    ///
    /// let mut map: TypeInfoMap<i32> = TypeInfoMap::new();
    /// map.set_by_name("test", 42);
    ///
    /// if let Some(v) = map.find_by_name_mut("test") {
    ///     *v = 100;
    /// }
    /// assert_eq!(map.find_by_name("test"), Some(&100));
    /// ```
    pub fn find_by_name_mut(&mut self, key: &str) -> Option<&mut V> {
        self.string_map
            .get(key)
            .copied()
            .map(|idx| &mut self.entries[idx].value)
    }

    /// Set a value by type.
    ///
    /// If the type already exists, updates the value. Otherwise creates a new entry
    /// with the type's name as the primary string key.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::type_info_map::TypeInfoMap;
    ///
    /// let mut map: TypeInfoMap<i32> = TypeInfoMap::new();
    ///
    /// map.set_by_type::<String>(42);
    /// assert_eq!(map.find_by_type::<String>(), Some(&42));
    ///
    /// map.set_by_type::<String>(100);
    /// assert_eq!(map.find_by_type::<String>(), Some(&100));
    /// ```
    pub fn set_by_type<T: 'static>(&mut self, value: V) {
        let type_id = TypeId::of::<T>();
        let type_name = std::any::type_name::<T>();

        // Check if entry exists by TypeId
        if let Some(&idx) = self.type_id_map.get(&type_id) {
            self.entries[idx].value = value;
            return;
        }

        // Check if entry exists by name
        if let Some(&idx) = self.string_map.get(type_name) {
            // Update existing entry and add TypeId alias
            self.entries[idx].value = value;
            self.entries[idx].type_id = Some(type_id);
            self.type_id_map.insert(type_id, idx);
            return;
        }

        // Create new entry
        let idx = self.entries.len();
        let entry = Entry {
            type_id: Some(type_id),
            primary_key: type_name.to_string(),
            string_aliases: vec![type_name.to_string()],
            value,
        };
        self.entries.push(entry);
        self.type_id_map.insert(type_id, idx);
        self.string_map.insert(type_name.to_string(), idx);
    }

    /// Set a value by string key.
    ///
    /// If the key already exists, updates the value. Otherwise creates a new entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::type_info_map::TypeInfoMap;
    ///
    /// let mut map: TypeInfoMap<i32> = TypeInfoMap::new();
    ///
    /// map.set_by_name("test", 42);
    /// assert_eq!(map.find_by_name("test"), Some(&42));
    ///
    /// map.set_by_name("test", 100);
    /// assert_eq!(map.find_by_name("test"), Some(&100));
    /// ```
    pub fn set_by_name(&mut self, key: &str, value: V) {
        if let Some(&idx) = self.string_map.get(key) {
            self.entries[idx].value = value;
            return;
        }

        // Create new entry
        let idx = self.entries.len();
        let entry = Entry {
            type_id: None,
            primary_key: key.to_string(),
            string_aliases: vec![key.to_string()],
            value,
        };
        self.entries.push(entry);
        self.string_map.insert(key.to_string(), idx);
    }

    /// Create a string alias for a type entry.
    ///
    /// Returns `true` if the alias was created, `false` if the type doesn't exist.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::type_info_map::TypeInfoMap;
    ///
    /// let mut map: TypeInfoMap<i32> = TypeInfoMap::new();
    /// map.set_by_type::<String>(42);
    ///
    /// assert!(map.create_alias_for_type::<String>("MyString"));
    /// assert_eq!(map.find_by_name("MyString"), Some(&42));
    ///
    /// assert!(!map.create_alias_for_type::<i32>("MyInt")); // i32 not in map
    /// ```
    pub fn create_alias_for_type<T: 'static>(&mut self, alias: &str) -> bool {
        let type_id = TypeId::of::<T>();

        if let Some(&idx) = self.type_id_map.get(&type_id) {
            self.create_alias_for_index(idx, alias);
            return true;
        }

        // Try type name fallback
        let type_name = std::any::type_name::<T>();
        if let Some(&idx) = self.string_map.get(type_name) {
            self.create_alias_for_index(idx, alias);
            return true;
        }

        false
    }

    /// Create a string alias for another string key.
    ///
    /// Returns `true` if the alias was created, `false` if the key doesn't exist.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::type_info_map::TypeInfoMap;
    ///
    /// let mut map: TypeInfoMap<i32> = TypeInfoMap::new();
    /// map.set_by_name("original", 42);
    ///
    /// assert!(map.create_alias("alias", "original"));
    /// assert_eq!(map.find_by_name("alias"), Some(&42));
    ///
    /// assert!(!map.create_alias("other", "nonexistent"));
    /// ```
    pub fn create_alias(&mut self, alias: &str, key: &str) -> bool {
        if let Some(&idx) = self.string_map.get(key) {
            self.create_alias_for_index(idx, alias);
            return true;
        }
        false
    }

    /// Internal helper to create an alias for an entry by index.
    fn create_alias_for_index(&mut self, idx: usize, alias: &str) {
        if self.string_map.contains_key(alias) {
            return; // Alias already exists
        }

        self.string_map.insert(alias.to_string(), idx);
        self.entries[idx].string_aliases.push(alias.to_string());
    }

    /// Remove an entry by type.
    ///
    /// This removes the entry and all its aliases.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::type_info_map::TypeInfoMap;
    ///
    /// let mut map: TypeInfoMap<i32> = TypeInfoMap::new();
    /// map.set_by_type::<String>(42);
    /// map.create_alias_for_type::<String>("MyString");
    ///
    /// map.remove_by_type::<String>();
    ///
    /// assert_eq!(map.find_by_type::<String>(), None);
    /// assert_eq!(map.find_by_name("MyString"), None);
    /// ```
    pub fn remove_by_type<T: 'static>(&mut self) {
        let type_id = TypeId::of::<T>();

        if let Some(&idx) = self.type_id_map.get(&type_id) {
            self.remove_by_index(idx);
            return;
        }

        // Try type name fallback
        let type_name = std::any::type_name::<T>();
        if let Some(&idx) = self.string_map.get(type_name) {
            self.remove_by_index(idx);
        }
    }

    /// Remove an entry by string key.
    ///
    /// This removes the entry and all its aliases.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::type_info_map::TypeInfoMap;
    ///
    /// let mut map: TypeInfoMap<i32> = TypeInfoMap::new();
    /// map.set_by_name("test", 42);
    /// map.create_alias("alias", "test");
    ///
    /// map.remove_by_name("test");
    ///
    /// assert_eq!(map.find_by_name("test"), None);
    /// assert_eq!(map.find_by_name("alias"), None);
    /// ```
    pub fn remove_by_name(&mut self, key: &str) {
        if let Some(&idx) = self.string_map.get(key) {
            self.remove_by_index(idx);
        }
    }

    /// Internal helper to remove an entry by index.
    ///
    /// This is O(n) because we need to update indices in the hash maps
    /// when we swap_remove the entry.
    fn remove_by_index(&mut self, idx: usize) {
        if idx >= self.entries.len() {
            return;
        }

        // Remove all string aliases
        let aliases: Vec<String> = self.entries[idx].string_aliases.clone();
        for alias in &aliases {
            self.string_map.remove(alias);
        }

        // Remove TypeId mapping
        if let Some(type_id) = self.entries[idx].type_id {
            self.type_id_map.remove(&type_id);
        }

        // If this isn't the last entry, swap_remove and update the swapped entry's indices
        if idx < self.entries.len() - 1 {
            let last_idx = self.entries.len() - 1;

            // Update indices for the entry that will be swapped
            for alias in &self.entries[last_idx].string_aliases {
                if let Some(map_idx) = self.string_map.get_mut(alias) {
                    *map_idx = idx;
                }
            }
            if let Some(type_id) = self.entries[last_idx].type_id {
                if let Some(map_idx) = self.type_id_map.get_mut(&type_id) {
                    *map_idx = idx;
                }
            }
        }

        self.entries.swap_remove(idx);
    }

    /// Clear all entries from the map.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::type_info_map::TypeInfoMap;
    ///
    /// let mut map: TypeInfoMap<i32> = TypeInfoMap::new();
    /// map.set_by_name("a", 1);
    /// map.set_by_name("b", 2);
    ///
    /// map.clear();
    /// assert!(map.is_empty());
    /// ```
    pub fn clear(&mut self) {
        self.entries.clear();
        self.type_id_map.clear();
        self.string_map.clear();
    }

    /// Iterate over all values in the map.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::type_info_map::TypeInfoMap;
    ///
    /// let mut map: TypeInfoMap<i32> = TypeInfoMap::new();
    /// map.set_by_name("a", 1);
    /// map.set_by_name("b", 2);
    ///
    /// let values: Vec<_> = map.values().copied().collect();
    /// assert!(values.contains(&1));
    /// assert!(values.contains(&2));
    /// ```
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.entries.iter().map(|e| &e.value)
    }

    /// Iterate over all primary keys in the map.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::type_info_map::TypeInfoMap;
    ///
    /// let mut map: TypeInfoMap<i32> = TypeInfoMap::new();
    /// map.set_by_name("a", 1);
    /// map.set_by_name("b", 2);
    ///
    /// let keys: Vec<_> = map.keys().collect();
    /// assert!(keys.contains(&"a"));
    /// assert!(keys.contains(&"b"));
    /// ```
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|e| e.primary_key.as_str())
    }

    /// Iterate over all (primary key, value) pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &V)> {
        self.entries
            .iter()
            .map(|e| (e.primary_key.as_str(), &e.value))
    }
}

impl<V> Default for TypeInfoMap<V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V: Clone> Clone for TypeInfoMap<V> {
    fn clone(&self) -> Self {
        Self {
            type_id_map: self.type_id_map.clone(),
            string_map: self.string_map.clone(),
            entries: self
                .entries
                .iter()
                .map(|e| Entry {
                    type_id: e.type_id,
                    primary_key: e.primary_key.clone(),
                    string_aliases: e.string_aliases.clone(),
                    value: e.value.clone(),
                })
                .collect(),
        }
    }
}

impl<V: std::fmt::Debug> std::fmt::Debug for TypeInfoMap<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map()
            .entries(self.entries.iter().map(|e| (&e.primary_key, &e.value)))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_empty() {
        let map: TypeInfoMap<i32> = TypeInfoMap::new();
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
    }

    #[test]
    fn test_set_and_find_by_type() {
        let mut map: TypeInfoMap<i32> = TypeInfoMap::new();

        map.set_by_type::<String>(42);
        assert_eq!(map.find_by_type::<String>(), Some(&42));
        assert_eq!(map.find_by_type::<Vec<u8>>(), None);
    }

    #[test]
    fn test_set_and_find_by_name() {
        let mut map: TypeInfoMap<i32> = TypeInfoMap::new();

        map.set_by_name("test", 42);
        assert_eq!(map.find_by_name("test"), Some(&42));
        assert_eq!(map.find_by_name("other"), None);
    }

    #[test]
    fn test_update_existing_by_type() {
        let mut map: TypeInfoMap<i32> = TypeInfoMap::new();

        map.set_by_type::<String>(42);
        map.set_by_type::<String>(100);

        assert_eq!(map.find_by_type::<String>(), Some(&100));
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn test_update_existing_by_name() {
        let mut map: TypeInfoMap<i32> = TypeInfoMap::new();

        map.set_by_name("test", 42);
        map.set_by_name("test", 100);

        assert_eq!(map.find_by_name("test"), Some(&100));
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn test_find_mut_by_type() {
        let mut map: TypeInfoMap<i32> = TypeInfoMap::new();
        map.set_by_type::<String>(42);

        if let Some(v) = map.find_by_type_mut::<String>() {
            *v = 100;
        }

        assert_eq!(map.find_by_type::<String>(), Some(&100));
    }

    #[test]
    fn test_find_mut_by_name() {
        let mut map: TypeInfoMap<i32> = TypeInfoMap::new();
        map.set_by_name("test", 42);

        if let Some(v) = map.find_by_name_mut("test") {
            *v = 100;
        }

        assert_eq!(map.find_by_name("test"), Some(&100));
    }

    #[test]
    fn test_create_alias_for_type() {
        let mut map: TypeInfoMap<i32> = TypeInfoMap::new();
        map.set_by_type::<String>(42);

        assert!(map.create_alias_for_type::<String>("MyString"));
        assert_eq!(map.find_by_name("MyString"), Some(&42));

        // Same value through both lookups
        assert_eq!(map.find_by_type::<String>(), map.find_by_name("MyString"));
    }

    #[test]
    fn test_create_alias_for_nonexistent() {
        let mut map: TypeInfoMap<i32> = TypeInfoMap::new();

        assert!(!map.create_alias_for_type::<String>("MyString"));
        assert_eq!(map.find_by_name("MyString"), None);
    }

    #[test]
    fn test_create_string_alias() {
        let mut map: TypeInfoMap<i32> = TypeInfoMap::new();
        map.set_by_name("original", 42);

        assert!(map.create_alias("alias", "original"));
        assert_eq!(map.find_by_name("alias"), Some(&42));
        assert_eq!(map.find_by_name("original"), Some(&42));
    }

    #[test]
    fn test_remove_by_type() {
        let mut map: TypeInfoMap<i32> = TypeInfoMap::new();
        map.set_by_type::<String>(42);
        map.create_alias_for_type::<String>("MyString");

        map.remove_by_type::<String>();

        assert_eq!(map.find_by_type::<String>(), None);
        assert_eq!(map.find_by_name("MyString"), None);
        assert!(map.is_empty());
    }

    #[test]
    fn test_remove_by_name() {
        let mut map: TypeInfoMap<i32> = TypeInfoMap::new();
        map.set_by_name("test", 42);
        map.create_alias("alias", "test");

        map.remove_by_name("test");

        assert_eq!(map.find_by_name("test"), None);
        assert_eq!(map.find_by_name("alias"), None);
        assert!(map.is_empty());
    }

    #[test]
    fn test_remove_maintains_other_entries() {
        let mut map: TypeInfoMap<i32> = TypeInfoMap::new();
        map.set_by_name("a", 1);
        map.set_by_name("b", 2);
        map.set_by_name("c", 3);

        map.remove_by_name("b");

        assert_eq!(map.find_by_name("a"), Some(&1));
        assert_eq!(map.find_by_name("b"), None);
        assert_eq!(map.find_by_name("c"), Some(&3));
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn test_exists_by_type() {
        let mut map: TypeInfoMap<i32> = TypeInfoMap::new();
        map.set_by_type::<String>(42);

        assert!(map.exists_by_type::<String>());
        assert!(!map.exists_by_type::<Vec<u8>>());
    }

    #[test]
    fn test_exists_by_name() {
        let mut map: TypeInfoMap<i32> = TypeInfoMap::new();
        map.set_by_name("test", 42);

        assert!(map.exists_by_name("test"));
        assert!(!map.exists_by_name("other"));
    }

    #[test]
    fn test_clear() {
        let mut map: TypeInfoMap<i32> = TypeInfoMap::new();
        map.set_by_name("a", 1);
        map.set_by_name("b", 2);

        map.clear();

        assert!(map.is_empty());
        assert_eq!(map.find_by_name("a"), None);
    }

    #[test]
    fn test_values_iterator() {
        let mut map: TypeInfoMap<i32> = TypeInfoMap::new();
        map.set_by_name("a", 1);
        map.set_by_name("b", 2);
        map.set_by_name("c", 3);

        let values: Vec<i32> = map.values().copied().collect();
        assert_eq!(values.len(), 3);
        assert!(values.contains(&1));
        assert!(values.contains(&2));
        assert!(values.contains(&3));
    }

    #[test]
    fn test_keys_iterator() {
        let mut map: TypeInfoMap<i32> = TypeInfoMap::new();
        map.set_by_name("a", 1);
        map.set_by_name("b", 2);

        let keys: Vec<&str> = map.keys().collect();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"a"));
        assert!(keys.contains(&"b"));
    }

    #[test]
    fn test_iter() {
        let mut map: TypeInfoMap<i32> = TypeInfoMap::new();
        map.set_by_name("a", 1);
        map.set_by_name("b", 2);

        let pairs: Vec<_> = map.iter().collect();
        assert_eq!(pairs.len(), 2);
    }

    #[test]
    fn test_clone() {
        let mut map: TypeInfoMap<i32> = TypeInfoMap::new();
        map.set_by_name("a", 1);
        map.set_by_name("b", 2);
        map.create_alias("c", "a");

        let cloned = map.clone();

        assert_eq!(cloned.find_by_name("a"), Some(&1));
        assert_eq!(cloned.find_by_name("b"), Some(&2));
        assert_eq!(cloned.find_by_name("c"), Some(&1));
    }

    #[test]
    fn test_multiple_types() {
        let mut map: TypeInfoMap<String> = TypeInfoMap::new();

        map.set_by_type::<i32>("integer".to_string());
        map.set_by_type::<f64>("float".to_string());
        map.set_by_type::<String>("string".to_string());

        assert_eq!(map.find_by_type::<i32>(), Some(&"integer".to_string()));
        assert_eq!(map.find_by_type::<f64>(), Some(&"float".to_string()));
        assert_eq!(map.find_by_type::<String>(), Some(&"string".to_string()));
    }

    #[test]
    fn test_type_and_name_interop() {
        let mut map: TypeInfoMap<i32> = TypeInfoMap::new();

        // Set by type
        map.set_by_type::<String>(42);

        // Should be findable by type name
        let type_name = std::any::type_name::<String>();
        assert_eq!(map.find_by_name(type_name), Some(&42));

        // Create an alias
        map.create_alias("MyString", type_name);

        // All three should work
        assert_eq!(map.find_by_type::<String>(), Some(&42));
        assert_eq!(map.find_by_name(type_name), Some(&42));
        assert_eq!(map.find_by_name("MyString"), Some(&42));
    }

    #[test]
    fn test_default() {
        let map: TypeInfoMap<i32> = TypeInfoMap::default();
        assert!(map.is_empty());
    }

    #[test]
    fn test_debug() {
        let mut map: TypeInfoMap<i32> = TypeInfoMap::new();
        map.set_by_name("test", 42);

        let debug_str = format!("{:?}", map);
        assert!(debug_str.contains("test"));
        assert!(debug_str.contains("42"));
    }
}
