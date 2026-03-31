//! Attribute specification for scene description.
//!
//! `AttributeSpec` represents a typed data property on a prim. Attributes can hold:
//! - A single default value
//! - Time samples (values varying over time)
//! - Connection paths to other attributes
//! - Metadata about the attribute (display info, allowed tokens, etc.)
//!
//! # Examples
//!
//! ```
//! use usd_sdf::{AttributeSpec, Spec, Path, LayerHandle, Value};
//!
//! // Create an attribute spec (typically via PrimSpec::new_attribute)
//! let layer = LayerHandle::null(); // Would be a real layer
//! let path = Path::from("/World.radius");
//! let spec = AttributeSpec::new(Spec::new(layer, path));
//!
//! // Access attribute properties
//! let type_name = spec.type_name();
//! let has_default = spec.has_default_value();
//! ```

use std::collections::{BTreeMap, HashMap};
use std::sync::OnceLock;

use usd_tf::Token;

use super::{
    LayerHandle,
    abstract_data::Value,
    list_op::PathListOp,
    path::Path,
    spec::{Spec, VtValue},
};

// Cached tokens for attribute field names
mod tokens {
    use super::*;

    macro_rules! cached_token {
        ($name:ident, $str:literal) => {
            pub fn $name() -> Token {
                static TOKEN: OnceLock<Token> = OnceLock::new();
                TOKEN.get_or_init(|| Token::new($str)).clone()
            }
        };
    }

    cached_token!(type_name, "typeName");
    cached_token!(variability, "variability");
    cached_token!(role_name, "roleName");
    cached_token!(default, "default");
    cached_token!(connection_paths, "connectionPaths");
    cached_token!(allowed_tokens, "allowedTokens");
    cached_token!(color_space, "colorSpace");
    cached_token!(display_unit, "displayUnit");
    cached_token!(time_samples, "timeSamples");
    cached_token!(limits, "limits");
    cached_token!(array_size_constraint, "arraySizeConstraint");
    cached_token!(spline, "spline");
}

// ============================================================================
// Helper Functions - VtValue conversions
// ============================================================================

/// Convert abstract_data::Value to VtValue.
///
/// Extracts common types from the internal Value storage and wraps in VtValue.
fn value_to_vt(value: &Value) -> VtValue {
    // Try common types
    if let Some(s) = value.downcast::<String>() {
        return VtValue::new(s.clone());
    }
    if let Some(i) = value.downcast::<i32>() {
        return VtValue::new(*i);
    }
    if let Some(i) = value.downcast::<i64>() {
        return VtValue::new(*i);
    }
    if let Some(f) = value.downcast::<f32>() {
        return VtValue::from_f32(*f);
    }
    if let Some(f) = value.downcast::<f64>() {
        return VtValue::from_f64(*f);
    }
    if let Some(b) = value.downcast::<bool>() {
        return VtValue::new(*b);
    }
    if let Some(t) = value.downcast::<Token>() {
        return VtValue::new(t.clone());
    }
    // Unsupported type
    VtValue::empty()
}

/// Extract PathListOp from VtValue.
fn extract_path_list_op(vt: &VtValue) -> Option<PathListOp> {
    vt.get::<PathListOp>().cloned()
}

/// Extract Vec<Token> from VtValue.
fn extract_token_vec(vt: &VtValue) -> Option<Vec<Token>> {
    vt.get::<Vec<Token>>().cloned()
}

/// Convert Vec<Token> to VtValue.
fn token_vec_to_vt(tokens: &[Token]) -> VtValue {
    VtValue::new(tokens.to_vec())
}

/// Extract time sample map from VtValue.
fn extract_time_sample_map(vt: &VtValue) -> Option<BTreeMap<OrderedFloat, Value>> {
    // Time samples are stored as BTreeMap<OrderedFloat, Value>
    vt.get::<BTreeMap<OrderedFloat, Value>>().cloned()
}

// ============================================================================
// OrderedFloat - f64 wrapper with total ordering
// ============================================================================

/// Wrapper for f64 that implements total ordering.
///
/// This allows f64 values to be used as keys in BTreeMap and other
/// ordered collections. NaN values are treated as equal and ordered
/// before all other values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrderedFloat(f64);

impl OrderedFloat {
    /// Create a new OrderedFloat.
    #[inline]
    pub const fn new(value: f64) -> Self {
        Self(value)
    }

    /// Get the wrapped value.
    #[inline]
    pub const fn value(self) -> f64 {
        self.0
    }
}

impl From<f64> for OrderedFloat {
    #[inline]
    fn from(value: f64) -> Self {
        Self::new(value)
    }
}

impl From<OrderedFloat> for f64 {
    #[inline]
    fn from(value: OrderedFloat) -> Self {
        value.0
    }
}

impl Eq for OrderedFloat {}

impl Ord for OrderedFloat {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Total ordering for f64: NaN < -inf < finite < +inf
        self.0.partial_cmp(&other.0).unwrap_or_else(|| {
            // Handle NaN cases
            match (self.0.is_nan(), other.0.is_nan()) {
                (true, true) => std::cmp::Ordering::Equal,
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                (false, false) => unreachable!(),
            }
        })
    }
}

impl PartialOrd for OrderedFloat {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl std::hash::Hash for OrderedFloat {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Hash the bit representation for consistent hashing
        self.0.to_bits().hash(state);
    }
}

// ============================================================================
// AttributeSpec - Typed attribute specification
// ============================================================================

/// Specification for an attribute property on a prim.
///
/// An attribute is a typed data container that can hold:
/// - A default value
/// - Time samples (animated values)
/// - Connection paths (references to other attributes)
/// - Metadata (display info, allowed values, etc.)
///
/// All values in an attribute must be of the same type, as specified by
/// the `type_name` field.
///
/// # Relationship to PropertySpec
///
/// In OpenUSD, AttributeSpec inherits from PropertySpec. Here we compose
/// directly with Spec since PropertySpec is not yet implemented. When
/// PropertySpec is added, this can be refactored to use it.
///
/// # Thread Safety
///
/// AttributeSpec is not thread-safe for mutation but can be read from
/// multiple threads if the underlying layer is not being modified.
#[derive(Debug, Clone, Default)]
pub struct AttributeSpec {
    /// Base spec containing layer and path.
    spec: Spec,
}

impl AttributeSpec {
    // ========================================================================
    // Construction
    // ========================================================================

    /// Create an AttributeSpec from a base Spec.
    ///
    /// This doesn't create the attribute in the layer - it just creates
    /// a handle to an attribute at that location. The spec must already
    /// exist in the layer.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::{AttributeSpec, Spec, Path, LayerHandle};
    ///
    /// let layer = LayerHandle::null();
    /// let path = Path::from("/World.radius");
    /// let spec = AttributeSpec::new(Spec::new(layer, path));
    /// ```
    #[inline]
    #[must_use]
    pub fn new(spec: Spec) -> Self {
        Self { spec }
    }

