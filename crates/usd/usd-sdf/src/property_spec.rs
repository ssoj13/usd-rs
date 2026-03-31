//! PropertySpec - base class for AttributeSpec and RelationshipSpec.
//!
//! `PropertySpec` is the abstract base class for scene description properties.
//! It provides the common interface for both attributes and relationships,
//! which are the basic properties that make up prims.
//!
//! # Common Property Features
//!
//! - Name and ownership information
//! - Custom flag for pipeline-specific properties
//! - Variability (varying, uniform, etc.)
//! - Metadata (comment, documentation, display hints)
//! - Permissions and visibility
//! - Custom data and asset info
//! - UI hints (deprecated but still supported)
//!
//! # Examples
//!
//! ```
//! use usd_sdf::{PropertySpec, Spec, Variability, Permission};
//!
//! // PropertySpec is abstract - use AttributeSpec or RelationshipSpec
//! // This shows the common interface available to both
//! ```

use std::sync::OnceLock;

use usd_tf::Token;

use super::{Permission, Spec, Variability, VtDictionary, VtValue};

// Cached tokens for property field names
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

    cached_token!(custom, "custom");
    cached_token!(variability, "variability");
    cached_token!(comment, "comment");
    cached_token!(documentation, "documentation");
    cached_token!(hidden, "hidden");
    cached_token!(display_name, "displayName");
    cached_token!(display_group, "displayGroup");
    cached_token!(permission, "permission");
    cached_token!(prefix, "prefix");
    cached_token!(suffix, "suffix");
    cached_token!(symmetric_peer, "symmetricPeer");
    cached_token!(custom_data, "customData");
    cached_token!(asset_info, "assetInfo");
    cached_token!(symmetry_arguments, "symmetryArguments");
    cached_token!(symmetry_function, "symmetryFunction");
    cached_token!(type_name, "typeName");
    cached_token!(default_value, "default");
}

// ============================================================================
// PropertySpec - Base for AttributeSpec and RelationshipSpec
// ============================================================================

/// Base class for property specs (attributes and relationships).
///
/// `PropertySpec` provides common functionality for both `AttributeSpec`
/// and `RelationshipSpec`. It wraps a `Spec` and provides property-specific
/// accessors for metadata and state.
///
/// # Thread Safety
///
/// PropertySpec is not thread-safe for mutation but can be read from
/// multiple threads if the underlying layer is not being modified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertySpec {
    /// The underlying spec
    spec: Spec,
}

impl PropertySpec {
    // ========================================================================
    // Construction
    // ========================================================================

    /// Create a PropertySpec wrapping the given Spec.
    ///
    /// # Parameters
    ///
    /// - `spec` - The underlying Spec to wrap
    ///
    /// # Note
    ///
    /// This does not validate that the spec is actually a property spec.
    /// Callers should ensure the spec type is appropriate.
    #[must_use]
    pub fn new(spec: Spec) -> Self {
        Self { spec }
    }

    /// Create a dormant (invalid) PropertySpec.
    #[must_use]
    pub fn dormant() -> Self {
        Self {
            spec: Spec::dormant(),
        }
    }

    /// Get a reference to the underlying Spec.
    #[must_use]
    pub fn spec(&self) -> &Spec {
        &self.spec
    }

    /// Get a mutable reference to the underlying Spec.
    #[must_use]
    pub fn spec_mut(&mut self) -> &mut Spec {
        &mut self.spec
    }

    /// Unwrap and return the underlying Spec.
    #[must_use]
    pub fn into_spec(self) -> Spec {
        self.spec
    }

    /// Try to convert this property to an AttributeSpec.
    ///
    /// Returns `Some(AttributeSpec)` if this property is an attribute,
    /// `None` if it's a relationship or invalid.
    #[must_use]
    pub fn as_attribute(&self) -> Option<super::AttributeSpec> {
        use super::SpecType;
        if self.spec.spec_type() == SpecType::Attribute {
            Some(super::AttributeSpec::new(self.spec.clone()))
        } else {
            None
        }
    }

    /// Try to convert this property to a RelationshipSpec.
    ///
    /// Returns `Some(RelationshipSpec)` if this property is a relationship,
    /// `None` if it's an attribute or invalid.
    #[must_use]
    pub fn as_relationship(&self) -> Option<super::RelationshipSpec> {
        use super::SpecType;
        if self.spec.spec_type() == SpecType::Relationship {
            Some(super::RelationshipSpec::from_spec(self.spec.clone()))
        } else {
            None
        }
    }

