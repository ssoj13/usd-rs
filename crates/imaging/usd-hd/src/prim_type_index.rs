//! HdPrimTypeIndex - Manage prims by type for render index.
//!
//! Corresponds to pxr/imaging/hd/primTypeIndex.h.

use std::collections::HashMap;
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

/// Path vector type.
pub type SdfPathVector = Vec<SdfPath>;

/// Trait for prim type operations (delegate-specific).
///
/// Specialized per PrimType in C++.
pub trait HdPrimTypeIndexOps<PrimType> {
    /// Insert prim into change tracker.
    fn tracker_insert_prim(tracker: &mut dyn std::any::Any, path: &SdfPath, initial_dirty: u64);
    /// Remove prim from change tracker.
    fn tracker_remove_prim(tracker: &mut dyn std::any::Any, path: &SdfPath);
    /// Get prim dirty bits.
    fn tracker_get_prim_dirty_bits(tracker: &dyn std::any::Any, path: &SdfPath) -> u64;
    /// Mark prim clean.
    fn tracker_mark_prim_clean(tracker: &mut dyn std::any::Any, path: &SdfPath, dirty_bits: u64);
    /// Create prim via render delegate.
    fn render_delegate_create_prim(
        delegate: &dyn std::any::Any,
        type_id: &Token,
        prim_id: &SdfPath,
    ) -> Option<Box<PrimType>>;
    /// Create fallback prim.
    fn render_delegate_create_fallback_prim(
        delegate: &dyn std::any::Any,
        type_id: &Token,
    ) -> Option<Box<PrimType>>;
    /// Destroy prim.
    fn render_delegate_destroy_prim(delegate: &dyn std::any::Any, prim: Box<PrimType>);
}

/// Prim info entry (scene delegate stored as type param for flexibility).
struct PrimInfo<PrimType, SD> {
    _scene_delegate: SD,
    prim: Box<PrimType>,
}

/// Index of prims by type.
///
/// Corresponds to C++ `Hd_PrimTypeIndex<PrimType>`.
pub struct HdPrimTypeIndex<PrimType, SD = ()> {
    prim_types: Vec<Token>,
    type_index: HashMap<Token, usize>,
    entries: Vec<PrimTypeEntry<PrimType, SD>>,
}

struct PrimTypeEntry<PrimType, SD> {
    prim_map: HashMap<SdfPath, PrimInfo<PrimType, SD>>,
    #[allow(dead_code)] // C++ sorted iteration support, not yet wired
    prim_ids: super::sorted_ids::HdSortedIds,
    fallback_prim: Option<Box<PrimType>>,
}

impl<PrimType, SD> Default for HdPrimTypeIndex<PrimType, SD> {
    fn default() -> Self {
        Self {
            prim_types: Vec::new(),
            type_index: HashMap::new(),
            entries: Vec::new(),
        }
    }
}

impl<PrimType, SD> HdPrimTypeIndex<PrimType, SD> {
    /// Create empty index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Initialize prim types.
    pub fn init_prim_types(&mut self, prim_types: &[Token]) {
        self.prim_types = prim_types.to_vec();
        self.type_index.clear();
        for (i, t) in prim_types.iter().enumerate() {
            self.type_index.insert(t.clone(), i);
        }
        self.entries = (0..prim_types.len())
            .map(|_| PrimTypeEntry::<PrimType, SD> {
                prim_map: HashMap::new(),
                prim_ids: super::sorted_ids::HdSortedIds::new(),
                fallback_prim: None,
            })
            .collect();
    }

    /// Get prim by type and id.
    pub fn get_prim(&self, type_id: &Token, prim_id: &SdfPath) -> Option<&PrimType> {
        let idx = *self.type_index.get(type_id)?;
        let entry = self.entries.get(idx)?;
        entry.prim_map.get(prim_id).map(|i| i.prim.as_ref())
    }

    /// Get mutable prim by type and id.
    pub fn get_prim_mut(&mut self, type_id: &Token, prim_id: &SdfPath) -> Option<&mut PrimType> {
        let idx = *self.type_index.get(type_id)?;
        let entry = self.entries.get_mut(idx)?;
        entry.prim_map.get_mut(prim_id).map(|i| i.prim.as_mut())
    }

    /// Get fallback prim.
    pub fn get_fallback_prim(&self, type_id: &Token) -> Option<&PrimType> {
        let idx = *self.type_index.get(type_id)?;
        self.entries
            .get(idx)?
            .fallback_prim
            .as_ref()
            .map(|b| b.as_ref())
    }

    /// Get prim subtree (paths of prims of given type under root).
    pub fn get_prim_subtree(&self, type_id: &Token, root_path: &SdfPath) -> SdfPathVector {
        let idx = match self.type_index.get(type_id) {
            Some(i) => *i,
            None => return Vec::new(),
        };
        let entry = match self.entries.get(idx) {
            Some(e) => e,
            None => return Vec::new(),
        };
        let mut out = Vec::new();
        for (path, _) in &entry.prim_map {
            if path.has_prefix(root_path) {
                out.push(path.clone());
            }
        }
        out.sort();
        out
    }

    /// Number of registered prim types.
    pub fn num_prim_types(&self) -> usize {
        self.prim_types.len()
    }
}
