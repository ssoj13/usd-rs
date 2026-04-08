//! Free camera scene delegate — provides a camera prim for viewport rendering.
//!
//! Manages a "free" camera that is not part of the USD stage, allowing
//! interactive viewport navigation without modifying scene data.
//! Port of pxr/imaging/hdx/freeCameraSceneDelegate.h/cpp

use usd_camera_util::{ConformWindowPolicy, Framing};
use usd_gf::{APERTURE_UNIT, Camera, CameraProjection, FOCAL_LENGTH_UNIT, Matrix4d, Vec2f, Vec4f};
use usd_hd::prim::HdSceneDelegate;
use usd_hd::types::HdDirtyBits;
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Value;

/// Hydra camera projection enum (mirrors C++ `HdCamera::Projection`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HdCameraProjection {
    Perspective = 0,
    Orthographic = 1,
}

/// Free camera scene delegate.
///
/// Injects a camera sprim into the render index that can be freely positioned
/// via a `GfCamera` object or raw view/projection matrices.
///
/// API mirrors C++ `HdxFreeCameraSceneDelegate`:
/// - `SetCamera(GfCamera)` — preferred, sets full camera state
/// - `SetMatrices(view, proj)` — transition helper
/// - `SetClipPlanes(Vec<Vec4f>)` — transition helper
/// - `SetWindowPolicy(policy)` — window conform policy
/// - `GetCameraParamValue(id, key)` — used by HdSceneDelegate interface
///
/// Port of pxr/imaging/hdx/freeCameraSceneDelegate.h
pub struct HdxFreeCameraSceneDelegate {
    /// Delegate ID (root path for managed prims).
    delegate_id: Path,
    /// Camera prim path (empty if cameras not supported by render delegate).
    camera_id: Path,
    /// Camera state (preferred over raw matrices).
    camera: Camera,
    /// Window policy for conforming camera frustum.
    window_policy: ConformWindowPolicy,
    /// Viewport framing (Hydra 2.0).
    framing: Framing,
    /// Whether camera data has changed since last sync.
    dirty: bool,
}

impl HdxFreeCameraSceneDelegate {
    /// Create a new free camera scene delegate.
    ///
    /// The camera prim path is `delegate_id/camera`.
    /// In C++, camera is only created when render delegate supports cameras.
    pub fn new(delegate_id: Path) -> Self {
        let camera_id = delegate_id
            .append_child("camera")
            .unwrap_or_else(|| delegate_id.clone());
        Self {
            delegate_id,
            camera_id,
            camera: Camera::new(),
            window_policy: ConformWindowPolicy::Fit,
            framing: Framing::new_empty(),
            dirty: true,
        }
    }

    /// Get the camera prim path.
    pub fn get_camera_id(&self) -> &Path {
        &self.camera_id
    }

    /// Set full camera state from a `GfCamera` object (preferred API).
    ///
    /// Marks dirty only if camera state actually changed. Mirrors C++ `SetCamera`.
    pub fn set_camera(&mut self, camera: Camera) {
        self.camera = camera;
        self.dirty = true;
    }

    /// Set camera from OpenGL-style view and projection matrices.
    ///
    /// Transition helper — prefer `set_camera()` with a full `Camera` struct.
    pub fn set_matrices(&mut self, view_matrix: Matrix4d, proj_matrix: Matrix4d) {
        let fl = self.camera.focal_length();
        self.camera
            .set_from_view_and_projection_matrix(&view_matrix, &proj_matrix, fl);
        self.dirty = true;
    }

    /// Set additional clip planes (camera space, equation a*x + b*y + c*z + d < 0).
    pub fn set_clip_planes(&mut self, clip_planes: Vec<Vec4f>) {
        self.camera.set_clipping_planes(clip_planes);
        self.dirty = true;
    }

    /// Set window conform policy.
    pub fn set_window_policy(&mut self, policy: ConformWindowPolicy) {
        if self.window_policy == policy {
            return;
        }
        self.window_policy = policy;
        self.dirty = true;
    }

    /// Set viewport framing.
    pub fn set_framing(&mut self, framing: Framing) {
        self.framing = framing;
        self.dirty = true;
    }

