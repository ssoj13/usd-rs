//! CollectionAPIAdapter - API schema adapter for UsdCollectionAPI.
//!
//! Port of pxr/usdImaging/usdImaging/collectionAPIAdapter.h/.cpp

use super::api_schema_adapter::APISchemaAdapter;
use super::data_source_stage_globals::DataSourceStageGlobalsHandle;
use super::types::PropertyInvalidationType;
use std::sync::Arc;
use usd_core::{collection_api::CollectionAPI, Prim};
use usd_hd::data_source::HdRetainedTypedSampledDataSource;
use usd_hd::schema::HdCollectionsSchema;
use usd_hd::{HdContainerDataSource, HdDataSourceBaseHandle, HdDataSourceLocatorSet};
use usd_sdf::PathExpression;
use usd_tf::Token;

#[derive(Debug, Clone)]
struct CollectionContainerDataSource {
    api: CollectionAPI,
}

impl CollectionContainerDataSource {
    fn new(api: CollectionAPI) -> Arc<Self> {
        Arc::new(Self { api })
    }
}

impl usd_hd::HdDataSourceBase for CollectionContainerDataSource {
    fn clone_box(&self) -> usd_hd::HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for CollectionContainerDataSource {
    fn get_names(&self) -> Vec<Token> {
        vec![Token::new("membershipExpression")]
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if name.as_str() == "membershipExpression" {
            return Some(
                HdRetainedTypedSampledDataSource::<PathExpression>::new(
                    self.api.resolve_complete_membership_expression(),
                ) as HdDataSourceBaseHandle,
            );
        }
        None
    }
}

#[derive(Debug, Clone)]
struct CollectionsContainerDataSource {
    api: CollectionAPI,
}

impl CollectionsContainerDataSource {
    fn new(prim: Prim, name: Token) -> Arc<Self> {
        Arc::new(Self {
            api: CollectionAPI::new(prim, name),
        })
    }
}

impl usd_hd::HdDataSourceBase for CollectionsContainerDataSource {
    fn clone_box(&self) -> usd_hd::HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for CollectionsContainerDataSource {
    fn get_names(&self) -> Vec<Token> {
        self.api.name().cloned().into_iter().collect()
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if self.api.name() == Some(name) {
            return Some(CollectionContainerDataSource::new(self.api.clone()) as HdDataSourceBaseHandle);
        }
        None
    }
}

#[derive(Debug, Clone)]
pub struct CollectionAPIAdapter;

impl Default for CollectionAPIAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl CollectionAPIAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl APISchemaAdapter for CollectionAPIAdapter {
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
        _stage_globals: &DataSourceStageGlobalsHandle,
    ) -> Option<usd_hd::HdContainerDataSourceHandle> {
        if !subprim.is_empty() || applied_instance_name.is_empty() {
            return None;
        }

        Some(usd_hd::HdRetainedContainerDataSource::from_entries(&[(
            (**HdCollectionsSchema::get_schema_token()).clone(),
            CollectionsContainerDataSource::new(prim.clone(), applied_instance_name.clone())
                as HdDataSourceBaseHandle,
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
        if !subprim.is_empty() || applied_instance_name.is_empty() {
            return HdDataSourceLocatorSet::empty();
        }

        let prefix = format!("collection:{}:", applied_instance_name.as_str());
        for property_name in properties {
            if property_name.as_str().starts_with(&prefix) {
                let mut locators = HdDataSourceLocatorSet::empty();
                locators.insert(usd_hd::HdDataSourceLocator::from_tokens_2(
                    (**HdCollectionsSchema::get_schema_token()).clone(),
                    applied_instance_name.clone(),
                ));
                return locators;
            }
        }

        HdDataSourceLocatorSet::empty()
    }
}

pub type CollectionAPIAdapterHandle = Arc<CollectionAPIAdapter>;

pub fn create_collection_api_adapter() -> Arc<dyn APISchemaAdapter> {
    Arc::new(CollectionAPIAdapter::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_core::Stage;

    #[test]
    fn test_collection_api_adapter() {
        let adapter = CollectionAPIAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let empty = Token::new("");

        assert!(adapter.get_imaging_subprims(&prim, &empty).is_empty());
    }

    #[test]
    fn test_collection_invalidation() {
        let adapter = CollectionAPIAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let instance_name = Token::new("lights");
        let properties = vec![Token::new("collection:lights:includes")];

        let locators = adapter.invalidate_imaging_subprim(
            &prim,
            &Token::new(""),
            &instance_name,
            &properties,
            PropertyInvalidationType::PropertyChanged,
        );

        assert!(!locators.is_empty());
    }
}
