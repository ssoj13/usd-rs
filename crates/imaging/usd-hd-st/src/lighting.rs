//! Lighting system for Storm wgpu pipeline.
//!
//! Provides GPU-ready light uniform data and WGSL code snippets for
//! multi-light evaluation in the fragment shader.

use crate::light::HdStLight;
use std::fmt::Write;
use usd_gf::Matrix4d;

/// Maximum number of lights supported in the uniform array.
pub const MAX_LIGHTS: usize = 16;

/// Size in bytes of a single LightGpuData entry (matches WGSL struct layout).
/// 5 * vec4 (params) + 1 * mat4x4 (worldToLight) = 5*16 + 64 = 144 bytes.
pub const LIGHT_ENTRY_SIZE: usize = 144;

/// Total size of the LightUniforms buffer:
/// header: light_count(u32) + pad(3*u32) = 16 bytes
/// lights: MAX_LIGHTS * 144 = 2304 bytes
/// Total = 2320 bytes
pub const LIGHT_UNIFORMS_SIZE: usize = 16 + MAX_LIGHTS * LIGHT_ENTRY_SIZE;

/// Light type discriminant for GPU.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuLightType {
    Directional = 0,
    Point = 1,
    Spot = 2,
    Dome = 3,
}

/// Shadow parameters for a single light source.
///
/// Matches C++ HdxShadowParams / simpleLightingShader shadow binding layout.
/// Shadow atlas + depth pass + PCF sampling are in `crate::shadow`.
#[derive(Debug, Clone, Copy)]
pub struct ShadowParams {
    /// Whether shadows are enabled for this light
    pub enabled: bool,
    /// Shadow blur radius (PCF kernel size, 0 = hard shadows)
    pub blur: f32,
    /// Depth bias to prevent shadow acne
    pub bias: f32,
    /// World-to-shadow-clip transform (light VP matrix)
    pub matrix: [f32; 16],
}

impl Default for ShadowParams {
    fn default() -> Self {
        Self {
            enabled: false,
            blur: 0.0,
            bias: 0.001,
            matrix: MAT4_IDENTITY,
        }
    }
}

/// GPU-ready light data (matches WGSL LightEntry struct).
/// Uses vec4 everywhere to avoid WGSL vec3 alignment pitfalls.
/// Layout: 5 * vec4 + mat4x4 = 144 bytes.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct LightGpuData {
    /// vec4(light_type, has_shadow, shadow_index_start, shadow_index_end)
    /// Matches C++ LightSource fields: hasShadow, shadowIndexStart, shadowIndexEnd.
    /// has_shadow: 1.0 = casts shadows, 0.0 = no shadows.
    /// shadow_index_start/end: range [start, end] in shadow atlas (cascade support).
    pub type_pad: [f32; 4],
    /// World-space position (point/spot) or unused (directional)
    pub position: [f32; 4],
    /// World-space direction (directional/spot) or unused (point)
    pub direction: [f32; 4],
    /// Light color (linear RGB, .w unused)
    pub color: [f32; 4],
    /// vec4(intensity, radius, inner_angle, outer_angle)
    pub params: [f32; 4],
    /// World-to-light transform matrix (for dome light IBL orientation).
    /// Identity for non-dome lights. Column-major, matching WGSL mat4x4<f32>.
    pub world_to_light_transform: [f32; 16],
}

/// Identity mat4x4 in column-major order (WGSL convention).
const MAT4_IDENTITY: [f32; 16] = [
    1.0, 0.0, 0.0, 0.0, // col 0
    0.0, 1.0, 0.0, 0.0, // col 1
    0.0, 0.0, 1.0, 0.0, // col 2
    0.0, 0.0, 0.0, 1.0, // col 3
];

/// Convert Matrix4d to [f32; 16] in row-major linear layout.
///
/// Matches C++ simpleLightingContext.cpp `setMatrix()`: dst[i*4+j] = mat[i][j].
/// WGSL mat4x4 reads columns from the linear array, so row 0 -> column 0 (transpose).
/// This is correct for GfMatrix4d's row-vector convention.
fn mat4d_to_f32_row_major(m: &Matrix4d) -> [f32; 16] {
    let mut out = [0.0f32; 16];
    for i in 0..4 {
        for j in 0..4 {
            out[i * 4 + j] = m[i][j] as f32;
        }
    }
    out
}

impl Default for LightGpuData {
    fn default() -> Self {
        Self {
            type_pad: [GpuLightType::Directional as u32 as f32, 0.0, 0.0, 0.0],
            position: [0.0, 0.0, 0.0, 1.0],
            direction: [0.577, 0.577, 0.577, 0.0],
            color: [1.0, 1.0, 1.0, 1.0],
            params: [1.0, 0.0, 0.0, std::f32::consts::FRAC_PI_4],
            world_to_light_transform: MAT4_IDENTITY,
        }
    }
}

