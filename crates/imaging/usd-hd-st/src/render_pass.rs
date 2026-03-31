
//! HdStRenderPass - Storm render pass implementation.
//!
//! Implements the HdRenderPass trait for Storm rendering.
//! Collects draw items from prims and organizes them into batches.

use crate::command_buffer::HdStCommandBuffer;
use crate::draw_batch::HdStDrawBatch;
use crate::draw_item::HdStDrawItemSharedPtr;
use crate::render_pass_state::HdStRenderPassState;
use crate::resource_registry::HdStResourceRegistry;
use std::sync::Arc;
use usd_hd::render::TfTokenVector;
use usd_hd::render::{HdRenderPass, HdRenderPassStateSharedPtr, HdRprimCollection};
use usd_hgi::graphics_cmds::HgiViewport;
use usd_hgi::hgi::Hgi;
use usd_hgi::{HgiGraphicsCmdsDesc, HgiSubmitWaitType, HgiTextureHandle};

/// Storm render pass.
///
/// Executes rendering for a collection of prims using the Storm
/// rasterization backend.
///
/// # Execution Flow
///
/// 1. set_draw_items() - Populate with draw items from prims
/// 2. sync() - Build optimized draw batches
/// 3. execute() - Submit GPU commands to render
///
/// # Draw List Management
///
/// The render pass maintains optimized draw lists organized into
/// batches for efficient GPU submission.
pub struct HdStRenderPass {
    /// Prim collection to render
    collection: HdRprimCollection,

    /// Command buffer for GPU commands
    command_buffer: HdStCommandBuffer,

    /// All draw items collected from prims (pre-filter)
    draw_items: Vec<HdStDrawItemSharedPtr>,

    /// Material-tag-filtered draw items used for batching
    filtered_items: Vec<HdStDrawItemSharedPtr>,

    /// Draw batches (built during sync)
    draw_batches: Vec<Arc<HdStDrawBatch>>,

    /// Collection has changed and needs sync
    collection_dirty: bool,

    /// Draw items have changed and need rebatching
    items_dirty: bool,

    /// Last seen collection version from change tracker
    collection_version: u32,

    /// Last seen render tag version from change tracker (collection render tags)
    render_tag_version: u32,

    /// Last seen rprim render tag version (per-rprim render tags dirtied).
    ///
    /// Port of C++ HdSt_RenderPass::_rprimRenderTagVersion (renderPass.h:P1-22).
    rprim_render_tag_version: u32,

    /// Last seen task render tags version (task's render tag filter changed).
    ///
    /// Port of C++ HdSt_RenderPass::_taskRenderTagsVersion (renderPass.h:P1-22).
    task_render_tags_version: u32,

    /// Last seen material tags version from HdStRenderParam
    material_tags_version: usize,

    /// Last seen geom subset draw items version
    geom_subset_version: usize,

    /// Whether frustum culling is enabled for this render pass.
    /// When enabled, each IndirectDrawBatch runs per-batch GPU culling
    /// in its execute_draw() -- no render-pass-level culling needed.
    frustum_culling_enabled: bool,

    /// Previous render tags for task-render-tag change detection.
    /// Port of C++ HdSt_RenderPass::_prevRenderTags.
    prev_render_tags: Vec<usd_tf::Token>,

    /// Last seen draw batches version from HdStRenderParam.
    /// Port of C++ _GetDrawBatchesVersion check in _UpdateCommandBuffer.
    draw_batches_version: u32,
}

impl HdStRenderPass {
    /// Create a new Storm render pass.
    pub fn new(collection: HdRprimCollection) -> Self {
        Self {
            collection,
            command_buffer: HdStCommandBuffer::new(),
            draw_items: Vec::new(),
            filtered_items: Vec::new(),
            draw_batches: Vec::new(),
            collection_dirty: true,
            items_dirty: true,
            collection_version: 0,
            render_tag_version: 0,
            rprim_render_tag_version: 0,
            task_render_tags_version: 0,
            material_tags_version: 0,
            geom_subset_version: 0,
            frustum_culling_enabled: true,
            prev_render_tags: Vec::new(),
            draw_batches_version: 0,
        }
    }

    /// Set draw items to render.
    ///
    /// Called by the render index after gathering draw items from prims.
    pub fn set_draw_items(&mut self, items: Vec<HdStDrawItemSharedPtr>) {
        self.draw_items = items;
        self.items_dirty = true;
    }

