//! UsdGeomBBoxCache - caches bounds by recursively computing and aggregating bounds.
//!
//! Port of pxr/usd/usdGeom/bboxCache.h/cpp
//!
//! Caches bounds by recursively computing and aggregating bounds of children in
//! world space and aggregating the result back into local space.

use super::boundable::Boundable;
use super::imageable::Imageable;
use super::model_api::ModelAPI;
use super::point_instancer::PointInstancer;
use super::tokens::usd_geom_tokens;
use super::xform_cache::XformCache;
use std::collections::{HashMap, HashSet};
use usd_core::Prim;
use usd_gf::vec3::Vec3f;
use usd_gf::{BBox3d, Matrix4d, Range3d, Vec3d};
use usd_sdf::{Path, TimeCode};
use usd_tf::Token;

// ============================================================================
// BBoxCache
// ============================================================================

/// Caches bounds by recursively computing and aggregating bounds of children.
///
/// Matches C++ `UsdGeomBBoxCache`.
///
/// The cache is configured for a specific time and set of purposes. When
/// querying a bound, transforms and extents are read either from the time
/// specified or UsdTimeCode::Default().
pub struct BBoxCache {
    /// Map of cached bounds (prim path -> entry).
    bounds_cache: HashMap<Path, BBoxEntry>,
    /// XformCache for computing transforms.
    xform_cache: XformCache,
    /// Set of included purposes.
    included_purposes: Vec<Token>,
    /// Whether to use extents hints.
    use_extents_hint: bool,
    /// Whether to ignore visibility.
    ignore_visibility: bool,
    /// The time at which this cache is querying values.
    time: TimeCode,
    /// Base time for point instancer bounds (C++ `_baseTime`).
    base_time: Option<TimeCode>,
}

/// Cached entry for a prim's bounding box.
///
/// Matches C++ `_Entry` with isComplete/isVarying/isIncluded tracking.
#[derive(Clone, Debug)]
#[allow(dead_code)] // C++ parity fields; is_varying/is_included used in tests and future impl
struct BBoxEntry {
    /// Cached bounding box.
    bbox: BBox3d,
    /// True when data in the entry is valid.
    is_complete: bool,
    /// True when the entry varies over time.
    is_varying: bool,
    /// True when the entry is visible/included.
    is_included: bool,
}

impl BBoxEntry {
    #[allow(dead_code)] // Used in tests
    fn new() -> Self {
        Self {
            bbox: BBox3d::new(),
            is_complete: false,
            is_varying: false,
            is_included: false,
        }
    }
}

impl BBoxCache {
    /// Construct a new BBoxCache for a specific time and set of included purposes.
    ///
    /// Matches C++ `UsdGeomBBoxCache(UsdTimeCode time, TfTokenVector includedPurposes, ...)`.
    pub fn new(
        time: TimeCode,
        included_purposes: Vec<Token>,
        use_extents_hint: bool,
        ignore_visibility: bool,
    ) -> Self {
        Self {
            bounds_cache: HashMap::new(),
            xform_cache: XformCache::new(time),
            included_purposes,
            use_extents_hint,
            ignore_visibility,
            time,
            base_time: None,
        }
    }

    /// Compute the bound of the given prim in world space.
    ///
    /// Matches C++ `ComputeWorldBound()`.
    pub fn compute_world_bound(&mut self, prim: &Prim) -> BBox3d {
        if !prim.is_valid() {
            return BBox3d::new();
        }

        // Check cache by path (stable across Arc clones)
        let path = prim.path().clone();
        if let Some(entry) = self.bounds_cache.get(&path) {
            if entry.is_complete {
                return entry.bbox;
            }
        }

        // Compute bound
        let bbox = self.compute_world_bound_internal(prim);

        // Cache result with tracking flags
        let entry = BBoxEntry {
            bbox,
            is_complete: true,
            is_varying: false, // Detecting time-varying extents requires checking whether
            // any contributing attribute has >1 time sample (C++ _IsVarying).
            // Not yet implemented; cache entries are conservatively non-varying.
            is_included: true,
        };
        self.bounds_cache.insert(path, entry);
        bbox
    }

