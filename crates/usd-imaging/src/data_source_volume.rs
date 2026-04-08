//! DataSourceVolume - Volume data source for Hydra.
//!
//! Port of pxr/usdImaging/usdImaging/dataSourceVolume.h/.cpp

use crate::data_source_gprim::DataSourceGprim;
use crate::data_source_stage_globals::DataSourceStageGlobalsHandle;
use crate::types::PropertyInvalidationType;
use std::sync::Arc;
use usd_core::Prim;
use usd_hd::data_source::HdRetainedTypedSampledDataSource;
use usd_hd::schema::HdVolumeFieldBindingSchema;
use usd_hd::{HdContainerDataSource, HdDataSourceBase, HdDataSourceBaseHandle};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vol::Volume;

// ============================================================================
// DataSourceVolumeFieldBindings
// ============================================================================

#[derive(Clone)]
pub struct DataSourceVolumeFieldBindings {
    volume: Volume,
    #[allow(dead_code)]
    stage_globals: DataSourceStageGlobalsHandle,
}

impl DataSourceVolumeFieldBindings {
    pub fn new(volume: Volume, stage_globals: DataSourceStageGlobalsHandle) -> Arc<Self> {
        Arc::new(Self {
            volume,
            stage_globals,
        })
    }
}

impl std::fmt::Debug for DataSourceVolumeFieldBindings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DataSourceVolumeFieldBindings")
    }
}

impl HdDataSourceBase for DataSourceVolumeFieldBindings {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for DataSourceVolumeFieldBindings {
    fn get_names(&self) -> Vec<Token> {
        self.volume.get_field_paths().into_keys().collect()
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        let path = self.volume.get_field_path(name)?;
        Some(HdRetainedTypedSampledDataSource::<Path>::new(path) as HdDataSourceBaseHandle)
    }
}

pub type DataSourceVolumeFieldBindingsHandle = Arc<DataSourceVolumeFieldBindings>;

// ============================================================================
// DataSourceVolumePrim
// ============================================================================

#[derive(Clone)]
pub struct DataSourceVolumePrim {
    base: Arc<DataSourceGprim>,
    prim: Prim,
    stage_globals: DataSourceStageGlobalsHandle,
}

impl DataSourceVolumePrim {
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
            stage_globals,
        }
    }

    pub fn get_names(&self) -> Vec<Token> {
        let mut names = self.base.get_names();
        names.push((**HdVolumeFieldBindingSchema::get_schema_token()).clone());
        names
    }

    pub fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if name == &**HdVolumeFieldBindingSchema::get_schema_token() {
            return Some(DataSourceVolumeFieldBindings::new(
                Volume::from_prim(&self.prim),
                self.stage_globals.clone(),
            ) as HdDataSourceBaseHandle);
        }
        self.base.get(name)
    }

    pub fn invalidate(
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> usd_hd::HdDataSourceLocatorSet {
        let mut locators =
            DataSourceGprim::invalidate(prim, subprim, properties, invalidation_type);

        for property_name in properties {
            if property_name.as_str().starts_with("field:") {
                locators.insert(HdVolumeFieldBindingSchema::get_default_locator());
                break;
            }
        }

        locators
    }
}

impl std::fmt::Debug for DataSourceVolumePrim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DataSourceVolumePrim")
    }
}

pub type DataSourceVolumePrimHandle = Arc<DataSourceVolumePrim>;

pub fn create_data_source_volume_prim(
    scene_index_path: Path,
    prim: Prim,
    stage_globals: DataSourceStageGlobalsHandle,
) -> DataSourceVolumePrimHandle {
    Arc::new(DataSourceVolumePrim::new(
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
        let prim = stage.define_prim("/Vol", "Volume").expect("define volume");
        let globals = create_test_globals();

        let ds = DataSourceVolumePrim::new(Path::absolute_root(), prim, globals);

        let names = ds.get_names();
        assert!(names.iter().any(|n| n == "volumeFieldBinding"));
    }

    #[test]
    fn test_field_bindings_names_and_values() {
        usd_core::schema_registry::register_builtin_schemas();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let volume_prim = stage.define_prim("/Vol", "Volume").expect("define volume");
        stage
            .define_prim("/Field", "OpenVDBAsset")
            .expect("define field");

        let volume = Volume::from_prim(&volume_prim);
        assert!(volume.create_field_relationship(
            &Token::new("density"),
            &Path::from_string("/Field").unwrap()
        ));

        let globals = create_test_globals();
        let ds = DataSourceVolumeFieldBindings::new(volume, globals);

        let names = ds.get_names();
        assert_eq!(names, vec![Token::new("density")]);
        assert!(ds.get(&Token::new("density")).is_some());
    }

    #[test]
    fn test_invalidate_uses_volume_field_binding_locator() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let properties = vec![Token::new("field:density")];

        let locators = DataSourceVolumePrim::invalidate(
            &prim,
            &Token::new(""),
            &properties,
            PropertyInvalidationType::PropertyChanged,
        );

        assert!(locators.contains(&HdVolumeFieldBindingSchema::get_default_locator()));
    }
}
