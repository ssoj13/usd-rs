//! UsdGeomCamera - camera geometry schema.
//!
//! Port of pxr/usd/usdGeom/camera.h/cpp
//!
//! Transformable camera with optical properties.

use super::tokens::usd_geom_tokens;
use super::xformable::Xformable;
use usd_core::attribute::Variability;
use usd_core::{Attribute, Prim, Stage};
use usd_gf::camera::{Camera as GfCamera, CameraProjection};
use usd_gf::matrix4::Matrix4d;
use usd_gf::range::Range1f;
use usd_gf::vec2::Vec2f;
use usd_gf::vec4::Vec4f;
use usd_sdf::{TimeCode, ValueTypeRegistry};
use usd_tf::Token;
use usd_vt::Value;

// ============================================================================
// Camera
// ============================================================================

/// Camera geometry schema.
///
/// Transformable camera with optical properties.
///
/// Matches C++ `UsdGeomCamera`.
#[derive(Debug, Clone)]
pub struct Camera {
    /// Base xformable schema.
    inner: Xformable,
}

impl Camera {
    /// Creates a Camera schema from a prim.
    ///
    /// Matches C++ `UsdGeomCamera(const UsdPrim& prim)`.
    pub fn new(prim: Prim) -> Self {
        Self {
            inner: Xformable::new(prim),
        }
    }

    /// Creates a Camera schema from a Xformable schema.
    ///
    /// Matches C++ `UsdGeomCamera(const UsdSchemaBase& schemaObj)`.
    pub fn from_xformable(xformable: Xformable) -> Self {
        Self { inner: xformable }
    }

    /// Creates an invalid Camera schema.
    pub fn invalid() -> Self {
        Self {
            inner: Xformable::invalid(),
        }
    }

    /// Returns true if this schema is valid.
    pub fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    /// Returns the wrapped prim.
    pub fn prim(&self) -> &Prim {
        self.inner.prim()
    }

    /// Returns the xformable base.
    pub fn xformable(&self) -> &Xformable {
        &self.inner
    }

    /// Returns the schema type name.
    pub fn schema_type_name() -> Token {
        Token::new("Camera")
    }

