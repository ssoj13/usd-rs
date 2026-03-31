//! Collection of lights and lighting state.
//!
//! Port of pxr/imaging/glf/simpleLightingContext.h

use super::{GlfBindingMap, GlfSimpleLight, GlfSimpleMaterial, GlfSimpleShadowArray, TfToken};
use std::sync::Arc;
use usd_gf::{Matrix4d, Vec4f};

// Type aliases to match USD naming convention

/// Type alias for 4x4 double-precision matrix.
///
/// Matches USD's `GfMatrix4d` type. Used for transformation matrices
/// in world, view, and projection spaces.
pub type GfMatrix4d = Matrix4d;

/// Type alias for 4-component floating-point vector.
///
/// Matches USD's `GfVec4f` type. Used for RGBA colors and homogeneous coordinates.
pub type GfVec4f = Vec4f;

// -----------------------------------------------------------------------
// UBO layout structs (std140, 16-byte aligned)
// Must match simpleLightingShader.glslfx / C++ BindUniformBlocks layout.
// -----------------------------------------------------------------------

/// Per-light data packed for std140 UBO upload.
/// Mirrors the C++ anonymous `LightSource` struct in BindUniformBlocks().
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct UboLightSource {
    position: [f32; 4],        // 16
    ambient: [f32; 4],         // 16
    diffuse: [f32; 4],         // 16
    specular: [f32; 4],        // 16
    spot_direction: [f32; 4],  // 16 (padded from vec3)
    spot_cutoff: f32,          //  4
    spot_falloff: f32,         //  4
    _padding: [f32; 2],        //  8
    attenuation: [f32; 4],     // 16 (padded from vec3)
    world_to_light: [f32; 16], // 64
    shadow_index_start: i32,   //  4
    shadow_index_end: i32,     //  4
    has_shadow: i32,           //  4
    is_indirect_light: i32,    //  4
}

/// Header block that precedes the light array in the Lighting UBO.
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct UboLightingHeader {
    use_lighting: i32,               // 4
    use_color_material_diffuse: i32, // 4
    _padding: [i32; 2],              // 8  — pad to 16
}

/// Per-shadow matrix data for std140 UBO.
/// Mirrors the C++ `ShadowMatrix` struct.
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct UboShadowMatrix {
    view_to_shadow: [f32; 16], // 64
    shadow_to_view: [f32; 16], // 64
    blur: f32,                 //  4
    bias: f32,                 //  4
    _padding: [f32; 2],        //  8
}

/// Material data for std140 UBO.
/// Mirrors the C++ `Material` struct.
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct UboMaterial {
    ambient: [f32; 4],     // 16
    diffuse: [f32; 4],     // 16
    specular: [f32; 4],    // 16
    emission: [f32; 4],    // 16
    scene_color: [f32; 4], // 16
    shininess: f32,        //  4
    _padding: [f32; 3],    // 12
}

// -----------------------------------------------------------------------

/// Name for the shadow compare textures sampler array (matches C++ token).
///
/// C++ uses `TfStringPrintf("%s[%zd]", _tokens->shadowCompareTextures.GetText(), i)`.
const SHADOW_COMPARE_TEXTURES: &str = "shadowCompareTextures";

/// Manages lighting state for OpenGL preview rendering.
///
/// `GlfSimpleLightingContext` encapsulates a collection of lights, shadow maps,
/// material properties, and camera matrices required for basic lighting scenarios
/// in OpenGL-based preview rendering. This context provides the state needed to
/// generate appropriate shader code and bind lighting uniforms.
///
/// The context tracks:
/// - Multiple light sources with optional shadow mapping
/// - Scene ambient lighting
/// - Material properties for surface shading
/// - Camera matrices (world-to-view and projection)
/// - Shader generation hints and caching
///
/// # Usage
///
/// ```ignore
/// let mut ctx = GlfSimpleLightingContext::new();
/// ctx.set_lights(vec![light1, light2]);
/// ctx.set_material(material);
/// ctx.set_camera(view_matrix, projection_matrix);
///
/// // Generate shader code
/// let hash = ctx.compute_shader_source_hash();
/// let glsl = ctx.compute_shader_source(&shader_stage);
/// ```
///
/// Port of `pxr::GlfSimpleLightingContext` from `pxr/imaging/glf/simpleLightingContext.h`.
#[derive(Debug, Clone)]
pub struct GlfSimpleLightingContext {
    /// Collection of light sources in the scene.
    lights: Vec<GlfSimpleLight>,

    /// Optional shadow map array for shadow-casting lights.
    shadows: Option<Arc<GlfSimpleShadowArray>>,

    /// World-to-view transformation matrix for camera space conversion.
    world_to_view_matrix: GfMatrix4d,

    /// Projection matrix for view-to-clip space transformation.
    projection_matrix: GfMatrix4d,

    /// Material properties for surface shading.
    material: GlfSimpleMaterial,

    /// Scene-wide ambient light color (RGB + alpha).
    scene_ambient: GfVec4f,

