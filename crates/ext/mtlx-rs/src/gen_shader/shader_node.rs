//! ShaderNode, ShaderPort — nodes and ports for shader generation DAG.

use std::collections::HashMap;

use crate::core::Document;
use crate::core::Value;

use super::shader_metadata_registry::ShaderPortMetadata;
use super::{BaseType, Semantic, ShaderGraphCreateContext, TypeDesc};

/// Flags for shader ports
#[allow(non_snake_case)]
pub mod ShaderPortFlag {
    pub const UNIFORM: u32 = 1 << 0;
    pub const EMITTED: u32 = 1 << 1;
    pub const BIND_INPUT: u32 = 1 << 2;
    pub const AUTHORED_VALUE: u32 = 1 << 3;
}

/// Shader port — input or output on a ShaderNode
#[derive(Clone, Debug)]
pub struct ShaderPort {
    pub type_desc: TypeDesc,
    pub name: String,
    pub path: String,
    pub variable: String,
    pub semantic: String,
    pub value: Option<Value>,
    pub unit: String,
    pub colorspace: String,
    pub geomprop: String,
    pub flags: u32,
    /// Metadata for emit (label, page, min, max, etc.)
    pub metadata: Vec<ShaderPortMetadata>,
}

impl ShaderPort {
    pub fn new(type_desc: TypeDesc, name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            type_desc,
            name: name.clone(),
            path: String::new(),
            variable: name,
            semantic: String::new(),
            value: None,
            unit: String::new(),
            colorspace: String::new(),
            geomprop: String::new(),
            flags: 0,
            metadata: Vec::new(),
        }
    }

    /// Get metadata for emit (по рефу getMetadata).
    pub fn get_metadata(&self) -> &[ShaderPortMetadata] {
        &self.metadata
    }

    /// Add metadata entry.
    pub fn add_metadata(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.metadata.push(ShaderPortMetadata {
            name: name.into(),
            value: value.into(),
        });
    }

    pub fn get_type(&self) -> &TypeDesc {
        &self.type_desc
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_variable(&self) -> &str {
        &self.variable
    }

    pub fn set_variable(&mut self, v: impl Into<String>) {
        self.variable = v.into();
    }

    pub fn get_value(&self) -> Option<&Value> {
        self.value.as_ref()
    }

    pub fn set_value(&mut self, value: Option<Value>, authored: bool) {
        self.value = value;
        self.set_flag(ShaderPortFlag::AUTHORED_VALUE, authored);
    }

    pub fn get_value_string(&self) -> String {
        self.value
            .as_ref()
            .map(|v| v.get_value_string())
            .unwrap_or_default()
    }

    pub fn set_flag(&mut self, flag: u32, value: bool) {
        if value {
            self.flags |= flag;
        } else {
            self.flags &= !flag;
        }
    }

    pub fn get_flag(&self, flag: u32) -> bool {
        (self.flags & flag) != 0
    }

    pub fn is_uniform(&self) -> bool {
        self.get_flag(ShaderPortFlag::UNIFORM)
    }

    pub fn set_uniform(&mut self, v: bool) {
        self.set_flag(ShaderPortFlag::UNIFORM, v);
    }

    pub fn is_emitted(&self) -> bool {
        self.get_flag(ShaderPortFlag::EMITTED)
    }

    pub fn set_emitted(&mut self, v: bool) {
        self.set_flag(ShaderPortFlag::EMITTED, v);
    }

    pub fn set_path(&mut self, p: impl Into<String>) {
        self.path = p.into();
    }
    pub fn set_unit(&mut self, u: impl Into<String>) {
        self.unit = u.into();
    }
    pub fn set_color_space(&mut self, cs: impl Into<String>) {
        self.colorspace = cs.into();
    }

    /// Return the path to this port (C++ getPath).
    pub fn get_path(&self) -> &str {
        &self.path
    }

    /// Return the unit type (C++ getUnit).
    pub fn get_unit(&self) -> &str {
        &self.unit
    }

    /// Return the source color space (C++ getColorSpace).
    pub fn get_color_space(&self) -> &str {
        &self.colorspace
    }

    /// Set geomprop name (C++ setGeomProp).
    pub fn set_geom_prop(&mut self, gp: impl Into<String>) {
        self.geomprop = gp.into();
    }

    /// Get geomprop name (C++ getGeomProp).
    pub fn get_geom_prop(&self) -> &str {
        &self.geomprop
    }

    /// Set the data type (C++ setType).
    pub fn set_type(&mut self, t: TypeDesc) {
        self.type_desc = t;
    }

    /// Set the name (C++ setName).
    pub fn set_name(&mut self, n: impl Into<String>) {
        self.name = n.into();
    }

    /// Return the variable semantic (C++ getSemantic).
    pub fn get_semantic(&self) -> &str {
        &self.semantic
    }

    /// Set the variable semantic (C++ setSemantic).
    pub fn set_semantic(&mut self, s: impl Into<String>) {
        self.semantic = s.into();
    }

    /// Return "nodeName_portName" (C++ getFullName).
    /// Since ShaderPort doesn't store a back-ref to its node, we use the port name only.
    /// For the full name with node prefix, use ShaderNode::get_port_full_name.
    pub fn get_full_name(&self) -> String {
        self.name.clone()
    }
}

