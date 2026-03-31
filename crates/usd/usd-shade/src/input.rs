//! USD Shade Input - shader/node-graph input attribute.
//!
//! Port of pxr/usd/usdShade/input.h and input.cpp
//!
//! This class encapsulates a shader or node-graph input, which is a connectable
//! attribute representing a typed value.

use super::tokens::tokens;
use super::types::{AttributeVector, ConnectionModification, SdrTokenMap};
use std::collections::HashMap;
use usd_core::attribute::Attribute;
use usd_core::prim::Prim;
use usd_sdf::{Path, TimeCode, ValueTypeName};
use usd_tf::Token;
use usd_vt::Value;

// Use real connectable_api module
use super::connectable_api::{ConnectableAPI, ConnectionSourceInfo};

// Utils is now in utils.rs module
use super::utils::Utils;

/// A shader or node-graph input attribute.
///
/// This class encapsulates a shader or node-graph input, which is a connectable
/// attribute representing a typed value.
///
/// # Examples
///
/// ```rust,ignore
/// use usd::usd_shade::Input;
///
/// // Create an input
/// let input = Input::new(&prim, &Token::new("diffuseColor"), &type_name)?;
///
/// // Set a value
/// input.set(Value::from([1.0, 0.0, 0.0]), TimeCode::default())?;
///
/// // Get a value
/// if let Some(color) = input.get::<[f32; 3]>(TimeCode::default()) {
///     println!("Color: {:?}", color);
/// }
/// ```
#[derive(Debug, Clone)]
pub struct Input {
    /// The underlying attribute.
    attr: Attribute,
}

impl Input {
    /// Default constructor returns an invalid Input.
    pub fn invalid() -> Self {
        Self {
            attr: Attribute::invalid(),
        }
    }

    /// Constructs an Input from an Attribute.
    ///
    /// Returns an invalid Input if the attribute is not a valid input.
    pub fn from_attribute(attr: Attribute) -> Self {
        Self { attr }
    }

    /// Constructs an Input from a Prim, name, and type name.
    ///
    /// Creates the input attribute if it doesn't exist.
    pub fn new(prim: &Prim, name: &Token, type_name: &ValueTypeName) -> Option<Self> {
        let input_attr_name = Self::get_input_attr_name(name);

        // Check if attribute already exists and has a spec
        let attr = if let Some(existing_attr) = prim
            .get_attribute(input_attr_name.as_str())
            .filter(|a| a.is_valid())
        {
            existing_attr
        } else {
            // Create the attribute
            prim.create_attribute(
                input_attr_name.as_str(),
                type_name,
                false, // custom = false
                None,  // variability = default
            )?
        };

        Some(Self { attr })
    }

    /// Get the name of the attribute associated with the Input.
    pub fn get_full_name(&self) -> Token {
        self.attr.name()
    }

    /// Returns the name of the input.
    ///
    /// We call this the base name since it strips off the "inputs:" namespace
    /// prefix from the attribute name, and returns it.
    pub fn get_base_name(&self) -> Token {
        let full_name = self.get_full_name();
        let full_name_str = full_name.as_str();
        let inputs_prefix = tokens().inputs.as_str();

        if full_name_str.starts_with(inputs_prefix) {
            Token::new(&full_name_str[inputs_prefix.len()..])
        } else {
            full_name
        }
    }

    /// Get the "scene description" value type name of the attribute associated
    /// with the Input.
    ///
    /// Uses composed stage-level query (matches C++ `_attr.GetTypeName()`).
    /// Correctly resolves types from sublayers, references, and payloads.
    pub fn get_type_name(&self) -> ValueTypeName {
        self.attr.get_type_name()
    }

    /// Get the prim that the input belongs to.
    pub fn get_prim(&self) -> Prim {
        let prim_path = self.attr.prim_path();
        let Some(stage) = self.attr.stage() else {
            return Prim::invalid();
        };
        stage
            .get_prim_at_path(&prim_path)
            .unwrap_or_else(Prim::invalid)
    }

