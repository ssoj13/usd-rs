#![allow(dead_code)]
//! NURBS patch schema for Hydra.
//!
//! Defines NURBS surface patch geometry including vertex counts, orders, knots,
//! ranges, trim curves, orientation, and double-sided flag.

use super::HdSchema;
use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceLocator, HdTypedSampledDataSource, cast_to_container,
};
use once_cell::sync::Lazy;
use std::sync::Arc;
use usd_gf::Vec2d;
use usd_tf::Token;
use usd_vt::Array;

/// NURBS patch schema token
pub static NURBS_PATCH: Lazy<Token> = Lazy::new(|| Token::new("nurbsPatch"));
/// U direction vertex count token
pub static U_VERTEX_COUNT: Lazy<Token> = Lazy::new(|| Token::new("uVertexCount"));
/// V direction vertex count token
pub static V_VERTEX_COUNT: Lazy<Token> = Lazy::new(|| Token::new("vVertexCount"));
/// U direction order token
pub static U_ORDER: Lazy<Token> = Lazy::new(|| Token::new("uOrder"));
/// V direction order token
pub static V_ORDER: Lazy<Token> = Lazy::new(|| Token::new("vOrder"));
/// U direction knots token
pub static U_KNOTS: Lazy<Token> = Lazy::new(|| Token::new("uKnots"));
/// V direction knots token
pub static V_KNOTS: Lazy<Token> = Lazy::new(|| Token::new("vKnots"));
/// U direction form token
pub static U_FORM: Lazy<Token> = Lazy::new(|| Token::new("uForm"));
/// V direction form token
pub static V_FORM: Lazy<Token> = Lazy::new(|| Token::new("vForm"));
/// U direction range token
pub static U_RANGE: Lazy<Token> = Lazy::new(|| Token::new("uRange"));
/// V direction range token
pub static V_RANGE: Lazy<Token> = Lazy::new(|| Token::new("vRange"));
/// Trim curve token
pub static TRIM_CURVE: Lazy<Token> = Lazy::new(|| Token::new("trimCurve"));
/// Orientation token
pub static ORIENTATION: Lazy<Token> = Lazy::new(|| Token::new("orientation"));
/// Double-sided flag token
pub static DOUBLE_SIDED: Lazy<Token> = Lazy::new(|| Token::new("doubleSided"));

// Form tokens
/// Open form token
pub static OPEN: Lazy<Token> = Lazy::new(|| Token::new("open"));
/// Closed form token
pub static CLOSED: Lazy<Token> = Lazy::new(|| Token::new("closed"));
/// Periodic form token
pub static PERIODIC: Lazy<Token> = Lazy::new(|| Token::new("periodic"));

// Orientation tokens
/// Left-handed orientation token
pub static LEFT_HANDED: Lazy<Token> = Lazy::new(|| Token::new("leftHanded"));
/// Right-handed orientation token
pub static RIGHT_HANDED: Lazy<Token> = Lazy::new(|| Token::new("rightHanded"));

/// Data source for int values
pub type HdIntDataSource = dyn HdTypedSampledDataSource<i32>;
/// Arc handle to int data source
pub type HdIntDataSourceHandle = Arc<HdIntDataSource>;

/// Data source for double array values
pub type HdDoubleArrayDataSource = dyn HdTypedSampledDataSource<Array<f64>>;
/// Arc handle to double array data source
pub type HdDoubleArrayDataSourceHandle = Arc<HdDoubleArrayDataSource>;

/// Data source for Token values
pub type HdTokenDataSource = dyn HdTypedSampledDataSource<Token>;
/// Arc handle to Token data source
pub type HdTokenDataSourceHandle = Arc<HdTokenDataSource>;

/// Data source for Vec2d values
pub type HdVec2dDataSource = dyn HdTypedSampledDataSource<Vec2d>;
/// Arc handle to Vec2d data source
pub type HdVec2dDataSourceHandle = Arc<HdVec2dDataSource>;

/// Data source for bool values
pub type HdBoolDataSource = dyn HdTypedSampledDataSource<bool> + Send + Sync;
/// Arc handle to bool data source
pub type HdBoolDataSourceHandle = Arc<HdBoolDataSource>;

