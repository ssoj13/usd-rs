//! Hydra primitive (prim) types.
//!
//! Prims are the fundamental scene objects in Hydra. There are three main categories:
//!
//! - **Rprims** (Renderable Prims): Geometry that can be rendered (mesh, curves, points)
//! - **Sprims** (State Prims): Rendering state objects (camera, light, material)
//! - **Bprims** (Buffer Prims): Buffer objects (render buffers, textures)
//!
//! # Architecture
//!
//! Each prim type follows a pull-based architecture:
//! - Prims have a unique `SdfPath` identifier
//! - Change tracking via `HdDirtyBits` flags
//! - `Sync()` method called by render index to update GPU resources
//! - `HdSceneDelegate` provides data in legacy mode
//!
//! # Example
//!
//! ```ignore
//! use usd_hd::prim::*;
//! use usd_sdf::Path;
//!
//! // Create a mesh prim
//! let id = Path::from_string("/World/Mesh").unwrap();
//! let mesh = HdMesh::new(id, None);
//!
//! // Check dirty bits
//! let dirty = mesh.get_initial_dirty_bits_mask();
//! println!("Initial dirty bits: 0x{:x}", dirty);
//! ```

use parking_lot::RwLock;

use crate::HdExtComputationContext;
use crate::enums::HdCullStyle;
use crate::scene_delegate::{
    HdDisplayStyle, HdExtComputationInputDescriptorVector, HdExtComputationOutputDescriptorVector,
    HdExtComputationPrimvarDescriptorVector, HdIdVectorSharedPtr, HdInstancerContext,
    HdModelDrawMode, HdPrimvarDescriptorVector, HdRenderBufferDescriptor,
    HdVolumeFieldDescriptorVector,
};
use crate::tokens::RENDER_TAG_GEOMETRY;
use crate::types::HdDirtyBits;
use usd_gf::Matrix4d;
use usd_px_osd::SubdivTags;
use usd_sdf::Path as SdfPath;

// Forward declarations for types that will be implemented later
// Note: Forward declarations for types in other modules.

// Re-export canonical HdRenderParam from render_delegate (C++ parity).
// Prims reference this trait for Sync/Finalize render param arguments.
pub use crate::render::render_delegate::HdRenderParam;

/// Repr descriptor for mesh draw items.
///
/// Configures how a mesh draw item should be rendered within a repr.
/// Corresponds to C++ `HdMeshReprDesc`.
#[derive(Debug, Clone)]
pub struct HdMeshReprDesc {
    /// Geometry rendering style (surface, edges, points, hull, etc.).
    pub geom_style: crate::enums::HdMeshGeomStyle,
    /// Face culling style.
    pub cull_style: HdCullStyle,
    /// Shading terminal token (e.g. surfaceShader, constantColor).
    pub shading_terminal: usd_tf::Token,
    /// Use flat shading.
    pub flat_shading_enabled: bool,
    /// Blend wireframe color into surface.
    pub blend_wireframe_color: bool,
    /// Force opaque edges (ignore opacity for edges).
    pub force_opaque_edges: bool,
    /// Generate edge ids for non-edge geom styles (for picking).
    pub surface_edge_ids: bool,
    /// Force double-sided rendering for this repr.
    pub double_sided: bool,
    /// Line width in pixels.
    pub line_width: f32,
    /// Use displacement shader.
    pub use_custom_displacement: bool,
    /// Allow scalar override visualization.
    pub enable_scalar_override: bool,
}

impl Default for HdMeshReprDesc {
    fn default() -> Self {
        Self {
            geom_style: crate::enums::HdMeshGeomStyle::Invalid,
            cull_style: HdCullStyle::DontCare,
            shading_terminal: usd_tf::Token::new("surfaceShader"),
            flat_shading_enabled: false,
            blend_wireframe_color: true,
            force_opaque_edges: true,
            surface_edge_ids: false,
            double_sided: false,
            line_width: 0.0,
            use_custom_displacement: true,
            enable_scalar_override: true,
        }
    }
}

impl HdMeshReprDesc {
    /// Returns true if this descriptor is empty/invalid.
    pub fn is_empty(&self) -> bool {
        self.geom_style == crate::enums::HdMeshGeomStyle::Invalid
    }
}