    /// Check if this property is an attribute.
    #[must_use]
    pub fn is_attribute(&self) -> bool {
        self.spec.spec_type() == super::SpecType::Attribute
    }

    /// Check if this property is a relationship.
    #[must_use]
    pub fn is_relationship(&self) -> bool {
        self.spec.spec_type() == super::SpecType::Relationship
    }

    // ========================================================================
    // Name and Ownership
    // ========================================================================

    /// Returns the property's name as a token.
    ///
    /// The name is the final component of the property's path.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::{PropertySpec, Path, LayerHandle, Spec};
    ///
    /// let spec = Spec::new(LayerHandle::null(), Path::from("/Prim.attr"));
    /// let prop = PropertySpec::new(spec);
    /// // In a real implementation with a valid layer:
    /// // assert_eq!(prop.name(), Token::new("attr"));
    /// ```
    #[must_use]
    pub fn name(&self) -> Token {
        // Get the name from the path
        let path = self.spec.path();
        if path.is_property_path() {
            Token::new(path.get_name())
        } else {
            Token::empty()
        }
    }

    /// Returns the owner of this property (placeholder).
    ///
    /// Returns the prim spec that owns this property. Currently returns
    /// None as a placeholder until PrimSpec is implemented.
    ///
    /// # Note
    ///
    /// This will be implemented properly when PrimSpec is available.
    #[must_use]
    pub fn owner(&self) -> Option<Spec> {
        if self.spec.is_dormant() {
            return None;
        }
        // Get parent prim spec
        let prim_path = self.spec.path().get_prim_path();
        if prim_path.is_empty() {
            return None;
        }
        Some(Spec::new(self.spec.layer(), prim_path))
    }

    // ========================================================================
    // Custom Property Flag
    // ========================================================================

    /// Returns true if this spec declares a custom property.
    ///
    /// Custom properties are user-defined properties not part of a schema.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::PropertySpec;
    ///
    /// let prop = PropertySpec::dormant();
    /// assert!(!prop.custom());
    /// ```
    #[must_use]
    pub fn custom(&self) -> bool {
        self.spec
            .get_field(&tokens::custom())
            .get::<bool>()
            .copied()
            .unwrap_or(false)
    }

    /// Sets whether this spec declares a custom property.
    ///
    /// # Parameters
    ///
    /// - `custom` - True if this is a custom property
    pub fn set_custom(&mut self, custom: bool) {
        if self.spec.is_dormant() {
            return;
        }
        let value = VtValue::new(custom);
        let _ = self.spec.set_field(&tokens::custom(), value);
    }

    // ========================================================================
    // Variability
    // ========================================================================

    /// Returns the variability of the property.
    ///
    /// Variability determines whether the property can be animated:
    /// - `Varying` - Can be animated and time-varying (default)
    /// - `Uniform` - Cannot be animated, only default values
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::{PropertySpec, Variability};
    ///
    /// let prop = PropertySpec::dormant();
    /// // Default is Varying
    /// assert_eq!(prop.variability(), Variability::Varying);
    /// ```
    #[must_use]
    pub fn variability(&self) -> Variability {
        if self.spec.is_dormant() {
            return Variability::default();
        }
        self.spec
            .get_field(&tokens::variability())
            .get::<Variability>()
            .copied()
            .unwrap_or_default()
    }

    /// Sets the variability of the property.
    ///
    /// # Parameters
    ///
    /// - `variability` - The variability to set
    pub fn set_variability(&mut self, variability: Variability) {
        if self.spec.is_dormant() {
            return;
        }
        let value = VtValue::new(variability);
        let _ = self.spec.set_field(&tokens::variability(), value);
    }

    // ========================================================================
    // Metadata - Comment and Documentation
    // ========================================================================

    /// Returns the comment string for this property spec.
    ///
    /// The default value for comment is empty string.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::PropertySpec;
    ///
    /// let prop = PropertySpec::dormant();
    /// assert_eq!(prop.comment(), "");
    /// ```
    #[must_use]
    pub fn comment(&self) -> String {
        if self.spec.is_dormant() {
            return String::new();
        }
        self.spec
            .get_field(&tokens::comment())
            .get::<String>()
            .cloned()
            .unwrap_or_default()
    }

