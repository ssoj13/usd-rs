//! Frame recorder for USD applications.
//!
//! Port of pxr/usdImaging/usdAppUtils/frameRecorder.h/cpp
//!
//! Utility for recording images of USD stages using Hydra.

use std::path::PathBuf;
use usd_core::{Stage, TimeCode};
use usd_geom::Camera;
use usd_sdf::Path;
use usd_tf::Token;

// ============================================================================
// FrameRecorder
// ============================================================================

/// A utility for recording images of USD stages.
///
/// FrameRecorder uses Hydra to produce rendered images of a USD stage looking
/// through a particular Camera on that stage at a particular TimeCode. The
/// images generated will be effectively the same as what you would see in the
/// viewer in usdview.
///
/// Note that it is assumed that an OpenGL context has already been setup for
/// the FrameRecorder if OpenGL is being used as the underlying HGI device.
/// This is not required for Metal or Vulkan.
///
/// Matches C++ `UsdAppUtilsFrameRecorder`.
///
/// # Examples
///
/// ```rust,ignore
/// use usd_app_utils::FrameRecorder;
/// use usd_core::{Stage, InitialLoadSet, TimeCode};
/// use usd_geom::Camera;
/// use usd_sdf::Path;
///
/// let stage = Stage::open("scene.usda", InitialLoadSet::LoadAll)?;
/// let camera = Camera::get(&stage, &Path::from_string("/cameras/main")?);
///
/// let recorder = FrameRecorder::builder()
///     .image_width(1920)
///     .complexity(2.0)
///     .gpu_enabled(true)
///     .build()?;
///
/// recorder.record(&stage, &camera, TimeCode::default(), "output.png")?;
/// ```
#[derive(Debug, Clone)]
pub struct FrameRecorder {
    /// Renderer plugin ID (empty = default)
    renderer_plugin_id: Token,

    /// Image width in pixels
    image_width: usize,

    /// Geometry refinement complexity
    complexity: f32,

    /// Color correction mode
    color_correction_mode: Token,

    /// GPU rendering enabled
    gpu_enabled: bool,

    /// USD draw modes enabled
    #[allow(dead_code)] // For future draw mode support (cards, bounds, etc.)
    enable_usd_draw_modes: bool,

    /// Camera light (headlight) enabled
    camera_light_enabled: bool,

    /// Dome light visibility
    dome_lights_visible: bool,

    /// Included purposes for rendering
    included_purposes: Vec<Token>,

    /// Primary camera prim path
    primary_camera_path: Option<Path>,

    /// Render pass prim path
    render_pass_prim_path: Option<Path>,

    /// Render settings prim path
    render_settings_prim_path: Option<Path>,
}

impl FrameRecorder {
    /// Creates a new FrameRecorder builder.
    pub fn builder() -> FrameRecorderBuilder {
        FrameRecorderBuilder::default()
    }

    /// Gets the current renderer plugin ID.
    pub fn get_current_renderer_id(&self) -> &Token {
        &self.renderer_plugin_id
    }

    /// Sets the renderer plugin to be used for recording.
    ///
    /// Note that the renderer plugins that may be set will be restricted if
    /// this FrameRecorder instance has disabled the GPU.
    ///
    /// Known renderer plugins:
    /// - `HdStormRendererPlugin` - OpenGL/Vulkan Storm renderer
    /// - `HdPrmanLoaderRendererPlugin` - RenderMan renderer
    /// - `HdEmbreeRendererPlugin` - Embree raytracer
    /// - `HdArnoldRendererPlugin` - Arnold renderer
    ///
    /// # Returns
    ///
    /// true if successful, false otherwise
    pub fn set_renderer_plugin(&mut self, id: Token) -> bool {
        // Validate plugin ID format (should end with "RendererPlugin" or be empty for default)
        let id_str = id.as_str();
        if !id_str.is_empty() && !id_str.ends_with("RendererPlugin") {
            log::warn!(
                "Renderer plugin ID '{}' doesn't follow naming convention (*RendererPlugin)",
                id_str
            );
        }

        // If GPU is disabled, only CPU-capable renderers should be used
        if !self.gpu_enabled && !id_str.is_empty() {
            let cpu_renderers = ["HdEmbreeRendererPlugin", "HdPrmanLoaderRendererPlugin"];
            if !cpu_renderers.contains(&id_str) {
                log::warn!(
                    "GPU is disabled; renderer '{}' may not work without GPU",
                    id_str
                );
            }
        }

        self.renderer_plugin_id = id;
        true
    }

