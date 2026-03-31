//! Material, ShaderRef, MaterialAssign, Look -- material hierarchy.

use crate::core::element::{
    ACTIVE_ATTRIBUTE, ElementPtr, INHERIT_ATTRIBUTE, LOOKS_ATTRIBUTE, NODE_GRAPH_ATTRIBUTE,
    OUTPUT_ATTRIBUTE, add_child_of_category, category,
};

const SURFACE_SHADER_TYPE: &str = "surfaceshader";

// Attribute constants matching C++ Look.cpp
const MATERIAL_ATTRIBUTE: &str = "material";
const EXCLUSIVE_ATTRIBUTE: &str = "exclusive";
const VIEWER_GEOM_ATTRIBUTE: &str = "viewergeom";
const VIEWER_COLLECTION_ATTRIBUTE: &str = "viewercollection";
const VISIBILITY_TYPE_ATTRIBUTE: &str = "vistype";
const VISIBLE_ATTRIBUTE: &str = "visible";

// --- Material ---

/// Get shader ref children of a material
pub fn get_shader_refs(material: &ElementPtr) -> Vec<ElementPtr> {
    material
        .borrow()
        .get_children()
        .iter()
        .filter(|c| c.borrow().get_category() == category::SHADER_REF)
        .cloned()
        .collect()
}

/// Get surface shader input from material (input with type surfaceshader)
pub fn get_surface_shader_input(material: &ElementPtr) -> Option<ElementPtr> {
    for child in material.borrow().get_children() {
        let c = child.borrow();
        if c.get_category() == category::INPUT && c.get_type() == Some(SURFACE_SHADER_TYPE) {
            return Some(child.clone());
        }
    }
    None
}

// --- Look inheritance ---

/// Get the inherit string on a Look
pub fn get_look_inherit_string(look: &ElementPtr) -> String {
    look.borrow()
        .get_attribute(INHERIT_ATTRIBUTE)
        .map(|s| s.to_string())
        .unwrap_or_default()
}

/// Set the inherit string on a Look
pub fn set_look_inherit_string(look: &ElementPtr, inherit: impl Into<String>) {
    let s = inherit.into();
    if s.is_empty() {
        look.borrow_mut().remove_attribute(INHERIT_ATTRIBUTE);
    } else {
        look.borrow_mut().set_attribute(INHERIT_ATTRIBUTE, s);
    }
}

// --- MaterialAssign ---

/// Get MaterialAssign children of a Look
pub fn get_material_assigns(look: &ElementPtr) -> Vec<ElementPtr> {
    look.borrow()
        .get_children_of_category(category::MATERIAL_ASSIGN)
}

/// Get MaterialAssign by name from Look
pub fn get_material_assign(look: &ElementPtr, name: &str) -> Option<ElementPtr> {
    look.borrow()
        .get_child_of_category(name, category::MATERIAL_ASSIGN)
}

/// Get material string from a MaterialAssign element
pub fn get_material_string(assign: &ElementPtr) -> String {
    assign.borrow().get_attribute_or_empty(MATERIAL_ATTRIBUTE)
}

/// Set material string on a MaterialAssign element
pub fn set_material_string(assign: &ElementPtr, material: impl Into<String>) {
    assign
        .borrow_mut()
        .set_attribute(MATERIAL_ATTRIBUTE, material.into());
}

/// Has material string on MaterialAssign
pub fn has_material_string(assign: &ElementPtr) -> bool {
    assign.borrow().has_attribute(MATERIAL_ATTRIBUTE)
}

/// Get exclusive flag from a MaterialAssign element
pub fn get_exclusive(assign: &ElementPtr) -> bool {
    assign.borrow().get_attribute(EXCLUSIVE_ATTRIBUTE) == Some("true")
}

/// Set exclusive flag on a MaterialAssign element
pub fn set_exclusive(assign: &ElementPtr, value: bool) {
    assign
        .borrow_mut()
        .set_attribute(EXCLUSIVE_ATTRIBUTE, if value { "true" } else { "false" });
}

