//! Integer type definitions.
//!
//! This module provides standard integer type definitions for compatibility
//! with C++ code. This is the Rust equivalent of `pxr/base/arch/inttypes.h`.
//!
//! In Rust, these types are already available in the standard library,
//! but this module provides them in a way that matches the C++ API.
//!
//! # Example
//!
//! ```ignore
//! use usd_arch::{int16_t, uint32_t, uchar};
//!
//! let value: uchar = 42;
//! let int16: int16_t = 1000;
//! let uint32: uint32_t = 1000000;
//! ```

// Re-export standard integer types from std::os::raw
// These match the C types used in the C++ codebase
// Note: We use non-camel-case names to match C++ API exactly
#[allow(non_camel_case_types)]
mod inttypes_impl {
    /// Signed 8-bit integer (equivalent to C `int8_t`)
    pub type int8_t = i8;

    /// Unsigned 8-bit integer (equivalent to C `uint8_t`)
    pub type uint8_t = u8;

    /// Signed 16-bit integer (equivalent to C `int16_t`)
    pub type int16_t = i16;

    /// Unsigned 16-bit integer (equivalent to C `uint16_t`)
    pub type uint16_t = u16;

    /// Signed 32-bit integer (equivalent to C `int32_t`)
    pub type int32_t = i32;

    /// Unsigned 32-bit integer (equivalent to C `uint32_t`)
    pub type uint32_t = u32;

    /// Signed 64-bit integer (equivalent to C `int64_t`)
    pub type int64_t = i64;

    /// Unsigned 64-bit integer (equivalent to C `uint64_t`)
    pub type uint64_t = u64;

    /// Unsigned char (equivalent to C `uchar`)
    pub type uchar = u8;
}

pub use inttypes_impl::*;

// Constants for integer limits (matching C limits.h)
/// Maximum value for `int8_t`
pub const INT8_MAX: i8 = i8::MAX;

/// Minimum value for `int8_t`
pub const INT8_MIN: i8 = i8::MIN;

/// Maximum value for `uint8_t`
pub const UINT8_MAX: u8 = u8::MAX;

/// Maximum value for `int16_t`
pub const INT16_MAX: i16 = i16::MAX;

/// Minimum value for `int16_t`
pub const INT16_MIN: i16 = i16::MIN;

/// Maximum value for `uint16_t`
pub const UINT16_MAX: u16 = u16::MAX;

/// Maximum value for `int32_t`
pub const INT32_MAX: i32 = i32::MAX;

/// Minimum value for `int32_t`
pub const INT32_MIN: i32 = i32::MIN;

/// Maximum value for `uint32_t`
pub const UINT32_MAX: u32 = u32::MAX;

/// Maximum value for `int64_t`
pub const INT64_MAX: i64 = i64::MAX;

/// Minimum value for `int64_t`
pub const INT64_MIN: i64 = i64::MIN;

/// Maximum value for `uint64_t`
pub const UINT64_MAX: u64 = u64::MAX;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_int_types() {
        let _i8: int8_t = -128;
        let _u8: uint8_t = 255;
        let _i16: int16_t = -32768;
        let _u16: uint16_t = 65535;
        let _i32: int32_t = -2147483648;
        let _u32: uint32_t = 4294967295;
        let _i64: int64_t = -9223372036854775808;
        let _u64: uint64_t = 18446744073709551615;
        let _uchar: uchar = 42;
    }

    #[test]
    fn test_int_limits() {
        assert_eq!(INT8_MAX, 127);
        assert_eq!(INT8_MIN, -128);
        assert_eq!(UINT8_MAX, 255);
        assert_eq!(INT16_MAX, 32767);
        assert_eq!(INT16_MIN, -32768);
        assert_eq!(UINT16_MAX, 65535);
        assert_eq!(INT32_MAX, 2147483647);
        assert_eq!(INT32_MIN, -2147483648);
        assert_eq!(UINT32_MAX, 4294967295);
        assert_eq!(INT64_MAX, 9223372036854775807);
        assert_eq!(INT64_MIN, -9223372036854775808);
        assert_eq!(UINT64_MAX, 18446744073709551615);
    }
}
