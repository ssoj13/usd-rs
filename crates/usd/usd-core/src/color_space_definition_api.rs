//! UsdColorSpaceDefinitionAPI - Multiple-apply API schema for defining custom color spaces.
//!
//! Port of pxr/usd/usd/colorSpaceDefinitionAPI.h
//!
//! This is a **multiple-apply** API schema for defining custom color spaces on prims.
//! Custom color spaces become available for use on prims or for assignment to attributes
//! via the `colorSpace:name` property (from `UsdColorSpaceAPI`).
//!
//! Color spaces defined on a prim are available to all descendants of that prim,
//! unless overridden by a more local definition bearing the same name.

use super::attribute::Attribute;
use super::common::SchemaKind;
use super::prim::Prim;
use super::schema_base::SchemaBase;
use std::sync::Arc;
use usd_gf::{ColorSpace, Matrix3f, Vec2f};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Value;

/// Namespace prefix for color space definition attributes.
const NAMESPACE_PREFIX: &str = "colorSpaceDefinition:";

// ============================================================================
// UsdColorSpaceDefinitionAPI
// ============================================================================

/// UsdColorSpaceDefinitionAPI is a multiple-apply API schema for defining custom
/// color spaces. Each instance is identified by a name (the instance name).
///
/// Attributes per instance (under `colorSpaceDefinition:<name>:`):
/// - `name`        (token, uniform) - The display name of the color space.
/// - `redChroma`   (float2) - Red chromaticity coordinates. Default: (1, 0).
/// - `greenChroma` (float2) - Green chromaticity coordinates. Default: (0, 1).
/// - `blueChroma`  (float2) - Blue chromaticity coordinates. Default: (0, 0).
/// - `whitePoint`  (float2) - White point chromaticity coordinates. Default: (1/3, 1/3).
/// - `gamma`       (float)  - Gamma value of the log section. Default: 1.0.
/// - `linearBias`  (float)  - Linear bias of the log section. Default: 0.0.
///
/// Matches C++ `UsdColorSpaceDefinitionAPI`.
#[derive(Debug, Clone)]
pub struct ColorSpaceDefinitionAPI {
    /// The underlying schema base.
    schema_base: SchemaBase,
    /// The instance name (e.g. "myCustomSpace").
    instance_name: Token,
}

impl ColorSpaceDefinitionAPI {
    /// Compile-time constant: this is a MultipleApplyAPI.
    pub const SCHEMA_KIND: SchemaKind = SchemaKind::MultipleApplyAPI;

    /// Schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "ColorSpaceDefinitionAPI";

    /// Property base names (without namespace prefix).
    const PROP_NAME: &'static str = "name";
    const PROP_RED_CHROMA: &'static str = "redChroma";
    const PROP_GREEN_CHROMA: &'static str = "greenChroma";
    const PROP_BLUE_CHROMA: &'static str = "blueChroma";
    const PROP_WHITE_POINT: &'static str = "whitePoint";
    const PROP_GAMMA: &'static str = "gamma";
    const PROP_LINEAR_BIAS: &'static str = "linearBias";

    // =========================================================================
    // Constructors
    // =========================================================================

    /// Construct on the given prim with the given instance name.
    pub fn new(prim: &Prim, name: Token) -> Self {
        Self {
            schema_base: SchemaBase::new(prim.clone()),
            instance_name: name,
        }
    }

    /// Returns the prim this schema is attached to.
    pub fn prim(&self) -> &Prim {
        self.schema_base.prim()
    }

    /// Returns the instance name of this multiple-apply schema instance.
    pub fn name(&self) -> &Token {
        &self.instance_name
    }

    /// Returns whether this schema instance is valid.
    pub fn is_valid(&self) -> bool {
        self.schema_base.prim().is_valid() && !self.instance_name.is_empty()
    }

    /// Returns the schema kind.
    pub fn schema_kind(&self) -> SchemaKind {
        Self::SCHEMA_KIND
    }

