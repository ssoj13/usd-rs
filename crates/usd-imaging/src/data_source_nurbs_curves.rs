//! DataSourceNurbsCurves - NurbsCurves data source for Hydra.
//!
//! Port of pxr/usdImaging/usdImaging/dataSourceNurbsCurves.h/.cpp

use crate::data_source_gprim::DataSourceGprim;
use crate::data_source_mapped::{
    AttributeMapping, DataSourceMapped, PropertyMapping, PropertyMappings,
};
use crate::data_source_primvars::{DataSourceCustomPrimvars, PrimvarMapping};
use crate::data_source_stage_globals::DataSourceStageGlobalsHandle;
use crate::types::PropertyInvalidationType;
use std::sync::LazyLock;
use std::sync::Arc;
use usd_core::Prim;
use usd_geom::curves::Curves;
use usd_geom::nurbs_curves::NurbsCurves;
use usd_geom::tokens::usd_geom_tokens;
use usd_hd::data_source::cast_to_container;
use usd_hd::schema::{HdNurbsCurvesSchema, HdPrimvarsSchema};
use usd_hd::{
    HdContainerDataSource, HdDataSourceBaseHandle, HdDataSourceLocator, HdDataSourceLocatorSet,
    HdOverlayContainerDataSource,
};
use usd_sdf::Path;
use usd_tf::Token;

fn get_property_mappings() -> Vec<PropertyMapping> {
    let mut result = Vec::new();

    for usd_name in NurbsCurves::get_schema_attribute_names(false) {
        if usd_name == usd_geom_tokens().point_weights {
            continue;
        }
        result.push(PropertyMapping::Attribute(AttributeMapping::new(
            usd_name.clone(),
            HdDataSourceLocator::from_token(usd_name),
        )));
    }

    for usd_name in Curves::get_schema_attribute_names(false) {
        if usd_name == usd_geom_tokens().widths {
            continue;
        }
        result.push(PropertyMapping::Attribute(AttributeMapping::new(
            usd_name.clone(),
            HdDataSourceLocator::from_token(usd_name),
        )));
    }

    result
}

fn get_mappings() -> &'static PropertyMappings {
    static MAPPINGS: LazyLock<PropertyMappings> = LazyLock::new(|| {
        PropertyMappings::new(
            get_property_mappings(),
            HdNurbsCurvesSchema::get_default_locator(),
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

pub type DataSourceNurbsCurves = DataSourceMapped;

#[derive(Clone)]
pub struct DataSourceNurbsCurvesPrim {
    base: Arc<DataSourceGprim>,
    prim: Prim,
    scene_index_path: Path,
    stage_globals: DataSourceStageGlobalsHandle,
}

impl HdContainerDataSource for DataSourceNurbsCurvesPrim {
    fn get_names(&self) -> Vec<Token> {
        Self::get_names(self)
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        Self::get(self, name)
    }
}

impl usd_hd::HdDataSourceBase for DataSourceNurbsCurvesPrim {
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

impl DataSourceNurbsCurvesPrim {
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
        names.push((**HdNurbsCurvesSchema::get_schema_token()).clone());
        names
    }

    pub fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if name == &**HdNurbsCurvesSchema::get_schema_token() {
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

impl std::fmt::Debug for DataSourceNurbsCurvesPrim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DataSourceNurbsCurvesPrim")
    }
}

pub type DataSourceNurbsCurvesPrimHandle = Arc<DataSourceNurbsCurvesPrim>;

pub fn create_data_source_nurbs_curves_prim(
    scene_index_path: Path,
    prim: Prim,
    stage_globals: DataSourceStageGlobalsHandle,
) -> DataSourceNurbsCurvesPrimHandle {
    Arc::new(DataSourceNurbsCurvesPrim::new(
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
    use usd_hd::schema::HdNurbsCurvesSchema;

    fn create_test_globals() -> DataSourceStageGlobalsHandle {
        Arc::new(NoOpStageGlobals::default())
    }

    #[test]
    fn test_prim_names() {
        usd_core::schema_registry::register_builtin_schemas();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage
            .define_prim("/Curve", "NurbsCurves")
            .expect("define nurbs curves");
        let globals = create_test_globals();

        let ds = DataSourceNurbsCurvesPrim::new(Path::absolute_root(), prim, globals);
        let names = ds.get_names();

        assert!(names.iter().any(|n| n == "nurbsCurves"));
    }

    #[test]
    fn test_get_nurbs_curves_schema_container() {
        usd_core::schema_registry::register_builtin_schemas();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage
            .define_prim("/Curve", "NurbsCurves")
            .expect("define nurbs curves");
        let globals = create_test_globals();

        let ds = DataSourceNurbsCurvesPrim::new(Path::absolute_root(), prim, globals);
        let result = ds.get(&(**HdNurbsCurvesSchema::get_schema_token()).clone());
        assert!(result.is_some());
        assert!(result.and_then(|d| cast_to_container(&d)).is_some());
    }

    #[test]
    fn test_point_weights_invalidate_as_custom_primvar() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let properties = vec![usd_geom_tokens().point_weights.clone()];

        let locators = DataSourceNurbsCurvesPrim::invalidate(
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
