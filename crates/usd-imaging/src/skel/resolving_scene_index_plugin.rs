//! UsdSkelImagingResolvingSceneIndexPlugin - Scene index plugin for skeletal skinning.
//!
//! Port of pxr/usdImaging/usdSkelImaging/resolvingSceneIndexPlugin.h/cpp
//!
//! Chains SkeletonResolvingSceneIndex and PointsResolvingSceneIndex.

use super::BindingSchema;
use super::points_resolving_scene_index::PointsResolvingSceneIndex;
use super::skeleton_resolving_scene_index::SkeletonResolvingSceneIndex;
use crate::scene_index_plugin::UsdImagingSceneIndexPlugin;
use usd_hd::data_source::HdContainerDataSourceHandle;
use usd_hd::scene_index::flattening::ProviderDataSource;
use usd_hd::scene_index::{HdSceneIndexHandle, scene_index_to_handle};
use usd_hd::schema::HdPrimvarsSchema;
use usd_hd::{
    HdFlattenedOverlayDataSourceProvider, HdRetainedContainerDataSource,
    skinning_settings::is_skinning_deferred,
};
use usd_tf::Token;

/// Plugin that appends UsdSkelImaging resolving scene indices to the pipeline.
///
/// Order: SkeletonResolvingSceneIndex -> PointsResolvingSceneIndex
pub struct ResolvingSceneIndexPlugin;

impl UsdImagingSceneIndexPlugin for ResolvingSceneIndexPlugin {
    fn append_scene_index(&self, input_scene: HdSceneIndexHandle) -> HdSceneIndexHandle {
        let skeleton_resolved = SkeletonResolvingSceneIndex::new(input_scene.clone());
        let skeleton_handle = scene_index_to_handle(skeleton_resolved);
        let points_resolved = PointsResolvingSceneIndex::new(skeleton_handle.clone());
        scene_index_to_handle(points_resolved)
    }

    fn flattened_data_source_providers(&self) -> HdContainerDataSourceHandle {
        HdRetainedContainerDataSource::new_1(
            BindingSchema::get_schema_token(),
            ProviderDataSource::new(HdFlattenedOverlayDataSourceProvider::new_handle()),
        )
    }

    fn instance_data_source_names(&self) -> Vec<Token> {
        vec![BindingSchema::get_schema_token()]
    }

    fn proxy_path_translation_data_source_names(&self) -> Vec<Token> {
        if !is_skinning_deferred() {
            return vec![BindingSchema::get_schema_token()];
        }
        vec![
            BindingSchema::get_schema_token(),
            (*HdPrimvarsSchema::get_schema_token()).clone(),
        ]
    }
}
