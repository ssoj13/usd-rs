//! SDR Shader Node Metadata - Metadata container for shader nodes.
//!
//! Port of pxr/usd/sdr/shaderNodeMetadata.h
//!
//! This module provides SdrShaderNodeMetadata which contains both generic
//! and named metadata for shader nodes. Named metadata have standardized
//! types and semantics for interchange between different shader systems.
//!
//! Used by: SdrShaderNode
//! Uses: Token, VtValue, VtDictionary

use super::declare::{SdrStringVec, SdrTokenMap, SdrTokenVec};
use super::tokens::tokens;
use std::collections::HashMap;
use usd_tf::Token;
use usd_vt::Value;

/// Metadata container for shader nodes.
///
/// Contains both generic key-value metadata and named metadata with
/// specific types. Named metadata items provide Has/Set/Get methods
/// for type-safe access.
///
/// Named metadata includes:
/// - Label, Category, Role - identification metadata
/// - Help, Departments - documentation metadata
/// - Pages, OpenPages, PagesShownIf - UI organization metadata
/// - Primvars - required primvars
/// - ImplementationName - implementation details
/// - SdrUsdEncodingVersion - encoding version
/// - SdrDefinitionNameFallbackPrefix - fallback naming
#[derive(Debug, Clone, Default)]
pub struct SdrShaderNodeMetadata {
    /// Generic metadata items stored as a dictionary.
    items: HashMap<Token, Value>,
}

