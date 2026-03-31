//! MDL emit — generate pipeline (ref: MdlShaderGenerator::generate).

use crate::core::Document;
use crate::core::ElementPtr;
use crate::gen_shader::{Shader, ShaderNodeClassification, ShaderStage, shader_stage};

use super::mdl_shader_generator::{
    MdlShaderGenerator, MdlShaderGraphContext, create_mdl_shader, geomprop_default, mdl_block,
};
use crate::gen_shader::ShaderGraphCreateContext;
use std::collections::BTreeSet;

use super::custom_node_mdl::CustomCodeNodeMdl;
/// Default (unversioned) imports (ref: DEFAULT_IMPORTS).
const DEFAULT_IMPORTS: &[&str] = &[
    "import ::df::*",
    "import ::base::*",
    "import ::math::*",
    "import ::state::*",
    "import ::anno::*",
    "import ::tex::*",
    "using ::materialx::core import *",
    "using ::materialx::sampling import *",
];

/// Versioned import prefixes (ref: DEFAULT_VERSIONED_IMPORTS).
const DEFAULT_VERSIONED_IMPORTS: &[&str] =
    &["using ::materialx::stdlib_", "using ::materialx::pbrlib_"];

const IMPORT_ALL: &str = " import *";

/// Build input annotations string (ref: emitInputAnnotations).
/// Returns `\n[[\n    materialx::core::origin("path")\n    ,anno::unused()\n]]`
fn build_input_annotations(port_path: &str, has_connections: bool) -> String {
    let origin_anno = format!("materialx::core::origin(\"{}\")", port_path);
    let mut s = String::new();
    s.push_str("\n[[");
    s.push_str(&format!("\n    {}", origin_anno));
    if !has_connections {
        s.push_str(",");
        s.push_str("\n    anno::unused()");
    }
    s.push_str("\n]]");
    s
}

/// Emit all dependent function calls in topological order (MDL-specific — closure/texture).
fn emit_all_dependent_function_calls_mdl(
    node_name: &str,
    graph: &crate::gen_shader::ShaderGraph,
    doc: &Document,
    ctx: &MdlShaderGraphContext<'_>,
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
                emit_all_dependent_function_calls_mdl(up_n, graph, doc, ctx, stage);
            }
        }
    }
    let node_def_name = match graph.get_node_def(node_name) {
        Some(nd) => nd,
        None => return,
    };
    let target = super::mdl_shader_generator::TARGET;
    if let Some(impl_) = ctx.get_implementation_for_nodedef(doc, node_def_name, target) {
        impl_.emit_function_call(node, ctx, stage);
        stage.add_function_call_emitted(node_name);
    }
}

/// Emit type definitions: export typedef statements for types that have an alias.
/// Ref: MdlShaderGenerator::emitTypeDefinitions
fn emit_type_definitions(syntax: &crate::gen_shader::Syntax, stage: &mut ShaderStage) {
    // Sort by key for deterministic output (HashMap iteration is unordered)
    let mut entries: Vec<_> = syntax.iter_type_syntax().collect();
    entries.sort_by_key(|(name, _)| name.as_str());
    let mut any = false;
    for (_name, ts) in entries {
        if !ts.type_definition.is_empty() {
            stage.append_line(&format!("export {}", ts.type_definition));
            any = true;
        }
    }
    if any {
        stage.append_line("");
    }
}

fn collect_custom_imports(
    graph: &crate::gen_shader::ShaderGraph,
    doc: &Document,
    ctx: &MdlShaderGraphContext<'_>,
) -> BTreeSet<String> {
    let mut imports = BTreeSet::new();
    let target = super::mdl_shader_generator::TARGET;

    for node_name in &graph.node_order {
        let Some(node_def_name) = graph.get_node_def(node_name) else {
            continue;
        };
        let Some(node_def) = doc.get_node_def(node_def_name) else {
            continue;
        };
        let Some(impl_elem) =
            crate::core::get_implementation_for_nodedef(&node_def, doc, target, false)
        else {
            continue;
        };
        if !CustomCodeNodeMdl::is_custom_impl_element(&impl_elem) {
            continue;
        }

        let impl_ref = impl_elem.borrow();
        let file_attr = impl_ref
            .get_attribute("file")
            .unwrap_or_default()
            .to_string();
        let impl_name = impl_ref.get_name().to_string();
        drop(impl_ref);

        let import_name =
            CustomCodeNodeMdl::qualified_module_name_from_file(&file_attr, ctx, &impl_name);
        if !import_name.is_empty() {
            imports.insert(format!("import {}::*", import_name));
        }
    }

    imports
}

