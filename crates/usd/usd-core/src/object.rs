//! Base class for prims and properties.
//!
//! UsdObject is the abstract base for UsdPrim and UsdProperty.
//! It provides common functionality for metadata access.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Weak};

use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::{Dictionary, Value};

// Re-export Stage from stage module
pub use super::stage::Stage;

fn applied_api_schema_has_property(schema_type: &Token, instance_name: &Token, prop_name: &str) -> bool {
    if super::schema_registry::schema_instance_has_property(schema_type, instance_name, prop_name) {
        return true;
    }

    if schema_type.get_text() == "CollectionAPI" {
        let collection_name = instance_name.get_text();
        if prop_name == format!("collection:{collection_name}") {
            return true;
        }
        if let Some(base_name) =
            prop_name.strip_prefix(&format!("collection:{collection_name}:"))
        {
            return matches!(
                base_name,
                "expansionRule" | "includeRoot" | "membershipExpression" | "includes" | "excludes"
            );
        }
    }

    false
}

// ============================================================================
// UsdObjType - Object type enumeration
// ============================================================================

/// Enum values to represent the various Usd object types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum ObjType {
    /// Base object type
    Object = 0,
    /// Prim type
    Prim = 1,
    /// Property type (base for Attribute and Relationship)
    Property = 2,
    /// Attribute type
    Attribute = 3,
    /// Relationship type
    Relationship = 4,
}

impl ObjType {
    /// Returns true if `subtype` is the same as or a subtype of `basetype`.
    #[inline]
    pub fn is_subtype(basetype: ObjType, subtype: ObjType) -> bool {
        basetype == ObjType::Object
            || basetype == subtype
            || (basetype == ObjType::Property && subtype as u8 > ObjType::Property as u8)
    }

    /// Returns true if `from` is convertible to `to`.
    #[inline]
    pub fn is_convertible(from: ObjType, to: ObjType) -> bool {
        Self::is_subtype(to, from)
    }

    /// Returns true if `obj_type` is a concrete type (Prim, Attribute, or Relationship).
    #[inline]
    pub fn is_concrete(obj_type: ObjType) -> bool {
        matches!(
            obj_type,
            ObjType::Prim | ObjType::Attribute | ObjType::Relationship
        )
    }
}

impl fmt::Display for ObjType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ObjType::Object => write!(f, "Object"),
            ObjType::Prim => write!(f, "Prim"),
            ObjType::Property => write!(f, "Property"),
            ObjType::Attribute => write!(f, "Attribute"),
            ObjType::Relationship => write!(f, "Relationship"),
        }
    }
}

// ============================================================================
// MetadataValueMap - sorted map of metadata
// ============================================================================

/// A sorted map of token keys to VtValue values for metadata.
pub type MetadataValueMap = BTreeMap<Token, Value>;

// ============================================================================
// Object
// ============================================================================

/// Base class for prims and properties on a stage.
///
/// UsdObject provides common functionality shared by UsdPrim and UsdProperty:
/// - Path access
/// - Stage access
/// - Validity checking
/// - **Metadata access** (GetMetadata, SetMetadata, ClearMetadata, etc.)
/// - **CustomData access** (GetCustomData, SetCustomData, etc.)
/// - **AssetInfo access** (GetAssetInfo, SetAssetInfo, etc.)
/// - **Documentation** (GetDocumentation, SetDocumentation, etc.)
///
/// The commonality between the three types of scenegraph objects in Usd
/// (UsdPrim, UsdAttribute, UsdRelationship) is that they can all have metadata.
#[derive(Debug, Clone)]
pub struct Object {
    /// Weak reference to the owning stage.
    pub(crate) stage: Weak<Stage>,
    /// Path to this object.
    pub(crate) path: Path,
    /// Object type.
    pub(crate) obj_type: ObjType,
    /// Property name (for properties only).
    pub(crate) prop_name: Option<Token>,
}

impl Object {
    // =========================================================================
    // Construction
    // =========================================================================

    /// Creates a new object.
    pub(crate) fn new(stage: Weak<Stage>, path: Path) -> Self {
        let obj_type = if path.is_property_path() {
            ObjType::Property
        } else {
            ObjType::Prim
        };
        Self {
            stage,
            path,
            obj_type,
            prop_name: None,
        }
    }