    /// Compute the bound of the given prim in local space.
    ///
    /// C++ pattern: _Resolve(prim) → untransformed bbox, then
    /// transform by local xform only (NOT by full CTM).
    ///
    /// Matches C++ `ComputeLocalBound()`.
    pub fn compute_local_bound(&mut self, prim: &Prim) -> BBox3d {
        if !prim.is_valid() {
            return BBox3d::new();
        }

        // Get untransformed bound (extent in prim-local object space)
        let bbox = self.resolve_untransformed_bound(prim);

        // Apply the prim's own local transform
        let (local_xform, _resets) = self.xform_cache.get_local_transformation(prim);
        let mut local_bbox = bbox;
        local_bbox.transform(&local_xform);
        local_bbox
    }

    /// Compute the untransformed bound of the given prim.
    ///
    /// Returns the combined extent in the prim's own object space,
    /// with NO transforms applied (not even the prim's own local xform).
    ///
    /// Matches C++ `ComputeUntransformedBound()`.
    pub fn compute_untransformed_bound(&mut self, prim: &Prim) -> BBox3d {
        if !prim.is_valid() {
            return BBox3d::new();
        }

        self.resolve_untransformed_bound(prim)
    }

    /// Compute the bound of the given prim in the space of an ancestor prim.
    ///
    /// The computed bound excludes the local transform at `relative_to_ancestor`.
    /// The result may be incorrect if `relative_to_ancestor` is not an ancestor of `prim`.
    ///
    /// Matches C++ `ComputeRelativeBound()`.
    pub fn compute_relative_bound(&mut self, prim: &Prim, relative_to_ancestor: &Prim) -> BBox3d {
        if !prim.is_valid() || !relative_to_ancestor.is_valid() {
            return BBox3d::new();
        }

        // Get world bound of the prim
        let world_bbox = self.compute_world_bound(prim);

        // Get the ancestor's parent-to-world transform (excludes ancestor's own local xform)
        let ancestor_to_world = self
            .xform_cache
            .get_local_to_world_transform(relative_to_ancestor);

        // Invert the ancestor CTM to transform from world into ancestor-local space
        if let Some(world_to_ancestor) = ancestor_to_world.inverse() {
            let mut relative_bbox = world_bbox;
            relative_bbox.transform(&world_to_ancestor);
            relative_bbox
        } else {
            BBox3d::new()
        }
    }

    /// Compute the untransformed bound of descendants, excluding subtrees
    /// rooted at paths in `paths_to_skip` and applying CTM overrides.
    ///
    /// Does NOT include the transform authored on `prim` itself.
    ///
    /// Matches C++ `ComputeUntransformedBound(prim, pathsToSkip, ctmOverrides)`.
    pub fn compute_untransformed_bound_with_overrides(
        &mut self,
        prim: &Prim,
        paths_to_skip: &HashSet<Path>,
        ctm_overrides: &HashMap<Path, Matrix4d>,
    ) -> BBox3d {
        if !prim.is_valid() {
            return BBox3d::new();
        }

        // Compute child bounds skipping specified subtrees
        self.compute_bound_from_children_with_skip(prim, paths_to_skip, ctm_overrides)
    }

    /// Compute the bound of the prim's descendants in world space while
    /// excluding subtrees rooted at `paths_to_skip`, with an override
    /// for the prim's local-to-world transform and additional CTM overrides.
    ///
    /// Matches C++ `ComputeWorldBoundWithOverrides()`.
    pub fn compute_world_bound_with_overrides(
        &mut self,
        prim: &Prim,
        paths_to_skip: &HashSet<Path>,
        prim_override: &Matrix4d,
        ctm_overrides: &HashMap<Path, Matrix4d>,
    ) -> BBox3d {
        if !prim.is_valid() {
            return BBox3d::new();
        }

        // Compute descendant bounds excluding skipped subtrees
        let untransformed =
            self.compute_bound_from_children_with_skip(prim, paths_to_skip, ctm_overrides);

        // Apply the overridden prim transform (instead of the authored one)
        let mut result = untransformed;
        result.transform(prim_override);
        result
    }

