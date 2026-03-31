
//! Scene globals schema for Hydra.
//!
//! Port of pxr/imaging/hd/sceneGlobalsSchema.
//!
//! Global state at the root prim: active render pass/settings, camera, frame, etc.

use super::HdSchema;
use crate::data_source::{HdContainerDataSourceHandle, HdDataSourceLocator, cast_to_container};
use crate::scene_index::HdSceneIndexBase;
use once_cell::sync::Lazy;
use usd_sdf::Path;
use usd_tf::Token;

use crate::schema::ext_computation_input_computation::HdPathDataSourceHandle;

static SCENE_GLOBALS: Lazy<Token> = Lazy::new(|| Token::new("sceneGlobals"));
static ACTIVE_RENDER_SETTINGS_PRIM: Lazy<Token> =
    Lazy::new(|| Token::new("activeRenderSettingsPrim"));
static ACTIVE_RENDER_PASS_PRIM: Lazy<Token> = Lazy::new(|| Token::new("activeRenderPassPrim"));
static PRIMARY_CAMERA_PRIM: Lazy<Token> = Lazy::new(|| Token::new("primaryCameraPrim"));
static CURRENT_FRAME: Lazy<Token> = Lazy::new(|| Token::new("currentFrame"));
static TIME_CODES_PER_SECOND: Lazy<Token> = Lazy::new(|| Token::new("timeCodesPerSecond"));

/// Schema for scene globals at the root prim.
#[derive(Debug, Clone)]
pub struct HdSceneGlobalsSchema {
    schema: HdSchema,
}

impl HdSceneGlobalsSchema {
    /// Create schema from container.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Get schema from parent container (prim data source, typically root).
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&SCENE_GLOBALS) {
            if let Some(container) = cast_to_container(&child) {
                return Self::new(container);
            }
        }
        Self {
            schema: HdSchema::empty(),
        }
    }

    /// Get schema from scene index (reads root prim's data source).
    pub fn get_from_scene_index(si: &dyn HdSceneIndexBase) -> Self {
        let root = Path::absolute_root();
        let prim = si.get_prim(&root);
        if let Some(ref ds) = prim.data_source {
            return Self::get_from_parent(ds);
        }
        Self {
            schema: HdSchema::empty(),
        }
    }

    /// Default prim path for scene globals (root).
    pub fn get_default_prim_path() -> Path {
        Path::absolute_root()
    }

    /// Get schema token.
    pub fn get_schema_token() -> &'static Lazy<Token> {
        &SCENE_GLOBALS
    }

    /// Get default locator for sceneGlobals.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_token(SCENE_GLOBALS.clone())
    }

    /// Get active render settings prim path.
    pub fn get_active_render_settings_prim(&self) -> Option<HdPathDataSourceHandle> {
        self.schema.get_typed(&ACTIVE_RENDER_SETTINGS_PRIM)
    }

    /// Get active render pass prim path.
    pub fn get_active_render_pass_prim(&self) -> Option<HdPathDataSourceHandle> {
        self.schema.get_typed(&ACTIVE_RENDER_PASS_PRIM)
    }

    /// Get primary camera prim path.
    pub fn get_primary_camera_prim(&self) -> Option<HdPathDataSourceHandle> {
        self.schema.get_typed(&PRIMARY_CAMERA_PRIM)
    }

    /// Get active render settings prim locator.
    pub fn get_active_render_settings_prim_locator() -> HdDataSourceLocator {
        Self::get_default_locator().append(&ACTIVE_RENDER_SETTINGS_PRIM)
    }

    /// Get current frame data source (f64).
    pub fn get_current_frame(
        &self,
    ) -> Option<std::sync::Arc<dyn crate::data_source::HdTypedSampledDataSource<f64> + Send + Sync>>
    {
        self.schema.get_typed(&CURRENT_FRAME)
    }

    /// Get current frame locator.
    pub fn get_current_frame_locator() -> HdDataSourceLocator {
        Self::get_default_locator().append(&CURRENT_FRAME)
    }

    /// Get time codes per second data source (f64).
    pub fn get_time_codes_per_second(
        &self,
    ) -> Option<std::sync::Arc<dyn crate::data_source::HdTypedSampledDataSource<f64> + Send + Sync>>
    {
        self.schema.get_typed(&TIME_CODES_PER_SECOND)
    }

    /// Get time codes per second locator.
    pub fn get_time_codes_per_second_locator() -> HdDataSourceLocator {
        Self::get_default_locator().append(&TIME_CODES_PER_SECOND)
    }
}
