//! Type helper utilities for splines.
//!
//! Port of pxr/base/ts/typeHelpers.h
//!
//! Provides type mapping and validation for spline value types.

use super::knot_data::KnotValueType;

/// Trait for supported spline value types.
///
/// Implemented for types that can be used as spline values.
pub trait SupportedValueType: Copy + Clone + Default + PartialEq + Send + Sync + 'static {
    /// Returns the value type enum for this type.
    fn value_type() -> KnotValueType;

    /// Returns true if the value is finite (not NaN or Inf).
    fn is_finite(&self) -> bool;

    /// Converts from f64.
    fn from_f64(val: f64) -> Self;

    /// Converts to f64.
    fn to_f64(&self) -> f64;
}

impl SupportedValueType for f64 {
    fn value_type() -> KnotValueType {
        KnotValueType::Double
    }

    fn is_finite(&self) -> bool {
        f64::is_finite(*self)
    }

    fn from_f64(val: f64) -> Self {
        val
    }

    fn to_f64(&self) -> f64 {
        *self
    }
}

impl SupportedValueType for f32 {
    fn value_type() -> KnotValueType {
        KnotValueType::Float
    }

    fn is_finite(&self) -> bool {
        f32::is_finite(*self)
    }

    fn from_f64(val: f64) -> Self {
        val as f32
    }

    fn to_f64(&self) -> f64 {
        *self as f64
    }
}

// Note: Half (f16) would need additional support for the half crate

/// Returns true if the value is finite (not NaN or infinity).
///
/// Works with any numeric type that implements SupportedValueType.
pub fn is_finite<T: SupportedValueType>(value: &T) -> bool {
    value.is_finite()
}

/// Returns the KnotValueType for a supported value type.
pub fn get_value_type<T: SupportedValueType>() -> KnotValueType {
    T::value_type()
}

/// Returns the type name string for a value type.
pub fn get_type_name(value_type: KnotValueType) -> &'static str {
    match value_type {
        KnotValueType::Double => "double",
        KnotValueType::Float => "float",
        KnotValueType::Half => "half",
    }
}

/// Returns the value type from a type name string.
pub fn get_type_from_name(type_name: &str) -> Option<KnotValueType> {
    match type_name {
        "double" | "float64" | "f64" => Some(KnotValueType::Double),
        "float" | "float32" | "f32" => Some(KnotValueType::Float),
        "half" | "float16" | "f16" => Some(KnotValueType::Half),
        _ => None,
    }
}

/// Checks if a type is a supported spline value type.
pub const fn is_supported_value_type<T: SupportedValueType>() -> bool {
    true
}

/// Clamps a value to the valid range for type T.
///
/// Prevents infinity when converting from higher precision types.
pub fn clamp_to_range<T: SupportedValueType>(value: f64) -> T {
    let typed = T::from_f64(value);
    if typed.is_finite() {
        typed
    } else if value > 0.0 {
        // Positive infinity - return max finite value
        T::from_f64(f64::MAX)
    } else if value < 0.0 {
        // Negative infinity - return min finite value
        T::from_f64(f64::MIN)
    } else {
        // NaN
        T::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supported_f64() {
        assert_eq!(f64::value_type(), KnotValueType::Double);
        assert!(1.5f64.is_finite());
        assert!(!f64::INFINITY.is_finite());
        assert!(!f64::NAN.is_finite());
    }

    #[test]
    fn test_supported_f32() {
        assert_eq!(f32::value_type(), KnotValueType::Float);
        assert!(1.5f32.is_finite());
        assert!(!f32::INFINITY.is_finite());
        assert!(!f32::NAN.is_finite());
    }

    #[test]
    fn test_get_type_name() {
        assert_eq!(get_type_name(KnotValueType::Double), "double");
        assert_eq!(get_type_name(KnotValueType::Float), "float");
        assert_eq!(get_type_name(KnotValueType::Half), "half");
    }

    #[test]
    fn test_get_type_from_name() {
        assert_eq!(get_type_from_name("double"), Some(KnotValueType::Double));
        assert_eq!(get_type_from_name("float"), Some(KnotValueType::Float));
        assert_eq!(get_type_from_name("half"), Some(KnotValueType::Half));
        assert_eq!(get_type_from_name("unknown"), None);
    }

    #[test]
    fn test_conversions() {
        let val: f64 = f64::from_f64(1.5);
        assert!((val - 1.5).abs() < 1e-10);
        assert!((val.to_f64() - 1.5).abs() < 1e-10);

        let val32: f32 = f32::from_f64(1.5);
        assert!((val32 - 1.5f32).abs() < 1e-6);
        assert!((val32.to_f64() - 1.5).abs() < 1e-6);
    }
}