    /// Create an AttributeSpec from layer and path.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::{AttributeSpec, Path, LayerHandle};
    ///
    /// let layer = LayerHandle::null();
    /// let path = Path::from("/World.radius");
    /// let attr = AttributeSpec::from_layer_and_path(layer, path);
    /// ```
    #[inline]
    #[must_use]
    pub fn from_layer_and_path(layer: LayerHandle, path: Path) -> Self {
        Self {
            spec: Spec::new(layer, path),
        }
    }

    /// Returns a reference to the underlying Spec.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::AttributeSpec;
    ///
    /// let attr = AttributeSpec::default();
    /// let spec = attr.as_spec();
    /// assert!(spec.is_dormant());
    /// ```
    #[inline]
    #[must_use]
    pub fn as_spec(&self) -> &Spec {
        &self.spec
    }

    /// Returns a mutable reference to the underlying Spec.
    #[inline]
    #[must_use]
    pub fn as_spec_mut(&mut self) -> &mut Spec {
        &mut self.spec
    }

    /// Consumes self and returns the underlying Spec.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::{AttributeSpec, Path, LayerHandle};
    ///
    /// let layer = LayerHandle::null();
    /// let path = Path::from("/World.radius");
    /// let attr = AttributeSpec::from_layer_and_path(layer, path.clone());
    /// let spec = attr.into_spec();
    /// assert_eq!(spec.path(), path);
    /// ```
    #[inline]
    #[must_use]
    pub fn into_spec(self) -> Spec {
        self.spec
    }

    // ========================================================================
    // Basic Properties (delegated to Spec)
    // ========================================================================

    /// Returns the layer containing this attribute.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::AttributeSpec;
    ///
    /// let attr = AttributeSpec::default();
    /// let layer = attr.layer();
    /// assert!(!layer.is_valid());
    /// ```
    #[inline]
    #[must_use]
    pub fn layer(&self) -> LayerHandle {
        self.spec.layer()
    }

    /// Returns the path to this attribute.
    ///
    /// For attributes, this is a property path (e.g., "/World.radius").
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::{AttributeSpec, Path, LayerHandle};
    ///
    /// let path = Path::from("/World.radius");
    /// let attr = AttributeSpec::from_layer_and_path(LayerHandle::null(), path.clone());
    /// assert_eq!(attr.path(), path);
    /// ```
    #[inline]
    #[must_use]
    pub fn path(&self) -> Path {
        self.spec.path()
    }

    /// Returns true if this spec is dormant (invalid).
    ///
    /// A dormant spec cannot be used for field operations.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::AttributeSpec;
    ///
    /// let attr = AttributeSpec::default();
    /// assert!(attr.is_dormant());
    /// ```
    #[inline]
    #[must_use]
    pub fn is_dormant(&self) -> bool {
        self.spec.is_dormant()
    }

    // ========================================================================
    // Type Information
    // ========================================================================

    /// Returns the type name of this attribute.
    ///
    /// The type name specifies what kind of data this attribute holds
    /// (e.g., "float", "Vec3f", "string"). All values (default and time
    /// samples) must be of this type.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::{AttributeSpec, Token};
    ///
    /// let attr = AttributeSpec::default();
    /// let type_name = attr.type_name();
    /// // Returns empty string if not set or dormant
    /// ```
    #[must_use]
    pub fn type_name(&self) -> String {
        if self.is_dormant() {
            return String::new();
        }

        // Get "typeName" field
        let field = self.spec.get_field(&tokens::type_name());
        if field.is_empty() {
            return String::new();
        }

        // Try String first, then Token (USDC stores typeName as Token)
        if let Some(s) = field.get::<String>() {
            return s.clone();
        }
        if let Some(t) = field.get::<usd_tf::Token>() {
            return t.as_str().to_string();
        }
        String::new()
    }

    /// Sets the type name of this attribute.
    ///
    /// # Parameters
    ///
    /// - `type_name` - The type name (e.g., "float", "Vec3f", "string")
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::{AttributeSpec, Path, LayerHandle};
    ///
    /// let mut attr = AttributeSpec::from_layer_and_path(
    ///     LayerHandle::null(),
    ///     Path::from("/World.radius")
    /// );
    /// attr.set_type_name("float");
    /// ```
    pub fn set_type_name(&mut self, type_name: impl Into<String>) {
        if self.is_dormant() {
            return;
        }

        let type_name_str = type_name.into();
        let value = VtValue::new(type_name_str);
        let _ = self.spec.set_field(&tokens::type_name(), value);
    }

    /// Returns the variability of this attribute.
    ///
    /// Variability indicates whether the attribute may vary over time:
    /// - `Varying` (default) - Value can change over time
    /// - `Uniform` - Value is constant across all times
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::{AttributeSpec, Variability};
    ///
    /// let attr = AttributeSpec::default();
    /// // Default is Varying
    /// assert_eq!(attr.variability(), Variability::Varying);
    /// ```
    #[must_use]
    pub fn variability(&self) -> super::Variability {
        if self.is_dormant() {
            return super::Variability::default();
        }
        self.spec
            .get_field(&tokens::variability())
            .get::<super::Variability>()
            .copied()
            .unwrap_or_default()
    }

    /// Sets the variability of this attribute.
    ///
    /// # Parameters
    ///
    /// - `variability` - The variability to set
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::{AttributeSpec, Variability};
    ///
    /// let mut attr = AttributeSpec::default();
    /// attr.set_variability(Variability::Uniform);
    /// ```
    pub fn set_variability(&mut self, variability: super::Variability) {
        if self.is_dormant() {
            return;
        }
        let value = VtValue::new(variability);
        let _ = self.spec.set_field(&tokens::variability(), value);
    }

    /// Returns the role name for this attribute's type.
    ///
    /// The role name provides semantic information about how the type
    /// should be interpreted (e.g., "Color", "Point", "Vector", "Normal").
    ///
    /// Returns `None` if the type has no role or if the attribute is dormant.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::AttributeSpec;
    ///
    /// let attr = AttributeSpec::default();
    /// assert_eq!(attr.role_name(), None);
    /// ```
    #[must_use]
    pub fn role_name(&self) -> Option<Token> {
        if self.is_dormant() {
            return None;
        }

        // Get "roleName" field
        let field = self.spec.get_field(&tokens::role_name());
        if field.is_empty() {
            return None;
        }

        // Try to extract as string and convert to Token
        field.get::<String>().map(|s| Token::new(s))
    }

    // ========================================================================
    // Default Value
    // ========================================================================

    /// Returns true if this attribute has a default value.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::AttributeSpec;
    ///
    /// let attr = AttributeSpec::default();
    /// assert!(!attr.has_default_value());
    /// ```
    #[inline]
    #[must_use]
    pub fn has_default_value(&self) -> bool {
        if self.is_dormant() {
            return false;
        }
        self.spec.has_field(&tokens::default())
    }

