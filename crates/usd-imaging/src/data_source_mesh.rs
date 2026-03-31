//! Mesh data source for USD imaging.
//!
//! Port of pxr/usdImaging/usdImaging/dataSourceMesh.h/cpp
//!
//! Provides specialized data sources for UsdGeomMesh prims including
//! topology, subdivision tags, and mesh-specific primvars.

use super::data_source_attribute::DataSourceAttribute;
use super::data_source_gprim::DataSourceGprim;
use super::data_source_stage_globals::DataSourceStageGlobalsHandle;
use super::types::PropertyInvalidationType;
use std::sync::Arc;
use usd_core::Prim;
use usd_geom::mesh::Mesh;
use usd_hd::{
    HdContainerDataSource, HdDataSourceBase, HdDataSourceBaseHandle, HdDataSourceLocator,
    HdDataSourceLocatorSet,
};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Array;

// Token constants for mesh data source
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    // Mesh schema tokens
    pub static MESH: LazyLock<Token> = LazyLock::new(|| Token::new("mesh"));
    pub static TOPOLOGY: LazyLock<Token> = LazyLock::new(|| Token::new("topology"));
    pub static SUBDIVISION_SCHEME: LazyLock<Token> =
        LazyLock::new(|| Token::new("subdivisionScheme"));
    pub static DOUBLE_SIDED: LazyLock<Token> = LazyLock::new(|| Token::new("doubleSided"));
    pub static SUBDIVISION_TAGS: LazyLock<Token> = LazyLock::new(|| Token::new("subdivisionTags"));

    // Topology tokens
    pub static FACE_VERTEX_COUNTS: LazyLock<Token> =
        LazyLock::new(|| Token::new("faceVertexCounts"));
    pub static FACE_VERTEX_INDICES: LazyLock<Token> =
        LazyLock::new(|| Token::new("faceVertexIndices"));
    pub static HOLE_INDICES: LazyLock<Token> = LazyLock::new(|| Token::new("holeIndices"));
    pub static ORIENTATION: LazyLock<Token> = LazyLock::new(|| Token::new("orientation"));

    // Subdivision tags tokens
    pub static CORNER_INDICES: LazyLock<Token> = LazyLock::new(|| Token::new("cornerIndices"));
    pub static CORNER_SHARPNESSES: LazyLock<Token> =
        LazyLock::new(|| Token::new("cornerSharpnesses"));
    pub static CREASE_INDICES: LazyLock<Token> = LazyLock::new(|| Token::new("creaseIndices"));
    pub static CREASE_LENGTHS: LazyLock<Token> = LazyLock::new(|| Token::new("creaseLengths"));
    pub static CREASE_SHARPNESSES: LazyLock<Token> =
        LazyLock::new(|| Token::new("creaseSharpnesses"));
    pub static INTERPOLATE_BOUNDARY: LazyLock<Token> =
        LazyLock::new(|| Token::new("interpolateBoundary"));
    pub static FACE_VARYING_LINEAR_INTERPOLATION: LazyLock<Token> =
        LazyLock::new(|| Token::new("faceVaryingLinearInterpolation"));
    pub static TRIANGLE_SUBDIVISION_RULE: LazyLock<Token> =
        LazyLock::new(|| Token::new("triangleSubdivisionRule"));
}

// ============================================================================
// DataSourceSubdivisionTags
// ============================================================================

/// Data source for subdivision tags.
///
/// Contains subdivision surface parameters like corner/crease sharpness,
/// boundary interpolation, and face-varying linear interpolation.
/// Reads directly from UsdGeomMesh attributes.
#[derive(Clone)]
pub struct DataSourceSubdivisionTags {
    mesh: Mesh,
    stage_globals: DataSourceStageGlobalsHandle,
}

impl std::fmt::Debug for DataSourceSubdivisionTags {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceSubdivisionTags")
            .field("prim", self.mesh.prim())
            .finish()
    }
}

impl DataSourceSubdivisionTags {
    /// Create new subdivision tags data source.
    pub fn new(mesh: Mesh, stage_globals: DataSourceStageGlobalsHandle) -> Arc<Self> {
        Arc::new(Self {
            mesh,
            stage_globals,
        })
    }
}

