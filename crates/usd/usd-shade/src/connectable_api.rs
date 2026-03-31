//! USD Shade ConnectableAPI - API for connecting shading attributes.
//!
//! Port of pxr/usd/usdShade/connectableAPI.h and connectableAPI.cpp
//!
//! UsdShadeConnectableAPI is an API schema that provides a common interface
//! for creating outputs and making connections between shading parameters and outputs.

use super::input::Input;
use super::output::Output;
use super::tokens::tokens;
use super::types::{AttributeType, ConnectionModification};
use super::utils::Utils;
use std::sync::Arc;
use usd_core::attribute::Attribute;
use usd_core::common::ListPosition;
use usd_core::prim::Prim;
use usd_core::schema_base::APISchemaBase;
use usd_core::stage::Stage;
use usd_sdf::{Path, ValueTypeName};
use usd_tf::Token;

// Use real connectable_api_behavior module
use super::connectable_api_behavior::get_behavior;

/// A compact struct to represent a bundle of information about an upstream
/// source attribute.
///
/// Port of `UsdShadeConnectionSourceInfo` struct.
#[derive(Debug, Clone)]
pub struct ConnectionSourceInfo {
    /// The connectable prim that produces or contains a value for the given shading attribute.
    pub source: ConnectableAPI,
    /// The name of the shading attribute that is the target of the connection.
    /// This excludes any namespace prefix that determines the type of the source.
    pub source_name: Token,
    /// Used to indicate the type of the shading attribute that is the target of the connection.
    pub source_type: AttributeType,
    /// If specified, is the typename of the attribute to create on the source if it doesn't exist.
    pub type_name: ValueTypeName,
}

impl ConnectionSourceInfo {
    /// Default constructor.
    pub fn new() -> Self {
        Self {
            source: ConnectableAPI::invalid(),
            source_name: Token::new(""),
            source_type: AttributeType::Invalid,
            type_name: ValueTypeName::invalid(),
        }
    }

    /// Constructs from ConnectableAPI, source name, source type, and optional type name.
    pub fn from_connectable(
        source: ConnectableAPI,
        source_name: Token,
        source_type: AttributeType,
        type_name: ValueTypeName,
    ) -> Self {
        Self {
            source,
            source_name,
            source_type,
            type_name,
        }
    }

    /// Constructs from an Input.
    pub fn from_input(input: &Input) -> Self {
        Self {
            source: ConnectableAPI::new(input.get_prim()),
            source_name: input.get_base_name(),
            source_type: AttributeType::Input,
            type_name: input.get_type_name(),
        }
    }

    /// Constructs from an Output.
    pub fn from_output(output: &Output) -> Self {
        Self {
            source: ConnectableAPI::new(output.get_prim()),
            source_name: output.get_base_name(),
            source_type: AttributeType::Output,
            type_name: output.get_type_name(),
        }
    }

    /// Constructs from a stage and source path.
    pub fn from_path(stage: Arc<Stage>, source_path: &Path) -> Self {
        if !source_path.is_property_path() {
            return Self::new();
        }

        // Extract base name and type from path
        let name_token = Token::new(source_path.get_name());
        let (source_name, source_type) = Utils::get_base_name_and_type(&name_token);

        // Get the prim path
        let prim_path = source_path.get_prim_path();
        let source = ConnectableAPI::get(&stage, &prim_path);

        // C++ connectableAPI.cpp:571-598: use composed stage-level attribute type name,
        // not root layer (handles types from referenced/sublayered prims correctly).
        let type_name = if let Some(source_attr) = stage.get_attribute_at_path(source_path) {
            source_attr.get_type_name()
        } else {
            ValueTypeName::invalid()
        };

        Self {
            source,
            source_name,
            source_type,
            type_name,
        }
    }

    /// Return true if this source info is valid for setting up a connection.
    pub fn is_valid(&self) -> bool {
        self.source_type != AttributeType::Invalid
            && !self.source_name.is_empty()
            && self.source.get_prim().is_valid()
    }
}

