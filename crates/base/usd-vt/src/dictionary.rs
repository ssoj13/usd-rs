//! Dictionary type for key-value storage.
//!
//! `Dictionary` is a string-keyed map that stores `Value` objects.
//! This is the Rust equivalent of OpenUSD's `VtDictionary`.
//!
//! # Examples
//!
//! ```
//! use usd_vt::{Dictionary, Value};
//!
//! let mut dict = Dictionary::new();
//! dict.insert("name", "MyObject");
//! dict.insert("size", 42i32);
//!
//! assert_eq!(dict.get("name").and_then(|v| v.get::<String>()), Some(&"MyObject".to_string()));
//! ```

use std::collections::BTreeMap;
use std::collections::btree_map;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Index;

use super::Value;

/// A string-keyed dictionary storing type-erased values.
///
/// `Dictionary` maps string keys to `Value` objects, providing a flexible
/// way to store heterogeneous data. It is commonly used in USD for metadata
/// and custom data storage.
///
/// # Examples
///
/// ```
/// use usd_vt::{Dictionary, Value};
///
/// let mut dict = Dictionary::new();
///
/// // Insert values of different types
/// dict.insert("visible", true);
/// dict.insert("scale", 2.5f64);
/// dict.insert_value("children", Value::new(vec![1i32, 2, 3]));
///
/// // Access values
/// if let Some(val) = dict.get("visible") {
///     if let Some(&b) = val.get::<bool>() {
///         println!("Visible: {}", b);
///     }
/// }
/// ```
#[derive(Clone, Default)]
pub struct Dictionary {
    /// The underlying storage (BTreeMap for sorted iteration matching C++ std::map).
    data: BTreeMap<String, Value>,
}

