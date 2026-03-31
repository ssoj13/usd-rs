//! CustomNodeMdl -- MDL custom code node (ref: MaterialXGenMdl/Nodes/CustomNodeMdl.cpp).
//! Handles user-defined implementations in external MDL files or inline `sourcecode`.
//! Key features:
//! - Inline sourcecode: wraps user MDL code in a local function, emits struct returns for multi-output
//! - External file: maps `file` attribute to MDL qualified module path, `function` to call name
//! - Port name prefixing: avoids MDL reserved word collisions via `mxp_` prefix
//! - {{MDL_VERSION_SUFFIX}} marker replacement in module paths
//! - initializeFunctionCallTemplateString: builds funcname(inputName: {{inputName}}, ...) template

use super::mdl_syntax::PORT_NAME_PREFIX;
use super::source_code_node_mdl::SourceCodeNodeMdl;
use crate::core::ElementPtr;
#[allow(unused_imports)]
use crate::core::element::category;
use crate::gen_shader::{ShaderImplContext, ShaderNode, ShaderNodeImpl, ShaderStage, hash_string};

/// MDL custom code node -- wraps external MDL modules or inline sourcecode.
#[derive(Debug, Default)]
pub struct CustomCodeNodeMdl {
    /// Base source code node fields
    base: SourceCodeNodeMdl,
    /// True if `file`+`function` are used (external); false if `sourcecode` (inline)
    use_external_source_code: bool,
    /// For inline: the function name to emit
    inline_function_name: String,
    /// For inline: the actual source code body
    inline_source_code: String,
    /// MDL qualified module name (e.g. `::mymodule::submod`)
    qualified_module_name: String,
    /// Output default values from nodedef (ref: _outputDefaults)
    output_defaults: Vec<Option<String>>,
}

impl CustomCodeNodeMdl {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create() -> Box<dyn ShaderNodeImpl> {
        Box::new(Self::new())
    }

    /// Get the MDL qualified module name for import statements.
    pub fn get_qualified_module_name(&self) -> &str {
        &self.qualified_module_name
    }

    pub fn is_custom_impl_element(element: &ElementPtr) -> bool {
        let elem = element.borrow();
        let file_attr = elem.get_attribute("file").unwrap_or_default();
        let func_attr = elem.get_attribute("function").unwrap_or_default();
        let source_code = elem.get_attribute("sourcecode").unwrap_or_default();
        (!file_attr.is_empty() && !func_attr.is_empty())
            || (!source_code.is_empty() && !source_code.contains("{{"))
    }

    pub fn qualified_module_name_from_file(
        file_attr: &str,
        context: &dyn ShaderImplContext,
        impl_name: &str,
    ) -> String {
        if file_attr.is_empty() {
            return String::new();
        }

        let mut mdl_name = file_attr.replace('/', "::");
        if !mdl_name.starts_with("::") {
            mdl_name = format!("::{}", mdl_name);
        }
        if mdl_name.ends_with(".mdl") {
            mdl_name.truncate(mdl_name.len() - 4);
        }

        let version_suffix = context.get_mdl_version_suffix();
        SourceCodeNodeMdl::replace_markers(&mdl_name, impl_name, &|marker| {
            if marker == "MDL_VERSION_SUFFIX" {
                version_suffix.to_string()
            } else {
                marker.to_string()
            }
        })
    }

    /// Modify port name to avoid MDL reserved word collisions.
    /// For inline: always prefix with mxp_. For external: only prefix reserved words.
    /// Ref: CustomCodeNodeMdl::modifyPortName
    pub fn modify_port_name_with_syntax(
        &self,
        name: &str,
        reserved_words: &std::collections::HashSet<String>,
    ) -> String {
        if self.use_external_source_code {
            // Only prefix if it collides with a reserved word
            if reserved_words.contains(name) {
                format!("{}{}", PORT_NAME_PREFIX, name)
            } else {
                name.to_string()
            }
        } else {
            // Inline: always prefix
            format!("{}{}", PORT_NAME_PREFIX, name)
        }
    }

    /// Build function call template string: funcname(inputName: {{inputName}}, ...)
    /// Ref: CustomCodeNodeMdl::initializeFunctionCallTemplateString
    fn build_function_call_template(
        &self,
        input_names: &[String],
        reserved_words: &std::collections::HashSet<String>,
    ) -> String {
        let prefix = if self.use_external_source_code {
            // Fully qualified: module::function(
            let module = self
                .qualified_module_name
                .strip_prefix("::")
                .unwrap_or(&self.qualified_module_name);
            format!("{}::{}(", module, self.base.function_name)
        } else {
            // Local: inlineFunctionName(
            format!("{}(", self.inline_function_name)
        };

        let mut result = prefix;
        let mut delim = "";
        for name in input_names {
            let port_name = self.modify_port_name_with_syntax(name, reserved_words);
            result.push_str(&format!("{}{}: {{{{{}}}}}", delim, port_name, name));
            if delim.is_empty() {
                delim = ", ";
            }
        }
        result.push(')');
        result
    }
}

