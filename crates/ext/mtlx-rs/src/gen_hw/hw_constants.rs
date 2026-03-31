//! HwConstants — token names and default identifiers (по рефу MaterialXGenHw HwConstants.h/cpp).

/// Token names (used in source before substitution)
pub mod token {
    pub const T_IN_POSITION: &str = "$inPosition";
    pub const T_IN_NORMAL: &str = "$inNormal";
    pub const T_IN_TANGENT: &str = "$inTangent";
    pub const T_IN_BITANGENT: &str = "$inBitangent";
    pub const T_IN_TEXCOORD: &str = "$inTexcoord";
    pub const T_IN_GEOMPROP: &str = "$inGeomprop";
    pub const T_IN_COLOR: &str = "$inColor";
    pub const T_POSITION_WORLD: &str = "$positionWorld";
    pub const T_NORMAL_WORLD: &str = "$normalWorld";
    pub const T_TANGENT_WORLD: &str = "$tangentWorld";
    pub const T_BITANGENT_WORLD: &str = "$bitangentWorld";
    pub const T_POSITION_OBJECT: &str = "$positionObject";
    pub const T_NORMAL_OBJECT: &str = "$normalObject";
    pub const T_TANGENT_OBJECT: &str = "$tangentObject";
    pub const T_BITANGENT_OBJECT: &str = "$bitangentObject";
    pub const T_TEXCOORD: &str = "$texcoord";
    pub const T_COLOR: &str = "$color";
    pub const T_WORLD_MATRIX: &str = "$worldMatrix";
    pub const T_WORLD_INVERSE_MATRIX: &str = "$worldInverseMatrix";
    pub const T_WORLD_TRANSPOSE_MATRIX: &str = "$worldTransposeMatrix";
    pub const T_WORLD_INVERSE_TRANSPOSE_MATRIX: &str = "$worldInverseTransposeMatrix";
    pub const T_VIEW_MATRIX: &str = "$viewMatrix";
    pub const T_VIEW_INVERSE_MATRIX: &str = "$viewInverseMatrix";
    pub const T_VIEW_TRANSPOSE_MATRIX: &str = "$viewTransposeMatrix";
    pub const T_VIEW_INVERSE_TRANSPOSE_MATRIX: &str = "$viewInverseTransposeMatrix";
    pub const T_PROJ_MATRIX: &str = "$projectionMatrix";
    pub const T_PROJ_INVERSE_MATRIX: &str = "$projectionInverseMatrix";
    pub const T_PROJ_TRANSPOSE_MATRIX: &str = "$projectionTransposeMatrix";
    pub const T_PROJ_INVERSE_TRANSPOSE_MATRIX: &str = "$projectionInverseTransposeMatrix";
    pub const T_WORLD_VIEW_MATRIX: &str = "$worldViewMatrix";
    pub const T_VIEW_PROJECTION_MATRIX: &str = "$viewProjectionMatrix";
    pub const T_WORLD_VIEW_PROJECTION_MATRIX: &str = "$worldViewProjectionMatrix";
    pub const T_VIEW_POSITION: &str = "$viewPosition";
    pub const T_VIEW_DIRECTION: &str = "$viewDirection";
    pub const T_FRAME: &str = "$frame";
    pub const T_TIME: &str = "$time";
    pub const T_GEOMPROP: &str = "$geomprop";
    pub const T_ALPHA_THRESHOLD: &str = "$alphaThreshold";
    pub const T_NUM_ACTIVE_LIGHT_SOURCES: &str = "$numActiveLightSources";
    pub const T_ENV_MATRIX: &str = "$envMatrix";
    pub const T_ENV_RADIANCE: &str = "$envRadiance";
    pub const T_ENV_RADIANCE_SAMPLER2D: &str = "$envRadianceSampler2D";
    pub const T_ENV_RADIANCE_MIPS: &str = "$envRadianceMips";
    pub const T_ENV_RADIANCE_SAMPLES: &str = "$envRadianceSamples";
    pub const T_ENV_IRRADIANCE: &str = "$envIrradiance";
    pub const T_ENV_IRRADIANCE_SAMPLER2D: &str = "$envIrradianceSampler2D";
    pub const T_TEX_SAMPLER_SAMPLER2D: &str = "$texSamplerSampler2D";
    pub const T_TEX_SAMPLER_SIGNATURE: &str = "$texSamplerSignature";
    pub const T_CLOSURE_DATA_CONSTRUCTOR: &str = "$closureDataConstructor";
    pub const T_ENV_LIGHT_INTENSITY: &str = "$envLightIntensity";
    pub const T_ENV_PREFILTER_MIP: &str = "$envPrefilterMip";
    pub const T_REFRACTION_TWO_SIDED: &str = "$refractionTwoSided";
    pub const T_ALBEDO_TABLE: &str = "$albedoTable";
    pub const T_ALBEDO_TABLE_SIZE: &str = "$albedoTableSize";
    pub const T_AMB_OCC_MAP: &str = "$ambOccMap";
    pub const T_AMB_OCC_GAIN: &str = "$ambOccGain";
    pub const T_SHADOW_MAP: &str = "$shadowMap";
    pub const T_SHADOW_MATRIX: &str = "$shadowMatrix";
    pub const T_VERTEX_DATA_INSTANCE: &str = "$vd";
    pub const T_LIGHT_DATA_INSTANCE: &str = "$lightData";
}

