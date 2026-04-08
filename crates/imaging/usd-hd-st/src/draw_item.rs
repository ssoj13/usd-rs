//! HdStDrawItem - A single drawable item for Storm.
//!
//! DrawItems represent atomic drawable units. They contain buffer ranges,
//! shader bindings, and other state needed to issue a draw call.

use crate::mesh_shader_key::DrawTopology;
use crate::wgsl_code_gen::MaterialParams;
use parking_lot::RwLock;
use std::sync::Arc;
use usd_hgi::sampler::HgiSamplerHandle;
use usd_hgi::texture::HgiTextureHandle;
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

/// High-level rprim family carried by a draw item.
///
/// `_ref` lets each rprim family contribute its own geometric shader and
/// binding contract. The earlier Rust batch path reconstructed everything from
/// topology alone, which let non-mesh prims quietly inherit mesh-only shader
/// assumptions. Keeping the family on the draw item makes the active draw
/// contract explicit at the batching boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DrawPrimitiveKind {
    #[default]
    Mesh,
    Points,
    BasisCurves,
}

/// GPU texture+sampler handles for one material's texture slots.
///
/// Parallel arrays indexed by material texture slot number (see binding::slots::TEXTURE_GROUP).
/// Each entry corresponds to one tex+sampler pair in @group(3):
///   0 = diffuse, 1 = normal, 2 = roughness, 3 = metallic,
///   4 = opacity, 5 = emissive, 6 = occlusion
///
/// Default (empty Vec) means all slots use 1x1 white fallback.
#[derive(Debug, Clone, Default)]
pub struct MaterialTextureHandles {
    /// HGI texture handles, indexed by slot [0..7]
    pub textures: Vec<HgiTextureHandle>,
    /// HGI sampler handles, indexed by slot [0..7]
    pub samplers: Vec<HgiSamplerHandle>,
}

impl MaterialTextureHandles {
    /// Number of texture slots declared in @group(3) (7 used + 1 reserved).
    pub const SLOT_COUNT: usize = 8;

    /// Create empty handle set (all slots use fallback).
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if any actual texture handle is set.
    pub fn has_any(&self) -> bool {
        self.textures.iter().any(|h| h.is_valid())
    }

    /// Set a texture+sampler at a specific slot index.
    pub fn set_slot(&mut self, slot: usize, tex: HgiTextureHandle, smp: HgiSamplerHandle) {
        // Extend vecs to cover the requested slot
        if self.textures.len() <= slot {
            self.textures
                .resize_with(slot + 1, HgiTextureHandle::default);
        }
        if self.samplers.len() <= slot {
            self.samplers
                .resize_with(slot + 1, HgiSamplerHandle::default);
        }
        self.textures[slot] = tex;
        self.samplers[slot] = smp;
    }
}

/// Shared pointer to buffer array range.
pub type HdBufferArrayRangeSharedPtr = Arc<dyn HdBufferArrayRange>;

/// Mapping entry from one face-varying topology channel to the primvars using it.
///
/// Port of one element of OpenUSD's `TopologyToPrimvarVector`, which is stored on
/// shared draw-item data and later consumed by the resource binder when assigning
/// face-varying channels.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TopologyToPrimvarEntry {
    /// Refined or authored face-varying topology indices for this channel.
    pub topology: Vec<i32>,
    /// Primvar names resolved onto this topology channel.
    pub primvars: Vec<Token>,
}

/// Mapping from refined/authored face-varying topologies to named primvars.
///
/// This mirrors `_ref` `TopologyToPrimvarVector` closely enough for the live Rust
/// draw path to carry the same metadata through `HdStDrawItem`.
pub type TopologyToPrimvarVector = Vec<TopologyToPrimvarEntry>;

/// Buffer array range - view into GPU buffer for drawing.
pub trait HdBufferArrayRange: Send + Sync + std::fmt::Debug {
    /// Check if range is valid.
    fn is_valid(&self) -> bool {
        true
    }

    /// For downcasting to concrete types (e.g. HdStBufferArrayRange).
    fn as_any(&self) -> &dyn std::any::Any {
        static PH: () = ();
        &PH
    }
}

