//! USD Shade Output - shader/node-graph output attribute.
//!
//! Port of pxr/usd/usdShade/output.h and output.cpp
//!
//! This class encapsulates a shader or node-graph output, which is a connectable
//! attribute representing a typed, externally computed value.

use super::connectable_api::{ConnectableAPI, ConnectionSourceInfo};
use super::tokens::tokens;
use super::types::{AttributeType, AttributeVector, ConnectionModification, SdrTokenMap};
use super::utils::Utils;
use std::collections::HashMap;
use usd_core::attribute::Attribute;
use usd_core::prim::Prim;
use usd_sdf::{Path, TimeCode, ValueTypeName};
use usd_tf::Token;
use usd_vt::Value;

/// A shader or node-graph output attribute.
///
/// This class encapsulates a shader or node-graph output, which is a connectable
/// attribute representing a typed, externally computed value.
///
/// # Examples
///
/// ```rust,ignore
/// use usd::usd_shade::Output;
///
/// // Create an output
/// let output = Output::new(&prim, &Token::new("out"), &type_name)?;
///
/// // Set a value (unusual but supported)
/// output.set(Value::from([1.0, 0.0, 0.0]), TimeCode::default())?;
/// ```
#[derive(Debug, Clone)]
pub struct Output {
    /// The underlying attribute.
    attr: Attribute,
}

impl Output {
    /// Default constructor returns an invalid Output.
    pub fn invalid() -> Self {
        Self {
            attr: Attribute::invalid(),
        }
    }

    /// Constructs an Output from an Attribute.
    ///
    /// Returns an invalid Output if the attribute is not a valid output.
    pub fn from_attribute(attr: Attribute) -> Self {
        Self { attr }
    }

    /// Constructs an Output from a Prim, name, and type name.
    ///
    /// Creates the output attribute if it doesn't exist.
    pub fn new(prim: &Prim, name: &Token, type_name: &ValueTypeName) -> Option<Self> {
        let output_attr_name = Self::get_output_attr_name(name);

        // Check if attribute actually exists in the layer (not just a lazy handle).
        // prim.get_attribute() always returns Some for valid paths, so we must
        // check the layer spec to determine if the attribute is truly authored.
        let attr = if prim.has_attribute(output_attr_name.as_str()) {
            prim.get_attribute(output_attr_name.as_str())?
        } else {
            // Create the attribute
            prim.create_attribute(
                output_attr_name.as_str(),
                type_name,
                false, // custom = false
                None,  // variability = default
            )?
        };

        Some(Self { attr })
    }

    /// Get the name of the attribute associated with the Output.
    pub fn get_full_name(&self) -> Token {
        self.attr.name()
    }

    /// Returns the name of the output.
    ///
    /// We call this the base name since it strips off the "outputs:" namespace
    /// prefix from the attribute name, and returns it.
    pub fn get_base_name(&self) -> Token {
        let full_name = self.get_full_name();
        let full_name_str = full_name.as_str();
        let outputs_prefix = tokens().outputs.as_str();

        if full_name_str.starts_with(outputs_prefix) {
            Token::new(&full_name_str[outputs_prefix.len()..])
        } else {
            full_name
        }
    }

    /// Get the prim that the output belongs to.
    pub fn get_prim(&self) -> Prim {
        let prim_path = self.attr.prim_path();
        let Some(stage) = self.attr.stage() else {
            return Prim::invalid();
        };
        stage
            .get_prim_at_path(&prim_path)
            .unwrap_or_else(Prim::invalid)
    }

    /// Get the "scene description" value type name of the attribute associated
    /// with the output.
    ///
    /// Uses composed stage-level query (matches C++ `_attr.GetTypeName()`).
    /// Correctly resolves types from sublayers, references, and payloads.
    pub fn get_type_name(&self) -> ValueTypeName {
        self.attr.get_type_name()
    }

