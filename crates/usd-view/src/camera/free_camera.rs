//! Free camera — orbit (tumble), pan (truck), zoom (adjust distance).
//!
//! Reference: _ref/OpenUSD/pxr/usdImaging/usdviewq/freeCamera.py
//! Y-up convention: camera looks toward center, up is +Y.
//!
//! # Convention
//!
//! Row-vector convention (Imath/OpenUSD standard): `v' = v * M`.
//! - `view_matrix()` returns world-to-camera (V), translation in row 3.
//! - `projection_matrix()` returns camera-to-clip (P).
//! - Compose as `VP = V * P`, project point as `clip = point * VP`.
//! - When extracting clip coords, use `VP.column(i)` NOT `VP.row(i)`.

use usd_gf::frustum::Frustum;
use usd_gf::matrix4::Matrix4d;
use usd_gf::vec3::Vec3d;
use usd_gf::{degrees_to_radians, vec3d};

/// Minimum near clipping plane (absolute floor).
const MIN_NEAR: f64 = 0.001;
/// Default near/far ratio for adaptive clipping.
const NEAR_FAR_RATIO: f64 = 0.0005;
/// Default near plane used for frame_selection minimum distance check.
///
/// C++ freeCamera.py uses the same hardcoded `defaultNear = 1`. For scenes
/// smaller than 1 m (e.g. Blender cm-unit exports with `xformOp:scale = 0.01`)
/// this constant is too large — `frame_selection` and `compute_auto_clip` scale
/// it down proportionally via `sel_size * 0.1` / `bbox_diag * 0.1` to avoid
/// pushing the camera to 1 m away from a 7 cm object.
const DEFAULT_NEAR: f64 = 1.0;

/// Free camera with orbit (tumble), pan (truck), and zoom.
///
/// Matches C++ usdviewq/freeCamera.py — handles both Y-up and Z-up stages.
/// For Z-up stages, `_YZUpInvMatrix` (Rx(+90)) is inserted into the camera
/// transform chain between rotation and center-translation, exactly as in C++:
///   `T(Z*dist) * Rz(-psi) * Rx(-phi) * Ry(-theta) * YZUpInv * T(center)`
#[derive(Debug, Clone)]
pub struct FreeCamera {
    /// Rotation theta (degrees) — horizontal orbit around Y.
    rot_theta: f64,
    /// Rotation phi (degrees) — vertical orbit (latitude). Clamped to avoid flip.
    rot_phi: f64,
    /// Rotation psi (degrees) — roll (typically 0). Reserved for future use.
    _rot_psi: f64,
    /// Look-at center point in world space.
    center: Vec3d,
    /// Distance from center to camera.
    dist: f64,
    /// Vertical field of view (degrees) for perspective.
    fov: f64,
    /// Selection/framing size — used for zoom scaling.
    sel_size: f64,
    /// True when the stage uses Z-up axis (upAxis = "Z").
    is_z_up: bool,
    /// Override near clipping plane (set by set_clip_planes, None = adaptive).
    override_near: Option<f64>,
    /// Override far clipping plane (set by set_clip_planes, None = adaptive).
    override_far: Option<f64>,
    /// Closest visible distance projected along the view ray.
    /// Used by auto-clip to prevent near from clipping visible geometry.
    /// Reference: freeCamera.py _closestVisibleDist
    cvd: Option<f64>,
    /// Distance at last frame_selection (detect zoom-in since last reframe).
    last_framed_dist: f64,
    /// Closest visible dist at last frame_selection.
    last_framed_cvd: f64,
    /// Orthographic projection mode (false = perspective).
    orthographic: bool,
}

impl Default for FreeCamera {
    fn default() -> Self {
        Self {
            rot_theta: 0.0,
            rot_phi: 0.0,
            _rot_psi: 0.0,
            center: Vec3d::new(0.0, 0.0, 0.0),
            dist: 100.0,
            fov: 60.0,
            sel_size: 10.0,
            is_z_up: false,
            override_near: None,
            override_far: None,
            cvd: None,
            last_framed_dist: 100.0,
            last_framed_cvd: 0.0,
            orthographic: false,
        }
    }
}

impl FreeCamera {
    /// Creates a new free camera at default position.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new free camera with Z-up flag set (matches C++ FreeCamera(isZUp)).
    pub fn new_with_z_up(is_z_up: bool) -> Self {
        Self {
            is_z_up,
            ..Self::default()
        }
    }

