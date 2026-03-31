//! Compile-time metaprogramming utilities.
//!
//! Provides type-level utilities for working with type lists and
//! compile-time type manipulation.
//!
//! # Note
//!
//! Rust handles many of these patterns differently than C++. This module
//! provides Rust-idiomatic equivalents where possible.

use std::marker::PhantomData;

/// A compile-time type list marker.
///
/// This is primarily useful for type-level programming and trait implementations.
/// In Rust, tuples serve a similar purpose to C++ type lists.
///
/// # Examples
///
/// ```
/// use usd_tf::meta::TypeList;
///
/// // TypeList is a marker type
/// let _: TypeList<(i32, String, bool)> = TypeList::new();
/// ```
pub struct TypeList<T>(PhantomData<T>);

impl<T> TypeList<T> {
    /// Create a new TypeList marker.
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}

impl<T> Default for TypeList<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Clone for TypeList<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for TypeList<T> {}

impl<T> std::fmt::Debug for TypeList<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TypeList").finish()
    }
}

/// Trait to get the length (number of elements) of a tuple type.
pub trait TupleLength {
    /// The number of elements in the tuple.
    const LENGTH: usize;
}

impl TupleLength for () {
    const LENGTH: usize = 0;
}

impl<A> TupleLength for (A,) {
    const LENGTH: usize = 1;
}

impl<A, B> TupleLength for (A, B) {
    const LENGTH: usize = 2;
}

impl<A, B, C> TupleLength for (A, B, C) {
    const LENGTH: usize = 3;
}

impl<A, B, C, D> TupleLength for (A, B, C, D) {
    const LENGTH: usize = 4;
}

impl<A, B, C, D, E> TupleLength for (A, B, C, D, E) {
    const LENGTH: usize = 5;
}

impl<A, B, C, D, E, F> TupleLength for (A, B, C, D, E, F) {
    const LENGTH: usize = 6;
}

impl<A, B, C, D, E, F, G> TupleLength for (A, B, C, D, E, F, G) {
    const LENGTH: usize = 7;
}

impl<A, B, C, D, E, F, G, H> TupleLength for (A, B, C, D, E, F, G, H) {
    const LENGTH: usize = 8;
}

/// Trait to get the head (first element type) of a tuple.
pub trait TupleHead {
    /// The type of the first element.
    type Head;
}

impl<A> TupleHead for (A,) {
    type Head = A;
}

impl<A, B> TupleHead for (A, B) {
    type Head = A;
}

impl<A, B, C> TupleHead for (A, B, C) {
    type Head = A;
}

impl<A, B, C, D> TupleHead for (A, B, C, D) {
    type Head = A;
}

impl<A, B, C, D, E> TupleHead for (A, B, C, D, E) {
    type Head = A;
}

/// Trait to get the tail (all but first element) type of a tuple.
pub trait TupleTail {
    /// The type of the remaining elements as a tuple.
    type Tail;
}

impl<A> TupleTail for (A,) {
    type Tail = ();
}

impl<A, B> TupleTail for (A, B) {
    type Tail = (B,);
}

impl<A, B, C> TupleTail for (A, B, C) {
    type Tail = (B, C);
}

impl<A, B, C, D> TupleTail for (A, B, C, D) {
    type Tail = (B, C, D);
}

impl<A, B, C, D, E> TupleTail for (A, B, C, D, E) {
    type Tail = (B, C, D, E);
}

impl<A, B, C, D, E, F> TupleTail for (A, B, C, D, E, F) {
    type Tail = (B, C, D, E, F);
}

/// Helper type for compile-time conditional type selection.
///
/// This is similar to `std::conditional` in C++.
///
/// # Examples
///
/// ```
/// use usd_tf::meta::{Conditional, Select};
///
/// type Result = <Conditional<true, i32, String> as Select>::Type;
/// // Result is i32 because condition is true
/// ```
pub struct Conditional<const COND: bool, T, F>(PhantomData<(T, F)>);

/// Trait for selecting between two types based on a compile-time condition.
pub trait Select {
    /// The selected type.
    type Type;
}

impl<T, F> Select for Conditional<true, T, F> {
    type Type = T;
}

impl<T, F> Select for Conditional<false, T, F> {
    type Type = F;
}