    // ========================================================================
    // Point Instance Bounds
    // ========================================================================

    /// Compute the bounds of given point instances in world space.
    ///
    /// The bounds of each instance is computed and transformed to world space.
    ///
    /// Matches C++ `ComputePointInstanceWorldBounds()`.
    pub fn compute_point_instance_world_bounds(
        &mut self,
        instancer: &PointInstancer,
        instance_ids: &[i64],
    ) -> Vec<BBox3d> {
        let local_to_world = self
            .xform_cache
            .get_local_to_world_transform(instancer.prim());
        self.compute_point_instance_bounds_helper(instancer, instance_ids, &local_to_world)
    }

    /// Compute the bound of a single point instance in world space.
    ///
    /// Matches C++ `ComputePointInstanceWorldBound()`.
    pub fn compute_point_instance_world_bound(
        &mut self,
        instancer: &PointInstancer,
        instance_id: i64,
    ) -> BBox3d {
        let result = self.compute_point_instance_world_bounds(instancer, &[instance_id]);
        result.into_iter().next().unwrap_or_else(BBox3d::new)
    }

    /// Compute the bounds of given point instances relative to an ancestor prim.
    ///
    /// The computed bound excludes the local transform at `relative_to_ancestor`.
    ///
    /// Matches C++ `ComputePointInstanceRelativeBounds()`.
    pub fn compute_point_instance_relative_bounds(
        &mut self,
        instancer: &PointInstancer,
        instance_ids: &[i64],
        relative_to_ancestor: &Prim,
    ) -> Vec<BBox3d> {
        let ancestor_to_world = self
            .xform_cache
            .get_local_to_world_transform(relative_to_ancestor);
        let world_results = self.compute_point_instance_world_bounds(instancer, instance_ids);

        if let Some(world_to_ancestor) = ancestor_to_world.inverse() {
            world_results
                .into_iter()
                .map(|mut bbox| {
                    bbox.transform(&world_to_ancestor);
                    bbox
                })
                .collect()
        } else {
            vec![BBox3d::new(); instance_ids.len()]
        }
    }

    /// Compute the bound of a single point instance relative to an ancestor prim.
    ///
    /// Matches C++ `ComputePointInstanceRelativeBound()`.
    pub fn compute_point_instance_relative_bound(
        &mut self,
        instancer: &PointInstancer,
        instance_id: i64,
        relative_to_ancestor: &Prim,
    ) -> BBox3d {
        let result = self.compute_point_instance_relative_bounds(
            instancer,
            &[instance_id],
            relative_to_ancestor,
        );
        result.into_iter().next().unwrap_or_else(BBox3d::new)
    }

    /// Compute the oriented bounding boxes of given point instances.
    ///
    /// Includes the instancer's own transform but not ancestor transforms.
    ///
    /// Matches C++ `ComputePointInstanceLocalBounds()`.
    pub fn compute_point_instance_local_bounds(
        &mut self,
        instancer: &PointInstancer,
        instance_ids: &[i64],
    ) -> Vec<BBox3d> {
        let (local_xform, _) = self.xform_cache.get_local_transformation(instancer.prim());
        self.compute_point_instance_bounds_helper(instancer, instance_ids, &local_xform)
    }

    /// Compute the oriented bounding box of a single point instance.
    ///
    /// Matches C++ `ComputePointInstanceLocalBound()`.
    pub fn compute_point_instance_local_bound(
        &mut self,
        instancer: &PointInstancer,
        instance_id: i64,
    ) -> BBox3d {
        let result = self.compute_point_instance_local_bounds(instancer, &[instance_id]);
        result.into_iter().next().unwrap_or_else(BBox3d::new)
    }

