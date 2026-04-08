//! Skeleton prim adapter for UsdSkelImaging.
//!
//! Port of pxr/usdImaging/usdSkelImaging/skeletonAdapter.h/cpp
//!
//! This module provides the adapter for converting UsdSkel::Skeleton prims
//! into Hydra representations for rendering skeletal rigs.

use super::DataSourceSkeletonPrim;
use crate::{
    data_source_stage_globals::DataSourceStageGlobalsHandle,
    prim_adapter::PrimAdapter,
    types::{PopulationMode, PropertyInvalidationType},
};
use std::sync::Arc;
use usd_core::Prim;
use usd_hd::{HdContainerDataSourceHandle, HdDataSourceLocatorSet};
use usd_skel::Cache as SkelCache;
use usd_tf::Token;

/// Tokens used by skeleton adapter.
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static SKELETON: LazyLock<Token> = LazyLock::new(|| Token::new("skeleton"));
}

/// Skeleton prim adapter.
///
/// Converts UsdSkel::Skeleton prims into Hydra representations.
/// The adapter:
/// 1. Creates a mesh representation for skeleton bones visualization
/// 2. Provides skeleton transform data to skinned meshes via ext computations
/// 3. Caches skeleton queries for efficient data access
pub struct SkeletonAdapter {
    /// Shared skeleton cache
    skel_cache: Arc<SkelCache>,
}

impl SkeletonAdapter {
    /// Create a new skeleton adapter.
    pub fn new() -> Self {
        Self {
            skel_cache: Arc::new(SkelCache::new()),
        }
    }

    /// Create with shared skeleton cache.
    pub fn with_cache(cache: Arc<SkelCache>) -> Self {
        Self { skel_cache: cache }
    }

    /// Get the skeleton cache.
    pub fn skel_cache(&self) -> &Arc<SkelCache> {
        &self.skel_cache
    }
}

impl Default for SkeletonAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl PrimAdapter for SkeletonAdapter {
    fn get_imaging_subprims(&self, _prim: &Prim) -> Vec<Token> {
        // Skeleton produces a single subprim for bones visualization
        vec![Token::new("")]
    }

    fn get_imaging_subprim_type(&self, _prim: &Prim, subprim: &Token) -> Token {
        // C++ returns empty token for non-empty subprims
        if !subprim.is_empty() {
            return Token::new("");
        }
        // Skeleton has its own prim type, not "mesh"
        tokens::SKELETON.clone()
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
        Some(DataSourceSkeletonPrim::new(
            prim.path().clone(),
            prim.clone(),
            stage_globals.clone(),
        ))
    }

    fn invalidate_imaging_subprim(
        &self,
        _prim: &Prim,
        _subprim: &Token,
        properties: &[Token],
        _invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        if !_subprim.is_empty() {
            return HdDataSourceLocatorSet::empty();
        }
        DataSourceSkeletonPrim::invalidate(_prim, _subprim, properties, _invalidation_type)
    }

    fn get_population_mode(&self) -> PopulationMode {
        PopulationMode::RepresentsSelf
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_core::Stage;

    #[test]
    fn test_skeleton_adapter_creation() {
        let adapter = SkeletonAdapter::new();
        let cache = adapter.skel_cache();
        assert!(Arc::strong_count(cache) >= 1);
    }

    #[test]
    fn test_subprim_type() {
        let adapter = SkeletonAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        let prim_type = adapter.get_imaging_subprim_type(&prim, &Token::new(""));
        assert_eq!(prim_type.as_str(), "skeleton");
    }

    #[test]
    fn test_population_mode() {
        let adapter = SkeletonAdapter::new();
        assert_eq!(
            adapter.get_population_mode(),
            PopulationMode::RepresentsSelf
        );
    }
}
