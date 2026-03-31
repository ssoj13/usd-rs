//! OSL emit — full generate() pipeline (по рефу OslShaderGenerator::generate).

use crate::core::Document;
use crate::core::ElementPtr;
use crate::gen_shader::{GenContext, ShaderGraphCreateContext};
use crate::gen_shader::{Shader, ShaderGraph, ShaderStage, VariableBlock, shader_stage};

use super::osl_block;
use super::osl_shader_generator::{OslShaderGenerator, OslShaderGraphContext};
use super::osl_syntax::OslSyntax;

/// Token for file UV transform (по рефу ShaderGenerator::T_FILE_TRANSFORM_UV).
const T_FILE_TRANSFORM_UV: &str = "$fileTransformUv";

const INCLUDE_FILES: &[&str] = &["mx_funcs.h"];

/// UI widget metadata by type (по рефу emitMetadata UI_WIDGET_METADATA).
fn osl_widget_for_type(type_name: &str) -> Option<&'static str> {
    match type_name {
        "float" | "integer" => Some("number"),
        "boolean" => Some("checkBox"),
        "filename" => Some("filename"),
        _ => None,
    }
}

/// Types that should not get custom metadata in [[ ]] (по рефу METADATA_TYPE_BLACKLIST).
fn metadata_type_blacklisted(type_name: &str) -> bool {
    matches!(
        type_name,
        "vector2" | "vector4" | "color4" | "filename" | "bsdf"
    )
}

/// Emit metadata suffix for a port (по рефу emitMetadata).
/// C++: emits metadata entries with proper type names from syntax, widget metadata,
/// and geomprop. Checks METADATA_TYPE_BLACKLIST on the metadata entry type (not port type).
fn emit_metadata_suffix(
    port: &crate::gen_shader::ShaderPort,
    _type_name: &str,
    syntax: &crate::gen_shader::Syntax,
) -> String {
    let port_type_name = port.get_type().get_name();
    let widget_meta = osl_widget_for_type(port_type_name);
    let metadata = port.get_metadata();
    let geomprop = &port.geomprop;

    let has_content = widget_meta.is_some() || !metadata.is_empty() || !geomprop.is_empty();
    if !has_content {
        return String::new();
    }

    let mut lines: Vec<String> = Vec::new();

    // Port metadata entries with proper type/value from syntax (C++ emitMetadata)
    for (j, m) in metadata.iter().enumerate() {
        // C++ checks METADATA_TYPE_BLACKLIST on the metadata entry's type, not the port type
        if metadata_type_blacklisted(&m.name) {
            continue;
        }
        // Use syntax to get type name and value for metadata
        let data_type = if m.value.starts_with('"') {
            "string"
        } else {
            "string"
        };
        let delim = if widget_meta.is_some() || j < metadata.len() - 1 {
            ","
        } else {
            ""
        };
        lines.push(format!("{} {} = {}{}", data_type, m.name, m.value, delim));
    }

    // Widget metadata
    if let Some(widget) = widget_meta {
        let delim = if !geomprop.is_empty() { "," } else { "" };
        let data_type = syntax
            .get_type_name(&syntax.type_system.get_type("string"))
            .unwrap_or("string");
        let data_value = format!("\"{}\" ", widget);
        lines.push(format!(
            "{} widget = {}{}",
            data_type,
            data_value.trim(),
            delim
        ));
    }

    // Geomprop
    if !geomprop.is_empty() {
        let data_type = syntax
            .get_type_name(&syntax.type_system.get_type("string"))
            .unwrap_or("string");
        lines.push(format!(
            "{} mtlx_defaultgeomprop = \"{}\"",
            data_type, geomprop
        ));
    }

    if lines.is_empty() {
        return String::new();
    }

    // C++ emits scope begin/end [[ ]] with lines inside
    let mut result = String::new();
    result.push_str("\n[[ ");
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            result.push_str("\n");
        }
        result.push_str(line);
    }
    result.push_str(" ]]");
    result
}

