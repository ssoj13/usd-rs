
//! Pick from render buffer task - GPU/deferred picking from AOV render buffers.
//!
//! Reads pick IDs from previously rendered AOV buffers (primId, instanceId, elementId,
//! edgeId, pointId, normal, depth) rather than issuing a dedicated pick render. This is the reference
//! OpenUSD path used when the main render already produced the needed AOVs.
//! Port of pxr/imaging/hdx/pickFromRenderBufferTask.h/cpp

use std::collections::HashMap;

use usd_camera_util::{ConformWindowPolicy, Framing};
use usd_gf::{Matrix4d, Range2f, Rect2i, Vec2f, Vec2i, Vec2d, Vec3d};
use usd_hd::prim::camera::{CameraUtilConformWindowPolicy as HdCameraWindowPolicy, HdCamera};
use usd_hd::prim::HdSceneDelegate;
use usd_hd::render::render_index::SprimAdapter;
use usd_hd::render::{HdRenderIndexTrait, HdTask, HdTaskContext, TfTokenVector};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Value;

use super::pick_task::{
    pick_tokens, HdxPickHit, HdxPickResult, HdxPickTask, HdxPickTaskContextParams,
};
use super::render_setup_task::{CameraUtilConformWindowPolicy, CameraUtilFraming};

/// Pick from render buffer task parameters.
///
/// Port of HdxPickFromRenderBufferTaskParams from pxr/imaging/hdx/pickFromRenderBufferTask.h
#[derive(Debug, Clone)]
pub struct HdxPickFromRenderBufferTaskParams {
    /// Path to the primId AOV render buffer.
    pub prim_id_buffer_path: Path,
    /// Path to the instanceId AOV render buffer.
    pub instance_id_buffer_path: Path,
    /// Path to the elementId AOV render buffer.
    pub element_id_buffer_path: Path,
    /// Path to the edgeId AOV render buffer.
    pub edge_id_buffer_path: Path,
    /// Path to the pointId AOV render buffer.
    pub point_id_buffer_path: Path,
    /// Path to the normal AOV render buffer.
    pub normal_buffer_path: Path,
    /// Path to the depth AOV render buffer.
    pub depth_buffer_path: Path,
    /// Camera prim used for un-projecting depth to world space.
    pub camera_path: Path,
    /// Viewport dimensions [x, y, width, height].
    pub viewport: [i32; 4],
    /// Camera framing information for projection conforming.
    pub framing: CameraUtilFraming,
    /// Optional override window policy.
    pub override_window_policy: Option<CameraUtilConformWindowPolicy>,
}

impl Default for HdxPickFromRenderBufferTaskParams {
    fn default() -> Self {
        Self {
            prim_id_buffer_path: Path::empty(),
            instance_id_buffer_path: Path::empty(),
            element_id_buffer_path: Path::empty(),
            edge_id_buffer_path: Path::empty(),
            point_id_buffer_path: Path::empty(),
            normal_buffer_path: Path::empty(),
            depth_buffer_path: Path::empty(),
            camera_path: Path::empty(),
            viewport: [0, 0, 0, 0],
            framing: CameraUtilFraming::default(),
            override_window_policy: None,
        }
    }
}

/// Pick from render buffer task.
///
/// Reads pick data from AOV render buffers instead of performing a separate
/// pick render pass. Supports primId, instanceId, elementId, edgeId, pointId and depth buffers.
///
/// Port of HdxPickFromRenderBufferTask from pxr/imaging/hdx/pickFromRenderBufferTask.h
pub struct HdxPickFromRenderBufferTask {
    /// Task path.
    id: Path,
    /// Task parameters.
    params: HdxPickFromRenderBufferTaskParams,
    /// Render tags for filtering.
    render_tags: TfTokenVector,
    /// Pick results from last execution.
    pick_results: Vec<HdxPickHit>,
    /// Whether all required inputs were available and read back.
    converged: bool,
    /// Main render camera view matrix used to generate the AOVs.
    render_view_matrix: Matrix4d,
    /// Main render projection matrix used to generate the AOVs.
    render_projection_matrix: Matrix4d,
    /// Pick-frustum view matrix from task context.
    pick_view_matrix: Matrix4d,
    /// Pick-frustum projection matrix from task context.
    pick_projection_matrix: Matrix4d,
    /// Resolve mode for the current pick request.
    resolve_mode: Token,
    /// Pick target for the current pick request.
    pick_target: Token,
    /// Whether the source camera sprim was resolved in Prepare.
    has_render_camera: bool,
    /// Map primId -> render index rprim path for result decoration.
    prim_id_to_rprim_path: HashMap<i32, Path>,
}

