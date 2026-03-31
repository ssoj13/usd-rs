//! Color representation with color space support.
//!
//! This module provides [`Color`] for representing colors in specific color spaces,
//! with support for color space conversion and blackbody (Planckian locus) colors.
//!
//! # Examples
//!
//! ```
//! use usd_gf::{Color, ColorSpace, ColorSpaceName, Vec3f};
//!
//! // Create a red color in sRGB
//! let srgb = ColorSpace::new(ColorSpaceName::SRGBRec709);
//! let red = Color::new(Vec3f::new(1.0, 0.0, 0.0), srgb);
//!
//! // Convert to linear
//! let linear = ColorSpace::new(ColorSpaceName::LinearRec709);
//! let red_linear = Color::convert(&red, linear);
//! ```

use std::fmt;

use crate::ostream_helpers::ostream_helper_p_float;
use crate::{ColorSpace, Vec2f, Vec3f};

#[cfg(test)]
use crate::ColorSpaceName;

/// A color with an associated color space.
///
/// Color values are stored as RGB tuples and are colorimetric (not photometric).
/// The color space determines how the RGB values are interpreted.
///
/// # Examples
///
/// ```
/// use usd_gf::{Color, ColorSpace, ColorSpaceName, Vec3f};
///
/// let cs = ColorSpace::new(ColorSpaceName::LinearRec709);
/// let white = Color::new(Vec3f::new(1.0, 1.0, 1.0), cs);
///
/// assert_eq!(white.rgb(), Vec3f::new(1.0, 1.0, 1.0));
/// ```
#[derive(Clone, Debug)]
pub struct Color {
    rgb: Vec3f,
    color_space: ColorSpace,
}

impl Color {
    // ========================================================================
    // Constructors
    // ========================================================================

    /// Create a black color in the default (LinearRec709) color space.
    #[must_use]
    pub fn black() -> Self {
        Self {
            rgb: Vec3f::zero(),
            color_space: ColorSpace::default(),
        }
    }

    /// Create a white color (1, 1, 1) in the given color space.
    #[must_use]
    pub fn white() -> Self {
        Self::white_in(ColorSpace::default())
    }

    /// Create a white color in the given color space.
    #[must_use]
    pub fn white_in(color_space: ColorSpace) -> Self {
        Self {
            rgb: Vec3f::new(1.0, 1.0, 1.0),
            color_space,
        }
    }

    /// Create a red color (1, 0, 0).
    #[must_use]
    pub fn red() -> Self {
        Self::red_in(ColorSpace::default())
    }

    /// Create a red color in the given color space.
    #[must_use]
    pub fn red_in(color_space: ColorSpace) -> Self {
        Self {
            rgb: Vec3f::new(1.0, 0.0, 0.0),
            color_space,
        }
    }

    /// Create a green color (0, 1, 0).
    #[must_use]
    pub fn green() -> Self {
        Self::green_in(ColorSpace::default())
    }

    /// Create a green color in the given color space.
    #[must_use]
    pub fn green_in(color_space: ColorSpace) -> Self {
        Self {
            rgb: Vec3f::new(0.0, 1.0, 0.0),
            color_space,
        }
    }

    /// Create a blue color (0, 0, 1).
    #[must_use]
    pub fn blue() -> Self {
        Self::blue_in(ColorSpace::default())
    }

    /// Create a blue color in the given color space.
    #[must_use]
    pub fn blue_in(color_space: ColorSpace) -> Self {
        Self {
            rgb: Vec3f::new(0.0, 0.0, 1.0),
            color_space,
        }
    }

    /// Create a yellow color (1, 1, 0).
    #[must_use]
    pub fn yellow() -> Self {
        Self::yellow_in(ColorSpace::default())
    }

    /// Create a yellow color in the given color space.
    #[must_use]
    pub fn yellow_in(color_space: ColorSpace) -> Self {
        Self {
            rgb: Vec3f::new(1.0, 1.0, 0.0),
            color_space,
        }
    }

    /// Create a cyan color (0, 1, 1).
    #[must_use]
    pub fn cyan() -> Self {
        Self::cyan_in(ColorSpace::default())
    }