/// Extract LightGpuData from an HdStLight.
///
/// Reads from the synced GlfSimpleLight cached in params.
/// Falls back to a default directional light if not yet synced.
pub fn light_from_hd(light: &HdStLight) -> LightGpuData {
    let type_str = light.get_light_type().as_str();
    let light_type = match type_str {
        "distantLight" => GpuLightType::Directional,
        "sphereLight" => GpuLightType::Point,
        "rectLight" | "diskLight" | "cylinderLight" => GpuLightType::Point,
        "domeLight" => GpuLightType::Dome,
        _ => GpuLightType::Point,
    };

    // Prefer reading from the synced GlfSimpleLight (position/direction already set)
    let Some(glf) = light.get_simple_light() else {
        return default_light();
    };

    // Position is already world-space in GlfSimpleLight after sync
    let pos = glf.get_position();
    let position = [pos.x, pos.y, pos.z, pos.w];

    // Direction: for directional/spot use spot_direction; others unused
    let sd = glf.get_spot_direction();
    let direction = match light_type {
        GpuLightType::Directional | GpuLightType::Spot => [sd.x, sd.y, sd.z, 0.0],
        _ => [0.0, 0.0, 1.0, 0.0],
    };

    // Color from diffuse channel (already includes intensity from Sync)
    let d = glf.get_diffuse();
    let color = [d.x, d.y, d.z, 1.0];

    // Effective intensity = luminance of diffuse color (already scaled)
    let intensity = (d.x * 0.2126 + d.y * 0.7152 + d.z * 0.0722).max(0.0);

    // Spot cone angles from GlfSimpleLight shaping params
    let outer_angle = glf.get_spot_cutoff().to_radians();
    let falloff = glf.get_spot_falloff();

    // Dome light orientation: worldToLightTransform = light.GetTransform().GetInverse()
    // Per C++ simpleLightingContext.cpp:345-346.
    let world_to_light_transform = if light_type == GpuLightType::Dome {
        let xform = glf.get_transform();
        match xform.inverse() {
            Some(inv) => mat4d_to_f32_row_major(&inv),
            None => MAT4_IDENTITY,
        }
    } else {
        MAT4_IDENTITY
    };

    // Shadow fields from GlfSimpleLight (C++ LightSource: hasShadow, shadowIndexStart, shadowIndexEnd)
    let has_shadow = if glf.has_shadow() { 1.0f32 } else { 0.0 };
    let shadow_index_start = glf.get_shadow_index_start() as f32;
    let shadow_index_end = glf.get_shadow_index_end() as f32;

    LightGpuData {
        type_pad: [
            light_type as u32 as f32,
            has_shadow,
            shadow_index_start,
            shadow_index_end,
        ],
        position,
        direction,
        color,
        params: [intensity, 0.0, falloff, outer_angle],
        world_to_light_transform,
    }
}

/// Build a default directional light (fallback when no scene lights exist).
pub fn default_light() -> LightGpuData {
    LightGpuData {
        type_pad: [GpuLightType::Directional as u32 as f32, 0.0, 0.0, 0.0],
        position: [0.0, 0.0, 0.0, 1.0],
        direction: [0.577, 0.577, 0.577, 0.0],
        color: [1.0, 1.0, 1.0, 1.0],
        params: [1.0, 0.0, 0.0, std::f32::consts::FRAC_PI_4],
        world_to_light_transform: MAT4_IDENTITY,
    }
}

/// 3-point light setup for scenes without explicit lights.
/// Key (warm, upper-right), fill (cool, left), rim (back).
/// Matches typical DCC viewport lighting (Maya, Blender).
pub fn default_lights() -> Vec<LightGpuData> {
    vec![
        // Key light: warm white, upper-right-front, intensity 1.2
        LightGpuData {
            type_pad: [GpuLightType::Directional as u32 as f32, 0.0, 0.0, 0.0],
            position: [0.0, 0.0, 0.0, 1.0],
            direction: [0.5, 0.7, 0.5, 0.0],
            color: [1.0, 0.95, 0.9, 1.0],
            params: [1.2, 0.0, 0.0, std::f32::consts::FRAC_PI_4],
            world_to_light_transform: MAT4_IDENTITY,
        },
        // Fill light: cool blue, left, intensity 0.4
        LightGpuData {
            type_pad: [GpuLightType::Directional as u32 as f32, 0.0, 0.0, 0.0],
            position: [0.0, 0.0, 0.0, 1.0],
            direction: [-0.7, 0.3, 0.6, 0.0],
            color: [0.85, 0.9, 1.0, 1.0],
            params: [0.4, 0.0, 0.0, std::f32::consts::FRAC_PI_4],
            world_to_light_transform: MAT4_IDENTITY,
        },
        // Rim/back light: white, behind, intensity 0.6
        LightGpuData {
            type_pad: [GpuLightType::Directional as u32 as f32, 0.0, 0.0, 0.0],
            position: [0.0, 0.0, 0.0, 1.0],
            direction: [0.0, 0.4, -0.9, 0.0],
            color: [1.0, 1.0, 1.0, 1.0],
            params: [0.6, 0.0, 0.0, std::f32::consts::FRAC_PI_4],
            world_to_light_transform: MAT4_IDENTITY,
        },
    ]
}

/// Build the LightUniforms byte buffer from a slice of lights.
///
/// Layout (matches WGSL LightUniforms struct):
/// - light_count: u32 (4 bytes)
/// - _pad: 3 * u32 (12 bytes)
/// - lights: array<LightEntry, MAX_LIGHTS> (MAX_LIGHTS * 80 bytes)
pub fn build_light_uniforms(lights: &[LightGpuData]) -> Vec<u8> {
    let count = lights.len().min(MAX_LIGHTS) as u32;
    let mut data = Vec::with_capacity(LIGHT_UNIFORMS_SIZE);

    // light_count + padding to 16-byte alignment
    data.extend_from_slice(&count.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes()); // pad
    data.extend_from_slice(&0u32.to_le_bytes()); // pad
    data.extend_from_slice(&0u32.to_le_bytes()); // pad

    // Write each light entry
    for i in 0..MAX_LIGHTS {
        if i < lights.len() {
            write_light_entry(&mut data, &lights[i]);
        } else {
            // Zero-fill unused slots
            data.extend_from_slice(&[0u8; LIGHT_ENTRY_SIZE]);
        }
    }

    debug_assert_eq!(data.len(), LIGHT_UNIFORMS_SIZE);
    data
}

