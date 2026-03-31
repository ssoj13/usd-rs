//! Delegated count smart pointer.
//!
//! Provides a smart pointer that delegates reference counting to external
//! functions. This is useful for adapting types with their own bespoke
//! reference counting logic to a common Rust smart pointer interface.
//!
//! # Overview
//!
//! [`DelegatedCountPtr`] calls user-provided increment and decrement functions
//! as needed during construction, assignment, and drop operations. The user
//! must implement the [`DelegatedCount`] trait for their type.
//!
//! # Examples
//!
//! ```
//! use usd_tf::delegated_count_ptr::{DelegatedCountPtr, DelegatedCount, IncrementTag, DoNotIncrementTag};
//! use std::sync::atomic::{AtomicUsize, Ordering};
//!
//! struct MyObject {
//!     ref_count: AtomicUsize,
//!     value: i32,
//! }
//!
//! impl DelegatedCount for MyObject {
//!     fn increment(ptr: *mut Self) {
//!         unsafe {
//!             (*ptr).ref_count.fetch_add(1, Ordering::Relaxed);
//!         }
//!     }
//!
//!     fn decrement(ptr: *mut Self) {
//!         unsafe {
//!             if (*ptr).ref_count.fetch_sub(1, Ordering::Release) == 1 {
//!                 std::sync::atomic::fence(Ordering::Acquire);
//!                 drop(Box::from_raw(ptr));
//!             }
//!         }
//!     }
//! }
//!
//! // Create object with initial ref count
//! let obj = Box::into_raw(Box::new(MyObject {
//!     ref_count: AtomicUsize::new(0),
//!     value: 42,
//! }));
//!
//! // Create ptr, incrementing count
//! let ptr = DelegatedCountPtr::new(IncrementTag, obj);
//! assert_eq!(unsafe { (*obj).ref_count.load(Ordering::Relaxed) }, 1);
//!
//! // Clone increments count
//! let ptr2 = ptr.clone();
//! assert_eq!(unsafe { (*obj).ref_count.load(Ordering::Relaxed) }, 2);
//!
//! // Drop decrements count
//! drop(ptr2);
//! assert_eq!(unsafe { (*obj).ref_count.load(Ordering::Relaxed) }, 1);
//!
//! // Access the value
//! assert_eq!(ptr.value, 42);
//! ```

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::ops::Deref;
use std::ptr::NonNull;

/// Tag type for constructing a [`DelegatedCountPtr`] with incrementing.
///
/// When using this tag, the delegated count will be incremented on construction.
/// This is the most common tag to use.
#[derive(Debug, Clone, Copy, Default)]
pub struct IncrementTag;

/// Tag type for constructing a [`DelegatedCountPtr`] without incrementing.
///
/// When using this tag, the delegated count will NOT be incremented on construction.
/// Use this carefully to avoid memory errors - typically when the pointer already
/// has its count incremented (e.g., from a C API that returns an incremented pointer).
#[derive(Debug, Clone, Copy, Default)]
pub struct DoNotIncrementTag;

/// Trait for types that support delegated reference counting.
///
/// Implementors must provide `increment` and `decrement` functions that manage
/// the reference count for the type. These functions are never called with null pointers.
///
/// # Safety
///
/// The `increment` and `decrement` functions receive raw pointers and must be
/// implemented correctly to avoid memory safety issues:
///
/// - `increment` should increase the reference count
/// - `decrement` should decrease the reference count and deallocate when it reaches zero
pub trait DelegatedCount {
    /// Increment the reference count for the object at `ptr`.
    ///
    /// This function is never called with a null pointer.
    fn increment(ptr: *mut Self);

    /// Decrement the reference count for the object at `ptr`.
    ///
    /// This function is never called with a null pointer.
    /// If the count reaches zero, this function should deallocate the object.
    fn decrement(ptr: *mut Self);
}