    /// Compute bounds of given point instances without the instancer's transform.
    ///
    /// Matches C++ `ComputePointInstanceUntransformedBounds()`.
    pub fn compute_point_instance_untransformed_bounds(
        &mut self,
        instancer: &PointInstancer,
        instance_ids: &[i64],
    ) -> Vec<BBox3d> {
        self.compute_point_instance_bounds_helper(instancer, instance_ids, &Matrix4d::identity())
    }

    /// Compute the untransformed bound of a single point instance.
    ///
    /// Matches C++ `ComputePointInstanceUntransformedBound()`.
    pub fn compute_point_instance_untransformed_bound(
        &mut self,
        instancer: &PointInstancer,
        instance_id: i64,
    ) -> BBox3d {
        let result = self.compute_point_instance_untransformed_bounds(instancer, &[instance_id]);
        result.into_iter().next().unwrap_or_else(BBox3d::new)
    }

    /// Clears all pre-cached values.
    ///
    /// Matches C++ `Clear()`.
    pub fn clear(&mut self) {
        self.bounds_cache.clear();
        self.xform_cache.clear();
    }

    /// Set the base time for point instancer bounds computation.
    ///
    /// Matches C++ `SetBaseTime(UsdTimeCode baseTime)`.
    pub fn set_base_time(&mut self, base_time: TimeCode) {
        self.base_time = Some(base_time);
    }

    /// Return the base time if set, otherwise `get_time()`.
    ///
    /// Matches C++ `GetBaseTime()`.
    pub fn get_base_time(&self) -> TimeCode {
        self.base_time.unwrap_or(self.time)
    }

    /// Clear the base time. Cache will use its time as base time.
    ///
    /// Matches C++ `ClearBaseTime()`.
    pub fn clear_base_time(&mut self) {
        self.base_time = None;
    }

    /// Return true if a base time has been explicitly set.
    ///
    /// Matches C++ `HasBaseTime()`.
    pub fn has_base_time(&self) -> bool {
        self.base_time.is_some()
    }

    /// Set the included purposes.
    ///
    /// Matches C++ `SetIncludedPurposes()`.
    pub fn set_included_purposes(&mut self, included_purposes: Vec<Token>) {
        self.included_purposes = included_purposes;
        // Note: Changing purposes doesn't invalidate cache in C++ implementation
    }

    /// Get the current set of included purposes.
    ///
    /// Matches C++ `GetIncludedPurposes()`.
    pub fn get_included_purposes(&self) -> &[Token] {
        &self.included_purposes
    }

    /// Returns whether authored extent hints are used.
    ///
    /// Matches C++ `GetUseExtentsHint()`.
    pub fn get_use_extents_hint(&self) -> bool {
        self.use_extents_hint
    }

    /// Returns whether prim visibility should be ignored.
    ///
    /// Matches C++ `GetIgnoreVisibility()`.
    pub fn get_ignore_visibility(&self) -> bool {
        self.ignore_visibility
    }

    /// Use the new time when computing values.
    ///
    /// Matches C++ `SetTime()`.
    pub fn set_time(&mut self, time: TimeCode) {
        if self.time != time {
            self.time = time;
            self.xform_cache.set_time(time);
            // Clear bounds cache when time changes
            self.bounds_cache.clear();
        }
    }

    /// Get the current time.
    ///
    /// Matches C++ `GetTime()`.
    pub fn get_time(&self) -> TimeCode {
        self.time
    }

    // ========================================================================
    // Private Helpers
    // ========================================================================

