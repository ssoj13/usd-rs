//! C++ style casting utilities for Rust.
//!
//! This module provides casting utilities that mirror some of C++'s casting
//! capabilities, adapted for Rust's type system and safety guarantees.
//!
//! In Rust, most C++ casting patterns are handled through:
//! - Trait objects and dynamic dispatch (`dyn Trait`)
//! - `Any` trait for type-erased downcasting
//! - `From`/`Into` traits for conversions
//! - `as` for primitive casts

use std::any::{Any, TypeId};

/// Marker trait for types that participate in the TfType casting system.
///
/// Types implementing `Castable` can be cast up/down the type hierarchy
/// using `cast_to_ancestor` and `cast_from_ancestor`.
pub trait Castable: Any + Send + Sync {
    /// Returns the TypeId of this concrete type.
    fn type_id_of(&self) -> TypeId {
        TypeId::of::<Self>()
    }

    /// Returns self as `&dyn Any` for downcasting.
    fn as_castable_any(&self) -> &dyn Any;

    /// Returns self as `&mut dyn Any` for downcasting.
    fn as_castable_any_mut(&mut self) -> &mut dyn Any;
}

// Blanket impl for all eligible types.
impl<T: Any + Send + Sync> Castable for T {
    fn as_castable_any(&self) -> &dyn Any {
        self
    }

    fn as_castable_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Casts a `&dyn Castable` to a concrete ancestor type `A`.
///
/// In C++ USD this uses `TfType::CastToAncestor`. In Rust we use
/// `Any::downcast_ref` which checks the concrete type at runtime.
/// Returns `None` if the object is not actually of type `A`.
pub fn cast_to_ancestor<A: Castable>(obj: &dyn Castable) -> Option<&A> {
    obj.as_castable_any().downcast_ref::<A>()
}

/// Casts a `&dyn Castable` from an ancestor type back to a derived type `D`.
///
/// In C++ USD this uses `TfType::CastFromAncestor`. In Rust we use
/// `Any::downcast_ref` which checks the concrete type at runtime.
pub fn cast_from_ancestor<D: Castable>(obj: &dyn Castable) -> Option<&D> {
    obj.as_castable_any().downcast_ref::<D>()
}

/// Mutable version of `cast_to_ancestor`.
pub fn cast_to_ancestor_mut<A: Castable>(obj: &mut dyn Castable) -> Option<&mut A> {
    obj.as_castable_any_mut().downcast_mut::<A>()
}

/// Mutable version of `cast_from_ancestor`.
pub fn cast_from_ancestor_mut<D: Castable>(obj: &mut dyn Castable) -> Option<&mut D> {
    obj.as_castable_any_mut().downcast_mut::<D>()
}

/// Attempts to downcast a trait object reference to a concrete type.
///
/// This is similar to C++'s `dynamic_cast` but for trait objects.
/// Returns `Some(&T)` if the cast succeeds, `None` otherwise.
///
/// # Examples
///
/// ```
/// use std::any::Any;
/// use usd_tf::cxx_cast::tf_downcast_ref;
///
/// trait MyTrait: Any {}
/// struct MyType(i32);
/// impl MyTrait for MyType {}
///
/// let obj: &dyn MyTrait = &MyType(42);
/// let any = obj as &dyn Any;
/// let concrete = tf_downcast_ref::<MyType>(any);
/// assert!(concrete.is_some());
/// assert_eq!(concrete.unwrap().0, 42);
/// ```
pub fn tf_downcast_ref<T: Any>(any: &dyn Any) -> Option<&T> {
    any.downcast_ref::<T>()
}

/// Attempts to downcast a mutable trait object reference to a concrete type.
///
/// This is the mutable version of `tf_downcast_ref`.
/// Returns `Some(&mut T)` if the cast succeeds, `None` otherwise.
pub fn tf_downcast_mut<T: Any>(any: &mut dyn Any) -> Option<&mut T> {
    any.downcast_mut::<T>()
}

/// Type-level utility to copy const-ness from one type to another.
///
/// In C++, this is done with conditional template metaprogramming.
/// In Rust, we handle this through different functions or traits.
pub trait CopyConst<T> {
    /// The target type with const-ness copied from source.
    type Output;
}

impl<T> CopyConst<T> for T {
    type Output = T;
}

/// Checks if a type can be safely cast to another type at compile time.
///
/// This uses Rust's type system to ensure cast safety.
pub trait SafeCast<T> {
    /// Performs the cast.
    fn safe_cast(self) -> T;
}

// Implement SafeCast for identity casts
impl<T> SafeCast<T> for T {
    fn safe_cast(self) -> T {
        self
    }
}

/// Attempts to cast a reference to its most derived type.
///
/// In C++, this uses `dynamic_cast<void*>` to get the most-derived pointer.
/// In Rust, for `dyn Any` types, we can get the TypeId, but getting the
/// actual most-derived pointer requires the type to implement specific traits.
///
/// For types that don't have dynamic dispatch, this returns the original pointer.
///
/// # Safety
///
/// This is safe because it only works with references and doesn't
/// violate Rust's aliasing rules.
#[must_use]
pub fn tf_cast_to_most_derived<T: ?Sized>(ptr: &T) -> &T {
    // In Rust, we don't have C++-style virtual inheritance, so the
    // reference is already to the "most derived" object in the memory layout.
    // This function exists for API compatibility with C++ USD.
    ptr
}

/// Mutable version of `tf_cast_to_most_derived`.
#[must_use]
pub fn tf_cast_to_most_derived_mut<T: ?Sized>(ptr: &mut T) -> &mut T {
    ptr
}

/// Trait for types that support downcasting to concrete types.
///
/// This is similar to the polymorphic base in C++ OpenUSD.
pub trait TfPolymorphic: Any {
    /// Returns this object as an Any reference for downcasting.
    fn as_any(&self) -> &dyn Any;

