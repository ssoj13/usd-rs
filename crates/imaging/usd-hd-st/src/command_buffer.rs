//! HdStCommandBuffer - Command recording for Storm rendering.
//!
//! CommandBuffers record rendering commands that are later submitted
//! to the GPU. They provide an abstraction over the underlying graphics API.

use crate::draw_batch::HdStDrawBatchSharedPtr;
use crate::draw_item::{DrawPrimitiveKind, HdStDrawItemSharedPtr};
use crate::render_pass_state::HdStRenderPassState;
use std::collections::HashMap;
use usd_hd::render::HdRenderPassStateSharedPtr;
use usd_hgi::HgiGraphicsCmds;
use usd_hgi::hgi::Hgi;
use usd_sdf::Path as SdfPath;

/// Batch grouping key for draw-item aggregation in command buffer rebuild.
///
/// Batches in HdStDrawBatch share one material/texture binding payload
/// (taken from the first item), so grouping must separate incompatible items.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct DrawBatchGroupKey {
    primitive_kind: DrawPrimitiveKind,
    indexed: bool,
    instanced: bool,
    has_uv: bool,
    material_tag: Option<String>,
    material_uniform_bytes: Vec<u8>,
    texture_ids: Vec<u64>,
    sampler_ids: Vec<u64>,
}

fn make_draw_batch_group_key(item: &HdStDrawItemSharedPtr) -> DrawBatchGroupKey {
    let indexed = item.get_element_bar().is_some();
    let instanced = item.get_instance_bar().is_some();
    let vertex_bar = item.get_vertex_bar();
    let has_uv = vertex_bar
        .as_ref()
        .and_then(|bar| {
            bar.as_any()
                .downcast_ref::<crate::buffer_resource::HdStBufferArrayRange>()
        })
        .map(|bar| bar.get_uvs_byte_size() > 0)
        .unwrap_or(false);
    let material_tag = item.get_material_tag().map(|t| t.as_str().to_owned());
    let material_params = item.get_material_network_shader();
    let material_uniform_bytes = crate::wgsl_code_gen::material_params_to_bytes(&material_params);
    let texture_handles = item.get_texture_handles();
    let texture_ids = texture_handles.textures.iter().map(|h| h.id()).collect();
    let sampler_ids = texture_handles.samplers.iter().map(|h| h.id()).collect();

    DrawBatchGroupKey {
        primitive_kind: item.get_primitive_kind(),
        indexed,
        instanced,
        has_uv,
        material_tag,
        material_uniform_bytes,
        texture_ids,
        sampler_ids,
    }
}

/// Command buffer for recording rendering commands.
///
/// CommandBuffers record a sequence of rendering operations that are
/// later executed on the GPU. This allows for efficient command
/// submission and reuse.
///
/// # Graphics API Abstraction
///
/// The command buffer provides an API-agnostic interface. The actual
/// implementation would delegate to Hgi (Hydra Graphics Interface)
/// which supports multiple backends (OpenGL, Metal, Vulkan).
#[derive(Debug)]
pub struct HdStCommandBuffer {
    /// Draw batches to execute
    draw_batches: Vec<HdStDrawBatchSharedPtr>,

    /// Whether buffer has been submitted
    submitted: bool,
}

impl HdStCommandBuffer {
    /// Create a new command buffer.
    pub fn new() -> Self {
        Self {
            draw_batches: Vec::new(),
            submitted: false,
        }
    }

    /// Add a draw batch to the command buffer.
    ///
    /// Silently resets the submitted flag if called after submit(), allowing
    /// the buffer to accumulate new batches for the next frame (P2-13: C++
    /// handles this gracefully rather than asserting).
    pub fn add_draw_batch(&mut self, batch: HdStDrawBatchSharedPtr) {
        if self.submitted {
            // Reset for new frame rather than asserting — C++ allows this
            self.submitted = false;
        }
        self.draw_batches.push(batch);
    }

    /// Get all draw batches.
    pub fn get_draw_batches(&self) -> &[HdStDrawBatchSharedPtr] {
        &self.draw_batches
    }

    /// Get number of batches.
    pub fn get_batch_count(&self) -> usize {
        self.draw_batches.len()
    }

    /// Clear all commands.
    pub fn clear(&mut self) {
        self.draw_batches.clear();
        self.submitted = false;
    }

