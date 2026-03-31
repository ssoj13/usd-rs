
//! HdCamera - Camera state primitive.
//!
//! Full port of pxr/imaging/hd/camera.h + camera.cpp.
//! Represents a camera in Hydra with all USD camera schema parameters:
//! frustum, depth of field, shutter, exposure, lens distortion, window policy.
//!
//! # Camera Types
//!
//! - **Perspective**: Standard perspective projection
//! - **Orthographic**: Parallel projection

use super::{HdRenderParam, HdSceneDelegate, HdSprim};
use crate::types::HdDirtyBits;
use std::sync::LazyLock;
use usd_gf::{
    Matrix4d, Range1f, Vec2f, Vec4d,
    camera::{
        APERTURE_UNIT, Camera as GfCamera, CameraProjection as GfCameraProjection,
        FOCAL_LENGTH_UNIT,
    },
};
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

// ---------------------------------------------------------------------------
// Camera tokens (mirrors HD_CAMERA_TOKENS)
// ---------------------------------------------------------------------------

// Frustum
static TOKEN_PROJECTION: LazyLock<Token> = LazyLock::new(|| Token::new("projection"));
static TOKEN_HORIZONTAL_APERTURE: LazyLock<Token> =
    LazyLock::new(|| Token::new("horizontalAperture"));
static TOKEN_VERTICAL_APERTURE: LazyLock<Token> = LazyLock::new(|| Token::new("verticalAperture"));
static TOKEN_HORIZONTAL_APERTURE_OFFSET: LazyLock<Token> =
    LazyLock::new(|| Token::new("horizontalApertureOffset"));
static TOKEN_VERTICAL_APERTURE_OFFSET: LazyLock<Token> =
    LazyLock::new(|| Token::new("verticalApertureOffset"));
static TOKEN_FOCAL_LENGTH: LazyLock<Token> = LazyLock::new(|| Token::new("focalLength"));
static TOKEN_CLIPPING_RANGE: LazyLock<Token> = LazyLock::new(|| Token::new("clippingRange"));
static TOKEN_CLIP_PLANES: LazyLock<Token> = LazyLock::new(|| Token::new("clipPlanes"));

// Depth of field
static TOKEN_F_STOP: LazyLock<Token> = LazyLock::new(|| Token::new("fStop"));
static TOKEN_FOCUS_DISTANCE: LazyLock<Token> = LazyLock::new(|| Token::new("focusDistance"));
static TOKEN_FOCUS_ON: LazyLock<Token> = LazyLock::new(|| Token::new("focusOn"));
static TOKEN_DOF_ASPECT: LazyLock<Token> = LazyLock::new(|| Token::new("dofAspect"));

// Split diopter
static TOKEN_SPLIT_DIOPTER_COUNT: LazyLock<Token> =
    LazyLock::new(|| Token::new("splitDiopter:count"));
static TOKEN_SPLIT_DIOPTER_ANGLE: LazyLock<Token> =
    LazyLock::new(|| Token::new("splitDiopter:angle"));
static TOKEN_SPLIT_DIOPTER_OFFSET1: LazyLock<Token> =
    LazyLock::new(|| Token::new("splitDiopter:offset1"));
static TOKEN_SPLIT_DIOPTER_WIDTH1: LazyLock<Token> =
    LazyLock::new(|| Token::new("splitDiopter:width1"));
static TOKEN_SPLIT_DIOPTER_FOCUS_DISTANCE1: LazyLock<Token> =
    LazyLock::new(|| Token::new("splitDiopter:focusDistance1"));
static TOKEN_SPLIT_DIOPTER_OFFSET2: LazyLock<Token> =
    LazyLock::new(|| Token::new("splitDiopter:offset2"));
static TOKEN_SPLIT_DIOPTER_WIDTH2: LazyLock<Token> =
    LazyLock::new(|| Token::new("splitDiopter:width2"));
static TOKEN_SPLIT_DIOPTER_FOCUS_DISTANCE2: LazyLock<Token> =
    LazyLock::new(|| Token::new("splitDiopter:focusDistance2"));

