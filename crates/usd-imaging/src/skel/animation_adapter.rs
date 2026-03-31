//! AnimationAdapter - Adapter for UsdSkelAnimation prims.
//!
//! Port of pxr/usdImaging/usdSkelImaging/animationAdapter.h/.cpp

use super::data_source_animation_prim::DataSourceAnimationPrim;
use crate::data_source_stage_globals::DataSourceStageGlobalsHandle;
use crate::prim_adapter::PrimAdapter;
use crate::types::PropertyInvalidationType;
use std::sync::Arc;
use usd_core::Prim;
use usd_hd::{HdContainerDataSourceHandle, HdDataSourceLocatorSet};
use usd_tf::Token;

#[derive(Debug, Clone)]
pub struct AnimationAdapter;

impl Default for AnimationAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl AnimationAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl PrimAdapter for AnimationAdapter {
    fn get_imaging_subprims(&self, _prim: &Prim) -> Vec<Token> {
        vec![Token::new("")]
    }

    fn get_imaging_subprim_type(&self, _prim: &Prim, subprim: &Token) -> Token {
        if !subprim.is_empty() {
            return Token::new("");
        }
        Token::new("skelAnimation")
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
        Some(DataSourceAnimationPrim::new(
            prim.path().clone(),
            prim.clone(),
            stage_globals.clone(),
        ))
    }

    fn invalidate_imaging_subprim(
        &self,
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        if !subprim.is_empty() {
            return HdDataSourceLocatorSet::empty();
        }
        DataSourceAnimationPrim::invalidate(prim, subprim, properties, invalidation_type)
    }
}

pub type AnimationAdapterHandle = Arc<AnimationAdapter>;

pub fn create_animation_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(AnimationAdapter::new())
}
