//! UsdLuxShadowAPI - API for controlling light shadow behavior.
//!
//! This module provides [`ShadowAPI`], a single-apply API schema that
//! controls shadow-related properties of lights.
//!
//! # Overview
//!
//! ShadowAPI provides non-physical controls for refining a light's shadow
//! behavior. These controls are valuable for visual lighting work where
//! artistic control takes precedence over physical accuracy.
//!
//! # Shadow Properties
//!
//! | Attribute | Type | Default | Description |
//! |-----------|------|---------|-------------|
//! | `inputs:shadow:enable` | bool | true | Enable/disable shadows |
//! | `inputs:shadow:color` | color3f | (0,0,0) | Shadow color (non-physical) |
//! | `inputs:shadow:distance` | float | -1 | Max shadow distance (-1 = unlimited) |
//! | `inputs:shadow:falloff` | float | -1 | Falloff zone size (-1 = no falloff) |
//! | `inputs:shadow:falloffGamma` | float | 1.0 | Falloff gamma curve |
//!
//! # Schema Type
//!
//! This is a **single-apply API schema** (`UsdSchemaKind::SingleApplyAPI`).
//! Apply it using [`apply`](Self::apply) to add shadow controls to a light prim.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdLux/shadowAPI.h`

use super::tokens::tokens;
use usd_core::{Attribute, Prim, Stage};
use usd_sdf::Path;
use usd_sdf::ValueTypeName;
use usd_shade::{ConnectableAPI, Input, Output};
use usd_tf::Token;

/// API schema for shadow-related controls on lights.
///
/// Provides non-physical controls that are valuable for visual lighting work.
///
/// # Schema Type
///
/// This is a **single-apply API schema** (`UsdSchemaKind::SingleApplyAPI`).
/// Use [`apply`](Self::apply) to add this schema to a prim.
///
/// # Attributes
///
/// | Attribute | Type | Default |
/// |-----------|------|---------|
/// | `inputs:shadow:enable` | bool | true |
/// | `inputs:shadow:color` | color3f | (0,0,0) |
/// | `inputs:shadow:distance` | float | -1 |
/// | `inputs:shadow:falloff` | float | -1 |
/// | `inputs:shadow:falloffGamma` | float | 1.0 |
///
/// Matches C++ `UsdLuxShadowAPI`.
#[derive(Clone)]
pub struct ShadowAPI {
    prim: Prim,
}

impl ShadowAPI {
    // =========================================================================
    // Construction
    // =========================================================================

    /// Constructs a ShadowAPI on the given prim.
    ///
    /// # Arguments
    /// * `prim` - The prim to wrap with this API schema
    #[inline]
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Returns a ShadowAPI holding the prim at `path` on `stage`.
    ///
    /// If no prim exists at the path, returns an invalid schema object.
    ///
    /// Matches C++ `UsdLuxShadowAPI::Get(stage, path)`.
    pub fn get(stage: &Stage, path: &Path) -> Self {
        match stage.get_prim_at_path(path) {
            Some(prim) => Self::new(prim),
            None => Self::invalid(),
        }
    }

    /// Creates an invalid ShadowAPI.
    #[inline]
    pub fn invalid() -> Self {
        Self {
            prim: Prim::invalid(),
        }
    }

    /// Returns true if this API schema can be applied to the given prim.
    ///
    /// Matches C++ `UsdLuxShadowAPI::CanApply(prim)`.
    pub fn can_apply(prim: &Prim) -> bool {
        prim.is_valid()
    }

    /// Applies ShadowAPI to the given prim.
    ///
    /// This adds "ShadowAPI" to the prim's `apiSchemas` metadata.
    ///
    /// # Returns
    /// A valid ShadowAPI on success, or `None` if the prim is invalid.
    ///
    /// Matches C++ `UsdLuxShadowAPI::Apply(prim)`.
    pub fn apply(prim: &Prim) -> Option<Self> {
        if !prim.is_valid() {
            return None;
        }
        prim.apply_api(&tokens().shadow_api);
        Some(Self::new(prim.clone()))
    }

    // =========================================================================
    // Schema Information
    // =========================================================================

    /// Returns true if this API schema is valid.
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
    /// * `_include_inherited` - Unused for API schemas (no inheritance)
    ///
    /// Matches C++ `UsdLuxShadowAPI::GetSchemaAttributeNames()`.
    pub fn get_schema_attribute_names(_include_inherited: bool) -> Vec<Token> {
        vec![
            tokens().inputs_shadow_enable.clone(),
            tokens().inputs_shadow_color.clone(),
            tokens().inputs_shadow_distance.clone(),
            tokens().inputs_shadow_falloff.clone(),
            tokens().inputs_shadow_falloff_gamma.clone(),
        ]
    }

    // =========================================================================
    // SHADOW:ENABLE Attribute
    // =========================================================================