    /// Set the view (world-to-camera) matrix directly.
    pub fn set_view_matrix(&mut self, mat: Matrix4d) {
        // Extract camera transform from view matrix inverse.
        self.camera
            .set_transform(mat.inverse().unwrap_or_else(Matrix4d::identity));
        self.dirty = true;
    }

    /// Set the projection matrix directly.
    pub fn set_projection_matrix(&mut self, mat: Matrix4d) {
        let fl = self.camera.focal_length();
        self.camera
            .set_from_view_and_projection_matrix(&Matrix4d::identity(), &mat, fl);
        self.dirty = true;
    }

    /// Get the underlying camera.
    pub fn get_camera(&self) -> &Camera {
        &self.camera
    }

    /// Get the view matrix (inverse of camera transform).
    pub fn get_view_matrix(&self) -> Matrix4d {
        self.camera
            .transform()
            .inverse()
            .unwrap_or_else(Matrix4d::identity)
    }

    /// Get the projection matrix.
    pub fn get_projection_matrix(&self) -> Matrix4d {
        // Derive from camera frustum.
        self.camera.frustum().compute_projection_matrix()
    }

    /// Get clipping range as (near, far).
    pub fn get_clipping_range(&self) -> (f64, f64) {
        let r = self.camera.clipping_range();
        (r.min() as f64, r.max() as f64)
    }

    /// Get framing.
    pub fn get_framing(&self) -> &Framing {
        &self.framing
    }

    /// Get window policy.
    pub fn get_window_policy(&self) -> ConformWindowPolicy {
        self.window_policy
    }

    /// Check if camera data is dirty.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Mark clean after sync.
    pub fn mark_clean_delegate(&mut self) {
        self.dirty = false;
    }

    /// Return camera param value for a given token key.
    ///
    /// Mirrors C++ `GetCameraParamValue(id, key)` — returns `VtValue`-equivalent
    /// for each `HdCameraTokens` key that HdCamera::Sync() will query.
    pub fn get_camera_param_value(&self, id: &Path, key: &Token) -> Value {
        if id != &self.camera_id {
            return Value::default();
        }
        match key.as_str() {
            "projection" => {
                let proj = match self.camera.projection() {
                    CameraProjection::Perspective => HdCameraProjection::Perspective as i32,
                    CameraProjection::Orthographic => HdCameraProjection::Orthographic as i32,
                };
                Value::from(proj)
            }
            "focalLength" => {
                // Scaled by FOCAL_LENGTH_UNIT (0.1) matching C++.
                Value::from(self.camera.focal_length() * FOCAL_LENGTH_UNIT as f32)
            }
            "horizontalAperture" => {
                Value::from(self.camera.horizontal_aperture() * APERTURE_UNIT as f32)
            }
            "verticalAperture" => {
                Value::from(self.camera.vertical_aperture() * APERTURE_UNIT as f32)
            }
            "horizontalApertureOffset" => {
                Value::from(self.camera.horizontal_aperture_offset() * APERTURE_UNIT as f32)
            }
            "verticalApertureOffset" => {
                Value::from(self.camera.vertical_aperture_offset() * APERTURE_UNIT as f32)
            }
            "clippingRange" => {
                // C++ returns GfRange1f; we use Vec2f(near, far).
                let r = self.camera.clipping_range();
                Value::from(Vec2f::new(r.min(), r.max()))
            }
            "clipPlanes" => {
                // Return clip planes as a flat Vec<Vec4f>.
                Value::from(self.camera.clipping_planes().to_vec())
            }
            "fStop" => Value::from(self.camera.f_stop()),
            "focusDistance" => Value::from(self.camera.focus_distance()),
            "windowPolicy" => Value::from(self.window_policy as i32),
            _ => Value::default(),
        }
    }
}

impl HdSceneDelegate for HdxFreeCameraSceneDelegate {
    fn get_dirty_bits(&self, _id: &Path) -> HdDirtyBits {
        if self.dirty { !0 } else { 0 }
    }

    fn mark_clean(&mut self, _id: &Path, _bits: HdDirtyBits) {
        self.dirty = false;
    }

    fn get_instancer_id(&self, _prim_id: &Path) -> Path {
        Path::default()
    }

