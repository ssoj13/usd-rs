//! Rendering parameters for UsdImagingGL.
//!
//! This module defines the rendering parameters used by the UsdImagingGL engine.

use usd_core::TimeCode;
use usd_gf::{BBox3d, Vec4d, Vec4f};
use usd_tf::Token;

/// Draw mode for rendering geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub enum DrawMode {
    /// Draw as points
    Points,
    /// Draw as wireframe
    Wireframe,
    /// Draw wireframe on top of surface
    WireframeOnSurface,
    /// Draw shaded with flat shading
    ShadedFlat,
    /// Draw shaded with smooth shading
    ShadedSmooth,
    /// Draw geometry only (no materials)
    GeomOnly,
    /// Draw geometry with flat shading
    GeomFlat,
    /// Draw geometry with smooth shading
    GeomSmooth,
}

impl Default for DrawMode {
    fn default() -> Self {
        Self::ShadedSmooth
    }
}

/// Culling style for backface culling.
///
/// Note: Some assumptions are made about the order of these enums in the C++ API,
/// so the order is preserved for compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub enum CullStyle {
    /// No opinion on culling
    NoOpinion,
    /// Don't cull anything
    Nothing,
    /// Cull back faces
    Back,
    /// Cull front faces
    Front,
    /// Cull back faces unless double-sided
    BackUnlessDoubleSided,
}

impl Default for CullStyle {
    fn default() -> Self {
        Self::Nothing
    }
}

/// Rendering parameters used as arguments for UsdImagingGLEngine methods.
///
/// This struct contains all the parameters that control how USD scenes are rendered,
/// including draw mode, lighting, materials, culling, and various rendering options.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderParams {
    /// The time code for which to render the scene
    pub frame: TimeCode,

    /// Complexity level for tessellation (1.0 = default)
    pub complexity: f32,

    /// Draw mode for geometry rendering
    pub draw_mode: DrawMode,

    /// Show guide geometry
    pub show_guides: bool,

    /// Show proxy geometry
    pub show_proxy: bool,

    /// Show render geometry
    pub show_render: bool,

    /// Force a complete refresh of the render
    pub force_refresh: bool,

    /// Flip front-facing direction
    pub flip_front_facing: bool,

    /// Backface culling style
    pub cull_style: CullStyle,

    /// Enable scene lighting
    pub enable_lighting: bool,

    /// Enable sample alpha to coverage
    pub enable_sample_alpha_to_coverage: bool,

    /// Apply render state
    pub apply_render_state: bool,

    /// Gamma correct colors
    pub gamma_correct_colors: bool,

    /// Highlight selected prims
    pub highlight: bool,

    /// Override color (if w > 0)
    pub override_color: Vec4f,

    /// Wireframe color (if w > 0)
    pub wireframe_color: Vec4f,

    /// Alpha threshold for transparency (< 0 implies automatic)
    pub alpha_threshold: f32,

    /// Clipping planes in camera space
    pub clip_planes: Vec<Vec4d>,

    /// Enable scene materials
    pub enable_scene_materials: bool,

    /// Enable scene lights
    pub enable_scene_lights: bool,

    /// Enable dome light IBL even when no scene dome light exists.
    /// When true and no scene dome light is found, a procedural sky fallback
    /// is generated for image-based lighting.
    pub dome_light_enabled: bool,

    /// Show dome light texture as background (sky dome).
    pub dome_light_textures_visible: bool,

    /// Optional HDRI file path for fallback dome light.
    /// When set and dome_light_enabled=true but no scene dome light, this file is loaded.
    pub dome_light_texture_path: Option<String>,

    /// Respect USD's model:drawMode attribute
    pub enable_usd_draw_modes: bool,

    /// Clear color for the viewport
    pub clear_color: Vec4f,

    /// Color correction mode token
    pub color_correction_mode: Token,

    /// LUT 3D size for OCIO (only valid when color_correction_mode is openColorIO)
    pub lut3d_size_ocio: i32,

    /// OCIO display name
    pub ocio_display: Token,

    /// OCIO view name
    pub ocio_view: Token,

    /// OCIO color space name
    pub ocio_color_space: Token,

    /// OCIO look name
    pub ocio_look: Token,

    /// Bounding boxes to render
    pub bboxes: Vec<BBox3d>,

    /// Bounding box line color
    pub bbox_line_color: Vec4f,

    /// Bounding box line dash size
    pub bbox_line_dash_size: f32,

    /// Default material ambient intensity [0..1].
    /// Scales the ambient light contribution when no scene material is bound.
    pub default_material_ambient: f32,

    /// Default material specular intensity [0..1].
    /// Controls specular highlight strength of the default material.
    pub default_material_specular: f32,

    /// Depth-only pass: write depth buffer but no color output.
    /// Used for HiddenSurfaceWireframe depth prepass.
    pub depth_only: bool,

    /// Preserve depth from a previous pass instead of clearing.
    /// Set for the wireframe pass of HiddenSurfaceWireframe so edges are
    /// occluded by the depth prepass geometry.
    pub preserve_depth: bool,

    /// Render tags controlling which prim categories to render.
    /// Always includes "geometry"; optionally "guide", "proxy", "render".
    pub render_tags: Vec<Token>,
}