/// Generate MDL shader (ref: MdlShaderGenerator::generate).
pub fn generate(
    name: &str,
    element: &ElementPtr,
    doc: &Document,
    context: &mut crate::gen_shader::GenContext<MdlShaderGenerator>,
) -> Result<Shader, String> {
    // MDL cannot cache node implementations between generation calls
    context.clear_node_implementations();
    let shader = create_mdl_shader(name, element, doc, context)?;
    let (mut graph, stages) = shader.into_parts();
    let mut stage = stages
        .into_iter()
        .find(|s| s.name == shader_stage::PIXEL)
        .ok_or("Pixel stage missing")?;

    let syntax = context.get_shader_generator().get_syntax().get_syntax();
    let version_suffix = context
        .get_shader_generator()
        .get_mdl_version_filename_suffix();
    let version_number = context.get_shader_generator().get_mdl_version_number();

    let mut func_name = name.to_string();
    syntax.make_identifier(&mut func_name, graph.get_identifier_map());

    // ---- Emit MDL version number (ref: emitMdlVersionNumber) ----
    stage.append_line(&format!("mdl {}", version_number));
    stage.append_line("");

    // ---- Emit default imports (ref: DEFAULT_IMPORTS) ----
    for module in DEFAULT_IMPORTS {
        stage.append_line(module);
    }

    // ---- Emit versioned imports with suffix (ref: DEFAULT_VERSIONED_IMPORTS) ----
    for module in DEFAULT_VERSIONED_IMPORTS {
        stage.append_line(&format!("{}{}{}", module, version_suffix, IMPORT_ALL));
    }

    // ---- Emit custom node imports (ref: CustomCodeNodeMdl::getQualifiedModuleName) ----
    let mdl_ctx = MdlShaderGraphContext::with_graph(context, &graph);
    for import_line in collect_custom_imports(&graph, doc, &mdl_ctx) {
        stage.append_line(&import_line);
    }
    stage.append_line("");

    // ---- Emit type definitions (ref: emitTypeDefinitions) ----
    emit_type_definitions(syntax, &mut stage);

    // ---- Emit function definitions for all nodes ----
    for node_name in &graph.node_order.clone() {
        let node = match graph.get_node(node_name) {
            Some(n) => n,
            None => continue,
        };
        let node_def_name = match graph.get_node_def(node_name) {
            Some(nd) => nd.to_string(),
            None => continue,
        };
        let target = super::mdl_shader_generator::TARGET;
        if let Some(impl_) = mdl_ctx.get_implementation_for_nodedef(doc, &node_def_name, target) {
            impl_.emit_function_definition(node, &mdl_ctx, &mut stage);
        }
    }

    // ---- Emit shader type (ref: "export material") ----
    stage.set_function_name(&func_name);
    stage.append_line(&format!("export material {}", func_name));
    stage.append_line("(");

    // ---- Emit shader inputs with annotations (ref: emitShaderInputs + emitInputAnnotations) ----
    let input_lines: Vec<String> = stage
        .get_input_block(mdl_block::INPUTS)
        .map(|inputs| {
            let var_order = inputs.get_variable_order();
            let count = var_order.len();
            var_order
                .iter()
                .enumerate()
                .filter_map(|(i, var_name)| {
                    let port = inputs.find(var_name)?;
                    let type_name = syntax.get_type_name(port.get_type()).unwrap_or("float");
                    let uniform_prefix =
                        if port.is_uniform() || port.get_type().get_name() == "filename" {
                            "uniform "
                        } else {
                            ""
                        };
                    // Resolve value: geomprop > value > default
                    let value = if !port.geomprop.is_empty() {
                        geomprop_default(&port.geomprop)
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| {
                                if port.get_value_string().is_empty() {
                                    syntax.get_default_value(port.get_type(), true)
                                } else {
                                    port.get_value_string()
                                }
                            })
                    } else if let Some(v) = port.get_value() {
                        syntax.get_value(port.get_type(), v, true)
                    } else {
                        syntax.get_default_value(port.get_type(), true)
                    };

                    let mut line = format!(
                        "    {}{} {} = {}",
                        uniform_prefix,
                        type_name,
                        port.get_variable(),
                        value
                    );

                    // Append input annotations (ref: emitInputAnnotations)
                    if !port.path.is_empty() {
                        let has_conn = !port.geomprop.is_empty() || port.get_value().is_some(); // approximate: C++ checks connections
                        line.push_str(&build_input_annotations(&port.path, has_conn));
                    }

                    // Comma between params
                    if i < count - 1 {
                        line.push(',');
                    }
                    Some(line)
                })
                .collect()
        })
        .unwrap_or_default();
    for line in &input_lines {
        stage.append_line(line);
    }

    stage.append_line(")");

    // ---- Begin shader body: = let { ... } ----
    stage.append_line("= let");
    stage.append_line("{");

    // ---- Emit filtered shader/closure/material inputs inside let block ----
    // These are inputs that were filtered from the public interface (shader/closure/material)
    {
        let graph_name = graph.get_name().to_string();
        for i in 0..graph.num_input_sockets() {
            let socket = match graph.get_input_socket_at(i) {
                Some(s) => s,
                None => continue,
            };
            let ty = socket.port.get_type();
            let has_connections = !graph
                .get_connections_for_output(&graph_name, socket.get_name())
                .is_empty();
            let is_shader_like = ty.is_closure()
                || ty.get_semantic() == crate::gen_shader::Semantic::Shader
                || ty.get_semantic() == crate::gen_shader::Semantic::Material;
            if has_connections && is_shader_like {
                let uniform_prefix = if socket.port.is_uniform() || ty.get_name() == "filename" {
                    "uniform "
                } else {
                    ""
                };
                let type_name = syntax.get_type_name(ty).unwrap_or("material");
                let default_val = syntax.get_default_value(ty, true);
                stage.append_line(&format!(
                    "    {}{} {} = {}",
                    uniform_prefix,
                    type_name,
                    socket.port.get_variable(),
                    default_val
                ));
            }
        }
    }

    // ---- Emit texture function calls first (ref: emitFunctionCalls TEXTURE) ----
    let mdl_ctx = MdlShaderGraphContext::with_graph(context, &graph);
    for node_name in &graph.node_order.clone() {
        if let Some(node) = graph.get_node(node_name) {
            if node.has_classification(ShaderNodeClassification::TEXTURE) {
                emit_all_dependent_function_calls_mdl(node_name, &graph, doc, &mdl_ctx, &mut stage);
            }
        }
    }

    // ---- Emit root closure/shader function calls (ref: per-output-socket closure emit) ----
    for i in 0..graph.num_output_sockets() {
        let output_socket = match graph.get_output_socket_at(i) {
            Some(s) => s,
            None => continue,
        };
        if let Some((up_node_name, _)) = output_socket.get_connection() {
            let up_node_name = up_node_name.to_string();
            if let Some(up_node) = graph.get_node(&up_node_name) {
                let is_sibling = up_node
                    .get_parent_name()
                    .map(|p| p == graph.get_name())
                    .unwrap_or(true);
                if is_sibling
                    && (up_node.has_classification(ShaderNodeClassification::CLOSURE)
                        || up_node.has_classification(ShaderNodeClassification::SHADER))
                {
                    let mdl_ctx2 = MdlShaderGraphContext::with_graph(context, &graph);
                    emit_all_dependent_function_calls_mdl(
                        &up_node_name,
                        &graph,
                        doc,
                        &mdl_ctx2,
                        &mut stage,
                    );
                }
            }
        }
    }

    // ---- Get final result from first output socket ----
    let output_socket = graph.get_output_socket_at(0).ok_or("No output socket")?;
    let result = if let Some((up_n, up_o)) = output_socket.get_connection() {
        // Use MDL-specific upstream result for multi-output handling
        super::mdl_shader_generator::get_upstream_result_mdl(
            "",
            Some((up_n, up_o)),
            &graph,
            context.get_shader_generator().get_syntax(),
        )
        .unwrap_or_else(|| {
            graph
                .get_connection_variable(up_n, up_o)
                .unwrap_or_else(|| syntax.get_default_value(output_socket.port.get_type(), false))
        })
    } else {
        syntax.get_default_value(output_socket.port.get_type(), false)
    };

    let output_type_name = output_socket.port.get_type().get_name().to_string();

    // ---- Output type branching (ref: textureMaterial vs shaderMaterial) ----
    if graph.has_classification(ShaderNodeClassification::TEXTURE) {
        // Texture graph: convert result to color for visualization via emission
        if output_type_name == "displacementshader" {
            stage.append_line(&format!(
                "    float3 displacement__ = {}.geometry.displacement",
                result
            ));
            stage.append_line(concat!(
                "    color finalOutput__ = mk_color3(",
                "r: math::dot(displacement__, state::texture_tangent_u(0)),",
                "g: math::dot(displacement__, state::texture_tangent_v(0)),",
                "b: math::dot(displacement__, state::normal()))"
            ));
        } else {
            stage.append_line("    float3 displacement__ = float3(0.0)");

            let final_output = match output_type_name.as_str() {
                "boolean" => format!(
                    "{} ? mk_color3(0.0, 1.0, 0.0) : mk_color3(1.0, 0.0, 0.0)",
                    result
                ),
                "integer" => format!("mk_color3({} / 100)", result),
                "float" => format!("mk_color3({})", result),
                "vector2" => format!("mk_color3({}.x, {}.y, 0.0)", result, result),
                "vector3" => format!("mk_color3({})", result),
                "vector4" => format!("mk_color3({}.x, {}.y, {}.z)", result, result, result),
                "color3" => result.clone(),
                "color4" => format!("{}.rgb", result),
                "matrix33" | "matrix44" => format!(
                    "mk_color3({}[0][0], {}[1][1], {}[2][2])",
                    result, result, result
                ),
                _ => "mk_color3(0.0)".to_string(),
            };

            stage.append_line(&format!("    color finalOutput__ = {}", final_output));
        }

        // End shader body
        stage.append_line("}");

        // textureMaterial block (ref: static const string textureMaterial)
        stage.append_line("in material");
        stage.append_line("(");
        stage.append_line("    surface: material_surface(");
        stage.append_line("        emission : material_emission(");
        stage.append_line("            emission : df::diffuse_edf(),");
        stage.append_line("            intensity : finalOutput__ * math::PI,");
        stage.append_line("            mode : intensity_radiant_exitance");
        stage.append_line("        )");
        stage.append_line("    ),");
        stage.append_line("    geometry: material_geometry(");
        stage.append_line("       displacement : displacement__");
        stage.append_line("    )");
        stage.append_line(");");
    } else {
        // Shader/closure graph: emit finalOutput__ with proper type name
        let type_syntax_name = syntax
            .get_type_name(output_socket.port.get_type())
            .unwrap_or("material");
        stage.append_line(&format!(
            "    {} finalOutput__ = {}",
            type_syntax_name, result
        ));

        // End shader body
        stage.append_line("}");

        // shaderMaterial block
        stage.append_line("in material(finalOutput__);");
    }

    // Token substitution (ref: replaceTokens)
    let source = stage.get_source_code().to_string();
    if source.contains("{{MDL_VERSION_SUFFIX}}") {
        let replaced = source.replace("{{MDL_VERSION_SUFFIX}}", version_suffix);
        stage.set_source_code(&replaced);
    }

    Ok(Shader::from_parts(name, graph, vec![stage]))
}
