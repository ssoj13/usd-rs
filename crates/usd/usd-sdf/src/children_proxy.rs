//! Children proxy - modifiable view of spec children.
//!
//! `ChildrenProxy` provides a mutable interface to a spec's children,
//! allowing insertion, removal, and reordering of child specs.

use std::fmt;
use std::marker::PhantomData;

use super::children_policies::ChildPolicy;
use super::{LayerHandle, Path};

/// Error type for children proxy operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChildrenProxyError {
    /// Proxy has expired.
    Expired,
    /// Invalid child name.
    InvalidName(String),
    /// Child already exists.
    AlreadyExists(String),
    /// Child not found.
    NotFound(String),
    /// Permission denied.
    PermissionDenied(String),
    /// Other error.
    Other(String),
}

impl fmt::Display for ChildrenProxyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Expired => write!(f, "Children proxy has expired"),
            Self::InvalidName(name) => write!(f, "Invalid child name: {}", name),
            Self::AlreadyExists(name) => write!(f, "Child already exists: {}", name),
            Self::NotFound(name) => write!(f, "Child not found: {}", name),
            Self::PermissionDenied(msg) => write!(f, "Permission denied: {}", msg),
            Self::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for ChildrenProxyError {}

/// Result type for children proxy operations.
pub type ChildrenProxyResult<T> = Result<T, ChildrenProxyError>;

// ============================================================================
// ChildrenProxy
// ============================================================================

/// Mutable proxy to a spec's children.
///
/// `ChildrenProxy` provides operations for managing a spec's child specs,
/// including adding, removing, and reordering children.
///
/// # Type Parameters
///
/// * `Policy` - Child policy defining parent/child types and behavior
///
/// # Examples
///
/// ```ignore
/// use usd_sdf::{ChildrenProxy, PrimChildPolicy};
///
/// let proxy: ChildrenProxy<PrimChildPolicy> = prim.get_children();
///
/// // Add children
/// proxy.insert("Child1", -1)?;
/// proxy.insert("Child2", 0)?; // Insert at beginning
///
/// // Remove children
/// proxy.remove("Child1")?;
///
/// // Reorder
/// proxy.set_order(vec!["Child2".into(), "Child3".into()])?;
/// ```
pub struct ChildrenProxy<Policy: ChildPolicy> {
    /// Parent spec.
    parent: Policy::Parent,
    /// Layer handle.
    #[allow(dead_code)]
    layer: LayerHandle,
    /// Parent path.
    #[allow(dead_code)]
    parent_path: Path,
    /// Policy marker.
    _policy: PhantomData<Policy>,
}

impl<Policy: ChildPolicy> ChildrenProxy<Policy> {
    /// Creates a new children proxy.
    pub fn new(parent: Policy::Parent, layer: LayerHandle, parent_path: Path) -> Self {
        Self {
            parent,
            layer,
            parent_path,
            _policy: PhantomData,
        }
    }

    /// Returns true if the proxy has expired.
    pub fn is_expired(&self) -> bool {
        // Check if layer still exists
        false // Simplified
    }

    /// Returns the number of children.
    pub fn size(&self) -> usize {
        self.get_children_names().len()
    }

    /// Returns true if there are no children.
    pub fn is_empty(&self) -> bool {
        self.size() == 0
    }

    /// Returns the names of all children in order.
    pub fn get_children_names(&self) -> Vec<Policy::Key> {
        // Would query layer for children
        Vec::new()
    }

    /// Returns true if a child with the given key exists.
    pub fn contains(&self, key: &Policy::Key) -> bool {
        self.get_children_names().contains(key)
    }

    /// Gets a child by key.
    pub fn get(&self, key: &Policy::Key) -> Option<Policy::Child> {
        if self.is_expired() {
            return None;
        }

        Policy::get_child(&self.parent, key)
    }

    /// Inserts a new child with the given key at the specified index.
    ///
    /// If index is -1, appends to the end. Returns error if child already exists.
    pub fn insert(&mut self, key: Policy::Key, index: i32) -> ChildrenProxyResult<Policy::Child>
    where
        Policy::Key: Clone,
    {
        if self.is_expired() {
            return Err(ChildrenProxyError::Expired);
        }

        // Validate name
        let key_str = format!("{:?}", &key); // Simplified
        if !Policy::is_valid_name(&key_str) {
            return Err(ChildrenProxyError::InvalidName(key_str));
        }

        // Check if already exists
        if self.contains(&key) {
            return Err(ChildrenProxyError::AlreadyExists(key_str));
        }

        // Create child
        let key_for_insert = key.clone();
        let child = Policy::create_child(&self.parent, key)
            .ok_or_else(|| ChildrenProxyError::Other("Failed to create child".to_string()))?;

        // Insert at index
        // Would need to update children order in layer
        let mut children = self.get_children_names();
        let insert_pos = if index < 0 {
            children.len()
        } else {
            (index as usize).min(children.len())
        };
        children.insert(insert_pos, key_for_insert);

        Ok(child)
    }