/// Repr descriptor for basis curves draw items.
///
/// Corresponds to C++ `HdBasisCurvesReprDesc`.
#[derive(Debug, Clone)]
pub struct HdBasisCurvesReprDesc {
    /// Geometry rendering style (wire, patch, points).
    pub geom_style: crate::enums::HdBasisCurvesGeomStyle,
    /// Shading terminal token.
    pub shading_terminal: usd_tf::Token,
}

impl Default for HdBasisCurvesReprDesc {
    fn default() -> Self {
        Self {
            geom_style: crate::enums::HdBasisCurvesGeomStyle::Invalid,
            shading_terminal: usd_tf::Token::new("surfaceShader"),
        }
    }
}

impl HdBasisCurvesReprDesc {
    /// Returns true if this descriptor is empty/invalid.
    pub fn is_empty(&self) -> bool {
        self.geom_style == crate::enums::HdBasisCurvesGeomStyle::Invalid
    }
}

/// Repr descriptor for points draw items.
///
/// Corresponds to C++ `HdPointsReprDesc`.
#[derive(Debug, Clone)]
pub struct HdPointsReprDesc {
    /// Geometry rendering style.
    pub geom_style: crate::enums::HdPointsGeomStyle,
}

impl Default for HdPointsReprDesc {
    fn default() -> Self {
        Self {
            geom_style: crate::enums::HdPointsGeomStyle::Invalid,
        }
    }
}

impl HdPointsReprDesc {
    /// Returns true if this descriptor is empty/invalid.
    pub fn is_empty(&self) -> bool {
        self.geom_style == crate::enums::HdPointsGeomStyle::Invalid
    }
}

/// Thread-safe repr descriptor config storage.
///
/// Maps repr names (tokens) to arrays of descriptors. Used by prim types
/// to configure how each repr should be drawn.
/// Port of C++ `_ReprDescConfigs<DESC_TYPE, N>` from rprim.h.
pub struct ReprDescConfigs<D: Clone, const N: usize> {
    configs: RwLock<Vec<(usd_tf::Token, [D; N])>>,
}

impl<D: Clone, const N: usize> ReprDescConfigs<D, N> {
    /// Create empty config storage.
    pub fn new() -> Self {
        Self {
            configs: RwLock::new(Vec::new()),
        }
    }

    /// Add or update repr descriptor array for the given name.
    pub fn add_or_update(&self, name: &usd_tf::Token, descs: [D; N]) {
        let mut configs = self.configs.write();
        for config in configs.iter_mut() {
            if &config.0 == name {
                config.1 = descs;
                return;
            }
        }
        configs.push((name.clone(), descs));
    }

    /// Find repr descriptor array by name. Returns None if not found.
    pub fn find(&self, name: &usd_tf::Token) -> Option<[D; N]> {
        let configs = self.configs.read();
        for config in configs.iter() {
            if &config.0 == name {
                return Some(config.1.clone());
            }
        }
        None
    }
}

/// Representation selector for choosing geometry style.
///
/// Describes one or more authored display representations for an rprim.
/// Supports up to 3 topology tokens: refined, unrefined, points.
///
/// Port of pxr/imaging/hd/repr.h
#[derive(Debug, Clone)]
pub struct HdReprSelector {
    /// Refined (subdivided) surface token.
    pub refined_token: usd_tf::Token,
    /// Unrefined (hull) token.
    pub unrefined_token: usd_tf::Token,
    /// Points token.
    pub points_token: usd_tf::Token,
}

impl Default for HdReprSelector {
    fn default() -> Self {
        Self {
            refined_token: usd_tf::Token::default(),
            unrefined_token: usd_tf::Token::default(),
            points_token: usd_tf::Token::default(),
        }
    }
}

impl HdReprSelector {
    /// Max topology representations.
    pub const MAX_TOPOLOGY_REPRS: usize = 3;

    /// Create empty repr selector.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create repr selector with single refined token.
    pub fn with_token(token: usd_tf::Token) -> Self {
        Self {
            refined_token: token.clone(),
            unrefined_token: usd_tf::Token::default(),
            points_token: usd_tf::Token::default(),
        }
    }

