//! createShader -- HW shader creation (by ref HwShaderGenerator::createShader).

use crate::core::Document;
use crate::core::ElementPtr;
use crate::core::Value;
use crate::core::element::category;
use crate::gen_hw::hw_constants::{attr, block, ident, token};
use crate::gen_shader::type_desc_types;
use crate::gen_shader::{
    HwDirectionalAlbedoMethod, HwSpecularEnvironmentMethod, Shader, ShaderGraph,
    ShaderGraphCreateContext, ShaderNodeClassification, add_default_geom_node, add_stage_input,
    add_stage_output, add_stage_uniform, add_stage_uniform_with_value, create_from_element,
    create_from_nodegraph, shader_stage,
};

/// Create ShaderGraph from element (Output, Node, or NodeGraph).
fn create_graph_from_element(
    name: &str,
    element: &ElementPtr,
    doc: &Document,
    context: &dyn ShaderGraphCreateContext,
) -> Result<ShaderGraph, String> {
    let cat = element.borrow().get_category().to_string();
    if cat == category::NODE_GRAPH {
        create_from_nodegraph(element, doc, context)
    } else {
        create_from_element(name, element, doc, context)
    }
}

/// Add geom nodes for input sockets with defaultgeomprop (C++ createShader lines 98-121).
/// C++ iterates graph->getInputSockets() — these are the graph's INPUT interface sockets.
/// If an input has defaultgeomprop="Nobject", we break its internal connections and
/// insert a geompropvalue node that generates that geometric data.
fn add_geom_nodes_for_input_sockets(
    graph: &mut ShaderGraph,
    doc: &Document,
    context: &dyn ShaderGraphCreateContext,
) -> bool {
    let graph_name = graph.get_name().to_string();
    let mut geom_added = false;

    // C++: for (ShaderGraphInputSocket* socket : graph->getInputSockets())
    // Graph input sockets are stored as node outputs (see ShaderGraph convention).
    for in_name in graph.node.output_order.clone() {
        let socket = match graph.node.outputs.get(&in_name) {
            Some(s) => s,
            None => continue,
        };
        let geom_prop = socket.port.geomprop.clone();
        if geom_prop.is_empty() {
            continue;
        }
        let geom_def = match doc.get_geom_prop_def(&geom_prop) {
            Some(g) => g,
            None => continue,
        };
        // C++: socket->getConnections() — downstream inputs connected to this graph input
        let connections = graph.get_connections_for_output(&graph_name, &in_name);
        for (down_node, down_input) in connections {
            graph.break_connection(&down_node, &down_input);
            add_default_geom_node(graph, &down_node, &down_input, &geom_def, doc, context);
            geom_added = true;
        }
    }
    geom_added
}

/// Check if graph requires lighting (C++ HwShaderGenerator::requiresLighting).
/// Returns true for BSDF graphs or lit surface shaders.
fn requires_lighting(graph: &ShaderGraph) -> bool {
    let is_bsdf = graph.has_classification(ShaderNodeClassification::BSDF);
    let is_lit_surface = graph.has_classification(ShaderNodeClassification::SHADER)
        && graph.has_classification(ShaderNodeClassification::SURFACE)
        && !graph.has_classification(ShaderNodeClassification::UNLIT);
    is_bsdf || is_lit_surface
}

