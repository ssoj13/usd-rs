
//! HdStLightingShader - Base lighting shader.
//!
//! Provides shader code for lighting calculations including:
//! - Light contribution accumulation
//! - Shadow mapping
//! - Environment lighting
//! - Material interaction

use crate::shader_code::{HdStShaderCode, NamedTextureHandle, ShaderParameter, ShaderStage};
use std::sync::Arc;

/// Lighting model type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightingModel {
    /// Constant (unlit) shading
    Constant,
    /// Simple Lambert diffuse
    Lambert,
    /// Blinn-Phong specular
    BlinnPhong,
    /// Physically-based rendering (PBR)
    Pbr,
    /// Custom lighting model
    Custom,
}

impl LightingModel {
    /// Get the model name as string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Constant => "constant",
            Self::Lambert => "lambert",
            Self::BlinnPhong => "blinn_phong",
            Self::Pbr => "pbr",
            Self::Custom => "custom",
        }
    }
}

/// Light type for shader generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightType {
    /// Directional light (sun)
    Directional,
    /// Point light (omni)
    Point,
    /// Spot light (cone)
    Spot,
    /// Dome light (environment)
    Dome,
    /// Area light (rectangular)
    Area,
}

/// Shadow parameters.
#[derive(Debug, Clone)]
pub struct ShadowParams {
    /// Enable shadows
    pub enabled: bool,
    /// Shadow map resolution
    pub resolution: u32,
    /// Shadow bias
    pub bias: f32,
    /// PCF filter size
    pub filter_size: u32,
}

impl Default for ShadowParams {
    fn default() -> Self {
        Self {
            enabled: false,
            resolution: 1024,
            bias: 0.001,
            filter_size: 2,
        }
    }
}

/// Lighting shader for fragment shading.
///
/// Generates fragment shader code for lighting calculations:
/// - Accumulates contributions from all lights
/// - Applies material properties (diffuse, specular, roughness)
/// - Computes shadows
/// - Applies environment lighting
///
/// # Integration
///
/// Lighting shaders integrate with geometric and material shaders:
/// - Geometric shader provides surface normals and positions
/// - Material shader provides BRDF parameters
/// - Lighting shader computes final color
#[derive(Debug)]
pub struct HdStLightingShader {
    /// Unique shader ID
    id: u64,

    /// Lighting model
    model: LightingModel,

    /// Active lights
    lights: Vec<LightType>,

    /// Shadow parameters
    shadow_params: ShadowParams,

    /// Fragment shader source
    fragment_source: String,

    /// Shader parameters
    params: Vec<ShaderParameter>,

    /// Texture handles (shadow maps, environment maps)
    textures: Vec<NamedTextureHandle>,

    /// Use ambient occlusion
    use_ao: bool,

    /// Use image-based lighting
    use_ibl: bool,
}

impl HdStLightingShader {
    /// Create a new lighting shader.
    pub fn new(id: u64, model: LightingModel) -> Self {
        let mut shader = Self {
            id,
            model,
            lights: Vec::new(),
            shadow_params: ShadowParams::default(),
            fragment_source: String::new(),
            params: Vec::new(),
            textures: Vec::new(),
            use_ao: false,
            use_ibl: false,
        };

        shader.generate_fragment_source();
        shader
    }

    /// Get lighting model.
    pub fn get_model(&self) -> LightingModel {
        self.model
    }

    /// Add a light to the shader.
    pub fn add_light(&mut self, light_type: LightType) {
        self.lights.push(light_type);
        self.generate_fragment_source();
    }

    /// Get all lights.
    pub fn get_lights(&self) -> &[LightType] {
        &self.lights
    }

    /// Set shadow parameters.
    pub fn set_shadow_params(&mut self, params: ShadowParams) {
        self.shadow_params = params;
        self.generate_fragment_source();
    }

    /// Get shadow parameters.
    pub fn get_shadow_params(&self) -> &ShadowParams {
        &self.shadow_params
    }

    /// Enable ambient occlusion.
    pub fn set_use_ao(&mut self, enable: bool) {
        self.use_ao = enable;
        self.generate_fragment_source();
    }

    /// Check if using ambient occlusion.
    pub fn uses_ao(&self) -> bool {
        self.use_ao
    }

    /// Enable image-based lighting.
    pub fn set_use_ibl(&mut self, enable: bool) {
        self.use_ibl = enable;
        self.generate_fragment_source();
    }

    /// Check if using image-based lighting.
    pub fn uses_ibl(&self) -> bool {
        self.use_ibl
    }