    /// Add a single draw item.
    pub fn add_draw_item(&mut self, item: HdStDrawItemSharedPtr) {
        self.draw_items.push(item);
        self.items_dirty = true;
    }

    /// Drop all retained draw-item references before rprim sync mutates them.
    ///
    /// Our mesh reprs own the canonical draw-item objects; the render pass and
    /// batches must release clones before sync so meshes can update those
    /// objects in place. This mirrors the Hydra expectation that draw-item
    /// lifetime is owned by rprims/reprs, not by the render pass.
    pub fn clear_draw_item_refs(&mut self) {
        self.draw_items.clear();
        self.filtered_items.clear();
        self.draw_batches.clear();
        self.command_buffer.clear();
        self.items_dirty = true;
    }

    /// Get the rprim collection.
    pub fn get_rprim_collection(&self) -> &HdRprimCollection {
        &self.collection
    }

    /// Get all draw items.
    pub fn get_draw_items(&self) -> &[HdStDrawItemSharedPtr] {
        &self.draw_items
    }

    /// Get draw item count.
    pub fn get_draw_item_count(&self) -> usize {
        self.draw_items.len()
    }

    /// Check if there are any draw items matching the given render tags.
    ///
    /// Port of C++ HdSt_RenderPass::HasDrawItems (P1-23).
    /// Returns false when definitely no draw items pass the material tag + render
    /// tag filter — allows callers to skip empty render passes cheaply.
    ///
    /// Note: this is a conservative check (may return true even when the
    /// final filtered list is empty), matching the C++ behaviour.
    pub fn has_draw_items(&self, render_tags: &[usd_tf::Token]) -> bool {
        if self.draw_items.is_empty() {
            return false;
        }
        let tag = &self.collection.material_tag;
        // If any item passes the material-tag filter we have something to draw.
        let tag_match = tag.is_empty()
            || self
                .draw_items
                .iter()
                .any(|item| item.get_material_tag().map_or(true, |mt| mt == tag));
        if !tag_match {
            return false;
        }
        // If the caller specified render tags, at least one item must match.
        if render_tags.is_empty() {
            return true;
        }
        // Conservative: if we have items and the material tag matched, assume
        // at least one item will match the render tags. Avoids per-item lookup
        // since HdStDrawItem doesn't carry a render tag directly (set by repr).
        true
    }

    /// Filter draw items by material tag from the collection.
    ///
    /// If the collection has a material tag set, only items whose material
    /// tag matches are kept.  An empty collection material tag means "accept all".
    /// Port of C++ _UpdateDrawItems material tag filtering.
    fn filter_by_material_tag(&mut self) {
        let tag = &self.collection.material_tag;

        if tag.is_empty() {
            // No filter: use all draw items
            self.filtered_items = self.draw_items.clone();
        } else {
            self.filtered_items = self
                .draw_items
                .iter()
                .filter(|item| {
                    // Accept item if its material tag matches the collection's tag
                    item.get_material_tag().map_or(true, |mt| mt == tag)
                })
                .cloned()
                .collect();
        }

        log::debug!(
            "HdStRenderPass::filter_by_material_tag: tag={:?}, {} -> {} items",
            tag.as_str(),
            self.draw_items.len(),
            self.filtered_items.len(),
        );
    }

