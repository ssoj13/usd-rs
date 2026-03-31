//! Shadow mapping for Storm wgpu pipeline.
//!
//! Implements a shadow atlas (texture_2d_array, depth32float) and a
//! depth-only shadow pass that renders scene geometry from each
//! shadow-casting light's viewpoint.
//!
//! # Architecture
//!
//! ```text
//! ShadowAtlas   -- owns the wgpu depth texture array (2048x2048 x MAX_SHADOWS)
//! ShadowPass    -- records depth-only draw commands per light slice
//! ShadowUniforms -- CPU buffer of per-light VP matrices uploaded each frame
//! ```
//!
//! C++ reference: hdx/shadowTask.cpp, glf/simpleLighting.glslfx
//! Shadow matrix convention: world -> NDC (light view * projection), matching
//! C++ HdGet_shadow_worldToShadowMatrix / eyeToShadowMatrix * viewToWorld.

use crate::lighting::ShadowParams;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum number of simultaneous shadow-casting lights.
pub const MAX_SHADOWS: usize = 4;

/// Shadow atlas tile dimension (pixels).  Each light gets one tile.
pub const SHADOW_MAP_SIZE: u32 = 2048;

/// Total size in bytes of the ShadowUniforms GPU buffer.
/// Layout: array<ShadowEntry, MAX_SHADOWS>
/// Each entry = 2 * mat4x4 (128) + vec4 blur/bias/pad (16) = 144 bytes.
/// Matches C++ `ShadowMatrix` struct in simpleLightingContext.cpp:303-309.
pub const SHADOW_UNIFORMS_SIZE: usize = MAX_SHADOWS * SHADOW_ENTRY_SIZE;

/// Size of one ShadowEntry in the uniform buffer (bytes).
/// C++: viewToShadowMatrix(64) + shadowToViewMatrix(64) + blur(4) + bias(4) + pad(8) = 144
pub const SHADOW_ENTRY_SIZE: usize = 144;

// ---------------------------------------------------------------------------
// CPU-side shadow entry (matches WGSL ShadowEntry struct)
// ---------------------------------------------------------------------------

/// Per-light shadow data uploaded to GPU each frame.
///
/// Layout matches C++ `ShadowMatrix` struct (simpleLightingContext.cpp:303-309):
/// ```c++
/// struct ShadowMatrix {
///     float viewToShadowMatrix[16];  // eye→shadow (already in UV [0,1] space)
///     float shadowToViewMatrix[16];  // shadow→eye (inverse, for filter width)
///     float blur;
///     float bias;
///     float padding[2];
/// };
/// ```
///
/// NOTE: C++ stores eye-to-shadow (view*worldToShadow). Our pipeline works in
/// world-space, so we store world-to-shadow directly. The shader input is
/// `world_pos` not `Peye`, so the matrix is `worldToShadow` = view*proj*bias
/// (with NDC→UV baked in, matching C++ GetWorldToShadowMatrix).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ShadowEntry {
    /// World-to-shadow matrix with NDC→UV [0,1] baked in.
    /// C++ equivalent: `viewToWorldMatrix * GetWorldToShadowMatrix()`.
    /// For us: `GetWorldToShadowMatrix()` directly (world-space pipeline).
    pub world_to_shadow: [f32; 16],
    /// Inverse: shadow→world matrix (used for filter width computation).
    /// C++ equivalent: `viewToShadowMatrix.GetInverse()`.
    pub shadow_to_world: [f32; 16],
    /// vec4(blur, bias, pad, pad) — matches C++ ShadowMatrix layout.
    pub params: [f32; 4],
}

impl Default for ShadowEntry {
    fn default() -> Self {
        Self {
            world_to_shadow: MAT4_IDENTITY,
            shadow_to_world: MAT4_IDENTITY,
            params: [0.0, 0.001, 0.0, 0.0],
        }
    }
}

/// Identity mat4x4 column-major.
pub const MAT4_IDENTITY: [f32; 16] = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];

// ---------------------------------------------------------------------------
// Shadow matrix computation (CPU, no GPU dependency)
// ---------------------------------------------------------------------------

