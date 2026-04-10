//! UsdLuxRectLight - rectangular area light source.
//!
//! This module provides [`RectLight`], a light that emits from one side
//! of a rectangle.
//!
//! # Overview
//!
//! RectLight is a rectangular area light, commonly used for simulating soft
//! lighting from flat rectangular sources like windows, monitors, or soft boxes.
//! The rectangle is centered in the XY plane and emits light along the -Z axis.
//!
//! # Geometry
//!
//! - Shape: Rectangle
//! - Centered at local origin
//! - Lies in XY plane
//! - Emits along -Z axis (front face only)
//! - Size controlled by `inputs:width` and `inputs:height` attributes
//!
//! # Texture Support
//!
//! RectLight supports an optional color texture (`inputs:texture:file`) that
//! modulates the light emission across the surface. In the default position,
//! the texture's min coordinates map to (+X, +Y) and max coordinates to (-X, -Y).
//!
//! # Attributes
//!
//! | Attribute | Type | Default | Description |
//! |-----------|------|---------|-------------|
//! | `inputs:width` | float | 1.0 | Width in local X axis |
//! | `inputs:height` | float | 1.0 | Height in local Y axis |
//! | `inputs:texture:file` | asset | | Optional color texture |
//!
//! Plus all inherited [`LightAPI`](super::light_api::LightAPI) attributes.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdLux/rectLight.h`

use super::boundable_light_base::BoundableLightBase;
use super::light_api::LightAPI;
use super::tokens::tokens;
use crate::schema_create_attr::create_lux_schema_attr;
use std::sync::Arc;
use usd_core::attribute::Variability;
use usd_core::{Attribute, Prim, Stage};
use usd_sdf::Path;
use usd_vt::Value;
use usd_tf::Token;

/// Light emitted from one side of a rectangle.
///
/// The rectangle is centered in the XY plane and emits light along the -Z axis.
/// The rectangle is 1 unit in length in the X and Y axis by default.
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
///     -> UsdGeomBoundable
///       -> UsdLuxBoundableLightBase
///         -> UsdLuxRectLight
/// ```
///
/// # Attributes
///
/// | Attribute | Type | Default |
/// |-----------|------|---------|
/// | `inputs:width` | float | 1.0 |
/// | `inputs:height` | float | 1.0 |
/// | `inputs:texture:file` | asset | |
///
/// Matches C++ `UsdLuxRectLight`.
#[derive(Clone)]
pub struct RectLight {
    prim: Prim,
}

impl RectLight {
    // =========================================================================
    // Construction
    // =========================================================================

    /// Constructs a RectLight on the given prim.
    ///
    /// # Arguments
    /// * `prim` - The prim to wrap with this schema
    #[inline]
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Returns a RectLight holding the prim at `path` on `stage`.
    ///
    /// If no prim exists at the path, or the prim doesn't adhere to this schema,
    /// returns an invalid schema object.
    ///
    /// Matches C++ `UsdLuxRectLight::Get(stage, path)`.
    pub fn get(stage: &Stage, path: &Path) -> Self {
        match stage.get_prim_at_path(path) {
            Some(prim) => Self::new(prim),
            None => Self::invalid(),
        }
    }

    /// Creates an invalid RectLight.
    #[inline]
    pub fn invalid() -> Self {
        Self {
            prim: Prim::invalid(),
        }
    }

