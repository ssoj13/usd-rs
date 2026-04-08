//! RenderMan camera API adapter.

use std::sync::{Arc, LazyLock};

use crate::api_schema_adapter::APISchemaAdapter;
use crate::data_source_attribute::DataSourceAttribute;
use crate::data_source_mapped::{
    AttributeMapping, DataSourceMapped, PropertyMapping, PropertyMappings,
};
use crate::data_source_stage_globals::DataSourceStageGlobalsHandle;
use crate::types::PropertyInvalidationType;
use usd_core::{Attribute, Prim, SchemaRegistry};
use usd_hd::schema::HdCameraSchema;
use usd_hd::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdDataSourceLocator,
    HdDataSourceLocatorSet, HdRetainedContainerDataSource,
};
use usd_tf::Token;
use usd_vt::Value;

fn split_namespace(name: &Token) -> Option<(Token, Token)> {
    let text = name.as_str();
    let (ns, prop) = text.split_once(':')?;
    if prop.is_empty() {
        None
    } else {
        Some((Token::new(ns), Token::new(prop)))
    }
}

fn authored_attribute_factory(
    attr: Attribute,
    stage_globals: DataSourceStageGlobalsHandle,
    scene_index_path: usd_sdf::Path,
    locator: HdDataSourceLocator,
) -> Option<HdDataSourceBaseHandle> {
    Some(DataSourceAttribute::<Value>::new_with_locator(
        attr,
        stage_globals,
        scene_index_path,
        locator,
    ) as HdDataSourceBaseHandle)
}

fn get_mappings() -> &'static PropertyMappings {
    static MAPPINGS: LazyLock<PropertyMappings> = LazyLock::new(|| {
        let registry = SchemaRegistry::get_instance();
        let schema_name = Token::new("PxrCameraAPI");
        let mut mappings = Vec::new();
        if let Some(prim_def) = registry.find_applied_api_prim_definition(&schema_name) {
            for usd_name in prim_def.property_names() {
                let prop = prim_def.get_property_definition(usd_name);
                if !prop.is_attribute() {
                    continue;
                }
                let Some((ns, prop_name)) = split_namespace(usd_name) else {
                    continue;
                };
                mappings.push(PropertyMapping::Attribute(
                    AttributeMapping::new_with_factory(
                        usd_name.clone(),
                        HdDataSourceLocator::new(&[ns, prop_name]),
                        authored_attribute_factory,
                    ),
                ));
            }
        }
        PropertyMappings::new(
            mappings,
            HdCameraSchema::get_namespaced_properties_locator(),
        )
    });
    &MAPPINGS
}

#[derive(Debug, Clone, Default)]
pub struct PxrCameraAPIAdapter;

impl PxrCameraAPIAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl APISchemaAdapter for PxrCameraAPIAdapter {
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

        let camera = HdCameraSchema::build_retained(
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(Arc::new(DataSourceMapped::new(
                prim.clone(),
                prim.path().clone(),
                get_mappings().clone(),
                stage_globals.clone(),
            ))),
        );
        Some(HdRetainedContainerDataSource::from_entries(&[(
            (*HdCameraSchema::get_schema_token()).clone(),
            camera as HdDataSourceBaseHandle,
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
        if !subprim.is_empty() || !applied_instance_name.is_empty() {
            return HdDataSourceLocatorSet::empty();
        }
        DataSourceMapped::invalidate(properties, get_mappings())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source_stage_globals::NoOpStageGlobals;
    use usd_core::Stage;

    #[test]
    fn test_camera_api_adapter_creation() {
        let adapter = PxrCameraAPIAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll).unwrap();
        let globals: DataSourceStageGlobalsHandle = Arc::new(NoOpStageGlobals::default());
        let data = adapter.get_imaging_subprim_data(
            &stage.get_pseudo_root(),
            &Token::new(""),
            &Token::new(""),
            &globals,
        );
        assert!(data.is_some());
    }
}