impl Default for RenderParams {
    fn default() -> Self {
        Self {
            frame: TimeCode::earliest_time(),
            complexity: 1.0,
            draw_mode: DrawMode::default(),
            show_guides: false,
            show_proxy: true,
            show_render: false,
            force_refresh: false,
            flip_front_facing: false,
            cull_style: CullStyle::default(),
            enable_lighting: true,
            enable_sample_alpha_to_coverage: false,
            apply_render_state: true,
            gamma_correct_colors: true,
            highlight: false,
            override_color: Vec4f::new(0.0, 0.0, 0.0, 0.0),
            wireframe_color: Vec4f::new(0.0, 0.0, 0.0, 0.0),
            alpha_threshold: -1.0,
            clip_planes: Vec::new(),
            enable_scene_materials: true,
            enable_scene_lights: true,
            dome_light_enabled: false,
            dome_light_textures_visible: true,
            dome_light_texture_path: None,
            enable_usd_draw_modes: true,
            clear_color: Vec4f::new(0.0, 0.0, 0.0, 1.0),
            color_correction_mode: Token::new(""),
            lut3d_size_ocio: 65,
            ocio_display: Token::new(""),
            ocio_view: Token::new(""),
            ocio_color_space: Token::new(""),
            ocio_look: Token::new(""),
            bboxes: Vec::new(),
            bbox_line_color: Vec4f::new(1.0, 1.0, 1.0, 1.0),
            bbox_line_dash_size: 3.0,
            default_material_ambient: 0.2,
            default_material_specular: 0.1,
            depth_only: false,
            preserve_depth: false,
            render_tags: Vec::new(),
        }
    }
}

impl RenderParams {
    /// Creates a new RenderParams with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the frame time.
    pub fn with_frame(mut self, frame: TimeCode) -> Self {
        self.frame = frame;
        self
    }

    /// Sets the complexity level.
    pub fn with_complexity(mut self, complexity: f32) -> Self {
        self.complexity = complexity;
        self
    }

    /// Sets the draw mode.
    pub fn with_draw_mode(mut self, draw_mode: DrawMode) -> Self {
        self.draw_mode = draw_mode;
        self
    }

    /// Sets whether to enable lighting.
    pub fn with_lighting(mut self, enable: bool) -> Self {
        self.enable_lighting = enable;
        self
    }

    /// Sets whether to enable scene materials.
    pub fn with_scene_materials(mut self, enable: bool) -> Self {
        self.enable_scene_materials = enable;
        self
    }

    /// Sets whether to enable scene lights.
    pub fn with_scene_lights(mut self, enable: bool) -> Self {
        self.enable_scene_lights = enable;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_draw_mode_default() {
        assert_eq!(DrawMode::default(), DrawMode::ShadedSmooth);
    }

    #[test]
    fn test_cull_style_default() {
        assert_eq!(CullStyle::default(), CullStyle::Nothing);
    }

    #[test]
    fn test_render_params_default() {
        let params = RenderParams::default();
        assert_eq!(params.complexity, 1.0);
        assert_eq!(params.draw_mode, DrawMode::ShadedSmooth);
        assert!(!params.show_guides);
        assert!(params.show_proxy);
        assert!(params.enable_lighting);
        assert!(params.enable_scene_materials);
        assert!(params.enable_scene_lights);
    }

    #[test]
    fn test_render_params_builder() {
        let params = RenderParams::new()
            .with_complexity(2.0)
            .with_draw_mode(DrawMode::Wireframe)
            .with_lighting(false);

        assert_eq!(params.complexity, 2.0);
        assert_eq!(params.draw_mode, DrawMode::Wireframe);
        assert!(!params.enable_lighting);
    }
}
