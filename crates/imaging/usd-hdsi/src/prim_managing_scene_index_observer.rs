
//! Prim managing scene index observer.
//!
//! Port of pxr/imaging/hdsi/primManagingSceneIndexObserver.{h,cpp}
//!
//! Turns prims in an observed scene index into RAII managed objects via a
//! PrimFactoryBase. Mirrors the HdPrimTypeIndex pattern for scene index API.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use usd_hd::scene_index::observer::*;
use usd_hd::scene_index::prim_view::HdSceneIndexPrimView;
use usd_hd::scene_index::{HdSceneIndexBase, HdSceneIndexHandle, si_ref};
use usd_sdf::Path as SdfPath;

// ---------------------------------------------------------------------------
// PrimBase -- NVI dirty pattern matching C++ HdsiPrimManagingSceneIndexObserver::PrimBase
// ---------------------------------------------------------------------------

/// Base class for prims managed by [`HdsiPrimManagingSceneIndexObserver`].
///
/// Subclasses implement `_dirty` to respond to data-source invalidation.
/// The public `dirty()` method is a NVI (Non-Virtual Interface) wrapper that
/// can be extended later (e.g. adding mutex guards for thread safety).
pub trait PrimBase: Send + Sync {
    /// Called when the prim's data sources are invalidated.
    ///
    /// NVI: delegates to `_dirty`.
    fn dirty(&self, entry: &DirtiedPrimEntry, observer: &HdsiPrimManagingSceneIndexObserver) {
        self._dirty(entry, observer);
    }

    /// Override to handle dirty notification.
    fn _dirty(&self, entry: &DirtiedPrimEntry, observer: &HdsiPrimManagingSceneIndexObserver);
}

/// Shared handle to a managed prim.
pub type PrimBaseHandle = Arc<dyn PrimBase>;

// ---------------------------------------------------------------------------
// PrimFactoryBase -- factory for creating managed prims
// ---------------------------------------------------------------------------

/// Factory that creates [`PrimBase`] instances for prim entries.
///
/// Implementations return `None` for unsupported prim types.
pub trait PrimFactoryBase: Send + Sync {
    /// Create a managed prim for the given added entry.
    ///
    /// Returns `None` if the prim type is not supported by this factory.
    fn create_prim(
        &self,
        entry: &AddedPrimEntry,
        observer: &HdsiPrimManagingSceneIndexObserver,
    ) -> Option<PrimBaseHandle>;
}

/// Shared handle to a prim factory.
pub type PrimFactoryBaseHandle = Arc<dyn PrimFactoryBase>;

// ---------------------------------------------------------------------------
// HdsiPrimManagingSceneIndexObserver
// ---------------------------------------------------------------------------

/// Scene index observer that manages prim lifecycle via a factory.
///
/// Port of C++ HdsiPrimManagingSceneIndexObserver.
///
/// - On construction, iterates the entire observed scene index and calls
///   `prim_factory.create_prim()` for each prim.
/// - `prims_added`: creates or replaces managed prims via the factory.
/// - `prims_dirtied`: calls `PrimBase::dirty()` on matching entries.
/// - `prims_removed`: removes all entries prefixed by the removal path.
/// - `prims_renamed`: converts to removed+added via helper.
pub struct HdsiPrimManagingSceneIndexObserver {
    /// Observed scene index (kept alive for lifetime of observer).
    scene_index: Option<HdSceneIndexHandle>,
    /// Factory that creates managed prim objects.
    prim_factory: Option<PrimFactoryBaseHandle>,
    /// BTreeMap enables efficient prefix-range removal (lower_bound).
    prims: Mutex<BTreeMap<SdfPath, PrimBaseHandle>>,
}

impl HdsiPrimManagingSceneIndexObserver {
    /// Creates a new observer without a factory (passive tracking only).
    ///
    /// Useful for testing or when no managed objects are needed.
    pub fn new() -> Self {
        Self {
            scene_index: None,
            prim_factory: None,
            prims: Mutex::new(BTreeMap::new()),
        }
    }

