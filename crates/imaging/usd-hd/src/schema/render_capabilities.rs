//! HdRenderCapabilitiesSchema - Render capabilities schema.
//!
//! Corresponds to pxr/imaging/hd/renderCapabilitiesSchema.h.

use super::base::HdSchema;
use crate::data_source::{HdContainerDataSourceHandle, HdDataSourceLocator, cast_to_container};
use once_cell::sync::Lazy;
use std::sync::Arc;
use usd_tf::Token;

static RENDER_CAPABILITIES: Lazy<Token> = Lazy::new(|| Token::new("renderCapabilities"));
static MOTION_BLUR: Lazy<Token> = Lazy::new(|| Token::new("motionBlur"));

/// Bool data source handle.
pub type HdBoolDataSourceHandle = Arc<dyn crate::data_source::HdTypedSampledDataSource<bool>>;

/// Schema for render capabilities.
#[derive(Debug, Clone)]
pub struct HdRenderCapabilitiesSchema {
    schema: HdSchema,
}

impl HdRenderCapabilitiesSchema {
    /// Construct from container.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Get schema from parent container and key.
    pub fn get_from_parent(
        parent: &HdContainerDataSourceHandle,
        key: &Token,
    ) -> Option<HdRenderCapabilitiesSchema> {
        parent
            .get(key)
            .as_ref()
            .and_then(cast_to_container)
            .map(Self::new)
    }

    /// Get motion blur support.
    pub fn get_motion_blur(&self) -> Option<HdBoolDataSourceHandle> {
        self.schema.get_typed(&MOTION_BLUR)
    }

    /// Get schema token.
    pub fn get_schema_token() -> &'static Token {
        &RENDER_CAPABILITIES
    }

    /// Get default locator.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_token(RENDER_CAPABILITIES.clone())
    }

    /// Get motion blur locator.
    pub fn get_motion_blur_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[RENDER_CAPABILITIES.clone(), MOTION_BLUR.clone()])
    }
}
