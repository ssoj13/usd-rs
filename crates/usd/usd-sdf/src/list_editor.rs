//! Sdf_ListEditor - base class for list editor implementations.
//!
//! Port of pxr/usd/sdf/listEditor.h
//!
//! Base class for list editor implementations in which list editing operations
//! are stored in data field(s) associated with an owning spec.

use crate::{Allowed, Layer, ListOpType, Path};
use std::sync::{Arc, Weak};
use usd_tf::Token;

/// Trait for list editor type policies.
pub trait TypePolicy: Clone {
    /// The value type for this policy.
    type Value: Clone + PartialEq + std::fmt::Debug;

    /// Canonicalizes a value.
    fn canonicalize(&self, value: &Self::Value) -> Self::Value {
        value.clone()
    }
}

/// Base trait for list editors.
///
/// List editors store editing operations (add, prepend, append, delete, order)
/// in data fields associated with an owning spec.
pub trait ListEditor<T: Clone + PartialEq> {
    /// Returns the layer containing this editor.
    fn layer(&self) -> Option<Arc<Layer>>;

    /// Returns the path of the owning spec.
    fn path(&self) -> Path;

    /// Returns true if the editor is valid.
    fn is_valid(&self) -> bool;

    /// Returns true if the editor is expired.
    fn is_expired(&self) -> bool;

    /// Returns true if any operations are stored.
    fn has_keys(&self) -> bool;

    /// Returns true if this is an explicit list.
    fn is_explicit(&self) -> bool;

    /// Returns true if this is ordered-only.
    fn is_ordered_only(&self) -> bool;

    /// Checks permission to edit.
    fn permission_to_edit(&self, op: ListOpType) -> Allowed;

    /// Copies edits from another editor.
    fn copy_edits<E: ListEditor<T>>(&mut self, rhs: &E) -> bool;

    /// Clears all edits.
    fn clear_edits(&mut self) -> bool;

    /// Clears edits and makes explicit.
    fn clear_edits_and_make_explicit(&mut self) -> bool;

    /// Modifies items in all operation lists.
    fn modify_item_edits<F>(&mut self, callback: F)
    where
        F: Fn(&T) -> Option<T>;

    /// Applies edits to a vector.
    fn apply_edits_to_list<F>(&self, vec: &mut Vec<T>, callback: Option<F>)
    where
        F: Fn(ListOpType, &T) -> Option<T>;

    /// Returns the size of an operation list.
    fn get_size(&self, op: ListOpType) -> usize;

    /// Returns the i'th value in an operation list.
    fn get(&self, op: ListOpType, i: usize) -> Option<T>;

    /// Returns the specified operation list.
    fn get_vector(&self, op: ListOpType) -> Vec<T>;

    /// Returns count of value in operation list.
    fn count(&self, op: ListOpType, val: &T) -> usize;

    /// Finds value index in operation list.
    fn find(&self, op: ListOpType, val: &T) -> Option<usize>;

    /// Replaces edits in range.
    fn replace_edits(&mut self, op: ListOpType, index: usize, n: usize, elems: &[T]) -> bool;

    /// Applies another editor's list to this one.
    fn apply_list<E: ListEditor<T>>(&mut self, op: ListOpType, rhs: &E);
}

/// Simple in-memory list editor implementation.
#[derive(Debug, Clone)]
pub struct SimpleListEditor<T: Clone + PartialEq> {
    /// Weak reference to layer.
    layer: Weak<Layer>,
    /// Path to owning spec.
    path: Path,
    /// Field name.
    #[allow(dead_code)]
    field: Token,
    /// Is explicit list.
    is_explicit: bool,
    /// Explicit items (when is_explicit=true).
    explicit_items: Vec<T>,
    /// Added items.
    added_items: Vec<T>,
    /// Prepended items.
    prepended_items: Vec<T>,
    /// Appended items.
    appended_items: Vec<T>,
    /// Deleted items.
    deleted_items: Vec<T>,
    /// Ordered items.
    ordered_items: Vec<T>,
}