/// Get all active MaterialAssign elements for a Look, walking the inherit chain
pub fn get_active_material_assigns(look: &ElementPtr) -> Vec<ElementPtr> {
    collect_active_children_by_category(look, category::MATERIAL_ASSIGN)
}

/// Add a MaterialAssign to a Look
pub fn add_material_assign(look: &ElementPtr, name: &str) -> Result<ElementPtr, String> {
    add_child_of_category(look, category::MATERIAL_ASSIGN, name)
}

/// Remove MaterialAssign by name from Look
pub fn remove_material_assign(look: &ElementPtr, name: &str) {
    look.borrow_mut()
        .remove_child_of_category(name, category::MATERIAL_ASSIGN);
}

// --- PropertyAssign ---

/// Get PropertyAssign children of a Look (direct, no inheritance)
pub fn get_property_assigns(look: &ElementPtr) -> Vec<ElementPtr> {
    look.borrow()
        .get_children_of_category(category::PROPERTY_ASSIGN)
}

/// Get PropertyAssign by name from Look
pub fn get_property_assign(look: &ElementPtr, name: &str) -> Option<ElementPtr> {
    look.borrow()
        .get_child_of_category(name, category::PROPERTY_ASSIGN)
}

/// Get all active PropertyAssign elements, walking the inherit chain
pub fn get_active_property_assigns(look: &ElementPtr) -> Vec<ElementPtr> {
    collect_active_children_by_category(look, category::PROPERTY_ASSIGN)
}

/// Add a PropertyAssign to a Look
pub fn add_property_assign(look: &ElementPtr, name: &str) -> Result<ElementPtr, String> {
    add_child_of_category(look, category::PROPERTY_ASSIGN, name)
}

/// Remove PropertyAssign by name from Look
pub fn remove_property_assign(look: &ElementPtr, name: &str) {
    look.borrow_mut()
        .remove_child_of_category(name, category::PROPERTY_ASSIGN);
}

// --- PropertySetAssign ---

/// Add a PropertySetAssign to a Look
pub fn add_property_set_assign(look: &ElementPtr, name: &str) -> Result<ElementPtr, String> {
    add_child_of_category(look, category::PROPERTY_SET_ASSIGN, name)
}

/// Get PropertySetAssign by name from Look
pub fn get_property_set_assign(look: &ElementPtr, name: &str) -> Option<ElementPtr> {
    look.borrow()
        .get_child_of_category(name, category::PROPERTY_SET_ASSIGN)
}

/// Get all PropertySetAssign children of a Look
pub fn get_property_set_assigns(look: &ElementPtr) -> Vec<ElementPtr> {
    look.borrow()
        .get_children_of_category(category::PROPERTY_SET_ASSIGN)
}

/// Get all active PropertySetAssign elements, walking the inherit chain
pub fn get_active_property_set_assigns(look: &ElementPtr) -> Vec<ElementPtr> {
    collect_active_children_by_category(look, category::PROPERTY_SET_ASSIGN)
}

/// Remove PropertySetAssign by name from Look
pub fn remove_property_set_assign(look: &ElementPtr, name: &str) {
    look.borrow_mut()
        .remove_child_of_category(name, category::PROPERTY_SET_ASSIGN);
}

// --- Visibility ---

/// Get Visibility children of a Look (direct, no inheritance)
pub fn get_visibilities(look: &ElementPtr) -> Vec<ElementPtr> {
    look.borrow().get_children_of_category(category::VISIBILITY)
}

/// Get Visibility by name from Look
pub fn get_visibility(look: &ElementPtr, name: &str) -> Option<ElementPtr> {
    look.borrow()
        .get_child_of_category(name, category::VISIBILITY)
}

/// Get viewer geom string from a Visibility element
pub fn get_viewer_geom(vis: &ElementPtr) -> String {
    vis.borrow().get_attribute_or_empty(VIEWER_GEOM_ATTRIBUTE)
}

