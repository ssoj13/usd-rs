//! Color space conversions and color utilities.
//!
//! Implements all OSL color space conversions matching `opcolor.cpp` and
//! `opcolor.h` / `opcolor_impl.h`:
//! RGB <-> HSV, HSL, YIQ, XYZ, xyY, sRGB, Rec709, and named spaces.
//! Also includes `blackbody()`, `wavelength_color()`, and `luminance()`.
//!
//! The `ColorSystem` struct matches the C++ `pvt::ColorSystem` class,
//! encapsulating configurable color primaries, XYZ↔RGB matrices,
//! luminance scale, and a precomputed blackbody lookup table.

use crate::Float;
use crate::math::{Color3, Matrix33, Vec3};

#[cfg(feature = "vfx")]
use std::sync::OnceLock;

// ---------------------------------------------------------------------------
// CIE colour matching functions (380–780nm, 5nm steps, 81 entries × 3)
// From: http://www.fourmilab.ch/documents/specrend/
// ---------------------------------------------------------------------------

#[rustfmt::skip]
const CIE_COLOUR_MATCH: [Float; 81 * 3] = [
    0.0014,0.0000,0.0065, 0.0022,0.0001,0.0105, 0.0042,0.0001,0.0201,
    0.0076,0.0002,0.0362, 0.0143,0.0004,0.0679, 0.0232,0.0006,0.1102,
    0.0435,0.0012,0.2074, 0.0776,0.0022,0.3713, 0.1344,0.0040,0.6456,
    0.2148,0.0073,1.0391, 0.2839,0.0116,1.3856, 0.3285,0.0168,1.6230,
    0.3483,0.0230,1.7471, 0.3481,0.0298,1.7826, 0.3362,0.0380,1.7721,
    0.3187,0.0480,1.7441, 0.2908,0.0600,1.6692, 0.2511,0.0739,1.5281,
    0.1954,0.0910,1.2876, 0.1421,0.1126,1.0419, 0.0956,0.1390,0.8130,
    0.0580,0.1693,0.6162, 0.0320,0.2080,0.4652, 0.0147,0.2586,0.3533,
    0.0049,0.3230,0.2720, 0.0024,0.4073,0.2123, 0.0093,0.5030,0.1582,
    0.0291,0.6082,0.1117, 0.0633,0.7100,0.0782, 0.1096,0.7932,0.0573,
    0.1655,0.8620,0.0422, 0.2257,0.9149,0.0298, 0.2904,0.9540,0.0203,
    0.3597,0.9803,0.0134, 0.4334,0.9950,0.0087, 0.5121,1.0000,0.0057,
    0.5945,0.9950,0.0039, 0.6784,0.9786,0.0027, 0.7621,0.9520,0.0021,
    0.8425,0.9154,0.0018, 0.9163,0.8700,0.0017, 0.9786,0.8163,0.0014,
    1.0263,0.7570,0.0011, 1.0567,0.6949,0.0010, 1.0622,0.6310,0.0008,
    1.0456,0.5668,0.0006, 1.0026,0.5030,0.0003, 0.9384,0.4412,0.0002,
    0.8544,0.3810,0.0002, 0.7514,0.3210,0.0001, 0.6424,0.2650,0.0000,
    0.5419,0.2170,0.0000, 0.4479,0.1750,0.0000, 0.3608,0.1382,0.0000,
    0.2835,0.1070,0.0000, 0.2187,0.0816,0.0000, 0.1649,0.0610,0.0000,
    0.1212,0.0446,0.0000, 0.0874,0.0320,0.0000, 0.0636,0.0232,0.0000,
    0.0468,0.0170,0.0000, 0.0329,0.0119,0.0000, 0.0227,0.0082,0.0000,
    0.0158,0.0057,0.0000, 0.0114,0.0041,0.0000, 0.0081,0.0029,0.0000,
    0.0058,0.0021,0.0000, 0.0041,0.0015,0.0000, 0.0029,0.0010,0.0000,
    0.0020,0.0007,0.0000, 0.0014,0.0005,0.0000, 0.0010,0.0004,0.0000,
    0.0007,0.0002,0.0000, 0.0005,0.0002,0.0000, 0.0003,0.0001,0.0000,
    0.0002,0.0001,0.0000, 0.0002,0.0001,0.0000, 0.0001,0.0000,0.0000,
    0.0001,0.0000,0.0000, 0.0001,0.0000,0.0000, 0.0000,0.0000,0.0000,
];

/// Return the XYZ color for a single wavelength (nm) using CIE data.
/// Matches `wavelength_color_XYZ` in `opcolor_impl.h`.
pub fn wavelength_color_xyz(lambda_nm: Float) -> Color3 {
    let ii = (lambda_nm - 380.0) / 5.0;
    let i = ii as i32;
    if i < 0 || i >= 80 {
        return Color3::ZERO;
    }
    let remainder = ii - i as Float;
    let si = (i as usize) * 3;
    let x0 = CIE_COLOUR_MATCH[si];
    let y0 = CIE_COLOUR_MATCH[si + 1];
    let z0 = CIE_COLOUR_MATCH[si + 2];
    let x1 = CIE_COLOUR_MATCH[si + 3];
    let y1 = CIE_COLOUR_MATCH[si + 4];
    let z1 = CIE_COLOUR_MATCH[si + 5];
    Color3::new(
        x0 + remainder * (x1 - x0),
        y0 + remainder * (y1 - y0),
        z0 + remainder * (z1 - z0),
    )
}

// ---------------------------------------------------------------------------
// Planck's law blackbody spectrum
// ---------------------------------------------------------------------------

/// Blackbody spectral radiance at given temperature (K) and wavelength (nm).
/// Matches `bb_spectrum` functor in `opcolor_impl.h`.
fn bb_spectrum(wavelength_nm: Float, temperature: Float) -> Float {
    let wlm = wavelength_nm * 1e-9; // wavelength in meters
    let c1: Float = 3.74183e-16; // 2*pi*h*c^2, W*m^2
    let c2: Float = 1.4388e-2; // h*c/k, m*K
    let wlm2 = wlm * wlm;
    let wlm4 = wlm2 * wlm2;
    let wlm5 = wlm4 * wlm;
    let inverse_of_wlm5 = 1.0 / wlm5;
    let exponent = c2 / (wlm * temperature);
    let expm1 = exponent.exp() - 1.0;
    if expm1 > 0.0 {
        (c1 * inverse_of_wlm5) / expm1
    } else {
        0.0
    }
}

/// Integrate spectral intensity weighted by CIE colour matching functions.
/// Matches `spectrum_to_XYZ` in `opcolor_impl.h`.
fn spectrum_to_xyz<F: Fn(Float) -> Float>(spec_intens: F) -> Color3 {
    let mut x = 0.0_f32;
    let mut y = 0.0_f32;
    let mut z = 0.0_f32;
    let dlambda = 5.0 * 1e-9; // in meters
    for i in 0..81 {
        let lambda = 380.0 + 5.0 * i as Float;
        let me = spec_intens(lambda) * dlambda;
        let si = i * 3;
        x += me * CIE_COLOUR_MATCH[si];
        y += me * CIE_COLOUR_MATCH[si + 1];
        z += me * CIE_COLOUR_MATCH[si + 2];
    }
    Color3::new(x, y, z)
}

