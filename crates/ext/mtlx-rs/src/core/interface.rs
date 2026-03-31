//! InterfaceElement, NodeDef, NodeGraph, Implementation — interface helpers.

use crate::core::element::{
    ElementPtr, INTERFACE_NAME_ATTRIBUTE, NODE_ATTRIBUTE, NODE_DEF_ATTRIBUTE, NODE_GRAPH_ATTRIBUTE,
    NODE_NAME_ATTRIBUTE, OUTPUT_ATTRIBUTE, VALUE_ATTRIBUTE, add_child_of_category, category,
};
use crate::core::node::{get_input, get_inputs, get_output, get_outputs};
use crate::core::traversal::traverse_inheritance;
use std::collections::HashSet;

/// Is NodeDef, NodeGraph, or Implementation
pub fn is_interface_element(elem: &ElementPtr) -> bool {
    let cat = elem.borrow().get_category().to_string();
    cat == category::NODEDEF || cat == category::NODE_GRAPH || cat == category::IMPLEMENTATION
}

/// Get nodedef attribute (InterfaceElement)
pub fn get_node_def_string(elem: &ElementPtr) -> Option<String> {
    elem.borrow()
        .get_attribute(NODE_DEF_ATTRIBUTE)
        .map(|s| s.to_string())
}

/// Set nodedef attribute
pub fn set_node_def_string(elem: &ElementPtr, node_def: impl Into<String>) {
    elem.borrow_mut()
        .set_attribute(NODE_DEF_ATTRIBUTE, node_def.into());
}

/// Has nodedef attribute
pub fn has_node_def_string(elem: &ElementPtr) -> bool {
    elem.borrow().has_attribute(NODE_DEF_ATTRIBUTE)
}

/// Get active inputs, walking the inheritance chain. C++ InterfaceElement::getActiveInputs.
pub fn get_active_inputs(elem: &ElementPtr) -> Vec<ElementPtr> {
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for ancestor in traverse_inheritance(elem.clone()).filter_map(|r| r.ok()) {
        for inp in get_inputs(&ancestor) {
            let name = inp.borrow().get_name().to_string();
            if seen.insert(name) {
                result.push(inp);
            }
        }
    }
    result
}

/// Get active outputs, walking the inheritance chain. C++ InterfaceElement::getActiveOutputs.
pub fn get_active_outputs(elem: &ElementPtr) -> Vec<ElementPtr> {
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for ancestor in traverse_inheritance(elem.clone()).filter_map(|r| r.ok()) {
        for out in get_outputs(&ancestor) {
            let name = out.borrow().get_name().to_string();
            if seen.insert(name) {
                result.push(out);
            }
        }
    }
    result
}

/// Get active input by name, walking inheritance. C++ InterfaceElement::getActiveInput.
pub fn get_active_input(elem: &ElementPtr, name: &str) -> Option<ElementPtr> {
    for ancestor in traverse_inheritance(elem.clone()).filter_map(|r| r.ok()) {
        if let Some(inp) = get_input(&ancestor, name) {
            return Some(inp);
        }
    }
    None
}

/// Get active output by name, walking inheritance. C++ InterfaceElement::getActiveOutput.
pub fn get_active_output(elem: &ElementPtr, name: &str) -> Option<ElementPtr> {
    for ancestor in traverse_inheritance(elem.clone()).filter_map(|r| r.ok()) {
        if let Some(out) = get_output(&ancestor, name) {
            return Some(out);
        }
    }
    None
}

/// NodeDef: get node string (node type, e.g. "standard_surface")
pub fn get_node_string(elem: &ElementPtr) -> Option<String> {
    elem.borrow()
        .get_attribute(NODE_ATTRIBUTE)
        .map(|s| s.to_string())
}

/// NodeDef: set node string
pub fn set_node_string(elem: &ElementPtr, node: impl Into<String>) {
    elem.borrow_mut().set_attribute(NODE_ATTRIBUTE, node.into());
}

/// NodeDef: has node string
pub fn has_node_string(elem: &ElementPtr) -> bool {
    elem.borrow().has_attribute(NODE_ATTRIBUTE)
}