/// Set viewer geom string on a Visibility element
pub fn set_viewer_geom(vis: &ElementPtr, geom: impl Into<String>) {
    vis.borrow_mut()
        .set_attribute(VIEWER_GEOM_ATTRIBUTE, geom.into());
}

/// Get viewer collection string from a Visibility element
pub fn get_viewer_collection(vis: &ElementPtr) -> String {
    vis.borrow()
        .get_attribute_or_empty(VIEWER_COLLECTION_ATTRIBUTE)
}

/// Set viewer collection string on a Visibility element
pub fn set_viewer_collection(vis: &ElementPtr, collection: impl Into<String>) {
    vis.borrow_mut()
        .set_attribute(VIEWER_COLLECTION_ATTRIBUTE, collection.into());
}

/// Get visibility type string from a Visibility element
pub fn get_visibility_type(vis: &ElementPtr) -> String {
    vis.borrow()
        .get_attribute_or_empty(VISIBILITY_TYPE_ATTRIBUTE)
}

/// Set visibility type string on a Visibility element
pub fn set_visibility_type(vis: &ElementPtr, vis_type: impl Into<String>) {
    vis.borrow_mut()
        .set_attribute(VISIBILITY_TYPE_ATTRIBUTE, vis_type.into());
}

/// Get visible boolean from a Visibility element
pub fn get_visible(vis: &ElementPtr) -> bool {
    vis.borrow().get_attribute(VISIBLE_ATTRIBUTE) == Some("true")
}

/// Set visible boolean on a Visibility element
pub fn set_visible(vis: &ElementPtr, visible: bool) {
    vis.borrow_mut()
        .set_attribute(VISIBLE_ATTRIBUTE, if visible { "true" } else { "false" });
}

/// Get all active Visibility elements, walking the inherit chain
pub fn get_active_visibilities(look: &ElementPtr) -> Vec<ElementPtr> {
    collect_active_children_by_category(look, category::VISIBILITY)
}

/// Add a Visibility to a Look
pub fn add_visibility(look: &ElementPtr, name: &str) -> Result<ElementPtr, String> {
    add_child_of_category(look, category::VISIBILITY, name)
}

/// Remove Visibility by name from Look
pub fn remove_visibility(look: &ElementPtr, name: &str) {
    look.borrow_mut()
        .remove_child_of_category(name, category::VISIBILITY);
}

// --- VariantAssign on MaterialAssign ---

/// Add a VariantAssign child to a MaterialAssign. C++ MaterialAssign::addVariantAssign.
pub fn add_variant_assign_to_material(
    assign: &ElementPtr,
    name: &str,
) -> Result<ElementPtr, String> {
    add_child_of_category(assign, category::VARIANT_ASSIGN, name)
}

/// Get VariantAssign by name from MaterialAssign.
pub fn get_variant_assign_of_material(assign: &ElementPtr, name: &str) -> Option<ElementPtr> {
    assign
        .borrow()
        .get_child_of_category(name, category::VARIANT_ASSIGN)
}

/// Get all VariantAssign children of a MaterialAssign.
pub fn get_variant_assigns_of_material(assign: &ElementPtr) -> Vec<ElementPtr> {
    assign
        .borrow()
        .get_children_of_category(category::VARIANT_ASSIGN)
}

/// Get active VariantAssign children of a MaterialAssign (walks inheritance).
pub fn get_active_variant_assigns_of_material(assign: &ElementPtr) -> Vec<ElementPtr> {
    collect_active_children_by_category(assign, category::VARIANT_ASSIGN)
}

/// Remove VariantAssign by name from MaterialAssign.
pub fn remove_variant_assign_from_material(assign: &ElementPtr, name: &str) {
    assign
        .borrow_mut()
        .remove_child_of_category(name, category::VARIANT_ASSIGN);
}

// --- MaterialAssign::getReferencedMaterial ---

