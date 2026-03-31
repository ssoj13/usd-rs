//! RepresentedByAncestorPrimAdapter - Base for prims represented by ancestors.
//!
//! Port of pxr/usdImaging/usdImaging/representedByAncestorPrimAdapter.h/cpp
//!
//! Base adapter for prims whose imaging is handled by an ancestor adapter.

use super::data_source_stage_globals::DataSourceStageGlobalsHandle;
use super::prim_adapter::PrimAdapter;
use super::types::{PopulationMode, PropertyInvalidationType};
use std::sync::Arc;
use usd_core::Prim;
use usd_hd::{HdContainerDataSourceHandle, HdDataSourceLocatorSet};
use usd_tf::Token;

// ============================================================================
// RepresentedByAncestorPrimAdapter
// ============================================================================

/// Base adapter for prims represented by an ancestor.
///
/// These prims don't produce their own hydra prims. Instead, changes
/// to them are forwarded to their ancestor adapter.
#[derive(Debug, Clone)]
pub struct RepresentedByAncestorPrimAdapter;

impl Default for RepresentedByAncestorPrimAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl RepresentedByAncestorPrimAdapter {
    /// Create a new represented by ancestor adapter.
    pub fn new() -> Self {
        Self
    }

    /// Returns RepresentedByAncestor mode.
    pub fn get_population_mode() -> PopulationMode {
        PopulationMode::RepresentedByAncestor
    }
}

impl PrimAdapter for RepresentedByAncestorPrimAdapter {
    fn get_imaging_subprims(&self, _prim: &Prim) -> Vec<Token> {
        // These prims don't produce hydra prims
        vec![]
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
        None
    }

    fn invalidate_imaging_subprim(
        &self,
        _prim: &Prim,
        _subprim: &Token,
        _properties: &[Token],
        _invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        // Invalidation is handled by ancestor adapter
        HdDataSourceLocatorSet::empty()
    }
}

/// Handle type for RepresentedByAncestorPrimAdapter.
pub type RepresentedByAncestorPrimAdapterHandle = Arc<RepresentedByAncestorPrimAdapter>;

/// Factory for creating represented by ancestor adapters.
pub fn create_represented_by_ancestor_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(RepresentedByAncestorPrimAdapter::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_core::Stage;

    #[test]
    fn test_represented_by_ancestor_adapter() {
        let adapter = RepresentedByAncestorPrimAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        // No subprims produced
        assert!(adapter.get_imaging_subprims(&prim).is_empty());
    }

    #[test]
    fn test_population_mode() {
        assert_eq!(
            RepresentedByAncestorPrimAdapter::get_population_mode(),
            PopulationMode::RepresentedByAncestor
        );
    }

    #[test]
    fn test_factory() {
        let _ = create_represented_by_ancestor_adapter();
    }
}
