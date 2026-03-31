//! Null pointer token for smart pointers.
//!
//! Provides a token type to represent null for smart pointers like `RefPtr` and `WeakPtr`.
//! Matches C++ `pxr/base/tf/nullPtr.h`.

/// A type used to create the `NullPtr` token.
///
/// Matches C++ `TfNullPtrType` struct.
///
/// # Examples
///
/// ```
/// use usd_tf::null_ptr::NullPtrType;
///
/// let null_type = NullPtrType;
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NullPtrType;

/// A token to represent null for smart pointers like `RefPtr` and `WeakPtr`.
///
/// Matches C++ `TfNullPtr` constant.
///
/// # Examples
///
/// ```ignore
/// use usd_tf::{NULL_PTR, RefPtr};
///
/// let ptr: RefPtr<i32> = NULL_PTR.into();
/// assert!(ptr.is_null());
/// ```
pub const NULL_PTR: NullPtrType = NullPtrType;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_null_ptr_type() {
        let null1 = NullPtrType;
        let null2 = NULL_PTR;
        assert_eq!(null1, null2);
    }

    #[test]
    fn test_null_ptr_constant() {
        assert_eq!(NULL_PTR, NullPtrType);
    }
}
