//! UsdLuxLightAPI - API schema for lights.
//!
//! This module provides [`LightAPI`], the single-apply API schema that imparts
//! the quality of being a light onto any prim.
//!
//! # Overview
//! A light is any prim that has this schema applied to it. This is true
//! regardless of whether LightAPI is included as a built-in API of the prim
//! type (e.g. RectLight, DistantLight) or is applied directly to a Gprim.
//!
//! # Quantities and Units
//! The emission of a default light with `intensity` 1 and `color` [1,1,1] is
//! normalized such that a ray normally incident upon the sensor with EV0
//! exposure settings will generate a pixel value of [1,1,1].
//!
//! The luminance of the default light is 1 nit (cd/m²).
//!
//! # Linking
//! Lights can be linked to geometry via collections:
//! - `GetLightLinkCollection()` - controls which geometry the light illuminates
//! - `GetShadowLinkCollection()` - controls which geometry casts shadows
//!
//! Both collections have `includeRoot=true` by default.
//!
//! # C++ Reference
//! Port of `pxr/usd/usdLux/lightAPI.h`

use super::tokens::tokens;
use crate::schema_create_attr::create_lux_schema_attr;
use usd_core::attribute::Variability;
use usd_core::collection_api::CollectionAPI;
use usd_core::{Attribute, Prim, Relationship, Stage};
use usd_gf::Vec3f;

use usd_sdf::Path;
use usd_sdf::TimeCode;
use usd_sdf::ValueTypeName;
use usd_shade::{ConnectableAPI, Input, Output};
use usd_tf::Token;
use usd_vt::Value;

/// API schema that imparts the quality of being a light onto a prim.
///
/// A light is any prim that has this schema applied to it. This is true
/// regardless of whether LightAPI is a built-in API or applied directly.
///
/// # Schema Type
/// This is a **single-apply API schema** (`UsdSchemaKind::SingleApplyAPI`).
/// Use [`apply`](Self::apply) to add this schema to a prim.
///
/// # Core Attributes
/// - `light:shaderId` - Shader identifier for the light
/// - `light:materialSyncMode` - How to sync with bound materials
/// - `inputs:intensity` - Linear brightness scale (default: 1.0)
/// - `inputs:exposure` - Exponential brightness as power of 2 (default: 0.0)
/// - `inputs:color` - Light color in linear RGB (default: white)
/// - `inputs:diffuse` - Diffuse contribution multiplier (default: 1.0)
/// - `inputs:specular` - Specular contribution multiplier (default: 1.0)
/// - `inputs:normalize` - Normalize by surface area (default: false)
/// - `inputs:enableColorTemperature` - Use color temp (default: false)
/// - `inputs:colorTemperature` - Color temp in Kelvin (default: 6500)
///
/// Matches C++ `UsdLuxLightAPI`.
#[derive(Clone)]
pub struct LightAPI {
    prim: Prim,
}

impl LightAPI {
    // =========================================================================
    // Construction
    // =========================================================================

    /// Constructs a LightAPI schema on the given prim.
    ///
    /// # Arguments
    /// * `prim` - The prim to wrap with this schema
    #[inline]
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Returns a LightAPI holding the prim at `path` on `stage`.
    ///
    /// Matches C++ `UsdLuxLightAPI::Get(stage, path)`.
    pub fn get(stage: &Stage, path: &Path) -> Self {
        match stage.get_prim_at_path(path) {
            Some(prim) => Self::new(prim),
            None => Self::invalid(),
        }
    }

    /// Creates an invalid LightAPI schema object.
    #[inline]
    pub fn invalid() -> Self {
        Self {
            prim: Prim::invalid(),
        }
    }

    /// Applies LightAPI to the given prim.
    ///
    /// This adds "LightAPI" to the prim's `apiSchemas` metadata.
    ///
    /// # Arguments
    /// * `prim` - The prim to apply the schema to
    ///
    /// # Returns
    /// A valid LightAPI on success, or `None` if the prim is invalid.
    ///
    /// Matches C++ `UsdLuxLightAPI::Apply(prim)`.
    pub fn apply(prim: &Prim) -> Option<Self> {
        if !prim.is_valid() {
            return None;
        }
        prim.apply_api(&tokens().light_api);
        Some(Self::new(prim.clone()))
    }

