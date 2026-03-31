
//! HdStDrawBatch - Batching system for draw items.
//!
//! DrawBatches group compatible draw items together for efficient rendering.
//! Items with similar state (shaders, buffers) can be drawn together
//! to minimize state changes.
//!
//! Port of pxr/imaging/hdSt/drawBatch.h, pipelineDrawBatch.h, indirectDrawBatch.h
//!
//! Architecture:
//! - `DrawBatch` trait: base interface (Validate, PrepareDraw, ExecuteDraw, EncodeDraw)
//! - `PipelineDrawBatch`: HGI-based path, groups draw items by pipeline state,
//!   builds dispatch buffer, issues wgpu draws (immediate or indirect)
//! - `HdStDrawBatch`: legacy flat batch (retained for backward compat)

use crate::binding::slots;
use crate::basis_curves_shader_key::{BasisCurvesShaderKey, CurveDrawStyle};
use crate::buffer_resource::HdStBufferArrayRange;
use crate::draw_item::{
    DrawPrimitiveKind, HdBufferArrayRangeSharedPtr, HdStDrawItem, HdStDrawItemSharedPtr,
    MaterialTextureHandles,
};
use crate::draw_program_key::{
    BasisCurvesProgramKey, DrawProgramKey, PointsProgramKey, BASIS_CURVES_DEFAULT_NORMAL_STYLE,
};
use crate::lighting;
use crate::mesh_shader_key::{MeshShaderKey, PrimvarInterp, ShadingModel};
use crate::points_shader_key::PointsShaderKey;
use crate::render_pass_state::{HdStPolygonRasterMode, HdStRenderPassState};
use crate::resource_binder::ResourceBinder;
use crate::resource_registry::HdStResourceRegistry;
use crate::wgsl_code_gen;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use usd_hgi::graphics_cmds::{HgiDrawIndexedOp, HgiDrawIndirectOp, HgiViewport};
use usd_hgi::hgi::Hgi;
use usd_hgi::{
    HgiBufferDesc, HgiBufferHandle, HgiBufferUsage, HgiGraphicsCmds, HgiGraphicsPipelineDesc,
    HgiGraphicsPipelineHandle, HgiShaderFunctionDesc, HgiShaderProgramDesc, HgiShaderProgramHandle,
    HgiShaderStage,
};
use usd_sdf::Path as SdfPath;

// ---------------------------------------------------------------------------
// DrawBatch trait (port of HdSt_DrawBatch base class)
// ---------------------------------------------------------------------------

/// Validation result for draw batches.
///
/// Port of HdSt_DrawBatch::ValidationResult.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationResult {
    /// Batch is valid and can be submitted as-is.
    ValidBatch,
    /// This batch needs to be rebuilt.
    RebuildBatch,
    /// All batches need to be rebuilt (buffer arrays changed).
    RebuildAllBatches,
}

/// Draw batch trait - base interface for all batch types.
///
/// Port of HdSt_DrawBatch. Defines the contract for batched drawing:
/// validate -> prepare -> encode -> execute.
pub trait DrawBatch: Send + Sync + std::fmt::Debug {
    /// Validate batch, returns whether it can be reused or needs rebuild.
    fn validate(&mut self, deep: bool) -> ValidationResult;

    /// Attempt to rebuild the batch in-place.
    ///
    /// Port of HdSt_DrawBatch::Rebuild. Returns false if draw items are
    /// no longer compatible and the batch must be discarded.
    fn rebuild(&mut self) -> bool;

    /// Prepare draw commands (frustum culling, command buffer update).
    ///
    /// Port of HdSt_DrawBatch::PrepareDraw.
    fn prepare_draw(
        &mut self,
        gfx_cmds: &mut dyn HgiGraphicsCmds,
        state: &HdStRenderPassState,
        registry: &HdStResourceRegistry,
    );

    /// Encode drawing commands (for indirect command encoding).
    ///
    /// Port of HdSt_DrawBatch::EncodeDraw.
    fn encode_draw(
        &mut self,
        state: &HdStRenderPassState,
        registry: &HdStResourceRegistry,
        first_draw_batch: bool,
    );

    /// Execute drawing commands through HGI graphics commands.
    ///
    /// Port of HdSt_DrawBatch::ExecuteDraw.
    /// Takes `&mut self` to allow in-place frustum culling of the command buffer.
    fn execute_draw(
        &mut self,
        gfx_cmds: &mut dyn HgiGraphicsCmds,
        hgi: &mut dyn Hgi,
        state: &HdStRenderPassState,
        first_draw_batch: bool,
    );

    /// Attempt to append a draw item. Returns false if incompatible.
    ///
    /// Port of HdSt_DrawBatch::Append.
    fn append(&mut self, item: HdStDrawItemSharedPtr) -> bool;

    /// Notify batch that a draw item instance has changed.
    ///
    /// Port of HdSt_DrawBatch::DrawItemInstanceChanged.
    /// Called from multiple threads, must be threadsafe.
    fn draw_item_instance_changed(&self, _item: &HdStDrawItemSharedPtr) {
        // Default: no-op
    }

    /// Enable/disable tiny prim culling for this batch.
    ///
    /// Port of HdSt_DrawBatch::SetEnableTinyPrimCulling.
    fn set_enable_tiny_prim_culling(&mut self, _enable: bool) {
        // Default: no-op
    }

    /// Get number of draw items.
    fn item_count(&self) -> usize;

    /// Check if batch is empty.
    fn is_empty(&self) -> bool;
}

/// Shared pointer to a draw batch trait object.
pub type DrawBatchSharedPtr = Arc<Mutex<dyn DrawBatch>>;

/// Vector of shared draw batch pointers.
///
/// Port of HdSt_DrawBatchSharedPtrVector.
pub type DrawBatchSharedPtrVec = Vec<DrawBatchSharedPtr>;

// ---------------------------------------------------------------------------
// PipelineDrawBatch (port of HdSt_PipelineDrawBatch)
// ---------------------------------------------------------------------------

/// Draw command for the dispatch buffer.
///
/// Matches Vulkan/GL/D3D indirect draw indexed layout.
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
#[allow(dead_code)]
struct DrawIndexedCommand {
    index_count: u32,
    instance_count: u32,
    base_index: u32,
    base_vertex: i32,
    base_instance: u32,
}

/// Per-item drawing coordinate (buffer offsets for shader).
///
/// Port of _DrawingCoord from pipelineDrawBatch.cpp.
///
/// C++ packs DrawingCoord into the dispatch buffer after the draw command
/// struct (5 u32). The shader reads it via SSBO resource views to locate
/// per-prim data (transforms, materials) in shared VBO pools.
///
/// Layout (10 u32, matching C++ drawingCoord0+1+2):
///   [0] modelDC, [1] constantDC, [2] elementDC, [3] primitiveDC,
///   [4] fvarDC, [5] instanceIndexDC, [6] shaderDC, [7] vertexDC,
///   [8] topVisDC, [9] varyingDC
///
/// DrawingCoord is written into the dispatch buffer after each draw command
/// struct (5 u32), matching C++ layout in pipelineDrawBatch.cpp.
///
/// C++ reads DrawingCoord in the shader via HdGet_drawingCoord0/1/2() accessors
/// generated by HdSt_CodeGen, backed by an SSBO (HdBufferResourceSharedPtr).
/// When multi-draw-indirect is enabled the GPU reads these offsets to locate
/// per-prim data (transforms, materials) in shared VBO pools without CPU round-trips.
///
/// Current status: CPU-driven path — DrawingCoord is packed here for layout
/// compatibility but the shader reads per-prim data via push constants set
/// before each individual draw call. When multi-draw-indirect lands, remove
/// the push-constant path and wire this buffer as an SSBO at group 3, binding 0.
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
#[allow(dead_code)]
struct DrawingCoord {
    model_dc: u32,
    constant_dc: u32,
    element_dc: u32,
    primitive_dc: u32,
    fvar_dc: u32,
    instance_index_dc: u32,
    shader_dc: u32,
    vertex_dc: u32,
    top_vis_dc: u32,
    varying_dc: u32,
}

/// Pipeline draw batch - groups draw items by pipeline state.
///
/// Port of HdSt_PipelineDrawBatch. Groups draw items that share the same
/// shader, topology, and buffer layout into a single batch. Builds a flat
/// draw command buffer and issues wgpu draw calls.
///
/// Aggregation rule: items are compatible if they share the same:
/// - MeshShaderKey (shading model, normals, topology)
/// - Buffer array hash (vertex/index/constant buffers from same pool)
/// - Instance level count
#[derive(Debug)]
pub struct PipelineDrawBatch {
    /// Draw items in this batch.
    draw_items: Vec<HdStDrawItemSharedPtr>,
    /// Flat draw command buffer (packed u32).
    draw_command_buffer: Vec<u32>,
    /// Whether the command buffer needs rebuild.
    draw_command_buffer_dirty: bool,
    /// Hash of buffer arrays (for detecting reallocation).
    #[allow(dead_code)]
    buffer_arrays_hash: u64,
    /// Hash of per-item element offsets.
    bar_element_offsets_hash: u64,
    /// Number of visible items after culling.
    num_visible_items: usize,
    /// Total vertex count across all items.
    num_total_vertices: usize,
    /// Total index element count across all items.
    num_total_elements: usize,
    /// Whether this batch uses indexed draw.
    use_draw_indexed: bool,
    /// Whether this batch uses instancing.
    use_instancing: bool,
    /// Shader key for pipeline creation.
    program_key: DrawProgramKey,
}

impl PipelineDrawBatch {
    /// Create a new pipeline draw batch seeded with one draw item.
    pub fn new(first_item: HdStDrawItemSharedPtr) -> Self {
        let use_draw_indexed = first_item.get_element_bar().is_some();
        let use_instancing = first_item.get_instance_bar().is_some();

        let program_key = infer_program_key_from_draw_item(first_item.as_ref(), use_instancing);

        let mut batch = Self {
            draw_items: Vec::new(),
            draw_command_buffer: Vec::new(),
            draw_command_buffer_dirty: true,
            buffer_arrays_hash: 0,
            bar_element_offsets_hash: 0,
            num_visible_items: 0,
            num_total_vertices: 0,
            num_total_elements: 0,
            use_draw_indexed,
            use_instancing,
            program_key,
        };
        batch.draw_items.push(first_item);
        batch
    }

    /// Check if a draw item is compatible with this batch.
    ///
    /// Port of HdSt_DrawBatch::_IsAggregated (pxr/imaging/hdSt/drawBatch.cpp:219-261).
    /// Items must share ALL of:
    /// 1. Indexed vs non-indexed topology type
    /// 2. Instancing mode
    /// 3. Material network shader params (C++ _CanAggregateMaterials)
    /// 4. Vertex, element, constant BARs from the same pool
    fn is_aggregated(&self, item: &HdStDrawItemSharedPtr) -> bool {
        if self.draw_items.is_empty() {
            return true;
        }
        let first = &self.draw_items[0];

        // Must match indexed vs non-indexed
        let item_indexed = item.get_element_bar().is_some();
        if item_indexed != self.use_draw_indexed {
            return false;
        }

        // Must match instancing
        let item_instanced = item.get_instance_bar().is_some();
        if item_instanced != self.use_instancing {
            return false;
        }

        // Material network shader must match (C++ _CanAggregateMaterials).
        // Items with different materials produce different visual output and
        // cannot share a single material push constant binding.
        if first.get_material_network_shader() != item.get_material_network_shader() {
            return false;
        }

        // Buffer arrays must be from the same pool (C++ IsAggregatedWith).
        // C++ checks: vertex, element, constant, topology, varying, fvar, instance, instance_index.
        if !bars_share_buffer(first.get_vertex_bar(), item.get_vertex_bar()) {
            return false;
        }
        if !bars_share_buffer(first.get_element_bar(), item.get_element_bar()) {
            return false;
        }
        if !bars_share_buffer(first.get_constant_bar(), item.get_constant_bar()) {
            return false;
        }
        // Topology BAR must match (same subdivision/triangulation pool).
        if !bars_share_buffer(first.get_topology_bar(), item.get_topology_bar()) {
            return false;
        }
        // Varying BAR must match (same vertex-varying primvar pool).
        if !bars_share_buffer(first.get_varying_bar(), item.get_varying_bar()) {
            return false;
        }
        // Face-varying BAR must match (same faceVarying primvar pool).
        if !bars_share_buffer(first.get_face_varying_bar(), item.get_face_varying_bar()) {
            return false;
        }
        // Instance BAR and instance index BAR must match for instanced draws.
        if !bars_share_buffer(first.get_instance_bar(), item.get_instance_bar()) {
            return false;
        }
        if !bars_share_buffer(
            first.get_instance_index_bar(),
            item.get_instance_index_bar(),
        ) {
            return false;
        }
        // Geometric shader key must match: same prim_type, cull_style, polygon_mode.
        // C++ HdSt_DrawBatch::_IsAggregated() checks geometric shader identity.
        if first.get_geometric_shader_key() != item.get_geometric_shader_key() {
            return false;
        }
        if first.get_primitive_topology() != item.get_primitive_topology() {
            return false;
        }

        // _CanAggregateTextures: items with different texture sets must not batch together.
        // C++ drawBatch.cpp:213-216. Different textures => different @group(3) bind group;
        // batching would silently corrupt the render (all items use first item's textures).
        if first.compute_texture_source_hash() != item.compute_texture_source_hash() {
            return false;
        }

        // TopologyVisibilityRange must share buffer pool.
        // C++ _IsAggregated checks GetTopologyVisibilityRange() via isAggregated().
        // Per-face visibility data must come from the same buffer array.
        if !bars_share_buffer(
            first.get_topology_visibility_bar(),
            item.get_topology_visibility_bar(),
        ) {
            return false;
        }

        // InstancePrimvarNumLevels must match: items with different multi-level
        // instancing depths cannot be batched (C++: GetInstancePrimvarNumLevels()).
        if first.get_instance_primvar_num_levels() != item.get_instance_primvar_num_levels() {
            return false;
        }

        // Per-level InstancePrimvarRange: each instancing level's BAR must share a pool.
        // C++ loops: for (int i = 0; i < numLevels; ++i) check isAggregated(bar_i_a, bar_i_b).
        let num_levels = first.get_instance_primvar_num_levels();
        for level in 0..num_levels {
            if !bars_share_buffer(
                first.get_instance_primvar_bar(level),
                item.get_instance_primvar_bar(level),
            ) {
                return false;
            }
        }

        true
    }

    /// Compile the draw command buffer from current draw items.
    ///
    /// Port of HdSt_PipelineDrawBatch::_CompileBatch.
    /// Builds a flat u32 command buffer with draw commands + drawing coords.
    fn compile_batch(&mut self) {
        if self.draw_items.is_empty() {
            return;
        }

        // Each entry: DrawIndexedCommand (5 u32) + DrawingCoord (10 u32) = 15 u32
        let cmd_stride = 15usize;
        let num_items = self.draw_items.len();
        self.draw_command_buffer.resize(num_items * cmd_stride, 0);

        self.num_visible_items = 0;
        self.num_total_elements = 0;
        self.num_total_vertices = 0;
        self.bar_element_offsets_hash = 0;

        for (i, item) in self.draw_items.iter().enumerate() {
            let base = i * cmd_stride;

            if !item.is_valid() || !item.is_visible() {
                // Zero out command (instance_count=0 means skip)
                continue;
            }

            // Extract index count from element BAR.
            // base_index=0: pool sub-alloc offset is added in execute_draw via idx_base_add
            // to prevent double-counting. (P1-1, P1-3)
            let (index_count, _raw_index_off) = if let Some(ebar) = item.get_element_bar() {
                if let Some(st_bar) = ebar.as_any().downcast_ref::<HdStBufferArrayRange>() {
                    let count = (st_bar.get_size() / std::mem::size_of::<u32>()) as u32;
                    let bi = st_bar.get_offset() / std::mem::size_of::<u32>();
                    (count, bi)
                } else {
                    (0u32, 0usize)
                }
            } else {
                (0u32, 0usize)
            };

            // Extract vertex count and base_vertex (element index, not byte offset). (P0-7, P1-2)
            // Use positions_byte_size to exclude packed normals+uvs from count.
            let (vertex_count, base_vertex) = if let Some(vbar) = item.get_vertex_bar() {
                if let Some(st_bar) = vbar.as_any().downcast_ref::<HdStBufferArrayRange>() {
                    let pos_bytes = st_bar.get_positions_byte_size();
                    let count = if pos_bytes > 0 {
                        (pos_bytes / (3 * std::mem::size_of::<f32>())) as u32
                    } else {
                        (st_bar.get_size() / (3 * std::mem::size_of::<f32>())) as u32
                    };
                    let bv = st_bar.get_offset() / (3 * std::mem::size_of::<f32>());
                    (count, bv as u32)
                } else {
                    (0u32, 0u32)
                }
            } else {
                (0u32, 0u32)
            };

            if index_count == 0 || vertex_count == 0 {
                continue;
            }

            let instance_count = 1u32;

            // DrawIndexedCommand (5 u32).
            // base_index=0: pool offset applied in execute_draw to avoid double-count.
            // baseInstance is 0-based within this batch (P2-6): C++ uses gl_BaseInstance
            // as an offset into the per-batch drawing coord buffer, not a global item index.
            let base_instance = self.num_visible_items as u32; // sequential within batch
            self.draw_command_buffer[base + 0] = index_count;
            self.draw_command_buffer[base + 1] = instance_count;
            self.draw_command_buffer[base + 2] = 0; // base_index — see execute_draw
            self.draw_command_buffer[base + 3] = base_vertex;
            self.draw_command_buffer[base + 4] = base_instance; // 0-based per-batch offset

            // DrawingCoord (10 u32) - buffer element offsets for shader
            let constant_dc = if let Some(cbar) = item.get_constant_bar() {
                if let Some(st_bar) = cbar.as_any().downcast_ref::<HdStBufferArrayRange>() {
                    st_bar.get_offset() as u32
                } else {
                    0
                }
            } else {
                0
            };
            let fvar_dc = face_varying_dc_from_item(item.as_ref());

            self.draw_command_buffer[base + 5] = 0; // modelDC (reserved)
            self.draw_command_buffer[base + 6] = constant_dc;
            self.draw_command_buffer[base + 7] = 0; // elementDC
            self.draw_command_buffer[base + 8] = 0; // primitiveDC
            self.draw_command_buffer[base + 9] = fvar_dc;
            self.draw_command_buffer[base + 10] = 0; // instanceIndexDC
            self.draw_command_buffer[base + 11] = 0; // shaderDC
            self.draw_command_buffer[base + 12] = base_vertex; // vertexDC
            self.draw_command_buffer[base + 13] = 0; // topVisDC
            self.draw_command_buffer[base + 14] = 0; // varyingDC

            // Update hash for validation
            self.bar_element_offsets_hash = self
                .bar_element_offsets_hash
                .wrapping_mul(0x9e3779b97f4a7c15)
                .wrapping_add(base_vertex as u64);

            self.num_visible_items += 1;
            self.num_total_elements += index_count as usize;
            self.num_total_vertices += vertex_count as usize;
        }

        self.draw_command_buffer_dirty = false;
        log::debug!(
            "[PipelineDrawBatch] compiled: {} items, {} visible, {} indices, {} verts",
            num_items,
            self.num_visible_items,
            self.num_total_elements,
            self.num_total_vertices
        );
    }