impl Dictionary {
    /// Creates a new empty dictionary.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Dictionary;
    ///
    /// let dict = Dictionary::new();
    /// assert!(dict.is_empty());
    /// ```
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: BTreeMap::new(),
        }
    }

    /// Creates a dictionary with the given capacity.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Dictionary;
    ///
    /// let dict = Dictionary::with_capacity(10);
    /// ```
    #[inline]
    #[must_use]
    pub fn with_capacity(_capacity: usize) -> Self {
        // BTreeMap doesn't have with_capacity; capacity hint is ignored
        Self {
            data: BTreeMap::new(),
        }
    }

    /// Returns the number of entries.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Dictionary;
    ///
    /// let mut dict = Dictionary::new();
    /// dict.insert("a", 1i32);
    /// dict.insert("b", 2i32);
    /// assert_eq!(dict.len(), 2);
    /// ```
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns true if the dictionary is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Dictionary;
    ///
    /// let dict = Dictionary::new();
    /// assert!(dict.is_empty());
    /// ```
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Inserts a key-value pair.
    ///
    /// Returns the previous value if the key existed.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Dictionary;
    ///
    /// let mut dict = Dictionary::new();
    /// assert!(dict.insert("key", 42i32).is_none());
    /// assert!(dict.insert("key", 43i32).is_some()); // Returns old value
    ///
    /// // Also works with floats
    /// dict.insert("pi", 3.14f64);
    /// ```
    #[inline]
    pub fn insert<K, V>(&mut self, key: K, value: V) -> Option<Value>
    where
        K: Into<String>,
        V: Into<Value>,
    {
        self.data.insert(key.into(), value.into())
    }

    /// Inserts a Value directly.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::{Dictionary, Value};
    ///
    /// let mut dict = Dictionary::new();
    /// dict.insert_value("key", Value::from(42i32));
    /// ```
    #[inline]
    pub fn insert_value<K: Into<String>>(&mut self, key: K, value: Value) -> Option<Value> {
        self.data.insert(key.into(), value)
    }

    /// Gets a reference to a value by key.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Dictionary;
    ///
    /// let mut dict = Dictionary::new();
    /// dict.insert("key", 42i32);
    ///
    /// assert!(dict.get("key").is_some());
    /// assert!(dict.get("missing").is_none());
    /// ```
    #[inline]
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.data.get(key)
    }

    /// Gets a mutable reference to a value by key.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::{Dictionary, Value};
    ///
    /// let mut dict = Dictionary::new();
    /// dict.insert("key", 42i32);
    ///
    /// if let Some(v) = dict.get_mut("key") {
    ///     *v = Value::from(100i32);
    /// }
    /// ```
    #[inline]
    pub fn get_mut(&mut self, key: &str) -> Option<&mut Value> {
        self.data.get_mut(key)
    }

    /// Returns true if the dictionary contains the key.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Dictionary;
    ///
    /// let mut dict = Dictionary::new();
    /// dict.insert("key", 42i32);
    ///
    /// assert!(dict.contains_key("key"));
    /// assert!(!dict.contains_key("missing"));
    /// ```
    #[inline]
    #[must_use]
    pub fn contains_key(&self, key: &str) -> bool {
        self.data.contains_key(key)
    }

    /// Removes a key and returns its value if present.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Dictionary;
    ///
    /// let mut dict = Dictionary::new();
    /// dict.insert("key", 42i32);
    ///
    /// assert!(dict.remove("key").is_some());
    /// assert!(dict.remove("key").is_none());
    /// ```
    #[inline]
    pub fn remove(&mut self, key: &str) -> Option<Value> {
        self.data.remove(key)
    }

    /// Clears all entries from the dictionary.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Dictionary;
    ///
    /// let mut dict = Dictionary::new();
    /// dict.insert("a", 1i32);
    /// dict.insert("b", 2i32);
    ///
    /// dict.clear();
    /// assert!(dict.is_empty());
    /// ```
    #[inline]
    pub fn clear(&mut self) {
        self.data.clear();
    }

    /// Returns an iterator over keys.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Dictionary;
    ///
    /// let mut dict = Dictionary::new();
    /// dict.insert("a", 1i32);
    /// dict.insert("b", 2i32);
    ///
    /// for key in dict.keys() {
    ///     println!("{}", key);
    /// }
    /// ```
    #[inline]
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.data.keys()
    }

    /// Returns an iterator over values.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Dictionary;
    ///
    /// let mut dict = Dictionary::new();
    /// dict.insert("a", 1i32);
    /// dict.insert("b", 2i32);
    ///
    /// for value in dict.values() {
    ///     println!("{:?}", value);
    /// }
    /// ```
    #[inline]
    pub fn values(&self) -> impl Iterator<Item = &Value> {
        self.data.values()
    }

    /// Returns an iterator over key-value pairs.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Dictionary;
    ///
    /// let mut dict = Dictionary::new();
    /// dict.insert("a", 1i32);
    ///
    /// for (key, value) in dict.iter() {
    ///     println!("{}: {:?}", key, value);
    /// }
    /// ```
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Value)> {
        self.data.iter()
    }

    /// Merges another dictionary into this one.
    ///
    /// Existing keys are overwritten.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Dictionary;
    ///
    /// let mut dict1 = Dictionary::new();
    /// dict1.insert("a", 1i32);
    ///
    /// let mut dict2 = Dictionary::new();
    /// dict2.insert("b", 2i32);
    ///
    /// dict1.merge(dict2);
    /// assert!(dict1.contains_key("a"));
    /// assert!(dict1.contains_key("b"));
    /// ```
    pub fn merge(&mut self, other: Dictionary) {
        for (key, value) in other.data {
            self.data.insert(key, value);
        }
    }

    /// Gets a typed value if present and correct type.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_vt::Dictionary;
    ///
    /// let mut dict = Dictionary::new();
    /// dict.insert("num", 42i32);
    ///
    /// assert_eq!(dict.get_as::<i32>("num"), Some(&42));
    /// assert_eq!(dict.get_as::<f64>("num"), None);
    /// ```
    #[inline]
    #[must_use]
    pub fn get_as<T: 'static>(&self, key: &str) -> Option<&T> {
        self.data.get(key).and_then(|v| v.get::<T>())
    }

    /// Returns a reference to the value at `key_path` if it exists.
    ///
    /// `key_path` is a delimited string of sub-dictionary names.
    /// Key path elements are produced by splitting `key_path` with `delimiters`.
    /// `key_path` may identify a leaf element or an entire sub-dictionary.
    ///
    /// Matches C++ `VtDictionary::GetValueAtPath(std::string const &keyPath, char const *delimiters)`.
    #[must_use]
    pub fn get_value_at_path(&self, key_path: &str, delimiters: &str) -> Option<&Value> {
        let parts: Vec<&str> = key_path.split(delimiters).collect();
        self.get_value_at_path_parts(&parts)
    }

    /// Returns a reference to the value at `key_path` if it exists.
    ///
    /// `key_path` is a vector of sub-dictionary names.
    ///
    /// Matches C++ `VtDictionary::GetValueAtPath(std::vector<std::string> const &keyPath)`.
    #[must_use]
    pub fn get_value_at_path_parts(&self, key_path: &[&str]) -> Option<&Value> {
        if key_path.is_empty() {
            return None;
        }

        // First lookup in our own BTreeMap
        let first_value = self.data.get(key_path[0])?;
        if key_path.len() == 1 {
            return Some(first_value);
        }

        // Traverse nested Dictionary values
        let mut current_value = first_value;
        for key in &key_path[1..] {
            let nested = current_value.get::<Dictionary>()?;
            current_value = nested.get(*key)?;
        }
        Some(current_value)
    }

    /// Sets the value at `key_path` to `value`.
    ///
    /// `key_path` is a delimited string of sub-dictionary names.
    /// Creates sub-dictionaries as necessary.
    ///
    /// Matches C++ `VtDictionary::SetValueAtPath(std::string const &keyPath, VtValue const &value, char const *delimiters)`.
    pub fn set_value_at_path(&mut self, key_path: &str, value: &Value, delimiters: &str) {
        let parts: Vec<String> = key_path.split(delimiters).map(|s| s.to_string()).collect();
        self.set_value_at_path_parts(&parts, value);
    }

    /// Sets the value at `key_path` to `value`.
    ///
    /// `key_path` is a vector of sub-dictionary names.
    /// Creates intermediate Dictionary values as needed.
    ///
    /// Matches C++ `VtDictionary::SetValueAtPath(std::vector<std::string> const &keyPath, VtValue const &value)`.
    pub fn set_value_at_path_parts(&mut self, key_path: &[String], value: &Value) {
        if key_path.is_empty() {
            return;
        }

        if key_path.len() == 1 {
            self.data.insert(key_path[0].clone(), value.clone());
        } else {
            let first_key = &key_path[0];
            // Get or create nested Dictionary
            let nested_value = self
                .data
                .entry(first_key.clone())
                .or_insert_with(|| Value::new(Dictionary::new()));

            if let Some(nested_dict) = nested_value.get::<Dictionary>() {
                let mut new_dict = nested_dict.clone();
                new_dict.set_value_at_path_parts(&key_path[1..], value);
                *nested_value = Value::new(new_dict);
            } else {
                let mut new_dict = Dictionary::new();
                new_dict.set_value_at_path_parts(&key_path[1..], value);
                *nested_value = Value::new(new_dict);
            }
        }
    }

    /// Erases the value at `key_path`.
    ///
    /// `key_path` is a delimited string of sub-dictionary names.
    ///
    /// Matches C++ `VtDictionary::EraseValueAtPath(std::string const &keyPath, char const *delimiters)`.
    pub fn erase_value_at_path(&mut self, key_path: &str, delimiters: &str) {
        let parts: Vec<String> = key_path.split(delimiters).map(|s| s.to_string()).collect();
        self.erase_value_at_path_parts(&parts);
    }

    /// Erases the value at `key_path`.
    ///
    /// `key_path` is a vector of sub-dictionary names.
    /// Removes empty intermediate dictionaries.
    ///
    /// Matches C++ `VtDictionary::EraseValueAtPath(std::vector<std::string> const &keyPath)`.
    pub fn erase_value_at_path_parts(&mut self, key_path: &[String]) {
        if key_path.is_empty() {
            return;
        }

        if key_path.len() == 1 {
            self.data.remove(&key_path[0]);
        } else if let Some(nested_value) = self.data.get_mut(&key_path[0]) {
            if let Some(nested_dict) = nested_value.get::<Dictionary>() {
                let mut new_dict = nested_dict.clone();
                new_dict.erase_value_at_path_parts(&key_path[1..]);

                if new_dict.is_empty() {
                    self.data.remove(&key_path[0]);
                } else {
                    *nested_value = Value::new(new_dict);
                }
            }
        }
    }

    /// Returns true if the dictionary contains `key` and the corresponding value is of type `T`.
    ///
    /// Matches C++ `VtDictionaryIsHolding<T>(VtDictionary const &dictionary, std::string const &key)`.
    #[inline]
    #[must_use]
    pub fn is_holding<T: 'static>(&self, key: &str) -> bool {
        self.data.get(key).is_some_and(|v| v.is::<T>())
    }

    /// Returns a mutable reference to the value for `key`, inserting an empty
    /// `Value` if the key is not present.
    ///
    /// Matches the C++ `VtDictionary::operator[]` semantics which auto-inserts
    /// a default-constructed `VtValue()` when the key is absent.
    #[inline]
    pub fn get_or_insert_default(&mut self, key: &str) -> &mut Value {
        self.data
            .entry(key.to_string())
            .or_insert_with(Value::empty)
    }

    /// Returns 1 if the key exists, 0 otherwise.
    ///
    /// Matches C++ `VtDictionary::count(const std::string& key)`.
    #[inline]
    #[must_use]
    pub fn count(&self, key: &str) -> usize {
        usize::from(self.data.contains_key(key))
    }

    /// Swaps contents with another dictionary.
    ///
    /// Matches C++ `VtDictionary::swap(VtDictionary& dict)`.
    #[inline]
    pub fn swap(&mut self, other: &mut Dictionary) {
        std::mem::swap(&mut self.data, &mut other.data);
    }

    /// Provides access to the entry API for in-place manipulation.
    ///
    /// Matches C++ `VtDictionary::operator[]` insert-or-access semantics.
    #[inline]
    pub fn entry(&mut self, key: String) -> btree_map::Entry<'_, String, Value> {
        self.data.entry(key)
    }

    /// Returns a mutable iterator over values.
    #[inline]
    pub fn values_mut(&mut self) -> btree_map::ValuesMut<'_, String, Value> {
        self.data.values_mut()
    }

    /// Returns a mutable iterator over key-value pairs.
    #[inline]
    pub fn iter_mut(&mut self) -> btree_map::IterMut<'_, String, Value> {
        self.data.iter_mut()
    }

    /// Retains only elements for which the predicate returns true.
    ///
    /// This is the Rust equivalent of C++ erase-by-range with a filter.
    #[inline]
    pub fn retain<F>(&mut self, f: F)
    where
        F: FnMut(&String, &mut Value) -> bool,
    {
        self.data.retain(f);
    }

    /// Extends the dictionary from an iterator of key-value pairs.
    ///
    /// Matches C++ `VtDictionary::insert(InputIterator f, InputIterator l)`.
    pub fn extend_from<I, K>(&mut self, iter: I)
    where
        I: IntoIterator<Item = (K, Value)>,
        K: Into<String>,
    {
        for (k, v) in iter {
            self.data.insert(k.into(), v);
        }
    }

    /// Converts this Dictionary to a `HashMap<String, Value>`.
    #[must_use]
    pub fn to_hash_map(&self) -> std::collections::HashMap<String, Value> {
        self.data
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    /// Returns a typed value or a default if the key is missing or wrong type.
    ///
    /// Matches C++ `VtDictionaryGet<T>(dict, key, VtDefault = ...)`.
    #[inline]
    #[must_use]
    pub fn get_or<T: 'static + Clone>(&self, key: &str, default: T) -> T {
        self.data
            .get(key)
            .and_then(|v| v.get::<T>())
            .cloned()
            .unwrap_or(default)
    }
}

