//! UsdImagingGL rendering engine.
//!
//! This module provides the main API for rendering USD scenes via Hydra/Storm.
//!
//! # Pipeline
//!
//! Stage → UsdImaging scene indices → HdRenderIndex → HdxTaskController →
//! HdEngine → HdStRenderPass → engine AOV bridge → deferred post-FX replay
//!
//! The engine builds the canonical UsdImaging scene-index chain, wires it into
//! `HdRenderIndex` via `HdSceneIndexAdapterSceneDelegate`, then executes the
//! reference-style HDX task graph before replaying backend-facing post tasks
//! once concrete AOV textures exist.
//!
//! # Color correction
//!
//! Deferred `HdxColorCorrectionTask` replay now stays entirely inside the
//! engine-side post-FX path. Both `sRGB` and `OpenColorIO` execute against the
//! live render AOV textures, which keeps viewport presentation and batch/frame
//! recording on the same reference-style Hydra/HDX flow.

use super::{CullStyle, DrawMode, RenderParams};

use bytemuck::{Pod, Zeroable, cast_slice};
use usd_camera_util::{CameraUtilConformWindowPolicy, CameraUtilFraming};
use usd_gf::{Matrix4d, Vec2i, Vec3d, Vec3i, Vec4d, Vec4f};
use usd_hgi::{
    HgiAttachmentLoadOp,
    enums::{
        HgiBlendFactor, HgiBlendOp, HgiCompareFunction, HgiCullMode, HgiSampleCount,
        HgiSubmitWaitType as WgpuSubmitWait, HgiTextureType, HgiTextureUsage,
    },
    hgi::Hgi,
    sampler::HgiSamplerDesc,
    texture::{HgiTextureDesc, HgiTextureHandle},
    types::HgiFormat,
};
use usd_hgi_wgpu::HgiWgpu;
use usd_tf::Token;
use usd_vt::Value;

use crate::scene_indices::{
    UsdImagingCreateSceneIndicesInfo, UsdImagingSceneIndices, create_scene_indices,
};
use crate::data_source_prim::{
    read_debug_data_source_prim_xform_stats, reset_debug_data_source_prim_xform_stats,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use parking_lot::RwLock;
use usd_core::attribute_query::{
    read_debug_attribute_query_stats, reset_debug_attribute_query_stats,
};
use usd_core::{Prim, TimeCode};
use usd_geom::xformable::{read_debug_xformable_stats, reset_debug_xformable_stats};

use usd_hd::enums::{HdBlendFactor, HdBlendOp, HdCompareFunction, HdCullStyle};
use usd_hd::change_tracker::HdRprimDirtyBits;
use usd_hd::render::{HdEngine, HdRenderIndex, HdRenderPass, HdRprimCollection};
use usd_hd::render::render_delegate::HdRenderDelegate;
use usd_hd_st::draw_item::{HdStDrawItem, HdStDrawItemSharedPtr};
use usd_hd_st::light::HdStLight;
use usd_hd_st::lighting;
use usd_hd_st::render_delegate::HdStRenderDelegate;
use usd_hd_st::render_pass::HdStRenderPass;
use usd_hd_st::render_pass_state::{
    DepthFunc, HdStAovBinding, HdStPolygonRasterMode, HdStRenderPassState,
};
use usd_hd_st::shadow;
use usd_hdx::{
    AovVisMode, HdxAovInputTaskRequest, HdxBoundingBoxTaskParams, HdxColorCorrectionTaskParams,
    HdxColorCorrectionTaskRequest, HdxColorizeSelectionTaskRequest, HdxPresentTaskRequest,
    HdxRenderTaskParams, HdxRenderTaskRequest, HdxShadowTaskParams,
    HdxTaskController, HdxVisualizeAovTaskRequest, SelectionTrackerExt, color_correction_tokens,
};
use usd_hdx::render_setup_task::{
    CameraUtilConformWindowPolicy as HdxCameraUtilConformWindowPolicy,
    CameraUtilFraming as HdxCameraUtilFraming,
};
use usd_sdf::Path;
use vfx_ocio::{GpuLanguage, GpuProcessor, GpuTextureType, Processor};

mod ibl;
mod mesh_sync;
mod picking;
mod skydome;
mod texture;

use ibl::IblGpuPipelines;

const ENGINE_SRGB_POST_SHADER: &str = r#"
@group(0) @binding(0) var input_texture: texture_2d<f32>;
@group(0) @binding(1) var output_texture: texture_storage_2d<rgba16float, write>;
@group(0) @binding(2) var tex_sampler: sampler;

fn linear_to_srgb(v: f32) -> f32 {
    if (v <= 0.0031308) {
        return v * 12.92;
    }
    return 1.055 * pow(v, 1.0 / 2.4) - 0.055;
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let dims = textureDimensions(input_texture);
    if (global_id.x >= dims.x || global_id.y >= dims.y) {
        return;
    }
    let uv =
        vec2<f32>(f32(global_id.x) + 0.5, f32(global_id.y) + 0.5) /
        vec2<f32>(f32(dims.x), f32(dims.y));
    let color = textureSampleLevel(input_texture, tex_sampler, uv, 0.0);
    textureStore(
        output_texture,
        vec2<i32>(global_id.xy),
        vec4<f32>(
            linear_to_srgb(color.r),
            linear_to_srgb(color.g),
            linear_to_srgb(color.b),
            color.a
        )
    );
}
"#;

const ENGINE_VISUALIZE_AOV_COLOR_POST_SHADER: &str = r#"
struct VisualizeUniforms {
    mode: u32,
    channel: i32,
    _pad0: u32,
    _pad1: u32,
    min_depth: f32,
    max_depth: f32,
    _pad2: vec2<f32>,
};

@group(0) @binding(0) var input_texture: texture_2d<f32>;
@group(0) @binding(1) var output_texture: texture_storage_2d<rgba16float, write>;
@group(0) @binding(2) var tex_sampler: sampler;
@group(0) @binding(3) var<uniform> uniforms: VisualizeUniforms;

fn load_input(coord: vec2<i32>) -> vec4<f32> {
    return textureLoad(input_texture, coord, 0);
}

fn luminance(color: vec3<f32>) -> f32 {
    return dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
}

fn false_color(value: f32) -> vec3<f32> {
    let t = clamp(value, 0.0, 1.0);
    if (t < 0.33333334) {
        let u = t / 0.33333334;
        return mix(vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(0.0, 1.0, 0.0), u);
    }
    if (t < 0.6666667) {
        let u = (t - 0.33333334) / 0.33333334;
        return mix(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 1.0, 0.0), u);
    }
    let u = (t - 0.6666667) / 0.33333334;
    return mix(vec3<f32>(1.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), u);
}

fn normalize_normal(normal: vec3<f32>) -> vec3<f32> {
    return 0.5 * normal + 0.5;
}

fn decode_id(sample_value: vec4<f32>) -> u32 {
    let bytes = vec4<u32>(round(clamp(sample_value, vec4<f32>(0.0), vec4<f32>(1.0)) * 255.0));
    return bytes.x | (bytes.y << 8u) | (bytes.z << 16u) | (bytes.w << 24u);
}

fn int_to_vec3(id: u32) -> vec3<f32> {
    let lead_bits = id >> 24u;
    let rest_bits = id & 0x00ffffffu;
    let result = rest_bits ^ lead_bits;
    return vec3<f32>(
        f32(result & 0xffu) / 255.0,
        f32((result >> 8u) & 0xffu) / 255.0,
        f32((result >> 16u) & 0xffu) / 255.0
    );
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let dims = textureDimensions(input_texture);
    if (global_id.x >= dims.x || global_id.y >= dims.y) {
        return;
    }

    let coord = vec2<i32>(global_id.xy);
    let sample_value = load_input(coord);
    var output_value = sample_value;

    switch uniforms.mode {
        case 1u: {
            let grey = luminance(sample_value.rgb);
            output_value = vec4<f32>(vec3<f32>(grey), 1.0);
        }
        case 2u: {
            let grey = luminance(sample_value.rgb);
            output_value = vec4<f32>(false_color(grey), 1.0);
        }
        case 3u: {
            if (uniforms.channel >= 0 && uniforms.channel < 4) {
                let channel_value = sample_value[uniforms.channel];
                output_value = vec4<f32>(vec3<f32>(channel_value), 1.0);
            } else {
                output_value = sample_value;
            }
        }
        case 4u: {
            let decoded = decode_id(sample_value);
            let viz_id = decoded * 11629091u;
            output_value = vec4<f32>(int_to_vec3(viz_id), 1.0);
        }
        case 5u: {
            output_value = vec4<f32>(normalize_normal(sample_value.xyz), 1.0);
        }
        default: {
            output_value = sample_value;
        }
    }

    textureStore(output_texture, coord, output_value);
}
"#;

const ENGINE_VISUALIZE_AOV_DEPTH_POST_SHADER: &str = r#"
struct VisualizeUniforms {
    mode: u32,
    channel: i32,
    _pad0: u32,
    _pad1: u32,
    min_depth: f32,
    max_depth: f32,
    _pad2: vec2<f32>,
};

@group(0) @binding(0) var input_texture: texture_depth_2d;
@group(0) @binding(1) var output_texture: texture_storage_2d<rgba16float, write>;
@group(0) @binding(2) var<uniform> uniforms: VisualizeUniforms;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let dims = textureDimensions(input_texture);
    if (global_id.x >= dims.x || global_id.y >= dims.y) {
        return;
    }

    let coord = vec2<i32>(global_id.xy);
    let depth = textureLoad(input_texture, coord, 0);
    let depth_range = max(uniforms.max_depth - uniforms.min_depth, 1e-6);
    let normalized = clamp((depth - uniforms.min_depth) / depth_range, 0.0, 1.0);
    textureStore(output_texture, coord, vec4<f32>(vec3<f32>(normalized), 1.0));
}
"#;

const ENGINE_SELECTION_POST_SHADER: &str = r#"
struct SelectionUniforms {
    outline_enabled: u32,
    outline_radius: u32,
    enable_locate_highlight: u32,
    _pad0: u32,
    selection_color: vec4<f32>,
    locate_color: vec4<f32>,
};

@group(0) @binding(0) var input_texture: texture_2d<f32>;
@group(0) @binding(1) var prim_id_texture: texture_2d<f32>;
@group(0) @binding(2) var instance_id_texture: texture_2d<f32>;
@group(0) @binding(3) var element_id_texture: texture_2d<f32>;
@group(0) @binding(4) var output_texture: texture_storage_2d<rgba16float, write>;
@group(0) @binding(5) var<uniform> uniforms: SelectionUniforms;
@group(0) @binding(6) var<storage, read> selection_buffer: array<i32>;

fn decode_id(sample_value: vec4<f32>) -> i32 {
    let bytes = vec4<u32>(round(clamp(sample_value, vec4<f32>(0.0), vec4<f32>(1.0)) * 255.0));
    return i32(bytes.x | (bytes.y << 8u) | (bytes.z << 16u) | (bytes.w << 24u));
}

fn mode_color(mode: i32) -> vec4<f32> {
    if (mode == 0) {
        return uniforms.selection_color;
    }
    return uniforms.locate_color;
}

fn selection_match_for_mode(mode: i32, prim_id: i32, instance_id: i32, element_id: i32) -> bool {
    let buffer_len = arrayLength(&selection_buffer);
    if (prim_id < 0 || buffer_len < 3u) {
        return false;
    }
    if (mode == 1 && uniforms.enable_locate_highlight == 0u) {
        return false;
    }
    let mode_count = min(u32(max(selection_buffer[0], 0)), 2u);
    if (u32(mode) >= mode_count) {
        return false;
    }
    let mode_offset = selection_buffer[1u + u32(mode)];
    if (mode_offset <= 0) {
        return false;
    }
    let start = u32(mode_offset);
    if (start + 1u >= buffer_len) {
        return false;
    }
    let min_id = selection_buffer[start];
    let max_id_exclusive = selection_buffer[start + 1u];
    if (prim_id < min_id || prim_id >= max_id_exclusive) {
        return false;
    }
    let slot = start + 2u + u32(prim_id - min_id);
    if (slot >= buffer_len) {
        return false;
    }

    var selection_data = selection_buffer[slot];
    var is_selected = (selection_data & 1) != 0;
    var next_offset = selection_data >> 1;

    if (next_offset != 0 && !is_selected) {
        let next_index = u32(next_offset);
        if (next_index + 2u < buffer_len) {
            let subprim_type = selection_buffer[next_index];
            if (subprim_type == 3) {
                let min_instance = selection_buffer[next_index + 1u];
                let max_instance = selection_buffer[next_index + 2u];
                if (instance_id >= min_instance && instance_id < max_instance) {
                    let instance_slot = next_index + 3u + u32(instance_id - min_instance);
                    if (instance_slot < buffer_len) {
                        selection_data = selection_buffer[instance_slot];
                        is_selected = is_selected || ((selection_data & 1) != 0);
                        next_offset = selection_data >> 1;
                    }
                }
            }
        }
    }

    if (next_offset != 0 && !is_selected) {
        let next_index = u32(next_offset);
        if (next_index + 2u < buffer_len) {
            let subprim_type = selection_buffer[next_index];
            if (subprim_type == 0) {
                let min_element = selection_buffer[next_index + 1u];
                let max_element = selection_buffer[next_index + 2u];
                if (element_id >= min_element && element_id < max_element) {
                    let element_slot = next_index + 3u + u32(element_id - min_element);
                    if (element_slot < buffer_len) {
                        selection_data = selection_buffer[element_slot];
                        is_selected = is_selected || ((selection_data & 1) != 0);
                    }
                }
            }
        }
    }

    return is_selected;
}

fn selection_overlay(prim_id: i32, instance_id: i32, element_id: i32) -> vec4<f32> {
    let buffer_len = arrayLength(&selection_buffer);
    if (buffer_len < 3u) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    let mode_count = min(u32(max(selection_buffer[0], 0)), 2u);
    var overlay = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    for (var mode = 0u; mode < mode_count; mode = mode + 1u) {
        if (selection_match_for_mode(i32(mode), prim_id, instance_id, element_id)) {
            let color = mode_color(i32(mode));
            let rgb = color.a * color.rgb + (1.0 - color.a) * overlay.rgb;
            let a = (1.0 - color.a) * overlay.a;
            overlay = vec4<f32>(rgb, a);
        }
    }
    return overlay;
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let dims = textureDimensions(input_texture);
    if (global_id.x >= dims.x || global_id.y >= dims.y) {
        return;
    }

    let coord = vec2<i32>(global_id.xy);
    let scene_color = textureLoad(input_texture, coord, 0);
    let prim_id = decode_id(textureLoad(prim_id_texture, coord, 0));
    let instance_id = decode_id(textureLoad(instance_id_texture, coord, 0));
    let element_id = decode_id(textureLoad(element_id_texture, coord, 0));
    let direct_overlay = selection_overlay(prim_id, instance_id, element_id);

    let direct_hit = direct_overlay.a < 0.9999;
    var outline_hit = false;
    var outline_overlay = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    if (uniforms.outline_enabled != 0u && !direct_hit) {
        let radius = i32(uniforms.outline_radius);
        for (var dy = -radius; dy <= radius && !outline_hit; dy = dy + 1) {
            for (var dx = -radius; dx <= radius; dx = dx + 1) {
                if (dx == 0 && dy == 0) {
                    continue;
                }
                let neighbor = coord + vec2<i32>(dx, dy);
                if (neighbor.x < 0 || neighbor.y < 0 || neighbor.x >= i32(dims.x) || neighbor.y >= i32(dims.y)) {
                    continue;
                }
                let neighbor_id = decode_id(textureLoad(prim_id_texture, neighbor, 0));
                let neighbor_instance_id = decode_id(textureLoad(instance_id_texture, neighbor, 0));
                let neighbor_element_id = decode_id(textureLoad(element_id_texture, neighbor, 0));
                let neighbor_overlay = selection_overlay(
                    neighbor_id,
                    neighbor_instance_id,
                    neighbor_element_id,
                );
                if (neighbor_overlay.a < 0.9999) {
                    outline_hit = true;
                    outline_overlay = neighbor_overlay;
                    break;
                }
            }
        }
    }

    var output_value = scene_color;
    let overlay = select(outline_overlay, direct_overlay, direct_hit);
    if (direct_hit || outline_hit) {
        output_value = vec4<f32>(overlay.rgb + overlay.a * scene_color.rgb, scene_color.a);
    }

    textureStore(output_texture, coord, output_value);
}
"#;

/// C++ UsdImagingGLRendererSetting::Type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererSettingType {
    Flag,
    Int,
    Float,
    String,
}

/// C++ UsdImagingGLRendererSetting
#[derive(Debug, Clone)]
pub struct RendererSetting {
    pub name: std::string::String,
    pub key: Token,
    pub setting_type: RendererSettingType,
    pub default_value: usd_vt::Value,
}

/// Snapshot of an engine-managed AOV render buffer.
#[derive(Debug, Clone)]
pub struct EngineAovRenderBuffer {
    pub aov_name: Token,
    pub render_buffer_path: Path,
    pub dimensions: Vec3i,
    pub format: HgiFormat,
    pub multi_sampled: bool,
    pub texture: HgiTextureHandle,
}

/// Parameters for constructing a UsdImagingGL engine.
#[derive(Debug, Clone)]
pub struct EngineParameters {
    /// Root path for scene delegation
    pub root_path: Path,

    /// Paths to exclude from rendering
    pub excluded_paths: Vec<Path>,

    /// Paths that are invisible
    pub invised_paths: Vec<Path>,

    /// Scene delegate ID
    pub scene_delegate_id: Path,

    /// Renderer plugin ID (empty token uses default)
    pub renderer_plugin_id: Token,

    /// Whether GPU rendering is enabled
    pub gpu_enabled: bool,

    /// Draw bounding boxes for unloaded prims with extents/extentsHint
    pub display_unloaded_prims_with_bounds: bool,

    /// Allow asynchronous scene processing
    pub allow_asynchronous_scene_processing: bool,

    /// Enable UsdGeomModelAPI draw mode feature
    pub enable_usd_draw_modes: bool,
}

impl Default for EngineParameters {
    fn default() -> Self {
        Self {
            root_path: Path::absolute_root(),
            excluded_paths: Vec::new(),
            invised_paths: Vec::new(),
            scene_delegate_id: Path::absolute_root(),
            renderer_plugin_id: Token::new(""),
            gpu_enabled: true,
            display_unloaded_prims_with_bounds: false,
            allow_asynchronous_scene_processing: false,
            enable_usd_draw_modes: true,
        }
    }
}

impl EngineParameters {
    /// Creates new engine parameters with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the root path.
    pub fn with_root_path(mut self, path: Path) -> Self {
        self.root_path = path;
        self
    }

    /// Sets the excluded paths.
    pub fn with_excluded_paths(mut self, paths: Vec<Path>) -> Self {
        self.excluded_paths = paths;
        self
    }

    /// Sets the renderer plugin ID.
    pub fn with_renderer_plugin_id(mut self, id: Token) -> Self {
        self.renderer_plugin_id = id;
        self
    }

    /// Sets whether GPU rendering is enabled.
    pub fn with_gpu_enabled(mut self, enabled: bool) -> Self {
        self.gpu_enabled = enabled;
        self
    }

    /// Sets whether unloaded prims are drawn with bounding boxes.
    pub fn with_display_unloaded_prims_with_bounds(mut self, enabled: bool) -> Self {
        self.display_unloaded_prims_with_bounds = enabled;
        self
    }
}

fn renderer_setting_type(value: &usd_vt::Value) -> RendererSettingType {
    if value.is::<bool>() {
        RendererSettingType::Flag
    } else if value.is::<i8>()
        || value.is::<i16>()
        || value.is::<i32>()
        || value.is::<i64>()
        || value.is::<u8>()
        || value.is::<u16>()
        || value.is::<u32>()
        || value.is::<u64>()
    {
        RendererSettingType::Int
    } else if value.is::<f32>() || value.is::<f64>() {
        RendererSettingType::Float
    } else {
        RendererSettingType::String
    }
}

#[cfg(feature = "wgpu")]
#[derive(Clone)]
struct SharedWgpuContext {
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct OcioUniforms {
    exposure_mult: f32,
    _pad: [f32; 3],
}

#[derive(Debug, Clone, Default)]
struct EngineOcioSettings {
    display: String,
    view: String,
    color_space: String,
    looks: String,
    lut3d_size: i32,
}

struct OcioGpuPass {
    config: Option<vfx_ocio::Config>,
    loaded_config: bool,
    settings_key: String,
    io_layout: Option<wgpu::BindGroupLayout>,
    uniform_layout: Option<wgpu::BindGroupLayout>,
    lut_layout: Option<wgpu::BindGroupLayout>,
    pipeline: Option<wgpu::ComputePipeline>,
    io_sampler: Option<wgpu::Sampler>,
    uniform_buf: Option<wgpu::Buffer>,
    uniform_bg: Option<wgpu::BindGroup>,
    gpu_processor: Option<GpuProcessor>,
    lut_textures: HashMap<String, (wgpu::Texture, wgpu::TextureView)>,
    lut_samplers: HashMap<String, wgpu::Sampler>,
}

impl OcioGpuPass {
    fn new() -> Self {
        Self {
            config: None,
            loaded_config: false,
            settings_key: String::new(),
            io_layout: None,
            uniform_layout: None,
            lut_layout: None,
            pipeline: None,
            io_sampler: None,
            uniform_buf: None,
            uniform_bg: None,
            gpu_processor: None,
            lut_textures: HashMap::new(),
            lut_samplers: HashMap::new(),
        }
    }

    fn reset_gpu_resources(&mut self) {
        self.settings_key.clear();
        self.io_layout = None;
        self.uniform_layout = None;
        self.lut_layout = None;
        self.pipeline = None;
        self.io_sampler = None;
        self.uniform_buf = None;
        self.uniform_bg = None;
        self.gpu_processor = None;
        self.lut_textures.clear();
        self.lut_samplers.clear();
    }

    fn load_config(&mut self) {
        if self.loaded_config {
            return;
        }
        self.loaded_config = true;
        let config = if let Ok(ocio_path) = std::env::var("OCIO") {
            if !ocio_path.is_empty() {
                match vfx_ocio::Config::from_file(&ocio_path) {
                    Ok(cfg) => {
                        log::info!("[engine][ocio] loaded config from $OCIO: {ocio_path}");
                        cfg
                    }
                    Err(err) => {
                        log::warn!(
                            "[engine][ocio] failed to load $OCIO ({ocio_path}): {err}; using builtin ACES 1.3"
                        );
                        vfx_ocio::builtin::aces_1_3()
                    }
                }
            } else {
                log::info!("[engine][ocio] $OCIO empty; using builtin ACES 1.3");
                vfx_ocio::builtin::aces_1_3()
            }
        } else {
            log::info!("[engine][ocio] $OCIO not set; using builtin ACES 1.3");
            vfx_ocio::builtin::aces_1_3()
        };
        self.config = Some(config);
    }

    fn processor_for_settings(&mut self, settings: &EngineOcioSettings) -> Option<Processor> {
        self.load_config();
        let config = self.config.as_ref()?;
        let display = if settings.display.is_empty() {
            config
                .default_display()
                .map(str::to_owned)
                .unwrap_or_default()
        } else {
            settings.display.clone()
        };
        let view = if settings.view.is_empty() {
            config.default_view(&display).map(str::to_owned).unwrap_or_default()
        } else {
            settings.view.clone()
        };
        let src = if settings.color_space.is_empty() {
            config
                .colorspace("scene_linear")
                .map(|cs| cs.name().to_string())
                .unwrap_or_else(|| "scene_linear".to_string())
        } else {
            settings.color_space.clone()
        };
        if display.is_empty() || view.is_empty() {
            log::warn!("[engine][ocio] no display/view available");
            return None;
        }
        let processor = if settings.looks.is_empty() {
            config.display_processor(&src, &display, &view)
        } else {
            match config.processor_with_looks(&src, &src, &settings.looks) {
                Ok(looks_proc) => match config.display_processor(&src, &display, &view) {
                    Ok(display_proc) => Processor::combine(&looks_proc, &display_proc),
                    Err(err) => Err(err),
                },
                Err(err) => Err(err),
            }
        };
        match processor {
            Ok(processor) => Some(processor),
            Err(err) => {
                log::warn!(
                    "[engine][ocio] failed to create display processor: {src} -> {display}/{view}: {err}"
                );
                None
            }
        }
    }

    fn rebuild_if_needed(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        settings: &EngineOcioSettings,
    ) -> Result<(), ()> {
        let settings_key = format!(
            "{}/{}/{}/{}/{}",
            settings.display,
            settings.view,
            settings.color_space,
            settings.looks,
            settings.lut3d_size
        );
        if settings_key == self.settings_key && self.pipeline.is_some() {
            return Ok(());
        }

        let Some(processor) = self.processor_for_settings(settings) else {
            return Err(());
        };
        let gpu_processor = GpuProcessor::from_processor_wgsl(&processor).map_err(|_| ())?;
        if settings.lut3d_size > 0 {
            log::trace!(
                "[engine][ocio] requested LUT size {} (current GPU path follows vfx-ocio generated textures)",
                settings.lut3d_size
            );
        }
        let raw_wgsl = gpu_processor.generate_shader(GpuLanguage::Wgsl);
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("engine_ocio_shader"),
            source: wgpu::ShaderSource::Wgsl(wrap_ocio_wgsl(raw_wgsl.fragment_code()).into()),
        });

