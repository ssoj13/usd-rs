//! ShaderGraph::create — построение ShaderGraph из NodeGraph/Element (по рефу MaterialX ShaderGraph.cpp).

use crate::core::element::{INTERFACE_NAME_ATTRIBUTE, category};
const ENUM_ATTRIBUTE: &str = "enum";
use crate::core::Value;
use crate::core::{
    Document, ElementPtr, GEOM_PROP_ATTRIBUTE, get_active_color_space, get_active_input,
    get_active_inputs, get_active_outputs, get_connected_node, get_default_geom_prop_string,
    get_geom_prop, get_index, get_inputs, get_interface_input, get_node_def_string, get_space,
    has_interface_name, traverse_graph,
};

use super::color_management::{ColorManagementSystem, ColorSpaceTransform};
use super::gen_options::ShaderInterfaceType;
use super::unit_system::{UnitSystem, UnitTransform};

use super::gen_context::ShaderImplContext;
use super::gen_options::GenOptions;
use super::shader_graph::ShaderGraph;
use super::shader_node::{ShaderNode, ShaderPortFlag};
use super::shader_node_factory::create_node_from_nodedef;
use super::shader_node_impl::ShaderNodeImpl;
use super::syntax::Syntax;

/// Context required for ShaderGraph::create (по рефу GenContext + ShaderGenerator).
/// Provides syntax, options, type resolution, implementation lookup.
pub trait ShaderGraphCreateContext: ShaderImplContext {
    /// Syntax for make_valid_name, get_variable_name.
    fn get_syntax(&self) -> &Syntax;

    /// Generation options (add_upstream_dependencies, emit_color_transforms, etc.).
    fn get_options(&self) -> &GenOptions;

    /// Type desc for type name. Default: get_type_system().get_type(name).
    fn get_type_desc(&self, name: &str) -> super::TypeDesc {
        self.get_type_system().get_type(name)
    }

    /// Resolve NodeDef to ShaderNodeImpl. Returns None if no implementation for target.
    /// По рефу ShaderGenerator::getImplementation(NodeDef, context).
    fn get_implementation_for_nodedef(
        &self,
        _doc: &Document,
        _node_def_name: &str,
        _target: &str,
    ) -> Option<Box<dyn ShaderNodeImpl>> {
        None
    }

    /// Target string (e.g. "genglsl"). Used for generator identification.
    fn get_target(&self) -> &str {
        ""
    }

    /// Target for implementation lookup. When a target inherits another (e.g. essl inherits genglsl),
    /// return the base target to find implementations. Default: same as get_target().
    fn get_implementation_target(&self) -> &str {
        self.get_target()
    }

    /// Color management system for color transforms. Default: None.
    fn get_color_management_system(&self) -> Option<&dyn ColorManagementSystem> {
        None
    }

    /// Unit system for unit transforms. Default: None.
    fn get_unit_system(&self) -> Option<&dyn UnitSystem> {
        None
    }

    /// Shader metadata registry for populating port metadata from nodedef (uiname→label etc.).
    /// Returns None if not set (e.g. non-OSL targets).
    fn get_shader_metadata_registry(&self) -> Option<&super::ShaderMetadataRegistry> {
        None
    }

    /// LightData type discriminator variable name.
    /// C++: HwShaderGenerator::getLightDataTypevarString(). Default: "type".
    /// WGSL overrides to "light_type" because `type` is a reserved word.
    fn get_light_data_type_var_string(&self) -> &str {
        "type"
    }
}

/// Resolve a NodeGraph to its corresponding NodeDef.
/// C++ NodeGraph::getNodeDef() — first checks direct `nodedef` attribute,
/// then scans Implementation elements whose `nodegraph` attribute matches.
fn resolve_nodegraph_nodedef(
    node_graph: &ElementPtr,
    doc: &Document,
) -> Result<ElementPtr, String> {
    let ng_name = node_graph.borrow().get_name().to_string();

    // 1. Direct attribute (most nodegraphs)
    if let Some(nd_name) = get_node_def_string(node_graph) {
        return doc
            .get_node_def(&nd_name)
            .ok_or_else(|| format!("NodeDef '{}' not found", nd_name));
    }

    // 2. Fallback: scan Implementation elements for one that references this nodegraph
    //    (C++ Node.cpp:748 — "If not directly defined look for an implementation
    //     which has a nodedef association")
    for impl_elem in doc.get_implementations() {
        let impl_ng = impl_elem
            .borrow()
            .get_attribute("nodegraph")
            .map(|s| s.to_string())
            .unwrap_or_default();
        if impl_ng == ng_name {
            if let Some(nd_attr) = impl_elem
                .borrow()
                .get_attribute("nodedef")
                .map(|s| s.to_string())
            {
                if let Some(nd) = doc.get_node_def(&nd_attr) {
                    return Ok(nd);
                }
            }
        }
    }

    Err(format!(
        "NodeGraph '{}' has no nodedef (neither direct attribute nor via Implementation)",
        ng_name
    ))
}

/// Create ShaderGraph from NodeGraph (по рефу ShaderGraph::create(parent, NodeGraph, context) line 431).
///
/// 1. addInputSockets from NodeDef
/// 2. addOutputSockets from NodeGraph
/// 3. addUpstreamDependencies for each output
/// 4. finalize
pub fn create_from_nodegraph(
    node_graph: &ElementPtr,
    doc: &Document,
    context: &dyn ShaderGraphCreateContext,
) -> Result<ShaderGraph, String> {
    // C++ NodeGraph::getNodeDef() — first check direct `nodedef` attribute,
    // then fallback to scanning Implementation elements for a `nodegraph` match.
    let node_def = resolve_nodegraph_nodedef(node_graph, doc)?;

    let mut graph_name = node_graph.borrow().get_name().to_string();
    context.get_syntax().make_valid_name(&mut graph_name);
    let mut graph = ShaderGraph::new(&graph_name);
    graph.node.classification = 0;

    add_input_sockets_from_interface(&mut graph, &node_def, context);
    add_output_sockets_from_interface(&mut graph, node_graph, context);

    if context.get_options().add_upstream_dependencies {
        for output in get_active_outputs(node_graph) {
            add_upstream_dependencies(&mut graph, &output, doc, context);
        }
    }

    finalize(&mut graph, doc, context);

    Ok(graph)
}

