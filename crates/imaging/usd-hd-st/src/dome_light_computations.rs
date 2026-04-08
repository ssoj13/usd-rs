//! Dome light environment map prefiltering computations.
//!
//! GPU compute dispatches that convert an equirectangular (latlong) HDRI
//! into the four textures required for image-based lighting (IBL):
//!
//! 1. **Cubemap** – equirectangular -> 6-face cubemap
//! 2. **Irradiance** – cosine-weighted hemisphere integral (diffuse IBL)
//! 3. **Prefilter** – GGX split-sum specular (one mip level per dispatch)
//! 4. **BRDF LUT** – pre-integrated split-sum BRDF (NdotV x roughness)
//!
//! Ported from `pxr/imaging/hdSt/domeLightComputations.cpp`.
//! GLSL shaders replaced with WGSL in `shaders/dome_light.wgsl`.

use usd_tf::Token;

// ============================================================================
// WGSL shader source (per-kernel, embedded at compile time)
// ============================================================================

/// Reference-only combined source (not compiled directly).
pub const DOME_LIGHT_WGSL: &str = include_str!("shaders/dome_light.wgsl");

/// Shared WGSL helpers prepended to each kernel module.
const WGSL_PREAMBLE: &str = r#"
const PI: f32 = 3.14159265358979323846;

fn cubemap_vec(face: u32, uv: vec2<f32>) -> vec3<f32> {
    let x_mix = uv.x;
    let y_mix = uv.y;
    var dir: vec3<f32>;
    switch face {
        case 0u: { dir = vec3<f32>(1.0, mix(1.0, -1.0, y_mix), mix(1.0, -1.0, x_mix)); }
        case 1u: { dir = vec3<f32>(-1.0, mix(1.0, -1.0, y_mix), mix(-1.0, 1.0, x_mix)); }
        case 2u: { dir = vec3<f32>(mix(-1.0, 1.0, x_mix), 1.0, mix(-1.0, 1.0, y_mix)); }
        case 3u: { dir = vec3<f32>(mix(-1.0, 1.0, x_mix), -1.0, mix(1.0, -1.0, y_mix)); }
        case 4u: { dir = vec3<f32>(mix(-1.0, 1.0, x_mix), mix(1.0, -1.0, y_mix), 1.0); }
        default: { dir = vec3<f32>(mix(1.0, -1.0, x_mix), mix(1.0, -1.0, y_mix), -1.0); }
    }
    return normalize(dir);
}

fn radical_inverse_vdc(bits_in: u32) -> f32 {
    var bits = bits_in;
    bits = (bits << 16u) | (bits >> 16u);
    bits = ((bits & 0x55555555u) << 1u) | ((bits & 0xAAAAAAAAu) >> 1u);
    bits = ((bits & 0x33333333u) << 2u) | ((bits & 0xCCCCCCCCu) >> 2u);
    bits = ((bits & 0x0F0F0F0Fu) << 4u) | ((bits & 0xF0F0F0F0u) >> 4u);
    bits = ((bits & 0x00FF00FFu) << 8u) | ((bits & 0xFF00FF00u) >> 8u);
    return f32(bits) * 2.3283064365386963e-10;
}

fn hammersley(i: u32, n: u32) -> vec2<f32> {
    return vec2<f32>(f32(i) / f32(n), radical_inverse_vdc(i));
}

fn importance_sample_ggx(xi: vec2<f32>, roughness: f32, normal: vec3<f32>) -> vec3<f32> {
    let alpha = roughness * roughness;
    let phi = 2.0 * PI * xi.x;
    let cos_theta = min(1.0, sqrt((1.0 - xi.y) / (1.0 + (alpha * alpha - 1.0) * xi.y)));
    let sin_theta = sqrt(max(0.0, 1.0 - cos_theta * cos_theta));
    let h_tangent = vec3<f32>(sin_theta * cos(phi), sin_theta * sin(phi), cos_theta);
    let up = select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), abs(normal.z) < 0.999);
    let tangent_x = normalize(cross(up, normal));
    let tangent_y = normalize(cross(normal, tangent_x));
    return normalize(tangent_x * h_tangent.x + tangent_y * h_tangent.y + normal * h_tangent.z);
}

fn distribution_ggx(dot_nh: f32, roughness: f32) -> f32 {
    let alpha = roughness * roughness;
    let alpha2 = alpha * alpha;
    let denom = dot_nh * dot_nh * (alpha2 - 1.0) + 1.0;
    return alpha2 / (PI * denom * denom);
}

fn geometry_schlick_smith(dot_nl: f32, dot_nv: f32, roughness: f32) -> f32 {
    let k = (roughness * roughness) * 0.5;
    let g_l = dot_nl / (dot_nl * (1.0 - k) + k);
    let g_v = dot_nv / (dot_nv * (1.0 - k) + k);
    return g_l * g_v;
}