    /// Internal implementation of compute_world_bound.
    /// Resolves the untransformed (object-space) bound for a prim.
    /// This is the equivalent of C++ `_Resolve` + `_GetCombinedBBoxForIncludedPurposes`.
    /// No transforms are applied to the result.
    fn resolve_untransformed_bound(&mut self, prim: &Prim) -> BBox3d {
        let imageable = Imageable::new(prim.clone());

        // Non-Imageable prims like the pseudo-root or typeless defs still need to
        // aggregate descendant bounds; visibility/purpose filtering only applies
        // when the prim actually satisfies the Imageable schema.
        if imageable.is_valid() && !self.ignore_visibility {
            let visibility = imageable.compute_visibility(self.time);
            if visibility == usd_geom_tokens().invisible {
                return BBox3d::new();
            }
        }

        if imageable.is_valid() {
            let purpose = imageable.compute_purpose();
            if !self.included_purposes.is_empty() && !self.included_purposes.contains(&purpose) {
                return BBox3d::new();
            }
        }

        if !prim.is_valid() {
            return BBox3d::new();
        }

        // M-2: Try extents hint from ModelAPI if enabled
        if self.use_extents_hint && prim.is_model() {
            if let Some(hint_bbox) = self.get_bbox_from_extents_hint(prim) {
                return hint_bbox;
            }
        }

        // Try to get extent from Boundable
        let boundable = Boundable::new(prim.clone());
        let mut bbox = BBox3d::new();

        if boundable.is_valid() {
            let extent_attr = boundable.get_extent_attr();
            if extent_attr.is_valid() {
                if let Some(extent_value) = extent_attr.get(self.time) {
                    if let Some(extent_array) = extent_value.as_vec_clone::<Vec3f>() {
                        if extent_array.len() >= 2 {
                            let min = extent_array[0];
                            let max = extent_array[1];
                            let min_vec = Vec3d::new(min.x as f64, min.y as f64, min.z as f64);
                            let max_vec = Vec3d::new(max.x as f64, max.y as f64, max.z as f64);
                            let range = Range3d::new(min_vec, max_vec);
                            bbox = BBox3d::from_range(range);
                        }
                    }
                }
            }

            if bbox.range().is_empty() {
                if let Some(computed_extent) = boundable.compute_extent(self.time) {
                    if computed_extent.len() >= 2 {
                        let min = computed_extent[0];
                        let max = computed_extent[1];
                        let min_vec = Vec3d::new(min.x as f64, min.y as f64, min.z as f64);
                        let max_vec = Vec3d::new(max.x as f64, max.y as f64, max.z as f64);
                        let range = Range3d::new(min_vec, max_vec);
                        bbox = BBox3d::from_range(range);
                    }
                }
            }
        }

        // If still empty, try to compute from children
        if bbox.range().is_empty() {
            bbox = self.compute_bound_from_children(prim);
        }

        bbox
    }

    /// Compute world bound by resolving untransformed bound + full CTM.
    fn compute_world_bound_internal(&mut self, prim: &Prim) -> BBox3d {
        let mut bbox = self.resolve_untransformed_bound(prim);
        let local_to_world = self.xform_cache.get_local_to_world_transform(prim);
        bbox.transform(&local_to_world);
        bbox
    }

    /// Compute bound from children, skipping specified subtrees and applying overrides.
    fn compute_bound_from_children_with_skip(
        &mut self,
        prim: &Prim,
        paths_to_skip: &HashSet<Path>,
        ctm_overrides: &HashMap<Path, Matrix4d>,
    ) -> BBox3d {
        let mut combined_bbox = BBox3d::new();
        let mut has_any_bounds = false;

        for child in prim.children() {
            let child_path = child.path().clone();

            // Skip subtrees rooted at specified paths
            if paths_to_skip.contains(&child_path) {
                continue;
            }

            let child_imageable = Imageable::new(child.clone());

            // Non-Imageable children may still contain Imageable descendants,
            // so only apply visibility/purpose pruning when the child is
            // actually Imageable.
            if child_imageable.is_valid() && !self.ignore_visibility {
                let visibility = child_imageable.compute_visibility(self.time);
                if visibility == usd_geom_tokens().invisible {
                    continue;
                }
            }

            if child_imageable.is_valid() {
                let purpose = child_imageable.compute_purpose();
                if !self.included_purposes.is_empty() && !self.included_purposes.contains(&purpose)
                {
                    continue;
                }
            }

            // Compute child's world bound, using CTM override if available
            let child_world_bbox = if ctm_overrides.contains_key(&child_path) {
                // Use override CTM for this child's subtree
                let override_ctm = ctm_overrides[&child_path];
                let child_local = self.compute_untransformed_bound(&child);
                let mut result = child_local;
                result.transform(&override_ctm);
                result
            } else {
                self.compute_world_bound(&child)
            };

            if !child_world_bbox.range().is_empty() {
                if has_any_bounds {
                    combined_bbox = BBox3d::combine(&combined_bbox, &child_world_bbox);
                } else {
                    combined_bbox = child_world_bbox;
                    has_any_bounds = true;
                }
            }
        }

        combined_bbox
    }

