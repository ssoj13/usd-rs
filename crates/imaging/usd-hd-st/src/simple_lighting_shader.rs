//! Simple lighting shader for Storm (ported from simpleLightingShader.h).
//!
//! Standard lighting shader using GlfSimpleLighting-style parameters.
//! Supports multiple light types, shadow mapping, and dome light
//! environment textures.

use crate::binding::BindingRequest;
use crate::lighting::{self, LIGHT_UNIFORMS_SIZE, LightGpuData, MAX_LIGHTS};
use crate::lighting_shader::{HdStLightingShader, LightingModel};
use crate::shader_code::NamedTextureHandle;
use crate::shadow::{self, SHADOW_UNIFORMS_SIZE, ShadowEntry};
use crate::texture_handle::HdStTextureHandleSharedPtr;
use std::collections::BTreeMap;
use std::sync::Arc;
use usd_tf::Token;

// ---------------------------------------------------------------------------
// IBL texture data (CPU-side, for upload to GPU)
// ---------------------------------------------------------------------------

/// Precomputed IBL texture data from a DomeLight HDRI.
///
/// All cubemaps are stored as flattened f32 RGBA arrays, face-major order
/// (+X, -X, +Y, -Y, +Z, -Z). The engine uploads these to GPU via HGI.
///
/// This struct is created by SimpleLightTask when a DomeLight texture is found.
#[derive(Debug, Clone)]
pub struct IblTextures {
    /// Source HDRI file path (for cache/invalidation)
    pub hdri_path: String,
    /// Cubemap face dimension (pow2, e.g. 512)
    pub face_dim: u32,
    /// Irradiance cubemap pixels: RGBA f32, 6 * face_dim^2 texels
    pub irradiance: Vec<f32>,
    /// Irradiance face_dim (may be smaller, e.g. 32)
    pub irradiance_dim: u32,
    /// Prefiltered specular cubemap pixels: RGBA f32, mip0 (all 6 faces)
    pub prefilter: Vec<f32>,
    /// Number of prefilter mip levels (typically 5)
    pub prefilter_mip_count: u32,
    /// BRDF LUT pixels: RG f32, brdf_dim^2 texels
    pub brdf_lut: Vec<f32>,
    /// BRDF LUT dimension
    pub brdf_dim: u32,
}

/// Simple lighting shader.
///
/// Provides standard multi-light shading with:
/// - Point, directional, and spot lights
/// - Shadow mapping support
/// - Dome light environment textures (pre-computed from environment maps)
/// - Custom buffer bindings for additional per-light data
///
/// Ported from C++ HdStSimpleLightingShader. The C++ version uses
/// GlfSimpleLightingContext; we embed equivalent functionality.
#[derive(Debug)]
pub struct HdStSimpleLightingShader {
    /// Inner lighting shader (PBR model by default)
    inner: HdStLightingShader,
    /// Whether lighting is enabled
    use_lighting: bool,
    /// World-to-view matrix
    world_to_view: [f32; 16],
    /// Projection matrix
    projection: [f32; 16],
    /// Custom buffer bindings (sorted by name for stable output)
    custom_buffers: BTreeMap<Token, BindingRequest>,
    /// Dome light environment texture handle
    dome_light_env_texture: Option<HdStTextureHandleSharedPtr>,
    /// Named texture handles for dome light pre-computed textures:
    /// [0] = cubemap, [1] = irradiance, [2] = prefilter, [3] = BRDF LUT
    named_texture_handles: Vec<NamedTextureHandle>,
    /// Dome light cubemap target memory (MB)
    dome_light_cubemap_target_memory_mb: u32,
    /// Current scene lights (set each frame by engine or light task)
    scene_lights: Vec<LightGpuData>,
    /// Precomputed IBL textures from DomeLight (None = no dome light)
    ibl_textures: Option<IblTextures>,
    /// Whether shadow mapping is active (any light has hasShadow=true).
    /// Matches C++ GlfSimpleLightingContext::GetUseShadows().
    use_shadows: bool,
    /// Per-light shadow entries for GPU upload.
    /// Matches C++ GlfSimpleShadowArray shadow data.
    shadow_entries: Vec<ShadowEntry>,
}

