//! Simple light task - Lighting setup and management.
//!
//! Collects lights from the render index and configures the lighting
//! context for rendering. Also handles shadow matrix computation.
//!
//! Port of pxr/imaging/hdx/simpleLightTask.h/cpp

use std::collections::HashMap;

use usd_camera_util::{CameraUtilConformWindowPolicy, CameraUtilFraming};
use usd_gf::{Matrix4d, Vec4f};
use usd_glf::simple_light::GlfSimpleLight;
use usd_glf::simple_material::GlfSimpleMaterial;
use usd_hd::prim::HdSceneDelegate;
use usd_hd::render::{HdRenderIndexTrait, HdTask, HdTaskContext};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Value;

/// Simple light task parameters.
///
/// Port of HdxSimpleLightTaskParams from pxr/imaging/hdx/simpleLightTask.h
#[derive(Debug, Clone, PartialEq)]
pub struct HdxSimpleLightTaskParams {
    /// Path to the camera for shadow computation.
    pub camera_path: Path,

    /// Paths to include when gathering lights.
    pub light_include_paths: Vec<Path>,

    /// Paths to exclude when gathering lights.
    pub light_exclude_paths: Vec<Path>,

    /// Whether shadows are enabled.
    pub enable_shadows: bool,

    /// Viewport for rendering.
    pub viewport: Vec4f,

    /// Camera framing information.
    pub framing: CameraUtilFraming,

    /// Override window policy (enabled, policy).
    pub override_window_policy: (bool, CameraUtilConformWindowPolicy),

    /// Material for compatibility.
    pub material: GlfSimpleMaterial,

    /// Scene ambient light color.
    pub scene_ambient: Vec4f,
}

impl Default for HdxSimpleLightTaskParams {
    fn default() -> Self {
        Self {
            camera_path: Path::empty(),
            light_include_paths: vec![Path::absolute_root()],
            light_exclude_paths: Vec::new(),
            enable_shadows: false,
            viewport: Vec4f::new(0.0, 0.0, 0.0, 0.0),
            framing: CameraUtilFraming::default(),
            override_window_policy: (false, CameraUtilConformWindowPolicy::Fit),
            material: GlfSimpleMaterial::default(),
            scene_ambient: Vec4f::new(0.0, 0.0, 0.0, 0.0),
        }
    }
}

/// Shadow parameters for a light.
///
/// Port of HdxShadowParams from pxr/imaging/hdx/simpleLightTask.h
#[derive(Debug, Clone, PartialEq)]
pub struct HdxShadowParams {
    /// Shadow bias to prevent self-shadowing.
    pub bias: f64,

    /// Shadow blur amount.
    pub blur: f64,

    /// Shadow map resolution.
    pub resolution: i32,

    /// Whether shadows are enabled for this light.
    pub enabled: bool,
}

impl Default for HdxShadowParams {
    fn default() -> Self {
        Self {
            bias: 0.0,
            blur: 0.0,
            resolution: 0,
            enabled: false,
        }
    }
}

impl std::fmt::Display for HdxShadowParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ShadowParams(bias={}, blur={}, resolution={}, enabled={})",
            self.bias, self.blur, self.resolution, self.enabled
        )
    }
}

/// Light types queried from the render index.
const LIGHT_TYPE_TOKENS: &[&str] = &[
    "domeLight",
    "simpleLight",
    "sphereLight",
    "rectLight",
    "diskLight",
    "cylinderLight",
    "distantLight",
];

/// Maximum number of lights supported.
const MAX_LIGHTS: usize = 16;

/// Simple lighting task.
///
/// Collects lights from the scene, computes shadow matrices, and
/// configures the lighting context for subsequent render tasks.
///
/// Port of HdxSimpleLightTask from pxr/imaging/hdx/simpleLightTask.h
#[allow(dead_code)]
pub struct HdxSimpleLightTask {
    /// Task path.
    id: Path,

    /// Camera path for shadow computation.
    camera_id: Path,

    /// Light paths by type.
    light_ids: HashMap<Token, Vec<Path>>,

    /// Paths to include when gathering lights.
    light_include_paths: Vec<Path>,

    /// Paths to exclude when gathering lights.
    light_exclude_paths: Vec<Path>,

    /// Number of light IDs.
    num_light_ids: usize,

    /// Whether shadows are enabled.
    enable_shadows: bool,

    /// Viewport for rendering.
    viewport: Vec4f,

    /// Camera framing.
    framing: CameraUtilFraming,

    /// Override window policy.
    override_window_policy: (bool, CameraUtilConformWindowPolicy),

    /// Material for compatibility.
    material: GlfSimpleMaterial,

    /// Scene ambient color.
    scene_ambient: Vec4f,

