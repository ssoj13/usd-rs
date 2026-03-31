//! SdfChildrenView - views over children of specs.
//!
//! Port of pxr/usd/sdf/childrenView.h
//!
//! Provides filtered, read-only views over the children of a spec.
//! Views can filter children using predicates and adapt values to
//! different types.

use crate::{Layer, Path, PrimSpec, PropertySpec};
use std::marker::PhantomData;
use std::sync::Arc;
use usd_tf::Token;

/// A view over children of a spec.
///
/// This provides a filtered, read-only view over children stored in a spec.
/// The view can filter children using a predicate and adapt them to different
/// types.
///
/// # Type Parameters
/// * `T` - The type of items in the view
/// * `P` - The predicate for filtering (defaults to accepting all)
#[derive(Clone)]
pub struct ChildrenView<T, P = TrivialPredicate<T>> {
    /// Items in the view.
    items: Vec<T>,
    /// Predicate for filtering.
    predicate: P,
}

/// Predicate that always returns true (no filtering).
#[derive(Clone, Copy, Default)]
pub struct TrivialPredicate<T>(PhantomData<T>);

impl<T> TrivialPredicate<T> {
    /// Creates a new trivial predicate.
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

/// Trait for predicates that filter children.
pub trait ChildPredicate<T> {
    /// Returns true if the item should be included.
    fn accept(&self, item: &T) -> bool;
}

impl<T> ChildPredicate<T> for TrivialPredicate<T> {
    fn accept(&self, _item: &T) -> bool {
        true
    }
}

impl<T, F> ChildPredicate<T> for F
where
    F: Fn(&T) -> bool,
{
    fn accept(&self, item: &T) -> bool {
        self(item)
    }
}

impl<T, P: ChildPredicate<T>> ChildrenView<T, P> {
    /// Creates a new children view with a predicate.
    pub fn new(items: Vec<T>, predicate: P) -> Self {
        Self { items, predicate }
    }

    /// Returns the number of items that pass the predicate.
    pub fn len(&self) -> usize {
        self.items
            .iter()
            .filter(|i| self.predicate.accept(i))
            .count()
    }

    /// Returns true if no items pass the predicate.
    pub fn is_empty(&self) -> bool {
        !self.items.iter().any(|i| self.predicate.accept(i))
    }

    /// Returns an iterator over items that pass the predicate.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.items.iter().filter(|i| self.predicate.accept(*i))
    }

    /// Returns true if the view contains the given item.
    pub fn contains(&self, item: &T) -> bool
    where
        T: PartialEq,
    {
        self.items
            .iter()
            .any(|i| i == item && self.predicate.accept(i))
    }

    /// Returns the item at the given index (counting only filtered items).
    pub fn get(&self, index: usize) -> Option<&T> {
        self.iter().nth(index)
    }

    /// Returns the first item that passes the predicate.
    pub fn first(&self) -> Option<&T> {
        self.get(0)
    }

    /// Returns the last item that passes the predicate.
    pub fn last(&self) -> Option<&T> {
        self.items.iter().rev().find(|i| self.predicate.accept(*i))
    }

    /// Finds an item by key.
    pub fn find<K>(&self, key: &K) -> Option<&T>
    where
        T: AsKey<K>,
        K: ?Sized + PartialEq,
    {
        self.items
            .iter()
            .find(|i| i.as_key() == key && self.predicate.accept(*i))
    }

    /// Collects the view into a vector.
    pub fn to_vec(&self) -> Vec<T>
    where
        T: Clone,
    {
        self.items
            .iter()
            .filter(|i| self.predicate.accept(*i))
            .cloned()
            .collect()
    }

    /// Returns the keys (names) of all items.
    pub fn keys(&self) -> Vec<Token>
    where
        T: AsKey<str>,
    {
        self.items
            .iter()
            .filter(|i| self.predicate.accept(*i))
            .map(|i| Token::new(i.as_key()))
            .collect()
    }
}

impl<T> ChildrenView<T, TrivialPredicate<T>> {
    /// Creates an unfiltered view from items.
    pub fn from_items(items: Vec<T>) -> Self {
        Self::new(items, TrivialPredicate::new())
    }

    /// Creates an empty view.
    pub fn empty() -> Self {
        Self::from_items(Vec::new())
    }
}

impl<T: Clone, P: ChildPredicate<T> + Clone> IntoIterator for ChildrenView<T, P> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.to_vec().into_iter()
    }
}

/// Trait for items that have a key for lookup.
pub trait AsKey<K: ?Sized> {
    /// Returns the key for this item.
    fn as_key(&self) -> &K;
}

impl AsKey<str> for Token {
    fn as_key(&self) -> &str {
        self.as_str()
    }
}

impl AsKey<str> for String {
    fn as_key(&self) -> &str {
        self.as_str()
    }
}

// Note: PrimSpec and PropertySpec can't impl AsKey<str> because name_token()/name() return
// owned values. Use find_by_name() methods instead which take String/&str comparisons.

impl PrimSpec {
    /// Find by name in a collection.
    pub fn name_matches(&self, name: &str) -> bool {
        self.name_token() == name
    }
}

impl PropertySpec {
    /// Check if name matches.
    pub fn name_matches(&self, name: &str) -> bool {
        self.name() == name
    }
}

/// View of prim children.
pub type PrimChildrenView = ChildrenView<PrimSpec>;

/// View of property children.
pub type PropertyChildrenView = ChildrenView<PropertySpec>;

/// View of token names (for ordering lists).
pub type NameChildrenView = ChildrenView<Token>;

/// Creates a prim children view for a prim spec.
pub fn prim_children(layer: &Arc<Layer>, path: &Path) -> PrimChildrenView {
    if let Some(prim) = layer.get_prim_at_path(path) {
        ChildrenView::from_items(prim.name_children())
    } else {
        ChildrenView::empty()
    }
}

/// Creates a property children view for a prim spec.
pub fn property_children(layer: &Arc<Layer>, path: &Path) -> PropertyChildrenView {
    if let Some(prim) = layer.get_prim_at_path(path) {
        ChildrenView::from_items(prim.properties())
    } else {
        ChildrenView::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_view() {
        let view: ChildrenView<Token> = ChildrenView::empty();
        assert!(view.is_empty());
        assert_eq!(view.len(), 0);
    }

    #[test]
    fn test_from_items() {
        let items = vec![Token::new("a"), Token::new("b"), Token::new("c")];
        let view = ChildrenView::from_items(items);

        assert_eq!(view.len(), 3);
        assert!(!view.is_empty());
        assert_eq!(view.first().unwrap().as_str(), "a");
        assert_eq!(view.last().unwrap().as_str(), "c");
    }

    #[test]
    fn test_filtered_view() {
        let items = vec![Token::new("a"), Token::new("ab"), Token::new("abc")];

        // Filter to only tokens starting with "ab"
        let predicate = |t: &Token| t.as_str().starts_with("ab");
        let view = ChildrenView::new(items, predicate);

        assert_eq!(view.len(), 2);
        assert_eq!(view.first().unwrap().as_str(), "ab");
    }

    #[test]
    fn test_find() {
        let items = vec![Token::new("foo"), Token::new("bar"), Token::new("baz")];
        let view = ChildrenView::from_items(items);

        assert!(view.find("bar").is_some());
        assert!(view.find("qux").is_none());
    }

    #[test]
    fn test_keys() {
        let items = vec![Token::new("x"), Token::new("y"), Token::new("z")];
        let view = ChildrenView::from_items(items);

        let keys = view.keys();
        assert_eq!(keys.len(), 3);
        assert_eq!(keys[0].as_str(), "x");
    }
}
