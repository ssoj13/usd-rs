//! Type traits for vt types.
//!
//! This module provides trait definitions for identifying array types and
//! determining value type characteristics.

use std::any::TypeId;

use super::Array;

/// Marker trait for array types.
///
/// Implemented for `Array<T>` to allow generic code to detect array types.
pub trait VtArray {}

impl<T: Clone + Send + Sync + 'static> VtArray for Array<T> {}

/// Marker trait for array edit types.
///
/// Implemented for `ArrayEdit<T>` to allow generic code to detect array edit types.
/// The actual implementation is in array_edit.rs to satisfy orphan rules.
pub trait VtArrayEdit {}

/// Trait for types that can be stored in a VtValue.
///
/// All types implementing this trait can be wrapped in a `Value`.
pub trait VtType: Clone + Send + Sync + 'static {
    /// Returns the type name.
    fn vt_type_name() -> &'static str {
        std::any::type_name::<Self>()
    }
}

// Implement VtType for common types
impl VtType for bool {}
impl VtType for i8 {}
impl VtType for i16 {}
impl VtType for i32 {}
impl VtType for i64 {}
impl VtType for u8 {}
impl VtType for u16 {}
impl VtType for u32 {}
impl VtType for u64 {}
impl VtType for f32 {}
impl VtType for f64 {}
impl VtType for String {}

// Implement for gf types
impl VtType for usd_gf::Vec2d {}
impl VtType for usd_gf::Vec2f {}
impl VtType for usd_gf::Vec2i {}
impl VtType for usd_gf::Vec3d {}
impl VtType for usd_gf::Vec3f {}
impl VtType for usd_gf::Vec3i {}
impl VtType for usd_gf::Vec4d {}
impl VtType for usd_gf::Vec4f {}
impl VtType for usd_gf::Vec4i {}
impl VtType for usd_gf::Matrix2d {}
impl VtType for usd_gf::Matrix2f {}
impl VtType for usd_gf::Matrix3d {}
impl VtType for usd_gf::Matrix3f {}
impl VtType for usd_gf::Matrix4d {}
impl VtType for usd_gf::Matrix4f {}
impl VtType for usd_gf::Quatd {}
impl VtType for usd_gf::Quatf {}
impl VtType for usd_gf::Half {}

// Vec2h, Vec3h, Vec4h
impl VtType for usd_gf::Vec2h {}
impl VtType for usd_gf::Vec3h {}
impl VtType for usd_gf::Vec4h {}

// Quaternions
impl VtType for usd_gf::Quath {}

// DualQuaternions
impl VtType for usd_gf::DualQuatd {}
impl VtType for usd_gf::DualQuatf {}
impl VtType for usd_gf::DualQuath {}

// Ranges
impl VtType for usd_gf::Range1d {}
impl VtType for usd_gf::Range1f {}
impl VtType for usd_gf::Range2d {}
impl VtType for usd_gf::Range2f {}
impl VtType for usd_gf::Range3d {}
impl VtType for usd_gf::Range3f {}

// Other types
impl VtType for usd_gf::Rect2i {}
impl VtType for usd_gf::Interval {}
impl VtType for usd_gf::MultiInterval {}
impl VtType for usd_gf::Frustum {}

// Token
impl VtType for usd_tf::Token {}

/// Checks if a type is an array type.
///
/// # Examples
///
/// ```
/// use usd_vt::{Array, is_array};
///
/// assert!(is_array::<Array<i32>>());
/// assert!(!is_array::<i32>());
/// ```
#[inline]
#[must_use]
pub fn is_array<T: 'static>() -> bool {
    let tid = TypeId::of::<T>();
    // All VT_SCALAR_VALUE_TYPES array variants (matching C++ VtIsArray<T>)
    tid == TypeId::of::<Array<bool>>()
        || tid == TypeId::of::<Array<u8>>()
        || tid == TypeId::of::<Array<i8>>()
        || tid == TypeId::of::<Array<u16>>()
        || tid == TypeId::of::<Array<i16>>()
        || tid == TypeId::of::<Array<i32>>()
        || tid == TypeId::of::<Array<u32>>()
        || tid == TypeId::of::<Array<i64>>()
        || tid == TypeId::of::<Array<u64>>()
        || tid == TypeId::of::<Array<f32>>()
        || tid == TypeId::of::<Array<f64>>()
        || tid == TypeId::of::<Array<usd_gf::half::Half>>()
        || tid == TypeId::of::<Array<String>>()
        || tid == TypeId::of::<Array<usd_tf::Token>>()
        // Vec2
        || tid == TypeId::of::<Array<usd_gf::Vec2i>>()
        || tid == TypeId::of::<Array<usd_gf::Vec2f>>()
        || tid == TypeId::of::<Array<usd_gf::Vec2d>>()
        || tid == TypeId::of::<Array<usd_gf::Vec2h>>()
        // Vec3
        || tid == TypeId::of::<Array<usd_gf::Vec3i>>()
        || tid == TypeId::of::<Array<usd_gf::Vec3f>>()
        || tid == TypeId::of::<Array<usd_gf::Vec3d>>()
        || tid == TypeId::of::<Array<usd_gf::Vec3h>>()
        // Vec4
        || tid == TypeId::of::<Array<usd_gf::Vec4i>>()
        || tid == TypeId::of::<Array<usd_gf::Vec4f>>()
        || tid == TypeId::of::<Array<usd_gf::Vec4d>>()
        || tid == TypeId::of::<Array<usd_gf::Vec4h>>()
        // Matrix
        || tid == TypeId::of::<Array<usd_gf::Matrix2d>>()
        || tid == TypeId::of::<Array<usd_gf::Matrix2f>>()
        || tid == TypeId::of::<Array<usd_gf::Matrix3d>>()
        || tid == TypeId::of::<Array<usd_gf::Matrix3f>>()
        || tid == TypeId::of::<Array<usd_gf::Matrix4d>>()
        || tid == TypeId::of::<Array<usd_gf::Matrix4f>>()
        // Quaternion
        || tid == TypeId::of::<Array<usd_gf::Quatd>>()
        || tid == TypeId::of::<Array<usd_gf::Quatf>>()
        || tid == TypeId::of::<Array<usd_gf::Quath>>()
        // DualQuat
        || tid == TypeId::of::<Array<usd_gf::DualQuatd>>()
        || tid == TypeId::of::<Array<usd_gf::DualQuatf>>()
        || tid == TypeId::of::<Array<usd_gf::DualQuath>>()
        // Range
        || tid == TypeId::of::<Array<usd_gf::Range1d>>()
        || tid == TypeId::of::<Array<usd_gf::Range1f>>()
        || tid == TypeId::of::<Array<usd_gf::Range2d>>()
        || tid == TypeId::of::<Array<usd_gf::Range2f>>()
        || tid == TypeId::of::<Array<usd_gf::Range3d>>()
        || tid == TypeId::of::<Array<usd_gf::Range3f>>()
        // Misc
        || tid == TypeId::of::<Array<usd_gf::Rect2i>>()
        || tid == TypeId::of::<Array<usd_gf::Interval>>()
}

