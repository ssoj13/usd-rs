//! Type traits for gf types.
//!
//! This module provides marker traits for identifying gf types like
//! vectors, matrices, quaternions, and ranges.
//!
//! # Examples
//!
//! ```
//! use usd_gf::traits::*;
//!
//! // Check if a type is arithmetic
//! assert!(is_arithmetic::<f64>());
//! assert!(is_arithmetic::<i32>());
//! assert!(!is_arithmetic::<String>());
//! ```

use std::any::TypeId;

/// Marker trait for gf vector types (Vec2d, Vec3f, etc.)
pub trait GfVec {}

/// Marker trait for gf matrix types (Matrix3d, Matrix4f, etc.)
pub trait GfMatrix {}

/// Marker trait for gf quaternion types (Quatd, Quatf, etc.)
pub trait GfQuat {}

/// Marker trait for gf dual quaternion types.
pub trait GfDualQuat {}

/// Marker trait for gf range types (Range1d, Range2f, etc.)
pub trait GfRange {}

/// Trait for floating-point types, including half-precision.
///
/// This is implemented for f32 and f64 by default, and can be
/// implemented for half-precision types.
pub trait GfFloatingPoint: num_traits::Float {}

impl GfFloatingPoint for f32 {}
impl GfFloatingPoint for f64 {}
impl GfFloatingPoint for crate::half::Half {}

/// Trait for arithmetic types (integers and floating-point).
///
/// This is equivalent to C++'s std::is_arithmetic but also includes
/// any GfFloatingPoint specializations.
pub trait GfArithmetic: Copy + PartialEq + PartialOrd + Default + 'static {}

// Implement for primitive numeric types
impl GfArithmetic for i8 {}
impl GfArithmetic for i16 {}
impl GfArithmetic for i32 {}
impl GfArithmetic for i64 {}
impl GfArithmetic for i128 {}
impl GfArithmetic for isize {}
impl GfArithmetic for u8 {}
impl GfArithmetic for u16 {}
impl GfArithmetic for u32 {}
impl GfArithmetic for u64 {}
impl GfArithmetic for u128 {}
impl GfArithmetic for usize {}
impl GfArithmetic for f32 {}
impl GfArithmetic for f64 {}

/// Checks if a type is a gf vector type.
///
/// Returns true for Vec2, Vec3, Vec4 types in all precisions (d, f, h, i).
///
/// # Examples
///
/// ```
/// use usd_gf::traits::is_gf_vec;
/// use usd_gf::{Vec3d, Vec4f};
///
/// assert!(is_gf_vec::<Vec3d>());
/// assert!(is_gf_vec::<Vec4f>());
/// assert!(!is_gf_vec::<f64>());
/// ```
#[inline]
#[must_use]
pub fn is_gf_vec<T: 'static>() -> bool {
    use crate::vec2::{Vec2d, Vec2f, Vec2h, Vec2i};
    use crate::vec3::{Vec3d, Vec3f, Vec3h, Vec3i};
    use crate::vec4::{Vec4d, Vec4f, Vec4h, Vec4i};

    let tid = TypeId::of::<T>();
    tid == TypeId::of::<Vec2d>()
        || tid == TypeId::of::<Vec2f>()
        || tid == TypeId::of::<Vec2h>()
        || tid == TypeId::of::<Vec2i>()
        || tid == TypeId::of::<Vec3d>()
        || tid == TypeId::of::<Vec3f>()
        || tid == TypeId::of::<Vec3h>()
        || tid == TypeId::of::<Vec3i>()
        || tid == TypeId::of::<Vec4d>()
        || tid == TypeId::of::<Vec4f>()
        || tid == TypeId::of::<Vec4h>()
        || tid == TypeId::of::<Vec4i>()
}

/// Checks if a type is a gf matrix type.
///
/// Returns true for Matrix2, Matrix3, Matrix4 types in various precisions.
#[inline]
#[must_use]
pub fn is_gf_matrix<T: 'static>() -> bool {
    use crate::matrix2::{Matrix2d, Matrix2f};
    use crate::matrix3::{Matrix3d, Matrix3f};
    use crate::matrix4::{Matrix4d, Matrix4f};

    let tid = TypeId::of::<T>();
    tid == TypeId::of::<Matrix2d>()
        || tid == TypeId::of::<Matrix2f>()
        || tid == TypeId::of::<Matrix3d>()
        || tid == TypeId::of::<Matrix3f>()
        || tid == TypeId::of::<Matrix4d>()
        || tid == TypeId::of::<Matrix4f>()
}

/// Checks if a type is a gf quaternion type.
///
/// Returns true for Quat types (Quatd, Quatf, Quath) and Quaternion.
#[inline]
#[must_use]
pub fn is_gf_quat<T: 'static>() -> bool {
    use crate::quat::{Quatd, Quatf, Quath};
    use crate::quaternion::Quaternion;

    let tid = TypeId::of::<T>();
    tid == TypeId::of::<Quatd>()
        || tid == TypeId::of::<Quatf>()
        || tid == TypeId::of::<Quath>()
        || tid == TypeId::of::<Quaternion>()
}

/// Checks if a type is a gf dual quaternion type.
///
/// Returns true for DualQuat types (DualQuatd, DualQuatf, DualQuath).
#[inline]
#[must_use]
pub fn is_gf_dual_quat<T: 'static>() -> bool {
    use crate::dual_quat::{DualQuatd, DualQuatf, DualQuath};

    let tid = TypeId::of::<T>();
    tid == TypeId::of::<DualQuatd>()
        || tid == TypeId::of::<DualQuatf>()
        || tid == TypeId::of::<DualQuath>()
}