        let io_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("engine_ocio_io_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("engine_ocio_uniform_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let mut lut_entries = Vec::new();
        for desc in gpu_processor.batch_texture_descriptors() {
            lut_entries.push(wgpu::BindGroupLayoutEntry {
                binding: desc.binding_index as u32 * 2,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: match desc.texture_type {
                        GpuTextureType::Texture1D | GpuTextureType::Texture2D => {
                            wgpu::TextureViewDimension::D2
                        }
                        GpuTextureType::Texture3D => wgpu::TextureViewDimension::D3,
                    },
                    multisampled: false,
                },
                count: None,
            });
            lut_entries.push(wgpu::BindGroupLayoutEntry {
                binding: desc.binding_index as u32 * 2 + 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            });
        }
        let lut_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("engine_ocio_lut_layout"),
            entries: &lut_entries,
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("engine_ocio_pipeline_layout"),
            bind_group_layouts: &[&io_layout, &uniform_layout, &lut_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("engine_ocio_pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader_module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("engine_ocio_uniform_buffer"),
            size: std::mem::size_of::<OcioUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(
            &uniform_buf,
            0,
            bytemuck::bytes_of(&OcioUniforms {
                exposure_mult: 1.0,
                _pad: [0.0; 3],
            }),
        );
        let uniform_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("engine_ocio_uniform_bind_group"),
            layout: &uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buf.as_entire_binding(),
            }],
        });
        let io_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("engine_ocio_io_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        self.upload_luts(device, queue, &gpu_processor);
        self.settings_key = settings_key;
        self.io_layout = Some(io_layout);
        self.uniform_layout = Some(uniform_layout);
        self.lut_layout = Some(lut_layout);
        self.pipeline = Some(pipeline);
        self.io_sampler = Some(io_sampler);
        self.uniform_buf = Some(uniform_buf);
        self.uniform_bg = Some(uniform_bg);
        self.gpu_processor = Some(gpu_processor);
        Ok(())
    }

    fn execute(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        input_view: &wgpu::TextureView,
        output_view: &wgpu::TextureView,
        width: u32,
        height: u32,
    ) -> bool {
        let (Some(io_layout), Some(uniform_bg), Some(pipeline), Some(io_sampler), Some(lut_layout)) = (
            self.io_layout.as_ref(),
            self.uniform_bg.as_ref(),
            self.pipeline.as_ref(),
            self.io_sampler.as_ref(),
            self.lut_layout.as_ref(),
        ) else {
            return false;
        };
        let io_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("engine_ocio_io_bind_group"),
            layout: io_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(input_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(output_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(io_sampler),
                },
            ],
        });
        let Some(lut_bg) = self.create_lut_bind_group(device, lut_layout) else {
            return false;
        };
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("engine_ocio_encoder"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("engine_ocio_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &io_bg, &[]);
            pass.set_bind_group(1, uniform_bg, &[]);
            pass.set_bind_group(2, &lut_bg, &[]);
            pass.dispatch_workgroups(width.div_ceil(8), height.div_ceil(8), 1);
        }
        queue.submit(Some(encoder.finish()));
        true
    }

    fn upload_luts(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        gpu_processor: &GpuProcessor,
    ) {
        self.lut_textures.clear();
        self.lut_samplers.clear();

        for texture in gpu_processor.textures() {
            let (wgpu_texture, wgpu_view) = match texture.texture_type {
                GpuTextureType::Texture1D => {
                    let tex = device.create_texture(&wgpu::TextureDescriptor {
                        label: Some(&texture.name),
                        size: wgpu::Extent3d {
                            width: texture.width,
                            height: 1,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Rgba32Float,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                        view_formats: &[],
                    });
                    queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: &tex,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        cast_slice(&texture.data),
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(texture.width * 16),
                            rows_per_image: Some(1),
                        },
                        wgpu::Extent3d {
                            width: texture.width,
                            height: 1,
                            depth_or_array_layers: 1,
                        },
                    );
                    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
                    (tex, view)
                }
                GpuTextureType::Texture2D => {
                    let tex = device.create_texture(&wgpu::TextureDescriptor {
                        label: Some(&texture.name),
                        size: wgpu::Extent3d {
                            width: texture.width,
                            height: texture.height,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Rgba32Float,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                        view_formats: &[],
                    });
                    queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: &tex,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        cast_slice(&texture.data),
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(texture.width * 16),
                            rows_per_image: Some(texture.height),
                        },
                        wgpu::Extent3d {
                            width: texture.width,
                            height: texture.height,
                            depth_or_array_layers: 1,
                        },
                    );
                    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
                    (tex, view)
                }
                GpuTextureType::Texture3D => {
                    let tex = device.create_texture(&wgpu::TextureDescriptor {
                        label: Some(&texture.name),
                        size: wgpu::Extent3d {
                            width: texture.width,
                            height: texture.height,
                            depth_or_array_layers: texture.depth,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D3,
                        format: wgpu::TextureFormat::Rgba32Float,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                        view_formats: &[],
                    });
                    queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: &tex,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        cast_slice(&texture.data),
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(texture.width * 16),
                            rows_per_image: Some(texture.height),
                        },
                        wgpu::Extent3d {
                            width: texture.width,
                            height: texture.height,
                            depth_or_array_layers: texture.depth,
                        },
                    );
                    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
                    (tex, view)
                }
            };
            let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some(&format!("{} sampler", texture.name)),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Nearest,
                ..Default::default()
            });
            self.lut_textures
                .insert(texture.name.clone(), (wgpu_texture, wgpu_view));
            self.lut_samplers.insert(texture.name.clone(), sampler);
        }
    }

    fn create_lut_bind_group(
        &self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
    ) -> Option<wgpu::BindGroup> {
        let gpu_processor = self.gpu_processor.as_ref()?;
        let descriptors = gpu_processor.batch_texture_descriptors();
        if descriptors.is_empty() {
            return Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("engine_ocio_empty_lut_bind_group"),
                layout,
                entries: &[],
            }));
        }

        let mut entries = Vec::new();
        for desc in descriptors {
            let (_, view) = self.lut_textures.get(&desc.name)?;
            let sampler = self.lut_samplers.get(&desc.name)?;
            entries.push(wgpu::BindGroupEntry {
                binding: desc.binding_index as u32 * 2,
                resource: wgpu::BindingResource::TextureView(view),
            });
            entries.push(wgpu::BindGroupEntry {
                binding: desc.binding_index as u32 * 2 + 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            });
        }
        Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("engine_ocio_lut_bind_group"),
            layout,
            entries: &entries,
        }))
    }
}

fn wrap_ocio_wgsl(raw_wgsl: &str) -> String {
    let split_idx = raw_wgsl.find("@group(0)").unwrap_or(raw_wgsl.len());
    let ocio_fns = &raw_wgsl[..split_idx];
    format!(
        r#"
struct OcioUniforms {{
    exposure_mult: f32,
    _pad1: f32,
    _pad2: f32,
    _pad3: f32,
}}
@group(1) @binding(0) var<uniform> u_ocio: OcioUniforms;

@group(0) @binding(0) var input_texture: texture_2d<f32>;
@group(0) @binding(1) var output_texture: texture_storage_2d<rgba16float, write>;
@group(0) @binding(2) var tex_sampler: sampler;

{ocio_fns}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {{
    let dims = textureDimensions(input_texture);
    if (global_id.x >= dims.x || global_id.y >= dims.y) {{
        return;
    }}
    let uv =
        vec2<f32>(f32(global_id.x) + 0.5, f32(global_id.y) + 0.5) /
        vec2<f32>(f32(dims.x), f32(dims.y));
    let color = textureSampleLevel(input_texture, tex_sampler, uv, 0.0);
    let exposed = vec4<f32>(color.rgb * u_ocio.exposure_mult, color.a);
    let result = ocio_transform(exposed);
    textureStore(output_texture, vec2<i32>(global_id.xy), result);
}}
"#
    )
}

/// Pick result from intersection testing.
#[derive(Debug, Clone, PartialEq)]
pub struct IntersectionResult {
    /// Hit point in world space
    pub hit_point: Vec3d,

    /// Hit normal in world space
    pub hit_normal: Vec3d,

    /// Path to the hit primitive
    pub hit_prim_path: Path,

    /// Path to the hit instancer (if applicable)
    pub hit_instancer_path: Path,

    /// Instance index of the hit
    pub hit_instance_index: i32,
}

/// Parameters for picking operations.
#[derive(Debug, Clone, PartialEq)]
pub struct PickParams {
    /// Resolve mode for picking (e.g., "resolveDeep", "resolveNearestToCenter")
    pub resolve_mode: Token,
    /// Pick target (prims, faces, edges, points).
    pub pick_target: Token,
}

impl Default for PickParams {
    fn default() -> Self {
        Self {
            resolve_mode: Token::new("resolveNearestToCenter"),
            pick_target: usd_hdx::pick_tokens::pick_prims_and_instances(),
        }
    }
}

impl PickParams {
    /// Creates new pick parameters with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the resolve mode.
    pub fn with_resolve_mode(mut self, mode: Token) -> Self {
        self.resolve_mode = mode;
        self
    }

    /// Sets the pick target.
    pub fn with_pick_target(mut self, target: Token) -> Self {
        self.pick_target = target;
        self
    }
}

/// The main UsdImagingGL rendering engine.
///
/// This is the primary entry point for rendering USD scenes with OpenGL/Hydra.
/// It manages the Hydra render delegate, scene index, and provides high-level
/// rendering operations.
///
/// # Example
///
/// ```ignore
/// use usd_imaging::gl::{Engine, EngineParameters, RenderParams};
/// use usd_core::{Stage, common::InitialLoadSet};
///
/// let params = EngineParameters::new()
///     .with_gpu_enabled(true);
/// let mut engine = Engine::new(params);
///
/// let stage = Stage::open("scene.usda", InitialLoadSet::LoadAll)
///     .expect("Failed to open stage");
/// let root = stage.get_pseudo_root();
/// let render_params = RenderParams::default();
///
/// engine.render(&root, &render_params);
/// ```
pub struct Engine {
    /// Engine parameters
    params: EngineParameters,

    /// Storm render delegate (wrapped for render index)
    render_delegate: Arc<RwLock<HdStRenderDelegate>>,

    /// Render index built from the canonical UsdImaging scene-index chain.
    render_index: Option<Arc<Mutex<HdRenderIndex>>>,

    /// Canonical UsdImaging scene-index chain for the currently prepared stage.
    scene_indices: Option<UsdImagingSceneIndices>,

    /// Current render pass
    render_pass: Option<HdStRenderPass>,

    /// Render pass state
    render_pass_state: HdStRenderPassState,

    /// Reference-style task controller for the Hydra task graph.
    task_controller: Option<HdxTaskController>,

    /// Hydra engine orchestrating Sync/Prepare/Commit/Execute across tasks.
    hd_engine: HdEngine,

    /// wgpu HGI backend
    wgpu_hgi: Option<Arc<RwLock<HgiWgpu>>>,

    /// Optional UI-owned wgpu context used to build Hydra on the same device as egui.
    #[cfg(feature = "wgpu")]
    shared_wgpu_context: Option<SharedWgpuContext>,

    /// wgpu color render target
    wgpu_color_texture: Option<HgiTextureHandle>,

    /// Secondary color target used for post-FX ping-pong passes.
    wgpu_post_color_texture: Option<HgiTextureHandle>,

    /// wgpu depth render target
    wgpu_depth_texture: Option<HgiTextureHandle>,

    /// Main primId AOV render target (RGBA encodes int32 primId per pixel).
    wgpu_prim_id_texture: Option<HgiTextureHandle>,

    /// Main instanceId AOV render target (RGBA encodes int32 instanceId per pixel).
    wgpu_instance_id_texture: Option<HgiTextureHandle>,

    /// Main elementId AOV render target (RGBA encodes int32 elementId per pixel).
    wgpu_element_id_texture: Option<HgiTextureHandle>,

    /// Lazily-created auxiliary AOV targets keyed by Hydra AOV name.
    wgpu_aux_aov_textures: HashMap<String, HgiTextureHandle>,

    /// wgpu 1x1 color pick target (RGBA encodes int32 primId, little-endian).
    wgpu_pick_color_texture: Option<HgiTextureHandle>,

    /// wgpu 1x1 color pick target for instanceId (RGBA encodes int32 instanceId).
    wgpu_pick_instance_texture: Option<HgiTextureHandle>,

    /// wgpu 1x1 depth pick target.
    wgpu_pick_depth_texture: Option<HgiTextureHandle>,

    /// Cached pick target dimensions for resize detection.
    wgpu_pick_rt_size: Vec2i,

    /// Full-resolution ID color target (RGBA encodes primId per pixel).
    /// Rendered alongside the main color pass for instant GPU picking.
    wgpu_id_color_texture: Option<HgiTextureHandle>,

    /// Full-resolution ID depth target (for correct occlusion in ID pass).
    wgpu_id_depth_texture: Option<HgiTextureHandle>,

    /// Cached full-res ID target dimensions for resize detection.
    wgpu_id_rt_size: Vec2i,

    /// Enable full-resolution ID pass for instant GPU picking.
    /// When true, an ID pass runs every frame at viewport resolution.
    id_pass_enabled: bool,

    /// Cached render target dimensions for resize detection
    wgpu_rt_size: Vec2i,

    /// Persistent staging buffer for efficient GPU->CPU readback (avoids per-frame alloc)
    wgpu_staging: usd_hgi_wgpu::StagingReadback,

    /// Root transform
    root_transform: Matrix4d,

    /// Per-drawable model transforms derived from synchronized Hydra rprims.
    /// Includes synthetic instance paths for GPU instancing.
    /// World transforms cache — retained for bbox computation and dirty-transform path.
    /// No longer used for rendering (draw items carry their own transforms per ARCH-03).
    model_transforms: HashMap<usd_sdf::Path, [[f64; 4]; 4]>,

    /// Path -> Hydra primId map for GPU ID picking.
    rprim_ids_by_path: HashMap<usd_sdf::Path, i32>,

    /// True when rprim IDs must be rebuilt (rprim add/remove, not time change).
    rprim_ids_dirty: bool,

    /// True when scene_bbox must be recomputed (structural change, not animation).
    scene_bbox_dirty: bool,

    /// Root visibility
    root_visible: bool,

    /// Current camera path
    camera_path: Option<Path>,

    /// View matrix
    view_matrix: Matrix4d,

    /// Projection matrix
    projection_matrix: Matrix4d,

    /// Render buffer size
    render_buffer_size: Vec2i,

    /// Selection color
    selection_color: Vec4f,

    /// Selected paths
    selected_paths: Vec<Path>,
    /// Located / rollover-highlighted paths.
    located_paths: Vec<Path>,

    /// Current time code
    time_code: TimeCode,

    /// True when Hydra rprim state must be re-synchronized before rendering.
    mesh_sync_dirty: bool,

    /// After `set_time` (C++ `SetRenderFrameTimecode` / scene globals frame): viewer-side
    /// caches (bbox, `model_transforms`, …) must be refreshed from the render index after
    /// `HdEngine::execute`, **without** implying geometry/topology invalidation like
    /// `mesh_sync_dirty`. Reference: `UsdImagingGLEngine` does not conflate time change with
    /// mesh pipeline invalidation; dirty prims come from the scene index / change tracker.
    viewer_bookkeeping_pending: bool,

    /// True when render pass draw items must be rebuilt from render index.
    draw_items_dirty: bool,

    /// True when camera/viewport/params changed and a new frame is needed.
    render_needs_update: bool,

    /// Last draw-item prim path list applied to render pass.
    last_storm_item_paths: Vec<Path>,

    /// Camera framing for filmback-to-pixel mapping
    framing: CameraUtilFraming,

    /// Override window policy (None = use camera's own policy)
    override_window_policy: Option<CameraUtilConformWindowPolicy>,

    /// Window policy for scene cameras
    window_policy: CameraUtilConformWindowPolicy,

    /// Current frame for scene globals (C++ SetSceneGlobalsCurrentFrame).
    /// Updated in set_time(). Used by scene index chain when wired.
    scene_globals_current_frame: f64,

    /// Active render settings prim path (C++ sceneGlobalsSceneIndex).
    active_render_settings_prim_path: usd_sdf::Path,
    /// Active render pass prim path (C++ sceneGlobalsSceneIndex).
    active_render_pass_prim_path: usd_sdf::Path,

    /// Whether GPU rendering is enabled (C++ _gpuEnabled).
    gpu_enabled: bool,
    /// Whether the renderer is paused (C++ via renderDelegate->Pause/Resume).
    renderer_paused: bool,
    /// Whether the renderer is stopped (C++ via renderDelegate->Stop/Restart).
    renderer_stopped: bool,
    /// Current active AOV (C++ set via taskController->SetRenderOutputs).
    current_aov: Token,
    /// Whether presentation is enabled (C++ _allowPresentation).
    enable_presentation: bool,

    /// Whether the engine has been prepared for rendering
    prepared: bool,

    /// Skip rendering for one frame after device invalidation to let
    /// all stale GPU resources (TextureViews, BindGroups) be fully dropped.
    device_just_invalidated: bool,

    /// Converged flag for progressive rendering
    converged: bool,

    /// Last applied refinement level (for change detection).
    /// When complexity changes, meshes must be re-synced.
    last_refine_level: i32,

    /// Scene bounding box (min, max) derived from synchronized Hydra mesh state.
    /// Used for auto-framing camera on load.
    scene_bbox: Option<([f32; 3], [f32; 3])>,

    /// Last loaded HDRI path (for cache invalidation — skip reload if unchanged).
    ibl_hdri_path: Option<String>,

    /// Whether dome light / fallback IBL is enabled (from viewer UI).
    dome_light_enabled: bool,
    /// IBL textures need reload (dome light path/enabled changed).
    ibl_dirty: bool,

    /// Optional explicit HDRI path for fallback dome light.
    dome_light_texture_path: Option<String>,

    /// Reusable texture cache: resolved-path(+colorspace) -> texture/sampler handles.
    texture_cache: HashMap<String, (HgiTextureHandle, usd_hgi::sampler::HgiSamplerHandle)>,

    /// Cached GPU compute pipelines for IBL dome light prefiltering.
    /// Created on first use, reused across HDRI reloads.
    ibl_gpu_pipelines: Option<IblGpuPipelines>,

    /// Shadow atlas depth texture (texture_depth_2d_array, Depth32Float, 2048x2048 x MAX_SHADOWS).
    wgpu_shadow_atlas: Option<HgiTextureHandle>,

    /// Shadow depth-only render pipeline.
    shadow_pipeline: Option<wgpu::RenderPipeline>,

    /// Shadow pass bind group layout (single UBO: world_to_shadow + model = 128 bytes).
    shadow_bind_group_layout: Option<wgpu::BindGroupLayout>,

    /// Shadow pass uniform buffer (128 bytes per shadow slice).
    shadow_uniform_buf: Option<wgpu::Buffer>,

    /// Shadow comparison sampler (sampler_comparison, LessEqual).
    wgpu_shadow_sampler: Option<usd_hgi::sampler::HgiSamplerHandle>,

    /// Skydome background render pipeline (fullscreen triangle sampling env cubemap).
    skydome_pipeline: Option<wgpu::RenderPipeline>,
    /// Color target format used to build the current skydome pipeline.
    skydome_pipeline_color_format: Option<wgpu::TextureFormat>,
    /// Skydome bind group layout (uniform + cubemap texture + sampler).
    skydome_bind_group_layout: Option<wgpu::BindGroupLayout>,
    /// Skydome uniform buffer (inv_proj + inv_view = 2x mat4x4f = 128 bytes).
    skydome_uniform_buf: Option<wgpu::Buffer>,

    /// sRGB post-FX compute bind-group layout.
    srgb_post_bind_group_layout: Option<wgpu::BindGroupLayout>,
    /// sRGB post-FX compute pipeline.
    srgb_post_pipeline: Option<wgpu::ComputePipeline>,
    /// sRGB post-FX sampler for reading the source color target.
    srgb_post_sampler: Option<wgpu::Sampler>,

    /// Visualize-AOV post-FX layout for color-like textures.
    visualize_aov_color_bind_group_layout: Option<wgpu::BindGroupLayout>,
    /// Visualize-AOV post-FX layout for depth textures.
    visualize_aov_depth_bind_group_layout: Option<wgpu::BindGroupLayout>,
    /// Visualize-AOV post-FX pipeline for color-like textures.
    visualize_aov_color_pipeline: Option<wgpu::ComputePipeline>,
    /// Visualize-AOV post-FX pipeline for depth textures.
    visualize_aov_depth_pipeline: Option<wgpu::ComputePipeline>,
    /// Visualize-AOV post-FX sampler.
    visualize_aov_sampler: Option<wgpu::Sampler>,
    /// Visualize-AOV post-FX uniform buffer.
    visualize_aov_uniform_buf: Option<wgpu::Buffer>,

    /// Selection post-FX bind-group layout.
    selection_post_bind_group_layout: Option<wgpu::BindGroupLayout>,
    /// Selection post-FX compute pipeline.
    selection_post_pipeline: Option<wgpu::ComputePipeline>,
    /// Selection post-FX sampler.
    selection_post_sampler: Option<wgpu::Sampler>,
    /// Selection post-FX uniform buffer.
    selection_post_uniform_buf: Option<wgpu::Buffer>,

    /// Color correction mode: "disabled", "sRGB", "openColorIO". C++: colorCorrectionMode.
    color_correction_mode: String,
    /// Stored OCIO settings used when per-frame RenderParams leave them empty.
    ocio_settings: EngineOcioSettings,
    /// Engine-side GPU OpenColorIO post-processing state.
    ocio_post_pass: OcioGpuPass,

    /// Renderer setting overrides authored by clients of the engine.
    renderer_settings: HashMap<Token, usd_vt::Value>,

    /// Shared material cache: material_path -> (params, features, tex_paths).
    /// Rebuilt when any prim has DIRTY_MATERIAL_ID. Avoids per-frame
    /// re-collection from Hydra sprims.
    material_cache: HashMap<
        Path,
        (
            usd_hd_st::wgsl_code_gen::MaterialParams,
            (bool, bool),
            HashMap<usd_tf::Token, String>,
        ),
    >,

    // --- Progressive rprim sync ---
    /// Rprims to sync per frame during initial load (0 = disabled).
    /// TODO: wire into sync loop to throttle initial rprim batch size.
    #[allow(dead_code)]
    progressive_sync_budget: usize,
    /// Whether progressive sync is currently in progress.
    progressive_sync_active: bool,
    /// Rprims synced so far in the current progressive pass.
    progressive_synced: usize,
    /// Total dirty rprims in the current progressive pass.
    progressive_total: usize,
    /// Whether phases 1-4 have been completed for the current progressive pass.
    progressive_phases_done: bool,

    /// Marks initial scene load (set by invalidate_scene, cleared after sync completes).
    initial_scene_load: bool,
}

impl Engine {
    /// Creates a new UsdImagingGL engine with the given parameters.
    ///
    /// # Arguments
    ///
    /// * `params` - Engine configuration parameters
    pub fn new(params: EngineParameters) -> Self {
        usd_trace::trace_scope!("Engine::new");
        let mut render_pass_state = HdStRenderPassState::new();
        // Engine color RT is Float16Vec4; set fallback so the render pass
        // descriptor matches even when no explicit AOV bindings are set.
        render_pass_state.set_fallback_attachment_formats(
            HgiFormat::Float16Vec4,
            HgiFormat::Float32,
        );
        let gpu_enabled = params.gpu_enabled;

        Self {
            params,
            render_delegate: Arc::new(RwLock::new(HdStRenderDelegate::new())),
            render_index: None,
            scene_indices: None,
            render_pass: None,
            render_pass_state,
            task_controller: None,
            hd_engine: HdEngine::new(),
            wgpu_hgi: None,
            #[cfg(feature = "wgpu")]
            shared_wgpu_context: None,
            wgpu_color_texture: None,
            wgpu_post_color_texture: None,
            wgpu_depth_texture: None,
            wgpu_prim_id_texture: None,
            wgpu_instance_id_texture: None,
            wgpu_element_id_texture: None,
            wgpu_aux_aov_textures: HashMap::new(),
            wgpu_pick_color_texture: None,
            wgpu_pick_instance_texture: None,
            wgpu_pick_depth_texture: None,
            wgpu_pick_rt_size: Vec2i::new(0, 0),
            wgpu_id_color_texture: None,
            wgpu_id_depth_texture: None,
            wgpu_id_rt_size: Vec2i::new(0, 0),
            id_pass_enabled: true,
            wgpu_rt_size: Vec2i::new(0, 0),
            wgpu_staging: usd_hgi_wgpu::StagingReadback::new(),
            root_transform: Matrix4d::identity(),
            model_transforms: HashMap::with_capacity(1024),
            rprim_ids_by_path: HashMap::with_capacity(1024),
            rprim_ids_dirty: true,
            scene_bbox_dirty: true,
            root_visible: true,
            camera_path: None,
            view_matrix: Matrix4d::identity(),
            projection_matrix: Matrix4d::identity(),
            render_buffer_size: Vec2i::new(1920, 1080),
            selection_color: Vec4f::new(1.0, 1.0, 0.0, 1.0),
            selected_paths: Vec::new(),
            located_paths: Vec::new(),
            time_code: TimeCode::default_time(),
            scene_globals_current_frame: 0.0,
            active_render_settings_prim_path: usd_sdf::Path::empty(),
            active_render_pass_prim_path: usd_sdf::Path::empty(),
            gpu_enabled,
            renderer_paused: false,
            renderer_stopped: false,
            current_aov: Token::new("color"),
            enable_presentation: true,
            mesh_sync_dirty: true,
            viewer_bookkeeping_pending: false,
            draw_items_dirty: true,
            render_needs_update: true,
            last_storm_item_paths: Vec::new(),
            framing: CameraUtilFraming::default(),
            override_window_policy: None,
            window_policy: CameraUtilConformWindowPolicy::Fit,
            prepared: false,
            device_just_invalidated: false,
            converged: true, // Default to converged (non-progressive rendering)
            last_refine_level: 0,
            scene_bbox: None,
            ibl_hdri_path: None,
            dome_light_enabled: false,
            ibl_dirty: false,
            dome_light_texture_path: None,
            texture_cache: HashMap::with_capacity(64),
            ibl_gpu_pipelines: None,
            wgpu_shadow_atlas: None,
            shadow_pipeline: None,
            shadow_bind_group_layout: None,
            shadow_uniform_buf: None,
            wgpu_shadow_sampler: None,
            skydome_pipeline: None,
            skydome_pipeline_color_format: None,
            skydome_bind_group_layout: None,
            skydome_uniform_buf: None,
            srgb_post_bind_group_layout: None,
            srgb_post_pipeline: None,
            srgb_post_sampler: None,
            visualize_aov_color_bind_group_layout: None,
            visualize_aov_depth_bind_group_layout: None,
            visualize_aov_color_pipeline: None,
            visualize_aov_depth_pipeline: None,
            visualize_aov_sampler: None,
            visualize_aov_uniform_buf: None,
            selection_post_bind_group_layout: None,
            selection_post_pipeline: None,
            selection_post_sampler: None,
            selection_post_uniform_buf: None,
            color_correction_mode: "disabled".to_string(),
            ocio_settings: EngineOcioSettings {
                lut3d_size: 65,
                ..EngineOcioSettings::default()
            },
            ocio_post_pass: OcioGpuPass::new(),
            renderer_settings: HashMap::new(),
            material_cache: HashMap::new(),
            progressive_sync_budget: 256,
            progressive_sync_active: false,
            progressive_synced: 0,
            progressive_total: 0,
            progressive_phases_done: false,

            initial_scene_load: false,
        }
    }

    /// Creates a new engine with default parameters.
    pub fn with_defaults() -> Self {
        Self::new(EngineParameters::default())
    }

    #[cfg(feature = "wgpu")]
    pub fn set_shared_wgpu_context(
        &mut self,
        adapter: wgpu::Adapter,
        device: wgpu::Device,
        queue: wgpu::Queue,
    ) {
        self.shared_wgpu_context = Some(SharedWgpuContext {
            adapter,
            device,
            queue,
        });
    }

    /// Gets the render delegate.
    pub fn render_delegate(&self) -> Arc<RwLock<HdStRenderDelegate>> {
        self.render_delegate.clone()
    }

    /// Convert complexity float [1.0, 2.0] to subdivision refine level int [0, 8].
    ///
    /// Port of C++ `_GetRefineLevel()` from usdImagingGL/engine.cpp.
    /// Each 0.1 step above 1.0 adds one refinement level.
    pub(crate) fn get_refine_level(complexity: f32) -> i32 {
        // Same control flow as C++ `_GetRefineLevel` (engine.cpp): float compares,
        // not half-open `match` ranges (avoids 1.09+0.01 landing in the wrong bin).
        let c = (complexity + 0.01).min(2.0);
        if (1.0..1.1).contains(&c) {
            0
        } else if (1.1..1.2).contains(&c) {
            1
        } else if (1.2..1.3).contains(&c) {
            2
        } else if (1.3..1.4).contains(&c) {
            3
        } else if (1.4..1.5).contains(&c) {
            4
        } else if (1.5..1.6).contains(&c) {
            5
        } else if (1.6..1.7).contains(&c) {
            6
        } else if (1.7..1.8).contains(&c) {
            7
        } else if (1.8..=2.0).contains(&c) {
            8
        } else {
            log::warn!(
                "[engine] Invalid complexity {complexity}, expected [1.0,2.0]; using refine level 0"
            );
            0
        }
    }

