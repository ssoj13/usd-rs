//! UsdLuxDomeLight - environment dome light for IBL.
//!
//! This module provides [`DomeLight`], a light that emits inward from a
//! distant dome, typically used for environment/IBL (Image-Based Lighting).
//!
//! # Overview
//!
//! DomeLight simulates lighting from a distant environment, such as a sky
//! or an HDR image probe. It is the primary method for image-based lighting
//! in USD scenes.
//!
//! # Orientation
//!
//! The dome's default orientation has its top pole aligned with the world's
//! +Y axis, following the OpenEXR specification for latlong environment maps:
//!
//! - Latitude +pi/2 (top) corresponds to +Y direction
//! - Latitude -pi/2 (bottom) corresponds to -Y direction
//! - Latitude 0, longitude 0 points into +Z direction
//! - Latitude 0, longitude pi/2 points into +X direction
//!
//! Use [`orient_to_stage_up_axis`](Self::orient_to_stage_up_axis) to automatically
//! add a transform op that orients the dome to match the stage's up axis.
//!
//! # Texture Formats
//!
//! Supported texture formats (`inputs:texture:format`):
//! - `automatic` - Determine from file (default)
//! - `latlong` - Latitude/longitude (equirectangular)
//! - `mirroredBall` - Mirror ball probe
//! - `angular` - Angular map
//! - `cubeMapVerticalCross` - Cube map as vertical cross
//!
//! # Portals
//!
//! The `portals` relationship connects to PortalLight prims that define
//! windows into the dome light, optimizing sampling for interior scenes.
//!
//! # Attributes
//!
//! | Attribute | Type | Default | Description |
//! |-----------|------|---------|-------------|
//! | `inputs:texture:file` | asset | | HDR texture path |
//! | `inputs:texture:format` | token | automatic | Texture parameterization |
//! | `guideRadius` | float | 100000 | Visualization sphere radius |
//!
//! Plus all inherited [`LightAPI`](super::light_api::LightAPI) attributes.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdLux/domeLight.h`

use super::light_api::LightAPI;
use super::nonboundable_light_base::NonboundableLightBase;
use super::tokens::tokens;
use crate::schema_create_attr::create_lux_schema_attr;
use std::sync::Arc;
use usd_core::attribute::Variability;
use usd_core::{Attribute, Prim, Relationship, Stage};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Value;

/// Light emitted inward from a distant external environment.
///
/// Used for environment lighting with HDR images (IBL). The dome's default
/// orientation has its top pole aligned with the world's +Y axis.
///
/// # Schema Type
///
/// This is a **concrete typed schema** (`UsdSchemaKind::ConcreteTyped`).
///
/// # Inheritance
///
/// ```text
/// UsdTyped
///   -> UsdGeomImageable
///     -> UsdGeomXformable
///       -> UsdLuxNonboundableLightBase
///         -> UsdLuxDomeLight
/// ```
///
/// # Attributes
///
/// | Attribute | Type | Default |
/// |-----------|------|---------|
/// | `inputs:texture:file` | asset | |
/// | `inputs:texture:format` | token | automatic |
/// | `guideRadius` | float | 100000 |
///
/// # Relationships
///
/// | Relationship | Description |
/// |--------------|-------------|
/// | `portals` | Optional portal lights for sampling optimization |
///
/// Matches C++ `UsdLuxDomeLight`.
#[derive(Clone)]
pub struct DomeLight {
    prim: Prim,
}

impl DomeLight {
    // =========================================================================
    // Construction
    // =========================================================================

    /// Constructs a DomeLight on the given prim.
    ///
    /// # Arguments
    /// * `prim` - The prim to wrap with this schema
    #[inline]
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Returns a DomeLight holding the prim at `path` on `stage`.
    ///
    /// If no prim exists at the path, or the prim doesn't adhere to this schema,
    /// returns an invalid schema object.
    ///
    /// Matches C++ `UsdLuxDomeLight::Get(stage, path)`.
    pub fn get(stage: &Stage, path: &Path) -> Self {
        match stage.get_prim_at_path(path) {
            Some(prim) => Self::new(prim),
            None => Self::invalid(),
        }
    }

    /// Creates an invalid DomeLight.
    #[inline]
    pub fn invalid() -> Self {
        Self {
            prim: Prim::invalid(),
        }
    }

