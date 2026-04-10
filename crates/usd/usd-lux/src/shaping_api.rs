//! UsdLuxShapingAPI - API for controlling light emission shaping.
//!
//! This module provides [`ShapingAPI`], a single-apply API schema that
//! controls how light emission is shaped (cone angle, focus, IES profiles).
//!
//! # Overview
//!
//! ShapingAPI provides controls for directing and shaping light emission.
//! It supports both analytical cone shaping and IES (Illumination Engineering
//! Society) photometric profiles for realistic light distribution.
//!
//! # Cone Shaping
//!
//! The cone controls restrict light emission to a cone centered on the
//! light's -Z axis:
//! - `shaping:cone:angle` - Half-angle of the cone in degrees
//! - `shaping:cone:softness` - Softness of the cone edge (0 = hard, 1 = fully soft)
//!
//! # Focus Control
//!
//! Focus pulls light toward the center of the emission:
//! - `shaping:focus` - Higher values = narrower spread
//! - `shaping:focusTint` - Off-axis color tint in falloff region
//!
//! # IES Profiles
//!
//! IES photometric profiles describe real-world light distribution:
//! - `shaping:ies:file` - Path to .ies profile file
//! - `shaping:ies:angleScale` - Rescales the angular distribution
//! - `shaping:ies:normalize` - Normalize to preserve total power
//!
//! # Schema Type
//!
//! This is a **single-apply API schema** (`UsdSchemaKind::SingleApplyAPI`).
//! Apply it using [`apply`](Self::apply) to add shaping controls to a light prim.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdLux/shapingAPI.h`

use super::tokens::tokens;
use crate::schema_create_attr::create_lux_schema_attr;
use usd_core::attribute::Variability;
use usd_core::{Attribute, Prim, Stage};
use usd_sdf::Path;
use usd_sdf::ValueTypeName;
use usd_shade::{ConnectableAPI, Input, Output};
use usd_tf::Token;
use usd_vt::Value;

/// API schema for shaping light emission (cone angle, focus, IES profiles).
///
/// Provides controls for directing and shaping how light is emitted.
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
/// | `inputs:shaping:focus` | float | 0 |
/// | `inputs:shaping:focusTint` | color3f | (0,0,0) |
/// | `inputs:shaping:cone:angle` | float | 90 |
/// | `inputs:shaping:cone:softness` | float | 0 |
/// | `inputs:shaping:ies:file` | asset | |
/// | `inputs:shaping:ies:angleScale` | float | 0 |
/// | `inputs:shaping:ies:normalize` | bool | false |
///
/// Matches C++ `UsdLuxShapingAPI`.
#[derive(Clone)]
pub struct ShapingAPI {
    prim: Prim,
}

impl ShapingAPI {
    // =========================================================================
    // Construction
    // =========================================================================

    /// Constructs a ShapingAPI on the given prim.
    ///
    /// # Arguments
    /// * `prim` - The prim to wrap with this API schema
    #[inline]
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Returns a ShapingAPI holding the prim at `path` on `stage`.
    ///
    /// If no prim exists at the path, returns an invalid schema object.
    ///
    /// Matches C++ `UsdLuxShapingAPI::Get(stage, path)`.
    pub fn get(stage: &Stage, path: &Path) -> Self {
        match stage.get_prim_at_path(path) {
            Some(prim) => Self::new(prim),
            None => Self::invalid(),
        }
    }

    /// Creates an invalid ShapingAPI.
    #[inline]
    pub fn invalid() -> Self {
        Self {
            prim: Prim::invalid(),
        }
    }

    /// Returns true if this API schema can be applied to the given prim.
    ///
    /// Matches C++ `UsdLuxShapingAPI::CanApply(prim)`.
    pub fn can_apply(prim: &Prim) -> bool {
        prim.is_valid()
    }

    /// Applies ShapingAPI to the given prim.
    ///
    /// This adds "ShapingAPI" to the prim's `apiSchemas` metadata.
    ///
    /// # Returns
    /// A valid ShapingAPI on success, or `None` if the prim is invalid.
    ///
    /// Matches C++ `UsdLuxShapingAPI::Apply(prim)`.
    pub fn apply(prim: &Prim) -> Option<Self> {
        if !prim.is_valid() {
            return None;
        }
        prim.apply_api(&tokens().shaping_api);
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
    /// Matches C++ `UsdLuxShapingAPI::GetSchemaAttributeNames()`.
    pub fn get_schema_attribute_names(_include_inherited: bool) -> Vec<Token> {
        vec![
            tokens().inputs_shaping_focus.clone(),
            tokens().inputs_shaping_focus_tint.clone(),
            tokens().inputs_shaping_cone_angle.clone(),
            tokens().inputs_shaping_cone_softness.clone(),
            tokens().inputs_shaping_ies_file.clone(),
            tokens().inputs_shaping_ies_angle_scale.clone(),
            tokens().inputs_shaping_ies_normalize.clone(),
        ]
    }

    // =========================================================================
    // SHAPING:FOCUS Attribute
    // =========================================================================

    /// Returns the shaping:focus attribute.
    ///
    /// A control to shape the spread of light. Higher focus values pull
    /// light towards the center and narrow the spread.
    ///
    /// Implemented as multiplication with |emissionDirection · lightNormal|^focus.
    /// Values < 0 are ignored.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float inputs:shaping:focus = 0` |
    /// | C++ Type | float |
    /// | Default | 0 |
    ///
    /// Matches C++ `UsdLuxShapingAPI::GetShapingFocusAttr()`.
    #[inline]
    pub fn get_shaping_focus_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(tokens().inputs_shaping_focus.as_str())
    }

