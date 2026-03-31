//! ListOp-based list editor implementation.
//!
//! Port of pxr/usd/sdf/listOpListEditor.h
//!
//! List editor implementation for list editing operations stored in
//! an SdfListOp object. Supports the full set of list operations:
//! explicit, prepend, append, delete, and reorder.

use crate::{Layer, ListOp, ListOpType, Path};
use usd_tf::Token;
use std::fmt;
use std::hash::Hash;
use std::sync::Arc;

/// Trait for list op list editor type policies.
///
/// Defines the value type and field handling for a specific kind of
/// list op editor (e.g., references, payloads, inherits).
pub trait ListOpTypePolicy: Clone + Send + Sync + 'static {
    /// The value type for list operations.
    type Value: Clone + Eq + Hash + fmt::Debug + Send + Sync;

    /// Returns the field name token for this list op.
    fn field_name() -> Token;

    /// Canonicalizes a value (e.g., making paths absolute).
    fn canonicalize(value: &Self::Value) -> Self::Value {
        value.clone()
    }
}

/// ListOp-based list editor.
///
/// Stores list editing operations (explicit, prepend, append, delete,
/// reorder) in an SdfListOp field associated with an owning spec.
pub struct ListOpListEditor<P: ListOpTypePolicy> {
    /// The owning layer.
    layer: Option<Arc<Layer>>,
    /// Path to the owning spec.
    owner_path: Path,
    /// The underlying list op.
    list_op: ListOp<P::Value>,
    /// Marker for the policy type.
    _policy: std::marker::PhantomData<P>,
}

impl<P: ListOpTypePolicy> ListOpListEditor<P> {
    /// Creates a new list op list editor.
    pub fn new(layer: Arc<Layer>, owner_path: Path) -> Self {
        Self {
            layer: Some(layer),
            owner_path,
            list_op: ListOp::new(),
            _policy: std::marker::PhantomData,
        }
    }

    /// Creates an editor with an existing list op.
    pub fn with_list_op(layer: Arc<Layer>, owner_path: Path, list_op: ListOp<P::Value>) -> Self {
        Self {
            layer: Some(layer),
            owner_path,
            list_op,
            _policy: std::marker::PhantomData,
        }
    }

    /// Returns the owning layer.
    pub fn layer(&self) -> Option<&Arc<Layer>> {
        self.layer.as_ref()
    }

    /// Returns the owning spec path.
    pub fn path(&self) -> &Path {
        &self.owner_path
    }

    /// Returns the field name for this editor.
    pub fn field_name(&self) -> Token {
        P::field_name()
    }

    /// Returns true if the list is in explicit mode.
    pub fn is_explicit(&self) -> bool {
        self.list_op.is_explicit()
    }

    /// Returns true if this is ordered-only (only reorder ops).
    pub fn is_ordered_only(&self) -> bool {
        !self.is_explicit()
            && self.list_op.get_prepended_items().is_empty()
            && self.list_op.get_appended_items().is_empty()
            && self.list_op.get_deleted_items().is_empty()
    }

    /// Returns a reference to the underlying list op.
    pub fn get_list_op(&self) -> &ListOp<P::Value> {
        &self.list_op
    }

    /// Returns a mutable reference to the underlying list op.
    pub fn get_list_op_mut(&mut self) -> &mut ListOp<P::Value> {
        &mut self.list_op
    }

    /// Returns items for the given operation type.
    pub fn get_items(&self, op: ListOpType) -> &[P::Value] {
        match op {
            ListOpType::Explicit => self.list_op.get_explicit_items(),
            ListOpType::Prepended => self.list_op.get_prepended_items(),
            ListOpType::Appended => self.list_op.get_appended_items(),
            ListOpType::Deleted => self.list_op.get_deleted_items(),
            ListOpType::Ordered => self.list_op.get_ordered_items(),
        }
    }

    /// Copies edits from another list op.
    pub fn copy_edits_from(&mut self, other: &ListOp<P::Value>) -> bool {
        self.list_op = other.clone();
        true
    }

    /// Clears all edits.
    pub fn clear_edits(&mut self) -> bool {
        self.list_op = ListOp::new();
        true
    }

    /// Clears edits and switches to explicit mode.
    pub fn clear_edits_and_make_explicit(&mut self) -> bool {
        self.list_op = ListOp::create_explicit(vec![]);
        true
    }

    /// Modifies all items using a callback.
    pub fn modify_item_edits<F>(&mut self, callback: F)
    where
        F: Fn(&P::Value) -> Option<P::Value>,
    {
        self.list_op.modify_operations(&callback);
    }

    /// Applies list edits to an existing vector.
    pub fn apply_edits_to_list<F>(&self, vec: &mut Vec<P::Value>, callback: F)
    where
        F: Fn(ListOpType, &P::Value) -> Option<P::Value>,
    {
        self.list_op.apply_operations(vec, callback);
    }

    /// Replaces items at the given position for the specified operation.
    pub fn replace_edits(
        &mut self,
        op: ListOpType,
        index: usize,
        n: usize,
        new_items: Vec<P::Value>,
    ) -> bool {
        let items = self.get_items(op).to_vec();
        if index > items.len() {
            return false;
        }

        let mut new_list = items;
        let end = (index + n).min(new_list.len());
        new_list.splice(index..end, new_items);

        match op {
            ListOpType::Explicit => self.list_op.set_explicit_items(new_list),
            ListOpType::Prepended => self.list_op.set_prepended_items(new_list),
            ListOpType::Appended => self.list_op.set_appended_items(new_list),
            ListOpType::Deleted => self.list_op.set_deleted_items(new_list),
            ListOpType::Ordered => self.list_op.set_ordered_items(new_list),
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct TestTokenPolicy;

    impl ListOpTypePolicy for TestTokenPolicy {
        type Value = Token;

        fn field_name() -> Token {
            Token::from("testListOp")
        }
    }

    #[test]
    fn test_list_op_new() {
        let list_op = ListOp::<Token>::new();
        assert!(!list_op.is_explicit());
    }
}