    /// Create a cyan color in the given color space.
    #[must_use]
    pub fn cyan_in(color_space: ColorSpace) -> Self {
        Self {
            rgb: Vec3f::new(0.0, 1.0, 1.0),
            color_space,
        }
    }

    /// Create a magenta color (1, 0, 1).
    #[must_use]
    pub fn magenta() -> Self {
        Self::magenta_in(ColorSpace::default())
    }

    /// Create a magenta color in the given color space.
    #[must_use]
    pub fn magenta_in(color_space: ColorSpace) -> Self {
        Self {
            rgb: Vec3f::new(1.0, 0.0, 1.0),
            color_space,
        }
    }

    /// Create a black color in the given color space.
    #[must_use]
    pub fn black_in(color_space: ColorSpace) -> Self {
        Self {
            rgb: Vec3f::zero(),
            color_space,
        }
    }

    /// Create a color from RGB values in a color space.
    #[must_use]
    pub fn new(rgb: Vec3f, color_space: ColorSpace) -> Self {
        Self { rgb, color_space }
    }

    /// Create a color from individual RGB components.
    #[must_use]
    pub fn from_rgb(r: f32, g: f32, b: f32, color_space: ColorSpace) -> Self {
        Self {
            rgb: Vec3f::new(r, g, b),
            color_space,
        }
    }

    /// Create a color from HSV (Hue, Saturation, Value) in the given color space.
    ///
    /// # Arguments
    ///
    /// * `h` - Hue in degrees [0, 360)
    /// * `s` - Saturation [0, 1]
    /// * `v` - Value [0, 1]
    /// * `color_space` - Target color space
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::{Color, ColorSpace, ColorSpaceName};
    ///
    /// let cs = ColorSpace::new(ColorSpaceName::LinearRec709);
    /// let red = Color::from_hsv(0.0, 1.0, 1.0, cs); // Pure red
    /// ```
    #[must_use]
    pub fn from_hsv(h: f32, s: f32, v: f32, color_space: ColorSpace) -> Self {
        let rgb = hsv_to_rgb(h, s, v);
        Self { rgb, color_space }
    }

    /// Create a color from a hex string (e.g., "#FF0000" or "FF0000").
    ///
    /// Returns None if the string is not a valid hex color.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::{Color, ColorSpace, ColorSpaceName};
    ///
    /// let cs = ColorSpace::new(ColorSpaceName::LinearRec709);
    /// let red = Color::from_hex("#FF0000", cs.clone()).unwrap();
    /// let green = Color::from_hex("00FF00", cs).unwrap();
    /// ```
    pub fn from_hex(hex: &str, color_space: ColorSpace) -> Option<Self> {
        let hex = hex.trim_start_matches('#');

        if hex.len() != 6 {
            return None;
        }

        let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f32 / 255.0;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f32 / 255.0;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f32 / 255.0;

        Some(Self {
            rgb: Vec3f::new(r, g, b),
            color_space,
        })
    }

    /// Convert a color to a different color space.
    #[must_use]
    pub fn convert(src: &Color, dst_space: ColorSpace) -> Self {
        let converted_rgb = dst_space.convert(&src.color_space, &src.rgb);
        Self {
            rgb: converted_rgb,
            color_space: dst_space,
        }
    }

    /// Set the color from blackbody (Planckian locus) temperature in Kelvin.
    ///
    /// Values are computed for temperatures between 1000K and 15000K.
    /// Temperatures below ~1900K are out of gamut for Rec.709.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::{Color, ColorSpace, ColorSpaceName};
    ///
    /// let cs = ColorSpace::new(ColorSpaceName::LinearRec709);
    /// let mut color = Color::black_in(cs);
    /// color.set_from_planckian_locus(6500.0, 1.0); // D65 white point
    /// ```
    pub fn set_from_planckian_locus(&mut self, kelvin: f32, luminance: f32) {
        let yxy = kelvin_to_yxy(kelvin, luminance);
        let xyz = yxy_to_xyz(&yxy);
        let rgb = self.color_space.xyz_to_rgb_with_transfer(&xyz);
        let maxc = rgb.x.abs().max(rgb.y.abs()).max(rgb.z.abs());
        if maxc > 1e-10 {
            let s = |x: f32| {
                if x > 0.0 {
                    1.0
                } else if x < 0.0 {
                    -1.0
                } else {
                    0.0
                }
            };
            self.rgb = Vec3f::new(
                s(rgb.x) * rgb.x / maxc,
                s(rgb.y) * rgb.y / maxc,
                s(rgb.z) * rgb.z / maxc,
            );
        } else {
            self.rgb = rgb;
        }
    }

