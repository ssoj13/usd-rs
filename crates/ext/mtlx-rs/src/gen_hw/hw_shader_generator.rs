//! HwShaderGenerator -- base for HW (rasterization) shader generators (by ref MaterialX GenHw).

use std::collections::HashMap;

use crate::gen_hw::hw_constants::{ident, lighting, token};
use crate::gen_shader::{
    ShaderGenerator, ShaderGraph, ShaderNode, ShaderNodeClassification, ShaderStage, TypeDesc,
    VariableBlock,
};

/// Closure context types for HW rendering (C++ HwShaderGenerator::ClosureContextType).
/// Order must match libraries/pbrlib/genglsl/lib/mx_closure_type.glsl.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[repr(u32)]
pub enum ClosureContextType {
    #[default]
    Default = 0,
    Reflection = 1,
    Transmission = 2,
    Indirect = 3,
    Emission = 4,
    Lighting = 5,
    Closure = 6,
}

/// Build the default token substitution map (C++ HwShaderGenerator constructor).
/// Maps token strings (e.g. "$inPosition") to default identifier names (e.g. "i_position").
pub fn build_token_substitutions() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    m.insert(token::T_IN_POSITION, ident::IN_POSITION);
    m.insert(token::T_IN_NORMAL, ident::IN_NORMAL);
    m.insert(token::T_IN_TANGENT, ident::IN_TANGENT);
    m.insert(token::T_IN_BITANGENT, ident::IN_BITANGENT);
    m.insert(token::T_IN_TEXCOORD, ident::IN_TEXCOORD);
    m.insert(token::T_IN_GEOMPROP, ident::IN_GEOMPROP);
    m.insert(token::T_IN_COLOR, ident::IN_COLOR);
    m.insert(token::T_POSITION_WORLD, ident::POSITION_WORLD);
    m.insert(token::T_NORMAL_WORLD, ident::NORMAL_WORLD);
    m.insert(token::T_TANGENT_WORLD, ident::TANGENT_WORLD);
    m.insert(token::T_BITANGENT_WORLD, ident::BITANGENT_WORLD);
    m.insert(token::T_POSITION_OBJECT, ident::POSITION_OBJECT);
    m.insert(token::T_NORMAL_OBJECT, ident::NORMAL_OBJECT);
    m.insert(token::T_TANGENT_OBJECT, ident::TANGENT_OBJECT);
    m.insert(token::T_BITANGENT_OBJECT, ident::BITANGENT_OBJECT);
    m.insert(token::T_TEXCOORD, ident::TEXCOORD);
    m.insert(token::T_COLOR, ident::COLOR);
    m.insert(token::T_WORLD_MATRIX, ident::WORLD_MATRIX);
    m.insert(token::T_WORLD_INVERSE_MATRIX, ident::WORLD_INVERSE_MATRIX);
    m.insert(
        token::T_WORLD_TRANSPOSE_MATRIX,
        ident::WORLD_TRANSPOSE_MATRIX,
    );
    m.insert(
        token::T_WORLD_INVERSE_TRANSPOSE_MATRIX,
        ident::WORLD_INVERSE_TRANSPOSE_MATRIX,
    );
    m.insert(token::T_VIEW_MATRIX, ident::VIEW_MATRIX);
    m.insert(token::T_VIEW_INVERSE_MATRIX, ident::VIEW_INVERSE_MATRIX);
    m.insert(token::T_VIEW_TRANSPOSE_MATRIX, ident::VIEW_TRANSPOSE_MATRIX);
    m.insert(
        token::T_VIEW_INVERSE_TRANSPOSE_MATRIX,
        ident::VIEW_INVERSE_TRANSPOSE_MATRIX,
    );
    m.insert(token::T_PROJ_MATRIX, ident::PROJ_MATRIX);
    m.insert(token::T_PROJ_INVERSE_MATRIX, ident::PROJ_INVERSE_MATRIX);
    m.insert(token::T_PROJ_TRANSPOSE_MATRIX, ident::PROJ_TRANSPOSE_MATRIX);
    m.insert(
        token::T_PROJ_INVERSE_TRANSPOSE_MATRIX,
        ident::PROJ_INVERSE_TRANSPOSE_MATRIX,
    );
    m.insert(token::T_WORLD_VIEW_MATRIX, ident::WORLD_VIEW_MATRIX);
    m.insert(
        token::T_VIEW_PROJECTION_MATRIX,
        ident::VIEW_PROJECTION_MATRIX,
    );
    m.insert(
        token::T_WORLD_VIEW_PROJECTION_MATRIX,
        ident::WORLD_VIEW_PROJECTION_MATRIX,
    );
    m.insert(token::T_VIEW_POSITION, ident::VIEW_POSITION);
    m.insert(token::T_VIEW_DIRECTION, ident::VIEW_DIRECTION);
    m.insert(token::T_FRAME, ident::FRAME);
    m.insert(token::T_TIME, ident::TIME);
    m.insert(token::T_GEOMPROP, ident::GEOMPROP);
    m.insert(token::T_ALPHA_THRESHOLD, ident::ALPHA_THRESHOLD);
    m.insert(
        token::T_NUM_ACTIVE_LIGHT_SOURCES,
        ident::NUM_ACTIVE_LIGHT_SOURCES,
    );
    m.insert(token::T_ENV_MATRIX, ident::ENV_MATRIX);
    m.insert(token::T_ENV_RADIANCE, ident::ENV_RADIANCE);
    m.insert(
        token::T_ENV_RADIANCE_SAMPLER2D,
        ident::ENV_RADIANCE_SAMPLER2D,
    );
    m.insert(token::T_ENV_RADIANCE_MIPS, ident::ENV_RADIANCE_MIPS);
    m.insert(token::T_ENV_RADIANCE_SAMPLES, ident::ENV_RADIANCE_SAMPLES);
    m.insert(token::T_ENV_IRRADIANCE, ident::ENV_IRRADIANCE);
    m.insert(
        token::T_ENV_IRRADIANCE_SAMPLER2D,
        ident::ENV_IRRADIANCE_SAMPLER2D,
    );
    m.insert(token::T_ENV_LIGHT_INTENSITY, ident::ENV_LIGHT_INTENSITY);
    m.insert(token::T_REFRACTION_TWO_SIDED, ident::REFRACTION_TWO_SIDED);
    m.insert(token::T_ALBEDO_TABLE, ident::ALBEDO_TABLE);
    m.insert(token::T_ALBEDO_TABLE_SIZE, ident::ALBEDO_TABLE_SIZE);
    m.insert(token::T_SHADOW_MAP, ident::SHADOW_MAP);
    m.insert(token::T_SHADOW_MATRIX, ident::SHADOW_MATRIX);
    m.insert(token::T_AMB_OCC_MAP, ident::AMB_OCC_MAP);
    m.insert(token::T_AMB_OCC_GAIN, ident::AMB_OCC_GAIN);
    m.insert(token::T_VERTEX_DATA_INSTANCE, ident::VERTEX_DATA_INSTANCE);
    m.insert(token::T_LIGHT_DATA_INSTANCE, ident::LIGHT_DATA_INSTANCE);
    m.insert(token::T_ENV_PREFILTER_MIP, ident::ENV_PREFILTER_MIP);
    m.insert(token::T_TEX_SAMPLER_SAMPLER2D, ident::TEX_SAMPLER_SAMPLER2D);
    m.insert(token::T_TEX_SAMPLER_SIGNATURE, ident::TEX_SAMPLER_SIGNATURE);
    m.insert(
        token::T_CLOSURE_DATA_CONSTRUCTOR,
        lighting::CLOSURE_DATA_CONSTRUCTOR,
    );
    m
}

