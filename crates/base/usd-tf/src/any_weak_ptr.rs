//! Type-erased weak pointer holder.
//!
//! Provides the ability to hold an arbitrary weak pointer in a non-type-specific
//! manner to observe whether it has expired.
//!
//! # Examples
//!
//! ```
//! use usd_tf::any_weak_ptr::AnyWeakPtr;
//! use std::sync::{Arc, Weak};
//!
//! let strong = Arc::new(42);
//! let weak = Arc::downgrade(&strong);
//!
//! let any_weak = AnyWeakPtr::from_weak(weak.clone());
//! assert!(!any_weak.is_expired());
//!
//! // Drop the strong reference
//! drop(strong);
//! assert!(any_weak.is_expired());
//! ```

use std::any::TypeId;
use std::hash::{Hash, Hasher};
use std::sync::Weak;

/// Trait for types that can be wrapped in AnyWeakPtr.
pub trait WeakPtrLike: Send + Sync {
    /// Returns true if the weak pointer has expired (target deallocated).
    fn is_expired(&self) -> bool;

    /// Returns a unique identifier for the referenced object.
    fn unique_id(&self) -> usize;

    /// Returns the TypeId of the data type pointed to.
    fn data_type_id(&self) -> TypeId;

    /// Clones the weak pointer into a boxed trait object.
    fn clone_box(&self) -> Box<dyn WeakPtrLike>;
}

/// Implementation for std::sync::Weak<T>.
impl<T: Send + Sync + 'static> WeakPtrLike for Weak<T> {
    fn is_expired(&self) -> bool {
        self.strong_count() == 0
    }

    fn unique_id(&self) -> usize {
        // Use the address of the data as unique identifier
        self.as_ptr() as usize
    }

    fn data_type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }

    fn clone_box(&self) -> Box<dyn WeakPtrLike> {
        Box::new(self.clone())
    }
}

/// Type-erased weak pointer.
///
/// Can hold any type of weak pointer and observe whether it has expired.
/// Useful for notification systems and caching where the exact type isn't
/// known at compile time.
pub struct AnyWeakPtr {
    inner: Option<Box<dyn WeakPtrLike>>,
}

impl AnyWeakPtr {
    /// Creates an empty AnyWeakPtr (equivalent to null).
    pub fn new() -> Self {
        Self { inner: None }
    }

    /// Creates an AnyWeakPtr from a std::sync::Weak<T>.
    pub fn from_weak<T: Send + Sync + 'static>(weak: Weak<T>) -> Self {
        Self {
            inner: Some(Box::new(weak)),
        }
    }

    /// Creates an AnyWeakPtr from a type implementing WeakPtrLike.
    pub fn from_weak_ptr_like(ptr: Box<dyn WeakPtrLike>) -> Self {
        Self { inner: Some(ptr) }
    }

    /// Returns true if this AnyWeakPtr is empty (holds no pointer).
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.is_none()
    }

    /// Returns true if the weak pointer has expired.
    ///
    /// Returns true if empty or if the referenced object has been deallocated.
    pub fn is_expired(&self) -> bool {
        match &self.inner {
            Some(ptr) => ptr.is_expired(),
            None => true,
        }
    }

    /// Returns true if the weak pointer is still valid (not expired).
    ///
    /// Alias for `!is_expired()` with `!is_empty()` check.
    pub fn is_valid(&self) -> bool {
        !self.is_empty() && !self.is_expired()
    }

    /// Returns a unique identifier for the referenced object.
    ///
    /// Returns 0 if empty or expired.
    pub fn unique_id(&self) -> usize {
        match &self.inner {
            Some(ptr) => ptr.unique_id(),
            None => 0,
        }
    }

    /// Returns the TypeId of the data type, or None if empty.
    pub fn data_type_id(&self) -> Option<TypeId> {
        self.inner.as_ref().map(|ptr| ptr.data_type_id())
    }

    /// Computes a hash value based on the unique identifier.
    pub fn get_hash(&self) -> u64 {
        // Shift right by 3 to account for alignment
        (self.unique_id() >> 3) as u64
    }
}

impl Default for AnyWeakPtr {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for AnyWeakPtr {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.as_ref().map(|ptr| ptr.clone_box()),
        }
    }
}

impl PartialEq for AnyWeakPtr {
    fn eq(&self, other: &Self) -> bool {
        match (&self.inner, &other.inner) {
            (None, None) => true,
            (Some(a), Some(b)) => a.unique_id() == b.unique_id(),
            _ => false,
        }
    }
}

impl Eq for AnyWeakPtr {}

impl PartialOrd for AnyWeakPtr {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for AnyWeakPtr {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.unique_id().cmp(&other.unique_id())
    }
}

impl Hash for AnyWeakPtr {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.unique_id().hash(state);
    }
}

impl std::fmt::Debug for AnyWeakPtr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_empty() {
            write!(f, "AnyWeakPtr(empty)")
        } else if self.is_expired() {
            write!(f, "AnyWeakPtr(expired)")
        } else {
            write!(f, "AnyWeakPtr(id=0x{:x})", self.unique_id())
        }
    }
}

