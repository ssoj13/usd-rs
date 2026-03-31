//! Tangent conversion utilities.
//!
//! This module provides functions for converting between different tangent
//! representations used by various animation software.
//!
//! # Overview
//!
//! Different software packages (Maya, Houdini, USD) represent tangents
//! differently. This module provides conversions between:
//!
//! - Standard format: width and slope
//! - Maya format: scaled width and height
//!
//! # Conversion Parameters
//!
//! The conversion functions use boolean flags to control the conversion:
//!
//! - `convert_height_to_slope` / `convert_slope_to_height`: Convert between
//!   height (value units) and slope (value per time)
//! - `divide_values_by_three` / `multiply_values_by_three`: Maya uses 1/3
//!   of the standard tangent width
//! - `negate_height`: Negate the height/slope value
//!
//! # Examples
//!
//! ```
//! use usd_ts::tangent_conversions::{convert_to_standard, convert_from_standard};
//!
//! // Convert Maya tangent to standard
//! let (width, slope) = convert_to_standard(
//!     1.0,    // width in
//!     3.0,    // height in
//!     true,   // convert height to slope
//!     true,   // divide by three
//!     false,  // don't negate
//! );
//!
//! // Convert back to Maya format
//! let (maya_width, maya_height) = convert_from_standard(
//!     width,
//!     slope,
//!     true,   // convert slope to height
//!     true,   // multiply by three
//!     false,  // don't negate
//! );
//! ```

use super::types::TsTime;
use usd_gf::Half;

/// Converts a tangent to standard format (width and slope).
///
/// # Parameters
///
/// - `width_in`: Input tangent width
/// - `slope_or_height_in`: Input slope or height value
/// - `convert_height_to_slope`: If true, input is height; convert to slope
/// - `divide_values_by_three`: If true, divide width (and slope if not
///   converting) by 3
/// - `negate_height`: If true, negate the output slope
///
/// # Returns
///
/// A tuple of (width_out, slope_out)
///
/// # Examples
///
/// ```
/// use usd_ts::tangent_conversions::convert_to_standard;
///
/// // Simple conversion without transformations
/// let (w, s) = convert_to_standard(1.0, 2.0, false, false, false);
/// assert!((w - 1.0).abs() < 1e-10);
/// assert!((s - 2.0).abs() < 1e-10);
///
/// // Convert height to slope: slope = height / width
/// let (w, s) = convert_to_standard(2.0, 4.0, true, false, false);
/// assert!((s - 2.0).abs() < 1e-10); // 4.0 / 2.0 = 2.0
/// ```
#[must_use]
pub fn convert_to_standard(
    width_in: TsTime,
    slope_or_height_in: f64,
    convert_height_to_slope: bool,
    divide_values_by_three: bool,
    negate_height: bool,
) -> (TsTime, f64) {
    let mut width = width_in;
    let mut slope = slope_or_height_in;

    if convert_height_to_slope || divide_values_by_three {
        let mut value = slope_or_height_in;

        if convert_height_to_slope {
            // Convert to slope before any possible division by 3
            if width.abs() > f64::EPSILON {
                value /= width;
            } else {
                // Handle zero width: use max slope
                value = if value >= 0.0 { f64::MAX } else { f64::MIN };
            }

            if divide_values_by_three {
                // Only width gets divided by 3 since value has already
                // been converted to a slope.
                width /= 3.0;
            }
        } else {
            value /= 3.0;
            width /= 3.0;
        }

        // Clamp to prevent overflow
        slope = value.clamp(-f64::MAX, f64::MAX);
    }

    if negate_height {
        slope = -slope;
    }

    (width, slope)
}

/// Converts a tangent to standard format with f32 value.
///
/// # Examples
///
/// ```
/// use usd_ts::tangent_conversions::convert_to_standard_f32;
///
/// let (w, s) = convert_to_standard_f32(1.0, 2.0f32, false, false, false);
/// ```
#[must_use]
pub fn convert_to_standard_f32(
    width_in: TsTime,
    slope_or_height_in: f32,
    convert_height_to_slope: bool,
    divide_values_by_three: bool,
    negate_height: bool,
) -> (TsTime, f32) {
    let (width, slope_f64) = convert_to_standard(
        width_in,
        f64::from(slope_or_height_in),
        convert_height_to_slope,
        divide_values_by_three,
        negate_height,
    );

    (width, slope_f64 as f32)
}