// Direction -> (face, uv) for cubemap array sampling
fn dir_to_face_uv(dir: vec3<f32>) -> vec3<f32> {
    let a = abs(dir);
    var face: f32;
    var uv: vec2<f32>;
    if a.x >= a.y && a.x >= a.z {
        if dir.x > 0.0 { face = 0.0; uv = vec2<f32>(0.5 - 0.5 * dir.z / a.x, 0.5 - 0.5 * dir.y / a.x); }
        else            { face = 1.0; uv = vec2<f32>(0.5 + 0.5 * dir.z / a.x, 0.5 - 0.5 * dir.y / a.x); }
    } else if a.y >= a.x && a.y >= a.z {
        if dir.y > 0.0 { face = 2.0; uv = vec2<f32>(0.5 + 0.5 * dir.x / a.y, 0.5 + 0.5 * dir.z / a.y); }
        else            { face = 3.0; uv = vec2<f32>(0.5 + 0.5 * dir.x / a.y, 0.5 - 0.5 * dir.z / a.y); }
    } else {
        if dir.z > 0.0 { face = 4.0; uv = vec2<f32>(0.5 + 0.5 * dir.x / a.z, 0.5 - 0.5 * dir.y / a.z); }
        else            { face = 5.0; uv = vec2<f32>(0.5 - 0.5 * dir.x / a.z, 0.5 - 0.5 * dir.y / a.z); }
    }
    return vec3<f32>(uv, face);
}

fn sample_cubemap(tex: texture_2d_array<f32>, smp: sampler, dir: vec3<f32>, mip: f32) -> vec4<f32> {
    let fu = dir_to_face_uv(dir);
    return textureSampleLevel(tex, smp, fu.xy, i32(fu.z), mip);
}

fn sanitize(v: vec3<f32>) -> vec3<f32> {
    let x = select(0.0, v.x, v.x == v.x && v.x < 1e38);
    let y = select(0.0, v.y, v.y == v.y && v.y < 1e38);
    let z = select(0.0, v.z, v.z == v.z && v.z < 1e38);
    return vec3<f32>(x, y, z);
}
"#;

/// WGSL source for kernel 1: equirectangular latlong -> 6-face cubemap.
pub const WGSL_LATLONG_TO_CUBEMAP: &str = concat!(
    // Preamble is prepended at runtime
    r#"
@group(0) @binding(0) var src_latlong: texture_2d<f32>;
@group(0) @binding(1) var src_latlong_sampler: sampler;
@group(0) @binding(2) var dst_cubemap: texture_storage_2d_array<rgba16float, write>;

@compute @workgroup_size(8, 8, 1)
fn latlong_to_cubemap(@builtin(global_invocation_id) gid: vec3<u32>) {
    let face_dim = textureDimensions(dst_cubemap);
    if gid.x >= face_dim.x || gid.y >= face_dim.y || gid.z >= 6u { return; }
    let uv = (vec2<f32>(gid.xy) + 0.5) / vec2<f32>(face_dim);
    let dir = cubemap_vec(gid.z, uv);
    let eq_u = (atan2(dir.z, dir.x) + 0.5 * PI) / (2.0 * PI);
    let eq_v = acos(clamp(dir.y, -1.0, 1.0)) / PI;
    let color = textureSampleLevel(src_latlong, src_latlong_sampler, vec2<f32>(eq_u, eq_v), 0.0);
    textureStore(dst_cubemap, vec2<i32>(gid.xy), i32(gid.z), color);
}
"#
);

/// WGSL source for kernel 2: cubemap -> diffuse irradiance cubemap.
pub const WGSL_IRRADIANCE_CONV: &str = concat!(
    r#"
@group(0) @binding(0) var src_env_cube: texture_2d_array<f32>;
@group(0) @binding(1) var src_env_cube_sampler: sampler;
@group(0) @binding(2) var dst_irradiance_cube: texture_storage_2d_array<rgba16float, write>;

const DELTA_PHI: f32   = (2.0 * PI) / 180.0;
const DELTA_THETA: f32 = (0.5 * PI) / 64.0;

@compute @workgroup_size(8, 8, 1)
fn irradiance_conv(@builtin(global_invocation_id) gid: vec3<u32>) {
    let face_dim = textureDimensions(dst_irradiance_cube);
    if gid.x >= face_dim.x || gid.y >= face_dim.y || gid.z >= 6u { return; }

    let uv = (vec2<f32>(gid.xy) + 0.5) / vec2<f32>(face_dim);
    let n = cubemap_vec(gid.z, uv);

    var up: vec3<f32>;
    if gid.z == 2u || gid.z == 3u { up = vec3<f32>(0.0, 0.0, 1.0); }
    else { up = vec3<f32>(0.0, 1.0, 0.0); }
    let right = normalize(cross(up, n));
    up = cross(n, right);

    let in_dims = vec2<f32>(textureDimensions(src_env_cube, 0));
    let mip_level = ceil(log2(4.0 * in_dims.x * DELTA_PHI / (2.0 * PI)) + 2.0);

    var color = vec3<f32>(0.0);
    var sample_count: u32 = 0u;
    var phi = 0.0;
    loop {
        if phi >= 2.0 * PI { break; }
        var theta = 0.0;
        loop {
            if theta >= 0.5 * PI { break; }
            let temp = cos(phi) * right + sin(phi) * up;
            let sample_vec = cos(theta) * n + sin(theta) * temp;
            let raw = sample_cubemap(src_env_cube, src_env_cube_sampler, sample_vec, mip_level).rgb;
            color += sanitize(raw) * cos(theta) * sin(theta);
            sample_count += 1u;
            theta += DELTA_THETA;
        }
        phi += DELTA_PHI;
    }

    let result = PI * color / f32(sample_count);
    textureStore(dst_irradiance_cube, vec2<i32>(gid.xy), i32(gid.z), vec4<f32>(result, 1.0));
}
"#
);

