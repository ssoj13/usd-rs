//! UsdStageCache - A strongly concurrency safe collection of UsdStage references.
//!
//! Port of pxr/usd/usd/stageCache.h/cpp
//!
//! A strongly concurrency safe collection of UsdStageRefPtr s, enabling
//! sharing across multiple clients and threads.

use crate::stage::Stage;
use std::collections::HashMap;
use std::sync::{
    Arc, RwLock,
    atomic::{AtomicI64, Ordering},
};
use usd_ar::ResolverContext;
use usd_sdf::{Layer, LayerHandle};

/// Whether a stage's stored resolver context matches a `Find*` query.
///
/// Stages created via `CreateInMemory` store `None`; those match only an **empty** query context
/// (C++ "null" / empty `ArResolverContext`), matching OpenUSD cache semantics.
fn resolver_query_matches_stage(
    stage: &Arc<Stage>,
    query: &ResolverContext,
) -> bool {
    match stage.get_path_resolver_context() {
        Some(ctx) => ctx == *query,
        None => query.is_empty(),
    }
}

/// Whether `stage_session` matches a `Find*` query with optional `session_layer` handle.
fn session_matches_stage_query(
    stage_session: Option<&Arc<Layer>>,
    query_session: Option<&LayerHandle>,
) -> bool {
    match (query_session, stage_session) {
        (None, None) => true,
        (Some(qh), Some(st)) => qh
            .upgrade()
            .map(|l| l.identifier() == st.identifier())
            .unwrap_or(false),
        _ => false,
    }
}

/// Global counter for generating unique IDs
static ID_COUNTER: AtomicI64 = AtomicI64::new(9223000);

fn get_next_id() -> i64 {
    ID_COUNTER.fetch_add(1, Ordering::SeqCst) + 1
}

/// A lightweight identifier that may be used to identify a particular cached stage.
///
/// Matches C++ `UsdStageCache::Id`.
///
/// An identifier may be converted to and from i64 and string, to facilitate
/// use within restricted contexts.
///
/// Id objects are only valid with the stage from which they were obtained.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StageCacheId {
    value: i64,
}

impl StageCacheId {
    /// Default construct an invalid id.
    ///
    /// Matches C++ `Id()`.
    pub fn invalid() -> Self {
        Self { value: -1 }
    }

    /// Create an Id from an integral value.
    ///
    /// Matches C++ `FromLongInt(long int val)`.
    pub fn from_long_int(val: i64) -> Self {
        Self { value: val }
    }

    /// Create an Id from a string value.
    ///
    /// Matches C++ `FromString(const std::string &s)`.
    pub fn from_string(s: &str) -> Self {
        match s.parse::<i64>() {
            Ok(val) => Self::from_long_int(val),
            Err(_) => Self::invalid(),
        }
    }

    /// Convert this Id to an integral representation.
    ///
    /// Matches C++ `ToLongInt() const`.
    pub fn to_long_int(&self) -> i64 {
        self.value
    }

    /// Convert this Id to a string representation.
    ///
    /// Matches C++ `ToString() const`.
    pub fn to_string(&self) -> String {
        self.value.to_string()
    }

    /// Return true if this Id is valid.
    ///
    /// Matches C++ `IsValid() const`.
    pub fn is_valid(&self) -> bool {
        self.value != -1
    }
}

impl Default for StageCacheId {
    fn default() -> Self {
        Self::invalid()
    }
}

impl std::fmt::Display for StageCacheId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

/// Internal storage for cached stages
struct StageCacheImpl {
    /// Stages indexed by id
    by_id: HashMap<StageCacheId, Arc<Stage>>,
    /// IDs indexed by stage pointer (using raw pointer for comparison)
    by_stage: HashMap<usize, StageCacheId>,
    /// Stages indexed by root layer identifier
    by_root_layer: HashMap<String, Vec<usize>>,
    /// Debug name for this cache
    debug_name: String,
}

