//! Dense hash set - space efficient for small collections.
//!
//! This container uses a vector for storage when small, switching to a
//! hash set when the size exceeds a threshold. This provides cache-efficient
//! iteration for small collections while maintaining O(1) lookup for larger ones.
//!
//! # Examples
//!
//! ```
//! use usd_tf::dense_hashset::DenseHashSet;
//!
//! let mut set: DenseHashSet<String> = DenseHashSet::new();
//! set.insert("one".to_string());
//! set.insert("two".to_string());
//!
//! assert!(set.contains("one"));
//! ```

use std::borrow::Borrow;
use std::collections::HashSet;
use std::hash::Hash;

/// Default threshold for switching from vector to hashset storage.
const DEFAULT_THRESHOLD: usize = 128;

/// A space-efficient set that uses vector storage for small sizes.
///
/// When the set has fewer than `THRESHOLD` elements, it uses a `Vec`
/// for storage (O(n) lookup but cache-efficient). Above the threshold,
/// it creates a `HashSet` for O(1) lookups.
///
/// # Type Parameters
///
/// * `T` - Element type
/// * `THRESHOLD` - Size at which to switch to hashset (default 128)
///
/// # Examples
///
/// ```
/// use usd_tf::dense_hashset::DenseHashSet;
///
/// let mut set = DenseHashSet::<i32>::new();
/// set.insert(1);
/// set.insert(2);
///
/// assert_eq!(set.len(), 2);
/// assert!(set.contains(&1));
/// ```
pub struct DenseHashSet<T, const THRESHOLD: usize = DEFAULT_THRESHOLD> {
    /// Vector storage for elements (always used).
    vec: Vec<T>,
    /// HashSet for fast lookup (only allocated when size > threshold).
    index: Option<HashSet<T>>,
}

impl<T> DenseHashSet<T, DEFAULT_THRESHOLD>
where
    T: Hash + Eq + Clone,
{
    /// Creates a new empty dense hash set.
    #[inline]
    pub fn new() -> Self {
        Self {
            vec: Vec::new(),
            index: None,
        }
    }

    /// Creates a dense hash set with the specified capacity.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            vec: Vec::with_capacity(capacity),
            index: None,
        }
    }
}

impl<T, const THRESHOLD: usize> DenseHashSet<T, THRESHOLD>
where
    T: Hash + Eq + Clone,
{
    /// Creates a new empty dense hash set with custom threshold.
    #[inline]
    pub fn new_with_threshold() -> Self {
        Self {
            vec: Vec::new(),
            index: None,
        }
    }

    /// Returns the number of elements in the set.
    #[inline]
    pub fn len(&self) -> usize {
        self.vec.len()
    }

    /// Returns true if the set is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.vec.is_empty()
    }

    /// Clears the set.
    #[inline]
    pub fn clear(&mut self) {
        self.vec.clear();
        self.index = None;
    }

    /// Inserts an element into the set.
    ///
    /// Returns `false` if the element was already present.
    pub fn insert(&mut self, value: T) -> bool {
        // Check if value exists
        if self.contains(&value) {
            return false;
        }

        // Insert new element
        self.vec.push(value.clone());

        // Update or create index if above threshold
        if self.vec.len() >= THRESHOLD {
            if let Some(ref mut index) = self.index {
                index.insert(value);
            } else {
                self.rebuild_index();
            }
        }

        true
    }

    /// Returns true if the set contains the given value.
    #[inline]
    pub fn contains<Q>(&self, value: &Q) -> bool
    where
        T: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        if let Some(ref index) = self.index {
            index.contains(value)
        } else {
            self.vec.iter().any(|v| v.borrow() == value)
        }
    }

    /// Removes a value from the set.
    ///
    /// Returns `true` if the value was present.
    pub fn remove<Q>(&mut self, value: &Q) -> bool
    where
        T: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let idx = self.vec.iter().position(|v| v.borrow() == value);
        if let Some(idx) = idx {
            let removed = self.vec.swap_remove(idx);

            // Remove from index using the actual removed value (turbofish avoids Q shadowing)
            if let Some(ref mut index) = self.index {
                index.remove::<T>(&removed);
            }

            true
        } else {
            false
        }
    }

    /// Returns an iterator over the elements.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.vec.iter()
    }

    /// Rebuilds the index from the vector.
    fn rebuild_index(&mut self) {
        let mut index = HashSet::with_capacity(self.vec.len());
        for v in &self.vec {
            index.insert(v.clone());
        }
        self.index = Some(index);
    }

    /// Returns the union of two sets.
    pub fn union<'a>(&'a self, other: &'a Self) -> impl Iterator<Item = &'a T> {
        self.iter()
            .chain(other.iter().filter(|v| !self.contains(*v)))
    }

    /// Returns the intersection of two sets.
    pub fn intersection<'a>(&'a self, other: &'a Self) -> impl Iterator<Item = &'a T> {
        self.iter().filter(|v| other.contains(*v))
    }

    /// Returns the difference (self - other).
    pub fn difference<'a>(&'a self, other: &'a Self) -> impl Iterator<Item = &'a T> {
        self.iter().filter(|v| !other.contains(*v))
    }
}

