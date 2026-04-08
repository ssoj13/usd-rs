//! Engine bookkeeping synchronization from the Hydra render index.
//!
//! Contains `Engine::sync_render_index_state`, which derives viewer-side caches
//! from already-synchronized Hydra rprims instead of walking the USD stage a
//! second time for geometry or transforms.

use std::collections::HashMap;
use usd_hd::change_tracker::HdRprimDirtyBits;
use usd_hd::prim::HdSceneDelegate;
use usd_hd::render::HdRenderIndex;
use usd_hd::tokens::INSTANCE_TRANSFORMS;
use usd_hd_st::material::HdStMaterial;
use usd_hd_st::mesh::HdStMesh;
use usd_sdf::Path;
use usd_shade::material_binding_api::MaterialBindingAPI;

use super::Engine;
#[cfg(feature = "wgpu")]
use super::texture::load_mesh_textures;

fn accumulate_world_bounds(
    local_min: [f32; 3],
    local_max: [f32; 3],
    xform: &[[f64; 4]; 4],
    world_min: &mut [f32; 3],
    world_max: &mut [f32; 3],
) -> bool {
    if local_min[0] > local_max[0] || local_min[1] > local_max[1] || local_min[2] > local_max[2] {
        return false;
    }

    let corners = [
        [local_min[0], local_min[1], local_min[2]],
        [local_min[0], local_min[1], local_max[2]],
        [local_min[0], local_max[1], local_min[2]],
        [local_min[0], local_max[1], local_max[2]],
        [local_max[0], local_min[1], local_min[2]],
        [local_max[0], local_min[1], local_max[2]],
        [local_max[0], local_max[1], local_min[2]],
        [local_max[0], local_max[1], local_max[2]],
    ];

    for corner in corners {
        let x = corner[0];
        let y = corner[1];
        let z = corner[2];
        let wx = xform[0][0] as f32 * x
            + xform[1][0] as f32 * y
            + xform[2][0] as f32 * z
            + xform[3][0] as f32;
        let wy = xform[0][1] as f32 * x
            + xform[1][1] as f32 * y
            + xform[2][1] as f32 * z
            + xform[3][1] as f32;
        let wz = xform[0][2] as f32 * x
            + xform[1][2] as f32 * y
            + xform[2][2] as f32 * z
            + xform[3][2] as f32;
        world_min[0] = world_min[0].min(wx);
        world_min[1] = world_min[1].min(wy);
        world_min[2] = world_min[2].min(wz);
        world_max[0] = world_max[0].max(wx);
        world_max[1] = world_max[1].max(wy);
        world_max[2] = world_max[2].max(wz);
    }

    true
}

fn mul_matrix4(lhs: &[[f64; 4]; 4], rhs: &[[f64; 4]; 4]) -> [[f64; 4]; 4] {
    let mut out = [[0.0; 4]; 4];
    for row in 0..4 {
        for col in 0..4 {
            out[row][col] = lhs[row][0] * rhs[0][col]
                + lhs[row][1] * rhs[1][col]
                + lhs[row][2] * rhs[2][col]
                + lhs[row][3] * rhs[3][col];
        }
    }
    out
}

fn decode_instance_transforms(
    scene_delegate: &dyn HdSceneDelegate,
    instancer_id: &Path,
    prototype_id: &Path,
) -> Vec<[[f64; 4]; 4]> {
    let raw = scene_delegate.get(instancer_id, &INSTANCE_TRANSFORMS);
    let flat: Vec<f64> = if let Some(values) = raw.as_vec_clone::<f64>() {
        values
    } else if let Some(values) = raw.as_vec_clone::<f32>() {
        values.iter().map(|&value| value as f64).collect()
    } else {
        Vec::new()
    };
    if flat.len() < 16 {
        return Vec::new();
    }

    let indices = scene_delegate.get_instance_indices(instancer_id, prototype_id);
    let instancer_xform = scene_delegate
        .get_instancer_transform(instancer_id)
        .to_array();
    let instance_count = flat.len() / 16;
    let selected: Vec<usize> = if indices.is_empty() {
        (0..instance_count).collect()
    } else {
        indices
            .into_iter()
            .filter_map(|index| usize::try_from(index).ok())
            .filter(|&index| index < instance_count)
            .collect()
    };

    selected
        .into_iter()
        .map(|index| {
            let base = index * 16;
            let local = [
                [flat[base], flat[base + 1], flat[base + 2], flat[base + 3]],
                [
                    flat[base + 4],
                    flat[base + 5],
                    flat[base + 6],
                    flat[base + 7],
                ],
                [
                    flat[base + 8],
                    flat[base + 9],
                    flat[base + 10],
                    flat[base + 11],
                ],
                [
                    flat[base + 12],
                    flat[base + 13],
                    flat[base + 14],
                    flat[base + 15],
                ],
            ];
            mul_matrix4(&local, &instancer_xform)
        })
        .collect()
}