    /// C++ `UsdImagingGLEngine::_PreSetTime` + `SetTime`/`SetSceneGlobals` fragment used from
    /// `PrepareBatch` after the scene index exists: refine fallback, batched
    /// `ApplyPendingUpdates`, then [`Self::set_time`] with `params.frame`.
    fn run_prepare_batch_scene_time_and_refine(&mut self, params: &RenderParams) {
        let refine_level = Self::get_refine_level(params.complexity);
        if refine_level != self.last_refine_level {
            log::info!(
                "[engine] refine level changed: {} -> {} (complexity={})",
                self.last_refine_level,
                refine_level,
                params.complexity
            );
            self.last_refine_level = refine_level;
            self.mesh_sync_dirty = true;
            if let Some(index) = &self.render_index {
                index
                    .lock()
                    .expect("Mutex poisoned")
                    .mark_all_rprims_dirty(HdRprimDirtyBits::ALL_DIRTY);
            }
        }

        if let Some(scene_indices) = self.scene_indices.as_ref() {
            usd_hd::scene_index::HdNoticeBatchingSceneIndex::set_batching_enabled_unlocked(
                &scene_indices.notice_batching_typed,
                true,
            );
            scene_indices.stage_scene_index.apply_pending_updates();
            usd_hd::scene_index::HdNoticeBatchingSceneIndex::set_batching_enabled_unlocked(
                &scene_indices.notice_batching_typed,
                false,
            );
        }

        // C++ `PrepareBatch` always calls `StageSceneIndex::SetTime(params.frame)` even when the
        // time is unchanged. Our `set_time` early-returns when `self.time_code == time`, which
        // breaks the first frame: viewport calls `set_time` before `scene_indices` exist, so the
        // stage never receives the time; then `prepare_batch` calls `set_time` with the same
        // frame and returns early — `StageSceneIndex::set_time` never runs → wrong xforms until
        // the user scrubs to a *different* time (TODO8 flower paths, `flo.usdz` hierarchy).
        let frame = params.frame;
        if let Some(scene_indices) = self.scene_indices.as_ref() {
            usd_hd::scene_index::HdNoticeBatchingSceneIndex::flush_unlocked(
                &scene_indices.notice_batching_typed,
            );
            scene_indices.stage_scene_index.set_time(frame, false);
            usd_hd::scene_index::HdNoticeBatchingSceneIndex::flush_unlocked(
                &scene_indices.notice_batching_typed,
            );
        }
        if self.time_code != frame {
            self.set_time(frame);
        } else if frame.is_numeric() {
            // Engine time already matches `params.frame`; keep globals aligned (avoid
            // `TimeCode::value()` on Default()).
            self.scene_globals_current_frame = frame.value();
        }
    }

    // -------------------------------------------------------------------------
    // Rendering
    // -------------------------------------------------------------------------

    /// `UsdImagingGLEngine::UseUsdImagingSceneIndex` — this port always uses the
    /// stage scene index path (Hydra 2.0-style input).
    pub fn use_usd_imaging_scene_index() -> bool {
        true
    }

    /// C++ `UsdImagingGLEngine::PrepareBatch`: time/refine/pending updates, optional populate.
    ///
    /// Call **every frame** before `render_batch` when mirroring `UsdImagingGLEngine::Render`
    /// (`PrepareBatch` then `RenderBatch`). `Engine::render` does this automatically.
    ///
    /// # Arguments
    ///
    /// * `root` - Root prim to prepare
    /// * `params` - Rendering parameters (`frame` matches C++ `UsdImagingGLRenderParams::frame`)
    pub fn prepare_batch(&mut self, root: &Prim, params: &RenderParams) {
        usd_trace::trace_scope!("Engine::prepare_batch");
        let _t0 = std::time::Instant::now();
        let transitioning_to_prepared = !self.prepared;
        let diag_prepare = std::env::var_os("USD_PROFILE_PREPARE").is_some();
        let diag = |msg: &str| {
            if diag_prepare {
                eprintln!("[prepare_batch] {msg}");
            }
        };
        // Create render pass if not exists
        let collection = HdRprimCollection::new(Token::new("renderPass"));
        if self.render_pass.is_none() {
            diag("create render pass");
            self.render_pass = Some(HdStRenderPass::new(collection.clone()));
        }

        // Create render index and populate when we have a stage.
        if let Some(stage) = root.stage() {
            if self.render_index.is_none() {
                diag("render_index is none; creating scene indices");
                // Reuse existing wgpu device on scene switch; only create
                // a new one on first init or after invalidate_device().
                let hgi_arc = if let Some(ref existing) = self.wgpu_hgi {
                    log::info!("[engine] Reusing existing HgiWgpu device for new scene");
                    Some(existing.clone())
                } else {
                    log::info!("[engine] Creating HgiWgpu backend");
                    let wgpu_t0 = std::time::Instant::now();
                    #[cfg(feature = "wgpu")]
                    let new_hgi = self.shared_wgpu_context.as_ref().map(|ctx| {
                        HgiWgpu::from_existing(
                            ctx.adapter.clone(),
                            ctx.device.clone(),
                            ctx.queue.clone(),
                        )
                    });
                    #[cfg(not(feature = "wgpu"))]
                    let new_hgi: Option<HgiWgpu> = None;
                    if let Some(hgi) = new_hgi.or_else(HgiWgpu::new) {
                        log::trace!("[PERF] HgiWgpu init: {:?}", wgpu_t0.elapsed());
                        let arc = Arc::new(RwLock::new(hgi));
                        self.wgpu_hgi = Some(arc.clone());
                        Some(arc)
                    } else {
                        log::warn!("[engine] HgiWgpu initialization failed");
                        None
                    }
                };

                if let Some(hgi_arc) = hgi_arc {
                    diag("hgi ready");
                    // Pass wgpu device + queue to render_pass_state for GPU frustum culling.
                    {
                        let hgi_read = hgi_arc.read();
                        self.render_pass_state.set_wgpu_device_queue(
                            Arc::clone(hgi_read.device()),
                            Arc::clone(hgi_read.queue()),
                        );
                    }

                    diag("create drivers");
                    let drivers = HdStRenderDelegate::create_drivers(hgi_arc.clone());
                    diag("create scene indices");
                    let scene_indices = create_scene_indices(UsdImagingCreateSceneIndicesInfo {
                        stage: Some(stage.clone()),
                        add_draw_mode_scene_index: self.params.enable_usd_draw_modes,
                        display_unloaded_prims_with_bounds: self
                            .params
                            .display_unloaded_prims_with_bounds,
                        ..Default::default()
                    });
                    diag("scene indices created");

                    let pop_t0 = std::time::Instant::now();
                    diag("create render index");
                    let index =
                        HdRenderIndex::new(self.render_delegate.clone(), drivers, None, None)
                            .expect("render index");
                    log::trace!(
                        "[PERF] create_scene_indices/render_index: {:?}",
                        pop_t0.elapsed()
                    );
                    let index = Arc::new(Mutex::new(index));
                    {
                        diag("set terminal scene index");
                        let mut guard = index.lock().expect("render index lock poisoned");
                        // Wire the terminal scene index only after the render index is in its
                        // final Arc allocation so the adapter's raw back-pointer remains valid.
                        guard.set_scene_index_emulation_enabled(false);
                        guard.set_terminal_scene_index(scene_indices.final_scene_index.clone());
                    }
                    diag("store render index");
                    self.render_index = Some(index);

                    self.scene_indices = Some(scene_indices);
                    diag("store scene indices done");
                    diag("PrepareBatch time/refine (_PreSetTime + SetTime)");
                    self.run_prepare_batch_scene_time_and_refine(params);
                }
            } else if self.scene_indices.is_some() {
                diag("reusing scene indices; PrepareBatch time/refine");
                self.run_prepare_batch_scene_time_and_refine(params);
            }
        }

        self.prepared = true;
        self.converged = false;
        if transitioning_to_prepared {
            // First `prepare_batch` after `invalidate` / initial — full mesh sync once.
            self.mesh_sync_dirty = true;
            self.last_storm_item_paths.clear();
        }
        log::trace!("[PERF] prepare_batch: {:?}", _t0.elapsed());
    }

    /// Read UsdGeomCamera properties from stage and update view/projection matrices.
    ///
    /// Per C++ engine.cpp `SetCameraPath` + task controller: when a camera_path is set,
    /// the engine reads `UsdGeomCamera::ComputeViewMatrix` and `ComputeProjectionMatrix`
    /// instead of using the manually set view/projection from `set_camera_state`.
    ///
    /// Called each frame before `update_render_pass_state` so the matrices are fresh
    /// for animated cameras.
    #[allow(dead_code)] // Called from pub render_batch(), used by usd-view
    fn sync_camera_from_stage(&mut self) {
        use usd_geom::Camera as UsdGeomCamera;
        use usd_sdf::TimeCode as SdfTimeCode;

        let camera_path = match &self.camera_path {
            Some(p) => p.clone(),
            None => return,
        };
        let stage = match self
            .scene_indices
            .as_ref()
            .and_then(|indices| indices.stage_scene_index.get_stage())
        {
            Some(s) => s,
            None => return,
        };
        let prim = match stage.get_prim_at_path(&camera_path) {
            Some(p) if p.is_valid() => p,
            _ => {
                log::warn!(
                    "[engine] sync_camera: camera prim not found at {}",
                    camera_path
                );
                return;
            }
        };
        let usd_cam = UsdGeomCamera::new(prim);
        let sdf_time = if self.time_code.is_default() {
            SdfTimeCode::new(0.0)
        } else {
            SdfTimeCode::new(self.time_code.value())
        };

        // ComputeViewMatrix = inverse of camera's world-space transform
        if let Some(view) = usd_cam.compute_view_matrix(sdf_time) {
            self.view_matrix = view;
            log::debug!(
                "[engine] sync_camera: view matrix from UsdGeomCamera '{}'",
                camera_path
            );
        }
        // ComputeProjectionMatrix = frustum projection from focal length + aperture + clip
        if let Some(proj) = usd_cam.compute_projection_matrix(sdf_time) {
            self.projection_matrix = proj;
            log::debug!(
                "[engine] sync_camera: projection matrix from UsdGeomCamera '{}'",
                camera_path
            );
        }
    }

    fn task_controller_id(&self) -> Path {
        self.params
            .scene_delegate_id
            .append_child("taskController")
            .or_else(|| Path::absolute_root().append_child("taskController"))
            .unwrap_or_else(Path::absolute_root)
    }

    fn aov_render_buffer_path(&self, aov_name: &Token) -> Path {
        let safe = aov_name.as_str().replace([':', '.'], "_");
        self.task_controller_id()
            .append_child(&format!("aov_{}", safe))
            .unwrap_or_default()
    }

    fn to_hdx_window_policy(
        policy: CameraUtilConformWindowPolicy,
    ) -> HdxCameraUtilConformWindowPolicy {
        match policy {
            CameraUtilConformWindowPolicy::MatchVertically => {
                HdxCameraUtilConformWindowPolicy::MatchVertically
            }
            CameraUtilConformWindowPolicy::MatchHorizontally => {
                HdxCameraUtilConformWindowPolicy::MatchHorizontally
            }
            CameraUtilConformWindowPolicy::Fit => HdxCameraUtilConformWindowPolicy::Fit,
            CameraUtilConformWindowPolicy::Crop => HdxCameraUtilConformWindowPolicy::CropToFill,
            CameraUtilConformWindowPolicy::DontConform => {
                HdxCameraUtilConformWindowPolicy::DontConform
            }
        }
    }

    fn to_hdx_framing(framing: &CameraUtilFraming) -> HdxCameraUtilFraming {
        HdxCameraUtilFraming {
            display_window: (
                framing.display_window.min().x as f64,
                framing.display_window.min().y as f64,
                framing.display_window.max().x as f64,
                framing.display_window.max().y as f64,
            ),
            data_window: (
                framing.data_window.min_x(),
                framing.data_window.min_y(),
                framing.data_window.max_x(),
                framing.data_window.max_y(),
            ),
            pixel_aspect_ratio: framing.pixel_aspect_ratio as f64,
        }
    }

    fn to_hdx_cull_style(cull_style: CullStyle) -> HdCullStyle {
        match cull_style {
            CullStyle::NoOpinion => HdCullStyle::DontCare,
            CullStyle::Nothing => HdCullStyle::Nothing,
            CullStyle::Back => HdCullStyle::Back,
            CullStyle::Front => HdCullStyle::Front,
            CullStyle::BackUnlessDoubleSided => HdCullStyle::BackUnlessDoubleSided,
        }
    }

    fn from_hdx_compare_function(compare: HdCompareFunction) -> DepthFunc {
        match compare {
            HdCompareFunction::Never => DepthFunc::Never,
            HdCompareFunction::Less => DepthFunc::Less,
            HdCompareFunction::Equal => DepthFunc::Equal,
            HdCompareFunction::LEqual => DepthFunc::LessEqual,
            HdCompareFunction::Greater => DepthFunc::Greater,
            HdCompareFunction::NotEqual => DepthFunc::NotEqual,
            HdCompareFunction::GEqual => DepthFunc::GreaterEqual,
            HdCompareFunction::Always => DepthFunc::Always,
        }
    }

    fn from_hdx_blend_op(op: HdBlendOp) -> HgiBlendOp {
        match op {
            HdBlendOp::Add => HgiBlendOp::Add,
            HdBlendOp::Subtract => HgiBlendOp::Subtract,
            HdBlendOp::ReverseSubtract => HgiBlendOp::ReverseSubtract,
            HdBlendOp::Min => HgiBlendOp::Min,
            HdBlendOp::Max => HgiBlendOp::Max,
        }
    }

    fn from_hdx_blend_factor(factor: HdBlendFactor) -> HgiBlendFactor {
        match factor {
            HdBlendFactor::Zero => HgiBlendFactor::Zero,
            HdBlendFactor::One => HgiBlendFactor::One,
            HdBlendFactor::SrcColor => HgiBlendFactor::SrcColor,
            HdBlendFactor::OneMinusSrcColor => HgiBlendFactor::OneMinusSrcColor,
            HdBlendFactor::DstColor => HgiBlendFactor::DstColor,
            HdBlendFactor::OneMinusDstColor => HgiBlendFactor::OneMinusDstColor,
            HdBlendFactor::SrcAlpha => HgiBlendFactor::SrcAlpha,
            HdBlendFactor::OneMinusSrcAlpha => HgiBlendFactor::OneMinusSrcAlpha,
            HdBlendFactor::DstAlpha => HgiBlendFactor::DstAlpha,
            HdBlendFactor::OneMinusDstAlpha => HgiBlendFactor::OneMinusDstAlpha,
            HdBlendFactor::ConstantColor => HgiBlendFactor::ConstantColor,
            HdBlendFactor::OneMinusConstantColor => HgiBlendFactor::OneMinusConstantColor,
            HdBlendFactor::ConstantAlpha => HgiBlendFactor::ConstantAlpha,
            HdBlendFactor::OneMinusConstantAlpha => HgiBlendFactor::OneMinusConstantAlpha,
            HdBlendFactor::SrcAlphaSaturate => HgiBlendFactor::SrcAlphaSaturate,
            HdBlendFactor::Src1Color => HgiBlendFactor::Src1Color,
            HdBlendFactor::OneMinusSrc1Color => HgiBlendFactor::OneMinusSrc1Color,
            HdBlendFactor::Src1Alpha => HgiBlendFactor::Src1Alpha,
            HdBlendFactor::OneMinusSrc1Alpha => HgiBlendFactor::OneMinusSrc1Alpha,
        }
    }

    fn apply_hdx_cull_style(render_pass_state: &mut HdStRenderPassState, style: HdCullStyle) {
        match style {
            HdCullStyle::DontCare => {}
            HdCullStyle::Nothing => {
                render_pass_state.set_cull_enabled(false);
                render_pass_state.set_cull_mode(HgiCullMode::None);
            }
            HdCullStyle::Back | HdCullStyle::BackUnlessDoubleSided => {
                render_pass_state.set_cull_enabled(true);
                render_pass_state.set_cull_mode(HgiCullMode::Back);
            }
            HdCullStyle::Front | HdCullStyle::FrontUnlessDoubleSided => {
                render_pass_state.set_cull_enabled(true);
                render_pass_state.set_cull_mode(HgiCullMode::Front);
            }
        }
    }

    fn build_hdx_render_task_params(&self, params: &RenderParams) -> HdxRenderTaskParams {
        let mut task_params = HdxRenderTaskParams::default();
        task_params.override_color = params.override_color;
        task_params.wireframe_color = params.wireframe_color;
        task_params.enable_lighting = params.enable_lighting;
        task_params.alpha_threshold = params.alpha_threshold.max(0.0);
        task_params.enable_scene_lights = params.enable_scene_lights;
        task_params.enable_clipping = !params.clip_planes.is_empty();
        task_params.viewport = Vec4d::new(
            0.0,
            0.0,
            self.render_buffer_size.x as f64,
            self.render_buffer_size.y as f64,
        );
        task_params.cull_style = Self::to_hdx_cull_style(params.cull_style);
        task_params
    }

    fn sync_task_controller_state(&mut self, params: &RenderParams) {
        if self.task_controller.is_none() {
            let mut controller = HdxTaskController::new(self.task_controller_id(), self.gpu_enabled);
            controller.set_is_storm_backend(true);
            controller.set_enable_presentation(self.enable_presentation);

            // Register all HDX tasks in render index (C++ does this in each _Create*Task).
            if let Some(index_arc) = self.render_index.clone() {
                if let Ok(mut index_guard) = index_arc.lock() {
                    for (path, task) in controller.get_all_tasks() {
                        index_guard.insert_task(None, &path, task);
                    }
                    log::info!("[engine] registered {} HDX tasks in render index", index_guard.get_task_count());
                }
            }

            self.task_controller = Some(controller);
        }

        let render_buffer_size = self.render_buffer_size.clone();
        let current_aov = self.current_aov.clone();
        let framing = Self::to_hdx_framing(&self.framing);
        let override_window_policy =
            self.override_window_policy.map(Self::to_hdx_window_policy);
        let camera_path = self.camera_path.clone();
        let view_matrix = self.view_matrix.clone();
        let projection_matrix = self.projection_matrix.clone();
        let clip_planes = params.clip_planes.clone();
        let selected_paths = self.selected_paths.clone();
        let selection_color = self.selection_color.clone();
        let render_tags = params.render_tags.clone();
        let render_task_params = self.build_hdx_render_task_params(params);
        let shadows_enabled = self.render_pass_state.has_shadows();
        let render_collection_name = self
            .render_pass
            .as_ref()
            .map(|render_pass| render_pass.get_rprim_collection().name.clone());

        let Some(controller) = self.task_controller.as_mut() else {
            return;
        };

        controller.set_enable_presentation(self.enable_presentation);
        if let Some(collection_name) = render_collection_name {
            controller.set_collection(&collection_name);
        }
        controller.set_render_buffer_size(render_buffer_size);
        controller.set_render_outputs(std::slice::from_ref(&current_aov));
        controller.set_viewport_render_output(current_aov.clone());
        controller.set_framing(framing);
        controller.set_override_window_policy(override_window_policy);

        if let Some(camera_path) = camera_path {
            controller.clear_free_camera();
            controller.set_camera_path(camera_path);
        } else {
            controller.clear_free_camera();
            controller.set_free_camera_matrices(view_matrix, projection_matrix);
            controller.set_free_camera_clip_planes(clip_planes);
        }

        let selection_tracker = controller.get_selection_tracker();
        selection_tracker.set_selection(&selected_paths);
        selection_tracker.set_locate_selection(&self.located_paths);
        controller.set_enable_selection(params.highlight);
        controller.set_selection_color(selection_color);
        controller.set_render_tags(&render_tags);
        controller.set_render_params(&render_task_params);

        let mut color_params = HdxColorCorrectionTaskParams::default();
        color_params.aov_name = current_aov;
        let effective_color_mode = if params.color_correction_mode.is_empty() {
            self.color_correction_mode.as_str()
        } else {
            params.color_correction_mode.as_str()
        };
        color_params.color_correction_mode = if effective_color_mode
            == color_correction_tokens::opencolorio().as_str()
        {
            color_correction_tokens::opencolorio()
        } else if effective_color_mode == color_correction_tokens::srgb().as_str() {
            color_correction_tokens::srgb()
        } else {
            color_correction_tokens::disabled()
        };
        color_params.display_ocio = if params.ocio_display.is_empty() {
            self.ocio_settings.display.clone()
        } else {
            params.ocio_display.as_str().to_owned()
        };
        color_params.view_ocio = if params.ocio_view.is_empty() {
            self.ocio_settings.view.clone()
        } else {
            params.ocio_view.as_str().to_owned()
        };
        color_params.colorspace_ocio = if params.ocio_color_space.is_empty() {
            self.ocio_settings.color_space.clone()
        } else {
            params.ocio_color_space.as_str().to_owned()
        };
        color_params.looks_ocio = if params.ocio_look.is_empty() {
            self.ocio_settings.looks.clone()
        } else {
            params.ocio_look.as_str().to_owned()
        };
        color_params.lut3d_size_ocio = if params.lut3d_size_ocio > 0 {
            params.lut3d_size_ocio
        } else {
            self.ocio_settings.lut3d_size.max(1)
        };
        controller.set_color_correction_params(&color_params);

        let mut shadow_params = HdxShadowTaskParams::default();
        shadow_params.cull_style = Self::to_hdx_cull_style(params.cull_style);
        controller.set_enable_shadows(shadows_enabled);
        controller.set_shadow_params(&shadow_params);

        let mut bbox_params = HdxBoundingBoxTaskParams::default();
        bbox_params.color = params.bbox_line_color;
        bbox_params.dash_size = params.bbox_line_dash_size;
        controller.set_bbox_params(&bbox_params);
    }

    fn sync_hdx_bridge_context(&mut self) {
        for token in [
            "renderTaskRequested",
            "renderTaskRequests",
            "pickTaskRequested",
            "pickTaskRequests",
            "aovInputTaskRequests",
            "colorizeSelectionTaskRequests",
            "colorCorrectionTaskRequests",
            "visualizeAovTaskRequests",
            "presentTaskRequests",
            "postTaskOrder",
            "shadowPassRequested",
            "skydomeRenderRequested",
            "skydomeTaskExecuted",
            "skydomeNeedsClear",
            "hgiFrameStarted",
            "selectionState",
            "selectionBuffer",
            "selectionOffsets",
            "selectionUniforms",
            "hasSelection",
            // Note: "color", "depth", "colorIntermediate", "depthIntermediate"
            // are NOT cleared here — they represent persistent AOV texture handles
            // that downstream tasks (skydome, colorize) check for presence.
        ] {
            self.hd_engine.remove_task_context_data(&Token::new(token));
        }
        for aov_name in [
            self.current_aov.as_str().to_owned(),
            "color".to_owned(),
            "depth".to_owned(),
            "primId".to_owned(),
            "instanceId".to_owned(),
            "elementId".to_owned(),
        ] {
            self.hd_engine
                .remove_task_context_data(&Token::new(&format!("aov_{}", aov_name)));
        }
        for aov_name in self.wgpu_aux_aov_textures.keys().cloned().collect::<Vec<_>>() {
            self.hd_engine
                .remove_task_context_data(&Token::new(&format!("aov_{}", aov_name)));
        }

        let shadow_count = self.render_pass_state.get_shadow_entries().len();
        self.hd_engine
            .set_task_context_data(Token::new("shadows"), Value::from(shadow_count));

        let inv_proj = self
            .projection_matrix
            .inverse()
            .unwrap_or_else(Matrix4d::identity);
        let view_to_world = self.view_matrix.inverse().unwrap_or_else(Matrix4d::identity);
        self.hd_engine
            .set_task_context_data(Token::new("invProjMatrix"), Value::from_no_hash(inv_proj));
        self.hd_engine.set_task_context_data(
            Token::new("viewToWorldMatrix"),
            Value::from_no_hash(view_to_world),
        );
        // Publish AOV presence keys so downstream tasks (skydome, colorize, etc.)
        // can check whether targets are available. Always publish "color" — the
        // actual texture is created later in ensure_wgpu_render_targets(), but tasks
        // just check for key presence during execute().
        self.hd_engine
            .set_task_context_data(Token::new("color"), Value::from(true));

        // Publish lighting context marker so skydome task can find dome light.
        if !self.render_pass_state.get_scene_lights().is_empty() {
            self.hd_engine
                .set_task_context_data(Token::new("lightingContext"), Value::from(true));
        }

        if self.dome_light_enabled {
            self.hd_engine
                .set_task_context_data(Token::new("domeLightTransformInv"), Value::from_no_hash(self.get_dome_light_inv_transform()));
            self.hd_engine
                .set_task_context_data(Token::new("skydomeTexture"), Value::from(true));
        } else {
            self.hd_engine
                .remove_task_context_data(&Token::new("domeLightTransformInv"));
            self.hd_engine
                .remove_task_context_data(&Token::new("skydomeTexture"));
        }

    }

    fn aux_aov_format(aov_name: &str) -> HgiFormat {
        match aov_name {
            "normal" | "Neye" => HgiFormat::UNorm8Vec4,
            "edgeId" | "pointId" => HgiFormat::UNorm8Vec4,
            _ => HgiFormat::Float16Vec4,
        }
    }

    fn ensure_aux_aov_texture(&mut self, aov_name: &str) -> Option<(HgiTextureHandle, HgiFormat)> {
        if let Some(texture) = self.wgpu_aux_aov_textures.get(aov_name).cloned() {
            return Some((texture, Self::aux_aov_format(aov_name)));
        }
        let Some(hgi_arc) = self.wgpu_hgi.clone() else {
            return None;
        };
        let w = self.render_buffer_size.x.max(1);
        let h = self.render_buffer_size.y.max(1);
        let dims = usd_gf::Vec3i::new(w, h, 1);
        let format = Self::aux_aov_format(aov_name);
        let mut hgi = hgi_arc.write();
        let desc = HgiTextureDesc::new()
            .with_debug_name(&format!("engine_{}_rt", aov_name))
            .with_format(format)
            .with_usage(HgiTextureUsage::COLOR_TARGET | HgiTextureUsage::SHADER_READ)
            .with_dimensions(dims);
        let texture = hgi.create_texture(&desc, None);
        self.wgpu_aux_aov_textures
            .insert(aov_name.to_string(), texture.clone());
        Some((texture, format))
    }

    fn engine_aov_texture(&self, aov_name: &str) -> Option<HgiTextureHandle> {
        match aov_name {
            "color" => self.wgpu_color_texture.clone(),
            "depth" => self.wgpu_depth_texture.clone(),
            "primId" => self.wgpu_prim_id_texture.clone(),
            "instanceId" => self.wgpu_instance_id_texture.clone(),
            "elementId" => self.wgpu_element_id_texture.clone(),
            "normal" => self
                .wgpu_aux_aov_textures
                .get("Neye")
                .cloned()
                .or_else(|| self.wgpu_aux_aov_textures.get(aov_name).cloned()),
            _ => self.wgpu_aux_aov_textures.get(aov_name).cloned(),
        }
    }

    fn aov_clear_value(binding: &usd_hdx::render_setup_task::HdRenderPassAovBinding, format: HgiFormat) -> Vec4f {
        match format {
            HgiFormat::Float32 => Vec4f::new(
                binding.clear_value.get::<f32>().copied().unwrap_or(1.0),
                0.0,
                0.0,
                0.0,
            ),
            HgiFormat::UNorm8Vec4 => {
                if let Some(value) = binding.clear_value.get::<Vec4f>() {
                    *value
                } else if binding.clear_value.get::<i32>().copied() == Some(-1) {
                    Vec4f::new(1.0, 1.0, 1.0, 1.0)
                } else {
                    Vec4f::new(0.0, 0.0, 0.0, 0.0)
                }
            }
            _ => binding
                .clear_value
                .get::<Vec4f>()
                .copied()
                .unwrap_or_else(|| Vec4f::new(0.0, 0.0, 0.0, 0.0)),
        }
    }

    fn storm_aov_binding_for_request(
        &mut self,
        binding: &usd_hdx::render_setup_task::HdRenderPassAovBinding,
    ) -> Option<HdStAovBinding> {
        let clear_on_load = !binding.clear_value.is_empty();
        let (texture, format) = match binding.aov_name.as_str() {
            "color" => (self.wgpu_color_texture.clone()?, HgiFormat::Float16Vec4),
            "depth" => (self.wgpu_depth_texture.clone()?, HgiFormat::Float32),
            "primId" => (self.wgpu_prim_id_texture.clone()?, HgiFormat::UNorm8Vec4),
            "instanceId" => (self.wgpu_instance_id_texture.clone()?, HgiFormat::UNorm8Vec4),
            "elementId" => (self.wgpu_element_id_texture.clone()?, HgiFormat::UNorm8Vec4),
            _ => self.ensure_aux_aov_texture(binding.aov_name.as_str())?,
        };
        Some(HdStAovBinding {
            aov_name: binding.aov_name.clone(),
            texture,
            format,
            clear_value: Self::aov_clear_value(binding, format),
            clear_on_load,
        })
    }

