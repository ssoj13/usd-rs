//! Screen window parameters for RenderMan-style rendering.
//!
//! Computes parameters suitable for RenderMan's RiScreenWindow and RiProjection
//! from a GfCamera.

use usd_gf::{APERTURE_UNIT, Camera, CameraProjection, FOVDirection, Matrix4d, Vec4d};

/// Screen window parameters computed from a camera.
///
/// These parameters are suitable for setting up RenderMan rendering:
/// - `screen_window`: Use with RiScreenWindow
/// - `field_of_view`: Use with RiProjection (perspective)
/// - `z_facing_view_matrix`: Use with RiConcatTransform before RiWorldBegin
///
/// # Examples
///
/// ```
/// use usd_camera_util::ScreenWindowParameters;
/// use usd_gf::{Camera, FOVDirection};
///
/// let camera = Camera::default();
/// let params = ScreenWindowParameters::new(&camera, FOVDirection::Horizontal);
///
/// // Get parameters for RenderMan
/// let screen_window = params.screen_window();
/// let fov = params.field_of_view();
/// let view_matrix = params.z_facing_view_matrix();
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ScreenWindowParameters {
    /// Screen window as (left, right, bottom, top).
    /// Pass to RenderMan's RiScreenWindow.
    screen_window: Vec4d,

    /// Field of view in degrees.
    /// Full angle perspective FOV between screen space (-1,0) and (1,0).
    /// Pass to RenderMan's RiProjection as "perspective" parameter.
    field_of_view: f64,

    /// View matrix for z-facing camera.
    /// Inverse of the transform for a y-up, z-facing camera
    /// (vs OpenGL's -z facing camera).
    /// Use with RiConcatTransform before RiWorldBegin.
    z_facing_view_matrix: Matrix4d,
}

impl ScreenWindowParameters {
    /// Constructs screen window parameters from a camera.
    ///
    /// # Arguments
    ///
    /// * `camera` - The camera to extract parameters from.
    /// * `fit_direction` - Direction in which screenwindow has length 2.
    ///
    /// # Returns
    ///
    /// Screen window parameters suitable for RenderMan rendering.
    pub fn new(camera: &Camera, fit_direction: FOVDirection) -> Self {
        // Compute screen window from camera aperture and offsets
        let h_aperture = camera.horizontal_aperture() as f64;
        let v_aperture = camera.vertical_aperture() as f64;
        let h_offset = camera.horizontal_aperture_offset() as f64;
        let v_offset = camera.vertical_aperture_offset() as f64;

        let mut screen_window = Vec4d::new(
            -h_aperture + 2.0 * h_offset,
            h_aperture + 2.0 * h_offset,
            -v_aperture + 2.0 * v_offset,
            v_aperture + 2.0 * v_offset,
        );

        // Normalize screen window based on projection type
        if camera.projection() == CameraProjection::Perspective {
            let denom = match fit_direction {
                FOVDirection::Horizontal => h_aperture,
                FOVDirection::Vertical => v_aperture,
            };

            if denom != 0.0 {
                screen_window /= denom;
            }
        } else {
            // Orthographic: scale by aperture unit
            screen_window *= APERTURE_UNIT / 2.0;
        }

        // Get field of view
        let field_of_view = camera.field_of_view(fit_direction) as f64;

        // Compute z-facing view matrix
        // OpenGL uses -z facing, but RenderMan uses +z facing
        let z_flip = Matrix4d::from_diagonal_vec(&Vec4d::new(1.0, 1.0, -1.0, 1.0));
        let transform = *camera.transform();
        let z_facing_view_matrix = (z_flip * transform)
            .inverse()
            .unwrap_or_else(Matrix4d::identity);

        Self {
            screen_window,
            field_of_view,
            z_facing_view_matrix,
        }
    }

    /// Returns the screen window as (left, right, bottom, top).
    ///
    /// Pass these values to RenderMan's RiScreenWindow.
    pub fn screen_window(&self) -> Vec4d {
        self.screen_window
    }