/// Compute the world-to-shadow-clip matrix for a **directional light**.
///
/// Matches C++ GlfSimpleShadowArray::ComputeShadowMatrix for distant lights:
/// view = look-at from `scene_center + dir * dist` toward `scene_center`,
/// proj = ortho(±half_size, near=0, far=2*dist).
///
/// # Arguments
/// * `dir` - world-space light direction (normalized, *toward* light source)
/// * `scene_center` - world-space center of the shadow receiver volume
/// * `scene_radius` - half-diagonal of the shadow receiver bounding box
pub fn compute_shadow_matrix_directional(
    dir: [f32; 3],
    scene_center: [f32; 3],
    scene_radius: f32,
) -> [f32; 16] {
    let r = scene_radius.max(0.01);

    // Light position: pull back far enough to cover the entire scene.
    let lx = scene_center[0] + dir[0] * r * 2.0;
    let ly = scene_center[1] + dir[1] * r * 2.0;
    let lz = scene_center[2] + dir[2] * r * 2.0;

    // View matrix: look-at from light toward scene center.
    let view = look_at([lx, ly, lz], scene_center, pick_up(dir));

    // Ortho projection: covers ±scene_radius in X/Y, [0..4r] in Z.
    let proj = ortho(-r, r, -r, r, 0.0, r * 4.0);

    // Bake NDC→UV [0,1] bias into the combined VP matrix.
    // Matches C++ GlfSimpleShadowArray::GetWorldToShadowMatrix().
    get_world_to_shadow_matrix(view, proj)
}

/// Compute the world-to-shadow-clip matrix for a **spot light**.
///
/// Matches C++ GlfSimpleShadowArray::ComputeShadowMatrix for spotlights:
/// perspective projection using the spot cone angle.
///
/// # Arguments
/// * `pos` - world-space light position
/// * `dir` - world-space spot direction (normalized, from light outward)
/// * `outer_angle_rad` - full cone half-angle in radians
pub fn compute_shadow_matrix_spot(pos: [f32; 3], dir: [f32; 3], outer_angle_rad: f32) -> [f32; 16] {
    // Target point: 1 unit along the spot direction.
    let target = [pos[0] + dir[0], pos[1] + dir[1], pos[2] + dir[2]];
    let view = look_at(pos, target, pick_up(dir));

    // Perspective: fov = 2 * outer_angle (full cone).
    let fov = (outer_angle_rad * 2.0).min(std::f32::consts::PI - 0.01);
    let proj = perspective(fov, 1.0, 0.1, 1000.0);

    // Bake NDC→UV [0,1] bias into the combined VP matrix.
    get_world_to_shadow_matrix(view, proj)
}

/// Build `ShadowEntry` from `ShadowParams`.
///
/// The matrix in `ShadowParams` must already include NDC→UV bias
/// (from `get_world_to_shadow_matrix()`).
pub fn build_shadow_entry(params: &ShadowParams) -> ShadowEntry {
    let shadow_to_world = mat4_inverse(params.matrix);
    ShadowEntry {
        world_to_shadow: params.matrix,
        shadow_to_world,
        params: [params.blur, params.bias, 0.0, 0.0],
    }
}

/// Serialize `MAX_SHADOWS` shadow entries to a byte buffer for GPU upload.
///
/// The buffer is SHADOW_UNIFORMS_SIZE bytes, matching the WGSL
/// `ShadowUniforms` struct layout.
pub fn build_shadow_uniforms(entries: &[ShadowEntry]) -> Vec<u8> {
    let mut data = Vec::with_capacity(SHADOW_UNIFORMS_SIZE);
    for i in 0..MAX_SHADOWS {
        let e = entries.get(i).copied().unwrap_or_default();
        write_shadow_entry(&mut data, &e);
    }
    debug_assert_eq!(data.len(), SHADOW_UNIFORMS_SIZE);
    data
}

/// Compute `worldToShadowMatrix` with NDC→UV [0,1] bias baked in.
///
/// Matches C++ `GlfSimpleShadowArray::GetWorldToShadowMatrix()`:
/// ```c++
/// GfMatrix4d size = SetScale(0.5, 0.5, 0.5);
/// GfMatrix4d center = SetTranslate(0.5, 0.5, 0.5);
/// return view * proj * size * center;
/// ```
///
/// After this transform, shadow coords are in [0,1] for XYZ.
/// X,Y = texture UV, Z = depth comparison value.
pub fn get_world_to_shadow_matrix(view: [f32; 16], proj: [f32; 16]) -> [f32; 16] {
    let vp = mat4_mul(proj, view);
    // NDC [-1,1] → UV [0,1]: scale by 0.5, then translate by 0.5
    // In column-major: scale * translate bakes into:
    //   x' = x * 0.5 + 0.5
    //   y' = y * 0.5 + 0.5
    //   z' = z * 0.5 + 0.5
    let bias = ndc_to_uv_bias_matrix();
    mat4_mul(bias, vp)
}

