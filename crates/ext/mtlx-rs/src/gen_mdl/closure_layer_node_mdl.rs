//! ClosureLayerNodeMdl -- MDL BSDF closure layering node.
//! Ref: MaterialXGenMdl/Nodes/ClosureLayerNodeMdl.cpp
//!
//! MDL does not have a generic layer operator. Instead, layering is achieved by
//! passing the base BSDF into the `base` input of the top BSDF node.
//! Special cases:
//! - BSDF-over-VDF: joins top BSDF surface/backface/ior with base VDF volume
//! - BSDF-over-BSDF: wires base into top node's base input via
//!   makeConnection/ScopedSetVariableName/breakConnection emulation
//! - Top without base input: emits only the top BSDF with a warning comment

use crate::core::ElementPtr;
use crate::gen_shader::{
    ShaderImplContext, ShaderNode, ShaderNodeClassification, ShaderNodeImpl, ShaderStage,
    hash_string,
};

/// Input/output port name constants for closure layer node (ref: StringConstantsMdl).
pub mod port {
    pub const TOP: &str = "top";
    pub const BASE: &str = "base";
    pub const FG: &str = "fg";
    pub const BG: &str = "bg";
    pub const MIX: &str = "mix";
    pub const TOP_WEIGHT: &str = "top_weight";
}

/// MDL closure layer node -- layers BSDF closures via MDL base input nesting.
#[derive(Debug, Default)]
pub struct ClosureLayerNodeMdl {
    name: String,
    hash: u64,
}

impl ClosureLayerNodeMdl {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create() -> Box<dyn ShaderNodeImpl> {
        Box::new(Self::new())
    }

    /// Get the upstream result variable for an input (resolve connection or default).
    fn get_upstream_result(
        inp: &crate::gen_shader::ShaderInput,
        context: &dyn ShaderImplContext,
    ) -> String {
        if let Some((up_node, up_out)) = inp.get_connection() {
            if let Some(g) = context.get_graph() {
                if let Some(var) = g.get_connection_variable(up_node, up_out) {
                    return var;
                }
            }
        }
        context.get_default_value(inp.get_type().get_name(), false)
    }

    /// Re-emit a node's function call with a modified base input and output variable name.
    /// This emulates the C++ pattern of makeConnection + ScopedSetVariableName + breakConnection.
    /// Ref: ClosureLayerNodeMdl::emitFunctionCall BSDF-over-BSDF path
    fn emit_layered_call(
        top_node: &ShaderNode,
        _base_receiver_name: &str,
        base_var: &str,
        mix_weight_var: Option<&str>,
        out_var: &str,
        context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
    ) {
        // Build the function call for the top node, replacing the base input
        // with the actual base BSDF variable and the output variable name
        let impl_name = top_node.get_impl_name().unwrap_or("").to_string();

        // Resolve the function name from the implementation
        // Function name is derived from the implementation name

        // Build the call by iterating inputs, replacing "base" with the base_var
        let mut call_args = Vec::new();
        for inp in top_node.get_inputs() {
            let inp_name = inp.get_name();
            let val = if inp_name == port::BASE {
                // Wire the base BSDF variable
                base_var.to_string()
            } else if inp_name == port::TOP_WEIGHT {
                // Wire the mix weight if provided
                if let Some(weight) = mix_weight_var {
                    weight.to_string()
                } else {
                    Self::get_upstream_result(inp, context)
                }
            } else {
                Self::get_upstream_result(inp, context)
            };
            call_args.push(format!("mxp_{}: {}", inp_name, val));
        }

        // Emit the layered call with the layer node's output variable name
        // We need to re-emit the top node's function call with its proper function
        // The top node is a SourceCodeNodeMdl (layerable BSDF) with an inlined function source
        // We construct the call from the inline template with replaced base input
        let top_out_type = top_node
            .get_outputs()
            .next()
            .map(|o| o.get_type().get_name())
            .unwrap_or("material");
        let (emit_type, _) = context
            .get_type_name_for_emit(top_out_type)
            .unwrap_or(("material", "material()"));

        // Emit the top node's function call with named parameters.
        // The function name comes from the implementation name or node name.
        // In C++ this uses getSourceCode() for inline nodes or getFunction() for non-inline.
        // In our model we always emit as a function call with named parameters.
        let func = impl_name;
        let func_name_final = if func.is_empty() {
            top_node.get_name().to_string()
        } else {
            func
        };
        stage.append_line(&format!(
            "{} {} = {}({});",
            emit_type,
            out_var,
            func_name_final,
            call_args.join(", ")
        ));
    }
}