    /// Check if there is nothing to draw.
    fn has_nothing_to_draw(&self) -> bool {
        self.num_visible_items == 0 || self.draw_items.is_empty()
    }

    /// Get draw items.
    pub fn get_draw_items(&self) -> &[HdStDrawItemSharedPtr] {
        &self.draw_items
    }

    /// GPU instancing path: gather all item transforms into SSBO, issue single draw call.
    fn execute_instanced_draw(
        &self,
        gfx_cmds: &mut dyn HgiGraphicsCmds,
        hgi: &mut dyn Hgi,
        fs: &FrameSetup,
        identity: &[[f64; 4]; 4],
    ) {
        if self.draw_items.is_empty() {
            return;
        }

        // Gather model transforms for all visible items as f32 (16 floats per instance).
        let cmd_stride = 15usize;
        let mut xform_data: Vec<f32> = Vec::with_capacity(self.draw_items.len() * 16);
        let mut visible_items: Vec<usize> = Vec::with_capacity(self.draw_items.len());

        for (i, item) in self.draw_items.iter().enumerate() {
            let base = i * cmd_stride;
            if base + cmd_stride > self.draw_command_buffer.len() {
                break;
            }
            let index_count = self.draw_command_buffer[base + 0];
            let instance_count = self.draw_command_buffer[base + 1];
            if index_count == 0 || instance_count == 0 {
                continue;
            }
            let model = item.get_world_transform();
            let _ = identity; // keep param for API compat
            for row in model.iter() {
                for &val in row.iter() {
                    xform_data.push(val as f32);
                }
            }
            visible_items.push(i);
        }

        let n_instances = visible_items.len();
        if n_instances == 0 {
            return;
        }

        // Upload scene uniforms with identity model (VS reads model from SSBO instead).
        let id_model = &[
            [1.0, 0.0, 0.0, 0.0f64],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let scene_data = build_scene_uniforms(
            &fs.vp,
            id_model,
            &fs.ambient_color,
            &fs.cam_pos,
            &[0.0f32; 4],
            0,
        );
        gfx_cmds.set_constant_values(&fs.pipeline, HgiShaderStage::VERTEX, 0, &scene_data);

        // Create storage buffer with instance transforms.
        // SAFETY: f32 is POD, reinterpreting as bytes for GPU upload.
        let byte_data: &[u8] = unsafe {
            std::slice::from_raw_parts(
                xform_data.as_ptr() as *const u8,
                xform_data.len() * std::mem::size_of::<f32>(),
            )
        };
        let ssbo_desc = HgiBufferDesc::new()
            .with_debug_name("instance_xforms")
            .with_usage(HgiBufferUsage::STORAGE)
            .with_byte_size(byte_data.len());
        let ssbo_handle = hgi.create_buffer(&ssbo_desc, Some(byte_data));

        // Bind SSBO at the instance group.
        let ig = instance_group_from_program_key(&fs.frame_key);
        gfx_cmds.bind_storage_buffer(ig, slots::INSTANCE_XFORMS_BINDING, &ssbo_handle);

        // Bind vertex/index buffers from the first visible item.
        let first_idx = visible_items[0];
        let first_item = &self.draw_items[first_idx];
        let (_, ibuf_handle) = match bind_packed_vertex_buffers(
            gfx_cmds,
            first_item,
            &fs.frame_key,
            first_idx,
            "InstBatch",
        ) {
            Some(v) => v,
            None => return,
        };

        // Index offset from the first item's element BAR.
        let ibuf_pool_offset = first_item
            .get_element_bar()
            .as_ref()
            .and_then(|b| b.as_any().downcast_ref::<HdStBufferArrayRange>())
            .map(|b| b.get_offset())
            .unwrap_or(0);
        let idx_base_add = (ibuf_pool_offset / std::mem::size_of::<u32>()) as u32;

        let base = first_idx * cmd_stride;
        let index_count = self.draw_command_buffer[base + 0];

        // Single instanced draw call.
        gfx_cmds.draw_indexed(
            &ibuf_handle,
            &HgiDrawIndexedOp {
                index_count,
                base_index: self.draw_command_buffer[base + 2] + idx_base_add,
                base_vertex: self.draw_command_buffer[base + 3] as i32,
                instance_count: n_instances as u32,
                base_instance: 0,
            },
        );

        log::debug!(
            "[PipelineDrawBatch] instanced draw: {} instances in 1 call",
            n_instances
        );
    }
}

impl DrawBatch for PipelineDrawBatch {
    fn rebuild(&mut self) -> bool {
        // Re-validate and rebuild the command buffer from current items.
        // Returns false if items are no longer compatible.
        if self.draw_items.is_empty() {
            return false;
        }
        // Check all items are still compatible: topology + material + BAR pool
        let all_compatible = self
            .draw_items
            .iter()
            .skip(1)
            .all(|item| self.is_aggregated(item));
        if !all_compatible {
            return false;
        }
        self.draw_command_buffer_dirty = true;
        self.compile_batch();
        true
    }

    fn validate(&mut self, deep: bool) -> ValidationResult {
        if self.draw_items.is_empty() {
            return ValidationResult::ValidBatch;
        }

        // Quick check: remove invalid/invisible and see if anything left
        self.draw_items.retain(|item| item.is_valid());

        if self.draw_items.is_empty() {
            return ValidationResult::ValidBatch;
        }

        if deep || self.draw_command_buffer_dirty {
            // Recompile the command buffer
            let old_hash = self.bar_element_offsets_hash;
            self.compile_batch();

            if self.bar_element_offsets_hash != old_hash {
                return ValidationResult::RebuildBatch;
            }
        }

        ValidationResult::ValidBatch
    }

    fn prepare_draw(
        &mut self,
        _gfx_cmds: &mut dyn HgiGraphicsCmds,
        state: &HdStRenderPassState,
        _registry: &HdStResourceRegistry,
    ) {
        // Recompile if dirty (matches C++ PrepareDraw -> _CompileBatch).
        if self.draw_command_buffer_dirty {
            self.compile_batch();
        }

        // CPU frustum culling: matches C++ _ExecuteFrustumCull path when GPU
        // culling is disabled. Extract 6 Griebel planes from the cull matrix
        // (view * projection) and zero out instance_count in the command buffer
        // for items whose AABBs are fully outside any plane.
        //
        // Port of HdSt_PipelineDrawBatch::_ExecuteFrustumCull (non-GPU path).
        // C++ zeroes dispatchBuffer[i].instanceCount on the CPU when
        // !_useGpuCulling. We mirror this by writing draw_command_buffer[base+1].
        let cull = state.get_cull_matrix();
        let planes = frustum_planes_from_matrix(cull);

        let cmd_stride = 15usize;
        let mut culled = 0u32;
        for (i, item) in self.draw_items.iter().enumerate() {
            let base = i * cmd_stride;
            if base + 2 > self.draw_command_buffer.len() {
                break;
            }
            // Skip already-invalid items (index_count==0 set by compile_batch).
            if self.draw_command_buffer[base] == 0 {
                continue;
            }
            // Restore instance_count before testing — previous frame's cull may
            // have zeroed it. Matches C++ which snapshots original counts.
            self.draw_command_buffer[base + 1] = 1;
            if !item.intersects_view_volume(&planes) {
                // Cull: zero instance_count so draw will be skipped.
                self.draw_command_buffer[base + 1] = 0;
                culled += 1;
            }
        }
        if culled > 0 {
            log::debug!(
                "[PipelineDrawBatch] prepare_draw: CPU culled {}/{} items",
                culled,
                self.draw_items.len()
            );
        }
    }

    fn encode_draw(
        &mut self,
        _state: &HdStRenderPassState,
        _registry: &HdStResourceRegistry,
        _first_draw_batch: bool,
    ) {
        // PipelineDrawBatch uses CPU-driven per-item draw calls in execute_draw().
        // Indirect draw encoding (multi_draw_indexed_indirect) is implemented
        // only in IndirectDrawBatch which owns a GPU dispatch buffer.
        // Recompile here if dirty so execute_draw sees consistent state.
        // Port of HdSt_PipelineDrawBatch::EncodeDraw (no-op in C++ too).
        if self.draw_command_buffer_dirty {
            self.compile_batch();
        }
    }

    fn execute_draw(
        &mut self,
        gfx_cmds: &mut dyn HgiGraphicsCmds,
        hgi: &mut dyn Hgi,
        state: &HdStRenderPassState,
        _first_draw_batch: bool,
    ) {
        if self.has_nothing_to_draw() {
            return;
        }

        // Build per-frame shader key (depth-only / IBL / flat / scene-materials).
        // PipelineDrawBatch respects get_enable_scene_materials() (scene_mats_controlled=true).
        let frame_key = build_frame_key(&self.program_key, state, true);

        // Set up pipeline + upload shared per-frame uniforms (lights, material, IBL).
        let fs = match begin_frame_draw(
            hgi,
            gfx_cmds,
            state,
            frame_key,
            &self.draw_items,
            "PipelineDrawBatch",
        ) {
            Some(fs) => fs,
            None => return,
        };

        let identity = [
            [1.0, 0.0, 0.0, 0.0f64],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];

        // GPU instancing path: all items share the same mesh, one draw call with SSBO.
        if self.use_instancing && !program_key_uses_face_varying_storage(&fs.frame_key) {
            self.execute_instanced_draw(gfx_cmds, hgi, &fs, &identity);
            return;
        }

        // Issue draw calls per item using compiled command buffer.
        let cmd_stride = 15usize;
        let mut drawn = 0u32;
        for (i, item) in self.draw_items.iter().enumerate() {
            let base = i * cmd_stride;
            if base + cmd_stride > self.draw_command_buffer.len() {
                break;
            }

            let index_count = self.draw_command_buffer[base + 0];
            let instance_count = self.draw_command_buffer[base + 1];
            if index_count == 0 || instance_count == 0 {
                continue;
            }

            // Per-item model transform from draw item's Hydra shared data.
            // Matches C++ HdDrawItem::GetMatrix() — no viewer-side HashMap.
            let item_xform = item.get_world_transform();
            let model = &item_xform;
            if drawn == 0 {
                log::trace!(
                    "[PipelineDrawBatch] first model diag=[{:.6},{:.6},{:.6}] row3=[{:.6},{:.6},{:.6}]",
                    model[0][0],
                    model[1][1],
                    model[2][2],
                    model[3][0],
                    model[3][1],
                    model[3][2]
                );
            }
            // Selection: if prim is selected, pass highlight tint; else transparent (a=0).
            let sel_color = if state.is_path_selected(item.get_prim_path()) {
                let c = state.get_selection_color();
                [c[0], c[1], c[2], c[3]]
            } else {
                [0.0f32; 4]
            };
            let scene_data = build_scene_uniforms(
                &fs.vp,
                model,
                &fs.ambient_color,
                &fs.cam_pos,
                &sel_color,
                bind_face_varying_storage(gfx_cmds, item.as_ref(), &fs.frame_key),
            );
            gfx_cmds.set_constant_values(&fs.pipeline, HgiShaderStage::VERTEX, 0, &scene_data);

            // Bind packed vertex buffer (positions / normals / uvs) and get buffer handles.
            let (_, ibuf_handle) =
                match bind_packed_vertex_buffers(gfx_cmds, item, &fs.frame_key, i, "PipelineBatch")
                {
                    Some(v) => v,
                    None => continue,
                };

            // Index BAR pool offset (bytes -> index elements) applied here, not at compile.
            let ibuf_pool_offset = item
                .get_element_bar()
                .as_ref()
                .and_then(|b| b.as_any().downcast_ref::<HdStBufferArrayRange>())
                .map(|b| b.get_offset())
                .unwrap_or(0);
            let idx_base_add = (ibuf_pool_offset / std::mem::size_of::<u32>()) as u32;

            gfx_cmds.draw_indexed(
                &ibuf_handle,
                &HgiDrawIndexedOp {
                    index_count,
                    base_index: self.draw_command_buffer[base + 2] + idx_base_add,
                    base_vertex: self.draw_command_buffer[base + 3] as i32,
                    instance_count,
                    base_instance: self.draw_command_buffer[base + 4],
                },
            );
            drawn += 1;
        }

        log::debug!(
            "[PipelineDrawBatch] execute_draw: drew {} / {} items",
            drawn,
            self.draw_items.len()
        );
    }

    fn append(&mut self, item: HdStDrawItemSharedPtr) -> bool {
        if !self.is_aggregated(&item) {
            return false;
        }
        self.draw_items.push(item);
        self.draw_command_buffer_dirty = true;
        true
    }

    fn item_count(&self) -> usize {
        self.draw_items.len()
    }

    fn is_empty(&self) -> bool {
        self.draw_items.is_empty()
    }
}

// ---------------------------------------------------------------------------
// DrawingProgram (port of HdSt_DrawBatch::_DrawingProgram)
// ---------------------------------------------------------------------------

/// Shader composition + resource binding state for a draw batch.
///
/// Port of HdSt_DrawBatch::_DrawingProgram. Wraps WGSL/GLSL code generation
/// and tracks binding assignments for bindable resources.
/// Each batch holds one DrawingProgram that gets compiled from draw item state.
#[derive(Debug, Default)]
pub struct DrawingProgram {
    /// Compiled shader program handle (None = not yet compiled).
    shader_program: Option<HgiShaderProgramHandle>,
    /// Shader key used to compile this program (for cache invalidation).
    program_key: Option<DrawProgramKey>,
    /// Whether the program has been linked successfully.
    is_valid: bool,
}

impl DrawingProgram {
    /// Create an empty drawing program.
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if program is compiled and valid.
    ///
    /// Port of _DrawingProgram::IsValid.
    pub fn is_valid(&self) -> bool {
        self.is_valid && self.shader_program.is_some()
    }

    /// Compile shader from a draw item's material + geometry state.
    ///
    /// Port of _DrawingProgram::CompileShader.
    /// Generates WGSL from the shader key and links the program.
    pub fn compile_shader(&mut self, hgi: &mut dyn Hgi, key: &DrawProgramKey) -> bool {
        // Skip if already compiled for same key
        if let Some(ref cached_key) = self.program_key {
            if cached_key.cache_hash() == key.cache_hash() && self.is_valid {
                return true;
            }
        }
        self.is_valid = false;
        self.shader_program = None;

        let wgsl = gen_wgsl_for_program(key);
        let program = compile_program(hgi, &wgsl.source, wgsl.vs_entry, wgsl.fs_entry);
        if let Some(prog) = program {
            self.shader_program = Some(prog);
            self.program_key = Some(key.clone());
            self.is_valid = true;
            true
        } else {
            false
        }
    }

    /// Get the compiled program handle.
    pub fn get_program(&self) -> Option<&HgiShaderProgramHandle> {
        self.shader_program.as_ref()
    }

    /// Reset the program (called when shaders are invalidated).
    ///
    /// Port of _DrawingProgram::Reset.
    pub fn reset(&mut self) {
        self.shader_program = None;
        self.program_key = None;
        self.is_valid = false;
    }
}

// ---------------------------------------------------------------------------
// IndirectDrawBatch (port of HdSt_IndirectDrawBatch)
// ---------------------------------------------------------------------------

/// GPU-side indirect draw command layout for `draw_indexed_indirect`.
///
/// Matches wgpu's `DrawIndexedIndirectArgs` (5 u32, 20 bytes).
/// Port of C++ `DrawElementsIndirectCommand` in indirectDrawBatch.cpp.
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct DrawIndexedIndirectCommand {
    /// Number of indices to draw.
    pub index_count: u32,
    /// Number of instances (0 = culled / skip).
    pub instance_count: u32,
    /// Offset into the index buffer (in indices).
    pub first_index: u32,
    /// Value added to each index before vertex fetch.
    pub base_vertex: i32,
    /// First instance (gl_BaseInstance / base_instance in WGSL).
    pub first_instance: u32,
}

/// Stride of one indirect draw command in u32 units (5: index_count, instance_count,
/// first_index, base_vertex, first_instance). Used by GPU culling passes to index
/// into the dispatch buffer.
#[allow(dead_code)]
pub const INDIRECT_CMD_NUM_UINTS: usize =
    std::mem::size_of::<DrawIndexedIndirectCommand>() / std::mem::size_of::<u32>();

/// Draw batch executed from an indirect dispatch buffer.
///
/// Port of HdSt_IndirectDrawBatch. Accepts draw items that share the same
/// primitive type and aggregated drawing resources (uniform + non-uniform
/// primvar buffers). Builds a flat dispatch buffer and issues indirect draws.
///
/// ## Indirect Draw Strategy
///
/// When all items in a batch share the same vertex and index buffer (typical
/// for aggregated Storm BAR pools), the batch uploads the compiled draw
/// commands to a GPU dispatch buffer and issues a single
/// `draw_indexed_indirect` per command. This avoids per-item CPU->GPU
/// round-trips and enables future GPU-driven culling passes to modify
/// `instance_count` in-place.
///
/// GPU frustum culling is optional (`allow_gpu_frustum_culling`).
/// When disabled, falls back to CPU-side culling via command buffer masking.
#[derive(Debug)]
pub struct IndirectDrawBatch {
    /// Draw items in this batch.
    draw_items: Vec<HdStDrawItemSharedPtr>,
    /// Flat u32 command buffer matching GPU draw dispatch layout.
    draw_command_buffer: Vec<u32>,
    /// Whether command buffer needs recompilation.
    /// AtomicBool so draw_item_instance_changed() can set it from &self (no &mut needed).
    draw_command_buffer_dirty: AtomicBool,
    /// Hash of buffer arrays (for detecting BAR reallocation).
    #[allow(dead_code)]
    buffer_arrays_hash: u64,
    /// Hash of per-item element offsets (for detecting resize).
    bar_element_offsets_hash: u64,
    /// Number of items visible after culling.
    num_visible_items: usize,
    /// Total vertices across all items.
    num_total_vertices: usize,
    /// Total index elements across all items.
    num_total_elements: usize,
    /// Compiled drawing program (shader + bindings).
    #[allow(dead_code)]
    drawing_program: DrawingProgram,
    /// Whether culling program is dirty (needs recompile).
    dirty_culling_program: bool,
    /// Whether this batch uses indexed drawing.
    use_draw_indexed: bool,
    /// Whether this batch uses GPU instancing.
    use_instancing: bool,
    /// Whether GPU frustum culling is allowed for this batch.
    allow_gpu_frustum_culling: bool,
    /// Whether tiny prim culling is enabled.
    use_tiny_prim_culling: bool,
    /// Shader key for this batch.
    program_key: DrawProgramKey,
    /// Whether the GPU dispatch buffer needs re-upload from CPU command buffer.
    dispatch_buffer_dirty: bool,
    /// GPU dispatch buffer handle for indirect draw commands (INDIRECT|STORAGE|COPY_DST).
    /// Created in encode_draw(), persists across frames, re-uploaded only when dirty.
    dispatch_buffer: Option<HgiBufferHandle>,
    /// Byte size of the current dispatch buffer (for reuse without re-allocation).
    dispatch_buffer_size: usize,
    /// Packed indirect draw commands (one per visible item).
    /// Separate from draw_command_buffer which includes DrawingCoord data.
    indirect_commands: Vec<DrawIndexedIndirectCommand>,
    /// Persistent GPU buffers for GPU frustum culling (dispatch, cull_input, item_data).
    #[cfg(feature = "gpu-culling")]
    gpu_bufs: crate::frustum_cull::CullGpuBuffers,
    /// Cached GPU frustum culling compute pipeline + bind group layout.
    #[cfg(feature = "gpu-culling")]
    cull_state: Option<crate::frustum_cull::FrustumCullState>,
}

impl IndirectDrawBatch {
    /// Create a new indirect draw batch seeded with one draw item.
    ///
    /// Port of HdSt_IndirectDrawBatch constructor.
    pub fn new(first_item: HdStDrawItemSharedPtr, allow_gpu_frustum_culling: bool) -> Self {
        let use_draw_indexed = first_item.get_element_bar().is_some();
        let use_instancing = first_item.get_instance_bar().is_some();
        let program_key = infer_program_key_from_draw_item(first_item.as_ref(), use_instancing);

        let mut batch = Self {
            draw_items: Vec::new(),
            draw_command_buffer: Vec::new(),
            draw_command_buffer_dirty: AtomicBool::new(true),
            buffer_arrays_hash: 0,
            bar_element_offsets_hash: 0,
            num_visible_items: 0,
            num_total_vertices: 0,
            num_total_elements: 0,
            drawing_program: DrawingProgram::new(),
            dirty_culling_program: true,
            use_draw_indexed,
            use_instancing,
            allow_gpu_frustum_culling,
            use_tiny_prim_culling: false,
            program_key,
            dispatch_buffer_dirty: true,
            dispatch_buffer: None,
            dispatch_buffer_size: 0,
            indirect_commands: Vec::new(),
            #[cfg(feature = "gpu-culling")]
            gpu_bufs: crate::frustum_cull::CullGpuBuffers::new(),
            #[cfg(feature = "gpu-culling")]
            cull_state: None,
        };
        batch.draw_items.push(first_item);
        batch
    }

