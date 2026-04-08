//! Adapter for `UsdGeomTetMesh`.

use super::data_source_stage_globals::DataSourceStageGlobalsHandle;
use super::prim_adapter::PrimAdapter;
use super::types::PropertyInvalidationType;
use crate::data_source_tet_mesh::DataSourceTetMeshPrim;
use std::sync::Arc;
use usd_core::Prim;
use usd_hd::{HdContainerDataSourceHandle, HdDataSourceLocatorSet};
use usd_tf::Token;

mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static TET_MESH: LazyLock<Token> = LazyLock::new(|| Token::new("tetMesh"));
}

#[derive(Debug, Clone)]
pub struct TetMeshAdapter;

impl Default for TetMeshAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl TetMeshAdapter {
    pub fn new() -> Self {
        Self
    }

    pub fn is_builtin_primvar(primvar_name: &Token) -> bool {
        matches!(
            primvar_name.as_str(),
            "points"
                | "tetVertexIndices"
                | "surfaceFaceVertexIndices"
                | "velocities"
                | "accelerations"
                | "displayColor"
                | "displayOpacity"
        )
    }
}

impl PrimAdapter for TetMeshAdapter {
    fn get_imaging_subprims(&self, _prim: &Prim) -> Vec<Token> {
        vec![Token::new("")]
    }

    fn get_imaging_subprim_type(&self, _prim: &Prim, subprim: &Token) -> Token {
        if subprim.is_empty() {
            tokens::TET_MESH.clone()
        } else {
            Token::new("")
        }
    }

    fn get_imaging_subprim_data(
        &self,
        prim: &Prim,
        subprim: &Token,
        stage_globals: &DataSourceStageGlobalsHandle,
    ) -> Option<HdContainerDataSourceHandle> {
        if !subprim.is_empty() {
            return None;
        }
        Some(Arc::new(DataSourceTetMeshPrim::new(
            prim.path().clone(),
            prim.clone(),
            stage_globals.clone(),
        )))
    }

    fn invalidate_imaging_subprim(
        &self,
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        DataSourceTetMeshPrim::invalidate(prim, subprim, properties, invalidation_type)
    }
}

pub type TetMeshAdapterHandle = Arc<TetMeshAdapter>;

pub fn create_tet_mesh_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(TetMeshAdapter::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source_stage_globals::NoOpStageGlobals;
    use usd_core::Stage;
    use usd_core::common::InitialLoadSet;

    fn create_test_globals() -> DataSourceStageGlobalsHandle {
        Arc::new(NoOpStageGlobals::default())
    }

    #[test]
    fn test_tet_mesh_adapter_type() {
        let adapter = TetMeshAdapter::new();
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let prim = stage.get_pseudo_root();
        assert_eq!(
            adapter
                .get_imaging_subprim_type(&prim, &Token::new(""))
                .as_str(),
            "tetMesh"
        );
    }

    #[test]
    fn test_tet_mesh_adapter_data_source_exists() {
        let adapter = TetMeshAdapter::new();
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let prim = stage.get_pseudo_root();
        let ds = adapter.get_imaging_subprim_data(&prim, &Token::new(""), &create_test_globals());
        assert!(ds.is_some());
    }
}
