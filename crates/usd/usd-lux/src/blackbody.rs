//! Blackbody temperature to RGB conversion.
//!
//! Converts blackbody temperature in Kelvin to RGB color values.
//!
//! # Algorithm
//!
//! Uses Catmull-Rom spline interpolation over a lookup table from 1000K to 10000K.
//! The table is derived from "Colour Rendering of Spectra" by John Walker,
//! assuming Rec.709/sRGB colorspace chromaticity.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdLux/blackbody.h` and `blackbody.cpp`

use usd_gf::Vec3f;

// ============================================================================
// Blackbody RGB lookup table
// ============================================================================

/// Blackbody temperature to RGB lookup table.
///
/// Covers range from 1000K to 10000K in 500K steps.
/// Beginning and ending knots are repeated to simplify boundary behavior.
static BLACKBODY_RGB: [Vec3f; 22] = [
    Vec3f {
        x: 1.000000,
        y: 0.027490,
        z: 0.000000,
    }, //  1000 K (Approximation)
    Vec3f {
        x: 1.000000,
        y: 0.027490,
        z: 0.000000,
    }, //  1000 K (Approximation)
    Vec3f {
        x: 1.000000,
        y: 0.149664,
        z: 0.000000,
    }, //  1500 K (Approximation)
    Vec3f {
        x: 1.000000,
        y: 0.256644,
        z: 0.008095,
    }, //  2000 K
    Vec3f {
        x: 1.000000,
        y: 0.372033,
        z: 0.067450,
    }, //  2500 K
    Vec3f {
        x: 1.000000,
        y: 0.476725,
        z: 0.153601,
    }, //  3000 K
    Vec3f {
        x: 1.000000,
        y: 0.570376,
        z: 0.259196,
    }, //  3500 K
    Vec3f {
        x: 1.000000,
        y: 0.653480,
        z: 0.377155,
    }, //  4000 K
    Vec3f {
        x: 1.000000,
        y: 0.726878,
        z: 0.501606,
    }, //  4500 K
    Vec3f {
        x: 1.000000,
        y: 0.791543,
        z: 0.628050,
    }, //  5000 K
    Vec3f {
        x: 1.000000,
        y: 0.848462,
        z: 0.753228,
    }, //  5500 K
    Vec3f {
        x: 1.000000,
        y: 0.898581,
        z: 0.874905,
    }, //  6000 K
    Vec3f {
        x: 1.000000,
        y: 0.942771,
        z: 0.991642,
    }, //  6500 K
    Vec3f {
        x: 0.906947,
        y: 0.890456,
        z: 1.000000,
    }, //  7000 K
    Vec3f {
        x: 0.828247,
        y: 0.841838,
        z: 1.000000,
    }, //  7500 K
    Vec3f {
        x: 0.765791,
        y: 0.801896,
        z: 1.000000,
    }, //  8000 K
    Vec3f {
        x: 0.715255,
        y: 0.768579,
        z: 1.000000,
    }, //  8500 K
    Vec3f {
        x: 0.673683,
        y: 0.740423,
        z: 1.000000,
    }, //  9000 K
    Vec3f {
        x: 0.638992,
        y: 0.716359,
        z: 1.000000,
    }, //  9500 K
    Vec3f {
        x: 0.609681,
        y: 0.695588,
        z: 1.000000,
    }, // 10000 K
    Vec3f {
        x: 0.609681,
        y: 0.695588,
        z: 1.000000,
    }, // 10000 K
    Vec3f {
        x: 0.609681,
        y: 0.695588,
        z: 1.000000,
    }, // 10000 K
];

/// Catmull-Rom basis matrix coefficients.
static BASIS: [[f32; 4]; 4] = [
    [-0.5, 1.5, -1.5, 0.5],
    [1.0, -2.5, 2.0, -0.5],
    [-0.5, 0.0, 0.5, 0.0],
    [0.0, 1.0, 0.0, 0.0],
];

// ============================================================================
// Public API
// ============================================================================

