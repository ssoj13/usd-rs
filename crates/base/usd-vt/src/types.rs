//! Built-in type definitions and array type aliases.
//!
//! This module provides type aliases for common array types used throughout USD,
//! equivalent to OpenUSD's VtIntArray, VtFloatArray, etc.
//!
//! # Examples
//!
//! ```
//! use usd_vt::{IntArray, FloatArray, DoubleArray};
//!
//! let ints: IntArray = vec![1, 2, 3].into();
//! let floats: FloatArray = vec![1.0, 2.0, 3.0].into();
//! let doubles: DoubleArray = vec![1.0, 2.0, 3.0].into();
//! ```

use super::Array;

// =============================================================================
// Scalar type arrays
// =============================================================================

/// Array of bool values.
pub type BoolArray = Array<bool>;

/// Array of i8 values.
pub type CharArray = Array<i8>;

/// Array of u8 values.
pub type UCharArray = Array<u8>;

/// Array of i16 values.
pub type ShortArray = Array<i16>;

/// Array of u16 values.
pub type UShortArray = Array<u16>;

/// Array of i32 values (VtIntArray equivalent).
pub type IntArray = Array<i32>;

/// Array of u32 values.
pub type UIntArray = Array<u32>;

/// Array of i64 values.
pub type Int64Array = Array<i64>;

/// Array of u64 values.
pub type UInt64Array = Array<u64>;

/// Array of f32 values (VtFloatArray equivalent).
pub type FloatArray = Array<f32>;

/// Array of f64 values (VtDoubleArray equivalent).
pub type DoubleArray = Array<f64>;

/// Array of half-precision float values.
pub type HalfArray = Array<usd_gf::Half>;

/// Array of String values.
pub type StringArray = Array<String>;

/// Array of TfToken values.
pub type TokenArray = Array<usd_tf::Token>;

// =============================================================================
// Vector type arrays
// =============================================================================

/// Array of Vec2i values.
pub type Vec2iArray = Array<usd_gf::Vec2i>;

/// Array of Vec2f values.
pub type Vec2fArray = Array<usd_gf::Vec2f>;

/// Array of Vec2d values.
pub type Vec2dArray = Array<usd_gf::Vec2d>;

/// Array of Vec2h values.
pub type Vec2hArray = Array<usd_gf::Vec2h>;

/// Array of Vec3i values.
pub type Vec3iArray = Array<usd_gf::Vec3i>;

/// Array of Vec3f values.
pub type Vec3fArray = Array<usd_gf::Vec3f>;

/// Array of Vec3d values.
pub type Vec3dArray = Array<usd_gf::Vec3d>;

/// Array of Vec3h values.
pub type Vec3hArray = Array<usd_gf::Vec3h>;

/// Array of Vec4i values.
pub type Vec4iArray = Array<usd_gf::Vec4i>;

/// Array of Vec4f values.
pub type Vec4fArray = Array<usd_gf::Vec4f>;

/// Array of Vec4d values.
pub type Vec4dArray = Array<usd_gf::Vec4d>;

/// Array of Vec4h values.
pub type Vec4hArray = Array<usd_gf::Vec4h>;

// =============================================================================
// Matrix type arrays
// =============================================================================

/// Array of Matrix2f values.
pub type Matrix2fArray = Array<usd_gf::Matrix2f>;

/// Array of Matrix2d values.
pub type Matrix2dArray = Array<usd_gf::Matrix2d>;

/// Array of Matrix3f values.
pub type Matrix3fArray = Array<usd_gf::Matrix3f>;

/// Array of Matrix3d values.
pub type Matrix3dArray = Array<usd_gf::Matrix3d>;

/// Array of Matrix4f values.
pub type Matrix4fArray = Array<usd_gf::Matrix4f>;

/// Array of Matrix4d values.
pub type Matrix4dArray = Array<usd_gf::Matrix4d>;

// =============================================================================
// Quaternion type arrays
// =============================================================================

/// Array of Quatf values.
pub type QuatfArray = Array<usd_gf::Quatf>;

