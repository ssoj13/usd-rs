//! Color space representation.
//!
//! This module provides [`ColorSpace`] for representing and converting between
//! color spaces used in computer graphics and film production.
//!
//! # Overview
//!
//! Color spaces define how RGB values are interpreted. Different industries
//! and standards use different color spaces (sRGB, ACEScg, Rec.709, etc.).
//!
//! # Supported Color Spaces
//!
//! - **Linear spaces**: lin_ap1_scene (ACEScg), lin_ap0_scene (ACES2065-1),
//!   lin_rec709_scene, lin_p3d65_scene, lin_rec2020_scene
//! - **Gamma spaces**: srgb_rec709_scene, g22_rec709_scene, g18_rec709_scene
//! - **Special**: data, unknown, identity, raw
//!
//! # Examples
//!
//! ```
//! use usd_gf::{ColorSpace, ColorSpaceName};
//!
//! let srgb = ColorSpace::new(ColorSpaceName::SRGBRec709);
//! let linear = ColorSpace::new(ColorSpaceName::LinearRec709);
//!
//! assert_ne!(srgb, linear);
//! ```

use std::fmt;
use std::sync::Arc;

use crate::{Matrix3f, Vec2f, Vec3f};
use usd_tf::Token;

/// Predefined color space names.
///
/// These names follow the Color Interop Forum naming convention:
/// `{encoding}_{primaries}_{state}`
///
/// - Encoding: `lin` (linear), `srgb`, `g22` (gamma 2.2), `g18` (gamma 1.8)
/// - Primaries: `ap1` (ACEScg), `ap0` (ACES), `rec709`, `p3d65`, `rec2020`
/// - State: `scene` (scene-referred)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ColorSpaceName {
    /// ACEScg - Linear AP1 primaries.
    LinearAP1,
    /// ACES2065-1 - Linear AP0 primaries.
    LinearAP0,
    /// Linear Rec.709 (sRGB primaries).
    #[default]
    LinearRec709,
    /// Linear P3-D65.
    LinearP3D65,
    /// Linear Rec.2020.
    LinearRec2020,
    /// Linear AdobeRGB.
    LinearAdobeRGB,
    /// CIE XYZ-D65 Scene-referred.
    LinearCIEXYZD65,
    /// sRGB Encoded Rec.709.
    SRGBRec709,
    /// Gamma 2.2 Encoded Rec.709.
    G22Rec709,
    /// Gamma 1.8 Encoded Rec.709.
    G18Rec709,
    /// sRGB Encoded AP1.
    SRGBAP1,
    /// Gamma 2.2 Encoded AP1.
    G22AP1,
    /// sRGB Encoded P3-D65.
    SRGBP3D65,
    /// Gamma 2.2 Encoded AdobeRGB.
    G22AdobeRGB,
    /// Identity/passthrough.
    Identity,
    /// Data (no color interpretation).
    Data,
    /// Raw data.
    Raw,
    /// Unknown color space.
    Unknown,
}

