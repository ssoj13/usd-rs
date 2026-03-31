//! GenUserData -- base trait for custom user data passed through GenContext.
//!
//! Mirrors C++ GenUserData (GenUserData.h) and the user-data methods on GenContext.
//! In Rust we use std::any::Any for downcasting instead of shared_ptr + dynamic_pointer_cast.

use std::any::Any;

// ---------------------------------------------------------------------------
// GenUserData trait
// ---------------------------------------------------------------------------

/// Base trait for custom user data stored in GenContext.
/// Implementors must provide `as_any` so callers can downcast to the
/// concrete type via `GenContext::get_user_data::<T>()`.
pub trait GenUserData: Any {
    /// Return self as `&dyn Any` for downcasting.
    fn as_any(&self) -> &dyn Any;
    /// Return self as `&mut dyn Any` for mutable downcasting.
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// Convenience blanket implementation. Concrete types need only implement
/// `GenUserData` by delegating to this:
///
/// ```ignore
/// impl GenUserData for MyData {
///     fn as_any(&self)     -> &dyn Any { self }
///     fn as_any_mut(&mut self) -> &mut dyn Any { self }
/// }
/// ```
///
/// Or derive via the `impl_gen_user_data!` macro below.
impl dyn GenUserData {
    /// Attempt to downcast to a concrete type reference.
    pub fn downcast_ref<T: GenUserData + 'static>(&self) -> Option<&T> {
        self.as_any().downcast_ref::<T>()
    }

    /// Attempt to downcast to a mutable concrete type reference.
    pub fn downcast_mut<T: GenUserData + 'static>(&mut self) -> Option<&mut T> {
        self.as_any_mut().downcast_mut::<T>()
    }
}

/// Implement `GenUserData` for a concrete type with a single macro call.
#[macro_export]
macro_rules! impl_gen_user_data {
    ($T:ty) => {
        impl $crate::gen_shader::GenUserData for $T {
            fn as_any(&self) -> &dyn ::std::any::Any {
                self
            }
            fn as_any_mut(&mut self) -> &mut dyn ::std::any::Any {
                self
            }
        }
    };
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    struct MyData {
        value: i32,
    }

    impl GenUserData for MyData {
        fn as_any(&self) -> &dyn Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    }

    #[test]
    fn downcast_ref_works() {
        let data: Box<dyn GenUserData> = Box::new(MyData { value: 42 });
        let downcasted = data.downcast_ref::<MyData>();
        assert!(downcasted.is_some());
        assert_eq!(downcasted.unwrap().value, 42);
    }

    #[test]
    fn downcast_wrong_type_returns_none() {
        struct Other;
        impl GenUserData for Other {
            fn as_any(&self) -> &dyn Any {
                self
            }
            fn as_any_mut(&mut self) -> &mut dyn Any {
                self
            }
        }

        let data: Box<dyn GenUserData> = Box::new(MyData { value: 1 });
        assert!(data.downcast_ref::<Other>().is_none());
    }
}