    /// Returns the default value of this attribute.
    ///
    /// Returns an empty Value if no default is set or the spec is dormant.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::AttributeSpec;
    ///
    /// let attr = AttributeSpec::default();
    /// let default = attr.default_value();
    /// ```
    #[must_use]
    pub fn default_value(&self) -> Value {
        if self.is_dormant() {
            return Value::new(());
        }

        // Return the field directly — do NOT re-wrap (would double-wrap VtValue inside VtValue)
        match self.spec.get_field(&tokens::default()) {
            field if !field.is_empty() => field,
            _ => Value::new(()),
        }
    }

    /// Sets the default value of this attribute.
    ///
    /// # Parameters
    ///
    /// - `value` - The default value to set
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::{AttributeSpec, Value};
    ///
    /// let mut attr = AttributeSpec::default();
    /// attr.set_default_value(Value::from_f32(42.0));
    /// ```
    pub fn set_default_value(&mut self, value: Value) {
        if self.is_dormant() {
            return;
        }

        // Convert abstract_data::Value to VtValue
        let vt_value = value_to_vt(&value);
        let _ = self.spec.set_field(&tokens::default(), vt_value);
    }

    /// Clears the default value of this attribute.
    ///
    /// After calling this, `has_default_value()` will return false.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::AttributeSpec;
    ///
    /// let mut attr = AttributeSpec::default();
    /// attr.clear_default_value();
    /// ```
    pub fn clear_default_value(&mut self) {
        if self.is_dormant() {
            return;
        }

        let _ = self.spec.clear_field(&tokens::default());
    }

    // ========================================================================
    // Connection Paths
    // ========================================================================

    /// Returns the connection paths list editor for this attribute.
    ///
    /// Connection paths specify which other attributes this attribute
    /// gets its value from. This is used for attribute relationships
    /// and shader networks.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::AttributeSpec;
    ///
    /// let attr = AttributeSpec::default();
    /// let connections = attr.connection_paths_list();
    /// ```
    #[must_use]
    pub fn connection_paths_list(&self) -> PathListOp {
        if self.is_dormant() {
            return PathListOp::default();
        }

        // Get "connectionPaths" field
        let field = self.spec.get_field(&tokens::connection_paths());
        if field.is_empty() {
            return PathListOp::default();
        }

        // Extract PathListOp from VtValue
        extract_path_list_op(&field).unwrap_or_default()
    }

    /// Returns true if this attribute has any connection paths.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::AttributeSpec;
    ///
    /// let attr = AttributeSpec::default();
    /// assert!(!attr.has_connection_paths());
    /// ```
    #[must_use]
    pub fn has_connection_paths(&self) -> bool {
        if self.is_dormant() {
            return false;
        }

        let list_op = self.connection_paths_list();
        !list_op.is_explicit() || !list_op.get_explicit_items().is_empty()
    }

    /// Sets the connection paths list for this attribute.
    ///
    /// # Parameters
    ///
    /// - `list_op` - The path list operation to set
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::{AttributeSpec, PathListOp, Path};
    ///
    /// let mut attr = AttributeSpec::default();
    /// let mut list_op = PathListOp::default();
    /// let _ = list_op.set_appended_items(vec![Path::from_string("/Source.attr").unwrap()]);
    /// attr.set_connection_paths_list(list_op);
    /// ```
    pub fn set_connection_paths_list(&mut self, list_op: PathListOp) {
        if self.is_dormant() {
            return;
        }

        let value: VtValue = list_op.into();
        let _ = self.spec.set_field(&tokens::connection_paths(), value);
    }

    /// Clears all connection paths from this attribute.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::AttributeSpec;
    ///
    /// let mut attr = AttributeSpec::default();
    /// attr.clear_connection_paths();
    /// ```
    pub fn clear_connection_paths(&mut self) {
        if self.is_dormant() {
            return;
        }

        let _ = self.spec.clear_field(&tokens::connection_paths());
    }

    // ========================================================================
    // Allowed Tokens
    // ========================================================================

    /// Returns the allowed tokens for this attribute.
    ///
    /// Allowed tokens define a set of predefined valid values for this
    /// attribute. This is advisory metadata - validation is up to the
    /// consumer.
    ///
    /// Returns `None` if no allowed tokens are set or the spec is dormant.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::AttributeSpec;
    ///
    /// let attr = AttributeSpec::default();
    /// assert_eq!(attr.allowed_tokens(), None);
    /// ```
    #[must_use]
    pub fn allowed_tokens(&self) -> Option<Vec<Token>> {
        if self.is_dormant() {
            return None;
        }

        let field = self.spec.get_field(&tokens::allowed_tokens());
        if field.is_empty() {
            return None;
        }

        // Extract Vec<Token> from VtValue
        extract_token_vec(&field)
    }

    /// Sets the allowed tokens for this attribute.
    ///
    /// # Parameters
    ///
    /// - `tokens` - The list of allowed token values
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::{AttributeSpec, Token};
    ///
    /// let mut attr = AttributeSpec::default();
    /// attr.set_allowed_tokens(vec![
    ///     Token::new("red"),
    ///     Token::new("green"),
    ///     Token::new("blue"),
    /// ]);
    /// ```
    pub fn set_allowed_tokens(&mut self, tokens: Vec<Token>) {
        if self.is_dormant() {
            return;
        }

        // Convert Vec<Token> to VtValue
        let vt_value = token_vec_to_vt(&tokens);
        let _ = self.spec.set_field(&tokens::allowed_tokens(), vt_value);
    }

    /// Returns true if allowed tokens metadata is set.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::AttributeSpec;
    ///
    /// let attr = AttributeSpec::default();
    /// assert!(!attr.has_allowed_tokens());
    /// ```
    #[inline]
    #[must_use]
    pub fn has_allowed_tokens(&self) -> bool {
        if self.is_dormant() {
            return false;
        }
        self.spec.has_field(&tokens::allowed_tokens())
    }

    /// Clears the allowed tokens metadata.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::AttributeSpec;
    ///
    /// let mut attr = AttributeSpec::default();
    /// attr.clear_allowed_tokens();
    /// ```
    pub fn clear_allowed_tokens(&mut self) {
        if self.is_dormant() {
            return;
        }

        let _ = self.spec.clear_field(&tokens::allowed_tokens());
    }

    // ========================================================================
    // Color Space
    // ========================================================================

    /// Returns the color space for this attribute.
    ///
    /// Color space indicates how color or texture values should be
    /// interpreted. Common values include "auto", "raw", "sRGB", "linear".
    ///
    /// Returns `None` if no color space is set or the spec is dormant.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::AttributeSpec;
    ///
    /// let attr = AttributeSpec::default();
    /// assert_eq!(attr.color_space(), None);
    /// ```
    #[must_use]
    pub fn color_space(&self) -> Option<Token> {
        if self.is_dormant() {
            return None;
        }

        let field = self.spec.get_field(&tokens::color_space());
        if field.is_empty() {
            return None;
        }

        field.get::<String>().map(|s| Token::new(s))
    }

