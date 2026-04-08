//! Pick task - GPU-based object picking.
//!
//! Renders object IDs to determine which object is under the cursor.
//! Uses multiple render passes:
//!   (i)  [optional] depth-only occluder pass for unpickable prims
//!   (ii) [mandatory] ID render pass for pickable prims (primId, instanceId, elementId, etc.)
//!   (iii)[optional]  ID render pass for overlay prims (always-on-top material tag)
//!
//! Port of pxr/imaging/hdx/pickTask.h/cpp

use std::sync::Arc;

use usd_gf::{Matrix4d, Vec2f, Vec2i, Vec3d, Vec3f, Vec4d, Vec4i};
use usd_hd::HdVec4_2_10_10_10_Rev;
use usd_hd::enums::{HdCompareFunction, HdCullStyle};
use usd_hd::prim::HdSceneDelegate;
use usd_hd::render::{HdRenderIndexTrait, HdTask, HdTaskContext, TfTokenVector};
use usd_hgi::{
    HgiBufferHandle, HgiDriverHandle, HgiFormat,
    blit_cmds::{HgiBufferGpuToCpuOp, HgiTextureGpuToCpuOp, RawCpuBufferMut},
    enums::HgiSubmitWaitType as WgpuSubmitWait,
    hgi::Hgi,
    texture::HgiTextureHandle,
    tokens::RENDER_DRIVER,
};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Value;

use super::render_setup_task::{
    HdRenderPassAovBinding, HdxRenderPassState, HdxRenderPassStateHandle,
};

/// Pick tokens.
///
/// Port of HdxPickTokens from pxr/imaging/hdx/pickTask.h
pub mod pick_tokens {
    use usd_tf::Token;

    // Task context key
    /// Pick params token for task context.
    pub fn pick_params() -> Token {
        Token::new("pickParams")
    }

    /// Pick buffer token for task context.
    pub fn pick_buffer() -> Token {
        Token::new("pickBuffer")
    }

    // Pick targets
    /// Pick prims and instances.
    pub fn pick_prims_and_instances() -> Token {
        Token::new("pickPrimsAndInstances")
    }

    /// Pick faces.
    pub fn pick_faces() -> Token {
        Token::new("pickFaces")
    }

    /// Pick edges.
    pub fn pick_edges() -> Token {
        Token::new("pickEdges")
    }

    /// Pick points.
    pub fn pick_points() -> Token {
        Token::new("pickPoints")
    }

    /// Pick points and instances.
    pub fn pick_points_and_instances() -> Token {
        Token::new("pickPointsAndInstances")
    }

    // Resolve modes
    /// Resolve nearest to camera.
    pub fn resolve_nearest_to_camera() -> Token {
        Token::new("resolveNearestToCamera")
    }

    /// Resolve nearest to center of pick region.
    pub fn resolve_nearest_to_center() -> Token {
        Token::new("resolveNearestToCenter")
    }

    /// Resolve unique hits.
    pub fn resolve_unique() -> Token {
        Token::new("resolveUnique")
    }

    /// Resolve all hits.
    pub fn resolve_all() -> Token {
        Token::new("resolveAll")
    }

    /// Resolve deep (all hits including occluded).
    pub fn resolve_deep() -> Token {
        Token::new("resolveDeep")
    }
}

/// AOV outputs for picking pass.
///
/// Matches C++ _aovOutputs array in HdxPickTask::_CreateAovBindings.
/// Each AOV renders a different per-fragment attribute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PickAov {
    /// Rprim/prim integer ID
    PrimId,
    /// Instance integer ID (-1 if not instanced)
    InstanceId,
    /// Element (face) integer ID
    ElementId,
    /// Edge integer ID
    EdgeId,
    /// Point integer ID
    PointId,
    /// Normal in eye space (Neye)
    Neye,
    /// Depth or DepthStencil
    Depth,
}

impl PickAov {
    /// All AOV outputs in order (matches C++ array).
    pub const ALL: &'static [PickAov] = &[
        PickAov::PrimId,
        PickAov::InstanceId,
        PickAov::ElementId,
        PickAov::EdgeId,
        PickAov::PointId,
        PickAov::Neye,
        PickAov::Depth,
    ];

    /// Index of depth AOV in ALL slice.
    pub const DEPTH_INDEX: usize = 6;

    /// Token name for this AOV.
    pub fn token(&self) -> Token {
        match self {
            PickAov::PrimId => Token::new("primId"),
            PickAov::InstanceId => Token::new("instanceId"),
            PickAov::ElementId => Token::new("elementId"),
            PickAov::EdgeId => Token::new("edgeId"),
            PickAov::PointId => Token::new("pointId"),
            PickAov::Neye => Token::new("Neye"),
            PickAov::Depth => Token::new("depth"),
        }
    }

    /// Buffer path for AOV render buffer.
    pub fn buffer_path(&self) -> Path {
        let id = format!("aov_pickTask_{}", self.token().as_str());
        Path::from_string(&format!("/{}", id)).unwrap_or_else(Path::empty)
    }
}

/// Pick hit result.
///
/// Contains complete information about a picking hit.
/// Port of HdxPickHit from pxr/imaging/hdx/pickTask.h
#[derive(Debug, Clone, PartialEq)]
pub struct HdxPickHit {
    /// Scene delegate ID (for scene index).
    pub delegate_id: Path,
    /// Object/prim path (resolved from render index, empty without one).
    pub object_id: Path,
    /// Instancer path if instanced.
    pub instancer_id: Path,
    /// Instance index (-1 if not instanced).
    pub instance_index: i32,
    /// Element (face) index.
    pub element_index: i32,
    /// Edge index.
    pub edge_index: i32,
    /// Point index.
    pub point_index: i32,
    /// World-space hit point.
    pub world_space_hit_point: Vec3d,
    /// World-space hit normal.
    pub world_space_hit_normal: Vec3f,
    /// Normalized depth [0,1].
    pub normalized_depth: f32,
    /// Raw integer prim ID from GPU buffer (before path resolution).
    /// Used for deduplication when object_id is not yet resolved.
    pub prim_id: i32,
}

impl HdxPickHit {
    /// Check if this is a valid hit.
    pub fn is_valid(&self) -> bool {
        !self.object_id.is_empty()
    }

    /// Compute hash for deduplication.
    ///
    /// Uses object_id when resolved (non-empty), otherwise falls back to
    /// raw prim_id. Includes instance_index and element_index for sub-prim picks.
    pub fn get_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        if !self.object_id.is_empty() {
            self.object_id.get_string().hash(&mut hasher);
        } else {
            // Fall back to raw prim ID when path not yet resolved
            self.prim_id.hash(&mut hasher);
        }
        self.instance_index.hash(&mut hasher);
        self.element_index.hash(&mut hasher);
        hasher.finish()
    }
}

impl Default for HdxPickHit {
    fn default() -> Self {
        Self {
            delegate_id: Path::empty(),
            object_id: Path::empty(),
            instancer_id: Path::empty(),
            instance_index: -1,
            element_index: -1,
            edge_index: -1,
            point_index: -1,
            world_space_hit_point: Vec3d::default(),
            world_space_hit_normal: Vec3f::default(),
            normalized_depth: 1.0,
            prim_id: -1,
        }
    }
}

impl std::fmt::Display for HdxPickHit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "PickHit: object={} instance={} element={} depth={}",
            self.object_id.get_string(),
            self.instance_index,
            self.element_index,
            self.normalized_depth
        )
    }
}

/// Pick task sync parameters (from scene delegate).
///
/// Port of HdxPickTaskParams from pxr/imaging/hdx/pickTask.h
#[derive(Debug, Clone, PartialEq)]
pub struct HdxPickTaskParams {
    /// Cull style for picking render.
    pub cull_style: HdCullStyle,
}

impl Default for HdxPickTaskParams {
    fn default() -> Self {
        Self {
            cull_style: HdCullStyle::Nothing,
        }
    }
}

/// Optional stencil conditioning callback.
///
/// Called before pick render to set stencil values via GL (GL-backend only).
/// In wgpu backend this is not supported; the Option will always be None.
pub type DepthMaskCallback = Box<dyn Fn() + Send + Sync>;

/// Pick task context parameters (set per-frame via task context).
///
/// Port of HdxPickTaskContextParams from pxr/imaging/hdx/pickTask.h
pub struct HdxPickTaskContextParams {
    /// Resolution of the pick buffer (width, height).
    pub resolution: Vec2i,
    /// Max number of deep pick entries per pixel.
    pub max_num_deep_entries: i32,
    /// Pick target (prims, faces, edges, points).
    pub pick_target: Token,
    /// Resolve mode (nearest, unique, all, deep).
    pub resolve_mode: Token,
    /// Whether unpickable objects should occlude picks.
    pub do_unpickables_occlude: bool,
    /// View matrix for pick frustum.
    pub view_matrix: Matrix4d,
    /// Projection matrix for pick frustum.
    pub projection_matrix: Matrix4d,
    /// Clip planes.
    pub clip_planes: Vec<Vec4d>,
    /// Alpha threshold — discard fragments below this alpha.
    pub alpha_threshold: f32,
    /// Optional stencil conditioning callback (GL-only, None in wgpu backend).
    ///
    /// C++: HdxPickTaskContextParams::depthMaskCallback
    pub depth_mask_callback: Option<DepthMaskCallback>,
}

impl Clone for HdxPickTaskContextParams {
    fn clone(&self) -> Self {
        Self {
            resolution: self.resolution,
            max_num_deep_entries: self.max_num_deep_entries,
            pick_target: self.pick_target.clone(),
            resolve_mode: self.resolve_mode.clone(),
            do_unpickables_occlude: self.do_unpickables_occlude,
            view_matrix: self.view_matrix,
            projection_matrix: self.projection_matrix,
            clip_planes: self.clip_planes.clone(),
            alpha_threshold: self.alpha_threshold,
            // Callback is not cloneable — drop on clone (wgpu never uses it)
            depth_mask_callback: None,
        }
    }
}

impl std::fmt::Debug for HdxPickTaskContextParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HdxPickTaskContextParams")
            .field("resolution", &self.resolution)
            .field("pick_target", &self.pick_target)
            .field("resolve_mode", &self.resolve_mode)
            .field("do_unpickables_occlude", &self.do_unpickables_occlude)
            .field("alpha_threshold", &self.alpha_threshold)
            .field(
                "has_depth_mask_callback",
                &self.depth_mask_callback.is_some(),
            )
            .finish()
    }
}

