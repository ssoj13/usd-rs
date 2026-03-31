//! STL-like utilities for container operations.
//!
//! Port of pxr/base/tf/stl.h
//!
//! Provides utility functions for working with maps, sets, and other containers.

use std::borrow::Borrow;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::hash::Hash;

// --- Map lookups ---

/// Look up a key in a map, returning an Option reference.
#[inline]
pub fn map_lookup<'a, K, V, Q>(map: &'a HashMap<K, V>, key: &Q) -> Option<&'a V>
where
    K: Eq + Hash + Borrow<Q>,
    Q: Eq + Hash + ?Sized,
{
    map.get(key)
}

/// Look up a key in a map, returning a mutable reference.
#[inline]
pub fn map_lookup_mut<'a, K, V, Q>(map: &'a mut HashMap<K, V>, key: &Q) -> Option<&'a mut V>
where
    K: Eq + Hash + Borrow<Q>,
    Q: Eq + Hash + ?Sized,
{
    map.get_mut(key)
}

/// Alias for `map_lookup_mut`, matching USD C++ API naming.
#[inline]
pub fn map_lookup_ptr_mut<'a, K, V, Q>(map: &'a mut HashMap<K, V>, key: &Q) -> Option<&'a mut V>
where
    K: Eq + Hash + Borrow<Q>,
    Q: Eq + Hash + ?Sized,
{
    map.get_mut(key)
}

/// Look up a key in a map, returning a reference to the value or a default reference.
#[inline]
pub fn map_lookup_or<'a, K, V, Q>(map: &'a HashMap<K, V>, key: &Q, default: &'a V) -> &'a V
where
    K: Eq + Hash + Borrow<Q>,
    Q: Eq + Hash + ?Sized,
{
    map.get(key).unwrap_or(default)
}

/// Look up a key in a map, returning the value by clone or a default owned value.
#[inline]
pub fn map_lookup_value<K, V, Q>(map: &HashMap<K, V>, key: &Q, default: V) -> V
where
    K: Eq + Hash + Borrow<Q>,
    V: Clone,
    Q: Eq + Hash + ?Sized,
{
    map.get(key).cloned().unwrap_or(default)
}

/// Alias for `map_lookup_value`, matching stl.h naming.
#[inline]
pub fn map_lookup_by_value<K, V, Q>(map: &HashMap<K, V>, key: &Q, default: V) -> V
where
    K: Eq + Hash + Borrow<Q>,
    V: Clone,
    Q: Eq + Hash + ?Sized,
{
    map.get(key).cloned().unwrap_or(default)
}

/// Alias for `map_lookup` to match USD API naming (TfMapLookupPtr).
#[inline]
pub fn map_lookup_ptr<'a, K, V, Q>(map: &'a HashMap<K, V>, key: &Q) -> Option<&'a V>
where
    K: Eq + Hash + Borrow<Q>,
    Q: Eq + Hash + ?Sized,
{
    map.get(key)
}

// --- Ordered pair ---

/// Returns a tuple with elements in sorted order.
///
/// Useful for map keys where (a, b) should be equivalent to (b, a).
#[inline]
pub fn ordered_pair<T: Ord>(a: T, b: T) -> (T, T) {
    if a < b { (a, b) } else { (b, a) }
}

// --- Memory reset ---

/// Clears a container and releases its memory (generic, C++ swap-with-default pattern).
#[inline]
pub fn reset<T: Default>(obj: &mut T) {
    *obj = T::default();
}

/// Reset a Vec to empty state, reclaiming memory.
#[inline]
pub fn reset_vec<T>(vec: &mut Vec<T>) {
    *vec = Vec::new();
}

/// Reset a HashMap to empty state, reclaiming memory.
#[inline]
pub fn reset_hashmap<K, V>(map: &mut HashMap<K, V>) {
    *map = HashMap::new();
}

/// Reset a HashSet to empty state, reclaiming memory.
#[inline]
pub fn reset_hashset<T>(set: &mut HashSet<T>) {
    *set = HashSet::new();
}

