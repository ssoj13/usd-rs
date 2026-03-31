//! UsdSkelBakeSkinning - functions for baking skinning effects.
//!
//! Port of pxr/usd/usdSkel/bakeSkinning.h/cpp

use super::binding::Binding;
use super::binding_api::BindingAPI;
use super::blend_shape_query::BlendShapeQuery;
use super::cache::Cache;
use super::root::SkelRoot;
use super::skeleton_query::SkeletonQuery;
use super::skinning_query::SkinningQuery;
use bitflags::bitflags;
use usd_core::Prim;
use usd_core::prim_flags::PrimFlagsPredicate;
use usd_geom::point_based::PointBased;
use usd_geom::xformable::Xformable;
use usd_gf::{Interval, Matrix4d, Vec3f};
use usd_sdf::LayerHandle;
use usd_sdf::TimeCode;

bitflags! {
    /// Flags for identifying different deformation paths.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct DeformationFlags: u32 {
        /// Deform points using skinning.
        const DEFORM_POINTS_WITH_SKINNING = 1 << 0;
        /// Deform normals using skinning.
        const DEFORM_NORMALS_WITH_SKINNING = 1 << 1;
        /// Deform transforms using skinning.
        const DEFORM_XFORM_WITH_SKINNING = 1 << 2;
        /// Deform points using blend shapes.
        const DEFORM_POINTS_WITH_BLEND_SHAPES = 1 << 3;
        /// Deform normals using blend shapes.
        const DEFORM_NORMALS_WITH_BLEND_SHAPES = 1 << 4;
        /// All skinning deformations.
        const DEFORM_WITH_SKINNING = Self::DEFORM_POINTS_WITH_SKINNING.bits()
            | Self::DEFORM_NORMALS_WITH_SKINNING.bits()
            | Self::DEFORM_XFORM_WITH_SKINNING.bits();
        /// All blend shape deformations.
        const DEFORM_WITH_BLEND_SHAPES = Self::DEFORM_POINTS_WITH_BLEND_SHAPES.bits()
            | Self::DEFORM_NORMALS_WITH_BLEND_SHAPES.bits();
        /// All deformations.
        const DEFORM_ALL = Self::DEFORM_WITH_SKINNING.bits()
            | Self::DEFORM_WITH_BLEND_SHAPES.bits();
        /// Flags indicating which components may be modified.
        const MODIFIES_POINTS = Self::DEFORM_POINTS_WITH_SKINNING.bits()
            | Self::DEFORM_POINTS_WITH_BLEND_SHAPES.bits();
        /// Indicates that normals may be modified.
        const MODIFIES_NORMALS = Self::DEFORM_NORMALS_WITH_SKINNING.bits()
            | Self::DEFORM_NORMALS_WITH_BLEND_SHAPES.bits();
        /// Indicates that transforms may be modified.
        const MODIFIES_XFORM = Self::DEFORM_XFORM_WITH_SKINNING.bits();
    }
}

impl Default for DeformationFlags {
    fn default() -> Self {
        Self::DEFORM_ALL
    }
}

/// Parameters for configuring bake_skinning().
#[derive(Clone, Default)]
pub struct BakeSkinningParams {
    /// Flags determining which deformation paths are enabled.
    pub deformation_flags: DeformationFlags,
    /// Determines whether or not layers are saved during skinning.
    /// If disabled, all skinning data is kept in-memory.
    pub save_layers: bool,
    /// Memory limit for pending stage writes, given in bytes.
    /// If zero, memory limits are ignored.
    pub memory_limit: usize,
    /// If true, extents of UsdGeomPointBased-derived prims are updated.
    pub update_extents: bool,
    /// If true, extents hints of models are updated.
    pub update_extent_hints: bool,
    /// The set of bindings to bake.
    pub bindings: Vec<Binding>,
    /// Data layers being written to.
    pub layers: Vec<LayerHandle>,
    /// Array providing an index per elem in bindings, indicating
    /// which layer the skinned result should be written to.
    pub layer_indices: Vec<u32>,
}