    /// Create a color from blackbody temperature.
    #[must_use]
    pub fn from_planckian_locus(kelvin: f32, luminance: f32, color_space: ColorSpace) -> Self {
        let mut color = Self::black_in(color_space);
        color.set_from_planckian_locus(kelvin, luminance);
        color
    }

    /// Get the RGB tuple.
    #[must_use]
    pub fn rgb(&self) -> Vec3f {
        self.rgb
    }

    /// Get a reference to the RGB tuple.
    #[must_use]
    pub fn rgb_ref(&self) -> &Vec3f {
        &self.rgb
    }

    /// Get the color space.
    #[must_use]
    pub fn color_space(&self) -> &ColorSpace {
        &self.color_space
    }

    /// Get the CIE xy chromaticity coordinates.
    ///
    /// Returns (x, y) chromaticity values in the CIE 1931 chromaticity diagram.
    /// Per OpenUSD: applies ToLinear before RGB->XYZ for correct chromaticity.
    #[must_use]
    pub fn chromaticity(&self) -> Vec2f {
        let xyz = self.color_space.rgb_to_xyz_linearized(&self.rgb);
        xyz_to_xy(&xyz)
    }

    /// Set the color from CIE xy chromaticity coordinates.
    ///
    /// The luminance (Y) is set to 1.0.
    pub fn set_from_chromaticity(&mut self, xy: &Vec2f) {
        let yxy = Vec3f::new(1.0, xy.x, xy.y);
        self.rgb = yxy_to_rgb(&self.color_space, &yxy);
    }

    // ========================================================================
    // Color operations
    // ========================================================================

    /// Convert RGB to HSV.
    ///
    /// Returns (hue [0, 360), saturation [0, 1], value [0, 1]).
    #[must_use]
    pub fn to_hsv(&self) -> (f32, f32, f32) {
        rgb_to_hsv(&self.rgb)
    }

    /// Convert to hex string (e.g., "#FF0000").
    #[must_use]
    pub fn to_hex(&self) -> String {
        let r = (self.rgb.x.clamp(0.0, 1.0) * 255.0) as u8;
        let g = (self.rgb.y.clamp(0.0, 1.0) * 255.0) as u8;
        let b = (self.rgb.z.clamp(0.0, 1.0) * 255.0) as u8;
        format!("#{:02X}{:02X}{:02X}", r, g, b)
    }

    /// Linear interpolation between two colors.
    ///
    /// Colors must be in the same color space. The result will be in that space.
    ///
    /// # Arguments
    ///
    /// * `other` - Target color
    /// * `t` - Interpolation parameter [0, 1]
    #[must_use]
    pub fn lerp(&self, other: &Color, t: f32) -> Self {
        Self {
            rgb: self.rgb * (1.0 - t) + other.rgb * t,
            color_space: self.color_space.clone(),
        }
    }

    /// Clamp RGB components to [0, 1] range.
    #[must_use]
    pub fn clamped(&self) -> Self {
        Self {
            rgb: Vec3f::new(
                self.rgb.x.clamp(0.0, 1.0),
                self.rgb.y.clamp(0.0, 1.0),
                self.rgb.z.clamp(0.0, 1.0),
            ),
            color_space: self.color_space.clone(),
        }
    }

    /// Clamp RGB components to [0, 1] range in place.
    pub fn clamp(&mut self) {
        self.rgb.x = self.rgb.x.clamp(0.0, 1.0);
        self.rgb.y = self.rgb.y.clamp(0.0, 1.0);
        self.rgb.z = self.rgb.z.clamp(0.0, 1.0);
    }