// --- Ordered set operations ---

/// Computes the ordered set difference of two sequences.
///
/// Returns elements from the first iterator that are not in the second,
/// maintaining the relative order of the first. Duplicates are handled
/// by count: [1,3,3,1] - [2,3,2] = [1,3,1].
///
/// Uses only `Ord` (no `Hash` required), matching C++ std::multiset semantics.
pub fn ordered_set_difference<'a, I1, I2, T>(first: I1, second: I2) -> impl Iterator<Item = &'a T>
where
    I1: Iterator<Item = &'a T>,
    I2: Iterator<Item = &'a T>,
    T: Ord + 'a,
{
    let mut second_set: Vec<&T> = second.collect();
    second_set.sort();

    OrderedSetDiffIter {
        first: first.collect::<Vec<_>>().into_iter(),
        second_set,
    }
}

struct OrderedSetDiffIter<'a, T> {
    first: std::vec::IntoIter<&'a T>,
    second_set: Vec<&'a T>,
}

impl<'a, T: Ord> Iterator for OrderedSetDiffIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let item = self.first.next()?;
            if let Ok(pos) = self.second_set.binary_search(&item) {
                // Consume one occurrence from the multiset
                self.second_set.remove(pos);
            } else {
                return Some(item);
            }
        }
    }
}

/// Computes the ordered unique set difference of two sequences.
///
/// Like `ordered_set_difference`, but each element appears at most once in output.
/// [1,3,3,1] - [2,3,2] = [1]
pub fn ordered_uniquing_set_difference<'a, I1, I2, T>(
    first: I1,
    second: I2,
) -> impl Iterator<Item = &'a T>
where
    I1: Iterator<Item = &'a T>,
    I2: Iterator<Item = &'a T>,
    T: Ord + 'a,
{
    let second_set: BTreeSet<&T> = second.collect();
    let mut seen: BTreeSet<&'a T> = BTreeSet::new();

    first.filter(move |item| !second_set.contains(item) && seen.insert(item))
}

// --- Binary search boundary ---

/// Finds the boundary in a partitioned sequence using binary search.
///
/// Given a predicate that is `true` for all elements before the boundary
/// and `false` after, returns the index of the first element where `pred`
/// returns `false`. Equivalent to `std::partition_point` / `lower_bound`.
pub fn find_boundary<T, F>(slice: &[T], pred: F) -> usize
where
    F: Fn(&T) -> bool,
{
    let mut left = 0;
    let mut right = slice.len();

    while left < right {
        let mid = left + (right - left) / 2;
        if pred(&slice[mid]) {
            left = mid + 1;
        } else {
            right = mid;
        }
    }
    left
}

// --- Tuple element access ---

/// Trait for accessing the Nth element of a tuple by const generic index (TfGet<N>).
pub trait TupleGet<const N: usize> {
    type Output;
    fn get(&self) -> &Self::Output;
}

impl<T, U> TupleGet<0> for (T, U) {
    type Output = T;
    #[inline]
    fn get(&self) -> &Self::Output {
        &self.0
    }
}

impl<T, U> TupleGet<1> for (T, U) {
    type Output = U;
    #[inline]
    fn get(&self) -> &Self::Output {
        &self.1
    }
}

/// Extract the first element of a pair reference (TfGet<0> function form).
#[inline]
pub fn get_first<A, B>(pair: &(A, B)) -> &A {
    &pair.0
}

/// Extract the second element of a pair reference (TfGet<1> function form).
#[inline]
pub fn get_second<A, B>(pair: &(A, B)) -> &B {
    &pair.1
}

/// Extract the first element from an owned pair.
#[inline]
pub fn into_first<A, B>(pair: (A, B)) -> A {
    pair.0
}

/// Extract the second element from an owned pair.
#[inline]
pub fn into_second<A, B>(pair: (A, B)) -> B {
    pair.1
}

// --- STL algorithm equivalents ---

/// Returns true if the collection contains `value` (linear scan).
pub fn contains<'a, I, T>(iter: I, value: &T) -> bool
where
    I: IntoIterator<Item = &'a T>,
    T: PartialEq + 'a,
{
    iter.into_iter().any(|x| x == value)
}