impl Engine {
    /// Returns true when the pending Hydra rprim dirties are transform-only, so
    /// viewer bookkeeping can refresh transforms/bounds without re-running the
    /// heavier material/texture/draw-item binding pass.
    /// Classify dirty rprims before `HdEngine::execute` clears dirty bits.
    ///
    /// Returns `(transform_only, dirty_transform_paths)`:
    /// - `transform_only`: true when **all** dirty bits are DIRTY_TRANSFORM|DIRTY_EXTENT
    ///   (pure xform animation — no mesh/material/topology edits).
    /// - `dirty_transform_paths`: rprim paths with DIRTY_TRANSFORM set, used for
    ///   incremental `model_transforms` update after execute.
    ///
    /// When `force_full_refresh` is true or nothing is dirty, returns `(false, empty)`.
    pub(super) fn classify_dirty_rprims(
        index: &HdRenderIndex,
        hydra_state_dirty: bool,
        force_full_refresh: bool,
    ) -> (bool, Vec<Path>) {
        if force_full_refresh || !hydra_state_dirty {
            return (false, Vec::new());
        }

        // Pure xform animation often emits `DIRTY_EXTENT` next to `DIRTY_TRANSFORM`
        // (world bounds / flattening), without mesh or material edits.
        const ALLOWED_WITH_XFORM: u32 =
            HdRprimDirtyBits::DIRTY_TRANSFORM | HdRprimDirtyBits::DIRTY_EXTENT;

        let mut transform_only = true;
        let mut dirty_xform_paths = Vec::new();

        for prim_id in index.get_rprim_ids() {
            let scene_bits = index.get_change_tracker().get_rprim_dirty_bits(&prim_id)
                & !HdRprimDirtyBits::VARYING;
            if scene_bits == 0 {
                continue;
            }
            if scene_bits & !ALLOWED_WITH_XFORM != 0 {
                transform_only = false;
            }
            if (scene_bits & HdRprimDirtyBits::DIRTY_TRANSFORM) != 0 {
                dirty_xform_paths.push(prim_id);
            }
        }

        if dirty_xform_paths.is_empty() {
            transform_only = false;
        }

        (transform_only, dirty_xform_paths)
    }