/// Array of Quatd values.
pub type QuatdArray = Array<usd_gf::Quatd>;

/// Array of Quath values.
pub type QuathArray = Array<usd_gf::Quath>;

/// Array of DualQuatf values.
pub type DualQuatfArray = Array<usd_gf::DualQuatf>;

/// Array of DualQuatd values.
pub type DualQuatdArray = Array<usd_gf::DualQuatd>;

/// Array of DualQuath values.
pub type DualQuathArray = Array<usd_gf::DualQuath>;

/// Array of legacy Quaternion values.
pub type QuaternionArray = Array<usd_gf::Quaternion>;

// =============================================================================
// Range type arrays
// =============================================================================

/// Array of Range1f values.
pub type Range1fArray = Array<usd_gf::Range1f>;

/// Array of Range1d values.
pub type Range1dArray = Array<usd_gf::Range1d>;

/// Array of Range2f values.
pub type Range2fArray = Array<usd_gf::Range2f>;

/// Array of Range2d values.
pub type Range2dArray = Array<usd_gf::Range2d>;

/// Array of Range3f values.
pub type Range3fArray = Array<usd_gf::Range3f>;

/// Array of Range3d values.
pub type Range3dArray = Array<usd_gf::Range3d>;

/// Array of Interval values.
pub type IntervalArray = Array<usd_gf::Interval>;

/// Array of Rect2i values.
pub type Rect2iArray = Array<usd_gf::Rect2i>;

// =============================================================================
// Zero values for types
// =============================================================================

/// Returns the "zero" value for a type.
///
/// This is used for initializing arrays and default values.
pub trait VtZero: Default {
    /// Returns the zero/default value for this type.
    fn vt_zero() -> Self {
        Self::default()
    }
}

// Implement VtZero for all types that implement Default
impl<T: Default> VtZero for T {}

// =============================================================================
// Shape data for legacy multi-dimensional arrays
// =============================================================================

/// Shape representation for legacy VtArray multi-dimensional support.
///
/// This structure stores array shape information. Modern USD typically uses
/// 1D arrays with typed elements (e.g., `Array<Vec3f>` instead of 2D array).
/// This is maintained for compatibility with legacy code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ShapeData {
    /// Total number of elements in the array.
    pub total_size: usize,
    /// Sizes of dimensions other than the last.
    /// The last dimension size is computed as total_size / product(other_dims).
    pub other_dims: [u32; 3],
}

impl ShapeData {
    /// Creates a new ShapeData with given total size.
    #[inline]
    #[must_use]
    pub fn new(total_size: usize) -> Self {
        Self {
            total_size,
            other_dims: [0, 0, 0],
        }
    }

    /// Returns the rank (number of dimensions) of the array.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_vt::ShapeData;
    ///
    /// let shape = ShapeData::new(10);
    /// assert_eq!(shape.get_rank(), 1); // 1D array
    /// ```
    #[inline]
    #[must_use]
    pub fn get_rank(&self) -> u32 {
        if self.other_dims[0] == 0 {
            1
        } else if self.other_dims[1] == 0 {
            2
        } else if self.other_dims[2] == 0 {
            3
        } else {
            4
        }
    }

    /// Clears all shape data.
    #[inline]
    pub fn clear(&mut self) {
        self.total_size = 0;
        self.other_dims = [0, 0, 0];
    }

    /// Returns true if the shape is empty (no elements).
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.total_size == 0
    }
}

// =============================================================================
// Known type index for VtVisitValue
// =============================================================================

use std::any::TypeId;
use std::collections::HashMap;
use std::sync::OnceLock;

/// Total number of known value types.
pub const KNOWN_TYPE_COUNT: usize = 56;

