#![allow(dead_code)]
//! Flattened data source providers registry for UsdImaging.
//!
//! Port of pxr/usdImaging/usdImaging/flattenedDataSourceProviders.cpp
//!
//! Contains all HdFlattenedDataSourceProviders needed for flattening
//! the output of UsdImagingStageSceneIndex.
//!
//! Can be given as inputArgs to HdFlatteningSceneIndex.

use crate::flattened_geom_model_data_source_provider::FlattenedGeomModelDataSourceProvider;
use crate::flattened_material_bindings_data_source_provider::FlattenedMaterialBindingsDataSourceProvider;
use crate::geom_model_schema::GeomModelSchema;
use crate::material_bindings_schema::MaterialBindingsSchema;
use crate::model_schema::ModelSchema;
use crate::scene_index_plugin::UsdImagingSceneIndexPluginRegistry;
use std::sync::{Arc, LazyLock};
use usd_hd::data_source::{HdDataSourceBaseHandle, HdOverlayContainerDataSource};
use usd_hd::scene_index::flattening::ProviderDataSource;
use usd_hd::{
    HdContainerDataSourceHandle, HdFlattenedDataSourceProviderHandle,
    HdFlattenedOverlayDataSourceProvider, HdRetainedContainerDataSource,
    hd_flattened_data_source_providers,
};

/// Returns container data source with all USD-specific flattened data source providers.
///
/// Per C++ _UsdFlattenedDataSourceProviders(): wraps each provider in a
/// ProviderDataSource and registers under the corresponding schema token.
///
/// Entries:
/// - materialBindings -> FlattenedMaterialBindingsDataSourceProvider
/// - geomModel -> FlattenedGeomModelDataSourceProvider
/// - model -> HdFlattenedOverlayDataSourceProvider (overlay semantics)
fn usd_flattened_data_source_providers() -> HdContainerDataSourceHandle {
    let mat_token = MaterialBindingsSchema::get_schema_token();
    let mat_provider: HdDataSourceBaseHandle =
        ProviderDataSource::new(Arc::new(FlattenedMaterialBindingsDataSourceProvider::new()));

    let geom_token = GeomModelSchema::get_schema_token();
    let geom_provider: HdDataSourceBaseHandle =
        ProviderDataSource::new(Arc::new(FlattenedGeomModelDataSourceProvider::new()));

    let model_token = ModelSchema::get_schema_token();
    let model_provider: HdDataSourceBaseHandle =
        ProviderDataSource::new(HdFlattenedOverlayDataSourceProvider::new_handle());

    HdRetainedContainerDataSource::new_3(
        mat_token,
        mat_provider,
        geom_token,
        geom_provider,
        model_token,
        model_provider,
    )
}

/// Global singleton for flattened data source providers.
///
/// Returns all HdFlattenedDataSourceProviders needed for flattening
/// the output of UsdImagingStageSceneIndex.
///
/// Overlays three layers (strongest first):
/// 1. USD-specific providers (materialBindings, geomModel, model)
/// 2. Plugin providers (e.g. UsdSkelImaging)
/// 3. Basic Hydra providers (xform, visibility, purpose, primvars)
///
/// This can be passed as inputArgs to HdFlatteningSceneIndex.
pub fn usd_imaging_flattened_data_source_providers() -> HdContainerDataSourceHandle {
    static PROVIDERS: LazyLock<HdContainerDataSourceHandle> = LazyLock::new(|| {
        let mut containers: Vec<HdContainerDataSourceHandle> = Vec::new();

        // 1. USD-specific flattening (materialBindings, geomModel, model)
        containers.push(usd_flattened_data_source_providers());

        // 2. Flattening from UsdImaging scene index plugins (e.g. UsdSkelImaging)
        let plugin_providers =
            UsdImagingSceneIndexPluginRegistry::flattened_data_source_providers_from_plugins();
        if !plugin_providers.get_names().is_empty() {
            containers.push(plugin_providers);
        }

        // 3. Basic Hydra flattening (xform, visibility, purpose, primvars)
        let hd_providers = hd_flattened_data_source_providers();
        let hd_container =
            usd_hd::scene_index::flattening::make_flattening_input_args(&hd_providers);
        containers.push(hd_container);

        if containers.len() == 1 {
            containers.into_iter().next().unwrap()
        } else {
            HdOverlayContainerDataSource::new(containers)
        }
    });

    PROVIDERS.clone()
}

/// Get material bindings flattened data source provider.
///
/// Returns a provider that aggregates material bindings from ancestors.
pub fn get_material_bindings_provider() -> HdFlattenedDataSourceProviderHandle {
    static PROVIDER: LazyLock<HdFlattenedDataSourceProviderHandle> =
        LazyLock::new(|| Arc::new(FlattenedMaterialBindingsDataSourceProvider::new()));
    PROVIDER.clone()
}

/// Get geom model flattened data source provider.
///
/// Returns a provider that handles draw mode inheritance.
pub fn get_geom_model_provider() -> HdFlattenedDataSourceProviderHandle {
    static PROVIDER: LazyLock<HdFlattenedDataSourceProviderHandle> =
        LazyLock::new(|| Arc::new(FlattenedGeomModelDataSourceProvider::new()));
    PROVIDER.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_usd_providers_has_three_entries() {
        let providers = usd_flattened_data_source_providers();
        let names = providers.get_names();
        // Must contain materialBindings, geomModel, model
        assert_eq!(
            names.len(),
            3,
            "Expected 3 USD providers, got {}",
            names.len()
        );

        let name_strs: Vec<&str> = names.iter().map(|t| t.as_str()).collect();
        assert!(
            name_strs.contains(&"usdMaterialBindings"),
            "Missing materialBindings"
        );
        assert!(name_strs.contains(&"geomModel"), "Missing geomModel");
        assert!(name_strs.contains(&"model"), "Missing model");
    }

    #[test]
    fn test_providers_includes_hydra_basics() {
        let providers = usd_imaging_flattened_data_source_providers();
        let names = providers.get_names();
        // Should include both USD providers and Hydra basics (xform, visibility, purpose, primvars)
        let name_strs: Vec<&str> = names.iter().map(|t| t.as_str()).collect();
        assert!(
            name_strs.contains(&"xform"),
            "Missing xform from Hydra providers"
        );
        assert!(
            name_strs.contains(&"visibility"),
            "Missing visibility from Hydra providers"
        );
        assert!(
            name_strs.contains(&"model"),
            "Missing model from USD providers"
        );
    }

    #[test]
    fn test_providers_singleton() {
        let providers1 = usd_imaging_flattened_data_source_providers();
        let providers2 = usd_imaging_flattened_data_source_providers();

        let names1 = providers1.get_names();
        let names2 = providers2.get_names();
        assert_eq!(names1.len(), names2.len());
    }

    #[test]
    fn test_individual_providers() {
        let _mat_provider = get_material_bindings_provider();
        let _geom_provider = get_geom_model_provider();
    }
}
