//! Weak pointer facade pattern.
//!
//! Port of pxr/base/tf/weakPtrFacade.h
//!
//! In C++, TfWeakPtrFacade is a CRTP base class that provides a common
//! interface for all weak pointer types. In Rust, we express this as a trait
//! with default method implementations, plus free-standing casting functions.

use std::any::Any;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Weak};

/// Trait providing the weak pointer facade interface.
///
/// Matches C++ `TfWeakPtrFacade<PtrTemplate, DataType>`.
///
/// Types implementing this trait get a uniform interface for
/// null-checking, identity comparison, pointer access, validity
/// checking, and hash support.
pub trait WeakPtrFacade: Sized {
    /// The data type this weak pointer points to.
    type DataType: ?Sized;

    /// Attempt to fetch the underlying pointer.
    /// Returns None if the object has been destroyed.
    fn fetch_pointer(&self) -> Option<Arc<Self::DataType>>;

    /// Get a unique identifier for this pointer (for identity comparison).
    /// Returns None if the pointer is null/expired.
    fn unique_id(&self) -> Option<usize>;

    /// Returns true if this pointer is expired/invalid.
    fn is_invalid(&self) -> bool;

    /// Returns true if this pointer is valid (not expired).
    fn is_valid(&self) -> bool {
        !self.is_invalid()
    }

    /// Returns true if this pointer points to the given object.
    fn points_to<U>(&self, _obj: &U) -> bool
    where
        Self::DataType: Sized,
        U: ?Sized,
    {
        false // Default: can't compare without concrete types
    }

    /// Reset this pointer to null.
    fn reset(&mut self);
}

/// Blanket implementation of WeakPtrFacade for `Weak<T>`.
///
/// This makes any `Weak<T>` automatically satisfy the facade trait.
impl<T: 'static> WeakPtrFacade for Weak<T> {
    type DataType = T;

    fn fetch_pointer(&self) -> Option<Arc<T>> {
        self.upgrade()
    }

    fn unique_id(&self) -> Option<usize> {
        // Use the Weak's data pointer as identity
        self.upgrade()
            .map(|arc| Arc::as_ptr(&arc) as *const () as usize)
    }

    fn is_invalid(&self) -> bool {
        self.strong_count() == 0
    }

    fn reset(&mut self) {
        *self = Weak::new();
    }
}

/// Attempt a dynamic cast of a weak pointer's target type.
///
/// Matches C++ `TfDynamic_cast<ToPtr>(weakPtr)`.
///
/// Returns None if the pointer is expired or the cast fails.
/// Note: In Rust, true dynamic downcasting of Arc requires the target to be
/// `Any + Send + Sync`. This version checks type identity.
pub fn dynamic_cast<T, U>(ptr: &Weak<T>) -> Option<Arc<T>>
where
    T: Any + 'static,
    U: 'static,
{
    let arc = ptr.upgrade()?;
    // Check if T and U are the same type
    if std::any::TypeId::of::<T>() == std::any::TypeId::of::<U>() {
        Some(arc)
    } else {
        None
    }
}

/// Static cast of a weak pointer's target type.
///
/// Matches C++ `TfStatic_cast<ToPtr>(weakPtr)`.
///
/// # Safety
///
/// Caller must ensure the cast is valid. This is a no-op in Rust's type system
/// since we require the types to be compatible at compile time.
pub fn static_cast<T>(ptr: &Weak<T>) -> Option<Arc<T>> {
    ptr.upgrade()
}

/// Helper: get a raw pointer from a weak pointer facade.
///
/// Matches C++ `get_pointer(TfWeakPtrFacade)`.
pub fn get_pointer<F: WeakPtrFacade>(facade: &F) -> Option<Arc<F::DataType>> {
    facade.fetch_pointer()
}

/// Compare two weak pointer facades for identity equality.
///
/// Two weak pointers are equal if they point to the same object.
pub fn ptr_eq<F: WeakPtrFacade>(a: &F, b: &F) -> bool {
    match (a.unique_id(), b.unique_id()) {
        (Some(id_a), Some(id_b)) => id_a == id_b,
        (None, None) => true, // Both null
        _ => false,
    }
}

/// Compare a weak pointer facade with null.
pub fn is_null<F: WeakPtrFacade>(ptr: &F) -> bool {
    ptr.is_invalid()
}

/// Wrapper providing Debug, PartialEq, Eq, Hash for any WeakPtrFacade.
///
/// Useful when storing weak pointers in collections that need these traits.
pub struct WeakPtrWrapper<T: 'static> {
    inner: Weak<T>,
}

