//! Type functions for pointer manipulation.
//!
//! Provides traits for uniformly working with different pointer types,
//! allowing code to work with both raw pointers, references, and smart pointers
//! in a generic way.
//!
//! # Overview
//!
//! The [`TypeFunctions`] trait provides methods for:
//! - Getting a raw pointer from any pointer-like type
//! - Constructing a type from a raw pointer
//! - Checking if a pointer-like value is null
//!
//! # Examples
//!
//! ```
//! use usd_tf::type_functions::TypeFunctions;
//!
//! // Works with references
//! let value = 42i32;
//! let reference = &value;
//! let ptr = reference.get_raw_ptr();
//! assert!(!ptr.is_null());
//!
//! // Works with Box
//! let boxed = Box::new(42i32);
//! let ptr2 = boxed.get_raw_ptr();
//! assert!(!ptr2.is_null());
//! ```

use std::rc::Rc;
use std::sync::Arc;

/// Trait for types that can provide a raw pointer to their contents.
///
/// This trait enables generic code to work uniformly with different
/// pointer-like types (references, raw pointers, smart pointers).
pub trait TypeFunctions {
    /// The target type that this pointer points to.
    type Target: ?Sized;

    /// Get a raw const pointer to the underlying value.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::type_functions::TypeFunctions;
    ///
    /// let value = 42i32;
    /// let reference = &value;
    /// let ptr = reference.get_raw_ptr();
    /// assert_eq!(unsafe { *ptr }, 42);
    /// ```
    fn get_raw_ptr(&self) -> *const Self::Target;

    /// Check if this pointer-like value is null or empty.
    ///
    /// For owned values and references, this always returns `false`.
    /// For pointers and Option types, this checks for null/None.
    fn is_null(&self) -> bool;
}

/// Trait for constructing a type from a raw pointer.
///
/// This is the inverse operation of [`TypeFunctions::get_raw_ptr`].
///
/// # Safety
///
/// Implementations must ensure that the pointer is valid and properly aligned.
pub trait ConstructFromRawPtr<T>: Sized {
    /// Construct this type from a raw pointer.
    ///
    /// # Safety
    ///
    /// The pointer must be valid, properly aligned, and point to valid data.
    #[allow(unsafe_code)]
    unsafe fn construct_from_raw_ptr(ptr: *const T) -> Self;
}

// Implementation for references
impl<T> TypeFunctions for &T {
    type Target = T;

    #[inline]
    fn get_raw_ptr(&self) -> *const T {
        *self as *const T
    }

    #[inline]
    fn is_null(&self) -> bool {
        false // References are never null
    }
}

impl<T> TypeFunctions for &mut T {
    type Target = T;

    #[inline]
    fn get_raw_ptr(&self) -> *const T {
        *self as *const T
    }

    #[inline]
    fn is_null(&self) -> bool {
        false // References are never null
    }
}

// Implementation for raw pointers
impl<T> TypeFunctions for *const T {
    type Target = T;

    #[inline]
    fn get_raw_ptr(&self) -> *const T {
        *self
    }

    #[inline]
    fn is_null(&self) -> bool {
        (*self).is_null()
    }
}

impl<T> TypeFunctions for *mut T {
    type Target = T;

    #[inline]
    fn get_raw_ptr(&self) -> *const T {
        *self as *const T
    }

    #[inline]
    fn is_null(&self) -> bool {
        (*self).is_null()
    }
}

// Implementation for Option<&T>
impl<T> TypeFunctions for Option<&T> {
    type Target = T;

    #[inline]
    fn get_raw_ptr(&self) -> *const T {
        match self {
            Some(r) => *r as *const T,
            None => std::ptr::null(),
        }
    }

    #[inline]
    fn is_null(&self) -> bool {
        self.is_none()
    }
}

impl<T> TypeFunctions for Option<&mut T> {
    type Target = T;

    #[inline]
    fn get_raw_ptr(&self) -> *const T {
        match self {
            Some(r) => *r as *const T,
            None => std::ptr::null(),
        }
    }

    #[inline]
    fn is_null(&self) -> bool {
        self.is_none()
    }
}

// Implementation for Box<T>
impl<T> TypeFunctions for Box<T> {
    type Target = T;

    #[inline]
    fn get_raw_ptr(&self) -> *const T {
        self.as_ref() as *const T
    }

    #[inline]
    fn is_null(&self) -> bool {
        false // Box is never null
    }
}

// Implementation for Rc<T>
impl<T> TypeFunctions for Rc<T> {
    type Target = T;

    #[inline]
    fn get_raw_ptr(&self) -> *const T {
        Rc::as_ptr(self)
    }

    #[inline]
    fn is_null(&self) -> bool {
        false // Rc is never null
    }
}

// Implementation for Arc<T>
impl<T> TypeFunctions for Arc<T> {
    type Target = T;

    #[inline]
    fn get_raw_ptr(&self) -> *const T {
        Arc::as_ptr(self)
    }

    #[inline]
    fn is_null(&self) -> bool {
        false // Arc is never null
    }
}

// Implementation for Option<Box<T>>
impl<T> TypeFunctions for Option<Box<T>> {
    type Target = T;

    #[inline]
    fn get_raw_ptr(&self) -> *const T {
        match self {
            Some(b) => b.as_ref() as *const T,
            None => std::ptr::null(),
        }
    }

    #[inline]
    fn is_null(&self) -> bool {
        self.is_none()
    }
}

// ConstructFromRawPtr implementations

impl<T> ConstructFromRawPtr<T> for &T {
    #[inline]
    #[allow(unsafe_code)]
    unsafe fn construct_from_raw_ptr(ptr: *const T) -> Self {
        // SAFETY: caller guarantees ptr is valid, aligned, and points to valid data
        unsafe { &*ptr }
    }
}