/// WGSL source for kernel 3: cubemap -> GGX specular prefilter (per-roughness dispatch).
pub const WGSL_PREFILTER_GGX: &str = concat!(
    r#"
@group(0) @binding(0) var src_env_cube_spec: texture_2d_array<f32>;
@group(0) @binding(1) var src_env_cube_spec_sampler: sampler;
@group(0) @binding(2) var dst_prefilter: texture_storage_2d_array<rgba16float, write>;

struct PrefilterUniforms { roughness: f32, }
var<push_constant> prefilter_params: PrefilterUniforms;
const NUM_SAMPLES_PREFILTER: u32 = 1024u;

@compute @workgroup_size(8, 8, 1)
fn prefilter_ggx(@builtin(global_invocation_id) gid: vec3<u32>) {
    let face_dim = textureDimensions(dst_prefilter);
    if gid.x >= face_dim.x || gid.y >= face_dim.y || gid.z >= 6u { return; }
    let roughness = prefilter_params.roughness;
    let uv = (vec2<f32>(gid.xy) + 0.5) / vec2<f32>(face_dim);
    let r = cubemap_vec(gid.z, uv);
    let n = r; let v = r;
    var color = vec3<f32>(0.0);
    var total_weight = 0.0;
    let env_map_dim = f32(textureDimensions(src_env_cube_spec, 0).x);
    for (var i = 0u; i < NUM_SAMPLES_PREFILTER; i++) {
        let xi = hammersley(i, NUM_SAMPLES_PREFILTER);
        let h = importance_sample_ggx(xi, roughness, n);
        let l = 2.0 * dot(v, h) * h - v;
        let dot_nl = clamp(dot(n, l), 0.0, 1.0);
        if dot_nl > 0.0 {
            let dot_nh = clamp(dot(n, h), 0.0, 1.0);
            let dot_vh = clamp(dot(v, h), 0.0, 1.0);
            let pdf = distribution_ggx(dot_nh, roughness) * dot_nh / (4.0 * dot_vh) + 0.0001;
            let omega_s = 1.0 / (f32(NUM_SAMPLES_PREFILTER) * pdf);
            let omega_p = 4.0 * PI / (6.0 * env_map_dim * env_map_dim);
            let mip_level = select(max(0.5 * log2(omega_s / omega_p) + 1.0, 0.0), 0.0, roughness == 0.0);
            let raw = sample_cubemap(src_env_cube_spec, src_env_cube_spec_sampler, l, mip_level).rgb;
            color += sanitize(raw) * dot_nl;
            total_weight += dot_nl;
        }
    }
    let result = color / max(total_weight, 0.0001);
    textureStore(dst_prefilter, vec2<i32>(gid.xy), i32(gid.z), vec4<f32>(result, 1.0));
}
"#
);

/// WGSL source for kernel 4: BRDF split-sum integration LUT.
pub const WGSL_BRDF_INTEGRATION: &str = concat!(
    r#"
@group(0) @binding(0) var dst_brdf_lut: texture_storage_2d<rgba16float, write>;

const NUM_SAMPLES_BRDF: u32 = 1024u;

@compute @workgroup_size(8, 8, 1)
fn brdf_integration(@builtin(global_invocation_id) gid: vec3<u32>) {
    let lut_dim = textureDimensions(dst_brdf_lut);
    if gid.x >= lut_dim.x || gid.y >= lut_dim.y { return; }
    let uv = (vec2<f32>(gid.xy) + 0.5) / vec2<f32>(lut_dim);
    let n_dot_v = max(uv.x, 0.001);
    let roughness = uv.y;
    let n = vec3<f32>(0.0, 0.0, 1.0);
    let v = vec3<f32>(sqrt(1.0 - n_dot_v * n_dot_v), 0.0, n_dot_v);
    var lut = vec2<f32>(0.0);
    for (var i = 0u; i < NUM_SAMPLES_BRDF; i++) {
        let xi = hammersley(i, NUM_SAMPLES_BRDF);
        let h = importance_sample_ggx(xi, roughness, n);
        let l = 2.0 * dot(v, h) * h - v;
        let dot_nl = max(dot(n, l), 0.0);
        let dot_nv = max(dot(n, v), 0.0);
        let dot_vh = max(dot(v, h), 0.0);
        let dot_nh = max(dot(h, n), 0.0);
        if dot_nl > 0.0 {
            let g = geometry_schlick_smith(dot_nl, dot_nv, roughness);
            let g_vis = (g * dot_vh) / (dot_nh * dot_nv);
            let fc = pow(1.0 - dot_vh, 5.0);
            lut += vec2<f32>((1.0 - fc) * g_vis, fc * g_vis);
        }
    }
    let result = lut / f32(NUM_SAMPLES_BRDF);
    textureStore(dst_brdf_lut, vec2<i32>(gid.xy), vec4f(result.x, result.y, 0.0, 1.0));
}
"#
);

