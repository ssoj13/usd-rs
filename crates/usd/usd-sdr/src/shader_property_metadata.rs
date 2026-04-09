//! SDR Shader Property Metadata - Metadata container for shader properties.
//!
//! Port of pxr/usd/sdr/shaderPropertyMetadata.h
//!
//! This module provides SdrShaderPropertyMetadata which contains both generic
//! and named metadata for shader properties. Named metadata have standardized
//! types and semantics for interchange between different shader systems.
//!
//! Used by: SdrShaderProperty
//! Uses: Token, VtValue, VtDictionary

use super::declare::{SdrTokenMap, SdrTokenVec};
use super::tokens::tokens;
use std::collections::HashMap;
use usd_tf::Token;
use usd_vt::Value;

/// Parses bool-like legacy strings from shader parsers (`"1"`/`"0"`, etc.).
/// Matches OpenUSD `SdrShaderPropertyMetadata` legacy ingestion (`"1"` is true).
fn parse_legacy_bool_str(s: &str) -> Option<bool> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    let lower = t.to_ascii_lowercase();
    match lower.as_str() {
        "1" | "true" | "t" | "yes" => Some(true),
        "0" | "false" | "f" | "no" => Some(false),
        _ => t.parse().ok(),
    }
}

/// Metadata container for shader properties.
///
/// Contains both generic key-value metadata and named metadata with
/// specific types. Named metadata items provide Has/Set/Get methods
/// for type-safe access.
///
/// Named metadata includes:
/// - Label, Help, Page, Widget - UI-related metadata
/// - RenderType, Role - rendering metadata
/// - IsDynamicArray, TupleSize - array metadata
/// - Connectable, ValidConnectionTypes - connection metadata
/// - VStruct members - virtual struct metadata
/// - IsAssetIdentifier, DefaultInput - special flags
#[derive(Debug, Clone, Default)]
pub struct SdrShaderPropertyMetadata {
    /// Generic metadata items stored as a dictionary.
    items: HashMap<Token, Value>,
}

impl SdrShaderPropertyMetadata {
    /// Creates empty metadata.
    pub fn new() -> Self {
        Self {
            items: HashMap::new(),
        }
    }

    /// Creates metadata from a legacy SdrTokenMap.
    ///
    /// Attempts to convert string values to richer types for named metadata items.
    pub fn from_token_map(legacy: &SdrTokenMap) -> Self {
        let mut metadata = Self::new();
        for (key, value) in legacy {
            metadata
                .items
                .insert(key.clone(), Value::from(value.clone()));
        }
        metadata
    }

    /// Creates metadata from a HashMap of VtValues.
    pub fn from_items(items: HashMap<Token, Value>) -> Self {
        Self { items }
    }

    /// Returns whether this metadata contains an item with the given key.
    pub fn has_item(&self, key: &Token) -> bool {
        self.items.contains_key(key)
    }

    /// Sets a key-value item for this metadata.
    ///
    /// If the key-value item already exists, it will be overwritten.
    pub fn set_item(&mut self, key: Token, value: Value) {
        self.items.insert(key, value);
    }

    /// Gets the VtValue for the given key.
    ///
    /// Returns None if the key doesn't exist.
    pub fn get_item_value(&self, key: &Token) -> Option<&Value> {
        self.items.get(key)
    }

