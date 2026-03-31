//! USD Shade NodeGraph - container for shading nodes.
//!
//! Port of pxr/usd/usdShade/nodeGraph.h and nodeGraph.cpp
//!
//! A node-graph is a container for shading nodes, as well as other node-graphs.
//! It has a public input interface and provides a list of public outputs.

use super::connectable_api::ConnectableAPI;
use super::input::Input;
use super::output::Output;
use super::types::AttributeType;
use super::utils::Utils;
use std::collections::HashMap;
use std::sync::Arc;
use usd_core::prim::Prim;
use usd_core::schema_base::SchemaBase;
use usd_core::stage::Stage;
use usd_core::typed::Typed;
use usd_sdf::Path;
use usd_sdf::ValueTypeName;
use usd_tf::Token;

/// A node-graph is a container for shading nodes, as well as other node-graphs.
///
/// It has a public input interface and provides a list of public outputs.
#[derive(Debug, Clone)]
pub struct NodeGraph {
    /// Base typed schema.
    typed: Typed,
}

impl NodeGraph {
    /// Construct a NodeGraph on UsdPrim.
    pub fn new(prim: Prim) -> Self {
        Self {
            typed: Typed::new(prim),
        }
    }

    /// Construct a NodeGraph from a SchemaBase.
    pub fn from_schema_base(schema: SchemaBase) -> Self {
        Self {
            typed: Typed::new(schema.prim().clone()),
        }
    }

    /// Construct a NodeGraph from a ConnectableAPI.
    ///
    /// Allow implicit (auto) conversion of UsdShadeConnectableAPI to UsdShadeNodeGraph.
    pub fn from_connectable_api(connectable: &ConnectableAPI) -> Self {
        Self::new(connectable.get_prim())
    }

    /// Creates an invalid NodeGraph.
    pub fn invalid() -> Self {
        Self {
            typed: Typed::invalid(),
        }
    }

    /// Return a NodeGraph holding the prim adhering to this schema at `path` on `stage`.
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Self {
        if let Some(prim) = stage.get_prim_at_path(path) {
            Self::new(prim)
        } else {
            Self::invalid()
        }
    }

    /// Attempt to ensure a UsdPrim adhering to this schema at `path` is defined on this stage.
    pub fn define(stage: &Arc<Stage>, path: &Path) -> Self {
        match stage.define_prim(path.to_string(), "NodeGraph") {
            Ok(prim) => Self::new(prim),
            Err(_) => Self::invalid(),
        }
    }

    /// Returns true if this NodeGraph is valid.
    ///
    /// Per C++ UsdShadeNodeGraph: accepts any prim that IsA<UsdShadeNodeGraph>,
    /// which includes Material and any user-defined types deriving from NodeGraph.
    pub fn is_valid(&self) -> bool {
        if !self.typed.is_valid() {
            return false;
        }
        // Use schema hierarchy check (matches C++ IsA<UsdShadeNodeGraph>)
        self.typed.prim().is_a(&usd_tf::Token::new("NodeGraph"))
    }

    /// Returns the wrapped prim.
    pub fn get_prim(&self) -> Prim {
        self.typed.prim().clone()
    }

    /// Returns the path to this prim.
    pub fn path(&self) -> &Path {
        self.typed.prim().path()
    }

    /// Returns the stage.
    pub fn stage(&self) -> Option<Arc<Stage>> {
        self.typed.prim().stage()
    }

    /// Return a vector of names of all pre-declared attributes for this schema
    /// class and all its ancestor classes.
    /// Cached per C++ pattern (static local vectors).
    pub fn get_schema_attribute_names(include_inherited: bool) -> &'static Vec<Token> {
        use std::sync::OnceLock;
        static LOCAL: OnceLock<Vec<Token>> = OnceLock::new();
        static ALL: OnceLock<Vec<Token>> = OnceLock::new();