// Shutter / exposure
static TOKEN_SHUTTER_OPEN: LazyLock<Token> = LazyLock::new(|| Token::new("shutterOpen"));
static TOKEN_SHUTTER_CLOSE: LazyLock<Token> = LazyLock::new(|| Token::new("shutterClose"));
static TOKEN_EXPOSURE: LazyLock<Token> = LazyLock::new(|| Token::new("exposure"));
static TOKEN_EXPOSURE_TIME: LazyLock<Token> = LazyLock::new(|| Token::new("exposureTime"));
static TOKEN_EXPOSURE_ISO: LazyLock<Token> = LazyLock::new(|| Token::new("exposureIso"));
static TOKEN_EXPOSURE_FSTOP: LazyLock<Token> = LazyLock::new(|| Token::new("exposureFStop"));
static TOKEN_EXPOSURE_RESPONSIVITY: LazyLock<Token> =
    LazyLock::new(|| Token::new("exposureResponsivity"));
static TOKEN_LINEAR_EXPOSURE_SCALE: LazyLock<Token> =
    LazyLock::new(|| Token::new("linearExposureScale"));

// Window policy
static TOKEN_WINDOW_POLICY: LazyLock<Token> = LazyLock::new(|| Token::new("windowPolicy"));

// Lens distortion
static TOKEN_LENS_DISTORTION_TYPE: LazyLock<Token> =
    LazyLock::new(|| Token::new("lensDistortion:type"));
static TOKEN_LENS_DISTORTION_K1: LazyLock<Token> =
    LazyLock::new(|| Token::new("lensDistortion:k1"));
static TOKEN_LENS_DISTORTION_K2: LazyLock<Token> =
    LazyLock::new(|| Token::new("lensDistortion:k2"));
static TOKEN_LENS_DISTORTION_CENTER: LazyLock<Token> =
    LazyLock::new(|| Token::new("lensDistortion:center"));
static TOKEN_LENS_DISTORTION_ANA_SQ: LazyLock<Token> =
    LazyLock::new(|| Token::new("lensDistortion:anaSq"));
static TOKEN_LENS_DISTORTION_ASYM: LazyLock<Token> =
    LazyLock::new(|| Token::new("lensDistortion:asym"));
static TOKEN_LENS_DISTORTION_SCALE: LazyLock<Token> =
    LazyLock::new(|| Token::new("lensDistortion:scale"));
static TOKEN_LENS_DISTORTION_IOR: LazyLock<Token> =
    LazyLock::new(|| Token::new("lensDistortion:ior"));

// Lens distortion type tokens
static TOKEN_STANDARD: LazyLock<Token> = LazyLock::new(|| Token::new("standard"));

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// Camera projection type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HdCameraProjection {
    /// Perspective projection with vanishing point.
    Perspective = 0,
    /// Orthographic projection (parallel lines).
    Orthographic = 1,
}

/// Window conform policy for aspect ratio handling.
///
/// Mirrors CameraUtilConformWindowPolicy from cameraUtil.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CameraUtilConformWindowPolicy {
    /// Match vertically, crop or letterbox horizontally.
    MatchVertically,
    /// Match horizontally, crop or pillarbox vertically.
    MatchHorizontally,
    /// Fit window inside, preserving entire frustum.
    Fit,
    /// Crop window to fill, possibly losing edges.
    Crop,
    /// Don't conform - use provided window as-is.
    DontConform,
}

impl Default for CameraUtilConformWindowPolicy {
    fn default() -> Self {
        Self::Fit
    }
}

// ---------------------------------------------------------------------------
// Dirty bits (mirrors C++ HdCamera::DirtyBits)
// ---------------------------------------------------------------------------

/// Camera-specific dirty bit constants.
pub struct HdCameraDirtyBits;

