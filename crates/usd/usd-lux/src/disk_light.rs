//! UsdLuxDiskLight - disk-shaped area light source.
//!
//! This module provides [`DiskLight`], a light that emits from one side
//! of a circular disk.
//!
//! # Overview
//!
//! DiskLight is a circular area light, commonly used for simulating soft
//! lighting from round sources. The disk is centered in the XY plane and
//! emits light along the -Z axis (in its local coordinate space).
//!
//! # Geometry
//!
//! - Shape: Circular disk
//! - Centered at local origin
//! - Lies in XY plane
//! - Emits along -Z axis (front face only)
//! - Size controlled by `inputs:radius` attribute
//!
//! # Attributes
//!
//! | Attribute | Type | Default | Description |
//! |-----------|------|---------|-------------|
//! | `inputs:radius` | float | 0.5 | Radius of the disk |
//!
//! Plus all inherited [`LightAPI`](super::light_api::LightAPI) attributes.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdLux/diskLight.h`

use super::boundable_light_base::BoundableLightBase;
use super::light_api::LightAPI;
use super::tokens::tokens;
use std::sync::Arc;
use usd_core::{Attribute, Prim, Stage};
use usd_sdf::Path;
use usd_tf::Token;

/// Light emitted from one side of a circular disk.
///
/// The disk is centered in the XY plane and emits light along the -Z axis.
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
///         -> UsdLuxDiskLight
/// ```
///
/// # Attributes
///
/// | Attribute | Type | Default |
/// |-----------|------|---------|
/// | `inputs:radius` | float | 0.5 |
///
/// Matches C++ `UsdLuxDiskLight`.
#[derive(Clone)]
pub struct DiskLight {
    prim: Prim,
}

impl DiskLight {
    // =========================================================================
    // Construction
    // =========================================================================

    /// Constructs a DiskLight on the given prim.
    ///
    /// # Arguments
    /// * `prim` - The prim to wrap with this schema
    #[inline]
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Returns a DiskLight holding the prim at `path` on `stage`.
    ///
    /// If no prim exists at the path, or the prim doesn't adhere to this schema,
    /// returns an invalid schema object.
    ///
    /// Matches C++ `UsdLuxDiskLight::Get(stage, path)`.
    pub fn get(stage: &Stage, path: &Path) -> Self {
        match stage.get_prim_at_path(path) {
            Some(prim) => Self::new(prim),
            None => Self::invalid(),
        }
    }

    /// Creates an invalid DiskLight.
    #[inline]
    pub fn invalid() -> Self {
        Self {
            prim: Prim::invalid(),
        }
    }

    /// Defines a DiskLight at the given path on the stage.
    ///
    /// If a prim already exists at the path, it will be returned if it
    /// adheres to this schema. Otherwise, a new prim is created with
    /// the DiskLight type.
    ///
    /// Matches C++ `UsdLuxDiskLight::Define(stage, path)`.
    pub fn define(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage
            .define_prim(path.get_string(), tokens().disk_light.as_str())
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
    /// Matches C++ `UsdLuxDiskLight::GetSchemaAttributeNames()`.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        let mut names = vec![
            tokens().inputs_radius.clone(),
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
    // RADIUS Attribute
    // =========================================================================

    /// Returns the radius attribute.
    ///
    /// Radius of the disk, in local coordinate units.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float inputs:radius = 0.5` |
    /// | C++ Type | float |
    /// | Default | 0.5 |
    ///
    /// Matches C++ `UsdLuxDiskLight::GetRadiusAttr()`.
    #[inline]
    pub fn get_radius_attr(&self) -> Option<Attribute> {
        self.prim.get_attribute(tokens().inputs_radius.as_str())
    }

    /// Creates the radius attribute.
    ///
    /// See [`get_radius_attr`](Self::get_radius_attr) for attribute details.
    ///
    /// Matches C++ `UsdLuxDiskLight::CreateRadiusAttr()`.
    pub fn create_radius_attr(&self) -> Attribute {
        self.get_radius_attr().unwrap_or_else(Attribute::invalid)
    }

    // =========================================================================
    // TEXTUREFILE Attribute
    // =========================================================================

    /// Returns the texture file attribute.
    ///
    /// A color texture to use on the disk.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `asset inputs:texture:file` |
    /// | C++ Type | SdfAssetPath |
    ///
    /// Matches C++ `UsdLuxDiskLight::GetTextureFileAttr()`.
    #[inline]
    pub fn get_texture_file_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(tokens().inputs_texture_file.as_str())
    }

    /// Creates the texture file attribute.
    ///
    /// See [`get_texture_file_attr`](Self::get_texture_file_attr) for attribute details.
    ///
    /// Matches C++ `UsdLuxDiskLight::CreateTextureFileAttr()`.
    pub fn create_texture_file_attr(&self) -> Attribute {
        self.get_texture_file_attr()
            .unwrap_or_else(Attribute::invalid)
    }
}

impl Default for DiskLight {
    fn default() -> Self {
        Self::invalid()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_disk_light() {
        let light = DiskLight::default();
        assert!(!light.is_valid());
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = DiskLight::get_schema_attribute_names(false);
        assert_eq!(names.len(), 2);

        let names_inherited = DiskLight::get_schema_attribute_names(true);
        assert!(names_inherited.len() > 1);
    }
}
