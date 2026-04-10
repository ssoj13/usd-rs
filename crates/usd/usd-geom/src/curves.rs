//! UsdGeomCurves - base class for curve geometry schemas.
//!
//! Port of pxr/usd/usdGeom/curves.h/cpp
//!
//! Base class for UsdGeomBasisCurves, UsdGeomNurbsCurves, and UsdGeomHermiteCurves.

use super::point_based::PointBased;
use super::points::widths_vec_from_usd_value;
use super::tokens::usd_geom_tokens;
use crate::schema_create_default::apply_optional_default;
use usd_core::attribute::Variability;
use usd_core::{Attribute, Prim, Stage};
use usd_gf::matrix4::Matrix4d;
use usd_gf::vec3::Vec3f;
use usd_sdf::{TimeCode, ValueTypeRegistry};
use usd_tf::Token;
use usd_vt::Value;

// ============================================================================
// Curves
// ============================================================================

/// Base class for curve geometry schemas.
///
/// Base class for UsdGeomBasisCurves, UsdGeomNurbsCurves, and UsdGeomHermiteCurves.
///
/// Matches C++ `UsdGeomCurves`.
#[derive(Debug, Clone)]
pub struct Curves {
    /// Base point-based schema.
    inner: PointBased,
}

impl Curves {
    /// Creates a Curves schema from a prim.
    ///
    /// Matches C++ `UsdGeomCurves(const UsdPrim& prim)`.
    pub fn new(prim: Prim) -> Self {
        Self {
            inner: PointBased::new(prim),
        }
    }

    /// Creates a Curves schema from a PointBased schema.
    ///
    /// Matches C++ `UsdGeomCurves(const UsdSchemaBase& schemaObj)`.
    pub fn from_point_based(point_based: PointBased) -> Self {
        Self { inner: point_based }
    }

    /// Creates an invalid Curves schema.
    pub fn invalid() -> Self {
        Self {
            inner: PointBased::invalid(),
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

    /// Returns the point-based base.
    pub fn point_based(&self) -> &PointBased {
        &self.inner
    }

    /// Returns the schema type name.
    pub fn schema_type_name() -> Token {
        Token::new("Curves")
    }

    /// Return a Curves holding the prim adhering to this schema at path on stage.
    ///
    /// Matches C++ `UsdGeomCurves::Get(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn get(stage: &Stage, path: &usd_sdf::Path) -> Self {
        if let Some(prim) = stage.get_prim_at_path(path) {
            Self::new(prim)
        } else {
            Self::invalid()
        }
    }

    // ========================================================================
    // CurveVertexCounts
    // ========================================================================

    /// Returns the curveVertexCounts attribute.
    ///
    /// Curves-derived primitives can represent multiple distinct,
    /// potentially disconnected curves. The length of 'curveVertexCounts'
    /// gives the number of such curves, and each element describes the
    /// number of vertices in the corresponding curve.
    ///
    /// Matches C++ `GetCurveVertexCountsAttr()`.
    pub fn get_curve_vertex_counts_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().curve_vertex_counts.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the curveVertexCounts attribute.
    ///
    /// Matches C++ `CreateCurveVertexCountsAttr()`.
    pub fn create_curve_vertex_counts_attr(
        &self,
        default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().curve_vertex_counts.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().curve_vertex_counts.as_str())
                .unwrap_or_else(Attribute::invalid);
        }

        let registry = ValueTypeRegistry::instance();
        let int_array_type = registry.find_type_by_token(&Token::new("int[]"));