    /// Compute bound from children, with prototype resolution.
    ///
    /// C++ bboxCache.cpp:1282-1483: iterates children, resolves each child's
    /// local-space bbox, transforms it by child's local transform into parent
    /// space, then unions all results. This function returns bounds in the
    /// PARENT's local space (not world space) because the caller
    /// (`resolve_untransformed_bound`) expects untransformed (local) bounds.
    ///
    /// Previously this called `compute_world_bound()` on children, which
    /// returned world-space bboxes. The caller then applied parent's
    /// local-to-world again, causing double-transformation.
    fn compute_bound_from_children(&mut self, prim: &Prim) -> BBox3d {
        let mut combined_bbox = BBox3d::new();
        let mut has_any_bounds = false;

        // If the prim is an instance, resolve its prototype and compute
        // the prototype's bounds instead of traversing (empty) instance children.
        // C++ bboxCache.cpp:1313-1324
        let children = if prim.is_instance() {
            let prototype = prim.get_prototype();
            if prototype.is_valid() {
                prototype.children()
            } else {
                Vec::new()
            }
        } else {
            prim.children()
        };

        for child in &children {
            let child_imageable = Imageable::new(child.clone());

            // C++ applies Imageable filtering only to Imageable prims; typeless
            // defs and other namespace prims must still contribute descendant bounds.
            if child_imageable.is_valid() && !self.ignore_visibility {
                let visibility = child_imageable.compute_visibility(self.time);
                if visibility == usd_geom_tokens().invisible {
                    continue;
                }
            }

            if child_imageable.is_valid() {
                let purpose = child_imageable.compute_purpose();
                if !self.included_purposes.is_empty() && !self.included_purposes.contains(&purpose)
                {
                    continue;
                }
            }

            // Get child's local-space (untransformed) bbox, then transform
            // by child's LOCAL transform (child-to-parent), not world transform.
            // C++ bboxCache.cpp:1454-1474: childEntry->bboxes transformed by
            // childLocalToComponentXform (which is child's local xform in the
            // simple non-component case).
            let mut child_bbox = self.resolve_untransformed_bound(child);
            if !child_bbox.range().is_empty() {
                // Transform child bbox from child-local to parent-local space
                let (child_local_xform, _resets_stack) =
                    self.xform_cache.get_local_transformation(child);
                child_bbox.transform(&child_local_xform);

                if has_any_bounds {
                    combined_bbox = BBox3d::combine(&combined_bbox, &child_bbox);
                } else {
                    combined_bbox = child_bbox;
                    has_any_bounds = true;
                }
            }
        }

        combined_bbox
    }