    /// Generate fragment shader source.
    fn generate_fragment_source(&mut self) {
        let mut source = String::from("#version 450\n\n");

        // Inputs from vertex/geometry shader
        source.push_str("in VertexData {\n");
        source.push_str("    vec3 position;\n");
        source.push_str("    vec3 normal;\n");
        source.push_str("    vec2 uv;\n");
        source.push_str("} inData;\n\n");

        // Material uniforms
        source.push_str("layout(binding = 1) uniform Material {\n");
        source.push_str("    vec4 diffuseColor;\n");
        source.push_str("    vec4 specularColor;\n");
        source.push_str("    float roughness;\n");
        source.push_str("    float metallic;\n");
        source.push_str("};\n\n");

        // Light uniforms
        if !self.lights.is_empty() {
            source.push_str("layout(binding = 2) uniform Lights {\n");
            source.push_str(&format!("    int numLights; // {}\n", self.lights.len()));
            source.push_str("    vec4 lightPositions[8];\n");
            source.push_str("    vec4 lightColors[8];\n");
            source.push_str("};\n\n");
        }

        // Output
        source.push_str("layout(location = 0) out vec4 outColor;\n\n");

        // Helper functions
        source.push_str(&self.generate_lighting_functions());

        // Main function
        source.push_str("void main() {\n");
        source.push_str("    vec3 N = normalize(inData.normal);\n");
        source.push_str("    vec3 V = normalize(-inData.position);\n");
        source.push_str("    vec3 color = vec3(0.0);\n\n");

        match self.model {
            LightingModel::Constant => {
                source.push_str("    color = diffuseColor.rgb;\n");
            }
            LightingModel::Lambert => {
                source.push_str("    color = computeLambert(N, V);\n");
            }
            LightingModel::BlinnPhong => {
                source.push_str("    color = computeBlinnPhong(N, V);\n");
            }
            LightingModel::Pbr => {
                source.push_str("    color = computePBR(N, V);\n");
            }
            LightingModel::Custom => {
                source.push_str("    color = diffuseColor.rgb; // Custom implementation\n");
            }
        }

        if self.use_ao {
            source.push_str("    color *= computeAO();\n");
        }

        if self.use_ibl {
            source.push_str("    color += computeIBL(N, V);\n");
        }

        source.push_str("\n    outColor = vec4(color, diffuseColor.a);\n");
        source.push_str("}\n");

        self.fragment_source = source;
    }

    /// Generate lighting calculation functions.
    fn generate_lighting_functions(&self) -> String {
        let mut funcs = String::new();

        // Lambert diffuse
        funcs.push_str("vec3 computeLambert(vec3 N, vec3 V) {\n");
        funcs.push_str("    vec3 result = vec3(0.0);\n");
        funcs.push_str("    for (int i = 0; i < numLights; i++) {\n");
        funcs.push_str("        vec3 L = normalize(lightPositions[i].xyz - inData.position);\n");
        funcs.push_str("        float NdotL = max(dot(N, L), 0.0);\n");
        funcs.push_str("        result += diffuseColor.rgb * lightColors[i].rgb * NdotL;\n");
        funcs.push_str("    }\n");
        funcs.push_str("    return result;\n");
        funcs.push_str("}\n\n");

        // Blinn-Phong
        funcs.push_str("vec3 computeBlinnPhong(vec3 N, vec3 V) {\n");
        funcs.push_str("    vec3 result = vec3(0.0);\n");
        funcs.push_str("    for (int i = 0; i < numLights; i++) {\n");
        funcs.push_str("        vec3 L = normalize(lightPositions[i].xyz - inData.position);\n");
        funcs.push_str("        vec3 H = normalize(L + V);\n");
        funcs.push_str("        float NdotL = max(dot(N, L), 0.0);\n");
        funcs.push_str("        float NdotH = max(dot(N, H), 0.0);\n");
        funcs.push_str("        vec3 diffuse = diffuseColor.rgb * NdotL;\n");
        funcs.push_str("        vec3 specular = specularColor.rgb * pow(NdotH, 32.0);\n");
        funcs.push_str("        result += (diffuse + specular) * lightColors[i].rgb;\n");
        funcs.push_str("    }\n");
        funcs.push_str("    return result;\n");
        funcs.push_str("}\n\n");

        // PBR (simplified)
        funcs.push_str("vec3 computePBR(vec3 N, vec3 V) {\n");
        funcs.push_str("    vec3 result = vec3(0.0);\n");
        funcs.push_str("    for (int i = 0; i < numLights; i++) {\n");
        funcs.push_str("        vec3 L = normalize(lightPositions[i].xyz - inData.position);\n");
        funcs.push_str("        float NdotL = max(dot(N, L), 0.0);\n");
        funcs.push_str("        vec3 radiance = lightColors[i].rgb * NdotL;\n");
        funcs.push_str("        result += diffuseColor.rgb * radiance * (1.0 - metallic);\n");
        funcs.push_str("    }\n");
        funcs.push_str("    return result;\n");
        funcs.push_str("}\n\n");

        // Ambient occlusion
        if self.use_ao {
            funcs.push_str("float computeAO() {\n");
            funcs.push_str("    return 1.0; // Placeholder\n");
            funcs.push_str("}\n\n");
        }

        // Image-based lighting
        if self.use_ibl {
            funcs.push_str("vec3 computeIBL(vec3 N, vec3 V) {\n");
            funcs.push_str("    return vec3(0.0); // Placeholder\n");
            funcs.push_str("}\n\n");
        }

        funcs
    }
}