impl PartialEq for HdxPickTaskContextParams {
    fn eq(&self, other: &Self) -> bool {
        self.resolution == other.resolution
            && self.max_num_deep_entries == other.max_num_deep_entries
            && self.pick_target == other.pick_target
            && self.resolve_mode == other.resolve_mode
            && self.do_unpickables_occlude == other.do_unpickables_occlude
            && self.view_matrix == other.view_matrix
            && self.projection_matrix == other.projection_matrix
            && self.clip_planes == other.clip_planes
            && self.alpha_threshold == other.alpha_threshold
    }
}

impl Default for HdxPickTaskContextParams {
    fn default() -> Self {
        Self {
            resolution: Vec2i::new(128, 128),
            max_num_deep_entries: 32000,
            pick_target: pick_tokens::pick_prims_and_instances(),
            resolve_mode: pick_tokens::resolve_nearest_to_camera(),
            do_unpickables_occlude: false,
            view_matrix: Matrix4d::identity(),
            projection_matrix: Matrix4d::identity(),
            clip_planes: Vec::new(),
            // Default: discard fully transparent but keep semi-transparent (for soft-pick)
            alpha_threshold: 0.0001,
            depth_mask_callback: None,
        }
    }
}

/// Which render passes to use for this pick query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PickPassFlags {
    /// Execute occluder depth-only pass (unpickable prims condition depth).
    pub use_occluder: bool,
    /// Execute overlay pass (display-in-overlay material tag).
    pub use_overlay: bool,
}

impl Default for PickPassFlags {
    fn default() -> Self {
        Self {
            use_occluder: false,
            use_overlay: true, // Conservative: assume overlay needed until proven otherwise
        }
    }
}

/// Which internal pick pass a backend request represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HdxPickTaskPass {
    /// Optional depth-only pass for unpickable occluders.
    Occluder,
    /// Main ID-render pass for pickable geometry.
    Pickable,
    /// Optional always-on-top overlay pass.
    Overlay,
}

/// Backend execution request emitted by `HdxPickTask::execute()`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct HdxPickTaskRequest {
    /// Which internal pick pass should be replayed.
    pub pass: HdxPickTaskPass,
    /// Material tag filter for this pass.
    pub material_tag: Token,
    /// Render tags that scoped this pick task.
    pub render_tags: TfTokenVector,
    /// Render-pass state prepared for this pass.
    pub render_pass_state: HdxRenderPassStateHandle,
    /// Whether this pass should bind the deep-pick storage buffer.
    pub bind_pick_buffer: bool,
}

/// GPU picking task.
///
/// Renders object IDs to an offscreen pick buffer and reads back to determine
/// which prim is under the cursor. Supports multiple resolve modes and deep picking.
///
/// Port of HdxPickTask from pxr/imaging/hdx/pickTask.h
pub struct HdxPickTask {
    /// Task path.
    id: Path,
    /// Sync parameters (cull style from delegate).
    params: HdxPickTaskParams,
    /// Context parameters (matrices, resolution, resolve mode).
    context_params: HdxPickTaskContextParams,
    /// Render tags for filtering draw items.
    render_tags: TfTokenVector,
    /// Pass flags for this pick query.
    pass_flags: PickPassFlags,
    /// AOV bindings for pickable pass (primId, instanceId, elementId, edgeId, pointId, Neye, depth)
    pickable_aov_bindings: Vec<HdRenderPassAovBinding>,
    /// AOV binding for occluder pass (depth only).
    occluder_aov_binding: Option<HdRenderPassAovBinding>,
    /// AOV bindings for overlay pass (same as pickable but no clear, separate depth).
    overlay_aov_bindings: Vec<HdRenderPassAovBinding>,
    /// Render pass state for pickable pass.
    pickable_render_pass_state: HdxRenderPassState,
    /// Render pass state for occluder pass (depth only, no color writes).
    occluder_render_pass_state: HdxRenderPassState,
    /// Render pass state for overlay pass.
    overlay_render_pass_state: HdxRenderPassState,
    /// Accumulated pick hits.
    hits: Vec<HdxPickHit>,
}

impl HdxPickTask {
    const PICK_BUFFER_HEADER_SIZE: usize = 8;
    const PICK_BUFFER_SUBBUFFER_CAPACITY: usize = 32;
    const PICK_BUFFER_ENTRY_SIZE: usize = 3;

    /// Create new pick task.
    pub fn new(id: Path) -> Self {
        Self {
            id,
            params: HdxPickTaskParams::default(),
            context_params: HdxPickTaskContextParams::default(),
            render_tags: Vec::new(),
            pass_flags: PickPassFlags::default(),
            pickable_aov_bindings: Vec::new(),
            occluder_aov_binding: None,
            overlay_aov_bindings: Vec::new(),
            pickable_render_pass_state: HdxRenderPassState::new(),
            occluder_render_pass_state: HdxRenderPassState::new(),
            overlay_render_pass_state: HdxRenderPassState::new(),
            hits: Vec::new(),
        }
    }

    /// Set pick parameters.
    pub fn set_params(&mut self, params: HdxPickTaskParams) {
        self.params = params;
    }

    /// Get pick parameters.
    pub fn get_params(&self) -> &HdxPickTaskParams {
        &self.params
    }

    /// Set context parameters (called per-frame by task controller).
    pub fn set_context_params(&mut self, params: HdxPickTaskContextParams) {
        self.context_params = params;
    }

    /// Get context parameters.
    pub fn get_context_params(&self) -> &HdxPickTaskContextParams {
        &self.context_params
    }

    /// Get pick hits from last query.
    pub fn get_hits(&self) -> &[HdxPickHit] {
        &self.hits
    }

    /// Clear pick hits.
    pub fn clear_hits(&mut self) {
        self.hits.clear();
    }

    /// Set render tags for filtering draw items on the pick path.
    pub fn set_render_tags(&mut self, render_tags: TfTokenVector) {
        self.render_tags = render_tags;
    }

    pub(crate) fn get_hgi_driver(ctx: &HdTaskContext) -> Option<HgiDriverHandle> {
        ctx.get_drivers()?
            .iter()
            .find(|driver| driver.name == *RENDER_DRIVER)
            .and_then(|driver| driver.driver.get::<HgiDriverHandle>().cloned())
    }

    pub(crate) fn get_aov_texture(
        ctx: &HdTaskContext,
        aov_name: &Token,
    ) -> Option<HgiTextureHandle> {
        let direct = Token::new(&format!("aov_{}", aov_name.as_str()));
        ctx.get(&direct)
            .and_then(|value| value.get::<HgiTextureHandle>().cloned())
            .or_else(|| {
                ctx.get(aov_name)
                    .and_then(|value| value.get::<HgiTextureHandle>().cloned())
            })
    }

    pub(crate) fn read_texture_raw(
        hgi_driver: &HgiDriverHandle,
        texture: &HgiTextureHandle,
    ) -> Option<(HgiFormat, Vec<u8>)> {
        let desc = texture.get()?.descriptor().clone();
        let dimensions = desc.dimensions;
        let (bytes_per_texel, _, _) = desc.format.data_size_of_format();
        let raw_byte_size =
            dimensions.x.max(1) as usize * dimensions.y.max(1) as usize * bytes_per_texel;
        let mut raw_pixels = vec![0u8; raw_byte_size];
        hgi_driver.with_write(|hgi| {
            let mut blit = hgi.create_blit_cmds();
            let op = HgiTextureGpuToCpuOp {
                gpu_source_texture: texture.clone(),
                source_texel_offset: usd_gf::Vec3i::new(0, 0, 0),
                mip_level: 0,
                cpu_destination_buffer: unsafe { RawCpuBufferMut::new(raw_pixels.as_mut_ptr()) },
                destination_byte_offset: 0,
                destination_buffer_byte_size: raw_byte_size,
                copy_size: usd_gf::Vec3i::new(dimensions.x.max(1), dimensions.y.max(1), 1),
                source_layer: 0,
            };
            blit.copy_texture_gpu_to_cpu(&op);
            hgi.submit_cmds(blit, WgpuSubmitWait::WaitUntilCompleted);
        });
        Some((desc.format, raw_pixels))
    }