/// Shader input — input port (connection to upstream output stored as node+output name)
#[derive(Clone, Debug)]
pub struct ShaderInput {
    pub port: ShaderPort,
    /// Connection to upstream: (node_name, output_name)
    pub connection: Option<(String, String)>,
}

impl ShaderInput {
    pub fn new(type_desc: TypeDesc, name: impl Into<String>) -> Self {
        Self {
            port: ShaderPort::new(type_desc, name),
            connection: None,
        }
    }

    pub fn get_type(&self) -> &TypeDesc {
        self.port.get_type()
    }

    pub fn get_name(&self) -> &str {
        self.port.get_name()
    }

    pub fn port(&self) -> &ShaderPort {
        &self.port
    }

    pub fn port_mut(&mut self) -> &mut ShaderPort {
        &mut self.port
    }

    /// Connect to upstream output by node and output name.
    /// Caller must update graph's downstream index separately via ShaderGraph::make_connection.
    pub fn make_connection(
        &mut self,
        node_name: impl Into<String>,
        output_name: impl Into<String>,
    ) {
        self.connection = Some((node_name.into(), output_name.into()));
    }

    /// Break connection. Caller must update graph's downstream index via ShaderGraph::break_connection.
    pub fn break_connection(&mut self) {
        self.connection = None;
    }

    /// Get connection if any: (node_name, output_name).
    pub fn get_connection(&self) -> Option<(&str, &str)> {
        self.connection
            .as_ref()
            .map(|(a, b)| (a.as_str(), b.as_str()))
    }

    pub fn has_connection(&self) -> bool {
        self.connection.is_some()
    }

    /// Return the sibling node name connected upstream, or None if not connected
    /// or if the connected node is not a sibling (C++ ShaderInput::getConnectedSibling).
    /// In our model: if the connection node name matches a node in the same parent graph,
    /// the caller should verify by looking up the graph. We return the node name only.
    pub fn get_connected_sibling_name(&self) -> Option<&str> {
        self.connection
            .as_ref()
            .map(|(node_name, _)| node_name.as_str())
    }
}

/// Shader output — output port (downstream connections stored externally in graph)
#[derive(Clone, Debug)]
pub struct ShaderOutput {
    pub port: ShaderPort,
}

impl ShaderOutput {
    pub fn new(type_desc: TypeDesc, name: impl Into<String>) -> Self {
        Self {
            port: ShaderPort::new(type_desc, name),
        }
    }

    pub fn get_type(&self) -> &TypeDesc {
        self.port.get_type()
    }

    pub fn get_name(&self) -> &str {
        self.port.get_name()
    }

    pub fn port(&self) -> &ShaderPort {
        &self.port
    }

    pub fn port_mut(&mut self) -> &mut ShaderPort {
        &mut self.port
    }
}

/// Classification flags for ShaderNode (matches C++ ShaderNode::Classification exactly).
/// Values must match C++ — DOT is 1<<20, not 1<<7.
#[allow(non_snake_case)]
pub mod ShaderNodeClassification {
    pub const TEXTURE: u32 = 1 << 0; // outputs floats, colors, vectors
    pub const CLOSURE: u32 = 1 << 1; // light integration
    pub const SHADER: u32 = 1 << 2; // outputs a shader
    pub const MATERIAL: u32 = 1 << 3; // outputs a material
    pub const FILETEXTURE: u32 = 1 << 4; // file texture node
    pub const CONDITIONAL: u32 = 1 << 5; // conditional node
    pub const CONSTANT: u32 = 1 << 6; // constant node
    pub const BSDF: u32 = 1 << 7; // BSDF node
    pub const BSDF_R: u32 = 1 << 8; // reflection BSDF
    pub const BSDF_T: u32 = 1 << 9; // transmission BSDF
    pub const EDF: u32 = 1 << 10; // EDF node
    pub const VDF: u32 = 1 << 11; // VDF node
    pub const LAYER: u32 = 1 << 12; // vertical layering of closures
    pub const SURFACE: u32 = 1 << 13; // surface shader
    pub const VOLUME: u32 = 1 << 14; // volume shader
    pub const LIGHT: u32 = 1 << 15; // light shader
    pub const UNLIT: u32 = 1 << 16; // unlit surface shader
    pub const SAMPLE2D: u32 = 1 << 17; // can be sampled in 2D (uv)
    pub const SAMPLE3D: u32 = 1 << 18; // can be sampled in 3D (position)
    pub const GEOMETRIC: u32 = 1 << 19; // geometric input
    pub const DOT: u32 = 1 << 20; // dot/passthrough node
}