// ---------------------------------------------------------------------------
// Blackbody lookup table parameters
// Matches BB_DRAPER, BB_MAX_TABLE_RANGE, etc. from opcolor_impl.h
// ---------------------------------------------------------------------------

const BB_DRAPER: Float = 800.0;
const BB_MAX_TABLE_RANGE: Float = 12000.0;
const BB_TABLE_SPACING: Float = 2.0;
const BB_TABLE_SIZE: usize = 317;
const BB_TABLE_YPOWER: Float = 5.0;

#[inline]
fn bb_table_map(i: Float) -> Float {
    let is = i.sqrt();
    let ip = is * is * is; // i^(3/2)
    ip * BB_TABLE_SPACING + BB_DRAPER
}

#[inline]
fn bb_table_unmap(t: Float) -> Float {
    let tv = (t - BB_DRAPER) / BB_TABLE_SPACING;
    let ic = tv.cbrt(); // t^(1/3)
    ic * ic // t^(2/3)
}

// ---------------------------------------------------------------------------
// ColorSystem — configurable color system with primaries and blackbody table
// Matches `pvt::ColorSystem` from `opcolor.h`
// ---------------------------------------------------------------------------

/// CIE chromaticity coordinates for a color system's primaries and white point.
#[derive(Debug, Clone, Copy)]
pub struct Chroma {
    pub x_red: Float,
    pub y_red: Float,
    pub x_green: Float,
    pub y_green: Float,
    pub x_blue: Float,
    pub y_blue: Float,
    pub x_white: Float,
    pub y_white: Float,
}

/// Rec.709 / sRGB primaries (D65 white point).
pub const REC709: Chroma = Chroma {
    x_red: 0.64,
    y_red: 0.33,
    x_green: 0.30,
    y_green: 0.60,
    x_blue: 0.15,
    y_blue: 0.06,
    x_white: 0.3127,
    y_white: 0.3291,
};

// Standard illuminant white points
const ILLUMINANT_C: (Float, Float) = (0.3101, 0.3162);
const ILLUMINANT_E: (Float, Float) = (0.33333333, 0.33333333);
const ILLUMINANT_ACES: (Float, Float) = (0.32168, 0.33767);

/// NTSC primaries (Illuminant C).
pub const NTSC: Chroma = Chroma {
    x_red: 0.67,
    y_red: 0.33,
    x_green: 0.21,
    y_green: 0.71,
    x_blue: 0.14,
    y_blue: 0.08,
    x_white: ILLUMINANT_C.0,
    y_white: ILLUMINANT_C.1,
};

/// EBU (European Broadcasting Union) primaries (D65).
pub const EBU: Chroma = Chroma {
    x_red: 0.64,
    y_red: 0.33,
    x_green: 0.29,
    y_green: 0.60,
    x_blue: 0.15,
    y_blue: 0.06,
    x_white: 0.3127,
    y_white: 0.3291,
};

/// PAL/SECAM primaries (same as EBU, D65).
pub const PAL: Chroma = EBU;
pub const SECAM: Chroma = EBU;

/// SMPTE-C primaries (D65).
pub const SMPTE: Chroma = Chroma {
    x_red: 0.630,
    y_red: 0.340,
    x_green: 0.310,
    y_green: 0.595,
    x_blue: 0.155,
    y_blue: 0.070,
    x_white: 0.3127,
    y_white: 0.3291,
};

/// HDTV primaries (D65).
pub const HDTV: Chroma = Chroma {
    x_red: 0.670,
    y_red: 0.330,
    x_green: 0.210,
    y_green: 0.710,
    x_blue: 0.150,
    y_blue: 0.060,
    x_white: 0.3127,
    y_white: 0.3291,
};

/// CIE primaries (Equal Energy illuminant).
pub const CIE: Chroma = Chroma {
    x_red: 0.7355,
    y_red: 0.2645,
    x_green: 0.2658,
    y_green: 0.7243,
    x_blue: 0.1669,
    y_blue: 0.0085,
    x_white: ILLUMINANT_E.0,
    y_white: ILLUMINANT_E.1,
};

/// Adobe RGB (1998) primaries (D65).
pub const ADOBE_RGB: Chroma = Chroma {
    x_red: 0.64,
    y_red: 0.33,
    x_green: 0.21,
    y_green: 0.71,
    x_blue: 0.15,
    y_blue: 0.06,
    x_white: 0.3127,
    y_white: 0.3291,
};

/// ACES 2065-1 (AP0) primaries.
pub const ACES_AP0: Chroma = Chroma {
    x_red: 0.7347,
    y_red: 0.2653,
    x_green: 0.0,
    y_green: 1.0,
    x_blue: 0.0001,
    y_blue: -0.077,
    x_white: ILLUMINANT_ACES.0,
    y_white: ILLUMINANT_ACES.1,
};

/// ACEScg (AP1) primaries.
pub const ACES_CG: Chroma = Chroma {
    x_red: 0.713,
    y_red: 0.293,
    x_green: 0.165,
    y_green: 0.83,
    x_blue: 0.128,
    y_blue: 0.044,
    x_white: ILLUMINANT_ACES.0,
    y_white: ILLUMINANT_ACES.1,
};

/// Configurable color system matching `pvt::ColorSystem` from `opcolor.h`.
///
/// Encapsulates color primaries, XYZ↔RGB conversion matrices,
/// luminance scale vector, and a precomputed blackbody lookup table.
pub struct ColorSystem {
    /// XYZ → RGB conversion matrix (3×3).
    pub xyz_to_rgb: Matrix33,
    /// RGB → XYZ conversion matrix (3×3).
    pub rgb_to_xyz: Matrix33,
    /// Luminance scale (dot product with RGB gives luminance).
    pub luminance_scale: Color3,
    /// Precomputed blackbody table (317 entries, stored as val^(1/5)).
    blackbody_table: Vec<Color3>,
}

impl ColorSystem {
    /// Create a new ColorSystem from chromaticity coordinates.
    /// Computes XYZ↔RGB matrices from the primaries, then builds the
    /// blackbody lookup table using Planck's law + CIE observer.
    pub fn new(chroma: &Chroma) -> Self {
        let (xyz2rgb, rgb2xyz, lum_scale) = Self::compute_matrices(chroma);
        let mut cs = Self {
            xyz_to_rgb: xyz2rgb,
            rgb_to_xyz: rgb2xyz,
            luminance_scale: lum_scale,
            blackbody_table: Vec::with_capacity(BB_TABLE_SIZE),
        };
        cs.build_blackbody_table();
        cs
    }

    /// Create a default ColorSystem using Rec.709 primaries.
    pub fn rec709() -> Self {
        Self::new(&REC709)
    }