/// A single drawable item.
///
/// DrawItems are the atomic unit of drawing in Storm. Each draw item
/// represents a single draw call with associated buffers and state.
///
/// # Architecture
///
/// DrawItems are organized into DrawBatches for efficient rendering.
/// Multiple draw items with compatible state can be batched together.
#[derive(Debug, Clone)]
struct HdStDrawItemState {
    /// Vertex buffer array range
    vertex_bar: Option<HdBufferArrayRangeSharedPtr>,

    /// Element (index) buffer array range
    element_bar: Option<HdBufferArrayRangeSharedPtr>,

    /// Constant (uniform) buffer array range
    constant_bar: Option<HdBufferArrayRangeSharedPtr>,

    /// Instance buffer array range (for instanced drawing)
    instance_bar: Option<HdBufferArrayRangeSharedPtr>,

    /// Instance index buffer array range (C++: instanceIndexBar).
    instance_index_bar: Option<HdBufferArrayRangeSharedPtr>,

    /// Topology (index) buffer array range (C++: topologyBar).
    topology_bar: Option<HdBufferArrayRangeSharedPtr>,

    /// Varying primvar buffer array range (C++: varyingBar).
    varying_bar: Option<HdBufferArrayRangeSharedPtr>,

    /// Face-varying primvar buffer array range (C++: fvarBar).
    face_varying_bar: Option<HdBufferArrayRangeSharedPtr>,

    /// Mapping from face-varying topology channels to authored primvar names.
    ///
    /// Port of `_ref` shared draw-item state `fvarTopologyToPrimvarVector`.
    fvar_topology_to_primvar_vector: TopologyToPrimvarVector,

    /// Topology visibility BAR: per-face visibility masks (C++: topologyVisibilityBar).
    /// Null when no per-face visibility is authored.
    topology_visibility_bar: Option<HdBufferArrayRangeSharedPtr>,

    /// Per-level instance primvar BARs for multi-level instancing (C++: instancePrimvarBars).
    /// Level 0 = outermost instancer, level N-1 = innermost.
    instance_primvar_bars: Vec<HdBufferArrayRangeSharedPtr>,

    /// Geometric shader key for batch aggregation.
    /// Encodes: prim_type + cull_style + polygon_mode as a u64 hash.
    /// Items with different keys cannot be batched. C++: _geometricShader.
    geometric_shader_key: u64,

    /// Primitive topology resolved for this draw item.
    ///
    /// The original Rust port reconstructed primitive topology later from
    /// mesh-centric heuristics inside `draw_batch`, which meant non-mesh rprims
    /// silently inherited triangle-list assumptions. Keeping the resolved
    /// topology on the draw item mirrors the `_ref` "draw item carries the
    /// actual draw contract" model and lets batch/program selection stay honest.
    primitive_topology: DrawTopology,

    /// High-level rprim family that owns this draw item.
    primitive_kind: DrawPrimitiveKind,

    /// Draw item is visible
    visible: bool,

    /// Representation this draw item belongs to (e.g., "refined", "hull")
    repr: Token,

    /// Material tag for render pass filtering (e.g. "defaultMaterialTag", "translucent")
    material_tag: Option<Token>,

    /// Material network shader params for this draw item.
    /// Matches C++ HdStDrawItem::_materialNetworkShader.
    material_params: MaterialParams,

    /// GPU texture+sampler handles for @group(3) texture binding.
    /// Empty = use 1x1 white fallback for all slots.
    texture_handles: MaterialTextureHandles,

    /// Local-space AABB minimum corner ([x, y, z]).
    /// Set by the sync phase; used by GPU frustum culling.
    /// Defaults to [FLT_MAX, FLT_MAX, FLT_MAX] (empty = skip culling).
    bbox_min: [f32; 3],

    /// Local-space AABB maximum corner ([x, y, z]).
    /// Set by the sync phase; used by GPU frustum culling.
    /// Defaults to [-FLT_MAX, -FLT_MAX, -FLT_MAX] (empty = skip culling).
    bbox_max: [f32; 3],

