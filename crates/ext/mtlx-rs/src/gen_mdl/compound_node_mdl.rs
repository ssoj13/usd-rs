//! CompoundNodeMdl -- MDL compound node wrapping sub-graphs with multi-output struct support.
//! Ref: MaterialXGenMdl/Nodes/CompoundNodeMdl.cpp
//!
//! For MDL, compound nodes (nodegraphs used as implementations) need special handling:
//! - single output: return the value directly
//! - multiple outputs: wrap in a return struct `<funcname>__result`
//! - material expressions: use `let ... in material(...)` syntax
//! - Full function signatures with parameters, defaults, geomprops, annotations

use super::mdl_shader_generator::geomprop_default;
use super::mdl_syntax::PORT_NAME_PREFIX;
use crate::core::ElementPtr;
use crate::gen_shader::{
    ShaderImplContext, ShaderNode, ShaderNodeClassification, ShaderNodeImpl, ShaderStage,
    hash_string,
};

/// User data key for passing return struct field name through context.
/// Ref: CompoundNodeMdl::GEN_USER_DATA_RETURN_STRUCT_FIELD_NAME
#[allow(dead_code)]
pub const GEN_USER_DATA_RETURN_STRUCT_FIELD_NAME: &str = "returnStructFieldName";

/// MDL compound node -- handles nodegraph-based compound node implementations for MDL.
/// When a compound has multiple outputs, a return struct `<funcname>__result` is generated.
#[derive(Debug, Default)]
pub struct CompoundNodeMdl {
    name: String,
    hash: u64,
    /// Function name derived from the implementing nodegraph
    pub(crate) function_name: String,
    /// Return struct name; empty if single output
    pub(crate) return_struct: String,
    /// True if struct members should be unrolled into separate functions (shader-semantic outputs)
    pub(crate) unroll_return_struct_members: bool,
}

impl CompoundNodeMdl {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create() -> Box<dyn ShaderNodeImpl> {
        Box::new(Self::new())
    }

    /// Check if return struct members should be unrolled
    pub fn unroll_return_struct_members(&self) -> bool {
        self.unroll_return_struct_members
    }

    /// Check if this is a return struct compound
    pub fn is_return_struct(&self) -> bool {
        !self.return_struct.is_empty()
    }

    /// Emit inputs as function call arguments (resolves connections or defaults)
    pub(crate) fn emit_inputs(node: &ShaderNode, context: &dyn ShaderImplContext) -> String {
        let mut args = String::new();
        let mut first = true;
        for inp in node.get_inputs() {
            if !first {
                args.push_str(", ");
            }
            first = false;
            if let Some((up_node, up_out)) = inp.get_connection() {
                if let Some(g) = context.get_graph() {
                    if let Some(var) = g.get_connection_variable(up_node, up_out) {
                        args.push_str(&var);
                        continue;
                    }
                }
            }
            let type_name = inp.get_type().get_name();
            args.push_str(&context.get_default_value(type_name, true));
        }
        args
    }

    /// Emit function signature with full parameter list.
    /// Ref: CompoundNodeMdl::emitFunctionSignature
    fn emit_function_signature(
        &self,
        node: &ShaderNode,
        context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
    ) {
        if !self.return_struct.is_empty() {
            if self.unroll_return_struct_members {
                // When unrolling, each field gets its own function -- handled by caller
                return;
            }

            // Define the output struct (ref: CompoundNodeMdl::emitFunctionSignature struct block)
            stage.append_line(&format!("struct {}", self.return_struct));
            stage.append_line("{");
            for out in node.get_outputs() {
                let out_type = out.get_type().get_name();
                let (emit_type, _) = context
                    .get_type_name_for_emit(out_type)
                    .unwrap_or(("float", "0.0"));
                let port_name = format!("{}{}", PORT_NAME_PREFIX, out.get_name());
                stage.append_line(&format!("    {} {};", emit_type, port_name));
            }
            stage.append_line("};");
            stage.append_line("");

            // Begin function signature with struct return
            stage.append_line(&format!("{} {}", self.return_struct, self.function_name));
        } else {
            // Single output function signature
            let out_type = node
                .get_outputs()
                .next()
                .map(|o| o.get_type().get_name())
                .unwrap_or("float");
            let (emit_type, _) = context
                .get_type_name_for_emit(out_type)
                .unwrap_or(("float", "0.0"));
            stage.append_line(&format!("{} {}", emit_type, self.function_name));
        }

        // Emit parameter list (ref: CompoundNodeMdl::emitFunctionSignature params)
        self.emit_parameter_list(node, context, stage);
    }

    /// Emit parameter list with types, defaults, geomprops, and annotations.
    fn emit_parameter_list(
        &self,
        node: &ShaderNode,
        context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
    ) {
        stage.append_line("(");

        let inputs: Vec<_> = node.get_inputs().collect();
        let count = inputs.len();
        for (i, inp) in inputs.iter().enumerate() {
            let inp_type = inp.get_type().get_name();
            let (mdl_type, _) = context
                .get_type_name_for_emit(inp_type)
                .unwrap_or(("float", "0.0"));
            let qualifier = if inp.port.is_uniform() || inp_type == "filename" {
                "uniform "
            } else {
                ""
            };

            // Get value: geomprop > explicit value > default
            let value = if !inp.port.geomprop.is_empty() {
                geomprop_default(&inp.port.geomprop)
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| context.get_default_value(inp_type, true))
            } else if let Some(v) = inp.port.get_value() {
                v.get_value_string()
            } else {
                context.get_default_value(inp_type, true)
            };

            let var_name = inp.port.get_variable();
            let delim = if i + 1 < count { "," } else { "" };

            // Emit annotations for unconnected inputs (ref: emitFunctionSignature annotations)
            if inp.get_connection().is_none() {
                stage.append_line(&format!(
                    "    {}{} {} = {}\n    [[\n        anno::unused()\n    ]]{}",
                    qualifier, mdl_type, var_name, value, delim
                ));
            } else {
                stage.append_line(&format!(
                    "    {}{} {} = {}{}",
                    qualifier, mdl_type, var_name, value, delim
                ));
            }
        }

