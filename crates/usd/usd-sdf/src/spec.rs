//! SDF Spec - base class for all scene description specs.
//!
//! `Spec` is the base class for all objects in a scene description layer.
//! Each spec represents a location in a layer (identified by a path) and
//! contains a set of fields that describe the scene element at that location.
//!
//! # Field Access
//!
//! Specs store their data in fields. Fields can be accessed directly:
//! - `get_field()` - Get field value as VtValue
//! - `set_field()` - Set field value
//! - `has_field()` - Check if field exists
//! - `clear_field()` - Remove field
//!
//! # Info/Metadata API
//!
//! The Info API provides typed access to metadata fields:
//! - `get_info()` - Get metadata with fallback to default
//! - `set_info()` - Set metadata with type validation
//! - `has_info()` - Check if metadata is authored
//! - `clear_info()` - Clear metadata
//!
//! # Examples
//!
//! ```
//! use usd_sdf::{Spec, Path};
//!
//! // Access spec properties
//! let spec = Spec::default();
//! let path = spec.path();
//! let layer = spec.layer();
//!
//! // Check spec state
//! assert!(spec.is_dormant()); // No layer/path
//! ```

use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};

use usd_tf::Token;

use super::schema::Schema;
use super::{LayerHandle, Path, SpecType};

// ============================================================================
// VtValue - Type alias to vt::Value
// ============================================================================

/// Type-erased value container.
///
/// VtValue is USD's type-erased value container, equivalent to `vt::Value`.
/// It can hold any type that implements the required traits.
///
/// # Examples
///
/// ```ignore
/// use usd_sdf::VtValue;
///
/// let int_val = VtValue::new(42i32);
/// let str_val = VtValue::new("hello".to_string());
///
/// assert!(int_val.is::<i32>());
/// assert_eq!(int_val.get::<i32>(), Some(&42));
/// ```
pub type VtValue = usd_vt::Value;

// ============================================================================
// VtDictionary - Dictionary of named values
// ============================================================================

/// Dictionary of named values.
///
/// VtDictionary maps string keys to VtValue (type-erased values).
/// Used for metadata, customData, and other key-value storage in USD.
pub type VtDictionary = HashMap<String, VtValue>;

// ============================================================================
// Identity - Internal spec identity
// ============================================================================

/// Internal identity for a spec, uniquely identifying it within a layer.
///
/// Each spec has an identity that consists of:
/// - A reference to the layer containing the spec
/// - The path within that layer
///
/// This is similar to Sdf_Identity in OpenUSD.
#[derive(Debug, Clone)]
struct Identity {
    layer: LayerHandle,
    path: Path,
}

impl Identity {
    /// Create a new identity.
    fn new(layer: LayerHandle, path: Path) -> Self {
        Self { layer, path }
    }

    /// Get the layer.
    fn layer(&self) -> &LayerHandle {
        &self.layer
    }

    /// Get the path.
    fn path(&self) -> &Path {
        &self.path
    }
}

impl PartialEq for Identity {
    fn eq(&self, other: &Self) -> bool {
        self.layer == other.layer && self.path == other.path
    }
}

impl Eq for Identity {}

impl PartialOrd for Identity {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Identity {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.layer.cmp(&other.layer) {
            Ordering::Equal => self.path.cmp(&other.path),
            ord => ord,
        }
    }
}

impl Hash for Identity {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.layer.hash(state);
        self.path.hash(state);
    }
}

// ============================================================================
// Spec - Base class for all specs
// ============================================================================

/// Base class for all Sdf spec objects.
///
/// `Spec` represents an object in a scene description layer. Each spec
/// has a path that identifies its location within the layer's namespace
/// and a set of fields that store its data.
///
/// # Lifecycle
///
/// A spec can be in one of two states:
/// - **Active** - Has a valid identity (layer + path) and data exists
/// - **Dormant** - Invalid or expired, no longer represents live data
///
/// # Field Storage
///
/// Specs don't store field data directly - they delegate to their layer.
/// All field operations go through the layer's data storage.
///
/// # Thread Safety
///
/// Specs are not thread-safe for mutation but can be read from multiple
/// threads if the underlying layer is not being modified.
#[derive(Debug, Clone, Default)]
pub struct Spec {
    /// Internal identity reference (None if dormant).
    identity: Option<Identity>,
}