    /// World transform (row-major 4×4) set during rprim sync.
    /// Replaces the viewer-side `model_transforms` HashMap lookup — transforms
    /// flow through Hydra draw items matching C++ `HdDrawItem::GetMatrix()`.
    world_transform: [[f64; 4]; 4],
}

#[derive(Debug)]
pub struct HdStDrawItem {
    /// Path to the prim this draw item belongs to. Identity is immutable.
    prim_path: SdfPath,

    /// Shared mutable draw-item payload. This matches Hydra's ownership model
    /// more closely: repr/render-pass users can retain the same draw-item object
    /// while rprim sync mutates its bindings in place.
    state: RwLock<HdStDrawItemState>,
}

impl HdStDrawItem {
    /// Create a new draw item.
    pub fn new(prim_path: SdfPath) -> Self {
        Self {
            prim_path,
            state: RwLock::new(HdStDrawItemState {
                vertex_bar: None,
                element_bar: None,
                constant_bar: None,
                instance_bar: None,
                instance_index_bar: None,
                topology_bar: None,
                varying_bar: None,
                face_varying_bar: None,
                fvar_topology_to_primvar_vector: TopologyToPrimvarVector::new(),
                topology_visibility_bar: None,
                instance_primvar_bars: Vec::new(),
                geometric_shader_key: 0,
                primitive_topology: DrawTopology::TriangleList,
                primitive_kind: DrawPrimitiveKind::Mesh,
                visible: true,
                repr: Token::new("refined"),
                material_tag: None,
                material_params: MaterialParams::default(),
                texture_handles: MaterialTextureHandles::new(),
                bbox_min: [f32::MAX, f32::MAX, f32::MAX],
                bbox_max: [f32::MIN, f32::MIN, f32::MIN],
                world_transform: [
                    [1.0, 0.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0, 0.0],
                    [0.0, 0.0, 1.0, 0.0],
                    [0.0, 0.0, 0.0, 1.0],
                ],
            }),
        }
    }

    /// Get the prim path.
    pub fn get_prim_path(&self) -> &SdfPath {
        &self.prim_path
    }

    /// Set vertex buffer array range.
    pub fn set_vertex_bar(&self, bar: HdBufferArrayRangeSharedPtr) {
        self.state.write().vertex_bar = Some(bar);
    }

    /// Get vertex buffer array range.
    pub fn get_vertex_bar(&self) -> Option<HdBufferArrayRangeSharedPtr> {
        self.state.read().vertex_bar.clone()
    }

    /// Set element (index) buffer array range.
    pub fn set_element_bar(&self, bar: HdBufferArrayRangeSharedPtr) {
        self.state.write().element_bar = Some(bar);
    }

    /// Get element buffer array range.
    pub fn get_element_bar(&self) -> Option<HdBufferArrayRangeSharedPtr> {
        self.state.read().element_bar.clone()
    }

    /// Set constant (uniform) buffer array range.
    pub fn set_constant_bar(&self, bar: HdBufferArrayRangeSharedPtr) {
        self.state.write().constant_bar = Some(bar);
    }

    /// Clear constant (uniform) buffer array range.
    ///
    /// Used when a draw item is retained across syncs and its constant BAR
    /// needs to be rebound or removed in place, matching Hydra's stable
    /// draw-item ownership model.
    pub fn clear_constant_bar(&self) {
        self.state.write().constant_bar = None;
    }

    /// Get constant buffer array range.
    pub fn get_constant_bar(&self) -> Option<HdBufferArrayRangeSharedPtr> {
        self.state.read().constant_bar.clone()
    }

    /// Set instance buffer array range.
    pub fn set_instance_bar(&self, bar: HdBufferArrayRangeSharedPtr) {
        self.state.write().instance_bar = Some(bar);
    }

    /// Get instance buffer array range.
    pub fn get_instance_bar(&self) -> Option<HdBufferArrayRangeSharedPtr> {
        self.state.read().instance_bar.clone()
    }

    /// Set instance index buffer array range.
    pub fn set_instance_index_bar(&self, bar: HdBufferArrayRangeSharedPtr) {
        self.state.write().instance_index_bar = Some(bar);
    }

    /// Get instance index buffer array range.
    pub fn get_instance_index_bar(&self) -> Option<HdBufferArrayRangeSharedPtr> {
        self.state.read().instance_index_bar.clone()
    }

