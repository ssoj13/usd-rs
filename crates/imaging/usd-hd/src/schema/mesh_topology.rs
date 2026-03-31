//! Mesh topology schema for Hydra.

use super::HdSchema;
use crate::data_source::HdDataSourceLocator;
use crate::data_source::{
    HdContainerDataSourceHandle, HdTypedSampledDataSource, cast_to_container,
};
use once_cell::sync::Lazy;
use std::sync::Arc;
use usd_tf::Token;
use usd_vt::Array;

pub static TOPOLOGY: Lazy<Token> = Lazy::new(|| Token::new("topology"));
pub static FACE_VERTEX_COUNTS: Lazy<Token> = Lazy::new(|| Token::new("faceVertexCounts"));
pub static FACE_VERTEX_INDICES: Lazy<Token> = Lazy::new(|| Token::new("faceVertexIndices"));
pub static HOLE_INDICES: Lazy<Token> = Lazy::new(|| Token::new("holeIndices"));
pub static ORIENTATION: Lazy<Token> = Lazy::new(|| Token::new("orientation"));
/// Token for left-handed mesh orientation.
#[allow(dead_code)]
pub static LEFT_HANDED: Lazy<Token> = Lazy::new(|| Token::new("leftHanded"));
/// Token for right-handed mesh orientation.
#[allow(dead_code)]
pub static RIGHT_HANDED: Lazy<Token> = Lazy::new(|| Token::new("rightHanded"));

/// Handle to int array data source.
pub type HdIntArrayDataSourceHandle = Arc<dyn HdTypedSampledDataSource<Array<i32>> + Send + Sync>;
/// Handle to token data source.
pub type HdTokenDataSourceHandle = Arc<dyn HdTypedSampledDataSource<Token> + Send + Sync>;

/// Schema for mesh topology data.
///
/// Provides structured access to mesh topology including face vertex counts,
/// indices, hole polygons, and orientation.
#[derive(Debug, Clone)]
pub struct HdMeshTopologySchema {
    schema: HdSchema,
}

impl HdMeshTopologySchema {
    /// Creates a new mesh topology schema from a container data source.
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

    /// Returns the array of vertex counts per face.
    pub fn get_face_vertex_counts(&self) -> Option<HdIntArrayDataSourceHandle> {
        self.schema
            .get_typed_retained::<Array<i32>>(&FACE_VERTEX_COUNTS)
    }

    /// Returns the array mapping faces to vertices.
    pub fn get_face_vertex_indices(&self) -> Option<HdIntArrayDataSourceHandle> {
        self.schema
            .get_typed_retained::<Array<i32>>(&FACE_VERTEX_INDICES)
    }

    /// Returns the array identifying hole polygons.
    pub fn get_hole_indices(&self) -> Option<HdIntArrayDataSourceHandle> {
        self.schema.get_typed_retained::<Array<i32>>(&HOLE_INDICES)
    }

    /// Returns the mesh orientation (leftHanded or rightHanded).
    pub fn get_orientation(&self) -> Option<HdTokenDataSourceHandle> {
        self.schema.get_typed_retained::<Token>(&ORIENTATION)
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
    /// This is a factory method that constructs a container with mesh topology data.
    pub fn build_retained(
        face_vertex_counts: Option<HdIntArrayDataSourceHandle>,
        face_vertex_indices: Option<HdIntArrayDataSourceHandle>,
        hole_indices: Option<HdIntArrayDataSourceHandle>,
        orientation: Option<HdTokenDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        use crate::data_source::{HdDataSourceBaseHandle, HdRetainedContainerDataSource};

        let mut entries = Vec::new();

        if let Some(c) = face_vertex_counts {
            entries.push((FACE_VERTEX_COUNTS.clone(), c as HdDataSourceBaseHandle));
        }
        if let Some(i) = face_vertex_indices {
            entries.push((FACE_VERTEX_INDICES.clone(), i as HdDataSourceBaseHandle));
        }
        if let Some(h) = hole_indices {
            entries.push((HOLE_INDICES.clone(), h as HdDataSourceBaseHandle));
        }
        if let Some(o) = orientation {
            entries.push((ORIENTATION.clone(), o as HdDataSourceBaseHandle));
        }

        HdRetainedContainerDataSource::from_entries(&entries)
    }
}