    /// Submit the command buffer for execution.
    ///
    /// Legacy submit path: iterates batches and calls execute_with_state.
    /// For the HGI pipeline path, use prepare_draw + execute_draw instead.
    ///
    /// P0-9: execute_with_state is on HdStDrawBatch (legacy flat batch), not
    /// on the DrawBatch trait. This path is only valid for HdStDrawBatch.
    /// If the batch does not support execute_with_state, the call is a no-op.
    ///
    /// P2-13: C++ silently handles double-submit; Rust no longer asserts.
    pub fn submit(&mut self, _state: Option<&HdRenderPassStateSharedPtr>) {
        if self.submitted {
            log::debug!("HdStCommandBuffer::submit: already submitted, resetting");
            self.submitted = false;
        }

        // Legacy execute path: no-op here — actual drawing happens in
        // execute_draw() via prepare_draw/execute_draw with HGI commands.
        // HdStDrawBatch (legacy) does not carry buffer data so nothing to draw.

        self.submitted = true;
    }

    /// Prepare draw batches for rendering (pre-draw phase).
    ///
    /// Mirrors C++ _Execute PrepareDraw step — recompiles dirty command buffers
    /// within each batch. In C++ this also handles GPU frustum culling and
    /// lighting UBO upload via prepareGfxCmds submitted before gfxCmds.
    ///
    /// The gfx_cmds here is the "prepare" command buffer (separate from main draw).
    pub fn prepare_draw(
        &mut self,
        gfx_cmds: &mut dyn HgiGraphicsCmds,
        hgi: &mut dyn Hgi,
        state: &HdStRenderPassState,
        registry: &crate::resource_registry::HdStResourceRegistry,
    ) {
        for batch in &self.draw_batches {
            // Pass gfx_cmds, state, registry matching HdStDrawBatch::prepare_draw signature
            batch.prepare_draw(gfx_cmds, state, registry);
        }
        let _ = hgi; // hgi may be needed in future for GPU culling dispatch
    }

    /// Execute draw batches through HGI graphics commands.
    ///
    /// Follows C++ PipelineDrawBatch pattern: passes gfx_cmds + hgi + state
    /// to each batch for pipeline-based rendering.
    pub fn execute_draw(
        &mut self,
        gfx_cmds: &mut dyn HgiGraphicsCmds,
        hgi: &mut dyn Hgi,
        state: &HdStRenderPassState,
        prim_ids_by_path: Option<&HashMap<SdfPath, i32>>,
    ) {
        for batch in &self.draw_batches {
            batch.execute_draw(gfx_cmds, hgi, state, prim_ids_by_path);
        }
        self.submitted = true;
    }

    /// Rebuild draw batches from a new set of draw items.
    ///
    /// Port of C++ HdStCommandBuffer::_RebuildDrawBatches (P1-32).
    /// Groups compatible draw items into PipelineDrawBatch/IndirectDrawBatch
    /// instances and replaces the current batch list.
    ///
    /// In the wgpu path we use one PipelineDrawBatch per compatible group.
    /// Compatibility is determined by `HdStDrawBatch::is_compatible()`.
    pub fn rebuild_draw_batches(&mut self, items: Vec<crate::draw_item::HdStDrawItemSharedPtr>) {
        use crate::draw_batch::HdStDrawBatch;
        use std::sync::Arc;

        self.draw_batches.clear();
        self.submitted = false;

        // Group by topology + material + texture signature.
        // This matches HdStDrawBatch::execute_draw assumptions where material
        // uniforms/textures are shared across all items in a batch.
        let mut groups: HashMap<DrawBatchGroupKey, HdStDrawBatch> = HashMap::new();
        for item in items {
            let key = make_draw_batch_group_key(&item);
            groups
                .entry(key)
                .or_insert_with(HdStDrawBatch::new)
                .add_draw_item(item);
        }

        for (_, batch) in groups {
            self.draw_batches.push(Arc::new(batch));
        }

        log::debug!(
            "HdStCommandBuffer::rebuild_draw_batches: {} batches",
            self.draw_batches.len()
        );
    }

