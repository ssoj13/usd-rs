//! RenderMan camera projection API adapter.
//!
//! Port of `pxr/usdImaging/usdRiPxrImaging/pxrCameraProjectionAPIAdapter.h/cpp`.

use crate::api_schema_adapter::APISchemaAdapter;
use crate::data_source_mapped::{
    AttributeMapping, DataSourceMapped, PropertyMapping, PropertyMappings,
};
use crate::data_source_stage_globals::DataSourceStageGlobalsHandle;
use crate::types::PropertyInvalidationType;
use std::sync::{Arc, LazyLock};
use usd_core::{Attribute, Prim, SchemaRegistry};
use usd_hd::data_source::HdRetainedTypedSampledDataSource;
use usd_hd::schema::{
    HdCameraSchema, HdDependenciesSchema, HdDependencySchemaBuilder, HdPathDataSourceHandle,
};
use usd_hd::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdDataSourceLocator,
    HdDataSourceLocatorSet, HdRetainedContainerDataSource, HdTypedSampledDataSource,
    hd_container_get,
};
use usd_sdf::Path;
use usd_shade::Output;
use usd_tf::Token;

mod tokens {
    use super::*;

    pub static PXR_CAMERA_PROJECTION_API: LazyLock<Token> =
        LazyLock::new(|| Token::new("PxrCameraProjectionAPI"));
    pub static PROJECTION: LazyLock<Token> = LazyLock::new(|| Token::new("projection"));
    pub static OUTPUTS: LazyLock<Token> = LazyLock::new(|| Token::new("outputs"));
    pub static RI: LazyLock<Token> = LazyLock::new(|| Token::new("ri"));
    pub static PRIM_DEP_PROJECTION_PRIM: LazyLock<Token> =
        LazyLock::new(|| Token::new("prim_dep_projection_prim"));
}

fn split_namespace(name: &Token) -> Option<(Token, Token)> {
    let text = name.as_str();
    let (ns, prop) = text.split_once(':')?;
    if prop.is_empty() {
        None
    } else {
        Some((Token::new(ns), Token::new(prop)))
    }
}

fn connected_path_attribute_factory(
    attr: Attribute,
    _stage_globals: DataSourceStageGlobalsHandle,
    _scene_index_path: Path,
    _locator: HdDataSourceLocator,
) -> Option<HdDataSourceBaseHandle> {
    let output = Output::from_attribute(attr);
    let mut paths = Vec::new();
    let path = if output.get_raw_connected_source_paths(&mut paths) {
        paths
            .first()
            .map(Path::get_prim_path)
            .unwrap_or_else(Path::empty)
    } else {
        Path::empty()
    };
    Some(HdRetainedTypedSampledDataSource::<Path>::new(path) as HdDataSourceBaseHandle)
}

fn get_mappings() -> &'static PropertyMappings {
    static MAPPINGS: LazyLock<PropertyMappings> = LazyLock::new(|| {
        let registry = SchemaRegistry::get_instance();
        let mut mappings = Vec::new();

        if let Some(prim_def) =
            registry.find_applied_api_prim_definition(&tokens::PXR_CAMERA_PROJECTION_API)
        {
            for usd_name in prim_def.property_names() {
                let prop = prim_def.get_property_definition(usd_name);
                if !prop.is_attribute() {
                    continue;
                }
                let Some((mut ns, mut prop_name)) = split_namespace(usd_name) else {
                    continue;
                };
                if ns == *tokens::OUTPUTS {
                    let Some((inner_ns, inner_prop)) = split_namespace(&prop_name) else {
                        continue;
                    };
                    ns = inner_ns;
                    prop_name = inner_prop;
                }
                mappings.push(PropertyMapping::Attribute(
                    AttributeMapping::new_with_factory(
                        usd_name.clone(),
                        HdDataSourceLocator::new(&[ns, prop_name]),
                        connected_path_attribute_factory,
                    ),
                ));
            }
        }

        if !mappings.iter().any(|mapping| {
            matches!(
                mapping,
                PropertyMapping::Attribute(attr)
                    if attr.base.usd_name.as_str() == "outputs:ri:projection"
            )
        }) {
            mappings.push(PropertyMapping::Attribute(
                AttributeMapping::new_with_factory(
                    Token::new("outputs:ri:projection"),
                    HdDataSourceLocator::new(&[tokens::RI.clone(), tokens::PROJECTION.clone()]),
                    connected_path_attribute_factory,
                ),
            ));
        }

        PropertyMappings::new(
            mappings,
            HdCameraSchema::get_namespaced_properties_locator(),
        )
    });
    &MAPPINGS
}