    /// Collected GlfSimpleLight data (synced from render index sprims).
    glf_simple_lights: Vec<GlfSimpleLight>,

    /// Sprim index version for change tracking.
    sprim_index_version: u32,

    /// Settings version for change tracking.
    settings_version: u32,

    /// Whether to rebuild lighting buffer sources.
    rebuild_lighting_buffer_sources: bool,

    /// Whether to rebuild light and shadow buffer sources.
    rebuild_light_and_shadow_buffer_sources: bool,

    /// Whether to rebuild material buffer sources.
    rebuild_material_buffer_sources: bool,
}

impl HdxSimpleLightTask {
    /// Create new simple light task.
    pub fn new(id: Path) -> Self {
        Self {
            id,
            camera_id: Path::empty(),
            light_ids: HashMap::new(),
            light_include_paths: vec![Path::absolute_root()],
            light_exclude_paths: Vec::new(),
            num_light_ids: 0,
            enable_shadows: false,
            viewport: Vec4f::new(0.0, 0.0, 0.0, 0.0),
            framing: CameraUtilFraming::default(),
            override_window_policy: (false, CameraUtilConformWindowPolicy::Fit),
            material: GlfSimpleMaterial::default(),
            scene_ambient: Vec4f::new(0.0, 0.0, 0.0, 0.0),
            glf_simple_lights: Vec::new(),
            sprim_index_version: 0,
            settings_version: 0,
            rebuild_lighting_buffer_sources: true,
            rebuild_light_and_shadow_buffer_sources: true,
            rebuild_material_buffer_sources: true,
        }
    }

    /// Set task parameters.
    pub fn set_params(&mut self, params: &HdxSimpleLightTaskParams) {
        self.camera_id = params.camera_path.clone();
        self.light_include_paths = params.light_include_paths.clone();
        self.light_exclude_paths = params.light_exclude_paths.clone();
        self.enable_shadows = params.enable_shadows;
        self.viewport = params.viewport;
        self.framing = params.framing.clone();
        self.override_window_policy = params.override_window_policy;
        self.material = params.material.clone();
        self.scene_ambient = params.scene_ambient;
        self.rebuild_material_buffer_sources = true;
    }

    /// Get current task parameters.
    pub fn get_params(&self) -> HdxSimpleLightTaskParams {
        HdxSimpleLightTaskParams {
            camera_path: self.camera_id.clone(),
            light_include_paths: self.light_include_paths.clone(),
            light_exclude_paths: self.light_exclude_paths.clone(),
            enable_shadows: self.enable_shadows,
            viewport: self.viewport,
            framing: self.framing.clone(),
            override_window_policy: self.override_window_policy,
            material: self.material.clone(),
            scene_ambient: self.scene_ambient,
        }
    }

    /// Get the camera path.
    pub fn get_camera_id(&self) -> &Path {
        &self.camera_id
    }

    /// Get whether shadows are enabled.
    pub fn is_shadows_enabled(&self) -> bool {
        self.enable_shadows
    }

    /// Get the collected GlfSimpleLight list.
    pub fn get_lights(&self) -> &[GlfSimpleLight] {
        &self.glf_simple_lights
    }

    /// Get the maximum number of lights supported.
    pub fn get_max_lights(&self) -> usize {
        MAX_LIGHTS
    }

    /// Mark lighting buffers as dirty, forcing a rebuild on next sync/prepare.
    ///
    /// Called by task controller when lighting state changes (e.g. new context).
    pub fn mark_lighting_dirty(&mut self) {
        self.rebuild_lighting_buffer_sources = true;
        self.rebuild_light_and_shadow_buffer_sources = true;
    }

    /// Set lights directly (used by engine after collecting from render index).
    ///
    /// Only lights with intensity are accepted; list is capped at MAX_LIGHTS.
    pub fn set_lights(&mut self, lights: Vec<GlfSimpleLight>) {
        self.glf_simple_lights = lights
            .into_iter()
            .filter(|l| l.has_intensity())
            .take(MAX_LIGHTS)
            .collect();
        self.rebuild_lighting_buffer_sources = true;
        self.rebuild_light_and_shadow_buffer_sources = true;
    }

    /// Compute shadow matrices for a light.
    #[allow(dead_code)]
    fn compute_shadow_matrices(&self, _camera_id: &Path) -> Vec<Matrix4d> {
        // In full impl: compute shadow matrices from camera and framing
        Vec::new()
    }

    /// Check whether a path is within the include/exclude filter.
    ///
    /// A light is included if its path is under one of light_include_paths
    /// and NOT under any light_exclude_paths.
    pub fn path_passes_filter(&self, path: &Path) -> bool {
        // Check exclude list first
        for excl in &self.light_exclude_paths {
            if path.has_prefix(excl) {
                return false;
            }
        }
        // Check include list
        for incl in &self.light_include_paths {
            if incl == &Path::absolute_root() || path.has_prefix(incl) {
                return true;
            }
        }
        false
    }
}