/// Get the full WGSL source for a kernel (preamble + kernel-specific code).
pub fn wgsl_source(comp_type: DomeLightCompType) -> String {
    let kernel = match comp_type {
        DomeLightCompType::EquirectToCubemap => WGSL_LATLONG_TO_CUBEMAP,
        DomeLightCompType::Irradiance => WGSL_IRRADIANCE_CONV,
        DomeLightCompType::PrefilteredSpecular => WGSL_PREFILTER_GGX,
        DomeLightCompType::BrdfLut => WGSL_BRDF_INTEGRATION,
        DomeLightCompType::Mipmap => return String::new(),
    };
    format!("{WGSL_PREAMBLE}\n{kernel}")
}

// ============================================================================
// Computation type
// ============================================================================

/// Which compute pass a `DomeLightComputationGpu` performs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomeLightCompType {
    /// Convert equirectangular latlong HDRI to a 6-face cubemap.
    EquirectToCubemap,
    /// Convolve cubemap to diffuse irradiance map.
    Irradiance,
    /// GGX importance-sample cubemap for specular prefilter at one roughness.
    PrefilteredSpecular,
    /// Generate mipmaps for the environment cubemap (no shader needed).
    Mipmap,
    /// Pre-integrate split-sum BRDF into a 2D LUT.
    BrdfLut,
}

// ============================================================================
// DomeLightComputationGpu
// ============================================================================

/// One GPU compute pass for dome light prefiltering.
///
/// Mirrors `HdSt_DomeLightComputationGPU` from the C++ reference.
///
/// Multiple computations are chained in this order:
///
/// 1. `EquirectToCubemap`  (latlong -> cubemap, mip 0, allocates texture)
/// 2. `Mipmap`             (generate cubemap mips – no dispatch, API call)
/// 3. `Irradiance`         (cubemap -> irradiance, mip 0, allocates texture)
/// 4. `PrefilteredSpecular` × N  (one per roughness level, mip 0 allocates)
/// 5. `BrdfLut`            (2D LUT, mip 0, allocates texture)
#[derive(Debug)]
pub struct DomeLightComputationGpu {
    /// Identifies which shader / pass this is.
    comp_type: DomeLightCompType,
    /// Token used by the lighting shader to look up the destination texture.
    shader_token: Token,
    /// When true, the source is the already-computed cubemap; otherwise latlong.
    use_cubemap_as_source: bool,
    /// Face dimension of the output cubemap (square face, power of two).
    cubemap_dim: u32,
    /// Total mip levels in the destination texture.
    num_levels: u32,
    /// Mip level this pass writes; level 0 also allocates the texture.
    level: u32,
    /// Roughness for prefiltered specular passes. Negative = not applicable.
    roughness: f32,
}

impl DomeLightComputationGpu {
    /// Create a new dome light GPU computation.
    ///
    /// `shader_token` selects the destination texture from the lighting shader
    /// and also determines `comp_type` via `classify_shader`.
    pub fn new(
        shader_token: Token,
        use_cubemap_as_source: bool,
        cubemap_dim: u32,
        num_levels: u32,
        level: u32,
        roughness: f32,
    ) -> Self {
        let comp_type = classify_shader(&shader_token);
        Self {
            comp_type,
            shader_token,
            use_cubemap_as_source,
            cubemap_dim,
            num_levels,
            level,
            roughness,
        }
    }

    // --- Accessors ---

    pub fn get_comp_type(&self) -> DomeLightCompType {
        self.comp_type
    }
    pub fn get_shader_token(&self) -> &Token {
        &self.shader_token
    }
    pub fn uses_cubemap_source(&self) -> bool {
        self.use_cubemap_as_source
    }
    pub fn get_cubemap_dim(&self) -> u32 {
        self.cubemap_dim
    }
    pub fn get_num_levels(&self) -> u32 {
        self.num_levels
    }
    pub fn get_level(&self) -> u32 {
        self.level
    }
    pub fn get_roughness(&self) -> f32 {
        self.roughness
    }

    /// Returns true if this pass must also allocate the destination texture.
    pub fn allocates_texture(&self) -> bool {
        self.level == 0
    }