/// Converts a tangent from standard format to another representation.
///
/// # Parameters
///
/// - `width_in`: Input tangent width
/// - `slope_in`: Input slope value
/// - `convert_slope_to_height`: If true, convert slope to height
/// - `multiply_values_by_three`: If true, multiply width (and height if
///   converting) by 3
/// - `negate_height`: If true, negate the output value
///
/// # Returns
///
/// A tuple of (width_out, slope_or_height_out)
///
/// # Examples
///
/// ```
/// use usd_ts::tangent_conversions::convert_from_standard;
///
/// // Simple conversion without transformations
/// let (w, s) = convert_from_standard(1.0, 2.0, false, false, false);
/// assert!((w - 1.0).abs() < 1e-10);
/// assert!((s - 2.0).abs() < 1e-10);
///
/// // Convert slope to height: height = slope * width
/// let (w, h) = convert_from_standard(2.0, 3.0, true, false, false);
/// assert!((h - 6.0).abs() < 1e-10); // 3.0 * 2.0 = 6.0
/// ```
#[must_use]
pub fn convert_from_standard(
    width_in: TsTime,
    slope_in: f64,
    convert_slope_to_height: bool,
    multiply_values_by_three: bool,
    negate_height: bool,
) -> (TsTime, f64) {
    let mut width = width_in;
    let mut slope_or_height = slope_in;

    if convert_slope_to_height || multiply_values_by_three {
        let mut soh = slope_or_height;

        if convert_slope_to_height {
            if multiply_values_by_three {
                width *= 3.0;
            }
            soh *= width;
        } else {
            // Just multiply by 3
            soh *= 3.0;
            width *= 3.0;
        }

        // Clamp to prevent overflow
        slope_or_height = soh.clamp(-f64::MAX, f64::MAX);
    }

    if negate_height {
        slope_or_height = -slope_or_height;
    }

    (width, slope_or_height)
}

/// Converts a tangent from standard format with f32 value.
///
/// # Examples
///
/// ```
/// use usd_ts::tangent_conversions::convert_from_standard_f32;
///
/// let (w, h) = convert_from_standard_f32(1.0, 2.0f32, false, false, false);
/// ```
#[must_use]
pub fn convert_from_standard_f32(
    width_in: TsTime,
    slope_in: f32,
    convert_slope_to_height: bool,
    multiply_values_by_three: bool,
    negate_height: bool,
) -> (TsTime, f32) {
    let (width, soh_f64) = convert_from_standard(
        width_in,
        f64::from(slope_in),
        convert_slope_to_height,
        multiply_values_by_three,
        negate_height,
    );

    (width, soh_f64 as f32)
}

/// Converts a tangent to standard format with Half (f16) value.
///
/// Matches C++ `TsConvertToStandardTangent<GfHalf>`.
#[must_use]
pub fn convert_to_standard_half(
    width_in: TsTime,
    slope_or_height_in: Half,
    convert_height_to_slope: bool,
    divide_values_by_three: bool,
    negate_height: bool,
) -> (TsTime, Half) {
    let (width, slope_f64) = convert_to_standard(
        width_in,
        slope_or_height_in.to_f64(),
        convert_height_to_slope,
        divide_values_by_three,
        negate_height,
    );

    (width, Half::from_f64(slope_f64))
}

/// Converts a tangent from standard format with Half (f16) value.
///
/// Matches C++ `TsConvertFromStandardTangent<GfHalf>`.
#[must_use]
pub fn convert_from_standard_half(
    width_in: TsTime,
    slope_in: Half,
    convert_slope_to_height: bool,
    multiply_values_by_three: bool,
    negate_height: bool,
) -> (TsTime, Half) {
    let (width, soh_f64) = convert_from_standard(
        width_in,
        slope_in.to_f64(),
        convert_slope_to_height,
        multiply_values_by_three,
        negate_height,
    );

    (width, Half::from_f64(soh_f64))
}

/// Converts Maya tangent format to USD standard format.
///
/// Maya tangents use:
/// - Width scaled by 1/3
/// - Height instead of slope
///
/// # Parameters
///
/// - `maya_width`: Maya tangent width (1/3 of standard)
/// - `maya_height`: Maya tangent height
///
/// # Returns
///
/// A tuple of (standard_width, standard_slope)
///
/// # Examples
///
/// ```
/// use usd_ts::tangent_conversions::maya_to_standard;
///
/// let (width, slope) = maya_to_standard(1.0, 3.0);
/// // width = 1.0 (unchanged for height conversion)
/// // slope = 3.0 / 1.0 = 3.0
/// ```
#[inline]
#[must_use]
pub fn maya_to_standard(maya_width: TsTime, maya_height: f64) -> (TsTime, f64) {
    convert_to_standard(maya_width, maya_height, true, true, false)
}

