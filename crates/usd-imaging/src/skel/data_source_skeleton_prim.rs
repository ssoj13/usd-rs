//! DataSourceSkeletonPrim - Prim data source for UsdSkel::Skeleton.
//!
//! Port of pxr/usdImaging/usdSkelImaging/dataSourceSkeletonPrim.h/cpp
//!
//! Extends DataSourceGprim with skeleton schema and purpose=guide overlay.

use super::skeleton_schema::SkeletonSchema;
use crate::{
    data_source_gprim::DataSourceGprim,
    data_source_mapped::{AttributeMapping, DataSourceMapped, PropertyMapping, PropertyMappings},
    data_source_stage_globals::DataSourceStageGlobalsHandle,
    types::PropertyInvalidationType,
};
use std::sync::Arc;
use usd_core::Prim;
use usd_hd::{
    HdContainerDataSource, HdDataSourceBaseHandle, HdDataSourceLocatorSet, HdPurposeSchema,
    data_source::HdRetainedTypedSampledDataSource,
};
use usd_sdf::Path;
use usd_skel::Skeleton;
use usd_tf::Token;

fn get_skeleton_property_mappings() -> PropertyMappings {
    use usd_hd::HdDataSourceLocator;
    let mappings: Vec<PropertyMapping> = Skeleton::get_schema_attribute_names(false)
        .into_iter()
        .map(|usd_name| {
            PropertyMapping::Attribute(AttributeMapping::new(
                usd_name.clone(),
                HdDataSourceLocator::from_token(usd_name),
            ))
        })
        .collect();
    PropertyMappings::new(mappings, SkeletonSchema::get_default_locator())
}

static SKELETON_MAPPINGS: std::sync::LazyLock<PropertyMappings> =
    std::sync::LazyLock::new(get_skeleton_property_mappings);

/// Data source for UsdSkel::Skeleton prims.
///
/// Extends DataSourceGprim with:
/// - SkeletonSchema (joints, jointNames, bindTransforms, restTransforms)
/// - purpose overlay to "guide"
#[derive(Clone)]
pub struct DataSourceSkeletonPrim {
    base: Arc<DataSourceGprim>,
}

impl std::fmt::Debug for DataSourceSkeletonPrim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceSkeletonPrim").finish()
    }
}

impl DataSourceSkeletonPrim {
    /// Create new skeleton prim data source.
    pub fn new(
        scene_index_path: Path,
        prim: Prim,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Arc<Self> {
        Arc::new(Self {
            base: DataSourceGprim::new(scene_index_path, prim, stage_globals),
        })
    }

    /// Get the base gprim data source.
    pub fn base(&self) -> &Arc<DataSourceGprim> {
        &self.base
    }

    /// Compute invalidation locators for property changes.
    pub fn invalidate(
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        let mut locators =
            DataSourceGprim::invalidate(prim, subprim, properties, invalidation_type);
        locators.insert_set(&DataSourceMapped::invalidate(
            properties,
            &SKELETON_MAPPINGS,
        ));
        locators
    }
}

impl usd_hd::HdDataSourceBase for DataSourceSkeletonPrim {
    fn clone_box(&self) -> usd_hd::HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for DataSourceSkeletonPrim {
    fn get_names(&self) -> Vec<Token> {
        let mut names = self.base.get_names();
        if !names
            .iter()
            .any(|n| n == &SkeletonSchema::get_schema_token())
        {
            names.push(SkeletonSchema::get_schema_token());
        }
        names
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if *name == SkeletonSchema::get_schema_token() {
            let mapped = DataSourceMapped::new(
                self.base.prim().clone(),
                self.base.scene_index_path().clone(),
                SKELETON_MAPPINGS.clone(),
                self.base.stage_globals().clone(),
            );
            return Some(Arc::new(mapped) as HdDataSourceBaseHandle);
        }
        if name == "purpose" {
            let guide = Token::new("guide");
            let guide_ds = HdRetainedTypedSampledDataSource::new(guide);
            let container = HdPurposeSchema::build_retained(Some(guide_ds));
            return Some(container as HdDataSourceBaseHandle);
        }
        self.base.get(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source_stage_globals::NoOpStageGlobals;
    use usd_core::Stage;

    #[test]
    fn test_skeleton_prim_creation() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let path = Path::from_string("/Skeleton").unwrap();
        let globals = Arc::new(NoOpStageGlobals::default());

        let ds = DataSourceSkeletonPrim::new(path, prim, globals);
        let names = ds.get_names();
        assert!(names.iter().any(|n| n == "skeleton"));
    }
}
