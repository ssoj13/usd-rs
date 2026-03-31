//! SdfPathTable - A mapping from SdfPath to a value type.
//!
//! Port of pxr/usd/sdf/pathTable.h
//!
//! A mapping from SdfPath to MappedType, somewhat similar to
//! HashMap<Path, MappedType>, but with key differences:
//!
//! - Works exclusively with absolute paths.
//! - Inserting a path also implicitly inserts all of its ancestors.
//! - Erasing a path also implicitly erases all of its descendants.
//! - Provides a preorder iteration of paths in the table.
//! - FindSubtreeRange returns an iterator range over a subtree.

use crate::Path;
use std::collections::BTreeMap;

/// A mapping from SdfPath to a value type with hierarchy-aware operations.
///
/// Inserting a path implicitly inserts all ancestors.
/// Erasing a path implicitly erases all descendants.
/// The table maintains a preorder traversal of paths.
#[derive(Debug, Clone)]
pub struct PathTable<V: Default> {
    /// The underlying storage, keyed by path string for ordering.
    entries: BTreeMap<String, PathTableEntry<V>>,
}

/// An entry in the path table.
#[derive(Debug, Clone)]
pub struct PathTableEntry<V: Default> {
    /// The path key.
    pub path: Path,
    /// The stored value.
    pub value: V,
}

/// An iterator over a subtree of the path table.
pub struct SubtreeRange<'a, V: Default> {
    prefix: String,
    iter: std::collections::btree_map::Range<'a, String, PathTableEntry<V>>,
}

impl<'a, V: Default> Iterator for SubtreeRange<'a, V> {
    type Item = (&'a Path, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (key, entry) = self.iter.next()?;
            if key.starts_with(&self.prefix) {
                return Some((&entry.path, &entry.value));
            }
            // BTreeMap is ordered, so once we pass the prefix range we're done.
            return None;
        }
    }
}

/// A mutable iterator over a subtree of the path table.
pub struct SubtreeRangeMut<'a, V: Default> {
    prefix: String,
    iter: std::collections::btree_map::RangeMut<'a, String, PathTableEntry<V>>,
}

impl<'a, V: Default> Iterator for SubtreeRangeMut<'a, V> {
    type Item = (&'a Path, &'a mut V);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (key, entry) = self.iter.next()?;
            if key.starts_with(&self.prefix) {
                return Some((&entry.path, &mut entry.value));
            }
            return None;
        }
    }
}

impl<V: Default> Default for PathTable<V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<V: Default> PathTable<V> {
    /// Creates an empty path table.
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    /// Returns true if the table contains no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the number of entries in the table.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Clears all entries from the table.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Inserts a path with a value, also inserting all ancestor paths
    /// with default values if not already present.
    ///
    /// Returns a mutable reference to the inserted (or existing) value.
    pub fn insert(&mut self, path: Path, value: V) -> &mut V {
        // Insert all ancestor paths with default values.
        let mut current = path.get_parent_path();
        while !current.is_empty() && current != Path::absolute_root() {
            let key = current.get_string().to_string();
            self.entries.entry(key).or_insert_with(|| PathTableEntry {
                path: current.clone(),
                value: V::default(),
            });
            current = current.get_parent_path();
        }

        // Insert the absolute root if we have any ancestor.
        if path != Path::absolute_root() && !path.is_empty() {
            let root = Path::absolute_root();
            let root_key = root.get_string().to_string();
            self.entries.entry(root_key).or_insert_with(|| PathTableEntry {
                path: root,
                value: V::default(),
            });
        }

        // Insert the actual entry.
        let key = path.get_string().to_string();
        let entry = self.entries.entry(key).or_insert_with(|| PathTableEntry {
            path: path.clone(),
            value: V::default(),
        });
        entry.value = value;
        &mut entry.value
    }

    /// Finds the value associated with the given path.
    pub fn find(&self, path: &Path) -> Option<&V> {
        let key = path.get_string();
        self.entries.get(key).map(|e| &e.value)
    }

    /// Finds the value associated with the given path (mutable).
    pub fn find_mut(&mut self, path: &Path) -> Option<&mut V> {
        let key = path.get_string().to_string();
        self.entries.get_mut(&key).map(|e| &mut e.value)
    }

    /// Returns true if the table contains the given path.
    pub fn contains(&self, path: &Path) -> bool {
        self.entries.contains_key(path.get_string())
    }

    /// Erases a path and all its descendants from the table.
    ///
    /// Returns the number of entries removed.
    pub fn erase(&mut self, path: &Path) -> usize {
        let prefix = path.get_string().to_string();
        // Collect keys to remove (can't remove while iterating).
        let to_remove: Vec<String> = self
            .entries
            .range(prefix.clone()..)
            .take_while(|(k, _)| k.starts_with(&prefix))
            .map(|(k, _)| k.clone())
            .collect();

        let count = to_remove.len();
        for key in to_remove {
            self.entries.remove(&key);
        }
        count
    }

    /// Returns an iterator range over all paths with the given prefix,
    /// including the prefix path itself if it exists in the table.
    pub fn find_subtree_range(&self, path: &Path) -> SubtreeRange<'_, V> {
        let prefix = path.get_string().to_string();
        SubtreeRange {
            iter: self.entries.range(prefix.clone()..),
            prefix,
        }
    }

    /// Returns a mutable iterator range over all paths with the given prefix.
    pub fn find_subtree_range_mut(&mut self, path: &Path) -> SubtreeRangeMut<'_, V> {
        let prefix = path.get_string().to_string();
        SubtreeRangeMut {
            iter: self.entries.range_mut(prefix.clone()..),
            prefix,
        }
    }

    /// Iterates over all entries in preorder.
    pub fn iter(&self) -> impl Iterator<Item = (&Path, &V)> {
        self.entries.values().map(|e| (&e.path, &e.value))
    }

    /// Iterates over all entries in preorder (mutable values).
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&Path, &mut V)> {
        self.entries.values_mut().map(|e| (&e.path, &mut e.value))
    }

    /// Swaps the contents of two path tables.
    pub fn swap(&mut self, other: &mut Self) {
        std::mem::swap(&mut self.entries, &mut other.entries);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_find() {
        let mut table: PathTable<i32> = PathTable::new();
        let path = Path::from_string("/World/Cube").unwrap();
        table.insert(path.clone(), 42);
        assert_eq!(table.find(&path), Some(&42));
    }

    #[test]
    fn test_ancestor_insertion() {
        let mut table: PathTable<i32> = PathTable::new();
        let path = Path::from_string("/World/Cube").unwrap();
        table.insert(path, 42);

        // Ancestors should have been inserted with default values.
        let world = Path::from_string("/World").unwrap();
        assert!(table.contains(&world));
        assert_eq!(table.find(&world), Some(&0));
    }

    #[test]
    fn test_erase_subtree() {
        let mut table: PathTable<i32> = PathTable::new();
        let a = Path::from_string("/A").unwrap();
        let ab = Path::from_string("/A/B").unwrap();
        let abc = Path::from_string("/A/B/C").unwrap();
        let d = Path::from_string("/D").unwrap();

        table.insert(a.clone(), 1);
        table.insert(ab.clone(), 2);
        table.insert(abc.clone(), 3);
        table.insert(d.clone(), 4);

        // Erase /A and all descendants.
        let removed = table.erase(&a);
        assert!(removed >= 3); // /A, /A/B, /A/B/C
        assert!(!table.contains(&a));
        assert!(!table.contains(&ab));
        assert!(!table.contains(&abc));
        assert!(table.contains(&d));
    }
}