    /// Set topology buffer array range.
    pub fn set_topology_bar(&self, bar: HdBufferArrayRangeSharedPtr) {
        self.state.write().topology_bar = Some(bar);
    }

    /// Get topology buffer array range.
    pub fn get_topology_bar(&self) -> Option<HdBufferArrayRangeSharedPtr> {
        self.state.read().topology_bar.clone()
    }

    /// Set varying primvar buffer array range.
    pub fn set_varying_bar(&self, bar: HdBufferArrayRangeSharedPtr) {
        self.state.write().varying_bar = Some(bar);
    }

    /// Get varying primvar buffer array range.
    pub fn get_varying_bar(&self) -> Option<HdBufferArrayRangeSharedPtr> {
        self.state.read().varying_bar.clone()
    }

    /// Set face-varying primvar buffer array range.
    pub fn set_face_varying_bar(&self, bar: HdBufferArrayRangeSharedPtr) {
        self.state.write().face_varying_bar = Some(bar);
    }

    /// Clear face-varying primvar buffer array range.
    pub fn clear_face_varying_bar(&self) {
        self.state.write().face_varying_bar = None;
    }

    /// Get face-varying primvar buffer array range.
    pub fn get_face_varying_bar(&self) -> Option<HdBufferArrayRangeSharedPtr> {
        self.state.read().face_varying_bar.clone()
    }

    /// Set mapping from face-varying topology channels to primvar names.
    pub fn set_fvar_topology_to_primvar_vector(&self, mapping: TopologyToPrimvarVector) {
        self.state.write().fvar_topology_to_primvar_vector = mapping;
    }

    /// Get mapping from face-varying topology channels to primvar names.
    pub fn get_fvar_topology_to_primvar_vector(&self) -> TopologyToPrimvarVector {
        self.state.read().fvar_topology_to_primvar_vector.clone()
    }

    /// Set topology visibility BAR (per-face visibility masks).
    pub fn set_topology_visibility_bar(&self, bar: HdBufferArrayRangeSharedPtr) {
        self.state.write().topology_visibility_bar = Some(bar);
    }

    /// Get topology visibility BAR. None when no per-face visibility authored.
    pub fn get_topology_visibility_bar(&self) -> Option<HdBufferArrayRangeSharedPtr> {
        self.state.read().topology_visibility_bar.clone()
    }

    /// Set per-level instance primvar BARs for multi-level instancing.
    /// C++: instancePrimvarBars (one per instancing level).
    pub fn set_instance_primvar_bars(&self, bars: Vec<HdBufferArrayRangeSharedPtr>) {
        self.state.write().instance_primvar_bars = bars;
    }

    /// Number of instance primvar levels. C++: GetInstancePrimvarNumLevels.
    pub fn get_instance_primvar_num_levels(&self) -> usize {
        self.state.read().instance_primvar_bars.len()
    }

    /// Get instance primvar BAR at a specific level. Returns None if level out of range.
    /// C++: GetInstancePrimvarRange(i).
    pub fn get_instance_primvar_bar(&self, level: usize) -> Option<HdBufferArrayRangeSharedPtr> {
        self.state.read().instance_primvar_bars.get(level).cloned()
    }