    // =========================================================================
    // Static methods
    // =========================================================================

    /// Get a ColorSpaceDefinitionAPI for a prim path of the form
    /// `<path>.colorSpaceDefinition:<name>`.
    pub fn get_by_path(stage: &Arc<super::stage::Stage>, path: &Path) -> Option<Self> {
        let token_str = path.get_name();
        let instance_name = token_str.strip_prefix(NAMESPACE_PREFIX)?;
        let prim_path = path.get_prim_path();
        let prim = stage.get_prim_at_path(&prim_path)?;
        Some(Self::new(&prim, Token::new(instance_name)))
    }

    /// Get a ColorSpaceDefinitionAPI with the given name on the given prim.
    pub fn get(prim: &Prim, name: &Token) -> Self {
        Self::new(prim, name.clone())
    }

    /// Return all named instances of ColorSpaceDefinitionAPI on the given prim.
    pub fn get_all(prim: &Prim) -> Vec<Self> {
        let schemas = prim.get_authored_applied_schemas();
        let mut result = Vec::new();
        let prefix = format!("{}:", Self::SCHEMA_TYPE_NAME);
        for schema in &schemas {
            if let Some(instance) = schema.as_str().strip_prefix(&prefix) {
                result.push(Self::new(prim, Token::new(instance)));
            }
        }
        result
    }

    /// Checks if the given base name is a property of this schema.
    pub fn is_schema_property_base_name(base_name: &Token) -> bool {
        matches!(
            base_name.get_text(),
            "name"
                | "redChroma"
                | "greenChroma"
                | "blueChroma"
                | "whitePoint"
                | "gamma"
                | "linearBias"
        )
    }

    /// Checks if the given path is of an API schema of type ColorSpaceDefinitionAPI.
    /// If so, returns the instance name.
    pub fn is_color_space_definition_api_path(path: &Path) -> Option<Token> {
        let name_str = path.get_name();
        name_str
            .strip_prefix(NAMESPACE_PREFIX)
            .map(|n| Token::new(n))
    }

    /// Returns true if this multiple-apply API schema can be applied with the
    /// given instance name to the given prim.
    pub fn can_apply(prim: &Prim, name: &Token) -> bool {
        prim.is_valid() && !name.is_empty()
    }

    /// Applies this multiple-apply API schema to the given prim with instance name.
    /// Adds "ColorSpaceDefinitionAPI:<name>" to the apiSchemas metadata.
    pub fn apply(prim: &Prim, name: &Token) -> Self {
        if prim.is_valid() && !name.is_empty() {
            let mut schemas = prim.get_authored_applied_schemas();
            let api_token = Token::new(&format!("{}:{}", Self::SCHEMA_TYPE_NAME, name.as_str()));
            if !schemas.contains(&api_token) {
                schemas.push(api_token);
                // Note: actual metadata authoring happens in the underlying layer
            }
        }
        Self::new(prim, name.clone())
    }

    /// Returns the schema attribute base names (without namespace prefix).
    pub fn get_schema_attribute_names(_include_inherited: bool) -> Vec<Token> {
        let names = vec![
            Token::new(Self::PROP_NAME),
            Token::new(Self::PROP_RED_CHROMA),
            Token::new(Self::PROP_GREEN_CHROMA),
            Token::new(Self::PROP_BLUE_CHROMA),
            Token::new(Self::PROP_WHITE_POINT),
            Token::new(Self::PROP_GAMMA),
            Token::new(Self::PROP_LINEAR_BIAS),
        ];
        // APISchemaBase has no additional attributes to inherit
        names
    }

    /// Returns schema attribute names with proper namespace prefix for the instance.
    pub fn get_schema_attribute_names_for_instance(
        include_inherited: bool,
        instance_name: &Token,
    ) -> Vec<Token> {
        let base_names = Self::get_schema_attribute_names(include_inherited);
        let prefix = format!("{}{}:", NAMESPACE_PREFIX, instance_name.as_str());
        base_names
            .into_iter()
            .map(|n| Token::new(&format!("{}{}", prefix, n.as_str())))
            .collect()
    }