/// Base for hardware shader generators (GLSL, MSL, etc.)
pub trait HwShaderGenerator: ShaderGenerator {
    /// Add lighting uniforms to a stage.
    /// C++: adds u_numActiveLightSources if hwMaxActiveLightSources > 0.
    fn add_stage_lighting_uniforms(&self, _context: &dyn std::any::Any, _stage: &mut ShaderStage) {}

    /// Return true if graph requires lighting (C++ HwShaderGenerator::requiresLighting).
    ///
    /// C++ logic: isBsdf || (isShader && isSurface && !isUnlit)
    fn requires_lighting(&self, graph: &ShaderGraph) -> bool {
        let is_bsdf = graph.has_classification(ShaderNodeClassification::BSDF);
        let is_lit_surface = graph.has_classification(ShaderNodeClassification::SHADER)
            && graph.has_classification(ShaderNodeClassification::SURFACE)
            && !graph.has_classification(ShaderNodeClassification::UNLIT);
        is_bsdf || is_lit_surface
    }

    /// Get vertex data variable prefix (e.g. "vd." for GLSL).
    fn get_vertex_data_prefix(&self, _vertex_data: &VariableBlock) -> String {
        "vd.".to_string()
    }

    /// Emit ClosureData argument in a function call if node is BSDF/EDF/VDF.
    /// C++: emits "closureData, " before other args.
    fn emit_closure_data_arg(&self, node: &ShaderNode, stage: &mut ShaderStage) {
        if self.node_needs_closure_data(node) {
            stage.emit_string(&format!("{}, ", lighting::CLOSURE_DATA_ARG));
        }
    }