    /// Flag controlling whether lighting calculations are enabled.
    use_lighting: bool,

    /// Flag indicating if any light in the scene has shadows enabled.
    /// Updated automatically when lights are set.
    use_shadows: bool,

    /// Flag to use vertex color as diffuse material component.
    use_color_material_diffuse: bool,

    /// Cached hash of shader source configuration for de-duplication.
    shader_source_hash: usize,

    // ---- CPU-side UBO caches (wgpu consumes these; GL uploads them) -----
    /// Packed Lighting UBO bytes (header + LightSource[n]).
    lighting_ubo_data: Vec<u8>,
    /// Packed Shadow UBO bytes (ShadowMatrix[n]).
    shadow_ubo_data: Vec<u8>,
    /// Packed Material UBO bytes.
    material_ubo_data: Vec<u8>,

    /// Dirty flags — mirror C++ `_*UniformBlockValid` fields.
    lighting_ubo_valid: bool,
    shadow_ubo_valid: bool,
    material_ubo_valid: bool,
}

impl GlfSimpleLightingContext {
    /// Creates a new lighting context with default settings.
    ///
    /// Initializes an empty lighting context with:
    /// - No lights
    /// - Identity camera matrices
    /// - Default material
    /// - Black scene ambient
    /// - Lighting enabled, shadows disabled
    ///
    /// # Returns
    ///
    /// A new `GlfSimpleLightingContext` with default configuration.
    pub fn new() -> Self {
        Self {
            lights: Vec::new(),
            // C++: _shadows(TfCreateRefPtr(new GlfSimpleShadowArray()))
            shadows: Some(Arc::new(GlfSimpleShadowArray::new())),
            world_to_view_matrix: GfMatrix4d::identity(),
            projection_matrix: GfMatrix4d::identity(),
            material: GlfSimpleMaterial::new(),
            // C++: _sceneAmbient(0.01, 0.01, 0.01, 1.0)
            scene_ambient: GfVec4f::new(0.01, 0.01, 0.01, 1.0),
            // C++: _useLighting(false)
            use_lighting: false,
            use_shadows: false,
            use_color_material_diffuse: false,
            shader_source_hash: 0,
            lighting_ubo_data: Vec::new(),
            shadow_ubo_data: Vec::new(),
            material_ubo_data: Vec::new(),
            lighting_ubo_valid: false,
            shadow_ubo_valid: false,
            material_ubo_valid: false,
        }
    }

    // Lights

    /// Sets the collection of lights for this context.
    ///
    /// Replaces the current light collection and automatically updates
    /// the `use_shadows` flag based on whether any light has shadows enabled.
    ///
    /// # Parameters
    ///
    /// * `lights` - Vector of light sources to use for rendering.
    pub fn set_lights(&mut self, lights: Vec<GlfSimpleLight>) {
        self.lights = lights;
        self.lighting_ubo_valid = false;
        self.shadow_ubo_valid = false;
        self.update_use_shadows();
    }

    /// Returns the current collection of lights.
    ///
    /// # Returns
    ///
    /// Slice of all light sources in this context.
    pub fn get_lights(&self) -> &[GlfSimpleLight] {
        &self.lights
    }

    /// Returns the effective number of lights used in shader generation.
    ///
    /// This count respects composable/compatible shader constraints and
    /// is used to determine which shader variant to generate or select.
    ///
    /// # Returns
    ///
    /// Number of lights that will affect shader generation.
    pub fn get_num_lights_used(&self) -> usize {
        self.lights.len()
    }

    /// Computes the total number of shadow maps required.
    ///
    /// Matches C++ `ComputeNumShadowsUsed()`: returns `max(shadow_index_end)+1`
    /// across all shadow-casting lights. This accounts for lights that span
    /// multiple shadow indices (shadow cascades), not just a simple count.
    ///
    /// # Returns
    ///
    /// Total number of shadow map slots needed for current light configuration.
    pub fn compute_num_shadows_used(&self) -> usize {
        let mut num_shadows: i32 = 0;
        for light in &self.lights {
            if light.has_shadow() {
                let end = light.get_shadow_index_end();
                if num_shadows <= end {
                    num_shadows = end + 1;
                }
            }
        }
        num_shadows as usize
    }

    // Shadows

    /// Sets the shadow map array for shadow-casting lights.
    ///
    /// # Parameters
    ///
    /// * `shadows` - Shared shadow map array containing depth textures for shadow mapping calculations.
    pub fn set_shadows(&mut self, shadows: Arc<GlfSimpleShadowArray>) {
        self.shadows = Some(shadows);
        self.shadow_ubo_valid = false;
    }

    /// Returns the current shadow map array, if set.
    ///
    /// # Returns
    ///
    /// Optional reference to the shadow array. `None` if no shadows are configured.
    pub fn get_shadows(&self) -> Option<&Arc<GlfSimpleShadowArray>> {
        self.shadows.as_ref()
    }

    // Material