impl ShaderNodeImpl for ClosureLayerNodeMdl {
    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_hash(&self) -> u64 {
        self.hash
    }

    fn initialize(&mut self, element: &ElementPtr, _context: &dyn ShaderImplContext) {
        self.name = element.borrow().get_name().to_string();
        self.hash = hash_string(&self.name);
    }

    fn add_classification(&self, node: &mut ShaderNode) {
        node.add_classification(ShaderNodeClassification::LAYER);
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

        let out_var = node
            .get_outputs()
            .next()
            .map(|o| o.port.get_variable().to_string())
            .unwrap_or_else(|| "out".to_string());

        let top_input = node.get_input(port::TOP);
        let base_input = node.get_input(port::BASE);

        // ---- 1. Handle the BSDF-over-VDF case ----
        if let Some(base_inp) = base_input {
            if base_inp.get_type().get_name() == "VDF" || base_inp.get_type().get_name() == "vdf" {
                // Make sure we have a top BSDF connected
                let top_conn = top_input.and_then(|i| i.get_connection());
                if top_conn.is_none() {
                    stage.append_line(&format!("material {} = material();", out_var));
                    return;
                }

                let (top_node_name, _) = top_conn.unwrap();
                let top_node_name = top_node_name.to_string();

                // Emit function call for top node if sibling
                if let Some(graph) = context.get_graph() {
                    if let Some(top_node) = graph.get_node(&top_node_name) {
                        if top_node.get_parent_name() == node.get_parent_name() {
                            // Top node call emitted by topological traversal
                        }
                    }
                }

                // Emit function call for base node
                if let Some((base_node_name, _)) = base_inp.get_connection() {
                    let base_node_name = base_node_name.to_string();
                    if let Some(graph) = context.get_graph() {
                        if graph.get_node(&base_node_name).is_some() {
                            // Base node call emitted by topological traversal
                        }
                    }
                }

                let t = Self::get_upstream_result(top_input.unwrap(), context);
                let b = Self::get_upstream_result(base_inp, context);

                // Join BSDF and VDF into a single material
                stage.append_line(&format!(
                    "material {} = material(surface: {}.surface, backface: {}.backface, ior: {}.ior, volume: {}.volume);",
                    out_var, t, t, t, b
                ));
                return;
            }
        }

        // ---- 2. Handle the BSDF-over-BSDF case ----

        let top_conn = top_input.and_then(|i| i.get_connection());
        let base_conn = base_input.and_then(|i| i.get_connection());

        // Check layer is fully connected
        if top_conn.is_none() || base_conn.is_none() {
            stage.append_line(&format!("material {} = material();", out_var));
            return;
        }

        let (top_node_name, _) = top_conn.unwrap();
        let (base_node_name, _) = base_conn.unwrap();
        let top_node_name = top_node_name.to_string();
        let _base_node_name = base_node_name.to_string();

        // Check if top is a sibling (not graph interface)
        let top_is_sibling = context
            .get_graph()
            .map(|g| g.get_node(&top_node_name).is_some())
            .unwrap_or(false);

        if !top_is_sibling {
            stage.append_line(
                "// Warning: MDL has no support for layering BSDFs through a graph interface. Only the top BSDF will used.",
            );
            let t = Self::get_upstream_result(top_input.unwrap(), context);
            stage.append_line(&format!("material {} = {};", out_var, t));
            return;
        }

        // Walk down the top node chain to find the base receiver
        let mut base_receiver_name = top_node_name.clone();
        let mut mix_top_weight_node: Option<String> = None;

        if let Some(graph) = context.get_graph() {
            loop {
                let receiver = graph.get_node(&base_receiver_name);
                if receiver.is_none() {
                    break;
                }
                let receiver = receiver.unwrap();

                if receiver.has_classification(ShaderNodeClassification::LAYER) {
                    // Follow layer's base input to find the elemental BSDF
                    if let Some(base_inp) = receiver.get_input(port::BASE) {
                        if let Some((next_name, _)) = base_inp.get_connection() {
                            base_receiver_name = next_name.to_string();
                            continue;
                        }
                    }
                    break;
                } else {
                    // Check for mix_bsdf special case
                    let impl_name = receiver.get_impl_name().unwrap_or("").to_string();
                    if impl_name == "IM_mix_bsdf_genmdl" {
                        let fg_conn = receiver
                            .get_input(port::FG)
                            .and_then(|i| i.get_connection())
                            .map(|(n, _)| n.to_string());
                        let bg_conn = receiver
                            .get_input(port::BG)
                            .and_then(|i| i.get_connection())
                            .map(|(n, _)| n.to_string());
                        let mix_conn = receiver
                            .get_input(port::MIX)
                            .and_then(|i| i.get_connection())
                            .map(|(n, _)| n.to_string());

                        let has_fg = fg_conn.is_some();
                        let has_bg = bg_conn.is_some();

                        if has_fg != has_bg {
                            let valid_node = if has_fg {
                                fg_conn.unwrap()
                            } else {
                                bg_conn.unwrap()
                            };
                            base_receiver_name = valid_node;
                            mix_top_weight_node = mix_conn;
                        }
                    }
                    break;
                }
            }
        }

        // Check if the base receiver has a "base" input for nesting
        let receiver_has_base = context
            .get_graph()
            .and_then(|g| g.get_node(&base_receiver_name))
            .and_then(|n| n.get_input(port::BASE))
            .is_some();

        if !receiver_has_base {
            stage.append_line(
                "// Warning: MDL has no support for layering BSDF nodes without a base input. Only the top BSDF will used.",
            );
            // Emit the top BSDF with the layer node's output variable name
            // Ref: ScopedSetVariableName(output->getVariable(), top->getOutput())
            if let Some(graph) = context.get_graph() {
                if let Some(top_node) = graph.get_node(&top_node_name) {
                    // Re-emit top node call with our output variable
                    Self::emit_layered_call(
                        top_node,
                        &base_receiver_name,
                        "material()",
                        None,
                        &out_var,
                        context,
                        stage,
                    );
                    return;
                }
            }
            let t = Self::get_upstream_result(top_input.unwrap(), context);
            stage.append_line(&format!("material {} = {};", out_var, t));
            return;
        }

        // Emit the base BSDF function call (already emitted by topological order)
        // Get the base variable
        let base_var = Self::get_upstream_result(base_input.unwrap(), context);

        // Get mix weight variable if present
        let mix_weight_var = mix_top_weight_node.as_ref().and_then(|weight_name| {
            context.get_graph().and_then(|g| {
                let weight_node = g.get_node(weight_name)?;
                let weight_out = weight_node.get_outputs().next()?;
                Some(weight_out.port.get_variable().to_string())
            })
        });

        // Emit the layer operation: re-emit top node's function call with base wired in
        // Ref: topNodeBaseInput->makeConnection(base->getOutput());
        //      ScopedSetVariableName setVariable(output->getVariable(), top->getOutput());
        //      top->getImplementation().emitFunctionCall(*top, context, stage);
        //      topNodeBaseInput->breakConnection();
        if let Some(graph) = context.get_graph() {
            if let Some(top_node) = graph.get_node(&top_node_name) {
                Self::emit_layered_call(
                    top_node,
                    &base_receiver_name,
                    &base_var,
                    mix_weight_var.as_deref(),
                    &out_var,
                    context,
                    stage,
                );
                return;
            }
        }

        // Fallback: emit the layered result directly
        let top_var = Self::get_upstream_result(top_input.unwrap(), context);
        stage.append_line(&format!(
            "material {} = {};  // layered: base={}",
            out_var, top_var, base_var
        ));
    }
}
