//! Mutable non-owning type-erased value reference.
//!
//! `MutableValueRef` extends `ValueRef` with in-place mutation capabilities.
//! Matches C++ `VtMutableValueRef`.

use std::any::TypeId;
use std::fmt;
use std::marker::PhantomData;

use super::value::Value;

/// A mutable non-owning type-erased reference to a value.
///
/// Extends `ValueRef` with assignment, swap, and mutation operations.
/// Matches C++ `VtMutableValueRef`.
///
/// # Examples
///
/// ```
/// use usd_vt::{Value, MutableValueRef};
///
/// let mut value = Value::from(42i32);
/// let mut val_ref = MutableValueRef::from(&mut value);
/// assert!(val_ref.is::<i32>());
/// ```
pub struct MutableValueRef<'a> {
    /// Mutable reference to the underlying Value.
    value: &'a mut Value,
    /// Lifetime marker.
    _marker: PhantomData<&'a mut ()>,
}

impl<'a> MutableValueRef<'a> {
    /// Creates a `MutableValueRef` from a mutable Value reference.
    #[inline]
    pub fn new(value: &'a mut Value) -> Self {
        Self {
            value,
            _marker: PhantomData,
        }
    }

    /// Returns true if this reference is empty.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    /// Returns true if this reference holds type `T`.
    #[inline]
    #[must_use]
    pub fn is<T: 'static>(&self) -> bool {
        self.value.is::<T>()
    }

    /// Returns a reference to the held value if type matches.
    #[inline]
    #[must_use]
    pub fn get<T: 'static>(&self) -> Option<&T> {
        self.value.get::<T>()
    }

    /// Returns the TypeId of the referenced type.
    #[inline]
    #[must_use]
    pub fn get_type_id(&self) -> Option<TypeId> {
        self.value.held_type_id()
    }

    /// Returns the type name for debugging.
    #[inline]
    #[must_use]
    pub fn type_name(&self) -> Option<&'static str> {
        self.value.type_name()
    }

    /// Assigns a new value, replacing the current contents.
    ///
    /// Matches C++ `VtMutableValueRef::operator=(T)`.
    #[inline]
    pub fn assign<T: Into<Value>>(&mut self, val: T) {
        *self.value = val.into();
    }

    /// Assigns a value without type checking.
    ///
    /// Matches C++ `VtMutableValueRef::UncheckedAssign()`.
    #[inline]
    pub fn unchecked_assign<T: Into<Value>>(&mut self, val: T) {
        *self.value = val.into();
    }

    /// Swaps the held value with a typed value.
    ///
    /// If this ref does not hold T, replaces with default T first.
    ///
    /// Matches C++ `VtMutableValueRef::Swap<T>(T&)`.
    pub fn swap_with<T>(&mut self, rhs: &mut T)
    where
        T: Clone
            + Default
            + Send
            + Sync
            + fmt::Debug
            + PartialEq
            + std::hash::Hash
            + 'static,
    {
        self.value.swap_with(rhs);
    }

    /// Swaps the held value with a typed value without type checking.
    ///
    /// Matches C++ `VtMutableValueRef::UncheckedSwap<T>(T&)`.
    pub fn unchecked_swap_with<T>(&mut self, rhs: &mut T)
    where
        T: Clone + Send + Sync + fmt::Debug + PartialEq + std::hash::Hash + 'static,
    {
        self.value.unchecked_swap_with(rhs);
    }

    /// Mutates the held value of type T via a closure.
    ///
    /// Returns true if the value was mutated (type matched), false otherwise.
    ///
    /// Matches C++ `VtMutableValueRef::Mutate<T>(Fn)`.
    pub fn mutate<T, F>(&mut self, mutate_fn: F) -> bool
    where
        T: Clone + Send + Sync + fmt::Debug + PartialEq + std::hash::Hash + 'static,
        F: FnOnce(&mut T),
    {
        self.value.mutate::<T, _>(mutate_fn)
    }

    /// Mutates the held value without type checking.
    ///
    /// Matches C++ `VtMutableValueRef::UncheckedMutate<T>(Fn)`.
    pub fn unchecked_mutate<T, F>(&mut self, mutate_fn: F)
    where
        T: Clone + Send + Sync + fmt::Debug + PartialEq + std::hash::Hash + 'static,
        F: FnOnce(&mut T),
    {
        self.value.unchecked_mutate::<T, _>(mutate_fn);
    }

    /// Returns the underlying Value reference.
    #[inline]
    pub fn as_value(&self) -> &Value {
        self.value
    }

    /// Returns the underlying mutable Value reference.
    #[inline]
    pub fn as_value_mut(&mut self) -> &mut Value {
        self.value
    }
}

impl<'a> From<&'a mut Value> for MutableValueRef<'a> {
    #[inline]
    fn from(value: &'a mut Value) -> Self {
        Self::new(value)
    }
}

impl<'a> fmt::Debug for MutableValueRef<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MutableValueRef({:?})", self.value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic() {
        let mut value = Value::from(42i32);
        let val_ref = MutableValueRef::new(&mut value);
        assert!(val_ref.is::<i32>());
        assert_eq!(val_ref.get::<i32>(), Some(&42));
    }

    #[test]
    fn test_assign() {
        let mut value = Value::from(42i32);
        let mut val_ref = MutableValueRef::new(&mut value);
        val_ref.assign(100i32);
        assert_eq!(val_ref.get::<i32>(), Some(&100));
    }

    #[test]
    fn test_mutate() {
        let mut value = Value::from(10i32);
        let mut val_ref = MutableValueRef::new(&mut value);
        assert!(val_ref.mutate::<i32, _>(|x| *x *= 2));
        assert_eq!(val_ref.get::<i32>(), Some(&20));
    }

    #[test]
    fn test_swap_with() {
        let mut value = Value::from(42i32);
        let mut val_ref = MutableValueRef::new(&mut value);
        let mut x = 100i32;
        val_ref.swap_with(&mut x);
        assert_eq!(val_ref.get::<i32>(), Some(&100));
        assert_eq!(x, 42);
    }

    #[test]
    fn test_empty() {
        let mut value = Value::empty();
        let val_ref = MutableValueRef::new(&mut value);
        assert!(val_ref.is_empty());
    }
}