        if include_inherited {
            ALL.get_or_init(|| Typed::get_schema_attribute_names(true))
        } else {
            LOCAL.get_or_init(Vec::new) // NodeGraph doesn't add any attributes itself
        }
    }

    /// Constructs and returns a UsdShadeConnectableAPI object with this node-graph.
    pub fn connectable_api(&self) -> ConnectableAPI {
        ConnectableAPI::new(self.get_prim())
    }

    // ========================================================================
    // Outputs API
    // ========================================================================

    /// Create an output which can either have a value or can be connected.
    pub fn create_output(&self, name: &Token, type_name: &ValueTypeName) -> Output {
        self.connectable_api().create_output(name, type_name)
    }

    /// Return the requested output if it exists.
    pub fn get_output(&self, name: &Token) -> Output {
        self.connectable_api().get_output(name)
    }

    /// Returns all outputs on the node-graph.
    pub fn get_outputs(&self, only_authored: bool) -> Vec<Output> {
        self.connectable_api().get_outputs(only_authored)
    }

    /// \deprecated in favor of GetValueProducingAttributes on UsdShadeOutput
    /// Resolves the connection source of the requested output to a shader output.
    ///
    /// Returns a valid shader object if the specified output exists and is connected to one.
    /// Returns an invalid shader object otherwise.
    pub fn compute_output_source(
        &self,
        output_name: &Token,
        source_name: &mut Token,
        source_type: &mut AttributeType,
    ) -> super::shader::Shader {
        // Check that we have a legit output
        let output = self.get_output(output_name);
        if !output.is_defined() {
            return super::shader::Shader::invalid();
        }

        let value_attrs = Utils::get_value_producing_attributes_output(&output, true);

        if value_attrs.is_empty() {
            return super::shader::Shader::invalid();
        }

        if value_attrs.len() > 1 {
            eprintln!(
                "Found multiple upstream attributes for output {} on NodeGraph {}. ComputeOutputSource will only report the first upstream UsdShadeShader.",
                output_name.as_str(),
                self.path()
            );
        }

        let attr = &value_attrs[0];
        let name_token = attr.name();
        let (name, attr_type) = Utils::get_base_name_and_type(&name_token);
        *source_name = name;
        *source_type = attr_type;

        let prim_path = attr.prim_path();
        let Some(stage) = attr.stage() else {
            return super::shader::Shader::invalid();
        };
        let Some(prim) = stage.get_prim_at_path(&prim_path) else {
            return super::shader::Shader::invalid();
        };
        let shader = super::shader::Shader::new(prim);

        if *source_type != AttributeType::Output || !shader.is_valid() {
            return super::shader::Shader::invalid();
        }

        shader
    }

    // ========================================================================
    // Inputs API
    // ========================================================================

    /// Create an Input which can either have a value or can be connected.
    pub fn create_input(&self, name: &Token, type_name: &ValueTypeName) -> Input {
        self.connectable_api().create_input(name, type_name)
    }

    /// Return the requested input if it exists.
    pub fn get_input(&self, name: &Token) -> Input {
        self.connectable_api().get_input(name)
    }

    /// Returns all inputs present on the node-graph.
    pub fn get_inputs(&self, only_authored: bool) -> Vec<Input> {
        self.connectable_api().get_inputs(only_authored)
    }

    /// Returns all the "Interface Inputs" of the node-graph.
    ///
    /// This is the same as GetInputs(), but is provided as a convenience.
    pub fn get_interface_inputs(&self) -> Vec<Input> {
        self.get_inputs(true)
    }

    // ========================================================================
    // Interface Input Consumers Map
    // ========================================================================

    // Map of interface inputs to corresponding vectors of inputs that consume their values.
    // Using type alias in a way that works with HashMap
}

/// Map of interface inputs to corresponding vectors of inputs that consume their values.
pub type InterfaceInputConsumersMap = HashMap<Input, Vec<Input>>;

impl NodeGraph {
    /// Walks the namespace subtree below the node-graph and computes a map
    /// containing the list of all inputs on the node-graph and the associated
    /// vector of consumers of their values.
    ///
    /// If `compute_transitive_consumers` is true, then value consumers
    /// belonging to node-graphs are resolved transitively to compute the
    /// transitive mapping from inputs on the node-graph to inputs on shaders
    /// inside the material.
    pub fn compute_interface_input_consumers_map(
        &self,
        compute_transitive_consumers: bool,
    ) -> HashMap<Input, Vec<Input>> {
        let result = Self::_compute_non_transitive_input_consumers_map(self);

        if !compute_transitive_consumers {
            return result;
        }

        // Collect all node-graphs for which we must compute the input-consumers map.
        let mut node_graph_input_consumers: HashMap<NodeGraph, InterfaceInputConsumersMap> =
            HashMap::new();
        Self::_recursive_compute_node_graph_interface_input_consumers(
            &result,
            &mut node_graph_input_consumers,
        );

        // If there are no consumers belonging to node-graphs, we're done.
        if node_graph_input_consumers.is_empty() {
            return result;
        }

        // Resolve transitive consumers
        let mut resolved = HashMap::new();
        for (input, consumers) in result {
            let mut resolved_consumers = Vec::new();
            for consumer in consumers {
                let mut nested_consumers = Vec::new();
                Self::_resolve_consumers(
                    &consumer,
                    &node_graph_input_consumers,
                    &mut nested_consumers,
                );
                resolved_consumers.extend(nested_consumers);
            }
            resolved.insert(input, resolved_consumers);
        }

        resolved
    }