/// Build the TypeId -> index map at first access.
fn known_type_map() -> &'static HashMap<TypeId, usize> {
    static MAP: OnceLock<HashMap<TypeId, usize>> = OnceLock::new();
    MAP.get_or_init(|| {
        let mut m = HashMap::with_capacity(KNOWN_TYPE_COUNT);
        // Scalar types (0..16)
        m.insert(TypeId::of::<bool>(), 0);
        m.insert(TypeId::of::<i8>(), 1);
        m.insert(TypeId::of::<u8>(), 2);
        m.insert(TypeId::of::<i16>(), 3);
        m.insert(TypeId::of::<u16>(), 4);
        m.insert(TypeId::of::<i32>(), 5);
        m.insert(TypeId::of::<u32>(), 6);
        m.insert(TypeId::of::<i64>(), 7);
        m.insert(TypeId::of::<u64>(), 8);
        m.insert(TypeId::of::<f32>(), 9);
        m.insert(TypeId::of::<f64>(), 10);
        m.insert(TypeId::of::<usd_gf::Half>(), 11);
        m.insert(TypeId::of::<String>(), 12);
        m.insert(TypeId::of::<usd_tf::Token>(), 13);
        // AssetPath and TimeCode are now defined in usd-vt; register their type IDs directly.
        m.insert(TypeId::of::<crate::AssetPath>(), 14);
        m.insert(TypeId::of::<crate::TimeCode>(), 15);
        // Vector types (16..27)
        m.insert(TypeId::of::<usd_gf::Vec2i>(), 16);
        m.insert(TypeId::of::<usd_gf::Vec2f>(), 17);
        m.insert(TypeId::of::<usd_gf::Vec2d>(), 18);
        m.insert(TypeId::of::<usd_gf::Vec2h>(), 19);
        m.insert(TypeId::of::<usd_gf::Vec3i>(), 20);
        m.insert(TypeId::of::<usd_gf::Vec3f>(), 21);
        m.insert(TypeId::of::<usd_gf::Vec3d>(), 22);
        m.insert(TypeId::of::<usd_gf::Vec3h>(), 23);
        m.insert(TypeId::of::<usd_gf::Vec4i>(), 24);
        m.insert(TypeId::of::<usd_gf::Vec4f>(), 25);
        m.insert(TypeId::of::<usd_gf::Vec4d>(), 26);
        m.insert(TypeId::of::<usd_gf::Vec4h>(), 27);
        // Matrix types (28..33)
        m.insert(TypeId::of::<usd_gf::Matrix2f>(), 28);
        m.insert(TypeId::of::<usd_gf::Matrix2d>(), 29);
        m.insert(TypeId::of::<usd_gf::Matrix3f>(), 30);
        m.insert(TypeId::of::<usd_gf::Matrix3d>(), 31);
        m.insert(TypeId::of::<usd_gf::Matrix4f>(), 32);
        m.insert(TypeId::of::<usd_gf::Matrix4d>(), 33);
        // Quaternion types (34..40)
        m.insert(TypeId::of::<usd_gf::Quatf>(), 34);
        m.insert(TypeId::of::<usd_gf::Quatd>(), 35);
        m.insert(TypeId::of::<usd_gf::Quath>(), 36);
        m.insert(TypeId::of::<usd_gf::DualQuatf>(), 37);
        m.insert(TypeId::of::<usd_gf::DualQuatd>(), 38);
        m.insert(TypeId::of::<usd_gf::DualQuath>(), 39);
        m.insert(TypeId::of::<usd_gf::Quaternion>(), 40);
        // Range types (41..48)
        m.insert(TypeId::of::<usd_gf::Range1f>(), 41);
        m.insert(TypeId::of::<usd_gf::Range1d>(), 42);
        m.insert(TypeId::of::<usd_gf::Range2f>(), 43);
        m.insert(TypeId::of::<usd_gf::Range2d>(), 44);
        m.insert(TypeId::of::<usd_gf::Range3f>(), 45);
        m.insert(TypeId::of::<usd_gf::Range3d>(), 46);
        m.insert(TypeId::of::<usd_gf::Interval>(), 47);
        m.insert(TypeId::of::<usd_gf::Rect2i>(), 48);
        // Container types (49..55)
        m.insert(TypeId::of::<super::Dictionary>(), 49);
        m.insert(TypeId::of::<Array<bool>>(), 50);
        m.insert(TypeId::of::<Array<i32>>(), 51);
        m.insert(TypeId::of::<Array<f32>>(), 52);
        m.insert(TypeId::of::<Array<f64>>(), 53);
        m.insert(TypeId::of::<Array<usd_gf::Vec3f>>(), 54);
        m.insert(TypeId::of::<Array<usd_gf::Matrix4d>>(), 55);
        m
    })
}