impl ColorSpaceName {
    /// Returns the string name for this color space.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::LinearAP1 => "lin_ap1_scene",
            Self::LinearAP0 => "lin_ap0_scene",
            Self::LinearRec709 => "lin_rec709_scene",
            Self::LinearP3D65 => "lin_p3d65_scene",
            Self::LinearRec2020 => "lin_rec2020_scene",
            Self::LinearAdobeRGB => "lin_adobergb_scene",
            Self::LinearCIEXYZD65 => "lin_ciexyzd65_scene",
            Self::SRGBRec709 => "srgb_rec709_scene",
            Self::G22Rec709 => "g22_rec709_scene",
            Self::G18Rec709 => "g18_rec709_scene",
            Self::SRGBAP1 => "srgb_ap1_scene",
            Self::G22AP1 => "g22_ap1_scene",
            Self::SRGBP3D65 => "srgb_p3d65_scene",
            Self::G22AdobeRGB => "g22_adobergb_scene",
            Self::Identity => "identity",
            Self::Data => "data",
            Self::Raw => "raw",
            Self::Unknown => "unknown",
        }
    }

    /// Returns the Token for this color space name.
    #[must_use]
    pub fn to_token(&self) -> Token {
        Token::new(self.as_str())
    }

    /// Parse a color space name from a string.
    /// Matches OpenUSD NcGetNamedColorSpace: supports both short (lin_ap1_scene)
    /// and descriptive ("ACEScg") names.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            // Short names (CIF)
            "lin_ap1_scene" => Some(Self::LinearAP1),
            "lin_ap0_scene" => Some(Self::LinearAP0),
            "lin_rec709_scene" => Some(Self::LinearRec709),
            "lin_p3d65_scene" => Some(Self::LinearP3D65),
            "lin_rec2020_scene" => Some(Self::LinearRec2020),
            "lin_adobergb_scene" => Some(Self::LinearAdobeRGB),
            "lin_ciexyzd65_scene" => Some(Self::LinearCIEXYZD65),
            "srgb_rec709_scene" => Some(Self::SRGBRec709),
            "g22_rec709_scene" => Some(Self::G22Rec709),
            "g18_rec709_scene" => Some(Self::G18Rec709),
            "srgb_ap1_scene" => Some(Self::SRGBAP1),
            "g22_ap1_scene" => Some(Self::G22AP1),
            "srgb_p3d65_scene" => Some(Self::SRGBP3D65),
            "g22_adobergb_scene" => Some(Self::G22AdobeRGB),
            "identity" => Some(Self::Identity),
            "data" => Some(Self::Data),
            "raw" => Some(Self::Raw),
            "unknown" => Some(Self::Unknown),
            // Deprecated aliases
            "CIEXYZ" => Some(Self::LinearCIEXYZD65),
            "LinearDisplayP3" => Some(Self::LinearP3D65),
            // Descriptive names (per nanocolor _colorSpaces[].desc.descriptiveName)
            "ACEScg" => Some(Self::LinearAP1),
            "ACES2065-1" => Some(Self::LinearAP0),
            "Linear Rec.709 (sRGB)" | "Linear Rec.709" => Some(Self::LinearRec709),
            "Linear P3-D65" => Some(Self::LinearP3D65),
            "Linear Rec.2020" => Some(Self::LinearRec2020),
            "Linear AdobeRGB" => Some(Self::LinearAdobeRGB),
            "CIE XYZ-D65 - Scene-referred" => Some(Self::LinearCIEXYZD65),
            "sRGB Encoded Rec.709 (sRGB)" | "sRGB" => Some(Self::SRGBRec709),
            "sRGB Encoded AP1" => Some(Self::SRGBAP1),
            "sRGB Encoded P3-D65" => Some(Self::SRGBP3D65),
            "Gamma 2.2 Encoded Rec.709" => Some(Self::G22Rec709),
            "Gamma 2.2 Encoded AP1" => Some(Self::G22AP1),
            "Gamma 2.2 Encoded AdobeRGB" => Some(Self::G22AdobeRGB),
            "Gamma 1.8 Encoded Rec.709" => Some(Self::G18Rec709),
            _ => None,
        }
    }

    /// Returns true if this is a linear color space.
    #[must_use]
    pub const fn is_linear(&self) -> bool {
        matches!(
            self,
            Self::LinearAP1
                | Self::LinearAP0
                | Self::LinearRec709
                | Self::LinearP3D65
                | Self::LinearRec2020
                | Self::LinearAdobeRGB
                | Self::LinearCIEXYZD65
        )
    }

    /// Returns the gamma value for this color space.
    #[must_use]
    pub const fn gamma(&self) -> f32 {
        match self {
            Self::LinearAP1
            | Self::LinearAP0
            | Self::LinearRec709
            | Self::LinearP3D65
            | Self::LinearRec2020
            | Self::LinearAdobeRGB
            | Self::LinearCIEXYZD65
            | Self::Identity
            | Self::Data
            | Self::Raw
            | Self::Unknown => 1.0,
            Self::SRGBRec709 | Self::SRGBAP1 | Self::SRGBP3D65 => 2.4, // sRGB uses ~2.4
            Self::G22Rec709 | Self::G22AP1 | Self::G22AdobeRGB => 2.2,
            Self::G18Rec709 => 1.8,
        }
    }
}

impl fmt::Display for ColorSpaceName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Internal color space data.
#[derive(Clone, Debug)]
struct ColorSpaceData {
    /// Color space name.
    name: Token,
    /// RGB to XYZ conversion matrix.
    rgb_to_xyz: Matrix3f,
    /// Gamma value.
    gamma: f32,
    /// Linear bias for transfer function.
    linear_bias: f32,
    /// K0, phi: transfer function params per OpenUSD NcGetK0Phi / _InitColorSpace.
    k0: f32,
    phi: f32,
    /// Red chromaticity (if constructed from primaries).
    red_chroma: Option<Vec2f>,
    /// Green chromaticity.
    green_chroma: Option<Vec2f>,
    /// Blue chromaticity.
    blue_chroma: Option<Vec2f>,
    /// White point.
    white_point: Option<Vec2f>,
}