    /// Emit ClosureData parameter in a function definition if node is BSDF/EDF/VDF.
    /// C++: emits "ClosureData closureData, " before other params.
    fn emit_closure_data_parameter(&self, node: &ShaderNode, stage: &mut ShaderStage) {
        if self.node_needs_closure_data(node) {
            stage.emit_string(&format!(
                "{} {}, ",
                lighting::CLOSURE_DATA_TYPE,
                lighting::CLOSURE_DATA_ARG
            ));
        }
    }

    /// Promote a variable to vec4 based on its type (C++ HwShaderGenerator::toVec4).
    ///
    /// - float3/color3/vector3 -> vec4(var, 1.0)
    /// - float2/vector2 -> vec4(var, 0.0, 1.0)
    /// - float/integer/boolean -> vec4(var, var, var, 1.0)
    /// - BSDF/EDF -> vec4(var, 1.0)
    /// - anything else -> vec4(0.0, 0.0, 0.0, 1.0) (black)
    fn to_vec4(&self, type_desc: &TypeDesc, variable: &str) -> String {
        let vec4 = "vec4";
        let tn = type_desc.get_name();
        if type_desc.is_float3() {
            format!("{}({}, 1.0)", vec4, variable)
        } else if type_desc.is_float2() {
            format!("{}({}, 0.0, 1.0)", vec4, variable)
        } else if tn == "float" || tn == "integer" || tn == "boolean" {
            format!("{}({v}, {v}, {v}, 1.0)", vec4, v = variable)
        } else if tn == "BSDF" || tn == "EDF" {
            format!("{}({}, 1.0)", vec4, variable)
        } else {
            format!("{}(0.0, 0.0, 0.0, 1.0)", vec4)
        }
    }

    /// Return the string used for the light data type discriminator variable
    /// (C++ getLightDataTypevarString). Default: "type".
    fn get_light_data_type_var_string(&self) -> &str {
        "type"
    }
}

/// Bind a light shader NodeDef to a light type ID in the context's HwLightShaders.
/// C++: HwShaderGenerator::bindLightShader (static).
///
/// Creates a ShaderNode from the NodeDef, and if it has a graph implementation,
/// prepends "light." to all graph input socket variables so they access the light struct.
pub fn bind_light_shader(
    light_shaders: &mut crate::gen_hw::HwLightShaders,
    node_def_name: &str,
    light_type_id: u32,
    shader_node: Box<ShaderNode>,
) -> Result<(), String> {
    if light_shaders.get(light_type_id).is_some() {
        return Err(format!(
            "Error binding light shader. Light type id '{}' has already been bound",
            light_type_id
        ));
    }
    light_shaders.bind(light_type_id, shader_node);
    let _ = node_def_name;
    Ok(())
}