    /// Sets the path to the render pass prim to use.
    ///
    /// Note: If there is a render settings prim designated by the render pass
    /// prim via renderSource, it must also be set with
    /// `set_active_render_settings_prim_path()`.
    pub fn set_active_render_pass_prim_path(&mut self, path: Path) {
        self.render_pass_prim_path = Some(path);
    }

    /// Sets the path to the render settings prim to use.
    pub fn set_active_render_settings_prim_path(&mut self, path: Path) {
        self.render_settings_prim_path = Some(path);
    }

    /// Sets the width of the recorded image.
    ///
    /// The height of the recorded image will be computed using this value and
    /// the aspect ratio of the camera used for recording.
    pub fn set_image_width(&mut self, width: usize) {
        if width == 0 {
            eprintln!("CODING ERROR: Image width cannot be zero");
            return;
        }
        self.image_width = width;
    }

    /// Sets the level of refinement complexity.
    pub fn set_complexity(&mut self, complexity: f32) {
        self.complexity = complexity;
    }

    /// Sets the color correction mode to be used for recording.
    pub fn set_color_correction_mode(&mut self, mode: Token) {
        self.color_correction_mode = mode;
    }

    /// Turns the built-in camera light on or off.
    ///
    /// When on, this will add a light at the camera's origin.
    /// This is sometimes called a "headlight".
    pub fn set_camera_light_enabled(&mut self, enabled: bool) {
        self.camera_light_enabled = enabled;
    }

    /// Sets the camera visibility of dome lights.
    ///
    /// When on, dome light textures will be drawn to the background as if
    /// mapped onto a sphere infinitely far away.
    pub fn set_dome_light_visibility(&mut self, visible: bool) {
        self.dome_lights_visible = visible;
    }

    /// Sets the imageable purposes to be used for rendering.
    ///
    /// We will always include "default" purpose, and by default, we will also
    /// include "proxy". Use this method to explicitly enumerate an alternate
    /// set of purposes to be included along with "default".
    ///
    /// Only valid purpose tokens are accepted: "render", "proxy", "guide".
    /// "default" is always prepended regardless of the input list.
    pub fn set_included_purposes(&mut self, purposes: Vec<Token>) {
        // Valid non-default purposes matching C++ UsdGeomTokens.
        const VALID_PURPOSES: &[&str] = &["render", "proxy", "guide"];

        let default_tok = Token::new("default");

        // Filter to only valid purpose tokens, warn on unknowns.
        let mut result = vec![default_tok.clone()];
        for tok in purposes {
            if tok == default_tok {
                // Already included — skip duplicates.
                continue;
            }
            if VALID_PURPOSES.contains(&tok.as_str()) {
                result.push(tok);
            } else {
                log::warn!(
                    "set_included_purposes: ignoring unknown purpose token \"{}\"",
                    tok.as_str()
                );
            }
        }

        self.included_purposes = result;
    }

    /// Sets the primary camera prim path.
    pub fn set_primary_camera_prim_path(&mut self, path: Path) {
        self.primary_camera_path = Some(path);
    }