    /// Create without GPU frustum culling.
    pub fn new_no_culling(first_item: HdStDrawItemSharedPtr) -> Self {
        Self::new(first_item, false)
    }

    /// Check if a draw item is compatible with this batch.
    ///
    /// Port of HdSt_DrawBatch::_IsAggregated. Items must share:
    /// - Valid state (has buffers)
    /// - Indexed vs non-indexed topology type
    /// - Instancing mode
    /// - Material network shader (same params = same visual output)
    /// - Buffer arrays from the same pool (vertex, index, constant BARs)
    fn is_aggregated(&self, item: &HdStDrawItemSharedPtr) -> bool {
        if self.draw_items.is_empty() {
            return true;
        }
        if !item.is_valid() {
            return false;
        }
        let first = &self.draw_items[0];

        let item_indexed = item.get_element_bar().is_some();
        let item_instanced = item.get_instance_bar().is_some();
        if item_indexed != self.use_draw_indexed || item_instanced != self.use_instancing {
            return false;
        }

        // Material network shader must match (C++ _CanAggregateMaterials).
        if first.get_material_network_shader() != item.get_material_network_shader() {
            return false;
        }

        // Buffer arrays must be from the same pool (C++ IsAggregatedWith).
        // C++ indirectDrawBatch.cpp:856-890 checks all 8 BAR types.
        if !bars_share_buffer(first.get_constant_bar(), item.get_constant_bar()) {
            return false;
        }
        if !bars_share_buffer(first.get_topology_bar(), item.get_topology_bar()) {
            return false;
        }
        if !bars_share_buffer(
            first.get_topology_visibility_bar(),
            item.get_topology_visibility_bar(),
        ) {
            return false;
        }
        if !bars_share_buffer(first.get_element_bar(), item.get_element_bar()) {
            return false;
        }
        if !bars_share_buffer(first.get_face_varying_bar(), item.get_face_varying_bar()) {
            return false;
        }
        if first.get_fvar_topology_to_primvar_vector() != item.get_fvar_topology_to_primvar_vector()
        {
            return false;
        }
        if !bars_share_buffer(first.get_varying_bar(), item.get_varying_bar()) {
            return false;
        }
        if !bars_share_buffer(first.get_vertex_bar(), item.get_vertex_bar()) {
            return false;
        }
        if !bars_share_buffer(
            first.get_instance_index_bar(),
            item.get_instance_index_bar(),
        ) {
            return false;
        }
        // Instance primvar BARs (multi-level instancing)
        let first_levels = first.get_instance_primvar_num_levels();
        let item_levels = item.get_instance_primvar_num_levels();
        if first_levels != item_levels {
            return false;
        }
        for level in 0..first_levels {
            if !bars_share_buffer(
                first.get_instance_primvar_bar(level),
                item.get_instance_primvar_bar(level),
            ) {
                return false;
            }
        }

        true
    }

    /// Compile the dispatch buffer from current draw items.
    ///
    /// Port of HdSt_IndirectDrawBatch::_CompileBatch.
    fn compile_batch(&mut self) {
        if self.draw_items.is_empty() {
            return;
        }
        // Each entry: DrawIndexedCommand (5 u32) + DrawingCoord (10 u32) = 15 u32
        let cmd_stride = 15usize;
        let num_items = self.draw_items.len();
        self.draw_command_buffer.resize(num_items * cmd_stride, 0);

        self.num_visible_items = 0;
        self.num_total_elements = 0;
        self.num_total_vertices = 0;
        self.bar_element_offsets_hash = 0;

        for (i, item) in self.draw_items.iter().enumerate() {
            let base = i * cmd_stride;
            if !item.is_valid() || !item.is_visible() {
                continue;
            }

            // index_count and first_index from element BAR.
            // Pool offset is baked into first_index so indirect dispatch works correctly.
            // Division by size_of::<u32>() converts byte offset to element index.
            let (index_count, first_index) = if let Some(ebar) = item.get_element_bar() {
                if let Some(st_bar) = ebar.as_any().downcast_ref::<HdStBufferArrayRange>() {
                    let count = (st_bar.get_size() / std::mem::size_of::<u32>()) as u32;
                    let fi = (st_bar.get_offset() / std::mem::size_of::<u32>()) as u32;
                    (count, fi)
                } else {
                    (0, 0)
                }
            } else {
                (0, 0)
            };

            // base_vertex = element (vertex) index, not byte offset — divide by vec3f stride.
            // Use positions_byte_size for vertex count to exclude packed normals+uvs.
            let (vertex_count, base_vertex) = if let Some(vbar) = item.get_vertex_bar() {
                if let Some(st_bar) = vbar.as_any().downcast_ref::<HdStBufferArrayRange>() {
                    let pos_bytes = st_bar.get_positions_byte_size();
                    let count = if pos_bytes > 0 {
                        (pos_bytes / (3 * std::mem::size_of::<f32>())) as u32
                    } else {
                        (st_bar.get_size() / (3 * std::mem::size_of::<f32>())) as u32
                    };
                    let bv = st_bar.get_offset() / (3 * std::mem::size_of::<f32>());
                    (count, bv as u32)
                } else {
                    (0, 0)
                }
            } else {
                (0, 0)
            };

            if index_count == 0 || vertex_count == 0 {
                continue;
            }

            let constant_dc = if let Some(cbar) = item.get_constant_bar() {
                if let Some(st_bar) = cbar.as_any().downcast_ref::<HdStBufferArrayRange>() {
                    st_bar.get_offset() as u32
                } else {
                    0
                }
            } else {
                0
            };
            let fvar_dc = face_varying_dc_from_item(item.as_ref());

            // DrawIndexedCommand layout (5 u32).
            // first_index includes pool sub-alloc offset (baked at compile time for indirect).
            self.draw_command_buffer[base + 0] = index_count;
            self.draw_command_buffer[base + 1] = 1; // instance_count
            self.draw_command_buffer[base + 2] = first_index; // pool offset baked in
            self.draw_command_buffer[base + 3] = base_vertex;
            self.draw_command_buffer[base + 4] = i as u32; // base_instance

            // DrawingCoord (10 u32)
            self.draw_command_buffer[base + 5] = 0; // modelDC
            self.draw_command_buffer[base + 6] = constant_dc; // constantDC
            self.draw_command_buffer[base + 7] = 0; // elementDC
            self.draw_command_buffer[base + 8] = 0; // primitiveDC (same as base_index)
            self.draw_command_buffer[base + 9] = fvar_dc;
            self.draw_command_buffer[base + 10] = 0; // instanceIndexDC
            self.draw_command_buffer[base + 11] = 0; // shaderDC
            self.draw_command_buffer[base + 12] = base_vertex; // vertexDC
            self.draw_command_buffer[base + 13] = 0; // topVisDC
            self.draw_command_buffer[base + 14] = 0; // varyingDC

            self.bar_element_offsets_hash = self
                .bar_element_offsets_hash
                .wrapping_mul(0x9e3779b97f4a7c15)
                .wrapping_add(base_vertex as u64);

            self.num_visible_items += 1;
            self.num_total_elements += index_count as usize;
            self.num_total_vertices += vertex_count as usize;
        }

        // Build packed indirect draw commands for GPU dispatch.
        // Only visible items (instance_count > 0) produce a command.
        self.indirect_commands.clear();
        for i in 0..num_items {
            let base = i * cmd_stride;
            let ic = self.draw_command_buffer[base]; // index_count
            let inst = self.draw_command_buffer[base + 1]; // instance_count
            if ic == 0 || inst == 0 {
                continue;
            }
            self.indirect_commands.push(DrawIndexedIndirectCommand {
                index_count: ic,
                instance_count: inst,
                first_index: self.draw_command_buffer[base + 2],
                base_vertex: self.draw_command_buffer[base + 3] as i32,
                first_instance: self.draw_command_buffer[base + 4],
            });
        }

        self.dispatch_buffer_dirty = true;

        self.draw_command_buffer_dirty
            .store(false, Ordering::Relaxed);
        log::debug!(
            "[IndirectDrawBatch] compiled: {} items, {} visible, {} indirect cmds",
            num_items,
            self.num_visible_items,
            self.indirect_commands.len(),
        );
    }

    fn has_nothing_to_draw(&self) -> bool {
        self.num_visible_items == 0 || self.draw_items.is_empty()
    }

    /// Execute GPU frustum culling via compute shader (NO CPU readback).
    ///
    /// Port of HdSt_IndirectDrawBatch::_ExecuteFrustumCull (compute path).
    ///
    /// The compute shader writes `instance_count = 0` for culled items directly
    /// into the GPU dispatch buffer. The subsequent `draw_indexed_indirect` reads
    /// from the SAME buffer. No CPU readback needed -- matches C++ Storm exactly.
    ///
    /// Flow:
    /// 1. Upload per-item model matrix + AABB to persistent item_data buffer
    /// 2. GPU copy: dispatch_buf -> cull_input_buf (snapshot original instance counts)
    /// 3. Compute pass: test frustum, write instance_count=0 for culled items
    /// 4. Draw pass reads from the same dispatch buffer (no readback)
    #[cfg(feature = "gpu-culling")]
    fn execute_frustum_cull(
        &mut self,
        state: &HdStRenderPassState,
    ) {
        use crate::frustum_cull::{FrustumCullState, GpuItemData};

        if !self.allow_gpu_frustum_culling || self.draw_items.is_empty() {
            return;
        }

        // Obtain wgpu device + queue from the render pass state.
        let (device_arc, queue_arc) = match state.get_wgpu_device_queue() {
            Some(dq) => (std::sync::Arc::clone(dq.0), std::sync::Arc::clone(dq.1)),
            None => {
                log::trace!("IndirectDrawBatch::execute_frustum_cull: no wgpu device, skipping");
                return;
            }
        };

        // Lazily create the culling compute pipeline.
        if self.cull_state.is_none() || self.dirty_culling_program {
            self.cull_state = FrustumCullState::new(device_arc, queue_arc);
            self.dirty_culling_program = false;
        }

        // Ensure dispatch buffer is up to date before culling.
        if self.draw_command_buffer_dirty.load(Ordering::Relaxed) {
            self.compile_batch();
        }
        self.upload_dispatch_buffer(state);

        // Build per-item data (model matrix + AABB) for the culling shader.
        let item_data: Vec<GpuItemData> = self
            .draw_items
            .iter()
            .map(|item| {
                let model = item.get_world_transform();
                GpuItemData::new(&model, item.get_bbox_min(), item.get_bbox_max())
            })
            .collect();

        // Cull matrix = view * projection (OpenGL convention, no depth remap).
        // The WGSL clip test checks [-w, w] on all axes; applying the wgpu depth
        // remap would weaken near-plane culling. Matches C++ _ExecuteFrustumCull.
        let view = state.get_view_matrix();
        let proj = state.get_proj_matrix();
        let cull_matrix = *view * *proj;
        let cull_matrix_arr = cull_matrix.to_array();

        // Tiny-prim NDC range: (0.0, -1.0) disables max-size check.
        let draw_range_ndc = if self.use_tiny_prim_culling {
            [1.0f32 / 512.0, -1.0]
        } else {
            [0.0f32, -1.0]
        };

        let cull_state = match self.cull_state.as_ref() {
            Some(cs) => cs,
            None => {
                log::warn!("IndirectDrawBatch: failed to create frustum cull pipeline");
                return;
            }
        };

        // Dispatch the GPU culling pass. Writes instance_count=0 for culled
        // items directly into the dispatch buffer. No CPU readback.
        cull_state.dispatch(
            &mut self.gpu_bufs,
            &item_data,
            &cull_matrix_arr,
            draw_range_ndc,
        );

        log::debug!(
            "IndirectDrawBatch::execute_frustum_cull: dispatched GPU cull for {} items (no readback)",
            self.draw_items.len(),
        );
    }

    /// Upload the CPU draw command buffer to the persistent GPU dispatch buffer.
    ///
    /// Port of C++ `_dispatchBuffer->CopyData(_drawCommandBuffer)`.
    /// Only uploads when dirty. The dispatch buffer has STORAGE|INDIRECT usage
    /// so it serves as both compute shader RW target and indirect draw source.
    #[cfg(feature = "gpu-culling")]
    fn upload_dispatch_buffer(&mut self, state: &HdStRenderPassState) {
        if !self.dispatch_buffer_dirty {
            return;
        }
        let (device_arc, queue_arc) = match state.get_wgpu_device_queue() {
            Some(dq) => (dq.0.as_ref(), dq.1.as_ref()),
            None => return,
        };

        self.gpu_bufs.upload_dispatch(
            device_arc,
            queue_arc,
            &self.draw_command_buffer,
            self.draw_items.len(),
        );
        self.dispatch_buffer_dirty = false;
    }

    /// Returns whether GPU frustum culling is enabled at runtime.
    pub fn is_gpu_frustum_culling_enabled() -> bool {
        cfg!(feature = "gpu-culling")
    }

    /// Returns whether GPU instance frustum culling is enabled.
    pub fn is_gpu_instance_frustum_culling_enabled() -> bool {
        false
    }

    /// Returns whether visible instance count readback from GPU is enabled.
    pub fn is_gpu_count_visible_instances_enabled() -> bool {
        false
    }

    /// Get the dispatch command buffer (CPU copy, for inspection/testing).
    pub fn get_draw_command_buffer(&self) -> &[u32] {
        &self.draw_command_buffer
    }

    /// Check if the dispatch buffer is dirty (needs re-upload).
    pub fn is_dispatch_buffer_dirty(&self) -> bool {
        self.dispatch_buffer_dirty
    }

    /// Get the GPU dispatch buffer handle (None if not yet uploaded).
    pub fn get_dispatch_buffer(&self) -> Option<&HgiBufferHandle> {
        self.dispatch_buffer.as_ref()
    }

    /// Get the packed indirect draw commands.
    pub fn get_indirect_commands(&self) -> &[DrawIndexedIndirectCommand] {
        &self.indirect_commands
    }

    /// GPU instancing path: gather all item transforms into SSBO, issue single draw call.
    fn execute_instanced_draw(
        &self,
        gfx_cmds: &mut dyn HgiGraphicsCmds,
        hgi: &mut dyn Hgi,
        fs: &FrameSetup,
        identity: &[[f64; 4]; 4],
    ) {
        if self.draw_items.is_empty() {
            return;
        }

        // Gather model transforms for all visible items as f32 (16 floats per instance).
        let cmd_stride = 15usize;
        let mut xform_data: Vec<f32> = Vec::with_capacity(self.draw_items.len() * 16);
        let mut visible_items: Vec<usize> = Vec::with_capacity(self.draw_items.len());

        for (i, item) in self.draw_items.iter().enumerate() {
            let base = i * cmd_stride;
            if base + cmd_stride > self.draw_command_buffer.len() {
                break;
            }
            let index_count = self.draw_command_buffer[base + 0];
            let instance_count = self.draw_command_buffer[base + 1];
            if index_count == 0 || instance_count == 0 {
                continue;
            }
            let model = item.get_world_transform();
            let _ = identity; // keep param for API compat
            for row in model.iter() {
                for &val in row.iter() {
                    xform_data.push(val as f32);
                }
            }
            visible_items.push(i);
        }

        let n_instances = visible_items.len();
        if n_instances == 0 {
            return;
        }

        // Upload scene uniforms with identity model (VS reads model from SSBO instead).
        let id_model = &[
            [1.0, 0.0, 0.0, 0.0f64],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let scene_data = build_scene_uniforms(
            &fs.vp,
            id_model,
            &fs.ambient_color,
            &fs.cam_pos,
            &[0.0f32; 4],
            0,
        );
        gfx_cmds.set_constant_values(&fs.pipeline, HgiShaderStage::VERTEX, 0, &scene_data);

        // Create storage buffer with instance transforms.
        // SAFETY: f32 is POD, reinterpreting as bytes for GPU upload.
        let byte_data: &[u8] = unsafe {
            std::slice::from_raw_parts(
                xform_data.as_ptr() as *const u8,
                xform_data.len() * std::mem::size_of::<f32>(),
            )
        };
        let ssbo_desc = HgiBufferDesc::new()
            .with_debug_name("instance_xforms_indirect")
            .with_usage(HgiBufferUsage::STORAGE)
            .with_byte_size(byte_data.len());
        let ssbo_handle = hgi.create_buffer(&ssbo_desc, Some(byte_data));

        // Bind SSBO at the instance group.
        let ig = instance_group_from_program_key(&fs.frame_key);
        gfx_cmds.bind_storage_buffer(ig, slots::INSTANCE_XFORMS_BINDING, &ssbo_handle);

        // Bind vertex/index buffers from the first visible item.
        let first_idx = visible_items[0];
        let first_item = &self.draw_items[first_idx];
        let (_, ibuf_handle) = match bind_packed_vertex_buffers(
            gfx_cmds,
            first_item,
            &fs.frame_key,
            first_idx,
            "IndirectInstBatch",
        ) {
            Some(v) => v,
            None => return,
        };

        // Index offset from the first item's element BAR.
        let ibuf_pool_offset = first_item
            .get_element_bar()
            .as_ref()
            .and_then(|b| b.as_any().downcast_ref::<HdStBufferArrayRange>())
            .map(|b| b.get_offset())
            .unwrap_or(0);
        let idx_base_add = (ibuf_pool_offset / std::mem::size_of::<u32>()) as u32;

        let base = first_idx * cmd_stride;
        let index_count = self.draw_command_buffer[base + 0];

        // Single instanced draw call.
        gfx_cmds.draw_indexed(
            &ibuf_handle,
            &HgiDrawIndexedOp {
                index_count,
                base_index: self.draw_command_buffer[base + 2] + idx_base_add,
                base_vertex: self.draw_command_buffer[base + 3] as i32,
                instance_count: n_instances as u32,
                base_instance: 0,
            },
        );

        log::debug!(
            "[IndirectDrawBatch] instanced draw: {} instances in 1 call",
            n_instances
        );
    }
}

impl DrawBatch for IndirectDrawBatch {
    fn rebuild(&mut self) -> bool {
        if self.draw_items.is_empty() {
            return false;
        }
        // Check all items are still compatible: topology + material + BAR pool
        let all_ok = self
            .draw_items
            .iter()
            .skip(1)
            .all(|item| self.is_aggregated(item));
        if !all_ok {
            return false;
        }
        self.draw_command_buffer_dirty
            .store(true, Ordering::Relaxed);
        self.compile_batch();
        true
    }