impl BakeSkinningParams {
    /// Create default parameters.
    pub fn new() -> Self {
        Self {
            deformation_flags: DeformationFlags::DEFORM_ALL,
            save_layers: true,
            memory_limit: 0,
            update_extents: true,
            update_extent_hints: true,
            bindings: Vec::new(),
            layers: Vec::new(),
            layer_indices: Vec::new(),
        }
    }

    /// Returns true if points may be modified.
    pub fn modifies_points(&self) -> bool {
        self.deformation_flags
            .intersects(DeformationFlags::MODIFIES_POINTS)
    }

    /// Returns true if normals may be modified.
    pub fn modifies_normals(&self) -> bool {
        self.deformation_flags
            .intersects(DeformationFlags::MODIFIES_NORMALS)
    }

    /// Returns true if transforms may be modified.
    pub fn modifies_xform(&self) -> bool {
        self.deformation_flags
            .intersects(DeformationFlags::MODIFIES_XFORM)
    }
}

/// Helper for tracking the state of a bake task.
struct BakeTask {
    active: bool,
    required: bool,
    might_be_time_varying: bool,
    is_first_sample: bool,
    has_sample_at_current_time: bool,
}

impl Default for BakeTask {
    fn default() -> Self {
        Self {
            active: false,
            required: false,
            might_be_time_varying: false,
            is_first_sample: true,
            has_sample_at_current_time: false,
        }
    }
}

impl BakeTask {
    fn is_runnable(&self) -> bool {
        self.active && self.required
    }

    #[allow(dead_code)] // Internal API - task execution
    fn run<F>(&mut self, time: &TimeCode, compute: F) -> bool
    where
        F: FnOnce(&TimeCode) -> bool,
    {
        if !self.is_runnable() {
            return false;
        }

        // Always compute for defaults or time-varying tasks.
        // For numeric times, if not time varying, only compute first time.
        if self.might_be_time_varying || self.is_first_sample || time.is_default() {
            self.has_sample_at_current_time = compute(time);
            if !time.is_default() {
                self.is_first_sample = false;
            }
        }

        self.has_sample_at_current_time
    }
}

/// State for baking a single skinned prim.
struct SkinnedPrimBaker {
    /// The skinning query for this prim.
    skinning_query: SkinningQuery,
    /// The blend shape query (if any).
    blend_shape_query: Option<BlendShapeQuery>,
    /// Cached rest points.
    rest_points: Vec<Vec3f>,
    /// Cached rest normals.
    rest_normals: Vec<Vec3f>,
    /// Task for computing points.
    points_task: BakeTask,
    /// Task for computing normals.
    normals_task: BakeTask,
    /// Task for computing transform.
    xform_task: BakeTask,
    /// Whether blend shape deformation of points is active.
    deform_points_with_blend_shapes: bool,
    /// Whether blend shape deformation of normals is active.
    deform_normals_with_blend_shapes: bool,
    /// Cached blend shape point indices (per blend shape).
    blend_shape_point_indices: Vec<Vec<i32>>,
    /// Cached sub-shape point offsets (per sub-shape).
    sub_shape_point_offsets: Vec<Vec<Vec3f>>,
    /// Cached sub-shape normal offsets (per sub-shape).
    sub_shape_normal_offsets: Vec<Vec<Vec3f>>,
}

impl SkinnedPrimBaker {
    fn new(skinning_query: SkinningQuery) -> Self {
        Self {
            skinning_query,
            blend_shape_query: None,
            rest_points: Vec::new(),
            rest_normals: Vec::new(),
            points_task: BakeTask::default(),
            normals_task: BakeTask::default(),
            xform_task: BakeTask::default(),
            deform_points_with_blend_shapes: false,
            deform_normals_with_blend_shapes: false,
            blend_shape_point_indices: Vec::new(),
            sub_shape_point_offsets: Vec::new(),
            sub_shape_normal_offsets: Vec::new(),
        }
    }