    /// Component-wise multiply with another color.
    #[must_use]
    pub fn multiply(&self, other: &Color) -> Self {
        Self {
            rgb: self.rgb.comp_mult(&other.rgb),
            color_space: self.color_space.clone(),
        }
    }

    /// Component-wise add with another color.
    #[must_use]
    pub fn add(&self, other: &Color) -> Self {
        Self {
            rgb: self.rgb + other.rgb,
            color_space: self.color_space.clone(),
        }
    }

    /// Scale by a scalar value.
    #[must_use]
    pub fn scale(&self, s: f32) -> Self {
        Self {
            rgb: self.rgb * s,
            color_space: self.color_space.clone(),
        }
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::black()
    }
}

impl PartialEq for Color {
    fn eq(&self, other: &Self) -> bool {
        self.rgb == other.rgb && self.color_space == other.color_space
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "({}, {}, {}, {})",
            ostream_helper_p_float(self.rgb.x),
            ostream_helper_p_float(self.rgb.y),
            ostream_helper_p_float(self.rgb.z),
            self.color_space.name()
        )
    }
}

/// Check if two colors are approximately equal.
#[must_use]
pub fn is_close(c1: &Color, c2: &Color, tolerance: f64) -> bool {
    let diff = c1.rgb - c2.rgb;
    let len_sq = (diff.x * diff.x + diff.y * diff.y + diff.z * diff.z) as f64;
    len_sq.sqrt() <= tolerance
}

// ============================================================================
// Color conversion utilities
// ============================================================================

/// Convert Kelvin temperature to Yxy (luminance and chromaticity).
///
/// Krystek (1985) u'v' rational approximation, matches OpenUSD NcKelvinToYxy.
fn kelvin_to_yxy(kelvin: f32, luminance: f32) -> Vec3f {
    let t = if kelvin < 1000.0 {
        1000.0
    } else if kelvin > 15000.0 {
        15000.0
    } else {
        kelvin
    };
    if t < 1000.0 || t > 15000.0 {
        return Vec3f::zero();
    }
    let t = t as f64;
    let u = (0.860117757 + 1.54118254e-4 * t + 1.2864121e-7 * t * t)
        / (1.0 + 8.42420235e-4 * t + 7.08145163e-7 * t * t);
    let v = (0.317398726 + 4.22806245e-5 * t + 4.20481691e-8 * t * t)
        / (1.0 - 2.89741816e-5 * t + 1.61456053e-7 * t * t);
    let v_prime = 1.5 * v;
    let d = 6.0 * u - 16.0 * v_prime + 12.0;
    let x = (9.0 * u / d) as f32;
    let y = (4.0 * v_prime / d) as f32;
    Vec3f::new(luminance, x, y)
}

/// Convert Yxy (luminance + chromaticity) to XYZ.
fn yxy_to_xyz(yxy: &Vec3f) -> Vec3f {
    let y_lum = yxy.x;
    let x = yxy.y;
    let y = yxy.z;

    if y.abs() < 1e-10 {
        return Vec3f::zero();
    }

    let xyz_x = (x * y_lum) / y;
    let xyz_y = y_lum;
    let xyz_z = ((1.0 - x - y) * y_lum) / y;

    Vec3f::new(xyz_x, xyz_y, xyz_z)
}

/// Convert XYZ to RGB in a color space.
/// Returns `xyz` unchanged if the color space matrix is singular.
fn xyz_to_rgb(color_space: &ColorSpace, xyz: &Vec3f) -> Vec3f {
    let rgb_to_xyz = color_space.rgb_to_xyz();
    match rgb_to_xyz.inverse() {
        Some(m) => m * *xyz,
        None => *xyz, // singular matrix — return input unchanged
    }
}

/// Convert Yxy to RGB in a color space.
fn yxy_to_rgb(color_space: &ColorSpace, yxy: &Vec3f) -> Vec3f {
    let xyz = yxy_to_xyz(yxy);
    xyz_to_rgb(color_space, &xyz)
}