/// Per OpenUSD _InitColorSpace: compute K0, phi from gamma and linearBias.
fn compute_k0_phi(gamma: f32, linear_bias: f32) -> (f32, f32) {
    let a = linear_bias;
    if gamma == 1.0 {
        (1e9, 1.0)
    } else if a <= 0.0 {
        (0.0, 1.0)
    } else {
        let k0 = a / (gamma - 1.0);
        let inner = gamma * a / (gamma + gamma * a - 1.0 - a);
        let phi = a / (inner.powf(gamma) * (gamma - 1.0));
        (k0, phi)
    }
}

impl Default for ColorSpaceData {
    fn default() -> Self {
        let gamma = 1.0;
        let linear_bias = 0.0;
        let (k0, phi) = compute_k0_phi(gamma, linear_bias);
        Self {
            name: Token::new("lin_rec709_scene"),
            rgb_to_xyz: rec709_to_xyz_matrix(),
            gamma,
            linear_bias,
            k0,
            phi,
            red_chroma: None,
            green_chroma: None,
            blue_chroma: None,
            white_point: None,
        }
    }
}

/// A color space for color conversion operations.
///
/// Color spaces define how RGB values map to physical colors. This type
/// supports conversion between different color spaces used in production.
///
/// # Examples
///
/// ```
/// use usd_gf::{ColorSpace, ColorSpaceName};
///
/// // Create from predefined name
/// let srgb = ColorSpace::new(ColorSpaceName::SRGBRec709);
///
/// // Get properties
/// assert!(!srgb.is_linear());
/// ```
#[derive(Clone, Debug)]
pub struct ColorSpace {
    data: Arc<ColorSpaceData>,
}

impl ColorSpace {
    /// Construct a color space from a predefined name.
    #[must_use]
    pub fn new(name: ColorSpaceName) -> Self {
        Self::from_token(&name.to_token())
    }

    /// Construct a color space from a name token.
    #[must_use]
    pub fn from_token(name: &Token) -> Self {
        let name_str = name.as_str();
        let parsed = ColorSpaceName::parse(name_str);

        let (rgb_to_xyz, gamma, linear_bias) = match parsed {
            // Linear color spaces - gamma 1.0, no bias
            Some(ColorSpaceName::LinearRec709) => (rec709_to_xyz_matrix(), 1.0, 0.0),
            Some(ColorSpaceName::LinearAP1) => (ap1_to_xyz_matrix(), 1.0, 0.0),
            Some(ColorSpaceName::LinearAP0) => (ap0_to_xyz_matrix(), 1.0, 0.0),
            Some(ColorSpaceName::LinearP3D65) => (p3d65_to_xyz_matrix(), 1.0, 0.0),
            Some(ColorSpaceName::LinearRec2020) => (rec2020_to_xyz_matrix(), 1.0, 0.0),
            Some(ColorSpaceName::LinearAdobeRGB) => (adobergb_to_xyz_matrix(), 1.0, 0.0),
            Some(ColorSpaceName::LinearCIEXYZD65) => (Matrix3f::identity(), 1.0, 0.0),

            // sRGB transfer function - gamma 2.4, bias 0.055
            Some(ColorSpaceName::SRGBRec709) => (rec709_to_xyz_matrix(), 2.4, 0.055),
            Some(ColorSpaceName::SRGBAP1) => (ap1_to_xyz_matrix(), 2.4, 0.055),
            Some(ColorSpaceName::SRGBP3D65) => (p3d65_to_xyz_matrix(), 2.4, 0.055),

            // Pure gamma color spaces - no bias
            Some(ColorSpaceName::G22Rec709) => (rec709_to_xyz_matrix(), 2.2, 0.0),
            Some(ColorSpaceName::G18Rec709) => (rec709_to_xyz_matrix(), 1.8, 0.0),
            Some(ColorSpaceName::G22AP1) => (ap1_to_xyz_matrix(), 2.2, 0.0),
            Some(ColorSpaceName::G22AdobeRGB) => (adobergb_to_xyz_matrix(), 2.2, 0.0),

            // Special/passthrough
            _ => (Matrix3f::identity(), 1.0, 0.0),
        };

        let (k0, phi) = compute_k0_phi(gamma, linear_bias);
        Self {
            data: Arc::new(ColorSpaceData {
                name: name.clone(),
                rgb_to_xyz,
                gamma,
                linear_bias,
                k0,
                phi,
                ..Default::default()
            }),
        }
    }

