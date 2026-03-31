//! Non-owning reference to type-erased callable.
//!
//! Provides a lightweight, non-owning reference to a callable object.
//! Unlike `Box<dyn Fn>`, this never allocates and is suitable for callbacks
//! that are only used during a function call.
//!
//! # Examples
//!
//! ```
//! use usd_tf::function_ref::FunctionRef;
//!
//! fn call_with_value(f: FunctionRef<'_, dyn Fn(i32) -> i32>) -> i32 {
//!     f.call((42,))
//! }
//!
//! let add_one = |x: i32| x + 1;
//! let func_ref: FunctionRef<'_, dyn Fn(i32) -> i32> = FunctionRef::new(&add_one);
//! let result = call_with_value(func_ref);
//! assert_eq!(result, 43);
//! ```

use std::marker::PhantomData;

/// Non-owning reference to a type-erased callable.
///
/// This is a thin wrapper around `&dyn Fn` that provides a more explicit API
/// similar to C++ TfFunctionRef.
///
/// # Type Parameters
///
/// * `F` - The function trait type, e.g. `dyn Fn(i32) -> i32`
///
/// # Examples
///
/// ```
/// use usd_tf::function_ref::FunctionRef;
///
/// let closure = |x: i32| x * 2;
/// let func_ref: FunctionRef<'_, dyn Fn(i32) -> i32> = FunctionRef::new(&closure);
///
/// assert_eq!(func_ref.call((21,)), 42);
/// ```
pub struct FunctionRef<'a, F: ?Sized> {
    /// The function reference.
    inner: &'a F,
    /// Marker for the lifetime.
    _marker: PhantomData<&'a ()>,
}

impl<'a, F: ?Sized> FunctionRef<'a, F> {
    /// Create a new FunctionRef from a reference to a callable.
    #[inline]
    pub fn new(f: &'a F) -> Self {
        Self {
            inner: f,
            _marker: PhantomData,
        }
    }
}

// Implement for Fn() -> R
impl<'a, R> FunctionRef<'a, dyn Fn() -> R + 'a> {
    /// Call the referenced function with no arguments.
    #[inline]
    pub fn call(&self, _args: ()) -> R {
        (self.inner)()
    }
}

// Implement for Fn(A) -> R
impl<'a, A, R> FunctionRef<'a, dyn Fn(A) -> R + 'a> {
    /// Call the referenced function with one argument.
    #[inline]
    pub fn call(&self, args: (A,)) -> R {
        (self.inner)(args.0)
    }
}

// Implement for Fn(A, B) -> R
impl<'a, A, B, R> FunctionRef<'a, dyn Fn(A, B) -> R + 'a> {
    /// Call the referenced function with two arguments.
    #[inline]
    pub fn call(&self, args: (A, B)) -> R {
        (self.inner)(args.0, args.1)
    }
}

// Implement for Fn(A, B, C) -> R
impl<'a, A, B, C, R> FunctionRef<'a, dyn Fn(A, B, C) -> R + 'a> {
    /// Call the referenced function with three arguments.
    #[inline]
    pub fn call(&self, args: (A, B, C)) -> R {
        (self.inner)(args.0, args.1, args.2)
    }
}

// Implement for Fn(A, B, C, D) -> R
impl<'a, A, B, C, D, R> FunctionRef<'a, dyn Fn(A, B, C, D) -> R + 'a> {
    /// Call the referenced function with four arguments.
    #[inline]
    pub fn call(&self, args: (A, B, C, D)) -> R {
        (self.inner)(args.0, args.1, args.2, args.3)
    }
}

impl<'a, F: ?Sized> Clone for FunctionRef<'a, F> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, F: ?Sized> Copy for FunctionRef<'a, F> {}

impl<'a, F: ?Sized> std::fmt::Debug for FunctionRef<'a, F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FunctionRef").finish_non_exhaustive()
    }
}

/// Helper trait to create FunctionRef from any callable.
///
/// This trait enables a more ergonomic API when passing callables to functions.
pub trait AsFunctionRef<'a, F: ?Sized> {
    /// Convert to a FunctionRef.
    fn as_function_ref(&'a self) -> FunctionRef<'a, F>;
}

impl<'a, T, R> AsFunctionRef<'a, dyn Fn() -> R + 'a> for T
where
    T: Fn() -> R + 'a,
{
    fn as_function_ref(&'a self) -> FunctionRef<'a, dyn Fn() -> R + 'a> {
        FunctionRef::new(self)
    }
}

impl<'a, T, A, R> AsFunctionRef<'a, dyn Fn(A) -> R + 'a> for T
where
    T: Fn(A) -> R + 'a,
{
    fn as_function_ref(&'a self) -> FunctionRef<'a, dyn Fn(A) -> R + 'a> {
        FunctionRef::new(self)
    }
}

impl<'a, T, A, B, R> AsFunctionRef<'a, dyn Fn(A, B) -> R + 'a> for T
where
    T: Fn(A, B) -> R + 'a,
{
    fn as_function_ref(&'a self) -> FunctionRef<'a, dyn Fn(A, B) -> R + 'a> {
        FunctionRef::new(self)
    }
}

/// Type alias for a FunctionRef with no arguments.
pub type FunctionRef0<'a, R> = FunctionRef<'a, dyn Fn() -> R + 'a>;