    fn validate(&mut self, deep: bool) -> ValidationResult {
        self.draw_items.retain(|item| item.is_valid());
        if self.draw_items.is_empty() {
            return ValidationResult::ValidBatch;
        }
        if deep || self.draw_command_buffer_dirty.load(Ordering::Relaxed) {
            let old_hash = self.bar_element_offsets_hash;
            self.compile_batch();
            if self.bar_element_offsets_hash != old_hash {
                return ValidationResult::RebuildBatch;
            }
        }
        ValidationResult::ValidBatch
    }

    fn prepare_draw(
        &mut self,
        _gfx_cmds: &mut dyn HgiGraphicsCmds,
        _state: &HdStRenderPassState,
        _registry: &HdStResourceRegistry,
    ) {
        // Recompile command buffer if dirty. Frustum culling runs in execute_draw
        // once model_transforms are available (they are not available here).
        if self.draw_command_buffer_dirty.load(Ordering::Relaxed) {
            self.compile_batch();
        }
    }

    fn encode_draw(
        &mut self,
        _state: &HdStRenderPassState,
        registry: &HdStResourceRegistry,
        _first_draw_batch: bool,
    ) {
        if self.draw_command_buffer_dirty.load(Ordering::Relaxed) {
            self.compile_batch();
        }

        // Upload indirect draw commands to GPU dispatch buffer.
        // Port of C++ _dispatchBuffer->CopyData(_drawCommandBuffer).
        //
        // Buffer uses INDIRECT | STORAGE:
        // - INDIRECT: required for draw_indexed_indirect
        // - STORAGE: enables GPU culling shader to modify instance_count in-place
        //
        // Buffer is created once and re-uploaded only when dirty (P1-4 fix: no leak).
        if self.dispatch_buffer_dirty && !self.indirect_commands.is_empty() {
            let cmd_bytes: &[u8] = unsafe {
                std::slice::from_raw_parts(
                    self.indirect_commands.as_ptr() as *const u8,
                    self.indirect_commands.len()
                        * std::mem::size_of::<DrawIndexedIndirectCommand>(),
                )
            };
            let needed_size = cmd_bytes.len();

            // Reuse existing buffer if size matches, otherwise allocate new one.
            let needs_new =
                self.dispatch_buffer.is_none() || self.dispatch_buffer_size != needed_size;

            if needs_new {
                let usage = HgiBufferUsage::INDIRECT | HgiBufferUsage::STORAGE;
                let buf = registry.allocate_buffer_with_usage(usage, needed_size);
                registry.upload_to_buffer(&buf, cmd_bytes);
                self.dispatch_buffer = Some(buf.get_handle().clone());
                self.dispatch_buffer_size = needed_size;
            } else if let Some(ref handle) = self.dispatch_buffer {
                // Re-upload to existing buffer via blit
                unsafe {
                    registry.copy_buffer_cpu_to_gpu(handle, cmd_bytes.as_ptr(), needed_size, 0);
                }
                registry.submit_blit_work(usd_hgi::enums::HgiSubmitWaitType::NoWait);
            }

            self.dispatch_buffer_dirty = false;
            log::debug!(
                "[IndirectDrawBatch] uploaded {} indirect cmds ({} bytes) to GPU (new={})",
                self.indirect_commands.len(),
                needed_size,
                needs_new,
            );
        }
    }

    fn execute_draw(
        &mut self,
        gfx_cmds: &mut dyn HgiGraphicsCmds,
        hgi: &mut dyn Hgi,
        state: &HdStRenderPassState,
        _first_draw_batch: bool,
    ) {
        // Ensure command buffer is up to date.
        if self.draw_command_buffer_dirty.load(Ordering::Relaxed) {
            self.compile_batch();
        }

        // --- GPU Frustum Culling ---
        // Run per-batch before the draw loop. The compute shader writes
        // instance_count=0 for culled items directly into the GPU dispatch
        // buffer. The draw pass reads from the SAME buffer via indirect draw.
        // No CPU readback needed -- matches C++ Storm architecture.
        #[cfg(feature = "gpu-culling")]
        self.execute_frustum_cull(state);

        if self.has_nothing_to_draw() {
            return;
        }

        // Ensure dispatch buffer is uploaded (may have been done in cull, but
        // also needed when culling is disabled or not available).
        #[cfg(feature = "gpu-culling")]
        self.upload_dispatch_buffer(state);

        // Build per-frame shader key.
        // IndirectDrawBatch does not apply scene_material toggle (scene_mats_controlled=false).
        let frame_key = build_frame_key(&self.program_key, state, false);

        // Set up pipeline + upload shared per-frame uniforms (lights, material, IBL).
        let fs = match begin_frame_draw(
            hgi,
            gfx_cmds,
            state,
            frame_key,
            &self.draw_items,
            "IndirectDrawBatch",
        ) {
            Some(fs) => fs,
            None => return,
        };

        let identity = [
            [1.0_f64, 0., 0., 0.],
            [0., 1., 0., 0.],
            [0., 0., 1., 0.],
            [0., 0., 0., 1.],
        ];

        // GPU instancing path: all items share the same mesh, one draw call with SSBO.
        if self.use_instancing && !program_key_uses_face_varying_storage(&fs.frame_key) {
            self.execute_instanced_draw(gfx_cmds, hgi, &fs, &identity);
            return;
        }

        // Determine if GPU dispatch buffer is available for indirect draws.
        #[cfg(feature = "gpu-culling")]
        let use_gpu_cull_indirect = self.gpu_bufs.get_dispatch_handle().is_some();
        #[cfg(not(feature = "gpu-culling"))]
        let use_gpu_cull_indirect = false;

        let use_indirect = use_gpu_cull_indirect
            || (self.dispatch_buffer.is_some() && !self.indirect_commands.is_empty());

        // Stride of one indirect draw command in bytes (5 u32 = 20 bytes).
        let cmd_stride_bytes = std::mem::size_of::<DrawIndexedIndirectCommand>();

        let cmd_stride = 15usize;
        let mut indirect_idx = 0usize;
        let mut drawn = 0u32;
        for (i, item) in self.draw_items.iter().enumerate() {
            let base = i * cmd_stride;
            if base + cmd_stride > self.draw_command_buffer.len() {
                break;
            }
            let index_count = self.draw_command_buffer[base + 0];
            let instance_count = self.draw_command_buffer[base + 1];
            // Skip items that were invalid at compile time (index_count=0).
            // When using indirect draws, instance_count check is on GPU side.
            if index_count == 0 || (!use_indirect && instance_count == 0) {
                continue;
            }

            let item_xform = item.get_world_transform();
            let model = &item_xform;
            let sel_color = if state.is_path_selected(item.get_prim_path()) {
                let c = state.get_selection_color();
                [c[0], c[1], c[2], c[3]]
            } else {
                [0.0f32; 4]
            };
            let scene_data = build_scene_uniforms(
                &fs.vp,
                model,
                &fs.ambient_color,
                &fs.cam_pos,
                &sel_color,
                bind_face_varying_storage(gfx_cmds, item.as_ref(), &fs.frame_key),
            );
            gfx_cmds.set_constant_values(&fs.pipeline, HgiShaderStage::VERTEX, 0, &scene_data);

            // Bind packed vertex buffer (positions / normals / uvs) and get buffer handles.
            let (_, ibuf) =
                match bind_packed_vertex_buffers(gfx_cmds, item, &fs.frame_key, i, "IndirectBatch")
                {
                    Some(v) => v,
                    None => continue,
                };

            // Issue draw. When GPU dispatch buffer is available, use indirect draw.
            if use_indirect {
                #[cfg(feature = "gpu-culling")]
                let (dispatch_handle, byte_offset, stride) = if use_gpu_cull_indirect {
                    let handle = self.gpu_bufs.get_dispatch_handle().unwrap();
                    let dispatch_cmd_stride = (crate::frustum_cull::DRAW_CMD_NUM_UINTS as usize)
                        * std::mem::size_of::<u32>();
                    (handle, i * dispatch_cmd_stride, dispatch_cmd_stride as u32)
                } else {
                    let handle = self.dispatch_buffer.as_ref().unwrap();
                    (
                        handle,
                        indirect_idx * cmd_stride_bytes,
                        cmd_stride_bytes as u32,
                    )
                };
                #[cfg(not(feature = "gpu-culling"))]
                let (dispatch_handle, byte_offset, stride) = {
                    let handle = self.dispatch_buffer.as_ref().unwrap();
                    (
                        handle,
                        indirect_idx * cmd_stride_bytes,
                        cmd_stride_bytes as u32,
                    )
                };
                gfx_cmds.draw_indexed_indirect(
                    &ibuf,
                    &HgiDrawIndirectOp {
                        draw_buffer: dispatch_handle.clone(),
                        draw_buffer_byte_offset: byte_offset,
                        draw_count: 1,
                        stride,
                    },
                );
                indirect_idx += 1;
            } else {
                // Fallback: direct indexed draw from CPU command buffer.
                gfx_cmds.draw_indexed(
                    &ibuf,
                    &HgiDrawIndexedOp {
                        index_count,
                        base_index: self.draw_command_buffer[base + 2],
                        base_vertex: self.draw_command_buffer[base + 3] as i32,
                        instance_count,
                        base_instance: self.draw_command_buffer[base + 4],
                    },
                );
            }
            drawn += 1;
        }
        log::debug!(
            "[IndirectDrawBatch] drew {}/{} items (indirect={})",
            drawn,
            self.draw_items.len(),
            use_indirect,
        );
    }

    fn append(&mut self, item: HdStDrawItemSharedPtr) -> bool {
        if !self.is_aggregated(&item) {
            return false;
        }
        self.draw_items.push(item);
        self.draw_command_buffer_dirty
            .store(true, Ordering::Relaxed);
        true
    }

    fn draw_item_instance_changed(&self, _item: &HdStDrawItemSharedPtr) {
        // Set dirty atomically from &self — safe because field is AtomicBool.
        // Port of HdSt_IndirectDrawBatch::DrawItemInstanceChanged which sets
        // _drawCommandBufferDirty = true under a shared (read) lock.
        self.draw_command_buffer_dirty
            .store(true, Ordering::Release);
        log::trace!("IndirectDrawBatch: draw_item_instance_changed => dirty");
    }

    fn set_enable_tiny_prim_culling(&mut self, enable: bool) {
        if self.use_tiny_prim_culling != enable {
            self.use_tiny_prim_culling = enable;
            self.dirty_culling_program = true;
        }
    }

    fn item_count(&self) -> usize {
        self.draw_items.len()
    }

    fn is_empty(&self) -> bool {
        self.draw_items.is_empty()
    }
}

// ---------------------------------------------------------------------------
// HdStDrawBatch (legacy flat batch, retained for backward compat)
// ---------------------------------------------------------------------------

/// Legacy draw batch for grouping compatible draw items.
///
/// Flat batch that wraps the PipelineDrawBatch trait for simple use cases.
/// Kept for backward compatibility with code that uses HdStDrawBatch directly.
#[derive(Debug)]
pub struct HdStDrawBatch {
    /// Collection of draw items in this batch
    draw_items: Vec<HdStDrawItemSharedPtr>,

    /// Whether batch needs validation
    needs_validation: bool,

    /// Cached draw command data for multi-draw
    #[allow(dead_code)]
    draw_commands: Vec<DrawCommand>,

    /// Active draw-program key inferred from the first item.
    /// Determines shader family (mesh/points/curves), pipeline topology,
    /// and vertex buffer binding contract.
    program_key: Option<DrawProgramKey>,
}

/// Individual draw command for multi-draw indirect.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct DrawCommand {
    /// Number of indices to draw
    count: u32,
    /// Number of instances
    instance_count: u32,
    /// First index in buffer
    first_index: u32,
    /// Base vertex offset
    base_vertex: i32,
    /// Base instance for instancing
    base_instance: u32,
}

impl HdStDrawBatch {
    /// Create a new empty draw batch.
    pub fn new() -> Self {
        Self {
            draw_items: Vec::new(),
            needs_validation: true,
            draw_commands: Vec::new(),
            program_key: None,
        }
    }

    /// Add a draw item to the batch.
    pub fn add_draw_item(&mut self, item: HdStDrawItemSharedPtr) {
        if self.program_key.is_none() {
            let is_instanced = item.get_instance_bar().is_some();
            self.program_key = Some(infer_program_key_from_draw_item(item.as_ref(), is_instanced));
        }
        self.draw_items.push(item);
        self.needs_validation = true;
    }

    /// Get all draw items in the batch.
    pub fn get_draw_items(&self) -> &[HdStDrawItemSharedPtr] {
        &self.draw_items
    }

    /// Get number of draw items in batch.
    pub fn get_item_count(&self) -> usize {
        self.draw_items.len()
    }

    /// Check if batch is empty.
    pub fn is_empty(&self) -> bool {
        self.draw_items.is_empty()
    }

    /// Clear all draw items.
    pub fn clear(&mut self) {
        self.draw_items.clear();
        self.draw_commands.clear();
        self.needs_validation = false;
    }

    /// Pre-draw preparation: recompile dirty command buffer (PrepareDraw phase).
    ///
    /// Mirrors C++ HdSt_DrawBatch::PrepareDraw. For the legacy HdStDrawBatch,
    /// actual compilation happens in validate()/build_draw_commands().
    /// GPU-side work (culling dispatch, lighting UBO) is a no-op here.
    pub fn prepare_draw(
        &self,
        _gfx_cmds: &mut dyn HgiGraphicsCmds,
        _state: &HdStRenderPassState,
        _registry: &HdStResourceRegistry,
    ) {
        // Preparation is handled lazily in validate()/build_draw_commands()
        // called from execute_draw. No GPU-side work at this stage for the
        // legacy batch path.
    }

    /// Validate the batch.
    ///
    /// Checks that all draw items are compatible and can
    /// actually be batched together.
    pub fn validate(&mut self) -> bool {
        if !self.needs_validation {
            return !self.draw_items.is_empty();
        }

        // Remove invalid or invisible items
        self.draw_items
            .retain(|item| item.is_valid() && item.is_visible());

        // Build draw commands for remaining items
        self.build_draw_commands();

        self.needs_validation = false;
        !self.draw_items.is_empty()
    }

    /// Build draw command data from draw items.
    fn build_draw_commands(&mut self) {
        self.draw_commands.clear();

        for item in &self.draw_items {
            // Get element (index) buffer info
            if let Some(element_bar) = item.get_element_bar() {
                if element_bar.is_valid() {
                    self.draw_commands.push(DrawCommand {
                        count: 0,
                        instance_count: 1,
                        first_index: 0,
                        base_vertex: 0,
                        base_instance: 0,
                    });
                }
            }
        }
    }

    /// Legacy GL execute path — no-op, use execute_draw() for wgpu.
    pub fn execute(&self) {}