    /// Sets the color space for this attribute.
    ///
    /// # Parameters
    ///
    /// - `color_space` - The color space token (e.g., "sRGB", "linear")
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::{AttributeSpec, Token};
    ///
    /// let mut attr = AttributeSpec::default();
    /// attr.set_color_space(Token::new("sRGB"));
    /// ```
    pub fn set_color_space(&mut self, color_space: Token) {
        if self.is_dormant() {
            return;
        }

        let value = VtValue::new(color_space.as_str().to_string());
        let _ = self.spec.set_field(&tokens::color_space(), value);
    }

    /// Returns true if color space metadata is set.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::AttributeSpec;
    ///
    /// let attr = AttributeSpec::default();
    /// assert!(!attr.has_color_space());
    /// ```
    #[inline]
    #[must_use]
    pub fn has_color_space(&self) -> bool {
        if self.is_dormant() {
            return false;
        }
        self.spec.has_field(&tokens::color_space())
    }

    /// Clears the color space metadata.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::AttributeSpec;
    ///
    /// let mut attr = AttributeSpec::default();
    /// attr.clear_color_space();
    /// ```
    pub fn clear_color_space(&mut self) {
        if self.is_dormant() {
            return;
        }

        let _ = self.spec.clear_field(&tokens::color_space());
    }

    // ========================================================================
    // Display Unit
    // ========================================================================

    /// Returns the display unit for this attribute.
    ///
    /// Display unit indicates how numeric values should be displayed in UI
    /// (e.g., "centimeters", "degrees", "seconds").
    ///
    /// Returns an empty string if not set or the spec is dormant.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::AttributeSpec;
    ///
    /// let attr = AttributeSpec::default();
    /// let unit = attr.display_unit();
    /// ```
    #[must_use]
    pub fn display_unit(&self) -> String {
        if self.is_dormant() {
            return String::new();
        }

        let field = self.spec.get_field(&tokens::display_unit());
        if field.is_empty() {
            return String::new();
        }

        field
            .get::<String>()
            .map(|s| s.as_str())
            .unwrap_or("")
            .to_string()
    }

    /// Sets the display unit for this attribute.
    ///
    /// # Parameters
    ///
    /// - `unit` - The display unit string
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::AttributeSpec;
    ///
    /// let mut attr = AttributeSpec::default();
    /// attr.set_display_unit("centimeters");
    /// ```
    pub fn set_display_unit(&mut self, unit: impl Into<String>) {
        if self.is_dormant() {
            return;
        }

        let value = VtValue::new(unit.into());
        let _ = self.spec.set_field(&tokens::display_unit(), value);
    }

    /// Returns true if display unit metadata is set.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::AttributeSpec;
    ///
    /// let attr = AttributeSpec::default();
    /// assert!(!attr.has_display_unit());
    /// ```
    #[inline]
    #[must_use]
    pub fn has_display_unit(&self) -> bool {
        if self.is_dormant() {
            return false;
        }
        self.spec.has_field(&tokens::display_unit())
    }

    /// Clears the display unit metadata.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::AttributeSpec;
    ///
    /// let mut attr = AttributeSpec::default();
    /// attr.clear_display_unit();
    /// ```
    pub fn clear_display_unit(&mut self) {
        if self.is_dormant() {
            return;
        }

        let _ = self.spec.clear_field(&tokens::display_unit());
    }

    // ========================================================================
    // Time Samples
    // ========================================================================

    /// Returns true if this attribute has time samples.
    ///
    /// Time samples allow attribute values to vary over time (animation).
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::AttributeSpec;
    ///
    /// let attr = AttributeSpec::default();
    /// assert!(!attr.has_time_samples());
    /// ```
    #[must_use]
    pub fn has_time_samples(&self) -> bool {
        if self.is_dormant() {
            return false;
        }

        self.spec.has_field(&tokens::time_samples())
    }

    /// Returns the number of time samples.
    ///
    /// Returns 0 if no time samples are set or the spec is dormant.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::AttributeSpec;
    ///
    /// let attr = AttributeSpec::default();
    /// assert_eq!(attr.num_time_samples(), 0);
    /// ```
    #[must_use]
    pub fn num_time_samples(&self) -> usize {
        if self.is_dormant() {
            return 0;
        }

        // Check spec field first (inline timeSamples map)
        let field = self.spec.get_field(&tokens::time_samples());
        if !field.is_empty() {
            if let Some(count) = extract_time_sample_map(&field).map(|m| m.len()) {
                if count > 0 {
                    return count;
                }
            }
        }

        // Fallback: check layer's time_samples store (used by layer.set_time_sample())
        if let Some(layer) = self.spec.layer().upgrade() {
            let count = layer.get_num_time_samples_for_path(&self.spec.path());
            if count > 0 {
                return count;
            }
        }

        0
    }

    /// Returns all time samples as a map from time to value.
    ///
    /// Returns an empty map if no time samples are set or the spec is dormant.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::AttributeSpec;
    ///
    /// let attr = AttributeSpec::default();
    /// let samples = attr.time_sample_map();
    /// assert!(samples.is_empty());
    /// ```
    #[must_use]
    pub fn time_sample_map(&self) -> HashMap<OrderedFloat, Value> {
        if self.is_dormant() {
            return HashMap::new();
        }

        // Check spec field first
        let field = self.spec.get_field(&tokens::time_samples());
        if !field.is_empty() {
            if let Some(map) = extract_time_sample_map(&field) {
                if !map.is_empty() {
                    return map.into_iter().collect();
                }
            }
        }

        // Fallback: layer's time_samples store
        if let Some(layer) = self.spec.layer().upgrade() {
            let path = self.spec.path();
            let times = layer.list_time_samples_for_path(&path);
            if !times.is_empty() {
                let mut map = HashMap::new();
                for t in &times {
                    if let Some(v) = layer.query_time_sample(&path, *t) {
                        map.insert(OrderedFloat::new(*t), v);
                    }
                }
                return map;
            }
        }

        HashMap::new()
    }

    /// Queries the value at the given time.
    ///
    /// Returns `None` if no sample exists at that time or the spec is dormant.
    ///
    /// # Parameters
    ///
    /// - `time` - The time to query
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::AttributeSpec;
    ///
    /// let attr = AttributeSpec::default();
    /// let value = attr.query_time_sample(1.0);
    /// assert!(value.is_none());
    /// ```
    #[must_use]
    pub fn query_time_sample(&self, time: f64) -> Option<Value> {
        if self.is_dormant() {
            return None;
        }

        // Check spec field first
        let field = self.spec.get_field(&tokens::time_samples());
        if !field.is_empty() {
            if let Some(map) = extract_time_sample_map(&field) {
                if let Some(v) = map.get(&OrderedFloat::new(time)) {
                    return Some(v.clone());
                }
            }
        }

        // Fallback: layer's time_samples store
        if let Some(layer) = self.spec.layer().upgrade() {
            return layer.query_time_sample(&self.spec.path(), time);
        }

        None
    }

