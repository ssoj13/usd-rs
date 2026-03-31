//! View frustum for perspective and orthographic projection.
//!
//! [`Frustum`] represents a viewing frustum defined by:
//! - Position of the viewpoint
//! - Rotation from default frame (looking along -Z, +Y up)
//! - 2D window on the reference plane
//! - Near/far distances
//! - Projection type (perspective or orthographic)
//!
//! # Examples
//!
//! ```
//! use usd_gf::{Frustum, ProjectionType, vec3d};
//!
//! let frustum = Frustum::new();
//! assert_eq!(frustum.projection_type(), ProjectionType::Perspective);
//! ```

use crate::bbox3d::BBox3d;
use crate::math::{degrees_to_radians, radians_to_degrees};
use crate::matrix4::Matrix4d;
use crate::ostream_helpers::ostream_helper_p_double;
use crate::plane::Plane;
use crate::range::{Range1d, Range2d};
use crate::ray::Ray;
use crate::rotation::Rotation;
use crate::vec2::Vec2d;
use crate::vec3::Vec3d;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::RwLock;

/// Map normalized window coords [-1,1] to window rect. Per C++ _WindowNormalizedToPoint.
fn window_normalized_to_point(window_pos: Vec2d, window: &Range2d) -> Vec2d {
    let scaled = Vec2d::new(0.5 * (1.0 + window_pos.x), 0.5 * (1.0 + window_pos.y));
    *window.min() + Vec2d::new(scaled.x * window.size().x, scaled.y * window.size().y)
}

/// Projection type for a frustum.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ProjectionType {
    /// Orthographic (parallel) projection.
    Orthographic,
    /// Perspective projection.
    #[default]
    Perspective,
}

/// Reference plane depth (always 1.0).
pub const REFERENCE_PLANE_DEPTH: f64 = 1.0;

/// A viewing frustum.
///
/// Stores position, rotation, window rectangle, near/far planes,
/// and projection type. Can compute view/projection matrices,
/// corners, rays, and intersection tests.
#[derive(Debug)]
pub struct Frustum {
    /// Position of the viewpoint in world space.
    position: Vec3d,
    /// Rotation from default frame (-Z view, +Y up).
    rotation: Rotation,
    /// Window rectangle on reference plane.
    window: Range2d,
    /// Near/far distances.
    near_far: Range1d,
    /// View distance (for look-at computation).
    view_distance: f64,
    /// Projection type.
    projection_type: ProjectionType,
    /// Cached frustum planes (lazily computed). Per C++ atomic pointer.
    planes: RwLock<Option<[Plane; 6]>>,
}

impl Default for Frustum {
    /// Creates a frustum with default parameters:
    /// - Position at origin
    /// - Identity rotation (looking along -Z)
    /// - Window: [-1, 1] x [-1, 1]
    /// - Near/far: [1, 10]
    /// - View distance: 5.0
    /// - Perspective projection
    fn default() -> Self {
        Self {
            position: Vec3d::new(0.0, 0.0, 0.0),
            rotation: Rotation::new(),
            window: Range2d::new(Vec2d::new(-1.0, -1.0), Vec2d::new(1.0, 1.0)),
            near_far: Range1d::new(1.0, 10.0),
            view_distance: 5.0,
            projection_type: ProjectionType::Perspective,
            planes: RwLock::new(None),
        }
    }
}

impl Frustum {
    /// Returns the depth of the reference plane (1.0).
    /// Matches C++ GfFrustum::GetReferencePlaneDepth().
    #[inline]
    #[must_use]
    pub fn get_reference_plane_depth() -> f64 {
        REFERENCE_PLANE_DEPTH
    }

    /// Creates a frustum with default parameters.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a frustum with the given parameters.
    #[must_use]
    pub fn from_params(
        position: Vec3d,
        rotation: Rotation,
        window: Range2d,
        near_far: Range1d,
        projection_type: ProjectionType,
        view_distance: f64,
    ) -> Self {
        Self {
            position,
            rotation,
            window,
            near_far,
            view_distance,
            projection_type,
            planes: RwLock::new(None),
        }
    }

    /// Creates a frustum from a camera-to-world matrix.
    #[must_use]
    pub fn from_matrix(
        cam_to_world: &Matrix4d,
        window: Range2d,
        near_far: Range1d,
        projection_type: ProjectionType,
        view_distance: f64,
    ) -> Self {
        let mut frustum = Self {
            position: Vec3d::new(0.0, 0.0, 0.0),
            rotation: Rotation::new(),
            window,
            near_far,
            view_distance,
            projection_type,
            planes: RwLock::new(None),
        };
        frustum.set_position_and_rotation_from_matrix(cam_to_world);
        frustum
    }

    // ========== Setters ==========

    /// Sets the position of the frustum.
    pub fn set_position(&mut self, position: Vec3d) {
        self.position = position;
        self.dirty_planes();
    }

    /// Sets the rotation of the frustum.
    pub fn set_rotation(&mut self, rotation: Rotation) {
        self.rotation = rotation;
        self.dirty_planes();
    }

