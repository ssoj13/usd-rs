//! Type header consolidation for VtValue-supported types.
//!
//! Port of pxr/base/vt/typeHeaders.h
//!
//! In C++, this header includes all the types that VtValue can hold.
//! In Rust, we re-export the relevant types from usd-gf and usd-tf
//! for convenience when working with VtValue.

// Re-export array and edit types from this crate
pub use crate::array::Array;
pub use crate::array_edit::ArrayEdit;

// Re-export Token from usd-tf
pub use usd_tf::Token;

// Re-export usd-gf types commonly held in VtValue.
// These match the C++ typeHeaders.h includes.
pub use usd_gf::{
    // Dual quaternions
    DualQuatd,
    DualQuatf,
    DualQuath,
    // Frustum
    Frustum,
    // Interval
    Interval,
    // Matrices
    Matrix2d,
    Matrix2f,
    Matrix3d,
    Matrix3f,
    Matrix4d,
    Matrix4f,
    MultiInterval,
    // Quaternions
    Quatd,
    Quaternion,
    Quatf,
    Quath,
    // Ranges
    Range1d,
    Range1f,
    Range2d,
    Range2f,
    Range3d,
    Range3f,
    // Rect
    Rect2i,
    // Vectors
    Vec2d,
    Vec2f,
    Vec2h,
    Vec2i,
    Vec3d,
    Vec3f,
    Vec3h,
    Vec3i,
    Vec4d,
    Vec4f,
    Vec4h,
    Vec4i,
};

/// List of all scalar type names that VtValue supports.
///
/// This is useful for dispatch tables and type registration.
pub const SCALAR_TYPE_NAMES: &[&str] = &[
    "bool",
    "u8",
    "i32",
    "u32",
    "i64",
    "u64",
    "f16",
    "f32",
    "f64",
    "String",
    "Token",
    "AssetPath",
    "Matrix2d",
    "Matrix2f",
    "Matrix3d",
    "Matrix3f",
    "Matrix4d",
    "Matrix4f",
    "Quatd",
    "Quatf",
    "Quath",
    "Quaternion",
    "DualQuatd",
    "DualQuatf",
    "DualQuath",
    "Vec2d",
    "Vec2f",
    "Vec2h",
    "Vec2i",
    "Vec3d",
    "Vec3f",
    "Vec3h",
    "Vec3i",
    "Vec4d",
    "Vec4f",
    "Vec4h",
    "Vec4i",
    "Range1d",
    "Range1f",
    "Range2d",
    "Range2f",
    "Range3d",
    "Range3f",
    "Rect2i",
    "Interval",
    "MultiInterval",
    "Frustum",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scalar_type_names() {
        assert!(SCALAR_TYPE_NAMES.len() > 30);
        assert!(SCALAR_TYPE_NAMES.contains(&"Matrix4d"));
        assert!(SCALAR_TYPE_NAMES.contains(&"Vec3f"));
        assert!(SCALAR_TYPE_NAMES.contains(&"Token"));
    }

    #[test]
    fn test_reexports_accessible() {
        // Verify key types are accessible
        let _m = Matrix4d::default();
        let _v = Vec3f::default();
        let _q = Quatd::default();
    }
}