impl HdDataSourceBase for DataSourceSubdivisionTags {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for DataSourceSubdivisionTags {
    fn get_names(&self) -> Vec<Token> {
        vec![
            tokens::FACE_VARYING_LINEAR_INTERPOLATION.clone(),
            tokens::INTERPOLATE_BOUNDARY.clone(),
            tokens::TRIANGLE_SUBDIVISION_RULE.clone(),
            tokens::CORNER_INDICES.clone(),
            tokens::CORNER_SHARPNESSES.clone(),
            tokens::CREASE_INDICES.clone(),
            tokens::CREASE_LENGTHS.clone(),
            tokens::CREASE_SHARPNESSES.clone(),
        ]
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        // Read subdivision tag attributes from UsdGeomMesh and wrap
        // as DataSourceAttribute for Hydra consumption.
        let attr = if *name == *tokens::FACE_VARYING_LINEAR_INTERPOLATION {
            self.mesh.get_face_varying_linear_interpolation_attr()
        } else if *name == *tokens::INTERPOLATE_BOUNDARY {
            self.mesh.get_interpolate_boundary_attr()
        } else if *name == *tokens::TRIANGLE_SUBDIVISION_RULE {
            self.mesh.get_triangle_subdivision_rule_attr()
        } else if *name == *tokens::CORNER_INDICES {
            self.mesh.get_corner_indices_attr()
        } else if *name == *tokens::CORNER_SHARPNESSES {
            self.mesh.get_corner_sharpnesses_attr()
        } else if *name == *tokens::CREASE_INDICES {
            self.mesh.get_crease_indices_attr()
        } else if *name == *tokens::CREASE_LENGTHS {
            self.mesh.get_crease_lengths_attr()
        } else if *name == *tokens::CREASE_SHARPNESSES {
            self.mesh.get_crease_sharpnesses_attr()
        } else {
            return None;
        };

        if !attr.is_valid() {
            return None;
        }

        // C++ uses typed instantiation per field:
        //   cornerIndices/creaseIndices/creaseLengths -> DataSourceAttribute<VtIntArray>
        //   cornerSharpnesses/creaseSharpnesses -> DataSourceAttribute<VtFloatArray>
        //   faceVaryingLinear/interpolateBoundary/triangleSubdiv -> DataSourceAttribute<TfToken>
        if *name == *tokens::CORNER_INDICES
            || *name == *tokens::CREASE_INDICES
            || *name == *tokens::CREASE_LENGTHS
        {
            Some(DataSourceAttribute::<Array<i32>>::new(
                attr,
                self.stage_globals.clone(),
                Path::empty(),
            ) as HdDataSourceBaseHandle)
        } else if *name == *tokens::CORNER_SHARPNESSES || *name == *tokens::CREASE_SHARPNESSES {
            Some(DataSourceAttribute::<Array<f32>>::new(
                attr,
                self.stage_globals.clone(),
                Path::empty(),
            ) as HdDataSourceBaseHandle)
        } else {
            // Token fields: faceVaryingLinearInterpolation, interpolateBoundary,
            // triangleSubdivisionRule
            Some(DataSourceAttribute::<usd_tf::Token>::new(
                attr,
                self.stage_globals.clone(),
                Path::empty(),
            ) as HdDataSourceBaseHandle)
        }
    }
}

// ============================================================================
// DataSourceMeshTopology
// ============================================================================

/// Data source for mesh topology.
///
/// Contains face counts, vertex indices, hole indices, and orientation.
/// Reads from UsdGeomMesh attributes with locator tracking for invalidation.
#[derive(Clone)]
pub struct DataSourceMeshTopology {
    scene_index_path: Path,
    mesh: Mesh,
    stage_globals: DataSourceStageGlobalsHandle,
}

impl std::fmt::Debug for DataSourceMeshTopology {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceMeshTopology")
            .field("scene_index_path", &self.scene_index_path)
            .field("prim", self.mesh.prim())
            .finish()
    }
}