    /// Sets position and rotation from a camera-to-world matrix.
    ///
    /// First conforms matrix to right-handed and orthonormal (per C++ GfFrustum),
    /// then extracts position and rotation.
    pub fn set_position_and_rotation_from_matrix(&mut self, cam_to_world: &Matrix4d) {
        let mut conformed = *cam_to_world;
        if !conformed.is_right_handed() {
            let flip = Matrix4d::from_scale_vec(&Vec3d::new(-1.0, 1.0, 1.0));
            conformed = flip * conformed;
        }
        conformed.orthonormalize();
        self.position = conformed.extract_translation();
        self.rotation = conformed.extract_rotation();
        self.dirty_planes();
    }

    /// Sets the window rectangle.
    pub fn set_window(&mut self, window: Range2d) {
        self.window = window;
        self.dirty_planes();
    }

    /// Sets the near/far interval.
    pub fn set_near_far(&mut self, near_far: Range1d) {
        self.near_far = near_far;
        self.dirty_planes();
    }

    /// Sets the view distance.
    #[inline]
    pub fn set_view_distance(&mut self, view_distance: f64) {
        self.view_distance = view_distance;
    }

    /// Sets the projection type.
    pub fn set_projection_type(&mut self, projection_type: ProjectionType) {
        self.projection_type = projection_type;
        self.dirty_planes();
    }

    // ========== Getters ==========

    /// Returns the position.
    #[inline]
    #[must_use]
    pub fn position(&self) -> Vec3d {
        self.position
    }

    /// Returns the rotation.
    #[inline]
    #[must_use]
    pub fn rotation(&self) -> &Rotation {
        &self.rotation
    }

    /// Returns the window rectangle.
    #[inline]
    #[must_use]
    pub fn window(&self) -> &Range2d {
        &self.window
    }

    /// Returns the near/far interval.
    #[inline]
    #[must_use]
    pub fn near_far(&self) -> &Range1d {
        &self.near_far
    }

    /// Returns the view distance.
    #[inline]
    #[must_use]
    pub fn view_distance(&self) -> f64 {
        self.view_distance
    }

    /// Returns the projection type.
    #[inline]
    #[must_use]
    pub fn projection_type(&self) -> ProjectionType {
        self.projection_type
    }

    // ========== Perspective setup ==========

    /// Sets up a symmetric perspective frustum.
    ///
    /// Sets up the frustum similar to gluPerspective(), using vertical FOV.
    /// Convenience overload matching C++ SetPerspective(fieldOfViewHeight, aspectRatio, near, far).
    pub fn set_perspective_from_fov_height(
        &mut self,
        field_of_view_height: f64,
        aspect_ratio: f64,
        near_distance: f64,
        far_distance: f64,
    ) {
        self.set_perspective(
            field_of_view_height,
            true, // is_fov_vertical
            aspect_ratio,
            near_distance,
            far_distance,
        );
    }

    /// # Arguments
    /// * `fov` - Field of view in degrees
    /// * `is_fov_vertical` - True if FOV is vertical, false if horizontal
    /// * `aspect_ratio` - Width / height
    /// * `near` - Near plane distance
    /// * `far` - Far plane distance
    pub fn set_perspective(
        &mut self,
        fov: f64,
        is_fov_vertical: bool,
        aspect_ratio: f64,
        near: f64,
        far: f64,
    ) {
        self.projection_type = ProjectionType::Perspective;

        let aspect = if aspect_ratio == 0.0 {
            1.0
        } else {
            aspect_ratio
        };

        let (x_dist, y_dist) = if is_fov_vertical {
            let y = (degrees_to_radians(fov / 2.0)).tan() * REFERENCE_PLANE_DEPTH;
            (y * aspect, y)
        } else {
            let x = (degrees_to_radians(fov / 2.0)).tan() * REFERENCE_PLANE_DEPTH;
            (x, x / aspect)
        };

        self.window = Range2d::new(Vec2d::new(-x_dist, -y_dist), Vec2d::new(x_dist, y_dist));
        self.near_far = Range1d::new(near, far);
        self.dirty_planes();
    }

    /// Gets the perspective parameters.
    ///
    /// Returns `None` if not a perspective frustum.
    #[must_use]
    pub fn get_perspective(&self, is_fov_vertical: bool) -> Option<(f64, f64, f64, f64)> {
        if self.projection_type != ProjectionType::Perspective {
            return None;
        }

        let win_size = self.window.size();
        let fov = if is_fov_vertical {
            2.0 * radians_to_degrees((win_size.y / (2.0 * REFERENCE_PLANE_DEPTH)).atan())
        } else {
            2.0 * radians_to_degrees((win_size.x / (2.0 * REFERENCE_PLANE_DEPTH)).atan())
        };

        let aspect = win_size.x / win_size.y;
        Some((fov, aspect, self.near_far.min(), self.near_far.max()))
    }

    /// Returns the field of view in degrees.
    ///
    /// Returns 0.0 if not a perspective frustum.
    #[must_use]
    pub fn get_fov(&self, is_fov_vertical: bool) -> f64 {
        self.get_perspective(is_fov_vertical)
            .map(|(fov, _, _, _)| fov)
            .unwrap_or(0.0)
    }

    // ========== Orthographic setup ==========

    /// Sets up an orthographic frustum.
    pub fn set_orthographic(
        &mut self,
        left: f64,
        right: f64,
        bottom: f64,
        top: f64,
        near: f64,
        far: f64,
    ) {
        self.projection_type = ProjectionType::Orthographic;
        self.window = Range2d::new(Vec2d::new(left, bottom), Vec2d::new(right, top));
        self.near_far = Range1d::new(near, far);
        self.dirty_planes();
    }