    /// Refresh engine bookkeeping from already-synchronized Hydra rprims.
    ///
    /// This follows the reference Hydra split of responsibilities more closely:
    /// `HdRenderIndex::SyncAll` owns rprim state population, while the viewer
    /// reads the resulting mesh state afterwards for framing, picking, and
    /// instance transform caches.
    pub(super) fn sync_render_index_state(&mut self) {
        usd_trace::trace_scope!("engine_render_index_state");
        log::debug!("[engine] sync_render_index_state called");

        let Some(index) = &self.render_index else {
            return;
        };

        let stage = self
            .scene_indices
            .as_ref()
            .and_then(|indices| indices.stage_scene_index.get_stage());
        let mut rprim_ids_by_path_out: HashMap<Path, i32> = HashMap::new();
        let mut bbox_min = [f32::MAX; 3];
        let mut bbox_max = [f32::MIN; 3];
        let mut has_any_bounds = false;
        let mut mesh_count = 0u32;
        let mut instanced_mesh_count = 0u32;
        let mut t_materials = std::time::Duration::ZERO;
        let mut t_textures = std::time::Duration::ZERO;
        let mut t_bbox = std::time::Duration::ZERO;

        let mut index_guard: std::sync::MutexGuard<'_, HdRenderIndex> =
            index.lock().expect("Mutex poisoned");
        let prim_ids: Vec<_> = index_guard.get_rprim_ids();
        let mat_token = usd_tf::Token::new("material");
        let scene_delegate = index_guard.get_scene_index_adapter_scene_delegate();

        // Only rebuild rprim ID map when structural changes occurred (add/remove).
        // IDs don't change on time ticks.
        if self.rprim_ids_dirty {
            for prim_path in &prim_ids {
                if let Some(prim_id) = index_guard.get_prim_id_for_rprim_path(prim_path) {
                    rprim_ids_by_path_out.insert(prim_path.clone(), prim_id);
                }
            }
        }

        for prim_id in prim_ids {
            let rprim_type = index_guard
                .get_rprim_type_id(&prim_id)
                .map(|t| t.as_str().to_owned());
            let is_mesh = rprim_type.as_deref() == Some("mesh");

            // Non-mesh rprims (basisCurves, points): transforms flow through draw
            // items (set during rprim sync via update_draw_items). No viewer-side
            // bookkeeping needed (ARCH-03).
            if !is_mesh {
                continue;
            }

            let resolved_material = scene_delegate.as_ref().and_then(|delegate| {
                let mat_id = delegate.get_material_id(&prim_id);

                // Stage-based fallback: if scene index doesn't resolve material,
                // check USD prim directly (direct binding + GeomSubset children)
                let mat_id =
                    mat_id.or_else(|| resolve_material_from_stage(stage.as_ref()?, &prim_id));

                mat_id.and_then(|mat_path| {
                    let cached = self
                        .material_cache
                        .get(&mat_path)
                        .cloned()
                        .map(|(params, features, tex_paths)| {
                            (mat_path.clone(), params, features, tex_paths)
                        })
                        .or_else(|| {
                            index_guard
                                .get_sprim(&mat_token, &mat_path)
                                .and_then(|handle| handle.downcast_ref::<HdStMaterial>())
                                .map(|st_mat| {
                                    (
                                        mat_path.clone(),
                                        st_mat.get_material_params().clone(),
                                        (st_mat.has_ptex(), st_mat.has_limit_surface_evaluation()),
                                        st_mat.get_texture_paths().clone(),
                                    )
                                })
                        });
                    cached.map(|(cache_path, params, features, tex_paths)| {
                        self.material_cache.insert(
                            cache_path.clone(),
                            (params.clone(), features, tex_paths.clone()),
                        );
                        (params, features, tex_paths)
                    })
                })
            });

            let instancer_xforms = {
                let instancer_id = index_guard.get_instancer_id_for_rprim(&prim_id);
                if instancer_id.is_empty() {
                    None
                } else {
                    index_guard
                        .get_instancer(&instancer_id)
                        .and_then(|instancer| {
                            instancer.get_delegate().map(|delegate| {
                                decode_instance_transforms(delegate, &instancer_id, &prim_id)
                            })
                        })
                        .filter(|xforms| xforms.len() > 1)
                }
            };

            // Prefer the typed sync handle (RprimAdapter<HdStMesh>) when available — it is
            // the mesh that was actually synced via HdRprim::sync. Fall back to the opaque
            // handle for backends that use the sync_rprim dispatch path.
            let mesh: &mut HdStMesh =
                if let Some(sh) = index_guard.get_rprim_sync_handle_mut(&prim_id) {
                    let any = sh.as_any_mut();
                    if let Some(adapter) =
                        any.downcast_mut::<usd_hd::render::render_index::RprimAdapter<HdStMesh>>()
                    {
                        &mut adapter.0
                    } else {
                        continue;
                    }
                } else if let Some(handle) = index_guard.get_rprim_handle_mut(&prim_id) {
                    if let Some(m) =
                        (handle.as_mut() as &mut dyn std::any::Any).downcast_mut::<HdStMesh>()
                    {
                        m
                    } else {
                        continue;
                    }
                } else {
                    continue;
                };

            let material_t0 = std::time::Instant::now();
            mesh.set_material_features(false, false);
            if let Some((mut params, features, tex_paths)) = resolved_material {
                mesh.set_material_params(params.clone());
                mesh.set_material_features(features.0, features.1);

                let texture_t0 = std::time::Instant::now();
                #[cfg(feature = "wgpu")]
                if let (Some(hgi_arc), Some(stage)) = (&self.wgpu_hgi, stage.as_ref()) {
                    if !tex_paths.is_empty() {
                        let tex_handles = load_mesh_textures(
                            &tex_paths,
                            hgi_arc,
                            &prim_id,
                            stage,
                            &mut self.texture_cache,
                        );
                        params.has_diffuse_tex = tex_handles
                            .textures
                            .first()
                            .map(|h| h.is_valid())
                            .unwrap_or(false);
                        params.has_normal_tex = tex_handles
                            .textures
                            .get(1)
                            .map(|h| h.is_valid())
                            .unwrap_or(false);
                        params.has_roughness_tex = tex_handles
                            .textures
                            .get(2)
                            .map(|h| h.is_valid())
                            .unwrap_or(false);
                        params.has_metallic_tex = tex_handles
                            .textures
                            .get(3)
                            .map(|h| h.is_valid())
                            .unwrap_or(false);
                        params.has_opacity_tex = tex_handles
                            .textures
                            .get(4)
                            .map(|h| h.is_valid())
                            .unwrap_or(false);
                        params.has_emissive_tex = tex_handles
                            .textures
                            .get(5)
                            .map(|h| h.is_valid())
                            .unwrap_or(false);
                        params.has_occlusion_tex = tex_handles
                            .textures
                            .get(6)
                            .map(|h| h.is_valid())
                            .unwrap_or(false);
                        mesh.set_material_params(params);
                        mesh.set_texture_handles(tex_handles);
                    }
                }
                t_textures += texture_t0.elapsed();
                mesh.refresh_draw_item_bindings();
            }
            t_materials += material_t0.elapsed();

            let world_transform = *mesh.get_world_transform();
            let local_bbox = mesh.get_local_bbox();
            let bbox_t0 = std::time::Instant::now();

            if let Some(xforms) = instancer_xforms {
                mesh.create_instance_draw_items(xforms.len());
                for (i, xf) in xforms.iter().enumerate() {
                    if let Some(inst_path) = prim_id.append_child(&format!("__inst_{}", i)) {
                        self.model_transforms.insert(inst_path, *xf);
                        has_any_bounds |= accumulate_world_bounds(
                            local_bbox.0,
                            local_bbox.1,
                            xf,
                            &mut bbox_min,
                            &mut bbox_max,
                        );
                    }
                }
                instanced_mesh_count += 1;
                mesh_count += 1;
                t_bbox += bbox_t0.elapsed();
                continue;
            }

            // Populate model_transforms from the already-read world_transform.
            // Zero additional cost — transform was fetched above for bbox.
            self.model_transforms
                .insert(prim_id.clone(), world_transform);

            has_any_bounds |= accumulate_world_bounds(
                local_bbox.0,
                local_bbox.1,
                &world_transform,
                &mut bbox_min,
                &mut bbox_max,
            );
            t_bbox += bbox_t0.elapsed();
            mesh_count += 1;
        }

        drop(index_guard);

        // Only update bbox when dirty (structural change, not animation).
        if self.scene_bbox_dirty {
            self.scene_bbox = has_any_bounds.then_some((bbox_min, bbox_max));
            self.scene_bbox_dirty = false;
        }
        if self.rprim_ids_dirty {
            self.rprim_ids_by_path = rprim_ids_by_path_out;
            self.rprim_ids_dirty = false;
        }

        log::trace!(
            "[PERF] render_index_state: meshes={} instanced={} materials={:.1}ms textures={:.1}ms bbox={:.1}ms",
            mesh_count,
            instanced_mesh_count,
            t_materials.as_secs_f64() * 1000.0,
            t_textures.as_secs_f64() * 1000.0,
            t_bbox.as_secs_f64() * 1000.0,
        );
    }