/// Build a combined LightUniforms + ShadowUniforms byte buffer.
///
/// When `shadow_entries` is non-empty, shadow data is appended after the light
/// array at the same group/binding (group 1, binding 0). The WGSL struct
/// conditionally includes the shadow fields when `use_shadows` is true.
///
/// Layout: header(16) + lights(MAX_LIGHTS*144) + shadows(MAX_SHADOWS*144)
pub fn build_light_and_shadow_uniforms(
    lights: &[LightGpuData],
    shadow_entries: &[crate::shadow::ShadowEntry],
) -> Vec<u8> {
    let mut data = build_light_uniforms(lights);
    // Append shadow entries
    let shadow_bytes = crate::shadow::build_shadow_uniforms(shadow_entries);
    data.extend_from_slice(&shadow_bytes);
    data
}

/// Extract lights from a collection of HdStLight references.
///
/// Only includes lights that have been synced and have intensity.
pub fn extract_lights(lights: &[&HdStLight]) -> Vec<LightGpuData> {
    lights
        .iter()
        .filter(|l| {
            l.get_simple_light()
                .map(|g| g.has_intensity())
                .unwrap_or(false)
        })
        .map(|l| light_from_hd(l))
        .take(MAX_LIGHTS)
        .collect()
}

/// Write a single LightGpuData entry as bytes (5 * vec4 + mat4x4 = 144 bytes).
fn write_light_entry(data: &mut Vec<u8>, light: &LightGpuData) {
    // 5 vec4 fields
    for arr in [
        &light.type_pad,
        &light.position,
        &light.direction,
        &light.color,
        &light.params,
    ] {
        for v in arr {
            data.extend_from_slice(&v.to_le_bytes());
        }
    }
    // mat4x4 world_to_light_transform (16 floats = 64 bytes)
    for v in &light.world_to_light_transform {
        data.extend_from_slice(&v.to_le_bytes());
    }
}

// --- WGSL code snippets for light evaluation ---

/// Generate the WGSL LightEntry struct and LightUniforms declaration.
pub fn emit_light_uniforms_wgsl(src: &mut String, group: u32, binding: u32) {
    emit_light_uniforms_wgsl_with_shadows(src, group, binding, false);
}

/// Emit the LightUniforms WGSL struct, optionally including shadow data.
///
/// When `use_shadows` is true, the struct also contains the ShadowEntry array
/// packed after the light array in the same UBO (group 1, binding 0).
/// This avoids a separate bind group for shadow UBO data.
pub fn emit_light_uniforms_wgsl_with_shadows(
    src: &mut String,
    group: u32,
    binding: u32,
    use_shadows: bool,
) {
    writeln!(
        src,
        "// Light uniform data (all vec4 to avoid WGSL vec3 alignment issues)"
    )
    .unwrap();
    writeln!(src, "struct LightEntry {{").unwrap();
    writeln!(src, "    type_pad: vec4<f32>,").unwrap();
    writeln!(src, "    position: vec4<f32>,").unwrap();
    writeln!(src, "    direction: vec4<f32>,").unwrap();
    writeln!(src, "    color: vec4<f32>,").unwrap();
    writeln!(src, "    params: vec4<f32>,").unwrap();
    // mat4x4 for dome light world-to-light orientation transform (matches C++ worldToLightTransform)
    writeln!(src, "    world_to_light: mat4x4<f32>,").unwrap();
    writeln!(src, "}};").unwrap();
    writeln!(src).unwrap();

    // ShadowEntry struct + shadow array in LightUniforms when use_shadows
    let max_shadows = crate::shadow::MAX_SHADOWS;
    if use_shadows {
        writeln!(src, "struct ShadowEntry {{").unwrap();
        writeln!(src, "    world_to_shadow: mat4x4<f32>,").unwrap();
        writeln!(src, "    shadow_to_world: mat4x4<f32>,").unwrap();
        writeln!(src, "    params: vec4<f32>,  // blur, bias, pad, pad").unwrap();
        writeln!(src, "}};").unwrap();
        writeln!(src).unwrap();
    }

    writeln!(src, "struct LightUniforms {{").unwrap();
    writeln!(src, "    light_count: vec4<u32>,").unwrap();
    writeln!(src, "    lights: array<LightEntry, {}>,", MAX_LIGHTS).unwrap();
    if use_shadows {
        writeln!(src, "    shadows: array<ShadowEntry, {}>,", max_shadows).unwrap();
    }
    writeln!(src, "}};").unwrap();
    writeln!(src, "@group({group}) @binding({binding})").unwrap();
    writeln!(src, "var<uniform> light_data: LightUniforms;").unwrap();
    writeln!(src).unwrap();
}