/// Create HW shader (by ref HwShaderGenerator::createShader).
pub fn create_shader(
    name: &str,
    element: &ElementPtr,
    doc: &Document,
    context: &dyn ShaderGraphCreateContext,
) -> Result<Shader, String> {
    let mut graph = create_graph_from_element(name, element, doc, context)?;

    if add_geom_nodes_for_input_sockets(&mut graph, doc, context) {
        graph.topological_sort();
    }

    let opts = context.get_options();
    let mut shader = Shader::new_hw(name, graph);

    // Create vertex stage
    {
        let vs = shader
            .get_stage_by_name_mut(shader_stage::VERTEX)
            .ok_or("Vertex stage missing")?;
        let vs_inputs = vs.create_input_block(block::VERTEX_INPUTS, "i_vs");
        vs_inputs.add(
            type_desc_types::vector3(),
            token::T_IN_POSITION,
            None,
            false,
        );
        let vs_prv = vs.create_uniform_block(block::PRIVATE_UNIFORMS, "u_prv");
        vs_prv.add(
            type_desc_types::matrix44(),
            token::T_WORLD_MATRIX,
            None,
            false,
        );
        vs_prv.add(
            type_desc_types::matrix44(),
            token::T_VIEW_PROJECTION_MATRIX,
            None,
            false,
        );
        vs.create_uniform_block(block::PUBLIC_UNIFORMS, "u_pub");
    }

    // Collect graph interface info before mutable borrow
    let graph = shader.get_graph();
    let input_uniforms: Vec<_> = (0..graph.num_input_sockets())
        .filter_map(|i| {
            let socket = graph.get_input_socket_at(i)?;
            let has_connections = !graph
                .get_connections_for_output(graph.get_name(), socket.get_name())
                .is_empty();
            if has_connections && graph.is_editable(socket.get_name()) {
                Some((
                    socket.port.get_type().clone(),
                    socket.port.get_variable().to_string(),
                ))
            } else {
                None
            }
        })
        .collect();
    let output_socket = graph
        .get_output_socket_at(0)
        .ok_or("Graph has no output socket")?;
    let _out_type = output_socket.port.get_type().clone();
    let out_name = output_socket.port.get_name().to_string();
    let out_var = output_socket.port.get_variable().to_string();
    let out_path = output_socket.port.path.clone();
    let graph_requires_lighting = requires_lighting(graph);

    // Collect FILETEXTURE nodes with unconnected filename inputs
    let file_tex_uniforms = collect_file_texture_uniforms(graph);

    // C++ getLightDataTypevarString(): WGSL uses "light_type" (reserved word avoidance)
    let light_type_var = context.get_light_data_type_var_string();

    // Create pixel stage
    {
        let ps = shader
            .get_stage_by_name_mut(shader_stage::PIXEL)
            .ok_or("Pixel stage missing")?;
        let _ps_outputs = ps.create_output_block(block::PIXEL_OUTPUTS, "o_ps");
        let _ps_prv = ps.create_uniform_block(block::PRIVATE_UNIFORMS, "u_prv");
        let _ps_pub = ps.create_uniform_block(block::PUBLIC_UNIFORMS, "u_pub");

        // Create LightData block with type discriminator
        // C++: lightData->add(Type::INTEGER, getLightDataTypevarString())
        let light_data = ps.create_uniform_block(block::LIGHT_DATA, ident::LIGHT_DATA_INSTANCE);
        light_data.add(type_desc_types::integer(), light_type_var, None, false);

        // Transparent rendering uniforms
        if opts.hw_transparency {
            add_stage_uniform_with_value(
                block::PRIVATE_UNIFORMS,
                type_desc_types::float(),
                token::T_ALPHA_THRESHOLD,
                ps,
                Some(Value::Float(0.001)),
            );
        }

        // Shadow map uniforms with default values
        if opts.hw_shadow_map {
            add_stage_uniform(
                block::PRIVATE_UNIFORMS,
                type_desc_types::filename(),
                token::T_SHADOW_MAP,
                ps,
            );
            // C++: Value::createValue(Matrix44::IDENTITY)
            add_stage_uniform_with_value(
                block::PRIVATE_UNIFORMS,
                type_desc_types::matrix44(),
                token::T_SHADOW_MATRIX,
                ps,
                Some(Value::Matrix44(crate::core::Matrix44::IDENTITY)),
            );
        }

        // Ambient occlusion uniforms
        if opts.hw_ambient_occlusion {
            add_stage_input(
                block::VERTEX_DATA,
                type_desc_types::vector2(),
                &format!("{}_0", token::T_TEXCOORD),
                ps,
                true,
            );
            add_stage_uniform(
                block::PRIVATE_UNIFORMS,
                type_desc_types::filename(),
                token::T_AMB_OCC_MAP,
                ps,
            );
            add_stage_uniform_with_value(
                block::PRIVATE_UNIFORMS,
                type_desc_types::float(),
                token::T_AMB_OCC_GAIN,
                ps,
                Some(Value::Float(1.0)),
            );
        }

        // IBL / environment lighting uniforms -- guarded by requiresLighting
        // C++: if (requiresLighting(*graph) && options.hwSpecularEnvironmentMethod != NONE)
        if graph_requires_lighting
            && opts.hw_specular_environment_method != HwSpecularEnvironmentMethod::None
        {
            // C++: yRotationPI = Matrix44::createScale(Vector3(-1, 1, -1))
            add_stage_uniform_with_value(
                block::PRIVATE_UNIFORMS,
                type_desc_types::matrix44(),
                token::T_ENV_MATRIX,
                ps,
                Some(Value::Matrix44(crate::core::Matrix44::Y_ROTATION_PI)),
            );
            add_stage_uniform(
                block::PRIVATE_UNIFORMS,
                type_desc_types::filename(),
                token::T_ENV_RADIANCE,
                ps,
            );
            add_stage_uniform_with_value(
                block::PRIVATE_UNIFORMS,
                type_desc_types::float(),
                token::T_ENV_LIGHT_INTENSITY,
                ps,
                Some(Value::Float(1.0)),
            );
            add_stage_uniform_with_value(
                block::PRIVATE_UNIFORMS,
                type_desc_types::integer(),
                token::T_ENV_RADIANCE_MIPS,
                ps,
                Some(Value::Integer(1)),
            );
            add_stage_uniform_with_value(
                block::PRIVATE_UNIFORMS,
                type_desc_types::integer(),
                token::T_ENV_RADIANCE_SAMPLES,
                ps,
                Some(Value::Integer(16)),
            );
            add_stage_uniform(
                block::PRIVATE_UNIFORMS,
                type_desc_types::filename(),
                token::T_ENV_IRRADIANCE,
                ps,
            );
            add_stage_uniform(
                block::PRIVATE_UNIFORMS,
                type_desc_types::boolean(),
                token::T_REFRACTION_TWO_SIDED,
                ps,
            );
        }

        // Albedo table uniforms for directional albedo precomputation
        if opts.hw_directional_albedo_method == HwDirectionalAlbedoMethod::Table
            || opts.hw_write_albedo_table
        {
            add_stage_uniform(
                block::PRIVATE_UNIFORMS,
                type_desc_types::filename(),
                token::T_ALBEDO_TABLE,
                ps,
            );
            add_stage_uniform_with_value(
                block::PRIVATE_UNIFORMS,
                type_desc_types::integer(),
                token::T_ALBEDO_TABLE_SIZE,
                ps,
                Some(Value::Integer(64)),
            );
        }

        // Environment prefilter uniforms -- with default env_matrix and env_radiance_mips
        if opts.hw_write_env_prefilter {
            add_stage_uniform(
                block::PRIVATE_UNIFORMS,
                type_desc_types::filename(),
                token::T_ENV_RADIANCE,
                ps,
            );
            add_stage_uniform_with_value(
                block::PRIVATE_UNIFORMS,
                type_desc_types::float(),
                token::T_ENV_LIGHT_INTENSITY,
                ps,
                Some(Value::Float(1.0)),
            );
            add_stage_uniform_with_value(
                block::PRIVATE_UNIFORMS,
                type_desc_types::integer(),
                token::T_ENV_PREFILTER_MIP,
                ps,
                Some(Value::Integer(1)),
            );
            // C++: yRotationPI = Matrix44::createScale(Vector3(-1, 1, -1))
            add_stage_uniform_with_value(
                block::PRIVATE_UNIFORMS,
                type_desc_types::matrix44(),
                token::T_ENV_MATRIX,
                ps,
                Some(Value::Matrix44(crate::core::Matrix44::Y_ROTATION_PI)),
            );
            add_stage_uniform_with_value(
                block::PRIVATE_UNIFORMS,
                type_desc_types::integer(),
                token::T_ENV_RADIANCE_MIPS,
                ps,
                Some(Value::Integer(1)),
            );
        }

        // Create uniforms for the published graph interface
        for (type_desc, var) in &input_uniforms {
            add_stage_uniform(block::PUBLIC_UNIFORMS, type_desc.clone(), var, ps);
        }

        // Create pixel output -- color4 for rendering
        // C++: psOutputs->add(Type::COLOR4, outputSocket->getName())
        let out_block = ps.create_output_block(block::PIXEL_OUTPUTS, "o_ps");
        out_block.add(type_desc_types::color4(), &out_name, None, false);
        if let Some(p) = out_block.find_mut(&out_name) {
            p.set_variable(out_var.clone());
            p.path = out_path;
        }

        // FILETEXTURE filename-to-uniform conversion loop
        // C++: walks graph + subgraphs for FILETEXTURE nodes with unconnected filename inputs
        for (var_name, path, value) in &file_tex_uniforms {
            let port = add_stage_uniform_with_value(
                block::PUBLIC_UNIFORMS,
                type_desc_types::filename(),
                var_name,
                ps,
                value.clone(),
            );
            port.path = path.clone();
        }
    }

    // Add vertex-to-pixel connector block
    // C++: addStageConnectorBlock(HW::VERTEX_DATA, HW::T_VERTEX_DATA_INSTANCE, *vs, *ps)
    {
        let vs = shader
            .get_stage_by_name_mut(shader_stage::VERTEX)
            .ok_or("Vertex stage missing")?;
        vs.create_output_block(block::VERTEX_DATA, ident::VERTEX_DATA_INSTANCE);
        // Ambient occlusion texcoord connector
        if opts.hw_ambient_occlusion {
            add_stage_input(
                block::VERTEX_INPUTS,
                type_desc_types::vector2(),
                &format!("{}_0", token::T_IN_TEXCOORD),
                vs,
                true,
            );
            add_stage_output(
                block::VERTEX_DATA,
                type_desc_types::vector2(),
                &format!("{}_0", token::T_TEXCOORD),
                vs,
                true,
            );
        }
    }
    {
        let ps = shader
            .get_stage_by_name_mut(shader_stage::PIXEL)
            .ok_or("Pixel stage missing")?;
        ps.create_input_block(block::VERTEX_DATA, ident::VERTEX_DATA_INSTANCE);
    }

    // Create shader variables for all nodes
    create_variables(&mut shader, doc, context);

    // Flag shader as transparent if hwTransparency is enabled
    // C++: shader->setAttribute(HW::ATTR_TRANSPARENT)
    if opts.hw_transparency {
        shader.set_attribute(attr::ATTR_TRANSPARENT.to_string(), Value::Boolean(true));
    }

    Ok(shader)
}

