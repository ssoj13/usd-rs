//! Display formatting for standard containers.
//!
//! This module provides Display implementations for various standard library
//! containers, making them easy to output for debugging and diagnostics.
//!
//! These are not meant for serialization but rather for human-readable output.
//! The formats may change without notice.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::{self, Display};
use std::hash::Hash;

/// Wrapper type to enable custom Display formatting for Vec.
///
/// Displays vectors using `[ ]` as delimiters.
/// Example: `[ 1 2 3 ]`
pub struct DisplayVec<'a, T>(pub &'a [T]);

impl<'a, T: Display> Display for DisplayVec<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[ ")?;
        for item in self.0 {
            write!(f, "{} ", item)?;
        }
        write!(f, "]")
    }
}

/// Wrapper type to enable custom Display formatting for HashSet.
///
/// Displays sets using `( )` as delimiters.
/// Example: `( 1 2 3 )`
pub struct DisplaySet<'a, T>(pub &'a HashSet<T>);

impl<'a, T: Display> Display for DisplaySet<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "( ")?;
        for item in self.0 {
            write!(f, "{} ", item)?;
        }
        write!(f, ")")
    }
}

/// Wrapper type to enable custom Display formatting for BTreeSet.
///
/// Displays sets using `( )` as delimiters.
/// Example: `( 1 2 3 )`
pub struct DisplayBTreeSet<'a, T>(pub &'a BTreeSet<T>);

impl<'a, T: Display> Display for DisplayBTreeSet<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "( ")?;
        for item in self.0 {
            write!(f, "{} ", item)?;
        }
        write!(f, ")")
    }
}

/// Wrapper type to enable custom Display formatting for HashMap.
///
/// Displays maps using `< >` as outer delimiters and `<key: value>` for entries.
/// Example: `< <a: 1> <b: 2> >`
pub struct DisplayHashMap<'a, K, V>(pub &'a HashMap<K, V>);

impl<'a, K: Display, V: Display> Display for DisplayHashMap<'a, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "< ")?;
        for (key, value) in self.0 {
            write!(f, "<{}: {}> ", key, value)?;
        }
        write!(f, ">")
    }
}

/// Wrapper type to enable custom Display formatting for BTreeMap.
///
/// Displays maps using `< >` as outer delimiters and `<key: value>` for entries.
/// Example: `< <a: 1> <b: 2> >`
pub struct DisplayBTreeMap<'a, K, V>(pub &'a BTreeMap<K, V>);

impl<'a, K: Display, V: Display> Display for DisplayBTreeMap<'a, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "< ")?;
        for (key, value) in self.0 {
            write!(f, "<{}: {}> ", key, value)?;
        }
        write!(f, ">")
    }
}

/// Helper trait to check if a type is ostreamable (has Display).
///
/// This is automatically implemented for all types that implement Display.
pub trait TfOstreamable: Display {}
impl<T: Display> TfOstreamable for T {}

/// Creates a displayable wrapper for a vector slice.
///
/// # Examples
///
/// ```
/// use usd_tf::ostream_methods::tf_display_vec;
///
/// let v = vec![1, 2, 3];
/// println!("{}", tf_display_vec(&v));
/// // Output: [ 1 2 3 ]
/// ```
#[must_use]
pub fn tf_display_vec<T: Display>(v: &[T]) -> DisplayVec<'_, T> {
    DisplayVec(v)
}

/// Creates a displayable wrapper for a HashSet.
///
/// # Examples
///
/// ```
/// use usd_tf::ostream_methods::tf_display_set;
/// use std::collections::HashSet;
///
/// let mut s = HashSet::new();
/// s.insert(1);
/// s.insert(2);
/// println!("{}", tf_display_set(&s));
/// // Output: ( 1 2 ) or ( 2 1 ) (unordered)
/// ```
#[must_use]
pub fn tf_display_set<T: Display + Eq + Hash>(s: &HashSet<T>) -> DisplaySet<'_, T> {
    DisplaySet(s)
}

/// Creates a displayable wrapper for a BTreeSet.
#[must_use]
pub fn tf_display_btree_set<T: Display>(s: &BTreeSet<T>) -> DisplayBTreeSet<'_, T> {
    DisplayBTreeSet(s)
}

/// Creates a displayable wrapper for a HashMap.
///
/// # Examples
///
/// ```
/// use usd_tf::ostream_methods::tf_display_map;
/// use std::collections::HashMap;
///
/// let mut m = HashMap::new();
/// m.insert("a", 1);
/// m.insert("b", 2);
/// println!("{}", tf_display_map(&m));
/// // Output: < <a: 1> <b: 2> > or < <b: 2> <a: 1> > (unordered)
/// ```
#[must_use]
pub fn tf_display_map<K: Display + Eq + Hash, V: Display>(
    m: &HashMap<K, V>,
) -> DisplayHashMap<'_, K, V> {
    DisplayHashMap(m)
}

/// Creates a displayable wrapper for a BTreeMap.
#[must_use]
pub fn tf_display_btree_map<K: Display, V: Display>(
    m: &BTreeMap<K, V>,
) -> DisplayBTreeMap<'_, K, V> {
    DisplayBTreeMap(m)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_vec() {
        let v = vec![1, 2, 3];
        let s = format!("{}", tf_display_vec(&v));
        assert_eq!(s, "[ 1 2 3 ]");
    }

    #[test]
    fn test_display_vec_empty() {
        let v: Vec<i32> = vec![];
        let s = format!("{}", tf_display_vec(&v));
        assert_eq!(s, "[ ]");
    }

    #[test]
    fn test_display_vec_strings() {
        let v = vec!["hello", "world"];
        let s = format!("{}", tf_display_vec(&v));
        assert_eq!(s, "[ hello world ]");
    }

    #[test]
    fn test_display_btree_set() {
        let mut s = BTreeSet::new();
        s.insert(1);
        s.insert(2);
        s.insert(3);
        let output = format!("{}", tf_display_btree_set(&s));
        assert_eq!(output, "( 1 2 3 )");
    }

    #[test]
    fn test_display_btree_map() {
        let mut m = BTreeMap::new();
        m.insert("a", 1);
        m.insert("b", 2);
        let output = format!("{}", tf_display_btree_map(&m));
        assert_eq!(output, "< <a: 1> <b: 2> >");
    }

    #[test]
    fn test_display_hash_set() {
        let mut s = HashSet::new();
        s.insert(42);
        let output = format!("{}", tf_display_set(&s));
        assert!(output.contains("42"));
        assert!(output.starts_with("( "));
        assert!(output.ends_with(")"));
    }

    #[test]
    fn test_display_hash_map() {
        let mut m = HashMap::new();
        m.insert("key", "value");
        let output = format!("{}", tf_display_map(&m));
        assert!(output.contains("key"));
        assert!(output.contains("value"));
        assert!(output.starts_with("< "));
        assert!(output.ends_with(">"));
    }
}
