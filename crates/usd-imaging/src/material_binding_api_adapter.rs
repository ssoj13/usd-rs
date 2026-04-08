//! MaterialBindingAPIAdapter - API schema adapter for UsdShadeMaterialBindingAPI.
//!
//! Port of pxr/usdImaging/usdImaging/materialBindingAPIAdapter.h/cpp

use super::api_schema_adapter::APISchemaAdapter;
use super::collection_material_binding_schema::CollectionMaterialBindingSchemaBuilder;
use super::data_source_stage_globals::DataSourceStageGlobalsHandle;
use super::direct_material_binding_schema::DirectMaterialBindingSchemaBuilder;
use super::material_bindings_schema::MaterialBindingsSchema;
use super::types::PropertyInvalidationType;
use std::sync::Arc;
use usd_core::Prim;
use usd_hd::{
    HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
    HdDataSourceLocatorSet, HdRetainedContainerDataSource, HdRetainedSmallVectorDataSource,
    HdRetainedTypedSampledDataSource,
};
use usd_sdf::Path;
use usd_shade::material_binding_api::MaterialBindingAPI;
use usd_tf::Token;

fn build_direct_material_binding_data_source(
    mb_api: &MaterialBindingAPI,
    purpose: &Token,
) -> Option<HdDataSourceBaseHandle> {
    let binding = mb_api.get_direct_binding(purpose);
    if !binding.is_bound() {
        return None;
    }

    Some(
        DirectMaterialBindingSchemaBuilder::new()
            .set_material_path(HdRetainedTypedSampledDataSource::new(
                binding.get_material_path().clone(),
            ) as HdDataSourceBaseHandle)
            .set_binding_strength(HdRetainedTypedSampledDataSource::new(
                MaterialBindingAPI::get_material_binding_strength(binding.get_binding_rel()),
            ) as HdDataSourceBaseHandle)
            .build() as HdDataSourceBaseHandle,
    )
}

fn build_collection_bindings_vector_data_source(
    mb_api: &MaterialBindingAPI,
    purpose: &Token,
) -> Option<HdDataSourceBaseHandle> {
    let bindings = mb_api.get_collection_bindings(purpose);
    if bindings.is_empty() {
        return None;
    }

    let mut entries: Vec<HdDataSourceBaseHandle> = Vec::new();
    entries.reserve(bindings.len());

    for binding in bindings {
        if !binding.is_valid() {
            continue;
        }

        let collection_name = {
            let prop_name = binding.get_collection_path().get_name();
            let (stripped, matched) = Path::strip_prefix_namespace(prop_name, "collection");
            Token::new(if matched {
                stripped.as_str()
            } else {
                prop_name
            })
        };

        entries.push(
            CollectionMaterialBindingSchemaBuilder::new()
                .set_collection_prim_path(HdRetainedTypedSampledDataSource::new(
                    binding.get_collection_path().get_prim_path(),
                ) as HdDataSourceBaseHandle)
                .set_collection_name(HdRetainedTypedSampledDataSource::new(collection_name)
                    as HdDataSourceBaseHandle)
                .set_material_path(HdRetainedTypedSampledDataSource::new(
                    binding.get_material_path().clone(),
                ) as HdDataSourceBaseHandle)
                .set_binding_strength(HdRetainedTypedSampledDataSource::new(
                    MaterialBindingAPI::get_material_binding_strength(binding.get_binding_rel()),
                ) as HdDataSourceBaseHandle)
                .build() as HdDataSourceBaseHandle,
        );
    }

    if entries.is_empty() {
        return None;
    }

    Some(HdRetainedSmallVectorDataSource::new(&entries) as HdDataSourceBaseHandle)
}

#[derive(Clone, Debug)]
struct MaterialBindingContainerDataSource {
    mb_api: MaterialBindingAPI,
    purpose: Token,
}

impl MaterialBindingContainerDataSource {
    fn new(mb_api: MaterialBindingAPI, purpose: Token) -> Arc<Self> {
        Arc::new(Self { mb_api, purpose })
    }
}

