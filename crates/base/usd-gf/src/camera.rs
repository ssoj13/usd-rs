//! Object-based representation of a camera.
//!
//! [`Camera`] provides a thin wrapper on the camera data model,
//! with a small number of computations. It stores physically-based
//! camera parameters (aperture, focal length) that can be converted
//! to a [`Frustum`] for rendering.
//!
//! # Examples
//!
//! ```
//! use usd_gf::{Camera, CameraProjection, FOVDirection};
//!
//! let mut cam = Camera::new();
//! cam.set_focal_length(35.0);
//! cam.set_perspective_from_aspect_ratio_and_fov(16.0 / 9.0, 45.0, FOVDirection::Vertical);
//!
//! let fov = cam.field_of_view(FOVDirection::Horizontal);
//! println!("Horizontal FOV: {} degrees", fov);
//! ```

use crate::frustum::{Frustum, ProjectionType};
use crate::math::{degrees_to_radians, radians_to_degrees};
use crate::matrix4::Matrix4d;
use crate::range::{Range1d, Range1f, Range2d};
use crate::vec2::Vec2d;
use crate::vec4::Vec4f;
use std::fmt;

/// Unit conversion factor for aperture (1/10 of world unit).
///
/// Horizontal and vertical aperture are in 1/10 of the world unit.
/// If the world unit is cm, the aperture unit is mm.
pub const APERTURE_UNIT: f64 = 0.1;

/// Unit conversion factor for focal length (1/10 of world unit).
///
/// Focal length is in 1/10 of the world unit.
/// If the world unit is cm, the focal length unit is mm.
pub const FOCAL_LENGTH_UNIT: f64 = 0.1;

/// Default horizontal aperture based on 35mm projector aperture.
///
/// 0.825 inches converted to cm, then divided by aperture unit (0.1).
/// OpenUSD uses `0.825 * 2.54` (inches to centimeters), not `25.4` (inches to mm).
pub const DEFAULT_HORIZONTAL_APERTURE: f32 = (0.825 * 2.54 / APERTURE_UNIT) as f32;

/// Default vertical aperture based on 35mm projector aperture.
///
/// 0.602 inches converted to cm, then divided by aperture unit (0.1).
/// OpenUSD uses `0.602 * 2.54` (inches to centimeters), not `25.4` (inches to mm).
pub const DEFAULT_VERTICAL_APERTURE: f32 = (0.602 * 2.54 / APERTURE_UNIT) as f32;

/// Projection type for a camera.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CameraProjection {
    /// Perspective projection (default).
    #[default]
    Perspective,
    /// Orthographic (parallel) projection.
    Orthographic,
}

/// Direction for field of view or orthographic size.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FOVDirection {
    /// Horizontal field of view.
    #[default]
    Horizontal,
    /// Vertical field of view.
    Vertical,
}

/// Object-based representation of a camera.
///
/// Stores physically-based camera parameters including transform,
/// aperture, focal length, and clipping planes. Can be converted
/// to a [`Frustum`] for rendering.
#[derive(Clone, Debug)]
pub struct Camera {
    /// Transform of the filmback in world space.
    transform: Matrix4d,
    /// Projection type.
    projection: CameraProjection,
    /// Width of projector aperture (in 1/10 world units, e.g. mm).
    horizontal_aperture: f32,
    /// Height of projector aperture (in 1/10 world units, e.g. mm).
    vertical_aperture: f32,
    /// Horizontal offset of projector aperture.
    horizontal_aperture_offset: f32,
    /// Vertical offset of projector aperture.
    vertical_aperture_offset: f32,
    /// Focal length (in 1/10 world units, e.g. mm).
    focal_length: f32,
    /// Near/far clipping range in world units.
    clipping_range: Range1f,
    /// Additional clipping planes (a,b,c,d) in camera space.
    /// Clips points where a*x + b*y + c*z + d < 0.
    clipping_planes: Vec<Vec4f>,
    /// Lens aperture (f-stop), unitless. 0 = no depth of field.
    f_stop: f32,
    /// Focus distance in world units. 0 = infinity.
    focus_distance: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            transform: Matrix4d::identity(),
            projection: CameraProjection::Perspective,
            horizontal_aperture: DEFAULT_HORIZONTAL_APERTURE,
            vertical_aperture: DEFAULT_VERTICAL_APERTURE,
            horizontal_aperture_offset: 0.0,
            vertical_aperture_offset: 0.0,
            focal_length: 50.0,
            clipping_range: Range1f::new(1.0, 1_000_000.0),
            clipping_planes: Vec::new(),
            f_stop: 0.0,
            focus_distance: 0.0,
        }
    }
}