    /// Return a Camera holding the prim adhering to this schema at path on stage.
    ///
    /// Matches C++ `UsdGeomCamera::Get(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn get(stage: &Stage, path: &usd_sdf::Path) -> Self {
        if let Some(prim) = stage.get_prim_at_path(path) {
            Self::new(prim)
        } else {
            Self::invalid()
        }
    }

    /// Attempt to ensure a prim adhering to this schema at path is defined on this stage.
    ///
    /// Matches C++ `UsdGeomCamera::Define(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn define(stage: &Stage, path: &usd_sdf::Path) -> Self {
        match stage.define_prim(path.get_string(), Self::schema_type_name().as_str()) {
            Ok(prim) => Self::new(prim),
            Err(_) => Self::invalid(),
        }
    }

    // ========================================================================
    // Projection
    // ========================================================================

    /// Returns the projection attribute.
    ///
    /// Matches C++ `GetProjectionAttr()`.
    pub fn get_projection_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().projection.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the projection attribute.
    ///
    /// Matches C++ `CreateProjectionAttr()`.
    pub fn create_projection_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let token_type = registry.find_type_by_token(&Token::new("token"));

        prim.create_attribute(
            usd_geom_tokens().projection.as_str(),
            &token_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // HorizontalAperture
    // ========================================================================

    /// Returns the horizontalAperture attribute.
    ///
    /// Matches C++ `GetHorizontalApertureAttr()`.
    pub fn get_horizontal_aperture_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().horizontal_aperture.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the horizontalAperture attribute.
    ///
    /// Matches C++ `CreateHorizontalApertureAttr()`.
    pub fn create_horizontal_aperture_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let float_type = registry.find_type_by_token(&Token::new("float"));

        prim.create_attribute(
            usd_geom_tokens().horizontal_aperture.as_str(),
            &float_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // VerticalAperture
    // ========================================================================

    /// Returns the verticalAperture attribute.
    ///
    /// Matches C++ `GetVerticalApertureAttr()`.
    pub fn get_vertical_aperture_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().vertical_aperture.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the verticalAperture attribute.
    ///
    /// Matches C++ `CreateVerticalApertureAttr()`.
    pub fn create_vertical_aperture_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let float_type = registry.find_type_by_token(&Token::new("float"));

        prim.create_attribute(
            usd_geom_tokens().vertical_aperture.as_str(),
            &float_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // HorizontalApertureOffset
    // ========================================================================

    /// Returns the horizontalApertureOffset attribute.
    ///
    /// Matches C++ `GetHorizontalApertureOffsetAttr()`.
    pub fn get_horizontal_aperture_offset_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().horizontal_aperture_offset.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the horizontalApertureOffset attribute.
    ///
    /// Matches C++ `CreateHorizontalApertureOffsetAttr()`.
    pub fn create_horizontal_aperture_offset_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let float_type = registry.find_type_by_token(&Token::new("float"));

        prim.create_attribute(
            usd_geom_tokens().horizontal_aperture_offset.as_str(),
            &float_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // VerticalApertureOffset
    // ========================================================================

    /// Returns the verticalApertureOffset attribute.
    ///
    /// Matches C++ `GetVerticalApertureOffsetAttr()`.
    pub fn get_vertical_aperture_offset_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().vertical_aperture_offset.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the verticalApertureOffset attribute.
    ///
    /// Matches C++ `CreateVerticalApertureOffsetAttr()`.
    pub fn create_vertical_aperture_offset_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let float_type = registry.find_type_by_token(&Token::new("float"));

        prim.create_attribute(
            usd_geom_tokens().vertical_aperture_offset.as_str(),
            &float_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // FocalLength
    // ========================================================================

    /// Returns the focalLength attribute.
    ///
    /// Matches C++ `GetFocalLengthAttr()`.
    pub fn get_focal_length_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().focal_length.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the focalLength attribute.
    ///
    /// Matches C++ `CreateFocalLengthAttr()`.
    pub fn create_focal_length_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let float_type = registry.find_type_by_token(&Token::new("float"));

        prim.create_attribute(
            usd_geom_tokens().focal_length.as_str(),
            &float_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // ClippingRange
    // ========================================================================

    /// Returns the clippingRange attribute.
    ///
    /// Matches C++ `GetClippingRangeAttr()`.
    pub fn get_clipping_range_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().clipping_range.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the clippingRange attribute.
    ///
    /// Matches C++ `CreateClippingRangeAttr()`.
    pub fn create_clipping_range_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let float2_type = registry.find_type_by_token(&Token::new("float2"));

        prim.create_attribute(
            usd_geom_tokens().clipping_range.as_str(),
            &float2_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // ClippingPlanes
    // ========================================================================

    /// Returns the clippingPlanes attribute.
    ///
    /// Matches C++ `GetClippingPlanesAttr()`.
    pub fn get_clipping_planes_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().clipping_planes.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the clippingPlanes attribute.
    ///
    /// Matches C++ `CreateClippingPlanesAttr()`.
    pub fn create_clipping_planes_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let float4_array_type = registry.find_type_by_token(&Token::new("float4[]"));

        prim.create_attribute(
            usd_geom_tokens().clipping_planes.as_str(),
            &float4_array_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // FStop
    // ========================================================================

    /// Returns the fStop attribute.
    ///
    /// Matches C++ `GetFStopAttr()`.
    pub fn get_f_stop_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().f_stop.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the fStop attribute.
    ///
    /// Matches C++ `CreateFStopAttr()`.
    pub fn create_f_stop_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let float_type = registry.find_type_by_token(&Token::new("float"));

        prim.create_attribute(
            usd_geom_tokens().f_stop.as_str(),
            &float_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // FocusDistance
    // ========================================================================

    /// Returns the focusDistance attribute.
    ///
    /// Matches C++ `GetFocusDistanceAttr()`.
    pub fn get_focus_distance_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().focus_distance.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the focusDistance attribute.
    ///
    /// Matches C++ `CreateFocusDistanceAttr()`.
    pub fn create_focus_distance_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let float_type = registry.find_type_by_token(&Token::new("float"));

        prim.create_attribute(
            usd_geom_tokens().focus_distance.as_str(),
            &float_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // StereoRole
    // ========================================================================

    /// Returns the stereoRole attribute.
    ///
    /// Matches C++ `GetStereoRoleAttr()`.
    pub fn get_stereo_role_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().stereo_role.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the stereoRole attribute.
    ///
    /// Matches C++ `CreateStereoRoleAttr()`.
    pub fn create_stereo_role_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let token_type = registry.find_type_by_token(&Token::new("token"));

        prim.create_attribute(
            usd_geom_tokens().stereo_role.as_str(),
            &token_type,
            false,                      // not custom
            Some(Variability::Uniform), // uniform
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // ShutterOpen
    // ========================================================================

    /// Returns the shutterOpen attribute.
    ///
    /// Matches C++ `GetShutterOpenAttr()`.
    pub fn get_shutter_open_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().shutter_open.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the shutterOpen attribute.
    ///
    /// Matches C++ `CreateShutterOpenAttr()`.
    pub fn create_shutter_open_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let double_type = registry.find_type_by_token(&Token::new("double"));

        prim.create_attribute(
            usd_geom_tokens().shutter_open.as_str(),
            &double_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // ShutterClose
    // ========================================================================

    /// Returns the shutterClose attribute.
    ///
    /// Matches C++ `GetShutterCloseAttr()`.
    pub fn get_shutter_close_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().shutter_close.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the shutterClose attribute.
    ///
    /// Matches C++ `CreateShutterCloseAttr()`.
    pub fn create_shutter_close_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let double_type = registry.find_type_by_token(&Token::new("double"));

        prim.create_attribute(
            usd_geom_tokens().shutter_close.as_str(),
            &double_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // Exposure
    // ========================================================================

    /// Returns the exposure attribute.
    ///
    /// Matches C++ `GetExposureAttr()`.
    pub fn get_exposure_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().exposure.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the exposure attribute.
    ///
    /// Matches C++ `CreateExposureAttr()`.
    pub fn create_exposure_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let float_type = registry.find_type_by_token(&Token::new("float"));

        prim.create_attribute(
            usd_geom_tokens().exposure.as_str(),
            &float_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // ExposureIso
    // ========================================================================

    /// Returns the exposureIso attribute.
    ///
    /// Matches C++ `GetExposureIsoAttr()`.
    pub fn get_exposure_iso_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().exposure_iso.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the exposureIso attribute.
    ///
    /// Matches C++ `CreateExposureIsoAttr()`.
    pub fn create_exposure_iso_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let float_type = registry.find_type_by_token(&Token::new("float"));

        prim.create_attribute(
            usd_geom_tokens().exposure_iso.as_str(),
            &float_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // ExposureTime
    // ========================================================================

    /// Returns the exposureTime attribute.
    ///
    /// Matches C++ `GetExposureTimeAttr()`.
    pub fn get_exposure_time_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().exposure_time.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the exposureTime attribute.
    ///
    /// Matches C++ `CreateExposureTimeAttr()`.
    pub fn create_exposure_time_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let float_type = registry.find_type_by_token(&Token::new("float"));

        prim.create_attribute(
            usd_geom_tokens().exposure_time.as_str(),
            &float_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // ExposureFStop
    // ========================================================================

    /// Returns the exposureFStop attribute.
    ///
    /// Matches C++ `GetExposureFStopAttr()`.
    pub fn get_exposure_f_stop_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().exposure_f_stop.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the exposureFStop attribute.
    ///
    /// Matches C++ `CreateExposureFStopAttr()`.
    pub fn create_exposure_f_stop_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let float_type = registry.find_type_by_token(&Token::new("float"));

        prim.create_attribute(
            usd_geom_tokens().exposure_f_stop.as_str(),
            &float_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // ExposureResponsivity
    // ========================================================================

    /// Returns the exposureResponsivity attribute.
    ///
    /// Matches C++ `GetExposureResponsivityAttr()`.
    pub fn get_exposure_responsivity_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().exposure_responsivity.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the exposureResponsivity attribute.
    ///
    /// Matches C++ `CreateExposureResponsivityAttr()`.
    pub fn create_exposure_responsivity_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let float_type = registry.find_type_by_token(&Token::new("float"));

        prim.create_attribute(
            usd_geom_tokens().exposure_responsivity.as_str(),
            &float_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // Helper Methods
    // ========================================================================

    /// Helper to convert Token to CameraProjection.
    fn token_to_projection(token: &Token) -> CameraProjection {
        if *token == usd_geom_tokens().orthographic {
            CameraProjection::Orthographic
        } else {
            CameraProjection::Perspective // default
        }
    }

    /// Helper to convert CameraProjection to Token.
    fn projection_to_token(projection: CameraProjection) -> Token {
        match projection {
            CameraProjection::Perspective => usd_geom_tokens().perspective.clone(),
            CameraProjection::Orthographic => usd_geom_tokens().orthographic.clone(),
        }
    }

    /// Helper to convert Vec2f to Range1f.
    fn vec2f_to_range1f(vec: Vec2f) -> Range1f {
        Range1f::new(vec.x, vec.y)
    }

    /// Helper to convert Range1f to Vec2f.
    fn range1f_to_vec2f(range: Range1f) -> Vec2f {
        Vec2f::new(range.min(), range.max())
    }

    /// Creates a Camera object from the attribute values at time.
    ///
    /// Matches C++ `GetCamera(const UsdTimeCode &time)`.
    pub fn get_camera(&self, time: TimeCode) -> GfCamera {
        let mut camera = GfCamera::new();

        // Set transform using Imageable's compute_local_to_world_transform
        let transform = self
            .inner
            .imageable()
            .compute_local_to_world_transform(time);
        camera.set_transform(transform);

        // Set projection (USDA stores as String, USDC as Token)
        if let Some(value) = self.get_projection_attr().get(time) {
            if let Some(token_str) = value.get::<String>() {
                let token = Token::new(token_str);
                camera.set_projection(Self::token_to_projection(&token));
            } else if let Some(token) = value.get::<Token>() {
                camera.set_projection(Self::token_to_projection(token));
            }
        }

        // Set horizontal aperture (schema: float, but try f64 fallback for USDC)
        if let Some(value) = self.get_horizontal_aperture_attr().get(time) {
            if let Some(&v) = value.get::<f32>() {
                camera.set_horizontal_aperture(v);
            } else if let Some(&v) = value.get::<f64>() {
                camera.set_horizontal_aperture(v as f32);
            } else if let Some(&v) = value.get::<i32>() {
                camera.set_horizontal_aperture(v as f32);
            } else if let Some(&v) = value.get::<i64>() {
                camera.set_horizontal_aperture(v as f32);
            }
        }

        // Set vertical aperture
        if let Some(value) = self.get_vertical_aperture_attr().get(time) {
            if let Some(&v) = value.get::<f32>() {
                camera.set_vertical_aperture(v);
            } else if let Some(&v) = value.get::<f64>() {
                camera.set_vertical_aperture(v as f32);
            } else if let Some(&v) = value.get::<i32>() {
                camera.set_vertical_aperture(v as f32);
            } else if let Some(&v) = value.get::<i64>() {
                camera.set_vertical_aperture(v as f32);
            }
        }

        // Set horizontal aperture offset
        if let Some(value) = self.get_horizontal_aperture_offset_attr().get(time) {
            if let Some(&v) = value.get::<f32>() {
                camera.set_horizontal_aperture_offset(v);
            } else if let Some(&v) = value.get::<f64>() {
                camera.set_horizontal_aperture_offset(v as f32);
            } else if let Some(&v) = value.get::<i32>() {
                camera.set_horizontal_aperture_offset(v as f32);
            } else if let Some(&v) = value.get::<i64>() {
                camera.set_horizontal_aperture_offset(v as f32);
            }
        }

        // Set vertical aperture offset
        if let Some(value) = self.get_vertical_aperture_offset_attr().get(time) {
            if let Some(&v) = value.get::<f32>() {
                camera.set_vertical_aperture_offset(v);
            } else if let Some(&v) = value.get::<f64>() {
                camera.set_vertical_aperture_offset(v as f32);
            } else if let Some(&v) = value.get::<i32>() {
                camera.set_vertical_aperture_offset(v as f32);
            } else if let Some(&v) = value.get::<i64>() {
                camera.set_vertical_aperture_offset(v as f32);
            }
        }

        // Set focal length
        if let Some(value) = self.get_focal_length_attr().get(time) {
            if let Some(&v) = value.get::<f32>() {
                camera.set_focal_length(v);
            } else if let Some(&v) = value.get::<f64>() {
                camera.set_focal_length(v as f32);
            } else if let Some(&v) = value.get::<i32>() {
                camera.set_focal_length(v as f32);
            } else if let Some(&v) = value.get::<i64>() {
                camera.set_focal_length(v as f32);
            }
        }

        // Set clipping range (Vec2f or fallback from USDC)
        if let Some(value) = self.get_clipping_range_attr().get(time) {
            if let Some(&vec2f) = value.get::<Vec2f>() {
                camera.set_clipping_range(Self::vec2f_to_range1f(vec2f));
            } else if let Some(vec2f) = value.get::<usd_gf::vec2::Vec2<f32>>() {
                camera.set_clipping_range(Range1f::new(vec2f.x, vec2f.y));
            } else if let Some(v) = value.get::<Vec<f64>>() {
                if v.len() == 2 {
                    camera.set_clipping_range(Range1f::new(v[0] as f32, v[1] as f32));
                }
            } else if let Some(v) = value.get::<Vec<f32>>() {
                if v.len() == 2 {
                    camera.set_clipping_range(Range1f::new(v[0], v[1]));
                }
            }
        }

        // Set clipping planes
        if let Some(value) = self.get_clipping_planes_attr().get(time) {
            if let Some(planes) = value.get::<Vec<Vec4f>>() {
                camera.set_clipping_planes(planes.clone());
            } else if let Some(planes) = value.get::<usd_vt::Array<Vec4f>>() {
                camera.set_clipping_planes(planes.iter().cloned().collect());
            }
        }

        // Set f-stop
        if let Some(value) = self.get_f_stop_attr().get(time) {
            if let Some(&v) = value.get::<f32>() {
                camera.set_f_stop(v);
            } else if let Some(&v) = value.get::<f64>() {
                camera.set_f_stop(v as f32);
            } else if let Some(&v) = value.get::<i32>() {
                camera.set_f_stop(v as f32);
            } else if let Some(&v) = value.get::<i64>() {
                camera.set_f_stop(v as f32);
            }
        }

        // Set focus distance
        if let Some(value) = self.get_focus_distance_attr().get(time) {
            if let Some(&v) = value.get::<f32>() {
                camera.set_focus_distance(v);
            } else if let Some(&v) = value.get::<f64>() {
                camera.set_focus_distance(v as f32);
            } else if let Some(&v) = value.get::<i32>() {
                camera.set_focus_distance(v as f32);
            } else if let Some(&v) = value.get::<i64>() {
                camera.set_focus_distance(v as f32);
            }
        }

        camera
    }

    /// Write attribute values from camera for time.
    ///
    /// Matches C++ `SetFromCamera(const GfCamera &camera, const UsdTimeCode &time)`.
    pub fn set_from_camera(&self, camera: &GfCamera, time: TimeCode) -> bool {
        // Compute cam-local matrix: camera.transform * parentToWorld^-1
        // Ref: `usd-refs/OpenUSD/pxr/usd/usdGeom/camera.cpp` `SetFromCamera`
        // Ref: `usd-refs/OpenUSD/pxr/base/gf/matrix4d.cpp` `GfMatrix4d::GetInverse` (default eps=0;
        //      singular → `SetScale(FLT_MAX)`, not identity)
        let parent_to_world = self
            .inner
            .imageable()
            .compute_parent_to_world_transform(time);
        let parent_to_world_inv = parent_to_world.inverse_with_eps(0.0).unwrap_or_else(|| {
            Matrix4d::from_scale(f32::MAX as f64)
        });
        let cam_matrix = *camera.transform() * parent_to_world_inv;

        // Author the matrix as a single xformOp:transform (clears existing xform ops).
        // If MakeMatrixXform returns invalid the edit target is weaker than existing opinions.
        let xform_op = self.inner.make_matrix_xform();
        if !xform_op.is_valid() {
            return false;
        }
        xform_op.set(usd_vt::Value::from_no_hash(cam_matrix), time);

        // Set projection
        let _ = self.get_projection_attr().set(
            Self::projection_to_token(camera.projection()).as_str(),
            time,
        );

        // Set apertures
        let _ = self
            .get_horizontal_aperture_attr()
            .set(camera.horizontal_aperture(), time);
        let _ = self
            .get_vertical_aperture_attr()
            .set(camera.vertical_aperture(), time);
        let _ = self
            .get_horizontal_aperture_offset_attr()
            .set(camera.horizontal_aperture_offset(), time);
        let _ = self
            .get_vertical_aperture_offset_attr()
            .set(camera.vertical_aperture_offset(), time);

        // Set focal length
        let _ = self
            .get_focal_length_attr()
            .set(camera.focal_length(), time);

        // Set clipping range
        let clipping_range_vec = Self::range1f_to_vec2f(camera.clipping_range());
        let _ = self
            .get_clipping_range_attr()
            .set(Value::from_no_hash(clipping_range_vec), time);

        // Set clipping planes
        let clipping_planes_vec: Vec<Vec4f> = camera.clipping_planes().to_vec();
        let _ = self
            .get_clipping_planes_attr()
            .set(Value::from_no_hash(clipping_planes_vec), time);

        // Set f-stop and focus distance
        let _ = self.get_f_stop_attr().set(camera.f_stop(), time);
        let _ = self
            .get_focus_distance_attr()
            .set(camera.focus_distance(), time);

        true
    }

    // ========================================================================
    // Get Methods (Value Retrieval)
    // ========================================================================

    /// Get the projection at the specified time.
    ///
    /// Matches C++ `GetProjection(TfToken* projection, UsdTimeCode time)`.
    pub fn get_projection(&self, time: TimeCode) -> Option<Token> {
        self.get_projection_attr().get_typed::<Token>(time)
    }

    /// Get the horizontalAperture at the specified time.
    ///
    /// Matches C++ `GetHorizontalAperture(float* horizontalAperture, UsdTimeCode time)`.
    pub fn get_horizontal_aperture(&self, time: TimeCode) -> Option<f32> {
        self.get_horizontal_aperture_attr().get_typed::<f32>(time)
    }

    /// Get the verticalAperture at the specified time.
    ///
    /// Matches C++ `GetVerticalAperture(float* verticalAperture, UsdTimeCode time)`.
    pub fn get_vertical_aperture(&self, time: TimeCode) -> Option<f32> {
        self.get_vertical_aperture_attr().get_typed::<f32>(time)
    }

    /// Get the horizontalApertureOffset at the specified time.
    ///
    /// Matches C++ `GetHorizontalApertureOffset(float* horizontalApertureOffset, UsdTimeCode time)`.
    pub fn get_horizontal_aperture_offset(&self, time: TimeCode) -> Option<f32> {
        self.get_horizontal_aperture_offset_attr()
            .get_typed::<f32>(time)
    }

    /// Get the verticalApertureOffset at the specified time.
    ///
    /// Matches C++ `GetVerticalApertureOffset(float* verticalApertureOffset, UsdTimeCode time)`.
    pub fn get_vertical_aperture_offset(&self, time: TimeCode) -> Option<f32> {
        self.get_vertical_aperture_offset_attr()
            .get_typed::<f32>(time)
    }

    /// Get the focalLength at the specified time.
    ///
    /// Matches C++ `GetFocalLength(float* focalLength, UsdTimeCode time)`.
    pub fn get_focal_length(&self, time: TimeCode) -> Option<f32> {
        self.get_focal_length_attr().get_typed::<f32>(time)
    }

    /// Get the clippingRange at the specified time.
    ///
    /// Matches C++ `GetClippingRange(GfVec2f* clippingRange, UsdTimeCode time)`.
    pub fn get_clipping_range(&self, time: TimeCode) -> Option<Vec2f> {
        self.get_clipping_range_attr().get_typed::<Vec2f>(time)
    }

    /// Get the clippingPlanes at the specified time.
    ///
    /// Matches C++ `GetClippingPlanes(VtVec4fArray* clippingPlanes, UsdTimeCode time)`.
    pub fn get_clipping_planes(&self, time: TimeCode) -> Option<usd_vt::Array<Vec4f>> {
        self.get_clipping_planes_attr()
            .get_typed::<usd_vt::Array<Vec4f>>(time)
    }

    /// Get the fStop at the specified time.
    ///
    /// Matches C++ `GetFStop(float* fStop, UsdTimeCode time)`.
    pub fn get_f_stop(&self, time: TimeCode) -> Option<f32> {
        self.get_f_stop_attr().get_typed::<f32>(time)
    }

    /// Get the focusDistance at the specified time.
    ///
    /// Matches C++ `GetFocusDistance(float* focusDistance, UsdTimeCode time)`.
    pub fn get_focus_distance(&self, time: TimeCode) -> Option<f32> {
        self.get_focus_distance_attr().get_typed::<f32>(time)
    }

    /// Get the stereoRole at the specified time.
    ///
    /// Matches C++ `GetStereoRole(TfToken* stereoRole, UsdTimeCode time)`.
    pub fn get_stereo_role(&self, time: TimeCode) -> Option<Token> {
        self.get_stereo_role_attr().get_typed::<Token>(time)
    }

    /// Get the shutterOpen at the specified time.
    ///
    /// Matches C++ `GetShutterOpen(double* shutterOpen, UsdTimeCode time)`.
    pub fn get_shutter_open(&self, time: TimeCode) -> Option<f64> {
        self.get_shutter_open_attr().get_typed::<f64>(time)
    }

    /// Get the shutterClose at the specified time.
    ///
    /// Matches C++ `GetShutterClose(double* shutterClose, UsdTimeCode time)`.
    pub fn get_shutter_close(&self, time: TimeCode) -> Option<f64> {
        self.get_shutter_close_attr().get_typed::<f64>(time)
    }

    /// Get the exposure at the specified time.
    ///
    /// Matches C++ `GetExposure(float* exposure, UsdTimeCode time)`.
    pub fn get_exposure(&self, time: TimeCode) -> Option<f32> {
        self.get_exposure_attr().get_typed::<f32>(time)
    }

    // ========================================================================
    // Compute Matrix Methods
    // ========================================================================

    /// Compute the projection matrix for the camera at the specified time.
    ///
    /// Matches C++ `ComputeProjectionMatrix(GfMatrix4d* projectionMatrix, UsdTimeCode time)`.
    pub fn compute_projection_matrix(&self, time: TimeCode) -> Option<Matrix4d> {
        let camera = self.get_camera(time);
        let frustum = camera.frustum();
        Some(frustum.compute_projection_matrix())
    }

    /// Compute the local-to-world matrix for the camera at the specified time.
    ///
    /// Matches C++ `ComputeLocalToWorldMatrix(GfMatrix4d* localToWorldMatrix, UsdTimeCode time)`.
    pub fn compute_local_to_world_matrix(&self, time: TimeCode) -> Option<Matrix4d> {
        // Use Imageable's compute_local_to_world_transform
        Some(
            self.inner
                .imageable()
                .compute_local_to_world_transform(time),
        )
    }

    /// Compute the view matrix for the camera at the specified time.
    ///
    /// Matches C++ `ComputeViewMatrix(GfMatrix4d* viewMatrix, UsdTimeCode time)`.
    pub fn compute_view_matrix(&self, time: TimeCode) -> Option<Matrix4d> {
        let camera = self.get_camera(time);
        let frustum = camera.frustum();
        Some(frustum.compute_view_matrix())
    }

    /// Compute the world-to-local matrix for the camera at the specified time.
    ///
    /// Matches C++ `ComputeWorldToLocalMatrix(GfMatrix4d* worldToLocalMatrix, UsdTimeCode time)`.
    pub fn compute_world_to_local_matrix(&self, time: TimeCode) -> Option<Matrix4d> {
        // Compute as inverse of local-to-world
        let local_to_world = self.compute_local_to_world_matrix(time)?;
        local_to_world.inverse()
    }

    /// Computes the linear exposure scale.
    ///
    /// Ref: `usd-refs/OpenUSD/pxr/usd/usdGeom/camera.cpp` `ComputeLinearExposureScale`
    fn exposure_scalar_at(attr: &Attribute, time: TimeCode, default: f32) -> f32 {
        let Some(value) = attr.get(time) else {
            return default;
        };
        if let Some(&v) = value.get::<f32>() {
            return v;
        }
        if let Some(&v) = value.get::<f64>() {
            return v as f32;
        }
        default
    }

    pub fn compute_linear_exposure_scale(&self, time: TimeCode) -> f32 {
        let exposure_time = Self::exposure_scalar_at(&self.get_exposure_time_attr(), time, 1.0);
        let exposure_iso = Self::exposure_scalar_at(&self.get_exposure_iso_attr(), time, 100.0);
        let exposure_f_stop = Self::exposure_scalar_at(&self.get_exposure_f_stop_attr(), time, 1.0);
        let exposure_responsivity =
            Self::exposure_scalar_at(&self.get_exposure_responsivity_attr(), time, 1.0);
        let exposure_exponent = Self::exposure_scalar_at(&self.get_exposure_attr(), time, 0.0);

        (exposure_time * exposure_iso * 2.0f32.powf(exposure_exponent) * exposure_responsivity)
            / (100.0f32 * exposure_f_stop * exposure_f_stop)
    }

    /// Return a vector of names of all pre-declared attributes for this schema
    /// class and all its ancestor classes.
    ///
    /// Matches C++ `GetSchemaAttributeNames(bool includeInherited)`.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        let local_names = vec![
            usd_geom_tokens().projection.clone(),
            usd_geom_tokens().horizontal_aperture.clone(),
            usd_geom_tokens().vertical_aperture.clone(),
            usd_geom_tokens().horizontal_aperture_offset.clone(),
            usd_geom_tokens().vertical_aperture_offset.clone(),
            usd_geom_tokens().focal_length.clone(),
            usd_geom_tokens().clipping_range.clone(),
            usd_geom_tokens().clipping_planes.clone(),
            usd_geom_tokens().f_stop.clone(),
            usd_geom_tokens().focus_distance.clone(),
            usd_geom_tokens().stereo_role.clone(),
            usd_geom_tokens().shutter_open.clone(),
            usd_geom_tokens().shutter_close.clone(),
            usd_geom_tokens().exposure.clone(),
            usd_geom_tokens().exposure_iso.clone(),
            usd_geom_tokens().exposure_time.clone(),
            usd_geom_tokens().exposure_f_stop.clone(),
            usd_geom_tokens().exposure_responsivity.clone(),
        ];

        if include_inherited {
            let mut all_names = Xformable::get_schema_attribute_names(true);
            all_names.extend(local_names);
            all_names
        } else {
            local_names
        }
    }
}

impl PartialEq for Camera {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for Camera {}
