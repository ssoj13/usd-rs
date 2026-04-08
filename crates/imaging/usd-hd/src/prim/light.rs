//! HdLight - Light state primitive.
//!
//! Represents lights in Hydra. Supports:
//! - Point lights
//! - Directional lights
//! - Spot lights
//! - Rect/disk/sphere/cylinder lights
//! - Dome/environment lights
//! - Custom light types via shader
//!
//! # Light Types
//!
//! Different light types have different parameters:
//! - **Point**: Position, color, intensity, radius
//! - **Directional**: Direction, color, intensity, angle
//! - **Spot**: Position, direction, color, intensity, cone angle
//! - **Rect**: Position, size, color, intensity
//! - **Dome**: Environment map, intensity

use super::{HdRenderParam, HdSceneDelegate, HdSprim};
use crate::types::HdDirtyBits;
use usd_sdf::Path as SdfPath;

/// Light type enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HdLightType {
    /// Distant/directional light.
    Distant,

    /// Point light (spherical).
    Point,

    /// Spot light (cone).
    Spot,

    /// Rectangular area light.
    Rect,

    /// Disk area light.
    Disk,

    /// Cylindrical area light.
    Cylinder,

    /// Spherical area light.
    Sphere,

    /// Dome/environment light.
    Dome,

    /// Custom light shader.
    Plugin,
}

impl HdLightType {
    /// Get string representation.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Distant => "distant",
            Self::Point => "point",
            Self::Spot => "spot",
            Self::Rect => "rect",
            Self::Disk => "disk",
            Self::Cylinder => "cylinder",
            Self::Sphere => "sphere",
            Self::Dome => "dome",
            Self::Plugin => "plugin",
        }
    }
}

/// Light parameters.
///
/// Contains common parameters shared by all light types.
/// Type-specific parameters accessed via scene delegate.
#[derive(Debug, Clone)]
pub struct HdLightParams {
    /// Light type.
    pub light_type: HdLightType,

    /// Light color (linear RGB).
    pub color: [f32; 3],

    /// Light intensity.
    pub intensity: f32,

    /// Exposure adjustment (stops).
    pub exposure: f32,

    /// Enable shadows.
    pub enable_shadows: bool,

    /// Shadow color.
    pub shadow_color: [f32; 3],

    /// Normalize power by area.
    pub normalize: bool,
}

impl Default for HdLightParams {
    fn default() -> Self {
        Self {
            light_type: HdLightType::Point,
            color: [1.0, 1.0, 1.0],
            intensity: 1.0,
            exposure: 0.0,
            enable_shadows: true,
            shadow_color: [0.0, 0.0, 0.0],
            normalize: false,
        }
    }
}

/// Base trait for lights.
///
/// All light types implement this trait.
pub trait HdLight: HdSprim {
    /// Get light type.
    fn get_light_type(&self) -> HdLightType;

    /// Get light parameters.
    fn get_light_params(&self) -> &HdLightParams;

    /// Set light parameters.
    fn set_light_params(&mut self, params: HdLightParams);
}

/// Generic light primitive.
///
/// Represents any type of light in Hydra.
#[derive(Debug)]
pub struct HdGenericLight {
    /// Prim identifier.
    id: SdfPath,

    /// Current dirty bits.
    dirty_bits: HdDirtyBits,

    /// Visibility state.
    visible: bool,

    /// Light parameters.
    params: HdLightParams,
}

impl HdGenericLight {
    /// Create a new light.
    pub fn new(id: SdfPath, light_type: HdLightType) -> Self {
        let mut params = HdLightParams::default();
        params.light_type = light_type;

        Self {
            id,
            dirty_bits: Self::get_initial_dirty_bits_mask(),
            visible: true,
            params,
        }
    }

    /// Check if light is visible.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Set visibility.
    pub fn set_visible(&mut self, visible: bool) {
        if self.visible != visible {
            self.visible = visible;
            self.mark_dirty(Self::DIRTY_VISIBILITY);
        }
    }
}

impl HdSprim for HdGenericLight {
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
        _delegate: &dyn HdSceneDelegate,
        _render_param: Option<&dyn HdRenderParam>,
        dirty_bits: &mut HdDirtyBits,
    ) {
        if (*dirty_bits & Self::DIRTY_PARAMS) != 0 {
            // Query light parameters from delegate
            // self.params = delegate.get_light_params(self.get_id());
        }

        if (*dirty_bits & Self::DIRTY_TRANSFORM) != 0 {
            // Query transform from delegate
            // let xform = delegate.get_transform(self.get_id());
        }

        if (*dirty_bits & Self::DIRTY_VISIBILITY) != 0 {
            // Query visibility from delegate
            // self.visible = delegate.get_visible(self.get_id());
        }

        *dirty_bits = Self::CLEAN;
        self.dirty_bits = Self::CLEAN;
    }
}

impl HdLight for HdGenericLight {
    fn get_light_type(&self) -> HdLightType {
        self.params.light_type
    }

    fn get_light_params(&self) -> &HdLightParams {
        &self.params
    }

    fn set_light_params(&mut self, params: HdLightParams) {
        self.params = params;
        self.mark_dirty(Self::DIRTY_PARAMS);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_light_creation() {
        let id = SdfPath::from_string("/Light").unwrap();
        let light = HdGenericLight::new(id.clone(), HdLightType::Point);

        assert_eq!(light.get_id(), &id);
        assert_eq!(light.get_light_type(), HdLightType::Point);
        assert!(light.is_visible());
    }

    #[test]
    fn test_light_types() {
        assert_eq!(HdLightType::Distant.as_str(), "distant");
        assert_eq!(HdLightType::Dome.as_str(), "dome");
    }

    #[test]
    fn test_light_params() {
        let mut light =
            HdGenericLight::new(SdfPath::from_string("/Light").unwrap(), HdLightType::Spot);

        let mut params = HdLightParams::default();
        params.light_type = HdLightType::Spot;
        params.color = [1.0, 0.5, 0.0];
        params.intensity = 100.0;

        light.set_light_params(params);

        let retrieved = light.get_light_params();
        assert_eq!(retrieved.intensity, 100.0);
        assert_eq!(retrieved.color, [1.0, 0.5, 0.0]);
    }

    #[test]
    fn test_light_visibility() {
        let mut light =
            HdGenericLight::new(SdfPath::from_string("/Light").unwrap(), HdLightType::Point);

        assert!(light.is_visible());

        light.set_visible(false);
        assert!(!light.is_visible());
        assert!(light.is_dirty_bits(HdGenericLight::DIRTY_VISIBILITY));
    }
}