    /// Incrementally update only dirty rprim transforms after `HdEngine::execute`.
    ///
    /// Instead of rebuilding the entire `model_transforms` HashMap from scratch,
    /// only re-reads world transforms for rprims that had `DIRTY_TRANSFORM` set.
    /// `rprim_ids_by_path` is left untouched (IDs don't change on time ticks).
    /// `scene_bbox` is recomputed from all rprims since individual bounds may
    /// have shrunk (incremental expand-only would accumulate stale maxima).
    pub(super) fn update_dirty_transforms(&mut self, dirty_paths: &[Path]) {
        usd_trace::trace_scope!("engine_update_dirty_transforms");
        let t0 = std::time::Instant::now();

        let Some(index) = &self.render_index else {
            return;
        };

        let mut index_guard = index.lock().expect("Mutex poisoned");
        let scene_delegate = index_guard.get_scene_index_adapter_scene_delegate();

        let mut updated = 0u32;
        for prim_id in dirty_paths {
            let rprim_type = index_guard
                .get_rprim_type_id(prim_id)
                .map(|t| t.as_str().to_owned());
            let is_mesh = rprim_type.as_deref() == Some("mesh");

            if !is_mesh {
                // Non-mesh rprim: re-read from scene delegate (no cached world_transform)
                if let Some(delegate) = scene_delegate.as_ref() {
                    let m = delegate.get_transform(prim_id);
                    self.model_transforms.insert(
                        prim_id.clone(),
                        [
                            [m[0][0], m[0][1], m[0][2], m[0][3]],
                            [m[1][0], m[1][1], m[1][2], m[1][3]],
                            [m[2][0], m[2][1], m[2][2], m[2][3]],
                            [m[3][0], m[3][1], m[3][2], m[3][3]],
                        ],
                    );
                    updated += 1;
                }
                continue;
            }

            // Mesh rprim: read cached world_transform from HdStMesh (set during sync)
            let mesh: &HdStMesh = if let Some(sh) = index_guard.get_rprim_sync_handle_mut(prim_id) {
                let any = sh.as_any_mut();
                if let Some(adapter) =
                    any.downcast_mut::<usd_hd::render::render_index::RprimAdapter<HdStMesh>>()
                {
                    &adapter.0
                } else {
                    continue;
                }
            } else if let Some(handle) = index_guard.get_rprim_handle_mut(prim_id) {
                if let Some(m) =
                    (handle.as_mut() as &mut dyn std::any::Any).downcast_mut::<HdStMesh>()
                {
                    m
                } else {
                    continue;
                }
            } else {
                continue;
            };

            let world_transform = *mesh.get_world_transform();

            // Handle instancing
            let instancer_id = index_guard.get_instancer_id_for_rprim(prim_id);
            if !instancer_id.is_empty() {
                if let Some(xforms) = index_guard
                    .get_instancer(&instancer_id)
                    .and_then(|instancer| {
                        instancer.get_delegate().map(|delegate| {
                            decode_instance_transforms(delegate, &instancer_id, prim_id)
                        })
                    })
                    .filter(|xforms| xforms.len() > 1)
                {
                    for (i, xf) in xforms.iter().enumerate() {
                        if let Some(inst_path) = prim_id.append_child(&format!("__inst_{}", i)) {
                            self.model_transforms.insert(inst_path, *xf);
                        }
                    }
                    updated += 1;
                    continue;
                }
            }

            self.model_transforms
                .insert(prim_id.clone(), world_transform);
            updated += 1;
        }

        drop(index_guard);

        // Recompute scene bbox from all current model_transforms + local extents.
        // Required because rprims that moved may have shrunk the overall bounds.
        self.recompute_scene_bbox();

        log::trace!(
            "[PERF] update_dirty_transforms: dirty={} updated={} total_transforms={} bbox={:.1}ms total={:.1}ms",
            dirty_paths.len(),
            updated,
            self.model_transforms.len(),
            0.0, // bbox timing is inside recompute_scene_bbox
            t0.elapsed().as_secs_f64() * 1000.0,
        );
    }