impl HdCameraDirtyBits {
    /// Clean state - no changes.
    pub const CLEAN: HdDirtyBits = 0;
    /// Camera transform changed.
    pub const DIRTY_TRANSFORM: HdDirtyBits = 1 << 0;
    /// Camera parameters changed.
    pub const DIRTY_PARAMS: HdDirtyBits = 1 << 1;
    /// Clip planes changed.
    pub const DIRTY_CLIP_PLANES: HdDirtyBits = 1 << 2;
    /// Window policy changed.
    pub const DIRTY_WINDOW_POLICY: HdDirtyBits = 1 << 3;
    /// All dirty bits combined.
    pub const ALL_DIRTY: HdDirtyBits = Self::DIRTY_TRANSFORM
        | Self::DIRTY_PARAMS
        | Self::DIRTY_CLIP_PLANES
        | Self::DIRTY_WINDOW_POLICY;
}

// ---------------------------------------------------------------------------
// HdCamera
// ---------------------------------------------------------------------------

/// Camera state primitive - full port of C++ HdCamera.
///
/// Stores all camera parameters synced from scene delegate.
/// Backends can inherit and pull additional parameters.
#[derive(Debug)]
pub struct HdCamera {
    // Identity
    id: SdfPath,
    dirty_bits: HdDirtyBits,

    // Frustum
    transform: Matrix4d,
    projection: HdCameraProjection,
    horizontal_aperture: f32,
    vertical_aperture: f32,
    horizontal_aperture_offset: f32,
    vertical_aperture_offset: f32,
    focal_length: f32,
    clipping_range: Range1f,
    clip_planes: Vec<Vec4d>,

    // Depth of field
    f_stop: f32,
    focus_distance: f32,
    focus_on: bool,
    dof_aspect: f32,

    // Split diopter
    split_diopter_count: i32,
    split_diopter_angle: f32,
    split_diopter_offset1: f32,
    split_diopter_width1: f32,
    split_diopter_focus_distance1: f32,
    split_diopter_offset2: f32,
    split_diopter_width2: f32,
    split_diopter_focus_distance2: f32,

    // Shutter
    shutter_open: f64,
    shutter_close: f64,

    // Exposure
    exposure: f32,
    exposure_time: f32,
    exposure_iso: f32,
    exposure_f_stop: f32,
    exposure_responsivity: f32,
    linear_exposure_scale: f32,

    // Lens distortion
    lens_distortion_type: Token,
    lens_distortion_k1: f32,
    lens_distortion_k2: f32,
    lens_distortion_center: Vec2f,
    lens_distortion_ana_sq: f32,
    lens_distortion_asym: Vec2f,
    lens_distortion_scale: f32,
    lens_distortion_ior: f32,

    // Window policy
    window_policy: CameraUtilConformWindowPolicy,
}

impl HdCamera {
    /// Create a new camera with default values (matches C++ constructor).
    pub fn new(id: SdfPath) -> Self {
        Self {
            id,
            dirty_bits: HdCameraDirtyBits::ALL_DIRTY,

            // Frustum defaults
            transform: Matrix4d::identity(),
            projection: HdCameraProjection::Perspective,
            horizontal_aperture: 0.0,
            vertical_aperture: 0.0,
            horizontal_aperture_offset: 0.0,
            vertical_aperture_offset: 0.0,
            focal_length: 0.0,
            clipping_range: Range1f::default(),
            clip_planes: Vec::new(),

            // DOF defaults
            f_stop: 0.0,
            focus_distance: 0.0,
            focus_on: false,
            dof_aspect: 1.0,

            // Split diopter defaults
            split_diopter_count: 0,
            split_diopter_angle: 0.0,
            split_diopter_offset1: 0.0,
            split_diopter_width1: 0.0,
            split_diopter_focus_distance1: 0.0,
            split_diopter_offset2: 0.0,
            split_diopter_width2: 0.0,
            split_diopter_focus_distance2: 0.0,

            // Shutter defaults
            shutter_open: 0.0,
            shutter_close: 0.0,

            // Exposure defaults
            exposure: 0.0,
            exposure_time: 1.0,
            exposure_iso: 100.0,
            exposure_f_stop: 1.0,
            exposure_responsivity: 1.0,
            linear_exposure_scale: 1.0,

            // Lens distortion defaults
            lens_distortion_type: TOKEN_STANDARD.clone(),
            lens_distortion_k1: 0.0,
            lens_distortion_k2: 0.0,
            lens_distortion_center: Vec2f::new(0.0, 0.0),
            lens_distortion_ana_sq: 1.0,
            lens_distortion_asym: Vec2f::new(0.0, 0.0),
            lens_distortion_scale: 1.0,
            lens_distortion_ior: 0.0,

            // Window policy default
            window_policy: CameraUtilConformWindowPolicy::Fit,
        }
    }