/// Compute the RGB equivalent of a blackbody spectrum at the given temperature.
///
/// Uses Catmull-Rom spline interpolation over a lookup table to convert
/// blackbody temperature (in Kelvin) to RGB color values with normalized
/// luminance.
///
/// # Arguments
///
/// * `temp` - Temperature in Kelvin (valid range: 1000K to 10000K)
///
/// # Returns
///
/// RGB color vector with normalized luminance. Values are clamped to [0, ∞).
///
/// # Notes
///
/// - 6500K doesn't give pure white because D65 illuminant doesn't lie exactly
///   on the Planckian locus.
/// - Temperatures below 1000K are clamped to 1000K values.
/// - Temperatures above 10000K are clamped to 10000K values.
///
/// # Example
///
/// ```ignore
/// use usd_core::usd_lux::blackbody_temperature_as_rgb;
///
/// // Candlelight (~1850K) - warm orange
/// let candlelight = blackbody_temperature_as_rgb(1850.0);
///
/// // Daylight (~6500K) - neutral white
/// let daylight = blackbody_temperature_as_rgb(6500.0);
///
/// // Blue sky (~10000K) - bluish white
/// let blue_sky = blackbody_temperature_as_rgb(10000.0);
/// ```
#[must_use]
pub fn blackbody_temperature_as_rgb(temp: f32) -> Vec3f {
    let num_knots = BLACKBODY_RGB.len();

    // Parametric distance along spline [0, 1]
    let u_spline = ((temp - 1000.0) / 9000.0).clamp(0.0, 1.0);

    // Last 4 knots represent trailing segment starting at u_spline==1.0
    let num_segs = num_knots - 4;
    let x = u_spline * num_segs as f32;
    let seg = x.floor() as usize;
    let u_seg = x - seg as f32; // Parameter within segment

    // Knot values for this segment
    let k0 = BLACKBODY_RGB[seg];
    let k1 = BLACKBODY_RGB[seg + 1];
    let k2 = BLACKBODY_RGB[seg + 2];
    let k3 = BLACKBODY_RGB[seg + 3];

    // Compute cubic coefficients using Catmull-Rom basis
    let a = k0 * BASIS[0][0] + k1 * BASIS[0][1] + k2 * BASIS[0][2] + k3 * BASIS[0][3];
    let b = k0 * BASIS[1][0] + k1 * BASIS[1][1] + k2 * BASIS[1][2] + k3 * BASIS[1][3];
    let c = k0 * BASIS[2][0] + k1 * BASIS[2][1] + k2 * BASIS[2][2] + k3 * BASIS[2][3];
    let d = k0 * BASIS[3][0] + k1 * BASIS[3][1] + k2 * BASIS[3][2] + k3 * BASIS[3][3];

    // Evaluate cubic polynomial: ((a*u + b)*u + c)*u + d
    let mut rgb = ((a * u_seg + b) * u_seg + c) * u_seg + d;

    // Normalize to same luminance as (1, 1, 1)
    let luma = rec709_rgb_to_luma(&rgb);
    if luma > 0.0 {
        rgb /= luma;
    }

    // Clamp at zero (spline can produce small negative values)
    Vec3f::new(rgb.x.max(0.0), rgb.y.max(0.0), rgb.z.max(0.0))
}

/// Compute Rec.709 luminance from RGB.
///
/// Uses standard Rec.709/sRGB luminance coefficients.
#[inline]
fn rec709_rgb_to_luma(rgb: &Vec3f) -> f32 {
    rgb.x * 0.2126 + rgb.y * 0.7152 + rgb.z * 0.0722
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blackbody_1000k() {
        let rgb = blackbody_temperature_as_rgb(1000.0);
        // Very warm reddish
        assert!(rgb.x > rgb.y);
        assert!(rgb.y > rgb.z || rgb.z < 0.01);
    }

    #[test]
    fn test_blackbody_6500k() {
        let rgb = blackbody_temperature_as_rgb(6500.0);
        // Close to white (all components similar)
        assert!((rgb.x - 1.0).abs() < 0.5);
        assert!((rgb.y - 1.0).abs() < 0.5);
        assert!((rgb.z - 1.0).abs() < 0.5);
    }

    #[test]
    fn test_blackbody_10000k() {
        let rgb = blackbody_temperature_as_rgb(10000.0);
        // Bluish white (blue component highest or equal)
        assert!(rgb.z >= rgb.x);
    }

    #[test]
    fn test_blackbody_clamp_low() {
        // Below 1000K should clamp to 1000K value
        let rgb_500 = blackbody_temperature_as_rgb(500.0);
        let rgb_1000 = blackbody_temperature_as_rgb(1000.0);
        assert!((rgb_500.x - rgb_1000.x).abs() < 0.001);
        assert!((rgb_500.y - rgb_1000.y).abs() < 0.001);
        assert!((rgb_500.z - rgb_1000.z).abs() < 0.001);
    }

    #[test]
    fn test_blackbody_clamp_high() {
        // Above 10000K should clamp to 10000K value
        let rgb_15000 = blackbody_temperature_as_rgb(15000.0);
        let rgb_10000 = blackbody_temperature_as_rgb(10000.0);
        assert!((rgb_15000.x - rgb_10000.x).abs() < 0.001);
        assert!((rgb_15000.y - rgb_10000.y).abs() < 0.001);
        assert!((rgb_15000.z - rgb_10000.z).abs() < 0.001);
    }

    #[test]
    fn test_blackbody_non_negative() {
        // All values should be non-negative across the range
        for temp in (1000..=10000).step_by(100) {
            let rgb = blackbody_temperature_as_rgb(temp as f32);
            assert!(rgb.x >= 0.0, "R negative at {}K", temp);
            assert!(rgb.y >= 0.0, "G negative at {}K", temp);
            assert!(rgb.z >= 0.0, "B negative at {}K", temp);
        }
    }

    #[test]
    fn test_rec709_luma() {
        // White should have luma ~1
        let white = Vec3f::new(1.0, 1.0, 1.0);
        let luma = rec709_rgb_to_luma(&white);
        assert!((luma - 1.0).abs() < 0.001);
    }
}