    /// Helper for computing per-instance bounds from a PointInstancer.
    ///
    /// Computes instance transforms and combines them with prototype bounds,
    /// then applies the given `xform` to each result.
    ///
    /// Matches C++ `_ComputePointInstanceBoundsHelper()`.
    fn compute_point_instance_bounds_helper(
        &mut self,
        instancer: &PointInstancer,
        instance_ids: &[i64],
        xform: &Matrix4d,
    ) -> Vec<BBox3d> {
        use super::point_instancer::ProtoXformInclusion;

        if instance_ids.is_empty() {
            return Vec::new();
        }

        let base_time = self.get_base_time();

        // Compute instance transforms at the requested time
        let mut xforms = Vec::new();
        let ok = instancer.compute_instance_transforms_at_time(
            &mut xforms,
            self.time,
            base_time,
            ProtoXformInclusion::IncludeProtoXform,
            super::point_instancer::MaskApplication::IgnoreMask,
        );

        if !ok || xforms.is_empty() {
            return vec![BBox3d::new(); instance_ids.len()];
        }

        // Get prototype paths to compute prototype bounds
        let proto_rel = instancer.get_prototypes_rel();
        let proto_paths = proto_rel.get_targets();
        let proto_indices_attr = instancer.get_proto_indices_attr();
        let proto_indices: Vec<i32> = proto_indices_attr
            .get(self.time)
            .and_then(|v| v.as_vec_clone::<i32>())
            .unwrap_or_default();

        // Pre-compute prototype bounds
        let stage = match instancer.prim().stage() {
            Some(s) => s,
            None => return vec![BBox3d::new(); instance_ids.len()],
        };
        let proto_bounds: Vec<BBox3d> = proto_paths
            .iter()
            .map(|path| {
                if let Some(proto_prim) = stage.get_prim_at_path(path) {
                    self.compute_untransformed_bound(&proto_prim)
                } else {
                    BBox3d::new()
                }
            })
            .collect();

        // Map each requested instance ID to its bound
        let num_instances = proto_indices.len();
        instance_ids
            .iter()
            .map(|&id| {
                let idx = id as usize;
                if idx >= num_instances {
                    return BBox3d::new();
                }

                let proto_idx = proto_indices[idx] as usize;
                if proto_idx >= proto_bounds.len() {
                    return BBox3d::new();
                }

                // Combine prototype bound with instance transform
                let proto_bbox = &proto_bounds[proto_idx];
                if proto_bbox.range().is_empty() {
                    return BBox3d::new();
                }

                let mut result = *proto_bbox;
                if idx < xforms.len() {
                    result.transform(&xforms[idx]);
                }
                result.transform(xform);
                result
            })
            .collect()
    }

