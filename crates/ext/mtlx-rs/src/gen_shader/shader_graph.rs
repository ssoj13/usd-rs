//! ShaderGraph — graph (DAG) for shader generation (ref: MaterialX ShaderGraph).

use std::collections::{HashMap, HashSet, VecDeque};

use crate::core::Document;

use super::ShaderGraphCreateContext;
use super::TypeDesc;
use super::color_management::ColorSpaceTransform;
use super::shader_node::{ShaderInput, ShaderNode, ShaderOutput};
use super::shader_node_factory::create_node_from_nodedef;
use super::unit_system::UnitTransform;

/// Input socket of a graph — receives data from outside. Maps to ShaderOutput on the root node.
pub type ShaderGraphInputSocket = ShaderOutput;

/// Output socket of a graph — provides data to outside. Maps to ShaderInput on the root node.
pub type ShaderGraphOutputSocket = ShaderInput;

/// Key for downstream connection index: (upstream_node, upstream_output)
type OutputKey = (String, String);
/// Value: (downstream_node, downstream_input)
type InputKey = (String, String);

/// Shader graph — contains nodes and input/output sockets (по рефу ShaderGraph).
/// Input sockets = root node outputs. Output sockets = root node inputs.
#[derive(Debug)]
pub struct ShaderGraph {
    /// Root node (the graph itself)
    pub node: ShaderNode,
    /// Child nodes by name
    pub nodes: HashMap<String, ShaderNode>,
    /// Order of child nodes (topological)
    pub node_order: Vec<String>,
    /// Identifier map for unique variable names
    pub identifiers: HashMap<String, usize>,
    /// Downstream connections: (upstream_node, upstream_output) -> [(downstream_node, downstream_input)]
    downstream_connections: HashMap<OutputKey, Vec<InputKey>>,
    /// NodeDef name per shader node (for create_variables lookup)
    node_def_names: HashMap<String, String>,
    /// Deferred color transforms (node_name, port_name) -> transform (по рефу _inputColorTransformMap etc.)
    pub(crate) input_color_transforms: Vec<(String, String, ColorSpaceTransform)>,
    pub(crate) output_color_transforms: Vec<(String, String, ColorSpaceTransform)>,
    pub(crate) input_unit_transforms: Vec<(String, String, UnitTransform)>,
    pub(crate) output_unit_transforms: Vec<(String, String, UnitTransform)>,
}