    /// Sets a time sample at the given time.
    ///
    /// # Parameters
    ///
    /// - `time` - The time at which to set the value
    /// - `value` - The value to set
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::{AttributeSpec, Value};
    ///
    /// let mut attr = AttributeSpec::default();
    /// attr.set_time_sample(1.0, Value::from_f32(10.0));
    /// attr.set_time_sample(2.0, Value::from_f32(20.0));
    /// ```
    pub fn set_time_sample(&mut self, time: f64, value: Value) {
        if self.is_dormant() {
            return;
        }

        // Get existing time samples or create new map
        let mut samples = self.time_sample_map();

        // Insert the new sample
        samples.insert(OrderedFloat::new(time), value);

        // Convert map to BTreeMap for storage
        let btree: BTreeMap<OrderedFloat, Value> = samples.into_iter().collect();

        // Store as VtValue (placeholder implementation)
        // In a full implementation, this would properly serialize the BTreeMap
        let vt_value = Value::new(btree);
        let vt_value_wrapped = value_to_vt(&vt_value);
        let _ = self
            .spec
            .set_field(&tokens::time_samples(), vt_value_wrapped);
    }

    /// Clears the time sample at the given time.
    ///
    /// # Parameters
    ///
    /// - `time` - The time at which to clear the sample
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::AttributeSpec;
    ///
    /// let mut attr = AttributeSpec::default();
    /// attr.clear_time_sample(1.0);
    /// ```
    pub fn clear_time_sample(&mut self, time: f64) {
        if self.is_dormant() {
            return;
        }

        // Get existing time samples
        let mut samples = self.time_sample_map();

        // Remove the sample at the given time
        samples.remove(&OrderedFloat::new(time));

        // If no samples remain, clear the field entirely
        if samples.is_empty() {
            let _ = self.spec.clear_field(&tokens::time_samples());
        } else {
            // Convert map to BTreeMap for storage
            let btree: BTreeMap<OrderedFloat, Value> = samples.into_iter().collect();

            // Store as VtValue (placeholder implementation)
            let vt_value = Value::new(btree);
            let vt_value_wrapped = value_to_vt(&vt_value);
            let _ = self
                .spec
                .set_field(&tokens::time_samples(), vt_value_wrapped);
        }
    }

    /// Clears all time samples.
    ///
    /// After calling this, `has_time_samples()` will return false.
    pub fn clear_time_samples(&mut self) {
        if self.is_dormant() {
            return;
        }

        let _ = self.spec.clear_field(&tokens::time_samples());
    }

    // ========================================================================
    // Limits API
    // ========================================================================

    /// Returns the limits dictionary for this attribute.
    ///
    /// Limits contain min/max constraints as key-value pairs.
    #[must_use]
    pub fn limits(&self) -> super::VtDictionary {
        if self.is_dormant() {
            return super::VtDictionary::new();
        }
        self.spec
            .get_field(&tokens::limits())
            .as_dictionary()
            .unwrap_or_default()
    }

    /// Sets the limits dictionary for this attribute.
    pub fn set_limits(&mut self, limits: super::VtDictionary) {
        if self.is_dormant() {
            return;
        }
        let value = VtValue::from_dictionary(limits);
        let _ = self.spec.set_field(&tokens::limits(), value);
    }

    /// Returns true if limits metadata is set for this attribute.
    #[must_use]
    pub fn has_limits(&self) -> bool {
        if self.is_dormant() {
            return false;
        }
        self.spec.has_field(&tokens::limits())
    }

    /// Clears the limits metadata for this attribute.
    pub fn clear_limits(&mut self) {
        if self.is_dormant() {
            return;
        }
        let _ = self.spec.clear_field(&tokens::limits());
    }

    // ========================================================================
    // Array Size Constraint API
    // ========================================================================

    /// Returns the array size constraint value for this attribute.
    ///
    /// - 0 (default): dynamic, unrestricted size
    /// - >0: exact fixed size
    /// - <0: abs value is tuple-length; size must be a multiple of it
    #[must_use]
    pub fn array_size_constraint(&self) -> i64 {
        if self.is_dormant() {
            return 0;
        }
        self.spec
            .get_field(&tokens::array_size_constraint())
            .get::<i64>()
            .copied()
            .unwrap_or(0)
    }

    /// Sets the array size constraint value for this attribute.
    pub fn set_array_size_constraint(&mut self, constraint: i64) {
        if self.is_dormant() {
            return;
        }
        let value = VtValue::new(constraint);
        let _ = self.spec.set_field(&tokens::array_size_constraint(), value);
    }

    /// Returns true if this attribute has an array size constraint authored.
    #[must_use]
    pub fn has_array_size_constraint(&self) -> bool {
        if self.is_dormant() {
            return false;
        }
        self.spec.has_field(&tokens::array_size_constraint())
    }

    /// Clears the array size constraint for this attribute.
    pub fn clear_array_size_constraint(&mut self) {
        if self.is_dormant() {
            return;
        }
        let _ = self.spec.clear_field(&tokens::array_size_constraint());
    }

    // ========================================================================
    // Spline API (TsSpline)
    // ========================================================================

    /// Returns true if this attribute has a TsSpline value authored.
    #[must_use]
    pub fn has_spline(&self) -> bool {
        if self.is_dormant() {
            return false;
        }
        self.spec.has_field(&tokens::spline())
    }

    /// Returns the spline at this attribute spec.
    ///
    /// Returns the raw VtValue containing the spline data. Use
    /// `TsSpline` from the `usd-ts` crate for full spline manipulation.
    #[must_use]
    pub fn spline(&self) -> VtValue {
        if self.is_dormant() {
            return VtValue::empty();
        }
        self.spec.get_field(&tokens::spline())
    }

    /// Sets the spline value for this attribute spec.
    pub fn set_spline(&mut self, value: VtValue) {
        if self.is_dormant() {
            return;
        }
        let _ = self.spec.set_field(&tokens::spline(), value);
    }

    /// Clears the spline from this attribute spec.
    pub fn clear_spline(&mut self) {
        if self.is_dormant() {
            return;
        }
        let _ = self.spec.clear_field(&tokens::spline());
    }

    // ========================================================================
    // Time Sample Queries
    // ========================================================================

    /// Returns the sorted set of all time sample times.
    ///
    /// Maps to C++ SdfAttributeSpec::ListTimeSamples().
    #[must_use]
    pub fn list_time_samples(&self) -> std::collections::BTreeSet<OrderedFloat> {
        if self.is_dormant() {
            return std::collections::BTreeSet::new();
        }
        let field = self.spec.get_field(&tokens::time_samples());
        if field.is_empty() {
            return std::collections::BTreeSet::new();
        }
        extract_time_sample_map(&field)
            .map(|btree| btree.keys().copied().collect())
            .unwrap_or_default()
    }