    /// Creates a new object with explicit type.
    #[allow(dead_code)] // Used in tests
    pub(crate) fn new_with_type(stage: Weak<Stage>, path: Path, obj_type: ObjType) -> Self {
        Self {
            stage,
            path,
            obj_type,
            prop_name: None,
        }
    }

    /// Creates an invalid object.
    pub fn invalid() -> Self {
        Self {
            stage: Weak::new(),
            path: Path::empty(),
            obj_type: ObjType::Object,
            prop_name: None,
        }
    }

    /// Creates an Object from a Prim.
    pub fn from_prim(prim: super::prim::Prim) -> Self {
        let stage = prim.stage().map(|s| Arc::downgrade(&s)).unwrap_or_default();
        Self {
            stage,
            path: prim.path().clone(),
            obj_type: ObjType::Prim,
            prop_name: None,
        }
    }

    // =========================================================================
    // Structural and Integrity Info
    // =========================================================================

    /// Returns true if this object is valid.
    ///
    /// Matches C++ `UsdObject::IsValid()` + `_GetDefiningSpecType`:
    /// walks PrimIndex via Resolver to find spec type across ALL composed
    /// layers (including payloads, references), not just root layer stack.
    pub fn is_valid(&self) -> bool {
        let Some(stage) = self.stage.upgrade() else {
            return false;
        };
        if self.path.is_empty() {
            return false;
        }
        match self.obj_type {
            ObjType::Prim => true,
            ObjType::Attribute => {
                let prim_path = self.path.get_prim_path();
                let prop_name = self.path.get_name();
                let spec = stage.get_defining_spec_type(&prim_path, &prop_name);
                if spec == usd_sdf::SpecType::Attribute {
                    return true;
                }
                // Fall back to schema-defined property check
                if let Some(prim) = stage.get_prim_at_path(&prim_path) {
                    let type_name = prim.type_name();
                    if !type_name.is_empty() {
                        return super::schema_registry::schema_has_property(&type_name, &prop_name);
                    }
                    for applied_schema in prim.get_applied_schemas() {
                        let (schema_type, instance_name) =
                            super::schema_registry::SchemaRegistry::get_type_name_and_instance(
                                &applied_schema,
                            );
                        if !instance_name.is_empty()
                            && applied_api_schema_has_property(
                                &schema_type,
                                &instance_name,
                                &prop_name,
                            )
                        {
                            return true;
                        }
                    }
                }
                false
            }
            ObjType::Relationship => {
                let prim_path = self.path.get_prim_path();
                let prop_name = self.path.get_name();
                let spec = stage.get_defining_spec_type(&prim_path, &prop_name);
                if spec == usd_sdf::SpecType::Relationship {
                    return true;
                }
                if let Some(prim) = stage.get_prim_at_path(&prim_path) {
                    let type_name = prim.type_name();
                    if !type_name.is_empty() {
                        return super::schema_registry::schema_has_property(&type_name, &prop_name);
                    }
                    for applied_schema in prim.get_applied_schemas() {
                        let (schema_type, instance_name) =
                            super::schema_registry::SchemaRegistry::get_type_name_and_instance(
                                &applied_schema,
                            );
                        if !instance_name.is_empty()
                            && applied_api_schema_has_property(
                                &schema_type,
                                &instance_name,
                                &prop_name,
                            )
                        {
                            return true;
                        }
                    }
                }
                false
            }
            ObjType::Property => {
                let prim_path = self.path.get_prim_path();
                let prop_name = self.path.get_name();
                let spec = stage.get_defining_spec_type(&prim_path, &prop_name);
                if spec == usd_sdf::SpecType::Attribute || spec == usd_sdf::SpecType::Relationship {
                    return true;
                }
                if let Some(prim) = stage.get_prim_at_path(&prim_path) {
                    let type_name = prim.type_name();
                    if !type_name.is_empty() {
                        return super::schema_registry::schema_has_property(&type_name, &prop_name);
                    }
                    for applied_schema in prim.get_applied_schemas() {
                        let (schema_type, instance_name) =
                            super::schema_registry::SchemaRegistry::get_type_name_and_instance(
                                &applied_schema,
                            );
                        if !instance_name.is_empty()
                            && applied_api_schema_has_property(
                                &schema_type,
                                &instance_name,
                                &prop_name,
                            )
                        {
                            return true;
                        }
                    }
                }
                false
            }
            ObjType::Object => false,
        }
    }