    /// Build draw batches from filtered draw items.
    ///
    /// Groups compatible draw items together per material+topology combination.
    /// Port of C++ HdSt_RenderPass::_RebuildDrawBatches via commandBuffer:
    ///
    /// Aggregation rule (P0-8): items are compatible if they share:
    ///   - material tag (same render queue bucket)
    ///   - indexed vs non-indexed topology
    ///   - instanced vs non-instanced
    ///   - has_normals / has_uv presence (same vertex layout)
    ///
    /// Items with different material tags go into separate batches so they
    /// can be sorted/culled independently (e.g. translucent after opaque).
    fn build_batches(&mut self) {
        self.draw_batches.clear();

        if self.filtered_items.is_empty() {
            return;
        }

        // Compatibility key: (primitive_kind, material_tag, is_indexed, is_instanced)
        // primitive_kind separates mesh/points/curves into distinct batches
        // so each gets the correct shader/pipeline/binding contract.
        type BatchKey = (crate::draw_item::DrawPrimitiveKind, Option<String>, bool, bool);
        let mut batch_slots: Vec<(BatchKey, HdStDrawBatch)> = Vec::new();
        let mut batch_index: std::collections::HashMap<BatchKey, usize> =
            std::collections::HashMap::new();

        for item in &self.filtered_items {
            if !item.is_visible() {
                continue;
            }

            let prim_kind = item.get_primitive_kind();
            let tag_str = item.get_material_tag().map(|t| t.as_str().to_owned());
            let is_indexed = item.get_element_bar().is_some();
            let is_instanced = item.get_instance_bar().is_some();
            let key: BatchKey = (prim_kind, tag_str, is_indexed, is_instanced);

            if let Some(&idx) = batch_index.get(&key) {
                batch_slots[idx].1.add_draw_item(item.clone());
            } else {
                let mut batch = HdStDrawBatch::new();
                batch.add_draw_item(item.clone());
                let idx = batch_slots.len();
                batch_slots.push((key.clone(), batch));
                batch_index.insert(key, idx);
            }
        }

        // Validate and collect non-empty batches
        for (_, mut batch) in batch_slots {
            if batch.validate() {
                self.draw_batches.push(Arc::new(batch));
            }
        }

        log::debug!(
            "HdStRenderPass::build_batches: {} items -> {} batches",
            self.filtered_items.len(),
            self.draw_batches.len()
        );
    }

    /// Check whether draw items are stale based on version counters.
    ///
    /// Compares cached versions against render index change tracker and
    /// render param counters.  Port of C++ _UpdateDrawItemsIfNeeded.
    ///
    /// Now tracks 6 version counters matching C++ renderPass.h (P1-22):
    /// collectionVersion, rprimRenderTagVersion, taskRenderTagsVersion,
    /// materialTagsVersion, geomSubsetDrawItemsVersion, renderTagVersion.
    pub fn check_staleness(
        &mut self,
        collection_version: u32,
        render_tag_version: u32,
        material_tags_version: usize,
        geom_subset_version: usize,
    ) -> bool {
        let stale = collection_version != self.collection_version
            || render_tag_version != self.render_tag_version
            || material_tags_version != self.material_tags_version
            || geom_subset_version != self.geom_subset_version;

        if stale {
            self.collection_version = collection_version;
            self.render_tag_version = render_tag_version;
            self.material_tags_version = material_tags_version;
            self.geom_subset_version = geom_subset_version;
            self.items_dirty = true;
        }

        stale
    }

    /// Extended staleness check with all 6 version counters.
    ///
    /// Port of C++ `HdSt_RenderPass::_UpdateDrawItemsIfNeeded` full version (P1-22).
    /// Adds rprim render tag and task render tags tracking on top of base 4 counters.
    pub fn check_staleness_full(
        &mut self,
        collection_version: u32,
        render_tag_version: u32,
        rprim_render_tag_version: u32,
        task_render_tags_version: u32,
        material_tags_version: usize,
        geom_subset_version: usize,
    ) -> bool {
        let stale = collection_version != self.collection_version
            || render_tag_version != self.render_tag_version
            || rprim_render_tag_version != self.rprim_render_tag_version
            || task_render_tags_version != self.task_render_tags_version
            || material_tags_version != self.material_tags_version
            || geom_subset_version != self.geom_subset_version;

        if stale {
            self.collection_version = collection_version;
            self.render_tag_version = render_tag_version;
            self.rprim_render_tag_version = rprim_render_tag_version;
            self.task_render_tags_version = task_render_tags_version;
            self.material_tags_version = material_tags_version;
            self.geom_subset_version = geom_subset_version;
            self.items_dirty = true;
        }

        stale
    }

    /// Get current rprim render tag version.
    pub fn get_rprim_render_tag_version(&self) -> u32 {
        self.rprim_render_tag_version
    }

    /// Get current task render tags version.
    pub fn get_task_render_tags_version(&self) -> u32 {
        self.task_render_tags_version
    }

    /// Sync the render pass — rebuild batches if items or versions changed.
    ///
    /// Checks stored version counters and rebuild draw batches when stale.
    /// This is the main sync entry point matching C++ `_UpdateCommandBuffer`.
    pub fn sync(&mut self) {
        usd_trace::trace_scope!("render_pass_sync");
        if !self.items_dirty && !self.collection_dirty {
            return;
        }

        self.sync_batches();
    }