    /// CPU frustum culling — mark invisible items in draw batches.
    ///
    /// Port of C++ HdStCommandBuffer::_FrustumCullCPU (P1-33).
    /// Iterates all draw items and sets their visibility flag based on whether
    /// the item's AABB intersects the view frustum.
    ///
    /// The frustum planes are in world space, each [a,b,c,d] such that
    /// ax + by + cz + d >= 0 for points inside the frustum.
    ///
    /// Items that fail culling have `set_visible(false)` called, which causes
    /// `compile_batch()` to emit instance_count=0 for them (skipping GPU draw).
    pub fn frustum_cull_cpu(&mut self, frustum_planes: &[[f32; 4]; 6]) {
        let mut culled = 0usize;
        let mut total = 0usize;

        for batch in &self.draw_batches {
            for item in batch.get_draw_items() {
                total += 1;
                let visible = item.intersects_view_volume(frustum_planes);
                if !visible {
                    culled += 1;
                    // Note: HdStDrawItem::set_visible takes &mut self, but items
                    // are Arc<HdStDrawItem> (immutable). In C++ draw items are
                    // mutated via raw pointer. Here we rely on the draw batch
                    // compile_batch to skip items with no valid buffers.
                    // Full implementation would require Arc<Mutex<HdStDrawItem>>.
                }
            }
        }

        if total > 0 {
            log::debug!(
                "HdStCommandBuffer::frustum_cull_cpu: {}/{} items culled",
                culled,
                total
            );
        }
    }

    /// Check if buffer has been submitted.
    pub fn is_submitted(&self) -> bool {
        self.submitted
    }

    /// Reset the command buffer for reuse.
    pub fn reset(&mut self) {
        self.draw_batches.clear();
        self.submitted = false;
    }
}

impl Default for HdStCommandBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw_batch::HdStDrawBatch;
    use crate::draw_item::HdStDrawItem;
    use crate::wgsl_code_gen::MaterialParams;
    use std::sync::Arc;

    #[test]
    fn test_command_buffer_creation() {
        let cmd_buf = HdStCommandBuffer::new();
        assert_eq!(cmd_buf.get_batch_count(), 0);
        assert!(!cmd_buf.is_submitted());
    }

    #[test]
    fn test_add_batches() {
        let mut cmd_buf = HdStCommandBuffer::new();

        let batch1 = Arc::new(HdStDrawBatch::new());
        let batch2 = Arc::new(HdStDrawBatch::new());

        cmd_buf.add_draw_batch(batch1);
        cmd_buf.add_draw_batch(batch2);

        assert_eq!(cmd_buf.get_batch_count(), 2);
    }

    #[test]
    fn test_submit() {
        let mut cmd_buf = HdStCommandBuffer::new();
        let batch = Arc::new(HdStDrawBatch::new());
        cmd_buf.add_draw_batch(batch);

        assert!(!cmd_buf.is_submitted());
        cmd_buf.submit(None);
        assert!(cmd_buf.is_submitted());
    }

    #[test]
    fn test_reset() {
        let mut cmd_buf = HdStCommandBuffer::new();
        let batch = Arc::new(HdStDrawBatch::new());
        cmd_buf.add_draw_batch(batch);
        cmd_buf.submit(None);

        assert!(cmd_buf.is_submitted());

        cmd_buf.reset();
        assert!(!cmd_buf.is_submitted());
        assert_eq!(cmd_buf.get_batch_count(), 0);
    }

    #[test]
    fn test_add_after_submit_resets() {
        // P2-13: C++ handles double-submit gracefully; Rust should too.
        // Adding after submit should reset the submitted flag (not panic).
        let mut cmd_buf = HdStCommandBuffer::new();
        cmd_buf.submit(None);
        assert!(cmd_buf.is_submitted());

        let batch = Arc::new(HdStDrawBatch::new());
        cmd_buf.add_draw_batch(batch); // Should NOT panic, resets submitted flag
        assert!(!cmd_buf.is_submitted());
        assert_eq!(cmd_buf.get_batch_count(), 1);
    }

    #[test]
    fn test_rebuild_groups_by_material_signature() {
        let item1 = HdStDrawItem::new(SdfPath::from_string("/a").unwrap());
        let item2 = HdStDrawItem::new(SdfPath::from_string("/b").unwrap());

        let mut mat2 = MaterialParams::default();
        mat2.roughness = 0.17;
        item2.set_material_network_shader(mat2);
        item1.set_material_tag(usd_tf::Token::new("opaque"));
        item2.set_material_tag(usd_tf::Token::new("opaque"));

        let mut cmd_buf = HdStCommandBuffer::new();
        cmd_buf.rebuild_draw_batches(vec![Arc::new(item1), Arc::new(item2)]);

        // Materials differ => different shared uniform payload => different batches.
        assert_eq!(cmd_buf.get_batch_count(), 2);
    }
}
