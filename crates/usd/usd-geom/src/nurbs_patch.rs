//! UsdGeomNurbsPatch - NURBS patch geometry schema.
//!
//! Port of pxr/usd/usdGeom/nurbsPatch.h/cpp
//!
//! Encodes a rational or polynomial non-uniform B-spline surface, with optional trim curves.

use super::point_based::PointBased;
use super::tokens::usd_geom_tokens;
use usd_core::attribute::Variability;
use usd_core::{Attribute, Prim, Stage};
use usd_sdf::ValueTypeRegistry;
use usd_tf::Token;
use usd_vt::Value;

// ============================================================================
// NurbsPatch
// ============================================================================

/// NURBS patch geometry schema.
///
/// Encodes a rational or polynomial non-uniform B-spline surface, with optional trim curves.
///
/// Matches C++ `UsdGeomNurbsPatch`.
#[derive(Debug, Clone)]
pub struct NurbsPatch {
    /// Base point-based schema.
    inner: PointBased,
}

impl NurbsPatch {
    /// Creates a NurbsPatch schema from a prim.
    ///
    /// Matches C++ `UsdGeomNurbsPatch(const UsdPrim& prim)`.
    pub fn new(prim: Prim) -> Self {
        Self {
            inner: PointBased::new(prim),
        }
    }

    /// Creates a NurbsPatch schema from a PointBased schema.
    ///
    /// Matches C++ `UsdGeomNurbsPatch(const UsdSchemaBase& schemaObj)`.
    pub fn from_point_based(point_based: PointBased) -> Self {
        Self { inner: point_based }
    }

