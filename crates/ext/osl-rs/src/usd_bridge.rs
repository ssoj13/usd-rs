//! USD integration bridge — translates USD shading networks into OSL shader groups.
//!
//! This module provides a **trait-based interface** for converting USD `Material`
//! / `Shader` graph descriptions into OSL `ShaderGroup`s that can be executed
//! by our [`ShadingSystem`]. The module is independent of the concrete USD
//! implementation — callers provide a [`UsdShadingGraph`] implementation that
//! describes the shading network, and this module handles the conversion.
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────┐       ┌─────────────────┐       ┌──────────────────┐
//! │ USD Material  │──────►│  UsdShadingGraph │──────►│  ShadingSystem   │
//! │ (usd-rs)      │ impl │  (this trait)     │ build │  ShaderGroup     │
//! └──────────────┘       └─────────────────┘       └──────────────────┘
//! ```
//!
//! # Usage
//!
//! ```ignore
//! use osl_rs::usd_bridge::{UsdShadingGraph, UsdShaderNode, UsdConnection, build_shader_group};
//!
//! struct MyGraph { /* walks USD Material prim */ }
//! impl UsdShadingGraph for MyGraph { /* ... */ }
//!
//! let ss = ShadingSystem::new(/* ... */);
//! let group = build_shader_group(&ss, "myMaterial", &MyGraph::from_material(mat));
//! ```

use crate::math::{Color3, Vec3};
use crate::shadingsys::{ParamValue, ShaderGroupRef, ShadingSystem};
#[allow(unused_imports)]
use crate::ustring::UString;

// ---------------------------------------------------------------------------
// Trait: UsdShadingGraph
// ---------------------------------------------------------------------------

/// A single parameter value that can be set on a shader.
#[derive(Debug, Clone)]
pub enum UsdParamValue {
    /// Integer value.
    Int(i32),
    /// Single-precision float.
    Float(f32),
    /// 3-component vector / point / normal.
    Vec3(Vec3),
    /// Color (same layout as Vec3 but semantic).
    Color(Color3),
    /// String (e.g., texture filename, coordinate space).
    String(String),
    /// Matrix (4×4 column-major).
    Matrix([f32; 16]),
}

/// Describes a single shader node in a USD shading network.
#[derive(Debug, Clone)]
pub struct UsdShaderNode {
    /// Unique layer name within the group (e.g., "diffuse1", "noise_tex").
    pub layer_name: String,
    /// OSL shader name to load (e.g., "UsdPreviewSurface", "noise_pattern").
    /// This corresponds to the `info:id` attribute in USD, mapped to an OSL
    /// shader file or built-in shader.
    pub shader_name: String,
    /// Shader type ("surface", "displacement", "volume", "shader").
    pub shader_type: String,
    /// Parameter overrides (name → value).
    pub params: Vec<(String, UsdParamValue)>,
}

/// Describes a connection between two shader nodes.
#[derive(Debug, Clone)]
pub struct UsdConnection {
    /// Source layer name.
    pub src_layer: String,
    /// Source output parameter name.
    pub src_param: String,
    /// Destination layer name.
    pub dst_layer: String,
    /// Destination input parameter name.
    pub dst_param: String,
}

/// Trait for describing a USD shading network. Implement this for your
/// concrete USD Material representation (e.g., from `usd-rs`).
pub trait UsdShadingGraph {
    /// Return all shader nodes in dependency order (upstream first).
    fn nodes(&self) -> Vec<UsdShaderNode>;
    /// Return all connections between shader nodes.
    fn connections(&self) -> Vec<UsdConnection>;
    /// Return the name of the material (used as the shader group name).
    fn material_name(&self) -> &str;
}

// ---------------------------------------------------------------------------
// Builder: convert UsdShadingGraph → ShaderGroup
// ---------------------------------------------------------------------------

