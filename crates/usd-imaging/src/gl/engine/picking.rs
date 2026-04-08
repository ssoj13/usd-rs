//! Picking and intersection submodule for Engine.
//!
//! Contains all CPU and GPU picking logic: ray-mesh tests, GPU ID pass,
//! readback utilities, and free helper functions.

// Engine, IntersectionResult, PickParams are defined in the parent engine.rs (super).
// RenderParams lives in gl::render_params, re-exported at gl level (super::super).
use super::super::RenderParams;
use super::{Engine, IntersectionResult, PickParams};

use usd_gf::{Matrix4d, Vec3d, Vec4f};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Value;

#[cfg(feature = "wgpu")]
use half::f16;
#[cfg(feature = "wgpu")]
use usd_gf::Vec2i;
#[cfg(feature = "wgpu")]
use usd_hd_st::render_pass_state::HdStAovBinding;
#[cfg(feature = "wgpu")]
use usd_hdx::{
    HdxPickFromRenderBufferTask, HdxPickHit, HdxPickTask, HdxPickTaskContextParams,
    HdxPickTaskRequest, HdxRenderTaskRequest, pick_tokens,
};
#[cfg(feature = "wgpu")]
use usd_hgi::{
    HgiBufferDesc, HgiBufferUsage,
    blit_cmds::{HgiTextureGpuToCpuOp, RawCpuBufferMut},
    enums::HgiSubmitWaitType as WgpuSubmitWait,
    hgi::Hgi,
    texture::HgiTextureHandle,
    types::HgiFormat,
};
#[cfg(feature = "wgpu")]
use usd_hgi::{enums::HgiTextureUsage, texture::HgiTextureDesc};

use usd_core::Prim;
use usd_hd::prim::HdSceneDelegate;
use usd_hd::render::HdRenderPass;

impl Engine {
    // -------------------------------------------------------------------------
    // Picking — impl methods
    // -------------------------------------------------------------------------

    /// Performs intersection testing with a rendered frustum.
    ///
    /// # Arguments
    ///
    /// * `pick_params` - Picking parameters
    /// * `view_matrix` - View matrix
    /// * `projection_matrix` - Projection matrix
    /// * `root` - Root prim
    /// * `render_params` - Rendering parameters
    ///
    /// # Returns
    ///
    /// Vector of intersection results, or None if no hits
    pub fn test_intersection(
        &mut self,
        pick_params: &PickParams,
        view_matrix: &Matrix4d,
        projection_matrix: &Matrix4d,
        root: &Prim,
        render_params: &RenderParams,
    ) -> Option<Vec<IntersectionResult>> {
        if !root.is_valid() {
            return None;
        }

        #[cfg(feature = "wgpu")]
        if let Some(hits) = self.try_pick_from_render_buffer_task(
            pick_params,
            view_matrix,
            projection_matrix,
            render_params,
        ) {
            return Some(hits);
        }

        #[cfg(feature = "wgpu")]
        if let Some(hits) = self.test_intersection_gpu_id(
            pick_params,
            view_matrix,
            projection_matrix,
            render_params,
        ) {
            return Some(hits);
        }

        let ray = ray_from_pick_frustum_center(view_matrix, projection_matrix)?;
        let purposes = pick_purposes_from_render_params(render_params);
        let sdf_time = if render_params.frame.is_default() {
            usd_sdf::TimeCode::default_time()
        } else {
            usd_sdf::TimeCode::new(render_params.frame.value())
        };

        let mut hits: Vec<(f64, IntersectionResult)> = Vec::new();
        collect_pick_hits(root, &ray, sdf_time, &purposes, &mut hits);

        if hits.is_empty() {
            return None;
        }

        hits.sort_by(|a, b| a.0.total_cmp(&b.0));

        let mode = pick_params.resolve_mode.as_str();
        if mode == "resolveAll" || mode == "resolveDeep" {
            return Some(hits.into_iter().map(|(_, hit)| hit).collect());
        }
        if mode == "resolveUnique" {
            let mut seen = std::collections::HashSet::new();
            let mut out = Vec::new();
            for (_, hit) in hits {
                if seen.insert(hit.hit_prim_path.clone()) {
                    out.push(hit);
                }
            }
            return (!out.is_empty()).then_some(out);
        }

        Some(vec![hits[0].1.clone()])
    }

    #[cfg(feature = "wgpu")]
    fn pick_storm_aov_binding_for_request(
        &self,
        binding: &usd_hdx::render_setup_task::HdRenderPassAovBinding,
        pick_color: &HgiTextureHandle,
        pick_depth: &HgiTextureHandle,
    ) -> Option<HdStAovBinding> {
        let clear_on_load = !binding.clear_value.is_empty();
        match binding.aov_name.as_str() {
            "primId" => Some(HdStAovBinding {
                aov_name: binding.aov_name.clone(),
                texture: pick_color.clone(),
                format: HgiFormat::UNorm8Vec4,
                clear_value: Self::aov_clear_value(binding, HgiFormat::UNorm8Vec4),
                clear_on_load,
            }),
            "depth" => Some(HdStAovBinding {
                aov_name: binding.aov_name.clone(),
                texture: pick_depth.clone(),
                format: HgiFormat::Float32,
                clear_value: Self::aov_clear_value(binding, HgiFormat::Float32),
                clear_on_load,
            }),
            _ => None,
        }
    }

    #[cfg(feature = "wgpu")]
    fn apply_pick_task_request_state(
        &mut self,
        request: &HdxPickTaskRequest,
        pick_color: &HgiTextureHandle,
        pick_depth: &HgiTextureHandle,
        pick_buffer: &usd_hgi::HgiBufferHandle,
    ) {
        let render_request = HdxRenderTaskRequest {
            material_tag: request.material_tag.clone(),
            render_tags: request.render_tags.clone(),
            render_pass_state: request.render_pass_state.clone(),
        };
        self.apply_hdx_render_task_request_state(&render_request);

        let state = request.render_pass_state.get();
        let storm_aov_bindings = state
            .get_aov_bindings()
            .iter()
            .filter_map(|binding| {
                self.pick_storm_aov_binding_for_request(binding, pick_color, pick_depth)
            })
            .collect();
        self.render_pass_state.set_aov_bindings(storm_aov_bindings);
        self.render_pass_state
            .set_pick_buffer(if request.bind_pick_buffer {
                Some(pick_buffer.clone())
            } else {
                None
            });
    }

    #[cfg(feature = "wgpu")]
    fn replay_pick_task_requests(
        &mut self,
        requests: &[HdxPickTaskRequest],
        pick_color: &HgiTextureHandle,
        pick_depth: &HgiTextureHandle,
        pick_buffer: &usd_hgi::HgiBufferHandle,
        pick_prim_ids: &std::collections::HashMap<Path, i32>,
    ) -> bool {
        if requests.is_empty() {
            return false;
        }
        let Some(hgi_arc) = self.wgpu_hgi.as_ref().cloned() else {
            return false;
        };

        let base_render_pass_state = self.render_pass_state.clone();
        let original_collection = self
            .render_pass
            .as_ref()
            .map(|render_pass| render_pass.get_rprim_collection().clone());
        let st_reg = self.render_delegate.read().get_st_resource_registry();
        let mut hgi = hgi_arc.write();
        let mut executed_any = false;

        for request in requests {
            self.apply_pick_task_request_state(request, pick_color, pick_depth, pick_buffer);

            if let Some(render_pass) = self.render_pass.as_mut() {
                let mut collection = render_pass.get_rprim_collection().clone();
                collection.material_tag = request.material_tag.clone();
                render_pass.set_rprim_collection(collection);

                // C++ parity: HdxPickTask::_UpdateUseOverlayPass() skips the
                // overlay pass when overlayRenderPass->HasDrawItems() is false.
                // In our architecture the task doesn't own render passes, so we
                // perform the equivalent check here at replay time.
                if !request.material_tag.is_empty()
                    && !render_pass.has_draw_items(&request.render_tags)
                {
                    continue;
                }
            }

            let st_state = self.render_pass_state.clone();
            if let Some(render_pass) = self.render_pass.as_mut() {
                render_pass.execute_with_hgi(
                    &st_state,
                    &mut *hgi,
                    pick_color,
                    pick_depth,
                    &st_reg,
                    Some(pick_prim_ids),
                    WgpuSubmitWait::WaitUntilCompleted,
                );
                executed_any = true;
            }
        }

        if let Some(collection) = original_collection {
            if let Some(render_pass) = self.render_pass.as_mut() {
                render_pass.set_rprim_collection(collection);
            }
        }
        self.render_pass_state = base_render_pass_state;
        executed_any
    }

