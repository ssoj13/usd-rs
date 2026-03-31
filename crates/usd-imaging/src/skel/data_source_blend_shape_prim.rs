//! DataSourceBlendShapePrim - Prim data source for UsdSkel::BlendShape.
//!
//! Port of pxr/usdImaging/usdSkelImaging/dataSourceBlendShapePrim.h/cpp
//!
//! Extends DataSourcePrim with BlendShapeSchema (offsets, normalOffsets,
//! pointIndices, inbetweenShapes).

use super::blend_shape_schema::BlendShapeSchema;
use crate::{
    data_source_mapped::{AttributeMapping, DataSourceMapped, PropertyMapping, PropertyMappings},
    data_source_prim::DataSourcePrim,
    data_source_stage_globals::DataSourceStageGlobalsHandle,
    types::PropertyInvalidationType,
};
use std::sync::Arc;
use usd_core::Prim;
use usd_hd::{HdContainerDataSource, HdDataSourceBaseHandle, HdDataSourceLocator};
use usd_sdf::Path;
use usd_skel::BlendShape;
use usd_tf::Token;

fn get_blend_shape_property_mappings() -> PropertyMappings {
    let mappings: Vec<PropertyMapping> = BlendShape::get_schema_attribute_names(false)
        .into_iter()
        .map(|usd_name| {
            PropertyMapping::Attribute(AttributeMapping::new(
                usd_name.clone(),
                HdDataSourceLocator::from_token(usd_name),
            ))
        })
        .collect();
    PropertyMappings::new(mappings, BlendShapeSchema::get_default_locator())
}

static BLEND_SHAPE_MAPPINGS: std::sync::LazyLock<PropertyMappings> =
    std::sync::LazyLock::new(get_blend_shape_property_mappings);

/// Data source for UsdSkel::BlendShape prims.
///
/// Extends DataSourcePrim with BlendShapeSchema (offsets, normalOffsets,
/// pointIndices). Inbetween shapes support requires additional
/// _InbetweenShapeContainerSchemaDataSource implementation.
#[derive(Clone)]
pub struct DataSourceBlendShapePrim {
    base: DataSourcePrim,
}

impl std::fmt::Debug for DataSourceBlendShapePrim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceBlendShapePrim").finish()
    }
}

impl DataSourceBlendShapePrim {
    /// Create new blend shape prim data source.
    pub fn new(
        scene_index_path: Path,
        prim: Prim,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Arc<Self> {
        Arc::new(Self {
            base: DataSourcePrim::new(prim, scene_index_path, stage_globals),
        })
    }

    /// Get the base prim data source.
    pub fn base(&self) -> &DataSourcePrim {
        &self.base
    }

    /// Compute invalidation locators for property changes.
    pub fn invalidate(
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> usd_hd::HdDataSourceLocatorSet {
        let mut locators = DataSourcePrim::invalidate(prim, subprim, properties, invalidation_type);
        locators.insert_set(&DataSourceMapped::invalidate(
            properties,
            &BLEND_SHAPE_MAPPINGS,
        ));
        locators
    }
}

impl usd_hd::HdDataSourceBase for DataSourceBlendShapePrim {
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

impl HdContainerDataSource for DataSourceBlendShapePrim {
    fn get_names(&self) -> Vec<Token> {
        let mut names = self.base.get_names();
        if !names
            .iter()
            .any(|n| n == &BlendShapeSchema::get_schema_token())
        {
            names.push(BlendShapeSchema::get_schema_token());
        }
        names
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if *name == BlendShapeSchema::get_schema_token() {
            let mapped = DataSourceMapped::new(
                self.base.prim().clone(),
                self.base.hydra_path().clone(),
                BLEND_SHAPE_MAPPINGS.clone(),
                self.base.stage_globals().clone(),
            );
            return Some(Arc::new(mapped) as HdDataSourceBaseHandle);
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
    fn test_blend_shape_prim_creation() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let path = Path::from_string("/BlendShape").unwrap();
        let globals = Arc::new(NoOpStageGlobals::default());

        let ds = DataSourceBlendShapePrim::new(path, prim, globals);
        let names = ds.get_names();
        assert!(names.iter().any(|n| n == "skelBlendShape"));
    }
}
