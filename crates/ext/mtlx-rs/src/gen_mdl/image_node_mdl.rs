//! ImageNodeMdl -- MDL image node (ref: MaterialXGenMdl/Nodes/ImageNodeMdl.cpp).
//! Extends SourceCodeNodeMdl to add a `flip_v` boolean input that controls
//! vertical texture coordinate flipping based on GenContext options.

use crate::core::ElementPtr;
use crate::gen_shader::{
    ShaderImplContext, ShaderNode, ShaderNodeImpl, ShaderStage, hash_string, type_desc_types,
};

/// Name of the additional flip_v parameter
pub const FLIP_V: &str = "flip_v";

/// MDL image node -- adds flip_v input for vertical texture flipping.
#[derive(Debug, Default)]
pub struct ImageNodeMdl {
    name: String,
    hash: u64,
    function_name: String,
    function_source: String,
    inlined: bool,
}

impl ImageNodeMdl {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create() -> Box<dyn ShaderNodeImpl> {
        Box::new(Self::new())
    }
}

impl ShaderNodeImpl for ImageNodeMdl {
    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_hash(&self) -> u64 {
        self.hash
    }

    fn initialize(&mut self, element: &ElementPtr, _context: &dyn ShaderImplContext) {
        let el = element.borrow();
        self.name = el.get_name().to_string();
        self.hash = hash_string(&self.name);
        self.function_name = el.get_attribute("function").unwrap_or_default().to_string();
        self.function_source = el
            .get_attribute("sourcecode")
            .unwrap_or_default()
            .to_string();
        self.inlined = !self.function_source.is_empty();
    }

    fn add_inputs(&self, node: &mut ShaderNode, _context: &dyn ShaderImplContext) {
        // Add the flip_v boolean input (uniform) -- ref: ImageNodeMdl::addInputs
        let inp = node.add_input(FLIP_V, type_desc_types::boolean());
        inp.port_mut().set_uniform(true);
    }

    fn is_editable(&self, input_name: &str) -> bool {
        // flip_v is not user-editable -- it's set from context options
        input_name != FLIP_V
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

        // Get flip_v from context options (ref: context.getOptions().fileTextureVerticalFlip)
        let flip_v = context.get_file_texture_vertical_flip();
        let flip_v_str = if flip_v { "true" } else { "false" };

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

        if self.inlined {
            let code = super::source_code_node_mdl::SourceCodeNodeMdl::replace_markers(
                &self.function_source,
                &self.name,
                &|marker| {
                    if marker == "MDL_VERSION_SUFFIX" {
                        return context.get_mdl_version_suffix().to_string();
                    }
                    if marker == FLIP_V {
                        return flip_v_str.to_string();
                    }
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
                    marker.to_string()
                },
            );

            let (emit_type, _) = context
                .get_type_name_for_emit(out_type)
                .unwrap_or(("float", "0.0"));
            stage.append_line(&format!("{} {} = {};", emit_type, out_var, code));
        } else {
            // Non-inlined function call
            let mut call = format!("{}(", self.function_name);
            let mut first = true;
            for inp in node.get_inputs() {
                if !first {
                    call.push_str(", ");
                }
                first = false;
                if inp.get_name() == FLIP_V {
                    call.push_str(flip_v_str);
                    continue;
                }
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

            let (emit_type, _) = context
                .get_type_name_for_emit(out_type)
                .unwrap_or(("float", "0.0"));
            stage.append_line(&format!("{} {} = {};", emit_type, out_var, call));
        }
    }
}
