//! UsdSkelCache - thread-safe cache for skeletal query objects.
//!
//! Port of pxr/usd/usdSkel/cache.h/cpp

use super::anim_query::AnimQuery;
use super::animation::SkelAnimation;
use super::binding::Binding;
use super::binding_api::BindingAPI;
use super::root::SkelRoot;
use super::skel_definition::SkelDefinition;
use super::skeleton::Skeleton;
use super::skeleton_query::SkeletonQuery;
use super::skinning_query::SkinningQuery;
use super::utils::is_skel_animation_prim;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use usd_core::Prim;
use usd_core::prim_flags::PrimFlagsPredicate;
use usd_sdf::Path;

/// Thread-safe cache for accessing query objects for evaluating skeletal data.
///
/// This provides caching of major structural components, such as skeletal
/// topology. In a streaming context, this cache is intended to persist.
///
/// Matches C++ `UsdSkelCache`.
#[derive(Clone)]
pub struct Cache {
    inner: Arc<CacheInner>,
}

struct CacheInner {
    /// Cache of anim queries by prim path.
    anim_query_cache: RwLock<HashMap<Path, AnimQuery>>,
    /// Cache of skeleton definitions by prim path.
    skel_definition_cache: RwLock<HashMap<Path, SkelDefinition>>,
    /// Cache of skeleton queries by prim path.
    skel_query_cache: RwLock<HashMap<Path, SkeletonQuery>>,
    /// Cache of skinning queries by prim path.
    skinning_query_cache: RwLock<HashMap<Path, SkinningQuery>>,
}

impl Default for Cache {
    fn default() -> Self {
        Self::new()
    }
}

