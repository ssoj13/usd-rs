//! Reference-counted smart pointers.
//!
//! This module provides `RefPtr<T>`, a thread-safe reference-counted pointer
//! similar to `std::sync::Arc<T>` but with additional features matching
//! OpenUSD's TfRefPtr.
//!
//! # Examples
//!
//! ```
//! use usd_tf::{RefPtr, RefBase};
//!
//! struct MyData {
//!     value: i32,
//! }
//!
//! let ptr1 = RefPtr::new(MyData { value: 42 });
//! let ptr2 = ptr1.clone();
//!
//! assert_eq!(ptr1.strong_count(), 2);
//! assert!(!ptr1.is_unique());
//!
//! drop(ptr2);
//! assert!(ptr1.is_unique());
//! ```

use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::Arc;

use super::null_ptr::NullPtrType;

/// Trait for types that support reference counting.
///
/// This trait is automatically implemented for all types when wrapped
/// in a `RefPtr`. It provides methods to query the reference count.
pub trait RefBase {
    /// Returns the current reference count.
    fn strong_count(&self) -> usize;

    /// Returns true if this is the only reference.
    fn is_unique(&self) -> bool {
        self.strong_count() == 1
    }
}

/// A thread-safe reference-counted pointer.
///
/// `RefPtr<T>` is a wrapper around `Arc<T>` that provides an API compatible
/// with OpenUSD's TfRefPtr. It provides automatic memory management through
/// reference counting, with the object being deallocated when the last
/// reference is dropped.
///
/// # Thread Safety
///
/// `RefPtr<T>` is thread-safe. The reference count is maintained using
/// atomic operations, so creating and dropping clones of a `RefPtr` can
/// be done from any thread.
///
/// However, accessing the underlying data requires `T: Send + Sync` for
/// safe multi-threaded access.
///
/// # Examples
///
/// ```
/// use usd_tf::RefPtr;
///
/// // Create a new reference-counted value
/// let ptr = RefPtr::new(vec![1, 2, 3]);
///
/// // Clone to create another reference
/// let ptr2 = ptr.clone();
///
/// // Both point to the same data
/// assert_eq!(ptr.len(), 3);
/// assert_eq!(ptr2.len(), 3);
/// ```
pub struct RefPtr<T: ?Sized> {
    inner: Arc<T>,
}

impl<T> RefPtr<T> {
    /// Creates a new `RefPtr` containing the given value.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::RefPtr;
    ///
    /// let ptr = RefPtr::new(42);
    /// assert_eq!(*ptr, 42);
    /// ```
    #[must_use]
    pub fn new(value: T) -> Self {
        Self {
            inner: Arc::new(value),
        }
    }

    /// Converts this `RefPtr` into the underlying `Arc`.
    #[must_use]
    pub fn into_arc(self) -> Arc<T> {
        self.inner
    }
}

impl<T: ?Sized> RefPtr<T> {
    /// Creates a `RefPtr` from an existing `Arc`.
    ///
    /// This is useful for interoperability with code that uses `Arc` directly.
    #[must_use]
    pub fn from_arc(arc: Arc<T>) -> Self {
        Self { inner: arc }
    }

    /// Returns a reference to the underlying `Arc`.
    #[must_use]
    pub fn as_arc(&self) -> &Arc<T> {
        &self.inner
    }
}

impl<T: ?Sized> RefPtr<T> {
    /// Returns the number of strong references to this value.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::RefPtr;
    ///
    /// let ptr = RefPtr::new(42);
    /// assert_eq!(ptr.strong_count(), 1);
    ///
    /// let ptr2 = ptr.clone();
    /// assert_eq!(ptr.strong_count(), 2);
    /// ```
    #[must_use]
    pub fn strong_count(&self) -> usize {
        Arc::strong_count(&self.inner)
    }

    /// Returns `true` if this is the only `RefPtr` pointing to this value.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::RefPtr;
    ///
    /// let ptr = RefPtr::new(42);
    /// assert!(ptr.is_unique());
    ///
    /// let ptr2 = ptr.clone();
    /// assert!(!ptr.is_unique());
    /// ```
    #[must_use]
    pub fn is_unique(&self) -> bool {
        self.strong_count() == 1
    }

