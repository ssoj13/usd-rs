//! RenderMan AOV light adapter.

use std::sync::Arc;

use crate::data_source_stage_globals::DataSourceStageGlobalsHandle;
use crate::prim_adapter::PrimAdapter;
use usd_core::Prim;
use usd_hd::{HdContainerDataSourceHandle, HdDataSourceLocatorSet};
use usd_tf::Token;

#[derive(Debug, Clone, Default)]
pub struct PxrAovLightAdapter;

impl PxrAovLightAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl PrimAdapter for PxrAovLightAdapter {
    fn get_imaging_subprims(&self, _prim: &Prim) -> Vec<Token> {
        vec![Token::new("")]
    }

    fn get_imaging_subprim_type(&self, _prim: &Prim, subprim: &Token) -> Token {
        if subprim.is_empty() {
            Token::new("light")
        } else {
            Token::new("")
        }
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
        _invalidation_type: crate::types::PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        HdDataSourceLocatorSet::empty()
    }
}

pub type PxrAovLightAdapterHandle = Arc<PxrAovLightAdapter>;

pub fn create_pxr_aov_light_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(PxrAovLightAdapter::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_core::Stage;

    #[test]
    fn test_aov_light_type() {
        let adapter = PxrAovLightAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll).unwrap();
        assert_eq!(
            adapter
                .get_imaging_subprim_type(&stage.get_pseudo_root(), &Token::new(""))
                .as_str(),
            "light"
        );
    }
}