    /// Create repr selector with refined and unrefined tokens.
    pub fn with_refined_unrefined(refined: usd_tf::Token, unrefined: usd_tf::Token) -> Self {
        Self {
            refined_token: refined,
            unrefined_token: unrefined,
            points_token: usd_tf::Token::default(),
        }
    }

    /// Create repr selector with all three topology tokens.
    pub fn with_tokens(
        refined: usd_tf::Token,
        unrefined: usd_tf::Token,
        points: usd_tf::Token,
    ) -> Self {
        Self {
            refined_token: refined,
            unrefined_token: unrefined,
            points_token: points,
        }
    }

    /// Returns true if repr_token is in the set.
    pub fn contains(&self, repr_token: &usd_tf::Token) -> bool {
        repr_token == &self.refined_token
            || repr_token == &self.unrefined_token
            || repr_token == &self.points_token
    }

    /// Returns true if topology token at index is active (not empty nor disabled).
    pub fn is_active_repr(&self, topology_index: usize) -> bool {
        if topology_index >= Self::MAX_TOPOLOGY_REPRS {
            return false;
        }
        let t = self.get_token(topology_index);
        !t.is_empty() && t != "disabled"
    }

    /// Returns true if any topology token is active.
    pub fn any_active_repr(&self) -> bool {
        (0..Self::MAX_TOPOLOGY_REPRS).any(|i| self.is_active_repr(i))
    }

    /// Composite this over `under`. For each empty token, use under's token.
    pub fn composite_over(&self, under: &Self) -> Self {
        fn has_opinion(t: &usd_tf::Token) -> bool {
            !t.as_str().is_empty()
        }
        Self {
            refined_token: if has_opinion(&self.refined_token) {
                self.refined_token.clone()
            } else {
                under.refined_token.clone()
            },
            unrefined_token: if has_opinion(&self.unrefined_token) {
                self.unrefined_token.clone()
            } else {
                under.unrefined_token.clone()
            },
            points_token: if has_opinion(&self.points_token) {
                self.points_token.clone()
            } else {
                under.points_token.clone()
            },
        }
    }

    /// Compute hash from all three topology tokens.
    pub fn hash(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        Hash::hash(&self.refined_token, &mut hasher);
        Hash::hash(&self.unrefined_token, &mut hasher);
        Hash::hash(&self.points_token, &mut hasher);
        hasher.finish()
    }

    /// Text of the refined token (primary representation).
    pub fn get_text(&self) -> &str {
        self.refined_token.as_str()
    }

    /// Get topology token by index (0=refined, 1=unrefined, 2=points).
    pub fn get_token(&self, topology_index: usize) -> &usd_tf::Token {
        match topology_index {
            0 => &self.refined_token,
            1 => &self.unrefined_token,
            2 => &self.points_token,
            _ => &self.refined_token,
        }
    }

    /// Index-based access returning Option (None for out-of-bounds).
    pub fn get(&self, topology_index: usize) -> Option<&usd_tf::Token> {
        match topology_index {
            0 => Some(&self.refined_token),
            1 => Some(&self.unrefined_token),
            2 => Some(&self.points_token),
            _ => None,
        }
    }
}

impl PartialEq for HdReprSelector {
    fn eq(&self, other: &Self) -> bool {
        self.refined_token == other.refined_token
            && self.unrefined_token == other.unrefined_token
            && self.points_token == other.points_token
    }
}

impl Eq for HdReprSelector {}

impl std::hash::Hash for HdReprSelector {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&self.refined_token, state);
        std::hash::Hash::hash(&self.unrefined_token, state);
        std::hash::Hash::hash(&self.points_token, state);
    }
}

impl PartialOrd for HdReprSelector {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HdReprSelector {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.refined_token
            .cmp(&other.refined_token)
            .then_with(|| self.unrefined_token.cmp(&other.unrefined_token))
            .then_with(|| self.points_token.cmp(&other.points_token))
    }
}

