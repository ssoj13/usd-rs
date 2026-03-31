#![allow(dead_code)]
//! NURBS curves schema for Hydra.
//!
//! Defines NURBS curve geometry including vertex counts, order, knots, and ranges.

use super::HdSchema;
use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceLocator, HdTypedSampledDataSource, cast_to_container,
};
use once_cell::sync::Lazy;
use std::sync::Arc;
use usd_gf::Vec2d;
use usd_tf::Token;
use usd_vt::Array;

/// NURBS curves schema token
pub static NURBS_CURVES: Lazy<Token> = Lazy::new(|| Token::new("nurbsCurves"));
/// Curve vertex counts token
pub static CURVE_VERTEX_COUNTS: Lazy<Token> = Lazy::new(|| Token::new("curveVertexCounts"));
/// Curve order token
pub static ORDER: Lazy<Token> = Lazy::new(|| Token::new("order"));
/// Knot vector token
pub static KNOTS: Lazy<Token> = Lazy::new(|| Token::new("knots"));
/// Parameter ranges token
pub static RANGES: Lazy<Token> = Lazy::new(|| Token::new("ranges"));

/// Data source for int array values
pub type HdIntArrayDataSource = dyn HdTypedSampledDataSource<Array<i32>>;
/// Arc handle to int array data source
pub type HdIntArrayDataSourceHandle = Arc<HdIntArrayDataSource>;

/// Data source for double array values
pub type HdDoubleArrayDataSource = dyn HdTypedSampledDataSource<Array<f64>>;
/// Arc handle to double array data source
pub type HdDoubleArrayDataSourceHandle = Arc<HdDoubleArrayDataSource>;

/// Data source for Vec2d array values
pub type HdVec2dArrayDataSource = dyn HdTypedSampledDataSource<Array<Vec2d>>;
/// Arc handle to Vec2d array data source
pub type HdVec2dArrayDataSourceHandle = Arc<HdVec2dArrayDataSource>;

/// Schema representing NURBS curves geometry.
///
/// Provides access to:
/// - `curveVertexCounts` - Number of vertices per curve
/// - `order` - Polynomial order per curve
/// - `knots` - Knot vector values
/// - `ranges` - Parameter ranges for each curve
///
/// # Location
///
/// Default locator: `nurbsCurves`
#[derive(Debug, Clone)]
pub struct HdNurbsCurvesSchema {
    schema: HdSchema,
}

impl HdNurbsCurvesSchema {
    /// Constructs a NURBS curves schema from a container data source.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Retrieves NURBS curves schema from parent container at "nurbsCurves" locator.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&NURBS_CURVES) {
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

    /// Gets curve vertex counts array.
    pub fn get_curve_vertex_counts(&self) -> Option<HdIntArrayDataSourceHandle> {
        self.schema.get_typed(&CURVE_VERTEX_COUNTS)
    }

    /// Gets curve order array.
    pub fn get_order(&self) -> Option<HdIntArrayDataSourceHandle> {
        self.schema.get_typed(&ORDER)
    }

    /// Gets knot vector array.
    pub fn get_knots(&self) -> Option<HdDoubleArrayDataSourceHandle> {
        self.schema.get_typed(&KNOTS)
    }

    /// Gets parameter ranges array.
    pub fn get_ranges(&self) -> Option<HdVec2dArrayDataSourceHandle> {
        self.schema.get_typed(&RANGES)
    }

