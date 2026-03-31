//! Safe type comparison utilities.
//!
//! Provides safe type comparison and casting utilities, similar to C++ RTTI
//! but using Rust's [`TypeId`] and [`Any`] traits.
//!
//! # Examples
//!
//! ```
//! use usd_tf::safe_type_compare::{safe_type_compare, safe_downcast_ref};
//! use std::any::Any;
//!
//! // Compare types
//! assert!(safe_type_compare::<i32, i32>());
//! assert!(!safe_type_compare::<i32, i64>());
//!
//! // Safe downcasting
//! let value: Box<dyn Any> = Box::new(42i32);
//! let downcasted = safe_downcast_ref::<i32>(&*value);
//! assert_eq!(downcasted, Some(&42));
//! ```

use std::any::{Any, TypeId};

/// Safely compare two types.
///
/// Returns `true` if `T1` and `T2` are the same type.
///
/// # Examples
///
/// ```
/// use usd_tf::safe_type_compare::safe_type_compare;
///
/// assert!(safe_type_compare::<i32, i32>());
/// assert!(safe_type_compare::<String, String>());
/// assert!(!safe_type_compare::<i32, i64>());
/// assert!(!safe_type_compare::<&str, String>());
/// ```
#[inline]
pub fn safe_type_compare<T1: 'static, T2: 'static>() -> bool {
    TypeId::of::<T1>() == TypeId::of::<T2>()
}

/// Compare two `TypeId` values for equality.
///
/// # Examples
///
/// ```
/// use usd_tf::safe_type_compare::safe_type_id_compare;
/// use std::any::TypeId;
///
/// let id1 = TypeId::of::<i32>();
/// let id2 = TypeId::of::<i32>();
/// let id3 = TypeId::of::<i64>();
///
/// assert!(safe_type_id_compare(id1, id2));
/// assert!(!safe_type_id_compare(id1, id3));
/// ```
#[inline]
pub fn safe_type_id_compare(t1: TypeId, t2: TypeId) -> bool {
    t1 == t2
}

/// Safely downcast a reference to `dyn Any` to a concrete type.
///
/// Returns `Some(&T)` if the underlying value is of type `T`, or `None` otherwise.
///
/// # Examples
///
/// ```
/// use usd_tf::safe_type_compare::safe_downcast_ref;
/// use std::any::Any;
///
/// let value: &dyn Any = &42i32;
/// assert_eq!(safe_downcast_ref::<i32>(value), Some(&42));
/// assert_eq!(safe_downcast_ref::<i64>(value), None);
///
/// let text: &dyn Any = &"hello";
/// assert_eq!(safe_downcast_ref::<&str>(text), Some(&"hello"));
/// ```
#[inline]
pub fn safe_downcast_ref<T: Any>(value: &dyn Any) -> Option<&T> {
    value.downcast_ref::<T>()
}

/// Safely downcast a mutable reference to `dyn Any` to a concrete type.
///
/// Returns `Some(&mut T)` if the underlying value is of type `T`, or `None` otherwise.
///
/// # Examples
///
/// ```
/// use usd_tf::safe_type_compare::{safe_downcast_mut, safe_downcast_ref};
/// use std::any::Any;
///
/// let mut value: Box<dyn Any> = Box::new(42i32);
/// if let Some(n) = safe_downcast_mut::<i32>(&mut *value) {
///     *n = 100;
/// }
/// assert_eq!(safe_downcast_ref::<i32>(&*value), Some(&100));
/// ```
#[inline]
pub fn safe_downcast_mut<T: Any>(value: &mut dyn Any) -> Option<&mut T> {
    value.downcast_mut::<T>()
}

/// Safely downcast a `Box<dyn Any>` to a concrete type.
///
/// Returns `Ok(Box<T>)` if the underlying value is of type `T`,
/// or `Err(Box<dyn Any>)` if the cast failed.
///
/// # Examples
///
/// ```
/// use usd_tf::safe_type_compare::safe_downcast_box;
/// use std::any::Any;
///
/// let value: Box<dyn Any> = Box::new(42i32);
/// let result = safe_downcast_box::<i32>(value);
/// assert_eq!(result.ok().map(|b| *b), Some(42));
///
/// let value: Box<dyn Any> = Box::new("hello");
/// let result = safe_downcast_box::<i32>(value);
/// assert!(result.is_err());
/// ```
#[inline]
pub fn safe_downcast_box<T: Any>(value: Box<dyn Any>) -> Result<Box<T>, Box<dyn Any>> {
    value.downcast::<T>()
}

/// Safely downcast a `Box<dyn Any + Send>` to a concrete type.
///
/// Returns `Ok(Box<T>)` if the underlying value is of type `T`,
/// or `Err(Box<dyn Any + Send>)` if the cast failed.
///
/// # Examples
///
/// ```
/// use usd_tf::safe_type_compare::safe_downcast_box_send;
/// use std::any::Any;
///
/// let value: Box<dyn Any + Send> = Box::new(42i32);
/// let result = safe_downcast_box_send::<i32>(value);
/// assert_eq!(result.ok().map(|b| *b), Some(42));
/// ```
#[inline]
pub fn safe_downcast_box_send<T: Any + Send>(
    value: Box<dyn Any + Send>,
) -> Result<Box<T>, Box<dyn Any + Send>> {
    value.downcast::<T>()
}

/// Check if a `dyn Any` value is of a specific type.
///
/// # Examples
///
/// ```
/// use usd_tf::safe_type_compare::is_type;
/// use std::any::Any;
///
/// let value: &dyn Any = &42i32;
/// assert!(is_type::<i32>(value));
/// assert!(!is_type::<i64>(value));
/// ```
#[inline]
pub fn is_type<T: Any>(value: &dyn Any) -> bool {
    value.is::<T>()
}

