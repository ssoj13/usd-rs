//! Function signature traits and type extraction.
//!
//! Provides traits for extracting function signature information at compile time,
//! including return type, argument types, and arity.
//!
//! # Examples
//!
//! ```
//! use usd_tf::function_traits::Arity;
//!
//! // Function pointer types implement Arity
//! assert_eq!(<fn() -> i32 as Arity>::ARITY, 0);
//! assert_eq!(<fn(i32) -> bool as Arity>::ARITY, 1);
//! assert_eq!(<fn(i32, i32) -> i32 as Arity>::ARITY, 2);
//! ```

/// Trait for getting the arity (number of arguments) of a function type.
pub trait Arity {
    /// The number of arguments the function takes.
    const ARITY: usize;
}

/// Trait for getting the return type of a function type.
pub trait ReturnType {
    /// The return type of the function.
    type Output;
}

/// Marker trait for function types.
pub trait FunctionTraits: Arity + ReturnType {}

// Implement for function pointers with 0-12 arguments

impl<R> Arity for fn() -> R {
    const ARITY: usize = 0;
}
impl<R> ReturnType for fn() -> R {
    type Output = R;
}
impl<R> FunctionTraits for fn() -> R {}

impl<R, A> Arity for fn(A) -> R {
    const ARITY: usize = 1;
}
impl<R, A> ReturnType for fn(A) -> R {
    type Output = R;
}
impl<R, A> FunctionTraits for fn(A) -> R {}

impl<R, A, B> Arity for fn(A, B) -> R {
    const ARITY: usize = 2;
}
impl<R, A, B> ReturnType for fn(A, B) -> R {
    type Output = R;
}
impl<R, A, B> FunctionTraits for fn(A, B) -> R {}

impl<R, A, B, C> Arity for fn(A, B, C) -> R {
    const ARITY: usize = 3;
}
impl<R, A, B, C> ReturnType for fn(A, B, C) -> R {
    type Output = R;
}
impl<R, A, B, C> FunctionTraits for fn(A, B, C) -> R {}

impl<R, A, B, C, D> Arity for fn(A, B, C, D) -> R {
    const ARITY: usize = 4;
}
impl<R, A, B, C, D> ReturnType for fn(A, B, C, D) -> R {
    type Output = R;
}
impl<R, A, B, C, D> FunctionTraits for fn(A, B, C, D) -> R {}

impl<R, A, B, C, D, E> Arity for fn(A, B, C, D, E) -> R {
    const ARITY: usize = 5;
}
impl<R, A, B, C, D, E> ReturnType for fn(A, B, C, D, E) -> R {
    type Output = R;
}
impl<R, A, B, C, D, E> FunctionTraits for fn(A, B, C, D, E) -> R {}

impl<R, A, B, C, D, E, F> Arity for fn(A, B, C, D, E, F) -> R {
    const ARITY: usize = 6;
}
impl<R, A, B, C, D, E, F> ReturnType for fn(A, B, C, D, E, F) -> R {
    type Output = R;
}
impl<R, A, B, C, D, E, F> FunctionTraits for fn(A, B, C, D, E, F) -> R {}

impl<R, A, B, C, D, E, F, G> Arity for fn(A, B, C, D, E, F, G) -> R {
    const ARITY: usize = 7;
}
impl<R, A, B, C, D, E, F, G> ReturnType for fn(A, B, C, D, E, F, G) -> R {
    type Output = R;
}
impl<R, A, B, C, D, E, F, G> FunctionTraits for fn(A, B, C, D, E, F, G) -> R {}

impl<R, A, B, C, D, E, F, G, H> Arity for fn(A, B, C, D, E, F, G, H) -> R {
    const ARITY: usize = 8;
}
impl<R, A, B, C, D, E, F, G, H> ReturnType for fn(A, B, C, D, E, F, G, H) -> R {
    type Output = R;
}
impl<R, A, B, C, D, E, F, G, H> FunctionTraits for fn(A, B, C, D, E, F, G, H) -> R {}

/// Trait for extracting the first argument type.
pub trait FirstArg {
    /// The type of the first argument.
    type Arg;
}

impl<R, A> FirstArg for fn(A) -> R {
    type Arg = A;
}

impl<R, A, B> FirstArg for fn(A, B) -> R {
    type Arg = A;
}

impl<R, A, B, C> FirstArg for fn(A, B, C) -> R {
    type Arg = A;
}

impl<R, A, B, C, D> FirstArg for fn(A, B, C, D) -> R {
    type Arg = A;
}

/// Trait for extracting the second argument type.
pub trait SecondArg {
    /// The type of the second argument.
    type Arg;
}

impl<R, A, B> SecondArg for fn(A, B) -> R {
    type Arg = B;
}

impl<R, A, B, C> SecondArg for fn(A, B, C) -> R {
    type Arg = B;
}

impl<R, A, B, C, D> SecondArg for fn(A, B, C, D) -> R {
    type Arg = B;
}