impl Default for HdStSimpleLightingShader {
    fn default() -> Self {
        Self::new()
    }
}

impl HdStSimpleLightingShader {
    /// Stable hash ID for simple lighting.
    const SHADER_ID: u64 = 0x5100_1E01;

    /// Create a new simple lighting shader.
    pub fn new() -> Self {
        Self {
            inner: HdStLightingShader::new(Self::SHADER_ID, LightingModel::Pbr),
            use_lighting: true,
            world_to_view: identity_matrix(),
            projection: identity_matrix(),
            custom_buffers: BTreeMap::new(),
            dome_light_env_texture: None,
            // Reserve 4 slots for dome light textures (cubemap, irradiance, prefilter, BRDF)
            named_texture_handles: Vec::with_capacity(4),
            dome_light_cubemap_target_memory_mb: 256,
            scene_lights: Vec::new(),
            ibl_textures: None,
            use_shadows: false,
            shadow_entries: Vec::new(),
        }
    }

    /// Set camera matrices.
    pub fn set_camera(&mut self, world_to_view: [f32; 16], projection: [f32; 16]) {
        self.world_to_view = world_to_view;
        self.projection = projection;
    }

    /// Enable or disable lighting.
    pub fn set_use_lighting(&mut self, enabled: bool) {
        self.use_lighting = enabled;
    }

    /// Whether lighting is enabled.
    pub fn use_lighting(&self) -> bool {
        self.use_lighting
    }

    /// Get a reference to the inner lighting shader.
    pub fn get_lighting_shader(&self) -> &HdStLightingShader {
        &self.inner
    }

    /// Get a mutable reference to the inner lighting shader.
    pub fn get_lighting_shader_mut(&mut self) -> &mut HdStLightingShader {
        &mut self.inner
    }

    /// Add a custom buffer binding request.
    pub fn add_buffer_binding(&mut self, req: BindingRequest) {
        self.custom_buffers.insert(req.name.clone(), req);
    }

    /// Remove a custom buffer binding by name.
    pub fn remove_buffer_binding(&mut self, name: &Token) {
        self.custom_buffers.remove(name);
    }

    /// Clear all custom buffer bindings.
    pub fn clear_buffer_bindings(&mut self) {
        self.custom_buffers.clear();
    }

    /// Get all custom buffer bindings.
    pub fn get_custom_bindings(&self) -> Vec<&BindingRequest> {
        self.custom_buffers.values().collect()
    }

    /// Get the dome light environment texture handle.
    pub fn get_dome_light_env_texture(&self) -> Option<&HdStTextureHandleSharedPtr> {
        self.dome_light_env_texture.as_ref()
    }

    /// Set dome light cubemap target memory in MB.
    pub fn set_dome_light_cubemap_target_memory(&mut self, target_mb: u32) {
        self.dome_light_cubemap_target_memory_mb = target_mb;
    }

    /// Get dome light cubemap target memory in MB.
    pub fn get_dome_light_cubemap_target_memory(&self) -> u32 {
        self.dome_light_cubemap_target_memory_mb
    }

    /// Get named texture handles for shader binding.
    ///
    /// Slot layout (dome light future support):
    /// - 0: environment cubemap
    /// - 1: irradiance map
    /// - 2: prefiltered specular map
    /// - 3: BRDF LUT
    pub fn get_named_texture_handles(&self) -> &[NamedTextureHandle] {
        &self.named_texture_handles
    }

    // -------------------------------------------------------------------------
    // IBL texture management
    // -------------------------------------------------------------------------

