//! Weak pointers with deletion detection.
//!
//! This module provides `WeakPtr<T>`, a non-owning pointer that can detect
//! when the pointed-to object has been deallocated.
//!
//! # Examples
//!
//! ```
//! use usd_tf::{RefPtr, WeakPtr};
//!
//! let strong = RefPtr::new(42);
//! let weak = WeakPtr::from_ref(&strong);
//!
//! assert!(!weak.is_expired());
//! assert_eq!(*weak.upgrade().unwrap(), 42);
//!
//! drop(strong);
//! assert!(weak.is_expired());
//! assert!(weak.upgrade().is_none());
//! ```

use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Weak};

use super::null_ptr::NullPtrType;
use super::ref_ptr::RefPtr;

/// A weak pointer that does not prevent deallocation.
///
/// `WeakPtr<T>` is a non-owning reference to an object managed by `RefPtr<T>`.
/// It can be used to break reference cycles and to observe objects without
/// preventing their deallocation.
///
/// The pointed-to object may be deallocated at any time. Before accessing
/// the object, you must upgrade the `WeakPtr` to a `RefPtr` using the
/// `upgrade()` method, which will return `None` if the object has been
/// deallocated.
///
/// # Null vs Expired
///
/// A `WeakPtr` can be in three states:
/// - **Null**: constructed with `new()`, never pointed to anything. `is_invalid()` is false.
/// - **Valid**: points to a live object. `is_expired()` is false, `is_invalid()` is false.
/// - **Expired**: object was deallocated. `is_expired()` is true, `is_invalid()` is true.
///
/// `is_invalid()` matches C++ `TfWeakPtr::IsInvalid()`: true only when the pointer
/// was associated with a live object that has since been destroyed.
///
/// # Thread Safety
///
/// `WeakPtr<T>` is thread-safe. Multiple threads can hold weak pointers
/// to the same object and upgrade them concurrently.
///
/// # Examples
///
/// ```
/// use usd_tf::{RefPtr, WeakPtr};
///
/// // Create a strong reference
/// let strong = RefPtr::new(vec![1, 2, 3]);
///
/// // Create a weak reference
/// let weak = WeakPtr::from_ref(&strong);
///
/// // The weak reference can be upgraded while strong exists
/// {
///     let upgraded = weak.upgrade();
///     assert!(upgraded.is_some());
///     assert_eq!(*upgraded.unwrap(), vec![1, 2, 3]);
/// }
///
/// // After dropping the strong reference, upgrade fails
/// drop(strong);
/// assert!(weak.upgrade().is_none());
/// ```
pub struct WeakPtr<T: ?Sized> {
    inner: Weak<T>,
    /// True if this WeakPtr was ever associated with a live object (i.e. created
    /// via `from_ref` or `From<Weak<T>>`). Used to implement C++ `IsInvalid()`
    /// semantics: invalid = once pointed somewhere, now dead.
    from_live: bool,
}

impl<T> WeakPtr<T> {
    /// Creates a new `WeakPtr` that points to nothing.
    ///
    /// This creates a null weak pointer. `is_invalid()` returns false because
    /// this pointer was never associated with any object.
    /// Calling `upgrade` on this will always return `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::WeakPtr;
    ///
    /// let weak: WeakPtr<i32> = WeakPtr::new();
    /// assert!(weak.is_expired());
    /// assert!(!weak.is_invalid());
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Weak::new(),
            from_live: false,
        }
    }
}

impl<T: ?Sized> WeakPtr<T> {
    /// Creates a `WeakPtr` from a `RefPtr`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::{RefPtr, WeakPtr};
    ///
    /// let strong = RefPtr::new(42);
    /// let weak = WeakPtr::from_ref(&strong);
    ///
    /// assert!(!weak.is_expired());
    /// ```
    #[must_use]
    pub fn from_ref(ptr: &RefPtr<T>) -> Self {
        Self {
            inner: Arc::downgrade(ptr.as_arc()),
            from_live: true,
        }
    }

    /// Attempts to upgrade this `WeakPtr` to a `RefPtr`.
    ///
    /// Returns `Some(RefPtr)` if the object still exists, `None` if it has
    /// been deallocated.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::{RefPtr, WeakPtr};
    ///
    /// let strong = RefPtr::new(42);
    /// let weak = WeakPtr::from_ref(&strong);
    ///
    /// // Upgrade succeeds while strong exists
    /// assert!(weak.upgrade().is_some());
    ///
    /// drop(strong);
    ///
    /// // Upgrade fails after strong is dropped
    /// assert!(weak.upgrade().is_none());
    /// ```
    #[must_use]
    pub fn upgrade(&self) -> Option<RefPtr<T>> {
        self.inner.upgrade().map(RefPtr::from_arc)
    }

