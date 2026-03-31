//! Built-in material schema.
//!
//! Port of pxr/imaging/hd/builtinMaterialSchema.h

use super::HdSchema;
use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdDataSourceLocator, cast_to_container,
};
use once_cell::sync::Lazy;

/// Schema token: "builtinMaterial"
pub static BUILTIN_MATERIAL: Lazy<usd_tf::Token> =
    Lazy::new(|| usd_tf::Token::new("builtinMaterial"));

/// Schema for built-in material flag on prims.
///
/// When true, the prim uses a built-in material and should not be pruned
/// by scene material pruning.
#[derive(Debug, Clone)]
pub struct HdBuiltinMaterialSchema {
    schema: HdSchema,
}

impl HdBuiltinMaterialSchema {
    /// Create from container.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Get from parent container.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&*BUILTIN_MATERIAL) {
            if let Some(container) = cast_to_container(&child) {
                return Self::new(container);
            }
        }
        Self {
            schema: HdSchema::empty(),
        }
    }

    /// Get builtin material bool. Returns true if set and value is true.
    pub fn get_builtin_material(&self) -> Option<bool> {
        let ds = self.schema.get_container()?;
        let child: HdDataSourceBaseHandle = ds.get(&*BUILTIN_MATERIAL)?.clone();
        let sampled = child.as_sampled()?;
        let value = sampled.get_value(0.0);
        value.get::<bool>().copied()
    }

    /// Default locator for builtinMaterial schema.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[BUILTIN_MATERIAL.clone()])
    }

    /// Locator for builtinMaterial member.
    pub fn get_builtin_material_locator() -> HdDataSourceLocator {
        Self::get_default_locator()
    }
}