    /// Orbit (tumble) by delta theta and phi in degrees.
    pub fn tumble(&mut self, d_theta: f64, d_phi: f64) {
        self.rot_theta += d_theta;
        self.rot_phi += d_phi;
        // Clamp phi to avoid flipping over the top. C++ reference uses 89.5.
        let max_phi = 89.5;
        self.rot_phi = self.rot_phi.clamp(-max_phi, max_phi);
    }

    /// PanTilt: rotate camera around its own position (heading/pitch) in degrees.
    ///
    /// Reference freeCamera.py PanTilt(): applies Rx(dTilt)*Ry(dPan) to current camera
    /// transform, then decomposes back to theta/phi. The center point moves, camera stays.
    pub fn pan_tilt(&mut self, d_pan: f64, d_tilt: f64) {
        let pan_rad = degrees_to_radians(d_pan);
        let tilt_rad = degrees_to_radians(d_tilt);

        // Ry(dPan)
        let (sp, cp) = (pan_rad.sin(), pan_rad.cos());
        let ry_pan = Matrix4d::from_array([
            [cp, 0.0, -sp, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [sp, 0.0, cp, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]);

        // Rx(dTilt)
        let (st, ct) = (tilt_rad.sin(), tilt_rad.cos());
        let rx_tilt = Matrix4d::from_array([
            [1.0, 0.0, 0.0, 0.0],
            [0.0, ct, st, 0.0],
            [0.0, -st, ct, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]);

        // Incremental rotation applied in camera space: Rx(tilt) * Ry(pan) * current_transform
        let xf = rx_tilt * ry_pan * self.camera_transform();

        // Decompose new orientation back to spherical angles from camera forward (row 2 = local +Z)
        let fwd_x = xf[2][0];
        let fwd_y = xf[2][1];
        let fwd_z = xf[2][2];
        self.rot_theta = f64::atan2(-fwd_x, -fwd_z).to_degrees();
        self.rot_phi = f64::asin(fwd_y.clamp(-1.0, 1.0)).to_degrees();
        self._rot_psi = 0.0; // zero roll to avoid drift

        // Recompute center so camera position stays fixed: cam + forward * dist
        let cam_pos = vec3d(xf[3][0], xf[3][1], xf[3][2]);
        let forward = vec3d(fwd_x, fwd_y, fwd_z).normalized();
        self.center = cam_pos + forward * (-self.dist);
    }

    /// Walk: move camera on its horizontal plane (forward/backward and strafe).
    ///
    /// Reference freeCamera.py Walk(): translates center by dForward*cam_forward + dRight*cam_right.
    pub fn walk(&mut self, d_forward: f64, d_right: f64) {
        let xf = self.camera_transform();
        let right = vec3d(xf[0][0], xf[0][1], xf[0][2]).normalized();
        let up = vec3d(xf[1][0], xf[1][1], xf[1][2]).normalized();
        // World-horizontal forward: cross(up, right) gives camera-plane forward
        let forward = up.cross(&right).normalized();
        self.center = self.center + forward * d_forward + right * d_right;
    }

    /// Pan (truck) by delta in view-plane coordinates.
    pub fn truck(&mut self, delta_right: f64, delta_up: f64) {
        let (right, up) = self.view_right_and_up();
        self.center = self.center + right * delta_right + up * delta_up;
    }

    /// Zoom by scale factor (>1 = zoom out, <1 = zoom in).
    pub fn adjust_distance(&mut self, scale_factor: f64) {
        // Pure exponential zoom: dist *= factor.
        // Visually uniform — each scroll tick moves the same perceptual distance
        // regardless of how far/close the camera is.
        self.dist = (self.dist * scale_factor).max(0.0001);
    }

    /// Frame the given bounding box with optional fit margin.
    ///
    /// Sets `center`, `sel_size`, and `dist` so the bbox fills the viewport.
    /// For small scenes (sel_size < 1 m), the near-plane floor is scaled down
    /// proportionally so the camera can get close enough. Without this, Blender
    /// exports with redundant 0.01 scale would appear as a tiny speck because
    /// `dist` was clamped to `DEFAULT_NEAR(1.0) + length_to_fit`.
    pub fn frame_selection(&mut self, bbox_min: Vec3d, bbox_max: Vec3d, frame_fit: f64) {
        // Reset closest visible dist on reframe (matches freeCamera.py frameSelection).
        self.cvd = None;
        // Ignore invalid bounds to avoid sending the camera to NaN/Inf space.
        if !bbox_min.x.is_finite()
            || !bbox_min.y.is_finite()
            || !bbox_min.z.is_finite()
            || !bbox_max.x.is_finite()
            || !bbox_max.y.is_finite()
            || !bbox_max.z.is_finite()
        {
            return;
        }

        let center_x = (bbox_min.x + bbox_max.x) * 0.5;
        let center_y = (bbox_min.y + bbox_max.y) * 0.5;
        let center_z = (bbox_min.z + bbox_max.z) * 0.5;
        if !center_x.is_finite() || !center_y.is_finite() || !center_z.is_finite() {
            return;
        }
        self.center = Vec3d::new(center_x, center_y, center_z);

        let size_x = (bbox_max.x - bbox_min.x).abs().max(0.01);
        let size_y = (bbox_max.y - bbox_min.y).abs().max(0.01);
        let size_z = (bbox_max.z - bbox_min.z).abs().max(0.01);
        if !size_x.is_finite() || !size_y.is_finite() || !size_z.is_finite() {
            return;
        }
        self.sel_size = size_x.max(size_y).max(size_z);

        let half_fov_rad = degrees_to_radians(self.fov * 0.5);
        let half_fov_rad = half_fov_rad.max(0.01);
        let length_to_fit = self.sel_size * frame_fit * 0.5;
        // Reference freeCamera.py: dist = lengthToFit / atan(radians(halfFov))
        self.dist = length_to_fit / half_fov_rad.atan();
        // Scale the near-plane floor by scene size so small scenes (e.g. Blender
        // exports with 0.01 scale) don't get pushed to DEFAULT_NEAR=1.0 away.
        let effective_near = if self.sel_size < DEFAULT_NEAR {
            self.sel_size * 0.1
        } else {
            DEFAULT_NEAR
        };
        // Reference freeCamera.py:327-328: if dist < defaultNear + selSize*0.5: dist = defaultNear + lengthToFit
        if self.dist < effective_near + self.sel_size * 0.5 {
            self.dist = effective_near + length_to_fit;
        }
        if !self.dist.is_finite() {
            self.dist = 1.0;
        }
    }

    /// Set Z-up flag (call when stage changes).
    pub fn set_is_z_up(&mut self, z_up: bool) {
        self.is_z_up = z_up;
    }

    /// Returns true if camera is configured for Z-up stage.
    pub fn is_z_up(&self) -> bool {
        self.is_z_up
    }

    /// YZUpInvMatrix — Rx(+90°), converts Y-up camera space to Z-up world.
    /// Matches C++ `Gf.Rotation(Gf.Vec3d.XAxis(), -90).GetInverse()`.
    fn yz_up_inv_matrix(&self) -> Matrix4d {
        if self.is_z_up {
            // Rx(+90): Y→Z, Z→-Y  (inverse of Rx(-90))
            Matrix4d::from_array([
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, -1.0, 0.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ])
        } else {
            Matrix4d::identity()
        }
    }

    /// Compute camera transform (camera-to-world) per C++ freeCamera.py:
    ///   T(Z*dist) * Rz(-psi) * Rx(-phi) * Ry(-theta) * YZUpInv * T(center)
    fn camera_transform(&self) -> Matrix4d {
        let theta = degrees_to_radians(-self.rot_theta);
        let phi = degrees_to_radians(-self.rot_phi);

        // Ry(-theta)
        let (st, ct) = (theta.sin(), theta.cos());
        let ry = Matrix4d::from_array([
            [ct, 0.0, -st, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [st, 0.0, ct, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]);

        // Rx(-phi)
        let (sp, cp) = (phi.sin(), phi.cos());
        let rx = Matrix4d::from_array([
            [1.0, 0.0, 0.0, 0.0],
            [0.0, cp, sp, 0.0],
            [0.0, -sp, cp, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]);

        // T(Z*dist) — translate along camera +Z by dist
        let t_dist = Matrix4d::from_array([
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, self.dist, 1.0],
        ]);

        // T(center)
        let t_center = Matrix4d::from_array([
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [self.center.x, self.center.y, self.center.z, 1.0],
        ]);

        // C++ order: T(Z*dist) * Rx(-phi) * Ry(-theta) * YZUpInv * T(center)
        // (skipping Rz(-psi) since psi is always 0)
        t_dist * rx * ry * self.yz_up_inv_matrix() * t_center
    }

    /// Gets the right and up vectors in view space.
    fn view_right_and_up(&self) -> (Vec3d, Vec3d) {
        let xf = self.camera_transform();
        let right = vec3d(xf[0][0], xf[0][1], xf[0][2]).normalized();
        let up = vec3d(xf[1][0], xf[1][1], xf[1][2]).normalized();
        (right, up)
    }

    /// Camera position in world space.
    pub fn position(&self) -> Vec3d {
        let xf = self.camera_transform();
        vec3d(xf[3][0], xf[3][1], xf[3][2])
    }

    /// Camera forward direction (view direction, normalized) in world space.
    /// -Z of the camera transform (camera looks toward -Z in its local space).
    pub fn view_direction(&self) -> Vec3d {
        let xf = self.camera_transform();
        // Row 2 of camera-to-world is the local +Z axis; negate for forward (-Z)
        vec3d(-xf[2][0], -xf[2][1], -xf[2][2]).normalized()
    }

    /// Set closest visible distance from a world-space point.
    /// Projects the point onto the camera ray and stores the signed distance.
    /// Reference: freeCamera.py setClosestVisibleDistFromPoint()
    pub fn set_closest_visible_dist_from_point(&mut self, point: Vec3d) {
        let cam_pos = self.position();
        let view_dir = self.view_direction();
        let delta = point - cam_pos;
        let t = delta.x * view_dir.x + delta.y * view_dir.y + delta.z * view_dir.z;
        self.cvd = Some(t.max(0.0));
        self.last_framed_dist = self.dist;
        self.last_framed_cvd = t.max(0.0);
    }

    /// Returns closest visible distance if set.
    pub fn closest_visible_dist(&self) -> Option<f64> {
        self.cvd
    }

    /// Clears closest visible distance (reset after frame_selection).
    pub fn clear_closest_visible_dist(&mut self) {
        self.cvd = None;
    }

    /// Last framed closest dist (used by auto-clip zoom-in detection).
    pub fn last_framed_closest_dist(&self) -> f64 {
        self.last_framed_cvd
    }

    /// Computes the view matrix (world to camera) — inverse of camera transform.
    pub fn view_matrix(&self) -> Matrix4d {
        let xf = self.camera_transform();
        // Camera transform is camera-to-world; invert for view (world-to-camera)
        xf.inverse().unwrap_or_else(Matrix4d::identity)
    }

    /// Computes the projection matrix with adaptive near/far planes.
    pub fn projection_matrix(&self, aspect_ratio: f64) -> Matrix4d {
        // Use override planes if set (via set_clip_planes), else adaptive.
        // Adapt to BOTH scene size and current camera distance so near/far
        // stay reasonable when zooming in or out.
        let (near, far) = if let (Some(n), Some(f)) = (self.override_near, self.override_far) {
            (n, f)
        } else {
            let scene_size = self.sel_size.max(0.01);
            // Use the larger of scene_size and dist to cover both far and close views.
            let effective_range = scene_size.max(self.dist * 2.0);
            (
                (effective_range * NEAR_FAR_RATIO).max(MIN_NEAR),
                effective_range * 2000.0,
            )
        };
        let mut frustum = Frustum::new();
        if self.orthographic {
            // Ortho half-size derived from dist*tan(fov/2) so zoom via dist still works.
            let half_h = self.ortho_size().max(0.001);
            let half_w = half_h * aspect_ratio;
            frustum.set_orthographic(-half_w, half_w, -half_h, half_h, near, far);
        } else {
            frustum.set_perspective(
                self.fov,
                true, // vertical FOV
                aspect_ratio,
                near,
                far,
            );
        }
        frustum.compute_projection_matrix()
    }

    /// Clear the near/far override (revert to adaptive clipping).
    pub fn clear_clip_override(&mut self) {
        self.override_near = None;
        self.override_far = None;
    }

    /// Distance from center.
    pub fn dist(&self) -> f64 {
        self.dist
    }

    /// Center point.
    pub fn center(&self) -> Vec3d {
        self.center
    }

    /// FOV in degrees.
    pub fn fov(&self) -> f64 {
        self.fov
    }

    /// Set distance from center.
    pub fn set_dist(&mut self, d: f64) {
        self.dist = d;
    }

    /// Set center point.
    pub fn set_center(&mut self, c: Vec3d) {
        self.center = c;
    }

    /// Set vertical FOV in degrees.
    pub fn set_fov(&mut self, fov: f64) {
        self.fov = fov;
    }

    /// Enable or disable orthographic projection mode.
    pub fn set_orthographic(&mut self, ortho: bool) {
        self.orthographic = ortho;
    }

    /// Returns true when camera is in orthographic mode.
    pub fn is_orthographic(&self) -> bool {
        self.orthographic
    }

    /// Half-size of orthographic frustum in world units (vertical).
    /// Based on dist so scroll-wheel zoom works by changing dist.
    pub fn ortho_size(&self) -> f64 {
        let half_fov_rad = degrees_to_radians(self.fov * 0.5);
        self.dist * half_fov_rad.tan()
    }

    /// View forward direction in world space (camera -Z).
    pub fn view_forward(&self) -> Vec3d {
        let xf = self.camera_transform();
        vec3d(-xf[2][0], -xf[2][1], -xf[2][2]).normalized()
    }

    /// Returns the current effective near/far clip planes.
    pub fn clip_planes(&self) -> (f64, f64) {
        if let (Some(n), Some(f)) = (self.override_near, self.override_far) {
            (n, f)
        } else {
            let scene_size = self.sel_size.max(0.01);
            let effective_range = scene_size.max(self.dist * 2.0);
            (
                (effective_range * NEAR_FAR_RATIO).max(MIN_NEAR),
                effective_range * 2000.0,
            )
        }
    }

    /// Set near/far clipping planes directly (does not touch sel_size).
    ///
    /// When set, projection_matrix() uses these values instead of adaptive planes.
    /// Call clear_clip_override() to revert to adaptive clipping.
    pub fn set_clip_planes(&mut self, near: f64, far: f64) {
        self.override_near = Some(near.max(MIN_NEAR));
        self.override_far = Some(far.max(near * 2.0));
    }

    /// Convert FreeCamera to a GfCamera.
    ///
    /// Derives physical camera parameters from the free camera's FOV and transform.
    /// Uses the standard 35mm projector aperture (DEFAULT_HORIZONTAL_APERTURE) and
    /// computes focal_length to match the current vertical FOV.
    ///
    /// # TODO
    /// Orthographic mode is not yet supported by FreeCamera; always sets Perspective.
    pub fn to_gf_camera(&self) -> usd_gf::Camera {
        use usd_gf::{Camera, CameraProjection, FOVDirection};
        let mut cam = Camera::new();
        cam.set_projection(CameraProjection::Perspective);
        // Set perspective FOV using the vertical FOV of FreeCamera
        let aspect_ratio = 1.0; // aspect-neutral; caller conforms with ConformWindow
        cam.set_perspective_from_aspect_ratio_and_fov(
            aspect_ratio as f32,
            self.fov as f32,
            FOVDirection::Vertical,
        );
        // Set transform = camera-to-world
        cam.set_transform(self.camera_transform());
        // Apply clipping range
        let (near, far) = if let (Some(n), Some(f)) = (self.override_near, self.override_far) {
            (n as f32, f as f32)
        } else {
            let scene_size = self.sel_size.max(0.01);
            (
                (scene_size * NEAR_FAR_RATIO).max(MIN_NEAR) as f32,
                (scene_size * 2000.0) as f32,
            )
        };
        cam.set_clipping_range(usd_gf::Range1f::new(near, far));
        cam
    }

    /// Initialize FreeCamera from a GfCamera.
    ///
    /// Extracts position, orientation (theta/phi/psi), center, dist, and FOV
    /// from the camera transform. Matches C++ freeCamera.py `FromGfCamera` +
    /// `_pullFromCameraTransform`.
    pub fn from_gf_camera(cam: &usd_gf::Camera, is_z_up: bool) -> Self {
        use usd_gf::FOVDirection;
        let mut fc = Self::new_with_z_up(is_z_up);

        // Extract vertical FOV
        fc.fov = cam.field_of_view(FOVDirection::Vertical) as f64;

        // Extract clipping range
        let clip = cam.clipping_range();
        fc.override_near = Some((clip.min() as f64).max(MIN_NEAR));
        fc.override_far = Some((clip.max() as f64).max(fc.override_near.unwrap() * 2.0));

        // --- _pullFromCameraTransform ---
        let cam_transform = cam.transform().clone();
        let dist = cam.focus_distance() as f64;
        let frustum = cam.frustum();
        let cam_pos = frustum.position();
        let cam_axis = frustum.compute_view_direction();

        // Translational parts
        fc.dist = dist;
        fc.sel_size = dist / 10.0;
        fc.center = cam_pos + cam_axis * dist;

        // Rotational part: transform * YZUpMatrix, then decompose Y/X/Z
        let yz_up_matrix = if is_z_up {
            // Rx(-90): inverse of yz_up_inv_matrix
            Matrix4d::from_array([
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 0.0, -1.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ])
        } else {
            Matrix4d::identity()
        };
        let mut rot_matrix = cam_transform * yz_up_matrix;
        rot_matrix.orthonormalize();

        // Decompose into Y(theta), X(phi), Z(psi) — negate per C++
        let y_axis = vec3d(0.0, 1.0, 0.0);
        let x_axis = vec3d(1.0, 0.0, 0.0);
        let z_axis = vec3d(0.0, 0.0, 1.0);
        let angles = rot_matrix.decompose_rotation(&y_axis, &x_axis, &z_axis);
        fc.rot_theta = -angles.x;
        fc.rot_phi = -angles.y;
        fc._rot_psi = -angles.z;

        fc
    }
}
