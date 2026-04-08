//! CoordSysAPIAdapter - API schema adapter for UsdShadeCoordSysAPI.
//!
//! Port of pxr/usdImaging/usdImaging/coordSysAPIAdapter.h/.cpp

use super::api_schema_adapter::APISchemaAdapter;
use super::data_source_stage_globals::DataSourceStageGlobalsHandle;
use super::types::PropertyInvalidationType;
use std::sync::Arc;
use usd_core::Prim;
use usd_hd::data_source::HdRetainedTypedSampledDataSource;
use usd_hd::schema::HdCoordSysBindingSchema;
use usd_hd::{HdContainerDataSourceHandle, HdDataSourceLocatorSet};
use usd_shade::coord_sys_api::CoordSysAPI;
use usd_tf::Token;

#[derive(Debug, Clone)]
pub struct CoordSysAPIAdapter;

impl Default for CoordSysAPIAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl CoordSysAPIAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl APISchemaAdapter for CoordSysAPIAdapter {
    fn get_imaging_subprims(&self, _prim: &Prim, applied_instance_name: &Token) -> Vec<Token> {
        if applied_instance_name.is_empty() {
            vec![]
        } else {
            vec![Token::new("")]
        }
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
        _stage_globals: &DataSourceStageGlobalsHandle,
    ) -> Option<HdContainerDataSourceHandle> {
        if applied_instance_name.is_empty() || !subprim.is_empty() {
            return None;
        }

        let binding =
            CoordSysAPI::new(prim.clone(), applied_instance_name.clone()).get_local_binding();
        if binding.name.is_empty() {
            return None;
        }

        let value = HdRetainedTypedSampledDataSource::new(binding.coord_sys_prim_path);
        let inner = HdCoordSysBindingSchema::build_retained(
            std::slice::from_ref(applied_instance_name),
            &[value],
        );

        Some(usd_hd::HdRetainedContainerDataSource::from_entries(&[(
            (**HdCoordSysBindingSchema::get_schema_token()).clone(),
            inner as usd_hd::HdDataSourceBaseHandle,
        )]))
    }

    fn invalidate_imaging_subprim(
        &self,
        _prim: &Prim,
        subprim: &Token,
        applied_instance_name: &Token,
        properties: &[Token],
        _invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        if applied_instance_name.is_empty() || !subprim.is_empty() {
            return HdDataSourceLocatorSet::empty();
        }

        for property_name in properties {
            if CoordSysAPI::can_contain_property_name(property_name) {
                let mut locators = HdDataSourceLocatorSet::empty();
                locators.insert(HdCoordSysBindingSchema::get_default_locator());
                return locators;
            }
        }

        HdDataSourceLocatorSet::empty()
    }
}

pub type CoordSysAPIAdapterHandle = Arc<CoordSysAPIAdapter>;

pub fn create_coord_sys_api_adapter() -> Arc<dyn APISchemaAdapter> {
    Arc::new(CoordSysAPIAdapter::new())
}