    /// Execute draw batch through HGI graphics commands.
    ///
    /// Uses the shared `begin_frame_draw` + `bind_packed_vertex_buffers` helpers
    /// so mesh, points, and basis-curves items all get the correct shader,
    /// pipeline topology, and vertex-buffer binding contract.
    pub fn execute_draw(
        &self,
        gfx_cmds: &mut dyn HgiGraphicsCmds,
        hgi: &mut dyn Hgi,
        state: &HdStRenderPassState,
        prim_ids_by_path: Option<&HashMap<SdfPath, i32>>,
    ) {
        if self.draw_items.is_empty() {
            return;
        }
        log::info!(
            "[draw_batch] execute_draw: {} items, program_key={}",
            self.draw_items.len(),
            self.program_key.as_ref().map(|k| k.debug_label()).unwrap_or("none")
        );

        // Resolve the program key from the cached value (set by add_draw_item)
        // or infer from the first valid/visible item.
        let base_program_key = self.program_key.clone().unwrap_or_else(|| {
            self.draw_items
                .iter()
                .find(|item| item.is_valid() && item.is_visible())
                .map(|item| {
                    let inst = item.get_instance_bar().is_some();
                    infer_program_key_from_draw_item(item.as_ref(), inst)
                })
                .unwrap_or_else(|| DrawProgramKey::Mesh(MeshShaderKey::fallback()))
        });

        // Build per-frame key (applies depth-only / scene-materials / IBL overrides).
        // scene_mats_controlled=true matches HdStDrawBatch semantics.
        let frame_key = build_frame_key(&base_program_key, state, true);

        // Set up pipeline + upload shared per-frame uniforms (lights, material, IBL).
        let fs = match begin_frame_draw(
            hgi,
            gfx_cmds,
            state,
            frame_key,
            &self.draw_items,
            "HdStDrawBatch",
        ) {
            Some(fs) => fs,
            None => return,
        };

        let is_pick_id_pass = state
            .get_aov_bindings()
            .iter()
            .any(|a| a.aov_name == "primId");

        // Draw each item with per-item model transform from Hydra shared data
        let mut drawn = 0u32;
        let mut skipped = 0u32;
        for (i, item) in self.draw_items.iter().enumerate() {
            if !item.is_valid() || !item.is_visible() {
                skipped += 1;
                continue;
            }

            let item_xform = item.get_world_transform();
            let model = &item_xform;
            let ambient_color = if is_pick_id_pass {
                let prim_id = prim_ids_by_path
                    .and_then(|m| m.get(item.get_prim_path()))
                    .copied()
                    .unwrap_or(-1);
                encode_prim_id_to_color(prim_id)
            } else {
                fs.ambient_color
            };
            let sel_color = if state.is_path_selected(item.get_prim_path()) {
                let c = state.get_selection_color();
                [c[0], c[1], c[2], c[3]]
            } else {
                [0.0f32; 4]
            };
            let scene_data = build_scene_uniforms(
                &fs.vp,
                model,
                &ambient_color,
                &fs.cam_pos,
                &sel_color,
                bind_face_varying_storage(gfx_cmds, item.as_ref(), &fs.frame_key),
            );
            gfx_cmds.set_constant_values(&fs.pipeline, HgiShaderStage::VERTEX, 0, &scene_data);

            // Bind vertex/index buffers via the shared dispatch that handles
            // mesh, points, and curves vertex layouts correctly.
            let (_, ibuf) =
                match bind_packed_vertex_buffers(gfx_cmds, item, &fs.frame_key, i, "HdStBatch") {
                    Some(v) => v,
                    None => {
                        skipped += 1;
                        continue;
                    }
                };

            // Derive index count from the element BAR via extract_buffer.
            let (_, ibuf_size, ibuf_pool_offset) = match extract_buffer(item.get_element_bar()) {
                Some(v) => v,
                None => continue,
            };
            let index_count = (ibuf_size / std::mem::size_of::<u32>()) as u32;
            if index_count == 0 {
                continue;
            }

            let idx_base_add = (ibuf_pool_offset / std::mem::size_of::<u32>()) as u32;
            if !matches!(fs.frame_key, DrawProgramKey::Mesh(_)) {
                log::info!(
                    "[draw_batch] non-mesh draw: path={} idx_count={} idx_base={} ibuf_size={} ibuf_offset={} vbuf_size={}",
                    item.get_prim_path(),
                    index_count,
                    idx_base_add,
                    ibuf_size,
                    ibuf_pool_offset,
                    extract_buffer(item.get_vertex_bar()).map(|(_, s, _)| s).unwrap_or(0),
                );
            }
            gfx_cmds.draw_indexed(
                &ibuf,
                &HgiDrawIndexedOp {
                    index_count,
                    base_index: idx_base_add,
                    base_vertex: 0,
                    instance_count: 1,
                    base_instance: 0,
                },
            );
            drawn += 1;
        }
        log::info!(
            "[draw_batch] drew={} skipped={} total={} key={}",
            drawn,
            skipped,
            self.draw_items.len(),
            self.program_key.as_ref().map(|k| k.debug_label()).unwrap_or("none")
        );
    }
}

impl Default for HdStDrawBatch {
    fn default() -> Self {
        Self::new()
    }
}

/// Shared pointer to draw batch.
pub type HdStDrawBatchSharedPtr = Arc<HdStDrawBatch>;

// --- Shared execute_draw helpers (eliminate Pipeline/IndirectDrawBatch duplication) ---

/// Result of per-frame pipeline setup, returned by `begin_frame_draw`.
struct FrameSetup {
    pipeline: HgiGraphicsPipelineHandle,
    frame_key: DrawProgramKey,
    vp: usd_gf::Matrix4d,
    ambient_color: [f32; 4],
    cam_pos: [f32; 4],
}

/// Return the typed vertex BAR for a draw item when available.
fn vertex_bar_from_draw_item(item: &HdStDrawItem) -> Option<HdBufferArrayRangeSharedPtr> {
    item.get_vertex_bar()
}

/// Return true when the retained face-varying mapping contains a named primvar.
fn draw_item_has_fvar_primvar(item: &HdStDrawItem, names: &[&str]) -> bool {
    item.get_fvar_topology_to_primvar_vector()
        .iter()
        .flat_map(|entry| entry.primvars.iter())
        .any(|token| names.iter().any(|name| token.as_str() == *name))
}

/// Extract the drawing-coordinate offset for the face-varying BAR.
///
/// The live WGSL path indexes `face_varying_data` as `array<u32>`, so the drawing
/// coordinate must be expressed in 32-bit words, not raw bytes.
fn face_varying_dc_from_item(item: &HdStDrawItem) -> u32 {
    let Some(bar) = item.get_face_varying_bar() else {
        return 0;
    };
    bar.as_any()
        .downcast_ref::<HdStBufferArrayRange>()
        .map(|bar| (bar.get_offset() / std::mem::size_of::<u32>()) as u32)
        .unwrap_or(0)
}

/// Bind the face-varying storage BAR for one draw item and return its base word offset.
fn bind_face_varying_storage(
    gfx_cmds: &mut dyn HgiGraphicsCmds,
    item: &HdStDrawItem,
    program_key: &DrawProgramKey,
) -> u32 {
    let DrawProgramKey::Mesh(shader_key) = program_key else {
        return 0;
    };
    let uses_face_varying = shader_key_uses_face_varying_storage(shader_key);
    if !uses_face_varying {
        return 0;
    }

    let Some((buffer_handle, _, offset_bytes)) = extract_buffer(item.get_face_varying_bar()) else {
        return 0;
    };
    let group = slots::face_varying_group(
        shader_key.has_uv,
        shader_key.has_ibl,
        shader_key.use_shadows,
        shader_key.use_instancing,
    );
    gfx_cmds.bind_storage_buffer(group, slots::FACE_VARYING_BINDING, &buffer_handle);
    (offset_bytes / std::mem::size_of::<u32>()) as u32
}

fn shader_key_uses_face_varying_storage(shader_key: &MeshShaderKey) -> bool {
    shader_key.has_fvar_normals
        || shader_key.has_fvar_uv
        || shader_key.has_fvar_color
        || shader_key.has_fvar_opacity
}

fn program_key_uses_face_varying_storage(program_key: &DrawProgramKey) -> bool {
    match program_key {
        DrawProgramKey::Mesh(key) => shader_key_uses_face_varying_storage(key),
        DrawProgramKey::Points(_) | DrawProgramKey::BasisCurves(_) => false,
    }
}

fn instance_group_from_program_key(program_key: &DrawProgramKey) -> u32 {
    match program_key {
        DrawProgramKey::Mesh(key) => {
            slots::instance_group(key.has_uv, key.has_ibl, key.use_shadows)
        }
        DrawProgramKey::Points(_) | DrawProgramKey::BasisCurves(_) => {
            slots::instance_group(false, false, false)
        }
    }
}

/// Derive the coarse mesh shader key from the retained draw item.
///
/// `_ref` chooses shader/codegen state from draw-item data rather than from a
/// loose collection of booleans reconstructed later from packed BAR sizes.
/// Rust still lacks full face-varying shader plumbing, but keeping this helper
/// draw-item-driven prevents the batch layer from collapsing face-varying and
/// vertex-authored variants onto the same key.
fn infer_mesh_shader_key_from_draw_item(
    item: &HdStDrawItem,
    use_instancing: bool,
) -> MeshShaderKey {
    let topology = item.get_primitive_topology();
    let vertex_bar = vertex_bar_from_draw_item(item);
    let has_normals = vertex_bar
        .as_ref()
        .and_then(|bar| bar.as_any().downcast_ref::<HdStBufferArrayRange>())
        .map(|bar| bar.get_normals_byte_size() > 0)
        .unwrap_or(false);
    let has_uv = vertex_bar
        .as_ref()
        .and_then(|bar| bar.as_any().downcast_ref::<HdStBufferArrayRange>())
        .map(|bar| bar.get_uvs_byte_size() > 0)
        .unwrap_or(false);
    let has_color = vertex_bar
        .as_ref()
        .and_then(|bar| bar.as_any().downcast_ref::<HdStBufferArrayRange>())
        .map(|bar| bar.get_colors_byte_size() > 0)
        .unwrap_or(false);
    let has_fvar_data = item.get_face_varying_bar().is_some();
    let has_fvar_normals = has_fvar_data && draw_item_has_fvar_primvar(item, &["normals"]);
    let has_fvar_uv = has_fvar_data
        && draw_item_has_fvar_primvar(item, &["st", "st0", "st1", "uv", "uvs", "map1"]);
    let has_fvar_color = has_fvar_data && draw_item_has_fvar_primvar(item, &["displayColor"]);
    let has_fvar_opacity = has_fvar_data && draw_item_has_fvar_primvar(item, &["displayOpacity"]);

    let shading = match topology {
        crate::mesh_shader_key::DrawTopology::TriangleList => ShadingModel::BlinnPhong,
        crate::mesh_shader_key::DrawTopology::LineList
        | crate::mesh_shader_key::DrawTopology::PointList => ShadingModel::FlatColor,
    };

    MeshShaderKey {
        shading,
        topology,
        has_normals: matches!(topology, crate::mesh_shader_key::DrawTopology::TriangleList)
            && (has_normals || has_fvar_normals),
        has_color: has_color || has_fvar_color,
        has_uv: matches!(topology, crate::mesh_shader_key::DrawTopology::TriangleList)
            && (has_uv || has_fvar_uv),
        normal_interp: if has_fvar_normals {
            PrimvarInterp::FaceVarying
        } else {
            PrimvarInterp::Vertex
        },
        has_fvar_normals: matches!(topology, crate::mesh_shader_key::DrawTopology::TriangleList)
            && has_fvar_normals,
        has_fvar_uv: matches!(topology, crate::mesh_shader_key::DrawTopology::TriangleList)
            && has_fvar_uv,
        has_fvar_color,
        has_fvar_opacity,
        use_instancing,
        ..Default::default()
    }
}

fn infer_program_key_from_draw_item(
    item: &HdStDrawItem,
    use_instancing: bool,
) -> DrawProgramKey {
    let vertex_bar = vertex_bar_from_draw_item(item);
    match item.get_primitive_kind() {
        DrawPrimitiveKind::Mesh => DrawProgramKey::Mesh(infer_mesh_shader_key_from_draw_item(
            item,
            use_instancing,
        )),
        DrawPrimitiveKind::Points => {
            let has_color = vertex_bar
                .as_ref()
                .and_then(|bar| bar.as_any().downcast_ref::<HdStBufferArrayRange>())
                .map(|bar| bar.get_colors_byte_size() > 0)
                .unwrap_or(false);
            let has_widths = vertex_bar
                .as_ref()
                .and_then(|bar| bar.as_any().downcast_ref::<HdStBufferArrayRange>())
                .map(|bar| bar.get_normals_byte_size() > 0)
                .unwrap_or(false);
            DrawProgramKey::Points(PointsProgramKey {
                shader_key: PointsShaderKey::new(false),
                has_color,
                has_widths,
                depth_only: false,
                pick_buffer_rw: false,
                use_instancing,
            })
        }
        DrawPrimitiveKind::BasisCurves => {
            let has_color = vertex_bar
                .as_ref()
                .and_then(|bar| bar.as_any().downcast_ref::<HdStBufferArrayRange>())
                .map(|bar| bar.get_colors_byte_size() > 0)
                .unwrap_or(false);
            let has_normals = vertex_bar
                .as_ref()
                .and_then(|bar| bar.as_any().downcast_ref::<HdStBufferArrayRange>())
                .map(|bar| bar.get_uvs_byte_size() > 0)
                .unwrap_or(false);
            let has_widths = vertex_bar
                .as_ref()
                .and_then(|bar| bar.as_any().downcast_ref::<HdStBufferArrayRange>())
                .map(|bar| bar.get_normals_byte_size() > 0)
                .unwrap_or(false);
            let draw_style = match item.get_primitive_topology() {
                crate::mesh_shader_key::DrawTopology::PointList => CurveDrawStyle::Points,
                _ => CurveDrawStyle::Wire,
            };
            DrawProgramKey::BasisCurves(BasisCurvesProgramKey {
                shader_key: BasisCurvesShaderKey::new(
                    usd_tf::Token::new("linear"),
                    usd_tf::Token::new("bezier"),
                    draw_style,
                    BASIS_CURVES_DEFAULT_NORMAL_STYLE,
                    has_widths,
                    has_normals,
                    usd_tf::Token::new(""),
                    false,
                    false,
                    false,
                    false,
                ),
                has_color,
                has_normals,
                has_widths,
                depth_only: false,
                pick_buffer_rw: false,
                use_instancing,
            })
        }
    }
}

/// Build frame_key from render pass state using the batch's base `shader_key`.
///
/// Shared between PipelineDrawBatch and IndirectDrawBatch. Handles depth-only,
/// IBL, flat shading, and scene material toggles.
///
/// `scene_mats_controlled` — when `true` the caller respects `state.get_enable_scene_materials()`
/// (PipelineDrawBatch); when `false` scene-materials check is omitted (IndirectDrawBatch).
fn build_frame_key(
    base_key: &DrawProgramKey,
    state: &HdStRenderPassState,
    scene_mats_controlled: bool,
) -> DrawProgramKey {
    match base_key {
        DrawProgramKey::Mesh(base_key) => {
            DrawProgramKey::Mesh(build_frame_mesh_key(base_key, state, scene_mats_controlled))
        }
        DrawProgramKey::Points(base_key) => DrawProgramKey::Points(PointsProgramKey {
            depth_only: state.is_depth_only(),
            pick_buffer_rw: state
                .get_aov_bindings()
                .iter()
                .any(|a| a.aov_name == "primId")
                && state.get_pick_buffer().is_some(),
            ..base_key.clone()
        }),
        DrawProgramKey::BasisCurves(base_key) => {
            DrawProgramKey::BasisCurves(BasisCurvesProgramKey {
                depth_only: state.is_depth_only(),
                pick_buffer_rw: state
                    .get_aov_bindings()
                    .iter()
                    .any(|a| a.aov_name == "primId")
                    && state.get_pick_buffer().is_some(),
                ..base_key.clone()
            })
        }
    }
}

fn build_frame_mesh_key(
    base_key: &MeshShaderKey,
    state: &HdStRenderPassState,
    scene_mats_controlled: bool,
) -> MeshShaderKey {
    let scene_mats = !scene_mats_controlled || state.get_enable_scene_materials();
    let has_ibl = scene_mats && state.has_ibl();
    let flat = state.is_flat_shading();
    let is_pick_id_pass = state
        .get_aov_bindings()
        .iter()
        .any(|a| a.aov_name == "primId");
    let has_pick_buffer = state.get_pick_buffer().is_some();

    if state.is_depth_only() {
        MeshShaderKey {
            depth_only: true,
            shading: ShadingModel::FlatColor,
            has_normals: false,
            has_uv: false,
            has_color: false,
            has_fvar_normals: false,
            has_fvar_uv: false,
            has_fvar_color: false,
            has_fvar_opacity: false,
            has_ibl: false,
            ..base_key.clone()
        }
    } else if is_pick_id_pass {
        MeshShaderKey {
            shading: ShadingModel::FlatColor,
            has_normals: false,
            has_uv: false,
            has_color: false,
            has_fvar_normals: false,
            has_fvar_uv: false,
            has_fvar_color: false,
            has_fvar_opacity: false,
            has_ibl: false,
            use_shadows: false,
            pick_buffer_rw: has_pick_buffer,
            ..base_key.clone()
        }
    } else {
        MeshShaderKey {
            has_ibl,
            flat_shading: flat,
            use_shadows: state.has_shadows(),
            pick_buffer_rw: false,
            has_normals: base_key.has_normals && !flat,
            has_fvar_normals: base_key.has_fvar_normals && !flat,
            shading: if !scene_mats {
                // Flat shading flag selects face-normal GeomFlat vs vertex-normal GeomSmooth
                if flat {
                    ShadingModel::GeomFlat
                } else {
                    ShadingModel::GeomSmooth
                }
            } else if has_ibl && base_key.shading != ShadingModel::FlatColor {
                ShadingModel::Pbr
            } else {
                base_key.shading
            },
            has_uv: if scene_mats { base_key.has_uv } else { false },
            has_color: if scene_mats {
                base_key.has_color
            } else {
                false
            },
            has_fvar_uv: if scene_mats {
                base_key.has_fvar_uv
            } else {
                false
            },
            has_fvar_color: if scene_mats {
                base_key.has_fvar_color
            } else {
                false
            },
            has_fvar_opacity: if scene_mats {
                base_key.has_fvar_opacity
            } else {
                false
            },
            ..base_key.clone()
        }
    }
}

/// Set up pipeline + upload per-frame shared uniforms (lights, material, textures, IBL).
///
/// Returns `None` if pipeline creation fails (caller should early-return).
/// Shared between PipelineDrawBatch and IndirectDrawBatch execute_draw.
fn begin_frame_draw(
    hgi: &mut dyn Hgi,
    gfx_cmds: &mut dyn HgiGraphicsCmds,
    state: &HdStRenderPassState,
    frame_key: DrawProgramKey,
    draw_items: &[HdStDrawItemSharedPtr],
    batch_label: &str,
) -> Option<FrameSetup> {
    let overrides = PipelineStateOverrides::from_state(state);
    let polygon_mode = state.get_polygon_raster_mode();
    let sample_item = draw_items.first().map(|item| item.as_ref());
    let pipeline = match get_or_create_pipeline_with_polygon_mode(
        hgi,
        &frame_key,
        sample_item,
        polygon_mode,
        &overrides,
    ) {
        Some(p) => p,
        None => {
            log::warn!("{}::execute_draw: failed to create pipeline", batch_label);
            return None;
        }
    };

    // Build VP with wgpu depth remap: Z' = 0.5*Z + 0.5*W (NDC [-1,1] -> [0,1]).
    let view = state.get_view_matrix();
    let proj = state.get_proj_matrix();
    let mut proj_wgpu = *proj;
    for r in 0..4 {
        proj_wgpu[r][2] = proj[r][2] * 0.5 + proj[r][3] * 0.5;
    }
    let vp = *view * proj_wgpu;
    let amb = state.get_default_material_ambient();
    let ambient_color: [f32; 4] = [amb, amb, amb, 1.0];
    let cam_pos = extract_camera_pos(view);

    let (vx, vy, vw, vh) = state.get_viewport();
    gfx_cmds.set_viewport(&HgiViewport::new(vx, vy, vw, vh));
    gfx_cmds.bind_pipeline(&pipeline);

    let needs_lighting = frame_key.needs_lighting_uniforms();
    if needs_lighting {
        // Upload light uniforms (shared across all items)
        let lights: &[lighting::LightGpuData] = if state.has_scene_lights() {
            state.get_scene_lights()
        } else {
            &[]
        };
        let fallback;
        let light_slice = if lights.is_empty() {
            fallback = lighting::default_lights();
            fallback.as_slice()
        } else {
            lights
        };
        // When shadows are active, pack shadow entries after lights in the same UBO.
        // Matches the combined LightUniforms WGSL struct layout.
        let light_data = match &frame_key {
            DrawProgramKey::Mesh(mesh_key) if mesh_key.use_shadows => {
                lighting::build_light_and_shadow_uniforms(light_slice, state.get_shadow_entries())
            }
            _ => lighting::build_light_uniforms(light_slice),
        };
        gfx_cmds.set_constant_values(&pipeline, HgiShaderStage::FRAGMENT, 1, &light_data);
    }

    // Non-lit programs (curves, points) still use @group(2) for material in WGSL.
    // wgpu requires all bind groups up to the max used index to be bound.
    // Bind an empty lighting UBO at group 1 so the gap is filled.
    if !needs_lighting {
        let empty_light_data = lighting::build_light_uniforms(&[]);
        gfx_cmds.set_constant_values(&pipeline, HgiShaderStage::FRAGMENT, 1, &empty_light_data);
    }

    if frame_key.needs_material_uniforms() {
        let mut default_mat = wgsl_code_gen::MaterialParams::default();
        default_mat.roughness = 1.0 - state.get_default_material_specular();
        let first_mat = draw_items
            .first()
            .map(|item| item.get_material_network_shader())
            .unwrap_or(default_mat);
        let mat_data = wgsl_code_gen::material_params_to_bytes(&first_mat);
        gfx_cmds.set_constant_values(&pipeline, HgiShaderStage::FRAGMENT, 2, &mat_data);
    }

    if let DrawProgramKey::Mesh(mesh_key) = &frame_key {
        if mesh_key.has_uv {
            let empty_handles = MaterialTextureHandles::new();
            let tex_handles = draw_items
                .first()
                .map(|item| item.get_texture_handles())
                .unwrap_or(empty_handles);
            gfx_cmds.bind_texture_group(3, &tex_handles.textures, &tex_handles.samplers);
        }

        if mesh_key.has_ibl {
            if let Some(ibl) = state.get_ibl_handles() {
                let ibl_group = slots::ibl_group(mesh_key.has_uv);
                gfx_cmds.bind_texture_group(
                    ibl_group,
                    &[
                        ibl.irradiance_tex.clone(),
                        ibl.prefilter_tex.clone(),
                        ibl.brdf_lut_tex.clone(),
                    ],
                    &[
                        ibl.irradiance_smp.clone(),
                        ibl.prefilter_smp.clone(),
                        ibl.brdf_lut_smp.clone(),
                    ],
                );
            }
        }

        if mesh_key.use_shadows {
            if let (Some(atlas), Some(smp)) = (state.get_shadow_atlas(), state.get_shadow_sampler())
            {
                let sg = slots::shadow_group(mesh_key.has_uv, mesh_key.has_ibl);
                gfx_cmds.bind_texture_group(sg, &[atlas.clone()], &[smp.clone()]);
            }
        }
    }

    if frame_key.pick_buffer_rw() {
        if let Some(pick_buffer) = state.get_pick_buffer() {
            gfx_cmds.bind_storage_buffer(
                slots::PICK_BUFFER_GROUP,
                slots::PICK_BUFFER_BINDING,
                pick_buffer,
            );
        }
    }

    Some(FrameSetup {
        pipeline,
        frame_key,
        vp,
        ambient_color,
        cam_pos,
    })
}

/// Bind packed vertex buffer slots (positions / normals / uvs) for one draw item.
///
/// The BAR packs all three sequentially into one GPU buffer.
/// Returns `(vbuf_handle, ibuf_handle)` for the subsequent draw call,
/// or `None` if either buffer is missing.
fn bind_packed_vertex_buffers(
    gfx_cmds: &mut dyn HgiGraphicsCmds,
    item: &HdStDrawItemSharedPtr,
    program_key: &DrawProgramKey,
    item_idx: usize,
    batch_label: &str,
) -> Option<(HgiBufferHandle, HgiBufferHandle)> {
    match program_key {
        DrawProgramKey::Mesh(shader_key) => bind_mesh_vertex_buffers(
            gfx_cmds, item, shader_key, item_idx, batch_label,
        ),
        DrawProgramKey::Points(points_key) => bind_points_vertex_buffers(
            gfx_cmds, item, points_key, item_idx, batch_label,
        ),
        DrawProgramKey::BasisCurves(curves_key) => bind_basis_curves_vertex_buffers(
            gfx_cmds, item, curves_key, item_idx, batch_label,
        ),
    }
}

fn bind_mesh_vertex_buffers(
    gfx_cmds: &mut dyn HgiGraphicsCmds,
    item: &HdStDrawItemSharedPtr,
    shader_key: &MeshShaderKey,
    item_idx: usize,
    batch_label: &str,
) -> Option<(HgiBufferHandle, HgiBufferHandle)> {
    let vbar_ref = item.get_vertex_bar();
    let (vbuf_handle, vbuf_size, vbuf_pool_offset) = extract_buffer(vbar_ref.clone())?;
    let (ibuf_handle, _, _) = extract_buffer(item.get_element_bar())?;

    let st_bar = vbar_ref
        .as_ref()
        .and_then(|b| b.as_any().downcast_ref::<HdStBufferArrayRange>());

    let positions_size = st_bar
        .map(|b| b.get_positions_byte_size())
        .filter(|&s| s > 0)
        .unwrap_or_else(|| {
            let vc = st_bar
                .map(|b| {
                    let pb = b.get_positions_byte_size();
                    if pb > 0 {
                        pb / 12
                    } else {
                        0
                    }
                })
                .unwrap_or(0);
            if vc > 0 {
                vc * 3 * std::mem::size_of::<f32>()
            } else {
                vbuf_size / 3
            }
        });

    let normals_size_in_bar = st_bar.map(|b| b.get_normals_byte_size()).unwrap_or(0);
    let normals_offset = (positions_size + 3) & !3; // 4-byte aligned
    let normals_estimated_size = if normals_size_in_bar > 0 {
        normals_size_in_bar
    } else {
        positions_size
    };
    let uvs_offset = if normals_size_in_bar > 0 {
        (normals_offset + normals_size_in_bar + 3) & !3
    } else {
        (normals_offset + normals_estimated_size + 3) & !3
    };
    let has_uv_data = st_bar.map(|b| b.get_uvs_byte_size() > 0).unwrap_or(false);
    let has_color_data = st_bar
        .map(|b| b.get_colors_byte_size() > 0)
        .unwrap_or(false);
    let uvs_size = st_bar.map(|b| b.get_uvs_byte_size()).unwrap_or(0);
    let colors_offset = if has_uv_data {
        (uvs_offset + uvs_size + 3) & !3
    } else {
        uvs_offset
    };

    log::trace!(
        "DrawBatch::{}[{}] vbuf_size={} pos_size={} nrm_off={} uv_off={} col_off={} has_uv={} has_col={}",
        batch_label,
        item_idx,
        vbuf_size,
        positions_size,
        normals_offset,
        uvs_offset,
        colors_offset,
        has_uv_data,
        has_color_data
    );
    if normals_offset >= vbuf_size {
        log::warn!(
            "DrawBatch::{}[{}] normals_offset {} >= vbuf_size {} — OOB normals!",
            batch_label,
            item_idx,
            normals_offset,
            vbuf_size
        );
    }

    let vbase = vbuf_pool_offset as u64;
    // Build buffer list matching pipeline vertex layout:
    // slot 0: positions, slot 1: normals, slot 2: uvs, slot 3: colors
    let mut bufs = vec![vbuf_handle.clone()];
    let mut offsets = vec![vbase];
    if shader_key.has_normals && !shader_key.has_fvar_normals {
        bufs.push(vbuf_handle.clone());
        offsets.push(vbase + normals_offset as u64);
    }
    if shader_key.has_uv && !shader_key.has_fvar_uv && has_uv_data {
        bufs.push(vbuf_handle.clone());
        offsets.push(vbase + uvs_offset as u64);
    }
    if shader_key.has_color && !shader_key.has_fvar_color {
        if has_color_data {
            bufs.push(vbuf_handle.clone());
            offsets.push(vbase + colors_offset as u64);
        } else {
            // Pipeline expects color slot but mesh has no color data.
            // Bind positions buffer as dummy to satisfy wgpu vertex layout.
            bufs.push(vbuf_handle.clone());
            offsets.push(vbase);
        }
    }
    gfx_cmds.bind_vertex_buffers(&bufs, &offsets);

    Some((vbuf_handle, ibuf_handle))
}

fn bind_points_vertex_buffers(
    gfx_cmds: &mut dyn HgiGraphicsCmds,
    item: &HdStDrawItemSharedPtr,
    shader_key: &PointsProgramKey,
    item_idx: usize,
    batch_label: &str,
) -> Option<(HgiBufferHandle, HgiBufferHandle)> {
    let vbar_ref = item.get_vertex_bar();
    let (vbuf_handle, _, vbuf_pool_offset) = extract_buffer(vbar_ref.clone())?;
    let (ibuf_handle, _, _) = extract_buffer(item.get_element_bar())?;
    let st_bar = vbar_ref
        .as_ref()
        .and_then(|b| b.as_any().downcast_ref::<HdStBufferArrayRange>());
    let positions_size = st_bar.map(|b| b.get_positions_byte_size()).unwrap_or(0);
    let widths_size = st_bar.map(|b| b.get_normals_byte_size()).unwrap_or(0);
    let widths_offset = (positions_size + 3) & !3;
    let colors_offset = (widths_offset + widths_size + 3) & !3;
    let vbase = vbuf_pool_offset as u64;
    let mut bufs = vec![vbuf_handle.clone()];
    let mut offsets = vec![vbase];
    if shader_key.has_widths {
        bufs.push(vbuf_handle.clone());
        offsets.push(vbase + widths_offset as u64);
    }
    if shader_key.has_color {
        bufs.push(vbuf_handle.clone());
        offsets.push(vbase + colors_offset as u64);
    }
    log::trace!(
        "DrawBatch::{}[{}] points pos_size={} width_off={} color_off={}",
        batch_label,
        item_idx,
        positions_size,
        widths_offset,
        colors_offset
    );
    gfx_cmds.bind_vertex_buffers(&bufs, &offsets);
    Some((vbuf_handle, ibuf_handle))
}

fn bind_basis_curves_vertex_buffers(
    gfx_cmds: &mut dyn HgiGraphicsCmds,
    item: &HdStDrawItemSharedPtr,
    shader_key: &BasisCurvesProgramKey,
    item_idx: usize,
    batch_label: &str,
) -> Option<(HgiBufferHandle, HgiBufferHandle)> {
    let vbar_ref = item.get_vertex_bar();
    let (vbuf_handle, _, vbuf_pool_offset) = extract_buffer(vbar_ref.clone())?;
    let (ibuf_handle, _, _) = extract_buffer(item.get_element_bar())?;
    let st_bar = vbar_ref
        .as_ref()
        .and_then(|b| b.as_any().downcast_ref::<HdStBufferArrayRange>());
    let positions_size = st_bar.map(|b| b.get_positions_byte_size()).unwrap_or(0);
    let widths_size = st_bar.map(|b| b.get_normals_byte_size()).unwrap_or(0);
    let normals_size = st_bar.map(|b| b.get_uvs_byte_size()).unwrap_or(0);
    let widths_offset = (positions_size + 3) & !3;
    let normals_offset = (widths_offset + widths_size + 3) & !3;
    let colors_offset = (normals_offset + normals_size + 3) & !3;
    let vbase = vbuf_pool_offset as u64;
    let mut bufs = vec![vbuf_handle.clone()];
    let mut offsets = vec![vbase];
    if shader_key.has_widths {
        bufs.push(vbuf_handle.clone());
        offsets.push(vbase + widths_offset as u64);
    }
    if shader_key.has_normals {
        bufs.push(vbuf_handle.clone());
        offsets.push(vbase + normals_offset as u64);
    }
    if shader_key.has_color {
        bufs.push(vbuf_handle.clone());
        offsets.push(vbase + colors_offset as u64);
    }
    log::trace!(
        "DrawBatch::{}[{}] curves pos_size={} width_off={} normal_off={} color_off={}",
        batch_label,
        item_idx,
        positions_size,
        widths_offset,
        normals_offset,
        colors_offset
    );
    gfx_cmds.bind_vertex_buffers(&bufs, &offsets);
    Some((vbuf_handle, ibuf_handle))
}

// --- Pipeline cache and helper functions for HGI path ---

/// Render pass state overrides that affect pipeline creation.
/// These are XOR'd into the cache hash so different blend/cull combos get different pipelines.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
struct PipelineStateOverrides {
    /// Blend enabled for color attachments
    blend_enabled: bool,
    /// Color blend op
    color_blend_op: usd_hgi::HgiBlendOp,
    /// Source color blend factor
    src_color_blend_factor: usd_hgi::HgiBlendFactor,
    /// Destination color blend factor
    dst_color_blend_factor: usd_hgi::HgiBlendFactor,
    /// Alpha blend op
    alpha_blend_op: usd_hgi::HgiBlendOp,
    /// Source alpha blend factor
    src_alpha_blend_factor: usd_hgi::HgiBlendFactor,
    /// Destination alpha blend factor
    dst_alpha_blend_factor: usd_hgi::HgiBlendFactor,
    /// Cull mode
    cull_mode: usd_hgi::HgiCullMode,
    /// Depth write enabled
    depth_write_enabled: bool,
    /// Active color attachment format.
    color_format: usd_hgi::HgiFormat,
    /// Active depth attachment format.
    depth_format: usd_hgi::HgiFormat,
}

impl Default for PipelineStateOverrides {
    fn default() -> Self {
        Self {
            blend_enabled: false,
            color_blend_op: usd_hgi::HgiBlendOp::Add,
            src_color_blend_factor: usd_hgi::HgiBlendFactor::One,
            dst_color_blend_factor: usd_hgi::HgiBlendFactor::Zero,
            alpha_blend_op: usd_hgi::HgiBlendOp::Add,
            src_alpha_blend_factor: usd_hgi::HgiBlendFactor::One,
            dst_alpha_blend_factor: usd_hgi::HgiBlendFactor::Zero,
            cull_mode: usd_hgi::HgiCullMode::Back,
            depth_write_enabled: true,
            color_format: usd_hgi::HgiFormat::UNorm8Vec4,
            depth_format: usd_hgi::HgiFormat::Float32,
        }
    }
}

impl PipelineStateOverrides {
    /// Build from render pass state.
    fn from_state(state: &HdStRenderPassState) -> Self {
        Self {
            blend_enabled: state.is_blend_enabled(),
            color_blend_op: state.get_blend_color_op(),
            src_color_blend_factor: state.get_blend_color_src_factor(),
            dst_color_blend_factor: state.get_blend_color_dst_factor(),
            alpha_blend_op: state.get_blend_alpha_op(),
            src_alpha_blend_factor: state.get_blend_alpha_src_factor(),
            dst_alpha_blend_factor: state.get_blend_alpha_dst_factor(),
            cull_mode: state.get_cull_mode(),
            depth_write_enabled: state.is_depth_write_enabled(),
            color_format: state.get_color_attachment_format(),
            depth_format: state.get_depth_attachment_format(),
        }
    }