    /// Returns the object type.
    pub fn obj_type(&self) -> ObjType {
        self.obj_type
    }

    /// Returns the path to this object.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the complete scene path to this object.
    pub fn get_path(&self) -> Path {
        self.path.clone()
    }

    /// Returns the stage that owns this object.
    pub fn stage(&self) -> Option<Arc<Stage>> {
        self.stage.upgrade()
    }

    /// Returns the prim path for this object.
    ///
    /// For prims, this is the same as path(). For properties, this returns
    /// the owning prim's path.
    pub fn prim_path(&self) -> Path {
        if self.path.is_property_path() {
            self.path.get_prim_path()
        } else {
            self.path.clone()
        }
    }

    /// Returns the name of this object.
    pub fn name(&self) -> &str {
        self.path.get_name()
    }

    /// Returns the name as a Token.
    pub fn get_name(&self) -> Token {
        if self.obj_type == ObjType::Prim {
            Token::new(self.path.get_name())
        } else {
            self.prop_name
                .clone()
                .unwrap_or_else(|| Token::new(self.path.get_name()))
        }
    }

    /// Convert this UsdObject to another object type T if possible.
    pub fn is_type(&self, target_type: ObjType) -> bool {
        ObjType::is_convertible(self.obj_type, target_type)
    }

    /// Returns a description of this object for debugging.
    pub fn description(&self) -> String {
        self.get_description()
    }

    /// Returns a brief summary description of the object.
    pub fn get_description(&self) -> String {
        let type_name = match self.obj_type {
            ObjType::Object => "Object",
            ObjType::Prim => "Prim",
            ObjType::Property => "Property",
            ObjType::Attribute => "Attribute",
            ObjType::Relationship => "Relationship",
        };

        if self.path.is_empty() {
            "Invalid object".to_string()
        } else if !self.is_valid() {
            // Object with path but no stage (expired or detached)
            format!(
                "expired {} <{}>",
                type_name.to_lowercase(),
                self.path.get_string()
            )
        } else {
            format!("{} <{}>", type_name.to_lowercase(), self.path.get_string())
        }
    }

    // =========================================================================
    // Generic Metadata Access
    // =========================================================================

    /// Resolve the requested metadatum named `key` into `value`.
    ///
    /// Returns true on success, false if `key` was not resolvable.
    ///
    /// For any composition-related metadata, this method will return only
    /// the strongest opinion found, not applying composition rules.
    pub fn get_metadata(&self, key: &Token) -> Option<Value> {
        let stage = self.stage.upgrade()?;
        stage.get_metadata_for_object(&self.path, key)
    }