/// Trait for extracting the third argument type.
pub trait ThirdArg {
    /// The type of the third argument.
    type Arg;
}

impl<R, A, B, C> ThirdArg for fn(A, B, C) -> R {
    type Arg = C;
}

impl<R, A, B, C, D> ThirdArg for fn(A, B, C, D) -> R {
    type Arg = C;
}

/// Helper function to get the arity of a callable.
///
/// This is a compile-time constant.
#[inline]
pub const fn arity<F: Arity>() -> usize {
    F::ARITY
}

/// Macro to get function traits at compile time.
///
/// # Examples
///
/// ```
/// use usd_tf::function_traits::{Arity, ReturnType};
///
/// fn my_func(x: i32) -> bool {
///     x > 0
/// }
///
/// type MyFn = fn(i32) -> bool;
/// assert_eq!(<MyFn as Arity>::ARITY, 1);
/// ```
#[macro_export]
macro_rules! fn_arity {
    ($fn_type:ty) => {
        <$fn_type as $crate::function_traits::Arity>::ARITY
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arity_0() {
        type F = fn() -> i32;
        assert_eq!(F::ARITY, 0);
    }

    #[test]
    fn test_arity_1() {
        type F = fn(i32) -> bool;
        assert_eq!(F::ARITY, 1);
    }

    #[test]
    fn test_arity_2() {
        type F = fn(i32, i64) -> bool;
        assert_eq!(F::ARITY, 2);
    }

    #[test]
    fn test_arity_3() {
        type F = fn(i32, i32, i32) -> i32;
        assert_eq!(F::ARITY, 3);
    }

    #[test]
    fn test_arity_4() {
        type F = fn(i32, i32, i32, i32) -> i32;
        assert_eq!(F::ARITY, 4);
    }

    #[test]
    fn test_arity_5() {
        type F = fn(i32, i32, i32, i32, i32) -> i32;
        assert_eq!(F::ARITY, 5);
    }

    #[test]
    fn test_arity_helper() {
        assert_eq!(arity::<fn() -> ()>(), 0);
        assert_eq!(arity::<fn(i32) -> i32>(), 1);
        assert_eq!(arity::<fn(i32, i32) -> i32>(), 2);
    }

    #[test]
    fn test_return_type() {
        fn _check_return_type<F: ReturnType<Output = bool>>() {}

        _check_return_type::<fn() -> bool>();
        _check_return_type::<fn(i32) -> bool>();
    }

    #[test]
    fn test_first_arg() {
        fn _check_first_arg<F: FirstArg<Arg = i32>>() {}

        _check_first_arg::<fn(i32) -> bool>();
        _check_first_arg::<fn(i32, i64) -> bool>();
    }

    #[test]
    fn test_second_arg() {
        fn _check_second_arg<F: SecondArg<Arg = bool>>() {}

        _check_second_arg::<fn(i32, bool) -> ()>();
        _check_second_arg::<fn(i32, bool, char) -> ()>();
    }

    #[test]
    fn test_third_arg() {
        fn _check_third_arg<F: ThirdArg<Arg = char>>() {}

        _check_third_arg::<fn(i32, bool, char) -> ()>();
    }

    #[test]
    fn test_fn_arity_macro() {
        assert_eq!(fn_arity!(fn() -> ()), 0);
        assert_eq!(fn_arity!(fn(i32) -> i32), 1);
        assert_eq!(fn_arity!(fn(i32, i32) -> i32), 2);
    }

    #[test]
    fn test_function_traits() {
        fn _check_traits<F: FunctionTraits>() {}

        _check_traits::<fn() -> ()>();
        _check_traits::<fn(i32) -> bool>();
        _check_traits::<fn(i32, i64) -> Option<i32>>();
    }

    #[test]
    fn test_with_various_return_types() {
        type F1 = fn() -> ();
        type F2 = fn() -> i32;
        type F3 = fn() -> String;
        type F4 = fn() -> Vec<u8>;

        assert_eq!(F1::ARITY, 0);
        assert_eq!(F2::ARITY, 0);
        assert_eq!(F3::ARITY, 0);
        assert_eq!(F4::ARITY, 0);
    }

    #[test]
    fn test_with_various_arg_types() {
        // Using non-reference types to avoid HRTB issues
        type F = fn(String, Vec<u8>) -> bool;
        assert_eq!(F::ARITY, 2);
    }

    #[test]
    fn test_high_arity() {
        type F6 = fn(i32, i32, i32, i32, i32, i32) -> i32;
        type F7 = fn(i32, i32, i32, i32, i32, i32, i32) -> i32;
        type F8 = fn(i32, i32, i32, i32, i32, i32, i32, i32) -> i32;

        assert_eq!(F6::ARITY, 6);
        assert_eq!(F7::ARITY, 7);
        assert_eq!(F8::ARITY, 8);
    }
}