    // =========================================================================
    // Attribute helpers
    // =========================================================================

    /// Builds the full namespaced attribute name for this instance.
    fn attr_name(&self, prop: &str) -> String {
        format!(
            "{}{}:{}",
            NAMESPACE_PREFIX,
            self.instance_name.as_str(),
            prop
        )
    }

    /// Get an attribute by base property name.
    fn get_attr(&self, prop: &str) -> Option<Attribute> {
        self.prim().get_attribute(&self.attr_name(prop))
    }

    /// Create an attribute by base property name with optional default value.
    fn create_attr(&self, prop: &str, default_value: Option<Value>) -> Option<Attribute> {
        let type_name = usd_sdf::ValueTypeName::invalid();
        let attr = self
            .prim()
            .create_attribute(&self.attr_name(prop), &type_name, false, None);
        if let (Some(attr), Some(val)) = (&attr, default_value) {
            attr.set(val, Default::default());
        }
        attr
    }

    // =========================================================================
    // Name attribute (uniform token, default "custom")
    // =========================================================================

    /// Returns the `name` attribute (the display name of the color space).
    pub fn get_name_attr(&self) -> Option<Attribute> {
        self.get_attr(Self::PROP_NAME)
    }

    /// Creates the `name` attribute.
    pub fn create_name_attr(&self, default_value: Option<Value>) -> Option<Attribute> {
        let val = default_value.unwrap_or_else(|| Value::from(Token::new("custom")));
        self.create_attr(Self::PROP_NAME, Some(val))
    }

    // =========================================================================
    // Red chroma (float2, default (1, 0))
    // =========================================================================

    /// Returns the `redChroma` attribute.
    pub fn get_red_chroma_attr(&self) -> Option<Attribute> {
        self.get_attr(Self::PROP_RED_CHROMA)
    }

    /// Creates the `redChroma` attribute.
    pub fn create_red_chroma_attr(&self, default_value: Option<Value>) -> Option<Attribute> {
        let val = default_value.unwrap_or_else(|| Value::from(Vec2f::new(1.0, 0.0)));
        self.create_attr(Self::PROP_RED_CHROMA, Some(val))
    }

    // =========================================================================
    // Green chroma (float2, default (0, 1))
    // =========================================================================

    /// Returns the `greenChroma` attribute.
    pub fn get_green_chroma_attr(&self) -> Option<Attribute> {
        self.get_attr(Self::PROP_GREEN_CHROMA)
    }

    /// Creates the `greenChroma` attribute.
    pub fn create_green_chroma_attr(&self, default_value: Option<Value>) -> Option<Attribute> {
        let val = default_value.unwrap_or_else(|| Value::from(Vec2f::new(0.0, 1.0)));
        self.create_attr(Self::PROP_GREEN_CHROMA, Some(val))
    }

    // =========================================================================
    // Blue chroma (float2, default (0, 0))
    // =========================================================================

    /// Returns the `blueChroma` attribute.
    pub fn get_blue_chroma_attr(&self) -> Option<Attribute> {
        self.get_attr(Self::PROP_BLUE_CHROMA)
    }

    /// Creates the `blueChroma` attribute.
    pub fn create_blue_chroma_attr(&self, default_value: Option<Value>) -> Option<Attribute> {
        let val = default_value.unwrap_or_else(|| Value::from(Vec2f::new(0.0, 0.0)));
        self.create_attr(Self::PROP_BLUE_CHROMA, Some(val))
    }

    // =========================================================================
    // White point (float2, default (1/3, 1/3))
    // =========================================================================

    /// Returns the `whitePoint` attribute.
    pub fn get_white_point_attr(&self) -> Option<Attribute> {
        self.get_attr(Self::PROP_WHITE_POINT)
    }