    /// Sets the material properties for surface shading.
    ///
    /// # Parameters
    ///
    /// * `material` - Material with ambient, diffuse, specular, and shininess properties.
    pub fn set_material(&mut self, material: GlfSimpleMaterial) {
        self.material = material;
        self.material_ubo_valid = false;
    }

    /// Returns the current material properties.
    ///
    /// # Returns
    ///
    /// Reference to the material used for surface shading.
    pub fn get_material(&self) -> &GlfSimpleMaterial {
        &self.material
    }

    // Scene ambient

    /// Sets the scene-wide ambient light color.
    ///
    /// Ambient light is added uniformly to all surfaces regardless of
    /// geometry or light positions.
    ///
    /// # Parameters
    ///
    /// * `ambient` - RGBA ambient light color. Alpha typically set to 1.0.
    pub fn set_scene_ambient(&mut self, ambient: GfVec4f) {
        if self.scene_ambient != ambient {
            self.scene_ambient = ambient;
            self.material_ubo_valid = false;
        }
    }

    /// Returns the current scene ambient light color.
    ///
    /// # Returns
    ///
    /// Reference to the RGBA ambient light color.
    pub fn get_scene_ambient(&self) -> &GfVec4f {
        &self.scene_ambient
    }

    // Camera

    /// Sets the camera transformation matrices.
    ///
    /// These matrices are used for transforming lights and geometry into
    /// appropriate coordinate spaces for lighting calculations.
    ///
    /// # Parameters
    ///
    /// * `world_to_view` - World-to-view (view) transformation matrix.
    /// * `projection` - View-to-clip (projection) transformation matrix.
    pub fn set_camera(&mut self, world_to_view: GfMatrix4d, projection: GfMatrix4d) {
        if self.world_to_view_matrix != world_to_view {
            self.world_to_view_matrix = world_to_view;
            self.lighting_ubo_valid = false;
            self.shadow_ubo_valid = false;
        }
        self.projection_matrix = projection;
    }

    /// Returns the world-to-view transformation matrix.
    ///
    /// # Returns
    ///
    /// Reference to the view matrix transforming from world to camera space.
    pub fn get_world_to_view_matrix(&self) -> &GfMatrix4d {
        &self.world_to_view_matrix
    }

    /// Returns the projection transformation matrix.
    ///
    /// # Returns
    ///
    /// Reference to the projection matrix transforming from view to clip space.
    pub fn get_projection_matrix(&self) -> &GfMatrix4d {
        &self.projection_matrix
    }

    // Use lighting

    /// Enables or disables lighting calculations.
    ///
    /// When disabled, surfaces may be rendered unlit or with a simplified shading model.
    ///
    /// # Parameters
    ///
    /// * `val` - `true` to enable lighting, `false` to disable.
    pub fn set_use_lighting(&mut self, val: bool) {
        if self.use_lighting != val {
            self.use_lighting = val;
            self.lighting_ubo_valid = false;
        }
    }

    /// Returns whether lighting calculations are enabled.
    ///
    /// # Returns
    ///
    /// `true` if lighting is enabled, `false` otherwise.
    pub fn get_use_lighting(&self) -> bool {
        self.use_lighting
    }

    // Use shadows

    /// Returns whether any light in the scene has shadows enabled.
    ///
    /// This flag is automatically updated when lights are set via `set_lights()`.
    ///
    /// # Returns
    ///
    /// `true` if at least one light casts shadows, `false` otherwise.
    pub fn get_use_shadows(&self) -> bool {
        self.use_shadows
    }

    /// Updates the `use_shadows` flag based on current light configuration.
    ///
    /// Internal helper that checks if any light has shadow mapping enabled.
    fn update_use_shadows(&mut self) {
        self.use_shadows = self.lights.iter().any(|light| light.has_shadow());
    }

    // Use color material diffuse

    /// Enables or disables using vertex color as diffuse material component.
    ///
    /// When enabled, per-vertex colors are multiplied with or replace the
    /// material's diffuse color during shading.
    ///
    /// # Parameters
    ///
    /// * `val` - `true` to use vertex colors for diffuse, `false` to use material only.
    pub fn set_use_color_material_diffuse(&mut self, val: bool) {
        if self.use_color_material_diffuse != val {
            self.use_color_material_diffuse = val;
            self.lighting_ubo_valid = false;
        }
    }

    /// Returns whether vertex colors are used for diffuse material.
    ///
    /// # Returns
    ///
    /// `true` if vertex colors affect diffuse shading, `false` otherwise.
    pub fn get_use_color_material_diffuse(&self) -> bool {
        self.use_color_material_diffuse
    }

    // Binding management

    /// Initializes uniform block bindings in the given binding map.
    ///
    /// Registers uniform blocks for lighting data, shadow parameters, and
    /// material properties. These blocks are later bound during rendering
    /// to provide shader uniforms.
    ///
    /// # Parameters
    ///
    /// * `binding_map` - Mutable binding map to register uniform blocks in.
    pub fn init_uniform_block_bindings(&self, binding_map: &mut GlfBindingMap) {
        // C++: lightingUB / shadowUB / materialUB / postSurfaceShaderUB
        binding_map.get_uniform_binding(&TfToken::new("Lighting"));
        binding_map.get_uniform_binding(&TfToken::new("Shadow"));
        binding_map.get_uniform_binding(&TfToken::new("Material"));
        binding_map.get_uniform_binding(&TfToken::new("PostSurfaceShaderParams"));
    }