    /// GPU ID-render pick pass.
    ///
    /// Renders a 1x1 frustum with each mesh encoded as primId color.
    /// After readback, resolves prim path, instancer path, and instance index
    /// from the render index. Matches C++ HdxPickTask::_ResolveNearestToCenter.
    #[cfg(feature = "wgpu")]
    fn test_intersection_gpu_id(
        &mut self,
        pick_params: &PickParams,
        view_matrix: &Matrix4d,
        projection_matrix: &Matrix4d,
        render_params: &RenderParams,
    ) -> Option<Vec<IntersectionResult>> {
        let ray = ray_from_pick_frustum_center(view_matrix, projection_matrix)?;
        let sdf_time = if render_params.frame.is_default() {
            usd_sdf::TimeCode::default_time()
        } else {
            usd_sdf::TimeCode::new(render_params.frame.value())
        };

        self.ensure_wgpu_pick_targets(Vec2i::new(1, 1));
        let hgi_arc = self.wgpu_hgi.as_ref()?.clone();
        let pick_color = self.wgpu_pick_color_texture.clone()?;
        let pick_depth = self.wgpu_pick_depth_texture.clone()?;
        let st_reg = self.render_delegate.read().get_st_resource_registry();

        // Build primId map from render index
        let pick_prim_ids: std::collections::HashMap<Path, i32> = {
            let index = self.render_index.as_ref()?.clone();
            let guard = index.lock().ok()?;
            let mut out = std::collections::HashMap::new();
            for prim_path in guard.get_rprim_ids() {
                if let Some(prim_id) = guard.get_prim_id_for_rprim_path(&prim_path) {
                    out.insert(prim_path, prim_id);
                }
            }
            out
        };

        let resolve_deep = pick_params.resolve_mode == pick_tokens::resolve_deep();
        let mut deep_context_params = HdxPickTaskContextParams::default();
        deep_context_params.resolution = Vec2i::new(1, 1);
        deep_context_params.pick_target = pick_params.pick_target.clone();
        deep_context_params.resolve_mode = pick_params.resolve_mode.clone();
        deep_context_params.view_matrix = *view_matrix;
        deep_context_params.projection_matrix = *projection_matrix;
        deep_context_params.clip_planes = render_params.clip_planes.clone();
        deep_context_params.alpha_threshold = if render_params.alpha_threshold >= 0.0 {
            render_params.alpha_threshold
        } else {
            0.0001
        };

        if resolve_deep {
            self.sync_task_controller_state(render_params);

            let pick_buffer_init = HdxPickTask::build_pick_buffer_init_data(&deep_context_params);
            let pick_buffer_bytes = unsafe {
                std::slice::from_raw_parts(
                    pick_buffer_init.as_ptr() as *const u8,
                    pick_buffer_init.len() * std::mem::size_of::<i32>(),
                )
            };
            let pick_buffer = {
                let mut hgi = hgi_arc.write();
                let pick_buffer_desc = HgiBufferDesc::new()
                    .with_debug_name("engine_pick_deep_buffer")
                    .with_usage(HgiBufferUsage::STORAGE)
                    .with_byte_size(pick_buffer_bytes.len());
                let pick_buffer_handle =
                    hgi.create_buffer(&pick_buffer_desc, Some(pick_buffer_bytes));
                if !pick_buffer_handle.is_valid() {
                    return None;
                }
                pick_buffer_handle
            };

            let mut task_hits = Vec::new();

            self.hd_engine
                .remove_task_context_data(&Token::new("pickTaskRequests"));
            self.hd_engine
                .remove_task_context_data(&Token::new("pickTaskRequested"));
            self.hd_engine
                .remove_task_context_data(&pick_tokens::pick_buffer());
            self.hd_engine.set_task_context_data(
                pick_tokens::pick_params(),
                Value::from_no_hash(deep_context_params.clone()),
            );

            if let Some(mut tasks) = self.task_controller.as_ref().map(|controller| {
                controller
                    .get_picking_tasks()
                    .into_iter()
                    .filter(|task| task.read().as_any().is::<HdxPickTask>())
                    .collect::<Vec<_>>()
            }) {
                if !tasks.is_empty() {
                    let index = self.render_index.as_ref()?.clone();
                    {
                        let mut index_guard = index.lock().ok()?;
                        self.hd_engine.execute(&mut index_guard, &mut tasks);
                    }

                    let pick_task_requests = self
                        .hd_engine
                        .get_task_context_data(&Token::new("pickTaskRequests"))
                        .and_then(|value| value.get::<Vec<HdxPickTaskRequest>>().cloned())
                        .unwrap_or_default();

                    if !pick_task_requests.is_empty()
                        && self.replay_pick_task_requests(
                            &pick_task_requests,
                            &pick_color,
                            &pick_depth,
                            &pick_buffer,
                            &pick_prim_ids,
                        )
                    {
                        self.hd_engine
                            .remove_task_context_data(&Token::new("pickTaskRequests"));
                        self.hd_engine
                            .remove_task_context_data(&Token::new("pickTaskRequested"));
                        self.hd_engine.set_task_context_data(
                            pick_tokens::pick_buffer(),
                            Value::new(pick_buffer.clone()),
                        );

                        {
                            let mut index_guard = index.lock().ok()?;
                            self.hd_engine.execute(&mut index_guard, &mut tasks);
                        }

                        for task in &tasks {
                            let guard = task.read();
                            let Some(pick_task) = guard.as_any().downcast_ref::<HdxPickTask>()
                            else {
                                continue;
                            };
                            task_hits = pick_task.get_hits().to_vec();
                            break;
                        }
                    }
                }
            }

            self.hd_engine
                .remove_task_context_data(&pick_tokens::pick_buffer());
            self.hd_engine
                .remove_task_context_data(&pick_tokens::pick_params());
            self.hd_engine
                .remove_task_context_data(&Token::new("pickTaskRequests"));
            self.hd_engine
                .remove_task_context_data(&Token::new("pickTaskRequested"));

            let mut hgi = hgi_arc.write();
            hgi.destroy_buffer(&pick_buffer);
            drop(hgi);
            return self.deep_pick_hits_to_intersections(&task_hits);
        }

        // Configure pick render pass state: 1x1 viewport, primId AOV
        let mut pick_state = self.render_pass_state.clone();
        pick_state.set_viewport(0.0, 0.0, 1.0, 1.0);
        pick_state.set_view_matrix(*view_matrix);
        pick_state.set_proj_matrix(*projection_matrix);
        // Clear to -1 (0xFFFFFFFF) = "no hit" in OpenUSD pick convention
        pick_state.set_clear_color(Vec4f::new(1.0, 1.0, 1.0, 1.0));
        pick_state.set_aov_bindings(vec![
            HdStAovBinding {
                aov_name: Token::new("primId"),
                texture: pick_color.clone(),
                format: HgiFormat::UNorm8Vec4,
                clear_value: Vec4f::new(1.0, 1.0, 1.0, 1.0),
                clear_on_load: true,
            },
            HdStAovBinding::new_depth(pick_depth.clone()),
        ]);

        {
            let mut hgi = hgi_arc.write();
            let render_pass = self.render_pass.as_mut()?;
            render_pass.execute_with_hgi(
                &pick_state,
                &mut *hgi,
                &pick_color,
                &pick_depth,
                &st_reg,
                Some(&pick_prim_ids),
                WgpuSubmitWait::WaitUntilCompleted,
            );
        }

        // Readback primId from color texture
        let mut hgi = hgi_arc.write();
        let pixels = self.readback_wgpu_texture(&mut *hgi, &pick_color)?;
        let prim_id = decode_pick_prim_id(&pixels)?;

        // Resolve rprim path and instancer from render index
        // Matches C++ flow: GetRprimPathFromPrimId -> GetSceneDelegateAndInstancerIds
        let (scene_index_path, instancer_path, adapter_delegate) = {
            let index = self.render_index.as_ref()?.clone();
            let guard = index.lock().ok()?;
            let rprim_path = guard.get_rprim_path_from_prim_id(prim_id);
            let inst_path = guard.get_instancer_id_for_rprim(&rprim_path);
            let adapter = guard.get_scene_index_adapter_scene_delegate();
            (rprim_path, inst_path, adapter)
        };
        if scene_index_path.is_empty() {
            return None;
        }

        // Determine instance index:
        //  - For non-instanced prims (no instancer): -1
        //  - For instanced prims (has instancer, instance_count=1 per draw): 0
        // When GPU instancing is implemented, this will come from instanceId AOV readback.
        let instance_index = if instancer_path.is_empty() { -1 } else { 0 };

        // Strip delegate prefix to get USD scene path
        let (prim_path, delegate_id) = if let Some(ref delegate) = adapter_delegate {
            (
                HdSceneDelegate::get_scene_prim_path(
                    delegate.as_ref(),
                    &scene_index_path,
                    instance_index,
                    None,
                ),
                Some(delegate.get_delegate_id().clone()),
            )
        } else {
            (scene_index_path.clone(), None)
        };
        if prim_path.is_empty() {
            return None;
        }

        // Convert instancer path: strip delegate prefix (C++ ConvertIndexPathToCachePath)
        let hit_instancer_path = if instancer_path.is_empty() {
            Path::empty()
        } else if let Some(delegate_id) = delegate_id.as_ref() {
            instancer_path
                .replace_prefix(delegate_id, &Path::absolute_root())
                .unwrap_or(instancer_path)
        } else {
            instancer_path
        };

        // Refine hit point via CPU ray-mesh test for precise position/normal
        let mut hit_point = ray.point(1.0);
        let mut hit_normal = Vec3d::new(0.0, 0.0, 1.0);

        if let Some(stage) = self
            .scene_indices
            .as_ref()
            .and_then(|indices| indices.stage_scene_index.get_stage())
        {
            if let Some(prim) = stage.get_prim_at_path(&prim_path) {
                let imageable = usd_geom::Imageable::new(prim.clone());
                if imageable.is_valid() && prim.type_name() == "Mesh" {
                    if let Some((_dist, p, n)) = mesh_pick_hit(&prim, &imageable, &ray, sdf_time) {
                        hit_point = p;
                        hit_normal = n;
                    }
                }
            }
        }

        Some(vec![IntersectionResult {
            hit_point,
            hit_normal,
            hit_prim_path: prim_path,
            hit_instancer_path,
            hit_instance_index: instance_index,
        }])
    }