impl HdxPickFromRenderBufferTask {
    /// Create new pick-from-render-buffer task.
    pub fn new(id: Path) -> Self {
        Self {
            id,
            params: HdxPickFromRenderBufferTaskParams::default(),
            render_tags: Vec::new(),
            pick_results: Vec::new(),
            converged: false,
            render_view_matrix: Matrix4d::identity(),
            render_projection_matrix: Matrix4d::identity(),
            pick_view_matrix: Matrix4d::identity(),
            pick_projection_matrix: Matrix4d::identity(),
            resolve_mode: pick_tokens::resolve_nearest_to_center(),
            pick_target: pick_tokens::pick_prims_and_instances(),
            has_render_camera: false,
            prim_id_to_rprim_path: HashMap::new(),
        }
    }

    /// Set task parameters.
    pub fn set_params(&mut self, params: HdxPickFromRenderBufferTaskParams) {
        self.params = params;
    }

    /// Get task parameters.
    pub fn get_params(&self) -> &HdxPickFromRenderBufferTaskParams {
        &self.params
    }

    /// Get pick results from last execution.
    pub fn get_pick_results(&self) -> &[HdxPickHit] {
        &self.pick_results
    }

    fn with_camera_from_render_index<T>(
        render_index: &dyn HdRenderIndexTrait,
        id: &Path,
        f: impl FnOnce(&HdCamera) -> T,
    ) -> Option<T> {
        let camera_token = Token::new("camera");
        let handle = render_index.get_sprim(&camera_token, id)?;
        if let Some(camera) = handle.downcast_ref::<HdCamera>() {
            return Some(f(camera));
        }
        handle
            .downcast_ref::<SprimAdapter<HdCamera>>()
            .map(|adapter| f(&adapter.0))
    }

    fn task_policy_to_hd(policy: CameraUtilConformWindowPolicy) -> HdCameraWindowPolicy {
        match policy {
            CameraUtilConformWindowPolicy::MatchVertically => HdCameraWindowPolicy::MatchVertically,
            CameraUtilConformWindowPolicy::MatchHorizontally => {
                HdCameraWindowPolicy::MatchHorizontally
            }
            CameraUtilConformWindowPolicy::Fit => HdCameraWindowPolicy::Fit,
            CameraUtilConformWindowPolicy::CropToFill => HdCameraWindowPolicy::Crop,
            CameraUtilConformWindowPolicy::DontConform => HdCameraWindowPolicy::DontConform,
        }
    }

    fn hd_policy_to_camera_util(policy: HdCameraWindowPolicy) -> ConformWindowPolicy {
        match policy {
            HdCameraWindowPolicy::MatchVertically => ConformWindowPolicy::MatchVertically,
            HdCameraWindowPolicy::MatchHorizontally => ConformWindowPolicy::MatchHorizontally,
            HdCameraWindowPolicy::Fit => ConformWindowPolicy::Fit,
            HdCameraWindowPolicy::Crop => ConformWindowPolicy::Crop,
            HdCameraWindowPolicy::DontConform => ConformWindowPolicy::DontConform,
        }
    }

