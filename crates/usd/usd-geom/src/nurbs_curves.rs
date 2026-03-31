//! UsdGeomNurbsCurves - NURBS curves geometry schema.
//!
//! Port of pxr/usd/usdGeom/nurbsCurves.h/cpp
//!
//! This schema is analagous to NURBS Curves in packages like Maya and Houdini.

use super::curves::Curves;
use super::tokens::usd_geom_tokens;
use usd_core::attribute::Variability;
use usd_core::{Attribute, Prim, Stage};
use usd_sdf::ValueTypeRegistry;
use usd_tf::Token;
use usd_vt::Value;

// ============================================================================
// NurbsCurves
// ============================================================================

/// NURBS curves geometry schema.
///
/// This schema is analagous to NURBS Curves in packages like Maya and Houdini.
///
/// Matches C++ `UsdGeomNurbsCurves`.
#[derive(Debug, Clone)]
pub struct NurbsCurves {
    /// Base curves schema.
    inner: Curves,
}

impl NurbsCurves {
    /// Creates a NurbsCurves schema from a prim.
    ///
    /// Matches C++ `UsdGeomNurbsCurves(const UsdPrim& prim)`.
    pub fn new(prim: Prim) -> Self {
        Self {
            inner: Curves::new(prim),
        }
    }

    /// Creates a NurbsCurves schema from a Curves schema.
    ///
    /// Matches C++ `UsdGeomNurbsCurves(const UsdSchemaBase& schemaObj)`.
    pub fn from_curves(curves: Curves) -> Self {
        Self { inner: curves }
    }