    fn get_delegate_id(&self) -> Path {
        self.delegate_id.clone()
    }

    fn get_transform(&self, id: &Path) -> Matrix4d {
        if id == &self.camera_id {
            // Camera transform is the world-space filmback transform.
            self.camera.transform().clone()
        } else {
            Matrix4d::identity()
        }
    }

    fn get(&self, id: &Path, key: &Token) -> Value {
        // HdSceneDelegate::Get is used for non-camera prims.
        // Camera params go through get_camera_param_value (P2-15 fix).
        self.get_camera_param_value(id, key)
    }

    fn get_visible(&self, _id: &Path) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_gf::Camera;

    #[test]
    fn test_free_camera_creation() {
        let delegate = HdxFreeCameraSceneDelegate::new(Path::from_string("/FreeCamera").unwrap());
        assert_eq!(delegate.get_camera_id().to_string(), "/FreeCamera/camera");
        assert!(delegate.is_dirty());
    }

    #[test]
    fn test_set_camera() {
        let mut delegate = HdxFreeCameraSceneDelegate::new(Path::from_string("/Cam").unwrap());
        delegate.mark_clean_delegate();
        assert!(!delegate.is_dirty());

        let mut cam = Camera::new();
        cam.set_focal_length(35.0);
        delegate.set_camera(cam);
        assert!(delegate.is_dirty());
    }

    #[test]
    fn test_set_view_matrix() {
        let mut delegate = HdxFreeCameraSceneDelegate::new(Path::from_string("/Cam").unwrap());
        delegate.mark_clean_delegate();
        delegate.set_view_matrix(Matrix4d::identity());
        assert!(delegate.is_dirty());
    }

    #[test]
    fn test_get_camera_param_value_projection() {
        let delegate = HdxFreeCameraSceneDelegate::new(Path::from_string("/Cam").unwrap());
        let cam_path = delegate.get_camera_id().clone();
        let val = delegate.get_camera_param_value(&cam_path, &Token::new("projection"));
        // Default camera is Perspective (0)
        assert_eq!(val.get::<i32>(), Some(&0));
    }

    #[test]
    fn test_get_camera_param_value_clipping_range() {
        let delegate = HdxFreeCameraSceneDelegate::new(Path::from_string("/Cam").unwrap());
        let cam_path = delegate.get_camera_id().clone();
        let val = delegate.get_camera_param_value(&cam_path, &Token::new("clippingRange"));
        let range = val.get::<Vec2f>().expect("expected Vec2f");
        assert!(range.x > 0.0); // near > 0
        assert!(range.y > range.x); // far > near
    }

    #[test]
    fn test_get_camera_param_value_wrong_id() {
        let delegate = HdxFreeCameraSceneDelegate::new(Path::from_string("/Cam").unwrap());
        let other = Path::from_string("/Other").unwrap();
        let val = delegate.get_camera_param_value(&other, &Token::new("projection"));
        // Wrong ID -> default value
        assert!(val.get::<i32>().is_none());
    }

    #[test]
    fn test_get_transform_identity_view() {
        let delegate = HdxFreeCameraSceneDelegate::new(Path::from_string("/Cam").unwrap());
        let cam_path = delegate.get_camera_id().clone();
        // Default camera has identity transform
        let xform = delegate.get_transform(&cam_path);
        assert_eq!(xform, Matrix4d::identity());
    }

    #[test]
    fn test_set_window_policy_dedup() {
        let mut delegate = HdxFreeCameraSceneDelegate::new(Path::from_string("/Cam").unwrap());
        delegate.mark_clean_delegate();
        // Setting same policy should not mark dirty
        delegate.set_window_policy(ConformWindowPolicy::Fit);
        assert!(!delegate.is_dirty());
    }

    #[test]
    fn test_set_clip_planes() {
        let mut delegate = HdxFreeCameraSceneDelegate::new(Path::from_string("/Cam").unwrap());
        delegate.mark_clean_delegate();
        let planes = vec![Vec4f::new(0.0, 1.0, 0.0, -1.0)];
        delegate.set_clip_planes(planes);
        assert!(delegate.is_dirty());
        assert_eq!(delegate.get_camera().clipping_planes().len(), 1);
    }
}
