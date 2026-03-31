//! RenderMan integrator adapter.

use std::sync::Arc;

use super::data_source_render_terminal::{DataSourceRenderTerminalPrim, integrator_resource_locator};
use crate::data_source_stage_globals::DataSourceStageGlobalsHandle;
use crate::prim_adapter::PrimAdapter;
use crate::types::PropertyInvalidationType;
use usd_core::Prim;
use usd_hd::{HdContainerDataSourceHandle, HdDataSourceLocatorSet, cast_to_container};
use usd_tf::Token;

#[derive(Debug, Clone, Default)]
pub struct PxrIntegratorAdapter;

impl PxrIntegratorAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl PrimAdapter for PxrIntegratorAdapter {
    fn get_imaging_subprims(&self, _prim: &Prim) -> Vec<Token> {
        vec![Token::new("")]
    }

    fn get_imaging_subprim_type(&self, _prim: &Prim, subprim: &Token) -> Token {
        if subprim.is_empty() {
            Token::new("integrator")
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
        let ds = DataSourceRenderTerminalPrim::new(
            &prim.path().clone(),
            prim.clone(),
            Token::new("integrator"),
            Token::new("ri:integrator:shaderId"),
            stage_globals,
        );
        ds.get(&Token::new("integrator"))
            .as_ref()
            .and_then(cast_to_container)
    }

    fn invalidate_imaging_subprim(
        &self,
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        DataSourceRenderTerminalPrim::invalidate(
            prim,
            subprim,
            properties,
            invalidation_type,
            &integrator_resource_locator(),
        )
    }
}

pub type PxrIntegratorAdapterHandle = Arc<PxrIntegratorAdapter>;

pub fn create_pxr_integrator_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(PxrIntegratorAdapter::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source_stage_globals::NoOpStageGlobals;
    use usd_core::Stage;

    #[test]
    fn test_integrator_type() {
        let adapter = PxrIntegratorAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll).unwrap();
        assert_eq!(
            adapter
                .get_imaging_subprim_type(&stage.get_pseudo_root(), &Token::new(""))
                .as_str(),
            "integrator"
        );
    }

    #[test]
    fn test_integrator_data_container_exists() {
        let adapter = PxrIntegratorAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll).unwrap();
        let globals: DataSourceStageGlobalsHandle = Arc::new(NoOpStageGlobals::default());
        let data = adapter.get_imaging_subprim_data(&stage.get_pseudo_root(), &Token::new(""), &globals);
        assert!(data.is_some());
    }
}