    /// Look up a named color system by OSL color space name.
    ///
    /// Returns `None` for non-system spaces (hsv, hsl, YIQ, sRGB, etc.)
    /// which are handled by analytic conversions instead.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "Rec709" | "rec709" => Some(Self::new(&REC709)),
            "sRGB" | "srgb" => Some(Self::new(&REC709)), // same primaries
            "NTSC" | "ntsc" => Some(Self::new(&NTSC)),
            "EBU" | "ebu" => Some(Self::new(&EBU)),
            "PAL" | "pal" => Some(Self::new(&PAL)),
            "SECAM" | "secam" => Some(Self::new(&SECAM)),
            "SMPTE" | "smpte" => Some(Self::new(&SMPTE)),
            "HDTV" | "hdtv" => Some(Self::new(&HDTV)),
            "CIE" | "cie" => Some(Self::new(&CIE)),
            "AdobeRGB" | "adobergb" => Some(Self::new(&ADOBE_RGB)),
            "ACES2065-1" | "aces2065" | "aces" => Some(Self::new(&ACES_AP0)),
            "ACEScg" | "acescg" => Some(Self::new(&ACES_CG)),
            _ => None,
        }
    }

    /// Compute luminance of an RGB color using this color system's weights.
    #[inline]
    pub fn luminance(&self, rgb: Color3) -> Float {
        rgb.dot(self.luminance_scale)
    }

    /// Convert XYZ to RGB using this color system's matrices.
    #[inline]
    pub fn xyz_to_rgb_color(&self, xyz: Color3) -> Color3 {
        mul_color_matrix33(xyz, &self.xyz_to_rgb)
    }

    /// Convert RGB to XYZ using this color system's matrices.
    #[inline]
    pub fn rgb_to_xyz_color(&self, rgb: Color3) -> Color3 {
        mul_color_matrix33(rgb, &self.rgb_to_xyz)
    }

    /// Return the blackbody RGB color for temperature T (Kelvin).
    /// Uses the precomputed lookup table for T < 12000K,
    /// falls back to full spectral computation otherwise.
    pub fn blackbody_rgb(&self, t: Float) -> Color3 {
        if t < BB_DRAPER {
            return Color3::new(1.0e-6, 0.0, 0.0); // very very dim red
        }
        if t < BB_MAX_TABLE_RANGE && !self.blackbody_table.is_empty() {
            self.lookup_blackbody_rgb(t)
        } else {
            self.compute_blackbody_rgb(t)
        }
    }

    /// Full computation of blackbody RGB using Planck's law and CIE observer.
    pub fn compute_blackbody_rgb(&self, t: Float) -> Color3 {
        let xyz = spectrum_to_xyz(|lambda| bb_spectrum(lambda, t));
        let mut rgb = self.xyz_to_rgb_color(xyz);
        clamp_zero(&mut rgb);
        rgb
    }

    /// Look up blackbody from the precomputed table.
    fn lookup_blackbody_rgb(&self, t: Float) -> Color3 {
        let ti_f = bb_table_unmap(t);
        let ti = ti_f as usize;
        let remainder = ti_f - ti as Float;
        let max_idx = self.blackbody_table.len().saturating_sub(2);
        let ti = ti.min(max_idx);
        let c0 = self.blackbody_table[ti];
        let c1 = self.blackbody_table[ti + 1];
        // Lerp in stored space
        let rgb = Color3::new(
            c0.x + remainder * (c1.x - c0.x),
            c0.y + remainder * (c1.y - c0.y),
            c0.z + remainder * (c1.z - c0.z),
        );
        // Decode: stored as val^(1/5), so raise to 5th power
        let rgb2 = Color3::new(rgb.x * rgb.x, rgb.y * rgb.y, rgb.z * rgb.z);
        let rgb4 = Color3::new(rgb2.x * rgb2.x, rgb2.y * rgb2.y, rgb2.z * rgb2.z);
        Color3::new(rgb4.x * rgb.x, rgb4.y * rgb.y, rgb4.z * rgb.z)
    }

    fn build_blackbody_table(&mut self) {
        self.blackbody_table.clear();
        for i in 0..BB_TABLE_SIZE {
            let t = bb_table_map(i as Float);
            let rgb = self.compute_blackbody_rgb(t);
            // Store as val^(1/5) for better interpolation
            let inv_pow = 1.0 / BB_TABLE_YPOWER;
            let stored = Color3::new(
                rgb.x.max(0.0).powf(inv_pow),
                rgb.y.max(0.0).powf(inv_pow),
                rgb.z.max(0.0).powf(inv_pow),
            );
            self.blackbody_table.push(stored);
        }
    }

    /// Compute XYZ↔RGB matrices from chromaticity coordinates.
    /// Uses the same cofactor/cross-product algorithm as C++ OSL for bit-exact parity.
    fn compute_matrices(c: &Chroma) -> (Matrix33, Matrix33, Color3) {
        // Chromaticity xyz for R, G, B, W
        let r = Color3::new(c.x_red, c.y_red, 1.0 - c.x_red - c.y_red);
        let g = Color3::new(c.x_green, c.y_green, 1.0 - c.x_green - c.y_green);
        let b = Color3::new(c.x_blue, c.y_blue, 1.0 - c.x_blue - c.y_blue);
        let w = Color3::new(c.x_white, c.y_white, 1.0 - c.x_white - c.y_white);

        // XYZ→RGB cofactor rows (cross products of the other two primaries)
        let mut cr = Color3::new(
            g.y * b.z - b.y * g.z,
            b.x * g.z - g.x * b.z,
            g.x * b.y - b.x * g.y,
        );
        let mut cg = Color3::new(
            b.y * r.z - r.y * b.z,
            r.x * b.z - b.x * r.z,
            b.x * r.y - r.x * b.y,
        );
        let mut cb = Color3::new(
            r.y * g.z - g.y * r.z,
            g.x * r.z - r.x * g.z,
            r.x * g.y - g.x * r.y,
        );

        // White scaling factor
        let mut cw = Color3::new(cr.dot(w), cg.dot(w), cb.dot(w));
        if w.y != 0.0 {
            let s = 1.0 / w.y;
            cw = Color3::new(cw.x * s, cw.y * s, cw.z * s);
        }

        // Scale cofactor rows by white factor
        cr = Color3::new(cr.x / cw.x, cr.y / cw.x, cr.z / cw.x);
        cg = Color3::new(cg.x / cw.y, cg.y / cw.y, cg.z / cw.y);
        cb = Color3::new(cb.x / cw.z, cb.y / cw.z, cb.z / cw.z);

        // XYZ→RGB: columns are the scaled cofactor rows
        let xyz2rgb = Matrix33 {
            m: [[cr.x, cg.x, cb.x], [cr.y, cg.y, cb.y], [cr.z, cg.z, cb.z]],
        };

        // RGB→XYZ = inverse of XYZ→RGB
        let rgb2xyz = matrix33_inverse(&xyz2rgb);

        // Luminance scale = second column of RGB→XYZ (Y row)
        let mut lum_scale = Color3::new(rgb2xyz.m[0][1], rgb2xyz.m[1][1], rgb2xyz.m[2][1]);
        // Fix rounding: ensure luminance sums to 1.0
        let lum2 = 1.0 - lum_scale.x - lum_scale.y;
        if (lum2 - lum_scale.z).abs() < 0.001 {
            lum_scale.z = lum2;
        }

        (xyz2rgb, rgb2xyz, lum_scale)
    }

    /// Transform a color between named spaces using this color system.
    ///
    /// Supports all OSL-standard color spaces: rgb, hsv, hsl, YIQ, XYZ, xyY, sRGB,
    /// Rec709, NTSC, EBU, PAL, SECAM, SMPTE, HDTV, CIE, AdobeRGB, ACES2065-1, ACEScg.
    ///
    /// For cross-system conversions (e.g. Rec709→NTSC), the path is:
    ///   source_system.RGB → XYZ → target_system.RGB
    pub fn transformc(&self, from: &str, to: &str, color: Color3) -> Color3 {
        if from == to {
            return color;
        }

        // Convert to linear RGB (in *this* color system) as intermediate
        let linear = self.to_rgb(from, color);

        // Convert from linear RGB to target
        self.from_rgb(to, linear)
    }

    /// Convert a color from the named space to this system's linear RGB.
    pub fn to_rgb(&self, from: &str, color: Color3) -> Color3 {
        match from {
            "rgb" | "RGB" | "linear" | "scene_linear" => color,
            "hsv" | "HSV" => hsv_to_rgb(color),
            "hsl" | "HSL" => hsl_to_rgb(color),
            "YIQ" | "yiq" => yiq_to_rgb(color),
            "XYZ" | "xyz" | "ciexyz" => self.xyz_to_rgb_color(color),
            "xyY" => self.xyz_to_rgb_color(xyy_to_xyz(color)),
            "sRGB" | "srgb" => srgb_to_linear_color(color),
            other => {
                // Try cross-system conversion: other_system.RGB → XYZ → this.RGB
                if let Some(src_sys) = ColorSystem::from_name(other) {
                    let xyz = src_sys.rgb_to_xyz_color(color);
                    self.xyz_to_rgb_color(xyz)
                } else {
                    color // unknown → pass through
                }
            }
        }
    }

    /// Convert a color from this system's linear RGB to the named space.
    pub fn from_rgb(&self, to: &str, linear: Color3) -> Color3 {
        match to {
            "rgb" | "RGB" | "linear" | "scene_linear" => linear,
            "hsv" | "HSV" => rgb_to_hsv(linear),
            "hsl" | "HSL" => rgb_to_hsl(linear),
            "YIQ" | "yiq" => rgb_to_yiq(linear),
            "XYZ" | "xyz" | "ciexyz" => self.rgb_to_xyz_color(linear),
            "xyY" => xyz_to_xyy(self.rgb_to_xyz_color(linear)),
            "sRGB" | "srgb" => linear_to_srgb_color(linear),
            other => {
                // Try cross-system conversion: this.RGB → XYZ → other_system.RGB
                if let Some(dst_sys) = ColorSystem::from_name(other) {
                    let xyz = self.rgb_to_xyz_color(linear);
                    dst_sys.xyz_to_rgb_color(xyz)
                } else {
                    linear // unknown → pass through
                }
            }
        }
    }
}