impl DataSourceMeshTopology {
    /// Create new mesh topology data source.
    pub fn new(
        scene_index_path: Path,
        mesh: Mesh,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Arc<Self> {
        Arc::new(Self {
            scene_index_path,
            mesh,
            stage_globals,
        })
    }
}

impl HdDataSourceBase for DataSourceMeshTopology {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for DataSourceMeshTopology {
    fn get_names(&self) -> Vec<Token> {
        vec![
            tokens::FACE_VERTEX_COUNTS.clone(),
            tokens::FACE_VERTEX_INDICES.clone(),
            tokens::HOLE_INDICES.clone(),
            tokens::ORIENTATION.clone(),
        ]
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        // Read topology attributes from UsdGeomMesh. faceVertexCounts,
        // faceVertexIndices, and holeIndices get scene path for locator
        // tracking (change tracking). Orientation is simpler (no locator).
        let attr = if *name == *tokens::FACE_VERTEX_COUNTS {
            self.mesh.get_face_vertex_counts_attr()
        } else if *name == *tokens::FACE_VERTEX_INDICES {
            self.mesh.get_face_vertex_indices_attr()
        } else if *name == *tokens::HOLE_INDICES {
            self.mesh.get_hole_indices_attr()
        } else if *name == *tokens::ORIENTATION {
            // Orientation comes from Gprim base (inherited)
            self.mesh
                .prim()
                .get_attribute("orientation")
                .unwrap_or_else(usd_core::Attribute::invalid)
        } else {
            return None;
        };

        if !attr.is_valid() {
            return None;
        }

        // C++ uses typed instantiation per field:
        //   faceVertexCounts/faceVertexIndices/holeIndices -> DataSourceAttribute<VtIntArray>
        //   orientation -> DataSourceAttribute<TfToken>
        if *name == *tokens::ORIENTATION {
            Some(DataSourceAttribute::<usd_tf::Token>::new(
                attr,
                self.stage_globals.clone(),
                self.scene_index_path.clone(),
            ) as HdDataSourceBaseHandle)
        } else {
            Some(DataSourceAttribute::<Array<i32>>::new(
                attr,
                self.stage_globals.clone(),
                self.scene_index_path.clone(),
            ) as HdDataSourceBaseHandle)
        }
    }
}

// ============================================================================
// DataSourceMesh
// ============================================================================

/// Data source representing data unique to meshes.
///
/// Contains topology, subdivisionScheme, doubleSided, and subdivisionTags.
/// Matches C++ UsdImagingDataSourceMesh.
#[derive(Clone)]
pub struct DataSourceMesh {
    scene_index_path: Path,
    mesh: Mesh,
    stage_globals: DataSourceStageGlobalsHandle,
}

impl std::fmt::Debug for DataSourceMesh {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceMesh")
            .field("scene_index_path", &self.scene_index_path)
            .field("prim", self.mesh.prim())
            .finish()
    }
}

impl DataSourceMesh {
    /// Create new mesh data source.
    pub fn new(
        scene_index_path: Path,
        mesh: Mesh,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Arc<Self> {
        Arc::new(Self {
            scene_index_path,
            mesh,
            stage_globals,
        })
    }
}

impl HdDataSourceBase for DataSourceMesh {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for DataSourceMesh {
    fn get_names(&self) -> Vec<Token> {
        // Matches C++ HdMeshSchemaTokens: topology, subdivisionScheme,
        // doubleSided, subdivisionTags
        vec![
            tokens::TOPOLOGY.clone(),
            tokens::SUBDIVISION_SCHEME.clone(),
            tokens::DOUBLE_SIDED.clone(),
            tokens::SUBDIVISION_TAGS.clone(),
        ]
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if *name == *tokens::TOPOLOGY {
            return Some(DataSourceMeshTopology::new(
                self.scene_index_path.clone(),
                self.mesh.clone(),
                self.stage_globals.clone(),
            ) as HdDataSourceBaseHandle);
        }

        if *name == *tokens::SUBDIVISION_SCHEME {
            let attr = self.mesh.get_subdivision_scheme_attr();
            if attr.is_valid() {
                // C++: DataSourceAttribute<TfToken>::New(...)
                return Some(DataSourceAttribute::<usd_tf::Token>::new(
                    attr,
                    self.stage_globals.clone(),
                    Path::empty(),
                ) as HdDataSourceBaseHandle);
            }
            return None;
        }

        if *name == *tokens::DOUBLE_SIDED {
            if let Some(attr) = self.mesh.prim().get_attribute("doubleSided") {
                // C++: DataSourceAttribute<bool>::New(...)
                return Some(DataSourceAttribute::<bool>::new(
                    attr,
                    self.stage_globals.clone(),
                    Path::empty(),
                ) as HdDataSourceBaseHandle);
            }
            return None;
        }

        if *name == *tokens::SUBDIVISION_TAGS {
            return Some(DataSourceSubdivisionTags::new(
                self.mesh.clone(),
                self.stage_globals.clone(),
            ) as HdDataSourceBaseHandle);
        }

        None
    }
}

// ============================================================================
// DataSourceMeshPrim
// ============================================================================

/// Prim data source representing UsdGeomMesh.
///
/// Extends DataSourceGprim with mesh-specific "mesh" container.
/// Matches C++ UsdImagingDataSourceMeshPrim.
#[derive(Clone)]
pub struct DataSourceMeshPrim {
    /// Base gprim data source (handles primvars, xform, visibility, etc.)
    gprim_ds: Arc<DataSourceGprim>,
    /// Scene index path
    scene_index_path: Path,
    /// Prim reference for creating mesh data source on demand
    prim: Prim,
    /// Stage globals
    stage_globals: DataSourceStageGlobalsHandle,
}

impl std::fmt::Debug for DataSourceMeshPrim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceMeshPrim")
            .field("scene_index_path", &self.scene_index_path)
            .finish()
    }
}

impl DataSourceMeshPrim {
    /// Create new mesh prim data source.
    pub fn new(
        scene_index_path: Path,
        prim: Prim,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Arc<Self> {
        let gprim_ds = DataSourceGprim::new(
            scene_index_path.clone(),
            prim.clone(),
            stage_globals.clone(),
        );

        Arc::new(Self {
            gprim_ds,
            scene_index_path,
            prim,
            stage_globals,
        })
    }

    /// Compute invalidation for property changes.
    ///
    /// Maps USD property names to Hydra locators following C++ logic exactly:
    /// - subdivisionScheme -> mesh/subdivisionScheme
    /// - topology attrs -> mesh/topology
    /// - subdivision tag attrs -> mesh/subdivisionTags
    /// - doubleSided -> mesh/doubleSided
    pub fn invalidate(
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        let mut locators = HdDataSourceLocatorSet::empty();

        for prop in properties {
            let prop_str = prop.as_str();

            // subdivisionScheme gets its own locator (separate from tags)
            if prop_str == "subdivisionScheme" {
                locators.insert(HdDataSourceLocator::from_tokens_2(
                    Token::new("mesh"),
                    Token::new("subdivisionScheme"),
                ));
            }

            // Topology attributes
            if prop_str == "faceVertexCounts"
                || prop_str == "faceVertexIndices"
                || prop_str == "holeIndices"
                || prop_str == "orientation"
            {
                locators.insert(HdDataSourceLocator::from_tokens_2(
                    Token::new("mesh"),
                    Token::new("topology"),
                ));
            }

            // Subdivision tag attributes
            if prop_str == "interpolateBoundary"
                || prop_str == "faceVaryingLinearInterpolation"
                || prop_str == "triangleSubdivisionRule"
                || prop_str == "creaseIndices"
                || prop_str == "creaseLengths"
                || prop_str == "creaseSharpnesses"
                || prop_str == "cornerIndices"
                || prop_str == "cornerSharpnesses"
            {
                locators.insert(HdDataSourceLocator::from_tokens_2(
                    Token::new("mesh"),
                    Token::new("subdivisionTags"),
                ));
            }

            // doubleSided
            if prop_str == "doubleSided" {
                locators.insert(HdDataSourceLocator::from_tokens_2(
                    Token::new("mesh"),
                    Token::new("doubleSided"),
                ));
            }
        }

        // Give base class a chance to invalidate
        let base = DataSourceGprim::invalidate(prim, subprim, properties, invalidation_type);
        locators.insert_set(&base);

        locators
    }
}

impl HdDataSourceBase for DataSourceMeshPrim {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for DataSourceMeshPrim {
    fn get_names(&self) -> Vec<Token> {
        let mut names = self.gprim_ds.get_names();
        names.push(tokens::MESH.clone());
        names
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        // Mesh-specific container: create DataSourceMesh on demand
        if *name == *tokens::MESH {
            let mesh = Mesh::new(self.prim.clone());
            return Some(DataSourceMesh::new(
                self.scene_index_path.clone(),
                mesh,
                self.stage_globals.clone(),
            ) as HdDataSourceBaseHandle);
        }

        // Delegate everything else to gprim
        self.gprim_ds.get(name)
    }
}

/// Handle type for DataSourceMeshPrim
pub type DataSourceMeshPrimHandle = Arc<DataSourceMeshPrim>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source_stage_globals::NoOpStageGlobals;
    use usd_core::Stage;

