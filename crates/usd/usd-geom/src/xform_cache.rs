//! UsdGeomXformCache - caching mechanism for transform matrices.
//!
//! Port of pxr/usd/usdGeom/xformCache.h/cpp
//!
//! A caching mechanism for transform matrices. For best performance, this
//! object should be reused for multiple CTM queries.

use super::xformable::{XformQuery, Xformable};
use std::collections::HashMap;
use std::sync::Weak;
use usd_core::Prim;
use usd_gf::Matrix4d;
use usd_sdf::{Path, TimeCode};
use usd_tf::Token;

// ============================================================================
// XformCache
// ============================================================================

/// A caching mechanism for transform matrices.
///
/// Matches C++ `UsdGeomXformCache`.
///
/// WARNING: This class does not automatically invalidate cached values based
/// on changes to the stage. Additionally, a separate instance should be used
/// per-thread, as calling Get* methods from multiple threads is not safe.
pub struct XformCache {
    /// Map of cached values (prim path -> entry).
    ctm_cache: HashMap<Path, Entry>,
    /// The time at which this cache is querying and caching attribute values.
    time: TimeCode,
}

/// Cache entry for a prim.
struct Entry {
    /// Cached XformQuery for the prim.
    query: Option<XformQuery>,
    /// Weak reference to the stage this entry was created from.
    stage: Weak<usd_core::Stage>,
    /// Cached CTM (composite transform matrix).
    ctm: Matrix4d,
    /// Whether CTM is valid.
    ctm_is_valid: bool,
}

impl Entry {
    fn new() -> Self {
        Self {
            query: None,
            stage: Weak::new(),
            ctm: Matrix4d::identity(),
            ctm_is_valid: false,
        }
    }

    /// Ensure the cached XformQuery matches the given prim's stage.
    /// If the stage changed, invalidate the query so it gets rebuilt.
    fn ensure_query(&mut self, prim: &Prim) {
        let stage_matches = if let Some(prim_stage) = prim.stage() {
            if let Some(entry_stage) = self.stage.upgrade() {
                std::sync::Arc::ptr_eq(&entry_stage, &prim_stage)
            } else {
                false
            }
        } else {
            false
        };

        if self.query.is_some() && !stage_matches {
            self.query = None;
            self.ctm_is_valid = false;
        }

        if self.query.is_none() {
            let xformable = Xformable::new(prim.clone());
            if xformable.is_valid() {
                self.query = Some(XformQuery::from_xformable(&xformable));
                if let Some(s) = prim.stage() {
                    self.stage = std::sync::Arc::downgrade(&s);
                }
            }
        }
    }
}

impl XformCache {
    /// Construct a new XformCache for the specified time.
    ///
    /// Matches C++ `UsdGeomXformCache(UsdTimeCode time)`.
    pub fn new(time: TimeCode) -> Self {
        Self {
            ctm_cache: HashMap::new(),
            time,
        }
    }

    /// Construct a new XformCache for default time.
    ///
    /// Matches C++ default constructor.
    pub fn default() -> Self {
        Self::new(TimeCode::default())
    }

    /// Compute the transformation matrix for the given prim, including the
    /// transform authored on the Prim itself, if present.
    ///
    /// Matches C++ `GetLocalToWorldTransform()`.
    pub fn get_local_to_world_transform(&mut self, prim: &Prim) -> Matrix4d {
        if !prim.is_valid() {
            return Matrix4d::identity();
        }

        let key = prim.path().clone();

        // Check if already cached AND from the same stage
        if let Some(entry) = self.ctm_cache.get(&key) {
            if entry.ctm_is_valid {
                let stage_ok = if let Some(prim_stage) = prim.stage() {
                    if let Some(entry_stage) = entry.stage.upgrade() {
                        std::sync::Arc::ptr_eq(&entry_stage, &prim_stage)
                    } else {
                        false
                    }
                } else {
                    false
                };
                if stage_ok {
                    return entry.ctm;
                }
            }
        }

        // Compute CTM by traversing up the hierarchy
        let ctm = self.compute_ctm(prim);

        // Store in cache
        let entry = self.ctm_cache.entry(key).or_insert_with(Entry::new);
        entry.ctm = ctm;
        entry.ctm_is_valid = true;
        if let Some(s) = prim.stage() {
            entry.stage = std::sync::Arc::downgrade(&s);
        }
        ctm
    }

    /// Compute the transformation matrix for the given prim, but do NOT
    /// include the transform authored on the prim itself.
    ///
    /// Matches C++ `GetParentToWorldTransform()`.
    pub fn get_parent_to_world_transform(&mut self, prim: &Prim) -> Matrix4d {
        if !prim.is_valid() {
            return Matrix4d::identity();
        }

        let parent = prim.parent();
        if !parent.is_valid() {
            return Matrix4d::identity();
        }

        self.get_local_to_world_transform(&parent)
    }

    /// Returns the local transformation of the prim.
    ///
    /// Matches C++ `GetLocalTransformation()`.
    pub fn get_local_transformation(&mut self, prim: &Prim) -> (Matrix4d, bool) {
        if !prim.is_valid() {
            return (Matrix4d::identity(), false);
        }

        let key = prim.path().clone();
        let entry = self.ctm_cache.entry(key).or_insert_with(Entry::new);
        entry.ensure_query(prim);

        if let Some(ref query) = entry.query {
            if let Some(transform) = query.get_local_transformation(self.time) {
                let resets_xform_stack = query.get_reset_xform_stack();
                return (transform, resets_xform_stack);
            }
        }
        (Matrix4d::identity(), false)
    }