    /// Returns `true` if two `RefPtr`s point to the same allocation.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::RefPtr;
    ///
    /// let ptr1 = RefPtr::new(42);
    /// let ptr2 = ptr1.clone();
    /// let ptr3 = RefPtr::new(42);
    ///
    /// assert!(RefPtr::ptr_eq(&ptr1, &ptr2));
    /// assert!(!RefPtr::ptr_eq(&ptr1, &ptr3));
    /// ```
    #[must_use]
    pub fn ptr_eq(this: &Self, other: &Self) -> bool {
        Arc::ptr_eq(&this.inner, &other.inner)
    }

    /// Returns a raw pointer to the underlying data.
    ///
    /// The pointer is valid as long as at least one `RefPtr` exists.
    #[must_use]
    pub fn as_ptr(&self) -> *const T {
        Arc::as_ptr(&self.inner)
    }
}

impl<T: Clone> RefPtr<T> {
    /// Makes a mutable reference to the value if possible.
    ///
    /// If there are other references to this value, clones the inner value
    /// and returns a mutable reference to the clone.
    ///
    /// This is equivalent to `Arc::make_mut`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::RefPtr;
    ///
    /// let mut ptr = RefPtr::new(vec![1, 2, 3]);
    /// RefPtr::make_mut(&mut ptr).push(4);
    /// assert_eq!(*ptr, vec![1, 2, 3, 4]);
    /// ```
    pub fn make_mut(this: &mut Self) -> &mut T {
        Arc::make_mut(&mut this.inner)
    }
}

impl<T: ?Sized> Clone for RefPtr<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<T: ?Sized> Deref for RefPtr<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for RefPtr<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: ?Sized + fmt::Display> fmt::Display for RefPtr<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

impl<T: Default> Default for RefPtr<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: ?Sized> PartialEq for RefPtr<T> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl<T: ?Sized> Eq for RefPtr<T> {}

impl<T: ?Sized + PartialOrd> PartialOrd for RefPtr<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        (**self).partial_cmp(&**other)
    }
}

impl<T: ?Sized + Ord> Ord for RefPtr<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (**self).cmp(&**other)
    }
}

impl<T: ?Sized> Hash for RefPtr<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.inner).hash(state);
    }
}

impl<T> From<T> for RefPtr<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<T: ?Sized> From<Arc<T>> for RefPtr<T> {
    fn from(arc: Arc<T>) -> Self {
        Self { inner: arc }
    }
}

impl<T: ?Sized> From<RefPtr<T>> for Arc<T> {
    fn from(ptr: RefPtr<T>) -> Self {
        ptr.inner
    }
}

impl<T: ?Sized> From<NullPtrType> for Option<RefPtr<T>> {
    fn from(_: NullPtrType) -> Self {
        None
    }
}

impl<T: ?Sized> AsRef<T> for RefPtr<T> {
    fn as_ref(&self) -> &T {
        &self.inner
    }
}

// Implement RefBase for RefPtr
impl<T: ?Sized> RefBase for RefPtr<T> {
    fn strong_count(&self) -> usize {
        RefPtr::strong_count(self)
    }
}

/// Creates a `RefPtr` from a value.
///
/// This is a convenience function equivalent to `RefPtr::new(value)`.
///
/// # Examples
///
/// ```
/// use usd_tf::create_ref_ptr;
///
/// let ptr = create_ref_ptr(42);
/// assert_eq!(*ptr, 42);
/// ```
#[must_use]
pub fn create_ref_ptr<T>(value: T) -> RefPtr<T> {
    RefPtr::new(value)
}