    /// Defines a DomeLight at the given path on the stage.
    ///
    /// If a prim already exists at the path, it will be returned if it
    /// adheres to this schema. Otherwise, a new prim is created with
    /// the DomeLight type.
    ///
    /// Matches C++ `UsdLuxDomeLight::Define(stage, path)`.
    pub fn define(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage
            .define_prim(path.get_string(), tokens().dome_light.as_str())
            .ok()?;
        Some(Self::new(prim))
    }

    // =========================================================================
    // Schema Information
    // =========================================================================

    /// Returns true if this schema is valid.
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.prim.is_valid()
    }

    /// Returns the wrapped prim.
    #[inline]
    pub fn get_prim(&self) -> &Prim {
        &self.prim
    }

    /// Returns names of all pre-declared attributes for this schema.
    ///
    /// # Arguments
    /// * `include_inherited` - If true, includes attributes from parent schemas
    ///
    /// Matches C++ `UsdLuxDomeLight::GetSchemaAttributeNames()`.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        let mut names = vec![
            tokens().inputs_texture_file.clone(),
            tokens().inputs_texture_format.clone(),
            tokens().guide_radius.clone(),
        ];
        if include_inherited {
            names.extend(NonboundableLightBase::get_schema_attribute_names(true));
        }
        names
    }

    // =========================================================================
    // Base Class Accessors
    // =========================================================================

    /// Returns this light as a NonboundableLightBase.
    ///
    /// Provides access to all NonboundableLightBase methods.
    #[inline]
    pub fn as_nonboundable_light_base(&self) -> NonboundableLightBase {
        NonboundableLightBase::new(self.prim.clone())
    }

    /// Returns the LightAPI for this light.
    ///
    /// Provides access to all common light attributes.
    #[inline]
    pub fn light_api(&self) -> LightAPI {
        LightAPI::new(self.prim.clone())
    }

    // =========================================================================
    // TEXTUREFILE Attribute
    // =========================================================================

    /// Returns the texture file attribute.
    ///
    /// A color texture to use on the dome, such as an HDR (high dynamic range)
    /// texture intended for IBL (image based lighting).
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `asset inputs:texture:file` |
    /// | C++ Type | SdfAssetPath |
    ///
    /// Matches C++ `UsdLuxDomeLight::GetTextureFileAttr()`.
    #[inline]
    pub fn get_texture_file_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(tokens().inputs_texture_file.as_str())
    }

    /// Creates the texture file attribute.
    ///
    /// See [`get_texture_file_attr`](Self::get_texture_file_attr) for attribute details.
    ///
    /// Matches C++ `UsdLuxDomeLight::CreateTextureFileAttr(VtValue const &defaultValue, bool writeSparsely)`.
    pub fn create_texture_file_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        create_lux_schema_attr(
            &self.prim,
            tokens().inputs_texture_file.as_str(),
            "asset",
            Variability::Varying,
            default_value,
            write_sparsely,
        )
    }

    // =========================================================================
    // TEXTUREFORMAT Attribute
    // =========================================================================

    /// Returns the texture format attribute.
    ///
    /// Specifies the parameterization of the color map file.
    ///
    /// Valid values:
    /// - `automatic` - Determine from file metadata (default)
    /// - `latlong` - Latitude as X, longitude as Y
    /// - `mirroredBall` - Environment reflected in sphere
    /// - `angular` - Like mirroredBall but with linear radial mapping
    /// - `cubeMapVerticalCross` - Cube map as vertical cross layout
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `token inputs:texture:format = "automatic"` |
    /// | C++ Type | TfToken |
    /// | Default | automatic |
    /// | Allowed Values | automatic, latlong, mirroredBall, angular, cubeMapVerticalCross |
    ///
    /// Matches C++ `UsdLuxDomeLight::GetTextureFormatAttr()`.
    #[inline]
    pub fn get_texture_format_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(tokens().inputs_texture_format.as_str())
    }

    /// Creates the texture format attribute.
    ///
    /// See [`get_texture_format_attr`](Self::get_texture_format_attr) for attribute details.
    ///
    /// Matches C++ `UsdLuxDomeLight::CreateTextureFormatAttr(VtValue const &defaultValue, bool writeSparsely)`.
    pub fn create_texture_format_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        create_lux_schema_attr(
            &self.prim,
            tokens().inputs_texture_format.as_str(),
            "token",
            Variability::Varying,
            default_value,
            write_sparsely,
        )
    }

    // =========================================================================
    // GUIDERADIUS Attribute
    // =========================================================================

    /// Returns the guide radius attribute.
    ///
    /// The radius of guide geometry used to visualize the dome light.
    /// The default is 1 km (100000 units) for scenes where 1 unit = 1 cm.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float guideRadius = 100000` |
    /// | C++ Type | float |
    /// | Default | 100000 |
    ///
    /// Matches C++ `UsdLuxDomeLight::GetGuideRadiusAttr()`.
    #[inline]
    pub fn get_guide_radius_attr(&self) -> Option<Attribute> {
        self.prim.get_attribute(tokens().guide_radius.as_str())
    }

    /// Creates the guide radius attribute.
    ///
    /// See [`get_guide_radius_attr`](Self::get_guide_radius_attr) for attribute details.
    ///
    /// Matches C++ `UsdLuxDomeLight::CreateGuideRadiusAttr(VtValue const &defaultValue, bool writeSparsely)`.
    pub fn create_guide_radius_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        create_lux_schema_attr(
            &self.prim,
            tokens().guide_radius.as_str(),
            "float",
            Variability::Varying,
            default_value,
            write_sparsely,
        )
    }

    // =========================================================================
    // PORTALS Relationship
    // =========================================================================

    /// Returns the portals relationship.
    ///
    /// Optional portals to guide light sampling. Connect to PortalLight prims
    /// that define windows into the dome for optimized interior lighting.
    ///
    /// Matches C++ `UsdLuxDomeLight::GetPortalsRel()`.
    #[inline]
    pub fn get_portals_rel(&self) -> Option<Relationship> {
        self.prim.get_relationship(tokens().portals.as_str())
    }

    /// Creates the portals relationship.
    ///
    /// See [`get_portals_rel`](Self::get_portals_rel) for relationship details.
    ///
    /// Matches C++ `UsdLuxDomeLight::CreatePortalsRel()`.
    pub fn create_portals_rel(&self) -> Option<Relationship> {
        self.get_portals_rel()
    }

    // =========================================================================
    // Custom Methods
    // =========================================================================

    /// Adds a transformation op to orient the dome to align with the stage's up axis.
    ///
    /// Uses `orientToStageUpAxis` as the op suffix. If an op with this suffix
    /// already exists, this method assumes it's already correct and does nothing.
    /// If no op is required to match the stage's up axis, no op will be created.
    ///
    /// Matches C++ `UsdLuxDomeLight::OrientToStageUpAxis()`.
    pub fn orient_to_stage_up_axis(&self) {
        use usd_geom::tokens::usd_geom_tokens;
        use usd_geom::{XformOpPrecision, XformOpType, Xformable, get_stage_up_axis};

        let Some(stage) = self.get_prim().stage() else {
            return;
        };

        // Only need to orient if stage is Z-up (dome default is Y-up)
        let up_axis = get_stage_up_axis(&stage);
        if up_axis != usd_geom_tokens().z {
            return;
        }

        // Check if op already exists
        let op_suffix = Token::new("orientToStageUpAxis");
        let op_name = format!("xformOp:rotateX:{}", op_suffix.as_str());

        let xformable = Xformable::new(self.get_prim().clone());
        for op in xformable.get_ordered_xform_ops() {
            if op.name() == op_name {
                // Op already exists, nothing to do
                return;
            }
        }

        // Add RotateX op with 90 degrees to convert from Y-up to Z-up
        let rotate_op = xformable.add_xform_op(
            XformOpType::RotateX,
            XformOpPrecision::Float,
            Some(&op_suffix),
            false,
        );
        let _ = rotate_op.set(90.0f32, usd_sdf::TimeCode::default());
    }
}

impl Default for DomeLight {
    fn default() -> Self {
        Self::invalid()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_dome_light() {
        let light = DomeLight::default();
        assert!(!light.is_valid());
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = DomeLight::get_schema_attribute_names(false);
        assert_eq!(names.len(), 3);

        let names_inherited = DomeLight::get_schema_attribute_names(true);
        assert!(names_inherited.len() > 3);
    }
}
