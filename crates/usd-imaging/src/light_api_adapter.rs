//! LightAPIAdapter - API schema adapter for UsdLuxLightAPI.
//!
//! Port of pxr/usdImaging/usdImaging/lightAPIAdapter.h/cpp

use super::api_schema_adapter::APISchemaAdapter;
use super::data_source_attribute::DataSourceAttribute;
use super::data_source_material::DataSourceMaterial;
use super::data_source_stage_globals::DataSourceStageGlobalsHandle;
use super::types::PropertyInvalidationType;
use std::sync::Arc;
use usd_core::Prim;
use usd_hd::{
    HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
    HdDataSourceLocatorSet, HdRetainedContainerDataSource, HdRetainedTypedSampledDataSource,
};
use usd_hd::schema::{HdLightSchema, HdMaterialSchema};
use usd_lux::light_api::LightAPI;
use usd_tf::Token;
use usd_vt::Value;

#[derive(Clone)]
struct LightDataSource {
    light_api: LightAPI,
    stage_globals: DataSourceStageGlobalsHandle,
}

impl std::fmt::Debug for LightDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LightDataSource").finish()
    }
}

impl LightDataSource {
    fn new(prim: Prim, stage_globals: DataSourceStageGlobalsHandle) -> Arc<Self> {
        Arc::new(Self {
            light_api: LightAPI::new(prim),
            stage_globals,
        })
    }
}

impl HdDataSourceBase for LightDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for LightDataSource {
    fn get_names(&self) -> Vec<Token> {
        vec![
            Token::new("filters"),
            Token::new("isLight"),
            Token::new("materialSyncMode"),
        ]
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if name.as_str() == "filters" {
            let paths = self
                .light_api
                .get_filters_rel()
                .map(|rel| rel.get_forwarded_targets())
                .unwrap_or_default();
            return Some(HdRetainedTypedSampledDataSource::new(paths) as HdDataSourceBaseHandle);
        }

        if name.as_str() == "isLight" {
            return Some(HdRetainedTypedSampledDataSource::new(true) as HdDataSourceBaseHandle);
        }

        if name.as_str() == "materialSyncMode" {
            if let Some(attr) = self.light_api.get_material_sync_mode_attr() {
                return Some(
                    DataSourceAttribute::<Token>::new(
                        attr,
                        self.stage_globals.clone(),
                        self.light_api.get_prim().path().clone(),
                    ) as HdDataSourceBaseHandle,
                );
            }
            return None;
        }

        let usd_attr_name = format!("inputs:{}", name.as_str());
        let attr = self.light_api.get_prim().get_attribute(&usd_attr_name)?;
        Some(
            DataSourceAttribute::<Value>::new(
                attr,
                self.stage_globals.clone(),
                self.light_api.get_prim().path().clone(),
            ) as HdDataSourceBaseHandle,
        )
    }
}

/// API schema adapter for UsdLuxLightAPI.
#[derive(Debug, Clone)]
pub struct LightAPIAdapter;

impl Default for LightAPIAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl LightAPIAdapter {
    /// Create a new light API adapter.
    pub fn new() -> Self {
        Self
    }
}

impl APISchemaAdapter for LightAPIAdapter {
    fn get_imaging_subprims(&self, _prim: &Prim, _applied_instance_name: &Token) -> Vec<Token> {
        vec![]
    }

    fn get_imaging_subprim_type(
        &self,
        _prim: &Prim,
        subprim: &Token,
        applied_instance_name: &Token,
    ) -> Token {
        if !applied_instance_name.is_empty() || !subprim.is_empty() {
            return Token::default();
        }
        Token::new("light")
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

        Some(HdRetainedContainerDataSource::new_2(
            (*HdMaterialSchema::get_schema_token()).clone(),
            Arc::new(DataSourceMaterial::new(
                prim.path().clone(),
                prim.clone(),
                stage_globals.clone(),
            )) as HdDataSourceBaseHandle,
            (*HdLightSchema::get_schema_token()).clone(),
            LightDataSource::new(prim.clone(), stage_globals.clone()) as HdDataSourceBaseHandle,
        ) as HdContainerDataSourceHandle)
    }

    fn invalidate_imaging_subprim(
        &self,
        _prim: &Prim,
        subprim: &Token,
        applied_instance_name: &Token,
        properties: &[Token],
        _invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        if !subprim.is_empty() || !applied_instance_name.is_empty() {
            return HdDataSourceLocatorSet::empty();
        }

        let mut result = HdDataSourceLocatorSet::empty();
        let mut dirtied_material = false;
        let mut dirtied_light = false;

        for prop in properties {
            let property_name = prop.as_str();
            if !dirtied_material && property_name.starts_with("inputs:") {
                dirtied_material = true;
                result.insert(HdMaterialSchema::get_default_locator());
                result.insert(HdLightSchema::get_default_locator());
                continue;
            }

            if !dirtied_light && !dirtied_material && property_name.starts_with("light:") {
                dirtied_light = true;
                result.insert(HdLightSchema::get_default_locator());
            }
        }

        result
    }
}

/// Handle type for LightAPIAdapter.
pub type LightAPIAdapterHandle = Arc<LightAPIAdapter>;

/// Factory for creating light API adapters.
pub fn create_light_api_adapter() -> Arc<dyn APISchemaAdapter> {
    Arc::new(LightAPIAdapter::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_core::Stage;

    #[test]
    fn test_light_api_adapter() {
        let adapter = LightAPIAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let empty = Token::new("");

        assert!(adapter.get_imaging_subprims(&prim, &empty).is_empty());
    }

    #[test]
    fn test_light_api_invalidation() {
        let adapter = LightAPIAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let properties = vec![Token::new("inputs:intensity")];

        let locators = adapter.invalidate_imaging_subprim(
            &prim,
            &Token::new(""),
            &Token::new(""),
            &properties,
            PropertyInvalidationType::PropertyChanged,
        );

        assert!(!locators.is_empty());
    }

    #[test]
    fn test_factory() {
        let _ = create_light_api_adapter();
    }
}
