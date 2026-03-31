//! List editor proxy - modifiable view of list editing operations.
//!
//! `ListEditorProxy` provides access to the different operation lists
//! (explicit, prepended, appended, deleted, ordered) that make up a
//! list editor. Simplified version that doesn't use trait objects.

use std::fmt;
use std::marker::PhantomData;

use super::list_op::ListOpType;
use super::list_proxy::ListProxy;
use super::proxy_policies::TypePolicy;

/// Proxy for list editing operations.
///
/// Simplified version that stores operation lists directly.
pub struct ListEditorProxy<Policy: TypePolicy> {
    /// Explicit list (replaces everything if set).
    explicit: Option<Vec<Policy::Value>>,
    /// Prepended items.
    prepended: Vec<Policy::Value>,
    /// Appended items.
    appended: Vec<Policy::Value>,
    /// Deleted items.
    deleted: Vec<Policy::Value>,
    /// Ordered items.
    ordered: Vec<Policy::Value>,
    /// Type policy marker.
    _policy: PhantomData<Policy>,
}

impl<Policy: TypePolicy> ListEditorProxy<Policy> {
    /// Creates a new empty list editor proxy.
    pub fn new() -> Self {
        Self {
            explicit: None,
            prepended: Vec::new(),
            appended: Vec::new(),
            deleted: Vec::new(),
            ordered: Vec::new(),
            _policy: PhantomData,
        }
    }

    /// Creates from explicit items.
    pub fn with_explicit(items: Vec<Policy::Value>) -> Self {
        Self {
            explicit: Some(items),
            prepended: Vec::new(),
            appended: Vec::new(),
            deleted: Vec::new(),
            ordered: Vec::new(),
            _policy: PhantomData,
        }
    }

    /// Returns true if the editor has expired.
    pub fn is_expired(&self) -> bool {
        false // Simplified
    }

    /// Returns true if the editor is in explicit mode.
    pub fn is_explicit(&self) -> bool {
        self.explicit.is_some()
    }

    /// Returns true if the editor only allows ordering operations.
    pub fn is_ordered_only(&self) -> bool {
        self.explicit.is_none()
            && self.prepended.is_empty()
            && self.appended.is_empty()
            && self.deleted.is_empty()
            && !self.ordered.is_empty()
    }

    /// Returns true if the editor has any keys.
    pub fn has_keys(&self) -> bool {
        !self.is_empty()
    }

    /// Returns true if the editor is empty.
    pub fn is_empty(&self) -> bool {
        match &self.explicit {
            Some(list) => list.is_empty(),
            None => {
                self.prepended.is_empty()
                    && self.appended.is_empty()
                    && self.deleted.is_empty()
                    && self.ordered.is_empty()
            }
        }
    }

    /// Gets a proxy to the explicit items list.
    pub fn get_explicit_items(&self) -> ListProxy<Policy> {
        match &self.explicit {
            Some(items) => ListProxy::from_vec(items.clone()),
            None => ListProxy::new(),
        }
    }

    /// Gets a proxy to the prepended items list.
    pub fn get_prepended_items(&self) -> ListProxy<Policy> {
        ListProxy::from_vec(self.prepended.clone())
    }

    /// Gets a proxy to the appended items list.
    pub fn get_appended_items(&self) -> ListProxy<Policy> {
        ListProxy::from_vec(self.appended.clone())
    }

    /// Gets a proxy to the deleted items list.
    pub fn get_deleted_items(&self) -> ListProxy<Policy> {
        ListProxy::from_vec(self.deleted.clone())
    }

    /// Gets a proxy to the ordered items list.
    pub fn get_ordered_items(&self) -> ListProxy<Policy> {
        ListProxy::from_vec(self.ordered.clone())
    }

    /// Applies the edits to a vector (simplified version).
    pub fn apply_edits_to_list(&self, vec: &mut Vec<Policy::Value>)
    where
        Policy::Value: Clone + PartialEq,
    {
        // Simplified application logic
        if let Some(explicit) = &self.explicit {
            *vec = explicit.clone();
        } else {
            // Apply prepend/append/delete operations
            for item in self.deleted.iter() {
                vec.retain(|x| x != item);
            }
            let mut result = self.prepended.clone();
            result.append(vec);
            result.extend(self.appended.clone());
            *vec = result;
        }
    }

    /// Clears all edits and changes to list operations mode.
    pub fn clear_edits(&mut self) -> bool {
        self.explicit = None;
        self.prepended.clear();
        self.appended.clear();
        self.deleted.clear();
        self.ordered.clear();
        true
    }

    /// Clears all edits and changes to explicit mode.
    pub fn clear_edits_and_make_explicit(&mut self) -> bool {
        self.explicit = Some(Vec::new());
        self.prepended.clear();
        self.appended.clear();
        self.deleted.clear();
        self.ordered.clear();
        true
    }

    /// Returns the size of the explicit list.
    pub fn size(&self) -> usize {
        match &self.explicit {
            Some(items) => items.len(),
            None => 0,
        }
    }

    /// Counts the total number of items across all operation lists.
    pub fn count(&self) -> usize {
        match &self.explicit {
            Some(items) => items.len(),
            None => {
                self.prepended.len() + self.appended.len() + self.deleted.len() + self.ordered.len()
            }
        }
    }
}

impl<Policy: TypePolicy> Default for ListEditorProxy<Policy> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Policy: TypePolicy> Clone for ListEditorProxy<Policy> {
    fn clone(&self) -> Self {
        Self {
            explicit: self.explicit.clone(),
            prepended: self.prepended.clone(),
            appended: self.appended.clone(),
            deleted: self.deleted.clone(),
            ordered: self.ordered.clone(),
            _policy: PhantomData,
        }
    }
}

impl<Policy: TypePolicy> fmt::Debug for ListEditorProxy<Policy> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ListEditorProxy")
            .field("explicit", &self.is_explicit())
            .field("ordered_only", &self.is_ordered_only())
            .field("has_keys", &self.has_keys())
            .finish()
    }
}

// ============================================================================
// Convenience functions
// ============================================================================

/// Callback type for applying edits.
pub type ApplyCallback<V> = Box<dyn Fn(ListOpType, &V) -> Option<V>>;

/// Callback type for modifying edits.
pub type ModifyCallback<V> = Box<dyn Fn(&V) -> Option<V>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_editor_proxy_empty() {
        let proxy: ListEditorProxy<super::super::proxy_policies::StringTypePolicy> =
            ListEditorProxy::new();
        assert!(proxy.is_empty());
        assert!(!proxy.is_explicit());
    }
}