    /// Construct a custom color space from chromaticity coordinates.
    #[must_use]
    pub fn from_primaries(
        name: &Token,
        red_chroma: Vec2f,
        green_chroma: Vec2f,
        blue_chroma: Vec2f,
        white_point: Vec2f,
        gamma: f32,
        linear_bias: f32,
    ) -> Self {
        let rgb_to_xyz =
            primaries_to_xyz_matrix(&red_chroma, &green_chroma, &blue_chroma, &white_point);

        let (k0, phi) = compute_k0_phi(gamma, linear_bias);
        Self {
            data: Arc::new(ColorSpaceData {
                name: name.clone(),
                rgb_to_xyz,
                gamma,
                linear_bias,
                k0,
                phi,
                red_chroma: Some(red_chroma),
                green_chroma: Some(green_chroma),
                blue_chroma: Some(blue_chroma),
                white_point: Some(white_point),
            }),
        }
    }

    /// Construct a color space from a 3x3 matrix and linearization parameters.
    #[must_use]
    pub fn from_matrix(name: &Token, rgb_to_xyz: Matrix3f, gamma: f32, linear_bias: f32) -> Self {
        let (k0, phi) = compute_k0_phi(gamma, linear_bias);
        Self {
            data: Arc::new(ColorSpaceData {
                name: name.clone(),
                rgb_to_xyz,
                gamma,
                linear_bias,
                k0,
                phi,
                ..Default::default()
            }),
        }
    }

    /// Check if a color space name is valid.
    /// Accepts both short names (lin_ap1_scene) and descriptive names (ACEScg).
    #[must_use]
    pub fn is_valid_name(name: &Token) -> bool {
        ColorSpaceName::parse(name.as_str()).is_some()
    }

    /// Get the name of the color space.
    #[must_use]
    pub fn name(&self) -> &Token {
        &self.data.name
    }

    /// Get the RGB to XYZ conversion matrix.
    #[must_use]
    pub fn rgb_to_xyz(&self) -> &Matrix3f {
        &self.data.rgb_to_xyz
    }

    /// Convert RGB to XYZ applying transfer function (ToLinear) first.
    /// Matches OpenUSD NcRGBToXYZ used for chromaticity.
    #[must_use]
    pub fn rgb_to_xyz_linearized(&self, rgb: &Vec3f) -> Vec3f {
        let linear = if self.is_linear() {
            *rgb
        } else {
            let (k0, phi) = self.transfer_function_params();
            to_linear(rgb, k0, phi, self.gamma(), self.linear_bias())
        };
        self.data.rgb_to_xyz * linear
    }

    /// Convert XYZ to RGB applying transfer function (FromLinear) after matrix.
    /// Matches OpenUSD NcXYZToRGB.
    #[must_use]
    pub fn xyz_to_rgb_with_transfer(&self, xyz: &Vec3f) -> Vec3f {
        let matrix = self.data.rgb_to_xyz.inverse().unwrap_or_default();
        let linear_rgb = matrix * *xyz;
        if self.is_linear() {
            linear_rgb
        } else {
            let (k0, phi) = self.transfer_function_params();
            from_linear(&linear_rgb, k0, phi, self.gamma(), self.linear_bias())
        }
    }

    /// Get the gamma value.
    #[must_use]
    pub fn gamma(&self) -> f32 {
        self.data.gamma
    }

    /// Get the linear bias.
    #[must_use]
    pub fn linear_bias(&self) -> f32 {
        self.data.linear_bias
    }

    /// Returns true if this is a linear color space.
    #[must_use]
    pub fn is_linear(&self) -> bool {
        (self.data.gamma - 1.0).abs() < f32::EPSILON
    }

    /// Get the primaries and white point if available.
    #[must_use]
    pub fn primaries_and_white_point(&self) -> Option<(Vec2f, Vec2f, Vec2f, Vec2f)> {
        match (
            &self.data.red_chroma,
            &self.data.green_chroma,
            &self.data.blue_chroma,
            &self.data.white_point,
        ) {
            (Some(r), Some(g), Some(b), Some(w)) => Some((*r, *g, *b, *w)),
            _ => None,
        }
    }

    /// Get the transfer function parameters (K0, Phi). Per OpenUSD NcGetK0Phi.
    #[must_use]
    pub fn transfer_function_params(&self) -> (f32, f32) {
        (self.data.k0, self.data.phi)
    }

    /// Get the RGB to RGB conversion matrix from another color space.
    #[must_use]
    pub fn rgb_to_rgb_matrix(&self, src: &ColorSpace) -> Matrix3f {
        let src_to_xyz = &src.data.rgb_to_xyz;
        let xyz_to_dst = self.data.rgb_to_xyz.inverse().unwrap_or_default();
        xyz_to_dst * *src_to_xyz
    }

