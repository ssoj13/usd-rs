//! HeightToNormalNodeMdl — height map to normal map conversion for MDL.
//! Ref: MaterialXGenMdl/Nodes/HeightToNormalNodeMdl.cpp
//!
//! Uses a 3x3 Sobel filter sampling grid to compute the normal from a float height field.
//! The filter calls `mx_normal_from_samples_sobel(samples[9], scale)` with 9 sample offsets
//! computed via `mx_compute_sample_size_uv`.

use crate::core::ElementPtr;
use crate::gen_shader::{ShaderImplContext, ShaderNode, ShaderNodeImpl, ShaderStage, hash_string};

/// Filter function name (Sobel normal-from-height)
const FILTER_FN: &str = "mx_normal_from_samples_sobel";
/// Sample size UV helper
const SAMPLE_SIZE_FN: &str = "mx_compute_sample_size_uv";
/// 3x3 grid = 9 samples
const SAMPLE_COUNT: usize = 9;
/// Filter kernel width for Sobel 3x3 convolution
#[allow(dead_code)]
const FILTER_WIDTH: usize = 3;
/// Default filter size
const FILTER_SIZE: f32 = 1.0;
/// Default filter offset
const FILTER_OFFSET: f32 = 0.0;

/// MDL height-to-normal node — converts a float height map to a normal map via Sobel filter.
#[derive(Debug, Default)]
pub struct HeightToNormalNodeMdl {
    name: String,
    hash: u64,
}

impl HeightToNormalNodeMdl {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create() -> Box<dyn ShaderNodeImpl> {
        Box::new(Self::new())
    }

    /// Build 3x3 Sobel sample offset strings (ref: computeSampleOffsetStrings).
    /// Each string is " + sampleSize * offsetType(col, row)" for a 3x3 grid.
    fn sample_offsets(sample_size_name: &str, offset_type: &str) -> Vec<String> {
        let mut offsets = Vec::with_capacity(SAMPLE_COUNT);
        for row in -1i32..=1 {
            for col in -1i32..=1 {
                offsets.push(format!(
                    " + {} * {}({:.1}, {:.1})",
                    sample_size_name, offset_type, col as f32, row as f32
                ));
            }
        }
        offsets
    }

    /// Emit input samples for UV-based convolution (ref: ConvolutionNode::emitInputSamplesUV).
    /// Returns sample variable names for each of the sampleCount samples.
    fn emit_input_samples_uv(
        node: &ShaderNode,
        context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
    ) -> Vec<String> {
        let mut sample_strings = Vec::new();

        let in_input = node.get_input("in");
        let out_var = node
            .get_outputs()
            .next()
            .map(|o| o.port.get_variable().to_string())
            .unwrap_or_else(|| "out".to_string());

        // Check for upstream connection on "in" input
        let has_upstream = in_input
            .map(|inp| inp.get_connection().is_some())
            .unwrap_or(false);

        if has_upstream {
            let in_input = in_input.unwrap();
            let (up_node_name, up_out_name) = in_input.get_connection().unwrap();

            // Check if upstream is a 2D-samplable node with a texcoord input
            if let Some(graph) = context.get_graph() {
                if let Some(up_node) = graph.get_node(up_node_name) {
                    let has_sampling_input = up_node
                        .get_input("texcoord")
                        .map(|i| i.get_type().get_name() == "vector2")
                        .unwrap_or(false);

                    if has_sampling_input {
                        // Get the sampling input value (texcoord)
                        let sample_input_value = up_node
                            .get_input("texcoord")
                            .and_then(|i| {
                                if let Some((n, o)) = i.get_connection() {
                                    graph.get_connection_variable(n, o)
                                } else {
                                    Some(
                                        i.port
                                            .get_value()
                                            .map(|v| v.get_value_string())
                                            .unwrap_or_else(|| {
                                                context.get_default_value("vector2", false)
                                            }),
                                    )
                                }
                            })
                            .unwrap_or_else(|| context.get_default_value("vector2", false));

                        // Emit sample size computation
                        let sample_size_name = format!("{}_sample_size", out_var);
                        stage.append_line(&format!(
                            "float2 {} = {}({}, {:.1}, {:.1});",
                            sample_size_name,
                            SAMPLE_SIZE_FN,
                            sample_input_value,
                            FILTER_SIZE,
                            FILTER_OFFSET
                        ));

                        // Build sample offset strings
                        let offsets = Self::sample_offsets(&sample_size_name, "float2");

                        // Get the upstream output variable
                        let up_out_var = graph
                            .get_connection_variable(up_node_name, up_out_name)
                            .unwrap_or_else(|| up_out_name.to_string());

                        // Emit sampling calls: re-emit upstream function call per sample with
                        // offset applied to sampling input. Each produces a distinct output variable.
                        for (i, offset) in offsets.iter().enumerate() {
                            let sample_out = format!("{}_{}", up_out_var, i);
                            // Emit the upstream call with modified texcoord (input + offset)
                            // In C++ this uses context input/output suffixes. We emit directly.
                            stage.append_line(&format!(
                                "float {} = {}({}{});",
                                sample_out, up_node_name, sample_input_value, offset
                            ));
                            sample_strings.push(sample_out);
                        }

                        return sample_strings;
                    }

                    // No sampling input — reuse the same upstream output for all samples
                    let up_out_var = graph
                        .get_connection_variable(up_node_name, up_out_name)
                        .unwrap_or_else(|| up_out_name.to_string());
                    for _ in 0..SAMPLE_COUNT {
                        sample_strings.push(up_out_var.clone());
                    }
                    return sample_strings;
                }
            }
        }

        // No upstream connection or upstream not samplable — use constant input value
        let in_value = in_input
            .and_then(|inp| {
                if let Some((up_n, up_o)) = inp.get_connection() {
                    context
                        .get_graph()
                        .and_then(|g| g.get_connection_variable(up_n, up_o))
                } else {
                    inp.port
                        .get_value()
                        .map(|v| v.get_value_string())
                        .or_else(|| Some(context.get_default_value("float", false)))
                }
            })
            .unwrap_or_else(|| context.get_default_value("float", false));

        for _ in 0..SAMPLE_COUNT {
            sample_strings.push(in_value.clone());
        }

        sample_strings
    }
}