impl<T: Clone + PartialEq + std::fmt::Debug> SimpleListEditor<T> {
    /// Creates a new simple list editor.
    pub fn new(layer: &Arc<Layer>, path: Path, field: Token) -> Self {
        Self {
            layer: Arc::downgrade(layer),
            path,
            field,
            is_explicit: false,
            explicit_items: Vec::new(),
            added_items: Vec::new(),
            prepended_items: Vec::new(),
            appended_items: Vec::new(),
            deleted_items: Vec::new(),
            ordered_items: Vec::new(),
        }
    }

    /// Creates an empty detached list editor.
    pub fn empty() -> Self {
        Self {
            layer: Weak::new(),
            path: Path::empty(),
            field: Token::empty(),
            is_explicit: false,
            explicit_items: Vec::new(),
            added_items: Vec::new(),
            prepended_items: Vec::new(),
            appended_items: Vec::new(),
            deleted_items: Vec::new(),
            ordered_items: Vec::new(),
        }
    }

    /// Sets explicit mode.
    pub fn set_explicit(&mut self, explicit: bool) {
        self.is_explicit = explicit;
    }

    /// Sets explicit items.
    pub fn set_explicit_items(&mut self, items: Vec<T>) {
        self.is_explicit = true;
        self.explicit_items = items;
    }

    /// Sets added items.
    pub fn set_added_items(&mut self, items: Vec<T>) {
        self.added_items = items;
    }

    /// Sets prepended items.
    pub fn set_prepended_items(&mut self, items: Vec<T>) {
        self.prepended_items = items;
    }

    /// Sets appended items.
    pub fn set_appended_items(&mut self, items: Vec<T>) {
        self.appended_items = items;
    }

    /// Sets deleted items.
    pub fn set_deleted_items(&mut self, items: Vec<T>) {
        self.deleted_items = items;
    }

    /// Sets ordered items.
    pub fn set_ordered_items(&mut self, items: Vec<T>) {
        self.ordered_items = items;
    }

    /// Gets mutable reference to operation list.
    fn get_ops_mut(&mut self, op: ListOpType) -> &mut Vec<T> {
        match op {
            ListOpType::Explicit => &mut self.explicit_items,
            ListOpType::Added => &mut self.added_items,
            ListOpType::Prepended => &mut self.prepended_items,
            ListOpType::Appended => &mut self.appended_items,
            ListOpType::Deleted => &mut self.deleted_items,
            ListOpType::Ordered => &mut self.ordered_items,
        }
    }

    /// Gets reference to operation list.
    fn get_ops(&self, op: ListOpType) -> &Vec<T> {
        match op {
            ListOpType::Explicit => &self.explicit_items,
            ListOpType::Added => &self.added_items,
            ListOpType::Prepended => &self.prepended_items,
            ListOpType::Appended => &self.appended_items,
            ListOpType::Deleted => &self.deleted_items,
            ListOpType::Ordered => &self.ordered_items,
        }
    }
}

impl<T: Clone + PartialEq + std::fmt::Debug> ListEditor<T> for SimpleListEditor<T> {
    fn layer(&self) -> Option<Arc<Layer>> {
        self.layer.upgrade()
    }

    fn path(&self) -> Path {
        self.path.clone()
    }

    fn is_valid(&self) -> bool {
        !self.is_expired()
    }

    fn is_expired(&self) -> bool {
        self.layer.upgrade().is_none() && !self.path.is_empty()
    }

    fn has_keys(&self) -> bool {
        if self.is_explicit {
            true
        } else if self.is_ordered_only() {
            !self.ordered_items.is_empty()
        } else {
            !self.added_items.is_empty()
                || !self.prepended_items.is_empty()
                || !self.appended_items.is_empty()
                || !self.deleted_items.is_empty()
                || !self.ordered_items.is_empty()
        }
    }