    /// Sets the comment string for this property spec.
    ///
    /// # Parameters
    ///
    /// - `value` - The comment text
    pub fn set_comment(&mut self, value: impl Into<String>) {
        if self.spec.is_dormant() {
            return;
        }
        let value = VtValue::new(value.into());
        let _ = self.spec.set_field(&tokens::comment(), value);
    }

    /// Returns the documentation string for this property spec.
    ///
    /// The default value for documentation is empty string.
    #[must_use]
    pub fn documentation(&self) -> String {
        if self.spec.is_dormant() {
            return String::new();
        }
        self.spec
            .get_field(&tokens::documentation())
            .get::<String>()
            .cloned()
            .unwrap_or_default()
    }

    /// Sets the documentation string for this property spec.
    ///
    /// # Parameters
    ///
    /// - `value` - The documentation text
    pub fn set_documentation(&mut self, value: impl Into<String>) {
        if self.spec.is_dormant() {
            return;
        }
        let value = VtValue::new(value.into());
        let _ = self.spec.set_field(&tokens::documentation(), value);
    }

    // ========================================================================
    // Metadata - Display Hints (Deprecated)
    // ========================================================================

    /// Returns whether this property spec will be hidden in browsers.
    ///
    /// The default value for hidden is false.
    ///
    /// # Deprecated
    ///
    /// See UsdUIObjectHints for the current approach to UI hints.
    #[must_use]
    pub fn hidden(&self) -> bool {
        if self.spec.is_dormant() {
            return false;
        }
        self.spec
            .get_field(&tokens::hidden())
            .get::<bool>()
            .copied()
            .unwrap_or(false)
    }

    /// Sets whether this property spec will be hidden in browsers.
    ///
    /// # Parameters
    ///
    /// - `value` - True to hide the property
    ///
    /// # Deprecated
    ///
    /// See UsdUIObjectHints for the current approach to UI hints.
    pub fn set_hidden(&mut self, value: bool) {
        if self.spec.is_dormant() {
            return;
        }
        let value = VtValue::new(value);
        let _ = self.spec.set_field(&tokens::hidden(), value);
    }

    /// Returns the display name string for this property spec.
    ///
    /// The default value for display name is empty string.
    ///
    /// # Deprecated
    ///
    /// See UsdUIObjectHints for the current approach to UI hints.
    #[must_use]
    pub fn display_name(&self) -> String {
        if self.spec.is_dormant() {
            return String::new();
        }
        self.spec
            .get_field(&tokens::display_name())
            .get::<String>()
            .cloned()
            .unwrap_or_default()
    }

    /// Sets the display name string for this property spec.
    ///
    /// # Parameters
    ///
    /// - `value` - The display name
    ///
    /// # Deprecated
    ///
    /// See UsdUIObjectHints for the current approach to UI hints.
    pub fn set_display_name(&mut self, value: impl Into<String>) {
        if self.spec.is_dormant() {
            return;
        }
        let value = VtValue::new(value.into());
        let _ = self.spec.set_field(&tokens::display_name(), value);
    }

    /// Returns the display group string for this property spec.
    ///
    /// The default value for display group is empty string.
    ///
    /// # Deprecated
    ///
    /// See UsdUIPropertyHints for the current approach to UI hints.
    #[must_use]
    pub fn display_group(&self) -> String {
        if self.spec.is_dormant() {
            return String::new();
        }
        self.spec
            .get_field(&tokens::display_group())
            .get::<String>()
            .cloned()
            .unwrap_or_default()
    }

    /// Sets the display group string for this property spec.
    ///
    /// # Parameters
    ///
    /// - `value` - The display group
    ///
    /// # Deprecated
    ///
    /// See UsdUIPropertyHints for the current approach to UI hints.
    pub fn set_display_group(&mut self, value: impl Into<String>) {
        if self.spec.is_dormant() {
            return;
        }
        let value = VtValue::new(value.into());
        let _ = self.spec.set_field(&tokens::display_group(), value);
    }

    // ========================================================================
    // Permission
    // ========================================================================

