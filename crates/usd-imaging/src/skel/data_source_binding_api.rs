//! DataSourceBindingAPI - Container data source for UsdSkel::SkelBindingAPI.
//!
//! Port of pxr/usdImaging/usdSkelImaging/dataSourceBindingAPI.h/cpp
//!
//! Provides skelBinding schema with skeleton, animationSource, joints,
//! blendShapes, blendShapeTargets mappings.

use super::binding_schema::BindingSchema;
use crate::{
    data_source_attribute::DataSourceAttribute,
    data_source_mapped::{
        AttributeMapping, DataSourceMapped, PropertyMapping, PropertyMappings, RelationshipMapping,
    },
    data_source_stage_globals::DataSourceStageGlobalsHandle,
    types::PropertyInvalidationType,
};
use std::sync::Arc;
use usd_core::Prim;
use usd_hd::{HdContainerDataSource, HdDataSourceBaseHandle, HdDataSourceLocator};
use usd_sdf::Path;
use usd_skel::tokens::tokens;
use usd_tf::Token;
use usd_vt::Value;

fn authored_attribute_factory(
    attr: usd_core::Attribute,
    stage_globals: DataSourceStageGlobalsHandle,
    scene_index_path: Path,
    locator: HdDataSourceLocator,
) -> Option<HdDataSourceBaseHandle> {
    if !attr.has_authored_value() {
        return None;
    }
    Some(DataSourceAttribute::<Value>::new_with_locator(
        attr,
        stage_globals,
        scene_index_path,
        locator,
    ) as HdDataSourceBaseHandle)
}

fn authored_path_relationship_factory(
    rel: usd_core::Relationship,
    _stage_globals: DataSourceStageGlobalsHandle,
    _scene_index_path: Path,
    _locator: HdDataSourceLocator,
) -> Option<HdDataSourceBaseHandle> {
    if !rel.has_authored_targets() {
        return None;
    }
    rel.get_forwarded_targets().into_iter().next().map(|path| {
        usd_hd::HdRetainedTypedSampledDataSource::<Path>::new(path) as HdDataSourceBaseHandle
    })
}

fn get_binding_property_mappings() -> PropertyMappings {
    let skel_tokens = tokens();

    let mappings = vec![
        PropertyMapping::Relationship(RelationshipMapping::new_with_factory(
            skel_tokens.skel_animation_source.clone(),
            HdDataSourceLocator::from_token(Token::new("animationSource")),
            authored_path_relationship_factory,
        )),
        PropertyMapping::Relationship(RelationshipMapping::new_with_factory(
            skel_tokens.skel_skeleton.clone(),
            HdDataSourceLocator::from_token(Token::new("skeleton")),
            authored_path_relationship_factory,
        )),
        PropertyMapping::Attribute(AttributeMapping::new_with_factory(
            skel_tokens.skel_joints.clone(),
            HdDataSourceLocator::from_token(Token::new("joints")),
            authored_attribute_factory,
        )),
        PropertyMapping::Attribute(AttributeMapping::new(
            skel_tokens.skel_blend_shapes.clone(),
            HdDataSourceLocator::from_token(Token::new("blendShapes")),
        )),
        PropertyMapping::Relationship(RelationshipMapping::new_path_array(
            skel_tokens.skel_blend_shape_targets.clone(),
            HdDataSourceLocator::from_token(Token::new("blendShapeTargets")),
        )),
    ];
    PropertyMappings::new(mappings, BindingSchema::get_default_locator())
}

static BINDING_MAPPINGS: std::sync::LazyLock<PropertyMappings> =
    std::sync::LazyLock::new(get_binding_property_mappings);

/// Data source for UsdSkel::SkelBindingAPI (API schema).
///
/// Implements HdContainerDataSource with single child "skelBinding"
/// providing skeleton, animationSource, joints, blendShapes,
/// blendShapeTargets from the binding API.
#[derive(Clone)]
pub struct DataSourceBindingAPI {
    prim: Prim,
    scene_index_path: Path,
    stage_globals: DataSourceStageGlobalsHandle,
}

impl std::fmt::Debug for DataSourceBindingAPI {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceBindingAPI").finish()
    }
}

impl DataSourceBindingAPI {
    /// Create new binding API data source.
    pub fn new(
        scene_index_path: Path,
        prim: Prim,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Arc<Self> {
        Arc::new(Self {
            prim,
            scene_index_path,
            stage_globals,
        })
    }

    /// Compute invalidation locators for property changes.
    pub fn invalidate(
        _prim: &Prim,
        _subprim: &Token,
        properties: &[Token],
        _invalidation_type: PropertyInvalidationType,
    ) -> usd_hd::HdDataSourceLocatorSet {
        DataSourceMapped::invalidate(properties, &BINDING_MAPPINGS)
    }
}

impl usd_hd::HdDataSourceBase for DataSourceBindingAPI {
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

impl HdContainerDataSource for DataSourceBindingAPI {
    fn get_names(&self) -> Vec<Token> {
        vec![BindingSchema::get_schema_token()]
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if *name == BindingSchema::get_schema_token() {
            let mapped = DataSourceMapped::new(
                self.prim.clone(),
                self.scene_index_path.clone(),
                BINDING_MAPPINGS.clone(),
                self.stage_globals.clone(),
            );
            return Some(Arc::new(mapped) as HdDataSourceBaseHandle);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source_stage_globals::NoOpStageGlobals;
    use usd_core::Stage;

    #[test]
    fn test_binding_api_creation() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let path = Path::from_string("/Mesh").unwrap();
        let globals = Arc::new(NoOpStageGlobals::default());

        let ds = DataSourceBindingAPI::new(path, prim, globals);
        let names = ds.get_names();
        assert_eq!(names.len(), 1);
        assert_eq!(names[0].as_str(), "skelBinding");
    }
}