    /// Initializes sampler unit bindings in the given binding map.
    ///
    /// Registers texture sampler units for shadow maps using the correct C++ naming
    /// convention: `shadowCompareTextures[i]`.
    ///
    /// # Parameters
    ///
    /// * `binding_map` - Mutable binding map to register sampler units in.
    pub fn init_sampler_unit_bindings(&self, binding_map: &mut GlfBindingMap) {
        // C++: TfStringPrintf("%s[%zd]", _tokens->shadowCompareTextures.GetText(), i)
        let num_shadows = self
            .shadows
            .as_ref()
            .map(|s| s.get_num_shadow_map_passes())
            .unwrap_or_else(|| self.compute_num_shadows_used());
        for i in 0..num_shadows {
            let sampler_name = TfToken::new(&format!("{}[{}]", SHADOW_COMPARE_TEXTURES, i));
            binding_map.get_sampler_unit(&sampler_name);
        }
    }

    // ------------------------------------------------------------------
    // CPU-side UBO packing helpers
    // ------------------------------------------------------------------

    /// Packs all UBO data into CPU-side byte buffers.
    ///
    /// Mirrors the logic of C++ `BindUniformBlocks()` up to the point of
    /// calling `GlfUniformBlock::Update()`.  After this call the buffers
    /// are accessible via `lighting_ubo_data()`, `shadow_ubo_data()`, and
    /// `material_ubo_data()` for wgpu upload or for GL via `bind_uniform_blocks()`.
    pub fn pack_ubo_data(&mut self) {
        self.pack_lighting_and_shadow_ubo();
        self.pack_material_ubo();
    }

    /// Returns the packed Lighting UBO bytes (after `pack_ubo_data()`).
    pub fn lighting_ubo_data(&self) -> &[u8] {
        &self.lighting_ubo_data
    }

    /// Returns the packed Shadow UBO bytes (after `pack_ubo_data()`).
    pub fn shadow_ubo_data(&self) -> &[u8] {
        &self.shadow_ubo_data
    }

    /// Returns the packed Material UBO bytes (after `pack_ubo_data()`).
    pub fn material_ubo_data(&self) -> &[u8] {
        &self.material_ubo_data
    }