impl HdStShaderCode for HdStLightingShader {
    fn get_id(&self) -> u64 {
        self.id
    }

    fn get_source(&self, stage: ShaderStage) -> String {
        match stage {
            ShaderStage::Fragment => self.fragment_source.clone(),
            _ => String::new(),
        }
    }

    fn get_params(&self) -> Vec<ShaderParameter> {
        self.params.clone()
    }

    fn get_textures(&self) -> Vec<NamedTextureHandle> {
        self.textures.clone()
    }

    fn is_valid(&self) -> bool {
        !self.fragment_source.is_empty()
    }
}

/// Shared pointer to lighting shader.
pub type HdStLightingShaderSharedPtr = Arc<HdStLightingShader>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lighting_model() {
        assert_eq!(LightingModel::Lambert.as_str(), "lambert");
        assert_eq!(LightingModel::Pbr.as_str(), "pbr");
    }

    #[test]
    fn test_shadow_params() {
        let params = ShadowParams::default();
        assert!(!params.enabled);
        assert_eq!(params.resolution, 1024);
    }

    #[test]
    fn test_lighting_shader_creation() {
        let shader = HdStLightingShader::new(42, LightingModel::Lambert);
        assert_eq!(shader.get_id(), 42);
        assert_eq!(shader.get_model(), LightingModel::Lambert);
        assert!(shader.is_valid());
    }

    #[test]
    fn test_add_lights() {
        let mut shader = HdStLightingShader::new(1, LightingModel::BlinnPhong);
        assert_eq!(shader.get_lights().len(), 0);

        shader.add_light(LightType::Directional);
        shader.add_light(LightType::Point);
        assert_eq!(shader.get_lights().len(), 2);
    }

    #[test]
    fn test_fragment_source() {
        let shader = HdStLightingShader::new(1, LightingModel::Lambert);
        let source = shader.get_source(ShaderStage::Fragment);

        assert!(!source.is_empty());
        assert!(source.contains("#version 450"));
        assert!(source.contains("computeLambert"));
    }

    #[test]
    fn test_pbr_shader() {
        let shader = HdStLightingShader::new(1, LightingModel::Pbr);
        let source = shader.get_source(ShaderStage::Fragment);

        assert!(source.contains("computePBR"));
        assert!(source.contains("roughness"));
        assert!(source.contains("metallic"));
    }

    #[test]
    fn test_constant_lighting() {
        let shader = HdStLightingShader::new(1, LightingModel::Constant);
        let source = shader.get_source(ShaderStage::Fragment);

        assert!(source.contains("diffuseColor.rgb"));
    }

    #[test]
    fn test_ambient_occlusion() {
        let mut shader = HdStLightingShader::new(1, LightingModel::Lambert);
        assert!(!shader.uses_ao());

        shader.set_use_ao(true);
        assert!(shader.uses_ao());

        let source = shader.get_source(ShaderStage::Fragment);
        assert!(source.contains("computeAO"));
    }

    #[test]
    fn test_image_based_lighting() {
        let mut shader = HdStLightingShader::new(1, LightingModel::Pbr);
        assert!(!shader.uses_ibl());

        shader.set_use_ibl(true);
        assert!(shader.uses_ibl());

        let source = shader.get_source(ShaderStage::Fragment);
        assert!(source.contains("computeIBL"));
    }

    #[test]
    fn test_shadow_params_config() {
        let mut shader = HdStLightingShader::new(1, LightingModel::Lambert);
        let mut params = ShadowParams::default();
        params.enabled = true;
        params.resolution = 2048;

        shader.set_shadow_params(params.clone());
        assert_eq!(shader.get_shadow_params().resolution, 2048);
        assert!(shader.get_shadow_params().enabled);
    }
}