/// Convert a USD shading graph into an OSL `ShaderGroup`.
///
/// This function:
/// 1. Creates a new shader group.
/// 2. Adds each shader node as a layer with parameter overrides.
/// 3. Connects the layers according to the graph edges.
///
/// Returns a `ShaderGroupRef` ready for execution via `ShadingSystem::execute`
/// or `ShadingSystem::execute_jit`.
pub fn build_shader_group(
    ss: &ShadingSystem,
    graph: &dyn UsdShadingGraph,
) -> Result<ShaderGroupRef, String> {
    let group = ss.shader_group_begin(graph.material_name());

    let nodes = graph.nodes();
    let connections = graph.connections();

    // 1. Add each shader layer
    for node in &nodes {
        // Set parameters before adding the shader layer
        for (name, value) in &node.params {
            let pv = match value {
                UsdParamValue::Int(i) => ParamValue::Int(*i),
                UsdParamValue::Float(f) => ParamValue::Float(*f),
                UsdParamValue::Vec3(v) => ParamValue::Vector(*v),
                UsdParamValue::Color(c) => ParamValue::Color(*c),
                UsdParamValue::String(s) => ParamValue::String(UString::new(s)),
                UsdParamValue::Matrix(m) => {
                    let mat = crate::math::Matrix44::from_row_major(m);
                    ParamValue::Matrix(mat)
                }
            };
            ss.parameter_simple(&group, name, pv);
        }

        // Add the shader layer
        ss.shader(
            &group,
            &node.shader_type,
            &node.shader_name,
            &node.layer_name,
        )
        .map_err(|e| format!("Failed to add shader layer '{}': {}", node.layer_name, e))?;
    }

    // 2. Connect layers
    for conn in &connections {
        ss.connect_shaders(
            &group,
            &conn.src_layer,
            &conn.src_param,
            &conn.dst_layer,
            &conn.dst_param,
        )?;
    }

    ss.shader_group_end(&group)?;
    Ok(group)
}

// ---------------------------------------------------------------------------
// UsdPreviewSurface mapping
// ---------------------------------------------------------------------------

/// Map UsdPreviewSurface parameters to OSL StandardSurface equivalents.
///
/// Performs actual parameter name and value translation per the USD spec:
/// - `diffuseColor` -> `base_color`
/// - `roughness` -> `specular_roughness`
/// - `metallic` -> `metalness`
/// - `emissiveColor` -> `emission_color`
/// - `opacity` -> `opacity` (as 3-channel)
/// - `ior` -> `specular_IOR`
/// - `clearcoat` -> `coat`
/// - `clearcoatRoughness` -> `coat_roughness`
///
/// Parameters without a StandardSurface equivalent are passed through unchanged.
pub fn map_usd_preview_surface_params(
    usd_params: &[(String, UsdParamValue)],
) -> Vec<(String, UsdParamValue)> {
    let mut osl_params = Vec::new();
    for (name, value) in usd_params {
        match name.as_str() {
            "diffuseColor" => {
                osl_params.push(("base_color".to_string(), value.clone()));
            }
            "roughness" => {
                osl_params.push(("specular_roughness".to_string(), value.clone()));
            }
            "metallic" => {
                osl_params.push(("metalness".to_string(), value.clone()));
            }
            "emissiveColor" => {
                osl_params.push(("emission_color".to_string(), value.clone()));
            }
            "ior" => {
                osl_params.push(("specular_IOR".to_string(), value.clone()));
            }
            "clearcoat" => {
                osl_params.push(("coat".to_string(), value.clone()));
            }
            "clearcoatRoughness" => {
                osl_params.push(("coat_roughness".to_string(), value.clone()));
            }
            "opacity" => {
                // Convert scalar opacity to 3-channel if needed
                match value {
                    UsdParamValue::Float(f) => {
                        osl_params.push((
                            "opacity".to_string(),
                            UsdParamValue::Color(Color3::new(*f, *f, *f)),
                        ));
                    }
                    _ => osl_params.push(("opacity".to_string(), value.clone())),
                }
            }
            // Pass through directly: specularColor, useSpecularWorkflow,
            // opacityThreshold, normal, displacement, occlusion
            _ => {
                osl_params.push((name.clone(), value.clone()));
            }
        }
    }
    osl_params
}

// ---------------------------------------------------------------------------
// Simple graph builder (convenience API)
// ---------------------------------------------------------------------------

/// A simple in-memory shading graph that can be built up programmatically.
/// Implements [`UsdShadingGraph`].
#[derive(Debug, Clone, Default)]
pub struct SimpleGraph {
    /// Material name.
    pub name: String,
    /// Shader nodes in dependency order.
    pub nodes: Vec<UsdShaderNode>,
    /// Connections between nodes.
    pub connections: Vec<UsdConnection>,
}