    /// Try the HDX pick-from-render-buffer task first, using the current engine-managed AOVs.
    ///
    /// This is closer to the C++ reference path than the older custom GPU-ID fallback because
    /// it executes the HDX pick task against the already-rendered Hydra outputs.
    #[cfg(feature = "wgpu")]
    fn try_pick_from_render_buffer_task(
        &mut self,
        pick_params: &PickParams,
        view_matrix: &Matrix4d,
        projection_matrix: &Matrix4d,
        render_params: &RenderParams,
    ) -> Option<Vec<IntersectionResult>> {
        if pick_params.resolve_mode == pick_tokens::resolve_deep() {
            return None;
        }
        if self.render_index.is_none() {
            return None;
        }

        self.sync_task_controller_state(render_params);

        let current_aov = self.current_aov.clone();
        self.publish_engine_aovs_to_task_context(&current_aov, true);

        let mut context_params = HdxPickTaskContextParams::default();
        context_params.resolution = Vec2i::new(1, 1);
        context_params.pick_target = pick_params.pick_target.clone();
        context_params.resolve_mode = pick_params.resolve_mode.clone();
        context_params.view_matrix = *view_matrix;
        context_params.projection_matrix = *projection_matrix;
        context_params.clip_planes = render_params.clip_planes.clone();
        context_params.alpha_threshold = if render_params.alpha_threshold >= 0.0 {
            render_params.alpha_threshold
        } else {
            0.0001
        };
        self.hd_engine.set_task_context_data(
            Token::new("pickParams"),
            Value::from_no_hash(context_params),
        );

        let mut tasks = self
            .task_controller
            .as_ref()
            .map(|controller| {
                controller
                    .get_picking_tasks()
                    .into_iter()
                    .filter(|task| task.read().as_any().is::<HdxPickFromRenderBufferTask>())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if tasks.is_empty() {
            return None;
        }

        let index = self.render_index.as_ref()?.clone();
        {
            let mut index_guard = index.lock().ok()?;
            self.hd_engine.execute(&mut index_guard, &mut tasks);
        }

        let adapter_delegate = {
            let index_guard = index.lock().ok()?;
            index_guard.get_scene_index_adapter_scene_delegate()
        };

        let mut intersections = Vec::new();
        for task in tasks {
            let guard = task.read();
            let Some(pick_task) = guard.as_any().downcast_ref::<HdxPickFromRenderBufferTask>()
            else {
                continue;
            };

            for hit in pick_task.get_pick_results() {
                let hit_prim_path = if let Some(ref delegate) = adapter_delegate {
                    HdSceneDelegate::get_scene_prim_path(
                        delegate.as_ref(),
                        &hit.object_id,
                        hit.instance_index,
                        None,
                    )
                } else {
                    hit.object_id.clone()
                };

                intersections.push(IntersectionResult {
                    hit_point: hit.world_space_hit_point,
                    hit_normal: Vec3d::new(
                        hit.world_space_hit_normal.x as f64,
                        hit.world_space_hit_normal.y as f64,
                        hit.world_space_hit_normal.z as f64,
                    ),
                    hit_prim_path,
                    hit_instancer_path: hit.instancer_id.clone(),
                    hit_instance_index: hit.instance_index,
                });
            }
        }

        self.hd_engine
            .remove_task_context_data(&Token::new("pickParams"));

        (!intersections.is_empty()).then_some(intersections)
    }

    #[cfg(feature = "wgpu")]
    fn deep_pick_hits_to_intersections(
        &self,
        hits: &[HdxPickHit],
    ) -> Option<Vec<IntersectionResult>> {
        if hits.is_empty() {
            return None;
        }

        let render_index = self.render_index.as_ref()?.clone();
        let index_guard = render_index.lock().ok()?;
        let adapter_delegate = index_guard.get_scene_index_adapter_scene_delegate();

        let mut intersections = Vec::new();
        for hit in hits {
            let scene_index_path = index_guard.get_rprim_path_from_prim_id(hit.prim_id);
            if scene_index_path.is_empty() {
                continue;
            }

            let instancer_path = index_guard.get_instancer_id_for_rprim(&scene_index_path);
            let (prim_path, delegate_id) = if let Some(ref delegate) = adapter_delegate {
                (
                    HdSceneDelegate::get_scene_prim_path(
                        delegate.as_ref(),
                        &scene_index_path,
                        hit.instance_index,
                        None,
                    ),
                    Some(delegate.get_delegate_id().clone()),
                )
            } else {
                (scene_index_path.clone(), None)
            };
            if prim_path.is_empty() {
                continue;
            }

            let hit_instancer_path = if instancer_path.is_empty() {
                Path::empty()
            } else if let Some(delegate_id) = delegate_id.as_ref() {
                instancer_path
                    .replace_prefix(delegate_id, &Path::absolute_root())
                    .unwrap_or(instancer_path)
            } else {
                instancer_path
            };

            intersections.push(IntersectionResult {
                hit_point: hit.world_space_hit_point,
                hit_normal: Vec3d::new(
                    hit.world_space_hit_normal.x as f64,
                    hit.world_space_hit_normal.y as f64,
                    hit.world_space_hit_normal.z as f64,
                ),
                hit_prim_path: prim_path,
                hit_instancer_path: hit_instancer_path,
                hit_instance_index: hit.instance_index,
            });
        }

        (!intersections.is_empty()).then_some(intersections)
    }

    /// Decodes a pick result from ID render colors.
    ///
    /// Matches C++ `UsdImagingGLEngine::DecodeIntersection`. Resolves primId and
    /// instanceId colors into prim path, instancer path, and instance index.
    pub fn decode_intersection(
        &self,
        prim_id_color: [u8; 4],
        instance_id_color: [u8; 4],
    ) -> Option<IntersectionResult> {
        let prim_id = decode_id_render_color(prim_id_color);
        if prim_id < 0 {
            return None;
        }
        let instance_index = decode_id_render_color(instance_id_color);

        // Resolve rprim path and instancer from render index
        let (scene_index_path, instancer_path, adapter_delegate) = {
            let index = self.render_index.as_ref()?.clone();
            let guard = index.lock().ok()?;
            let rprim_path = guard.get_rprim_path_from_prim_id(prim_id);
            let inst_path = guard.get_instancer_id_for_rprim(&rprim_path);
            let adapter = guard.get_scene_index_adapter_scene_delegate();
            (rprim_path, inst_path, adapter)
        };
        if scene_index_path.is_empty() {
            return None;
        }
        let (prim_path, delegate_id) = if let Some(ref delegate) = adapter_delegate {
            (
                HdSceneDelegate::get_scene_prim_path(
                    delegate.as_ref(),
                    &scene_index_path,
                    instance_index,
                    None,
                ),
                Some(delegate.get_delegate_id().clone()),
            )
        } else {
            (scene_index_path.clone(), None)
        };
        if prim_path.is_empty() {
            return None;
        }

        // Convert instancer path: strip delegate prefix
        let hit_instancer_path = if instancer_path.is_empty() {
            Path::empty()
        } else if let Some(delegate_id) = delegate_id.as_ref() {
            instancer_path
                .replace_prefix(delegate_id, &Path::absolute_root())
                .unwrap_or(instancer_path)
        } else {
            instancer_path
        };

        Some(IntersectionResult {
            hit_point: Vec3d::new(0.0, 0.0, 0.0),
            hit_normal: Vec3d::new(0.0, 0.0, 1.0),
            hit_prim_path: prim_path,
            hit_instancer_path,
            hit_instance_index: instance_index,
        })
    }

    /// Create or resize 1x1 pick render targets (for single-ray GPU picking).
    ///
    /// Two color targets: primId (RGBA8) and instanceId (RGBA8). Plus depth.
    /// Matches C++ HdxPickTask AOV layout: primId + instanceId + depth.
    #[cfg(feature = "wgpu")]
    fn ensure_wgpu_pick_targets(&mut self, size: Vec2i) {
        let w = size.x.max(1);
        let h = size.y.max(1);
        let needed = Vec2i::new(w, h);

        if self.wgpu_pick_color_texture.is_some() && self.wgpu_pick_rt_size == needed {
            return;
        }

        let Some(ref hgi_arc) = self.wgpu_hgi else {
            return;
        };
        let mut hgi = hgi_arc.write();

        if let Some(ref old) = self.wgpu_pick_color_texture {
            hgi.destroy_texture(old);
        }
        if let Some(ref old) = self.wgpu_pick_instance_texture {
            hgi.destroy_texture(old);
        }
        if let Some(ref old) = self.wgpu_pick_depth_texture {
            hgi.destroy_texture(old);
        }

        // primId color target
        let color_desc = HgiTextureDesc::new()
            .with_debug_name("engine_pick_primid_rt")
            .with_format(HgiFormat::UNorm8Vec4)
            .with_usage(HgiTextureUsage::COLOR_TARGET)
            .with_dimensions(usd_gf::Vec3i::new(w, h, 1));
        self.wgpu_pick_color_texture = Some(hgi.create_texture(&color_desc, None));

        // instanceId color target
        let instance_desc = HgiTextureDesc::new()
            .with_debug_name("engine_pick_instanceid_rt")
            .with_format(HgiFormat::UNorm8Vec4)
            .with_usage(HgiTextureUsage::COLOR_TARGET)
            .with_dimensions(usd_gf::Vec3i::new(w, h, 1));
        self.wgpu_pick_instance_texture = Some(hgi.create_texture(&instance_desc, None));

        let depth_desc = HgiTextureDesc::new()
            .with_debug_name("engine_pick_depth_rt")
            .with_format(HgiFormat::Float32)
            .with_usage(HgiTextureUsage::DEPTH_TARGET)
            .with_dimensions(usd_gf::Vec3i::new(w, h, 1));
        self.wgpu_pick_depth_texture = Some(hgi.create_texture(&depth_desc, None));

        self.wgpu_pick_rt_size = needed;
    }

    /// Create or resize explicit full-resolution ID render targets (viewport-sized).
    ///
    /// These targets back the slower fallback ID pass. The common interactive
    /// path now samples the current frame's pick AOVs directly instead of
    /// keeping a dedicated ID render target hot at all times.
    #[cfg(feature = "wgpu")]
    fn ensure_wgpu_id_targets(&mut self) {
        let w = self.render_buffer_size.x.max(1);
        let h = self.render_buffer_size.y.max(1);
        let needed = Vec2i::new(w, h);

        if self.wgpu_id_color_texture.is_some() && self.wgpu_id_rt_size == needed {
            return;
        }

        let Some(ref hgi_arc) = self.wgpu_hgi else {
            return;
        };
        let mut hgi = hgi_arc.write();

        if let Some(ref old) = self.wgpu_id_color_texture {
            hgi.destroy_texture(old);
        }
        if let Some(ref old) = self.wgpu_id_depth_texture {
            hgi.destroy_texture(old);
        }

        // Full-res primId color target (RGBA8 encodes i32 primId per pixel)
        let color_desc = HgiTextureDesc::new()
            .with_debug_name("engine_id_pass_color_rt")
            .with_format(HgiFormat::UNorm8Vec4)
            .with_usage(HgiTextureUsage::COLOR_TARGET)
            .with_dimensions(usd_gf::Vec3i::new(w, h, 1));
        self.wgpu_id_color_texture = Some(hgi.create_texture(&color_desc, None));

        // Full-res depth target for correct occlusion
        let depth_desc = HgiTextureDesc::new()
            .with_debug_name("engine_id_pass_depth_rt")
            .with_format(HgiFormat::Float32)
            .with_usage(HgiTextureUsage::DEPTH_TARGET)
            .with_dimensions(usd_gf::Vec3i::new(w, h, 1));
        self.wgpu_id_depth_texture = Some(hgi.create_texture(&depth_desc, None));

        self.wgpu_id_rt_size = needed;
        log::info!("[engine] ID pass targets created: {}x{}", w, h);
    }

    /// Render the explicit full-resolution fallback ID pass (primId encoded per pixel).
    ///
    /// This is only used when current-frame pick AOVs are unavailable or miss.
    /// It reuses the same geometry and draw items as the main render.
    #[cfg(feature = "wgpu")]
    pub(super) fn render_id_pass(&mut self) {
        usd_trace::trace_scope!("engine_render_id_pass");
        if !self.id_pass_enabled {
            return;
        }

        self.ensure_wgpu_id_targets();

        let hgi_arc = match self.wgpu_hgi.as_ref() {
            Some(h) => h.clone(),
            None => return,
        };
        let id_color = match self.wgpu_id_color_texture.clone() {
            Some(t) => t,
            None => return,
        };
        let id_depth = match self.wgpu_id_depth_texture.clone() {
            Some(t) => t,
            None => return,
        };
        let st_reg = self.render_delegate.read().get_st_resource_registry();

        // Configure ID pass state: same viewport, primId AOV trigger
        let w = self.render_buffer_size.x.max(1);
        let h = self.render_buffer_size.y.max(1);
        let mut id_state = self.render_pass_state.clone();
        id_state.set_viewport(0.0, 0.0, w as f32, h as f32);
        // Clear to -1 (0xFFFFFFFF) = "no hit" in OpenUSD pick convention
        id_state.set_clear_color(Vec4f::new(1.0, 1.0, 1.0, 1.0));
        id_state.set_aov_bindings(vec![
            HdStAovBinding {
                aov_name: Token::new("primId"),
                texture: id_color.clone(),
                format: HgiFormat::UNorm8Vec4,
                clear_value: Vec4f::new(1.0, 1.0, 1.0, 1.0),
                clear_on_load: true,
            },
            HdStAovBinding::new_depth(id_depth.clone()),
        ]);

        // Execute ID render pass with same draw items
        {
            let mut hgi = hgi_arc.write();
            if let Some(ref mut render_pass) = self.render_pass {
                render_pass.execute_with_hgi(
                    &id_state,
                    &mut *hgi,
                    &id_color,
                    &id_depth,
                    &st_reg,
                    Some(&self.rprim_ids_by_path),
                    WgpuSubmitWait::NoWait,
                );
            }
        }

        log::trace!("[engine] ID pass rendered at {}x{}", w, h);
    }

    /// Pick a prim at viewport pixel coordinates.
    ///
    /// # Policy
    ///
    /// This is the general-purpose picking entry point. It first tries to
    /// decode the current frame's already-rendered pick AOVs, because that path
    /// avoids an extra render pass and keeps common interactions cheap. If
    /// those AOVs are unavailable or miss, it falls back to an explicit
    /// full-resolution ID pass for correctness.
    ///
    /// This split exists because interactive viewport work has two competing
    /// requirements:
    /// - hover and ordinary picking must stay fast on heavy scenes,
    /// - explicit selection still needs a conservative fallback when cached
    ///   AOVs are missing or stale.
    #[cfg(feature = "wgpu")]
    pub fn pick_at_pixel(&mut self, px: i32, py: i32) -> Option<IntersectionResult> {
        usd_trace::trace_scope!("engine_pick_at_pixel");
        if !self.id_pass_enabled {
            return None;
        }
        if let Some(hit) = self.pick_at_pixel_from_current_aovs(px, py) {
            return Some(hit);
        }
        self.render_id_pass();
        let id_color = self.wgpu_id_color_texture.clone()?;
        let w = self.wgpu_id_rt_size.x;
        let h = self.wgpu_id_rt_size.y;
        if px < 0 || py < 0 || px >= w || py >= h {
            return None;
        }

        // Read back a single pixel at (px, py) from the ID texture
        let hgi_arc = self.wgpu_hgi.as_ref()?.clone();
        let mut hgi = hgi_arc.write();
        let pixel = self.readback_wgpu_pixel(&mut *hgi, &id_color, px, py)?;
        drop(hgi);

        let prim_id = decode_id_render_color(pixel);
        if prim_id < 0 {
            return None; // No hit (cleared to -1)
        }

        // Resolve rprim path and instancer from render index
        let (scene_index_path, instancer_path, adapter_delegate) = {
            let index = self.render_index.as_ref()?.clone();
            let guard = index.lock().ok()?;
            let rprim_path = guard.get_rprim_path_from_prim_id(prim_id);
            let inst_path = guard.get_instancer_id_for_rprim(&rprim_path);
            let adapter = guard.get_scene_index_adapter_scene_delegate();
            (rprim_path, inst_path, adapter)
        };
        if scene_index_path.is_empty() {
            return None;
        }

        let instance_index = if instancer_path.is_empty() { -1 } else { 0 };

        // Strip delegate prefix to get USD scene path
        let (prim_path, delegate_id) = if let Some(ref delegate) = adapter_delegate {
            (
                HdSceneDelegate::get_scene_prim_path(
                    delegate.as_ref(),
                    &scene_index_path,
                    instance_index,
                    None,
                ),
                Some(delegate.get_delegate_id().clone()),
            )
        } else {
            (scene_index_path.clone(), None)
        };
        if prim_path.is_empty() {
            return None;
        }

        // Convert instancer path: strip delegate prefix
        let hit_instancer_path = if instancer_path.is_empty() {
            Path::empty()
        } else if let Some(delegate_id) = delegate_id.as_ref() {
            instancer_path
                .replace_prefix(delegate_id, &Path::absolute_root())
                .unwrap_or(instancer_path)
        } else {
            instancer_path
        };

        Some(IntersectionResult {
            hit_point: Vec3d::new(0.0, 0.0, 0.0),
            hit_normal: Vec3d::new(0.0, 0.0, 1.0),
            hit_prim_path: prim_path,
            hit_instancer_path,
            hit_instance_index: instance_index,
        })
    }

    /// Pick a prim using only the dedicated full-resolution ID-pass path.
    ///
    /// # Why this exists
    ///
    /// The cached current-frame AOV path is the fastest option, but for some
    /// interactions the caller may prefer a more conservative route that forces
    /// a fresh ID pass and samples that result directly. This is primarily
    /// useful for click selection, where correctness matters more than shaving
    /// the last few milliseconds and a single extra GPU pass is acceptable.
    #[cfg(feature = "wgpu")]
    pub fn pick_at_pixel_via_id_pass(&mut self, px: i32, py: i32) -> Option<IntersectionResult> {
        usd_trace::trace_scope!("engine_pick_at_pixel_id_pass");
        if !self.id_pass_enabled {
            return None;
        }
        self.render_id_pass();
        let id_color = self.wgpu_id_color_texture.clone()?;
        let w = self.wgpu_id_rt_size.x;
        let h = self.wgpu_id_rt_size.y;
        if px < 0 || py < 0 || px >= w || py >= h {
            return None;
        }

        let hgi_arc = self.wgpu_hgi.as_ref()?.clone();
        let mut hgi = hgi_arc.write();
        let pixel = self.readback_wgpu_pixel(&mut *hgi, &id_color, px, py)?;
        drop(hgi);

        let prim_id = decode_id_render_color(pixel);
        if prim_id < 0 {
            return None;
        }

        let (scene_index_path, instancer_path, adapter_delegate) = {
            let index = self.render_index.as_ref()?.clone();
            let guard = index.lock().ok()?;
            let rprim_path = guard.get_rprim_path_from_prim_id(prim_id);
            let inst_path = guard.get_instancer_id_for_rprim(&rprim_path);
            let adapter = guard.get_scene_index_adapter_scene_delegate();
            (rprim_path, inst_path, adapter)
        };
        if scene_index_path.is_empty() {
            return None;
        }

        let instance_index = if instancer_path.is_empty() { -1 } else { 0 };
        let (prim_path, delegate_id) = if let Some(ref delegate) = adapter_delegate {
            (
                HdSceneDelegate::get_scene_prim_path(
                    delegate.as_ref(),
                    &scene_index_path,
                    instance_index,
                    None,
                ),
                Some(delegate.get_delegate_id().clone()),
            )
        } else {
            (scene_index_path.clone(), None)
        };
        if prim_path.is_empty() {
            return None;
        }

        let hit_instancer_path = if instancer_path.is_empty() {
            Path::empty()
        } else if let Some(delegate_id) = delegate_id.as_ref() {
            instancer_path
                .replace_prefix(delegate_id, &Path::absolute_root())
                .unwrap_or(instancer_path)
        } else {
            instancer_path
        };

        Some(IntersectionResult {
            hit_point: Vec3d::new(0.0, 0.0, 0.0),
            hit_normal: Vec3d::new(0.0, 0.0, 1.0),
            hit_prim_path: prim_path,
            hit_instancer_path,
            hit_instance_index: instance_index,
        })
    }

    /// Pick from the engine's current-frame AOVs without triggering an extra
    /// ID render pass.
    ///
    /// # Why this exists
    ///
    /// Hover and pre-drag interaction paths must remain bounded and cheap on
    /// large scenes. Using the current frame's `primId` / `instanceId` AOVs
    /// lets the viewer sample a single texel from data that already exists on
    /// the GPU instead of paying for another full pass or a CPU-side exact-pick
    /// fallback on the UI thread.
    ///
    /// Callers that need a stricter correctness fallback can layer
    /// [`Self::pick_at_pixel_via_id_pass`] on top of this method.
    #[cfg(feature = "wgpu")]
    pub fn pick_at_pixel_from_current_aovs(
        &mut self,
        px: i32,
        py: i32,
    ) -> Option<IntersectionResult> {
        usd_trace::trace_scope!("engine_pick_at_pixel_cached");
        let prim_id_tex = self.wgpu_prim_id_texture.clone()?;
        let instance_id_tex = self.wgpu_instance_id_texture.clone();
        let w = self.render_buffer_size.x.max(1);
        let h = self.render_buffer_size.y.max(1);
        if px < 0 || py < 0 || px >= w || py >= h {
            return None;
        }

        let hgi_arc = self.wgpu_hgi.as_ref()?.clone();
        let mut hgi = hgi_arc.write();
        let prim_id_color = self.readback_wgpu_pixel(&mut *hgi, &prim_id_tex, px, py)?;
        let instance_id_color = if let Some(tex) = instance_id_tex.as_ref() {
            self.readback_wgpu_pixel(&mut *hgi, tex, px, py)
                .unwrap_or([255, 255, 255, 255])
        } else {
            [255, 255, 255, 255]
        };
        drop(hgi);

        self.decode_intersection(prim_id_color, instance_id_color)
    }

    /// Enable or disable the explicit full-resolution ID-pass fallback.
    ///
    /// When enabled, `pick_at_pixel()` may issue the slower fallback ID render
    /// if the current frame's pick AOVs are unavailable or miss. The common
    /// hover path does not depend on this fallback anymore.
    #[cfg(feature = "wgpu")]
    pub fn set_id_pass_enabled(&mut self, enabled: bool) {
        self.id_pass_enabled = enabled;
    }

    /// Returns true when the full-resolution ID pass is active.
    #[cfg(feature = "wgpu")]
    pub fn is_id_pass_enabled(&self) -> bool {
        self.id_pass_enabled
    }

    /// Read back the color texture contents via HGI blit commands.
    #[cfg(feature = "wgpu")]
    pub(super) fn readback_wgpu_texture(
        &self,
        hgi: &mut dyn Hgi,
        color_tex: &HgiTextureHandle,
    ) -> Option<Vec<u8>> {
        let (format, raw_pixels) = self.readback_wgpu_texture_raw(hgi, color_tex)?;
        match format {
            HgiFormat::UNorm8Vec4 => Some(raw_pixels),
            HgiFormat::Float16Vec4 => Some(convert_rgba16f_to_rgba8(&raw_pixels)),
            other => {
                log::warn!(
                    "[engine] readback_wgpu_texture: unsupported color format for CPU export: {:?}",
                    other
                );
                None
            }
        }
    }

    /// Read back the color texture as linear RGBA32F.
    #[cfg(feature = "wgpu")]
    pub(super) fn readback_wgpu_texture_linear_rgba32f(
        &self,
        hgi: &mut dyn Hgi,
        color_tex: &HgiTextureHandle,
    ) -> Option<Vec<f32>> {
        let (format, raw_pixels) = self.readback_wgpu_texture_raw(hgi, color_tex)?;
        match format {
            HgiFormat::UNorm8Vec4 => Some(convert_rgba8_to_rgba32f(&raw_pixels)),
            HgiFormat::Float16Vec4 => Some(convert_rgba16f_to_rgba32f(&raw_pixels)),
            other => {
                log::warn!(
                    "[engine] readback_wgpu_texture_linear_rgba32f: unsupported color format: {:?}",
                    other
                );
                None
            }
        }
    }

    /// Raw GPU->CPU readback of the full color texture, without format conversion.
    #[cfg(feature = "wgpu")]
    pub(super) fn readback_wgpu_texture_raw(
        &self,
        hgi: &mut dyn Hgi,
        color_tex: &HgiTextureHandle,
    ) -> Option<(HgiFormat, Vec<u8>)> {
        usd_trace::trace_scope!("engine_readback_texture");
        let desc = color_tex.get()?.descriptor().clone();
        let w = desc.dimensions[0].max(1) as usize;
        let h = desc.dimensions[1].max(1) as usize;
        let (bytes_per_texel, _, _) = desc.format.data_size_of_format();
        let raw_byte_size = w * h * bytes_per_texel;
        let mut raw_pixels = vec![0u8; raw_byte_size];

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut blit = hgi.create_blit_cmds();
            let op = HgiTextureGpuToCpuOp {
                gpu_source_texture: color_tex.clone(),
                source_texel_offset: usd_gf::Vec3i::new(0, 0, 0),
                mip_level: 0,
                cpu_destination_buffer: unsafe { RawCpuBufferMut::new(raw_pixels.as_mut_ptr()) },
                destination_byte_offset: 0,
                destination_buffer_byte_size: raw_byte_size,
                copy_size: usd_gf::Vec3i::new(w as i32, h as i32, 1),
                source_layer: 0,
            };
            blit.copy_texture_gpu_to_cpu(&op);
            hgi.submit_cmds(blit, WgpuSubmitWait::WaitUntilCompleted);
        }));

