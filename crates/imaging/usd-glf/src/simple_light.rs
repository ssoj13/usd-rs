//! Simple light structure for GL-based preview rendering.
//!
//! This module provides [`GlfSimpleLight`], a basic light representation used in
//! OpenGL-based preview rendering. It supports point lights, directional lights,
//! spot lights, and dome lights with shadow mapping capabilities.
//!
//! Port of pxr/imaging/glf/simpleLight.h from OpenUSD.

use super::{SdfAssetPath, SdfPath, TfToken, VtArray};
use usd_gf::{Matrix4d, Vec3f, Vec4f};

// Type aliases to match USD naming convention

/// 4x4 double-precision transformation matrix.
pub type GfMatrix4d = Matrix4d;

/// 3-component single-precision vector.
pub type GfVec3f = Vec3f;

/// 4-component single-precision vector (homogeneous coordinates or RGBA).
pub type GfVec4f = Vec4f;

/// Simple light representation for GL-based preview rendering.
///
/// `GlfSimpleLight` encapsulates all parameters needed for basic lighting in
/// OpenGL preview contexts. It supports:
///
/// - **Point lights**: position.w = 1.0
/// - **Directional lights**: position.w = 0.0
/// - **Spot lights**: with direction, cutoff angle, and falloff
/// - **Dome lights**: environment lighting with texture maps
///
/// # Lighting Model
///
/// The light provides standard Phong lighting components:
/// - Ambient: base illumination independent of surface orientation
/// - Diffuse: Lambertian reflection based on surface normal
/// - Specular: glossy highlights based on view direction
///
/// # Shadow Mapping
///
/// Lights can cast shadows using shadow maps. Shadow parameters include:
/// - Resolution: shadow map texture size
/// - Bias: offset to prevent shadow acne
/// - Blur: softness of shadow edges
/// - Matrices: transformations for shadow projection
///
/// # Coordinate Spaces
///
/// Lights can be specified in either world space or camera space.
/// Camera-space lights move with the viewer, useful for headlights or
/// fixed lighting rigs.
///
/// # Post-Surface Lighting
///
/// Supports custom post-surface lighting shaders for advanced effects
/// beyond the standard Phong model.
#[derive(Debug, Clone, PartialEq)]
pub struct GlfSimpleLight {
    /// Light transformation matrix from light space to world/camera space.
    transform: GfMatrix4d,
    /// Ambient light color (RGBA). Base illumination independent of geometry.
    ambient: GfVec4f,
    /// Diffuse light color (RGBA). Lambertian surface reflection component.
    diffuse: GfVec4f,
    /// Specular light color (RGBA). Glossy highlight component.
    specular: GfVec4f,
    /// Light position in homogeneous coordinates.
    /// - w=1.0: point light at (x,y,z)
    /// - w=0.0: directional light from direction (x,y,z)
    position: GfVec4f,
    /// Spot light cone direction vector (normalized).
    spot_direction: GfVec3f,
    /// Spot light cutoff angle in degrees (0-180).
    /// Geometry outside this cone receives no illumination.
    /// Default 180.0 (no spotlight effect).
    spot_cutoff: f32,
    /// Spot light falloff exponent (0.0 = hard edge, higher = softer).
    /// Controls intensity rolloff from center to edge of spotlight cone.
    spot_falloff: f32,
    /// Distance attenuation coefficients (constant, linear, quadratic).
    /// Attenuation factor = 1.0 / (c + l*d + q*d²) where d is distance.
    attenuation: GfVec3f,
    /// Whether this light is defined in camera space vs world space.
    /// Camera-space lights move with the camera (e.g., headlights).
    is_camera_space_light: bool,
    /// Whether this light uses explicit intensity values.
    /// When true, light intensity is explicitly controlled.
    has_intensity: bool,
    /// Projection matrices for shadow map rendering.
    /// Each matrix transforms from world space to shadow map texture space.
    shadow_matrices: Vec<GfMatrix4d>,
    /// Shadow map texture resolution (width and height in pixels).
    /// Higher resolution produces sharper shadows at cost of memory.
    shadow_resolution: i32,
    /// Depth bias to prevent shadow acne artifacts.
    /// Offsets shadow comparison to avoid self-shadowing.
    shadow_bias: f32,
    /// Shadow edge blur/softness factor.
    /// Controls PCF (percentage closer filtering) radius.
    shadow_blur: f32,
    /// Starting index in shadow map array for this light.
    /// Used when multiple lights share a shadow atlas.
    shadow_index_start: i32,
    /// Ending index in shadow map array for this light.
    /// Defines range [shadow_index_start, shadow_index_end) in atlas.
    shadow_index_end: i32,
    /// Whether this light casts shadows.
    /// When true, shadow maps are generated and used for shadow calculations.
    has_shadow: bool,
    /// Unique scene path identifier for this light.
    id: SdfPath,
    /// Whether this is a dome/environment light.
    /// Dome lights provide image-based lighting from a spherical texture.
    is_dome_light: bool,
    /// Asset path to dome light environment texture (HDR image).
    /// Typically a lat-long or cubemap format.
    dome_light_texture_file: SdfAssetPath,
    /// Identifier token for post-surface lighting shader.
    post_surface_identifier: TfToken,
    /// Source code for custom post-surface lighting shader.
    post_surface_shader_source: String,
    /// Binary parameter data for post-surface shader.
    post_surface_shader_params: VtArray<u8>,
}