    /// Returns the property's permission restriction.
    ///
    /// The default value for permission is `Permission::Public`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::{PropertySpec, Permission};
    ///
    /// let prop = PropertySpec::dormant();
    /// assert_eq!(prop.permission(), Permission::Public);
    /// ```
    #[must_use]
    pub fn permission(&self) -> Permission {
        if self.spec.is_dormant() {
            return Permission::default();
        }
        self.spec
            .get_field(&tokens::permission())
            .get::<Permission>()
            .copied()
            .unwrap_or_default()
    }

    /// Sets the property's permission restriction.
    ///
    /// # Parameters
    ///
    /// - `value` - The permission level to set
    pub fn set_permission(&mut self, value: Permission) {
        if self.spec.is_dormant() {
            return;
        }
        let value = VtValue::new(value);
        let _ = self.spec.set_field(&tokens::permission(), value);
    }

    // ========================================================================
    // Prefix and Suffix
    // ========================================================================

    /// Returns the prefix string for this property spec.
    ///
    /// The default value for prefix is empty string.
    #[must_use]
    pub fn prefix(&self) -> String {
        if self.spec.is_dormant() {
            return String::new();
        }
        self.spec
            .get_field(&tokens::prefix())
            .get::<String>()
            .cloned()
            .unwrap_or_default()
    }

    /// Sets the prefix string for this property spec.
    ///
    /// # Parameters
    ///
    /// - `value` - The prefix string
    pub fn set_prefix(&mut self, value: impl Into<String>) {
        if self.spec.is_dormant() {
            return;
        }
        let value = VtValue::new(value.into());
        let _ = self.spec.set_field(&tokens::prefix(), value);
    }

    /// Returns the suffix string for this property spec.
    ///
    /// The default value for suffix is empty string.
    #[must_use]
    pub fn suffix(&self) -> String {
        if self.spec.is_dormant() {
            return String::new();
        }
        self.spec
            .get_field(&tokens::suffix())
            .get::<String>()
            .cloned()
            .unwrap_or_default()
    }

    /// Sets the suffix string for this property spec.
    ///
    /// # Parameters
    ///
    /// - `value` - The suffix string
    pub fn set_suffix(&mut self, value: impl Into<String>) {
        if self.spec.is_dormant() {
            return;
        }
        let value = VtValue::new(value.into());
        let _ = self.spec.set_field(&tokens::suffix(), value);
    }

    // ========================================================================
    // Symmetric Peer
    // ========================================================================

    /// Returns the property's symmetric peer.
    ///
    /// The default value for the symmetric peer is empty string.
    ///
    /// Symmetric peers are used for properties that represent symmetric
    /// relationships, where setting one property should update its peer.
    #[must_use]
    pub fn symmetric_peer(&self) -> String {
        if self.spec.is_dormant() {
            return String::new();
        }
        self.spec
            .get_field(&tokens::symmetric_peer())
            .get::<String>()
            .cloned()
            .unwrap_or_default()
    }

    /// Sets the property's symmetric peer.
    ///
    /// If `peer_name` is empty, this removes any symmetric peer for the
    /// given property.
    ///
    /// # Parameters
    ///
    /// - `peer_name` - The name of the symmetric peer property
    pub fn set_symmetric_peer(&mut self, peer_name: impl Into<String>) {
        if self.spec.is_dormant() {
            return;
        }
        let peer_name = peer_name.into();
        if peer_name.is_empty() {
            let _ = self.spec.clear_field(&tokens::symmetric_peer());
        } else {
            let value = VtValue::new(peer_name);
            let _ = self.spec.set_field(&tokens::symmetric_peer(), value);
        }
    }

    // ========================================================================
    // Custom Data and Asset Info
    // ========================================================================

    /// Returns the property's custom data.
    ///
    /// The default value for custom data is an empty dictionary.
    ///
    /// Custom data is for use by plugins or other non-tools supplied
    /// extensions that need to store data attached to arbitrary scene objects.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::PropertySpec;
    ///
    /// let prop = PropertySpec::dormant();
    /// let custom_data = prop.custom_data();
    /// assert!(custom_data.is_empty());
    /// ```
    #[must_use]
    pub fn custom_data(&self) -> VtDictionary {
        self.spec.custom_data()
    }

