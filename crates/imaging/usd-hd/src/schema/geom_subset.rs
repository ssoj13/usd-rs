//! Geometry subset schema for Hydra.

use super::HdSchema;
use crate::data_source::HdDataSourceLocator;
use crate::data_source::{
    HdContainerDataSourceHandle, HdTypedSampledDataSource, cast_to_container,
};
use once_cell::sync::Lazy;
use std::sync::Arc;
use usd_tf::Token;
use usd_vt::Array;

/// Schema token: "geomSubset".
pub static GEOM_SUBSET: Lazy<Token> = Lazy::new(|| Token::new("geomSubset"));
/// Member token: "type".
pub static TYPE: Lazy<Token> = Lazy::new(|| Token::new("type"));
/// Member token: "indices".
pub static INDICES: Lazy<Token> = Lazy::new(|| Token::new("indices"));
/// Token for face set type (subset of faces).
#[allow(dead_code)]
pub static TYPE_FACE_SET: Lazy<Token> = Lazy::new(|| Token::new("typeFaceSet"));
/// Token for point set type (subset of points).
#[allow(dead_code)]
pub static TYPE_POINT_SET: Lazy<Token> = Lazy::new(|| Token::new("typePointSet"));
/// Token for curve set type (subset of curves).
#[allow(dead_code)]
pub static TYPE_CURVE_SET: Lazy<Token> = Lazy::new(|| Token::new("typeCurveSet"));

pub type HdTokenDataSourceHandle = Arc<dyn HdTypedSampledDataSource<Token> + Send + Sync>;
pub type HdIntArrayDataSourceHandle = Arc<dyn HdTypedSampledDataSource<Array<i32>> + Send + Sync>;

/// Schema for geometry subsets.
///
/// Geometry subsets define named groups of faces, points, or curves within
/// a geometry primitive. They are commonly used for material assignments
/// or other per-subset operations.
#[derive(Debug, Clone)]
pub struct HdGeomSubsetSchema {
    schema: HdSchema,
}

impl HdGeomSubsetSchema {
    /// Creates a new geometry subset schema from a container data source.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Retrieves the schema from a parent container data source.
    ///
    /// Looks for a child container under the `geomSubset` token.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&GEOM_SUBSET) {
            if let Some(container) = cast_to_container(&child) {
                return Self::new(container);
            }
        }
        Self {
            schema: HdSchema::empty(),
        }
    }

    /// Checks if the schema is defined (has a valid container).
    pub fn is_defined(&self) -> bool {
        self.schema.is_defined()
    }

    /// Returns the underlying container data source.
    pub fn get_container(&self) -> Option<&HdContainerDataSourceHandle> {
        self.schema.get_container()
    }

    /// Returns the subset type (typeFaceSet, typePointSet, or typeCurveSet).
    pub fn get_type(&self) -> Option<HdTokenDataSourceHandle> {
        self.schema.get_typed(&TYPE)
    }

    /// Returns the array of indices defining this subset.
    pub fn get_indices(&self) -> Option<HdIntArrayDataSourceHandle> {
        self.schema.get_typed(&INDICES)
    }

    /// Returns the schema's identifying token.
    pub fn get_schema_token() -> &'static Lazy<Token> {
        &GEOM_SUBSET
    }

    /// Returns the indices field token.
    pub fn get_indices_token() -> &'static Lazy<Token> {
        &INDICES
    }

    /// Returns the type field token.
    pub fn get_type_token() -> &'static Lazy<Token> {
        &TYPE
    }

    /// Returns the default data source locator for this schema.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[GEOM_SUBSET.clone()])
    }

    /// Builds a retained container data source with the specified subset data.
    ///
    /// This is a factory method that constructs a container with geometry subset information.
    pub fn build_retained(
        typ: Option<HdTokenDataSourceHandle>,
        indices: Option<HdIntArrayDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        use crate::data_source::{HdDataSourceBaseHandle, HdRetainedContainerDataSource};

        let mut entries = Vec::new();
        if let Some(t) = typ {
            entries.push((TYPE.clone(), t as HdDataSourceBaseHandle));
        }
        if let Some(i) = indices {
            entries.push((INDICES.clone(), i as HdDataSourceBaseHandle));
        }

        HdRetainedContainerDataSource::from_entries(&entries)
    }
}