/// Get the `TypeId` of a value's underlying type.
///
/// # Examples
///
/// ```
/// use usd_tf::safe_type_compare::type_id_of_val;
/// use std::any::TypeId;
///
/// let value: &dyn std::any::Any = &42i32;
/// assert_eq!(type_id_of_val(value), TypeId::of::<i32>());
/// ```
#[inline]
pub fn type_id_of_val(value: &dyn Any) -> TypeId {
    value.type_id()
}

/// Trait for types that can be safely compared by type.
///
/// This is automatically implemented for all `'static` types.
pub trait SafeTypeCompare: 'static {
    /// Check if this type is the same as another type.
    fn is_same_type<Other: 'static>() -> bool {
        TypeId::of::<Self>() == TypeId::of::<Other>()
    }

    /// Get the `TypeId` for this type.
    fn static_type_id() -> TypeId {
        TypeId::of::<Self>()
    }
}

// Blanket implementation for all 'static types
impl<T: 'static> SafeTypeCompare for T {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_type_compare_same() {
        assert!(safe_type_compare::<i32, i32>());
        assert!(safe_type_compare::<String, String>());
        assert!(safe_type_compare::<Vec<u8>, Vec<u8>>());
    }

    #[test]
    fn test_safe_type_compare_different() {
        assert!(!safe_type_compare::<i32, i64>());
        assert!(!safe_type_compare::<&str, String>());
        assert!(!safe_type_compare::<Vec<u8>, Vec<i8>>());
    }

    #[test]
    fn test_safe_type_id_compare() {
        let id1 = TypeId::of::<i32>();
        let id2 = TypeId::of::<i32>();
        let id3 = TypeId::of::<i64>();

        assert!(safe_type_id_compare(id1, id2));
        assert!(!safe_type_id_compare(id1, id3));
    }

    #[test]
    fn test_safe_downcast_ref() {
        let value: &dyn Any = &42i32;
        assert_eq!(safe_downcast_ref::<i32>(value), Some(&42));
        assert_eq!(safe_downcast_ref::<i64>(value), None);
        assert_eq!(safe_downcast_ref::<String>(value), None);
    }

    #[test]
    fn test_safe_downcast_mut() {
        let mut value: Box<dyn Any> = Box::new(42i32);
        if let Some(n) = safe_downcast_mut::<i32>(&mut *value) {
            *n = 100;
        }
        assert_eq!(safe_downcast_ref::<i32>(&*value), Some(&100));
    }

    #[test]
    fn test_safe_downcast_box_success() {
        let value: Box<dyn Any> = Box::new(42i32);
        let result = safe_downcast_box::<i32>(value);
        assert!(result.is_ok());
        assert_eq!(*result.unwrap(), 42);
    }

    #[test]
    fn test_safe_downcast_box_failure() {
        let value: Box<dyn Any> = Box::new("hello");
        let result = safe_downcast_box::<i32>(value);
        assert!(result.is_err());
    }

    #[test]
    fn test_safe_downcast_box_send() {
        let value: Box<dyn Any + Send> = Box::new(42i32);
        let result = safe_downcast_box_send::<i32>(value);
        assert!(result.is_ok());
        assert_eq!(*result.unwrap(), 42);
    }

    #[test]
    fn test_is_type() {
        let value: &dyn Any = &42i32;
        assert!(is_type::<i32>(value));
        assert!(!is_type::<i64>(value));
    }

    #[test]
    fn test_type_id_of_val() {
        let value: &dyn Any = &42i32;
        assert_eq!(type_id_of_val(value), TypeId::of::<i32>());
    }

    #[test]
    fn test_safe_type_compare_trait() {
        assert!(i32::is_same_type::<i32>());
        assert!(!i32::is_same_type::<i64>());
        assert_eq!(i32::static_type_id(), TypeId::of::<i32>());
    }

    #[test]
    fn test_with_custom_types() {
        struct MyType {
            value: i32,
        }

        let my_val = MyType { value: 42 };
        let boxed: Box<dyn Any> = Box::new(my_val);

        assert!(is_type::<MyType>(&*boxed));
        assert!(!is_type::<i32>(&*boxed));

        let downcasted = safe_downcast_ref::<MyType>(&*boxed);
        assert!(downcasted.is_some());
        assert_eq!(downcasted.unwrap().value, 42);
    }

    #[test]
    fn test_with_generics() {
        assert!(safe_type_compare::<Vec<i32>, Vec<i32>>());
        assert!(!safe_type_compare::<Vec<i32>, Vec<i64>>());
        assert!(!safe_type_compare::<Option<i32>, Option<i64>>());
    }

    #[test]
    fn test_with_references() {
        // Reference types are different from owned types
        assert!(!safe_type_compare::<&i32, i32>());
        assert!(!safe_type_compare::<&str, String>());
        assert!(safe_type_compare::<&'static str, &'static str>());
    }

    #[test]
    fn test_with_trait_objects() {
        #[allow(dead_code)]
        trait MyTrait {}
        impl MyTrait for i32 {}
        impl MyTrait for String {}

        let val_i32: Box<dyn Any> = Box::new(42i32);
        let val_string: Box<dyn Any> = Box::new(String::from("hello"));

        assert!(is_type::<i32>(&*val_i32));
        assert!(!is_type::<String>(&*val_i32));

        assert!(is_type::<String>(&*val_string));
        assert!(!is_type::<i32>(&*val_string));
    }

    #[test]
    fn test_type_compare() {
        // Runtime comparison
        let is_same = safe_type_compare::<i32, i32>();
        let is_different = safe_type_compare::<i32, i64>();

        assert!(is_same);
        assert!(!is_different);
    }
}