/// Returns the known-type index for type `T`, or `None` if `T` is not a known type.
///
/// Known types are the core scalar, vector, matrix, quaternion, and range types
/// used throughout USD. This index can be used for O(1) dispatch tables.
#[inline]
pub fn get_known_value_type_index<T: 'static>() -> Option<usize> {
    known_type_map().get(&TypeId::of::<T>()).copied()
}

/// Returns true if `T` is a known USD value type.
#[inline]
pub fn is_known_value_type<T: 'static>() -> bool {
    known_type_map().contains_key(&TypeId::of::<T>())
}

/// Returns the known-type index for a runtime TypeId, or `None`.
#[inline]
pub fn get_known_type_index_by_id(type_id: TypeId) -> Option<usize> {
    known_type_map().get(&type_id).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scalar_arrays() {
        let ints: IntArray = vec![1, 2, 3].into();
        assert_eq!(ints.len(), 3);
        assert_eq!(ints[0], 1);

        let floats: FloatArray = vec![1.0, 2.0, 3.0].into();
        assert_eq!(floats.len(), 3);

        let doubles: DoubleArray = vec![1.0, 2.0, 3.0].into();
        assert_eq!(doubles.len(), 3);
    }

    #[test]
    fn test_vector_arrays() {
        use usd_gf::Vec3f;

        let vecs: Vec3fArray = vec![
            Vec3f::new(1.0, 0.0, 0.0),
            Vec3f::new(0.0, 1.0, 0.0),
            Vec3f::new(0.0, 0.0, 1.0),
        ]
        .into();
        assert_eq!(vecs.len(), 3);
    }

    #[test]
    fn test_matrix_arrays() {
        use usd_gf::Matrix4d;

        let mats: Matrix4dArray = vec![Matrix4d::identity(), Matrix4d::identity()].into();
        assert_eq!(mats.len(), 2);
    }

    #[test]
    fn test_vt_zero() {
        assert_eq!(i32::vt_zero(), 0);
        assert_eq!(f64::vt_zero(), 0.0);
        assert_eq!(bool::vt_zero(), false);
    }

    #[test]
    fn test_known_type_index_scalars() {
        assert_eq!(get_known_value_type_index::<bool>(), Some(0));
        assert_eq!(get_known_value_type_index::<i32>(), Some(5));
        assert_eq!(get_known_value_type_index::<f64>(), Some(10));
        assert_eq!(get_known_value_type_index::<String>(), Some(12));
    }

    #[test]
    fn test_known_type_index_vectors() {
        assert_eq!(get_known_value_type_index::<usd_gf::Vec3f>(), Some(21));
        assert_eq!(get_known_value_type_index::<usd_gf::Vec4d>(), Some(26));
    }

    #[test]
    fn test_known_type_index_matrices() {
        assert_eq!(get_known_value_type_index::<usd_gf::Matrix4d>(), Some(33));
        assert_eq!(get_known_value_type_index::<usd_gf::Matrix2f>(), Some(28));
    }

    #[test]
    fn test_is_known_value_type() {
        assert!(is_known_value_type::<i32>());
        assert!(is_known_value_type::<f64>());
        assert!(is_known_value_type::<usd_gf::Vec3f>());
        // Custom struct is NOT a known type
        assert!(!is_known_value_type::<ShapeData>());
    }

    #[test]
    fn test_known_type_index_by_id() {
        use std::any::TypeId;
        assert_eq!(get_known_type_index_by_id(TypeId::of::<i32>()), Some(5));
        assert_eq!(get_known_type_index_by_id(TypeId::of::<ShapeData>()), None);
    }
}
