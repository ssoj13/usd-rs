//! HdDirtyList - Cached list of dirty rprims for efficient sync iteration.
//!
//! Corresponds to pxr/imaging/hd/dirtyList.h.
//! Uses versioning to avoid recomputation when nothing changed.

use super::change_tracker::HdChangeTracker;
use super::prim::HdReprSelector;
use super::prim_gather::HdPrimGather;
use parking_lot::RwLock;
use std::sync::Arc;
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

/// Path vector.
pub type SdfPathVector = Vec<SdfPath>;

/// Repr selector vector.
pub type HdReprSelectorVector = Vec<HdReprSelector>;

/// Provides data for dirty list. Implemented by HdRenderIndex.
pub trait HdDirtyListDataSource: Send + Sync {
    /// Get all rprim ids (sorted).
    fn get_rprim_ids(&self) -> SdfPathVector;

    /// Get change tracker.
    fn get_change_tracker(&self) -> &HdChangeTracker;

    /// Update render tag for prim (optional, for render-tag filtering).
    fn update_render_tag(&self, _prim_id: &SdfPath, _dirty_bits: u32) -> Token {
        Token::default()
    }
}

/// Cached dirty rprim list for fast sync iteration.
///
/// Corresponds to C++ `HdDirtyList`.
/// GetDirtyRprims returns empty if nothing changed; otherwise returns ids to sync.
pub struct HdDirtyList<D: HdDirtyListDataSource> {
    data_source: Arc<D>,
    tracked_render_tags: RwLock<Vec<Token>>,
    tracked_reprs: RwLock<HdReprSelectorVector>,
    dirty_ids: RwLock<SdfPathVector>,
    scene_state_version: RwLock<u32>,
    rprim_index_version: RwLock<u32>,
    rprim_render_tag_version: RwLock<u32>,
    varying_state_version: RwLock<u32>,
    rebuild_dirty_list: RwLock<bool>,
    prune_dirty_list: RwLock<bool>,
}

impl<D: HdDirtyListDataSource> HdDirtyList<D> {
    /// Create new dirty list.
    pub fn new(data_source: Arc<D>) -> Self {
        let (scene_ver, rprim_idx_ver, render_tag_ver, varying_ver) = {
            let ct = data_source.get_change_tracker();
            (
                ct.get_scene_state_version().wrapping_sub(1),
                ct.get_rprim_index_version().wrapping_sub(1),
                ct.get_render_tag_version().wrapping_sub(1),
                ct.get_varying_state_version().wrapping_sub(1),
            )
        };
        Self {
            data_source,
            tracked_render_tags: RwLock::new(Vec::new()),
            tracked_reprs: RwLock::new(Vec::new()),
            dirty_ids: RwLock::new(Vec::new()),
            scene_state_version: RwLock::new(scene_ver),
            rprim_index_version: RwLock::new(rprim_idx_ver),
            rprim_render_tag_version: RwLock::new(render_tag_ver),
            varying_state_version: RwLock::new(varying_ver),
            rebuild_dirty_list: RwLock::new(false),
            prune_dirty_list: RwLock::new(false),
        }
    }

    /// Get dirty rprim ids. Returns empty if nothing changed.
    pub fn get_dirty_rprims(&self) -> parking_lot::RwLockReadGuard<'_, SdfPathVector> {
        self.update_dirty_ids_if_needed();
        self.dirty_ids.read()
    }

    /// Update render tags and repr selectors from tasks.
    pub fn update_render_tags_and_repr_selectors(&self, tags: &[Token], reprs: &[HdReprSelector]) {
        let mut tracked = self.tracked_render_tags.write();
        for tag in tags {
            if !tracked.contains(tag) {
                tracked.push(tag.clone());
            }
        }
        *self.tracked_reprs.write() = reprs.to_vec();
    }

    /// Prune to varying rprims on next GetDirtyRprims.
    pub fn prune_to_varying_rprims(&self) {
        *self.prune_dirty_list.write() = true;
    }

    fn update_dirty_ids_if_needed(&self) {
        let ct = self.data_source.get_change_tracker();
        let current_scene = ct.get_scene_state_version();
        let scene_guard = self.scene_state_version.read();
        let scene_ok = *scene_guard == current_scene;
        let prune = *self.prune_dirty_list.read();
        let rebuild = *self.rebuild_dirty_list.read();
        drop(scene_guard);

        if scene_ok && !prune && !rebuild {
            return;
        }

        *self.scene_state_version.write() = current_scene;
        let rprim_ids = self.data_source.get_rprim_ids();
        let tracked_tags = self.tracked_render_tags.read().clone();

        let mask = if rebuild
            || *self.rprim_index_version.read() != ct.get_rprim_index_version()
            || *self.rprim_render_tag_version.read() != ct.get_render_tag_version()
        {
            *self.rprim_index_version.write() = ct.get_rprim_index_version();
            *self.rprim_render_tag_version.write() = ct.get_render_tag_version();
            *self.varying_state_version.write() = ct.get_varying_state_version();
            *self.rebuild_dirty_list.write() = false;
            *self.prune_dirty_list.write() = true;
            super::change_tracker::HdRprimDirtyBits::CLEAN
        } else if *self.varying_state_version.read() != ct.get_varying_state_version() {
            *self.varying_state_version.write() = ct.get_varying_state_version();
            *self.prune_dirty_list.write() = false;
            super::change_tracker::HdRprimDirtyBits::VARYING
        } else {
            return;
        };

        let include_paths = vec![SdfPath::absolute_root()];
        let exclude_paths: SdfPathVector = Vec::new();
        let ds = Arc::clone(&self.data_source);

        let mut results = Vec::new();
        HdPrimGather::predicated_filter(
            &rprim_ids,
            &include_paths,
            &exclude_paths,
            |path: &SdfPath| {
                let bits = ct.get_rprim_dirty_bits(path);
                if mask == super::change_tracker::HdRprimDirtyBits::CLEAN || (bits & mask) != 0 {
                    if tracked_tags.is_empty() {
                        return true;
                    }
                    let prim_tag = ds.update_render_tag(path, bits);
                    tracked_tags.iter().any(|t| *t == prim_tag)
                } else {
                    false
                }
            },
            &mut results,
        );
        *self.dirty_ids.write() = results;
    }
}
