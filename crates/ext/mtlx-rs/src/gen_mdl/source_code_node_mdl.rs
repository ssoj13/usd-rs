//! SourceCodeNodeMdl -- MDL source code node (ref: MaterialXGenMdl/Nodes/SourceCodeNodeMdl.cpp).
//! Extends SourceCodeNode with MDL-specific inline source code handling:
//! - resolveSourceCode is no-op (MDL compiler handles module resolution)
//! - Function calls support {{marker}} replacement for version suffixes and inputs
//! - Multi-output nodes use `<funcname>__result` return struct with `auto` unpacking

use crate::core::ElementPtr;
use crate::core::element::category;
use crate::gen_shader::{ShaderImplContext, ShaderNode, ShaderNodeImpl, ShaderStage, hash_string};

/// MDL source code node -- handles data-driven source code implementations.
/// Default impl for all MDL nodes that don't have a custom ShaderNodeImpl.
#[derive(Debug, Default)]
pub struct SourceCodeNodeMdl {
    name: String,
    pub(crate) hash: u64,
    /// The function name from the implementation's `function` attribute.
    pub(crate) function_name: String,
    /// The inline source code template (with {{param}} markers).
    pub(crate) function_source: String,
    /// True if the function is inlined (uses source code markers).
    pub(crate) inlined: bool,
    /// Return struct name for multi-output nodes (empty if single output).
    pub(crate) return_struct: String,
}

impl SourceCodeNodeMdl {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create() -> Box<dyn ShaderNodeImpl> {
        Box::new(Self::new())
    }

    /// Replace `{{marker}}` tokens in source code using the provided resolver.
    /// Ref: MdlSyntax::replaceSourceCodeMarkers
    pub fn replace_markers(
        source: &str,
        node_name: &str,
        resolver: &dyn Fn(&str) -> String,
    ) -> String {
        let mut result = String::with_capacity(source.len());
        let mut pos = 0;
        while let Some(start) = source[pos..].find("{{") {
            let abs_start = pos + start;
            result.push_str(&source[pos..abs_start]);
            let after = abs_start + 2;
            if let Some(end) = source[after..].find("}}") {
                let marker = &source[after..after + end];
                result.push_str(&resolver(marker));
                pos = after + end + 2;
            } else {
                panic!(
                    "SourceCodeNodeMdl: Malformed inline expression in implementation for node {}",
                    node_name
                );
            }
        }
        result.push_str(&source[pos..]);
        result
    }
}

impl ShaderNodeImpl for SourceCodeNodeMdl {
    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_hash(&self) -> u64 {
        self.hash
    }

    fn initialize(&mut self, element: &ElementPtr, context: &dyn ShaderImplContext) {
        let el = element.borrow();
        self.name = el.get_name().to_string();

        // Extract function name and source from the element attributes
        self.function_name = el.get_attribute("function").unwrap_or_default().to_string();
        self.function_source = el
            .get_attribute("sourcecode")
            .unwrap_or_default()
            .to_string();
        self.inlined = self.function_name.is_empty();

        if self.inlined {
            self.function_source = self.function_source.replace('\n', "");
        } else {
            let mut valid_function_name = self.function_name.clone();
            context.make_valid_name(&mut valid_function_name);
            if self.function_name != valid_function_name {
                panic!(
                    "SourceCodeNodeMdl: Function name '{}' used by implementation '{}' is not a valid identifier",
                    self.function_name,
                    el.get_name()
                );
            }
        }

        // Derive return struct name from nodedef output count (ref: SourceCodeNodeMdl::initialize)
        self.return_struct = String::new();

        let nodedef_name = el.get_attribute("nodedef").unwrap_or_default().to_string();
        drop(el);

        if element.borrow().get_category() == category::IMPLEMENTATION {
            let output_count = if let Some(doc) = crate::core::Document::from_element(element) {
                let nodedef = doc.get_node_def(&nodedef_name).unwrap_or_else(|| {
                    panic!(
                        "SourceCodeNodeMdl: Can't find nodedef '{}' for implementation element {}",
                        nodedef_name,
                        element.borrow().get_name()
                    )
                });
                nodedef
                    .borrow()
                    .get_children()
                    .iter()
                    .filter(|c| c.borrow().get_category() == category::OUTPUT)
                    .count()
            } else {
                panic!(
                    "SourceCodeNodeMdl: Can't resolve document for implementation element {}",
                    element.borrow().get_name()
                );
            };
            if output_count > 1 {
                if self.function_name.is_empty() {
                    let fn_name = if let Some(pos) = self.function_source.find('(') {
                        let raw = &self.function_source[..pos];
                        let version_suffix = context.get_mdl_version_suffix();
                        raw.replace("{{MDL_VERSION_SUFFIX}}", version_suffix)
                    } else {
                        self.function_source.clone()
                    };
                    self.return_struct = format!("{}__result", fn_name);
                } else {
                    self.return_struct = format!("{}__result", self.function_name);
                }
            }
        }
        self.hash = hash_string(&self.function_name);
    }