/// Converts USD standard format to Maya tangent format.
///
/// # Parameters
///
/// - `width`: Standard tangent width
/// - `slope`: Standard tangent slope
///
/// # Returns
///
/// A tuple of (maya_width, maya_height)
///
/// # Examples
///
/// ```
/// use usd_ts::tangent_conversions::standard_to_maya;
///
/// let (maya_width, maya_height) = standard_to_maya(3.0, 2.0);
/// // maya_width = 3.0 * 3 = 9.0
/// // maya_height = 2.0 * 9.0 = 18.0
/// ```
#[inline]
#[must_use]
pub fn standard_to_maya(width: TsTime, slope: f64) -> (TsTime, f64) {
    convert_from_standard(width, slope, true, true, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_to_standard_identity() {
        let (w, s) = convert_to_standard(1.0, 2.0, false, false, false);
        assert!((w - 1.0).abs() < 1e-10);
        assert!((s - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_convert_to_standard_height_to_slope() {
        // slope = height / width = 4.0 / 2.0 = 2.0
        let (w, s) = convert_to_standard(2.0, 4.0, true, false, false);
        assert!((w - 2.0).abs() < 1e-10);
        assert!((s - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_convert_to_standard_divide_by_three() {
        // Without height conversion: both divided by 3
        let (w, s) = convert_to_standard(3.0, 9.0, false, true, false);
        assert!((w - 1.0).abs() < 1e-10);
        assert!((s - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_convert_to_standard_height_and_divide() {
        // height = 6.0, width = 3.0
        // slope = 6.0 / 3.0 = 2.0
        // width /= 3 => 1.0
        let (w, s) = convert_to_standard(3.0, 6.0, true, true, false);
        assert!((w - 1.0).abs() < 1e-10);
        assert!((s - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_convert_to_standard_negate() {
        let (w, s) = convert_to_standard(1.0, 2.0, false, false, true);
        assert!((w - 1.0).abs() < 1e-10);
        assert!((s - (-2.0)).abs() < 1e-10);
    }

    #[test]
    fn test_convert_from_standard_identity() {
        let (w, s) = convert_from_standard(1.0, 2.0, false, false, false);
        assert!((w - 1.0).abs() < 1e-10);
        assert!((s - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_convert_from_standard_slope_to_height() {
        // height = slope * width = 3.0 * 2.0 = 6.0
        let (w, h) = convert_from_standard(2.0, 3.0, true, false, false);
        assert!((w - 2.0).abs() < 1e-10);
        assert!((h - 6.0).abs() < 1e-10);
    }

    #[test]
    fn test_convert_from_standard_multiply_by_three() {
        // Without height conversion: both multiplied by 3
        let (w, s) = convert_from_standard(1.0, 2.0, false, true, false);
        assert!((w - 3.0).abs() < 1e-10);
        assert!((s - 6.0).abs() < 1e-10);
    }

    #[test]
    fn test_convert_from_standard_height_and_multiply() {
        // width *= 3 => 3.0
        // height = slope * width = 2.0 * 3.0 = 6.0
        let (w, h) = convert_from_standard(1.0, 2.0, true, true, false);
        assert!((w - 3.0).abs() < 1e-10);
        assert!((h - 6.0).abs() < 1e-10);
    }

    #[test]
    fn test_convert_from_standard_negate() {
        let (w, s) = convert_from_standard(1.0, 2.0, false, false, true);
        assert!((w - 1.0).abs() < 1e-10);
        assert!((s - (-2.0)).abs() < 1e-10);
    }

    #[test]
    fn test_roundtrip() {
        let original_width = 2.0;
        let original_slope = 3.0;

        // Convert to Maya format
        let (maya_w, maya_h) = convert_from_standard(
            original_width,
            original_slope,
            true,  // slope to height
            true,  // multiply by 3
            false, // no negate
        );

        // Convert back to standard
        let (width, slope) = convert_to_standard(
            maya_w, maya_h, true,  // height to slope
            true,  // divide by 3
            false, // no negate
        );

        assert!((width - original_width).abs() < 1e-10);
        assert!((slope - original_slope).abs() < 1e-10);
    }

    #[test]
    fn test_maya_to_standard() {
        let (w, s) = maya_to_standard(1.0, 3.0);
        // width stays 1.0 (divided by 3 after height conversion, but we started with scaled)
        // Actually: with convert=true, divide=true:
        //   slope = height / width = 3.0 / 1.0 = 3.0
        //   width /= 3 => 0.333...
        assert!((w - 1.0 / 3.0).abs() < 1e-10);
        assert!((s - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_standard_to_maya() {
        let (w, h) = standard_to_maya(1.0, 2.0);
        // width *= 3 => 3.0
        // height = slope * width = 2.0 * 3.0 = 6.0
        assert!((w - 3.0).abs() < 1e-10);
        assert!((h - 6.0).abs() < 1e-10);
    }

    #[test]
    fn test_maya_roundtrip() {
        let width = 0.5;
        let slope = 4.0;

        let (maya_w, maya_h) = standard_to_maya(width, slope);
        let (back_w, back_s) = maya_to_standard(maya_w, maya_h);

        assert!((back_w - width).abs() < 1e-10);
        assert!((back_s - slope).abs() < 1e-10);
    }

    #[test]
    fn test_zero_width_height_to_slope() {
        // With zero width, slope should be max
        let (w, s) = convert_to_standard(0.0, 5.0, true, false, false);
        assert!(w.abs() < 1e-10);
        assert!(s > 1e100); // Very large positive

        let (_, s_neg) = convert_to_standard(0.0, -5.0, true, false, false);
        assert!(s_neg < -1e100); // Very large negative
    }

    #[test]
    fn test_f32_conversion() {
        let (w, s) = convert_to_standard_f32(1.0, 2.0f32, false, false, false);
        assert!((w - 1.0).abs() < 1e-10);
        assert!((s - 2.0f32).abs() < 1e-5);

        let (w2, h) = convert_from_standard_f32(1.0, 2.0f32, true, false, false);
        assert!((w2 - 1.0).abs() < 1e-10);
        assert!((h - 2.0f32).abs() < 1e-5);
    }
}