impl Spec {
    // ========================================================================
    // Construction and Identity
    // ========================================================================

    /// Create a new spec with the given layer and path.
    ///
    /// Note: This doesn't create the spec in the layer - it just creates
    /// a handle to a spec at that location. The spec must already exist
    /// in the layer for operations to succeed.
    #[must_use]
    pub fn new(layer: LayerHandle, path: Path) -> Self {
        Self {
            identity: Some(Identity::new(layer, path)),
        }
    }

    /// Create a dormant (invalid) spec.
    #[must_use]
    pub fn dormant() -> Self {
        Self { identity: None }
    }

    /// Returns the layer that this spec belongs to.
    ///
    /// Returns an invalid layer handle if the spec is dormant.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Spec;
    ///
    /// let spec = Spec::default();
    /// let layer = spec.layer();
    /// assert!(!layer.is_valid());
    /// ```
    #[must_use]
    pub fn layer(&self) -> LayerHandle {
        self.identity
            .as_ref()
            .map(|id| id.layer().clone())
            .unwrap_or_else(LayerHandle::null)
    }

    /// Returns the scene path of this spec.
    ///
    /// Returns an empty path if the spec is dormant.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::{Spec, Path};
    ///
    /// let spec = Spec::new(LayerHandle::null(), Path::from("/World"));
    /// assert_eq!(spec.path(), Path::from("/World"));
    /// ```
    #[must_use]
    pub fn path(&self) -> Path {
        self.identity
            .as_ref()
            .map(|id| id.path().clone())
            .unwrap_or_else(Path::empty)
    }

    /// Returns the spec type for this spec.
    ///
    /// The spec type identifies what kind of scene description object
    /// this spec represents (prim, attribute, relationship, etc.).
    ///
    /// Returns `SpecType::Unknown` if the spec is dormant or if the
    /// layer doesn't have a spec at this path.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::{Spec, SpecType};
    ///
    /// let spec = Spec::default();
    /// assert_eq!(spec.spec_type(), SpecType::Unknown);
    /// ```
    #[must_use]
    pub fn spec_type(&self) -> SpecType {
        if self.is_dormant() {
            SpecType::Unknown
        } else {
            self.layer().get_spec_type(&self.path())
        }
    }