    /// Convenience wrapper for the templated Attribute::Get().
    pub fn get<T: Clone + 'static>(&self, time: TimeCode) -> Option<T> {
        self.attr.get_typed(time)
    }

    /// Convenience wrapper for VtValue version of Attribute::Get().
    pub fn get_value(&self, time: TimeCode) -> Option<Value> {
        self.attr.get(time)
    }

    /// Set a value for the Input at `time`.
    pub fn set(&self, value: impl Into<Value>, time: TimeCode) -> bool {
        self.attr.set(value, time)
    }

    /// Returns true if this Input is valid for querying and authoring
    /// values and metadata.
    pub fn is_defined(&self) -> bool {
        self.attr.is_valid() && Self::is_input(&self.attr)
    }

    /// Test whether a given Attribute represents a valid Input.
    ///
    /// Per C++ reference: checks `attr.IsDefined()` (spec exists in any
    /// composed layer) and name starts with "inputs:".
    pub fn is_input(attr: &Attribute) -> bool {
        // is_valid() checks spec exists in layer stack (matches C++ IsDefined)
        attr.is_valid() && attr.name().as_str().starts_with(tokens().inputs.as_str())
    }

    /// Test if this name has a namespace that indicates it could be an input.
    pub fn is_interface_input_name(name: &str) -> bool {
        name.starts_with(tokens().inputs.as_str())
    }

    /// Explicit Attribute extractor.
    pub fn get_attr(&self) -> &Attribute {
        &self.attr
    }

    /// Allow Input to auto-convert to Attribute.
    pub fn as_attribute(&self) -> &Attribute {
        &self.attr
    }

    // ========================================================================
    // Render Type API
    // ========================================================================

    /// Specify an alternative, renderer-specific type to use when
    /// emitting/translating this Input, rather than translating based
    /// on its GetTypeName().
    ///
    /// For example, we set the renderType to "struct" for Inputs that
    /// are of renderman custom struct types.
    pub fn set_render_type(&self, render_type: &Token) -> bool {
        self.attr
            .set_metadata(&Token::new("renderType"), Value::from(render_type.as_str()))
    }

    /// Return this Input's specialized renderType, or an empty
    /// token if none was authored.
    pub fn get_render_type(&self) -> Token {
        if let Some(v) = self.attr.get_metadata(&Token::new("renderType")) {
            if let Some(s) = v.get::<String>() {
                return Token::new(s);
            }
        }
        Token::new("")
    }

    /// Return true if a renderType has been specified for this Input.
    pub fn has_render_type(&self) -> bool {
        self.attr.has_authored_metadata(&Token::new("renderType"))
    }

    // ========================================================================
    // Sdr Metadata API
    // ========================================================================

    /// Returns this Input's composed "sdrMetadata" dictionary as a SdrTokenMap.
    pub fn get_sdr_metadata(&self) -> SdrTokenMap {
        let mut result = HashMap::new();

        if let Some(dict_val) = self.attr.get_metadata(&tokens().sdr_metadata) {
            if let Some(dict) = dict_val.get::<usd_vt::Dictionary>() {
                for (key, value) in dict.iter() {
                    // Convert value to string (as per C++ implementation)
                    let value_str = value.to_string();
                    result.insert(Token::new(key), value_str);
                }
            }
        }

        result
    }

    /// Returns the value corresponding to `key` in the composed "sdrMetadata" dictionary.
    pub fn get_sdr_metadata_by_key(&self, key: &Token) -> String {
        if let Some(v) = self
            .attr
            .get_metadata_by_dict_key(&tokens().sdr_metadata, key)
        {
            v.to_string()
        } else {
            String::new()
        }
    }

    /// Authors the given `sdrMetadata` value on this Input at the current EditTarget.
    pub fn set_sdr_metadata(&self, sdr_metadata: &SdrTokenMap) {
        for (key, value) in sdr_metadata {
            self.set_sdr_metadata_by_key(key, value);
        }
    }

    /// Sets the value corresponding to `key` to the given string `value`, in
    /// the Input's "sdrMetadata" dictionary at the current EditTarget.
    pub fn set_sdr_metadata_by_key(&self, key: &Token, value: &str) {
        self.attr
            .set_metadata_by_dict_key(&tokens().sdr_metadata, key, Value::from(value));
    }

    /// Returns true if the Input has a non-empty composed "sdrMetadata"
    /// dictionary value.
    pub fn has_sdr_metadata(&self) -> bool {
        self.attr.has_authored_metadata(&tokens().sdr_metadata)
    }

    /// Returns true if there is a value corresponding to the given `key` in
    /// the composed "sdrMetadata" dictionary.
    pub fn has_sdr_metadata_by_key(&self, key: &Token) -> bool {
        self.attr.has_metadata_dict_key(&tokens().sdr_metadata, key)
    }

    /// Clears any "sdrMetadata" value authored on the Input in the current EditTarget.
    pub fn clear_sdr_metadata(&self) {
        // clear_metadata is implemented in attribute.rs
        let _ = self.attr.clear_metadata(&tokens().sdr_metadata);
    }

    /// Clears the entry corresponding to the given `key` in the
    /// "sdrMetadata" dictionary authored in the current EditTarget.
    pub fn clear_sdr_metadata_by_key(&self, key: &Token) {
        self.attr
            .clear_metadata_by_dict_key(&tokens().sdr_metadata, key);
    }

    // ========================================================================
    // UsdAttribute API
    // ========================================================================

    /// Set documentation string for this Input.
    pub fn set_documentation(&self, docs: &str) -> bool {
        if !self.attr.is_valid() {
            return false;
        }
        self.attr
            .set_metadata(&Token::new("documentation"), Value::from(docs))
    }

    /// Get documentation string for this Input.
    pub fn get_documentation(&self) -> String {
        if !self.attr.is_valid() {
            return String::new();
        }
        if let Some(v) = self.attr.get_metadata(&Token::new("documentation")) {
            if let Some(s) = v.get::<String>() {
                return s.clone();
            }
        }
        String::new()
    }

    /// Set the displayGroup metadata for this Input.
    pub fn set_display_group(&self, display_group: &str) -> bool {
        if !self.attr.is_valid() {
            return false;
        }
        self.attr
            .set_metadata(&Token::new("displayGroup"), Value::from(display_group))
    }

    /// Get the displayGroup metadata for this Input.
    pub fn get_display_group(&self) -> String {
        if !self.attr.is_valid() {
            return String::new();
        }
        if let Some(v) = self.attr.get_metadata(&Token::new("displayGroup")) {
            if let Some(s) = v.get::<String>() {
                return s.clone();
            }
        }
        String::new()
    }

    // ========================================================================
    // Connections API (delegated to ConnectableAPI)
    // ========================================================================

    /// Determines whether this Input can be connected to the given source attribute.
    pub fn can_connect(&self, source: &Attribute) -> bool {
        ConnectableAPI::can_connect(self, source)
    }

    /// Authors a connection for this Input.
    pub fn connect_to_source(
        &self,
        source: &ConnectionSourceInfo,
        mod_: ConnectionModification,
    ) -> bool {
        ConnectableAPI::connect_to_source(&self.attr, source, mod_)
    }

    /// Connect this Input to the given source path.
    pub fn connect_to_source_path(&self, source_path: &Path) -> bool {
        ConnectableAPI::connect_to_source_path(&self.attr, source_path)
    }

    /// Finds the valid sources of connections for the Input.
    pub fn get_connected_sources(
        &self,
        invalid_source_paths: &mut Vec<Path>,
    ) -> Vec<ConnectionSourceInfo> {
        ConnectableAPI::get_connected_sources(&self.attr, invalid_source_paths)
    }

    /// Returns true if and only if this Input is currently connected to a valid source.
    pub fn has_connected_source(&self) -> bool {
        ConnectableAPI::has_connected_source(&self.attr)
    }

    /// Returns true if the connection to this Input's source is authored across a specializes arc.
    pub fn is_source_connection_from_base_material(&self) -> bool {
        ConnectableAPI::is_source_connection_from_base_material(&self.attr)
    }

    /// Disconnect source for this Input.
    pub fn disconnect_source(&self, source_attr: Option<&Attribute>) -> bool {
        ConnectableAPI::disconnect_source(&self.attr, source_attr)
    }

    /// Clears sources for this Input in the current EditTarget.
    pub fn clear_sources(&self) -> bool {
        ConnectableAPI::clear_sources(&self.attr)
    }

    /// \deprecated Alias for clear_sources().
    ///
    /// Matches C++ `ClearSource()`.
    pub fn clear_source(&self) -> bool {
        self.clear_sources()
    }

    /// Connects this Input to the given source input.
    ///
    /// Matches C++ `ConnectToSource(UsdShadeInput const &sourceInput)`.
    pub fn connect_to_source_input(&self, source_input: &Input) -> bool {
        ConnectableAPI::connect_to_source_input(&self.attr, source_input)
    }

    /// Connects this Input to the given source output.
    ///
    /// Matches C++ `ConnectToSource(UsdShadeOutput const &sourceOutput)`.
    pub fn connect_to_source_output(&self, source_output: &super::output::Output) -> bool {
        ConnectableAPI::connect_to_source_output(&self.attr, source_output)
    }

    /// Connects this Input to the given sources.
    ///
    /// Matches C++ `SetConnectedSources(vector<ConnectionSourceInfo>)`.
    pub fn set_connected_sources(&self, source_infos: &[ConnectionSourceInfo]) -> bool {
        ConnectableAPI::set_connected_sources(&self.attr, source_infos)
    }

    /// Returns the "raw" (authored) connected source paths for this Input.
    ///
    /// Matches C++ `GetRawConnectedSourcePaths(SdfPathVector *sourcePaths)`.
    pub fn get_raw_connected_source_paths(&self, source_paths: &mut Vec<Path>) -> bool {
        ConnectableAPI::get_raw_connected_source_paths(&self.attr, source_paths)
    }

    /// \deprecated Please use get_connected_sources instead.
    ///
    /// Matches C++ `GetConnectedSource(source, name, type)` (deprecated 3-out-param version).
    pub fn get_connected_source(
        &self,
        source: &mut ConnectableAPI,
        source_name: &mut Token,
        source_type: &mut super::types::AttributeType,
    ) -> bool {
        let mut invalid_paths = Vec::new();
        let sources = self.get_connected_sources(&mut invalid_paths);
        if sources.is_empty() {
            return false;
        }
        if sources.len() > 1 {
            eprintln!(
                "More than one connection for input {}. GetConnectedSource will only report the first one.",
                self.attr.path().get_string()
            );
        }
        let info = &sources[0];
        *source = info.source.clone();
        *source_name = info.source_name.clone();
        *source_type = info.source_type;
        true
    }

    /// \deprecated singular version returning first value-producing attribute.
    ///
    /// Matches C++ `GetValueProducingAttribute(UsdShadeAttributeType* attrType)`.
    pub fn get_value_producing_attribute(
        &self,
        attr_type: &mut super::types::AttributeType,
    ) -> Attribute {
        let attrs = self.get_value_producing_attributes(false);
        if attrs.is_empty() {
            *attr_type = super::types::AttributeType::Invalid;
            return Attribute::invalid();
        }
        if attrs.len() > 1 {
            eprintln!(
                "More than one value producing attribute for shading input {}. \
                 GetValueProducingAttribute will only report the first one. \
                 Please use GetValueProducingAttributes to retrieve all.",
                self.attr.path().get_string()
            );
        }
        let attr = &attrs[0];
        // Determine type from attribute name prefix
        let name = attr.name();
        let name_str = name.as_str();
        if name_str.starts_with(tokens().outputs.as_str()) {
            *attr_type = super::types::AttributeType::Output;
        } else if name_str.starts_with(tokens().inputs.as_str()) {
            *attr_type = super::types::AttributeType::Input;
        } else {
            *attr_type = super::types::AttributeType::Invalid;
        }
        attr.clone()
    }

    // ========================================================================
    // Connectability API
    // ========================================================================

    /// Set the connectability of the Input.
    ///
    /// Connectability can be set to "full" or "interfaceOnly".
    /// - "full" implies that the Input can be connected to any other Input or Output.
    /// - "interfaceOnly" implies that the Input can only be connected to a NodeGraph Input.
    ///
    /// The default connectability of an input is "full".
    pub fn set_connectability(&self, connectability: &Token) -> bool {
        self.attr.set_metadata(
            &Token::new("connectability"),
            Value::from(connectability.as_str()),
        )
    }

    /// Returns the connectability of the Input.
    ///
    /// Returns "full" if no connectability is authored.
    pub fn get_connectability(&self) -> Token {
        let connectability = if let Some(v) = self.attr.get_metadata(&Token::new("connectability"))
        {
            if let Some(s) = v.get::<String>() {
                Token::new(s)
            } else {
                Token::new("")
            }
        } else {
            Token::new("")
        };

        // If there's an authored non-empty connectability value, return it.
        // If not, return "full".
        if !connectability.is_empty() {
            connectability
        } else {
            tokens().full.clone()
        }
    }

    /// Clears any authored connectability on the Input.
    pub fn clear_connectability(&self) -> bool {
        self.attr.clear_metadata(&Token::new("connectability"))
    }

    // ========================================================================
    // Connected Value API
    // ========================================================================

    /// Find what is connected to this Input recursively.
    pub fn get_value_producing_attributes(&self, shader_outputs_only: bool) -> AttributeVector {
        Utils::get_value_producing_attributes(self, shader_outputs_only)
    }

    // ========================================================================
    // Helper methods
    // ========================================================================

    /// Helper to get the full attribute name for an input.
    fn get_input_attr_name(input_name: &Token) -> Token {
        Token::new(&format!(
            "{}{}",
            tokens().inputs.as_str(),
            input_name.as_str()
        ))
    }
}

impl std::ops::Deref for Input {
    type Target = Attribute;

    fn deref(&self) -> &Self::Target {
        &self.attr
    }
}

impl PartialEq for Input {
    fn eq(&self, other: &Self) -> bool {
        self.attr.path() == other.attr.path()
    }
}

impl Eq for Input {}

impl std::hash::Hash for Input {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.attr.path().hash(state);
    }
}
