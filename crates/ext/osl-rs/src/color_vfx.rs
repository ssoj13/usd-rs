//! VFX color system adapter — bridges `vfx-ocio` to OSL color transforms.
//!
//! Wraps `vfx_ocio::Config` and `vfx_io::ColorConfig` to provide production-quality
//! OCIO-backed color space conversions for OSL's `transformc()` function.
//!
//! # Feature gate
//!
//! Only available with the `vfx` feature. Without it, OSL uses the built-in
//! matrix-based conversions in [`crate::color`].

use std::sync::Arc;

use vfx_io::ColorConfig;

use crate::math::Color3;

/// OSL color system backed by `vfx-ocio`.
///
/// Provides OCIO-backed `transformc()` for accurate color space conversions
/// using the ACES 1.3 config (or a custom OCIO config).
#[derive(Clone)]
pub struct VfxColorSystem {
    config: Arc<ColorConfig>,
}

impl VfxColorSystem {
    /// Create with the built-in ACES 1.3 config (default for VFX).
    pub fn new() -> Self {
        Self {
            config: Arc::new(ColorConfig::aces_1_3()),
        }
    }

    /// Create from an OCIO config file.
    pub fn from_file(path: &str) -> Result<Self, String> {
        let cc = ColorConfig::from_file(path);
        if !cc.valid() {
            return Err(format!(
                "failed to load OCIO config: {}",
                cc.error_message()
            ));
        }
        Ok(Self {
            config: Arc::new(cc),
        })
    }

    /// Create from an existing `ColorConfig`.
    pub fn from_config(config: ColorConfig) -> Self {
        Self {
            config: Arc::new(config),
        }
    }

    /// Access the underlying `ColorConfig`.
    pub fn config(&self) -> &ColorConfig {
        &self.config
    }

    /// Transform a color between named color spaces.
    ///
    /// This is the main entry point for OSL's `transformc("from", "to", color)`.
    /// Maps OSL color space names to OCIO color spaces.
    ///
    /// For OSL-native color spaces (HSV, HSL, YIQ, XYZ, xyY) that OCIO doesn't
    /// handle, falls back to the built-in matrix/analytic conversions in
    /// [`crate::color`] to convert to/from linear RGB, then uses OCIO for any
    /// remaining transform.
    pub fn transformc(&self, from: &str, to: &str, color: Color3) -> Result<Color3, String> {
        // Same space → identity
        if from == to {
            return Ok(color);
        }

        // OSL-native spaces not in OCIO: convert to/from linear RGB analytically,
        // then let OCIO handle the rest.
        let osl_native = |s: &str| {
            matches!(
                s.to_lowercase().as_str(),
                "hsv" | "hsl" | "yiq" | "xyz" | "ciexyz" | "xyy"
            )
        };

        if osl_native(from) || osl_native(to) {
            // Step 1: convert from source to linear RGB via built-in color module
            let cs = crate::color::ColorSystem::rec709();
            let linear = if osl_native(from) {
                cs.to_rgb(from, color)
            } else {
                color
            };
            // Step 2: convert from linear RGB to dest via built-in color module
            let result = if osl_native(to) {
                cs.from_rgb(to, linear)
            } else {
                // Need OCIO for the target space
                let to_ocio = map_osl_colorspace(to);
                let from_ocio = map_osl_colorspace("linear");
                let processor = self
                    .config
                    .processor(from_ocio, to_ocio)
                    .map_err(|e| format!("OCIO processor error: {e}"))?;
                let mut pixels = [[linear.x, linear.y, linear.z]];
                processor.apply_rgb(&mut pixels);
                Color3::new(pixels[0][0], pixels[0][1], pixels[0][2])
            };
            return Ok(result);
        }

        let from_ocio = map_osl_colorspace(from);
        let to_ocio = map_osl_colorspace(to);

        let processor = self
            .config
            .processor(from_ocio, to_ocio)
            .map_err(|e| format!("OCIO processor error: {e}"))?;

        let mut pixels = [[color.x, color.y, color.z]];
        processor.apply_rgb(&mut pixels);

        Ok(Color3::new(pixels[0][0], pixels[0][1], pixels[0][2]))
    }