    /// Type-safe metadata access.
    pub fn get_metadata_typed<T: Clone + 'static>(&self, key: &Token) -> Option<T> {
        self.get_metadata(key).and_then(|v| v.get::<T>().cloned())
    }

    /// Set metadatum `key`'s value to `value`.
    ///
    /// Returns false if `value`'s type does not match the schema type for `key`.
    pub fn set_metadata(&self, key: &Token, value: Value) -> bool {
        if let Some(stage) = self.stage.upgrade() {
            stage.set_metadata_for_object(&self.path, key, value)
        } else {
            false
        }
    }

    /// Clears the authored `key`'s value at the current EditTarget.
    ///
    /// Returns false on error. If no value is present, this is a no-op
    /// and returns true.
    pub fn clear_metadata(&self, key: &Token) -> bool {
        if let Some(stage) = self.stage.upgrade() {
            stage.clear_metadata_for_object(&self.path, key)
        } else {
            false
        }
    }

    /// Returns true if the `key` has a meaningful value (authored or fallback).
    pub fn has_metadata(&self, key: &Token) -> bool {
        self.get_metadata(key).is_some()
    }

    /// Returns true if the `key` has an authored value (excluding fallbacks).
    pub fn has_authored_metadata(&self, key: &Token) -> bool {
        if let Some(stage) = self.stage.upgrade() {
            stage.has_authored_metadata_for_object(&self.path, key)
        } else {
            false
        }
    }

    /// Resolve the requested dictionary sub-element `key_path` of
    /// dictionary-valued metadatum named `key`.
    ///
    /// The `key_path` is a ':'-separated path addressing an element
    /// in subdictionaries.
    pub fn get_metadata_by_dict_key(&self, key: &Token, key_path: &Token) -> Option<Value> {
        let dict = self.get_metadata(key)?;
        let dict = dict.get::<Dictionary>()?;
        get_value_at_key_path(dict, key_path.as_str())
    }

    /// Author `value` to the field identified by `key` and `key_path`.
    ///
    /// The `key_path` is a ':'-separated path identifying a value in
    /// subdictionaries stored in the metadata field at `key`.
    pub fn set_metadata_by_dict_key(&self, key: &Token, key_path: &Token, value: Value) -> bool {
        let mut dict = self
            .get_metadata(key)
            .and_then(|v| v.try_into_inner::<Dictionary>().ok())
            .unwrap_or_default();

        if set_value_at_key_path(&mut dict, key_path.as_str(), value) {
            self.set_metadata(key, Value::from(dict))
        } else {
            false
        }
    }

    /// Clear any authored value identified by `key` and `key_path`.
    pub fn clear_metadata_by_dict_key(&self, key: &Token, key_path: &Token) -> bool {
        let mut dict = match self
            .get_metadata(key)
            .and_then(|v| v.try_into_inner::<Dictionary>().ok())
        {
            Some(d) => d,
            None => return true, // Nothing to clear
        };

        if clear_value_at_key_path(&mut dict, key_path.as_str()) {
            if dict.is_empty() {
                self.clear_metadata(key)
            } else {
                self.set_metadata(key, Value::from(dict))
            }
        } else {
            false
        }
    }

    /// Returns true if there exists any authored or fallback opinion for
    /// `key` and `key_path`.
    pub fn has_metadata_dict_key(&self, key: &Token, key_path: &Token) -> bool {
        self.get_metadata_by_dict_key(key, key_path).is_some()
    }

    /// Returns true if there exists any authored opinion (excluding fallbacks)
    /// for `key` and `key_path`.
    pub fn has_authored_metadata_dict_key(&self, key: &Token, key_path: &Token) -> bool {
        if !self.has_authored_metadata(key) {
            return false;
        }
        self.get_metadata_by_dict_key(key, key_path).is_some()
    }

    /// Resolve and return all metadata (including both authored and fallback
    /// values) on this object, sorted lexicographically.
    ///
    /// This method does not return field keys for composition arcs.
    pub fn get_all_metadata(&self) -> MetadataValueMap {
        if let Some(stage) = self.stage.upgrade() {
            stage.get_all_metadata_for_object(&self.path)
        } else {
            MetadataValueMap::new()
        }
    }

    /// Resolve and return all user-authored metadata on this object.
    pub fn get_all_authored_metadata(&self) -> MetadataValueMap {
        if let Some(stage) = self.stage.upgrade() {
            stage.get_all_authored_metadata_for_object(&self.path)
        } else {
            MetadataValueMap::new()
        }
    }

    // =========================================================================
    // Core Metadata Fields - Hidden
    // =========================================================================

    /// Gets the value of the 'hidden' metadata field, false if not authored.
    ///
    /// When an object is marked as hidden, it is an indicator to clients who
    /// generically display objects that this object should not be included.
    pub fn is_hidden(&self) -> bool {
        self.get_metadata_typed::<bool>(&Token::new("hidden"))
            .unwrap_or(false)
    }

    /// Alias for is_hidden() for API compatibility.
    pub fn get_hidden(&self) -> bool {
        self.is_hidden()
    }

    /// Sets the value of the 'hidden' metadata field.
    pub fn set_hidden(&self, hidden: bool) -> bool {
        self.set_metadata(&Token::new("hidden"), Value::from(hidden))
    }

    /// Clears the opinion for "hidden" at the current EditTarget.
    pub fn clear_hidden(&self) -> bool {
        self.clear_metadata(&Token::new("hidden"))
    }

    /// Returns true if hidden was explicitly authored.
    pub fn has_authored_hidden(&self) -> bool {
        self.has_authored_metadata(&Token::new("hidden"))
    }

    // =========================================================================
    // Core Metadata Fields - CustomData
    // =========================================================================

    /// Return this object's composed customData dictionary.
    ///
    /// CustomData is "custom metadata", a place for applications and users
    /// to put uniform data that is entirely dynamic and subject to no schema.
    pub fn get_custom_data(&self) -> Dictionary {
        self.get_metadata(&Token::new("customData"))
            .and_then(|v| v.try_into_inner::<Dictionary>().ok())
            .unwrap_or_default()
    }

    /// Return the element identified by `key_path` in customData dictionary.
    ///
    /// The `key_path` is a ':'-separated path identifying a value in
    /// subdictionaries.
    pub fn get_custom_data_by_key(&self, key_path: &Token) -> Option<Value> {
        let dict = self.get_custom_data();
        get_value_at_key_path(&dict, key_path.as_str())
    }

    /// Author this object's customData dictionary at the current EditTarget.
    pub fn set_custom_data(&self, custom_data: Dictionary) {
        self.set_metadata(&Token::new("customData"), Value::from(custom_data));
    }

    /// Author the element identified by `key_path` in customData dictionary.
    pub fn set_custom_data_by_key(&self, key_path: &Token, value: Value) {
        self.set_metadata_by_dict_key(&Token::new("customData"), key_path, value);
    }

    /// Clear the authored opinion for customData dictionary.
    pub fn clear_custom_data(&self) {
        self.clear_metadata(&Token::new("customData"));
    }

    /// Clear the authored opinion identified by `key_path` in customData.
    pub fn clear_custom_data_by_key(&self, key_path: &Token) {
        self.clear_metadata_by_dict_key(&Token::new("customData"), key_path);
    }

    /// Returns true if there are any authored or fallback opinions for customData.
    pub fn has_custom_data(&self) -> bool {
        self.has_metadata(&Token::new("customData"))
    }

    /// Returns true if there are any authored or fallback opinions for
    /// the element identified by `key_path` in customData.
    pub fn has_custom_data_key(&self, key_path: &Token) -> bool {
        self.get_custom_data_by_key(key_path).is_some()
    }

    /// Returns true if there are any authored opinions for customData.
    pub fn has_authored_custom_data(&self) -> bool {
        self.has_authored_metadata(&Token::new("customData"))
    }

    /// Returns true if there are any authored opinions for the element
    /// identified by `key_path` in customData.
    pub fn has_authored_custom_data_key(&self, key_path: &Token) -> bool {
        if !self.has_authored_custom_data() {
            return false;
        }
        self.has_custom_data_key(key_path)
    }

    // =========================================================================
    // Core Metadata Fields - AssetInfo
    // =========================================================================

    /// Return this object's composed assetInfo dictionary.
    ///
    /// The asset info dictionary is used to annotate objects representing the
    /// root-prims of assets with various data related to asset management.
    pub fn get_asset_info(&self) -> Dictionary {
        self.get_metadata(&Token::new("assetInfo"))
            .and_then(|v| v.try_into_inner::<Dictionary>().ok())
            .unwrap_or_default()
    }

    /// Return the element identified by `key_path` in assetInfo dictionary.
    pub fn get_asset_info_by_key(&self, key_path: &Token) -> Option<Value> {
        let dict = self.get_asset_info();
        get_value_at_key_path(&dict, key_path.as_str())
    }

    /// Author this object's assetInfo dictionary at the current EditTarget.
    pub fn set_asset_info(&self, asset_info: Dictionary) {
        self.set_metadata(&Token::new("assetInfo"), Value::from(asset_info));
    }

    /// Author the element identified by `key_path` in assetInfo dictionary.
    pub fn set_asset_info_by_key(&self, key_path: &Token, value: Value) {
        self.set_metadata_by_dict_key(&Token::new("assetInfo"), key_path, value);
    }

    /// Clear the authored opinion for assetInfo dictionary.
    pub fn clear_asset_info(&self) {
        self.clear_metadata(&Token::new("assetInfo"));
    }

    /// Clear the authored opinion identified by `key_path` in assetInfo.
    pub fn clear_asset_info_by_key(&self, key_path: &Token) {
        self.clear_metadata_by_dict_key(&Token::new("assetInfo"), key_path);
    }

    /// Returns true if there are any authored or fallback opinions for assetInfo.
    pub fn has_asset_info(&self) -> bool {
        self.has_metadata(&Token::new("assetInfo"))
    }

    /// Returns true if there are any authored or fallback opinions for
    /// the element identified by `key_path` in assetInfo.
    pub fn has_asset_info_key(&self, key_path: &Token) -> bool {
        self.get_asset_info_by_key(key_path).is_some()
    }

    /// Returns true if there are any authored opinions for assetInfo.
    pub fn has_authored_asset_info(&self) -> bool {
        self.has_authored_metadata(&Token::new("assetInfo"))
    }

    /// Returns true if there are any authored opinions for the element
    /// identified by `key_path` in assetInfo.
    pub fn has_authored_asset_info_key(&self, key_path: &Token) -> bool {
        if !self.has_authored_asset_info() {
            return false;
        }
        self.has_asset_info_key(key_path)
    }

    // =========================================================================
    // Core Metadata Fields - Documentation
    // =========================================================================

    /// Return this object's documentation (metadata).
    ///
    /// Returns the empty string if no documentation has been set.
    pub fn get_documentation(&self) -> String {
        self.get_metadata_typed::<String>(&Token::new("documentation"))
            .unwrap_or_default()
    }

    /// Sets this object's documentation (metadata).
    ///
    /// Returns true on success.
    pub fn set_documentation(&self, doc: &str) -> bool {
        self.set_metadata(&Token::new("documentation"), Value::from(doc.to_string()))
    }

    /// Clears this object's documentation (metadata) in the current EditTarget.
    ///
    /// Returns true on success.
    pub fn clear_documentation(&self) -> bool {
        self.clear_metadata(&Token::new("documentation"))
    }

    /// Returns true if documentation was explicitly authored.
    pub fn has_authored_documentation(&self) -> bool {
        self.has_authored_metadata(&Token::new("documentation"))
    }

    // =========================================================================
    // Core Metadata Fields - DisplayName (deprecated)
    // =========================================================================

    /// Return this object's display name (metadata).
    ///
    /// Returns the empty string if no display name has been set.
    ///
    /// # Deprecated
    /// See UsdUIObjectHints.
    #[deprecated(note = "See UsdUIObjectHints")]
    pub fn get_display_name(&self) -> String {
        self.get_metadata_typed::<String>(&Token::new("displayName"))
            .unwrap_or_default()
    }

    /// Sets this object's display name (metadata).
    ///
    /// DisplayName is meant to be a descriptive label, not necessarily an
    /// alternate identifier.
    ///
    /// # Deprecated
    /// See UsdUIObjectHints.
    #[deprecated(note = "See UsdUIObjectHints")]
    pub fn set_display_name(&self, name: &str) -> bool {
        self.set_metadata(&Token::new("displayName"), Value::from(name.to_string()))
    }

    /// Clears this object's display name (metadata) in the current EditTarget.
    ///
    /// # Deprecated
    /// See UsdUIObjectHints.
    #[deprecated(note = "See UsdUIObjectHints")]
    pub fn clear_display_name(&self) -> bool {
        self.clear_metadata(&Token::new("displayName"))
    }

    /// Returns true if displayName was explicitly authored.
    ///
    /// # Deprecated
    /// See UsdUIObjectHints.
    #[deprecated(note = "See UsdUIObjectHints")]
    pub fn has_authored_display_name(&self) -> bool {
        self.has_authored_metadata(&Token::new("displayName"))
    }

    // =========================================================================
    // Utility
    // =========================================================================

    /// Return the namespace delimiter character.
    pub fn get_namespace_delimiter() -> char {
        ':'
    }
}