impl GlfSimpleLight {
    /// Creates a new simple light with the specified position.
    ///
    /// # Arguments
    ///
    /// * `position` - Light position in homogeneous coordinates.
    ///   Use w=1.0 for point lights, w=0.0 for directional lights.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_glf::GlfSimpleLight;
    /// use usd_gf::Vec4f;
    /// // Point light at (5, 10, 15)
    /// let point_light = GlfSimpleLight::new(Vec4f::new(5.0, 10.0, 15.0, 1.0));
    ///
    /// // Directional light from above
    /// let dir_light = GlfSimpleLight::new(Vec4f::new(0.0, -1.0, 0.0, 0.0));
    /// ```
    pub fn new(position: GfVec4f) -> Self {
        Self {
            transform: GfMatrix4d::identity(),
            // C++: _ambient(0.2, 0.2, 0.2, 1.0)
            ambient: GfVec4f::new(0.2, 0.2, 0.2, 1.0),
            diffuse: GfVec4f::new(1.0, 1.0, 1.0, 1.0),
            specular: GfVec4f::new(1.0, 1.0, 1.0, 1.0),
            // C++: _position(position[0], position[1], position[2], 1.0) — forces w=1.0
            position: GfVec4f::new(position.x, position.y, position.z, 1.0),
            spot_direction: GfVec3f::new(0.0, 0.0, -1.0),
            spot_cutoff: 180.0,
            spot_falloff: 0.0,
            attenuation: GfVec3f::new(1.0, 0.0, 0.0),
            is_camera_space_light: false,
            // C++: _hasIntensity(true)
            has_intensity: true,
            // C++: _shadowMatrices(std::vector<GfMatrix4d>(1, GfMatrix4d().SetIdentity()))
            shadow_matrices: vec![GfMatrix4d::identity()],
            shadow_resolution: 512,
            shadow_bias: 0.0,
            shadow_blur: 0.0,
            shadow_index_start: 0,
            shadow_index_end: 0,
            has_shadow: false,
            // C++: _id() — default empty SdfPath
            id: SdfPath::default(),
            is_dome_light: false,
            dome_light_texture_file: SdfAssetPath::new(""),
            post_surface_identifier: TfToken::new(""),
            post_surface_shader_source: String::new(),
            post_surface_shader_params: VtArray::new(),
        }
    }

    // Transform

    /// Returns the light's transformation matrix.
    ///
    /// This matrix transforms from light local space to world or camera space.
    pub fn get_transform(&self) -> &GfMatrix4d {
        &self.transform
    }

    /// Sets the light's transformation matrix.
    ///
    /// # Arguments
    ///
    /// * `mat` - Transformation matrix from light space to world/camera space
    pub fn set_transform(&mut self, mat: GfMatrix4d) {
        self.transform = mat;
    }

    // Ambient

    /// Returns the ambient light color (RGBA).
    ///
    /// Ambient light provides base illumination independent of surface orientation.
    pub fn get_ambient(&self) -> &GfVec4f {
        &self.ambient
    }

    /// Sets the ambient light color.
    ///
    /// # Arguments
    ///
    /// * `ambient` - Ambient color in RGBA format
    pub fn set_ambient(&mut self, ambient: GfVec4f) {
        self.ambient = ambient;
    }

    // Diffuse

    /// Returns the diffuse light color (RGBA).
    ///
    /// Diffuse light provides Lambertian surface reflection based on surface normal.
    pub fn get_diffuse(&self) -> &GfVec4f {
        &self.diffuse
    }

    /// Sets the diffuse light color.
    ///
    /// # Arguments
    ///
    /// * `diffuse` - Diffuse color in RGBA format
    pub fn set_diffuse(&mut self, diffuse: GfVec4f) {
        self.diffuse = diffuse;
    }