/// Add Input to InterfaceElement (NodeGraph, NodeDef). Matches MaterialX InterfaceElement::addInput.
pub fn add_input(elem: &ElementPtr, name: &str, type_str: &str) -> Result<ElementPtr, String> {
    let inp = add_child_of_category(elem, category::INPUT, name)?;
    if !type_str.is_empty() {
        inp.borrow_mut().set_attribute("type", type_str);
    }
    Ok(inp)
}

/// Remove Input by name from InterfaceElement.
pub fn remove_input(elem: &ElementPtr, name: &str) {
    if get_input(elem, name).is_some() {
        elem.borrow_mut().remove_child(name);
    }
}

/// Add Output to InterfaceElement (NodeGraph, NodeDef). Matches MaterialX InterfaceElement::addOutput.
pub fn add_output(elem: &ElementPtr, name: &str, type_str: &str) -> Result<ElementPtr, String> {
    let out = add_child_of_category(elem, category::OUTPUT, name)?;
    if !type_str.is_empty() {
        out.borrow_mut().set_attribute("type", type_str);
    }
    Ok(out)
}

/// Remove Output by name from InterfaceElement.
pub fn remove_output(elem: &ElementPtr, name: &str) {
    if get_output(elem, name).is_some() {
        elem.borrow_mut().remove_child(name);
    }
}

// --- Token management (C++ InterfaceElement::addToken etc.) ---

/// Add a Token to InterfaceElement.
pub fn add_token(elem: &ElementPtr, name: &str) -> Result<ElementPtr, String> {
    add_child_of_category(elem, category::TOKEN, name)
}

/// Get Token by name.
pub fn get_token(elem: &ElementPtr, name: &str) -> Option<ElementPtr> {
    let child = elem.borrow().get_child(name)?;
    if child.borrow().get_category() == category::TOKEN {
        Some(child)
    } else {
        None
    }
}

/// Get all Token children.
pub fn get_tokens(elem: &ElementPtr) -> Vec<ElementPtr> {
    elem.borrow().get_children_of_category(category::TOKEN)
}

/// Get active tokens (same as get_tokens without inheritance).
pub fn get_active_tokens(elem: &ElementPtr) -> Vec<ElementPtr> {
    get_tokens(elem)
}

/// Remove Token by name.
pub fn remove_token(elem: &ElementPtr, name: &str) {
    if get_token(elem, name).is_some() {
        elem.borrow_mut().remove_child(name);
    }
}

// --- Value helpers ---

/// Set input value as a string (get or create input, set type and value).
pub fn set_input_value(
    elem: &ElementPtr,
    name: &str,
    value: &str,
    type_str: &str,
) -> Result<ElementPtr, String> {
    let inp = if let Some(i) = get_input(elem, name) {
        i
    } else {
        add_input(elem, name, type_str)?
    };
    if !type_str.is_empty() {
        inp.borrow_mut().set_type(type_str);
    }
    inp.borrow_mut().set_attribute(VALUE_ATTRIBUTE, value);
    Ok(inp)
}

/// Get input value as string.
pub fn get_input_value(elem: &ElementPtr, name: &str) -> Option<String> {
    let inp = get_input(elem, name)?;
    inp.borrow()
        .get_attribute(VALUE_ATTRIBUTE)
        .map(|s| s.to_string())
}

/// Set token value.
pub fn set_token_value(elem: &ElementPtr, name: &str, value: &str) -> Result<ElementPtr, String> {
    let tok = if let Some(t) = get_token(elem, name) {
        t
    } else {
        add_token(elem, name)?
    };
    tok.borrow_mut().set_attribute(VALUE_ATTRIBUTE, value);
    Ok(tok)
}

/// Get token value.
pub fn get_token_value(elem: &ElementPtr, name: &str) -> Option<String> {
    let tok = get_token(elem, name)?;
    tok.borrow()
        .get_attribute(VALUE_ATTRIBUTE)
        .map(|s| s.to_string())
}

/// Get the declaration (NodeDef) for an InterfaceElement.
/// Looks up the nodedef attribute string, then resolves it in the document.
pub fn get_declaration(elem: &ElementPtr) -> Option<ElementPtr> {
    let nd_name = get_node_def_string(elem)?;
    let doc = crate::core::Document::from_element(elem)?;
    doc.get_node_def(&nd_name)
}