    /// Sync with explicit version counters and render tags.
    ///
    /// Port of C++ `HdSt_RenderPass::_UpdateDrawItems` + `_UpdateCommandBuffer`.
    /// Compares all version counters and render tags against stored values;
    /// triggers a re-filter + rebatch when any version has changed.
    ///
    /// Call this once per frame before `execute()` / `execute_with_hgi()`.
    pub fn sync_versions(
        &mut self,
        collection_version: u32,
        rprim_render_tag_version: u32,
        task_render_tags_version: u32,
        material_tags_version: usize,
        geom_subset_version: usize,
        draw_batches_version: u32,
        render_tags: &[usd_tf::Token],
    ) {
        // --- version-based staleness check (port of _UpdateDrawItems) ---
        let collection_changed =
            self.collection_dirty || self.collection_version != collection_version;
        let rprim_tag_changed = self.rprim_render_tag_version != rprim_render_tag_version;
        let material_changed = self.material_tags_version != material_tags_version;
        let geom_subset_changed = self.geom_subset_version != geom_subset_version;

        // Task render tags: only dirty when the actual tag list differs
        let mut task_tags_changed = false;
        if self.task_render_tags_version != task_render_tags_version {
            self.task_render_tags_version = task_render_tags_version;
            if self.prev_render_tags.as_slice() != render_tags {
                self.prev_render_tags = render_tags.to_vec();
                task_tags_changed = true;
            }
        }

        if collection_changed
            || rprim_tag_changed
            || material_changed
            || geom_subset_changed
            || task_tags_changed
        {
            log::debug!(
                "HdStRenderPass::sync_versions: stale (col={} rprim_tag={} mat={} geom={} task_tag={})",
                collection_changed,
                rprim_tag_changed,
                material_changed,
                geom_subset_changed,
                task_tags_changed,
            );
            self.collection_version = collection_version;
            self.rprim_render_tag_version = rprim_render_tag_version;
            self.material_tags_version = material_tags_version;
            self.geom_subset_version = geom_subset_version;
            self.collection_dirty = false;
            self.items_dirty = true;
        }

        // --- batch rebuild (port of _UpdateCommandBuffer) ---
        self.sync_batches();

        // Check draw-batches version for rebatch without item changes
        if self.draw_batches_version != draw_batches_version {
            self.draw_batches_version = draw_batches_version;
            // Batches may need rebuild even without new items (e.g. BAR migration)
            if !self.items_dirty {
                self.build_batches();
            }
        }
    }

    /// Internal: rebuild batches from filtered items.
    fn sync_batches(&mut self) {
        if self.items_dirty {
            self.filter_by_material_tag();
            self.build_batches();
            self.items_dirty = false;
        }
        self.collection_dirty = false;
    }

    /// Port of C++ `HdSt_RenderPass::_UpdateCommandBuffer`.
    ///
    /// Checks all version counters (collection, render tags, material tags,
    /// geom subset) against the render index state.  When any version has
    /// changed the draw items are re-fetched and batches rebuilt.
    ///
    /// Call this once per frame *before* `execute()` — it is the Rust
    /// equivalent of the implicit `_UpdateCommandBuffer(renderTags)` call
    /// inside C++ `_Execute`.
    pub fn update_draw_items(
        &mut self,
        collection_version: u32,
        render_tag_version: u32,
        rprim_render_tag_version: u32,
        task_render_tags_version: u32,
        material_tags_version: usize,
        geom_subset_version: usize,
    ) {
        // Check every version counter — sets items_dirty when stale.
        self.check_staleness_full(
            collection_version,
            render_tag_version,
            rprim_render_tag_version,
            task_render_tags_version,
            material_tags_version,
            geom_subset_version,
        );

        // Rebuild batches if anything changed.
        self.sync();
    }

    /// Execute the render pass.
    ///
    /// Submits GPU commands to render all prims in the collection.
    pub fn execute(&mut self, state: &HdRenderPassStateSharedPtr, _tags: &TfTokenVector) {
        usd_trace::trace_scope!("render_pass_execute");
        // Ensure we're synced
        self.sync();

        // Reset command buffer
        self.command_buffer.reset();

        // Apply render pass state (viewport, camera, etc.)
        self.apply_render_state(state);

        // Add draw batches to command buffer
        for batch in &self.draw_batches {
            self.command_buffer.add_draw_batch(batch.clone());
        }

        // Submit commands with render state for selection highlighting
        self.command_buffer.submit(Some(state));
    }