impl SdrShaderNodeMetadata {
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
        self.has_item(&tokens().node_metadata.label)
    }

    /// Gets the label metadata value.
    pub fn get_label(&self) -> Token {
        self.get_item_value_as::<String>(&tokens().node_metadata.label)
            .map(|s| Token::new(&s))
            .unwrap_or_default()
    }

    /// Sets the label metadata value.
    pub fn set_label(&mut self, v: &Token) {
        self.set_item(
            tokens().node_metadata.label.clone(),
            Value::from(v.as_str().to_string()),
        );
    }

    /// Clears the label metadata.
    pub fn clear_label(&mut self) {
        self.clear_item(&tokens().node_metadata.label);
    }

    // ========================================================================
    // Named metadata accessors - Category
    // ========================================================================

    /// Returns whether category metadata exists.
    pub fn has_category(&self) -> bool {
        self.has_item(&tokens().node_metadata.category)
    }

    /// Gets the category metadata value.
    pub fn get_category(&self) -> Token {
        self.get_item_value_as::<String>(&tokens().node_metadata.category)
            .map(|s| Token::new(&s))
            .unwrap_or_default()
    }

    /// Sets the category metadata value.
    pub fn set_category(&mut self, v: &Token) {
        self.set_item(
            tokens().node_metadata.category.clone(),
            Value::from(v.as_str().to_string()),
        );
    }

    /// Clears the category metadata.
    pub fn clear_category(&mut self) {
        self.clear_item(&tokens().node_metadata.category);
    }

    // ========================================================================
    // Named metadata accessors - Role
    // ========================================================================

    /// Returns whether role metadata exists.
    ///
    /// An empty Token value for Role indicates non-existence.
    pub fn has_role(&self) -> bool {
        if !self.has_item(&tokens().node_metadata.role) {
            return false;
        }
        // Empty role is considered non-existent
        !self.get_role().as_str().is_empty()
    }

    /// Gets the role metadata value.
    pub fn get_role(&self) -> Token {
        self.get_item_value_as::<String>(&tokens().node_metadata.role)
            .map(|s| Token::new(&s))
            .unwrap_or_default()
    }

    /// Sets the role metadata value.
    ///
    /// If given an empty string, clears the Role item.
    pub fn set_role(&mut self, v: &Token) {
        if v.as_str().is_empty() {
            self.clear_role();
        } else {
            self.set_item(
                tokens().node_metadata.role.clone(),
                Value::from(v.as_str().to_string()),
            );
        }
    }

    /// Clears the role metadata.
    pub fn clear_role(&mut self) {
        self.clear_item(&tokens().node_metadata.role);
    }

    // ========================================================================
    // Named metadata accessors - Help
    // ========================================================================

    /// Returns whether help metadata exists.
    pub fn has_help(&self) -> bool {
        self.has_item(&tokens().node_metadata.help)
    }

    /// Gets the help metadata value.
    pub fn get_help(&self) -> String {
        self.get_item_value_as::<String>(&tokens().node_metadata.help)
            .unwrap_or_default()
    }

    /// Sets the help metadata value.
    pub fn set_help(&mut self, v: &str) {
        self.set_item(
            tokens().node_metadata.help.clone(),
            Value::from(v.to_string()),
        );
    }

    /// Clears the help metadata.
    pub fn clear_help(&mut self) {
        self.clear_item(&tokens().node_metadata.help);
    }

    // ========================================================================
    // Named metadata accessors - Departments
    // ========================================================================

    /// Returns whether departments metadata exists.
    pub fn has_departments(&self) -> bool {
        self.has_item(&tokens().node_metadata.departments)
    }

    /// Gets the departments metadata value.
    pub fn get_departments(&self) -> SdrTokenVec {
        if let Some(strings) =
            self.get_item_value_as::<Vec<String>>(&tokens().node_metadata.departments)
        {
            return strings.into_iter().map(|s| Token::new(&s)).collect();
        }
        // Try parsing from comma-separated string
        if let Some(s) = self.get_item_value_as::<String>(&tokens().node_metadata.departments) {
            return s.split(',').map(|part| Token::new(part.trim())).collect();
        }
        Vec::new()
    }

    /// Sets the departments metadata value.
    pub fn set_departments(&mut self, v: &SdrTokenVec) {
        let strings: Vec<String> = v.iter().map(|t| t.as_str().to_string()).collect();
        self.set_item(
            tokens().node_metadata.departments.clone(),
            Value::from(strings),
        );
    }

    /// Clears the departments metadata.
    pub fn clear_departments(&mut self) {
        self.clear_item(&tokens().node_metadata.departments);
    }

    // ========================================================================
    // Named metadata accessors - Pages (deprecated)
    // ========================================================================

    /// Returns whether pages metadata exists.
    #[deprecated(
        note = "SdrShaderNode::GetPages is computed via SdrShaderProperty's Pages metadata"
    )]
    pub fn has_pages(&self) -> bool {
        self.has_item(&tokens().node_metadata.pages)
    }

    /// Gets the pages metadata value.
    #[deprecated(
        note = "SdrShaderNode::GetPages is computed via SdrShaderProperty's Pages metadata"
    )]
    pub fn get_pages(&self) -> SdrTokenVec {
        if let Some(strings) = self.get_item_value_as::<Vec<String>>(&tokens().node_metadata.pages)
        {
            return strings.into_iter().map(|s| Token::new(&s)).collect();
        }
        if let Some(s) = self.get_item_value_as::<String>(&tokens().node_metadata.pages) {
            return s.split(',').map(|part| Token::new(part.trim())).collect();
        }
        Vec::new()
    }

    /// Sets the pages metadata value.
    #[deprecated(
        note = "SdrShaderNode::GetPages is computed via SdrShaderProperty's Pages metadata"
    )]
    #[allow(deprecated)]
    pub fn set_pages(&mut self, v: &SdrTokenVec) {
        let strings: Vec<String> = v.iter().map(|t| t.as_str().to_string()).collect();
        self.set_item(tokens().node_metadata.pages.clone(), Value::from(strings));
    }

    /// Clears the pages metadata.
    #[deprecated(
        note = "SdrShaderNode::GetPages is computed via SdrShaderProperty's Pages metadata"
    )]
    pub fn clear_pages(&mut self) {
        self.clear_item(&tokens().node_metadata.pages);
    }

    // ========================================================================
    // Named metadata accessors - OpenPages
    // ========================================================================

    /// Returns whether openPages metadata exists.
    pub fn has_open_pages(&self) -> bool {
        self.has_item(&tokens().node_metadata.open_pages)
    }

    /// Gets the openPages metadata value.
    pub fn get_open_pages(&self) -> SdrTokenVec {
        if let Some(strings) =
            self.get_item_value_as::<Vec<String>>(&tokens().node_metadata.open_pages)
        {
            return strings.into_iter().map(|s| Token::new(&s)).collect();
        }
        if let Some(s) = self.get_item_value_as::<String>(&tokens().node_metadata.open_pages) {
            return s.split(',').map(|part| Token::new(part.trim())).collect();
        }
        Vec::new()
    }

    /// Sets the openPages metadata value.
    pub fn set_open_pages(&mut self, v: &SdrTokenVec) {
        let strings: Vec<String> = v.iter().map(|t| t.as_str().to_string()).collect();
        self.set_item(
            tokens().node_metadata.open_pages.clone(),
            Value::from(strings),
        );
    }

    /// Clears the openPages metadata.
    pub fn clear_open_pages(&mut self) {
        self.clear_item(&tokens().node_metadata.open_pages);
    }

    // ========================================================================
    // Named metadata accessors - PagesShownIf
    // ========================================================================

    /// Returns whether pagesShownIf metadata exists.
    pub fn has_pages_shown_if(&self) -> bool {
        self.has_item(&tokens().node_metadata.pages_shown_if)
    }

    /// Gets the pagesShownIf metadata value.
    ///
    /// Each key is a page name, each value is a "shownIf" expression.
    pub fn get_pages_shown_if(&self) -> SdrTokenMap {
        // Try to get from stored map
        if let Some(map) = self
            .get_item_value_as::<HashMap<String, String>>(&tokens().node_metadata.pages_shown_if)
        {
            return map.into_iter().map(|(k, v)| (Token::new(&k), v)).collect();
        }
        SdrTokenMap::new()
    }

    /// Sets the pagesShownIf metadata value.
    pub fn set_pages_shown_if(&mut self, v: &SdrTokenMap) {
        let map: HashMap<String, String> = v
            .iter()
            .map(|(k, v)| (k.as_str().to_string(), v.clone()))
            .collect();
        self.set_item(
            tokens().node_metadata.pages_shown_if.clone(),
            Value::from(map),
        );
    }

    /// Clears the pagesShownIf metadata.
    pub fn clear_pages_shown_if(&mut self) {
        self.clear_item(&tokens().node_metadata.pages_shown_if);
    }

    // ========================================================================
    // Named metadata accessors - Primvars
    // ========================================================================

    /// Returns whether primvars metadata exists.
    pub fn has_primvars(&self) -> bool {
        self.has_item(&tokens().node_metadata.primvars)
    }

    /// Gets the primvars metadata value.
    pub fn get_primvars(&self) -> SdrStringVec {
        if let Some(strings) =
            self.get_item_value_as::<Vec<String>>(&tokens().node_metadata.primvars)
        {
            return strings;
        }
        if let Some(s) = self.get_item_value_as::<String>(&tokens().node_metadata.primvars) {
            return s.split(',').map(|part| part.trim().to_string()).collect();
        }
        Vec::new()
    }

    /// Sets the primvars metadata value.
    pub fn set_primvars(&mut self, v: &SdrStringVec) {
        self.set_item(
            tokens().node_metadata.primvars.clone(),
            Value::from(v.clone()),
        );
    }

    /// Clears the primvars metadata.
    pub fn clear_primvars(&mut self) {
        self.clear_item(&tokens().node_metadata.primvars);
    }

    // ========================================================================
    // Named metadata accessors - ImplementationName
    // ========================================================================

    /// Returns whether implementationName metadata exists.
    pub fn has_implementation_name(&self) -> bool {
        self.has_item(&tokens().node_metadata.implementation_name)
    }

    /// Gets the implementationName metadata value.
    pub fn get_implementation_name(&self) -> String {
        self.get_item_value_as::<String>(&tokens().node_metadata.implementation_name)
            .unwrap_or_default()
    }

    /// Sets the implementationName metadata value.
    pub fn set_implementation_name(&mut self, v: &str) {
        self.set_item(
            tokens().node_metadata.implementation_name.clone(),
            Value::from(v.to_string()),
        );
    }

    /// Clears the implementationName metadata.
    pub fn clear_implementation_name(&mut self) {
        self.clear_item(&tokens().node_metadata.implementation_name);
    }

    // ========================================================================
    // Named metadata accessors - SdrUsdEncodingVersion
    // ========================================================================

    /// Returns whether sdrUsdEncodingVersion metadata exists.
    pub fn has_sdr_usd_encoding_version(&self) -> bool {
        self.has_item(&tokens().node_metadata.sdr_usd_encoding_version)
    }

    /// Gets the sdrUsdEncodingVersion metadata value.
    pub fn get_sdr_usd_encoding_version(&self) -> i32 {
        self.get_item_value_as::<i32>(&tokens().node_metadata.sdr_usd_encoding_version)
            .or_else(|| {
                self.get_item_value_as::<String>(&tokens().node_metadata.sdr_usd_encoding_version)
                    .and_then(|s| s.parse().ok())
            })
            .unwrap_or(0)
    }

    /// Sets the sdrUsdEncodingVersion metadata value.
    pub fn set_sdr_usd_encoding_version(&mut self, v: i32) {
        self.set_item(
            tokens().node_metadata.sdr_usd_encoding_version.clone(),
            Value::from(v),
        );
    }

    /// Clears the sdrUsdEncodingVersion metadata.
    pub fn clear_sdr_usd_encoding_version(&mut self) {
        self.clear_item(&tokens().node_metadata.sdr_usd_encoding_version);
    }

    // ========================================================================
    // Named metadata accessors - SdrDefinitionNameFallbackPrefix
    // ========================================================================

    /// Returns whether sdrDefinitionNameFallbackPrefix metadata exists.
    pub fn has_sdr_definition_name_fallback_prefix(&self) -> bool {
        self.has_item(&tokens().node_metadata.sdr_definition_name_fallback_prefix)
    }

    /// Gets the sdrDefinitionNameFallbackPrefix metadata value.
    pub fn get_sdr_definition_name_fallback_prefix(&self) -> String {
        self.get_item_value_as::<String>(
            &tokens().node_metadata.sdr_definition_name_fallback_prefix,
        )
        .unwrap_or_default()
    }

    /// Sets the sdrDefinitionNameFallbackPrefix metadata value.
    pub fn set_sdr_definition_name_fallback_prefix(&mut self, v: &str) {
        self.set_item(
            tokens()
                .node_metadata
                .sdr_definition_name_fallback_prefix
                .clone(),
            Value::from(v.to_string()),
        );
    }

    /// Clears the sdrDefinitionNameFallbackPrefix metadata.
    pub fn clear_sdr_definition_name_fallback_prefix(&mut self) {
        self.clear_item(&tokens().node_metadata.sdr_definition_name_fallback_prefix);
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

impl From<SdrTokenMap> for SdrShaderNodeMetadata {
    fn from(legacy: SdrTokenMap) -> Self {
        Self::from_token_map(&legacy)
    }
}

impl From<&SdrTokenMap> for SdrShaderNodeMetadata {
    fn from(legacy: &SdrTokenMap) -> Self {
        Self::from_token_map(legacy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_metadata() {
        let mut metadata = SdrShaderNodeMetadata::new();

        assert!(!metadata.has_label());
        metadata.set_label(&Token::new("My Node"));
        assert!(metadata.has_label());
        assert_eq!(metadata.get_label().as_str(), "My Node");

        metadata.clear_label();
        assert!(!metadata.has_label());
    }

    #[test]
    fn test_role_empty_is_nonexistent() {
        let mut metadata = SdrShaderNodeMetadata::new();

        assert!(!metadata.has_role());

        // Setting empty role should clear it
        metadata.set_role(&Token::new(""));
        assert!(!metadata.has_role());

        // Setting non-empty role should work
        metadata.set_role(&Token::new("primvar"));
        assert!(metadata.has_role());
    }

    #[test]
    fn test_from_token_map() {
        let mut legacy = SdrTokenMap::new();
        legacy.insert(Token::new("label"), "Test Node".to_string());
        legacy.insert(Token::new("help"), "Help text".to_string());

        let metadata = SdrShaderNodeMetadata::from_token_map(&legacy);
        assert_eq!(metadata.get_label().as_str(), "Test Node");
        assert_eq!(metadata.get_help(), "Help text");
    }

    #[test]
    fn test_departments() {
        let mut metadata = SdrShaderNodeMetadata::new();

        let depts = vec![Token::new("lighting"), Token::new("shading")];
        metadata.set_departments(&depts);

        let retrieved = metadata.get_departments();
        assert_eq!(retrieved.len(), 2);
        assert_eq!(retrieved[0].as_str(), "lighting");
        assert_eq!(retrieved[1].as_str(), "shading");
    }
}