impl<T: 'static> WeakPtrWrapper<T> {
    /// Wrap a Weak<T> to add comparison/hash support.
    pub fn new(weak: Weak<T>) -> Self {
        Self { inner: weak }
    }

    /// Get the underlying Weak<T>.
    pub fn inner(&self) -> &Weak<T> {
        &self.inner
    }

    /// Unwrap into the underlying Weak<T>.
    pub fn into_inner(self) -> Weak<T> {
        self.inner
    }
}

impl<T: 'static> Clone for WeakPtrWrapper<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T: 'static> fmt::Debug for WeakPtrWrapper<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let valid = self.inner.strong_count() > 0;
        f.debug_struct("WeakPtrWrapper")
            .field("valid", &valid)
            .field("strong_count", &self.inner.strong_count())
            .finish()
    }
}

impl<T: 'static> PartialEq for WeakPtrWrapper<T> {
    fn eq(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.inner, &other.inner)
    }
}

impl<T: 'static> Eq for WeakPtrWrapper<T> {}

impl<T: 'static> Hash for WeakPtrWrapper<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Use the pointer address for hashing
        (self.inner.as_ptr() as usize).hash(state);
    }
}

impl<T: 'static> From<Weak<T>> for WeakPtrWrapper<T> {
    fn from(weak: Weak<T>) -> Self {
        Self::new(weak)
    }
}

impl<T: 'static> From<&Arc<T>> for WeakPtrWrapper<T> {
    fn from(arc: &Arc<T>) -> Self {
        Self::new(Arc::downgrade(arc))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_weak_facade_valid() {
        let arc = Arc::new(42);
        let weak = Arc::downgrade(&arc);

        assert!(!weak.is_invalid());
        assert!(weak.is_valid());
        assert!(weak.fetch_pointer().is_some());
        assert!(weak.unique_id().is_some());
    }

    #[test]
    fn test_weak_facade_expired() {
        let weak = {
            let arc = Arc::new(42);
            Arc::downgrade(&arc)
        };

        assert!(weak.is_invalid());
        assert!(!weak.is_valid());
        assert!(weak.fetch_pointer().is_none());
    }

    #[test]
    fn test_weak_facade_reset() {
        let arc = Arc::new(42);
        let mut weak = Arc::downgrade(&arc);

        assert!(weak.is_valid());
        weak.reset();
        assert!(weak.is_invalid());
    }

    #[test]
    fn test_ptr_eq() {
        let arc = Arc::new(42);
        let w1 = Arc::downgrade(&arc);
        let w2 = Arc::downgrade(&arc);
        let arc2 = Arc::new(42);
        let w3 = Arc::downgrade(&arc2);

        assert!(ptr_eq(&w1, &w2));
        assert!(!ptr_eq(&w1, &w3));
    }

    #[test]
    fn test_null_comparison() {
        let weak: Weak<i32> = Weak::new();
        assert!(is_null(&weak));

        let arc = Arc::new(42);
        let weak2 = Arc::downgrade(&arc);
        assert!(!is_null(&weak2));
    }

    #[test]
    fn test_get_pointer() {
        let arc = Arc::new(42);
        let weak = Arc::downgrade(&arc);

        let fetched = get_pointer(&weak);
        assert!(fetched.is_some());
        assert_eq!(*fetched.unwrap(), 42);
    }

    #[test]
    fn test_wrapper_eq_hash() {
        let arc = Arc::new(42);
        let w1 = WeakPtrWrapper::from(&arc);
        let w2 = WeakPtrWrapper::from(&arc);

        assert_eq!(w1, w2);

        let mut set = HashSet::new();
        set.insert(w1);
        assert!(set.contains(&w2));
    }

    #[test]
    fn test_wrapper_debug() {
        let arc = Arc::new(42);
        let wrapper = WeakPtrWrapper::from(&arc);
        let debug = format!("{:?}", wrapper);
        assert!(debug.contains("valid"));
        assert!(debug.contains("true"));
    }

    #[test]
    fn test_wrapper_clone() {
        let arc = Arc::new(42);
        let w1 = WeakPtrWrapper::from(&arc);
        let w2 = w1.clone();
        assert_eq!(w1, w2);
    }

    #[test]
    fn test_wrapper_into_inner() {
        let arc = Arc::new(42);
        let wrapper = WeakPtrWrapper::from(&arc);
        let weak = wrapper.into_inner();
        assert!(weak.upgrade().is_some());
    }
}
