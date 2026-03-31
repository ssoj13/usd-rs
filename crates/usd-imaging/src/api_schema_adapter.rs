//! APISchemaAdapter - Base adapter for applied API schemas.
//!
//! Port of pxr/usdImaging/usdImaging/apiSchemaAdapter.h/cpp
//!
//! API schema adapters provide data source contributions for applied API
//! schemas. Their results are overlaid on top of the prim adapter's data.

use super::data_source_stage_globals::DataSourceStageGlobalsHandle;
use super::types::PropertyInvalidationType;
use std::sync::Arc;
use usd_core::Prim;
use usd_hd::{HdContainerDataSourceHandle, HdDataSourceLocatorSet};
use usd_tf::Token;

/// Base trait for API schema adapters.
///
/// These map behavior of applied API schemas to contributions to hydra prims
/// and data sources generated for a given USD prim.
pub trait APISchemaAdapter: Send + Sync + std::fmt::Debug {
    /// Returns additional child hydra prims beyond the primary prim.
    ///
    /// Token values returned are appended as property names to the hydra path.
    /// `applied_instance_name` is non-empty for multiple-apply schema instances.
    fn get_imaging_subprims(&self, _prim: &Prim, _applied_instance_name: &Token) -> Vec<Token> {
        vec![]
    }

    /// Returns the hydra type for a given subprim.
    ///
    /// `subprim` corresponds to an element from `get_imaging_subprims`.
    /// `applied_instance_name` is non-empty for multiple-apply schema instances.
    fn get_imaging_subprim_type(
        &self,
        _prim: &Prim,
        _subprim: &Token,
        _applied_instance_name: &Token,
    ) -> Token {
        Token::new("")
    }

    /// Returns data source contributions for the primary prim or a subprim.
    ///
    /// Non-null results from prim adapter and each API schema adapter are
    /// overlaid in application order.
    fn get_imaging_subprim_data(
        &self,
        _prim: &Prim,
        _subprim: &Token,
        _applied_instance_name: &Token,
        _stage_globals: &DataSourceStageGlobalsHandle,
    ) -> Option<HdContainerDataSourceHandle> {
        None
    }

    /// Returns locators for data sources that should be dirtied.
    ///
    /// Given names of USD properties which have changed, the adapter may
    /// provide locators describing which data sources should be flagged dirty.
    fn invalidate_imaging_subprim(
        &self,
        _prim: &Prim,
        _subprim: &Token,
        _applied_instance_name: &Token,
        _properties: &[Token],
        _invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        HdDataSourceLocatorSet::empty()
    }
}

/// Handle type for API schema adapters.
pub type APISchemaAdapterHandle = Arc<dyn APISchemaAdapter>;

/// Factory function type for creating API schema adapters.
pub type APISchemaAdapterFactory = fn() -> APISchemaAdapterHandle;

// ============================================================================
// NoOpAPISchemaAdapter - Default implementation
// ============================================================================

/// No-op API schema adapter that returns empty results.
#[derive(Debug, Clone, Default)]
pub struct NoOpAPISchemaAdapter;

impl NoOpAPISchemaAdapter {
    /// Create a new no-op adapter.
    pub fn new() -> Self {
        Self
    }
}

impl APISchemaAdapter for NoOpAPISchemaAdapter {}

/// Factory for no-op API schema adapter.
pub fn create_noop_api_schema_adapter() -> APISchemaAdapterHandle {
    Arc::new(NoOpAPISchemaAdapter::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_core::Stage;

    #[test]
    fn test_noop_adapter() {
        let adapter = NoOpAPISchemaAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let empty_token = Token::new("");

        assert!(adapter.get_imaging_subprims(&prim, &empty_token).is_empty());
        assert!(
            adapter
                .get_imaging_subprim_type(&prim, &empty_token, &empty_token)
                .is_empty()
        );
    }

    #[test]
    fn test_factory() {
        let _ = create_noop_api_schema_adapter();
    }
}
