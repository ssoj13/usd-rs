//! SurfaceNodeMdl -- MDL-specific surface node (ref: MaterialXGenMdl SurfaceNodeMdl).
//! Emits materialx::pbrlib_<version>::mx_surface(...) with named params.
//! For MDL < 1.9, adds mxp_transmission_ior parameter via findTransmissionIOR.

use crate::gen_shader::{
    ShaderImplContext, ShaderNode, ShaderNodeClassification, ShaderNodeImpl, ShaderStage,
    hash_string,
};

const MDL_PORT_PREFIX: &str = "mxp_";

/// MDL-specific surface node -- emits mx_surface with named params.
#[derive(Debug, Default)]
pub struct SurfaceNodeMdl {
    name: String,
    hash: u64,
}

impl SurfaceNodeMdl {
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

/// Recursively search for transmission IOR through BSDF nodes.
/// Ref: findTransmissionIOR in SurfaceNodeMdl.cpp
fn find_transmission_ior<'a>(
    node: &'a ShaderNode,
    graph: &'a crate::gen_shader::ShaderGraph,
) -> Option<String> {
    // Check if this is a BSDF_T node with IOR input
    if node.has_classification(ShaderNodeClassification::BSDF_T) {
        if let Some(ior_inp) = node.get_input("ior") {
            // Check scatter_mode for transparency
            let mut transparent = true;
            if let Some(scatter_mode) = node.get_input("scatter_mode") {
                if let Some(val) = scatter_mode.port.get_value() {
                    let mode_str = val.get_value_string();
                    transparent = mode_str == "T" || mode_str == "RT";
                }
            }
            if transparent {
                // Return the IOR value/connection
                if let Some((up_n, up_o)) = ior_inp.get_connection() {
                    if let Some(var) = graph.get_connection_variable(up_n, up_o) {
                        return Some(var);
                    }
                }
                if let Some(val) = ior_inp.port.get_value() {
                    return Some(val.get_value_string());
                }
            }
        }
    }

    // Recursively search through BSDF input connections
    for inp in node.get_inputs() {
        if inp.get_type().get_name() == "BSDF" || inp.get_type().get_name() == "material" {
            if let Some((up_name, _)) = inp.get_connection() {
                if let Some(up_node) = graph.get_node(up_name) {
                    if let Some(ior) = find_transmission_ior(up_node, graph) {
                        return Some(ior);
                    }
                }
            }
        }
    }
    None
}

impl ShaderNodeImpl for SurfaceNodeMdl {
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
        node.add_classification(ShaderNodeClassification::SHADER);
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

        // Emit dependent closure calls (ref: emitDependentFunctionCalls(node, CLOSURE))
        // In our Rust port, closure deps resolved by topological traversal

        self.emit_output_variables(node, context, stage);

        let output = node.get_outputs().next().expect("Surface has out");
        let out_var = output.port.get_variable();

        // Get MDL version suffix from context
        let version_suffix = context.get_mdl_version_suffix();

        // Check for transmission IOR for MDL versions before 1.9
        // Ref: SurfaceNodeMdl::emitFunctionCall IOR handling
        let ior_value =
            if version_suffix == "1_6" || version_suffix == "1_7" || version_suffix == "1_8" {
                // Find transmission IOR in the BSDF tree
                context
                    .get_graph()
                    .and_then(|graph| find_transmission_ior(node, graph))
            } else {
                None
            };

        let mut args = Vec::new();
        for inp in node.get_inputs() {
            let port_name = format!("{}{}", MDL_PORT_PREFIX, inp.get_name());
            let val = self.emit_input_value(node, inp.get_name(), context);
            args.push(format!("{}: {}", port_name, val));
        }

        // Add transmission IOR parameter if found (ref: mxp_transmission_ior)
        if let Some(ior) = ior_value {
            args.push(format!("mxp_transmission_ior: {}", ior));
        }

        stage.append_line(&format!(
            "{} = materialx::pbrlib_{}::mx_surface({});",
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
            let type_name = output.get_type().get_name();
            let (emit_type, default_val) = context
                .get_type_name_for_emit(type_name)
                .unwrap_or(("material", "material()"));
            let var = output.port.get_variable();
            stage.append_line(&format!("{} {} = {};", emit_type, var, default_val));
        }
    }
}
