//! SourceCodeNode — implementation using data-driven static source code from file or attribute.

use std::path::Path;

use crate::core::ElementPtr;
use crate::core::element::category;
use crate::core::util::replace_substrings;
use crate::format::{FilePath, read_file};

use super::gen_context::ShaderImplContext;
use super::shader::ShaderStage;
use super::shader_node::ShaderNode;
use super::shader_node_impl::ShaderNodeImpl;
use super::util::hash_string;

/// Implementation for nodes using static source code (from "sourcecode" attr or "file").
/// Default implementation for nodes without a custom ShaderNodeImpl.
#[derive(Debug)]
pub struct SourceCodeNode {
    name: String,
    hash: u64,
    function_name: String,
    function_source: String,
    source_filename: String,
    inlined: bool,
}

impl SourceCodeNode {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            hash: 0,
            function_name: String::new(),
            function_source: String::new(),
            source_filename: String::new(),
            inlined: false,
        }
    }

    pub fn create() -> Box<dyn ShaderNodeImpl> {
        Box::new(Self::new())
    }

    fn resolve_source_code(
        &mut self,
        element: &ElementPtr,
        context: &dyn ShaderImplContext,
    ) -> Result<(), String> {
        let file_attr = element.borrow().get_attribute_or_empty("file");
        if file_attr.is_empty() {
            return Err("Implementation has no 'file' attribute".to_string());
        }

        let local_path = get_local_path_for_element(element);
        let resolved = context
            .resolve_source_file(&file_attr, local_path.as_ref())
            .ok_or_else(|| format!("Failed to resolve source file '{}'", file_attr))?;

        self.function_source = read_file(&resolved);
        self.source_filename = resolved.as_str().to_string();

        if self.function_source.is_empty() {
            return Err(format!(
                "Failed to get source code from file '{}' used by implementation '{}'",
                self.source_filename,
                element.borrow().get_name()
            ));
        }
        Ok(())
    }
}

impl Default for SourceCodeNode {
    fn default() -> Self {
        Self::new()
    }
}

impl ShaderNodeImpl for SourceCodeNode {
    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_hash(&self) -> u64 {
        self.hash
    }

    fn initialize(&mut self, element: &ElementPtr, context: &dyn ShaderImplContext) {
        let elem = element.borrow();
        if elem.get_category() != category::IMPLEMENTATION {
            log::error!(
                "SourceCodeNode: element '{}' is not an Implementation (got category '{}')",
                elem.get_name(),
                elem.get_category()
            );
            return;
        }
        drop(elem);

        self.name = element.borrow().get_name().to_string();

        self.function_source = element.borrow().get_attribute_or_empty("sourcecode");
        if self.function_source.is_empty() {
            if let Err(e) = self.resolve_source_code(element, context) {
                log::error!("SourceCodeNode: {}", e);
                return;
            }
        }

        self.function_name = element.borrow().get_attribute_or_empty("function");
        self.inlined = self.function_name.is_empty();

        if self.inlined {
            self.function_source = replace_substrings(&self.function_source, &[("\n", "")]);
        } else {
            let mut valid_function_name = self.function_name.clone();
            context.make_valid_name(&mut valid_function_name);
            if self.function_name != valid_function_name {
                log::error!(
                    "SourceCodeNode: Function name '{}' used by implementation '{}' is not a valid identifier",
                    self.function_name,
                    element.borrow().get_name()
                );
                return;
            }
        }

        self.hash = hash_string(&self.function_name);
    }

    fn emit_function_definition(
        &self,
        _node: &ShaderNode,
        context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
    ) {
        if stage.get_name() != super::shader::stage::PIXEL {
            return;
        }
        if self.inlined || self.function_source.is_empty() || self.source_filename.is_empty() {
            return;
        }
        // C++ emitBlock -> stage.addBlock(source, filename, context) which resolves #include
        // directives recursively. Our add_block_with_includes matches this behavior.
        if !stage.has_source_dependency(&self.source_filename) {
            stage.add_block_with_includes(&self.function_source, &self.source_filename, context);
            stage.append_line("");
            stage.add_source_dependency(self.source_filename.clone());
        }
    }