    /// Creates an invalid NurbsCurves schema.
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
        Token::new("NurbsCurves")
    }

    /// Return a NurbsCurves holding the prim adhering to this schema at path on stage.
    ///
    /// Matches C++ `UsdGeomNurbsCurves::Get(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn get(stage: &Stage, path: &usd_sdf::Path) -> Self {
        if let Some(prim) = stage.get_prim_at_path(path) {
            Self::new(prim)
        } else {
            Self::invalid()
        }
    }

    /// Attempt to ensure a prim adhering to this schema at path is defined on this stage.
    ///
    /// Matches C++ `UsdGeomNurbsCurves::Define(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn define(stage: &Stage, path: &usd_sdf::Path) -> Self {
        match stage.define_prim(path.get_string(), Self::schema_type_name().as_str()) {
            Ok(prim) => Self::new(prim),
            Err(_) => Self::invalid(),
        }
    }

    // ========================================================================
    // Order
    // ========================================================================

    /// Returns the order attribute.
    ///
    /// Order of the curve. Order must be positive and is equal to the degree
    /// of the polynomial basis to be evaluated, plus 1.
    ///
    /// Matches C++ `GetOrderAttr()`.
    pub fn get_order_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().order.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the order attribute.
    ///
    /// Matches C++ `CreateOrderAttr()`.
    pub fn create_order_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().order.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().order.as_str())
                .unwrap_or_else(|| Attribute::invalid());
        }

        let registry = ValueTypeRegistry::instance();
        let int_array_type = registry.find_type_by_token(&Token::new("int[]"));

        prim.create_attribute(
            usd_geom_tokens().order.as_str(),
            &int_array_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // Knots
    // ========================================================================

    /// Returns the knots attribute.
    ///
    /// Knot vector providing curve parameterization.
    ///
    /// Matches C++ `GetKnotsAttr()`.
    pub fn get_knots_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().knots.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the knots attribute.
    ///
    /// Matches C++ `CreateKnotsAttr()`.
    pub fn create_knots_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().knots.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().knots.as_str())
                .unwrap_or_else(|| Attribute::invalid());
        }

        let registry = ValueTypeRegistry::instance();
        let double_array_type = registry.find_type_by_token(&Token::new("double[]"));

        prim.create_attribute(
            usd_geom_tokens().knots.as_str(),
            &double_array_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // Ranges
    // ========================================================================

    /// Returns the ranges attribute.
    ///
    /// Provides the minimum and maximum parametric values over which the curve is defined.
    ///
    /// Matches C++ `GetRangesAttr()`.
    pub fn get_ranges_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().ranges.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the ranges attribute.
    ///
    /// Matches C++ `CreateRangesAttr()`.
    pub fn create_ranges_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().ranges.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().ranges.as_str())
                .unwrap_or_else(|| Attribute::invalid());
        }

        let registry = ValueTypeRegistry::instance();
        let double2_array_type = registry.find_type_by_token(&Token::new("double2[]"));

        prim.create_attribute(
            usd_geom_tokens().ranges.as_str(),
            &double2_array_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // PointWeights
    // ========================================================================

    /// Returns the pointWeights attribute.
    ///
    /// Provides the weight for each control point.
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
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().point_weights.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().point_weights.as_str())
                .unwrap_or_else(|| Attribute::invalid());
        }

        let registry = ValueTypeRegistry::instance();
        let double_array_type = registry.find_type_by_token(&Token::new("double[]"));

        prim.create_attribute(
            usd_geom_tokens().point_weights.as_str(),
            &double_array_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // Get Methods (Value Retrieval)
    // ========================================================================

    /// Get the order at the specified time.
    ///
    /// Matches C++ `GetOrder(VtIntArray* order, UsdTimeCode time)`.
    pub fn get_order(&self, time: usd_sdf::TimeCode) -> Option<usd_vt::Array<i32>> {
        self.get_order_attr().get_typed::<usd_vt::Array<i32>>(time)
    }

    /// Get the knots at the specified time.
    ///
    /// Matches C++ `GetKnots(VtDoubleArray* knots, UsdTimeCode time)`.
    pub fn get_knots(&self, time: usd_sdf::TimeCode) -> Option<usd_vt::Array<f64>> {
        self.get_knots_attr().get_typed::<usd_vt::Array<f64>>(time)
    }

    /// Get the ranges at the specified time.
    ///
    /// Matches C++ `GetRanges(VtVec2dArray* ranges, UsdTimeCode time)`.
    pub fn get_ranges(&self, time: usd_sdf::TimeCode) -> Option<usd_vt::Array<(f64, f64)>> {
        // Try to get as Vec2d array, but fallback to tuple array
        // Note: USD uses double2[] which is Vec2d in C++, but we'll use (f64, f64) tuples
        self.get_ranges_attr()
            .get_typed::<usd_vt::Array<(f64, f64)>>(time)
    }

    /// Get the point weights at the specified time.
    ///
    /// Matches C++ `GetPointWeights(VtDoubleArray* weights, UsdTimeCode time)`.
    pub fn get_point_weights(&self, time: usd_sdf::TimeCode) -> Option<usd_vt::Array<f64>> {
        self.get_point_weights_attr()
            .get_typed::<usd_vt::Array<f64>>(time)
    }

    // ========================================================================
    // Compute Extent At Time
    // ========================================================================

    /// Compute the extent for the NURBS curves at the specified time.
    ///
    /// Returns true on success, false if extent was unable to be calculated.
    /// On success, extent will contain the axis-aligned bounding box of the curves.
    ///
    /// Matches C++ `ComputeExtentAtTime(VtVec3fArray* extent, UsdTimeCode time, UsdTimeCode baseTime)`.
    pub fn compute_extent_at_time(
        &self,
        extent: &mut Vec<usd_gf::vec3::Vec3f>,
        time: usd_sdf::TimeCode,
        base_time: usd_sdf::TimeCode,
    ) -> bool {
        // Use Curves base implementation
        self.inner.compute_extent_at_time(extent, time, base_time)
    }

    /// Compute the extent for the NURBS curves at the specified time with transform.
    ///
    /// Returns true on success, false if extent was unable to be calculated.
    ///
    /// Matches C++ `ComputeExtentAtTime(VtVec3fArray* extent, UsdTimeCode time, UsdTimeCode baseTime, const GfMatrix4d& transform)`.
    pub fn compute_extent_at_time_with_transform(
        &self,
        extent: &mut Vec<usd_gf::vec3::Vec3f>,
        time: usd_sdf::TimeCode,
        base_time: usd_sdf::TimeCode,
        transform: &usd_gf::matrix4::Matrix4d,
    ) -> bool {
        // Use Curves base implementation
        self.inner
            .compute_extent_at_time_with_transform(extent, time, base_time, transform)
    }

    /// Compute the extent for the NURBS curves at multiple times.
    ///
    /// Returns true on success, false if extent was unable to be calculated.
    ///
    /// Matches C++ `ComputeExtentAtTimes(std::vector<VtVec3fArray>* extents, const std::vector<UsdTimeCode>& times, UsdTimeCode baseTime)`.
    pub fn compute_extent_at_times(
        &self,
        extents: &mut Vec<Vec<usd_gf::vec3::Vec3f>>,
        times: &[usd_sdf::TimeCode],
        base_time: usd_sdf::TimeCode,
    ) -> bool {
        // Use Curves base implementation
        self.inner
            .compute_extent_at_times(extents, times, base_time)
    }

    /// Compute the extent for the NURBS curves at multiple times with transform.
    ///
    /// Returns true on success, false if extent was unable to be calculated.
    ///
    /// Matches C++ `ComputeExtentAtTimes(std::vector<VtVec3fArray>* extents, const std::vector<UsdTimeCode>& times, UsdTimeCode baseTime, const GfMatrix4d& transform)`.
    pub fn compute_extent_at_times_with_transform(
        &self,
        extents: &mut Vec<Vec<usd_gf::vec3::Vec3f>>,
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
            usd_geom_tokens().order.clone(),
            usd_geom_tokens().knots.clone(),
            usd_geom_tokens().ranges.clone(),
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

impl PartialEq for NurbsCurves {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for NurbsCurves {}