/// Get the referenced material node from a MaterialAssign element.
/// Resolves the "material" attribute to a node in the parent document.
/// C++ MaterialAssign::getReferencedMaterial.
pub fn get_referenced_material(assign: &ElementPtr) -> Option<ElementPtr> {
    let mat_name = assign
        .borrow()
        .get_attribute(MATERIAL_ATTRIBUTE)
        .map(|s| s.to_string())
        .unwrap_or_default();
    if mat_name.is_empty() {
        return None;
    }
    let doc = crate::core::Document::from_element(assign)?;
    // Look for a node with the material name.
    let root = doc.get_root();
    for child in root.borrow().get_children() {
        if child.borrow().get_category() == category::NODE && child.borrow().get_name() == mat_name
        {
            return Some(child.clone());
        }
    }
    None
}

// --- getShaderNodes free function ---

/// Return all shader nodes connected to a material node, filtered by type and target.
/// Walks material inputs, resolves connections (direct and via nodegraphs).
/// C++ getShaderNodes (Material.cpp).
pub fn get_shader_nodes(
    material_node: &ElementPtr,
    node_type: &str,
    target: &str,
) -> Vec<ElementPtr> {
    use crate::core::interface::{get_active_inputs, get_active_outputs};
    use crate::core::node::{get_node_def, get_output, get_outputs};
    use std::collections::HashSet;

    let mut shader_vec = Vec::new();
    let mut shader_names = HashSet::new();

    let shader_matches = |node: &ElementPtr| {
        if node_type.is_empty() {
            return true;
        }

        let node_output_type = node.borrow().get_type().unwrap_or("").to_string();
        if node_output_type == node_type {
            return true;
        }

        if node_output_type == crate::core::types::MULTI_OUTPUT_TYPE_STRING {
            return get_active_outputs(node).iter().any(|output| {
                output
                    .borrow()
                    .get_type()
                    .map(|t| t == node_type)
                    .unwrap_or(false)
            });
        }

        false
    };

    let parent = match material_node.borrow().get_parent() {
        Some(p) => p,
        None => return shader_vec,
    };

    let inputs = get_active_inputs(material_node);
    for inp in &inputs {
        // Try direct connection via nodename.
        let shader_node = {
            let nn = inp.borrow().get_node_name().map(|s| s.to_string());
            nn.and_then(|n| parent.borrow().get_child(&n))
        };

        if let Some(ref sn) = shader_node {
            let sn_name = sn.borrow().get_name().to_string();
            if shader_names.contains(&sn_name) {
                continue;
            }
            if !shader_matches(sn) {
                continue;
            }
            if !target.is_empty() && get_node_def(sn, target, true).is_none() {
                continue;
            }
            shader_vec.push(sn.clone());
            shader_names.insert(sn_name);
        } else {
            // Check upstream nodegraph.
            let ng_name = inp
                .borrow()
                .get_attribute(NODE_GRAPH_ATTRIBUTE)
                .map(|s| s.to_string());
            if let Some(ng_str) = ng_name {
                if ng_str.is_empty() {
                    continue;
                }
                let node_graph = parent.borrow().get_child(&ng_str);
                let ng = match node_graph {
                    Some(ref g) if g.borrow().get_category() == category::NODE_GRAPH => g.clone(),
                    _ => continue,
                };
                // Collect outputs to scan.
                let outputs_to_check: Vec<ElementPtr> = {
                    let out_str = inp
                        .borrow()
                        .get_attribute(OUTPUT_ATTRIBUTE)
                        .map(|s| s.to_string())
                        .unwrap_or_default();
                    if !out_str.is_empty() {
                        if let Some(o) = get_output(&ng, &out_str) {
                            vec![o]
                        } else {
                            vec![]
                        }
                    } else {
                        get_outputs(&ng)
                    }
                };
                for out in &outputs_to_check {
                    let upstream_name = out.borrow().get_node_name().map(|s| s.to_string());
                    if let Some(ref un) = upstream_name {
                        if let Some(upstream) = ng.borrow().get_child(un) {
                            let up_name = upstream.borrow().get_name().to_string();
                            if shader_names.contains(&up_name) {
                                continue;
                            }
                            if !shader_matches(&upstream) {
                                continue;
                            }
                            if !target.is_empty() && get_node_def(&upstream, target, true).is_none()
                            {
                                continue;
                            }
                            shader_vec.push(upstream.clone());
                            shader_names.insert(up_name);
                        }
                    }
                }
            }
        }
    }

    shader_vec
}