impl Default for ConnectionSourceInfo {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for ConnectionSourceInfo {
    fn eq(&self, other: &Self) -> bool {
        self.source_name == other.source_name
            && self.source_type == other.source_type
            && self.source.get_prim().path() == other.source.get_prim().path()
    }
}

impl Eq for ConnectionSourceInfo {}

/// UsdShadeConnectableAPI is an API schema that provides a common interface
/// for creating outputs and making connections between shading parameters and outputs.
///
/// Port of `UsdShadeConnectableAPI` class.
#[derive(Debug, Clone)]
pub struct ConnectableAPI {
    /// Base API schema.
    base: APISchemaBase,
}

impl ConnectableAPI {
    /// Constructs a ConnectableAPI on the given prim.
    pub fn new(prim: Prim) -> Self {
        Self {
            base: APISchemaBase::new(prim),
        }
    }

    /// Constructs a ConnectableAPI from an APISchemaBase.
    pub fn from_schema_base(schema: APISchemaBase) -> Self {
        Self { base: schema }
    }

    /// Creates an invalid ConnectableAPI.
    pub fn invalid() -> Self {
        Self {
            base: APISchemaBase::invalid(),
        }
    }

    /// Returns a ConnectableAPI holding the prim at `path` on `stage`.
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Self {
        if let Some(prim) = stage.get_prim_at_path(path) {
            Self::new(prim)
        } else {
            Self::invalid()
        }
    }

    /// Returns true if this ConnectableAPI is valid.
    pub fn is_valid(&self) -> bool {
        self.base.is_valid() && self._is_compatible()
    }

    /// Returns the wrapped prim.
    pub fn get_prim(&self) -> Prim {
        self.base.get_prim().clone()
    }

    /// Returns the path to this prim.
    pub fn path(&self) -> &Path {
        self.base.path()
    }

    /// Returns true if the given prim is compatible with this API schema.
    ///
    /// A prim has a compatible connectableAPI if a valid behavior is registered for it.
    fn _is_compatible(&self) -> bool {
        // Check if behavior exists
        get_behavior(self.base.get_prim()).is_some() ||
            // Default: allow Shader, NodeGraph, Material types
            self.base.get_prim().type_name() == "Shader" ||
            self.base.get_prim().type_name() == "NodeGraph" ||
            self.base.get_prim().type_name() == "Material"
    }

    // ========================================================================
    // Container Behavior
    // ========================================================================

    /// Returns true if the prim is a container.
    pub fn is_container(&self) -> bool {
        if let Some(beh) = get_behavior(self.base.get_prim()) {
            return beh.is_container();
        }
        // Default: NodeGraph and Material are containers
        let type_name = self.base.get_prim().type_name();
        let type_name_str = type_name.as_str();
        type_name_str == "NodeGraph" || type_name_str == "Material"
    }

    /// Returns true if container encapsulation rules should be respected.
    pub fn requires_encapsulation(&self) -> bool {
        if let Some(beh) = get_behavior(self.base.get_prim()) {
            return beh.requires_encapsulation();
        }
        // Default: require encapsulation
        true
    }

    // ========================================================================
    // Connections API
    // ========================================================================

    /// Determines whether the given input can be connected to the given source attribute.
    pub fn can_connect_input(input: &Input, source: &Attribute) -> bool {
        Self::can_connect(input, source)
    }

    /// Determines whether the given output can be connected to the given source attribute.
    ///
    /// An output is considered to be connectable only if it belongs to a node-graph.
    /// Shader outputs are not connectable.
    pub fn can_connect_output(output: &super::output::Output, source: &Attribute) -> bool {
        Self::can_connect_output_to_source(output, source)
    }

