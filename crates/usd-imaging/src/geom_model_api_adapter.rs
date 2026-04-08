//! GeomModelAPIAdapter - API schema adapter for UsdGeomModelAPI.
//!
//! Port of pxr/usdImaging/usdImaging/geomModelAPIAdapter.h/.cpp

use super::api_schema_adapter::APISchemaAdapter;
use super::data_source_mapped::{
    AttributeMapping, DataSourceMapped, PropertyMapping, PropertyMappings,
};
use super::data_source_stage_globals::DataSourceStageGlobalsHandle;
use super::geom_model_schema::GeomModelSchemaBuilder;
use super::types::PropertyInvalidationType;
use std::sync::Arc;
use std::sync::LazyLock;
use usd_core::{Prim, model_api::KindValidation, model_api::ModelAPI};
use usd_geom::model_api::ModelAPI as GeomModelAPI;
use usd_hd::data_source::HdRetainedTypedSampledDataSource;
use usd_hd::{HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdOverlayContainerDataSource};
use usd_sdf::Path;
use usd_tf::Token;

fn get_property_mappings() -> Vec<PropertyMapping> {
    let mut result = Vec::new();

    for usd_name in GeomModelAPI::get_schema_attribute_names(false) {
        let (name, matched) = Path::strip_prefix_namespace(usd_name.as_str(), "model");
        if matched {
            result.push(PropertyMapping::Attribute(AttributeMapping::new(
                usd_name,
                usd_hd::HdDataSourceLocator::from_token(Token::new(&name)),
            )));
        }
    }

    result
}

fn get_mappings() -> &'static PropertyMappings {
    static MAPPINGS: LazyLock<PropertyMappings> = LazyLock::new(|| {
        PropertyMappings::new(
            get_property_mappings(),
            crate::GeomModelSchema::get_default_locator(),
        )
    });
    &MAPPINGS
}

#[derive(Debug, Clone)]
pub struct GeomModelAPIAdapter;

impl Default for GeomModelAPIAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl GeomModelAPIAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl APISchemaAdapter for GeomModelAPIAdapter {
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

        let mut geom_model_ds: HdContainerDataSourceHandle = Arc::new(DataSourceMapped::new(
            prim.clone(),
            prim.path().clone(),
            get_mappings().clone(),
            stage_globals.clone(),
        ));

        if ModelAPI::new(prim.clone())
            .is_kind(&Token::new("component"), KindValidation::ModelHierarchy)
        {
            static APPLY_DRAW_MODE_DS: LazyLock<HdContainerDataSourceHandle> =
                LazyLock::new(|| {
                    GeomModelSchemaBuilder::new()
                        .set_apply_draw_mode(HdRetainedTypedSampledDataSource::<bool>::new(true))
                        .build()
                });
            geom_model_ds =
                HdOverlayContainerDataSource::new_2(APPLY_DRAW_MODE_DS.clone(), geom_model_ds);
        }

        Some(usd_hd::HdRetainedContainerDataSource::from_entries(&[(
            crate::GeomModelSchema::get_schema_token(),
            geom_model_ds as HdDataSourceBaseHandle,
        )]))
    }

    fn invalidate_imaging_subprim(
        &self,
        _prim: &Prim,
        subprim: &Token,
        applied_instance_name: &Token,
        properties: &[Token],
        _invalidation_type: PropertyInvalidationType,
    ) -> usd_hd::HdDataSourceLocatorSet {
        if !subprim.is_empty() || !applied_instance_name.is_empty() {
            return usd_hd::HdDataSourceLocatorSet::empty();
        }

        DataSourceMapped::invalidate(properties, get_mappings())
    }
}

pub type GeomModelAPIAdapterHandle = Arc<GeomModelAPIAdapter>;

pub fn create_geom_model_api_adapter() -> Arc<dyn APISchemaAdapter> {
    Arc::new(GeomModelAPIAdapter::new())
}