    /// Whether this pass needs the roughness push constant.
    pub fn has_uniforms(&self) -> bool {
        self.roughness >= 0.0
    }

    // --- Dispatch geometry ---

    /// Compute workgroup dispatch dimensions for this pass.
    ///
    /// Cubemap passes: `ceil(face_dim/8) x ceil(face_dim/8) x 6`.
    /// BRDF LUT: `ceil(dim/8) x ceil(dim/8) x 1`.
    ///
    /// The C++ reference dispatches `(width * layerCount, height)` as a 2D
    /// flat dispatch; here we use the 3D form that the WGSL shaders expect.
    pub fn get_dispatch_dims(&self) -> (u32, u32, u32) {
        const WG: u32 = 8;
        match self.comp_type {
            DomeLightCompType::BrdfLut => {
                // LUT is a 2D texture (no layers)
                let dim = self.cubemap_dim.max(1);
                let x = (dim + WG - 1) / WG;
                let y = (dim + WG - 1) / WG;
                (x, y, 1)
            }
            DomeLightCompType::Mipmap => {
                // Mipmap is a CPU/API call; no shader dispatch.
                (0, 0, 0)
            }
            _ => {
                // Cubemap face dimension shrinks by 2x per mip level.
                let face_dim = (self.cubemap_dim >> self.level).max(1);
                // Align to local size (matches C++ _MakeMultipleOf).
                let aligned = ((face_dim + WG - 1) / WG) * WG;
                let x = (aligned + WG - 1) / WG;
                let y = (aligned + WG - 1) / WG;
                (x, y, 6)
            }
        }
    }

    /// Entry point name in `dome_light.wgsl` for this computation type.
    pub fn wgsl_entry_point(&self) -> &'static str {
        match self.comp_type {
            DomeLightCompType::EquirectToCubemap => "latlong_to_cubemap",
            DomeLightCompType::Irradiance => "irradiance_conv",
            DomeLightCompType::PrefilteredSpecular => "prefilter_ggx",
            DomeLightCompType::BrdfLut => "brdf_integration",
            DomeLightCompType::Mipmap => "(no shader)",
        }
    }

    /// Binding layout description for this pass.
    ///
    /// Returns `(src_binding, dst_binding, has_sampler, has_push_constant)`.
    pub fn binding_layout(&self) -> BindingLayout {
        match self.comp_type {
            DomeLightCompType::EquirectToCubemap => BindingLayout {
                src_texture: true,
                src_sampler: true,
                dst_texture_array: true,
                dst_texture_2d: false,
                push_constant_roughness: false,
            },
            DomeLightCompType::Irradiance => BindingLayout {
                src_texture: true,
                src_sampler: true,
                dst_texture_array: true,
                dst_texture_2d: false,
                push_constant_roughness: false,
            },
            DomeLightCompType::PrefilteredSpecular => BindingLayout {
                src_texture: true,
                src_sampler: true,
                dst_texture_array: true,
                dst_texture_2d: false,
                push_constant_roughness: true,
            },
            DomeLightCompType::BrdfLut => BindingLayout {
                src_texture: false,
                src_sampler: false,
                dst_texture_array: false,
                dst_texture_2d: true,
                push_constant_roughness: false,
            },
            DomeLightCompType::Mipmap => BindingLayout::default(),
        }
    }
}

/// Describes what resources a compute pass binds.
#[derive(Debug, Default, Clone, Copy)]
pub struct BindingLayout {
    pub src_texture: bool,
    pub src_sampler: bool,
    /// Output is a cubemap (texture_2d_array, 6 layers).
    pub dst_texture_array: bool,
    /// Output is a plain 2D texture (BRDF LUT).
    pub dst_texture_2d: bool,
    /// Roughness is passed as a push constant.
    pub push_constant_roughness: bool,
}

// ============================================================================
// DomeLightMipmapComputationGpu
// ============================================================================

/// Triggers cubemap mipmap generation between the environment conversion
/// and the irradiance/prefilter passes.
///
/// Holds a weak reference to the lighting shader so it can call
/// `generate_mipmaps` on the cubemap texture object at execute time.
/// The actual API call goes to `HdStDynamicCubemapTextureObject::generate_mipmaps`.
#[derive(Debug, Default)]
pub struct DomeLightMipmapComputationGpu;

impl DomeLightMipmapComputationGpu {
    pub fn new() -> Self {
        Self
    }
}

// ============================================================================
// Cubemap width heuristic
// ============================================================================

