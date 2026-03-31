//! Debug printing / stream output for VtValue and VtArray types.
//!
//! Port of pxr/base/vt/streamOut.h
//!
//! Provides `VtStreamOut` equivalents: formatting functions for
//! type-erased values and arrays to human-readable strings.

use std::fmt;

use crate::array::ShapeData;
use crate::value::Value;

/// Stream out a generic (non-displayable) type.
///
/// Matches C++ `Vt_StreamOutGeneric` - produces `<'typeName' @ 0xADDR>`.
pub fn stream_out_generic(type_name: &str, addr: usize) -> String {
    format!("<'{}' @ {:#x}>", type_name, addr)
}

/// Stream out a value to a string using Display if available, or generic format.
///
/// Matches C++ `VtStreamOut<T>`.
pub fn stream_out<T: fmt::Display>(obj: &T) -> String {
    format!("{}", obj)
}

/// Stream out a boolean value.
///
/// Matches C++ `VtStreamOut(bool const &, std::ostream &)`.
pub fn stream_out_bool(v: bool) -> String {
    if v {
        "true".to_string()
    } else {
        "false".to_string()
    }
}

/// Stream out a char value.
pub fn stream_out_char(v: char) -> String {
    format!("{}", v)
}

/// Stream out an unsigned byte value.
pub fn stream_out_u8(v: u8) -> String {
    format!("{}", v)
}

/// Stream out a signed byte value.
pub fn stream_out_i8(v: i8) -> String {
    format!("{}", v)
}

/// Stream out a float with full precision.
///
/// Matches C++ `VtStreamOut(float const &, std::ostream &)`.
pub fn stream_out_float(v: f32) -> String {
    // Use enough precision for roundtrip
    format!("{}", v)
}

/// Stream out a double with full precision.
///
/// Matches C++ `VtStreamOut(double const &, std::ostream &)`.
pub fn stream_out_double(v: f64) -> String {
    format!("{}", v)
}

/// Stream out an array with shape information.
///
/// Matches C++ `VtStreamOutArray`.
///
/// Produces output like: `[1, 2, 3]` for 1D or shaped output for multi-dimensional.
pub fn stream_out_array_shaped<T: fmt::Display>(shape: &ShapeData, values: &[T]) -> String {
    let total = shape.total_size;
    if total == 0 {
        return "[]".to_string();
    }

    let mut result = String::new();
    result.push('[');

    for (i, v) in values.iter().take(total).enumerate() {
        if i > 0 {
            result.push_str(", ");
        }
        // Limit output for very large arrays
        if i >= 1000 {
            result.push_str(&format!("... ({} more)", total - i));
            break;
        }
        result.push_str(&format!("{}", v));
    }

    result.push(']');
    result
}

/// Stream out a VtValue to a string.
///
/// This dispatches to the appropriate stream_out function based on the
/// runtime type of the value.
pub fn stream_out_value(value: &Value) -> String {
    // Try common types first
    if let Some(v) = value.get::<bool>() {
        return stream_out_bool(*v);
    }
    if let Some(v) = value.get::<i32>() {
        return format!("{}", v);
    }
    if let Some(v) = value.get::<i64>() {
        return format!("{}", v);
    }
    if let Some(v) = value.get::<u32>() {
        return format!("{}", v);
    }
    if let Some(v) = value.get::<u64>() {
        return format!("{}", v);
    }
    if let Some(v) = value.get::<f32>() {
        return stream_out_float(*v);
    }
    if let Some(v) = value.get::<f64>() {
        return stream_out_double(*v);
    }
    if let Some(v) = value.get::<String>() {
        return v.clone();
    }

    // Fall back to the Value's Display impl if available
    format!("{}", value)
}

/// Trait for types that can stream themselves out.
///
/// Implement this to customize how a type appears when stored
/// in a VtValue and printed.
pub trait StreamOutable: fmt::Display {
    /// Stream this value to a string.
    fn stream_out(&self) -> String {
        format!("{}", self)
    }
}

// Blanket impl for all Display types
impl<T: fmt::Display> StreamOutable for T {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_out_generic() {
        let s = stream_out_generic("MyType", 0xDEAD);
        assert!(s.contains("MyType"));
        assert!(s.contains("0xdead"));
    }

    #[test]
    fn test_stream_out_bool() {
        assert_eq!(stream_out_bool(true), "true");
        assert_eq!(stream_out_bool(false), "false");
    }

    #[test]
    fn test_stream_out_float() {
        let s = stream_out_float(3.14);
        assert!(s.starts_with("3.14"));
    }

    #[test]
    fn test_stream_out_double() {
        let s = stream_out_double(2.718281828459045);
        assert!(s.starts_with("2.718"));
    }

    #[test]
    fn test_stream_out_value() {
        let v = Value::from(42i32);
        assert_eq!(stream_out_value(&v), "42");

        let v = Value::from(true);
        assert_eq!(stream_out_value(&v), "true");

        let v = Value::from("hello".to_string());
        assert_eq!(stream_out_value(&v), "hello");
    }

    #[test]
    fn test_stream_out_display() {
        assert_eq!(stream_out(&42), "42");
        assert_eq!(stream_out(&"hello"), "hello");
    }

    #[test]
    fn test_stream_outable() {
        assert_eq!(42i32.stream_out(), "42");
        assert_eq!(3.14f64.stream_out(), "3.14");
    }
}
