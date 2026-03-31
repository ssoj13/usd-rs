//! RenderMan camera projection adapter.
//!
//! Port of `pxr/usdImaging/usdRiPxrImaging/pxrCameraProjectionAdapter.h/cpp`.

use super::data_source_render_terminal::DataSourceRenderTerminalPrim;
use super::projection_schema::ProjectionSchema;
use crate::data_source_stage_globals::DataSourceStageGlobalsHandle;
use crate::prim_adapter::PrimAdapter;
use crate::types::PropertyInvalidationType;
use std::sync::Arc;
use usd_core::Prim;
use usd_hd::{HdContainerDataSourceHandle, HdDataSourceLocatorSet, cast_to_container};
use usd_tf::Token;

#[allow(dead_code)]
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static PXR_PROJECTION: LazyLock<Token> = LazyLock::new(|| Token::new("pxrProjection"));
}

#[derive(Debug, Clone)]
pub struct PxrCameraProjectionAdapter;

impl Default for PxrCameraProjectionAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl PxrCameraProjectionAdapter {
    /// Create a new camera projection adapter.
    pub fn new() -> Self {
        Self
    }
}

impl PrimAdapter for PxrCameraProjectionAdapter {
    fn get_imaging_subprims(&self, _prim: &Prim) -> Vec<Token> {
        vec![Token::new("")]
    }

    fn get_imaging_subprim_type(&self, _prim: &Prim, subprim: &Token) -> Token {
        if subprim.is_empty() {
            tokens::PXR_PROJECTION.clone()
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
            tokens::PXR_PROJECTION.clone(),
            Token::new("ri:projection:shaderId"),
            stage_globals,
        );
        ds.get(&tokens::PXR_PROJECTION)
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
            &ProjectionSchema::get_resource_locator(),
        )
    }
}

/// Handle type for PxrCameraProjectionAdapter.
pub type PxrCameraProjectionAdapterHandle = Arc<PxrCameraProjectionAdapter>;

/// Factory for creating camera projection adapters.
pub fn create_pxr_camera_projection_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(PxrCameraProjectionAdapter::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_core::Stage;

    #[test]
    fn test_pxr_camera_projection_adapter() {
        let adapter = PxrCameraProjectionAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        let prim_type = adapter.get_imaging_subprim_type(&prim, &Token::new(""));
        assert_eq!(prim_type.as_str(), "pxrProjection");
    }

    #[test]
    fn test_factory() {
        let _ = create_pxr_camera_projection_adapter();
    }
}
