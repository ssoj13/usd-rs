//! Frame recorder for USD applications.
//!
//! Port of `pxr/usdImaging/usdAppUtils/frameRecorder.h/cpp`.

use std::path::Path as StdPath;
use std::thread;
use std::time::Duration;

use image::{ImageBuffer, ImageFormat, Rgba};
use usd_camera_util::CameraUtilFraming;
use usd_core::{Stage, TimeCode as UsdTimeCode};
use usd_geom::{BBoxCache, Camera, get_stage_up_axis, usd_geom_tokens};
use usd_gf::{Camera as GfCamera, FOVDirection, Matrix4d, Rect2i, Rotation, Vec2d, Vec2i, Vec3d, Vec4f};
use usd_render::{RenderPass, RenderProduct, RenderSettings, USD_RENDER_TOKENS};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::{TimeCode, Value};

use crate::gl::{Engine, EngineParameters, RenderParams};

/// Default image width in pixels.
const DEFAULT_IMAGE_WIDTH: usize = 960;

/// Default complexity level.
const DEFAULT_COMPLEXITY: f32 = 1.0;

/// Output image format derived from the output path extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FrameOutputFormat {
    Png,
    Jpeg,
    Exr,
}

/// Resolve export format from the file extension.
pub(crate) fn detect_output_format(path: &StdPath) -> Result<FrameOutputFormat, String> {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => Ok(FrameOutputFormat::Png),
        Some("jpg" | "jpeg") => Ok(FrameOutputFormat::Jpeg),
        Some("exr") => Ok(FrameOutputFormat::Exr),
        Some(ext) => Err(format!(
            "unsupported format: .{ext} (use .png, .jpg, or .exr)"
        )),
        None => Err("no file extension — use .png, .jpg, or .exr".to_string()),
    }
}

/// Save display-corrected RGBA8 pixels to PNG or JPEG.
pub(crate) fn save_ldr_pixels(
    pixels: &[u8],
    width: usize,
    height: usize,
    path: &StdPath,
) -> Result<(), String> {
    let expected = width * height * 4;
    if pixels.len() < expected {
        return Err(format!(
            "pixel buffer too small: {} < {} ({}x{}x4)",
            pixels.len(),
            expected,
            width,
            height
        ));
    }

    let format = match detect_output_format(path)? {
        FrameOutputFormat::Png => ImageFormat::Png,
        FrameOutputFormat::Jpeg => ImageFormat::Jpeg,
        FrameOutputFormat::Exr => {
            return Err("EXR export requires linear float pixels".to_string());
        }
    };

    let image = ImageBuffer::<Rgba<u8>, _>::from_raw(
        width as u32,
        height as u32,
        pixels[..expected].to_vec(),
    )
    .ok_or_else(|| "failed to create image buffer".to_string())?;

    image
        .save_with_format(path, format)
        .map_err(|e| format!("failed to save {}: {e}", path.display()))
}

/// Save linear RGBA32F pixels to OpenEXR.
pub(crate) fn save_exr_pixels(
    pixels: &[f32],
    width: usize,
    height: usize,
    path: &StdPath,
) -> Result<(), String> {
    let expected = width * height * 4;
    if pixels.len() < expected {
        return Err(format!(
            "pixel buffer too small: {} < {} ({}x{}x4)",
            pixels.len(),
            expected,
            width,
            height
        ));
    }
    if detect_output_format(path)? != FrameOutputFormat::Exr {
        return Err("save_exr_pixels requires a .exr path".to_string());
    }

    vfx_exr::prelude::write_rgba_file(path, width, height, |x, y| {
        let idx = (y * width + x) * 4;
        (
            pixels[idx],
            pixels[idx + 1],
            pixels[idx + 2],
            pixels[idx + 3],
        )
    })
    .map_err(|e| format!("failed to save {}: {e}", path.display()))
}

pub(crate) fn write_frame_from_engine(engine: &Engine, output_path: &StdPath) -> Result<(), String> {
    match detect_output_format(output_path)? {
        FrameOutputFormat::Exr => {
            let pixels = engine
                .read_render_pixels_linear_rgba32f()
                .ok_or_else(|| "failed to read linear render pixels".to_string())?;
            let size = engine.render_buffer_size();
            save_exr_pixels(
                &pixels,
                size.x.max(1) as usize,
                size.y.max(1) as usize,
                output_path,
            )
        }
        FrameOutputFormat::Png | FrameOutputFormat::Jpeg => {
            let pixels = engine
                .read_render_pixels()
                .ok_or_else(|| "failed to read render pixels".to_string())?;
            let size = engine.render_buffer_size();
            save_ldr_pixels(
                &pixels,
                size.x.max(1) as usize,
                size.y.max(1) as usize,
                output_path,
            )
        }
    }
}