/// GEOMPROP default values (по рефу OslShaderGenerator::emitShaderInputs GEOMPROP_DEFINITIONS).
fn geomprop_default(geomprop: &str) -> Option<&'static str> {
    match geomprop {
        "Pobject" => Some("transform(\"object\", P)"),
        "Pworld" => Some("P"),
        "Nobject" => Some("transform(\"object\", N)"),
        "Nworld" => Some("N"),
        "Tobject" => Some("transform(\"object\", dPdu)"),
        "Tworld" => Some("dPdu"),
        "Bobject" => Some("transform(\"object\", dPdv)"),
        "Bworld" => Some("dPdv"),
        "UV0" => Some("{u,v}"),
        "Vworld" => Some("I"),
        _ => None,
    }
}

/// getUpstreamResult — variable or default for output socket (по рефу ShaderGenerator::getUpstreamResult).
fn get_upstream_result(
    socket: &crate::gen_shader::ShaderInput,
    graph: &ShaderGraph,
    syntax: &OslSyntax,
) -> String {
    if let Some((up_node, up_out)) = socket.get_connection() {
        if let Some(var) = graph.get_connection_variable(up_node, up_out) {
            return var;
        }
    }
    let val = socket.port().get_value_string();
    if !val.is_empty() {
        return val;
    }
    syntax
        .get_syntax()
        .get_default_value(socket.port.get_type(), false)
}

/// Emit all dependent function calls (depth-first from outputs) — по рефу emitAllDependentFunctionCalls.
fn emit_all_dependent_function_calls(
    node_name: &str,
    graph: &ShaderGraph,
    doc: &Document,
    ctx: &OslShaderGraphContext<'_>,
    stage: &mut ShaderStage,
) {
    let node = match graph.get_node(node_name) {
        Some(n) => n,
        None => return,
    };
    if stage.is_function_call_emitted(node_name) {
        return;
    }
    for inp in node.get_inputs() {
        if let Some((up_n, _up_o)) = inp.get_connection() {
            if up_n != graph.get_name() {
                emit_all_dependent_function_calls(up_n, graph, doc, ctx, stage);
            }
        }
    }
    let node_def_name = match graph.get_node_def(node_name) {
        Some(nd) => nd,
        None => return,
    };
    let target = ctx.get_implementation_target();
    if let Some(impl_) = ctx.get_implementation_for_nodedef(doc, node_def_name, target) {
        impl_.emit_function_call(node, ctx, stage);
        stage.add_function_call_emitted(node_name);
    }
}

/// Emit library includes (по рефу emitLibraryIncludes).
fn emit_library_includes(ctx: &GenContext<OslShaderGenerator>, stage: &mut ShaderStage) {
    for file in INCLUDE_FILES {
        let path = match ctx.resolve_source_file(*file, None) {
            Some(p) => p,
            None => continue,
        };
        let path_str = path.as_str().replace('\\', "/");
        stage.append_line(&format!("#include \"{}\"", path_str));
    }
    stage.append_line("");
}

/// Emit type definitions from syntax (по рефу emitTypeDefinitions).
fn emit_type_definitions(syntax: &OslSyntax, stage: &mut ShaderStage) {
    let syn = syntax.get_syntax();
    // Sort by key for deterministic output (HashMap iteration is unordered)
    let mut entries: Vec<_> = syn.iter_type_syntax().collect();
    entries.sort_by_key(|(name, _)| name.as_str());
    for (_, type_syn) in entries {
        if !type_syn.type_definition.is_empty() {
            stage.append_line(&type_syn.type_definition);
        }
    }
}