    /// Returns this object as a mutable Any reference for downcasting.
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

// Blanket implementation for all Any types
impl<T: Any> TfPolymorphic for T {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Attempts to cast between trait object types.
///
/// This is useful when you have a trait object and want to see if it
/// implements another trait.
///
/// # Examples
///
/// ```
/// use usd_tf::cxx_cast::tf_trait_cast;
/// use std::any::Any;
///
/// trait Trait1: Any {}
/// trait Trait2: Any {}
///
/// struct MyType;
/// impl Trait1 for MyType {}
/// impl Trait2 for MyType {}
///
/// // This would require runtime trait checking which isn't directly
/// // supported in stable Rust without additional infrastructure
/// ```
pub fn tf_trait_cast<'a, T: ?Sized + Any, U: ?Sized + 'static>(
    _obj: &'a (dyn Any + 'static),
) -> Option<&'a U> {
    // This is a simplified version. Full trait casting requires
    // additional infrastructure and isn't directly expressible in stable Rust
    // without macros or proc macros to generate the necessary vtables.
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_castable_to_ancestor() {
        // Simulate a "derived" value used as dyn Castable
        let val = 42i32;
        let obj: &dyn Castable = &val;

        // Cast to the same type (acts as "ancestor" check)
        let result = cast_to_ancestor::<i32>(obj);
        assert!(result.is_some());
        assert_eq!(*result.unwrap(), 42);

        // Wrong type returns None
        let wrong = cast_to_ancestor::<f64>(obj);
        assert!(wrong.is_none());
    }

    #[test]
    fn test_castable_from_ancestor() {
        let val = 3.14f64;
        let obj: &dyn Castable = &val;

        let result = cast_from_ancestor::<f64>(obj);
        assert!(result.is_some());
        assert!((result.unwrap() - 3.14).abs() < f64::EPSILON);
    }

    #[test]
    fn test_castable_mut() {
        let mut val = 10u32;
        let obj: &mut dyn Castable = &mut val;

        let r = cast_to_ancestor_mut::<u32>(obj);
        assert!(r.is_some());
        *r.unwrap() = 99;
        assert_eq!(val, 99);
    }

    #[test]
    fn test_downcast_ref() {
        let value: Box<dyn Any> = Box::new(42i32);
        let as_any: &dyn Any = value.as_ref();

        let downcasted = tf_downcast_ref::<i32>(as_any);
        assert!(downcasted.is_some());
        assert_eq!(*downcasted.unwrap(), 42);

        let wrong_type = tf_downcast_ref::<f64>(as_any);
        assert!(wrong_type.is_none());
    }

    #[test]
    fn test_downcast_mut() {
        let mut value: Box<dyn Any> = Box::new(42i32);
        let as_any: &mut dyn Any = value.as_mut();

        let downcasted = tf_downcast_mut::<i32>(as_any);
        assert!(downcasted.is_some());

        if let Some(val) = downcasted {
            *val = 100;
        }

        let final_value = value.downcast::<i32>().unwrap();
        assert_eq!(*final_value, 100);
    }

    #[test]
    fn test_cast_to_most_derived() {
        let value = 42i32;
        let ptr = &value;
        let casted = tf_cast_to_most_derived(ptr);
        assert_eq!(*casted, 42);
    }

    #[test]
    fn test_polymorphic_trait() {
        struct TestType(i32);

        let obj = TestType(42);
        let any_ref = obj.as_any();
        let downcasted = any_ref.downcast_ref::<TestType>();
        assert!(downcasted.is_some());
        assert_eq!(downcasted.unwrap().0, 42);
    }

    #[test]
    fn test_safe_cast_identity() {
        let value = 42i32;
        let casted = value.safe_cast();
        assert_eq!(casted, 42);
    }
}
