//! Scene light.
//!
//! What: Light definition matching KHR_lights_punctual.
//! Why: glTF IO must preserve light metadata.
//! How: Stores light parameters with defaults matching Draco.
//! Where used: Scene graphs and glTF IO.

use draco_core::core::constants::DRACO_PI;
use draco_core::core::vector_d::Vector3f;

/// Light type (directional, point, spot).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LightType {
    Directional,
    Point,
    Spot,
}

/// Light descriptor.
#[derive(Clone, Debug)]
pub struct Light {
    name: String,
    color: Vector3f,
    intensity: f64,
    light_type: LightType,
    range: f64,
    inner_cone_angle: f64,
    outer_cone_angle: f64,
}

impl Default for Light {
    fn default() -> Self {
        Self {
            name: String::new(),
            color: Vector3f::new3(1.0, 1.0, 1.0),
            intensity: 1.0,
            light_type: LightType::Point,
            range: f32::MAX as f64,
            inner_cone_angle: 0.0,
            outer_cone_angle: DRACO_PI / 4.0,
        }
    }
}

impl Light {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn copy_from(&mut self, src: &Light) {
        self.name = src.name.clone();
        self.color = src.color;
        self.intensity = src.intensity;
        self.light_type = src.light_type;
        self.range = src.range;
        self.inner_cone_angle = src.inner_cone_angle;
        self.outer_cone_angle = src.outer_cone_angle;
    }

    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_color(&mut self, color: Vector3f) {
        self.color = color;
    }

    pub fn color(&self) -> &Vector3f {
        &self.color
    }

    pub fn set_intensity(&mut self, intensity: f64) {
        self.intensity = intensity;
    }

    pub fn intensity(&self) -> f64 {
        self.intensity
    }

    pub fn set_type(&mut self, light_type: LightType) {
        self.light_type = light_type;
    }

    pub fn light_type(&self) -> LightType {
        self.light_type
    }

    pub fn set_range(&mut self, range: f64) {
        self.range = range;
    }

    pub fn range(&self) -> f64 {
        self.range
    }

    pub fn set_inner_cone_angle(&mut self, angle: f64) {
        self.inner_cone_angle = angle;
    }

    pub fn inner_cone_angle(&self) -> f64 {
        self.inner_cone_angle
    }

    pub fn set_outer_cone_angle(&mut self, angle: f64) {
        self.outer_cone_angle = angle;
    }

    pub fn outer_cone_angle(&self) -> f64 {
        self.outer_cone_angle
    }
}
