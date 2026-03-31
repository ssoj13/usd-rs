//! Node, Input, Output — node graph elements.

use crate::core::element::{
    DEFAULT_GEOM_PROP_ATTRIBUTE, ElementPtr, INTERFACE_NAME_ATTRIBUTE, NODE_ATTRIBUTE,
    NODE_DEF_ATTRIBUTE, NODE_GRAPH_ATTRIBUTE, NODE_NAME_ATTRIBUTE, OUTPUT_ATTRIBUTE,
    TARGET_ATTRIBUTE, VALUE_ATTRIBUTE, category,
};

/// Get input child by name (Node, NodeDef, NodeGraph)
pub fn get_input(parent: &ElementPtr, name: &str) -> Option<ElementPtr> {
    let child = parent.borrow().get_child(name)?;
    if child.borrow().get_category() == category::INPUT {
        Some(child)
    } else {
        None
    }
}

/// Get output child by name (NodeGraph, NodeDef)
pub fn get_output(parent: &ElementPtr, name: &str) -> Option<ElementPtr> {
    let child = parent.borrow().get_child(name)?;
    if child.borrow().get_category() == category::OUTPUT {
        Some(child)
    } else {
        None
    }
}

/// Get connected node for an input (nodename -> sibling in same graph)
pub fn get_connected_node(input: &ElementPtr, graph: &ElementPtr) -> Option<ElementPtr> {
    let node_name = input.borrow().get_node_name()?.to_string();
    graph.borrow().get_child(&node_name)
}

/// Resolve the connected output for an input, following interface bindings when needed.
/// Matches MaterialX Input::getConnectedOutput / PortElement::getConnectedOutput semantics.
pub fn get_connected_output(input: &ElementPtr) -> Option<ElementPtr> {
    if has_interface_name(input) {
        if let Some(interface_input) = get_interface_input(input) {
            return get_connected_output(&interface_input);
        }
    }

    let parent = input.borrow().get_parent()?;
    let scope = parent.borrow().get_parent();
    let output_name = input.borrow().get_output_string().unwrap_or("").to_string();

    if let Some(node_graph_name) = input
        .borrow()
        .get_node_graph_string()
        .map(|s| s.to_string())
    {
        if node_graph_name.is_empty() {
            return None;
        }
        let node_graph = scope
            .as_ref()
            .and_then(|s| s.borrow().get_child(&node_graph_name))
            .or_else(|| parent.borrow().get_child(&node_graph_name))?;
        return if output_name.is_empty() {
            get_outputs(&node_graph).into_iter().next()
        } else {
            get_output(&node_graph, &output_name)
        };
    }

    let node_name = input.borrow().get_node_name().map(|s| s.to_string())?;
    if node_name.is_empty() {
        return None;
    }
    let node = scope
        .as_ref()
        .and_then(|s| s.borrow().get_child(&node_name))
        .or_else(|| parent.borrow().get_child(&node_name))?;
    if output_name.is_empty() {
        get_outputs(&node).into_iter().next()
    } else {
        get_output(&node, &output_name)
    }
}

/// Set connected node for an input (by node name)
pub fn set_connected_node_name(
    input: &mut std::cell::RefMut<'_, crate::core::element::Element>,
    node_name: impl Into<String>,
) {
    let name = node_name.into();
    if name.is_empty() {
        input.remove_attribute(NODE_NAME_ATTRIBUTE);
    } else {
        input.set_node_name(name);
    }
}

/// Get all input children
pub fn get_inputs(parent: &ElementPtr) -> Vec<ElementPtr> {
    parent
        .borrow()
        .get_children()
        .iter()
        .filter(|c| c.borrow().get_category() == category::INPUT)
        .cloned()
        .collect()
}

/// Get all output children
pub fn get_outputs(parent: &ElementPtr) -> Vec<ElementPtr> {
    parent
        .borrow()
        .get_children()
        .iter()
        .filter(|c| c.borrow().get_category() == category::OUTPUT)
        .cloned()
        .collect()
}

/// Input: get defaultgeomprop string
pub fn get_default_geom_prop_string(input: &ElementPtr) -> Option<String> {
    input
        .borrow()
        .get_attribute(DEFAULT_GEOM_PROP_ATTRIBUTE)
        .map(|s| s.to_string())
}

/// Input: has defaultgeomprop
pub fn has_default_geom_prop_string(input: &ElementPtr) -> bool {
    input.borrow().has_attribute(DEFAULT_GEOM_PROP_ATTRIBUTE)
}

/// Input: has interfacename attribute (binds to parent NodeGraph/NodeDef input).
pub fn has_interface_name(input: &ElementPtr) -> bool {
    input
        .borrow()
        .get_attribute(INTERFACE_NAME_ATTRIBUTE)
        .map(|s| !s.is_empty())
        .unwrap_or(false)
}