impl ShaderGraph {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            node: ShaderNode::new(name),
            nodes: HashMap::new(),
            node_order: Vec::new(),
            identifiers: HashMap::new(),
            downstream_connections: HashMap::new(),
            node_def_names: HashMap::new(),
            input_color_transforms: Vec::new(),
            output_color_transforms: Vec::new(),
            input_unit_transforms: Vec::new(),
            output_unit_transforms: Vec::new(),
        }
    }

    /// Make connection from downstream (node, input) to upstream (up_node, up_output).
    /// Updates both ShaderInput.connection and downstream index (по рефу ShaderInput::makeConnection).
    pub fn make_connection(
        &mut self,
        down_node: &str,
        down_input: &str,
        up_node: &str,
        up_output: &str,
    ) -> Result<(), String> {
        let old_key = self
            .get_input(down_node, down_input)
            .ok_or_else(|| format!("Input {}.{} not found", down_node, down_input))?
            .connection
            .clone();
        if let Some((old_up, old_out)) = old_key {
            if let Some(v) = self.downstream_connections.get_mut(&(old_up, old_out)) {
                v.retain(|(n, i)| n != down_node || i != down_input);
            }
        }
        self.get_input_mut(down_node, down_input)
            .ok_or_else(|| format!("Input {}.{} not found", down_node, down_input))?
            .make_connection(up_node, up_output);
        self.downstream_connections
            .entry((up_node.to_string(), up_output.to_string()))
            .or_default()
            .push((down_node.to_string(), down_input.to_string()));
        Ok(())
    }

    /// Break connection on (node, input) (по рефу ShaderInput::breakConnection).
    pub fn break_connection(&mut self, down_node: &str, down_input: &str) {
        if let Some(inp) = self.get_input_mut(down_node, down_input) {
            if let Some((up_n, up_o)) = inp.connection.take() {
                if let Some(v) = self.downstream_connections.get_mut(&(up_n, up_o)) {
                    v.retain(|(n, i)| n != down_node || i != down_input);
                }
            }
        }
    }

    /// Get inputs connected to (node, output) — по рефу ShaderOutput::getConnections.
    pub fn get_connections_for_output(&self, up_node: &str, up_output: &str) -> Vec<InputKey> {
        self.downstream_connections
            .get(&(up_node.to_string(), up_output.to_string()))
            .cloned()
            .unwrap_or_default()
    }

    fn get_input(&self, node_name: &str, input_name: &str) -> Option<&ShaderInput> {
        if node_name == self.get_name() {
            self.node.inputs.get(input_name)
        } else {
            self.nodes.get(node_name)?.inputs.get(input_name)
        }
    }

    fn get_input_mut(&mut self, node_name: &str, input_name: &str) -> Option<&mut ShaderInput> {
        if node_name == self.get_name() {
            self.node.inputs.get_mut(input_name)
        } else {
            self.nodes.get_mut(node_name)?.inputs.get_mut(input_name)
        }
    }

    pub fn get_name(&self) -> &str {
        self.node.get_name()
    }

    /// Add input socket (data feeds into the graph from outside).
    /// Implementation: adds to node's outputs.
    pub fn add_input_socket(
        &mut self,
        name: impl Into<String>,
        type_desc: TypeDesc,
    ) -> &mut ShaderOutput {
        self.node.add_output(name, type_desc)
    }

    /// Add output socket (data comes out of the graph to outside).
    /// Implementation: adds to node's inputs.
    pub fn add_output_socket(
        &mut self,
        name: impl Into<String>,
        type_desc: TypeDesc,
    ) -> &mut ShaderInput {
        self.node.add_input(name, type_desc)
    }

    /// Number of input sockets
    pub fn num_input_sockets(&self) -> usize {
        self.node.num_outputs()
    }

    /// Number of output sockets
    pub fn num_output_sockets(&self) -> usize {
        self.node.num_inputs()
    }

    pub fn get_input_socket(&self, name: &str) -> Option<&ShaderOutput> {
        self.node.get_output(name)
    }

    pub fn get_output_socket(&self, name: &str) -> Option<&ShaderInput> {
        self.node.get_input(name)
    }

    pub fn get_output_socket_mut(&mut self, name: &str) -> Option<&mut ShaderInput> {
        self.node.inputs.get_mut(name)
    }

    pub fn get_input_socket_at(&self, index: usize) -> Option<&ShaderOutput> {
        self.node
            .output_order
            .get(index)
            .and_then(|n| self.node.outputs.get(n))
    }

    pub fn get_output_socket_at(&self, index: usize) -> Option<&ShaderInput> {
        self.node
            .input_order
            .get(index)
            .and_then(|n| self.node.inputs.get(n))
    }

    /// Check if graph input socket is editable (по рефу ShaderGraph::isEditable).
    /// Default: true (matches ShaderNodeImpl::isEditable default).
    pub fn is_editable(&self, socket_name: &str) -> bool {
        let _ = socket_name;
        true
    }

    /// Add a child node to the graph
    pub fn add_node(&mut self, node: ShaderNode) -> Option<&ShaderNode> {
        let name = node.name.clone();
        if !self.node_order.contains(&name) {
            self.node_order.push(name.clone());
        }
        self.nodes.insert(name.clone(), node);
        self.nodes.get(&name)
    }

    /// Set NodeDef name for a shader node (for create_variables lookup)
    pub fn set_node_def(&mut self, node_name: &str, node_def_name: &str) {
        self.node_def_names
            .insert(node_name.to_string(), node_def_name.to_string());
    }

    /// Get NodeDef name for a shader node
    pub fn get_node_def(&self, node_name: &str) -> Option<&str> {
        self.node_def_names.get(node_name).map(|s| s.as_str())
    }

    /// Create and add a new node
    pub fn create_node(&mut self, name: impl Into<String>) -> &mut ShaderNode {
        let name = name.into();
        if !self.node_order.contains(&name) {
            self.node_order.push(name.clone());
        }
        self.nodes
            .entry(name.clone())
            .or_insert_with(|| ShaderNode::new(name))
    }

    pub fn get_node(&self, name: &str) -> Option<&ShaderNode> {
        self.nodes.get(name)
    }

    pub fn get_node_mut(&mut self, name: &str) -> Option<&mut ShaderNode> {
        self.nodes.get_mut(name)
    }

    pub fn get_nodes(&self) -> impl Iterator<Item = &ShaderNode> {
        self.node_order.iter().filter_map(|n| self.nodes.get(n))
    }

    pub fn has_classification(&self, c: u32) -> bool {
        self.node.has_classification(c)
    }

    /// Check if graph has CLOSURE classification.
    pub fn has_classification_closure(&self) -> bool {
        self.has_classification(super::ShaderNodeClassification::CLOSURE)
    }

    /// Check if graph has SHADER classification.
    pub fn has_classification_shader(&self) -> bool {
        self.has_classification(super::ShaderNodeClassification::SHADER)
    }

    /// Check if graph has SURFACE classification.
    pub fn has_classification_surface(&self) -> bool {
        self.has_classification(super::ShaderNodeClassification::SURFACE)
    }

    /// Check if the graph requires lighting (has any closure nodes).
    /// Matches C++ HwShaderGenerator::requiresLighting.
    pub fn requires_lighting(&self) -> bool {
        self.has_classification(super::ShaderNodeClassification::CLOSURE)
    }

    pub fn get_identifier_map(&mut self) -> &mut HashMap<String, usize> {
        &mut self.identifiers
    }

    /// Inline a node before the given output socket (по рефу ShaderGraph::inlineNodeBeforeOutput).
    /// Inserts new_node between the output and its upstream connection. Document must contain
    /// the NodeDef. Returns Ok(new_node_name) or Err.
    pub fn inline_node_before_output(
        &mut self,
        output_index: usize,
        new_node_name: &str,
        node_def_name: &str,
        input_name: &str,
        output_name: &str,
        doc: &Document,
        context: &dyn ShaderGraphCreateContext,
    ) -> Result<String, String> {
        let node_def = doc
            .get_node_def(node_def_name)
            .ok_or_else(|| format!("NodeDef '{}' not found", node_def_name))?;
        let _ = node_def.borrow().get_child(output_name).ok_or_else(|| {
            format!(
                "Output '{}' not found on NodeDef '{}'",
                output_name, node_def_name
            )
        })?;
        if !input_name.is_empty() {
            let _ = node_def.borrow().get_child(input_name).ok_or_else(|| {
                format!(
                    "Input '{}' not found on NodeDef '{}'",
                    input_name, node_def_name
                )
            })?;
        }

        let out_socket_name = self
            .node
            .input_order
            .get(output_index)
            .cloned()
            .ok_or_else(|| format!("Output socket index {} out of bounds", output_index))?;

        let original_upstream: Option<(String, String)> = self
            .get_input(self.get_name(), &out_socket_name)
            .and_then(|i| i.get_connection())
            .map(|(a, b)| (a.to_string(), b.to_string()));

        if let Some((up_n, up_o)) = original_upstream.as_ref() {
            if !input_name.is_empty() {
                let up_type = self
                    .get_output(up_n, up_o)
                    .map(|p| p.get_type().get_name().to_string())
                    .unwrap_or_default();
                let inp_type = node_def
                    .borrow()
                    .get_child(input_name)
                    .and_then(|c| c.borrow().get_attribute("type").map(|s| s.to_string()))
                    .unwrap_or_default();
                if !up_type.is_empty() && !inp_type.is_empty() && up_type != inp_type {
                    return Err(format!(
                        "Type mismatch connecting {} to {}: '{}' vs '{}'",
                        up_n, input_name, up_type, inp_type
                    ));
                }
            }
        }

        let mut new_node = create_node_from_nodedef(new_node_name, &node_def, doc, context)?;
        let graph_name = self.get_name().to_string();

        let new_out_var = context.get_syntax().get_variable_name(
            &format!("{}_{}", new_node_name, output_name),
            new_node
                .outputs
                .get(output_name)
                .map(|p| p.get_type())
                .unwrap_or(&context.get_type_desc("float")),
            self.get_identifier_map(),
        );
        new_node
            .outputs
            .get_mut(output_name)
            .ok_or("output port")?
            .port_mut()
            .set_variable(&new_out_var);

        self.get_input_mut(&graph_name, &out_socket_name)
            .ok_or("output socket")?
            .make_connection(new_node_name, output_name);

        let new_out_type = new_node
            .outputs
            .get(output_name)
            .map(|p| p.get_type().clone())
            .unwrap_or_else(|| context.get_type_desc("float"));
        if let Some(socket) = self.node.inputs.get_mut(&out_socket_name) {
            socket.port_mut().type_desc = new_out_type;
        }

        if let Some((ref up_n, ref up_o)) = original_upstream {
            if !input_name.is_empty() {
                let inp_var = context.get_syntax().get_variable_name(
                    &format!("{}_{}", new_node_name, input_name),
                    new_node
                        .inputs
                        .get(input_name)
                        .map(|p| p.get_type())
                        .unwrap_or(&context.get_type_desc("float")),
                    self.get_identifier_map(),
                );
                if let Some(inp) = new_node.inputs.get_mut(input_name) {
                    inp.port_mut().set_variable(&inp_var);
                    inp.make_connection(up_n, up_o);
                }
                if let Some(v) = self
                    .downstream_connections
                    .get_mut(&(up_n.clone(), up_o.clone()))
                {
                    v.retain(|(n, i)| n != &graph_name || i != &out_socket_name);
                }
                self.downstream_connections
                    .entry((up_n.clone(), up_o.clone()))
                    .or_default()
                    .push((new_node_name.to_string(), input_name.to_string()));
            }
        }
        self.downstream_connections
            .entry((new_node_name.to_string(), output_name.to_string()))
            .or_default()
            .push((graph_name.clone(), out_socket_name.clone()));

        self.add_node(new_node);
        self.set_node_def(new_node_name, node_def_name);
        Ok(new_node_name.to_string())
    }

    fn get_output(&self, node_name: &str, output_name: &str) -> Option<&ShaderOutput> {
        if node_name == self.get_name() {
            self.node.get_output(output_name)
        } else {
            self.nodes.get(node_name)?.get_output(output_name)
        }
    }

    /// Resolve connection (node_name, output_name) to variable string.
    /// For graph input sockets the "node" is the graph itself (root).
    pub fn get_connection_variable(&self, node_name: &str, output_name: &str) -> Option<String> {
        if node_name == self.get_name() {
            self.node
                .get_output(output_name)
                .map(|o| o.port.get_variable().to_string())
        } else {
            self.nodes
                .get(node_name)?
                .get_output(output_name)
                .map(|o| o.port.get_variable().to_string())
        }
    }

    /// Disconnect all inputs and outputs of a node (по рефу ShaderGraph::disconnect).
    pub fn disconnect(&mut self, node_name: &str) {
        let (input_names, output_names): (Vec<String>, Vec<String>) =
            if node_name == self.get_name() {
                (
                    self.node.input_order.clone(),
                    self.node.output_order.clone(),
                )
            } else if let Some(node) = self.nodes.get(node_name) {
                (node.input_order.clone(), node.output_order.clone())
            } else {
                return;
            };

        for name in &input_names {
            self.break_connection(node_name, name);
        }
        for name in &output_names {
            let conns = self.get_connections_for_output(node_name, name);
            for (dn, di) in conns {
                self.break_connection(&dn, &di);
            }
        }
    }

    /// Topological sort of child nodes (по рефу ShaderGraph::topologicalSort, Kahn's algorithm).
    pub fn topological_sort(&mut self) {
        let graph_name = self.get_name().to_string();
        let child_names: Vec<String> = self.node_order.clone();

        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut node_queue: VecDeque<String> = VecDeque::new();

        for name in &child_names {
            let Some(node) = self.nodes.get(name) else {
                continue;
            };
            let mut count = 0usize;
            for inp in node.get_inputs() {
                if let Some((up_n, _)) = inp.get_connection() {
                    if up_n != graph_name {
                        count += 1;
                    }
                }
            }
            in_degree.insert(name.clone(), count);
            if count == 0 {
                node_queue.push_back(name.clone());
            }
        }

        let mut new_order: Vec<String> = Vec::with_capacity(child_names.len());
        while let Some(name) = node_queue.pop_front() {
            new_order.push(name.clone());
            if let Some(node) = self.nodes.get(&name) {
                for out_name in node.get_outputs().map(|o| o.get_name().to_string()) {
                    for (dn, _) in self.get_connections_for_output(&name, &out_name) {
                        if dn != graph_name {
                            if let Some(d) = in_degree.get_mut(&dn) {
                                *d = d.saturating_sub(1);
                                if *d == 0 {
                                    node_queue.push_back(dn.clone());
                                }
                            }
                        }
                    }
                }
            }
        }

        self.node_order = new_order;
    }

    /// Bypass a node: connect input's upstream to output's downstream (по рефу ShaderGraph::bypass).
    /// If input has no connection, push value/path/unit/colorspace downstream.
    pub fn bypass(
        &mut self,
        node_name: &str,
        input_index: usize,
        output_index: usize,
    ) -> Result<(), String> {
        let node = self
            .nodes
            .get(node_name)
            .ok_or_else(|| format!("Node '{}' not found", node_name))?;
        if input_index >= node.num_inputs() {
            return Err(format!(
                "Input index {} out of bounds for node '{}'",
                input_index, node_name
            ));
        }
        if output_index >= node.num_outputs() {
            return Err(format!(
                "Output index {} out of bounds for node '{}'",
                output_index, node_name
            ));
        }

        let inp_name = node
            .input_order
            .get(input_index)
            .cloned()
            .ok_or("input_order")?;
        let out_name = node
            .output_order
            .get(output_index)
            .cloned()
            .ok_or("output_order")?;

        let upstream: Option<(String, String)> = self
            .get_input(node_name, &inp_name)
            .and_then(|i| i.get_connection())
            .map(|(a, b)| (a.to_string(), b.to_string()));
        let inp_value = self
            .get_input(node_name, &inp_name)
            .and_then(|i| i.port.get_value())
            .cloned();
        let inp_path = self
            .get_input(node_name, &inp_name)
            .map(|i| i.port.path.clone())
            .unwrap_or_default();
        let inp_unit = self
            .get_input(node_name, &inp_name)
            .map(|i| i.port.unit.clone())
            .unwrap_or_default();
        let inp_cs = self
            .get_input(node_name, &inp_name)
            .map(|i| i.port.colorspace.clone())
            .unwrap_or_default();

        let downstream = self.get_connections_for_output(node_name, &out_name);

        for (dn, di) in downstream {
            self.break_connection(&dn, &di);
            if let Some((ref up_n, ref up_o)) = upstream {
                let _ = self.make_connection(&dn, &di, up_n, up_o);
            } else {
                if let Some(down_inp) = self.get_input_mut(&dn, &di) {
                    down_inp.port_mut().set_value(inp_value.clone(), false);
                    down_inp.port_mut().set_path(&inp_path);
                    if !inp_unit.is_empty() {
                        down_inp.port_mut().set_unit(&inp_unit);
                    }
                    if !inp_cs.is_empty() {
                        down_inp.port_mut().set_color_space(&inp_cs);
                    }
                }
            }
        }

        Ok(())
    }

    // ----- Color/Unit transform accessors (H-GS7) -----

    /// Register a color transform for an output port (по рефу _outputColorTransformMap).
    pub fn set_output_color_transform(&mut self, output: &str, transform: ColorSpaceTransform) {
        // Remove existing entry for the same output to avoid duplicates.
        self.output_color_transforms.retain(|(_, o, _)| o != output);
        self.output_color_transforms
            .push((self.node.name.clone(), output.to_string(), transform));
    }

    /// Register a unit transform for an output port (по рефу _outputUnitTransformMap).
    pub fn set_output_unit_transform(&mut self, output: &str, transform: UnitTransform) {
        self.output_unit_transforms.retain(|(_, o, _)| o != output);
        self.output_unit_transforms
            .push((self.node.name.clone(), output.to_string(), transform));
    }

    /// Get color transform for an output port, if any.
    pub fn get_output_color_transform(&self, output: &str) -> Option<&ColorSpaceTransform> {
        self.output_color_transforms
            .iter()
            .find(|(_, o, _)| o == output)
            .map(|(_, _, t)| t)
    }

    /// Get unit transform for an output port, if any.
    pub fn get_output_unit_transform(&self, output: &str) -> Option<&UnitTransform> {
        self.output_unit_transforms
            .iter()
            .find(|(_, o, _)| o == output)
            .map(|(_, _, t)| t)
    }

    /// Register a color transform for an input port (по рефу _inputColorTransformMap).
    pub fn set_input_color_transform(
        &mut self,
        node: &str,
        input: &str,
        transform: ColorSpaceTransform,
    ) {
        self.input_color_transforms
            .retain(|(n, i, _)| n != node || i != input);
        self.input_color_transforms
            .push((node.to_string(), input.to_string(), transform));
    }

    /// Register a unit transform for an input port (по рефу _inputUnitTransformMap).
    pub fn set_input_unit_transform(&mut self, node: &str, input: &str, transform: UnitTransform) {
        self.input_unit_transforms
            .retain(|(n, i, _)| n != node || i != input);
        self.input_unit_transforms
            .push((node.to_string(), input.to_string(), transform));
    }

    /// Get color transform for an input port, if any.
    pub fn get_input_color_transform(
        &self,
        node: &str,
        input: &str,
    ) -> Option<&ColorSpaceTransform> {
        self.input_color_transforms
            .iter()
            .find(|(n, i, _)| n == node && i == input)
            .map(|(_, _, t)| t)
    }

    /// Get unit transform for an input port, if any.
    pub fn get_input_unit_transform(&self, node: &str, input: &str) -> Option<&UnitTransform> {
        self.input_unit_transforms
            .iter()
            .find(|(n, i, _)| n == node && i == input)
            .map(|(_, _, t)| t)
    }

    /// Collect node names reachable from output sockets (used for optimize).
    fn collect_used_nodes(&self) -> HashSet<String> {
        let graph_name = self.get_name().to_string();
        let mut used = HashSet::new();
        let mut queue: VecDeque<String> = VecDeque::new();

        for i in 0..self.num_output_sockets() {
            if let Some(socket) = self.get_output_socket_at(i) {
                if let Some((up_n, _up_o)) = socket.get_connection() {
                    if up_n != graph_name && !used.contains(up_n) {
                        used.insert(up_n.to_string());
                        queue.push_back(up_n.to_string());
                    }
                }
            }
        }

        while let Some(n) = queue.pop_front() {
            if let Some(node) = self.nodes.get(&n) {
                for inp in node.get_inputs() {
                    if let Some((up_n, _)) = inp.get_connection() {
                        if up_n != graph_name && !used.contains(up_n) {
                            used.insert(up_n.to_string());
                            queue.push_back(up_n.to_string());
                        }
                    }
                }
            }
        }

        used
    }

    /// Optimize graph: elide CONSTANT/DOT nodes, remove unused (по рефу ShaderGraph::optimize).
    pub fn optimize(&mut self, elide_constant_nodes: bool) -> usize {
        use super::shader_node::ShaderNodeClassification;

        let type_system = crate::gen_shader::TypeSystem::new();
        let filename_type = type_system.get_type("filename");
        let mut num_edits = 0usize;

        let node_names: Vec<String> = self.node_order.clone();
        for node_name in &node_names {
            let node = match self.nodes.get(node_name) {
                Some(n) => n,
                None => continue,
            };
            if node.has_classification(ShaderNodeClassification::CONSTANT) {
                if node.num_inputs() != 1 || node.num_outputs() != 1 {
                    continue;
                }
                let can_elide = elide_constant_nodes
                    || node
                        .get_input("value")
                        .map(|i| i.get_type().get_name() == filename_type.get_name())
                        .unwrap_or(false);
                if can_elide {
                    let _ = self.bypass(node_name, 0, 0);
                    num_edits += 1;
                }
            } else if node.has_classification(ShaderNodeClassification::DOT) {
                if node.num_outputs() != 1 {
                    continue;
                }
                // C++ only elides DOT nodes when input "in" has FILENAME type.
                // Filename DOT nodes MUST be elided to avoid extra samplers.
                let is_filename = node
                    .get_input("in")
                    .map(|i| i.get_type().get_name() == filename_type.get_name())
                    .unwrap_or(false);
                if is_filename {
                    let _ = self.bypass(node_name, 0, 0);
                    num_edits += 1;
                }
            }
        }

        if num_edits > 0 {
            let used = self.collect_used_nodes();
            for name in self
                .node_order
                .iter()
                .filter(|n| !used.contains(*n))
                .cloned()
                .collect::<Vec<_>>()
            {
                self.disconnect(&name);
                self.nodes.remove(&name);
            }
            self.node_order.retain(|n| used.contains(n));
        }

        num_edits
    }

    // ----- ShaderGraphEdgeIterator traversal (ref: ShaderGraph::traverseUpstream) -----

    /// Return an iterator for DFS traversal upstream from the given output socket name.
    /// Matches C++ ShaderGraph::traverseUpstream(ShaderOutput*).
    pub fn traverse_upstream(&self, output_socket_name: &str) -> ShaderGraphEdgeIterator<'_> {
        ShaderGraphEdgeIterator::new(self, output_socket_name)
    }

    // ----- Variable name assignment (ref: ShaderGraph::setVariableNames) -----

    /// Assign unique variable names to all ports using the identifier map.
    /// Matches C++ ShaderGraph::setVariableNames(GenContext& context).
    /// The Syntax's get_variable_name() guarantees uniqueness via the identifiers map.
    pub fn set_variable_names(&mut self, syntax: &super::syntax::Syntax) {
        // Input sockets (ShaderOutput ports on root node)
        let in_names: Vec<String> = self.node.output_order.clone();
        for name in in_names {
            if let Some(sock) = self.node.outputs.get(&name) {
                let type_ref = sock.get_type().clone();
                let var = syntax.get_variable_name(&name, &type_ref, &mut self.identifiers);
                if let Some(sock) = self.node.outputs.get_mut(&name) {
                    sock.port_mut().set_variable(&var);
                }
            }
        }
        // Output sockets (ShaderInput ports on root node)
        let out_names: Vec<String> = self.node.input_order.clone();
        for name in out_names {
            if let Some(sock) = self.node.inputs.get(&name) {
                let type_ref = sock.get_type().clone();
                let var = syntax.get_variable_name(&name, &type_ref, &mut self.identifiers);
                if let Some(sock) = self.node.inputs.get_mut(&name) {
                    sock.port_mut().set_variable(&var);
                }
            }
        }
        // All child nodes
        let node_names: Vec<String> = self.node_order.clone();
        for node_name in node_names {
            // Collect (port_name, full_name, type) for inputs and outputs first to avoid borrow conflict
            let Some(node) = self.nodes.get(&node_name) else {
                continue;
            };
            let inp_info: Vec<(String, String, TypeDesc)> = node
                .input_order
                .iter()
                .filter_map(|n| {
                    node.inputs
                        .get(n)
                        .map(|i| (n.clone(), node.get_port_full_name(n), i.get_type().clone()))
                })
                .collect();
            let out_info: Vec<(String, String, TypeDesc)> = node
                .output_order
                .iter()
                .filter_map(|n| {
                    node.outputs
                        .get(n)
                        .map(|o| (n.clone(), node.get_port_full_name(n), o.get_type().clone()))
                })
                .collect();

            // Assign variable names for inputs
            for (port_name, full_name, type_ref) in inp_info {
                let var = syntax.get_variable_name(&full_name, &type_ref, &mut self.identifiers);
                if let Some(node) = self.nodes.get_mut(&node_name) {
                    if let Some(inp) = node.inputs.get_mut(&port_name) {
                        inp.port_mut().set_variable(&var);
                    }
                }
            }
            // Assign variable names for outputs
            for (port_name, full_name, type_ref) in out_info {
                let var = syntax.get_variable_name(&full_name, &type_ref, &mut self.identifiers);
                if let Some(node) = self.nodes.get_mut(&node_name) {
                    if let Some(out) = node.outputs.get_mut(&port_name) {
                        out.port_mut().set_variable(&var);
                    }
                }
            }
        }
    }
} // impl ShaderGraph