    /// Try to get bounding box from extents hint (M-2).
    ///
    /// Matches C++ `_GetBBoxFromExtentsHint()`. Uses `UsdGeomModelAPI::GetExtentsHint()`
    /// to read cached extents for model-root prims.
    fn get_bbox_from_extents_hint(&mut self, prim: &Prim) -> Option<BBox3d> {
        // ModelAPI wraps the prim -- check if prim is valid
        if !prim.is_valid() {
            return None;
        }
        let model_api = ModelAPI::new(prim.clone());

        // ExtentsHint is a flat array of Vec3f pairs, ordered by purpose:
        // [default_min, default_max, render_min, render_max, proxy_min, proxy_max, guide_min, guide_max]
        let hint = model_api.get_extents_hint(self.time)?;
        if hint.len() < 2 {
            return None;
        }

        // Purpose order in extentsHint: default=0, render=1, proxy=2, guide=3
        let purpose_indices: Vec<usize> = self
            .included_purposes
            .iter()
            .filter_map(|p| {
                let s = p.as_str();
                match s {
                    "default" => Some(0),
                    "render" => Some(1),
                    "proxy" => Some(2),
                    "guide" => Some(3),
                    _ => None,
                }
            })
            .collect();

        // Combine hint ranges for all included purposes
        let mut combined: Option<Range3d> = None;

        for idx in &purpose_indices {
            let base = idx * 2;
            if base + 1 < hint.len() {
                let mn = hint[base];
                let mx = hint[base + 1];
                let r = Range3d::new(
                    Vec3d::new(mn.x as f64, mn.y as f64, mn.z as f64),
                    Vec3d::new(mx.x as f64, mx.y as f64, mx.z as f64),
                );
                if !r.is_empty() {
                    match combined.as_mut() {
                        Some(c) => c.union_with(&r),
                        None => combined = Some(r),
                    }
                }
            }
        }

        combined
            .filter(|c| !c.is_empty())
            .map(|c| BBox3d::from_range(c))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bbox_cache_base_time() {
        let mut cache = BBoxCache::new(
            TimeCode::default(),
            vec![Token::new("default")],
            false,
            false,
        );
        // Initially no base time
        assert!(!cache.has_base_time());
        assert_eq!(cache.get_base_time(), TimeCode::default());

        // Set base time
        cache.set_base_time(TimeCode::from(10.0));
        assert!(cache.has_base_time());
        assert_eq!(cache.get_base_time(), TimeCode::from(10.0));

        // Clear base time
        cache.clear_base_time();
        assert!(!cache.has_base_time());
    }

    #[test]
    fn test_bbox_cache_entry_tracking() {
        let entry = BBoxEntry::new();
        assert!(!entry.is_complete);
        assert!(!entry.is_varying);
        assert!(!entry.is_included);
        assert!(entry.bbox.range().is_empty());
    }

    #[test]
    fn test_bbox_cache_invalid_prim() {
        let mut cache = BBoxCache::new(
            TimeCode::default(),
            vec![Token::new("default")],
            true,
            false,
        );
        let invalid_prim = Prim::invalid();
        let bbox = cache.compute_world_bound(&invalid_prim);
        assert!(bbox.range().is_empty());
    }

    #[test]
    fn test_bbox_cache_use_extents_hint_flag() {
        let cache = BBoxCache::new(TimeCode::default(), vec![], true, false);
        assert!(cache.get_use_extents_hint());

        let cache2 = BBoxCache::new(TimeCode::default(), vec![], false, false);
        assert!(!cache2.get_use_extents_hint());
    }

    #[test]
    fn test_compute_relative_bound_invalid() {
        let mut cache = BBoxCache::new(
            TimeCode::default(),
            vec![Token::new("default")],
            false,
            false,
        );
        let invalid = Prim::invalid();
        // Both invalid returns empty
        let bbox = cache.compute_relative_bound(&invalid, &invalid);
        assert!(bbox.range().is_empty());
    }

    #[test]
    fn test_compute_untransformed_bound_with_overrides_invalid() {
        use std::collections::HashSet;
        let mut cache = BBoxCache::new(
            TimeCode::default(),
            vec![Token::new("default")],
            false,
            false,
        );
        let invalid = Prim::invalid();
        let skip = HashSet::new();
        let overrides = HashMap::new();
        let bbox = cache.compute_untransformed_bound_with_overrides(&invalid, &skip, &overrides);
        assert!(bbox.range().is_empty());
    }

    #[test]
    fn test_compute_relative_bound_api_exists() {
        // Verify the compute_relative_bound method signature compiles
        let mut cache = BBoxCache::new(
            TimeCode::from(1.0),
            vec![Token::new("default"), Token::new("render")],
            true,
            false,
        );
        let p = Prim::invalid();
        let _bbox = cache.compute_relative_bound(&p, &p);
        // Method exists and returns BBox3d
    }

    #[test]
    fn test_compute_untransformed_bound_with_overrides_api() {
        use std::collections::HashSet;
        use usd_gf::Matrix4d;
        let mut cache = BBoxCache::new(
            TimeCode::default(),
            vec![Token::new("default")],
            false,
            false,
        );
        let p = Prim::invalid();
        let mut skip = HashSet::new();
        skip.insert(Path::from("/some/child"));
        let mut overrides = HashMap::new();
        overrides.insert(Path::from("/some/other"), Matrix4d::identity());
        let bbox = cache.compute_untransformed_bound_with_overrides(&p, &skip, &overrides);
        assert!(bbox.range().is_empty());
    }
}
