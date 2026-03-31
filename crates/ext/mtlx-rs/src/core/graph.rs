//! GraphElement helper functions — topological sort, subgraph flattening, DOT output.

use std::collections::{HashMap, HashSet, VecDeque};

use crate::core::element::{ElementPtr, category};
use crate::core::node::{get_inputs, get_outputs};

// ─── Topological Sort ────────────────────────────────────────────────────────

/// Return nodes + outputs of a graph element in topological (dependency) order.
///
/// Uses Kahn's algorithm: compute in-degree for each child by counting how many
/// of its inputs have a nodename pointing to a sibling in the same graph. Start
/// with zero-in-degree nodes, then iteratively reduce in-degrees as we emit each
/// node, following downstream connections via nodename attributes.
///
/// Matches C++ `GraphElement::topologicalSort()`.
pub fn topological_sort(graph: &ElementPtr) -> Vec<ElementPtr> {
    let children: Vec<ElementPtr> = graph.borrow().get_children().to_vec();

    // Build a name → ElementPtr map for fast sibling lookup.
    let sibling_map: HashMap<String, ElementPtr> = children
        .iter()
        .map(|e| (e.borrow().get_name().to_string(), e.clone()))
        .collect();

    // Compute in-degree: number of inputs that connect to a sibling.
    let mut in_degree: HashMap<String, usize> = children
        .iter()
        .map(|e| (e.borrow().get_name().to_string(), 0usize))
        .collect();

    for child in &children {
        let cat = child.borrow().get_category().to_string();
        if cat == category::OUTPUT {
            // Output node: connected to a sibling via nodename
            if let Some(node_name) = child.borrow().get_node_name().map(|s| s.to_string()) {
                if sibling_map.contains_key(&node_name) {
                    let name = child.borrow().get_name().to_string();
                    *in_degree.entry(name).or_insert(0) += 1;
                }
            }
        } else {
            // Node: count inputs with nodename pointing to a sibling
            for inp in get_inputs(child) {
                if let Some(node_name) = inp.borrow().get_node_name().map(|s| s.to_string()) {
                    if sibling_map.contains_key(&node_name) {
                        let name = child.borrow().get_name().to_string();
                        *in_degree.entry(name).or_insert(0) += 1;
                    }
                }
            }
        }
    }

    // Seed queue with zero-in-degree elements.
    let mut queue: VecDeque<ElementPtr> = children
        .iter()
        .filter(|e| in_degree[e.borrow().get_name()] == 0)
        .cloned()
        .collect();

    let mut result: Vec<ElementPtr> = Vec::with_capacity(children.len());

    while let Some(elem) = queue.pop_front() {
        result.push(elem.clone());

        let cat = elem.borrow().get_category().to_string();
        // Only nodes can have downstream consumers — find them and reduce their in-degree.
        if cat != category::INPUT && cat != category::OUTPUT {
            let elem_name = elem.borrow().get_name().to_string();
            // Walk all siblings looking for things that depend on this node.
            for sibling in &children {
                let sib_name = sibling.borrow().get_name().to_string();
                let sib_cat = sibling.borrow().get_category().to_string();

                let depends = if sib_cat == category::OUTPUT {
                    sibling
                        .borrow()
                        .get_node_name()
                        .map(|n| n == elem_name)
                        .unwrap_or(false)
                } else {
                    get_inputs(sibling).iter().any(|inp| {
                        inp.borrow()
                            .get_node_name()
                            .map(|n| n == elem_name)
                            .unwrap_or(false)
                    })
                };

                if depends {
                    let cnt = in_degree.entry(sib_name.clone()).or_insert(0);
                    if *cnt > 1 {
                        *cnt -= 1;
                    } else {
                        *cnt = 0;
                        queue.push_back(sibling.clone());
                    }
                }
            }
        }
    }

    result
}

// ─── Flatten Subgraphs ───────────────────────────────────────────────────────