// --- getConnectedOutputs free function ---

/// Return unique Output elements connected to the inputs of a node.
/// C++ getConnectedOutputs (Material.cpp).
pub fn get_connected_outputs(node: &ElementPtr) -> Vec<ElementPtr> {
    use crate::core::node::{get_connected_output, get_inputs};
    use std::collections::HashSet;

    let mut result = Vec::new();
    let mut seen = HashSet::new();

    for inp in get_inputs(node) {
        if let Some(o) = get_connected_output(&inp) {
            let oname = o.borrow().get_name().to_string();
            let oparent = o
                .borrow()
                .get_parent()
                .map(|p| p.borrow().get_name().to_string())
                .unwrap_or_default();
            let key = format!("{}:{}", oparent, oname);
            if seen.insert(key) {
                result.push(o);
            }
        }
    }
    result
}

// --- getGeometryBindings free function ---

/// Return all MaterialAssign elements from a document that match the given geometry string.
/// C++ getGeometryBindings (Look.cpp).
pub fn get_geometry_bindings(doc_root: &ElementPtr, geom: &str) -> Vec<ElementPtr> {
    let mut result = Vec::new();
    // Scan all Look children for MaterialAssigns.
    for child in doc_root.borrow().get_children() {
        if child.borrow().get_category() == category::LOOK {
            for assign in child
                .borrow()
                .get_children_of_category(category::MATERIAL_ASSIGN)
            {
                let assign_geom = assign
                    .borrow()
                    .get_attribute("geom")
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                if !assign_geom.is_empty()
                    && crate::core::geom::geom_strings_match(&assign_geom, geom)
                {
                    result.push(assign.clone());
                }
            }
        }
    }
    result
}

// --- LookGroup helpers ---

/// Set looks string on LookGroup (C++ LookGroup::setLooks)
pub fn set_look_group_looks(lg: &ElementPtr, looks: impl Into<String>) {
    lg.borrow_mut().set_attribute(LOOKS_ATTRIBUTE, looks.into());
}

/// Get looks string from LookGroup
pub fn get_look_group_looks(lg: &ElementPtr) -> String {
    lg.borrow().get_attribute_or_empty(LOOKS_ATTRIBUTE)
}

/// Set active look on LookGroup (C++ LookGroup::setActiveLook)
pub fn set_look_group_active(lg: &ElementPtr, active: impl Into<String>) {
    lg.borrow_mut()
        .set_attribute(ACTIVE_ATTRIBUTE, active.into());
}

/// Get active look from LookGroup
pub fn get_look_group_active(lg: &ElementPtr) -> String {
    lg.borrow().get_attribute_or_empty(ACTIVE_ATTRIBUTE)
}

// --- Look: getActiveVariantAssigns ---

/// Get all active VariantAssign elements for a Look, walking the inherit chain.
/// C++ Look::getActiveVariantAssigns.
pub fn get_active_variant_assigns(look: &ElementPtr) -> Vec<ElementPtr> {
    collect_active_children_by_category(look, category::VARIANT_ASSIGN)
}

// --- MaterialAssign::getMaterialOutputs ---

/// Get material-type outputs from the NodeGraph referenced by this MaterialAssign.
/// Resolves the "material" attribute to a NodeGraph and returns its material outputs.
/// C++ MaterialAssign::getMaterialOutputs.
pub fn get_material_outputs_for_assign(assign: &ElementPtr) -> Vec<ElementPtr> {
    let mat_name = assign
        .borrow()
        .get_attribute(MATERIAL_ATTRIBUTE)
        .map(|s| s.to_string())
        .unwrap_or_default();
    if mat_name.is_empty() {
        return Vec::new();
    }
    // Resolve to a NodeGraph (C++ resolveNameReference<NodeGraph>).
    let doc = match crate::core::Document::from_element(assign) {
        Some(d) => d,
        None => return Vec::new(),
    };
    match doc.get_node_graph(&mat_name) {
        Some(ng) => crate::core::node::nodegraph_get_material_outputs(&ng),
        None => Vec::new(),
    }
}