    fn create_test_globals() -> DataSourceStageGlobalsHandle {
        Arc::new(NoOpStageGlobals::default())
    }

    #[test]
    fn test_data_source_mesh_prim_creation() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let path = Path::from_string("/TestMesh").unwrap();
        let globals = create_test_globals();

        let ds = DataSourceMeshPrim::new(path.clone(), prim, globals);
        let names = ds.get_names();

        // Should have mesh data source
        assert!(names.iter().any(|n| n == "mesh"));
        // Should also have gprim data sources
        assert!(names.iter().any(|n| n == "primvars"));
    }

    #[test]
    fn test_mesh_data_source_names() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let path = Path::from_string("/TestMesh").unwrap();
        let globals = create_test_globals();
        let mesh = Mesh::new(prim);

        let ds = DataSourceMesh::new(path, mesh, globals);
        let names = ds.get_names();

        // Matches C++ HdMeshSchemaTokens
        assert!(names.iter().any(|n| n == "topology"));
        assert!(names.iter().any(|n| n == "subdivisionScheme"));
        assert!(names.iter().any(|n| n == "doubleSided"));
        assert!(names.iter().any(|n| n == "subdivisionTags"));
    }

    #[test]
    fn test_mesh_topology_data_source() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let path = Path::from_string("/TestMesh").unwrap();
        let globals = create_test_globals();
        let mesh = Mesh::new(prim);