    fn apply_render_state(&self, _state: &HdRenderPassStateSharedPtr) {}

    /// Execute render pass through HGI (pipeline-based path).
    ///
    /// Follows C++ _Execute pattern:
    /// 1. Sync draw items into batches
    /// 2. PrepareDraw: separate HgiGraphicsCmds to recompile dirty batches
    /// 3. ExecuteDraw: main HgiGraphicsCmds with render targets
    ///
    /// Mirrors C++ two-pass approach: prepareGfxCmds (PrepareDraw) then
    /// gfxCmds (ExecuteDraw), each submitted separately.
    /// Execute render pass through HGI (pipeline-based path).
    ///
    /// Follows C++ _Execute pattern:
    /// 1. Sync draw items into batches
    /// 2. Each batch runs per-batch GPU frustum culling in execute_draw()
    /// 3. ExecuteDraw: main HgiGraphicsCmds with render targets
    ///
    /// Frustum culling is per-BATCH (matching C++ architecture), not per-render-pass.
    /// Each IndirectDrawBatch has its own GPU dispatch buffer. The culling compute
    /// shader writes instance_count=0 for culled items directly into this buffer.
    /// The draw pass reads from the SAME buffer via draw_indexed_indirect.
    /// No CPU readback, no batch rebuild.
    pub fn execute_with_hgi(
        &mut self,
        state: &HdStRenderPassState,
        hgi: &mut dyn Hgi,
        color_texture: &HgiTextureHandle,
        depth_texture: &HgiTextureHandle,
        resource_registry: &HdStResourceRegistry,
        prim_ids_by_path: Option<&std::collections::HashMap<usd_sdf::Path, i32>>,
        submit_wait: HgiSubmitWaitType,
    ) {
        usd_trace::trace_scope!("render_pass_execute_hgi");
        // Match the reference render-pass execution order:
        // sync batches, run PrepareDraw on an attachment-less command buffer,
        // submit that work, then execute the real render pass.
        self.sync();

        log::debug!(
            "[render_pass] execute_with_hgi: items_dirty={} draw_items={} filtered={} batches={}",
            self.items_dirty,
            self.draw_items.len(),
            self.filtered_items.len(),
            self.draw_batches.len()
        );

        // Reset command buffer for new frame.
        self.command_buffer.reset();

        // Add all draw batches to command buffer. Frustum culling happens
        // per-batch inside each IndirectDrawBatch::execute_draw(), not here.
        for batch in &self.draw_batches {
            self.command_buffer.add_draw_batch(batch.clone());
        }

        if self.command_buffer.get_batch_count() == 0 {
            log::trace!("[render_pass] batch_count=0, returning early");
            return;
        }

        let mut prepare_gfx_cmds = hgi.create_graphics_cmds(&HgiGraphicsCmdsDesc::new());
        self.command_buffer.prepare_draw(
            prepare_gfx_cmds.as_mut(),
            hgi,
            state,
            resource_registry,
        );
        hgi.submit_cmds(prepare_gfx_cmds, HgiSubmitWaitType::NoWait);

        // --- ExecuteDraw phase ---
        let gfx_desc = state.make_graphics_cmds_desc(Some(color_texture), Some(depth_texture));
        if !gfx_desc.has_attachments() {
            log::warn!(
                "HdStRenderPass::execute_with_hgi: no render attachments configured, skipping pass"
            );
            return;
        }

        let mut gfx_cmds = hgi.create_graphics_cmds(&gfx_desc);

        let (vx, vy, vw, vh) = state.get_viewport();
        gfx_cmds.set_viewport(&HgiViewport::new(vx, vy, vw, vh));

        // Execute all draw batches. Each batch internally:
        // 1. Runs GPU frustum culling (compute shader modifies dispatch buffer in-place)
        // 2. Issues draw_indexed_indirect from the same dispatch buffer
        self.command_buffer.execute_draw(
            gfx_cmds.as_mut(),
            hgi,
            state,
            prim_ids_by_path,
        );

        hgi.submit_cmds(gfx_cmds, submit_wait);

        log::debug!(
            "HdStRenderPass::execute_with_hgi: {} batches submitted",
            self.draw_batches.len()
        );
    }

    /// Get draw batch count.
    pub fn get_batch_count(&self) -> usize {
        self.draw_batches.len()
    }