// =============================================================================
// Helper functions for dictionary key path navigation
// =============================================================================

/// Get a value from a dictionary at the given ':'-separated key path.
fn get_value_at_key_path(dict: &Dictionary, key_path: &str) -> Option<Value> {
    let parts: Vec<&str> = key_path.split(':').collect();
    get_value_recursive(dict, &parts)
}

fn get_value_recursive(dict: &Dictionary, parts: &[&str]) -> Option<Value> {
    if parts.is_empty() {
        return None;
    }

    let key = parts[0];
    let value = dict.get(key)?;

    if parts.len() == 1 {
        return Some(value.clone());
    }

    // Need to descend into subdictionary
    let sub_dict = value.get::<Dictionary>()?;
    get_value_recursive(sub_dict, &parts[1..])
}

/// Set a value in a dictionary at the given ':'-separated key path.
fn set_value_at_key_path(dict: &mut Dictionary, key_path: &str, value: Value) -> bool {
    let parts: Vec<&str> = key_path.split(':').collect();
    set_value_recursive(dict, &parts, value)
}

fn set_value_recursive(dict: &mut Dictionary, parts: &[&str], value: Value) -> bool {
    if parts.is_empty() {
        return false;
    }

    let key = parts[0];

    if parts.len() == 1 {
        dict.insert(key.to_string(), value);
        return true;
    }

    // Need to descend into subdictionary
    // Check if key exists and is a dictionary
    if let Some(existing) = dict.get(key) {
        if let Some(sub) = existing.get::<Dictionary>() {
            let mut sub_clone = sub.clone();
            let result = set_value_recursive(&mut sub_clone, &parts[1..], value);
            dict.insert(key.to_string(), Value::from(sub_clone));
            return result;
        }
    }

    // Key doesn't exist or is not a dictionary - create new one
    let mut new_sub = Dictionary::new();
    let result = set_value_recursive(&mut new_sub, &parts[1..], value);
    dict.insert(key.to_string(), Value::from(new_sub));
    result
}