    /// Returns the shadow:enable attribute.
    ///
    /// Enables shadows to be cast by this light.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `bool inputs:shadow:enable = 1` |
    /// | C++ Type | bool |
    /// | Default | true |
    ///
    /// Matches C++ `UsdLuxShadowAPI::GetShadowEnableAttr()`.
    #[inline]
    pub fn get_shadow_enable_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(tokens().inputs_shadow_enable.as_str())
    }

    /// Creates the shadow:enable attribute.
    ///
    /// See [`get_shadow_enable_attr`](Self::get_shadow_enable_attr) for attribute details.
    ///
    /// Matches C++ `UsdLuxShadowAPI::CreateShadowEnableAttr()`.
    pub fn create_shadow_enable_attr(&self) -> Attribute {
        self.get_shadow_enable_attr()
            .unwrap_or_else(Attribute::invalid)
    }

    // =========================================================================
    // SHADOW:COLOR Attribute
    // =========================================================================

    /// Returns the shadow:color attribute.
    ///
    /// The color of shadows cast by the light. This is a non-physical control.
    /// The default is to cast black shadows.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `color3f inputs:shadow:color = (0, 0, 0)` |
    /// | C++ Type | GfVec3f |
    /// | Default | (0, 0, 0) |
    ///
    /// Matches C++ `UsdLuxShadowAPI::GetShadowColorAttr()`.
    #[inline]
    pub fn get_shadow_color_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(tokens().inputs_shadow_color.as_str())
    }

    /// Creates the shadow:color attribute.
    ///
    /// See [`get_shadow_color_attr`](Self::get_shadow_color_attr) for attribute details.
    ///
    /// Matches C++ `UsdLuxShadowAPI::CreateShadowColorAttr()`.
    pub fn create_shadow_color_attr(&self) -> Attribute {
        self.get_shadow_color_attr()
            .unwrap_or_else(Attribute::invalid)
    }

    // =========================================================================
    // SHADOW:DISTANCE Attribute
    // =========================================================================

    /// Returns the shadow:distance attribute.
    ///
    /// The maximum distance shadows are cast. Distance is measured between
    /// the surface point and the occluder. The default (-1) indicates no limit.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float inputs:shadow:distance = -1` |
    /// | C++ Type | float |
    /// | Default | -1 (no limit) |
    ///
    /// Matches C++ `UsdLuxShadowAPI::GetShadowDistanceAttr()`.
    #[inline]
    pub fn get_shadow_distance_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(tokens().inputs_shadow_distance.as_str())
    }

    /// Creates the shadow:distance attribute.
    ///
    /// See [`get_shadow_distance_attr`](Self::get_shadow_distance_attr) for attribute details.
    ///
    /// Matches C++ `UsdLuxShadowAPI::CreateShadowDistanceAttr()`.
    pub fn create_shadow_distance_attr(&self) -> Attribute {
        self.get_shadow_distance_attr()
            .unwrap_or_else(Attribute::invalid)
    }

    // =========================================================================
    // SHADOW:FALLOFF Attribute
    // =========================================================================

    /// Returns the shadow:falloff attribute.
    ///
    /// The size of the shadow falloff zone within the shadow max distance.
    /// Used to hide the hard cut-off for shadows stretching past max distance.
    /// The falloff zone fades from full shadow to no shadow.
    /// A value <= 0 (default -1) indicates no falloff.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float inputs:shadow:falloff = -1` |
    /// | C++ Type | float |
    /// | Default | -1 (no falloff) |
    ///
    /// Matches C++ `UsdLuxShadowAPI::GetShadowFalloffAttr()`.
    #[inline]
    pub fn get_shadow_falloff_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(tokens().inputs_shadow_falloff.as_str())
    }

    /// Creates the shadow:falloff attribute.
    ///
    /// See [`get_shadow_falloff_attr`](Self::get_shadow_falloff_attr) for attribute details.
    ///
    /// Matches C++ `UsdLuxShadowAPI::CreateShadowFalloffAttr()`.
    pub fn create_shadow_falloff_attr(&self) -> Attribute {
        self.get_shadow_falloff_attr()
            .unwrap_or_else(Attribute::invalid)
    }

    // =========================================================================
    // SHADOW:FALLOFFGAMMA Attribute
    // =========================================================================

    /// Returns the shadow:falloffGamma attribute.
    ///
    /// A gamma (exponential) control over shadow strength with linear distance
    /// within the falloff zone. Controls the rate of falloff.
    /// Requires use of shadowDistance and shadowFalloff.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float inputs:shadow:falloffGamma = 1` |
    /// | C++ Type | float |
    /// | Default | 1.0 |
    ///
    /// Matches C++ `UsdLuxShadowAPI::GetShadowFalloffGammaAttr()`.
    #[inline]
    pub fn get_shadow_falloff_gamma_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(tokens().inputs_shadow_falloff_gamma.as_str())
    }

    /// Creates the shadow:falloffGamma attribute.
    ///
    /// See [`get_shadow_falloff_gamma_attr`](Self::get_shadow_falloff_gamma_attr) for attribute details.
    ///
    /// Matches C++ `UsdLuxShadowAPI::CreateShadowFalloffGammaAttr()`.
    pub fn create_shadow_falloff_gamma_attr(&self) -> Attribute {
        self.get_shadow_falloff_gamma_attr()
            .unwrap_or_else(Attribute::invalid)
    }

    // =========================================================================
    // ConnectableAPI Conversion
    // =========================================================================

    /// Constructs a ShadowAPI from a ConnectableAPI.
    ///
    /// Allows implicit conversion from UsdShadeConnectableAPI.
    ///
    /// Matches C++ `UsdLuxShadowAPI(const UsdShadeConnectableAPI &connectable)`.
    #[inline]
    pub fn from_connectable(connectable: &ConnectableAPI) -> Self {
        Self::new(connectable.get_prim().clone())
    }

    /// Returns a UsdShadeConnectableAPI for this shadow API.
    ///
    /// Note that a valid UsdLuxShadowAPI will only return a valid
    /// UsdShadeConnectableAPI if its prim's Typed schema type is actually
    /// connectable.
    ///
    /// Matches C++ `UsdLuxShadowAPI::ConnectableAPI()`.
    #[inline]
    pub fn connectable_api(&self) -> ConnectableAPI {
        ConnectableAPI::new(self.prim.clone())
    }

    // =========================================================================
    // Outputs API
    // =========================================================================

    /// Creates an output which can either have a value or be connected.
    ///
    /// The attribute representing the output is created in the "outputs:"
    /// namespace. Outputs on a shadow API cannot be connected, as their
    /// value is assumed to be computed externally.
    ///
    /// # Arguments
    /// * `name` - Name of the output (without "outputs:" prefix)
    /// * `type_name` - Value type name for the output
    ///
    /// Matches C++ `UsdLuxShadowAPI::CreateOutput()`.
    pub fn create_output(&self, name: &Token, type_name: &ValueTypeName) -> Option<Output> {
        Output::new(&self.prim, name, type_name)
    }

    /// Returns the requested output if it exists.
    ///
    /// # Arguments
    /// * `name` - Name of the output (without "outputs:" prefix)
    ///
    /// Matches C++ `UsdLuxShadowAPI::GetOutput()`.
    pub fn get_output(&self, name: &Token) -> Option<Output> {
        // Try to get existing output attribute
        let attr_name = format!("outputs:{}", name.as_str());
        if self.prim.get_attribute(&attr_name).is_some() {
            Output::new(&self.prim, name, &ValueTypeName::invalid())
        } else {
            None
        }
    }

    /// Returns all outputs on this shadow API.
    ///
    /// Outputs are represented by attributes in the "outputs:" namespace.
    ///
    /// # Arguments
    /// * `only_authored` - If true, return only authored attributes
    ///
    /// Matches C++ `UsdLuxShadowAPI::GetOutputs()`.
    pub fn get_outputs(&self, only_authored: bool) -> Vec<Output> {
        self.connectable_api().get_outputs(only_authored)
    }

    // =========================================================================
    // Inputs API
    // =========================================================================

    /// Creates an input which can either have a value or be connected.
    ///
    /// The attribute representing the input is created in the "inputs:"
    /// namespace. Inputs on shadow API are connectable.
    ///
    /// # Arguments
    /// * `name` - Name of the input (without "inputs:" prefix)
    /// * `type_name` - Value type name for the input
    ///
    /// Matches C++ `UsdLuxShadowAPI::CreateInput()`.
    pub fn create_input(&self, name: &Token, type_name: &ValueTypeName) -> Option<Input> {
        Input::new(&self.prim, name, type_name)
    }

    /// Returns the requested input if it exists.
    ///
    /// # Arguments
    /// * `name` - Name of the input (without "inputs:" prefix)
    ///
    /// Matches C++ `UsdLuxShadowAPI::GetInput()`.
    pub fn get_input(&self, name: &Token) -> Option<Input> {
        // Try to get existing input attribute
        let attr_name = format!("inputs:{}", name.as_str());
        if self.prim.get_attribute(&attr_name).is_some() {
            Input::new(&self.prim, name, &ValueTypeName::invalid())
        } else {
            None
        }
    }

    /// Returns all inputs on this shadow API.
    ///
    /// Inputs are represented by attributes in the "inputs:" namespace.
    ///
    /// # Arguments
    /// * `only_authored` - If true, return only authored attributes
    ///
    /// Matches C++ `UsdLuxShadowAPI::GetInputs()`.
    pub fn get_inputs(&self, only_authored: bool) -> Vec<Input> {
        self.connectable_api().get_inputs(only_authored)
    }
}

impl Default for ShadowAPI {
    fn default() -> Self {
        Self::invalid()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_shadow_api() {
        let api = ShadowAPI::default();
        assert!(!api.is_valid());
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = ShadowAPI::get_schema_attribute_names(true);
        assert_eq!(names.len(), 5);
    }
}