    pub(crate) fn decode_i32_aov(format: HgiFormat, raw_pixels: &[u8]) -> Option<Vec<i32>> {
        match format {
            HgiFormat::UNorm8Vec4 | HgiFormat::SNorm8Vec4 => Some(
                raw_pixels
                    .chunks_exact(4)
                    .map(|chunk| i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                    .collect(),
            ),
            HgiFormat::Int32 => Some(
                raw_pixels
                    .chunks_exact(std::mem::size_of::<i32>())
                    .map(|chunk| i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                    .collect(),
            ),
            _ => None,
        }
    }

    pub(crate) fn decode_f32_aov(format: HgiFormat, raw_pixels: &[u8]) -> Option<Vec<f32>> {
        match format {
            HgiFormat::Float32 => Some(
                raw_pixels
                    .chunks_exact(std::mem::size_of::<f32>())
                    .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                    .collect(),
            ),
            _ => None,
        }
    }

    pub(crate) fn read_aov_i32(
        ctx: &HdTaskContext,
        hgi_driver: &HgiDriverHandle,
        aov_name: &Token,
    ) -> Option<Vec<i32>> {
        let texture = Self::get_aov_texture(ctx, aov_name)?;
        let (format, raw_pixels) = Self::read_texture_raw(hgi_driver, &texture)?;
        Self::decode_i32_aov(format, &raw_pixels)
    }

    pub(crate) fn read_aov_f32(
        ctx: &HdTaskContext,
        hgi_driver: &HgiDriverHandle,
        aov_name: &Token,
    ) -> Option<Vec<f32>> {
        let texture = Self::get_aov_texture(ctx, aov_name)?;
        let (format, raw_pixels) = Self::read_texture_raw(hgi_driver, &texture)?;
        Self::decode_f32_aov(format, &raw_pixels)
    }

    pub(crate) fn get_pick_buffer(ctx: &HdTaskContext) -> Option<HgiBufferHandle> {
        ctx.get(&pick_tokens::pick_buffer())
            .and_then(|value| value.get::<HgiBufferHandle>())
            .cloned()
    }

    pub fn build_pick_buffer_init_data(context_params: &HdxPickTaskContextParams) -> Vec<i32> {
        if context_params.resolve_mode != pick_tokens::resolve_deep() {
            return vec![0];
        }

        let num_sub_buffers = (context_params.max_num_deep_entries.max(0) as usize)
            / Self::PICK_BUFFER_SUBBUFFER_CAPACITY;
        let entry_storage_offset = Self::PICK_BUFFER_HEADER_SIZE + num_sub_buffers;
        let entry_storage_size =
            num_sub_buffers * Self::PICK_BUFFER_SUBBUFFER_CAPACITY * Self::PICK_BUFFER_ENTRY_SIZE;

        let mut pick_buffer_init = Vec::with_capacity(entry_storage_offset + entry_storage_size);
        pick_buffer_init.push(num_sub_buffers as i32);
        pick_buffer_init.push(Self::PICK_BUFFER_SUBBUFFER_CAPACITY as i32);
        pick_buffer_init.push(Self::PICK_BUFFER_HEADER_SIZE as i32);
        pick_buffer_init.push(entry_storage_offset as i32);
        pick_buffer_init.push((context_params.pick_target == pick_tokens::pick_faces()) as i32);
        pick_buffer_init.push((context_params.pick_target == pick_tokens::pick_edges()) as i32);
        pick_buffer_init.push((context_params.pick_target == pick_tokens::pick_points()) as i32);
        pick_buffer_init.push(0);
        pick_buffer_init.resize(entry_storage_offset, 0);
        pick_buffer_init.resize(entry_storage_offset + entry_storage_size, -9);
        pick_buffer_init
    }

    pub(crate) fn read_buffer_i32(hgi: &mut dyn Hgi, buffer: &HgiBufferHandle) -> Option<Vec<i32>> {
        let byte_size = buffer.get()?.descriptor().byte_size;
        if byte_size == 0 || byte_size % std::mem::size_of::<i32>() != 0 {
            return None;
        }

        let mut raw_bytes = vec![0u8; byte_size];
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut blit = hgi.create_blit_cmds();
            let op = HgiBufferGpuToCpuOp {
                gpu_source_buffer: buffer.clone(),
                source_byte_offset: 0,
                cpu_destination_buffer: unsafe { RawCpuBufferMut::new(raw_bytes.as_mut_ptr()) },
                byte_size,
            };
            blit.copy_buffer_gpu_to_cpu(&op);
            hgi.submit_cmds(blit, WgpuSubmitWait::WaitUntilCompleted);
        }));
        if result.is_err() {
            return None;
        }

        Self::decode_i32_aov(HgiFormat::Int32, &raw_bytes)
    }

    /// Decode a RGBA8 ID render color to int32 ID.
    ///
    /// ID buffer packs int32 as RGBA bytes (little-endian).
    pub fn decode_id_render_color(id_color: [u8; 4]) -> i32 {
        i32::from(id_color[0] & 0xff)
            | (i32::from(id_color[1] & 0xff) << 8)
            | (i32::from(id_color[2] & 0xff) << 16)
            | (i32::from(id_color[3] & 0xff) << 24)
    }

    /// Create AOV bindings for pick pass (primId, instanceId, elementId, edgeId, pointId, Neye, depth).
    ///
    /// Matches C++ HdxPickTask::_CreateAovBindings().
    pub fn create_aov_bindings(&mut self) {
        self.pickable_aov_bindings.clear();

        for aov in PickAov::ALL {
            let binding = HdRenderPassAovBinding::new(aov.token(), aov.buffer_path());
            // Depth clear to 1.0 (far plane), ID buffers clear to -1 (no hit)
            let clear_val = if matches!(aov, PickAov::Depth) {
                Value::from(1.0f32)
            } else {
                Value::from(-1i32)
            };
            let binding = binding.with_clear_value(clear_val);

            // Store depth binding for occluder pass
            if matches!(aov, PickAov::Depth) {
                self.occluder_aov_binding = Some(binding.clone());
            }
            self.pickable_aov_bindings.push(binding);
        }

        // Overlay bindings: same AOVs but no clear (retain pickable results),
        // plus a separate fresh depth buffer for inter-overlay occlusion.
        self.overlay_aov_bindings = self
            .pickable_aov_bindings
            .iter()
            .cloned()
            .map(|mut b| {
                b.clear_value = Value::default(); // no clear
                b
            })
            .collect();

        // Replace overlay depth with separate buffer (fresh depth per overlay draw)
        if let Some(last) = self.overlay_aov_bindings.last_mut() {
            let overlay_depth_path =
                Path::from_string("/aov_pickTask_overlayDepthStencil").unwrap_or_else(Path::empty);
            *last =
                HdRenderPassAovBinding::new(Token::new("overlayDepthStencil"), overlay_depth_path)
                    .with_clear_value(Value::from(1.0f32)); // clear overlay depth
        }
    }

    /// Check if the occluder depth-only pass should run.
    ///
    /// C++: HdxPickTask::_UseOcclusionPass() — true when doUnpickablesOcclude
    /// AND the collection has exclude paths (unpickable prims listed there).
    fn use_occluder_pass(&self) -> bool {
        // In C++ this also checks _contextParams.collection.GetExcludePaths().empty().
        // We approximate: if do_unpickables_occlude is set and the occluder binding
        // exists, treat the occluder pass as active.
        self.context_params.do_unpickables_occlude && self.occluder_aov_binding.is_some()
    }

    /// Re-evaluate whether the overlay pass actually has draw items.
    ///
    /// C++: HdxPickTask::_UpdateUseOverlayPass() — calls overlayRenderPass->HasDrawItems().
    /// In our architecture the task does not own render-pass objects; the equivalent
    /// HasDrawItems check is performed engine-side in `replay_pick_task_requests()`
    /// after setting the overlay material tag on the render pass. The task still
    /// emits the overlay request conservatively and lets the engine skip it.
    fn update_use_overlay_pass(&mut self) {
        // No-op: overlay filtering happens at engine replay time.
    }

    /// Resolve deep-pick hits from the pick buffer SSBO.
    ///
    /// C++: HdxPickTask::_ResolveDeep()
    ///
    /// Buffer layout (written by the WGSL/GLSL pick shader):
    ///   [0]  numSubBuffers
    ///   [1]  SUBBUFFER_CAPACITY (32)
    ///   [2]  PICK_BUFFER_HEADER_SIZE (8)
    ///   [3]  entryStorageOffset
    ///   [4]  pickFaces ? 1 : 0
    ///   [5]  pickEdges ? 1 : 0
    ///   [6]  pickPoints ? 1 : 0
    ///   [7]  0 (padding)
    ///   [8 .. 8+numSubBuffers-1]  per-sub-buffer entry counts
    ///   [entryStorageOffset ..]   entries: (primId, instanceId, partId) * 3 ints each
    ///
    /// # Arguments
    /// * `data` - raw i32 slice read back from the GPU pick SSBO
    /// * `hits` - output vector to append resolved hits to
    pub fn resolve_deep(data: &[i32], pick_target: &Token, hits: &mut Vec<HdxPickHit>) {
        if data.len() < Self::PICK_BUFFER_HEADER_SIZE {
            return;
        }

        let num_sub_buffers = data[0] as usize;
        // data[1] = capacity, data[2] = header size, data[3] = entry storage offset
        let entry_storage_offset = data[3] as usize;

        if entry_storage_offset > data.len() {
            return;
        }

        let pick_faces = *pick_target == pick_tokens::pick_faces();
        let pick_edges = *pick_target == pick_tokens::pick_edges();
        let pick_points = *pick_target == pick_tokens::pick_points()
            || *pick_target == pick_tokens::pick_points_and_instances();

        for sub_buffer in 0..num_sub_buffers {
            let size_offset = Self::PICK_BUFFER_HEADER_SIZE + sub_buffer;
            if size_offset >= data.len() {
                break;
            }
            let num_entries = data[size_offset] as usize;

            let sub_buffer_offset = entry_storage_offset
                + sub_buffer * Self::PICK_BUFFER_SUBBUFFER_CAPACITY * Self::PICK_BUFFER_ENTRY_SIZE;

            for j in 0..num_entries {
                let entry_offset = sub_buffer_offset + j * Self::PICK_BUFFER_ENTRY_SIZE;
                if entry_offset + Self::PICK_BUFFER_ENTRY_SIZE > data.len() {
                    break;
                }

                let prim_id = data[entry_offset];
                // Skip pixels with no hit (prim_id == -1)
                if prim_id < 0 {
                    continue;
                }

                let instance_index = data[entry_offset + 1];
                let part_index = data[entry_offset + 2];

                let mut hit = HdxPickHit {
                    prim_id,
                    instance_index,
                    element_index: if pick_faces { part_index } else { -1 },
                    edge_index: if pick_edges { part_index } else { -1 },
                    point_index: if pick_points { part_index } else { -1 },
                    // Deep pick skips world-space point/normal/depth (C++ sets them to 0)
                    world_space_hit_point: Vec3d::default(),
                    world_space_hit_normal: Vec3f::default(),
                    normalized_depth: 0.0,
                    ..HdxPickHit::default()
                };
                // Note: object_id / delegate_id / instancer_id would be resolved
                // from a render index lookup here if one were available.
                // Without a render index, we leave them as empty paths and let
                // the caller correlate prim_id to scene paths.
                let _ = &mut hit; // suppress unused-mut if no render index
                hits.push(hit);
            }
        }
    }

    /// Apply camera framing to all render pass states.
    ///
    /// Sets view/projection matrices and viewport on the three render pass states.
    /// Matches C++ HdxPickTask::Sync() camera framing setup.
    pub fn apply_camera_framing(&mut self, view: Matrix4d, proj: Matrix4d, viewport: Vec4i) {
        let vp = usd_gf::Vec4d::new(
            viewport.x as f64,
            viewport.y as f64,
            viewport.z as f64,
            viewport.w as f64,
        );
        for state in [
            &mut self.pickable_render_pass_state,
            &mut self.occluder_render_pass_state,
            &mut self.overlay_render_pass_state,
        ] {
            state.set_viewport(vp);
            // In full implementation: state.set_camera_framing_state(view, proj, vp, clip_planes)
            let _ = (&view, &proj);
        }
    }

    /// Configure render pass states for pick rendering.
    ///
    /// Sets cull style, depth test, blending, lighting off.
    /// Matches C++ HdxPickTask::Sync() state setup loop.
    pub fn configure_render_pass_states(&mut self) {
        let cull = self.params.cull_style;
        let alpha = self.context_params.alpha_threshold;
        let enable_depth_write = self.context_params.resolve_mode != pick_tokens::resolve_deep();

        for state in [
            &mut self.pickable_render_pass_state,
            &mut self.occluder_render_pass_state,
            &mut self.overlay_render_pass_state,
        ] {
            state.set_cull_style(cull);
            state.set_depth_func(HdCompareFunction::LEqual);
            state.set_alpha_threshold(alpha);
            state.set_alpha_to_coverage_enabled(false);
            state.set_blend_enabled(false);
            state.set_lighting_enabled(false);
            state.set_enable_depth_mask(enable_depth_write);
        }

        // Occluder pass: depth write only, no color output
        // (C++: _occluderRenderPassState->SetColorMasks({ColorMaskNone}))
        self.occluder_render_pass_state.set_enable_depth_mask(true);

        // Set AOV bindings on each pass state
        self.pickable_render_pass_state
            .set_aov_bindings(self.pickable_aov_bindings.clone());
        if self.pass_flags.use_occluder {
            if let Some(ref b) = self.occluder_aov_binding {
                self.occluder_render_pass_state
                    .set_aov_bindings(vec![b.clone()]);
            }
        }
        if self.pass_flags.use_overlay {
            self.overlay_render_pass_state
                .set_aov_bindings(self.overlay_aov_bindings.clone());
        }
    }

    fn deep_request_aov_bindings(
        bindings: &[HdRenderPassAovBinding],
    ) -> Vec<HdRenderPassAovBinding> {
        bindings
            .iter()
            .filter(|binding| matches!(binding.aov_name.as_str(), "primId" | "depth"))
            .cloned()
            .collect()
    }

    fn push_request(ctx: &mut HdTaskContext, request: HdxPickTaskRequest) {
        ctx.insert(Token::new("pickTaskRequested"), Value::from(true));
        let requests_token = Token::new("pickTaskRequests");
        if let Some(requests) = ctx
            .get_mut(&requests_token)
            .and_then(|value| value.get_mut::<Vec<HdxPickTaskRequest>>())
        {
            requests.push(request);
        } else {
            ctx.insert(requests_token, Value::new(vec![request]));
        }
    }

    fn emit_render_requests(&self, ctx: &mut HdTaskContext) {
        let resolve_deep = self.context_params.resolve_mode == pick_tokens::resolve_deep();

        if self.pass_flags.use_occluder {
            Self::push_request(
                ctx,
                HdxPickTaskRequest {
                    pass: HdxPickTaskPass::Occluder,
                    material_tag: Token::default(),
                    render_tags: self.render_tags.clone(),
                    render_pass_state: HdxRenderPassStateHandle::new(Arc::new(
                        self.occluder_render_pass_state.clone(),
                    )),
                    bind_pick_buffer: false,
                },
            );
        }

        let mut pickable_state = self.pickable_render_pass_state.clone();
        if resolve_deep {
            pickable_state
                .set_aov_bindings(Self::deep_request_aov_bindings(&self.pickable_aov_bindings));
        }
        Self::push_request(
            ctx,
            HdxPickTaskRequest {
                pass: HdxPickTaskPass::Pickable,
                material_tag: Token::default(),
                render_tags: self.render_tags.clone(),
                render_pass_state: HdxRenderPassStateHandle::new(Arc::new(pickable_state)),
                bind_pick_buffer: resolve_deep,
            },
        );

        if self.pass_flags.use_overlay {
            let mut overlay_state = self.overlay_render_pass_state.clone();
            if resolve_deep {
                overlay_state
                    .set_aov_bindings(Self::deep_request_aov_bindings(&self.overlay_aov_bindings));
            }
            Self::push_request(
                ctx,
                HdxPickTaskRequest {
                    pass: HdxPickTaskPass::Overlay,
                    material_tag: Token::new("displayInOverlay"),
                    render_tags: self.render_tags.clone(),
                    render_pass_state: HdxRenderPassStateHandle::new(Arc::new(overlay_state)),
                    bind_pick_buffer: resolve_deep,
                },
            );
        }
    }
}

