//! Utilities to map colors between gamma spaces.
//!
//! Provides functions for applying gamma correction to color values,
//! and converting between linear and display gamma spaces.
//!
//! # Display Gamma
//!
//! The display gamma is hard-coded to 2.2, which is the standard for
//! sRGB displays and most UI rendering.
//!
//! # Examples
//!
//! ```
//! use usd_gf::gamma::{apply_gamma, linear_to_display, display_to_linear, DISPLAY_GAMMA};
//!
//! // Apply gamma correction
//! let linear = 0.5_f32;
//! let display = apply_gamma(linear, 1.0 / DISPLAY_GAMMA);
//!
//! // Convert between color spaces
//! let display_color = linear_to_display(0.5_f32);
//! let linear_color = display_to_linear(display_color);
//! ```

use crate::half::Half;
use crate::vec3::{Vec3d, Vec3f, Vec3h};
use crate::vec4::{Vec4d, Vec4f, Vec4h};

/// Standard display gamma (2.2 for sRGB).
///
/// Display colors (such as colors for UI elements) are always gamma 2.2
/// and aspects of interactive rendering such as OpenGL's sRGB texture
/// format assume that space as well.
pub const DISPLAY_GAMMA: f64 = 2.2;

/// Returns the system display gamma (2.2).
#[inline]
#[must_use]
pub fn get_display_gamma() -> f64 {
    DISPLAY_GAMMA
}

/// Trait for types that support gamma correction.
pub trait GammaCorrect: Sized {
    /// Applies gamma correction to this value.
    fn apply_gamma(self, gamma: f64) -> Self;

    /// Converts from linear to display gamma space.
    fn to_display(self) -> Self {
        self.apply_gamma(1.0 / DISPLAY_GAMMA)
    }

    /// Converts from display gamma to linear space.
    fn to_linear(self) -> Self {
        self.apply_gamma(DISPLAY_GAMMA)
    }
}

impl GammaCorrect for f32 {
    fn apply_gamma(self, gamma: f64) -> Self {
        self.powf(gamma as f32)
    }
}

impl GammaCorrect for f64 {
    fn apply_gamma(self, gamma: f64) -> Self {
        self.powf(gamma)
    }
}

impl GammaCorrect for u8 {
    fn apply_gamma(self, gamma: f64) -> Self {
        ((self as f64 / 255.0).powf(gamma) * 255.0) as u8
    }
}

impl GammaCorrect for Vec3f {
    fn apply_gamma(self, gamma: f64) -> Self {
        let g = gamma as f32;
        Vec3f::new(self.x.powf(g), self.y.powf(g), self.z.powf(g))
    }
}

impl GammaCorrect for Vec3d {
    fn apply_gamma(self, gamma: f64) -> Self {
        Vec3d::new(self.x.powf(gamma), self.y.powf(gamma), self.z.powf(gamma))
    }
}

impl GammaCorrect for Vec4f {
    /// Applies gamma to RGB components, leaving alpha unchanged.
    fn apply_gamma(self, gamma: f64) -> Self {
        let g = gamma as f32;
        Vec4f::new(self.x.powf(g), self.y.powf(g), self.z.powf(g), self.w)
    }
}

impl GammaCorrect for Vec4d {
    /// Applies gamma to RGB components, leaving alpha unchanged.
    fn apply_gamma(self, gamma: f64) -> Self {
        Vec4d::new(
            self.x.powf(gamma),
            self.y.powf(gamma),
            self.z.powf(gamma),
            self.w,
        )
    }
}

impl GammaCorrect for Vec3h {
    /// Applies gamma to each component. Casts to float for pow, then back to half.
    fn apply_gamma(self, gamma: f64) -> Self {
        let g = gamma as f32;
        Vec3h::new(
            Half::from_f32(self.x.to_f32().powf(g)),
            Half::from_f32(self.y.to_f32().powf(g)),
            Half::from_f32(self.z.to_f32().powf(g)),
        )
    }
}