fn has_purpose(purposes: &[Token], purpose: &str) -> bool {
    purposes.iter().any(|p| p == purpose)
}

fn value_to_path(value: &Value) -> Option<Path> {
    value
        .downcast_clone::<Path>()
        .or_else(|| value.downcast_clone::<String>().and_then(|s| Path::from_string(&s)))
        .or_else(|| {
            value
                .downcast_clone::<Token>()
                .and_then(|t| Path::from_string(t.as_str()))
        })
}

fn vt_time_code(time_code: UsdTimeCode) -> TimeCode {
    if time_code.is_default() {
        TimeCode::default()
    } else {
        TimeCode::from(time_code.value())
    }
}

fn compute_camera_to_frame_stage(
    stage: &Stage,
    time_code: UsdTimeCode,
    purposes: &[Token],
) -> GfCamera {
    let mut camera = GfCamera::new();
    let mut bbox_cache = BBoxCache::new(vt_time_code(time_code), purposes.to_vec(), true, false);
    let bbox = bbox_cache.compute_world_bound(&stage.get_pseudo_root());
    let center = bbox.compute_centroid();
    let range = bbox.compute_aligned_range();
    let dim = range.size();
    let up_axis = get_stage_up_axis(stage);

    let plane_corner = if up_axis == usd_geom_tokens().y {
        Vec2d::new(dim[0], dim[1]) / 2.0
    } else {
        Vec2d::new(dim[0], dim[2]) / 2.0
    };
    let plane_radius = plane_corner.length();

    let half_fov = f64::from(camera.field_of_view(FOVDirection::Horizontal)) / 2.0;
    let half_fov_radians = half_fov.to_radians();
    let mut distance = if half_fov_radians.tan().abs() > f64::EPSILON {
        plane_radius / half_fov_radians.tan()
    } else {
        plane_radius
    };

    if up_axis == usd_geom_tokens().y {
        distance += dim[2] / 2.0;
    } else {
        distance += dim[1] / 2.0;
    }

    let clipping_range = camera.clipping_range();
    if distance < f64::from(clipping_range.min()) {
        distance += f64::from(clipping_range.min());
    }

    let mut transform = Matrix4d::identity();
    if up_axis == usd_geom_tokens().y {
        transform.set_translate(&Vec3d::new(center[0], center[1], center[2] + distance));
    } else {
        let rotation = Rotation::from_axis_angle(Vec3d::new(1.0, 0.0, 0.0), 90.0);
        transform.set_rotate(&rotation.get_quat());
        transform.set_translate_only(&Vec3d::new(center[0], center[1] - distance, center[2]));
    }
    camera.set_transform(transform);
    camera
}

fn resolve_render_settings_path(
    stage: &Stage,
    explicit_render_pass_path: &Path,
    explicit_render_settings_path: &Path,
) -> Path {
    if !explicit_render_settings_path.is_empty() {
        return explicit_render_settings_path.clone();
    }

    if !explicit_render_pass_path.is_empty() {
        if let Some(prim) = stage.get_prim_at_path(explicit_render_pass_path) {
            let render_pass = RenderPass::new(prim);
            if let Some(rel) = render_pass.get_render_source_rel() {
                if let Some(path) = rel.get_forwarded_targets().into_iter().next() {
                    return path;
                }
            }
        }
    }

    if stage.has_authored_metadata(&USD_RENDER_TOKENS.render_settings_prim_path) {
        if let Some(value) = stage.get_metadata(&USD_RENDER_TOKENS.render_settings_prim_path) {
            if let Some(path) = value_to_path(&value) {
                return path;
            }
        }
    }

    Path::empty()
}

fn resolve_authored_camera(stage: &Stage, render_settings_path: &Path) -> Camera {
    if render_settings_path.is_empty() {
        return Camera::invalid();
    }
    let Some(prim) = stage.get_prim_at_path(render_settings_path) else {
        return Camera::invalid();
    };
    let settings = RenderSettings::new(prim);
    let Some(rel) = settings.as_settings_base().get_camera_rel() else {
        return Camera::invalid();
    };
    let Some(path) = rel.get_forwarded_targets().into_iter().next() else {
        return Camera::invalid();
    };
    Camera::get(stage, &path)
}