    /// Returns the two bracketing time samples around `time`.
    ///
    /// Maps to C++ SdfAttributeSpec::GetBracketingTimeSamples().
    /// Returns `Some((t_lower, t_upper))` if bracketing samples exist,
    /// or `None` if there are no time samples.
    #[must_use]
    pub fn get_bracketing_time_samples(&self, time: f64) -> Option<(f64, f64)> {
        if self.is_dormant() {
            return None;
        }
        let field = self.spec.get_field(&tokens::time_samples());
        if field.is_empty() {
            return None;
        }
        let btree = extract_time_sample_map(&field)?;
        if btree.is_empty() {
            return None;
        }

        let key = OrderedFloat::new(time);

        // Find lower bound (largest key <= time)
        let lower = btree.range(..=key).next_back().map(|(k, _)| k.value());
        // Find upper bound (smallest key >= time)
        let upper = btree.range(key..).next().map(|(k, _)| k.value());

        match (lower, upper) {
            (Some(lo), Some(hi)) => Some((lo, hi)),
            (Some(lo), None) => Some((lo, lo)),
            (None, Some(hi)) => Some((hi, hi)),
            (None, None) => None,
        }
    }
}

// ============================================================================
// Trait Implementations
// ============================================================================

impl PartialEq for AttributeSpec {
    fn eq(&self, other: &Self) -> bool {
        self.spec == other.spec
    }
}

impl Eq for AttributeSpec {}

impl std::hash::Hash for AttributeSpec {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.spec.hash(state);
    }
}