/// Returns a reference to an empty Dictionary.
///
/// Matches C++ `VtGetEmptyDictionary()`.
#[inline]
#[must_use]
pub fn get_empty_dictionary() -> Dictionary {
    Dictionary::new()
}

// ============================================================================
// Free functions: VtDictionaryOver / VtDictionaryOverRecursive
// ============================================================================

/// Non-recursive compose: strong values override weak, no nested merge.
///
/// If `coerce` is true, coerce strong values to the weaker value's type
/// (mainly for enum type promotion).
///
/// Matches C++ `VtDictionaryOver(const VtDictionary&, const VtDictionary&, bool)`.
pub fn dictionary_over(strong: &Dictionary, weak: &Dictionary) -> Dictionary {
    dictionary_over_coerce(strong, weak, false)
}

/// Non-recursive compose with optional type coercion.
pub fn dictionary_over_coerce(strong: &Dictionary, weak: &Dictionary, coerce: bool) -> Dictionary {
    let mut result = strong.clone();
    dictionary_over_in_place_coerce(&mut result, weak, coerce);
    result
}

/// Non-recursive in-place compose: inserts missing keys from `weak` into `strong`.
///
/// Matches C++ `VtDictionaryOver(VtDictionary*, const VtDictionary&, bool)`.
pub fn dictionary_over_in_place(strong: &mut Dictionary, weak: &Dictionary) {
    dictionary_over_in_place_coerce(strong, weak, false);
}