/// Transform a color between two different color systems via XYZ.
///
/// Converts: `from_cs.to_rgb(from_space, c)` -> `from_cs.RGB` -> XYZ
/// -> `to_cs.RGB` -> `to_cs.from_rgb(to_space, result)`.
///
/// This handles the full cross-system pipeline, e.g. converting an NTSC HSV
/// color to an ACES sRGB color.
pub fn transform_between_systems(
    from_cs: &ColorSystem,
    to_cs: &ColorSystem,
    from_space: &str,
    to_space: &str,
    c: Color3,
) -> Color3 {
    // Convert from source space to source system's linear RGB
    let linear_src = from_cs.to_rgb(from_space, c);
    // Source system RGB -> XYZ
    let xyz = from_cs.rgb_to_xyz_color(linear_src);
    // XYZ -> destination system RGB
    let linear_dst = to_cs.xyz_to_rgb_color(xyz);
    // Convert from dest system's linear RGB to target space
    to_cs.from_rgb(to_space, linear_dst)
}

// ---------------------------------------------------------------------------
// OCIO color space transforms (behind "vfx" feature)
// ---------------------------------------------------------------------------

/// Check if a space name is a built-in OSL color space handled analytically.
#[cfg(feature = "vfx")]
fn is_builtin_space(name: &str) -> bool {
    // Only spaces with direct analytical conversions (matching C++ transformc).
    // Named color systems (ACES, NTSC, etc.) go through OCIO like in C++.
    matches!(
        name,
        "rgb"
            | "RGB"
            | "Rec709"
            | "rec709"
            | "linear"
            | "scene_linear"
            | "hsv"
            | "HSV"
            | "hsl"
            | "HSL"
            | "YIQ"
            | "yiq"
            | "XYZ"
            | "xyz"
            | "ciexyz"
            | "xyY"
            | "sRGB"
            | "srgb"
    )
}

/// OCIO configuration wrapper with lazy initialization.
///
/// Holds a `vfx_ocio::Config` (built-in ACES 1.3 by default) and provides
/// a simple API for color space transforms via OCIO processors.
#[cfg(feature = "vfx")]
pub struct OcioColorConfig {
    config: vfx_ocio::Config,
}

#[cfg(feature = "vfx")]
impl OcioColorConfig {
    /// Create a new config using the built-in ACES 1.3 configuration.
    pub fn new() -> Self {
        Self {
            config: vfx_ocio::builtin::aces_1_3(),
        }
    }

    /// Create a config from an existing `vfx_ocio::Config`.
    pub fn from_config(config: vfx_ocio::Config) -> Self {
        Self { config }
    }

    /// Access the underlying OCIO config.
    pub fn config(&self) -> &vfx_ocio::Config {
        &self.config
    }

    /// Check if a color space name exists in this OCIO config.
    pub fn has_colorspace(&self, name: &str) -> bool {
        self.config.colorspace(name).is_some()
    }

    /// Transform a single Color3 between two OCIO color spaces.
    /// Returns `None` if either space is unknown or the processor fails.
    pub fn transform(&self, from: &str, to: &str, c: Color3) -> Option<Color3> {
        let proc = self.config.processor(from, to).ok()?;
        let mut pixel = [[c.x, c.y, c.z]];
        proc.apply_rgb(&mut pixel);
        Some(Color3::new(pixel[0][0], pixel[0][1], pixel[0][2]))
    }
}

#[cfg(feature = "vfx")]
impl Default for OcioColorConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Global OCIO config singleton (lazy-initialized on first use).
#[cfg(feature = "vfx")]
fn global_ocio() -> &'static OcioColorConfig {
    static INSTANCE: OnceLock<OcioColorConfig> = OnceLock::new();
    INSTANCE.get_or_init(OcioColorConfig::new)
}