// --- Shared inherit-chain traversal ---

fn collect_active_children_by_category(look: &ElementPtr, cat: &str) -> Vec<ElementPtr> {
    let mut result = Vec::new();
    let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut current: Option<ElementPtr> = Some(look.clone());

    while let Some(elem) = current.take() {
        let name = elem.borrow().get_name().to_string();
        if !visited.insert(name) {
            break;
        }

        for child in elem.borrow().get_children() {
            if child.borrow().get_category() == cat {
                result.push(child.clone());
            }
        }

        let inherit = elem
            .borrow()
            .get_attribute(INHERIT_ATTRIBUTE)
            .map(|s| s.to_string());
        if let Some(inherit_name) = inherit {
            if !inherit_name.is_empty() {
                current = elem
                    .borrow()
                    .get_parent()
                    .and_then(|p| p.borrow().get_child(&inherit_name));
            }
        }
    }

    result
}

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::document::create_document;

    #[test]
    fn test_look_inherit_string() {
        let mut doc = create_document();
        let look = doc.add_child_of_category(category::LOOK, "lookA").unwrap();
        assert_eq!(get_look_inherit_string(&look), "");
        set_look_inherit_string(&look, "lookBase");
        assert_eq!(get_look_inherit_string(&look), "lookBase");
        set_look_inherit_string(&look, "");
        assert_eq!(get_look_inherit_string(&look), "");
    }

    #[test]
    fn test_material_assign_attrs() {
        let mut doc = create_document();
        let look = doc.add_child_of_category(category::LOOK, "look1").unwrap();
        let ma = add_material_assign(&look, "ma1").unwrap();
        set_material_string(&ma, "mat_pbr");
        assert_eq!(get_material_string(&ma), "mat_pbr");
        set_exclusive(&ma, true);
        assert!(get_exclusive(&ma));
        set_exclusive(&ma, false);
        assert!(!get_exclusive(&ma));
    }

    #[test]
    fn test_get_material_assigns() {
        let mut doc = create_document();
        let look = doc.add_child_of_category(category::LOOK, "look1").unwrap();
        add_material_assign(&look, "ma1").unwrap();
        add_material_assign(&look, "ma2").unwrap();
        let assigns = get_material_assigns(&look);
        assert_eq!(assigns.len(), 2);
    }

    #[test]
    fn test_visibility_attrs() {
        let mut doc = create_document();
        let look = doc.add_child_of_category(category::LOOK, "look1").unwrap();
        let vis = add_visibility(&look, "vis1").unwrap();
        set_viewer_geom(&vis, "/root/geo");
        assert_eq!(get_viewer_geom(&vis), "/root/geo");
        set_visibility_type(&vis, "shadow");
        assert_eq!(get_visibility_type(&vis), "shadow");
        set_visible(&vis, true);
        assert!(get_visible(&vis));
    }

    #[test]
    fn test_active_material_assigns_with_inheritance() {
        let mut doc = create_document();
        let base = doc
            .add_child_of_category(category::LOOK, "lookBase")
            .unwrap();
        add_material_assign(&base, "ma_base").unwrap();
        let derived = doc
            .add_child_of_category(category::LOOK, "lookDerived")
            .unwrap();
        set_look_inherit_string(&derived, "lookBase");
        add_material_assign(&derived, "ma_derived").unwrap();
        let active = get_active_material_assigns(&derived);
        let names: Vec<String> = active
            .iter()
            .map(|e| e.borrow().get_name().to_string())
            .collect();
        assert!(names.contains(&"ma_derived".to_string()));
        assert!(names.contains(&"ma_base".to_string()));
    }
}