    /// Create a new baker with a blend shape query.
    fn with_blend_shapes(
        skinning_query: SkinningQuery,
        blend_shape_query: BlendShapeQuery,
    ) -> Self {
        Self {
            skinning_query,
            blend_shape_query: Some(blend_shape_query),
            rest_points: Vec::new(),
            rest_normals: Vec::new(),
            points_task: BakeTask::default(),
            normals_task: BakeTask::default(),
            xform_task: BakeTask::default(),
            deform_points_with_blend_shapes: false,
            deform_normals_with_blend_shapes: false,
            blend_shape_point_indices: Vec::new(),
            sub_shape_point_offsets: Vec::new(),
            sub_shape_normal_offsets: Vec::new(),
        }
    }

    /// Returns the blend shape query if available.
    #[cfg(test)]
    fn get_blend_shape_query(&self) -> Option<&BlendShapeQuery> {
        self.blend_shape_query.as_ref()
    }

    /// Returns true if this prim has blend shapes.
    #[cfg(test)]
    fn has_blend_shapes(&self) -> bool {
        self.blend_shape_query.is_some()
    }

    /// Initialize the baking state for this prim.
    /// Matches C++ `_SkinningAdapter` constructor: caches rest geometry,
    /// sets up blend shape data (point indices, sub-shape offsets), and
    /// activates the appropriate deformation tasks.
    fn init(&mut self, params: &BakeSkinningParams) {
        let prim = match self.skinning_query.get_prim() {
            Some(p) => p.clone(),
            None => return,
        };

        let has_joint_influences = self.skinning_query.has_joint_influences();
        let is_rigidly_deformed = self.skinning_query.is_rigidly_deformed();
        let is_point_based = !is_rigidly_deformed;

        // Cache rest points if we'll need them (skinning or blend shapes)
        let point_based = PointBased::new(prim.clone());
        if is_point_based && params.modifies_points() {
            let points_attr = point_based.get_points_attr();
            if points_attr.is_valid() {
                if let Some(points) = points_attr.get_typed_vec::<Vec3f>(TimeCode::default()) {
                    self.rest_points = points;
                }
            }
        }

        // Cache rest normals
        if is_point_based && params.modifies_normals() {
            let normals_attr = point_based.get_normals_attr();
            if normals_attr.is_valid() {
                if let Some(normals) = normals_attr.get_typed_vec::<Vec3f>(TimeCode::default()) {
                    self.rest_normals = normals;
                }
            }
        }

        // Initialize blend shape caches (matches C++ _SkinningAdapter init).
        // Blend shapes are applied before LBS/DQS skinning.
        if let Some(bsq) = &self.blend_shape_query {
            if bsq.is_valid() {
                // Cache point offsets for blend shape deformation
                if params
                    .deformation_flags
                    .intersects(DeformationFlags::DEFORM_POINTS_WITH_BLEND_SHAPES)
                    && !self.rest_points.is_empty()
                {
                    let offsets = bsq.compute_sub_shape_point_offsets();
                    let has_offsets = offsets.iter().any(|o| !o.is_empty());
                    if has_offsets {
                        self.sub_shape_point_offsets = offsets;
                        self.deform_points_with_blend_shapes = true;
                    }
                }

                // Cache normal offsets for blend shape deformation
                if params
                    .deformation_flags
                    .intersects(DeformationFlags::DEFORM_NORMALS_WITH_BLEND_SHAPES)
                    && !self.rest_normals.is_empty()
                {
                    let offsets = bsq.compute_sub_shape_normal_offsets();
                    let has_offsets = offsets.iter().any(|o| !o.is_empty());
                    if has_offsets {
                        self.sub_shape_normal_offsets = offsets;
                        self.deform_normals_with_blend_shapes = true;
                    }
                }

                // Cache point indices if any blend shape deformation is active
                if self.deform_points_with_blend_shapes || self.deform_normals_with_blend_shapes {
                    self.blend_shape_point_indices = bsq.compute_blend_shape_point_indices();
                }
            }
        }

        // Points task (skinning)
        if params.modifies_points() && has_joint_influences && !is_rigidly_deformed {
            self.points_task.active = true;
            self.points_task.required = true;
            self.points_task.might_be_time_varying = true;
        }

        // Also activate points task if blend shapes affect points
        if self.deform_points_with_blend_shapes && !self.rest_points.is_empty() {
            self.points_task.active = true;
            self.points_task.required = true;
            self.points_task.might_be_time_varying = true;
        }

        // Normals task (skinning)
        if params.modifies_normals() && has_joint_influences && !is_rigidly_deformed {
            self.normals_task.active = true;
            self.normals_task.required = true;
            self.normals_task.might_be_time_varying = true;
        }

        // Also activate normals task if blend shapes affect normals
        if self.deform_normals_with_blend_shapes && !self.rest_normals.is_empty() {
            self.normals_task.active = true;
            self.normals_task.required = true;
            self.normals_task.might_be_time_varying = true;
        }

        // Xform task - for rigid deformation
        if params.modifies_xform() && has_joint_influences && is_rigidly_deformed {
            self.xform_task.active = true;
            self.xform_task.required = true;
            self.xform_task.might_be_time_varying = true;
        }
    }