/// Trait for types that have cheap copy semantics.
///
/// Types implementing this trait are candidates for local storage
/// optimization in Value.
pub trait ValueTypeHasCheapCopy {}

// Primitives have cheap copy
impl ValueTypeHasCheapCopy for bool {}
impl ValueTypeHasCheapCopy for i8 {}
impl ValueTypeHasCheapCopy for i16 {}
impl ValueTypeHasCheapCopy for i32 {}
impl ValueTypeHasCheapCopy for i64 {}
impl ValueTypeHasCheapCopy for u8 {}
impl ValueTypeHasCheapCopy for u16 {}
impl ValueTypeHasCheapCopy for u32 {}
impl ValueTypeHasCheapCopy for u64 {}
impl ValueTypeHasCheapCopy for f32 {}
impl ValueTypeHasCheapCopy for f64 {}

/// Trait indicating whether VtValue compose-over functionality can be
/// registered for a type.
///
/// Types implementing this trait support compose-over operations in VtValue.
pub trait ValueTypeCanCompose {}

/// Helper macro for marking types as supporting compose operations.
#[macro_export]
macro_rules! vt_value_type_can_compose {
    ($t:ty) => {
        impl $crate::ValueTypeCanCompose for $t {}
    };
}

/// Trait indicating whether VtValue transform functionality can be registered
/// for a type.
///
/// Types implementing this trait support transform operations in VtValue.
pub trait ValueTypeCanTransform {}

/// Helper macro for marking types as supporting transform operations.
#[macro_export]
macro_rules! vt_value_type_can_transform {
    ($t:ty) => {
        impl $crate::ValueTypeCanTransform for $t {}
    };
}

/// Base trait for typed value proxies.
///
/// Types implementing this trait can be used as typed proxies in VtValue,
/// where the proxied type can be determined at compile-time.
pub trait TypedValueProxyBase {}

/// Marker trait for typed value proxy types.
pub trait IsTypedValueProxy {}

impl<T: TypedValueProxyBase> IsTypedValueProxy for T {}

/// Base trait for erased value proxies.
///
/// Types implementing this trait can be used as type-erased proxies in VtValue,
/// where the proxied type cannot be determined at compile-time.
pub trait ErasedValueProxyBase {
    /// Returns the proxied value, resolved at runtime.
    fn get_erased_proxied_value(&self) -> super::Value;
}

/// Marker trait for erased value proxy types.
pub trait IsErasedValueProxy {}

impl<T: ErasedValueProxyBase> IsErasedValueProxy for T {}

/// Marker trait for all value proxy types (typed or erased).
pub trait IsValueProxy {}

// Note: Blanket implementations would conflict due to overlapping trait bounds.
// Proxy types should explicitly implement IsValueProxy when needed.

/// Get the proxied object from a typed value proxy.
///
/// For non-proxy types, returns the value itself.
pub trait GetProxiedObject {
    /// The type being proxied.
    type Proxied;
    /// Returns a reference to the proxied object.
    fn get_proxied(&self) -> &Self::Proxied;
}

// Default implementation for non-proxy types
// Note: Negative trait bounds are unstable, so we skip this for now

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_array() {
        assert!(is_array::<Array<i32>>());
        assert!(is_array::<Array<f64>>());
        assert!(!is_array::<i32>());
        assert!(!is_array::<Vec<i32>>());
    }

    #[test]
    fn test_vt_type_name() {
        assert!(i32::vt_type_name().contains("i32"));
        assert!(f64::vt_type_name().contains("f64"));
    }
}