    // =========================================================================
    // Frustum accessors
    // =========================================================================

    /// Camera transform (world space).
    pub fn get_transform(&self) -> &Matrix4d {
        &self.transform
    }

    /// Projection type (perspective or orthographic).
    pub fn get_projection(&self) -> HdCameraProjection {
        self.projection
    }

    /// Horizontal aperture in world units.
    pub fn get_horizontal_aperture(&self) -> f32 {
        self.horizontal_aperture
    }

    /// Vertical aperture in world units.
    pub fn get_vertical_aperture(&self) -> f32 {
        self.vertical_aperture
    }

    /// Horizontal aperture offset in world units.
    pub fn get_horizontal_aperture_offset(&self) -> f32 {
        self.horizontal_aperture_offset
    }

    /// Vertical aperture offset in world units.
    pub fn get_vertical_aperture_offset(&self) -> f32 {
        self.vertical_aperture_offset
    }

    /// Focal length in world units.
    pub fn get_focal_length(&self) -> f32 {
        self.focal_length
    }

    /// Near and far clipping range.
    pub fn get_clipping_range(&self) -> &Range1f {
        &self.clipping_range
    }

    /// Additional clipping planes in camera space.
    pub fn get_clip_planes(&self) -> &[Vec4d] {
        &self.clip_planes
    }

    // =========================================================================
    // Depth of field accessors
    // =========================================================================

    /// F-stop value for depth of field.
    pub fn get_f_stop(&self) -> f32 {
        self.f_stop
    }

    /// Focus distance in world units.
    pub fn get_focus_distance(&self) -> f32 {
        self.focus_distance
    }

    /// Whether focus is enabled.
    pub fn get_focus_on(&self) -> bool {
        self.focus_on
    }

    /// DOF aspect ratio.
    pub fn get_dof_aspect(&self) -> f32 {
        self.dof_aspect
    }

    // =========================================================================
    // Split diopter accessors
    // =========================================================================

    /// Number of split diopter planes.
    pub fn get_split_diopter_count(&self) -> i32 {
        self.split_diopter_count
    }
    /// Split diopter rotation angle in degrees.
    pub fn get_split_diopter_angle(&self) -> f32 {
        self.split_diopter_angle
    }
    /// Split diopter plane 1 offset from center.
    pub fn get_split_diopter_offset1(&self) -> f32 {
        self.split_diopter_offset1
    }
    /// Split diopter plane 1 transition width.
    pub fn get_split_diopter_width1(&self) -> f32 {
        self.split_diopter_width1
    }
    /// Split diopter plane 1 focus distance.
    pub fn get_split_diopter_focus_distance1(&self) -> f32 {
        self.split_diopter_focus_distance1
    }
    /// Split diopter plane 2 offset from center.
    pub fn get_split_diopter_offset2(&self) -> f32 {
        self.split_diopter_offset2
    }
    /// Split diopter plane 2 transition width.
    pub fn get_split_diopter_width2(&self) -> f32 {
        self.split_diopter_width2
    }
    /// Split diopter plane 2 focus distance.
    pub fn get_split_diopter_focus_distance2(&self) -> f32 {
        self.split_diopter_focus_distance2
    }

    // =========================================================================
    // Shutter / exposure accessors
    // =========================================================================

