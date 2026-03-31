//! SceneIndexPrimAdapter - Base for scene index-based prim adapters.
//!
//! Port of pxr/usdImaging/usdImaging/sceneIndexPrimAdapter.h/cpp
//!
//! Base adapter that uses the modern scene index approach.

use super::data_source_stage_globals::DataSourceStageGlobalsHandle;
use super::prim_adapter::PrimAdapter;
use super::types::PropertyInvalidationType;
use std::sync::Arc;
use usd_core::Prim;
use usd_hd::{HdContainerDataSourceHandle, HdDataSourceLocatorSet};
use usd_tf::Token;

// ============================================================================
// SceneIndexPrimAdapter
// ============================================================================

/// Base adapter for scene index-based prim conversion.
///
/// This is the modern approach for USD to Hydra conversion, using
/// data sources and scene indices instead of the legacy delegate pattern.
#[derive(Debug, Clone)]
pub struct SceneIndexPrimAdapter;

impl Default for SceneIndexPrimAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl SceneIndexPrimAdapter {
    /// Create a new scene index prim adapter.
    pub fn new() -> Self {
        Self
    }
}

impl PrimAdapter for SceneIndexPrimAdapter {
    fn get_imaging_subprims(&self, _prim: &Prim) -> Vec<Token> {
        vec![Token::new("")]
    }

    fn get_imaging_subprim_type(&self, _prim: &Prim, _subprim: &Token) -> Token {
        Token::new("")
    }

    fn get_imaging_subprim_data(
        &self,
        _prim: &Prim,
        _subprim: &Token,
        _stage_globals: &DataSourceStageGlobalsHandle,
    ) -> Option<HdContainerDataSourceHandle> {
        // Derived classes override to provide data
        None
    }

    fn invalidate_imaging_subprim(
        &self,
        _prim: &Prim,
        _subprim: &Token,
        _properties: &[Token],
        _invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        // Derived classes override for proper invalidation
        HdDataSourceLocatorSet::empty()
    }
}

/// Handle type for SceneIndexPrimAdapter.
pub type SceneIndexPrimAdapterHandle = Arc<SceneIndexPrimAdapter>;

/// Factory for creating scene index prim adapters.
pub fn create_scene_index_prim_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(SceneIndexPrimAdapter::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_core::Stage;

    #[test]
    fn test_scene_index_prim_adapter() {
        let adapter = SceneIndexPrimAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        let subprims = adapter.get_imaging_subprims(&prim);
        assert_eq!(subprims.len(), 1);
    }

    #[test]
    fn test_factory() {
        let _ = create_scene_index_prim_adapter();
    }
}