fn render_products_generated(stage: &Stage, render_settings_path: &Path) -> bool {
    if render_settings_path.is_empty() {
        return false;
    }
    let Some(prim) = stage.get_prim_at_path(render_settings_path) else {
        return false;
    };
    let settings = RenderSettings::new(prim);
    let Some(products_rel) = settings.get_products_rel() else {
        return false;
    };

    let mut generated_any = false;
    for product_path in products_rel.get_forwarded_targets() {
        let Some(product_prim) = stage.get_prim_at_path(&product_path) else {
            continue;
        };
        let product = RenderProduct::new(product_prim);
        let Some(attr) = product.get_product_name_attr() else {
            continue;
        };

        let product_path_text = attr
            .get_typed::<Token>(TimeCode::default())
            .map(|t| t.as_str().to_string())
            .or_else(|| attr.get_typed::<String>(TimeCode::default()));
        if let Some(product_path_text) = product_path_text {
            generated_any |= StdPath::new(&product_path_text).exists();
        }
    }

    generated_any
}

/// A utility class for recording images of USD stages.
#[derive(Debug)]
pub struct FrameRecorder {
    renderer_plugin_id: Token,
    gpu_enabled: bool,
    enable_draw_modes: bool,
    image_width: usize,
    complexity: f32,
    color_correction_mode: Token,
    purposes: Vec<Token>,
    render_pass_prim_path: Path,
    render_settings_prim_path: Path,
    camera_light_enabled: bool,
    dome_lights_visible: bool,
    primary_camera_path: Path,
}

impl FrameRecorder {
    pub fn new(
        renderer_plugin_id: Option<Token>,
        gpu_enabled: bool,
        enable_draw_modes: bool,
    ) -> Self {
        Self {
            renderer_plugin_id: renderer_plugin_id.unwrap_or_else(|| Token::new("")),
            gpu_enabled,
            enable_draw_modes,
            image_width: DEFAULT_IMAGE_WIDTH,
            complexity: DEFAULT_COMPLEXITY,
            color_correction_mode: Token::new("disabled"),
            purposes: vec![Token::new("default"), Token::new("proxy")],
            render_pass_prim_path: Path::empty(),
            render_settings_prim_path: Path::empty(),
            camera_light_enabled: true,
            dome_lights_visible: false,
            primary_camera_path: Path::empty(),
        }
    }

    pub fn get_current_renderer_id(&self) -> Token {
        self.renderer_plugin_id.clone()
    }

    pub fn set_renderer_plugin(&mut self, id: Token) -> bool {
        self.renderer_plugin_id = id;
        true
    }

    pub fn set_active_render_pass_prim_path(&mut self, path: Path) {
        self.render_pass_prim_path = path;
    }

    pub fn set_active_render_settings_prim_path(&mut self, path: Path) {
        self.render_settings_prim_path = path;
    }

    pub fn set_image_width(&mut self, width: usize) {
        if width != 0 {
            self.image_width = width;
        }
    }

    pub fn get_image_width(&self) -> usize {
        self.image_width
    }

    pub fn set_complexity(&mut self, complexity: f32) {
        self.complexity = complexity;
    }

    pub fn get_complexity(&self) -> f32 {
        self.complexity
    }

    pub fn set_color_correction_mode(&mut self, mode: Token) {
        if self.gpu_enabled {
            self.color_correction_mode = mode;
        } else {
            self.color_correction_mode = Token::new("disabled");
        }
    }

    pub fn get_color_correction_mode(&self) -> Token {
        self.color_correction_mode.clone()
    }

    pub fn set_camera_light_enabled(&mut self, enabled: bool) {
        self.camera_light_enabled = enabled;
    }

    pub fn is_camera_light_enabled(&self) -> bool {
        self.camera_light_enabled
    }

    pub fn set_dome_light_visibility(&mut self, visible: bool) {
        self.dome_lights_visible = visible;
    }

    pub fn is_dome_light_visible(&self) -> bool {
        self.dome_lights_visible
    }

    pub fn set_included_purposes(&mut self, purposes: Vec<Token>) {
        self.purposes = vec![Token::new("default")];
        for purpose in purposes {
            let purpose_name = purpose.as_str();
            if matches!(purpose_name, "render" | "proxy" | "guide") {
                self.purposes.push(purpose);
            }
        }
    }

    pub fn get_included_purposes(&self) -> &[Token] {
        &self.purposes
    }

    pub fn set_primary_camera_prim_path(&mut self, path: Path) {
        self.primary_camera_path = path;
    }

    pub fn get_primary_camera_prim_path(&self) -> &Path {
        &self.primary_camera_path
    }