    /// Compute a hash over the item's bound textures for aggregation checks.
    ///
    /// Port of C++ HdSt_MaterialNetworkShader::ComputeTextureSourceHash().
    /// Two items with different texture bindings must NOT be batched together
    /// (they would share the same @group(3) bind group with conflicting handles).
    pub fn compute_texture_source_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let state = self.state.read();
        let mut hasher = DefaultHasher::new();
        // Hash handle IDs — each GPU-created resource has a unique u64 id.
        for tex in &state.texture_handles.textures {
            tex.id().hash(&mut hasher);
        }
        for smp in &state.texture_handles.samplers {
            smp.id().hash(&mut hasher);
        }
        hasher.finish()
    }

    /// Compute a hash over retained face-varying channel metadata.
    ///
    /// This mirrors the `_ref` need for binding/codegen state to vary with
    /// topology-to-primvar mapping, not only with coarse shader-key booleans.
    pub fn compute_fvar_topology_source_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let state = self.state.read();
        let mut hasher = DefaultHasher::new();
        for entry in &state.fvar_topology_to_primvar_vector {
            entry.topology.hash(&mut hasher);
            for primvar in &entry.primvars {
                Hash::hash(primvar, &mut hasher);
            }
        }
        hasher.finish()
    }

    /// Set geometric shader key for batch aggregation.
    /// Encodes prim_type + cull_style + polygon_mode as a u64 hash.
    pub fn set_geometric_shader_key(&self, key: u64) {
        self.state.write().geometric_shader_key = key;
    }

    /// Get geometric shader key for batch aggregation.
    pub fn get_geometric_shader_key(&self) -> u64 {
        self.state.read().geometric_shader_key
    }

    /// Set the primitive topology consumed by the active draw path.
    pub fn set_primitive_topology(&self, topology: DrawTopology) {
        self.state.write().primitive_topology = topology;
    }

    /// Get the primitive topology consumed by the active draw path.
    pub fn get_primitive_topology(&self) -> DrawTopology {
        self.state.read().primitive_topology
    }

    /// Set the rprim family consumed by the active draw path.
    pub fn set_primitive_kind(&self, kind: DrawPrimitiveKind) {
        self.state.write().primitive_kind = kind;
    }

    /// Get the rprim family consumed by the active draw path.
    pub fn get_primitive_kind(&self) -> DrawPrimitiveKind {
        self.state.read().primitive_kind
    }

    /// Set visibility.
    pub fn set_visible(&self, visible: bool) {
        self.state.write().visible = visible;
    }

    /// Check if visible.
    pub fn is_visible(&self) -> bool {
        self.state.read().visible
    }

    /// Check if the draw item has all required buffers.
    pub fn is_valid(&self) -> bool {
        // At minimum, need vertex and element buffers
        let state = self.state.read();
        state.vertex_bar.is_some() && state.element_bar.is_some()
    }

    /// Set the representation token.
    pub fn set_repr(&self, repr: Token) {
        self.state.write().repr = repr;
    }

    /// Get the representation token.
    pub fn get_repr(&self) -> Token {
        self.state.read().repr.clone()
    }

    /// Set material tag for render pass filtering.
    pub fn set_material_tag(&self, tag: Token) {
        self.state.write().material_tag = Some(tag);
    }

    /// Get material tag (None if not assigned from a material).
    pub fn get_material_tag(&self) -> Option<Token> {
        self.state.read().material_tag.clone()
    }

    /// Set material network shader params.
    /// Matches C++ HdStDrawItem::SetMaterialNetworkShader.
    pub fn set_material_network_shader(&self, params: MaterialParams) {
        self.state.write().material_params = params;
    }

    /// Get material network shader params.
    /// Matches C++ HdStDrawItem::GetMaterialNetworkShader.
    pub fn get_material_network_shader(&self) -> MaterialParams {
        self.state.read().material_params.clone()
    }

    /// Set GPU texture+sampler handles for @group(3) texture bind group.
    pub fn set_texture_handles(&self, handles: MaterialTextureHandles) {
        self.state.write().texture_handles = handles;
    }

    /// Get GPU texture+sampler handles for @group(3) texture bind group.
    pub fn get_texture_handles(&self) -> MaterialTextureHandles {
        self.state.read().texture_handles.clone()
    }

    /// Set local-space AABB for this draw item.
    ///
    /// Called during sync from CPU positions; used by GPU frustum culling.
    /// When min > max (default), culling is bypassed for this item (pass-through).
    pub fn set_bbox(&self, min: [f32; 3], max: [f32; 3]) {
        let mut state = self.state.write();
        state.bbox_min = min;
        state.bbox_max = max;
    }

    /// Get local-space AABB minimum corner.
    pub fn get_bbox_min(&self) -> [f32; 3] {
        self.state.read().bbox_min
    }

    /// Get local-space AABB maximum corner.
    pub fn get_bbox_max(&self) -> [f32; 3] {
        self.state.read().bbox_max
    }

    /// Set world transform (row-major 4×4).
    /// Called during rprim sync to propagate transform from Hydra to draw item.
    pub fn set_world_transform(&self, xform: [[f64; 4]; 4]) {
        self.state.write().world_transform = xform;
    }

    /// Get world transform (row-major 4×4).
    /// Matches C++ `HdDrawItem::GetMatrix()` — returns the model-to-world transform.
    pub fn get_world_transform(&self) -> [[f64; 4]; 4] {
        self.state.read().world_transform
    }

    /// Returns true when a valid AABB has been set (min <= max on all axes).
    pub fn has_valid_bbox(&self) -> bool {
        let state = self.state.read();
        state.bbox_min[0] <= state.bbox_max[0]
            && state.bbox_min[1] <= state.bbox_max[1]
            && state.bbox_min[2] <= state.bbox_max[2]
    }

    /// Compute a hash of the buffer array handles for batch validation.
    ///
    /// Port of C++ HdStDrawItem::GetBufferArraysHash (P1-10).
    /// When any BAR is reallocated (pool grew), this hash changes, triggering
    /// a batch rebuild in the command buffer.
    pub fn get_buffer_arrays_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let state = self.state.read();
        let mut hasher = DefaultHasher::new();
        // Hash the Arc pointer addresses of each BAR (stable within a frame).
        // Use the vtable pointer from the fat pointer as a stable identity marker.
        if let Some(ref bar) = state.vertex_bar {
            let ptr = Arc::as_ptr(bar);
            (ptr as *const () as usize).hash(&mut hasher);
        }
        if let Some(ref bar) = state.element_bar {
            let ptr = Arc::as_ptr(bar);
            (ptr as *const () as usize).hash(&mut hasher);
        }
        if let Some(ref bar) = state.constant_bar {
            let ptr = Arc::as_ptr(bar);
            (ptr as *const () as usize).hash(&mut hasher);
        }
        if let Some(ref bar) = state.instance_bar {
            let ptr = Arc::as_ptr(bar);
            (ptr as *const () as usize).hash(&mut hasher);
        }
        if let Some(ref bar) = state.instance_index_bar {
            let ptr = Arc::as_ptr(bar);
            (ptr as *const () as usize).hash(&mut hasher);
        }
        if let Some(ref bar) = state.topology_bar {
            let ptr = Arc::as_ptr(bar);
            (ptr as *const () as usize).hash(&mut hasher);
        }
        if let Some(ref bar) = state.varying_bar {
            let ptr = Arc::as_ptr(bar);
            (ptr as *const () as usize).hash(&mut hasher);
        }
        if let Some(ref bar) = state.face_varying_bar {
            let ptr = Arc::as_ptr(bar);
            (ptr as *const () as usize).hash(&mut hasher);
        }
        state.geometric_shader_key.hash(&mut hasher);
        state.primitive_kind.hash(&mut hasher);
        state.primitive_topology.hash(&mut hasher);
        hasher.finish()
    }

    /// Compute a hash of element-level BAR offsets for batch reorder detection.
    ///
    /// Port of C++ HdStDrawItem::GetElementOffsetsHash (P1-10).
    /// When elements within a shared buffer array are reordered (garbage collection
    /// compacted them), this hash changes, triggering a command buffer rebuild.
    pub fn get_element_offsets_hash(&self) -> u64 {
        use crate::buffer_resource::HdStBufferArrayRange;
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let state = self.state.read();
        let mut hasher = DefaultHasher::new();
        if let Some(ref bar) = state.vertex_bar {
            if let Some(st_bar) = bar.as_any().downcast_ref::<HdStBufferArrayRange>() {
                st_bar.get_offset().hash(&mut hasher);
                st_bar.get_size().hash(&mut hasher);
            }
        }
        if let Some(ref bar) = state.element_bar {
            if let Some(st_bar) = bar.as_any().downcast_ref::<HdStBufferArrayRange>() {
                st_bar.get_offset().hash(&mut hasher);
                st_bar.get_size().hash(&mut hasher);
            }
        }
        hasher.finish()
    }

    /// Test whether this draw item's bbox intersects the view volume.
    ///
    /// Port of C++ HdStDrawItem::IntersectsViewVolume (P1-11).
    /// Used for CPU frustum culling. Returns true when the AABB is either
    /// invalid (unbounded) or overlaps the frustum planes.
    ///
    /// `frustum_planes`: 6 frustum planes in world space, each as [a,b,c,d]
    /// where ax+by+cz+d >= 0 is inside.  World-space transform must be applied
    /// to bbox_min/max before testing.
    pub fn intersects_view_volume(&self, frustum_planes: &[[f32; 4]; 6]) -> bool {
        if !self.has_valid_bbox() {
            // No AABB set — conservatively include in draw list
            return true;
        }
        let state = self.state.read();
        let mn = state.bbox_min;
        let mx = state.bbox_max;
        // AABB vs frustum half-space test (positive vertex test).
        // For each plane, find the "most positive" corner of the AABB.
        for plane in frustum_planes {
            let px = if plane[0] > 0.0 { mx[0] } else { mn[0] };
            let py = if plane[1] > 0.0 { mx[1] } else { mn[1] };
            let pz = if plane[2] > 0.0 { mx[2] } else { mn[2] };
            // If the most positive corner is outside the plane, AABB is outside
            if plane[0] * px + plane[1] * py + plane[2] * pz + plane[3] < 0.0 {
                return false;
            }
        }
        true
    }
}

