//! Adapters for `UsdRender*` prims.

use super::data_source_stage_globals::DataSourceStageGlobalsHandle;
use super::prim_adapter::PrimAdapter;
use super::types::PropertyInvalidationType;
use crate::data_source_render_prims::{
    DataSourceRenderPassPrim, DataSourceRenderProductPrim, DataSourceRenderSettingsPrim,
    DataSourceRenderVarPrim,
};
use std::sync::Arc;
use usd_core::Prim;
use usd_hd::{HdContainerDataSourceHandle, HdDataSourceLocatorSet};
use usd_tf::Token;

mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static RENDER_SETTINGS: LazyLock<Token> = LazyLock::new(|| Token::new("renderSettings"));
    pub static RENDER_PRODUCT: LazyLock<Token> = LazyLock::new(|| Token::new("renderProduct"));
    pub static RENDER_VAR: LazyLock<Token> = LazyLock::new(|| Token::new("renderVar"));
    pub static RENDER_PASS: LazyLock<Token> = LazyLock::new(|| Token::new("renderPass"));
}

#[derive(Debug, Clone)]
pub struct RenderSettingsAdapter;

impl Default for RenderSettingsAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderSettingsAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl PrimAdapter for RenderSettingsAdapter {
    fn get_imaging_subprims(&self, _prim: &Prim) -> Vec<Token> {
        vec![Token::new("")]
    }

    fn get_imaging_subprim_type(&self, _prim: &Prim, subprim: &Token) -> Token {
        if subprim.is_empty() {
            tokens::RENDER_SETTINGS.clone()
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
        Some(Arc::new(DataSourceRenderSettingsPrim::new(
            prim.path().clone(),
            prim.clone(),
            stage_globals.clone(),
        )))
    }

    fn invalidate_imaging_subprim(
        &self,
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        DataSourceRenderSettingsPrim::invalidate(prim, subprim, properties, invalidation_type)
    }
}

#[derive(Debug, Clone)]
pub struct RenderProductAdapter;

impl Default for RenderProductAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderProductAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl PrimAdapter for RenderProductAdapter {
    fn get_imaging_subprims(&self, _prim: &Prim) -> Vec<Token> {
        vec![Token::new("")]
    }

    fn get_imaging_subprim_type(&self, _prim: &Prim, subprim: &Token) -> Token {
        if subprim.is_empty() {
            tokens::RENDER_PRODUCT.clone()
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
        Some(Arc::new(DataSourceRenderProductPrim::new(
            prim.path().clone(),
            prim.clone(),
            stage_globals.clone(),
        )))
    }

    fn invalidate_imaging_subprim(
        &self,
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        DataSourceRenderProductPrim::invalidate(prim, subprim, properties, invalidation_type)
    }
}

#[derive(Debug, Clone)]
pub struct RenderVarAdapter;

impl Default for RenderVarAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderVarAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl PrimAdapter for RenderVarAdapter {
    fn get_imaging_subprims(&self, _prim: &Prim) -> Vec<Token> {
        vec![Token::new("")]
    }

    fn get_imaging_subprim_type(&self, _prim: &Prim, subprim: &Token) -> Token {
        if subprim.is_empty() {
            tokens::RENDER_VAR.clone()
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
        Some(Arc::new(DataSourceRenderVarPrim::new(
            prim.path().clone(),
            prim.clone(),
            stage_globals.clone(),
        )))
    }

    fn invalidate_imaging_subprim(
        &self,
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        DataSourceRenderVarPrim::invalidate(prim, subprim, properties, invalidation_type)
    }
}

#[derive(Debug, Clone)]
pub struct RenderPassAdapter;

impl Default for RenderPassAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderPassAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl PrimAdapter for RenderPassAdapter {
    fn get_imaging_subprims(&self, _prim: &Prim) -> Vec<Token> {
        vec![Token::new("")]
    }

    fn get_imaging_subprim_type(&self, _prim: &Prim, subprim: &Token) -> Token {
        if subprim.is_empty() {
            tokens::RENDER_PASS.clone()
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
        Some(Arc::new(DataSourceRenderPassPrim::new(
            prim.path().clone(),
            prim.clone(),
            stage_globals.clone(),
        )))
    }

    fn invalidate_imaging_subprim(
        &self,
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        DataSourceRenderPassPrim::invalidate(prim, subprim, properties, invalidation_type)
    }
}

pub type RenderSettingsAdapterHandle = Arc<RenderSettingsAdapter>;
pub type RenderProductAdapterHandle = Arc<RenderProductAdapter>;
pub type RenderVarAdapterHandle = Arc<RenderVarAdapter>;
pub type RenderPassAdapterHandle = Arc<RenderPassAdapter>;

pub fn create_render_settings_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(RenderSettingsAdapter::new())
}

pub fn create_render_product_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(RenderProductAdapter::new())
}

pub fn create_render_var_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(RenderVarAdapter::new())
}

pub fn create_render_pass_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(RenderPassAdapter::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source_stage_globals::NoOpStageGlobals;
    use crate::usd_render_settings_schema::UsdRenderSettingsSchema;
    use usd_core::Stage;
    use usd_core::common::InitialLoadSet;

    fn create_test_globals() -> DataSourceStageGlobalsHandle {
        Arc::new(NoOpStageGlobals::default())
    }

    #[test]
    fn test_render_settings_adapter_provides_schema_wrapper() {
        let adapter = RenderSettingsAdapter::new();
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let prim = stage.get_pseudo_root();
        let data = adapter
            .get_imaging_subprim_data(&prim, &Token::new(""), &create_test_globals())
            .expect("render settings data");
        let names = data.get_names();
        assert_eq!(names, vec![UsdRenderSettingsSchema::get_schema_token()]);
    }
}