/// Generate WGSL helper functions for light evaluation.
pub fn emit_light_eval_functions(src: &mut String) {
    // Directional light evaluation
    writeln!(src, "// Evaluate a directional light contribution").unwrap();
    writeln!(src, "fn eval_directional(light: LightEntry, N: vec3<f32>, V: vec3<f32>, base_color: vec3<f32>, roughness: f32, metallic: f32, f0: vec3<f32>, f90: vec3<f32>) -> vec3<f32> {{").unwrap();
    // direction = emit direction (from light); negate for L (towards light)
    writeln!(src, "    let L = normalize(-light.direction.xyz);").unwrap();
    writeln!(src, "    let H = normalize(L + V);").unwrap();
    writeln!(src, "    let n_dot_l = max(dot(N, L), 0.0);").unwrap();
    writeln!(src, "    let n_dot_h = max(dot(N, H), 0.0);").unwrap();
    writeln!(src, "    let v_dot_h = max(dot(V, H), 0.0);").unwrap();
    writeln!(src, "    let radiance = light.color.xyz * light.params.x;").unwrap();
    // C++ evaluateLight line 237: specularRoughness = max(0.001, specularRoughness)
    writeln!(src, "    let sr = max(roughness, 0.001);").unwrap();
    // C++ evaluateLight: d = diffuseColor / PI; if (!specWf) d *= 1-metallic
    writeln!(src, "    var d = base_color / 3.14159265;").unwrap();
    writeln!(src, "    if (material.use_specular_workflow < 0.5) {{").unwrap();
    writeln!(src, "        d = d * (1.0 - metallic);").unwrap();
    writeln!(src, "    }}").unwrap();
    writeln!(src, "    let alpha = sr * sr;").unwrap();
    writeln!(src, "    let n_dot_v = max(dot(N, V), 0.001);").unwrap();
    // C++ SchlickFresnel: scalar weight, then mix(F0, F90, fresnel)
    writeln!(
        src,
        "    let fresnel_w = pow(max(1.0 - v_dot_h, 0.0), 5.0);"
    )
    .unwrap();
    writeln!(src, "    let F = mix(f0, f90, fresnel_w);").unwrap();
    writeln!(src, "    let D = ggx_distribution(n_dot_h, alpha);").unwrap();
    writeln!(src, "    let G = smith_ggx(n_dot_v, n_dot_l, sr);").unwrap();
    writeln!(
        src,
        "    let spec = (D * G * F) / (4.0 * n_dot_v * n_dot_l + 0.001);"
    )
    .unwrap();
    // C++ line 269: d *= (1.0 - mix(F0, F90, fresnel))
    writeln!(
        src,
        "    d = d * (vec3<f32>(1.0) - mix(f0, f90, fresnel_w));"
    )
    .unwrap();
    writeln!(src, "    return (d + spec) * radiance * n_dot_l;").unwrap();
    writeln!(src, "}}").unwrap();
    writeln!(src).unwrap();

    // Point light evaluation
    writeln!(src, "// Evaluate a point light contribution").unwrap();
    writeln!(src, "fn eval_point_light(light: LightEntry, world_pos: vec3<f32>, N: vec3<f32>, V: vec3<f32>, base_color: vec3<f32>, roughness: f32, metallic: f32, f0: vec3<f32>, f90: vec3<f32>) -> vec3<f32> {{").unwrap();
    writeln!(src, "    let to_light = light.position.xyz - world_pos;").unwrap();
    writeln!(src, "    let dist = length(to_light);").unwrap();
    writeln!(src, "    let L = to_light / max(dist, 0.001);").unwrap();
    writeln!(src, "    let H = normalize(L + V);").unwrap();
    writeln!(src, "    let n_dot_l = max(dot(N, L), 0.0);").unwrap();
    writeln!(src, "    let n_dot_h = max(dot(N, H), 0.0);").unwrap();
    writeln!(src, "    let v_dot_h = max(dot(V, H), 0.0);").unwrap();
    writeln!(
        src,
        "    // Inverse-square attenuation with optional radius cutoff"
    )
    .unwrap();
    writeln!(src, "    var atten = 1.0 / max(dist * dist, 0.0001);").unwrap();
    writeln!(src, "    if (light.params.y > 0.0) {{").unwrap();
    writeln!(
        src,
        "        atten *= max(1.0 - dist / light.params.y, 0.0);"
    )
    .unwrap();
    writeln!(src, "    }}").unwrap();
    writeln!(
        src,
        "    let radiance = light.color.xyz * light.params.x * atten;"
    )
    .unwrap();
    // C++ evaluateLight line 237: specularRoughness = max(0.001, specularRoughness)
    writeln!(src, "    let sr = max(roughness, 0.001);").unwrap();
    // C++ evaluateLight: d = diffuseColor / PI; if (!specWf) d *= 1-metallic
    writeln!(src, "    var d = base_color / 3.14159265;").unwrap();
    writeln!(src, "    if (material.use_specular_workflow < 0.5) {{").unwrap();
    writeln!(src, "        d = d * (1.0 - metallic);").unwrap();
    writeln!(src, "    }}").unwrap();
    writeln!(src, "    let alpha = sr * sr;").unwrap();
    writeln!(src, "    let n_dot_v = max(dot(N, V), 0.001);").unwrap();
    // C++ SchlickFresnel: scalar weight, then mix(F0, F90, fresnel)
    writeln!(
        src,
        "    let fresnel_w = pow(max(1.0 - v_dot_h, 0.0), 5.0);"
    )
    .unwrap();
    writeln!(src, "    let F = mix(f0, f90, fresnel_w);").unwrap();
    writeln!(src, "    let D = ggx_distribution(n_dot_h, alpha);").unwrap();
    writeln!(src, "    let G = smith_ggx(n_dot_v, n_dot_l, sr);").unwrap();
    writeln!(
        src,
        "    let spec = (D * G * F) / (4.0 * n_dot_v * n_dot_l + 0.001);"
    )
    .unwrap();
    // C++ line 269: d *= (1.0 - mix(F0, F90, fresnel))
    writeln!(
        src,
        "    d = d * (vec3<f32>(1.0) - mix(f0, f90, fresnel_w));"
    )
    .unwrap();
    writeln!(src, "    return (d + spec) * radiance * n_dot_l;").unwrap();
    writeln!(src, "}}").unwrap();
    writeln!(src).unwrap();

    // Spot light evaluation
    writeln!(src, "// Evaluate a spot light contribution").unwrap();
    writeln!(src, "fn eval_spot_light(light: LightEntry, world_pos: vec3<f32>, N: vec3<f32>, V: vec3<f32>, base_color: vec3<f32>, roughness: f32, metallic: f32, f0: vec3<f32>, f90: vec3<f32>) -> vec3<f32> {{").unwrap();
    writeln!(src, "    let to_light = light.position.xyz - world_pos;").unwrap();
    writeln!(src, "    let dist = length(to_light);").unwrap();
    writeln!(src, "    let L = to_light / max(dist, 0.001);").unwrap();
    writeln!(
        src,
        "    // Spot cone: hard cutoff at outer_angle, pow(cosDir, falloff) decay."
    )
    .unwrap();
    writeln!(
        src,
        "    // params.z = falloff exponent, params.w = outer cone angle (radians)."
    )
    .unwrap();
    writeln!(
        src,
        "    let cos_dir = dot(-L, normalize(light.direction.xyz));"
    )
    .unwrap();
    writeln!(src, "    let cos_cutoff = cos(light.params.w);").unwrap();
    // C++ simpleLighting.glslfx: spotAtten = (cosLight < cos(cutoff)) ? 0 : pow(cosLight, falloff)
    writeln!(src, "    var spot_factor = 0.0;").unwrap();
    writeln!(src, "    if (cos_dir >= cos_cutoff) {{").unwrap();
    writeln!(
        src,
        "        spot_factor = pow(max(cos_dir, 0.0), max(light.params.z, 0.001));"
    )
    .unwrap();
    writeln!(src, "    }}").unwrap();
    writeln!(src, "    let H = normalize(L + V);").unwrap();
    writeln!(src, "    let n_dot_l = max(dot(N, L), 0.0);").unwrap();
    writeln!(src, "    let n_dot_h = max(dot(N, H), 0.0);").unwrap();
    writeln!(src, "    let v_dot_h = max(dot(V, H), 0.0);").unwrap();
    writeln!(
        src,
        "    var atten = spot_factor / max(dist * dist, 0.0001);"
    )
    .unwrap();
    writeln!(src, "    if (light.params.y > 0.0) {{").unwrap();
    writeln!(
        src,
        "        atten *= max(1.0 - dist / light.params.y, 0.0);"
    )
    .unwrap();
    writeln!(src, "    }}").unwrap();
    writeln!(
        src,
        "    let radiance = light.color.xyz * light.params.x * atten;"
    )
    .unwrap();
    // C++ evaluateLight line 237: specularRoughness = max(0.001, specularRoughness)
    writeln!(src, "    let sr = max(roughness, 0.001);").unwrap();
    // C++ evaluateLight: d = diffuseColor / PI; if (!specWf) d *= 1-metallic
    writeln!(src, "    var d = base_color / 3.14159265;").unwrap();
    writeln!(src, "    if (material.use_specular_workflow < 0.5) {{").unwrap();
    writeln!(src, "        d = d * (1.0 - metallic);").unwrap();
    writeln!(src, "    }}").unwrap();
    writeln!(src, "    let alpha = sr * sr;").unwrap();
    writeln!(src, "    let n_dot_v = max(dot(N, V), 0.001);").unwrap();
    // C++ SchlickFresnel: scalar weight, then mix(F0, F90, fresnel)
    writeln!(
        src,
        "    let fresnel_w = pow(max(1.0 - v_dot_h, 0.0), 5.0);"
    )
    .unwrap();
    writeln!(src, "    let F = mix(f0, f90, fresnel_w);").unwrap();
    writeln!(src, "    let D = ggx_distribution(n_dot_h, alpha);").unwrap();
    writeln!(src, "    let G = smith_ggx(n_dot_v, n_dot_l, sr);").unwrap();
    writeln!(
        src,
        "    let spec = (D * G * F) / (4.0 * n_dot_v * n_dot_l + 0.001);"
    )
    .unwrap();
    // C++ line 269: d *= (1.0 - mix(F0, F90, fresnel))
    writeln!(
        src,
        "    d = d * (vec3<f32>(1.0) - mix(f0, f90, fresnel_w));"
    )
    .unwrap();
    writeln!(src, "    return (d + spec) * radiance * n_dot_l;").unwrap();
    writeln!(src, "}}").unwrap();
    writeln!(src).unwrap();

    // Dome light (IBL approximation - hemisphere ambient)
    writeln!(src, "// Dome light IBL approximation (hemisphere ambient)").unwrap();
    writeln!(src, "fn eval_dome_light(light: LightEntry, N: vec3<f32>, base_color: vec3<f32>, roughness: f32, metallic: f32) -> vec3<f32> {{").unwrap();
    writeln!(
        src,
        "    // Simple hemisphere ambient with normal-dependent blend"
    )
    .unwrap();
    writeln!(
        src,
        "    let up_factor = dot(N, vec3<f32>(0.0, 1.0, 0.0)) * 0.5 + 0.5;"
    )
    .unwrap();
    writeln!(src, "    let sky_color = light.color.xyz * light.params.x;").unwrap();
    writeln!(src, "    let ground_color = sky_color * 0.3;").unwrap();
    writeln!(
        src,
        "    let ambient = mix(ground_color, sky_color, up_factor);"
    )
    .unwrap();
    writeln!(src, "    let k_d = (1.0 - metallic);").unwrap();
    writeln!(src, "    return ambient * base_color * k_d * 0.3;").unwrap();
    writeln!(src, "}}").unwrap();
    writeln!(src).unwrap();
}

