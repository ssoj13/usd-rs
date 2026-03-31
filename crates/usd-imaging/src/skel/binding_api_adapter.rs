//! BindingAPIAdapter - API schema adapter for UsdSkelBindingAPI.
//!
//! Port of pxr/usdImaging/usdSkelImaging/bindingAPIAdapter.h/.cpp

use crate::api_schema_adapter::APISchemaAdapter;
use crate::data_source_stage_globals::DataSourceStageGlobalsHandle;
use crate::types::PropertyInvalidationType;
use std::sync::Arc;
use usd_core::Prim;
use usd_hd::{HdContainerDataSourceHandle, HdDataSourceLocatorSet};
use usd_tf::Token;

use super::data_source_binding_api::DataSourceBindingAPI;

#[derive(Debug, Clone)]
pub struct BindingAPIAdapter;

impl Default for BindingAPIAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl BindingAPIAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl APISchemaAdapter for BindingAPIAdapter {
    fn get_imaging_subprims(&self, _prim: &Prim, _applied_instance_name: &Token) -> Vec<Token> {
        vec![]
    }

    fn get_imaging_subprim_type(
        &self,
        _prim: &Prim,
        _subprim: &Token,
        _applied_instance_name: &Token,
    ) -> Token {
        Token::new("")
    }

    fn get_imaging_subprim_data(
        &self,
        prim: &Prim,
        subprim: &Token,
        applied_instance_name: &Token,
        stage_globals: &DataSourceStageGlobalsHandle,
    ) -> Option<HdContainerDataSourceHandle> {
        if !subprim.is_empty() || !applied_instance_name.is_empty() {
            return None;
        }

        Some(DataSourceBindingAPI::new(
            prim.path().clone(),
            prim.clone(),
            stage_globals.clone(),
        ))
    }

    fn invalidate_imaging_subprim(
        &self,
        prim: &Prim,
        subprim: &Token,
        applied_instance_name: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        if !subprim.is_empty() || !applied_instance_name.is_empty() {
            return HdDataSourceLocatorSet::empty();
        }

        DataSourceBindingAPI::invalidate(prim, subprim, properties, invalidation_type)
    }
}

pub type BindingAPIAdapterHandle = Arc<BindingAPIAdapter>;

pub fn create_binding_api_adapter() -> Arc<dyn APISchemaAdapter> {
    Arc::new(BindingAPIAdapter::new())
}

/// Create adapter compatible with `AdapterManager`.
pub fn create_default() -> crate::adapter_manager::ApiSchemaAdapterHandle {
    Arc::new(BindingAPIAdapter::new())
}

impl crate::adapter_manager::ApiSchemaAdapter for BindingAPIAdapter {
    fn get_schema_name(&self) -> Token {
        Token::new("SkelBindingAPI")
    }

    fn get_imaging_subprims(&self, prim: &Prim, applied_instance_name: &Token) -> Vec<Token> {
        <Self as APISchemaAdapter>::get_imaging_subprims(self, prim, applied_instance_name)
    }

    fn get_imaging_subprim_type(
        &self,
        prim: &Prim,
        subprim: &Token,
        applied_instance_name: &Token,
    ) -> Token {
        <Self as APISchemaAdapter>::get_imaging_subprim_type(
            self,
            prim,
            subprim,
            applied_instance_name,
        )
    }

    fn get_imaging_subprim_data(
        &self,
        prim: &Prim,
        subprim: &Token,
        applied_instance_name: &Token,
        stage_globals: &DataSourceStageGlobalsHandle,
    ) -> Option<HdContainerDataSourceHandle> {
        <Self as APISchemaAdapter>::get_imaging_subprim_data(
            self,
            prim,
            subprim,
            applied_instance_name,
            stage_globals,
        )
    }

    fn invalidate_imaging_subprim(
        &self,
        prim: &Prim,
        subprim: &Token,
        applied_instance_name: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        <Self as APISchemaAdapter>::invalidate_imaging_subprim(
            self,
            prim,
            subprim,
            applied_instance_name,
            properties,
            invalidation_type,
        )
    }
}
