//! Material bindings schema.
//!
//! Port of pxr/imaging/hd/materialBindingsSchema.h

use super::HdSchema;
use super::material_binding::HdMaterialBindingSchema;
use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceLocator, HdDataSourceLocatorSet, cast_to_container,
};
use once_cell::sync::Lazy;

/// Schema token: "materialBindings"
pub static MATERIAL_BINDINGS: Lazy<usd_tf::Token> =
    Lazy::new(|| usd_tf::Token::new("materialBindings"));

/// Schema for material bindings on prims.
#[derive(Debug, Clone)]
pub struct HdMaterialBindingsSchema {
    schema: HdSchema,
}

impl HdMaterialBindingsSchema {
    /// Create from container.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Get from parent container.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&*MATERIAL_BINDINGS) {
            if let Some(container) = cast_to_container(&child) {
                return Self::new(container);
            }
        }
        Self {
            schema: HdSchema::empty(),
        }
    }

    /// Get material binding for a purpose. Falls back to allPurpose if not found.
    pub fn get_material_binding(&self, purpose: &usd_tf::Token) -> HdMaterialBindingSchema {
        if let Some(container) = self.schema.get_container() {
            if let Some(child) = container.get(purpose) {
                if let Some(binding_container) = cast_to_container(&child) {
                    return HdMaterialBindingSchema::new(binding_container);
                }
            }
            if let Some(child) = container.get(&*super::material_binding::ALL_PURPOSE) {
                if let Some(binding_container) = cast_to_container(&child) {
                    return HdMaterialBindingSchema::new(binding_container);
                }
            }
        }
        HdMaterialBindingSchema::empty()
    }

    /// Returns true if schema has material bindings.
    pub fn is_defined(&self) -> bool {
        self.schema.is_defined()
    }

    /// Default locator for materialBindings schema (single locator).
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[MATERIAL_BINDINGS.clone()])
    }

    /// Append a purpose token to the material bindings locator.
    pub fn get_locator_for_purpose(&self, purpose: &usd_tf::Token) -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[MATERIAL_BINDINGS.clone(), purpose.clone()])
    }

    /// Default locator set for materialBindings (for dirtied entries).
    pub fn get_default_locator_set() -> HdDataSourceLocatorSet {
        HdDataSourceLocatorSet::from_locator(Self::get_default_locator())
    }
}
