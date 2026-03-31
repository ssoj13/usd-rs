//! WGSL shader code generation for Storm.
//!
//! Generates WGSL vertex + fragment shaders from a MeshShaderKey and
//! material parameters. This replaces the C++ HdSt_CodeGen GLSL path
//! with direct WGSL output for the wgpu backend.

use crate::binding::slots;
use crate::draw_program_key::{BasisCurvesProgramKey, PointsProgramKey};
use crate::lighting;
use crate::mesh_shader_key::{MeshShaderKey, ShadingModel};
use std::fmt::Write;

/// Generated WGSL shader code (single source with both VS and FS entry points).
#[derive(Debug, Clone)]
pub struct WgslShaderCode {
    /// Combined WGSL source containing all structs, VS, and FS
    pub source: String,
    /// Vertex entry point name
    pub vs_entry: &'static str,
    /// Fragment entry point name
    pub fs_entry: &'static str,
}

/// Material parameters for UsdPreviewSurface -> WGSL codegen.
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialParams {
    /// Base diffuse color (linear RGB)
    pub diffuse_color: [f32; 3],
    /// Metallic factor [0..1]
    pub metallic: f32,
    /// Roughness factor [0..1]
    pub roughness: f32,
    /// Opacity [0..1]
    pub opacity: f32,
    /// Emissive color (linear RGB)
    pub emissive_color: [f32; 3],
    /// Whether to use vertex color as diffuse
    pub use_vertex_color: bool,
    /// IOR for specular calculation
    pub ior: f32,
    /// Clearcoat layer intensity [0..1] (0 = disabled)
    pub clearcoat: f32,
    /// Clearcoat roughness [0..1]
    pub clearcoat_roughness: f32,
    /// When true, use specularColor as F0 instead of metallic+IOR workflow
    pub use_specular_workflow: bool,
    /// Specular color (F0) for specular workflow (linear RGB)
    pub specular_color: [f32; 3],
    /// Displacement amount (vertex offset along normal)
    pub displacement: f32,
    // --- Advanced PBR features ---
    /// Subsurface scattering weight [0..1]
    pub subsurface: f32,
    /// Subsurface scattering tint color
    pub subsurface_color: [f32; 3],
    /// Subsurface mean free path radius per channel
    pub subsurface_radius: [f32; 3],
    /// Transmission weight [0..1]
    pub transmission: f32,
    /// Transmission tint color
    pub transmission_color: [f32; 3],
    /// Transmission absorption depth
    pub transmission_depth: f32,
    /// Anisotropy strength [-1..1]
    pub anisotropy: f32,
    /// Anisotropy rotation [0..1] maps to [0..2pi]
    pub anisotropy_rotation: f32,
    /// Sheen lobe color
    pub sheen_color: [f32; 3],
    /// Sheen roughness [0..1]
    pub sheen_roughness: f32,
    /// Iridescence weight [0..1]
    pub iridescence: f32,
    /// Iridescence film IOR
    pub iridescence_ior: f32,
    /// Iridescence film thickness in nm
    pub iridescence_thickness: f32,
    // --- Texture presence flags (set when a texture is actually bound) ---
    /// Diffuse (baseColor) texture bound
    pub has_diffuse_tex: bool,
    /// Normal map texture bound
    pub has_normal_tex: bool,
    /// Roughness texture bound
    pub has_roughness_tex: bool,
    /// Metallic texture bound
    pub has_metallic_tex: bool,
    /// Opacity texture bound
    pub has_opacity_tex: bool,
    /// Emissive texture bound
    pub has_emissive_tex: bool,
    /// Occlusion texture bound
    pub has_occlusion_tex: bool,
    /// Opacity threshold for alpha cutout (0 = disabled, >0 = masked discard)
    pub opacity_threshold: f32,
    /// Opacity mode: 0 = presence (multiply all), 1 = transparent (premultiply diffuse only)
    pub opacity_mode: u32,
}

impl Default for MaterialParams {
    fn default() -> Self {
        Self {
            diffuse_color: [0.8, 0.8, 0.8], // 80% grey for visible default
            metallic: 0.0,
            roughness: 0.5,
            opacity: 1.0,
            emissive_color: [0.0, 0.0, 0.0],
            use_vertex_color: false,
            ior: 1.5,
            clearcoat: 0.0,
            clearcoat_roughness: 0.03, // typical automotive clearcoat
            use_specular_workflow: false,
            specular_color: [1.0, 1.0, 1.0],
            displacement: 0.0,
            subsurface: 0.0,
            subsurface_color: [1.0, 1.0, 1.0],
            subsurface_radius: [1.0, 0.2, 0.1],
            transmission: 0.0,
            transmission_color: [1.0, 1.0, 1.0],
            transmission_depth: 0.0,
            anisotropy: 0.0,
            anisotropy_rotation: 0.0,
            sheen_color: [0.0, 0.0, 0.0],
            sheen_roughness: 0.5,
            iridescence: 0.0,
            iridescence_ior: 1.3,
            iridescence_thickness: 400.0,
            has_diffuse_tex: false,
            has_normal_tex: false,
            has_roughness_tex: false,
            has_metallic_tex: false,
            has_opacity_tex: false,
            has_emissive_tex: false,
            has_occlusion_tex: false,
            opacity_threshold: 0.0,
            opacity_mode: 1, // C++ default: "transparent"
        }
    }
}

/// Byte layout of MaterialParams as a WGSL uniform buffer.
/// Must match the struct MaterialUniforms in generated WGSL.
///
/// Layout (all f32/u32 = 4 bytes each):
///   offset   0: diffuse_color (3xf32 = 12 bytes)
///   offset  12: metallic (f32)
///   offset  16: emissive_color (3xf32 = 12 bytes)
///   offset  28: roughness (f32)
///   offset  32: opacity (f32)
///   offset  36: ior (f32)
///   offset  40: use_vertex_color (f32, bool-as-float)
///   offset  44: clearcoat (f32)
///   offset  48: clearcoat_roughness (f32)
///   offset  52: use_specular_workflow (f32, bool-as-float)
///   offset  56: displacement (f32)
///   offset  60: subsurface (f32)
///   offset  64: specular_color (3xf32 = 12 bytes)
///   offset  76: transmission (f32)
///   offset  80: subsurface_color (3xf32 = 12 bytes)
///   offset  92: transmission_depth (f32)
///   offset  96: subsurface_radius (3xf32 = 12 bytes)
///   offset 108: anisotropy (f32)
///   offset 112: transmission_color (3xf32 = 12 bytes)
///   offset 124: anisotropy_rotation (f32)
///   offset 128: sheen_color (3xf32 = 12 bytes)
///   offset 140: sheen_roughness (f32)
///   offset 144: iridescence (f32)
///   offset 148: iridescence_ior (f32)
///   offset 152: iridescence_thickness (f32)
///   offset 156: opacity_threshold (f32)
///   offset 160: has_diffuse_tex (u32)
///   offset 164: has_normal_tex (u32)
///   offset 168: has_roughness_tex (u32)
///   offset 172: has_metallic_tex (u32)
///   offset 176: has_opacity_tex (u32)
///   offset 180: has_emissive_tex (u32)
///   offset 184: has_occlusion_tex (u32)
///   offset 188: opacity_mode (u32, 0=presence 1=transparent)
/// Total: 192 bytes (16-byte aligned)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MaterialUniformData {
    // -- row 0 (offset 0) --
    pub diffuse_color: [f32; 3],
    pub metallic: f32,
    // -- row 1 (offset 16) --
    pub emissive_color: [f32; 3],
    pub roughness: f32,
    // -- row 2 (offset 32) --
    pub opacity: f32,
    pub ior: f32,
    pub use_vertex_color: f32, // bool as float
    pub clearcoat: f32,
    // -- row 3 (offset 48) --
    pub clearcoat_roughness: f32,
    pub use_specular_workflow: f32, // bool as float
    pub displacement: f32,
    pub subsurface: f32,
    // -- row 4 (offset 64) --
    pub specular_color: [f32; 3],
    pub transmission: f32,
    // -- row 5 (offset 80) --
    pub subsurface_color: [f32; 3],
    pub transmission_depth: f32,
    // -- row 6 (offset 96) --
    pub subsurface_radius: [f32; 3],
    pub anisotropy: f32,
    // -- row 7 (offset 112) --
    pub transmission_color: [f32; 3],
    pub anisotropy_rotation: f32,
    // -- row 8 (offset 128) --
    pub sheen_color: [f32; 3],
    pub sheen_roughness: f32,
    // -- row 9 (offset 144) --
    pub iridescence: f32,
    pub iridescence_ior: f32,
    pub iridescence_thickness: f32,
    pub opacity_threshold: f32,
    // -- row 10 (offset 160) -- texture flags
    pub has_diffuse_tex: u32,
    pub has_normal_tex: u32,
    pub has_roughness_tex: u32,
    pub has_metallic_tex: u32,
    // -- row 11 (offset 176) --
    pub has_opacity_tex: u32,
    pub has_emissive_tex: u32,
    pub has_occlusion_tex: u32,
    pub opacity_mode: u32,
}

impl From<&MaterialParams> for MaterialUniformData {
    fn from(p: &MaterialParams) -> Self {
        Self {
            diffuse_color: p.diffuse_color,
            metallic: p.metallic,
            emissive_color: p.emissive_color,
            roughness: p.roughness,
            opacity: p.opacity,
            ior: p.ior,
            use_vertex_color: if p.use_vertex_color { 1.0 } else { 0.0 },
            clearcoat: p.clearcoat,
            clearcoat_roughness: p.clearcoat_roughness,
            use_specular_workflow: if p.use_specular_workflow { 1.0 } else { 0.0 },
            displacement: p.displacement,
            subsurface: p.subsurface,
            specular_color: p.specular_color,
            transmission: p.transmission,
            subsurface_color: p.subsurface_color,
            transmission_depth: p.transmission_depth,
            subsurface_radius: p.subsurface_radius,
            anisotropy: p.anisotropy,
            transmission_color: p.transmission_color,
            anisotropy_rotation: p.anisotropy_rotation,
            sheen_color: p.sheen_color,
            sheen_roughness: p.sheen_roughness,
            iridescence: p.iridescence,
            iridescence_ior: p.iridescence_ior,
            iridescence_thickness: p.iridescence_thickness,
            opacity_threshold: p.opacity_threshold,
            has_diffuse_tex: p.has_diffuse_tex as u32,
            has_normal_tex: p.has_normal_tex as u32,
            has_roughness_tex: p.has_roughness_tex as u32,
            has_metallic_tex: p.has_metallic_tex as u32,
            has_opacity_tex: p.has_opacity_tex as u32,
            has_emissive_tex: p.has_emissive_tex as u32,
            has_occlusion_tex: p.has_occlusion_tex as u32,
            opacity_mode: p.opacity_mode,
        }
    }
}

/// Generate complete WGSL shader from mesh key and material params.
pub fn gen_mesh_shader(key: &MeshShaderKey) -> WgslShaderCode {
    let mut src = String::with_capacity(8192);

    // Header comment
    writeln!(src, "// Storm WGSL - auto-generated from MeshShaderKey").unwrap();
    writeln!(
        src,
        "// shading={:?} normals={} color={} uv={}",
        key.shading, key.has_normals, key.has_color, key.has_uv
    )
    .unwrap();
    writeln!(src).unwrap();

    // Scene uniforms (group 0, binding 0) -- VP matrix, camera, ambient
    emit_scene_uniforms(&mut src);

    // Light uniforms (group 1, binding 0) -- multi-light array
    // When use_shadows=true, shadow data is packed into the same UBO.
    if key.shading != ShadingModel::FlatColor {
        lighting::emit_light_uniforms_wgsl_with_shadows(
            &mut src,
            slots::LIGHT_GROUP,
            slots::LIGHT_UNIFORMS_BINDING,
            key.use_shadows,
        );
    }

    // Material uniforms (group 2, binding 0) -- UsdPreviewSurface params + tex flags
    if key.shading != ShadingModel::FlatColor {
        emit_material_uniforms(&mut src);
    }

    // Texture bindings (group 3) -- 7 texture+sampler pairs, only when UV data is present.
    // Naga reflection only includes bindings that are actually referenced in the shader code,
    // so if has_uv=false the sampling calls are absent and the group must not be declared.
    if key.shading != ShadingModel::FlatColor && key.has_uv {
        emit_texture_bindings(&mut src);
    }

    // IBL bindings -- irradiance cube, prefilter cube, BRDF LUT.
    // Group index is dynamic: group 3 when !has_uv (no texture gap), group 4 when has_uv.
    // Only emitted when a DomeLight with texture is present and shading is PBR.
    if key.has_ibl && key.shading == ShadingModel::Pbr {
        let ibl_group = slots::ibl_group(key.has_uv);
        emit_ibl_bindings(&mut src, ibl_group);
    }

    // Shadow atlas bindings -- depth texture array + comparison sampler.
    // Group index is dynamic: placed after the last used group to stay contiguous.
    if key.use_shadows && key.shading != ShadingModel::FlatColor {
        let sg = slots::shadow_group(key.has_uv, key.has_ibl);
        lighting::emit_shadow_atlas_wgsl(&mut src, sg);
    }

    // Instance transforms SSBO -- storage buffer of mat4x4<f32> indexed by instance_index.
    // Group placed after all other dynamic groups to stay contiguous.
    if key.use_instancing {
        let ig = slots::instance_group(key.has_uv, key.has_ibl, key.use_shadows);
        emit_instance_xforms_ssbo(&mut src, ig);
    }

    // Deep-pick SSBO for resolveDeep-style shader-side writes.
    // Reuses the flat-color pick variant's otherwise-unused light group slot.
    if key.pick_buffer_rw {
        emit_pick_buffer_ssbo(&mut src);
    }

    if uses_face_varying_storage(key) {
        emit_face_varying_ssbo(&mut src, key);
        emit_face_varying_helpers(&mut src);
    }

    // Vertex input/output structs
    emit_vertex_structs(&mut src, key);

    // Vertex shader
    emit_vertex_shader(&mut src, key);

    // PBR helpers + light eval functions (must precede fragment shader)
    if matches!(
        key.shading,
        ShadingModel::Pbr
            | ShadingModel::BlinnPhong
            | ShadingModel::GeomFlat
            | ShadingModel::GeomSmooth
    ) {
        emit_pbr_helpers(&mut src);
        lighting::emit_light_eval_functions(&mut src);
        // Shadow PCF sampling function (must follow shadow uniforms declaration)
        // Matches C++ simpleLighting.glslfx shadowCompare() + shadowFilter().
        if key.use_shadows {
            lighting::emit_shadow_pcf_function(&mut src);
        }
        // IBL sampling helpers (need ibl_* texture vars, so after IBL bindings)
        if key.has_ibl && key.shading == ShadingModel::Pbr {
            emit_ibl_helpers(&mut src);
        }
    }

    // sRGB conversion helper (needed for texture sampling)
    if key.shading != ShadingModel::FlatColor {
        emit_srgb_helpers(&mut src);
    }

    // Fragment shader
    emit_fragment_shader(&mut src, key);

    WgslShaderCode {
        source: src,
        vs_entry: "vs_main",
        fs_entry: "fs_main",
    }
}