    /// Records an image and writes the result to output_image_path.
    ///
    /// The recorded image will represent the view from camera looking at
    /// the imageable prims on USD stage at time_code.
    ///
    /// If camera is not valid, a camera will be computed to automatically
    /// frame the stage geometry.
    ///
    /// When using a RenderSettings prim, the generated image will be written
    /// to the file indicated on the connected RenderProducts, instead of the
    /// given output_image_path. Note that in this case the given camera will
    /// later be overridden by the one authored on the RenderSettings Prim.
    ///
    /// # Returns
    ///
    /// true if the image was generated and written successfully, false otherwise
    ///
    /// # Arguments
    ///
    /// * `stage` - The USD stage to render
    /// * `camera` - The camera to render through
    /// * `time_code` - The time at which to evaluate the stage
    /// * `output_path` - Path to write the output image
    pub fn record(
        &self,
        stage: &Stage,
        camera: &Camera,
        time_code: TimeCode,
        output_path: impl Into<PathBuf>,
    ) -> Result<bool, String> {
        let mut recorder = usd_imaging::app_utils::FrameRecorder::new(
            if self.renderer_plugin_id.is_empty() {
                None
            } else {
                Some(self.renderer_plugin_id.clone())
            },
            self.gpu_enabled,
            self.enable_usd_draw_modes,
        );
        recorder.set_image_width(self.image_width);
        recorder.set_complexity(self.complexity);
        recorder.set_color_correction_mode(self.color_correction_mode.clone());
        recorder.set_camera_light_enabled(self.camera_light_enabled);
        recorder.set_dome_light_visibility(self.dome_lights_visible);
        recorder.set_included_purposes(self.included_purposes.clone());

        if let Some(path) = &self.primary_camera_path {
            recorder.set_primary_camera_prim_path(path.clone());
        }
        if let Some(path) = &self.render_pass_prim_path {
            recorder.set_active_render_pass_prim_path(path.clone());
        }
        if let Some(path) = &self.render_settings_prim_path {
            recorder.set_active_render_settings_prim_path(path.clone());
        }

        let output_path = output_path.into();
        let output_path = output_path
            .to_str()
            .ok_or_else(|| "output path is not valid UTF-8".to_string())?;
        Ok(recorder.record(stage, camera, time_code, output_path))
    }

    /// Gets the image width.
    pub fn image_width(&self) -> usize {
        self.image_width
    }

    /// Gets the complexity.
    pub fn complexity(&self) -> f32 {
        self.complexity
    }

    /// Gets whether GPU is enabled.
    pub fn gpu_enabled(&self) -> bool {
        self.gpu_enabled
    }

    /// Gets whether camera light is enabled.
    pub fn camera_light_enabled(&self) -> bool {
        self.camera_light_enabled
    }

    /// Gets whether dome lights are visible.
    pub fn dome_lights_visible(&self) -> bool {
        self.dome_lights_visible
    }
}

// ============================================================================
// FrameRecorderBuilder
// ============================================================================

/// Builder for FrameRecorder.
#[derive(Debug, Clone)]
pub struct FrameRecorderBuilder {
    renderer_plugin_id: Token,
    image_width: usize,
    complexity: f32,
    color_correction_mode: Token,
    gpu_enabled: bool,
    enable_usd_draw_modes: bool,
    camera_light_enabled: bool,
    dome_lights_visible: bool,
    included_purposes: Vec<Token>,
    primary_camera_path: Option<Path>,
    render_pass_prim_path: Option<Path>,
    render_settings_prim_path: Option<Path>,
}

impl Default for FrameRecorderBuilder {
    fn default() -> Self {
        Self {
            renderer_plugin_id: Token::new(""),
            image_width: 960,
            complexity: 1.0,
            color_correction_mode: Token::new("disabled"),
            gpu_enabled: true,
            enable_usd_draw_modes: true,
            camera_light_enabled: true,
            dome_lights_visible: false,
            included_purposes: vec![Token::new("default"), Token::new("proxy")],
            primary_camera_path: None,
            render_pass_prim_path: None,
            render_settings_prim_path: None,
        }
    }
}

impl FrameRecorderBuilder {
    /// Sets the renderer plugin ID.
    pub fn renderer_plugin_id(mut self, id: impl Into<String>) -> Self {
        self.renderer_plugin_id = Token::new(&id.into());
        self
    }

    /// Sets the image width.
    pub fn image_width(mut self, width: usize) -> Self {
        self.image_width = width;
        self
    }

    /// Sets the complexity.
    pub fn complexity(mut self, complexity: f32) -> Self {
        self.complexity = complexity;
        self
    }

    /// Sets the color correction mode.
    pub fn color_correction_mode(mut self, mode: impl Into<String>) -> Self {
        self.color_correction_mode = Token::new(&mode.into());
        self
    }

    /// Sets whether GPU rendering is enabled.
    pub fn gpu_enabled(mut self, enabled: bool) -> Self {
        self.gpu_enabled = enabled;
        self
    }