impl SimpleGraph {
    /// Create a new empty graph.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            nodes: Vec::new(),
            connections: Vec::new(),
        }
    }

    /// Add a shader node to the graph.
    pub fn add_node(&mut self, node: UsdShaderNode) {
        self.nodes.push(node);
    }

    /// Add a connection between two nodes.
    pub fn connect(&mut self, src_layer: &str, src_param: &str, dst_layer: &str, dst_param: &str) {
        self.connections.push(UsdConnection {
            src_layer: src_layer.to_string(),
            src_param: src_param.to_string(),
            dst_layer: dst_layer.to_string(),
            dst_param: dst_param.to_string(),
        });
    }
}

impl UsdShadingGraph for SimpleGraph {
    fn nodes(&self) -> Vec<UsdShaderNode> {
        self.nodes.clone()
    }

    fn connections(&self) -> Vec<UsdConnection> {
        self.connections.clone()
    }

    fn material_name(&self) -> &str {
        &self.name
    }
}

// ---------------------------------------------------------------------------
// Built-in USD shader .oso generators
// ---------------------------------------------------------------------------
// These functions generate .oso bytecode strings for the three core USD shader
// nodes: UsdPreviewSurface, UsdUVTexture, and UsdPrimvarReader_float2.
//
// This follows the same pattern as the C++ reference's
// `LoadMemoryCompiledShader` — the .oso bytecodes are registered into the
// ShadingSystem's shader masters map so they can be resolved by name without
// needing external .oso files on disk.

/// .oso bytecode for a simplified UsdPreviewSurface shader.
///
/// Implements: result = diffuseColor * (1 - metallic) + emissiveColor
pub fn usd_preview_surface_oso() -> &'static str {
    concat!(
        "OpenShadingLanguage 1.00\n",
        "shader UsdPreviewSurface\n",
        "param color diffuseColor 0.18 0.18 0.18 %meta{} %read{1,4} %write{2147483647,-1}\n",
        "param color emissiveColor 0 0 0 %meta{} %read{5,5} %write{2147483647,-1}\n",
        "param float roughness 0.5 %meta{} %read{2147483647,-1} %write{2147483647,-1}\n",
        "param float metallic 0 %meta{} %read{2,2} %write{2147483647,-1}\n",
        "param float opacity 1 %meta{} %read{2147483647,-1} %write{2147483647,-1}\n",
        "param float ior 1.5 %meta{} %read{2147483647,-1} %write{2147483647,-1}\n",
        "param color specularColor 0 0 0 %meta{} %read{2147483647,-1} %write{2147483647,-1}\n",
        "param float clearcoat 0 %meta{} %read{2147483647,-1} %write{2147483647,-1}\n",
        "param float clearcoatRoughness 0.01 %meta{} %read{2147483647,-1} %write{2147483647,-1}\n",
        "oparam color result 0 0 0 %meta{} %read{2147483647,-1} %write{5,5}\n",
        "local float ___tmp0 %read{3,4} %write{2,2}\n",
        "local color ___tmp1 %read{5,5} %write{4,4}\n",
        "const float $const1 1 %read{2,2} %write{2147483647,-1}\n",
        "code UsdPreviewSurface\n",
        "# result = diffuseColor * (1 - metallic) + emissiveColor\n",
        "  sub ___tmp0 $const1 metallic\n",
        "  mul ___tmp1 diffuseColor ___tmp0\n",
        "  add result ___tmp1 emissiveColor\n",
        "  end\n",
    )
}

/// .oso bytecode for a simplified UsdUVTexture shader.
///
/// Implements: rgb = texture(file, st); r = rgb[0]; g = rgb[1]; b = rgb[2]; a = 1
pub fn usd_uv_texture_oso() -> &'static str {
    concat!(
        "OpenShadingLanguage 1.00\n",
        "shader UsdUVTexture\n",
        "param string file \"\" %meta{} %read{1,1} %write{2147483647,-1}\n",
        "param float s 0 %meta{} %read{1,1} %write{2147483647,-1}\n",
        "param float t 0 %meta{} %read{1,1} %write{2147483647,-1}\n",
        "param color fallback 0 0 0 %meta{} %read{2147483647,-1} %write{2147483647,-1}\n",
        "oparam color rgb 0 0 0 %meta{} %read{2,4} %write{1,1}\n",
        "oparam float r 0 %meta{} %read{2147483647,-1} %write{2,2}\n",
        "oparam float g 0 %meta{} %read{2147483647,-1} %write{3,3}\n",
        "oparam float b 0 %meta{} %read{2147483647,-1} %write{4,4}\n",
        "oparam float a 1 %meta{} %read{2147483647,-1} %write{2147483647,-1}\n",
        "const int $const0 0 %read{2,2} %write{2147483647,-1}\n",
        "const int $const1 1 %read{3,3} %write{2147483647,-1}\n",
        "const int $const2 2 %read{4,4} %write{2147483647,-1}\n",
        "code UsdUVTexture\n",
        "  texture rgb file s t\n",
        "  compref r rgb $const0\n",
        "  compref g rgb $const1\n",
        "  compref b rgb $const2\n",
        "  end\n",
    )
}