    /// Sets a property custom data entry.
    ///
    /// If `value` is empty, this removes the given custom data entry.
    ///
    /// # Parameters
    ///
    /// - `name` - The key in the custom data dictionary
    /// - `value` - The value to set (empty to remove)
    pub fn set_custom_data(&mut self, name: impl Into<String>, value: VtValue) {
        if self.spec.is_dormant() {
            return;
        }
        let name = name.into();
        self.spec
            .set_info_dict_value(&tokens::custom_data(), &name, value);
    }

    /// Returns the asset info dictionary for this property.
    ///
    /// The default value is an empty dictionary.
    ///
    /// The asset info dictionary is used to annotate SdfAssetPath-valued
    /// attributes with various data related to asset management.
    ///
    /// # Note
    ///
    /// It is only valid to author assetInfo on attributes that are of
    /// type SdfAssetPath.
    #[must_use]
    pub fn asset_info(&self) -> VtDictionary {
        self.spec.asset_info()
    }

    /// Sets an asset info entry for this property.
    ///
    /// If `value` is empty, this removes the given asset info entry.
    ///
    /// # Parameters
    ///
    /// - `name` - The key in the asset info dictionary
    /// - `value` - The value to set (empty to remove)
    pub fn set_asset_info(&mut self, name: impl Into<String>, value: VtValue) {
        if self.spec.is_dormant() {
            return;
        }
        let name = name.into();
        self.spec
            .set_info_dict_value(&tokens::asset_info(), &name, value);
    }

    // ========================================================================
    // Name Mutation
    // ========================================================================

    /// Returns true if setting the property's name to `new_name` will succeed.
    ///
    /// Returns false if it won't, and sets `why_not` with a description.
    pub fn can_set_name(&self, new_name: &str, why_not: &mut String) -> bool {
        if self.spec.is_dormant() {
            *why_not = "spec is dormant".to_string();
            return false;
        }
        if !Self::is_valid_name(new_name) {
            *why_not = format!("'{}' is not a valid property name", new_name);
            return false;
        }
        // Check for sibling property with same name
        let layer = self.spec.layer();
        if layer.is_valid() {
            let prim_path = self.spec.path().get_prim_path();
            if let Some(sibling_path) = prim_path.append_property(new_name) {
                if layer.has_spec(&sibling_path) && sibling_path != self.spec.path() {
                    *why_not = format!("a sibling property named '{}' already exists", new_name);
                    return false;
                }
            }
        }
        true
    }

    /// Sets the property's name.
    ///
    /// Returns true if successful. Setting `validate` to false skips
    /// the `can_set_name` check.
    pub fn set_name(&mut self, new_name: &str, validate: bool) -> bool {
        if self.spec.is_dormant() {
            return false;
        }
        if validate {
            let mut why_not = String::new();
            if !self.can_set_name(new_name, &mut why_not) {
                return false;
            }
        }
        let layer = self.spec.layer();
        if !layer.is_valid() {
            return false;
        }
        let old_path = self.spec.path();
        let prim_path = old_path.get_prim_path();
        let new_path = match prim_path.append_property(new_name) {
            Some(p) => p,
            None => return false,
        };
        if let Some(layer_arc) = layer.upgrade() {
            if layer_arc.move_spec(&old_path, &new_path) {
                self.spec = Spec::new(layer, new_path);
                return true;
            }
        }
        false
    }

    /// Returns true if the given name is a valid property name.
    ///
    /// Valid names are non-empty, don't start with a digit, and don't
    /// contain invalid characters ('/', '[', ']').
    #[must_use]
    pub fn is_valid_name(name: &str) -> bool {
        if name.is_empty() {
            return false;
        }
        let first = name.chars().next().expect("checked non-empty");
        if first.is_ascii_digit() {
            return false;
        }
        // Disallow path-significant characters
        !name.contains('/') && !name.contains('[') && !name.contains(']')
    }

    // ========================================================================
    // Symmetry
    // ========================================================================

    /// Returns the property's symmetry arguments dictionary.
    ///
    /// Default value is an empty dictionary.
    #[must_use]
    pub fn symmetry_arguments(&self) -> VtDictionary {
        let value = self.spec.get_field(&tokens::symmetry_arguments());
        value.as_dictionary().unwrap_or_default()
    }

    /// Sets a property symmetry argument.
    ///
    /// If `value` is empty, removes the argument with the given `name`.
    pub fn set_symmetry_argument(&mut self, name: &str, value: VtValue) {
        self.spec
            .set_info_dict_value(&tokens::symmetry_arguments(), name, value);
    }

