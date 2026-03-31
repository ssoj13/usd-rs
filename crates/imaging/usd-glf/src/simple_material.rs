//! Basic material properties.
//!
//! Port of pxr/imaging/glf/simpleMaterial.h

use usd_gf::Vec4f;

/// Type alias for 4D floating point vector to match USD naming convention.
///
/// This alias provides compatibility with OpenUSD's `GfVec4f` type.
pub type GfVec4f = Vec4f;

/// Simple material representation for basic shading.
///
/// Provides ambient, diffuse, specular, emission colors and shininess.
#[derive(Debug, Clone, PartialEq)]
pub struct GlfSimpleMaterial {
    /// Ambient color (RGBA in working color space)
    ambient: GfVec4f,
    /// Diffuse color (RGBA in working color space)
    diffuse: GfVec4f,
    /// Specular color (RGBA in working color space)
    specular: GfVec4f,
    /// Emission color (RGBA in working color space)
    emission: GfVec4f,
    /// Shininess/specular exponent
    shininess: f64,
}

impl GlfSimpleMaterial {
    /// Creates a new material with default properties.
    pub fn new() -> Self {
        Self {
            ambient: GfVec4f::new(0.2, 0.2, 0.2, 1.0),
            diffuse: GfVec4f::new(0.8, 0.8, 0.8, 1.0),
            // C++: _specular(0.5, 0.5, 0.5, 1)
            specular: GfVec4f::new(0.5, 0.5, 0.5, 1.0),
            emission: GfVec4f::new(0.0, 0.0, 0.0, 1.0),
            // C++: _shininess(32.0)
            shininess: 32.0,
        }
    }

    /// Returns the ambient color component.
    ///
    /// Ambient color represents the base lighting contribution when no direct light hits the surface.
    /// The color is in working color space (typically linear, not sRGB).
    ///
    /// # Returns
    /// Reference to the RGBA ambient color vector
    pub fn get_ambient(&self) -> &GfVec4f {
        &self.ambient
    }

    /// Sets the ambient color component.
    ///
    /// # Arguments
    /// * `ambient` - RGBA ambient color in working color space
    pub fn set_ambient(&mut self, ambient: GfVec4f) {
        self.ambient = ambient;
    }

    /// Returns the diffuse color component.
    ///
    /// Diffuse color represents the base surface color under diffuse (non-specular) lighting.
    /// This is typically the most prominent color component. The color is in working color space.
    ///
    /// # Returns
    /// Reference to the RGBA diffuse color vector
    pub fn get_diffuse(&self) -> &GfVec4f {
        &self.diffuse
    }

    /// Sets the diffuse color component.
    ///
    /// # Arguments
    /// * `diffuse` - RGBA diffuse color in working color space
    pub fn set_diffuse(&mut self, diffuse: GfVec4f) {
        self.diffuse = diffuse;
    }

    /// Returns the specular color component.
    ///
    /// Specular color defines the color and intensity of specular highlights (reflections).
    /// Used in Phong/Blinn-Phong shading models. The color is in working color space.
    ///
    /// # Returns
    /// Reference to the RGBA specular color vector
    pub fn get_specular(&self) -> &GfVec4f {
        &self.specular
    }

    /// Sets the specular color component.
    ///
    /// # Arguments
    /// * `specular` - RGBA specular color in working color space
    pub fn set_specular(&mut self, specular: GfVec4f) {
        self.specular = specular;
    }

    /// Returns the emission (emissive) color component.
    ///
    /// Emission color represents light emitted by the surface itself, independent of external lighting.
    /// This adds to the final color and can be used for glowing effects. The color is in working color space.
    ///
    /// # Returns
    /// Reference to the RGBA emission color vector
    pub fn get_emission(&self) -> &GfVec4f {
        &self.emission
    }

    /// Sets the emission (emissive) color component.
    ///
    /// # Arguments
    /// * `emission` - RGBA emission color in working color space
    pub fn set_emission(&mut self, emission: GfVec4f) {
        self.emission = emission;
    }

    /// Returns the shininess (specular exponent).
    ///
    /// Shininess controls the size and sharpness of specular highlights in Phong/Blinn-Phong shading.
    /// Higher values create smaller, sharper highlights (more polished surface).
    /// Lower values create larger, softer highlights (rougher surface).
    ///
    /// Typical range: 0.0 (no specular) to 128.0 (very sharp highlights).
    ///
    /// # Returns
    /// Specular exponent value
    pub fn get_shininess(&self) -> f64 {
        self.shininess
    }

    /// Sets the shininess (specular exponent).
    ///
    /// # Arguments
    /// * `shininess` - Specular exponent value (typically 0.0 to 128.0)
    pub fn set_shininess(&mut self, shininess: f64) {
        self.shininess = shininess;
    }
}

impl Default for GlfSimpleMaterial {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_material_creation() {
        let mat = GlfSimpleMaterial::new();
        assert_eq!(mat.get_ambient(), &GfVec4f::new(0.2, 0.2, 0.2, 1.0));
        assert_eq!(mat.get_diffuse(), &GfVec4f::new(0.8, 0.8, 0.8, 1.0));
        // C++: _specular(0.5, 0.5, 0.5, 1)
        assert_eq!(mat.get_specular(), &GfVec4f::new(0.5, 0.5, 0.5, 1.0));
        // C++: _shininess(32.0)
        assert_eq!(mat.get_shininess(), 32.0);
    }

    #[test]
    fn test_material_properties() {
        let mut mat = GlfSimpleMaterial::default();
        mat.set_diffuse(GfVec4f::new(1.0, 0.0, 0.0, 1.0));
        mat.set_shininess(32.0);

        assert_eq!(mat.get_diffuse(), &GfVec4f::new(1.0, 0.0, 0.0, 1.0));
        assert_eq!(mat.get_shininess(), 32.0);
    }

    #[test]
    fn test_material_equality() {
        let mat1 = GlfSimpleMaterial::new();
        let mat2 = GlfSimpleMaterial::new();
        assert_eq!(mat1, mat2);

        let mut mat3 = GlfSimpleMaterial::new();
        mat3.set_shininess(50.0);
        assert_ne!(mat1, mat3);
    }
}
