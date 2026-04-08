//! HdRenderSettings - Bprim for render settings.
//!
//! A buffer primitive that represents render settings applied to a render pass.
//! Contains render products, namespaced settings, and active/valid state.
//! Port of pxr/imaging/hd/renderSettings.h/cpp

use super::{HdBprim, HdRenderParam, HdSceneDelegate};
use crate::types::HdDirtyBits;
use std::collections::HashMap;
use usd_sdf::Path as SdfPath;
use usd_tf::Token;
use usd_vt::Value;

/// Dirty bits for render settings prim.
pub mod render_settings_dirty_bits {
    use crate::types::HdDirtyBits;

    /// Render products changed.
    pub const DIRTY_PRODUCTS: HdDirtyBits = 1 << 2;
    /// Namespaced settings changed.
    pub const DIRTY_NAMESPACED_SETTINGS: HdDirtyBits = 1 << 3;
    /// Active state changed.
    pub const DIRTY_ACTIVE: HdDirtyBits = 1 << 4;
    /// All render-settings-specific bits.
    pub const ALL_DIRTY: HdDirtyBits = 0xFFFF_FFFF;
}

/// Render product descriptor.
///
/// Describes an output product (e.g. an EXR file with specific AOVs).
#[derive(Debug, Clone)]
pub struct HdRenderProduct {
    /// Product path in the scene.
    pub path: SdfPath,
    /// Product type (e.g. "raster", "deepRaster").
    pub product_type: Token,
    /// Product name / output file path.
    pub name: String,
    /// Resolution [width, height].
    pub resolution: [u32; 2],
    /// Pixel aspect ratio.
    pub pixel_aspect_ratio: f32,
    /// Render vars (AOVs) in this product.
    pub render_vars: Vec<SdfPath>,
    /// Additional namespaced settings for this product.
    pub namespaced_settings: HashMap<Token, Value>,
}

impl Default for HdRenderProduct {
    fn default() -> Self {
        Self {
            path: SdfPath::empty(),
            product_type: Token::new("raster"),
            name: String::new(),
            resolution: [512, 512],
            pixel_aspect_ratio: 1.0,
            render_vars: Vec::new(),
            namespaced_settings: HashMap::new(),
        }
    }
}

/// Render settings Bprim.
///
/// Stores render settings for a render pass, including output products,
/// namespaced settings from renderers, and active/valid state.
///
/// Port of HdRenderSettings from pxr/imaging/hd/renderSettings.h
#[derive(Debug)]
pub struct HdRenderSettings {
    /// Prim identifier.
    id: SdfPath,
    /// Current dirty bits.
    dirty_bits: HdDirtyBits,
    /// Whether this render settings prim is the active one.
    active: bool,
    /// Whether settings are valid and complete.
    valid: bool,
    /// Output render products.
    render_products: Vec<HdRenderProduct>,
    /// Namespaced settings (renderer-specific).
    namespaced_settings: HashMap<Token, Value>,
}

impl HdRenderSettings {
    /// Create new render settings prim.
    pub fn new(id: SdfPath) -> Self {
        Self {
            id,
            dirty_bits: render_settings_dirty_bits::ALL_DIRTY,
            active: false,
            valid: false,
            render_products: Vec::new(),
            namespaced_settings: HashMap::new(),
        }
    }

    /// Whether this is the active render settings prim.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Whether settings are valid.
    pub fn is_valid(&self) -> bool {
        self.valid
    }

    /// Get render products.
    pub fn get_render_products(&self) -> &[HdRenderProduct] {
        &self.render_products
    }

    /// Get namespaced settings.
    pub fn get_namespaced_settings(&self) -> &HashMap<Token, Value> {
        &self.namespaced_settings
    }

    /// Get a specific namespaced setting.
    pub fn get_setting(&self, key: &Token) -> Option<&Value> {
        self.namespaced_settings.get(key)
    }

    /// Set active state.
    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }
}

impl HdBprim for HdRenderSettings {
    fn get_id(&self) -> &SdfPath {
        &self.id
    }

    fn get_dirty_bits(&self) -> HdDirtyBits {
        self.dirty_bits
    }

    fn set_dirty_bits(&mut self, bits: HdDirtyBits) {
        self.dirty_bits = bits;
    }

    fn sync(
        &mut self,
        delegate: &dyn HdSceneDelegate,
        _render_param: Option<&dyn HdRenderParam>,
        dirty_bits: &mut HdDirtyBits,
    ) {
        if *dirty_bits & render_settings_dirty_bits::DIRTY_ACTIVE != 0 {
            // Pull active state from scene delegate
            let val = delegate.get(&self.id, &Token::new("active"));
            if let Some(b) = val.get::<bool>() {
                self.active = *b;
            }
        }

        if *dirty_bits & render_settings_dirty_bits::DIRTY_NAMESPACED_SETTINGS != 0 {
            // In full implementation: pull namespaced settings from delegate
        }

        if *dirty_bits & render_settings_dirty_bits::DIRTY_PRODUCTS != 0 {
            // In full implementation: pull render products from delegate
        }

        self.valid = !self.render_products.is_empty() || !self.namespaced_settings.is_empty();
        self.dirty_bits = Self::CLEAN;
        *dirty_bits = Self::CLEAN;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_settings_creation() {
        let settings = HdRenderSettings::new(SdfPath::from_string("/Render/Settings").unwrap());
        assert!(!settings.is_active());
        assert!(!settings.is_valid());
        assert!(settings.get_render_products().is_empty());
    }

    #[test]
    fn test_render_product_default() {
        let product = HdRenderProduct::default();
        assert_eq!(product.resolution, [512, 512]);
        assert_eq!(product.pixel_aspect_ratio, 1.0);
    }

    #[test]
    fn test_set_active() {
        let mut settings = HdRenderSettings::new(SdfPath::from_string("/Render/Settings").unwrap());
        settings.set_active(true);
        assert!(settings.is_active());
    }

    #[test]
    fn test_dirty_bits() {
        let settings = HdRenderSettings::new(SdfPath::from_string("/Render/Settings").unwrap());
        assert_eq!(
            settings.get_dirty_bits(),
            render_settings_dirty_bits::ALL_DIRTY
        );
    }
}