    pub fn record(
        &self,
        stage: &Stage,
        camera: &Camera,
        time_code: UsdTimeCode,
        output_path: &str,
    ) -> bool {
        let output_path = StdPath::new(output_path);
        if output_path.as_os_str().is_empty() || detect_output_format(output_path).is_err() {
            return false;
        }

        let render_settings_path = resolve_render_settings_path(
            stage,
            &self.render_pass_prim_path,
            &self.render_settings_prim_path,
        );
        let authored_camera = resolve_authored_camera(stage, &render_settings_path);
        let use_authored_camera = authored_camera.is_valid();
        let effective_camera = if use_authored_camera {
            authored_camera
        } else if camera.is_valid() {
            camera.clone()
        } else {
            Camera::invalid()
        };

        let gf_camera = if effective_camera.is_valid() {
            effective_camera.get_camera(vt_time_code(time_code))
        } else {
            compute_camera_to_frame_stage(stage, time_code, &self.purposes)
        };

        let mut aspect_ratio = gf_camera.aspect_ratio();
        if aspect_ratio.abs() < 1e-4 {
            aspect_ratio = 1.0;
        }
        let image_height = (((self.image_width as f32) / aspect_ratio).round() as usize).max(1);

        let mut engine_params = EngineParameters::new()
            .with_renderer_plugin_id(self.renderer_plugin_id.clone())
            .with_gpu_enabled(self.gpu_enabled);
        engine_params.enable_usd_draw_modes = self.enable_draw_modes;

        let mut engine = Engine::new(engine_params);
        engine.set_enable_presentation(false);
        engine.set_renderer_setting(&Token::new("enableInteractive"), Value::from_no_hash(false));
        engine.set_renderer_setting(
            &Token::new("domeLightCameraVisibility"),
            Value::from_no_hash(self.dome_lights_visible),
        );

        if !render_settings_path.is_empty() {
            engine.set_active_render_settings_prim_path(render_settings_path.clone());
        }
        if !self.render_pass_prim_path.is_empty() {
            engine.set_active_render_pass_prim_path(self.render_pass_prim_path.clone());
        }

        engine.set_renderer_aov(&Token::new("color"));

        let frustum = gf_camera.frustum();
        if effective_camera.is_valid() {
            engine.set_camera_path(effective_camera.prim().path().clone());
        } else {
            engine.set_camera_state(
                frustum.compute_view_matrix(),
                frustum.compute_projection_matrix(),
            );
        }

        let framing = CameraUtilFraming::from_data_window(Rect2i::from_min_size(
            Vec2i::new(0, 0),
            self.image_width as i32,
            image_height as i32,
        ));
        engine.set_framing(framing);
        engine.set_render_buffer_size(Vec2i::new(self.image_width as i32, image_height as i32));
        engine.set_color_correction_settings(
            self.color_correction_mode.as_str(),
            "",
            "",
            "",
            "",
        );

        let render_params = RenderParams {
            frame: time_code,
            complexity: self.complexity,
            color_correction_mode: self.color_correction_mode.clone(),
            clear_color: Vec4f::new(0.0, 0.0, 0.0, 0.0),
            show_proxy: has_purpose(&self.purposes, "proxy"),
            show_render: has_purpose(&self.purposes, "render"),
            show_guides: has_purpose(&self.purposes, "guide"),
            enable_lighting: true,
            enable_scene_materials: true,
            enable_scene_lights: true,
            dome_light_textures_visible: self.dome_lights_visible,
            default_material_ambient: 0.2,
            default_material_specular: 0.1,
            ..RenderParams::default()
        };

        let pseudo_root = stage.get_pseudo_root();
        let mut sleep_ms = 10u64;
        loop {
            engine.render(&pseudo_root, &render_params);
            if engine.is_converged() {
                break;
            }
            thread::sleep(Duration::from_millis(sleep_ms));
            sleep_ms = (sleep_ms + 5).min(100);
        }

        if render_products_generated(stage, &render_settings_path) {
            return true;
        }

        write_frame_from_engine(&engine, output_path).is_ok()
    }
}

impl Default for FrameRecorder {
    fn default() -> Self {
        Self::new(None, true, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_output_format_rejects_missing_extension() {
        assert!(detect_output_format(StdPath::new("frame")).is_err());
    }

    #[test]
    fn test_set_included_purposes_filters_unknown_values() {
        let mut recorder = FrameRecorder::default();
        recorder.set_included_purposes(vec![
            Token::new("default"),
            Token::new("proxy"),
            Token::new("bogus"),
        ]);
        assert_eq!(recorder.get_included_purposes(), &[Token::new("default"), Token::new("proxy")]);
    }

    #[test]
    fn test_gpu_disabled_forces_disabled_color_correction() {
        let mut recorder = FrameRecorder::new(None, false, true);
        recorder.set_color_correction_mode(Token::new("sRGB"));
        assert_eq!(recorder.get_color_correction_mode(), Token::new("disabled"));
    }
}
