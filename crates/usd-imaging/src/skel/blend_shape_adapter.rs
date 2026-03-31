//! Blend shape prim adapter for UsdSkelImaging.
//!
//! Port of pxr/usdImaging/usdSkelImaging/blendShapeAdapter.h/cpp
//!
//! This module provides the adapter for UsdSkel::BlendShape prims,
//! enabling corrective shape deformations commonly used for facial animation.

use crate::{
    data_source_stage_globals::DataSourceStageGlobalsHandle,
    prim_adapter::PrimAdapter,
    types::{PopulationMode, PropertyInvalidationType},
};
use super::DataSourceBlendShapePrim;
use usd_core::Prim;
use usd_hd::{HdContainerDataSourceHandle, HdDataSourceLocatorSet};
use usd_tf::Token;

/// Tokens used by blend shape adapter.
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static SKEL_BLEND_SHAPE: LazyLock<Token> = LazyLock::new(|| Token::new("skelBlendShape"));
}

/// Blend shape prim adapter.
///
/// Converts UsdSkel::BlendShape prims into Hydra data sources.
/// The adapter provides:
/// 1. Point offsets for shape deformation
/// 2. Normal offsets for lighting correction
/// 3. Optional point indices for sparse shapes
/// 4. Inbetween shape data for interpolated poses
pub struct BlendShapeAdapter;

impl BlendShapeAdapter {
    /// Create a new blend shape adapter.
    pub fn new() -> Self {
        Self
    }

}

impl Default for BlendShapeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl PrimAdapter for BlendShapeAdapter {
    fn get_imaging_subprims(&self, _prim: &Prim) -> Vec<Token> {
        // Blend shape produces a single subprim
        vec![Token::new("")]
    }

    fn get_imaging_subprim_type(&self, _prim: &Prim, subprim: &Token) -> Token {
        if !subprim.is_empty() {
            return Token::new("");
        }
        tokens::SKEL_BLEND_SHAPE.clone()
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
        Some(DataSourceBlendShapePrim::new(
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
        DataSourceBlendShapePrim::invalidate(prim, subprim, properties, invalidation_type)
    }

    fn get_population_mode(&self) -> PopulationMode {
        // Blend shapes are represented by ancestor (SkelRoot/skeleton binding)
        PopulationMode::RepresentedByAncestor
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_core::Stage;

    #[test]
    fn test_blend_shape_adapter_creation() {
        let _adapter = BlendShapeAdapter::new();
    }

    #[test]
    fn test_subprim_type() {
        let adapter = BlendShapeAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        let prim_type = adapter.get_imaging_subprim_type(&prim, &Token::new(""));
        assert_eq!(prim_type.as_str(), "skelBlendShape");
    }

    #[test]
    fn test_population_mode() {
        let adapter = BlendShapeAdapter::new();
        assert_eq!(
            adapter.get_population_mode(),
            PopulationMode::RepresentedByAncestor
        );
    }
}
