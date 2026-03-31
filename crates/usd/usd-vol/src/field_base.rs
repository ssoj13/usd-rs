//! Field Base schema.
//!
//! Abstract base class for all field primitives. Inherits from UsdGeomXformable
//! to provide transform capabilities.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdVol/fieldBase.h`

use std::sync::Arc;

use usd_core::{Prim, Stage};
use usd_sdf::Path;
use usd_tf::Token;

use super::tokens::USD_VOL_TOKENS;

/// Abstract base class for field primitives.
///
/// FieldBase provides the foundation for all volumetric field types.
/// It inherits from UsdGeomXformable, meaning fields can be positioned
/// and oriented in 3D space.
///
/// # Schema Kind
///
/// This is an abstract typed schema (AbstractTyped).
///
/// # Inheritance
///
/// Inherits from UsdGeomXformable (not directly modeled here).
#[derive(Debug, Clone)]
pub struct FieldBase {
    prim: Prim,
}

impl FieldBase {
    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "FieldBase";

    /// Construct a FieldBase on the given prim.
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Construct from another prim.
    pub fn from_prim(prim: &Prim) -> Self {
        Self::new(prim.clone())
    }

    /// Return a FieldBase holding the prim at `path` on `stage`.
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage.get_prim_at_path(path)?;
        if prim.is_a(&USD_VOL_TOKENS.field_base) {
            Some(Self::new(prim))
        } else {
            None
        }
    }

    /// Returns true if this schema is valid.
    pub fn is_valid(&self) -> bool {
        self.prim.is_valid()
    }

    /// Get the wrapped prim.
    pub fn get_prim(&self) -> &Prim {
        &self.prim
    }

    /// Returns all pre-declared attributes for this schema class.
    ///
    /// FieldBase has no custom attributes beyond those from Xformable.
    pub fn get_schema_attribute_names(_include_inherited: bool) -> Vec<Token> {
        Vec::new()
    }
}

impl From<Prim> for FieldBase {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

impl From<FieldBase> for Prim {
    fn from(field: FieldBase) -> Self {
        field.prim
    }
}

impl AsRef<Prim> for FieldBase {
    fn as_ref(&self) -> &Prim {
        &self.prim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_type_name() {
        assert_eq!(FieldBase::SCHEMA_TYPE_NAME, "FieldBase");
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = FieldBase::get_schema_attribute_names(false);
        assert!(names.is_empty());
    }
}