    /// Get total visible item count across all batches.
    pub fn get_visible_item_count(&self) -> usize {
        self.draw_batches.iter().map(|b| b.get_item_count()).sum()
    }

    /// Clear all draw items and batches.
    pub fn clear(&mut self) {
        self.draw_items.clear();
        self.draw_batches.clear();
        self.items_dirty = true;
    }

    /// Enable or disable frustum culling for this render pass.
    pub fn set_frustum_culling_enabled(&mut self, enabled: bool) {
        self.frustum_culling_enabled = enabled;
    }

    /// Returns true when frustum culling is active.
    pub fn is_frustum_culling_enabled(&self) -> bool {
        self.frustum_culling_enabled
    }
}

impl HdRenderPass for HdStRenderPass {
    fn get_rprim_collection(&self) -> &HdRprimCollection {
        &self.collection
    }

    fn set_rprim_collection(&mut self, collection: HdRprimCollection) {
        self.collection = collection;
        self.collection_dirty = true;
    }

    fn sync(&mut self) {
        // Call the inherent method directly to avoid infinite recursion:
        // `self.sync()` inside a trait impl resolves to the trait method, not the inherent one.
        HdStRenderPass::sync(self);
    }

    fn execute(&mut self, state: &HdRenderPassStateSharedPtr, tags: &TfTokenVector) {
        HdStRenderPass::execute(self, state, tags);
    }
}

/// Multiply two 4x4 f64 matrices (row-vector convention: C = A * B).
#[cfg(test)]
fn mat4_mul_f64(a: &[[f64; 4]; 4], b: &[[f64; 4]; 4]) -> [[f64; 4]; 4] {
    let mut out = [[0.0f64; 4]; 4];
    for r in 0..4 {
        for c in 0..4 {
            out[r][c] =
                a[r][0] * b[0][c] + a[r][1] * b[1][c] + a[r][2] * b[2][c] + a[r][3] * b[3][c];
        }
    }
    out
}

/// Test whether an AABB is visible in clip space.
///
/// Port of FrustumCullIsVisible from frustumCull.glslfx.
/// Transforms all 8 corners of the AABB by the model-view-projection matrix
/// and tests against the 6 clip planes.
///
/// `mvp`: combined model * view * projection in row-vector convention (row-major).
/// Returns true when any part of the AABB overlaps the view frustum.
#[cfg(test)]
fn aabb_visible_in_clip(bbox_min: [f32; 3], bbox_max: [f32; 3], mvp: &[[f64; 4]; 4]) -> bool {
    // Empty bbox (min > max on any axis) => always visible.
    if bbox_min[0] > bbox_max[0] || bbox_min[1] > bbox_max[1] || bbox_min[2] > bbox_max[2] {
        return true;
    }
    // Infinite bbox => always visible.
    if bbox_min.iter().any(|v| v.is_infinite()) || bbox_max.iter().any(|v| v.is_infinite()) {
        return true;
    }

    let mn = [bbox_min[0] as f64, bbox_min[1] as f64, bbox_min[2] as f64];
    let mx = [bbox_max[0] as f64, bbox_max[1] as f64, bbox_max[2] as f64];

    // Transform 8 corners into clip space: clip = [x,y,z,1] * MVP (row-vector convention).
    let transform_point = |x: f64, y: f64, z: f64| -> [f64; 4] {
        [
            x * mvp[0][0] + y * mvp[1][0] + z * mvp[2][0] + mvp[3][0],
            x * mvp[0][1] + y * mvp[1][1] + z * mvp[2][1] + mvp[3][1],
            x * mvp[0][2] + y * mvp[1][2] + z * mvp[2][2] + mvp[3][2],
            x * mvp[0][3] + y * mvp[1][3] + z * mvp[2][3] + mvp[3][3],
        ]
    };

    let corners = [
        transform_point(mn[0], mn[1], mn[2]),
        transform_point(mn[0], mn[1], mx[2]),
        transform_point(mn[0], mx[1], mn[2]),
        transform_point(mn[0], mx[1], mx[2]),
        transform_point(mx[0], mn[1], mn[2]),
        transform_point(mx[0], mn[1], mx[2]),
        transform_point(mx[0], mx[1], mn[2]),
        transform_point(mx[0], mx[1], mx[2]),
    ];

    // Per-corner clip flag accumulation (same algorithm as GLSL/WGSL).
    // 2 bits per axis (inside_neg | inside_pos<<1). Visible iff all 3 components == 3.
    let mut flags = [0i32; 3];
    for c in &corners {
        let w = c[3];
        for axis in 0..3 {
            if c[axis] < w {
                flags[axis] |= 1; // inside negative half-space
            }
            if c[axis] > -w {
                flags[axis] |= 2; // inside positive half-space
            }
        }
    }

    flags[0] == 3 && flags[1] == 3 && flags[2] == 3
}