    /// Compute a hash for cache key contribution.
    fn cache_hash(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }
}

/// Pipeline cache entry: pipeline handle + associated shader key.
struct PipelineCacheEntry {
    pipeline: HgiGraphicsPipelineHandle,
    _program: HgiShaderProgramHandle,
}

/// Pipeline cache keyed by (device_id, shader_hash) to partition by GPU device.
static PIPELINE_CACHE: OnceLock<Mutex<HashMap<(u64, u64), PipelineCacheEntry>>> = OnceLock::new();

/// Clear all pipeline cache entries. Backward-compat blanket clear.
pub fn clear_pipeline_cache() {
    if let Some(cache) = PIPELINE_CACHE.get() {
        if let Ok(mut map) = cache.lock() {
            map.clear();
        }
    }
}

/// Clear pipeline cache entries for a specific device.
/// Called when destroying/recreating a wgpu device to avoid BGL epoch panics.
pub fn clear_pipeline_cache_for_device(device_id: u64) {
    if let Some(cache) = PIPELINE_CACHE.get() {
        if let Ok(mut map) = cache.lock() {
            map.retain(|&(dev, _), _| dev != device_id);
        }
    }
}

/// Create or retrieve a cached graphics pipeline for the given shader key.
fn get_or_create_pipeline(
    hgi: &mut dyn Hgi,
    key: &DrawProgramKey,
    sample_item: Option<&HdStDrawItem>,
    overrides: &PipelineStateOverrides,
) -> Option<HgiGraphicsPipelineHandle> {
    let cache = PIPELINE_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = cache.lock().ok()?;

    let device_id = hgi.device_identity();
    let binder = ResourceBinder::from_program_key(key, sample_item);
    let item_hash = sample_item
        .map(HdStDrawItem::compute_fvar_topology_source_hash)
        .unwrap_or(0);
    let hash = key.cache_hash() ^ overrides.cache_hash() ^ binder.cache_hash() ^ item_hash;
    let cache_key = (device_id, hash);
    if let Some(entry) = map.get(&cache_key) {
        if entry.pipeline.is_valid() {
            return Some(entry.pipeline.clone());
        }
    }

    // Generate WGSL from shader key
    let wgsl = gen_wgsl_for_program(key);
    log::debug!("[draw_batch] generating WGSL for key {:?}", key);

    // Compile shader program
    let program = compile_program(hgi, &wgsl.source, wgsl.vs_entry, wgsl.fs_entry)?;

    // Build vertex layout from resource binder
    let vertex_buffers = binder.build_vertex_descs();
    // Pipeline descriptor -- must have at least one color attachment for wgpu
    let color_attachment = usd_hgi::HgiAttachmentDesc {
        format: overrides.color_format,
        load_op: usd_hgi::HgiAttachmentLoadOp::Clear,
        store_op: usd_hgi::HgiAttachmentStoreOp::Store,
        ..Default::default()
    };

    // Depth attachment -- Float32 maps to Depth32Float in wgpu backend
    let depth_attachment = usd_hgi::HgiAttachmentDesc {
        format: overrides.depth_format,
        load_op: usd_hgi::HgiAttachmentLoadOp::Clear,
        store_op: usd_hgi::HgiAttachmentStoreOp::Store,
        ..Default::default()
    };

    // Depth-only prepass (HiddenSurfaceWireframe): disable color writes via ColorMask,
    // matching C++ glColorMask(false,false,false,false). Depth is still written.
    // Otherwise, apply blend state from render pass state (port of C++ _InitAttachmentDesc).
    let color_blend_states = if is_depth_only_program(key) {
        vec![usd_hgi::HgiColorBlendState {
            color_mask: usd_hgi::HgiColorMask::empty(),
            ..Default::default()
        }]
    } else if overrides.blend_enabled {
        vec![usd_hgi::HgiColorBlendState {
            blend_enabled: true,
            color_blend_op: overrides.color_blend_op,
            src_color_blend_factor: overrides.src_color_blend_factor,
            dst_color_blend_factor: overrides.dst_color_blend_factor,
            alpha_blend_op: overrides.alpha_blend_op,
            src_alpha_blend_factor: overrides.src_alpha_blend_factor,
            dst_alpha_blend_factor: overrides.dst_alpha_blend_factor,
            ..Default::default()
        }]
    } else {
        Vec::new()
    };

    let pipeline_desc = HgiGraphicsPipelineDesc {
        debug_name: format!("StormPipeline_{}", key.debug_label()),
        shader_program: program.clone(),
        vertex_buffers,
        primitive_type: key.hgi_primitive_type(),
        color_attachments: vec![color_attachment],
        depth_attachment: Some(depth_attachment),
        color_blend_states,
        rasterization_state: usd_hgi::HgiRasterizationState {
            cull_mode: overrides.cull_mode,
            ..Default::default()
        },
        depth_stencil_state: usd_hgi::HgiDepthStencilState {
            depth_test_enabled: true,
            depth_write_enabled: overrides.depth_write_enabled,
            depth_compare_function: usd_hgi::HgiCompareFunction::Less,
            ..Default::default()
        },
        ..Default::default()
    };

    let handle = hgi.create_graphics_pipeline(&pipeline_desc);
    if !handle.is_valid() {
        log::warn!("get_or_create_pipeline: HGI returned invalid pipeline");
        return None;
    }

    map.insert(
        cache_key,
        PipelineCacheEntry {
            pipeline: handle.clone(),
            _program: program,
        },
    );
    Some(handle)
}

/// Create or retrieve a cached pipeline, applying polygon raster mode (wireframe/points).
///
/// Wraps `get_or_create_pipeline` and overrides `polygon_mode` in the rasterization
/// state. Cache key includes polygon mode to avoid conflicts.
fn get_or_create_pipeline_with_polygon_mode(
    hgi: &mut dyn Hgi,
    key: &DrawProgramKey,
    sample_item: Option<&HdStDrawItem>,
    polygon_mode: HdStPolygonRasterMode,
    overrides: &PipelineStateOverrides,
) -> Option<HgiGraphicsPipelineHandle> {
    // For Fill mode, delegate to standard path (most common, avoids extra hash bits).
    if polygon_mode == HdStPolygonRasterMode::Fill {
        return get_or_create_pipeline(hgi, key, sample_item, overrides);
    }

    // Build a modified cache key that includes polygon mode.
    // We XOR a mode discriminant into the hash to keep wireframe/point pipelines separate.
    let mode_bits: u64 = match polygon_mode {
        HdStPolygonRasterMode::Line => 0xA1A1_0000_0000_0001,
        HdStPolygonRasterMode::Point => 0xB2B2_0000_0000_0002,
        HdStPolygonRasterMode::Fill => unreachable!(),
    };
    let binder = ResourceBinder::from_program_key(key, sample_item);
    let item_hash = sample_item
        .map(HdStDrawItem::compute_fvar_topology_source_hash)
        .unwrap_or(0);
    let hash =
        key.cache_hash() ^ mode_bits ^ overrides.cache_hash() ^ binder.cache_hash() ^ item_hash;

    let cache = PIPELINE_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = cache.lock().ok()?;

    let device_id = hgi.device_identity();
    let cache_key = (device_id, hash);
    if let Some(entry) = map.get(&cache_key) {
        if entry.pipeline.is_valid() {
            return Some(entry.pipeline.clone());
        }
    }

    let wgsl = gen_wgsl_for_program(key);
    log::debug!(
        "[draw_batch] generating WGSL for key {:?} polygon_mode={:?}",
        key,
        polygon_mode
    );

    let program = compile_program(hgi, &wgsl.source, wgsl.vs_entry, wgsl.fs_entry)?;
    let vertex_buffers = binder.build_vertex_descs();

    let color_attachment = usd_hgi::HgiAttachmentDesc {
        format: overrides.color_format,
        load_op: usd_hgi::HgiAttachmentLoadOp::Clear,
        store_op: usd_hgi::HgiAttachmentStoreOp::Store,
        ..Default::default()
    };
    let depth_attachment = usd_hgi::HgiAttachmentDesc {
        format: overrides.depth_format,
        load_op: usd_hgi::HgiAttachmentLoadOp::Clear,
        store_op: usd_hgi::HgiAttachmentStoreOp::Store,
        ..Default::default()
    };

    let hgi_polygon_mode = match polygon_mode {
        HdStPolygonRasterMode::Line => usd_hgi::HgiPolygonMode::Line,
        HdStPolygonRasterMode::Point => usd_hgi::HgiPolygonMode::Point,
        HdStPolygonRasterMode::Fill => usd_hgi::HgiPolygonMode::Fill,
    };

    // Depth-only: disable color writes (C++ glColorMask equivalent).
    // Otherwise, apply blend state from render pass state.
    let color_blend_states = if is_depth_only_program(key) {
        vec![usd_hgi::HgiColorBlendState {
            color_mask: usd_hgi::HgiColorMask::empty(),
            ..Default::default()
        }]
    } else if overrides.blend_enabled {
        vec![usd_hgi::HgiColorBlendState {
            blend_enabled: true,
            color_blend_op: overrides.color_blend_op,
            src_color_blend_factor: overrides.src_color_blend_factor,
            dst_color_blend_factor: overrides.dst_color_blend_factor,
            alpha_blend_op: overrides.alpha_blend_op,
            src_alpha_blend_factor: overrides.src_alpha_blend_factor,
            dst_alpha_blend_factor: overrides.dst_alpha_blend_factor,
            ..Default::default()
        }]
    } else {
        Vec::new()
    };

    let pipeline_desc = HgiGraphicsPipelineDesc {
        debug_name: format!("StormPipeline_{}_{:?}", key.debug_label(), polygon_mode),
        shader_program: program.clone(),
        vertex_buffers,
        primitive_type: key.hgi_primitive_type(),
        color_attachments: vec![color_attachment],
        depth_attachment: Some(depth_attachment),
        color_blend_states,
        rasterization_state: usd_hgi::HgiRasterizationState {
            polygon_mode: hgi_polygon_mode,
            cull_mode: overrides.cull_mode,
            ..Default::default()
        },
        depth_stencil_state: usd_hgi::HgiDepthStencilState {
            depth_test_enabled: true,
            depth_write_enabled: overrides.depth_write_enabled,
            depth_compare_function: usd_hgi::HgiCompareFunction::Less,
            ..Default::default()
        },
        ..Default::default()
    };

    let handle = hgi.create_graphics_pipeline(&pipeline_desc);
    if !handle.is_valid() {
        log::warn!("get_or_create_pipeline_with_polygon_mode: HGI returned invalid pipeline");
        return None;
    }

    map.insert(
        cache_key,
        PipelineCacheEntry {
            pipeline: handle.clone(),
            _program: program,
        },
    );
    Some(handle)
}

fn is_depth_only_program(key: &DrawProgramKey) -> bool {
    match key {
        DrawProgramKey::Mesh(key) => key.depth_only,
        DrawProgramKey::Points(key) => key.depth_only,
        DrawProgramKey::BasisCurves(key) => key.depth_only,
    }
}

fn gen_wgsl_for_program(key: &DrawProgramKey) -> wgsl_code_gen::WgslShaderCode {
    match key {
        DrawProgramKey::Mesh(key) => wgsl_code_gen::gen_mesh_shader(key),
        DrawProgramKey::Points(key) => wgsl_code_gen::gen_points_shader(key),
        DrawProgramKey::BasisCurves(key) => wgsl_code_gen::gen_basis_curves_shader(key),
    }
}

/// Compile vertex + fragment shader into an HGI shader program.
fn compile_program(
    hgi: &mut dyn Hgi,
    wgsl_src: &str,
    vs_entry: &str,
    fs_entry: &str,
) -> Option<HgiShaderProgramHandle> {
    let vs_desc = HgiShaderFunctionDesc::new()
        .with_debug_name("StormVS")
        .with_shader_stage(HgiShaderStage::VERTEX)
        .with_shader_code(wgsl_src)
        .with_entry_point(vs_entry);

    let vs_handle = hgi.create_shader_function(&vs_desc);
    // Check both handle validity AND shader compilation success via trait
    let vs_ok = vs_handle.get().map_or(false, |f| f.is_valid());
    if !vs_ok {
        let errs = vs_handle.get().map_or("", |f| f.compile_errors());
        log::error!("compile_program: vertex shader failed: {}", errs);
        return None;
    }

    let fs_desc = HgiShaderFunctionDesc::new()
        .with_debug_name("StormFS")
        .with_shader_stage(HgiShaderStage::FRAGMENT)
        .with_shader_code(wgsl_src)
        .with_entry_point(fs_entry);

    let fs_handle = hgi.create_shader_function(&fs_desc);
    let fs_ok = fs_handle.get().map_or(false, |f| f.is_valid());
    if !fs_ok {
        let errs = fs_handle.get().map_or("", |f| f.compile_errors());
        log::error!("compile_program: fragment shader failed: {}", errs);
        hgi.destroy_shader_function(&vs_handle);
        return None;
    }

    let prog_desc = HgiShaderProgramDesc::new()
        .with_debug_name("StormProgram")
        .with_shader_function(vs_handle)
        .with_shader_function(fs_handle);

    let prog_handle = hgi.create_shader_program(&prog_desc);
    if !prog_handle.is_valid() {
        log::warn!("compile_program: shader program link failed");
        return None;
    }

    Some(prog_handle)
}

/// Extract 6 frustum planes from a view-projection (cull) matrix.
///
/// Port of the plane extraction used by C++ frustum culling in HdSt.
/// Each plane is [a, b, c, d] such that `a*x + b*y + c*z + d > 0` means inside.
/// Planes: left, right, bottom, top, near, far (Griebel / Hartmann method).
/// Uses f32 for compatibility with `HdStDrawItem::intersects_view_volume`.
fn frustum_planes_from_matrix(m: &usd_gf::Matrix4d) -> [[f32; 4]; 6] {
    // Row-major indexing: m[row][col]. Row-vector convention means clip = p * MVP.
    // Extract rows via Index trait (data is private, use m[row][col]).
    let r0 = [m[0][0], m[0][1], m[0][2], m[0][3]];
    let r1 = [m[1][0], m[1][1], m[1][2], m[1][3]];
    let r2 = [m[2][0], m[2][1], m[2][2], m[2][3]];
    let r3 = [m[3][0], m[3][1], m[3][2], m[3][3]];
    // For row-vector convention: clip.x = dot(p, col0), etc. BUT for plane
    // extraction via Griebel's method with row-vector matrices the planes are:
    //   left:   row3 + row0   (clip.w + clip.x >= 0)
    //   right:  row3 - row0
    //   bottom: row3 + row1
    //   top:    row3 - row1
    //   near:   row3        (wgpu [0,1] NDC - clip.z >= 0)
    //   far:    row3 - row2 (clip.z <= clip.w)
    // Note: GL uses row3+row2 for near, wgpu uses row3 only (depth range [0,1]).
    let planes_f64: [[f64; 4]; 6] = [
        [r3[0] + r0[0], r3[1] + r0[1], r3[2] + r0[2], r3[3] + r0[3]], // left
        [r3[0] - r0[0], r3[1] - r0[1], r3[2] - r0[2], r3[3] - r0[3]], // right
        [r3[0] + r1[0], r3[1] + r1[1], r3[2] + r1[2], r3[3] + r1[3]], // bottom
        [r3[0] - r1[0], r3[1] - r1[1], r3[2] - r1[2], r3[3] - r1[3]], // top
        [r3[0], r3[1], r3[2], r3[3]], // near: row3 only (wgpu [0,1] NDC)
        [r3[0] - r2[0], r3[1] - r2[1], r3[2] - r2[2], r3[3] - r2[3]], // far: row3 - row2
    ];
    planes_f64.map(|p| [p[0] as f32, p[1] as f32, p[2] as f32, p[3] as f32])
}

/// C++ `IsAggregatedWith` checks whether two buffer array ranges belong to
/// the same buffer array (same pool allocation). We approximate this by
/// comparing the GPU buffer handle IDs. Both-None counts as shared (no buffer).
fn bars_share_buffer(
    a: Option<crate::draw_item::HdBufferArrayRangeSharedPtr>,
    b: Option<crate::draw_item::HdBufferArrayRangeSharedPtr>,
) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(_), None) | (None, Some(_)) => false,
        (Some(a_bar), Some(b_bar)) => {
            let a_st = a_bar.as_any().downcast_ref::<HdStBufferArrayRange>();
            let b_st = b_bar.as_any().downcast_ref::<HdStBufferArrayRange>();
            match (a_st, b_st) {
                (Some(a_r), Some(b_r)) => {
                    // Same backing buffer = same pool
                    let a_buf = a_r.get_buffer();
                    let b_buf = b_r.get_buffer();
                    match (a_buf, b_buf) {
                        (Some(ab), Some(bb)) => ab.get_handle().id() == bb.get_handle().id(),
                        (None, None) => true,
                        _ => false,
                    }
                }
                _ => false,
            }
        }
    }
}

