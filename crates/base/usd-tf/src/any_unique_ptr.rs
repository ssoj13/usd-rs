//! Type-erased unique pointer holder.
//!
//! Provides a simple type-erased container that supports only destruction,
//! moves, and immutable, untyped access to the held value.
//!
//! This is designed for holding fallback or default values in error cases
//! where the values are often instantiated but rarely accessed.
//!
//! # Examples
//!
//! ```
//! use usd_tf::any_unique_ptr::AnyUniquePtr;
//!
//! // Create with default value
//! let ptr = AnyUniquePtr::new::<i32>();
//! assert!(!ptr.is_empty());
//!
//! // Create with specific value
//! let ptr = AnyUniquePtr::with_value(42i32);
//! let value = ptr.get::<i32>();
//! assert_eq!(*value.unwrap(), 42);
//! ```

use std::any::TypeId;

/// Deleter function type for type-erased destruction.
type Deleter = unsafe fn(*mut u8);

/// Type-erased unique pointer.
///
/// A simple container that owns a heap-allocated value of any type,
/// providing only destruction and immutable access.
///
/// Unlike `Box<dyn Any>`, this has minimal overhead and doesn't require
/// the held type to implement any traits beyond `Send + Sync`.
pub struct AnyUniquePtr {
    /// Raw pointer to the owned data.
    ptr: *mut u8,
    /// Function to properly delete the data.
    deleter: Deleter,
    /// TypeId of the stored data for safe downcasting.
    type_id: TypeId,
}

// SAFETY: AnyUniquePtr owns its data exclusively and only holds Send + Sync types.
#[allow(unsafe_code)]
unsafe impl Send for AnyUniquePtr {}

#[allow(unsafe_code)]
unsafe impl Sync for AnyUniquePtr {}

impl AnyUniquePtr {
    /// Creates a new AnyUniquePtr with a default-constructed value of type T.
    ///
    /// # Panics
    ///
    /// This function does not panic, but may abort on allocation failure.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::any_unique_ptr::AnyUniquePtr;
    ///
    /// let ptr = AnyUniquePtr::new::<String>();
    /// let s = ptr.get::<String>().unwrap();
    /// assert!(s.is_empty());
    /// ```
    pub fn new<T: Default + Send + Sync + 'static>() -> Self {
        Self::with_value(T::default())
    }

    /// Creates a new AnyUniquePtr with the given value.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::any_unique_ptr::AnyUniquePtr;
    ///
    /// let ptr = AnyUniquePtr::with_value(vec![1, 2, 3]);
    /// let v = ptr.get::<Vec<i32>>().unwrap();
    /// assert_eq!(v.len(), 3);
    /// ```
    pub fn with_value<T: Send + Sync + 'static>(value: T) -> Self {
        let boxed = Box::new(value);
        let ptr = Box::into_raw(boxed) as *mut u8;
        Self {
            ptr,
            deleter: delete::<T>,
            type_id: TypeId::of::<T>(),
        }
    }

    /// Creates an empty AnyUniquePtr (null).
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::any_unique_ptr::AnyUniquePtr;
    ///
    /// let ptr = AnyUniquePtr::empty();
    /// assert!(ptr.is_empty());
    /// ```
    pub fn empty() -> Self {
        Self {
            ptr: std::ptr::null_mut(),
            deleter: delete_null,
            type_id: TypeId::of::<()>(),
        }
    }

    /// Returns true if this AnyUniquePtr is empty (holds no value).
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.ptr.is_null()
    }

    /// Returns a raw pointer to the owned data.
    ///
    /// Returns null if empty.
    #[inline]
    pub fn as_ptr(&self) -> *const u8 {
        self.ptr
    }

    /// Returns a reference to the owned value if the type matches.
    ///
    /// # Safety
    ///
    /// The type T must match the type used when creating this AnyUniquePtr.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::any_unique_ptr::AnyUniquePtr;
    ///
    /// let ptr = AnyUniquePtr::with_value(42i32);
    /// assert_eq!(*ptr.get::<i32>().unwrap(), 42);
    /// assert!(ptr.get::<String>().is_none()); // Wrong type
    /// ```
    pub fn get<T: 'static>(&self) -> Option<&T> {
        if self.ptr.is_null() || self.type_id != TypeId::of::<T>() {
            None
        } else {
            // SAFETY: We verified the type matches and pointer is non-null.
            #[allow(unsafe_code)]
            Some(unsafe { &*(self.ptr as *const T) })
        }
    }

    /// Returns the TypeId of the stored value.
    ///
    /// Returns TypeId of () if empty.
    #[inline]
    pub fn type_id(&self) -> TypeId {
        self.type_id
    }

    /// Consumes this AnyUniquePtr and returns the contained value if type matches.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::any_unique_ptr::AnyUniquePtr;
    ///
    /// let ptr = AnyUniquePtr::with_value(String::from("hello"));
    /// let s: String = ptr.into_inner().unwrap();
    /// assert_eq!(s, "hello");
    /// ```
    pub fn into_inner<T: 'static>(self) -> Option<T> {
        if self.ptr.is_null() || self.type_id != TypeId::of::<T>() {
            None
        } else {
            let ptr = self.ptr as *mut T;
            // Prevent destructor from running since we're taking ownership.
            std::mem::forget(self);
            // SAFETY: We verified the type matches and pointer is non-null.
            #[allow(unsafe_code)]
            Some(unsafe { *Box::from_raw(ptr) })
        }
    }
}