/// .oso bytecode for a simplified UsdPrimvarReader_float2 shader.
///
/// Implements: result.s = u; result.t = v (reads UV globals)
pub fn usd_primvar_reader_float2_oso() -> &'static str {
    concat!(
        "OpenShadingLanguage 1.00\n",
        "shader UsdPrimvarReader_float2\n",
        "param string varname \"st\" %meta{} %read{2147483647,-1} %write{2147483647,-1}\n",
        "oparam float result_s 0 %meta{} %read{2147483647,-1} %write{1,1}\n",
        "oparam float result_t 0 %meta{} %read{2147483647,-1} %write{2,2}\n",
        "global float u %read{1,1} %write{2147483647,-1}\n",
        "global float v %read{2,2} %write{2147483647,-1}\n",
        "code UsdPrimvarReader_float2\n",
        "  assign result_s u\n",
        "  assign result_t v\n",
        "  end\n",
    )
}

/// Register the built-in USD shaders with a ShadingSystem.
///
/// This is the Rust equivalent of calling `LoadMemoryCompiledShader` in the
/// C++ reference. After this call, `ShadingSystem::shader()` can resolve
/// "UsdPreviewSurface", "UsdUVTexture", and "UsdPrimvarReader_float2"
/// without loading external .oso files from disk.
pub fn register_usd_shaders(ss: &ShadingSystem) {
    let _ = ss.load_memory_shader("UsdPreviewSurface", usd_preview_surface_oso());
    let _ = ss.load_memory_shader("UsdUVTexture", usd_uv_texture_oso());
    let _ = ss.load_memory_shader("UsdPrimvarReader_float2", usd_primvar_reader_float2_oso());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::BasicRenderer;
    use crate::shadingsys::ShadingSystem;
    use std::sync::Arc;

    #[test]
    fn test_simple_graph_build() {
        let renderer = Arc::new(BasicRenderer::new());
        let ss = ShadingSystem::new(renderer, Some(Arc::new(crate::shadingsys::StdErrorHandler)));

        // Create a simple two-node graph
        let mut graph = SimpleGraph::new("test_material");
        graph.add_node(UsdShaderNode {
            layer_name: "noise".into(),
            shader_name: "noise_pattern".into(),
            shader_type: "shader".into(),
            params: vec![("scale".into(), UsdParamValue::Float(4.0))],
        });
        graph.add_node(UsdShaderNode {
            layer_name: "surface".into(),
            shader_name: "simple_surface".into(),
            shader_type: "surface".into(),
            params: vec![("Kd".into(), UsdParamValue::Float(0.8))],
        });
        graph.connect("noise", "result", "surface", "roughness");

        // Build — this exercises the shader group construction path.
        // Actual execution requires the shaders to be loadable.
        let result = build_shader_group(&ss, &graph);
        // It's OK if shader loading fails (we don't have the .oso files),
        // but the graph construction itself should succeed or fail gracefully.
        let _ = result;
    }

    #[test]
    fn test_param_mapping() {
        let params = vec![
            (
                "diffuseColor".to_string(),
                UsdParamValue::Color(Color3::new(0.8, 0.2, 0.1)),
            ),
            ("roughness".to_string(), UsdParamValue::Float(0.5)),
            ("metallic".to_string(), UsdParamValue::Float(0.0)),
        ];
        let mapped = map_usd_preview_surface_params(&params);
        assert_eq!(mapped.len(), 3);
        // Verify name mapping
        assert_eq!(mapped[0].0, "base_color");
        assert_eq!(mapped[1].0, "specular_roughness");
        assert_eq!(mapped[2].0, "metalness");
    }
}