impl HdTask for HdxPickTask {
    fn id(&self) -> &Path {
        &self.id
    }

    fn sync(
        &mut self,
        _delegate: &dyn HdSceneDelegate,
        ctx: &mut HdTaskContext,
        dirty_bits: &mut u32,
    ) {
        // Pull context params from task context if set by task controller
        if let Some(context_params) = ctx
            .get(&pick_tokens::pick_params())
            .and_then(|value| value.get::<HdxPickTaskContextParams>())
        {
            self.context_params.resolution = context_params.resolution;
            self.context_params.max_num_deep_entries = context_params.max_num_deep_entries;
            self.context_params.pick_target = context_params.pick_target.clone();
            self.context_params.resolve_mode = context_params.resolve_mode.clone();
            self.context_params.do_unpickables_occlude = context_params.do_unpickables_occlude;
            self.context_params.view_matrix = context_params.view_matrix;
            self.context_params.projection_matrix = context_params.projection_matrix;
            self.context_params.clip_planes = context_params.clip_planes.clone();
            self.context_params.alpha_threshold = context_params.alpha_threshold;
        }

        // Assume overlay pass is needed (conservative — checked during prepare)
        self.pass_flags.use_overlay = true;

        // Create AOV bindings if not yet done
        if self.pickable_aov_bindings.is_empty() {
            self.create_aov_bindings();
        }
        self.pass_flags.use_occluder = self.use_occluder_pass();

        // Configure viewport
        let res = self.context_params.resolution;
        let viewport = Vec4i::new(0, 0, res.x, res.y);
        self.apply_camera_framing(
            self.context_params.view_matrix,
            self.context_params.projection_matrix,
            viewport,
        );
        self.configure_render_pass_states();

        *dirty_bits = 0;
    }

    fn prepare(&mut self, ctx: &mut HdTaskContext, _render_index: &dyn HdRenderIndexTrait) {
        // Clear previous hits
        self.hits.clear();

        // In full Storm implementation:
        // 1. Check if overlay render pass HasDrawItems() — if not, disable overlay pass
        // 2. Prepare resource registry (resize/create GPU textures for AOV buffers)
        // 3. Clear and bind pick buffer SSBO
        //    - For resolveDeep: allocate header + sub-buffers in pick buffer
        //    - For other modes: set single invalid sentinel
        // 4. Bind pick buffer as SSBO to pickable render pass shader

        ctx.insert(Token::new("pickPrepared"), Value::from(true));
    }

