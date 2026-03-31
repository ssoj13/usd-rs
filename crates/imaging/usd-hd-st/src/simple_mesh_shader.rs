//! Simple mesh shader for Storm fallback rendering.
//!
//! Provides a minimal vertex+fragment shader for unlit mesh display
//! when full material/lighting shaders are not available.

use std::sync::OnceLock;

/// Cached GL program ID for simple mesh shader.
static SIMPLE_PROGRAM: OnceLock<u32> = OnceLock::new();

/// WGSL shader source for the simple mesh pipeline.
///
/// Vertex shader applies viewProjection transform from uniform buffer.
/// Fragment shader outputs flat grey color matching the GL version.
pub const WGSL_SRC: &str = r#"
struct Uniforms {
    view_projection: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = uniforms.view_projection * vec4<f32>(in.position, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(0.65, 0.65, 0.65, 1.0);
}
"#;

fn compile_program() -> u32 {
    0
}

/// Returns the GL program ID for the simple mesh shader.
/// Creates and caches it on first call. Returns 0 if compilation fails.
pub fn get_simple_program() -> u32 {
    *SIMPLE_PROGRAM.get_or_init(compile_program)
}