/// Generate WGSL for Storm points.
///
/// This is a dedicated non-mesh program family. The previous Rust path routed
/// points through `MeshShaderKey`, which made the live pipeline depend on mesh
/// attribute packing and mesh-only shader features. The dedicated points path
/// keeps the draw contract honest even though WebGPU still limits the backend
/// to rasterized point primitives rather than OpenUSD's full GLSL `gl_PointSize`
/// feature set.
pub fn gen_points_shader(key: &PointsProgramKey) -> WgslShaderCode {
    let mut src = String::with_capacity(4096);
    writeln!(src, "// Storm WGSL - auto-generated for points").unwrap();
    writeln!(src).unwrap();
    emit_scene_uniforms(&mut src);
    emit_material_uniforms(&mut src);
    if key.pick_buffer_rw {
        emit_pick_buffer_ssbo(&mut src);
    }

    writeln!(src, "struct VertexInput {{").unwrap();
    writeln!(src, "    @location(0) position: vec3<f32>,").unwrap();
    if key.has_widths {
        writeln!(src, "    @location(1) width: f32,").unwrap();
    }
    if key.has_color {
        writeln!(src, "    @location(2) color: vec3<f32>,").unwrap();
    }
    if key.use_instancing {
        writeln!(src, "    @builtin(instance_index) instance_id: u32,").unwrap();
    }
    writeln!(src, "}}").unwrap();
    writeln!(src, "struct VertexOutput {{").unwrap();
    writeln!(src, "    @builtin(position) position: vec4<f32>,").unwrap();
    writeln!(src, "    @location(0) color: vec3<f32>,").unwrap();
    if key.use_instancing || key.pick_buffer_rw {
        writeln!(src, "    @location(1) instance_id: u32,").unwrap();
    }
    writeln!(src, "}}").unwrap();
    writeln!(src).unwrap();

    writeln!(src, "@vertex").unwrap();
    writeln!(src, "fn vs_main(in: VertexInput) -> VertexOutput {{").unwrap();
    writeln!(src, "    var out: VertexOutput;").unwrap();
    writeln!(src, "    let model = scene.model;").unwrap();
    writeln!(src, "    let world_pos = (model * vec4<f32>(in.position, 1.0)).xyz;").unwrap();
    writeln!(src, "    out.position = scene.view_proj * vec4<f32>(world_pos, 1.0);").unwrap();
    if key.has_color {
        writeln!(src, "    out.color = in.color;").unwrap();
    } else {
        writeln!(src, "    out.color = material.diffuse_color;").unwrap();
    }
    if key.use_instancing || key.pick_buffer_rw {
        let src_id = if key.use_instancing { "in.instance_id" } else { "0u" };
        writeln!(src, "    out.instance_id = {};", src_id).unwrap();
    }
    writeln!(src, "    return out;").unwrap();
    writeln!(src, "}}").unwrap();
    writeln!(src).unwrap();

    if !key.depth_only {
        emit_selection_highlight_fn(&mut src);
    }
    writeln!(src, "@fragment").unwrap();
    if key.use_instancing || key.pick_buffer_rw {
        writeln!(
            src,
            "fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {{"
        )
        .unwrap();
    } else {
        writeln!(src, "fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {{").unwrap();
    }
    if key.pick_buffer_rw {
        writeln!(
            src,
            "    render_deep_picks(decode_pick_prim_id(scene.ambient_color), i32(in.instance_id), -1, -1, -1);"
        )
        .unwrap();
        writeln!(src, "    return scene.ambient_color;").unwrap();
    } else if key.depth_only {
        writeln!(src, "    return vec4<f32>(in.color, 1.0);").unwrap();
    } else {
        writeln!(src, "    return apply_selection(vec4<f32>(in.color, 1.0));").unwrap();
    }
    writeln!(src, "}}").unwrap();
    WgslShaderCode {
        source: src,
        vs_entry: "vs_main",
        fs_entry: "fs_main",
    }
}

/// Generate WGSL for retained basis curves.
///
/// `_ref` uses `HdSt_BasisCurvesShaderKey` instead of the mesh shader family.
/// The Rust wgpu backend still draws only the retained wire/points variants,
/// but they must nevertheless keep their own program/binding layout so curve
/// data is not interpreted with mesh packing rules.
pub fn gen_basis_curves_shader(key: &BasisCurvesProgramKey) -> WgslShaderCode {
    let mut src = String::with_capacity(4096);
    writeln!(src, "// Storm WGSL - auto-generated for basis curves").unwrap();
    writeln!(src).unwrap();
    emit_scene_uniforms(&mut src);
    emit_material_uniforms(&mut src);
    if key.pick_buffer_rw {
        emit_pick_buffer_ssbo(&mut src);
    }

    writeln!(src, "struct VertexInput {{").unwrap();
    writeln!(src, "    @location(0) position: vec3<f32>,").unwrap();
    if key.has_widths {
        writeln!(src, "    @location(1) width: f32,").unwrap();
    }
    if key.has_normals {
        writeln!(src, "    @location(2) normal: vec3<f32>,").unwrap();
    }
    if key.has_color {
        writeln!(src, "    @location(3) color: vec3<f32>,").unwrap();
    }
    if key.use_instancing {
        writeln!(src, "    @builtin(instance_index) instance_id: u32,").unwrap();
    }
    writeln!(src, "}}").unwrap();
    writeln!(src, "struct VertexOutput {{").unwrap();
    writeln!(src, "    @builtin(position) position: vec4<f32>,").unwrap();
    writeln!(src, "    @location(0) color: vec3<f32>,").unwrap();
    if key.use_instancing || key.pick_buffer_rw {
        writeln!(src, "    @location(1) instance_id: u32,").unwrap();
    }
    writeln!(src, "}}").unwrap();
    writeln!(src).unwrap();

    writeln!(src, "@vertex").unwrap();
    writeln!(src, "fn vs_main(in: VertexInput) -> VertexOutput {{").unwrap();
    writeln!(src, "    var out: VertexOutput;").unwrap();
    writeln!(src, "    let model = scene.model;").unwrap();
    writeln!(src, "    let world_pos = (model * vec4<f32>(in.position, 1.0)).xyz;").unwrap();
    writeln!(src, "    out.position = scene.view_proj * vec4<f32>(world_pos, 1.0);").unwrap();
    if key.has_color {
        writeln!(src, "    out.color = in.color;").unwrap();
    } else {
        writeln!(src, "    out.color = material.diffuse_color;").unwrap();
    }
    if key.use_instancing || key.pick_buffer_rw {
        let src_id = if key.use_instancing { "in.instance_id" } else { "0u" };
        writeln!(src, "    out.instance_id = {};", src_id).unwrap();
    }
    writeln!(src, "    return out;").unwrap();
    writeln!(src, "}}").unwrap();
    writeln!(src).unwrap();

    if !key.depth_only {
        emit_selection_highlight_fn(&mut src);
    }
    writeln!(src, "@fragment").unwrap();
    writeln!(src, "fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {{").unwrap();
    if key.pick_buffer_rw {
        writeln!(
            src,
            "    render_deep_picks(decode_pick_prim_id(scene.ambient_color), i32(in.instance_id), -1, -1, -1);"
        )
        .unwrap();
        writeln!(src, "    return scene.ambient_color;").unwrap();
    } else if key.depth_only {
        writeln!(src, "    return vec4<f32>(in.color, 1.0);").unwrap();
    } else {
        writeln!(src, "    return apply_selection(vec4<f32>(in.color, 1.0));").unwrap();
    }
    writeln!(src, "}}").unwrap();
    WgslShaderCode {
        source: src,
        vs_entry: "vs_main",
        fs_entry: "fs_main",
    }
}

// --- Private codegen helpers ---

fn emit_scene_uniforms(src: &mut String) {
    writeln!(src, "// Scene-level uniforms: camera + ambient + selection").unwrap();
    writeln!(src, "struct SceneUniforms {{").unwrap();
    writeln!(src, "    view_proj: mat4x4<f32>,").unwrap();
    writeln!(src, "    model: mat4x4<f32>,").unwrap();
    writeln!(src, "    ambient_color: vec4<f32>,").unwrap();
    writeln!(src, "    camera_pos: vec4<f32>,").unwrap();
    // selection_color.a > 0 means this prim is selected; rgb = highlight tint
    writeln!(src, "    selection_color: vec4<f32>,").unwrap();
    writeln!(src, "    fvar_base_words: u32,").unwrap();
    writeln!(src, "    _fvar_pad0: u32,").unwrap();
    writeln!(src, "    _fvar_pad1: u32,").unwrap();
    writeln!(src, "    _fvar_pad2: u32,").unwrap();
    writeln!(src, "}};").unwrap();
    writeln!(
        src,
        "@group({}) @binding({})",
        slots::SCENE_GROUP,
        slots::SCENE_UNIFORMS_BINDING
    )
    .unwrap();
    writeln!(src, "var<uniform> scene: SceneUniforms;").unwrap();
    writeln!(src).unwrap();
}

fn uses_face_varying_storage(key: &MeshShaderKey) -> bool {
    key.has_fvar_normals || key.has_fvar_uv || key.has_fvar_color || key.has_fvar_opacity
}

fn emit_face_varying_ssbo(src: &mut String, key: &MeshShaderKey) {
    let group = slots::face_varying_group(
        key.has_uv,
        key.has_ibl,
        key.use_shadows,
        key.use_instancing,
    );
    writeln!(src, "// Packed face-varying payload: [header u32s | float words]").unwrap();
    writeln!(
        src,
        "@group({}) @binding({}) var<storage, read> face_varying_data: array<u32>;",
        group,
        slots::FACE_VARYING_BINDING
    )
    .unwrap();
    writeln!(src).unwrap();
}

fn emit_face_varying_helpers(src: &mut String) {
    writeln!(src, "const FVAR_SLOT_NORMALS: u32 = 0u;").unwrap();
    writeln!(src, "const FVAR_SLOT_UV: u32 = 1u;").unwrap();
    writeln!(src, "const FVAR_SLOT_COLOR: u32 = 2u;").unwrap();
    writeln!(src, "const FVAR_SLOT_OPACITY: u32 = 3u;").unwrap();
    writeln!(src, "fn fvar_word(idx: u32) -> f32 {{").unwrap();
    writeln!(src, "    return bitcast<f32>(face_varying_data[idx]);").unwrap();
    writeln!(src, "}}").unwrap();
    writeln!(src, "fn fvar_channel_base(slot: u32) -> u32 {{").unwrap();
    writeln!(src, "    return scene.fvar_base_words + face_varying_data[scene.fvar_base_words + slot];").unwrap();
    writeln!(src, "}}").unwrap();
    writeln!(src, "fn load_fvar_vec3(slot: u32, local_index: u32) -> vec3<f32> {{").unwrap();
    writeln!(src, "    let base = fvar_channel_base(slot) + local_index * 3u;").unwrap();
    writeln!(
        src,
        "    return vec3<f32>(fvar_word(base), fvar_word(base + 1u), fvar_word(base + 2u));"
    )
    .unwrap();
    writeln!(src, "}}").unwrap();
    writeln!(src, "fn load_fvar_vec2(slot: u32, local_index: u32) -> vec2<f32> {{").unwrap();
    writeln!(src, "    let base = fvar_channel_base(slot) + local_index * 2u;").unwrap();
    writeln!(src, "    return vec2<f32>(fvar_word(base), fvar_word(base + 1u));").unwrap();
    writeln!(src, "}}").unwrap();
    writeln!(src, "fn load_fvar_scalar(slot: u32, local_index: u32) -> f32 {{").unwrap();
    writeln!(src, "    let base = fvar_channel_base(slot) + local_index;").unwrap();
    writeln!(src, "    return fvar_word(base);").unwrap();
    writeln!(src, "}}").unwrap();
    writeln!(src).unwrap();
}

fn emit_material_uniforms(src: &mut String) {
    writeln!(
        src,
        "// Material parameters (UsdPreviewSurface) + texture presence flags"
    )
    .unwrap();
    writeln!(src, "struct MaterialUniforms {{").unwrap();
    writeln!(src, "    diffuse_color: vec3<f32>,").unwrap();
    writeln!(src, "    metallic: f32,").unwrap();
    writeln!(src, "    emissive_color: vec3<f32>,").unwrap();
    writeln!(src, "    roughness: f32,").unwrap();
    writeln!(src, "    opacity: f32,").unwrap();
    writeln!(src, "    ior: f32,").unwrap();
    writeln!(src, "    use_vertex_color: f32,").unwrap();
    writeln!(src, "    clearcoat: f32,").unwrap();
    writeln!(src, "    clearcoat_roughness: f32,").unwrap();
    writeln!(src, "    use_specular_workflow: f32,").unwrap();
    writeln!(src, "    displacement: f32,").unwrap();
    writeln!(src, "    subsurface: f32,").unwrap();
    writeln!(src, "    specular_color: vec3<f32>,").unwrap();
    writeln!(src, "    transmission: f32,").unwrap();
    // Advanced PBR: subsurface / transmission / anisotropy / sheen / iridescence
    writeln!(src, "    subsurface_color: vec3<f32>,").unwrap();
    writeln!(src, "    transmission_depth: f32,").unwrap();
    writeln!(src, "    subsurface_radius: vec3<f32>,").unwrap();
    writeln!(src, "    anisotropy: f32,").unwrap();
    writeln!(src, "    transmission_color: vec3<f32>,").unwrap();
    writeln!(src, "    anisotropy_rotation: f32,").unwrap();
    writeln!(src, "    sheen_color: vec3<f32>,").unwrap();
    writeln!(src, "    sheen_roughness: f32,").unwrap();
    writeln!(src, "    iridescence: f32,").unwrap();
    writeln!(src, "    iridescence_ior: f32,").unwrap();
    writeln!(src, "    iridescence_thickness: f32,").unwrap();
    writeln!(src, "    opacity_threshold: f32,").unwrap();
    // Texture presence flags as u32 (0=no texture, 1=texture bound)
    writeln!(src, "    has_diffuse_tex: u32,").unwrap();
    writeln!(src, "    has_normal_tex: u32,").unwrap();
    writeln!(src, "    has_roughness_tex: u32,").unwrap();
    writeln!(src, "    has_metallic_tex: u32,").unwrap();
    writeln!(src, "    has_opacity_tex: u32,").unwrap();
    writeln!(src, "    has_emissive_tex: u32,").unwrap();
    writeln!(src, "    has_occlusion_tex: u32,").unwrap();
    writeln!(src, "    opacity_mode: u32,").unwrap();
    writeln!(src, "}};").unwrap();
    writeln!(
        src,
        "@group({}) @binding({})",
        slots::MATERIAL_GROUP,
        slots::MATERIAL_PARAMS_BINDING
    )
    .unwrap();
    writeln!(src, "var<uniform> material: MaterialUniforms;").unwrap();
    writeln!(src).unwrap();
}