/// NDC [-1,1] → UV [0,1] bias matrix (column-major).
/// Equivalent to C++ `SetScale(0.5) * SetTranslate(0.5)`.
fn ndc_to_uv_bias_matrix() -> [f32; 16] {
    [
        0.5, 0.0, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 0.0, 0.5, 0.0, 0.5, 0.5, 0.5, 1.0,
    ]
}

// ---------------------------------------------------------------------------
// Private math helpers (no external dependency)
// ---------------------------------------------------------------------------

/// Column-major mat4 multiply: result = a * b (both column-major).
fn mat4_mul(a: [f32; 16], b: [f32; 16]) -> [f32; 16] {
    let mut r = [0.0f32; 16];
    for col in 0..4 {
        for row in 0..4 {
            let mut s = 0.0f32;
            for k in 0..4 {
                // a[row + k*4] is element (row, k) in column-major
                // b[k + col*4] is element (k, col) in column-major
                s += a[row + k * 4] * b[k + col * 4];
            }
            r[row + col * 4] = s;
        }
    }
    r
}

/// Look-at view matrix (column-major, right-handed, matching OpenGL/USD).
///
/// eye    = camera position
/// center = target to look at
/// up     = world-up hint (must not be parallel to forward)
fn look_at(eye: [f32; 3], center: [f32; 3], up: [f32; 3]) -> [f32; 16] {
    let f = normalize3([center[0] - eye[0], center[1] - eye[1], center[2] - eye[2]]);
    let s = normalize3(cross3(f, up));
    let u = cross3(s, f);

    // Column-major layout: col0=s, col1=u, col2=-f, col3=translation
    [
        s[0],
        u[0],
        -f[0],
        0.0,
        s[1],
        u[1],
        -f[1],
        0.0,
        s[2],
        u[2],
        -f[2],
        0.0,
        -dot3(s, eye),
        -dot3(u, eye),
        dot3(f, eye),
        1.0,
    ]
}

/// Orthographic projection matrix (column-major, maps to NDC [-1,1] in Z
/// matching OpenGL convention used by C++ GlfSimpleShadowArray).
fn ortho(l: f32, r: f32, b: f32, t: f32, n: f32, f: f32) -> [f32; 16] {
    let rml = r - l;
    let tmb = t - b;
    let fmn = f - n;
    [
        2.0 / rml,
        0.0,
        0.0,
        0.0,
        0.0,
        2.0 / tmb,
        0.0,
        0.0,
        0.0,
        0.0,
        -2.0 / fmn,
        0.0,
        -(r + l) / rml,
        -(t + b) / tmb,
        -(f + n) / fmn,
        1.0,
    ]
}

/// Perspective projection matrix (column-major, right-handed, depth [-1,1]).
/// fov_y is in radians, aspect = w/h.
fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> [f32; 16] {
    let tan_half = (fov_y * 0.5).tan();
    let f_range = far - near;
    [
        1.0 / (aspect * tan_half),
        0.0,
        0.0,
        0.0,
        0.0,
        1.0 / tan_half,
        0.0,
        0.0,
        0.0,
        0.0,
        -(far + near) / f_range,
        -1.0,
        0.0,
        0.0,
        -(2.0 * far * near) / f_range,
        0.0,
    ]
}

/// Choose a world-up vector that is not parallel to `dir`.
fn pick_up(dir: [f32; 3]) -> [f32; 3] {
    // If dir is nearly vertical, use X-axis as up hint
    if dir[1].abs() > 0.9 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 1.0, 0.0]
    }
}

fn normalize3(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-8);
    [v[0] / len, v[1] / len, v[2] / len]
}

