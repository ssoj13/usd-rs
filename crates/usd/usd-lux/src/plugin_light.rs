//! PluginLight schema - plugin-defined light type.
//!
//! Light that provides properties allowing it to identify an external
//! SdrShadingNode definition through UsdShadeNodeDefAPI. This enables
//! render delegates to use custom light types without requiring a
//! schema definition for each light type.
//!
//! # Usage
//!
//! PluginLight extends Xformable and uses NodeDefAPI to specify the
//! shader node definition. The actual light parameters are defined
//! by the shader node, not by this schema.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdLux/pluginLight.h` and `pluginLight.cpp`

use std::sync::Arc;

use usd_core::{Prim, SchemaKind, Stage};
use usd_geom::{XformQuery, Xformable};
use usd_sdf::Path;
use usd_shade::NodeDefAPI;
use usd_tf::Token;

/// Plugin-defined light type.
///
/// Allows custom light types via UsdShadeNodeDefAPI without
/// requiring schema definitions. The light's shader ID and
/// parameters are discovered through the Sdr registry.
///
/// # Schema Kind
///
/// This is a ConcreteTyped schema.
#[derive(Clone)]
pub struct PluginLight {
    xformable: Xformable,
}

impl PluginLight {
    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "PluginLight";

    /// The schema kind.
    pub const SCHEMA_KIND: SchemaKind = SchemaKind::ConcreteTyped;

    // =========================================================================
    // Construction
    // =========================================================================

    /// Construct a PluginLight on the given prim.
    pub fn new(prim: Prim) -> Self {
        Self {
            xformable: Xformable::new(prim),
        }
    }

    /// Construct from Xformable.
    pub fn from_xformable(xformable: Xformable) -> Self {
        Self { xformable }
    }

    /// Create an invalid PluginLight.
    pub fn invalid() -> Self {
        Self {
            xformable: Xformable::invalid(),
        }
    }

    /// Returns true if this schema is valid.
    pub fn is_valid(&self) -> bool {
        self.xformable.is_valid()
    }

    /// Get the wrapped prim.
    pub fn get_prim(&self) -> &Prim {
        self.xformable.prim()
    }

    /// Get as Xformable.
    pub fn xformable(&self) -> &Xformable {
        &self.xformable
    }

    /// Get XformQuery for efficient transform computation.
    pub fn xform_query(&self) -> XformQuery {
        XformQuery::new()
    }

    /// Get the schema kind.
    pub fn get_schema_kind(&self) -> SchemaKind {
        Self::SCHEMA_KIND
    }

    /// Return a PluginLight holding the prim at `path` on `stage`.
    ///
    /// Matches C++ `UsdLuxPluginLight::Get()` — no type check performed.
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage.get_prim_at_path(path)?;
        Some(Self::new(prim))
    }

    /// Define a PluginLight at `path` on `stage`.
    pub fn define(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage
            .define_prim(path.as_str(), Self::SCHEMA_TYPE_NAME)
            .ok()?;
        Some(Self::new(prim))
    }

    // =========================================================================
    // NodeDefAPI Access
    // =========================================================================

    /// Get the UsdShadeNodeDefAPI for this prim.
    ///
    /// Provides access to the shader node definition that describes
    /// this light's parameters and behavior.
    pub fn get_node_def_api(&self) -> NodeDefAPI {
        NodeDefAPI::new(self.get_prim().clone())
    }

    // =========================================================================
    // Schema attribute names
    // =========================================================================

    /// Returns all pre-declared attributes for this schema class.
    ///
    /// PluginLight has no local attributes - all light parameters
    /// come from the shader node definition.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        if include_inherited {
            Xformable::get_schema_attribute_names(true)
        } else {
            Vec::new()
        }
    }
}

// ============================================================================
// Trait implementations
// ============================================================================

impl From<Prim> for PluginLight {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

impl From<Xformable> for PluginLight {
    fn from(xformable: Xformable) -> Self {
        Self::from_xformable(xformable)
    }
}

impl From<PluginLight> for Prim {
    fn from(light: PluginLight) -> Self {
        light.xformable.prim().clone()
    }
}

impl AsRef<Prim> for PluginLight {
    fn as_ref(&self) -> &Prim {
        self.get_prim()
    }
}

impl AsRef<Xformable> for PluginLight {
    fn as_ref(&self) -> &Xformable {
        &self.xformable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_type_name() {
        assert_eq!(PluginLight::SCHEMA_TYPE_NAME, "PluginLight");
    }

    #[test]
    fn test_schema_kind() {
        assert_eq!(PluginLight::SCHEMA_KIND, SchemaKind::ConcreteTyped);
    }
}
