//! OSL network emit — param, connect, shader format (по рефу OslNetworkShaderGenerator::generate).

use crate::core::Document;
use crate::core::ElementPtr;
use crate::core::Value;
use crate::gen_shader::{Shader, ShaderGraph, shader_stage};

use super::osl_emit::add_set_ci_terminal_node;
use super::osl_network_shader_generator::{
    OslNetworkShaderGenerator, OslNetworkShaderGraphContext, TARGET, create_osl_network_shader,
};
use crate::gen_shader::ShaderGraphCreateContext;

/// Generate OSL network format (param, connect, shader lines).
pub fn generate(
    name: &str,
    element: &ElementPtr,
    doc: &Document,
    context: &mut crate::gen_shader::GenContext<OslNetworkShaderGenerator>,
) -> Result<Shader, String> {
    let mut shader = create_osl_network_shader(name, element, doc, context)?;
    let mut graph = shader.graph;
    let opts = context.get_options();

    if opts.osl_connect_ci_wrapper {
        let net_ctx = OslNetworkShaderGraphContext::new(context);
        add_set_ci_terminal_node(&mut graph, doc, &net_ctx)?;
    }
    shader.graph = graph;

    let node_order: Vec<String> = shader.graph.node_order.clone();
    let graph_name = shader.graph.get_name().to_string();
    let syntax = context.get_shader_generator().get_syntax().get_syntax();
    let net_ctx = OslNetworkShaderGraphContext::new(context);
    let target = TARGET;

    let mut lines: Vec<String> = Vec::new();
    let mut connections: Vec<String> = Vec::new();
    // BTreeSet for sorted order matching C++ std::set<std::string>
    let mut oso_paths: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut last_output: Option<String> = None;

    for node_name in &node_order {
        let node = match shader.graph.get_node(node_name) {
            Some(n) => n,
            None => continue,
        };
        let node_def_name = match shader.graph.get_node_def(node_name) {
            Some(nd) => nd,
            None => continue,
        };
        let impl_: Box<dyn crate::gen_shader::ShaderNodeImpl> =
            match net_ctx.get_implementation_for_nodedef(doc, node_def_name, target) {
                Some(i) => i,
                None => continue,
            };
        let (oso_name, oso_path) = match impl_.as_oso() {
            Some(p) => p,
            None => continue,
        };
        oso_paths.insert(oso_path.to_string());

        for input in node.get_inputs() {
            let mut input_name = input.get_name().to_string();
            syntax.make_valid_name(&mut input_name);

            let connection = input.get_connection().and_then(|(up_n, up_o)| {
                if up_n == graph_name {
                    None
                } else {
                    Some((up_n.to_string(), up_o.to_string()))
                }
            });

            if connection.is_none() {
                if !input.port().get_value_string().is_empty() {
                    let skip_names = ["backsurfaceshader", "displacementshader"];
                    if skip_names.contains(&input.get_name()) {
                        continue;
                    }
                    let value = get_input_value_network(input, &shader.graph, syntax, &net_ctx);
                    if value == "null_closure()" {
                        continue;
                    }
                    let type_name = input.port().get_type().get_name();
                    let syn_type = syntax
                        .get_type_name(input.port().get_type())
                        .unwrap_or("float");
                    if type_name == "vector2" {
                        let parts: Vec<&str> = value.split_whitespace().collect();
                        if parts.len() >= 2 {
                            lines.push(param_string(
                                syn_type,
                                &format!("{}.x", input_name),
                                parts[0],
                            ));
                            lines.push(param_string(
                                syn_type,
                                &format!("{}.y", input_name),
                                parts[1],
                            ));
                        }
                    } else if type_name == "vector4" {
                        let parts: Vec<&str> = value.split_whitespace().collect();
                        if parts.len() >= 4 {
                            lines.push(param_string(
                                syn_type,
                                &format!("{}.x", input_name),
                                parts[0],
                            ));
                            lines.push(param_string(
                                syn_type,
                                &format!("{}.y", input_name),
                                parts[1],
                            ));
                            lines.push(param_string(
                                syn_type,
                                &format!("{}.z", input_name),
                                parts[2],
                            ));
                            lines.push(param_string(
                                syn_type,
                                &format!("{}.w", input_name),
                                parts[3],
                            ));
                        }
                    } else if type_name == "color4" {
                        let parts: Vec<&str> = value.split_whitespace().collect();
                        if parts.len() >= 4 {
                            let rgb = format!("{} {} {}", parts[0], parts[1], parts[2]);
                            lines.push(param_string("color", &format!("{}.rgb", input_name), &rgb));
                            lines.push(param_string(
                                syn_type,
                                &format!("{}.a", input_name),
                                parts[3],
                            ));
                        }
                    } else {
                        lines.push(param_string(syn_type, &input_name, &value));
                    }
                }
            } else {
                let (up_node, up_out) = connection.unwrap();
                let mut conn_name = up_out;
                syntax.make_valid_name(&mut conn_name);
                connections.push(connect_string(&up_node, &conn_name, node_name, &input_name));
            }
        }

        // Track last output for validation
        last_output = Some(node_name.clone());
        lines.push(format!("shader {} {} ;", oso_name, node_name));
    }

    // Validate that at least one node was processed (C++ lastOutput null check)
    if last_output.is_none() {
        eprintln!("Invalid shader");
        return Err("Invalid shader: no nodes processed".to_string());
    }

    let stage = shader
        .get_stage_by_name_mut(shader_stage::PIXEL)
        .ok_or("Pixel stage missing")?;
    for line in lines {
        stage.append_line(&line);
    }
    for conn in connections {
        stage.append_line(&conn);
    }

    let oso_path_str: String = oso_paths
        .iter()
        .filter_map(|p| {
            context
                .resolve_source_file(p, None)
                .map(|fp| fp.as_str().replace('\\', "/"))
        })
        .collect::<Vec<_>>()
        .join(",");
    if !oso_path_str.is_empty() {
        shader.set_attribute("osoPath", Value::String(oso_path_str));
    }

    Ok(shader)
}

fn param_string(param_type: &str, param_name: &str, param_value: &str) -> String {
    format!("param {} {} {} ;", param_type, param_name, param_value)
}

fn connect_string(from_node: &str, from_name: &str, to_node: &str, to_name: &str) -> String {
    // C++: "connect " + fromNode + "." + fromName + " " + toNode + "." + toName + " ;"
    format!(
        "connect {}.{} {}.{} ;",
        from_node, from_name, to_node, to_name
    )
}

fn get_input_value_network(
    input: &crate::gen_shader::ShaderInput,
    graph: &ShaderGraph,
    syntax: &crate::gen_shader::Syntax,
    _ctx: &OslNetworkShaderGraphContext<'_>,
) -> String {
    if let Some((up_node, up_out)) = input.get_connection() {
        if let Some(var) = graph.get_connection_variable(up_node, up_out) {
            return var;
        }
    }
    let port = input.port();
    let val = port.get_value_string();
    if !val.is_empty() {
        if let Some(v) = port.get_value() {
            return syntax.get_value_network(port.get_type(), v, false);
        }
        return val.replace(',', " ");
    }
    syntax
        .get_default_value(port.get_type(), false)
        .replace(',', " ")
}