/// A smart pointer that delegates reference counting to external functions.
///
/// This type is useful for wrapping types that have their own reference counting
/// mechanism (e.g., types from C libraries) into a Rust smart pointer interface.
///
/// The type `T` must implement [`DelegatedCount`] to provide the increment and
/// decrement functions.
///
/// # Thread Safety
///
/// `DelegatedCountPtr` is `Send` and `Sync` if `T` is `Send` and `Sync` and
/// the `DelegatedCount` implementation is thread-safe.
pub struct DelegatedCountPtr<T: DelegatedCount> {
    ptr: Option<NonNull<T>>,
    _marker: PhantomData<T>,
}

impl<T: DelegatedCount> DelegatedCountPtr<T> {
    /// Create a new `DelegatedCountPtr` from a raw pointer without incrementing.
    ///
    /// # Safety
    ///
    /// The caller must ensure that:
    /// - The pointer is valid (if non-null)
    /// - The pointer's reference count is already properly set up
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::delegated_count_ptr::{DelegatedCountPtr, DelegatedCount, DoNotIncrementTag};
    /// use std::sync::atomic::{AtomicUsize, Ordering};
    ///
    /// struct Obj { count: AtomicUsize }
    /// impl DelegatedCount for Obj {
    ///     fn increment(ptr: *mut Self) { unsafe { (*ptr).count.fetch_add(1, Ordering::Relaxed); } }
    ///     fn decrement(ptr: *mut Self) { unsafe { (*ptr).count.fetch_sub(1, Ordering::Relaxed); } }
    /// }
    ///
    /// // When already incremented (e.g., from C API)
    /// let obj = Box::into_raw(Box::new(Obj { count: AtomicUsize::new(1) }));
    /// let ptr = DelegatedCountPtr::new_no_increment(DoNotIncrementTag, obj);
    /// assert_eq!(unsafe { (*obj).count.load(Ordering::Relaxed) }, 1);
    /// std::mem::forget(ptr); // Prevent drop for this example
    /// unsafe { drop(Box::from_raw(obj)); }
    /// ```
    #[inline]
    pub fn new_no_increment(_tag: DoNotIncrementTag, raw: *mut T) -> Self {
        Self {
            ptr: NonNull::new(raw),
            _marker: PhantomData,
        }
    }

    /// Create a null `DelegatedCountPtr`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::delegated_count_ptr::{DelegatedCountPtr, DelegatedCount};
    ///
    /// struct Obj;
    /// impl DelegatedCount for Obj {
    ///     fn increment(_: *mut Self) {}
    ///     fn decrement(_: *mut Self) {}
    /// }
    ///
    /// let ptr: DelegatedCountPtr<Obj> = DelegatedCountPtr::null();
    /// assert!(ptr.is_null());
    /// ```
    #[inline]
    pub const fn null() -> Self {
        Self {
            ptr: None,
            _marker: PhantomData,
        }
    }

    /// Check if this pointer is null.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::delegated_count_ptr::{DelegatedCountPtr, DelegatedCount};
    ///
    /// struct Obj;
    /// impl DelegatedCount for Obj {
    ///     fn increment(_: *mut Self) {}
    ///     fn decrement(_: *mut Self) {}
    /// }
    ///
    /// let ptr: DelegatedCountPtr<Obj> = DelegatedCountPtr::null();
    /// assert!(ptr.is_null());
    /// ```
    #[inline]
    pub fn is_null(&self) -> bool {
        self.ptr.is_none()
    }

    /// Get the raw pointer.
    ///
    /// Returns null if the pointer is null.
    #[inline]
    pub fn get(&self) -> *mut T {
        self.ptr.map_or(std::ptr::null_mut(), |p| p.as_ptr())
    }

    /// Reset this pointer to null.
    ///
    /// If the pointer was non-null, `decrement` will be called.
    #[inline]
    pub fn reset(&mut self) {
        if let Some(ptr) = self.ptr.take() {
            T::decrement(ptr.as_ptr());
        }
    }