/// Extract HgiBufferHandle + size from a draw item's buffer array range.
///
/// Downcast chain: dyn HdBufferArrayRange -> HdStBufferArrayRange -> HdStBufferResource -> HgiBufferHandle
fn extract_buffer(
    bar: Option<crate::draw_item::HdBufferArrayRangeSharedPtr>,
) -> Option<(HgiBufferHandle, usize, usize)> {
    let bar = bar?;
    let st_bar = bar.as_any().downcast_ref::<HdStBufferArrayRange>()?;
    let buf = st_bar.get_buffer()?;
    let handle = buf.get_handle();
    if !handle.is_valid() {
        return None;
    }
    // Return (handle, size, pool_offset) — offset is the BAR's sub-allocation
    // position within the shared GPU buffer.
    Some((handle.clone(), st_bar.get_size(), st_bar.get_offset()))
}

/// Extract camera world position from the view matrix (inv translation).
fn extract_camera_pos(view: &usd_gf::Matrix4d) -> [f32; 4] {
    // USD/Imath row-vector convention: v' = v * M.
    // view[row][col] — so R occupies rows 0..2, cols 0..2.
    // Translation t is in row 3: view[3][0..2].
    //
    // cam_pos = -t * R^T.  Computing t * R^T column by column:
    //   out[0] = tx*r00 + ty*r10 + tz*r20  (dot t with col 0 of R = row 0 of R^T)
    //   out[1] = tx*r01 + ty*r11 + tz*r21
    //   out[2] = tx*r02 + ty*r12 + tz*r22
    // Previously the code used row indices (r01, r02) instead of column indices,
    // which is wrong for non-orthogonal (e.g. scaled) views. (P1-6)
    let r00 = view[0][0] as f32;
    let r01 = view[0][1] as f32;
    let r02 = view[0][2] as f32;
    let r10 = view[1][0] as f32;
    let r11 = view[1][1] as f32;
    let r12 = view[1][2] as f32;
    let r20 = view[2][0] as f32;
    let r21 = view[2][1] as f32;
    let r22 = view[2][2] as f32;
    let tx = view[3][0] as f32;
    let ty = view[3][1] as f32;
    let tz = view[3][2] as f32;
    [
        -(tx * r00 + ty * r10 + tz * r20),
        -(tx * r01 + ty * r11 + tz * r21),
        -(tx * r02 + ty * r12 + tz * r22),
        1.0,
    ]
}

/// Build SceneUniforms byte buffer (192 bytes).
///
/// Layout matches WGSL SceneUniforms struct:
/// - view_proj: mat4x4<f32> (64 bytes)
/// - model: mat4x4<f32> (64 bytes)
/// - ambient_color: vec4<f32> (16 bytes)
/// - camera_pos: vec4<f32> (16 bytes)
/// - selection_color: vec4<f32> (16 bytes) — a>0 means selected, rgb=tint
/// - fvar_base_words + padding (16 bytes)
fn build_scene_uniforms(
    vp: &usd_gf::Matrix4d,
    model: &[[f64; 4]; 4],
    ambient_color: &[f32; 4],
    camera_pos: &[f32; 4],
    selection_color: &[f32; 4],
    fvar_base_words: u32,
) -> [u8; wgsl_code_gen::SCENE_UNIFORMS_SIZE] {
    let mut data = [0u8; wgsl_code_gen::SCENE_UNIFORMS_SIZE];
    let mut offset = 0usize;

    fn write_u32(data: &mut [u8], offset: &mut usize, value: u32) {
        let bytes = value.to_le_bytes();
        data[*offset..*offset + 4].copy_from_slice(&bytes);
        *offset += 4;
    }

    fn write_f32(data: &mut [u8], offset: &mut usize, value: f32) {
        let bytes = value.to_le_bytes();
        data[*offset..*offset + 4].copy_from_slice(&bytes);
        *offset += 4;
    }

    // view_proj mat4x4 (64 bytes)
    for row in 0..4 {
        for col in 0..4 {
            write_f32(&mut data, &mut offset, vp[row][col] as f32);
        }
    }
    // model mat4x4 (64 bytes)
    for row in model {
        for val in row {
            write_f32(&mut data, &mut offset, *val as f32);
        }
    }
    // ambient_color + camera_pos + selection_color (3 * 16 bytes)
    for vec4 in [ambient_color, camera_pos, selection_color] {
        for val in vec4 {
            write_f32(&mut data, &mut offset, *val);
        }
    }
    write_u32(&mut data, &mut offset, fvar_base_words);
    write_f32(&mut data, &mut offset, 0.0);
    write_f32(&mut data, &mut offset, 0.0);
    write_f32(&mut data, &mut offset, 0.0);

    debug_assert_eq!(offset, wgsl_code_gen::SCENE_UNIFORMS_SIZE);
    log::trace!("DrawBatch::build_scene_uniforms size={}", data.len());
    data
}