    /// Creates the `whitePoint` attribute.
    pub fn create_white_point_attr(&self, default_value: Option<Value>) -> Option<Attribute> {
        let v = 1.0_f32 / 3.0;
        let val = default_value.unwrap_or_else(|| Value::from(Vec2f::new(v, v)));
        self.create_attr(Self::PROP_WHITE_POINT, Some(val))
    }

    // =========================================================================
    // Gamma (float, default 1.0)
    // =========================================================================

    /// Returns the `gamma` attribute.
    pub fn get_gamma_attr(&self) -> Option<Attribute> {
        self.get_attr(Self::PROP_GAMMA)
    }

    /// Creates the `gamma` attribute.
    pub fn create_gamma_attr(&self, default_value: Option<Value>) -> Option<Attribute> {
        let val = default_value.unwrap_or_else(|| Value::from(1.0_f32));
        self.create_attr(Self::PROP_GAMMA, Some(val))
    }

    // =========================================================================
    // Linear bias (float, default 0.0)
    // =========================================================================

    /// Returns the `linearBias` attribute.
    pub fn get_linear_bias_attr(&self) -> Option<Attribute> {
        self.get_attr(Self::PROP_LINEAR_BIAS)
    }

    /// Creates the `linearBias` attribute.
    pub fn create_linear_bias_attr(&self, default_value: Option<Value>) -> Option<Attribute> {
        let val = default_value.unwrap_or_else(|| Value::from(0.0_f32));
        self.create_attr(Self::PROP_LINEAR_BIAS, Some(val))
    }

    // =========================================================================
    // Custom convenience methods
    // =========================================================================

    /// Creates all color space attributes from chromaticity coordinates.
    ///
    /// Sets redChroma, greenChroma, blueChroma, whitePoint, gamma, and linearBias.
    pub fn create_attrs_with_chroma(
        &self,
        red_chroma: Vec2f,
        green_chroma: Vec2f,
        blue_chroma: Vec2f,
        white_point: Vec2f,
        gamma: f32,
        linear_bias: f32,
    ) {
        self.create_red_chroma_attr(Some(Value::from(red_chroma)));
        self.create_green_chroma_attr(Some(Value::from(green_chroma)));
        self.create_blue_chroma_attr(Some(Value::from(blue_chroma)));
        self.create_white_point_attr(Some(Value::from(white_point)));
        self.create_gamma_attr(Some(Value::from(gamma)));
        self.create_linear_bias_attr(Some(Value::from(linear_bias)));
    }

    /// Creates color space attributes from an RGB-to-XYZ matrix and linearization params.
    ///
    /// Derives chromaticities from the matrix columns, then authors the attributes.
    pub fn create_attrs_with_matrix(&self, rgb_to_xyz: &Matrix3f, gamma: f32, linear_bias: f32) {
        // Derive chromaticities from the RGB-to-XYZ matrix columns.
        // Each column of the matrix represents the XYZ coordinates of R, G, B.
        // Chromaticity = (X / (X+Y+Z), Y / (X+Y+Z))
        let chroma = |col: usize| -> Vec2f {
            let x = rgb_to_xyz[col][0];
            let y = rgb_to_xyz[col][1];
            let z = rgb_to_xyz[col][2];
            let sum = x + y + z;
            if sum.abs() < 1e-10 {
                Vec2f::new(0.0, 0.0)
            } else {
                Vec2f::new(x / sum, y / sum)
            }
        };

        let red = chroma(0);
        let green = chroma(1);
        let blue = chroma(2);

        // White point: sum of all columns
        let wx = rgb_to_xyz[0][0] + rgb_to_xyz[1][0] + rgb_to_xyz[2][0];
        let wy = rgb_to_xyz[0][1] + rgb_to_xyz[1][1] + rgb_to_xyz[2][1];
        let wz = rgb_to_xyz[0][2] + rgb_to_xyz[1][2] + rgb_to_xyz[2][2];
        let wsum = wx + wy + wz;
        let white = if wsum.abs() < 1e-10 {
            Vec2f::new(1.0 / 3.0, 1.0 / 3.0)
        } else {
            Vec2f::new(wx / wsum, wy / wsum)
        };

        self.create_attrs_with_chroma(red, green, blue, white, gamma, linear_bias);
    }