        let ds = DataSourceMeshTopology::new(path, mesh, globals);
        let names = ds.get_names();

        assert!(names.iter().any(|n| n == "faceVertexCounts"));
        assert!(names.iter().any(|n| n == "faceVertexIndices"));
        assert!(names.iter().any(|n| n == "holeIndices"));
        assert!(names.iter().any(|n| n == "orientation"));
    }

    #[test]
    fn test_subdivision_tags_names() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let globals = create_test_globals();
        let mesh = Mesh::new(prim);

        let ds = DataSourceSubdivisionTags::new(mesh, globals);
        let names = ds.get_names();

        assert!(names.iter().any(|n| n == "faceVaryingLinearInterpolation"));
        assert!(names.iter().any(|n| n == "interpolateBoundary"));
        assert!(names.iter().any(|n| n == "cornerIndices"));
        assert!(names.iter().any(|n| n == "creaseSharpnesses"));
    }

    #[test]
    fn test_mesh_get_returns_topology() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let path = Path::from_string("/TestMesh").unwrap();
        let globals = create_test_globals();
        let mesh = Mesh::new(prim);

        let ds = DataSourceMesh::new(path, mesh, globals);
        let topology_token = Token::new("topology");

        // Should return a topology container
        let result = ds.get(&topology_token);
        assert!(result.is_some());
    }

    #[test]
    fn test_mesh_prim_get_delegates_to_gprim() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let path = Path::from_string("/TestMesh").unwrap();
        let globals = create_test_globals();

        let ds = DataSourceMeshPrim::new(path, prim, globals);
        let mesh_token = Token::new("mesh");

        // "mesh" should return the mesh container
        let result = ds.get(&mesh_token);
        assert!(result.is_some());
    }

    #[test]
    fn test_mesh_invalidation_separation() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        // subdivisionScheme should NOT trigger subdivisionTags locator
        let props = vec![Token::new("subdivisionScheme")];
        let locators = DataSourceMeshPrim::invalidate(
            &prim,
            &Token::new(""),
            &props,
            PropertyInvalidationType::PropertyChanged,
        );
        assert!(!locators.is_empty());

        // Topology properties should trigger mesh/topology
        let props = vec![Token::new("faceVertexCounts"), Token::new("points")];
        let locators = DataSourceMeshPrim::invalidate(
            &prim,
            &Token::new(""),
            &props,
            PropertyInvalidationType::PropertyChanged,
        );
        assert!(!locators.is_empty());
    }
}
