
//! HdRendererCreateArgsSchema - Arguments for HdRendererPlugin.
//!
//! Corresponds to pxr/imaging/hd/rendererCreateArgsSchema.h.

use super::base::HdSchema;
use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceLocator, HdTypedSampledDataSource, cast_to_container,
};
use once_cell::sync::Lazy;
use std::sync::Arc;
use usd_tf::Token;

/// Schema tokens.
pub static RENDERER_CREATE_ARGS: Lazy<Token> = Lazy::new(|| Token::new("rendererCreateArgs"));
pub static GPU_ENABLED: Lazy<Token> = Lazy::new(|| Token::new("gpuEnabled"));
pub static DRIVERS: Lazy<Token> = Lazy::new(|| Token::new("drivers"));
#[allow(dead_code)] // C++ schema token for drivers container key, not yet wired
pub static HGI: Lazy<Token> = Lazy::new(|| Token::new("hgi"));

/// Bool data source handle.
pub type HdBoolDataSourceHandle = Arc<dyn HdTypedSampledDataSource<bool>>;

/// Schema for renderer create args.
#[derive(Debug, Clone)]
pub struct HdRendererCreateArgsSchema {
    schema: HdSchema,
}

impl HdRendererCreateArgsSchema {
    /// Construct from container.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Get from parent container at "rendererCreateArgs".
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&RENDERER_CREATE_ARGS) {
            if let Some(cont) = cast_to_container(&child) {
                return Self::new(cont);
            }
        }
        Self {
            schema: HdSchema::empty(),
        }
    }

    /// Is GPU enabled.
    pub fn get_gpu_enabled(&self) -> Option<HdBoolDataSourceHandle> {
        self.schema.get_typed(&GPU_ENABLED)
    }

    /// Get drivers container.
    pub fn get_drivers(&self) -> Option<HdContainerDataSourceHandle> {
        self.schema.get_typed(&DRIVERS)
    }

    /// Get schema token.
    pub fn get_schema_token() -> &'static Token {
        &RENDERER_CREATE_ARGS
    }

    /// Get default locator.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_token(RENDERER_CREATE_ARGS.clone())
    }
}