impl Camera {
    /// Creates a camera with default parameters.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a camera with the given parameters.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn from_params(
        transform: Matrix4d,
        projection: CameraProjection,
        horizontal_aperture: f32,
        vertical_aperture: f32,
        horizontal_aperture_offset: f32,
        vertical_aperture_offset: f32,
        focal_length: f32,
        clipping_range: Range1f,
        clipping_planes: Vec<Vec4f>,
        f_stop: f32,
        focus_distance: f32,
    ) -> Self {
        Self {
            transform,
            projection,
            horizontal_aperture,
            vertical_aperture,
            horizontal_aperture_offset,
            vertical_aperture_offset,
            focal_length,
            clipping_range,
            clipping_planes,
            f_stop,
            focus_distance,
        }
    }

    // ========== Setters ==========

    /// Sets the transform of the filmback in world space.
    #[inline]
    pub fn set_transform(&mut self, transform: Matrix4d) {
        self.transform = transform;
    }

    /// Sets the projection type.
    #[inline]
    pub fn set_projection(&mut self, projection: CameraProjection) {
        self.projection = projection;
    }

    /// Sets the focal length in 1/10 of a world unit (e.g., mm if world unit is cm).
    #[inline]
    pub fn set_focal_length(&mut self, focal_length: f32) {
        self.focal_length = focal_length;
    }

    /// Sets the width of the projector aperture in 1/10 of a world unit.
    #[inline]
    pub fn set_horizontal_aperture(&mut self, aperture: f32) {
        self.horizontal_aperture = aperture;
    }

    /// Sets the height of the projector aperture in 1/10 of a world unit.
    #[inline]
    pub fn set_vertical_aperture(&mut self, aperture: f32) {
        self.vertical_aperture = aperture;
    }

    /// Sets the horizontal offset of the projector aperture.
    #[inline]
    pub fn set_horizontal_aperture_offset(&mut self, offset: f32) {
        self.horizontal_aperture_offset = offset;
    }

    /// Sets the vertical offset of the projector aperture.
    #[inline]
    pub fn set_vertical_aperture_offset(&mut self, offset: f32) {
        self.vertical_aperture_offset = offset;
    }

    /// Sets up a perspective camera from aspect ratio and field of view.
    ///
    /// Similar to `gluPerspective` when direction is `Vertical`.
    ///
    /// # Arguments
    ///
    /// * `aspect_ratio` - Width / height ratio
    /// * `field_of_view` - Field of view in degrees
    /// * `direction` - Whether FOV is horizontal or vertical
    /// * `horizontal_aperture` - Optional aperture (for depth of field)
    pub fn set_perspective_from_aspect_ratio_and_fov(
        &mut self,
        aspect_ratio: f32,
        field_of_view: f32,
        direction: FOVDirection,
    ) {
        self.set_perspective_from_aspect_ratio_and_fov_with_aperture(
            aspect_ratio,
            field_of_view,
            direction,
            DEFAULT_HORIZONTAL_APERTURE,
        );
    }

    /// Sets up a perspective camera from aspect ratio, field of view, and aperture.
    pub fn set_perspective_from_aspect_ratio_and_fov_with_aperture(
        &mut self,
        aspect_ratio: f32,
        field_of_view: f32,
        direction: FOVDirection,
        horizontal_aperture: f32,
    ) {
        self.projection = CameraProjection::Perspective;

        // Set apertures to achieve the aspect ratio
        self.horizontal_aperture = horizontal_aperture;
        let safe_aspect = if aspect_ratio != 0.0 {
            aspect_ratio
        } else {
            1.0
        };
        self.vertical_aperture = horizontal_aperture / safe_aspect;

        // Pick aperture based on direction
        let aperture = match direction {
            FOVDirection::Horizontal => self.horizontal_aperture,
            FOVDirection::Vertical => self.vertical_aperture,
        };

        // Compute focal length from FOV
        let tan_value = (0.5 * degrees_to_radians(field_of_view as f64)).tan();

        if tan_value == 0.0 {
            self.focal_length = 50.0;
            return;
        }

        self.focal_length =
            ((aperture as f64 * APERTURE_UNIT) / (2.0 * tan_value) / FOCAL_LENGTH_UNIT) as f32;
    }

    /// Sets up an orthographic camera from aspect ratio and size.
    ///
    /// # Arguments
    ///
    /// * `aspect_ratio` - Width / height ratio
    /// * `orthographic_size` - Width or height in world units (cm)
    /// * `direction` - Whether size is horizontal or vertical
    pub fn set_orthographic_from_aspect_ratio_and_size(
        &mut self,
        aspect_ratio: f32,
        orthographic_size: f32,
        direction: FOVDirection,
    ) {
        self.projection = CameraProjection::Orthographic;
        self.focal_length = 50.0; // Not used, but set to sane value

        match direction {
            FOVDirection::Horizontal => {
                self.horizontal_aperture = orthographic_size / APERTURE_UNIT as f32;
                self.vertical_aperture = if aspect_ratio > 0.0 {
                    self.horizontal_aperture / aspect_ratio
                } else {
                    self.horizontal_aperture
                };
            }
            FOVDirection::Vertical => {
                self.vertical_aperture = orthographic_size / APERTURE_UNIT as f32;
                self.horizontal_aperture = self.vertical_aperture * aspect_ratio;
            }
        }
    }

    /// Sets the camera from view and projection matrices.
    ///
    /// Note: The projection matrix only determines the ratio of aperture
    /// to focal length, so a default focal length is used.
    pub fn set_from_view_and_projection_matrix(
        &mut self,
        view_matrix: &Matrix4d,
        proj_matrix: &Matrix4d,
        focal_length: f32,
    ) {
        self.transform = view_matrix.inverse().unwrap_or_else(Matrix4d::identity);
        self.focal_length = focal_length;

        // Check if perspective (proj[2][3] should be -1 for perspective)
        if proj_matrix[2][3] < -0.5 {
            self.projection = CameraProjection::Perspective;

            let aperture_base = 2.0 * focal_length as f64 * (FOCAL_LENGTH_UNIT / APERTURE_UNIT);

            self.horizontal_aperture = (aperture_base / proj_matrix[0][0]) as f32;
            self.vertical_aperture = (aperture_base / proj_matrix[1][1]) as f32;
            self.horizontal_aperture_offset =
                (0.5 * self.horizontal_aperture as f64 * proj_matrix[2][0]) as f32;
            self.vertical_aperture_offset =
                (0.5 * self.vertical_aperture as f64 * proj_matrix[2][1]) as f32;

            // Extract clipping range from perspective matrix
            let near = proj_matrix[3][2] / (proj_matrix[2][2] - 1.0);
            let far = proj_matrix[3][2] / (proj_matrix[2][2] + 1.0);
            self.clipping_range = Range1f::new(near as f32, far as f32);
        } else {
            self.projection = CameraProjection::Orthographic;

            self.horizontal_aperture = ((2.0 / APERTURE_UNIT) / proj_matrix[0][0]) as f32;
            self.vertical_aperture = ((2.0 / APERTURE_UNIT) / proj_matrix[1][1]) as f32;
            self.horizontal_aperture_offset =
                (-0.5 * self.horizontal_aperture as f64 * proj_matrix[3][0]) as f32;
            self.vertical_aperture_offset =
                (-0.5 * self.vertical_aperture as f64 * proj_matrix[3][1]) as f32;

            // Extract clipping range from orthographic matrix
            let near_minus_far_half = 1.0 / proj_matrix[2][2];
            let near_plus_far_half = near_minus_far_half * proj_matrix[3][2];
            let near = near_plus_far_half + near_minus_far_half;
            let far = near_plus_far_half - near_minus_far_half;
            self.clipping_range = Range1f::new(near as f32, far as f32);
        }
    }

    /// Sets the clipping range in world units.
    #[inline]
    pub fn set_clipping_range(&mut self, range: Range1f) {
        self.clipping_range = range;
    }

    /// Sets additional clipping planes.
    ///
    /// Each Vec4f(a,b,c,d) encodes a plane that clips off points (x,y,z) where:
    /// `a*x + b*y + c*z + d*1 < 0`
    ///
    /// Coordinates are in camera space.
    #[inline]
    pub fn set_clipping_planes(&mut self, planes: Vec<Vec4f>) {
        self.clipping_planes = planes;
    }

    /// Sets the lens aperture (f-stop), unitless.
    ///
    /// 0 means no depth of field effect.
    #[inline]
    pub fn set_f_stop(&mut self, f_stop: f32) {
        self.f_stop = f_stop;
    }

    /// Sets the focus distance in world units.
    ///
    /// 0 means infinity.
    #[inline]
    pub fn set_focus_distance(&mut self, distance: f32) {
        self.focus_distance = distance;
    }

    // ========== Getters ==========

    /// Returns the transform of the filmback in world space.
    #[inline]
    #[must_use]
    pub fn transform(&self) -> &Matrix4d {
        &self.transform
    }

    /// Returns the projection type.
    #[inline]
    #[must_use]
    pub fn projection(&self) -> CameraProjection {
        self.projection
    }

    /// Returns the width of the projector aperture in 1/10 world units.
    #[inline]
    #[must_use]
    pub fn horizontal_aperture(&self) -> f32 {
        self.horizontal_aperture
    }

    /// Returns the height of the projector aperture in 1/10 world units.
    #[inline]
    #[must_use]
    pub fn vertical_aperture(&self) -> f32 {
        self.vertical_aperture
    }

    /// Returns the horizontal offset of the projector aperture.
    #[inline]
    #[must_use]
    pub fn horizontal_aperture_offset(&self) -> f32 {
        self.horizontal_aperture_offset
    }

    /// Returns the vertical offset of the projector aperture.
    #[inline]
    #[must_use]
    pub fn vertical_aperture_offset(&self) -> f32 {
        self.vertical_aperture_offset
    }

    /// Returns the projector aperture aspect ratio.
    #[inline]
    #[must_use]
    pub fn aspect_ratio(&self) -> f32 {
        if self.vertical_aperture == 0.0 {
            0.0
        } else {
            self.horizontal_aperture / self.vertical_aperture
        }
    }

    /// Returns the focal length in 1/10 world units.
    #[inline]
    #[must_use]
    pub fn focal_length(&self) -> f32 {
        self.focal_length
    }

    /// Returns the horizontal or vertical field of view in degrees.
    #[must_use]
    pub fn field_of_view(&self, direction: FOVDirection) -> f32 {
        let aperture = match direction {
            FOVDirection::Horizontal => self.horizontal_aperture,
            FOVDirection::Vertical => self.vertical_aperture,
        };

        let fov_rad = 2.0
            * ((aperture as f64 * APERTURE_UNIT)
                / (2.0 * self.focal_length as f64 * FOCAL_LENGTH_UNIT))
                .atan();

        radians_to_degrees(fov_rad) as f32
    }

    /// Returns the clipping range in world units.
    #[inline]
    #[must_use]
    pub fn clipping_range(&self) -> Range1f {
        self.clipping_range
    }

    /// Returns additional clipping planes.
    #[inline]
    #[must_use]
    pub fn clipping_planes(&self) -> &[Vec4f] {
        &self.clipping_planes
    }

    /// Returns the lens aperture (f-stop).
    #[inline]
    #[must_use]
    pub fn f_stop(&self) -> f32 {
        self.f_stop
    }

    /// Returns the focus distance in world units.
    #[inline]
    #[must_use]
    pub fn focus_distance(&self) -> f32 {
        self.focus_distance
    }

    /// Returns the computed world-space camera frustum.
    ///
    /// The frustum is that of a Y-up, -Z-looking camera.
    #[must_use]
    pub fn frustum(&self) -> Frustum {
        // Build window from aperture
        let max = Vec2d::new(
            self.horizontal_aperture as f64 / 2.0,
            self.vertical_aperture as f64 / 2.0,
        );
        let mut window = Range2d::new(Vec2d::new(-max.x, -max.y), max);

        // Apply aperture offset
        let offset = Vec2d::new(
            self.horizontal_aperture_offset as f64,
            self.vertical_aperture_offset as f64,
        );
        window = Range2d::new(
            Vec2d::new(window.min().x + offset.x, window.min().y + offset.y),
            Vec2d::new(window.max().x + offset.x, window.max().y + offset.y),
        );

        // Convert from mm to cm (aperture unit)
        window *= APERTURE_UNIT;

        // For perspective, divide by focal length to get normalized window
        if self.projection != CameraProjection::Orthographic && self.focal_length != 0.0 {
            window /= self.focal_length as f64 * FOCAL_LENGTH_UNIT;
        }

        // Build clipping range
        let clipping_range = Range1d::new(
            self.clipping_range.min() as f64,
            self.clipping_range.max() as f64,
        );

        // Convert projection type
        let projection_type = match self.projection {
            CameraProjection::Orthographic => ProjectionType::Orthographic,
            CameraProjection::Perspective => ProjectionType::Perspective,
        };

        // Create frustum from transform matrix
        Frustum::from_matrix(
            &self.transform,
            window,
            clipping_range,
            projection_type,
            5.0, // Default view distance
        )
    }
}