    /// Recompute `scene_bbox` from all model_transforms + HdStMesh local extents.
    ///
    /// Separated so it can be called from both full and incremental paths.
    fn recompute_scene_bbox(&mut self) {
        let Some(index) = &self.render_index else {
            return;
        };

        let mut bbox_min = [f32::MAX; 3];
        let mut bbox_max = [f32::MIN; 3];
        let mut has_any_bounds = false;

        let mut index_guard = index.lock().expect("Mutex poisoned");

        for prim_id in index_guard.get_rprim_ids() {
            let mesh: &HdStMesh = if let Some(sh) = index_guard.get_rprim_sync_handle_mut(&prim_id)
            {
                let any = sh.as_any_mut();
                if let Some(adapter) =
                    any.downcast_mut::<usd_hd::render::render_index::RprimAdapter<HdStMesh>>()
                {
                    &adapter.0
                } else {
                    // Non-mesh: use model_transforms entry for bbox
                    if let Some(xf) = self.model_transforms.get(&prim_id) {
                        // No local bbox for non-mesh; skip
                        let _ = xf;
                    }
                    continue;
                }
            } else {
                continue;
            };

            let world_transform = *mesh.get_world_transform();
            let local_bbox = mesh.get_local_bbox();

            has_any_bounds |= accumulate_world_bounds(
                local_bbox.0,
                local_bbox.1,
                &world_transform,
                &mut bbox_min,
                &mut bbox_max,
            );
        }

        drop(index_guard);
        self.scene_bbox = has_any_bounds.then_some((bbox_min, bbox_max));
    }
}