    /// Transform an array of colors between named color spaces (batch).
    pub fn transformc_batch(
        &self,
        from: &str,
        to: &str,
        colors: &mut [Color3],
    ) -> Result<(), String> {
        if from == to {
            return Ok(());
        }

        let osl_native = |s: &str| {
            matches!(
                s.to_lowercase().as_str(),
                "hsv" | "hsl" | "yiq" | "xyz" | "ciexyz" | "xyy"
            )
        };

        if osl_native(from) || osl_native(to) {
            // Per-element fallback through built-in color module
            for c in colors.iter_mut() {
                *c = self.transformc(from, to, *c)?;
            }
            return Ok(());
        }

        let from_ocio = map_osl_colorspace(from);
        let to_ocio = map_osl_colorspace(to);

        let processor = self
            .config
            .processor(from_ocio, to_ocio)
            .map_err(|e| format!("OCIO processor error: {e}"))?;

        // Convert Color3 slice to [[f32; 3]] for OCIO
        let mut pixels: Vec<[f32; 3]> = colors.iter().map(|c| [c.x, c.y, c.z]).collect();
        processor.apply_rgb(&mut pixels);

        for (i, px) in pixels.iter().enumerate() {
            colors[i] = Color3::new(px[0], px[1], px[2]);
        }
        Ok(())
    }

    /// Check if a color space is known to the config.
    pub fn has_colorspace(&self, name: &str) -> bool {
        let mapped = map_osl_colorspace(name);
        self.config.has_colorspace(mapped)
    }

    /// Check if a color space is linear.
    pub fn is_linear(&self, name: &str) -> bool {
        let mapped = map_osl_colorspace(name);
        self.config.is_colorspace_linear(mapped)
    }

    /// List all available color space names.
    pub fn colorspace_names(&self) -> Vec<String> {
        self.config
            .colorspace_names()
            .iter()
            .map(|s| s.to_string())
            .collect()
    }
}

impl Default for VfxColorSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// Map OSL color space names to OCIO color space names.
///
/// OSL uses short names like "sRGB", "linear", "ACEScg" etc.
/// OCIO configs may use different canonical names.
fn map_osl_colorspace(name: &str) -> &str {
    match name {
        // OSL built-in names → OCIO equivalents
        "rgb" | "RGB" | "linear" | "scene_linear" => "ACEScg",
        "sRGB" | "srgb" => "sRGB",
        "Rec709" | "rec709" => "sRGB", // Rec709 primaries ≈ sRGB in ACES config
        "ACEScg" | "acescg" => "ACEScg",
        "ACES2065-1" | "aces" | "ACES" => "ACES2065-1",
        "ACEScc" | "acescc" => "ACEScc",
        "ACEScct" | "acescct" => "ACEScct",
        "XYZ" | "xyz" | "ciexyz" => "CIE-XYZ-D65",
        // Pass through unknown names directly (OCIO may know them)
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vfx_color_system_creation() {
        let cs = VfxColorSystem::new();
        assert!(cs.has_colorspace("ACEScg"));
        assert!(cs.has_colorspace("sRGB"));
    }

    #[test]
    fn test_transformc_identity() {
        let cs = VfxColorSystem::new();
        let input = Color3::new(0.18, 0.18, 0.18);
        let output = cs.transformc("ACEScg", "ACEScg", input).unwrap();
        assert!((output.x - input.x).abs() < 1e-5);
        assert!((output.y - input.y).abs() < 1e-5);
        assert!((output.z - input.z).abs() < 1e-5);
    }

    #[test]
    fn test_transformc_acescg_to_srgb() {
        let cs = VfxColorSystem::new();
        let linear_gray = Color3::new(0.18, 0.18, 0.18);
        let srgb = cs.transformc("ACEScg", "sRGB", linear_gray).unwrap();
        // sRGB of 18% gray should be ~0.46 (gamma curve)
        assert!(srgb.x > 0.3 && srgb.x < 0.6, "sRGB value: {}", srgb.x);
    }

    #[test]
    fn test_osl_name_mapping() {
        assert_eq!(map_osl_colorspace("linear"), "ACEScg");
        assert_eq!(map_osl_colorspace("sRGB"), "sRGB");
        assert_eq!(map_osl_colorspace("ACEScg"), "ACEScg");
        assert_eq!(map_osl_colorspace("custom_space"), "custom_space");
    }

    #[test]
    fn test_colorspace_names() {
        let cs = VfxColorSystem::new();
        let names = cs.colorspace_names();
        assert!(!names.is_empty());
    }

    #[test]
    fn test_batch_transform() {
        let cs = VfxColorSystem::new();
        let mut colors = vec![
            Color3::new(0.18, 0.18, 0.18),
            Color3::new(0.5, 0.5, 0.5),
            Color3::new(0.0, 0.0, 0.0),
        ];
        cs.transformc_batch("ACEScg", "sRGB", &mut colors).unwrap();
        // Neutral grays should map cleanly to non-negative sRGB values
        for c in &colors {
            assert!(
                c.x >= -0.01 && c.y >= -0.01 && c.z >= -0.01,
                "unexpected negative: {:?}",
                c
            );
        }
    }
}
