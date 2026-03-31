#![allow(dead_code)]
//! Tetrahedral mesh topology schema for Hydra.
//!
//! Defines tetrahedral mesh connectivity and orientation.

use super::HdSchema;
use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdDataSourceLocator,
    HdRetainedContainerDataSource, HdRetainedTypedSampledDataSource, HdTypedSampledDataSource,
    cast_to_container,
};
use std::sync::Arc;
use std::sync::LazyLock;
use usd_gf::{Vec3i, Vec4i};
use usd_tf::Token;

/// Topology token
pub static TOPOLOGY: LazyLock<Token> = LazyLock::new(|| Token::new("topology"));
/// Tet vertex indices token
pub static TET_VERTEX_INDICES: LazyLock<Token> = LazyLock::new(|| Token::new("tetVertexIndices"));
/// Surface face vertex indices token
pub static SURFACE_FACE_VERTEX_INDICES: LazyLock<Token> =
    LazyLock::new(|| Token::new("surfaceFaceVertexIndices"));
/// Orientation token
pub static ORIENTATION: LazyLock<Token> = LazyLock::new(|| Token::new("orientation"));
/// Left-handed orientation token
pub static LEFT_HANDED: LazyLock<Token> = LazyLock::new(|| Token::new("leftHanded"));
/// Right-handed orientation token
pub static RIGHT_HANDED: LazyLock<Token> = LazyLock::new(|| Token::new("rightHanded"));

/// Data source for Vec4i array values
pub type HdVec4iArrayDataSource = dyn HdTypedSampledDataSource<Vec<Vec4i>>;
/// Arc handle to Vec4i array data source
pub type HdVec4iArrayDataSourceHandle = Arc<HdVec4iArrayDataSource>;
/// Data source for Vec3i array values
pub type HdVec3iArrayDataSource = dyn HdTypedSampledDataSource<Vec<Vec3i>>;
/// Arc handle to Vec3i array data source
pub type HdVec3iArrayDataSourceHandle = Arc<HdVec3iArrayDataSource>;
/// Data source for Token values
pub type HdTokenDataSource = dyn HdTypedSampledDataSource<Token>;
/// Arc handle to Token data source
pub type HdTokenDataSourceHandle = Arc<HdTokenDataSource>;

/// Schema representing tetrahedral mesh topology.
///
/// Provides access to:
/// - `tetVertexIndices` - Vertex indices for each tetrahedron (4 indices per tet)
/// - `surfaceFaceVertexIndices` - Vertex indices for surface faces (3 indices per triangle)
/// - `orientation` - Handedness of the mesh (leftHanded or rightHanded)
///
/// # Location
///
/// Default locator: `topology`
#[derive(Debug, Clone)]
pub struct HdTetMeshTopologySchema {
    schema: HdSchema,
}

impl HdTetMeshTopologySchema {
    /// Constructs a tet mesh topology schema from a container data source.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Retrieves tet mesh topology schema from parent container at "topology" locator.
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

    /// Returns true if the schema is non-empty.
    pub fn is_defined(&self) -> bool {
        self.schema.is_defined()
    }

    /// Gets the underlying container data source.
    pub fn get_container(&self) -> Option<&HdContainerDataSourceHandle> {
        self.schema.get_container()
    }

    /// Gets tet vertex indices.
    pub fn get_tet_vertex_indices(&self) -> Option<HdVec4iArrayDataSourceHandle> {
        self.schema.get_typed(&TET_VERTEX_INDICES)
    }

    /// Gets surface face vertex indices.
    pub fn get_surface_face_vertex_indices(&self) -> Option<HdVec3iArrayDataSourceHandle> {
        self.schema.get_typed(&SURFACE_FACE_VERTEX_INDICES)
    }

    /// Gets orientation.
    pub fn get_orientation(&self) -> Option<HdTokenDataSourceHandle> {
        self.schema.get_typed(&ORIENTATION)
    }