// ---------------------------------------------------------------------------
// ShaderGraphEdge
// ---------------------------------------------------------------------------

/// A directed edge in the shader graph: upstream output -> downstream input.
/// Matches C++ ShaderGraphEdge.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ShaderGraphEdge {
    /// Name of upstream node.
    pub upstream_node: String,
    /// Name of upstream output port.
    pub upstream_output: String,
    /// Name of downstream node.
    pub downstream_node: String,
    /// Name of downstream input port.
    pub downstream_input: String,
}

impl ShaderGraphEdge {
    pub fn new(
        upstream_node: impl Into<String>,
        upstream_output: impl Into<String>,
        downstream_node: impl Into<String>,
        downstream_input: impl Into<String>,
    ) -> Self {
        Self {
            upstream_node: upstream_node.into(),
            upstream_output: upstream_output.into(),
            downstream_node: downstream_node.into(),
            downstream_input: downstream_input.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// ShaderGraphEdgeIterator
// ---------------------------------------------------------------------------

/// DFS-based iterator over edges of a shader graph, traversing upstream.
/// Matches C++ ShaderGraphEdgeIterator.
///
/// Starts from an output socket of the graph and walks upstream via connections.
/// Each `next()` yields a `ShaderGraphEdge` (upstream -> downstream).
/// Detects cycles and skips graph boundary (nodes whose connections point to the graph root).
pub struct ShaderGraphEdgeIterator<'g> {
    graph: &'g ShaderGraph,
    /// Current upstream (node_name, output_name), None means iteration ended.
    upstream: Option<(String, String)>,
    /// Current downstream input, None at start.
    downstream: Option<(String, String)>,
    /// Stack frame: (upstream_node, upstream_output, next_input_index)
    stack: Vec<(String, String, usize)>,
    /// Current path for cycle detection (set of (node, output) pairs)
    path: HashSet<(String, String)>,
    /// Already visited edges to avoid duplicates
    visited: HashSet<(String, String, String, String)>,
}

impl<'g> ShaderGraphEdgeIterator<'g> {
    /// Create iterator starting from the named output socket.
    pub fn new(graph: &'g ShaderGraph, output_socket_name: &str) -> Self {
        // The output socket is a ShaderInput on the graph root node.
        // Its connection points to (upstream_node, upstream_output).
        let start_upstream = graph
            .get_output_socket(output_socket_name)
            .and_then(|sock| sock.get_connection())
            .filter(|(up_n, _)| *up_n != graph.get_name())
            .map(|(n, o)| (n.to_string(), o.to_string()));

        let mut iter = Self {
            graph,
            upstream: None,
            downstream: None,
            stack: Vec::new(),
            path: HashSet::new(),
            visited: HashSet::new(),
        };

        if let Some((up_n, up_o)) = start_upstream {
            // The downstream of the first edge is the graph output socket itself.
            let edge_key = (
                up_n.clone(),
                up_o.clone(),
                output_socket_name.to_string(),
                String::new(),
            );
            iter.visited.insert(edge_key);
            iter.path.insert((up_n.clone(), up_o.clone()));
            iter.upstream = Some((up_n, up_o));
            iter.downstream = Some((output_socket_name.to_string(), String::new()));
        }

        iter
    }

    /// Advance to next edge (DFS). Returns None when traversal is complete.
    fn advance(&mut self) -> Option<ShaderGraphEdge> {
        // Yield current edge then step into upstream inputs
        if let Some((ref up_n, ref up_o)) = self.upstream.clone() {
            let result = if let Some((ref dn, ref di)) = self.downstream.clone() {
                Some(ShaderGraphEdge::new(up_n, up_o, dn, di))
            } else {
                None
            };

            // Push current frame to stack and move into first input of upstream node
            let node = self.graph.nodes.get(up_n.as_str());
            let num_inputs = node.map(|n| n.num_inputs()).unwrap_or(0);

            if num_inputs > 0 {
                self.stack.push((up_n.clone(), up_o.clone(), 0));
                self.step_to_input(up_n.as_str(), 0);
            } else {
                // Leaf node: unwind
                self.path.remove(&(up_n.clone(), up_o.clone()));
                self.upstream = None;
                self.downstream = None;
            }

            return result;
        }

        // Unwind stack looking for unvisited siblings
        while let Some(mut frame) = self.stack.pop() {
            // Remove current frame node from path
            self.path.remove(&(frame.0.clone(), frame.1.clone()));

            let node_name = frame.0.clone();
            let node_out = frame.1.clone();
            let node = match self.graph.nodes.get(node_name.as_str()) {
                Some(n) => n,
                None => continue,
            };
            let num_inputs = node.num_inputs();

            // Try next siblings
            frame.2 += 1;
            while frame.2 < num_inputs {
                let stepped = self.step_to_input(&node_name, frame.2);
                if stepped {
                    // Re-push parent frame (with incremented index) and process new upstream
                    self.stack
                        .push((node_name.clone(), node_out.clone(), frame.2));
                    // Recurse via next call
                    return self.advance();
                }
                frame.2 += 1;
            }
        }

        None
    }

    /// Try to step into input `idx` of `node_name`. Returns true if a valid upstream was found.
    fn step_to_input(&mut self, node_name: &str, idx: usize) -> bool {
        let graph_name = self.graph.get_name();
        let node = match self.graph.nodes.get(node_name) {
            Some(n) => n,
            None => return false,
        };
        let inp = match node.get_input_at(idx) {
            Some(i) => i,
            None => return false,
        };
        let (up_n, up_o) = match inp.get_connection() {
            Some((n, o)) => (n.to_string(), o.to_string()),
            None => return false,
        };
        // Skip connections back to the graph itself
        if up_n == graph_name {
            return false;
        }
        let inp_name = inp.get_name().to_string();
        let edge_key = (
            up_n.clone(),
            up_o.clone(),
            node_name.to_string(),
            inp_name.clone(),
        );
        // Skip already visited edges
        if self.visited.contains(&edge_key) {
            return false;
        }
        // Cycle check
        if self.path.contains(&(up_n.clone(), up_o.clone())) {
            // Cycle detected — skip (C++ throws, we skip to be safe in iterator context)
            return false;
        }
        self.visited.insert(edge_key);
        self.path.insert((up_n.clone(), up_o.clone()));
        self.upstream = Some((up_n, up_o));
        self.downstream = Some((node_name.to_string(), inp_name));
        true
    }
}

impl<'g> Iterator for ShaderGraphEdgeIterator<'g> {
    type Item = ShaderGraphEdge;

    fn next(&mut self) -> Option<Self::Item> {
        // When upstream is set we have an edge to yield; otherwise keep unwinding
        if self.upstream.is_some() || !self.stack.is_empty() {
            self.advance()
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gen_shader::type_desc::types;

    fn make_graph() -> ShaderGraph {
        let mut g = ShaderGraph::new("test_graph");
        // output socket "out" (ShaderInput on root)
        g.add_output_socket("out", types::color3());
        // three child nodes in a chain: nodeA -> nodeB -> output
        let mut a = ShaderNode::new("nodeA");
        a.add_output("out", types::color3());
        g.add_node(a);

        let mut b = ShaderNode::new("nodeB");
        b.add_input("in", types::color3());
        b.add_output("out", types::color3());
        g.add_node(b);

        // nodeA.out -> nodeB.in
        g.make_connection("nodeB", "in", "nodeA", "out").unwrap();
        // nodeB.out -> graph output socket "out"
        g.make_connection("test_graph", "out", "nodeB", "out")
            .unwrap();
        g
    }

    #[test]
    fn test_edge_basic() {
        let e = ShaderGraphEdge::new("A", "out", "B", "in");
        assert_eq!(e.upstream_node, "A");
        assert_eq!(e.downstream_node, "B");
    }

    #[test]
    fn test_traverse_upstream_chain() {
        let g = make_graph();
        let edges: Vec<ShaderGraphEdge> = g.traverse_upstream("out").collect();
        // Expect two edges: nodeB->graph_out, nodeA->nodeB
        assert_eq!(edges.len(), 2, "Expected 2 edges, got {:?}", edges);
        // First edge: nodeB.out upstream of graph output socket
        assert_eq!(edges[0].upstream_node, "nodeB");
        // Second: nodeA upstream of nodeB
        assert_eq!(edges[1].upstream_node, "nodeA");
    }

    #[test]
    fn test_traverse_empty_output() {
        let g = ShaderGraph::new("empty");
        // No output socket at all
        let edges: Vec<_> = g.traverse_upstream("nonexistent").collect();
        assert!(edges.is_empty());
    }

    #[test]
    fn test_set_variable_names() {
        use crate::gen_shader::{Syntax, TypeSystem};
        let mut g = make_graph();
        let syntax = Syntax::new(TypeSystem::new());
        g.set_variable_names(&syntax);
        // Output socket variable should be set (non-empty)
        let sock = g.get_output_socket("out").unwrap();
        assert!(!sock.port.get_variable().is_empty());
    }
}