/// Clear a value from a dictionary at the given ':'-separated key path.
fn clear_value_at_key_path(dict: &mut Dictionary, key_path: &str) -> bool {
    let parts: Vec<&str> = key_path.split(':').collect();
    clear_value_recursive(dict, &parts)
}

fn clear_value_recursive(dict: &mut Dictionary, parts: &[&str]) -> bool {
    if parts.is_empty() {
        return false;
    }

    let key = parts[0];

    if parts.len() == 1 {
        dict.remove(key);
        return true;
    }

    // Need to descend into subdictionary
    if let Some(existing) = dict.get(key) {
        if let Some(sub) = existing.get::<Dictionary>() {
            let mut sub_clone = sub.clone();
            let result = clear_value_recursive(&mut sub_clone, &parts[1..]);
            // Clean up empty subdictionaries
            if sub_clone.is_empty() {
                dict.remove(key);
            } else {
                dict.insert(key.to_string(), Value::from(sub_clone));
            }
            return result;
        }
    }

    false
}

// =============================================================================
// Trait implementations
// =============================================================================

impl PartialEq for Object {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
            && self.obj_type == other.obj_type
            && self.stage.ptr_eq(&other.stage)
    }
}

impl Eq for Object {}

impl PartialOrd for Object {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Object {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.path.cmp(&other.path)
    }
}