    /// Returns the schema token for tet mesh topology.
    pub fn get_schema_token() -> &'static LazyLock<Token> {
        &TOPOLOGY
    }

    /// Returns the default locator for tet mesh topology schema.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[TOPOLOGY.clone()])
    }

    /// Returns the locator for tet vertex indices.
    pub fn get_tet_vertex_indices_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[TOPOLOGY.clone(), TET_VERTEX_INDICES.clone()])
    }

    /// Returns the locator for surface face vertex indices.
    pub fn get_surface_face_vertex_indices_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[TOPOLOGY.clone(), SURFACE_FACE_VERTEX_INDICES.clone()])
    }

    /// Returns the locator for orientation.
    pub fn get_orientation_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[TOPOLOGY.clone(), ORIENTATION.clone()])
    }

    /// Builds a retained container with tet mesh topology parameters.
    ///
    /// # Parameters
    /// All topology settings as optional data source handles.
    pub fn build_retained(
        tet_vertex_indices: Option<HdVec4iArrayDataSourceHandle>,
        surface_face_vertex_indices: Option<HdVec3iArrayDataSourceHandle>,
        orientation: Option<HdTokenDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        let mut entries = Vec::new();

        if let Some(tvi) = tet_vertex_indices {
            entries.push((TET_VERTEX_INDICES.clone(), tvi as HdDataSourceBaseHandle));
        }
        if let Some(sfvi) = surface_face_vertex_indices {
            entries.push((
                SURFACE_FACE_VERTEX_INDICES.clone(),
                sfvi as HdDataSourceBaseHandle,
            ));
        }
        if let Some(o) = orientation {
            entries.push((ORIENTATION.clone(), o as HdDataSourceBaseHandle));
        }

        HdRetainedContainerDataSource::from_entries(&entries)
    }

    /// Builds orientation data source from token.
    ///
    /// Creates a statically cached data source for common orientation values.
    pub fn build_orientation_data_source(orientation: &Token) -> HdTokenDataSourceHandle {
        HdRetainedTypedSampledDataSource::new(orientation.clone())
    }
}

/// Builder for HdTetMeshTopologySchema.
///
/// Provides a fluent interface for constructing tet mesh topology schemas.
pub struct HdTetMeshTopologySchemaBuilder {
    tet_vertex_indices: Option<HdVec4iArrayDataSourceHandle>,
    surface_face_vertex_indices: Option<HdVec3iArrayDataSourceHandle>,
    orientation: Option<HdTokenDataSourceHandle>,
}

impl HdTetMeshTopologySchemaBuilder {
    /// Creates a new builder.
    pub fn new() -> Self {
        Self {
            tet_vertex_indices: None,
            surface_face_vertex_indices: None,
            orientation: None,
        }
    }

    /// Sets the tet vertex indices.
    pub fn set_tet_vertex_indices(
        mut self,
        tet_vertex_indices: HdVec4iArrayDataSourceHandle,
    ) -> Self {
        self.tet_vertex_indices = Some(tet_vertex_indices);
        self
    }

    /// Sets the surface face vertex indices.
    pub fn set_surface_face_vertex_indices(
        mut self,
        surface_face_vertex_indices: HdVec3iArrayDataSourceHandle,
    ) -> Self {
        self.surface_face_vertex_indices = Some(surface_face_vertex_indices);
        self
    }

    /// Sets the orientation.
    pub fn set_orientation(mut self, orientation: HdTokenDataSourceHandle) -> Self {
        self.orientation = Some(orientation);
        self
    }

    /// Builds the container data source.
    pub fn build(self) -> HdContainerDataSourceHandle {
        HdTetMeshTopologySchema::build_retained(
            self.tet_vertex_indices,
            self.surface_face_vertex_indices,
            self.orientation,
        )
    }
}

impl Default for HdTetMeshTopologySchemaBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tet_mesh_topology_schema_empty() {
        let empty_container: HdContainerDataSourceHandle =
            HdRetainedContainerDataSource::from_entries(&[]);
        let schema = HdTetMeshTopologySchema::get_from_parent(&empty_container);
        assert!(!schema.is_defined());
    }

    #[test]
    fn test_tet_mesh_topology_schema_tokens() {
        assert_eq!(TOPOLOGY.as_str(), "topology");
        assert_eq!(TET_VERTEX_INDICES.as_str(), "tetVertexIndices");
        assert_eq!(
            SURFACE_FACE_VERTEX_INDICES.as_str(),
            "surfaceFaceVertexIndices"
        );
        assert_eq!(ORIENTATION.as_str(), "orientation");
        assert_eq!(LEFT_HANDED.as_str(), "leftHanded");
        assert_eq!(RIGHT_HANDED.as_str(), "rightHanded");
    }

    #[test]
    fn test_tet_mesh_topology_schema_locators() {
        let default_loc = HdTetMeshTopologySchema::get_default_locator();
        assert_eq!(default_loc.elements().len(), 1);

        let tet_indices_loc = HdTetMeshTopologySchema::get_tet_vertex_indices_locator();
        assert_eq!(tet_indices_loc.elements().len(), 2);

        let surface_indices_loc =
            HdTetMeshTopologySchema::get_surface_face_vertex_indices_locator();
        assert_eq!(surface_indices_loc.elements().len(), 2);
    }

    #[test]
    fn test_orientation_data_source() {
        let left_handed = HdTetMeshTopologySchema::build_orientation_data_source(&LEFT_HANDED);
        let value = left_handed.get_typed_value(0.0);
        assert_eq!(value.as_str(), "leftHanded");

        let right_handed = HdTetMeshTopologySchema::build_orientation_data_source(&RIGHT_HANDED);
        let value = right_handed.get_typed_value(0.0);
        assert_eq!(value.as_str(), "rightHanded");
    }
}