fn cross3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// General 4x4 matrix inverse (column-major). Returns identity if singular.
/// Used to compute shadow_to_world from world_to_shadow.
fn mat4_inverse(m: [f32; 16]) -> [f32; 16] {
    // Index helper: element at (row, col) in column-major = m[row + col*4]
    let a = |r: usize, c: usize| m[r + c * 4];

    // Compute cofactors via 2x2 determinants
    let s0 = a(0, 0) * a(1, 1) - a(1, 0) * a(0, 1);
    let s1 = a(0, 0) * a(1, 2) - a(1, 0) * a(0, 2);
    let s2 = a(0, 0) * a(1, 3) - a(1, 0) * a(0, 3);
    let s3 = a(0, 1) * a(1, 2) - a(1, 1) * a(0, 2);
    let s4 = a(0, 1) * a(1, 3) - a(1, 1) * a(0, 3);
    let s5 = a(0, 2) * a(1, 3) - a(1, 2) * a(0, 3);

    let c5 = a(2, 2) * a(3, 3) - a(3, 2) * a(2, 3);
    let c4 = a(2, 1) * a(3, 3) - a(3, 1) * a(2, 3);
    let c3 = a(2, 1) * a(3, 2) - a(3, 1) * a(2, 2);
    let c2 = a(2, 0) * a(3, 3) - a(3, 0) * a(2, 3);
    let c1 = a(2, 0) * a(3, 2) - a(3, 0) * a(2, 2);
    let c0 = a(2, 0) * a(3, 1) - a(3, 0) * a(2, 1);

    let det = s0 * c5 - s1 * c4 + s2 * c3 + s3 * c2 - s4 * c1 + s5 * c0;
    if det.abs() < 1e-12 {
        return MAT4_IDENTITY;
    }
    let inv_det = 1.0 / det;

    let mut r = [0.0f32; 16];
    r[0 + 0 * 4] = (a(1, 1) * c5 - a(1, 2) * c4 + a(1, 3) * c3) * inv_det;
    r[0 + 1 * 4] = (-a(0, 1) * c5 + a(0, 2) * c4 - a(0, 3) * c3) * inv_det;
    r[0 + 2 * 4] = (a(3, 1) * s5 - a(3, 2) * s4 + a(3, 3) * s3) * inv_det;
    r[0 + 3 * 4] = (-a(2, 1) * s5 + a(2, 2) * s4 - a(2, 3) * s3) * inv_det;

    r[1 + 0 * 4] = (-a(1, 0) * c5 + a(1, 2) * c2 - a(1, 3) * c1) * inv_det;
    r[1 + 1 * 4] = (a(0, 0) * c5 - a(0, 2) * c2 + a(0, 3) * c1) * inv_det;
    r[1 + 2 * 4] = (-a(3, 0) * s5 + a(3, 2) * s2 - a(3, 3) * s1) * inv_det;
    r[1 + 3 * 4] = (a(2, 0) * s5 - a(2, 2) * s2 + a(2, 3) * s1) * inv_det;

    r[2 + 0 * 4] = (a(1, 0) * c4 - a(1, 1) * c2 + a(1, 3) * c0) * inv_det;
    r[2 + 1 * 4] = (-a(0, 0) * c4 + a(0, 1) * c2 - a(0, 3) * c0) * inv_det;
    r[2 + 2 * 4] = (a(3, 0) * s4 - a(3, 1) * s2 + a(3, 3) * s0) * inv_det;
    r[2 + 3 * 4] = (-a(2, 0) * s4 + a(2, 1) * s2 - a(2, 3) * s0) * inv_det;

    r[3 + 0 * 4] = (-a(1, 0) * c3 + a(1, 1) * c1 - a(1, 2) * c0) * inv_det;
    r[3 + 1 * 4] = (a(0, 0) * c3 - a(0, 1) * c1 + a(0, 2) * c0) * inv_det;
    r[3 + 2 * 4] = (-a(3, 0) * s3 + a(3, 1) * s1 - a(3, 2) * s0) * inv_det;
    r[3 + 3 * 4] = (a(2, 0) * s3 - a(2, 1) * s1 + a(2, 2) * s0) * inv_det;

    r
}

/// Serialize one ShadowEntry: 2*mat4x4 + vec4 = 144 bytes.
/// Matches C++ ShadowMatrix struct layout.
fn write_shadow_entry(data: &mut Vec<u8>, e: &ShadowEntry) {
    for v in &e.world_to_shadow {
        data.extend_from_slice(&v.to_le_bytes());
    }
    for v in &e.shadow_to_world {
        data.extend_from_slice(&v.to_le_bytes());
    }
    for v in &e.params {
        data.extend_from_slice(&v.to_le_bytes());
    }
}

// ---------------------------------------------------------------------------
// Depth-only WGSL vertex shader (used by the shadow render pass)
// ---------------------------------------------------------------------------