    /// Packs Lighting + Shadow UBOs from current state.
    fn pack_lighting_and_shadow_ubo(&mut self) {
        if self.lighting_ubo_valid && self.shadow_ubo_valid {
            return;
        }

        let num_lights = self.lights.len();
        let num_shadows = self.compute_num_shadows_used();

        // Build Lighting UBO: header + LightSource[num_lights]
        let header = UboLightingHeader {
            use_lighting: self.use_lighting as i32,
            use_color_material_diffuse: self.use_color_material_diffuse as i32,
            _padding: [0; 2],
        };

        let header_bytes = unsafe {
            std::slice::from_raw_parts(
                &header as *const UboLightingHeader as *const u8,
                std::mem::size_of::<UboLightingHeader>(),
            )
        };

        let mut light_sources: Vec<UboLightSource> = vec![UboLightSource::default(); num_lights];
        let mut shadow_matrices: Vec<UboShadowMatrix> =
            vec![UboShadowMatrix::default(); num_shadows];

        // view-to-world: used for shadow matrix chain
        let view_to_world = self
            .world_to_view_matrix
            .inverse()
            .unwrap_or_else(|| GfMatrix4d::identity());

        for (i, light) in self.lights.iter().enumerate() {
            // Transform light position into view space (row-vector: pos * M)
            let pos_w = light.get_position();
            let pos_wv = usd_gf::Vec4d::new(
                pos_w.x as f64,
                pos_w.y as f64,
                pos_w.z as f64,
                pos_w.w as f64,
            ) * self.world_to_view_matrix;
            light_sources[i].position = [
                pos_wv.x as f32,
                pos_wv.y as f32,
                pos_wv.z as f32,
                pos_wv.w as f32,
            ];

            let amb = light.get_ambient();
            light_sources[i].ambient = [amb.x, amb.y, amb.z, amb.w];
            let diff = light.get_diffuse();
            light_sources[i].diffuse = [diff.x, diff.y, diff.z, diff.w];
            let spec = light.get_specular();
            light_sources[i].specular = [spec.x, spec.y, spec.z, spec.w];

            // Spot direction: world -> view space via transform_dir (row-vector)
            let sdir_w = light.get_spot_direction();
            let sdir_v = self.world_to_view_matrix.transform_dir(&usd_gf::Vec3d::new(
                sdir_w.x as f64,
                sdir_w.y as f64,
                sdir_w.z as f64,
            ));
            light_sources[i].spot_direction =
                [sdir_v.x as f32, sdir_v.y as f32, sdir_v.z as f32, 0.0];
            light_sources[i].spot_cutoff = light.get_spot_cutoff();
            light_sources[i].spot_falloff = light.get_spot_falloff();

            let att = light.get_attenuation();
            light_sources[i].attenuation = [att.x, att.y, att.z, 0.0];

            // world_to_light = inverse of light transform
            let wtl = light
                .get_transform()
                .inverse()
                .unwrap_or_else(|| GfMatrix4d::identity());
            for r in 0..4 {
                for c in 0..4 {
                    light_sources[i].world_to_light[r * 4 + c] = wtl[r][c] as f32;
                }
            }

            light_sources[i].has_shadow = light.has_shadow() as i32;
            light_sources[i].is_indirect_light = light.is_dome_light() as i32;

            if light.has_shadow() {
                let shadow_start = light.get_shadow_index_start() as usize;
                let shadow_end = light.get_shadow_index_end() as usize;
                light_sources[i].shadow_index_start = shadow_start as i32;
                light_sources[i].shadow_index_end = shadow_end as i32;

                for shadow_idx in shadow_start..=shadow_end {
                    if shadow_idx >= num_shadows {
                        break;
                    }
                    // view_to_shadow = view_to_world * world_to_shadow (biased)
                    let view_to_shadow = if let Some(ref shadows) = self.shadows {
                        view_to_world * shadows.get_world_to_shadow_matrix(shadow_idx)
                    } else {
                        GfMatrix4d::identity()
                    };
                    let shadow_to_view = view_to_shadow
                        .inverse()
                        .unwrap_or_else(|| GfMatrix4d::identity());

                    shadow_matrices[shadow_idx].blur = light.get_shadow_blur();
                    shadow_matrices[shadow_idx].bias = light.get_shadow_bias();
                    for r in 0..4 {
                        for c in 0..4 {
                            shadow_matrices[shadow_idx].view_to_shadow[r * 4 + c] =
                                view_to_shadow[r][c] as f32;
                            shadow_matrices[shadow_idx].shadow_to_view[r * 4 + c] =
                                shadow_to_view[r][c] as f32;
                        }
                    }
                }
            }
        }

        // Pack Lighting UBO
        let light_bytes = unsafe {
            std::slice::from_raw_parts(
                light_sources.as_ptr() as *const u8,
                num_lights * std::mem::size_of::<UboLightSource>(),
            )
        };
        self.lighting_ubo_data.clear();
        self.lighting_ubo_data.extend_from_slice(header_bytes);
        self.lighting_ubo_data.extend_from_slice(light_bytes);
        self.lighting_ubo_valid = true;

        // Pack Shadow UBO
        if num_shadows > 0 {
            let shadow_bytes = unsafe {
                std::slice::from_raw_parts(
                    shadow_matrices.as_ptr() as *const u8,
                    num_shadows * std::mem::size_of::<UboShadowMatrix>(),
                )
            };
            self.shadow_ubo_data.clear();
            self.shadow_ubo_data.extend_from_slice(shadow_bytes);
            self.shadow_ubo_valid = true;
        }
    }

    /// Packs Material UBO from current material + scene ambient.
    fn pack_material_ubo(&mut self) {
        if self.material_ubo_valid {
            return;
        }

        let mat = UboMaterial {
            ambient: {
                let v = self.material.get_ambient();
                [v.x, v.y, v.z, v.w]
            },
            diffuse: {
                let v = self.material.get_diffuse();
                [v.x, v.y, v.z, v.w]
            },
            specular: {
                let v = self.material.get_specular();
                [v.x, v.y, v.z, v.w]
            },
            emission: {
                let v = self.material.get_emission();
                [v.x, v.y, v.z, v.w]
            },
            scene_color: [
                self.scene_ambient.x,
                self.scene_ambient.y,
                self.scene_ambient.z,
                self.scene_ambient.w,
            ],
            shininess: self.material.get_shininess() as f32,
            _padding: [0.0; 3],
        };

        let bytes = unsafe {
            std::slice::from_raw_parts(
                &mat as *const UboMaterial as *const u8,
                std::mem::size_of::<UboMaterial>(),
            )
        };
        self.material_ubo_data.clear();
        self.material_ubo_data.extend_from_slice(bytes);
        self.material_ubo_valid = true;
    }

    // ------------------------------------------------------------------
    // GL bind paths
    // ------------------------------------------------------------------