impl<T> ConstructFromRawPtr<T> for *const T {
    #[inline]
    #[allow(unsafe_code)]
    unsafe fn construct_from_raw_ptr(ptr: *const T) -> Self {
        ptr
    }
}

impl<T> ConstructFromRawPtr<T> for *mut T {
    #[inline]
    #[allow(unsafe_code)]
    unsafe fn construct_from_raw_ptr(ptr: *const T) -> Self {
        ptr as *mut T
    }
}

/// Get a raw pointer from any pointer-like type.
///
/// This is a convenience function that calls [`TypeFunctions::get_raw_ptr`].
///
/// # Examples
///
/// ```
/// use usd_tf::type_functions::get_raw_ptr;
///
/// let value = 42i32;
/// let reference = &value;
/// let ptr = get_raw_ptr(&reference);
/// assert_eq!(unsafe { *ptr }, 42);
/// ```
#[inline]
pub fn get_raw_ptr<P: TypeFunctions>(p: &P) -> *const P::Target {
    p.get_raw_ptr()
}

/// Check if a pointer-like type is null.
///
/// This is a convenience function that calls [`TypeFunctions::is_null`].
///
/// # Examples
///
/// ```
/// use usd_tf::type_functions::is_null;
///
/// let value = 42i32;
/// assert!(!is_null(&&value));
///
/// let null_ptr: *const i32 = std::ptr::null();
/// assert!(is_null(&null_ptr));
/// ```
#[inline]
pub fn is_null<P: TypeFunctions>(p: &P) -> bool {
    p.is_null()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reference() {
        let value = 42i32;
        let r = &value;

        let ptr = r.get_raw_ptr();
        assert!(!ptr.is_null());
        assert_eq!(unsafe { *ptr }, 42);
        assert!(!r.is_null());
    }

    #[test]
    fn test_mut_reference() {
        let mut value = 42i32;
        let r = &mut value;

        let ptr = r.get_raw_ptr();
        assert!(!ptr.is_null());
        assert_eq!(unsafe { *ptr }, 42);
        assert!(!r.is_null());
    }

    #[test]
    fn test_const_pointer() {
        let value = 42i32;
        let p: *const i32 = &value;

        let ptr = p.get_raw_ptr();
        assert_eq!(ptr, p);
        assert!(!p.is_null());
    }

    #[test]
    fn test_mut_pointer() {
        let mut value = 42i32;
        let p: *mut i32 = &mut value;

        let ptr = p.get_raw_ptr();
        assert_eq!(ptr, p as *const i32);
        assert!(!p.is_null());
    }

    #[test]
    fn test_null_pointer() {
        let p: *const i32 = std::ptr::null();
        assert!(p.is_null());
        assert!(p.get_raw_ptr().is_null());
    }

    #[test]
    fn test_option_some() {
        let value = 42i32;
        let opt = Some(&value);

        let ptr = opt.get_raw_ptr();
        assert!(!ptr.is_null());
        assert_eq!(unsafe { *ptr }, 42);
        assert!(!opt.is_null());
    }

    #[test]
    fn test_option_none() {
        let opt: Option<&i32> = None;

        let ptr = opt.get_raw_ptr();
        assert!(ptr.is_null());
        assert!(opt.is_null());
    }

    #[test]
    fn test_box() {
        let b = Box::new(42i32);

        let ptr = b.get_raw_ptr();
        assert!(!ptr.is_null());
        assert_eq!(unsafe { *ptr }, 42);
        assert!(!b.is_null());
    }

    #[test]
    fn test_rc() {
        let rc = Rc::new(42i32);

        let ptr = rc.get_raw_ptr();
        assert!(!ptr.is_null());
        assert_eq!(unsafe { *ptr }, 42);
        assert!(!rc.is_null());
    }

    #[test]
    fn test_arc() {
        let arc = Arc::new(42i32);

        let ptr = arc.get_raw_ptr();
        assert!(!ptr.is_null());
        assert_eq!(unsafe { *ptr }, 42);
        assert!(!arc.is_null());
    }

    #[test]
    fn test_option_box() {
        let opt = Some(Box::new(42i32));
        assert!(!opt.is_null());

        let none: Option<Box<i32>> = None;
        assert!(none.is_null());
    }

    #[test]
    fn test_get_raw_ptr_function() {
        let value = 42i32;
        let ptr = get_raw_ptr(&&value);
        assert!(!ptr.is_null());
    }

    #[test]
    fn test_is_null_function() {
        let value = 42i32;
        assert!(!is_null(&&value));

        let null: *const i32 = std::ptr::null();
        assert!(is_null(&null));
    }

    #[test]
    fn test_construct_from_raw_ptr_reference() {
        let value = 42i32;
        let ptr: *const i32 = &value;

        let r: &i32 = unsafe { ConstructFromRawPtr::construct_from_raw_ptr(ptr) };
        assert_eq!(*r, 42);
    }

    #[test]
    fn test_construct_from_raw_ptr_pointer() {
        let value = 42i32;
        let ptr: *const i32 = &value;

        let p: *const i32 = unsafe { ConstructFromRawPtr::construct_from_raw_ptr(ptr) };
        assert_eq!(p, ptr);
    }

    #[test]
    fn test_generic_function() {
        fn process<P: TypeFunctions<Target = i32>>(p: &P) -> i32 {
            if p.is_null() {
                0
            } else {
                unsafe { *p.get_raw_ptr() }
            }
        }

        let value = 42i32;
        assert_eq!(process(&&value), 42);

        let ptr: *const i32 = &value;
        assert_eq!(process(&ptr), 42);

        let null: *const i32 = std::ptr::null();
        assert_eq!(process(&null), 0);
    }
}