/// Scene delegate interface for accessing scene data.
///
/// The scene delegate provides a pull-based interface for render delegates
/// to query scene data. This is the legacy Hydra 1.0 API.
///
/// Modern Hydra 2.0 uses scene index and data sources instead.
///
/// Rprim methods (get_transform, get_extent, etc.) have default implementations
/// returning sensible fallbacks. Override them to provide actual scene data.
///
/// `Send + Sync` is required for multi-threaded parallel rprim sync (C++ parity).
pub trait HdSceneDelegate: Send + Sync {
    /// Get dirty bits for a prim.
    fn get_dirty_bits(&self, id: &SdfPath) -> HdDirtyBits;

    /// Mark prim as clean.
    fn mark_clean(&mut self, id: &SdfPath, bits: HdDirtyBits);

    /// Get parent instancer id for this prim.
    ///
    /// Returns `SdfPath::default()` (empty path) if the prim has no instancer,
    /// matching C++ `GetInstancerId` which returns an empty path for non-instanced prims.
    fn get_instancer_id(&self, prim_id: &SdfPath) -> SdfPath {
        let _ = prim_id;
        SdfPath::default()
    }

    // ----------------------------------------------------------------------- //
    // Rprim aspects (default impls for parity with C++ HdSceneDelegate)
    // ----------------------------------------------------------------------- //

    /// Get object-space transform (includes parent transforms).
    /// Default: identity matrix.
    fn get_transform(&self, _id: &SdfPath) -> usd_gf::Matrix4d {
        usd_gf::Matrix4d::identity()
    }

    /// Get axis-aligned bounds in local space.
    /// Default: empty range.
    fn get_extent(&self, _id: &SdfPath) -> usd_gf::Range3d {
        usd_gf::Range3d::empty()
    }

    /// Get visibility state. Default: visible.
    fn get_visible(&self, _id: &SdfPath) -> bool {
        true
    }

    /// Get double-sided flag. Default: false (C++ parity).
    fn get_double_sided(&self, _id: &SdfPath) -> bool {
        false
    }

    /// Get mesh topology for mesh prims. Default: empty topology.
    fn get_mesh_topology(&self, _id: &SdfPath) -> mesh::HdMeshTopology {
        mesh::HdMeshTopology::new()
    }

    /// Get named value. Default: empty value.
    fn get(&self, _id: &SdfPath, _key: &usd_tf::Token) -> usd_vt::Value {
        usd_vt::Value::default()
    }

    /// Get material id for prim. Default: None.
    fn get_material_id(&self, _id: &SdfPath) -> Option<SdfPath> {
        None
    }

    // ----------------------------------------------------------------------- //
    // Core / Options (C++ HdSceneDelegate)
    // ----------------------------------------------------------------------- //

    /// Delegate ID used as prefix for all objects. Default: absolute root.
    fn get_delegate_id(&self) -> SdfPath {
        SdfPath::absolute_root()
    }

    /// Synchronize delegate state for the given request. Default: no-op.
    fn sync(&mut self, _request: &mut crate::HdSyncRequestVector) {}

    /// Cleanup after parallel sync work. Default: no-op.
    fn post_sync_cleanup(&mut self) {}

    /// Returns true if the named option is enabled. Default: parallelRprimSync true, else false.
    fn is_enabled(&self, option: &usd_tf::Token) -> bool {
        option == "parallelRprimSync"
    }

    // ----------------------------------------------------------------------- //
    // Rprim aspects (continued)
    // ----------------------------------------------------------------------- //

    /// Get basis curves topology. Default: empty.
    fn get_basis_curves_topology(&self, _id: &SdfPath) -> basis_curves::HdBasisCurvesTopology {
        basis_curves::HdBasisCurvesTopology::default()
    }

    /// Get subdivision surface tags. Default: empty.
    fn get_subdiv_tags(&self, _id: &SdfPath) -> SubdivTags {
        SubdivTags::default()
    }

    /// Get cull style. Default: DontCare.
    fn get_cull_style(&self, _id: &SdfPath) -> HdCullStyle {
        HdCullStyle::DontCare
    }

    /// Get shading style. Default: empty value.
    fn get_shading_style(&self, _id: &SdfPath) -> usd_vt::Value {
        usd_vt::Value::default()
    }

    /// Get display style. Default: default struct.
    fn get_display_style(&self, _id: &SdfPath) -> HdDisplayStyle {
        HdDisplayStyle::default()
    }

