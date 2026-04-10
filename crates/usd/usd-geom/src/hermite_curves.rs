//! UsdGeomHermiteCurves - hermite curves geometry schema.
//!
//! Port of pxr/usd/usdGeom/hermiteCurves.h/cpp
//!
//! This schema specifies a cubic hermite interpolated curve batch.

use super::curves::Curves;
use super::tokens::usd_geom_tokens;
use crate::schema_create_default::apply_optional_default;
use usd_core::attribute::Variability;
use usd_core::{Attribute, Prim, Stage};
use usd_gf::vec3::Vec3f;
use usd_sdf::ValueTypeRegistry;
use usd_tf::Token;
use usd_vt::Value;

// ============================================================================
// HermiteCurves
// ============================================================================

/// Hermite curves geometry schema.
///
/// This schema specifies a cubic hermite interpolated curve batch as
/// sometimes used for defining guides for animation.
///
/// Matches C++ `UsdGeomHermiteCurves`.
#[derive(Debug, Clone)]
pub struct HermiteCurves {
    /// Base curves schema.
    inner: Curves,
}

impl HermiteCurves {
    /// Creates a HermiteCurves schema from a prim.
    ///
    /// Matches C++ `UsdGeomHermiteCurves(const UsdPrim& prim)`.
    pub fn new(prim: Prim) -> Self {
        Self {
            inner: Curves::new(prim),
        }
    }

    /// Creates a HermiteCurves schema from a Curves schema.
    ///
    /// Matches C++ `UsdGeomHermiteCurves(const UsdSchemaBase& schemaObj)`.
    pub fn from_curves(curves: Curves) -> Self {
        Self { inner: curves }
    }

    /// Creates an invalid HermiteCurves schema.
    pub fn invalid() -> Self {
        Self {
            inner: Curves::invalid(),
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

    /// Returns the curves base.
    pub fn curves(&self) -> &Curves {
        &self.inner
    }

    /// Returns the schema type name.
    pub fn schema_type_name() -> Token {
        Token::new("HermiteCurves")
    }

    /// Return a HermiteCurves holding the prim adhering to this schema at path on stage.
    ///
    /// Matches C++ `UsdGeomHermiteCurves::Get(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn get(stage: &Stage, path: &usd_sdf::Path) -> Self {
        if let Some(prim) = stage.get_prim_at_path(path) {
            Self::new(prim)
        } else {
            Self::invalid()
        }
    }

    /// Attempt to ensure a prim adhering to this schema at path is defined on this stage.
    ///
    /// Matches C++ `UsdGeomHermiteCurves::Define(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn define(stage: &Stage, path: &usd_sdf::Path) -> Self {
        match stage.define_prim(path.get_string(), Self::schema_type_name().as_str()) {
            Ok(prim) => Self::new(prim),
            Err(_) => Self::invalid(),
        }
    }

    // ========================================================================
    // Tangents
    // ========================================================================