    // Specular

    /// Returns the specular light color (RGBA).
    ///
    /// Specular light provides glossy highlights based on view direction.
    pub fn get_specular(&self) -> &GfVec4f {
        &self.specular
    }

    /// Sets the specular light color.
    ///
    /// # Arguments
    ///
    /// * `specular` - Specular color in RGBA format
    pub fn set_specular(&mut self, specular: GfVec4f) {
        self.specular = specular;
    }

    // Position

    /// Returns the light position in homogeneous coordinates.
    ///
    /// * w=1.0 indicates a point light at position (x,y,z)
    /// * w=0.0 indicates a directional light from direction (x,y,z)
    pub fn get_position(&self) -> &GfVec4f {
        &self.position
    }

    /// Sets the light position.
    ///
    /// # Arguments
    ///
    /// * `position` - Position/direction in homogeneous coordinates (x,y,z,w)
    pub fn set_position(&mut self, position: GfVec4f) {
        self.position = position;
    }

    // Spot direction

    /// Returns the spotlight cone direction vector.
    ///
    /// This vector defines the central axis of the spotlight cone.
    /// Should be normalized.
    pub fn get_spot_direction(&self) -> &GfVec3f {
        &self.spot_direction
    }

    /// Sets the spotlight direction.
    ///
    /// # Arguments
    ///
    /// * `direction` - Direction vector (should be normalized)
    pub fn set_spot_direction(&mut self, direction: GfVec3f) {
        self.spot_direction = direction;
    }

    // Spot cutoff

    /// Returns the spotlight cutoff angle in degrees.
    ///
    /// Range: 0-180 degrees. Default 180.0 (no spotlight effect).
    /// Geometry outside this cone receives no light.
    pub fn get_spot_cutoff(&self) -> f32 {
        self.spot_cutoff
    }

    /// Sets the spotlight cutoff angle.
    ///
    /// # Arguments
    ///
    /// * `cutoff` - Cutoff angle in degrees (0-180)
    pub fn set_spot_cutoff(&mut self, cutoff: f32) {
        self.spot_cutoff = cutoff;
    }

    // Spot falloff

    /// Returns the spotlight falloff exponent.
    ///
    /// Controls how intensity drops from center to edge of spotlight cone.
    /// 0.0 = hard edge, higher values = softer rolloff.
    pub fn get_spot_falloff(&self) -> f32 {
        self.spot_falloff
    }

    /// Sets the spotlight falloff exponent.
    ///
    /// # Arguments
    ///
    /// * `falloff` - Falloff exponent (0.0 = hard, higher = softer)
    pub fn set_spot_falloff(&mut self, falloff: f32) {
        self.spot_falloff = falloff;
    }

    // Attenuation

    /// Returns the distance attenuation coefficients (constant, linear, quadratic).
    ///
    /// Attenuation factor = 1.0 / (constant + linear*d + quadratic*d²)
    /// where d is the distance from light to surface.
    pub fn get_attenuation(&self) -> &GfVec3f {
        &self.attenuation
    }

    /// Sets the distance attenuation coefficients.
    ///
    /// # Arguments
    ///
    /// * `attenuation` - Coefficients (constant, linear, quadratic)
    pub fn set_attenuation(&mut self, attenuation: GfVec3f) {
        self.attenuation = attenuation;
    }

    // Shadow matrices

    /// Returns the shadow projection matrices.
    ///
    /// Each matrix transforms from world space to shadow map texture space
    /// for shadow map lookup.
    pub fn get_shadow_matrices(&self) -> &[GfMatrix4d] {
        &self.shadow_matrices
    }

    /// Sets the shadow projection matrices.
    ///
    /// # Arguments
    ///
    /// * `matrices` - Array of projection matrices for shadow mapping
    pub fn set_shadow_matrices(&mut self, matrices: Vec<GfMatrix4d>) {
        self.shadow_matrices = matrices;
    }

    // Shadow resolution

    /// Returns the shadow map resolution in pixels.
    ///
    /// Higher resolution produces sharper shadows but uses more memory.
    /// Default is 512x512.
    pub fn get_shadow_resolution(&self) -> i32 {
        self.shadow_resolution
    }

    /// Sets the shadow map resolution.
    ///
    /// # Arguments
    ///
    /// * `resolution` - Shadow map width and height in pixels
    pub fn set_shadow_resolution(&mut self, resolution: i32) {
        self.shadow_resolution = resolution;
    }

    // Shadow bias