    /// Returns the property's symmetry function.
    ///
    /// Default value is an empty token.
    #[must_use]
    pub fn symmetry_function(&self) -> Token {
        if self.spec.is_dormant() {
            return Token::empty();
        }
        self.spec
            .get_field(&tokens::symmetry_function())
            .get::<String>()
            .map(|s| Token::new(s))
            .unwrap_or_else(Token::empty)
    }

    /// Sets the property's symmetry function.
    ///
    /// If `function_name` is empty, removes any symmetry function.
    pub fn set_symmetry_function(&mut self, function_name: &Token) {
        if self.spec.is_dormant() {
            return;
        }
        if function_name.is_empty() {
            let _ = self.spec.clear_field(&tokens::symmetry_function());
        } else {
            let value = VtValue::new(function_name.as_str().to_string());
            let _ = self.spec.set_field(&tokens::symmetry_function(), value);
        }
    }

    // ========================================================================
    // Property Value API
    // ========================================================================

    /// Returns the name of the value type this property holds.
    ///
    /// Maps to C++ SdfPropertySpec::GetTypeName(). Returns the type name
    /// token (e.g. "float", "double3", "token").
    #[must_use]
    pub fn type_name(&self) -> Token {
        if self.spec.is_dormant() {
            return Token::empty();
        }
        let field = self.spec.get_field(&tokens::type_name());
        field
            .get::<String>()
            .map(|s| Token::new(s))
            .unwrap_or_else(Token::empty)
    }

    /// Returns the property's default value.
    ///
    /// If no default is set, returns an empty VtValue.
    #[must_use]
    pub fn default_value(&self) -> VtValue {
        if self.spec.is_dormant() {
            return VtValue::empty();
        }
        self.spec.get_field(&tokens::default_value())
    }

    /// Returns true if a default value is set for this property.
    #[must_use]
    pub fn has_default_value(&self) -> bool {
        if self.spec.is_dormant() {
            return false;
        }
        self.spec.has_field(&tokens::default_value())
    }

    /// Sets the property's default value.
    ///
    /// Returns true if successful, false if the value has the wrong type.
    pub fn set_default_value(&mut self, default_value: VtValue) -> bool {
        if self.spec.is_dormant() {
            return false;
        }
        self.spec.set_field(&tokens::default_value(), default_value)
    }

    /// Clears the property's default value.
    pub fn clear_default_value(&mut self) {
        if self.spec.is_dormant() {
            return;
        }
        let _ = self.spec.clear_field(&tokens::default_value());
    }

    /// Returns true if this property has no significant data other than
    /// what is necessary for instantiation.
    ///
    /// For example, "double foo" has only required fields, but
    /// "double foo = 3" has more than just what is required.
    #[must_use]
    pub fn has_only_required_fields(&self) -> bool {
        if self.spec.is_dormant() {
            return true;
        }
        // A property has only required fields if it has no default value,
        // no time samples, no connections/targets, and no other metadata
        // beyond the minimum (typeName, variability, custom).
        !self.has_default_value()
            && self.comment().is_empty()
            && self.documentation().is_empty()
            && !self.hidden()
            && self.display_name().is_empty()
            && self.display_group().is_empty()
            && self.symmetric_peer().is_empty()
            && self.custom_data().is_empty()
            && self.asset_info().is_empty()
            && self.symmetry_arguments().is_empty()
            && self.symmetry_function().is_empty()
    }

    // ========================================================================
    // State Queries
    // ========================================================================

    /// Returns true if this spec is dormant (invalid or expired).
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::PropertySpec;
    ///
    /// let prop = PropertySpec::dormant();
    /// assert!(prop.is_dormant());
    /// ```
    #[must_use]
    pub fn is_dormant(&self) -> bool {
        self.spec.is_dormant()
    }
}

// ============================================================================
// AsRef Implementation
// ============================================================================

impl AsRef<Spec> for PropertySpec {
    fn as_ref(&self) -> &Spec {
        &self.spec
    }
}

impl AsMut<Spec> for PropertySpec {
    fn as_mut(&mut self) -> &mut Spec {
        &mut self.spec
    }
}

// ============================================================================
// From/Into Implementations
// ============================================================================

impl From<Spec> for PropertySpec {
    fn from(spec: Spec) -> Self {
        Self::new(spec)
    }
}