    /// Swap the pointers of two `DelegatedCountPtr` instances.
    #[inline]
    pub fn swap(&mut self, other: &mut Self) {
        std::mem::swap(&mut self.ptr, &mut other.ptr);
    }

    /// Get a reference to the inner value, if non-null.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::delegated_count_ptr::{DelegatedCountPtr, DelegatedCount, IncrementTag};
    ///
    /// struct Obj { value: i32 }
    /// impl DelegatedCount for Obj {
    ///     fn increment(_: *mut Self) {}
    ///     fn decrement(_: *mut Self) {}
    /// }
    ///
    /// let obj = Box::into_raw(Box::new(Obj { value: 42 }));
    /// let ptr = DelegatedCountPtr::new(IncrementTag, obj);
    /// assert_eq!(ptr.as_ref().map(|o| o.value), Some(42));
    /// std::mem::forget(ptr);
    /// unsafe { drop(Box::from_raw(obj)); }
    /// ```
    #[inline]
    pub fn as_ref(&self) -> Option<&T> {
        // SAFETY: NonNull guarantees the pointer is non-null
        #[allow(unsafe_code)]
        self.ptr.map(|p| unsafe { p.as_ref() })
    }

    /// Create a new `DelegatedCountPtr` from a raw pointer, incrementing the count.
    ///
    /// If the pointer is non-null, `increment` will be called.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the pointer is valid (if non-null).
    #[inline]
    pub fn new(_tag: IncrementTag, raw: *mut T) -> Self {
        if let Some(ptr) = NonNull::new(raw) {
            T::increment(ptr.as_ptr());
            Self {
                ptr: Some(ptr),
                _marker: PhantomData,
            }
        } else {
            Self::null()
        }
    }
}

impl<T: DelegatedCount> Default for DelegatedCountPtr<T> {
    #[inline]
    fn default() -> Self {
        Self::null()
    }
}

impl<T: DelegatedCount> Clone for DelegatedCountPtr<T> {
    #[inline]
    fn clone(&self) -> Self {
        if let Some(ptr) = self.ptr {
            T::increment(ptr.as_ptr());
            Self {
                ptr: Some(ptr),
                _marker: PhantomData,
            }
        } else {
            Self::null()
        }
    }
}

impl<T: DelegatedCount> Drop for DelegatedCountPtr<T> {
    #[inline]
    fn drop(&mut self) {
        if let Some(ptr) = self.ptr {
            T::decrement(ptr.as_ptr());
        }
    }
}

impl<T: DelegatedCount> Deref for DelegatedCountPtr<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_ref().expect("dereferenced null DelegatedCountPtr")
    }
}

impl<T: DelegatedCount> PartialEq for DelegatedCountPtr<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.get() == other.get()
    }
}

impl<T: DelegatedCount> Eq for DelegatedCountPtr<T> {}

impl<T: DelegatedCount> PartialOrd for DelegatedCountPtr<T> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: DelegatedCount> Ord for DelegatedCountPtr<T> {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.get().cmp(&other.get())
    }
}

impl<T: DelegatedCount> Hash for DelegatedCountPtr<T> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.get().hash(state);
    }
}

impl<T: DelegatedCount + fmt::Debug> fmt::Debug for DelegatedCountPtr<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.as_ref() {
            Some(v) => f.debug_tuple("DelegatedCountPtr").field(v).finish(),
            None => f.debug_tuple("DelegatedCountPtr").field(&"null").finish(),
        }
    }
}

impl<T: DelegatedCount> fmt::Pointer for DelegatedCountPtr<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Pointer::fmt(&self.get(), f)
    }
}

// SAFETY: DelegatedCountPtr<T> is Send if T is Send because we only hold
// a pointer to T and the DelegatedCount operations are expected to be thread-safe.
#[allow(unsafe_code)]
unsafe impl<T: DelegatedCount + Send> Send for DelegatedCountPtr<T> {}