    fn publish_engine_aov_aliases(&mut self, active_aov_name: &Token, include_depth: bool) {
        if active_aov_name.as_str() == "color" {
            if let Some(color_texture) = self.wgpu_color_texture.clone() {
                self.hd_engine.set_task_context_data(
                    Token::new("aov_color"),
                    Value::new(color_texture),
                );
            }
        } else {
            self.hd_engine
                .remove_task_context_data(&Token::new("aov_color"));
        }
        if include_depth {
            if let Some(depth_texture) = self.wgpu_depth_texture.clone() {
                self.hd_engine.set_task_context_data(
                    Token::new("aov_depth"),
                    Value::new(depth_texture.clone()),
                );
                self.hd_engine.set_task_context_data(
                    Token::new("depth"),
                    Value::new(depth_texture.clone()),
                );
                self.hd_engine.set_task_context_data(
                    Token::new("depthIntermediate"),
                    Value::new(depth_texture),
                );
            }
        } else {
            self.hd_engine.remove_task_context_data(&Token::new("depth"));
            self.hd_engine
                .remove_task_context_data(&Token::new("aov_depth"));
            self.hd_engine
                .remove_task_context_data(&Token::new("depthIntermediate"));
        }
        if let Some(prim_id_texture) = self.wgpu_prim_id_texture.clone() {
            self.hd_engine.set_task_context_data(
                Token::new("aov_primId"),
                Value::new(prim_id_texture),
            );
        } else {
            self.hd_engine
                .remove_task_context_data(&Token::new("aov_primId"));
        }
        if let Some(instance_id_texture) = self.wgpu_instance_id_texture.clone() {
            self.hd_engine.set_task_context_data(
                Token::new("aov_instanceId"),
                Value::new(instance_id_texture),
            );
        } else {
            self.hd_engine
                .remove_task_context_data(&Token::new("aov_instanceId"));
        }
        if let Some(element_id_texture) = self.wgpu_element_id_texture.clone() {
            self.hd_engine.set_task_context_data(
                Token::new("aov_elementId"),
                Value::new(element_id_texture),
            );
        } else {
            self.hd_engine
                .remove_task_context_data(&Token::new("aov_elementId"));
        }
        for name in ["edgeId", "pointId", "Neye"] {
            if let Some(texture) = self.engine_aov_texture(name) {
                self.hd_engine
                    .set_task_context_data(Token::new(&format!("aov_{}", name)), Value::new(texture));
            } else {
                self.hd_engine
                    .remove_task_context_data(&Token::new(&format!("aov_{}", name)));
            }
        }

        if !matches!(
            active_aov_name.as_str(),
            "color" | "depth" | "primId" | "instanceId" | "elementId" | "edgeId" | "pointId" | "Neye"
        ) {
            if let Some(active_texture) = self.engine_aov_texture(active_aov_name.as_str()) {
                self.hd_engine.set_task_context_data(
                    Token::new(&format!("aov_{}", active_aov_name.as_str())),
                    Value::new(active_texture),
                );
            } else {
                self.hd_engine.remove_task_context_data(&Token::new(&format!(
                    "aov_{}",
                    active_aov_name.as_str()
                )));
            }
        }
    }

    fn publish_active_aov_source_to_task_context(&mut self, active_aov_name: &Token) {
        let Some(active_texture) = self.engine_aov_texture(active_aov_name.as_str()) else {
            self.hd_engine.remove_task_context_data(&Token::new("color"));
            self.hd_engine
                .remove_task_context_data(&Token::new("colorIntermediate"));
            self.hd_engine.remove_task_context_data(&Token::new(&format!(
                "aov_{}",
                active_aov_name.as_str()
            )));
            return;
        };
        self.hd_engine
            .set_task_context_data(Token::new("color"), Value::new(active_texture.clone()));
        let intermediate = self
            .wgpu_post_color_texture
            .clone()
            .unwrap_or_else(|| active_texture.clone());
        self.hd_engine.set_task_context_data(
            Token::new("colorIntermediate"),
            Value::new(intermediate),
        );
        self.hd_engine.set_task_context_data(
            Token::new(&format!("aov_{}", active_aov_name.as_str())),
            Value::new(active_texture),
        );
    }

    fn publish_post_fx_display_output_to_task_context(&mut self, active_aov_name: &Token) {
        self.publish_engine_aov_aliases(active_aov_name, self.wgpu_depth_texture.is_some());
        let Some(display_texture) = self.wgpu_color_texture.clone() else {
            return;
        };
        self.hd_engine
            .set_task_context_data(Token::new("color"), Value::new(display_texture.clone()));
        let intermediate = self
            .wgpu_post_color_texture
            .clone()
            .unwrap_or_else(|| display_texture.clone());
        self.hd_engine.set_task_context_data(
            Token::new("colorIntermediate"),
            Value::new(intermediate),
        );
        if active_aov_name.as_str() == "color" {
            self.hd_engine.set_task_context_data(
                Token::new("aov_color"),
                Value::new(display_texture),
            );
        }
    }

    fn publish_engine_aovs_to_task_context(&mut self, aov_name: &Token, include_depth: bool) {
        self.publish_engine_aov_aliases(aov_name, include_depth);
        self.publish_active_aov_source_to_task_context(aov_name);
    }

    fn swap_post_fx_color_targets(&mut self, aov_name: &Token) {
        std::mem::swap(
            &mut self.wgpu_color_texture,
            &mut self.wgpu_post_color_texture,
        );
        self.publish_post_fx_display_output_to_task_context(aov_name);
    }

    fn ensure_srgb_post_pipeline(&mut self, device: &wgpu::Device) {
        if self.srgb_post_pipeline.is_some()
            && self.srgb_post_bind_group_layout.is_some()
            && self.srgb_post_sampler.is_some()
        {
            return;
        }

        let bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("engine_srgb_post_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::WriteOnly,
                            format: wgpu::TextureFormat::Rgba16Float,
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("engine_srgb_post_shader"),
            source: wgpu::ShaderSource::Wgsl(ENGINE_SRGB_POST_SHADER.into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("engine_srgb_post_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("engine_srgb_post_pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("engine_srgb_post_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        self.srgb_post_bind_group_layout = Some(bind_group_layout);
        self.srgb_post_pipeline = Some(pipeline);
        self.srgb_post_sampler = Some(sampler);
    }

    fn apply_srgb_color_correction(&mut self, aov_name: &Token) -> bool {
        let Some(hgi_arc) = self.wgpu_hgi.clone() else {
            return false;
        };
        let Some(color_texture) = self.wgpu_color_texture.clone() else {
            return false;
        };
        let Some(post_color_texture) = self.wgpu_post_color_texture.clone() else {
            return false;
        };

        let width = self.render_buffer_size.x.max(1) as u32;
        let height = self.render_buffer_size.y.max(1) as u32;
        let hgi = hgi_arc.write();
        let device = hgi.device();
        let queue = hgi.queue();
        let Some(input_view) = usd_hgi_wgpu::resolve_texture_view(&color_texture) else {
            return false;
        };
        let Some(output_view) = usd_hgi_wgpu::resolve_texture_view(&post_color_texture) else {
            return false;
        };
        self.ensure_srgb_post_pipeline(device);
        let (Some(bind_group_layout), Some(pipeline), Some(sampler)) = (
            self.srgb_post_bind_group_layout.as_ref(),
            self.srgb_post_pipeline.as_ref(),
            self.srgb_post_sampler.as_ref(),
        ) else {
            return false;
        };

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("engine_srgb_post_bind_group"),
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(input_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(output_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("engine_srgb_post_encoder"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("engine_srgb_post_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(width.div_ceil(8), height.div_ceil(8), 1);
        }
        queue.submit(Some(encoder.finish()));
        drop(hgi);

        self.swap_post_fx_color_targets(aov_name);
        true
    }

    fn apply_ocio_color_correction(
        &mut self,
        request: &HdxColorCorrectionTaskRequest,
    ) -> bool {
        let Some(hgi_arc) = self.wgpu_hgi.clone() else {
            return false;
        };
        let Some(color_texture) = self.wgpu_color_texture.clone() else {
            return false;
        };
        let Some(post_color_texture) = self.wgpu_post_color_texture.clone() else {
            return false;
        };

        let width = self.render_buffer_size.x.max(1) as u32;
        let height = self.render_buffer_size.y.max(1) as u32;
        let hgi = hgi_arc.write();
        let device = hgi.device();
        let queue = hgi.queue();
        let Some(input_view) = usd_hgi_wgpu::resolve_texture_view(&color_texture) else {
            return false;
        };
        let Some(output_view) = usd_hgi_wgpu::resolve_texture_view(&post_color_texture) else {
            return false;
        };
        let settings = EngineOcioSettings {
            display: request.display_ocio.clone(),
            view: request.view_ocio.clone(),
            color_space: request.colorspace_ocio.clone(),
            looks: request.looks_ocio.clone(),
            lut3d_size: request.lut3d_size_ocio,
        };
        if self
            .ocio_post_pass
            .rebuild_if_needed(device, queue, &settings)
            .is_err()
        {
            return false;
        }
        let applied = self
            .ocio_post_pass
            .execute(device, queue, input_view, output_view, width, height);
        drop(hgi);
        if applied {
            self.swap_post_fx_color_targets(&request.aov_name);
        }
        applied
    }

    fn visualize_mode_code(request: &HdxVisualizeAovTaskRequest) -> u32 {
        match request.mode {
            AovVisMode::Grayscale => 1,
            AovVisMode::FalseColor => 2,
            AovVisMode::ChannelSplit => 3,
            AovVisMode::Depth => 6,
            AovVisMode::Normal => 5,
            AovVisMode::Raw => match request.aov_name.as_str() {
                "depth" => 6,
                "primId" | "instanceId" | "elementId" | "edgeId" | "pointId" => 4,
                "normal" | "Neye" => 5,
                _ => 0,
            },
        }
    }

    fn write_visualize_uniform_bytes(
        mode: u32,
        channel: i32,
        min_depth: f32,
        max_depth: f32,
    ) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        bytes[0..4].copy_from_slice(&mode.to_le_bytes());
        bytes[4..8].copy_from_slice(&channel.to_le_bytes());
        bytes[16..20].copy_from_slice(&min_depth.to_le_bytes());
        bytes[20..24].copy_from_slice(&max_depth.to_le_bytes());
        bytes
    }

    fn write_selection_uniform_bytes(
        selection_color: Vec4f,
        locate_color: Vec4f,
        enable_locate_highlight: bool,
        enable_outline: bool,
        outline_radius: u32,
    ) -> [u8; 48] {
        let mut bytes = [0u8; 48];
        bytes[0..4].copy_from_slice(&(enable_outline as u32).to_le_bytes());
        bytes[4..8].copy_from_slice(&outline_radius.to_le_bytes());
        bytes[8..12].copy_from_slice(&(enable_locate_highlight as u32).to_le_bytes());
        bytes[16..20].copy_from_slice(&selection_color.x.to_le_bytes());
        bytes[20..24].copy_from_slice(&selection_color.y.to_le_bytes());
        bytes[24..28].copy_from_slice(&selection_color.z.to_le_bytes());
        bytes[28..32].copy_from_slice(&selection_color.w.to_le_bytes());
        bytes[32..36].copy_from_slice(&locate_color.x.to_le_bytes());
        bytes[36..40].copy_from_slice(&locate_color.y.to_le_bytes());
        bytes[40..44].copy_from_slice(&locate_color.z.to_le_bytes());
        bytes[44..48].copy_from_slice(&locate_color.w.to_le_bytes());
        bytes
    }

    fn ensure_visualize_aov_post_pipelines(&mut self, device: &wgpu::Device) {
        if self.visualize_aov_color_pipeline.is_some()
            && self.visualize_aov_depth_pipeline.is_some()
            && self.visualize_aov_color_bind_group_layout.is_some()
            && self.visualize_aov_depth_bind_group_layout.is_some()
            && self.visualize_aov_sampler.is_some()
            && self.visualize_aov_uniform_buf.is_some()
        {
            return;
        }

        let color_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("engine_visualize_aov_color_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let depth_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("engine_visualize_aov_depth_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let color_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("engine_visualize_aov_color_shader"),
            source: wgpu::ShaderSource::Wgsl(ENGINE_VISUALIZE_AOV_COLOR_POST_SHADER.into()),
        });
        let depth_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("engine_visualize_aov_depth_shader"),
            source: wgpu::ShaderSource::Wgsl(ENGINE_VISUALIZE_AOV_DEPTH_POST_SHADER.into()),
        });
        let color_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("engine_visualize_aov_color_pipeline_layout"),
            bind_group_layouts: &[&color_layout],
            push_constant_ranges: &[],
        });
        let depth_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("engine_visualize_aov_depth_pipeline_layout"),
            bind_group_layouts: &[&depth_layout],
            push_constant_ranges: &[],
        });
        let color_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("engine_visualize_aov_color_pipeline"),
            layout: Some(&color_pipeline_layout),
            module: &color_shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let depth_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("engine_visualize_aov_depth_pipeline"),
            layout: Some(&depth_pipeline_layout),
            module: &depth_shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("engine_visualize_aov_sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("engine_visualize_aov_uniforms"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        self.visualize_aov_color_bind_group_layout = Some(color_layout);
        self.visualize_aov_depth_bind_group_layout = Some(depth_layout);
        self.visualize_aov_color_pipeline = Some(color_pipeline);
        self.visualize_aov_depth_pipeline = Some(depth_pipeline);
        self.visualize_aov_sampler = Some(sampler);
        self.visualize_aov_uniform_buf = Some(uniform_buf);
    }

    fn ensure_selection_post_pipeline(&mut self, device: &wgpu::Device) {
        if self.selection_post_pipeline.is_some()
            && self.selection_post_bind_group_layout.is_some()
            && self.selection_post_sampler.is_some()
            && self.selection_post_uniform_buf.is_some()
        {
            return;
        }

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("engine_selection_post_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("engine_selection_post_shader"),
            source: wgpu::ShaderSource::Wgsl(ENGINE_SELECTION_POST_SHADER.into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("engine_selection_post_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("engine_selection_post_pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("engine_selection_post_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("engine_selection_post_uniforms"),
            size: 48,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        self.selection_post_bind_group_layout = Some(bind_group_layout);
        self.selection_post_pipeline = Some(pipeline);
        self.selection_post_sampler = Some(sampler);
        self.selection_post_uniform_buf = Some(uniform_buf);
    }

    fn compute_depth_visualization_range(
        &self,
        depth_texture: &HgiTextureHandle,
    ) -> Option<(f32, f32)> {
        let hgi_arc = self.wgpu_hgi.clone()?;
        let mut hgi = hgi_arc.write();
        let (format, raw_pixels) = self.readback_wgpu_texture_raw(&mut *hgi, depth_texture)?;
        if format != HgiFormat::Float32 {
            return Some((0.0, 1.0));
        }

        let mut min_depth = f32::INFINITY;
        let mut max_depth = f32::NEG_INFINITY;
        for chunk in raw_pixels.chunks_exact(4) {
            let value = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            if value.is_finite() {
                min_depth = min_depth.min(value);
                max_depth = max_depth.max(value);
            }
        }
        if !min_depth.is_finite() || !max_depth.is_finite() {
            return Some((0.0, 1.0));
        }
        if (max_depth - min_depth).abs() < f32::EPSILON {
            return Some((min_depth, min_depth + 1.0));
        }
        Some((min_depth, max_depth))
    }

    fn apply_visualize_aov(&mut self, request: &HdxVisualizeAovTaskRequest) -> bool {
        let Some(source_texture) = self.engine_aov_texture(request.aov_name.as_str()) else {
            return false;
        };
        let Some(post_color_texture) = self.wgpu_post_color_texture.clone() else {
            return false;
        };
        let Some(hgi_arc) = self.wgpu_hgi.clone() else {
            return false;
        };

        let visualize_mode = Self::visualize_mode_code(request);
        let use_depth_pipeline = visualize_mode == 6 && request.aov_name == "depth";
        let (min_depth, max_depth) = if use_depth_pipeline {
            self.compute_depth_visualization_range(&source_texture)
                .unwrap_or((0.0, 1.0))
        } else {
            (0.0, 1.0)
        };

        let width = self.render_buffer_size.x.max(1) as u32;
        let height = self.render_buffer_size.y.max(1) as u32;
        let hgi = hgi_arc.write();
        let device = hgi.device();
        let queue = hgi.queue();
        let Some(input_view) = usd_hgi_wgpu::resolve_texture_view(&source_texture) else {
            return false;
        };
        let Some(output_view) = usd_hgi_wgpu::resolve_texture_view(&post_color_texture) else {
            return false;
        };
        self.ensure_visualize_aov_post_pipelines(device);
        let Some(uniform_buf) = self.visualize_aov_uniform_buf.as_ref() else {
            return false;
        };
        queue.write_buffer(
            uniform_buf,
            0,
            &Self::write_visualize_uniform_bytes(
                visualize_mode,
                request.channel,
                min_depth,
                max_depth,
            ),
        );

        let bind_group = if use_depth_pipeline {
            let (Some(layout), Some(pipeline)) = (
                self.visualize_aov_depth_bind_group_layout.as_ref(),
                self.visualize_aov_depth_pipeline.as_ref(),
            ) else {
                return false;
            };
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("engine_visualize_aov_depth_bind_group"),
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(input_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(output_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: uniform_buf.as_entire_binding(),
                    },
                ],
            });
            (bind_group, pipeline.clone())
        } else {
            let (Some(layout), Some(pipeline), Some(sampler)) = (
                self.visualize_aov_color_bind_group_layout.as_ref(),
                self.visualize_aov_color_pipeline.as_ref(),
                self.visualize_aov_sampler.as_ref(),
            ) else {
                return false;
            };
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("engine_visualize_aov_color_bind_group"),
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(input_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(output_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: uniform_buf.as_entire_binding(),
                    },
                ],
            });
            (bind_group, pipeline.clone())
        };

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("engine_visualize_aov_encoder"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("engine_visualize_aov_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&bind_group.1);
            pass.set_bind_group(0, &bind_group.0, &[]);
            pass.dispatch_workgroups(width.div_ceil(8), height.div_ceil(8), 1);
        }
        queue.submit(Some(encoder.finish()));
        drop(hgi);

        self.swap_post_fx_color_targets(&request.aov_name);
        true
    }

    fn apply_selection_colorize(&mut self, request: &HdxColorizeSelectionTaskRequest) -> bool {
        if !request.is_active || !request.has_selection {
            return false;
        }
        let Some(selection_buffer) = self
            .hd_engine
            .get_task_context_data(&Token::new("selectionBuffer"))
            .and_then(|value| value.get::<Vec<i32>>().cloned())
        else {
            return false;
        };
        let Some(color_texture) = self.wgpu_color_texture.clone() else {
            return false;
        };
        let Some(prim_id_texture) = self.wgpu_prim_id_texture.clone() else {
            return false;
        };
        let instance_id_texture = self
            .wgpu_instance_id_texture
            .clone()
            .unwrap_or_else(|| prim_id_texture.clone());
        let element_id_texture = self
            .wgpu_element_id_texture
            .clone()
            .unwrap_or_else(|| prim_id_texture.clone());
        let Some(post_color_texture) = self.wgpu_post_color_texture.clone() else {
            return false;
        };
        let Some(hgi_arc) = self.wgpu_hgi.clone() else {
            return false;
        };

        let width = self.render_buffer_size.x.max(1) as u32;
        let height = self.render_buffer_size.y.max(1) as u32;
        let hgi = hgi_arc.write();
        let device = hgi.device();
        let queue = hgi.queue();
        let Some(color_view) = usd_hgi_wgpu::resolve_texture_view(&color_texture) else {
            return false;
        };
        let Some(prim_id_view) = usd_hgi_wgpu::resolve_texture_view(&prim_id_texture) else {
            return false;
        };
        let Some(instance_id_view) = usd_hgi_wgpu::resolve_texture_view(&instance_id_texture) else {
            return false;
        };
        let Some(element_id_view) = usd_hgi_wgpu::resolve_texture_view(&element_id_texture) else {
            return false;
        };
        let Some(output_view) = usd_hgi_wgpu::resolve_texture_view(&post_color_texture) else {
            return false;
        };
        self.ensure_selection_post_pipeline(device);
        let (Some(layout), Some(pipeline), Some(uniform_buf)) = (
            self.selection_post_bind_group_layout.as_ref(),
            self.selection_post_pipeline.as_ref(),
            self.selection_post_uniform_buf.as_ref(),
        ) else {
            return false;
        };
        queue.write_buffer(
            uniform_buf,
            0,
            &Self::write_selection_uniform_bytes(
                request.selection_color,
                request.locate_color,
                request.enable_locate_highlight,
                request.enable_outline,
                request.outline_radius.max(1),
            ),
        );

        let selection_bytes: Vec<u8> = selection_buffer
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        let selection_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("engine_selection_post_offsets"),
            size: selection_bytes.len().max(4) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&selection_buf, 0, &selection_bytes);

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("engine_selection_post_bind_group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(color_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(prim_id_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(instance_id_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(element_id_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(output_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: uniform_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: selection_buf.as_entire_binding(),
                },
            ],
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("engine_selection_post_encoder"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("engine_selection_post_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(width.div_ceil(8), height.div_ceil(8), 1);
        }
        queue.submit(Some(encoder.finish()));
        drop(hgi);

        self.swap_post_fx_color_targets(&Token::new("color"));
        true
    }

    fn finish_hgi_frame_if_started(&mut self) {
        let frame_started = self
            .hd_engine
            .get_task_context_data(&Token::new("hgiFrameStarted"))
            .and_then(|value| value.get::<bool>().copied())
            .unwrap_or(false);
        if !frame_started {
            return;
        }

        if let Some(hgi_arc) = &self.wgpu_hgi {
            hgi_arc.write().end_frame();
        }
        self.hd_engine
            .remove_task_context_data(&Token::new("hgiFrameStarted"));
    }

    fn replay_deferred_post_tasks(
        &mut self,
        post_task_order: &[Token],
        aov_input_task_requests: &[HdxAovInputTaskRequest],
        colorize_selection_task_requests: &[HdxColorizeSelectionTaskRequest],
        color_correction_task_requests: &[HdxColorCorrectionTaskRequest],
        visualize_aov_task_requests: &[HdxVisualizeAovTaskRequest],
        present_task_requests: &[HdxPresentTaskRequest],
    ) {
        let mut aov_index = 0usize;
        let mut colorize_index = 0usize;
        let mut color_correction_index = 0usize;
        let mut visualize_index = 0usize;
        let mut present_index = 0usize;
        let mut frame_finished = false;

        for task_kind in post_task_order {
            match task_kind.as_str() {
                "aovInput" => {
                    if let Some(request) = aov_input_task_requests.get(aov_index) {
                        self.publish_engine_aovs_to_task_context(
                            &request.aov_name,
                            request.include_depth,
                        );
                        aov_index += 1;
                    }
                }
                "colorizeSelection" => {
                    if let Some(request) = colorize_selection_task_requests.get(colorize_index) {
                        let applied = self.apply_selection_colorize(request);
                        log::trace!(
                            "[engine] deferred colorize-selection request: active={} selection={} outline={} applied={}",
                            request.is_active,
                            request.has_selection,
                            request.enable_outline,
                            applied
                        );
                        colorize_index += 1;
                    }
                }
                "colorCorrection" => {
                    if let Some(request) = color_correction_task_requests.get(color_correction_index)
                    {
                        let applied = if request.aov_name == "color"
                            && request.color_correction_mode == color_correction_tokens::srgb()
                        {
                            self.apply_srgb_color_correction(&request.aov_name)
                        } else if request.aov_name == "color"
                            && request.color_correction_mode == color_correction_tokens::opencolorio()
                        {
                            self.apply_ocio_color_correction(request)
                        } else {
                            false
                        };
                        log::trace!(
                            "[engine] deferred color-correction request: aov={} mode={} applied={}",
                            request.aov_name.as_str(),
                            request.color_correction_mode.as_str(),
                            applied
                        );
                        color_correction_index += 1;
                    }
                }
                "visualizeAov" => {
                    if let Some(request) = visualize_aov_task_requests.get(visualize_index) {
                        let applied = self.apply_visualize_aov(request);
                        log::trace!(
                            "[engine] deferred visualize-aov request: aov={} channel={} applied={}",
                            request.aov_name.as_str(),
                            request.channel,
                            applied
                        );
                        visualize_index += 1;
                    }
                }
                "present" => {
                    if let Some(request) = present_task_requests.get(present_index) {
                        log::trace!(
                            "[engine] deferred present request: enabled={} api={}",
                            request.enabled,
                            request.dst_api.as_str()
                        );
                        if request.enabled {
                            self.finish_hgi_frame_if_started();
                            frame_finished = true;
                        }
                        present_index += 1;
                    }
                }
                _ => {}
            }
        }

        if !frame_finished {
            self.finish_hgi_frame_if_started();
        }
    }

    fn apply_hdx_render_task_request_state(&mut self, request: &HdxRenderTaskRequest) {
        let state = request.render_pass_state.get();

        if !state.get_camera_id().is_empty() {
            self.render_pass_state
                .set_camera(state.get_camera_id().clone());
        }

        let viewport = state.get_viewport();
        // Only override viewport if HDX state has meaningful dimensions.
        // Default (0,0,1,1) from uninitialized task state would clobber engine viewport.
        if viewport.z > 1.0 && viewport.w > 1.0 {
            self.render_pass_state
                .set_viewport(viewport.x as f32, viewport.y as f32, viewport.z as f32, viewport.w as f32);
        }

        if let Some(view_matrix) = state.get_view_matrix() {
            self.render_pass_state.set_view_matrix(view_matrix.clone());
        }
        if let Some(proj_matrix) = state.get_proj_matrix() {
            self.render_pass_state.set_proj_matrix(proj_matrix.clone());
        }

        self.render_pass_state
            .set_depth_func(Self::from_hdx_compare_function(state.get_depth_func()));
        self.render_pass_state
            .set_depth_write_enabled(state.get_depth_mask_enabled());
        self.render_pass_state
            .set_blend_enabled(state.get_blend_enabled());
        let (
            color_op,
            color_src,
            color_dst,
            alpha_op,
            alpha_src,
            alpha_dst,
        ) = state.get_blend_state();
        self.render_pass_state.set_blend(
            Self::from_hdx_blend_op(color_op),
            Self::from_hdx_blend_factor(color_src),
            Self::from_hdx_blend_factor(color_dst),
            Self::from_hdx_blend_op(alpha_op),
            Self::from_hdx_blend_factor(alpha_src),
            Self::from_hdx_blend_factor(alpha_dst),
        );
        self.render_pass_state
            .set_alpha_to_coverage(state.get_alpha_to_coverage_enabled());
        Self::apply_hdx_cull_style(&mut self.render_pass_state, state.get_cull_style());

        let hdx_aovs = state.get_aov_bindings();
        let storm_aov_bindings: Vec<HdStAovBinding> = hdx_aovs
            .iter()
            .filter_map(|binding| self.storm_aov_binding_for_request(binding))
            .collect();
        // Only override AOV bindings if HDX task actually specified them.
        // When empty, keep the engine's own color/depth target bindings.
        if !storm_aov_bindings.is_empty() {
            self.render_pass_state.set_aov_bindings(storm_aov_bindings);
        }
    }

