//! Filtering scene indices - chain scene indices together.

use super::base::HdSceneIndexBaseImpl;
use super::base::SceneRwLock;
use super::base::{HdSceneIndexBase, HdSceneIndexHandle, SdfPathVector};
use super::observer::{
    AddedPrimEntry, DirtiedPrimEntry, HdSceneIndexObserver, HdSceneIndexObserverHandle,
    RemovedPrimEntry, RenamedPrimEntry,
};
use super::prim::HdSceneIndexPrim;
use std::sync::Arc;
use usd_sdf::Path as SdfPath;

/// Base trait for filtering scene indices.
///
/// Filtering scene indices take one or more input scene indices and
/// transform their data. They observe their inputs and forward
/// (potentially modified) notifications.
pub trait HdFilteringSceneIndexBase: HdSceneIndexBase {
    /// Get all input scene indices.
    fn get_input_scenes(&self) -> Vec<HdSceneIndexHandle>;
}

/// Trait for scene indices that encapsulate other scene index graphs.
///
/// Port of C++ `HdEncapsulatingSceneIndexBase`. Used by the Hydra Scene
/// Debugger to understand composite scene index structure.
///
/// Unlike HdFilteringSceneIndexBase which returns operational inputs,
/// this returns the "conceptual" sub-graphs that make up the encapsulated
/// processing.
pub trait HdEncapsulatingSceneIndexBaseTrait {
    /// Get the encapsulated scene sub-graphs.
    fn get_encapsulated_scenes(&self) -> Vec<HdSceneIndexHandle>;
}

/// Cast-like check: try to get encapsulated scenes from a scene index handle.
///
/// Port of C++ `HdEncapsulatingSceneIndexBase::Cast`. Since Rust uses traits
/// instead of RTTI, we check via Any downcast on the inner type.
pub fn cast_to_encapsulating(_scene: &HdSceneIndexHandle) -> Option<Vec<HdSceneIndexHandle>> {
    // In C++, this uses dynamic_cast. In Rust, concrete types implement
    // HdEncapsulatingSceneIndexBaseTrait and callers use it directly.
    // This function exists for API parity; callers should prefer trait bounds.
    None
}

// ---------------------------------------------------------------------------
// _NoOpSceneIndex - fallback for null input (G3)
// ---------------------------------------------------------------------------

/// Fallback scene index used when null input is passed to a filtering scene
/// index constructor. Port of C++ `_NoOpSceneIndex`.
struct NoOpSceneIndex;

impl HdSceneIndexBase for NoOpSceneIndex {
    fn get_prim(&self, _prim_path: &SdfPath) -> HdSceneIndexPrim {
        HdSceneIndexPrim::empty()
    }

    fn get_child_prim_paths(&self, _prim_path: &SdfPath) -> SdfPathVector {
        Vec::new()
    }

    fn add_observer(&self, _observer: HdSceneIndexObserverHandle) {}
    fn remove_observer(&self, _observer: &HdSceneIndexObserverHandle) {}

    fn get_display_name(&self) -> String {
        "_NoOpSceneIndex".to_string()
    }
}

/// Create the singleton NoOp scene index handle.
fn noop_scene_index() -> HdSceneIndexHandle {
    Arc::new(SceneRwLock::new(NoOpSceneIndex))
}

/// Base for scene indices with a single input.
///
/// This is a convenience base for the common case of a scene index
/// that filters/transforms a single input scene index.
///
/// If a null (None) input scene is passed, a NoOp fallback is created
/// and a coding error is logged, matching C++ behavior.
///
/// Subclasses should implement:
/// - get_prim() and get_child_prim_paths() to query/transform input
/// - _prims_added(), _prims_removed(), _prims_dirtied() to handle input changes
pub struct HdSingleInputFilteringSceneIndexBase {
    /// Base implementation for observer management
    base: HdSceneIndexBaseImpl,
    /// The input scene index (always valid after construction)
    input_scene: Option<HdSceneIndexHandle>,
}

impl HdSingleInputFilteringSceneIndexBase {
    /// Create a new single-input filtering scene index.
    ///
    /// Port of C++ constructor: if `input_scene` is None, logs a coding error
    /// and substitutes a NoOp scene index fallback.
    pub fn new(input_scene: Option<HdSceneIndexHandle>) -> Self {
        let actual_input = match input_scene {
            Some(scene) => Some(scene),
            None => {
                log::error!(
                    "Invalid input sceneIndex passed to filtering scene index; using NoOp fallback."
                );
                Some(noop_scene_index())
            }
        };
        Self {
            base: HdSceneIndexBaseImpl::new(),
            input_scene: actual_input,
        }
    }

