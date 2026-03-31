//! DomeLight_1 schema - improved dome/environment light.
//!
//! Light emitted inward from a distant external environment, such as
//! a sky or IBL light probe. This version adds explicit pole axis control.
//!
//! # Pole Axis
//!
//! The dome's default orientation is determined by its `poleAxis` property:
//! - "scene" (default): Top pole aligned with stage's up axis
//! - "Y": Top pole aligned with +Y axis
//! - "Z": Top pole aligned with +Z axis
//!
//! The rotation to align with poleAxis is applied only to the dome itself,
//! not inherited by USD namespace children.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdLux/domeLight_1.h` and `domeLight_1.cpp`

use std::sync::Arc;

use usd_core::attribute::Variability;
use usd_core::{Attribute, Prim, Relationship, SchemaKind, Stage};
use usd_geom::XformQuery;
use usd_sdf::{Path, ValueTypeRegistry};
use usd_tf::Token;

use super::nonboundable_light_base::NonboundableLightBase;
use super::tokens::tokens;

/// Improved dome light with explicit pole axis control.
///
/// Light emitted inward from a distant external environment.
/// Uses `poleAxis` to determine initial orientation instead of
/// the deprecated `orientToStageUpAxis` attribute.
///
/// # Schema Kind
///
/// This is a ConcreteTyped schema.
#[derive(Clone)]
pub struct DomeLight1 {
    base: NonboundableLightBase,
}

impl DomeLight1 {
    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "DomeLight_1";

    /// The schema kind.
    pub const SCHEMA_KIND: SchemaKind = SchemaKind::ConcreteTyped;

    // =========================================================================
    // Construction
    // =========================================================================

    /// Construct a DomeLight_1 on the given prim.
    pub fn new(prim: Prim) -> Self {
        Self {
            base: NonboundableLightBase::new(prim),
        }
    }

    /// Construct from NonboundableLightBase.
    pub fn from_base(base: NonboundableLightBase) -> Self {
        Self { base }
    }

    /// Create an invalid DomeLight_1.
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
    pub fn get_prim(&self) -> &Prim {
        self.base.get_prim()
    }

    /// Get XformQuery for efficient transform computation.
    pub fn xform_query(&self) -> XformQuery {
        XformQuery::new()
    }

    /// Get the schema kind.
    pub fn get_schema_kind(&self) -> SchemaKind {
        Self::SCHEMA_KIND
    }

    /// Get the base NonboundableLightBase.
    pub fn base(&self) -> &NonboundableLightBase {
        &self.base
    }

