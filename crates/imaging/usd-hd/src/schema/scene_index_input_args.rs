//! HdSceneIndexInputArgsSchema - Scene index input args from HdRendererPlugin.
//!
//! Corresponds to pxr/imaging/hd/sceneIndexInputArgsSchema.h.

use super::base::HdSchema;
use crate::data_source::HdContainerDataSourceHandle;
use once_cell::sync::Lazy;
use std::sync::Arc;
use usd_tf::Token;

/// Schema tokens.
pub static MOTION_BLUR_SUPPORT: Lazy<Token> = Lazy::new(|| Token::new("motionBlurSupport"));
pub static CAMERA_MOTION_BLUR_SUPPORT: Lazy<Token> =
    Lazy::new(|| Token::new("cameraMotionBlurSupport"));
pub static LEGACY_RENDER_DELEGATE_INFO: Lazy<Token> =
    Lazy::new(|| Token::new("legacyRenderDelegateInfo"));

use crate::data_source::HdDataSourceBaseHandle;

/// Bool data source handle.
pub type HdBoolDataSourceHandle = Arc<dyn crate::data_source::HdTypedSampledDataSource<bool>>;

/// Render delegate info data source handle (opaque container).
pub type HdRenderDelegateInfoDataSourceHandle = HdDataSourceBaseHandle;

/// Schema for scene index input args.
#[derive(Debug, Clone)]
pub struct HdSceneIndexInputArgsSchema {
    schema: HdSchema,
}

impl HdSceneIndexInputArgsSchema {
    /// Construct from container.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Motion blur support.
    pub fn get_motion_blur_support(&self) -> Option<HdBoolDataSourceHandle> {
        self.schema.get_typed(&MOTION_BLUR_SUPPORT)
    }

    /// Camera motion blur support.
    pub fn get_camera_motion_blur_support(&self) -> Option<HdBoolDataSourceHandle> {
        self.schema.get_typed(&CAMERA_MOTION_BLUR_SUPPORT)
    }

    /// Legacy render delegate info (opaque container).
    pub fn get_legacy_render_delegate_info(&self) -> Option<HdRenderDelegateInfoDataSourceHandle> {
        let container = self.schema.get_container()?;
        container.get(&LEGACY_RENDER_DELEGATE_INFO)
    }
}