/// Check if two InterfaceElements have exactly matching inputs.
pub fn has_exact_input_match(elem1: &ElementPtr, elem2: &ElementPtr) -> bool {
    let inputs1 = get_active_inputs(elem1);
    let inputs2 = get_active_inputs(elem2);
    if inputs1.len() != inputs2.len() {
        return false;
    }
    for inp1 in &inputs1 {
        let name = inp1.borrow().get_name().to_string();
        let found = inputs2.iter().any(|inp2| {
            let n2 = inp2.borrow().get_name().to_string();
            n2 == name
        });
        if !found {
            return false;
        }
    }
    true
}

/// Set input to connect to output (MaterialX InterfaceElement::setConnectedOutput).
/// Gets or creates input, sets type from output, connects to output.
pub fn set_connected_output(
    elem: &ElementPtr,
    input_name: &str,
    output: Option<&ElementPtr>,
) -> Result<ElementPtr, String> {
    use crate::core::node::set_connected_output as set_input_connected_output;
    let inp = if let Some(i) = get_input(elem, input_name) {
        i
    } else {
        let type_str = output
            .as_ref()
            .and_then(|o| o.borrow().get_type().map(|s| s.to_string()))
            .unwrap_or_else(|| crate::core::types::DEFAULT_TYPE_STRING.to_string());
        add_input(elem, input_name, &type_str)?
    };
    if let Some(out) = output {
        let ty = out
            .borrow()
            .get_type()
            .map(|s| s.to_string())
            .unwrap_or_else(|| crate::core::types::DEFAULT_TYPE_STRING.to_string());
        inp.borrow_mut().set_type(&ty);
    }
    set_input_connected_output(&inp, output);
    Ok(inp)
}

/// Get a specific value element (Input, Output, or Token) by name, walking inheritance.
/// C++ InterfaceElement::getActiveValueElement.
pub fn get_active_value_element(elem: &ElementPtr, name: &str) -> Option<ElementPtr> {
    for ancestor in traverse_inheritance(elem.clone()).filter_map(|r| r.ok()) {
        if let Some(child) = ancestor.borrow().get_child(name) {
            let cat = child.borrow().get_category().to_string();
            if cat == category::INPUT || cat == category::OUTPUT || cat == category::TOKEN {
                return Some(child);
            }
        }
    }
    None
}

/// Return all value elements (Inputs, Outputs, Tokens) belonging to this interface,
/// walking the inheritance chain. C++ InterfaceElement::getActiveValueElements.
pub fn get_active_value_elements(elem: &ElementPtr) -> Vec<ElementPtr> {
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for ancestor in traverse_inheritance(elem.clone()).filter_map(|r| r.ok()) {
        for child in ancestor.borrow().get_children() {
            let cat = child.borrow().get_category().to_string();
            if cat == category::INPUT || cat == category::OUTPUT || cat == category::TOKEN {
                let name = child.borrow().get_name().to_string();
                if seen.insert(name) {
                    result.push(child.clone());
                }
            }
        }
    }
    result
}

/// Set version as major.minor pair (C++ InterfaceElement::setVersionIntegers).
/// Serialises as "major.minor" into the `version` attribute.
pub fn set_version_integers(elem: &ElementPtr, major: i32, minor: i32) {
    elem.borrow_mut().set_attribute(
        crate::core::element::VERSION_ATTRIBUTE,
        format!("{}.{}", major, minor),
    );
}

/// Parse version from `version` attribute as (major, minor) integers.
/// C++ InterfaceElement::getVersionIntegers.
pub fn get_version_integers(elem: &ElementPtr) -> (i32, i32) {
    let ver = elem
        .borrow()
        .get_attribute(crate::core::element::VERSION_ATTRIBUTE)
        .unwrap_or("")
        .to_string();
    if ver.is_empty() {
        return (0, 0);
    }
    let parts: Vec<&str> = ver.splitn(2, '.').collect();
    let major = parts
        .first()
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(0);
    let minor = parts
        .get(1)
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(0);
    (major, minor)
}