impl GammaCorrect for Vec4h {
    /// Applies gamma to RGB components, leaving alpha unchanged.
    fn apply_gamma(self, gamma: f64) -> Self {
        let g = gamma as f32;
        Vec4h::new(
            Half::from_f32(self.x.to_f32().powf(g)),
            Half::from_f32(self.y.to_f32().powf(g)),
            Half::from_f32(self.z.to_f32().powf(g)),
            self.w,
        )
    }
}

/// Applies gamma correction to a value.
///
/// Returns a new value with each component raised to the power `gamma`.
/// For Vec4 types, the fourth (alpha) component is unchanged.
#[inline]
pub fn apply_gamma<T: GammaCorrect>(value: T, gamma: f64) -> T {
    value.apply_gamma(gamma)
}

/// Converts a linear color value to display gamma space.
///
/// Equivalent to `apply_gamma(value, 1.0 / DISPLAY_GAMMA)`.
#[inline]
pub fn linear_to_display<T: GammaCorrect>(value: T) -> T {
    value.to_display()
}

/// Converts a display gamma color value to linear space.
///
/// Equivalent to `apply_gamma(value, DISPLAY_GAMMA)`.
#[inline]
pub fn display_to_linear<T: GammaCorrect>(value: T) -> T {
    value.to_linear()
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 1e-6;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < EPSILON
    }

    #[test]
    fn test_display_gamma() {
        assert_eq!(get_display_gamma(), 2.2);
    }

    #[test]
    fn test_apply_gamma_f32() {
        let v = 0.5_f32;
        let result = apply_gamma(v, 2.0);
        assert!((result - 0.25).abs() < 1e-6);
    }

    #[test]
    fn test_apply_gamma_f64() {
        let v = 0.5_f64;
        let result = apply_gamma(v, 2.0);
        assert!(approx_eq(result, 0.25));
    }

    #[test]
    fn test_apply_gamma_u8() {
        let v: u8 = 128;
        let result = apply_gamma(v, 2.2);
        // 128/255 ^ 2.2 * 255 ≈ 55
        assert!(result > 50 && result < 60);
    }

    #[test]
    fn test_apply_gamma_vec3f() {
        let v = Vec3f::new(0.5, 0.5, 0.5);
        let result = apply_gamma(v, 2.0);
        assert!((result.x - 0.25).abs() < 1e-6);
        assert!((result.y - 0.25).abs() < 1e-6);
        assert!((result.z - 0.25).abs() < 1e-6);
    }

    #[test]
    fn test_apply_gamma_vec4f_alpha_preserved() {
        let v = Vec4f::new(0.5, 0.5, 0.5, 0.8);
        let result = apply_gamma(v, 2.0);
        assert!((result.x - 0.25).abs() < 1e-6);
        assert!((result.w - 0.8).abs() < 1e-6); // Alpha unchanged
    }

    #[test]
    fn test_linear_to_display() {
        let linear = 0.5_f64;
        let display = linear_to_display(linear);
        // 0.5 ^ (1/2.2) ≈ 0.73
        assert!(display > 0.7 && display < 0.75);
    }

    #[test]
    fn test_display_to_linear() {
        let display = 0.73_f64;
        let linear = display_to_linear(display);
        // 0.73 ^ 2.2 ≈ 0.5
        assert!(linear > 0.45 && linear < 0.55);
    }

    #[test]
    fn test_roundtrip() {
        let original = 0.5_f64;
        let display = linear_to_display(original);
        let back = display_to_linear(display);
        assert!(approx_eq(original, back));
    }

    #[test]
    fn test_identity_gamma() {
        let v = 0.5_f64;
        let result = apply_gamma(v, 1.0);
        assert!(approx_eq(v, result));
    }

    #[test]
    fn test_vec3d_gamma() {
        let v = Vec3d::new(0.25, 0.5, 0.75);
        let result = apply_gamma(v, 0.5); // Square root
        assert!(approx_eq(result.x, 0.5));
        assert!(approx_eq(result.y, 0.5_f64.sqrt()));
    }
}