/// Non-recursive in-place compose with optional type coercion.
pub fn dictionary_over_in_place_coerce(strong: &mut Dictionary, weak: &Dictionary, coerce: bool) {
    // Insert keys from weak that are missing in strong.
    for (key, value) in weak.iter() {
        if !strong.contains_key(key) {
            strong.insert_value(key.clone(), value.clone());
        }
    }
    // If coercing, cast strong values to the weaker value's type where both exist.
    if coerce {
        let keys: Vec<String> = strong.keys().cloned().collect();
        for key in keys {
            if let Some(weak_val) = weak.get(&key) {
                if let Some(strong_val) = strong.get(&key) {
                    if let Some(coerced) = strong_val.cast_to_type_of(weak_val) {
                        strong.insert_value(key, coerced);
                    }
                }
            }
        }
    }
}

/// Non-recursive compose into weak: strong values overwrite weak.
///
/// Matches C++ `VtDictionaryOver(const VtDictionary&, VtDictionary*, bool)`.
pub fn dictionary_over_into_weak(strong: &Dictionary, weak: &mut Dictionary) {
    dictionary_over_into_weak_coerce(strong, weak, false);
}

/// Non-recursive compose into weak with optional type coercion.
pub fn dictionary_over_into_weak_coerce(strong: &Dictionary, weak: &mut Dictionary, coerce: bool) {
    if coerce {
        for (key, strong_val) in strong.iter() {
            if let Some(weak_val) = weak.get(key) {
                // Coerce strong to weak's type, then overwrite.
                let coerced = strong_val
                    .cast_to_type_of(weak_val)
                    .unwrap_or_else(|| strong_val.clone());
                weak.insert_value(key.clone(), coerced);
            } else {
                weak.insert_value(key.clone(), strong_val.clone());
            }
        }
    } else {
        for (key, value) in strong.iter() {
            weak.insert_value(key.clone(), value.clone());
        }
    }
}

/// Recursive compose: nested dictionaries are merged recursively.
///
/// Matches C++ `VtDictionaryOverRecursive(const VtDictionary&, const VtDictionary&)`.
pub fn dictionary_over_recursive(strong: &Dictionary, weak: &Dictionary) -> Dictionary {
    let mut result = strong.clone();
    dictionary_over_recursive_in_place(&mut result, weak);
    result
}