    fn emit_function_call(
        &self,
        node: &ShaderNode,
        context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
    ) {
        if stage.get_name() != super::shader::stage::PIXEL {
            return;
        }
        if self.inlined {
            self.emit_inlined_function_call(node, context, stage);
            return;
        }

        // Ordinary function call: outputVars; functionName(in1, in2, out1);
        self.emit_output_variables(node, context, stage);

        let mut args: Vec<String> = Vec::new();
        let closure_arg = context.get_closure_data_argument(node);
        if let Some(closure_data_arg) = closure_arg {
            args.push(closure_data_arg);
        }
        for input in node.get_inputs() {
            args.push(self.resolve_input_arg(node, input, context));
        }
        for output in node.get_outputs() {
            args.push(output.port.get_variable().to_string());
        }
        let args = args.join(", ");

        stage.append_line(&format!("{}({});", self.function_name, args));
    }

    fn emit_output_variables(
        &self,
        node: &ShaderNode,
        context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
    ) {
        for output in node.get_outputs() {
            let type_desc = output.get_type();
            let mtlx_name = type_desc.get_name();
            let (type_name, _) = context
                .get_type_name_for_emit(mtlx_name)
                .unwrap_or_else(|| (glsl_type_name(mtlx_name), glsl_default_value(mtlx_name)));
            let default_val = context.get_default_value(mtlx_name, false);
            let var = output.port.get_variable();
            stage.append_line(&format!("{} {} = {};", type_name, var, default_val));
        }
    }
}

impl SourceCodeNode {
    /// Emit inline sourcecode with {{var}} substitution (по рефу SourceCodeNode.cpp emitFunctionCall).
    fn emit_inlined_function_call(
        &self,
        node: &ShaderNode,
        context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
    ) {
        let subs: Vec<(String, String)> = context.get_substitution_tokens();
        let graph = match context.get_graph() {
            Some(g) => g,
            None => return,
        };
        let node_name = node.get_name().to_string();
        let mut emitted_tmps = std::collections::HashSet::<String>::new();

        let mut result = String::new();
        let mut pos = 0;
        let source = &self.function_source;
        while let Some(i) = source[pos..].find("{{") {
            let i = pos + i;
            result.push_str(&source[pos..i]);
            let j = source[i..].find("}}");
            let j = match j {
                Some(off) => i + off + 2,
                None => panic!(
                    "SourceCodeNode: Malformed inline expression in implementation for node '{}'",
                    node.get_name()
                ),
            };
            let var_name = source[i + 2..j - 2].trim();
            let replacement = match node.get_input(var_name) {
                Some(input) => {
                    if let Some((up_node, up_output)) = input.get_connection() {
                        graph
                            .get_connection_variable(up_node, up_output)
                            .unwrap_or_else(|| "0.0".to_string())
                    } else {
                        let tmp_name = format!("{}_{}_tmp", node_name, var_name);
                        if !emitted_tmps.contains(&tmp_name) {
                            let type_name = input.port.get_type().get_name();
                            let (tn, default) = context
                                .get_type_name_for_emit(type_name)
                                .unwrap_or_else(|| {
                                    (glsl_type_name(type_name), glsl_default_value(type_name))
                                });
                            let val = input.port.get_value_string();
                            let val = if val.is_empty() {
                                context.get_default_value(type_name, false)
                            } else {
                                // C++ AggregateTypeSyntax::getValue wraps raw components
                                wrap_aggregate_value(type_name, &val)
                            };
                            let qualifier = context.get_constant_qualifier();
                            let qualifier = if qualifier.is_empty() {
                                String::new()
                            } else {
                                format!("{} ", qualifier)
                            };
                            let _ = default;
                            stage.append_line(&format!(
                                "{}{} {} = {};",
                                qualifier, tn, tmp_name, val
                            ));
                            emitted_tmps.insert(tmp_name.clone());
                        }
                        tmp_name
                    }
                }
                None => panic!(
                    "SourceCodeNode: Could not find an input named '{}' on node '{}'",
                    var_name,
                    node.get_name()
                ),
            };
            result.push_str(&replacement);
            pos = j;
        }
        result.push_str(&source[pos..]);

        // C++ emitOutput(node.getOutput(), true, false, ...) — includeDeclaration=true
        // emits type + variable name as declaration.
        let (output_type, output_var) = match node.get_output_at(0) {
            Some(o) => {
                let mtlx = o.get_type().get_name();
                let tn = context
                    .get_type_name_for_emit(mtlx)
                    .map(|(t, _)| t)
                    .unwrap_or_else(|| glsl_type_name(mtlx));
                (tn, o.port.get_variable().to_string())
            }
            None => ("float", "out".to_string()),
        };

        for (from, to) in &subs {
            result = result.replace(from, to);
        }
        stage.append_line(&format!("{} {} = {};", output_type, output_var, result));
    }