/// Emit all 7 texture+sampler pairs in group 3 (bindings 0-13).
///
/// These are ALWAYS emitted for lit shaders so the bind group layout is
/// stable. Missing textures are covered by 1x1 white fallback at bind time.
fn emit_texture_bindings(src: &mut String) {
    writeln!(
        src,
        "// Group 3: Per-material texture bindings (7 slots, always present)"
    )
    .unwrap();
    // Diffuse (baseColor) -- slots 0,1
    writeln!(
        src,
        "@group({}) @binding({}) var diffuse_tex: texture_2d<f32>;",
        slots::TEXTURE_GROUP,
        slots::DIFFUSE_TEX_BINDING
    )
    .unwrap();
    writeln!(
        src,
        "@group({}) @binding({}) var diffuse_sampler: sampler;",
        slots::TEXTURE_GROUP,
        slots::DIFFUSE_SAMPLER_BINDING
    )
    .unwrap();
    // Normal map -- slots 2,3
    writeln!(
        src,
        "@group({}) @binding({}) var normal_tex: texture_2d<f32>;",
        slots::TEXTURE_GROUP,
        slots::NORMAL_TEX_BINDING
    )
    .unwrap();
    writeln!(
        src,
        "@group({}) @binding({}) var normal_sampler: sampler;",
        slots::TEXTURE_GROUP,
        slots::NORMAL_SAMPLER_BINDING
    )
    .unwrap();
    // Roughness -- slots 4,5
    writeln!(
        src,
        "@group({}) @binding({}) var roughness_tex: texture_2d<f32>;",
        slots::TEXTURE_GROUP,
        slots::ROUGHNESS_TEX_BINDING
    )
    .unwrap();
    writeln!(
        src,
        "@group({}) @binding({}) var roughness_sampler: sampler;",
        slots::TEXTURE_GROUP,
        slots::ROUGHNESS_SAMPLER_BINDING
    )
    .unwrap();
    // Metallic -- slots 6,7
    writeln!(
        src,
        "@group({}) @binding({}) var metallic_tex: texture_2d<f32>;",
        slots::TEXTURE_GROUP,
        slots::METALLIC_TEX_BINDING
    )
    .unwrap();
    writeln!(
        src,
        "@group({}) @binding({}) var metallic_sampler: sampler;",
        slots::TEXTURE_GROUP,
        slots::METALLIC_SAMPLER_BINDING
    )
    .unwrap();
    // Opacity -- slots 8,9
    writeln!(
        src,
        "@group({}) @binding({}) var opacity_tex: texture_2d<f32>;",
        slots::TEXTURE_GROUP,
        slots::OPACITY_TEX_BINDING
    )
    .unwrap();
    writeln!(
        src,
        "@group({}) @binding({}) var opacity_sampler: sampler;",
        slots::TEXTURE_GROUP,
        slots::OPACITY_SAMPLER_BINDING
    )
    .unwrap();
    // Emissive -- slots 10,11
    writeln!(
        src,
        "@group({}) @binding({}) var emissive_tex: texture_2d<f32>;",
        slots::TEXTURE_GROUP,
        slots::EMISSIVE_TEX_BINDING
    )
    .unwrap();
    writeln!(
        src,
        "@group({}) @binding({}) var emissive_sampler: sampler;",
        slots::TEXTURE_GROUP,
        slots::EMISSIVE_SAMPLER_BINDING
    )
    .unwrap();
    // Occlusion -- slots 12,13
    writeln!(
        src,
        "@group({}) @binding({}) var occlusion_tex: texture_2d<f32>;",
        slots::TEXTURE_GROUP,
        slots::OCCLUSION_TEX_BINDING
    )
    .unwrap();
    writeln!(
        src,
        "@group({}) @binding({}) var occlusion_sampler: sampler;",
        slots::TEXTURE_GROUP,
        slots::OCCLUSION_SAMPLER_BINDING
    )
    .unwrap();
    writeln!(src).unwrap();

    // Emit a helper that references every texture+sampler pair so naga reflection
    // keeps all 14 bindings in the BGL. Without this, naga strips unused pairs
    // and create_bind_group panics on count mismatch.
    writeln!(src, "fn _touch_textures() -> f32 {{").unwrap();
    writeln!(src, "    let uv = vec2<f32>(0.0);").unwrap();
    writeln!(src, "    var s = 0.0;").unwrap();
    for (tex, smp) in [
        ("diffuse_tex", "diffuse_sampler"),
        ("normal_tex", "normal_sampler"),
        ("roughness_tex", "roughness_sampler"),
        ("metallic_tex", "metallic_sampler"),
        ("opacity_tex", "opacity_sampler"),
        ("emissive_tex", "emissive_sampler"),
        ("occlusion_tex", "occlusion_sampler"),
    ] {
        writeln!(
            src,
            "    s += textureSampleLevel({}, {}, uv, 0.0).r;",
            tex, smp
        )
        .unwrap();
    }
    writeln!(src, "    return s;").unwrap();
    writeln!(src, "}}").unwrap();
    writeln!(src).unwrap();
}

/// Emit IBL texture declarations in group 4.
///
/// Uses texture_2d_array (6 layers) for cubemaps since WGSL doesn't have texture_cube.
/// Irradiance and prefilter use 6-layer arrays; BRDF LUT is a plain texture_2d.
fn emit_ibl_bindings(src: &mut String, group: u32) {
    writeln!(
        src,
        "// Group {}: IBL textures (dome light, only when has_ibl=true)",
        group
    )
    .unwrap();
    // Irradiance cubemap stored as 6-layer 2d array
    writeln!(
        src,
        "@group({}) @binding({}) var ibl_irradiance: texture_2d_array<f32>;",
        group,
        slots::IBL_IRRADIANCE_TEX_BINDING
    )
    .unwrap();
    writeln!(
        src,
        "@group({}) @binding({}) var ibl_irradiance_sampler: sampler;",
        group,
        slots::IBL_IRRADIANCE_SAMPLER_BINDING
    )
    .unwrap();
    // Prefiltered specular cubemap
    writeln!(
        src,
        "@group({}) @binding({}) var ibl_prefilter: texture_2d_array<f32>;",
        group,
        slots::IBL_PREFILTER_TEX_BINDING
    )
    .unwrap();
    writeln!(
        src,
        "@group({}) @binding({}) var ibl_prefilter_sampler: sampler;",
        group,
        slots::IBL_PREFILTER_SAMPLER_BINDING
    )
    .unwrap();
    // BRDF LUT (2D)
    writeln!(
        src,
        "@group({}) @binding({}) var ibl_brdf_lut: texture_2d<f32>;",
        group,
        slots::IBL_BRDF_LUT_TEX_BINDING
    )
    .unwrap();
    writeln!(
        src,
        "@group({}) @binding({}) var ibl_brdf_lut_sampler: sampler;",
        group,
        slots::IBL_BRDF_LUT_SAMPLER_BINDING
    )
    .unwrap();
    writeln!(src).unwrap();
}

/// Emit instance transforms storage buffer declaration.
fn emit_instance_xforms_ssbo(src: &mut String, group: u32) {
    writeln!(
        src,
        "// Per-instance model transforms (GPU instancing SSBO)"
    )
    .unwrap();
    writeln!(
        src,
        "@group({}) @binding({}) var<storage, read> instance_xforms: array<mat4x4<f32>>;",
        group,
        slots::INSTANCE_XFORMS_BINDING
    )
    .unwrap();
    writeln!(src).unwrap();
}

/// Emit the resolveDeep pick-buffer storage declaration and helpers.
fn emit_pick_buffer_ssbo(src: &mut String) {
    writeln!(src, "// Deep-pick storage buffer (OpenUSD PickBuffer analogue)").unwrap();
    writeln!(src, "struct PickBufferData {{").unwrap();
    writeln!(src, "    data: array<atomic<i32>>,").unwrap();
    writeln!(src, "}};").unwrap();
    writeln!(
        src,
        "@group({}) @binding({}) var<storage, read_write> pick_buffer: PickBufferData;",
        slots::PICK_BUFFER_GROUP,
        slots::PICK_BUFFER_BINDING
    )
    .unwrap();
    writeln!(src).unwrap();
    writeln!(src, "fn pick_load(index: i32) -> i32 {{").unwrap();
    writeln!(src, "    return atomicLoad(&pick_buffer.data[u32(index)]);").unwrap();
    writeln!(src, "}}").unwrap();
    writeln!(src).unwrap();
    writeln!(src, "fn pick_compare_swap(index: i32, expected: i32, value: i32) -> i32 {{")
        .unwrap();
    writeln!(
        src,
        "    return atomicCompareExchangeWeak(&pick_buffer.data[u32(index)], expected, value).old_value;"
    )
    .unwrap();
    writeln!(src, "}}").unwrap();
    writeln!(src).unwrap();
    writeln!(src, "fn cantor(a: i32, b: i32) -> i32 {{").unwrap();
    writeln!(src, "    return ((a + b + 1) * (a + b)) / 2 + b;").unwrap();
    writeln!(src, "}}").unwrap();
    writeln!(src).unwrap();
    writeln!(src, "fn hash3(a: i32, b: i32, c: i32) -> i32 {{").unwrap();
    writeln!(src, "    return cantor(a, cantor(b, c));").unwrap();
    writeln!(src, "}}").unwrap();
    writeln!(src).unwrap();
    writeln!(
        src,
        "fn decode_pick_prim_id(color: vec4<f32>) -> i32 {{"
    )
    .unwrap();
    writeln!(
        src,
        "    let r = u32(round(clamp(color.x, 0.0, 1.0) * 255.0));"
    )
    .unwrap();
    writeln!(
        src,
        "    let g = u32(round(clamp(color.y, 0.0, 1.0) * 255.0));"
    )
    .unwrap();
    writeln!(
        src,
        "    let b = u32(round(clamp(color.z, 0.0, 1.0) * 255.0));"
    )
    .unwrap();
    writeln!(
        src,
        "    let a = u32(round(clamp(color.w, 0.0, 1.0) * 255.0));"
    )
    .unwrap();
    writeln!(
        src,
        "    return bitcast<i32>(r | (g << 8u) | (b << 16u) | (a << 24u));"
    )
    .unwrap();
    writeln!(src, "}}").unwrap();
    writeln!(src).unwrap();
    writeln!(
        src,
        "fn compare_or_set(value_offset: i32, prim_id: i32, instance_id: i32, part_id: i32) -> bool {{"
    )
    .unwrap();
    writeln!(src, "    let prim_id_offset = value_offset;").unwrap();
    writeln!(src, "    let instance_id_offset = value_offset + 1;").unwrap();
    writeln!(src, "    let part_id_offset = value_offset + 2;").unwrap();
    writeln!(src, "    let prim_id_value = pick_load(prim_id_offset);").unwrap();
    writeln!(
        src,
        "    let instance_id_value = pick_load(instance_id_offset);"
    )
    .unwrap();
    writeln!(src, "    let part_id_value = pick_load(part_id_offset);").unwrap();
    writeln!(
        src,
        "    if ((prim_id_value != -9) && (instance_id_value != -9) && (part_id_value != -9)) {{"
    )
    .unwrap();
    writeln!(
        src,
        "        return (prim_id == prim_id_value) && (instance_id == instance_id_value) && (part_id == part_id_value);"
    )
    .unwrap();
    writeln!(src, "    }}").unwrap();
    writeln!(
        src,
        "    let prim_prev = pick_compare_swap(prim_id_offset, -9, prim_id);"
    )
    .unwrap();
    writeln!(
        src,
        "    if (prim_prev != -9 && prim_prev != prim_id) {{"
    )
    .unwrap();
    writeln!(src, "        return false;").unwrap();
    writeln!(src, "    }}").unwrap();
    writeln!(
        src,
        "    let instance_prev = pick_compare_swap(instance_id_offset, -9, instance_id);"
    )
    .unwrap();
    writeln!(
        src,
        "    if (instance_prev != -9 && instance_prev != instance_id) {{"
    )
    .unwrap();
    writeln!(src, "        return false;").unwrap();
    writeln!(src, "    }}").unwrap();
    writeln!(
        src,
        "    let part_prev = pick_compare_swap(part_id_offset, -9, part_id);"
    )
    .unwrap();
    writeln!(src, "    if (part_prev != -9 && part_prev != part_id) {{").unwrap();
    writeln!(src, "        return false;").unwrap();
    writeln!(src, "    }}").unwrap();
    writeln!(src, "    return true;").unwrap();
    writeln!(src, "}}").unwrap();
    writeln!(src).unwrap();
    writeln!(
        src,
        "fn render_deep_picks(prim_id: i32, instance_id: i32, element_id: i32, edge_id: i32, point_id: i32) {{"
    )
    .unwrap();
    writeln!(
        src,
        "    if (pick_load(0) == 0 || prim_id < 0) {{"
    )
    .unwrap();
    writeln!(src, "        return;").unwrap();
    writeln!(src, "    }}").unwrap();
    writeln!(src, "    let entry_size = 3;").unwrap();
    writeln!(src, "    let num_sub_buffers = pick_load(0);").unwrap();
    writeln!(src, "    let sub_buffer_capacity = pick_load(1);").unwrap();
    writeln!(src, "    let table_offset = pick_load(2);").unwrap();
    writeln!(src, "    let storage_offset = pick_load(3);").unwrap();
    writeln!(
        src,
        "    let part_id = pick_load(4) * element_id + pick_load(5) * edge_id + pick_load(6) * point_id;"
    )
    .unwrap();
    writeln!(src, "    if (num_sub_buffers <= 0 || sub_buffer_capacity <= 0) {{").unwrap();
    writeln!(src, "        return;").unwrap();
    writeln!(src, "    }}").unwrap();
    writeln!(src, "    let hash_value = hash3(prim_id, instance_id, part_id);").unwrap();
    writeln!(
        src,
        "    let sub_buffer_number = ((hash_value % num_sub_buffers) + num_sub_buffers) % num_sub_buffers;"
    )
    .unwrap();
    writeln!(src, "    var buffer_number = sub_buffer_number;").unwrap();
    writeln!(src, "    loop {{").unwrap();
    writeln!(src, "        let size_offset = table_offset + buffer_number;").unwrap();
    writeln!(
        src,
        "        let sub_buffer_offset = storage_offset + buffer_number * sub_buffer_capacity * entry_size;"
    )
    .unwrap();
    writeln!(src, "        var entry_number = 0;").unwrap();
    writeln!(src, "        loop {{").unwrap();
    writeln!(
        src,
        "            if (entry_number == pick_load(size_offset)) {{"
    )
    .unwrap();
    writeln!(
        src,
        "                let _prev = pick_compare_swap(size_offset, entry_number, entry_number + 1);"
    )
    .unwrap();
    writeln!(src, "            }}").unwrap();
    writeln!(
        src,
        "            if (compare_or_set(sub_buffer_offset + entry_number * entry_size, prim_id, instance_id, part_id)) {{"
    )
    .unwrap();
    writeln!(src, "                return;").unwrap();
    writeln!(src, "            }}").unwrap();
    writeln!(src, "            entry_number = entry_number + 1;").unwrap();
    writeln!(
        src,
        "            if (entry_number == sub_buffer_capacity) {{"
    )
    .unwrap();
    writeln!(src, "                break;").unwrap();
    writeln!(src, "            }}").unwrap();
    writeln!(src, "        }}").unwrap();
    writeln!(
        src,
        "        buffer_number = (buffer_number + 1) % num_sub_buffers;"
    )
    .unwrap();
    writeln!(
        src,
        "        if (buffer_number == sub_buffer_number) {{"
    )
    .unwrap();
    writeln!(src, "            break;").unwrap();
    writeln!(src, "        }}").unwrap();
    writeln!(src, "    }}").unwrap();
    writeln!(src, "}}").unwrap();
    writeln!(src).unwrap();
}