/// Recursive in-place compose into strong: merge weak keys, recurse into nested dicts.
///
/// Matches C++ `VtDictionaryOverRecursive(VtDictionary*, const VtDictionary&)`.
pub fn dictionary_over_recursive_in_place(strong: &mut Dictionary, weak: &Dictionary) {
    for (key, weak_val) in weak.iter() {
        if let Some(strong_val) = strong.get(key) {
            // Both have this key - try recursive compose for nested dicts
            if let (Some(s_dict), Some(w_dict)) =
                (strong_val.get::<Dictionary>(), weak_val.get::<Dictionary>())
            {
                let mut merged = s_dict.clone();
                dictionary_over_recursive_in_place(&mut merged, w_dict);
                strong.insert_value(key.clone(), Value::new(merged));
            }
            // Otherwise strong already has the key, keep it
        } else {
            // Only in weak - add to strong
            strong.insert_value(key.clone(), weak_val.clone());
        }
    }
}

/// Recursive compose into weak: strong values overwrite, nested dicts merge.
///
/// Matches C++ `VtDictionaryOverRecursive(const VtDictionary&, VtDictionary*)`.
pub fn dictionary_over_recursive_into_weak(strong: &Dictionary, weak: &mut Dictionary) {
    for (key, strong_val) in strong.iter() {
        if let Some(weak_val) = weak.get(key) {
            // Both have this key - try recursive compose for nested dicts
            if let (Some(s_dict), Some(w_dict)) =
                (strong_val.get::<Dictionary>(), weak_val.get::<Dictionary>())
            {
                let mut merged = w_dict.clone();
                dictionary_over_recursive_into_weak(s_dict, &mut merged);
                weak.insert_value(key.clone(), Value::new(merged));
            } else {
                // Not both dicts - strong replaces
                weak.insert_value(key.clone(), strong_val.clone());
            }
        } else {
            // Only in strong - add to weak
            weak.insert_value(key.clone(), strong_val.clone());
        }
    }
}

/// Static empty value returned by Index on missing key (matches C++ operator[]
/// which auto-inserts a default-constructed VtValue).
static EMPTY_VALUE: Value = Value::empty();

impl Index<&str> for Dictionary {
    type Output = Value;

    /// Returns a reference to the value for the given key, or a static empty
    /// `Value` if the key is not found. This matches C++ `VtDictionary::operator[]`
    /// which auto-inserts a default VtValue on missing keys.
    /// Use [`Dictionary::get`] for `Option`-based access.
    fn index(&self, key: &str) -> &Self::Output {
        self.data.get(key).unwrap_or(&EMPTY_VALUE)
    }
}

impl fmt::Debug for Dictionary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.data.iter()).finish()
    }
}

impl PartialEq for Dictionary {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
    }
}

impl Eq for Dictionary {}

impl Hash for Dictionary {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // BTreeMap iterates in sorted key order, so no manual sort needed
        for (key, value) in &self.data {
            key.hash(state);
            value.hash(state);
        }
    }
}

/// Formats as `{ 'key1': value1, 'key2': value2 }`.
///
/// Matches C++ `operator<<(std::ostream&, VtDictionary const&)`.
impl fmt::Display for Dictionary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{{")?;
        let mut first = true;
        for (key, value) in &self.data {
            if first {
                first = false;
            } else {
                write!(f, ", ")?;
            }
            write!(f, "'{key}': {value}")?;
        }
        write!(f, "}}")
    }
}

impl FromIterator<(String, Value)> for Dictionary {
    fn from_iter<I: IntoIterator<Item = (String, Value)>>(iter: I) -> Self {
        Self {
            data: iter.into_iter().collect(),
        }
    }
}

impl<'a> IntoIterator for &'a Dictionary {
    type Item = (&'a String, &'a Value);
    type IntoIter = btree_map::Iter<'a, String, Value>;

    fn into_iter(self) -> Self::IntoIter {
        self.data.iter()
    }
}

impl<'a> IntoIterator for &'a mut Dictionary {
    type Item = (&'a String, &'a mut Value);
    type IntoIter = btree_map::IterMut<'a, String, Value>;

    fn into_iter(self) -> Self::IntoIter {
        self.data.iter_mut()
    }
}

impl IntoIterator for Dictionary {
    type Item = (String, Value);
    type IntoIter = btree_map::IntoIter<String, Value>;