#[inline]
fn encode_prim_id_to_color(prim_id: i32) -> [f32; 4] {
    // OpenUSD pick AOV convention: encode signed int32 as little-endian RGBA8.
    // -1 (0xFF,0xFF,0xFF,0xFF) represents "no hit".
    let bytes = prim_id.to_le_bytes();
    [
        bytes[0] as f32 / 255.0,
        bytes[1] as f32 / 255.0,
        bytes[2] as f32 / 255.0,
        bytes[3] as f32 / 255.0,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw_item::{HdStDrawItem, TopologyToPrimvarEntry};
    use usd_sdf::Path as SdfPath;
    use usd_tf::Token;

    #[test]
    fn test_draw_batch_creation() {
        let batch = HdStDrawBatch::new();
        assert!(batch.is_empty());
        assert_eq!(batch.get_item_count(), 0);
    }

    #[test]
    fn test_add_items() {
        let mut batch = HdStDrawBatch::new();

        let path1 = SdfPath::from_string("/item1").unwrap();
        let item1 = Arc::new(HdStDrawItem::new(path1));

        let path2 = SdfPath::from_string("/item2").unwrap();
        let item2 = Arc::new(HdStDrawItem::new(path2));

        batch.add_draw_item(item1);
        batch.add_draw_item(item2);

        assert_eq!(batch.get_item_count(), 2);
        assert!(!batch.is_empty());
    }

    #[test]
    fn test_clear() {
        let mut batch = HdStDrawBatch::new();

        let path = SdfPath::from_string("/item").unwrap();
        let item = Arc::new(HdStDrawItem::new(path));
        batch.add_draw_item(item);

        assert!(!batch.is_empty());

        batch.clear();
        assert!(batch.is_empty());
    }

    #[test]
    fn test_validate_removes_invalid() {
        let mut batch = HdStDrawBatch::new();

        // Items without buffers are invalid
        let path = SdfPath::from_string("/item").unwrap();
        let item = Arc::new(HdStDrawItem::new(path));
        batch.add_draw_item(item);

        assert_eq!(batch.get_item_count(), 1);

        // Validate should remove invalid items
        let valid = batch.validate();
        assert!(!valid); // No valid items
        assert!(batch.is_empty());
    }

    #[test]
    fn test_execute() {
        let batch = HdStDrawBatch::new();
        // Should not panic on empty batch
        batch.execute();
    }

    #[test]
    fn test_pipeline_draw_batch_creation() {
        let path = SdfPath::from_string("/prim").unwrap();
        let item = Arc::new(HdStDrawItem::new(path));
        let batch = PipelineDrawBatch::new(item);

        assert_eq!(batch.item_count(), 1);
        assert!(!batch.is_empty());
    }

    #[test]
    fn test_infer_mesh_shader_key_tracks_fvar_metadata() {
        use crate::buffer_resource::{HdStBufferArrayRange, HdStBufferResource};

        let path = SdfPath::from_string("/prim").unwrap();
        let item = HdStDrawItem::new(path);
        item.set_fvar_topology_to_primvar_vector(vec![TopologyToPrimvarEntry {
            topology: vec![0, 1, 2],
            primvars: vec![
                Token::new("normals"),
                Token::new("st"),
                Token::new("displayColor"),
                Token::new("displayOpacity"),
            ],
        }]);
        item.set_face_varying_bar(Arc::new(HdStBufferArrayRange::new(
            Arc::new(HdStBufferResource::with_size(64)),
            0,
            64,
        )));

        let key = infer_mesh_shader_key_from_draw_item(&item, false);
        assert!(key.has_normals);
        assert!(key.has_uv);
        assert!(key.has_color);
        assert_eq!(key.normal_interp, PrimvarInterp::FaceVarying);
        assert!(key.has_fvar_normals);
        assert!(key.has_fvar_uv);
        assert!(key.has_fvar_color);
        assert!(key.has_fvar_opacity);
    }

    #[test]
    fn test_build_frame_key_flat_shading_drops_vertex_normals() {
        let mut key = MeshShaderKey::default();
        key.has_normals = true;
        key.normal_interp = PrimvarInterp::FaceVarying;

        let mut state = HdStRenderPassState::new();
        state.set_flat_shading(true);
        state.set_enable_scene_materials(false);

        let frame_key = build_frame_key(&DrawProgramKey::Mesh(key), &state, true);
        let DrawProgramKey::Mesh(frame_key) = frame_key else {
            panic!("expected mesh frame key");
        };
        assert!(!frame_key.has_normals);
        assert_eq!(frame_key.normal_interp, PrimvarInterp::FaceVarying);
        assert_eq!(frame_key.shading, ShadingModel::GeomFlat);
    }

    #[test]
    fn test_pipeline_draw_batch_append() {
        let path1 = SdfPath::from_string("/prim1").unwrap();
        let item1 = Arc::new(HdStDrawItem::new(path1));
        let mut batch = PipelineDrawBatch::new(item1);

        let path2 = SdfPath::from_string("/prim2").unwrap();
        let item2 = Arc::new(HdStDrawItem::new(path2));

        // Both items invalid (no buffers) so they are compatible
        // but compile_batch will produce 0 visible items
        let appended = batch.append(item2);
        assert!(appended);
        assert_eq!(batch.item_count(), 2);
    }

    #[test]
    fn test_pipeline_draw_batch_validate_empty() {
        let path = SdfPath::from_string("/prim").unwrap();
        let item = Arc::new(HdStDrawItem::new(path));
        let mut batch = PipelineDrawBatch::new(item);

        // Item is invalid (no buffers), validate removes it
        let result = batch.validate(false);
        assert_eq!(result, ValidationResult::ValidBatch);
        assert!(batch.is_empty());
    }

    #[test]
    fn test_validation_result_variants() {
        assert_ne!(ValidationResult::ValidBatch, ValidationResult::RebuildBatch);
        assert_ne!(
            ValidationResult::RebuildBatch,
            ValidationResult::RebuildAllBatches
        );
    }

    // ---------------------------------------------------------------
    // IndirectDrawBatch + DrawIndexedIndirectCommand tests
    // ---------------------------------------------------------------

    #[test]
    fn test_draw_indexed_indirect_command_layout() {
        // wgpu requires DrawIndexedIndirectArgs to be exactly 20 bytes (5 u32).
        assert_eq!(
            std::mem::size_of::<DrawIndexedIndirectCommand>(),
            20,
            "DrawIndexedIndirectCommand must be 20 bytes for wgpu indirect draw"
        );
        assert_eq!(
            INDIRECT_CMD_NUM_UINTS, 5,
            "indirect command stride must be 5 u32"
        );
    }

    #[test]
    fn test_draw_indexed_indirect_command_default() {
        let cmd = DrawIndexedIndirectCommand::default();
        assert_eq!(cmd.index_count, 0);
        assert_eq!(cmd.instance_count, 0);
        assert_eq!(cmd.first_index, 0);
        assert_eq!(cmd.base_vertex, 0);
        assert_eq!(cmd.first_instance, 0);
    }

    #[test]
    fn test_indirect_draw_batch_creation() {
        let path = SdfPath::from_string("/prim").unwrap();
        let item = Arc::new(HdStDrawItem::new(path));
        let batch = IndirectDrawBatch::new_no_culling(item);

        assert_eq!(batch.item_count(), 1);
        assert!(!batch.is_empty());
        assert!(batch.get_dispatch_buffer().is_none());
        assert!(batch.is_dispatch_buffer_dirty());
    }

    #[test]
    fn test_indirect_draw_batch_append_rejects_invalid() {
        // IndirectDrawBatch.is_aggregated rejects invalid items (no buffers).
        let path1 = SdfPath::from_string("/prim1").unwrap();
        let item1 = Arc::new(HdStDrawItem::new(path1));
        let mut batch = IndirectDrawBatch::new_no_culling(item1);

        let path2 = SdfPath::from_string("/prim2").unwrap();
        let item2 = Arc::new(HdStDrawItem::new(path2));

        // Items without buffers are invalid -> append rejects
        let appended = batch.append(item2);
        assert!(
            !appended,
            "invalid items should be rejected by is_aggregated"
        );
        assert_eq!(batch.item_count(), 1);
    }

    #[test]
    fn test_indirect_draw_batch_compile_builds_indirect_commands() {
        // Items without buffers are invalid -> compile produces 0 visible items
        let path = SdfPath::from_string("/prim").unwrap();
        let item = Arc::new(HdStDrawItem::new(path));
        let mut batch = IndirectDrawBatch::new_no_culling(item);

        // Validate triggers compile
        let result = batch.validate(false);
        assert_eq!(result, ValidationResult::ValidBatch);
        // No valid items -> 0 indirect commands
        assert_eq!(batch.get_indirect_commands().len(), 0);
    }

    // ---------------------------------------------------------------
    // SceneUniforms layout: must match WGSL struct exactly
    // ---------------------------------------------------------------

    #[test]
    fn test_scene_uniforms_size() {
        // build_scene_uniforms must produce exactly SCENE_UNIFORMS_SIZE bytes.
        // If this breaks, push constants / UBO binding will silently corrupt.
        let vp = usd_gf::Matrix4d::identity();
        let model = [
            [1.0f64, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let ambient = [0.1f32, 0.1, 0.1, 1.0];
        let cam_pos = [0.0f32, 0.0, 5.0, 1.0];

        let data = build_scene_uniforms(&vp, &model, &ambient, &cam_pos, &[0.0f32; 4], 0);
        assert_eq!(
            data.len(),
            wgsl_code_gen::SCENE_UNIFORMS_SIZE,
            "SceneUniforms byte layout must match WGSL struct size"
        );
        assert_eq!(
            data.len(),
            192,
            "view_proj(64) + model(64) + ambient(16) + cam(16) + selection(16) + fvar(16)"
        );
    }

    #[test]
    fn test_scene_uniforms_identity_roundtrip() {
        // Identity matrices should produce recognizable float patterns.
        let vp = usd_gf::Matrix4d::identity();
        let model = [
            [1.0f64, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let ambient = [0.0f32; 4];
        let cam_pos = [0.0f32; 4];

        let data = build_scene_uniforms(&vp, &model, &ambient, &cam_pos, &[0.0f32; 4], 0);

        // view_proj[0][0] = 1.0f32 at offset 0
        let val = f32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        assert_eq!(val, 1.0, "identity vp[0][0] must be 1.0");

        // view_proj[1][1] = 1.0f32 at offset 4*(4+1) = 20
        let off = (1 * 4 + 1) * 4; // row 1, col 1
        let val = f32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
        assert_eq!(val, 1.0, "identity vp[1][1] must be 1.0");
    }

    // ---------------------------------------------------------------
    // Vertex buffer offset computation (draw-time safety)
    // ---------------------------------------------------------------

    #[test]
    fn test_normals_offset_within_buffer() {
        // Simulates what execute_draw does: given a vbuf_size and positions_size,
        // normals_offset must be < vbuf_size. If not, wgpu will crash.
        use crate::buffer_resource::{HdStBufferArrayRange, HdStBufferResource};

        let test_cases: Vec<(usize, usize)> = vec![
            (1056, 528),    // 44 verts * 12b (our original bug)
            (72, 36),       // 3 verts triangle
            (240, 120),     // 10 verts
            (24000, 12000), // 1000 verts
        ];

        for (total, pos_size) in test_cases {
            let buf = Arc::new(HdStBufferResource::with_size(total));
            let bar = HdStBufferArrayRange::with_positions_size(buf, 0, total, pos_size);

            let positions_size = bar.get_positions_byte_size();
            let normals_offset = (positions_size + 3) & !3;

            assert!(
                normals_offset < total,
                "normals_offset {} must be < buffer {} (pos_size={})",
                normals_offset,
                total,
                pos_size
            );
            assert!(
                normals_offset >= positions_size,
                "normals_offset must be >= positions_size"
            );
            // Enough room for normals after offset
            assert!(
                total - normals_offset >= pos_size,
                "normals data ({} bytes) must fit after offset {} in buffer {}",
                pos_size,
                normals_offset,
                total
            );
        }
    }

    #[test]
    fn test_fallback_half_split_when_no_positions_size() {
        // When positions_byte_size == 0, draw_batch falls back to vbuf_size / 2.
        // Verify the fallback produces valid offsets.
        use crate::buffer_resource::{HdStBufferArrayRange, HdStBufferResource};

        for total in [72usize, 240, 1056, 24000] {
            let buf = Arc::new(HdStBufferResource::with_size(total));
            let bar = HdStBufferArrayRange::new(buf, 0, total);

            // Simulate draw_batch fallback logic
            let raw = bar.get_positions_byte_size();
            let positions_size = if raw > 0 { raw } else { total / 2 };
            let normals_offset = (positions_size + 3) & !3;

            assert!(
                normals_offset < total,
                "fallback offset {} must be < total {}",
                normals_offset,
                total
            );
            assert_eq!(normals_offset % 4, 0, "offset must be 4-aligned");
        }
    }

    /// Helper: create a valid HdStBufferResource with a real (mock) HGI handle.
    fn make_valid_buffer(total_size: usize) -> Arc<crate::buffer_resource::HdStBufferResource> {
        use usd_hgi::{
            buffer::{HgiBuffer, HgiBufferDesc},
            HgiHandle,
        };

        struct MockBuffer(HgiBufferDesc);
        impl HgiBuffer for MockBuffer {
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
            fn descriptor(&self) -> &HgiBufferDesc {
                &self.0
            }
            fn byte_size_of_resource(&self) -> usize {
                self.0.byte_size
            }
            fn raw_resource(&self) -> u64 {
                0
            }
            fn cpu_staging_address(&mut self) -> Option<*mut u8> {
                None
            }
        }

        let desc = HgiBufferDesc {
            byte_size: total_size,
            ..Default::default()
        };
        let mock: Arc<dyn HgiBuffer> = Arc::new(MockBuffer(desc));
        let handle = HgiHandle::new(mock, 1);

        let mut res = crate::buffer_resource::HdStBufferResource::with_size(0);
        res.set_allocation(handle, total_size);
        Arc::new(res)
    }

    #[test]
    fn test_extract_buffer_returns_pool_offset() {
        // Verify extract_buffer propagates pool sub-allocation offset from BAR.
        use crate::buffer_resource::HdStBufferArrayRange;
        use crate::draw_item::HdBufferArrayRangeSharedPtr;

        let buf = make_valid_buffer(4096);
        let pool_offset = 512usize;
        let bar_size = 256usize;
        let bar: HdBufferArrayRangeSharedPtr =
            Arc::new(HdStBufferArrayRange::new(buf, pool_offset, bar_size));

        let result = extract_buffer(Some(bar.clone()));
        assert!(
            result.is_some(),
            "extract_buffer must succeed for valid BAR"
        );
        let (_handle, size, offset) = result.unwrap();
        assert_eq!(size, bar_size, "size must match BAR size");
        assert_eq!(offset, pool_offset, "offset must match BAR pool offset");
    }

    #[test]
    fn test_extract_buffer_zero_offset_for_raw_buffer() {
        // Raw buffers (no pool sub-alloc) have offset=0.
        use crate::buffer_resource::HdStBufferArrayRange;
        use crate::draw_item::HdBufferArrayRangeSharedPtr;

        let buf = make_valid_buffer(1024);
        let bar: HdBufferArrayRangeSharedPtr = Arc::new(HdStBufferArrayRange::new(buf, 0, 1024));

        let (_handle, size, offset) = extract_buffer(Some(bar.clone())).unwrap();
        assert_eq!(size, 1024);
        assert_eq!(offset, 0, "raw buffer BAR must have zero offset");
    }

    #[test]
    fn test_vertex_binding_offsets_include_pool_base() {
        // Simulate the draw_batch binding logic and verify offsets include pool base.
        use crate::buffer_resource::{HdStBufferArrayRange, HdStBufferResource};

        let pool_offset = 768usize;
        let vbuf_size = 192usize; // 8 verts * (12 pos + 12 nrm) = 192 bytes
        let pos_byte_size = 96usize;

        let buf = Arc::new(HdStBufferResource::with_size(4096));
        let bar =
            HdStBufferArrayRange::with_positions_size(buf, pool_offset, vbuf_size, pos_byte_size);

        // Replicate draw_batch binding logic
        let vbase = bar.get_offset() as u64;
        let positions_size = {
            let raw = bar.get_positions_byte_size();
            if raw > 0 {
                raw
            } else {
                vbuf_size / 2
            }
        };
        let normals_offset = (positions_size + 3) & !3;
        let offsets = [vbase, vbase + normals_offset as u64];

        // Positions must start at pool_offset, not 0
        assert_eq!(
            offsets[0], pool_offset as u64,
            "positions binding must start at pool base offset"
        );
        assert_eq!(
            offsets[1],
            (pool_offset + normals_offset) as u64,
            "normals binding must be pool_base + normals_offset"
        );
        // Both must be within the pool buffer range
        assert!(
            offsets[1] < (pool_offset + vbuf_size) as u64,
            "normals offset must be within BAR range"
        );
    }

    #[test]
    fn test_index_base_offset_from_pool() {
        // Verify index buffer pool offset converts to correct base_index.
        let ibuf_pool_offset = 576usize; // e.g. 144 indices * 4 bytes
        let idx_base_add = (ibuf_pool_offset / std::mem::size_of::<u32>()) as u32;
        assert_eq!(idx_base_add, 144, "576 bytes / 4 = 144 indices");

        // Non-aligned offset (should still work via integer division)
        let odd_offset = 580usize;
        let idx_odd = (odd_offset / std::mem::size_of::<u32>()) as u32;
        assert_eq!(idx_odd, 145, "580 / 4 = 145 (truncated)");
    }

    #[test]
    fn test_multiple_bars_different_offsets() {
        // Simulate multiple meshes sharing one GPU buffer at different offsets.
        // Each mesh must bind at its own offset, not at 0.
        use crate::buffer_resource::HdStBufferArrayRange;
        use crate::draw_item::HdBufferArrayRangeSharedPtr;

        let shared_buf = make_valid_buffer(8192);

        // Mesh 0: offset=0, size=192
        let bar0: HdBufferArrayRangeSharedPtr = Arc::new(
            HdStBufferArrayRange::with_positions_size(shared_buf.clone(), 0, 192, 96),
        );
        // Mesh 1: offset=192, size=384
        let bar1: HdBufferArrayRangeSharedPtr = Arc::new(
            HdStBufferArrayRange::with_positions_size(shared_buf.clone(), 192, 384, 192),
        );
        // Mesh 2: offset=576, size=6000
        let bar2: HdBufferArrayRangeSharedPtr = Arc::new(
            HdStBufferArrayRange::with_positions_size(shared_buf.clone(), 576, 6000, 3000),
        );

        for (i, bar) in [bar0.clone(), bar1.clone(), bar2.clone()].iter().enumerate() {
            let (_, size, offset) = extract_buffer(Some(bar.clone())).unwrap();
            let st_bar = bar.as_any().downcast_ref::<HdStBufferArrayRange>().unwrap();
            let pos_size = st_bar.get_positions_byte_size();
            let normals_off = (pos_size + 3) & !3;
            let vbase = offset as u64;
            let offsets = [vbase, vbase + normals_off as u64];

            // Positions must start at THIS mesh's pool offset
            assert_eq!(
                offsets[0], offset as u64,
                "mesh {i}: positions must bind at pool offset {offset}"
            );
            // Normals must be within THIS mesh's range
            assert!(
                offsets[1] < (offset + size) as u64,
                "mesh {i}: normals offset {} must be < {}",
                offsets[1],
                offset + size
            );
            // No mesh should bind at 0 (except mesh 0)
            if i > 0 {
                assert_ne!(
                    offsets[0], 0,
                    "mesh {i}: must NOT bind at offset 0 (that's mesh 0's data!)"
                );
            }
        }
    }
}