    /// Set precomputed IBL textures from a DomeLight.
    ///
    /// Call from SimpleLightTask::prepare after loading and convolving HDRI.
    pub fn set_ibl_textures(&mut self, ibl: IblTextures) {
        self.ibl_textures = Some(ibl);
    }

    /// Clear IBL textures (no dome light in scene).
    pub fn clear_ibl(&mut self) {
        self.ibl_textures = None;
    }

    /// Get IBL textures if available.
    pub fn get_ibl_textures(&self) -> Option<&IblTextures> {
        self.ibl_textures.as_ref()
    }

    /// Whether IBL textures are loaded and ready.
    pub fn has_ibl(&self) -> bool {
        self.ibl_textures.is_some()
    }

    // -------------------------------------------------------------------------
    // Shadow management (matches C++ HdStSimpleLightingShader shadow path)
    // -------------------------------------------------------------------------

    /// Whether shadows are enabled (any light casts shadows).
    /// Matches C++ GlfSimpleLightingContext::GetUseShadows().
    pub fn use_shadows(&self) -> bool {
        self.use_shadows
    }

    /// Set shadow state. Called by light task after collecting shadow-casting lights.
    /// Matches C++ simpleLightingShader.cpp ComputeHash() useShadows path.
    pub fn set_shadows(&mut self, entries: Vec<ShadowEntry>) {
        self.use_shadows = !entries.is_empty();
        self.shadow_entries = entries;
    }

    /// Clear shadow state (no shadow-casting lights).
    pub fn clear_shadows(&mut self) {
        self.use_shadows = false;
        self.shadow_entries.clear();
    }

    /// Get shadow entries.
    pub fn get_shadow_entries(&self) -> &[ShadowEntry] {
        &self.shadow_entries
    }

    /// Number of active shadow maps.
    /// Matches C++ GlfSimpleLightingContext::ComputeNumShadowsUsed().
    pub fn num_shadows(&self) -> usize {
        self.shadow_entries.len()
    }

    /// Build the GPU shadow uniform buffer for the current frame.
    /// Returns exactly SHADOW_UNIFORMS_SIZE bytes ready for GPU upload.
    pub fn build_shadow_uniforms_bytes(&self) -> Vec<u8> {
        shadow::build_shadow_uniforms(&self.shadow_entries)
    }

    /// Get the expected size of the shadow uniforms buffer.
    pub fn shadow_uniforms_size() -> usize {
        SHADOW_UNIFORMS_SIZE
    }

    // -------------------------------------------------------------------------
    // Scene light management
    // -------------------------------------------------------------------------

    /// Set scene lights for the current frame.
    ///
    /// Called by the engine or light task after collecting lights from the
    /// render index. Falls back to default 3-point lighting if empty.
    pub fn set_scene_lights(&mut self, lights: Vec<LightGpuData>) {
        self.scene_lights = lights;
    }

    /// Get currently set scene lights.
    pub fn get_scene_lights(&self) -> &[LightGpuData] {
        &self.scene_lights
    }

    /// Whether any scene lights are set.
    pub fn has_scene_lights(&self) -> bool {
        !self.scene_lights.is_empty()
    }

    /// Build the GPU light uniform buffer for the current frame.
    ///
    /// Uses scene lights if available, otherwise falls back to default 3-point lighting.
    /// Returns exactly LIGHT_UNIFORMS_SIZE bytes ready for GPU upload.
    pub fn build_light_uniforms_bytes(&self) -> Vec<u8> {
        let lights = if self.scene_lights.is_empty() {
            lighting::default_lights()
        } else {
            self.scene_lights.clone()
        };
        lighting::build_light_uniforms(&lights)
    }

    /// Get the expected size of the light uniforms buffer.
    pub fn light_uniforms_size() -> usize {
        LIGHT_UNIFORMS_SIZE
    }

    /// Get the maximum number of lights supported.
    pub fn max_lights() -> usize {
        MAX_LIGHTS
    }