impl From<PropertySpec> for Spec {
    fn from(prop: PropertySpec) -> Self {
        prop.into_spec()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LayerHandle, Path};

    #[test]
    fn test_property_spec_dormant() {
        let prop = PropertySpec::dormant();
        assert!(prop.is_dormant());
        assert_eq!(prop.name(), Token::empty());
        assert!(prop.owner().is_none());
    }

    #[test]
    fn test_property_spec_new() {
        let spec = Spec::new(LayerHandle::null(), Path::from("/Prim.attr"));
        let prop = PropertySpec::new(spec);
        assert!(prop.is_dormant()); // Null layer = dormant
    }

    #[test]
    fn test_property_spec_name() {
        let spec = Spec::new(LayerHandle::null(), Path::from("/World/Cube.visibility"));
        let prop = PropertySpec::new(spec);
        // Name extraction from path
        assert_eq!(prop.name().as_str(), "visibility");
    }

    #[test]
    fn test_custom_flag() {
        let mut prop = PropertySpec::dormant();
        assert!(!prop.custom());

        // Setting on dormant spec should not panic
        prop.set_custom(true);
        assert!(!prop.custom()); // Still false because dormant
    }

    #[test]
    fn test_variability() {
        let mut prop = PropertySpec::dormant();
        assert_eq!(prop.variability(), Variability::Varying);

        // Setting on dormant spec should not panic
        prop.set_variability(Variability::Uniform);
        assert_eq!(prop.variability(), Variability::Varying); // Unchanged
    }

    #[test]
    fn test_comment() {
        let mut prop = PropertySpec::dormant();
        assert_eq!(prop.comment(), "");

        prop.set_comment("Test comment");
        assert_eq!(prop.comment(), ""); // Dormant, no change
    }

    #[test]
    fn test_documentation() {
        let mut prop = PropertySpec::dormant();
        assert_eq!(prop.documentation(), "");

        prop.set_documentation("Test docs");
        assert_eq!(prop.documentation(), ""); // Dormant, no change
    }

    #[test]
    fn test_hidden() {
        let mut prop = PropertySpec::dormant();
        assert!(!prop.hidden());

        prop.set_hidden(true);
        assert!(!prop.hidden()); // Dormant, no change
    }

    #[test]
    fn test_display_name() {
        let mut prop = PropertySpec::dormant();
        assert_eq!(prop.display_name(), "");

        prop.set_display_name("Display Name");
        assert_eq!(prop.display_name(), ""); // Dormant, no change
    }

    #[test]
    fn test_display_group() {
        let mut prop = PropertySpec::dormant();
        assert_eq!(prop.display_group(), "");

        prop.set_display_group("Transform");
        assert_eq!(prop.display_group(), ""); // Dormant, no change
    }

    #[test]
    fn test_permission() {
        let mut prop = PropertySpec::dormant();
        assert_eq!(prop.permission(), Permission::Public);

        prop.set_permission(Permission::Private);
        assert_eq!(prop.permission(), Permission::Public); // Dormant, no change
    }

    #[test]
    fn test_prefix() {
        let mut prop = PropertySpec::dormant();
        assert_eq!(prop.prefix(), "");

        prop.set_prefix("pre_");
        assert_eq!(prop.prefix(), ""); // Dormant, no change
    }

    #[test]
    fn test_suffix() {
        let mut prop = PropertySpec::dormant();
        assert_eq!(prop.suffix(), "");

        prop.set_suffix("_post");
        assert_eq!(prop.suffix(), ""); // Dormant, no change
    }

    #[test]
    fn test_symmetric_peer() {
        let mut prop = PropertySpec::dormant();
        assert_eq!(prop.symmetric_peer(), "");

        prop.set_symmetric_peer("peer_prop");
        assert_eq!(prop.symmetric_peer(), ""); // Dormant, no change

        // Test clearing
        prop.set_symmetric_peer("");
        assert_eq!(prop.symmetric_peer(), "");
    }

    #[test]
    fn test_custom_data() {
        let prop = PropertySpec::dormant();
        let custom_data = prop.custom_data();
        assert!(custom_data.is_empty());
    }

    #[test]
    fn test_set_custom_data() {
        let mut prop = PropertySpec::dormant();
        prop.set_custom_data("key", VtValue::new("value"));
        // Dormant spec, no effect
        assert!(prop.custom_data().is_empty());
    }