impl Drop for AnyUniquePtr {
    fn drop(&mut self) {
        // SAFETY: deleter is always valid and handles null pointers.
        #[allow(unsafe_code)]
        unsafe {
            (self.deleter)(self.ptr);
        }
    }
}

impl Default for AnyUniquePtr {
    fn default() -> Self {
        Self::empty()
    }
}

impl std::fmt::Debug for AnyUniquePtr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_empty() {
            write!(f, "AnyUniquePtr(empty)")
        } else {
            write!(f, "AnyUniquePtr(ptr={:p})", self.ptr)
        }
    }
}

/// Deleter function for type T.
///
/// # Safety
///
/// The pointer must have been allocated as Box<T> or be null.
#[allow(unsafe_code)]
unsafe fn delete<T>(ptr: *mut u8) {
    // SAFETY: ptr was allocated as Box<T> by with_value, or is null
    unsafe {
        if !ptr.is_null() {
            drop(Box::from_raw(ptr as *mut T));
        }
    }
}

/// Deleter function for null pointers (no-op).
///
/// # Safety
///
/// This function is always safe to call.
#[allow(unsafe_code)]
unsafe fn delete_null(_ptr: *mut u8) {
    // No-op for null pointers.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty() {
        let ptr = AnyUniquePtr::empty();
        assert!(ptr.is_empty());
        assert!(ptr.as_ptr().is_null());
        assert!(ptr.get::<i32>().is_none());
    }

    #[test]
    fn test_new_default() {
        let ptr = AnyUniquePtr::new::<i32>();
        assert!(!ptr.is_empty());
        assert_eq!(*ptr.get::<i32>().unwrap(), 0);
    }

    #[test]
    fn test_with_value() {
        let ptr = AnyUniquePtr::with_value(42i32);
        assert!(!ptr.is_empty());
        assert_eq!(*ptr.get::<i32>().unwrap(), 42);
    }

    #[test]
    fn test_string_value() {
        let ptr = AnyUniquePtr::with_value(String::from("hello"));
        let s = ptr.get::<String>().unwrap();
        assert_eq!(s, "hello");
    }

    #[test]
    fn test_wrong_type() {
        let ptr = AnyUniquePtr::with_value(42i32);
        assert!(ptr.get::<String>().is_none());
        assert!(ptr.get::<i64>().is_none());
        assert!(ptr.get::<u32>().is_none());
    }

    #[test]
    fn test_into_inner() {
        let ptr = AnyUniquePtr::with_value(vec![1, 2, 3]);
        let v: Vec<i32> = ptr.into_inner().unwrap();
        assert_eq!(v, vec![1, 2, 3]);
    }

    #[test]
    fn test_into_inner_wrong_type() {
        let ptr = AnyUniquePtr::with_value(42i32);
        let result: Option<String> = ptr.into_inner();
        assert!(result.is_none());
    }

    #[test]
    fn test_type_id() {
        let ptr = AnyUniquePtr::with_value(42i32);
        assert_eq!(ptr.type_id(), TypeId::of::<i32>());
    }

    #[test]
    fn test_default() {
        let ptr = AnyUniquePtr::default();
        assert!(ptr.is_empty());
    }

    #[test]
    fn test_debug() {
        let empty = AnyUniquePtr::empty();
        assert!(format!("{:?}", empty).contains("empty"));

        let ptr = AnyUniquePtr::with_value(42);
        let debug = format!("{:?}", ptr);
        assert!(debug.contains("ptr="));
    }

    #[test]
    fn test_drop_complex_type() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let drop_count = Arc::new(AtomicUsize::new(0));

        struct DropCounter {
            counter: Arc<AtomicUsize>,
        }

        impl Drop for DropCounter {
            fn drop(&mut self) {
                self.counter.fetch_add(1, Ordering::SeqCst);
            }
        }

        {
            let _ptr = AnyUniquePtr::with_value(DropCounter {
                counter: drop_count.clone(),
            });
            assert_eq!(drop_count.load(Ordering::SeqCst), 0);
        }

        assert_eq!(drop_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_multiple_values() {
        let p1 = AnyUniquePtr::with_value(1i32);
        let p2 = AnyUniquePtr::with_value(2i32);
        let p3 = AnyUniquePtr::with_value(3i32);

        assert_eq!(*p1.get::<i32>().unwrap(), 1);
        assert_eq!(*p2.get::<i32>().unwrap(), 2);
        assert_eq!(*p3.get::<i32>().unwrap(), 3);
    }

    #[test]
    fn test_struct_value() {
        #[derive(Debug, PartialEq)]
        struct Point {
            x: f32,
            y: f32,
        }

        let ptr = AnyUniquePtr::with_value(Point { x: 1.0, y: 2.0 });
        let p = ptr.get::<Point>().unwrap();
        assert_eq!(p.x, 1.0);
        assert_eq!(p.y, 2.0);
    }
}