    /// Determines whether the given output can be connected to the given source attribute.
    ///
    /// An output is considered to be connectable only if it belongs to a node-graph.
    /// Shader outputs are not connectable.
    pub fn can_connect_output_to_source(
        output: &super::output::Output,
        source: &Attribute,
    ) -> bool {
        // Use behavior registry if available
        let output_prim = output.get_prim();
        if let Some(behavior) = get_behavior(&output_prim) {
            let mut reason = None;
            return behavior.can_connect_output_to_source(output, source, &mut reason);
        }

        // Fallback to default logic
        // Check if output is defined
        if !output.is_defined() {
            return false;
        }

        // Check if source is valid
        if !source.is_valid() {
            return false;
        }

        // Outputs can only be connected if they belong to a node-graph (container)
        let output_api = ConnectableAPI::new(output_prim.clone());

        if !output_api.is_container() {
            // Shader outputs are not connectable
            return false;
        }

        // Get connectability rules - similar to input but for outputs
        let inputs_prefix = tokens().inputs.as_str();
        let outputs_prefix = tokens().outputs.as_str();
        let source_name = source.name();
        let source_name_str = source_name.as_str();

        // Check if source is an input or output
        let source_is_input = source_name_str.starts_with(inputs_prefix);
        let source_is_output = source_name_str.starts_with(outputs_prefix);

        if source_is_input || source_is_output {
            // Check encapsulation rules if required
            if output_api.requires_encapsulation() {
                let output_parent = output_prim.path().get_parent_path();
                let source_prim_path = source.path().get_prim_path();
                let Some(stage) = source.stage() else {
                    return false;
                };
                let Some(source_prim) = stage.get_prim_at_path(&source_prim_path) else {
                    return false;
                };
                let source_parent = source_prim.path().get_parent_path();
                return output_parent == source_parent;
            }
            return true;
        }

        false
    }

    /// Determines whether the given input can be connected to the given source attribute.
    ///
    /// Delegates to the registered ConnectableAPIBehavior for the input's prim type,
    /// falling back to DefaultConnectableAPIBehavior. Matches C++ can_connect() logic.
    pub fn can_connect(input: &Input, source: &Attribute) -> bool {
        if !input.is_defined() || !source.is_valid() {
            return false;
        }

        let input_prim = input.get_prim();

        // Delegate to registered behavior, matching C++ UsdShadeConnectableAPI::CanConnect(input).
        if let Some(behavior) = get_behavior(&input_prim) {
            let mut reason = None;
            return behavior.can_connect_input_to_source(input, source, &mut reason);
        }

        // Fall back to default behavior (same as C++ DefaultConnectableAPIBehavior).
        use super::connectable_api_behavior::{
            ConnectableAPIBehavior, DefaultConnectableAPIBehavior,
        };
        let default_behavior = DefaultConnectableAPIBehavior::new();
        let mut reason = None;
        default_behavior.can_connect_input_to_source(input, source, &mut reason)
    }

    /// Authors a connection for a given shading attribute.
    ///
    /// `source` describes the upstream source attribute with all information
    /// necessary to make a connection.
    /// `mod` describes the operation that should be applied to the list of connections.
    pub fn connect_to_source(
        shading_attr: &Attribute,
        source: &ConnectionSourceInfo,
        mod_: ConnectionModification,
    ) -> bool {
        if !source.is_valid() {
            return false;
        }

        // Get or create source attribute.
        // C++ connectableAPI.cpp:158: use shadingAttr.GetTypeName() (composed type, not root layer).
        let Some(_stage) = shading_attr.stage() else {
            return false;
        };
        // Use the composed attribute's type name directly — works across layers.
        let fallback_type_name = shading_attr.get_type_name();
        let source_attr = Self::_get_or_create_source_attr(source, fallback_type_name);
        // C++ checks !sourceAttr — i.e. only that a valid UsdAttribute handle was returned.
        // We check path validity rather than is_valid() because a freshly created attr
        // may not pass composed-query validation due to stale PrimIndex cache.
        if source_attr.path().is_empty() || !source_attr.path().is_property_path() {
            return false;
        }

        match mod_ {
            ConnectionModification::Replace => {
                shading_attr.set_connections(vec![source_attr.path().clone()])
            }
            ConnectionModification::Prepend => shading_attr
                .add_connection_with_position(source_attr.path(), ListPosition::FrontOfPrependList),
            ConnectionModification::Append => shading_attr
                .add_connection_with_position(source_attr.path(), ListPosition::BackOfAppendList),
        }
    }