/// Type alias for conditional type selection.
///
/// # Examples
///
/// ```
/// use usd_tf::meta::ConditionalType;
///
/// // Selects i32 because condition is true
/// fn example<const B: bool>() {
///     // Type depends on B
/// }
/// ```
pub type ConditionalType<const COND: bool, T, F> = <Conditional<COND, T, F> as Select>::Type;

/// Get the length of a tuple type at compile time.
///
/// # Examples
///
/// ```
/// use usd_tf::meta::tuple_length;
///
/// assert_eq!(tuple_length::<()>(), 0);
/// assert_eq!(tuple_length::<(i32,)>(), 1);
/// assert_eq!(tuple_length::<(i32, bool)>(), 2);
/// ```
pub const fn tuple_length<T: TupleLength>() -> usize {
    T::LENGTH
}

/// Macro to check if a type implements a trait at compile time.
///
/// Returns true if the type implements the trait.
#[macro_export]
macro_rules! implements {
    ($ty:ty: $trait:path) => {{
        trait DoesNotImplement {
            const IMPLEMENTS: bool = false;
        }
        impl<T> DoesNotImplement for T {}

        struct Checker<T>(std::marker::PhantomData<T>);
        impl<T: $trait> Checker<T> {
            const IMPLEMENTS: bool = true;
        }

        Checker::<$ty>::IMPLEMENTS
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_list() {
        let _: TypeList<(i32, String)> = TypeList::new();
        let list = TypeList::<()>::default();
        let _copy = list;
    }

    #[test]
    fn test_tuple_length() {
        assert_eq!(<() as TupleLength>::LENGTH, 0);
        assert_eq!(<(i32,) as TupleLength>::LENGTH, 1);
        assert_eq!(<(i32, bool) as TupleLength>::LENGTH, 2);
        assert_eq!(<(i32, bool, String) as TupleLength>::LENGTH, 3);
        assert_eq!(<(i32, i32, i32, i32) as TupleLength>::LENGTH, 4);
        assert_eq!(<(i32, i32, i32, i32, i32) as TupleLength>::LENGTH, 5);
    }

    #[test]
    fn test_tuple_length_helper() {
        assert_eq!(tuple_length::<()>(), 0);
        assert_eq!(tuple_length::<(i32,)>(), 1);
        assert_eq!(tuple_length::<(i32, bool)>(), 2);
    }

    #[test]
    fn test_tuple_head() {
        fn _check_head<T: TupleHead<Head = i32>>() {}

        _check_head::<(i32,)>();
        _check_head::<(i32, bool)>();
        _check_head::<(i32, bool, String)>();
    }

    #[test]
    fn test_tuple_tail() {
        fn _check_tail_empty<T: TupleTail<Tail = ()>>() {}
        fn _check_tail_one<T: TupleTail<Tail = (bool,)>>() {}

        _check_tail_empty::<(i32,)>();
        _check_tail_one::<(i32, bool)>();
    }

    #[test]
    fn test_conditional_true() {
        fn _check_type<T>()
        where
            Conditional<true, i32, String>: Select<Type = T>,
        {
        }

        _check_type::<i32>();
    }

    #[test]
    fn test_conditional_false() {
        fn _check_type<T>()
        where
            Conditional<false, i32, String>: Select<Type = T>,
        {
        }

        _check_type::<String>();
    }

    #[test]
    fn test_conditional_type_alias() {
        type T1 = ConditionalType<true, i32, String>;
        type T2 = ConditionalType<false, i32, String>;

        fn _check_t1(_: T1) {}
        fn _check_t2(_: T2) {}

        _check_t1(42i32);
        _check_t2(String::new());
    }

    #[test]
    fn test_type_list_debug() {
        let list = TypeList::<(i32,)>::new();
        let debug = format!("{:?}", list);
        assert!(debug.contains("TypeList"));
    }

    #[test]
    fn test_high_arity_tuples() {
        assert_eq!(<(i32, i32, i32, i32, i32, i32) as TupleLength>::LENGTH, 6);
        assert_eq!(
            <(i32, i32, i32, i32, i32, i32, i32) as TupleLength>::LENGTH,
            7
        );
        assert_eq!(
            <(i32, i32, i32, i32, i32, i32, i32, i32) as TupleLength>::LENGTH,
            8
        );
    }
}