        if result.is_err() {
            log::warn!("[engine] readback_wgpu_texture_raw: blit failed (stale texture?)");
            return None;
        }

        Some((desc.format, raw_pixels))
    }

    /// Read back a single pixel at (px, py) from a wgpu texture.
    /// Returns 4 bytes (RGBA8) or None on failure. ~2000x faster than full readback.
    #[cfg(feature = "wgpu")]
    fn readback_wgpu_pixel(
        &self,
        hgi: &mut dyn Hgi,
        color_tex: &HgiTextureHandle,
        px: i32,
        py: i32,
    ) -> Option<[u8; 4]> {
        let desc = color_tex.get()?.descriptor().clone();
        let width = desc.dimensions.x.max(1);
        let height = desc.dimensions.y.max(1);
        if px < 0 || py < 0 || px >= width || py >= height {
            return None;
        }

        let mut pixel = [0u8; 4];

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut blit = hgi.create_blit_cmds();
            let op = HgiTextureGpuToCpuOp {
                gpu_source_texture: color_tex.clone(),
                // Direct WGPU texture readback uses texture space, which is
                // top-left-origin like the viewport coordinates we pass in.
                source_texel_offset: usd_gf::Vec3i::new(px, py, 0),
                mip_level: 0,
                cpu_destination_buffer: unsafe { RawCpuBufferMut::new(pixel.as_mut_ptr()) },
                destination_byte_offset: 0,
                destination_buffer_byte_size: 4,
                copy_size: usd_gf::Vec3i::new(1, 1, 1),
                source_layer: 0,
            };
            blit.copy_texture_gpu_to_cpu(&op);
            hgi.submit_cmds(blit, WgpuSubmitWait::WaitUntilCompleted);
        }));

        if result.is_err() {
            log::warn!("[engine] readback_wgpu_pixel: blit failed");
            return None;
        }

        Some(pixel)
    }

    // -------------------------------------------------------------------------
    // Efficient wgpu readback (persistent staging buffer)
    // -------------------------------------------------------------------------

    /// Read rendered pixels using a persistent staging buffer.
    ///
    /// Unlike `read_render_pixels()` which allocates a new Vec every frame,
    /// this reuses a staging buffer and returns a borrowed slice.
    /// Much more efficient for per-frame readback in interactive viewports.
    ///
    /// Returns (pixels_slice, width, height) or None if no render target.
    #[cfg(feature = "wgpu")]
    pub fn read_render_pixels_staged(&mut self) -> Option<(&[u8], u32, u32)> {
        usd_trace::trace_scope!("engine_readback_staged");
        let hgi_arc = self.wgpu_hgi.as_ref()?.clone();
        let color_tex = self.wgpu_color_texture.clone()?;

        let hgi = hgi_arc.read();
        let device = hgi.device();
        let queue = hgi.queue();

        let (pixels, w, h) = self.wgpu_staging.readback(device, queue, &color_tex)?;
        Some((pixels, w, h))
    }
}