fn emit_vertex_structs(src: &mut String, key: &MeshShaderKey) {
    // VertexInput
    writeln!(src, "struct VertexInput {{").unwrap();
    writeln!(
        src,
        "    @location({}) position: vec3<f32>,",
        slots::POSITION_LOCATION
    )
    .unwrap();
    if key.has_normals && !key.has_fvar_normals {
        writeln!(
            src,
            "    @location({}) normal: vec3<f32>,",
            slots::NORMAL_LOCATION
        )
        .unwrap();
    }
    if key.has_uv && !key.has_fvar_uv {
        writeln!(src, "    @location({}) uv: vec2<f32>,", slots::UV_LOCATION).unwrap();
    }
    if key.has_color && !key.has_fvar_color {
        writeln!(
            src,
            "    @location({}) color: vec3<f32>,",
            slots::COLOR_LOCATION
        )
        .unwrap();
    }
    writeln!(src, "}};").unwrap();
    writeln!(src).unwrap();

    // VertexOutput (varyings passed to fragment)
    // needs_world_pos: required for any lit shading (Blinn-Phong, PBR, GeomFlat/Smooth)
    // and for flat_shading face normals (dpdx/dpdy of world_pos).
    let needs_world_pos = key.has_normals
        || key.flat_shading
        || matches!(
            key.shading,
            ShadingModel::BlinnPhong
                | ShadingModel::Pbr
                | ShadingModel::GeomFlat
                | ShadingModel::GeomSmooth
        );
    writeln!(src, "struct VertexOutput {{").unwrap();
    writeln!(src, "    @builtin(position) clip_position: vec4<f32>,").unwrap();
    // NOTE: @builtin(front_facing) is fragment-only, cannot be in vertex output struct.
    // It's passed as a separate parameter to fs_main instead.
    let mut next_loc = 0u32;
    if key.has_normals {
        writeln!(src, "    @location({}) world_normal: vec3<f32>,", next_loc).unwrap();
        next_loc += 1;
    }
    if needs_world_pos {
        writeln!(src, "    @location({}) world_pos: vec3<f32>,", next_loc).unwrap();
        next_loc += 1;
    }
    if key.has_uv {
        writeln!(src, "    @location({}) uv: vec2<f32>,", next_loc).unwrap();
        next_loc += 1;
    }
    if key.has_color {
        writeln!(src, "    @location({}) color: vec3<f32>,", next_loc).unwrap();
        next_loc += 1;
    }
    if key.has_fvar_opacity {
        writeln!(src, "    @location({}) opacity: f32,", next_loc).unwrap();
        next_loc += 1;
    }
    if key.pick_buffer_rw && key.use_instancing {
        writeln!(
            src,
            "    @interpolate(flat) @location({}) instance_id: u32,",
            next_loc
        )
        .unwrap();
        next_loc += 1;
    }
    let _ = next_loc;
    writeln!(src, "}};").unwrap();
    writeln!(src).unwrap();
}

fn emit_vertex_shader(src: &mut String, key: &MeshShaderKey) {
    writeln!(src, "@vertex").unwrap();
    // When instancing, add @builtin(instance_index) as second parameter
    if key.use_instancing {
        writeln!(src, "fn vs_main(in: VertexInput, @builtin(vertex_index) vertex_index: u32, @builtin(instance_index) instance_id: u32) -> VertexOutput {{").unwrap();
    } else {
        writeln!(src, "fn vs_main(in: VertexInput, @builtin(vertex_index) vertex_index: u32) -> VertexOutput {{").unwrap();
    }
    writeln!(src, "    var out: VertexOutput;").unwrap();

    // Model matrix: from SSBO when instanced, from scene.model when not
    if key.use_instancing {
        writeln!(
            src,
            "    // GPU instancing: model matrix from per-instance SSBO"
        )
        .unwrap();
        writeln!(src, "    let model = instance_xforms[instance_id];").unwrap();
    } else {
        writeln!(src, "    let model = scene.model;").unwrap();
    }

    if key.has_fvar_normals {
        writeln!(
            src,
            "    // The current Rust fvar BAR is triangle-vertex ordered, so vertex_index"
        )
        .unwrap();
        writeln!(
            src,
            "    // addresses the matching face-varying tuple directly for triangle-list draws."
        )
        .unwrap();
        writeln!(
            src,
            "    let input_normal = load_fvar_vec3(FVAR_SLOT_NORMALS, vertex_index);"
        )
        .unwrap();
    }
    if key.has_fvar_uv {
        writeln!(src, "    let input_uv = load_fvar_vec2(FVAR_SLOT_UV, vertex_index);").unwrap();
    }
    if key.has_fvar_color {
        writeln!(src, "    let input_color = load_fvar_vec3(FVAR_SLOT_COLOR, vertex_index);").unwrap();
    }
    if key.has_fvar_opacity {
        writeln!(src, "    let input_opacity = load_fvar_scalar(FVAR_SLOT_OPACITY, vertex_index);").unwrap();
    }

    // Displacement: offset position along normal before transform.
    if key.has_displacement && key.has_normals {
        writeln!(src, "    // Vertex displacement along normal").unwrap();
        writeln!(
            src,
            "    let disp_pos = in.position + {} * material.displacement;",
            if key.has_fvar_normals { "input_normal" } else { "in.normal" }
        )
        .unwrap();
        writeln!(
            src,
            "    let world_pos = (model * vec4<f32>(disp_pos, 1.0)).xyz;"
        )
        .unwrap();
    } else {
        writeln!(
            src,
            "    let world_pos = (model * vec4<f32>(in.position, 1.0)).xyz;"
        )
        .unwrap();
    }
    writeln!(
        src,
        "    out.clip_position = scene.view_proj * vec4<f32>(world_pos, 1.0);"
    )
    .unwrap();

    // Output world_pos for any lit shading or flat_shading (dpdx/dpdy face normals)
    let needs_world_pos = key.has_normals
        || key.flat_shading
        || matches!(
            key.shading,
            ShadingModel::BlinnPhong
                | ShadingModel::Pbr
                | ShadingModel::GeomFlat
                | ShadingModel::GeomSmooth
        );
    if key.has_normals {
        // Transform normal by transpose(inverse(model3x3)) — the normal matrix.
        // WGSL has no mat3 inverse, so we use cofactor matrix:
        // normalMatrix = transpose(cofactor(M)) where cofactor columns = cross products of M rows.
        writeln!(src, "    let m0 = model[0].xyz;").unwrap();
        writeln!(src, "    let m1 = model[1].xyz;").unwrap();
        writeln!(src, "    let m2 = model[2].xyz;").unwrap();
        writeln!(
            src,
            "    let normal_mat = mat3x3<f32>(cross(m1, m2), cross(m2, m0), cross(m0, m1));"
        )
        .unwrap();
        writeln!(
            src,
            "    out.world_normal = normalize(normal_mat * {});",
            if key.has_fvar_normals { "input_normal" } else { "in.normal" }
        )
        .unwrap();
    }
    if needs_world_pos {
        writeln!(src, "    out.world_pos = world_pos;").unwrap();
    }
    if key.has_uv {
        writeln!(
            src,
            "    out.uv = {};",
            if key.has_fvar_uv { "input_uv" } else { "in.uv" }
        )
        .unwrap();
    }
    if key.has_color {
        writeln!(
            src,
            "    out.color = {};",
            if key.has_fvar_color { "input_color" } else { "in.color" }
        )
        .unwrap();
    }
    if key.has_fvar_opacity {
        writeln!(src, "    out.opacity = input_opacity;").unwrap();
    }
    if key.pick_buffer_rw && key.use_instancing {
        writeln!(src, "    out.instance_id = instance_id;").unwrap();
    }

    writeln!(src, "    return out;").unwrap();
    writeln!(src, "}}").unwrap();
    writeln!(src).unwrap();
}

/// Emit selection highlight helper applied to final fragment color.
/// Uses scene.selection_color: alpha > 0 means selected, rgb = tint.
fn emit_selection_highlight_fn(src: &mut String) {
    writeln!(
        src,
        "// Selection highlight: mix tint into final color when selection_color.a > 0"
    )
    .unwrap();
    writeln!(src, "fn apply_selection(c: vec4<f32>) -> vec4<f32> {{").unwrap();
    writeln!(src, "    let sel = scene.selection_color;").unwrap();
    writeln!(src, "    if (sel.a > 0.0) {{").unwrap();
    writeln!(
        src,
        "        return vec4<f32>(mix(c.rgb, sel.rgb, sel.a), c.a);"
    )
    .unwrap();
    writeln!(src, "    }}").unwrap();
    writeln!(src, "    return c;").unwrap();
    writeln!(src, "}}").unwrap();
    writeln!(src).unwrap();
}

fn emit_fragment_shader(src: &mut String, key: &MeshShaderKey) {
    // depth_only is handled at the pipeline level via ColorMask::empty(),
    // not in the shader. We still emit a minimal FS for depth_only keys
    // because wgpu requires a fragment stage when color attachments exist.
    if key.depth_only {
        emit_fs_flat(src, key);
        return;
    }
    // Emit selection highlight helper for non-depth passes
    emit_selection_highlight_fn(src);
    match key.shading {
        ShadingModel::FlatColor => emit_fs_flat(src, key),
        ShadingModel::GeomFlat | ShadingModel::GeomSmooth => emit_fs_blinn_phong(src, key),
        ShadingModel::BlinnPhong => emit_fs_blinn_phong(src, key),
        ShadingModel::Pbr => emit_fs_pbr(src, key),
    }
}

/// Flat color fragment shader (no lighting, fallback).
fn emit_fs_flat(src: &mut String, key: &MeshShaderKey) {
    writeln!(src, "@fragment").unwrap();
    writeln!(
        src,
        "fn fs_main(in: VertexOutput, @builtin(front_facing) front_facing: bool) -> @location(0) vec4<f32> {{"
    )
    .unwrap();
    if key.has_color {
        if key.has_fvar_opacity {
            writeln!(src, "    return vec4<f32>(in.color, in.opacity);").unwrap();
        } else {
            writeln!(src, "    return vec4<f32>(in.color, 1.0);").unwrap();
        }
    } else {
        // For FlatColor, scene.ambient_color carries either fallback tint or encoded pick ID.
        if key.pick_buffer_rw {
            let instance_expr = if key.use_instancing {
                "i32(in.instance_id)"
            } else {
                "-1"
            };
            writeln!(
                src,
                "    render_deep_picks(decode_pick_prim_id(scene.ambient_color), {}, -1, -1, -1);",
                instance_expr
            )
            .unwrap();
        }
        if key.has_fvar_opacity {
            writeln!(
                src,
                "    return vec4<f32>(scene.ambient_color.rgb, scene.ambient_color.a * in.opacity);"
            )
            .unwrap();
        } else {
            writeln!(src, "    return scene.ambient_color;").unwrap();
        }
    }
    writeln!(src, "}}").unwrap();
}

/// Emit sRGB <-> linear conversion helpers.
///
/// Diffuse textures are typically stored as sRGB and must be converted to
/// linear before lighting calculations.
fn emit_srgb_helpers(src: &mut String) {
    // sRGB to linear: IEC 61966-2-1 piecewise formula (correct for dark values)
    writeln!(
        src,
        "// sRGB to linear conversion (IEC 61966-2-1 piecewise)"
    )
    .unwrap();
    writeln!(src, "fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {{").unwrap();
    writeln!(src, "    let cutoff = vec3<f32>(0.04045);").unwrap();
    writeln!(src, "    let lo = c / vec3<f32>(12.92);").unwrap();
    writeln!(
        src,
        "    let hi = pow((c + vec3<f32>(0.055)) / vec3<f32>(1.055), vec3<f32>(2.4));"
    )
    .unwrap();
    writeln!(src, "    return select(hi, lo, c <= cutoff);").unwrap();
    writeln!(src, "}}").unwrap();
    writeln!(src).unwrap();
    // Reconstruct TBN from screen-space derivatives for normal mapping
    // Uses dpdx/dpdy builtins which require non-uniform control flow (must be in fragment stage)
    writeln!(
        src,
        "// Perturb world-space normal using tangent-space normal map sample."
    )
    .unwrap();
    writeln!(
        src,
        "// Uses screen-space dpdx/dpdy of world_pos and uv to build TBN matrix."
    )
    .unwrap();
    writeln!(
        src,
        "fn perturb_normal(world_pos: vec3<f32>, geom_normal: vec3<f32>,"
    )
    .unwrap();
    writeln!(
        src,
        "                  uv: vec2<f32>, map_normal: vec3<f32>) -> vec3<f32> {{"
    )
    .unwrap();
    writeln!(
        src,
        "    // Compute tangent from position+UV screen derivatives"
    )
    .unwrap();
    writeln!(src, "    let dp1  = dpdx(world_pos);").unwrap();
    writeln!(src, "    let dp2  = dpdy(world_pos);").unwrap();
    writeln!(src, "    let duv1 = dpdx(uv);").unwrap();
    writeln!(src, "    let duv2 = dpdy(uv);").unwrap();
    writeln!(
        src,
        "    // Gram-Schmidt TBN (handles mirrored UVs correctly)"
    )
    .unwrap();
    writeln!(src, "    let N = normalize(geom_normal);").unwrap();
    writeln!(src, "    let T = normalize(dp1 * duv2.y - dp2 * duv1.y);").unwrap();
    // Bitangent using full cross-terms: handles mirrored UVs (UV handedness)
    writeln!(src, "    let B = normalize(-dp1 * duv2.x + dp2 * duv1.x);").unwrap();
    writeln!(src, "    let tbn = mat3x3<f32>(T, B, N);").unwrap();
    writeln!(src, "    return normalize(tbn * map_normal);").unwrap();
    writeln!(src, "}}").unwrap();
    writeln!(src).unwrap();
}