    /// Whether the attribute named attr_name affects the local transform value.
    ///
    /// Matches C++ `IsAttributeIncludedInLocalTransform()`.
    pub fn is_attribute_included_in_local_transform(
        &mut self,
        prim: &Prim,
        attr_name: &Token,
    ) -> bool {
        if !prim.is_valid() {
            return false;
        }

        let key = prim.path().clone();
        let entry = self.ctm_cache.entry(key).or_insert_with(Entry::new);
        entry.ensure_query(prim);

        if let Some(ref query) = entry.query {
            return query.is_attribute_included_in_local_transform(attr_name);
        }
        false
    }

    /// Whether the local transformation value at the prim may vary over time.
    ///
    /// Matches C++ `TransformMightBeTimeVarying()`.
    pub fn transform_might_be_time_varying(&mut self, prim: &Prim) -> bool {
        if !prim.is_valid() {
            return false;
        }

        let key = prim.path().clone();
        let entry = self.ctm_cache.entry(key).or_insert_with(Entry::new);
        entry.ensure_query(prim);

        if let Some(ref query) = entry.query {
            return query.transform_might_be_time_varying();
        }
        false
    }

    /// Whether the xform stack is reset at the given prim.
    ///
    /// Matches C++ `GetResetXformStack()`.
    pub fn get_reset_xform_stack(&mut self, prim: &Prim) -> bool {
        if !prim.is_valid() {
            return false;
        }

        let key = prim.path().clone();
        let entry = self.ctm_cache.entry(key).or_insert_with(Entry::new);
        entry.ensure_query(prim);

        if let Some(ref query) = entry.query {
            return query.get_reset_xform_stack();
        }
        false
    }

    /// Compute the relative transform from `prim` to `ancestor`.
    /// Multiplies local transforms from prim up to (but not including) ancestor.
    ///
    /// Matches C++ `ComputeRelativeTransform()`.
    pub fn compute_relative_transform(&mut self, prim: &Prim, ancestor: &Prim) -> (Matrix4d, bool) {
        let mut xform = Matrix4d::identity();
        let ancestor_path = ancestor.path().clone();
        let mut cur = prim.clone();

        while cur.is_valid() && cur.path() != &ancestor_path {
            let (local, resets) = self.get_local_transformation(&cur);
            xform = xform * local;
            if resets {
                return (xform, true);
            }
            cur = cur.parent();
        }
        (xform, false)
    }

    /// Clears all pre-cached values.
    ///
    /// Matches C++ `Clear()`.
    pub fn clear(&mut self) {
        self.ctm_cache.clear();
    }

    /// Use the new time when computing values and may clear any existing
    /// values cached for the previous time.
    ///
    /// C++ xformCache.cpp:170-182: iterates all entries and sets
    /// `ctmIsValid = false`, preserving the cached XformQuery objects.
    pub fn set_time(&mut self, time: TimeCode) {
        if self.time != time {
            self.time = time;
            for entry in self.ctm_cache.values_mut() {
                entry.ctm_is_valid = false;
            }
        }
    }

    /// Get the current time from which this cache is reading values.
    ///
    /// Matches C++ `GetTime()`.
    pub fn get_time(&self) -> TimeCode {
        self.time
    }

    /// Swap the contents of this XformCache with other.
    ///
    /// Matches C++ `Swap()`.
    pub fn swap(&mut self, other: &mut XformCache) {
        std::mem::swap(&mut self.ctm_cache, &mut other.ctm_cache);
        std::mem::swap(&mut self.time, &mut other.time);
    }

    // ========================================================================
    // Private Helpers
    // ========================================================================

    /// Traverses backwards the hierarchy starting from prim all the way to
    /// the root and computes the CTM (composite transform matrix).
    fn compute_ctm(&mut self, prim: &Prim) -> Matrix4d {
        if !prim.is_valid() {
            return Matrix4d::identity();
        }

        let (local_transform, resets_stack) = self.get_local_transformation_internal(prim);

        if resets_stack {
            return local_transform;
        }

        let parent = prim.parent();
        if !parent.is_valid() {
            return local_transform;
        }

        let parent_ctm = self.get_local_to_world_transform(&parent);

        // Row-vector convention: world = local * parent (matches C++ xform * parentCtm)
        local_transform * parent_ctm
    }

    /// Internal helper to get local transformation without caching CTM.
    fn get_local_transformation_internal(&mut self, prim: &Prim) -> (Matrix4d, bool) {
        if !prim.is_valid() {
            return (Matrix4d::identity(), false);
        }

        let key = prim.path().clone();
        let entry = self.ctm_cache.entry(key).or_insert_with(Entry::new);
        entry.ensure_query(prim);

        if let Some(ref query) = entry.query {
            if let Some(transform) = query.get_local_transformation(self.time) {
                let resets_xform_stack = query.get_reset_xform_stack();
                return (transform, resets_xform_stack);
            }
        }

        (Matrix4d::identity(), false)
    }
}

impl Default for XformCache {
    fn default() -> Self {
        Self::default()
    }
}