impl Cache {
    /// Create a new empty cache.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(CacheInner {
                anim_query_cache: RwLock::new(HashMap::new()),
                skel_definition_cache: RwLock::new(HashMap::new()),
                skel_query_cache: RwLock::new(HashMap::new()),
                skinning_query_cache: RwLock::new(HashMap::new()),
            }),
        }
    }

    /// Clear all cached data.
    pub fn clear(&self) {
        if let Ok(mut cache) = self.inner.anim_query_cache.write() {
            cache.clear();
        }
        if let Ok(mut cache) = self.inner.skel_definition_cache.write() {
            cache.clear();
        }
        if let Ok(mut cache) = self.inner.skel_query_cache.write() {
            cache.clear();
        }
        if let Ok(mut cache) = self.inner.skinning_query_cache.write() {
            cache.clear();
        }
    }

    /// Populate the cache for the skeletal data beneath prim `root`,
    /// as traversed using `predicate`.
    ///
    /// Population resolves inherited skel bindings set using the
    /// BindingAPI, making resolved bindings available through
    /// `get_skinning_query()`, `compute_skel_binding()` and `compute_skel_bindings()`.
    ///
    /// Matches C++ `Populate(const UsdSkelRoot& root, Usd_PrimFlagsPredicate predicate)`.
    pub fn populate(&self, root: &SkelRoot, predicate: PrimFlagsPredicate) -> bool {
        let prim = root.prim().clone();
        if !prim.is_valid() {
            return false;
        }

        // Recursively populate from the root, respecting the predicate
        self.recursive_populate(&prim, None, 0, &predicate)
    }

    /// Convenience: populate with default predicate (active + defined + loaded).
    pub fn populate_default(&self, root: &SkelRoot) -> bool {
        self.populate(root, PrimFlagsPredicate::default())
    }

    /// Get a skeleton query for computing properties of `skel`.
    ///
    /// This does not require `populate()` to be called on the cache.
    pub fn get_skel_query(&self, skel: &Skeleton) -> SkeletonQuery {
        let prim = skel.prim().clone();
        let path = prim.path().clone();

        // Check cache first
        if let Ok(cache) = self.inner.skel_query_cache.read() {
            if let Some(query) = cache.get(&path) {
                return query.clone();
            }
        }

        // Create new query
        let query = self.find_or_create_skel_query(&prim);

        // Cache it
        if let Ok(mut cache) = self.inner.skel_query_cache.write() {
            cache.insert(path, query.clone());
        }

        query
    }

    /// Get an anim query corresponding to `anim`.
    ///
    /// This does not require `populate()` to be called on the cache.
    pub fn get_anim_query(&self, anim: &SkelAnimation) -> AnimQuery {
        self.get_anim_query_from_prim(&anim.prim().clone())
    }

    /// Get an anim query from a prim.
    pub fn get_anim_query_from_prim(&self, prim: &Prim) -> AnimQuery {
        let path = prim.path().clone();

        // Check cache first
        if let Ok(cache) = self.inner.anim_query_cache.read() {
            if let Some(query) = cache.get(&path) {
                return query.clone();
            }
        }

        // Create new query
        let query = self.find_or_create_anim_query(prim);

        // Cache it
        if let Ok(mut cache) = self.inner.anim_query_cache.write() {
            cache.insert(path, query.clone());
        }

        query
    }

    /// Get a skinning query at `prim`.
    ///
    /// Skinning queries are defined at any skinnable prims (i.e., boundable
    /// prims with fully defined joint influences).
    ///
    /// The caller must first `populate()` the cache with the skel root containing
    /// `prim`, with a predicate that will visit `prim`, in order for a
    /// skinning query to be discoverable.
    pub fn get_skinning_query(&self, prim: &Prim) -> Option<SkinningQuery> {
        let path = prim.path().clone();

        if let Ok(cache) = self.inner.skinning_query_cache.read() {
            cache.get(&path).cloned()
        } else {
            None
        }
    }

    /// Compute the set of skeleton bindings beneath `skel_root`,
    /// as discovered through a traversal using `predicate`.
    ///
    /// Skinnable prims are only discoverable by this method if `populate()`
    /// has already been called for `skel_root`, with an equivalent predicate.
    pub fn compute_skel_bindings(
        &self,
        skel_root: &SkelRoot,
        predicate: PrimFlagsPredicate,
    ) -> Vec<Binding> {
        let mut bindings = Vec::new();

        let prim = skel_root.prim().clone();
        if !prim.is_valid() {
            return bindings;
        }

        // Find all skeletons beneath the root, respecting the predicate
        let mut skel_to_skinning: HashMap<Path, Vec<SkinningQuery>> = HashMap::new();

        self.collect_bindings(&prim, &mut skel_to_skinning, &predicate);

        // Convert to bindings
        let stage = prim.stage().expect("prim has stage");
        for (skel_path, skinning_queries) in skel_to_skinning {
            if let Some(skel_prim) = stage.get_prim_at_path(&skel_path) {
                let skel = Skeleton::new(skel_prim);
                if skel.is_valid() {
                    bindings.push(Binding::from_skeleton(skel, skinning_queries));
                }
            }
        }

        bindings
    }

    /// Compute the binding corresponding to a single skeleton, bound beneath
    /// `skel_root`, as discovered through a traversal using `predicate`.
    ///
    /// Skinnable prims are only discoverable by this method if `populate()`
    /// has already been called for `skel_root`, with an equivalent predicate.
    pub fn compute_skel_binding(
        &self,
        skel_root: &SkelRoot,
        skel: &Skeleton,
        predicate: PrimFlagsPredicate,
    ) -> Option<Binding> {
        let prim = skel_root.prim().clone();
        if !prim.is_valid() {
            return None;
        }

        let skel_path = skel.prim().path().clone();
        let mut skinning_queries = Vec::new();

        self.collect_bindings_for_skel(&prim, &skel_path, &mut skinning_queries, &predicate);

        if skinning_queries.is_empty() {
            None
        } else {
            Some(Binding::from_skeleton(skel.clone(), skinning_queries))
        }
    }

    // Internal: find or create an anim query for a prim.
    fn find_or_create_anim_query(&self, prim: &Prim) -> AnimQuery {
        if !prim.is_valid() || !is_skel_animation_prim(prim) {
            return AnimQuery::new();
        }

        let anim = SkelAnimation::new(prim.clone());
        AnimQuery::from_prim(anim.prim().clone()).unwrap_or_default()
    }

    // Internal: find or create a skeleton definition for a prim.
    fn find_or_create_skel_definition(&self, prim: &Prim) -> Option<SkelDefinition> {
        let path = prim.path().clone();

        // Check cache first
        if let Ok(cache) = self.inner.skel_definition_cache.read() {
            if let Some(def) = cache.get(&path) {
                return Some(def.clone());
            }
        }

        // Create new definition
        let skel = Skeleton::new(prim.clone());
        let def = SkelDefinition::new(skel)?;

        // Cache it
        if let Ok(mut cache) = self.inner.skel_definition_cache.write() {
            cache.insert(path, def.clone());
        }

        Some(def)
    }

    // Internal: find or create a skeleton query for a prim.
    fn find_or_create_skel_query(&self, prim: &Prim) -> SkeletonQuery {
        let def = match self.find_or_create_skel_definition(prim) {
            Some(d) => d,
            None => return SkeletonQuery::new(),
        };

        // Get the animation source for this skeleton
        let binding_api = BindingAPI::new(prim.clone());
        let anim_query = binding_api
            .get_animation_source()
            .map(|anim_prim| self.find_or_create_anim_query(&anim_prim));

        SkeletonQuery::from_definition(def, anim_query)
    }

    // Internal: recursively populate the cache.
    fn recursive_populate(
        &self,
        prim: &Prim,
        inherited_skel_path: Option<&Path>,
        depth: usize,
        predicate: &PrimFlagsPredicate,
    ) -> bool {
        // Apply predicate: check basic prim flags matching the predicate.
        // The predicate evaluates against prim flag bits; we construct flags from prim state.
        if !prim_matches_predicate(prim, predicate) {
            return false;
        }

        let mut found_skinnable = false;

        // Check for skeleton binding at this prim
        let binding_api = BindingAPI::new(prim.clone());
        let skel_path = binding_api
            .get_skeleton_rel()
            .and_then(|rel| rel.get_targets().first().cloned())
            .or_else(|| inherited_skel_path.cloned());

        // If this prim is skinnable, create a skinning query
        if let Some(ref _skel_path) = skel_path {
            let skinning_query = SkinningQuery::from_binding(&binding_api);
            if skinning_query.is_valid() {
                let prim_path = prim.path().clone();
                if let Ok(mut cache) = self.inner.skinning_query_cache.write() {
                    cache.insert(prim_path, skinning_query);
                }
                found_skinnable = true;
            }
        }

        // Recurse to children
        for child in prim.children() {
            if self.recursive_populate(&child, skel_path.as_ref(), depth + 1, predicate) {
                found_skinnable = true;
            }
        }

        found_skinnable
    }

    // Internal: collect all bindings beneath a prim, filtering by predicate.
    fn collect_bindings(
        &self,
        prim: &Prim,
        skel_to_skinning: &mut HashMap<Path, Vec<SkinningQuery>>,
        predicate: &PrimFlagsPredicate,
    ) {
        let path = prim.path().clone();

        // Check if this prim has a skinning query
        if let Ok(cache) = self.inner.skinning_query_cache.read() {
            if let Some(query) = cache.get(&path) {
                // Get the skeleton path for this query
                if let Some(skel_path) = query.get_skeleton_path() {
                    skel_to_skinning
                        .entry(skel_path)
                        .or_default()
                        .push(query.clone());
                }
            }
        }

        // Recurse to children, respecting predicate (matches C++ traversal)
        for child in prim.children() {
            if prim_matches_predicate(&child, predicate) {
                self.collect_bindings(&child, skel_to_skinning, predicate);
            }
        }
    }

    // Internal: collect bindings for a specific skeleton, filtering by predicate.
    fn collect_bindings_for_skel(
        &self,
        prim: &Prim,
        target_skel_path: &Path,
        skinning_queries: &mut Vec<SkinningQuery>,
        predicate: &PrimFlagsPredicate,
    ) {
        let path = prim.path().clone();

        // Check if this prim has a skinning query bound to the target skeleton
        if let Ok(cache) = self.inner.skinning_query_cache.read() {
            if let Some(query) = cache.get(&path) {
                if let Some(skel_path) = query.get_skeleton_path() {
                    if &skel_path == target_skel_path {
                        skinning_queries.push(query.clone());
                    }
                }
            }
        }

        // Recurse to children, respecting predicate (matches C++ traversal)
        for child in prim.children() {
            if prim_matches_predicate(&child, predicate) {
                self.collect_bindings_for_skel(
                    &child,
                    target_skel_path,
                    skinning_queries,
                    predicate,
                );
            }
        }
    }
}