/// Recursively inline all sub-nodegraphs into the parent graph.
///
/// Mirrors C++ `GraphElement::flattenSubgraphs()`. For each node in the graph
/// whose implementation resolves to a NodeGraph, inline that subgraph's nodes
/// into the parent graph, rewire connections, transfer interface bindings, and
/// remove the original compound node. Repeats until no more compound nodes remain.
pub fn flatten_subgraphs(graph: &mut ElementPtr) {
    use crate::core::definition::get_implementation_for_nodedef;
    use crate::core::element::{
        INTERFACE_NAME_ATTRIBUTE, add_child_of_category, copy_content_from_element,
    };
    use crate::core::node::{
        get_downstream_ports, get_input, get_interface_name, get_node_def, has_interface_name,
    };

    // Get the Document for resolving implementations.
    let doc = match crate::core::Document::from_element(graph) {
        Some(d) => d,
        None => return,
    };

    // Collect initial queue of node children (not inputs/outputs).
    let mut node_queue: Vec<ElementPtr> = graph
        .borrow()
        .get_children()
        .iter()
        .filter(|c| {
            let cat = c.borrow().get_category().to_string();
            cat != category::INPUT && cat != category::OUTPUT
        })
        .cloned()
        .collect();

    while !node_queue.is_empty() {
        // Phase 1: identify nodes with NodeGraph implementations.
        let mut process_nodes: Vec<ElementPtr> = Vec::new();
        let mut graph_impl_map: HashMap<String, ElementPtr> = HashMap::new();
        let mut decl_map: HashMap<String, Option<ElementPtr>> = HashMap::new();
        let mut downstream_map: HashMap<String, Vec<ElementPtr>> = HashMap::new();

        for node in &node_queue {
            let nd = match get_node_def(node, "", true) {
                Some(nd) => nd,
                None => continue,
            };
            let impl_elem = match get_implementation_for_nodedef(&nd, &doc, "", true) {
                Some(e) => e,
                None => continue,
            };
            if impl_elem.borrow().get_category() != category::NODE_GRAPH {
                continue;
            }
            let node_name = node.borrow().get_name().to_string();
            process_nodes.push(node.clone());
            downstream_map.insert(node_name.clone(), get_downstream_ports(node));
            decl_map.insert(node_name.clone(), Some(nd.clone()));
            // Pre-compute downstream ports for each sub-node in the impl.
            let sub_nodes: Vec<ElementPtr> = impl_elem
                .borrow()
                .get_children()
                .iter()
                .filter(|c| {
                    let cat = c.borrow().get_category().to_string();
                    cat != category::INPUT && cat != category::OUTPUT
                })
                .cloned()
                .collect();
            for sn in &sub_nodes {
                let sn_name = sn.borrow().get_name().to_string();
                downstream_map.insert(sn_name, get_downstream_ports(sn));
            }
            graph_impl_map.insert(node_name, impl_elem);
        }
        node_queue.clear();

        // Phase 2: process each compound node.
        for process_node in &process_nodes {
            let pn_name = process_node.borrow().get_name().to_string();
            let source_subgraph = match graph_impl_map.get(&pn_name) {
                Some(sg) => sg.clone(),
                None => continue,
            };

            // Map from source sub-node name to new dest sub-node name.
            let mut sub_node_map: HashMap<String, String> = HashMap::new();

            // Get insert position of the process node.
            let insert_idx = graph.borrow().get_child_index(&pn_name).unwrap_or(0);

            // Create new instances of each sub-node in the parent graph.
            let sub_nodes: Vec<ElementPtr> = source_subgraph
                .borrow()
                .get_children()
                .iter()
                .filter(|c| {
                    let cat = c.borrow().get_category().to_string();
                    cat != category::INPUT && cat != category::OUTPUT
                })
                .cloned()
                .collect();

            for source_sub in &sub_nodes {
                let orig_name = source_sub.borrow().get_name().to_string();
                let dest_name = graph.borrow().create_valid_child_name(&orig_name);
                let src_cat = source_sub.borrow().get_category().to_string();

                if let Ok(dest_sub) = add_child_of_category(graph, &src_cat, &dest_name) {
                    // Copy all content from source to dest.
                    copy_content_from_element(&dest_sub, &source_sub.borrow());
                    // Reorder to process_node's position.
                    let _ = graph.borrow_mut().set_child_index(&dest_name, insert_idx);

                    sub_node_map.insert(orig_name.clone(), dest_name.clone());
                    // Queue for recursive processing.
                    node_queue.push(dest_sub);
                }
            }

            // Rewire internal connections: update nodenames on dest sub-node inputs.
            for (src_name, dest_name) in &sub_node_map {
                if let Some(downstream_ports) = downstream_map.get(src_name) {
                    for port in downstream_ports {
                        let port_cat = port.borrow().get_category().to_string();
                        if port_cat == category::INPUT {
                            // This port is an input on a downstream sub-node.
                            // Find its parent, map the parent name, update the input.
                            if let Some(parent) = port.borrow().get_parent() {
                                let parent_name = parent.borrow().get_name().to_string();
                                if let Some(dest_parent_name) = sub_node_map.get(&parent_name) {
                                    let port_name = port.borrow().get_name().to_string();
                                    if let Some(dest_parent) =
                                        graph.borrow().get_child(dest_parent_name)
                                    {
                                        if let Some(dest_inp) = get_input(&dest_parent, &port_name)
                                        {
                                            dest_inp.borrow_mut().set_node_name(dest_name);
                                        }
                                    }
                                }
                            }
                        } else if port_cat == category::OUTPUT {
                            // Output of the subgraph: redirect processNode's downstream.
                            if let Some(pn_downstream) = downstream_map.get(&pn_name) {
                                for pn_port in pn_downstream {
                                    pn_port.borrow_mut().set_node_name(dest_name);
                                }
                            }
                        }
                    }
                }
            }

            // Transfer interface properties.
            for (_src_name, dest_name) in &sub_node_map {
                if let Some(dest_sub) = graph.borrow().get_child(dest_name) {
                    let dest_inputs = get_inputs(&dest_sub);
                    for dest_inp in &dest_inputs {
                        if has_interface_name(dest_inp) {
                            let iface_name = match get_interface_name(dest_inp) {
                                Some(n) => n,
                                None => continue,
                            };
                            // Try to get the corresponding input from processNode.
                            if let Some(src_inp) = get_input(process_node, &iface_name) {
                                copy_content_from_element(dest_inp, &src_inp.borrow());
                            } else {
                                // Fall back to nodedef default.
                                if let Some(Some(decl)) = decl_map.get(&pn_name) {
                                    if let Some(decl_inp) = get_input(decl, &iface_name) {
                                        if decl_inp.borrow().has_value_string() {
                                            let val = decl_inp.borrow().get_value_string();
                                            dest_inp.borrow_mut().set_value_string(val);
                                        }
                                    }
                                }
                                dest_inp
                                    .borrow_mut()
                                    .remove_attribute(INTERFACE_NAME_ATTRIBUTE);
                            }
                        }
                    }
                }
            }

            // Update downstream ports referencing subgraph outputs.
            if let Some(pn_downstream) = downstream_map.get(&pn_name) {
                for downstream_port in pn_downstream {
                    if downstream_port.borrow().has_output_string() {
                        let out_str = downstream_port
                            .borrow()
                            .get_output_string()
                            .unwrap_or("")
                            .to_string();
                        if let Some(sub_output) =
                            crate::core::node::get_output(&source_subgraph, &out_str)
                        {
                            let upstream_name = sub_output
                                .borrow()
                                .get_node_name()
                                .unwrap_or("")
                                .to_string();
                            // Map to dest sub-node name.
                            let dest_name = sub_node_map
                                .get(&upstream_name)
                                .cloned()
                                .unwrap_or(upstream_name);
                            downstream_port.borrow_mut().set_node_name(&dest_name);
                            downstream_port.borrow_mut().set_output_string("");
                        }
                    }
                }
            }

            // Remove the processed compound node.
            graph.borrow_mut().remove_child(&pn_name);
        }
    }
}