    fn emit_function_definition(
        &self,
        _node: &ShaderNode,
        _context: &dyn ShaderImplContext,
        _stage: &mut ShaderStage,
    ) {
        // No-op for MDL: module resolution handled by MDL compiler.
        // Ref: SourceCodeNodeMdl::emitFunctionDefinition is empty.
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

        // Emit dependent closure calls upstream (ref: SourceCodeNodeMdl.cpp)
        // In C++ this calls shadergen.emitDependentFunctionCalls(node, context, stage, CLOSURE).
        // Closure dependencies are resolved during graph traversal in our Rust port.

        if self.inlined {
            // Replace {{marker}} in source with input values
            let code = Self::replace_markers(&self.function_source, &self.name, &|marker| {
                // MDL version suffix marker
                if marker == "MDL_VERSION_SUFFIX" {
                    return context.get_mdl_version_suffix().to_string();
                }
                // Look up input by name
                if let Some(inp) = node.get_input(marker) {
                    if let Some((up_node, up_out)) = inp.get_connection() {
                        if let Some(g) = context.get_graph() {
                            if let Some(var) = g.get_connection_variable(up_node, up_out) {
                                return var;
                            }
                        }
                    }
                    let type_name = inp.get_type().get_name();
                    return context.get_default_value(type_name, true);
                }
                panic!(
                    "SourceCodeNodeMdl: Could not find an input named '{}' on node '{}'",
                    marker,
                    node.get_name()
                )
            });

            if !self.return_struct.is_empty() {
                // Multi-output: auto result = <code>
                let result_var = format!("{}_result", node.get_name());
                stage.append_line(&format!("auto {} = {};", result_var, code));
            } else {
                // Single output
                let out_var = node
                    .get_outputs()
                    .next()
                    .map(|o| o.port.get_variable().to_string())
                    .unwrap_or_else(|| "out".to_string());
                let out_type = node
                    .get_outputs()
                    .next()
                    .map(|o| o.get_type().get_name())
                    .unwrap_or("float");
                let (emit_type, _) = context
                    .get_type_name_for_emit(out_type)
                    .unwrap_or(("float", "0.0"));
                stage.append_line(&format!("{} {} = {};", emit_type, out_var, code));
            }
        } else {
            // Ordinary function call
            let mut call = format!("{}(", self.function_name);
            let mut first = true;
            for inp in node.get_inputs() {
                if !first {
                    call.push_str(", ");
                }
                first = false;
                if let Some((up_node, up_out)) = inp.get_connection() {
                    if let Some(g) = context.get_graph() {
                        if let Some(var) = g.get_connection_variable(up_node, up_out) {
                            call.push_str(&var);
                            continue;
                        }
                    }
                }
                let type_name = inp.get_type().get_name();
                call.push_str(&context.get_default_value(type_name, true));
            }
            call.push(')');

            if !self.return_struct.is_empty() {
                let result_var = format!("{}_result", node.get_name());
                stage.append_line(&format!("auto {} = {};", result_var, call));
            } else {
                let out_var = node
                    .get_outputs()
                    .next()
                    .map(|o| o.port.get_variable().to_string())
                    .unwrap_or_else(|| "out".to_string());
                let out_type = node
                    .get_outputs()
                    .next()
                    .map(|o| o.get_type().get_name())
                    .unwrap_or("float");
                let (emit_type, _) = context
                    .get_type_name_for_emit(out_type)
                    .unwrap_or(("float", "0.0"));
                stage.append_line(&format!("{} {} = {};", emit_type, out_var, call));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::format::read_from_xml_str;
    use crate::gen_mdl::MdlShaderGenerator;
    use crate::gen_shader::GenContext;

    #[test]
    fn source_code_node_mdl_uses_nodedef_output_count_for_return_struct() {
        let doc = read_from_xml_str(
            r#"<?xml version="1.0"?>
<materialx version="1.39">
  <nodedef name="ND_multi" node="multi" nodegroup="math">
    <output name="out1" type="float" />
    <output name="out2" type="float" />
  </nodedef>
  <implementation name="IM_multi" nodedef="ND_multi" function="multi_impl" />
</materialx>"#,
        )
        .expect("parse");
        let impl_elem = doc.get_implementation("IM_multi").expect("implementation");
        let ctx = GenContext::new(MdlShaderGenerator::create(None));
        let mut node = SourceCodeNodeMdl::new();
        node.initialize(&impl_elem, &ctx);
        assert_eq!(node.return_struct, "multi_impl__result");
    }

    #[test]
    fn source_code_node_mdl_replace_markers_rejects_malformed_inline_expression() {
        let result = std::panic::catch_unwind(|| {
            SourceCodeNodeMdl::replace_markers("foo({{broken)", "test_node", &|marker| {
                marker.to_string()
            })
        });
        assert!(
            result.is_err(),
            "malformed inline expressions must be rejected"
        );
    }
}