    /// Helper to get or create source attribute.
    fn _get_or_create_source_attr(
        source_info: &ConnectionSourceInfo,
        fallback_type_name: ValueTypeName,
    ) -> Attribute {
        let source_prim = source_info.source.get_prim();
        let prefix = Utils::get_prefix_for_attribute_type(source_info.source_type);
        let source_attr_name =
            Token::new(&format!("{}{}", prefix, source_info.source_name.as_str()));

        // Try to get existing attribute
        if let Some(attr) = source_prim.get_attribute(source_attr_name.as_str()) {
            return attr;
        }

        // Create new attribute
        // C++ uses sourceInfo.typeName if valid, else fallback — never a hardcoded default
        let type_name = if source_info.type_name.is_valid() {
            source_info.type_name.clone()
        } else if fallback_type_name.is_valid() {
            fallback_type_name
        } else {
            return Attribute::invalid();
        };

        if source_prim
            .create_attribute(
                source_attr_name.as_str(),
                &type_name,
                false, // custom = false
                None,  // variability = default
            )
            .is_none()
        {
            return Attribute::invalid();
        }

        // Re-fetch via get_attribute to get a composed-query handle
        // (create_attribute returns a raw handle whose is_valid() may fail
        //  due to stale PrimIndex cache)
        source_prim
            .get_attribute(source_attr_name.as_str())
            .unwrap_or_else(Attribute::invalid)
    }

    /// Authors a connection using deprecated API (ConnectableAPI, sourceName, sourceType).
    pub fn connect_to_source_deprecated(
        shading_attr: &Attribute,
        source: &ConnectableAPI,
        source_name: &Token,
        source_type: AttributeType,
        type_name: ValueTypeName,
    ) -> bool {
        let source_info = ConnectionSourceInfo::from_connectable(
            source.clone(),
            source_name.clone(),
            source_type,
            type_name,
        );
        Self::connect_to_source(shading_attr, &source_info, ConnectionModification::Replace)
    }

    /// Authors a connection to the source at the given path.
    pub fn connect_to_source_path(shading_attr: &Attribute, source_path: &Path) -> bool {
        let Some(stage) = shading_attr.stage() else {
            return false;
        };
        let source_info = ConnectionSourceInfo::from_path(stage, source_path);
        Self::connect_to_source(shading_attr, &source_info, ConnectionModification::Replace)
    }

    /// Authors a connection to the given source input.
    pub fn connect_to_source_input(shading_attr: &Attribute, source_input: &Input) -> bool {
        let source_info = ConnectionSourceInfo::from_input(source_input);
        Self::connect_to_source(shading_attr, &source_info, ConnectionModification::Replace)
    }

    /// Authors a connection to the given source output.
    pub fn connect_to_source_output(shading_attr: &Attribute, source_output: &Output) -> bool {
        let source_info = ConnectionSourceInfo::from_output(source_output);
        Self::connect_to_source(shading_attr, &source_info, ConnectionModification::Replace)
    }

    /// Authors a list of connections for a given shading attribute.
    pub fn set_connected_sources(
        shading_attr: &Attribute,
        source_infos: &[ConnectionSourceInfo],
    ) -> bool {
        let mut source_paths = Vec::new();

        // Guard: ensure attribute has a valid stage before authoring connections.
        if shading_attr.stage().is_none() {
            return false;
        }
        // C++ connectableAPI.cpp:253: use shadingAttr.GetTypeName() (composed, stage-level).
        // Same fix pattern as P1-SHADE-2/3/4 — do not read from root_layer only.
        let fallback_type_name = shading_attr.get_type_name();

        for source_info in source_infos {
            if !source_info.is_valid() {
                return false;
            }

            let source_attr =
                Self::_get_or_create_source_attr(source_info, fallback_type_name.clone());
            if source_attr.path().is_empty() || !source_attr.path().is_property_path() {
                return false;
            }

            source_paths.push(source_attr.path().clone());
        }

        shading_attr.set_connections(source_paths)
    }