    /// Get indexed primvar. Returns (value, indices). Default: (empty, None).
    fn get_indexed_primvar(
        &self,
        _id: &SdfPath,
        _key: &usd_tf::Token,
    ) -> (usd_vt::Value, Option<Vec<i32>>) {
        (usd_vt::Value::default(), None)
    }

    /// Get repr selector. Default: default.
    fn get_repr_selector(&self, _id: &SdfPath) -> HdReprSelector {
        HdReprSelector::default()
    }

    /// Get render tag for bucketing. Default: geometry.
    fn get_render_tag(&self, _id: &SdfPath) -> usd_tf::Token {
        (*RENDER_TAG_GEOMETRY).clone()
    }

    /// Get prim categories. Default: empty.
    fn get_categories(&self, _id: &SdfPath) -> Vec<usd_tf::Token> {
        Vec::new()
    }

    /// Get instance categories per instance. Default: empty.
    fn get_instance_categories(&self, _instancer_id: &SdfPath) -> Vec<Vec<usd_tf::Token>> {
        Vec::new()
    }

    /// Get coordinate system bindings. Default: None.
    fn get_coord_sys_bindings(&self, _id: &SdfPath) -> Option<HdIdVectorSharedPtr> {
        None
    }

    /// Get model draw mode. Default: default struct.
    fn get_model_draw_mode(&self, _id: &SdfPath) -> HdModelDrawMode {
        HdModelDrawMode::default()
    }

    // ----------------------------------------------------------------------- //
    // Motion samples
    // ----------------------------------------------------------------------- //

    /// Sample transform over time. Default: single sample at t=0 with GetTransform.
    fn sample_transform(&self, id: &SdfPath, max_sample_count: usize) -> Vec<(f32, Matrix4d)> {
        if max_sample_count > 0 {
            vec![(0.0, self.get_transform(id))]
        } else {
            Vec::new()
        }
    }

    /// Sample transform over interval. Default: delegates to sample_transform.
    fn sample_transform_interval(
        &self,
        id: &SdfPath,
        _start_time: f32,
        _end_time: f32,
        max_sample_count: usize,
    ) -> Vec<(f32, Matrix4d)> {
        self.sample_transform(id, max_sample_count)
    }

    /// Sample primvar over time. Default: single sample at t=0 with Get.
    fn sample_primvar(
        &self,
        id: &SdfPath,
        key: &usd_tf::Token,
        max_sample_count: usize,
    ) -> Vec<(f32, usd_vt::Value)> {
        if max_sample_count > 0 {
            vec![(0.0, self.get(id, key))]
        } else {
            Vec::new()
        }
    }

    /// Sample primvar over interval. Default: delegates to sample_primvar.
    fn sample_primvar_interval(
        &self,
        id: &SdfPath,
        key: &usd_tf::Token,
        _start_time: f32,
        _end_time: f32,
        max_sample_count: usize,
    ) -> Vec<(f32, usd_vt::Value)> {
        self.sample_primvar(id, key, max_sample_count)
    }

    /// Sample indexed primvar. Default: single sample with GetIndexedPrimvar.
    fn sample_indexed_primvar(
        &self,
        id: &SdfPath,
        key: &usd_tf::Token,
        max_sample_count: usize,
    ) -> Vec<(f32, usd_vt::Value, Option<Vec<i32>>)> {
        if max_sample_count > 0 {
            let (val, indices) = self.get_indexed_primvar(id, key);
            vec![(0.0, val, indices)]
        } else {
            Vec::new()
        }
    }

    /// Sample indexed primvar over interval.
    fn sample_indexed_primvar_interval(
        &self,
        id: &SdfPath,
        key: &usd_tf::Token,
        _start_time: f32,
        _end_time: f32,
        max_sample_count: usize,
    ) -> Vec<(f32, usd_vt::Value, Option<Vec<i32>>)> {
        self.sample_indexed_primvar(id, key, max_sample_count)
    }

    // ----------------------------------------------------------------------- //
    // Instancer
    // ----------------------------------------------------------------------- //

    /// Get instance indices for prototype. Default: empty.
    fn get_instance_indices(&self, _instancer_id: &SdfPath, _prototype_id: &SdfPath) -> Vec<i32> {
        Vec::new()
    }