    /// Apply blend shape deformation to points and normals.
    /// Matches C++ `_SkinningAdapter::_DeformWithBlendShapes()`:
    /// remaps skel-level weights to prim-local order, resolves sub-shapes,
    /// then deforms points/normals in-place before skinning.
    fn deform_with_blend_shapes(
        &self,
        skel_weights: &[f32],
        points: &mut Vec<Vec3f>,
        normals: &mut Vec<Vec3f>,
    ) -> bool {
        let bsq = match &self.blend_shape_query {
            Some(q) if q.is_valid() => q,
            _ => return false,
        };

        // Remap skel-level weights to prim-local blend shape order
        let weights_for_prim = if let Some(mapper) = self.skinning_query.get_blend_shape_mapper() {
            let mut remapped = Vec::new();
            let zero = 0.0f32;
            if !mapper.remap(skel_weights, &mut remapped, 1, Some(&zero)) {
                return false;
            }
            remapped
        } else {
            skel_weights.to_vec()
        };

        // Resolve sub-shape weights (handles inbetweens)
        let mut sub_shape_weights = Vec::new();
        let mut blend_shape_indices = Vec::new();
        let mut sub_shape_indices = Vec::new();

        if !bsq.compute_sub_shape_weights(
            &weights_for_prim,
            &mut sub_shape_weights,
            &mut blend_shape_indices,
            &mut sub_shape_indices,
        ) {
            return false;
        }

        let mut ok = true;

        // Deform points with blend shapes (before LBS/DQS skinning)
        if self.deform_points_with_blend_shapes && !points.is_empty() {
            if !bsq.compute_deformed_points(
                &sub_shape_weights,
                &blend_shape_indices,
                &sub_shape_indices,
                &self.blend_shape_point_indices,
                &self.sub_shape_point_offsets,
                points,
            ) {
                ok = false;
            }
        }

        // Deform normals with blend shapes
        if self.deform_normals_with_blend_shapes && !normals.is_empty() {
            if !bsq.compute_deformed_normals(
                &sub_shape_weights,
                &blend_shape_indices,
                &sub_shape_indices,
                &self.blend_shape_point_indices,
                &self.sub_shape_normal_offsets,
                normals,
            ) {
                ok = false;
            }
        }

        ok
    }