    /// Shutter open time (frame-relative).
    pub fn get_shutter_open(&self) -> f64 {
        self.shutter_open
    }
    /// Shutter close time (frame-relative).
    pub fn get_shutter_close(&self) -> f64 {
        self.shutter_close
    }

    /// Raw exposure exponent. Prefer get_linear_exposure_scale() in most cases.
    pub fn get_exposure(&self) -> f32 {
        self.exposure
    }

    /// Computed linear exposure scale from all exposure attributes.
    pub fn get_linear_exposure_scale(&self) -> f32 {
        self.linear_exposure_scale
    }

    // =========================================================================
    // Lens distortion accessors
    // =========================================================================

    /// Lens distortion model type (e.g. "standard").
    pub fn get_lens_distortion_type(&self) -> &Token {
        &self.lens_distortion_type
    }
    /// Radial distortion coefficient k1.
    pub fn get_lens_distortion_k1(&self) -> f32 {
        self.lens_distortion_k1
    }
    /// Radial distortion coefficient k2.
    pub fn get_lens_distortion_k2(&self) -> f32 {
        self.lens_distortion_k2
    }
    /// Distortion center offset.
    pub fn get_lens_distortion_center(&self) -> &Vec2f {
        &self.lens_distortion_center
    }
    /// Anamorphic squeeze ratio.
    pub fn get_lens_distortion_ana_sq(&self) -> f32 {
        self.lens_distortion_ana_sq
    }
    /// Asymmetric distortion vector.
    pub fn get_lens_distortion_asym(&self) -> &Vec2f {
        &self.lens_distortion_asym
    }
    /// Distortion scale factor.
    pub fn get_lens_distortion_scale(&self) -> f32 {
        self.lens_distortion_scale
    }
    /// Index of refraction for distortion model.
    pub fn get_lens_distortion_ior(&self) -> f32 {
        self.lens_distortion_ior
    }

    // =========================================================================
    // Window policy
    // =========================================================================

    /// Window conform policy. Default: Fit.
    pub fn get_window_policy(&self) -> CameraUtilConformWindowPolicy {
        self.window_policy
    }

    // =========================================================================
    // Convenience: projection matrix
    // =========================================================================

    /// Compute projection matrix from physical camera params.
    ///
    /// Mirrors C++ HdCamera::ComputeProjectionMatrix().
    /// Converts world-unit apertures to GfCamera units and builds frustum.
    pub fn compute_projection_matrix(&self) -> Matrix4d {
        let mut cam = GfCamera::new();

        let gf_proj = match self.projection {
            HdCameraProjection::Orthographic => GfCameraProjection::Orthographic,
            HdCameraProjection::Perspective => GfCameraProjection::Perspective,
        };
        cam.set_projection(gf_proj);

        cam.set_horizontal_aperture((self.horizontal_aperture as f64 / APERTURE_UNIT) as f32);
        cam.set_vertical_aperture((self.vertical_aperture as f64 / APERTURE_UNIT) as f32);
        cam.set_horizontal_aperture_offset(
            (self.horizontal_aperture_offset as f64 / APERTURE_UNIT) as f32,
        );
        cam.set_vertical_aperture_offset(
            (self.vertical_aperture_offset as f64 / APERTURE_UNIT) as f32,
        );
        cam.set_focal_length((self.focal_length as f64 / FOCAL_LENGTH_UNIT) as f32);
        cam.set_clipping_range(self.clipping_range);

        cam.frustum().compute_projection_matrix()
    }

    /// View matrix is the inverse of the camera transform.
    pub fn get_view_matrix(&self) -> Matrix4d {
        self.transform.inverse().unwrap_or_else(Matrix4d::identity)
    }

    /// Get the projection matrix (convenience wrapper for compute_projection_matrix).
    pub fn get_projection_matrix(&self) -> Matrix4d {
        self.compute_projection_matrix()
    }

    // =========================================================================
    // Aspect ratio (convenience)
    // =========================================================================