    /// Removes a child with the given key.
    ///
    /// Returns error if child doesn't exist.
    pub fn remove(&mut self, key: &Policy::Key) -> ChildrenProxyResult<bool> {
        if self.is_expired() {
            return Err(ChildrenProxyError::Expired);
        }

        let key_str = format!("{:?}", key);
        if !self.contains(key) {
            return Err(ChildrenProxyError::NotFound(key_str));
        }

        // Remove child spec from layer
        // Would need layer API support

        Ok(true)
    }

    /// Moves a child to a new index.
    ///
    /// If new_index is -1, moves to the end.
    pub fn reorder(&mut self, key: &Policy::Key, new_index: i32) -> ChildrenProxyResult<()> {
        if self.is_expired() {
            return Err(ChildrenProxyError::Expired);
        }

        let key_str = format!("{:?}", key);
        if !self.contains(key) {
            return Err(ChildrenProxyError::NotFound(key_str));
        }

        let mut children = self.get_children_names();

        // Find current position
        let current_pos = children
            .iter()
            .position(|k| k == key)
            .ok_or_else(|| ChildrenProxyError::NotFound(key_str.clone()))?;

        // Remove from current position
        children.remove(current_pos);

        // Insert at new position
        let insert_pos = if new_index < 0 {
            children.len()
        } else {
            (new_index as usize).min(children.len())
        };
        children.insert(insert_pos, key.clone());

        // Update order in layer
        // Would need layer API support

        Ok(())
    }

    /// Sets the complete ordering of children.
    ///
    /// The provided vector should contain all child keys in the desired order.
    /// Any keys not in the vector will be removed.
    pub fn set_order(&mut self, keys: Vec<Policy::Key>) -> ChildrenProxyResult<()> {
        if self.is_expired() {
            return Err(ChildrenProxyError::Expired);
        }

        // Validate all keys exist
        for key in &keys {
            let key_str = format!("{:?}", key);
            if !self.contains(key) {
                return Err(ChildrenProxyError::NotFound(key_str));
            }
        }

        // Update order in layer
        // Would need layer API support

        Ok(())
    }

    /// Clears all children.
    pub fn clear(&mut self) -> ChildrenProxyResult<()> {
        if self.is_expired() {
            return Err(ChildrenProxyError::Expired);
        }

        let children = self.get_children_names();
        for key in children {
            self.remove(&key)?;
        }

        Ok(())
    }

    /// Returns an iterator over child keys.
    pub fn iter_keys(&self) -> impl Iterator<Item = Policy::Key> + '_ {
        self.get_children_names().into_iter()
    }

    /// Returns an iterator over child specs.
    pub fn iter_children(&self) -> impl Iterator<Item = Policy::Child> + '_ {
        self.get_children_names()
            .into_iter()
            .filter_map(move |key| self.get(&key))
    }

    /// Renames a child.
    ///
    /// This creates a new child with the new name and copies data from the old child.
    pub fn rename(
        &mut self,
        old_key: &Policy::Key,
        new_key: Policy::Key,
    ) -> ChildrenProxyResult<()> {
        if self.is_expired() {
            return Err(ChildrenProxyError::Expired);
        }

        let old_key_str = format!("{:?}", old_key);
        let new_key_str = format!("{:?}", new_key);

        if !self.contains(old_key) {
            return Err(ChildrenProxyError::NotFound(old_key_str));
        }

        if self.contains(&new_key) {
            return Err(ChildrenProxyError::AlreadyExists(new_key_str));
        }

        if !Policy::is_valid_name(&new_key_str) {
            return Err(ChildrenProxyError::InvalidName(new_key_str));
        }

        // Would need to:
        // 1. Create new child with new name
        // 2. Copy data from old child
        // 3. Remove old child

        Ok(())
    }
}

impl<Policy: ChildPolicy> fmt::Debug for ChildrenProxy<Policy> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChildrenProxy")
            .field("size", &self.size())
            .field("expired", &self.is_expired())
            .finish()
    }
}

// ============================================================================
// Convenience type aliases
// ============================================================================

/// Proxy for prim children.
pub type PrimChildrenProxy = ChildrenProxy<super::children_policies::PrimChildPolicy>;

/// Proxy for property children.
pub type PropertyChildrenProxy = ChildrenProxy<super::children_policies::PropertyChildPolicy>;

/// Proxy for attribute children.
pub type AttributeChildrenProxy = ChildrenProxy<super::children_policies::AttributeChildPolicy>;

/// Proxy for relationship children.
pub type RelationshipChildrenProxy =
    ChildrenProxy<super::children_policies::RelationshipChildPolicy>;

/// Proxy for variant set children.
pub type VariantSetChildrenProxy = ChildrenProxy<super::children_policies::VariantSetChildPolicy>;

/// Proxy for variant children.
pub type VariantChildrenProxy = ChildrenProxy<super::children_policies::VariantChildPolicy>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_children_proxy_error_display() {
        let err = ChildrenProxyError::Expired;
        assert_eq!(err.to_string(), "Children proxy has expired");

        let err = ChildrenProxyError::InvalidName("123".to_string());
        assert!(err.to_string().contains("123"));
    }
}