/// Default identifier names (after token substitution)
pub mod ident {
    pub const IN_POSITION: &str = "i_position";
    pub const IN_NORMAL: &str = "i_normal";
    pub const IN_TANGENT: &str = "i_tangent";
    pub const IN_BITANGENT: &str = "i_bitangent";
    pub const IN_TEXCOORD: &str = "i_texcoord";
    pub const IN_GEOMPROP: &str = "i_geomprop";
    pub const IN_COLOR: &str = "i_color";
    pub const POSITION_WORLD: &str = "positionWorld";
    pub const NORMAL_WORLD: &str = "normalWorld";
    pub const TANGENT_WORLD: &str = "tangentWorld";
    pub const BITANGENT_WORLD: &str = "bitangentWorld";
    pub const POSITION_OBJECT: &str = "positionObject";
    pub const NORMAL_OBJECT: &str = "normalObject";
    pub const TANGENT_OBJECT: &str = "tangentObject";
    pub const BITANGENT_OBJECT: &str = "bitangentObject";
    pub const TEXCOORD: &str = "texcoord";
    pub const COLOR: &str = "color";
    pub const WORLD_MATRIX: &str = "u_worldMatrix";
    pub const WORLD_INVERSE_MATRIX: &str = "u_worldInverseMatrix";
    pub const WORLD_TRANSPOSE_MATRIX: &str = "u_worldTransposeMatrix";
    pub const WORLD_INVERSE_TRANSPOSE_MATRIX: &str = "u_worldInverseTransposeMatrix";
    pub const VIEW_MATRIX: &str = "u_viewMatrix";
    pub const VIEW_INVERSE_MATRIX: &str = "u_viewInverseMatrix";
    pub const VIEW_TRANSPOSE_MATRIX: &str = "u_viewTransposeMatrix";
    pub const VIEW_INVERSE_TRANSPOSE_MATRIX: &str = "u_viewInverseTransposeMatrix";
    pub const PROJ_MATRIX: &str = "u_projectionMatrix";
    pub const PROJ_INVERSE_MATRIX: &str = "u_projectionInverseMatrix";
    pub const PROJ_TRANSPOSE_MATRIX: &str = "u_projectionTransposeMatrix";
    pub const PROJ_INVERSE_TRANSPOSE_MATRIX: &str = "u_projectionInverseTransposeMatrix";
    pub const WORLD_VIEW_MATRIX: &str = "u_worldViewMatrix";
    pub const VIEW_PROJECTION_MATRIX: &str = "u_viewProjectionMatrix";
    pub const WORLD_VIEW_PROJECTION_MATRIX: &str = "u_worldViewProjectionMatrix";
    pub const VIEW_POSITION: &str = "u_viewPosition";
    pub const VIEW_DIRECTION: &str = "u_viewDirection";
    pub const FRAME: &str = "u_frame";
    pub const TIME: &str = "u_time";
    pub const GEOMPROP: &str = "u_geomprop";
    pub const ALPHA_THRESHOLD: &str = "u_alphaThreshold";
    pub const NUM_ACTIVE_LIGHT_SOURCES: &str = "u_numActiveLightSources";
    pub const ENV_MATRIX: &str = "u_envMatrix";
    pub const ENV_RADIANCE: &str = "u_envRadiance";
    pub const ENV_RADIANCE_SPLIT: &str = "u_envRadiance_texture, u_envRadiance_sampler";
    pub const ENV_RADIANCE_SAMPLER2D: &str = "u_envRadiance";
    pub const ENV_RADIANCE_SAMPLER2D_SPLIT: &str =
        "sampler2D(u_envRadiance_texture, u_envRadiance_sampler)";
    pub const ENV_RADIANCE_MIPS: &str = "u_envRadianceMips";
    pub const ENV_RADIANCE_SAMPLES: &str = "u_envRadianceSamples";
    pub const ENV_IRRADIANCE: &str = "u_envIrradiance";
    pub const ENV_IRRADIANCE_SPLIT: &str = "u_envIrradiance_texture, u_envIrradiance_sampler";
    pub const ENV_IRRADIANCE_SAMPLER2D: &str = "u_envIrradiance";
    pub const ENV_IRRADIANCE_SAMPLER2D_SPLIT: &str =
        "sampler2D(u_envIrradiance_texture, u_envIrradiance_sampler)";
    pub const ENV_LIGHT_INTENSITY: &str = "u_envLightIntensity";
    pub const ENV_PREFILTER_MIP: &str = "u_envPrefilterMip";
    pub const REFRACTION_TWO_SIDED: &str = "u_refractionTwoSided";
    pub const ALBEDO_TABLE: &str = "u_albedoTable";
    pub const ALBEDO_TABLE_SIZE: &str = "u_albedoTableSize";
    pub const AMB_OCC_MAP: &str = "u_ambOccMap";
    pub const AMB_OCC_GAIN: &str = "u_ambOccGain";
    pub const SHADOW_MAP: &str = "u_shadowMap";
    pub const SHADOW_MATRIX: &str = "u_shadowMatrix";
    pub const VERTEX_DATA_INSTANCE: &str = "vd";
    pub const LIGHT_DATA_INSTANCE: &str = "u_lightData";
    pub const LIGHT_DATA_MAX_LIGHT_SOURCES: &str = "MAX_LIGHT_SOURCES";
    pub const TEX_SAMPLER_SAMPLER2D: &str = "tex_sampler";
    pub const TEX_SAMPLER_SIGNATURE: &str = "sampler2D tex_sampler";
    /// WGSL split: texture + sampler separate
    pub const TEX_SAMPLER_SAMPLER2D_SPLIT: &str = "sampler2D(tex_texture, tex_sampler)";
    pub const TEX_SAMPLER_SIGNATURE_SPLIT: &str = "texture2D tex_texture, sampler tex_sampler";
}