    /// Gets the orthographic parameters.
    ///
    /// Returns `None` if not an orthographic frustum.
    #[must_use]
    pub fn get_orthographic(&self) -> Option<(f64, f64, f64, f64, f64, f64)> {
        if self.projection_type != ProjectionType::Orthographic {
            return None;
        }

        Some((
            self.window.min().x,
            self.window.max().x,
            self.window.min().y,
            self.window.max().y,
            self.near_far.min(),
            self.near_far.max(),
        ))
    }

    // ========== Computed values ==========

    /// Computes the view direction (normalized).
    #[must_use]
    pub fn compute_view_direction(&self) -> Vec3d {
        // Default view is along -Z
        self.rotation.transform_dir(&Vec3d::new(0.0, 0.0, -1.0))
    }

    /// Computes the up vector (normalized).
    #[must_use]
    pub fn compute_up_vector(&self) -> Vec3d {
        // Default up is +Y
        self.rotation.transform_dir(&Vec3d::new(0.0, 1.0, 0.0))
    }

    /// Computes the view frame (side, up, view).
    #[must_use]
    pub fn compute_view_frame(&self) -> (Vec3d, Vec3d, Vec3d) {
        let view = self.compute_view_direction();
        let up = self.compute_up_vector();
        let side = view.cross(&up).normalized();
        (side, up, view)
    }

    /// Computes the look-at point from position, rotation, and view distance.
    #[must_use]
    pub fn compute_look_at_point(&self) -> Vec3d {
        self.position + self.compute_view_direction() * self.view_distance
    }

    /// Computes the aspect ratio (width / height).
    /// Uses fabs to match C++; negative window sizes (e.g. env cubes) yield positive ratio.
    #[must_use]
    pub fn compute_aspect_ratio(&self) -> f64 {
        let size = self.window.size();
        if size.y == 0.0 {
            0.0
        } else {
            (size.x / size.y).abs()
        }
    }

    /// Computes the view matrix (world to eye space).
    #[must_use]
    pub fn compute_view_matrix(&self) -> Matrix4d {
        // C++ frustum.cpp SetLookAt: view = T(-eye) * R(rot_inv)
        // In USD row-major convention, A*B means apply A first.
        // So translate first (move world to origin), then rotate.
        let rot_inv = self.rotation.inverse();
        let rot_mat = Matrix4d::from_rotation(rot_inv.axis(), degrees_to_radians(rot_inv.angle()));
        let trans_mat = Matrix4d::from_translation(-self.position);

        trans_mat * rot_mat
    }

    /// Computes the inverse view matrix (eye to world space).
    #[must_use]
    pub fn compute_view_inverse(&self) -> Matrix4d {
        // Inverse of view: R(rot) * T(pos) — rotate first, then translate
        let rot_mat = Matrix4d::from_rotation(
            self.rotation.axis(),
            degrees_to_radians(self.rotation.angle()),
        );
        let trans_mat = Matrix4d::from_translation(self.position);

        rot_mat * trans_mat
    }

    /// Computes the projection matrix (GL-style).
    #[must_use]
    pub fn compute_projection_matrix(&self) -> Matrix4d {
        let l = self.window.min().x;
        let r = self.window.max().x;
        let b = self.window.min().y;
        let t = self.window.max().y;
        let n = self.near_far.min();
        let f = self.near_far.max();

        match self.projection_type {
            ProjectionType::Perspective => {
                // Scale window from reference plane to near plane
                let scale = n / REFERENCE_PLANE_DEPTH;
                let l = l * scale;
                let r = r * scale;
                let b = b * scale;
                let t = t * scale;

                Matrix4d::new(
                    2.0 * n / (r - l),
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    2.0 * n / (t - b),
                    0.0,
                    0.0,
                    (r + l) / (r - l),
                    (t + b) / (t - b),
                    -(f + n) / (f - n),
                    -1.0,
                    0.0,
                    0.0,
                    -2.0 * f * n / (f - n),
                    0.0,
                )
            }
            ProjectionType::Orthographic => Matrix4d::new(
                2.0 / (r - l),
                0.0,
                0.0,
                0.0,
                0.0,
                2.0 / (t - b),
                0.0,
                0.0,
                0.0,
                0.0,
                -2.0 / (f - n),
                0.0,
                -(r + l) / (r - l),
                -(t + b) / (t - b),
                -(f + n) / (f - n),
                1.0,
            ),
        }
    }

    /// Computes the 8 corners of the frustum in world space.
    ///
    /// Order: LBN, RBN, LTN, RTN, LBF, RBF, LTF, RTF
    /// (L=left, R=right, B=bottom, T=top, N=near, F=far)
    #[must_use]
    pub fn compute_corners(&self) -> [Vec3d; 8] {
        let near_corners = self.compute_corners_at_distance(self.near_far.min());
        let far_corners = self.compute_corners_at_distance(self.near_far.max());

        [
            near_corners[0],
            near_corners[1],
            near_corners[2],
            near_corners[3],
            far_corners[0],
            far_corners[1],
            far_corners[2],
            far_corners[3],
        ]
    }