    /// Binds uniform blocks using the given binding map.
    ///
    /// Builds Lighting / Shadow / Material UBO data on the CPU (matching the
    /// C++ `BindUniformBlocks()` layout) and uploads to GL uniform buffer objects.
    ///
    /// # Parameters
    ///
    /// * `binding_map` - Binding map containing registered uniform block locations.
    #[cfg(feature = "opengl")]
    pub fn bind_uniform_blocks(&mut self, binding_map: &GlfBindingMap) {
        // Refresh CPU-side buffers if dirty.
        self.pack_lighting_and_shadow_ubo();
        self.pack_material_ubo();

        // Upload Lighting UBO.
        if let Some(&binding) = binding_map
            .get_uniform_bindings()
            .get(&TfToken::new("Lighting"))
        {
            Self::upload_ubo(binding as u32, &self.lighting_ubo_data);
        }

        // Upload Shadow UBO (only if shadows exist).
        if !self.shadow_ubo_data.is_empty() {
            if let Some(&binding) = binding_map
                .get_uniform_bindings()
                .get(&TfToken::new("Shadow"))
            {
                Self::upload_ubo(binding as u32, &self.shadow_ubo_data);
            }
        }

        // Upload Material UBO.
        if let Some(&binding) = binding_map
            .get_uniform_bindings()
            .get(&TfToken::new("Material"))
        {
            Self::upload_ubo(binding as u32, &self.material_ubo_data);
        }
    }

    /// Uploads byte slice to a GL uniform buffer object at the given binding point.
    #[cfg(feature = "opengl")]
    fn upload_ubo(binding: u32, data: &[u8]) {
        if data.is_empty() {
            return;
        }
        unsafe {
            let mut ubo: u32 = 0;
            gl::GenBuffers(1, &mut ubo);
            gl::BindBuffer(gl::UNIFORM_BUFFER, ubo);
            gl::BufferData(
                gl::UNIFORM_BUFFER,
                data.len() as gl::types::GLsizeiptr,
                data.as_ptr() as *const _,
                gl::DYNAMIC_DRAW,
            );
            gl::BindBufferBase(gl::UNIFORM_BUFFER, binding, ubo);
            // Note: buffer is orphaned after bind — a real implementation
            // would cache and reuse UBO objects via a resource registry.
            gl::BindBuffer(gl::UNIFORM_BUFFER, 0);
        }
    }

    /// No-op when OpenGL feature is disabled.
    /// Packs UBO data so wgpu consumers can still read the buffers.
    #[cfg(not(feature = "opengl"))]
    pub fn bind_uniform_blocks(&mut self, _binding_map: &GlfBindingMap) {
        self.pack_lighting_and_shadow_ubo();
        self.pack_material_ubo();
    }

    /// Binds shadow map samplers using the given binding map.
    ///
    /// Uses the correct C++ sampler name format: `shadowCompareTextures[i]`.
    ///
    /// # Parameters
    ///
    /// * `binding_map` - Binding map containing registered sampler unit assignments.
    #[cfg(feature = "opengl")]
    pub fn bind_samplers(&self, binding_map: &GlfBindingMap) {
        if let Some(shadows) = &self.shadows {
            let num = shadows.get_num_shadow_map_passes();
            for i in 0..num {
                // C++ name: "shadowCompareTextures[i]"
                let sampler_name = TfToken::new(&format!("{}[{}]", SHADOW_COMPARE_TEXTURES, i));
                if let Some(&unit) = binding_map.get_sampler_bindings().get(&sampler_name) {
                    unsafe {
                        gl::ActiveTexture(gl::TEXTURE0 + unit as u32);
                        gl::BindTexture(gl::TEXTURE_2D, shadows.get_shadow_map_texture(i));
                        // Bind the compare sampler for PCF lookups.
                        gl::BindSampler(unit as u32, shadows.get_shadow_map_compare_sampler());
                    }
                }
            }
            unsafe {
                gl::ActiveTexture(gl::TEXTURE0);
            }
        }
    }

    /// No-op when OpenGL feature is disabled.
    #[cfg(not(feature = "opengl"))]
    pub fn bind_samplers(&self, _binding_map: &GlfBindingMap) {}

    /// Unbinds shadow map samplers using the given binding map.
    ///
    /// Deactivates shadow map textures, restoring sampler units to clean state.
    ///
    /// # Parameters
    ///
    /// * `binding_map` - Binding map containing sampler unit assignments to unbind.
    #[cfg(feature = "opengl")]
    pub fn unbind_samplers(&self, binding_map: &GlfBindingMap) {
        let num_shadows = self.compute_num_shadows_used();
        for i in 0..num_shadows {
            let sampler_name = TfToken::new(&format!("{}[{}]", SHADOW_COMPARE_TEXTURES, i));
            if let Some(&unit) = binding_map.get_sampler_bindings().get(&sampler_name) {
                unsafe {
                    gl::ActiveTexture(gl::TEXTURE0 + unit as u32);
                    gl::BindTexture(gl::TEXTURE_2D, 0);
                    gl::BindSampler(unit as u32, 0);
                }
            }
        }
        unsafe {
            gl::ActiveTexture(gl::TEXTURE0);
        }
    }

    /// No-op when OpenGL feature is disabled.
    #[cfg(not(feature = "opengl"))]
    pub fn unbind_samplers(&self, _binding_map: &GlfBindingMap) {}

    /// Synchronizes lighting context state from current OpenGL context.
    ///
    /// Queries OpenGL state for light positions, colors, materials, and other
    /// lighting parameters, updating this context to match the current GL state.
    /// Useful for interoperability with legacy OpenGL lighting.
    ///
    /// # Stub Implementation
    ///
    /// Currently no-op. Full implementation would call `glGetLight()`, `glGetMaterial()`, etc.
    pub fn set_state_from_opengl(&mut self) {
        // Note: Legacy GL interop requires GL bindings to query glGetLightfv, glGetMaterialfv
        // for GL_LIGHT0..GL_LIGHT7 positions, colors, attenuation. No-op without GL.
    }