    /// Convert an RGB value from another color space to this one.
    #[must_use]
    pub fn convert(&self, src: &ColorSpace, rgb: &Vec3f) -> Vec3f {
        let linear_rgb = if src.is_linear() {
            *rgb
        } else {
            to_linear(
                rgb,
                src.data.k0,
                src.data.phi,
                src.data.gamma,
                src.data.linear_bias,
            )
        };
        let matrix = self.rgb_to_rgb_matrix(src);
        let converted = matrix * linear_rgb;
        if self.is_linear() {
            converted
        } else {
            from_linear(
                &converted,
                self.data.k0,
                self.data.phi,
                self.data.gamma,
                self.data.linear_bias,
            )
        }
    }

    /// Convert a color from another space to this one, returning Color.
    /// Per C++ GfColorSpace::Convert(rgb) -> GfColor.
    #[must_use]
    pub fn convert_to_color(&self, src: &ColorSpace, rgb: &Vec3f) -> crate::Color {
        let converted = self.convert(src, rgb);
        crate::Color::new(converted, self.clone())
    }

    /// Convert a packed array of RGB values from another color space to this one.
    ///
    /// The array is modified in place. Values are expected to be packed as [r,g,b,r,g,b,...].
    /// The array length must be a multiple of 3.
    ///
    /// # Arguments
    ///
    /// * `src` - Source color space
    /// * `rgb` - Mutable slice of RGB values
    ///
    /// # Panics
    ///
    /// Panics if the array length is not a multiple of 3.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::{ColorSpace, ColorSpaceName};
    ///
    /// let src = ColorSpace::new(ColorSpaceName::SRGBRec709);
    /// let dst = ColorSpace::new(ColorSpaceName::LinearRec709);
    /// let mut values = vec![0.5, 0.5, 0.5, 1.0, 0.0, 0.0];
    /// dst.convert_rgb_span(&src, &mut values);
    /// // values are now in linear space
    /// ```
    pub fn convert_rgb_span(&self, src: &ColorSpace, rgb: &mut [f32]) {
        if !rgb.len().is_multiple_of(3) {
            return;
        }

        let matrix = self.rgb_to_rgb_matrix(src);

        for chunk in rgb.chunks_exact_mut(3) {
            let mut v = Vec3f::new(chunk[0], chunk[1], chunk[2]);

            // Linearize if source is non-linear (per OpenUSD K0/phi)
            if !src.is_linear() {
                let (k0, phi) = src.transfer_function_params();
                v = to_linear(&v, k0, phi, src.gamma(), src.linear_bias());
            }

            // Transform
            v = matrix * v;

            // Delinearize if destination is non-linear
            if !self.is_linear() {
                let (k0, phi) = self.transfer_function_params();
                v = from_linear(&v, k0, phi, self.gamma(), self.linear_bias());
            }

            chunk[0] = v.x;
            chunk[1] = v.y;
            chunk[2] = v.z;
        }
    }

    /// Convert a packed array of RGBA values from another color space to this one.
    ///
    /// The array is modified in place. Values are expected to be packed as [r,g,b,a,r,g,b,a,...].
    /// The alpha channel is preserved unchanged. The array length must be a multiple of 4.
    ///
    /// # Arguments
    ///
    /// * `src` - Source color space
    /// * `rgba` - Mutable slice of RGBA values
    ///
    /// # Panics
    ///
    /// Panics if the array length is not a multiple of 4.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_gf::{ColorSpace, ColorSpaceName};
    ///
    /// let src = ColorSpace::new(ColorSpaceName::SRGBRec709);
    /// let dst = ColorSpace::new(ColorSpaceName::LinearRec709);
    /// let mut values = vec![0.5, 0.5, 0.5, 1.0, 1.0, 0.0, 0.0, 0.5];
    /// dst.convert_rgba_span(&src, &mut values);
    /// // RGB values are now in linear space, alpha unchanged
    /// ```
    pub fn convert_rgba_span(&self, src: &ColorSpace, rgba: &mut [f32]) {
        if !rgba.len().is_multiple_of(4) {
            return;
        }

        let matrix = self.rgb_to_rgb_matrix(src);

        for chunk in rgba.chunks_exact_mut(4) {
            let mut v = Vec3f::new(chunk[0], chunk[1], chunk[2]);
            let alpha = chunk[3]; // Preserve alpha

            // Linearize if source is non-linear (per OpenUSD K0/phi)
            if !src.is_linear() {
                let (k0, phi) = src.transfer_function_params();
                v = to_linear(&v, k0, phi, src.gamma(), src.linear_bias());
            }

            // Transform
            v = matrix * v;

            // Delinearize if destination is non-linear
            if !self.is_linear() {
                let (k0, phi) = self.transfer_function_params();
                v = from_linear(&v, k0, phi, self.gamma(), self.linear_bias());
            }

            chunk[0] = v.x;
            chunk[1] = v.y;
            chunk[2] = v.z;
            chunk[3] = alpha; // Keep alpha unchanged
        }
    }
}