    /// Returns true if this schema can be applied to the given prim.
    ///
    /// Matches C++ `UsdLuxLightAPI::CanApply(prim)`.
    pub fn can_apply(prim: &Prim) -> bool {
        prim.is_valid()
    }

    // =========================================================================
    // Schema Information
    // =========================================================================

    /// Returns true if this API schema object is valid.
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
    /// Matches C++ `UsdLuxLightAPI::GetSchemaAttributeNames()`.
    pub fn get_schema_attribute_names(_include_inherited: bool) -> Vec<Token> {
        vec![
            tokens().light_shader_id.clone(),
            tokens().light_material_sync_mode.clone(),
            tokens().inputs_intensity.clone(),
            tokens().inputs_exposure.clone(),
            tokens().inputs_diffuse.clone(),
            tokens().inputs_specular.clone(),
            tokens().inputs_normalize.clone(),
            tokens().inputs_color.clone(),
            tokens().inputs_enable_color_temperature.clone(),
            tokens().inputs_color_temperature.clone(),
        ]
    }

    // =========================================================================
    // SHADERID Attribute
    // =========================================================================

    /// Returns the shader ID attribute.
    ///
    /// Default ID for the light's shader. This defines the shader ID when
    /// a render-context-specific shader ID is not available.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `uniform token light:shaderId = ""` |
    /// | Variability | Uniform |
    #[inline]
    pub fn get_shader_id_attr(&self) -> Option<Attribute> {
        self.prim.get_attribute(tokens().light_shader_id.as_str())
    }