/// Shared pointer to draw item.
pub type HdStDrawItemSharedPtr = Arc<HdStDrawItem>;

/// DrawingCoord slot indices for the barContainer.
///
/// Port of C++ HdDrawItem::DrawingCoord enum. Each slot maps to a different
/// buffer array range class (model/constant/element/vertex/etc.).
/// The barContainer holds Arc<dyn HdBufferArrayRange> at these indices.
pub mod drawing_coord {
    pub const MODEL: usize = 0;
    pub const CONSTANT: usize = 1;
    pub const VERTEX: usize = 2;
    pub const ELEMENT: usize = 3;
    pub const VARYING: usize = 4;
    pub const FACE_VARYING: usize = 5;
    pub const TOP_VIS: usize = 6;
    pub const INSTANCE_INDEX: usize = 7;
    /// First instance primvar level (additional levels follow sequentially).
    pub const INSTANCE_PVAR: usize = 8;
    /// Total number of standard DC slots.
    pub const COUNT: usize = 9;
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_sdf::Path as SdfPath;

    #[test]
    fn test_draw_item_creation() {
        let path = SdfPath::from_string("/test").unwrap();
        let item = HdStDrawItem::new(path.clone());

        assert_eq!(item.get_prim_path(), &path);
        assert!(item.is_visible());
        assert!(!item.is_valid()); // No buffers set yet
    }