impl PartialEq for ColorSpace {
    /// Per OpenUSD: compares RGB-to-XYZ matrix, gamma, and linear bias
    /// (colorimetric equality), not just the name token.
    fn eq(&self, other: &Self) -> bool {
        const MATRIX_EPS: f32 = 1e-5;
        const PARAM_EPS: f32 = 1e-3;
        let m1 = self.rgb_to_xyz();
        let m2 = other.rgb_to_xyz();
        for i in 0..3 {
            for j in 0..3 {
                if (m1[i][j] - m2[i][j]).abs() > MATRIX_EPS {
                    return false;
                }
            }
        }
        (self.gamma() - other.gamma()).abs() <= PARAM_EPS
            && (self.linear_bias() - other.linear_bias()).abs() <= PARAM_EPS
    }
}

impl Eq for ColorSpace {}

impl Default for ColorSpace {
    fn default() -> Self {
        Self::new(ColorSpaceName::LinearRec709)
    }
}

impl fmt::Display for ColorSpace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ColorSpace({})", self.data.name)
    }
}

// Helper functions for color space matrices

/// Rec.709/sRGB to XYZ matrix.
fn rec709_to_xyz_matrix() -> Matrix3f {
    Matrix3f::new(
        0.4124564, 0.3575761, 0.1804375, 0.2126729, 0.7151522, 0.0721750, 0.0193339, 0.119_192,
        0.9503041,
    )
}

/// ACEScg (AP1) to XYZ matrix.
fn ap1_to_xyz_matrix() -> Matrix3f {
    Matrix3f::new(
        0.6624542, 0.1340042, 0.1561877, 0.2722287, 0.6740818, 0.0536895, -0.0055746, 0.0040607,
        1.0103391,
    )
}

/// ACES (AP0) to XYZ matrix.
fn ap0_to_xyz_matrix() -> Matrix3f {
    Matrix3f::new(
        0.9525524, 0.0000000, 0.0000937, 0.3439664, 0.7281661, -0.0721325, 0.0000000, 0.0000000,
        1.0088251,
    )
}

/// Rec.2020 to XYZ matrix (D65 white point).
fn rec2020_to_xyz_matrix() -> Matrix3f {
    Matrix3f::new(
        0.6369580, 0.1446169, 0.1688810, 0.2627002, 0.6779981, 0.0593017, 0.0000000, 0.0280727,
        1.0609851,
    )
}

/// AdobeRGB to XYZ matrix (D65 white point).
fn adobergb_to_xyz_matrix() -> Matrix3f {
    Matrix3f::new(
        0.5767309, 0.1855540, 0.1881852, 0.2973769, 0.6273491, 0.0752741, 0.0270343, 0.0706872,
        0.9911085,
    )
}

/// P3-D65 to XYZ matrix.
fn p3d65_to_xyz_matrix() -> Matrix3f {
    Matrix3f::new(
        0.4865709, 0.2656677, 0.1982173, 0.2289746, 0.6917385, 0.0792869, 0.0000000, 0.0451134,
        1.0439444,
    )
}

/// Compute RGB to XYZ matrix from chromaticity primaries.
fn primaries_to_xyz_matrix(red: &Vec2f, green: &Vec2f, blue: &Vec2f, white: &Vec2f) -> Matrix3f {
    // Convert chromaticity to XYZ
    let rx = red.x / red.y;
    let ry = 1.0;
    let rz = (1.0 - red.x - red.y) / red.y;

    let gx = green.x / green.y;
    let gy = 1.0;
    let gz = (1.0 - green.x - green.y) / green.y;

    let bx = blue.x / blue.y;
    let by = 1.0;
    let bz = (1.0 - blue.x - blue.y) / blue.y;

    let wx = white.x / white.y;
    let wy = 1.0;
    let wz = (1.0 - white.x - white.y) / white.y;

    // Build primaries matrix
    let primaries = Matrix3f::new(rx, gx, bx, ry, gy, by, rz, gz, bz);

    // Solve for scale factors
    if let Some(inv) = primaries.inverse() {
        let w = Vec3f::new(wx, wy, wz);
        let s = inv * w;

        // Scale primaries by luminance
        Matrix3f::new(
            s.x * rx,
            s.y * gx,
            s.z * bx,
            s.x * ry,
            s.y * gy,
            s.z * by,
            s.x * rz,
            s.y * gz,
            s.z * bz,
        )
    } else {
        Matrix3f::identity()
    }
}