    /// Helper to compute non-transitive input consumers map.
    fn _compute_non_transitive_input_consumers_map(
        node_graph: &NodeGraph,
    ) -> HashMap<Input, Vec<Input>> {
        let mut result = HashMap::new();

        // Initialize map with all inputs
        for input in node_graph.get_inputs(true) {
            result.insert(input, Vec::new());
        }

        // Walk descendants and find consumers
        let prim = node_graph.get_prim();
        for descendant in prim.descendants() {
            let connectable = ConnectableAPI::new(descendant.clone());
            if !connectable.is_valid() {
                continue;
            }

            let internal_inputs = connectable.get_inputs(true);
            for internal_input in internal_inputs {
                let mut invalid_paths = Vec::new();
                let sources = internal_input.get_connected_sources(&mut invalid_paths);

                for source_info in sources {
                    if source_info.source.get_prim().path() == prim.path()
                        && Self::_is_valid_input(&source_info.source, source_info.source_type)
                    {
                        let interface_input = node_graph.get_input(&source_info.source_name);
                        if let Some(consumers) = result.get_mut(&interface_input) {
                            consumers.push(internal_input.clone());
                        }
                    }
                }
            }
        }

        result
    }

    /// Helper to check if source is a valid input.
    fn _is_valid_input(_source: &ConnectableAPI, source_type: AttributeType) -> bool {
        source_type == AttributeType::Input
    }

    /// Recursive helper to compute node graph interface input consumers.
    fn _recursive_compute_node_graph_interface_input_consumers(
        input_consumers_map: &HashMap<Input, Vec<Input>>,
        node_graph_input_consumers: &mut HashMap<NodeGraph, HashMap<Input, Vec<Input>>>,
    ) {
        for consumers in input_consumers_map.values() {
            for consumer in consumers {
                let consumer_prim = consumer.get_prim();
                // Check if consumer prim is a NodeGraph or Material (per C++ IsA<UsdShadeNodeGraph>)
                let type_name = consumer_prim.type_name();
                let tn = type_name.as_str();
                if tn == "NodeGraph" || tn == "Material" {
                    let consumer_node_graph = NodeGraph::new(consumer_prim);
                    if !node_graph_input_consumers.contains_key(&consumer_node_graph) {
                        let ir_map =
                            Self::_compute_non_transitive_input_consumers_map(&consumer_node_graph);
                        node_graph_input_consumers
                            .insert(consumer_node_graph.clone(), ir_map.clone());
                        Self::_recursive_compute_node_graph_interface_input_consumers(
                            &ir_map,
                            node_graph_input_consumers,
                        );
                    }
                }
            }
        }
    }

    /// Resolve consumers transitively.
    fn _resolve_consumers(
        consumer: &Input,
        node_graph_input_consumers: &HashMap<NodeGraph, HashMap<Input, Vec<Input>>>,
        resolved_consumers: &mut Vec<Input>,
    ) {
        let consumer_prim = consumer.get_prim();
        let consumer_node_graph = NodeGraph::new(consumer_prim.clone());

        if !consumer_node_graph.is_valid() {
            resolved_consumers.push(consumer.clone());
            return;
        }

        if let Some(input_consumers) = node_graph_input_consumers.get(&consumer_node_graph) {
            if let Some(consumers) = input_consumers.get(consumer) {
                if !consumers.is_empty() {
                    for nested_consumer in consumers {
                        Self::_resolve_consumers(
                            nested_consumer,
                            node_graph_input_consumers,
                            resolved_consumers,
                        );
                    }
                } else {
                    // If the node-graph input has no consumers, then add it to the list
                    resolved_consumers.push(consumer.clone());
                }
            } else {
                resolved_consumers.push(consumer.clone());
            }
        } else {
            resolved_consumers.push(consumer.clone());
        }
    }
}

impl PartialEq for NodeGraph {
    fn eq(&self, other: &Self) -> bool {
        self.get_prim().path() == other.get_prim().path()
    }
}

impl Eq for NodeGraph {}

impl std::hash::Hash for NodeGraph {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.get_prim().path().hash(state);
    }
}