// ─── DOT Visualization ───────────────────────────────────────────────────────

/// Generate a DOT language representation of the graph for Graphviz visualization.
///
/// Writes `digraph { ... }` with:
/// - Each node as a box-shaped vertex labeled by its element name.
/// - Each connection (input.nodename -> node) as a directed edge labeled by the input name.
/// - Output elements shown as edges from their connected node to themselves.
///
/// Matches the structure of C++ `GraphElement::asStringDot()`.
pub fn as_string_dot(graph: &ElementPtr) -> String {
    let sorted = topological_sort(graph);

    let mut dot = String::from("digraph {\n");

    // ── Emit node declarations ────────────────────────────────────────────
    for elem in &sorted {
        let cat = elem.borrow().get_category().to_string();
        if cat != category::INPUT && cat != category::OUTPUT {
            let name = elem.borrow().get_name().to_string();
            dot.push_str(&format!("    \"{}\" [shape=box];\n", name));
        }
    }

    // ── Emit output declarations ──────────────────────────────────────────
    for out_elem in get_outputs(graph) {
        let name = out_elem.borrow().get_name().to_string();
        dot.push_str(&format!("    \"{}\" [shape=ellipse];\n", name));
    }

    // ── Emit edges from node inputs ───────────────────────────────────────
    let mut seen_edges: HashSet<String> = HashSet::new();

    for elem in &sorted {
        let cat = elem.borrow().get_category().to_string();
        if cat == category::OUTPUT {
            // output → (no downstream in DOT, it IS the terminal)
            if let Some(upstream_name) = elem.borrow().get_node_name().map(|s| s.to_string()) {
                let out_name = elem.borrow().get_name().to_string();
                let key = format!("{}->{}:{}", upstream_name, out_name, "");
                if seen_edges.insert(key) {
                    dot.push_str(&format!("    \"{}\" -> \"{}\";\n", upstream_name, out_name));
                }
            }
        } else {
            // Node: emit edge for each connected input
            let node_name = elem.borrow().get_name().to_string();
            for inp in get_inputs(elem) {
                let inp_name = inp.borrow().get_name().to_string();
                if let Some(upstream_name) = inp.borrow().get_node_name().map(|s| s.to_string()) {
                    let key = format!("{}->{}:{}", upstream_name, node_name, inp_name);
                    if seen_edges.insert(key) {
                        dot.push_str(&format!(
                            "    \"{}\" -> \"{}\" [label=\"{}\"];\n",
                            upstream_name, node_name, inp_name
                        ));
                    }
                }
            }
        }
    }

    dot.push_str("}\n");
    dot
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::document::create_document;
    use crate::core::element::{add_child_of_category, category};

    fn make_test_graph() -> (ElementPtr, ElementPtr) {
        let mut doc = create_document();
        let graph = doc.add_node_graph("graph1").unwrap();

        // Three nodes: A -> B -> C, plus an output
        let node_a = add_child_of_category(&graph, "image", "nodeA").unwrap();
        node_a.borrow_mut().set_type("color3");

        let node_b = add_child_of_category(&graph, "multiply", "nodeB").unwrap();
        node_b.borrow_mut().set_type("color3");
        let inp_b = add_child_of_category(&node_b, category::INPUT, "in1").unwrap();
        inp_b.borrow_mut().set_node_name("nodeA");
        inp_b.borrow_mut().set_type("color3");

        let node_c = add_child_of_category(&graph, "add", "nodeC").unwrap();
        node_c.borrow_mut().set_type("color3");
        let inp_c = add_child_of_category(&node_c, category::INPUT, "in1").unwrap();
        inp_c.borrow_mut().set_node_name("nodeB");
        inp_c.borrow_mut().set_type("color3");

        let out = add_child_of_category(&graph, category::OUTPUT, "out").unwrap();
        out.borrow_mut().set_node_name("nodeC");
        out.borrow_mut().set_type("color3");

        (graph, doc.get_root())
    }

    #[test]
    fn test_topological_sort_order() {
        let (graph, _doc) = make_test_graph();
        let sorted = topological_sort(&graph);

        // Collect names in result order
        let names: Vec<String> = sorted
            .iter()
            .map(|e| e.borrow().get_name().to_string())
            .collect();

        // nodeA must appear before nodeB, nodeB before nodeC, nodeC before out
        let pos = |n: &str| names.iter().position(|x| x == n).unwrap();
        assert!(pos("nodeA") < pos("nodeB"), "A must precede B");
        assert!(pos("nodeB") < pos("nodeC"), "B must precede C");
        assert!(pos("nodeC") < pos("out"), "C must precede out");
    }

    #[test]
    fn test_as_string_dot_contains_nodes() {
        let (graph, _doc) = make_test_graph();
        let dot = as_string_dot(&graph);

        assert!(dot.starts_with("digraph {"), "must start with digraph");
        assert!(dot.contains("\"nodeA\""), "must contain nodeA");
        assert!(dot.contains("\"nodeB\""), "must contain nodeB");
        assert!(dot.contains("\"nodeC\""), "must contain nodeC");
        assert!(dot.contains("nodeA\" -> \"nodeB\""), "A->B edge");
        assert!(dot.contains("nodeB\" -> \"nodeC\""), "B->C edge");
        assert!(dot.ends_with("}\n"), "must end with }}");
    }

    #[test]
    fn test_flatten_subgraphs_noop() {
        // Just verify it compiles and doesn't panic on an empty graph.
        let mut doc = create_document();
        let mut graph = doc.add_node_graph("g").unwrap();
        flatten_subgraphs(&mut graph); // should be a no-op for now
    }
}