    /// Aspect ratio from apertures.
    pub fn get_aspect_ratio(&self) -> f32 {
        if self.vertical_aperture.abs() < f32::EPSILON {
            1.0
        } else {
            self.horizontal_aperture / self.vertical_aperture
        }
    }
}

// ---------------------------------------------------------------------------
// HdSprim implementation
// ---------------------------------------------------------------------------

impl HdSprim for HdCamera {
    fn get_id(&self) -> &SdfPath {
        &self.id
    }

    fn get_dirty_bits(&self) -> HdDirtyBits {
        self.dirty_bits
    }

    fn set_dirty_bits(&mut self, bits: HdDirtyBits) {
        self.dirty_bits = bits;
    }

    /// Sync camera params from scene delegate. Mirrors C++ HdCamera::Sync.
    fn sync(
        &mut self,
        delegate: &dyn HdSceneDelegate,
        _render_param: Option<&dyn HdRenderParam>,
        dirty_bits: &mut HdDirtyBits,
    ) {
        let id = self.id.clone();
        let bits = *dirty_bits;

        // Transform
        if (bits & HdCameraDirtyBits::DIRTY_TRANSFORM) != 0 {
            self.transform = delegate.get_transform(&id);
        }

        // Camera parameters
        if (bits & HdCameraDirtyBits::DIRTY_PARAMS) != 0 {
            // Helper macro to pull typed values from delegate
            macro_rules! pull_param {
                ($field:ident, $token:expr, $ty:ty) => {
                    let v = delegate.get_camera_param_value(&id, &$token);
                    if let Some(val) = v.get::<$ty>() {
                        self.$field = *val;
                    }
                };
            }

            // Projection (stored as i32 enum in scene delegate)
            let v = delegate.get_camera_param_value(&id, &TOKEN_PROJECTION);
            if let Some(val) = v.get::<i32>() {
                self.projection = match *val {
                    1 => HdCameraProjection::Orthographic,
                    _ => HdCameraProjection::Perspective,
                };
            }

            // Frustum params
            pull_param!(horizontal_aperture, TOKEN_HORIZONTAL_APERTURE, f32);
            pull_param!(vertical_aperture, TOKEN_VERTICAL_APERTURE, f32);
            pull_param!(
                horizontal_aperture_offset,
                TOKEN_HORIZONTAL_APERTURE_OFFSET,
                f32
            );
            pull_param!(
                vertical_aperture_offset,
                TOKEN_VERTICAL_APERTURE_OFFSET,
                f32
            );
            pull_param!(focal_length, TOKEN_FOCAL_LENGTH, f32);

            // Clipping range (Range1f)
            let v = delegate.get_camera_param_value(&id, &TOKEN_CLIPPING_RANGE);
            if let Some(val) = v.get::<Range1f>() {
                self.clipping_range = *val;
            }

            // DOF
            pull_param!(f_stop, TOKEN_F_STOP, f32);
            pull_param!(focus_distance, TOKEN_FOCUS_DISTANCE, f32);

            let v = delegate.get_camera_param_value(&id, &TOKEN_FOCUS_ON);
            if let Some(val) = v.get::<bool>() {
                self.focus_on = *val;
            }

            pull_param!(dof_aspect, TOKEN_DOF_ASPECT, f32);

            // Split diopter
            let v = delegate.get_camera_param_value(&id, &TOKEN_SPLIT_DIOPTER_COUNT);
            if let Some(val) = v.get::<i32>() {
                self.split_diopter_count = *val;
            }
            pull_param!(split_diopter_angle, TOKEN_SPLIT_DIOPTER_ANGLE, f32);
            pull_param!(split_diopter_offset1, TOKEN_SPLIT_DIOPTER_OFFSET1, f32);
            pull_param!(split_diopter_width1, TOKEN_SPLIT_DIOPTER_WIDTH1, f32);
            pull_param!(
                split_diopter_focus_distance1,
                TOKEN_SPLIT_DIOPTER_FOCUS_DISTANCE1,
                f32
            );
            pull_param!(split_diopter_offset2, TOKEN_SPLIT_DIOPTER_OFFSET2, f32);
            pull_param!(split_diopter_width2, TOKEN_SPLIT_DIOPTER_WIDTH2, f32);
            pull_param!(
                split_diopter_focus_distance2,
                TOKEN_SPLIT_DIOPTER_FOCUS_DISTANCE2,
                f32
            );

            // Shutter
            let v = delegate.get_camera_param_value(&id, &TOKEN_SHUTTER_OPEN);
            if let Some(val) = v.get::<f64>() {
                self.shutter_open = *val;
            }
            let v = delegate.get_camera_param_value(&id, &TOKEN_SHUTTER_CLOSE);
            if let Some(val) = v.get::<f64>() {
                self.shutter_close = *val;
            }

            // Exposure
            pull_param!(exposure, TOKEN_EXPOSURE, f32);
            pull_param!(exposure_time, TOKEN_EXPOSURE_TIME, f32);
            pull_param!(exposure_iso, TOKEN_EXPOSURE_ISO, f32);
            pull_param!(exposure_f_stop, TOKEN_EXPOSURE_FSTOP, f32);
            pull_param!(exposure_responsivity, TOKEN_EXPOSURE_RESPONSIVITY, f32);
            pull_param!(linear_exposure_scale, TOKEN_LINEAR_EXPOSURE_SCALE, f32);

            // Lens distortion
            let v = delegate.get_camera_param_value(&id, &TOKEN_LENS_DISTORTION_TYPE);
            if let Some(val) = v.get::<Token>() {
                self.lens_distortion_type = val.clone();
            }
            pull_param!(lens_distortion_k1, TOKEN_LENS_DISTORTION_K1, f32);
            pull_param!(lens_distortion_k2, TOKEN_LENS_DISTORTION_K2, f32);
            let v = delegate.get_camera_param_value(&id, &TOKEN_LENS_DISTORTION_CENTER);
            if let Some(val) = v.get::<Vec2f>() {
                self.lens_distortion_center = *val;
            }
            pull_param!(lens_distortion_ana_sq, TOKEN_LENS_DISTORTION_ANA_SQ, f32);
            let v = delegate.get_camera_param_value(&id, &TOKEN_LENS_DISTORTION_ASYM);
            if let Some(val) = v.get::<Vec2f>() {
                self.lens_distortion_asym = *val;
            }
            pull_param!(lens_distortion_scale, TOKEN_LENS_DISTORTION_SCALE, f32);
            pull_param!(lens_distortion_ior, TOKEN_LENS_DISTORTION_IOR, f32);
        }

        // Window policy
        if (bits & HdCameraDirtyBits::DIRTY_WINDOW_POLICY) != 0 {
            let v = delegate.get_camera_param_value(&id, &TOKEN_WINDOW_POLICY);
            if let Some(val) = v.get::<i32>() {
                self.window_policy = match *val {
                    0 => CameraUtilConformWindowPolicy::MatchVertically,
                    1 => CameraUtilConformWindowPolicy::MatchHorizontally,
                    2 => CameraUtilConformWindowPolicy::Fit,
                    3 => CameraUtilConformWindowPolicy::Crop,
                    4 => CameraUtilConformWindowPolicy::DontConform,
                    _ => CameraUtilConformWindowPolicy::Fit,
                };
            }
        }

        // Clip planes
        if (bits & HdCameraDirtyBits::DIRTY_CLIP_PLANES) != 0 {
            let v = delegate.get_camera_param_value(&id, &TOKEN_CLIP_PLANES);
            if let Some(planes) = v.get::<Vec<Vec4d>>() {
                self.clip_planes = planes.clone();
            }
        }

        // Clear all dirty bits
        *dirty_bits = HdCameraDirtyBits::CLEAN;
        self.dirty_bits = HdCameraDirtyBits::CLEAN;
    }

