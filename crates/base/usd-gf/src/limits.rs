//! Useful mathematical limits for gf.
//!
//! This module defines constants used for numerical stability checks
//! in vector, matrix, and other geometric operations.
//!
//! # Examples
//!
//! ```
//! use usd_gf::limits::*;
//!
//! // Check if a vector length is too small
//! let length = 1e-12;
//! if length < MIN_VECTOR_LENGTH {
//!     println!("Vector is too small to normalize");
//! }
//! ```

/// Minimum vector length for safe operations.
///
/// This constant is used to determine whether the length of a vector
/// is too small to handle accurately. Operations like normalization
/// should check against this value to avoid division by near-zero.
///
/// # Examples
///
/// ```
/// use usd_gf::limits::MIN_VECTOR_LENGTH;
///
/// fn safe_normalize(length: f64) -> Option<f64> {
///     if length < MIN_VECTOR_LENGTH {
///         None
///     } else {
///         Some(1.0 / length)
///     }
/// }
///
/// assert!(safe_normalize(1e-12).is_none());
/// assert!(safe_normalize(1.0).is_some());
/// ```
pub const MIN_VECTOR_LENGTH: f64 = 1e-10;

/// Minimum tolerance for orthogonality checks.
///
/// This constant is used to determine when a set of basis vectors
/// is close enough to orthogonal. Values below this tolerance are
/// considered effectively zero in dot product checks.
///
/// # Examples
///
/// ```
/// use usd_gf::limits::MIN_ORTHO_TOLERANCE;
///
/// fn is_orthogonal(dot_product: f64) -> bool {
///     dot_product.abs() < MIN_ORTHO_TOLERANCE
/// }
///
/// assert!(is_orthogonal(1e-8));
/// assert!(!is_orthogonal(0.1));
/// ```
pub const MIN_ORTHO_TOLERANCE: f64 = 1e-6;

/// Default epsilon for floating-point comparisons.
///
/// A general-purpose tolerance for comparing floating-point values.
pub const DEFAULT_EPSILON: f64 = 1e-6;

/// Very small epsilon for high-precision comparisons.
pub const SMALL_EPSILON: f64 = 1e-12;

/// Maximum safe value for f64 before overflow concerns.
///
/// Approximately 1e+300, leaving room for squaring operations.
pub const MAX_SAFE_VALUE: f64 = 1e150;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_min_vector_length() {
        assert!(MIN_VECTOR_LENGTH > 0.0);
        assert!(MIN_VECTOR_LENGTH < 1e-6);
    }

    #[test]
    fn test_min_ortho_tolerance() {
        assert!(MIN_ORTHO_TOLERANCE > 0.0);
        assert!(MIN_ORTHO_TOLERANCE > MIN_VECTOR_LENGTH);
    }

    #[test]
    fn test_default_epsilon() {
        assert!(DEFAULT_EPSILON > 0.0);
        assert!(DEFAULT_EPSILON < 1.0);
    }

    #[test]
    fn test_small_epsilon() {
        assert!(SMALL_EPSILON > 0.0);
        assert!(SMALL_EPSILON < DEFAULT_EPSILON);
    }
}