    /// Returns the schema instance for this spec.
    ///
    /// The schema provides field definitions, spec definitions, and
    /// validation rules for USD scene description.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Spec;
    ///
    /// let spec = Spec::default();
    /// let schema = spec.schema();
    /// ```
    #[must_use]
    pub fn schema(&self) -> &'static Schema {
        Schema::instance()
    }

    /// Returns true if this spec is dormant (invalid or expired).
    ///
    /// A spec is dormant if:
    /// - It has no identity (layer/path)
    /// - Its path is empty
    /// - Its layer is invalid
    /// - The layer has no spec at this path
    ///
    /// Dormant specs cannot be used for field operations.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Spec;
    ///
    /// let spec = Spec::default();
    /// assert!(spec.is_dormant());
    /// ```
    #[must_use]
    pub fn is_dormant(&self) -> bool {
        match &self.identity {
            None => true,
            Some(id) => {
                // Check if path is empty
                if id.path().is_empty() {
                    return true;
                }
                // Check if layer is invalid
                if !id.layer().is_valid() {
                    return true;
                }
                // Check if layer has spec at this path
                !id.layer().has_spec(id.path())
            }
        }
    }

    /// Returns true if this spec is inert.
    ///
    /// A spec is inert if it has no significant data - it doesn't
    /// contribute any opinions to the scene. Inert specs are typically
    /// removed during cleanup operations.
    ///
    /// # Parameters
    ///
    /// - `ignore_children` - If true, don't consider child specs when
    ///   determining inertness. A spec with only children and no local
    ///   data would be considered inert.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Spec;
    ///
    /// let spec = Spec::default();
    /// assert!(!spec.is_inert(false)); // Dormant specs are not inert
    /// ```
    #[must_use]
    pub fn is_inert(&self, ignore_children: bool) -> bool {
        // Dormant specs are not inert (they're invalid)
        if self.is_dormant() {
            return false;
        }
        // A spec is inert if it has no fields (no authored data)
        let fields = self.list_fields();
        if fields.is_empty() {
            return true;
        }
        // If ignoring children, only check non-child fields
        if ignore_children {
            // Filter out child-related fields
            fields.iter().all(|f| {
                let name = f.as_str();
                name == "primChildren" || name == "properties"
            })
        } else {
            false
        }
    }

    /// Returns whether this spec's layer can be edited.
    ///
    /// Returns false if the spec is dormant or if the layer is read-only.
    #[must_use]
    pub fn permission_to_edit(&self) -> bool {
        if self.is_dormant() {
            return false;
        }
        !self.layer().is_read_only()
    }

    // ========================================================================
    // Field Access API
    // ========================================================================

    /// Returns all field names that have values in this spec.
    ///
    /// This includes all authored fields but excludes fields that only
    /// hold child specs.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Spec;
    ///
    /// let spec = Spec::default();
    /// let fields = spec.list_fields();
    /// assert!(fields.is_empty()); // Dormant spec has no fields
    /// ```
    #[must_use]
    pub fn list_fields(&self) -> Vec<Token> {
        if self.is_dormant() {
            return Vec::new();
        }
        // Delegate to layer
        let layer = self.layer();
        if !layer.is_valid() {
            return Vec::new();
        }
        layer.list_fields(&self.path())
    }

    /// Returns true if the spec has a non-empty value for the given field.
    ///
    /// # Parameters
    ///
    /// - `name` - The field name to check
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::{Spec, Token};
    ///
    /// let spec = Spec::default();
    /// assert!(!spec.has_field(&Token::new("comment")));
    /// ```
    #[must_use]
    pub fn has_field(&self, name: &Token) -> bool {
        if self.is_dormant() {
            return false;
        }
        // Delegate to layer
        let layer = self.layer();
        if !layer.is_valid() {
            return false;
        }
        layer.has_field(&self.path(), name)
    }

    /// Returns the value for the given field.
    ///
    /// Returns an empty VtValue if the field doesn't exist or the spec
    /// is dormant.
    ///
    /// # Parameters
    ///
    /// - `name` - The field name to retrieve
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::{Spec, Token};
    ///
    /// let spec = Spec::default();
    /// let value = spec.get_field(&Token::new("comment"));
    /// assert!(value.is_empty());
    /// ```
    #[must_use]
    pub fn get_field(&self, name: &Token) -> VtValue {
        if self.is_dormant() {
            return VtValue::empty();
        }
        // Delegate to layer to get field
        let layer = self.layer();
        if !layer.is_valid() {
            return VtValue::empty();
        }
        layer
            .get_field(&self.path(), name)
            .map(|v| VtValue::from_value(&v))
            .unwrap_or_else(VtValue::empty)
    }

    /// Sets the value for the given field.
    ///
    /// Returns false if the spec is dormant or if the layer doesn't
    /// allow editing.
    ///
    /// # Parameters
    ///
    /// - `name` - The field name to set
    /// - `value` - The value to set
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::{Spec, Token, VtValue};
    ///
    /// let mut spec = Spec::default();
    /// let result = spec.set_field(
    ///     &Token::new("comment"),
    ///     VtValue::from("My comment")
    /// );
    /// assert!(!result); // Dormant spec can't be edited
    /// ```
    pub fn set_field(&mut self, name: &Token, value: VtValue) -> bool {
        if self.is_dormant() {
            return false;
        }
        // Delegate to layer to set field
        let layer = self.layer();
        if !layer.is_valid() {
            return false;
        }
        layer.set_field(&self.path(), name, value)
    }

    /// Clears (removes) the given field.
    ///
    /// Returns false if the spec is dormant or if the layer doesn't
    /// allow editing.
    ///
    /// # Parameters
    ///
    /// - `name` - The field name to clear
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::{Spec, Token};
    ///
    /// let mut spec = Spec::default();
    /// let result = spec.clear_field(&Token::new("comment"));
    /// assert!(!result); // Dormant spec can't be edited
    /// ```
    pub fn clear_field(&mut self, name: &Token) -> bool {
        if self.is_dormant() {
            return false;
        }
        // Delegate to layer to erase field
        let layer = self.layer();
        if !layer.is_valid() {
            return false;
        }
        layer.erase_field(&self.path(), name)
    }

    // ========================================================================
    // Info/Metadata API
    // ========================================================================

    /// Returns all info keys currently set on this spec.
    ///
    /// This does not include fields that hold child specs, only value fields.
    /// Also filters to only include fields that are valid for this spec type.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Spec;
    ///
    /// let spec = Spec::default();
    /// let keys = spec.list_info_keys();
    /// assert!(keys.is_empty());
    /// ```
    #[must_use]
    pub fn list_info_keys(&self) -> Vec<Token> {
        if self.is_dormant() {
            return Vec::new();
        }
        // Get spec definition from schema and filter fields
        let spec_type = self.spec_type();
        if spec_type == SpecType::Unknown {
            return self.list_fields();
        }

        let schema = self.schema();
        let spec_def = schema.base().get_spec_def(spec_type);

        if let Some(spec_def) = spec_def {
            // Return only fields that are valid for this spec type
            let all_fields = self.list_fields();
            all_fields
                .into_iter()
                .filter(|field| spec_def.is_valid_field(field))
                .collect()
        } else {
            // Fallback to all fields if no spec definition
            self.list_fields()
        }
    }

    /// Returns the list of metadata info keys for this spec.
    ///
    /// This returns only the keys that should be considered metadata
    /// by inspectors or other presentation UI, as determined by the schema.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::Spec;
    ///
    /// let spec = Spec::default();
    /// let keys = spec.metadata_info_keys();
    /// assert!(keys.is_empty());
    /// ```
    #[must_use]
    pub fn metadata_info_keys(&self) -> Vec<Token> {
        if self.is_dormant() {
            return Vec::new();
        }
        // Query schema for metadata fields for this spec type
        let spec_type = self.spec_type();
        if spec_type == SpecType::Unknown {
            return Vec::new();
        }

        self.schema().base().get_metadata_fields(spec_type)
    }

    /// Returns the display group for the given metadata key.
    ///
    /// Metadata fields can be organized into display groups for UI purposes.
    /// This returns the group name for the given key, or an empty token if
    /// the key has no group or is not a metadata field.
    ///
    /// # Parameters
    ///
    /// - `key` - The metadata key to query
    #[must_use]
    pub fn metadata_display_group(&self, key: &Token) -> Token {
        if self.is_dormant() {
            return Token::empty();
        }
        // Query schema for display group
        let spec_type = self.spec_type();
        if spec_type == SpecType::Unknown {
            return Token::empty();
        }

        self.schema()
            .base()
            .get_metadata_display_group(spec_type, key)
            .unwrap_or_else(Token::empty)
    }

    /// Gets the value for the given metadata key.
    ///
    /// If the field is not set, returns the fallback (default) value
    /// defined in the schema. This differs from `get_field()` which
    /// returns an empty value if the field is not set.
    ///
    /// # Parameters
    ///
    /// - `key` - The metadata key to retrieve
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::{Spec, Token};
    ///
    /// let spec = Spec::default();
    /// let value = spec.get_info(&Token::new("comment"));
    /// // Returns default value (empty string) if not set
    /// ```
    #[must_use]
    pub fn get_info(&self, key: &Token) -> VtValue {
        if self.is_dormant() {
            return VtValue::empty();
        }
        // Get the field value
        let value = self.get_field(key);
        if !value.is_empty() {
            return value;
        }
        // Return fallback from schema
        self.schema().base().get_fallback(key)
    }

    /// Sets the value for the given metadata key.
    ///
    /// This performs validation to ensure:
    /// - The field is defined in the schema
    /// - The field is not read-only
    /// - The field is valid for this spec type
    /// - The value type matches the expected type
    ///
    /// # Parameters
    ///
    /// - `key` - The metadata key to set
    /// - `value` - The value to set (will be type-checked)
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::{Spec, Token, VtValue};
    ///
    /// let mut spec = Spec::default();
    /// spec.set_info(
    ///     &Token::new("comment"),
    ///     VtValue::from("A comment")
    /// );
    /// ```
    pub fn set_info(&mut self, key: &Token, value: VtValue) {
        if self.is_dormant() {
            return;
        }

        let schema = self.schema();
        let spec_type = self.spec_type();

        // Validate field with schema
        // Check field exists
        if !schema.base().is_registered(key) {
            // Allow unregistered fields (custom fields)
            let _ = self.set_field(key, value);
            return;
        }

        // Check field is valid for spec type
        if spec_type != SpecType::Unknown && !schema.base().is_valid_field_for_spec(key, spec_type)
        {
            // Field not valid for this spec type, but allow it anyway
            // (might be a custom field or plugin field)
            let _ = self.set_field(key, value);
            return;
        }

        // Check field is not read-only
        if let Some(field_def) = schema.base().get_field_def(key) {
            if field_def.is_read_only() {
                // Read-only field, skip setting
                return;
            }

            // Validate value type if validator exists
            let validation = field_def.is_valid_value(&value);
            if !validation.is_allowed() {
                // Invalid value, skip setting
                return;
            }
        }

        // All checks passed, set the field
        let _ = self.set_field(key, value);
    }

    /// Sets a value in a dictionary-valued metadata field.
    ///
    /// This is a convenience method for modifying dictionary fields.
    /// It reads the current dictionary, modifies it, and writes it back.
    ///
    /// # Parameters
    ///
    /// - `dict_key` - The dictionary field name
    /// - `entry_key` - The key within the dictionary
    /// - `value` - The value to set (empty to remove)
    pub fn set_info_dict_value(&mut self, dict_key: &Token, entry_key: &str, value: VtValue) {
        if self.is_dormant() {
            return;
        }

        // Get current dictionary based on dict_key
        let mut dict = if dict_key == "customData" {
            self.custom_data()
        } else if dict_key == "assetInfo" {
            self.asset_info()
        } else if dict_key == "customLayerData" {
            // For layer specs, get customLayerData
            if self.spec_type() == super::SpecType::PseudoRoot {
                let layer_value = self.get_field(&Token::new("customLayerData"));
                layer_value.as_dictionary().unwrap_or_default()
            } else {
                VtDictionary::new()
            }
        } else {
            // Unknown dict field, try to get it as a regular field
            let field_value = self.get_field(dict_key);
            field_value.as_dictionary().unwrap_or_default()
        };

        // Modify dictionary
        if value.is_empty() {
            dict.remove(entry_key);
        } else {
            dict.insert(entry_key.to_string(), value);
        }

        // Set dictionary back based on dict_key
        if dict_key == "customData" {
            self.set_custom_data(dict);
        } else if dict_key == "assetInfo" {
            self.set_asset_info(dict);
        } else if dict_key == "customLayerData" {
            // For layer specs, set customLayerData
            if self.spec_type() == super::SpecType::PseudoRoot {
                let dict_value = VtValue::from_dictionary(dict);
                let _ = self.set_field(&Token::new("customLayerData"), dict_value);
            }
        } else {
            // Unknown dict field, try to set it as a regular field
            let dict_value = VtValue::from_dictionary(dict);
            let _ = self.set_field(dict_key, dict_value);
        }
    }

    /// Returns whether the spec has an authored value for the given metadata key.
    ///
    /// Unlike `get_info()` which always returns a value (falling back to defaults),
    /// this tells you whether a value was explicitly set.
    ///
    /// # Parameters
    ///
    /// - `key` - The metadata key to check
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::{Spec, Token};
    ///
    /// let spec = Spec::default();
    /// assert!(!spec.has_info(&Token::new("comment")));
    /// ```
    #[must_use]
    pub fn has_info(&self, key: &Token) -> bool {
        self.has_field(key)
    }

    /// Clears the authored value for the given metadata key.
    ///
    /// After calling this, `has_info()` will return false and `get_info()`
    /// will return the default value from the schema.
    ///
    /// # Parameters
    ///
    /// - `key` - The metadata key to clear
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::{Spec, Token};
    ///
    /// let mut spec = Spec::default();
    /// spec.clear_info(&Token::new("comment"));
    /// ```
    pub fn clear_info(&mut self, key: &Token) {
        if self.is_dormant() {
            return;
        }
        // Clear the field
        let _ = self.clear_field(key);
    }

    /// Returns the expected type for the given metadata key.
    ///
    /// This queries the schema for the field definition and returns
    /// the type information.
    ///
    /// # Parameters
    ///
    /// - `key` - The metadata key to query
    #[must_use]
    pub fn type_for_info(&self, key: &Token) -> Option<&'static str> {
        if self.is_dormant() {
            return None;
        }
        // Query schema for field type by getting fallback value and its type name
        let fallback = self.schema().base().get_fallback(key);
        if fallback.is_empty() {
            return None;
        }
        fallback.type_name()
    }

    /// Returns the fallback (default) value for the given metadata key.
    ///
    /// This is the value returned by `get_info()` when the field is not
    /// explicitly authored.
    ///
    /// # Parameters
    ///
    /// - `key` - The metadata key to query
    #[must_use]
    pub fn fallback_for_info(&self, key: &Token) -> VtValue {
        if self.is_dormant() {
            return VtValue::empty();
        }
        // Query schema for fallback value
        self.schema().base().get_fallback(key)
    }

    // ========================================================================
    // Custom Data and Asset Info
    // ========================================================================

    /// Returns the custom data dictionary for this spec.
    ///
    /// Custom data is a dictionary field that can hold arbitrary user-defined
    /// metadata. It's typically used for pipeline-specific data.
    #[must_use]
    pub fn custom_data(&self) -> VtDictionary {
        if self.is_dormant() {
            return VtDictionary::new();
        }
        // Get customData field and convert to dictionary
        let value = self.get_field(&Token::new("customData"));
        value.as_dictionary().unwrap_or_default()
    }

    /// Sets the custom data dictionary for this spec.
    ///
    /// # Parameters
    ///
    /// - `data` - The custom data dictionary to set
    pub fn set_custom_data(&mut self, data: VtDictionary) {
        if self.is_dormant() {
            return;
        }
        // Convert VtDictionary to VtValue and set
        let value = VtValue::from_dictionary(data);
        let _ = self.set_field(&Token::new("customData"), value);
    }

    /// Returns the asset info dictionary for this spec.
    ///
    /// Asset info contains metadata about assets, such as identifiers,
    /// versions, and other asset management data.
    #[must_use]
    pub fn asset_info(&self) -> VtDictionary {
        if self.is_dormant() {
            return VtDictionary::new();
        }
        // Get assetInfo field and convert to dictionary
        let value = self.get_field(&Token::new("assetInfo"));
        value.as_dictionary().unwrap_or_default()
    }

    /// Sets the asset info dictionary for this spec.
    ///
    /// # Parameters
    ///
    /// - `info` - The asset info dictionary to set
    pub fn set_asset_info(&mut self, info: VtDictionary) {
        if self.is_dormant() {
            return;
        }
        // Convert VtDictionary to VtValue and set
        let value = VtValue::from_dictionary(info);
        let _ = self.set_field(&Token::new("assetInfo"), value);
    }

    // ========================================================================
    // Protected Methods (for subclasses)
    // ========================================================================

    /// Move a spec from one path to another.
    ///
    /// This is a protected method used by subclasses to implement
    /// rename/reparent operations.
    #[must_use]
    pub(crate) fn _move_spec(&self, old_path: &Path, new_path: &Path) -> bool {
        if self.is_dormant() {
            return false;
        }
        // Delegate to layer
        match self.layer().upgrade() {
            Some(layer) => layer.move_spec(old_path, new_path),
            None => false,
        }
    }

    /// Delete a spec at the given path.
    ///
    /// This is a protected method used by subclasses to implement
    /// deletion operations.
    #[must_use]
    pub(crate) fn _delete_spec(&self, path: &Path) -> bool {
        if self.is_dormant() {
            return false;
        }
        // Delegate to layer
        match self.layer().upgrade() {
            Some(layer) => layer.delete_spec(path),
            None => false,
        }
    }
}