    /// Convenience method to get an item value as a specific type.
    ///
    /// Returns None if the key doesn't exist or conversion fails.
    pub fn get_item_value_as<T: Clone + 'static>(&self, key: &Token) -> Option<T> {
        self.items.get(key).and_then(|v| v.get::<T>().cloned())
    }

    /// Clears the metadata item for the given key if it exists.
    pub fn clear_item(&mut self, key: &Token) {
        self.items.remove(key);
    }

    /// Gets all key-value items.
    pub fn get_items(&self) -> &HashMap<Token, Value> {
        &self.items
    }

    // ========================================================================
    // Named metadata accessors - Label
    // ========================================================================

    /// Returns whether label metadata exists.
    pub fn has_label(&self) -> bool {
        self.has_item(&tokens().property_metadata.label)
    }

    /// Gets the label metadata value.
    pub fn get_label(&self) -> Token {
        self.get_item_value_as::<String>(&tokens().property_metadata.label)
            .map(|s| Token::new(&s))
            .unwrap_or_default()
    }

    /// Sets the label metadata value.
    pub fn set_label(&mut self, v: &Token) {
        self.set_item(
            tokens().property_metadata.label.clone(),
            Value::from(v.as_str().to_string()),
        );
    }

    /// Clears the label metadata.
    pub fn clear_label(&mut self) {
        self.clear_item(&tokens().property_metadata.label);
    }

    // ========================================================================
    // Named metadata accessors - Help
    // ========================================================================

    /// Returns whether help metadata exists.
    pub fn has_help(&self) -> bool {
        self.has_item(&tokens().property_metadata.help)
    }

    /// Gets the help metadata value.
    pub fn get_help(&self) -> String {
        self.get_item_value_as::<String>(&tokens().property_metadata.help)
            .unwrap_or_default()
    }

    /// Sets the help metadata value.
    pub fn set_help(&mut self, v: &str) {
        self.set_item(
            tokens().property_metadata.help.clone(),
            Value::from(v.to_string()),
        );
    }

    /// Clears the help metadata.
    pub fn clear_help(&mut self) {
        self.clear_item(&tokens().property_metadata.help);
    }

    // ========================================================================
    // Named metadata accessors - Page
    // ========================================================================

    /// Returns whether page metadata exists.
    pub fn has_page(&self) -> bool {
        self.has_item(&tokens().property_metadata.page)
    }

    /// Gets the page metadata value.
    pub fn get_page(&self) -> Token {
        self.get_item_value_as::<String>(&tokens().property_metadata.page)
            .map(|s| Token::new(&s))
            .unwrap_or_default()
    }

    /// Sets the page metadata value.
    pub fn set_page(&mut self, v: &Token) {
        self.set_item(
            tokens().property_metadata.page.clone(),
            Value::from(v.as_str().to_string()),
        );
    }

    /// Clears the page metadata.
    pub fn clear_page(&mut self) {
        self.clear_item(&tokens().property_metadata.page);
    }

    // ========================================================================
    // Named metadata accessors - RenderType
    // ========================================================================

    /// Returns whether render type metadata exists.
    pub fn has_render_type(&self) -> bool {
        self.has_item(&tokens().property_metadata.render_type)
    }

    /// Gets the render type metadata value.
    pub fn get_render_type(&self) -> String {
        self.get_item_value_as::<String>(&tokens().property_metadata.render_type)
            .unwrap_or_default()
    }

    /// Sets the render type metadata value.
    pub fn set_render_type(&mut self, v: &str) {
        self.set_item(
            tokens().property_metadata.render_type.clone(),
            Value::from(v.to_string()),
        );
    }

    /// Clears the render type metadata.
    pub fn clear_render_type(&mut self) {
        self.clear_item(&tokens().property_metadata.render_type);
    }

    // ========================================================================
    // Named metadata accessors - Role
    // ========================================================================

    /// Returns whether role metadata exists.
    pub fn has_role(&self) -> bool {
        self.has_item(&tokens().property_metadata.role)
    }

    /// Gets the role metadata value.
    pub fn get_role(&self) -> String {
        self.get_item_value_as::<String>(&tokens().property_metadata.role)
            .unwrap_or_default()
    }

    /// Sets the role metadata value.
    pub fn set_role(&mut self, v: &str) {
        self.set_item(
            tokens().property_metadata.role.clone(),
            Value::from(v.to_string()),
        );
    }

    /// Clears the role metadata.
    pub fn clear_role(&mut self) {
        self.clear_item(&tokens().property_metadata.role);
    }

    // ========================================================================
    // Named metadata accessors - Widget
    // ========================================================================

    /// Returns whether widget metadata exists.
    pub fn has_widget(&self) -> bool {
        self.has_item(&tokens().property_metadata.widget)
    }

    /// Gets the widget metadata value.
    pub fn get_widget(&self) -> Token {
        self.get_item_value_as::<String>(&tokens().property_metadata.widget)
            .map(|s| Token::new(&s))
            .unwrap_or_default()
    }

    /// Sets the widget metadata value.
    pub fn set_widget(&mut self, v: &Token) {
        self.set_item(
            tokens().property_metadata.widget.clone(),
            Value::from(v.as_str().to_string()),
        );
    }

    /// Clears the widget metadata.
    pub fn clear_widget(&mut self) {
        self.clear_item(&tokens().property_metadata.widget);
    }

    // ========================================================================
    // Named metadata accessors - IsDynamicArray
    // ========================================================================

    /// Returns whether isDynamicArray metadata exists.
    pub fn has_is_dynamic_array(&self) -> bool {
        self.has_item(&tokens().property_metadata.is_dynamic_array)
    }

    /// Gets the isDynamicArray metadata value.
    pub fn get_is_dynamic_array(&self) -> bool {
        self.get_item_value_as::<bool>(&tokens().property_metadata.is_dynamic_array)
            .or_else(|| {
                self.get_item_value_as::<String>(&tokens().property_metadata.is_dynamic_array)
                    .and_then(|s| parse_legacy_bool_str(&s))
            })
            .unwrap_or(false)
    }

    /// Sets the isDynamicArray metadata value.
    pub fn set_is_dynamic_array(&mut self, v: bool) {
        self.set_item(
            tokens().property_metadata.is_dynamic_array.clone(),
            Value::from(v),
        );
    }

    /// Clears the isDynamicArray metadata.
    pub fn clear_is_dynamic_array(&mut self) {
        self.clear_item(&tokens().property_metadata.is_dynamic_array);
    }

    // ========================================================================
    // Named metadata accessors - TupleSize
    // ========================================================================

    /// Returns whether tupleSize metadata exists.
    pub fn has_tuple_size(&self) -> bool {
        self.has_item(&tokens().property_metadata.tuple_size)
    }

    /// Gets the tupleSize metadata value.
    pub fn get_tuple_size(&self) -> i32 {
        self.get_item_value_as::<i32>(&tokens().property_metadata.tuple_size)
            .or_else(|| {
                // Try parsing from string
                self.get_item_value_as::<String>(&tokens().property_metadata.tuple_size)
                    .and_then(|s| s.parse().ok())
            })
            .unwrap_or(0)
    }

    /// Sets the tupleSize metadata value.
    pub fn set_tuple_size(&mut self, v: i32) {
        self.set_item(
            tokens().property_metadata.tuple_size.clone(),
            Value::from(v),
        );
    }

    /// Clears the tupleSize metadata.
    pub fn clear_tuple_size(&mut self) {
        self.clear_item(&tokens().property_metadata.tuple_size);
    }

    // ========================================================================
    // Named metadata accessors - Connectable
    // ========================================================================

    /// Returns whether connectable metadata exists.
    pub fn has_connectable(&self) -> bool {
        self.has_item(&tokens().property_metadata.connectable)
    }

    /// Gets the connectable metadata value.
    pub fn get_connectable(&self) -> bool {
        self.get_item_value_as::<bool>(&tokens().property_metadata.connectable)
            .or_else(|| {
                self.get_item_value_as::<String>(&tokens().property_metadata.connectable)
                    .and_then(|s| parse_legacy_bool_str(&s))
            })
            .unwrap_or(true) // Default is connectable
    }

    /// Sets the connectable metadata value.
    pub fn set_connectable(&mut self, v: bool) {
        self.set_item(
            tokens().property_metadata.connectable.clone(),
            Value::from(v),
        );
    }

    /// Clears the connectable metadata.
    pub fn clear_connectable(&mut self) {
        self.clear_item(&tokens().property_metadata.connectable);
    }

    // ========================================================================
    // Named metadata accessors - ShownIf
    // ========================================================================

    /// Returns whether shownIf metadata exists.
    pub fn has_shown_if(&self) -> bool {
        self.has_item(&tokens().property_metadata.shown_if)
    }

    /// Gets the shownIf metadata value.
    pub fn get_shown_if(&self) -> String {
        self.get_item_value_as::<String>(&tokens().property_metadata.shown_if)
            .unwrap_or_default()
    }

    /// Sets the shownIf metadata value.
    pub fn set_shown_if(&mut self, v: &str) {
        self.set_item(
            tokens().property_metadata.shown_if.clone(),
            Value::from(v.to_string()),
        );
    }

    /// Clears the shownIf metadata.
    pub fn clear_shown_if(&mut self) {
        self.clear_item(&tokens().property_metadata.shown_if);
    }

    // ========================================================================
    // Named metadata accessors - ValidConnectionTypes
    // ========================================================================

    /// Returns whether validConnectionTypes metadata exists.
    pub fn has_valid_connection_types(&self) -> bool {
        self.has_item(&tokens().property_metadata.valid_connection_types)
    }

    /// Gets the validConnectionTypes metadata value.
    pub fn get_valid_connection_types(&self) -> SdrTokenVec {
        // Try to get as a vector of strings first
        if let Some(strings) = self
            .get_item_value_as::<Vec<String>>(&tokens().property_metadata.valid_connection_types)
        {
            return strings.into_iter().map(|s| Token::new(&s)).collect();
        }
        // Try parsing from comma-separated string
        if let Some(s) =
            self.get_item_value_as::<String>(&tokens().property_metadata.valid_connection_types)
        {
            return s.split(',').map(|part| Token::new(part.trim())).collect();
        }
        Vec::new()
    }

    /// Sets the validConnectionTypes metadata value.
    pub fn set_valid_connection_types(&mut self, v: &SdrTokenVec) {
        let strings: Vec<String> = v.iter().map(|t| t.as_str().to_string()).collect();
        self.set_item(
            tokens().property_metadata.valid_connection_types.clone(),
            Value::from(strings),
        );
    }

    /// Clears the validConnectionTypes metadata.
    pub fn clear_valid_connection_types(&mut self) {
        self.clear_item(&tokens().property_metadata.valid_connection_types);
    }

    // ========================================================================
    // Named metadata accessors - IsAssetIdentifier
    // ========================================================================

    /// Returns whether isAssetIdentifier metadata exists.
    pub fn has_is_asset_identifier(&self) -> bool {
        self.has_item(&tokens().property_metadata.is_asset_identifier)
    }

    /// Gets the isAssetIdentifier metadata value.
    ///
    /// The OSL parser sets `__SDR__isAssetIdentifier` to an **empty** string as a
    /// presence flag (OpenUSD `inject_parser_metadata`); treat that as `true`.
    pub fn get_is_asset_identifier(&self) -> bool {
        if !self.has_is_asset_identifier() {
            return false;
        }
        if let Some(b) = self.get_item_value_as::<bool>(&tokens().property_metadata.is_asset_identifier) {
            return b;
        }
        if let Some(s) = self.get_item_value_as::<String>(&tokens().property_metadata.is_asset_identifier) {
            if s.is_empty() {
                return true;
            }
            return parse_legacy_bool_str(&s).unwrap_or(false);
        }
        true
    }

    /// Sets the isAssetIdentifier metadata value.
    pub fn set_is_asset_identifier(&mut self, v: bool) {
        self.set_item(
            tokens().property_metadata.is_asset_identifier.clone(),
            Value::from(v),
        );
    }

    /// Clears the isAssetIdentifier metadata.
    pub fn clear_is_asset_identifier(&mut self) {
        self.clear_item(&tokens().property_metadata.is_asset_identifier);
    }

    // ========================================================================
    // Named metadata accessors - ImplementationName
    // ========================================================================

    /// Returns whether implementationName metadata exists.
    pub fn has_implementation_name(&self) -> bool {
        self.has_item(&tokens().property_metadata.implementation_name)
    }

    /// Gets the implementationName metadata value.
    pub fn get_implementation_name(&self) -> String {
        self.get_item_value_as::<String>(&tokens().property_metadata.implementation_name)
            .unwrap_or_default()
    }

    /// Sets the implementationName metadata value.
    pub fn set_implementation_name(&mut self, v: &str) {
        self.set_item(
            tokens().property_metadata.implementation_name.clone(),
            Value::from(v.to_string()),
        );
    }

    /// Clears the implementationName metadata.
    pub fn clear_implementation_name(&mut self) {
        self.clear_item(&tokens().property_metadata.implementation_name);
    }

    // ========================================================================
    // Named metadata accessors - SdrUsdDefinitionType
    // ========================================================================

    /// Returns whether sdrUsdDefinitionType metadata exists.
    pub fn has_sdr_usd_definition_type(&self) -> bool {
        self.has_item(&tokens().property_metadata.sdr_usd_definition_type)
    }

    /// Gets the sdrUsdDefinitionType metadata value.
    pub fn get_sdr_usd_definition_type(&self) -> Token {
        self.get_item_value_as::<String>(&tokens().property_metadata.sdr_usd_definition_type)
            .map(|s| Token::new(&s))
            .unwrap_or_default()
    }

    /// Sets the sdrUsdDefinitionType metadata value.
    pub fn set_sdr_usd_definition_type(&mut self, v: &Token) {
        self.set_item(
            tokens().property_metadata.sdr_usd_definition_type.clone(),
            Value::from(v.as_str().to_string()),
        );
    }

    /// Clears the sdrUsdDefinitionType metadata.
    pub fn clear_sdr_usd_definition_type(&mut self) {
        self.clear_item(&tokens().property_metadata.sdr_usd_definition_type);
    }

    // ========================================================================
    // Named metadata accessors - DefaultInput
    // ========================================================================

    /// Returns whether defaultInput metadata exists.
    pub fn has_default_input(&self) -> bool {
        self.has_item(&tokens().property_metadata.default_input)
    }

    /// Gets the defaultInput metadata value.
    pub fn get_default_input(&self) -> bool {
        self.get_item_value_as::<bool>(&tokens().property_metadata.default_input)
            .or_else(|| {
                self.get_item_value_as::<String>(&tokens().property_metadata.default_input)
                    .and_then(|s| s.parse().ok())
            })
            .unwrap_or(false)
    }

    /// Sets the defaultInput metadata value.
    pub fn set_default_input(&mut self, v: bool) {
        self.set_item(
            tokens().property_metadata.default_input.clone(),
            Value::from(v),
        );
    }

    /// Clears the defaultInput metadata.
    pub fn clear_default_input(&mut self) {
        self.clear_item(&tokens().property_metadata.default_input);
    }

    // ========================================================================
    // Named metadata accessors - Colorspace
    // ========================================================================

    /// Returns whether colorspace metadata exists.
    pub fn has_colorspace(&self) -> bool {
        self.has_item(&tokens().property_metadata.colorspace)
    }

    /// Gets the colorspace metadata value.
    pub fn get_colorspace(&self) -> Token {
        self.get_item_value_as::<String>(&tokens().property_metadata.colorspace)
            .map(|s| Token::new(&s))
            .unwrap_or_default()
    }

    /// Sets the colorspace metadata value.
    pub fn set_colorspace(&mut self, v: &Token) {
        self.set_item(
            tokens().property_metadata.colorspace.clone(),
            Value::from(v.as_str().to_string()),
        );
    }

    /// Clears the colorspace metadata.
    pub fn clear_colorspace(&mut self) {
        self.clear_item(&tokens().property_metadata.colorspace);
    }

    // ========================================================================
    // Legacy metadata encoding (for backwards compatibility)
    // ========================================================================

    /// Encodes metadata to legacy SdrTokenMap format.
    ///
    /// Unnamed metadata with non-string values are not returned.
    pub fn encode_legacy_metadata(&self) -> SdrTokenMap {
        let mut result = SdrTokenMap::new();
        for (key, value) in &self.items {
            if let Some(s) = value.get::<String>() {
                result.insert(key.clone(), s.clone());
            } else if let Some(b) = value.get::<bool>() {
                result.insert(key.clone(), b.to_string());
            } else if let Some(i) = value.get::<i32>() {
                result.insert(key.clone(), i.to_string());
            }
        }
        result
    }
}