    /// Creates the shader ID attribute.
    ///
    /// Matches C++ `UsdLuxLightAPI::CreateShaderIdAttr(VtValue const &defaultValue, bool writeSparsely)`.
    pub fn create_shader_id_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        let t = tokens();
        create_lux_schema_attr(
            &self.prim,
            t.light_shader_id.as_str(),
            "token",
            Variability::Uniform,
            default_value,
            write_sparsely,
        )
    }

    // =========================================================================
    // MATERIALSYNCMODE Attribute
    // =========================================================================

    /// Returns the material sync mode attribute.
    ///
    /// For LightAPI applied to geometry with a bound Material, specifies
    /// the relationship between Material response and lighting response.
    ///
    /// Valid values:
    /// - `materialGlowTintsLight` - Material glow tints the light color
    /// - `independent` - Material and light are independent
    /// - `noMaterialResponse` - No material response (standard for UsdLux lights)
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `uniform token light:materialSyncMode = "noMaterialResponse"` |
    /// | Default | `noMaterialResponse` |
    #[inline]
    pub fn get_material_sync_mode_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(tokens().light_material_sync_mode.as_str())
    }

    /// Creates the material sync mode attribute.
    ///
    /// Matches C++ `UsdLuxLightAPI::CreateMaterialSyncModeAttr(VtValue const &defaultValue, bool writeSparsely)`.
    pub fn create_material_sync_mode_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        let t = tokens();
        create_lux_schema_attr(
            &self.prim,
            t.light_material_sync_mode.as_str(),
            "token",
            Variability::Uniform,
            default_value,
            write_sparsely,
        )
    }

    // =========================================================================
    // INTENSITY Attribute
    // =========================================================================

    /// Returns the intensity attribute.
    ///
    /// Scales the brightness of the light linearly. Expresses the unmultiplied
    /// luminance (L) of the light in nits (cd/m²).
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float inputs:intensity = 1` |
    /// | Default | 1.0 |
    #[inline]
    pub fn get_intensity_attr(&self) -> Option<Attribute> {
        self.prim.get_attribute(tokens().inputs_intensity.as_str())
    }

    /// Creates the intensity attribute.
    ///
    /// Matches C++ `UsdLuxLightAPI::CreateIntensityAttr(VtValue const &defaultValue, bool writeSparsely)`.
    pub fn create_intensity_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        let t = tokens();
        create_lux_schema_attr(
            &self.prim,
            t.inputs_intensity.as_str(),
            "float",
            Variability::Varying,
            default_value,
            write_sparsely,
        )
    }

    // =========================================================================
    // EXPOSURE Attribute
    // =========================================================================

    /// Returns the exposure attribute.
    ///
    /// Scales the brightness exponentially as a power of 2 (like F-stop).
    /// Result is multiplied against intensity: `L = L * 2^exposure`
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float inputs:exposure = 0` |
    /// | Default | 0.0 |
    #[inline]
    pub fn get_exposure_attr(&self) -> Option<Attribute> {
        self.prim.get_attribute(tokens().inputs_exposure.as_str())
    }

    /// Creates the exposure attribute.
    ///
    /// Matches C++ `UsdLuxLightAPI::CreateExposureAttr(VtValue const &defaultValue, bool writeSparsely)`.
    pub fn create_exposure_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        let t = tokens();
        create_lux_schema_attr(
            &self.prim,
            t.inputs_exposure.as_str(),
            "float",
            Variability::Varying,
            default_value,
            write_sparsely,
        )
    }

    // =========================================================================
    // DIFFUSE Attribute
    // =========================================================================

    /// Returns the diffuse attribute.
    ///
    /// Multiplier for the effect on diffuse shading.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float inputs:diffuse = 1` |
    /// | Default | 1.0 |
    #[inline]
    pub fn get_diffuse_attr(&self) -> Option<Attribute> {
        self.prim.get_attribute(tokens().inputs_diffuse.as_str())
    }

    /// Creates the diffuse attribute.
    ///
    /// Matches C++ `UsdLuxLightAPI::CreateDiffuseAttr(VtValue const &defaultValue, bool writeSparsely)`.
    pub fn create_diffuse_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        let t = tokens();
        create_lux_schema_attr(
            &self.prim,
            t.inputs_diffuse.as_str(),
            "float",
            Variability::Varying,
            default_value,
            write_sparsely,
        )
    }

    // =========================================================================
    // SPECULAR Attribute
    // =========================================================================

    /// Returns the specular attribute.
    ///
    /// Multiplier for the effect on specular shading.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float inputs:specular = 1` |
    /// | Default | 1.0 |
    #[inline]
    pub fn get_specular_attr(&self) -> Option<Attribute> {
        self.prim.get_attribute(tokens().inputs_specular.as_str())
    }

    /// Creates the specular attribute.
    ///
    /// Matches C++ `UsdLuxLightAPI::CreateSpecularAttr(VtValue const &defaultValue, bool writeSparsely)`.
    pub fn create_specular_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        let t = tokens();
        create_lux_schema_attr(
            &self.prim,
            t.inputs_specular.as_str(),
            "float",
            Variability::Varying,
            default_value,
            write_sparsely,
        )
    }

    // =========================================================================
    // COLOR Attribute
    // =========================================================================

    /// Returns the color attribute.
    ///
    /// The color of emitted light, in energy-linear terms.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `color3f inputs:color = (1, 1, 1)` |
    /// | Default | (1, 1, 1) |
    #[inline]
    pub fn get_color_attr(&self) -> Option<Attribute> {
        self.prim.get_attribute(tokens().inputs_color.as_str())
    }

    /// Creates the color attribute.
    ///
    /// Matches C++ `UsdLuxLightAPI::CreateColorAttr(VtValue const &defaultValue, bool writeSparsely)`.
    pub fn create_color_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        let t = tokens();
        create_lux_schema_attr(
            &self.prim,
            t.inputs_color.as_str(),
            "color3f",
            Variability::Varying,
            default_value,
            write_sparsely,
        )
    }

    // =========================================================================
    // ENABLECOLORTEMPERATURE Attribute
    // =========================================================================

    /// Returns the enable color temperature attribute.
    ///
    /// If true, computes light color from `colorTemperature` instead of `color`.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `bool inputs:enableColorTemperature = 0` |
    /// | Default | false |
    #[inline]
    pub fn get_enable_color_temperature_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(tokens().inputs_enable_color_temperature.as_str())
    }

    /// Creates the enable color temperature attribute.
    ///
    /// Matches C++ `UsdLuxLightAPI::CreateEnableColorTemperatureAttr(VtValue const &defaultValue, bool writeSparsely)`.
    pub fn create_enable_color_temperature_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        let t = tokens();
        create_lux_schema_attr(
            &self.prim,
            t.inputs_enable_color_temperature.as_str(),
            "bool",
            Variability::Varying,
            default_value,
            write_sparsely,
        )
    }

    // =========================================================================
    // COLORTEMPERATURE Attribute
    // =========================================================================

    /// Returns the color temperature attribute.
    ///
    /// Color temperature in Kelvin, ranging from 1000 (warm) to 10000 (cool).
    /// Only effective when `enableColorTemperature` is true.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `float inputs:colorTemperature = 6500` |
    /// | Default | 6500 |
    #[inline]
    pub fn get_color_temperature_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(tokens().inputs_color_temperature.as_str())
    }

    /// Creates the color temperature attribute.
    ///
    /// Matches C++ `UsdLuxLightAPI::CreateColorTemperatureAttr(VtValue const &defaultValue, bool writeSparsely)`.
    pub fn create_color_temperature_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        let t = tokens();
        create_lux_schema_attr(
            &self.prim,
            t.inputs_color_temperature.as_str(),
            "float",
            Variability::Varying,
            default_value,
            write_sparsely,
        )
    }

    // =========================================================================
    // NORMALIZE Attribute
    // =========================================================================

    /// Returns the normalize attribute.
    ///
    /// If true, normalizes power by the surface area of the light.
    /// This makes brightness independent of light size.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `bool inputs:normalize = 0` |
    /// | Default | false |
    #[inline]
    pub fn get_normalize_attr(&self) -> Option<Attribute> {
        self.prim.get_attribute(tokens().inputs_normalize.as_str())
    }

    /// Creates the normalize attribute.
    ///
    /// Matches C++ `UsdLuxLightAPI::CreateNormalizeAttr(VtValue const &defaultValue, bool writeSparsely)`.
    pub fn create_normalize_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        let t = tokens();
        create_lux_schema_attr(
            &self.prim,
            t.inputs_normalize.as_str(),
            "bool",
            Variability::Varying,
            default_value,
            write_sparsely,
        )
    }

    // =========================================================================
    // Convenience Value Getters
    // =========================================================================
    // These match the pattern of returning the value directly with a default,
    // analogous to the C++ generated schema accessors.

    /// Returns the intensity value at the given time, or the schema default (1.0).
    ///
    /// Matches C++ `UsdLuxLightAPI::GetIntensityAttr().Get(&v, time)`.
    #[inline]
    pub fn get_intensity(&self, time: TimeCode) -> f32 {
        self.get_intensity_attr()
            .and_then(|a| a.get_typed::<f32>(time))
            .unwrap_or(1.0)
    }

    /// Returns the exposure value at the given time, or the schema default (0.0).
    ///
    /// Matches C++ `UsdLuxLightAPI::GetExposureAttr().Get(&v, time)`.
    #[inline]
    pub fn get_exposure(&self, time: TimeCode) -> f32 {
        self.get_exposure_attr()
            .and_then(|a| a.get_typed::<f32>(time))
            .unwrap_or(0.0)
    }

    /// Returns the color value at the given time, or the schema default (1, 1, 1).
    ///
    /// Matches C++ `UsdLuxLightAPI::GetColorAttr().Get(&v, time)`.
    #[inline]
    pub fn get_color(&self, time: TimeCode) -> Vec3f {
        self.get_color_attr()
            .and_then(|a| a.get_typed::<Vec3f>(time))
            .unwrap_or(Vec3f::new(1.0, 1.0, 1.0))
    }

    /// Returns the diffuse multiplier at the given time, or the schema default (1.0).
    ///
    /// Matches C++ `UsdLuxLightAPI::GetDiffuseAttr().Get(&v, time)`.
    #[inline]
    pub fn get_diffuse(&self, time: TimeCode) -> f32 {
        self.get_diffuse_attr()
            .and_then(|a| a.get_typed::<f32>(time))
            .unwrap_or(1.0)
    }

    /// Returns the specular multiplier at the given time, or the schema default (1.0).
    ///
    /// Matches C++ `UsdLuxLightAPI::GetSpecularAttr().Get(&v, time)`.
    #[inline]
    pub fn get_specular(&self, time: TimeCode) -> f32 {
        self.get_specular_attr()
            .and_then(|a| a.get_typed::<f32>(time))
            .unwrap_or(1.0)
    }

    /// Returns whether power normalization is enabled at the given time, or the schema default (false).
    ///
    /// Matches C++ `UsdLuxLightAPI::GetNormalizeAttr().Get(&v, time)`.
    #[inline]
    pub fn get_normalize_power(&self, time: TimeCode) -> bool {
        self.get_normalize_attr()
            .and_then(|a| a.get_typed::<bool>(time))
            .unwrap_or(false)
    }

    /// Returns the color temperature in Kelvin at the given time, or the schema default (6500.0).
    ///
    /// Matches C++ `UsdLuxLightAPI::GetColorTemperatureAttr().Get(&v, time)`.
    #[inline]
    pub fn get_color_temperature(&self, time: TimeCode) -> f32 {
        self.get_color_temperature_attr()
            .and_then(|a| a.get_typed::<f32>(time))
            .unwrap_or(6500.0)
    }

    /// Returns whether color temperature is enabled at the given time, or the schema default (false).
    ///
    /// Matches C++ `UsdLuxLightAPI::GetEnableColorTemperatureAttr().Get(&v, time)`.
    #[inline]
    pub fn get_enable_color_temperature(&self, time: TimeCode) -> bool {
        self.get_enable_color_temperature_attr()
            .and_then(|a| a.get_typed::<bool>(time))
            .unwrap_or(false)
    }

    // =========================================================================
    // FILTERS Relationship
    // =========================================================================

    /// Returns the filters relationship.
    ///
    /// Ordered list of light filters that affect this light.
    #[inline]
    pub fn get_filters_rel(&self) -> Option<Relationship> {
        self.prim.get_relationship(tokens().light_filters.as_str())
    }

    /// Creates the filters relationship.
    pub fn create_filters_rel(&self) -> Option<Relationship> {
        self.get_filters_rel()
    }

    // =========================================================================
    // Linking Collections
    // =========================================================================

    /// Returns the name of the light link collection.
    ///
    /// This collection controls which geometry the light illuminates.
    /// Default has `includeRoot=true`, illuminating everything.
    #[inline]
    pub fn get_light_link_collection_name() -> &'static Token {
        &tokens().light_link
    }

    /// Returns the name of the shadow link collection.
    ///
    /// This collection controls which geometry casts shadows from this light.
    /// Default has `includeRoot=true`, all geometry casts shadows.
    #[inline]
    pub fn get_shadow_link_collection_name() -> &'static Token {
        &tokens().shadow_link
    }

    /// Returns the light link CollectionAPI for this light.
    ///
    /// This collection controls which geometry is illuminated by this light.
    /// Default includes all geometry (`includeRoot=true`).
    ///
    /// Matches C++ `UsdLuxLightAPI::GetLightLinkCollectionAPI()`.
    pub fn get_light_link_collection_api(&self) -> CollectionAPI {
        CollectionAPI::new_with_include_root_fallback(
            self.prim.clone(),
            tokens().light_link.clone(),
            true,
        )
    }

    /// Returns the shadow link CollectionAPI for this light.
    ///
    /// This collection controls which geometry casts shadows from this light.
    /// Default includes all geometry (`includeRoot=true`).
    ///
    /// Matches C++ `UsdLuxLightAPI::GetShadowLinkCollectionAPI()`.
    pub fn get_shadow_link_collection_api(&self) -> CollectionAPI {
        CollectionAPI::new_with_include_root_fallback(
            self.prim.clone(),
            tokens().shadow_link.clone(),
            true,
        )
    }

    // =========================================================================
    // ConnectableAPI Conversion
    // =========================================================================

    /// Constructs a LightAPI from a ConnectableAPI.
    ///
    /// Allows implicit conversion from UsdShadeConnectableAPI.
    ///
    /// Matches C++ `UsdLuxLightAPI(const UsdShadeConnectableAPI &connectable)`.
    #[inline]
    pub fn from_connectable(connectable: &ConnectableAPI) -> Self {
        Self::new(connectable.get_prim().clone())
    }

    /// Returns a UsdShadeConnectableAPI for this light.
    ///
    /// Note that most tasks can be accomplished without explicitly constructing
    /// a ConnectableAPI, since connection-related API such as
    /// `ConnectableAPI::connect_to_source()` are static methods.
    ///
    /// Matches C++ `UsdLuxLightAPI::ConnectableAPI()`.
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
    /// namespace. Outputs on a light cannot be connected, as their value
    /// is assumed to be computed externally.
    ///
    /// # Arguments
    /// * `name` - Name of the output (without "outputs:" prefix)
    /// * `type_name` - Value type name for the output
    ///
    /// Matches C++ `UsdLuxLightAPI::CreateOutput()`.
    pub fn create_output(&self, name: &Token, type_name: &ValueTypeName) -> Option<Output> {
        Output::new(&self.prim, name, type_name)
    }

    /// Returns the requested output if it exists.
    ///
    /// # Arguments
    /// * `name` - Name of the output (without "outputs:" prefix)
    ///
    /// Matches C++ `UsdLuxLightAPI::GetOutput()`.
    pub fn get_output(&self, name: &Token) -> Option<Output> {
        // Try to get existing output attribute
        let attr_name = format!("outputs:{}", name.as_str());
        if self.prim.get_attribute(&attr_name).is_some() {
            // Use a scalar type name for retrieval
            Output::new(&self.prim, name, &ValueTypeName::invalid())
        } else {
            None
        }
    }

    /// Returns all outputs on this light.
    ///
    /// Outputs are represented by attributes in the "outputs:" namespace.
    ///
    /// # Arguments
    /// * `only_authored` - If true, return only authored attributes
    ///
    /// Matches C++ `UsdLuxLightAPI::GetOutputs()`.
    pub fn get_outputs(&self, only_authored: bool) -> Vec<Output> {
        self.connectable_api().get_outputs(only_authored)
    }

    // =========================================================================
    // Inputs API
    // =========================================================================

    /// Creates an input which can either have a value or be connected.
    ///
    /// The attribute representing the input is created in the "inputs:"
    /// namespace. Inputs on lights are connectable.
    ///
    /// # Arguments
    /// * `name` - Name of the input (without "inputs:" prefix)
    /// * `type_name` - Value type name for the input
    ///
    /// Matches C++ `UsdLuxLightAPI::CreateInput()`.
    pub fn create_input(&self, name: &Token, type_name: &ValueTypeName) -> Option<Input> {
        Input::new(&self.prim, name, type_name)
    }

    /// Returns the requested input if it exists.
    ///
    /// # Arguments
    /// * `name` - Name of the input (without "inputs:" prefix)
    ///
    /// Matches C++ `UsdLuxLightAPI::GetInput()`.
    pub fn get_input(&self, name: &Token) -> Option<Input> {
        // Try to get existing input attribute
        let attr_name = format!("inputs:{}", name.as_str());
        if self.prim.get_attribute(&attr_name).is_some() {
            // Use a scalar type name for retrieval
            Input::new(&self.prim, name, &ValueTypeName::invalid())
        } else {
            None
        }
    }

    /// Returns all inputs on this light.
    ///
    /// Inputs are represented by attributes in the "inputs:" namespace.
    ///
    /// # Arguments
    /// * `only_authored` - If true, return only authored attributes
    ///
    /// Matches C++ `UsdLuxLightAPI::GetInputs()`.
    pub fn get_inputs(&self, only_authored: bool) -> Vec<Input> {
        self.connectable_api().get_inputs(only_authored)
    }

    // =========================================================================
    // Render Context Shader ID
    // =========================================================================

    /// Returns the shader ID attribute for the given render context.
    ///
    /// If `render_context` is non-empty, this will try to return an attribute
    /// named `light:shaderId` with the namespace prefix `render_context`.
    /// For example, if the render context is "ri", the attribute would be
    /// `ri:light:shaderId`.
    ///
    /// If the render context is empty, returns the default shader ID attribute.
    ///
    /// # Arguments
    /// * `render_context` - Render context namespace (e.g., "ri", "prman")
    ///
    /// Matches C++ `UsdLuxLightAPI::GetShaderIdAttrForRenderContext()`.
    pub fn get_shader_id_attr_for_render_context(
        &self,
        render_context: &Token,
    ) -> Option<Attribute> {
        if render_context.as_str().is_empty() {
            return self.get_shader_id_attr();
        }

        // C++ uses SdfPath::JoinIdentifier(renderContext, lightShaderId)
        let attr_name = format!(
            "{}:{}",
            render_context.as_str(),
            tokens().light_shader_id.as_str()
        );
        // Only return the attribute if it actually has a spec authored.
        // get_attribute() returns handles for any namespaced path even
        // without a spec; has_attribute() checks the composed prim index.
        if self.prim.has_attribute(&attr_name) {
            self.prim.get_attribute(&attr_name)
        } else {
            None
        }
    }

    /// Creates the shader ID attribute for the given render context.
    ///
    /// See [`get_shader_id_attr_for_render_context`](Self::get_shader_id_attr_for_render_context).
    ///
    /// # Arguments
    /// * `render_context` - Render context namespace
    /// * `default_value` - Default value for the attribute (VtValue parity)
    /// * `write_sparsely` - Parity with pxr `writeSparsely` (not yet applied in Rust)
    ///
    /// Matches C++ `UsdLuxLightAPI::CreateShaderIdAttrForRenderContext()`.
    pub fn create_shader_id_attr_for_render_context(
        &self,
        render_context: &Token,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        if render_context.as_str().is_empty() {
            return self.create_shader_id_attr(default_value, write_sparsely);
        }

        let t = tokens();
        let name = format!("{}:{}", render_context.as_str(), t.light_shader_id.as_str());
        create_lux_schema_attr(
            &self.prim,
            &name,
            "token",
            Variability::Uniform,
            default_value,
            write_sparsely,
        )
    }

    /// Returns the light's shader ID for the given list of render contexts.
    ///
    /// The shader ID returned is the identifier to use when looking up the
    /// shader definition for this light in the shader registry.
    ///
    /// The render contexts are expected to be listed in priority order.
    /// For each render context, this tries to find the shader ID attribute
    /// specific to that render context and returns the first non-empty value.
    /// If no shader ID value can be found for any context, returns the value
    /// of the default shader ID attribute.
    ///
    /// # Arguments
    /// * `render_contexts` - Prioritized list of render contexts
    ///
    /// Matches C++ `UsdLuxLightAPI::GetShaderId()`.
    pub fn get_shader_id(&self, render_contexts: &[Token]) -> Token {
        // Try render-context-specific shader IDs in priority order
        for context in render_contexts {
            if let Some(attr) = self.get_shader_id_attr_for_render_context(context) {
                if let Some(id) = attr.get_typed::<Token>(TimeCode::default()) {
                    if !id.as_str().is_empty() {
                        return id;
                    }
                }
            }
        }

        // Fall back to default shader ID
        if let Some(attr) = self.get_shader_id_attr() {
            if let Some(id) = attr.get_typed::<Token>(TimeCode::default()) {
                return id;
            }
        }

        Token::new("")
    }
}

