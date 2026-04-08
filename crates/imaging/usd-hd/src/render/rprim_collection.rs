//! HdRprimCollection - Named collection of rprims for render passes.
//!
//! Corresponds to pxr/imaging/hd/rprimCollection.h.
//! A collection identifies a group of rprims by root paths, exclude paths,
//! repr selector, and optional material tag.

use crate::prim::HdReprSelector;
use std::hash::{Hash, Hasher};
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

/// Path vector type.
pub type SdfPathVector = Vec<SdfPath>;

/// Named collection of rprims for a render pass.
///
/// Corresponds to C++ `HdRprimCollection`.
/// Does not hold rprim pointers; it acts as an addressing mechanism.
#[derive(Debug, Clone)]
pub struct HdRprimCollection {
    /// Semantic name (e.g. "visible", "selected").
    pub name: Token,
    /// Repr selector (refined, hull, points, etc.).
    pub repr_selector: HdReprSelector,
    /// If true, prims' authored repr is ignored.
    pub forced_repr: bool,
    /// Material tag filter (e.g. "translucent"). Empty = no filter.
    pub material_tag: Token,
    /// Root paths; prims must be rooted under one of these.
    pub root_paths: SdfPathVector,
    /// Paths to exclude from the collection.
    pub exclude_paths: SdfPathVector,
}

impl HdRprimCollection {
    /// Create with default repr selector and root at absolute root.
    pub fn new(name: Token) -> Self {
        Self {
            name,
            repr_selector: HdReprSelector::default(),
            forced_repr: false,
            material_tag: Token::default(),
            root_paths: vec![SdfPath::absolute_root()],
            exclude_paths: Vec::new(),
        }
    }

    /// Create with repr selector.
    pub fn with_repr(
        name: Token,
        repr_selector: HdReprSelector,
        forced_repr: bool,
        material_tag: Token,
    ) -> Self {
        Self {
            name,
            repr_selector,
            forced_repr,
            material_tag,
            root_paths: vec![SdfPath::absolute_root()],
            exclude_paths: Vec::new(),
        }
    }

    /// Create with root path.
    pub fn with_root(
        name: Token,
        repr_selector: HdReprSelector,
        root_path: SdfPath,
        forced_repr: bool,
        material_tag: Token,
    ) -> Self {
        let root_paths = if root_path.is_absolute_path() {
            vec![root_path]
        } else {
            vec![SdfPath::absolute_root()]
        };
        Self {
            name,
            repr_selector,
            forced_repr,
            material_tag,
            root_paths,
            exclude_paths: Vec::new(),
        }
    }

    /// Create inverse collection (root and exclude paths swapped).
    pub fn create_inverse_collection(&self) -> Self {
        let mut inv = self.clone();
        std::mem::swap(&mut inv.root_paths, &mut inv.exclude_paths);
        inv
    }

    /// Get repr selector.
    pub fn get_repr_selector(&self) -> &HdReprSelector {
        &self.repr_selector
    }

    /// Set repr selector.
    pub fn set_repr_selector(&mut self, repr_selector: HdReprSelector) {
        self.repr_selector = repr_selector;
    }

    /// Check if forced repr.
    pub fn is_forced_repr(&self) -> bool {
        self.forced_repr
    }

    /// Set forced repr flag.
    pub fn set_forced_repr(&mut self, flag: bool) {
        self.forced_repr = flag;
    }

    /// Get root paths (always sorted).
    pub fn get_root_paths(&self) -> &[SdfPath] {
        &self.root_paths
    }

    /// Set root paths. Paths must be absolute.
    pub fn set_root_paths(&mut self, paths: SdfPathVector) {
        let mut sorted = paths;
        sorted.sort();
        self.root_paths = sorted;
    }

    /// Set single root path.
    pub fn set_root_path(&mut self, path: SdfPath) {
        if path.is_absolute_path() {
            self.root_paths = vec![path];
        }
    }

    /// Set exclude paths.
    pub fn set_exclude_paths(&mut self, paths: SdfPathVector) {
        let mut sorted = paths;
        sorted.sort();
        self.exclude_paths = sorted;
    }

    /// Get exclude paths.
    pub fn get_exclude_paths(&self) -> &[SdfPath] {
        &self.exclude_paths
    }

    /// Set material tag.
    pub fn set_material_tag(&mut self, tag: Token) {
        self.material_tag = tag;
    }

    /// Get material tag.
    pub fn get_material_tag(&self) -> &Token {
        &self.material_tag
    }

    /// Compute hash for use in caches.
    pub fn compute_hash(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        std::hash::Hash::hash(self, &mut hasher);
        hasher.finish()
    }
}

impl Default for HdRprimCollection {
    fn default() -> Self {
        Self {
            name: Token::default(),
            repr_selector: HdReprSelector::default(),
            forced_repr: false,
            material_tag: Token::default(),
            root_paths: vec![SdfPath::absolute_root()],
            exclude_paths: Vec::new(),
        }
    }
}

impl PartialEq for HdRprimCollection {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.repr_selector == other.repr_selector
            && self.forced_repr == other.forced_repr
            && self.material_tag == other.material_tag
            && self.root_paths == other.root_paths
            && self.exclude_paths == other.exclude_paths
    }
}

impl Eq for HdRprimCollection {}

impl Hash for HdRprimCollection {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&self.name, state);
        std::hash::Hash::hash(&self.repr_selector, state);
        self.forced_repr.hash(state);
        std::hash::Hash::hash(&self.material_tag, state);
        for p in &self.root_paths {
            p.hash(state);
        }
        for p in &self.exclude_paths {
            p.hash(state);
        }
    }
}