// SAFETY: DelegatedCountPtr<T> is Sync if T is Sync because we only hold
// a pointer to T and the DelegatedCount operations are expected to be thread-safe.
#[allow(unsafe_code)]
unsafe impl<T: DelegatedCount + Sync> Sync for DelegatedCountPtr<T> {}

/// Create a new object on the heap and wrap it in a [`DelegatedCountPtr`].
///
/// This function allocates the object with `Box::new` and then wraps it
/// in a `DelegatedCountPtr` with increment.
///
/// # Examples
///
/// ```
/// use usd_tf::delegated_count_ptr::{make_delegated_count_ptr, DelegatedCount};
/// use std::sync::atomic::{AtomicUsize, Ordering};
///
/// struct Obj {
///     count: AtomicUsize,
///     value: i32,
/// }
///
/// impl Obj {
///     fn new(value: i32) -> Self {
///         Self {
///             count: AtomicUsize::new(0),
///             value,
///         }
///     }
/// }
///
/// impl DelegatedCount for Obj {
///     fn increment(ptr: *mut Self) {
///         unsafe { (*ptr).count.fetch_add(1, Ordering::Relaxed); }
///     }
///     fn decrement(ptr: *mut Self) {
///         unsafe {
///             if (*ptr).count.fetch_sub(1, Ordering::Release) == 1 {
///                 std::sync::atomic::fence(Ordering::Acquire);
///                 drop(Box::from_raw(ptr));
///             }
///         }
///     }
/// }
///
/// let ptr = make_delegated_count_ptr(Obj::new(42));
/// assert_eq!(ptr.value, 42);
/// ```
#[inline]
pub fn make_delegated_count_ptr<T: DelegatedCount>(value: T) -> DelegatedCountPtr<T> {
    let raw = Box::into_raw(Box::new(value));
    DelegatedCountPtr::new(IncrementTag, raw)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

    // Test object with atomic reference counting
    #[derive(Debug)]
    struct TestObj {
        count: AtomicUsize,
        value: i32,
        #[allow(dead_code)]
        drop_count: *const AtomicUsize,
    }

    impl DelegatedCount for TestObj {
        fn increment(ptr: *mut Self) {
            unsafe {
                (*ptr).count.fetch_add(1, AtomicOrdering::Relaxed);
            }
        }

        fn decrement(ptr: *mut Self) {
            unsafe {
                if (*ptr).count.fetch_sub(1, AtomicOrdering::Release) == 1 {
                    std::sync::atomic::fence(AtomicOrdering::Acquire);
                    let drop_count = (*ptr).drop_count;
                    drop(Box::from_raw(ptr));
                    if !drop_count.is_null() {
                        (*drop_count).fetch_add(1, AtomicOrdering::Relaxed);
                    }
                }
            }
        }
    }

    fn make_test_obj(value: i32, drop_count: &AtomicUsize) -> *mut TestObj {
        Box::into_raw(Box::new(TestObj {
            count: AtomicUsize::new(0),
            value,
            drop_count: drop_count as *const AtomicUsize,
        }))
    }

    #[test]
    fn test_null_pointer() {
        let ptr: DelegatedCountPtr<TestObj> = DelegatedCountPtr::null();
        assert!(ptr.is_null());
        assert!(ptr.get().is_null());
    }

    #[test]
    fn test_default_is_null() {
        let ptr: DelegatedCountPtr<TestObj> = DelegatedCountPtr::default();
        assert!(ptr.is_null());
    }

    #[test]
    fn test_new_with_increment() {
        let drop_count = AtomicUsize::new(0);
        let raw = make_test_obj(42, &drop_count);

        let ptr = DelegatedCountPtr::new(IncrementTag, raw);
        assert!(!ptr.is_null());
        assert_eq!(unsafe { (*raw).count.load(AtomicOrdering::Relaxed) }, 1);

        drop(ptr);
        assert_eq!(drop_count.load(AtomicOrdering::Relaxed), 1);
    }

    #[test]
    fn test_new_without_increment() {
        let drop_count = AtomicUsize::new(0);
        let raw = make_test_obj(42, &drop_count);
        unsafe {
            (*raw).count.store(1, AtomicOrdering::Relaxed);
        }

        let ptr = DelegatedCountPtr::new_no_increment(DoNotIncrementTag, raw);
        assert!(!ptr.is_null());
        assert_eq!(unsafe { (*raw).count.load(AtomicOrdering::Relaxed) }, 1);

        drop(ptr);
        assert_eq!(drop_count.load(AtomicOrdering::Relaxed), 1);
    }

    #[test]
    fn test_clone_increments() {
        let drop_count = AtomicUsize::new(0);
        let raw = make_test_obj(42, &drop_count);

        let ptr1 = DelegatedCountPtr::new(IncrementTag, raw);
        assert_eq!(unsafe { (*raw).count.load(AtomicOrdering::Relaxed) }, 1);

        let ptr2 = ptr1.clone();
        assert_eq!(unsafe { (*raw).count.load(AtomicOrdering::Relaxed) }, 2);

        drop(ptr1);
        assert_eq!(unsafe { (*raw).count.load(AtomicOrdering::Relaxed) }, 1);
        assert_eq!(drop_count.load(AtomicOrdering::Relaxed), 0);

        drop(ptr2);
        assert_eq!(drop_count.load(AtomicOrdering::Relaxed), 1);
    }

    #[test]
    fn test_deref() {
        let drop_count = AtomicUsize::new(0);
        let raw = make_test_obj(42, &drop_count);
        let ptr = DelegatedCountPtr::new(IncrementTag, raw);

        assert_eq!(ptr.value, 42);
        drop(ptr);
    }

    #[test]
    fn test_as_ref() {
        let drop_count = AtomicUsize::new(0);
        let raw = make_test_obj(42, &drop_count);
        let ptr = DelegatedCountPtr::new(IncrementTag, raw);

        assert_eq!(ptr.as_ref().map(|o| o.value), Some(42));

        let null_ptr: DelegatedCountPtr<TestObj> = DelegatedCountPtr::null();
        assert!(null_ptr.as_ref().is_none());

        drop(ptr);
    }

    #[test]
    fn test_reset() {
        let drop_count = AtomicUsize::new(0);
        let raw = make_test_obj(42, &drop_count);
        let mut ptr = DelegatedCountPtr::new(IncrementTag, raw);

        assert!(!ptr.is_null());
        ptr.reset();
        assert!(ptr.is_null());
        assert_eq!(drop_count.load(AtomicOrdering::Relaxed), 1);
    }

    #[test]
    fn test_swap() {
        let drop_count1 = AtomicUsize::new(0);
        let drop_count2 = AtomicUsize::new(0);
        let raw1 = make_test_obj(1, &drop_count1);
        let raw2 = make_test_obj(2, &drop_count2);

        let mut ptr1 = DelegatedCountPtr::new(IncrementTag, raw1);
        let mut ptr2 = DelegatedCountPtr::new(IncrementTag, raw2);

        assert_eq!(ptr1.value, 1);
        assert_eq!(ptr2.value, 2);

        ptr1.swap(&mut ptr2);

        assert_eq!(ptr1.value, 2);
        assert_eq!(ptr2.value, 1);

        drop(ptr1);
        drop(ptr2);
    }

    #[test]
    fn test_equality() {
        let drop_count = AtomicUsize::new(0);
        let raw = make_test_obj(42, &drop_count);

        let ptr1 = DelegatedCountPtr::new(IncrementTag, raw);
        let ptr2 = ptr1.clone();

        assert_eq!(ptr1, ptr2);
        assert_eq!(ptr1.get(), ptr2.get());

        let null1: DelegatedCountPtr<TestObj> = DelegatedCountPtr::null();
        let null2: DelegatedCountPtr<TestObj> = DelegatedCountPtr::null();
        assert_eq!(null1, null2);

        drop(ptr1);
        drop(ptr2);
    }

    #[test]
    fn test_ordering() {
        let drop_count1 = AtomicUsize::new(0);
        let drop_count2 = AtomicUsize::new(0);
        let raw1 = make_test_obj(1, &drop_count1);
        let raw2 = make_test_obj(2, &drop_count2);

        let ptr1 = DelegatedCountPtr::new(IncrementTag, raw1);
        let ptr2 = DelegatedCountPtr::new(IncrementTag, raw2);

        // Ordering is based on pointer address
        let cmp = ptr1.cmp(&ptr2);
        assert!(cmp == Ordering::Less || cmp == Ordering::Greater);

        drop(ptr1);
        drop(ptr2);
    }

    #[test]
    fn test_hash() {
        use std::collections::hash_map::DefaultHasher;

        let drop_count = AtomicUsize::new(0);
        let raw = make_test_obj(42, &drop_count);

        let ptr1 = DelegatedCountPtr::new(IncrementTag, raw);
        let ptr2 = ptr1.clone();

        let hash1 = {
            let mut h = DefaultHasher::new();
            ptr1.hash(&mut h);
            h.finish()
        };
        let hash2 = {
            let mut h = DefaultHasher::new();
            ptr2.hash(&mut h);
            h.finish()
        };

        assert_eq!(hash1, hash2);

        drop(ptr1);
        drop(ptr2);
    }

    #[test]
    fn test_make_delegated_count_ptr() {
        static DROP_FLAG: AtomicUsize = AtomicUsize::new(0);

        struct SimpleObj {
            count: AtomicUsize,
            value: i32,
        }

        impl DelegatedCount for SimpleObj {
            fn increment(ptr: *mut Self) {
                unsafe {
                    (*ptr).count.fetch_add(1, AtomicOrdering::Relaxed);
                }
            }

            fn decrement(ptr: *mut Self) {
                unsafe {
                    if (*ptr).count.fetch_sub(1, AtomicOrdering::Release) == 1 {
                        std::sync::atomic::fence(AtomicOrdering::Acquire);
                        drop(Box::from_raw(ptr));
                        DROP_FLAG.fetch_add(1, AtomicOrdering::Relaxed);
                    }
                }
            }
        }

        DROP_FLAG.store(0, AtomicOrdering::Relaxed);

        let ptr = make_delegated_count_ptr(SimpleObj {
            count: AtomicUsize::new(0),
            value: 123,
        });

        assert_eq!(ptr.value, 123);
        drop(ptr);
        assert_eq!(DROP_FLAG.load(AtomicOrdering::Relaxed), 1);
    }

    #[test]
    fn test_null_increment_no_call() {
        // Incrementing a null pointer should not panic
        let ptr: DelegatedCountPtr<TestObj> =
            DelegatedCountPtr::new(IncrementTag, std::ptr::null_mut());
        assert!(ptr.is_null());
    }

    #[test]
    fn test_clone_null() {
        let ptr: DelegatedCountPtr<TestObj> = DelegatedCountPtr::null();
        let cloned = ptr.clone();
        assert!(cloned.is_null());
    }

    #[test]
    fn test_multiple_references() {
        let drop_count = AtomicUsize::new(0);
        let raw = make_test_obj(42, &drop_count);

        let ptr1 = DelegatedCountPtr::new(IncrementTag, raw);
        let ptr2 = ptr1.clone();
        let ptr3 = ptr2.clone();

        assert_eq!(unsafe { (*raw).count.load(AtomicOrdering::Relaxed) }, 3);

        drop(ptr1);
        assert_eq!(unsafe { (*raw).count.load(AtomicOrdering::Relaxed) }, 2);
        assert_eq!(drop_count.load(AtomicOrdering::Relaxed), 0);

        drop(ptr2);
        assert_eq!(unsafe { (*raw).count.load(AtomicOrdering::Relaxed) }, 1);
        assert_eq!(drop_count.load(AtomicOrdering::Relaxed), 0);

        drop(ptr3);
        assert_eq!(drop_count.load(AtomicOrdering::Relaxed), 1);
    }
}
