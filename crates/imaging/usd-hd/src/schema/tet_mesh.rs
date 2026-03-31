#![allow(dead_code)]
//! Tetrahedral mesh schema for Hydra.
//!
//! Defines tetrahedral mesh geometry including topology and rendering flags.

use super::{HdSchema, HdTetMeshTopologySchema};
use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdDataSourceLocator,
    HdRetainedContainerDataSource, HdTypedSampledDataSource, cast_to_container,
};
use std::sync::Arc;
use std::sync::LazyLock;
use usd_tf::Token;

/// Tet mesh schema token
pub static TET_MESH: LazyLock<Token> = LazyLock::new(|| Token::new("tetMesh"));
/// Topology token
pub static TOPOLOGY: LazyLock<Token> = LazyLock::new(|| Token::new("topology"));
/// Double-sided rendering flag token
pub static DOUBLE_SIDED: LazyLock<Token> = LazyLock::new(|| Token::new("doubleSided"));

/// Data source for bool values
pub type HdBoolDataSource = dyn HdTypedSampledDataSource<bool>;
/// Arc handle to bool data source
pub type HdBoolDataSourceHandle = Arc<HdBoolDataSource>;

/// Schema representing tetrahedral mesh geometry.
///
/// Provides access to:
/// - `topology` - Tet mesh topology (tet indices, surface face indices, orientation)
/// - `doubleSided` - Whether mesh is double-sided
///
/// # Location
///
/// Default locator: `tetMesh`
#[derive(Debug, Clone)]
pub struct HdTetMeshSchema {
    schema: HdSchema,
}

impl HdTetMeshSchema {
    /// Constructs a tet mesh schema from a container data source.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Retrieves tet mesh schema from parent container at "tetMesh" locator.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&TET_MESH) {
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

    /// Gets tet mesh topology schema.
    pub fn get_topology(&self) -> Option<HdTetMeshTopologySchema> {
        if let Some(container) = self.schema.get_container() {
            let topo = HdTetMeshTopologySchema::get_from_parent(container);
            if topo.is_defined() {
                return Some(topo);
            }
        }
        None
    }

    /// Gets double-sided flag.
    pub fn get_double_sided(&self) -> Option<HdBoolDataSourceHandle> {
        self.schema.get_typed(&DOUBLE_SIDED)
    }

    /// Returns the schema token for tet mesh.
    pub fn get_schema_token() -> &'static LazyLock<Token> {
        &TET_MESH
    }

    /// Returns the default locator for tet mesh schema.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[TET_MESH.clone()])
    }

    /// Returns the locator for tet mesh topology.
    pub fn get_topology_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[TET_MESH.clone(), TOPOLOGY.clone()])
    }

    /// Returns the locator for double-sided flag.
    pub fn get_double_sided_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[TET_MESH.clone(), DOUBLE_SIDED.clone()])
    }

    /// Builds a retained container with tet mesh parameters.
    ///
    /// # Parameters
    /// All tet mesh settings as optional data source handles.
    pub fn build_retained(
        topology: Option<HdContainerDataSourceHandle>,
        double_sided: Option<HdBoolDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        let mut entries = Vec::new();

        if let Some(t) = topology {
            entries.push((TOPOLOGY.clone(), t as HdDataSourceBaseHandle));
        }
        if let Some(d) = double_sided {
            entries.push((DOUBLE_SIDED.clone(), d as HdDataSourceBaseHandle));
        }

        HdRetainedContainerDataSource::from_entries(&entries)
    }
}

/// Builder for HdTetMeshSchema.
///
/// Provides a fluent interface for constructing tet mesh schemas.
pub struct HdTetMeshSchemaBuilder {
    topology: Option<HdContainerDataSourceHandle>,
    double_sided: Option<HdBoolDataSourceHandle>,
}

impl HdTetMeshSchemaBuilder {
    /// Creates a new builder.
    pub fn new() -> Self {
        Self {
            topology: None,
            double_sided: None,
        }
    }

    /// Sets the topology.
    pub fn set_topology(mut self, topology: HdContainerDataSourceHandle) -> Self {
        self.topology = Some(topology);
        self
    }

    /// Sets the double-sided flag.
    pub fn set_double_sided(mut self, double_sided: HdBoolDataSourceHandle) -> Self {
        self.double_sided = Some(double_sided);
        self
    }

    /// Builds the container data source.
    pub fn build(self) -> HdContainerDataSourceHandle {
        HdTetMeshSchema::build_retained(self.topology, self.double_sided)
    }
}

impl Default for HdTetMeshSchemaBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tet_mesh_schema_empty() {
        let empty_container: HdContainerDataSourceHandle =
            HdRetainedContainerDataSource::from_entries(&[]);
        let schema = HdTetMeshSchema::get_from_parent(&empty_container);
        assert!(!schema.is_defined());
    }

    #[test]
    fn test_tet_mesh_schema_tokens() {
        assert_eq!(TET_MESH.as_str(), "tetMesh");
        assert_eq!(TOPOLOGY.as_str(), "topology");
        assert_eq!(DOUBLE_SIDED.as_str(), "doubleSided");
    }

    #[test]
    fn test_tet_mesh_schema_locators() {
        let default_loc = HdTetMeshSchema::get_default_locator();
        assert_eq!(default_loc.elements().len(), 1);

        let topology_loc = HdTetMeshSchema::get_topology_locator();
        assert_eq!(topology_loc.elements().len(), 2);

        let double_sided_loc = HdTetMeshSchema::get_double_sided_locator();
        assert_eq!(double_sided_loc.elements().len(), 2);
    }
}