/// Create ShaderGraph from Element — Output or Node (по рефу ShaderGraph::create(parent, name, element, context) line 464).
///
/// For Output: interface from NodeGraph/NodeDef, output socket, addUpstreamDependencies.
/// For Node: interface from NodeDef, create shader node, connect inputs/outputs, addUpstreamDependencies.
pub fn create_from_element(
    name: &str,
    element: &ElementPtr,
    doc: &Document,
    context: &dyn ShaderGraphCreateContext,
) -> Result<ShaderGraph, String> {
    let cat = element.borrow().get_category().to_string();

    if cat == category::OUTPUT {
        create_from_output(name, element, doc, context)
    } else if cat == category::NODE_GRAPH {
        Err(format!(
            "ShaderGraph::create does not support element category '{}'",
            cat
        ))
    } else {
        create_from_node(name, element, doc, context)
    }
}

fn create_from_output(
    name: &str,
    output: &ElementPtr,
    doc: &Document,
    context: &dyn ShaderGraphCreateContext,
) -> Result<ShaderGraph, String> {
    let output_parent = output.borrow().get_parent().ok_or("Output has no parent")?;
    let parent_cat = output_parent.borrow().get_category().to_string();

    let (interface, root): (ElementPtr, ElementPtr) = if parent_cat == category::NODE_GRAPH {
        let node_def = get_node_def_string(&output_parent)
            .and_then(|nd| doc.get_node_def(&nd))
            .ok_or("NodeGraph has no nodedef")?;
        (node_def, output.clone())
    } else if parent_cat == category::DOCUMENT {
        let connected = output
            .borrow()
            .get_node_name()
            .and_then(|nn| output_parent.borrow().get_child(nn));
        let Some(conn) = connected else {
            return Err("Free output has no connected node".to_string());
        };
        let cat = conn.borrow().get_category().to_string();
        if cat == category::NODE_GRAPH || cat == category::NODEDEF {
            (conn.clone(), output.clone())
        } else {
            return Err("Free output connected node is not interface".to_string());
        }
    } else {
        return Err("Output parent not NodeGraph or Document".to_string());
    };

    let mut graph_name = name.to_string();
    context.get_syntax().make_valid_name(&mut graph_name);
    let mut graph = ShaderGraph::new(&graph_name);
    graph.node.classification = 0;

    add_input_sockets_from_interface(&mut graph, &interface, context);
    let out_name = output.borrow().get_name().to_string();
    let out_ty = output
        .borrow()
        .get_type()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "float".to_string());
    let type_desc = context.get_type_desc(&out_ty);
    let socket = graph.add_output_socket(&out_name, type_desc);
    socket.port.path = output.borrow().get_name_path(None);
    if let Some(u) = output.borrow().get_attribute("unit") {
        socket.port.unit = u.to_string();
    }
    if let Some(cs) = output.borrow().get_attribute("colorspace") {
        socket.port.colorspace = cs.to_string();
    }

    if context.get_options().add_upstream_dependencies {
        add_upstream_dependencies(&mut graph, &root, doc, context);
    }

    finalize(&mut graph, doc, context);

    Ok(graph)
}

fn create_from_node(
    name: &str,
    node: &ElementPtr,
    doc: &Document,
    context: &dyn ShaderGraphCreateContext,
) -> Result<ShaderGraph, String> {
    let node_def = crate::core::get_node_def(node, context.get_target(), true)
        .ok_or_else(|| format!("Node '{}' has no nodedef", node.borrow().get_name()))?;
    let node_def_name = node_def.borrow().get_name().to_string();

    let mut graph_name = name.to_string();
    context.get_syntax().make_valid_name(&mut graph_name);
    let mut graph = ShaderGraph::new(&graph_name);
    graph.node.classification = 0;

    add_input_sockets_from_interface(&mut graph, &node_def, context);
    add_output_sockets_from_interface(&mut graph, &node_def, context);

    let node_name = node.borrow().get_name().to_string();
    let shader_node = create_node_from_node(node, &node_def, &graph, doc, context)?;
    let output_names: Vec<String> = shader_node.output_order.iter().cloned().collect();
    graph.add_node(shader_node);
    graph.set_node_def(&node_name, &node_def_name);

    let graph_name_str = graph.get_name().to_string();
    for (i, out_name) in output_names.iter().enumerate() {
        let socket_name = graph
            .get_output_socket_at(i)
            .map(|s| s.get_name().to_string())
            .unwrap_or_else(|| "out".to_string());
        let _ = graph.make_connection(&graph_name_str, &socket_name, &node_name, out_name);
    }

    for nodedef_inp in get_active_inputs(&node_def) {
        let inp_name = nodedef_inp.borrow().get_name().to_string();
        if graph.get_input_socket(&inp_name).is_some()
            && graph
                .get_node(&node_name)
                .and_then(|n| n.get_input(&inp_name))
                .is_some()
        {
            let _ = graph.make_connection(&node_name, &inp_name, &graph_name_str, &inp_name);
        }
    }

    if context.get_options().add_upstream_dependencies {
        add_upstream_dependencies(&mut graph, node, doc, context);
    }

    finalize(&mut graph, doc, context);

    Ok(graph)
}

/// Format value for OSL metadata (string→quoted, etc.)
fn format_metadata_value(type_name: &str, value: &str) -> String {
    match type_name {
        "string" | "filename" => {
            format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
        }
        "boolean" => match value.to_lowercase().as_str() {
            "true" | "1" => "1".to_string(),
            _ => "0".to_string(),
        },
        _ => value.to_string(),
    }
}

