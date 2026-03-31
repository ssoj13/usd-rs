//! ConvolutionNode — abstract base for MDL convolution operations.
//! Ref: MaterialXGenMdl/Nodes/ConvolutionNode.cpp
//!
//! Provides:
//! - Box and Gaussian filter weight constants
//! - UV-space sampling with offset computation
//! - Subclasses implement `accepts_input_type` and `compute_sample_offset_strings`

use crate::gen_shader::ShaderStage;

/// Gaussian kernel 3x3 weights (1D separable)
pub const GAUSSIAN_KERNEL_3: [f32; 3] = [0.27901, 0.44198, 0.27901];
/// Gaussian kernel 5x5 weights (1D separable)
pub const GAUSSIAN_KERNEL_5: [f32; 5] = [0.06136, 0.24477, 0.38774, 0.24477, 0.06136];
/// Gaussian kernel 7x7 weights (1D separable)
pub const GAUSSIAN_KERNEL_7: [f32; 7] = [
    0.00598, 0.060626, 0.241843, 0.383103, 0.241843, 0.060626, 0.00598,
];

/// Box filter weights: 1/9, 1/25, 1/49
pub fn box_filter_weights() -> Vec<f32> {
    let mut weights = Vec::with_capacity(1 + 9 + 25 + 49);
    weights.push(1.0);
    for _ in 0..9 {
        weights.push(1.0 / 9.0);
    }
    for _ in 0..25 {
        weights.push(1.0 / 25.0);
    }
    for _ in 0..49 {
        weights.push(1.0 / 49.0);
    }
    weights
}

/// Gaussian filter weights: 3x3, 5x5, 7x7 (2D from separable 1D kernels)
pub fn gaussian_filter_weights() -> Vec<f32> {
    let mut weights = Vec::with_capacity(1 + 9 + 25 + 49);
    weights.push(1.0);
    for y in 0..3 {
        for x in 0..3 {
            weights.push(GAUSSIAN_KERNEL_3[y] * GAUSSIAN_KERNEL_3[x]);
        }
    }
    for y in 0..5 {
        for x in 0..5 {
            weights.push(GAUSSIAN_KERNEL_5[y] * GAUSSIAN_KERNEL_5[x]);
        }
    }
    for y in 0..7 {
        for x in 0..7 {
            weights.push(GAUSSIAN_KERNEL_7[y] * GAUSSIAN_KERNEL_7[x]);
        }
    }
    weights
}

/// Emit box and Gaussian weight constant arrays into the stage constants block.
/// Ref: ConvolutionNode::createVariables
pub fn emit_convolution_constants(stage: &mut ShaderStage) {
    // Emit box filter weights as constant array
    let box_w = box_filter_weights();
    let box_str: Vec<String> = box_w.iter().map(|w| format!("{:.6}", w)).collect();
    stage.append_line(&format!(
        "float c_box_filter_weights[{}] = float[{}]({});",
        box_w.len(),
        box_w.len(),
        box_str.join(", ")
    ));

    // Emit Gaussian filter weights as constant array
    let gauss_w = gaussian_filter_weights();
    let gauss_str: Vec<String> = gauss_w.iter().map(|w| format!("{:.6}", w)).collect();
    stage.append_line(&format!(
        "float c_gaussian_filter_weights[{}] = float[{}]({});",
        gauss_w.len(),
        gauss_w.len(),
        gauss_str.join(", ")
    ));
}