fn projection_locator() -> HdDataSourceLocator {
    HdDataSourceLocator::new(&[
        (*HdCameraSchema::get_schema_token()).clone(),
        Token::new("namespacedProperties"),
        tokens::RI.clone(),
        tokens::PROJECTION.clone(),
    ])
}

fn dependency_locator_handle() -> usd_hd::schema::HdLocatorDataSourceHandle {
    HdRetainedTypedSampledDataSource::<HdDataSourceLocator>::new(HdDataSourceLocator::empty())
}

#[derive(Debug, Clone)]
pub struct PxrCameraProjectionAPIAdapter;

impl Default for PxrCameraProjectionAPIAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl PxrCameraProjectionAPIAdapter {
    /// Create a new PxrCameraProjectionAPI adapter.
    pub fn new() -> Self {
        Self
    }
}

impl APISchemaAdapter for PxrCameraProjectionAPIAdapter {
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

        let namespaced_properties: HdContainerDataSourceHandle = Arc::new(DataSourceMapped::new(
            prim.clone(),
            prim.path().clone(),
            get_mappings().clone(),
            stage_globals.clone(),
        ));
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
            Some(namespaced_properties),
        );

        let mut entries = vec![(
            (*HdCameraSchema::get_schema_token()).clone(),
            camera as HdDataSourceBaseHandle,
        )];
        let full_projection_locator = projection_locator();
        if let Some(projection_ds) = entries[0]
            .1
            .as_container()
            .and_then(|container| hd_container_get(container, &full_projection_locator))
            .and_then(|value| {
                value
                    .as_any()
                    .downcast_ref::<HdRetainedTypedSampledDataSource<Path>>()
                    .map(|ds| ds.clone())
            })
        {
            let projection_path = projection_ds.get_typed_value(0.0);
            if !projection_path.is_empty() {
                let dependency = HdDependencySchemaBuilder::default()
                    .set_depended_on_data_source_locator(dependency_locator_handle())
                    .set_affected_data_source_locator(dependency_locator_handle())
                    .set_depended_on_prim_path(HdRetainedTypedSampledDataSource::<Path>::new(
                        projection_path,
                    ) as HdPathDataSourceHandle)
                    .build();
                let dependencies = HdDependenciesSchema::build_retained(
                    &[tokens::PRIM_DEP_PROJECTION_PRIM.clone()],
                    &[dependency as HdDataSourceBaseHandle],
                );
                entries.push((
                    (*HdDependenciesSchema::get_schema_token()).clone(),
                    dependencies as HdDataSourceBaseHandle,
                ));
            }
        }

        Some(HdRetainedContainerDataSource::from_entries(&entries))
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

        let invalidation = DataSourceMapped::invalidate(properties, get_mappings());
        let full_projection_locator = projection_locator();
        if invalidation.contains(&full_projection_locator) {
            return HdDataSourceLocatorSet::from_locator(HdDataSourceLocator::empty());
        }
        invalidation
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source_stage_globals::NoOpStageGlobals;
    use std::sync::Arc;
    use usd_core::Stage;
    use usd_core::common::InitialLoadSet;

    fn create_test_globals() -> DataSourceStageGlobalsHandle {
        Arc::new(NoOpStageGlobals::default())
    }

    #[test]
    fn test_adapter_creation() {
        let adapter = PxrCameraProjectionAPIAdapter::new();
        let empty_token = Token::new("");
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let prim = stage.get_pseudo_root();
        let globals = create_test_globals();

        assert!(
            adapter
                .get_imaging_subprim_data(&prim, &empty_token, &empty_token, &globals)
                .is_some()
        );
    }

    #[test]
    fn test_outputs_invalidation_resyncs_root() {
        let adapter = PxrCameraProjectionAPIAdapter::new();
        let empty_token = Token::new("");
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
        let prim = stage.get_pseudo_root();

        let locators = adapter.invalidate_imaging_subprim(
            &prim,
            &empty_token,
            &empty_token,
            &[Token::new("outputs:ri:projection")],
            PropertyInvalidationType::Resync,
        );
        assert!(locators.contains(&HdDataSourceLocator::empty()));
    }
}