/// Add input sockets from InterfaceElement (NodeDef, NodeGraph) (по рефу addInputSockets).
fn add_input_sockets_from_interface(
    graph: &mut ShaderGraph,
    interface: &ElementPtr,
    context: &dyn ShaderGraphCreateContext,
) {
    let registry = context.get_shader_metadata_registry();
    let syntax = context.get_syntax();
    for input in get_active_inputs(interface) {
        let inp = input.borrow();
        let name = inp.get_name().to_string();
        let port_type = inp.get_type().unwrap_or("float");
        let mut type_desc = context.get_type_desc(port_type);
        let value_str = inp.get_value().unwrap_or("").to_string();
        let mut value_opt: Option<Value> = None;
        let enum_names = inp.get_attribute(ENUM_ATTRIBUTE).unwrap_or("").to_string();
        if !value_str.is_empty() {
            if let Some((remapped_type, remapped_value)) =
                syntax.remap_enumeration(&value_str, &type_desc, &enum_names)
            {
                type_desc = remapped_type;
                value_opt = Some(remapped_value);
            } else if let Some(v) = Value::from_strings(&value_str, port_type) {
                value_opt = Some(v);
            }
        }
        let socket = graph.add_input_socket(&name, type_desc);
        if let Some(v) = value_opt {
            socket.port_mut().value = Some(v);
            socket
                .port_mut()
                .set_flag(ShaderPortFlag::AUTHORED_VALUE, true);
        }
        if inp.has_attribute("uniform")
            && inp
                .get_attribute("uniform")
                .map(|s| s == "true")
                .unwrap_or(false)
        {
            socket.port.set_uniform(true);
        }
        if let Some(geom) = get_default_geom_prop_string(&input) {
            socket.port.geomprop = geom;
        }
        if let Some(reg) = registry {
            if reg.has_metadata() {
                for attr_name in inp.get_attribute_names() {
                    if let Some(entry) = reg.find_metadata(attr_name) {
                        if let Some(attr_value) = inp.get_attribute(attr_name) {
                            if !attr_value.is_empty() {
                                let emit_value =
                                    format_metadata_value(&entry.type_name, attr_value);
                                socket
                                    .port_mut()
                                    .add_metadata(entry.name.clone(), emit_value);
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Add output sockets from InterfaceElement (по рефу addOutputSockets).
fn add_output_sockets_from_interface(
    graph: &mut ShaderGraph,
    interface: &ElementPtr,
    context: &dyn ShaderGraphCreateContext,
) {
    for output in get_active_outputs(interface) {
        let out = output.borrow();
        let name = out.get_name().to_string();
        let ty = out.get_type().unwrap_or("float");
        graph.add_output_socket(&name, context.get_type_desc(ty));
    }
    if graph.num_output_sockets() == 0 {
        let ty = interface
            .borrow()
            .get_type()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "float".to_string());
        graph.add_output_socket("out", context.get_type_desc(&ty));
    }
}

/// Traverse from root and add upstream dependencies (по рефу addUpstreamDependencies).
fn add_upstream_dependencies(
    graph: &mut ShaderGraph,
    root: &ElementPtr,
    doc: &Document,
    context: &dyn ShaderGraphCreateContext,
) {
    let mut processed = std::collections::HashSet::new();

    traverse_graph(root, &mut |edge| {
        let (upstream, downstream) = match (edge.upstream.clone(), edge.downstream.clone()) {
            (Some(u), Some(d)) => (u, d),
            _ => return,
        };
        let connecting = edge.connecting.clone();

        if processed.contains(&downstream.borrow().get_name_path(None)) {
            return;
        }

        let upstream_elem = if upstream.borrow().get_category() == category::OUTPUT {
            processed.insert(upstream.borrow().get_name_path(None));
            let node_name_opt = upstream.borrow().get_node_name().map(|s| s.to_string());
            node_name_opt
                .and_then(|node_name| {
                    upstream
                        .borrow()
                        .get_parent()
                        .and_then(|parent| parent.borrow().get_child(&node_name))
                })
                .unwrap_or_else(|| upstream.clone())
        } else {
            upstream.clone()
        };

        create_connected_nodes(
            graph,
            &downstream,
            &upstream_elem,
            connecting.as_ref(),
            doc,
            context,
        );
    });
}

/// Create connected nodes for an edge (по рефу createConnectedNodes).
/// Upstream may have category "node" or a concrete node type (e.g. "image", "multiply")
/// — XML tags become categories, so we resolve NodeDef to determine if it's a graph node.
fn create_connected_nodes(
    graph: &mut ShaderGraph,
    downstream: &ElementPtr,
    upstream: &ElementPtr,
    connecting: Option<&ElementPtr>,
    doc: &Document,
    context: &dyn ShaderGraphCreateContext,
) {
    let up_name = upstream.borrow().get_name().to_string();
    if graph.get_node(&up_name).is_none() {
        if let Some(nd) = get_node_def_from_node(upstream, doc) {
            if let Ok(shader_node) = create_node_from_node(upstream, &nd, graph, doc, context) {
                let nd_name = nd.borrow().get_name().to_string();
                graph.add_node(shader_node);
                graph.set_node_def(&up_name, &nd_name);
                let graph_name = graph.get_name().to_string();
                // Connect interface inputs (interfacename → graph input socket, по рефу ~724)
                for node_input in get_inputs(upstream) {
                    if let Some(iface) = node_input.borrow().get_attribute(INTERFACE_NAME_ATTRIBUTE)
                    {
                        let iface = iface.to_string();
                        if !iface.is_empty() && graph.get_input_socket(&iface).is_some() {
                            let inp_name = node_input.borrow().get_name().to_string();
                            if graph
                                .get_node(&up_name)
                                .and_then(|n| n.get_input(&inp_name))
                                .is_some()
                            {
                                let _ =
                                    graph.make_connection(&up_name, &inp_name, &graph_name, &iface);
                            }
                        }
                    }
                }
                // Handle defaultgeomprop on unconnected inputs (по рефу createConnectedNodes ~93-107).
                // Path 1: Node input has interfacename → use graph input's defaultgeomprop.
                // Path 2: No interfacename → use nodedef input's defaultgeomprop (fallback).
                for node_input in get_inputs(upstream) {
                    if upstream
                        .borrow()
                        .get_parent()
                        .and_then(|ng| get_connected_node(&node_input, &ng))
                        .is_some()
                    {
                        continue;
                    }
                    let inp_name = node_input.borrow().get_name().to_string();
                    let geom_def_opt = if has_interface_name(&node_input) {
                        get_interface_input(&node_input)
                            .and_then(|gi| get_default_geom_prop_string(&gi))
                            .and_then(|n| doc.get_geom_prop_def(&n))
                    } else {
                        get_active_inputs(&nd)
                            .into_iter()
                            .find(|ndi| ndi.borrow().get_name() == inp_name)
                            .and_then(|ndi| get_default_geom_prop_string(&ndi))
                            .and_then(|n| doc.get_geom_prop_def(&n))
                    };
                    if let Some(geom_def) = geom_def_opt {
                        add_default_geom_node(graph, &up_name, &inp_name, &geom_def, doc, context);
                    }
                }
                // Apply input transforms — populate color/unit transform maps (по рефу applyInputTransforms)
                apply_input_transforms(graph, upstream, &up_name, doc, context);
            }
        }
    }

    let out_name = connecting
        .and_then(|c| c.borrow().get_attribute("output").map(|s| s.to_string()))
        .unwrap_or_else(|| "out".to_string());

    let down_cat = downstream.borrow().get_category().to_string();
    let graph_name = graph.get_name().to_string();

    if down_cat != category::INPUT && down_cat != category::OUTPUT {
        let down_name = downstream.borrow().get_name().to_string();
        if graph.get_node(&down_name).is_some() {
            let conn_input = connecting
                .map(|c| c.borrow().get_name().to_string())
                .unwrap_or_else(|| "in1".to_string());
            if graph
                .get_node(&down_name)
                .and_then(|n| n.get_input(&conn_input))
                .is_some()
            {
                let _ = graph.make_connection(&down_name, &conn_input, &up_name, &out_name);
            }
        }
    } else if down_cat == category::INPUT {
        // Downstream is an input of a node; connect that node's input to upstream
        if let Some(parent) = downstream.borrow().get_parent() {
            let down_name = parent.borrow().get_name().to_string();
            let conn_input = downstream.borrow().get_name().to_string();
            if graph.get_node(&down_name).is_some()
                && graph
                    .get_node(&down_name)
                    .and_then(|n| n.get_input(&conn_input))
                    .is_some()
            {
                let _ = graph.make_connection(&down_name, &conn_input, &up_name, &out_name);
            }
        }
    } else if down_cat == category::OUTPUT {
        let out_socket_name = downstream.borrow().get_name().to_string();
        if graph.get_output_socket(&out_socket_name).is_some() {
            let _ = graph.make_connection(&graph_name, &out_socket_name, &up_name, &out_name);
        }
    }
}

/// Get NodeDef output type. If NodeDef has 2+ outputs, returns "multioutput"
/// per C++ convention (MULTI_OUTPUT_TYPE_STRING). Otherwise returns element type
/// or first output's type.
fn get_node_def_output_type(nd: &ElementPtr) -> String {
    // C++: NodeDef with multiple outputs → "multioutput"
    let outputs = get_active_outputs(nd);
    if outputs.len() >= 2 {
        return crate::core::MULTI_OUTPUT_TYPE_STRING.to_string();
    }
    nd.borrow()
        .get_type()
        .map(|s| s.to_string())
        .or_else(|| {
            outputs
                .first()
                .and_then(|o| o.borrow().get_type().map(|s| s.to_string()))
        })
        .unwrap_or_else(|| "float".to_string())
}

/// Resolve NodeDef for a Node element. Uses nodedef attribute if present,
/// otherwise matches by node category + output type + exact input types
/// (по рефу Node::getNodeDef with hasExactInputMatch).
fn get_node_def_from_node(node: &ElementPtr, doc: &Document) -> Option<ElementPtr> {
    if let Some(nd_name) = node.borrow().get_attribute("nodedef") {
        return doc.get_node_def(&nd_name);
    }
    let node_cat = node.borrow().get_category().to_string();
    // C++ parity: empty type means "no type constraint" — don't filter by type.
    // Nodes inside NodeGraphs often omit the type attribute.
    let node_type = node.borrow().get_type().unwrap_or("").to_string();
    // C++: getMatchingNodeDefs(getQualifiedName) + getMatchingNodeDefs(getCategory)
    let mut rough_match: Option<ElementPtr> = None;
    for nd in doc.get_matching_node_defs(&node_cat) {
        let nd_type = get_node_def_output_type(&nd);
        if !node_type.is_empty() && nd_type != node_type {
            continue;
        }
        // C++: hasExactInputMatch — check each node input matches nodedef input by type
        if has_exact_input_match(node, &nd) {
            return Some(nd);
        }
        // C++: allowRoughMatch=true by default, collect first rough match
        if rough_match.is_none() {
            rough_match = Some(nd);
        }
    }
    rough_match
}

/// Check if node's active inputs all match the NodeDef's inputs by name and type
/// (по рефу InterfaceElement::hasExactInputMatch).
fn has_exact_input_match(node: &ElementPtr, node_def: &ElementPtr) -> bool {
    for input in get_active_inputs(node) {
        let inp = input.borrow();
        let name = inp.get_name();
        let inp_type = match inp.get_type() {
            Some(t) => t.to_string(),
            None => continue,
        };
        // Find matching input in NodeDef
        let nd = node_def.borrow();
        let decl_input = nd.get_child(name);
        match decl_input {
            Some(di) => {
                let di_type = di.borrow().get_type().unwrap_or("").to_string();
                if di_type != inp_type {
                    return false;
                }
            }
            None => return false,
        }
    }
    true
}

/// Populate color/unit transform maps from node inputs (по рефу ShaderGraph::applyInputTransforms ~649).
fn apply_input_transforms(
    graph: &mut ShaderGraph,
    node: &ElementPtr,
    shader_node_name: &str,
    doc: &Document,
    context: &dyn ShaderGraphCreateContext,
) {
    let cms = match context.get_color_management_system() {
        Some(c) => c,
        None => return,
    };
    let unit_sys = context.get_unit_system();
    let target_color_space = if context.get_options().target_color_space_override.is_empty() {
        get_active_color_space(&doc.get_root())
    } else {
        context.get_options().target_color_space_override.clone()
    };
    let target_distance_unit = context.get_options().target_distance_unit.clone();

    for input in get_inputs(node) {
        let has_value = input
            .borrow()
            .get_attribute("value")
            .map(|s| !s.is_empty())
            .unwrap_or(false);
        let has_interface = input.borrow().has_attribute(INTERFACE_NAME_ATTRIBUTE);
        if !has_value && !has_interface {
            continue;
        }
        let source_color_space = get_active_color_space(&input);
        let inp_name = input.borrow().get_name().to_string();

        if let Some(shader_input) = graph
            .get_node(shader_node_name)
            .and_then(|n| n.get_input(&inp_name))
        {
            let type_desc = shader_input.port.get_type().clone();
            let type_name = type_desc.get_name();
            if type_name == "color3" || type_name == "color4" {
                if !source_color_space.is_empty()
                    && !target_color_space.is_empty()
                    && source_color_space != target_color_space
                    && source_color_space != "none"
                    && target_color_space != "none"
                {
                    let transform = ColorSpaceTransform::new(
                        source_color_space.clone(),
                        target_color_space.clone(),
                        type_desc,
                    );
                    if cms.supports_transform(&transform) {
                        graph.input_color_transforms.push((
                            shader_node_name.to_string(),
                            inp_name.clone(),
                            transform,
                        ));
                    }
                }
            }
        }

        if let Some(unit_sys) = unit_sys {
            let source_unit = input.borrow().get_unit().unwrap_or("").to_string();
            if !source_unit.is_empty() && !target_distance_unit.is_empty() {
                let unit_type = input
                    .borrow()
                    .get_unit_type()
                    .unwrap_or("distance")
                    .to_string();
                if doc.get_unit_type_def(&unit_type).is_some() {
                    if let Some(shader_input) = graph
                        .get_node(shader_node_name)
                        .and_then(|n| n.get_input(&inp_name))
                    {
                        let type_desc = shader_input.port.get_type().clone();
                        let tn = type_desc.get_name();
                        if tn == "float" || tn == "vector2" || tn == "vector3" || tn == "vector4" {
                            let transform = UnitTransform::new(
                                source_unit,
                                target_distance_unit.clone(),
                                type_desc,
                                unit_type,
                            );
                            if unit_sys.supports_transform(&transform) {
                                graph.input_unit_transforms.push((
                                    shader_node_name.to_string(),
                                    inp_name,
                                    transform,
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Create ShaderNode from MaterialX Node (по рефу ShaderNode::create, createNode(Node)).
/// Context's get_implementation_for_nodedef returns impl already initialized with impl element.
fn create_node_from_node(
    node: &ElementPtr,
    node_def: &ElementPtr,
    graph: &ShaderGraph,
    doc: &Document,
    context: &dyn ShaderGraphCreateContext,
) -> Result<ShaderNode, String> {
    let node_name = node.borrow().get_name().to_string();
    let node_def_name = node_def.borrow().get_name().to_string();
    let target = context.get_implementation_target();

    let impl_ = context
        .get_implementation_for_nodedef(doc, &node_def_name, target)
        .ok_or_else(|| {
            format!(
                "No implementation for node '{}' target '{}'",
                node_def_name, target
            )
        })?;

    let mut shader_node =
        ShaderNode::create_from_nodedef(Some(graph.get_name()), &node_name, node_def, context);
    impl_.add_inputs(&mut shader_node, context);
    if let Some(registry) = context.get_shader_metadata_registry() {
        if registry.has_metadata() {
            shader_node.create_metadata(node_def, registry);
        }
    }
    shader_node.initialize(node, node_def, context.get_type_system());
    impl_.set_values(node, &mut shader_node, context);

    Ok(shader_node)
}

/// Add default geom node for unconnected input with defaultgeomprop (по рефу ShaderGraph::addDefaultGeomNode ~213).
pub fn add_default_geom_node(
    graph: &mut ShaderGraph,
    node_name: &str,
    input_name: &str,
    geom_prop_def: &ElementPtr,
    doc: &Document,
    context: &dyn ShaderGraphCreateContext,
) {
    let geom_prop_name = geom_prop_def.borrow().get_name().to_string();
    let geom_node_name = format!("geomprop_{}", geom_prop_name);
    if graph.get_node(&geom_node_name).is_none() {
        let input_type = graph
            .get_node(node_name)
            .and_then(|n| n.get_input(input_name))
            .map(|i| i.port.get_type().get_name().to_string())
            .unwrap_or_else(|| "float".to_string());
        let geom_prop = get_geom_prop(geom_prop_def).unwrap_or_default();
        let geom_node_def_name = format!("ND_{}_{}", geom_prop, input_type);
        let geom_node_def = match doc.get_node_def(&geom_node_def_name) {
            Some(nd) => nd,
            None => return,
        };
        let shader_node =
            match create_node_from_nodedef(&geom_node_name, &geom_node_def, doc, context) {
                Ok(n) => n,
                Err(_) => return,
            };
        let mut geom_node = shader_node;
        // Set geom node inputs from GeomPropDef (по рефу ~234-275)
        let name_path = geom_prop_def.borrow().get_name_path(None);
        if let Some(space) = get_space(geom_prop_def) {
            if !space.is_empty() {
                if let Some(space_inp) = geom_node.inputs.get_mut("space") {
                    let space_value: Option<Value> =
                        if let Some(space_input_elem) = get_active_input(&geom_node_def, "space") {
                            let enum_names = space_input_elem
                                .borrow()
                                .get_attribute(ENUM_ATTRIBUTE)
                                .unwrap_or("")
                                .to_string();
                            let type_desc = context.get_type_desc("string");
                            if let Some((_, remapped)) = context.get_syntax().remap_enumeration(
                                &space,
                                &type_desc,
                                &enum_names,
                            ) {
                                Some(remapped)
                            } else {
                                Value::from_strings(&space, "string")
                            }
                        } else {
                            Value::from_strings(&space, "string")
                        };
                    if let Some(v) = space_value {
                        space_inp.port_mut().set_value(Some(v), false);
                    }
                    space_inp.port_mut().set_path(&name_path);
                }
            }
        }
        if let Some(index) = get_index(geom_prop_def) {
            if !index.is_empty() {
                if let Some(idx_inp) = geom_node.inputs.get_mut("index") {
                    let _ = Value::from_strings(&index, "integer").map(|v| {
                        idx_inp.port_mut().set_value(Some(v), false);
                    });
                    idx_inp.port_mut().set_path(&name_path);
                }
            }
        }
        if let Some(gp) = get_geom_prop(geom_prop_def) {
            if !gp.is_empty() {
                if let Some(gp_inp) = geom_node.inputs.get_mut(GEOM_PROP_ATTRIBUTE) {
                    let _ = Value::from_strings(&gp, "string").map(|v| {
                        gp_inp.port_mut().set_value(Some(v), false);
                    });
                    gp_inp.port_mut().set_path(&name_path);
                }
            }
        }
        // Assign variable for output
        let out_order: Vec<_> = geom_node.output_order.clone();
        let out_name = out_order
            .first()
            .cloned()
            .unwrap_or_else(|| "out".to_string());
        let (pname, ptype) = geom_node
            .outputs
            .get(&out_name)
            .map(|p| {
                (
                    format!("{}_{}", geom_node_name, p.get_name()),
                    p.get_type().clone(),
                )
            })
            .unwrap_or((geom_node_name.clone(), context.get_type_desc("float")));
        let var =
            context
                .get_syntax()
                .get_variable_name(&pname, &ptype, graph.get_identifier_map());
        if let Some(port) = geom_node.outputs.get_mut(&out_name) {
            port.port_mut().set_variable(&var);
        }
        graph.add_node(geom_node);
        graph.set_node_def(&geom_node_name, &geom_node_def.borrow().get_name());
    }
    let geom_out: String = graph
        .get_node(&geom_node_name)
        .and_then(|n| n.output_order.first())
        .cloned()
        .unwrap_or_else(|| "out".to_string());
    let _ = graph.make_connection(node_name, input_name, &geom_node_name, &geom_out);
}

/// Add color transform node for input (по рефу ShaderGraph::addColorTransformNode(ShaderInput*) ~290).
fn add_color_transform_node(
    graph: &mut ShaderGraph,
    node_name: &str,
    input_name: &str,
    transform: &ColorSpaceTransform,
    doc: &Document,
    context: &dyn ShaderGraphCreateContext,
    cms: &dyn ColorManagementSystem,
) {
    let full_name = format!("{}_{}", node_name, input_name);
    let cm_node_name = format!("{}_cm", full_name);
    if graph.get_node(&cm_node_name).is_some() {
        return;
    }
    let (has_conn, value_to_copy) = {
        let shader_node = match graph.get_node(node_name) {
            Some(n) => n,
            None => return,
        };
        let shader_input = match shader_node.get_input(input_name) {
            Some(i) => i,
            None => return,
        };
        (
            shader_input.has_connection(),
            shader_input.port.get_value().cloned(),
        )
    };
    if has_conn {
        return;
    }

    let mut transform_node = match cms.create_node(transform, &cm_node_name, doc, context) {
        Some(n) => n,
        None => return,
    };
    let out_name = transform_node
        .output_order
        .first()
        .cloned()
        .unwrap_or_else(|| "out".to_string());
    let var = context.get_syntax().get_variable_name(
        &full_name,
        &transform.type_desc,
        graph.get_identifier_map(),
    );
    let first_inp = transform_node.input_order.first().cloned();
    if let Some(inp_key) = first_inp {
        if let Some(port) = transform_node.inputs.get_mut(&inp_key) {
            port.port_mut().set_variable(&var);
            if let Some(v) = value_to_copy {
                port.port_mut().set_value(Some(v), false);
            }
        }
    }
    if let Some(port) = transform_node.outputs.get_mut(&out_name) {
        port.port_mut()
            .set_variable(&format!("{}_{}", cm_node_name, out_name));
    }
    graph.add_node(transform_node);
    let _ = graph.make_connection(node_name, input_name, &cm_node_name, &out_name);
}

/// Add unit transform node for input (по рефу ShaderGraph::addUnitTransformNode(ShaderInput*) ~360).
fn add_unit_transform_node(
    graph: &mut ShaderGraph,
    node_name: &str,
    input_name: &str,
    transform: &UnitTransform,
    doc: &Document,
    context: &dyn ShaderGraphCreateContext,
    unit_sys: &dyn UnitSystem,
) {
    let full_name = format!("{}_{}", node_name, input_name);
    let unit_node_name = format!("{}_unit", full_name);
    if graph.get_node(&unit_node_name).is_some() {
        return;
    }
    if graph
        .get_node(node_name)
        .and_then(|n| n.get_input(input_name))
        .map(|i| i.has_connection())
        .unwrap_or(true)
    {
        return;
    }
    let mut transform_node = match unit_sys.create_node(transform, &unit_node_name, doc, context) {
        Some(n) => n,
        None => return,
    };
    let out_name = transform_node
        .output_order
        .first()
        .cloned()
        .unwrap_or_else(|| "out".to_string());
    let var = context.get_syntax().get_variable_name(
        &full_name,
        &transform.type_desc,
        graph.get_identifier_map(),
    );
    if let Some(port) = transform_node.inputs.get_mut("in1") {
        port.port_mut().set_variable(&var);
    }
    if let Some(port) = transform_node.outputs.get_mut(&out_name) {
        port.port_mut()
            .set_variable(&format!("{}_{}", unit_node_name, out_name));
    }
    graph.add_node(transform_node);
    let _ = graph.make_connection(node_name, input_name, &unit_node_name, &out_name);
}

/// Add color transform node for output (по рефу ShaderGraph::addColorTransformNode(ShaderOutput*) ~329).
/// Inserts transform between upstream and all downstream connections of the output socket.
fn add_output_color_transform_node(
    graph: &mut ShaderGraph,
    output_name: &str,
    transform: &ColorSpaceTransform,
    doc: &Document,
    context: &dyn ShaderGraphCreateContext,
    cms: &dyn ColorManagementSystem,
) {
    let graph_name = graph.get_name().to_string();
    let full_name = format!("{}_{}", graph_name, output_name);
    let cm_node_name = format!("{}_cm", full_name);
    if graph.get_node(&cm_node_name).is_some() {
        return;
    }
    // Get upstream connection from the output socket
    let upstream = graph
        .get_output_socket(output_name)
        .and_then(|s| s.get_connection())
        .map(|(n, o)| (n.to_string(), o.to_string()));

    let mut transform_node = match cms.create_node(transform, &cm_node_name, doc, context) {
        Some(n) => n,
        None => return,
    };
    let out_name = transform_node
        .output_order
        .first()
        .cloned()
        .unwrap_or_else(|| "out".to_string());
    let first_inp = transform_node.input_order.first().cloned();

    // Set variable names on the transform node ports
    if let Some(port) = transform_node.outputs.get_mut(&out_name) {
        port.port_mut()
            .set_variable(&format!("{}_{}", cm_node_name, out_name));
    }
    if let Some(ref inp_key) = first_inp {
        if let Some(port) = transform_node.inputs.get_mut(inp_key) {
            port.port_mut()
                .set_variable(&format!("{}_{}", cm_node_name, inp_key));
        }
    }

    graph.add_node(transform_node);

    // Connect: output_socket -> cm_node output
    let _ = graph.make_connection(&graph_name, output_name, &cm_node_name, &out_name);

    // Connect: upstream -> cm_node input
    if let (Some((up_n, up_o)), Some(inp_key)) = (upstream, first_inp) {
        let _ = graph.make_connection(&cm_node_name, &inp_key, &up_n, &up_o);
    }
}

/// Add unit transform node for output (по рефу ShaderGraph::addUnitTransformNode(ShaderOutput*) ~400).
/// Inserts transform between upstream and all downstream connections of the output socket.
fn add_output_unit_transform_node(
    graph: &mut ShaderGraph,
    output_name: &str,
    transform: &UnitTransform,
    doc: &Document,
    context: &dyn ShaderGraphCreateContext,
    unit_sys: &dyn UnitSystem,
) {
    let graph_name = graph.get_name().to_string();
    let full_name = format!("{}_{}", graph_name, output_name);
    let unit_node_name = format!("{}_unit", full_name);
    if graph.get_node(&unit_node_name).is_some() {
        return;
    }
    let upstream = graph
        .get_output_socket(output_name)
        .and_then(|s| s.get_connection())
        .map(|(n, o)| (n.to_string(), o.to_string()));

    let mut transform_node = match unit_sys.create_node(transform, &unit_node_name, doc, context) {
        Some(n) => n,
        None => return,
    };
    let out_name = transform_node
        .output_order
        .first()
        .cloned()
        .unwrap_or_else(|| "out".to_string());
    let first_inp = transform_node.input_order.first().cloned();

    if let Some(port) = transform_node.outputs.get_mut(&out_name) {
        port.port_mut()
            .set_variable(&format!("{}_{}", unit_node_name, out_name));
    }
    if let Some(ref inp_key) = first_inp {
        if let Some(port) = transform_node.inputs.get_mut(inp_key) {
            port.port_mut()
                .set_variable(&format!("{}_{}", unit_node_name, inp_key));
        }
    }

    graph.add_node(transform_node);

    let _ = graph.make_connection(&graph_name, output_name, &unit_node_name, &out_name);

    if let (Some((up_n, up_o)), Some(inp_key)) = (upstream, first_inp) {
        let _ = graph.make_connection(&unit_node_name, &inp_key, &up_n, &up_o);
    }
}

/// Finalize graph: classification, transforms, optimize, topological sort, set variable names (по рефу finalize ~870).
fn finalize(graph: &mut ShaderGraph, doc: &Document, context: &dyn ShaderGraphCreateContext) {
    let target = context.get_implementation_target();
    let node_order = graph.node_order.clone();
    for node_name in node_order {
        let Some(node_def_name) = graph.get_node_def(&node_name).map(str::to_string) else {
            continue;
        };
        let Some(impl_) = context.get_implementation_for_nodedef(doc, &node_def_name, target)
        else {
            continue;
        };
        if let Some(node) = graph.get_node_mut(&node_name) {
            impl_.add_classification(node);
        }
    }

    if let Some(output_socket) = graph.get_output_socket_at(0) {
        if let Some((upstream_node, _upstream_output)) = output_socket.get_connection() {
            if let Some(node) = graph.get_node(upstream_node) {
                graph.node.add_classification(node.classification);
            }
        }
    }

    // Insert color transformation nodes for inputs and outputs (по рефу ~887-898)
    if context.get_options().emit_color_transforms {
        if let Some(cms) = context.get_color_management_system() {
            let input_transforms = std::mem::take(&mut graph.input_color_transforms);
            for (node_name, inp_name, transform) in input_transforms {
                add_color_transform_node(
                    graph, &node_name, &inp_name, &transform, doc, context, cms,
                );
            }
            let output_transforms = std::mem::take(&mut graph.output_color_transforms);
            for (_node_name, out_name, transform) in output_transforms {
                add_output_color_transform_node(graph, &out_name, &transform, doc, context, cms);
            }
        }
    }
    // Insert unit transformation nodes for inputs and outputs (по рефу ~902-911)
    if let Some(unit_sys) = context.get_unit_system() {
        let input_transforms = std::mem::take(&mut graph.input_unit_transforms);
        for (node_name, inp_name, transform) in input_transforms {
            add_unit_transform_node(
                graph, &node_name, &inp_name, &transform, doc, context, unit_sys,
            );
        }
        let output_transforms = std::mem::take(&mut graph.output_unit_transforms);
        for (_node_name, out_name, transform) in output_transforms {
            add_output_unit_transform_node(graph, &out_name, &transform, doc, context, unit_sys);
        }
    }

    if context.get_options().shader_interface_type == ShaderInterfaceType::Complete {
        let graph_name = graph.get_name().to_string();
        let node_order = graph.node_order.clone();
        let mut published_inputs: Vec<(
            String,
            String,
            String,
            super::TypeDesc,
            String,
            Option<Value>,
            String,
            String,
            bool,
            Vec<super::shader_metadata_registry::ShaderPortMetadata>,
        )> = Vec::new();

        for node_name in node_order {
            let Some(node) = graph.get_node(&node_name) else {
                continue;
            };
            let node_def_name = graph.get_node_def(&node_name).map(str::to_string);
            for input_name in node.input_order.clone() {
                let Some(input) = node.get_input(&input_name) else {
                    continue;
                };
                if input.has_connection() || input.get_type().is_closure() {
                    continue;
                }
                if !node.is_editable(&input_name, node_def_name.as_deref(), doc, context) {
                    continue;
                }
                published_inputs.push((
                    node_name.clone(),
                    input_name.clone(),
                    format!("{}_{}", node_name, input_name),
                    input.get_type().clone(),
                    input.port.get_path().to_string(),
                    input.port.get_value().cloned(),
                    input.port.get_unit().to_string(),
                    input.port.get_color_space().to_string(),
                    input.port.is_uniform(),
                    input.port.get_metadata().to_vec(),
                ));
            }
        }

        for (
            node_name,
            input_name,
            interface_name,
            type_desc,
            path,
            value,
            unit,
            colorspace,
            is_uniform,
            metadata,
        ) in published_inputs
        {
            if graph.get_input_socket(&interface_name).is_none() {
                let input_socket = graph.add_input_socket(&interface_name, type_desc);
                input_socket.port_mut().set_path(&path);
                input_socket.port_mut().set_value(value, false);
                input_socket.port_mut().set_unit(&unit);
                input_socket.port_mut().set_color_space(&colorspace);
                if is_uniform {
                    input_socket.port_mut().set_uniform(true);
                }
            }

            let _ = graph.make_connection(&node_name, &input_name, &graph_name, &interface_name);
            if let Some(socket) = graph.node.outputs.get_mut(&interface_name) {
                socket.port_mut().metadata = metadata;
            }
        }
    }

    graph.optimize(context.get_options().elide_constant_nodes);
    graph.topological_sort();
    set_variable_names(graph, context);
}

/// Set variable names on all ports (по рефу setVariableNames).
fn set_variable_names(graph: &mut ShaderGraph, context: &dyn ShaderGraphCreateContext) {
    let syntax = context.get_syntax();

    let out_order: Vec<_> = graph.node.output_order.clone();
    for name in out_order {
        let (pname, ptype) = graph
            .node
            .outputs
            .get(&name)
            .map(|p| (p.get_name().to_string(), p.get_type().clone()))
            .unwrap_or((name.clone(), context.get_type_desc("float")));
        let var = syntax.get_variable_name(&pname, &ptype, graph.get_identifier_map());
        if let Some(port) = graph.node.outputs.get_mut(&name) {
            port.port_mut().set_variable(&var);
        }
    }
    let inp_order: Vec<_> = graph.node.input_order.clone();
    for name in inp_order {
        let (pname, ptype) = graph
            .node
            .inputs
            .get(&name)
            .map(|p| (p.get_name().to_string(), p.get_type().clone()))
            .unwrap_or((name.clone(), context.get_type_desc("float")));
        let var = syntax.get_variable_name(&pname, &ptype, graph.get_identifier_map());
        if let Some(port) = graph.node.inputs.get_mut(&name) {
            port.port_mut().set_variable(&var);
        }
    }
    let node_names: Vec<String> = graph.nodes.keys().cloned().collect();
    for node_name in node_names {
        let out_order: Vec<_> = graph
            .nodes
            .get(&node_name)
            .map(|n| n.output_order.clone())
            .unwrap_or_default();
        for out_name in out_order {
            let (full, ptype) = graph
                .nodes
                .get(&node_name)
                .and_then(|n| n.outputs.get(&out_name))
                .map(|p| {
                    (
                        format!("{}_{}", node_name, p.get_name()),
                        p.get_type().clone(),
                    )
                })
                .unwrap_or((
                    format!("{}_{}", node_name, out_name),
                    context.get_type_desc("float"),
                ));
            let var = syntax.get_variable_name(&full, &ptype, graph.get_identifier_map());
            if let Some(node) = graph.nodes.get_mut(&node_name) {
                if let Some(port) = node.outputs.get_mut(&out_name) {
                    port.port_mut().set_variable(&var);
                }
            }
        }
        let inp_order: Vec<_> = graph
            .nodes
            .get(&node_name)
            .map(|n| n.input_order.clone())
            .unwrap_or_default();
        for inp_name in inp_order {
            let (full, ptype) = graph
                .nodes
                .get(&node_name)
                .and_then(|n| n.inputs.get(&inp_name))
                .map(|p| {
                    (
                        format!("{}_{}", node_name, p.get_name()),
                        p.get_type().clone(),
                    )
                })
                .unwrap_or((
                    format!("{}_{}", node_name, inp_name),
                    context.get_type_desc("float"),
                ));
            let var = syntax.get_variable_name(&full, &ptype, graph.get_identifier_map());
            if let Some(node) = graph.nodes.get_mut(&node_name) {
                if let Some(port) = node.inputs.get_mut(&inp_name) {
                    port.port_mut().set_variable(&var);
                }
            }
        }
    }
}