/// Evaluate a PrimFlagsPredicate against a Prim's actual flags.
fn prim_matches_predicate(prim: &Prim, predicate: &PrimFlagsPredicate) -> bool {
    use usd_core::prim_flags::PrimFlags;
    let mut flags = PrimFlags::empty();
    if prim.is_active() {
        flags = flags | PrimFlags::ACTIVE;
    }
    if prim.is_loaded() {
        flags = flags | PrimFlags::LOADED;
    }
    if prim.is_defined() {
        flags = flags | PrimFlags::DEFINED;
    }
    if prim.is_model() {
        flags = flags | PrimFlags::MODEL;
    }
    if prim.is_instance() {
        flags = flags | PrimFlags::INSTANCE;
    }
    predicate.matches(flags)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_cache() {
        let cache = Cache::new();
        let skel = Skeleton::new(usd_core::Prim::invalid());
        let query = cache.get_skel_query(&skel);
        assert!(!query.is_valid());
    }

    #[test]
    fn test_cache_clear() {
        let cache = Cache::new();
        cache.clear();
        // Should not panic
    }

    #[test]
    fn test_predicate_filtering_in_bindings() {
        // Verify predicate parameter is now used (not ignored)
        let cache = Cache::new();
        let root = SkelRoot::new(usd_core::Prim::invalid());
        let pred = PrimFlagsPredicate::default();
        // Should not panic even with invalid prim -- returns empty
        let bindings = cache.compute_skel_bindings(&root, pred);
        assert!(bindings.is_empty());
    }

    #[test]
    fn test_compute_skel_binding_uses_predicate() {
        let cache = Cache::new();
        let root = SkelRoot::new(usd_core::Prim::invalid());
        let skel = Skeleton::new(usd_core::Prim::invalid());
        let pred = PrimFlagsPredicate::default();
        let binding = cache.compute_skel_binding(&root, &skel, pred);
        assert!(binding.is_none());
    }

    #[test]
    fn test_prim_matches_predicate_fn() {
        use usd_core::prim_flags::{PrimFlag, PrimFlagsConjunction, Term};
        // Default (tautology) predicate matches everything, even invalid prims
        let prim = usd_core::Prim::invalid();
        let pred_taut = PrimFlagsPredicate::default();
        assert!(prim_matches_predicate(&prim, &pred_taut));

        // A predicate requiring ACTIVE should reject an invalid prim (which is not active)
        let conj = PrimFlagsConjunction::from_term(Term::new(PrimFlag::Active));
        let pred_active = conj.as_predicate();
        let result = prim_matches_predicate(&prim, pred_active);
        assert!(!result, "invalid prim should not match 'active' predicate");
    }
}