    /// Returns the shadow depth bias.
    ///
    /// Bias prevents shadow acne (self-shadowing artifacts) by offsetting
    /// the depth comparison. Too much bias causes peter-panning.
    pub fn get_shadow_bias(&self) -> f32 {
        self.shadow_bias
    }

    /// Sets the shadow depth bias.
    ///
    /// # Arguments
    ///
    /// * `bias` - Depth offset for shadow comparison
    pub fn set_shadow_bias(&mut self, bias: f32) {
        self.shadow_bias = bias;
    }

    // Shadow blur

    /// Returns the shadow edge blur factor.
    ///
    /// Controls the PCF (percentage closer filtering) radius for soft shadows.
    /// Higher values produce softer shadow edges.
    pub fn get_shadow_blur(&self) -> f32 {
        self.shadow_blur
    }

    /// Sets the shadow blur factor.
    ///
    /// # Arguments
    ///
    /// * `blur` - PCF radius for shadow edge softness
    pub fn set_shadow_blur(&mut self, blur: f32) {
        self.shadow_blur = blur;
    }

    // Shadow index start

    /// Returns the starting index in the shadow map array.
    ///
    /// Used when multiple lights share a shadow atlas texture.
    pub fn get_shadow_index_start(&self) -> i32 {
        self.shadow_index_start
    }

    /// Sets the starting shadow array index.
    ///
    /// # Arguments
    ///
    /// * `start` - Starting index in shadow atlas
    pub fn set_shadow_index_start(&mut self, start: i32) {
        self.shadow_index_start = start;
    }

    // Shadow index end

    /// Returns the ending index in the shadow map array.
    ///
    /// Defines the range [start, end) for this light in the shadow atlas.
    pub fn get_shadow_index_end(&self) -> i32 {
        self.shadow_index_end
    }

    /// Sets the ending shadow array index.
    ///
    /// # Arguments
    ///
    /// * `end` - Ending index (exclusive) in shadow atlas
    pub fn set_shadow_index_end(&mut self, end: i32) {
        self.shadow_index_end = end;
    }

    // Has shadow

    /// Returns whether this light casts shadows.
    ///
    /// When true, shadow maps are generated and used for shadow calculations.
    pub fn has_shadow(&self) -> bool {
        self.has_shadow
    }

    /// Sets whether this light casts shadows.
    ///
    /// # Arguments
    ///
    /// * `has_shadow` - True to enable shadow casting
    pub fn set_has_shadow(&mut self, has_shadow: bool) {
        self.has_shadow = has_shadow;
    }

    // Has intensity

    /// Returns whether this light uses explicit intensity control.
    ///
    /// When true, light brightness is controlled by an intensity value
    /// rather than just color values.
    pub fn has_intensity(&self) -> bool {
        self.has_intensity
    }

    /// Sets whether this light has intensity control.
    ///
    /// # Arguments
    ///
    /// * `has_intensity` - True to enable intensity control
    pub fn set_has_intensity(&mut self, has_intensity: bool) {
        self.has_intensity = has_intensity;
    }

    // Is camera space light

    /// Returns whether this light is defined in camera space.
    ///
    /// Camera-space lights move with the camera (e.g., headlights).
    /// World-space lights remain fixed in the scene.
    pub fn is_camera_space_light(&self) -> bool {
        self.is_camera_space_light
    }

    /// Sets whether this light is in camera space.
    ///
    /// # Arguments
    ///
    /// * `is_camera_space` - True for camera space, false for world space
    pub fn set_is_camera_space_light(&mut self, is_camera_space: bool) {
        self.is_camera_space_light = is_camera_space;
    }

    // ID

    /// Returns the light's unique scene path identifier.
    ///
    /// This path identifies the light within the USD scene graph.
    pub fn get_id(&self) -> &SdfPath {
        &self.id
    }

    /// Sets the light's scene path identifier.
    ///
    /// # Arguments
    ///
    /// * `id` - Scene path for this light
    pub fn set_id(&mut self, id: SdfPath) {
        self.id = id;
    }

    // Is dome light

    /// Returns whether this is a dome/environment light.
    ///
    /// Dome lights provide image-based lighting using a spherical
    /// environment texture (HDR image).
    pub fn is_dome_light(&self) -> bool {
        self.is_dome_light
    }

    /// Sets whether this is a dome light.
    ///
    /// # Arguments
    ///
    /// * `is_dome` - True for dome light, false for standard light
    pub fn set_is_dome_light(&mut self, is_dome: bool) {
        self.is_dome_light = is_dome;
    }

    // Dome light texture file