#[cfg(feature = "wgpu")]
fn convert_rgba16f_to_rgba8(raw_pixels: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw_pixels.len() / 2);
    for chunk in raw_pixels.chunks_exact(8) {
        for c in 0..4 {
            let i = c * 2;
            let bits = u16::from_le_bytes([chunk[i], chunk[i + 1]]);
            let value = f16::from_bits(bits).to_f32();
            out.push((value.clamp(0.0, 1.0) * 255.0 + 0.5) as u8);
        }
    }
    out
}

#[cfg(feature = "wgpu")]
fn convert_rgba16f_to_rgba32f(raw_pixels: &[u8]) -> Vec<f32> {
    let mut out = Vec::with_capacity(raw_pixels.len() / 2);
    for chunk in raw_pixels.chunks_exact(8) {
        for c in 0..4 {
            let i = c * 2;
            let bits = u16::from_le_bytes([chunk[i], chunk[i + 1]]);
            out.push(f16::from_bits(bits).to_f32());
        }
    }
    out
}

#[cfg(feature = "wgpu")]
fn convert_rgba8_to_rgba32f(raw_pixels: &[u8]) -> Vec<f32> {
    raw_pixels.iter().map(|&v| f32::from(v) / 255.0).collect()
}

#[cfg(all(test, feature = "wgpu"))]
mod tests {
    use super::{convert_rgba16f_to_rgba8, convert_rgba16f_to_rgba32f};
    use half::f16;

