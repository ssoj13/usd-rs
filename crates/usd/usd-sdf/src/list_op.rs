//! List editing operations.
//!
//! `ListOp<T>` represents an operation that edits a list of items.
//! It supports prepending, appending, deleting, or explicitly replacing items.

use std::collections::HashSet;
use std::fmt;
use std::hash::Hash;

/// Enum for specifying one of the list editing operation types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ListOpType {
    /// Explicitly set the list (replaces all items).
    Explicit,
    /// Added items (deprecated, use Prepended/Appended).
    Added,
    /// Deleted items.
    Deleted,
    /// Ordered items.
    Ordered,
    /// Prepended items (added to front).
    Prepended,
    /// Appended items (added to back).
    Appended,
}

/// A value type representing an operation that edits a list.
///
/// `ListOp` maintains lists of items to be prepended, appended, deleted, or
/// used explicitly. In explicit mode, ApplyOperations replaces the list entirely.
/// Otherwise, operations are applied in order: Delete, Prepend, Append.
///
/// Lists contain unique values - all operations remove duplicates.
/// Prepending preserves the first occurrence, appending preserves the last.
///
/// # Type Parameters
///
/// * `T` - The item type. Must be Clone, Eq, and Hash.
///
/// # Examples
///
/// ```ignore
/// use usd_sdf::{ListOp, ListOpType};
///
/// // Create a list op that prepends and appends
/// let mut op = ListOp::<String>::new();
/// op.set_prepended_items(vec!["first".into()]);
/// op.set_appended_items(vec!["last".into()]);
///
/// let mut items = vec!["middle".into()];
/// let no_callback: Option<fn(ListOpType, &String) -> Option<String>> = None;
/// op.apply_operations(&mut items, no_callback);
/// assert_eq!(items, vec!["first", "middle", "last"]);
/// ```
#[derive(Clone)]
pub struct ListOp<T>
where
    T: Clone + Eq + Hash,
{
    /// Whether the list is in explicit mode.
    is_explicit: bool,
    /// Explicit items (when is_explicit is true).
    explicit_items: Vec<T>,
    /// Added items (deprecated).
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

impl<T> Default for ListOp<T>
where
    T: Clone + Eq + Hash + fmt::Debug,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T> ListOp<T>
where
    T: Clone + Eq + Hash + fmt::Debug,
{
    /// Creates an empty ListOp in non-explicit mode.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::ListOp;
    ///
    /// let op = ListOp::<i32>::new();
    /// assert!(!op.is_explicit());
    /// assert!(!op.has_keys());
    /// ```
    pub fn new() -> Self {
        Self {
            is_explicit: false,
            explicit_items: Vec::new(),
            added_items: Vec::new(),
            prepended_items: Vec::new(),
            appended_items: Vec::new(),
            deleted_items: Vec::new(),
            ordered_items: Vec::new(),
        }
    }

    /// Creates a ListOp in explicit mode with the given items.
    ///
    /// # Arguments
    ///
    /// * `explicit_items` - The explicit items
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::ListOp;
    ///
    /// let op = ListOp::create_explicit(vec![1, 2, 3]);
    /// assert!(op.is_explicit());
    /// assert_eq!(op.get_explicit_items(), &[1, 2, 3]);
    /// ```
    pub fn create_explicit(explicit_items: Vec<T>) -> Self {
        let mut op = Self::new();
        op.is_explicit = true;
        let _ = op.set_explicit_items(explicit_items);
        op
    }

    /// Creates a ListOp with prepended, appended, and deleted items.
    ///
    /// # Arguments
    ///
    /// * `prepended_items` - Items to prepend
    /// * `appended_items` - Items to append
    /// * `deleted_items` - Items to delete
    pub fn create(prepended_items: Vec<T>, appended_items: Vec<T>, deleted_items: Vec<T>) -> Self {
        let mut op = Self::new();
        let _ = op.set_prepended_items(prepended_items);
        let _ = op.set_appended_items(appended_items);
        let _ = op.set_deleted_items(deleted_items);
        op
    }

    /// Returns true if the list has any keys (explicit, added, prepended,
    /// appended, deleted, or ordered items).
    pub fn has_keys(&self) -> bool {
        if self.is_explicit {
            return true;
        }
        !self.added_items.is_empty()
            || !self.prepended_items.is_empty()
            || !self.appended_items.is_empty()
            || !self.deleted_items.is_empty()
            || !self.ordered_items.is_empty()
    }

    /// Returns true if the given item is in any of the item lists.
    pub fn has_item(&self, item: &T) -> bool {
        self.explicit_items.contains(item)
            || self.added_items.contains(item)
            || self.prepended_items.contains(item)
            || self.appended_items.contains(item)
            || self.deleted_items.contains(item)
            || self.ordered_items.contains(item)
    }

    /// Returns true if the list is in explicit mode.
    pub fn is_explicit(&self) -> bool {
        self.is_explicit
    }

    /// Returns the explicit items.
    pub fn get_explicit_items(&self) -> &[T] {
        &self.explicit_items
    }

    /// Returns the added items.
    pub fn get_added_items(&self) -> &[T] {
        &self.added_items
    }

    /// Returns the prepended items.
    pub fn get_prepended_items(&self) -> &[T] {
        &self.prepended_items
    }

    /// Returns the appended items.
    pub fn get_appended_items(&self) -> &[T] {
        &self.appended_items
    }

    /// Returns the deleted items.
    pub fn get_deleted_items(&self) -> &[T] {
        &self.deleted_items
    }

    /// Returns the ordered items.
    pub fn get_ordered_items(&self) -> &[T] {
        &self.ordered_items
    }

    /// Returns the items for the given operation type.
    pub fn get_items(&self, op_type: ListOpType) -> &[T] {
        match op_type {
            ListOpType::Explicit => &self.explicit_items,
            ListOpType::Added => &self.added_items,
            ListOpType::Deleted => &self.deleted_items,
            ListOpType::Ordered => &self.ordered_items,
            ListOpType::Prepended => &self.prepended_items,
            ListOpType::Appended => &self.appended_items,
        }
    }

    /// Returns the effective list of items.
    ///
    /// This is equivalent to calling apply_operations on an empty vector.
    pub fn get_applied_items(&self) -> Vec<T> {
        let mut result = Vec::new();
        self.apply_operations(&mut result, None::<fn(ListOpType, &T) -> Option<T>>);
        result
    }

    /// Switch explicit/non-explicit mode, clearing all items if mode changes.
    /// Matches C++ `SdfListOp::_SetExplicit()`.
    fn set_explicit(&mut self, is_explicit: bool) {
        if is_explicit != self.is_explicit {
            self.is_explicit = is_explicit;
            self.explicit_items.clear();
            self.added_items.clear();
            self.prepended_items.clear();
            self.appended_items.clear();
            self.deleted_items.clear();
            self.ordered_items.clear();
        }
    }

    /// Sets the explicit items, removing duplicates (preserves first occurrence).
    ///
    /// Switches to explicit mode (clearing all other lists if transitioning).
    /// Returns Ok(()) if no duplicates, Err with the duplicate item otherwise.
    pub fn set_explicit_items(&mut self, items: Vec<T>) -> Result<(), String> {
        self.set_explicit(true);
        let (unique, duplicate) = Self::remove_duplicates_first(&items);
        self.explicit_items = unique;
        if let Some(dup) = duplicate {
            Err(format!("Duplicate item: {:?}", dup))
        } else {
            Ok(())
        }
    }

    /// Sets the added items.
    ///
    /// Switches to non-explicit mode (clearing all lists if transitioning).
    /// Note: C++ SetAddedItems does NOT deduplicate.
    pub fn set_added_items(&mut self, items: Vec<T>) {
        self.set_explicit(false);
        self.added_items = items;
    }

    /// Sets the prepended items, removing duplicates (preserves first occurrence).
    ///
    /// Switches to non-explicit mode (clearing all lists if transitioning).
    pub fn set_prepended_items(&mut self, items: Vec<T>) -> Result<(), String> {
        self.set_explicit(false);
        let (unique, duplicate) = Self::remove_duplicates_first(&items);
        self.prepended_items = unique;
        if let Some(dup) = duplicate {
            Err(format!("Duplicate item: {:?}", dup))
        } else {
            Ok(())
        }
    }

    /// Sets the appended items, removing duplicates (preserves last occurrence).
    ///
    /// Switches to non-explicit mode (clearing all lists if transitioning).
    pub fn set_appended_items(&mut self, items: Vec<T>) -> Result<(), String> {
        self.set_explicit(false);
        let (unique, duplicate) = Self::remove_duplicates_last(&items);
        self.appended_items = unique;
        if let Some(dup) = duplicate {
            Err(format!("Duplicate item: {:?}", dup))
        } else {
            Ok(())
        }
    }

    /// Sets the deleted items, removing duplicates (preserves first occurrence).
    ///
    /// Switches to non-explicit mode (clearing all lists if transitioning).
    pub fn set_deleted_items(&mut self, items: Vec<T>) -> Result<(), String> {
        self.set_explicit(false);
        let (unique, duplicate) = Self::remove_duplicates_first(&items);
        self.deleted_items = unique;
        if let Some(dup) = duplicate {
            Err(format!("Duplicate item: {:?}", dup))
        } else {
            Ok(())
        }
    }

    /// Sets the ordered items.
    ///
    /// Switches to non-explicit mode (clearing all lists if transitioning).
    /// Note: C++ SetOrderedItems does NOT deduplicate.
    pub fn set_ordered_items(&mut self, items: Vec<T>) {
        self.set_explicit(false);
        self.ordered_items = items;
    }

    // Keep backward-compat wrapper returning Result for ordered
    /// Sets ordered items (Result-returning variant for API compat).
    pub fn set_ordered_items_checked(&mut self, items: Vec<T>) -> Result<(), String> {
        self.set_ordered_items(items);
        Ok(())
    }

    // Keep backward-compat wrapper returning Result for added
    /// Sets added items (Result-returning variant for API compat).
    pub fn set_added_items_checked(&mut self, items: Vec<T>) -> Result<(), String> {
        self.set_added_items(items);
        Ok(())
    }

    /// Sets items for the given operation type.
    pub fn set_items(&mut self, items: Vec<T>, op_type: ListOpType) -> Result<(), String> {
        match op_type {
            ListOpType::Explicit => self.set_explicit_items(items),
            ListOpType::Added => {
                self.set_added_items(items);
                Ok(())
            }
            ListOpType::Deleted => self.set_deleted_items(items),
            ListOpType::Ordered => {
                self.set_ordered_items(items);
                Ok(())
            }
            ListOpType::Prepended => self.set_prepended_items(items),
            ListOpType::Appended => self.set_appended_items(items),
        }
    }

    /// Removes all items and sets the list to non-explicit mode.
    pub fn clear(&mut self) {
        self.is_explicit = false;
        self.explicit_items.clear();
        self.added_items.clear();
        self.prepended_items.clear();
        self.appended_items.clear();
        self.deleted_items.clear();
        self.ordered_items.clear();
    }

    /// Removes all items and sets the list to explicit mode.
    pub fn clear_and_make_explicit(&mut self) {
        self.clear();
        self.is_explicit = true;
    }

    /// Applies edit operations to the given item vector.
    ///
    /// If a callback is provided, it will be called for each item before
    /// it is applied. The callback can transform or filter items.
    ///
    /// Operations are applied in order (matches C++ `_ApplyOperations`):
    /// 1. If explicit, replace the list entirely
    /// 2. Delete items
    /// 3. Add items (legacy)
    /// 4. Prepend items to front
    /// 5. Append items to back
    /// 6. Reorder via ordered items (`_ReorderKeysHelper`)
    pub fn apply_operations<F>(&self, items: &mut Vec<T>, callback: Option<F>)
    where
        F: Fn(ListOpType, &T) -> Option<T>,
    {
        if self.is_explicit {
            // Explicit mode: replace list, dedup via HashSet (O(n) vs O(n^2) contains).
            items.clear();
            // Collect mapped items first (callback may discard some).
            let mut mapped: Vec<T> = Vec::with_capacity(self.explicit_items.len());
            for item in &self.explicit_items {
                if let Some(i) = callback
                    .as_ref()
                    .map_or_else(|| Some(item.clone()), |cb| cb(ListOpType::Explicit, item))
                {
                    mapped.push(i);
                }
            }
            // Dedup while preserving order.
            let mut seen: HashSet<T> = HashSet::with_capacity(mapped.len());
            for i in mapped {
                if seen.insert(i.clone()) {
                    items.push(i);
                }
            }
            return;
        }

        // --- Delete phase: build a set of items to remove (O(1) lookup). ---
        let mut deleted_set: HashSet<T> = HashSet::new();
        for item in &self.deleted_items {
            if let Some(i) = callback
                .as_ref()
                .map_or_else(|| Some(item.clone()), |cb| cb(ListOpType::Deleted, item))
            {
                deleted_set.insert(i);
            }
        }
        if !deleted_set.is_empty() {
            items.retain(|x| !deleted_set.contains(x));
        }

        // --- Build a set of items already present for O(1) membership checks. ---
        let mut present: HashSet<T> = items.iter().cloned().collect();

        // --- Added items (legacy): append if not already present. ---
        for item in &self.added_items {
            if let Some(i) = callback
                .as_ref()
                .map_or_else(|| Some(item.clone()), |cb| cb(ListOpType::Added, item))
            {
                if present.insert(i.clone()) {
                    items.push(i);
                }
            }
        }

        // --- Prepend phase ---
        // Collect unique prepend candidates (last-writer-wins for position).
        let mut prepended: Vec<T> = Vec::new();
        let mut prepend_seen: HashSet<T> = HashSet::new();
        for item in &self.prepended_items {
            if let Some(i) = callback
                .as_ref()
                .map_or_else(|| Some(item.clone()), |cb| cb(ListOpType::Prepended, item))
            {
                if prepend_seen.insert(i.clone()) {
                    prepended.push(i);
                }
            }
        }
        // Remove prepended items from the current list, then prepend.
        if !prepended.is_empty() {
            let prepend_set: HashSet<&T> = prepended.iter().collect();
            items.retain(|x| !prepend_set.contains(x));
            let mut result = prepended;
            result.append(items);
            *items = result;
        }

        // --- Append phase ---
        let mut appended: Vec<T> = Vec::new();
        let mut append_seen: HashSet<T> = HashSet::new();
        for item in &self.appended_items {
            if let Some(i) = callback
                .as_ref()
                .map_or_else(|| Some(item.clone()), |cb| cb(ListOpType::Appended, item))
            {
                if append_seen.insert(i.clone()) {
                    appended.push(i);
                }
            }
        }
        if !appended.is_empty() {
            // Remove appended items from current list (including prepended section).
            let append_set: HashSet<&T> = appended.iter().collect();
            items.retain(|x| !append_set.contains(x));
            items.append(&mut appended);
        }

        // --- Reorder phase (Phase 5 per C++ _ReorderKeys) ---
        // Apply ordered_items to rearrange items in result.
        // This matches C++ listOp.cpp line 437: _ReorderKeys(cb, &result, &search).
        if !self.ordered_items.is_empty() {
            // Apply callback to ordered items (same pattern as other phases).
            let mut mapped_order: Vec<T> = Vec::new();
            let mut order_seen: HashSet<T> = HashSet::new();
            for item in &self.ordered_items {
                if let Some(i) = callback
                    .as_ref()
                    .map_or_else(|| Some(item.clone()), |cb| cb(ListOpType::Ordered, item))
                {
                    // Deduplicate via orderSet (matches C++ step 1 in _ReorderKeysHelper)
                    if order_seen.insert(i.clone()) {
                        mapped_order.push(i);
                    }
                }
            }
            if !mapped_order.is_empty() {
                Self::apply_reorder(&mapped_order, items);
            }
        }
    }

    /// Modifies operations specified in this object.
    ///
    /// `callback` is called for every item in all operation vectors. If the
    /// returned value is `None` then the item is removed, otherwise it's
    /// replaced with the returned value.
    ///
    /// If `callback` returns an item that was previously returned for the
    /// current operation vector being processed, the returned item will be
    /// removed (deduplication).
    ///
    /// Returns true if a change was made, false otherwise.
    ///
    /// Matches C++ `SdfListOp::ModifyOperations`.
    pub fn modify_operations<F>(&mut self, mut callback: F) -> bool
    where
        F: FnMut(&T) -> Option<T>,
    {
        let mut changed = false;

        fn modify_vec<T, F>(vec: &mut Vec<T>, callback: &mut F) -> bool
        where
            T: Clone + Eq + Hash + std::fmt::Debug,
            F: FnMut(&T) -> Option<T>,
        {
            let original_len = vec.len();
            let mut new_items = Vec::new();
            let mut seen = HashSet::new();

            for item in vec.iter() {
                if let Some(new_item) = callback(item) {
                    if seen.insert(new_item.clone()) {
                        new_items.push(new_item);
                    }
                }
            }

            let modified = new_items.len() != original_len
                || new_items.iter().zip(vec.iter()).any(|(a, b)| a != b);

            if modified {
                *vec = new_items;
            }
            modified
        }

        // Matches C++: unconditionally process ALL vectors (not conditional on is_explicit)
        changed |= modify_vec(&mut self.explicit_items, &mut callback);
        changed |= modify_vec(&mut self.added_items, &mut callback);
        changed |= modify_vec(&mut self.prepended_items, &mut callback);
        changed |= modify_vec(&mut self.appended_items, &mut callback);
        changed |= modify_vec(&mut self.deleted_items, &mut callback);
        changed |= modify_vec(&mut self.ordered_items, &mut callback);

        changed
    }

    /// Replaces the items in the specified operation vector in the range
    /// `[index, index + n)` with the given `new_items`. If `new_items` is
    /// empty the items in the range will simply be removed.
    ///
    /// Returns true if the replacement was performed successfully, false
    /// if the index/range is out of bounds.
    ///
    /// Matches C++ `SdfListOp::ReplaceOperations`.
    pub fn replace_operations(
        &mut self,
        op: ListOpType,
        index: usize,
        n: usize,
        new_items: Vec<T>,
    ) -> bool {
        // C++: if op requires mode switch, ignore replace/remove, but allow insert into empty.
        let needs_mode_switch = (self.is_explicit && op != ListOpType::Explicit)
            || (!self.is_explicit && op == ListOpType::Explicit);
        if needs_mode_switch && (n > 0 || new_items.is_empty()) {
            return false;
        }

        // C++: ReplaceOperations calls SetItems() which calls _SetExplicit().
        // _SetExplicit clears ALL lists when switching explicit<->non-explicit mode.
        if needs_mode_switch {
            let new_explicit = op == ListOpType::Explicit;
            // Only clear and switch if mode actually changes (matches C++ _SetExplicit check)
            if new_explicit != self.is_explicit {
                self.is_explicit = new_explicit;
                self.explicit_items.clear();
                self.added_items.clear();
                self.prepended_items.clear();
                self.appended_items.clear();
                self.deleted_items.clear();
                self.ordered_items.clear();
            }
        }

        let vec = match op {
            ListOpType::Explicit => &mut self.explicit_items,
            ListOpType::Added => &mut self.added_items,
            ListOpType::Prepended => &mut self.prepended_items,
            ListOpType::Appended => &mut self.appended_items,
            ListOpType::Deleted => &mut self.deleted_items,
            ListOpType::Ordered => &mut self.ordered_items,
        };

        if index > vec.len() {
            return false;
        }
        if index + n > vec.len() {
            return false;
        }

        // Replace range [index..index+n) with new_items
        vec.splice(index..index + n, new_items);

        // Remove duplicates (keep first occurrence for prepend/explicit/delete,
        // keep last for append). Note: added/ordered not deduped in C++.
        let deduped = if matches!(op, ListOpType::Appended) {
            Self::remove_duplicates_last(vec).0
        } else if matches!(op, ListOpType::Added | ListOpType::Ordered) {
            vec.drain(..).collect() // no dedup for added/ordered
        } else {
            Self::remove_duplicates_first(vec).0
        };
        *vec = deduped;

        true
    }

    /// Composes a stronger SdfListOp's opinions for a given operation list
    /// over this one.
    ///
    /// Matches C++ `SdfListOp::ComposeOperations(stronger, op)`.
    pub fn compose_operations(&mut self, stronger: &ListOp<T>, op: ListOpType) {
        match op {
            ListOpType::Explicit => {
                if !stronger.explicit_items.is_empty() || stronger.is_explicit {
                    self.explicit_items = stronger.explicit_items.clone();
                }
            }
            ListOpType::Added => {
                Self::compose_add_keys(&stronger.added_items, &mut self.added_items);
            }
            ListOpType::Prepended => {
                Self::compose_prepend_keys(
                    &stronger.prepended_items,
                    &mut self.prepended_items,
                    &stronger.deleted_items,
                );
            }
            ListOpType::Appended => {
                Self::compose_append_keys(
                    &stronger.appended_items,
                    &mut self.appended_items,
                    &stronger.deleted_items,
                );
            }
            ListOpType::Deleted => {
                Self::compose_add_keys(&stronger.deleted_items, &mut self.deleted_items);
            }
            ListOpType::Ordered => {
                if !stronger.ordered_items.is_empty() {
                    Self::compose_add_keys(&stronger.ordered_items, &mut self.ordered_items);
                    Self::apply_reorder(&stronger.ordered_items, &mut self.ordered_items);
                }
            }
        }
    }

    /// Applies edit operations to the given ListOp.
    ///
    /// The result is a ListOp that, when applied to a list, has the same
    /// effect as applying `inner` and then `self` in sequence.
    ///
    /// Returns `None` if the result is not well defined.
    /// The result is well-defined when `inner` and `self` do not use the
    /// 'ordered' or 'added' item lists. In other words, only the explicit,
    /// prepended, appended, and deleted portions of SdfListOp are closed
    /// under composition with apply_operations.
    ///
    /// Matches C++ `SdfListOp::ApplyOperations(const SdfListOp<T>&)`.
    pub fn apply_operations_to_list_op(&self, inner: &ListOp<T>) -> Option<ListOp<T>> {
        // If self is explicit, it replaces the result entirely — inner is irrelevant.
        // Matches C++ `if (IsExplicit()) { return *this; }`.
        if self.is_explicit {
            return Some(self.clone());
        }

        // Self must not use added/ordered items (those aren't composable).
        // C++ checks only `self` here, NOT inner.
        if !self.added_items.is_empty() || !self.ordered_items.is_empty() {
            return None;
        }

        // If inner is explicit, apply self's operations to inner's explicit list
        if inner.is_explicit {
            let mut result_items = inner.explicit_items.clone();
            self.apply_operations(&mut result_items, None::<fn(ListOpType, &T) -> Option<T>>);
            return Some(ListOp::create_explicit(result_items));
        }

        // Both are non-explicit: inner must also not use added/ordered items.
        // C++ checks inner here in the nested if, returning empty optional if not satisfied.
        if !inner.added_items.is_empty() || !inner.ordered_items.is_empty() {
            return None;
        }

        // Apply self's del/pre/app onto inner's del/pre/app.
        // This is C++ SdfListOp::ApplyOperations(inner) exact algorithm.
        let mut del = inner.deleted_items.clone();
        let mut pre = inner.prepended_items.clone();
        let mut app = inner.appended_items.clone();

        // Apply self's deletes: remove from inner pre/app, add to del if not present
        for x in &self.deleted_items {
            pre.retain(|item| item != x);
            app.retain(|item| item != x);
            if !del.contains(x) {
                del.push(x.clone());
            }
        }

        // Apply self's prepends: remove from del/pre/app, then prepend in order
        for x in &self.prepended_items {
            del.retain(|item| item != x);
            pre.retain(|item| item != x);
            app.retain(|item| item != x);
        }
        // Insert self.prepended_items at the front of pre (in order)
        let mut new_pre = self.prepended_items.clone();
        new_pre.extend(pre.drain(..));
        pre = new_pre;

        // Apply self's appends: remove from del/pre/app, then append in order
        for x in &self.appended_items {
            del.retain(|item| item != x);
            pre.retain(|item| item != x);
            app.retain(|item| item != x);
        }
        app.extend(self.appended_items.iter().cloned());

        let mut result = ListOp::new();
        let _ = result.set_deleted_items(del);
        let _ = result.set_prepended_items(pre);
        let _ = result.set_appended_items(app);
        Some(result)
    }

    /// Composes this list op with a stronger list op.
    ///
    /// The stronger op's opinions take precedence.
    /// Matches C++ `SdfListOp::ComposeOperations` per-operation semantics.
    pub fn compose_stronger(&mut self, stronger: &ListOp<T>) {
        if stronger.is_explicit {
            self.is_explicit = true;
            self.explicit_items = stronger.explicit_items.clone();
            self.added_items.clear();
            self.prepended_items.clear();
            self.appended_items.clear();
            self.deleted_items.clear();
            self.ordered_items.clear();
            return;
        }

        // Compose each operation type per C++ ComposeOperations logic

        // Explicit: just replace
        if !stronger.explicit_items.is_empty() {
            self.explicit_items = stronger.explicit_items.clone();
        }

        // Added: merge (stronger adds go into weaker's added list)
        Self::compose_add_keys(&stronger.added_items, &mut self.added_items);

        // Deleted: merge (stronger deletes go into weaker's deleted list)
        Self::compose_add_keys(&stronger.deleted_items, &mut self.deleted_items);

        // Prepended: stronger prepended items go first, weaker's follow
        // (excluding items deleted by stronger)
        Self::compose_prepend_keys(
            &stronger.prepended_items,
            &mut self.prepended_items,
            &stronger.deleted_items,
        );

        // Appended: weaker's appended (minus stronger's deletes and stronger's appended)
        // followed by stronger's appended
        Self::compose_append_keys(
            &stronger.appended_items,
            &mut self.appended_items,
            &stronger.deleted_items,
        );

        // Ordered: merge stronger's ordered items into weaker's,
        // then apply reordering. This matches C++ ComposeOperations
        // for SdfListOpTypeOrdered which calls _AddKeys then _ReorderKeys.
        if !stronger.ordered_items.is_empty() {
            Self::compose_add_keys(&stronger.ordered_items, &mut self.ordered_items);
            Self::apply_reorder(&stronger.ordered_items, &mut self.ordered_items);
        }
    }

    /// Adds items from `stronger` into `weaker` list, appending any that are new.
    /// Matches C++ `_AddKeys`.
    fn compose_add_keys(stronger: &[T], weaker: &mut Vec<T>) {
        for item in stronger {
            if !weaker.contains(item) {
                weaker.push(item.clone());
            }
        }
    }

    /// Composes prepended items: stronger items go first, then weaker items
    /// that aren't in stronger's delete list or already present.
    /// Matches C++ `_PrependKeys`.
    fn compose_prepend_keys(stronger: &[T], weaker: &mut Vec<T>, stronger_deleted: &[T]) {
        let mut new_list = Vec::new();
        let mut seen = HashSet::new();

        // Stronger prepended first
        for item in stronger {
            if seen.insert(item.clone()) {
                new_list.push(item.clone());
            }
        }

        // Then weaker prepended (not deleted by stronger, not already present)
        for item in weaker.iter() {
            if !stronger_deleted.contains(item) && seen.insert(item.clone()) {
                new_list.push(item.clone());
            }
        }

        *weaker = new_list;
    }

    /// Composes appended items: weaker items first (minus deleted/conflicting),
    /// then stronger appended items.
    /// Matches C++ `_AppendKeys`.
    fn compose_append_keys(stronger: &[T], weaker: &mut Vec<T>, stronger_deleted: &[T]) {
        let mut new_list = Vec::new();
        let mut seen = HashSet::new();

        // Weaker appended first (not in stronger deleted or stronger appended)
        for item in weaker.iter() {
            if !stronger_deleted.contains(item)
                && !stronger.contains(item)
                && seen.insert(item.clone())
            {
                new_list.push(item.clone());
            }
        }

        // Stronger appended last
        for item in stronger {
            if seen.insert(item.clone()) {
                new_list.push(item.clone());
            }
        }

        *weaker = new_list;
    }

    /// Applies reordering from an `order` vector to the `items` list.
    ///
    /// Implements the C++ `_ReorderKeysHelper` segment-splice algorithm:
    /// 1. Deduplicate `order` (first occurrence wins).
    /// 2. Move `items` into a scratch buffer (items becomes empty).
    /// 3. For each ordered item found in scratch, splice its "segment" —
    ///    the ordered item plus all immediately following non-ordered items
    ///    (up to the next ordered item or end) — to the end of result.
    /// 4. Prepend any remaining scratch items (those before all ordered items)
    ///    to the front of result.
    ///
    /// Non-ordered items "stick" to the ordered item that precedes them in
    /// the original list. Items before all ordered items stay at the front.
    ///
    /// Matches C++ `_ReorderKeysHelper` exactly.
    fn apply_reorder(order: &[T], items: &mut Vec<T>) {
        if order.is_empty() || items.is_empty() {
            return;
        }

        // Step 1: Deduplicate order (first occurrence wins, matches C++ orderSet logic).
        let mut unique_order: Vec<T> = Vec::new();
        let mut order_set: HashSet<T> = HashSet::new();
        for item in order {
            if order_set.insert(item.clone()) {
                unique_order.push(item.clone());
            }
        }
        if unique_order.is_empty() {
            return;
        }

        // Step 2: Move items into scratch; result starts empty.
        // We operate on index positions rather than a true linked list.
        // `used[i]` tracks whether items[i] has been spliced into result.
        let scratch = std::mem::take(items);
        let n = scratch.len();
        let mut used = vec![false; n];

        // Build position map: item -> first index in scratch (O(n) via scan).
        // We need this to locate each ordered item in scratch.
        // Note: if an item appears multiple times, we use the first occurrence
        // (matching C++ std::map which also stores one iterator per key).
        let mut pos_map: std::collections::HashMap<&T, usize> =
            std::collections::HashMap::with_capacity(n);
        for (i, item) in scratch.iter().enumerate() {
            pos_map.entry(item).or_insert(i);
        }

        // Step 3: For each ordered item, collect its segment into result.
        // A segment starts at the ordered item and includes all consecutive
        // non-ordered items following it, until the next ordered item or end.
        let mut result: Vec<T> = Vec::with_capacity(n);

        for oi in &unique_order {
            let start = match pos_map.get(oi) {
                Some(&idx) if !used[idx] => idx,
                _ => continue, // ordered item not in scratch or already spliced
            };

            // Mark the ordered item itself as used.
            used[start] = true;
            result.push(scratch[start].clone());

            // Collect trailing non-ordered items from start+1 forward,
            // skipping already-used slots, stopping at the next ordered item.
            for k in (start + 1)..n {
                if used[k] {
                    continue;
                }
                if order_set.contains(&scratch[k]) {
                    // Hit the next ordered item — stop segment here.
                    break;
                }
                used[k] = true;
                result.push(scratch[k].clone());
            }
        }

        // Step 4: Collect remaining (unused) scratch items — these are items
        // that appeared before all ordered items in the original list.
        // They go at the FRONT of the final result (C++ splices scratch to begin).
        let mut prefix: Vec<T> = Vec::new();
        for (i, item) in scratch.iter().enumerate() {
            if !used[i] {
                prefix.push(item.clone());
            }
        }

        // Final result: prefix + ordered segments.
        *items = prefix;
        items.extend(result);
    }

    /// Remove duplicates preserving first occurrence.
    fn remove_duplicates_first(items: &[T]) -> (Vec<T>, Option<T>) {
        let mut seen = HashSet::new();
        let mut result = Vec::new();
        let mut duplicate = None;

        for item in items {
            if seen.contains(item) {
                if duplicate.is_none() {
                    duplicate = Some(item.clone());
                }
            } else {
                seen.insert(item.clone());
                result.push(item.clone());
            }
        }

        (result, duplicate)
    }

    /// Remove duplicates preserving last occurrence.
    fn remove_duplicates_last(items: &[T]) -> (Vec<T>, Option<T>) {
        let mut seen = HashSet::new();
        let mut duplicate = None;

        // Find duplicates
        for item in items {
            if seen.contains(item) && duplicate.is_none() {
                duplicate = Some(item.clone());
            }
            seen.insert(item.clone());
        }

        // Build result keeping last occurrence
        let mut result = Vec::new();
        let len = items.len();
        for (i, item) in items.iter().enumerate().rev() {
            // Check if this is the last occurrence
            let is_last = items[i + 1..len].iter().all(|x| x != item);
            if is_last && !result.contains(item) {
                result.push(item.clone());
            }
        }
        result.reverse();

        (result, duplicate)
    }
}

impl<T> PartialEq for ListOp<T>
where
    T: Clone + Eq + Hash + fmt::Debug,
{
    fn eq(&self, other: &Self) -> bool {
        self.is_explicit == other.is_explicit
            && self.explicit_items == other.explicit_items
            && self.added_items == other.added_items
            && self.prepended_items == other.prepended_items
            && self.appended_items == other.appended_items
            && self.deleted_items == other.deleted_items
            && self.ordered_items == other.ordered_items
    }
}

impl<T> Eq for ListOp<T> where T: Clone + Eq + Hash + fmt::Debug {}

impl<T> Hash for ListOp<T>
where
    T: Clone + Eq + Hash + fmt::Debug,
{
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.is_explicit.hash(state);
        self.explicit_items.hash(state);
        self.added_items.hash(state);
        self.prepended_items.hash(state);
        self.appended_items.hash(state);
        self.deleted_items.hash(state);
        self.ordered_items.hash(state);
    }
}