    /// Creates an invalid NurbsPatch schema.
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
        Token::new("NurbsPatch")
    }

    /// Return a NurbsPatch holding the prim adhering to this schema at path on stage.
    ///
    /// Matches C++ `UsdGeomNurbsPatch::Get(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn get(stage: &Stage, path: &usd_sdf::Path) -> Self {
        if let Some(prim) = stage.get_prim_at_path(path) {
            Self::new(prim)
        } else {
            Self::invalid()
        }
    }

    /// Attempt to ensure a prim adhering to this schema at path is defined on this stage.
    ///
    /// Matches C++ `UsdGeomNurbsPatch::Define(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn define(stage: &Stage, path: &usd_sdf::Path) -> Self {
        match stage.define_prim(path.get_string(), Self::schema_type_name().as_str()) {
            Ok(prim) => Self::new(prim),
            Err(_) => Self::invalid(),
        }
    }

    // ========================================================================
    // UVertexCount
    // ========================================================================

    /// Returns the uVertexCount attribute.
    ///
    /// Number of vertices in the U direction.
    ///
    /// Matches C++ `GetUVertexCountAttr()`.
    pub fn get_u_vertex_count_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().u_vertex_count.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the uVertexCount attribute.
    ///
    /// Matches C++ `CreateUVertexCountAttr()`.
    pub fn create_u_vertex_count_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().u_vertex_count.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().u_vertex_count.as_str())
                .unwrap_or_else(|| Attribute::invalid());
        }

        let registry = ValueTypeRegistry::instance();
        let int_type = registry.find_type_by_token(&Token::new("int"));

        prim.create_attribute(
            usd_geom_tokens().u_vertex_count.as_str(),
            &int_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // VVertexCount
    // ========================================================================

    /// Returns the vVertexCount attribute.
    ///
    /// Number of vertices in the V direction.
    ///
    /// Matches C++ `GetVVertexCountAttr()`.
    pub fn get_v_vertex_count_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().v_vertex_count.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the vVertexCount attribute.
    ///
    /// Matches C++ `CreateVVertexCountAttr()`.
    pub fn create_v_vertex_count_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().v_vertex_count.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().v_vertex_count.as_str())
                .unwrap_or_else(|| Attribute::invalid());
        }

        let registry = ValueTypeRegistry::instance();
        let int_type = registry.find_type_by_token(&Token::new("int"));

        prim.create_attribute(
            usd_geom_tokens().v_vertex_count.as_str(),
            &int_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // UOrder
    // ========================================================================

    /// Returns the uOrder attribute.
    ///
    /// Order in the U direction.
    ///
    /// Matches C++ `GetUOrderAttr()`.
    pub fn get_u_order_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().u_order.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the uOrder attribute.
    ///
    /// Matches C++ `CreateUOrderAttr()`.
    pub fn create_u_order_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().u_order.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().u_order.as_str())
                .unwrap_or_else(|| Attribute::invalid());
        }

        let registry = ValueTypeRegistry::instance();
        let int_type = registry.find_type_by_token(&Token::new("int"));

        prim.create_attribute(
            usd_geom_tokens().u_order.as_str(),
            &int_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // VOrder
    // ========================================================================

    /// Returns the vOrder attribute.
    ///
    /// Order in the V direction.
    ///
    /// Matches C++ `GetVOrderAttr()`.
    pub fn get_v_order_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().v_order.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the vOrder attribute.
    ///
    /// Matches C++ `CreateVOrderAttr()`.
    pub fn create_v_order_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().v_order.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().v_order.as_str())
                .unwrap_or_else(|| Attribute::invalid());
        }

        let registry = ValueTypeRegistry::instance();
        let int_type = registry.find_type_by_token(&Token::new("int"));

        prim.create_attribute(
            usd_geom_tokens().v_order.as_str(),
            &int_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // UKnots
    // ========================================================================

    /// Returns the uKnots attribute.
    ///
    /// Knot vector for U direction providing U parameterization.
    ///
    /// Matches C++ `GetUKnotsAttr()`.
    pub fn get_u_knots_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().u_knots.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the uKnots attribute.
    ///
    /// Matches C++ `CreateUKnotsAttr()`.
    pub fn create_u_knots_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().u_knots.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().u_knots.as_str())
                .unwrap_or_else(|| Attribute::invalid());
        }

        let registry = ValueTypeRegistry::instance();
        let double_array_type = registry.find_type_by_token(&Token::new("double[]"));

        prim.create_attribute(
            usd_geom_tokens().u_knots.as_str(),
            &double_array_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // VKnots
    // ========================================================================

    /// Returns the vKnots attribute.
    ///
    /// Knot vector for V direction providing V parameterization.
    ///
    /// Matches C++ `GetVKnotsAttr()`.
    pub fn get_v_knots_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().v_knots.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the vKnots attribute.
    ///
    /// Matches C++ `CreateVKnotsAttr()`.
    pub fn create_v_knots_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().v_knots.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().v_knots.as_str())
                .unwrap_or_else(|| Attribute::invalid());
        }

        let registry = ValueTypeRegistry::instance();
        let double_array_type = registry.find_type_by_token(&Token::new("double[]"));

        prim.create_attribute(
            usd_geom_tokens().v_knots.as_str(),
            &double_array_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // URange
    // ========================================================================

    /// Returns the uRange attribute.
    ///
    /// Provides the minimum and maximum parametric values over which the surface is defined.
    ///
    /// Matches C++ `GetURangeAttr()`.
    pub fn get_u_range_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().u_range.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the uRange attribute.
    ///
    /// Matches C++ `CreateURangeAttr()`.
    pub fn create_u_range_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().u_range.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().u_range.as_str())
                .unwrap_or_else(|| Attribute::invalid());
        }

        let registry = ValueTypeRegistry::instance();
        let double2_type = registry.find_type_by_token(&Token::new("double2"));

        prim.create_attribute(
            usd_geom_tokens().u_range.as_str(),
            &double2_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // VRange
    // ========================================================================

    /// Returns the vRange attribute.
    ///
    /// Provides the minimum and maximum parametric values over which the surface is defined.
    ///
    /// Matches C++ `GetVRangeAttr()`.
    pub fn get_v_range_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().v_range.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the vRange attribute.
    ///
    /// Matches C++ `CreateVRangeAttr()`.
    pub fn create_v_range_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().v_range.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().v_range.as_str())
                .unwrap_or_else(|| Attribute::invalid());
        }

        let registry = ValueTypeRegistry::instance();
        let double2_type = registry.find_type_by_token(&Token::new("double2"));

        prim.create_attribute(
            usd_geom_tokens().v_range.as_str(),
            &double2_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // UForm
    // ========================================================================

    /// Returns the uForm attribute.
    ///
    /// Interpret the control grid and knot vectors as representing an open, closed, or periodic surface.
    ///
    /// Matches C++ `GetUFormAttr()`.
    pub fn get_u_form_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().u_form.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the uForm attribute.
    ///
    /// Matches C++ `CreateUFormAttr()`.
    pub fn create_u_form_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().u_form.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().u_form.as_str())
                .unwrap_or_else(|| Attribute::invalid());
        }

        let registry = ValueTypeRegistry::instance();
        let token_type = registry.find_type_by_token(&Token::new("token"));

        prim.create_attribute(
            usd_geom_tokens().u_form.as_str(),
            &token_type,
            false,                      // not custom
            Some(Variability::Uniform), // uniform
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // VForm
    // ========================================================================

    /// Returns the vForm attribute.
    ///
    /// Interpret the control grid and knot vectors as representing an open, closed, or periodic surface.
    ///
    /// Matches C++ `GetVFormAttr()`.
    pub fn get_v_form_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().v_form.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the vForm attribute.
    ///
    /// Matches C++ `CreateVFormAttr()`.
    pub fn create_v_form_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().v_form.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().v_form.as_str())
                .unwrap_or_else(|| Attribute::invalid());
        }

        let registry = ValueTypeRegistry::instance();
        let token_type = registry.find_type_by_token(&Token::new("token"));

        prim.create_attribute(
            usd_geom_tokens().v_form.as_str(),
            &token_type,
            false,                      // not custom
            Some(Variability::Uniform), // uniform
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // PointWeights
    // ========================================================================

    /// Returns the pointWeights attribute.
    ///
    /// Optionally provides "w" components for each control point.
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
    // TrimCurveCounts
    // ========================================================================

    /// Returns the trimCurveCounts attribute.
    ///
    /// Each element specifies how many curves are present in each "loop" of the trimCurve.
    ///
    /// Matches C++ `GetTrimCurveCountsAttr()`.
    pub fn get_trim_curve_counts_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().trim_curve_counts.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the trimCurveCounts attribute.
    ///
    /// Matches C++ `CreateTrimCurveCountsAttr()`.
    pub fn create_trim_curve_counts_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().trim_curve_counts.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().trim_curve_counts.as_str())
                .unwrap_or_else(|| Attribute::invalid());
        }

        let registry = ValueTypeRegistry::instance();
        let int_array_type = registry.find_type_by_token(&Token::new("int[]"));

        prim.create_attribute(
            usd_geom_tokens().trim_curve_counts.as_str(),
            &int_array_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // TrimCurveOrders
    // ========================================================================

    /// Returns the trimCurveOrders attribute.
    ///
    /// Flat list of orders for each of the curves.
    ///
    /// Matches C++ `GetTrimCurveOrdersAttr()`.
    pub fn get_trim_curve_orders_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().trim_curve_orders.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the trimCurveOrders attribute.
    ///
    /// Matches C++ `CreateTrimCurveOrdersAttr()`.
    pub fn create_trim_curve_orders_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().trim_curve_orders.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().trim_curve_orders.as_str())
                .unwrap_or_else(|| Attribute::invalid());
        }

        let registry = ValueTypeRegistry::instance();
        let int_array_type = registry.find_type_by_token(&Token::new("int[]"));

        prim.create_attribute(
            usd_geom_tokens().trim_curve_orders.as_str(),
            &int_array_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // TrimCurveVertexCounts
    // ========================================================================

    /// Returns the trimCurveVertexCounts attribute.
    ///
    /// Flat list of number of vertices for each of the curves.
    ///
    /// Matches C++ `GetTrimCurveVertexCountsAttr()`.
    pub fn get_trim_curve_vertex_counts_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().trim_curve_vertex_counts.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the trimCurveVertexCounts attribute.
    ///
    /// Matches C++ `CreateTrimCurveVertexCountsAttr()`.
    pub fn create_trim_curve_vertex_counts_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().trim_curve_vertex_counts.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().trim_curve_vertex_counts.as_str())
                .unwrap_or_else(|| Attribute::invalid());
        }

        let registry = ValueTypeRegistry::instance();
        let int_array_type = registry.find_type_by_token(&Token::new("int[]"));

        prim.create_attribute(
            usd_geom_tokens().trim_curve_vertex_counts.as_str(),
            &int_array_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // TrimCurveKnots
    // ========================================================================

    /// Returns the trimCurveKnots attribute.
    ///
    /// Flat list of parametric values for each of the curves.
    ///
    /// Matches C++ `GetTrimCurveKnotsAttr()`.
    pub fn get_trim_curve_knots_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().trim_curve_knots.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the trimCurveKnots attribute.
    ///
    /// Matches C++ `CreateTrimCurveKnotsAttr()`.
    pub fn create_trim_curve_knots_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().trim_curve_knots.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().trim_curve_knots.as_str())
                .unwrap_or_else(|| Attribute::invalid());
        }

        let registry = ValueTypeRegistry::instance();
        let double_array_type = registry.find_type_by_token(&Token::new("double[]"));

        prim.create_attribute(
            usd_geom_tokens().trim_curve_knots.as_str(),
            &double_array_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // TrimCurveRanges
    // ========================================================================

    /// Returns the trimCurveRanges attribute.
    ///
    /// Flat list of minimum and maximum parametric values for each of the curves.
    ///
    /// Matches C++ `GetTrimCurveRangesAttr()`.
    pub fn get_trim_curve_ranges_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().trim_curve_ranges.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the trimCurveRanges attribute.
    ///
    /// Matches C++ `CreateTrimCurveRangesAttr()`.
    pub fn create_trim_curve_ranges_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().trim_curve_ranges.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().trim_curve_ranges.as_str())
                .unwrap_or_else(|| Attribute::invalid());
        }

        let registry = ValueTypeRegistry::instance();
        let double2_array_type = registry.find_type_by_token(&Token::new("double2[]"));

        prim.create_attribute(
            usd_geom_tokens().trim_curve_ranges.as_str(),
            &double2_array_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // TrimCurvePoints
    // ========================================================================

    /// Returns the trimCurvePoints attribute.
    ///
    /// Flat list of homogeneous 2D points (u, v, w) that comprise the curves.
    ///
    /// Matches C++ `GetTrimCurvePointsAttr()`.
    pub fn get_trim_curve_points_attr(&self) -> Attribute {
        let prim = self.inner.prim();
        prim.get_attribute(usd_geom_tokens().trim_curve_points.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the trimCurvePoints attribute.
    ///
    /// Matches C++ `CreateTrimCurvePointsAttr()`.
    pub fn create_trim_curve_points_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = self.inner.prim();
        if !prim.is_valid() {
            return Attribute::invalid();
        }

        if prim.has_authored_attribute(usd_geom_tokens().trim_curve_points.as_str()) {
            return prim
                .get_attribute(usd_geom_tokens().trim_curve_points.as_str())
                .unwrap_or_else(|| Attribute::invalid());
        }

        let registry = ValueTypeRegistry::instance();
        let double3_array_type = registry.find_type_by_token(&Token::new("double3[]"));

        prim.create_attribute(
            usd_geom_tokens().trim_curve_points.as_str(),
            &double3_array_type,
            false,                      // not custom
            Some(Variability::Varying), // can vary over time
        )
        .unwrap_or_else(Attribute::invalid)
    }

    // ========================================================================
    // Get Methods (Value Retrieval)
    // ========================================================================

    /// Get the uVertexCount at the specified time.
    ///
    /// Matches C++ `GetUVertexCount(int* uVertexCount, UsdTimeCode time)`.
    pub fn get_u_vertex_count(&self, time: usd_sdf::TimeCode) -> Option<i32> {
        self.get_u_vertex_count_attr().get_typed::<i32>(time)
    }

    /// Get the vVertexCount at the specified time.
    ///
    /// Matches C++ `GetVVertexCount(int* vVertexCount, UsdTimeCode time)`.
    pub fn get_v_vertex_count(&self, time: usd_sdf::TimeCode) -> Option<i32> {
        self.get_v_vertex_count_attr().get_typed::<i32>(time)
    }

    /// Get the uOrder at the specified time.
    ///
    /// Matches C++ `GetUOrder(int* uOrder, UsdTimeCode time)`.
    pub fn get_u_order(&self, time: usd_sdf::TimeCode) -> Option<i32> {
        self.get_u_order_attr().get_typed::<i32>(time)
    }

    /// Get the vOrder at the specified time.
    ///
    /// Matches C++ `GetVOrder(int* vOrder, UsdTimeCode time)`.
    pub fn get_v_order(&self, time: usd_sdf::TimeCode) -> Option<i32> {
        self.get_v_order_attr().get_typed::<i32>(time)
    }

    /// Get the uKnots at the specified time.
    ///
    /// Matches C++ `GetUKnots(VtDoubleArray* uKnots, UsdTimeCode time)`.
    pub fn get_u_knots(&self, time: usd_sdf::TimeCode) -> Option<usd_vt::Array<f64>> {
        self.get_u_knots_attr()
            .get_typed::<usd_vt::Array<f64>>(time)
    }

    /// Get the vKnots at the specified time.
    ///
    /// Matches C++ `GetVKnots(VtDoubleArray* vKnots, UsdTimeCode time)`.
    pub fn get_v_knots(&self, time: usd_sdf::TimeCode) -> Option<usd_vt::Array<f64>> {
        self.get_v_knots_attr()
            .get_typed::<usd_vt::Array<f64>>(time)
    }

    /// Get the uRange at the specified time.
    ///
    /// Matches C++ `GetURange(GfVec2d* uRange, UsdTimeCode time)`.
    pub fn get_u_range(&self, time: usd_sdf::TimeCode) -> Option<(f64, f64)> {
        // Try to get as double2 (tuple)
        self.get_u_range_attr().get_typed::<(f64, f64)>(time)
    }

    /// Get the vRange at the specified time.
    ///
    /// Matches C++ `GetVRange(GfVec2d* vRange, UsdTimeCode time)`.
    pub fn get_v_range(&self, time: usd_sdf::TimeCode) -> Option<(f64, f64)> {
        // Try to get as double2 (tuple)
        self.get_v_range_attr().get_typed::<(f64, f64)>(time)
    }

    /// Get the uForm at the specified time.
    ///
    /// Matches C++ `GetUForm(TfToken* uForm, UsdTimeCode time)`.
    pub fn get_u_form(&self, time: usd_sdf::TimeCode) -> Option<Token> {
        self.get_u_form_attr().get_typed::<Token>(time)
    }

    /// Get the vForm at the specified time.
    ///
    /// Matches C++ `GetVForm(TfToken* vForm, UsdTimeCode time)`.
    pub fn get_v_form(&self, time: usd_sdf::TimeCode) -> Option<Token> {
        self.get_v_form_attr().get_typed::<Token>(time)
    }

    /// Get the pointWeights at the specified time.
    ///
    /// Matches C++ `GetPointWeights(VtDoubleArray* pointWeights, UsdTimeCode time)`.
    pub fn get_point_weights(&self, time: usd_sdf::TimeCode) -> Option<usd_vt::Array<f64>> {
        self.get_point_weights_attr()
            .get_typed::<usd_vt::Array<f64>>(time)
    }

    /// Get the trimCurveCounts at the specified time.
    ///
    /// Matches C++ `GetTrimCurveCounts(VtIntArray* trimCurveCounts, UsdTimeCode time)`.
    pub fn get_trim_curve_counts(&self, time: usd_sdf::TimeCode) -> Option<usd_vt::Array<i32>> {
        self.get_trim_curve_counts_attr()
            .get_typed::<usd_vt::Array<i32>>(time)
    }

    /// Get the trimCurveOrders at the specified time.
    ///
    /// Matches C++ `GetTrimCurveOrders(VtIntArray* trimCurveOrders, UsdTimeCode time)`.
    pub fn get_trim_curve_orders(&self, time: usd_sdf::TimeCode) -> Option<usd_vt::Array<i32>> {
        self.get_trim_curve_orders_attr()
            .get_typed::<usd_vt::Array<i32>>(time)
    }

    /// Get the trimCurveVertexCounts at the specified time.
    ///
    /// Matches C++ `GetTrimCurveVertexCounts(VtIntArray* trimCurveVertexCounts, UsdTimeCode time)`.
    pub fn get_trim_curve_vertex_counts(
        &self,
        time: usd_sdf::TimeCode,
    ) -> Option<usd_vt::Array<i32>> {
        self.get_trim_curve_vertex_counts_attr()
            .get_typed::<usd_vt::Array<i32>>(time)
    }

    /// Get the trimCurveKnots at the specified time.
    ///
    /// Matches C++ `GetTrimCurveKnots(VtDoubleArray* trimCurveKnots, UsdTimeCode time)`.
    pub fn get_trim_curve_knots(&self, time: usd_sdf::TimeCode) -> Option<usd_vt::Array<f64>> {
        self.get_trim_curve_knots_attr()
            .get_typed::<usd_vt::Array<f64>>(time)
    }

    /// Get the trimCurveRanges at the specified time.
    ///
    /// Matches C++ `GetTrimCurveRanges(VtVec2dArray* trimCurveRanges, UsdTimeCode time)`.
    pub fn get_trim_curve_ranges(
        &self,
        time: usd_sdf::TimeCode,
    ) -> Option<usd_vt::Array<(f64, f64)>> {
        self.get_trim_curve_ranges_attr()
            .get_typed::<usd_vt::Array<(f64, f64)>>(time)
    }

    /// Get the trimCurvePoints at the specified time.
    ///
    /// Matches C++ `GetTrimCurvePoints(VtVec3dArray* trimCurvePoints, UsdTimeCode time)`.
    pub fn get_trim_curve_points(
        &self,
        time: usd_sdf::TimeCode,
    ) -> Option<usd_vt::Array<(f64, f64, f64)>> {
        self.get_trim_curve_points_attr()
            .get_typed::<usd_vt::Array<(f64, f64, f64)>>(time)
    }

    // ========================================================================
    // Compute Extent At Time
    // ========================================================================

    /// Compute the extent for the NURBS patch at the specified time.
    ///
    /// Returns true on success, false if extent was unable to be calculated.
    /// On success, extent will contain the axis-aligned bounding box of the patch.
    ///
    /// Matches C++ `ComputeExtentAtTime(VtVec3fArray* extent, UsdTimeCode time, UsdTimeCode baseTime)`.
    pub fn compute_extent_at_time(
        &self,
        extent: &mut Vec<usd_gf::vec3::Vec3f>,
        time: usd_sdf::TimeCode,
        _base_time: usd_sdf::TimeCode,
    ) -> bool {
        // Get points at the specified time
        let points_attr = self.inner.get_points_attr();
        if !points_attr.is_valid() {
            return false;
        }

        let points = match points_attr.get_typed::<usd_vt::Array<usd_gf::vec3::Vec3f>>(time) {
            Some(p) => p,
            None => return false,
        };

        // Convert to slice for compute_extent
        let points_slice: Vec<usd_gf::vec3::Vec3f> = points.iter().cloned().collect();
        if points_slice.is_empty() {
            return false;
        }

        // Compute extent using PointBased static method
        let mut extent_array = [
            usd_gf::vec3::Vec3f::new(0.0, 0.0, 0.0),
            usd_gf::vec3::Vec3f::new(0.0, 0.0, 0.0),
        ];
        if !super::point_based::PointBased::compute_extent(&points_slice, &mut extent_array) {
            return false;
        }

        extent.clear();
        extent.push(extent_array[0]);
        extent.push(extent_array[1]);
        true
    }

    /// Compute the extent for the NURBS patch at the specified time with transform.
    ///
    /// Returns true on success, false if extent was unable to be calculated.
    ///
    /// Matches C++ `ComputeExtentAtTime(VtVec3fArray* extent, UsdTimeCode time, UsdTimeCode baseTime, const GfMatrix4d& transform)`.
    pub fn compute_extent_at_time_with_transform(
        &self,
        extent: &mut Vec<usd_gf::vec3::Vec3f>,
        time: usd_sdf::TimeCode,
        _base_time: usd_sdf::TimeCode,
        transform: &usd_gf::matrix4::Matrix4d,
    ) -> bool {
        // Get points at the specified time
        let points_attr = self.inner.get_points_attr();
        if !points_attr.is_valid() {
            return false;
        }

        let points = match points_attr.get_typed::<usd_vt::Array<usd_gf::vec3::Vec3f>>(time) {
            Some(p) => p,
            None => return false,
        };

        // Convert to slice for compute_extent_with_transform
        let points_slice: Vec<usd_gf::vec3::Vec3f> = points.iter().cloned().collect();
        if points_slice.is_empty() {
            return false;
        }

        // Compute extent using PointBased static method with transform
        let mut extent_array = [
            usd_gf::vec3::Vec3f::new(0.0, 0.0, 0.0),
            usd_gf::vec3::Vec3f::new(0.0, 0.0, 0.0),
        ];
        if !super::point_based::PointBased::compute_extent_with_transform(
            &points_slice,
            transform,
            &mut extent_array,
        ) {
            return false;
        }

        extent.clear();
        extent.push(extent_array[0]);
        extent.push(extent_array[1]);
        true
    }

    /// Compute the extent for the NURBS patch at multiple times.
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

    /// Compute the extent for the NURBS patch at multiple times with transform.
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

    /// Return a vector of names of all pre-declared attributes for this schema
    /// class and all its ancestor classes.
    ///
    /// Matches C++ `GetSchemaAttributeNames(bool includeInherited)`.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        let local_names = vec![
            usd_geom_tokens().u_vertex_count.clone(),
            usd_geom_tokens().v_vertex_count.clone(),
            usd_geom_tokens().u_order.clone(),
            usd_geom_tokens().v_order.clone(),
            usd_geom_tokens().u_knots.clone(),
            usd_geom_tokens().v_knots.clone(),
            usd_geom_tokens().u_range.clone(),
            usd_geom_tokens().v_range.clone(),
            usd_geom_tokens().u_form.clone(),
            usd_geom_tokens().v_form.clone(),
            usd_geom_tokens().point_weights.clone(),
            usd_geom_tokens().trim_curve_counts.clone(),
            usd_geom_tokens().trim_curve_orders.clone(),
            usd_geom_tokens().trim_curve_vertex_counts.clone(),
            usd_geom_tokens().trim_curve_knots.clone(),
            usd_geom_tokens().trim_curve_ranges.clone(),
            usd_geom_tokens().trim_curve_points.clone(),
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

impl PartialEq for NurbsPatch {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for NurbsPatch {}