    /// Finds the valid sources of connections for the given shading attribute.
    pub fn get_connected_sources(
        shading_attr: &Attribute,
        invalid_source_paths: &mut Vec<Path>,
    ) -> Vec<ConnectionSourceInfo> {
        let mut source_paths = Vec::new();
        if !shading_attr.get_connections_to(&mut source_paths) {
            return Vec::new();
        }

        let mut source_infos = Vec::new();
        if source_paths.is_empty() {
            return source_infos;
        }

        let Some(stage) = shading_attr.stage() else {
            return Vec::new();
        };

        for source_path in source_paths {
            if !source_path.is_property_path() {
                invalid_source_paths.push(source_path);
                continue;
            }

            // Check that the attribute has a legal prefix
            let name_token = Token::new(source_path.get_name());
            let (source_name, source_type) = Utils::get_base_name_and_type(&name_token);
            if source_type == AttributeType::Invalid {
                invalid_source_paths.push(source_path);
                continue;
            }

            // Get source prim
            let source_prim_path = source_path.get_prim_path();
            let Some(source_prim) = stage.get_prim_at_path(&source_prim_path) else {
                invalid_source_paths.push(source_path);
                continue;
            };

            // C++ uses stage->GetAttributeAtPath which checks existence.
            // We use prim.get_attribute + has_attribute to verify attr exists
            // without hitting stale PrimIndex validation.
            let prop_name = source_path.get_name();
            if !source_prim.has_attribute(prop_name) {
                invalid_source_paths.push(source_path);
                continue;
            }
            let Some(source_attr) = source_prim.get_attribute(prop_name) else {
                invalid_source_paths.push(source_path);
                continue;
            };

            let source = ConnectableAPI::new(source_prim);
            let type_name = source_attr.get_type_name();

            source_infos.push(ConnectionSourceInfo {
                source,
                source_name,
                source_type,
                type_name,
            });
        }

        source_infos
    }

    /// Returns true if and only if the shading attribute is currently connected
    /// to at least one valid source.
    pub fn has_connected_source(shading_attr: &Attribute) -> bool {
        !Self::get_connected_sources(shading_attr, &mut Vec::new()).is_empty()
    }

    /// Returns true if the connection to the given shading attribute's source
    /// is authored across a specializes arc.
    ///
    /// C++ implementation:
    /// 1. GetPropertyStack to find strongest spec with connections
    /// 2. Traverse PcpPrimIndex to find which node introduced that spec
    /// 3. Check if any node in origin chain has PcpArcTypeSpecialize
    ///
    /// Partial implementation: checks property stack for connection specs,
    /// but cannot yet verify specialize-arc origin (requires PcpNodeRef
    /// traversal via Prim::GetPrimIndex which is not exposed).
    pub fn is_source_connection_from_base_material(shading_attr: &Attribute) -> bool {
        // Check if this attribute has any connections at all in its property stack.
        let prop_stack = shading_attr.as_property().get_property_stack();
        let has_connections = prop_stack.iter().any(|spec| {
            spec.as_attribute()
                .map(|a| a.has_connection_paths())
                .unwrap_or(false)
        });
        if !has_connections {
            return false;
        }

        // Full PcpPrimIndex traversal is not yet exposed. As an approximation
        // matching C++ semantics, we check if the prim has specializes arcs and
        // if the corresponding attribute on the base material carries the same
        // connections — indicating the connection was authored on the base material.
        let Some(stage) = shading_attr.stage() else {
            return false;
        };
        let attr_prim_path = shading_attr.path().get_prim_path();
        let Some(prim) = stage.get_prim_at_path(&attr_prim_path) else {
            return false;
        };

        let specializes_token = Token::new("specializes");
        let Some(list_op) = prim.get_metadata::<usd_sdf::list_op::PathListOp>(&specializes_token)
        else {
            return false;
        };

        // Collect all specializes target paths (prepended then appended).
        let specialize_paths: Vec<Path> = list_op
            .get_prepended_items()
            .iter()
            .chain(list_op.get_appended_items().iter())
            .cloned()
            .collect();
        if specialize_paths.is_empty() {
            return false;
        }

        // Compare local connections against the base material attribute's connections.
        let attr_name = shading_attr.name();
        let local_connections = shading_attr.get_connections();
        if local_connections.is_empty() {
            return false;
        }

        for base_path in &specialize_paths {
            let Some(base_attr_path) = base_path.append_property(attr_name.as_str()) else {
                continue;
            };
            if let Some(base_attr) = stage.get_attribute_at_path(&base_attr_path) {
                let base_connections = base_attr.get_connections();
                if !base_connections.is_empty() && base_connections == local_connections {
                    return true;
                }
            }
        }

        false
    }

