//! MaterialNodeMdl -- MDL-specific material node (ref: MaterialXGenMdl MaterialNodeMdl).
//! Emits materialx::stdlib_<version>::mx_surfacematerial(surfaceshader, backsurfaceshader).

use crate::gen_shader::{
    ShaderImplContext, ShaderNode, ShaderNodeClassification, ShaderNodeImpl, ShaderStage,
    hash_string,
};

const MDL_PORT_PREFIX: &str = "mxp_";
const SURFACESHADER: &str = "surfaceshader";
#[allow(dead_code)]
const BACKSURFACESHADER: &str = "backsurfaceshader";

/// MDL-specific material node -- emits mx_surfacematerial with named params.
#[derive(Debug, Default)]
pub struct MaterialNodeMdl {
    name: String,
    hash: u64,
}

impl MaterialNodeMdl {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create() -> Box<dyn ShaderNodeImpl> {
        Box::new(Self::new())
    }

    fn emit_input_value(
        &self,
        node: &ShaderNode,
        input_name: &str,
        context: &dyn ShaderImplContext,
    ) -> String {
        let inp = match node.get_input(input_name) {
            Some(i) => i,
            None => return context.get_default_value("material", true),
        };
        if let Some((up_node, up_output)) = inp.get_connection() {
            if let Some(g) = context.get_graph() {
                if let Some(var) = g.get_connection_variable(up_node, up_output) {
                    return var;
                }
            }
        }
        let type_name = inp.get_type().get_name();
        context.get_default_value(type_name, true)
    }
}

impl ShaderNodeImpl for MaterialNodeMdl {
    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_hash(&self) -> u64 {
        self.hash
    }

    fn initialize(&mut self, element: &crate::core::ElementPtr, _context: &dyn ShaderImplContext) {
        self.name = element.borrow().get_name().to_string();
        self.hash = hash_string(&self.name);
    }

    fn add_classification(&self, node: &mut ShaderNode) {
        if node.get_input(SURFACESHADER).is_some() {
            node.add_classification(ShaderNodeClassification::MATERIAL);
        }
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

        let surfaceshader_inp = match node.get_input(SURFACESHADER) {
            Some(i) => i,
            None => {
                self.emit_output_variables(node, context, stage);
                return;
            }
        };

        if surfaceshader_inp.get_connection().is_none() {
            self.emit_output_variables(node, context, stage);
            return;
        }

        // Emit function call for upstream surface shader (ref: MaterialNodeMdl::emitFunctionCall)
        // The upstream surface shader node's call is already emitted by topological traversal.

        // Emit function call for upstream backsurface shader if connected as sibling
        // Ref: MaterialNodeMdl.cpp checks backsurfaceshaderInput->getConnectedSibling()

        self.emit_output_variables(node, context, stage);

        let output = node.get_outputs().next().expect("Material has out");
        let out_var = output.port.get_variable();

        // Use version suffix from context (ref: emitMdlVersionFilenameSuffix)
        let version_suffix = context.get_mdl_version_suffix();

        let mut args = Vec::new();
        for inp in node.get_inputs() {
            let port_name = format!("{}{}", MDL_PORT_PREFIX, inp.get_name());
            let val = self.emit_input_value(node, inp.get_name(), context);
            args.push(format!("{}: {}", port_name, val));
        }

        stage.append_line(&format!(
            "{} = materialx::stdlib_{}::mx_surfacematerial({});",
            out_var,
            version_suffix,
            args.join(", ")
        ));
    }

    fn emit_output_variables(
        &self,
        node: &ShaderNode,
        context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
    ) {
        for output in node.get_outputs() {
            let type_desc = output.get_type();
            let type_name = type_desc.get_name();
            let (emit_type, default_val) = context
                .get_type_name_for_emit(type_name)
                .unwrap_or(("material", "material()"));
            let var = output.port.get_variable();
            stage.append_line(&format!("{} {} = {};", emit_type, var, default_val));
        }
    }
}