/// Generate the multi-light evaluation loop for the fragment shader body.
///
/// Expects these variables to be defined in the caller scope:
/// N, V, world_pos, base_color, roughness, metallic, f0, f90
///
/// Returns WGSL code that accumulates into a `lo` variable.
pub fn emit_light_loop(src: &mut String) {
    writeln!(src, "    // Multi-light evaluation loop").unwrap();
    writeln!(src, "    var lo = vec3<f32>(0.0);").unwrap();
    writeln!(
        src,
        "    for (var li = 0u; li < light_data.light_count.x; li++) {{"
    )
    .unwrap();
    writeln!(src, "        let light = light_data.lights[li];").unwrap();
    writeln!(src, "        if (light.type_pad.x == 0.0) {{").unwrap();
    writeln!(
        src,
        "            lo += eval_directional(light, N, V, base_color, roughness, metallic, f0, f90);"
    )
    .unwrap();
    writeln!(src, "        }} else if (light.type_pad.x == 1.0) {{").unwrap();
    writeln!(src, "            lo += eval_point_light(light, in.world_pos, N, V, base_color, roughness, metallic, f0, f90);").unwrap();
    writeln!(src, "        }} else if (light.type_pad.x == 2.0) {{").unwrap();
    writeln!(src, "            lo += eval_spot_light(light, in.world_pos, N, V, base_color, roughness, metallic, f0, f90);").unwrap();
    writeln!(src, "        }} else if (light.type_pad.x == 3.0) {{").unwrap();
    writeln!(
        src,
        "            lo += eval_dome_light(light, N, base_color, roughness, metallic);"
    )
    .unwrap();
    writeln!(src, "        }}").unwrap();
    writeln!(src, "    }}").unwrap();
}

