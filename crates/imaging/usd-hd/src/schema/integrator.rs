//! HdIntegratorSchema - Integrator sprim schema.
//!
//! Corresponds to pxr/imaging/hd/integratorSchema.h.

use super::base::HdSchema;
use super::material_node::HdMaterialNodeSchema;
use crate::data_source::{HdContainerDataSourceHandle, HdDataSourceLocator, cast_to_container};
use once_cell::sync::Lazy;
use usd_tf::Token;

static INTEGRATOR: Lazy<Token> = Lazy::new(|| Token::new("integrator"));
static RESOURCE: Lazy<Token> = Lazy::new(|| Token::new("resource"));

/// Schema for integrator sprim.
#[derive(Debug, Clone)]
pub struct HdIntegratorSchema {
    schema: HdSchema,
}

impl HdIntegratorSchema {
    /// Construct from container.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Get from parent container at "integrator".
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&INTEGRATOR) {
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
        &INTEGRATOR
    }

    /// Get default locator.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_token(INTEGRATOR.clone())
    }

    /// Get resource locator.
    pub fn get_resource_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[INTEGRATOR.clone(), RESOURCE.clone()])
    }
}
