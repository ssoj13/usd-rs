//! ClosureCompoundNodeMdl -- MDL closure compound node (ref: MaterialXGenMdl/Nodes/ClosureCompoundNodeMdl.cpp).
//! Extends CompoundNodeMdl for closure/shader compound nodes.
//! Key differences from regular CompoundNodeMdl:
//! - Adds graph classification to the node
//! - Emits dependent closure calls before its own call
//! - Uses `= let { ... } in material(...)` for material expressions
//! - Supports unrolled struct members for shader-semantic outputs
//! - Emits texture nodes before closure nodes in function body

use super::mdl_syntax::PORT_NAME_PREFIX;
use crate::core::ElementPtr;
use crate::gen_shader::{
    Semantic, ShaderImplContext, ShaderNode, ShaderNodeClassification, ShaderNodeImpl, ShaderStage,
    hash_string,
};

/// MDL closure compound node -- handles closure/shader compound graph implementations.
#[derive(Debug, Default)]
pub struct ClosureCompoundNodeMdl {
    name: String,
    hash: u64,
    function_name: String,
    return_struct: String,
    unroll_return_struct_members: bool,
}

impl ClosureCompoundNodeMdl {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create() -> Box<dyn ShaderNodeImpl> {
        Box::new(Self::new())
    }
}

impl ShaderNodeImpl for ClosureCompoundNodeMdl {
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
        self.return_struct = String::new();
        self.unroll_return_struct_members = false;

        // Detect multi-output and shader-semantic outputs (same as CompoundNodeMdl)
        let el = element.borrow();
        let output_count = el
            .get_children()
            .iter()
            .filter(|c| c.borrow().get_category() == crate::core::element::category::OUTPUT)
            .count();
        if output_count > 1 {
            self.return_struct = format!("{}__result", self.function_name);
        }
        for child in el.get_children() {
            let child_ref = child.borrow();
            if child_ref.get_category() == crate::core::element::category::OUTPUT {
                if let Some(ty) = child_ref.get_type() {
                    let type_desc = context.get_type_system().get_type(ty);
                    if type_desc.get_semantic() == Semantic::Shader {
                        self.unroll_return_struct_members = true;
                    }
                }
            }
        }
    }

    fn add_classification(&self, node: &mut ShaderNode) {
        // Add classification from the graph implementation
        node.add_classification(ShaderNodeClassification::CLOSURE);
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

        // Handle unrolled struct members -- separate function per output field
        if !self.return_struct.is_empty() && self.unroll_return_struct_members {
            for out in node.get_outputs() {
                let field_name = out.get_name();
                let out_type = out.get_type().get_name();
                let is_material_expr = out.get_type().is_closure()
                    || out.get_type().get_semantic() == Semantic::Shader;

                let (emit_type, _) = context
                    .get_type_name_for_emit(out_type)
                    .unwrap_or(("material", "material()"));

                // Emit comment and function signature per field
                stage.append_line(&format!(
                    "// unrolled structure field: {}.{} (name=\"{}\")",
                    self.return_struct,
                    field_name,
                    node.get_name()
                ));
                stage.append_line(&format!(
                    "{} {}__{}",
                    emit_type, self.function_name, field_name
                ));

                // Emit parameter list
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
                    let value = context.get_default_value(inp_type, true);
                    let var_name = inp.port.get_variable();
                    let delim = if i + 1 < count { "," } else { "" };
                    stage.append_line(&format!(
                        "    {}{} {} = {}{}",
                        qualifier, mdl_type, var_name, value, delim
                    ));
                }
                stage.append_line(")");

                if is_material_expr {
                    stage.append_line(" = let");
                }
                stage.append_line("{");

                // Emit final results -- use the output variable name
                let result = out.port.get_variable().to_string();
                if is_material_expr {
                    stage.append_line("}");
                    stage.append_line(&format!("in material({})", result));
                } else {
                    stage.append_line(&format!("    return {};", result));
                    stage.append_line("}");
                }
                stage.append_line("");
            }
            return;
        }

        let is_material_expr = node.has_classification(ShaderNodeClassification::CLOSURE)
            || node.has_classification(ShaderNodeClassification::SHADER);

        // Emit function signature
        if !self.return_struct.is_empty() {
            // Define struct + function
            stage.append_line(&format!("struct {}", self.return_struct));
            stage.append_line("{");
            for out in node.get_outputs() {
                let out_name = out.get_name();
                let out_type = out.get_type().get_name();
                let (emit_type, _) = context
                    .get_type_name_for_emit(out_type)
                    .unwrap_or(("float", "0.0"));
                let port_name = format!("{}{}", PORT_NAME_PREFIX, out_name);
                stage.append_line(&format!("    {} {};", emit_type, port_name));
            }
            stage.append_line("};");
            stage.append_line("");
            stage.append_line(&format!("{} {}", self.return_struct, self.function_name));
        } else {
            let out_type = node
                .get_outputs()
                .next()
                .map(|o| o.get_type().get_name())
                .unwrap_or("material");
            let (emit_type, _) = context
                .get_type_name_for_emit(out_type)
                .unwrap_or(("material", "material()"));
            stage.append_line(&format!("{} {}", emit_type, self.function_name));
        }

        // Emit parameter list
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
            let value = context.get_default_value(inp_type, true);
            let var_name = inp.port.get_variable();
            let delim = if i + 1 < count { "," } else { "" };
            stage.append_line(&format!(
                "    {}{} {} = {}{}",
                qualifier, mdl_type, var_name, value, delim
            ));
        }
        stage.append_line(")");

        if is_material_expr {
            stage.append_line(" = let");
        }
        stage.append_line("{");

        if is_material_expr {
            stage.append_line("}");
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

        // Delegate to CompoundNodeMdl-style function call emission
        let args = super::compound_node_mdl::CompoundNodeMdl::emit_inputs(node, context);

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
            // Single output call
            let out = node.get_outputs().next();
            let out_var = out
                .map(|o| o.port.get_variable().to_string())
                .unwrap_or_else(|| "out".to_string());
            let out_type = out.map(|o| o.get_type().get_name()).unwrap_or("material");
            let (emit_type, _) = context
                .get_type_name_for_emit(out_type)
                .unwrap_or(("material", "material()"));
            stage.append_line(&format!(
                "{} {} = {}({});",
                emit_type, out_var, self.function_name, args
            ));
        }
    }
}