    /// Returns the dome light environment texture path.
    ///
    /// This is typically an HDR image in lat-long or cubemap format.
    pub fn get_dome_light_texture_file(&self) -> &SdfAssetPath {
        &self.dome_light_texture_file
    }

    /// Sets the dome light texture file path.
    ///
    /// # Arguments
    ///
    /// * `path` - Asset path to HDR environment texture
    pub fn set_dome_light_texture_file(&mut self, path: SdfAssetPath) {
        self.dome_light_texture_file = path;
    }

    // Post surface lighting

    /// Returns the post-surface shader identifier token.
    ///
    /// This identifies which custom shader to use for post-surface lighting effects.
    pub fn get_post_surface_identifier(&self) -> &TfToken {
        &self.post_surface_identifier
    }

    /// Returns the post-surface shader source code.
    ///
    /// Custom shader code for advanced lighting effects beyond standard Phong.
    pub fn get_post_surface_shader_source(&self) -> &str {
        &self.post_surface_shader_source
    }

    /// Returns the post-surface shader parameter data.
    ///
    /// Binary parameter buffer passed to the post-surface shader.
    pub fn get_post_surface_shader_params(&self) -> &VtArray<u8> {
        &self.post_surface_shader_params
    }

    /// Sets all post-surface shader parameters at once.
    ///
    /// Post-surface shaders enable custom lighting effects beyond the
    /// standard Phong model.
    ///
    /// # Arguments
    ///
    /// * `identifier` - Shader identifier token
    /// * `shader_source` - Shader source code
    /// * `shader_params` - Binary parameter buffer
    pub fn set_post_surface_params(
        &mut self,
        identifier: TfToken,
        shader_source: String,
        shader_params: VtArray<u8>,
    ) {
        self.post_surface_identifier = identifier;
        self.post_surface_shader_source = shader_source;
        self.post_surface_shader_params = shader_params;
    }
}

/// Vector of simple lights, matching C++ `GlfSimpleLightVector` typedef.
pub type GlfSimpleLightVector = Vec<GlfSimpleLight>;

impl Default for GlfSimpleLight {
    /// Creates a default simple light at the origin.
    ///
    /// Default configuration:
    /// - Position: (0, 0, 0, 1) - point light at origin
    /// - Diffuse: white (1, 1, 1, 1)
    /// - Specular: white (1, 1, 1, 1)
    /// - Ambient: black (0, 0, 0, 1)
    /// - No shadows
    /// - No spotlight effect
    /// - Constant attenuation only
    fn default() -> Self {
        Self::new(GfVec4f::new(0.0, 0.0, 0.0, 1.0))
    }
}

impl std::fmt::Display for GlfSimpleLight {
    /// Human-readable summary of this light (mirrors C++ `operator<<`).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let pos = self.get_position();
        let diff = self.get_diffuse();
        write!(
            f,
            "GlfSimpleLight {{ pos=({:.3},{:.3},{:.3},{:.3}) diff=({:.3},{:.3},{:.3},{:.3}) \
             has_shadow={} }}",
            pos[0],
            pos[1],
            pos[2],
            pos[3],
            diff[0],
            diff[1],
            diff[2],
            diff[3],
            self.has_shadow(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_light_creation() {
        let light = GlfSimpleLight::new(GfVec4f::new(1.0, 2.0, 3.0, 1.0));
        // C++ forces w=1.0 in constructor
        assert_eq!(light.get_position(), &GfVec4f::new(1.0, 2.0, 3.0, 1.0));
        assert!(!light.has_shadow());
        // C++ defaults
        assert_eq!(light.get_ambient(), &GfVec4f::new(0.2, 0.2, 0.2, 1.0));
        assert!(light.has_intensity());
        assert_eq!(light.get_shadow_matrices().len(), 1);
    }

    #[test]
    fn test_position_w_forced_to_one() {
        // C++: _position(position[0], position[1], position[2], 1.0)
        let light = GlfSimpleLight::new(GfVec4f::new(1.0, 2.0, 3.0, 0.0));
        assert_eq!(light.get_position().w, 1.0);
    }

    #[test]
    fn test_light_properties() {
        let mut light = GlfSimpleLight::default();
        light.set_diffuse(GfVec4f::new(1.0, 0.0, 0.0, 1.0));
        light.set_shadow_resolution(1024);
        light.set_has_shadow(true);

        assert_eq!(light.get_diffuse(), &GfVec4f::new(1.0, 0.0, 0.0, 1.0));
        assert_eq!(light.get_shadow_resolution(), 1024);
        assert!(light.has_shadow());
    }
}