    // -------------------------------------------------------------------------
    // WGSL code generation
    // -------------------------------------------------------------------------

    /// Generate WGSL uniform struct declarations for lighting.
    ///
    /// Emits `LightEntry`, `LightUniforms`, and the `@group(group) @binding(binding)`
    /// variable declaration. Must be included before any light evaluation functions.
    ///
    /// # Parameters
    /// - `group` - bind group index (typically 1)
    /// - `binding` - binding index within the group (typically 1)
    pub fn gen_light_uniforms_wgsl(group: u32, binding: u32) -> String {
        let mut src = String::new();
        lighting::emit_light_uniforms_wgsl(&mut src, group, binding);
        src
    }

    /// Generate WGSL light evaluation helper functions.
    ///
    /// Includes eval_directional, eval_point_light, eval_spot_light,
    /// eval_dome_light, and the GGX/Smith/Fresnel PBR helpers.
    pub fn gen_light_eval_functions_wgsl() -> String {
        let mut src = String::new();
        lighting::emit_light_eval_functions(&mut src);
        src
    }

    /// Generate the multi-light loop WGSL snippet.
    ///
    /// Produces `lo` accumulation variable and loop over `light_data.lights`.
    /// Caller is responsible for declaring N, V, world_pos, base_color,
    /// roughness, metallic, f0 before this snippet.
    pub fn gen_light_loop_wgsl() -> String {
        let mut src = String::new();
        lighting::emit_light_loop(&mut src);
        src
    }

    /// Generate the complete lighting WGSL preamble (structs + functions).
    ///
    /// Convenience method that combines gen_light_uniforms_wgsl and
    /// gen_light_eval_functions_wgsl with standard bind group 1, binding 1.
    pub fn gen_lighting_preamble_wgsl() -> String {
        let mut src = String::new();
        lighting::emit_light_uniforms_wgsl(&mut src, 1, 1);
        lighting::emit_light_eval_functions(&mut src);
        src
    }

    /// Get the fragment shader source for simple lighting.
    pub fn get_fragment_source() -> &'static str {
        SIMPLE_LIGHTING_FRAGMENT
    }

    /// Get the vertex shader source snippet for simple lighting.
    pub fn get_vertex_source() -> &'static str {
        SIMPLE_LIGHTING_VERTEX
    }
}

/// Shared pointer to simple lighting shader.
pub type HdStSimpleLightingShaderSharedPtr = Arc<HdStSimpleLightingShader>;

// Embedded simple lighting vertex shader snippet (WGSL).
const SIMPLE_LIGHTING_VERTEX: &str = r#"
// Simple lighting: pass eye-space position and normal to fragment
fn lighting_vertex(position_eye: vec3<f32>, normal_eye: vec3<f32>) -> vec3<f32> {
    return normal_eye;
}
"#;

// Embedded simple lighting fragment shader snippet (WGSL).
// NOTE: this is a minimal fallback; the real PBR multi-light shader is
// generated by wgsl_code_gen.rs using emit_light_uniforms_wgsl + emit_light_loop.
const SIMPLE_LIGHTING_FRAGMENT: &str = r#"
// Simple lighting: multi-light accumulation
fn simple_lighting(
    normal_eye: vec3<f32>,
    position_eye: vec3<f32>,
    shininess: f32,
) -> vec3<f32> {
    let light_dir = normalize(vec3<f32>(0.5, 1.0, 0.8));
    let n = normalize(normal_eye);
    let n_dot_l = max(dot(n, light_dir), 0.0);
    let ambient = vec3<f32>(0.04, 0.04, 0.04);
    let diffuse = vec3<f32>(0.8, 0.8, 0.8) * n_dot_l;
    let view_dir = normalize(-position_eye);
    let half_dir = normalize(light_dir + view_dir);
    let spec = pow(max(dot(n, half_dir), 0.0), shininess);
    let specular = vec3<f32>(1.0, 1.0, 1.0) * spec;
    return ambient + diffuse + specular * 0.5;
}
"#;