    /// Computes the 4 corners at a given distance from the viewpoint.
    ///
    /// Order: LB, RB, LT, RT (L=left, R=right, B=bottom, T=top)
    #[must_use]
    pub fn compute_corners_at_distance(&self, distance: f64) -> [Vec3d; 4] {
        let view_inv = self.compute_view_inverse();

        let (l, r, b, t) = match self.projection_type {
            ProjectionType::Perspective => {
                let scale = distance / REFERENCE_PLANE_DEPTH;
                (
                    self.window.min().x * scale,
                    self.window.max().x * scale,
                    self.window.min().y * scale,
                    self.window.max().y * scale,
                )
            }
            ProjectionType::Orthographic => (
                self.window.min().x,
                self.window.max().x,
                self.window.min().y,
                self.window.max().y,
            ),
        };

        // Corners in camera space (looking along -Z)
        let corners_cam = [
            Vec3d::new(l, b, -distance),
            Vec3d::new(r, b, -distance),
            Vec3d::new(l, t, -distance),
            Vec3d::new(r, t, -distance),
        ];

        // Transform to world space
        [
            view_inv.transform_point(&corners_cam[0]),
            view_inv.transform_point(&corners_cam[1]),
            view_inv.transform_point(&corners_cam[2]),
            view_inv.transform_point(&corners_cam[3]),
        ]
    }

    /// Computes a ray from the viewpoint through a window position.
    ///
    /// Window position in normalized coords [-1, +1]. Per C++ ComputeRay(GfVec2d).
    #[must_use]
    pub fn compute_ray_from_window(&self, window_pos: Vec2d) -> Ray {
        let window_point = window_normalized_to_point(window_pos, &self.window);
        let near = self.near_far.min();
        let (cam_pos, cam_dir) = match self.projection_type {
            ProjectionType::Perspective => {
                let dir = Vec3d::new(window_point.x, window_point.y, -1.0).normalized();
                (Vec3d::zero(), dir)
            }
            ProjectionType::Orthographic => {
                let pos = Vec3d::new(window_point.x, window_point.y, -near);
                (pos, Vec3d::new(0.0, 0.0, -1.0))
            }
        };
        let view_inv = self.compute_view_inverse();
        let origin = view_inv.transform_point(&cam_pos);
        let direction = view_inv.transform_dir(&cam_dir);
        Ray::new(origin, direction)
    }

    /// Computes a ray from the viewpoint to a world-space point.
    /// Per C++ ComputeRay(GfVec3d).
    #[must_use]
    pub fn compute_ray_from_world(&self, world_space_pos: Vec3d) -> Ray {
        let view = self.compute_view_matrix();
        let cam_space = view.transform_point(&world_space_pos);
        let (cam_pos, cam_dir) = match self.projection_type {
            ProjectionType::Perspective => (
                Vec3d::zero(),
                Vec3d::new(cam_space.x, cam_space.y, cam_space.z).normalized(),
            ),
            ProjectionType::Orthographic => (
                Vec3d::new(cam_space.x, cam_space.y, 0.0),
                Vec3d::new(0.0, 0.0, -1.0),
            ),
        };
        let view_inv = self.compute_view_inverse();
        let origin = view_inv.transform_point(&cam_pos);
        let direction = view_inv.transform_dir(&cam_dir);
        Ray::new(origin, direction)
    }

    /// Backward-compatible alias for `compute_ray_from_window`.
    #[must_use]
    pub fn compute_ray(&self, window_pos: Vec2d) -> Ray {
        self.compute_ray_from_window(window_pos)
    }

    /// Computes a pick ray from near plane. Per C++ _ComputePickRayOffsetToNearPlane.
    fn compute_pick_ray_offset_to_near(&self, cam_from: Vec3d, cam_dir: Vec3d) -> Ray {
        let near = self.near_far.min();
        let ray_from = cam_from + cam_dir * near;
        let view_inv = self.compute_view_inverse();
        let origin = view_inv.transform_point(&ray_from);
        let direction = view_inv.transform_dir(&cam_dir);
        Ray::new(origin, direction)
    }

    /// Computes a pick ray from near plane through window position.
    #[must_use]
    pub fn compute_pick_ray_from_window(&self, window_pos: Vec2d) -> Ray {
        let window_point = window_normalized_to_point(window_pos, &self.window);
        let near = self.near_far.min();
        let (cam_pos, cam_dir) = match self.projection_type {
            ProjectionType::Perspective => {
                let dir = Vec3d::new(window_point.x, window_point.y, -1.0).normalized();
                (Vec3d::zero(), dir)
            }
            ProjectionType::Orthographic => (
                Vec3d::new(window_point.x, window_point.y, -near),
                Vec3d::new(0.0, 0.0, -1.0),
            ),
        };
        self.compute_pick_ray_offset_to_near(cam_pos, cam_dir)
    }

    /// Computes a pick ray from near plane through a world-space point.
    #[must_use]
    pub fn compute_pick_ray_from_world(&self, world_space_pos: Vec3d) -> Ray {
        let view = self.compute_view_matrix();
        let cam_space = view.transform_point(&world_space_pos);
        let (cam_pos, cam_dir) = match self.projection_type {
            ProjectionType::Perspective => (
                Vec3d::zero(),
                Vec3d::new(cam_space.x, cam_space.y, cam_space.z).normalized(),
            ),
            ProjectionType::Orthographic => (
                Vec3d::new(cam_space.x, cam_space.y, 0.0),
                Vec3d::new(0.0, 0.0, -1.0),
            ),
        };
        self.compute_pick_ray_offset_to_near(cam_pos, cam_dir)
    }