/// Per OpenUSD _ToLinear (nanocolor.c:46-51): display -> linear using K0, phi.
fn to_linear(rgb: &Vec3f, k0: f32, phi: f32, gamma: f32, linear_bias: f32) -> Vec3f {
    let comp = |c: f32| -> f32 {
        if c < k0 {
            c / phi
        } else {
            ((c + linear_bias) / (1.0 + linear_bias)).powf(gamma)
        }
    };
    Vec3f::new(comp(rgb.x), comp(rgb.y), comp(rgb.z))
}

/// Per OpenUSD _FromLinear (nanocolor.c:39-44): linear -> display using K0, phi.
fn from_linear(rgb: &Vec3f, k0: f32, phi: f32, gamma: f32, linear_bias: f32) -> Vec3f {
    let comp = |c: f32| -> f32 {
        if c < k0 / phi {
            c * phi
        } else {
            (1.0 + linear_bias) * c.powf(1.0 / gamma) - linear_bias
        }
    };
    Vec3f::new(comp(rgb.x), comp(rgb.y), comp(rgb.z))
}

/// Legacy linearize (uses to_linear with cs params). Use to_linear when k0/phi available.
#[allow(dead_code)] // used by test_linearize_delinearize_roundtrip
fn linearize(rgb: &Vec3f, gamma: f32, linear_bias: f32) -> Vec3f {
    let (k0, phi) = compute_k0_phi(gamma, linear_bias);
    to_linear(rgb, k0, phi, gamma, linear_bias)
}

