//! ShaderNode factory — create ShaderNode from NodeDef (для geom nodes, color/unit transforms).

use crate::core::{Document, ElementPtr, get_active_inputs, get_active_outputs};

use super::ShaderGraphCreateContext;
use super::shader_metadata_registry::ShaderMetadataRegistry;
use super::shader_node::ShaderNode;

/// Populate ShaderNode port metadata from NodeDef (по рефу ShaderNode::createMetadata).
fn create_metadata_from_nodedef(
    node_def: &ElementPtr,
    shader_node: &mut ShaderNode,
    registry: &ShaderMetadataRegistry,
) {
    for port_elem in get_active_inputs(node_def)
        .into_iter()
        .chain(get_active_outputs(node_def).into_iter())
    {
        let port_name = port_elem.borrow().get_name().to_string();
        let nodedef_port = port_elem.borrow();
        let mut meta_to_add: Vec<(String, String)> = Vec::new();
        for attr_name in nodedef_port.get_attribute_names() {
            if let Some(entry) = registry.find_metadata(attr_name) {
                if let Some(attr_value) = nodedef_port.get_attribute(attr_name) {
                    if !attr_value.is_empty() {
                        let emit_value = format_value_for_metadata(&entry.type_name, attr_value);
                        meta_to_add.push((entry.name.clone(), emit_value));
                    }
                }
            }
        }
        drop(nodedef_port);
        if !meta_to_add.is_empty() {
            if let Some(inp) = shader_node.inputs.get_mut(&port_name) {
                for (n, v) in &meta_to_add {
                    inp.port_mut().add_metadata(n, v);
                }
            } else if let Some(out) = shader_node.outputs.get_mut(&port_name) {
                for (n, v) in &meta_to_add {
                    out.port_mut().add_metadata(n, v);
                }
            }
        }
    }
}

fn format_value_for_metadata(type_name: &str, value: &str) -> String {
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

/// Create ShaderNode from NodeDef only (по рефу ShaderNode::create(NodeDef)).
pub fn create_node_from_nodedef(
    node_name: &str,
    node_def: &ElementPtr,
    doc: &Document,
    context: &dyn ShaderGraphCreateContext,
) -> Result<ShaderNode, String> {
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
    let mut shader_node = ShaderNode::create_from_nodedef(None, node_name, node_def, context);
    impl_.add_inputs(&mut shader_node, context);
    if let Some(registry) = context.get_shader_metadata_registry() {
        if registry.has_metadata() {
            create_metadata_from_nodedef(node_def, &mut shader_node, registry);
        }
    }
    Ok(shader_node)
}