    /// Backward-compatible alias for `compute_pick_ray_from_window`.
    #[must_use]
    pub fn compute_pick_ray(&self, window_pos: Vec2d) -> Ray {
        self.compute_pick_ray_from_window(window_pos)
    }

    // ========== Intersection tests ==========

    /// Tests if a bounding box intersects the frustum.
    ///
    /// Per C++ GfFrustum::Intersects: transforms frustum planes into bbox local
    /// space and tests against the axis-aligned range. Returns false for empty bboxes.
    #[must_use]
    pub fn intersects_bbox(&self, bbox: &BBox3d) -> bool {
        if bbox.range().is_empty() {
            return false;
        }
        let planes = self.get_planes();
        let local_range = bbox.range();
        let world_to_local = bbox.inverse_matrix();

        for plane in planes.iter() {
            let mut local_plane = *plane;
            local_plane.transform(world_to_local);
            if !local_plane.intersects_positive_half_space_box(local_range) {
                return false;
            }
        }
        true
    }

    /// Tests if a point is inside the frustum.
    #[must_use]
    pub fn intersects_point(&self, point: &Vec3d) -> bool {
        let planes = self.get_planes();

        for plane in planes.iter() {
            if plane.distance(point) < 0.0 {
                return false;
            }
        }
        true
    }

    /// Bitmask per C++ _CalcIntersectionBitMask: 1 bit per plane, 1 = inside.
    fn calc_intersection_bitmask(planes: &[Plane; 6], p: &Vec3d) -> u32 {
        (planes[0].intersects_positive_half_space_point(p) as u32) << 0
            | (planes[1].intersects_positive_half_space_point(p) as u32) << 1
            | (planes[2].intersects_positive_half_space_point(p) as u32) << 2
            | (planes[3].intersects_positive_half_space_point(p) as u32) << 3
            | (planes[4].intersects_positive_half_space_point(p) as u32) << 4
            | (planes[5].intersects_positive_half_space_point(p) as u32) << 5
    }

    /// Per C++ _SegmentIntersects: parametric clipping against straddling planes.
    fn segment_intersects_inner(
        planes: &[Plane; 6],
        p0: &Vec3d,
        p0_mask: u32,
        p1: &Vec3d,
        p1_mask: u32,
    ) -> bool {
        if (p0_mask | p1_mask) != 0x3f {
            return false;
        }
        if p0_mask == 0x3f || p1_mask == 0x3f {
            return true;
        }
        let mut t0 = 0.0;
        let mut t1 = 1.0;
        let v = *p1 - *p0;
        for (i, plane) in planes.iter().enumerate() {
            let plane_bit = 1u32 << i;
            let p0_bit = p0_mask & plane_bit;
            let p1_bit = p1_mask & plane_bit;
            if p0_bit == p1_bit {
                continue;
            }
            let denom = plane.normal().dot(&v);
            if denom.abs() < 1e-15 {
                continue;
            }
            let t = -plane.distance(p0) / denom;
            if p0_bit != 0 {
                if t < t1 {
                    t1 = t;
                }
            } else if t > t0 {
                t0 = t;
            }
            if t0 > t1 {
                return false;
            }
        }
        true
    }

    /// Tests if a line segment intersects the frustum.
    /// Per C++ GfFrustum::Intersects(p0,p1): bitmask clipping.
    #[must_use]
    pub fn intersects_segment(&self, p0: &Vec3d, p1: &Vec3d) -> bool {
        let planes = self.get_planes();
        let p0_mask = Self::calc_intersection_bitmask(&planes, p0);
        let p1_mask = Self::calc_intersection_bitmask(&planes, p1);
        Self::segment_intersects_inner(&planes, p0, p0_mask, p1, p1_mask)
    }

    /// Tests if a triangle intersects the frustum.
    /// Per C++ GfFrustum::Intersects(p0,p1,p2): bitmask, segment clip, ray-triangle for enclosure.
    #[must_use]
    pub fn intersects_triangle(&self, p0: &Vec3d, p1: &Vec3d, p2: &Vec3d) -> bool {
        let planes = self.get_planes();
        let p0_mask = Self::calc_intersection_bitmask(&planes, p0);
        let p1_mask = Self::calc_intersection_bitmask(&planes, p1);
        let p2_mask = Self::calc_intersection_bitmask(&planes, p2);

        if (p0_mask | p1_mask | p2_mask) != 0x3f {
            return false;
        }
        if p0_mask == 0x3f || p1_mask == 0x3f || p2_mask == 0x3f {
            return true;
        }

        if Self::segment_intersects_inner(&planes, p0, p0_mask, p1, p1_mask)
            || Self::segment_intersects_inner(&planes, p1, p1_mask, p2, p2_mask)
            || Self::segment_intersects_inner(&planes, p2, p2_mask, p0, p0_mask)
        {
            return true;
        }

        let near_bit = 1 << 4;
        let far_bit = 1 << 5;
        let num_corners_to_check = if (p0_mask & near_bit) != 0
            && (p1_mask & near_bit) != 0
            && (p2_mask & near_bit) != 0
            && (p0_mask & far_bit) != 0
            && (p1_mask & far_bit) != 0
            && (p2_mask & far_bit) != 0
        {
            1
        } else {
            4
        };

        let pick_points = [
            Vec2d::new(-1.0, -1.0),
            Vec2d::new(-1.0, 1.0),
            Vec2d::new(1.0, 1.0),
            Vec2d::new(1.0, -1.0),
        ];
        for i in 0..num_corners_to_check {
            let pick_ray = self.compute_pick_ray(pick_points[i]);
            if pick_ray
                .intersect_triangle(p0, p1, p2, f64::INFINITY)
                .is_some()
            {
                return true;
            }
        }
        false
    }