/// Compute the cubemap face dimension for a given latlong HDRI.
///
/// C++ reference: `HdSt_ComputeDomeLightCubemapWidth`.
///
/// The standard heuristic is `face_dim = texture_width / 4`, then reduced
/// if it would exceed `target_memory_mb`.
///
/// * `tex_width`  – width of the latlong texture in pixels
/// * `tex_height` – height of the latlong texture in pixels (unused in C++ ref)
/// * `target_memory_mb` – GPU memory budget in MiB (0 = no limit)
/// * `bytes_per_texel` – format size (e.g. 8 for rgba16f)
///
/// Returns a positive integer; **not** rounded to a power of two by default
/// (the C++ reference does not do that either). Callers may round if needed.
pub fn compute_cubemap_width(
    tex_width: u32,
    _tex_height: u32,
    target_memory_mb: u32,
    bytes_per_texel: u32,
) -> u32 {
    // Standard width: 1/4 of the equirectangular width.
    let standard_width = (tex_width / 4).max(1);

    if target_memory_mb == 0 {
        return standard_width;
    }

    // Target memory width: sqrt(0.75 * budget / (6 * bytes_per_texel))
    // The 0.75 accounts for all lower mip levels (sum of geometric series).
    const MB: u64 = 1_048_576;
    let budget = target_memory_mb as u64 * MB;
    let target_width = (((0.75 * budget as f64) / (6.0 * bytes_per_texel as f64)).sqrt()) as u32;

    standard_width.min(target_width).max(1)
}

/// Round a positive integer up to the nearest power of two.
pub fn next_pow2(v: u32) -> u32 {
    if v == 0 {
        return 1;
    }
    let mut n = v - 1;
    n |= n >> 1;
    n |= n >> 2;
    n |= n >> 4;
    n |= n >> 8;
    n |= n >> 16;
    n + 1
}

// ============================================================================
// Shader token classifier
// ============================================================================

/// Determine computation type from a shader token name.
///
/// Follows the naming used by `HdStTokens` in the C++ codebase:
/// - `domeLightCubemap`   -> `EquirectToCubemap`
/// - `domeLightIrradiance` -> `Irradiance`
/// - `domeLightPrefilter`  -> `PrefilteredSpecular`
/// - `domeLightBRDF`       -> `BrdfLut`
fn classify_shader(token: &Token) -> DomeLightCompType {
    let s = token.as_str();
    if s.contains("Cubemap") || s.contains("cubemap") || s.contains("Equirect") {
        DomeLightCompType::EquirectToCubemap
    } else if s.contains("Irradiance") || s.contains("irradiance") {
        DomeLightCompType::Irradiance
    } else if s.contains("Prefilter")
        || s.contains("prefilter")
        || s.contains("Specular")
        || s.contains("specular")
    {
        DomeLightCompType::PrefilteredSpecular
    } else if s.contains("BRDF") || s.contains("brdf") || s.contains("Lut") {
        DomeLightCompType::BrdfLut
    } else if s.contains("mipmap") || s.contains("Mipmap") {
        DomeLightCompType::Mipmap
    } else {
        // Default: treat unknown tokens as cubemap conversion.
        DomeLightCompType::EquirectToCubemap
    }
}

// ============================================================================
// WGSL math helpers mirrored in Rust for unit testing
// ============================================================================

/// Van der Corput radical inverse (bit-reversal).
#[inline]
pub fn radical_inverse_vdc(mut bits: u32) -> f32 {
    bits = (bits << 16) | (bits >> 16);
    bits = ((bits & 0x5555_5555) << 1) | ((bits & 0xAAAA_AAAA) >> 1);
    bits = ((bits & 0x3333_3333) << 2) | ((bits & 0xCCCC_CCCC) >> 2);
    bits = ((bits & 0x0F0F_0F0F) << 4) | ((bits & 0xF0F0_F0F0) >> 4);
    bits = ((bits & 0x00FF_00FF) << 8) | ((bits & 0xFF00_FF00) >> 8);
    bits as f32 * 2.328_306_4e-10 // / 2^32
}

/// Hammersley 2D sequence, sample `i` of `n`.
#[inline]
pub fn hammersley(i: u32, n: u32) -> [f32; 2] {
    [i as f32 / n as f32, radical_inverse_vdc(i)]
}

/// Schlick-Smith GGX geometry term (IBL, k = roughness^2 / 2).
#[inline]
pub fn geometry_schlick_smith(dot_nl: f32, dot_nv: f32, roughness: f32) -> f32 {
    let k = roughness * roughness * 0.5;
    let g_l = dot_nl / (dot_nl * (1.0 - k) + k);
    let g_v = dot_nv / (dot_nv * (1.0 - k) + k);
    g_l * g_v
}