    fn conform_projection(
        projection: &Matrix4d,
        policy: HdCameraWindowPolicy,
        target_aspect: f64,
    ) -> Matrix4d {
        if target_aspect <= 0.0 {
            return *projection;
        }
        let sx = projection[0][0];
        let sy = projection[1][1];
        if sx.abs() < 1e-12 || sy.abs() < 1e-12 {
            return *projection;
        }

        let projection_aspect = sx / sy;
        let ratio = projection_aspect / target_aspect;
        if (ratio - 1.0).abs() < 1e-12 {
            return *projection;
        }

        let mut result = *projection;
        match policy {
            HdCameraWindowPolicy::MatchVertically => {
                result[0][0] /= ratio;
            }
            HdCameraWindowPolicy::MatchHorizontally => {
                result[1][1] *= ratio;
            }
            HdCameraWindowPolicy::Fit => {
                if ratio > 1.0 {
                    result[1][1] *= ratio;
                } else {
                    result[0][0] /= ratio;
                }
            }
            HdCameraWindowPolicy::Crop => {
                if ratio > 1.0 {
                    result[0][0] /= ratio;
                } else {
                    result[1][1] *= ratio;
                }
            }
            HdCameraWindowPolicy::DontConform => {}
        }
        result
    }

    fn compute_projection_matrix(camera: &HdCamera, params: &HdxPickFromRenderBufferTaskParams) -> Matrix4d {
        let projection = camera.compute_projection_matrix();
        let effective_policy = params
            .override_window_policy
            .map(Self::task_policy_to_hd)
            .unwrap_or_else(|| camera.get_window_policy());

        if params.framing.is_valid() {
            let display_window = params.framing.display_window;
            let data_window = params.framing.data_window;
            let display = Range2f::new(
                Vec2f::new(display_window.0 as f32, display_window.1 as f32),
                Vec2f::new(display_window.2 as f32, display_window.3 as f32),
            );
            let data = Rect2i::new(
                Vec2i::new(data_window.0, data_window.1),
                Vec2i::new(data_window.2, data_window.3),
            );
            let framing = Framing::new(display, data, params.framing.pixel_aspect_ratio as f32);
            return framing.apply_to_projection_matrix(
                projection,
                Self::hd_policy_to_camera_util(effective_policy),
            );
        }

        let aspect = if params.viewport[3] != 0 {
            params.viewport[2] as f64 / params.viewport[3] as f64
        } else {
            1.0
        };
        Self::conform_projection(&projection, effective_policy, aspect)
    }

    fn compute_sub_rect(
        &self,
        render_buffer_size: Vec2i,
        pick_view: Matrix4d,
        pick_projection: Matrix4d,
    ) -> [i32; 4] {
        let mut render_buffer_xf = Matrix4d::identity();
        render_buffer_xf.set_scale_vec(&Vec3d::new(
            0.5 * render_buffer_size.x as f64,
            0.5 * render_buffer_size.y as f64,
            1.0,
        ));
        render_buffer_xf.set_translate_only(&Vec3d::new(
            0.5 * render_buffer_size.x as f64,
            0.5 * render_buffer_size.y as f64,
            0.0,
        ));

        let pick_to_render = (pick_view * pick_projection)
            .inverse()
            .unwrap_or_else(Matrix4d::identity)
            * self.render_view_matrix
            * self.render_projection_matrix
            * render_buffer_xf;

        let corner0 = pick_to_render.transform(&Vec3d::new(-1.0, -1.0, -1.0));
        let corner1 = pick_to_render.transform(&Vec3d::new(1.0, 1.0, -1.0));

        let pick_min = Vec2d::new(corner0[0].min(corner1[0]).floor(), corner0[1].min(corner1[1]).floor());
        let pick_max = Vec2d::new(corner0[0].max(corner1[0]).ceil(), corner0[1].max(corner1[1]).ceil());

        [
            pick_min[0] as i32,
            pick_min[1] as i32,
            (pick_max[0] - pick_min[0]) as i32,
            (pick_max[1] - pick_min[1]) as i32,
        ]
    }
}