/// Schema representing NURBS patch geometry.
///
/// Provides access to:
/// - `uVertexCount`, `vVertexCount` - Vertex counts in U/V directions
/// - `uOrder`, `vOrder` - Polynomial orders in U/V directions
/// - `uKnots`, `vKnots` - Knot vectors in U/V directions
/// - `uForm`, `vForm` - Forms in U/V directions (open, closed, periodic)
/// - `uRange`, `vRange` - Parameter ranges in U/V directions
/// - `trimCurve` - Trim curve container
/// - `orientation` - Surface orientation (leftHanded, rightHanded)
/// - `doubleSided` - Whether patch is double-sided
///
/// # Location
///
/// Default locator: `nurbsPatch`
#[derive(Debug, Clone)]
pub struct HdNurbsPatchSchema {
    schema: HdSchema,
}

impl HdNurbsPatchSchema {
    /// Constructs a NURBS patch schema from a container data source.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Retrieves NURBS patch schema from parent container at "nurbsPatch" locator.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&NURBS_PATCH) {
            if let Some(container) = cast_to_container(&child) {
                return Self::new(container);
            }
        }
        Self {
            schema: HdSchema::empty(),
        }
    }

    /// Returns true if the schema is non-empty.
    pub fn is_defined(&self) -> bool {
        self.schema.is_defined()
    }

    /// Gets the underlying container data source.
    pub fn get_container(&self) -> Option<&HdContainerDataSourceHandle> {
        self.schema.get_container()
    }

    /// Gets U direction vertex count.
    pub fn get_u_vertex_count(&self) -> Option<HdIntDataSourceHandle> {
        self.schema.get_typed(&U_VERTEX_COUNT)
    }

    /// Gets V direction vertex count.
    pub fn get_v_vertex_count(&self) -> Option<HdIntDataSourceHandle> {
        self.schema.get_typed(&V_VERTEX_COUNT)
    }

    /// Gets U direction order.
    pub fn get_u_order(&self) -> Option<HdIntDataSourceHandle> {
        self.schema.get_typed(&U_ORDER)
    }

    /// Gets V direction order.
    pub fn get_v_order(&self) -> Option<HdIntDataSourceHandle> {
        self.schema.get_typed(&V_ORDER)
    }

    /// Gets U direction knots.
    pub fn get_u_knots(&self) -> Option<HdDoubleArrayDataSourceHandle> {
        self.schema.get_typed(&U_KNOTS)
    }

    /// Gets V direction knots.
    pub fn get_v_knots(&self) -> Option<HdDoubleArrayDataSourceHandle> {
        self.schema.get_typed(&V_KNOTS)
    }

    /// Gets U direction form.
    pub fn get_u_form(&self) -> Option<HdTokenDataSourceHandle> {
        self.schema.get_typed(&U_FORM)
    }

    /// Gets V direction form.
    pub fn get_v_form(&self) -> Option<HdTokenDataSourceHandle> {
        self.schema.get_typed(&V_FORM)
    }

    /// Gets U direction range.
    pub fn get_u_range(&self) -> Option<HdVec2dDataSourceHandle> {
        self.schema.get_typed(&U_RANGE)
    }

    /// Gets V direction range.
    pub fn get_v_range(&self) -> Option<HdVec2dDataSourceHandle> {
        self.schema.get_typed(&V_RANGE)
    }

    /// Gets trim curve container.
    pub fn get_trim_curve(&self) -> Option<HdContainerDataSourceHandle> {
        if let Some(container) = self.schema.get_container() {
            if let Some(child) = container.get(&TRIM_CURVE) {
                return cast_to_container(&child);
            }
        }
        None
    }

    /// Gets orientation token.
    pub fn get_orientation(&self) -> Option<HdTokenDataSourceHandle> {
        self.schema.get_typed(&ORIENTATION)
    }

    /// Gets double-sided flag.
    pub fn get_double_sided(&self) -> Option<HdBoolDataSourceHandle> {
        self.schema.get_typed(&DOUBLE_SIDED)
    }

    /// Returns the schema token for NURBS patch.
    pub fn get_schema_token() -> &'static Lazy<Token> {
        &NURBS_PATCH
    }

    /// Returns the default locator for NURBS patch schema.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[NURBS_PATCH.clone()])
    }

    /// Returns the locator for U vertex count.
    pub fn get_u_vertex_count_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[NURBS_PATCH.clone(), U_VERTEX_COUNT.clone()])
    }

    /// Returns the locator for V vertex count.
    pub fn get_v_vertex_count_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[NURBS_PATCH.clone(), V_VERTEX_COUNT.clone()])
    }

    /// Returns the locator for U order.
    pub fn get_u_order_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[NURBS_PATCH.clone(), U_ORDER.clone()])
    }

    /// Returns the locator for V order.
    pub fn get_v_order_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[NURBS_PATCH.clone(), V_ORDER.clone()])
    }

    /// Returns the locator for U knots.
    pub fn get_u_knots_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[NURBS_PATCH.clone(), U_KNOTS.clone()])
    }

    /// Returns the locator for V knots.
    pub fn get_v_knots_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[NURBS_PATCH.clone(), V_KNOTS.clone()])
    }

    /// Returns the locator for U form.
    pub fn get_u_form_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[NURBS_PATCH.clone(), U_FORM.clone()])
    }

    /// Returns the locator for V form.
    pub fn get_v_form_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[NURBS_PATCH.clone(), V_FORM.clone()])
    }

    /// Returns the locator for U range.
    pub fn get_u_range_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[NURBS_PATCH.clone(), U_RANGE.clone()])
    }

    /// Returns the locator for V range.
    pub fn get_v_range_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[NURBS_PATCH.clone(), V_RANGE.clone()])
    }

    /// Returns the locator for trim curve.
    pub fn get_trim_curve_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[NURBS_PATCH.clone(), TRIM_CURVE.clone()])
    }

    /// Returns the locator for orientation.
    pub fn get_orientation_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[NURBS_PATCH.clone(), ORIENTATION.clone()])
    }

    /// Returns the locator for double-sided flag.
    pub fn get_double_sided_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[NURBS_PATCH.clone(), DOUBLE_SIDED.clone()])
    }

    /// Builds a retained container with NURBS patch parameters.
    #[allow(clippy::too_many_arguments)]
    pub fn build_retained(
        u_vertex_count: Option<HdIntDataSourceHandle>,
        v_vertex_count: Option<HdIntDataSourceHandle>,
        u_order: Option<HdIntDataSourceHandle>,
        v_order: Option<HdIntDataSourceHandle>,
        u_knots: Option<HdDoubleArrayDataSourceHandle>,
        v_knots: Option<HdDoubleArrayDataSourceHandle>,
        u_form: Option<HdTokenDataSourceHandle>,
        v_form: Option<HdTokenDataSourceHandle>,
        u_range: Option<HdVec2dDataSourceHandle>,
        v_range: Option<HdVec2dDataSourceHandle>,
        trim_curve: Option<HdContainerDataSourceHandle>,
        orientation: Option<HdTokenDataSourceHandle>,
        double_sided: Option<HdBoolDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        use crate::data_source::{HdDataSourceBaseHandle, HdRetainedContainerDataSource};

        let mut entries = Vec::new();

        if let Some(v) = u_vertex_count {
            entries.push((U_VERTEX_COUNT.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = v_vertex_count {
            entries.push((V_VERTEX_COUNT.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = u_order {
            entries.push((U_ORDER.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = v_order {
            entries.push((V_ORDER.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = u_knots {
            entries.push((U_KNOTS.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = v_knots {
            entries.push((V_KNOTS.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = u_form {
            entries.push((U_FORM.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = v_form {
            entries.push((V_FORM.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = u_range {
            entries.push((U_RANGE.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = v_range {
            entries.push((V_RANGE.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = trim_curve {
            entries.push((TRIM_CURVE.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = orientation {
            entries.push((ORIENTATION.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = double_sided {
            entries.push((DOUBLE_SIDED.clone(), v as HdDataSourceBaseHandle));
        }

        HdRetainedContainerDataSource::from_entries(&entries)
    }
}

/// Builder for HdNurbsPatchSchema using builder pattern.
#[derive(Default)]
pub struct HdNurbsPatchSchemaBuilder {
    u_vertex_count: Option<HdIntDataSourceHandle>,
    v_vertex_count: Option<HdIntDataSourceHandle>,
    u_order: Option<HdIntDataSourceHandle>,
    v_order: Option<HdIntDataSourceHandle>,
    u_knots: Option<HdDoubleArrayDataSourceHandle>,
    v_knots: Option<HdDoubleArrayDataSourceHandle>,
    u_form: Option<HdTokenDataSourceHandle>,
    v_form: Option<HdTokenDataSourceHandle>,
    u_range: Option<HdVec2dDataSourceHandle>,
    v_range: Option<HdVec2dDataSourceHandle>,
    trim_curve: Option<HdContainerDataSourceHandle>,
    orientation: Option<HdTokenDataSourceHandle>,
    double_sided: Option<HdBoolDataSourceHandle>,
}

impl HdNurbsPatchSchemaBuilder {
    /// Creates a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets U vertex count.
    pub fn set_u_vertex_count(mut self, v: HdIntDataSourceHandle) -> Self {
        self.u_vertex_count = Some(v);
        self
    }

    /// Sets V vertex count.
    pub fn set_v_vertex_count(mut self, v: HdIntDataSourceHandle) -> Self {
        self.v_vertex_count = Some(v);
        self
    }

    /// Sets U order.
    pub fn set_u_order(mut self, v: HdIntDataSourceHandle) -> Self {
        self.u_order = Some(v);
        self
    }

    /// Sets V order.
    pub fn set_v_order(mut self, v: HdIntDataSourceHandle) -> Self {
        self.v_order = Some(v);
        self
    }

    /// Sets U knots.
    pub fn set_u_knots(mut self, v: HdDoubleArrayDataSourceHandle) -> Self {
        self.u_knots = Some(v);
        self
    }

    /// Sets V knots.
    pub fn set_v_knots(mut self, v: HdDoubleArrayDataSourceHandle) -> Self {
        self.v_knots = Some(v);
        self
    }

    /// Sets U form.
    pub fn set_u_form(mut self, v: HdTokenDataSourceHandle) -> Self {
        self.u_form = Some(v);
        self
    }

    /// Sets V form.
    pub fn set_v_form(mut self, v: HdTokenDataSourceHandle) -> Self {
        self.v_form = Some(v);
        self
    }

    /// Sets U range.
    pub fn set_u_range(mut self, v: HdVec2dDataSourceHandle) -> Self {
        self.u_range = Some(v);
        self
    }

    /// Sets V range.
    pub fn set_v_range(mut self, v: HdVec2dDataSourceHandle) -> Self {
        self.v_range = Some(v);
        self
    }

    /// Sets trim curve container.
    pub fn set_trim_curve(mut self, v: HdContainerDataSourceHandle) -> Self {
        self.trim_curve = Some(v);
        self
    }

    /// Sets orientation.
    pub fn set_orientation(mut self, v: HdTokenDataSourceHandle) -> Self {
        self.orientation = Some(v);
        self
    }

    /// Sets double-sided flag.
    pub fn set_double_sided(mut self, v: HdBoolDataSourceHandle) -> Self {
        self.double_sided = Some(v);
        self
    }

    /// Builds the container data source.
    pub fn build(self) -> HdContainerDataSourceHandle {
        HdNurbsPatchSchema::build_retained(
            self.u_vertex_count,
            self.v_vertex_count,
            self.u_order,
            self.v_order,
            self.u_knots,
            self.v_knots,
            self.u_form,
            self.v_form,
            self.u_range,
            self.v_range,
            self.trim_curve,
            self.orientation,
            self.double_sided,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nurbs_patch_schema_tokens() {
        assert_eq!(NURBS_PATCH.as_str(), "nurbsPatch");
        assert_eq!(U_VERTEX_COUNT.as_str(), "uVertexCount");
        assert_eq!(V_VERTEX_COUNT.as_str(), "vVertexCount");
        assert_eq!(U_ORDER.as_str(), "uOrder");
        assert_eq!(V_ORDER.as_str(), "vOrder");
        assert_eq!(U_KNOTS.as_str(), "uKnots");
        assert_eq!(V_KNOTS.as_str(), "vKnots");
        assert_eq!(U_FORM.as_str(), "uForm");
        assert_eq!(V_FORM.as_str(), "vForm");
        assert_eq!(U_RANGE.as_str(), "uRange");
        assert_eq!(V_RANGE.as_str(), "vRange");
        assert_eq!(TRIM_CURVE.as_str(), "trimCurve");
        assert_eq!(ORIENTATION.as_str(), "orientation");
        assert_eq!(DOUBLE_SIDED.as_str(), "doubleSided");
    }

    #[test]
    fn test_nurbs_patch_form_tokens() {
        assert_eq!(OPEN.as_str(), "open");
        assert_eq!(CLOSED.as_str(), "closed");
        assert_eq!(PERIODIC.as_str(), "periodic");
    }

    #[test]
    fn test_nurbs_patch_orientation_tokens() {
        assert_eq!(LEFT_HANDED.as_str(), "leftHanded");
        assert_eq!(RIGHT_HANDED.as_str(), "rightHanded");
    }

    #[test]
    fn test_nurbs_patch_schema_locators() {
        let default_loc = HdNurbsPatchSchema::get_default_locator();
        assert_eq!(default_loc.len(), 1);

        let u_vert_loc = HdNurbsPatchSchema::get_u_vertex_count_locator();
        assert_eq!(u_vert_loc.len(), 2);

        let trim_loc = HdNurbsPatchSchema::get_trim_curve_locator();
        assert_eq!(trim_loc.len(), 2);
    }
}