impl<T> fmt::Debug for ListOp<T>
where
    T: Clone + Eq + Hash + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ListOp")
            .field("is_explicit", &self.is_explicit)
            .field("explicit_items", &self.explicit_items)
            .field("prepended_items", &self.prepended_items)
            .field("appended_items", &self.appended_items)
            .field("deleted_items", &self.deleted_items)
            .finish()
    }
}

// ============================================================================
// Common type aliases
// ============================================================================

/// ListOp for integers.
pub type IntListOp = ListOp<i32>;

/// ListOp for unsigned integers.
pub type UIntListOp = ListOp<u32>;

/// ListOp for 64-bit integers.
pub type Int64ListOp = ListOp<i64>;

/// ListOp for 64-bit unsigned integers.
pub type UInt64ListOp = ListOp<u64>;

/// ListOp for strings.
pub type StringListOp = ListOp<String>;

/// ListOp for tokens (interned strings).
pub type TokenListOp = ListOp<usd_tf::Token>;

/// ListOp for paths.
pub type PathListOp = ListOp<super::Path>;

/// ListOp for references (composition arc).
pub type ReferenceListOp = ListOp<super::Reference>;

/// ListOp for payloads (composition arc).
pub type PayloadListOp = ListOp<super::Payload>;

/// ListOp for unregistered values.
pub type UnregisteredValueListOp = ListOp<String>;

