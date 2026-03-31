
//! HdNurbsPatchTrimCurveSchema - NURBS patch trim curve.
//!
//! Corresponds to pxr/imaging/hd/nurbsPatchTrimCurveSchema.h

use super::HdSchema;
use crate::data_source::{HdContainerDataSourceHandle, cast_to_container};
use once_cell::sync::Lazy;
use std::sync::Arc;
use usd_gf::{Vec2d, Vec3d};
use usd_tf::Token;
use usd_vt::Array;

static TRIM_CURVE: Lazy<Token> = Lazy::new(|| Token::new("trimCurve"));
static COUNTS: Lazy<Token> = Lazy::new(|| Token::new("counts"));
static ORDERS: Lazy<Token> = Lazy::new(|| Token::new("orders"));
static VERTEX_COUNTS: Lazy<Token> = Lazy::new(|| Token::new("vertexCounts"));
static KNOTS: Lazy<Token> = Lazy::new(|| Token::new("knots"));
static RANGES: Lazy<Token> = Lazy::new(|| Token::new("ranges"));
static POINTS: Lazy<Token> = Lazy::new(|| Token::new("points"));

pub type HdIntArrayDataSourceHandle =
    Arc<dyn crate::data_source::HdTypedSampledDataSource<Array<i32>> + Send + Sync>;
pub type HdDoubleArrayDataSourceHandle =
    Arc<dyn crate::data_source::HdTypedSampledDataSource<Array<f64>> + Send + Sync>;
pub type HdVec2dArrayDataSourceHandle =
    Arc<dyn crate::data_source::HdTypedSampledDataSource<Array<Vec2d>> + Send + Sync>;
pub type HdVec3dArrayDataSourceHandle =
    Arc<dyn crate::data_source::HdTypedSampledDataSource<Array<Vec3d>> + Send + Sync>;

/// Schema for NURBS patch trim curve (counts, orders, vertexCounts, knots, ranges, points).
#[derive(Debug, Clone)]
pub struct HdNurbsPatchTrimCurveSchema {
    schema: HdSchema,
}

impl HdNurbsPatchTrimCurveSchema {
    /// Create from container.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Get from parent container (looks up "trimCurve" child).
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&TRIM_CURVE) {
            if let Some(container) = cast_to_container(&child) {
                return Self::new(container);
            }
        }
        Self {
            schema: HdSchema::empty(),
        }
    }

    /// Get counts.
    pub fn get_counts(&self) -> Option<HdIntArrayDataSourceHandle> {
        self.schema.get_typed(&COUNTS)
    }

    /// Get orders.
    pub fn get_orders(&self) -> Option<HdIntArrayDataSourceHandle> {
        self.schema.get_typed(&ORDERS)
    }

    /// Get vertex counts.
    pub fn get_vertex_counts(&self) -> Option<HdIntArrayDataSourceHandle> {
        self.schema.get_typed(&VERTEX_COUNTS)
    }

    /// Get knots.
    pub fn get_knots(&self) -> Option<HdDoubleArrayDataSourceHandle> {
        self.schema.get_typed(&KNOTS)
    }

    /// Get ranges.
    pub fn get_ranges(&self) -> Option<HdVec2dArrayDataSourceHandle> {
        self.schema.get_typed(&RANGES)
    }

    /// Get points.
    pub fn get_points(&self) -> Option<HdVec3dArrayDataSourceHandle> {
        self.schema.get_typed(&POINTS)
    }

    /// Schema token.
    pub fn get_schema_token() -> &'static Lazy<Token> {
        &TRIM_CURVE
    }
}
