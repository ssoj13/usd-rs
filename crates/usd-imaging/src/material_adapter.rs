//! MaterialAdapter - Adapter for UsdShadeMaterial prims.
//!
//! Port of pxr/usdImaging/usdImaging/materialAdapter.h/cpp

use super::data_source_material::DataSourceMaterialPrim;
use super::data_source_stage_globals::DataSourceStageGlobalsHandle;
use super::prim_adapter::PrimAdapter;
use super::types::{PopulationMode, PropertyInvalidationType};
use std::sync::Arc;
use usd_core::Prim;
use usd_hd::{HdContainerDataSourceHandle, HdDataSourceLocatorSet};
use usd_tf::Token;

mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static MATERIAL: LazyLock<Token> = LazyLock::new(|| Token::new("material"));
}

/// Adapter for UsdShadeMaterial prims.
#[derive(Debug, Clone)]
pub struct MaterialAdapter;

impl Default for MaterialAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl MaterialAdapter {
    pub fn new() -> Self {
        Self
    }

    pub fn get_population_mode() -> PopulationMode {
        PopulationMode::RepresentsSelfAndDescendents
    }
}

impl PrimAdapter for MaterialAdapter {
    fn get_imaging_subprims(&self, _prim: &Prim) -> Vec<Token> {
        vec![Token::default()]
    }

    fn get_imaging_subprim_type(&self, _prim: &Prim, subprim: &Token) -> Token {
        if subprim.is_empty() {
            tokens::MATERIAL.clone()
        } else {
            Token::default()
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

        Some(Arc::new(DataSourceMaterialPrim::new(
            prim.path().clone(),
            prim.clone(),
            stage_globals.clone(),
        )) as HdContainerDataSourceHandle)
    }

    fn invalidate_imaging_subprim(
        &self,
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        DataSourceMaterialPrim::invalidate(prim, subprim, properties, invalidation_type)
    }
}

/// Adapter for UsdShadeShader prims.
#[derive(Debug, Clone)]
pub struct ShaderAdapter;

impl Default for ShaderAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ShaderAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl PrimAdapter for ShaderAdapter {
    fn get_imaging_subprims(&self, _prim: &Prim) -> Vec<Token> {
        vec![]
    }

    fn get_imaging_subprim_type(&self, _prim: &Prim, _subprim: &Token) -> Token {
        Token::default()
    }

    fn get_imaging_subprim_data(
        &self,
        _prim: &Prim,
        _subprim: &Token,
        _stage_globals: &DataSourceStageGlobalsHandle,
    ) -> Option<HdContainerDataSourceHandle> {
        None
    }

    fn invalidate_imaging_subprim(
        &self,
        _prim: &Prim,
        _subprim: &Token,
        _properties: &[Token],
        _invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        HdDataSourceLocatorSet::empty()
    }
}

/// Adapter for UsdShadeNodeGraph prims.
#[derive(Debug, Clone)]
pub struct NodeGraphAdapter;

impl Default for NodeGraphAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeGraphAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl PrimAdapter for NodeGraphAdapter {
    fn get_imaging_subprims(&self, _prim: &Prim) -> Vec<Token> {
        vec![]
    }

    fn get_imaging_subprim_type(&self, _prim: &Prim, _subprim: &Token) -> Token {
        Token::default()
    }

    fn get_imaging_subprim_data(
        &self,
        _prim: &Prim,
        _subprim: &Token,
        _stage_globals: &DataSourceStageGlobalsHandle,
    ) -> Option<HdContainerDataSourceHandle> {
        None
    }

    fn invalidate_imaging_subprim(
        &self,
        _prim: &Prim,
        _subprim: &Token,
        _properties: &[Token],
        _invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        HdDataSourceLocatorSet::empty()
    }
}

pub fn create_material_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(MaterialAdapter::new())
}

pub fn create_shader_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(ShaderAdapter::new())
}

pub fn create_node_graph_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(NodeGraphAdapter::new())
}

pub type MaterialAdapterHandle = Arc<MaterialAdapter>;
pub type ShaderAdapterHandle = Arc<ShaderAdapter>;
pub type NodeGraphAdapterHandle = Arc<NodeGraphAdapter>;

#[cfg(test)]
mod tests {
    use super::*;
    use usd_core::Stage;

    #[test]
    fn test_material_adapter() {
        let adapter = MaterialAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        let prim_type = adapter.get_imaging_subprim_type(&prim, &Token::new(""));
        assert_eq!(prim_type.as_str(), "material");
    }

    #[test]
    fn test_shader_adapter() {
        let adapter = ShaderAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        assert!(adapter.get_imaging_subprims(&prim).is_empty());
    }

    #[test]
    fn test_node_graph_adapter() {
        let adapter = NodeGraphAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        assert!(adapter.get_imaging_subprims(&prim).is_empty());
    }

    #[test]
    fn test_material_population_mode() {
        assert_eq!(
            MaterialAdapter::get_population_mode(),
            PopulationMode::RepresentsSelfAndDescendents
        );
    }
}