/// Generate the depth-only WGSL shader used for the shadow pass.
///
/// Takes only vertex positions and outputs gl_Position in light clip space.
/// No fragment shader — depth write is the sole output.
///
/// Binding layout (matches shadow pass pipeline):
///   @group(0) @binding(0) — ShadowPassUniforms (world_to_shadow mat4x4)
///   @location(0)          — vertex position (vec3<f32>)
pub fn depth_only_vertex_shader_wgsl() -> String {
    r#"// Storm shadow depth-only vertex shader (auto-generated)
// Renders geometry from the light's viewpoint to write shadow depth.

struct ShadowPassUniforms {
    world_to_shadow: mat4x4<f32>,
    model: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> shadow_pass: ShadowPassUniforms;

@vertex
fn vs_shadow(@location(0) pos: vec3<f32>) -> @builtin(position) vec4<f32> {
    // Transform vertex: model -> world -> light clip space
    return shadow_pass.world_to_shadow * shadow_pass.model * vec4<f32>(pos, 1.0);
}
"#
    .to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: multiply vec4 by column-major mat4.
    fn mat4_mul_vec4(m: [f32; 16], v: [f32; 4]) -> [f32; 4] {
        let mut r = [0.0f32; 4];
        for row in 0..4 {
            for col in 0..4 {
                r[row] += m[row + col * 4] * v[col];
            }
        }
        r
    }

    // -----------------------------------------------------------------------
    // Shadow entry serialization
    // -----------------------------------------------------------------------

    #[test]
    fn test_shadow_uniforms_size() {
        // 4 entries * 144 bytes = 576 bytes (C++ ShadowMatrix: 2*mat4+vec4)
        assert_eq!(SHADOW_ENTRY_SIZE, 144);
        assert_eq!(SHADOW_UNIFORMS_SIZE, MAX_SHADOWS * 144);
        let data = build_shadow_uniforms(&[]);
        assert_eq!(data.len(), SHADOW_UNIFORMS_SIZE);
    }

    #[test]
    fn test_shadow_entry_roundtrip() {
        let entry = ShadowEntry {
            world_to_shadow: MAT4_IDENTITY,
            shadow_to_world: MAT4_IDENTITY,
            params: [0.5, 0.002, 0.0, 0.0],
        };
        let data = build_shadow_uniforms(&[entry]);
        // bias = params[1] at offset 128 (2*mat4) + 4 = 132
        let off = 128 + 4;
        let bias = f32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
        assert!((bias - 0.002).abs() < 1e-6, "bias={bias}");
    }

    #[test]
    fn test_build_shadow_entry() {
        let params = ShadowParams {
            enabled: true,
            blur: 1.5,
            bias: 0.003,
            matrix: MAT4_IDENTITY,
        };
        let e = build_shadow_entry(&params);
        assert!((e.params[0] - 1.5).abs() < 1e-6);
        assert!((e.params[1] - 0.003).abs() < 1e-6);
        // shadow_to_world should be identity (inverse of identity)
        assert!((e.shadow_to_world[0] - 1.0).abs() < 1e-6);
        assert!((e.shadow_to_world[5] - 1.0).abs() < 1e-6);
    }

    // -----------------------------------------------------------------------
    // Shadow matrix computation
    // -----------------------------------------------------------------------

    #[test]
    fn test_directional_shadow_matrix_origin() {
        // Light from above (+Y), scene centered at origin, radius=1.
        let m = compute_shadow_matrix_directional([0.0, 1.0, 0.0], [0.0, 0.0, 0.0], 1.0);
        // NDC→UV bias is baked in: origin maps to UV (0.5, 0.5).
        let clip = mat4_mul_vec4(m, [0.0, 0.0, 0.0, 1.0]);
        let w = clip[3];
        assert!(w > 0.0, "ortho w={}", w);
        let uvx = clip[0] / w;
        let uvy = clip[1] / w;
        assert!((uvx - 0.5).abs() < 0.01, "origin UV.x={uvx}");
        assert!((uvy - 0.5).abs() < 0.01, "origin UV.y={uvy}");
    }

    #[test]
    fn test_directional_shadow_matrix_diagonal() {
        // Light from diagonal, ensure origin stays in UV [0,1] volume.
        let dir = {
            let v = [1.0f32, 1.0, 1.0];
            let len = (3.0f32).sqrt();
            [v[0] / len, v[1] / len, v[2] / len]
        };
        let m = compute_shadow_matrix_directional(dir, [0.0; 3], 5.0);
        let clip = mat4_mul_vec4(m, [0.0, 0.0, 0.0, 1.0]);
        let w = clip[3];
        assert!(w > 0.0);
        let uvx = clip[0] / w;
        let uvy = clip[1] / w;
        // NDC→UV bias: origin at center → UV ~0.5
        assert!(uvx >= -0.01 && uvx <= 1.01, "uvx={uvx}");
        assert!(uvy >= -0.01 && uvy <= 1.01, "uvy={uvy}");
    }

    #[test]
    fn test_spot_shadow_matrix_w_positive() {
        // Spot at (0,5,0) pointing down (0,-1,0), 30-degree cone.
        let m =
            compute_shadow_matrix_spot([0.0, 5.0, 0.0], [0.0, -1.0, 0.0], 30.0_f32.to_radians());
        // Origin should produce w > 0 (in front of camera)
        let clip = mat4_mul_vec4(m, [0.0, 0.0, 0.0, 1.0]);
        assert!(clip[3] > 0.0, "w must be positive: w={}", clip[3]);
    }

    #[test]
    fn test_spot_shadow_matrix_target_on_axis() {
        // A point on the spot axis should have UV X,Y near 0.5 (NDC 0 + bias).
        let m =
            compute_shadow_matrix_spot([0.0, 10.0, 0.0], [0.0, -1.0, 0.0], 45.0_f32.to_radians());
        // Point 2 units below light, directly on axis.
        let clip = mat4_mul_vec4(m, [0.0, 8.0, 0.0, 1.0]);
        let w = clip[3];
        assert!(w > 0.0, "w={w}");
        let uvx = clip[0] / w;
        let uvy = clip[1] / w;
        assert!((uvx - 0.5).abs() < 0.01, "on-axis uvx={uvx}");
        assert!((uvy - 0.5).abs() < 0.01, "on-axis uvy={uvy}");
    }

    // -----------------------------------------------------------------------
    // Matrix math helpers
    // -----------------------------------------------------------------------

    #[test]
    fn test_mat4_identity_mul() {
        let a = MAT4_IDENTITY;
        let b = MAT4_IDENTITY;
        let r = mat4_mul(a, b);
        for (i, (rv, expected)) in r.iter().zip(MAT4_IDENTITY.iter()).enumerate() {
            assert!((rv - expected).abs() < 1e-6, "element {i}: got {rv}");
        }
    }

    #[test]
    fn test_look_at_forward() {
        // Camera at (0,0,5) looking at origin — forward is (0,0,-1).
        let m = look_at([0.0, 0.0, 5.0], [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        // Origin should end up at (0,0,-5) in view space = last column translation.
        let v = mat4_mul_vec4(m, [0.0, 0.0, 0.0, 1.0]);
        assert!(
            v[2].abs() < 5.1 && v[2].abs() > 4.9,
            "view-z of origin={}",
            v[2]
        );
    }

    #[test]
    fn test_ortho_maps_extents() {
        // Points at (±1, ±1, ±far) should map to NDC ±1.
        let m = ortho(-1.0, 1.0, -1.0, 1.0, 0.0, 10.0);
        // Right edge X=1 -> NDC X=1
        let clip = mat4_mul_vec4(m, [1.0, 0.0, 0.0, 1.0]);
        assert!((clip[0] - 1.0).abs() < 1e-5, "right edge ndcx={}", clip[0]);
    }

    #[test]
    fn test_depth_only_shader_valid_wgsl() {
        let src = depth_only_vertex_shader_wgsl();
        assert!(src.contains("@vertex"));
        assert!(src.contains("vs_shadow"));
        assert!(src.contains("world_to_shadow"));
        assert!(src.contains("ShadowPassUniforms"));
        // Must not have a fragment entry point
        assert!(!src.contains("@fragment"));
    }

    #[test]
    fn test_shadow_uniforms_overflow_clamp() {
        // More entries than MAX_SHADOWS — buffer size must stay constant.
        let entries: Vec<_> = (0..MAX_SHADOWS + 3)
            .map(|_| ShadowEntry::default())
            .collect();
        let data = build_shadow_uniforms(&entries);
        assert_eq!(data.len(), SHADOW_UNIFORMS_SIZE);
    }
}