impl HdTask for HdxSimpleLightTask {
    fn id(&self) -> &Path {
        &self.id
    }

    fn sync(
        &mut self,
        _delegate: &dyn HdSceneDelegate,
        ctx: &mut HdTaskContext,
        dirty_bits: &mut u32,
    ) {
        // Store lighting shader in context for other tasks
        ctx.insert(
            Token::new("lightingShader"),
            Value::from(format!("HdxSimpleLightTask@{}", self.id.get_string())),
        );

        // Check if params changed
        let params_dirty = (*dirty_bits & 0x1) != 0;
        if params_dirty {
            self.rebuild_material_buffer_sources = true;
        }

        // Store lighting context in task context
        // The actual light data is stored in glf_simple_lights (set by engine or
        // by set_lights() external call). Publish the count so downstream tasks can read it.
        ctx.insert(
            Token::new("lightingContext"),
            Value::from(format!(
                "SimpleLightingContext(lights={})",
                self.glf_simple_lights.len()
            )),
        );

        *dirty_bits = 0;
    }

    fn prepare(&mut self, ctx: &mut HdTaskContext, _render_index: &dyn HdRenderIndexTrait) {
        // Publish the light count in the task context so downstream render tasks
        // can check whether lighting data is available.
        ctx.insert(
            Token::new("numLights"),
            Value::from(self.glf_simple_lights.len() as i32),
        );

        // Publish individual light params for inspection / debugging
        for (i, light) in self.glf_simple_lights.iter().enumerate().take(MAX_LIGHTS) {
            let key = format!("light{}_diffuse", i);
            let d = light.get_diffuse();
            // Store as string for debugging (Vec4f may not implement Hash for Value)
            let s = format!("{:.3},{:.3},{:.3},{:.3}", d.x, d.y, d.z, d.w);
            ctx.insert(Token::new(&key), Value::from(s));
        }

        // Reset dirty flags after prepare
        self.rebuild_lighting_buffer_sources = false;
        self.rebuild_light_and_shadow_buffer_sources = false;
    }

    fn execute(&mut self, _ctx: &mut HdTaskContext) {
        // C++ Execute() is intentionally empty (just HD_TRACE_FUNCTION).
        // All real work (light gathering, shadow setup, lighting context creation,
        // material buffer upload) happens in Sync(), not Execute().
    }

    fn get_render_tags(&self) -> &[Token] {
        &[]
    }

    fn is_converged(&self) -> bool {
        true
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl std::fmt::Display for HdxSimpleLightTaskParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SimpleLightTaskParams(camera={}, shadows={}, viewport={:?})",
            self.camera_path.get_string(),
            self.enable_shadows,
            self.viewport
        )
    }
}

/// Collect GlfSimpleLights from a Hydra render index.
///
/// Queries all light sprim types, downcasts handles to GlfSimpleLight params,
/// and returns the list filtered by intensity and capped at max_lights.
///
/// This is a free function because `HdRenderIndex` (struct) has `get_sprim_ids_for_type`
/// and `get_sprim`, while the `HdRenderIndex` trait exposed to tasks does not.
/// Engine calls this directly with its concrete render index.
///
/// # Parameters
/// - `light_type_tokens` - Token list of sprim type names to query
/// - `get_ids_for_type` - Callback: type token -> Vec<Path>
/// - `get_light_glf` - Callback: (type, path) -> Option<GlfSimpleLight>
/// - `max_lights` - Maximum lights to return
pub fn collect_glf_lights_from_callbacks(
    light_type_tokens: &[Token],
    mut get_ids_for_type: impl FnMut(&Token) -> Vec<Path>,
    mut get_light_glf: impl FnMut(&Token, &Path) -> Option<GlfSimpleLight>,
    max_lights: usize,
) -> Vec<GlfSimpleLight> {
    let mut result = Vec::new();

    for tok in light_type_tokens {
        let ids = get_ids_for_type(tok);
        for id in &ids {
            if result.len() >= max_lights {
                break;
            }
            if let Some(glf) = get_light_glf(tok, id) {
                if glf.has_intensity() {
                    result.push(glf);
                }
            }
        }
        if result.len() >= max_lights {
            break;
        }
    }

    result
}

