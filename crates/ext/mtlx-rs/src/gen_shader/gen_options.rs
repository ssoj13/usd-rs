//! GenOptions — shader generation options (по рефу MaterialX GenOptions.h).

/// Shader interface type
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ShaderInterfaceType {
    /// Full interface with uniforms for all editable inputs
    #[default]
    Complete,
    /// Reduced interface — uniforms only for nodedef-declared inputs
    Reduced,
}

/// Method for specular environment lighting
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum HwSpecularEnvironmentMethod {
    #[default]
    Fis,
    Prefilter,
    None,
}

/// Method for directional albedo evaluation
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum HwDirectionalAlbedoMethod {
    #[default]
    Analytic,
    Table,
    MonteCarlo,
}

/// Method for transmission rendering
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum HwTransmissionRenderMethod {
    #[default]
    Refraction,
    Opacity,
}

/// GenOptions — configuration for shader generation
#[derive(Clone, Debug)]
pub struct GenOptions {
    pub shader_interface_type: ShaderInterfaceType,
    pub file_texture_vertical_flip: bool,
    pub add_upstream_dependencies: bool,
    pub library_prefix: String,
    pub emit_color_transforms: bool,
    pub elide_constant_nodes: bool,
    pub target_color_space_override: String,
    pub target_distance_unit: String,
    // HW-specific (по рефу GenOptions.h)
    pub hw_transparency: bool,
    pub hw_specular_environment_method: HwSpecularEnvironmentMethod,
    pub hw_directional_albedo_method: HwDirectionalAlbedoMethod,
    pub hw_transmission_render_method: HwTransmissionRenderMethod,
    pub hw_airy_fresnel_iterations: u32,
    pub hw_srgb_encode_output: bool,
    pub hw_write_depth_moments: bool,
    pub hw_shadow_map: bool,
    pub hw_ambient_occlusion: bool,
    pub hw_max_active_light_sources: u32,
    pub hw_normalize_udim_tex_coords: bool,
    pub hw_write_albedo_table: bool,
    pub hw_write_env_prefilter: bool,
    pub hw_implicit_bitangents: bool,
    // OSL-specific (по рефу GenOptions.h)
    pub osl_implicit_surface_shader_conversion: bool,
    pub osl_connect_ci_wrapper: bool,
}

impl Default for GenOptions {
    fn default() -> Self {
        Self {
            shader_interface_type: ShaderInterfaceType::Complete,
            file_texture_vertical_flip: false,
            add_upstream_dependencies: true,
            library_prefix: "libraries".to_string(),
            emit_color_transforms: true,
            elide_constant_nodes: true,
            target_color_space_override: String::new(),
            target_distance_unit: String::new(),
            hw_transparency: false,
            hw_specular_environment_method: HwSpecularEnvironmentMethod::Fis,
            hw_directional_albedo_method: HwDirectionalAlbedoMethod::Analytic,
            hw_transmission_render_method: HwTransmissionRenderMethod::Refraction,
            hw_airy_fresnel_iterations: 2,
            hw_srgb_encode_output: false,
            hw_write_depth_moments: false,
            hw_shadow_map: false,
            hw_ambient_occlusion: false,
            hw_max_active_light_sources: 3,
            hw_normalize_udim_tex_coords: false,
            hw_write_albedo_table: false,
            hw_write_env_prefilter: false,
            hw_implicit_bitangents: true,
            osl_implicit_surface_shader_conversion: true,
            osl_connect_ci_wrapper: false,
        }
    }
}