        let attr = prim
            .create_attribute(
                usd_geom_tokens().curve_vertex_counts.as_str(),
                &int_array_type,
                false,                      // not custom
                Some(Variability::Varying), // can vary over time
            )
            .unwrap_or_else(Attribute::invalid);
        apply_optional_default(attr, default_value)
    }

    // ========================================================================
    // Widths
    // ========================================================================

    /// Returns the widths attribute.
    ///
    /// Provides width specification for the curves, whose application
    /// will depend on whether the curve is oriented (normals are defined for
    /// it), in which case widths are "ribbon width", or unoriented, in which
    /// case widths are cylinder width.
    ///
    /// Matches C++ `GetWidthsAttr()`.
    pub fn get_widths_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().widths.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the widths attribute.
    ///
    /// Matches C++ `CreateWidthsAttr()`.
    pub fn create_widths_attr(
        &self,
        default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        let attr = if prim.has_authored_attribute(usd_geom_tokens().widths.as_str()) {
            prim.get_attribute(usd_geom_tokens().widths.as_str())
                .unwrap_or_else(Attribute::invalid)
        } else {
            let registry = ValueTypeRegistry::instance();
            let float_array_type = registry.find_type_by_token(&Token::new("float[]"));
            prim.create_attribute(
                usd_geom_tokens().widths.as_str(),
                &float_array_type,
                false,                      // not custom
                Some(Variability::Varying), // can vary over time
            )
            .unwrap_or_else(Attribute::invalid)
        };

        if attr.is_valid() {
            if let Some(v) = default_value {
                let _ = attr.set(v, TimeCode::default());
            }
        }
        attr
    }

    /// Get the interpolation for the widths attribute.
    ///
    /// The fallback interpolation, if left unspecified, is "vertex".
    ///
    /// Matches C++ `GetWidthsInterpolation()`.
    pub fn get_widths_interpolation(&self) -> Token {
        let widths_attr = self.get_widths_attr();
        if !widths_attr.is_valid() {
            return usd_geom_tokens().vertex.clone();
        }

        // Check for interpolation metadata
        if let Some(interp_value) = widths_attr.get_metadata(&usd_geom_tokens().interpolation) {
            // Try to extract token from value
            if let Some(interp_str) = interp_value.get::<String>() {
                return Token::new(interp_str);
            } else if let Some(interp_token) = interp_value.get::<Token>() {
                return interp_token.clone();
            }
        }

        usd_geom_tokens().vertex.clone()
    }

    /// Set the interpolation for the widths attribute.
    ///
    /// Returns true upon success, false if interpolation is not a legal value.
    ///
    /// Matches C++ `SetWidthsInterpolation()`.
    pub fn set_widths_interpolation(&self, interpolation: &Token) -> bool {
        // Validate interpolation - valid values are: constant, uniform, varying, vertex, faceVarying
        let interp_str = interpolation.as_str();
        if interp_str != usd_geom_tokens().constant.as_str()
            && interp_str != usd_geom_tokens().uniform.as_str()
            && interp_str != usd_geom_tokens().varying.as_str()
            && interp_str != usd_geom_tokens().vertex.as_str()
            && interp_str != usd_geom_tokens().face_varying.as_str()
        {
            return false;
        }

        let widths_attr = self.get_widths_attr();
        if !widths_attr.is_valid() {
            return false;
        }

        widths_attr.set_metadata(
            &usd_geom_tokens().interpolation,
            interpolation.as_str().to_string(),
        )
    }

    // ========================================================================
    // Compute Extent
    // ========================================================================

    /// Compute the extent for the curves defined by points and widths.
    ///
    /// Returns true upon success, false if unable to calculate extent.
    ///
    /// On success, extent will contain an approximate axis-aligned bounding
    /// box of the curve defined by points with the given widths.
    ///
    /// Matches C++ `ComputeExtent(const VtVec3fArray& points, const VtFloatArray& widths, VtVec3fArray* extent)`.
    pub fn compute_extent(points: &[Vec3f], widths: &[f32], extent: &mut [Vec3f; 2]) -> bool {
        // We know nothing about the curve basis. Compute the extent as if it were
        // a point cloud with some max width (convex hull).
        let max_width = if !widths.is_empty() {
            widths.iter().fold(0.0f32, |a, &b| a.max(b))
        } else {
            0.0
        };

        // Use PointBased::compute_extent as base
        if !PointBased::compute_extent(points, extent) {
            return false;
        }

        // Expand extent by max width in all directions
        let half_width = max_width * 0.5;
        extent[0].x -= half_width;
        extent[0].y -= half_width;
        extent[0].z -= half_width;
        extent[1].x += half_width;
        extent[1].y += half_width;
        extent[1].z += half_width;

        true
    }

    /// Compute the extent as if the matrix transform was first applied.
    ///
    /// Matches C++ `ComputeExtent(const VtVec3fArray& points, const VtFloatArray& widths, const GfMatrix4d& transform, VtVec3fArray* extent)`.
    pub fn compute_extent_with_transform(
        points: &[Vec3f],
        widths: &[f32],
        transform: &Matrix4d,
        extent: &mut [Vec3f; 2],
    ) -> bool {
        // We know nothing about the curve basis. Compute the extent as if it were
        // a point cloud with some max width (convex hull).
        let max_width = if !widths.is_empty() {
            widths.iter().fold(0.0f32, |a, &b| a.max(b))
        } else {
            0.0
        };

        // Use PointBased::compute_extent_with_transform as base
        if !PointBased::compute_extent_with_transform(points, transform, extent) {
            return false;
        }

        // Per-axis row lengths matching C++ axisLengths approach
        let row0_len = ((transform[0][0] * transform[0][0]
            + transform[0][1] * transform[0][1]
            + transform[0][2] * transform[0][2])
            .sqrt()) as f32;
        let row1_len = ((transform[1][0] * transform[1][0]
            + transform[1][1] * transform[1][1]
            + transform[1][2] * transform[1][2])
            .sqrt()) as f32;
        let row2_len = ((transform[2][0] * transform[2][0]
            + transform[2][1] * transform[2][1]
            + transform[2][2] * transform[2][2])
            .sqrt()) as f32;
        let half_width = max_width * 0.5;
        extent[0].x -= half_width * row0_len;
        extent[0].y -= half_width * row1_len;
        extent[0].z -= half_width * row2_len;
        extent[1].x += half_width * row0_len;
        extent[1].y += half_width * row1_len;
        extent[1].z += half_width * row2_len;

        true
    }

    // ========================================================================
    // Get Methods (Value Retrieval)
    // ========================================================================

    /// Get the curve vertex counts at the specified time.
    ///
    /// Matches C++ `GetCurveVertexCounts(VtIntArray* counts, UsdTimeCode time)`.
    pub fn get_curve_vertex_counts(&self, time: TimeCode) -> Option<usd_vt::Array<i32>> {
        self.get_curve_vertex_counts_attr()
            .get_typed::<usd_vt::Array<i32>>(time)
    }

    /// Get the widths at the specified time.
    ///
    /// Matches C++ `GetWidths(VtFloatArray* widths, UsdTimeCode time)`.
    pub fn get_widths(&self, time: TimeCode) -> Option<usd_vt::Array<f32>> {
        self.get_widths_attr().get_typed::<usd_vt::Array<f32>>(time)
    }

    // ========================================================================
    // Compute Extent At Time
    // ========================================================================

    /// Compute the extent for the curves at the specified time.
    ///
    /// Returns true on success, false if extent was unable to be calculated.
    /// On success, extent will contain the axis-aligned bounding box of the curves.
    ///
    /// Matches C++ `ComputeExtentAtTime(VtVec3fArray* extent, UsdTimeCode time, UsdTimeCode baseTime)`.
    pub fn compute_extent_at_time(
        &self,
        extent: &mut Vec<Vec3f>,
        time: TimeCode,
        base_time: TimeCode,
    ) -> bool {
        // Get points at the specified time
        let mut points = Vec::new();
        if !self
            .inner
            .compute_points_at_time(&mut points, time, base_time)
        {
            return false;
        }

        if points.is_empty() {
            return false;
        }

        // Get widths if available (same breadth as Points / vt authorship: int arrays, scalars, etc.)
        let widths_attr = self.get_widths_attr();
        let widths: Vec<f32> = if widths_attr.is_valid() {
            widths_attr
                .get(time)
                .map(|widths_value| widths_vec_from_usd_value(&widths_value))
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        // Compute extent
        let mut extent_array = [Vec3f::new(0.0, 0.0, 0.0), Vec3f::new(0.0, 0.0, 0.0)];
        if !Self::compute_extent(&points, &widths, &mut extent_array) {
            return false;
        }

        extent.clear();
        extent.push(extent_array[0]);
        extent.push(extent_array[1]);
        true
    }

    /// Compute the extent for the curves at the specified time with transform.
    ///
    /// Returns true on success, false if extent was unable to be calculated.
    ///
    /// Matches C++ `ComputeExtentAtTime(VtVec3fArray* extent, UsdTimeCode time, UsdTimeCode baseTime, const GfMatrix4d& transform)`.
    pub fn compute_extent_at_time_with_transform(
        &self,
        extent: &mut Vec<Vec3f>,
        time: TimeCode,
        base_time: TimeCode,
        transform: &usd_gf::matrix4::Matrix4d,
    ) -> bool {
        // Get points at the specified time
        let mut points = Vec::new();
        if !self
            .inner
            .compute_points_at_time(&mut points, time, base_time)
        {
            return false;
        }

        if points.is_empty() {
            return false;
        }

        // Get widths if available (same breadth as Points / vt authorship: int arrays, scalars, etc.)
        let widths_attr = self.get_widths_attr();
        let widths: Vec<f32> = if widths_attr.is_valid() {
            widths_attr
                .get(time)
                .map(|widths_value| widths_vec_from_usd_value(&widths_value))
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        // Compute extent with transform
        let mut extent_array = [Vec3f::new(0.0, 0.0, 0.0), Vec3f::new(0.0, 0.0, 0.0)];
        if !Self::compute_extent_with_transform(&points, &widths, transform, &mut extent_array) {
            return false;
        }

        extent.clear();
        extent.push(extent_array[0]);
        extent.push(extent_array[1]);
        true
    }

    /// Compute the extent for the curves at multiple times.
    ///
    /// Returns true on success, false if extent was unable to be calculated.
    ///
    /// Matches C++ `ComputeExtentAtTimes(std::vector<VtVec3fArray>* extents, const std::vector<UsdTimeCode>& times, UsdTimeCode baseTime)`.
    pub fn compute_extent_at_times(
        &self,
        extents: &mut Vec<Vec<Vec3f>>,
        times: &[TimeCode],
        base_time: TimeCode,
    ) -> bool {
        let num_samples = times.len();
        extents.clear();
        extents.reserve(num_samples);

        for &time in times {
            let mut extent = Vec::new();
            if !self.compute_extent_at_time(&mut extent, time, base_time) {
                return false;
            }
            extents.push(extent);
        }

        true
    }

    /// Compute the extent for the curves at multiple times with transform.
    ///
    /// Returns true on success, false if extent was unable to be calculated.
    ///
    /// Matches C++ `ComputeExtentAtTimes(std::vector<VtVec3fArray>* extents, const std::vector<UsdTimeCode>& times, UsdTimeCode baseTime, const GfMatrix4d& transform)`.
    pub fn compute_extent_at_times_with_transform(
        &self,
        extents: &mut Vec<Vec<Vec3f>>,
        times: &[TimeCode],
        base_time: TimeCode,
        transform: &usd_gf::matrix4::Matrix4d,
    ) -> bool {
        let num_samples = times.len();
        extents.clear();
        extents.reserve(num_samples);

        for &time in times {
            let mut extent = Vec::new();
            if !self.compute_extent_at_time_with_transform(&mut extent, time, base_time, transform)
            {
                return false;
            }
            extents.push(extent);
        }

        true
    }

    /// Returns the number of curves as defined by the size of the
    /// curveVertexCounts array at time_code.
    ///
    /// Matches C++ `GetCurveCount()`.
    pub fn get_curve_count(&self, time_code: TimeCode) -> usize {
        let vertex_counts_attr = self.get_curve_vertex_counts_attr();
        if !vertex_counts_attr.is_valid() {
            return 0;
        }

        if let Some(value) = vertex_counts_attr.get(time_code) {
            if let Some(counts) = value.get::<Vec<i32>>() {
                return counts.len();
            } else if let Some(counts) = value.get::<usd_vt::Array<i32>>() {
                return counts.len();
            }
        }
        0
    }

    /// Return a vector of names of all pre-declared attributes for this schema
    /// class and all its ancestor classes.
    ///
    /// Matches C++ `GetSchemaAttributeNames(bool includeInherited)`.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        let local_names = vec![
            usd_geom_tokens().curve_vertex_counts.clone(),
            usd_geom_tokens().widths.clone(),
        ];

        if include_inherited {
            let mut all_names = PointBased::get_schema_attribute_names(true);
            all_names.extend(local_names);
            all_names
        } else {
            local_names
        }
    }
}

impl PartialEq for Curves {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for Curves {}