    fn get_initial_dirty_bits_mask() -> HdDirtyBits
    where
        Self: Sized,
    {
        HdCameraDirtyBits::ALL_DIRTY
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_camera_creation() {
        let id = SdfPath::from_string("/Camera").unwrap();
        let camera = HdCamera::new(id.clone());

        assert_eq!(camera.get_id(), &id);
        assert!(camera.is_dirty());
        assert_eq!(camera.get_projection(), HdCameraProjection::Perspective);
    }

    #[test]
    fn test_camera_defaults() {
        let camera = HdCamera::new(SdfPath::from_string("/Camera").unwrap());

        // Frustum defaults
        assert_eq!(camera.get_horizontal_aperture(), 0.0);
        assert_eq!(camera.get_vertical_aperture(), 0.0);
        assert_eq!(camera.get_focal_length(), 0.0);

        // DOF defaults
        assert_eq!(camera.get_f_stop(), 0.0);
        assert_eq!(camera.get_focus_distance(), 0.0);
        assert!(!camera.get_focus_on());
        assert_eq!(camera.get_dof_aspect(), 1.0);

        // Exposure defaults
        assert_eq!(camera.get_exposure(), 0.0);
        assert_eq!(camera.get_linear_exposure_scale(), 1.0);

        // Lens distortion defaults
        assert_eq!(camera.get_lens_distortion_type().as_str(), "standard");
        assert_eq!(camera.get_lens_distortion_k1(), 0.0);
        assert_eq!(camera.get_lens_distortion_ana_sq(), 1.0);
        assert_eq!(camera.get_lens_distortion_scale(), 1.0);

        // Window policy default
        assert_eq!(
            camera.get_window_policy(),
            CameraUtilConformWindowPolicy::Fit
        );
    }

    #[test]
    fn test_camera_aspect_ratio() {
        let mut camera = HdCamera::new(SdfPath::from_string("/Camera").unwrap());

        // With zero apertures, aspect should be 1.0 (safe division)
        assert_eq!(camera.get_aspect_ratio(), 1.0);

        // Set apertures manually for testing
        camera.horizontal_aperture = 36.0;
        camera.vertical_aperture = 24.0;
        let aspect = camera.get_aspect_ratio();
        assert!((aspect - 1.5).abs() < 0.001);
    }

    #[test]
    fn test_camera_dirty_bits() {
        let camera = HdCamera::new(SdfPath::from_string("/Camera").unwrap());
        assert_eq!(camera.get_dirty_bits(), HdCameraDirtyBits::ALL_DIRTY);
    }

    #[test]
    fn test_camera_projection_types() {
        let mut camera = HdCamera::new(SdfPath::from_string("/Camera").unwrap());

        assert_eq!(camera.get_projection(), HdCameraProjection::Perspective);
        camera.projection = HdCameraProjection::Orthographic;
        assert_eq!(camera.get_projection(), HdCameraProjection::Orthographic);
    }

    #[test]
    fn test_camera_view_matrix() {
        let camera = HdCamera::new(SdfPath::from_string("/Camera").unwrap());

        // Default transform is identity, so view matrix is also identity
        let view = camera.get_view_matrix();
        assert_eq!(view, Matrix4d::identity());
    }

    #[test]
    fn test_window_policy_values() {
        assert_ne!(
            CameraUtilConformWindowPolicy::Fit,
            CameraUtilConformWindowPolicy::Crop
        );
        assert_eq!(
            CameraUtilConformWindowPolicy::default(),
            CameraUtilConformWindowPolicy::Fit
        );
    }

    #[test]
    fn test_split_diopter_defaults() {
        let camera = HdCamera::new(SdfPath::from_string("/Camera").unwrap());
        assert_eq!(camera.get_split_diopter_count(), 0);
        assert_eq!(camera.get_split_diopter_angle(), 0.0);
    }

    #[test]
    fn test_shutter_defaults() {
        let camera = HdCamera::new(SdfPath::from_string("/Camera").unwrap());
        assert_eq!(camera.get_shutter_open(), 0.0);
        assert_eq!(camera.get_shutter_close(), 0.0);
    }
}
