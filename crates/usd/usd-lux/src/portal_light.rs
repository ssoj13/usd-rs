//! Portal Light schema.
//!
//! A rectangular portal in the local XY plane that guides sampling
//! of a dome light. Transmits light in the -Z direction.
//! The rectangle is 1 unit in length by default.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdLux/portalLight.h` and `portalLight.cpp`

use std::sync::Arc;

use usd_core::attribute::Variability;
use usd_core::{Attribute, Prim, SchemaKind, Stage};
use usd_sdf::{Path, TimeCode, ValueTypeRegistry};
use usd_tf::Token;

use super::boundable_light_base::BoundableLightBase;
use super::tokens::tokens;

/// A rectangular portal that guides dome light sampling.
///
/// Portal lights are placed in scene openings (windows, doors) to improve
/// importance sampling of dome lights in interior scenes. They define
/// rectangular regions that transmit light in the -Z direction.
///
/// # Schema Kind
///
/// This is a ConcreteTyped schema extending BoundableLightBase.
#[derive(Clone)]
pub struct PortalLight {
    base: BoundableLightBase,
}

impl PortalLight {
    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "PortalLight";

    /// The schema kind.
    pub const SCHEMA_KIND: SchemaKind = SchemaKind::ConcreteTyped;

    // =========================================================================
    // Construction
    // =========================================================================

    /// Construct a PortalLight on the given prim.
    pub fn new(prim: Prim) -> Self {
        Self {
            base: BoundableLightBase::new(prim),
        }
    }

    /// Construct from BoundableLightBase.
    pub fn from_base(base: BoundableLightBase) -> Self {
        Self { base }
    }

    /// Create an invalid PortalLight.
    pub fn invalid() -> Self {
        Self {
            base: BoundableLightBase::invalid(),
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

    /// Get as BoundableLightBase.
    pub fn base(&self) -> &BoundableLightBase {
        &self.base
    }

    /// Get the schema kind.
    pub fn get_schema_kind(&self) -> SchemaKind {
        Self::SCHEMA_KIND
    }

    /// Return a PortalLight holding the prim at `path` on `stage`.
    ///
    /// Matches C++ `UsdLuxPortalLight::Get()` — no type check performed.
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage.get_prim_at_path(path)?;
        Some(Self::new(prim))
    }

    /// Define a PortalLight at `path` on `stage`.
    pub fn define(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage
            .define_prim(path.as_str(), Self::SCHEMA_TYPE_NAME)
            .ok()?;
        Some(Self::new(prim))
    }

    // =========================================================================
    // Width Attribute
    // =========================================================================

    /// Get the inputs:width attribute.
    ///
    /// Width of the portal rectangle in the local X axis.
    /// Default: 1.0
    pub fn get_width_attr(&self) -> Option<Attribute> {
        self.prim().get_attribute(tokens().inputs_width.as_str())
    }

    /// Create the inputs:width attribute.
    pub fn create_width_attr(&self, default_value: Option<f32>) -> Option<Attribute> {
        let registry = ValueTypeRegistry::instance();
        let float_type = registry.find_type_by_token(&Token::new("float"));

        let attr = self.prim().create_attribute(
            tokens().inputs_width.as_str(),
            &float_type,
            false,
            Some(Variability::Varying),
        )?;

        if let Some(value) = default_value {
            attr.set(value, TimeCode::default());
        }

        Some(attr)
    }

    /// Get the width value at the given time.
    pub fn get_width(&self, time: TimeCode) -> Option<f32> {
        self.get_width_attr()?.get_typed::<f32>(time)
    }

    // =========================================================================
    // Height Attribute
    // =========================================================================

    /// Get the inputs:height attribute.
    ///
    /// Height of the portal rectangle in the local Y axis.
    /// Default: 1.0
    pub fn get_height_attr(&self) -> Option<Attribute> {
        self.prim().get_attribute(tokens().inputs_height.as_str())
    }

    /// Create the inputs:height attribute.
    pub fn create_height_attr(&self, default_value: Option<f32>) -> Option<Attribute> {
        let registry = ValueTypeRegistry::instance();
        let float_type = registry.find_type_by_token(&Token::new("float"));

        let attr = self.prim().create_attribute(
            tokens().inputs_height.as_str(),
            &float_type,
            false,
            Some(Variability::Varying),
        )?;

        if let Some(value) = default_value {
            attr.set(value, TimeCode::default());
        }

        Some(attr)
    }

    /// Get the height value at the given time.
    pub fn get_height(&self, time: TimeCode) -> Option<f32> {
        self.get_height_attr()?.get_typed::<f32>(time)
    }

    // =========================================================================
    // Schema attribute names
    // =========================================================================

    /// Returns all pre-declared attributes for this schema class.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        let mut names = if include_inherited {
            BoundableLightBase::get_schema_attribute_names(true)
        } else {
            Vec::new()
        };

        names.extend([
            tokens().inputs_width.clone(),
            tokens().inputs_height.clone(),
        ]);

        names
    }
}

// ============================================================================
// Trait implementations
// ============================================================================

impl From<Prim> for PortalLight {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

impl From<BoundableLightBase> for PortalLight {
    fn from(base: BoundableLightBase) -> Self {
        Self::from_base(base)
    }
}

impl From<PortalLight> for Prim {
    fn from(light: PortalLight) -> Self {
        light.base.get_prim().clone()
    }
}

impl AsRef<Prim> for PortalLight {
    fn as_ref(&self) -> &Prim {
        self.prim()
    }
}

impl AsRef<BoundableLightBase> for PortalLight {
    fn as_ref(&self) -> &BoundableLightBase {
        &self.base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_type_name() {
        assert_eq!(PortalLight::SCHEMA_TYPE_NAME, "PortalLight");
    }

    #[test]
    fn test_schema_kind() {
        assert_eq!(PortalLight::SCHEMA_KIND, SchemaKind::ConcreteTyped);
    }
}