/// Blinn-Phong fragment shader with multi-light evaluation and texture support.
fn emit_fs_blinn_phong(src: &mut String, key: &MeshShaderKey) {
    writeln!(src, "@fragment").unwrap();
    writeln!(
        src,
        "fn fs_main(in: VertexOutput, @builtin(front_facing) front_facing: bool) -> @location(0) vec4<f32> {{"
    )
    .unwrap();

    // Keep all texture bindings alive for naga reflection (prevents BGL mismatch)
    if key.has_uv {
        writeln!(src, "    let _keep = _touch_textures();").unwrap();
    }

    // Base diffuse color: texture > vertex color > material constant.
    // sRGB linearization needed because we sample Rgba8Unorm (not Srgb format),
    // matching PBR path. C++ Storm handles this via HGI sRGB format conversion.
    emit_diffuse_color_logic(src, key, true);

    // C++ previewSurface.glslfx: opacity + threshold + mode applied BEFORE lighting
    emit_opacity_logic(src, key);
    // Masked mode: discard fragments below opacity threshold (C++ HD_MATERIAL_TAG_MASKED)
    writeln!(src, "    if (material.opacity_threshold > 0.0) {{").unwrap();
    writeln!(
        src,
        "        if (final_opacity < material.opacity_threshold) {{"
    )
    .unwrap();
    writeln!(src, "            discard;").unwrap();
    writeln!(src, "        }}").unwrap();
    writeln!(src, "        final_opacity = 1.0;").unwrap(); // C++ line 72
    writeln!(src, "    }} else if (material.opacity_mode > 0u) {{").unwrap();
    // Transparent mode (opacityMode==1): pre-multiply diffuseColor by opacity (C++ line 108-109)
    writeln!(src, "        base_color = base_color * final_opacity;").unwrap();
    writeln!(src, "    }}").unwrap();
    writeln!(src).unwrap();

    // Lit path: vertex normals OR flat_shading (face normals via dpdx/dpdy)
    let has_lighting = key.has_normals || key.flat_shading;
    if has_lighting {
        emit_normal_logic(src, key);

        writeln!(
            src,
            "    let V = normalize(scene.camera_pos.xyz - in.world_pos);"
        )
        .unwrap();
        writeln!(src, "    let roughness = material.roughness;").unwrap();
        writeln!(src, "    let metallic = material.metallic;").unwrap();
        // F0/F90 matching C++ evaluateLight for Blinn-Phong path
        writeln!(
            src,
            "    let R = (1.0 - material.ior) / (1.0 + material.ior);"
        )
        .unwrap();
        writeln!(src, "    var f0: vec3<f32>;").unwrap();
        writeln!(src, "    var f90: vec3<f32>;").unwrap();
        writeln!(src, "    if (material.use_specular_workflow > 0.5) {{").unwrap();
        writeln!(src, "        f0 = material.specular_color;").unwrap();
        writeln!(src, "        f90 = vec3<f32>(1.0);").unwrap();
        writeln!(src, "    }} else {{").unwrap();
        writeln!(
            src,
            "        let spec_color = mix(vec3<f32>(1.0), base_color, metallic);"
        )
        .unwrap();
        writeln!(
            src,
            "        f0 = mix(R * R * spec_color, spec_color, metallic);"
        )
        .unwrap();
        writeln!(src, "        f90 = spec_color;").unwrap();
        writeln!(src, "    }}").unwrap();
        writeln!(src).unwrap();

        // Multi-light evaluation loop
        // C++ simpleLighting.glslfx: USE_SHADOWS gate on shadowing() call
        if key.use_shadows {
            lighting::emit_light_loop_with_shadows(src);
        } else {
            lighting::emit_light_loop(src);
        }

        writeln!(src).unwrap();
        writeln!(
            src,
            "    let ambient = base_color * scene.ambient_color.xyz;"
        )
        .unwrap();
        emit_emissive_logic(src, key);
        // Occlusion applies only to diffuse+specular, NOT emissive (PBR spec).
        // Accumulate non-emissive light first, apply occlusion, then add emissive.
        writeln!(src, "    var color = ambient + lo;").unwrap();

        // Occlusion modulation (excludes emissive)
        emit_occlusion_logic(src, key);

        // Add emissive after occlusion so it is unaffected
        writeln!(src, "    color = color + emissive;").unwrap();

        // C++ line 136-137: presence mode post-multiply
        writeln!(
            src,
            "    if (material.opacity_threshold == 0.0 && material.opacity_mode == 0u) {{"
        )
        .unwrap();
        writeln!(src, "        color = color * final_opacity;").unwrap();
        writeln!(src, "    }}").unwrap();

        writeln!(
            src,
            "    return apply_selection(vec4<f32>(color, final_opacity));"
        )
        .unwrap();
    } else {
        emit_emissive_logic(src, key);

        // Presence mode post-multiply for unlit path
        writeln!(src, "    var unlit_color = base_color + emissive;").unwrap();
        writeln!(
            src,
            "    if (material.opacity_threshold == 0.0 && material.opacity_mode == 0u) {{"
        )
        .unwrap();
        writeln!(src, "        unlit_color = unlit_color * final_opacity;").unwrap();
        writeln!(src, "    }}").unwrap();
        writeln!(
            src,
            "    return apply_selection(vec4<f32>(unlit_color, final_opacity));"
        )
        .unwrap();
    }

    writeln!(src, "}}").unwrap();
}

/// PBR metallic-roughness fragment shader with full texture pipeline.
fn emit_fs_pbr(src: &mut String, key: &MeshShaderKey) {
    writeln!(src, "@fragment").unwrap();
    writeln!(
        src,
        "fn fs_main(in: VertexOutput, @builtin(front_facing) front_facing: bool) -> @location(0) vec4<f32> {{"
    )
    .unwrap();

    // Keep all texture bindings alive for naga reflection (prevents BGL mismatch)
    if key.has_uv {
        writeln!(src, "    let _keep = _touch_textures();").unwrap();
    }

    // Base diffuse color with sRGB conversion
    emit_diffuse_color_logic(src, key, true);

    // C++ previewSurface.glslfx: opacity + threshold + mode applied BEFORE lighting
    emit_opacity_logic(src, key);
    // Masked mode: discard fragments below opacity threshold (C++ HD_MATERIAL_TAG_MASKED)
    writeln!(src, "    if (material.opacity_threshold > 0.0) {{").unwrap();
    writeln!(
        src,
        "        if (final_opacity < material.opacity_threshold) {{"
    )
    .unwrap();
    writeln!(src, "            discard;").unwrap();
    writeln!(src, "        }}").unwrap();
    writeln!(src, "        final_opacity = 1.0;").unwrap(); // C++ line 72
    writeln!(src, "    }} else if (material.opacity_mode > 0u) {{").unwrap();
    // Transparent mode (opacityMode==1): pre-multiply diffuseColor by opacity (C++ line 108-109)
    writeln!(src, "        base_color = base_color * final_opacity;").unwrap();
    writeln!(src, "    }}").unwrap();
    writeln!(src).unwrap();

    // Lit path: vertex normals OR flat_shading (face normals via dpdx/dpdy)
    let has_lighting = key.has_normals || key.flat_shading;
    if has_lighting {
        emit_normal_logic(src, key);

        // Roughness from texture or material
        emit_scalar_tex_or_fallback(
            src,
            key,
            "roughness",
            "roughness_tex",
            "roughness_sampler",
            "has_roughness_tex",
            "material.roughness",
            ".r",
        );

        // Metallic from texture or material
        emit_scalar_tex_or_fallback(
            src,
            key,
            "metallic",
            "metallic_tex",
            "metallic_sampler",
            "has_metallic_tex",
            "material.metallic",
            ".r",
        );

        writeln!(
            src,
            "    let V = normalize(scene.camera_pos.xyz - in.world_pos);"
        )
        .unwrap();
        writeln!(src, "    let NdotV = max(dot(N, V), 0.0);").unwrap();
        // F0/F90 per C++ previewSurface.glslfx evaluateLight():
        //   R = (1 - ior) / (1 + ior)
        //   specular workflow: F0 = specularColor, F90 = vec3(1)
        //   metallic workflow: specColor = mix(1, diffuseColor, metallic)
        //                     F0 = mix(R*R*specColor, specColor, metallic)
        //                     F90 = specColor, diffuse *= (1 - metallic)
        writeln!(
            src,
            "    let R = (1.0 - material.ior) / (1.0 + material.ior);"
        )
        .unwrap();
        writeln!(src, "    var f0: vec3<f32>;").unwrap();
        writeln!(src, "    var f90: vec3<f32>;").unwrap();
        writeln!(src, "    if (material.use_specular_workflow > 0.5) {{").unwrap();
        writeln!(src, "        f0 = material.specular_color;").unwrap();
        writeln!(src, "        f90 = vec3<f32>(1.0);").unwrap();
        writeln!(src, "    }} else {{").unwrap();
        writeln!(
            src,
            "        let spec_color = mix(vec3<f32>(1.0), base_color, metallic);"
        )
        .unwrap();
        writeln!(
            src,
            "        f0 = mix(R * R * spec_color, spec_color, metallic);"
        )
        .unwrap();
        writeln!(src, "        f90 = spec_color;").unwrap();
        writeln!(src, "    }}").unwrap();
        writeln!(src).unwrap();

        // Iridescence: replace Fresnel f0 with thin-film interference color
        writeln!(
            src,
            "    // Iridescence: replace Fresnel when iridescence > 0"
        )
        .unwrap();
        writeln!(src, "    if (material.iridescence > 0.0) {{").unwrap();
        writeln!(src, "        let f0_irid = fresnel_iridescence(NdotV, f0, material.iridescence_ior, material.iridescence_thickness);").unwrap();
        writeln!(src, "        f0 = mix(f0, f0_irid, material.iridescence);").unwrap();
        writeln!(src, "    }}").unwrap();
        writeln!(src).unwrap();

        // Multi-light evaluation loop
        // C++ simpleLighting.glslfx: USE_SHADOWS gate on shadowing() call
        if key.use_shadows {
            lighting::emit_light_loop_with_shadows(src);
        } else {
            lighting::emit_light_loop(src);
        }

        writeln!(src).unwrap();

        // --- Advanced PBR post-lighting adjustments ---

        // SSS: wrap lighting approximation
        writeln!(
            src,
            "    // Subsurface scattering wrap-lighting approximation"
        )
        .unwrap();
        writeln!(src, "    if (material.subsurface > 0.0) {{").unwrap();
        writeln!(src, "        let sss_tint = material.subsurface_color;").unwrap();
        writeln!(src, "        let sss_contrib = base_color * sss_tint * scene.ambient_color.xyz * material.subsurface * 0.25;").unwrap();
        writeln!(src, "        lo = lo + sss_contrib;").unwrap();
        writeln!(src, "    }}").unwrap();
        writeln!(src).unwrap();

        // Sheen: additive Charlie lobe
        writeln!(src, "    // Sheen lobe (Charlie NDF + Neubelt visibility)").unwrap();
        writeln!(src, "    let sheen_col = material.sheen_color;").unwrap();
        writeln!(
            src,
            "    if (sheen_col.x + sheen_col.y + sheen_col.z > 0.0) {{"
        )
        .unwrap();
        writeln!(src, "        let sheen_alpha = max(material.sheen_roughness * material.sheen_roughness, 0.001);").unwrap();
        writeln!(src, "        let H_s = normalize(N + V);").unwrap();
        writeln!(src, "        let NdotH_s = max(dot(N, H_s), 0.0);").unwrap();
        writeln!(
            src,
            "        let NdotL_s = max(dot(N, normalize(reflect(-V, N))), 0.0);"
        )
        .unwrap();
        writeln!(
            src,
            "        let D_sheen = distribution_charlie(NdotH_s, sheen_alpha);"
        )
        .unwrap();
        writeln!(
            src,
            "        let V_sheen = visibility_neubelt(NdotV, max(NdotL_s, 0.001));"
        )
        .unwrap();
        writeln!(
            src,
            "        lo = lo + sheen_col * D_sheen * V_sheen * NdotV;"
        )
        .unwrap();
        writeln!(src, "    }}").unwrap();
        writeln!(src).unwrap();

        // IBL contribution (diffuse irradiance + specular prefilter + BRDF LUT)
        if key.has_ibl {
            emit_ibl_contribution(src);
        } else {
            // Fallback ambient when no IBL
            writeln!(
                src,
                "    let ambient = scene.ambient_color.xyz * base_color * 0.3;"
            )
            .unwrap();
        }

        emit_emissive_logic(src, key);

        // Occlusion applies only to diffuse+specular, NOT emissive (per PBR spec and
        // C++ Storm preview.glslfx). Build color without emissive first.
        if key.has_ibl {
            writeln!(src, "    var color = lo + ibl_diffuse + ibl_specular;").unwrap();
        } else {
            writeln!(src, "    var color = ambient + lo;").unwrap();
        }

        // Clearcoat: second specular lobe, added before occlusion so it gets
        // occluded together with base specular (C++ evaluateLight lines 288-289).
        emit_clearcoat_logic(src);

        // Occlusion modulation (excludes emissive)
        emit_occlusion_logic(src, key);

        // Add emissive after occlusion so it is unaffected by ambient occlusion
        writeln!(src, "    color = color + emissive;").unwrap();

        // Transmission: reduce alpha to simulate transparent transmission
        writeln!(
            src,
            "    // Transmission: reduce opacity by transmission weight"
        )
        .unwrap();
        writeln!(
            src,
            "    var transmission_factor = 1.0 - material.transmission;"
        )
        .unwrap();
        writeln!(src, "    if (material.transmission > 0.0) {{").unwrap();
        writeln!(src, "        let abs_coeff = (vec3<f32>(1.0) - material.transmission_color) / max(material.transmission_depth, 0.001);").unwrap();
        writeln!(
            src,
            "        let absorbed = exp(-abs_coeff * material.transmission_depth);"
        )
        .unwrap();
        writeln!(
            src,
            "        color = color * mix(vec3<f32>(1.0), absorbed, material.transmission);"
        )
        .unwrap();
        writeln!(src, "    }}").unwrap();
        writeln!(src).unwrap();

        // C++ line 136-137: presence mode (opacityMode==0) post-multiplies all color
        writeln!(
            src,
            "    if (material.opacity_threshold == 0.0 && material.opacity_mode == 0u) {{"
        )
        .unwrap();
        writeln!(src, "        color = color * final_opacity;").unwrap();
        writeln!(src, "    }}").unwrap();

        // Output linear HDR: tonemapping is handled by the display pipeline
        writeln!(
            src,
            "    let final_opacity_adj = final_opacity * transmission_factor;"
        )
        .unwrap();
        writeln!(
            src,
            "    return apply_selection(vec4<f32>(color, final_opacity_adj));"
        )
        .unwrap();
    } else {
        emit_emissive_logic(src, key);

        // Presence mode post-multiply for unlit path
        writeln!(src, "    var unlit_color = base_color + emissive;").unwrap();
        writeln!(
            src,
            "    if (material.opacity_threshold == 0.0 && material.opacity_mode == 0u) {{"
        )
        .unwrap();
        writeln!(src, "        unlit_color = unlit_color * final_opacity;").unwrap();
        writeln!(src, "    }}").unwrap();
        writeln!(
            src,
            "    return apply_selection(vec4<f32>(unlit_color, final_opacity));"
        )
        .unwrap();
    }

    writeln!(src, "}}").unwrap();
}