    fn into_iter(self) -> Self::IntoIter {
        self.data.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_empty() {
        let dict = Dictionary::new();
        assert!(dict.is_empty());
        assert_eq!(dict.len(), 0);
    }

    #[test]
    fn test_insert_and_get() {
        let mut dict = Dictionary::new();
        dict.insert("num", 42i32);
        dict.insert("text", "hello".to_string());

        assert_eq!(dict.get_as::<i32>("num"), Some(&42));
        assert_eq!(dict.get_as::<String>("text"), Some(&"hello".to_string()));
    }

    #[test]
    fn test_insert_floats() {
        let mut dict = Dictionary::new();
        dict.insert("f32", 3.14f32);
        dict.insert("f64", 2.71828f64);

        assert_eq!(dict.get_as::<f32>("f32"), Some(&3.14f32));
        assert_eq!(dict.get_as::<f64>("f64"), Some(&2.71828f64));
    }

    #[test]
    fn test_contains_key() {
        let mut dict = Dictionary::new();
        dict.insert("key", 42i32);

        assert!(dict.contains_key("key"));
        assert!(!dict.contains_key("missing"));
    }

    #[test]
    fn test_remove() {
        let mut dict = Dictionary::new();
        dict.insert("key", 42i32);

        assert!(dict.remove("key").is_some());
        assert!(dict.remove("key").is_none());
        assert!(!dict.contains_key("key"));
    }

    #[test]
    fn test_clear() {
        let mut dict = Dictionary::new();
        dict.insert("a", 1i32);
        dict.insert("b", 2i32);

        dict.clear();
        assert!(dict.is_empty());
    }

    #[test]
    fn test_merge() {
        let mut dict1 = Dictionary::new();
        dict1.insert("a", 1i32);

        let mut dict2 = Dictionary::new();
        dict2.insert("b", 2i32);
        dict2.insert("a", 10i32); // Override

        dict1.merge(dict2);

        assert_eq!(dict1.get_as::<i32>("a"), Some(&10)); // Overwritten
        assert_eq!(dict1.get_as::<i32>("b"), Some(&2));
    }

    #[test]
    fn test_iter() {
        let mut dict = Dictionary::new();
        dict.insert("a", 1i32);
        dict.insert("b", 2i32);

        let keys: Vec<_> = dict.keys().collect();
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn test_equality() {
        let mut dict1 = Dictionary::new();
        dict1.insert("a", 1i32);

        let mut dict2 = Dictionary::new();
        dict2.insert("a", 1i32);

        let mut dict3 = Dictionary::new();
        dict3.insert("a", 2i32);

        assert_eq!(dict1, dict2);
        assert_ne!(dict1, dict3);
    }

    // ====================================================================
    // DictionaryOver tests
    // ====================================================================

    #[test]
    fn test_dictionary_over_strong_wins() {
        let mut strong = Dictionary::new();
        strong.insert("a", 1i32);
        strong.insert("b", 2i32);

        let mut weak = Dictionary::new();
        weak.insert("b", 20i32);
        weak.insert("c", 30i32);

        let result = dictionary_over(&strong, &weak);

        assert_eq!(result.get_as::<i32>("a"), Some(&1));
        assert_eq!(result.get_as::<i32>("b"), Some(&2)); // strong wins
        assert_eq!(result.get_as::<i32>("c"), Some(&30)); // from weak
    }

    #[test]
    fn test_dictionary_over_in_place() {
        let mut strong = Dictionary::new();
        strong.insert("a", 1i32);

        let mut weak = Dictionary::new();
        weak.insert("a", 10i32);
        weak.insert("b", 20i32);

        dictionary_over_in_place(&mut strong, &weak);

        assert_eq!(strong.get_as::<i32>("a"), Some(&1)); // not overwritten
        assert_eq!(strong.get_as::<i32>("b"), Some(&20)); // added from weak
    }

    #[test]
    fn test_dictionary_over_into_weak() {
        let mut strong = Dictionary::new();
        strong.insert("a", 1i32);
        strong.insert("b", 2i32);

        let mut weak = Dictionary::new();
        weak.insert("b", 20i32);
        weak.insert("c", 30i32);

        dictionary_over_into_weak(&strong, &mut weak);

        assert_eq!(weak.get_as::<i32>("a"), Some(&1)); // from strong
        assert_eq!(weak.get_as::<i32>("b"), Some(&2)); // strong overwrites
        assert_eq!(weak.get_as::<i32>("c"), Some(&30)); // kept from weak
    }

    #[test]
    fn test_dictionary_over_recursive_nested() {
        let mut inner_s = Dictionary::new();
        inner_s.insert("x", 1i32);

        let mut inner_w = Dictionary::new();
        inner_w.insert("y", 2i32);

        let mut strong = Dictionary::new();
        strong.insert_value("nested", Value::new(inner_s));

        let mut weak = Dictionary::new();
        weak.insert_value("nested", Value::new(inner_w));
        weak.insert("flat", 99i32);

        let result = dictionary_over_recursive(&strong, &weak);

        // flat key from weak should be present
        assert_eq!(result.get_as::<i32>("flat"), Some(&99));

        // Nested dicts should be recursively merged
        let nested = result.get("nested").unwrap().get::<Dictionary>().unwrap();
        assert_eq!(nested.get_as::<i32>("x"), Some(&1));
        assert_eq!(nested.get_as::<i32>("y"), Some(&2));
    }

    #[test]
    fn test_dictionary_over_recursive_in_place() {
        let mut inner_s = Dictionary::new();
        inner_s.insert("x", 1i32);

        let mut inner_w = Dictionary::new();
        inner_w.insert("x", 10i32);
        inner_w.insert("y", 2i32);

        let mut strong = Dictionary::new();
        strong.insert_value("nested", Value::new(inner_s));

        let mut weak = Dictionary::new();
        weak.insert_value("nested", Value::new(inner_w));

        dictionary_over_recursive_in_place(&mut strong, &weak);

        let nested = strong.get("nested").unwrap().get::<Dictionary>().unwrap();
        assert_eq!(nested.get_as::<i32>("x"), Some(&1)); // strong wins
        assert_eq!(nested.get_as::<i32>("y"), Some(&2)); // from weak
    }

    #[test]
    fn test_dictionary_over_recursive_into_weak() {
        let mut inner_s = Dictionary::new();
        inner_s.insert("x", 1i32);

        let mut inner_w = Dictionary::new();
        inner_w.insert("x", 10i32);
        inner_w.insert("y", 2i32);

        let mut strong = Dictionary::new();
        strong.insert_value("nested", Value::new(inner_s));

        let mut weak = Dictionary::new();
        weak.insert_value("nested", Value::new(inner_w));
        weak.insert("only_weak", 99i32);

        dictionary_over_recursive_into_weak(&strong, &mut weak);

        assert_eq!(weak.get_as::<i32>("only_weak"), Some(&99));
        let nested = weak.get("nested").unwrap().get::<Dictionary>().unwrap();
        assert_eq!(nested.get_as::<i32>("x"), Some(&1)); // strong wins
        assert_eq!(nested.get_as::<i32>("y"), Some(&2)); // from weak
    }

    #[test]
    fn test_dictionary_over_empty() {
        let strong = Dictionary::new();
        let mut weak = Dictionary::new();
        weak.insert("a", 1i32);

        let result = dictionary_over(&strong, &weak);
        assert_eq!(result.get_as::<i32>("a"), Some(&1));

        let result2 = dictionary_over(&weak, &strong);
        assert_eq!(result2.get_as::<i32>("a"), Some(&1));
    }

    // =====================================================================
    // H-vt-4: BTreeMap sorted iteration test
    // =====================================================================

    #[test]
    fn test_keys_sorted_order() {
        // BTreeMap guarantees sorted key iteration (matches C++ std::map)
        let mut dict = Dictionary::new();
        dict.insert("zebra", 1i32);
        dict.insert("alpha", 2i32);
        dict.insert("mango", 3i32);
        dict.insert("banana", 4i32);

        let keys: Vec<&String> = dict.keys().collect();
        assert_eq!(
            keys.iter().map(|k| k.as_str()).collect::<Vec<_>>(),
            vec!["alpha", "banana", "mango", "zebra"]
        );
    }

    #[test]
    fn test_iter_sorted_order() {
        let mut dict = Dictionary::new();
        dict.insert("c", 3i32);
        dict.insert("a", 1i32);
        dict.insert("b", 2i32);

        let pairs: Vec<(&String, &Value)> = dict.iter().collect();
        assert_eq!(pairs[0].0.as_str(), "a");
        assert_eq!(pairs[1].0.as_str(), "b");
        assert_eq!(pairs[2].0.as_str(), "c");
    }

    // =====================================================================
    // M1: get_or_insert_default (C++ operator[] auto-insert parity)
    // =====================================================================

    #[test]
    fn test_get_or_insert_default_new_key() {
        let mut dict = Dictionary::new();
        let val = dict.get_or_insert_default("new_key");
        assert!(val.is_empty());
        assert_eq!(dict.len(), 1);
        assert!(dict.contains_key("new_key"));
    }

    #[test]
    fn test_get_or_insert_default_existing_key() {
        let mut dict = Dictionary::new();
        dict.insert("key", 42i32);
        let val = dict.get_or_insert_default("key");
        // Existing value should be returned, not overwritten
        assert_eq!(val.get::<i32>(), Some(&42));
        assert_eq!(dict.len(), 1);
    }

    #[test]
    fn test_get_or_insert_default_then_assign() {
        let mut dict = Dictionary::new();
        *dict.get_or_insert_default("key") = Value::from(99i32);
        assert_eq!(dict.get_as::<i32>("key"), Some(&99));
    }

    // =========================================================================
    // M-vt: Index trait returns empty Value for missing key (not panic)
    // =========================================================================

    #[test]
    fn test_index_missing_key_returns_empty() {
        let dict = Dictionary::new();
        // Should NOT panic; returns empty Value (matching C++ operator[])
        let val = &dict["nonexistent"];
        assert!(val.is_empty());
    }

    #[test]
    fn test_index_existing_key_returns_value() {
        let mut dict = Dictionary::new();
        dict.insert("key", 42i32);
        let val = &dict["key"];
        assert_eq!(val.get::<i32>(), Some(&42));
    }

    #[test]
    fn test_index_empty_dict() {
        let dict = Dictionary::new();
        let val = &dict["anything"];
        assert!(val.is_empty());
        assert_eq!(dict.len(), 0); // dict not mutated
    }

    // =====================================================================
    // New method tests
    // =====================================================================

    #[test]
    fn test_count() {
        let mut dict = Dictionary::new();
        dict.insert("a", 1i32);
        assert_eq!(dict.count("a"), 1);
        assert_eq!(dict.count("b"), 0);
    }

    #[test]
    fn test_swap() {
        let mut d1 = Dictionary::new();
        d1.insert("a", 1i32);

        let mut d2 = Dictionary::new();
        d2.insert("b", 2i32);

        d1.swap(&mut d2);
        assert!(d1.contains_key("b"));
        assert!(!d1.contains_key("a"));
        assert!(d2.contains_key("a"));
    }

    #[test]
    fn test_entry_api() {
        let mut dict = Dictionary::new();
        dict.entry("key".to_string())
            .or_insert_with(|| Value::from(42i32));
        assert_eq!(dict.get_as::<i32>("key"), Some(&42));

        // Entry on existing key should not overwrite
        dict.entry("key".to_string())
            .or_insert_with(|| Value::from(99i32));
        assert_eq!(dict.get_as::<i32>("key"), Some(&42));
    }

    #[test]
    fn test_values_mut() {
        let mut dict = Dictionary::new();
        dict.insert("a", 1i32);
        dict.insert("b", 2i32);

        for v in dict.values_mut() {
            *v = Value::from(0i32);
        }
        assert_eq!(dict.get_as::<i32>("a"), Some(&0));
        assert_eq!(dict.get_as::<i32>("b"), Some(&0));
    }

    #[test]
    fn test_iter_mut() {
        let mut dict = Dictionary::new();
        dict.insert("x", 10i32);

        for (_k, v) in dict.iter_mut() {
            *v = Value::from(20i32);
        }
        assert_eq!(dict.get_as::<i32>("x"), Some(&20));
    }

    #[test]
    fn test_retain() {
        let mut dict = Dictionary::new();
        dict.insert("keep", 1i32);
        dict.insert("drop", 2i32);
        dict.insert("keep2", 3i32);

        dict.retain(|k, _| k.starts_with("keep"));
        assert_eq!(dict.len(), 2);
        assert!(dict.contains_key("keep"));
        assert!(dict.contains_key("keep2"));
        assert!(!dict.contains_key("drop"));
    }

    #[test]
    fn test_extend_from() {
        let mut dict = Dictionary::new();
        dict.extend_from(vec![
            ("a".to_string(), Value::from(1i32)),
            ("b".to_string(), Value::from(2i32)),
        ]);
        assert_eq!(dict.len(), 2);
        assert_eq!(dict.get_as::<i32>("a"), Some(&1));
    }

    #[test]
    fn test_get_or_default() {
        let mut dict = Dictionary::new();
        dict.insert("x", 42i32);

        assert_eq!(dict.get_or::<i32>("x", 0), 42);
        assert_eq!(dict.get_or::<i32>("missing", 99), 99);
        // Wrong type returns default
        assert_eq!(dict.get_or::<f64>("x", 1.0), 1.0);
    }

    #[test]
    fn test_display() {
        let dict = Dictionary::new();
        assert_eq!(format!("{}", dict), "{}");

        let mut dict = Dictionary::new();
        dict.insert("key", 42i32);
        let s = format!("{}", dict);
        assert!(s.contains("'key'"));
        assert!(s.starts_with('{'));
        assert!(s.ends_with('}'));
    }

    #[test]
    fn test_from_iter() {
        let dict: Dictionary = vec![
            ("a".to_string(), Value::from(1i32)),
            ("b".to_string(), Value::from(2i32)),
        ]
        .into_iter()
        .collect();

        assert_eq!(dict.len(), 2);
        assert_eq!(dict.get_as::<i32>("a"), Some(&1));
    }

    #[test]
    fn test_into_iter_owned() {
        let mut dict = Dictionary::new();
        dict.insert("a", 1i32);
        dict.insert("b", 2i32);

        let pairs: Vec<(String, Value)> = dict.into_iter().collect();
        assert_eq!(pairs.len(), 2);
    }

    #[test]
    fn test_into_iter_ref() {
        let mut dict = Dictionary::new();
        dict.insert("a", 1i32);

        // &Dictionary should work in for loops
        let mut count = 0;
        for (_k, _v) in &dict {
            count += 1;
        }
        assert_eq!(count, 1);
    }

    #[test]
    fn test_into_iter_mut() {
        let mut dict = Dictionary::new();
        dict.insert("a", 1i32);

        for (_k, v) in &mut dict {
            *v = Value::from(99i32);
        }
        assert_eq!(dict.get_as::<i32>("a"), Some(&99));
    }
}