    #[test]
    fn test_convert_rgba16f_to_rgba8_clamps_and_scales() {
        let samples = [
            f16::from_f32(0.0).to_bits(),
            f16::from_f32(0.5).to_bits(),
            f16::from_f32(1.0).to_bits(),
            f16::from_f32(2.0).to_bits(),
        ];
        let mut raw = Vec::new();
        for bits in samples {
            raw.extend_from_slice(&bits.to_le_bytes());
        }

        assert_eq!(convert_rgba16f_to_rgba8(&raw), vec![0, 128, 255, 255]);
    }

    #[test]
    fn test_convert_rgba16f_to_rgba32f_preserves_hdr_values() {
        let samples = [
            f16::from_f32(-0.25).to_bits(),
            f16::from_f32(0.5).to_bits(),
            f16::from_f32(1.0).to_bits(),
            f16::from_f32(2.0).to_bits(),
        ];
        let mut raw = Vec::new();
        for bits in samples {
            raw.extend_from_slice(&bits.to_le_bytes());
        }

        assert_eq!(convert_rgba16f_to_rgba32f(&raw), vec![-0.25, 0.5, 1.0, 2.0]);
    }
}

// -------------------------------------------------------------------------
// Free helper functions
// -------------------------------------------------------------------------

/// Build a ray from the center of the pick frustum (NDC origin).
/// Unprojects near/far points at (0,0) NDC and returns the ray.
fn ray_from_pick_frustum_center(
    view_matrix: &Matrix4d,
    projection_matrix: &Matrix4d,
) -> Option<usd_gf::ray::Ray> {
    let vp = *view_matrix * *projection_matrix;
    let inv_vp = vp.inverse()?;
    let near_world = unproject_ndc(&inv_vp, 0.0, 0.0, -1.0)?;
    let far_world = unproject_ndc(&inv_vp, 0.0, 0.0, 1.0)?;
    let dir = far_world - near_world;
    let len2 = dir.x * dir.x + dir.y * dir.y + dir.z * dir.z;
    if len2 < 1e-20 {
        return None;
    }
    Some(usd_gf::ray::Ray::new(near_world, dir))
}