    /// Fits the frustum to a sphere.
    ///
    /// Per C++ GfFrustum::FitToSphere: orthographic sets window to enclose sphere;
    /// perspective uses similar-triangles formula for view distance.
    pub fn fit_to_sphere(&mut self, center: Vec3d, radius: f64, slack: f64) {
        if self.projection_type == ProjectionType::Orthographic {
            self.view_distance = radius + slack;
            self.window = Range2d::new(Vec2d::new(-radius, -radius), Vec2d::new(radius, radius));
        } else {
            let aspect = self.compute_aspect_ratio();
            let which_dim = if aspect > 1.0 { 1 } else { 0 };
            let (min, max) = (self.window.min(), self.window.max());
            let min_val = [min.x, min.y][which_dim];
            let max_val = [max.x, max.y][which_dim];
            let half_size = if min_val > 0.0 {
                max_val
            } else if max_val < 0.0 {
                min_val
            } else if -min_val > max_val {
                min_val
            } else {
                max_val
            };
            let half_size = half_size.abs().max(1.0);
            let near_min = self.near_far.min();
            self.view_distance =
                radius * (1.0 / half_size) * (half_size * half_size + near_min * near_min).sqrt();
        }
        let near_min = self.view_distance - (radius + slack);
        self.near_far = Range1d::new(near_min, near_min + 2.0 * (radius + slack));
        self.position = center - self.compute_view_direction() * self.view_distance;
        self.dirty_planes();
    }

    /// Transforms the frustum by a matrix.
    ///
    /// Per C++ GfFrustum::Transform: rescales near/far/view_distance by view-dir
    /// length, transforms reference-plane corners, fixes negative-scale window flips.
    pub fn transform(&mut self, matrix: &Matrix4d) -> &mut Self {
        let mut frustum = Frustum::new();
        frustum.projection_type = self.projection_type;

        frustum.position = matrix.transform_point(&self.position);

        let view_dir = self.compute_view_direction();
        let up_vec = self.compute_up_vector();

        let mut view_dir_prime = matrix.transform_dir(&view_dir);
        let mut up_vec_prime = matrix.transform_dir(&up_vec);
        let scale = view_dir_prime.length();
        view_dir_prime = view_dir_prime.normalized();
        up_vec_prime = up_vec_prime.normalized();

        let view_right_prime = view_dir_prime.cross(&up_vec_prime).normalized();

        let rot_matrix = Matrix4d::new(
            view_right_prime.x,
            view_right_prime.y,
            view_right_prime.z,
            0.0,
            up_vec_prime.x,
            up_vec_prime.y,
            up_vec_prime.z,
            0.0,
            -view_dir_prime.x,
            -view_dir_prime.y,
            -view_dir_prime.z,
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
        );
        frustum.rotation = rot_matrix.extract_rotation();

        frustum.near_far = Range1d::new(self.near_far.min() * scale, self.near_far.max() * scale);
        frustum.view_distance = self.view_distance * scale;

        let min = self.window.min();
        let max = self.window.max();
        let mut left_bottom =
            self.position + self.rotation.transform_dir(&Vec3d::new(min.x, min.y, -1.0));
        let mut right_top =
            self.position + self.rotation.transform_dir(&Vec3d::new(max.x, max.y, -1.0));

        left_bottom = matrix.transform_point(&left_bottom);
        right_top = matrix.transform_point(&right_top);
        left_bottom = left_bottom - frustum.position;
        right_top = right_top - frustum.position;
        left_bottom = frustum.rotation.inverse().transform_dir(&left_bottom);
        right_top = frustum.rotation.inverse().transform_dir(&right_top);

        if self.projection_type == ProjectionType::Perspective {
            left_bottom = left_bottom / scale;
            right_top = right_top / scale;
        }

        let mut w_min = Vec2d::new(left_bottom.x, left_bottom.y);
        let mut w_max = Vec2d::new(right_top.x, right_top.y);
        if w_min.x > w_max.x {
            std::mem::swap(&mut w_min.x, &mut w_max.x);
        }
        if w_min.y > w_max.y {
            std::mem::swap(&mut w_min.y, &mut w_max.y);
        }
        frustum.window = Range2d::new(w_min, w_max);

        *self = frustum;
        self.dirty_planes();
        self
    }

    /// Computes a narrowed frustum for picking. Per C++ ComputeNarrowedFrustum(windowPos, size).
    #[must_use]
    pub fn compute_narrowed_frustum(&self, window_pos: Vec2d, size: Vec2d) -> Frustum {
        let window_point = window_normalized_to_point(window_pos, &self.window);
        self.compute_narrowed_frustum_sub(window_point, size)
    }