impl<T, const THRESHOLD: usize> Default for DenseHashSet<T, THRESHOLD>
where
    T: Hash + Eq + Clone,
{
    fn default() -> Self {
        Self {
            vec: Vec::new(),
            index: None,
        }
    }
}

impl<T, const THRESHOLD: usize> FromIterator<T> for DenseHashSet<T, THRESHOLD>
where
    T: Hash + Eq + Clone,
{
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut set = Self::default();
        for v in iter {
            set.insert(v);
        }
        set
    }
}

impl<T, const THRESHOLD: usize> IntoIterator for DenseHashSet<T, THRESHOLD> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.vec.into_iter()
    }
}

impl<'a, T, const THRESHOLD: usize> IntoIterator for &'a DenseHashSet<T, THRESHOLD>
where
    T: Hash + Eq + Clone,
{
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.vec.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_operations() {
        let mut set: DenseHashSet<String> = DenseHashSet::new();

        // Insert
        assert!(set.insert("one".to_string()));
        assert!(set.insert("two".to_string()));
        assert!(set.insert("three".to_string()));

        assert_eq!(set.len(), 3);

        // Duplicate insert
        assert!(!set.insert("one".to_string()));
        assert_eq!(set.len(), 3);

        // Contains
        assert!(set.contains("one"));
        assert!(set.contains("two"));
        assert!(!set.contains("four"));
    }

    #[test]
    fn test_remove() {
        let mut set: DenseHashSet<i32> = DenseHashSet::new();
        set.insert(1);
        set.insert(2);
        set.insert(3);

        assert!(set.remove(&2));
        assert_eq!(set.len(), 2);
        assert!(!set.contains(&2));

        assert!(!set.remove(&5));
    }

    #[test]
    fn test_clear() {
        let mut set: DenseHashSet<i32> = DenseHashSet::new();
        set.insert(1);
        set.insert(2);

        set.clear();
        assert!(set.is_empty());
        assert_eq!(set.len(), 0);
    }

    #[test]
    fn test_iteration() {
        let mut set: DenseHashSet<i32> = DenseHashSet::new();
        set.insert(1);
        set.insert(2);
        set.insert(3);

        let sum: i32 = set.iter().sum();
        assert_eq!(sum, 6);
    }

    #[test]
    fn test_threshold_switch() {
        // Use a small threshold for testing
        let mut set: DenseHashSet<i32, 4> = DenseHashSet::new_with_threshold();

        // Below threshold - no index
        for i in 0..4 {
            set.insert(i);
        }

        // Above threshold - index created
        set.insert(4);
        set.insert(5);

        // Lookups should still work
        assert!(set.contains(&0));
        assert!(set.contains(&5));
        assert!(!set.contains(&10));
    }

    #[test]
    fn test_from_iterator() {
        let values = vec![1, 2, 3, 4, 5];
        let set: DenseHashSet<i32> = values.into_iter().collect();

        assert_eq!(set.len(), 5);
        assert!(set.contains(&3));
    }

    #[test]
    fn test_into_iterator() {
        let mut set: DenseHashSet<i32> = DenseHashSet::new();
        set.insert(1);
        set.insert(2);

        let vec: Vec<_> = set.into_iter().collect();
        assert_eq!(vec.len(), 2);
    }

    #[test]
    fn test_set_operations() {
        let mut a: DenseHashSet<i32> = DenseHashSet::new();
        a.insert(1);
        a.insert(2);
        a.insert(3);

        let mut b: DenseHashSet<i32> = DenseHashSet::new();
        b.insert(2);
        b.insert(3);
        b.insert(4);

        // Intersection
        let intersection: Vec<_> = a.intersection(&b).copied().collect();
        assert_eq!(intersection.len(), 2);
        assert!(intersection.contains(&2));
        assert!(intersection.contains(&3));

        // Difference
        let diff: Vec<_> = a.difference(&b).copied().collect();
        assert_eq!(diff.len(), 1);
        assert!(diff.contains(&1));
    }

    #[test]
    fn test_with_capacity() {
        let set: DenseHashSet<i32> = DenseHashSet::with_capacity(100);
        assert!(set.is_empty());
    }
}