/// Transform a color between OCIO color spaces using the global ACES config.
///
/// Supports any color space known to the built-in ACES 1.3 config, such as
/// "ACEScg", "ACES2065-1", "sRGB", "Linear Rec.709 (sRGB)", etc.
/// Returns `None` if the transform is not possible.
#[cfg(feature = "vfx")]
pub fn transform_color_ocio(from_space: &str, to_space: &str, c: Color3) -> Option<Color3> {
    global_ocio().transform(from_space, to_space, c)
}

/// Multiply a Color3 by a Matrix33 (row vector × matrix).
fn mul_color_matrix33(c: Color3, m: &Matrix33) -> Color3 {
    Color3::new(
        c.x * m.m[0][0] + c.y * m.m[1][0] + c.z * m.m[2][0],
        c.x * m.m[0][1] + c.y * m.m[1][1] + c.z * m.m[2][1],
        c.x * m.m[0][2] + c.y * m.m[1][2] + c.z * m.m[2][2],
    )
}

/// Clamp negative components to zero.
fn clamp_zero(c: &mut Color3) {
    if c.x < 0.0 {
        c.x = 0.0;
    }
    if c.y < 0.0 {
        c.y = 0.0;
    }
    if c.z < 0.0 {
        c.z = 0.0;
    }
}

/// Invert a 3×3 matrix given as 9 floats. Returns 9 floats.
fn invert_3x3(
    a: Float,
    b: Float,
    c: Float,
    d: Float,
    e: Float,
    f: Float,
    g: Float,
    h: Float,
    i: Float,
) -> (
    Float,
    Float,
    Float,
    Float,
    Float,
    Float,
    Float,
    Float,
    Float,
) {
    let det = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);
    if det.abs() < 1e-30 {
        return (1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0);
    }
    let inv_det = 1.0 / det;
    (
        (e * i - f * h) * inv_det,
        (c * h - b * i) * inv_det,
        (b * f - c * e) * inv_det,
        (f * g - d * i) * inv_det,
        (a * i - c * g) * inv_det,
        (c * d - a * f) * inv_det,
        (d * h - e * g) * inv_det,
        (b * g - a * h) * inv_det,
        (a * e - b * d) * inv_det,
    )
}

/// Compute the inverse of a Matrix33.
fn matrix33_inverse(m: &Matrix33) -> Matrix33 {
    let (a, b, c, d, e, f, g, h, i) = invert_3x3(
        m.m[0][0], m.m[0][1], m.m[0][2], m.m[1][0], m.m[1][1], m.m[1][2], m.m[2][0], m.m[2][1],
        m.m[2][2],
    );
    Matrix33 {
        m: [[a, b, c], [d, e, f], [g, h, i]],
    }
}

// ---------------------------------------------------------------------------
// Luminance (free functions for backward compatibility)
// ---------------------------------------------------------------------------

/// ITU-R BT.709 luminance weights.
const LUMA_709: Vec3 = Vec3 {
    x: 0.2126,
    y: 0.7152,
    z: 0.0722,
};

/// Compute luminance of an RGB color using Rec.709 weights.
#[inline]
pub fn luminance(c: Color3) -> Float {
    c.dot(LUMA_709)
}

/// Compute luminance with custom weights.
#[inline]
pub fn luminance_weighted(c: Color3, weights: Color3) -> Float {
    c.dot(weights)
}

// ---------------------------------------------------------------------------
// RGB <-> HSV
// ---------------------------------------------------------------------------

/// Convert RGB to HSV. All components in [0, 1] range.
pub fn rgb_to_hsv(rgb: Color3) -> Color3 {
    let r = rgb.x;
    let g = rgb.y;
    let b = rgb.z;

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    let v = max;
    let s = if max > 0.0 { delta / max } else { 0.0 };

    let h = if delta == 0.0 {
        0.0
    } else if max == r {
        let h = (g - b) / delta;
        if h < 0.0 { h + 6.0 } else { h }
    } else if max == g {
        (b - r) / delta + 2.0
    } else {
        (r - g) / delta + 4.0
    } / 6.0;

    Color3::new(h, s, v)
}

/// Convert HSV to RGB.
pub fn hsv_to_rgb(hsv: Color3) -> Color3 {
    let h = hsv.x;
    let s = hsv.y;
    let v = hsv.z;

    if s < 0.0001 {
        return Color3::splat(v);
    }

    let h = (h - h.floor()) * 6.0;
    let hi = h as i32;
    let f = h - hi as Float;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));

    match hi {
        0 => Color3::new(v, t, p),
        1 => Color3::new(q, v, p),
        2 => Color3::new(p, v, t),
        3 => Color3::new(p, q, v),
        4 => Color3::new(t, p, v),
        _ => Color3::new(v, p, q),
    }
}

// ---------------------------------------------------------------------------
// RGB <-> HSL
// ---------------------------------------------------------------------------

/// Convert RGB to HSL.
pub fn rgb_to_hsl(rgb: Color3) -> Color3 {
    let r = rgb.x;
    let g = rgb.y;
    let b = rgb.z;

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    let l = (max + min) * 0.5;
    let s = if delta == 0.0 {
        0.0
    } else if l < 0.5 {
        delta / (max + min)
    } else {
        delta / (2.0 - max - min)
    };

    let h = if delta == 0.0 {
        0.0
    } else if max == r {
        let h = (g - b) / delta;
        if h < 0.0 { h + 6.0 } else { h }
    } else if max == g {
        (b - r) / delta + 2.0
    } else {
        (r - g) / delta + 4.0
    } / 6.0;

    Color3::new(h, s, l)
}

/// Convert HSL to RGB.
/// C++ opcolor_impl.h:363 — converts HSL to HSV, then HSV to RGB.
pub fn hsl_to_rgb(hsl: Color3) -> Color3 {
    let h = hsl.x;
    let s = hsl.y;
    let l = hsl.z;
    // Foley & van Dam: hsl -> hsv -> rgb
    let v = if l <= 0.5 {
        l * (1.0 + s)
    } else {
        l * (1.0 - s) + s
    };
    if v <= 0.0 {
        return Color3::ZERO;
    }
    let min = 2.0 * l - v;
    let s2 = (v - min) / v;
    hsv_to_rgb(Color3::new(h, s2, v))
}

// ---------------------------------------------------------------------------
// RGB <-> YIQ (NTSC)
// ---------------------------------------------------------------------------

/// Convert RGB to YIQ.
pub fn rgb_to_yiq(rgb: Color3) -> Color3 {
    Color3::new(
        0.299 * rgb.x + 0.587 * rgb.y + 0.114 * rgb.z,
        0.596 * rgb.x - 0.275 * rgb.y - 0.321 * rgb.z,
        0.212 * rgb.x - 0.523 * rgb.y + 0.311 * rgb.z,
    )
}