    /// Returns the field of view in degrees.
    ///
    /// Full angle perspective FOV between screen space (-1,0) and (1,0).
    /// Pass to RenderMan's RiProjection as "perspective" parameter.
    pub fn field_of_view(&self) -> f64 {
        self.field_of_view
    }

    /// Returns the z-facing view matrix.
    ///
    /// Inverse of the transform for a y-up, z-facing camera.
    /// Use with RenderMan's RiConcatTransform before RiWorldBegin.
    pub fn z_facing_view_matrix(&self) -> &Matrix4d {
        &self.z_facing_view_matrix
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_gf::{Matrix4d, Vec3d};

    #[test]
    fn test_screen_window_parameters_default_camera() {
        let camera = Camera::default();
        let params = ScreenWindowParameters::new(&camera, FOVDirection::Horizontal);

        // Check that screen window is computed
        let sw = params.screen_window();
        assert!(sw[0] < sw[1]); // left < right
        assert!(sw[2] < sw[3]); // bottom < top

        // Check FOV is positive
        assert!(params.field_of_view() > 0.0);
    }

    #[test]
    fn test_screen_window_parameters_perspective() {
        let mut camera = Camera::default();
        camera.set_projection(CameraProjection::Perspective);
        camera.set_horizontal_aperture(36.0);
        camera.set_vertical_aperture(24.0);

        let params = ScreenWindowParameters::new(&camera, FOVDirection::Horizontal);

        let sw = params.screen_window();

        // For perspective, screen window is normalized by aperture
        // Should be approximately [-1, 1] horizontally when no offset
        assert!((sw[0] - (-1.0)).abs() < 1e-10);
        assert!((sw[1] - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_screen_window_parameters_orthographic() {
        let mut camera = Camera::default();
        camera.set_projection(CameraProjection::Orthographic);
        camera.set_horizontal_aperture(40.0);
        camera.set_vertical_aperture(30.0);

        let params = ScreenWindowParameters::new(&camera, FOVDirection::Horizontal);

        let sw = params.screen_window();

        // For orthographic, scaled by APERTURE_UNIT / 2.0
        let expected_scale = APERTURE_UNIT / 2.0;
        assert!((sw[0] - (-40.0 * expected_scale)).abs() < 1e-10);
        assert!((sw[1] - (40.0 * expected_scale)).abs() < 1e-10);
    }

    #[test]
    fn test_screen_window_parameters_with_offset() {
        let mut camera = Camera::default();
        camera.set_projection(CameraProjection::Perspective);
        camera.set_horizontal_aperture(36.0);
        camera.set_vertical_aperture(24.0);
        camera.set_horizontal_aperture_offset(5.0);
        camera.set_vertical_aperture_offset(3.0);

        let params = ScreenWindowParameters::new(&camera, FOVDirection::Horizontal);

        let sw = params.screen_window();

        // Window should be shifted by offset
        let h_shift = 2.0 * 5.0 / 36.0; // normalized offset
        assert!((sw[0] - (-1.0 + h_shift)).abs() < 1e-10);
        assert!((sw[1] - (1.0 + h_shift)).abs() < 1e-10);
    }

    #[test]
    fn test_z_facing_view_matrix() {
        let mut camera = Camera::default();
        let transform = Matrix4d::from_translation(Vec3d::new(0.0, 0.0, 10.0));
        camera.set_transform(transform);

        let params = ScreenWindowParameters::new(&camera, FOVDirection::Horizontal);

        let view_matrix = params.z_facing_view_matrix();

        // View matrix should be invertible
        let inv = view_matrix.inverse().expect("Matrix should be invertible");
        let identity = *view_matrix * inv;

        // Check it's close to identity
        for i in 0..4 {
            for j in 0..4 {
                let expected = if i == j { 1.0 } else { 0.0 };
                assert!((identity[i][j] - expected).abs() < 1e-10);
            }
        }
    }
}
