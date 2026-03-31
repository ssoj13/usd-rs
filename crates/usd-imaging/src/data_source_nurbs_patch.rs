//! DataSourceNurbsPatch - NurbsPatch data source for Hydra.
//!
//! Port of pxr/usdImaging/usdImaging/dataSourceNurbsPatch.h/.cpp

use crate::data_source_gprim::DataSourceGprim;
use crate::data_source_mapped::{
    AttributeMapping, DataSourceMapped, PropertyMapping, PropertyMappings,
};
use crate::data_source_primvars::{DataSourceCustomPrimvars, PrimvarMapping};
use crate::data_source_stage_globals::DataSourceStageGlobalsHandle;
use crate::types::PropertyInvalidationType;
use std::sync::Arc;
use std::sync::LazyLock;
use usd_core::Prim;
use usd_geom::nurbs_patch::NurbsPatch;
use usd_geom::tokens::usd_geom_tokens;
use usd_hd::data_source::cast_to_container;
use usd_hd::schema::{HdNurbsPatchSchema, HdPrimvarsSchema};
use usd_hd::{
    HdContainerDataSource, HdDataSourceBaseHandle, HdDataSourceLocator, HdDataSourceLocatorSet,
    HdOverlayContainerDataSource,
};
use usd_sdf::Path;
use usd_tf::Token;

fn to_locator(name: &Token) -> HdDataSourceLocator {
    let tokens = Path::tokenize_identifier_as_tokens(name.as_str());
    HdDataSourceLocator::new(&tokens)
}

fn get_property_mappings() -> Vec<PropertyMapping> {
    let mut result = vec![
        PropertyMapping::Attribute(AttributeMapping::new(
            usd_geom_tokens().double_sided.clone(),
            HdDataSourceLocator::from_token(Token::new("doubleSided")),
        )),
        PropertyMapping::Attribute(AttributeMapping::new(
            usd_geom_tokens().orientation.clone(),
            HdDataSourceLocator::from_token(Token::new("orientation")),
        )),
    ];

    for usd_name in NurbsPatch::get_schema_attribute_names(false) {
        if usd_name == usd_geom_tokens().point_weights {
            continue;
        }
        result.push(PropertyMapping::Attribute(AttributeMapping::new(
            usd_name.clone(),
            to_locator(&usd_name),
        )));
    }

    result
}

fn get_mappings() -> &'static PropertyMappings {
    static MAPPINGS: LazyLock<PropertyMappings> = LazyLock::new(|| {
        PropertyMappings::new(
            get_property_mappings(),
            HdNurbsPatchSchema::get_default_locator(),
        )
    });
    &MAPPINGS
}

fn get_custom_primvar_mappings() -> Vec<PrimvarMapping> {
    vec![PrimvarMapping::new(
        usd_geom_tokens().point_weights.clone(),
        usd_geom_tokens().point_weights.clone(),
    )]
}

pub type DataSourceNurbsPatch = DataSourceMapped;

#[derive(Clone)]
pub struct DataSourceNurbsPatchPrim {
    base: Arc<DataSourceGprim>,
    prim: Prim,
    scene_index_path: Path,
    stage_globals: DataSourceStageGlobalsHandle,
}

impl HdContainerDataSource for DataSourceNurbsPatchPrim {
    fn get_names(&self) -> Vec<Token> {
        Self::get_names(self)
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        Self::get(self, name)
    }
}

impl usd_hd::HdDataSourceBase for DataSourceNurbsPatchPrim {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            base: Arc::clone(&self.base),
            prim: self.prim.clone(),
            scene_index_path: self.scene_index_path.clone(),
            stage_globals: self.stage_globals.clone(),
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(Arc::new(Self {
            base: Arc::clone(&self.base),
            prim: self.prim.clone(),
            scene_index_path: self.scene_index_path.clone(),
            stage_globals: self.stage_globals.clone(),
        }))
    }
}