/// Unbind a light shader for the given type ID.
/// C++: HwShaderGenerator::unbindLightShader (static).
pub fn unbind_light_shader(light_shaders: &mut crate::gen_hw::HwLightShaders, light_type_id: u32) {
    light_shaders.unbind(light_type_id);
}

/// Unbind all light shaders.
/// C++: HwShaderGenerator::unbindLightShaders (static).
pub fn unbind_light_shaders(light_shaders: &mut crate::gen_hw::HwLightShaders) {
    light_shaders.clear();
}

#[cfg(test)]
mod tests {
    use super::HwShaderGenerator;
    use crate::gen_shader::{
        ShaderGenerator, ShaderGraph, ShaderNode, ShaderNodeClassification, TypeSystem,
        VariableBlock,
    };

    /// Minimal concrete HwShaderGenerator for testing trait methods
    struct TestHwGen {
        type_system: TypeSystem,
    }
    impl TestHwGen {
        fn new() -> Self {
            Self {
                type_system: TypeSystem::new(),
            }
        }
    }
    impl ShaderGenerator for TestHwGen {
        fn get_type_system(&self) -> &TypeSystem {
            &self.type_system
        }
        fn target(&self) -> &str {
            "test"
        }
        /// Override: HW generators return true for closure nodes (BSDF/EDF/VDF).
        fn node_needs_closure_data(&self, node: &ShaderNode) -> bool {
            node.has_classification(ShaderNodeClassification::BSDF)
                || node.has_classification(ShaderNodeClassification::EDF)
                || node.has_classification(ShaderNodeClassification::VDF)
        }
    }
    impl HwShaderGenerator for TestHwGen {}

    // -- node_needs_closure_data tests --

    #[test]
    fn node_needs_closure_data_bsdf() {
        let hw = TestHwGen::new();
        let mut node = ShaderNode::new("test_bsdf");
        node.add_classification(ShaderNodeClassification::BSDF);
        assert!(hw.node_needs_closure_data(&node));
    }

    #[test]
    fn node_needs_closure_data_edf() {
        let hw = TestHwGen::new();
        let mut node = ShaderNode::new("test_edf");
        node.add_classification(ShaderNodeClassification::EDF);
        assert!(hw.node_needs_closure_data(&node));
    }

    #[test]
    fn node_needs_closure_data_vdf() {
        let hw = TestHwGen::new();
        let mut node = ShaderNode::new("test_vdf");
        node.add_classification(ShaderNodeClassification::VDF);
        assert!(hw.node_needs_closure_data(&node));
    }

    #[test]
    fn node_needs_closure_data_texture_false() {
        let hw = TestHwGen::new();
        let mut node = ShaderNode::new("test_tex");
        node.add_classification(ShaderNodeClassification::TEXTURE);
        assert!(!hw.node_needs_closure_data(&node));
    }

    #[test]
    fn node_needs_closure_data_empty_false() {
        let hw = TestHwGen::new();
        let node = ShaderNode::new("empty");
        assert!(!hw.node_needs_closure_data(&node));
    }

    // -- requires_lighting tests --

    #[test]
    fn requires_lighting_bsdf_graph() {
        let hw = TestHwGen::new();
        let mut graph = ShaderGraph::new("bsdf_graph");
        graph
            .node
            .add_classification(ShaderNodeClassification::BSDF);
        assert!(hw.requires_lighting(&graph));
    }

    #[test]
    fn requires_lighting_lit_surface_shader() {
        let hw = TestHwGen::new();
        let mut graph = ShaderGraph::new("surface_graph");
        graph
            .node
            .add_classification(ShaderNodeClassification::SHADER);
        graph
            .node
            .add_classification(ShaderNodeClassification::SURFACE);
        assert!(hw.requires_lighting(&graph));
    }