/// Convert RGB to XYZ in a color space.
/// Reserved for future inverse conversion paths.
#[allow(dead_code)]
fn rgb_to_xyz(color_space: &ColorSpace, rgb: &Vec3f) -> Vec3f {
    let rgb_to_xyz = color_space.rgb_to_xyz();
    *rgb_to_xyz * *rgb
}

/// Convert XYZ to xy chromaticity.
///
/// Per OpenUSD NcXYZToYxy: when sum is 0, returns (0, 0) with y = xyz.y preserved.
fn xyz_to_xy(xyz: &Vec3f) -> Vec2f {
    let sum = xyz.x + xyz.y + xyz.z;
    if sum.abs() < 1e-10 {
        return Vec2f::new(0.0, xyz.y);
    }
    Vec2f::new(xyz.x / sum, xyz.y / sum)
}

/// Convert HSV to RGB.
///
/// # Arguments
///
/// * `h` - Hue in degrees [0, 360)
/// * `s` - Saturation [0, 1]
/// * `v` - Value [0, 1]
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> Vec3f {
    let s = s.clamp(0.0, 1.0);
    let v = v.clamp(0.0, 1.0);
    let h = h.rem_euclid(360.0);

    if s == 0.0 {
        // Grayscale
        return Vec3f::new(v, v, v);
    }

    let h_sector = h / 60.0;
    let sector = h_sector.floor() as i32;
    let f = h_sector - sector as f32;

    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));

    match sector {
        0 => Vec3f::new(v, t, p),
        1 => Vec3f::new(q, v, p),
        2 => Vec3f::new(p, v, t),
        3 => Vec3f::new(p, q, v),
        4 => Vec3f::new(t, p, v),
        _ => Vec3f::new(v, p, q), // 5 or wrapped
    }
}