/// Unproject an NDC point through the inverse view-projection matrix.
fn unproject_ndc(inv_vp: &Matrix4d, x: f64, y: f64, z: f64) -> Option<Vec3d> {
    let wx = x * inv_vp[0][0] + y * inv_vp[1][0] + z * inv_vp[2][0] + inv_vp[3][0];
    let wy = x * inv_vp[0][1] + y * inv_vp[1][1] + z * inv_vp[2][1] + inv_vp[3][1];
    let wz = x * inv_vp[0][2] + y * inv_vp[1][2] + z * inv_vp[2][2] + inv_vp[3][2];
    let ww = x * inv_vp[0][3] + y * inv_vp[1][3] + z * inv_vp[2][3] + inv_vp[3][3];
    if !ww.is_finite() || ww.abs() < 1e-14 {
        return None;
    }
    Some(Vec3d::new(wx / ww, wy / ww, wz / ww))
}

/// Decode an RGBA8 ID render color into a signed primId/instanceId.
/// Uses little-endian byte order matching C++ HdxPickTask encoding.
#[inline]
fn decode_id_render_color(id_color: [u8; 4]) -> i32 {
    i32::from_le_bytes(id_color)
}

/// Extract primId from a readback pixel buffer (first 4 bytes).
/// Returns None if the buffer is too short or the id encodes "no hit" (-1).
#[cfg(feature = "wgpu")]
fn decode_pick_prim_id(pixels: &[u8]) -> Option<i32> {
    if pixels.len() < 4 {
        return None;
    }
    let prim_id = decode_id_render_color([pixels[0], pixels[1], pixels[2], pixels[3]]);
    if prim_id < 0 {
        return None;
    }
    Some(prim_id)
}

/// Build the list of allowed pick purposes from render params.
fn pick_purposes_from_render_params(params: &RenderParams) -> Vec<usd_tf::Token> {
    let t = usd_geom::tokens::usd_geom_tokens();
    let mut purposes = vec![t.default_.clone()];
    if params.show_render {
        purposes.push(t.render.clone());
    }
    if params.show_proxy {
        purposes.push(t.proxy.clone());
    }
    if params.show_guides {
        purposes.push(t.guide.clone());
    }
    purposes
}

/// Recursively collect ray-prim intersection hits under `prim`.
/// Skips invisible prims and prims whose purpose is not in `purposes`.
fn collect_pick_hits(
    prim: &Prim,
    ray: &usd_gf::ray::Ray,
    time: usd_sdf::TimeCode,
    purposes: &[usd_tf::Token],
    out_hits: &mut Vec<(f64, IntersectionResult)>,
) {
    if !prim.is_valid() {
        return;
    }

    let imageable = usd_geom::Imageable::new(prim.clone());
    if imageable.is_valid() {
        let t = usd_geom::tokens::usd_geom_tokens();
        if imageable.compute_visibility(time) == t.invisible {
            return;
        }

        let purpose = imageable.compute_purpose();
        let purpose_allowed = purposes.is_empty() || purposes.contains(&purpose);
        if purpose_allowed && is_pickable_geom_prim(prim) {
            let mut args: Vec<usd_tf::Token> = if purposes.is_empty() {
                vec![t.default_.clone()]
            } else {
                purposes.iter().take(4).cloned().collect()
            };
            if args.is_empty() {
                args.push(t.default_.clone());
            }
            let bbox = imageable.compute_world_bound(
                time,
                args.get(0),
                args.get(1),
                args.get(2),
                args.get(3),
            );
            if let Some((enter, _)) = ray.intersect_bbox(&bbox) {
                if enter >= 0.0 {
                    if prim.type_name() == "Mesh" {
                        if let Some((dist, hit_point, hit_normal)) =
                            mesh_pick_hit(prim, &imageable, ray, time)
                        {
                            out_hits.push((
                                dist,
                                IntersectionResult {
                                    hit_point,
                                    hit_normal,
                                    hit_prim_path: prim.path().clone(),
                                    hit_instancer_path: Path::empty(),
                                    hit_instance_index: -1,
                                },
                            ));
                        }
                    } else {
                        out_hits.push((
                            enter,
                            IntersectionResult {
                                hit_point: ray.point(enter),
                                hit_normal: Vec3d::new(0.0, 0.0, 1.0),
                                hit_prim_path: prim.path().clone(),
                                hit_instancer_path: Path::empty(),
                                hit_instance_index: -1,
                            },
                        ));
                    }
                }
            }
        }
    }

    for child in prim.get_children() {
        collect_pick_hits(&child, ray, time, purposes, out_hits);
    }
}

/// Returns true if this prim type can be picked (has geometry for ray testing).
#[inline]
fn is_pickable_geom_prim(prim: &Prim) -> bool {
    matches!(
        prim.type_name().as_str(),
        "Mesh"
            | "Points"
            | "BasisCurves"
            | "Curves"
            | "Sphere"
            | "Cube"
            | "Cone"
            | "Cylinder"
            | "Capsule"
            | "Plane"
    )
}