    /// Wire this filter as observer of its input scene index.
    /// C++ parity: in C++ the constructor calls `_inputSceneIndex->AddObserver(this)`.
    /// In Rust we use two-phase init because `self` isn't yet in Arc during `new()`.
    pub fn wire_observer(
        &self,
        owner_weak: std::sync::Weak<SceneRwLock<dyn FilteringObserverTarget>>,
    ) {
        if let Some(input) = &self.input_scene {
            let observer = FilteringSceneIndexObserver::new(owner_weak);
            let observer_handle: HdSceneIndexObserverHandle = Arc::new(observer);
            let input_guard = input.write();
            input_guard.add_observer(observer_handle);
        }
    }

    /// Get the input scene index.
    pub fn get_input_scene(&self) -> Option<&HdSceneIndexHandle> {
        self.input_scene.as_ref()
    }

    /// Set the input scene index.
    ///
    /// Note: In a full implementation with automatic observer registration,
    /// this would unregister from old input and register with new input.
    /// Currently, observer registration is handled explicitly by the caller.
    pub fn set_input_scene(&mut self, input_scene: Option<HdSceneIndexHandle>) {
        self.input_scene = input_scene;
    }

    /// Access the base implementation.
    pub fn base(&self) -> &HdSceneIndexBaseImpl {
        &self.base
    }

    /// Access the base implementation mutably.
    pub fn base_mut(&mut self) -> &mut HdSceneIndexBaseImpl {
        &mut self.base
    }

    /// Forward prims added notification from this filtering scene index.
    ///
    /// Hydra observers may query `sender.get_prim(...)` while processing a
    /// notice, so the sender must be the filtering scene index that exposes the
    /// transformed view, not the upstream input sender.
    pub fn forward_prims_added(
        &self,
        scene_index: &dyn HdSceneIndexBase,
        entries: &[AddedPrimEntry],
    ) {
        self.base.send_prims_added(scene_index, entries);
    }

    /// Forward prims removed notification.
    pub fn forward_prims_removed(
        &self,
        scene_index: &dyn HdSceneIndexBase,
        entries: &[RemovedPrimEntry],
    ) {
        self.base.send_prims_removed(scene_index, entries);
    }

    /// Forward prims dirtied notification.
    pub fn forward_prims_dirtied(
        &self,
        scene_index: &dyn HdSceneIndexBase,
        entries: &[DirtiedPrimEntry],
    ) {
        if entries.len() >= 1000 {
            let emitter_name = scene_index.get_display_name();
            if emitter_name == "PointsResolvingSceneIndex"
                || emitter_name == "SkeletonResolvingSceneIndex"
                || emitter_name == "MaterialBindingsResolvingSceneIndex"
                || emitter_name == "InstanceProxyPathTranslationSceneIndex"
            {
                let first = entries
                    .first()
                    .map(|entry| entry.prim_path.to_string())
                    .unwrap_or_default();
                eprintln!(
                    "[filtering] forward_prims_dirtied emitter={} out={} first={}",
                    emitter_name,
                    entries.len(),
                    first
                );
            }
        }
        self.base.send_prims_dirtied(scene_index, entries);
    }

    /// Forward prims renamed notification.
    pub fn forward_prims_renamed(
        &self,
        scene_index: &dyn HdSceneIndexBase,
        entries: &[RenamedPrimEntry],
    ) {
        self.base.send_prims_renamed(scene_index, entries);
    }
}

/// Wire a filter scene index as observer of an input scene index.
/// C++ parity: HdSingleInputFilteringSceneIndex constructor calls AddObserver.
/// In Rust, call this after wrapping the filter in Arc<SceneRwLock<>>.
pub fn wire_filter_to_input<T: FilteringObserverTarget + 'static>(
    filter: &Arc<SceneRwLock<T>>,
    input: &HdSceneIndexHandle,
) {
    let dyn_arc: Arc<SceneRwLock<dyn FilteringObserverTarget>> = filter.clone();
    let weak = Arc::downgrade(&dyn_arc);
    let observer = FilteringSceneIndexObserver::new(weak);
    let observer_handle: HdSceneIndexObserverHandle = Arc::new(observer);
    log::info!("[wire] about to read lock input for add_observer");
    input.read().add_observer(observer_handle);
    log::info!("[wire] add_observer done");
}

/// Observer for filtering scene indices.
///
/// This observer forwards notifications from input scene indices
/// to the filtering scene index, allowing it to process changes.
pub struct FilteringSceneIndexObserver {
    /// Weak reference to the owning scene index
    owner: std::sync::Weak<SceneRwLock<dyn FilteringObserverTarget>>,
}

impl FilteringSceneIndexObserver {
    /// Create a new filtering observer.
    pub fn new(owner: std::sync::Weak<SceneRwLock<dyn FilteringObserverTarget>>) -> Self {
        Self { owner }
    }
}