/// Collect lines for shader inputs/uniforms block (avoids borrow conflict with stage).
/// C++ ref: OslShaderGenerator::emitShaderInputs.
fn collect_input_block_lines(
    inputs: &VariableBlock,
    syntax: &OslSyntax,
    has_more_after: bool,
) -> Vec<String> {
    let mut lines = Vec::new();
    let syn = syntax.get_syntax();
    let var_order: Vec<String> = inputs.get_variable_order().to_vec();
    for (i, var_name) in var_order.iter().enumerate() {
        let port = match inputs.find(var_name) {
            Some(p) => p,
            None => continue,
        };
        let type_name = syn
            .get_type_name(port.get_type())
            .unwrap_or("float")
            .to_string();
        let var = port.get_variable();

        if port.get_type().get_name() == "filename" {
            // C++: split filename into file string + colorspace string inputs
            let value_str = port
                .get_value()
                .map(|v| v.get_value_string())
                .unwrap_or_default();
            let cs = port.colorspace.as_str();
            // File string input with metadata
            let meta = emit_metadata_suffix(port, port.get_type().get_name(), syn);
            lines.push(format!("string {} = \"{}\"{},", var, value_str, meta));
            // Colorspace string input
            lines.push(format!("string {}_colorspace = \"{}\"", var, cs,));
            lines.push("[[ string widget = \"colorspace\" ]]".to_string());
        } else {
            // C++: value = _syntax->getValue(input, true); then geomprop override; then default fallback
            let mut value = if let Some(v) = port.get_value() {
                syn.get_value(port.get_type(), v, true)
            } else {
                String::new()
            };
            // C++: geomprop ALWAYS overrides value (not just when empty)
            let geomprop = &port.geomprop;
            if !geomprop.is_empty() {
                if let Some(def) = geomprop_default(geomprop) {
                    value = def.to_string();
                }
            }
            if value.is_empty() {
                value = syn.get_default_value(port.get_type(), false);
            }
            let meta = emit_metadata_suffix(port, port.get_type().get_name(), syn);
            lines.push(format!("{} {} = {}{}", type_name, var, value, meta));
        }

        // Append comma delimiter (always for all inputs in the block)
        if i < var_order.len() || has_more_after {
            if let Some(last) = lines.last_mut() {
                last.push(',');
            }
        }
    }
    lines
}

/// Collect lines for shader outputs block.
fn collect_output_block_lines(outputs: &VariableBlock, syntax: &OslSyntax) -> Vec<String> {
    let mut lines = Vec::new();
    let syn = syntax.get_syntax();
    let var_order: Vec<String> = outputs.get_variable_order().to_vec();
    for (i, var_name) in var_order.iter().enumerate() {
        let delim = if i < var_order.len() - 1 { "," } else { "" };
        let port = match outputs.find(var_name) {
            Some(p) => p,
            None => continue,
        };
        let type_name = syntax.get_output_type_name(port.get_type().get_name());
        let def_val = syn.get_default_value(port.get_type(), true);
        lines.push(format!(
            "output {} {} = {}{}",
            type_name,
            port.get_variable(),
            def_val,
            delim
        ));
    }
    lines
}

/// Replace tokens in stage source.
fn replace_tokens(stage: &mut ShaderStage, file_transform_uv: &str) {
    stage.source_code = stage
        .source_code
        .replace(T_FILE_TRANSFORM_UV, file_transform_uv);
}