impl DataSourceNurbsPatchPrim {
    pub fn new(
        scene_index_path: Path,
        prim: Prim,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Self {
        Self {
            base: DataSourceGprim::new(
                scene_index_path.clone(),
                prim.clone(),
                stage_globals.clone(),
            ),
            prim,
            scene_index_path,
            stage_globals,
        }
    }

    pub fn get_names(&self) -> Vec<Token> {
        let mut names = self.base.get_names();
        names.push((**HdNurbsPatchSchema::get_schema_token()).clone());
        names
    }

    pub fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if name == &**HdNurbsPatchSchema::get_schema_token() {
            return Some(Arc::new(DataSourceMapped::new(
                self.prim.clone(),
                self.scene_index_path.clone(),
                get_mappings().clone(),
                self.stage_globals.clone(),
            )) as HdDataSourceBaseHandle);
        }

        if name == &**HdPrimvarsSchema::get_schema_token() {
            let base = self.base.get(name);
            let custom = Arc::new(DataSourceCustomPrimvars::new(
                self.scene_index_path.clone(),
                self.prim.clone(),
                get_custom_primvar_mappings(),
                self.stage_globals.clone(),
            )) as usd_hd::HdContainerDataSourceHandle;

            return Some(match base.as_ref().and_then(cast_to_container) {
                Some(base_container) => {
                    HdOverlayContainerDataSource::new_2(base_container, custom)
                        as HdDataSourceBaseHandle
                }
                None => custom as HdDataSourceBaseHandle,
            });
        }

        self.base.get(name)
    }

    pub fn invalidate(
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        let mut locators = DataSourceMapped::invalidate(properties, get_mappings());
        locators.insert_set(&DataSourceGprim::invalidate(
            prim,
            subprim,
            properties,
            invalidation_type,
        ));
        locators.insert_set(&DataSourceCustomPrimvars::invalidate(
            properties,
            &get_custom_primvar_mappings(),
        ));
        locators
    }
}

impl std::fmt::Debug for DataSourceNurbsPatchPrim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DataSourceNurbsPatchPrim")
    }
}

pub type DataSourceNurbsPatchPrimHandle = Arc<DataSourceNurbsPatchPrim>;

pub fn create_data_source_nurbs_patch_prim(
    scene_index_path: Path,
    prim: Prim,
    stage_globals: DataSourceStageGlobalsHandle,
) -> DataSourceNurbsPatchPrimHandle {
    Arc::new(DataSourceNurbsPatchPrim::new(
        scene_index_path,
        prim,
        stage_globals,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source_stage_globals::NoOpStageGlobals;
    use usd_core::Stage;

    fn create_test_globals() -> DataSourceStageGlobalsHandle {
        Arc::new(NoOpStageGlobals::default())
    }

    #[test]
    fn test_prim_names() {
        usd_core::schema_registry::register_builtin_schemas();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage
            .define_prim("/Patch", "NurbsPatch")
            .expect("define nurbs patch");
        let globals = create_test_globals();

        let ds = DataSourceNurbsPatchPrim::new(Path::absolute_root(), prim, globals);
        let names = ds.get_names();

        assert!(names.iter().any(|n| n == "nurbsPatch"));
    }

    #[test]
    fn test_orientation_mapping_invalidates_nurbs_patch_locator() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let properties = vec![usd_geom_tokens().orientation.clone()];

        let locators = DataSourceNurbsPatchPrim::invalidate(
            &prim,
            &Token::new(""),
            &properties,
            PropertyInvalidationType::PropertyChanged,
        );

        assert!(locators.contains(&HdNurbsPatchSchema::get_orientation_locator()));
    }

    #[test]
    fn test_point_weights_invalidates_primvars_overlay() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let properties = vec![usd_geom_tokens().point_weights.clone()];

        let locators = DataSourceNurbsPatchPrim::invalidate(
            &prim,
            &Token::new(""),
            &properties,
            PropertyInvalidationType::PropertyChanged,
        );

        assert!(locators.contains(&HdPrimvarsSchema::get_default_locator().append(
            &usd_geom_tokens().point_weights
        )));
    }
}