/// Returns the first element matching `pred`, or `None`.
pub fn find_if<'a, I, T, F>(iter: I, pred: F) -> Option<&'a T>
where
    I: IntoIterator<Item = &'a T>,
    F: Fn(&T) -> bool,
{
    iter.into_iter().find(|&x| pred(x))
}

/// Counts elements matching `pred`.
pub fn count_if<'a, I, T: 'a, F>(iter: I, pred: F) -> usize
where
    I: IntoIterator<Item = &'a T>,
    F: Fn(&T) -> bool,
{
    iter.into_iter().filter(|&x| pred(x)).count()
}

/// Returns true if all elements satisfy `pred` (vacuously true for empty).
pub fn all_of<'a, I, T: 'a, F>(iter: I, pred: F) -> bool
where
    I: IntoIterator<Item = &'a T>,
    F: Fn(&T) -> bool,
{
    iter.into_iter().all(pred)
}

/// Returns true if any element satisfies `pred`.
pub fn any_of<'a, I, T: 'a, F>(iter: I, pred: F) -> bool
where
    I: IntoIterator<Item = &'a T>,
    F: Fn(&T) -> bool,
{
    iter.into_iter().any(pred)
}

/// Returns true if no element satisfies `pred`.
pub fn none_of<'a, I, T: 'a, F>(iter: I, pred: F) -> bool
where
    I: IntoIterator<Item = &'a T>,
    F: Fn(&T) -> bool,
{
    iter.into_iter().all(|x| !pred(x))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_lookup() {
        let mut m: HashMap<&str, i32> = HashMap::new();
        m.insert("foo", 42);

        assert_eq!(map_lookup(&m, &"foo"), Some(&42));
        assert_eq!(map_lookup(&m, &"bar"), None);
    }

    #[test]
    fn test_map_lookup_or() {
        let mut m: HashMap<&str, i32> = HashMap::new();
        m.insert("a", 1);

        assert_eq!(map_lookup_or(&m, &"a", &-1), &1);
        assert_eq!(map_lookup_or(&m, &"b", &-1), &-1);
    }

    #[test]
    fn test_map_lookup_value() {
        let mut m: HashMap<&str, i32> = HashMap::new();
        m.insert("foo", 42);

        assert_eq!(map_lookup_value(&m, &"foo", -1), 42);
        assert_eq!(map_lookup_value(&m, &"bar", -1), -1);
        assert_eq!(map_lookup_by_value(&m, &"foo", -1), 42);
        assert_eq!(map_lookup_by_value(&m, &"bar", -1), -1);
    }

    #[test]
    fn test_map_lookup_ptr_mut() {
        let mut m: HashMap<&str, i32> = HashMap::new();
        m.insert("foo", 42);

        if let Some(v) = map_lookup_ptr_mut(&mut m, &"foo") {
            *v = 100;
        }
        assert_eq!(m.get("foo"), Some(&100));
    }

    #[test]
    fn test_ordered_pair() {
        assert_eq!(ordered_pair(5, 3), (3, 5));
        assert_eq!(ordered_pair(2, 7), (2, 7));
        assert_eq!(ordered_pair(5, 5), (5, 5));
        assert_eq!(ordered_pair("b", "a"), ("a", "b"));
    }

    #[test]
    fn test_reset() {
        let mut v = vec![1, 2, 3, 4, 5];
        reset(&mut v);
        assert!(v.is_empty());
        assert_eq!(v.capacity(), 0);
    }

    #[test]
    fn test_reset_typed() {
        let mut v = vec![1, 2, 3];
        reset_vec(&mut v);
        assert!(v.is_empty());
        assert_eq!(v.capacity(), 0);

        let mut m: HashMap<i32, i32> = HashMap::new();
        m.insert(1, 1);
        reset_hashmap(&mut m);
        assert!(m.is_empty());

        let mut s: HashSet<i32> = HashSet::new();
        s.insert(1);
        reset_hashset(&mut s);
        assert!(s.is_empty());
    }

    #[test]
    fn test_ordered_set_difference() {
        let a = vec![1, 3, 3, 1];
        let b = vec![2, 3, 2];
        let result: Vec<_> = ordered_set_difference(a.iter(), b.iter()).collect();
        assert_eq!(result, vec![&1, &3, &1]);

        let c = vec![1, 2, 3];
        let d = vec![1, 2, 3, 4, 5];
        assert!(ordered_set_difference(c.iter(), d.iter()).next().is_none());

        let e = vec![1, 2, 3];
        let f = vec![4, 5, 6];
        let result2: Vec<_> = ordered_set_difference(e.iter(), f.iter()).collect();
        assert_eq!(result2, vec![&1, &2, &3]);
    }

    #[test]
    fn test_ordered_uniquing_set_difference() {
        let a = vec![1, 3, 3, 1];
        let b = vec![2, 3, 2];
        let result: Vec<_> = ordered_uniquing_set_difference(a.iter(), b.iter()).collect();
        assert_eq!(result, vec![&1]);

        let c = vec![1, 2, 3, 2, 1];
        let d = vec![2];
        let result2: Vec<_> = ordered_uniquing_set_difference(c.iter(), d.iter()).collect();
        assert_eq!(result2, vec![&1, &3]);
    }

    #[test]
    fn test_find_boundary() {
        let v = vec![1, 2, 3, 4, 5, 6, 7, 8, 9];

        assert_eq!(find_boundary(&v, |&x| x < 5), 4);
        assert_eq!(find_boundary(&v, |&x| x < 1), 0);
        assert_eq!(find_boundary(&v, |&x| x < 10), 9);

        let empty: Vec<i32> = vec![];
        assert_eq!(find_boundary(&empty, |&x| x < 5), 0);
        assert_eq!(find_boundary(&empty, |_| true), 0);
    }

    #[test]
    fn test_tuple_get() {
        let pair = (10, "hello");
        assert_eq!(*TupleGet::<0>::get(&pair), 10);
        assert_eq!(*TupleGet::<1>::get(&pair), "hello");
    }

    #[test]
    fn test_get_first_second() {
        let pairs = vec![(1, "a"), (2, "b"), (3, "c")];

        let firsts: Vec<_> = pairs.iter().map(get_first).collect();
        assert_eq!(firsts, vec![&1, &2, &3]);

        let seconds: Vec<_> = pairs.iter().map(get_second).collect();
        assert_eq!(seconds, vec![&"a", &"b", &"c"]);
    }

    #[test]
    fn test_into_first_second() {
        assert_eq!(into_first((1, "a")), 1);
        assert_eq!(into_second((1, "a")), "a");
    }

    #[test]
    fn test_contains() {
        let v = vec![1, 2, 3, 4, 5];
        assert!(contains(&v, &3));
        assert!(!contains(&v, &10));
    }

    #[test]
    fn test_find_if() {
        let v = vec![1, 2, 3, 4, 5];
        assert_eq!(find_if(&v, |&x| x > 3), Some(&4));
        assert_eq!(find_if(&v, |&x| x > 10), None);
    }

    #[test]
    fn test_count_if() {
        let v = vec![1, 2, 3, 4, 5, 6];
        assert_eq!(count_if(&v, |&x| x % 2 == 0), 3);
        assert_eq!(count_if(&v, |&x| x > 10), 0);
    }

    #[test]
    fn test_all_any_none() {
        let evens = vec![2, 4, 6, 8];
        let mixed = vec![1, 2, 3, 4];

        assert!(all_of(&evens, |&x| x % 2 == 0));
        assert!(!all_of(&mixed, |&x| x % 2 == 0));

        assert!(any_of(&mixed, |&x| x % 2 == 0));
        assert!(!any_of(&[1, 3, 5], |&x| x % 2 == 0));

        assert!(none_of(&[1, 3, 5], |&x| x % 2 == 0));
        assert!(!none_of(&mixed, |&x| x % 2 == 0));
    }
}