/// Variable block names
pub mod block {
    pub const VERTEX_INPUTS: &str = "VertexInputs";
    pub const VERTEX_DATA: &str = "VertexData";
    pub const PRIVATE_UNIFORMS: &str = "PrivateUniforms";
    pub const PUBLIC_UNIFORMS: &str = "PublicUniforms";
    pub const LIGHT_DATA: &str = "LightData";
    pub const PIXEL_OUTPUTS: &str = "PixelOutputs";
}

/// Variable names for lighting parameters
pub mod lighting {
    pub const DIR_N: &str = "N";
    pub const DIR_L: &str = "L";
    pub const DIR_V: &str = "V";
    pub const WORLD_POSITION: &str = "P";
    pub const OCCLUSION: &str = "occlusion";
    /// ClosureData struct type name — used by HW shader emit for closure data passing.
    pub const CLOSURE_DATA_TYPE: &str = "ClosureData";
    /// ClosureData argument name in function signatures — used by HW shader emit.
    pub const CLOSURE_DATA_ARG: &str = "closureData";
    /// ClosureData constructor call — used in token substitution.
    pub const CLOSURE_DATA_CONSTRUCTOR: &str = "ClosureData(closureType, L, V, N, P, occlusion)";
}

/// Attribute names
pub mod attr {
    #[allow(dead_code)] // Used by HW emit code to mark transparent materials
    pub const ATTR_TRANSPARENT: &str = "transparent";
}