    /// Updates render pass state from current camera/viewport settings.
    fn update_render_pass_state(&mut self, params: &RenderParams) {
        self.render_pass_state.set_viewport(
            0.0,
            0.0,
            self.render_buffer_size.x as f32,
            self.render_buffer_size.y as f32,
        );
        self.render_pass_state
            .set_view_matrix(self.view_matrix.clone());
        self.render_pass_state
            .set_proj_matrix(self.projection_matrix.clone());
        self.render_pass_state.set_clear_color(params.clear_color);

        let polygon_mode = match params.draw_mode {
            DrawMode::Wireframe | DrawMode::WireframeOnSurface => HdStPolygonRasterMode::Line,
            DrawMode::Points => HdStPolygonRasterMode::Point,
            _ => HdStPolygonRasterMode::Fill,
        };
        self.render_pass_state.set_polygon_raster_mode(polygon_mode);

        // ShadedFlat/GeomFlat: face normals via dpdx/dpdy in fragment shader
        self.render_pass_state.set_flat_shading(matches!(
            params.draw_mode,
            DrawMode::ShadedFlat | DrawMode::GeomFlat
        ));

        self.render_pass_state
            .set_enable_scene_materials(params.enable_scene_materials);

        // Selection highlight follows the HdxSelectionTask -> HdxColorizeSelectionTask
        // post-FX path. Keeping the legacy in-pass highlight here causes a
        // double-application once deferred colorizeSelection is replayed.
        self.render_pass_state
            .set_selection_highlight(false, self.selection_color, std::iter::empty());

        // Pass default material ambient/specular from UI to draw batches
        self.render_pass_state
            .set_default_material_ambient(params.default_material_ambient);
        self.render_pass_state
            .set_default_material_specular(params.default_material_specular);

        // Depth-only mode: write depth but suppress color (HiddenSurfaceWireframe prepass)
        self.render_pass_state.set_depth_only(params.depth_only);

        // Preserve depth from a previous pass (wireframe over depth prepass).
        let depth_load_op = if params.preserve_depth {
            HgiAttachmentLoadOp::Load
        } else {
            HgiAttachmentLoadOp::Clear
        };
        self.render_pass_state.set_depth_load_op(depth_load_op);

        // Forward cull style from render params to render pass state
        let cull_mode = match params.cull_style {
            CullStyle::Nothing | CullStyle::NoOpinion => HgiCullMode::None,
            CullStyle::Back | CullStyle::BackUnlessDoubleSided => HgiCullMode::Back,
            CullStyle::Front => HgiCullMode::Front,
        };
        self.render_pass_state.set_cull_mode(cull_mode);
        self.render_pass_state
            .set_cull_enabled(cull_mode != HgiCullMode::None);

        // Forward clip planes from render params
        if !params.clip_planes.is_empty() {
            let planes: Vec<[f64; 4]> = params
                .clip_planes
                .iter()
                .map(|v| [v.x, v.y, v.z, v.w])
                .collect();
            self.render_pass_state.set_clip_planes(planes);
        }
    }

    /// Renders a batch of paths.
    ///
    /// **OpenUSD reference:** `UsdImagingGLEngine::PrepareBatch` + execute path (see `AGENTS.md`).
    /// Order aligned with C++: apply pending USD updates / flush notices → render pass state /
    /// task controller → `HdEngine::execute` (`HdRenderIndex::SyncAll`) → refresh viewer caches
    /// from the render index (`update_dirty_transforms` or `sync_render_index_state`) →
    /// rebuild draw items only when [`HdStRenderPass::check_staleness_full`] reports stale versions
    /// or `draw_items_dirty` (port of `_UpdateDrawItemsIfNeeded`).
    ///
    /// # Arguments
    ///
    /// * `paths` - Paths to render
    /// * `params` - Rendering parameters
    pub fn render_batch(&mut self, paths: &[Path], params: &RenderParams) {
        usd_trace::trace_scope!("engine_render_batch");
        if !self.prepared {
            return;
        }
        if let Some(render_pass) = self.render_pass.as_mut() {
            let requested_root_paths = if paths.is_empty() {
                vec![Path::absolute_root()]
            } else {
                let mut requested_paths = paths.to_vec();
                requested_paths.sort();
                requested_paths
            };
            if render_pass.get_rprim_collection().get_root_paths() != requested_root_paths.as_slice() {
                let mut collection = render_pass.get_rprim_collection().clone();
                collection.set_root_paths(requested_root_paths);
                render_pass.set_rprim_collection(collection);
                self.draw_items_dirty = true;
            }
        }

        // `_PreSetTime` / `ApplyPendingUpdates` / refine fallback run in `prepare_batch`
        // (C++ `PrepareBatch`), which must be called before each `RenderBatch` when
        // mirroring `UsdImagingGLEngine::Render`.

        // Sync view/projection from scene camera if camera_path is set (P1-CP fix).
        // Must run before update_render_pass_state which pushes matrices to render_pass_state.
        self.sync_camera_from_stage();

        // Update render pass state (viewport, camera, clear color)
        self.update_render_pass_state(params);

        // Sync dome light settings from render params to engine state.
        // dome_light_enabled comes from UI checkbox every frame.
        // dome_light_texture_path is set separately via set_dome_light_texture_path()
        // and must NOT be overwritten from params (which defaults to None).
        if self.dome_light_enabled != params.dome_light_enabled {
            self.dome_light_enabled = params.dome_light_enabled;
            self.ibl_dirty = true;
        }

        self.sync_task_controller_state(params);

        // Sync mesh data from USD and upload to GPU only when scene/time changed.
        let mut hydra_state_dirty = self.draw_items_dirty;
        if self.mesh_sync_dirty {
            let sync_t0 = std::time::Instant::now();
            log::debug!(
                "[engine] mesh_sync triggered (draw_items_dirty={})",
                self.draw_items_dirty
            );
            self.mesh_sync_dirty = false;
            hydra_state_dirty = true;
            log::trace!("[PERF] sync_meshes: {:?}", sync_t0.elapsed());
        }
        // Time change alone: refresh viewer caches after `HdEngine::execute` (C++: not `mesh_sync_dirty`).
        hydra_state_dirty |= self.viewer_bookkeeping_pending;

        let mut refreshed_hydra_state = false;
        let mut refreshed_hydra_transforms_only = false;
        // Full rebuild needed on first render, structural changes, or
        // non-transform dirty bits (materials, topology, etc.).
        // Use dirty flags instead of .is_empty()/.is_none() — cached values
        // survive animation ticks (rprim IDs and bbox don't change on time change).
        let force_full_refresh = self.draw_items_dirty
            || (self.scene_bbox_dirty && self.scene_bbox.is_none())
            || self.rprim_ids_dirty;
        let needs_render_index_state_refresh = hydra_state_dirty || force_full_refresh;

        // Must run BEFORE locking render_index — internally may re-lock it.
        self.sync_hdx_bridge_context();
        if let Some(index) = self.render_index.clone() {
            if let Some(collection) = self
                .render_pass
                .as_ref()
                .map(|render_pass| render_pass.get_rprim_collection().clone())
            {
                let _di_t0 = std::time::Instant::now();
                let mut index_guard = index.lock().expect("Mutex poisoned");
                let rprim_count = index_guard.get_rprim_ids().len();
                log::debug!("[engine] render_batch: {} rprims in index", rprim_count);

                let mut tasks = self
                    .task_controller
                    .as_ref()
                    .map(|controller| controller.get_rendering_tasks())
                    .unwrap_or_default();
                let exec_t0 = std::time::Instant::now();
                let debug_time_dirty = std::env::var_os("USD_RS_DEBUG_TIME_DIRTY").is_some();
                // Collect dirty rprim set BEFORE execute clears dirty bits.
                let (transform_only, dirty_xform_paths) = Self::classify_dirty_rprims(
                    &index_guard,
                    hydra_state_dirty,
                    force_full_refresh,
                );
                if debug_time_dirty {
                    reset_debug_xformable_stats();
                    reset_debug_data_source_prim_xform_stats();
                    reset_debug_attribute_query_stats();
                }
                self.hd_engine.execute(&mut index_guard, &mut tasks);
                self.progressive_sync_active = false;
                self.progressive_phases_done = false;
                self.initial_scene_load = false;

                log::debug!(
                    "[PERF]   hd_engine_execute: {:?} ({} rprims, {} tasks, xform_only={} dirty_xform={})",
                    exec_t0.elapsed(),
                    rprim_count,
                    tasks.len(),
                    transform_only,
                    dirty_xform_paths.len(),
                );
                if debug_time_dirty {
                    let xformable_stats = read_debug_xformable_stats();
                    let data_source_xform_stats = read_debug_data_source_prim_xform_stats();
                    let attribute_query_stats = read_debug_attribute_query_stats();
                    eprintln!(
                        "[usd_geom_xform-debug] xform_query_calls={} xform_query_total_ms={:.2} ordered_ops_calls={} ordered_ops_total_ms={:.2} ordered_ops_total_ops={} local_xform_calls={} local_xform_total_ms={:.2}",
                        xformable_stats.xform_query_calls,
                        xformable_stats.xform_query_total_ns as f64 / 1_000_000.0,
                        xformable_stats.get_ordered_xform_ops_calls,
                        xformable_stats.get_ordered_xform_ops_total_ns as f64 / 1_000_000.0,
                        xformable_stats.get_ordered_xform_ops_total_ops,
                        xformable_stats.xform_query_local_xform_calls,
                        xformable_stats.xform_query_local_xform_total_ns as f64 / 1_000_000.0,
                    );
                    eprintln!(
                        "[usd_geom_xform-order-debug] op_order_calls={} op_order_total_ms={:.2}",
                        xformable_stats.get_xform_op_order_value_calls,
                        xformable_stats.get_xform_op_order_value_total_ns as f64 / 1_000_000.0,
                    );
                    eprintln!(
                        "[usd_imaging_xform-debug] from_query_calls={} from_query_total_ms={:.2} matrix_typed_calls={} matrix_typed_total_ms={:.2}",
                        data_source_xform_stats.data_source_xform_from_query_calls,
                        data_source_xform_stats.data_source_xform_from_query_total_ns as f64
                            / 1_000_000.0,
                        data_source_xform_stats.data_source_xform_matrix_typed_calls,
                        data_source_xform_stats.data_source_xform_matrix_typed_total_ns as f64
                            / 1_000_000.0,
                    );
                    eprintln!(
                        "[usd_attr_query-debug] new_calls={} new_total_ms={:.2}",
                        attribute_query_stats.new_calls,
                        attribute_query_stats.new_total_ns as f64 / 1_000_000.0,
                    );
                }
                usd_hd_st::mesh::HdStMesh::flush_sync_stats();
                let tracker = index_guard.get_change_tracker();
                let draw_items_version_state = {
                    let render_delegate = self.render_delegate.read();
                    (
                        tracker.get_collection_version(&collection.name),
                        tracker.get_render_tag_version(),
                        tracker.get_task_render_tags_version(),
                        render_delegate.get_material_tags_version(),
                        render_delegate.get_geom_subset_draw_items_version(),
                    )
                };
                drop(index_guard);

                if needs_render_index_state_refresh {
                    let bookkeep_t0 = std::time::Instant::now();
                    if force_full_refresh || !transform_only {
                        // Non-xform changes (points, topology) can alter bbox —
                        // mark dirty so sync_render_index_state recomputes it.
                        if !transform_only {
                            self.scene_bbox_dirty = true;
                        }
                        self.sync_render_index_state();
                        refreshed_hydra_state = true;
                        log::debug!("[PERF]   sync_render_index_state: {:?}", bookkeep_t0.elapsed());
                    } else if !dirty_xform_paths.is_empty() {
                        self.update_dirty_transforms(&dirty_xform_paths);
                        refreshed_hydra_transforms_only = true;
                        log::debug!("[PERF]   update_dirty_transforms: {:?} (dirty={})", bookkeep_t0.elapsed(), dirty_xform_paths.len());
                    }
                    self.viewer_bookkeeping_pending = false;
                }

                let di_check_t0 = std::time::Instant::now();
                let mut draw_items_refresh_needed = self.draw_items_dirty;
                if let Some(render_pass) = self.render_pass.as_mut() {
                    draw_items_refresh_needed |= render_pass.check_staleness_full(
                        draw_items_version_state.0,
                        draw_items_version_state.1,
                        draw_items_version_state.1,
                        draw_items_version_state.2,
                        draw_items_version_state.3,
                        draw_items_version_state.4,
                    );
                }

                if draw_items_refresh_needed {
                    let index_guard = index.lock().expect("Mutex poisoned");
                    let gdi_t0 = std::time::Instant::now();
                    let draw_item_handles = index_guard.get_draw_items(&collection, &[]);
                    log::debug!(
                        "[PERF]   get_draw_items: {:?} ({} items)",
                        gdi_t0.elapsed(),
                        draw_item_handles.len()
                    );
                    drop(index_guard);

                    let cast_t0 = std::time::Instant::now();
                    let storm_items: Vec<HdStDrawItemSharedPtr> = draw_item_handles
                        .into_iter()
                        .filter_map(|h| h.downcast::<HdStDrawItem>().ok())
                        .collect();
                    let storm_paths: Vec<Path> = storm_items
                        .iter()
                        .map(|it| it.get_prim_path().clone())
                        .collect();
                    if let Some(render_pass) = self.render_pass.as_mut() {
                        render_pass.set_draw_items(storm_items);
                    }
                    self.last_storm_item_paths = storm_paths;
                    self.draw_items_dirty = false;
                    log::debug!("[PERF]   draw_items_refresh: {:?} (cast+set)", cast_t0.elapsed());
                }
                log::debug!("[PERF]   draw_items_check: needed={} total={:?}", draw_items_refresh_needed, di_check_t0.elapsed());
            } else {
                log::warn!("[engine] render_batch: no render_pass");
            }
        } else {
            log::warn!("[engine] render_batch: no render_index");
        }

        if refreshed_hydra_state {
            self.collect_scene_lights();
            self.collect_dome_light_ibl();
            self.ibl_dirty = false;
        } else if refreshed_hydra_transforms_only {
            log::trace!("[engine] render_batch: refreshed transform caches without structural relight");
        } else if self.ibl_dirty {
            // HDRI path or dome_light_enabled changed from UI — reload IBL only.
            self.collect_dome_light_ibl();
            self.ibl_dirty = false;
        }

        let mut rendered_any_backend = false;
        let shadow_requested = self
            .hd_engine
            .get_task_context_data(&Token::new("shadowPassRequested"))
            .and_then(|value| value.get::<bool>().copied())
            .unwrap_or(false);
        let skydome_requested = self
            .hd_engine
            .get_task_context_data(&Token::new("skydomeRenderRequested"))
            .and_then(|value| value.get::<bool>().copied())
            .unwrap_or(false);
        let render_task_requests = self
            .hd_engine
            .get_task_context_data(&Token::new("renderTaskRequests"))
            .and_then(|value| value.get::<Vec<HdxRenderTaskRequest>>().cloned())
            .unwrap_or_default();
        let aov_input_task_requests = self
            .hd_engine
            .get_task_context_data(&Token::new("aovInputTaskRequests"))
            .and_then(|value| value.get::<Vec<HdxAovInputTaskRequest>>().cloned())
            .unwrap_or_default();
        let colorize_selection_task_requests = self
            .hd_engine
            .get_task_context_data(&Token::new("colorizeSelectionTaskRequests"))
            .and_then(|value| value.get::<Vec<HdxColorizeSelectionTaskRequest>>().cloned())
            .unwrap_or_default();
        let color_correction_task_requests = self
            .hd_engine
            .get_task_context_data(&Token::new("colorCorrectionTaskRequests"))
            .and_then(|value| value.get::<Vec<HdxColorCorrectionTaskRequest>>().cloned())
            .unwrap_or_default();
        let visualize_aov_task_requests = self
            .hd_engine
            .get_task_context_data(&Token::new("visualizeAovTaskRequests"))
            .and_then(|value| value.get::<Vec<HdxVisualizeAovTaskRequest>>().cloned())
            .unwrap_or_default();
        let present_task_requests = self
            .hd_engine
            .get_task_context_data(&Token::new("presentTaskRequests"))
            .and_then(|value| value.get::<Vec<HdxPresentTaskRequest>>().cloned())
            .unwrap_or_default();
        let post_task_order = self
            .hd_engine
            .get_task_context_data(&Token::new("postTaskOrder"))
            .and_then(|value| value.get::<Vec<Token>>().cloned())
            .unwrap_or_default();
        let legacy_render_requested = self
            .hd_engine
            .get_task_context_data(&Token::new("renderTaskRequested"))
            .and_then(|value| value.get::<bool>().copied())
            .unwrap_or(false);
        let render_requested = legacy_render_requested || !render_task_requests.is_empty() || self.render_needs_update;
        self.render_needs_update = false;

        // -- wgpu path: render via HGI --
        {
            log::trace!(
                "[engine] wgpu_hgi={} render_pass={} color_tex={} depth_tex={}",
                self.wgpu_hgi.is_some(),
                self.render_pass.is_some(),
                self.wgpu_color_texture.is_some(),
                self.wgpu_depth_texture.is_some()
            );
            if self.wgpu_hgi.is_some() {
                // Ensure render targets exist and match viewport size
                self.ensure_wgpu_render_targets();

                if let Some(color_format) = self.render_color_format() {
                    self.render_pass_state
                        .set_fallback_attachment_formats(color_format, HgiFormat::Float32);
                }

                if shadow_requested {
                    // C++ HdxShadowTask::Execute runs before HdxRenderTask.
                    self.render_shadow_pass();
                }

                if let (Some(color_tex), Some(depth_tex)) = (
                    &self.wgpu_color_texture.clone(),
                    &self.wgpu_depth_texture.clone(),
                ) {
                    let base_render_pass_state = self.render_pass_state.clone();
                    let skydome_rendered = if skydome_requested {
                        self.render_skydome(color_tex, depth_tex)
                    } else {
                        false
                    };
                    if skydome_rendered {
                        rendered_any_backend = true;
                    }

                    // Skydome task may set `skydomeNeedsClear` when the dome pass early-outs
                    // (no texture, etc.). Do **not** call `render_clear` when the main geometry
                    // pass is skipped: after the user stops orbiting, `render_requested` is often
                    // false while the camera matrices are unchanged, and clearing here would wipe
                    // the previous shaded frame — solid background / "blue screen" in the viewport.

                    // If skydome rendered, geometry pass must Load (not Clear)
                    // to preserve the background behind geometry.
                    if skydome_rendered {
                        self.render_pass_state
                            .set_color_load_op(HgiAttachmentLoadOp::Load);
                        self.render_pass_state
                            .set_depth_load_op(HgiAttachmentLoadOp::Load);
                    }

                    if render_requested {
                        if let Some(hgi_arc) = self.wgpu_hgi.clone() {
                            let original_collection = self
                                .render_pass
                                .as_ref()
                                .map(|render_pass| render_pass.get_rprim_collection().clone());
                            let st_reg = self
                                .render_delegate
                                .read()
                                .get_st_resource_registry();
                            let mut hgi = hgi_arc.write();

                            if render_task_requests.is_empty() {
                                let st_state = self.render_pass_state.clone();
                                if let Some(ref mut render_pass) = self.render_pass {
                                    let exec_t0 = std::time::Instant::now();
                                    render_pass.execute_with_hgi(
                                        &st_state,
                                        &mut *hgi,
                                        color_tex,
                                        depth_tex,
                                        &st_reg,
                                        Some(&self.rprim_ids_by_path),
                                        WgpuSubmitWait::NoWait,
                                    );
                                    log::debug!("[PERF]   execute_with_hgi: {:?}", exec_t0.elapsed());
                                    rendered_any_backend = true;
                                }
                            } else {
                                for (request_index, request) in render_task_requests.iter().enumerate()
                                {
                                    self.apply_hdx_render_task_request_state(request);
                                    if skydome_rendered || request_index > 0 {
                                        self.render_pass_state
                                            .set_color_load_op(HgiAttachmentLoadOp::Load);
                                        self.render_pass_state
                                            .set_depth_load_op(HgiAttachmentLoadOp::Load);
                                    } else {
                                        self.render_pass_state
                                            .set_color_load_op(HgiAttachmentLoadOp::Clear);
                                        self.render_pass_state
                                            .set_depth_load_op(HgiAttachmentLoadOp::Clear);
                                    }

                                    if let Some(render_pass) = self.render_pass.as_mut() {
                                        let mut collection =
                                            render_pass.get_rprim_collection().clone();
                                        collection.material_tag = request.material_tag.clone();
                                        render_pass.set_rprim_collection(collection);
                                    }

                                    let st_state = self.render_pass_state.clone();
                                    if let Some(ref mut render_pass) = self.render_pass {
                                        let exec_t0 = std::time::Instant::now();
                                        render_pass.execute_with_hgi(
                                            &st_state,
                                            &mut *hgi,
                                            color_tex,
                                            depth_tex,
                                            &st_reg,
                                            Some(&self.rprim_ids_by_path),
                                            WgpuSubmitWait::NoWait,
                                        );
                                        log::trace!(
                                            "[PERF] execute_with_hgi({}): {:?}",
                                            request.material_tag.as_str(),
                                            exec_t0.elapsed()
                                        );
                                        rendered_any_backend = true;
                                    }
                                }
                            }

                            if let Some(collection) = original_collection {
                                if let Some(render_pass) = self.render_pass.as_mut() {
                                    render_pass.set_rprim_collection(collection);
                                }
                            }


                        }
                    }

                    // Restore default load ops for next frame
                    if skydome_rendered || render_requested {
                        self.render_pass_state
                            .set_color_load_op(HgiAttachmentLoadOp::Clear);
                        self.render_pass_state
                            .set_depth_load_op(HgiAttachmentLoadOp::Clear);
                    }
                    self.render_pass_state = base_render_pass_state;
                }
            }

        }

        self.replay_deferred_post_tasks(
            &post_task_order,
            &aov_input_task_requests,
            &colorize_selection_task_requests,
            &color_correction_task_requests,
            &visualize_aov_task_requests,
            &present_task_requests,
        );

        if !rendered_any_backend {
            log::warn!("[engine] render_batch: no active backend rendered a frame");
        }

        self.converged = self.is_converged();
    }

    /// Collect scene lights from the render index and store in render pass state.
    ///
    /// Queries all light sprim types from the render index, downcasts to HdStLight,
    /// converts to GPU-ready LightGpuData, and calls render_pass_state.set_scene_lights().
    /// Falls back gracefully — if no lights found, the draw batch will use default 3-point.
    fn collect_scene_lights(&mut self) {
        let index = match &self.render_index {
            Some(idx) => idx.clone(),
            None => return,
        };

        let light_types = [
            usd_tf::Token::new("domeLight"),
            usd_tf::Token::new("simpleLight"),
            usd_tf::Token::new("sphereLight"),
            usd_tf::Token::new("rectLight"),
            usd_tf::Token::new("diskLight"),
            usd_tf::Token::new("cylinderLight"),
            usd_tf::Token::new("distantLight"),
        ];

        // Scene bbox for directional light shadow computation.
        let (scene_center, scene_radius) = match self.scene_bbox {
            Some((mn, mx)) => {
                let c = [
                    (mn[0] + mx[0]) * 0.5,
                    (mn[1] + mx[1]) * 0.5,
                    (mn[2] + mx[2]) * 0.5,
                ];
                let dx = mx[0] - mn[0];
                let dy = mx[1] - mn[1];
                let dz = mx[2] - mn[2];
                let r = (dx * dx + dy * dy + dz * dz).sqrt() * 0.5;
                (c, r)
            }
            None => ([0.0f32; 3], 10.0f32),
        };

        let index_guard = index.lock().expect("Mutex poisoned");
        let mut hd_lights: Vec<lighting::LightGpuData> = Vec::new();
        let mut shadow_entries: Vec<shadow::ShadowEntry> = Vec::new();
        let mut shadow_index: i32 = -1;

        for light_type in &light_types {
            let ids = index_guard.get_sprim_ids_for_type(light_type);
            for id in &ids {
                if hd_lights.len() >= lighting::MAX_LIGHTS {
                    break;
                }
                if let Some(handle) = index_guard.get_sprim(light_type, id) {
                    if let Some(hd_light) = handle.downcast_ref::<HdStLight>() {
                        // Only include lights with actual intensity
                        let glf = hd_light.get_simple_light();
                        if !glf.as_ref().map(|g| g.has_intensity()).unwrap_or(false) {
                            continue;
                        }

                        let mut gpu_data = lighting::light_from_hd(hd_light);

                        // Shadow computation per C++ simpleLightTask.cpp:278-316.
                        // If the light casts shadows and we haven't exceeded MAX_SHADOWS,
                        // compute the shadow matrix and build a ShadowEntry.
                        if let Some(ref glf) = glf {
                            let sp = hd_light.get_shadow_params();
                            if glf.has_shadow()
                                && sp.enabled
                                && shadow_entries.len() < shadow::MAX_SHADOWS
                            {
                                let type_val = gpu_data.type_pad[0] as u32;
                                let matrix =
                                    if type_val == lighting::GpuLightType::Directional as u32 {
                                        // Directional: ortho from scene bbox
                                        let dir = gpu_data.direction;
                                        shadow::compute_shadow_matrix_directional(
                                            [dir[0], dir[1], dir[2]],
                                            scene_center,
                                            scene_radius,
                                        )
                                    } else {
                                        // Point/Spot: perspective from light position
                                        let pos = gpu_data.position;
                                        let dir = gpu_data.direction;
                                        let outer = gpu_data.params[3]; // outer_angle in radians
                                        let angle = if outer > 0.01 {
                                            outer
                                        } else {
                                            std::f32::consts::FRAC_PI_4
                                        };
                                        shadow::compute_shadow_matrix_spot(
                                            [pos[0], pos[1], pos[2]],
                                            [dir[0], dir[1], dir[2]],
                                            angle,
                                        )
                                    };

                                // Build shadow entry (matrix already has NDC→UV bias)
                                let entry = shadow::build_shadow_entry(&lighting::ShadowParams {
                                    enabled: true,
                                    blur: sp.blur,
                                    bias: sp.bias,
                                    matrix,
                                });
                                shadow_entries.push(entry);

                                // Update shadow index range on LightGpuData
                                // Per C++ simpleLightTask.cpp:302-304
                                shadow_index += 1;
                                gpu_data.type_pad[2] = shadow_index as f32; // shadow_index_start
                                gpu_data.type_pad[3] = shadow_index as f32; // shadow_index_end (no cascades)
                            }
                        }

                        hd_lights.push(gpu_data);
                    }
                }
            }
            if hd_lights.len() >= lighting::MAX_LIGHTS {
                break;
            }
        }

        drop(index_guard);

        log::debug!(
            "[engine] collect_scene_lights: {} lights, {} shadows",
            hd_lights.len(),
            shadow_entries.len(),
        );
        self.render_pass_state.set_scene_lights(hd_lights);
        self.render_pass_state.set_shadow_entries(shadow_entries);
    }

    /// Clear the framebuffer to background color without rendering geometry.
    /// Used by Bounds draw mode to get a clean background before bbox overlays.
    pub fn render_clear(&mut self, params: &RenderParams) {
        if self.device_just_invalidated {
            return;
        }
        self.update_render_pass_state(params);

        self.ensure_wgpu_render_targets();
        if let (Some(hgi_arc), Some(color_tex), Some(depth_tex)) = (
            &self.wgpu_hgi,
            &self.wgpu_color_texture,
            &self.wgpu_depth_texture,
        ) {
            let mut hgi = hgi_arc.write();
            // Create and immediately submit an empty render pass that only clears
            let desc = self
                .render_pass_state
                .make_graphics_cmds_desc(Some(color_tex), Some(depth_tex));
            let gfx_cmds = hgi.create_graphics_cmds(&desc);
            hgi.submit_cmds(gfx_cmds, WgpuSubmitWait::NoWait);
        }
    }