/// Emit IBL diffuse + specular contribution using split-sum approximation.
///
/// Requires: N, V, NdotV, base_color, roughness, metallic, f0 in scope.
/// Produces: ibl_diffuse, ibl_specular local vars.
///
/// Per C++ Storm simpleLighting.glslfx, the dome light world-to-light transform
/// is applied to the reflection and normal vectors before sampling the IBL maps.
/// This correctly orients the environment map to match the dome light's rotation.
fn emit_ibl_contribution(src: &mut String) {
    // Find the dome light world-to-light transform. We use the first dome light
    // in the light array (light_type == 3.0). Matches C++ GetLightSource() which
    // reads light.worldToLightTransform for dome lights.
    writeln!(
        src,
        "    // IBL: find dome light orientation transform from light array"
    )
    .unwrap();
    writeln!(src, "    var dome_xform = mat4x4<f32>(").unwrap();
    writeln!(src, "        vec4<f32>(1.0, 0.0, 0.0, 0.0),").unwrap();
    writeln!(src, "        vec4<f32>(0.0, 1.0, 0.0, 0.0),").unwrap();
    writeln!(src, "        vec4<f32>(0.0, 0.0, 1.0, 0.0),").unwrap();
    writeln!(src, "        vec4<f32>(0.0, 0.0, 0.0, 1.0)").unwrap();
    writeln!(src, "    );").unwrap();
    writeln!(src, "    let num_lights = i32(light_data.light_count.x);").unwrap();
    writeln!(src, "    for (var li = 0; li < num_lights; li++) {{").unwrap();
    writeln!(
        src,
        "        if (light_data.lights[li].type_pad.x == 3.0) {{"
    )
    .unwrap();
    writeln!(
        src,
        "            dome_xform = light_data.lights[li].world_to_light;"
    )
    .unwrap();
    writeln!(src, "            break;").unwrap();
    writeln!(src, "        }}").unwrap();
    writeln!(src, "    }}").unwrap();
    writeln!(src).unwrap();

    // C++ evaluateIndirectLighting: for dome lights EdotH=1.0, so SchlickFresnel(1)=0,
    // F = mix(F0, F90, 0) = F0. Use F0 directly for exact C++ parity.
    writeln!(src, "    // IBL: diffuse irradiance (Lambertian)").unwrap();
    writeln!(src, "    let ibl_F = f0;").unwrap();
    writeln!(src, "    var ibl_d = base_color;").unwrap();
    writeln!(src, "    if (material.use_specular_workflow < 0.5) {{").unwrap();
    writeln!(src, "        ibl_d = ibl_d * (1.0 - metallic);").unwrap();
    writeln!(src, "    }}").unwrap();
    writeln!(src, "    ibl_d = ibl_d * (vec3<f32>(1.0) - ibl_F);").unwrap();
    // Apply dome orientation to the normal before irradiance sampling.
    // Transforms world-space N into the dome light's local space.
    writeln!(
        src,
        "    let N_ibl = normalize((dome_xform * vec4<f32>(N, 0.0)).xyz);"
    )
    .unwrap();
    writeln!(src, "    let irradiance = sample_irradiance(N_ibl);").unwrap();
    writeln!(src, "    let ibl_diffuse = ibl_d * irradiance;").unwrap();
    writeln!(src).unwrap();
    // C++ evaluateIndirectLighting: specular = prefilter * (F * brdf.x + brdf.y)
    writeln!(src, "    // IBL: specular prefilter + BRDF LUT (split-sum)").unwrap();
    writeln!(src, "    let R_refl = reflect(-V, N);").unwrap();
    // Apply dome orientation to the reflection vector before prefilter sampling.
    // Matches C++ Storm worldToLightTransform * reflection_dir usage.
    writeln!(
        src,
        "    let R_ibl = normalize((dome_xform * vec4<f32>(R_refl, 0.0)).xyz);"
    )
    .unwrap();
    writeln!(
        src,
        "    let prefiltered = sample_prefilter(R_ibl, roughness);"
    )
    .unwrap();
    writeln!(src, "    let brdf_uv = vec2<f32>(NdotV, roughness);").unwrap();
    writeln!(
        src,
        "    let brdf = textureSampleLevel(ibl_brdf_lut, ibl_brdf_lut_sampler, brdf_uv, 0.0).rg;"
    )
    .unwrap();
    // C++ line 359: specular = prefilter * (F * brdf.x + brdf.y)
    writeln!(
        src,
        "    let ibl_spec_base = prefiltered * (ibl_F * brdf.x + vec3<f32>(brdf.y));"
    )
    .unwrap();
    writeln!(src).unwrap();

    // C++ evaluateIndirectLighting lines 361-372: clearcoat IBL component
    writeln!(src, "    // IBL clearcoat: second specular lobe").unwrap();
    writeln!(src, "    var ibl_clearcoat = vec3<f32>(0.0);").unwrap();
    writeln!(src, "    if (material.clearcoat > 0.0) {{").unwrap();
    // C++ line 330: R = (1-ior)/(1+ior) — already computed as `R` in caller scope
    // C++ line 364-367: clearcoatF = clearcoatAmount * mix(R*R*ccColor, ccColor, fresnel)
    // fresnel = SchlickFresnel(EdotH) = 0 for dome light, so clearcoatF = clearcoatAmount * R*R*ccColor
    writeln!(src, "        let cc_color_ibl = vec3<f32>(1.0);").unwrap();
    writeln!(src, "        let cc_F0_ibl = R * R * cc_color_ibl;").unwrap();
    writeln!(
        src,
        "        let cc_F_ibl = material.clearcoat * cc_F0_ibl;"
    )
    .unwrap();
    // C++ line 368-370: lod = ccRoughness * MAX_LOD, resample prefilter + brdf
    writeln!(
        src,
        "        let cc_prefiltered = sample_prefilter(R_ibl, material.clearcoat_roughness);"
    )
    .unwrap();
    writeln!(
        src,
        "        let cc_brdf_uv = vec2<f32>(NdotV, material.clearcoat_roughness);"
    )
    .unwrap();
    writeln!(src, "        let cc_brdf = textureSampleLevel(ibl_brdf_lut, ibl_brdf_lut_sampler, cc_brdf_uv, 0.0).rg;").unwrap();
    // C++ line 371: clearcoat = prefilter * (clearcoatF * brdf.x + brdf.y)
    writeln!(
        src,
        "        ibl_clearcoat = cc_prefiltered * (cc_F_ibl * cc_brdf.x + vec3<f32>(cc_brdf.y));"
    )
    .unwrap();
    writeln!(src, "    }}").unwrap();
    // C++ line 376: indirect.specular = (specular + clearcoat) * occlusion
    writeln!(src, "    let ibl_specular = ibl_spec_base + ibl_clearcoat;").unwrap();
    writeln!(src).unwrap();
}

/// Emit base_color variable with texture/vertex/material fallback chain.
///
/// When `do_srgb` is true, applies sRGB->linear conversion to sampled texture.
fn emit_diffuse_color_logic(src: &mut String, key: &MeshShaderKey, do_srgb: bool) {
    writeln!(src, "    var base_color = material.diffuse_color;").unwrap();
    if key.has_color {
        writeln!(src, "    if (material.use_vertex_color > 0.5) {{").unwrap();
        writeln!(src, "        base_color = in.color;").unwrap();
        writeln!(src, "    }}").unwrap();
    }
    if key.has_uv {
        writeln!(src, "    if (material.has_diffuse_tex != 0u) {{").unwrap();
        writeln!(
            src,
            "        let tex_rgba = textureSample(diffuse_tex, diffuse_sampler, in.uv);"
        )
        .unwrap();
        if do_srgb {
            writeln!(src, "        base_color = srgb_to_linear(tex_rgba.rgb);").unwrap();
        } else {
            writeln!(src, "        base_color = tex_rgba.rgb;").unwrap();
        }
        writeln!(src, "    }}").unwrap();
    }
}

/// Emit N (world normal) with optional normal map perturbation.
///
/// When `flat_shading` is true, computes face normal from screen-space
/// derivatives of world_pos (dpdx/dpdy) instead of interpolated vertex normal.
/// This gives each triangle a single flat normal, equivalent to GL_FLAT.
fn emit_normal_logic(src: &mut String, key: &MeshShaderKey) {
    if key.flat_shading {
        // Face normal from screen-space derivatives of world position
        writeln!(src, "    let dp1 = dpdx(in.world_pos);").unwrap();
        writeln!(src, "    let dp2 = dpdy(in.world_pos);").unwrap();
        writeln!(src, "    var N = normalize(cross(dp1, dp2));").unwrap();
    } else {
        writeln!(src, "    var N = normalize(in.world_normal);").unwrap();
    }
    // Flip normal for back-facing fragments (double-sided lighting)
    writeln!(src, "    if (!front_facing) {{ N = -N; }}").unwrap();
    // Normal map perturbation only applies to smooth shading (vertex normals)
    if !key.flat_shading && key.has_uv {
        writeln!(src, "    if (material.has_normal_tex != 0u) {{").unwrap();
        // Sample normal map and decode [0,1] -> [-1,1]
        writeln!(
            src,
            "        let nmap = textureSample(normal_tex, normal_sampler, in.uv).rgb;"
        )
        .unwrap();
        writeln!(src, "        let tspace_n = nmap * 2.0 - vec3<f32>(1.0);").unwrap();
        writeln!(
            src,
            "        N = perturb_normal(in.world_pos, in.world_normal, in.uv, tspace_n);"
        )
        .unwrap();
        writeln!(src, "    }}").unwrap();
    }
}

/// Emit scalar material property from texture (single channel) or material fallback.
fn emit_scalar_tex_or_fallback(
    src: &mut String,
    key: &MeshShaderKey,
    var_name: &str,
    tex: &str,
    smp: &str,
    flag: &str,
    fallback: &str,
    channel: &str,
) {
    writeln!(src, "    var {} = {};", var_name, fallback).unwrap();
    if key.has_uv {
        writeln!(src, "    if (material.{} != 0u) {{", flag).unwrap();
        writeln!(
            src,
            "        {} = textureSample({}, {}, in.uv){};",
            var_name, tex, smp, channel
        )
        .unwrap();
        writeln!(src, "    }}").unwrap();
    }
}

/// Emit emissive color variable.
fn emit_emissive_logic(src: &mut String, key: &MeshShaderKey) {
    writeln!(src, "    var emissive = material.emissive_color;").unwrap();
    if key.has_uv {
        writeln!(src, "    if (material.has_emissive_tex != 0u) {{").unwrap();
        writeln!(
            src,
            "        emissive = textureSample(emissive_tex, emissive_sampler, in.uv).rgb;"
        )
        .unwrap();
        writeln!(src, "    }}").unwrap();
    }
}

/// Emit occlusion and apply to accumulated color.
fn emit_occlusion_logic(src: &mut String, key: &MeshShaderKey) {
    if key.has_uv {
        writeln!(src, "    if (material.has_occlusion_tex != 0u) {{").unwrap();
        writeln!(
            src,
            "        let occlusion = textureSample(occlusion_tex, occlusion_sampler, in.uv).r;"
        )
        .unwrap();
        writeln!(src, "        color = color * occlusion;").unwrap();
        writeln!(src, "    }}").unwrap();
    }
}

/// Emit final_opacity from opacity texture or material.
fn emit_opacity_logic(src: &mut String, key: &MeshShaderKey) {
    writeln!(src, "    var final_opacity = material.opacity;").unwrap();
    if key.has_fvar_opacity {
        writeln!(src, "    final_opacity = final_opacity * in.opacity;").unwrap();
    }
    if key.has_uv {
        writeln!(src, "    if (material.has_opacity_tex != 0u) {{").unwrap();
        writeln!(
            src,
            "        final_opacity = textureSample(opacity_tex, opacity_sampler, in.uv).r;"
        )
        .unwrap();
        writeln!(src, "    }}").unwrap();
    }
}

/// Emit clearcoat specular layer (second GGX lobe) per C++ previewSurface.glslfx.
///
/// C++ evaluateLight lines 272-281:
///   s2 = clearcoatAmount * evaluateDirectSpecular(
///        R*R*clearcoatColor, clearcoatColor, clearcoatRoughness, fresnel, ...)
///   specular = occlusion * NdotL * (s1 + s2) * lightSpecularIrradiance
///
/// No energy conservation attenuation — C++ simply adds s1 + s2.
/// Requires: N, V, NdotV, R, color (accumulated base shading) in scope.
fn emit_clearcoat_logic(src: &mut String) {
    writeln!(
        src,
        "    // Clearcoat layer (second GGX lobe) per C++ previewSurface.glslfx"
    )
    .unwrap();
    writeln!(src, "    if (material.clearcoat > 0.0) {{").unwrap();
    // C++ line 238: clearcoatRoughness = max(0.001, clearcoatRoughness)
    writeln!(
        src,
        "        let cc_roughness = max(0.001, material.clearcoat_roughness);"
    )
    .unwrap();
    writeln!(src, "        let cc_alpha = cc_roughness * cc_roughness;").unwrap();
    // C++ line 276-277: F0 = R*R*clearcoatColor, F90 = clearcoatColor
    // clearcoatColor defaults to vec3(1.0) in UsdPreviewSurface spec
    writeln!(src, "        let cc_color = vec3<f32>(1.0);").unwrap();
    writeln!(src, "        let cc_f0 = R * R * cc_color;").unwrap();
    writeln!(src, "        let cc_f90 = cc_color;").unwrap();
    // Iterate lights for clearcoat specular contribution
    writeln!(src, "        var cc_spec = vec3<f32>(0.0);").unwrap();
    writeln!(
        src,
        "        let cc_n_lights = i32(light_data.light_count.x);"
    )
    .unwrap();
    writeln!(src, "        for (var ci = 0; ci < cc_n_lights; ci++) {{").unwrap();
    writeln!(src, "            let cc_light = light_data.lights[ci];").unwrap();
    writeln!(src, "            let cc_ltype = i32(cc_light.type_pad.x);").unwrap();
    writeln!(
        src,
        "            if (cc_ltype == 3) {{ continue; }} // skip dome"
    )
    .unwrap();
    writeln!(src, "            var cc_L: vec3<f32>;").unwrap();
    writeln!(
        src,
        "            if (cc_ltype == 0) {{ cc_L = normalize(-cc_light.direction.xyz); }}"
    )
    .unwrap();
    writeln!(
        src,
        "            else {{ cc_L = normalize(cc_light.position.xyz - in.world_pos); }}"
    )
    .unwrap();
    writeln!(src, "            let cc_H = normalize(V + cc_L);").unwrap();
    writeln!(src, "            let cc_NdotL = max(dot(N, cc_L), 0.0);").unwrap();
    writeln!(src, "            let cc_NdotH = max(dot(N, cc_H), 0.0);").unwrap();
    writeln!(src, "            let cc_VdotH = max(dot(V, cc_H), 0.0);").unwrap();
    writeln!(
        src,
        "            let cc_D = ggx_distribution(cc_NdotH, cc_alpha);"
    )
    .unwrap();
    writeln!(
        src,
        "            let cc_G = smith_ggx(NdotV, cc_NdotL, cc_roughness);"
    )
    .unwrap();
    // C++ evaluateDirectSpecular: F = mix(F0, F90, fresnel) where fresnel = SchlickFresnel(EdotH)
    writeln!(
        src,
        "            let cc_fw = pow(max(1.0 - cc_VdotH, 0.0), 5.0);"
    )
    .unwrap();
    writeln!(src, "            let cc_F = mix(cc_f0, cc_f90, cc_fw);").unwrap();
    // C++ evaluateDirectSpecular: denominator = 4 * NdotL * NdotE + EPSILON (EPSILON=0.001)
    writeln!(
        src,
        "            let cc_denom = 4.0 * NdotV * cc_NdotL + 0.001;"
    )
    .unwrap();
    writeln!(
        src,
        "            let cc_lum = cc_light.color.xyz * cc_light.params.x;"
    )
    .unwrap();
    writeln!(
        src,
        "            cc_spec = cc_spec + cc_lum * (cc_D * cc_G * cc_F / cc_denom) * cc_NdotL;"
    )
    .unwrap();
    writeln!(src, "        }}").unwrap();
    // C++ line 288-289: specular = occlusion * NdotL * (s1 + s2) * lightSpecularIrradiance
    // Simply add clearcoat contribution — no energy conservation in C++ reference.
    writeln!(src, "        color = color + cc_spec * material.clearcoat;").unwrap();
    writeln!(src, "    }}").unwrap();
}