    // Post surface lighting

    /// Computes a hash for shader source de-duplication and caching.
    ///
    /// Generates a hash value based on the lighting configuration (number of lights,
    /// shadow usage, material flags). Shaders with identical hashes can share the
    /// same compiled program, reducing shader compilation overhead.
    ///
    /// # Returns
    ///
    /// Hash value representing the current shader generation configuration.
    /// Same configuration will produce the same hash across calls.
    pub fn compute_shader_source_hash(&mut self) -> usize {
        // Simple hash based on lighting configuration
        let mut hash = 0usize;
        hash ^= self.lights.len();
        hash ^= (self.use_lighting as usize) << 1;
        hash ^= (self.use_shadows as usize) << 2;
        hash ^= (self.use_color_material_diffuse as usize) << 3;
        self.shader_source_hash = hash;
        hash
    }

    /// Generates GLSL shader source code for the specified shader stage.
    ///
    /// Produces shader code implementing lighting calculations appropriate for
    /// the current configuration (lights, shadows, materials). The generated code
    /// can be inserted into vertex, fragment, or other shader stages.
    ///
    /// # Parameters
    ///
    /// * `shader_stage_key` - Token identifying the shader stage (e.g., "fragment", "vertex").
    ///
    /// # Returns
    ///
    /// GLSL source code string for the specified stage. Empty string if stage is unsupported.
    pub fn compute_shader_source(&self, shader_stage_key: &TfToken) -> String {
        let stage = shader_stage_key.as_str();
        let num_lights = self.lights.len();
        let num_shadows = self.compute_num_shadows_used();

        if stage == "fragment" {
            let mut src = String::with_capacity(2048);

            // Light structure
            src.push_str("struct LightSource {\n");
            src.push_str("    vec4 position;\n");
            src.push_str("    vec4 ambient;\n");
            src.push_str("    vec4 diffuse;\n");
            src.push_str("    vec4 specular;\n");
            src.push_str("    vec3 spotDirection;\n");
            src.push_str("    float spotCutoff;\n");
            src.push_str("    float spotFalloff;\n");
            src.push_str("    vec3 attenuation;\n");
            src.push_str("    bool hasShadow;\n");
            src.push_str("    int shadowIndex;\n");
            src.push_str("};\n\n");

            // Light uniform array
            src.push_str(&format!(
                "uniform LightSource lights[{}];\n",
                num_lights.max(1)
            ));
            src.push_str(&format!("uniform int numLights = {};\n", num_lights));
            src.push_str("uniform vec4 sceneAmbient;\n\n");

            // Shadow compare samplers — correct name format matching C++
            if self.use_shadows && num_shadows > 0 {
                src.push_str(&format!(
                    "uniform sampler2DShadow {}[{}];\n\n",
                    SHADOW_COMPARE_TEXTURES, num_shadows
                ));
            }

            // Material structure
            src.push_str("struct Material {\n");
            src.push_str("    vec4 ambient;\n");
            src.push_str("    vec4 diffuse;\n");
            src.push_str("    vec4 specular;\n");
            src.push_str("    vec4 emission;\n");
            src.push_str("    float shininess;\n");
            src.push_str("};\n\n");
            src.push_str("uniform Material material;\n\n");

            src
        } else {
            String::new()
        }
    }
}