/// CPU ray-mesh intersection test for a single UsdGeomMesh prim.
/// Returns (distance, hit_point_world, hit_normal_world) for the closest triangle hit.
fn mesh_pick_hit(
    prim: &Prim,
    imageable: &usd_geom::Imageable,
    ray: &usd_gf::ray::Ray,
    time: usd_sdf::TimeCode,
) -> Option<(f64, Vec3d, Vec3d)> {
    let mesh = usd_geom::Mesh::new(prim.clone());
    let face_counts = read_int_array_attr(&mesh.get_face_vertex_counts_attr(), time);
    let face_indices = read_int_array_attr(&mesh.get_face_vertex_indices_attr(), time);
    if face_counts.is_empty() || face_indices.len() < 3 {
        return None;
    }

    let mut points = Vec::new();
    if !mesh.point_based().compute_points_at_time(
        &mut points,
        time,
        usd_sdf::TimeCode::default_time(),
    ) || points.is_empty()
    {
        let raw = read_float3_array_attr(&mesh.point_based().get_points_attr(), time);
        if raw.is_empty() {
            return None;
        }
        points = raw
            .iter()
            .map(|p| usd_gf::Vec3f::new(p[0], p[1], p[2]))
            .collect();
    }

    let xf = imageable.compute_local_to_world_transform(time);
    let world_points: Vec<Vec3d> = points
        .iter()
        .map(|p| xf.transform_point(&Vec3d::new(p[0] as f64, p[1] as f64, p[2] as f64)))
        .collect();

    let mut best_dist = f64::INFINITY;
    let mut best_point = Vec3d::new(0.0, 0.0, 0.0);
    let mut best_normal = Vec3d::new(0.0, 0.0, 1.0);
    let mut hit = false;

    let mut offset = 0usize;
    for count in face_counts {
        let count = count.max(0) as usize;
        if count < 3 {
            offset = offset.saturating_add(count);
            continue;
        }
        if offset + count > face_indices.len() {
            break;
        }

        let Some(p0) = tri_point(&world_points, face_indices[offset]) else {
            offset += count;
            continue;
        };
        for j in 1..(count - 1) {
            let (Some(p1), Some(p2)) = (
                tri_point(&world_points, face_indices[offset + j]),
                tri_point(&world_points, face_indices[offset + j + 1]),
            ) else {
                continue;
            };

            if let Some((dist, _bary, _front)) = ray.intersect_triangle(&p0, &p1, &p2, best_dist) {
                if dist >= 0.0 && dist < best_dist {
                    let n = (p1 - p0).cross(&(p2 - p0));
                    let nlen2 = n.x * n.x + n.y * n.y + n.z * n.z;
                    best_normal = if nlen2 > 1e-20 {
                        let inv_len = 1.0 / nlen2.sqrt();
                        Vec3d::new(n.x * inv_len, n.y * inv_len, n.z * inv_len)
                    } else {
                        Vec3d::new(0.0, 0.0, 1.0)
                    };
                    best_dist = dist;
                    best_point = ray.point(dist);
                    hit = true;
                }
            }
        }
        offset += count;
    }

    hit.then_some((best_dist, best_point, best_normal))
}

/// Index into world_points by face index, returning None for out-of-range indices.
#[inline]
fn tri_point(points: &[Vec3d], index: i32) -> Option<Vec3d> {
    usize::try_from(index)
        .ok()
        .and_then(|i| points.get(i))
        .cloned()
}

/// Read an `int[]` attribute, handling multiple storage representations.
///
/// USDA parser yields `Vec<Value>` (each wrapping i32/i64), while USDC
/// produces native `Vec<i32>`. Falls back to `TimeCode::default_time()`
/// if the requested time has no sample.
pub(super) fn read_int_array_attr(
    attr: &usd_core::attribute::Attribute,
    time: usd_sdf::TimeCode,
) -> Vec<i32> {
    if !attr.is_valid() {
        return Vec::new();
    }
    let val_opt = attr.get(time);
    let val_opt2 = if val_opt.is_none() {
        attr.get(usd_sdf::TimeCode::default_time())
    } else {
        None
    };
    let Some(val) = val_opt.or(val_opt2) else {
        return Vec::new();
    };
    // as_vec_clone handles both Vec<T> and Array<T> storage
    if let Some(arr) = val.as_vec_clone::<i32>() {
        return arr;
    }
    // Try Vec<Value> (USDA parser output)
    if let Some(vec) = val.downcast::<Vec<usd_vt::Value>>() {
        return vec
            .iter()
            .filter_map(|v| {
                v.downcast_clone::<i32>()
                    .or_else(|| v.downcast_clone::<i64>().map(|n| n as i32))
            })
            .collect();
    }
    Vec::new()
}

/// Read a float3 array attribute, handling both native types and Vec<Value>.
/// USDA parser stores point3f[] as Vec<Value> where each element is a
/// nested Vec<Value> of 3 f64 values (from tuple conversion).
pub(super) fn read_float3_array_attr(
    attr: &usd_core::attribute::Attribute,
    time: usd_sdf::TimeCode,
) -> Vec<[f32; 3]> {
    if !attr.is_valid() {
        log::warn!("[read_f3] attr {} invalid", attr.path());
        return Vec::new();
    }
    let Some(val) = attr
        .get(time)
        .or_else(|| attr.get(usd_sdf::TimeCode::default_time()))
    else {
        log::warn!(
            "[read_f3] attr {} get({:?}) returned None",
            attr.path(),
            time.value()
        );
        return Vec::new();
    };
    // as_vec_clone handles both Vec<T> and Array<T> storage
    if let Some(arr) = val.as_vec_clone::<usd_gf::Vec3f>() {
        return arr.iter().map(|v| [v[0], v[1], v[2]]).collect();
    }
    // Vec<Value> containing GfVec3f, [f32;3], or nested Vec<Value> tuples (USDA)
    if let Some(vec) = val.downcast::<Vec<usd_vt::Value>>() {
        return vec.iter().filter_map(|v| value_to_f3(v)).collect();
    }
    // abc_reader returns Array<Vec3f> wrapped in Value — try downcast
    if let Some(arr) = val.downcast::<usd_vt::Array<usd_gf::Vec3f>>() {
        return arr.iter().map(|v| [v[0], v[1], v[2]]).collect();
    }
    log::warn!(
        "[read_f3] attr {} val type not recognized: {:?}",
        attr.path(),
        val.type_name()
    );
    Vec::new()
}

/// Convert a single Value to [f32; 3].
/// Handles: GfVec3f, [f32; 3], nested Vec<Value> of 3 floats/doubles.
pub(super) fn value_to_f3(v: &usd_vt::Value) -> Option<[f32; 3]> {
    // Direct GfVec3f
    if let Some(gv) = v.downcast::<usd_gf::Vec3f>() {
        return Some([gv[0], gv[1], gv[2]]);
    }
    // Direct [f32; 3]
    if let Some(arr) = v.downcast_clone::<[f32; 3]>() {
        return Some(arr);
    }
    // Nested Vec<Value> of 3 numeric elements (USDA tuple representation)
    if let Some(inner) = v.downcast::<Vec<usd_vt::Value>>() {
        if inner.len() >= 3 {
            let x = value_to_f64(&inner[0])?;
            let y = value_to_f64(&inner[1])?;
            let z = value_to_f64(&inner[2])?;
            return Some([x as f32, y as f32, z as f32]);
        }
    }
    None
}

/// Extract f64 from a Value (handles f64, f32, i64, i32).
pub(super) fn value_to_f64(v: &usd_vt::Value) -> Option<f64> {
    if let Some(&f) = v.downcast::<f64>() {
        return Some(f);
    }
    if let Some(&f) = v.downcast::<f32>() {
        return Some(f as f64);
    }
    if let Some(&i) = v.downcast::<i64>() {
        return Some(i as f64);
    }
    if let Some(&i) = v.downcast::<i32>() {
        return Some(i as f64);
    }
    None
}