    /// Computes a narrowed frustum for picking at a world position.
    /// Returns clone of self if world point is behind/at eye. Per C++ ComputeNarrowedFrustum(worldPoint, size).
    #[must_use]
    pub fn compute_narrowed_frustum_at_world(&self, world_point: Vec3d, size: Vec2d) -> Frustum {
        let view = self.compute_view_matrix();
        let cam_space = view.transform_point(&world_point);
        if cam_space.z >= 0.0 {
            return self.clone();
        }
        let mut window_point = Vec2d::new(cam_space.x, cam_space.y);
        if self.projection_type == ProjectionType::Perspective {
            window_point = window_point / (-cam_space.z);
        }
        self.compute_narrowed_frustum_sub(window_point, size)
    }

    /// Per C++ _ComputeNarrowedFrustumSub: clamp new window to existing.
    fn compute_narrowed_frustum_sub(&self, window_point: Vec2d, size: Vec2d) -> Frustum {
        let mut narrowed = self.clone();
        let half_size = Vec2d::new(
            0.5 * size.x * self.window.size().x,
            0.5 * size.y * self.window.size().y,
        );
        let mut min_pt = window_point - half_size;
        let mut max_pt = window_point + half_size;
        let w_min = self.window.min();
        let w_max = self.window.max();
        if min_pt.x < w_min.x {
            min_pt.x = w_min.x;
        }
        if min_pt.y < w_min.y {
            min_pt.y = w_min.y;
        }
        if max_pt.x > w_max.x {
            max_pt.x = w_max.x;
        }
        if max_pt.y > w_max.y {
            max_pt.y = w_max.y;
        }
        narrowed.set_window(Range2d::new(min_pt, max_pt));
        narrowed
    }

    /// Tests if a bbox intersects the view volume. Per C++ IntersectsViewVolume.
    /// Multiplies bbox matrix into view-projection, uses signed clipPos[3].
    #[must_use]
    pub fn intersects_view_volume(bbox: &BBox3d, view_proj_matrix: &Matrix4d) -> bool {
        let range = bbox.range();
        let corners = [
            Vec3d::new(range.min().x, range.min().y, range.min().z),
            Vec3d::new(range.min().x, range.min().y, range.max().z),
            Vec3d::new(range.min().x, range.max().y, range.min().z),
            Vec3d::new(range.min().x, range.max().y, range.max().z),
            Vec3d::new(range.max().x, range.min().y, range.min().z),
            Vec3d::new(range.max().x, range.min().y, range.max().z),
            Vec3d::new(range.max().x, range.max().y, range.min().z),
            Vec3d::new(range.max().x, range.max().y, range.max().z),
        ];
        let bbox_matrix = *bbox.matrix() * *view_proj_matrix;
        let mut clip_flags = 0u32;
        for corner in &corners {
            let clip_x = corner.x * bbox_matrix[0][0]
                + corner.y * bbox_matrix[1][0]
                + corner.z * bbox_matrix[2][0]
                + bbox_matrix[3][0];
            let clip_y = corner.x * bbox_matrix[0][1]
                + corner.y * bbox_matrix[1][1]
                + corner.z * bbox_matrix[2][1]
                + bbox_matrix[3][1];
            let clip_z = corner.x * bbox_matrix[0][2]
                + corner.y * bbox_matrix[1][2]
                + corner.z * bbox_matrix[2][2]
                + bbox_matrix[3][2];
            let clip_w = corner.x * bbox_matrix[0][3]
                + corner.y * bbox_matrix[1][3]
                + corner.z * bbox_matrix[2][3]
                + bbox_matrix[3][3];
            let flag = ((clip_x < clip_w) as u32)
                | ((clip_x > -clip_w) as u32) << 1
                | ((clip_y < clip_w) as u32) << 2
                | ((clip_y > -clip_w) as u32) << 3
                | ((clip_z < clip_w) as u32) << 4
                | ((clip_z > -clip_w) as u32) << 5;
            clip_flags |= flag;
        }
        clip_flags == 0x3f
    }

    // ========== Private helpers ==========

    /// Marks cached planes as dirty. Per C++ _DirtyFrustumPlanes.
    fn dirty_planes(&mut self) {
        if let Ok(mut g) = self.planes.write() {
            *g = None;
        }
    }

    /// Gets the frustum planes, computing and caching them if necessary.
    /// Per C++ _CalculateFrustumPlanes + atomic cache.
    fn get_planes(&self) -> [Plane; 6] {
        {
            let cache = self.planes.read().expect("lock");
            if let Some(p) = *cache {
                return p;
            }
        }
        let p = self.calculate_planes();
        *self.planes.write().expect("lock") = Some(p);
        p
    }