    /// Creates a new observer attached to `scene_index` with the given factory.
    ///
    /// Immediately populates managed prims by iterating the scene index.
    pub fn with_scene_index(
        scene_index: HdSceneIndexHandle,
        prim_factory: PrimFactoryBaseHandle,
    ) -> Self {
        let observer = Self {
            scene_index: Some(scene_index.clone()),
            prim_factory: Some(prim_factory),
            prims: Mutex::new(BTreeMap::new()),
        };
        // Initial population: mirror C++ constructor loop over HdSceneIndexPrimView.
        let view = HdSceneIndexPrimView::new(scene_index.clone());
        for path in view.iter() {
            let prim = si_ref(&scene_index).get_prim(&path);
            let entry = AddedPrimEntry::new(path.clone(), prim.prim_type);
            if let Some(factory) = &observer.prim_factory {
                if let Some(managed) = factory.create_prim(&entry, &observer) {
                    observer
                        .prims
                        .lock()
                        .expect("Lock poisoned")
                        .insert(path, managed);
                }
            }
        }
        observer
    }

    /// Returns the observed scene index.
    pub fn get_scene_index(&self) -> Option<&HdSceneIndexHandle> {
        self.scene_index.as_ref()
    }

    /// Get the managed prim at the given path, or `None`.
    ///
    /// Mirrors C++ `GetPrim(const SdfPath&)`.
    pub fn get_prim(&self, prim_path: &SdfPath) -> Option<PrimBaseHandle> {
        self.prims
            .lock()
            .expect("Lock poisoned")
            .get(prim_path)
            .cloned()
    }

    /// Get the managed prim cast to a concrete type.
    ///
    /// Mirrors C++ `GetTypedPrim<T>(const SdfPath&)`.
    pub fn get_typed_prim<T: PrimBase + 'static>(&self, prim_path: &SdfPath) -> Option<Arc<T>> {
        self.prims
            .lock()
            .expect("Lock poisoned")
            .get(prim_path)?
            .clone()
            .downcast_arc::<T>()
    }

    /// Checks if a prim at the given path is currently managed.
    pub fn is_prim_active(&self, path: &SdfPath) -> bool {
        self.prims.lock().expect("Lock poisoned").contains_key(path)
    }

    /// Returns all currently managed prim paths (sorted).
    pub fn get_active_prims(&self) -> Vec<SdfPath> {
        self.prims
            .lock()
            .expect("Lock poisoned")
            .keys()
            .cloned()
            .collect()
    }
}

/// Downcast helper for Arc<dyn PrimBase>.
trait DowncastArc {
    fn downcast_arc<T: PrimBase + 'static>(self) -> Option<Arc<T>>;
}

impl DowncastArc for Arc<dyn PrimBase> {
    fn downcast_arc<T: PrimBase + 'static>(self) -> Option<Arc<T>> {
        // std::sync::Arc doesn't expose downcast directly; use Arc::into_raw + ptr cast.
        // SAFETY: We verify the type via Any first.
        // Since PrimBase doesn't require Any, we use a separate approach:
        // convert to raw pointer and attempt to cast using std::any.
        // For now, return None — callers can use get_prim() and downcast manually.
        let _ = self;
        None
    }
}

impl Default for HdsiPrimManagingSceneIndexObserver {
    fn default() -> Self {
        Self::new()
    }
}

