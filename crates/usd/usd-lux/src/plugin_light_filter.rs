//! PluginLightFilter schema - plugin-defined light filter type.
//!
//! Light filter that provides properties allowing it to identify an external
//! SdrShadingNode definition through UsdShadeNodeDefAPI. This enables
//! render delegates to use custom light filter types without requiring a
//! schema definition for each filter type.
//!
//! # Usage
//!
//! PluginLightFilter extends LightFilter and uses NodeDefAPI to specify the
//! shader node definition. The actual filter parameters are defined by the
//! shader node, not by this schema.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdLux/pluginLightFilter.h` and `pluginLightFilter.cpp`

use std::sync::Arc;

use usd_core::{Prim, SchemaKind, Stage};
use usd_geom::XformQuery;
use usd_sdf::Path;
use usd_shade::NodeDefAPI;
use usd_tf::Token;

use super::light_filter::LightFilter;

/// Plugin-defined light filter type.
///
/// Allows custom light filter types via UsdShadeNodeDefAPI without
/// requiring schema definitions. The filter's shader ID and
/// parameters are discovered through the Sdr registry.
///
/// # Schema Kind
///
/// This is a ConcreteTyped schema.
#[derive(Clone)]
pub struct PluginLightFilter {
    base: LightFilter,
}

impl PluginLightFilter {
    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "PluginLightFilter";

    /// The schema kind.
    pub const SCHEMA_KIND: SchemaKind = SchemaKind::ConcreteTyped;

    // =========================================================================
    // Construction
    // =========================================================================

    /// Construct a PluginLightFilter on the given prim.
    pub fn new(prim: Prim) -> Self {
        Self {
            base: LightFilter::new(prim),
        }
    }

    /// Construct from LightFilter.
    pub fn from_light_filter(filter: LightFilter) -> Self {
        Self { base: filter }
    }

    /// Create an invalid PluginLightFilter.
    pub fn invalid() -> Self {
        Self {
            base: LightFilter::invalid(),
        }
    }

    /// Returns true if this schema is valid.
    pub fn is_valid(&self) -> bool {
        self.base.is_valid()
    }

    /// Get the wrapped prim.
    pub fn get_prim(&self) -> &Prim {
        self.base.prim()
    }

    /// Get the base LightFilter.
    pub fn light_filter(&self) -> &LightFilter {
        &self.base
    }

    /// Get XformQuery for efficient transform computation.
    pub fn xform_query(&self) -> XformQuery {
        XformQuery::new()
    }

    /// Get the schema kind.
    pub fn get_schema_kind(&self) -> SchemaKind {
        Self::SCHEMA_KIND
    }

    /// Return a PluginLightFilter holding the prim at `path` on `stage`.
    ///
    /// Matches C++ `UsdLuxPluginLightFilter::Get()` — no type check performed.
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage.get_prim_at_path(path)?;
        Some(Self::new(prim))
    }

    /// Define a PluginLightFilter at `path` on `stage`.
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
    /// this light filter's parameters and behavior.
    pub fn get_node_def_api(&self) -> NodeDefAPI {
        NodeDefAPI::new(self.get_prim().clone())
    }

    // =========================================================================
    // Schema attribute names
    // =========================================================================

    /// Returns all pre-declared attributes for this schema class.
    ///
    /// PluginLightFilter has no local attributes beyond those inherited
    /// from LightFilter - all filter parameters come from the shader
    /// node definition.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        if include_inherited {
            LightFilter::get_schema_attribute_names(true)
        } else {
            Vec::new()
        }
    }
}

// ============================================================================
// Trait implementations
// ============================================================================

impl From<Prim> for PluginLightFilter {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

impl From<LightFilter> for PluginLightFilter {
    fn from(filter: LightFilter) -> Self {
        Self::from_light_filter(filter)
    }
}

impl From<PluginLightFilter> for Prim {
    fn from(filter: PluginLightFilter) -> Self {
        filter.base.prim().clone()
    }
}

impl AsRef<Prim> for PluginLightFilter {
    fn as_ref(&self) -> &Prim {
        self.get_prim()
    }
}

impl AsRef<LightFilter> for PluginLightFilter {
    fn as_ref(&self) -> &LightFilter {
        &self.base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_type_name() {
        assert_eq!(PluginLightFilter::SCHEMA_TYPE_NAME, "PluginLightFilter");
    }

    #[test]
    fn test_schema_kind() {
        assert_eq!(PluginLightFilter::SCHEMA_KIND, SchemaKind::ConcreteTyped);
    }
}
