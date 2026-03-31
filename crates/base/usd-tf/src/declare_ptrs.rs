//! Standard pointer type declaration utilities.
//!
//! Port of pxr/base/tf/declarePtrs.h
//!
//! In C++, TF_DECLARE_WEAK_PTRS / TF_DECLARE_REF_PTRS create typedefs for
//! smart pointer types. In Rust, we use Arc/Weak directly, but provide
//! macros for consistent type alias declaration across the codebase.

use std::sync::{Arc, Weak};

/// Declare standard weak pointer type aliases for a type.
///
/// Usage: `declare_weak_ptrs!(MyType);`
///
/// Generates:
/// - `MyTypePtr` = `Weak<MyType>`
/// - `MyTypePtrVec` = `Vec<Weak<MyType>>`
#[macro_export]
macro_rules! declare_weak_ptrs {
    ($t:ident) => {
        paste::paste! {
            /// Weak pointer to the type.
            pub type [<$t Ptr>] = std::sync::Weak<$t>;
            /// Vector of weak pointers.
            pub type [<$t PtrVec>] = Vec<std::sync::Weak<$t>>;
        }
    };
}

/// Declare standard ref pointer type aliases for a type.
///
/// Usage: `declare_ref_ptrs!(MyType);`
///
/// Generates:
/// - `MyTypeRefPtr` = `Arc<MyType>`
/// - `MyTypeRefPtrVec` = `Vec<Arc<MyType>>`
#[macro_export]
macro_rules! declare_ref_ptrs {
    ($t:ident) => {
        paste::paste! {
            /// Reference-counted pointer to the type.
            pub type [<$t RefPtr>] = std::sync::Arc<$t>;
            /// Vector of reference-counted pointers.
            pub type [<$t RefPtrVec>] = Vec<std::sync::Arc<$t>>;
        }
    };
}

/// Declare both weak and ref pointer type aliases for a type.
///
/// Usage: `declare_weak_and_ref_ptrs!(MyType);`
///
/// Combines `declare_weak_ptrs!` and `declare_ref_ptrs!`.
#[macro_export]
macro_rules! declare_weak_and_ref_ptrs {
    ($t:ident) => {
        $crate::declare_weak_ptrs!($t);
        $crate::declare_ref_ptrs!($t);
    };
}

/// Helper struct for declaring pointer types (non-macro approach).
///
/// Provides a phantom type marker for generic pointer declaration.
/// Use the associated functions to create pointer types.
/// Matches C++ `TfDeclarePtrs<T>`.
pub struct DeclarePtrs<T> {
    _phantom: std::marker::PhantomData<T>,
}

impl<T> DeclarePtrs<T> {
    /// Create a new Weak<T>.
    pub fn new_weak() -> Weak<T> {
        Weak::new()
    }

    /// Create a new Arc<T> from a value.
    pub fn new_ref(value: T) -> Arc<T> {
        Arc::new(value)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_declare_ptrs_types() {
        // Verify the generic types work
        type WeakI32 = std::sync::Weak<i32>;
        type ArcI32 = std::sync::Arc<i32>;
        type WeakVec = Vec<std::sync::Weak<i32>>;
        type ArcVec = Vec<std::sync::Arc<i32>>;

        let _w: WeakI32 = std::sync::Weak::new();
        let a: ArcI32 = std::sync::Arc::new(42);
        let _wv: WeakVec = vec![std::sync::Arc::downgrade(&a)];
        let _av: ArcVec = vec![a];
    }
}
