//! HdDisplayFilterSchema - Display filter sprim schema.
//!
//! Corresponds to pxr/imaging/hd/displayFilterSchema.h.

use super::base::HdSchema;
use super::material_node::HdMaterialNodeSchema;
use crate::data_source::{HdContainerDataSourceHandle, HdDataSourceLocator};
use once_cell::sync::Lazy;
use usd_tf::Token;

static DISPLAY_FILTER: Lazy<Token> = Lazy::new(|| Token::new("displayFilter"));
static RESOURCE: Lazy<Token> = Lazy::new(|| Token::new("resource"));

/// Schema for display filter sprim.
#[derive(Debug, Clone)]
pub struct HdDisplayFilterSchema {
    schema: HdSchema,
}

impl HdDisplayFilterSchema {
    /// Construct from container.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Get from parent container at "displayFilter".
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        parent
            .get(&DISPLAY_FILTER)
            .as_ref()
            .and_then(crate::data_source::cast_to_container)
            .map(Self::new)
            .unwrap_or_else(|| Self {
                schema: HdSchema::empty(),
            })
    }

    /// Get resource (material node schema).
    pub fn get_resource(&self) -> HdMaterialNodeSchema {
        if let Some(container) = self.schema.get_container() {
            if let Some(cont) = container
                .get(&RESOURCE)
                .as_ref()
                .and_then(crate::data_source::cast_to_container)
            {
                return HdMaterialNodeSchema::new(cont);
            }
        }
        HdMaterialNodeSchema::new(crate::data_source::HdRetainedContainerDataSource::new_empty())
    }

    /// Get schema token.
    pub fn get_schema_token() -> &'static Token {
        &DISPLAY_FILTER
    }

    /// Get default locator.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_token(DISPLAY_FILTER.clone())
    }

    /// Get resource locator.
    pub fn get_resource_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[DISPLAY_FILTER.clone(), RESOURCE.clone()])
    }
}