/// Full generate pipeline — по рефу OslShaderGenerator::generate.
pub fn generate(
    name: &str,
    element: &ElementPtr,
    doc: &Document,
    context: &mut GenContext<OslShaderGenerator>,
) -> Result<Shader, String> {
    let shader = super::create_osl_shader(name, element, doc, context)?;
    let mut stage = shader
        .get_stage_by_name(shader_stage::PIXEL)
        .ok_or("Pixel stage missing")?
        .clone();
    let mut graph = shader.graph;
    let opts = context.get_options();

    if opts.osl_connect_ci_wrapper {
        let osl_ctx = OslShaderGraphContext::new(context);
        add_set_ci_terminal_node(&mut graph, doc, &osl_ctx)?;
    }

    let syntax = context.get_shader_generator().get_syntax();
    let osl_ctx_no_graph = OslShaderGraphContext::new(context);

    // emitLibraryIncludes
    emit_library_includes(context, &mut stage);

    // emitTypeDefinitions
    emit_type_definitions(syntax, &mut stage);
    stage.append_line("#define M_FLOAT_EPS 1e-8");
    stage.append_line(
        "closure color null_closure() { closure color null_closure = 0; return null_closure; }",
    );
    stage.append_line("");

    // Token substitution for file transform UV
    let file_transform_uv = if opts.file_texture_vertical_flip {
        "mx_transform_uv_vflip.osl"
    } else {
        "mx_transform_uv.osl"
    };

    // emitFunctionDefinitions (ctx without graph — get_implementation doesn't need it)
    let target = osl_ctx_no_graph.get_implementation_target();
    for node_name in &graph.node_order {
        let node = match graph.get_node(node_name) {
            Some(n) => n,
            None => continue,
        };
        let node_def_name = match graph.get_node_def(node_name) {
            Some(nd) => nd,
            None => continue,
        };
        if let Some(impl_) =
            osl_ctx_no_graph.get_implementation_for_nodedef(doc, node_def_name, target)
        {
            impl_.emit_function_definition(node, &osl_ctx_no_graph, &mut stage);
        }
    }

    // Shader type from first output
    let output0 = graph.get_output_socket_at(0).ok_or("No output socket")?;
    let out_type = output0.port.get_type().get_name();
    let shader_type = if out_type == "surfaceshader" {
        "surface"
    } else if out_type == "volumeshader" {
        "volume"
    } else {
        "shader"
    };
    stage.append_line(&format!("{} ", shader_type));

    // Function name
    let mut func_name = name.to_string();
    syntax
        .get_syntax()
        .make_identifier(&mut func_name, graph.get_identifier_map());
    stage.set_function_name(&func_name);
    stage.append_line(&func_name);

    // Metadata [[ ... ]] — C++ emitScopeBegin(DOUBLE_SQUARE_BRACKETS) + graph metadata
    let elem = element.borrow();
    let cat = elem.get_category().to_string();
    let qualified = elem.get_qualified_name(elem.get_name());
    drop(elem);

    let graph_metadata = &graph.node.metadata;
    let have_shader_metadata = !graph_metadata.is_empty();
    let _syn = syntax.get_syntax(); // used for metadata type names if needed

    stage.append_line("[[");
    stage.append_line(&format!(
        "string mtlx_category = \"{}\"{}",
        cat,
        "," // always comma (mtlx_name follows)
    ));
    stage.append_line(&format!(
        "string mtlx_name = \"{}\"{}",
        qualified,
        if have_shader_metadata { "," } else { "" }
    ));
    // Emit all graph-level metadata entries (C++ iterates ShaderMetadataVec)
    for (j, data) in graph_metadata.iter().enumerate() {
        let delim = if j == graph_metadata.len() - 1 {
            ""
        } else {
            ","
        };
        // data.name is the remapped name (e.g. "label"), data.value is pre-formatted
        let data_type = "string";
        stage.append_line(&format!(
            "{} {} = {}{}",
            data_type, data.name, data.value, delim
        ));
    }
    stage.append_line("]]");

    // Begin signature ( — inputs, uniforms, outputs all inside parens (по рефу)
    stage.append_line("(");

    let has_uniforms = stage
        .get_uniform_block(osl_block::UNIFORMS)
        .map(|b| !b.is_empty())
        .unwrap_or(false);
    let has_outputs = stage
        .get_output_block(osl_block::OUTPUTS)
        .map(|b| !b.is_empty())
        .unwrap_or(false);

    let input_lines = stage
        .get_input_block(osl_block::INPUTS)
        .map(|b| collect_input_block_lines(b, syntax, has_uniforms || has_outputs))
        .unwrap_or_default();
    let uniform_lines = stage
        .get_uniform_block(osl_block::UNIFORMS)
        .map(|b| collect_input_block_lines(b, syntax, has_outputs))
        .unwrap_or_default();
    let output_lines = stage
        .get_output_block(osl_block::OUTPUTS)
        .map(|b| collect_output_block_lines(b, syntax))
        .unwrap_or_default();

    for line in input_lines
        .into_iter()
        .chain(uniform_lines)
        .chain(output_lines)
    {
        stage.append_line(&line);
    }

    stage.append_line(")");
    stage.append_line(" {");

    // Constants
    if !stage.constants.is_empty() {
        let const_order: Vec<String> = stage.constants.get_variable_order().to_vec();
        for var_name in &const_order {
            if let Some(port) = stage.constants.find(var_name) {
                let type_name = syntax
                    .get_syntax()
                    .get_type_name(port.get_type())
                    .unwrap_or("float");
                let def = syntax
                    .get_syntax()
                    .get_default_value(port.get_type(), false);
                stage.append_line(&format!(
                    "    const {} {} = {};",
                    type_name,
                    port.get_variable(),
                    def
                ));
            }
        }
        stage.append_line("");
    }

    // Construct textureresource for filename uniforms (по рефу)
    // Update both stage uniforms and graph root outputs so get_connection_variable returns var_
    let mut filename_vars: Vec<String> = Vec::new();
    if let Some(uniforms) = stage.get_uniform_block(osl_block::UNIFORMS) {
        for var_name in uniforms.get_variable_order() {
            if let Some(port) = uniforms.find(var_name) {
                if port.get_type().get_name() == "filename" {
                    filename_vars.push(port.get_variable().to_string());
                }
            }
        }
    }
    for var in &filename_vars {
        stage.append_line(&format!(
            "    textureresource {}_ = {{ {}, {}_colorspace }};",
            var, var, var
        ));
    }
    for var in &filename_vars {
        let new_var = format!("{}_", var);
        if let Some(blk) = stage.get_uniform_block_mut(osl_block::UNIFORMS) {
            if let Some(port) = blk.find_mut(var) {
                port.set_variable(&new_var);
            }
        }
        // Update graph root outputs so get_connection_variable returns new name
        for out_port in graph.node.outputs.values_mut() {
            if out_port.port.get_variable() == var {
                out_port.port.set_variable(&new_var);
                break;
            }
        }
    }

    // Emit all dependent function calls (ctx with graph for connection resolution)
    let osl_ctx = OslShaderGraphContext::with_graph(context, &graph);
    for i in 0..graph.num_output_sockets() {
        let socket = match graph.get_output_socket_at(i) {
            Some(s) => s,
            None => continue,
        };
        if let Some((up_n, _)) = socket.get_connection() {
            if up_n != graph.get_name() {
                emit_all_dependent_function_calls(up_n, &graph, doc, &osl_ctx, &mut stage);
            }
        }
    }

    // Assign results to outputs
    for i in 0..graph.num_output_sockets() {
        let socket = match graph.get_output_socket_at(i) {
            Some(s) => s,
            None => continue,
        };
        let rhs = get_upstream_result(socket, &graph, syntax);
        stage.append_line(&format!("    {} = {};", socket.port.get_variable(), rhs));
    }

    stage.append_line("}");

    // Token substitution
    replace_tokens(&mut stage, file_transform_uv);

    Ok(Shader::from_parts(name, graph, vec![stage]))
}