        stage.append_line(")");
    }
}

impl ShaderNodeImpl for CompoundNodeMdl {
    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_hash(&self) -> u64 {
        self.hash
    }

    fn initialize(&mut self, element: &ElementPtr, context: &dyn ShaderImplContext) {
        self.name = element.borrow().get_name().to_string();
        self.hash = hash_string(&self.name);
        self.function_name = self.name.clone();

        // Detect multi-output: count outputs
        self.return_struct = String::new();
        self.unroll_return_struct_members = false;

        let el = element.borrow();
        let output_count = el
            .get_children()
            .iter()
            .filter(|c| c.borrow().get_category() == crate::core::element::category::OUTPUT)
            .count();
        if output_count > 1 {
            self.return_struct = format!("{}__result", self.function_name);
        }

        // Check if any output is shader-semantic (material) -- can't be struct members
        for child in el.get_children() {
            let child_ref = child.borrow();
            if child_ref.get_category() == crate::core::element::category::OUTPUT {
                if let Some(ty) = child_ref.get_type() {
                    let type_desc = context.get_type_system().get_type(ty);
                    if type_desc.get_semantic() == crate::gen_shader::Semantic::Shader {
                        self.unroll_return_struct_members = true;
                    }
                }
            }
        }
    }

    fn emit_function_definition(
        &self,
        node: &ShaderNode,
        context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
    ) {
        if stage.get_name() != crate::gen_shader::shader_stage::PIXEL {
            return;
        }

        let is_material_expr = node.has_classification(ShaderNodeClassification::CLOSURE)
            || node.has_classification(ShaderNodeClassification::SHADER);

        // Emit function signature (ref: CompoundNodeMdl::emitFunctionDefinition)
        self.emit_function_signature(node, context, stage);

        // Special case for material expressions
        if is_material_expr {
            stage.append_line(" = let");
        }

        // Function body
        stage.append_line("{");

        // Child node function calls are emitted by the generator traversal
        // (In C++ this calls shadergen.emitFunctionCalls(*_rootGraph, ...))

        // Emit final results
        if is_material_expr {
            stage.append_line("}");
            // in material(result) -- use the output variable name
            let result = node
                .get_outputs()
                .next()
                .map(|o| o.port.get_variable().to_string())
                .unwrap_or_else(|| "result".to_string());
            stage.append_line(&format!("in material({})", result));
        } else {
            if !self.return_struct.is_empty() {
                let result_var = "result__";
                stage.append_line(&format!("    {} {}", self.return_struct, result_var));
                for out in node.get_outputs() {
                    let out_name = out.get_name();
                    let port_name = format!("{}{}", PORT_NAME_PREFIX, out_name);
                    // Use the output variable name as result
                    let result = out.port.get_variable().to_string();
                    stage.append_line(&format!("    {}.{} = {};", result_var, port_name, result));
                }
                stage.append_line(&format!("    return {};", result_var));
            } else {
                let result = node
                    .get_outputs()
                    .next()
                    .map(|o| o.port.get_variable().to_string())
                    .unwrap_or_else(|| "result".to_string());
                stage.append_line(&format!("    return {};", result));
            }
            stage.append_line("}");
        }

        stage.append_line("");
    }

    fn emit_function_call(
        &self,
        node: &ShaderNode,
        context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
    ) {
        if stage.get_name() != crate::gen_shader::shader_stage::PIXEL {
            return;
        }

        let args = Self::emit_inputs(node, context);

        if !self.return_struct.is_empty() {
            if self.unroll_return_struct_members {
                // Unrolled: call separate function per output field
                stage.append_line(&format!(
                    "// fill unrolled structure fields: {} (name=\"{}\")",
                    self.return_struct,
                    node.get_name()
                ));
                for out in node.get_outputs() {
                    let field_name = out.get_name();
                    let out_type = out.get_type().get_name();
                    let (emit_type, _) = context
                        .get_type_name_for_emit(out_type)
                        .unwrap_or(("material", "material()"));
                    let result_var = format!("{}__{}", node.get_name(), field_name);
                    stage.append_line(&format!(
                        "{} {} = {}__{}({});",
                        emit_type, result_var, self.function_name, field_name, args
                    ));
                }
                return;
            }

            // Multi-output struct call
            let result_var = format!("{}_result", node.get_name());
            stage.append_line(&format!(
                "{} {} = {}({});",
                self.return_struct, result_var, self.function_name, args
            ));
        } else {
            // Single output call -- emit output type and variable
            let out = node.get_outputs().next();
            let out_var = out
                .map(|o| o.port.get_variable().to_string())
                .unwrap_or_else(|| "out".to_string());
            let out_type = out.map(|o| o.get_type().get_name()).unwrap_or("float");
            let (emit_type, _) = context
                .get_type_name_for_emit(out_type)
                .unwrap_or(("float", "0.0"));
            stage.append_line(&format!(
                "{} {} = {}({});",
                emit_type, out_var, self.function_name, args
            ));
        }
    }
}