    #[test]
    fn test_visibility() {
        let path = SdfPath::from_string("/test").unwrap();
        let item = HdStDrawItem::new(path);

        assert!(item.is_visible());
        item.set_visible(false);
        assert!(!item.is_visible());
    }

    #[test]
    fn test_fvar_topology_mapping_roundtrips() {
        let path = SdfPath::from_string("/test").unwrap();
        let item = HdStDrawItem::new(path);
        let mapping = vec![TopologyToPrimvarEntry {
            topology: vec![0, 1, 2, 3],
            primvars: vec![Token::new("normals"), Token::new("st")],
        }];

        item.set_fvar_topology_to_primvar_vector(mapping.clone());

        assert_eq!(item.get_fvar_topology_to_primvar_vector(), mapping);
    }

    #[test]
    fn test_shared_draw_item_mutation_is_visible_across_arcs() {
        let path = SdfPath::from_string("/shared").unwrap();
        let item = Arc::new(HdStDrawItem::new(path));
        let shared_view = Arc::clone(&item);

        item.set_repr(Token::new("hull"));
        item.set_visible(false);
        item.set_bbox([1.0, 2.0, 3.0], [4.0, 5.0, 6.0]);

        assert_eq!(shared_view.get_repr(), Token::new("hull"));
        assert!(!shared_view.is_visible());
        assert_eq!(shared_view.get_bbox_min(), [1.0, 2.0, 3.0]);
        assert_eq!(shared_view.get_bbox_max(), [4.0, 5.0, 6.0]);
    }
}