impl std::hash::Hash for Object {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.obj_type.hash(state);
        self.path.hash(state);
    }
}

impl Default for Object {
    fn default() -> Self {
        Self::invalid()
    }
}

// =============================================================================
// UsdObject enum — tagged union for Prim/Attribute/Relationship
// =============================================================================

/// Tagged union representing any USD object type (Prim, Attribute, Relationship).
///
/// Used by `Prim::get_object_at_path()` and `Stage::get_object_at_path()`.
#[derive(Debug, Clone)]
pub enum UsdObject {
    Prim(super::prim::Prim),
    Attribute(super::attribute::Attribute),
    Relationship(super::relationship::Relationship),
}

impl UsdObject {
    pub fn is_prim(&self) -> bool {
        matches!(self, UsdObject::Prim(_))
    }

    pub fn is_attribute(&self) -> bool {
        matches!(self, UsdObject::Attribute(_))
    }

    pub fn is_relationship(&self) -> bool {
        matches!(self, UsdObject::Relationship(_))
    }

    pub fn as_prim(&self) -> Option<&super::prim::Prim> {
        match self {
            UsdObject::Prim(p) => Some(p),
            _ => None,
        }
    }

    pub fn as_attribute(&self) -> Option<&super::attribute::Attribute> {
        match self {
            UsdObject::Attribute(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_relationship(&self) -> Option<&super::relationship::Relationship> {
        match self {
            UsdObject::Relationship(r) => Some(r),
            _ => None,
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_object() {
        let obj = Object::invalid();
        assert!(!obj.is_valid());
    }

    #[test]
    fn test_object_path() {
        let path = Path::from_string("/World").unwrap();
        let obj = Object::new(Weak::new(), path.clone());
        assert_eq!(obj.path(), &path);
    }

    #[test]
    fn test_prim_path_for_property() {
        let path = Path::from_string("/World.visibility").unwrap();
        let obj = Object::new(Weak::new(), path);
        let prim_path = obj.prim_path();
        assert_eq!(prim_path.get_string(), "/World");
    }

    #[test]
    fn test_obj_type_subtype() {
        assert!(ObjType::is_subtype(ObjType::Object, ObjType::Prim));
        assert!(ObjType::is_subtype(ObjType::Object, ObjType::Attribute));
        assert!(ObjType::is_subtype(ObjType::Property, ObjType::Attribute));
        assert!(ObjType::is_subtype(
            ObjType::Property,
            ObjType::Relationship
        ));
        assert!(!ObjType::is_subtype(ObjType::Prim, ObjType::Attribute));
    }

    #[test]
    fn test_obj_type_concrete() {
        assert!(ObjType::is_concrete(ObjType::Prim));
        assert!(ObjType::is_concrete(ObjType::Attribute));
        assert!(ObjType::is_concrete(ObjType::Relationship));
        assert!(!ObjType::is_concrete(ObjType::Object));
        assert!(!ObjType::is_concrete(ObjType::Property));
    }

    #[test]
    fn test_dict_key_path() {
        let mut dict = Dictionary::new();
        dict.insert("foo".to_string(), Value::from(42i32));

        let mut sub = Dictionary::new();
        sub.insert("baz".to_string(), Value::from("hello".to_string()));
        dict.insert("bar".to_string(), Value::from(sub));

        // Test get
        let val = get_value_at_key_path(&dict, "foo");
        assert_eq!(val.and_then(|v| v.get::<i32>().cloned()), Some(42));

        let val = get_value_at_key_path(&dict, "bar:baz");
        assert_eq!(
            val.and_then(|v| v.get::<String>().cloned()),
            Some("hello".to_string())
        );

        // Test set
        let mut dict2 = Dictionary::new();
        set_value_at_key_path(&mut dict2, "a:b:c", Value::from(123i32));
        let val = get_value_at_key_path(&dict2, "a:b:c");
        assert_eq!(val.and_then(|v| v.get::<i32>().cloned()), Some(123));

        // Test clear
        clear_value_at_key_path(&mut dict2, "a:b:c");
        assert!(get_value_at_key_path(&dict2, "a:b:c").is_none());
    }

    #[test]
    fn test_object_description() {
        let path = Path::from_string("/World/Cube").unwrap();
        let obj = Object::new_with_type(Weak::new(), path, ObjType::Prim);
        // Object without stage shows as "expired"
        assert!(obj.description().contains("prim"));
        assert!(obj.description().contains("/World/Cube"));

        let invalid = Object::invalid();
        assert_eq!(invalid.description(), "Invalid object");
    }

    // M1: Object::is_valid() type-specific validation
    #[test]
    fn test_is_valid_object_type_false() {
        // Abstract ObjType::Object must always be invalid
        let obj = Object::invalid();
        assert_eq!(obj.obj_type(), ObjType::Object);
        assert!(!obj.is_valid());
    }

    #[test]
    fn test_is_valid_prim_no_stage() {
        // Prim with dead stage ref is invalid
        let path = Path::from_string("/World").unwrap();
        let obj = Object::new_with_type(Weak::new(), path, ObjType::Prim);
        assert!(!obj.is_valid());
    }

    #[test]
    fn test_is_valid_attribute_needs_spec() {
        use super::super::common::InitialLoadSet;
        use super::super::stage::Stage;

        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let _prim = stage.define_prim("/World", "Xform").unwrap();

        // Attribute that doesn't exist in any layer => invalid
        let path = Path::from_string("/World.nonExistentAttr").unwrap();
        let obj = Object::new_with_type(Arc::downgrade(&stage), path, ObjType::Attribute);
        assert!(!obj.is_valid());
    }

    #[test]
    fn test_is_valid_property_needs_spec() {
        use super::super::common::InitialLoadSet;
        use super::super::stage::Stage;

        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let prim = stage.define_prim("/World", "Xform").unwrap();

        // Create an actual attribute so it IS valid
        use usd_sdf::value_type_registry::ValueTypeRegistry;
        let type_name = ValueTypeRegistry::instance().find_type("float");
        let attr = prim.create_attribute("size", &type_name, false, None);
        assert!(attr.is_some());

        // Property type pointing at real attr => valid
        let path = Path::from_string("/World.size").unwrap();
        let obj = Object::new_with_type(Arc::downgrade(&stage), path, ObjType::Property);
        assert!(obj.is_valid());
    }
}