    /// Returns the tangents attribute.
    ///
    /// Defines the outgoing trajectory tangent for each point.
    /// Tangents should be the same size as the points attribute.
    ///
    /// Matches C++ `GetTangentsAttr()`.
    pub fn get_tangents_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().tangents.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the tangents attribute.
    ///
    /// Matches C++ `CreateTangentsAttr()`.
    pub fn create_tangents_attr(
        &self,
        default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().tangents.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().tangents.as_str())
                .unwrap_or_else(Attribute::invalid);
        }

        let registry = ValueTypeRegistry::instance();
        let vector3f_array_type = registry.find_type_by_token(&Token::new("vector3f[]"));

        let attr = prim
            .create_attribute(
                usd_geom_tokens().tangents.as_str(),
                &vector3f_array_type,
                false,                      // not custom
                Some(Variability::Varying), // can vary over time
            )
            .unwrap_or_else(Attribute::invalid);
        apply_optional_default(attr, default_value)
    }

    // ========================================================================
    // PointWeights
    // ========================================================================

    /// Returns the pointWeights attribute.
    ///
    /// Optional weighting for each point.
    ///
    /// Matches C++ `GetPointWeightsAttr()`.
    pub fn get_point_weights_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().point_weights.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the pointWeights attribute.
    ///
    /// Matches C++ `CreatePointWeightsAttr()`.
    pub fn create_point_weights_attr(
        &self,
        default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().point_weights.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().point_weights.as_str())
                .unwrap_or_else(Attribute::invalid);
        }

        let registry = ValueTypeRegistry::instance();
        let float_array_type = registry.find_type_by_token(&Token::new("float[]"));

        let attr = prim
            .create_attribute(
                usd_geom_tokens().point_weights.as_str(),
                &float_array_type,
                false,                      // not custom
                Some(Variability::Varying), // can vary over time
            )
            .unwrap_or_else(Attribute::invalid);
        apply_optional_default(attr, default_value)
    }

    // ========================================================================
    // Get Methods (Value Retrieval)
    // ========================================================================

    /// Get the tangents at the specified time.
    ///
    /// Matches C++ `GetTangents(VtVec3fArray* tangents, UsdTimeCode time)`.
    pub fn get_tangents(&self, time: usd_sdf::TimeCode) -> Option<usd_vt::Array<Vec3f>> {
        self.get_tangents_attr()
            .get_typed::<usd_vt::Array<Vec3f>>(time)
    }

    /// Get the point weights at the specified time.
    ///
    /// Matches C++ `GetPointWeights(VtFloatArray* weights, UsdTimeCode time)`.
    pub fn get_point_weights(&self, time: usd_sdf::TimeCode) -> Option<usd_vt::Array<f32>> {
        self.get_point_weights_attr()
            .get_typed::<usd_vt::Array<f32>>(time)
    }

    // ========================================================================
    // Compute Extent At Time
    // ========================================================================

    /// Compute the extent for the hermite curves at the specified time.
    ///
    /// Returns true on success, false if extent was unable to be calculated.
    /// On success, extent will contain the axis-aligned bounding box of the curves.
    ///
    /// Matches C++ `ComputeExtentAtTime(VtVec3fArray* extent, UsdTimeCode time, UsdTimeCode baseTime)`.
    pub fn compute_extent_at_time(
        &self,
        extent: &mut Vec<Vec3f>,
        time: usd_sdf::TimeCode,
        base_time: usd_sdf::TimeCode,
    ) -> bool {
        // Use Curves base implementation
        self.inner.compute_extent_at_time(extent, time, base_time)
    }

    /// Compute the extent for the hermite curves at the specified time with transform.
    ///
    /// Returns true on success, false if extent was unable to be calculated.
    ///
    /// Matches C++ `ComputeExtentAtTime(VtVec3fArray* extent, UsdTimeCode time, UsdTimeCode baseTime, const GfMatrix4d& transform)`.
    pub fn compute_extent_at_time_with_transform(
        &self,
        extent: &mut Vec<Vec3f>,
        time: usd_sdf::TimeCode,
        base_time: usd_sdf::TimeCode,
        transform: &usd_gf::matrix4::Matrix4d,
    ) -> bool {
        // Use Curves base implementation
        self.inner
            .compute_extent_at_time_with_transform(extent, time, base_time, transform)
    }

    /// Compute the extent for the hermite curves at multiple times.
    ///
    /// Returns true on success, false if extent was unable to be calculated.
    ///
    /// Matches C++ `ComputeExtentAtTimes(std::vector<VtVec3fArray>* extents, const std::vector<UsdTimeCode>& times, UsdTimeCode baseTime)`.
    pub fn compute_extent_at_times(
        &self,
        extents: &mut Vec<Vec<Vec3f>>,
        times: &[usd_sdf::TimeCode],
        base_time: usd_sdf::TimeCode,
    ) -> bool {
        // Use Curves base implementation
        self.inner
            .compute_extent_at_times(extents, times, base_time)
    }

    /// Compute the extent for the hermite curves at multiple times with transform.
    ///
    /// Returns true on success, false if extent was unable to be calculated.
    ///
    /// Matches C++ `ComputeExtentAtTimes(std::vector<VtVec3fArray>* extents, const std::vector<UsdTimeCode>& times, UsdTimeCode baseTime, const GfMatrix4d& transform)`.
    pub fn compute_extent_at_times_with_transform(
        &self,
        extents: &mut Vec<Vec<Vec3f>>,
        times: &[usd_sdf::TimeCode],
        base_time: usd_sdf::TimeCode,
        transform: &usd_gf::matrix4::Matrix4d,
    ) -> bool {
        // Use Curves base implementation
        self.inner
            .compute_extent_at_times_with_transform(extents, times, base_time, transform)
    }

    /// Return a vector of names of all pre-declared attributes for this schema
    /// class and all its ancestor classes.
    ///
    /// Matches C++ `GetSchemaAttributeNames(bool includeInherited)`.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        let local_names = vec![
            usd_geom_tokens().tangents.clone(),
            usd_geom_tokens().point_weights.clone(),
        ];

        if include_inherited {
            let mut all_names = Curves::get_schema_attribute_names(true);
            all_names.extend(local_names);
            all_names
        } else {
            local_names
        }
    }
}