/// Clear all inputs, outputs, and tokens from this interface element.
/// Preserves other children and all attributes.
/// C++ InterfaceElement::clearContent (override clears inputs/outputs/tokens).
pub fn clear_interface_content(elem: &ElementPtr) {
    let to_remove: Vec<String> = elem
        .borrow()
        .get_children()
        .iter()
        .filter(|c| {
            let cat = c.borrow().get_category().to_string();
            cat == category::INPUT || cat == category::OUTPUT || cat == category::TOKEN
        })
        .map(|c| c.borrow().get_name().to_string())
        .collect();
    for name in to_remove {
        elem.borrow_mut().remove_child(&name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::document::create_document;
    use crate::core::element::add_child_of_category;

    #[test]
    fn test_version_integers_roundtrip() {
        let doc = create_document();
        let root = doc.get_root();
        let nd = add_child_of_category(&root, category::NODEDEF, "ND_test").unwrap();
        set_version_integers(&nd, 2, 7);
        assert_eq!(get_version_integers(&nd), (2, 7));
    }

    #[test]
    fn test_version_integers_zero() {
        let doc = create_document();
        let root = doc.get_root();
        let nd = add_child_of_category(&root, category::NODEDEF, "ND_test2").unwrap();
        // No version set yet.
        assert_eq!(get_version_integers(&nd), (0, 0));
        set_version_integers(&nd, 1, 39);
        assert_eq!(get_version_integers(&nd), (1, 39));
    }

    #[test]
    fn test_get_active_value_elements() {
        let doc = create_document();
        let root = doc.get_root();
        let nd = add_child_of_category(&root, category::NODEDEF, "ND_ve").unwrap();
        add_child_of_category(&nd, category::INPUT, "in1").unwrap();
        add_child_of_category(&nd, category::OUTPUT, "out").unwrap();
        add_child_of_category(&nd, category::TOKEN, "tok").unwrap();
        // Non-value-element child.
        add_child_of_category(&nd, category::INPUT, "in2").unwrap();

        let elems = get_active_value_elements(&nd);
        assert_eq!(elems.len(), 4); // in1, out, tok, in2
        assert!(get_active_value_element(&nd, "in1").is_some());
        assert!(get_active_value_element(&nd, "out").is_some());
        assert!(get_active_value_element(&nd, "tok").is_some());
        assert!(get_active_value_element(&nd, "missing").is_none());
    }

    #[test]
    fn test_clear_interface_content() {
        let doc = create_document();
        let root = doc.get_root();
        let nd = add_child_of_category(&root, category::NODEDEF, "ND_clr").unwrap();
        add_child_of_category(&nd, category::INPUT, "in1").unwrap();
        add_child_of_category(&nd, category::OUTPUT, "out").unwrap();
        add_child_of_category(&nd, category::TOKEN, "tok").unwrap();
        assert_eq!(nd.borrow().get_children().len(), 3);

        clear_interface_content(&nd);
        assert_eq!(nd.borrow().get_children().len(), 0);
        // Attributes preserved.
        assert_eq!(nd.borrow().get_name(), "ND_clr");
    }

    #[test]
    fn test_hint_get_set_has() {
        let doc = create_document();
        let root = doc.get_root();
        let nd = add_child_of_category(&root, category::NODEDEF, "ND_hint").unwrap();
        let inp = add_child_of_category(&nd, category::INPUT, "opacity").unwrap();
        // No hint initially.
        assert!(!has_hint(&inp));
        assert_eq!(get_hint(&inp), "");
        // Set hint.
        set_hint(&inp, "opacity");
        assert!(has_hint(&inp));
        assert_eq!(get_hint(&inp), "opacity");
        // Overwrite.
        set_hint(&inp, "transparency");
        assert_eq!(get_hint(&inp), "transparency");
    }

    #[test]
    fn test_set_get_connected_interface_name() {
        let doc = create_document();
        let root = doc.get_root();
        // Create a nodegraph with an interface input.
        let graph = add_child_of_category(&root, category::NODE_GRAPH, "g").unwrap();
        let iface_inp = add_child_of_category(&graph, category::INPUT, "base_color").unwrap();
        iface_inp.borrow_mut().set_attribute("type", "color3");

        // Node inside graph with an input that can bind to the interface.
        let node = add_child_of_category(&graph, category::NODE, "n1").unwrap();
        let inp = add_child_of_category(&node, category::INPUT, "in").unwrap();
        inp.borrow_mut().set_attribute("type", "color3");
        inp.borrow_mut().set_value_string("0.5 0.5 0.5");

        // Connect to interface input.
        set_connected_interface_name(&inp, "base_color");
        assert_eq!(get_connected_interface_name(&inp), "base_color");
        // Value should be cleared.
        assert!(!inp.borrow().has_value_string());

        // Disconnect.
        set_connected_interface_name(&inp, "");
        assert_eq!(get_connected_interface_name(&inp), "");
    }

    #[test]
    fn test_set_connected_interface_name_nonexistent() {
        // Trying to bind to a non-existent interface name should do nothing.
        let doc = create_document();
        let root = doc.get_root();
        let graph = add_child_of_category(&root, category::NODE_GRAPH, "g2").unwrap();
        let node = add_child_of_category(&graph, category::NODE, "n1").unwrap();
        let inp = add_child_of_category(&node, category::INPUT, "in").unwrap();
        // No interface input named "ghost" exists.
        set_connected_interface_name(&inp, "ghost");
        assert_eq!(get_connected_interface_name(&inp), "");
    }

    #[test]
    fn test_get_default_geom_prop() {
        let mut doc = create_document();
        let root = doc.get_root();
        // Add a GeomPropDef to the document.
        doc.add_geom_prop_def("Nworld", "normalworld").unwrap();
        // NodeDef with an input that has defaultgeomprop.
        let nd = add_child_of_category(&root, category::NODEDEF, "ND_gpd").unwrap();
        let inp = add_child_of_category(&nd, category::INPUT, "normal").unwrap();
        inp.borrow_mut()
            .set_attribute(crate::core::element::DEFAULT_GEOM_PROP_ATTRIBUTE, "Nworld");

        let gpd = get_default_geom_prop(&inp);
        assert!(gpd.is_some(), "should resolve GeomPropDef");
        assert_eq!(gpd.unwrap().borrow().get_name(), "Nworld");

        // Input without defaultgeomprop.
        let inp2 = add_child_of_category(&nd, category::INPUT, "color").unwrap();
        assert!(get_default_geom_prop(&inp2).is_none());
    }

    #[test]
    fn test_has_upstream_cycle_no_cycle() {
        let doc = create_document();
        let root = doc.get_root();
        let graph = add_child_of_category(&root, category::NODE_GRAPH, "g").unwrap();
        let n1 = add_child_of_category(&graph, category::NODE, "n1").unwrap();
        n1.borrow_mut().set_attribute("type", "float");
        let n2 = add_child_of_category(&graph, category::NODE, "n2").unwrap();
        n2.borrow_mut().set_attribute("type", "float");
        let inp2 = add_child_of_category(&n2, category::INPUT, "in1").unwrap();
        inp2.borrow_mut().set_node_name("n1");

        let out = add_child_of_category(&graph, category::OUTPUT, "out").unwrap();
        out.borrow_mut().set_node_name("n2");

        assert!(!has_upstream_cycle(&out), "linear chain has no cycle");
    }

    #[test]
    fn test_has_upstream_cycle_with_cycle() {
        let doc = create_document();
        let root = doc.get_root();
        let graph = add_child_of_category(&root, category::NODE_GRAPH, "g2").unwrap();
        // n1 -> n2 -> n1 (cycle)
        let n1 = add_child_of_category(&graph, category::NODE, "n1").unwrap();
        n1.borrow_mut().set_attribute("type", "float");
        let n2 = add_child_of_category(&graph, category::NODE, "n2").unwrap();
        n2.borrow_mut().set_attribute("type", "float");
        let inp1 = add_child_of_category(&n1, category::INPUT, "in1").unwrap();
        inp1.borrow_mut().set_node_name("n2"); // n1 depends on n2
        let inp2 = add_child_of_category(&n2, category::INPUT, "in1").unwrap();
        inp2.borrow_mut().set_node_name("n1"); // n2 depends on n1 -> cycle

        let out = add_child_of_category(&graph, category::OUTPUT, "out").unwrap();
        out.borrow_mut().set_node_name("n1");

        assert!(has_upstream_cycle(&out), "cycle should be detected");
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Input-specific methods (C++ Input class)
// ─────────────────────────────────────────────────────────────────────────────

/// Set the connected interface name on an input, validating that it exists on the parent graph.
/// Clears other connection attributes (value, output, nodegraph, nodename).
/// If name is empty, removes the interfacename attribute (disconnects).
/// C++ Input::setConnectedInterfaceName.
pub fn set_connected_interface_name(input: &ElementPtr, interface_name: &str) {
    use crate::core::element::{
        INTERFACE_NAME_ATTRIBUTE, NODE_GRAPH_ATTRIBUTE, NODE_NAME_ATTRIBUTE, OUTPUT_ATTRIBUTE,
        VALUE_ATTRIBUTE,
    };
    if !interface_name.is_empty() {
        // Walk up to find the parent GraphElement and verify the interface input exists.
        let mut current = input.borrow().get_parent();
        while let Some(p) = current {
            let cat = p.borrow().get_category().to_string();
            if cat == category::NODE_GRAPH || cat == category::DOCUMENT {
                // Only set if the interface input exists on the graph.
                if get_input(&p, interface_name).is_some() {
                    input
                        .borrow_mut()
                        .set_attribute(INTERFACE_NAME_ATTRIBUTE, interface_name);
                    input.borrow_mut().remove_attribute(VALUE_ATTRIBUTE);
                    input.borrow_mut().remove_attribute(OUTPUT_ATTRIBUTE);
                    input.borrow_mut().remove_attribute(NODE_GRAPH_ATTRIBUTE);
                    input.borrow_mut().remove_attribute(NODE_NAME_ATTRIBUTE);
                }
                break;
            }
            current = p.borrow().get_parent();
        }
    } else {
        input
            .borrow_mut()
            .remove_attribute(INTERFACE_NAME_ATTRIBUTE);
    }
}

/// Return the current interface name string for this input (empty if not set).
/// C++ Input::getInterfaceName (via attribute).
pub fn get_connected_interface_name(input: &ElementPtr) -> String {
    input
        .borrow()
        .get_attribute(INTERFACE_NAME_ATTRIBUTE)
        .map(|s| s.to_string())
        .unwrap_or_default()
}

/// Return true if this input has a hint attribute.
/// C++ Input::hasHint.
pub fn has_hint(input: &ElementPtr) -> bool {
    input
        .borrow()
        .has_attribute(crate::core::element::HINT_ATTRIBUTE)
}

/// Return the hint string for this input.
/// C++ Input::getHint.
pub fn get_hint(input: &ElementPtr) -> String {
    input
        .borrow()
        .get_attribute(crate::core::element::HINT_ATTRIBUTE)
        .map(|s| s.to_string())
        .unwrap_or_default()
}

/// Set the hint string on this input.
/// C++ Input::setHint.
pub fn set_hint(input: &ElementPtr, hint: &str) {
    input
        .borrow_mut()
        .set_attribute(crate::core::element::HINT_ATTRIBUTE, hint);
}

/// Return the GeomPropDef element referenced by defaultgeomprop attribute on this input.
/// Walks to the document root and resolves the GeomPropDef by name.
/// C++ Input::getDefaultGeomProp.
pub fn get_default_geom_prop(input: &ElementPtr) -> Option<ElementPtr> {
    let geom_prop_name = input
        .borrow()
        .get_attribute(crate::core::element::DEFAULT_GEOM_PROP_ATTRIBUTE)
        .map(|s| s.to_string())?;
    if geom_prop_name.is_empty() {
        return None;
    }
    let doc = crate::core::Document::from_element(input)?;
    doc.get_geom_prop_def(&geom_prop_name)
}

// ─────────────────────────────────────────────────────────────────────────────
// Output-specific methods (C++ Output class)
// ─────────────────────────────────────────────────────────────────────────────

/// Return true if a cycle exists in any upstream path from this output.
/// Detects cycles by performing a depth-first traversal following node connections
/// and stopping if any node is visited twice.
/// C++ Output::hasUpstreamCycle.
pub fn has_upstream_cycle(output: &ElementPtr) -> bool {
    use std::collections::HashSet; // local import for HashSet used in inner fn and below

    // Get the parent graph of this output.
    let parent = match output.borrow().get_parent() {
        Some(p) => p,
        None => return false,
    };

    // DFS from connected node; detect cycle if we visit a node twice.
    fn dfs(node: &ElementPtr, graph: &ElementPtr, visited: &mut HashSet<String>) -> bool {
        let name = node.borrow().get_name().to_string();
        if visited.contains(&name) {
            return true; // cycle
        }
        visited.insert(name);
        // Walk all inputs of this node.
        let inputs = get_inputs(node);
        for inp in &inputs {
            let node_name = inp.borrow().get_node_name().map(|s| s.to_string());
            if let Some(nn) = node_name {
                if let Some(upstream) = graph.borrow().get_child(&nn) {
                    if dfs(&upstream, graph, visited) {
                        return true;
                    }
                }
            }
        }
        false
    }

    // Start from the node this output connects to.
    let node_name = output.borrow().get_node_name().map(|s| s.to_string());
    if let Some(nn) = node_name {
        if let Some(start_node) = parent.borrow().get_child(&nn) {
            let mut visited = HashSet::new();
            return dfs(&start_node, &parent, &mut visited);
        }
    }
    false
}

// ---- validate_port ----

/// Validate a PortElement (input or output): check that connection targets exist
/// and that connected types match. Returns (valid, error_messages).
/// Mirrors C++ PortElement::validate + Input::validate + Output::validate.
pub fn validate_port(port: &ElementPtr) -> (bool, Vec<String>) {
    let mut errors = Vec::new();
    let mut valid = true;

    let cat = port.borrow().get_category().to_string();
    let port_type = port
        .borrow()
        .get_type()
        .map(|s| s.to_string())
        .unwrap_or_default();

    // Retrieve parent graph for sibling lookups.
    let parent = port.borrow().get_parent();
    let graph = parent.as_ref().and_then(|p| p.borrow().get_parent());

    let has_node_name = port.borrow().has_attribute(NODE_NAME_ATTRIBUTE);
    let has_node_graph = port.borrow().has_attribute(NODE_GRAPH_ATTRIBUTE);
    let has_output_str = port.borrow().has_attribute(OUTPUT_ATTRIBUTE);
    let node_name_val = port
        .borrow()
        .get_attribute(NODE_NAME_ATTRIBUTE)
        .map(|s| s.to_string())
        .unwrap_or_default();
    let output_str = port
        .borrow()
        .get_attribute(OUTPUT_ATTRIBUTE)
        .map(|s| s.to_string())
        .unwrap_or_default();

    // If nodename is set, the referenced node must exist in the parent graph.
    if has_node_name && !node_name_val.is_empty() {
        let node_exists = graph
            .as_ref()
            .and_then(|g| g.borrow().get_child(&node_name_val))
            .is_some();
        if !node_exists {
            // Also check if the name resolves to a nodegraph (valid cross-graph ref).
            let is_nodegraph_ref = graph
                .as_ref()
                .and_then(|g| g.borrow().get_child(&node_name_val))
                .map(|c| c.borrow().get_category().to_string() == category::NODE_GRAPH)
                .unwrap_or(false);
            if !is_nodegraph_ref {
                valid = false;
                errors.push(format!(
                    "Invalid port connection: node '{}' not found for port '{}'",
                    node_name_val,
                    port.borrow().get_name()
                ));
            }
        } else if has_output_str && !output_str.is_empty() {
            // If specific output is named, verify type match.
            if let Some(src_node) = graph
                .as_ref()
                .and_then(|g| g.borrow().get_child(&node_name_val))
            {
                let src_type = src_node
                    .borrow()
                    .get_type()
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                // Multioutput nodes can supply any type.
                if src_type != crate::core::types::MULTI_OUTPUT_TYPE_STRING
                    && !port_type.is_empty()
                    && !src_type.is_empty()
                    && src_type != port_type
                {
                    valid = false;
                    errors.push(format!(
                        "Mismatched types in port connection: port '{}' type '{}' vs node '{}' type '{}'",
                        port.borrow().get_name(), port_type, node_name_val, src_type
                    ));
                }
            }
        }
    }

    // Input-specific checks.
    if cat == category::INPUT {
        if let Some(ref par) = parent {
            let par_cat = par.borrow().get_category().to_string();
            if par_cat == category::NODE {
                // For node inputs, count bindings (value, nodename, nodegraph, interfacename, outputstring standalone).
                let has_value = port.borrow().has_value();
                let has_interface = port.borrow().has_attribute(INTERFACE_NAME_ATTRIBUTE);
                let standalone_output = has_output_str && !has_node_name && !has_node_graph;
                let num_bindings = [
                    has_value,
                    has_node_name,
                    has_node_graph,
                    has_interface,
                    standalone_output,
                ]
                .iter()
                .filter(|&&b| b)
                .count();
                if num_bindings == 0 {
                    valid = false;
                    errors.push(format!(
                        "Node input '{}' binds no value or connection",
                        port.borrow().get_name()
                    ));
                }
                if num_bindings > 1 {
                    valid = false;
                    errors.push(format!(
                        "Node input '{}' has too many bindings ({})",
                        port.borrow().get_name(),
                        num_bindings
                    ));
                }
            }
        }
    }

    (valid, errors)
}

#[cfg(test)]
mod validate_port_tests {
    use super::*;
    use crate::core::element::{Element, ElementPtr, add_child_of_category, category};

    fn make_graph() -> (ElementPtr, ElementPtr) {
        let root = ElementPtr::new(Element::new(None, category::DOCUMENT, ""));
        let graph = add_child_of_category(&root, category::NODE_GRAPH, "g").unwrap();
        (root, graph)
    }

    #[test]
    fn validate_port_input_with_value_passes() {
        let (_root, graph) = make_graph();
        let node = add_child_of_category(&graph, category::NODE, "n1").unwrap();
        node.borrow_mut().set_attribute("type", "float");
        let inp = add_child_of_category(&node, category::INPUT, "in1").unwrap();
        inp.borrow_mut().set_attribute("type", "float");
        inp.borrow_mut().set_value_string("1.0");

        let (valid, errors) = validate_port(&inp);
        assert!(valid, "input with value should pass: {:?}", errors);
    }

    #[test]
    fn validate_port_input_no_binding_fails() {
        let (_root, graph) = make_graph();
        let node = add_child_of_category(&graph, category::NODE, "n1").unwrap();
        node.borrow_mut().set_attribute("type", "float");
        let inp = add_child_of_category(&node, category::INPUT, "in1").unwrap();
        inp.borrow_mut().set_attribute("type", "float");
        // No value, no connection.

        let (valid, errors) = validate_port(&inp);
        assert!(!valid, "input with no binding should fail");
        assert!(errors.iter().any(|e| e.contains("binds no value")));
    }

    #[test]
    fn validate_port_input_nodename_valid() {
        let (_root, graph) = make_graph();
        let src = add_child_of_category(&graph, category::NODE, "src").unwrap();
        src.borrow_mut().set_attribute("type", "float");
        let node = add_child_of_category(&graph, category::NODE, "n1").unwrap();
        node.borrow_mut().set_attribute("type", "float");
        let inp = add_child_of_category(&node, category::INPUT, "in1").unwrap();
        inp.borrow_mut().set_attribute("type", "float");
        inp.borrow_mut().set_node_name("src");

        let (valid, errors) = validate_port(&inp);
        assert!(
            valid,
            "input connected to existing node should pass: {:?}",
            errors
        );
    }

    #[test]
    fn validate_port_input_nodename_missing() {
        let (_root, graph) = make_graph();
        let node = add_child_of_category(&graph, category::NODE, "n1").unwrap();
        node.borrow_mut().set_attribute("type", "float");
        let inp = add_child_of_category(&node, category::INPUT, "in1").unwrap();
        inp.borrow_mut().set_attribute("type", "float");
        inp.borrow_mut().set_node_name("nonexistent");

        let (valid, errors) = validate_port(&inp);
        assert!(!valid, "connection to missing node should fail");
        assert!(errors.iter().any(|e| e.contains("nonexistent")));
    }

    #[test]
    fn validate_port_output_no_connection_passes() {
        let (_root, graph) = make_graph();
        let out = add_child_of_category(&graph, category::OUTPUT, "out").unwrap();
        out.borrow_mut().set_attribute("type", "float");
        // Output with no connection is ok at the port level.
        let (valid, _errors) = validate_port(&out);
        assert!(valid, "disconnected output should pass port validation");
    }
}