    /// Return a DomeLight_1 holding the prim at `path` on `stage`.
    ///
    /// Matches C++ `UsdLuxDomeLight_1::Get()` — no type check performed.
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage.get_prim_at_path(path)?;
        Some(Self::new(prim))
    }

    /// Define a DomeLight_1 at `path` on `stage`.
    pub fn define(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage
            .define_prim(path.as_str(), Self::SCHEMA_TYPE_NAME)
            .ok()?;
        Some(Self::new(prim))
    }

    // =========================================================================
    // Texture File Attribute
    // =========================================================================

    /// Get the inputs:texture:file attribute.
    ///
    /// A color texture to use on the dome, such as an HDR texture
    /// intended for image-based lighting (IBL).
    pub fn get_texture_file_attr(&self) -> Option<Attribute> {
        self.get_prim()
            .get_attribute(tokens().inputs_texture_file.as_str())
    }

    /// Create the inputs:texture:file attribute.
    pub fn create_texture_file_attr(&self) -> Option<Attribute> {
        let registry = ValueTypeRegistry::instance();
        let asset_type = registry.find_type_by_token(&Token::new("asset"));

        self.get_prim().create_attribute(
            tokens().inputs_texture_file.as_str(),
            &asset_type,
            false,
            None,
        )
    }

    // =========================================================================
    // Texture Format Attribute
    // =========================================================================

    /// Get the inputs:texture:format attribute.
    ///
    /// Specifies the parameterization of the color map file:
    /// - automatic: Determine from file itself
    /// - latlong: Latitude/longitude mapping
    /// - mirroredBall: Mirrored ball projection
    /// - angular: Angular mapping (better edge sampling)
    /// - cubeMapVerticalCross: Vertical cross cube map
    ///
    /// Default: "automatic"
    pub fn get_texture_format_attr(&self) -> Option<Attribute> {
        self.get_prim()
            .get_attribute(tokens().inputs_texture_format.as_str())
    }

    /// Create the inputs:texture:format attribute.
    pub fn create_texture_format_attr(&self) -> Option<Attribute> {
        let registry = ValueTypeRegistry::instance();
        let token_type = registry.find_type_by_token(&Token::new("token"));

        self.get_prim().create_attribute(
            tokens().inputs_texture_format.as_str(),
            &token_type,
            false,
            None,
        )
    }

    // =========================================================================
    // Guide Radius Attribute
    // =========================================================================

    /// Get the guideRadius attribute.
    ///
    /// The radius of guide geometry to visualize the dome light.
    /// Default: 100000 (1 km for scenes with metersPerUnit = 0.01)
    pub fn get_guide_radius_attr(&self) -> Option<Attribute> {
        self.get_prim()
            .get_attribute(tokens().guide_radius.as_str())
    }

    /// Create the guideRadius attribute.
    pub fn create_guide_radius_attr(&self) -> Option<Attribute> {
        let registry = ValueTypeRegistry::instance();
        let float_type = registry.find_type_by_token(&Token::new("float"));

        self.get_prim()
            .create_attribute(tokens().guide_radius.as_str(), &float_type, false, None)
    }

    // =========================================================================
    // Pole Axis Attribute
    // =========================================================================

    /// Get the poleAxis attribute.
    ///
    /// A token indicating the starting alignment of the dome light's top pole:
    /// - scene: Aligned with stage's up axis
    /// - Y: Aligned with +Y axis
    /// - Z: Aligned with +Z axis
    ///
    /// Default: "scene"
    ///
    /// Note: This alignment is for the dome itself and is NOT inherited
    /// by namespace children.
    pub fn get_pole_axis_attr(&self) -> Option<Attribute> {
        self.get_prim().get_attribute(tokens().pole_axis.as_str())
    }

    /// Create the poleAxis attribute (uniform variability).
    pub fn create_pole_axis_attr(&self) -> Option<Attribute> {
        let registry = ValueTypeRegistry::instance();
        let token_type = registry.find_type_by_token(&Token::new("token"));

        self.get_prim().create_attribute(
            tokens().pole_axis.as_str(),
            &token_type,
            false,
            Some(Variability::Uniform),
        )
    }

    // =========================================================================
    // Portals Relationship
    // =========================================================================

    /// Get the portals relationship.
    ///
    /// Optional portals to guide light sampling.
    pub fn get_portals_rel(&self) -> Option<Relationship> {
        self.get_prim().get_relationship(tokens().portals.as_str())
    }

    /// Create the portals relationship.
    pub fn create_portals_rel(&self) -> Option<Relationship> {
        self.get_prim()
            .create_relationship(tokens().portals.as_str(), false)
    }

    // =========================================================================
    // Schema attribute names
    // =========================================================================

    /// Returns all pre-declared attributes for this schema class.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        let mut names = if include_inherited {
            NonboundableLightBase::get_schema_attribute_names(true)
        } else {
            Vec::new()
        };

        // Local attributes
        names.push(tokens().inputs_texture_file.clone());
        names.push(tokens().inputs_texture_format.clone());
        names.push(tokens().guide_radius.clone());
        names.push(tokens().pole_axis.clone());

        names
    }
}

// ============================================================================
// Trait implementations
// ============================================================================

impl From<Prim> for DomeLight1 {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

impl From<NonboundableLightBase> for DomeLight1 {
    fn from(base: NonboundableLightBase) -> Self {
        Self::from_base(base)
    }
}

impl From<DomeLight1> for Prim {
    fn from(light: DomeLight1) -> Self {
        light.base.get_prim().clone()
    }
}

impl AsRef<Prim> for DomeLight1 {
    fn as_ref(&self) -> &Prim {
        self.get_prim()
    }
}

impl AsRef<NonboundableLightBase> for DomeLight1 {
    fn as_ref(&self) -> &NonboundableLightBase {
        &self.base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_type_name() {
        assert_eq!(DomeLight1::SCHEMA_TYPE_NAME, "DomeLight_1");
    }

    #[test]
    fn test_schema_kind() {
        assert_eq!(DomeLight1::SCHEMA_KIND, SchemaKind::ConcreteTyped);
    }
}