impl std::fmt::Display for AttributeSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_dormant() {
            write!(f, "<dormant attribute spec>")
        } else {
            write!(f, "<attribute {} at {}>", self.type_name(), self.path())
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attribute_spec_default() {
        let attr = AttributeSpec::default();
        assert!(attr.is_dormant());
        assert!(attr.path().is_empty());
        assert!(!attr.layer().is_valid());
    }

    #[test]
    fn test_attribute_spec_new() {
        let layer = LayerHandle::null();
        let path = Path::from("/World.radius");
        let spec = Spec::new(layer, path.clone());
        let attr = AttributeSpec::new(spec);

        assert!(attr.is_dormant()); // Dormant because layer is null
        assert_eq!(attr.path(), path);
    }

    #[test]
    fn test_attribute_spec_from_layer_and_path() {
        let layer = LayerHandle::null();
        let path = Path::from("/World.radius");
        let attr = AttributeSpec::from_layer_and_path(layer, path.clone());

        assert_eq!(attr.path(), path);
    }

    #[test]
    fn test_attribute_spec_into_spec() {
        let layer = LayerHandle::null();
        let path = Path::from("/World.radius");
        let attr = AttributeSpec::from_layer_and_path(layer, path.clone());
        let spec = attr.into_spec();

        assert_eq!(spec.path(), path);
    }

    #[test]
    fn test_type_name_dormant() {
        let attr = AttributeSpec::default();
        assert_eq!(attr.type_name(), "");
        assert_eq!(attr.role_name(), None);
    }

    #[test]
    fn test_default_value_dormant() {
        let attr = AttributeSpec::default();
        assert!(!attr.has_default_value());

        let _default = attr.default_value();
        // Value is empty/placeholder for dormant spec
    }

    #[test]
    fn test_connection_paths_dormant() {
        let attr = AttributeSpec::default();
        assert!(!attr.has_connection_paths());

        let connections = attr.connection_paths_list();
        assert_eq!(connections, PathListOp::default());
    }

    #[test]
    fn test_allowed_tokens_dormant() {
        let attr = AttributeSpec::default();
        assert!(!attr.has_allowed_tokens());
        assert_eq!(attr.allowed_tokens(), None);
    }

    #[test]
    fn test_color_space_dormant() {
        let attr = AttributeSpec::default();
        assert!(!attr.has_color_space());
        assert_eq!(attr.color_space(), None);
    }

    #[test]
    fn test_display_unit_dormant() {
        let attr = AttributeSpec::default();
        assert!(!attr.has_display_unit());
        assert_eq!(attr.display_unit(), "");
    }

    #[test]
    fn test_time_samples_dormant() {
        let attr = AttributeSpec::default();
        assert!(!attr.has_time_samples());
        assert_eq!(attr.num_time_samples(), 0);

        let samples = attr.time_sample_map();
        assert!(samples.is_empty());

        // Query should return None for dormant spec
        assert!(attr.query_time_sample(1.0).is_none());
    }

    #[test]
    fn test_mutations_dormant() {
        let mut attr = AttributeSpec::default();

        // These should not panic on dormant specs
        attr.set_type_name("float");
        attr.set_default_value(Value::from_f32(42.0));
        attr.clear_default_value();
        attr.clear_connection_paths();
        attr.set_allowed_tokens(vec![Token::new("a"), Token::new("b")]);
        attr.clear_allowed_tokens();
        attr.set_color_space(Token::new("sRGB"));
        attr.clear_color_space();
        attr.set_display_unit("meters");
        attr.clear_display_unit();
        attr.set_time_sample(1.0, Value::from_f32(10.0));
        attr.clear_time_sample(1.0);
        attr.clear_time_samples();
    }

    #[test]
    fn test_attribute_spec_equality() {
        let layer = LayerHandle::null();
        let path1 = Path::from("/World.radius");
        let path2 = Path::from("/World.radius");
        let path3 = Path::from("/World.height");

        let attr1 = AttributeSpec::from_layer_and_path(layer.clone(), path1);
        let attr2 = AttributeSpec::from_layer_and_path(layer.clone(), path2);
        let attr3 = AttributeSpec::from_layer_and_path(layer, path3);

        assert_eq!(attr1, attr2);
        assert_ne!(attr1, attr3);
    }

    #[test]
    fn test_attribute_spec_hash() {
        use std::collections::HashSet;

        let layer = LayerHandle::null();
        let mut set = HashSet::new();

        set.insert(AttributeSpec::from_layer_and_path(
            layer.clone(),
            Path::from("/World.radius"),
        ));
        set.insert(AttributeSpec::from_layer_and_path(
            layer.clone(),
            Path::from("/World.height"),
        ));
        set.insert(AttributeSpec::from_layer_and_path(
            layer,
            Path::from("/World.radius"),
        )); // Duplicate

        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_attribute_spec_display() {
        let dormant = AttributeSpec::default();
        assert_eq!(format!("{}", dormant), "<dormant attribute spec>");

        // Spec with null layer is also dormant
        let attr_null_layer =
            AttributeSpec::from_layer_and_path(LayerHandle::null(), Path::from("/World.radius"));
        assert_eq!(format!("{}", attr_null_layer), "<dormant attribute spec>");
    }

    #[test]
    fn test_as_spec() {
        let layer = LayerHandle::null();
        let path = Path::from("/World.radius");
        let attr = AttributeSpec::from_layer_and_path(layer, path.clone());

        let spec = attr.as_spec();
        assert_eq!(spec.path(), path);
    }

    #[test]
    fn test_as_spec_mut() {
        let layer = LayerHandle::null();
        let path = Path::from("/World.radius");
        let mut attr = AttributeSpec::from_layer_and_path(layer, path);

        let spec_mut = attr.as_spec_mut();
        assert!(spec_mut.is_dormant());
    }

    #[test]
    fn test_clone() {
        let layer = LayerHandle::null();
        let path = Path::from("/World.radius");
        let attr1 = AttributeSpec::from_layer_and_path(layer, path);
        let attr2 = attr1.clone();

        assert_eq!(attr1, attr2);
    }

    // ========================================================================
    // Behavioral tests ported from testSdfAttribute.py
    // ========================================================================

    // Helper: create a live AttributeSpec in a fresh anonymous layer.
    //
    // Returns (layer Arc, PrimSpec, AttributeSpec) so callers keep the layer
    // alive for the duration of the test.
    fn make_attr(
        tag: &str,
        prim_name: &str,
        attr_name: &str,
        type_name: &str,
    ) -> (
        std::sync::Arc<super::super::Layer>,
        super::super::PrimSpec,
        AttributeSpec,
    ) {
        use super::super::{Layer, SpecType, Specifier};

        let layer = Layer::create_anonymous(Some(tag));
        let prim_path = Path::from_string(&format!("/{}", prim_name)).unwrap();
        let prim = layer
            .create_prim_spec(&prim_path, Specifier::Def, "bogus_type")
            .expect("prim creation failed");

        let attr_path = prim_path.append_property(attr_name).unwrap();
        layer.create_spec(&attr_path, SpecType::Attribute);
        layer.set_field(
            &attr_path,
            &usd_tf::Token::new("typeName"),
            super::super::abstract_data::Value::new(type_name.to_string()),
        );

        let attr = layer
            .get_attribute_at_path(&attr_path)
            .expect("attribute not found after creation");

        (layer, prim, attr)
    }

    // ported from test_Creation
    #[test]
    fn test_creation() {
        use super::super::{Layer, PropertySpec, SpecType, Specifier};

        let layer = Layer::create_anonymous(Some("creation_test"));
        let prim_path = Path::from_string("/test").unwrap();
        layer.create_prim_spec(&prim_path, Specifier::Def, "bogus_type");

        // Create an attribute on the prim.
        let attr_path = prim_path.append_property("numCrvs").unwrap();
        layer.create_spec(&attr_path, SpecType::Attribute);
        layer.set_field(
            &attr_path,
            &usd_tf::Token::new("typeName"),
            super::super::abstract_data::Value::new("int".to_string()),
        );

        let attr = layer
            .get_attribute_at_path(&attr_path)
            .expect("attribute not found");

        // Verify name comes from path.
        assert_eq!(attr.path().get_name(), "numCrvs");
        // Verify type_name round-trips.
        assert_eq!(attr.type_name(), "int");
        // Verify path is /test.numCrvs.
        assert_eq!(attr.path().as_str(), "/test.numCrvs");

        // The prim should now report one attribute.
        let prim = layer.get_prim_at_path(&prim_path).unwrap();
        let attrs = prim.attributes();
        assert_eq!(attrs.len(), 1);
        assert_eq!(attrs[0], attr);

        // custom flag default is false (property spec wraps same spec).
        let prop = PropertySpec::new(attr.as_spec().clone());
        assert!(!prop.custom());

        // Creating a duplicate attribute must not succeed — the spec already
        // exists so create_spec is a no-op and the path stays valid.
        let before = prim.attributes().len();
        layer.create_spec(&attr_path, SpecType::Attribute); // duplicate, should be no-op
        assert_eq!(prim.attributes().len(), before);

        // An attribute with an invalid name must not be reachable.
        for bad in &["", "a.b", "a[]", "a/"] {
            // append_property validates the name and returns None for invalid ones.
            let result = prim_path.append_property(bad);
            assert!(
                result.is_none(),
                "expected None for invalid attr name {:?}",
                bad
            );
        }
    }

    // ported from test_Path
    #[test]
    fn test_path() {
        let (_layer, prim, attr) = make_attr("path_test", "test", "numCrvs", "int");
        // Attribute path must be /test.numCrvs.
        let expected = format!("/{}.{}", prim.name(), attr.path().get_name());
        assert_eq!(attr.path().as_str(), expected);
        // Prim path component must match prim path.
        assert_eq!(attr.path().get_prim_path(), prim.path());
    }

    // ported from test_Metadata (comment, documentation, display_group, custom)
    #[test]
    fn test_metadata() {
        use super::super::PropertySpec;

        let (_layer, _prim, attr) = make_attr("metadata_test", "test", "numCrvs", "int");

        // Wrap the same Spec as a PropertySpec to access property metadata.
        let spec = attr.as_spec().clone();
        let mut prop = PropertySpec::new(spec);

        // custom flag: default false, toggle, back to false.
        assert!(!prop.custom());
        prop.set_custom(true);
        assert!(prop.custom());
        prop.set_custom(false);
        assert!(!prop.custom());

        // comment: starts empty, set and clear.
        assert_eq!(prop.comment(), "");
        prop.set_comment("foo");
        assert_eq!(prop.comment(), "foo");
        prop.set_comment("bar");
        assert_eq!(prop.comment(), "bar");
        prop.set_comment("");
        assert_eq!(prop.comment(), "");

        // documentation: starts empty, set and clear.
        assert_eq!(prop.documentation(), "");
        prop.set_documentation("some docs");
        assert_eq!(prop.documentation(), "some docs");
        prop.set_documentation("other docs");
        assert_eq!(prop.documentation(), "other docs");
        prop.set_documentation("");
        assert_eq!(prop.documentation(), "");

        // display_group: starts empty, set and clear.
        assert_eq!(prop.display_group(), "");
        prop.set_display_group("foo");
        assert_eq!(prop.display_group(), "foo");
        prop.set_display_group("bar");
        assert_eq!(prop.display_group(), "bar");
        prop.set_display_group("");
        assert_eq!(prop.display_group(), "");
    }

    // ported from test_Metadata: default value set/query/clear
    #[test]
    fn test_default_value() {
        let (layer, _prim, mut attr) = make_attr("default_val_test", "test", "numCrvs", "int");
        let attr_path = attr.path();

        assert!(!attr.has_default_value());

        // Set an integer default via the layer's field API.
        layer.set_field(
            &attr_path,
            &usd_tf::Token::new("default"),
            Value::new(42i32),
        );
        // Re-fetch so the attribute sees the newly authored field.
        attr = layer.get_attribute_at_path(&attr_path).unwrap();
        assert!(attr.has_default_value());

        // Clear default.
        attr.clear_default_value();
        assert!(!attr.has_default_value());
    }

    // ported from test_Connections
    #[test]
    fn test_connections() {
        let (_layer, _prim, mut attr) = make_attr("connections_test", "test", "numCrvs", "int");

        let conn_token = usd_tf::Token::new("connectionPaths");

        // Initially the connectionPaths field must not exist.
        // Note: has_connection_paths() has a known quirk — it returns true
        // whenever the stored PathListOp is non-explicit (i.e. the default).
        // We use has_field directly to mirror C++ HasInfo('connectionPaths').
        assert!(!attr.as_spec().has_field(&conn_token));
        assert!(attr.connection_paths_list().get_explicit_items().is_empty());

        // Add a connection path.
        let conn1 = Path::from_string("/test.attr").unwrap();
        attr.set_connection_paths_list(PathListOp::create_explicit(vec![conn1.clone()]));

        // After setting an explicit list the field must now exist.
        assert!(attr.as_spec().has_field(&conn_token));
        let fetched = attr.connection_paths_list();
        assert!(fetched.get_explicit_items().contains(&conn1));

        // Add a second connection.
        let conn2 = Path::from_string("/test2.attr").unwrap();
        attr.set_connection_paths_list(PathListOp::create_explicit(vec![
            conn1.clone(),
            conn2.clone(),
        ]));

        let fetched2 = attr.connection_paths_list();
        let items = fetched2.get_explicit_items();
        assert_eq!(items.len(), 2);
        assert!(items.contains(&conn1));
        assert!(items.contains(&conn2));

        // Clear connection paths erases the field entirely.
        attr.clear_connection_paths();
        assert!(!attr.as_spec().has_field(&conn_token));
        assert!(attr.connection_paths_list().get_explicit_items().is_empty());
    }

    // ported from test_TimeSamples
    #[test]
    fn test_time_samples() {
        use super::super::Layer;

        let layer = Layer::create_anonymous(Some("ts_test"));
        let usda = r#"#usda 1.0
def Scope "Scope"
{
    custom double radius = 1.0
    double radius.timeSamples = {
        1.23: 5,
        3.23: 10,
        6: 5,
    }
    custom string desc = ""
    string desc.timeSamples = {
        1.23: "foo",
        3.23: "bar",
        6: "baz",
    }
}
"#;
        assert!(layer.import_from_string(usda));

        let scope_path = Path::from_string("/Scope").unwrap();
        let radius_path = scope_path.append_property("radius").unwrap();
        let desc_path = scope_path.append_property("desc").unwrap();

        // Verify time sample counts loaded from USDA.
        let radius_attr = layer.get_attribute_at_path(&radius_path).unwrap();
        assert_eq!(radius_attr.num_time_samples(), 3);

        // Set a new time sample on radius.
        layer.set_time_sample(&radius_path, 4.0, Value::from_f64(2.0));
        // QueryTimeSample via layer.
        let queried = layer.query_time_sample(&radius_path, 4.0);
        assert!(queried.is_some());

        // desc: set a new sample, verify count, erase it, verify count.
        let desc_attr = layer.get_attribute_at_path(&desc_path).unwrap();
        assert_eq!(desc_attr.num_time_samples(), 3);

        layer.set_time_sample(&desc_path, 10.0, Value::new("boom".to_string()));
        let desc_attr2 = layer.get_attribute_at_path(&desc_path).unwrap();
        assert_eq!(desc_attr2.num_time_samples(), 4);

        // list_time_samples on the attribute reads the inline timeSamples field.
        // Samples set via Layer::set_time_sample go into the layer's dedicated
        // store, not the inline field, so we list via the layer instead.
        let mut times = layer.list_time_samples_for_path(&desc_path);
        times.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(times, vec![1.23, 3.23, 6.0, 10.0]);

        // The layer store now holds 4 samples; verify via num_time_samples.
        let desc_attr3 = layer.get_attribute_at_path(&desc_path).unwrap();
        assert_eq!(desc_attr3.num_time_samples(), 4);
    }

    // ported from test_Inertness
    #[test]
    fn test_inertness() {
        use super::super::PropertySpec;

        let (_layer, _prim, attr) = make_attr("inert_test", "test", "testAttr", "int");

        // A freshly created attribute has only required fields (typeName).
        // has_only_required_fields checks via PropertySpec.
        let prop = PropertySpec::new(attr.as_spec().clone());
        // The attribute was just created with typeName set; no default, no
        // extra metadata beyond the minimum — has_only_required_fields is true.
        assert!(prop.has_only_required_fields());

        // Setting documentation (a non-required field) makes it non-inert.
        let mut prop_mut = PropertySpec::new(attr.as_spec().clone());
        prop_mut.set_documentation("some docs");
        assert!(!prop_mut.has_only_required_fields());

        // Clearing it brings us back to required-fields only.
        prop_mut.set_documentation("");
        assert!(prop_mut.has_only_required_fields());
        // Note: PropertySpec::has_only_required_fields does not currently
        // account for connectionPaths, so that aspect of the C++ inertness
        // test (adding connections → non-inert) is not yet mirrored here.
    }

    // ported from test_Metadata: array_size_constraint
    #[test]
    fn test_array_size_constraint() {
        let (_layer, _prim, mut attr) = make_attr("array_sz_test", "test", "numCrvs", "int");

        // Default: no constraint authored.
        assert!(!attr.has_array_size_constraint());
        assert_eq!(attr.array_size_constraint(), 0);

        // Set a positive constraint.
        attr.set_array_size_constraint(10);
        assert_eq!(attr.array_size_constraint(), 10);
        assert!(attr.has_array_size_constraint());

        // Set a negative constraint (tuple-length semantics).
        attr.set_array_size_constraint(-10);
        assert_eq!(attr.array_size_constraint(), -10);
        assert!(attr.has_array_size_constraint());

        // Clear.
        attr.clear_array_size_constraint();
        assert_eq!(attr.array_size_constraint(), 0);
        assert!(!attr.has_array_size_constraint());
    }

    // ported from test_Metadata: variability
    #[test]
    fn test_variability() {
        use super::super::Variability;

        let (_layer, _prim, mut attr) = make_attr("variability_test", "test", "numCrvs", "int");

        // Default variability is Varying.
        assert_eq!(attr.variability(), Variability::Varying);

        attr.set_variability(Variability::Uniform);
        assert_eq!(attr.variability(), Variability::Uniform);

        attr.set_variability(Variability::Varying);
        assert_eq!(attr.variability(), Variability::Varying);
    }

    // ported from test_Metadata: allowed_tokens, color_space, display_unit
    #[test]
    fn test_attribute_metadata_fields() {
        use usd_tf::Token;

        let (_layer, _prim, mut attr) = make_attr("attr_meta_test", "test", "vis", "token");

        // allowed_tokens: none initially.
        assert!(!attr.has_allowed_tokens());
        assert_eq!(attr.allowed_tokens(), None);

        let toks = vec![Token::new("inherited"), Token::new("invisible")];
        attr.set_allowed_tokens(toks.clone());
        assert!(attr.has_allowed_tokens());
        let got = attr.allowed_tokens().unwrap();
        assert_eq!(got, toks);

        attr.clear_allowed_tokens();
        assert!(!attr.has_allowed_tokens());

        // color_space: none initially.
        assert!(!attr.has_color_space());
        attr.set_color_space(Token::new("sRGB"));
        assert!(attr.has_color_space());
        assert_eq!(attr.color_space().unwrap().as_str(), "sRGB");
        attr.clear_color_space();
        assert!(!attr.has_color_space());

        // display_unit: none initially.
        assert!(!attr.has_display_unit());
        attr.set_display_unit("centimeters");
        assert_eq!(attr.display_unit(), "centimeters");
        attr.clear_display_unit();
        assert_eq!(attr.display_unit(), "");
    }
}