/// Legacy delinearize (uses from_linear with cs params).
#[allow(dead_code)] // used by test_linearize_delinearize_roundtrip
fn delinearize(rgb: &Vec3f, gamma: f32, linear_bias: f32) -> Vec3f {
    let (k0, phi) = compute_k0_phi(gamma, linear_bias);
    from_linear(rgb, k0, phi, gamma, linear_bias)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_space_name_str() {
        assert_eq!(ColorSpaceName::LinearRec709.as_str(), "lin_rec709_scene");
        assert_eq!(ColorSpaceName::SRGBRec709.as_str(), "srgb_rec709_scene");
        assert_eq!(ColorSpaceName::LinearAP1.as_str(), "lin_ap1_scene");
    }

    #[test]
    fn test_color_space_name_parse() {
        assert_eq!(
            ColorSpaceName::parse("lin_rec709_scene"),
            Some(ColorSpaceName::LinearRec709)
        );
        assert_eq!(
            ColorSpaceName::parse("srgb_rec709_scene"),
            Some(ColorSpaceName::SRGBRec709)
        );
        assert_eq!(ColorSpaceName::parse("invalid"), None);
    }

    #[test]
    fn test_color_space_name_is_linear() {
        assert!(ColorSpaceName::LinearRec709.is_linear());
        assert!(ColorSpaceName::LinearAP1.is_linear());
        assert!(!ColorSpaceName::SRGBRec709.is_linear());
        assert!(!ColorSpaceName::G22Rec709.is_linear());
    }

    #[test]
    fn test_color_space_new() {
        let cs = ColorSpace::new(ColorSpaceName::LinearRec709);
        assert_eq!(cs.name().as_str(), "lin_rec709_scene");
        assert!(cs.is_linear());
    }

    #[test]
    fn test_color_space_srgb() {
        let cs = ColorSpace::new(ColorSpaceName::SRGBRec709);
        assert!(!cs.is_linear());
        assert!((cs.gamma() - 2.4).abs() < 0.01);
    }

    #[test]
    fn test_color_space_equality() {
        let cs1 = ColorSpace::new(ColorSpaceName::LinearRec709);
        let cs2 = ColorSpace::new(ColorSpaceName::LinearRec709);
        let cs3 = ColorSpace::new(ColorSpaceName::SRGBRec709);

        assert_eq!(cs1, cs2);
        assert_ne!(cs1, cs3);
    }

    #[test]
    fn test_color_space_valid_name() {
        assert!(ColorSpace::is_valid_name(&Token::new("lin_rec709_scene")));
        assert!(!ColorSpace::is_valid_name(&Token::new("invalid_name")));
    }

    #[test]
    fn test_color_space_default() {
        let cs = ColorSpace::default();
        assert_eq!(cs.name().as_str(), "lin_rec709_scene");
    }

    #[test]
    fn test_color_space_display() {
        let cs = ColorSpace::new(ColorSpaceName::LinearAP1);
        let s = format!("{}", cs);
        assert!(s.contains("lin_ap1_scene"));
    }

    #[test]
    fn test_linearize_delinearize_roundtrip() {
        let rgb = Vec3f::new(0.5, 0.5, 0.5);
        let gamma = 2.2;
        let bias = 0.0;

        let linear = linearize(&rgb, gamma, bias);
        let back = delinearize(&linear, gamma, bias);

        assert!((rgb.x - back.x).abs() < 1e-5);
        assert!((rgb.y - back.y).abs() < 1e-5);
        assert!((rgb.z - back.z).abs() < 1e-5);
    }

    #[test]
    fn test_rec709_matrix() {
        let m = rec709_to_xyz_matrix();
        // Check that matrix is approximately correct
        assert!((m[0][0] - 0.4124564).abs() < 1e-5);
    }

    #[test]
    fn test_transfer_function_params() {
        // Per OpenUSD _InitColorSpace: linear (gamma=1) -> K0=1e9, phi=1
        let linear = ColorSpace::new(ColorSpaceName::LinearRec709);
        let (k0, phi) = linear.transfer_function_params();
        assert!(
            (k0 - 1e9).abs() < 1e3,
            "linear space K0 should be 1e9, got {k0}"
        );
        assert!((phi - 1.0).abs() < 1e-5);

        // Pure gamma (a<=0) -> K0=0, phi=1
        let g22 = ColorSpace::new(ColorSpaceName::G22Rec709);
        let (k0_g, phi_g) = g22.transfer_function_params();
        assert!((k0_g - 0.0).abs() < 1e-5);
        assert!((phi_g - 1.0).abs() < 1e-5);

        // sRGB (gamma=2.4, a=0.055) -> K0 and phi per formula
        let srgb = ColorSpace::new(ColorSpaceName::SRGBRec709);
        let (k0_s, phi_s) = srgb.transfer_function_params();
        assert!(k0_s > 0.0 && k0_s < 1.0);
        assert!(phi_s > 0.0);
    }

    #[test]
    fn test_convert_rgb_span() {
        let srgb = ColorSpace::new(ColorSpaceName::SRGBRec709);
        let linear = ColorSpace::new(ColorSpaceName::LinearRec709);

        // Two colors: mid-gray and pure red
        let mut values = vec![0.5, 0.5, 0.5, 1.0, 0.0, 0.0];
        linear.convert_rgb_span(&srgb, &mut values);

        // First color should be darker in linear space
        assert!(values[0] < 0.5);
        assert!(values[1] < 0.5);
        assert!(values[2] < 0.5);

        // Pure red should remain mostly red
        assert!(values[3] > 0.9);
        assert!(values[4] < 0.1);
        assert!(values[5] < 0.1);
    }

    #[test]
    fn test_convert_rgb_span_invalid_length() {
        // Per OpenUSD: invalid length returns without converting, no panic
        let cs = ColorSpace::new(ColorSpaceName::LinearRec709);
        let mut values = vec![0.5, 0.5]; // Invalid: not multiple of 3
        cs.convert_rgb_span(&cs, &mut values);
        assert_eq!(values, [0.5, 0.5]); // Unchanged
    }

    #[test]
    fn test_convert_rgba_span() {
        let srgb = ColorSpace::new(ColorSpaceName::SRGBRec709);
        let linear = ColorSpace::new(ColorSpaceName::LinearRec709);

        // Two colors with alpha: mid-gray@0.5 and pure red@1.0
        let mut values = vec![0.5, 0.5, 0.5, 0.5, 1.0, 0.0, 0.0, 1.0];
        let original_alpha = [values[3], values[7]];

        linear.convert_rgba_span(&srgb, &mut values);

        // RGB should be converted
        assert!(values[0] < 0.5);
        assert!(values[1] < 0.5);
        assert!(values[2] < 0.5);

        // Alpha should be preserved
        assert_eq!(values[3], original_alpha[0]);
        assert_eq!(values[7], original_alpha[1]);
    }

    #[test]
    fn test_convert_rgba_span_invalid_length() {
        let cs = ColorSpace::new(ColorSpaceName::LinearRec709);
        let mut values = vec![0.5, 0.5, 0.5]; // Invalid: not multiple of 4
        cs.convert_rgba_span(&cs, &mut values);
        // Per OpenUSD: returns without converting, no panic
        assert_eq!(values, vec![0.5, 0.5, 0.5]);
    }

    #[test]
    fn test_batch_conversion_roundtrip() {
        let srgb = ColorSpace::new(ColorSpaceName::SRGBRec709);
        let linear = ColorSpace::new(ColorSpaceName::LinearRec709);

        let mut values = vec![0.5, 0.3, 0.7, 0.2, 0.8, 0.4];
        let original = values.clone();

        // Convert to linear and back
        linear.convert_rgb_span(&srgb, &mut values);
        srgb.convert_rgb_span(&linear, &mut values);

        // Should be close to original
        for (a, b) in values.iter().zip(original.iter()) {
            assert!((a - b).abs() < 1e-5);
        }
    }
}
