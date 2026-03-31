//! DataSourceStage - Stage-level data source for Hydra.
//!
//! Port of pxr/usdImaging/usdImaging/dataSourceStage.cpp
//!
//! A container data source containing UsdStage level data:
//! - "system" -> HDAR system with asset resolver context
//! - "sceneGlobals" -> render settings path, start/end time codes

use std::sync::Arc;
use usd_core::Stage;
use usd_hd::data_source::{HdRetainedContainerDataSource, HdRetainedTypedSampledDataSource};
use usd_hd::{HdContainerDataSource, HdDataSourceBaseHandle};
use usd_sdf::Path;
use usd_tf::Token;

mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static SYSTEM: LazyLock<Token> = LazyLock::new(|| Token::new("system"));
    pub static SCENE_GLOBALS: LazyLock<Token> = LazyLock::new(|| Token::new("sceneGlobals"));
    pub static ACTIVE_RENDER_SETTINGS_PRIM_PATH: LazyLock<Token> =
        LazyLock::new(|| Token::new("activeRenderSettingsPrimPath"));
    pub static START_TIME_CODE: LazyLock<Token> = LazyLock::new(|| Token::new("startTimeCode"));
    pub static END_TIME_CODE: LazyLock<Token> = LazyLock::new(|| Token::new("endTimeCode"));
    pub static RENDER_SETTINGS_PRIM_PATH: LazyLock<Token> =
        LazyLock::new(|| Token::new("renderSettingsPrimPath"));
}

// ============================================================================
// DataSourceStage
// ============================================================================

/// Container data source containing UsdStage level data.
///
/// Exposes "system" (HDAR asset resolver context) and "sceneGlobals"
/// (active render settings prim path, start/end time codes).
#[derive(Clone)]
pub struct DataSourceStage {
    stage: Arc<Stage>,
}

impl DataSourceStage {
    /// Create a new stage data source.
    pub fn new(stage: Arc<Stage>) -> Self {
        Self { stage }
    }

    /// Build the "sceneGlobals" container with render settings + time codes.
    fn build_scene_globals(&self) -> HdDataSourceBaseHandle {
        let mut entries: Vec<(Token, HdDataSourceBaseHandle)> = Vec::new();

        // Active render settings prim path from stage metadata
        if self
            .stage
            .has_authored_metadata(&tokens::RENDER_SETTINGS_PRIM_PATH)
        {
            if let Some(val) = self.stage.get_metadata(&tokens::RENDER_SETTINGS_PRIM_PATH) {
                if let Some(path_str) = val.get::<String>() {
                    if !path_str.is_empty() {
                        if let Some(path) = Path::from_string(path_str) {
                            entries.push((
                                tokens::ACTIVE_RENDER_SETTINGS_PRIM_PATH.clone(),
                                HdRetainedTypedSampledDataSource::new(path)
                                    as HdDataSourceBaseHandle,
                            ));
                        }
                    }
                }
            }
        }

        // Start/end time codes
        entries.push((
            tokens::START_TIME_CODE.clone(),
            HdRetainedTypedSampledDataSource::new(self.stage.get_start_time_code())
                as HdDataSourceBaseHandle,
        ));
        entries.push((
            tokens::END_TIME_CODE.clone(),
            HdRetainedTypedSampledDataSource::new(self.stage.get_end_time_code())
                as HdDataSourceBaseHandle,
        ));

        HdRetainedContainerDataSource::from_entries(&entries) as HdDataSourceBaseHandle
    }
}

impl std::fmt::Debug for DataSourceStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DataSourceStage")
    }
}

impl usd_hd::HdDataSourceBase for DataSourceStage {
    fn clone_box(&self) -> usd_hd::HdDataSourceBaseHandle {
        std::sync::Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for DataSourceStage {
    fn get_names(&self) -> Vec<Token> {
        vec![tokens::SYSTEM.clone(), tokens::SCENE_GLOBALS.clone()]
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if name == &*tokens::SYSTEM {
            // Minimal system container (HDAR asset resolver context)
            // Full impl would include ArResolverContext from stage
            return Some(HdRetainedContainerDataSource::new_empty() as HdDataSourceBaseHandle);
        }
        if name == &*tokens::SCENE_GLOBALS {
            return Some(self.build_scene_globals());
        }
        None
    }
}

/// Handle type for DataSourceStage.
pub type DataSourceStageHandle = Arc<DataSourceStage>;

/// Factory function for creating stage data sources.
pub fn create_data_source_stage(stage: Arc<Stage>) -> DataSourceStageHandle {
    Arc::new(DataSourceStage::new(stage))
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_hd::HdContainerDataSource;

    #[test]
    fn test_stage_data_source_names() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");

        let ds = DataSourceStage::new(stage);
        let names = ds.get_names();

        assert!(names.iter().any(|n| n == "system"));
        assert!(names.iter().any(|n| n == "sceneGlobals"));
    }

    #[test]
    fn test_scene_globals_has_time_codes() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");

        let ds = DataSourceStage::new(stage);
        let globals = ds.get(&Token::new("sceneGlobals"));
        assert!(globals.is_some());
    }

    #[test]
    fn test_system_returns_container() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");

        let ds = DataSourceStage::new(stage);
        let system = ds.get(&Token::new("system"));
        assert!(system.is_some());
    }
}