    /// Convenience wrapper for the templated UsdAttribute::Get().
    ///
    /// Matches C++ `Get<T>(value, time)` template.
    pub fn get<T: Clone + 'static>(&self, time: TimeCode) -> Option<T> {
        self.attr.get_typed(time)
    }

    /// Convenience wrapper for VtValue version of UsdAttribute::Get().
    pub fn get_value(&self, time: TimeCode) -> Option<Value> {
        self.attr.get(time)
    }

    /// Set a value for the output.
    ///
    /// It's unusual to be setting a value on an output since it represents
    /// an externally computed value. The Set API is provided here just for the
    /// sake of completeness and uniformity with other property schema.
    pub fn set(&self, value: impl Into<Value>, time: TimeCode) -> bool {
        if let Some(attr) = self.get_attr() {
            return attr.set(value.into(), time);
        }
        false
    }

    // ========================================================================
    // Render Type API
    // ========================================================================

    /// Specify an alternative, renderer-specific type to use when
    /// emitting/translating this output, rather than translating based
    /// on its GetTypeName().
    ///
    /// For example, we set the renderType to "struct" for outputs that
    /// are of renderman custom struct types.
    pub fn set_render_type(&self, render_type: &Token) -> bool {
        self.attr
            .set_metadata(&Token::new("renderType"), Value::from(render_type.as_str()))
    }

    /// Return this output's specialized renderType, or an empty
    /// token if none was authored.
    pub fn get_render_type(&self) -> Token {
        if let Some(v) = self.attr.get_metadata(&Token::new("renderType")) {
            if let Some(s) = v.get::<String>() {
                return Token::new(s);
            }
        }
        Token::new("")
    }

    /// Return true if a renderType has been specified for this output.
    ///
    /// Uses has_authored_metadata to check key existence regardless of value,
    /// matching C++ `_attr.HasAuthoredMetadata(renderType)`.
    pub fn has_render_type(&self) -> bool {
        self.attr.has_authored_metadata(&Token::new("renderType"))
    }

    // ========================================================================
    // SdrMetadata API
    // ========================================================================

    /// Returns this Output's composed "sdrMetadata" dictionary as a SdrTokenMap.
    pub fn get_sdr_metadata(&self) -> SdrTokenMap {
        let mut result = HashMap::new();

        if let Some(v) = self.attr.get_metadata(&tokens().sdr_metadata) {
            if let Some(dict) = v.get::<usd_vt::Dictionary>() {
                for (key, val) in dict.iter() {
                    result.insert(Token::new(key), val.to_string());
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

    /// Authors the given `sdrMetadata` value on this Output at the current EditTarget.
    pub fn set_sdr_metadata(&self, sdr_metadata: &SdrTokenMap) {
        for (key, value) in sdr_metadata {
            self.set_sdr_metadata_by_key(key, value);
        }
    }

    /// Sets the value corresponding to `key` to the given string `value`, in
    /// the Output's "sdrMetadata" dictionary at the current EditTarget.
    pub fn set_sdr_metadata_by_key(&self, key: &Token, value: &str) {
        let _ = self
            .attr
            .set_metadata_by_dict_key(&tokens().sdr_metadata, key, Value::from(value));
    }

    /// Returns true if the Output has a non-empty composed "sdrMetadata"
    /// dictionary value.
    pub fn has_sdr_metadata(&self) -> bool {
        !self.get_sdr_metadata().is_empty()
    }

    /// Returns true if there is a value corresponding to the given `key` in
    /// the composed "sdrMetadata" dictionary.
    pub fn has_sdr_metadata_by_key(&self, key: &Token) -> bool {
        self.attr.has_metadata_dict_key(&tokens().sdr_metadata, key)
    }

    /// Clears any "sdrMetadata" value authored on the Output in the current EditTarget.
    pub fn clear_sdr_metadata(&self) {
        let _ = self.attr.clear_metadata(&tokens().sdr_metadata);
    }

    /// Clears the entry corresponding to the given `key` in the
    /// "sdrMetadata" dictionary authored in the current EditTarget.
    pub fn clear_sdr_metadata_by_key(&self, key: &Token) {
        let _ = self
            .attr
            .clear_metadata_by_dict_key(&tokens().sdr_metadata, key);
    }

    // ========================================================================
    // UsdAttribute API
    // ========================================================================

    /// Explicit UsdAttribute extractor.
    pub fn get_attr(&self) -> Option<Attribute> {
        if self.attr.is_valid() {
            Some(self.attr.clone())
        } else {
            None
        }
    }

    /// Test whether a given UsdAttribute represents a valid Output, which
    /// implies that creating a UsdShadeOutput from the attribute will succeed.
    ///
    /// Per C++ reference: checks `attr.IsDefined()` (spec exists in any
    /// composed layer) and name starts with "outputs:".
    pub fn is_output(attr: &Attribute) -> bool {
        // is_valid() checks spec exists in layer stack (matches C++ IsDefined)
        attr.is_valid() && attr.name().as_str().starts_with(tokens().outputs.as_str())
    }

    /// Return true if the wrapped UsdAttribute is defined, and in
    /// addition the attribute is identified as an output.
    pub fn is_defined(&self) -> bool {
        Self::is_output(&self.attr)
    }

    // ========================================================================
    // Connections API (delegated to ConnectableAPI)
    // ========================================================================

    /// Determines whether this Output can be connected to the given source attribute.
    ///
    /// An output is considered to be connectable only if it belongs to a
    /// node-graph. Shader outputs are not connectable.
    pub fn can_connect(&self, source: &Attribute) -> bool {
        ConnectableAPI::can_connect_output_to_source(self, source)
    }

    /// \overload
    pub fn can_connect_input(&self, source_input: &super::input::Input) -> bool {
        // Input derefs to Attribute, so we can use it directly
        self.can_connect(source_input)
    }

    /// \overload
    pub fn can_connect_output(&self, source_output: &Output) -> bool {
        // Use the attribute directly from Output
        if let Some(attr) = source_output.get_attr() {
            self.can_connect(&attr)
        } else {
            false
        }
    }

    /// Authors a connection for this Output.
    pub fn connect_to_source(
        &self,
        source: &ConnectionSourceInfo,
        mod_: ConnectionModification,
    ) -> bool {
        ConnectableAPI::connect_to_source(&self.attr, source, mod_)
    }

    /// Authors a connection using deprecated API.
    pub fn connect_to_source_deprecated(
        &self,
        source: &ConnectableAPI,
        source_name: &Token,
        source_type: AttributeType,
        type_name: ValueTypeName,
    ) -> bool {
        ConnectableAPI::connect_to_source_deprecated(
            &self.attr,
            source,
            source_name,
            source_type,
            type_name,
        )
    }

    /// Authors a connection for this Output to the source at the given path.
    pub fn connect_to_source_path(&self, source_path: &Path) -> bool {
        ConnectableAPI::connect_to_source_path(&self.attr, source_path)
    }

    /// Connects this Output to the given input, `source_input`.
    pub fn connect_to_source_input(&self, source_input: &super::input::Input) -> bool {
        ConnectableAPI::connect_to_source_input(&self.attr, source_input)
    }

    /// Connects this Output to the given output, `source_output`.
    pub fn connect_to_source_output(&self, source_output: &Output) -> bool {
        let source_info = ConnectionSourceInfo::from_output(source_output);
        ConnectableAPI::connect_to_source(&self.attr, &source_info, ConnectionModification::Replace)
    }

    /// Connects this Output to the given sources, `source_infos`.
    pub fn set_connected_sources(&self, source_infos: &[ConnectionSourceInfo]) -> bool {
        ConnectableAPI::set_connected_sources(&self.attr, source_infos)
    }

    /// Finds the valid sources of connections for the Output.
    pub fn get_connected_sources(
        &self,
        invalid_source_paths: &mut Vec<Path>,
    ) -> Vec<ConnectionSourceInfo> {
        ConnectableAPI::get_connected_sources(&self.attr, invalid_source_paths)
    }

    /// \deprecated Please use GetConnectedSources instead
    pub fn get_connected_source(
        &self,
        source: &mut ConnectableAPI,
        source_name: &mut Token,
        source_type: &mut AttributeType,
    ) -> bool {
        let mut invalid_paths = Vec::new();
        let sources = self.get_connected_sources(&mut invalid_paths);

        if sources.is_empty() {
            return false;
        }

        if sources.len() > 1 {
            // Warn about multiple connections
            eprintln!(
                "More than one connection for output {}. GetConnectedSource will only report the first one.",
                self.attr.path().get_string()
            );
        }

        let source_info = &sources[0];
        *source = source_info.source.clone();
        *source_name = source_info.source_name.clone();
        *source_type = source_info.source_type;
        true
    }

    /// Returns the "raw" (authored) connected source paths for this Output.
    pub fn get_raw_connected_source_paths(&self, source_paths: &mut Vec<Path>) -> bool {
        ConnectableAPI::get_raw_connected_source_paths(&self.attr, source_paths)
    }

    /// Returns true if and only if this Output is currently connected to a
    /// valid (defined) source.
    pub fn has_connected_source(&self) -> bool {
        ConnectableAPI::has_connected_source(&self.attr)
    }

    /// Returns true if the connection to this Output's source, as returned by
    /// GetConnectedSource(), is authored across a specializes arc.
    pub fn is_source_connection_from_base_material(&self) -> bool {
        ConnectableAPI::is_source_connection_from_base_material(&self.attr)
    }

    /// Disconnect source for this Output.
    pub fn disconnect_source(&self, source_attr: Option<&Attribute>) -> bool {
        ConnectableAPI::disconnect_source(&self.attr, source_attr)
    }

    /// Clears sources for this Output in the current EditTarget.
    pub fn clear_sources(&self) -> bool {
        ConnectableAPI::clear_sources(&self.attr)
    }

    /// \deprecated
    pub fn clear_source(&self) -> bool {
        self.clear_sources()
    }

    // ========================================================================
    // Connected Value API
    // ========================================================================

    /// Find what is connected to this Output recursively.
    pub fn get_value_producing_attributes(&self, shader_outputs_only: bool) -> AttributeVector {
        Utils::get_value_producing_attributes_output(self, shader_outputs_only)
    }

    // ========================================================================
    // Helper methods
    // ========================================================================

    /// Helper to get the full attribute name for an output.
    fn get_output_attr_name(output_name: &Token) -> Token {
        Token::new(&format!(
            "{}{}",
            tokens().outputs.as_str(),
            output_name.as_str()
        ))
    }
}

impl std::ops::Deref for Output {
    type Target = Attribute;

    fn deref(&self) -> &Self::Target {
        &self.attr
    }
}

impl PartialEq for Output {
    fn eq(&self, other: &Self) -> bool {
        self.attr.path() == other.attr.path()
    }
}

impl Eq for Output {}

impl std::hash::Hash for Output {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.attr.path().hash(state);
    }
}