/// Emit shadow atlas texture and comparison sampler bindings.
///
/// Emit shadow atlas texture + comparison sampler WGSL declarations.
///
/// Shadow UBO data is packed inside LightUniforms (emitted by
/// `emit_light_uniforms_wgsl_with_shadows`). This only emits the atlas
/// texture and comparison sampler used by `textureSampleCompare`.
///
/// Uses `BindTextureGroup` convention: binding 0 = texture, binding 1 = sampler.
pub fn emit_shadow_atlas_wgsl(src: &mut String, group: u32) {
    writeln!(
        src,
        "// Shadow atlas depth texture (2D array, one layer per shadow light)"
    )
    .unwrap();
    writeln!(src, "@group({group}) @binding(0)").unwrap();
    writeln!(src, "var shadow_atlas: texture_depth_2d_array;").unwrap();
    writeln!(src, "@group({group}) @binding(1)").unwrap();
    writeln!(src, "var shadow_sampler: sampler_comparison;").unwrap();
    writeln!(src).unwrap();
}

/// Generate the WGSL shadow comparison and cascade functions.
///
/// Matches C++ simpleLighting.glslfx:
/// - `shadowCompare(shadowIndex, coord)` — perspective divide, bias, clamp, compare
/// - `shadowing(lightIndex, world_pos)` — cascade iteration shadowIndexStart..End
///
/// Shadow matrix has NDC->UV [0,1] baked in (from get_world_to_shadow_matrix()),
/// so coords after transform are already in [0,1]. No manual NDC->UV conversion.
pub fn emit_shadow_pcf_function(src: &mut String) {
    // shadowing: matches C++ shadowing (simpleLighting.glslfx:269-291)
    // Iterates shadowIndexStart..shadowIndexEnd per light (cascade support).
    // Shadow data is packed in light_data.shadows[] (same UBO).
    // Uses textureSampleCompare for hardware PCF depth comparison.
    writeln!(src, "// C++ shadowing: cascade iteration per light").unwrap();
    writeln!(
        src,
        "fn shadowing(light_idx: u32, world_pos: vec3<f32>) -> f32 {{"
    )
    .unwrap();
    writeln!(src, "    let light = light_data.lights[light_idx];").unwrap();
    writeln!(src, "    let shadow_start = i32(light.type_pad.z);").unwrap();
    writeln!(src, "    let shadow_end = i32(light.type_pad.w);").unwrap();
    writeln!(
        src,
        "    for (var si = shadow_start; si <= shadow_end; si++) {{"
    )
    .unwrap();
    writeln!(
        src,
        "        let coord = light_data.shadows[si].world_to_shadow * vec4<f32>(world_pos, 1.0);"
    )
    .unwrap();
    // C++ coverage check: any(lessThan(xyz, 0)) || any(greaterThan(xyz, www))
    writeln!(
        src,
        "        if (coord.x < 0.0 || coord.y < 0.0 || coord.z < 0.0 ||"
    )
    .unwrap();
    writeln!(
        src,
        "            coord.x > coord.w || coord.y > coord.w || coord.z > coord.w) {{"
    )
    .unwrap();
    writeln!(src, "            continue;").unwrap();
    writeln!(src, "        }}").unwrap();
    // Perspective divide to UV [0,1] (NDC->UV bias baked into shadow matrix)
    writeln!(src, "        let c = coord.xyz / coord.w;").unwrap();
    // Apply depth bias from shadow params
    writeln!(
        src,
        "        let biased_z = min(1.0, c.z + light_data.shadows[si].params.y);"
    )
    .unwrap();
    // Hardware PCF depth comparison: returns 1.0 (lit) or 0.0 (shadowed)
    writeln!(
        src,
        "        return textureSampleCompare(shadow_atlas, shadow_sampler, c.xy, si, biased_z);"
    )
    .unwrap();
    writeln!(src, "    }}").unwrap();
    writeln!(src, "    return 1.0;").unwrap();
    writeln!(src, "}}").unwrap();
    writeln!(src).unwrap();
}

