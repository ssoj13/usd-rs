
//! HdSampleFilterSchema - Sample filter sprim schema.
//!
//! Corresponds to pxr/imaging/hd/sampleFilterSchema.h.

use super::base::HdSchema;
use super::material_node::HdMaterialNodeSchema;
use crate::data_source::{HdContainerDataSourceHandle, HdDataSourceLocator, cast_to_container};
use once_cell::sync::Lazy;
use usd_tf::Token;

static SAMPLE_FILTER: Lazy<Token> = Lazy::new(|| Token::new("sampleFilter"));
static RESOURCE: Lazy<Token> = Lazy::new(|| Token::new("resource"));

/// Schema for sample filter sprim.
#[derive(Debug, Clone)]
pub struct HdSampleFilterSchema {
    schema: HdSchema,
}

impl HdSampleFilterSchema {
    /// Construct from container.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Get from parent container at "sampleFilter".
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&SAMPLE_FILTER) {
            if let Some(cont) = cast_to_container(&child) {
                return Self::new(cont);
            }
        }
        Self {
            schema: HdSchema::empty(),
        }
    }

    /// Get resource (material node schema).
    pub fn get_resource(&self) -> HdMaterialNodeSchema {
        if let Some(container) = self.schema.get_container() {
            if let Some(child) = container.get(&RESOURCE) {
                if let Some(cont) = cast_to_container(&child) {
                    return HdMaterialNodeSchema::new(cont);
                }
            }
        }
        HdMaterialNodeSchema::new(crate::data_source::HdRetainedContainerDataSource::new_empty())
    }

    /// Get schema token.
    pub fn get_schema_token() -> &'static Token {
        &SAMPLE_FILTER
    }

    /// Get default locator.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_token(SAMPLE_FILTER.clone())
    }

    /// Get resource locator.
    pub fn get_resource_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[SAMPLE_FILTER.clone(), RESOURCE.clone()])
    }
}