/// Input: get interface name (value of interfacename attribute)
pub fn get_interface_name(input: &ElementPtr) -> Option<String> {
    input
        .borrow()
        .get_attribute(INTERFACE_NAME_ATTRIBUTE)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

/// Set Input to connect to the given Output (MaterialX PortElement::setConnectedOutput).
/// Connects to NodeGraph output (nodegraph+nodename cleared) or Node output (nodename+nodegraph cleared).
pub fn set_connected_output(input: &ElementPtr, output: Option<&ElementPtr>) {
    let mut inp = input.borrow_mut();
    if let Some(out) = output {
        inp.set_output_string(out.borrow().get_name());
        if let Some(ref parent) = out.borrow().get_parent() {
            let cat = parent.borrow().get_category().to_string();
            if cat == category::NODE_GRAPH {
                inp.set_node_graph_string(parent.borrow().get_name());
                inp.remove_attribute(NODE_NAME_ATTRIBUTE);
            } else if cat == category::NODE {
                inp.set_node_name(parent.borrow().get_name());
                inp.remove_attribute(NODE_GRAPH_ATTRIBUTE);
            }
        }
        inp.remove_attribute(VALUE_ATTRIBUTE);
    } else {
        inp.remove_attribute(OUTPUT_ATTRIBUTE);
        inp.remove_attribute(NODE_GRAPH_ATTRIBUTE);
        inp.remove_attribute(NODE_NAME_ATTRIBUTE);
    }
}

/// Input: get the NodeGraph's input that this binds to (C++ Input::getInterfaceInput).
/// Returns the ancestor NodeGraph's input element, or NodeDef's if NodeGraph has nodedef but no direct input.
pub fn get_interface_input(input: &ElementPtr) -> Option<ElementPtr> {
    let iface_name = get_interface_name(input)?;
    let mut current = input.borrow().get_parent();
    while let Some(p) = current {
        if p.borrow().get_category() == category::NODE_GRAPH {
            if let Some(inp) = get_input(&p, &iface_name) {
                return Some(inp);
            }
            if let Some(nd_name) = p.borrow().get_attribute(NODE_DEF_ATTRIBUTE) {
                if let Some(doc) = crate::core::Document::from_element(&p) {
                    if let Some(nd) = doc.get_node_def(nd_name) {
                        return get_input(&nd, &iface_name);
                    }
                }
            }
            return None;
        }
        current = p.borrow().get_parent();
    }
    None
}

/// Get active color space (walk element and parents). C++ Element::getActiveColorSpace.
pub fn get_active_color_space(elem: &ElementPtr) -> String {
    let mut current: Option<ElementPtr> = Some(elem.clone());
    while let Some(p) = current.take() {
        if p.borrow().has_color_space() {
            return p
                .borrow()
                .get_attribute(crate::core::element::COLOR_SPACE_ATTRIBUTE)
                .map(|s| s.to_string())
                .unwrap_or_default();
        }
        current = p.borrow().get_parent();
    }
    String::new()
}

// --- Node-specific helpers (C++ Node class) ---

/// Get the first NodeDef that declares this node, optionally filtered by target.
/// C++ Node::getNodeDef.
pub fn get_node_def(
    node: &ElementPtr,
    target: &str,
    allow_rough_match: bool,
) -> Option<ElementPtr> {
    // First check nodedef attribute
    if let Some(nd_name) = node
        .borrow()
        .get_attribute(NODE_DEF_ATTRIBUTE)
        .map(|s| s.to_string())
    {
        if !nd_name.is_empty() {
            if let Some(doc) = crate::core::Document::from_element(node) {
                if let Some(nd) = doc.get_node_def(&nd_name) {
                    return Some(nd);
                }
            }
        }
    }
    // Find via matching node defs by node string
    let node_str = {
        let node_ref = node.borrow();
        if node_ref.get_category() != category::NODE {
            node_ref.get_category().to_string()
        } else {
            node_ref
                .get_attribute(NODE_ATTRIBUTE)
                .map(|s| s.to_string())?
        }
    };
    let doc = crate::core::Document::from_element(node)?;
    let defs = doc.get_matching_node_defs(&node_str);

    // First pass: exact match (target + exact inputs)
    for nd in &defs {
        if !target.is_empty() && nd.borrow().has_attribute(TARGET_ATTRIBUTE) {
            let nd_target = nd
                .borrow()
                .get_attribute(TARGET_ATTRIBUTE)
                .unwrap_or("")
                .to_string();
            if !crate::core::element::target_strings_match(&nd_target, target) {
                continue;
            }
        }
        if crate::core::interface::has_exact_input_match(node, nd) {
            return Some(nd.clone());
        }
    }

    // Second pass: rough match (just target)
    if allow_rough_match {
        for nd in &defs {
            if !target.is_empty() && nd.borrow().has_attribute(TARGET_ATTRIBUTE) {
                let nd_target = nd
                    .borrow()
                    .get_attribute(TARGET_ATTRIBUTE)
                    .unwrap_or("")
                    .to_string();
                if !crate::core::element::target_strings_match(&nd_target, target) {
                    continue;
                }
            }
            return Some(nd.clone());
        }
    }

    None
}

/// Add inputs from the associated NodeDef to this node.
/// C++ Node::addInputsFromNodeDef.
pub fn add_inputs_from_node_def(node: &ElementPtr) {
    let nd = match get_node_def(node, "", true) {
        Some(nd) => nd,
        None => return,
    };
    let nd_inputs = get_inputs(&nd);
    for nd_inp in &nd_inputs {
        let inp_name = nd_inp.borrow().get_name().to_string();
        if get_input(node, &inp_name).is_some() {
            continue; // already exists
        }
        let inp_type = nd_inp
            .borrow()
            .get_type()
            .map(|s| s.to_string())
            .unwrap_or_default();
        if let Ok(new_inp) =
            crate::core::element::add_child_of_category(node, category::INPUT, &inp_name)
        {
            if !inp_type.is_empty() {
                new_inp.borrow_mut().set_attribute("type", &inp_type);
            }
            // Copy value if present
            if nd_inp.borrow().has_value_string() {
                let val = nd_inp.borrow().get_value_string();
                new_inp.borrow_mut().set_value_string(val);
            }
        }
    }
}

/// Get interface name (C++ Input::getInterfaceName without filter)
pub fn get_interface_name_raw(input: &ElementPtr) -> String {
    input
        .borrow()
        .get_attribute(INTERFACE_NAME_ATTRIBUTE)
        .map(|s| s.to_string())
        .unwrap_or_default()
}

/// Return all downstream ports that connect to this node.
/// C++ Node::getDownstreamPorts.
pub fn get_downstream_ports(node: &ElementPtr) -> Vec<ElementPtr> {
    let node_name = node.borrow().get_name().to_string();
    let parent = match node.borrow().get_parent() {
        Some(p) => p,
        None => return Vec::new(),
    };
    let mut result = Vec::new();
    for sibling in parent.borrow().get_children() {
        let cat = sibling.borrow().get_category().to_string();
        // Check inputs/outputs of sibling nodes and outputs of the parent graph
        let children_to_check: Vec<ElementPtr> = if cat == category::OUTPUT {
            // Direct output of the graph
            vec![sibling.clone()]
        } else {
            // Inputs of sibling nodes
            sibling
                .borrow()
                .get_children()
                .iter()
                .filter(|c| {
                    let c_cat = c.borrow().get_category().to_string();
                    c_cat == category::INPUT || c_cat == category::OUTPUT
                })
                .cloned()
                .collect()
        };
        for child in &children_to_check {
            if let Some(nn) = child.borrow().get_node_name() {
                if nn == node_name {
                    result.push(child.clone());
                }
            }
        }
    }
    // Sort by name for deterministic order (C++ sorts by name)
    result.sort_by(|a, b| a.borrow().get_name().cmp(b.borrow().get_name()));
    result
}

// ─────────────────────────────────────────────────────────────────────────────
// NodeGraph methods (C++ NodeGraph class)
// ─────────────────────────────────────────────────────────────────────────────

/// Set the NodeDef reference for a NodeGraph (C++ NodeGraph::setNodeDef).
/// Stores the nodedef name as the `nodedef` attribute.
pub fn nodegraph_set_node_def(graph: &ElementPtr, nodedef_name: impl Into<String>) {
    let name = nodedef_name.into();
    if name.is_empty() {
        graph.borrow_mut().remove_attribute(NODE_DEF_ATTRIBUTE);
    } else {
        graph.borrow_mut().set_attribute(NODE_DEF_ATTRIBUTE, name);
    }
}

/// Get the NodeDef element for a NodeGraph (C++ NodeGraph::getNodeDef).
/// Returns the name of the nodedef attribute, if any.
pub fn nodegraph_get_node_def_name(graph: &ElementPtr) -> Option<String> {
    graph
        .borrow()
        .get_attribute(NODE_DEF_ATTRIBUTE)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

/// Resolve the NodeDef element for a NodeGraph via the document.
/// Also checks Implementation elements that reference this graph.
/// C++ NodeGraph::getNodeDef.
pub fn nodegraph_resolve_node_def(graph: &ElementPtr) -> Option<ElementPtr> {
    // Direct nodedef attribute.
    if let Some(nd_name) = nodegraph_get_node_def_name(graph) {
        if let Some(doc) = crate::core::Document::from_element(graph) {
            return doc.get_node_def(&nd_name);
        }
    }
    // Fallback: find via implementations that reference this graph.
    let graph_name = graph.borrow().get_name().to_string();
    if let Some(doc) = crate::core::Document::from_element(graph) {
        let root = crate::core::element::get_root(graph);
        for child in root.borrow().get_children() {
            if child.borrow().get_category() == category::IMPLEMENTATION {
                let impl_graph = child
                    .borrow()
                    .get_attribute("nodegraph")
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                if impl_graph == graph_name {
                    // This implementation points to our graph.
                    if let Some(nd_attr) = child
                        .borrow()
                        .get_attribute(NODE_DEF_ATTRIBUTE)
                        .map(|s| s.to_string())
                    {
                        if let Some(nd) = doc.get_node_def(&nd_attr) {
                            return Some(nd);
                        }
                    }
                }
            }
        }
    }
    None
}

/// Get the implementation element for a NodeGraph (C++ NodeGraph::getImplementation).
/// Resolves to the NodeDef and returns its implementation.
pub fn nodegraph_get_implementation(graph: &ElementPtr) -> Option<ElementPtr> {
    let nd = nodegraph_resolve_node_def(graph)?;
    crate::core::definition::get_implementation_for_nodedef(
        &nd,
        &crate::core::Document::from_element(graph)?,
        "",
        false,
    )
}

/// Get all downstream ports that connect to this NodeGraph.
/// C++ NodeGraph::getDownstreamPorts.
pub fn nodegraph_get_downstream_ports(graph: &ElementPtr) -> Vec<ElementPtr> {
    let graph_name = graph.borrow().get_name().to_string();
    let parent = match graph.borrow().get_parent() {
        Some(p) => p,
        None => return Vec::new(),
    };
    let mut result = Vec::new();
    // Walk all children of parent, including other NodeGraphs, recursing one level into
    // containers to find inputs that reference this graph by nodegraph attribute.
    for sibling in parent.borrow().get_children() {
        let cat = sibling.borrow().get_category().to_string();
        if cat == category::DOCUMENT {
            continue;
        }
        // If sibling is a NodeGraph, look at inputs of its child nodes.
        if cat == category::NODE_GRAPH {
            for node in sibling.borrow().get_children() {
                for inp in get_inputs(&node) {
                    if inp
                        .borrow()
                        .get_attribute(NODE_GRAPH_ATTRIBUTE)
                        .map(|s| s == graph_name)
                        .unwrap_or(false)
                    {
                        result.push(inp);
                    }
                }
            }
        } else {
            for inp in get_inputs(&sibling) {
                if inp
                    .borrow()
                    .get_attribute(NODE_GRAPH_ATTRIBUTE)
                    .map(|s| s == graph_name)
                    .unwrap_or(false)
                {
                    result.push(inp);
                }
            }
        }
    }
    result.sort_by(|a, b| a.borrow().get_name().cmp(b.borrow().get_name()));
    result
}

/// Add an interface name binding on a NodeGraph input.
/// C++ NodeGraph::addInterfaceName.
/// - `input_path`: descendant path like "node1/in" to the input element.
/// - `interface_name`: name to give to the exposed interface input.
/// Returns the created/existing interface input element, or None on failure.
pub fn nodegraph_add_interface_name(
    graph: &ElementPtr,
    input_path: &str,
    interface_name: &str,
) -> Option<ElementPtr> {
    // Resolve the descendant input.
    let desc = graph.borrow().get_descendant(input_path)?;
    if desc.borrow().get_category() != category::INPUT {
        return None;
    }
    // Must not be already connected to a node.
    if desc.borrow().has_node_name() {
        return None;
    }

    // Determine target interface element: nodedef if attached, else the graph itself.
    let iface_elem: ElementPtr = nodegraph_resolve_node_def(graph).unwrap_or_else(|| graph.clone());

    // Refuse if name already exists on interface.
    if iface_elem.borrow().get_child(interface_name).is_some() {
        return None;
    }

    // Set interfacename on the descriptor input.
    desc.borrow_mut()
        .set_attribute(INTERFACE_NAME_ATTRIBUTE, interface_name);

    // Create or find interface input.
    let iface_input = if let Some(existing) = get_input(&iface_elem, interface_name) {
        existing
    } else {
        let type_str = desc
            .borrow()
            .get_type()
            .map(|s| s.to_string())
            .unwrap_or_default();
        crate::core::element::add_child_of_category(&iface_elem, category::INPUT, interface_name)
            .ok()
            .map(|inp| {
                if !type_str.is_empty() {
                    inp.borrow_mut().set_attribute("type", &type_str);
                }
                inp
            })?
    };

    // Transfer value: move from desc input to interface input, clear from desc.
    if desc.borrow().has_value_string() {
        let val = desc.borrow().get_value_string();
        iface_input.borrow_mut().set_value_string(val);
        desc.borrow_mut().remove_attribute(VALUE_ATTRIBUTE);
    }

    Some(iface_input)
}

/// Remove an interface name binding from a NodeGraph input.
/// C++ NodeGraph::removeInterfaceName.
pub fn nodegraph_remove_interface_name(graph: &ElementPtr, input_path: &str) {
    let desc = match graph.borrow().get_descendant(input_path) {
        Some(d) => d,
        None => return,
    };
    let iface_name = get_interface_name_raw(&desc);
    if iface_name.is_empty() {
        return;
    }
    let iface_elem: ElementPtr = nodegraph_resolve_node_def(graph).unwrap_or_else(|| graph.clone());
    // Restore value from interface input back to the descriptor input, then remove interface port.
    if let Some(iface_inp) = get_input(&iface_elem, &iface_name) {
        if iface_inp.borrow().has_value_string() {
            let val = iface_inp.borrow().get_value_string();
            desc.borrow_mut().set_value_string(val);
        }
        iface_elem.borrow_mut().remove_child(&iface_name);
    }
    // Clear the binding.
    desc.borrow_mut()
        .set_attribute(INTERFACE_NAME_ATTRIBUTE, "");
}

/// Modify the interface name on a NodeGraph input.
/// C++ NodeGraph::modifyInterfaceName.
pub fn nodegraph_modify_interface_name(graph: &ElementPtr, input_path: &str, new_name: &str) {
    let desc = match graph.borrow().get_descendant(input_path) {
        Some(d) => d,
        None => return,
    };
    let old_name = get_interface_name_raw(&desc);
    if old_name == new_name {
        return;
    }
    let iface_elem: ElementPtr = nodegraph_resolve_node_def(graph).unwrap_or_else(|| graph.clone());
    // Rename the interface input child (use rename() to keep child_map in sync).
    if let Some(iface_inp) = get_input(&iface_elem, &old_name) {
        let _ = iface_inp.rename(new_name);
    }
    // Update interfacename on the descriptor input.
    desc.borrow_mut()
        .set_attribute(INTERFACE_NAME_ATTRIBUTE, new_name);
}

/// Get all material-type outputs of a NodeGraph (C++ NodeGraph::getMaterialOutputs).
/// Returns outputs whose type is "material" and whose connected node is also type "material".
pub fn nodegraph_get_material_outputs(graph: &ElementPtr) -> Vec<ElementPtr> {
    get_outputs(graph)
        .into_iter()
        .filter(|out| {
            let type_ok = out
                .borrow()
                .get_type()
                .map(|t| t == crate::core::types::MATERIAL_TYPE_STRING)
                .unwrap_or(false);
            if !type_ok {
                return false;
            }
            // Check that the connected node is also a material node.
            if let Some(node_name) = out.borrow().get_node_name().map(|s| s.to_string()) {
                if let Some(node) = graph.borrow().get_child(&node_name) {
                    return node
                        .borrow()
                        .get_type()
                        .map(|t| t == crate::core::types::MATERIAL_TYPE_STRING)
                        .unwrap_or(false);
                }
            }
            false
        })
        .collect()
}

/// Return nodes in the graph whose type matches the given type string.
/// C++ GraphElement::getNodesOfType.
pub fn get_nodes_of_type(graph: &ElementPtr, type_name: &str) -> Vec<ElementPtr> {
    graph
        .borrow()
        .get_children()
        .iter()
        .filter(|c| {
            c.borrow().get_category() == category::NODE
                && c.borrow()
                    .get_type()
                    .map(|t| t == type_name)
                    .unwrap_or(false)
        })
        .cloned()
        .collect()
}

/// Add (or reuse) a geometry node for a GeomPropDef in this graph.
/// C++ GraphElement::addGeomNode.
///
/// `geom_node_def` must be an element with `geomprop` attribute (GeomPropDef category).
/// The node name is `{name_prefix}_{geom_prop_def_name}`.
pub fn add_geom_node(
    graph: &ElementPtr,
    geom_node_def: &ElementPtr,
    name_prefix: &str,
) -> Option<ElementPtr> {
    use crate::core::element::add_child_of_category;

    let def_name = geom_node_def.borrow().get_name().to_string();
    let geom_node_name = format!("{}_{}", name_prefix, def_name);

    // Return existing node if already present.
    if let Some(existing) = graph.borrow().get_child(&geom_node_name) {
        return Some(existing);
    }

    // `geomprop` attribute is the node category for the geom node.
    let geom_prop_cat = geom_node_def
        .borrow()
        .get_attribute("geomprop")
        .map(|s| s.to_string())
        .unwrap_or_else(|| geom_node_def.borrow().get_name().to_string());
    let node_type = geom_node_def
        .borrow()
        .get_type()
        .map(|s| s.to_string())
        .unwrap_or_default();

    let node = add_child_of_category(graph, category::NODE, &geom_node_name).ok()?;
    node.borrow_mut().set_attribute("node", &geom_prop_cat);
    if !node_type.is_empty() {
        node.borrow_mut().set_attribute("type", &node_type);
    }

    // Copy space/index attributes as input values if present.
    for attr in ["space", "index"] {
        if let Some(val) = geom_node_def
            .borrow()
            .get_attribute(attr)
            .map(|s| s.to_string())
        {
            let type_s = if attr == "index" { "integer" } else { "string" };
            if let Ok(inp) = add_child_of_category(&node, category::INPUT, attr) {
                inp.borrow_mut().set_attribute("type", type_s);
                inp.borrow_mut().set_value_string(&val);
            }
        }
    }
    Some(node)
}

/// Given a connecting element (Input or Output), return the NodeDef output that this element
/// connects to. Only meaningful when the NodeDef has explicitly named outputs.
/// Returns None if there is no corresponding NodeDef output (single-output implicit case).
/// C++ Node::getNodeDefOutput.
pub fn get_node_def_output(
    node: &ElementPtr,
    connecting_element: &ElementPtr,
) -> Option<ElementPtr> {
    // Determine the output name from the connecting element's output attribute.
    let mut output_name = connecting_element
        .borrow()
        .get_attribute(OUTPUT_ATTRIBUTE)
        .map(|s| s.to_string())
        .unwrap_or_default();

    // If the connecting element is an input, resolve via interface input or connected output.
    let cat = connecting_element.borrow().get_category().to_string();
    if cat == category::INPUT {
        // Check interface input first.
        let iface_out_name: Option<String> =
            if let Some(iface) = get_interface_input(connecting_element) {
                // Interface input may have its own connected output string.
                let out = iface
                    .borrow()
                    .get_attribute(OUTPUT_ATTRIBUTE)
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                if !out.is_empty() { Some(out) } else { None }
            } else {
                None
            };
        if let Some(n) = iface_out_name {
            output_name = n;
        }
    }

    if output_name.is_empty() {
        return None;
    }

    // Find on NodeDef.
    let nd = get_node_def(node, "", true)?;
    get_output(&nd, &output_name)
}

/// Rename this node and propagate the new name to all downstream ports (inputs that reference it).
/// C++ Node::setNameGlobal.
pub fn set_name_global(node: &ElementPtr, new_name: &str) -> Result<(), String> {
    // Gather all downstream ports before rename (they reference current name).
    let current_name = node.borrow().get_name().to_string();
    let qualified_name = node.borrow().get_qualified_name(&current_name);
    let ports = if let Some(doc) = crate::core::Document::from_element(node) {
        doc.get_matching_ports(&qualified_name)
    } else {
        get_downstream_ports(node)
    };
    // Rename (use rename() to keep parent child_map in sync).
    node.rename(new_name)
        .map_err(|_| format!("Name '{}' is already taken at this scope", new_name))?;
    let actual_name = node.borrow().get_name().to_string();
    // Update all downstream ports to reference the new name.
    for port in &ports {
        port.borrow_mut().set_node_name(&actual_name);
    }
    Ok(())
}

/// Rename a NodeGraph and propagate the new name to all downstream ports
/// (inputs that reference it via nodegraph attribute).
/// C++ NodeGraph::setNameGlobal uses document->getMatchingPorts for document-wide search.
/// C++ NodeGraph::setNameGlobal.
pub fn nodegraph_set_name_global(graph: &ElementPtr, new_name: &str) -> Result<(), String> {
    // Use document-level matching to find ALL ports referencing this graph by name
    // (C++ calls document->getMatchingPorts(getQualifiedName(getName()))).
    let graph_name = graph.borrow().get_name().to_string();
    let qualified_name = graph.borrow().get_qualified_name(&graph_name);
    let ports = if let Some(doc) = crate::core::document::Document::from_element(graph) {
        doc.get_matching_ports(&qualified_name)
    } else {
        nodegraph_get_downstream_ports(graph)
    };
    // Rename (use rename() to keep parent child_map in sync).
    graph
        .rename(new_name)
        .map_err(|_| format!("Name '{}' is already taken at this scope", new_name))?;
    let actual_name = graph.borrow().get_name().to_string();
    for port in &ports {
        // Update only ports that connect via nodegraph attribute.
        if port.borrow().has_attribute(NODE_GRAPH_ATTRIBUTE) {
            port.borrow_mut().set_node_graph_string(&actual_name);
        }
    }
    Ok(())
}

// ---- validate_node ----

/// Validate a Node element: check category and type are set, and if a NodeDef is
/// found, verify input/output interface consistency.
/// Returns (valid, error_messages). Mirrors C++ Node::validate.
pub fn validate_node(node: &ElementPtr) -> (bool, Vec<String>) {
    let mut errors = Vec::new();
    let mut valid = true;

    let cat = node.borrow().get_category().to_string();
    if cat.is_empty() {
        valid = false;
        errors.push(format!(
            "Node element is missing a category: {}",
            node.borrow().get_name()
        ));
    }

    let has_type = node.borrow().has_type();
    if !has_type {
        valid = false;
        errors.push(format!(
            "Node element is missing a type: {}",
            node.borrow().get_name()
        ));
    }

    // Check against nodedef if available.
    if let Some(nd) = get_node_def(node, "", true) {
        // Verify input types match.
        let node_inputs = get_inputs(node);
        for inp in &node_inputs {
            let inp_name = inp.borrow().get_name().to_string();
            let nd_inp = get_input(&nd, &inp_name);
            if let Some(nd_inp) = nd_inp {
                let inp_type = inp
                    .borrow()
                    .get_type()
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                let nd_type = nd_inp
                    .borrow()
                    .get_type()
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                if !inp_type.is_empty() && !nd_type.is_empty() && inp_type != nd_type {
                    valid = false;
                    errors.push(format!(
                        "Node input '{}' type '{}' doesn't match nodedef type '{}': {}",
                        inp_name,
                        inp_type,
                        nd_type,
                        node.borrow().get_name()
                    ));
                }
            }
        }

        // Check output type consistency.
        let nd_outputs = get_outputs(&nd);
        let node_type = node
            .borrow()
            .get_type()
            .map(|s| s.to_string())
            .unwrap_or_default();
        let multi_output = crate::core::types::MULTI_OUTPUT_TYPE_STRING;
        if nd_outputs.len() > 1 {
            if node_type != multi_output {
                valid = false;
                errors.push(format!(
                    "Node type is not 'multioutput' for node with multiple outputs: {}",
                    node.borrow().get_name()
                ));
            }
        } else if nd_outputs.len() == 1 {
            let out_type = nd_outputs[0]
                .borrow()
                .get_type()
                .map(|s| s.to_string())
                .unwrap_or_default();
            if !out_type.is_empty() && !node_type.is_empty() && node_type != out_type {
                valid = false;
                errors.push(format!(
                    "Node type '{}' does not match output port type '{}': {}",
                    node_type,
                    out_type,
                    node.borrow().get_name()
                ));
            }
        }
    }

    (valid, errors)
}

// ---- validate_node_graph ----

/// Validate a NodeGraph: check that all port connections resolve to existing nodes,
/// and that output types match connected node output types.
/// Returns (valid, error_messages). Mirrors C++ NodeGraph::validate.
pub fn validate_node_graph(graph: &ElementPtr) -> (bool, Vec<String>) {
    fn infer_connected_output_type(node: &ElementPtr, output_name: Option<&str>) -> Option<String> {
        if let Some(name) = output_name.filter(|name| !name.is_empty()) {
            if let Some(output) = get_output(node, name) {
                if let Some(output_type) = output.borrow().get_type() {
                    return Some(output_type.to_string());
                }
            }
        }
        if let Some(value_input) = get_input(node, "value") {
            if let Some(input_type) = value_input.borrow().get_type() {
                return Some(input_type.to_string());
            }
        }
        node.borrow().get_type().map(|s| s.to_string())
    }

    let mut errors = Vec::new();
    let mut valid = true;

    let children: Vec<ElementPtr> = graph.borrow().get_children().to_vec();

    for child in &children {
        let cat = child.borrow().get_category().to_string();

        // Check outputs of the graph: node connections must exist.
        if cat == category::OUTPUT {
            if let Some(node_name) = child.borrow().get_node_name().map(|s| s.to_string()) {
                let connected_node = graph.borrow().get_child(&node_name);
                if connected_node.is_none() {
                    valid = false;
                    errors.push(format!(
                        "Output '{}' connects to missing node '{}' in graph '{}'",
                        child.borrow().get_name(),
                        node_name,
                        graph.borrow().get_name()
                    ));
                } else if let Some(out_type) = child.borrow().get_type().map(|s| s.to_string()) {
                    let connected_output = child
                        .borrow()
                        .get_attribute(OUTPUT_ATTRIBUTE)
                        .map(|s| s.to_string());
                    let connected_type = infer_connected_output_type(
                        &connected_node.expect("checked is_some"),
                        connected_output.as_deref(),
                    );
                    if let Some(connected_type) = connected_type {
                        if connected_type != out_type {
                            valid = false;
                            errors.push(format!(
                                "Output '{}' type '{}' does not match connected node output type '{}' in graph '{}'",
                                child.borrow().get_name(),
                                out_type,
                                connected_type,
                                graph.borrow().get_name()
                            ));
                        }
                    }
                }
            }
            continue;
        }

        // Check inputs of each node-like child inside the graph.
        let node_inputs: Vec<ElementPtr> = child
            .borrow()
            .get_children()
            .iter()
            .filter(|c| c.borrow().get_category() == category::INPUT)
            .cloned()
            .collect();
        if !node_inputs.is_empty() {
            for inp in &node_inputs {
                // If this input has nodename, verify the referenced node exists.
                if let Some(src_name) = inp.borrow().get_node_name().map(|s| s.to_string()) {
                    if graph.borrow().get_child(&src_name).is_none() {
                        valid = false;
                        errors.push(format!(
                            "Input '{}' of node '{}' connects to missing node '{}' in graph '{}'",
                            inp.borrow().get_name(),
                            child.borrow().get_name(),
                            src_name,
                            graph.borrow().get_name()
                        ));
                    }
                }
            }
        }
    }

    (valid, errors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::document::create_document;
    use crate::core::element::add_child_of_category;

    /// Build a minimal document with a nodegraph for testing.
    fn make_doc_with_graph() -> (crate::core::document::Document, ElementPtr) {
        let doc = create_document();
        let root = doc.get_root();
        let graph =
            add_child_of_category(&root, category::NODE_GRAPH, "testGraph").expect("add nodegraph");
        (doc, graph)
    }

    #[test]
    fn test_nodegraph_set_get_node_def_name() {
        let (_doc, graph) = make_doc_with_graph();
        nodegraph_set_node_def(&graph, "ND_foo");
        assert_eq!(
            nodegraph_get_node_def_name(&graph),
            Some("ND_foo".to_string())
        );
        nodegraph_set_node_def(&graph, "");
        assert!(nodegraph_get_node_def_name(&graph).is_none());
    }

    #[test]
    fn test_nodegraph_add_remove_interface_name() {
        let (_doc, graph) = make_doc_with_graph();
        // Add a child node with an input inside the graph.
        let node = add_child_of_category(&graph, category::NODE, "node1").unwrap();
        node.borrow_mut().set_attribute("node", "add");
        node.borrow_mut().set_attribute("type", "float");
        let inp = add_child_of_category(&node, category::INPUT, "in1").unwrap();
        inp.borrow_mut().set_attribute("type", "float");
        inp.borrow_mut().set_value_string("0.5");

        // Add interface name — path is "node1/in1".
        let iface_inp = nodegraph_add_interface_name(&graph, "node1/in1", "in1_exposed");
        assert!(iface_inp.is_some(), "interface input should be created");
        let iface = iface_inp.unwrap();
        assert_eq!(iface.borrow().get_name(), "in1_exposed");
        // Value moved to interface input, cleared from descriptor.
        assert_eq!(iface.borrow().get_value_string(), "0.5");
        assert!(
            !inp.borrow().has_value_string(),
            "value should be cleared from inner input"
        );
        // interfacename set on inner input.
        assert_eq!(get_interface_name_raw(&inp), "in1_exposed");

        // Modify interface name.
        nodegraph_modify_interface_name(&graph, "node1/in1", "in1_renamed");
        assert_eq!(get_interface_name_raw(&inp), "in1_renamed");
        // Old name should be gone from graph, new present.
        assert!(get_input(&graph, "in1_exposed").is_none());
        assert!(get_input(&graph, "in1_renamed").is_some());

        // Remove interface name.
        nodegraph_remove_interface_name(&graph, "node1/in1");
        // Value restored to inner input.
        assert!(inp.borrow().has_value_string());
        // interfacename cleared.
        assert!(get_interface_name_raw(&inp).is_empty());
    }

    #[test]
    fn test_nodegraph_get_material_outputs() {
        let (_doc, graph) = make_doc_with_graph();
        // Add a material node and an output referencing it.
        let mat_node = add_child_of_category(&graph, category::NODE, "mat1").unwrap();
        mat_node
            .borrow_mut()
            .set_attribute("type", crate::core::types::MATERIAL_TYPE_STRING);
        let out = add_child_of_category(&graph, category::OUTPUT, "out1").unwrap();
        out.borrow_mut()
            .set_attribute("type", crate::core::types::MATERIAL_TYPE_STRING);
        out.borrow_mut().set_node_name("mat1");

        let mat_outs = nodegraph_get_material_outputs(&graph);
        assert_eq!(mat_outs.len(), 1);
        assert_eq!(mat_outs[0].borrow().get_name(), "out1");
    }

    #[test]
    fn test_get_nodes_of_type() {
        let (_doc, graph) = make_doc_with_graph();
        let n1 = add_child_of_category(&graph, category::NODE, "n1").unwrap();
        n1.borrow_mut().set_attribute("type", "float");
        let n2 = add_child_of_category(&graph, category::NODE, "n2").unwrap();
        n2.borrow_mut().set_attribute("type", "color3");
        let n3 = add_child_of_category(&graph, category::NODE, "n3").unwrap();
        n3.borrow_mut().set_attribute("type", "float");

        let floats = get_nodes_of_type(&graph, "float");
        assert_eq!(floats.len(), 2);
        let colors = get_nodes_of_type(&graph, "color3");
        assert_eq!(colors.len(), 1);
    }

    #[test]
    fn test_add_geom_node() {
        let (_doc, graph) = make_doc_with_graph();
        // Build a fake GeomPropDef-like element.
        let root = crate::core::element::get_root(&graph);
        let gp_def = add_child_of_category(&root, category::GEOM_PROP_DEF, "Nworld").unwrap();
        gp_def.borrow_mut().set_attribute("geomprop", "normalworld");
        gp_def.borrow_mut().set_attribute("type", "vector3");
        gp_def.borrow_mut().set_attribute("space", "world");

        let geom_node = add_geom_node(&graph, &gp_def, "geomNode");
        assert!(geom_node.is_some());
        let gn = geom_node.unwrap();
        assert_eq!(gn.borrow().get_name(), "geomNode_Nworld");
        assert_eq!(gn.borrow().get_attribute("node"), Some("normalworld"));
        // Second call returns the same node.
        let geom_node2 = add_geom_node(&graph, &gp_def, "geomNode");
        assert!(geom_node2.is_some());
        assert_eq!(geom_node2.unwrap().borrow().get_name(), "geomNode_Nworld");
    }

    #[test]
    fn test_set_name_global_updates_ports() {
        let (_doc, graph) = make_doc_with_graph();
        // Create node1 and node2, with node2's input connected to node1.
        let node1 = add_child_of_category(&graph, category::NODE, "node1").unwrap();
        node1.borrow_mut().set_attribute("node", "add");
        node1.borrow_mut().set_attribute("type", "float");

        let node2 = add_child_of_category(&graph, category::NODE, "node2").unwrap();
        node2.borrow_mut().set_attribute("node", "add");
        node2.borrow_mut().set_attribute("type", "float");
        let inp = add_child_of_category(&node2, category::INPUT, "in1").unwrap();
        inp.borrow_mut().set_node_name("node1");

        // Graph output also connects to node1.
        let out = add_child_of_category(&graph, category::OUTPUT, "out1").unwrap();
        out.borrow_mut().set_node_name("node1");

        // Rename node1 -> node1_renamed.
        set_name_global(&node1, "node1_renamed").expect("rename ok");

        assert_eq!(node1.borrow().get_name(), "node1_renamed");
        assert_eq!(inp.borrow().get_node_name(), Some("node1_renamed"));
        assert_eq!(out.borrow().get_node_name(), Some("node1_renamed"));
    }

    #[test]
    fn test_nodegraph_set_name_global() {
        let (_doc, _graph) = make_doc_with_graph();
        let root = _doc.get_root();
        // Add a second graph to act as consumer.
        let graph2 = add_child_of_category(&root, category::NODE_GRAPH, "graph2").unwrap();
        let node_in_g2 = add_child_of_category(&graph2, category::NODE, "mixer").unwrap();
        node_in_g2.borrow_mut().set_attribute("node", "mix");
        let inp = add_child_of_category(&node_in_g2, category::INPUT, "fg").unwrap();
        // Connect to testGraph via nodegraph attribute.
        inp.borrow_mut().set_node_graph_string("testGraph");

        // Rename testGraph.
        let graph = root.borrow().get_child("testGraph").unwrap();
        nodegraph_set_name_global(&graph, "testGraph_renamed").expect("rename ok");

        assert_eq!(graph.borrow().get_name(), "testGraph_renamed");
        // The downstream port should now reference the new name.
        assert_eq!(
            inp.borrow().get_attribute(NODE_GRAPH_ATTRIBUTE),
            Some("testGraph_renamed")
        );
    }

    #[test]
    fn test_get_node_def_output_named() {
        let mut doc = create_document();
        let root = doc.get_root();
        // NodeDef with an explicit named output.
        let nd = doc.add_node_def("ND_multi", "", "multiout").unwrap();
        let _out_a = add_child_of_category(&nd, category::OUTPUT, "outA").unwrap();
        _out_a.borrow_mut().set_attribute("type", "float");
        nd.borrow_mut().set_attribute("node", "multiout");
        nd.borrow_mut()
            .set_attribute("type", crate::core::types::MULTI_OUTPUT_TYPE_STRING);

        // Node instance inside a graph.
        let graph = add_child_of_category(&root, category::NODE_GRAPH, "g").unwrap();
        let node = add_child_of_category(&graph, category::NODE, "mynode").unwrap();
        node.borrow_mut().set_attribute("node", "multiout");
        node.borrow_mut()
            .set_attribute("type", crate::core::types::MULTI_OUTPUT_TYPE_STRING);
        node.borrow_mut()
            .set_attribute(NODE_DEF_ATTRIBUTE, "ND_multi");

        // Connecting input that requests output "outA".
        let consumer = add_child_of_category(&graph, category::NODE, "consumer").unwrap();
        consumer.borrow_mut().set_attribute("node", "add");
        consumer.borrow_mut().set_attribute("type", "float");
        let inp = add_child_of_category(&consumer, category::INPUT, "in1").unwrap();
        inp.borrow_mut().set_node_name("mynode");
        inp.borrow_mut().set_attribute(OUTPUT_ATTRIBUTE, "outA");

        let result = get_node_def_output(&node, &inp);
        assert!(result.is_some(), "should find named output outA");
        assert_eq!(result.unwrap().borrow().get_name(), "outA");

        // Connecting element without an output string returns None.
        let inp_no_out = add_child_of_category(&consumer, category::INPUT, "in2").unwrap();
        inp_no_out.borrow_mut().set_node_name("mynode");
        let result2 = get_node_def_output(&node, &inp_no_out);
        assert!(result2.is_none(), "no output string -> None");
    }

    #[test]
    fn test_validate_node_basic() {
        use crate::core::element::Element;
        let root_ptr = ElementPtr::new(Element::new(None, category::DOCUMENT, ""));

        let node = add_child_of_category(&root_ptr, category::NODE, "mynode").unwrap();
        node.borrow_mut().set_attribute("type", "float");
        node.borrow_mut().set_attribute("node", "constant");

        let (valid, _errors) = validate_node(&node);
        // No nodedef available, so category undeclared check passes (valid with warning).
        assert!(valid, "basic node with type should pass");
    }

    #[test]
    fn test_validate_node_missing_type() {
        use crate::core::element::Element;
        let root_ptr = ElementPtr::new(Element::new(None, category::DOCUMENT, ""));

        let node = add_child_of_category(&root_ptr, category::NODE, "mynode").unwrap();
        // No type set.
        let (valid, errors) = validate_node(&node);
        assert!(!valid, "node without type should fail");
        assert!(
            errors.iter().any(|e| e.contains("type")),
            "should mention missing type"
        );
    }

    #[test]
    fn test_validate_node_graph_valid() {
        use crate::core::element::Element;
        let root_ptr = ElementPtr::new(Element::new(None, category::DOCUMENT, ""));
        let graph = add_child_of_category(&root_ptr, category::NODE_GRAPH, "g1").unwrap();

        let n1 = add_child_of_category(&graph, category::NODE, "n1").unwrap();
        n1.borrow_mut().set_attribute("type", "float");
        n1.borrow_mut().set_attribute("node", "constant");

        let out = add_child_of_category(&graph, category::OUTPUT, "out").unwrap();
        out.borrow_mut().set_attribute("type", "float");
        out.borrow_mut().set_node_name("n1");

        let (valid, errors) = validate_node_graph(&graph);
        assert!(valid, "valid graph should pass: {:?}", errors);
    }

    #[test]
    fn test_validate_node_graph_dangling() {
        use crate::core::element::Element;
        let root_ptr = ElementPtr::new(Element::new(None, category::DOCUMENT, ""));
        let graph = add_child_of_category(&root_ptr, category::NODE_GRAPH, "g1").unwrap();

        let out = add_child_of_category(&graph, category::OUTPUT, "out").unwrap();
        out.borrow_mut().set_attribute("type", "float");
        // Points to missing node.
        out.borrow_mut().set_node_name("nonexistent");

        let (valid, errors) = validate_node_graph(&graph);
        assert!(!valid, "dangling connection should fail");
        assert!(
            errors.iter().any(|e| e.contains("nonexistent")),
            "error should mention missing node"
        );
    }
}