/// Emit IBL helper functions for cubemap direction sampling.
///
/// WGSL has no texture_cube, so we use texture_2d_array with 6 layers.
/// The helper maps a direction vector to (face_index, uv) for sampling.
fn emit_ibl_helpers(src: &mut String) {
    // Map a world-space direction to a 2D-array cube face index + UV.
    // Face order: +X=0, -X=1, +Y=2, -Y=3, +Z=4, -Z=5 (matches D3D/Vulkan convention).
    writeln!(
        src,
        "// Map direction to cubemap face index + UV (no texture_cube in WGSL)."
    )
    .unwrap();
    writeln!(src, "fn dir_to_cube_face(d: vec3<f32>) -> vec3<f32> {{").unwrap();
    writeln!(
        src,
        "    let ax = abs(d.x); let ay = abs(d.y); let az = abs(d.z);"
    )
    .unwrap();
    writeln!(
        src,
        "    var face: u32; var uc: f32; var vc: f32; var ma: f32;"
    )
    .unwrap();
    writeln!(src, "    if ax >= ay && ax >= az {{").unwrap();
    writeln!(src, "        ma = ax;").unwrap();
    writeln!(
        src,
        "        if d.x > 0.0 {{ face = 0u; uc = -d.z; vc = -d.y; }}"
    )
    .unwrap();
    writeln!(
        src,
        "        else         {{ face = 1u; uc =  d.z; vc = -d.y; }}"
    )
    .unwrap();
    writeln!(src, "    }} else if ay >= ax && ay >= az {{").unwrap();
    writeln!(src, "        ma = ay;").unwrap();
    writeln!(
        src,
        "        if d.y > 0.0 {{ face = 2u; uc =  d.x; vc =  d.z; }}"
    )
    .unwrap();
    writeln!(
        src,
        "        else         {{ face = 3u; uc =  d.x; vc = -d.z; }}"
    )
    .unwrap();
    writeln!(src, "    }} else {{").unwrap();
    writeln!(src, "        ma = az;").unwrap();
    writeln!(
        src,
        "        if d.z > 0.0 {{ face = 4u; uc =  d.x; vc = -d.y; }}"
    )
    .unwrap();
    writeln!(
        src,
        "        else         {{ face = 5u; uc = -d.x; vc = -d.y; }}"
    )
    .unwrap();
    writeln!(src, "    }}").unwrap();
    writeln!(src, "    let uv = (vec2<f32>(uc, vc) / ma) * 0.5 + 0.5;").unwrap();
    writeln!(src, "    return vec3<f32>(uv, f32(face));").unwrap();
    writeln!(src, "}}").unwrap();
    writeln!(src).unwrap();

    // Sample irradiance cubemap for diffuse IBL.
    writeln!(
        src,
        "fn sample_irradiance(normal: vec3<f32>) -> vec3<f32> {{"
    )
    .unwrap();
    writeln!(src, "    let cube = dir_to_cube_face(normal);").unwrap();
    writeln!(src, "    let uv = cube.xy;").unwrap();
    writeln!(src, "    let layer = i32(cube.z);").unwrap();
    writeln!(
        src,
        "    return textureSampleLevel(ibl_irradiance, ibl_irradiance_sampler, uv, layer, 0.0).rgb;"
    )
    .unwrap();
    writeln!(src, "}}").unwrap();
    writeln!(src).unwrap();

    // Sample prefiltered specular cubemap at roughness-derived mip level.
    // mip_count is queried from the texture at runtime via textureNumLevels(),
    // matching C++ Storm which derives it from envMapDim (log2 of texture size).
    writeln!(
        src,
        "fn sample_prefilter(R: vec3<f32>, roughness: f32) -> vec3<f32> {{"
    )
    .unwrap();
    writeln!(src, "    let cube = dir_to_cube_face(R);").unwrap();
    writeln!(src, "    let uv = cube.xy;").unwrap();
    writeln!(src, "    let layer = i32(cube.z);").unwrap();
    // Derive mip count from actual texture dimensions -- not hardcoded.
    // textureNumLevels() returns the mip level count of the texture array.
    writeln!(
        src,
        "    let mip_count = f32(textureNumLevels(ibl_prefilter));"
    )
    .unwrap();
    writeln!(src, "    let mip = roughness * (mip_count - 1.0);").unwrap();
    writeln!(
        src,
        "    return textureSampleLevel(ibl_prefilter, ibl_prefilter_sampler, uv, layer, mip).rgb;"
    )
    .unwrap();
    writeln!(src, "}}").unwrap();
    writeln!(src).unwrap();
}

/// Emit PBR helper functions (GGX distribution, Smith geometry, Fresnel).
fn emit_pbr_helpers(src: &mut String) {
    // GGX/Trowbridge-Reitz normal distribution
    writeln!(
        src,
        "fn ggx_distribution(n_dot_h: f32, alpha: f32) -> f32 {{"
    )
    .unwrap();
    // C++ previewSurface.glslfx NormalDistribution: (alpha2 + EPSILON) / (PI * denom^2)
    // EPSILON = 0.001 (C++ line 155), NO epsilon in denominator
    writeln!(src, "    let a2 = alpha * alpha;").unwrap();
    writeln!(src, "    let denom = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;").unwrap();
    writeln!(
        src,
        "    return (a2 + 0.001) / (3.14159265 * denom * denom);"
    )
    .unwrap();
    writeln!(src, "}}").unwrap();
    writeln!(src).unwrap();

    // Smith GGX geometry function. C++ previewSurface.glslfx uses k = alpha * 0.5
    // where alpha = roughness^2. This matches C++ Storm for correct specular intensity.
    writeln!(
        src,
        "// Smith-GGX geometry term. k = roughness^2 / 2 per C++ previewSurface.glslfx."
    )
    .unwrap();
    writeln!(
        src,
        "fn smith_ggx(n_dot_v: f32, n_dot_l: f32, roughness: f32) -> f32 {{"
    )
    .unwrap();
    // k = alpha * 0.5 = roughness^2 * 0.5 (matches C++ Storm)
    writeln!(src, "    let k = roughness * roughness * 0.5;").unwrap();
    writeln!(src, "    let g_v = n_dot_v / (n_dot_v * (1.0 - k) + k);").unwrap();
    writeln!(src, "    let g_l = n_dot_l / (n_dot_l * (1.0 - k) + k);").unwrap();
    writeln!(src, "    return g_v * g_l;").unwrap();
    writeln!(src, "}}").unwrap();
    writeln!(src).unwrap();

    // Fresnel-Schlick approximation
    writeln!(
        src,
        "fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {{"
    )
    .unwrap();
    writeln!(
        src,
        "    return f0 + (vec3<f32>(1.0) - f0) * pow(1.0 - cos_theta, 5.0);"
    )
    .unwrap();
    writeln!(src, "}}").unwrap();
    writeln!(src).unwrap();

    // Fresnel-Schlick scalar (for clearcoat, single-channel F0)
    writeln!(
        src,
        "fn fresnel_schlick_scalar(cos_theta: f32, f0: f32) -> f32 {{"
    )
    .unwrap();
    writeln!(
        src,
        "    return f0 + (1.0 - f0) * pow(1.0 - cos_theta, 5.0);"
    )
    .unwrap();
    writeln!(src, "}}").unwrap();
    writeln!(src).unwrap();
    // --- Advanced PBR helpers ---

    // Anisotropic GGX NDF (Burley 2012)
    writeln!(src, "// Anisotropic GGX NDF (Burley 2012)").unwrap();
    writeln!(src, "fn ggx_anisotropic(n_dot_h: f32, h: vec3<f32>, t: vec3<f32>, b: vec3<f32>, ax: f32, ay: f32) -> f32 {{").unwrap();
    writeln!(src, "    let t_dot_h = dot(t, h);").unwrap();
    writeln!(src, "    let b_dot_h = dot(b, h);").unwrap();
    writeln!(src, "    let a2 = ax * ay;").unwrap();
    writeln!(
        src,
        "    let d = vec3<f32>(t_dot_h / ax, b_dot_h / ay, n_dot_h);"
    )
    .unwrap();
    writeln!(src, "    let d2 = dot(d, d);").unwrap();
    writeln!(
        src,
        "    return 1.0 / (3.14159265 * a2 * d2 * d2 + 0.0001);"
    )
    .unwrap();
    writeln!(src, "}}").unwrap();
    writeln!(src).unwrap();

    // Charlie sheen NDF (Estevez & Kulla 2017)
    writeln!(src, "// Charlie sheen NDF (Estevez & Kulla 2017)").unwrap();
    writeln!(
        src,
        "fn distribution_charlie(n_dot_h: f32, sheen_alpha: f32) -> f32 {{"
    )
    .unwrap();
    writeln!(src, "    let inv_alpha = 1.0 / max(sheen_alpha, 0.001);").unwrap();
    writeln!(src, "    let sin2 = 1.0 - n_dot_h * n_dot_h;").unwrap();
    writeln!(
        src,
        "    return (2.0 + inv_alpha) * pow(max(sin2, 0.0), inv_alpha * 0.5) / (2.0 * 3.14159265);"
    )
    .unwrap();
    writeln!(src, "}}").unwrap();
    writeln!(src).unwrap();

    // Neubelt visibility for sheen (cheap V-term for cloth)
    writeln!(src, "// Neubelt sheen visibility term").unwrap();
    writeln!(
        src,
        "fn visibility_neubelt(n_dot_v: f32, n_dot_l: f32) -> f32 {{"
    )
    .unwrap();
    writeln!(
        src,
        "    return 1.0 / (4.0 * (n_dot_l + n_dot_v - n_dot_l * n_dot_v) + 0.0001);"
    )
    .unwrap();
    writeln!(src, "}}").unwrap();
    writeln!(src).unwrap();

    // Thin-film iridescence spectral sensitivity (Belcour & Barla 2017)
    writeln!(src, "// Spectral sensitivity for thin-film iridescence").unwrap();
    writeln!(
        src,
        "fn eval_sensitivity(opd: f32, shift: vec3<f32>) -> vec3<f32> {{"
    )
    .unwrap();
    writeln!(src, "    let phase = 2.0 * 3.14159265 * opd * 1.0e-9;").unwrap();
    writeln!(
        src,
        "    let val = vec3<f32>(cos(phase + shift.x), cos(phase + shift.y), cos(phase + shift.z));"
    )
    .unwrap();
    writeln!(src, "    return val * 0.5 + vec3<f32>(0.5);").unwrap();
    writeln!(src, "}}").unwrap();
    writeln!(src).unwrap();

    // Thin-film iridescence Fresnel
    writeln!(
        src,
        "// Thin-film iridescence Fresnel (Belcour & Barla 2017 approx)"
    )
    .unwrap();
    writeln!(src, "fn fresnel_iridescence(cos_theta: f32, f0: vec3<f32>, film_ior: f32, film_thickness: f32) -> vec3<f32> {{").unwrap();
    writeln!(
        src,
        "    let opd = 2.0 * film_ior * film_thickness * cos_theta;"
    )
    .unwrap();
    writeln!(src, "    let shift = vec3<f32>(0.0, 2.094395, 4.18879);").unwrap();
    writeln!(src, "    let spec = eval_sensitivity(opd, shift);").unwrap();
    writeln!(src, "    return mix(f0, spec, vec3<f32>(0.5));").unwrap();
    writeln!(src, "}}").unwrap();
    writeln!(src).unwrap();
}

/// Byte representation of MaterialUniformData for GPU upload.
pub fn material_params_to_bytes(params: &MaterialParams) -> Vec<u8> {
    let data = MaterialUniformData::from(params);
    let ptr = &data as *const MaterialUniformData as *const u8;
    let size = std::mem::size_of::<MaterialUniformData>();
    // SAFETY: MaterialUniformData is repr(C) with only f32/u32 fields, no padding surprises
    unsafe { std::slice::from_raw_parts(ptr, size) }.to_vec()
}

/// Size in bytes of the SceneUniforms struct as declared in WGSL.
/// view_proj(64) + model(64) + ambient_color(16) + camera_pos(16)
/// + selection_color(16) + fvar_base_words/padding(16) = 192
pub const SCENE_UNIFORMS_SIZE: usize = 192;