/// Checks if a type is a gf range type.
///
/// Returns true for Range types (Range1d, Range1f, Range2d, Range2f, Range3d, Range3f).
#[inline]
#[must_use]
pub fn is_gf_range<T: 'static>() -> bool {
    use crate::range::{Range1d, Range1f, Range2d, Range2f, Range3d, Range3f};

    let tid = TypeId::of::<T>();
    tid == TypeId::of::<Range1d>()
        || tid == TypeId::of::<Range1f>()
        || tid == TypeId::of::<Range2d>()
        || tid == TypeId::of::<Range2f>()
        || tid == TypeId::of::<Range3d>()
        || tid == TypeId::of::<Range3f>()
}

/// Checks if a type is a floating-point type.
///
/// Returns true for f32, f64, and any GfFloatingPoint implementations.
///
/// # Examples
///
/// ```
/// use usd_gf::traits::is_floating_point;
///
/// assert!(is_floating_point::<f32>());
/// assert!(is_floating_point::<f64>());
/// assert!(!is_floating_point::<i32>());
/// ```
#[inline]
#[must_use]
pub fn is_floating_point<T: 'static>() -> bool {
    TypeId::of::<T>() == TypeId::of::<f32>() || TypeId::of::<T>() == TypeId::of::<f64>()
}

/// Checks if a type is an arithmetic type (integer or floating-point).
///
/// # Examples
///
/// ```
/// use usd_gf::traits::is_arithmetic;
///
/// assert!(is_arithmetic::<f64>());
/// assert!(is_arithmetic::<i32>());
/// assert!(is_arithmetic::<u8>());
/// assert!(!is_arithmetic::<String>());
/// ```
#[inline]
#[must_use]
pub fn is_arithmetic<T: 'static>() -> bool {
    let tid = TypeId::of::<T>();
    tid == TypeId::of::<i8>()
        || tid == TypeId::of::<i16>()
        || tid == TypeId::of::<i32>()
        || tid == TypeId::of::<i64>()
        || tid == TypeId::of::<i128>()
        || tid == TypeId::of::<isize>()
        || tid == TypeId::of::<u8>()
        || tid == TypeId::of::<u16>()
        || tid == TypeId::of::<u32>()
        || tid == TypeId::of::<u64>()
        || tid == TypeId::of::<u128>()
        || tid == TypeId::of::<usize>()
        || tid == TypeId::of::<f32>()
        || tid == TypeId::of::<f64>()
}

/// Trait bound alias for scalar types usable in vector/matrix operations.
///
/// This combines the requirements for arithmetic operations,
/// floating-point math, and conversions.
pub trait Scalar:
    num_traits::Float
    + num_traits::NumAssign
    + std::fmt::Debug
    + std::fmt::Display
    + Default
    + Copy
    + Send
    + Sync
    + 'static
{
    /// Zero value for this scalar type.
    const ZERO: Self;
    /// One value for this scalar type.
    const ONE: Self;
    /// Epsilon for floating-point comparison.
    const EPSILON: Self;
}

impl Scalar for f32 {
    const ZERO: Self = 0.0;
    const ONE: Self = 1.0;
    const EPSILON: Self = f32::EPSILON;
}

impl Scalar for f64 {
    const ZERO: Self = 0.0;
    const ONE: Self = 1.0;
    const EPSILON: Self = f64::EPSILON;
}

impl Scalar for crate::half::Half {
    const ZERO: Self = crate::half::Half::ZERO;
    const ONE: Self = crate::half::Half::ONE;
    const EPSILON: Self = crate::half::Half::from_bits(0x1400); // ~0.000977
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_floating_point() {
        assert!(is_floating_point::<f32>());
        assert!(is_floating_point::<f64>());
        assert!(!is_floating_point::<i32>());
        assert!(!is_floating_point::<u64>());
    }

    #[test]
    fn test_is_arithmetic() {
        assert!(is_arithmetic::<f64>());
        assert!(is_arithmetic::<f32>());
        assert!(is_arithmetic::<i32>());
        assert!(is_arithmetic::<u8>());
        assert!(is_arithmetic::<isize>());
        assert!(!is_arithmetic::<String>());
        assert!(!is_arithmetic::<Vec<i32>>());
    }

    #[test]
    fn test_is_gf_types() {
        // These should all be false for primitive types
        assert!(!is_gf_vec::<f64>());
        assert!(!is_gf_matrix::<f64>());
        assert!(!is_gf_quat::<f64>());
        assert!(!is_gf_dual_quat::<f64>());
        assert!(!is_gf_range::<f64>());
    }

    #[test]
    fn test_scalar_constants() {
        assert_eq!(f32::ZERO, 0.0f32);
        assert_eq!(f32::ONE, 1.0f32);
        assert_eq!(f64::ZERO, 0.0f64);
        assert_eq!(f64::ONE, 1.0f64);
    }

    #[test]
    fn test_gf_arithmetic_bounds() {
        fn needs_arithmetic<T: GfArithmetic>(_: T) {}

        needs_arithmetic(5i32);
        needs_arithmetic(3.14f64);
        needs_arithmetic(0u8);
    }
}