    /// Get instancer transform. Default: identity.
    fn get_instancer_transform(&self, _instancer_id: &SdfPath) -> Matrix4d {
        Matrix4d::identity()
    }

    /// Get instancer prototypes. Default: empty.
    fn get_instancer_prototypes(&self, _instancer_id: &SdfPath) -> Vec<SdfPath> {
        Vec::new()
    }

    /// Sample instancer transform. Default: single sample at t=0.
    fn sample_instancer_transform(
        &self,
        instancer_id: &SdfPath,
        max_sample_count: usize,
    ) -> Vec<(f32, Matrix4d)> {
        if max_sample_count > 0 {
            vec![(0.0, self.get_instancer_transform(instancer_id))]
        } else {
            Vec::new()
        }
    }

    /// Sample instancer transform over interval.
    fn sample_instancer_transform_interval(
        &self,
        instancer_id: &SdfPath,
        _start_time: f32,
        _end_time: f32,
        max_sample_count: usize,
    ) -> Vec<(f32, Matrix4d)> {
        self.sample_instancer_transform(instancer_id, max_sample_count)
    }

    // ----------------------------------------------------------------------- //
    // Path translation
    // ----------------------------------------------------------------------- //

    /// Get scene prim path from rprim id and instance index. Strips delegate prefix.
    fn get_scene_prim_path(
        &self,
        rprim_id: &SdfPath,
        _instance_index: i32,
        _instancer_context: Option<&mut HdInstancerContext>,
    ) -> SdfPath {
        rprim_id
            .replace_prefix(&self.get_delegate_id(), &SdfPath::absolute_root())
            .unwrap_or_else(|| rprim_id.clone())
    }

    /// Get scene prim paths for multiple instances.
    fn get_scene_prim_paths(
        &self,
        rprim_id: &SdfPath,
        instance_indices: &[i32],
        _instancer_contexts: Option<&mut Vec<HdInstancerContext>>,
    ) -> Vec<SdfPath> {
        let scene_path = rprim_id
            .replace_prefix(&self.get_delegate_id(), &SdfPath::absolute_root())
            .unwrap_or_else(|| rprim_id.clone());
        vec![scene_path; instance_indices.len()]
    }

    // ----------------------------------------------------------------------- //
    // Material
    // ----------------------------------------------------------------------- //

    /// Get material resource. Default: empty value.
    fn get_material_resource(&self, _material_id: &SdfPath) -> usd_vt::Value {
        usd_vt::Value::default()
    }

    // ----------------------------------------------------------------------- //
    // Render buffer
    // ----------------------------------------------------------------------- //

    /// Get render buffer descriptor. Default: default struct.
    fn get_render_buffer_descriptor(&self, _id: &SdfPath) -> HdRenderBufferDescriptor {
        HdRenderBufferDescriptor::default()
    }

    // ----------------------------------------------------------------------- //
    // Light
    // ----------------------------------------------------------------------- //

    /// Get light param value. Default: empty.
    fn get_light_param_value(&self, _id: &SdfPath, _param_name: &usd_tf::Token) -> usd_vt::Value {
        usd_vt::Value::default()
    }

    // ----------------------------------------------------------------------- //
    // Camera
    // ----------------------------------------------------------------------- //

    /// Get camera param value. Default: empty.
    fn get_camera_param_value(
        &self,
        _camera_id: &SdfPath,
        _param_name: &usd_tf::Token,
    ) -> usd_vt::Value {
        usd_vt::Value::default()
    }

    // ----------------------------------------------------------------------- //
    // Volume
    // ----------------------------------------------------------------------- //

    /// Get volume field descriptors. Default: empty.
    fn get_volume_field_descriptors(&self, _volume_id: &SdfPath) -> HdVolumeFieldDescriptorVector {
        Vec::new()
    }

    // ----------------------------------------------------------------------- //
    // ExtComputation
    // ----------------------------------------------------------------------- //

    /// Get ext computation scene input names. Default: empty.
    fn get_ext_computation_scene_input_names(
        &self,
        _computation_id: &SdfPath,
    ) -> Vec<usd_tf::Token> {
        Vec::new()
    }