/// Create a 4x4 identity matrix.
fn identity_matrix() -> [f32; 16] {
    [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_lighting_shader() {
        let shader = HdStSimpleLightingShader::new();
        assert!(shader.use_lighting());
        assert_eq!(shader.get_lighting_shader().get_model(), LightingModel::Pbr);
    }

    #[test]
    fn test_custom_bindings() {
        let mut shader = HdStSimpleLightingShader::new();

        let req = BindingRequest::texture("shadowTexture", 2, 0);
        shader.add_buffer_binding(req);
        assert_eq!(shader.get_custom_bindings().len(), 1);

        shader.remove_buffer_binding(&Token::new("shadowTexture"));
        assert!(shader.get_custom_bindings().is_empty());
    }

    #[test]
    fn test_fragment_source() {
        let src = HdStSimpleLightingShader::get_fragment_source();
        assert!(src.contains("simple_lighting"));
    }

    #[test]
    fn test_gen_light_uniforms_wgsl() {
        let src = HdStSimpleLightingShader::gen_light_uniforms_wgsl(1, 1);
        assert!(src.contains("LightEntry"));
        assert!(src.contains("LightUniforms"));
        assert!(src.contains("@group(1) @binding(1)"));
    }

    #[test]
    fn test_gen_light_eval_functions() {
        let src = HdStSimpleLightingShader::gen_light_eval_functions_wgsl();
        assert!(src.contains("fn eval_directional"));
        assert!(src.contains("fn eval_point_light"));
        assert!(src.contains("fn eval_dome_light"));
    }

    #[test]
    fn test_gen_lighting_preamble() {
        let src = HdStSimpleLightingShader::gen_lighting_preamble_wgsl();
        assert!(src.contains("LightUniforms"));
        assert!(src.contains("eval_directional"));
    }

    #[test]
    fn test_gen_light_loop() {
        let src = HdStSimpleLightingShader::gen_light_loop_wgsl();
        assert!(src.contains("light_data.light_count"));
        assert!(src.contains("light_data.lights[li]"));
    }

    #[test]
    fn test_build_light_uniforms_bytes_default() {
        let shader = HdStSimpleLightingShader::new();
        let bytes = shader.build_light_uniforms_bytes();
        assert_eq!(
            bytes.len(),
            LIGHT_UNIFORMS_SIZE,
            "light uniforms buffer must be exactly LIGHT_UNIFORMS_SIZE bytes"
        );
        // Default 3-point lighting -> count = 3
        let count = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        assert_eq!(count, 3, "default 3-point lighting has 3 lights");
    }

    #[test]
    fn test_build_light_uniforms_bytes_scene() {
        let mut shader = HdStSimpleLightingShader::new();
        shader.set_scene_lights(vec![lighting::default_light()]);
        let bytes = shader.build_light_uniforms_bytes();
        assert_eq!(bytes.len(), LIGHT_UNIFORMS_SIZE);
        let count = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        assert_eq!(count, 1, "one scene light set");
    }

    #[test]
    fn test_dome_light_texture_slots() {
        let shader = HdStSimpleLightingShader::new();
        // Slots are reserved but empty until dome light is attached
        assert!(shader.get_named_texture_handles().is_empty());
        assert_eq!(
            shader.named_texture_handles.capacity(),
            4,
            "4 slots reserved for dome light textures"
        );
    }

    #[test]
    fn test_max_lights_constant() {
        assert_eq!(HdStSimpleLightingShader::max_lights(), MAX_LIGHTS);
    }

    #[test]
    fn test_light_uniforms_size_constant() {
        assert_eq!(
            HdStSimpleLightingShader::light_uniforms_size(),
            LIGHT_UNIFORMS_SIZE
        );
    }
}