/// Shader node in the generation DAG
#[derive(Debug)]
pub struct ShaderNode {
    pub name: String,
    pub classification: u32,
    pub inputs: HashMap<String, ShaderInput>,
    pub input_order: Vec<String>,
    pub outputs: HashMap<String, ShaderOutput>,
    pub output_order: Vec<String>,
    /// Cached implementation element name (C++ ShaderNode::_impl / ShaderNodeImplPtr).
    /// Populated during graph creation; used by emit code to dispatch to the right ShaderNodeImpl.
    pub impl_name: Option<String>,
    /// Parent graph name (C++ ShaderNode::_parent — stored as name to avoid Rc cycles).
    pub parent_name: Option<String>,
    /// Node-level metadata (C++ ShaderNode::_metadata).
    pub metadata: Vec<ShaderPortMetadata>,
}

impl ShaderNode {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            classification: 0,
            inputs: HashMap::new(),
            input_order: Vec::new(),
            outputs: HashMap::new(),
            output_order: Vec::new(),
            impl_name: None,
            parent_name: None,
            metadata: Vec::new(),
        }
    }

    /// Create a minimal ShaderNode with just a name and output type.
    /// Used for light sampling nodes (numActiveLightSources, sampleLightSource)
    /// that are created synthetically (ref: C++ ShaderNode::create in constructor).
    pub fn new_minimal(name: impl Into<String>, output_type: &str) -> Self {
        let mut node = Self::new(name);
        // Add a default output so emit_function_definition can find it
        let out_name = "out".to_string();
        let td = TypeDesc::new(output_type, BaseType::Float, Semantic::None, 1);
        node.outputs
            .insert(out_name.clone(), ShaderOutput::new(td, "out"));
        node.output_order.push(out_name);
        node
    }

    /// Set the parent graph name (C++ ShaderNode::_parent).
    pub fn set_parent_name(&mut self, name: impl Into<String>) {
        self.parent_name = Some(name.into());
    }

    /// Get the parent graph name, if set.
    pub fn get_parent_name(&self) -> Option<&str> {
        self.parent_name.as_deref()
    }

    /// Set node-level metadata (C++ ShaderNode::setMetadata).
    pub fn set_metadata_vec(&mut self, m: Vec<ShaderPortMetadata>) {
        self.metadata = m;
    }

    /// Get node-level metadata (C++ ShaderNode::getMetadata).
    pub fn get_node_metadata(&self) -> &[ShaderPortMetadata] {
        &self.metadata
    }

    /// Append a node-level metadata entry.
    pub fn add_node_metadata(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.metadata.push(ShaderPortMetadata {
            name: name.into(),
            value: value.into(),
        });
    }

    /// Return the cached implementation element name, if set.
    pub fn get_impl_name(&self) -> Option<&str> {
        self.impl_name.as_deref()
    }

    /// Cache the implementation element name for this node.
    /// Called during graph creation when the NodeDef implementation is resolved.
    pub fn set_impl_name(&mut self, name: impl Into<String>) {
        self.impl_name = Some(name.into());
    }

    /// Clear the cached implementation name (e.g. when the node is reconfigured).
    pub fn clear_impl_name(&mut self) {
        self.impl_name = None;
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn set_classification(&mut self, c: u32) {
        self.classification = c;
    }

    pub fn add_classification(&mut self, c: u32) {
        self.classification |= c;
    }

    pub fn has_classification(&self, c: u32) -> bool {
        (self.classification & c) == c
    }

    pub fn add_input(&mut self, name: impl Into<String>, type_desc: TypeDesc) -> &mut ShaderInput {
        let name = name.into();
        if !self.input_order.contains(&name) {
            self.input_order.push(name.clone());
        }
        self.inputs
            .entry(name.clone())
            .or_insert_with(|| ShaderInput::new(type_desc, name))
    }

    pub fn add_output(
        &mut self,
        name: impl Into<String>,
        type_desc: TypeDesc,
    ) -> &mut ShaderOutput {
        let name = name.into();
        if !self.output_order.contains(&name) {
            self.output_order.push(name.clone());
        }
        self.outputs
            .entry(name.clone())
            .or_insert_with(|| ShaderOutput::new(type_desc, name))
    }

    pub fn get_input(&self, name: &str) -> Option<&ShaderInput> {
        self.inputs.get(name)
    }

    pub fn get_input_mut(&mut self, name: &str) -> Option<&mut ShaderInput> {
        self.inputs.get_mut(name)
    }

    pub fn get_output(&self, name: &str) -> Option<&ShaderOutput> {
        self.outputs.get(name)
    }

    /// Get input by index (по рефу getInput(index)).
    pub fn get_input_at(&self, index: usize) -> Option<&ShaderInput> {
        self.input_order.get(index).and_then(|n| self.inputs.get(n))
    }

    /// Get output by index (по рефу getOutput(index)).
    pub fn get_output_at(&self, index: usize) -> Option<&ShaderOutput> {
        self.output_order
            .get(index)
            .and_then(|n| self.outputs.get(n))
    }

    pub fn num_inputs(&self) -> usize {
        self.input_order.len()
    }

    pub fn num_outputs(&self) -> usize {
        self.output_order.len()
    }

    pub fn get_inputs(&self) -> impl Iterator<Item = &ShaderInput> {
        self.input_order.iter().filter_map(|n| self.inputs.get(n))
    }

    pub fn get_outputs(&self) -> impl Iterator<Item = &ShaderOutput> {
        self.output_order.iter().filter_map(|n| self.outputs.get(n))
    }

    /// Return true if this node is a graph (C++ isAGraph). Overridden by ShaderGraph.
    pub fn is_a_graph(&self) -> bool {
        false
    }

    /// Set the name of this node (C++ ShaderNode has no setName but it exists conceptually).
    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }

    /// Get mutable reference to output by name.
    pub fn get_output_mut(&mut self, name: &str) -> Option<&mut ShaderOutput> {
        self.outputs.get_mut(name)
    }

    /// Get mutable references to all inputs in order.
    /// Returns inputs sorted by input_order without unsafe code.
    pub fn get_inputs_mut(&mut self) -> Vec<&mut ShaderInput> {
        // Collect the ordered names first to avoid borrow conflict between
        // self.input_order and self.inputs.get_mut.
        let order: Vec<String> = self.input_order.clone();
        // iter_mut over HashMap gives unordered refs; we sort them by order index.
        let mut indexed: Vec<(usize, &mut ShaderInput)> = self
            .inputs
            .iter_mut()
            .filter_map(|(k, v)| order.iter().position(|n| n == k).map(|idx| (idx, v)))
            .collect();
        indexed.sort_by_key(|(idx, _)| *idx);
        indexed.into_iter().map(|(_, v)| v).collect()
    }

    /// Get the full name of a port belonging to this node: "nodeName_portName".
    pub fn get_port_full_name(&self, port_name: &str) -> String {
        format!("{}_{}", self.name, port_name)
    }

    /// Returns true if an input is editable by users (C++ ShaderNode::isEditable).
    /// Editable inputs may be published as shader uniforms.
    pub fn is_editable(
        &self,
        input_name: &str,
        node_def_name: Option<&str>,
        doc: &Document,
        context: &dyn ShaderGraphCreateContext,
    ) -> bool {
        let Some(node_def_name) = node_def_name else {
            return true;
        };
        let Some(impl_) = context.get_implementation_for_nodedef(
            doc,
            node_def_name,
            context.get_implementation_target(),
        ) else {
            return true;
        };
        impl_.is_editable(input_name)
    }

    /// Initialize this ShaderNode from a MaterialX Node element and its NodeDef.
    /// Copies input values, sets element paths for uniform emit.
    ///
    /// This is the Rust equivalent of C++ `ShaderNode::initialize(const Node&, const NodeDef&, GenContext&)`.
    ///
    /// `node_elem`    - The instance node (has input values/connections).
    /// `nodedef_elem` - The NodeDef for the node (has port declarations).
    /// `type_system`  - Used to look up TypeDesc by name.
    pub fn initialize(
        &mut self,
        node_elem: &crate::core::ElementPtr,
        nodedef_elem: &crate::core::ElementPtr,
        _type_system: &super::TypeSystem,
    ) {
        use crate::core::Value;
        use crate::core::get_active_inputs;
        use crate::core::get_active_value_elements;
        use crate::core::get_interface_input;

        // --- Step 1: copy input values from the node instance ---
        // For each active input on the node, find the matching ShaderInput and set its value.
        for node_inp in get_active_inputs(node_elem) {
            let inp_name = node_inp.borrow().get_name().to_string();
            // Find the matching nodedef input to get enum info.
            let nd_val_elem = {
                let nd_borrow = nodedef_elem.borrow();
                nd_borrow.get_child(&inp_name)
            };
            if self.inputs.contains_key(&inp_name) {
                // Resolve value: check direct value, then interface input.
                let value_str: String = {
                    let has_val = node_inp.borrow().has_value_string();
                    if has_val {
                        node_inp.borrow().get_value_string()
                    } else {
                        // Walk interface input chain.
                        if let Some(iface_inp) = get_interface_input(&node_inp) {
                            iface_inp.borrow().get_value_string()
                        } else {
                            String::new()
                        }
                    }
                };

                if !value_str.is_empty() {
                    // Get type from nodedef or from the ShaderInput already created.
                    let type_name: String = nd_val_elem
                        .as_ref()
                        .and_then(|e| e.borrow().get_type().map(|s| s.to_string()))
                        .or_else(|| {
                            self.inputs
                                .get(&inp_name)
                                .map(|i| i.get_type().name.clone())
                        })
                        .unwrap_or_default();

                    if let Some(val) = Value::from_strings(&value_str, &type_name) {
                        if let Some(inp) = self.inputs.get_mut(&inp_name) {
                            inp.port.set_value(Some(val), true);
                        }
                    }
                }
            }
        }

        // --- Step 2: set element paths for inputs from node's value elements ---
        // Gives each ShaderInput its path so codegen can track origin.
        let node_path = node_elem.borrow().get_name_path(None);
        for node_val in get_active_value_elements(node_elem) {
            let val_name = node_val.borrow().get_name().to_string();
            let cat = node_val.borrow().get_category().to_string();
            if cat != crate::core::element::category::INPUT {
                continue;
            }
            if let Some(inp) = self.inputs.get_mut(&val_name) {
                // Resolve path: walk interface input chain if available.
                let path: String = {
                    let iface_inp = get_interface_input(&node_val);
                    if let Some(iface) = iface_inp {
                        iface.borrow().get_name_path(None)
                    } else {
                        node_val.borrow().get_name_path(None)
                    }
                };
                inp.port.set_path(path);
            }
        }

        // --- Step 3: set paths for nodedef inputs that have no path yet ---
        // These are inputs declared on the nodedef but not on the node instance.
        let sep = crate::core::NAME_PATH_SEPARATOR;
        for nd_inp in get_active_inputs(nodedef_elem) {
            let nd_inp_name = nd_inp.borrow().get_name().to_string();
            if let Some(inp) = self.inputs.get_mut(&nd_inp_name) {
                if inp.port.get_path().is_empty() {
                    let path = format!("{}{}{}", node_path, sep, nd_inp_name);
                    inp.port.set_path(path);
                }
            }
        }

        // --- Step 4: fill in unit/colorspace from node inputs ---
        for node_inp in get_active_inputs(node_elem) {
            let inp_name = node_inp.borrow().get_name().to_string();
            if let Some(inp) = self.inputs.get_mut(&inp_name) {
                // Copy unit string if set.
                let unit_str: String = node_inp
                    .borrow()
                    .get_unit()
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                if !unit_str.is_empty() {
                    inp.port.set_unit(unit_str);
                }
                if node_inp.borrow().has_color_space() {
                    let cs = node_inp.borrow().get_color_space();
                    inp.port.set_color_space(cs);
                }
                // Set geomprop from defaultgeomprop if present.
                let gp: String = node_inp
                    .borrow()
                    .get_attribute(crate::core::element::DEFAULT_GEOM_PROP_ATTRIBUTE)
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                if !gp.is_empty() {
                    inp.port.set_geom_prop(gp);
                }
            }
        }
    }

    /// Populate node-level metadata from nodedef attributes (C++ ShaderNode::createMetadata).
    ///
    /// For each attribute in the nodedef that is registered in the metadata registry,
    /// add a ShaderPortMetadata entry on this node. Then do the same for each input.
    pub fn create_metadata(
        &mut self,
        nodedef_elem: &crate::core::ElementPtr,
        registry: &super::ShaderMetadataRegistry,
    ) {
        if !registry.has_metadata() {
            return;
        }

        // Node-level metadata from nodedef attributes.
        let nd_borrow = nodedef_elem.borrow();
        for (attr_name, attr_val) in nd_borrow.iter_attributes() {
            if attr_val.is_empty() {
                continue;
            }
            if let Some(entry) = registry.find_metadata(attr_name) {
                self.metadata.push(ShaderPortMetadata {
                    name: entry.name.clone(),
                    value: attr_val.to_string(),
                });
            }
        }
        drop(nd_borrow);

        // Per-input metadata from nodedef input attributes.
        let nd_inputs = crate::core::get_active_value_elements(nodedef_elem);
        for nd_port in nd_inputs {
            let port_cat = nd_port.borrow().get_category().to_string();
            if port_cat != crate::core::element::category::INPUT {
                continue;
            }
            let port_name = nd_port.borrow().get_name().to_string();
            let attrs: Vec<(String, String)> = nd_port
                .borrow()
                .iter_attributes()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            if let Some(inp) = self.inputs.get_mut(&port_name) {
                for (attr_name, attr_val) in &attrs {
                    if attr_val.is_empty() {
                        continue;
                    }
                    if let Some(entry) = registry.find_metadata(attr_name) {
                        inp.port.metadata.push(ShaderPortMetadata {
                            name: entry.name.clone(),
                            value: attr_val.clone(),
                        });
                    }
                }
            }
        }
    }

    /// Create a ShaderNode from a NodeDef element and type system.
    /// Populates inputs/outputs/classification.
    /// C++ ShaderNode::create(parent, name, NodeDef&, GenContext&).
    pub fn create_from_nodedef(
        parent_name: Option<&str>,
        name: &str,
        nodedef_elem: &crate::core::ElementPtr,
        context: &dyn ShaderGraphCreateContext,
    ) -> Self {
        use super::shader_node_category as cat_str;
        use crate::core::element::category;
        use crate::core::get_active_value_elements;

        let type_system = context.get_type_system();
        let mut node = ShaderNode::new(name);
        if let Some(p) = parent_name {
            node.set_parent_name(p);
        }

        // Populate inputs/outputs from nodedef value elements.
        for port in get_active_value_elements(nodedef_elem) {
            let port_name = port.borrow().get_name().to_string();
            let port_cat = port.borrow().get_category().to_string();
            let type_str = port
                .borrow()
                .get_type()
                .map(|s| s.to_string())
                .unwrap_or_default();
            let td = type_system.get_type(&type_str);

            if port_cat == category::OUTPUT {
                node.add_output(&port_name, td);
            } else if port_cat == category::INPUT {
                let value_str = port.borrow().get_value_string();
                if !value_str.is_empty() {
                    let enum_names = port
                        .borrow()
                        .get_attribute(crate::core::element::ENUM_ATTRIBUTE)
                        .unwrap_or("")
                        .to_string();
                    if let Some((remap_type, remap_val)) =
                        context
                            .get_syntax()
                            .remap_enumeration(&value_str, &td, &enum_names)
                    {
                        let inp = node.add_input(&port_name, remap_type);
                        inp.port.set_value(Some(remap_val), false);
                    } else {
                        let inp = node.add_input(&port_name, td);
                        if let Some(val) = Value::from_strings(&value_str, &type_str) {
                            inp.port.set_value(Some(val), false);
                        }
                    }
                } else {
                    let inp = node.add_input(&port_name, td);
                    if port.borrow().get_is_uniform() {
                        inp.port.set_uniform(true);
                    }
                    continue;
                }
                if let Some(inp) = node.inputs.get_mut(&port_name) {
                    if port.borrow().get_is_uniform() {
                        inp.port.set_uniform(true);
                    }
                }
            }
        }

        // Add default output if none declared.
        if node.num_outputs() == 0 {
            let nd_type = nodedef_elem
                .borrow()
                .get_type()
                .map(|s| s.to_string())
                .unwrap_or_default();
            let td = type_system.get_type(&nd_type);
            node.add_output("out", td);
        }

        // --- Classification ---
        node.classification = ShaderNodeClassification::TEXTURE;

        let primary_out_type = node
            .get_output_at(0)
            .map(|o| o.get_type().name.clone())
            .unwrap_or_default();

        let nd_name = nodedef_elem
            .borrow()
            .get_attribute("name")
            .map(|s| s.to_string())
            .unwrap_or_default();
        let node_str = nodedef_elem
            .borrow()
            .get_attribute("node")
            .map(|s| s.to_string())
            .unwrap_or_default();
        let group_name = nodedef_elem
            .borrow()
            .get_attribute("nodegroup")
            .map(|s| s.to_string())
            .unwrap_or_default();
        let bsdf_attr = nodedef_elem
            .borrow()
            .get_attribute("bsdf")
            .map(|s| s.to_string())
            .unwrap_or_default();

        use ShaderNodeClassification as C;
        if primary_out_type == "material" {
            node.classification = C::MATERIAL;
        } else if primary_out_type == "surfaceshader" {
            if nd_name == "ND_surface_unlit" {
                node.classification = C::SHADER | C::SURFACE | C::UNLIT;
            } else {
                node.classification = C::SHADER | C::SURFACE | C::CLOSURE;
            }
        } else if primary_out_type == "volumeshader" {
            node.classification = C::SHADER | C::VOLUME | C::CLOSURE;
        } else if primary_out_type == "lightshader" {
            node.classification = C::LIGHT | C::SHADER | C::CLOSURE;
        } else if primary_out_type == "BSDF" {
            node.classification = C::BSDF | C::CLOSURE;
            if bsdf_attr == cat_str::BSDF_R {
                node.classification |= C::BSDF_R;
            } else if bsdf_attr == cat_str::BSDF_T {
                node.classification |= C::BSDF_T;
            } else {
                node.classification |= C::BSDF_R | C::BSDF_T;
            }
            if nd_name == "ND_layer_bsdf" || nd_name == "ND_layer_vdf" {
                node.classification |= C::LAYER;
            }
        } else if primary_out_type == "EDF" {
            node.classification = C::EDF | C::CLOSURE;
        } else if primary_out_type == "VDF" {
            node.classification = C::VDF | C::CLOSURE;
        } else if node_str == cat_str::CONSTANT {
            node.classification = C::TEXTURE | C::CONSTANT;
        } else if node_str == cat_str::DOT {
            node.classification = C::TEXTURE | C::DOT;
        } else if group_name == cat_str::TEXTURE2D_GROUPNAME
            || group_name == cat_str::TEXTURE3D_GROUPNAME
        {
            node.classification = C::TEXTURE | C::FILETEXTURE;
        }

        // Group-based extra classifications.
        if group_name == cat_str::TEXTURE2D_GROUPNAME
            || group_name == cat_str::PROCEDURAL2D_GROUPNAME
        {
            node.classification |= C::SAMPLE2D;
        } else if group_name == cat_str::TEXTURE3D_GROUPNAME
            || group_name == cat_str::PROCEDURAL3D_GROUPNAME
        {
            node.classification |= C::SAMPLE3D;
        } else if group_name == cat_str::GEOMETRIC_GROUPNAME {
            node.classification |= C::GEOMETRIC;
        }

        node
    }

    /// Create a ShaderNode from an implementation element name (C++ ShaderNode::create(impl, classification)).
    /// The impl_name is stored for dispatch; inputs/outputs are not populated here.
    pub fn create_from_impl(
        parent_name: Option<&str>,
        name: &str,
        impl_name: &str,
        classification: u32,
    ) -> Self {
        let mut node = ShaderNode::new(name);
        if let Some(p) = parent_name {
            node.set_parent_name(p);
        }
        node.impl_name = Some(impl_name.to_string());
        node.classification = classification;
        node
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Document;
    use crate::core::document::create_document;
    use crate::core::element::add_child_of_category;
    use crate::core::element::category;
    use crate::format::FilePath;
    use crate::gen_shader::{
        GenOptions, ShaderGraphCreateContext, ShaderImplContext, Syntax, TypeSystem,
    };

    struct TestCtx {
        type_system: TypeSystem,
        syntax: Syntax,
        options: GenOptions,
    }

    impl ShaderImplContext for TestCtx {
        fn resolve_source_file(
            &self,
            _filename: &str,
            _local_path: Option<&FilePath>,
        ) -> Option<FilePath> {
            None
        }

        fn get_type_system(&self) -> &TypeSystem {
            &self.type_system
        }
    }

    impl ShaderGraphCreateContext for TestCtx {
        fn get_syntax(&self) -> &Syntax {
            &self.syntax
        }

        fn get_options(&self) -> &GenOptions {
            &self.options
        }
    }

    fn make_test_context() -> TestCtx {
        TestCtx {
            syntax: Syntax::new(TypeSystem::new()),
            type_system: TypeSystem::new(),
            options: GenOptions::default(),
        }
    }

    fn make_nodedef(_ts: &TypeSystem) -> (Document, crate::core::ElementPtr) {
        let doc = create_document();
        let root = doc.get_root();
        let nd = add_child_of_category(&root, category::NODEDEF, "ND_add_float").unwrap();
        nd.borrow_mut().set_attribute("node", "add");
        nd.borrow_mut().set_attribute("type", "float");
        let inp1 = add_child_of_category(&nd, category::INPUT, "in1").unwrap();
        inp1.borrow_mut().set_attribute("type", "float");
        inp1.borrow_mut().set_value_string("0.0");
        let inp2 = add_child_of_category(&nd, category::INPUT, "in2").unwrap();
        inp2.borrow_mut().set_attribute("type", "float");
        inp2.borrow_mut().set_value_string("0.0");
        (doc, nd)
    }

    #[test]
    fn test_create_from_nodedef_basic() {
        let ctx = make_test_context();
        let (_doc, nd) = make_nodedef(ctx.get_type_system());
        let node = ShaderNode::create_from_nodedef(Some("graph1"), "add1", &nd, &ctx);
        assert_eq!(node.get_name(), "add1");
        assert_eq!(node.get_parent_name(), Some("graph1"));
        assert_eq!(node.num_inputs(), 2);
        // Default output auto-added since nodedef has type="float".
        assert!(node.num_outputs() >= 1);
        // Classification: float output -> TEXTURE.
        assert!(node.has_classification(ShaderNodeClassification::TEXTURE));
    }

    #[test]
    fn test_create_from_nodedef_surfaceshader() {
        let ctx = make_test_context();
        let doc = create_document();
        let root = doc.get_root();
        let nd = add_child_of_category(&root, category::NODEDEF, "ND_surface").unwrap();
        nd.borrow_mut().set_attribute("node", "surface");
        nd.borrow_mut().set_attribute("type", "surfaceshader");
        let out = add_child_of_category(&nd, category::OUTPUT, "out").unwrap();
        out.borrow_mut().set_attribute("type", "surfaceshader");

        let node = ShaderNode::create_from_nodedef(None, "surf1", &nd, &ctx);
        assert!(node.has_classification(ShaderNodeClassification::SURFACE));
        assert!(node.has_classification(ShaderNodeClassification::SHADER));
    }

    #[test]
    fn test_initialize_copies_values() {
        let ctx = make_test_context();
        let (_doc, nd) = make_nodedef(ctx.get_type_system());
        let mut sn = ShaderNode::create_from_nodedef(None, "add1", &nd, &ctx);

        // Build a node instance with different values.
        let doc2 = create_document();
        let root2 = doc2.get_root();
        let graph = add_child_of_category(&root2, category::NODE_GRAPH, "g1").unwrap();
        let node_inst = add_child_of_category(&graph, category::NODE, "myAdd").unwrap();
        node_inst.borrow_mut().set_attribute("node", "add");
        node_inst.borrow_mut().set_attribute("type", "float");
        let i1 = add_child_of_category(&node_inst, category::INPUT, "in1").unwrap();
        i1.borrow_mut().set_attribute("type", "float");
        i1.borrow_mut().set_value_string("1.5");

        sn.initialize(&node_inst, &nd, ctx.get_type_system());

        // in1 should now have value 1.5.
        let in1 = sn.get_input("in1").expect("in1 missing");
        let val = in1.port.get_value();
        assert!(val.is_some(), "value should be set after initialize");
        if let Some(Value::Float(f)) = val {
            assert!((f - 1.5).abs() < 1e-6);
        } else {
            panic!("expected Float value, got {:?}", val);
        }

        // in1 path should point inside graph.
        assert!(
            in1.port.get_path().contains("myAdd"),
            "path should contain node name"
        );
    }

    #[test]
    fn test_create_from_impl() {
        let node = ShaderNode::create_from_impl(
            Some("parentGraph"),
            "implNode",
            "some_impl_file",
            ShaderNodeClassification::TEXTURE,
        );
        assert_eq!(node.get_name(), "implNode");
        assert_eq!(node.get_impl_name(), Some("some_impl_file"));
        assert_eq!(node.get_parent_name(), Some("parentGraph"));
        assert!(node.has_classification(ShaderNodeClassification::TEXTURE));
    }

    #[test]
    fn test_metadata_add_get() {
        let mut node = ShaderNode::new("meta_test");
        node.add_node_metadata("uiname", "My Node");
        node.add_node_metadata("uifolder", "Misc");
        let md = node.get_node_metadata();
        assert_eq!(md.len(), 2);
        assert_eq!(md[0].name, "uiname");
        assert_eq!(md[0].value, "My Node");
    }

    #[test]
    fn test_is_editable() {
        let node = ShaderNode::new("edit_test");
        let doc = Document::new();
        let ctx = make_test_context();
        assert!(node.is_editable("any_input", None, &doc, &ctx));
    }

    #[test]
    fn test_connected_sibling_name() {
        let td = crate::gen_shader::type_desc_types::none();
        let mut inp = ShaderInput::new(td, "in1");
        assert!(inp.get_connected_sibling_name().is_none());
        inp.make_connection("node_a", "out");
        assert_eq!(inp.get_connected_sibling_name(), Some("node_a"));
        inp.break_connection();
        assert!(inp.get_connected_sibling_name().is_none());
    }
}

/// Category name constants for ShaderNode (C++ ShaderNode::CONSTANT, DOT, IMAGE, etc.).
#[allow(non_snake_case)]
pub mod shader_node_category {
    pub const CONSTANT: &str = "constant";
    pub const DOT: &str = "dot";
    pub const IMAGE: &str = "image";
    pub const SURFACESHADER: &str = "surfaceshader";
    pub const BACKSURFACESHADER: &str = "backsurfaceshader";
    pub const BSDF_R: &str = "R";
    pub const BSDF_T: &str = "T";
    pub const TEXTURE2D_GROUPNAME: &str = "texture2d";
    pub const TEXTURE3D_GROUPNAME: &str = "texture3d";
    pub const PROCEDURAL2D_GROUPNAME: &str = "procedural2d";
    pub const PROCEDURAL3D_GROUPNAME: &str = "procedural3d";
    pub const GEOMETRIC_GROUPNAME: &str = "geometric";
}