    /// Main rendering entry point.
    ///
    /// Renders the given root prim and its descendants according to the
    /// specified parameters.
    ///
    /// # Arguments
    ///
    /// * `root` - Root prim to render
    /// * `params` - Rendering parameters
    pub fn render(&mut self, root: &Prim, params: &RenderParams) {
        usd_trace::trace_scope!("engine_render");
        // Skip one frame after device invalidation so all stale GPU
        // resources (TextureViews, Buffers) are fully dropped before
        // we try to create new bind groups referencing new resources.
        if self.device_just_invalidated {
            self.device_just_invalidated = false;
            log::info!("[engine] skipping render frame after device invalidation");
            return;
        }
        // C++ `UsdImagingGLEngine::Render`: `PrepareBatch` then `RenderBatch` every frame.
        let prepare_t0 = std::time::Instant::now();
        self.prepare_batch(root, params);
        let prepare_ms = prepare_t0.elapsed().as_secs_f64() * 1000.0;
        if prepare_ms > 5.0 {
            log::info!("[PERF] prepare_batch: {:.1}ms", prepare_ms);
        }

        // Render the actual root requested by the caller.
        let paths = vec![root.path().clone()];
        log::trace!(
            "[engine] render: buffer={}x{}",
            self.render_buffer_size.x,
            self.render_buffer_size.y
        );
        let rb_t0 = std::time::Instant::now();
        self.render_batch(&paths, params);
        let rb_ms = rb_t0.elapsed().as_secs_f64() * 1000.0;
        if rb_ms > 5.0 {
            log::info!("[PERF] render_batch: {:.1}ms", rb_ms);
        }
    }

    /// Returns whether the renderer has converged.
    ///
    /// For progressive renderers, this indicates whether more rendering
    /// passes are needed to refine the result.
    pub fn is_converged(&self) -> bool {
        if let (Some(controller), Some(index)) = (&self.task_controller, &self.render_index) {
            if let Ok(index_guard) = index.lock() {
                return self
                    .hd_engine
                    .are_tasks_converged(&index_guard, &controller.get_rendering_task_paths());
            }
        }
        self.converged
    }

    /// Whether progressive rprim sync is currently in progress.
    pub fn is_progressive_sync_active(&self) -> bool {
        self.progressive_sync_active
    }

    /// Returns (synced, total) rprim counts for progressive sync progress.
    pub fn sync_progress(&self) -> (usize, usize) {
        (self.progressive_synced, self.progressive_total)
    }

    /// Scene bounding box (min, max) computed during last mesh sync.
    pub fn scene_bbox(&self) -> Option<([f32; 3], [f32; 3])> {
        self.scene_bbox
    }

    /// Mark scene_bbox for recomputation on next sync.
    /// Call from zoom-all, frame-selected, or scene-change actions.
    pub fn request_bbox_update(&mut self) {
        self.scene_bbox_dirty = true;
    }

    /// Returns basic render statistics for HUD display.
    ///
    /// (draw_items, total_triangles, total_vertices)
    pub fn render_stats(&self) -> (usize, usize, usize) {
        let draw_items = self.last_storm_item_paths.len();
        let total_tris = draw_items; // approximate: 1 mesh per draw item
        let total_verts = 0; // not tracked yet
        (draw_items, total_tris, total_verts)
    }

    /// Soft invalidate: repopulate scene but keep wgpu device/textures.
    ///
    /// Use for visibility, transform, or display-mode changes.
    pub fn invalidate(&mut self) {
        self.prepared = false;
        self.converged = false;
        self.render_index = None;
        self.scene_indices = None;
        self.task_controller = None;
        self.hd_engine = HdEngine::new();
        self.mesh_sync_dirty = true;
        self.viewer_bookkeeping_pending = false;
        self.draw_items_dirty = true;
        self.last_storm_item_paths.clear();
        self.rprim_ids_by_path.clear();
        self.rprim_ids_dirty = true;
        self.scene_bbox_dirty = true;
        // Full scene rebuild: clear all caches.
        self.material_cache.clear();
    }

    /// Scene-level invalidate: keep wgpu device alive, but clear all GPU
    /// resource caches from the previous scene (buffers, textures, pipelines,
    /// draw batches). Per C++ reference, switching scenes with the same
    /// renderer should NOT destroy the device — just swap the stage.
    pub fn invalidate_scene(&mut self) {
        usd_trace::trace_scope!("Engine::invalidate_scene");
        let _t0 = std::time::Instant::now();
        self.invalidate();
        self.initial_scene_load = true;
        self.progressive_sync_active = false;
        self.progressive_phases_done = false;
        self.progressive_synced = 0;
        self.progressive_total = 0;

        // Drop draw batches from old scene (they reference old mesh buffers)
        self.render_pass = None;
        self.render_pass_state.clear_device_resources();

        // Clear old scene's GPU resources (mesh buffers, shader programs,
        // pipelines) from the resource registry. The wgpu device stays alive.
        {
            let rd = self.render_delegate.read();
            rd.get_st_resource_registry().clear_all_gpu_resources();
        }

        // Flush stale pipeline cache entries for the current device
        let dev_id = self
            .wgpu_hgi
            .as_ref()
            .map(|h| h.read().device_identity())
            .unwrap_or(0);
        usd_hd_st::draw_batch::clear_pipeline_cache_for_device(dev_id);

        // Clear texture cache (old scene textures)
        self.texture_cache.clear();

        // IBL pipelines stay alive — they're device-bound, not scene-bound
        log::trace!("[PERF] invalidate_scene (soft): {:?}", _t0.elapsed());
    }

    /// Full device reset: destroy wgpu device, textures, staging buffers.
    ///
    /// Use when loading a new file or switching stages — the old device's
    /// GPU resources become stale and must not be reused.
    pub fn invalidate_device(&mut self) {
        usd_trace::trace_scope!("Engine::invalidate_device");
        let _t0 = std::time::Instant::now();
        self.invalidate();

        // Drop the render pass first — it holds IndirectDrawBatch::cull_state
        // (wgpu::ComputePipeline + wgpu::BindGroupLayout tied to the old device).
        // Without this, those BGLs outlive the device and cause epoch panics on
        // the next file load when new BGLs are validated against the stale slab.
        self.render_pass = None;

        // Release old device Arcs from render_pass_state (GPU frustum culling refs).
        // Must happen before wgpu_hgi = None so the old device can drop to zero refs.
        self.render_pass_state.clear_device_resources();

        // Drop IBL compute pipelines (bound to old device BGL slab)
        self.ibl_gpu_pipelines = None;

        // Drop skydome pipeline resources tied to old device.
        // Without this, ensure_skydome_pipeline() sees is_some()=true and
        // skips reallocation, leaving skydome_uniform_buf pointing at a dead
        // buffer → queue.write_buffer() panics on next render_skydome().
        self.skydome_pipeline = None;
        self.skydome_pipeline_color_format = None;
        self.skydome_bind_group_layout = None;
        self.skydome_uniform_buf = None;
        self.srgb_post_bind_group_layout = None;
        self.srgb_post_pipeline = None;
        self.srgb_post_sampler = None;
        self.visualize_aov_color_bind_group_layout = None;
        self.visualize_aov_depth_bind_group_layout = None;
        self.visualize_aov_color_pipeline = None;
        self.visualize_aov_depth_pipeline = None;
        self.visualize_aov_sampler = None;
        self.visualize_aov_uniform_buf = None;
        self.selection_post_bind_group_layout = None;
        self.selection_post_pipeline = None;
        self.selection_post_sampler = None;
        self.selection_post_uniform_buf = None;
        self.ocio_post_pass.reset_gpu_resources();

        // Clear resource registry GPU resources BEFORE dropping device.
        // Without this, Arc<WgpuBuffer> handles survive device death and
        // cause `Buffer does not exist` panics on queue.write_buffer().
        {
            let rd = self.render_delegate.read();
            rd.get_st_resource_registry().clear_all_gpu_resources();
        }

        // Flush stale pipelines for this device before dropping it
        let dev_id = self
            .wgpu_hgi
            .as_ref()
            .map(|h| h.read().device_identity())
            .unwrap_or(0);
        usd_hd_st::draw_batch::clear_pipeline_cache_for_device(dev_id);
        if let Some(hgi_arc) = self.wgpu_hgi.clone() {
            let mut hgi = hgi_arc.write();
            for (_, texture) in self.wgpu_aux_aov_textures.drain() {
                hgi.destroy_texture(&texture);
            }
        }
        self.wgpu_color_texture = None;
        self.wgpu_post_color_texture = None;
        self.wgpu_depth_texture = None;
        self.wgpu_prim_id_texture = None;
        self.wgpu_instance_id_texture = None;
        self.wgpu_element_id_texture = None;
        self.wgpu_pick_color_texture = None;
        self.wgpu_pick_instance_texture = None;
        self.wgpu_pick_depth_texture = None;
        self.wgpu_pick_rt_size = Vec2i::new(0, 0);
        self.wgpu_id_color_texture = None;
        self.wgpu_id_depth_texture = None;
        self.wgpu_id_rt_size = Vec2i::new(0, 0);
        self.wgpu_rt_size = Vec2i::new(0, 0);
        self.wgpu_staging = usd_hgi_wgpu::StagingReadback::new();
        self.wgpu_hgi = None;
        self.texture_cache.clear();

        self.device_just_invalidated = true;
        log::trace!("[PERF] invalidate_device: {:?}", _t0.elapsed());
    }

    // -------------------------------------------------------------------------
    // Root Transform and Visibility
    // -------------------------------------------------------------------------

    /// Sets the root transform.
    ///
    /// # Arguments
    ///
    /// * `transform` - New root transform matrix
    pub fn set_root_transform(&mut self, transform: Matrix4d) {
        self.root_transform = transform;
        self.invalidate();
    }

    /// Gets the current root transform.
    pub fn root_transform(&self) -> &Matrix4d {
        &self.root_transform
    }

    /// Sets the root visibility.
    ///
    /// # Arguments
    ///
    /// * `visible` - Whether the root should be visible
    pub fn set_root_visibility(&mut self, visible: bool) {
        self.root_visible = visible;
        self.invalidate();
    }

    /// Gets the current root visibility.
    pub fn is_root_visible(&self) -> bool {
        self.root_visible
    }

    /// Sets whether unloaded prims are displayed with bounding boxes.
    /// Invalidates when the value changes (requires repopulation).
    pub fn set_display_unloaded_prims_with_bounds(&mut self, enabled: bool) {
        if self.params.display_unloaded_prims_with_bounds != enabled {
            self.params.display_unloaded_prims_with_bounds = enabled;
            self.invalidate();
        }
    }

    /// Gets whether unloaded prims are displayed with bounds.
    pub fn display_unloaded_prims_with_bounds(&self) -> bool {
        self.params.display_unloaded_prims_with_bounds
    }

    // -------------------------------------------------------------------------
    // Dome Light / IBL
    // -------------------------------------------------------------------------

    /// Enable or disable fallback dome light IBL.
    /// When enabled and no scene dome light exists, a procedural sky
    /// or user-specified HDRI is used for image-based lighting.
    pub fn set_dome_light_enabled(&mut self, enabled: bool) {
        if self.dome_light_enabled != enabled {
            self.dome_light_enabled = enabled;
            self.ibl_dirty = true;
        }
    }

    /// Set the fallback HDRI file path for dome light.
    /// Loaded when dome_light_enabled=true but no scene dome light exists.
    pub fn set_dome_light_texture_path(&mut self, path: Option<String>) {
        if self.dome_light_texture_path != path {
            self.dome_light_texture_path = path;
            self.ibl_dirty = true;
        }
    }

    // -------------------------------------------------------------------------
    // Camera State
    // -------------------------------------------------------------------------

    /// Sets the scene camera path for rendering.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the camera prim
    pub fn set_camera_path(&mut self, path: Path) {
        // C++ engine does NOT invalidate on camera path change — it forwards
        // to the task controller. invalidate() destroys the render index and
        // delegate, causing a full scene repopulation. Just store the path.
        self.camera_path = Some(path.clone());
        if let Some(controller) = self.task_controller.as_mut() {
            controller.clear_free_camera();
            controller.set_camera_path(path);
        }
    }

    /// Gets the current camera path.
    pub fn camera_path(&self) -> Option<&Path> {
        self.camera_path.as_ref()
    }

    /// Sets framing for camera-to-viewport mapping.
    ///
    /// Determines how the camera's filmback plane maps to pixels and which
    /// pixels will be rendered. See `CameraUtilFraming` for details.
    pub fn set_framing(&mut self, framing: CameraUtilFraming) {
        self.framing = framing.clone();
        if let Some(controller) = self.task_controller.as_mut() {
            controller.set_framing(Self::to_hdx_framing(&framing));
        }
    }

    /// Gets the current camera framing.
    pub fn framing(&self) -> &CameraUtilFraming {
        &self.framing
    }

    /// Sets an optional override window policy for frustum conforming.
    ///
    /// When set to `Some(policy)`, this overrides the camera's own window
    /// policy. When `None`, the camera's own policy is used.
    pub fn set_override_window_policy(&mut self, policy: Option<CameraUtilConformWindowPolicy>) {
        self.override_window_policy = policy;
        if let Some(controller) = self.task_controller.as_mut() {
            controller.set_override_window_policy(policy.map(Self::to_hdx_window_policy));
        }
    }

    /// Gets the current override window policy.
    pub fn override_window_policy(&self) -> Option<CameraUtilConformWindowPolicy> {
        self.override_window_policy
    }

    /// Sets the window policy for scene cameras.
    ///
    /// Used when rendering with a scene camera set via `set_camera_path`.
    pub fn set_window_policy(&mut self, policy: CameraUtilConformWindowPolicy) {
        self.window_policy = policy;
    }

    /// Gets the current window policy.
    pub fn window_policy(&self) -> CameraUtilConformWindowPolicy {
        self.window_policy
    }

    /// Sets the render buffer size.
    ///
    /// GUI applications should set this to the window size.
    /// Does not invalidate the scene; only resizes the draw target.
    ///
    /// # Arguments
    ///
    /// * `size` - Buffer size in pixels (width, height)
    pub fn set_render_buffer_size(&mut self, size: Vec2i) {
        if self.render_buffer_size != size {
            self.render_buffer_size = size.clone();
            self.render_needs_update = true;
            if let Some(controller) = self.task_controller.as_mut() {
                controller.set_render_buffer_size(size);
            }
        }
    }

    /// Gets the current render buffer size.
    pub fn render_buffer_size(&self) -> &Vec2i {
        &self.render_buffer_size
    }

    /// Sets the viewport for rendering (deprecated - use set_render_buffer_size).
    ///
    /// # Arguments
    ///
    /// * `viewport` - Viewport as (x, y, width, height)
    #[deprecated(note = "Use set_render_buffer_size instead")]
    pub fn set_render_viewport(&mut self, viewport: Vec4d) {
        self.render_buffer_size = Vec2i::new(viewport[2] as i32, viewport[3] as i32);
        self.invalidate();
    }

    /// Sets camera state directly with matrices.
    ///
    /// Does not invalidate the scene; camera changes do not require repopulation.
    ///
    /// # Arguments
    ///
    /// * `view_matrix` - View matrix
    /// * `projection_matrix` - Projection matrix
    pub fn set_camera_state(&mut self, view_matrix: Matrix4d, projection_matrix: Matrix4d) {
        if self.view_matrix != view_matrix || self.projection_matrix != projection_matrix {
            self.render_needs_update = true;
        }
        self.view_matrix = view_matrix.clone();
        self.projection_matrix = projection_matrix.clone();
        if self.camera_path.is_none() {
            if let Some(controller) = self.task_controller.as_mut() {
                controller.clear_free_camera();
                controller.set_free_camera_matrices(view_matrix, projection_matrix);
            }
        }
    }

    /// Gets the current view matrix.
    pub fn view_matrix(&self) -> &Matrix4d {
        &self.view_matrix
    }

    /// Gets the current projection matrix.
    pub fn projection_matrix(&self) -> &Matrix4d {
        &self.projection_matrix
    }

    // -------------------------------------------------------------------------
    // Time
    // -------------------------------------------------------------------------

    /// Set the current time code for animation.
    ///
    /// Port of `UsdImagingGLEngine::SetRenderFrameTimecode` /
    /// `UsdImagingGLEngine::SetSceneGlobalsCurrentFrame`.
    ///
    /// Propagates time to `StageSceneIndex::set_time` which dispatches
    /// `PrimsDirtied` notices through the scene index observer chain.
    /// The chain terminates at `HdSceneIndexAdapterSceneDelegate::prims_dirtied`
    /// which translates dirty locators into `HdChangeTracker` dirty bits,
    /// causing `sync_rprims` to re-read affected attributes on next sync.
    pub fn set_time(&mut self, time: TimeCode) {
        if self.time_code == time {
            return;
        }
        log::debug!(
            "[engine] set_time: {} -> {}",
            self.time_code.value(),
            time.value(),
        );
        self.time_code = time;
        self.viewer_bookkeeping_pending = true;
        self.scene_globals_current_frame = time.value();

        if let Some(scene_indices) = self.scene_indices.as_ref() {
            // C++ _PreSetTime: flush pending notices before time change
            usd_hd::scene_index::HdNoticeBatchingSceneIndex::flush_unlocked(
                &scene_indices.notice_batching_typed,
            );
            scene_indices
                .stage_scene_index
                .set_time(time, false);

            // C++ _PostSetTime: flush notices generated by SetTime
            usd_hd::scene_index::HdNoticeBatchingSceneIndex::flush_unlocked(
                &scene_indices.notice_batching_typed,
            );
        }
        log::debug!("[engine] set_time complete: {}", self.time_code.value());
    }

    /// Gets the current time code.
    pub fn time(&self) -> TimeCode {
        self.time_code
    }

    // -------------------------------------------------------------------------
    // Selection
    // -------------------------------------------------------------------------

    /// Sets the list of selected prim paths for highlighting.
    ///
    /// # Arguments
    ///
    /// * `paths` - Paths to highlight
    pub fn set_selected(&mut self, paths: Vec<Path>) {
        self.selected_paths = paths;
        if let Some(controller) = self.task_controller.as_ref() {
            controller
                .get_selection_tracker()
                .set_selection(&self.selected_paths);
        }
    }

    /// Clears the selection.
    pub fn clear_selected(&mut self) {
        self.selected_paths.clear();
        if let Some(controller) = self.task_controller.as_ref() {
            controller.get_selection_tracker().clear_selection();
        }
    }

    /// Adds a path to the selection.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to add
    /// * `instance_index` - Instance index (use -1 for all instances)
    pub fn add_selected(&mut self, path: Path, _instance_index: i32) {
        if !self.selected_paths.contains(&path) {
            self.selected_paths.push(path.clone());
            if let Some(controller) = self.task_controller.as_ref() {
                controller.get_selection_tracker().select(path);
            }
        }
    }

    /// Gets the currently selected paths.
    pub fn selected_paths(&self) -> &[Path] {
        &self.selected_paths
    }

    /// Replaces the locate / rollover highlight set.
    pub fn set_located(&mut self, paths: Vec<Path>) {
        self.located_paths = paths;
        if let Some(controller) = self.task_controller.as_ref() {
            controller
                .get_selection_tracker()
                .set_locate_selection(&self.located_paths);
        }
    }

    /// Clears locate / rollover highlighting.
    pub fn clear_located(&mut self) {
        self.located_paths.clear();
        if let Some(controller) = self.task_controller.as_ref() {
            controller.get_selection_tracker().clear_locate_selection();
        }
    }

    /// Gets the currently located paths.
    pub fn located_paths(&self) -> &[Path] {
        &self.located_paths
    }

    /// Sets the selection highlighting color.
    ///
    /// # Arguments
    ///
    /// * `color` - Selection color (RGBA)
    pub fn set_selection_color(&mut self, color: Vec4f) {
        self.selection_color = color.clone();
        if let Some(controller) = self.task_controller.as_mut() {
            controller.set_selection_color(color);
        }
    }

    /// Gets the current selection color.
    pub fn selection_color(&self) -> &Vec4f {
        &self.selection_color
    }

    /// Ensure the main color+depth render targets exist and match current render_buffer_size.
    ///
    /// Creates or recreates wgpu_color_texture / wgpu_depth_texture when the
    /// viewport size changes. Must be called before each render pass.
    fn ensure_wgpu_render_targets(&mut self) {
        let w = self.render_buffer_size.x.max(1);
        let h = self.render_buffer_size.y.max(1);
        let needed = Vec2i::new(w, h);

        if self.wgpu_color_texture.is_some()
            && self.wgpu_post_color_texture.is_some()
            && self.wgpu_depth_texture.is_some()
            && self.wgpu_prim_id_texture.is_some()
            && self.wgpu_instance_id_texture.is_some()
            && self.wgpu_element_id_texture.is_some()
            && self.wgpu_rt_size == needed
        {
            return;
        }

        let Some(ref hgi_arc) = self.wgpu_hgi else {
            return;
        };
        let mut hgi = hgi_arc.write();

        // Destroy old targets.
        if let Some(ref old) = self.wgpu_color_texture {
            hgi.destroy_texture(old);
        }
        if let Some(ref old) = self.wgpu_post_color_texture {
            hgi.destroy_texture(old);
        }
        if let Some(ref old) = self.wgpu_depth_texture {
            hgi.destroy_texture(old);
        }
        if let Some(ref old) = self.wgpu_prim_id_texture {
            hgi.destroy_texture(old);
        }
        if let Some(ref old) = self.wgpu_instance_id_texture {
            hgi.destroy_texture(old);
        }
        if let Some(ref old) = self.wgpu_element_id_texture {
            hgi.destroy_texture(old);
        }
        for (_, texture) in self.wgpu_aux_aov_textures.drain() {
            hgi.destroy_texture(&texture);
        }

        let dims = usd_gf::Vec3i::new(w, h, 1);

        // Same storage usage as post_color: `swap_post_fx_color_targets` swaps the two RTs.
        let color_desc = HgiTextureDesc::new()
            .with_debug_name("engine_color_rt")
            .with_format(HgiFormat::Float16Vec4)
            .with_usage(
                HgiTextureUsage::COLOR_TARGET
                    | HgiTextureUsage::SHADER_READ
                    | HgiTextureUsage::SHADER_WRITE,
            )
            .with_dimensions(dims);
        self.wgpu_color_texture = Some(hgi.create_texture(&color_desc, None));
        let post_color_desc = HgiTextureDesc::new()
            .with_debug_name("engine_post_color_rt")
            .with_format(HgiFormat::Float16Vec4)
            .with_usage(
                HgiTextureUsage::COLOR_TARGET
                    | HgiTextureUsage::SHADER_READ
                    | HgiTextureUsage::SHADER_WRITE,
            )
            .with_dimensions(dims);
        self.wgpu_post_color_texture = Some(hgi.create_texture(&post_color_desc, None));

        let depth_desc = HgiTextureDesc::new()
            .with_debug_name("engine_depth_rt")
            .with_format(HgiFormat::Float32)
            .with_usage(HgiTextureUsage::DEPTH_TARGET | HgiTextureUsage::SHADER_READ)
            .with_dimensions(dims);
        self.wgpu_depth_texture = Some(hgi.create_texture(&depth_desc, None));

        // ID AOV textures must be initialized to 0xFF ("no hit" / primId -1).
        // The main render pass does not write primId/instanceId/elementId AOVs yet;
        // if left zeroed, the selection-colorize post-pass reads primId=0 for every
        // pixel and paints the entire viewport with the locate highlight color.
        let id_init = vec![0xFFu8; (w * h * 4) as usize];

        let prim_id_desc = HgiTextureDesc::new()
            .with_debug_name("engine_prim_id_rt")
            .with_format(HgiFormat::UNorm8Vec4)
            .with_usage(HgiTextureUsage::COLOR_TARGET | HgiTextureUsage::SHADER_READ)
            .with_dimensions(dims);
        self.wgpu_prim_id_texture = Some(hgi.create_texture(&prim_id_desc, Some(&id_init)));

        let instance_id_desc = HgiTextureDesc::new()
            .with_debug_name("engine_instance_id_rt")
            .with_format(HgiFormat::UNorm8Vec4)
            .with_usage(HgiTextureUsage::COLOR_TARGET | HgiTextureUsage::SHADER_READ)
            .with_dimensions(dims);
        self.wgpu_instance_id_texture = Some(hgi.create_texture(&instance_id_desc, Some(&id_init)));

        let element_id_desc = HgiTextureDesc::new()
            .with_debug_name("engine_element_id_rt")
            .with_format(HgiFormat::UNorm8Vec4)
            .with_usage(HgiTextureUsage::COLOR_TARGET | HgiTextureUsage::SHADER_READ)
            .with_dimensions(dims);
        self.wgpu_element_id_texture = Some(hgi.create_texture(&element_id_desc, Some(&id_init)));

        self.wgpu_rt_size = needed;
    }

    /// Ensure shadow atlas depth texture array exists.
    ///
    /// Creates a `texture_depth_2d_array` (Depth32Float, SHADOW_MAP_SIZE x SHADOW_MAP_SIZE x
    /// MAX_SHADOWS) and a comparison sampler. Only created once; reused across frames.
    fn ensure_shadow_atlas(&mut self) {
        if self.wgpu_shadow_atlas.is_some() {
            return;
        }
        let Some(ref hgi_arc) = self.wgpu_hgi else {
            return;
        };
        let mut hgi = hgi_arc.write();

        let sz = shadow::SHADOW_MAP_SIZE as i32;
        let atlas_desc = HgiTextureDesc::new()
            .with_debug_name("shadow_atlas")
            .with_format(HgiFormat::Float32)
            .with_usage(HgiTextureUsage::DEPTH_TARGET | HgiTextureUsage::SHADER_READ)
            .with_dimensions(usd_gf::Vec3i::new(sz, sz, 1))
            .with_texture_type(HgiTextureType::Texture2DArray)
            .with_layer_count(shadow::MAX_SHADOWS as u16);
        self.wgpu_shadow_atlas = Some(hgi.create_texture(&atlas_desc, None));

        // Comparison sampler: LessEqual matches C++ GL_COMPARE_R_TO_TEXTURE.
        let smp_desc = HgiSamplerDesc::new()
            .with_debug_name("shadow_cmp_sampler")
            .with_compare(HgiCompareFunction::LEqual);
        self.wgpu_shadow_sampler = Some(hgi.create_sampler(&smp_desc));

        log::info!(
            "[engine] shadow atlas created: {}x{} x {} layers",
            shadow::SHADOW_MAP_SIZE,
            shadow::SHADOW_MAP_SIZE,
            shadow::MAX_SHADOWS
        );
    }

    /// Render depth-only shadow passes for each shadow-casting light.
    ///
    /// For each shadow entry, renders all scene geometry from the light's VP
    /// into the corresponding shadow atlas layer. Uses the depth-only vertex
    /// shader from `shadow::depth_only_vertex_shader_wgsl()`.
    ///
    /// C++ reference: HdxShadowTask::Execute (shadowTask.cpp).
    fn render_shadow_pass(&mut self) {
        if !self.render_pass_state.has_shadows() {
            return;
        }

        // Ensure atlas exists
        self.ensure_shadow_atlas();

        let shadow_entries = self.render_pass_state.get_shadow_entries().to_vec();
        if shadow_entries.is_empty() {
            return;
        }

        let Some(ref hgi_arc) = self.wgpu_hgi else {
            return;
        };
        let Some(ref atlas_handle) = self.wgpu_shadow_atlas else {
            return;
        };
        let Some(ref sampler_handle) = self.wgpu_shadow_sampler else {
            return;
        };

        // Pass the atlas + sampler to render_pass_state so draw_batch can bind them.
        self.render_pass_state
            .set_shadow_atlas(atlas_handle.clone(), sampler_handle.clone());

        let hgi = hgi_arc.write();
        let device = hgi.device();
        let queue = hgi.queue();

        // Create shadow pipeline lazily
        if self.shadow_pipeline.is_none() {
            let shader_src = shadow::depth_only_vertex_shader_wgsl();
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("shadow_depth_vs"),
                source: wgpu::ShaderSource::Wgsl(shader_src.into()),
            });