impl HdTask for HdxPickFromRenderBufferTask {
    fn id(&self) -> &Path {
        &self.id
    }

    fn sync(
        &mut self,
        _delegate: &dyn HdSceneDelegate,
        ctx: &mut HdTaskContext,
        dirty_bits: &mut u32,
    ) {
        if let Some(context_params) = ctx
            .get(&pick_tokens::pick_params())
            .and_then(|value| value.get::<HdxPickTaskContextParams>())
        {
            self.pick_view_matrix = context_params.view_matrix;
            self.pick_projection_matrix = context_params.projection_matrix;
            self.resolve_mode = context_params.resolve_mode.clone();
            self.pick_target = context_params.pick_target.clone();
        }
        *dirty_bits = 0;
    }

    fn prepare(&mut self, ctx: &mut HdTaskContext, _render_index: &dyn HdRenderIndexTrait) {
        let render_index = _render_index;
        self.pick_results.clear();
        self.prim_id_to_rprim_path.clear();
        self.render_view_matrix = Matrix4d::identity();
        self.render_projection_matrix = Matrix4d::identity();
        self.has_render_camera = false;
        self.converged = false;

        for rprim_path in render_index.get_rprim_ids() {
            if let Some(prim_id) = render_index.get_prim_id_for_rprim_path(&rprim_path) {
                self.prim_id_to_rprim_path.insert(prim_id, rprim_path);
            }
        }

        if let Some((view_matrix, projection_matrix)) =
            Self::with_camera_from_render_index(render_index, &self.params.camera_path, |camera| {
                (
                    camera.get_view_matrix(),
                    Self::compute_projection_matrix(camera, &self.params),
                )
            })
        {
            self.render_view_matrix = view_matrix;
            self.render_projection_matrix = projection_matrix;
            self.has_render_camera = true;
        }

        ctx.insert(
            Token::new("pickFromRenderBufferPrepared"),
            Value::from(true),
        );
    }