/// Build the default light type token list used for querying sprims.
pub fn light_type_tokens() -> Vec<Token> {
    LIGHT_TYPE_TOKENS.iter().map(|s| Token::new(s)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_light_task_params_default() {
        let params = HdxSimpleLightTaskParams::default();
        assert!(!params.enable_shadows);
        assert_eq!(params.light_include_paths.len(), 1);
        assert!(params.light_exclude_paths.is_empty());
    }

    #[test]
    fn test_simple_light_task_creation() {
        let task = HdxSimpleLightTask::new(Path::from_string("/light").unwrap());
        assert!(!task.is_shadows_enabled());
        assert!(task.get_lights().is_empty());
        assert_eq!(task.get_max_lights(), MAX_LIGHTS);
    }

    #[test]
    fn test_simple_light_task_set_params() {
        let mut task = HdxSimpleLightTask::new(Path::from_string("/light").unwrap());

        let mut params = HdxSimpleLightTaskParams::default();
        params.camera_path = Path::from_string("/camera").unwrap();
        params.enable_shadows = true;
        params.viewport = Vec4f::new(0.0, 0.0, 1920.0, 1080.0);

        task.set_params(&params);

        assert_eq!(task.get_camera_id().get_string(), "/camera");
        assert!(task.is_shadows_enabled());
    }

    #[test]
    fn test_shadow_params_default() {
        let params = HdxShadowParams::default();
        assert_eq!(params.bias, 0.0);
        assert_eq!(params.blur, 0.0);
        assert_eq!(params.resolution, 0);
        assert!(!params.enabled);
    }

    #[test]
    fn test_shadow_params_display() {
        let params = HdxShadowParams {
            bias: 0.001,
            blur: 1.0,
            resolution: 1024,
            enabled: true,
        };
        let display = format!("{}", params);
        assert!(display.contains("ShadowParams"));
        assert!(display.contains("enabled=true"));
    }

    #[test]
    fn test_simple_light_task_params_display() {
        let params = HdxSimpleLightTaskParams::default();
        let display = format!("{}", params);
        assert!(display.contains("SimpleLightTaskParams"));
    }

    #[test]
    fn test_simple_light_task_params_equality() {
        let params1 = HdxSimpleLightTaskParams::default();
        let params2 = HdxSimpleLightTaskParams::default();
        assert_eq!(params1, params2);

        let mut params3 = HdxSimpleLightTaskParams::default();
        params3.enable_shadows = true;
        assert_ne!(params1, params3);
    }

    #[test]
    fn test_set_lights_filters_zero_intensity() {
        let mut task = HdxSimpleLightTask::new(Path::from_string("/light").unwrap());

        // Light with has_intensity = true
        let mut bright = GlfSimpleLight::default();
        bright.set_has_intensity(true);

        // Light with has_intensity = false
        let mut dark = GlfSimpleLight::default();
        dark.set_has_intensity(false);

        task.set_lights(vec![bright.clone(), dark]);

        // Only bright light should pass filter
        assert_eq!(task.get_lights().len(), 1);
    }

    #[test]
    fn test_set_lights_caps_at_max() {
        let mut task = HdxSimpleLightTask::new(Path::from_string("/light").unwrap());

        let mut lights = Vec::new();
        for _ in 0..MAX_LIGHTS + 5 {
            let mut l = GlfSimpleLight::default();
            l.set_has_intensity(true);
            lights.push(l);
        }

        task.set_lights(lights);
        assert_eq!(task.get_lights().len(), MAX_LIGHTS);
    }

    #[test]
    fn test_light_type_tokens() {
        let toks = light_type_tokens();
        assert!(toks.iter().any(|t| t == "domeLight"));
        assert!(toks.iter().any(|t| t == "distantLight"));
        assert!(toks.iter().any(|t| t == "sphereLight"));
    }

    #[test]
    fn test_collect_glf_lights_from_callbacks() {
        let types = light_type_tokens();

        // One distant light at /World/sun
        let sun_path = Path::from_string("/World/sun").unwrap();
        let mut sun = GlfSimpleLight::default();
        sun.set_has_intensity(true);
        sun.set_diffuse(usd_gf::Vec4f::new(1.0, 0.95, 0.9, 1.0));

        let lights = collect_glf_lights_from_callbacks(
            &types,
            |tok| {
                if tok == "distantLight" {
                    vec![sun_path.clone()]
                } else {
                    vec![]
                }
            },
            |_tok, _id| Some(sun.clone()),
            MAX_LIGHTS,
        );

        assert_eq!(lights.len(), 1);
        assert!(lights[0].has_intensity());
    }

    #[test]
    fn test_path_passes_filter_exclude() {
        let mut task = HdxSimpleLightTask::new(Path::from_string("/light").unwrap());
        task.light_exclude_paths = vec![Path::from_string("/hidden").unwrap()];

        let hidden = Path::from_string("/hidden/light1").unwrap();
        let visible = Path::from_string("/visible/light1").unwrap();

        assert!(!task.path_passes_filter(&hidden));
        assert!(task.path_passes_filter(&visible));
    }
}