/// addSetCiTerminalNode — по рефу OslShaderGenerator::addSetCiTerminalNode.
/// Builds outputModeMap from nodedef inputs, inlines setCi node, sets output_mode value.
pub(crate) fn add_set_ci_terminal_node(
    graph: &mut ShaderGraph,
    doc: &Document,
    context: &dyn ShaderGraphCreateContext,
) -> Result<(), String> {
    const SET_CI_NODE_DEF: &str = "ND_osl_set_ci";
    let set_ci_def = doc
        .get_node_def(SET_CI_NODE_DEF)
        .ok_or_else(|| format!("NodeDef '{}' not found", SET_CI_NODE_DEF))?;

    // Build outputModeMap: type_name -> index for inputs starting with "input_"
    // C++: iterate nodedef inputs, map each input_<type> to sequential index
    let mut output_mode_map: std::collections::HashMap<String, i32> =
        std::collections::HashMap::new();
    let mut index: i32 = 0;
    {
        let def_borrow = set_ci_def.borrow();
        for child in def_borrow.get_children() {
            let child_b = child.borrow();
            let child_name = child_b.get_name().to_string();
            if child_name.starts_with("input_") {
                if let Some(input_type) = child_b.get_attribute("type") {
                    output_mode_map.insert(input_type.to_string(), index);
                }
                index += 1;
            }
        }
    }

    for i in 0..graph.num_output_sockets() {
        let socket = graph.get_output_socket_at(i).ok_or("Output socket")?;
        let out_type = socket.port.get_type().get_name().to_string();
        let new_node_name = graph.inline_node_before_output(
            i,
            "oslSetCi",
            SET_CI_NODE_DEF,
            &format!("input_{}", out_type),
            "out_ci",
            doc,
            context,
        )?;

        // C++: set the output_mode input value on the setCi node
        if let Some(&mode_value) = output_mode_map.get(&out_type) {
            if let Some(node) = graph.get_node_mut(&new_node_name) {
                if let Some(input) = node.inputs.get_mut("output_mode") {
                    input
                        .port_mut()
                        .set_value(Some(crate::core::Value::Integer(mode_value)), true);
                }
            }
        }
    }
    Ok(())
}