    /// Sets whether USD draw modes are enabled.
    pub fn enable_usd_draw_modes(mut self, enabled: bool) -> Self {
        self.enable_usd_draw_modes = enabled;
        self
    }

    /// Sets whether camera light is enabled.
    pub fn camera_light_enabled(mut self, enabled: bool) -> Self {
        self.camera_light_enabled = enabled;
        self
    }

    /// Sets whether dome lights are visible.
    pub fn dome_lights_visible(mut self, visible: bool) -> Self {
        self.dome_lights_visible = visible;
        self
    }

    /// Sets the included purposes.
    pub fn included_purposes(mut self, purposes: Vec<Token>) -> Self {
        self.included_purposes = purposes;
        self
    }

    /// Sets the primary camera path.
    pub fn primary_camera_path(mut self, path: Path) -> Self {
        self.primary_camera_path = Some(path);
        self
    }

    /// Sets the render pass prim path.
    pub fn render_pass_prim_path(mut self, path: Path) -> Self {
        self.render_pass_prim_path = Some(path);
        self
    }

    /// Sets the render settings prim path.
    pub fn render_settings_prim_path(mut self, path: Path) -> Self {
        self.render_settings_prim_path = Some(path);
        self
    }

    /// Builds the FrameRecorder.
    pub fn build(self) -> Result<FrameRecorder, String> {
        if self.image_width == 0 {
            return Err("Image width cannot be zero".to_string());
        }

        Ok(FrameRecorder {
            renderer_plugin_id: self.renderer_plugin_id,
            image_width: self.image_width,
            complexity: self.complexity,
            color_correction_mode: self.color_correction_mode,
            gpu_enabled: self.gpu_enabled,
            enable_usd_draw_modes: self.enable_usd_draw_modes,
            camera_light_enabled: self.camera_light_enabled,
            dome_lights_visible: self.dome_lights_visible,
            included_purposes: self.included_purposes,
            primary_camera_path: self.primary_camera_path,
            render_pass_prim_path: self.render_pass_prim_path,
            render_settings_prim_path: self.render_settings_prim_path,
        })
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_recorder_builder_default() {
        let recorder = FrameRecorder::builder().build().unwrap();

        assert_eq!(recorder.image_width(), 960);
        assert_eq!(recorder.complexity(), 1.0);
        assert!(recorder.gpu_enabled());
        assert!(recorder.camera_light_enabled());
        assert!(!recorder.dome_lights_visible());
    }

    #[test]
    fn test_frame_recorder_builder_custom() {
        let recorder = FrameRecorder::builder()
            .image_width(1920)
            .complexity(2.0)
            .gpu_enabled(false)
            .camera_light_enabled(false)
            .dome_lights_visible(true)
            .build()
            .unwrap();

        assert_eq!(recorder.image_width(), 1920);
        assert_eq!(recorder.complexity(), 2.0);
        assert!(!recorder.gpu_enabled());
        assert!(!recorder.camera_light_enabled());
        assert!(recorder.dome_lights_visible());
    }

    #[test]
    fn test_frame_recorder_zero_width() {
        let result = FrameRecorder::builder().image_width(0).build();

        assert!(result.is_err());
    }

    #[test]
    fn test_frame_recorder_setters() {
        let mut recorder = FrameRecorder::builder().build().unwrap();

        recorder.set_image_width(2048);
        assert_eq!(recorder.image_width(), 2048);

        recorder.set_complexity(3.0);
        assert_eq!(recorder.complexity(), 3.0);

        recorder.set_camera_light_enabled(false);
        assert!(!recorder.camera_light_enabled());

        recorder.set_dome_light_visibility(true);
        assert!(recorder.dome_lights_visible());
    }

    #[test]
    fn test_frame_recorder_renderer_plugin() {
        let mut recorder = FrameRecorder::builder()
            .renderer_plugin_id("HdStormRendererPlugin")
            .build()
            .unwrap();

        assert_eq!(
            recorder.get_current_renderer_id().as_str(),
            "HdStormRendererPlugin"
        );

        let success = recorder.set_renderer_plugin(Token::new("HdPrmanLoaderRendererPlugin"));
        assert!(success);
        assert_eq!(
            recorder.get_current_renderer_id().as_str(),
            "HdPrmanLoaderRendererPlugin"
        );
    }
}