impl From<SdrTokenMap> for SdrShaderPropertyMetadata {
    fn from(legacy: SdrTokenMap) -> Self {
        Self::from_token_map(&legacy)
    }
}

impl From<&SdrTokenMap> for SdrShaderPropertyMetadata {
    fn from(legacy: &SdrTokenMap) -> Self {
        Self::from_token_map(legacy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_metadata() {
        let mut metadata = SdrShaderPropertyMetadata::new();

        assert!(!metadata.has_label());
        metadata.set_label(&Token::new("My Label"));
        assert!(metadata.has_label());
        assert_eq!(metadata.get_label().as_str(), "My Label");

        metadata.clear_label();
        assert!(!metadata.has_label());
    }

    #[test]
    fn test_from_token_map() {
        let mut legacy = SdrTokenMap::new();
        legacy.insert(Token::new("label"), "Test Label".to_string());
        legacy.insert(Token::new("help"), "Help text".to_string());

        let metadata = SdrShaderPropertyMetadata::from_token_map(&legacy);
        assert_eq!(metadata.get_label().as_str(), "Test Label");
        assert_eq!(metadata.get_help(), "Help text");
    }

    #[test]
    fn test_boolean_metadata() {
        let mut metadata = SdrShaderPropertyMetadata::new();

        // Test default value (true for connectable)
        assert!(metadata.get_connectable());

        metadata.set_connectable(false);
        assert!(!metadata.get_connectable());
    }
}