    /// Bake skinning at the given time using skinning transforms.
    /// Matches C++ `_SkinningAdapter::Update()`: blend shapes are applied
    /// first, then LBS/DQS skinning on top.
    fn bake(
        &mut self,
        time: &TimeCode,
        skinning_xforms: &[Matrix4d],
        blend_shape_weights: Option<&[f32]>,
    ) -> bool {
        let mut success = true;

        // Start from rest geometry
        let mut points = self.rest_points.clone();
        let mut normals = self.rest_normals.clone();

        // Step 1: Blend shapes precede LBS/DQS skinning (matches C++ ordering)
        let has_bs = self.deform_points_with_blend_shapes || self.deform_normals_with_blend_shapes;
        if has_bs {
            if let Some(weights) = blend_shape_weights {
                self.deform_with_blend_shapes(weights, &mut points, &mut normals);
            }
        }

        // Step 2: Bake points with skinning
        if self.points_task.is_runnable() {
            let prim = match self.skinning_query.get_prim() {
                Some(p) => p.clone(),
                None => return false,
            };

            if self
                .skinning_query
                .compute_skinned_points(skinning_xforms, &mut points, time)
            {
                let point_based = PointBased::new(prim);
                let points_attr = point_based.get_points_attr();
                if points_attr.is_valid() {
                    let _ = points_attr.set(usd_vt::Value::from_no_hash(points.clone()), *time);
                }
            } else {
                success = false;
            }
        } else if has_bs && blend_shape_weights.is_some() {
            // No skinning but blend shapes were applied -- write points anyway
            if let Some(prim) = self.skinning_query.get_prim() {
                let point_based = PointBased::new(prim.clone());
                let points_attr = point_based.get_points_attr();
                if points_attr.is_valid() && !points.is_empty() {
                    let _ = points_attr.set(usd_vt::Value::from_no_hash(points.clone()), *time);
                }
            }
        }

        // Step 3: Bake normals with skinning
        if self.normals_task.is_runnable() {
            let prim = match self.skinning_query.get_prim() {
                Some(p) => p.clone(),
                None => return false,
            };

            if self
                .skinning_query
                .compute_skinned_normals(skinning_xforms, &mut normals, time)
            {
                let point_based = PointBased::new(prim);
                let normals_attr = point_based.get_normals_attr();
                if normals_attr.is_valid() {
                    let _ = normals_attr.set(usd_vt::Value::from_no_hash(normals.clone()), *time);
                }
            } else {
                success = false;
            }
        } else if has_bs && blend_shape_weights.is_some() {
            // No skinning but blend shapes were applied -- write normals anyway
            if let Some(prim) = self.skinning_query.get_prim() {
                let point_based = PointBased::new(prim.clone());
                let normals_attr = point_based.get_normals_attr();
                if normals_attr.is_valid() && !normals.is_empty() {
                    let _ = normals_attr.set(usd_vt::Value::from_no_hash(normals.clone()), *time);
                }
            }
        }

        // Step 4: Bake transform for rigid skinning
        if self.xform_task.is_runnable() {
            let prim = match self.skinning_query.get_prim() {
                Some(p) => p.clone(),
                None => return false,
            };

            let mut xform = Matrix4d::identity();
            if self
                .skinning_query
                .compute_skinned_transform(skinning_xforms, &mut xform, time)
            {
                let xformable = Xformable::new(prim);
                let xform_ops = xformable.get_ordered_xform_ops();
                if let Some(first_op) = xform_ops.first() {
                    let data: Vec<f64> = xform.as_slice().to_vec();
                    let _ = first_op.set(usd_vt::Value::from_no_hash(data), *time);
                }
            } else {
                success = false;
            }
        }

        success
    }
}

/// State for baking an entire skeleton binding.
struct BindingBaker {
    /// The skeleton query.
    skel_query: SkeletonQuery,
    /// Bakers for each skinned prim.
    skinned_prim_bakers: Vec<SkinnedPrimBaker>,
    /// Cached skinning transforms.
    skinning_xforms: Vec<Matrix4d>,
    /// Cached blend shape weights from the anim source.
    blend_shape_weights: Vec<f32>,
    /// Whether any prim needs blend shape weights.
    needs_blend_shape_weights: bool,
}

impl BindingBaker {
    fn new(skel_query: SkeletonQuery, binding: &Binding) -> Self {
        let skinned_prim_bakers: Vec<_> = binding
            .get_skinning_targets()
            .iter()
            .map(|sq| {
                // Check if this skinning target has blend shapes
                if sq.has_blend_shapes() {
                    if let Some(prim) = sq.get_prim() {
                        // Create BlendShapeQuery from the prim's binding API
                        let binding_api = BindingAPI::new(prim.clone());
                        let bsq = BlendShapeQuery::from_binding(&binding_api);
                        // Use constructor with blend shapes
                        return SkinnedPrimBaker::with_blend_shapes(sq.clone(), bsq);
                    }
                }
                // No blend shapes - use basic constructor
                SkinnedPrimBaker::new(sq.clone())
            })
            .collect();

        Self {
            skel_query,
            skinned_prim_bakers,
            skinning_xforms: Vec::new(),
            blend_shape_weights: Vec::new(),
            needs_blend_shape_weights: false,
        }
    }

