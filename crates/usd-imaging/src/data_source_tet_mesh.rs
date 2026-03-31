//! Data sources for `UsdGeomTetMesh`.

use crate::data_source_attribute::DataSourceAttribute;
use crate::data_source_gprim::DataSourceGprim;
use crate::data_source_stage_globals::DataSourceStageGlobalsHandle;
use crate::types::PropertyInvalidationType;
use std::sync::Arc;
use usd_core::Prim;
use usd_geom::TetMesh;
use usd_hd::{
    HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
    HdDataSourceLocatorSet,
};
use usd_hd::schema::HdTetMeshSchema;
use usd_sdf::Path;
use usd_tf::Token;

mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static TET_MESH: LazyLock<Token> = LazyLock::new(|| Token::new("tetMesh"));
    pub static TOPOLOGY: LazyLock<Token> = LazyLock::new(|| Token::new("topology"));
    pub static ORIENTATION: LazyLock<Token> = LazyLock::new(|| Token::new("orientation"));
    pub static TET_VERTEX_INDICES: LazyLock<Token> =
        LazyLock::new(|| Token::new("tetVertexIndices"));
    pub static SURFACE_FACE_VERTEX_INDICES: LazyLock<Token> =
        LazyLock::new(|| Token::new("surfaceFaceVertexIndices"));
    pub static DOUBLE_SIDED: LazyLock<Token> = LazyLock::new(|| Token::new("doubleSided"));
}

#[derive(Clone)]
struct DataSourceTetMeshTopology {
    scene_index_path: Path,
    tet_mesh: TetMesh,
    stage_globals: DataSourceStageGlobalsHandle,
}

impl DataSourceTetMeshTopology {
    fn new(
        scene_index_path: Path,
        tet_mesh: TetMesh,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Self {
        Self {
            scene_index_path,
            tet_mesh,
            stage_globals,
        }
    }
}

impl std::fmt::Debug for DataSourceTetMeshTopology {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DataSourceTetMeshTopology")
    }
}

impl HdDataSourceBase for DataSourceTetMeshTopology {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for DataSourceTetMeshTopology {
    fn get_names(&self) -> Vec<Token> {
        vec![
            tokens::ORIENTATION.clone(),
            tokens::TET_VERTEX_INDICES.clone(),
            tokens::SURFACE_FACE_VERTEX_INDICES.clone(),
        ]
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if name == &*tokens::TET_VERTEX_INDICES {
            return Some(DataSourceAttribute::<Vec<usd_gf::Vec4i>>::new(
                self.tet_mesh.get_tet_vertex_indices_attr(),
                self.stage_globals.clone(),
                self.scene_index_path.clone(),
            ));
        }
        if name == &*tokens::SURFACE_FACE_VERTEX_INDICES {
            return Some(DataSourceAttribute::<Vec<usd_gf::Vec3i>>::new(
                self.tet_mesh.get_surface_face_vertex_indices_attr(),
                self.stage_globals.clone(),
                self.scene_index_path.clone(),
            ));
        }
        if name == &*tokens::ORIENTATION {
            return Some(DataSourceAttribute::<Token>::new(
                self.tet_mesh.point_based().gprim().get_orientation_attr(),
                self.stage_globals.clone(),
                self.scene_index_path.clone(),
            ));
        }
        None
    }
}

#[derive(Clone)]
pub struct DataSourceTetMesh {
    scene_index_path: Path,
    tet_mesh: TetMesh,
    stage_globals: DataSourceStageGlobalsHandle,
}

impl DataSourceTetMesh {
    pub fn new(
        scene_index_path: Path,
        prim: Prim,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Self {
        Self {
            scene_index_path,
            tet_mesh: TetMesh::new(prim),
            stage_globals,
        }
    }
}

impl std::fmt::Debug for DataSourceTetMesh {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DataSourceTetMesh")
    }
}

impl HdDataSourceBase for DataSourceTetMesh {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for DataSourceTetMesh {
    fn get_names(&self) -> Vec<Token> {
        vec![tokens::TOPOLOGY.clone(), tokens::DOUBLE_SIDED.clone()]
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if name == &*tokens::TOPOLOGY {
            return Some(Arc::new(DataSourceTetMeshTopology::new(
                self.scene_index_path.clone(),
                self.tet_mesh.clone(),
                self.stage_globals.clone(),
            )));
        }
        if name == &*tokens::DOUBLE_SIDED {
            return Some(DataSourceAttribute::<bool>::new(
                self.tet_mesh.point_based().gprim().get_double_sided_attr(),
                self.stage_globals.clone(),
                self.scene_index_path.clone(),
            ));
        }
        None
    }
}

pub type DataSourceTetMeshHandle = Arc<DataSourceTetMesh>;

#[derive(Clone)]
pub struct DataSourceTetMeshPrim {
    base: Arc<DataSourceGprim>,
    prim: Prim,
    stage_globals: DataSourceStageGlobalsHandle,
}

impl DataSourceTetMeshPrim {
    pub fn new(
        scene_index_path: Path,
        prim: Prim,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Self {
        Self {
            base: DataSourceGprim::new(
                scene_index_path.clone(),
                prim.clone(),
                stage_globals.clone(),
            ),
            prim,
            stage_globals,
        }
    }

    pub fn get_names(&self) -> Vec<Token> {
        let mut result = self.base.get_names();
        result.push(tokens::TET_MESH.clone());
        result
    }

    pub fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if name == &*tokens::TET_MESH {
            return Some(Arc::new(DataSourceTetMesh::new(
                self.base.scene_index_path().clone(),
                self.prim.clone(),
                self.stage_globals.clone(),
            )));
        }
        self.base.get(name)
    }