/// Convert RGB to HSV.
///
/// Returns (hue [0, 360), saturation [0, 1], value [0, 1]).
fn rgb_to_hsv(rgb: &Vec3f) -> (f32, f32, f32) {
    let r = rgb.x;
    let g = rgb.y;
    let b = rgb.z;

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    // Value
    let v = max;

    // Saturation
    let s = if max == 0.0 { 0.0 } else { delta / max };

    // Hue
    let h = if delta == 0.0 {
        0.0 // Undefined, use 0
    } else if (max - r).abs() < f32::EPSILON {
        60.0 * (((g - b) / delta).rem_euclid(6.0))
    } else if (max - g).abs() < f32::EPSILON {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };

    (h, s, v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_default() {
        let c = Color::default();
        assert_eq!(c.rgb(), Vec3f::zero());
    }

    #[test]
    fn test_color_black() {
        let c = Color::black();
        assert_eq!(c.rgb(), Vec3f::zero());
        assert_eq!(c.color_space().name().as_str(), "lin_rec709_scene");
    }

    #[test]
    fn test_color_new() {
        let cs = ColorSpace::new(ColorSpaceName::LinearRec709);
        let c = Color::new(Vec3f::new(1.0, 0.5, 0.25), cs);
        assert_eq!(c.rgb(), Vec3f::new(1.0, 0.5, 0.25));
    }

    #[test]
    fn test_color_equality() {
        let cs = ColorSpace::new(ColorSpaceName::LinearRec709);
        let c1 = Color::new(Vec3f::new(1.0, 0.0, 0.0), cs.clone());
        let c2 = Color::new(Vec3f::new(1.0, 0.0, 0.0), cs);
        assert_eq!(c1, c2);
    }

    #[test]
    fn test_color_inequality_rgb() {
        let cs = ColorSpace::new(ColorSpaceName::LinearRec709);
        let c1 = Color::new(Vec3f::new(1.0, 0.0, 0.0), cs.clone());
        let c2 = Color::new(Vec3f::new(0.0, 1.0, 0.0), cs);
        assert_ne!(c1, c2);
    }

    #[test]
    fn test_color_inequality_space() {
        let cs1 = ColorSpace::new(ColorSpaceName::LinearRec709);
        let cs2 = ColorSpace::new(ColorSpaceName::LinearAP1);
        let c1 = Color::new(Vec3f::new(1.0, 0.0, 0.0), cs1);
        let c2 = Color::new(Vec3f::new(1.0, 0.0, 0.0), cs2);
        assert_ne!(c1, c2);
    }

    #[test]
    fn test_color_display() {
        let cs = ColorSpace::new(ColorSpaceName::LinearRec709);
        let c = Color::new(Vec3f::new(1.0, 0.5, 0.0), cs);
        let s = format!("{}", c);
        assert!(s.contains("1"));
        assert!(s.contains("0.5"));
        assert!(s.contains("lin_rec709"));
    }

    #[test]
    fn test_is_close() {
        let cs = ColorSpace::new(ColorSpaceName::LinearRec709);
        let c1 = Color::new(Vec3f::new(1.0, 0.0, 0.0), cs.clone());
        let c2 = Color::new(Vec3f::new(1.001, 0.0, 0.0), cs);
        assert!(is_close(&c1, &c2, 0.01));
        assert!(!is_close(&c1, &c2, 0.0001));
    }

    #[test]
    fn test_kelvin_to_yxy_d65() {
        // D65 is approximately 6500K
        let yxy = kelvin_to_yxy(6500.0, 1.0);
        // D65 chromaticity is approximately (0.3127, 0.3290)
        assert!((yxy.y - 0.3127).abs() < 0.01);
        assert!((yxy.z - 0.3290).abs() < 0.01);
    }

    #[test]
    fn test_kelvin_to_yxy_incandescent() {
        // Incandescent is approximately 2700K
        let yxy = kelvin_to_yxy(2700.0, 1.0);
        // Should be warm (x > 0.4)
        assert!(yxy.y > 0.4);
    }

    #[test]
    fn test_planckian_locus() {
        let cs = ColorSpace::new(ColorSpaceName::LinearRec709);
        let color = Color::from_planckian_locus(6500.0, 1.0, cs);

        // D65 should be close to white
        let rgb = color.rgb();
        // All components should be positive and roughly equal for white
        assert!(rgb.x > 0.0);
        assert!(rgb.y > 0.0);
        assert!(rgb.z > 0.0);
    }

    #[test]
    fn test_chromaticity_roundtrip() {
        let cs = ColorSpace::new(ColorSpaceName::LinearRec709);
        let original = Color::new(Vec3f::new(0.5, 0.3, 0.2), cs.clone());

        let xy = original.chromaticity();

        let mut reconstructed = Color::black_in(cs);
        reconstructed.set_from_chromaticity(&xy);

        // Chromaticity doesn't preserve luminance, so just check that
        // the chromaticity matches
        let recon_xy = reconstructed.chromaticity();
        assert!((xy.x - recon_xy.x).abs() < 0.001);
        assert!((xy.y - recon_xy.y).abs() < 0.001);
    }

    #[test]
    fn test_xyz_to_xy() {
        // Test with known white point
        let xyz = Vec3f::new(0.95047, 1.0, 1.08883); // D65
        let xy = xyz_to_xy(&xyz);
        assert!((xy.x - 0.3127).abs() < 0.001);
        assert!((xy.y - 0.3290).abs() < 0.001);
    }

    #[test]
    fn test_color_convert() {
        let srgb_space = ColorSpace::new(ColorSpaceName::SRGBRec709);
        let linear_space = ColorSpace::new(ColorSpaceName::LinearRec709);

        // Mid-gray in sRGB
        let srgb_gray = Color::new(Vec3f::new(0.5, 0.5, 0.5), srgb_space);

        // Convert to linear
        let linear_gray = Color::convert(&srgb_gray, linear_space);

        // Linear value should be different (lower) than sRGB value
        // sRGB 0.5 is approximately 0.214 in linear
        let lin_rgb = linear_gray.rgb();
        assert!(lin_rgb.x < 0.5);
        assert!(lin_rgb.y < 0.5);
        assert!(lin_rgb.z < 0.5);
    }

    // ========================================================================
    // Test color constants
    // ========================================================================

    #[test]
    fn test_color_white() {
        let white = Color::white();
        assert_eq!(white.rgb(), Vec3f::new(1.0, 1.0, 1.0));
    }

    #[test]
    fn test_color_red() {
        let red = Color::red();
        assert_eq!(red.rgb(), Vec3f::new(1.0, 0.0, 0.0));
    }

    #[test]
    fn test_color_green() {
        let green = Color::green();
        assert_eq!(green.rgb(), Vec3f::new(0.0, 1.0, 0.0));
    }

    #[test]
    fn test_color_blue() {
        let blue = Color::blue();
        assert_eq!(blue.rgb(), Vec3f::new(0.0, 0.0, 1.0));
    }

    #[test]
    fn test_color_yellow() {
        let yellow = Color::yellow();
        assert_eq!(yellow.rgb(), Vec3f::new(1.0, 1.0, 0.0));
    }

    #[test]
    fn test_color_cyan() {
        let cyan = Color::cyan();
        assert_eq!(cyan.rgb(), Vec3f::new(0.0, 1.0, 1.0));
    }

    #[test]
    fn test_color_magenta() {
        let magenta = Color::magenta();
        assert_eq!(magenta.rgb(), Vec3f::new(1.0, 0.0, 1.0));
    }

    // ========================================================================
    // Test HSV conversion
    // ========================================================================

    #[test]
    fn test_from_hsv_red() {
        let cs = ColorSpace::new(ColorSpaceName::LinearRec709);
        let red = Color::from_hsv(0.0, 1.0, 1.0, cs);
        let rgb = red.rgb();
        assert!((rgb.x - 1.0).abs() < 1e-5);
        assert!(rgb.y.abs() < 1e-5);
        assert!(rgb.z.abs() < 1e-5);
    }

    #[test]
    fn test_from_hsv_green() {
        let cs = ColorSpace::new(ColorSpaceName::LinearRec709);
        let green = Color::from_hsv(120.0, 1.0, 1.0, cs);
        let rgb = green.rgb();
        assert!(rgb.x.abs() < 1e-5);
        assert!((rgb.y - 1.0).abs() < 1e-5);
        assert!(rgb.z.abs() < 1e-5);
    }

    #[test]
    fn test_from_hsv_blue() {
        let cs = ColorSpace::new(ColorSpaceName::LinearRec709);
        let blue = Color::from_hsv(240.0, 1.0, 1.0, cs);
        let rgb = blue.rgb();
        assert!(rgb.x.abs() < 1e-5);
        assert!(rgb.y.abs() < 1e-5);
        assert!((rgb.z - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_hsv_roundtrip() {
        let cs = ColorSpace::new(ColorSpaceName::LinearRec709);
        let original = Color::from_rgb(0.5, 0.75, 0.25, cs.clone());

        let (h, s, v) = original.to_hsv();
        let reconstructed = Color::from_hsv(h, s, v, cs);

        let diff = original.rgb() - reconstructed.rgb();
        assert!(diff.x.abs() < 1e-5);
        assert!(diff.y.abs() < 1e-5);
        assert!(diff.z.abs() < 1e-5);
    }

    #[test]
    fn test_to_hsv_white() {
        let white = Color::white();
        let (_h, s, v) = white.to_hsv();
        assert!(s.abs() < 1e-5); // White has no saturation
        assert!((v - 1.0).abs() < 1e-5);
    }

    // ========================================================================
    // Test hex conversion
    // ========================================================================

    #[test]
    fn test_from_hex_with_hash() {
        let cs = ColorSpace::new(ColorSpaceName::LinearRec709);
        let red = Color::from_hex("#FF0000", cs).unwrap();
        let rgb = red.rgb();
        assert!((rgb.x - 1.0).abs() < 1e-5);
        assert!(rgb.y.abs() < 1e-5);
        assert!(rgb.z.abs() < 1e-5);
    }

    #[test]
    fn test_from_hex_without_hash() {
        let cs = ColorSpace::new(ColorSpaceName::LinearRec709);
        let green = Color::from_hex("00FF00", cs).unwrap();
        let rgb = green.rgb();
        assert!(rgb.x.abs() < 1e-5);
        assert!((rgb.y - 1.0).abs() < 1e-5);
        assert!(rgb.z.abs() < 1e-5);
    }

    #[test]
    fn test_to_hex() {
        let red = Color::red();
        assert_eq!(red.to_hex(), "#FF0000");

        let green = Color::green();
        assert_eq!(green.to_hex(), "#00FF00");

        let blue = Color::blue();
        assert_eq!(blue.to_hex(), "#0000FF");
    }

    #[test]
    fn test_hex_roundtrip() {
        let cs = ColorSpace::new(ColorSpaceName::LinearRec709);
        let original_hex = "#FF8040";
        let color = Color::from_hex(original_hex, cs.clone()).unwrap();
        let result_hex = color.to_hex();
        assert_eq!(original_hex, result_hex);
    }

    #[test]
    fn test_from_hex_invalid() {
        let cs = ColorSpace::new(ColorSpaceName::LinearRec709);
        assert!(Color::from_hex("#FF00", cs.clone()).is_none()); // Too short
        assert!(Color::from_hex("#GGGGGG", cs.clone()).is_none()); // Invalid chars
        assert!(Color::from_hex("notahex", cs).is_none());
    }

    // ========================================================================
    // Test color operations
    // ========================================================================

    #[test]
    fn test_color_lerp() {
        let black = Color::black();
        let white = Color::white();

        let mid = black.lerp(&white, 0.5);
        let rgb = mid.rgb();
        assert!((rgb.x - 0.5).abs() < 1e-5);
        assert!((rgb.y - 0.5).abs() < 1e-5);
        assert!((rgb.z - 0.5).abs() < 1e-5);
    }

    #[test]
    fn test_color_clamp() {
        let cs = ColorSpace::new(ColorSpaceName::LinearRec709);
        let over = Color::from_rgb(1.5, -0.5, 0.5, cs);
        let clamped = over.clamped();

        assert_eq!(clamped.rgb().x, 1.0);
        assert_eq!(clamped.rgb().y, 0.0);
        assert_eq!(clamped.rgb().z, 0.5);
    }

    #[test]
    fn test_color_clamp_in_place() {
        let cs = ColorSpace::new(ColorSpaceName::LinearRec709);
        let mut color = Color::from_rgb(1.5, -0.5, 0.5, cs);
        color.clamp();

        assert_eq!(color.rgb().x, 1.0);
        assert_eq!(color.rgb().y, 0.0);
        assert_eq!(color.rgb().z, 0.5);
    }

    #[test]
    fn test_color_multiply() {
        let c1 = Color::from_rgb(0.5, 0.6, 0.7, ColorSpace::default());
        let c2 = Color::from_rgb(2.0, 0.5, 0.1, ColorSpace::default());
        let result = c1.multiply(&c2);

        let rgb = result.rgb();
        assert!((rgb.x - 1.0).abs() < 1e-5);
        assert!((rgb.y - 0.3).abs() < 1e-5);
        assert!((rgb.z - 0.07).abs() < 1e-5);
    }

    #[test]
    fn test_color_add() {
        let c1 = Color::from_rgb(0.2, 0.3, 0.4, ColorSpace::default());
        let c2 = Color::from_rgb(0.3, 0.2, 0.1, ColorSpace::default());
        let result = c1.add(&c2);

        let rgb = result.rgb();
        assert!((rgb.x - 0.5).abs() < 1e-5);
        assert!((rgb.y - 0.5).abs() < 1e-5);
        assert!((rgb.z - 0.5).abs() < 1e-5);
    }

    #[test]
    fn test_color_scale() {
        let c = Color::from_rgb(0.2, 0.4, 0.6, ColorSpace::default());
        let scaled = c.scale(2.0);

        let rgb = scaled.rgb();
        assert!((rgb.x - 0.4).abs() < 1e-5);
        assert!((rgb.y - 0.8).abs() < 1e-5);
        assert!((rgb.z - 1.2).abs() < 1e-5);
    }
}