impl Default for GlfSimpleLightingContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lighting_context_creation() {
        let ctx = GlfSimpleLightingContext::new();
        assert_eq!(ctx.get_num_lights_used(), 0);
        // C++: _useLighting(false) by default
        assert!(!ctx.get_use_lighting());
        assert!(!ctx.get_use_shadows());
    }

    #[test]
    fn test_scene_ambient_default() {
        let ctx = GlfSimpleLightingContext::new();
        // C++: _sceneAmbient(0.01, 0.01, 0.01, 1.0)
        let a = ctx.get_scene_ambient();
        let eps = 1e-6;
        assert!((a.x - 0.01).abs() < eps);
        assert!((a.y - 0.01).abs() < eps);
        assert!((a.z - 0.01).abs() < eps);
        assert!((a.w - 1.0).abs() < eps);
    }

    #[test]
    fn test_lighting_context_lights() {
        let mut ctx = GlfSimpleLightingContext::default();
        let mut light = GlfSimpleLight::default();
        light.set_has_shadow(true);
        light.set_shadow_index_start(0);
        light.set_shadow_index_end(0);

        ctx.set_lights(vec![light]);
        assert_eq!(ctx.get_num_lights_used(), 1);
        // C++: max(shadow_index_end)+1 = 0+1 = 1
        assert_eq!(ctx.compute_num_shadows_used(), 1);
        assert!(ctx.get_use_shadows());
    }

    #[test]
    fn test_compute_num_shadows_cascade() {
        // Light with shadow_index_end=3 means 4 shadow maps (indices 0..3)
        let mut ctx = GlfSimpleLightingContext::default();
        let mut light = GlfSimpleLight::default();
        light.set_has_shadow(true);
        light.set_shadow_index_start(0);
        light.set_shadow_index_end(3);
        ctx.set_lights(vec![light]);
        // C++: ComputeNumShadowsUsed = max(shadow_index_end)+1 = 3+1 = 4
        assert_eq!(ctx.compute_num_shadows_used(), 4);
    }

    #[test]
    fn test_compute_num_shadows_two_lights() {
        // Two lights with different shadow index ranges
        let mut ctx = GlfSimpleLightingContext::default();
        let mut light0 = GlfSimpleLight::default();
        light0.set_has_shadow(true);
        light0.set_shadow_index_start(0);
        light0.set_shadow_index_end(1);

        let mut light1 = GlfSimpleLight::default();
        light1.set_has_shadow(true);
        light1.set_shadow_index_start(2);
        light1.set_shadow_index_end(4);

        ctx.set_lights(vec![light0, light1]);
        // C++: max end index is 4, so num_shadows = 4+1 = 5
        assert_eq!(ctx.compute_num_shadows_used(), 5);
    }

    #[test]
    fn test_compute_num_shadows_no_shadow_lights() {
        let mut ctx = GlfSimpleLightingContext::default();
        let light = GlfSimpleLight::default(); // has_shadow = false
        ctx.set_lights(vec![light]);
        assert_eq!(ctx.compute_num_shadows_used(), 0);
    }

    #[test]
    fn test_lighting_context_material() {
        let mut ctx = GlfSimpleLightingContext::new();
        let mut mat = GlfSimpleMaterial::new();
        mat.set_shininess(64.0);

        ctx.set_material(mat);
        assert_eq!(ctx.get_material().get_shininess(), 64.0);
    }

    #[test]
    fn test_pack_material_ubo_size() {
        // Material UBO must be exactly sizeof(UboMaterial) = 5*16 + 16 = 96 bytes
        let mut ctx = GlfSimpleLightingContext::new();
        ctx.pack_ubo_data();
        assert_eq!(
            ctx.material_ubo_data().len(),
            std::mem::size_of::<UboMaterial>(),
            "Material UBO size mismatch"
        );
    }

    #[test]
    fn test_pack_lighting_ubo_no_lights() {
        // With no lights: header only = sizeof(UboLightingHeader) = 16 bytes
        let mut ctx = GlfSimpleLightingContext::new();
        ctx.pack_ubo_data();
        assert_eq!(
            ctx.lighting_ubo_data().len(),
            std::mem::size_of::<UboLightingHeader>(),
            "Lighting UBO with no lights should be header-only"
        );
        // Shadow UBO should be empty
        assert!(ctx.shadow_ubo_data().is_empty());
    }

    #[test]
    fn test_pack_lighting_ubo_with_lights() {
        // With 2 lights: header + 2 * LightSource
        let mut ctx = GlfSimpleLightingContext::new();
        ctx.set_lights(vec![GlfSimpleLight::default(), GlfSimpleLight::default()]);
        ctx.pack_ubo_data();
        let expected =
            std::mem::size_of::<UboLightingHeader>() + 2 * std::mem::size_of::<UboLightSource>();
        assert_eq!(ctx.lighting_ubo_data().len(), expected);
    }

    #[test]
    fn test_sampler_name_format() {
        // Verify sampler name format matches C++: "shadowCompareTextures[0]"
        let name = format!("{}[{}]", SHADOW_COMPARE_TEXTURES, 0);
        assert_eq!(name, "shadowCompareTextures[0]");
        let name2 = format!("{}[{}]", SHADOW_COMPARE_TEXTURES, 3);
        assert_eq!(name2, "shadowCompareTextures[3]");
    }

    #[test]
    fn test_shader_source_uses_correct_sampler_name() {
        let mut ctx = GlfSimpleLightingContext::new();
        let mut light = GlfSimpleLight::default();
        light.set_has_shadow(true);
        light.set_shadow_index_start(0);
        light.set_shadow_index_end(0);
        ctx.set_lights(vec![light]);
        ctx.set_use_lighting(true);

        let src = ctx.compute_shader_source(&TfToken::new("fragment"));
        assert!(
            src.contains("shadowCompareTextures"),
            "Shader source must use 'shadowCompareTextures', got:\n{}",
            src
        );
        assert!(
            !src.contains("shadowMap"),
            "Old 'shadowMap' name must not appear in shader source"
        );
    }

    #[test]
    fn test_ubo_dirty_flags_reset_on_set_lights() {
        let mut ctx = GlfSimpleLightingContext::new();
        ctx.pack_ubo_data();
        assert!(ctx.lighting_ubo_valid);
        // set_lights should invalidate lighting UBO
        ctx.set_lights(vec![GlfSimpleLight::default()]);
        assert!(!ctx.lighting_ubo_valid);
    }
}