    /// Defines a RectLight at the given path on the stage.
    ///
    /// If a prim already exists at the path, it will be returned if it
    /// adheres to this schema. Otherwise, a new prim is created with
    /// the RectLight type.
    ///
    /// Matches C++ `UsdLuxRectLight::Define(stage, path)`.
    pub fn define(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage
            .define_prim(path.get_string(), tokens().rect_light.as_str())
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
    /// Matches C++ `UsdLuxRectLight::GetSchemaAttributeNames()`.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        let mut names = vec![
            tokens().inputs_width.clone(),
            tokens().inputs_height.clone(),
            tokens().inputs_texture_file.clone(),
        ];
        if include_inherited {
            names.extend(BoundableLightBase::get_schema_attribute_names(true));
        }
        names
    }

    // =========================================================================
    // Base Class Accessors
    // =========================================================================

    /// Returns this light as a BoundableLightBase.
    ///
    /// Provides access to all BoundableLightBase methods.
    #[inline]
    pub fn as_boundable_light_base(&self) -> BoundableLightBase {
        BoundableLightBase::new(self.prim.clone())
    }

    /// Returns the LightAPI for this light.
    ///
    /// Provides access to all common light attributes.
    #[inline]
    pub fn light_api(&self) -> LightAPI {
        LightAPI::new(self.prim.clone())
    }

    // =========================================================================
    // WIDTH Attribute
    // =========================================================================

    /// Returns the width attribute.
    ///
    /// Width of the rectangle, in the local X axis.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float inputs:width = 1` |
    /// | C++ Type | float |
    /// | Default | 1.0 |
    ///
    /// Matches C++ `UsdLuxRectLight::GetWidthAttr()`.
    #[inline]
    pub fn get_width_attr(&self) -> Option<Attribute> {
        self.prim.get_attribute(tokens().inputs_width.as_str())
    }

    /// Creates the width attribute.
    ///
    /// See [`get_width_attr`](Self::get_width_attr) for attribute details.
    ///
    /// Matches C++ `UsdLuxRectLight::CreateWidthAttr(VtValue const &defaultValue, bool writeSparsely)`.
    pub fn create_width_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        create_lux_schema_attr(
            &self.prim,
            tokens().inputs_width.as_str(),
            "float",
            Variability::Varying,
            default_value,
            write_sparsely,
        )
    }

    // =========================================================================
    // HEIGHT Attribute
    // =========================================================================

    /// Returns the height attribute.
    ///
    /// Height of the rectangle, in the local Y axis.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float inputs:height = 1` |
    /// | C++ Type | float |
    /// | Default | 1.0 |
    ///
    /// Matches C++ `UsdLuxRectLight::GetHeightAttr()`.
    #[inline]
    pub fn get_height_attr(&self) -> Option<Attribute> {
        self.prim.get_attribute(tokens().inputs_height.as_str())
    }

    /// Creates the height attribute.
    ///
    /// See [`get_height_attr`](Self::get_height_attr) for attribute details.
    ///
    /// Matches C++ `UsdLuxRectLight::CreateHeightAttr(VtValue const &defaultValue, bool writeSparsely)`.
    pub fn create_height_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        create_lux_schema_attr(
            &self.prim,
            tokens().inputs_height.as_str(),
            "float",
            Variability::Varying,
            default_value,
            write_sparsely,
        )
    }

    // =========================================================================
    // TEXTUREFILE Attribute
    // =========================================================================

    /// Returns the texture file attribute.
    ///
    /// A color texture to use on the rectangle.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `asset inputs:texture:file` |
    /// | C++ Type | SdfAssetPath |
    ///
    /// Matches C++ `UsdLuxRectLight::GetTextureFileAttr()`.
    #[inline]
    pub fn get_texture_file_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(tokens().inputs_texture_file.as_str())
    }

    /// Creates the texture file attribute.
    ///
    /// See [`get_texture_file_attr`](Self::get_texture_file_attr) for attribute details.
    ///
    /// Matches C++ `UsdLuxRectLight::CreateTextureFileAttr(VtValue const &defaultValue, bool writeSparsely)`.
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
}

impl Default for RectLight {
    fn default() -> Self {
        Self::invalid()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_rect_light() {
        let light = RectLight::default();
        assert!(!light.is_valid());
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = RectLight::get_schema_attribute_names(false);
        assert_eq!(names.len(), 3);

        let names_inherited = RectLight::get_schema_attribute_names(true);
        assert!(names_inherited.len() > 3);
    }
}