    /// Initialize baking state.
    /// After initializing each prim baker, check if any need blend shape
    /// weights so we can compute them from the anim query during bake().
    fn init(&mut self, params: &BakeSkinningParams) {
        for baker in &mut self.skinned_prim_bakers {
            baker.init(params);
        }
        // Check if any prim baker needs blend shape weights
        self.needs_blend_shape_weights = self
            .skinned_prim_bakers
            .iter()
            .any(|b| b.deform_points_with_blend_shapes || b.deform_normals_with_blend_shapes);
    }

    /// Bake skinning at the given time.
    /// Matches C++ ordering: compute skel-level animation (skinning xforms +
    /// blend shape weights), then update each skinned prim (blend shapes first,
    /// then LBS/DQS skinning).
    fn bake(&mut self, time: &TimeCode) -> bool {
        // Compute skinning transforms
        if !self
            .skel_query
            .compute_skinning_transforms(&mut self.skinning_xforms, time)
        {
            return false;
        }

        // Compute blend shape weights from the anim source (if any prim needs them)
        let bs_weights: Option<&[f32]> = if self.needs_blend_shape_weights {
            let anim_query = self.skel_query.get_anim_query();
            if anim_query.compute_blend_shape_weights(&mut self.blend_shape_weights, time) {
                Some(&self.blend_shape_weights)
            } else {
                None
            }
        } else {
            None
        };

        // Clone weights to avoid borrow conflict (self.blend_shape_weights
        // is borrowed by bs_weights while self.skinned_prim_bakers is &mut)
        let bs_weights_owned: Option<Vec<f32>> = bs_weights.map(|w| w.to_vec());
        let bs_ref = bs_weights_owned.as_deref();

        let mut success = true;
        for baker in &mut self.skinned_prim_bakers {
            if !baker.bake(time, &self.skinning_xforms, bs_ref) {
                success = false;
            }
        }

        success
    }
}

/// Bake the effect of skinning prims directly into points and transforms,
/// over `interval`.
///
/// This is intended to serve as a complete reference implementation,
/// providing a ground truth for testing and validation purposes.
///
/// WARNING: This will undo the IO gains that deferred deformations provide.
/// A USD file, once skinning has been baked, may easily see an increase of 100x
/// in disk usage. The intent of UsdSkel is to defer skinning until render time.
pub fn bake_skinning(skel_cache: &Cache, params: &BakeSkinningParams, interval: &Interval) -> bool {
    if params.bindings.is_empty() {
        return true;
    }

    // Create bakers for each binding
    let mut bakers: Vec<BindingBaker> = params
        .bindings
        .iter()
        .map(|binding| {
            let skel_query = skel_cache.get_skel_query(binding.get_skeleton());
            BindingBaker::new(skel_query, binding)
        })
        .collect();

    // Initialize all bakers
    for baker in &mut bakers {
        baker.init(params);
    }

    // Get time samples to process
    let times = get_bake_times(interval, &bakers);

    // Bake at each time
    let mut success = true;
    for time in times {
        let time_code = TimeCode::from(time);
        for baker in &mut bakers {
            if !baker.bake(&time_code) {
                success = false;
            }
        }
    }

    // Save layers if requested
    if params.save_layers {
        for _layer in &params.layers {
            // layer.save() not available in this implementation
        }
    }

    success
}