    #[test]
    fn test_asset_info() {
        let prop = PropertySpec::dormant();
        let asset_info = prop.asset_info();
        assert!(asset_info.is_empty());
    }

    #[test]
    fn test_set_asset_info() {
        let mut prop = PropertySpec::dormant();
        prop.set_asset_info("identifier", VtValue::new("asset_id"));
        // Dormant spec, no effect
        assert!(prop.asset_info().is_empty());
    }

    #[test]
    fn test_as_ref_spec() {
        let spec = Spec::dormant();
        let prop = PropertySpec::new(spec.clone());
        assert_eq!(prop.as_ref(), &spec);
    }

    #[test]
    fn test_as_mut_spec() {
        let spec = Spec::dormant();
        let mut prop = PropertySpec::new(spec);
        let spec_mut = prop.as_mut();
        assert!(spec_mut.is_dormant());
    }

    #[test]
    fn test_from_spec() {
        let spec = Spec::new(LayerHandle::null(), Path::from("/Prim.attr"));
        let prop: PropertySpec = spec.clone().into();
        assert_eq!(prop.spec(), &spec);
    }

    #[test]
    fn test_into_spec() {
        let spec = Spec::new(LayerHandle::null(), Path::from("/Prim.attr"));
        let prop = PropertySpec::new(spec.clone());
        let unwrapped: Spec = prop.into();
        assert_eq!(unwrapped, spec);
    }

    #[test]
    fn test_property_spec_clone() {
        let spec = Spec::new(LayerHandle::null(), Path::from("/Prim.attr"));
        let prop1 = PropertySpec::new(spec);
        let prop2 = prop1.clone();
        assert_eq!(prop1, prop2);
    }

    #[test]
    fn test_property_spec_equality() {
        let spec1 = Spec::new(LayerHandle::null(), Path::from("/Prim.attr1"));
        let spec2 = Spec::new(LayerHandle::null(), Path::from("/Prim.attr1"));
        let spec3 = Spec::new(LayerHandle::null(), Path::from("/Prim.attr2"));

        let prop1 = PropertySpec::new(spec1);
        let prop2 = PropertySpec::new(spec2);
        let prop3 = PropertySpec::new(spec3);

        assert_eq!(prop1, prop2);
        assert_ne!(prop1, prop3);
    }

    #[test]
    fn test_spec_accessor() {
        let spec = Spec::new(LayerHandle::null(), Path::from("/Prim.attr"));
        let prop = PropertySpec::new(spec.clone());
        assert_eq!(prop.spec(), &spec);
        assert_eq!(*prop.as_ref(), spec);
    }

    #[test]
    fn test_spec_mut_accessor() {
        let spec = Spec::new(LayerHandle::null(), Path::from("/Prim.attr"));
        let mut prop = PropertySpec::new(spec);
        let spec_mut = prop.spec_mut();
        assert!(spec_mut.is_dormant());
    }

    #[test]
    fn test_into_spec_unwrap() {
        let spec = Spec::new(LayerHandle::null(), Path::from("/Prim.attr"));
        let prop = PropertySpec::new(spec.clone());
        let unwrapped = prop.into_spec();
        assert_eq!(unwrapped, spec);
    }

    #[test]
    fn test_owner_placeholder() {
        let spec = Spec::new(LayerHandle::null(), Path::from("/Prim.attr"));
        let prop = PropertySpec::new(spec);
        // Currently returns None as placeholder
        assert!(prop.owner().is_none());
    }

    #[test]
    fn test_name_from_property_path() {
        let paths = vec![
            ("/Prim.attr", "attr"),
            ("/World/Cube.visibility", "visibility"),
            ("/Root.transform:translate", "transform:translate"),
        ];

        for (path_str, expected_name) in paths {
            let spec = Spec::new(LayerHandle::null(), Path::from(path_str));
            let prop = PropertySpec::new(spec);
            assert_eq!(prop.name().as_str(), expected_name);
        }
    }

    #[test]
    fn test_name_from_non_property_path() {
        // Non-property paths should return empty token
        let spec = Spec::new(LayerHandle::null(), Path::from("/Prim"));
        let prop = PropertySpec::new(spec);
        assert_eq!(prop.name(), Token::empty());
    }
}