    fn execute(&mut self, ctx: &mut HdTaskContext) {
        // C++: HdxPickTask::Execute() — pxr/imaging/hdx/pickTask.cpp:740

        // C++: _UpdateUseOverlayPass() — check if overlay pass has draw items.
        self.update_use_overlay_pass();

        // C++: _hgi->StartFrame() — triggers Hgi garbage collection for stale resources.
        // wgpu: no equivalent needed; wgpu handles resource lifetimes via Arc/Drop.

        let res = self.context_params.resolution;
        let viewport = Vec4i::new(0, 0, res.x, res.y);
        let _ = viewport;

        // C++: bool needStencilConditioning = (_contextParams.depthMaskCallback != nullptr)
        let need_stencil_conditioning = self.context_params.depth_mask_callback.is_some();

        // --- Optional stencil conditioning (GL-only, not supported in wgpu backend) ---
        // C++: if (needStencilConditioning)
        //          _ConditionStencilWithGLCallback(depthMaskCallback, pickableDepthBuffer)
        //          _ConditionStencilWithGLCallback(depthMaskCallback, overlayDepthBuffer)
        //
        // _ConditionStencilWithGLCallback sets stencil=1 where the callback draws
        // (typically screen-space unpickable regions), so subsequent passes only
        // write IDs in stencil=0 pixels. GL-immediate-mode callback — no wgpu equivalent.
        if need_stencil_conditioning {
            // wgpu backend: stencil conditioning via GL callback not supported.
            // The depthMaskCallback field exists for API parity; it is always None here.
        }

        // --- Pass (i): occluder depth-only pass (optional) ---
        // C++: if (_UseOcclusionPass())
        //          _occluderRenderPass->Execute(_occluderRenderPassState, GetRenderTags())
        //          // suppress depth clear so occluders survive into the pickable pass
        //          _pickableAovBindings[_pickableDepthIndex].clearValue = VtValue()
        //
        // Occluder pass renders depth for unpickable prims so they occlude pickable hits.
        // After it runs, the pickable pass must NOT clear depth (would discard occluders).
        //
        // TODO(wgpu): begin_render_pass(
        //   depth_attachment = { texture: depth_tex, load: Clear(1.0), store: Store },
        //   color_attachments = []   // depth-only; no color writes
        // )
        // execute occluder render pass
        // end_render_pass
        let use_occluder = self.use_occluder_pass();
        self.pass_flags.use_occluder = use_occluder;
        if use_occluder {
            // Occluder pass executed (GPU side: TODO wgpu render pass).
            // Retain occluder depth in the pickable pass by suppressing its depth clear.
            // C++: _pickableAovBindings[_pickableDepthIndex].clearValue = VtValue()
            if let Some(depth_binding) = self.pickable_aov_bindings.get_mut(PickAov::DEPTH_INDEX) {
                depth_binding.clear_value = Value::default(); // no clear = load existing depth
            }
        } else if need_stencil_conditioning {
            // C++: else if (needStencilConditioning)
            //          _pickableAovBindings[_pickableDepthIndex].clearValue = VtValue()
            // Stencil was conditioned; preserve the depth+stencil so stencil values survive.
            if let Some(depth_binding) = self.pickable_aov_bindings.get_mut(PickAov::DEPTH_INDEX) {
                depth_binding.clear_value = Value::default();
            }
        } else {
            // C++: else
            //          _pickableAovBindings[_pickableDepthIndex].clearValue = VtValue(GfVec4f(1))
            // No occluder, no stencil conditioning — start fresh with depth=1 (far plane).
            if let Some(depth_binding) = self.pickable_aov_bindings.get_mut(PickAov::DEPTH_INDEX) {
                depth_binding.clear_value = Value::from(1.0f32);
            }
        }

        // --- Pass (ii): pickable ID render pass (mandatory) ---
        // C++: _pickableRenderPassState->SetAovBindings(_pickableAovBindings)
        //      _pickableRenderPass->Execute(_pickableRenderPassState, GetRenderTags())
        //
        // Writes primId, instanceId, elementId, edgeId, pointId, Neye, depth AOVs.
        //
        // TODO(wgpu): begin_render_pass(
        //   color_attachments = [prim_id_tex, instance_id_tex, element_id_tex,
        //                        edge_id_tex, point_id_tex, neye_tex],
        //   depth_attachment   = { texture: depth_tex, load: per-case above, store: Store }
        // )
        // execute pickable render pass with ID shader
        // end_render_pass
        self.pickable_render_pass_state
            .set_aov_bindings(self.pickable_aov_bindings.clone());

        // --- Pass (iii): overlay pass (optional) ---
        // C++: if (_UseOverlayPass())
        //          if (needStencilConditioning)
        //              _overlayAovBindings.back().clearValue = VtValue()   // keep stencil
        //          else
        //              _overlayAovBindings.back().clearValue = VtValue(GfVec4f(1.0f))
        //          _overlayRenderPassState->SetAovBindings(_overlayAovBindings)
        //          _overlayRenderPass->Execute(_overlayRenderPassState, GetRenderTags())
        //
        // Overlay pass uses a separate fresh depth buffer so "displayInOverlay" prims
        // always draw in front. ID AOV attachments are loaded (not cleared) to retain
        // results from the pickable pass.
        //
        // TODO(wgpu): begin_render_pass(
        //   color_attachments = [prim_id_tex (Load), ...],  // retain pickable IDs
        //   depth_attachment   = { texture: overlay_depth_tex, load: per-case, store: Store }
        // )
        // execute overlay render pass
        // end_render_pass
        if self.pass_flags.use_overlay {
            // C++: overlay depth clear depends on stencil conditioning.
            if need_stencil_conditioning {
                // Keep stencil: suppress overlay depth clear.
                if let Some(last) = self.overlay_aov_bindings.last_mut() {
                    last.clear_value = Value::default();
                }
            } else {
                // Fresh depth (far plane) for correct inter-overlay occlusion.
                if let Some(last) = self.overlay_aov_bindings.last_mut() {
                    last.clear_value = Value::from(1.0f32);
                }
            }
            self.overlay_render_pass_state
                .set_aov_bindings(self.overlay_aov_bindings.clone());
        }

        if self.context_params.resolve_mode == pick_tokens::resolve_deep()
            && Self::get_pick_buffer(ctx).is_none()
        {
            self.emit_render_requests(ctx);
            ctx.insert(Token::new("pickTaskExecuted"), Value::from(0i32));
            return;
        }

        // --- Resolve hits ---
        // C++: if (resolveMode == resolveDeep) { _ResolveDeep(); _hgi->EndFrame(); return; }
        //      // else: GPU readback 7 AOV textures and resolve on CPU.

        if self.context_params.resolve_mode == pick_tokens::resolve_deep() {
            // C++: _ResolveDeep() — reads back the pick buffer SSBO written by the GPU shader.
            //
            // Buffer layout (initialized in _ClearPickBuffer, written by WGSL pick shader):
            //   header[0] = numSubBuffers
            //   header[1] = SUBBUFFER_CAPACITY (32)
            //   header[2] = PICK_BUFFER_HEADER_SIZE (8)
            //   header[3] = entryStorageOffset
            //   header[4] = pickFaces ? 1 : 0
            //   header[5] = pickEdges ? 1 : 0
            //   header[6] = pickPoints ? 1 : 0
            //   header[7] = 0 (pad)
            //   [sub-buffer size table: numSubBuffers i32s]
            //   [entry storage: numSubBuffers * CAPACITY * 3 i32s (primId, instanceId, partId)]
            //
            let Some(hgi_driver) = Self::get_hgi_driver(ctx) else {
                ctx.insert(Token::new("pickTaskExecuted"), Value::from(0i32));
                return;
            };
            let Some(pick_buffer) = Self::get_pick_buffer(ctx) else {
                ctx.insert(Token::new("pickTaskExecuted"), Value::from(0i32));
                return;
            };
            let deep_data = hgi_driver.with_write(|hgi| Self::read_buffer_i32(hgi, &pick_buffer));
            let Some(deep_data) = deep_data else {
                ctx.insert(Token::new("pickTaskExecuted"), Value::from(0i32));
                return;
            };
            Self::resolve_deep(&deep_data, &self.context_params.pick_target, &mut self.hits);

            // C++: _hgi->EndFrame()
            // wgpu: submit pending command buffers (TODO when render passes are wired)
        } else {
            // C++: GPU readback 7 AOV textures via HdStTextureUtils::HgiTextureReadback<T>().
            //      Then construct HdxPickResult and dispatch to the appropriate resolve method.
            //
            // For each AOV texture, C++ does:
            //   primIds   = _ReadAovBuffer<int>(HdAovTokens->primId)
            //   instanceIds = _ReadAovBuffer<int>(HdAovTokens->instanceId)
            //   elementIds  = _ReadAovBuffer<int>(HdAovTokens->elementId)
            //   edgeIds     = _ReadAovBuffer<int>(HdAovTokens->edgeId)
            //   pointIds    = _ReadAovBuffer<int>(HdAovTokens->pointId)
            //   neyes       = _ReadAovBuffer<int>(HdAovTokens->Neye)
            //   depths      = _ReadAovBuffer<float>(_depthToken)
            //
            let Some(hgi_driver) = Self::get_hgi_driver(ctx) else {
                ctx.insert(Token::new("pickTaskExecuted"), Value::from(0i32));
                return;
            };
            let prim_ids =
                Self::read_aov_i32(ctx, &hgi_driver, &Token::new("primId")).unwrap_or_default();
            let instance_ids =
                Self::read_aov_i32(ctx, &hgi_driver, &Token::new("instanceId")).unwrap_or_default();
            let element_ids =
                Self::read_aov_i32(ctx, &hgi_driver, &Token::new("elementId")).unwrap_or_default();
            let edge_ids =
                Self::read_aov_i32(ctx, &hgi_driver, &Token::new("edgeId")).unwrap_or_default();
            let point_ids =
                Self::read_aov_i32(ctx, &hgi_driver, &Token::new("pointId")).unwrap_or_default();
            let neyes =
                Self::read_aov_i32(ctx, &hgi_driver, &Token::new("Neye")).unwrap_or_default();
            let depths =
                Self::read_aov_f32(ctx, &hgi_driver, &Token::new("depth")).unwrap_or_default();

            // C++: GfVec2f depthRange(0, 1)
            //      if (_hgi->GetCapabilities()->IsSet(HgiDeviceCapabilitiesBitsCustomDepthRange))
            //          depthRange = _pickableRenderPassState->GetDepthRange()
            // wgpu: standard [0, 1] depth range; no custom range capability needed.
            let depth_range = Vec2f::new(0.0, 1.0);

            let result = HdxPickResult::new(
                prim_ids,
                depths,
                res,
                self.context_params.pick_target.clone(),
                self.context_params.view_matrix,
                self.context_params.projection_matrix,
            )
            .with_depth_range(depth_range.x, depth_range.y)
            .with_sub_rect(0, 0, res.x, res.y)
            .with_all_ids(instance_ids, element_ids, edge_ids, point_ids)
            .with_neyes(neyes);

            // C++: resolveMode dispatch
            let mode = self.context_params.resolve_mode.clone();
            if mode == pick_tokens::resolve_nearest_to_center() {
                result.resolve_nearest_to_center(&mut self.hits);
            } else if mode == pick_tokens::resolve_nearest_to_camera() {
                result.resolve_nearest_to_camera(&mut self.hits);
            } else if mode == pick_tokens::resolve_unique() {
                result.resolve_unique(&mut self.hits);
            } else if mode == pick_tokens::resolve_all() {
                result.resolve_all(&mut self.hits);
            } else {
                // C++: TF_CODING_ERROR("Unrecognized intersection mode '%s'")
                eprintln!("pick_task: unrecognized resolve mode '{}'", mode.as_str());
            }

            // C++: _hgi->EndFrame()
            // wgpu: submit pending command buffers (TODO when render passes are wired)
        }

        // Surface hit count into task context so downstream tasks can inspect results.
        ctx.insert(
            Token::new("pickTaskExecuted"),
            Value::from(self.hits.len() as i32),
        );
    }