    pub fn invalidate(
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        let mut locators =
            DataSourceGprim::invalidate(prim, subprim, properties, invalidation_type);

        for property in properties {
            if property.as_str() == "tetVertexIndices"
                || property.as_str() == "surfaceFaceVertexIndices"
                || property.as_str() == "orientation"
            {
                locators.insert(HdTetMeshSchema::get_topology_locator());
            }
            if property.as_str() == "doubleSided" {
                locators.insert(HdTetMeshSchema::get_double_sided_locator());
            }
        }

        locators
    }
}

impl std::fmt::Debug for DataSourceTetMeshPrim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DataSourceTetMeshPrim")
    }
}

impl HdDataSourceBase for DataSourceTetMeshPrim {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for DataSourceTetMeshPrim {
    fn get_names(&self) -> Vec<Token> {
        DataSourceTetMeshPrim::get_names(self)
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        DataSourceTetMeshPrim::get(self, name)
    }
}

pub type DataSourceTetMeshPrimHandle = Arc<DataSourceTetMeshPrim>;

pub fn create_data_source_tet_mesh_prim(
    scene_index_path: Path,
    prim: Prim,
    stage_globals: DataSourceStageGlobalsHandle,
) -> DataSourceTetMeshPrimHandle {
    Arc::new(DataSourceTetMeshPrim::new(
        scene_index_path,
        prim,
        stage_globals,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source_stage_globals::NoOpStageGlobals;
    use usd_core::common::InitialLoadSet;
    use usd_core::Stage;
    use usd_hd::HdValueExtract;
    use usd_vt::Value;

    fn create_test_globals() -> DataSourceStageGlobalsHandle {
        Arc::new(NoOpStageGlobals::default())
    }

    #[test]
    fn test_tet_mesh_names_match_reference() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let prim = stage.get_pseudo_root();
        let ds = DataSourceTetMesh::new(Path::absolute_root(), prim, create_test_globals());
        let names = ds.get_names();
        assert_eq!(names.len(), 2);
        assert!(names.iter().any(|n| n == &*tokens::TOPOLOGY));
        assert!(names.iter().any(|n| n == &*tokens::DOUBLE_SIDED));
    }

    #[test]
    fn test_tet_mesh_invalidation_matches_reference() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let prim = stage.get_pseudo_root();
        let locators = DataSourceTetMeshPrim::invalidate(
            &prim,
            &Token::new(""),
            &[
                Token::new("orientation"),
                Token::new("doubleSided"),
                Token::new("tetVertexIndices"),
            ],
            PropertyInvalidationType::PropertyChanged,
        );

        assert!(locators.contains(&HdTetMeshSchema::get_topology_locator()));
        assert!(locators.contains(&HdTetMeshSchema::get_double_sided_locator()));
    }

    #[test]
    fn test_tet_mesh_value_extract_supports_topology_arrays() {
        let value = Value::from_no_hash(vec![usd_gf::Vec4i::new(0, 1, 2, 3)]);
        let extracted = Vec::<usd_gf::Vec4i>::extract(&value).expect("vec4i array");
        assert_eq!(extracted[0], usd_gf::Vec4i::new(0, 1, 2, 3));
    }
}