// ============================================================================
// PointAndTangentArrays
// ============================================================================

/// Separated points and tangents arrays for HermiteCurves.
///
/// Provides utilities for converting between interleaved (P0,T0,...,Pn,Tn) and
/// separated (P0,...,Pn) + (T0,...,Tn) representations.
///
/// Matches C++ `UsdGeomHermiteCurves::PointAndTangentArrays`.
#[derive(Debug, Clone, PartialEq)]
pub struct PointAndTangentArrays {
    /// Separated points array.
    points: Vec<Vec3f>,
    /// Separated tangents array.
    tangents: Vec<Vec3f>,
}

impl PointAndTangentArrays {
    /// Construct empty arrays.
    pub fn new() -> Self {
        Self {
            points: Vec::new(),
            tangents: Vec::new(),
        }
    }

    /// Construct from separate points and tangents arrays.
    ///
    /// If the arrays are not the same size, returns an empty container.
    pub fn from_points_and_tangents(points: Vec<Vec3f>, tangents: Vec<Vec3f>) -> Self {
        if points.len() != tangents.len() {
            log::error!("Points and tangents must be the same size.");
            return Self::new();
        }
        Self { points, tangents }
    }

    /// Separate an interleaved array (P0,T0,...,Pn,Tn) into points and tangents.
    ///
    /// Matches C++ `Separate()`.
    pub fn separate(interleaved: &[Vec3f]) -> Self {
        if interleaved.len() % 2 != 0 {
            log::error!("Interleaved array must have an even number of elements.");
            return Self::new();
        }

        let count = interleaved.len() / 2;
        let mut points = Vec::with_capacity(count);
        let mut tangents = Vec::with_capacity(count);

        for i in 0..count {
            points.push(interleaved[i * 2]);
            tangents.push(interleaved[i * 2 + 1]);
        }

        Self { points, tangents }
    }

    /// Interleave points and tangents into a single array (P0,T0,...,Pn,Tn).
    ///
    /// Matches C++ `Interleave()`.
    pub fn interleave(&self) -> Vec<Vec3f> {
        let mut result = Vec::with_capacity(self.points.len() * 2);
        for i in 0..self.points.len() {
            result.push(self.points[i]);
            result.push(self.tangents[i]);
        }
        result
    }

    /// Returns true if the container is empty.
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Returns the number of point/tangent pairs.
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Get the separated points array.
    pub fn get_points(&self) -> &[Vec3f] {
        &self.points
    }

    /// Get the separated tangents array.
    pub fn get_tangents(&self) -> &[Vec3f] {
        &self.tangents
    }
}

impl Default for PointAndTangentArrays {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for HermiteCurves {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for HermiteCurves {}