    /// Creates the shaping:focus attribute.
    ///
    /// See [`get_shaping_focus_attr`](Self::get_shaping_focus_attr) for attribute details.
    ///
    /// Matches C++ `UsdLuxShapingAPI::CreateShapingFocusAttr(VtValue const &defaultValue, bool writeSparsely)`.
    pub fn create_shaping_focus_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        create_lux_schema_attr(
            &self.prim,
            tokens().inputs_shaping_focus.as_str(),
            "float",
            Variability::Varying,
            default_value,
            write_sparsely,
        )
    }

    // =========================================================================
    // SHAPING:FOCUSTINT Attribute
    // =========================================================================

    /// Returns the shaping:focusTint attribute.
    ///
    /// Off-axis color tint. Tints the emission in the falloff region.
    /// The default tint is black. A focusTint of pure white disables focus.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `color3f inputs:shaping:focusTint = (0, 0, 0)` |
    /// | C++ Type | GfVec3f |
    /// | Default | (0, 0, 0) |
    ///
    /// Matches C++ `UsdLuxShapingAPI::GetShapingFocusTintAttr()`.
    #[inline]
    pub fn get_shaping_focus_tint_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(tokens().inputs_shaping_focus_tint.as_str())
    }

    /// Creates the shaping:focusTint attribute.
    ///
    /// See [`get_shaping_focus_tint_attr`](Self::get_shaping_focus_tint_attr) for attribute details.
    ///
    /// Matches C++ `UsdLuxShapingAPI::CreateShapingFocusTintAttr(VtValue const &defaultValue, bool writeSparsely)`.
    pub fn create_shaping_focus_tint_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        create_lux_schema_attr(
            &self.prim,
            tokens().inputs_shaping_focus_tint.as_str(),
            "color3f",
            Variability::Varying,
            default_value,
            write_sparsely,
        )
    }

    // =========================================================================
    // SHAPING:CONE:ANGLE Attribute
    // =========================================================================

    /// Returns the shaping:cone:angle attribute.
    ///
    /// Angular limit off the primary axis to restrict light spread, in degrees.
    /// Light emissions at angles greater than this are guaranteed to be zero.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float inputs:shaping:cone:angle = 90` |
    /// | C++ Type | float |
    /// | Default | 90 |
    ///
    /// Matches C++ `UsdLuxShapingAPI::GetShapingConeAngleAttr()`.
    #[inline]
    pub fn get_shaping_cone_angle_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(tokens().inputs_shaping_cone_angle.as_str())
    }

    /// Creates the shaping:cone:angle attribute.
    ///
    /// See [`get_shaping_cone_angle_attr`](Self::get_shaping_cone_angle_attr) for attribute details.
    ///
    /// Matches C++ `UsdLuxShapingAPI::CreateShapingConeAngleAttr(VtValue const &defaultValue, bool writeSparsely)`.
    pub fn create_shaping_cone_angle_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        create_lux_schema_attr(
            &self.prim,
            tokens().inputs_shaping_cone_angle.as_str(),
            "float",
            Variability::Varying,
            default_value,
            write_sparsely,
        )
    }

    // =========================================================================
    // SHAPING:CONE:SOFTNESS Attribute
    // =========================================================================

    /// Returns the shaping:cone:softness attribute.
    ///
    /// Controls the cutoff softness for cone angle. At softness = 0 (default),
    /// the cone angle functions as a hard binary cutoff. For softness in (0, 1],
    /// it defines the proportion of angles over which luminance is interpolated.
    ///
    /// Values outside [0, 1] are clamped.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float inputs:shaping:cone:softness = 0` |
    /// | C++ Type | float |
    /// | Default | 0 |
    ///
    /// Matches C++ `UsdLuxShapingAPI::GetShapingConeSoftnessAttr()`.
    #[inline]
    pub fn get_shaping_cone_softness_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(tokens().inputs_shaping_cone_softness.as_str())
    }

    /// Creates the shaping:cone:softness attribute.
    ///
    /// See [`get_shaping_cone_softness_attr`](Self::get_shaping_cone_softness_attr) for attribute details.
    ///
    /// Matches C++ `UsdLuxShapingAPI::CreateShapingConeSoftnessAttr(VtValue const &defaultValue, bool writeSparsely)`.
    pub fn create_shaping_cone_softness_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        create_lux_schema_attr(
            &self.prim,
            tokens().inputs_shaping_cone_softness.as_str(),
            "float",
            Variability::Varying,
            default_value,
            write_sparsely,
        )
    }

    // =========================================================================
    // SHAPING:IES:FILE Attribute
    // =========================================================================

    /// Returns the shaping:ies:file attribute.
    ///
    /// An IES (Illumination Engineering Society) light profile describing
    /// the angular distribution of light. The profile values scale luminance.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `asset inputs:shaping:ies:file` |
    /// | C++ Type | SdfAssetPath |
    ///
    /// Matches C++ `UsdLuxShapingAPI::GetShapingIesFileAttr()`.
    #[inline]
    pub fn get_shaping_ies_file_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(tokens().inputs_shaping_ies_file.as_str())
    }

    /// Creates the shaping:ies:file attribute.
    ///
    /// See [`get_shaping_ies_file_attr`](Self::get_shaping_ies_file_attr) for attribute details.
    ///
    /// Matches C++ `UsdLuxShapingAPI::CreateShapingIesFileAttr(VtValue const &defaultValue, bool writeSparsely)`.
    pub fn create_shaping_ies_file_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        create_lux_schema_attr(
            &self.prim,
            tokens().inputs_shaping_ies_file.as_str(),
            "asset",
            Variability::Varying,
            default_value,
            write_sparsely,
        )
    }

    // =========================================================================
    // SHAPING:IES:ANGLESCALE Attribute
    // =========================================================================

    /// Returns the shaping:ies:angleScale attribute.
    ///
    /// Rescales the angular distribution of the IES profile. Applies a scaling
    /// factor to the theta coordinate before sampling:
    /// - Positive values: scale origin at theta = 0 (top)
    /// - Negative values: scale origin at theta = pi (bottom)
    /// - Zero: no scaling (passthrough)
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float inputs:shaping:ies:angleScale = 0` |
    /// | C++ Type | float |
    /// | Default | 0 |
    ///
    /// Matches C++ `UsdLuxShapingAPI::GetShapingIesAngleScaleAttr()`.
    #[inline]
    pub fn get_shaping_ies_angle_scale_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(tokens().inputs_shaping_ies_angle_scale.as_str())
    }

    /// Creates the shaping:ies:angleScale attribute.
    ///
    /// See [`get_shaping_ies_angle_scale_attr`](Self::get_shaping_ies_angle_scale_attr) for attribute details.
    ///
    /// Matches C++ `UsdLuxShapingAPI::CreateShapingIesAngleScaleAttr(VtValue const &defaultValue, bool writeSparsely)`.
    pub fn create_shaping_ies_angle_scale_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        create_lux_schema_attr(
            &self.prim,
            tokens().inputs_shaping_ies_angle_scale.as_str(),
            "float",
            Variability::Varying,
            default_value,
            write_sparsely,
        )
    }

    // =========================================================================
    // SHAPING:IES:NORMALIZE Attribute
    // =========================================================================

    /// Returns the shaping:ies:normalize attribute.
    ///
    /// Normalizes the IES profile so it affects shaping while preserving
    /// overall energy output. The sampled intensity is scaled by the
    /// total power of the IES profile.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `bool inputs:shaping:ies:normalize = 0` |
    /// | C++ Type | bool |
    /// | Default | false |
    ///
    /// Matches C++ `UsdLuxShapingAPI::GetShapingIesNormalizeAttr()`.
    #[inline]
    pub fn get_shaping_ies_normalize_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(tokens().inputs_shaping_ies_normalize.as_str())
    }

    /// Creates the shaping:ies:normalize attribute.
    ///
    /// See [`get_shaping_ies_normalize_attr`](Self::get_shaping_ies_normalize_attr) for attribute details.
    ///
    /// Matches C++ `UsdLuxShapingAPI::CreateShapingIesNormalizeAttr(VtValue const &defaultValue, bool writeSparsely)`.
    pub fn create_shaping_ies_normalize_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        create_lux_schema_attr(
            &self.prim,
            tokens().inputs_shaping_ies_normalize.as_str(),
            "bool",
            Variability::Varying,
            default_value,
            write_sparsely,
        )
    }

    // =========================================================================
    // ConnectableAPI Conversion
    // =========================================================================

    /// Constructs a ShapingAPI from a ConnectableAPI.
    ///
    /// Allows implicit conversion from UsdShadeConnectableAPI.
    ///
    /// Matches C++ `UsdLuxShapingAPI(const UsdShadeConnectableAPI &connectable)`.
    #[inline]
    pub fn from_connectable(connectable: &ConnectableAPI) -> Self {
        Self::new(connectable.get_prim().clone())
    }

    /// Returns a UsdShadeConnectableAPI for this shaping API.
    ///
    /// Note that a valid UsdLuxShapingAPI will only return a valid
    /// UsdShadeConnectableAPI if its prim's Typed schema type is actually
    /// connectable.
    ///
    /// Matches C++ `UsdLuxShapingAPI::ConnectableAPI()`.
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
    /// namespace. Outputs on a shaping API cannot be connected, as their
    /// value is assumed to be computed externally.
    ///
    /// # Arguments
    /// * `name` - Name of the output (without "outputs:" prefix)
    /// * `type_name` - Value type name for the output
    ///
    /// Matches C++ `UsdLuxShapingAPI::CreateOutput()`.
    pub fn create_output(&self, name: &Token, type_name: &ValueTypeName) -> Option<Output> {
        Output::new(&self.prim, name, type_name)
    }

    /// Returns the requested output if it exists.
    ///
    /// # Arguments
    /// * `name` - Name of the output (without "outputs:" prefix)
    ///
    /// Matches C++ `UsdLuxShapingAPI::GetOutput()`.
    pub fn get_output(&self, name: &Token) -> Option<Output> {
        // Try to get existing output attribute
        let attr_name = format!("outputs:{}", name.as_str());
        if self.prim.get_attribute(&attr_name).is_some() {
            Output::new(&self.prim, name, &ValueTypeName::invalid())
        } else {
            None
        }
    }

    /// Returns all outputs on this shaping API.
    ///
    /// Outputs are represented by attributes in the "outputs:" namespace.
    ///
    /// # Arguments
    /// * `only_authored` - If true, return only authored attributes
    ///
    /// Matches C++ `UsdLuxShapingAPI::GetOutputs()`.
    pub fn get_outputs(&self, only_authored: bool) -> Vec<Output> {
        self.connectable_api().get_outputs(only_authored)
    }

    // =========================================================================
    // Inputs API
    // =========================================================================

    /// Creates an input which can either have a value or be connected.
    ///
    /// The attribute representing the input is created in the "inputs:"
    /// namespace. Inputs on shaping API are connectable.
    ///
    /// # Arguments
    /// * `name` - Name of the input (without "inputs:" prefix)
    /// * `type_name` - Value type name for the input
    ///
    /// Matches C++ `UsdLuxShapingAPI::CreateInput()`.
    pub fn create_input(&self, name: &Token, type_name: &ValueTypeName) -> Option<Input> {
        Input::new(&self.prim, name, type_name)
    }

    /// Returns the requested input if it exists.
    ///
    /// # Arguments
    /// * `name` - Name of the input (without "inputs:" prefix)
    ///
    /// Matches C++ `UsdLuxShapingAPI::GetInput()`.
    pub fn get_input(&self, name: &Token) -> Option<Input> {
        // Try to get existing input attribute
        let attr_name = format!("inputs:{}", name.as_str());
        if self.prim.get_attribute(&attr_name).is_some() {
            Input::new(&self.prim, name, &ValueTypeName::invalid())
        } else {
            None
        }
    }

    /// Returns all inputs on this shaping API.
    ///
    /// Inputs are represented by attributes in the "inputs:" namespace.
    ///
    /// # Arguments
    /// * `only_authored` - If true, return only authored attributes
    ///
    /// Matches C++ `UsdLuxShapingAPI::GetInputs()`.
    pub fn get_inputs(&self, only_authored: bool) -> Vec<Input> {
        self.connectable_api().get_inputs(only_authored)
    }
}

impl Default for ShapingAPI {
    fn default() -> Self {
        Self::invalid()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_core::{InitialLoadSet, Stage};
    use usd_sdf::TimeCode;

    #[test]
    fn test_invalid_shaping_api() {
        let api = ShapingAPI::default();
        assert!(!api.is_valid());
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = ShapingAPI::get_schema_attribute_names(true);
        assert_eq!(names.len(), 7);
    }

    #[test]
    fn create_shaping_cone_angle_attr_sets_optional_default() {
        let _ = usd_sdf::init();
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("stage");
        let prim = stage.define_prim("/L", "").expect("prim");
        let api = ShapingAPI::apply(&prim).expect("apply");
        let attr = api.create_shaping_cone_angle_attr(Some(Value::from_f32(60.0)), false);
        assert!(attr.is_valid());
        assert_eq!(attr.get_typed::<f32>(TimeCode::default()), Some(60.0));
    }
}