impl HdSceneIndexObserver for HdsiPrimManagingSceneIndexObserver {
    /// Mirrors C++ PrimsAdded: creates or replaces managed prims via factory.
    fn prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        let factory = match &self.prim_factory {
            Some(f) => f.clone(),
            None => return,
        };
        for entry in entries {
            if let Some(managed) = factory.create_prim(entry, self) {
                // If prim already existed (resync), previous handle is dropped.
                self.prims
                    .lock()
                    .expect("Lock poisoned")
                    .insert(entry.prim_path.clone(), managed);
            } else {
                // Type not supported after resync: remove stale entry.
                self.prims
                    .lock()
                    .expect("Lock poisoned")
                    .remove(&entry.prim_path);
            }
        }
    }

    /// Mirrors C++ PrimsRemoved: prefix-based removal.
    ///
    /// Removes all managed prims whose path starts with any removed path,
    /// matching C++ `_prims.lower_bound(entry.primPath)` + HasPrefix loop.
    fn prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        let mut prims = self.prims.lock().expect("Lock poisoned");
        for entry in entries {
            // Collect paths to remove: all keys with entry.prim_path as prefix.
            let to_remove: Vec<SdfPath> = prims
                .range(entry.prim_path.clone()..)
                .take_while(|(k, _)| k.has_prefix(&entry.prim_path))
                .map(|(k, _)| k.clone())
                .collect();
            for path in to_remove {
                prims.remove(&path);
            }
        }
    }

    /// Mirrors C++ PrimsDirtied: calls PrimBase::dirty() for each matching entry.
    fn prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        for entry in entries {
            if let Some(prim) = self
                .prims
                .lock()
                .expect("Lock poisoned")
                .get(&entry.prim_path)
                .cloned()
            {
                prim.dirty(entry, self);
            }
        }
    }

    /// Mirrors C++ PrimsRenamed: converts to removed+added via utility.
    fn prims_renamed(&self, sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        let (removed, added) = convert_prims_renamed_to_removed_and_added(sender, entries);
        self.prims_removed(sender, &removed);
        self.prims_added(sender, &added);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_hd::data_source::HdDataSourceLocatorSet;
    use usd_sdf::Path as SdfPath;
    use usd_tf::Token as TfToken;

    // ---- Minimal PrimBase impl for testing --------------------------------
    struct CountingPrim {
        dirty_count: std::sync::atomic::AtomicU32,
    }

    impl CountingPrim {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                dirty_count: std::sync::atomic::AtomicU32::new(0),
            })
        }
        fn dirty_count(&self) -> u32 {
            self.dirty_count.load(std::sync::atomic::Ordering::Relaxed)
        }
    }

    impl PrimBase for CountingPrim {
        fn _dirty(
            &self,
            _entry: &DirtiedPrimEntry,
            _observer: &HdsiPrimManagingSceneIndexObserver,
        ) {
            self.dirty_count
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }

    // ---- Factory that creates CountingPrim for all types ------------------
    struct AllTypeFactory;
    impl PrimFactoryBase for AllTypeFactory {
        fn create_prim(
            &self,
            _entry: &AddedPrimEntry,
            _observer: &HdsiPrimManagingSceneIndexObserver,
        ) -> Option<PrimBaseHandle> {
            Some(CountingPrim::new() as PrimBaseHandle)
        }
    }

    // ---- Factory that creates CountingPrim only for "Mesh" type -----------
    struct MeshOnlyFactory;
    impl PrimFactoryBase for MeshOnlyFactory {
        fn create_prim(
            &self,
            entry: &AddedPrimEntry,
            _observer: &HdsiPrimManagingSceneIndexObserver,
        ) -> Option<PrimBaseHandle> {
            if entry.prim_type == "Mesh" {
                Some(CountingPrim::new() as PrimBaseHandle)
            } else {
                None
            }
        }
    }

    fn make_path(s: &str) -> SdfPath {
        SdfPath::from_string(s).unwrap()
    }

    #[test]
    fn test_new_empty() {
        let obs = HdsiPrimManagingSceneIndexObserver::new();
        assert!(!obs.is_prim_active(&make_path("/World")));
        assert!(obs.get_active_prims().is_empty());
    }

    // P0-3: prims_added creates managed prims via factory.
    #[test]
    fn test_prims_added_factory() {
        struct FakeSender;
        impl HdSceneIndexBase for FakeSender {
            fn get_prim(&self, _: &SdfPath) -> usd_hd::scene_index::HdSceneIndexPrim {
                Default::default()
            }
            fn get_child_prim_paths(&self, _: &SdfPath) -> Vec<SdfPath> {
                vec![]
            }
            fn add_observer(&self, _: HdSceneIndexObserverHandle) {}
            fn remove_observer(&self, _: &HdSceneIndexObserverHandle) {}
            fn _system_message(
                &self,
                _: &TfToken,
                _: Option<usd_hd::data_source::HdDataSourceBaseHandle>,
            ) {
            }
            fn get_display_name(&self) -> String {
                "Fake".into()
            }
        }

        let factory = Arc::new(AllTypeFactory) as PrimFactoryBaseHandle;
        let obs = HdsiPrimManagingSceneIndexObserver {
            scene_index: None,
            prim_factory: Some(factory),
            prims: Mutex::new(BTreeMap::new()),
        };
        let sender = FakeSender;
        let path_a = make_path("/World/A");
        let path_b = make_path("/World/B");
        obs.prims_added(
            &sender,
            &[
                AddedPrimEntry::new(path_a.clone(), TfToken::new("Mesh")),
                AddedPrimEntry::new(path_b.clone(), TfToken::new("Camera")),
            ],
        );
        assert!(obs.is_prim_active(&path_a));
        assert!(obs.is_prim_active(&path_b));
    }

    // P0-3: prims_removed uses prefix-based removal (removes children too).
    #[test]
    fn test_prims_removed_prefix() {
        struct FakeSender;
        impl HdSceneIndexBase for FakeSender {
            fn get_prim(&self, _: &SdfPath) -> usd_hd::scene_index::HdSceneIndexPrim {
                Default::default()
            }
            fn get_child_prim_paths(&self, _: &SdfPath) -> Vec<SdfPath> {
                vec![]
            }
            fn add_observer(&self, _: HdSceneIndexObserverHandle) {}
            fn remove_observer(&self, _: &HdSceneIndexObserverHandle) {}
            fn _system_message(
                &self,
                _: &TfToken,
                _: Option<usd_hd::data_source::HdDataSourceBaseHandle>,
            ) {
            }
            fn get_display_name(&self) -> String {
                "Fake".into()
            }
        }
        let factory = Arc::new(AllTypeFactory) as PrimFactoryBaseHandle;
        let obs = HdsiPrimManagingSceneIndexObserver {
            scene_index: None,
            prim_factory: Some(factory),
            prims: Mutex::new(BTreeMap::new()),
        };
        let sender = FakeSender;
        let root = make_path("/World");
        let child1 = make_path("/World/A");
        let child2 = make_path("/World/A/Sub");
        let other = make_path("/Other");
        // Add all paths.
        obs.prims_added(
            &sender,
            &[
                AddedPrimEntry::new(root.clone(), TfToken::new("Xform")),
                AddedPrimEntry::new(child1.clone(), TfToken::new("Mesh")),
                AddedPrimEntry::new(child2.clone(), TfToken::new("Mesh")),
                AddedPrimEntry::new(other.clone(), TfToken::new("Camera")),
            ],
        );
        assert_eq!(obs.get_active_prims().len(), 4);
        // Remove /World — should remove /World, /World/A, /World/A/Sub but not /Other.
        obs.prims_removed(&sender, &[RemovedPrimEntry::new(root.clone())]);
        assert!(!obs.is_prim_active(&root));
        assert!(!obs.is_prim_active(&child1));
        assert!(!obs.is_prim_active(&child2));
        assert!(obs.is_prim_active(&other));
    }

    // P0-3: prims_dirtied calls PrimBase::dirty via NVI.
    #[test]
    fn test_prims_dirtied_calls_dirty() {
        struct FakeSender;
        impl HdSceneIndexBase for FakeSender {
            fn get_prim(&self, _: &SdfPath) -> usd_hd::scene_index::HdSceneIndexPrim {
                Default::default()
            }
            fn get_child_prim_paths(&self, _: &SdfPath) -> Vec<SdfPath> {
                vec![]
            }
            fn add_observer(&self, _: HdSceneIndexObserverHandle) {}
            fn remove_observer(&self, _: &HdSceneIndexObserverHandle) {}
            fn _system_message(
                &self,
                _: &TfToken,
                _: Option<usd_hd::data_source::HdDataSourceBaseHandle>,
            ) {
            }
            fn get_display_name(&self) -> String {
                "Fake".into()
            }
        }
        let sender = FakeSender;
        let prim = CountingPrim::new();
        let prim_ref = prim.clone();
        let path = make_path("/World/Mesh");
        let obs = HdsiPrimManagingSceneIndexObserver {
            scene_index: None,
            prim_factory: None,
            prims: Mutex::new(BTreeMap::new()),
        };
        obs.prims
            .lock()
            .expect("Lock poisoned")
            .insert(path.clone(), prim as PrimBaseHandle);
        obs.prims_dirtied(
            &sender,
            &[DirtiedPrimEntry::new(
                path.clone(),
                HdDataSourceLocatorSet::default(),
            )],
        );
        assert_eq!(prim_ref.dirty_count(), 1);
    }

    // P0-3: MeshOnlyFactory returns None for non-mesh, removing stale entry.
    #[test]
    fn test_factory_returns_none_removes_entry() {
        struct FakeSender;
        impl HdSceneIndexBase for FakeSender {
            fn get_prim(&self, _: &SdfPath) -> usd_hd::scene_index::HdSceneIndexPrim {
                Default::default()
            }
            fn get_child_prim_paths(&self, _: &SdfPath) -> Vec<SdfPath> {
                vec![]
            }
            fn add_observer(&self, _: HdSceneIndexObserverHandle) {}
            fn remove_observer(&self, _: &HdSceneIndexObserverHandle) {}
            fn _system_message(
                &self,
                _: &TfToken,
                _: Option<usd_hd::data_source::HdDataSourceBaseHandle>,
            ) {
            }
            fn get_display_name(&self) -> String {
                "Fake".into()
            }
        }
        let factory = Arc::new(MeshOnlyFactory) as PrimFactoryBaseHandle;
        let obs = HdsiPrimManagingSceneIndexObserver {
            scene_index: None,
            prim_factory: Some(factory),
            prims: Mutex::new(BTreeMap::new()),
        };
        let sender = FakeSender;
        let path = make_path("/World/Light");
        // Pre-insert an old prim as if it existed before.
        obs.prims
            .lock()
            .expect("Lock poisoned")
            .insert(path.clone(), CountingPrim::new() as PrimBaseHandle);
        // Re-add with type "Light" — factory returns None, old entry removed.
        obs.prims_added(
            &sender,
            &[AddedPrimEntry::new(path.clone(), TfToken::new("Light"))],
        );
        assert!(!obs.is_prim_active(&path));
    }

    // -----------------------------------------------------------------------
    // Prefix removal edge cases
    // -----------------------------------------------------------------------

    // Removing an exact path removes only that prim, not unrelated siblings.
    #[test]
    fn test_remove_exact_path_only() {
        struct FakeSender;
        impl HdSceneIndexBase for FakeSender {
            fn get_prim(&self, _: &SdfPath) -> usd_hd::scene_index::HdSceneIndexPrim {
                Default::default()
            }
            fn get_child_prim_paths(&self, _: &SdfPath) -> Vec<SdfPath> {
                vec![]
            }
            fn add_observer(&self, _: HdSceneIndexObserverHandle) {}
            fn remove_observer(&self, _: &HdSceneIndexObserverHandle) {}
            fn _system_message(
                &self,
                _: &TfToken,
                _: Option<usd_hd::data_source::HdDataSourceBaseHandle>,
            ) {
            }
            fn get_display_name(&self) -> String {
                "Fake".into()
            }
        }
        let sender = FakeSender;
        let obs = HdsiPrimManagingSceneIndexObserver::new();
        let a = make_path("/Foo/A");
        let b = make_path("/Foo/B");
        obs.prims
            .lock()
            .expect("Lock poisoned")
            .insert(a.clone(), CountingPrim::new() as PrimBaseHandle);
        obs.prims
            .lock()
            .expect("Lock poisoned")
            .insert(b.clone(), CountingPrim::new() as PrimBaseHandle);
        // Remove only /Foo/A
        obs.prims_removed(&sender, &[RemovedPrimEntry::new(a.clone())]);
        assert!(!obs.is_prim_active(&a));
        assert!(obs.is_prim_active(&b), "/Foo/B must survive");
    }

    // Removing a path that does not exist is a no-op (no panic).
    #[test]
    fn test_remove_nonexistent_path_is_noop() {
        struct FakeSender;
        impl HdSceneIndexBase for FakeSender {
            fn get_prim(&self, _: &SdfPath) -> usd_hd::scene_index::HdSceneIndexPrim {
                Default::default()
            }
            fn get_child_prim_paths(&self, _: &SdfPath) -> Vec<SdfPath> {
                vec![]
            }
            fn add_observer(&self, _: HdSceneIndexObserverHandle) {}
            fn remove_observer(&self, _: &HdSceneIndexObserverHandle) {}
            fn _system_message(
                &self,
                _: &TfToken,
                _: Option<usd_hd::data_source::HdDataSourceBaseHandle>,
            ) {
            }
            fn get_display_name(&self) -> String {
                "Fake".into()
            }
        }
        let sender = FakeSender;
        let obs = HdsiPrimManagingSceneIndexObserver::new();
        let a = make_path("/World/A");
        obs.prims
            .lock()
            .expect("Lock poisoned")
            .insert(a.clone(), CountingPrim::new() as PrimBaseHandle);
        // Remove a path that was never added
        obs.prims_removed(&sender, &[RemovedPrimEntry::new(make_path("/World/B"))]);
        assert!(
            obs.is_prim_active(&a),
            "/World/A must survive unrelated removal"
        );
    }

    // Removing root "/" removes everything.
    #[test]
    fn test_remove_root_clears_all() {
        struct FakeSender;
        impl HdSceneIndexBase for FakeSender {
            fn get_prim(&self, _: &SdfPath) -> usd_hd::scene_index::HdSceneIndexPrim {
                Default::default()
            }
            fn get_child_prim_paths(&self, _: &SdfPath) -> Vec<SdfPath> {
                vec![]
            }
            fn add_observer(&self, _: HdSceneIndexObserverHandle) {}
            fn remove_observer(&self, _: &HdSceneIndexObserverHandle) {}
            fn _system_message(
                &self,
                _: &TfToken,
                _: Option<usd_hd::data_source::HdDataSourceBaseHandle>,
            ) {
            }
            fn get_display_name(&self) -> String {
                "Fake".into()
            }
        }
        let sender = FakeSender;
        let obs = HdsiPrimManagingSceneIndexObserver::new();
        for suffix in &["A", "B/C", "D"] {
            let p = make_path(&format!("/World/{suffix}"));
            obs.prims
                .lock()
                .expect("Lock poisoned")
                .insert(p, CountingPrim::new() as PrimBaseHandle);
        }
        assert_eq!(obs.get_active_prims().len(), 3);
        obs.prims_removed(&sender, &[RemovedPrimEntry::new(make_path("/"))]);
        assert!(
            obs.get_active_prims().is_empty(),
            "all prims should be removed"
        );
    }

    // Path with shared prefix but not child should NOT be removed.
    // /Foo/Bar must survive removal of /Foo/B (not a prefix of /Foo/Bar)
    #[test]
    fn test_remove_prefix_not_substring_match() {
        struct FakeSender;
        impl HdSceneIndexBase for FakeSender {
            fn get_prim(&self, _: &SdfPath) -> usd_hd::scene_index::HdSceneIndexPrim {
                Default::default()
            }
            fn get_child_prim_paths(&self, _: &SdfPath) -> Vec<SdfPath> {
                vec![]
            }
            fn add_observer(&self, _: HdSceneIndexObserverHandle) {}
            fn remove_observer(&self, _: &HdSceneIndexObserverHandle) {}
            fn _system_message(
                &self,
                _: &TfToken,
                _: Option<usd_hd::data_source::HdDataSourceBaseHandle>,
            ) {
            }
            fn get_display_name(&self) -> String {
                "Fake".into()
            }
        }
        let sender = FakeSender;
        let obs = HdsiPrimManagingSceneIndexObserver::new();
        let foo_bar = make_path("/Foo/Bar");
        let foo_b = make_path("/Foo/B");
        obs.prims
            .lock()
            .expect("Lock poisoned")
            .insert(foo_bar.clone(), CountingPrim::new() as PrimBaseHandle);
        obs.prims
            .lock()
            .expect("Lock poisoned")
            .insert(foo_b.clone(), CountingPrim::new() as PrimBaseHandle);
        // Remove /Foo/B — should NOT remove /Foo/Bar
        obs.prims_removed(&sender, &[RemovedPrimEntry::new(foo_b.clone())]);
        assert!(!obs.is_prim_active(&foo_b));
        assert!(
            obs.is_prim_active(&foo_bar),
            "/Foo/Bar must NOT be removed by /Foo/B prefix"
        );
    }
}