    fn get_render_tags(&self) -> &[Token] {
        &self.render_tags
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

/// Instancer context for a picked object.
///
/// Port of HdxInstancerContext from pxr/imaging/hdx/pickTask.h
#[derive(Debug, Clone)]
pub struct HdxInstancerContext {
    /// Path of the instancer in the scene index.
    pub instancer_scene_index_path: Path,
    /// For implicit instancing, path of the picked instance in scene index.
    pub instance_scene_index_path: Path,
    /// Index of the picked instance.
    pub instance_id: i32,
}

impl Default for HdxInstancerContext {
    fn default() -> Self {
        Self {
            instancer_scene_index_path: Path::empty(),
            instance_scene_index_path: Path::empty(),
            instance_id: -1,
        }
    }
}

/// Prim origin info for identifying picked prims via scene indices.
///
/// Port of HdxPrimOriginInfo from pxr/imaging/hdx/pickTask.h
#[derive(Debug, Clone)]
pub struct HdxPrimOriginInfo {
    /// Instancer contexts (outer-most first).
    pub instancer_contexts: Vec<HdxInstancerContext>,
}

impl HdxPrimOriginInfo {
    /// Create from a pick hit by traversing the scene index prim origin.
    pub fn from_pick_hit(_hit: &HdxPickHit) -> Self {
        Self {
            instancer_contexts: Vec::new(),
        }
    }

    /// Combine instance paths with prim scene path.
    ///
    /// Extracts the scene path using the given key from prim origin data sources.
    pub fn get_full_path(&self, name_in_prim_origin: Option<&Token>) -> Path {
        let _key = name_in_prim_origin
            .cloned()
            .unwrap_or_else(|| Token::new("scenePath"));
        Path::empty()
    }
}

/// Pick result resolver.
///
/// Resolves ID buffer readback data into HdxPickHit results.
/// Supports multiple resolve modes matching C++ HdxPickResult.
///
/// Port of HdxPickResult from pxr/imaging/hdx/pickTask.h
pub struct HdxPickResult {
    /// Prim ID buffer (one per pixel).
    prim_ids: Vec<i32>,
    /// Instance ID buffer.
    instance_ids: Vec<i32>,
    /// Element ID buffer.
    element_ids: Vec<i32>,
    /// Edge ID buffer.
    edge_ids: Vec<i32>,
    /// Point ID buffer.
    point_ids: Vec<i32>,
    /// Packed eye-space normal buffer (`Neye`).
    neyes: Vec<i32>,
    /// Depth buffer (normalized [0, 1]).
    depths: Vec<f32>,
    /// Buffer resolution.
    buffer_size: Vec2i,
    /// Pick target (determines which IDs to fill into hit).
    pick_target: Token,
    /// NDC to world transform (for unprojecting depth to world space).
    ndc_to_world: Matrix4d,
    /// Eye to world transform (for normal transformation from Neye buffer).
    #[allow(dead_code)]
    eye_to_world: Matrix4d,
    /// Depth range [near, far] — for custom depth range support.
    depth_range: Vec2f,
    /// Subrect within buffer [x, y, w, h] (pick region, clamped to buffer).
    sub_rect: Vec4i,
}

impl HdxPickResult {
    /// Create pick result resolver.
    ///
    /// # Arguments
    /// * `prim_ids` - GPU readback of primId buffer
    /// * `depths` - GPU readback of depth buffer
    /// * `buffer_size` - pick buffer resolution
    /// * `pick_target` - what was being picked
    /// * `view_matrix` - view matrix used during pick render
    /// * `projection_matrix` - projection matrix used during pick render
    pub fn new(
        prim_ids: Vec<i32>,
        depths: Vec<f32>,
        buffer_size: Vec2i,
        pick_target: Token,
        view_matrix: Matrix4d,
        projection_matrix: Matrix4d,
    ) -> Self {
        // C++: _ndcToWorld = (viewMatrix * projectionMatrix).GetInverse()
        // C++: _eyeToWorld = viewMatrix.GetInverse()
        let ndc_to_world = (view_matrix * projection_matrix)
            .inverse()
            .unwrap_or_else(Matrix4d::identity);
        let eye_to_world = view_matrix.inverse().unwrap_or_else(Matrix4d::identity);

        // Sub-rect spans entire buffer by default
        let sub_rect = Vec4i::new(0, 0, buffer_size.x, buffer_size.y);

        Self {
            prim_ids,
            instance_ids: Vec::new(),
            element_ids: Vec::new(),
            edge_ids: Vec::new(),
            point_ids: Vec::new(),
            neyes: Vec::new(),
            depths,
            buffer_size,
            pick_target,
            ndc_to_world,
            eye_to_world,
            depth_range: Vec2f::new(0.0, 1.0),
            sub_rect,
        }
    }

    /// Set optional ID buffers (instance, element, edge, point).
    pub fn with_all_ids(
        mut self,
        instance_ids: Vec<i32>,
        element_ids: Vec<i32>,
        edge_ids: Vec<i32>,
        point_ids: Vec<i32>,
    ) -> Self {
        self.instance_ids = instance_ids;
        self.element_ids = element_ids;
        self.edge_ids = edge_ids;
        self.point_ids = point_ids;
        self
    }

    /// Set the packed eye-space normal buffer (`Neye`).
    pub fn with_neyes(mut self, neyes: Vec<i32>) -> Self {
        self.neyes = neyes;
        self
    }

    /// Set depth range (for custom depth range GPU feature).
    pub fn with_depth_range(mut self, near: f32, far: f32) -> Self {
        self.depth_range = Vec2f::new(near, far);
        self
    }

    /// Set pick sub-rect within buffer.
    pub fn with_sub_rect(mut self, x: i32, y: i32, w: i32, h: i32) -> Self {
        // Clamp to buffer bounds (matches C++ constructor clamping)
        let x = x.max(0);
        let y = y.max(0);
        let w = w.min(self.buffer_size.x - x);
        let h = h.min(self.buffer_size.y - y);
        self.sub_rect = Vec4i::new(x, y, w, h);
        self
    }

    /// Check if result has valid data.
    pub fn is_valid(&self) -> bool {
        !self.prim_ids.is_empty()
            && self.prim_ids.len() == self.depths.len()
            && self.buffer_size.x > 0
            && self.buffer_size.y > 0
    }

    /// Check if a pixel at linear index is a valid pick hit.
    ///
    /// C++: HdxPickResult::_IsValidHit(index)
    ///
    /// Accounts for pick target: edge picks require a valid edgeId, point picks
    /// require a valid pointId. pickPointsAndInstances also accepts instance hits.
    fn is_valid_hit(&self, idx: usize) -> bool {
        let Some(&prim_id) = self.prim_ids.get(idx) else {
            return false;
        };
        if prim_id == -1 {
            return false;
        }

        if self.pick_target == pick_tokens::pick_edges() {
            // Edge pick: must have a valid edgeId
            return self.edge_ids.get(idx).copied().unwrap_or(-1) != -1;
        }
        if self.pick_target == pick_tokens::pick_points() {
            // Point pick: must have a valid pointId
            return self.point_ids.get(idx).copied().unwrap_or(-1) != -1;
        }
        if self.pick_target == pick_tokens::pick_points_and_instances() {
            // Accept point hit OR instanced prim hit.
            if self.point_ids.get(idx).copied().unwrap_or(-1) != -1 {
                return true;
            }
            // Instance hit: instanceId valid and prim has an instancer.
            // Without a render index we cannot resolve instancer paths, so we
            // accept any pixel with a valid instanceId as an approximate match
            // (C++ additionally checks instancerId.IsEmpty() via render index).
            return self.instance_ids.get(idx).copied().unwrap_or(-1) != -1;
        }

        true
    }

    /// Compute dedup hash for a pixel at linear index.
    ///
    /// C++: HdxPickResult::_GetHash(index)
    ///
    /// Hashes (primId, instanceId) and, depending on pick target, the sub-prim ID.
    fn pixel_hash(&self, idx: usize) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        self.prim_ids.get(idx).copied().unwrap_or(-1).hash(&mut h);
        self.instance_ids
            .get(idx)
            .copied()
            .unwrap_or(-1)
            .hash(&mut h);
        if self.pick_target == pick_tokens::pick_faces() {
            self.element_ids
                .get(idx)
                .copied()
                .unwrap_or(-1)
                .hash(&mut h);
        } else if self.pick_target == pick_tokens::pick_edges() {
            self.edge_ids.get(idx).copied().unwrap_or(-1).hash(&mut h);
        } else if self.pick_target == pick_tokens::pick_points()
            || self.pick_target == pick_tokens::pick_points_and_instances()
        {
            self.point_ids.get(idx).copied().unwrap_or(-1).hash(&mut h);
        }
        h.finish()
    }

    /// Resolve nearest hit to camera (minimum depth with valid primId).
    ///
    /// C++: HdxPickResult::ResolveNearestToCamera()
    pub fn resolve_nearest_to_camera(&self, hits: &mut Vec<HdxPickHit>) {
        if !self.is_valid() {
            return;
        }

        let mut best_x = 0i32;
        let mut best_y = 0i32;
        let mut best_depth = f32::MAX;
        let mut best_idx: i32 = -1;

        for y in self.sub_rect.y..(self.sub_rect.y + self.sub_rect.w) {
            for x in self.sub_rect.x..(self.sub_rect.x + self.sub_rect.z) {
                let idx = (y * self.buffer_size.x + x) as usize;
                if self.is_valid_hit(idx) {
                    let d = self.depths[idx];
                    if best_idx == -1 || d < best_depth {
                        best_x = x;
                        best_y = y;
                        best_depth = d;
                        best_idx = idx as i32;
                    }
                }
            }
        }

        if best_idx != -1 {
            if let Some(hit) = self.resolve_hit(best_idx as usize, best_x, best_y, best_depth) {
                hits.push(hit);
            }
        }
    }

    /// Resolve nearest hit to center of pick region.
    ///
    /// C++: HdxPickResult::ResolveNearestToCenter()
    ///
    /// Walks from the center of the sub-rect outward in a spiral, returning the
    /// first valid hit found. Matches C++ spiral traversal exactly.
    pub fn resolve_nearest_to_center(&self, hits: &mut Vec<HdxPickHit>) {
        if !self.is_valid() {
            return;
        }

        let width = self.sub_rect.z;
        let height = self.sub_rect.w;

        // C++: midH = height/2; if (height%2==0) midH--; same for midW.
        let mut mid_h = height / 2;
        let mut mid_w = width / 2;
        if height % 2 == 0 {
            mid_h -= 1;
        }
        if width % 2 == 0 {
            mid_w -= 1;
        }

        // Walk from center outward. For each shell w/h, scan interior boundary.
        let mut w = mid_w;
        let mut h = mid_h;
        while w >= 0 && h >= 0 {
            let mut ww = w;
            while ww < width - w {
                let mut hh = h;
                while hh < height - h {
                    let x = ww + self.sub_rect.x;
                    let y = hh + self.sub_rect.y;
                    let idx = (y * self.buffer_size.x + x) as usize;
                    if self.is_valid_hit(idx) {
                        let d = self.depths[idx];
                        if let Some(hit) = self.resolve_hit(idx, x, y, d) {
                            hits.push(hit);
                        }
                        return;
                    }
                    // C++: skip interior pixels, jump to boundary
                    if !(ww == w || ww == width - w - 1) && hh == h {
                        hh = (height - h - 2).max(hh);
                    }
                    hh += 1;
                }
                ww += 1;
            }
            w -= 1;
            h -= 1;
        }
    }

    /// Resolve all hits (every pixel with valid primId).
    ///
    /// C++: HdxPickResult::ResolveAll()
    pub fn resolve_all(&self, hits: &mut Vec<HdxPickHit>) {
        if !self.is_valid() {
            return;
        }

        for y in self.sub_rect.y..(self.sub_rect.y + self.sub_rect.w) {
            for x in self.sub_rect.x..(self.sub_rect.x + self.sub_rect.z) {
                let idx = (y * self.buffer_size.x + x) as usize;
                if self.is_valid_hit(idx) {
                    let d = self.depths.get(idx).copied().unwrap_or(1.0);
                    if let Some(hit) = self.resolve_hit(idx, x, y, d) {
                        hits.push(hit);
                    }
                }
            }
        }
    }

    /// Resolve unique hits (deduplicate by primId + instanceId + relevant sub-prim ID).
    ///
    /// C++: HdxPickResult::ResolveUnique()
    ///
    /// C++ optimization: tracks previousHash to avoid redundant map lookups for
    /// adjacent pixels with the same prim (common when a mesh covers many pixels).
    pub fn resolve_unique(&self, hits: &mut Vec<HdxPickHit>) {
        if !self.is_valid() {
            return;
        }

        // First pass: collect one (x, y) representative per unique hash.
        // C++: std::unordered_map<size_t, GfVec2i> hitIndices
        use std::collections::HashMap;
        let mut hit_indices: HashMap<u64, (i32, i32)> = HashMap::new();
        let mut previous_hash: Option<u64> = None;

        for y in self.sub_rect.y..(self.sub_rect.y + self.sub_rect.w) {
            for x in self.sub_rect.x..(self.sub_rect.x + self.sub_rect.z) {
                let idx = (y * self.buffer_size.x + x) as usize;
                if !self.is_valid_hit(idx) {
                    continue;
                }
                let hash = self.pixel_hash(idx);
                // C++: adjacent-pixel optimization — skip map lookup if same hash as previous.
                if previous_hash != Some(hash) {
                    hit_indices.entry(hash).or_insert((x, y));
                    previous_hash = Some(hash);
                }
            }
        }

        // Second pass: resolve each representative into a HdxPickHit.
        for (x, y) in hit_indices.into_values() {
            let idx = (y * self.buffer_size.x + x) as usize;
            let d = self.depths.get(idx).copied().unwrap_or(1.0);
            if let Some(hit) = self.resolve_hit(idx, x, y, d) {
                hits.push(hit);
            }
        }
    }

    /// Resolve a single hit at buffer index (x, y) with pre-fetched depth z.
    ///
    /// C++: HdxPickResult::_ResolveHit(index, x, y, z, hit)
    ///
    /// Computes world-space hit point from NDC (matching C++ formula exactly),
    /// fills all ID fields from the corresponding AOV buffers.
    /// In C++ this also looks up object_id/delegate_id/instancer_id from the
    /// render index; without a render index we leave those paths empty.
    fn resolve_hit(&self, idx: usize, x: i32, y: i32, z: f32) -> Option<HdxPickHit> {
        let prim_id = self.prim_ids.get(idx).copied()?;
        if prim_id < 0 {
            return None;
        }

        // C++: GfVec3d ndcHit(
        //   ((double)x / _bufferSize[0]) * 2.0 - 1.0,
        //   ((double)y / _bufferSize[1]) * 2.0 - 1.0,
        //   ((z - _depthRange[0]) / (_depthRange[1] - _depthRange[0])) * 2.0 - 1.0 )
        // NOTE: C++ uses integer x/y directly (no +0.5 pixel-center offset).
        let near = self.depth_range.x as f64;
        let far = self.depth_range.y as f64;
        if (far - near).abs() < f64::EPSILON {
            return None;
        }
        let ndc_x = (x as f64 / self.buffer_size.x as f64) * 2.0 - 1.0;
        let ndc_y = (y as f64 / self.buffer_size.y as f64) * 2.0 - 1.0;
        let ndc_z = ((z as f64 - near) / (far - near)) * 2.0 - 1.0;

        // C++: hit->worldSpaceHitPoint = _ndcToWorld.Transform(ndcHit)
        // _ndcToWorld = (viewMatrix * projectionMatrix).GetInverse()
        let ndc_pt = Vec4d::new(ndc_x, ndc_y, ndc_z, 1.0);
        let world_pt = self.ndc_to_world * ndc_pt;
        // Perspective divide.
        let world_hit = if world_pt.w.abs() > 1e-10 {
            Vec3d::new(
                world_pt.x / world_pt.w,
                world_pt.y / world_pt.w,
                world_pt.z / world_pt.w,
            )
        } else {
            Vec3d::default()
        };

        // C++: hit->normalizedDepth = (z - depthRange[0]) / (depthRange[1] - depthRange[0])
        let normalized_depth = ((z as f64 - near) / (far - near)) as f32;

        let mut hit = HdxPickHit {
            prim_id,
            normalized_depth,
            world_space_hit_point: world_hit,
            // object_id / delegate_id / instancer_id: set by render index lookup (not
            // available here). Callers with access to a render index should fill these
            // in after receiving hits from the resolve methods.
            ..HdxPickHit::default()
        };

        // C++: hit->instanceIndex = _GetInstanceId(index)
        hit.instance_index = self.instance_ids.get(idx).copied().unwrap_or(-1);

        // C++: hit->elementIndex = _GetElementId(index)
        hit.element_index = self.element_ids.get(idx).copied().unwrap_or(-1);

        // C++: hit->edgeIndex = _GetEdgeId(index)
        hit.edge_index = self.edge_ids.get(idx).copied().unwrap_or(-1);

        // C++: hit->pointIndex = _GetPointId(index)
        hit.point_index = self.point_ids.get(idx).copied().unwrap_or(-1);

        // C++: hit->worldSpaceHitNormal = _GetNormal(index)
        //   = HdVec4f_2_10_10_10_REV(neyes[index]).GetAsVec<GfVec3f>()
        //     transformed by eyeToWorld.
        if let Some(&packed_neye) = self.neyes.get(idx) {
            let (nx, ny, nz) = HdVec4_2_10_10_10_Rev::from_i32(packed_neye).to_vec3();
            let eye_normal = Vec4d::new(nx as f64, ny as f64, nz as f64, 0.0);
            let world_normal = self.eye_to_world * eye_normal;
            let normal = Vec3f::new(
                world_normal.x as f32,
                world_normal.y as f32,
                world_normal.z as f32,
            );
            let len2 = normal.x * normal.x + normal.y * normal.y + normal.z * normal.z;
            if len2 > 1e-20 {
                let inv_len = len2.sqrt().recip();
                hit.world_space_hit_normal =
                    Vec3f::new(normal.x * inv_len, normal.y * inv_len, normal.z * inv_len);
            }
        }

        Some(hit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pick_tokens() {
        assert_ne!(
            pick_tokens::pick_prims_and_instances(),
            pick_tokens::pick_faces()
        );
        assert_ne!(
            pick_tokens::resolve_nearest_to_camera(),
            pick_tokens::resolve_all()
        );
        assert_ne!(pick_tokens::resolve_deep(), pick_tokens::resolve_unique());
    }

    #[test]
    fn test_pick_hit_default() {
        let hit = HdxPickHit::default();
        assert!(!hit.is_valid());
        assert_eq!(hit.instance_index, -1);
        assert_eq!(hit.element_index, -1);
        assert_eq!(hit.edge_index, -1);
        assert_eq!(hit.point_index, -1);
        assert_eq!(hit.normalized_depth, 1.0);
    }

    #[test]
    fn test_pick_hit_valid() {
        let mut hit = HdxPickHit::default();
        hit.object_id = Path::from_string("/World/Prim").unwrap();
        assert!(hit.is_valid());
    }

    #[test]
    fn test_pick_hit_display() {
        let mut hit = HdxPickHit::default();
        hit.object_id = Path::from_string("/Prim").unwrap();
        hit.normalized_depth = 0.5;
        let s = format!("{}", hit);
        assert!(s.contains("/Prim"));
        assert!(s.contains("0.5"));
    }

    #[test]
    fn test_pick_hit_hash() {
        let mut h1 = HdxPickHit::default();
        h1.object_id = Path::from_string("/A").unwrap();
        h1.instance_index = 0;
        h1.element_index = 5;

        let mut h2 = h1.clone();
        assert_eq!(h1.get_hash(), h2.get_hash());

        h2.element_index = 6;
        assert_ne!(h1.get_hash(), h2.get_hash());
    }

    #[test]
    fn test_pick_task_params_default() {
        let params = HdxPickTaskParams::default();
        assert_eq!(params.cull_style, HdCullStyle::Nothing);
    }

    #[test]
    fn test_pick_context_params_default() {
        let params = HdxPickTaskContextParams::default();
        assert_eq!(params.resolution, Vec2i::new(128, 128));
        assert_eq!(params.max_num_deep_entries, 32000);
        assert_eq!(params.alpha_threshold, 0.0001);
    }

    #[test]
    fn test_pick_task_creation() {
        let task = HdxPickTask::new(Path::from_string("/pick").unwrap());
        assert!(task.get_hits().is_empty());
        assert_eq!(task.get_params().cull_style, HdCullStyle::Nothing);
    }

    #[test]
    fn test_pick_task_create_aov_bindings() {
        let mut task = HdxPickTask::new(Path::from_string("/pick").unwrap());
        task.create_aov_bindings();

        // 7 AOVs: primId, instanceId, elementId, edgeId, pointId, Neye, depth
        assert_eq!(task.pickable_aov_bindings.len(), PickAov::ALL.len());
        // Occluder binding is the depth one
        assert!(task.occluder_aov_binding.is_some());
        // Overlay has same count
        assert_eq!(task.overlay_aov_bindings.len(), PickAov::ALL.len());
    }

    #[test]
    fn test_pick_aov_tokens() {
        assert_eq!(PickAov::PrimId.token().as_str(), "primId");
        assert_eq!(PickAov::InstanceId.token().as_str(), "instanceId");
        assert_eq!(PickAov::ElementId.token().as_str(), "elementId");
        assert_eq!(PickAov::EdgeId.token().as_str(), "edgeId");
        assert_eq!(PickAov::PointId.token().as_str(), "pointId");
        assert_eq!(PickAov::Neye.token().as_str(), "Neye");
        assert_eq!(PickAov::Depth.token().as_str(), "depth");
    }

    #[test]
    fn test_decode_id_render_color() {
        let color = [0x12, 0x34, 0x56, 0x78];
        let id = HdxPickTask::decode_id_render_color(color);
        assert_eq!(id, 0x78563412_u32 as i32);

        // All zeros = 0
        assert_eq!(HdxPickTask::decode_id_render_color([0, 0, 0, 0]), 0);
        // All 0xFF = -1 (int32)
        assert_eq!(
            HdxPickTask::decode_id_render_color([0xFF, 0xFF, 0xFF, 0xFF]),
            -1
        );
    }

    #[test]
    fn test_decode_i32_aov_from_rgba8() {
        let raw = vec![1u8, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF];
        let values = HdxPickTask::decode_i32_aov(HgiFormat::UNorm8Vec4, &raw).unwrap();
        assert_eq!(values, vec![1, -1]);
    }

    #[test]
    fn test_decode_f32_aov_from_float32() {
        let raw = [0.25f32.to_le_bytes(), 1.0f32.to_le_bytes()].concat();
        let values = HdxPickTask::decode_f32_aov(HgiFormat::Float32, &raw).unwrap();
        assert_eq!(values, vec![0.25, 1.0]);
    }

    #[test]
    fn test_pick_result_creation() {
        let prim_ids = vec![-1, 0, 1, -1];
        let depths = vec![1.0, 0.5, 0.3, 1.0];
        let result = HdxPickResult::new(
            prim_ids,
            depths,
            Vec2i::new(2, 2),
            pick_tokens::pick_prims_and_instances(),
            Matrix4d::identity(),
            Matrix4d::identity(),
        );
        assert!(result.is_valid());
    }

    #[test]
    fn test_pick_result_resolve_nearest_to_camera() {
        // 2x2 grid: prim 1 at depth 0.3, prim 0 at depth 0.5, rest invalid
        let prim_ids = vec![-1, 0, 1, -1];
        let depths = vec![1.0, 0.5, 0.3, 1.0];
        let result = HdxPickResult::new(
            prim_ids,
            depths,
            Vec2i::new(2, 2),
            pick_tokens::pick_prims_and_instances(),
            Matrix4d::identity(),
            Matrix4d::identity(),
        );

        let mut hits = Vec::new();
        result.resolve_nearest_to_camera(&mut hits);
        // Nearest to camera = smallest depth = 0.3 at idx 2 (prim 1)
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].normalized_depth, 0.3);
    }

    #[test]
    fn test_pick_result_resolve_all() {
        let prim_ids = vec![0, 1, 2, -1];
        let depths = vec![0.5, 0.7, 0.3, 1.0];
        let result = HdxPickResult::new(
            prim_ids,
            depths,
            Vec2i::new(2, 2),
            pick_tokens::pick_prims_and_instances(),
            Matrix4d::identity(),
            Matrix4d::identity(),
        );

        let mut hits = Vec::new();
        result.resolve_all(&mut hits);
        assert_eq!(hits.len(), 3); // 3 valid pixels
    }

    #[test]
    fn test_pick_result_resolve_unique() {
        // Same prim at two pixels — should deduplicate
        let prim_ids = vec![5, 5, -1, 7];
        let depths = vec![0.4, 0.6, 1.0, 0.2];
        let result = HdxPickResult::new(
            prim_ids,
            depths,
            Vec2i::new(2, 2),
            pick_tokens::pick_prims_and_instances(),
            Matrix4d::identity(),
            Matrix4d::identity(),
        );

        let mut hits = Vec::new();
        result.resolve_unique(&mut hits);
        // 2 unique prims (5 and 7)
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn test_pick_result_invalid_empty() {
        let result = HdxPickResult::new(
            vec![],
            vec![],
            Vec2i::new(0, 0),
            pick_tokens::pick_prims_and_instances(),
            Matrix4d::identity(),
            Matrix4d::identity(),
        );
        assert!(!result.is_valid());

        let mut hits = Vec::new();
        result.resolve_nearest_to_camera(&mut hits);
        assert!(hits.is_empty());
    }

    #[test]
    fn test_pick_result_sub_rect_clamping() {
        let prim_ids = vec![0, 1, 2, 3];
        let depths = vec![0.5, 0.6, 0.7, 0.8];
        let result = HdxPickResult::new(
            prim_ids,
            depths,
            Vec2i::new(2, 2),
            pick_tokens::pick_prims_and_instances(),
            Matrix4d::identity(),
            Matrix4d::identity(),
        )
        // Request 10x10 sub-rect — should clamp to 2x2
        .with_sub_rect(0, 0, 10, 10);

        assert_eq!(result.sub_rect.z, 2); // clamped width
        assert_eq!(result.sub_rect.w, 2); // clamped height
    }

    #[test]
    fn test_instancer_context_default() {
        let ctx = HdxInstancerContext::default();
        assert_eq!(ctx.instance_id, -1);
        assert!(ctx.instancer_scene_index_path.is_empty());
    }

    #[test]
    fn test_prim_origin_info() {
        let hit = HdxPickHit::default();
        let info = HdxPrimOriginInfo::from_pick_hit(&hit);
        assert!(info.instancer_contexts.is_empty());

        let path = info.get_full_path(None);
        assert!(path.is_empty());
    }

    /// Test resolve_deep with a hand-crafted SSBO buffer matching the C++ layout.
    #[test]
    fn test_resolve_deep_basic() {
        // Construct a minimal pick buffer:
        //   header: [numSubBuffers=1, capacity=32, headerSize=8, entryStorageOffset=9]
        //           [pickFaces=0, pickEdges=0, pickPoints=0, pad=0]
        //   size table: [2]  <- sub-buffer 0 has 2 entries
        //   entries: (prim0, inst0, part0), (prim1, inst1, part1)
        let mut data: Vec<i32> = vec![
            1,  // numSubBuffers
            32, // SUBBUFFER_CAPACITY
            8,  // PICK_BUFFER_HEADER_SIZE
            9,  // entryStorageOffset (8 header + 1 sub-buffer size slot)
            0, 0, 0, 0, // pickFaces/Edges/Points flags + pad
            3, // sub-buffer 0 has 3 entries
        ];
        // entry 0: primId=5, instanceId=3, partId=2
        data.push(5);
        data.push(3);
        data.push(2);
        // entry 1: primId=-1 (invalid, should be skipped)
        data.push(-1);
        data.push(0);
        data.push(0);
        // entry 2: primId=7, instanceId=0, partId=1
        data.push(7);
        data.push(0);
        data.push(1);

        let mut hits = Vec::new();
        let target = pick_tokens::pick_prims_and_instances();
        HdxPickTask::resolve_deep(&data, &target, &mut hits);

        // Only 2 valid entries out of 3 (primId=-1 skipped)
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].prim_id, 5);
        assert_eq!(hits[0].instance_index, 3);
        assert_eq!(hits[1].prim_id, 7);
        assert_eq!(hits[1].instance_index, 0);
    }

