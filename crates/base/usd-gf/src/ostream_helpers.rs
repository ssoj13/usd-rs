//! Ostream helpers for Gf types.
//!
//! Helpers for Gf stream operators.
//!
//! These functions are useful to help with writing stream operators for
//! Gf types. Please do not include this file in any header.
//!
//! Matches C++ `pxr/base/gf/ostreamHelpers.h`.

use usd_tf::string_utils::{StreamDouble, StreamFloat};

/// Helper function for ostream operators.
///
/// Make the template function general so that we can use the same function
/// on all variables and not worry about making a mistake.
///
/// Matches C++ `Gf_OstreamHelperP<T>` template function.
///
/// # Examples
///
/// ```
/// use usd_gf::ostream_helpers::ostream_helper_p;
///
/// let val = 42i32;
/// let result = ostream_helper_p(val);
/// assert_eq!(result, 42);
/// ```
#[inline]
pub fn ostream_helper_p<T>(v: T) -> T {
    v
}

/// Helper function for ostream operators - float specialization.
///
/// Returns a `StreamFloat` wrapper for proper float formatting.
///
/// Matches C++ `Gf_OstreamHelperP(float)` specialization.
///
/// # Examples
///
/// ```
/// use usd_gf::ostream_helpers::ostream_helper_p;
/// use std::fmt;
///
/// let val = 3.14f32;
/// let stream_float = ostream_helper_p(val);
/// // stream_float can be used with Display trait for proper formatting
/// ```
#[inline]
pub fn ostream_helper_p_float(v: f32) -> StreamFloat {
    StreamFloat::new(v)
}

/// Helper function for ostream operators - double specialization.
///
/// Returns a `StreamDouble` wrapper for proper double formatting.
///
/// Matches C++ `Gf_OstreamHelperP(double)` specialization.
///
/// # Examples
///
/// ```
/// use usd_gf::ostream_helpers::ostream_helper_p;
/// use std::fmt;
///
/// let val = 3.14f64;
/// let stream_double = ostream_helper_p(val);
/// // stream_double can be used with Display trait for proper formatting
/// ```
#[inline]
pub fn ostream_helper_p_double(v: f64) -> StreamDouble {
    StreamDouble::new(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ostream_helper_p_generic() {
        assert_eq!(ostream_helper_p(42i32), 42);
        assert_eq!(ostream_helper_p(100u64), 100);
        assert_eq!(ostream_helper_p("hello"), "hello");
    }

    #[test]
    fn test_ostream_helper_p_float() {
        let val = 3.14f32;
        let stream_float = ostream_helper_p_float(val);
        assert_eq!(stream_float.value(), val);
    }

    #[test]
    fn test_ostream_helper_p_double() {
        let val = 3.14159f64;
        let stream_double = ostream_helper_p_double(val);
        assert_eq!(stream_double.value(), val);
    }
}
