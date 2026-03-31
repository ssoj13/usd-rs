//! Volume Light API schema.
//!
//! This is the preferred API schema to apply to Volume type prims when adding
//! light behaviors to a volume. At its base, this API schema has the built-in
//! behavior of applying LightAPI to the volume and overriding the default
//! materialSyncMode to allow the emission/glow of the bound material to
//! affect the color of the light.
//!
//! Additionally serves as a hook for plugins to attach additional properties
//! to "volume lights" through the creation of API schemas which are authored
//! to auto-apply to VolumeLightAPI.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdLux/volumeLightAPI.h` and `volumeLightAPI.cpp`

use std::sync::Arc;

use usd_core::{Prim, SchemaKind, Stage};
use usd_sdf::Path;
use usd_tf::Token;

use super::tokens::tokens;

/// API schema for volume lights.
///
/// VolumeLightAPI is applied to Volume prims to make them emit light.
/// When applied, it automatically applies LightAPI as well.
///
/// # Schema Kind
///
/// This is a SingleApplyAPI schema.
#[derive(Clone)]
pub struct VolumeLightAPI {
    prim: Prim,
}

impl VolumeLightAPI {
    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "VolumeLightAPI";

    /// The schema kind.
    pub const SCHEMA_KIND: SchemaKind = SchemaKind::SingleApplyAPI;

    // =========================================================================
    // Construction
    // =========================================================================

    /// Construct a VolumeLightAPI on the given prim.
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Construct from another prim.
    pub fn from_prim(prim: &Prim) -> Self {
        Self::new(prim.clone())
    }

    /// Return a VolumeLightAPI holding the prim at `path` on `stage`.
    ///
    /// Returns None if no prim exists at path or if the prim doesn't have
    /// this API schema applied.
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage.get_prim_at_path(path)?;
        if prim.has_api(&tokens().volume_light_api) {
            Some(Self::new(prim))
        } else {
            None
        }
    }

    /// Returns true if this API schema can be applied to the given prim.
    ///
    /// VolumeLightAPI should be applied to Volume prims.
    pub fn can_apply(prim: &Prim, _why_not: Option<&mut String>) -> bool {
        if !prim.is_valid() {
            return false;
        }
        // Check if it's a Volume prim
        let type_name = prim.type_name();
        type_name == "Volume" || type_name.is_empty()
    }

    /// Applies this single-apply API schema to the given prim.
    ///
    /// This adds "VolumeLightAPI" to the apiSchemas metadata on the prim.
    /// Applying VolumeLightAPI also implicitly applies LightAPI.
    pub fn apply(prim: &Prim) -> Option<Self> {
        if !prim.is_valid() {
            return None;
        }

        if prim.apply_api(&tokens().volume_light_api) {
            Some(Self::new(prim.clone()))
        } else {
            None
        }
    }

    /// Returns the schema kind.
    pub fn get_schema_kind(&self) -> SchemaKind {
        Self::SCHEMA_KIND
    }

    /// Returns true if this schema is valid.
    pub fn is_valid(&self) -> bool {
        self.prim.is_valid()
    }

    /// Get the wrapped prim.
    pub fn get_prim(&self) -> &Prim {
        &self.prim
    }

    // =========================================================================
    // Schema attribute names
    // =========================================================================

    /// Returns all pre-declared attributes for this schema class.
    ///
    /// VolumeLightAPI has no additional attributes beyond what LightAPI provides.
    /// LightAPI is auto-applied when VolumeLightAPI is applied.
    pub fn get_schema_attribute_names(_include_inherited: bool) -> Vec<Token> {
        Vec::new()
    }
}

// ============================================================================
// Trait implementations
// ============================================================================

impl From<Prim> for VolumeLightAPI {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

impl From<VolumeLightAPI> for Prim {
    fn from(api: VolumeLightAPI) -> Self {
        api.prim
    }
}

impl AsRef<Prim> for VolumeLightAPI {
    fn as_ref(&self) -> &Prim {
        &self.prim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_type_name() {
        assert_eq!(VolumeLightAPI::SCHEMA_TYPE_NAME, "VolumeLightAPI");
    }

    #[test]
    fn test_schema_kind() {
        assert_eq!(VolumeLightAPI::SCHEMA_KIND, SchemaKind::SingleApplyAPI);
    }
}