/// Type alias for a FunctionRef with one argument.
pub type FunctionRef1<'a, A, R> = FunctionRef<'a, dyn Fn(A) -> R + 'a>;

/// Type alias for a FunctionRef with two arguments.
pub type FunctionRef2<'a, A, B, R> = FunctionRef<'a, dyn Fn(A, B) -> R + 'a>;

/// Type alias for a FunctionRef with three arguments.
pub type FunctionRef3<'a, A, B, C, R> = FunctionRef<'a, dyn Fn(A, B, C) -> R + 'a>;

/// Type alias for a FunctionRef with four arguments.
pub type FunctionRef4<'a, A, B, C, D, R> = FunctionRef<'a, dyn Fn(A, B, C, D) -> R + 'a>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_args() {
        let f = || 42;
        let func_ref = FunctionRef::new(&f as &dyn Fn() -> i32);
        assert_eq!(func_ref.call(()), 42);
    }

    #[test]
    fn test_one_arg() {
        let f = |x: i32| x + 1;
        let func_ref = FunctionRef::new(&f as &dyn Fn(i32) -> i32);
        assert_eq!(func_ref.call((41,)), 42);
    }

    #[test]
    fn test_two_args() {
        let f = |x: i32, y: i32| x + y;
        let func_ref = FunctionRef::new(&f as &dyn Fn(i32, i32) -> i32);
        assert_eq!(func_ref.call((20, 22)), 42);
    }

    #[test]
    fn test_three_args() {
        let f = |a: i32, b: i32, c: i32| a + b + c;
        let func_ref = FunctionRef::new(&f as &dyn Fn(i32, i32, i32) -> i32);
        assert_eq!(func_ref.call((10, 20, 12)), 42);
    }

    #[test]
    fn test_four_args() {
        let f = |a: i32, b: i32, c: i32, d: i32| a + b + c + d;
        let func_ref = FunctionRef::new(&f as &dyn Fn(i32, i32, i32, i32) -> i32);
        assert_eq!(func_ref.call((10, 10, 10, 12)), 42);
    }

    #[test]
    fn test_string_arg() {
        let f = |s: &str| s.len();
        let func_ref = FunctionRef::new(&f as &dyn Fn(&str) -> usize);
        assert_eq!(func_ref.call(("hello",)), 5);
    }

    #[test]
    fn test_closure_with_capture() {
        let multiplier = 7;
        let f = |x: i32| x * multiplier;
        let func_ref = FunctionRef::new(&f as &dyn Fn(i32) -> i32);
        assert_eq!(func_ref.call((6,)), 42);
    }

    #[test]
    fn test_copy() {
        let f = |x: i32| x + 1;
        let func_ref = FunctionRef::new(&f as &dyn Fn(i32) -> i32);
        let copy = func_ref;
        assert_eq!(copy.call((41,)), 42);
        assert_eq!(func_ref.call((41,)), 42);
    }

    #[test]
    fn test_clone() {
        let f = |x: i32| x + 1;
        let func_ref = FunctionRef::new(&f as &dyn Fn(i32) -> i32);
        let cloned = func_ref.clone();
        assert_eq!(cloned.call((41,)), 42);
    }

    #[test]
    fn test_debug() {
        let f = |x: i32| x;
        let func_ref = FunctionRef::new(&f as &dyn Fn(i32) -> i32);
        let debug = format!("{:?}", func_ref);
        assert!(debug.contains("FunctionRef"));
    }

    #[test]
    fn test_pass_to_function() {
        fn accepts_callback(f: FunctionRef<'_, dyn Fn(i32) -> i32>) -> i32 {
            f.call((10,))
        }

        let double = |x: i32| x * 2;
        let result = accepts_callback(FunctionRef::new(&double as &dyn Fn(i32) -> i32));
        assert_eq!(result, 20);
    }

    #[test]
    fn test_type_alias_0() {
        let f = || 42;
        let func_ref: FunctionRef0<'_, i32> = FunctionRef::new(&f);
        assert_eq!(func_ref.call(()), 42);
    }

    #[test]
    fn test_type_alias_1() {
        let f = |x: i32| x * 2;
        let func_ref: FunctionRef1<'_, i32, i32> = FunctionRef::new(&f);
        assert_eq!(func_ref.call((21,)), 42);
    }

    #[test]
    fn test_type_alias_2() {
        let f = |x: i32, y: i32| x + y;
        let func_ref: FunctionRef2<'_, i32, i32, i32> = FunctionRef::new(&f);
        assert_eq!(func_ref.call((20, 22)), 42);
    }

    #[test]
    fn test_as_function_ref_0() {
        let f = || 42;
        let func_ref: FunctionRef0<'_, i32> = f.as_function_ref();
        assert_eq!(func_ref.call(()), 42);
    }

    #[test]
    fn test_as_function_ref_1() {
        let f = |x: i32| x * 2;
        let func_ref: FunctionRef1<'_, i32, i32> = f.as_function_ref();
        assert_eq!(func_ref.call((21,)), 42);
    }

    #[test]
    fn test_as_function_ref_2() {
        let f = |x: i32, y: i32| x + y;
        let func_ref: FunctionRef2<'_, i32, i32, i32> = f.as_function_ref();
        assert_eq!(func_ref.call((20, 22)), 42);
    }
}