    fn resolve_input_arg(
        &self,
        _node: &ShaderNode,
        input: &super::shader_node::ShaderInput,
        context: &dyn ShaderImplContext,
    ) -> String {
        if let Some((up_node, up_output)) = input.get_connection() {
            if let Some(g) = context.get_graph() {
                if let Some(var) = g.get_connection_variable(up_node, up_output) {
                    let type_name = input.port.get_type().get_name();
                    if type_name == "filename" {
                        return context.format_filename_arg(&var);
                    }
                    return var;
                }
            }
        }
        let type_name = input.port.get_type().get_name();
        let val = input.port().get_value_string();
        if !val.is_empty() {
            // C++ AggregateTypeSyntax::getValue wraps raw components in type ctor
            return wrap_aggregate_value(type_name, &val);
        }
        context.get_default_value(type_name, false)
    }
}

/// Wrap raw comma-separated components in a GLSL type constructor for aggregate types.
/// Matches C++ AggregateTypeSyntax::getValue: `"1, 1"` → `"vec2(1, 1)"`.
/// Scalar types pass through unchanged.
fn wrap_aggregate_value(mtlx_type: &str, raw_value: &str) -> String {
    let ctor = match mtlx_type {
        "vector2" => "vec2",
        "vector3" => "vec3",
        "vector4" => "vec4",
        "color3" => "vec3",
        "color4" => "vec4",
        "matrix33" => "mat3",
        "matrix44" => "mat4",
        _ => return raw_value.to_string(),
    };
    format!("{}({})", ctor, raw_value)
}

/// Map MaterialX type name to GLSL (simplified for SourceCodeNode).
pub fn glsl_type_name(mtlx: &str) -> &'static str {
    match mtlx {
        "float" => "float",
        "integer" => "int",
        "boolean" => "bool",
        "vector2" => "vec2",
        "vector3" => "vec3",
        "vector4" => "vec4",
        "color3" => "vec3",
        "color4" => "vec4",
        "matrix33" => "mat3",
        "matrix44" => "mat4",
        "BSDF" => "BSDF",
        "EDF" => "EDF",
        "VDF" => "BSDF",
        "surfaceshader" | "material" => "surfaceshader",
        "volumeshader" => "volumeshader",
        "displacementshader" => "displacementshader",
        "lightshader" => "lightshader",
        _ => "float",
    }
}

/// Default GLSL value per MaterialX type (for unconnected inputs).
fn glsl_default_value(mtlx: &str) -> &'static str {
    match mtlx {
        "float" => "0.0",
        "integer" => "0",
        "boolean" => "false",
        "vector2" => "vec2(0.0)",
        "vector3" | "color3" => "vec3(0.0)",
        "vector4" | "color4" => "vec4(0.0)",
        "BSDF" | "VDF" => "BSDF(vec3(0.0),vec3(1.0))",
        "EDF" => "EDF(0.0)",
        "surfaceshader" | "material" => "surfaceshader(vec3(0.0),vec3(0.0))",
        "volumeshader" => "volumeshader(vec3(0.0),vec3(0.0))",
        "displacementshader" => "displacementshader(vec3(0.0),1.0)",
        "lightshader" => "lightshader(vec3(0.0),vec3(0.0))",
        _ => "0.0",
    }
}

/// Get parent directory of the document containing this element (for resolving relative paths).
fn get_local_path_for_element(element: &ElementPtr) -> Option<FilePath> {
    let mut current = element.clone();
    loop {
        let parent = current.borrow().get_parent();
        match parent {
            Some(p) => current = p,
            None => break,
        }
    }
    let uri = current.borrow().get_source_uri()?.to_string();
    if uri.is_empty() {
        return None;
    }
    let path = Path::new(&uri);
    path.parent().map(FilePath::new)
}