/// Collect FILETEXTURE nodes' unconnected filename inputs as (variable, path, value) tuples.
/// C++: walks graphStack (graph + light shader graphs + subgraphs) for FILETEXTURE nodes.
fn collect_file_texture_uniforms(graph: &ShaderGraph) -> Vec<(String, String, Option<Value>)> {
    let mut result = Vec::new();

    // Walk all nodes in the graph
    for node_name in &graph.node_order {
        if let Some(node) = graph.get_node(node_name) {
            if node.has_classification(ShaderNodeClassification::FILETEXTURE) {
                for input in node.get_inputs() {
                    if !input.has_connection() && input.port.get_type().get_name() == "filename" {
                        let var = input.port.get_variable().to_string();
                        let path = input.port.path.clone();
                        let val = input.port.get_value().cloned();
                        result.push((var, path, val));
                    }
                }
            }
        }
    }

    result
}

/// Call create_variables for all nodes (by ref ShaderGenerator::createVariables).
fn create_variables(shader: &mut Shader, doc: &Document, context: &dyn ShaderGraphCreateContext) {
    let target = context.get_implementation_target();
    let node_order: Vec<String> = shader.get_graph().node_order.clone();
    let node_def_pairs: Vec<(String, String)> = node_order
        .iter()
        .filter_map(|n| {
            let nd = shader.get_graph().get_node_def(n)?.to_string();
            Some((n.clone(), nd))
        })
        .collect();
    for (node_name, node_def_name) in node_def_pairs {
        let impl_ = match context.get_implementation_for_nodedef(doc, &node_def_name, target) {
            Some(i) => i,
            None => continue,
        };
        impl_.create_variables(&node_name, context, shader);
    }
}