    /// Returns the schema token for NURBS curves.
    pub fn get_schema_token() -> &'static Lazy<Token> {
        &NURBS_CURVES
    }

    /// Returns the default locator for NURBS curves schema.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[NURBS_CURVES.clone()])
    }

    /// Returns the locator for curve vertex counts.
    pub fn get_curve_vertex_counts_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[NURBS_CURVES.clone(), CURVE_VERTEX_COUNTS.clone()])
    }

    /// Returns the locator for order.
    pub fn get_order_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[NURBS_CURVES.clone(), ORDER.clone()])
    }

    /// Returns the locator for knots.
    pub fn get_knots_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[NURBS_CURVES.clone(), KNOTS.clone()])
    }

    /// Returns the locator for ranges.
    pub fn get_ranges_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[NURBS_CURVES.clone(), RANGES.clone()])
    }

    /// Builds a retained container with NURBS curves parameters.
    ///
    /// # Parameters
    /// - `curve_vertex_counts` - Vertex counts per curve
    /// - `order` - Polynomial order per curve
    /// - `knots` - Knot vector
    /// - `ranges` - Parameter ranges
    pub fn build_retained(
        curve_vertex_counts: Option<HdIntArrayDataSourceHandle>,
        order: Option<HdIntArrayDataSourceHandle>,
        knots: Option<HdDoubleArrayDataSourceHandle>,
        ranges: Option<HdVec2dArrayDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        use crate::data_source::{HdDataSourceBaseHandle, HdRetainedContainerDataSource};

        let mut entries = Vec::new();

        if let Some(c) = curve_vertex_counts {
            entries.push((CURVE_VERTEX_COUNTS.clone(), c as HdDataSourceBaseHandle));
        }
        if let Some(o) = order {
            entries.push((ORDER.clone(), o as HdDataSourceBaseHandle));
        }
        if let Some(k) = knots {
            entries.push((KNOTS.clone(), k as HdDataSourceBaseHandle));
        }
        if let Some(r) = ranges {
            entries.push((RANGES.clone(), r as HdDataSourceBaseHandle));
        }

        HdRetainedContainerDataSource::from_entries(&entries)
    }
}

/// Builder for HdNurbsCurvesSchema using builder pattern.
#[derive(Default)]
pub struct HdNurbsCurvesSchemaBuilder {
    curve_vertex_counts: Option<HdIntArrayDataSourceHandle>,
    order: Option<HdIntArrayDataSourceHandle>,
    knots: Option<HdDoubleArrayDataSourceHandle>,
    ranges: Option<HdVec2dArrayDataSourceHandle>,
}

impl HdNurbsCurvesSchemaBuilder {
    /// Creates a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets curve vertex counts.
    pub fn set_curve_vertex_counts(mut self, v: HdIntArrayDataSourceHandle) -> Self {
        self.curve_vertex_counts = Some(v);
        self
    }

    /// Sets curve order.
    pub fn set_order(mut self, v: HdIntArrayDataSourceHandle) -> Self {
        self.order = Some(v);
        self
    }

    /// Sets knot vector.
    pub fn set_knots(mut self, v: HdDoubleArrayDataSourceHandle) -> Self {
        self.knots = Some(v);
        self
    }

    /// Sets parameter ranges.
    pub fn set_ranges(mut self, v: HdVec2dArrayDataSourceHandle) -> Self {
        self.ranges = Some(v);
        self
    }

    /// Builds the container data source.
    pub fn build(self) -> HdContainerDataSourceHandle {
        HdNurbsCurvesSchema::build_retained(
            self.curve_vertex_counts,
            self.order,
            self.knots,
            self.ranges,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nurbs_curves_schema_tokens() {
        assert_eq!(NURBS_CURVES.as_str(), "nurbsCurves");
        assert_eq!(CURVE_VERTEX_COUNTS.as_str(), "curveVertexCounts");
        assert_eq!(ORDER.as_str(), "order");
        assert_eq!(KNOTS.as_str(), "knots");
        assert_eq!(RANGES.as_str(), "ranges");
    }

    #[test]
    fn test_nurbs_curves_schema_locators() {
        let default_loc = HdNurbsCurvesSchema::get_default_locator();
        assert_eq!(default_loc.len(), 1);

        let counts_loc = HdNurbsCurvesSchema::get_curve_vertex_counts_locator();
        assert_eq!(counts_loc.len(), 2);

        let order_loc = HdNurbsCurvesSchema::get_order_locator();
        assert_eq!(order_loc.len(), 2);

        let knots_loc = HdNurbsCurvesSchema::get_knots_locator();
        assert_eq!(knots_loc.len(), 2);

        let ranges_loc = HdNurbsCurvesSchema::get_ranges_locator();
        assert_eq!(ranges_loc.len(), 2);
    }
}