/// Generate shadow-aware light evaluation loop.
///
/// Matches C++ integrateLightsDefault (simpleLighting.glslfx:430-435):
/// ```glsl
/// float shadow = light.hasShadow ? shadowing(i, Peye) : 1.0;
/// ```
/// Like `emit_light_loop` but multiplies each non-dome light contribution
/// by `shadowing()`. Requires shadow uniforms/atlas bound.
pub fn emit_light_loop_with_shadows(src: &mut String) {
    writeln!(
        src,
        "    // Multi-light evaluation loop (shadow-aware, C++ integrateLightsDefault)"
    )
    .unwrap();
    writeln!(src, "    var lo = vec3<f32>(0.0);").unwrap();
    writeln!(
        src,
        "    for (var li = 0u; li < light_data.light_count.x; li++) {{"
    )
    .unwrap();
    writeln!(src, "        let light = light_data.lights[li];").unwrap();
    writeln!(src, "        var contrib = vec3<f32>(0.0);").unwrap();
    writeln!(src, "        if (light.type_pad.x == 0.0) {{").unwrap();
    writeln!(src, "            contrib = eval_directional(light, N, V, base_color, roughness, metallic, f0, f90);").unwrap();
    writeln!(src, "        }} else if (light.type_pad.x == 1.0) {{").unwrap();
    writeln!(src, "            contrib = eval_point_light(light, in.world_pos, N, V, base_color, roughness, metallic, f0, f90);").unwrap();
    writeln!(src, "        }} else if (light.type_pad.x == 2.0) {{").unwrap();
    writeln!(src, "            contrib = eval_spot_light(light, in.world_pos, N, V, base_color, roughness, metallic, f0, f90);").unwrap();
    writeln!(src, "        }} else if (light.type_pad.x == 3.0) {{").unwrap();
    writeln!(
        src,
        "            // Dome lights are not shadow-mapped (C++ isIndirectLight path)."
    )
    .unwrap();
    writeln!(
        src,
        "            lo += eval_dome_light(light, N, base_color, roughness, metallic);"
    )
    .unwrap();
    writeln!(src, "            continue;").unwrap();
    writeln!(src, "        }}").unwrap();
    // C++: float shadow = light.hasShadow ? shadowing(i, Peye) : 1.0;
    writeln!(src, "        var shadow = 1.0;").unwrap();
    writeln!(src, "        if (light.type_pad.y > 0.5) {{").unwrap();
    writeln!(src, "            shadow = shadowing(li, in.world_pos);").unwrap();
    writeln!(src, "        }}").unwrap();
    writeln!(src, "        lo += contrib * shadow;").unwrap();
    writeln!(src, "    }}").unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_light() {
        let light = default_light();
        assert_eq!(
            f32::to_bits(light.type_pad[0]),
            GpuLightType::Directional as u32
        );
        assert_eq!(light.params[0], 1.0);
    }

    #[test]
    fn test_build_light_uniforms_size() {
        let lights = vec![default_light()];
        let data = build_light_uniforms(&lights);
        assert_eq!(data.len(), LIGHT_UNIFORMS_SIZE);

        // Check light_count = 1
        let count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        assert_eq!(count, 1);
    }

    #[test]
    fn test_build_empty_lights() {
        let data = build_light_uniforms(&[]);
        assert_eq!(data.len(), LIGHT_UNIFORMS_SIZE);
        let count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        assert_eq!(count, 0);
    }

    #[test]
    fn test_light_entry_size() {
        // 5 vec4 (5*16=80) + mat4x4 (64) = 144 bytes
        assert_eq!(LIGHT_ENTRY_SIZE, 144);
        assert_eq!(std::mem::size_of::<LightGpuData>(), LIGHT_ENTRY_SIZE);
    }

    #[test]
    fn test_emit_light_uniforms_wgsl() {
        let mut src = String::new();
        emit_light_uniforms_wgsl(&mut src, 0, 1);
        assert!(src.contains("LightEntry"));
        assert!(src.contains("LightUniforms"));
        assert!(src.contains("@group(0) @binding(1)"));
        assert!(src.contains("light_count"));
    }

    #[test]
    fn test_emit_light_eval_functions() {
        let mut src = String::new();
        emit_light_eval_functions(&mut src);
        assert!(src.contains("fn eval_directional"));
        assert!(src.contains("fn eval_point_light"));
        assert!(src.contains("fn eval_spot_light"));
        assert!(src.contains("fn eval_dome_light"));
    }

    #[test]
    fn test_emit_light_loop() {
        let mut src = String::new();
        emit_light_loop(&mut src);
        assert!(src.contains("light_data.light_count.x"));
        assert!(src.contains("light_data.lights[li]"));
        // Light type dispatch must compare as f32 (not bitcast to u32)
        assert!(
            src.contains("type_pad.x == 0.0"),
            "directional type check must be f32"
        );
        assert!(
            src.contains("type_pad.x == 1.0"),
            "point type check must be f32"
        );
        assert!(
            src.contains("type_pad.x == 2.0"),
            "spot type check must be f32"
        );
        assert!(
            src.contains("type_pad.x == 3.0"),
            "dome type check must be f32"
        );
        // Must NOT use bitcast (bitcast<u32>(1.0f) != 1u)
        assert!(
            !src.contains("bitcast"),
            "must not use bitcast for f32 type comparison"
        );
    }

    // ---------------------------------------------------------------
    // Overflow / edge case safety
    // ---------------------------------------------------------------

    #[test]
    fn test_build_light_uniforms_max_lights() {
        // Exactly MAX_LIGHTS should fill all slots
        let lights: Vec<_> = (0..MAX_LIGHTS).map(|_| default_light()).collect();
        let data = build_light_uniforms(&lights);
        assert_eq!(data.len(), LIGHT_UNIFORMS_SIZE);
        let count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        assert_eq!(count, MAX_LIGHTS as u32);
    }

    #[test]
    fn test_build_light_uniforms_overflow_clamps() {
        // More than MAX_LIGHTS should clamp, not overflow
        let lights: Vec<_> = (0..MAX_LIGHTS + 5).map(|_| default_light()).collect();
        let data = build_light_uniforms(&lights);
        assert_eq!(
            data.len(),
            LIGHT_UNIFORMS_SIZE,
            "overflow must not change buffer size"
        );
        let count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        assert_eq!(count, MAX_LIGHTS as u32, "count must clamp to MAX_LIGHTS");
    }

    #[test]
    fn test_write_light_entry_roundtrip() {
        // Verify write_light_entry produces exactly LIGHT_ENTRY_SIZE bytes
        // and data round-trips correctly.
        let light = LightGpuData {
            type_pad: [1.0, 0.0, 0.0, 0.0],
            position: [1.0, 2.0, 3.0, 1.0],
            direction: [0.0, -1.0, 0.0, 0.0],
            color: [0.5, 0.8, 1.0, 1.0],
            params: [2.5, 10.0, 0.3, 0.7],
            world_to_light_transform: MAT4_IDENTITY,
        };
        let mut buf = Vec::new();
        write_light_entry(&mut buf, &light);
        assert_eq!(buf.len(), LIGHT_ENTRY_SIZE);

        // Verify position.y = 2.0 at offset 4*4 + 1*4 = 20
        let off = 4 * 4 + 1 * 4; // skip type_pad(16) + position.x(4)
        let val = f32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]]);
        assert!((val - 2.0).abs() < 1e-6, "position.y = {}", val);
    }

    #[test]
    fn test_light_uniforms_total_size_formula() {
        // Verify the constant formula is internally consistent
        assert_eq!(LIGHT_UNIFORMS_SIZE, 16 + MAX_LIGHTS * LIGHT_ENTRY_SIZE);
        // 5*vec4 + mat4x4 = 144 bytes per entry; 16 lights = 2304; + 16 header = 2320
        assert_eq!(LIGHT_UNIFORMS_SIZE, 16 + 16 * 144);
        assert_eq!(LIGHT_UNIFORMS_SIZE, 2320);
    }

    #[test]
    fn test_hd_light_extraction() {
        use crate::light::{DIRTY_PARAMS, DIRTY_TRANSFORM, LightSceneDelegate};
        use std::collections::HashMap;
        use usd_gf::{Matrix4d, Vec3f};
        use usd_sdf::Path as SdfPath;
        use usd_tf::Token;
        use usd_vt::Value;

        struct MockDel {
            params: HashMap<String, Value>,
        }
        impl LightSceneDelegate for MockDel {
            fn get_light_param_value(&self, _id: &SdfPath, p: &Token) -> Value {
                self.params.get(p.as_str()).cloned().unwrap_or_default()
            }
            fn get_transform(&self, _: &SdfPath) -> Matrix4d {
                Matrix4d::identity()
            }
            fn get_visible(&self, _: &SdfPath) -> bool {
                true
            }
            fn get(&self, _: &SdfPath, k: &Token) -> Value {
                self.params.get(k.as_str()).cloned().unwrap_or_default()
            }
        }

        let path = SdfPath::from_string("/light/key").unwrap();
        let mut hd_light = HdStLight::new_distant(path);

        let mut del = MockDel {
            params: HashMap::new(),
        };
        del.params
            .insert("color".into(), Value::from(Vec3f::new(1.0, 1.0, 1.0)));
        del.params.insert("intensity".into(), Value::from(2.0f32));
        del.params.insert("exposure".into(), Value::from(1.0f32)); // 2^1=2; total=4
        del.params.insert("normalize".into(), Value::from(true));

        let mut bits = DIRTY_PARAMS | DIRTY_TRANSFORM;
        hd_light.sync(&del, &mut bits);

        let gpu = light_from_hd(&hd_light);
        // type_pad[0] should be Directional = 0
        assert_eq!(
            f32::to_bits(gpu.type_pad[0]),
            GpuLightType::Directional as u32
        );
        // intensity (luma of diffuse) should be > 0
        assert!(gpu.params[0] > 0.0, "intensity={}", gpu.params[0]);
    }
}