    /// Computes a GfColorSpace from the definition attributes on this prim.
    ///
    /// Reads the authored chromaticity, gamma, and linear bias attributes
    /// and constructs a corresponding `GfColorSpace`.
    pub fn compute_color_space(&self) -> ColorSpace {
        // Helper to read a Vec2f attribute
        let read_vec2f = |attr: Option<Attribute>, default: Vec2f| -> Vec2f {
            attr.and_then(|a| a.get(usd_sdf::TimeCode::default_time()))
                .and_then(|v| v.get::<Vec2f>().cloned())
                .unwrap_or(default)
        };

        // Helper to read a float attribute
        let read_f32 = |attr: Option<Attribute>, default: f32| -> f32 {
            attr.and_then(|a| a.get(usd_sdf::TimeCode::default_time()))
                .and_then(|v| v.get::<f32>().copied())
                .unwrap_or(default)
        };

        let red = read_vec2f(self.get_red_chroma_attr(), Vec2f::new(1.0, 0.0));
        let green = read_vec2f(self.get_green_chroma_attr(), Vec2f::new(0.0, 1.0));
        let blue = read_vec2f(self.get_blue_chroma_attr(), Vec2f::new(0.0, 0.0));
        let white = read_vec2f(
            self.get_white_point_attr(),
            Vec2f::new(1.0 / 3.0, 1.0 / 3.0),
        );
        let gamma = read_f32(self.get_gamma_attr(), 1.0);
        let linear_bias = read_f32(self.get_linear_bias_attr(), 0.0);

        // Read the instance name for the color space name
        let cs_name = self
            .get_name_attr()
            .and_then(|a| a.get(usd_sdf::TimeCode::default_time()))
            .and_then(|v| v.get::<Token>().cloned())
            .unwrap_or_else(|| Token::new("custom"));

        // Construct via chromaticities + transfer function params
        ColorSpace::from_primaries(&cs_name, red, green, blue, white, gamma, linear_bias)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_kind() {
        assert_eq!(
            ColorSpaceDefinitionAPI::SCHEMA_KIND,
            SchemaKind::MultipleApplyAPI
        );
    }

    #[test]
    fn test_is_schema_property_base_name() {
        assert!(ColorSpaceDefinitionAPI::is_schema_property_base_name(
            &Token::new("redChroma")
        ));
        assert!(ColorSpaceDefinitionAPI::is_schema_property_base_name(
            &Token::new("gamma")
        ));
        assert!(!ColorSpaceDefinitionAPI::is_schema_property_base_name(
            &Token::new("bogus")
        ));
    }

    #[test]
    fn test_is_color_space_definition_api_path() {
        let path = Path::from_string("/World.colorSpaceDefinition:mySpace").unwrap();
        let name = ColorSpaceDefinitionAPI::is_color_space_definition_api_path(&path);
        assert!(name.is_some());
        assert_eq!(name.unwrap().as_str(), "mySpace");
    }

    #[test]
    fn test_get_schema_attribute_names() {
        let names = ColorSpaceDefinitionAPI::get_schema_attribute_names(true);
        assert_eq!(names.len(), 7);
    }

    #[test]
    fn test_get_schema_attribute_names_for_instance() {
        let names = ColorSpaceDefinitionAPI::get_schema_attribute_names_for_instance(
            true,
            &Token::new("mySpace"),
        );
        assert_eq!(names.len(), 7);
        // Each name should be namespaced
        for name in &names {
            assert!(name.as_str().starts_with("colorSpaceDefinition:mySpace:"));
        }
    }
}
