//! Basis curves topology schema for Hydra.

use super::HdSchema;
use crate::data_source::HdDataSourceLocator;
use crate::data_source::{
    HdContainerDataSourceHandle, HdTypedSampledDataSource, cast_to_container,
};
use once_cell::sync::Lazy;
use std::sync::Arc;
use usd_tf::Token;
use usd_vt::Array;

/// Schema token: "topology".
pub static TOPOLOGY: Lazy<Token> = Lazy::new(|| Token::new("topology"));
/// Member token: "curveVertexCounts".
pub static CURVE_VERTEX_COUNTS: Lazy<Token> = Lazy::new(|| Token::new("curveVertexCounts"));
/// Member token: "curveIndices".
pub static CURVE_INDICES: Lazy<Token> = Lazy::new(|| Token::new("curveIndices"));
/// Member token: "basis".
pub static BASIS: Lazy<Token> = Lazy::new(|| Token::new("basis"));
/// Member token: "type".
pub static TYPE: Lazy<Token> = Lazy::new(|| Token::new("type"));
/// Member token: "wrap".
pub static WRAP: Lazy<Token> = Lazy::new(|| Token::new("wrap"));

/// Handle to int array data source.
pub type HdIntArrayDataSourceHandle = Arc<dyn HdTypedSampledDataSource<Array<i32>> + Send + Sync>;
/// Handle to token data source.
pub type HdTokenDataSourceHandle = Arc<dyn HdTypedSampledDataSource<Token> + Send + Sync>;

/// Schema for basis curves topology data.
///
/// Provides structured access to curve topology including vertex counts,
/// indices, basis type, curve type, and wrap mode. Used for rendering
/// hair, fur, and other curve-based geometry.
#[derive(Debug, Clone)]
pub struct HdBasisCurvesTopologySchema {
    schema: HdSchema,
}

impl HdBasisCurvesTopologySchema {
    /// Creates a new basis curves topology schema from a container data source.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Retrieves the schema from a parent container data source.
    ///
    /// Looks for a child container under the `topology` token.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&TOPOLOGY) {
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

    /// Returns the array of vertex counts per curve.
    pub fn get_curve_vertex_counts(&self) -> Option<HdIntArrayDataSourceHandle> {
        self.schema.get_typed_retained(&CURVE_VERTEX_COUNTS)
    }

    /// Returns the array of vertex indices for curves.
    pub fn get_curve_indices(&self) -> Option<HdIntArrayDataSourceHandle> {
        self.schema.get_typed_retained(&CURVE_INDICES)
    }

    /// Returns the curve basis type (e.g., bezier, bspline, catmullRom).
    pub fn get_basis(&self) -> Option<HdTokenDataSourceHandle> {
        self.schema.get_typed_retained(&BASIS)
    }

    /// Returns the curve type (e.g., cubic, linear).
    pub fn get_type(&self) -> Option<HdTokenDataSourceHandle> {
        self.schema.get_typed_retained(&TYPE)
    }

    /// Returns the wrap mode (e.g., nonperiodic, periodic, pinned).
    pub fn get_wrap(&self) -> Option<HdTokenDataSourceHandle> {
        self.schema.get_typed_retained(&WRAP)
    }

    /// Returns the schema's identifying token.
    pub fn get_schema_token() -> &'static Lazy<Token> {
        &TOPOLOGY
    }

    /// Returns the default data source locator for this schema.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[TOPOLOGY.clone()])
    }

    /// Builds a retained container data source with the specified topology fields.
    ///
    /// This is a factory method that constructs a container with basis curves topology data.
    pub fn build_retained(
        curve_vertex_counts: Option<HdIntArrayDataSourceHandle>,
        curve_indices: Option<HdIntArrayDataSourceHandle>,
        basis: Option<HdTokenDataSourceHandle>,
        typ: Option<HdTokenDataSourceHandle>,
        wrap: Option<HdTokenDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        use crate::data_source::{HdDataSourceBaseHandle, HdRetainedContainerDataSource};

        let mut entries = Vec::new();

        if let Some(c) = curve_vertex_counts {
            entries.push((CURVE_VERTEX_COUNTS.clone(), c as HdDataSourceBaseHandle));
        }
        if let Some(i) = curve_indices {
            entries.push((CURVE_INDICES.clone(), i as HdDataSourceBaseHandle));
        }
        if let Some(b) = basis {
            entries.push((BASIS.clone(), b as HdDataSourceBaseHandle));
        }
        if let Some(t) = typ {
            entries.push((TYPE.clone(), t as HdDataSourceBaseHandle));
        }
        if let Some(w) = wrap {
            entries.push((WRAP.clone(), w as HdDataSourceBaseHandle));
        }

        HdRetainedContainerDataSource::from_entries(&entries)
    }
}
