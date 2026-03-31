
//! Fallback lighting shader for Storm (ported from fallbackLightingShader.h).
//!
//! Provides default lighting when no lights are present in the scene.
//! Implements a simple directional light from the camera direction
//! with ambient contribution.

use crate::lighting_shader::{HdStLightingShader, LightingModel};
use std::sync::Arc;

/// Fallback lighting shader.
///
/// Used when no lights exist in the scene. Wraps HdStLightingShader
/// with constant lighting model and provides camera-relative fallback.
///
/// In C++, this loads from fallbackLighting.glslfx. Here we create
/// a constant-model lighting shader with embedded WGSL.
#[derive(Debug)]
pub struct HdStFallbackLightingShader {
    /// Inner lighting shader (constant model)
    inner: HdStLightingShader,
    /// World-to-view matrix (set by camera)
    world_to_view: [f32; 16],
    /// Projection matrix (set by camera)
    projection: [f32; 16],
}

impl Default for HdStFallbackLightingShader {
    fn default() -> Self {
        Self::new()
    }
}

impl HdStFallbackLightingShader {
    /// Stable hash ID for fallback lighting.
    const SHADER_ID: u64 = 0x0FA1_1BAC_0001;

    /// Create a new fallback lighting shader.
    pub fn new() -> Self {
        Self {
            inner: HdStLightingShader::new(Self::SHADER_ID, LightingModel::Constant),
            world_to_view: identity_matrix(),
            projection: identity_matrix(),
        }
    }

    /// Set camera matrices for the fallback light direction.
    pub fn set_camera(&mut self, world_to_view: [f32; 16], projection: [f32; 16]) {
        self.world_to_view = world_to_view;
        self.projection = projection;
    }

    /// Get the world-to-view matrix.
    pub fn get_world_to_view(&self) -> &[f32; 16] {
        &self.world_to_view
    }

    /// Get the projection matrix.
    pub fn get_projection(&self) -> &[f32; 16] {
        &self.projection
    }

    /// Get a reference to the inner lighting shader.
    pub fn get_lighting_shader(&self) -> &HdStLightingShader {
        &self.inner
    }

    /// Get the fragment shader source for fallback lighting.
    pub fn get_fragment_source() -> &'static str {
        FALLBACK_LIGHTING_FRAGMENT
    }
}

/// Shared pointer to fallback lighting shader.
pub type HdStFallbackLightingShaderSharedPtr = Arc<HdStFallbackLightingShader>;

// Embedded fallback lighting fragment shader (WGSL).
// Provides simple N dot L from camera direction + ambient.
const FALLBACK_LIGHTING_FRAGMENT: &str = r#"
// Fallback lighting: camera-relative directional + ambient
fn fallback_lighting(normal_eye: vec3<f32>) -> vec3<f32> {
    let light_dir = vec3<f32>(0.0, 0.0, 1.0); // camera direction
    let ambient = vec3<f32>(0.04, 0.04, 0.04);
    let diffuse_color = vec3<f32>(0.18, 0.18, 0.18);
    let n_dot_l = max(dot(normal_eye, light_dir), 0.0);
    return ambient + diffuse_color * n_dot_l;
}
"#;

/// Create a 4x4 identity matrix.
fn identity_matrix() -> [f32; 16] {
    [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fallback_shader() {
        let shader = HdStFallbackLightingShader::new();
        assert_eq!(
            shader.get_lighting_shader().get_model(),
            LightingModel::Constant
        );
    }

    #[test]
    fn test_fallback_source() {
        let src = HdStFallbackLightingShader::get_fragment_source();
        assert!(src.contains("fallback_lighting"));
        assert!(src.contains("light_dir"));
    }
}