    fn execute(&mut self, ctx: &mut HdTaskContext) {
        self.pick_results.clear();
        self.converged = true;

        if self.params.prim_id_buffer_path.is_empty()
            || self.params.depth_buffer_path.is_empty()
            || self.params.camera_path.is_empty()
            || !self.has_render_camera
        {
            return;
        }

        let Some(hgi_driver) = HdxPickTask::get_hgi_driver(ctx) else {
            self.converged = false;
            return;
        };

        let Some(prim_texture) = HdxPickTask::get_aov_texture(ctx, &Token::new("primId")) else {
            self.converged = false;
            return;
        };
        let Some(depth_texture) = HdxPickTask::get_aov_texture(ctx, &Token::new("depth")) else {
            self.converged = false;
            return;
        };

        let buffer_size = prim_texture
            .get()
            .map(|texture| Vec2i::new(texture.descriptor().dimensions.x, texture.descriptor().dimensions.y))
            .unwrap_or_else(|| Vec2i::new(0, 0));
        if buffer_size.x <= 0 || buffer_size.y <= 0 {
            self.converged = false;
            return;
        }

        let Some(depth_desc) = depth_texture.get().map(|texture| texture.descriptor().clone()) else {
            self.converged = false;
            return;
        };
        if depth_desc.dimensions.x != buffer_size.x || depth_desc.dimensions.y != buffer_size.y {
            self.converged = false;
            return;
        }

        let prim_ids = HdxPickTask::read_aov_i32(ctx, &hgi_driver, &Token::new("primId"))
            .unwrap_or_default();
        let depths = HdxPickTask::read_aov_f32(ctx, &hgi_driver, &Token::new("depth"))
            .unwrap_or_default();
        if prim_ids.is_empty() || depths.is_empty() || prim_ids.len() != depths.len() {
            self.converged = false;
            return;
        }

        let instance_ids = if self.params.instance_id_buffer_path.is_empty() {
            Vec::new()
        } else {
            HdxPickTask::read_aov_i32(ctx, &hgi_driver, &Token::new("instanceId")).unwrap_or_default()
        };
        if !instance_ids.is_empty() && instance_ids.len() != prim_ids.len() {
            self.converged = false;
            return;
        }
        let element_ids = if self.params.element_id_buffer_path.is_empty() {
            Vec::new()
        } else {
            HdxPickTask::read_aov_i32(ctx, &hgi_driver, &Token::new("elementId")).unwrap_or_default()
        };
        if !element_ids.is_empty() && element_ids.len() != prim_ids.len() {
            self.converged = false;
            return;
        }
        let edge_ids = if self.params.edge_id_buffer_path.is_empty() {
            Vec::new()
        } else {
            HdxPickTask::read_aov_i32(ctx, &hgi_driver, &Token::new("edgeId")).unwrap_or_default()
        };
        if !edge_ids.is_empty() && edge_ids.len() != prim_ids.len() {
            self.converged = false;
            return;
        }
        let point_ids = if self.params.point_id_buffer_path.is_empty() {
            Vec::new()
        } else {
            HdxPickTask::read_aov_i32(ctx, &hgi_driver, &Token::new("pointId"))
                .unwrap_or_default()
        };
        if !point_ids.is_empty() && point_ids.len() != prim_ids.len() {
            self.converged = false;
            return;
        }
        let neyes = if self.params.normal_buffer_path.is_empty() {
            Vec::new()
        } else {
            HdxPickTask::read_aov_i32(ctx, &hgi_driver, &Token::new("normal"))
                .or_else(|| HdxPickTask::read_aov_i32(ctx, &hgi_driver, &Token::new("Neye")))
                .unwrap_or_default()
        };
        if !neyes.is_empty() && neyes.len() != prim_ids.len() {
            self.converged = false;
            return;
        }

        let sub_rect = self.compute_sub_rect(
            buffer_size,
            self.pick_view_matrix,
            self.pick_projection_matrix,
        );
        let result = HdxPickResult::new(
            prim_ids,
            depths,
            buffer_size,
            self.pick_target.clone(),
            self.render_view_matrix,
            self.render_projection_matrix,
        )
        .with_all_ids(instance_ids, element_ids, edge_ids, point_ids)
        .with_neyes(neyes)
        .with_depth_range(0.0, 1.0)
        .with_sub_rect(sub_rect[0], sub_rect[1], sub_rect[2], sub_rect[3]);

        if self.resolve_mode == pick_tokens::resolve_nearest_to_camera() {
            result.resolve_nearest_to_camera(&mut self.pick_results);
        } else if self.resolve_mode == pick_tokens::resolve_unique() {
            result.resolve_unique(&mut self.pick_results);
        } else if self.resolve_mode == pick_tokens::resolve_all() {
            result.resolve_all(&mut self.pick_results);
        } else {
            result.resolve_nearest_to_center(&mut self.pick_results);
        }
        let prim_map = &self.prim_id_to_rprim_path;
        for hit in &mut self.pick_results {
            if let Some(path) = prim_map.get(&hit.prim_id) {
                hit.object_id = path.clone();
            }
        }
    }

    fn get_render_tags(&self) -> &[Token] {
        &self.render_tags
    }

    fn is_converged(&self) -> bool {
        self.converged
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pick_from_buffer_task() {
        let task = HdxPickFromRenderBufferTask::new(Path::from_string("/pickBuffer").unwrap());
        assert!(task.get_pick_results().is_empty());
    }

    #[test]
    fn test_params_default() {
        let params = HdxPickFromRenderBufferTaskParams::default();
        assert!(params.prim_id_buffer_path.is_empty());
        assert!(params.edge_id_buffer_path.is_empty());
        assert!(params.point_id_buffer_path.is_empty());
        assert!(params.normal_buffer_path.is_empty());
        assert_eq!(params.viewport, [0, 0, 0, 0]);
    }
}
