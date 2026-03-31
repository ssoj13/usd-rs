//! List proxy - modifiable view of a list field in a spec.
//!
//! `ListProxy` provides a mutable interface to a list stored in a spec,
//! similar to a vector. Changes made through the proxy are immediately
//! reflected in the underlying layer data.
//!
//! This is a simplified version that doesn't use trait objects to avoid
//! dyn compatibility issues.

use std::fmt;
use std::marker::PhantomData;

use super::proxy_policies::TypePolicy;

/// Result type for list operations.
pub type ListProxyResult<T> = Result<T, ListProxyError>;

/// Error type for list proxy operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListProxyError {
    /// Proxy has expired (underlying spec no longer exists).
    Expired,
    /// Index out of bounds.
    IndexOutOfBounds {
        /// The requested index.
        index: usize,
        /// The list size.
        size: usize,
    },
    /// Invalid value.
    InvalidValue(String),
    /// Permission denied.
    PermissionDenied(String),
    /// Other error.
    Other(String),
}

impl fmt::Display for ListProxyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Expired => write!(f, "List proxy has expired"),
            Self::IndexOutOfBounds { index, size } => {
                write!(f, "Index {} out of bounds (size: {})", index, size)
            }
            Self::InvalidValue(msg) => write!(f, "Invalid value: {}", msg),
            Self::PermissionDenied(msg) => write!(f, "Permission denied: {}", msg),
            Self::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for ListProxyError {}

/// Invalid index constant (similar to C++ SdfListProxy::invalidIndex).
pub const INVALID_INDEX: usize = usize::MAX;

// ============================================================================
// ListProxy - Simplified version
// ============================================================================

/// Mutable proxy to a list field in a spec.
///
/// This is a simplified version that stores the list directly rather than
/// using a trait object editor, to avoid dyn compatibility issues.
///
/// # Type Parameters
///
/// * `Policy` - Type policy defining the value type and conversions
pub struct ListProxy<Policy: TypePolicy> {
    /// The list items.
    items: Vec<Policy::Value>,
    /// Type policy marker.
    _policy: PhantomData<Policy>,
}

impl<Policy: TypePolicy> ListProxy<Policy> {
    /// Creates a new empty list proxy.
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            _policy: PhantomData,
        }
    }

    /// Creates a list proxy from a vector.
    pub fn from_vec(items: Vec<Policy::Value>) -> Self {
        Self {
            items,
            _policy: PhantomData,
        }
    }

    /// Returns true if the proxy is expired.
    pub fn is_expired(&self) -> bool {
        false // Simplified - no expiration
    }

    /// Returns the number of items in the list.
    pub fn size(&self) -> usize {
        self.items.len()
    }

    /// Returns true if the list is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Gets the item at the given index.
    pub fn get(&self, index: usize) -> ListProxyResult<Policy::Value> {
        self.items
            .get(index)
            .cloned()
            .ok_or_else(|| ListProxyError::IndexOutOfBounds {
                index,
                size: self.size(),
            })
    }

    /// Sets the item at the given index.
    pub fn set(&mut self, index: usize, value: Policy::Value) -> ListProxyResult<()> {
        if index >= self.size() {
            return Err(ListProxyError::IndexOutOfBounds {
                index,
                size: self.size(),
            });
        }

        if !Policy::is_valid(&value) {
            return Err(ListProxyError::InvalidValue(
                "Value failed validation".to_string(),
            ));
        }

        self.items[index] = value;
        Ok(())
    }

    /// Appends an item to the end of the list.
    pub fn push(&mut self, value: Policy::Value) -> ListProxyResult<()> {
        if !Policy::is_valid(&value) {
            return Err(ListProxyError::InvalidValue(
                "Value failed validation".to_string(),
            ));
        }

        self.items.push(value);
        Ok(())
    }

    /// Removes and returns the last item from the list.
    pub fn pop(&mut self) -> ListProxyResult<Policy::Value> {
        self.items
            .pop()
            .ok_or(ListProxyError::IndexOutOfBounds { index: 0, size: 0 })
    }

    /// Inserts an item at the given index.
    pub fn insert(&mut self, index: usize, value: Policy::Value) -> ListProxyResult<()> {
        if index > self.size() {
            return Err(ListProxyError::IndexOutOfBounds {
                index,
                size: self.size(),
            });
        }

        if !Policy::is_valid(&value) {
            return Err(ListProxyError::InvalidValue(
                "Value failed validation".to_string(),
            ));
        }

        self.items.insert(index, value);
        Ok(())
    }

    /// Removes the item at the given index.
    pub fn remove(&mut self, index: usize) -> ListProxyResult<Policy::Value> {
        if index >= self.size() {
            return Err(ListProxyError::IndexOutOfBounds {
                index,
                size: self.size(),
            });
        }

        Ok(self.items.remove(index))
    }

    /// Clears all items from the list.
    pub fn clear(&mut self) -> ListProxyResult<()> {
        self.items.clear();
        Ok(())
    }

    /// Finds the first occurrence of a value in the list.
    pub fn find(&self, value: &Policy::Value) -> usize
    where
        Policy::Value: PartialEq,
    {
        self.items
            .iter()
            .position(|item| item == value)
            .unwrap_or(INVALID_INDEX)
    }

    /// Returns true if the list contains the given value.
    pub fn contains(&self, value: &Policy::Value) -> bool
    where
        Policy::Value: PartialEq,
    {
        self.find(value) != INVALID_INDEX
    }

    /// Returns an iterator over the list items.
    pub fn iter(&self) -> impl Iterator<Item = &Policy::Value> {
        self.items.iter()
    }

    /// Collects all items into a vector.
    pub fn to_vec(&self) -> Vec<Policy::Value> {
        self.items.clone()
    }

    /// Returns a reference to the underlying vector.
    pub fn as_slice(&self) -> &[Policy::Value] {
        &self.items
    }
}

impl<Policy: TypePolicy> Default for ListProxy<Policy> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Policy: TypePolicy> Clone for ListProxy<Policy> {
    fn clone(&self) -> Self {
        Self {
            items: self.items.clone(),
            _policy: PhantomData,
        }
    }
}

impl<Policy: TypePolicy> fmt::Debug for ListProxy<Policy> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ListProxy")
            .field("size", &self.size())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_index_constant() {
        assert_eq!(INVALID_INDEX, usize::MAX);
    }

    #[test]
    fn test_list_proxy_error_display() {
        let err = ListProxyError::Expired;
        assert_eq!(err.to_string(), "List proxy has expired");

        let err = ListProxyError::IndexOutOfBounds { index: 5, size: 3 };
        assert!(err.to_string().contains("5"));
        assert!(err.to_string().contains("3"));
    }
}