    /// Returns `true` if the pointed-to object has been deallocated.
    ///
    /// This is equivalent to `self.upgrade().is_none()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::{RefPtr, WeakPtr};
    ///
    /// let strong = RefPtr::new(42);
    /// let weak = WeakPtr::from_ref(&strong);
    ///
    /// assert!(!weak.is_expired());
    /// drop(strong);
    /// assert!(weak.is_expired());
    /// ```
    #[must_use]
    pub fn is_expired(&self) -> bool {
        self.inner.strong_count() == 0
    }

    /// Returns `true` if the weak pointer is still valid (not expired).
    ///
    /// This is the opposite of `is_expired()`.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        !self.is_expired()
    }

    /// Returns `true` if this pointer once referred to a live object that has
    /// since been destroyed. Matches C++ `TfWeakPtr::IsInvalid()`.
    ///
    /// Returns `false` for null-constructed pointers (those never associated
    /// with any object), and `false` for pointers to still-live objects.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::{RefPtr, WeakPtr};
    ///
    /// // Null-constructed: never pointed anywhere, so NOT invalid
    /// let null_weak: WeakPtr<i32> = WeakPtr::new();
    /// assert!(!null_weak.is_invalid());
    ///
    /// // Live object: not invalid
    /// let strong = RefPtr::new(42);
    /// let weak = WeakPtr::from_ref(&strong);
    /// assert!(!weak.is_invalid());
    ///
    /// // After drop: was live, now dead — IS invalid
    /// drop(strong);
    /// assert!(weak.is_invalid());
    /// ```
    #[must_use]
    pub fn is_invalid(&self) -> bool {
        // C++ _remnant && !_IsAlive(): had a live target, now destroyed
        self.from_live && self.inner.strong_count() == 0
    }

    /// Returns the number of strong references to the object.
    ///
    /// Returns 0 if the object has been deallocated.
    #[must_use]
    pub fn strong_count(&self) -> usize {
        self.inner.strong_count()
    }

    /// Returns the number of weak references to the object.
    ///
    /// Returns 0 if the object has been deallocated.
    #[must_use]
    pub fn weak_count(&self) -> usize {
        self.inner.weak_count()
    }

    /// Returns `true` if two `WeakPtr`s point to the same allocation,
    /// or if both are expired.
    #[must_use]
    pub fn ptr_eq(this: &Self, other: &Self) -> bool {
        Weak::ptr_eq(&this.inner, &other.inner)
    }

    /// Returns a raw pointer to the underlying data, or null if expired.
    #[must_use]
    pub fn as_ptr(&self) -> *const T {
        Weak::as_ptr(&self.inner)
    }
}

impl<T: ?Sized> Clone for WeakPtr<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            from_live: self.from_live,
        }
    }
}

impl<T> Default for WeakPtr<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for WeakPtr<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WeakPtr({:?})", self.inner.upgrade())
    }
}

// WeakPtr equality is based on whether they point to the same allocation
impl<T: ?Sized> PartialEq for WeakPtr<T> {
    fn eq(&self, other: &Self) -> bool {
        WeakPtr::ptr_eq(self, other)
    }
}

impl<T: ?Sized> Eq for WeakPtr<T> {}

// Hash based on pointer address (for collections)
impl<T: ?Sized> Hash for WeakPtr<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_ptr().hash(state);
    }
}

impl<T: ?Sized> From<&RefPtr<T>> for WeakPtr<T> {
    fn from(ptr: &RefPtr<T>) -> Self {
        WeakPtr::from_ref(ptr)
    }
}

impl<T: ?Sized> From<Weak<T>> for WeakPtr<T> {
    fn from(weak: Weak<T>) -> Self {
        Self {
            inner: weak,
            from_live: true,
        }
    }
}

impl<T: ?Sized> From<NullPtrType> for Option<WeakPtr<T>> {
    fn from(_: NullPtrType) -> Self {
        None
    }
}