/// Stage-based material resolution fallback.
///
/// When the scene index pipeline doesn't resolve a material binding (e.g. because
/// the binding is on a GeomSubset child, not on the Mesh itself), we fall back
/// to querying the USD stage directly.
///
/// Resolution order:
/// 1. Direct `material:binding` on the mesh prim
/// 2. First GeomSubset child with a `material:binding`
/// 3. Walk up ancestors for inherited binding
fn resolve_material_from_stage(
    stage: &std::sync::Arc<usd_core::Stage>,
    prim_path: &Path,
) -> Option<Path> {
    let prim = stage.get_prim_at_path(prim_path)?;
    let all_purpose = usd_tf::Token::new("");

    // 1. Direct binding on mesh itself
    let binding_api = MaterialBindingAPI::new(prim.clone());
    let direct = binding_api.get_direct_binding(&all_purpose);
    if direct.is_bound() {
        let mat_path = direct.get_material_path();
        if !mat_path.is_empty() {
            return Some(mat_path.clone());
        }
    }

    // 2. GeomSubset children — use first one that has a material binding
    for child in prim.get_all_children() {
        if child.get_type_name().as_str() == "GeomSubset" {
            let child_api = MaterialBindingAPI::new(child.clone());
            let child_binding = child_api.get_direct_binding(&all_purpose);
            if child_binding.is_bound() {
                let mat_path = child_binding.get_material_path();
                if !mat_path.is_empty() {
                    return Some(mat_path.clone());
                }
            }
        }
    }

    // 3. Walk ancestors for inherited binding
    let mut ancestor_path = prim_path.get_parent_path();
    while !ancestor_path.is_empty() && !ancestor_path.is_absolute_root_path() {
        if let Some(ancestor) = stage.get_prim_at_path(&ancestor_path) {
            let ancestor_api = MaterialBindingAPI::new(ancestor);
            let ancestor_binding = ancestor_api.get_direct_binding(&all_purpose);
            if ancestor_binding.is_bound() {
                let mat_path = ancestor_binding.get_material_path();
                if !mat_path.is_empty() {
                    return Some(mat_path.clone());
                }
            }
        }
        ancestor_path = ancestor_path.get_parent_path();
    }

    None
}