impl ShaderNodeImpl for HeightToNormalNodeMdl {
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

    fn emit_function_call(
        &self,
        node: &ShaderNode,
        context: &dyn ShaderImplContext,
        stage: &mut ShaderStage,
    ) {
        if stage.get_name() != crate::gen_shader::shader_stage::PIXEL {
            return;
        }

        let _in_input = node.get_input("in");
        let scale_input = node.get_input("scale");

        if _in_input.is_none() || scale_input.is_none() {
            stage.append_line(&format!(
                "// Error: Node '{}' is not a valid heighttonormal node",
                node.get_name()
            ));
            return;
        }

        let out_var = node
            .get_outputs()
            .next()
            .map(|o| o.port.get_variable().to_string())
            .unwrap_or_else(|| "out".to_string());

        // Get scale value
        let scale_val = scale_input
            .and_then(|i| {
                if let Some((up_node, up_out)) = i.get_connection() {
                    context
                        .get_graph()
                        .and_then(|g| g.get_connection_variable(up_node, up_out))
                } else {
                    i.port
                        .get_value()
                        .map(|v| v.get_value_string())
                        .or_else(|| Some("1.0".to_string()))
                }
            })
            .unwrap_or_else(|| "1.0".to_string());

        // Emit input samples using UV convolution (ref: emitInputSamplesUV)
        let sample_strings = Self::emit_input_samples_uv(node, context, stage);

        // Emit code to evaluate samples array
        let samples_name = format!("{}_samples", out_var);
        let array_decl = format!("float[{}]", SAMPLE_COUNT);
        stage.append_line(&format!(
            "{} {} = {}(",
            array_decl, samples_name, array_decl
        ));
        for (i, sample) in sample_strings.iter().enumerate() {
            let comma = if i + 1 < SAMPLE_COUNT { "," } else { "" };
            stage.append_line(&format!("    {}{}", sample, comma));
        }
        stage.append_line(");");

        // Emit the Sobel filter call
        let out_type = node
            .get_outputs()
            .next()
            .map(|o| o.get_type().get_name())
            .unwrap_or("vector3");
        let (emit_type, _) = context
            .get_type_name_for_emit(out_type)
            .unwrap_or(("float3", "float3(0.0)"));
        stage.append_line(&format!(
            "{} {} = {}({}, {});",
            emit_type, out_var, FILTER_FN, samples_name, scale_val
        ));
    }
}