/// Attempts to downcast a `RefPtr<dyn Any + Send + Sync>` to a concrete type.
///
/// Returns `Some(RefPtr<T>)` if the downcast succeeds, `None` otherwise.
///
/// # Examples
///
/// ```
/// use std::any::Any;
/// use std::sync::Arc;
/// use usd_tf::{RefPtr, dynamic_cast};
///
/// // Create an Arc<dyn Any + Send + Sync> first, then wrap in RefPtr
/// let arc: Arc<dyn Any + Send + Sync> = Arc::new(42i32);
/// let ptr = RefPtr::from_arc(arc);
/// let int_ptr = dynamic_cast::<i32>(&ptr);
/// assert!(int_ptr.is_some());
/// assert_eq!(*int_ptr.unwrap(), 42);
/// ```
pub fn dynamic_cast<T: 'static + Send + Sync>(
    ptr: &RefPtr<dyn std::any::Any + Send + Sync>,
) -> Option<RefPtr<T>> {
    ptr.inner.clone().downcast::<T>().ok().map(RefPtr::from_arc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let ptr = RefPtr::new(42);
        assert_eq!(*ptr, 42);
        assert_eq!(ptr.strong_count(), 1);
    }

    #[test]
    fn test_clone() {
        let ptr1 = RefPtr::new(42);
        let ptr2 = ptr1.clone();

        assert_eq!(*ptr1, 42);
        assert_eq!(*ptr2, 42);
        assert_eq!(ptr1.strong_count(), 2);
        assert_eq!(ptr2.strong_count(), 2);
    }

    #[test]
    fn test_is_unique() {
        let ptr1 = RefPtr::new(42);
        assert!(ptr1.is_unique());

        let ptr2 = ptr1.clone();
        assert!(!ptr1.is_unique());
        assert!(!ptr2.is_unique());

        drop(ptr2);
        assert!(ptr1.is_unique());
    }

    #[test]
    fn test_ptr_eq() {
        let ptr1 = RefPtr::new(42);
        let ptr2 = ptr1.clone();
        let ptr3 = RefPtr::new(42);

        assert!(RefPtr::ptr_eq(&ptr1, &ptr2));
        assert!(!RefPtr::ptr_eq(&ptr1, &ptr3));
    }

    #[test]
    fn test_make_mut() {
        let mut ptr1 = RefPtr::new(vec![1, 2, 3]);
        let ptr2 = ptr1.clone();

        // This should clone since there are multiple references
        RefPtr::make_mut(&mut ptr1).push(4);

        // ptr1 now points to [1, 2, 3, 4]
        // ptr2 still points to [1, 2, 3]
        assert_eq!(*ptr1, vec![1, 2, 3, 4]);
        assert_eq!(*ptr2, vec![1, 2, 3]);
    }

    #[test]
    fn test_from_arc() {
        let arc = Arc::new(42);
        let ptr = RefPtr::from_arc(arc.clone());

        assert_eq!(*ptr, 42);
        assert_eq!(Arc::strong_count(&arc), 2);
    }

    #[test]
    fn test_into_arc() {
        let ptr = RefPtr::new(42);
        let arc = ptr.into_arc();

        assert_eq!(*arc, 42);
    }

    #[test]
    fn test_default() {
        let ptr: RefPtr<i32> = RefPtr::default();
        assert_eq!(*ptr, 0);
    }

    #[test]
    fn test_equality() {
        let ptr1 = RefPtr::new(42);
        let ptr2 = ptr1.clone(); // same allocation
        let ptr3 = RefPtr::new(42); // different allocation, same value

        assert_eq!(ptr1, ptr2);
        assert_ne!(ptr1, ptr3); // pointer identity, not value equality
    }

    #[test]
    fn test_ordering() {
        let ptr1 = RefPtr::new(1);
        let ptr2 = RefPtr::new(2);
        let ptr3 = RefPtr::new(2);

        assert!(ptr1 < ptr2);
        assert!(ptr2 >= ptr3);
    }

    #[test]
    fn test_hash() {
        use std::collections::HashSet;

        let ptr1 = RefPtr::new(1);
        let ptr2 = ptr1.clone(); // same allocation
        let ptr3 = RefPtr::new(1); // different allocation, same value

        let mut set = HashSet::new();
        set.insert(ptr1);
        set.insert(ptr2); // same pointer as ptr1, no new entry
        set.insert(ptr3); // different pointer, new entry

        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_debug() {
        let ptr = RefPtr::new(42);
        assert_eq!(format!("{:?}", ptr), "42");
    }

    #[test]
    fn test_display() {
        let ptr = RefPtr::new(42);
        assert_eq!(format!("{}", ptr), "42");
    }

    #[test]
    fn test_create_ref_ptr() {
        let ptr = create_ref_ptr(42);
        assert_eq!(*ptr, 42);
    }

    #[test]
    fn test_ref_base_trait() {
        let ptr = RefPtr::new(42);
        assert_eq!(RefBase::strong_count(&ptr), 1);
        assert!(RefBase::is_unique(&ptr));
    }

    #[test]
    fn test_thread_safety() {
        use std::thread;

        let ptr = RefPtr::new(42);
        let ptr2 = ptr.clone();

        let handle = thread::spawn(move || {
            assert_eq!(*ptr2, 42);
        });

        assert_eq!(*ptr, 42);
        handle.join().expect("Thread panicked");
    }
}