/// Applies a given ordering to a vector of items.
///
/// Items in `order` that are present in `items` are moved to appear in the
/// specified order. Items not in `order` maintain their relative positions.
///
/// Matches C++ `SdfApplyListOrdering`.
pub fn apply_list_ordering<T>(items: &mut Vec<T>, order: &[T])
where
    T: Clone + Eq + Hash + std::fmt::Debug,
{
    ListOp::<T>::apply_reorder(order, items);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let op = ListOp::<i32>::new();
        assert!(!op.is_explicit());
        assert!(!op.has_keys());
    }

    #[test]
    fn test_create_explicit() {
        let op = ListOp::create_explicit(vec![1, 2, 3]);
        assert!(op.is_explicit());
        assert!(op.has_keys());
        assert_eq!(op.get_explicit_items(), &[1, 2, 3]);
    }

    #[test]
    fn test_create() {
        let op = ListOp::create(vec![1], vec![3], vec![2]);
        assert!(!op.is_explicit());
        assert!(op.has_keys());
        assert_eq!(op.get_prepended_items(), &[1]);
        assert_eq!(op.get_appended_items(), &[3]);
        assert_eq!(op.get_deleted_items(), &[2]);
    }

    #[test]
    fn test_has_item() {
        let op = ListOp::create(vec![1], vec![2], vec![3]);
        assert!(op.has_item(&1));
        assert!(op.has_item(&2));
        assert!(op.has_item(&3));
        assert!(!op.has_item(&4));
    }

    #[test]
    fn test_set_explicit_removes_duplicates() {
        let mut op = ListOp::<i32>::new();
        let result = op.set_explicit_items(vec![1, 2, 1, 3]);
        assert!(result.is_err());
        assert_eq!(op.get_explicit_items(), &[1, 2, 3]);
    }

    #[test]
    fn test_set_appended_preserves_last() {
        let mut op = ListOp::<i32>::new();
        let _ = op.set_appended_items(vec![1, 2, 1, 3, 2]);
        // Should preserve last occurrences: 1, 3, 2
        assert_eq!(op.get_appended_items(), &[1, 3, 2]);
    }

    #[test]
    fn test_clear() {
        let mut op = ListOp::create_explicit(vec![1, 2, 3]);
        op.clear();
        assert!(!op.is_explicit());
        assert!(!op.has_keys());
    }

    #[test]
    fn test_clear_and_make_explicit() {
        let mut op = ListOp::create(vec![1], vec![2], vec![3]);
        op.clear_and_make_explicit();
        assert!(op.is_explicit());
        assert!(op.has_keys()); // Explicit mode always has keys
        assert!(op.get_explicit_items().is_empty());
    }

    #[test]
    fn test_apply_explicit() {
        let op = ListOp::create_explicit(vec![1, 2, 3]);
        let mut items = vec![10, 20, 30];
        op.apply_operations(&mut items, None::<fn(ListOpType, &i32) -> Option<i32>>);
        assert_eq!(items, vec![1, 2, 3]);
    }

    #[test]
    fn test_apply_prepend() {
        let mut op = ListOp::<i32>::new();
        let _ = op.set_prepended_items(vec![1, 2]);
        let mut items = vec![3, 4];
        op.apply_operations(&mut items, None::<fn(ListOpType, &i32) -> Option<i32>>);
        assert_eq!(items, vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_apply_append() {
        let mut op = ListOp::<i32>::new();
        let _ = op.set_appended_items(vec![3, 4]);
        let mut items = vec![1, 2];
        op.apply_operations(&mut items, None::<fn(ListOpType, &i32) -> Option<i32>>);
        assert_eq!(items, vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_apply_delete() {
        let mut op = ListOp::<i32>::new();
        let _ = op.set_deleted_items(vec![2, 4]);
        let mut items = vec![1, 2, 3, 4, 5];
        op.apply_operations(&mut items, None::<fn(ListOpType, &i32) -> Option<i32>>);
        assert_eq!(items, vec![1, 3, 5]);
    }

    #[test]
    fn test_apply_combined() {
        let op = ListOp::create(vec![0], vec![6], vec![3]);
        let mut items = vec![1, 2, 3, 4, 5];
        op.apply_operations(&mut items, None::<fn(ListOpType, &i32) -> Option<i32>>);
        assert_eq!(items, vec![0, 1, 2, 4, 5, 6]);
    }

    #[test]
    fn test_get_applied_items() {
        let op = ListOp::create(vec![1, 2], vec![5, 6], vec![]);
        let items = op.get_applied_items();
        assert_eq!(items, vec![1, 2, 5, 6]);
    }

    #[test]
    fn test_equality() {
        let op1 = ListOp::create(vec![1], vec![2], vec![3]);
        let op2 = ListOp::create(vec![1], vec![2], vec![3]);
        let op3 = ListOp::create(vec![1], vec![2], vec![4]);

        assert_eq!(op1, op2);
        assert_ne!(op1, op3);
    }

    #[test]
    fn test_compose_stronger() {
        let mut weaker = ListOp::create(vec![1, 2], vec![5, 6], vec![]);
        let stronger = ListOp::create(vec![0], vec![7], vec![2]);
        weaker.compose_stronger(&stronger);

        // Stronger prepended (0) + weaker prepended minus deleted (1)
        assert_eq!(weaker.get_prepended_items(), &[0, 1]);
        // Weaker appended minus deleted (5, 6) + stronger appended (7)
        assert_eq!(weaker.get_appended_items(), &[5, 6, 7]);
        // Stronger deleted (2)
        assert!(weaker.get_deleted_items().contains(&2));
    }

    #[test]
    fn test_compose_explicit_wins() {
        let mut weaker = ListOp::create(vec![1], vec![2], vec![]);
        let stronger = ListOp::create_explicit(vec![10, 20]);
        weaker.compose_stronger(&stronger);

        assert!(weaker.is_explicit());
        assert_eq!(weaker.get_explicit_items(), &[10, 20]);
        assert!(weaker.get_prepended_items().is_empty());
        assert!(weaker.get_appended_items().is_empty());
    }

    // -------------------------------------------------------------------------
    // apply_operations_to_list_op tests (C++ ApplyOperations(inner) semantics)
    // -------------------------------------------------------------------------

    #[test]
    fn test_apply_to_list_op_self_explicit_wins() {
        // When self is explicit, result = self regardless of inner
        let outer = ListOp::create_explicit(vec![10, 20]);
        let inner = ListOp::create(vec![1], vec![2], vec![3]);
        let result = outer.apply_operations_to_list_op(&inner).unwrap();
        assert!(result.is_explicit());
        assert_eq!(result.get_explicit_items(), &[10, 20]);
    }

    #[test]
    fn test_apply_to_list_op_inner_explicit() {
        // When inner is explicit, apply self's ops to inner's explicit list
        let outer = ListOp::create(vec![0], vec![99], vec![2]);
        let inner = ListOp::create_explicit(vec![1, 2, 3]);
        let result = outer.apply_operations_to_list_op(&inner).unwrap();
        assert!(result.is_explicit());
        // inner explicit = [1,2,3], delete 2 -> [1,3], prepend 0 -> [0,1,3], append 99 -> [0,1,3,99]
        assert_eq!(result.get_explicit_items(), &[0, 1, 3, 99]);
    }

    #[test]
    fn test_apply_to_list_op_both_non_explicit_delete() {
        // self deletes item from inner's prepended list
        let outer = ListOp::create(vec![], vec![], vec![2]);
        let inner = ListOp::create(vec![1, 2, 3], vec![], vec![]);
        let result = outer.apply_operations_to_list_op(&inner).unwrap();
        assert!(!result.is_explicit());
        // inner pre = [1,2,3], outer deletes 2 -> pre=[1,3], del=[2]
        assert_eq!(result.get_prepended_items(), &[1, 3]);
        assert!(result.get_deleted_items().contains(&2));
    }

    #[test]
    fn test_apply_to_list_op_both_non_explicit_prepend() {
        // self prepends an item that is in inner's deleted list
        // C++: prepend removes from del
        let outer = ListOp::create(vec![5], vec![], vec![]);
        let inner = ListOp::create(vec![], vec![], vec![5]);
        let result = outer.apply_operations_to_list_op(&inner).unwrap();
        assert!(!result.is_explicit());
        // inner del = [5], outer prepends 5 -> del removes 5, pre = [5]
        assert_eq!(result.get_prepended_items(), &[5]);
        assert!(!result.get_deleted_items().contains(&5));
    }

    #[test]
    fn test_apply_to_list_op_both_non_explicit_append() {
        // self appends an item, result appended list = inner.app + outer.app
        let outer = ListOp::create(vec![], vec![7], vec![]);
        let inner = ListOp::create(vec![], vec![4, 5], vec![]);
        let result = outer.apply_operations_to_list_op(&inner).unwrap();
        assert_eq!(result.get_appended_items(), &[4, 5, 7]);
    }

    #[test]
    fn test_apply_to_list_op_returns_none_for_added() {
        // If either has added items, result is None
        let mut outer = ListOp::create(vec![], vec![], vec![]);
        outer.set_added_items(vec![1]);
        let inner = ListOp::create(vec![], vec![], vec![]);
        assert!(outer.apply_operations_to_list_op(&inner).is_none());
    }

    #[test]
    fn test_apply_to_list_op_returns_none_for_ordered() {
        let outer = ListOp::create(vec![], vec![], vec![]);
        let mut inner = ListOp::create(vec![], vec![], vec![]);
        inner.set_ordered_items(vec![1, 2]);
        assert!(outer.apply_operations_to_list_op(&inner).is_none());
    }

    // =========================================================================
    // apply_reorder / _ReorderKeysHelper tests
    // =========================================================================

    /// Verify the basic example from ref_sdf_listop.md section 3.2.
    /// scratch=[X,A,Y,B,Z,C,W], order=[C,A,B] -> [X,C,W,A,Y,B,Z]
    #[test]
    fn test_reorder_basic_example_from_doc() {
        // Using integers: X=0, A=1, Y=2, B=3, Z=4, C=5, W=6
        let mut items = vec![0i32, 1, 2, 3, 4, 5, 6]; // X A Y B Z C W
        ListOp::<i32>::apply_reorder(&[5, 1, 3], &mut items); // C A B
        assert_eq!(items, vec![0, 5, 6, 1, 2, 3, 4]); // X C W A Y B Z
    }

    /// Verify the example from bughunt_sdf.md.
    /// items=[X,A,Y,B,Z], order=[B,A] -> [X,B,Z,A,Y]
    #[test]
    fn test_reorder_bughunt_example() {
        // X=10, A=1, Y=20, B=2, Z=30
        let mut items = vec![10i32, 1, 20, 2, 30];
        ListOp::<i32>::apply_reorder(&[2, 1], &mut items);
        assert_eq!(items, vec![10, 2, 30, 1, 20]);
    }

    /// Items before any ordered item go to the front unchanged.
    #[test]
    fn test_reorder_prefix_items_stay_at_front() {
        let mut items = vec![99i32, 100, 1, 2, 3];
        ListOp::<i32>::apply_reorder(&[3, 1], &mut items);
        // ordered: 3 (segment=[3]), 1 (segment=[1,2])
        // prefix (before first ordered item 1): [99, 100]
        assert_eq!(items, vec![99, 100, 3, 1, 2]);
    }

    /// Empty order = no change.
    #[test]
    fn test_reorder_empty_order_noop() {
        let mut items = vec![3i32, 1, 2];
        ListOp::<i32>::apply_reorder(&[], &mut items);
        assert_eq!(items, vec![3, 1, 2]);
    }

    /// Empty items = no change.
    #[test]
    fn test_reorder_empty_items_noop() {
        let mut items: Vec<i32> = vec![];
        ListOp::<i32>::apply_reorder(&[1i32, 2], &mut items);
        assert_eq!(items, Vec::<i32>::new());
    }

    /// Ordered items not present in the list are skipped.
    #[test]
    fn test_reorder_ordered_not_in_items() {
        let mut items = vec![1i32, 2, 3];
        ListOp::<i32>::apply_reorder(&[99, 100], &mut items);
        // None of the ordered items are in the list, so all items stay as prefix.
        assert_eq!(items, vec![1, 2, 3]);
    }

    /// All items are ordered -> they reorder exactly per order vector.
    #[test]
    fn test_reorder_all_items_ordered() {
        let mut items = vec![1i32, 2, 3];
        ListOp::<i32>::apply_reorder(&[3, 1, 2], &mut items);
        assert_eq!(items, vec![3, 1, 2]);
    }

    /// Duplicates in order are deduplicated (first occurrence wins).
    #[test]
    fn test_reorder_duplicate_order_items() {
        let mut items = vec![1i32, 2, 3];
        ListOp::<i32>::apply_reorder(&[2, 1, 2, 3], &mut items); // 2 appears twice
        // unique order = [2, 1, 3], all ordered, no prefix
        assert_eq!(items, vec![2, 1, 3]);
    }

    /// Non-ordered items between two ordered items attach to the one BEFORE them.
    #[test]
    fn test_reorder_non_ordered_attach_to_preceding() {
        // [A, x, y, B, z, C] with order=[C, B, A]
        // A=1, x=10, y=11, B=2, z=20, C=3
        let mut items = vec![1i32, 10, 11, 2, 20, 3];
        ListOp::<i32>::apply_reorder(&[3, 2, 1], &mut items);
        // C segment: [3] (nothing after C before end)
        // B segment: [2, 20] (20 follows B, not ordered)
        // A segment: [1, 10, 11] (10,11 follow A until B which is ordered -> stop)
        // prefix: [] (no items before first ordered item A at pos 0)
        assert_eq!(items, vec![3, 2, 20, 1, 10, 11]);
    }

    /// apply_reorder also used by SdfApplyListOrdering public API.
    #[test]
    fn test_apply_list_ordering_public_api() {
        let mut items = vec![10i32, 1, 20, 2, 30];
        super::apply_list_ordering(&mut items, &[2, 1]);
        assert_eq!(items, vec![10, 2, 30, 1, 20]);
    }

    // =========================================================================
    // apply_operations reorder phase integration tests
    // =========================================================================

    /// apply_operations must apply ordered_items after append phase.
    #[test]
    fn test_apply_operations_reorder_phase() {
        let mut op = ListOp::<i32>::new();
        op.set_ordered_items(vec![3, 1, 2]);
        // Start with items [1, 2, 3]
        let mut items = vec![1i32, 2, 3];
        op.apply_operations(&mut items, None::<fn(ListOpType, &i32) -> Option<i32>>);
        // Ordered = [3, 1, 2], all in items, no non-ordered -> [3, 1, 2]
        assert_eq!(items, vec![3, 1, 2]);
    }

    /// Ordered items combined with prepend and append.
    #[test]
    fn test_apply_operations_reorder_with_prepend_append() {
        let mut op = ListOp::<i32>::new();
        let _ = op.set_prepended_items(vec![10]);
        let _ = op.set_appended_items(vec![20]);
        op.set_ordered_items(vec![20, 10]); // reverse the prepended/appended
        let mut items = vec![5i32];
        op.apply_operations(&mut items, None::<fn(ListOpType, &i32) -> Option<i32>>);
        // After prepend: [10, 5], after append: [10, 5, 20]
        // Reorder [20, 10]: 20 segment=[20], 10 segment=[10, 5], prefix=[]
        assert_eq!(items, vec![20, 10, 5]);
    }

    /// Ordered items with callback transformation.
    #[test]
    fn test_apply_operations_reorder_with_callback() {
        let mut op = ListOp::<i32>::new();
        op.set_ordered_items(vec![2, 1]);
        let mut items = vec![1i32, 2, 3];
        // Callback returns None for ordered item 2 (filter it out of order)
        op.apply_operations(
            &mut items,
            Some(|op_type: ListOpType, item: &i32| {
                if op_type == ListOpType::Ordered && *item == 2 {
                    None // skip this ordered item
                } else {
                    Some(*item)
                }
            }),
        );
        // effective order = [1], segment of 1 = [1], others are prefix = [2, 3]
        // Wait: items=[1,2,3], order after callback=[1]
        // 1 is at pos 0, segment=[1], trailing non-ordered until ordered or end: [2,3] not ordered
        // But 2 is NOT in order_set (filtered out), so it's non-ordered -> [1,2,3] segment
        // prefix = [] (1 is the first item)
        // result = [1,2,3]
        assert_eq!(items, vec![1, 2, 3]);
    }
}