    #[test]
    fn requires_lighting_unlit_surface_false() {
        let hw = TestHwGen::new();
        let mut graph = ShaderGraph::new("unlit_graph");
        graph
            .node
            .add_classification(ShaderNodeClassification::SHADER);
        graph
            .node
            .add_classification(ShaderNodeClassification::SURFACE);
        graph
            .node
            .add_classification(ShaderNodeClassification::UNLIT);
        assert!(!hw.requires_lighting(&graph));
    }

    #[test]
    fn requires_lighting_shader_only_false() {
        let hw = TestHwGen::new();
        let mut graph = ShaderGraph::new("shader_graph");
        graph
            .node
            .add_classification(ShaderNodeClassification::SHADER);
        // SHADER without SURFACE should NOT require lighting (C++ behavior)
        assert!(!hw.requires_lighting(&graph));
    }

    #[test]
    fn requires_lighting_texture_graph_false() {
        let hw = TestHwGen::new();
        let mut graph = ShaderGraph::new("tex_graph");
        graph
            .node
            .add_classification(ShaderNodeClassification::TEXTURE);
        assert!(!hw.requires_lighting(&graph));
    }

    #[test]
    fn requires_lighting_empty_graph_false() {
        let hw = TestHwGen::new();
        let graph = ShaderGraph::new("empty");
        assert!(!hw.requires_lighting(&graph));
    }

    // -- get_vertex_data_prefix test --

    #[test]
    fn vertex_data_prefix_default() {
        let hw = TestHwGen::new();
        let vb = VariableBlock::new("VertexData", "");
        assert_eq!(hw.get_vertex_data_prefix(&vb), "vd.");
    }

    // -- ClosureContextType tests --

    #[test]
    fn closure_context_type_default() {
        let t = super::ClosureContextType::default();
        assert_eq!(t, super::ClosureContextType::Default);
        assert_eq!(t as u32, 0);
    }

    #[test]
    fn closure_context_type_values() {
        use super::ClosureContextType::*;
        assert_eq!(Reflection as u32, 1);
        assert_eq!(Transmission as u32, 2);
        assert_eq!(Indirect as u32, 3);
        assert_eq!(Emission as u32, 4);
        assert_eq!(Lighting as u32, 5);
        assert_eq!(Closure as u32, 6);
    }

    // -- token substitution tests --

    #[test]
    fn token_substitutions_has_all_entries() {
        let subs = super::build_token_substitutions();
        assert!(
            subs.len() >= 50,
            "Should have 50+ token substitutions, got {}",
            subs.len()
        );
        assert_eq!(subs.get("$inPosition"), Some(&"i_position"));
        assert_eq!(subs.get("$worldMatrix"), Some(&"u_worldMatrix"));
        assert_eq!(subs.get("$vd"), Some(&"vd"));
        assert_eq!(subs.get("$lightData"), Some(&"u_lightData"));
    }

    // -- bind/unbind light shader tests --

    #[test]
    fn bind_unbind_light_shaders() {
        let mut ls = crate::gen_hw::HwLightShaders::new();
        let node = Box::new(ShaderNode::new("point_light"));
        assert!(super::bind_light_shader(&mut ls, "ND_point_light", 1, node).is_ok());
        assert!(ls.get(1).is_some());

        // Duplicate bind should fail
        let node2 = Box::new(ShaderNode::new("point_light2"));
        assert!(super::bind_light_shader(&mut ls, "ND_point_light", 1, node2).is_err());

        super::unbind_light_shader(&mut ls, 1);
        assert!(ls.get(1).is_none());

        let node3 = Box::new(ShaderNode::new("spot_light"));
        super::bind_light_shader(&mut ls, "ND_spot_light", 2, node3).unwrap();
        super::unbind_light_shaders(&mut ls);
        assert!(ls.is_empty());
    }
}