    /// Disconnect source for this shading attribute.
    pub fn disconnect_source(shading_attr: &Attribute, source_attr: Option<&Attribute>) -> bool {
        if let Some(source) = source_attr {
            shading_attr.remove_connection(source.path())
        } else {
            shading_attr.set_connections(Vec::new())
        }
    }

    /// Clears sources for this shading attribute in the current EditTarget.
    pub fn clear_sources(shading_attr: &Attribute) -> bool {
        shading_attr.clear_connections()
    }

    /// Returns the "raw" (authored) connected source paths for the given shading attribute.
    pub fn get_raw_connected_source_paths(
        shading_attr: &Attribute,
        source_paths: &mut Vec<Path>,
    ) -> bool {
        *source_paths = shading_attr.get_connections();
        true
    }

    // ========================================================================
    // Inputs API
    // ========================================================================

    /// Create an input which can both have a value and be connected.
    pub fn create_input(&self, name: &Token, type_name: &ValueTypeName) -> Input {
        Input::new(self.base.get_prim(), name, type_name).unwrap_or_else(Input::invalid)
    }

    /// Return the requested input if it exists.
    pub fn get_input(&self, name: &Token) -> Input {
        let input_attr_name = Token::new(&format!("{}{}", tokens().inputs.as_str(), name.as_str()));
        if self.base.get_prim().has_attribute(input_attr_name.as_str()) {
            if let Some(attr) = self.base.get_prim().get_attribute(input_attr_name.as_str()) {
                return Input::from_attribute(attr);
            }
        }
        Input::invalid()
    }

    /// Returns all inputs on the connectable prim.
    pub fn get_inputs(&self, only_authored: bool) -> Vec<Input> {
        let props = if only_authored {
            self.base
                .get_prim()
                .get_authored_properties_in_namespace(&tokens().inputs)
        } else {
            self.base
                .get_prim()
                .get_properties_in_namespace(&tokens().inputs)
        };

        let mut inputs = Vec::new();
        for prop in props {
            if let Some(attr) = prop.as_attribute() {
                inputs.push(Input::from_attribute(attr));
            }
        }
        inputs
    }

    // ========================================================================
    // Outputs API
    // ========================================================================

    /// Create an output, which represents an externally computed, typed value.
    pub fn create_output(&self, name: &Token, type_name: &ValueTypeName) -> Output {
        Output::new(self.base.get_prim(), name, type_name).unwrap_or_else(Output::invalid)
    }

    /// Return the requested output if it exists.
    pub fn get_output(&self, name: &Token) -> Output {
        let output_attr_name =
            Token::new(&format!("{}{}", tokens().outputs.as_str(), name.as_str()));
        if self
            .base
            .get_prim()
            .has_attribute(output_attr_name.as_str())
        {
            if let Some(attr) = self
                .base
                .get_prim()
                .get_attribute(output_attr_name.as_str())
            {
                return Output::from_attribute(attr);
            }
        }
        Output::invalid()
    }

    /// Returns all outputs on the connectable prim.
    pub fn get_outputs(&self, only_authored: bool) -> Vec<Output> {
        let props = if only_authored {
            self.base
                .get_prim()
                .get_authored_properties_in_namespace(&tokens().outputs)
        } else {
            self.base
                .get_prim()
                .get_properties_in_namespace(&tokens().outputs)
        };

        let mut outputs = Vec::new();
        for prop in props {
            if let Some(attr) = prop.as_attribute() {
                outputs.push(Output::from_attribute(attr));
            }
        }
        outputs
    }

    /// Returns true if the given schema type has a ConnectableAPI behavior
    /// registered for it.
    ///
    /// Matches C++ `HasConnectableAPI(const TfType& schemaType)` /
    /// `HasConnectableAPI<T>()`.
    pub fn has_connectable_api(type_name: &str) -> bool {
        use super::connectable_api_behavior::has_behavior_for_type;
        // Check registered behaviors and known connectable types
        has_behavior_for_type(type_name)
            || type_name == "Shader"
            || type_name == "NodeGraph"
            || type_name == "Material"
    }
}

impl PartialEq for ConnectableAPI {
    fn eq(&self, other: &Self) -> bool {
        self.base.get_prim().path() == other.base.get_prim().path()
    }
}

impl Eq for ConnectableAPI {}