// ============================================================================
// Trait Implementations
// ============================================================================

impl PartialEq for Spec {
    /// Two specs are equal if they refer to the same identity.
    ///
    /// This compares layer identity (pointer equality) and path.
    fn eq(&self, other: &Self) -> bool {
        self.identity == other.identity
    }
}

impl Eq for Spec {}

impl PartialOrd for Spec {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Spec {
    /// Specs are ordered first by layer, then by path.
    fn cmp(&self, other: &Self) -> Ordering {
        self.identity.cmp(&other.identity)
    }
}

impl Hash for Spec {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.identity.hash(state);
    }
}

impl fmt::Display for Spec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_dormant() {
            write!(f, "<dormant spec>")
        } else {
            write!(f, "<{} at {}>", self.spec_type(), self.path())
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
    fn test_spec_default() {
        let spec = Spec::default();
        assert!(spec.is_dormant());
        assert_eq!(spec.path(), Path::empty());
        assert!(!spec.layer().is_valid());
        assert_eq!(spec.spec_type(), SpecType::Unknown);
    }

    #[test]
    fn test_spec_dormant() {
        let spec = Spec::dormant();
        assert!(spec.is_dormant());
        assert!(!spec.permission_to_edit());
    }

    #[test]
    fn test_spec_new() {
        let layer = LayerHandle::null();
        let path = Path::from("/World");
        let spec = Spec::new(layer, path.clone());

        // Spec is dormant because layer is null
        assert!(spec.is_dormant());
        assert_eq!(spec.path(), path);
    }

    #[test]
    fn test_spec_equality() {
        let layer = LayerHandle::null();
        let path1 = Path::from("/World");
        let path2 = Path::from("/World");
        let path3 = Path::from("/Other");

        let spec1 = Spec::new(layer.clone(), path1);
        let spec2 = Spec::new(layer.clone(), path2);
        let spec3 = Spec::new(layer, path3);

        assert_eq!(spec1, spec2);
        assert_ne!(spec1, spec3);
    }

    #[test]
    fn test_spec_ordering() {
        let layer = LayerHandle::null();
        let spec1 = Spec::new(layer.clone(), Path::from("/A"));
        let spec2 = Spec::new(layer, Path::from("/B"));

        assert!(spec1 < spec2);
        assert!(spec2 > spec1);
    }

    #[test]
    fn test_spec_field_access_dormant() {
        let spec = Spec::dormant();
        let key = Token::new("comment");

        assert!(!spec.has_field(&key));
        assert!(spec.get_field(&key).is_empty());
        assert_eq!(spec.list_fields(), Vec::<Token>::new());
    }

    #[test]
    fn test_spec_info_access_dormant() {
        let spec = Spec::dormant();
        let key = Token::new("comment");

        assert!(!spec.has_info(&key));
        assert!(spec.get_info(&key).is_empty());
        assert_eq!(spec.list_info_keys(), Vec::<Token>::new());
        assert_eq!(spec.metadata_info_keys(), Vec::<Token>::new());
    }

    #[test]
    fn test_spec_field_mutations_dormant() {
        let mut spec = Spec::dormant();
        let key = Token::new("comment");
        let value = VtValue::from("test");

        assert!(!spec.set_field(&key, value.clone()));
        assert!(!spec.clear_field(&key));
    }

    #[test]
    fn test_spec_info_mutations_dormant() {
        let mut spec = Spec::dormant();
        let key = Token::new("comment");
        let value = VtValue::from("test");

        // Should not panic, just no-op
        spec.set_info(&key, value);
        spec.clear_info(&key);
        spec.set_info_dict_value(&key, "entry", VtValue::from("val"));
    }

    #[test]
    fn test_spec_custom_data() {
        let spec = Spec::dormant();
        let data = spec.custom_data();
        assert!(data.is_empty());
    }

    #[test]
    fn test_spec_asset_info() {
        let spec = Spec::dormant();
        let info = spec.asset_info();
        assert!(info.is_empty());
    }

    #[test]
    fn test_spec_display() {
        let dormant = Spec::dormant();
        assert_eq!(format!("{}", dormant), "<dormant spec>");

        // Spec with null layer is also dormant
        let spec_null_layer = Spec::new(LayerHandle::null(), Path::from("/World"));
        assert_eq!(format!("{}", spec_null_layer), "<dormant spec>");

        // When Layer is implemented, a valid spec would show:
        // "<unknown at /World>" or "<prim at /World>" etc.
    }

    #[test]
    fn test_spec_hash() {
        use std::collections::HashSet;

        let layer = LayerHandle::null();
        let mut set = HashSet::new();

        set.insert(Spec::new(layer.clone(), Path::from("/World")));
        set.insert(Spec::new(layer.clone(), Path::from("/Other")));
        set.insert(Spec::new(layer, Path::from("/World"))); // Duplicate

        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_vtvalue_basic() {
        let empty = VtValue::empty();
        assert!(empty.is_empty());

        let value = VtValue::from("test");
        assert!(!value.is_empty());
        assert_eq!(value.get::<String>().map(|s| s.as_str()), Some("test"));
    }

    #[test]
    fn test_vtvalue_equality() {
        let v1 = VtValue::from("test");
        let v2 = VtValue::from("test");
        let v3 = VtValue::from("other");

        assert_eq!(v1, v2);
        assert_ne!(v1, v3);
    }

    #[test]
    fn test_identity_equality() {
        let layer = LayerHandle::null();
        let path1 = Path::from("/World");
        let path2 = Path::from("/World");
        let path3 = Path::from("/Other");

        let id1 = Identity::new(layer.clone(), path1);
        let id2 = Identity::new(layer.clone(), path2);
        let id3 = Identity::new(layer, path3);

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_identity_ordering() {
        let layer = LayerHandle::null();
        let id1 = Identity::new(layer.clone(), Path::from("/A"));
        let id2 = Identity::new(layer, Path::from("/B"));

        assert!(id1 < id2);
        assert!(id2 > id1);
    }

    #[test]
    fn test_protected_methods() {
        let spec = Spec::dormant();
        assert!(!spec._move_spec(&Path::from("/A"), &Path::from("/B")));
        assert!(!spec._delete_spec(&Path::from("/A")));
    }

    #[test]
    fn test_type_for_info() {
        let spec = Spec::dormant();
        assert_eq!(spec.type_for_info(&Token::new("comment")), None);
    }

    #[test]
    fn test_fallback_for_info() {
        let spec = Spec::dormant();
        assert!(spec.fallback_for_info(&Token::new("comment")).is_empty());
    }

    #[test]
    fn test_metadata_display_group() {
        let spec = Spec::dormant();
        assert_eq!(
            spec.metadata_display_group(&Token::new("comment")),
            Token::empty()
        );
    }

    #[test]
    fn test_is_inert() {
        let spec = Spec::dormant();
        assert!(!spec.is_inert(false));
        assert!(!spec.is_inert(true));
    }

    #[test]
    fn test_spec_clone() {
        let layer = LayerHandle::null();
        let path = Path::from("/World");
        let spec1 = Spec::new(layer, path);
        let spec2 = spec1.clone();

        assert_eq!(spec1, spec2);
        assert_eq!(spec1.path(), spec2.path());
    }
}