/// GGX normal distribution D(N,H,roughness).
#[inline]
pub fn distribution_ggx(dot_nh: f32, roughness: f32) -> f32 {
    use std::f32::consts::PI;
    let alpha = roughness * roughness;
    let alpha2 = alpha * alpha;
    let denom = dot_nh * dot_nh * (alpha2 - 1.0) + 1.0;
    alpha2 / (PI * denom * denom)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // --- classify_shader ---

    #[test]
    fn test_classify_cubemap() {
        let c =
            DomeLightComputationGpu::new(Token::new("domeLightCubemap"), false, 256, 9, 0, -1.0);
        assert_eq!(c.get_comp_type(), DomeLightCompType::EquirectToCubemap);
    }

    #[test]
    fn test_classify_irradiance() {
        let c =
            DomeLightComputationGpu::new(Token::new("domeLightIrradiance"), true, 64, 1, 0, -1.0);
        assert_eq!(c.get_comp_type(), DomeLightCompType::Irradiance);
    }

    #[test]
    fn test_classify_prefilter() {
        let c =
            DomeLightComputationGpu::new(Token::new("domeLightPrefilter"), true, 256, 5, 0, 0.0);
        assert_eq!(c.get_comp_type(), DomeLightCompType::PrefilteredSpecular);
    }

    #[test]
    fn test_classify_brdf() {
        let c = DomeLightComputationGpu::new(Token::new("domeLightBRDF"), false, 256, 1, 0, -1.0);
        assert_eq!(c.get_comp_type(), DomeLightCompType::BrdfLut);
    }

    // --- allocates_texture ---

    #[test]
    fn test_allocates_on_level_zero() {
        let c =
            DomeLightComputationGpu::new(Token::new("domeLightCubemap"), false, 256, 9, 0, -1.0);
        assert!(c.allocates_texture());
        let c2 =
            DomeLightComputationGpu::new(Token::new("domeLightCubemap"), false, 256, 9, 1, -1.0);
        assert!(!c2.allocates_texture());
    }

    // --- dispatch dims ---

    #[test]
    fn test_dispatch_cubemap_256() {
        // face 256, level 0: aligned to 256, wg=8 -> 32x32x6
        let c =
            DomeLightComputationGpu::new(Token::new("domeLightCubemap"), false, 256, 9, 0, -1.0);
        let (x, y, z) = c.get_dispatch_dims();
        assert_eq!(x, 32);
        assert_eq!(y, 32);
        assert_eq!(z, 6);
    }

    #[test]
    fn test_dispatch_irradiance_64() {
        // irradiance face 64, level 0: 8x8x6
        let c =
            DomeLightComputationGpu::new(Token::new("domeLightIrradiance"), true, 64, 1, 0, -1.0);
        let (x, y, z) = c.get_dispatch_dims();
        assert_eq!(x, 8);
        assert_eq!(y, 8);
        assert_eq!(z, 6);
    }

    #[test]
    fn test_dispatch_prefilter_mip2() {
        // prefilter face 256, level 2 -> face_dim = 64: 8x8x6
        let c =
            DomeLightComputationGpu::new(Token::new("domeLightPrefilter"), true, 256, 5, 2, 0.5);
        let (x, y, z) = c.get_dispatch_dims();
        assert_eq!(x, 8);
        assert_eq!(y, 8);
        assert_eq!(z, 6);
    }

    #[test]
    fn test_dispatch_brdf_lut() {
        // BRDF LUT stored as cubemap_dim x cubemap_dim, no layers
        let c = DomeLightComputationGpu::new(Token::new("domeLightBRDF"), false, 256, 1, 0, -1.0);
        let (x, y, z) = c.get_dispatch_dims();
        assert_eq!(x, 32);
        assert_eq!(y, 32);
        assert_eq!(z, 1);
    }

    #[test]
    fn test_dispatch_mipmap_zero() {
        let c = DomeLightComputationGpu::new(Token::new("domeLightMipmap"), false, 256, 9, 0, -1.0);
        assert_eq!(c.get_dispatch_dims(), (0, 0, 0));
    }

    // --- cubemap width heuristic ---

    #[test]
    fn test_compute_cubemap_width_no_limit() {
        // 2048 wide latlong -> 512 face
        let w = compute_cubemap_width(2048, 1024, 0, 8);
        assert_eq!(w, 512);
    }

    #[test]
    fn test_compute_cubemap_width_memory_limited() {
        // Very small budget -> width reduced
        let w = compute_cubemap_width(4096, 2048, 1, 8); // 1 MiB budget
        assert!(w > 0);
        // 1 MiB / (6 * 8) * 0.75 = 16384 -> sqrt = 128
        assert!(w <= 512);
    }

    #[test]
    fn test_next_pow2() {
        assert_eq!(next_pow2(0), 1);
        assert_eq!(next_pow2(1), 1);
        assert_eq!(next_pow2(3), 4);
        assert_eq!(next_pow2(512), 512);
        assert_eq!(next_pow2(513), 1024);
    }

    // --- Rust math helpers (mirror WGSL) ---

    #[test]
    fn test_radical_inverse_known_values() {
        // i=0 -> 0.0
        assert_eq!(radical_inverse_vdc(0), 0.0);
        // i=1 -> 0.5 (bit-reversed 1 = 0x8000_0000 -> /2^32 = 0.5)
        let v1 = radical_inverse_vdc(1);
        assert!((v1 - 0.5).abs() < 1e-7, "expected 0.5, got {v1}");
        // i=2 -> 0.25
        let v2 = radical_inverse_vdc(2);
        assert!((v2 - 0.25).abs() < 1e-7, "expected 0.25, got {v2}");
    }

    #[test]
    fn test_hammersley_sequence() {
        let h0 = hammersley(0, 16);
        assert_eq!(h0[0], 0.0); // i/N = 0
        assert_eq!(h0[1], 0.0); // radical_inverse(0) = 0

        let h1 = hammersley(1, 16);
        assert!((h1[0] - 1.0 / 16.0).abs() < 1e-7);
        assert!((h1[1] - 0.5).abs() < 1e-7);
    }

    #[test]
    fn test_geometry_schlick_smith_perpendicular() {
        // At NdotL=NdotV=1, G=1 regardless of roughness
        let g = geometry_schlick_smith(1.0, 1.0, 0.5);
        assert!((g - 1.0).abs() < 1e-6, "g={g}");
    }

    #[test]
    fn test_geometry_schlick_smith_smooth() {
        // Roughness=0: k=0, G = NdotL/(NdotL) * NdotV/(NdotV) = 1
        let g = geometry_schlick_smith(0.8, 0.6, 0.0);
        assert!((g - 1.0).abs() < 1e-6, "g={g}");
    }

    #[test]
    fn test_distribution_ggx_peak() {
        // At dotNH=1, roughness=0.5:
        //   alpha = 0.25, alpha2 = 0.0625
        //   denom = 1*(0.0625-1)+1 = 0.0625
        //   D = 0.0625 / (PI * 0.0625^2) = 1 / (PI * 0.0625) = 16/PI
        let roughness = 0.5f32;
        let d = distribution_ggx(1.0, roughness);
        let expected = 16.0 / std::f32::consts::PI;
        assert!((d - expected).abs() < 1e-4, "D={d} expected {expected}");
    }

    #[test]
    fn test_distribution_ggx_roughness_one() {
        // At roughness=1, dotNH=1:
        //   alpha=1, alpha2=1
        //   denom = 1*(1-1)+1 = 1
        //   D = 1 / PI
        let d = distribution_ggx(1.0, 1.0);
        let expected = 1.0 / std::f32::consts::PI;
        assert!((d - expected).abs() < 1e-6, "D={d} expected {expected}");
    }

    #[test]
    fn test_distribution_ggx_glancing() {
        // At dotNH=0 (90 degrees), D should drop significantly vs dotNH=1
        let d_peak = distribution_ggx(1.0, 0.5);
        let d_glancing = distribution_ggx(0.0, 0.5);
        // alpha^2=0.0625, denom=(0)*(0.0625-1)+1 = 1 -> D = 0.0625/PI ~= 0.02
        let expected_glancing = 0.0625 / std::f32::consts::PI;
        assert!(
            (d_glancing - expected_glancing).abs() < 1e-5,
            "d_glancing={d_glancing} expected {expected_glancing}"
        );
        assert!(
            d_peak > d_glancing,
            "peak should be higher than glancing angle"
        );
    }

    #[test]
    fn test_wgsl_embedded() {
        // Verify the per-kernel WGSL sources contain expected entry points
        let src1 = wgsl_source(DomeLightCompType::EquirectToCubemap);
        assert!(src1.contains("latlong_to_cubemap"));
        assert!(src1.contains("radical_inverse_vdc")); // preamble
        let src2 = wgsl_source(DomeLightCompType::Irradiance);
        assert!(src2.contains("irradiance_conv"));
        let src3 = wgsl_source(DomeLightCompType::PrefilteredSpecular);
        assert!(src3.contains("prefilter_ggx"));
        assert!(src3.contains("importance_sample_ggx"));
        let src4 = wgsl_source(DomeLightCompType::BrdfLut);
        assert!(src4.contains("brdf_integration"));
    }

    #[test]
    fn test_entry_points() {
        let names = [
            ("domeLightCubemap", "latlong_to_cubemap"),
            ("domeLightIrradiance", "irradiance_conv"),
            ("domeLightPrefilter", "prefilter_ggx"),
            ("domeLightBRDF", "brdf_integration"),
        ];
        for (token, expected) in &names {
            let c = DomeLightComputationGpu::new(Token::new(token), false, 64, 1, 0, -1.0);
            assert_eq!(c.wgsl_entry_point(), *expected, "token={token}");
        }
    }

    #[test]
    fn test_binding_layout_brdf_has_no_src() {
        let c = DomeLightComputationGpu::new(Token::new("domeLightBRDF"), false, 256, 1, 0, -1.0);
        let bl = c.binding_layout();
        assert!(!bl.src_texture);
        assert!(!bl.src_sampler);
        assert!(bl.dst_texture_2d);
        assert!(!bl.dst_texture_array);
        assert!(!bl.push_constant_roughness);
    }

    #[test]
    fn test_binding_layout_prefilter_has_push_constant() {
        let c =
            DomeLightComputationGpu::new(Token::new("domeLightPrefilter"), true, 256, 5, 0, 0.3);
        let bl = c.binding_layout();
        assert!(bl.src_texture);
        assert!(bl.dst_texture_array);
        assert!(bl.push_constant_roughness);
    }
}