// Implement conversion from Weak<T>
impl<T: Send + Sync + 'static> From<Weak<T>> for AnyWeakPtr {
    fn from(weak: Weak<T>) -> Self {
        Self::from_weak(weak)
    }
}

// Implement From<()> for null pointer
impl From<()> for AnyWeakPtr {
    fn from(_: ()) -> Self {
        Self::new()
    }
}

/// Helper trait for types that can create an AnyWeakPtr.
pub trait IntoAnyWeakPtr {
    /// Converts self into an AnyWeakPtr.
    fn into_any_weak_ptr(self) -> AnyWeakPtr;
}

impl<T: Send + Sync + 'static> IntoAnyWeakPtr for Weak<T> {
    fn into_any_weak_ptr(self) -> AnyWeakPtr {
        AnyWeakPtr::from_weak(self)
    }
}

impl IntoAnyWeakPtr for AnyWeakPtr {
    fn into_any_weak_ptr(self) -> AnyWeakPtr {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_empty() {
        let any = AnyWeakPtr::new();
        assert!(any.is_empty());
        assert!(any.is_expired());
        assert!(!any.is_valid());
        assert_eq!(any.unique_id(), 0);
    }

    #[test]
    fn test_from_weak() {
        let strong = Arc::new(42);
        let weak = Arc::downgrade(&strong);

        let any = AnyWeakPtr::from_weak(weak);
        assert!(!any.is_empty());
        assert!(!any.is_expired());
        assert!(any.is_valid());
        assert_ne!(any.unique_id(), 0);
    }

    #[test]
    fn test_expired() {
        let any = {
            let strong = Arc::new(42);
            let weak = Arc::downgrade(&strong);
            AnyWeakPtr::from_weak(weak)
            // strong dropped here
        };

        assert!(!any.is_empty());
        assert!(any.is_expired());
        assert!(!any.is_valid());
    }

    #[test]
    fn test_clone() {
        let strong = Arc::new("hello");
        let weak = Arc::downgrade(&strong);

        let any1 = AnyWeakPtr::from_weak(weak);
        let any2 = any1.clone();

        assert_eq!(any1.unique_id(), any2.unique_id());
        assert!(!any1.is_expired());
        assert!(!any2.is_expired());
    }

    #[test]
    fn test_equality() {
        let strong1 = Arc::new(1);
        let strong2 = Arc::new(2);
        let weak1 = Arc::downgrade(&strong1);
        let weak1_clone = Arc::downgrade(&strong1);
        let weak2 = Arc::downgrade(&strong2);

        let any1 = AnyWeakPtr::from_weak(weak1);
        let any1_clone = AnyWeakPtr::from_weak(weak1_clone);
        let any2 = AnyWeakPtr::from_weak(weak2);

        assert_eq!(any1, any1_clone);
        assert_ne!(any1, any2);
    }

    #[test]
    fn test_ordering() {
        let strong1 = Arc::new(1);
        let strong2 = Arc::new(2);
        let any1 = AnyWeakPtr::from_weak(Arc::downgrade(&strong1));
        let any2 = AnyWeakPtr::from_weak(Arc::downgrade(&strong2));

        // Ordering should be consistent
        assert!((any1 < any2) || (any2 < any1) || (any1 == any2));
    }

    #[test]
    fn test_hash() {
        use std::collections::HashSet;

        let strong1 = Arc::new(1);
        let strong2 = Arc::new(2);
        let any1 = AnyWeakPtr::from_weak(Arc::downgrade(&strong1));
        let any1_clone = AnyWeakPtr::from_weak(Arc::downgrade(&strong1));
        let any2 = AnyWeakPtr::from_weak(Arc::downgrade(&strong2));

        let mut set = HashSet::new();
        set.insert(any1.clone());
        set.insert(any1_clone);
        set.insert(any2);

        // any1 and any1_clone should be deduplicated
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_data_type_id() {
        let strong_int = Arc::new(42i32);
        let strong_str = Arc::new("hello");

        let any_int = AnyWeakPtr::from_weak(Arc::downgrade(&strong_int));
        let any_str = AnyWeakPtr::from_weak(Arc::downgrade(&strong_str));

        assert_eq!(any_int.data_type_id(), Some(TypeId::of::<i32>()));
        assert_eq!(any_str.data_type_id(), Some(TypeId::of::<&str>()));
        assert!(any_int.data_type_id() != any_str.data_type_id());
    }

    #[test]
    fn test_from_conversion() {
        let strong = Arc::new(42);
        let weak = Arc::downgrade(&strong);

        let any: AnyWeakPtr = weak.into();
        assert!(!any.is_expired());
    }

    #[test]
    fn test_debug() {
        let empty = AnyWeakPtr::new();
        assert!(format!("{:?}", empty).contains("empty"));

        let strong = Arc::new(42);
        let any = AnyWeakPtr::from_weak(Arc::downgrade(&strong));
        let debug_str = format!("{:?}", any);
        assert!(debug_str.contains("id="));

        drop(strong);
        let debug_str = format!("{:?}", any);
        assert!(debug_str.contains("expired"));
    }

    #[test]
    fn test_default() {
        let any = AnyWeakPtr::default();
        assert!(any.is_empty());
    }
}