/// Constant typed values used in hardware shader generation.
#[allow(dead_code)] // MaterialX API constants -- will be used by HW emit code
pub mod constant_values {
    /// Vec2 zero constant
    pub const VEC2_ZERO: [f32; 2] = [0.0, 0.0];
    /// Vec2 one constant
    pub const VEC2_ONE: [f32; 2] = [1.0, 1.0];
    /// Vec3 zero constant
    pub const VEC3_ZERO: [f32; 3] = [0.0, 0.0, 0.0];
    /// Vec3 one constant
    pub const VEC3_ONE: [f32; 3] = [1.0, 1.0, 1.0];
}

/// User data keys for GenContext
pub mod user_data {
    #[allow(dead_code)] // Used by HW emit code to pass light shader data via GenContext
    pub const USER_DATA_LIGHT_SHADERS: &str = "udls";
    #[allow(dead_code)] // Used by HW emit code to pass binding context via GenContext
    pub const USER_DATA_BINDING_CONTEXT: &str = "udbinding";
}

/// Geometry space enum values (matches C++ HW_ENUM_TYPE_OBJECT_SPACE / WORLD_SPACE).
pub mod space {
    /// Input attribute name for the space enum on geometry nodes.
    pub const SPACE_ATTR: &str = "space";
    /// Model space enum value.
    pub const MODEL_SPACE: i32 = 0;
    /// Object space enum value.
    pub const OBJECT_SPACE: i32 = 1;
    /// World space enum value.
    pub const WORLD_SPACE: i32 = 2;
}

/// Extract the integer space value from a ShaderNode's "space" input.
/// Returns `default` if the input is missing or not an integer.
pub fn get_node_space(node: &crate::gen_shader::ShaderNode, default: i32) -> i32 {
    node.get_input(space::SPACE_ATTR)
        .and_then(|i| i.port.get_value())
        .and_then(|v| {
            if let crate::core::Value::Integer(n) = v {
                Some(*n)
            } else {
                None
            }
        })
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    #[test]
    fn constant_values_vec2() {
        assert_eq!(super::constant_values::VEC2_ZERO, [0.0, 0.0]);
        assert_eq!(super::constant_values::VEC2_ONE, [1.0, 1.0]);
    }

    #[test]
    fn constant_values_vec3() {
        assert_eq!(super::constant_values::VEC3_ZERO, [0.0, 0.0, 0.0]);
        assert_eq!(super::constant_values::VEC3_ONE, [1.0, 1.0, 1.0]);
    }

    #[test]
    fn block_names() {
        assert_eq!(super::block::VERTEX_INPUTS, "VertexInputs");
        assert_eq!(super::block::VERTEX_DATA, "VertexData");
        assert_eq!(super::block::PRIVATE_UNIFORMS, "PrivateUniforms");
        assert_eq!(super::block::PUBLIC_UNIFORMS, "PublicUniforms");
        assert_eq!(super::block::LIGHT_DATA, "LightData");
        assert_eq!(super::block::PIXEL_OUTPUTS, "PixelOutputs");
    }

    #[test]
    fn lighting_direction_names() {
        assert_eq!(super::lighting::DIR_N, "N");
        assert_eq!(super::lighting::DIR_L, "L");
        assert_eq!(super::lighting::DIR_V, "V");
        assert_eq!(super::lighting::WORLD_POSITION, "P");
        assert_eq!(super::lighting::OCCLUSION, "occlusion");
    }

    #[test]
    fn space_enum_values() {
        assert_eq!(super::space::OBJECT_SPACE, 1);
        assert_eq!(super::space::WORLD_SPACE, 2);
        assert_eq!(super::space::SPACE_ATTR, "space");
    }

    #[test]
    fn get_node_space_returns_default_for_empty_node() {
        let node = crate::gen_shader::ShaderNode::new("test");
        assert_eq!(super::get_node_space(&node, 42), 42);
    }

    #[test]
    fn get_node_space_returns_value_when_set() {
        let mut node = crate::gen_shader::ShaderNode::new("test");
        let input = node.add_input("space", crate::gen_shader::type_desc_types::integer());
        input
            .port_mut()
            .set_value(Some(crate::core::Value::Integer(2)), false);
        assert_eq!(super::get_node_space(&node, 1), 2);
    }
}