impl ShaderNodeImpl for CustomCodeNodeMdl {
    fn get_name(&self) -> &str {
        self.base.get_name()
    }

    fn get_hash(&self) -> u64 {
        self.base.get_hash()
    }

    fn initialize(&mut self, element: &ElementPtr, context: &dyn ShaderImplContext) {
        self.base.initialize(element, context);

        let el = element.borrow();
        let source_code = el
            .get_attribute("sourcecode")
            .unwrap_or_default()
            .to_string();
        let file_attr = el.get_attribute("file").unwrap_or_default().to_string();

        // Collect input names from the element's nodedef reference or direct children
        let mut input_names: Vec<String> = Vec::new();
        let mut output_info: Vec<(String, String)> = Vec::new(); // (name, type)

        // Try to get nodedef to find input/output definitions
        let _nodedef_name = el.get_attribute("nodedef").unwrap_or_default().to_string();
        // Fallback: collect Input children from the element
        for child in el.get_children() {
            let child_ref = child.borrow();
            match child_ref.get_category() {
                "input" => {
                    input_names.push(child_ref.get_name().to_string());
                }
                "output" => {
                    let out_type = child_ref.get_type().unwrap_or("float").to_string();
                    let out_name = child_ref.get_name().to_string();
                    let default_val = child_ref.get_attribute("value").map(|s| s.to_string());
                    output_info.push((out_name, out_type));
                    self.output_defaults.push(default_val);
                }
                _ => {}
            }
        }

        let reserved_words = context.get_reserved_words().cloned().unwrap_or_default();

        drop(el);

        if !source_code.is_empty() {
            // Inline sourcecode mode (ref: initializeForInlineSourceCode)
            self.use_external_source_code = false;
            self.inline_source_code = source_code.clone();

            // Validate no // comments
            if self.inline_source_code.contains("//") {
                eprintln!(
                    "WARNING: Source code contains unsupported comments '//', please use '/* */' instead"
                );
            }

            // Use nodedef name as function name (ref: _inlineFunctionName = nodeDef->getName())
            let el = element.borrow();
            self.inline_function_name = el.get_name().to_string();
            self.base.hash = hash_string(&self.inline_function_name);
            drop(el);

            // Build function call template (ref: initializeFunctionCallTemplateString)
            self.base.function_source =
                self.build_function_call_template(&input_names, &reserved_words);
            self.base.inlined = true;
        } else {
            // External file + function mode (ref: initializeForExternalSourceCode)
            self.use_external_source_code = true;

            if !file_attr.is_empty() {
                self.qualified_module_name = Self::qualified_module_name_from_file(
                    &file_attr,
                    context,
                    self.base.get_name(),
                );
            }

            // Build function call template (ref: initializeFunctionCallTemplateString)
            self.base.function_source =
                self.build_function_call_template(&input_names, &reserved_words);
            self.base.inlined = true;
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

        // External functions: no definition needed (ref: CustomCodeNodeMdl::emitFunctionDefinition)
        if self.use_external_source_code {
            return;
        }

        // Inline: emit a local function wrapping the user sourcecode
        stage.append_line(&format!(
            "// generated code for implementation: '{}'",
            node.get_impl_name().unwrap_or(node.get_name())
        ));

        // Collect output info with proper MDL type names
        struct OutputField {
            name: String,
            type_name: String,
            default_value: String,
        }
        let mut outputs: Vec<OutputField> = Vec::new();
        let mut idx = 0;
        for output in node.get_outputs() {
            let out_type = output.get_type().get_name();
            let (mdl_type, _) = context
                .get_type_name_for_emit(out_type)
                .unwrap_or(("float", "0.0"));
            let port_name = format!("{}{}", PORT_NAME_PREFIX, output.get_name());

            // Get default value from stored defaults or syntax default
            let default_val = self
                .output_defaults
                .get(idx)
                .and_then(|v| v.as_ref())
                .map(|_v| {
                    // Use syntax-formatted value
                    let td = context.get_type_system().get_type(out_type);
                    context.get_default_value(td.get_name(), false)
                })
                .unwrap_or_else(|| context.get_default_value(out_type, false));

            outputs.push(OutputField {
                name: port_name,
                type_name: mdl_type.to_string(),
                default_value: default_val,
            });
            idx += 1;
        }

        let num_outputs = outputs.len();

        // Determine return type
        let return_type_name = if num_outputs == 1 {
            outputs[0].type_name.clone()
        } else {
            let rtn = format!("{}_return_type", self.inline_function_name);
            // Emit return struct definition
            stage.append_line(&format!("struct {}", rtn));
            stage.append_line("{");
            for field in &outputs {
                stage.append_line(&format!("    {} {};", field.type_name, field.name));
            }
            stage.append_line("}");
            stage.append_line("");
            rtn
        };

        // Function signature (ref: CustomCodeNodeMdl::emitFunctionDefinition)
        stage.append_line("");

        // Emit function with parameters
        stage.append_line(&format!(
            "{} {}",
            return_type_name, self.inline_function_name
        ));
        stage.append_line("(");
        let param_count = node.get_inputs().count();
        let mut i = 0;
        for inp in node.get_inputs() {
            let inp_type = inp.get_type().get_name();
            let (mdl_type, _) = context
                .get_type_name_for_emit(inp_type)
                .unwrap_or(("float", "0.0"));
            let qualifier = if inp.port.is_uniform() || inp_type == "filename" {
                "uniform "
            } else {
                ""
            };
            let port_name = format!("{}{}", PORT_NAME_PREFIX, inp.get_name());
            let delim = if i + 1 < param_count { "," } else { "" };
            stage.append_line(&format!(
                "    {}{} {}{}",
                qualifier, mdl_type, port_name, delim
            ));
            i += 1;
        }
        stage.append_line(")");

        // Function body
        stage.append_line("{");

        // Initialize output variables
        stage.append_line("    // initialize outputs:");
        for field in &outputs {
            stage.append_line(&format!(
                "    {} {} = {};",
                field.type_name, field.name, field.default_value
            ));
        }

        // Emit inline source code
        stage.append_line("    // inlined shader source code:");
        stage.append_line(&format!("    {}", self.inline_source_code));

        // Pack and return
        stage.append_line("    // pack (in case of multiple outputs) and return outputs:");
        if num_outputs == 1 {
            stage.append_line(&format!("    return {};", outputs[0].name));
        } else {
            let mut ret = format!("    return {}(", return_type_name);
            for (i, field) in outputs.iter().enumerate() {
                if i > 0 {
                    ret.push_str(", ");
                }
                ret.push_str(&field.name);
            }
            ret.push(')');
            stage.append_line(&format!("{};", ret));
        }

        stage.append_line("}");
        stage.append_line("");
    }

    fn emit_function_call(
        &self,
        node: &ShaderNode,
        context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
    ) {
        // Delegate to base SourceCodeNodeMdl emit (which uses the function_source template)
        self.base.emit_function_call(node, context, stage);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::document::create_document;
    use crate::core::element::{add_child_of_category, category};
    use crate::format::FilePath;
    use crate::gen_shader::TypeSystem;
    use std::collections::HashSet;

    struct TestContext {
        type_system: TypeSystem,
        reserved_words: HashSet<String>,
        mdl_version_suffix: String,
    }

    impl ShaderImplContext for TestContext {
        fn resolve_source_file(
            &self,
            _filename: &str,
            _local_path: Option<&FilePath>,
        ) -> Option<FilePath> {
            None
        }

        fn get_type_system(&self) -> &TypeSystem {
            &self.type_system
        }

        fn get_reserved_words(&self) -> Option<&HashSet<String>> {
            Some(&self.reserved_words)
        }

        fn get_mdl_version_suffix(&self) -> &str {
            &self.mdl_version_suffix
        }
    }

    #[test]
    fn external_custom_node_rewrites_reserved_ports() {
        let doc = create_document();
        let root = doc.get_root();
        let nodedef =
            add_child_of_category(&root, category::NODEDEF, "ND_custom_float").expect("nodedef");
        nodedef.borrow_mut().set_attribute("node", "custom");
        let nd_input = add_child_of_category(&nodedef, category::INPUT, "float").expect("nd input");
        nd_input.borrow_mut().set_attribute("type", "float");
        let nd_output =
            add_child_of_category(&nodedef, category::OUTPUT, "out").expect("nd output");
        nd_output.borrow_mut().set_attribute("type", "float");
        let impl_elem =
            add_child_of_category(&root, category::IMPLEMENTATION, "IM_custom").expect("impl");
        impl_elem
            .borrow_mut()
            .set_attribute("nodedef", "ND_custom_float");
        impl_elem
            .borrow_mut()
            .set_attribute("file", "mdl/custom_module.mdl");
        impl_elem
            .borrow_mut()
            .set_attribute("function", "do_custom");
        let input = add_child_of_category(&impl_elem, category::INPUT, "float").expect("input");
        input.borrow_mut().set_attribute("type", "float");

        let ctx = TestContext {
            type_system: TypeSystem::new(),
            reserved_words: HashSet::from(["float".to_string()]),
            mdl_version_suffix: "1_10".to_string(),
        };

        let mut node = CustomCodeNodeMdl::new();
        node.initialize(&impl_elem, &ctx);

        assert_eq!(node.get_qualified_module_name(), "::mdl::custom_module");
        assert!(node.base.function_source.contains("mxp_float: {{float}}"));
    }

    #[test]
    fn qualified_module_name_applies_version_markers() {
        let ctx = TestContext {
            type_system: TypeSystem::new(),
            reserved_words: HashSet::new(),
            mdl_version_suffix: "1_11".to_string(),
        };

        let module_name = CustomCodeNodeMdl::qualified_module_name_from_file(
            "mdl/custom_{{MDL_VERSION_SUFFIX}}.mdl",
            &ctx,
            "IM_custom",
        );

        assert_eq!(module_name, "::mdl::custom_1_11");
    }
}