    fn is_explicit(&self) -> bool {
        self.is_explicit
    }

    fn is_ordered_only(&self) -> bool {
        !self.is_explicit
            && self.added_items.is_empty()
            && self.prepended_items.is_empty()
            && self.appended_items.is_empty()
            && self.deleted_items.is_empty()
    }

    fn permission_to_edit(&self, _op: ListOpType) -> Allowed {
        if self.is_expired() {
            return Allowed::no("List editor is expired");
        }
        Allowed::yes()
    }

    fn copy_edits<E: ListEditor<T>>(&mut self, rhs: &E) -> bool {
        self.is_explicit = rhs.is_explicit();
        self.explicit_items = rhs.get_vector(ListOpType::Explicit);
        self.added_items = rhs.get_vector(ListOpType::Added);
        self.prepended_items = rhs.get_vector(ListOpType::Prepended);
        self.appended_items = rhs.get_vector(ListOpType::Appended);
        self.deleted_items = rhs.get_vector(ListOpType::Deleted);
        self.ordered_items = rhs.get_vector(ListOpType::Ordered);
        true
    }

    fn clear_edits(&mut self) -> bool {
        self.is_explicit = false;
        self.explicit_items.clear();
        self.added_items.clear();
        self.prepended_items.clear();
        self.appended_items.clear();
        self.deleted_items.clear();
        self.ordered_items.clear();
        true
    }

    fn clear_edits_and_make_explicit(&mut self) -> bool {
        self.clear_edits();
        self.is_explicit = true;
        true
    }

    fn modify_item_edits<F>(&mut self, callback: F)
    where
        F: Fn(&T) -> Option<T>,
    {
        for op in [
            ListOpType::Explicit,
            ListOpType::Added,
            ListOpType::Prepended,
            ListOpType::Appended,
            ListOpType::Deleted,
            ListOpType::Ordered,
        ] {
            let ops = self.get_ops_mut(op);
            let mut new_ops = Vec::with_capacity(ops.len());
            for item in ops.iter() {
                if let Some(new_item) = callback(item) {
                    // Check for duplicates
                    if !new_ops.contains(&new_item) {
                        new_ops.push(new_item);
                    }
                }
            }
            *ops = new_ops;
        }
    }

    fn apply_edits_to_list<F>(&self, vec: &mut Vec<T>, callback: Option<F>)
    where
        F: Fn(ListOpType, &T) -> Option<T>,
    {
        if self.is_explicit {
            vec.clear();
            for item in &self.explicit_items {
                let item = if let Some(ref cb) = callback {
                    match cb(ListOpType::Explicit, item) {
                        Some(v) => v,
                        None => continue,
                    }
                } else {
                    item.clone()
                };
                if !vec.contains(&item) {
                    vec.push(item);
                }
            }
            return;
        }

        // Apply deletions
        for item in &self.deleted_items {
            let item = if let Some(ref cb) = callback {
                match cb(ListOpType::Deleted, item) {
                    Some(v) => v,
                    None => continue,
                }
            } else {
                item.clone()
            };
            vec.retain(|v| v != &item);
        }

        // Apply additions
        for item in &self.added_items {
            let item = if let Some(ref cb) = callback {
                match cb(ListOpType::Added, item) {
                    Some(v) => v,
                    None => continue,
                }
            } else {
                item.clone()
            };
            if !vec.contains(&item) {
                vec.push(item);
            }
        }

        // Apply prepends (at beginning)
        for item in self.prepended_items.iter().rev() {
            let item = if let Some(ref cb) = callback {
                match cb(ListOpType::Prepended, item) {
                    Some(v) => v,
                    None => continue,
                }
            } else {
                item.clone()
            };
            vec.retain(|v| v != &item);
            vec.insert(0, item);
        }

        // Apply appends (at end)
        for item in &self.appended_items {
            let item = if let Some(ref cb) = callback {
                match cb(ListOpType::Appended, item) {
                    Some(v) => v,
                    None => continue,
                }
            } else {
                item.clone()
            };
            vec.retain(|v| v != &item);
            vec.push(item);
        }

        // Apply ordering
        if !self.ordered_items.is_empty() {
            let mut ordered = Vec::new();
            for item in &self.ordered_items {
                let item = if let Some(ref cb) = callback {
                    match cb(ListOpType::Ordered, item) {
                        Some(v) => v,
                        None => continue,
                    }
                } else {
                    item.clone()
                };
                if let Some(pos) = vec.iter().position(|v| v == &item) {
                    ordered.push(vec.remove(pos));
                }
            }
            // Append remaining items
            ordered.append(vec);
            *vec = ordered;
        }
    }