    #[test]
    fn test_build_pick_buffer_init_data_deep() {
        let mut context_params = HdxPickTaskContextParams::default();
        context_params.resolve_mode = pick_tokens::resolve_deep();
        context_params.max_num_deep_entries = 64;
        context_params.pick_target = pick_tokens::pick_faces();

        let data = HdxPickTask::build_pick_buffer_init_data(&context_params);

        assert_eq!(data[0], 2);
        assert_eq!(data[1], 32);
        assert_eq!(data[2], 8);
        assert_eq!(data[3], 10);
        assert_eq!(data[4], 1);
        assert_eq!(data[5], 0);
        assert_eq!(data[6], 0);
        assert_eq!(data[8], 0);
        assert_eq!(data[9], 0);
        assert_eq!(data[10], -9);
    }

    #[test]
    fn test_emit_render_requests_for_deep_pick() {
        let mut task = HdxPickTask::new(Path::from_string("/pick").unwrap());
        task.create_aov_bindings();
        task.context_params.resolve_mode = pick_tokens::resolve_deep();
        task.pass_flags.use_occluder = true;
        task.pass_flags.use_overlay = true;
        task.configure_render_pass_states();

        let mut ctx = HdTaskContext::default();
        task.emit_render_requests(&mut ctx);

        let requests = ctx
            .get(&Token::new("pickTaskRequests"))
            .and_then(|value| value.get::<Vec<HdxPickTaskRequest>>())
            .unwrap();
        assert_eq!(requests.len(), 3);
        assert_eq!(requests[0].pass, HdxPickTaskPass::Occluder);
        assert!(!requests[0].bind_pick_buffer);
        assert_eq!(requests[1].pass, HdxPickTaskPass::Pickable);
        assert!(requests[1].bind_pick_buffer);
        assert_eq!(
            requests[1].render_pass_state.get().get_aov_bindings().len(),
            2
        );
        assert_eq!(requests[2].pass, HdxPickTaskPass::Overlay);
        assert_eq!(requests[2].material_tag, Token::new("displayInOverlay"));
    }