impl HdDataSourceBase for MaterialBindingContainerDataSource {
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

impl HdContainerDataSource for MaterialBindingContainerDataSource {
    fn get_names(&self) -> Vec<Token> {
        vec![
            Token::new("directMaterialBinding"),
            Token::new("collectionMaterialBindings"),
        ]
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if name.as_str() == "directMaterialBinding" {
            return build_direct_material_binding_data_source(&self.mb_api, &self.purpose);
        }
        if name.as_str() == "collectionMaterialBindings" {
            return build_collection_bindings_vector_data_source(&self.mb_api, &self.purpose);
        }
        None
    }
}

#[derive(Clone, Debug)]
struct MaterialBindingsContainerDataSource {
    mb_api: MaterialBindingAPI,
}

impl MaterialBindingsContainerDataSource {
    fn new(mb_api: MaterialBindingAPI) -> Arc<Self> {
        Arc::new(Self { mb_api })
    }
}

impl HdDataSourceBase for MaterialBindingsContainerDataSource {
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

impl HdContainerDataSource for MaterialBindingsContainerDataSource {
    fn get_names(&self) -> Vec<Token> {
        MaterialBindingAPI::get_material_purposes()
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        let has_direct = self.mb_api.get_direct_binding(name).is_bound();
        let has_collection = !self.mb_api.get_collection_bindings(name).is_empty();
        if !has_direct && !has_collection {
            return None;
        }

        Some(
            HdRetainedSmallVectorDataSource::new(&[MaterialBindingContainerDataSource::new(
                self.mb_api.clone(),
                name.clone(),
            ) as HdDataSourceBaseHandle]) as HdDataSourceBaseHandle,
        )
    }
}

/// API schema adapter for UsdShadeMaterialBindingAPI.
#[derive(Debug, Clone)]
pub struct MaterialBindingAPIAdapter;

impl Default for MaterialBindingAPIAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl MaterialBindingAPIAdapter {
    /// Create a new material binding API adapter.
    pub fn new() -> Self {
        Self
    }
}

impl APISchemaAdapter for MaterialBindingAPIAdapter {
    fn get_imaging_subprims(&self, _prim: &Prim, _applied_instance_name: &Token) -> Vec<Token> {
        vec![]
    }

    fn get_imaging_subprim_type(
        &self,
        _prim: &Prim,
        _subprim: &Token,
        _applied_instance_name: &Token,
    ) -> Token {
        Token::default()
    }

    fn get_imaging_subprim_data(
        &self,
        prim: &Prim,
        subprim: &Token,
        applied_instance_name: &Token,
        _stage_globals: &DataSourceStageGlobalsHandle,
    ) -> Option<HdContainerDataSourceHandle> {
        if !subprim.is_empty() || !applied_instance_name.is_empty() {
            return None;
        }

        // Only create materialBindings data source if the prim has a material:binding.
        // Full keyless support crashes due to RwLock reentrancy in scene index chain.
        // TODO: fix RwLock deadlock and enable keyless adapter (C++ parity)
        let binding_api = MaterialBindingAPI::new(prim.clone());
        let all_purpose = Token::new("");
        let rel = binding_api.get_direct_binding_rel(&all_purpose);
        if !rel.is_valid() {
            return None;
        }
        Some(HdRetainedContainerDataSource::new_1(
            MaterialBindingsSchema::get_schema_token(),
            MaterialBindingsContainerDataSource::new(binding_api) as HdDataSourceBaseHandle,
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

        let locator = MaterialBindingsSchema::get_default_locator();
        for prop in properties {
            let prop_name = prop.as_str();
            if prop_name.starts_with("material:binding:collection")
                || prop_name.starts_with("material:binding")
                || prop_name.starts_with("collection")
            {
                return HdDataSourceLocatorSet::from_locator(locator);
            }
        }

        HdDataSourceLocatorSet::empty()
    }
}

/// Handle type for MaterialBindingAPIAdapter.
pub type MaterialBindingAPIAdapterHandle = Arc<MaterialBindingAPIAdapter>;

/// Factory for creating material binding API adapters.
pub fn create_material_binding_api_adapter() -> Arc<dyn APISchemaAdapter> {
    Arc::new(MaterialBindingAPIAdapter::new())
}

/// Create adapter compatible with AdapterManager's ApiSchemaAdapter trait.
pub fn create_default() -> crate::adapter_manager::ApiSchemaAdapterHandle {
    Arc::new(MaterialBindingAPIAdapter::new())
}

// Bridge: implement AdapterManager's ApiSchemaAdapter for MaterialBindingAPIAdapter
// so it can be registered and used in the scene index populate/get_prim pipeline.
impl crate::adapter_manager::ApiSchemaAdapter for MaterialBindingAPIAdapter {
    fn get_schema_name(&self) -> Token {
        Token::new("MaterialBindingAPI")
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

#[cfg(test)]
mod tests {
    use super::*;
    use usd_core::Stage;

    #[test]
    fn test_material_binding_api_adapter() {
        let adapter = MaterialBindingAPIAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let empty = Token::new("");

        assert!(adapter.get_imaging_subprims(&prim, &empty).is_empty());
    }

    #[test]
    fn test_material_binding_invalidation() {
        let adapter = MaterialBindingAPIAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let properties = vec![Token::new("material:binding")];

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
        let _ = create_material_binding_api_adapter();
    }

    #[test]
    fn test_collection_invalidation_hits_material_bindings() {
        let adapter = MaterialBindingAPIAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let properties = vec![Token::new("collection:materialBind:includes")];

        let locators = adapter.invalidate_imaging_subprim(
            &prim,
            &Token::new(""),
            &Token::new(""),
            &properties,
            PropertyInvalidationType::PropertyChanged,
        );

        assert!(!locators.is_empty());
    }
}
