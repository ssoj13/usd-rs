//! Mesh Light API schema.
//!
//! This is the preferred API schema to apply to Mesh type prims when adding
//! light behaviors to a mesh. At its base, this API schema has the built-in
//! behavior of applying LightAPI to the mesh and overriding the default
//! materialSyncMode to allow the emission/glow of the bound material to
//! affect the color of the light.
//!
//! Additionally serves as a hook for plugins to attach additional properties
//! to "mesh lights" through the creation of API schemas which are authored
//! to auto-apply to MeshLightAPI.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdLux/meshLightAPI.h` and `meshLightAPI.cpp`

use std::sync::Arc;

use usd_core::{Prim, SchemaKind, Stage};
use usd_sdf::Path;
use usd_tf::Token;

use super::tokens::tokens;

/// API schema for mesh lights.
///
/// MeshLightAPI is the modern replacement for GeometryLight. When applied to
/// a Mesh prim, it makes that mesh emit light based on its surface area.
///
/// # Auto-Applied Schemas
///
/// When MeshLightAPI is applied, it automatically applies LightAPI as well.
/// The materialSyncMode is set to allow bound material emission to affect
/// the light color.
///
/// # Schema Kind
///
/// This is a SingleApplyAPI schema.
#[derive(Clone)]
pub struct MeshLightAPI {
    prim: Prim,
}

impl MeshLightAPI {
    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "MeshLightAPI";

    /// The schema kind.
    pub const SCHEMA_KIND: SchemaKind = SchemaKind::SingleApplyAPI;

    // =========================================================================
    // Construction
    // =========================================================================

    /// Construct a MeshLightAPI on the given prim.
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Construct from another prim.
    pub fn from_prim(prim: &Prim) -> Self {
        Self::new(prim.clone())
    }

    /// Return a MeshLightAPI holding the prim at `path` on `stage`.
    ///
    /// Returns None if no prim exists at path or if the prim doesn't have
    /// this API schema applied.
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage.get_prim_at_path(path)?;
        if prim.has_api(&tokens().mesh_light_api) {
            Some(Self::new(prim))
        } else {
            None
        }
    }

    /// Returns true if this API schema can be applied to the given prim.
    ///
    /// MeshLightAPI should be applied to Mesh prims.
    pub fn can_apply(prim: &Prim, _why_not: Option<&mut String>) -> bool {
        if !prim.is_valid() {
            return false;
        }
        // Check if it's a Mesh prim
        let type_name = prim.type_name();
        type_name == "Mesh" || type_name.is_empty()
    }

    /// Applies this single-apply API schema to the given prim.
    ///
    /// This adds "MeshLightAPI" to the apiSchemas metadata on the prim.
    /// Applying MeshLightAPI also implicitly applies LightAPI.
    pub fn apply(prim: &Prim) -> Option<Self> {
        if !prim.is_valid() {
            return None;
        }

        if prim.apply_api(&tokens().mesh_light_api) {
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
    /// MeshLightAPI has no additional attributes beyond what LightAPI provides.
    /// LightAPI is auto-applied when MeshLightAPI is applied.
    pub fn get_schema_attribute_names(_include_inherited: bool) -> Vec<Token> {
        Vec::new()
    }
}

// ============================================================================
// Trait implementations
// ============================================================================

impl From<Prim> for MeshLightAPI {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

impl From<MeshLightAPI> for Prim {
    fn from(api: MeshLightAPI) -> Self {
        api.prim
    }
}

impl AsRef<Prim> for MeshLightAPI {
    fn as_ref(&self) -> &Prim {
        &self.prim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_type_name() {
        assert_eq!(MeshLightAPI::SCHEMA_TYPE_NAME, "MeshLightAPI");
    }

    #[test]
    fn test_schema_kind() {
        assert_eq!(MeshLightAPI::SCHEMA_KIND, SchemaKind::SingleApplyAPI);
    }
}