/// Creates a `WeakPtr` from a `RefPtr`.
///
/// This is a convenience function equivalent to `WeakPtr::from_ref(ptr)`.
///
/// # Examples
///
/// ```
/// use usd_tf::{RefPtr, create_weak_ptr};
///
/// let strong = RefPtr::new(42);
/// let weak = create_weak_ptr(&strong);
///
/// assert!(!weak.is_expired());
/// ```
#[must_use]
pub fn create_weak_ptr<T: ?Sized>(ptr: &RefPtr<T>) -> WeakPtr<T> {
    WeakPtr::from_ref(ptr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let weak: WeakPtr<i32> = WeakPtr::new();
        assert!(weak.is_expired());
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn test_from_ref() {
        let strong = RefPtr::new(42);
        let weak = WeakPtr::from_ref(&strong);

        assert!(!weak.is_expired());
        assert!(weak.is_valid());
    }

    #[test]
    fn test_upgrade() {
        let strong = RefPtr::new(42);
        let weak = WeakPtr::from_ref(&strong);

        let upgraded = weak.upgrade();
        assert!(upgraded.is_some());
        assert_eq!(*upgraded.unwrap(), 42);
    }

    #[test]
    fn test_expired_after_drop() {
        let weak;
        {
            let strong = RefPtr::new(42);
            weak = WeakPtr::from_ref(&strong);
            assert!(!weak.is_expired());
        }
        assert!(weak.is_expired());
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn test_clone() {
        let strong = RefPtr::new(42);
        let weak1 = WeakPtr::from_ref(&strong);
        let weak2 = weak1.clone();

        assert!(!weak1.is_expired());
        assert!(!weak2.is_expired());
        assert!(WeakPtr::ptr_eq(&weak1, &weak2));
    }

    #[test]
    fn test_ptr_eq() {
        let strong1 = RefPtr::new(42);
        let strong2 = RefPtr::new(42);

        let weak1 = WeakPtr::from_ref(&strong1);
        let weak2 = WeakPtr::from_ref(&strong1);
        let weak3 = WeakPtr::from_ref(&strong2);

        assert!(WeakPtr::ptr_eq(&weak1, &weak2));
        assert!(!WeakPtr::ptr_eq(&weak1, &weak3));
    }

    #[test]
    fn test_strong_count() {
        let strong1 = RefPtr::new(42);
        let weak = WeakPtr::from_ref(&strong1);

        assert_eq!(weak.strong_count(), 1);

        let strong2 = strong1.clone();
        assert_eq!(weak.strong_count(), 2);

        drop(strong2);
        assert_eq!(weak.strong_count(), 1);

        drop(strong1);
        assert_eq!(weak.strong_count(), 0);
    }

    #[test]
    fn test_default() {
        let weak: WeakPtr<i32> = WeakPtr::default();
        assert!(weak.is_expired());
    }

    #[test]
    fn test_create_weak_ptr() {
        let strong = RefPtr::new(42);
        let weak = create_weak_ptr(&strong);

        assert!(!weak.is_expired());
        assert_eq!(*weak.upgrade().unwrap(), 42);
    }

    #[test]
    fn test_equality() {
        let strong = RefPtr::new(42);
        let weak1 = WeakPtr::from_ref(&strong);
        let weak2 = WeakPtr::from_ref(&strong);

        assert_eq!(weak1, weak2);
    }

    #[test]
    fn test_hash() {
        use std::collections::HashSet;

        let strong = RefPtr::new(42);
        let weak1 = WeakPtr::from_ref(&strong);
        let weak2 = weak1.clone();

        let mut set = HashSet::new();
        set.insert(weak1);
        set.insert(weak2); // Same as weak1, should not increase size

        assert_eq!(set.len(), 1);
    }

    #[test]
    fn test_thread_safety() {
        use std::thread;

        let strong = RefPtr::new(42);
        let weak = WeakPtr::from_ref(&strong);
        let weak_clone = weak.clone();

        let handle = thread::spawn(move || {
            assert!(!weak_clone.is_expired());
            let upgraded = weak_clone.upgrade();
            assert!(upgraded.is_some());
        });

        handle.join().expect("Thread panicked");
    }

    #[test]
    fn test_is_invalid_null_constructed() {
        // Null-constructed WeakPtr was never associated with an object — NOT invalid
        let weak: WeakPtr<i32> = WeakPtr::new();
        assert!(!weak.is_invalid());
        assert!(weak.is_expired());
    }

    #[test]
    fn test_is_invalid_from_ref() {
        // Live pointer: not invalid
        let strong = RefPtr::new(42);
        let weak = WeakPtr::from_ref(&strong);
        assert!(!weak.is_invalid());
        assert!(!weak.is_expired());

        drop(strong);
        // Was live, now dead: IS invalid (C++ IsInvalid semantics)
        assert!(weak.is_invalid());
        assert!(weak.is_expired());
    }

    #[test]
    fn test_is_invalid_clone_preserves() {
        // Cloned null stays not-invalid
        let weak: WeakPtr<i32> = WeakPtr::new();
        let cloned = weak.clone();
        assert!(!cloned.is_invalid());

        // Cloned live pointer expires together with original
        let strong = RefPtr::new(42);
        let weak2 = WeakPtr::from_ref(&strong);
        let cloned2 = weak2.clone();
        assert!(!cloned2.is_invalid());
        drop(strong);
        assert!(cloned2.is_invalid());
    }

    #[test]
    fn test_is_invalid_default() {
        // Default (null-constructed) is NOT invalid
        let weak: WeakPtr<i32> = WeakPtr::default();
        assert!(!weak.is_invalid());
    }
}