impl StageCacheImpl {
    fn new() -> Self {
        Self {
            by_id: HashMap::new(),
            by_stage: HashMap::new(),
            by_root_layer: HashMap::new(),
            debug_name: String::new(),
        }
    }
}

/// A strongly concurrency safe collection of UsdStage references.
///
/// Matches C++ `UsdStageCache`.
///
/// A strongly concurrency safe collection of UsdStageRefPtr s, enabling
/// sharing across multiple clients and threads. See UsdStageCacheContext for
/// typical use cases finding UsdStage s in a cache and publishing UsdStage s to
/// a cache.
///
/// UsdStageCache is strongly thread safe: all operations other than
/// construction and destruction may be performed concurrently.
///
/// Caches provide a mechanism that associates a lightweight key,
/// UsdStageCache::Id, with a cached stage. A UsdStageCache::Id can be
/// converted to and from long int and string.
pub struct StageCache {
    inner: RwLock<StageCacheImpl>,
}

impl StageCache {
    /// Default construct an empty cache.
    ///
    /// Matches C++ `UsdStageCache()`.
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(StageCacheImpl::new()),
        }
    }

    /// Return a vector containing the stages present in this cache.
    ///
    /// Matches C++ `GetAllStages() const`.
    pub fn get_all_stages(&self) -> Vec<Arc<Stage>> {
        let inner = self.inner.read().expect("rwlock poisoned");
        inner.by_id.values().cloned().collect()
    }

    /// Return the number of stages present in this cache.
    ///
    /// Matches C++ `Size() const`.
    pub fn size(&self) -> usize {
        let inner = self.inner.read().expect("rwlock poisoned");
        inner.by_id.len()
    }

    /// Return true if this cache holds no stages, false otherwise.
    ///
    /// Matches C++ `IsEmpty() const`.
    pub fn is_empty(&self) -> bool {
        self.size() == 0
    }

    /// Find the stage in this cache corresponding to id.
    ///
    /// Matches C++ `Find(Id id) const`.
    ///
    /// If id is not valid or if this cache does not have a stage
    /// corresponding to id, return None.
    pub fn find(&self, id: StageCacheId) -> Option<Arc<Stage>> {
        if !id.is_valid() {
            return None;
        }
        let inner = self.inner.read().expect("rwlock poisoned");
        inner.by_id.get(&id).cloned()
    }

    /// Find a stage in this cache with rootLayer.
    ///
    /// Matches C++ `FindOneMatching(const SdfLayerHandle &rootLayer) const`.
    ///
    /// If there is no matching stage, return None. If there is more than one
    /// matching stage, return an arbitrary matching one.
    pub fn find_one_matching(&self, root_layer: &LayerHandle) -> Option<Arc<Stage>> {
        let inner = self.inner.read().expect("rwlock poisoned");
        let identifier = root_layer
            .upgrade()
            .map(|l| l.identifier().to_string())
            .unwrap_or_default();

        if let Some(stage_ptrs) = inner.by_root_layer.get(&identifier) {
            if let Some(&ptr) = stage_ptrs.first() {
                // Find the stage with this pointer
                if let Some(&id) = inner.by_stage.get(&ptr) {
                    return inner.by_id.get(&id).cloned();
                }
            }
        }
        None
    }

    /// Find a stage in this cache with rootLayer and sessionLayer.
    ///
    /// Matches C++ `FindOneMatching(const SdfLayerHandle &rootLayer, const SdfLayerHandle &sessionLayer) const`.
    pub fn find_one_matching_with_session(
        &self,
        root_layer: &LayerHandle,
        session_layer: Option<&LayerHandle>,
    ) -> Option<Arc<Stage>> {
        let inner = self.inner.read().expect("rwlock poisoned");
        let identifier = root_layer
            .upgrade()
            .map(|l| l.identifier().to_string())
            .unwrap_or_default();

        if let Some(stage_ptrs) = inner.by_root_layer.get(&identifier) {
            for &ptr in stage_ptrs {
                if let Some(&id) = inner.by_stage.get(&ptr) {
                    if let Some(stage) = inner.by_id.get(&id) {
                        let stage_session = stage.get_session_layer();
                        let matches = match (session_layer, &stage_session) {
                            (None, None) => true,
                            (Some(sl), Some(ssl)) => {
                                if let Some(l1) = sl.upgrade() {
                                    l1.identifier() == ssl.identifier()
                                } else {
                                    false
                                }
                            }
                            _ => false,
                        };
                        if matches {
                            return Some(stage.clone());
                        }
                    }
                }
            }
        }
        None
    }

    /// Find a stage in this cache with rootLayer and pathResolverContext.
    ///
    /// Matches C++ `FindOneMatching(const SdfLayerHandle &rootLayer, const ArResolverContext &pathResolverContext) const`.
    pub fn find_one_matching_with_resolver(
        &self,
        root_layer: &LayerHandle,
        path_resolver_context: &ResolverContext,
    ) -> Option<Arc<Stage>> {
        let inner = self.inner.read().expect("rwlock poisoned");
        let identifier = root_layer
            .upgrade()
            .map(|l| l.identifier().to_string())
            .unwrap_or_default();

        if let Some(stage_ptrs) = inner.by_root_layer.get(&identifier) {
            for &ptr in stage_ptrs {
                if let Some(&id) = inner.by_stage.get(&ptr) {
                    if let Some(stage) = inner.by_id.get(&id) {
                        let matches = resolver_query_matches_stage(stage, path_resolver_context);
                        if matches {
                            return Some(stage.clone());
                        }
                    }
                }
            }
        }
        None
    }

    /// Find all stages in this cache with rootLayer.
    ///
    /// Matches C++ `FindAllMatching(const SdfLayerHandle &rootLayer) const`.
    pub fn find_all_matching(&self, root_layer: &LayerHandle) -> Vec<Arc<Stage>> {
        let inner = self.inner.read().expect("rwlock poisoned");
        let identifier = root_layer
            .upgrade()
            .map(|l| l.identifier().to_string())
            .unwrap_or_default();
        let mut result = Vec::new();

        if let Some(stage_ptrs) = inner.by_root_layer.get(&identifier) {
            for &ptr in stage_ptrs {
                if let Some(&id) = inner.by_stage.get(&ptr) {
                    if let Some(stage) = inner.by_id.get(&id) {
                        result.push(stage.clone());
                    }
                }
            }
        }
        result
    }

    /// Find all stages with `rootLayer` whose session layer matches `session_layer` when provided.
    ///
    /// When `session_layer` is `None`, only stages with **no** session layer match.
    pub fn find_all_matching_with_session(
        &self,
        root_layer: &LayerHandle,
        session_layer: Option<&LayerHandle>,
    ) -> Vec<Arc<Stage>> {
        self.find_all_matching(root_layer)
            .into_iter()
            .filter(|stage| {
                session_matches_stage_query(stage.get_session_layer().as_ref(), session_layer)
            })
            .collect()
    }

    /// Find all stages with `rootLayer` and `pathResolverContext`.
    pub fn find_all_matching_with_resolver(
        &self,
        root_layer: &LayerHandle,
        path_resolver_context: &ResolverContext,
    ) -> Vec<Arc<Stage>> {
        self.find_all_matching(root_layer)
            .into_iter()
            .filter(|stage| resolver_query_matches_stage(stage, path_resolver_context))
            .collect()
    }

    /// Find all stages matching root, optional session layer, and resolver context.
    pub fn find_all_matching_with_session_and_resolver(
        &self,
        root_layer: &LayerHandle,
        session_layer: Option<&LayerHandle>,
        path_resolver_context: &ResolverContext,
    ) -> Vec<Arc<Stage>> {
        self.find_all_matching_with_session(root_layer, session_layer)
            .into_iter()
            .filter(|stage| resolver_query_matches_stage(stage, path_resolver_context))
            .collect()
    }

    /// `FindOneMatching` for (root, session, resolver) — arbitrary match if multiple.
    pub fn find_one_matching_with_session_and_resolver(
        &self,
        root_layer: &LayerHandle,
        session_layer: Option<&LayerHandle>,
        path_resolver_context: &ResolverContext,
    ) -> Option<Arc<Stage>> {
        self.find_all_matching_with_session_and_resolver(
            root_layer,
            session_layer,
            path_resolver_context,
        )
        .into_iter()
        .next()
    }

    /// Return the Id associated with stage in this cache.
    ///
    /// Matches C++ `GetId(const UsdStageRefPtr &stage) const`.
    ///
    /// If stage is not present in this cache, return an invalid Id.
    pub fn get_id(&self, stage: &Arc<Stage>) -> StageCacheId {
        let inner = self.inner.read().expect("rwlock poisoned");
        let ptr = Arc::as_ptr(stage) as usize;
        inner
            .by_stage
            .get(&ptr)
            .copied()
            .unwrap_or(StageCacheId::invalid())
    }

    /// Return true if stage is present in this cache, false otherwise.
    ///
    /// Matches C++ `Contains(const UsdStageRefPtr &stage) const`.
    pub fn contains(&self, stage: &Arc<Stage>) -> bool {
        self.get_id(stage).is_valid()
    }

    /// Return true if id is present in this cache, false otherwise.
    ///
    /// Matches C++ `Contains(Id id) const`.
    pub fn contains_id(&self, id: StageCacheId) -> bool {
        self.find(id).is_some()
    }

    /// Insert stage into this cache and return its associated Id.
    ///
    /// Matches C++ `Insert(const UsdStageRefPtr &stage)`.
    ///
    /// If the given stage is already present in this cache, simply return its
    /// associated Id.
    pub fn insert(&self, stage: Arc<Stage>) -> StageCacheId {
        let mut inner = self.inner.write().expect("rwlock poisoned");
        let ptr = Arc::as_ptr(&stage) as usize;

        // Check if already present
        if let Some(&id) = inner.by_stage.get(&ptr) {
            return id;
        }

        // Generate new ID and insert
        let id = StageCacheId::from_long_int(get_next_id());
        let root_layer_id = stage.get_root_layer().identifier().to_string();

        inner.by_id.insert(id, stage);
        inner.by_stage.insert(ptr, id);
        inner
            .by_root_layer
            .entry(root_layer_id)
            .or_default()
            .push(ptr);

        id
    }

    /// Erase the stage identified by id from this cache and return true.
    ///
    /// Matches C++ `Erase(Id id)`.
    ///
    /// If id is invalid or there is no associated stage in this cache, do
    /// nothing and return false.
    pub fn erase(&self, id: StageCacheId) -> bool {
        if !id.is_valid() {
            return false;
        }

        let mut inner = self.inner.write().expect("rwlock poisoned");

        if let Some(stage) = inner.by_id.remove(&id) {
            let ptr = Arc::as_ptr(&stage) as usize;
            inner.by_stage.remove(&ptr);

            // Remove from root layer index
            let root_layer = stage.get_root_layer();
            let layer_id = root_layer.identifier().to_string();
            if let Some(ptrs) = inner.by_root_layer.get_mut(&layer_id) {
                ptrs.retain(|&p| p != ptr);
            }
            true
        } else {
            false
        }
    }

    /// Erase stage from this cache and return true.
    ///
    /// Matches C++ `Erase(const UsdStageRefPtr &stage)`.
    pub fn erase_stage(&self, stage: &Arc<Stage>) -> bool {
        let id = self.get_id(stage);
        self.erase(id)
    }

    /// Erase all stages present in the cache with rootLayer and return the number erased.
    ///
    /// Matches C++ `EraseAll(const SdfLayerHandle &rootLayer)`.
    pub fn erase_all(&self, root_layer: &LayerHandle) -> usize {
        let mut inner = self.inner.write().expect("rwlock poisoned");
        let identifier = root_layer
            .upgrade()
            .map(|l| l.identifier().to_string())
            .unwrap_or_default();
        let mut count = 0;

        if let Some(stage_ptrs) = inner.by_root_layer.remove(&identifier) {
            for ptr in stage_ptrs {
                if let Some(id) = inner.by_stage.remove(&ptr) {
                    inner.by_id.remove(&id);
                    count += 1;
                }
            }
        }

        count
    }

    /// Erase all stages matching `rootLayer` and session layer filter (see `find_all_matching_with_session`).
    pub fn erase_all_with_session(
        &self,
        root_layer: &LayerHandle,
        session_layer: Option<&LayerHandle>,
    ) -> usize {
        let stages = self.find_all_matching_with_session(root_layer, session_layer);
        let mut count = 0;
        for s in stages {
            if self.erase_stage(&s) {
                count += 1;
            }
        }
        count
    }

    /// Erase all stages matching `rootLayer` and resolver context.
    pub fn erase_all_with_resolver(
        &self,
        root_layer: &LayerHandle,
        path_resolver_context: &ResolverContext,
    ) -> usize {
        let stages = self.find_all_matching_with_resolver(root_layer, path_resolver_context);
        let mut count = 0;
        for s in stages {
            if self.erase_stage(&s) {
                count += 1;
            }
        }
        count
    }

    /// Erase all stages matching root, optional session, and resolver context.
    pub fn erase_all_with_session_and_resolver(
        &self,
        root_layer: &LayerHandle,
        session_layer: Option<&LayerHandle>,
        path_resolver_context: &ResolverContext,
    ) -> usize {
        let stages = self.find_all_matching_with_session_and_resolver(
            root_layer,
            session_layer,
            path_resolver_context,
        );
        let mut count = 0;
        for s in stages {
            if self.erase_stage(&s) {
                count += 1;
            }
        }
        count
    }

    /// Remove all entries from this cache.
    ///
    /// Matches C++ `Clear()`.
    pub fn clear(&self) {
        let mut inner = self.inner.write().expect("rwlock poisoned");
        inner.by_id.clear();
        inner.by_stage.clear();
        inner.by_root_layer.clear();
    }

    /// Assign a debug name to this cache.
    ///
    /// Matches C++ `SetDebugName(const std::string &debugName)`.
    pub fn set_debug_name(&self, debug_name: &str) {
        let mut inner = self.inner.write().expect("rwlock poisoned");
        inner.debug_name = debug_name.to_string();
    }

    /// Retrieve this cache's debug name.
    ///
    /// Matches C++ `GetDebugName() const`.
    pub fn get_debug_name(&self) -> String {
        let inner = self.inner.read().expect("rwlock poisoned");
        inner.debug_name.clone()
    }
}

impl Default for StageCache {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for StageCache {
    fn clone(&self) -> Self {
        let inner = self.inner.read().expect("rwlock poisoned");
        let new_inner = StageCacheImpl {
            by_id: inner.by_id.clone(),
            by_stage: inner.by_stage.clone(),
            by_root_layer: inner.by_root_layer.clone(),
            debug_name: inner.debug_name.clone(),
        };
        Self {
            inner: RwLock::new(new_inner),
        }
    }
}

impl std::fmt::Debug for StageCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let inner = self.inner.read().expect("rwlock poisoned");
        let name = if inner.debug_name.is_empty() {
            format!("{:p}", self)
        } else {
            format!("\"{}\"", inner.debug_name)
        };
        write!(f, "stage cache {} (size={})", name, inner.by_id.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stage_cache_id() {
        let id = StageCacheId::invalid();
        assert!(!id.is_valid());

        let id = StageCacheId::from_long_int(42);
        assert!(id.is_valid());
        assert_eq!(id.to_long_int(), 42);
        assert_eq!(id.to_string(), "42");

        let id2 = StageCacheId::from_string("42");
        assert_eq!(id, id2);
    }

    #[test]
    fn test_stage_cache_empty() {
        let cache = StageCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.size(), 0);
    }
}