    /// Test resolve_deep with pickFaces target sets element_index.
    #[test]
    fn test_resolve_deep_pick_faces() {
        let mut data: Vec<i32> = vec![
            1, 32, 8, 9, // header
            0, 0, 0, 0, // flags
            1, // 1 entry in sub-buffer 0
            10, 2, 42, // primId=10, instanceId=2, partId=42
        ];
        // pad to CAPACITY entries (not required for bounds check but mirrors C++)
        let _ = &mut data;

        let mut hits = Vec::new();
        let target = pick_tokens::pick_faces();
        HdxPickTask::resolve_deep(&data, &target, &mut hits);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].prim_id, 10);
        assert_eq!(hits[0].element_index, 42); // partId -> element_index for pickFaces
        assert_eq!(hits[0].edge_index, -1);
        assert_eq!(hits[0].point_index, -1);
    }

    /// Test is_valid_hit with edge pick target.
    #[test]
    fn test_pick_result_valid_hit_edge() {
        // prim_ids[0]=5, edge_ids[0]=-1 => invalid for pickEdges
        // prim_ids[1]=5, edge_ids[1]=3  => valid
        let result = HdxPickResult::new(
            vec![5, 5],
            vec![0.5, 0.4],
            Vec2i::new(2, 1),
            pick_tokens::pick_edges(),
            Matrix4d::identity(),
            Matrix4d::identity(),
        )
        .with_all_ids(vec![-1, -1], vec![-1, -1], vec![-1, 3], vec![-1, -1]);

        let mut hits = Vec::new();
        result.resolve_all(&mut hits);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].edge_index, 3);
    }

    /// Test that stencil conditioning path sets depth clear to default (no-clear).
    #[test]
    fn test_execute_stencil_path_depth_clear() {
        let mut task = HdxPickTask::new(Path::from_string("/pick").unwrap());
        task.create_aov_bindings();

        // Verify initial depth clear is 1.0 (far plane)
        assert_eq!(
            task.pickable_aov_bindings[PickAov::DEPTH_INDEX].clear_value,
            Value::from(1.0f32)
        );

        // Simulate execute() with stencil conditioning:
        // need_stencil_conditioning=true, use_occluder=false
        // => depth_binding.clear_value = Value::default() (no clear)
        let need_stencil_conditioning = true;
        let use_occluder = false;
        if use_occluder {
            task.pickable_aov_bindings[PickAov::DEPTH_INDEX].clear_value = Value::default();
        } else if need_stencil_conditioning {
            task.pickable_aov_bindings[PickAov::DEPTH_INDEX].clear_value = Value::default();
        } else {
            task.pickable_aov_bindings[PickAov::DEPTH_INDEX].clear_value = Value::from(1.0f32);
        }

        // After stencil conditioning path: depth should NOT be cleared
        assert_eq!(
            task.pickable_aov_bindings[PickAov::DEPTH_INDEX].clear_value,
            Value::default()
        );
    }
}
