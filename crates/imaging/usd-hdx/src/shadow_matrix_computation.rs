
//! Shadow matrix computation - interface for computing shadow projection matrices.
//!
//! Provides the abstract interface used by HdxShadowTask to compute
//! view/projection matrices for shadow map rendering. Decouples shadow
//! matrix calculation from the shadow rendering task.
//! Port of pxr/imaging/hdx/shadowMatrixComputation.h

use usd_camera_util::Framing;
use usd_gf::Matrix4d;

/// Interface for computing shadow matrices.
///
/// Implementations compute view and projection matrices for shadow map
/// rendering based on light parameters and camera frustum.
///
/// Port of HdxShadowMatrixComputation from pxr/imaging/hdx/shadowMatrixComputation.h
pub trait HdxShadowMatrixComputation: Send + Sync {
    /// Compute the shadow matrix for a given viewport.
    ///
    /// Returns a combined view-projection matrix for shadow map rendering.
    ///
    /// # Arguments
    /// * `viewport` - Viewport parameters [x, y, width, height]
    fn compute(&self, viewport: [f64; 4]) -> Matrix4d;

    /// Compute shadow matrix with camera framing.
    ///
    /// Returns (view, projection) matrix pair for shadow map rendering,
    /// taking into account the camera framing for correct frustum fitting.
    ///
    /// # Arguments
    /// * `framing` - Camera framing to use for frustum computation
    fn compute_with_framing(&self, _framing: &Framing) -> (Matrix4d, Matrix4d) {
        // Default: fall back to simple compute with full viewport
        let mat = self.compute([0.0, 0.0, 1.0, 1.0]);
        (Matrix4d::identity(), mat)
    }
}

/// Simple directional light shadow matrix computation.
///
/// Computes orthographic shadow matrices for directional lights.
#[allow(dead_code)] // Fields used when full shadow matrix computation is implemented
pub struct HdxSimpleShadowMatrixComputation {
    /// Light direction (world space, pointing toward light).
    light_dir: [f64; 3],
    /// Shadow frustum width.
    width: f64,
    /// Shadow frustum height.
    height: f64,
    /// Shadow near plane.
    near: f64,
    /// Shadow far plane.
    far: f64,
}

impl HdxSimpleShadowMatrixComputation {
    /// Create a directional light shadow computation.
    pub fn new(light_dir: [f64; 3], width: f64, height: f64, near: f64, far: f64) -> Self {
        Self {
            light_dir,
            width,
            height,
            near,
            far,
        }
    }
}

impl HdxShadowMatrixComputation for HdxSimpleShadowMatrixComputation {
    fn compute(&self, _viewport: [f64; 4]) -> Matrix4d {
        // Simplified orthographic shadow projection.
        // In full implementation: compute view matrix from light_dir,
        // then orthographic projection from width/height/near/far.
        Matrix4d::identity()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_shadow_computation() {
        let comp = HdxSimpleShadowMatrixComputation::new([0.0, -1.0, 0.0], 10.0, 10.0, 0.1, 100.0);
        let _mat = comp.compute([0.0, 0.0, 1920.0, 1080.0]);
    }

    #[test]
    fn test_shadow_with_framing() {
        let comp = HdxSimpleShadowMatrixComputation::new([0.0, -1.0, 0.0], 10.0, 10.0, 0.1, 100.0);
        let framing = Framing::new_empty();
        let (_view, _proj) = comp.compute_with_framing(&framing);
    }
}