/// Bake skinning for all skels bound beneath `root`, over `interval`.
///
/// Skinning is baked into the current edit target. The edit target is *not*
/// saved during skinning: the caller should save or export the result.
pub fn bake_skinning_for_root(root: &SkelRoot, interval: &Interval) -> bool {
    let cache = Cache::new();

    // Populate the cache
    if !cache.populate_default(root) {
        return false;
    }

    // Get all bindings
    let bindings = cache.compute_skel_bindings(root, PrimFlagsPredicate::default());
    if bindings.is_empty() {
        return true;
    }

    // Get the edit target layer
    let prim = root.prim().clone();
    let stage = prim.stage().expect("prim has stage");
    let layer = LayerHandle::from_layer(&stage.get_root_layer());

    let params = BakeSkinningParams {
        bindings,
        layers: vec![layer],
        layer_indices: vec![0],
        ..Default::default()
    };

    bake_skinning(&cache, &params, interval)
}

/// Bake skinning for all SkelRoot prims in `prims`, over `interval`.
pub fn bake_skinning_for_prims(prims: &[Prim], interval: &Interval) -> bool {
    let mut success = true;

    for prim in prims {
        let root = SkelRoot::new(prim.clone());
        if root.is_valid() && !bake_skinning_for_root(&root, interval) {
            success = false;
        }
    }

    success
}

/// Get time samples for baking, based on the interval and baker state.
///
/// Collects the union of time samples from all skeleton anim queries
/// (joint transforms, blend shape weights) and skinning queries across
/// all bakers, then sorts and deduplicates.
///
/// Matches C++ `_GetBakeTimes()` in bakeSkinning.cpp.
fn get_bake_times(interval: &Interval, bakers: &[BindingBaker]) -> Vec<f64> {
    let mut all_times: Vec<f64> = Vec::new();

    // Always include default time (matches C++ which prepends UsdTimeCode::Default)
    all_times.push(TimeCode::default().value());

    let start = interval.get_min();
    let end = interval.get_max();

    for baker in bakers {
        // Collect time samples from skeleton anim query
        let anim_query = baker.skel_query.get_anim_query();

        // Joint transform time samples
        let mut joint_times = Vec::new();
        anim_query.get_joint_transform_time_samples_in_interval(start, end, &mut joint_times);
        all_times.extend(&joint_times);

        // Blend shape weight time samples
        let mut bs_times = Vec::new();
        anim_query.get_blend_shape_weight_time_samples_in_interval(start, end, &mut bs_times);
        all_times.extend(&bs_times);

        // Collect time samples from each skinning query
        for prim_baker in &baker.skinned_prim_bakers {
            let mut sq_times = Vec::new();
            prim_baker
                .skinning_query
                .get_time_samples_in_interval(start, end, &mut sq_times);
            all_times.extend(&sq_times);
        }
    }

    // Sort and deduplicate
    all_times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    all_times.dedup_by(|a, b| (*a - *b).abs() < 1e-12);

    all_times
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deformation_flags() {
        let flags = DeformationFlags::DEFORM_ALL;
        assert!(flags.contains(DeformationFlags::DEFORM_POINTS_WITH_SKINNING));
        assert!(flags.contains(DeformationFlags::DEFORM_NORMALS_WITH_SKINNING));
        assert!(flags.contains(DeformationFlags::DEFORM_XFORM_WITH_SKINNING));
    }

    #[test]
    fn test_params_default() {
        let params = BakeSkinningParams::new();
        assert!(params.modifies_points());
        assert!(params.modifies_normals());
        assert!(params.modifies_xform());
        assert!(params.save_layers);
        assert!(params.update_extents);
    }

    #[test]
    fn test_skinned_prim_baker_with_blend_shapes() {
        // Create a mock skinning query and blend shape query
        // This tests that the constructor properly initializes the blend shape query
        let skinning_query = SkinningQuery::default();
        let blend_shape_query = BlendShapeQuery::default();

        let baker = SkinnedPrimBaker::with_blend_shapes(skinning_query, blend_shape_query);

        assert!(baker.has_blend_shapes());
        assert!(baker.get_blend_shape_query().is_some());
    }

    #[test]
    fn test_skinned_prim_baker_without_blend_shapes() {
        let skinning_query = SkinningQuery::default();
        let baker = SkinnedPrimBaker::new(skinning_query);

        assert!(!baker.has_blend_shapes());
        assert!(baker.get_blend_shape_query().is_none());
    }
}