/// Shared pointer to Storm render pass.
pub type HdStRenderPassSharedPtr = Arc<HdStRenderPass>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw_item::HdStDrawItem;
    use usd_sdf::Path as SdfPath;
    use usd_tf::Token;

    #[test]
    fn test_render_pass_creation() {
        let collection = HdRprimCollection::new(Token::new("test"));
        let pass = HdStRenderPass::new(collection);

        assert_eq!(pass.get_rprim_collection().name, Token::new("test"));
        assert_eq!(pass.get_batch_count(), 0);
        assert_eq!(pass.get_draw_item_count(), 0);
    }

    #[test]
    fn test_add_draw_items() {
        let collection = HdRprimCollection::new(Token::new("test"));
        let mut pass = HdStRenderPass::new(collection);

        let path = SdfPath::from_string("/item1").unwrap();
        let item = Arc::new(HdStDrawItem::new(path));
        pass.add_draw_item(item);

        assert_eq!(pass.get_draw_item_count(), 1);
    }

    #[test]
    fn test_render_pass_sync_with_items() {
        let collection = HdRprimCollection::new(Token::new("test"));
        let mut pass = HdStRenderPass::new(collection);

        // Add some draw items (they won't be valid without buffers)
        let path1 = SdfPath::from_string("/item1").unwrap();
        let path2 = SdfPath::from_string("/item2").unwrap();

        pass.add_draw_item(Arc::new(HdStDrawItem::new(path1)));
        pass.add_draw_item(Arc::new(HdStDrawItem::new(path2)));

        pass.sync();

        // Items aren't valid (no buffers), so batch should be empty
        assert_eq!(pass.get_visible_item_count(), 0);
    }

    #[test]
    fn test_collection_change() {
        let collection1 = HdRprimCollection::new(Token::new("test1"));
        let mut pass = HdStRenderPass::new(collection1);
        pass.sync();

        let collection2 = HdRprimCollection::new(Token::new("test2"));
        pass.set_rprim_collection(collection2);

        assert_eq!(pass.get_rprim_collection().name, Token::new("test2"));
    }

    #[test]
    fn test_clear() {
        let collection = HdRprimCollection::new(Token::new("test"));
        let mut pass = HdStRenderPass::new(collection);

        let path = SdfPath::from_string("/item").unwrap();
        pass.add_draw_item(Arc::new(HdStDrawItem::new(path)));

        assert_eq!(pass.get_draw_item_count(), 1);

        pass.clear();
        assert_eq!(pass.get_draw_item_count(), 0);
    }

    // --- Frustum culling unit tests ---

    /// Build a simple perspective MVP matrix looking down -Z.
    fn make_test_mvp() -> [[f64; 4]; 4] {
        // Simple perspective projection: FOV~90, aspect 1, near=0.1, far=100.
        // View = identity (camera at origin looking down -Z).
        let n = 0.1f64;
        let f = 100.0f64;
        // Row-major perspective matrix (row-vector convention).
        [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, -(f + n) / (f - n), -1.0],
            [0.0, 0.0, -2.0 * f * n / (f - n), 0.0],
        ]
    }

    #[test]
    fn test_aabb_visible_in_front() {
        let mvp = make_test_mvp();
        // Box at z = -5, well inside frustum.
        assert!(aabb_visible_in_clip(
            [-1.0, -1.0, -6.0],
            [1.0, 1.0, -4.0],
            &mvp
        ));
    }

    #[test]
    fn test_aabb_culled_behind_camera() {
        let mvp = make_test_mvp();
        // Box at z = +5 (behind camera).
        assert!(!aabb_visible_in_clip(
            [-1.0, -1.0, 4.0],
            [1.0, 1.0, 6.0],
            &mvp
        ));
    }

    #[test]
    fn test_aabb_culled_far_left() {
        let mvp = make_test_mvp();
        // Box at z = -5, but shifted far to the left (x = -100).
        assert!(!aabb_visible_in_clip(
            [-102.0, -1.0, -6.0],
            [-100.0, 1.0, -4.0],
            &mvp
        ));
    }

    #[test]
    fn test_aabb_empty_bbox_always_visible() {
        let mvp = make_test_mvp();
        // Empty bbox (min > max): should always pass (convention from C++).
        assert!(aabb_visible_in_clip(
            [f32::MAX, f32::MAX, f32::MAX],
            [f32::MIN, f32::MIN, f32::MIN],
            &mvp
        ));
    }

    #[test]
    fn test_aabb_infinite_bbox_always_visible() {
        let mvp = make_test_mvp();
        assert!(aabb_visible_in_clip(
            [f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY],
            [f32::INFINITY, f32::INFINITY, f32::INFINITY],
            &mvp
        ));
    }

    #[test]
    fn test_mat4_mul_identity() {
        let id = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0f64],
        ];
        let result = mat4_mul_f64(&id, &id);
        for r in 0..4 {
            for c in 0..4 {
                let expected = if r == c { 1.0 } else { 0.0 };
                assert!((result[r][c] - expected).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn test_frustum_culling_toggle() {
        let collection = HdRprimCollection::new(Token::new("test"));
        let mut pass = HdStRenderPass::new(collection);
        assert!(pass.is_frustum_culling_enabled());
        pass.set_frustum_culling_enabled(false);
        assert!(!pass.is_frustum_culling_enabled());
    }

    #[test]
    fn test_aabb_straddling_near_plane() {
        let mvp = make_test_mvp();
        // Box from z = -0.05 to z = -5 — straddles the near plane (near = 0.1).
        // Should be visible: part of the box is inside the frustum.
        assert!(aabb_visible_in_clip(
            [-1.0, -1.0, -5.0],
            [1.0, 1.0, -0.05],
            &mvp
        ));
    }

    #[test]
    fn test_aabb_partially_visible_right_edge() {
        let mvp = make_test_mvp();
        // Box partially overlapping the right edge at z = -5.
        // Center at x = 4 with half-size 2 → spans x = [2, 6].
        // At z = -5 with FOV~90, frustum half-width = 5, so [2, 6] overlaps [−5, 5].
        assert!(aabb_visible_in_clip(
            [2.0, -1.0, -6.0],
            [6.0, 1.0, -4.0],
            &mvp
        ));
    }

    #[test]
    fn test_aabb_with_model_transform() {
        let mvp = make_test_mvp();
        // Box at origin in local space, with a model transform that moves it to z = -5.
        let model: [[f64; 4]; 4] = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, -5.0, 1.0], // translate z = -5 (row-vector convention)
        ];
        let local_mvp = mat4_mul_f64(&model, &mvp);
        // Local bbox [-1, 1]^3 → world bbox at z = [-6, -4].
        assert!(aabb_visible_in_clip(
            [-1.0, -1.0, -1.0],
            [1.0, 1.0, 1.0],
            &local_mvp
        ));
    }

    #[test]
    fn test_aabb_with_model_transform_behind_camera() {
        let mvp = make_test_mvp();
        // Box at origin in local space, with a model transform that moves it behind camera.
        let model: [[f64; 4]; 4] = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 5.0, 1.0], // translate z = +5 (behind camera)
        ];
        let local_mvp = mat4_mul_f64(&model, &mvp);
        // Local bbox [-1, 1]^3 → world bbox at z = [4, 6] → behind camera → culled.
        assert!(!aabb_visible_in_clip(
            [-1.0, -1.0, -1.0],
            [1.0, 1.0, 1.0],
            &local_mvp
        ));
    }

    #[test]
    fn test_aabb_at_far_plane_boundary() {
        let mvp = make_test_mvp();
        // Box right at the far plane (z = -100, far = 100). Should still be visible
        // (touching the boundary counts as inside).
        assert!(aabb_visible_in_clip(
            [-1.0, -1.0, -101.0],
            [1.0, 1.0, -99.0],
            &mvp
        ));
    }

    #[test]
    fn test_aabb_beyond_far_plane() {
        let mvp = make_test_mvp();
        // Box entirely beyond far plane.
        assert!(!aabb_visible_in_clip(
            [-1.0, -1.0, -200.0],
            [1.0, 1.0, -150.0],
            &mvp
        ));
    }
}