impl HdSceneIndexObserver for FilteringSceneIndexObserver {
    fn prims_added(&self, sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        if let Some(owner_arc) = self.owner.upgrade() {
            // No read lock — matches C++ shared_ptr semantics. Callbacks use &self
            // with interior mutability. Locking here causes recursive deadlocks
            // when notification cascades re-enter the scene index chain.
            let owner = super::base::rwlock_data_ref(owner_arc.as_ref());
            let owner_name = owner.get_display_name();
            if entries.len() >= 1000
                && (owner_name == "PointsResolvingSceneIndex"
                    || owner_name == "SkeletonResolvingSceneIndex"
                    || owner_name == "MaterialBindingsResolvingSceneIndex"
                    || owner_name == "InstanceProxyPathTranslationSceneIndex")
            {
                let first = entries
                    .first()
                    .map(|entry| entry.prim_path.to_string())
                    .unwrap_or_default();
                eprintln!(
                    "[filtering_observer] callback=added owner={} sender={} in={} first={}",
                    owner_name,
                    sender.get_display_name(),
                    entries.len(),
                    first
                );
            }
            owner.on_prims_added(sender, entries);
        }
    }

    fn prims_removed(&self, sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        if let Some(owner_arc) = self.owner.upgrade() {
            let owner = super::base::rwlock_data_ref(owner_arc.as_ref());
            let owner_name = owner.get_display_name();
            if entries.len() >= 1000
                && (owner_name == "PointsResolvingSceneIndex"
                    || owner_name == "SkeletonResolvingSceneIndex"
                    || owner_name == "MaterialBindingsResolvingSceneIndex"
                    || owner_name == "InstanceProxyPathTranslationSceneIndex")
            {
                let first = entries
                    .first()
                    .map(|entry| entry.prim_path.to_string())
                    .unwrap_or_default();
                eprintln!(
                    "[filtering_observer] callback=removed owner={} sender={} in={} first={}",
                    owner_name,
                    sender.get_display_name(),
                    entries.len(),
                    first
                );
            }
            owner.on_prims_removed(sender, entries);
        }
    }

    fn prims_dirtied(&self, sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        if let Some(owner_arc) = self.owner.upgrade() {
            let owner = super::base::rwlock_data_ref(owner_arc.as_ref());
            let owner_name = owner.get_display_name();
            if entries.len() >= 1000
                && (owner_name == "PointsResolvingSceneIndex"
                    || owner_name == "SkeletonResolvingSceneIndex"
                    || owner_name == "MaterialBindingsResolvingSceneIndex"
                    || owner_name == "InstanceProxyPathTranslationSceneIndex")
            {
                let first = entries
                    .first()
                    .map(|entry| entry.prim_path.to_string())
                    .unwrap_or_default();
                eprintln!(
                    "[filtering_observer] callback=dirtied owner={} sender={} in={} first={}",
                    owner_name,
                    sender.get_display_name(),
                    entries.len(),
                    first
                );
            }
            owner.on_prims_dirtied(sender, entries);
        }
    }

    fn prims_renamed(&self, sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        if let Some(owner_arc) = self.owner.upgrade() {
            let owner = super::base::rwlock_data_ref(owner_arc.as_ref());
            owner.on_prims_renamed(sender, entries);
        }
    }
}

/// Target trait for filtering observer callbacks.
///
/// Filtering scene indices should implement this to receive
/// notifications from their input scenes.
pub trait FilteringObserverTarget: HdSceneIndexBase + Send + Sync {
    /// Handle prims added notification from input scene.
    fn on_prims_added(&self, sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]);

    /// Handle prims removed notification from input scene.
    fn on_prims_removed(&self, sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]);

    /// Handle prims dirtied notification from input scene.
    fn on_prims_dirtied(&self, sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]);

    /// Handle prims renamed notification from input scene.
    fn on_prims_renamed(&self, sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_input_filtering_base() {
        // G3: None input creates NoOp fallback (not actually None)
        let base = HdSingleInputFilteringSceneIndexBase::new(None);
        assert!(base.get_input_scene().is_some()); // NoOp fallback
        assert!(!base.base().is_observed());
    }

    #[test]
    fn test_noop_fallback_returns_empty() {
        let base = HdSingleInputFilteringSceneIndexBase::new(None);
        let input = base.get_input_scene().unwrap();
        let input_lock = input.read();
        let prim = input_lock.get_prim(&SdfPath::absolute_root());
        assert!(!prim.is_defined());
        assert!(
            input_lock
                .get_child_prim_paths(&SdfPath::absolute_root())
                .is_empty()
        );
    }

    #[test]
    fn test_single_input_with_scene() {
        let base = HdSingleInputFilteringSceneIndexBase::new(None);
        assert!(base.get_input_scene().is_some()); // NoOp
    }
}