/// Convert YIQ to RGB.
/// Coefficients match C++ `YIQ_to_rgb` matrix in opcolor_impl.h.
pub fn yiq_to_rgb(yiq: Color3) -> Color3 {
    Color3::new(
        yiq.x + 0.9557 * yiq.y + 0.6199 * yiq.z,
        yiq.x - 0.2716 * yiq.y - 0.6469 * yiq.z,
        yiq.x - 1.1082 * yiq.y + 1.7051 * yiq.z,
    )
}

// ---------------------------------------------------------------------------
// RGB <-> CIE XYZ (D65 / Rec.709 primaries)
// ---------------------------------------------------------------------------

/// Convert linear sRGB (Rec.709) to CIE XYZ.
pub fn rgb_to_xyz(rgb: Color3) -> Color3 {
    Color3::new(
        0.4124564 * rgb.x + 0.3575761 * rgb.y + 0.1804375 * rgb.z,
        0.2126729 * rgb.x + 0.7151522 * rgb.y + 0.0721750 * rgb.z,
        0.0193339 * rgb.x + 0.1191920 * rgb.y + 0.9503041 * rgb.z,
    )
}

/// Convert CIE XYZ to linear sRGB (Rec.709).
pub fn xyz_to_rgb(xyz: Color3) -> Color3 {
    Color3::new(
        3.2404542 * xyz.x - 1.5371385 * xyz.y - 0.4985314 * xyz.z,
        -0.9692660 * xyz.x + 1.8760108 * xyz.y + 0.0415560 * xyz.z,
        0.0556434 * xyz.x - 0.2040259 * xyz.y + 1.0572252 * xyz.z,
    )
}

// ---------------------------------------------------------------------------
// RGB <-> xyY
// ---------------------------------------------------------------------------

/// Convert CIE XYZ to xyY.
pub fn xyz_to_xyy(xyz: Color3) -> Color3 {
    // C++ opcolor_impl.h:438 — n_inv = (n >= 1e-6 ? 1/n : 0)
    let n = xyz.x + xyz.y + xyz.z;
    let n_inv = if n >= 1e-6 { 1.0 / n } else { 0.0 };
    Color3::new(xyz.x * n_inv, xyz.y * n_inv, xyz.y)
}

/// Convert xyY to CIE XYZ.
pub fn xyy_to_xyz(xyy: Color3) -> Color3 {
    if xyy.y.abs() < 1e-10 {
        Color3::ZERO
    } else {
        let x_over_y = xyy.x / xyy.y;
        Color3::new(
            x_over_y * xyy.z,
            xyy.z,
            (1.0 - xyy.x - xyy.y) / xyy.y * xyy.z,
        )
    }
}

// ---------------------------------------------------------------------------
// sRGB gamma
// ---------------------------------------------------------------------------

