//! GenShader utility functions.
//! Mirrors MaterialXGenShader/Util.cpp.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::core::{Document, ElementPtr, Value};

/// Gaussian kernel weights for kernel size 3 (sigma 1).
pub const GAUSSIAN_KERNEL_3: [f32; 3] = [0.27901, 0.44198, 0.27901];

/// Gaussian kernel weights for kernel size 5 (sigma 1).
pub const GAUSSIAN_KERNEL_5: [f32; 5] = [0.06136, 0.24477, 0.38774, 0.24477, 0.06136];

/// Gaussian kernel weights for kernel size 7 (sigma 1).
pub const GAUSSIAN_KERNEL_7: [f32; 7] = [
    0.00598, 0.060626, 0.241843, 0.383103, 0.241843, 0.060626, 0.00598,
];

/// Hash a string (used to deduplicate ShaderNodeImpl instances).
pub fn hash_string(s: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

/// Replace `$TOKEN` patterns in `source` using the provided substitution map.
/// Tokens are alphanumeric (including `_`) after the `$` prefix.
/// Unmatched tokens are left unchanged including their `$` prefix.
pub fn token_substitution(source: &str, substitutions: &[(String, String)]) -> String {
    if substitutions.is_empty() || source.is_empty() {
        return source.to_string();
    }
    let mut result = String::with_capacity(source.len());
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut pos = 0;
    while pos < len {
        if let Some(rel) = source[pos..].find('$') {
            let p1 = pos + rel;
            result.push_str(&source[pos..p1]);
            pos = p1 + 1;
            let tok_start = pos;
            while pos < len && (bytes[pos].is_ascii_alphanumeric() || bytes[pos] == b'_') {
                pos += 1;
            }
            let tok_body = &source[tok_start..pos];
            let full_token = format!("${}", tok_body);
            if let Some(replacement) = substitutions
                .iter()
                .find(|(k, _)| k == &full_token)
                .map(|(_, v)| v)
            {
                result.push_str(replacement);
            } else {
                result.push_str(&full_token);
            }
        } else {
            result.push_str(&source[pos..]);
            break;
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Opacity / transparency helpers (ref: Util.cpp anonymous namespace)
// ---------------------------------------------------------------------------

/// (input_name, opaque_value): inputs checked for transparency.
/// opacity/existence/alpha expected 1.0 (opaque), transmission expected 0.0 (opaque).
const OPAQUE_INPUT_LIST: &[(&str, f32)] = &[
    ("opacity", 1.0),
    ("existence", 1.0),
    ("alpha", 1.0),
    ("transmission", 0.0),
];

const EPSILON: f32 = 0.00001;

fn is_equal_f(v1: f32, v2: f32) -> bool {
    (v1 - v2).abs() < EPSILON
}

/// Check if a Value equals float `f` (supports float and color3).
fn value_equals_float(value: &Value, f: f32) -> bool {
    match value {
        Value::Float(v) => is_equal_f(*v, f),
        Value::Color3(c) => is_equal_f(c.0[0], f) && is_equal_f(c.0[1], f) && is_equal_f(c.0[2], f),
        _ => false,
    }
}

/// Find the NodeDef matching a node element's "nodedef" attribute or its category + type.
/// Matches C++ Node::getNodeDef(target).
fn find_node_def_for_node(node: &ElementPtr, doc: &Document, target: &str) -> Option<ElementPtr> {
    // Try explicit nodedef attribute first
    if let Some(nd_name) = node.borrow().get_attribute("nodedef") {
        if let Some(nd) = doc.get_node_def(nd_name) {
            return Some(nd);
        }
    }
    // Fallback: match by node category + target
    let node_category = node.borrow().get_category().to_string();
    let node_type = node
        .borrow()
        .get_type()
        .map(|t| t.to_string())
        .unwrap_or_default();
    let candidates = doc.get_matching_node_defs(&node_category);
    // Prefer matching target, then empty target
    let mut fallback: Option<ElementPtr> = None;
    for nd in candidates {
        let nd_type = nd
            .borrow()
            .get_type()
            .map(|t| t.to_string())
            .unwrap_or_default();
        if !node_type.is_empty() && nd_type != node_type {
            continue;
        }
        let nd_target = nd.borrow().get_target().to_string();
        if !target.is_empty() && nd_target == target {
            return Some(nd);
        }
        if nd_target.is_empty() || nd_target == target {
            fallback = Some(nd);
        }
    }
    fallback
}

/// Get shader nodes connected to a material node's surfaceshader inputs.
fn get_shader_nodes_for_material(material: &ElementPtr) -> Vec<ElementPtr> {
    let mut result = Vec::new();
    let parent = material.borrow().get_parent();
    for child in material.borrow().get_children().to_vec() {
        let child_cat = child.borrow().get_category().to_string();
        if child_cat != "input" {
            continue;
        }
        let inp_type = child
            .borrow()
            .get_type()
            .map(|t| t.to_string())
            .unwrap_or_default();
        if inp_type != "surfaceshader" {
            continue;
        }
        if let Some(conn_name) = child.borrow().get_node_name().map(|s| s.to_string()) {
            let found = parent
                .as_ref()
                .and_then(|p| p.borrow().get_child(&conn_name));
            if let Some(f) = found {
                result.push(f);
            }
        }
    }
    result
}

/// Input hint attribute names from C++ Input::TRANSPARENCY_HINT / OPACITY_HINT.
const TRANSPARENCY_HINT: &str = "transparency";
const OPACITY_HINT: &str = "opacity";
/// NodeDef node group attribute (C++ NodeDef::NODE_GROUP_ATTRIBUTE).
const NODE_GROUP_ATTRIBUTE: &str = "nodegroup";
/// Node groups that are NOT transparent when connected (C++ adjustment/channel).
const ADJUSTMENT_NODE_GROUP: &str = "adjustment";
const CHANNEL_NODE_GROUP: &str = "channel";

/// Returns true if the surface shader node at `node` has transparency-indicating inputs.
/// Handles mix nodes (recursion on fg/bg), inputHints, nodeGroup filtering,
/// interface names, and graph definitions.
/// Matches C++ isTransparentShaderNode exactly.
fn is_transparent_shader_node(
    node: &ElementPtr,
    doc: &Document,
    target: &str,
    interface_node: Option<&ElementPtr>,
) -> bool {
    let node_type = node
        .borrow()
        .get_type()
        .map(|t| t.to_string())
        .unwrap_or_default();
    if node_type != "surfaceshader" {
        return false;
    }
    let node_category = node.borrow().get_category().to_string();
    let parent = node.borrow().get_parent();

    // Mix node: recurse on fg and bg
    if node_category == "mix" {
        for mix_input_name in &["fg", "bg"] {
            if let Some(mix_inp) = node.borrow().get_child(mix_input_name) {
                if let Some(conn_name) = mix_inp.borrow().get_node_name().map(|s| s.to_string()) {
                    let conn = parent
                        .as_ref()
                        .and_then(|p| p.borrow().get_child(&conn_name));
                    if let Some(c) = conn {
                        if is_transparent_shader_node(&c, doc, target, None) {
                            return true;
                        }
                    }
                }
            }
        }
        return false;
    }

    // Build input pair list: start with defaults, extend with inputHints from nodedef (по рефу ~129-143)
    let mut owned_input_pairs: Vec<(String, f32)> = OPAQUE_INPUT_LIST
        .iter()
        .map(|&(n, v)| (n.to_string(), v))
        .collect();

    if let Some(node_def) = find_node_def_for_node(node, doc, target) {
        for child in node_def.borrow().get_children().to_vec() {
            let child_cat = child.borrow().get_category().to_string();
            if child_cat != "input" {
                continue;
            }
            if let Some(hint) = child.borrow().get_attribute("hint").map(|s| s.to_string()) {
                let child_name = child.borrow().get_name().to_string();
                if hint == TRANSPARENCY_HINT {
                    owned_input_pairs.push((child_name, 0.0));
                } else if hint == OPACITY_HINT {
                    owned_input_pairs.push((child_name, 1.0));
                }
            }
        }
    }

    // Check against interface node (по рефу ~146-168)
    if let Some(iface_node) = interface_node {
        let mut interface_names: Vec<(String, f32)> = Vec::new();
        for (inp_name, opaque_val) in &owned_input_pairs {
            // Use getActiveInput - check child directly
            if let Some(check_input) = node.borrow().get_child(inp_name) {
                let iface_name = check_input.borrow().get_interface_name().to_string();
                if !iface_name.is_empty() {
                    interface_names.push((iface_name, *opaque_val));
                }
            }
        }
        if !interface_names.is_empty() {
            // Check if interface node has transparent inputs
            for (iface_name, opaque_val) in &interface_names {
                if let Some(iface_inp) = iface_node.borrow().get_child(iface_name) {
                    if iface_inp.borrow().get_node_name().is_some() {
                        return true;
                    }
                    let val_str = iface_inp.borrow().get_value().map(|s| s.to_string());
                    let type_str = iface_inp
                        .borrow()
                        .get_type()
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "float".to_string());
                    if let Some(vs) = val_str {
                        if let Some(parsed) = Value::from_strings(&vs, &type_str) {
                            if !value_equals_float(&parsed, *opaque_val) {
                                return true;
                            }
                        }
                    }
                }
            }
        }
    }

    // Check each input (по рефу ~170-214)
    for (inp_name, opaque_val) in &owned_input_pairs {
        if let Some(check_input_elem) = node.borrow().get_child(inp_name) {
            // Check interface name remapping
            let iface_name = check_input_elem.borrow().get_interface_name().to_string();
            let resolved_input = if !iface_name.is_empty() {
                // Try to resolve via nodegraph interface (по рефу getInputInterface)
                let resolved = if let Some(par) = parent.as_ref() {
                    let nd_name = par.borrow().get_attribute("nodedef").map(|s| s.to_string());
                    if let Some(nd_name) = nd_name {
                        doc.get_node_def(&nd_name)
                            .and_then(|nd| nd.borrow().get_child(&iface_name))
                    } else {
                        par.borrow().get_child(&iface_name)
                    }
                } else {
                    None
                };
                match resolved {
                    Some(r) => r,
                    None => continue, // C++ returns false for unresolved; we skip this input
                }
            } else {
                check_input_elem.clone()
            };

            // Check if connected to a node (по рефу ~193-203)
            if let Some(conn_name) = resolved_input
                .borrow()
                .get_node_name()
                .map(|s| s.to_string())
            {
                // Check nodeGroup of connected node: adjustment/channel groups are NOT transparent
                let input_node = parent
                    .as_ref()
                    .and_then(|p| p.borrow().get_child(&conn_name));
                if let Some(ref input_node) = input_node {
                    let input_node_def = find_node_def_for_node(input_node, doc, target);
                    let node_group = input_node_def
                        .and_then(|nd| {
                            nd.borrow()
                                .get_attribute(NODE_GROUP_ATTRIBUTE)
                                .map(|s| s.to_string())
                        })
                        .unwrap_or_default();
                    if node_group != ADJUSTMENT_NODE_GROUP && node_group != CHANNEL_NODE_GROUP {
                        return true;
                    }
                } else {
                    // Connected but can't resolve node — assume transparent
                    return true;
                }
            } else {
                // Not connected: check value (по рефу ~205-210)
                let val_str = resolved_input.borrow().get_value().map(|s| s.to_string());
                let type_str = resolved_input
                    .borrow()
                    .get_type()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "float".to_string());
                if let Some(vs) = val_str {
                    if let Some(parsed) = Value::from_strings(&vs, &type_str) {
                        if !value_equals_float(&parsed, *opaque_val) {
                            return true;
                        }
                    }
                }
            }
        }
    }

    // Check BSDF implementation via NodeGraph
    if let Some(node_def) = find_node_def_for_node(node, doc, target) {
        let nd_name = node_def.borrow().get_name().to_string();
        for impl_elem in doc.get_matching_implementations(&nd_name) {
            let impl_cat = impl_elem.borrow().get_category().to_string();
            if impl_cat != "nodegraph" {
                continue;
            }
            let impl_target = impl_elem.borrow().get_target().to_string();
            if !target.is_empty() && !impl_target.is_empty() && impl_target != target {
                continue;
            }
            if is_transparent_nodegraph(&impl_elem, doc, target, Some(node)) {
                return true;
            }
        }
    }

    false
}

/// Walk all nodes in a nodegraph and check if any are transparent surface shaders.
fn is_transparent_nodegraph(
    graph: &ElementPtr,
    doc: &Document,
    target: &str,
    interface_node: Option<&ElementPtr>,
) -> bool {
    // Collect outputs of type surfaceshader
    for child in graph.borrow().get_children().to_vec() {
        let cat = child.borrow().get_category().to_string();
        if cat != "output" {
            continue;
        }
        let out_type = child
            .borrow()
            .get_type()
            .map(|t| t.to_string())
            .unwrap_or_default();
        if out_type != "surfaceshader" {
            continue;
        }
        // Traverse upstream from this output
        if is_transparent_graph_output(&child, graph, doc, target, interface_node) {
            return true;
        }
    }
    false
}

/// BFS upstream from a graph output checking for transparent shader nodes.
fn is_transparent_graph_output(
    output: &ElementPtr,
    graph: &ElementPtr,
    doc: &Document,
    target: &str,
    interface_node: Option<&ElementPtr>,
) -> bool {
    let mut visited = std::collections::HashSet::new();
    let mut queue = std::collections::VecDeque::new();

    if let Some(conn_name) = output.borrow().get_node_name().map(|s| s.to_string()) {
        if let Some(conn) = graph.borrow().get_child(&conn_name) {
            queue.push_back(conn);
        }
    }

    while let Some(elem) = queue.pop_front() {
        let elem_name = elem.borrow().get_name().to_string();
        if visited.contains(&elem_name) {
            continue;
        }
        visited.insert(elem_name);

        if is_transparent_shader_node(&elem, doc, target, interface_node) {
            return true;
        }

        // Check BSDF nodes for nodegraph impl
        let elem_type = elem
            .borrow()
            .get_type()
            .map(|t| t.to_string())
            .unwrap_or_default();
        if elem_type == "BSDF" {
            if let Some(nd) = find_node_def_for_node(&elem, doc, target) {
                let nd_name = nd.borrow().get_name().to_string();
                for impl_e in doc.get_matching_implementations(&nd_name) {
                    if impl_e.borrow().get_category() == "nodegraph" {
                        if is_transparent_nodegraph(&impl_e, doc, target, Some(&elem)) {
                            return true;
                        }
                    }
                }
            }
        }

        // Queue all upstream connected nodes
        for child in elem.borrow().get_children().to_vec() {
            if child.borrow().get_category() == "input" {
                if let Some(conn_name) = child.borrow().get_node_name().map(|s| s.to_string()) {
                    if let Some(conn) = graph.borrow().get_child(&conn_name) {
                        queue.push_back(conn);
                    }
                }
            }
        }
    }
    false
}

/// Returns true if the given element is a surface shader with potential transparency.
/// Matches C++ isTransparentSurface(ElementPtr element, const string& target).
///
/// Handles:
/// - Material nodes: follows to surface shader nodes
/// - Surface shader nodes: checks opacity/transmission/alpha inputs
/// - Output elements: follows to connected node
/// - NodeGraph implementations: recursively checks
pub fn is_transparent_surface(element: &ElementPtr, target: &str, doc: &Document) -> bool {
    let cat = element.borrow().get_category().to_string();

    if cat == "node" {
        let node_type = element
            .borrow()
            .get_type()
            .map(|t| t.to_string())
            .unwrap_or_default();

        // Material nodes: follow to shader nodes
        let node_to_check = if node_type == "material" || cat == "surfacematerial" {
            let shader_nodes = get_shader_nodes_for_material(element);
            if !shader_nodes.is_empty() {
                shader_nodes[0].clone()
            } else {
                return false;
            }
        } else {
            element.clone()
        };

        // Direct transparency check
        if is_transparent_shader_node(&node_to_check, doc, target, None) {
            return true;
        }

        // Check graph implementation
        if let Some(nd) = find_node_def_for_node(&node_to_check, doc, target) {
            let nd_name = nd.borrow().get_name().to_string();
            let _node_type2 = node_to_check
                .borrow()
                .get_type()
                .map(|t| t.to_string())
                .unwrap_or_default();
            for impl_e in doc.get_matching_implementations(&nd_name) {
                if impl_e.borrow().get_category() == "nodegraph" {
                    // Only check surface shader type outputs
                    let has_surface = impl_e.borrow().get_children().iter().any(|c| {
                        c.borrow().get_category() == "output"
                            && c.borrow().get_type().unwrap_or("") == "surfaceshader"
                    });
                    if has_surface
                        && is_transparent_nodegraph(&impl_e, doc, target, Some(&node_to_check))
                    {
                        return true;
                    }
                }
            }
        }
    } else if cat == "output" {
        // Follow output to connected node
        if let Some(conn_name) = element.borrow().get_node_name().map(|s| s.to_string()) {
            let parent = element.borrow().get_parent();
            if let Some(conn) = parent
                .as_ref()
                .and_then(|p| p.borrow().get_child(&conn_name))
            {
                return is_transparent_surface(&conn, target, doc);
            }
        }
    }

    false
}

/// Map a value to a four-channel color [r, g, b, a].
/// If no mapping is possible, returns opaque black [0,0,0,1].
/// Matches C++ mapValueToColor.
pub fn map_value_to_color(value: Option<&Value>) -> [f32; 4] {
    let mut color = [0.0f32, 0.0, 0.0, 1.0];
    let Some(val) = value else {
        return color;
    };
    match val {
        Value::Float(f) => {
            color[0] = *f;
        }
        Value::Color3(c) => {
            color[0] = c.0[0];
            color[1] = c.0[1];
            color[2] = c.0[2];
        }
        Value::Vector3(v) => {
            color[0] = v.0[0];
            color[1] = v.0[1];
            color[2] = v.0[2];
        }
        Value::Color4(c) => {
            color[0] = c.0[0];
            color[1] = c.0[1];
            color[2] = c.0[2];
            color[3] = c.0[3];
        }
        Value::Vector4(v) => {
            color[0] = v.0[0];
            color[1] = v.0[1];
            color[2] = v.0[2];
            color[3] = v.0[3];
        }
        Value::Vector2(v) => {
            color[0] = v.0[0];
            color[1] = v.0[1];
        }
        _ => {}
    }
    color
}

/// Return whether a nodedef requires an implementation.
/// Organization nodes and type="none" do not.
pub fn requires_implementation(node_group: Option<&str>, type_attr: Option<&str>) -> bool {
    if let Some(group) = node_group {
        if group == "organization" {
            return false;
        }
    }
    match type_attr {
        Some(t) if !t.is_empty() && t != "none" => true,
        _ => false,
    }
}

/// Determine if an element type requires shading/lighting for rendering.
/// Matches C++ elementRequiresShading.
pub fn element_requires_shading(element_type: &str) -> bool {
    static COLOR_CLOSURES: &[&str] = &[
        "material",
        "surfaceshader",
        "volumeshader",
        "lightshader",
        "BSDF",
        "EDF",
        "VDF",
    ];
    COLOR_CLOSURES.contains(&element_type)
}

// ---------------------------------------------------------------------------
// Renderable elements (ref: Util.cpp findRenderableElements / findRenderableMaterialNodes)
// ---------------------------------------------------------------------------

/// Find all renderable material nodes in the document.
/// A material node is renderable if it has connected surface shader nodes.
/// Matches C++ findRenderableMaterialNodes.
pub fn find_renderable_material_nodes(doc: &Document) -> Vec<ElementPtr> {
    let mut result = Vec::new();
    for child in doc.get_root().borrow().get_children().to_vec() {
        let cat = child.borrow().get_category().to_string();
        if cat == "surfacematerial" || cat == "volumematerial" {
            if !get_shader_nodes_for_material(&child).is_empty() {
                result.push(child.clone());
            }
        }
    }
    result
}

/// Output types excluded from renderable graph outputs.
const UNSUPPORTED_OUTPUT_TYPES: &[&str] = &["BSDF", "EDF", "VDF", "lightshader"];

/// Find all renderable elements: material nodes, or graph outputs if no materials.
/// Matches C++ findRenderableElements.
pub fn find_renderable_elements(doc: &Document) -> Vec<ElementPtr> {
    let mut result = find_renderable_material_nodes(doc);
    if !result.is_empty() {
        return result;
    }

    let doc_uri = doc.get_root().borrow().get_active_source_uri();

    // Collect renderable outputs from all NodeGraphs
    for graph in doc.get_node_graphs() {
        for out in graph.borrow().get_children().to_vec() {
            if out.borrow().get_category() != "output" {
                continue;
            }
            // Filter by source URI
            let out_uri = out.borrow().get_active_source_uri();
            if !doc_uri.is_empty() && !out_uri.is_empty() && out_uri != doc_uri {
                continue;
            }
            // Must be connected to a node
            let conn_name = out.borrow().get_node_name().map(|s| s.to_string());
            if let Some(cn) = conn_name {
                if let Some(conn) = graph.borrow().get_child(&cn) {
                    let node_type = conn
                        .borrow()
                        .get_type()
                        .map(|t| t.to_string())
                        .unwrap_or_default();
                    if !UNSUPPORTED_OUTPUT_TYPES.contains(&node_type.as_str()) {
                        result.push(out.clone());
                    }
                }
            }
        }
    }

    // Also check top-level document outputs
    for child in doc.get_root().borrow().get_children().to_vec() {
        if child.borrow().get_category() != "output" {
            continue;
        }
        let out_uri = child.borrow().get_active_source_uri();
        if !doc_uri.is_empty() && !out_uri.is_empty() && out_uri != doc_uri {
            continue;
        }
        if let Some(conn_name) = child.borrow().get_node_name().map(|s| s.to_string()) {
            if let Some(conn) = doc.get_root().borrow().get_child(&conn_name) {
                let node_type = conn
                    .borrow()
                    .get_type()
                    .map(|t| t.to_string())
                    .unwrap_or_default();
                if !UNSUPPORTED_OUTPUT_TYPES.contains(&node_type.as_str()) {
                    result.push(child.clone());
                }
            }
        }
    }
    result
}

/// Given a node input element, return the corresponding input in its NodeDef.
/// Matches C++ getNodeDefInput(InputPtr nodeInput, const string& target).
pub fn get_node_def_input(
    node_input: &ElementPtr,
    target: &str,
    doc: &Document,
) -> Option<ElementPtr> {
    let parent = node_input.borrow().get_parent()?;
    let nd = find_node_def_for_node(&parent, doc, target)?;
    let input_name = node_input.borrow().get_name().to_string();
    nd.borrow().get_child(&input_name)
}

// ---------------------------------------------------------------------------
// UDIM coordinate helpers (ref: Util.cpp getUdimCoordinates / getUdimScaleAndOffset)
// ---------------------------------------------------------------------------

/// Compute UDIM tile coordinates from UDIM identifier strings (e.g. "1001" -> (0, 0)).
/// Matches C++ getUdimCoordinates.
pub fn get_udim_coordinates(udim_identifiers: &[String]) -> Vec<(i32, i32)> {
    let mut coords = Vec::new();
    for id in udim_identifiers {
        if id.is_empty() {
            continue;
        }
        let udim_val: i32 = match id.parse() {
            Ok(v) => v,
            Err(_) => continue,
        };
        if udim_val <= 1000 || udim_val >= 2000 {
            continue; // C++ throws; we skip
        }
        // UDIM 1001 = tile (0,0), 1002 = (1,0), 1011 = (0,1), etc.
        let idx = udim_val - 1001; // 0-based index
        let u_val = idx % 10;
        let v_val = idx / 10;
        coords.push((u_val, v_val));
    }
    coords
}

/// Compute UV scale and offset to transform UDIM tile space to 0..1.
/// Matches C++ getUdimScaleAndOffset.
/// Returns (scale_uv, offset_uv).
pub fn get_udim_scale_and_offset(udim_coords: &[(i32, i32)]) -> ([f32; 2], [f32; 2]) {
    if udim_coords.is_empty() {
        return ([1.0, 1.0], [0.0, 0.0]);
    }

    let mut min_u = udim_coords[0].0 as f32;
    let mut min_v = udim_coords[0].1 as f32;
    let mut max_u = min_u;
    let mut max_v = min_v;

    for &(u, v) in udim_coords.iter().skip(1) {
        let uf = u as f32;
        let vf = v as f32;
        if uf < min_u {
            min_u = uf;
        }
        if vf < min_v {
            min_v = vf;
        }
        if uf > max_u {
            max_u = uf;
        }
        if vf > max_v {
            max_v = vf;
        }
    }

    // Extend to upper-right corner of last tile (each tile is 1x1)
    max_u += 1.0;
    max_v += 1.0;

    let scale = [1.0 / (max_u - min_u), 1.0 / (max_v - min_v)];
    let offset = [-min_u, -min_v];
    (scale, offset)
}

// ---------------------------------------------------------------------------
// Graph attribute / world space queries (ref: Util.cpp)
// ---------------------------------------------------------------------------

/// Return the connected node if it is a world-space generating node (e.g. "normalmap").
/// Matches C++ connectsToWorldSpaceNode(OutputPtr output).
pub fn connects_to_world_space_node(output: &ElementPtr) -> Option<ElementPtr> {
    const WORLD_SPACE_CATEGORIES: &[&str] = &["normalmap"];
    let conn_name = output.borrow().get_node_name()?.to_string();
    let parent = output.borrow().get_parent()?;
    let conn = parent.borrow().get_child(&conn_name)?;
    let cat = conn.borrow().get_category().to_string();
    if WORLD_SPACE_CATEGORIES.contains(&cat.as_str()) {
        Some(conn.clone())
    } else {
        None
    }
}

/// Returns true if any value element in the graph upstream of `output`
/// has any of the given attributes.
/// Matches C++ hasElementAttributes(OutputPtr output, const StringVec& attributes).
pub fn has_element_attributes(output: &ElementPtr, attributes: &[String]) -> bool {
    if attributes.is_empty() {
        return false;
    }

    let mut visited = std::collections::HashSet::new();
    let mut queue = std::collections::VecDeque::new();
    queue.push_back(output.clone());

    while let Some(elem) = queue.pop_front() {
        let elem_name = elem.borrow().get_name().to_string();
        let elem_cat = elem.borrow().get_category().to_string();

        if visited.contains(&elem_name) {
            continue;
        }
        visited.insert(elem_name);

        let parent = elem.borrow().get_parent();

        if elem_cat == "node" {
            // Check all input/parameter children for requested attributes
            for child in elem.borrow().get_children().to_vec() {
                let child_cat = child.borrow().get_category().to_string();
                if child_cat == "input" || child_cat == "parameter" {
                    for attr in attributes {
                        if child.borrow().get_attribute(attr).is_some() {
                            return true;
                        }
                    }
                    // Also follow upstream connections
                    if let Some(conn_name) = child.borrow().get_node_name().map(|s| s.to_string()) {
                        if let Some(par) = parent.as_ref() {
                            if let Some(conn) = par.borrow().get_child(&conn_name) {
                                queue.push_back(conn);
                            }
                        }
                    }
                }
            }
        } else if elem_cat == "output" || elem_cat == "input" {
            if let Some(conn_name) = elem.borrow().get_node_name().map(|s| s.to_string()) {
                if let Some(par) = parent.as_ref() {
                    if let Some(conn) = par.borrow().get_child(&conn_name) {
                        queue.push_back(conn);
                    }
                }
            }
        }
    }

    false
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_substitution_basic() {
        let source = "Hello $NAME world";
        let subs = vec![("$NAME".to_string(), "Alice".to_string())];
        assert_eq!(token_substitution(source, &subs), "Hello Alice world");
    }

    #[test]
    fn test_token_substitution_multiple() {
        let source = "$TYPE $VAR = $VALUE;";
        let subs = vec![
            ("$TYPE".to_string(), "int".to_string()),
            ("$VAR".to_string(), "x".to_string()),
            ("$VALUE".to_string(), "42".to_string()),
        ];
        assert_eq!(token_substitution(source, &subs), "int x = 42;");
    }

    #[test]
    fn test_token_substitution_not_found() {
        let source = "Value is $UNKNOWN here";
        let subs = vec![("$OTHER".to_string(), "value".to_string())];
        assert_eq!(token_substitution(source, &subs), "Value is $UNKNOWN here");
    }

    #[test]
    fn test_token_substitution_adjacent() {
        let source = "$A$B$C";
        let subs = vec![
            ("$A".to_string(), "x".to_string()),
            ("$B".to_string(), "y".to_string()),
            ("$C".to_string(), "z".to_string()),
        ];
        assert_eq!(token_substitution(source, &subs), "xyz");
    }

    #[test]
    fn test_udim_coordinates_basic() {
        let ids = vec!["1001".to_string(), "1002".to_string(), "1011".to_string()];
        let coords = get_udim_coordinates(&ids);
        assert_eq!(coords.len(), 3);
        assert_eq!(coords[0], (0, 0)); // 1001 -> u=0, v=0
        assert_eq!(coords[1], (1, 0)); // 1002 -> u=1, v=0
        assert_eq!(coords[2], (0, 1)); // 1011 -> u=0, v=1
    }

    #[test]
    fn test_udim_invalid_skipped() {
        // 1000 and 2000 are invalid
        let ids = vec!["1000".to_string(), "2000".to_string(), "1001".to_string()];
        let coords = get_udim_coordinates(&ids);
        assert_eq!(coords.len(), 1);
        assert_eq!(coords[0], (0, 0));
    }

    #[test]
    fn test_udim_scale_single_tile() {
        let coords = vec![(0i32, 0i32)];
        let (scale, offset) = get_udim_scale_and_offset(&coords);
        assert!((scale[0] - 1.0).abs() < 1e-5);
        assert!((scale[1] - 1.0).abs() < 1e-5);
        assert!((offset[0] - 0.0).abs() < 1e-5);
        assert!((offset[1] - 0.0).abs() < 1e-5);
    }

    #[test]
    fn test_udim_scale_2x1() {
        let coords = vec![(0i32, 0i32), (1i32, 0i32)];
        let (scale, _offset) = get_udim_scale_and_offset(&coords);
        // max_u = 1+1=2, min_u=0 -> scale_u = 1/2 = 0.5
        assert!((scale[0] - 0.5).abs() < 1e-5, "scale_u={}", scale[0]);
        assert!((scale[1] - 1.0).abs() < 1e-5, "scale_v={}", scale[1]);
    }

    #[test]
    fn test_gaussian_kernels_sum_to_one() {
        let sum3: f32 = GAUSSIAN_KERNEL_3.iter().sum();
        let sum5: f32 = GAUSSIAN_KERNEL_5.iter().sum();
        let sum7: f32 = GAUSSIAN_KERNEL_7.iter().sum();
        assert!((sum3 - 1.0).abs() < 0.001, "kernel3 sum={}", sum3);
        assert!((sum5 - 1.0).abs() < 0.001, "kernel5 sum={}", sum5);
        assert!((sum7 - 1.0).abs() < 0.001, "kernel7 sum={}", sum7);
    }

    #[test]
    fn test_value_equals_float() {
        use crate::core::Color3;
        assert!(value_equals_float(&Value::Float(1.0), 1.0));
        assert!(!value_equals_float(&Value::Float(0.5), 1.0));
        assert!(value_equals_float(
            &Value::Color3(Color3([1.0, 1.0, 1.0])),
            1.0
        ));
        assert!(!value_equals_float(
            &Value::Color3(Color3([1.0, 0.5, 1.0])),
            1.0
        ));
    }

    #[test]
    fn test_element_requires_shading() {
        assert!(element_requires_shading("surfaceshader"));
        assert!(element_requires_shading("BSDF"));
        assert!(!element_requires_shading("float"));
        assert!(!element_requires_shading("integer"));
    }

    #[test]
    fn test_requires_implementation() {
        assert!(requires_implementation(None, Some("surfaceshader")));
        assert!(!requires_implementation(
            Some("organization"),
            Some("float")
        ));
        assert!(!requires_implementation(None, Some("none")));
        assert!(!requires_implementation(None, None));
    }

    #[test]
    fn test_map_value_to_color() {
        use crate::core::Color3;
        let c = map_value_to_color(None);
        assert_eq!(c, [0.0, 0.0, 0.0, 1.0]);
        let c = map_value_to_color(Some(&Value::Float(0.5)));
        assert!((c[0] - 0.5).abs() < 1e-5);
        let c = map_value_to_color(Some(&Value::Color3(Color3([0.1, 0.2, 0.3]))));
        assert!((c[0] - 0.1).abs() < 1e-5);
        assert!((c[2] - 0.3).abs() < 1e-5);
    }
}