/// Size in bytes of MaterialUniforms struct.
pub const MATERIAL_UNIFORMS_SIZE: usize = std::mem::size_of::<MaterialUniformData>();

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gen_flat_shader() {
        let key = MeshShaderKey::fallback();
        let code = gen_mesh_shader(&key);
        assert!(code.source.contains("fn vs_main"));
        assert!(code.source.contains("fn fs_main"));
        assert!(code.source.contains("scene.ambient_color")); // flat color from scene
        assert!(!code.source.contains("MaterialUniforms")); // no material for flat
    }

    #[test]
    fn test_gen_flat_pick_buffer_shader() {
        let key = MeshShaderKey {
            shading: ShadingModel::FlatColor,
            has_normals: false,
            has_uv: false,
            has_color: false,
            pick_buffer_rw: true,
            ..Default::default()
        };
        let code = gen_mesh_shader(&key);
        assert!(code.source.contains("var<storage, read_write> pick_buffer"));
        assert!(code.source.contains("fn render_deep_picks"));
        assert!(code.source.contains("decode_pick_prim_id(scene.ambient_color)"));
    }

    #[test]
    fn test_gen_lit_shader() {
        let key = MeshShaderKey::lit();
        let code = gen_mesh_shader(&key);
        assert!(code.source.contains("fn vs_main"));
        assert!(code.source.contains("fn fs_main"));
        assert!(code.source.contains("world_normal"));
        assert!(code.source.contains("MaterialUniforms"));
        assert!(code.source.contains("LightUniforms"));
        assert!(code.source.contains("light_data"));
        // lit() has has_uv=false: material flags are in group 2, but NO texture bindings (group 3)
        // because naga only includes bindings that are actually used in the shader.
        assert!(
            !code.source.contains("var diffuse_tex:"),
            "lit() without UV must not emit texture var decl"
        );
    }

    #[test]
    fn test_gen_lit_uv_shader() {
        // lit shader WITH UV should have texture bindings in group 3
        let key = MeshShaderKey {
            has_normals: true,
            has_uv: true,
            shading: ShadingModel::BlinnPhong,
            ..Default::default()
        };
        let code = gen_mesh_shader(&key);
        assert!(code.source.contains("diffuse_tex"));
        assert!(code.source.contains("has_diffuse_tex"));
    }

    #[test]
    fn test_gen_pbr_shader() {
        let key = MeshShaderKey::pbr();
        let code = gen_mesh_shader(&key);
        assert!(code.source.contains("ggx_distribution"));
        assert!(code.source.contains("fresnel_schlick"));
        assert!(code.source.contains("smith_ggx"));
        assert!(code.source.contains("metallic"));
        // pbr() has has_uv=false: NO group 3 texture bindings (sampling code is absent)
        assert!(
            !code.source.contains("var normal_tex:"),
            "pbr() without UV must not emit texture var decl"
        );
        // Material struct flags (group 2) are always present for lit shaders
        assert!(code.source.contains("has_diffuse_tex: u32"));
        // sRGB and perturb_normal helpers NOT present without UV (no sampling calls)
        // They are included but only active when has_uv=true
    }

    #[test]
    fn test_gen_pbr_uv_shader() {
        // PBR shader WITH UV should have all 7 texture slots and helpers
        let key = MeshShaderKey {
            has_normals: true,
            has_uv: true,
            shading: ShadingModel::Pbr,
            ..Default::default()
        };
        let code = gen_mesh_shader(&key);
        assert!(code.source.contains("ggx_distribution"));
        assert!(code.source.contains("fresnel_schlick"));
        assert!(code.source.contains("smith_ggx"));
        // All 7 texture slots present
        assert!(code.source.contains("normal_tex"));
        assert!(code.source.contains("roughness_tex"));
        assert!(code.source.contains("metallic_tex"));
        assert!(code.source.contains("opacity_tex"));
        assert!(code.source.contains("emissive_tex"));
        assert!(code.source.contains("occlusion_tex"));
        // sRGB conversion present in PBR with UV
        assert!(code.source.contains("srgb_to_linear"));
        // Normal map perturbation present
        assert!(code.source.contains("perturb_normal"));
    }

    #[test]
    fn test_texture_flags_in_material_struct() {
        let key = MeshShaderKey::pbr();
        let code = gen_mesh_shader(&key);
        assert!(code.source.contains("has_diffuse_tex: u32"));
        assert!(code.source.contains("has_normal_tex: u32"));
        assert!(code.source.contains("has_roughness_tex: u32"));
        assert!(code.source.contains("has_metallic_tex: u32"));
        assert!(code.source.contains("has_opacity_tex: u32"));
        assert!(code.source.contains("has_emissive_tex: u32"));
        assert!(code.source.contains("has_occlusion_tex: u32"));
    }

    // ---------------------------------------------------------------
    // GPU struct layout invariants (size mismatches = silent corruption)
    // ---------------------------------------------------------------

    #[test]
    fn test_material_uniform_data_size() {
        // MaterialUniformData is repr(C) and serialized via raw pointer cast.
        // Any size change silently corrupts GPU data.
        let size = std::mem::size_of::<MaterialUniformData>();
        assert_eq!(
            size, MATERIAL_UNIFORMS_SIZE,
            "MaterialUniformData sizeof must match MATERIAL_UNIFORMS_SIZE"
        );
        // 40 f32 (160 bytes) + 8 u32 (32 bytes) = 192 bytes total
        assert_eq!(
            size, 192,
            "MaterialUniformData = 40*f32 + 8*u32 = 192 bytes"
        );
    }

    #[test]
    fn test_material_uniform_data_alignment() {
        // repr(C) struct must be 4-byte aligned (f32/u32 fields only)
        assert_eq!(std::mem::align_of::<MaterialUniformData>(), 4);
    }

    #[test]
    fn test_material_params_to_bytes_roundtrip() {
        let params = MaterialParams {
            diffuse_color: [0.8, 0.2, 0.1],
            metallic: 0.5,
            roughness: 0.3,
            opacity: 1.0,
            emissive_color: [0.0, 0.0, 0.0],
            ior: 1.5,
            use_vertex_color: false,
            clearcoat: 0.0,
            clearcoat_roughness: 0.03,
            use_specular_workflow: false,
            specular_color: [1.0, 1.0, 1.0],
            displacement: 0.0,
            has_diffuse_tex: true,
            has_normal_tex: false,
            has_roughness_tex: true,
            has_metallic_tex: false,
            has_opacity_tex: false,
            has_emissive_tex: false,
            has_occlusion_tex: false,
            ..Default::default()
        };
        let bytes = material_params_to_bytes(&params);
        assert_eq!(bytes.len(), MATERIAL_UNIFORMS_SIZE);

        // First 3 floats = diffuse_color
        let r = f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        assert!((r - 0.8).abs() < 1e-6, "diffuse_color.r = {}", r);

        // Float at offset 12 = metallic
        let m = f32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
        assert!((m - 0.5).abs() < 1e-6, "metallic = {}", m);

        // u32 at offset 160 = has_diffuse_tex = 1
        let flag = u32::from_le_bytes([bytes[160], bytes[161], bytes[162], bytes[163]]);
        assert_eq!(flag, 1u32, "has_diffuse_tex should be 1");

        // u32 at offset 164 = has_normal_tex = 0
        let flag2 = u32::from_le_bytes([bytes[164], bytes[165], bytes[166], bytes[167]]);
        assert_eq!(flag2, 0u32, "has_normal_tex should be 0");
    }

    #[test]
    fn test_scene_uniforms_size_const() {
        assert_eq!(
            SCENE_UNIFORMS_SIZE, 192,
            "SceneUniforms = view_proj(64) + model(64) + ambient(16) + cam(16) + selection(16) + fvar(16)"
        );
    }

    // ---------------------------------------------------------------
    // WGSL shader/binder cross-check: @location count must match
    // ---------------------------------------------------------------

    #[test]
    fn test_wgsl_location_count_matches_binder() {
        use crate::resource_binder::ResourceBinder;

        let keys = vec![
            ("fallback", MeshShaderKey::fallback()),
            ("lit", MeshShaderKey::lit()),
            ("pbr", MeshShaderKey::pbr()),
        ];

        for (name, key) in keys {
            let binder = ResourceBinder::from_mesh_key(&key);
            let code = gen_mesh_shader(&key);

            let location_count = code
                .source
                .lines()
                .filter(|l| l.contains("@location(") && !l.contains("VertexInput"))
                .count();
            let vb_count = binder.vertex_buffer_count() as usize;

            assert!(
                location_count >= vb_count,
                "[{}] WGSL has {} @location but binder expects {} vertex buffers",
                name,
                location_count,
                vb_count
            );
        }
    }

    #[test]
    fn test_gen_with_uv_and_color() {
        let key = MeshShaderKey {
            has_normals: true,
            has_color: true,
            has_uv: true,
            shading: ShadingModel::BlinnPhong,
            ..Default::default()
        };
        let code = gen_mesh_shader(&key);
        assert!(code.source.contains("@location(2) uv: vec2<f32>"));
        assert!(code.source.contains("@location(3) color: vec3<f32>"));
        // With has_uv=true, texture sampling branches should be in shader
        assert!(code.source.contains("has_diffuse_tex"));
    }

    #[test]
    fn test_gen_face_varying_shader_uses_storage_path() {
        let key = MeshShaderKey {
            has_normals: true,
            has_uv: true,
            has_color: true,
            has_fvar_normals: true,
            has_fvar_uv: true,
            has_fvar_color: true,
            has_fvar_opacity: true,
            shading: ShadingModel::BlinnPhong,
            normal_interp: crate::mesh_shader_key::PrimvarInterp::FaceVarying,
            ..Default::default()
        };
        let code = gen_mesh_shader(&key);
        let vertex_input_start = code
            .source
            .find("struct VertexInput {")
            .expect("shader must emit VertexInput");
        let vertex_output_start = code
            .source
            .find("struct VertexOutput {")
            .expect("shader must emit VertexOutput");
        let vertex_input_block = &code.source[vertex_input_start..vertex_output_start];
        assert!(code.source.contains("face_varying_data"));
        assert!(code.source.contains("load_fvar_vec3"));
        assert!(code.source.contains("load_fvar_vec2"));
        assert!(code.source.contains("load_fvar_scalar"));
        assert!(!vertex_input_block.contains("@location(1) normal: vec3<f32>"));
        assert!(!vertex_input_block.contains("@location(2) uv: vec2<f32>"));
        assert!(!vertex_input_block.contains("@location(3) color: vec3<f32>"));
    }

    #[test]
    fn test_material_uniform_size() {
        assert_eq!(MATERIAL_UNIFORMS_SIZE, 192); // 40 f32 + 8 u32 = 192 bytes
    }

    #[test]
    fn test_material_params_to_bytes() {
        let params = MaterialParams::default();
        let bytes = material_params_to_bytes(&params);
        assert_eq!(bytes.len(), 192);
        // All texture flags should be 0 for default params (start at offset 160)
        for i in 0..7 {
            let flag = u32::from_le_bytes([
                bytes[160 + i * 4],
                bytes[161 + i * 4],
                bytes[162 + i * 4],
                bytes[163 + i * 4],
            ]);
            assert_eq!(flag, 0u32, "tex flag {} should be 0 for default params", i);
        }
    }

    #[test]
    fn test_pbr_with_ibl_emits_group4_bindings() {
        // When has_uv=false, IBL uses group 3 (no texture gap).
        let key_no_uv = MeshShaderKey {
            has_normals: true,
            has_uv: false,
            shading: ShadingModel::Pbr,
            has_ibl: true,
            ..Default::default()
        };
        let code_no_uv = gen_mesh_shader(&key_no_uv);
        assert!(
            code_no_uv.source.contains("@group(3)"),
            "IBL w/o UV must use group 3"
        );
        assert!(
            !code_no_uv.source.contains("@group(4)"),
            "IBL w/o UV must NOT use group 4"
        );

        // When has_uv=true, IBL uses group 4 (textures at group 3).
        let key = MeshShaderKey {
            has_normals: true,
            has_uv: true,
            shading: ShadingModel::Pbr,
            has_ibl: true,
            ..Default::default()
        };
        let code = gen_mesh_shader(&key);
        assert!(
            code.source.contains("@group(4)"),
            "IBL with UV must use group 4"
        );
        assert!(
            code.source.contains("ibl_irradiance"),
            "must have irradiance cubemap"
        );
        assert!(
            code.source.contains("ibl_prefilter"),
            "must have prefilter cubemap"
        );
        assert!(
            code.source.contains("ibl_brdf_lut"),
            "must have BRDF LUT texture"
        );
    }

    #[test]
    fn test_pbr_with_ibl_emits_ibl_helpers() {
        let key = MeshShaderKey {
            has_normals: true,
            shading: ShadingModel::Pbr,
            has_ibl: true,
            ..Default::default()
        };
        let code = gen_mesh_shader(&key);
        assert!(
            code.source.contains("fn dir_to_cube_face"),
            "must emit dir_to_cube_face helper"
        );
        assert!(
            code.source.contains("fn sample_irradiance"),
            "must emit sample_irradiance helper"
        );
        assert!(
            code.source.contains("fn sample_prefilter"),
            "must emit sample_prefilter helper"
        );
    }

    #[test]
    fn test_pbr_with_ibl_emits_ibl_contribution() {
        let key = MeshShaderKey {
            has_normals: true,
            shading: ShadingModel::Pbr,
            has_ibl: true,
            ..Default::default()
        };
        let code = gen_mesh_shader(&key);
        assert!(
            code.source.contains("ibl_diffuse"),
            "must compute ibl_diffuse"
        );
        assert!(
            code.source.contains("ibl_specular"),
            "must compute ibl_specular"
        );
        assert!(
            code.source.contains("ibl_diffuse + ibl_specular"),
            "must sum IBL terms"
        );
    }

    #[test]
    fn test_pbr_without_ibl_no_group4() {
        let key = MeshShaderKey {
            has_normals: true,
            shading: ShadingModel::Pbr,
            has_ibl: false,
            ..Default::default()
        };
        let code = gen_mesh_shader(&key);
        assert!(
            !code.source.contains("@group(4)"),
            "no IBL = no group 4 bindings"
        );
        assert!(
            !code.source.contains("ibl_diffuse"),
            "no IBL = no ibl_diffuse term"
        );
    }

    #[test]
    fn test_front_facing_not_in_vertex_output() {
        // Regression: @builtin(front_facing) is fragment-only, must NOT be in VertexOutput struct.
        // It must be a separate parameter of fs_main instead.
        for shading in [
            ShadingModel::FlatColor,
            ShadingModel::BlinnPhong,
            ShadingModel::Pbr,
        ] {
            let key = MeshShaderKey {
                has_normals: shading != ShadingModel::FlatColor,
                shading,
                ..Default::default()
            };
            let code = gen_mesh_shader(&key);
            // VertexOutput must NOT contain front_facing
            let vo_start = code
                .source
                .find("struct VertexOutput")
                .expect("no VertexOutput");
            let vo_end = code.source[vo_start..].find("};").unwrap() + vo_start;
            let vo_block = &code.source[vo_start..vo_end];
            assert!(
                !vo_block.contains("front_facing"),
                "{:?}: front_facing must not be in VertexOutput struct",
                shading
            );
            // fs_main must accept front_facing as separate builtin parameter
            assert!(
                code.source
                    .contains("@builtin(front_facing) front_facing: bool"),
                "{:?}: fs_main must have @builtin(front_facing) parameter",
                shading
            );
        }
    }

    #[test]
    fn test_blinnphong_with_ibl_flag_ignored() {
        // has_ibl has no effect on BlinnPhong shaders (IBL is PBR-only)
        let key = MeshShaderKey {
            has_normals: true,
            shading: ShadingModel::BlinnPhong,
            has_ibl: true,
            ..Default::default()
        };
        let code = gen_mesh_shader(&key);
        assert!(
            !code.source.contains("@group(4)"),
            "BlinnPhong must not emit IBL group 4"
        );
        assert!(
            !code.source.contains("ibl_diffuse"),
            "BlinnPhong must not emit ibl_diffuse"
        );
    }
}