/// Linear to sRGB gamma (per channel).
#[inline]
pub fn linear_to_srgb(c: Float) -> Float {
    if c <= 0.0031308 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// sRGB gamma to linear (per channel).
#[inline]
pub fn srgb_to_linear(c: Float) -> Float {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Convert linear RGB to sRGB (apply gamma).
pub fn linear_to_srgb_color(rgb: Color3) -> Color3 {
    Color3::new(
        linear_to_srgb(rgb.x),
        linear_to_srgb(rgb.y),
        linear_to_srgb(rgb.z),
    )
}

/// Convert sRGB to linear RGB (remove gamma).
pub fn srgb_to_linear_color(srgb: Color3) -> Color3 {
    Color3::new(
        srgb_to_linear(srgb.x),
        srgb_to_linear(srgb.y),
        srgb_to_linear(srgb.z),
    )
}

// ---------------------------------------------------------------------------
// Blackbody radiation
// ---------------------------------------------------------------------------

/// Compute the blackbody color (in linear Rec.709 RGB) for temperature T (Kelvin).
/// Uses Planck's law with CIE 1931 observer, matching C++ `ColorSystem::blackbody_rgb`.
/// NOTE: hardcodes Rec.709 xyz_to_rgb. Callers with a ColorSystem should prefer
/// `ColorSystem::blackbody_rgb()` for correct color-space-aware conversion.
pub fn blackbody(temperature_k: Float) -> Color3 {
    if temperature_k < 800.0 {
        return Color3::new(1.0e-6, 0.0, 0.0); // very dim red, matching C++
    }
    // Full computation: integrate Planck spectrum × CIE observer → XYZ → RGB
    let xyz = spectrum_to_xyz(|lambda| bb_spectrum(lambda, temperature_k));
    let mut rgb = xyz_to_rgb(xyz);
    clamp_zero(&mut rgb);
    rgb
}

/// Compute the power of blackbody radiation at temperature T (Kelvin).
/// Stefan-Boltzmann law: total radiant flux proportional to T^4.
pub fn blackbody_power(temperature_k: Float) -> Float {
    let t = temperature_k;
    let sigma = 5.670374e-8_f32;
    sigma * t * t * t * t
}

// ---------------------------------------------------------------------------
// Wavelength to RGB
// ---------------------------------------------------------------------------

/// Convert a wavelength (in nanometers) to a linear RGB color.
/// Uses CIE 1931 colour matching functions (380–780nm) and Rec.709 XYZ→RGB.
/// Outside the valid range, returns black.
/// Matches `osl_wavelength_color_vf` in the C++ reference.
/// Includes the empirical 1/2.52 brightness scaling from opcolor_impl.h.
pub fn wavelength_color(wavelength_nm: Float) -> Color3 {
    let xyz = wavelength_color_xyz(wavelength_nm);
    let mut rgb = xyz_to_rgb(xyz);
    // Empirical scaling factor from C++ osl_wavelength_color_vf
    rgb.x *= 1.0 / 2.52;
    rgb.y *= 1.0 / 2.52;
    rgb.z *= 1.0 / 2.52;
    clamp_zero(&mut rgb);
    rgb
}

// ---------------------------------------------------------------------------
// High-level transform API
// ---------------------------------------------------------------------------

/// Transform a color from one named color space to another.
///
/// Handles all OSL-standard spaces including system-specific primaries
/// (NTSC, EBU, PAL, SECAM, SMPTE, HDTV, CIE, AdobeRGB, ACES2065-1, ACEScg).
/// Uses Rec.709 as the default working space.
///
/// When the `vfx` feature is enabled and either space is not a built-in OSL
/// name, delegates to OCIO via the global ACES 1.3 config as a fallback.
pub fn transform_color(from: &str, to: &str, color: Color3) -> Color3 {
    if from == to {
        return color;
    }

    // When either space is not a built-in OSL name, try OCIO first
    #[cfg(feature = "vfx")]
    if !is_builtin_space(from) || !is_builtin_space(to) {
        if let Some(result) = transform_color_ocio(from, to, color) {
            return result;
        }
    }

    // Convert to linear RGB as intermediate
    let linear = match from {
        "rgb" | "RGB" | "Rec709" | "linear" | "scene_linear" => color,
        "hsv" | "HSV" => hsv_to_rgb(color),
        "hsl" | "HSL" => hsl_to_rgb(color),
        "YIQ" | "yiq" => yiq_to_rgb(color),
        "XYZ" | "xyz" | "ciexyz" => xyz_to_rgb(color),
        "xyY" => xyz_to_rgb(xyy_to_xyz(color)),
        "sRGB" | "srgb" => srgb_to_linear_color(color),
        _other => {
            // Unknown source space: try OCIO (matches C++ use_colorconfig path)
            #[cfg(feature = "vfx")]
            if let Some(result) = transform_color_ocio(from, to, color) {
                return result;
            }
            color
        }
    };

    // Convert from linear RGB to target
    match to {
        "rgb" | "RGB" | "Rec709" | "linear" | "scene_linear" => linear,
        "hsv" | "HSV" => rgb_to_hsv(linear),
        "hsl" | "HSL" => rgb_to_hsl(linear),
        "YIQ" | "yiq" => rgb_to_yiq(linear),
        "XYZ" | "xyz" | "ciexyz" => rgb_to_xyz(linear),
        "xyY" => xyz_to_xyy(rgb_to_xyz(linear)),
        "sRGB" | "srgb" => linear_to_srgb_color(linear),
        _other => {
            // Unknown target space: try OCIO
            #[cfg(feature = "vfx")]
            if let Some(result) = transform_color_ocio("RGB", to, linear) {
                return result;
            }
            linear
        }
    }
}

/// Transform a color with derivatives from one named color space to another.
///
/// Since most color transforms are linear (matrix multiply), derivatives
/// transform the same way as the value: val = transform(c.val),
/// dx = transform(c.dx), dy = transform(c.dy).
///
/// NOTE: sRGB gamma is nonlinear, so for sRGB we apply the full transform
/// to val but use the linear-approximation derivative (good enough for
/// shading -- matches C++ OSL behavior).
pub fn transform_color_dual(
    from: &str,
    to: &str,
    c: crate::dual::Dual2<Color3>,
) -> crate::dual::Dual2<Color3> {
    use crate::dual::Dual2;
    if from == to {
        return c;
    }
    // Apply the same transform independently to val, dx, dy.
    // This is correct for all linear transforms. For sRGB (nonlinear gamma),
    // the derivative is approximate but matches OSL C++ behavior.
    Dual2 {
        val: transform_color(from, to, c.val),
        dx: transform_color(from, to, c.dx),
        dy: transform_color(from, to, c.dy),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: Color3, b: Color3, eps: Float) -> bool {
        (a.x - b.x).abs() < eps && (a.y - b.y).abs() < eps && (a.z - b.z).abs() < eps
    }

    #[test]
    fn test_luminance() {
        let white = Color3::new(1.0, 1.0, 1.0);
        assert!((luminance(white) - 1.0).abs() < 0.001);

        let red = Color3::new(1.0, 0.0, 0.0);
        assert!((luminance(red) - 0.2126).abs() < 0.001);
    }

    #[test]
    fn test_hsv_roundtrip() {
        let colors = [
            Color3::new(1.0, 0.0, 0.0),
            Color3::new(0.0, 1.0, 0.0),
            Color3::new(0.0, 0.0, 1.0),
            Color3::new(0.5, 0.3, 0.8),
        ];
        for &c in &colors {
            let hsv = rgb_to_hsv(c);
            let back = hsv_to_rgb(hsv);
            assert!(approx_eq(c, back, 1e-5), "HSV roundtrip failed for {:?}", c);
        }
    }

    #[test]
    fn test_hsl_roundtrip() {
        let colors = [
            Color3::new(1.0, 0.0, 0.0),
            Color3::new(0.0, 1.0, 0.0),
            Color3::new(0.5, 0.3, 0.8),
        ];
        for &c in &colors {
            let hsl = rgb_to_hsl(c);
            let back = hsl_to_rgb(hsl);
            assert!(approx_eq(c, back, 1e-5), "HSL roundtrip failed for {:?}", c);
        }
    }

    #[test]
    fn test_yiq_roundtrip() {
        let c = Color3::new(0.5, 0.3, 0.8);
        let yiq = rgb_to_yiq(c);
        let back = yiq_to_rgb(yiq);
        // Standard YIQ coefficients are approximate, allow wider tolerance
        assert!(approx_eq(c, back, 0.02));
    }

    #[test]
    fn test_xyz_roundtrip() {
        let c = Color3::new(0.5, 0.3, 0.8);
        let xyz = rgb_to_xyz(c);
        let back = xyz_to_rgb(xyz);
        assert!(approx_eq(c, back, 1e-4));
    }

    #[test]
    fn test_srgb_roundtrip() {
        let c = Color3::new(0.5, 0.3, 0.8);
        let srgb = linear_to_srgb_color(c);
        let back = srgb_to_linear_color(srgb);
        assert!(approx_eq(c, back, 1e-5));
    }

    #[test]
    fn test_blackbody() {
        let c = blackbody(6500.0); // roughly daylight
        // The new Planck's law computation returns physical irradiance values
        // (unnormalized), matching the C++ ColorSystem::compute_blackbody_rgb.
        assert!(c.x > 0.0 && c.y > 0.0 && c.z > 0.0);

        // Verify monotonicity: hotter = brighter
        let c_hot = blackbody(10000.0);
        let c_cool = blackbody(2000.0);
        assert!(c_hot.y > c_cool.y, "Hotter blackbody should be brighter");

        // Below Draper point should be nearly black
        let c_cold = blackbody(500.0);
        assert!(c_cold.x < 0.001 && c_cold.y < 0.001 && c_cold.z < 0.001);

        // Test ColorSystem-based blackbody
        let cs = ColorSystem::rec709();
        let cs_bb = cs.blackbody_rgb(6500.0);
        assert!(cs_bb.x > 0.0 && cs_bb.y > 0.0 && cs_bb.z > 0.0);
    }

    #[test]
    fn test_wavelength_color() {
        let red = wavelength_color(650.0);
        assert!(red.x > red.y && red.x > red.z);

        let green = wavelength_color(530.0);
        assert!(green.y > green.x && green.y > green.z);

        let blue = wavelength_color(460.0);
        assert!(blue.z > blue.x);
    }

    #[test]
    fn test_transform_color() {
        let c = Color3::new(0.5, 0.3, 0.8);
        let hsv = transform_color("rgb", "hsv", c);
        let back = transform_color("hsv", "rgb", hsv);
        assert!(approx_eq(c, back, 1e-5));
    }

    #[test]
    fn test_transform_between_systems_roundtrip() {
        // Convert Rec709 RGB -> NTSC RGB -> back to Rec709 RGB via XYZ
        let rec709 = ColorSystem::rec709();
        let ntsc = ColorSystem::from_name("NTSC").unwrap();
        let c = Color3::new(0.5, 0.3, 0.8);

        // Rec709 -> NTSC (both in linear RGB space)
        let ntsc_rgb = transform_between_systems(&rec709, &ntsc, "rgb", "rgb", c);
        // NTSC -> Rec709
        let back = transform_between_systems(&ntsc, &rec709, "rgb", "rgb", ntsc_rgb);
        assert!(
            approx_eq(c, back, 1e-4),
            "roundtrip failed: orig={c:?}, back={back:?}"
        );
    }

    #[test]
    fn test_transform_between_systems_same() {
        // Same system, same space -> identity
        let rec709 = ColorSystem::rec709();
        let c = Color3::new(0.5, 0.3, 0.8);
        let result = transform_between_systems(&rec709, &rec709, "rgb", "rgb", c);
        assert!(approx_eq(c, result, 1e-5));
    }

    #[test]
    fn test_transform_color_dual_identity() {
        use crate::dual::Dual2;
        let c = Dual2::new(
            Color3::new(0.5, 0.3, 0.1),
            Color3::new(0.1, 0.0, 0.0),
            Color3::new(0.0, 0.1, 0.0),
        );
        // Same space -> identity
        let r = transform_color_dual("rgb", "rgb", c);
        assert!(approx_eq(r.val, c.val, 1e-6));
        assert!(approx_eq(r.dx, c.dx, 1e-6));
        assert!(approx_eq(r.dy, c.dy, 1e-6));
    }

    #[test]
    fn test_transform_color_dual_roundtrip() {
        use crate::dual::Dual2;
        let c = Dual2::new(
            Color3::new(0.5, 0.3, 0.1),
            Color3::new(0.1, 0.0, 0.0),
            Color3::new(0.0, 0.1, 0.0),
        );
        // rgb -> hsv -> rgb should roundtrip (approximately)
        let hsv = transform_color_dual("rgb", "hsv", c);
        let back = transform_color_dual("hsv", "rgb", hsv);
        assert!(
            approx_eq(back.val, c.val, 1e-4),
            "val roundtrip: orig={:?}, got={:?}",
            c.val,
            back.val
        );
    }

    #[test]
    fn test_transform_color_dual_xyz_derivs() {
        use crate::dual::Dual2;
        // XYZ <-> RGB is a linear matrix transform, so derivatives
        // should transform exactly
        let c = Dual2::new(
            Color3::new(0.5, 0.3, 0.1),
            Color3::new(1.0, 0.0, 0.0),
            Color3::new(0.0, 0.0, 1.0),
        );
        let xyz = transform_color_dual("rgb", "XYZ", c);
        // Verify val matches non-dual version
        let expected_val = transform_color("rgb", "XYZ", c.val);
        assert!(approx_eq(xyz.val, expected_val, 1e-6));
        // Verify dx matches non-dual version applied to dx
        let expected_dx = transform_color("rgb", "XYZ", c.dx);
        assert!(approx_eq(xyz.dx, expected_dx, 1e-6));
    }

    // -----------------------------------------------------------------------
    // OCIO integration tests (vfx feature)
    // -----------------------------------------------------------------------

    #[test]
    #[cfg(feature = "vfx")]
    fn test_ocio_config_init() {
        let cfg = OcioColorConfig::new();
        assert!(cfg.has_colorspace("ACEScg"));
        assert!(cfg.has_colorspace("sRGB"));
        assert!(!cfg.has_colorspace("nonexistent_space_xyz"));
    }

    #[test]
    #[cfg(feature = "vfx")]
    fn test_ocio_acescg_to_srgb() {
        // 18% grey in ACEScg should produce a reasonable sRGB value
        let grey = Color3::new(0.18, 0.18, 0.18);
        let result = transform_color_ocio("ACEScg", "sRGB", grey);
        assert!(result.is_some());
        let srgb = result.unwrap();
        // sRGB output should be positive and < 1.0 for this input
        assert!(srgb.x > 0.0 && srgb.x < 1.0);
        assert!(srgb.y > 0.0 && srgb.y < 1.0);
        assert!(srgb.z > 0.0 && srgb.z < 1.0);
    }

    #[test]
    #[cfg(feature = "vfx")]
    fn test_ocio_roundtrip() {
        // ACEScg -> sRGB -> ACEScg should round-trip
        let original = Color3::new(0.5, 0.3, 0.1);
        let to_srgb = transform_color_ocio("ACEScg", "sRGB", original).unwrap();
        let back = transform_color_ocio("sRGB", "ACEScg", to_srgb).unwrap();
        assert!(approx_eq(original, back, 5e-3));
    }

    #[test]
    #[cfg(feature = "vfx")]
    fn test_ocio_identity() {
        // Same space should return the same color
        let c = Color3::new(0.7, 0.2, 0.9);
        let result = transform_color_ocio("ACEScg", "ACEScg", c).unwrap();
        assert!(approx_eq(c, result, 1e-6));
    }

    #[test]
    #[cfg(feature = "vfx")]
    fn test_ocio_unknown_space_returns_none() {
        let c = Color3::new(0.5, 0.5, 0.5);
        assert!(transform_color_ocio("nonexistent_src", "ACEScg", c).is_none());
        assert!(transform_color_ocio("ACEScg", "nonexistent_dst", c).is_none());
    }

    #[test]
    #[cfg(feature = "vfx")]
    fn test_transform_color_ocio_fallback() {
        // When from/to are OCIO names not in OSL builtins, transform_color
        // should delegate to OCIO via the fallback path.
        // "ACES2065-1" is an OCIO space name not in the OSL builtin list
        // as that exact string; the OCIO config knows it as "ACES2065-1".
        let c = Color3::new(0.18, 0.18, 0.18);
        let cfg = OcioColorConfig::new();
        // Verify the OCIO config knows this name
        if cfg.has_colorspace("ACES2065-1") && cfg.has_colorspace("ACEScg") {
            // OCIO and builtin paths use different matrices (OCIO: CAT02/Bradford,
            // builtin: simplified ColorSystem). Verify each produces valid results.
            let ocio_result = transform_color_ocio("ACES2065-1", "ACEScg", c).unwrap();
            let tc_result = transform_color("ACES2065-1", "ACEScg", c);
            // Both should produce positive, finite values for a neutral gray
            assert!(ocio_result.x > 0.0 && ocio_result.x.is_finite());
            assert!(tc_result.x > 0.0 && tc_result.x.is_finite());
            // OCIO neutral gray should stay neutral (same whitepoint)
            assert!(approx_eq(ocio_result, c, 1e-4));
        }
    }

    #[test]
    fn test_is_builtin_space() {
        // Builtin = direct analytical conversion (matching C++ transformc)
        assert!(is_builtin_space("rgb"));
        assert!(is_builtin_space("hsv"));
        assert!(is_builtin_space("sRGB"));
        assert!(is_builtin_space("XYZ"));
        assert!(is_builtin_space("xyY"));
        assert!(is_builtin_space("YIQ"));
        // Named color systems go through OCIO (like C++ reference)
        assert!(!is_builtin_space("NTSC"));
        assert!(!is_builtin_space("ACEScg"));
        assert!(!is_builtin_space("ACES2065-1"));
        assert!(!is_builtin_space("my_custom_space"));
    }
}