impl Default for LightAPI {
    fn default() -> Self {
        Self::invalid()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_core::{InitialLoadSet, Stage};

    #[test]
    fn test_invalid_light_api() {
        let light_api = LightAPI::default();
        assert!(!light_api.is_valid());
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = LightAPI::get_schema_attribute_names(true);
        assert!(names.len() >= 10);
    }

    #[test]
    fn test_collection_names() {
        let light_link = LightAPI::get_light_link_collection_name();
        let shadow_link = LightAPI::get_shadow_link_collection_name();
        assert_eq!(light_link.as_str(), "lightLink");
        assert_eq!(shadow_link.as_str(), "shadowLink");
    }

    #[test]
    fn test_render_context_shader_id_attr_name() {
        // Verify the render-context-specific attr name format
        let t = tokens();
        let render_context = "ri";
        let expected = format!("{}:{}", render_context, t.light_shader_id.as_str());
        assert_eq!(expected, "ri:light:shaderId");
    }

    #[test]
    fn create_intensity_attr_sets_optional_default() {
        let _ = usd_sdf::init();
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("stage");
        let prim = stage.define_prim("/Light", "").expect("prim");
        let api = LightAPI::apply(&prim).expect("apply");
        let attr = api.create_intensity_attr(Some(Value::from_f32(2.5)), false);
        assert!(attr.is_valid());
        assert_eq!(attr.get_typed::<f32>(TimeCode::default()), Some(2.5));
        let attr2 = api.create_intensity_attr(None, false);
        assert_eq!(attr.path(), attr2.path());
    }

    #[test]
    fn create_shader_id_attr_sets_optional_default() {
        let _ = usd_sdf::init();
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("stage");
        let prim = stage.define_prim("/Light2", "").expect("prim");
        let api = LightAPI::apply(&prim).expect("apply");
        let tok = Token::new("MyLightShader");
        let attr = api.create_shader_id_attr(Some(Value::from(tok.clone())), false);
        assert!(attr.is_valid());
        assert_eq!(attr.get_typed::<Token>(TimeCode::default()), Some(tok));
    }
}
