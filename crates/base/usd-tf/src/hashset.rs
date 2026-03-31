//! Hash set type alias for USD compatibility.
//!
//! In USD C++, `TfHashSet` wraps `std::unordered_set` or GNU `hash_set`.
//! In Rust, we simply use `std::collections::HashSet`.
//!
//! # Examples
//!
//! ```
//! use usd_tf::hashset::TfHashSet;
//!
//! let mut set: TfHashSet<String> = TfHashSet::new();
//! set.insert("one".to_string());
//! set.insert("two".to_string());
//!
//! assert!(set.contains("one"));
//! ```

use std::collections::HashSet;
use std::hash::{BuildHasher, Hash};

/// Hash set type matching USD's TfHashSet.
///
/// This is a type alias for `std::collections::HashSet` using the default hasher.
/// For custom hashers, use `TfHashSetWith`.
///
/// # Examples
///
/// ```
/// use usd_tf::hashset::TfHashSet;
///
/// let mut set = TfHashSet::new();
/// set.insert("value");
/// assert!(set.contains("value"));
/// ```
pub type TfHashSet<T> = HashSet<T>;

/// Hash set with custom hasher.
///
/// # Type Parameters
///
/// * `T` - Element type (must implement `Hash` and `Eq`)
/// * `S` - Hasher builder type
pub type TfHashSetWith<T, S> = HashSet<T, S>;

/// Creates a new empty hash set.
///
/// # Examples
///
/// ```
/// use usd_tf::hashset::new_hashset;
///
/// let set: std::collections::HashSet<i32> = new_hashset();
/// assert!(set.is_empty());
/// ```
#[inline]
pub fn new_hashset<T>() -> TfHashSet<T>
where
    T: Hash + Eq,
{
    HashSet::new()
}

/// Creates a hash set with the specified capacity.
///
/// # Examples
///
/// ```
/// use usd_tf::hashset::with_capacity;
///
/// let set: std::collections::HashSet<i32> = with_capacity(100);
/// assert!(set.capacity() >= 100);
/// ```
#[inline]
pub fn with_capacity<T>(capacity: usize) -> TfHashSet<T>
where
    T: Hash + Eq,
{
    HashSet::with_capacity(capacity)
}

/// Creates a hash set with a custom hasher.
///
/// # Examples
///
/// ```
/// use usd_tf::hashset::with_hasher;
/// use std::collections::hash_map::RandomState;
///
/// let set: std::collections::HashSet<i32, _> = with_hasher(RandomState::new());
/// assert!(set.is_empty());
/// ```
#[inline]
pub fn with_hasher<T, S>(hash_builder: S) -> TfHashSetWith<T, S>
where
    T: Hash + Eq,
    S: BuildHasher,
{
    HashSet::with_hasher(hash_builder)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tfhashset_basic() {
        let mut set: TfHashSet<String> = TfHashSet::new();
        set.insert("one".to_string());
        set.insert("two".to_string());
        set.insert("three".to_string());

        assert_eq!(set.len(), 3);
        assert!(set.contains("one"));
        assert!(set.contains("two"));
        assert!(set.contains("three"));
        assert!(!set.contains("four"));
    }

    #[test]
    fn test_new_hashset() {
        let set: TfHashSet<i32> = new_hashset();
        assert!(set.is_empty());
    }

    #[test]
    fn test_with_capacity() {
        let set: TfHashSet<i32> = with_capacity(100);
        assert!(set.capacity() >= 100);
    }

    #[test]
    fn test_hashset_operations() {
        let mut set: TfHashSet<i32> = new_hashset();

        // Insert
        set.insert(1);
        set.insert(2);
        set.insert(3);

        // Contains
        assert!(set.contains(&1));
        assert!(!set.contains(&4));

        // Remove
        assert!(set.remove(&1));
        assert!(!set.contains(&1));

        // Duplicate insert
        assert!(!set.insert(2)); // Already present
    }

    #[test]
    fn test_hashset_set_operations() {
        let mut a: TfHashSet<i32> = new_hashset();
        a.insert(1);
        a.insert(2);
        a.insert(3);

        let mut b: TfHashSet<i32> = new_hashset();
        b.insert(2);
        b.insert(3);
        b.insert(4);

        // Union
        let union: TfHashSet<_> = a.union(&b).copied().collect();
        assert_eq!(union.len(), 4);

        // Intersection
        let intersection: TfHashSet<_> = a.intersection(&b).copied().collect();
        assert_eq!(intersection.len(), 2);
        assert!(intersection.contains(&2));
        assert!(intersection.contains(&3));

        // Difference
        let diff: TfHashSet<_> = a.difference(&b).copied().collect();
        assert_eq!(diff.len(), 1);
        assert!(diff.contains(&1));
    }

    #[test]
    fn test_hashset_iteration() {
        let mut set: TfHashSet<i32> = new_hashset();
        set.insert(1);
        set.insert(2);
        set.insert(3);

        let sum: i32 = set.iter().sum();
        assert_eq!(sum, 6);
    }
}