            let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("shadow_bgl"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(128),
                    },
                    count: None,
                }],
            });

            let pll = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("shadow_pll"),
                bind_group_layouts: &[&bgl],
                push_constant_ranges: &[],
            });

            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("shadow_depth_pipeline"),
                layout: Some(&pll),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs_shadow"),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: 12, // vec3<f32> = 12 bytes
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 0,
                            shader_location: 0,
                        }],
                    }],
                    compilation_options: Default::default(),
                },
                fragment: None, // depth-only: no fragment shader
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: Some(wgpu::Face::Back),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::LessEqual,
                    stencil: Default::default(),
                    bias: wgpu::DepthBiasState {
                        constant: 2,      // small constant bias to prevent acne
                        slope_scale: 1.5, // slope-scaled bias
                        clamp: 0.0,
                    },
                }),
                multisample: Default::default(),
                multiview: None,
                cache: None,
            });

            // Uniform buffer: 128 bytes (2x mat4x4) per shadow slice
            let buf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("shadow_uniforms"),
                size: 128 * shadow::MAX_SHADOWS as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            self.shadow_bind_group_layout = Some(bgl);
            self.shadow_pipeline = Some(pipeline);
            self.shadow_uniform_buf = Some(buf);
        }

        let pipeline = self.shadow_pipeline.as_ref().unwrap();
        let bgl = self.shadow_bind_group_layout.as_ref().unwrap();
        let uniform_buf = self.shadow_uniform_buf.as_ref().unwrap();

        // Resolve atlas texture handle to wgpu::Texture for per-layer view creation.
        let atlas_texture = {
            use usd_hgi_wgpu::resolve::resolve_texture;
            match resolve_texture(atlas_handle) {
                Some(t) => t,
                None => {
                    log::warn!("[engine] shadow pass: failed to resolve atlas texture");
                    return;
                }
            }
        };

        // Collect draw item vertex/index data for the shadow pass.
        // We reuse the same geometry as the main render pass.
        let render_pass = match &self.render_pass {
            Some(rp) => rp,
            None => return,
        };
        let draw_items = render_pass.get_draw_items();
        if draw_items.is_empty() {
            return;
        }

        // Render each shadow slice
        for (si, entry) in shadow_entries.iter().enumerate() {
            // Upload shadow VP + identity model matrix to uniform buffer
            let mut ubo_data = [0u8; 128];
            for (i, v) in entry.world_to_shadow.iter().enumerate() {
                ubo_data[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
            }
            // Identity model matrix (64..128)
            let identity = shadow::MAT4_IDENTITY;
            for (i, v) in identity.iter().enumerate() {
                ubo_data[64 + i * 4..64 + i * 4 + 4].copy_from_slice(&v.to_le_bytes());
            }
            queue.write_buffer(uniform_buf, 0, &ubo_data);

            let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: bgl,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buf.as_entire_binding(),
                }],
                label: Some("shadow_bg"),
            });

            // Create depth attachment view targeting this atlas layer
            let depth_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some("shadow_layer_view"),
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_array_layer: si as u32,
                array_layer_count: Some(1),
                ..Default::default()
            });

            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("shadow_encoder"),
            });

            {
                let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("shadow_pass"),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    ..Default::default()
                });

                rpass.set_pipeline(pipeline);
                rpass.set_bind_group(0, Some(&bg), &[]);
                rpass.set_viewport(
                    0.0,
                    0.0,
                    shadow::SHADOW_MAP_SIZE as f32,
                    shadow::SHADOW_MAP_SIZE as f32,
                    0.0,
                    1.0,
                );

                // Draw all meshes using their position-only vertex buffers
                for item in draw_items.iter() {
                    use usd_hd_st::buffer_resource::HdStBufferArrayRange;
                    use usd_hgi_wgpu::resolve::resolve_buffer;
                    let Some(vbar) = item.get_vertex_bar() else {
                        continue;
                    };
                    let Some(st_bar) = vbar.as_any().downcast_ref::<HdStBufferArrayRange>() else {
                        continue;
                    };
                    let Some(buf_res) = st_bar.get_buffer() else {
                        continue;
                    };
                    let Some(gpu_buf) = resolve_buffer(buf_res.get_handle()) else {
                        continue;
                    };
                    let pos_offset = st_bar.get_offset() as u64;
                    rpass.set_vertex_buffer(0, gpu_buf.slice(pos_offset..));

                    // Index buffer from element BAR
                    let Some(ebar) = item.get_element_bar() else {
                        continue;
                    };
                    let Some(st_ebar) = ebar.as_any().downcast_ref::<HdStBufferArrayRange>() else {
                        continue;
                    };
                    let Some(ebuf_res) = st_ebar.get_buffer() else {
                        continue;
                    };
                    let Some(idx_buf) = resolve_buffer(ebuf_res.get_handle()) else {
                        continue;
                    };
                    let idx_offset = st_ebar.get_offset() as u64;
                    let idx_count = (st_ebar.get_size() / 4) as u32; // u32 indices
                    rpass.set_index_buffer(idx_buf.slice(idx_offset..), wgpu::IndexFormat::Uint32);
                    rpass.draw_indexed(0..idx_count, 0, 0..1);
                }
            }

            queue.submit(std::iter::once(encoder.finish()));
        }

        log::debug!(
            "[engine] shadow depth pass: {} slices rendered",
            shadow_entries.len()
        );
    }

    /// Get the raw wgpu HGI instance (for advanced integration).
    ///
    /// Use this to access device/queue for creating presentation textures
    /// or registering native textures in the UI framework.
    pub fn wgpu_hgi(&self) -> Option<&Arc<RwLock<HgiWgpu>>> {
        self.wgpu_hgi.as_ref()
    }

    /// Get the wgpu color render target handle (for texture view extraction).
    pub fn wgpu_color_texture(&self) -> Option<&HgiTextureHandle> {
        self.wgpu_color_texture.as_ref()
    }

    /// Get the format of the current color render target.
    pub fn render_color_format(&self) -> Option<HgiFormat> {
        self.wgpu_color_texture
            .as_ref()?
            .get()
            .map(|texture| texture.descriptor().format)
    }

    /// Whether staged readback returns packed RGBA8 bytes directly.
    pub fn render_color_readback_is_u8(&self) -> bool {
        matches!(self.render_color_format(), Some(HgiFormat::UNorm8Vec4))
    }

    /// Writes the current framebuffer to an image file.
    pub fn write_to_file(&self, filename: &str) -> bool {
        crate::app_utils::frame_recorder::write_frame_from_engine(self, std::path::Path::new(filename))
            .is_ok()
    }

    /// Read rendered pixels as RGBA u8 vec. Backend-agnostic.
    pub fn read_render_pixels(&self) -> Option<Vec<u8>> {
        if let (Some(hgi_arc), Some(color_tex)) = (&self.wgpu_hgi, &self.wgpu_color_texture) {
            log::trace!("[engine] readback via wgpu blit");
            return self.readback_wgpu_texture(
                &mut *hgi_arc.write(),
                color_tex,
            );
        }

        log::trace!("[engine] read_render_pixels: no backend available");
        None
    }

    /// Read rendered pixels as linear RGBA32F.
    ///
    /// On wgpu this preserves the full float render target for HDR export.
    /// Non-float backends fall back to normalized 0..1 conversion where possible.
    pub fn read_render_pixels_linear_rgba32f(&self) -> Option<Vec<f32>> {
        if let (Some(hgi_arc), Some(color_tex)) = (&self.wgpu_hgi, &self.wgpu_color_texture) {
            log::trace!("[engine] linear readback via wgpu blit");
            return self.readback_wgpu_texture_linear_rgba32f(
                &mut *hgi_arc.write(),
                color_tex,
            );
        }

        self.read_render_pixels()
            .map(|pixels| pixels.into_iter().map(|v| f32::from(v) / 255.0).collect())
    }

    // ---------------------------------------------------------------------- //
    // P2-D7: Renderer plugin / AOV queries
    // ---------------------------------------------------------------------- //

    /// Get available renderer plugin IDs. We only have Storm (wgpu).
    /// C++: UsdImagingGLEngine::GetRendererPlugins.
    pub fn get_renderer_plugins(&self) -> Vec<Token> {
        vec![Token::new("HdStormRendererPlugin")]
    }

    /// Get display name for a renderer plugin ID.
    /// C++: UsdImagingGLEngine::GetRendererDisplayName.
    pub fn get_renderer_display_name(id: &Token) -> String {
        if id == "HdStormRendererPlugin" {
            "Storm (wgpu)".to_string()
        } else {
            id.as_str().to_string()
        }
    }

    /// Get current renderer ID. Returns our single Storm plugin.
    pub fn get_current_renderer_id(&self) -> Token {
        Token::new("HdStormRendererPlugin")
    }

    /// Set renderer plugin. Returns true if the plugin was set (always true for Storm).
    /// C++: UsdImagingGLEngine::SetRendererPlugin.
    pub fn set_renderer_plugin(&mut self, id: &Token) -> bool {
        if id == "HdStormRendererPlugin" || id.is_empty() {
            true
        } else {
            log::warn!(
                "[engine] Unknown renderer plugin '{}', only Storm (wgpu) is supported",
                id.as_str()
            );
            false
        }
    }

    /// C++ UsdImagingGLEngine::GetRendererSettingsList
    pub fn get_renderer_settings_list(&self) -> Vec<RendererSetting> {
        self.renderer_settings
            .iter()
            .map(|(key, value)| RendererSetting {
                name: key.as_str().to_string(),
                key: key.clone(),
                setting_type: renderer_setting_type(value),
                default_value: value.clone(),
            })
            .collect()
    }

    /// C++ UsdImagingGLEngine::GetRendererSetting
    pub fn get_renderer_setting(&self, id: &Token) -> usd_vt::Value {
        self.renderer_settings
            .get(id)
            .cloned()
            .unwrap_or_default()
    }

    /// C++ UsdImagingGLEngine::SetRendererSetting
    pub fn set_renderer_setting(&mut self, id: &Token, value: usd_vt::Value) {
        self.renderer_settings.insert(id.clone(), value);
    }

    /// Get available AOV outputs. C++: UsdImagingGLEngine::GetRendererAovs.
    pub fn get_renderer_aovs(&self) -> Vec<Token> {
        let st_candidate = usd_hd::hd_aov_tokens_make_primvar(&Token::new("st"));
        let candidates = [
            Token::new("primId"),
            Token::new("depth"),
            Token::new("normal"),
            Token::new("Neye"),
            st_candidate,
        ];
        let delegate = self.render_delegate.read();
        let mut aovs = vec![Token::new("color")];
        for candidate in candidates {
            let descriptor = delegate.get_default_aov_descriptor(&candidate);
            if descriptor.format != usd_hd::types::HdFormat::Invalid {
                aovs.push(candidate);
            }
        }
        aovs
    }

    // ---------------------------------------------------------------------- //
    // Render settings / render pass prim (C++ sceneGlobalsSceneIndex)
    // ---------------------------------------------------------------------- //

    /// C++ UsdImagingGLEngine::SetActiveRenderSettingsPrimPath
    pub fn set_active_render_settings_prim_path(&mut self, path: usd_sdf::Path) {
        self.active_render_settings_prim_path = path;
    }

    /// C++ UsdImagingGLEngine::GetActiveRenderSettingsPrimPath
    pub fn get_active_render_settings_prim_path(&self) -> &usd_sdf::Path {
        &self.active_render_settings_prim_path
    }

    /// C++ UsdImagingGLEngine::SetActiveRenderPassPrimPath
    pub fn set_active_render_pass_prim_path(&mut self, path: usd_sdf::Path) {
        self.active_render_pass_prim_path = path;
    }

    /// C++ UsdImagingGLEngine::GetActiveRenderPassPrimPath
    pub fn get_active_render_pass_prim_path(&self) -> &usd_sdf::Path {
        &self.active_render_pass_prim_path
    }

    // ---------------------------------------------------------------------- //
    // P2-D8: Color correction
    // ---------------------------------------------------------------------- //

    /// Set color correction mode. C++: SetColorCorrectionSettings.
    /// Valid values: "disabled", "sRGB", "openColorIO".
    pub fn set_color_correction_settings(
        &mut self,
        mode: &str,
        ocio_display: &str,
        ocio_view: &str,
        ocio_color_space: &str,
        ocio_look: &str,
    ) {
        self.color_correction_mode = mode.to_string();
        self.ocio_settings.display = ocio_display.to_string();
        self.ocio_settings.view = ocio_view.to_string();
        self.ocio_settings.color_space = ocio_color_space.to_string();
        self.ocio_settings.looks = ocio_look.to_string();
        log::trace!("[engine] color correction mode set to '{}'", mode);
    }

    /// Get current color correction mode.
    pub fn color_correction_mode(&self) -> &str {
        &self.color_correction_mode
    }

    // ---------------------------------------------------------------------- //
    // P2-D9: BBox / InvokeRendererCommand
    // ---------------------------------------------------------------------- //

    /// Get scene bounding box. Returns (min, max) if computed during mesh sync.
    /// For full UsdGeomBBoxCache-based computation, see usd-geom BBoxCache.
    pub fn get_scene_bbox(&self) -> Option<([f32; 3], [f32; 3])> {
        self.scene_bbox
    }

    /// Invoke a named renderer command. Returns false (no commands supported).
    /// C++: UsdImagingGLEngine::InvokeRendererCommand.
    pub fn invoke_renderer_command(&self, _command: &Token, _args: &HashMap<Token, Value>) -> bool {
        false
    }

    // ---------------------------------------------------------------------- //
    // AOV selection (C++ taskController->SetRenderOutputs)
    // ---------------------------------------------------------------------- //

    /// C++ UsdImagingGLEngine::SetRendererAov
    pub fn set_renderer_aov(&mut self, id: &Token) -> bool {
        self.current_aov = id.clone();
        if let Some(controller) = self.task_controller.as_mut() {
            controller.set_render_outputs(std::slice::from_ref(id));
            controller.set_viewport_render_output(id.clone());
        }
        log::trace!("[engine] AOV set to '{}'", id.as_str());
        true
    }

    /// C++ UsdImagingGLEngine::SetRendererAovs
    pub fn set_renderer_aovs(&mut self, ids: &[Token]) -> bool {
        if let Some(first) = ids.first() {
            self.current_aov = first.clone();
            if let Some(controller) = self.task_controller.as_mut() {
                controller.set_render_outputs(ids);
                controller.set_viewport_render_output(first.clone());
            }
        }
        true
    }

    /// C++ UsdImagingGLEngine::GetAovRenderBuffer.
    ///
    /// Rust returns an owned snapshot because render-index bprims are stored behind
    /// internal locks/type-erased handles instead of raw stable pointers.
    pub fn get_aov_render_buffer(&self, name: &Token) -> Option<EngineAovRenderBuffer> {
        let texture = self.engine_aov_texture(name.as_str())?;
        let descriptor = texture.get()?.descriptor().clone();
        Some(EngineAovRenderBuffer {
            aov_name: name.clone(),
            render_buffer_path: self.aov_render_buffer_path(name),
            dimensions: Vec3i::new(
                descriptor.dimensions.x.max(1),
                descriptor.dimensions.y.max(1),
                descriptor.dimensions.z.max(1),
            ),
            format: descriptor.format,
            multi_sampled: descriptor.sample_count != HgiSampleCount::Count1,
            texture,
        })
    }

    /// Get current active AOV token.
    pub fn current_aov(&self) -> &Token {
        &self.current_aov
    }

    // ---------------------------------------------------------------------- //
    // Pause / Stop / Resume / Restart (C++ renderDelegate control)
    // ---------------------------------------------------------------------- //

    /// C++ UsdImagingGLEngine::IsPauseRendererSupported
    pub fn is_pause_renderer_supported(&self) -> bool {
        // Storm (wgpu) is single-frame, no background rendering to pause
        false
    }

    /// C++ UsdImagingGLEngine::PauseRenderer
    pub fn pause_renderer(&mut self) -> bool {
        self.renderer_paused = true;
        true
    }

    /// C++ UsdImagingGLEngine::ResumeRenderer
    pub fn resume_renderer(&mut self) -> bool {
        self.renderer_paused = false;
        true
    }

    /// Whether the renderer is currently paused.
    pub fn is_renderer_paused(&self) -> bool {
        self.renderer_paused
    }

    /// C++ UsdImagingGLEngine::IsStopRendererSupported
    pub fn is_stop_renderer_supported(&self) -> bool {
        false
    }

    /// C++ UsdImagingGLEngine::StopRenderer
    pub fn stop_renderer(&mut self) -> bool {
        self.renderer_stopped = true;
        true
    }

    /// C++ UsdImagingGLEngine::RestartRenderer
    pub fn restart_renderer(&mut self) -> bool {
        self.renderer_stopped = false;
        true
    }

    /// Whether the renderer is currently stopped.
    pub fn is_renderer_stopped(&self) -> bool {
        self.renderer_stopped
    }

    // ---------------------------------------------------------------------- //
    // GPU / HGI info (C++ queries)
    // ---------------------------------------------------------------------- //

    /// C++ UsdImagingGLEngine::GetGPUEnabled
    pub fn get_gpu_enabled(&self) -> bool {
        self.gpu_enabled
    }

    /// C++ UsdImagingGLEngine::GetRendererHgiDisplayName
    pub fn get_renderer_hgi_display_name(&self) -> &str {
        "wgpu"
    }

    /// C++ UsdImagingGLEngine::IsColorCorrectionCapable (static in C++)
    pub fn is_color_correction_capable() -> bool {
        true
    }

    // ---------------------------------------------------------------------- //
    // Presentation (C++ framebuffer output)
    // ---------------------------------------------------------------------- //

    /// C++ UsdImagingGLEngine::SetEnablePresentation
    pub fn set_enable_presentation(&mut self, enabled: bool) {
        self.enable_presentation = enabled;
        if let Some(controller) = self.task_controller.as_mut() {
            controller.set_enable_presentation(enabled);
        }
    }

    /// Whether presentation is enabled.
    pub fn is_presentation_enabled(&self) -> bool {
        self.enable_presentation
    }

    /// C++ UsdImagingGLEngine::SetPresentationOutput.
    pub fn set_presentation_output(&mut self, api: &Token, _framebuffer: &usd_vt::Value) {
        if let Some(controller) = self.task_controller.as_mut() {
            controller.set_presentation_output(api, &[]);
        }
    }

    // ---------------------------------------------------------------------- //
    // Render stats / commands / async (C++ queries)
    // ---------------------------------------------------------------------- //

    /// C++ UsdImagingGLEngine::GetRenderStats — returns empty dict for Storm (wgpu).
    pub fn get_render_stats(&self) -> HashMap<String, usd_vt::Value> {
        HashMap::new()
    }

    /// C++ UsdImagingGLEngine::GetRendererCommandDescriptors — no commands available.
    pub fn get_renderer_command_descriptors(&self) -> Vec<(Token, String)> {
        Vec::new()
    }

    /// C++ UsdImagingGLEngine::PollForAsynchronousUpdates
    pub fn poll_for_async_updates(&self) -> bool {
        false
    }

    // ---------------------------------------------------------------------- //
    // Lighting (C++ GlfSimpleLightingContext)
    // ---------------------------------------------------------------------- //

    // NOTE: SetLightingState not needed — GlfSimpleLightingContext is OpenGL-specific.
    // Our wgpu pipeline reads scene lights directly via Hydra delegate (LightGpuData).
    // Camera light / manual light override can be added here if needed in the future.

    // ---------------------------------------------------------------------- //
    // Picking
    // ---------------------------------------------------------------------- //
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::Arc;
    use usd_core::{Stage, common::InitialLoadSet};
    use usd_sdf::Layer;

    fn open_reference_stage(relative_path: &str) -> Arc<Stage> {
        let fixture_path: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative_path);
        Stage::open(
            fixture_path.to_str().expect("fixture path utf8"),
            InitialLoadSet::LoadAll,
        )
        .expect("open reference stage")
    }

    fn scene_index_xform_chain(
        scene: &usd_hd::scene_index::HdSceneIndexHandle,
        path: &Path,
    ) -> Vec<String> {
        let mut chain = Vec::new();
        let mut current = path.clone();
        loop {
            let prim = usd_hd::scene_index::si_ref(scene).get_prim(&current);
            let matrix = prim
                .data_source
                .as_ref()
                .and_then(|ds| usd_hd::schema::HdXformSchema::get_from_parent(ds).get_matrix())
                .map(|matrix_ds| matrix_ds.get_typed_value(0.0).to_array());
            chain.push(format!(
                "{} defined={} type={} xform={:?}",
                current,
                prim.is_defined(),
                prim.prim_type,
                matrix
            ));
            let parent = current.get_parent_path();
            if parent == current {
                break;
            }
            current = parent;
        }
        chain
    }

    fn stage_scene_index_local_xform_chain(
        scene: &crate::stage_scene_index::StageSceneIndex,
        path: &Path,
    ) -> Vec<String> {
        let mut chain = Vec::new();
        let mut current = path.clone();
        loop {
            let prim = usd_hd::scene_index::HdSceneIndexBase::get_prim(scene, &current);
            let matrix = prim
                .data_source
                .as_ref()
                .and_then(|ds| usd_hd::schema::HdXformSchema::get_from_parent(ds).get_matrix())
                .map(|matrix_ds| matrix_ds.get_typed_value(0.0).to_array());
            chain.push(format!(
                "{} defined={} type={} local_xform={:?}",
                current,
                prim.is_defined(),
                prim.prim_type,
                matrix
            ));
            let parent = current.get_parent_path();
            if parent == current {
                break;
            }
            current = parent;
        }
        chain
    }

    #[test]
    fn test_engine_parameters_default() {
        let params = EngineParameters::default();
        assert!(params.gpu_enabled);
        assert!(params.enable_usd_draw_modes);
        assert!(!params.display_unloaded_prims_with_bounds);
    }

    #[test]
    fn test_engine_parameters_builder() {
        let params = EngineParameters::new()
            .with_gpu_enabled(false)
            .with_renderer_plugin_id(Token::new("GL"));

        assert!(!params.gpu_enabled);
        assert_eq!(params.renderer_plugin_id, Token::new("GL"));
    }

    #[test]
    fn test_engine_new() {
        let engine = Engine::with_defaults();
        assert!(engine.is_root_visible());
        assert_eq!(engine.render_buffer_size()[0], 1920);
        assert_eq!(engine.render_buffer_size()[1], 1080);
    }

    #[test]
    fn test_engine_root_transform() {
        let mut engine = Engine::with_defaults();
        let transform = Matrix4d::identity();
        engine.set_root_transform(transform);
        assert_eq!(engine.root_transform(), &Matrix4d::identity());
    }

    #[test]
    fn test_engine_selection() {
        let mut engine = Engine::with_defaults();

        let path1 = Path::from_string("/World/Cube").unwrap();
        let path2 = Path::from_string("/World/Sphere").unwrap();

        engine.set_selected(vec![path1.clone()]);
        assert_eq!(engine.selected_paths().len(), 1);

        engine.add_selected(path2.clone(), -1);
        assert_eq!(engine.selected_paths().len(), 2);

        engine.clear_selected();
        assert_eq!(engine.selected_paths().len(), 0);
    }

    #[test]
    fn test_engine_locate_selection() {
        let mut engine = Engine::with_defaults();
        let path = Path::from_string("/World/Cube").unwrap();

        engine.set_located(vec![path.clone()]);
        assert_eq!(engine.located_paths(), &[path]);

        engine.clear_located();
        assert!(engine.located_paths().is_empty());
    }

    #[test]
    fn test_pick_params_default() {
        let params = PickParams::default();
        assert_eq!(params.resolve_mode, Token::new("resolveNearestToCenter"));
        assert_eq!(params.pick_target, usd_hdx::pick_tokens::pick_prims_and_instances());
    }

    #[test]
    fn test_engine_render_buffer_size() {
        let mut engine = Engine::with_defaults();
        let new_size = Vec2i::new(800, 600);
        engine.set_render_buffer_size(new_size);
        assert_eq!(engine.render_buffer_size(), &new_size);
    }

    #[test]
    fn test_engine_camera_state() {
        let mut engine = Engine::with_defaults();
        let view = Matrix4d::identity();
        let proj = Matrix4d::identity();
        engine.set_camera_state(view, proj);
        assert_eq!(engine.view_matrix(), &Matrix4d::identity());
        assert_eq!(engine.projection_matrix(), &Matrix4d::identity());
    }

    #[test]
    fn test_engine_time() {
        let mut engine = Engine::with_defaults();
        let time = TimeCode::new(24.0);
        engine.set_time(time);
        assert_eq!(engine.time(), time);
    }

    /// Matches `UsdImagingGLEngine::_GetRefineLevel` in `usd-refs/.../engine.cpp`.
    #[test]
    fn test_get_refine_level_matches_openusd_table() {
        assert_eq!(Engine::get_refine_level(0.99), 0);
        assert_eq!(Engine::get_refine_level(1.0), 0);
        // 1.08+0.01 stays below 1.1 in f32; 1.09+0.01 can round into the [1.1,1.2) bin like C++.
        assert_eq!(Engine::get_refine_level(1.08), 0);
        assert_eq!(Engine::get_refine_level(1.11), 1);
        // 1.19+0.01 == 1.2 → C++ lands in [1.2,1.3), refine 2.
        assert_eq!(Engine::get_refine_level(1.19), 2);
        assert_eq!(Engine::get_refine_level(1.99), 8);
        assert_eq!(Engine::get_refine_level(2.0), 8);
    }

    #[test]
    fn test_use_usd_imaging_scene_index_is_true() {
        assert!(Engine::use_usd_imaging_scene_index());
    }

    #[test]
    fn test_render_batch_refreshes_draw_items_when_time_changes_and_paths_do_not() {
        usd_core::schema_registry::register_builtin_schemas();

        let usda = r#"#usda 1.0
def Mesh "Mesh" {
    int[] faceVertexCounts = [3]
    int[] faceVertexIndices = [0, 1, 2]
    point3f[] points.timeSamples = {
        1: [(0,0,0), (1,0,0), (0,1,0)],
        2: [(0,0,2), (1,0,2), (0,1,2)]
    }
}
"#;

        let layer: Arc<Layer> = Layer::create_anonymous(Some("engine_time_draw_items"));
        layer.import_from_string(usda);
        let stage =
            Stage::open_with_root_layer(layer, InitialLoadSet::LoadAll).expect("open stage");
        let root = stage.get_pseudo_root();

        let mut engine = Engine::with_defaults();
        let mut params = RenderParams::default();
        let paths = vec![Path::absolute_root()];
        let mesh_path = Path::from_string("/Mesh").expect("mesh path");

        params.frame = TimeCode::new(1.0);
        engine.prepare_batch(&root, &params);

        engine.render_batch(&paths, &params);

        let adapter = engine
            .render_index
            .as_ref()
            .expect("render index after render")
            .lock()
            .expect("render index mutex")
            .get_scene_index_adapter_scene_delegate()
            .expect("scene-index adapter");
        let authored_points_attr = stage
            .get_prim_at_path(&mesh_path)
            .and_then(|prim| prim.get_attribute("points"))
            .expect("authored points attr");
        let terminal_prim = engine
            .scene_indices
            .as_ref()
            .expect("scene indices after render")
            .final_scene_index
            .read()
            .get_prim(&mesh_path);
        let terminal_names = terminal_prim
            .data_source
            .as_ref()
            .map(|ds| ds.get_names())
            .unwrap_or_default();
        let mesh_names = terminal_prim
            .data_source
            .as_ref()
            .and_then(|ds| ds.get(&Token::new("mesh")))
            .and_then(|ds| ds.as_container())
            .map(|ds| ds.get_names())
            .unwrap_or_default();
        let topology_names = terminal_prim
            .data_source
            .as_ref()
            .and_then(|ds| ds.get(&Token::new("mesh")))
            .and_then(|ds| ds.as_container())
            .and_then(|mesh| mesh.get(&Token::new("topology")))
            .and_then(|ds| ds.as_container())
            .map(|ds| ds.get_names())
            .unwrap_or_default();
        let face_counts_value_type = terminal_prim
            .data_source
            .as_ref()
            .and_then(|ds| ds.get(&Token::new("mesh")))
            .and_then(|ds| ds.as_container())
            .and_then(|mesh| mesh.get(&Token::new("topology")))
            .and_then(|ds| ds.as_container())
            .and_then(|topology| topology.get(&Token::new("faceVertexCounts")))
            .and_then(|ds| ds.as_sampled().map(|sampled| sampled.get_value(0.0)))
            .and_then(|value| value.type_name())
            .unwrap_or("<not-sampled>");
        let point_value_type = terminal_prim
            .data_source
            .as_ref()
            .and_then(|ds| ds.get(&Token::new("primvars")))
            .and_then(|ds| ds.as_container())
            .and_then(|primvars| primvars.get(&Token::new("points")))
            .and_then(|ds| ds.as_container())
            .and_then(|primvar| primvar.get(&Token::new("primvarValue")))
            .and_then(|ds| ds.as_sampled().map(|sampled| sampled.get_value(0.0)))
            .and_then(|value| value.type_name())
            .unwrap_or("<not-sampled>");
        let topo = adapter.get_mesh_topology(&mesh_path);
        assert_eq!(
            topo.face_vertex_counts,
            vec![3],
            "terminal prim_type={} names={:?} mesh_names={:?} topology_names={:?} face_counts_type={} point_attr_type={} point_sdf_type={} point_value_type={}",
            terminal_prim.prim_type,
            terminal_names,
            mesh_names,
            topology_names,
            face_counts_value_type,
            authored_points_attr.type_name(),
            authored_points_attr.get_type_name().as_token(),
            point_value_type,
        );
        assert_eq!(topo.face_vertex_indices, vec![0, 1, 2]);
        let mut point_times = Vec::new();
        let mut point_values = Vec::new();
        assert_eq!(
            adapter.sample_primvar(
                &mesh_path,
                &Token::new("points"),
                2,
                &mut point_times,
                &mut point_values,
            ),
            2,
            "scene-index adapter must expose time-sampled points after pending updates flush",
        );
        assert_eq!(point_times, vec![0.0, 1.0]);
        assert_eq!(
            point_values[0]
                .as_vec_clone::<usd_gf::Vec3f>()
                .unwrap_or_else(|| {
                    panic!(
                        "points payload at t=1 type={:?} attr_type={} sdf_type={} terminal_point_type={}",
                        point_values[0].type_name(),
                        authored_points_attr.type_name(),
                        authored_points_attr.get_type_name().as_token(),
                        point_value_type,
                    )
                })[2],
            usd_gf::Vec3f::new(0.0, 1.0, 0.0),
        );
        drop(adapter);

        let first_draw_item = engine
            .render_pass
            .as_ref()
            .expect("render pass after first render")
            .get_draw_items()
            .first()
            .expect("draw item at t=1");
        let first_draw_item_ptr = Arc::as_ptr(first_draw_item);
        assert_eq!(first_draw_item.get_bbox_min()[2], 0.0);
        assert_eq!(first_draw_item.get_bbox_max()[2], 0.0);

        params.frame = TimeCode::new(2.0);
        engine.prepare_batch(&root, &params);
        engine.render_batch(&paths, &params);

        let adapter = engine
            .render_index
            .as_ref()
            .expect("render index after second render")
            .lock()
            .expect("render index mutex")
            .get_scene_index_adapter_scene_delegate()
            .expect("scene-index adapter");
        let mut second_point_times = Vec::new();
        let mut second_point_values = Vec::new();
        adapter.sample_primvar(
            &mesh_path,
            &Token::new("points"),
            1,
            &mut second_point_times,
            &mut second_point_values,
        );
        let second_points_z = second_point_values
            .first()
            .and_then(|value| value.as_vec_clone::<usd_gf::Vec3f>())
            .map(|points| points[2].z)
            .unwrap_or(f32::NAN);
        drop(adapter);

        let second_draw_item = engine
            .render_pass
            .as_ref()
            .expect("render pass after second render")
            .get_draw_items()
            .first()
            .expect("draw item at t=2");
        assert_eq!(
            Arc::as_ptr(second_draw_item),
            first_draw_item_ptr,
            "time sync should update the existing draw item, not replace it"
        );
        assert_eq!(
            second_draw_item.get_bbox_min()[2],
            2.0,
            "render pass must observe rebuilt draw items after time change (delegate_z={})",
            second_points_z,
        );
        assert_eq!(second_draw_item.get_bbox_max()[2], 2.0);
    }

    #[test]
    fn test_render_batch_refreshes_transforms_when_time_changes_and_paths_do_not() {
        usd_core::schema_registry::register_builtin_schemas();

        let usda = r#"#usda 1.0
def Xform "Root" {
    double3 xformOp:translate.timeSamples = {
        1: (10, 0, 5),
        2: (20, 0, 7)
    }
    uniform token[] xformOpOrder = ["xformOp:translate"]

    def Mesh "Mesh" {
        int[] faceVertexCounts = [3]
        int[] faceVertexIndices = [0, 1, 2]
        point3f[] points = [(0,0,0), (1,0,0), (0,2,0)]
    }
}
"#;

        let layer: Arc<Layer> = Layer::create_anonymous(Some("engine_time_xform_draw_items"));
        layer.import_from_string(usda);
        let stage =
            Stage::open_with_root_layer(layer, InitialLoadSet::LoadAll).expect("open stage");
        let root = stage.get_pseudo_root();

        let mut engine = Engine::with_defaults();
        let mut params = RenderParams::default();
        let paths = vec![Path::absolute_root()];
        let mesh_path = Path::from_string("/Root/Mesh").expect("mesh path");

        params.frame = TimeCode::new(1.0);
        engine.prepare_batch(&root, &params);
        engine.render_batch(&paths, &params);

        let adapter = engine
            .render_index
            .as_ref()
            .expect("render index after first render")
            .lock()
            .expect("render index mutex")
            .get_scene_index_adapter_scene_delegate()
            .expect("scene-index adapter");
        let first_transform = adapter.get_transform(&mesh_path).to_array();
        drop(adapter);

        let direct_first_transform = stage
            .get_prim_at_path(&mesh_path)
            .map(|prim| {
                usd_geom::imageable::Imageable::new(prim)
                    .compute_local_to_world_transform(usd_vt::TimeCode::new(1.0))
                    .to_array()
            })
            .expect("mesh prim at t=1");
        assert_eq!(first_transform, direct_first_transform);
        assert_eq!(engine.model_transforms.get(&mesh_path).copied(), Some(first_transform));
        assert_eq!(engine.scene_bbox(), Some(([10.0, 0.0, 5.0], [11.0, 2.0, 5.0])));

        let first_draw_item = engine
            .render_pass
            .as_ref()
            .expect("render pass after first render")
            .get_draw_items()
            .first()
            .expect("draw item at t=1");
        let first_draw_item_ptr = Arc::as_ptr(first_draw_item);
        assert_eq!(first_draw_item.get_bbox_min(), [0.0, 0.0, 0.0]);
        assert_eq!(first_draw_item.get_bbox_max(), [1.0, 2.0, 0.0]);

        params.frame = TimeCode::new(2.0);
        engine.prepare_batch(&root, &params);
        engine.render_batch(&paths, &params);

        let adapter = engine
            .render_index
            .as_ref()
            .expect("render index after second render")
            .lock()
            .expect("render index mutex")
            .get_scene_index_adapter_scene_delegate()
            .expect("scene-index adapter");
        let second_transform = adapter.get_transform(&mesh_path).to_array();
        drop(adapter);

        let direct_second_transform = stage
            .get_prim_at_path(&mesh_path)
            .map(|prim| {
                usd_geom::imageable::Imageable::new(prim)
                    .compute_local_to_world_transform(usd_vt::TimeCode::new(2.0))
                    .to_array()
            })
            .expect("mesh prim at t=2");
        assert_eq!(second_transform, direct_second_transform);
        assert_ne!(first_transform, second_transform);
        assert_eq!(
            engine.model_transforms.get(&mesh_path).copied(),
            Some(second_transform)
        );
        assert_eq!(engine.scene_bbox(), Some(([20.0, 0.0, 7.0], [21.0, 2.0, 7.0])));

        let second_draw_item = engine
            .render_pass
            .as_ref()
            .expect("render pass after second render")
            .get_draw_items()
            .first()
            .expect("draw item at t=2");
        assert_eq!(
            Arc::as_ptr(second_draw_item),
            first_draw_item_ptr,
            "time sync should update the existing draw item, not replace it"
        );
        assert_eq!(second_draw_item.get_bbox_min(), [0.0, 0.0, 0.0]);
        assert_eq!(second_draw_item.get_bbox_max(), [1.0, 2.0, 0.0]);
    }

    #[test]
    fn test_sync_render_index_state_uses_synced_mesh_data_without_stage_access() {
        usd_core::schema_registry::register_builtin_schemas();

        let usda = r#"#usda 1.0
def Xform "Root" {
    double3 xformOp:translate = (10, 0, 5)
    uniform token[] xformOpOrder = ["xformOp:translate"]

    def Mesh "Mesh" {
        int[] faceVertexCounts = [3]
        int[] faceVertexIndices = [0, 1, 2]
        point3f[] points = [(0,0,0), (1,0,0), (0,2,0)]
    }
}
"#;

        let layer: Arc<Layer> = Layer::create_anonymous(Some("engine_post_sync_bookkeeping"));
        layer.import_from_string(usda);
        let stage =
            Stage::open_with_root_layer(layer, InitialLoadSet::LoadAll).expect("open stage");
        let root = stage.get_pseudo_root();

        let mut engine = Engine::with_defaults();
        let mut params = RenderParams::default();
        let paths = vec![Path::absolute_root()];
        let mesh_path = Path::from_string("/Root/Mesh").expect("mesh path");

        params.frame = TimeCode::earliest_time();
        engine.prepare_batch(&root, &params);
        engine.render_batch(&paths, &params);

        let adapter = engine
            .render_index
            .as_ref()
            .expect("render index after render")
            .lock()
            .expect("render index mutex")
            .get_scene_index_adapter_scene_delegate()
            .expect("scene-index adapter");
        let mesh_xform = adapter.get_transform(&mesh_path).to_array();
        drop(adapter);
        let direct_world_xform = stage
            .get_prim_at_path(&mesh_path)
            .map(|prim| {
                usd_geom::imageable::Imageable::new(prim)
                    .compute_local_to_world_transform(usd_vt::TimeCode::default())
                    .to_array()
            })
            .expect("mesh prim");
        let terminal_xform_type = engine
            .scene_indices
            .as_ref()
            .expect("scene indices after render")
            .final_scene_index
            .read()
            .get_prim(&mesh_path)
            .data_source
            .as_ref()
            .and_then(|ds| ds.get(&Token::new("xform")))
            .and_then(|ds| ds.as_container())
            .and_then(|xform| xform.get(&Token::new("matrix")))
            .and_then(|ds| ds.as_sampled().map(|sampled| sampled.get_value(0.0)))
            .and_then(|value| value.type_name())
            .unwrap_or("<no-xform>");

        let expected_bbox = engine.scene_bbox().expect("scene bbox after render");
        assert_eq!(
            expected_bbox.0,
            [10.0, 0.0, 5.0],
            "mesh_xform={:?} direct_world_xform={:?} terminal_xform_type={}",
            mesh_xform,
            direct_world_xform,
            terminal_xform_type,
        );
        assert_eq!(expected_bbox.1, [11.0, 2.0, 5.0]);

        engine.scene_bbox = None;
        engine.scene_bbox_dirty = true;
        engine.model_transforms.clear();
        engine.rprim_ids_by_path.clear();
        engine.rprim_ids_dirty = true;
        engine.scene_indices = None;

        engine.sync_render_index_state();

        assert_eq!(engine.scene_bbox(), Some(expected_bbox));
        assert_eq!(
            engine.model_transforms.get(&mesh_path).copied(),
            Some([
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [10.0, 0.0, 5.0, 1.0],
            ]),
        );
        assert!(
            engine.rprim_ids_by_path.contains_key(&mesh_path),
            "primId bookkeeping should come from the render index, not the stage",
        );
    }

    #[test]
    fn test_render_batch_renders_skeleton_guide_from_reference_fixture() {
        usd_core::schema_registry::register_builtin_schemas();

        let stage = open_reference_stage(
            "testenv/testUsdImagingGLSkeleton/skeleton.usda",
        );
        let root = stage.get_pseudo_root();
        let skeleton_path = Path::from_string("/SkelChar/Skeleton").expect("skeleton path");

        let mut engine = Engine::with_defaults();
        let params = RenderParams::default();
        let paths = vec![Path::absolute_root()];

        engine.prepare_batch(&root, &params);
        engine.render_batch(&paths, &params);

        let render_index = engine
            .render_index
            .as_ref()
            .expect("render index after render")
            .lock()
            .expect("render index mutex");
        let adapter = render_index
            .get_scene_index_adapter_scene_delegate()
            .expect("scene-index adapter");
        let topo = adapter.get_mesh_topology(&skeleton_path);
        let mut point_times = Vec::new();
        let mut point_values = Vec::new();
        let sampled_points = adapter.sample_primvar(
            &skeleton_path,
            &Token::new("points"),
            1,
            &mut point_times,
            &mut point_values,
        );
        let rprim_type = render_index
            .get_rprim_type_id(&skeleton_path)
            .cloned()
            .unwrap_or_default();
        drop(adapter);
        drop(render_index);

        let terminal_prim = engine
            .scene_indices
            .as_ref()
            .expect("scene indices after render")
            .final_scene_index
            .read()
            .get_prim(&skeleton_path);

        assert_eq!(terminal_prim.prim_type.as_str(), "mesh");
        assert!(
            engine.rprim_ids_by_path.contains_key(&skeleton_path),
            "skeleton guide must be registered as a renderable rprim",
        );
        assert_eq!(
            rprim_type.as_str(),
            "mesh",
            "render index must classify the resolved skeleton guide as a mesh rprim",
        );
        assert!(
            !topo.face_vertex_counts.is_empty() && !topo.face_vertex_indices.is_empty(),
            "skeleton guide mesh topology must reach the scene-index adapter",
        );
        assert!(
            sampled_points > 0 && !point_values.is_empty(),
            "skeleton guide points primvar must reach the scene-index adapter",
        );
        assert!(
            !engine
                .render_pass
                .as_ref()
                .expect("render pass after render")
                .get_draw_items()
                .is_empty(),
            "skeleton fixture should produce draw items once mesh topology and points are synced",
        );
        assert!(engine.scene_bbox().is_some(), "skeleton fixture should produce a scene bbox");
    }

    #[test]
    fn test_render_batch_animates_skinned_mesh_from_reference_fixture() {
        usd_core::schema_registry::register_builtin_schemas();

        let stage = open_reference_stage(
            "testenv/testUsdImagingGLUsdSkel/arm.usda",
        );
        let root = stage.get_pseudo_root();
        let mesh_path = Path::from_string("/Model/Arm").expect("mesh path");

        let mut engine = Engine::with_defaults();
        let mut params = RenderParams::default();
        let paths = vec![Path::absolute_root()];

        params.frame = TimeCode::new(1.0);
        engine.prepare_batch(&root, &params);
        engine.render_batch(&paths, &params);

        let render_index = engine
            .render_index
            .as_ref()
            .expect("render index after render")
            .lock()
            .expect("render index mutex");
        let adapter = render_index
            .get_scene_index_adapter_scene_delegate()
            .expect("scene-index adapter");
        let topo = adapter.get_mesh_topology(&mesh_path);
        let mut point_times = Vec::new();
        let mut point_values = Vec::new();
        let sampled_points = adapter.sample_primvar(
            &mesh_path,
            &Token::new("points"),
            1,
            &mut point_times,
            &mut point_values,
        );
        let rprim_type = render_index.get_rprim_type_id(&mesh_path).cloned().unwrap_or_default();
        drop(adapter);
        drop(render_index);

        let first_bbox = engine.scene_bbox().expect("scene bbox at t=1");
        let first_draw_items = engine
            .render_pass
            .as_ref()
            .expect("render pass at t=1")
            .get_draw_items()
            .len();

        let final_scene_index = engine
            .scene_indices
            .as_ref()
            .expect("scene indices after render")
            .final_scene_index
            .clone();
        let prim_source = final_scene_index
            .read()
            .get_prim(&mesh_path)
            .data_source
            .expect("mesh data source");
        let resolved = crate::skel::DataSourceResolvedPointsBasedPrim::new_from_scene(
            final_scene_index.clone(),
            &mesh_path,
            &prim_source,
        );

        assert!(resolved.is_some(), "skinned mesh must resolve through the final scene index");
        assert!(
            engine.rprim_ids_by_path.contains_key(&mesh_path),
            "skinned mesh must be registered as a renderable rprim",
        );
        assert_eq!(
            rprim_type.as_str(),
            "mesh",
            "render index must classify the skinned prim as a mesh rprim",
        );
        assert!(
            !topo.face_vertex_counts.is_empty() && !topo.face_vertex_indices.is_empty(),
            "skinned mesh topology must reach the scene-index adapter",
        );
        assert!(
            sampled_points > 0 && !point_values.is_empty(),
            "skinned mesh points primvar must reach the scene-index adapter",
        );
        assert!(first_draw_items > 0, "skinned mesh fixture should produce draw items");

        params.frame = TimeCode::new(10.0);
        engine.prepare_batch(&root, &params);
        engine.render_batch(&paths, &params);

        let second_bbox = engine.scene_bbox().expect("scene bbox at t=10");
        let second_draw_items = engine
            .render_pass
            .as_ref()
            .expect("render pass at t=10")
            .get_draw_items()
            .len();

        assert_eq!(
            second_draw_items, first_draw_items,
            "time changes should update the existing skinned draw set, not drop it",
        );
        assert_ne!(
            first_bbox, second_bbox,
            "skinned mesh animation should change the rendered scene bbox across time samples",
        );
    }

    #[test]
    fn test_render_batch_animates_flo_usdz_hierarchical_xforms() {
        usd_core::schema_registry::register_builtin_schemas();

        let stage = open_reference_stage("../../data/usd/flo.usda");
        let root = stage.get_pseudo_root();
        let mesh_path = Path::from_string(
            "/root/flo/noga_a/noga1/noga3_001/group88/group78/group79/noga21/noga21Shape",
        )
        .expect("flo mesh path");

        let mut engine = Engine::with_defaults();
        let mut params = RenderParams::default();
        let paths = vec![Path::absolute_root()];

        params.frame = TimeCode::new(1.0);
        engine.prepare_batch(&root, &params);
        engine.render_batch(&paths, &params);
        let adapter = engine
            .render_index
            .as_ref()
            .expect("render index after first flo render")
            .lock()
            .expect("render index mutex")
            .get_scene_index_adapter_scene_delegate()
            .expect("scene-index adapter");
        let first_transform = adapter.get_transform(&mesh_path).to_array();
        drop(adapter);

        let terminal_first_transform = engine
            .scene_indices
            .as_ref()
            .expect("scene indices after first flo render")
            .final_scene_index
            .read()
            .get_prim(&mesh_path)
            .data_source
            .as_ref()
            .and_then(|ds| usd_hd::schema::HdXformSchema::get_from_parent(ds).get_matrix())
            .map(|matrix_ds| matrix_ds.get_typed_value(0.0).to_array());

        let direct_first_transform = stage
            .get_prim_at_path(&mesh_path)
            .map(|prim| {
                usd_geom::imageable::Imageable::new(prim)
                    .compute_local_to_world_transform(usd_vt::TimeCode::new(1.0))
                    .to_array()
            })
            .expect("flo mesh prim at t=1");
        let final_chain_t1 = scene_index_xform_chain(
            &engine
                .scene_indices
                .as_ref()
                .expect("scene indices after first flo render")
                .final_scene_index,
            &mesh_path,
        );
        let stage_local_chain_t1 = stage_scene_index_local_xform_chain(
            engine
                .scene_indices
                .as_ref()
                .expect("scene indices after first flo render")
                .stage_scene_index
                .as_ref(),
            &mesh_path,
        );
        assert_eq!(
            terminal_first_transform,
            Some(direct_first_transform),
            "terminal scene index must expose the flattened world-space matrix at t=1\nfinal_chain_t1={:#?}\nstage_local_chain_t1={:#?}",
            final_chain_t1,
            stage_local_chain_t1
        );
        assert_eq!(
            first_transform, direct_first_transform,
            "Hydra adapter must expose the same world-space matrix as stage evaluation at t=1\nfinal_chain_t1={:#?}\nstage_local_chain_t1={:#?}",
            final_chain_t1,
            stage_local_chain_t1
        );
        let first_draw_items = engine
            .render_pass
            .as_ref()
            .expect("render pass at flo t=1")
            .get_draw_items()
            .len();
        assert!(first_draw_items > 0, "flo fixture should produce draw items at t=1");

        params.frame = TimeCode::new(50.0);
        engine.prepare_batch(&root, &params);
        engine.render_batch(&paths, &params);
        let adapter = engine
            .render_index
            .as_ref()
            .expect("render index after second flo render")
            .lock()
            .expect("render index mutex")
            .get_scene_index_adapter_scene_delegate()
            .expect("scene-index adapter");
        let second_transform = adapter.get_transform(&mesh_path).to_array();
        drop(adapter);

        let terminal_second_transform = engine
            .scene_indices
            .as_ref()
            .expect("scene indices after second flo render")
            .final_scene_index
            .read()
            .get_prim(&mesh_path)
            .data_source
            .as_ref()
            .and_then(|ds| usd_hd::schema::HdXformSchema::get_from_parent(ds).get_matrix())
            .map(|matrix_ds| matrix_ds.get_typed_value(0.0).to_array());

        let direct_second_transform = stage
            .get_prim_at_path(&mesh_path)
            .map(|prim| {
                usd_geom::imageable::Imageable::new(prim)
                    .compute_local_to_world_transform(usd_vt::TimeCode::new(50.0))
                    .to_array()
            })
            .expect("flo mesh prim at t=50");
        let final_chain_t50 = scene_index_xform_chain(
            &engine
                .scene_indices
                .as_ref()
                .expect("scene indices after second flo render")
                .final_scene_index,
            &mesh_path,
        );
        let stage_local_chain_t50 = stage_scene_index_local_xform_chain(
            engine
                .scene_indices
                .as_ref()
                .expect("scene indices after second flo render")
                .stage_scene_index
                .as_ref(),
            &mesh_path,
        );
        assert_eq!(
            terminal_second_transform,
            Some(direct_second_transform),
            "terminal scene index must expose the flattened world-space matrix at t=50\nfinal_chain_t50={:#?}\nstage_local_chain_t50={:#?}",
            final_chain_t50,
            stage_local_chain_t50
        );
        assert_eq!(
            second_transform, direct_second_transform,
            "Hydra adapter must expose the same world-space matrix as stage evaluation at t=50\nfinal_chain_t50={:#?}\nstage_local_chain_t50={:#?}",
            final_chain_t50,
            stage_local_chain_t50
        );
        assert_ne!(
            first_transform, second_transform,
            "flo hierarchical xform animation must change the sampled mesh transform"
        );

        let second_draw_items = engine
            .render_pass
            .as_ref()
            .expect("render pass at flo t=50")
            .get_draw_items()
            .len();
        assert_eq!(
            second_draw_items, first_draw_items,
            "time changes should update the existing flo draw set, not drop it"
        );
        assert_eq!(
            engine.model_transforms.get(&mesh_path).copied(),
            Some(second_transform),
            "engine transform bookkeeping must track the animated world matrix"
        );
    }

    #[test]
    fn test_render_batch_keeps_flo_usdz_animating_across_multiple_time_changes() {
        usd_core::schema_registry::register_builtin_schemas();

        let stage = open_reference_stage("../../data/usd/flo.usdz");
        let root = stage.get_pseudo_root();
        let mesh_path = Path::from_string(
            "/root/flo/noga_a/noga1/noga3_001/group88/group78/group79/noga21/noga21Shape",
        )
        .expect("flo mesh path");

        let mut engine = Engine::with_defaults();
        let mut params = RenderParams::default();
        let paths = vec![Path::absolute_root()];

        let sample_time = |engine: &Engine, time: f64| {
            let adapter_transform = engine
                .render_index
                .as_ref()
                .expect("render index after flo render")
                .lock()
                .expect("render index mutex")
                .get_scene_index_adapter_scene_delegate()
                .expect("scene-index adapter")
                .get_transform(&mesh_path)
                .to_array();
            let terminal_transform = engine
                .scene_indices
                .as_ref()
                .expect("scene indices after flo render")
                .final_scene_index
                .read()
                .get_prim(&mesh_path)
                .data_source
                .as_ref()
                .and_then(|ds| usd_hd::schema::HdXformSchema::get_from_parent(ds).get_matrix())
                .map(|matrix_ds| matrix_ds.get_typed_value(0.0).to_array());
            let stage_transform = stage
                .get_prim_at_path(&mesh_path)
                .map(|prim| {
                    usd_geom::imageable::Imageable::new(prim)
                        .compute_local_to_world_transform(usd_vt::TimeCode::new(time))
                        .to_array()
                })
                .expect("flo mesh prim on sampled frame");
            let draw_items = engine
                .render_pass
                .as_ref()
                .expect("render pass after flo render")
                .get_draw_items()
                .len();

            (
                adapter_transform,
                terminal_transform,
                stage_transform,
                draw_items,
            )
        };

        let mut sampled = Vec::new();
        for time in [1.0, 10.0, 50.0, 480.0] {
            params.frame = TimeCode::new(time);
            engine.prepare_batch(&root, &params);
            engine.render_batch(&paths, &params);

            let (adapter_transform, terminal_transform, stage_transform, draw_items) =
                sample_time(&engine, time);
            sampled.push((time, adapter_transform, stage_transform, draw_items));

            assert_eq!(
                terminal_transform,
                Some(stage_transform),
                "terminal scene index must expose the stage world transform at t={time}"
            );
            assert_eq!(
                adapter_transform, stage_transform,
                "Hydra adapter must keep updating the animated world transform at t={time}"
            );
            assert_eq!(
                engine.model_transforms.get(&mesh_path).copied(),
                Some(stage_transform),
                "engine transform bookkeeping must stay aligned with stage evaluation at t={time}"
            );
        }

        let first_draw_items = sampled[0].3;
        assert!(
            first_draw_items > 0,
            "flo.usdz fixture should produce draw items on the first sampled frame"
        );
        assert!(
            sampled.iter().all(|(_, _, _, draw_items)| *draw_items == first_draw_items),
            "time changes should update the existing flo.usdz draw set, not drop it"
        );
        assert!(
            sampled.windows(2).any(|pair| pair[0].1 != pair[1].1),
            "sampled flo.usdz frames must actually animate across multiple time changes: {sampled:#?}"
        );
    }

    #[test]
    #[ignore = "diagnostic test for live flo.usdz dirty fanout"]
    fn diag_render_batch_animates_flo_usdz_dirty_fanout() {
        usd_core::schema_registry::register_builtin_schemas();

        let stage = open_reference_stage("../../data/usd/flo.usdz");
        let root = stage.get_pseudo_root();

        let mut engine = Engine::with_defaults();
        let mut params = RenderParams::default();
        let paths = vec![Path::absolute_root()];

        params.frame = TimeCode::new(1.0);
        engine.prepare_batch(&root, &params);
        engine.render_batch(&paths, &params);

        params.frame = TimeCode::new(10.0);
        engine.prepare_batch(&root, &params);
        engine.render_batch(&paths, &params);

        params.frame = TimeCode::new(50.0);
        engine.prepare_batch(&root, &params);
        engine.render_batch(&paths, &params);
    }
}