    fn get_size(&self, op: ListOpType) -> usize {
        self.get_ops(op).len()
    }

    fn get(&self, op: ListOpType, i: usize) -> Option<T> {
        self.get_ops(op).get(i).cloned()
    }

    fn get_vector(&self, op: ListOpType) -> Vec<T> {
        self.get_ops(op).clone()
    }

    fn count(&self, op: ListOpType, val: &T) -> usize {
        self.get_ops(op).iter().filter(|v| *v == val).count()
    }

    fn find(&self, op: ListOpType, val: &T) -> Option<usize> {
        self.get_ops(op).iter().position(|v| v == val)
    }

    fn replace_edits(&mut self, op: ListOpType, index: usize, n: usize, elems: &[T]) -> bool {
        let ops = self.get_ops_mut(op);
        if index > ops.len() {
            return false;
        }
        let end = (index + n).min(ops.len());
        ops.splice(index..end, elems.iter().cloned());
        true
    }

    fn apply_list<E: ListEditor<T>>(&mut self, op: ListOpType, rhs: &E) {
        let rhs_vec = rhs.get_vector(op);
        let ops = self.get_ops_mut(op);
        for item in rhs_vec {
            if !ops.contains(&item) {
                ops.push(item);
            }
        }
    }
}

/// Token list editor.
pub type TokenListEditor = SimpleListEditor<Token>;

/// Path list editor.
pub type PathListEditor = SimpleListEditor<Path>;

/// String list editor.
pub type StringListEditor = SimpleListEditor<String>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_list_editor() {
        let mut editor: SimpleListEditor<i32> = SimpleListEditor::empty();

        editor.set_prepended_items(vec![1, 2]);
        editor.set_appended_items(vec![3, 4]);

        let mut vec = vec![10, 20];
        editor.apply_edits_to_list::<fn(ListOpType, &i32) -> Option<i32>>(&mut vec, None);

        // Prepended [1, 2] with base [10, 20] gives [1, 2, 10, 20]
        // Appended [3, 4] gives final [1, 2, 10, 20, 3, 4]
        // (OpenUSD preserves order: prepend [1,2,3] to [] = [1,2,3])
        assert_eq!(vec, vec![1, 2, 10, 20, 3, 4]);
    }

    #[test]
    fn test_explicit_list() {
        let mut editor: SimpleListEditor<i32> = SimpleListEditor::empty();
        editor.set_explicit_items(vec![100, 200, 300]);

        let mut vec = vec![1, 2, 3];
        editor.apply_edits_to_list::<fn(ListOpType, &i32) -> Option<i32>>(&mut vec, None);

        assert_eq!(vec, vec![100, 200, 300]);
    }

    #[test]
    fn test_deletions() {
        let mut editor: SimpleListEditor<i32> = SimpleListEditor::empty();
        editor.set_deleted_items(vec![2, 4]);

        let mut vec = vec![1, 2, 3, 4, 5];
        editor.apply_edits_to_list::<fn(ListOpType, &i32) -> Option<i32>>(&mut vec, None);

        assert_eq!(vec, vec![1, 3, 5]);
    }
}