    /// Get ext computation input descriptors. Default: empty.
    fn get_ext_computation_input_descriptors(
        &self,
        _computation_id: &SdfPath,
    ) -> HdExtComputationInputDescriptorVector {
        Vec::new()
    }

    /// Get ext computation output descriptors. Default: empty.
    fn get_ext_computation_output_descriptors(
        &self,
        _computation_id: &SdfPath,
    ) -> HdExtComputationOutputDescriptorVector {
        Vec::new()
    }

    /// Get ext computation primvar descriptors. Default: empty.
    fn get_ext_computation_primvar_descriptors(
        &self,
        _id: &SdfPath,
        _interpolation: crate::HdInterpolation,
    ) -> HdExtComputationPrimvarDescriptorVector {
        Vec::new()
    }

    /// Get ext computation input. Default: empty.
    fn get_ext_computation_input(
        &self,
        _computation_id: &SdfPath,
        _input: &usd_tf::Token,
    ) -> usd_vt::Value {
        usd_vt::Value::default()
    }

    /// Sample ext computation input. Default: single sample at t=0.
    fn sample_ext_computation_input(
        &self,
        computation_id: &SdfPath,
        input: &usd_tf::Token,
        max_sample_count: usize,
    ) -> Vec<(f32, usd_vt::Value)> {
        if max_sample_count > 0 {
            vec![(0.0, self.get_ext_computation_input(computation_id, input))]
        } else {
            Vec::new()
        }
    }

    /// Sample ext computation input over interval.
    fn sample_ext_computation_input_interval(
        &self,
        computation_id: &SdfPath,
        input: &usd_tf::Token,
        _start_time: f32,
        _end_time: f32,
        max_sample_count: usize,
    ) -> Vec<(f32, usd_vt::Value)> {
        self.sample_ext_computation_input(computation_id, input, max_sample_count)
    }

    /// Get ext computation kernel source. Default: empty string.
    fn get_ext_computation_kernel(&self, _computation_id: &SdfPath) -> String {
        String::new()
    }

    /// Invoke ext computation. Default: no-op.
    fn invoke_ext_computation(
        &mut self,
        _computation_id: &SdfPath,
        _context: &mut dyn HdExtComputationContext,
    ) {
    }

    // ----------------------------------------------------------------------- //
    // Primvars
    // ----------------------------------------------------------------------- //

    /// Get primvar descriptors. Default: empty.
    fn get_primvar_descriptors(
        &self,
        _id: &SdfPath,
        _interpolation: crate::HdInterpolation,
    ) -> HdPrimvarDescriptorVector {
        Vec::new()
    }

    // ----------------------------------------------------------------------- //
    // Task
    // ----------------------------------------------------------------------- //

    /// Get task render tags. Default: empty.
    fn get_task_render_tags(&self, _task_id: &SdfPath) -> Vec<usd_tf::Token> {
        Vec::new()
    }
}

// Submodules
pub mod basis_curves;
pub mod bprim;
pub mod camera;
pub mod coord_sys;
pub mod ext_computation;
pub mod field;
pub mod image_shader;
pub mod instancer;
pub mod light;
pub mod material;
pub mod mesh;
pub mod points;
pub mod render_buffer;
pub mod render_settings;
pub mod rprim;
pub mod sprim;
pub mod volume;

// Re-exports
pub use basis_curves::HdBasisCurves;
pub use bprim::HdBprim;
pub use camera::{CameraUtilConformWindowPolicy, HdCamera, HdCameraDirtyBits, HdCameraProjection};
pub use coord_sys::HdCoordSys;
pub use ext_computation::HdExtComputation;
pub use field::HdField;
pub use image_shader::HdImageShader;
pub use instancer::HdInstancer;
pub use light::{HdLight, HdLightType};
pub use material::HdMaterial;
pub use mesh::HdMesh;
pub use points::HdPoints;
pub use render_buffer::HdRenderBuffer;
pub use render_settings::{HdRenderProduct, HdRenderSettings, render_settings_dirty_bits};
pub use rprim::HdRprim;
pub use sprim::HdSprim;
pub use volume::HdVolume;

// HdRenderParam, HdMeshReprDesc, HdBasisCurvesReprDesc, HdPointsReprDesc
// are defined directly in this module and already public.