impl PartialEq for Camera {
    fn eq(&self, other: &Self) -> bool {
        self.transform == other.transform
            && self.projection == other.projection
            && self.horizontal_aperture == other.horizontal_aperture
            && self.vertical_aperture == other.vertical_aperture
            && self.horizontal_aperture_offset == other.horizontal_aperture_offset
            && self.vertical_aperture_offset == other.vertical_aperture_offset
            && self.focal_length == other.focal_length
            && self.clipping_range == other.clipping_range
            && self.clipping_planes == other.clipping_planes
            && self.f_stop == other.f_stop
            && self.focus_distance == other.focus_distance
    }
}

impl Eq for Camera {}

impl fmt::Display for Camera {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Camera(projection={:?}, aperture={}x{}, focal={}, clip={}..{})",
            self.projection,
            self.horizontal_aperture,
            self.vertical_aperture,
            self.focal_length,
            self.clipping_range.min(),
            self.clipping_range.max()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vec3d;

    const EPSILON: f32 = 1e-4;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < EPSILON
    }

    #[test]
    fn test_default_camera() {
        let cam = Camera::new();
        assert_eq!(cam.projection(), CameraProjection::Perspective);
        assert!(approx_eq(cam.focal_length(), 50.0));
        assert!(approx_eq(
            cam.horizontal_aperture(),
            DEFAULT_HORIZONTAL_APERTURE
        ));
        assert!(approx_eq(
            cam.vertical_aperture(),
            DEFAULT_VERTICAL_APERTURE
        ));
    }

    #[test]
    fn test_aspect_ratio() {
        let cam = Camera::new();
        let expected = DEFAULT_HORIZONTAL_APERTURE / DEFAULT_VERTICAL_APERTURE;
        assert!(approx_eq(cam.aspect_ratio(), expected));
    }

    #[test]
    fn test_field_of_view() {
        let mut cam = Camera::new();
        cam.set_horizontal_aperture(36.0);
        cam.set_vertical_aperture(24.0);
        cam.set_focal_length(50.0);

        // FOV = 2 * atan(aperture * 0.1 / (2 * focal * 0.1))
        // Horizontal: 2 * atan(36 * 0.1 / (2 * 50 * 0.1)) = 2 * atan(3.6 / 10)
        let h_fov = cam.field_of_view(FOVDirection::Horizontal);
        let v_fov = cam.field_of_view(FOVDirection::Vertical);

        // Horizontal FOV should be larger than vertical for landscape sensor
        assert!(h_fov > v_fov);
        assert!(h_fov > 0.0 && h_fov < 180.0);
    }

    #[test]
    fn test_perspective_setup() {
        let mut cam = Camera::new();
        cam.set_perspective_from_aspect_ratio_and_fov(16.0 / 9.0, 90.0, FOVDirection::Horizontal);

        assert_eq!(cam.projection(), CameraProjection::Perspective);
        // After setup, horizontal FOV should be approximately 90 degrees
        let h_fov = cam.field_of_view(FOVDirection::Horizontal);
        assert!((h_fov - 90.0).abs() < 1.0); // Within 1 degree tolerance
    }

    #[test]
    fn test_orthographic_setup() {
        let mut cam = Camera::new();
        cam.set_orthographic_from_aspect_ratio_and_size(2.0, 10.0, FOVDirection::Horizontal);

        assert_eq!(cam.projection(), CameraProjection::Orthographic);
        // Horizontal aperture should be 10.0 / APERTURE_UNIT = 100
        assert!(approx_eq(cam.horizontal_aperture(), 100.0));
        // Vertical aperture = 100 / 2 = 50
        assert!(approx_eq(cam.vertical_aperture(), 50.0));
    }

    #[test]
    fn test_clipping_range() {
        let mut cam = Camera::new();
        cam.set_clipping_range(Range1f::new(0.1, 1000.0));
        assert!(approx_eq(cam.clipping_range().min(), 0.1));
        assert!(approx_eq(cam.clipping_range().max(), 1000.0));
    }

    #[test]
    fn test_transform() {
        let mut cam = Camera::new();
        let xform = Matrix4d::from_translation(vec3d(10.0, 20.0, 30.0));
        cam.set_transform(xform);
        assert_eq!(*cam.transform(), xform);
    }

    #[test]
    fn test_frustum_creation() {
        let cam = Camera::new();
        let frustum = cam.frustum();
        assert_eq!(frustum.projection_type(), ProjectionType::Perspective);
    }

    #[test]
    fn test_equality() {
        let cam1 = Camera::new();
        let cam2 = Camera::new();
        assert_eq!(cam1, cam2);

        let mut cam3 = Camera::new();
        cam3.set_focal_length(35.0);
        assert_ne!(cam1, cam3);
    }

    #[test]
    fn test_display() {
        let cam = Camera::new();
        let s = format!("{}", cam);
        assert!(s.contains("Camera"));
        assert!(s.contains("Perspective"));
    }

    #[test]
    fn test_depth_of_field_params() {
        let mut cam = Camera::new();
        cam.set_f_stop(2.8);
        cam.set_focus_distance(5.0);
        assert!(approx_eq(cam.f_stop(), 2.8));
        assert!(approx_eq(cam.focus_distance(), 5.0));
    }

    #[test]
    fn test_clipping_planes() {
        let mut cam = Camera::new();
        let planes = vec![
            Vec4f::new(1.0, 0.0, 0.0, 5.0),
            Vec4f::new(0.0, 1.0, 0.0, 5.0),
        ];
        cam.set_clipping_planes(planes.clone());
        assert_eq!(cam.clipping_planes().len(), 2);
        assert_eq!(cam.clipping_planes()[0], planes[0]);
    }

    #[test]
    fn test_aperture_offsets() {
        let mut cam = Camera::new();
        cam.set_horizontal_aperture_offset(2.0);
        cam.set_vertical_aperture_offset(1.5);
        assert!(approx_eq(cam.horizontal_aperture_offset(), 2.0));
        assert!(approx_eq(cam.vertical_aperture_offset(), 1.5));
    }
}