    /// Calculates the 6 frustum planes.
    ///
    /// Order: left, right, bottom, top, near, far.
    /// All planes have normals pointing INWARD (toward frustum center).
    fn calculate_planes(&self) -> [Plane; 6] {
        let corners = self.compute_corners();

        // Compute frustum center for orienting planes
        let center = (corners[0]
            + corners[1]
            + corners[2]
            + corners[3]
            + corners[4]
            + corners[5]
            + corners[6]
            + corners[7])
            * 0.125;

        // Create planes and orient them to point inward
        let mut near = Plane::from_three_points(corners[0], corners[1], corners[3]);
        near.reorient(&center);

        let mut far = Plane::from_three_points(corners[4], corners[5], corners[7]);
        far.reorient(&center);

        let mut left = Plane::from_three_points(corners[0], corners[2], corners[6]);
        left.reorient(&center);

        let mut right = Plane::from_three_points(corners[1], corners[5], corners[3]);
        right.reorient(&center);

        let mut bottom = Plane::from_three_points(corners[0], corners[4], corners[1]);
        bottom.reorient(&center);

        let mut top = Plane::from_three_points(corners[2], corners[3], corners[6]);
        top.reorient(&center);

        [left, right, bottom, top, near, far]
    }
}

impl Clone for Frustum {
    fn clone(&self) -> Self {
        Self {
            position: self.position,
            rotation: self.rotation,
            window: self.window,
            near_far: self.near_far,
            view_distance: self.view_distance,
            projection_type: self.projection_type,
            planes: RwLock::new(None), // Cache not copied, recomputed on first use
        }
    }
}

impl PartialEq for Frustum {
    fn eq(&self, other: &Self) -> bool {
        self.position == other.position
            && self.rotation == other.rotation
            && self.window == other.window
            && self.near_far == other.near_far
            && self.view_distance == other.view_distance
            && self.projection_type == other.projection_type
    }
}

impl Eq for Frustum {}

impl Hash for Frustum {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Per C++ TfHash::Combine all fields (f64 via to_bits)
        self.position.x.to_bits().hash(state);
        self.position.y.to_bits().hash(state);
        self.position.z.to_bits().hash(state);
        self.rotation.hash(state);
        self.window.min().x.to_bits().hash(state);
        self.window.min().y.to_bits().hash(state);
        self.window.max().x.to_bits().hash(state);
        self.window.max().y.to_bits().hash(state);
        self.near_far.min().to_bits().hash(state);
        self.near_far.max().to_bits().hash(state);
        self.view_distance.to_bits().hash(state);
        self.projection_type.hash(state);
    }
}

impl fmt::Display for Frustum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let proj = match self.projection_type {
            ProjectionType::Perspective => "Perspective",
            ProjectionType::Orthographic => "Orthographic",
        };
        write!(
            f,
            "[{} {} {} {} {} {}]",
            self.position,
            self.rotation,
            self.window,
            self.near_far,
            ostream_helper_p_double(self.view_distance),
            proj
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vec3d;

    #[test]
    fn test_default() {
        let f = Frustum::new();
        assert_eq!(f.projection_type(), ProjectionType::Perspective);
        assert_eq!(f.position(), vec3d(0.0, 0.0, 0.0));
    }

    #[test]
    fn test_set_perspective() {
        let mut f = Frustum::new();
        f.set_perspective(90.0, true, 1.0, 0.1, 100.0);

        let (fov, aspect, near, far) = f.get_perspective(true).unwrap();
        assert!((fov - 90.0).abs() < 0.1);
        assert!((aspect - 1.0).abs() < 1e-10);
        assert!((near - 0.1).abs() < 1e-10);
        assert!((far - 100.0).abs() < 1e-10);
    }

    #[test]
    fn test_set_orthographic() {
        let mut f = Frustum::new();
        f.set_orthographic(-10.0, 10.0, -10.0, 10.0, 1.0, 100.0);

        assert_eq!(f.projection_type(), ProjectionType::Orthographic);
        let (l, r, _b, _t, _n, _fa) = f.get_orthographic().unwrap();
        assert!((l - (-10.0)).abs() < 1e-10);
        assert!((r - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_view_direction() {
        let f = Frustum::new();
        let dir = f.compute_view_direction();
        // Default looks along -Z
        assert!((dir.z - (-1.0)).abs() < 1e-10);
    }

    #[test]
    fn test_up_vector() {
        let f = Frustum::new();
        let up = f.compute_up_vector();
        // Default up is +Y
        assert!((up.y - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_look_at_point() {
        let mut f = Frustum::new();
        f.set_view_distance(10.0);
        let look_at = f.compute_look_at_point();
        // Position at origin, looking at -Z, distance 10
        assert!((look_at.z - (-10.0)).abs() < 1e-10);
    }

    #[test]
    fn test_compute_corners() {
        let mut f = Frustum::new();
        f.set_perspective(90.0, true, 1.0, 1.0, 10.0);
        let corners = f.compute_corners();
        assert_eq!(corners.len(), 8);
    }

    #[test]
    fn test_compute_ray() {
        let f = Frustum::new();
        let ray = f.compute_ray(Vec2d::new(0.0, 0.0));
        // Center ray should go along -Z
        assert!((ray.direction().z - (-1.0)).abs() < 0.1);
    }

    #[test]
    fn test_point_intersection() {
        let mut f = Frustum::new();
        f.set_perspective(90.0, true, 1.0, 1.0, 100.0);

        // Point in front, inside frustum
        assert!(f.intersects_point(&vec3d(0.0, 0.0, -5.0)));

        // Point behind camera
        assert!(!f.intersects_point(&vec3d(0.0, 0.0, 5.0)));
    }
}
