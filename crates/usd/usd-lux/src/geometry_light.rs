//! Geometry Light schema.
//!
//! **DEPRECATED**: Light emitted outward from a geometric prim (UsdGeomGprim),
//! which is typically a mesh.
//!
//! Use MeshLightAPI instead for new projects.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdLux/geometryLight.h` and `geometryLight.cpp`

use std::sync::Arc;

use usd_core::{Prim, Relationship, SchemaKind, Stage};
use usd_sdf::Path;
use usd_tf::Token;

use super::nonboundable_light_base::NonboundableLightBase;
use super::tokens::tokens;

/// Light emitted outward from a geometric prim.
///
/// **DEPRECATED**: Use MeshLightAPI instead.
///
/// GeometryLight uses a relationship to reference geometry that will emit
/// light. The geometry is typically a mesh.
///
/// # Schema Kind
///
/// This is a ConcreteTyped schema.
#[derive(Clone)]
#[deprecated(note = "Use MeshLightAPI instead")]
pub struct GeometryLight {
    base: NonboundableLightBase,
}

#[allow(deprecated)]
impl GeometryLight {
    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "GeometryLight";

    /// The schema kind.
    pub const SCHEMA_KIND: SchemaKind = SchemaKind::ConcreteTyped;

    // =========================================================================
    // Construction
    // =========================================================================

    /// Construct a GeometryLight on the given prim.
    pub fn new(prim: Prim) -> Self {
        Self {
            base: NonboundableLightBase::new(prim),
        }
    }

    /// Construct from NonboundableLightBase.
    pub fn from_base(base: NonboundableLightBase) -> Self {
        Self { base }
    }

    /// Create an invalid GeometryLight.
    pub fn invalid() -> Self {
        Self {
            base: NonboundableLightBase::invalid(),
        }
    }

    /// Returns true if this schema is valid.
    pub fn is_valid(&self) -> bool {
        self.base.is_valid()
    }

    /// Get the wrapped prim.
    pub fn prim(&self) -> &Prim {
        self.base.get_prim()
    }

    /// Get as NonboundableLightBase.
    pub fn base(&self) -> &NonboundableLightBase {
        &self.base
    }

    /// Get the schema kind.
    pub fn get_schema_kind(&self) -> SchemaKind {
        Self::SCHEMA_KIND
    }

    /// Return a GeometryLight holding the prim at `path` on `stage`.
    ///
    /// Matches C++ `UsdLuxGeometryLight::Get()` — no type check performed.
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage.get_prim_at_path(path)?;
        Some(Self::new(prim))
    }

    /// Define a GeometryLight at `path` on `stage`.
    pub fn define(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage
            .define_prim(path.as_str(), Self::SCHEMA_TYPE_NAME)
            .ok()?;
        Some(Self::new(prim))
    }

    // =========================================================================
    // Geometry Relationship
    // =========================================================================

    /// Get the geometry relationship.
    ///
    /// Relationship to the geometry to use as the light source.
    pub fn get_geometry_rel(&self) -> Option<Relationship> {
        self.prim().get_relationship(tokens().geometry.as_str())
    }

    /// Create the geometry relationship.
    pub fn create_geometry_rel(&self) -> Option<Relationship> {
        self.prim()
            .create_relationship(tokens().geometry.as_str(), false)
    }

    // =========================================================================
    // Schema attribute names
    // =========================================================================

    /// Returns all pre-declared attributes for this schema class.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        if include_inherited {
            NonboundableLightBase::get_schema_attribute_names(true)
        } else {
            Vec::new()
        }
    }
}

// ============================================================================
// Trait implementations
// ============================================================================

#[allow(deprecated)]
impl From<Prim> for GeometryLight {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

#[allow(deprecated)]
impl From<NonboundableLightBase> for GeometryLight {
    fn from(base: NonboundableLightBase) -> Self {
        Self::from_base(base)
    }
}

#[allow(deprecated)]
impl From<GeometryLight> for Prim {
    fn from(light: GeometryLight) -> Self {
        light.base.get_prim().clone()
    }
}

#[allow(deprecated)]
impl AsRef<Prim> for GeometryLight {
    fn as_ref(&self) -> &Prim {
        self.prim()
    }
}

#[allow(deprecated)]
impl AsRef<NonboundableLightBase> for GeometryLight {
    fn as_ref(&self) -> &NonboundableLightBase {
        &self.base
    }
}

#[cfg(test)]
mod tests {
    #[allow(deprecated)]
    use super::*;

    #[test]
    #[allow(deprecated)]
    fn test_schema_type_name() {
        assert_eq!(GeometryLight::SCHEMA_TYPE_NAME, "GeometryLight");
    }

    #[test]
    #[allow(deprecated)]
    fn test_schema_kind() {
        assert_eq!(GeometryLight::SCHEMA_KIND, SchemaKind::ConcreteTyped);
    }
}
