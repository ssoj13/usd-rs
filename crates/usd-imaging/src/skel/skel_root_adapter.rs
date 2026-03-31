//! SkelRoot prim adapter for UsdSkelImaging.
//!
//! Port of pxr/usdImaging/usdSkelImaging/skelRootAdapter.h/cpp
//!
//! This module provides the adapter for UsdSkel::SkelRoot prims, which
//! define the scope for skeletal bindings and manage the relationship
//! between skeletons and skinned geometry.

use crate::{
    data_source_prim::DataSourcePrim,
    data_source_stage_globals::DataSourceStageGlobalsHandle,
    prim_adapter::PrimAdapter,
    types::{PopulationMode, PropertyInvalidationType},
};
use super::BindingSchema;
use std::sync::Arc;
use usd_core::Prim;
use usd_hd::data_source::HdRetainedTypedSampledDataSource;
use usd_hd::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdDataSourceLocatorSet,
    HdOverlayContainerDataSource, HdRetainedContainerDataSource,
};
use usd_skel::{Cache as SkelCache, SkelRoot};
use usd_tf::Token;

/// Tokens used by skel root adapter.
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    #[allow(dead_code)] // C++ prim type token, wiring in progress
    pub static SKEL_ROOT: LazyLock<Token> = LazyLock::new(|| Token::new("SkelRoot"));
    pub static SKEL_SKELETON: LazyLock<Token> = LazyLock::new(|| Token::new("skel:skeleton"));
}

/// SkelRoot prim adapter.
///
/// Handles UsdSkel::SkelRoot prims which define the scope for skeletal bindings.
/// The adapter:
/// 1. Resolves skeleton bindings for descendant prims
/// 2. Manages SkelCache for efficient skeleton query access
/// 3. Provides binding data to skinned mesh adapters
pub struct SkelRootAdapter {
    /// Shared skeleton cache
    skel_cache: Arc<SkelCache>,
}

impl SkelRootAdapter {
    /// Create a new skel root adapter.
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

    /// Populate skeleton cache for this skel root.
    pub fn populate(&self, prim: &Prim) -> bool {
        let skel_root = SkelRoot::new(prim.clone());
        if !skel_root.is_valid() {
            return false;
        }
        self.skel_cache.populate_default(&skel_root)
    }

    fn build_skel_root_data_source(
        &self,
        prim: &Prim,
        stage_globals: &DataSourceStageGlobalsHandle,
    ) -> Option<HdContainerDataSourceHandle> {
        let skel_root = SkelRoot::new(prim.clone());
        if !skel_root.is_valid() {
            return None;
        }
        let binding = HdRetainedContainerDataSource::from_entries(&[(
            Token::new("hasSkelRoot"),
            HdRetainedTypedSampledDataSource::new(true) as HdDataSourceBaseHandle,
        )]);
        let overlay = HdRetainedContainerDataSource::from_entries(&[(
            BindingSchema::get_schema_token(),
            binding as HdDataSourceBaseHandle,
        )]);
        HdOverlayContainerDataSource::overlayed(
            Some(overlay),
            Some(Arc::new(DataSourcePrim::new(
                prim.clone(),
                prim.path().clone(),
                stage_globals.clone(),
            ))),
        )
    }
}

impl Default for SkelRootAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl PrimAdapter for SkelRootAdapter {
    fn get_imaging_subprims(&self, _prim: &Prim) -> Vec<Token> {
        vec![Token::new("")]
    }

    fn get_imaging_subprim_type(&self, _prim: &Prim, _subprim: &Token) -> Token {
        // C++ returns empty TfToken() - SkelRoot has no imaging subprim type
        Token::empty()
    }

    fn get_imaging_subprim_data(
        &self,
        prim: &Prim,
        subprim: &Token,
        stage_globals: &DataSourceStageGlobalsHandle,
    ) -> Option<HdContainerDataSourceHandle> {
        // C++: returns nullptr for non-empty subprims
        if !subprim.is_empty() {
            return None;
        }
        self.build_skel_root_data_source(prim, stage_globals)
    }

    fn invalidate_imaging_subprim(
        &self,
        _prim: &Prim,
        subprim: &Token,
        _properties: &[Token],
        _invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        if !subprim.is_empty() {
            return HdDataSourceLocatorSet::empty();
        }
        DataSourcePrim::invalidate(_prim, subprim, _properties, _invalidation_type)
    }

    fn get_population_mode(&self) -> PopulationMode {
        // C++ SkelRootAdapter does NOT override GetPopulationMode();
        // base class default is RepresentsSelf.
        // The old Delegate path used RepresentsSelfAndDescendents, but in
        // the Scene Index architecture descendants are traversed normally
        // and skeleton resolution happens via SkeletonResolvingSceneIndex.
        PopulationMode::RepresentsSelf
    }

    fn should_cull_children(&self) -> bool {
        // Don't cull - we need to traverse descendants for binding resolution
        false
    }

    fn invalidate_imaging_subprim_from_descendant(
        &self,
        _prim: &Prim,
        _descendant_prim: &Prim,
        _subprim: &Token,
        properties: &[Token],
        _invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        // Check if descendant skeleton binding changed
        let skel_skeleton = tokens::SKEL_SKELETON.clone();

        for prop in properties {
            if prop == &skel_skeleton {
                return HdDataSourceLocatorSet::universal();
            }
        }

        HdDataSourceLocatorSet::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_core::Stage;

    #[test]
    fn test_skel_root_adapter_creation() {
        let adapter = SkelRootAdapter::new();
        let cache = adapter.skel_cache();
        assert!(Arc::strong_count(cache) >= 1);
    }

    #[test]
    fn test_subprim_type() {
        let adapter = SkelRootAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        let prim_type = adapter.get_imaging_subprim_type(&prim, &Token::new(""));
        assert_eq!(prim_type.as_str(), "");
    }

    #[test]
    fn test_population_mode() {
        let adapter = SkelRootAdapter::new();
        assert_eq!(
            adapter.get_population_mode(),
            PopulationMode::RepresentsSelf
        );
    }

    #[test]
    fn test_should_not_cull_children() {
        let adapter = SkelRootAdapter::new();
        assert!(!adapter.should_cull_children());
    }
}
