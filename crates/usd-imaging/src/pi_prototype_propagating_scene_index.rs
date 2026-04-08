#![allow(dead_code)]
//! PiPrototypePropagatingSceneIndex - Propagates prototype data for point instancers.
//!
//! Port of pxr/usdImaging/usdImaging/piPrototypePropagatingSceneIndex.cpp (~735 lines).
//!
//! A scene index translating USD point instancers into Hydra instancers.
//! It discovers point instancers in the scene, creates re-rooted copies of
//! their prototypes, and updates instancer topology to reference the copies.
//!
//! ## Architecture (matching C++)
//!
//! - `Context`: shared state with merging + retained + usdPrimInfo scene indices
//! - `_InstancerObserver`: recursive observer that watches for point instancers,
//!   creates PiPrototypeSceneIndex + rerooting for each prototype, and adds them
//!   to the merging scene index. Nested instancers get sub-observers.
//! - `_MergingSceneIndexObserver`: forwards merging SI notifications to outer observers.
//! - `GetPrim`/`GetChildPrimPaths`: delegate to the merging scene index.

use parking_lot::RwLock;
use std::collections::BTreeMap;
use std::sync::{Arc, Weak};
use usd_hd::data_source::{
    HdDataSourceBaseHandle, HdDataSourceLocator, HdRetainedContainerDataSource,
    HdRetainedTypedSampledDataSource,
};
use usd_hd::scene_index::observer::{
    AddedPrimEntry, DirtiedPrimEntry, HdSceneIndexObserver, HdSceneIndexObserverHandle,
    RemovedPrimEntry, RenamedPrimEntry,
};
use usd_hd::scene_index::retained::{HdRetainedSceneIndex, RetainedAddedPrimEntry};
use usd_hd::scene_index::{
    HdContainerDataSourceHandle, HdMergingSceneIndex, HdSceneIndexBase, HdSceneIndexHandle,
    HdSceneIndexPrim, SdfPathVector, si_ref,
};
use usd_hd::schema::HdInstancerTopologySchema;
use usd_sdf::Path;
use usd_tf::Token;

mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static INSTANCER: LazyLock<Token> = LazyLock::new(|| Token::new("instancer"));
    pub static PROTOTYPES: LazyLock<Token> = LazyLock::new(|| Token::new("prototypes"));
    pub static INSTANCED_BY: LazyLock<Token> = LazyLock::new(|| Token::new("instancedBy"));
    pub static PROTOTYPE_ROOT: LazyLock<Token> = LazyLock::new(|| Token::new("prototypeRoot"));
    pub static INSTANCER_TOPOLOGY: LazyLock<Token> =
        LazyLock::new(|| Token::new("instancerTopology"));
}

// ---------------------------------------------------------------------------
// _PropagatedPrototypesSource (C++ data source for __usdPrimInfo)
// ---------------------------------------------------------------------------

/// Map from instancer hash tokens to propagated prototype paths.
/// Port of C++ `_PropagatedPrototypesSource`.
#[derive(Debug, Clone, Default)]
struct PropagatedPrototypesSource {
    map: BTreeMap<Token, Path>,
}

impl PropagatedPrototypesSource {
    fn add(&mut self, instancer_hash: Token, propagated_prototype: Path) {
        self.map.insert(instancer_hash, propagated_prototype);
    }

    fn remove(&mut self, instancer_hash: &Token) {
        self.map.remove(instancer_hash);
    }

    fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

// ---------------------------------------------------------------------------
// _UsdPrimInfoSceneIndex (retained SI for __usdPrimInfo:piPropagatedPrototypes)
// ---------------------------------------------------------------------------

/// Retained scene index providing `__usdPrimInfo:piPropagatedPrototypes`.
/// Port of C++ `_UsdPrimInfoSceneIndex`.
struct UsdPrimInfoSceneIndex {
    inner: Arc<RwLock<HdRetainedSceneIndex>>,
    sources: BTreeMap<Path, PropagatedPrototypesSource>,
}

impl UsdPrimInfoSceneIndex {
    fn new() -> Self {
        Self {
            inner: HdRetainedSceneIndex::new(),
            sources: BTreeMap::new(),
        }
    }

    /// Record that `propagated_prototype` is the re-rooted copy for
    /// `prototype` created by the instancer identified by `instancer_hash`.
    fn add_propagated_prototype(
        &mut self,
        prototype: &Path,
        instancer_hash: Token,
        propagated_prototype: Path,
    ) {
        let src = self.sources.entry(prototype.clone()).or_default();
        src.add(instancer_hash, propagated_prototype);
        // Dirty the prim so downstream observers see the change
        self.inner.write().add_prims(&[RetainedAddedPrimEntry::new(
            prototype.clone(),
            Token::empty(),
            None,
        )]);
    }

    /// Remove propagated prototype entry.
    fn remove_propagated_prototype(&mut self, prototype: &Path, instancer_hash: &Token) {
        if let Some(src) = self.sources.get_mut(prototype) {
            src.remove(instancer_hash);
            if src.is_empty() {
                self.sources.remove(prototype);
                self.inner
                    .write()
                    .remove_prims(&vec![RemovedPrimEntry::new(prototype.clone())]);
            }
        }
    }

    /// Get the inner scene index handle for adding to merging SI.
    fn handle(&self) -> HdSceneIndexHandle {
        self.inner.clone() as HdSceneIndexHandle
    }
}

// ---------------------------------------------------------------------------
// Context (shared between all _InstancerObservers)
// ---------------------------------------------------------------------------

/// Shared context for prototype propagation.
///
/// Port of C++ `_Context`. Contains the three scene indices:
/// - `input_scene`: original input (e.g. UsdImagingStageSceneIndex)
/// - `instancer_scene`: retained SI for instancer topology overrides
/// - `usd_prim_info_scene`: retained SI for __usdPrimInfo
/// - `merging_scene`: merges input + instancer + usdPrimInfo + re-rooted prototypes
struct Context {
    input_scene: HdSceneIndexHandle,
    /// Retained SI overriding instancer topology to point to re-rooted prototypes
    instancer_scene: Arc<RwLock<HdRetainedSceneIndex>>,
    /// Retained SI providing __usdPrimInfo:piPropagatedPrototypes
    usd_prim_info_scene: UsdPrimInfoSceneIndex,
    /// Output merging scene index
    merging_scene: Arc<RwLock<HdMergingSceneIndex>>,
}

impl Context {
    /// Create a new context. Merging SI initially contains instancer + usdPrimInfo.
    /// Input scene is added later by PiPrototypePropagatingSceneIndex::new().
    fn new(input_scene: HdSceneIndexHandle) -> Self {
        let instancer_scene = HdRetainedSceneIndex::new();
        let usd_prim_info_scene = UsdPrimInfoSceneIndex::new();
        let merging_scene = HdMergingSceneIndex::new();

        // Add retained overrides first (stronger opinions)
        {
            let ms = merging_scene.write();
            ms.add_input_scene(
                instancer_scene.clone() as HdSceneIndexHandle,
                Path::absolute_root(),
            );
            ms.add_input_scene(usd_prim_info_scene.handle(), Path::absolute_root());
        }

        Self {
            input_scene,
            instancer_scene,
            usd_prim_info_scene,
            merging_scene,
        }
    }
}

// ---------------------------------------------------------------------------
// Helper: rerooting scene index creation (matches C++ free function)
// ---------------------------------------------------------------------------

/// Create a rerooting scene index, or return input unchanged if both
/// src_prefix and dst_prefix are absolute root.
fn make_rerooting_scene(
    scene_index: HdSceneIndexHandle,
    src_prefix: &Path,
    dst_prefix: &Path,
) -> HdSceneIndexHandle {
    use crate::rerooting_scene_index::HdRerootingSceneIndex;

    if src_prefix.is_absolute_root_path() && dst_prefix.is_absolute_root_path() {
        scene_index
    } else {
        HdRerootingSceneIndex::new_with_prefixes(
            Some(scene_index),
            src_prefix.clone(),
            dst_prefix.clone(),
        ) as HdSceneIndexHandle
    }
}

// ---------------------------------------------------------------------------
// Helper: get prototypes array from instancer prim
// ---------------------------------------------------------------------------

/// Extract prototype paths from an instancer prim's topology.
fn get_prototypes(prim: &HdSceneIndexPrim) -> Vec<Path> {
    let ds = match &prim.data_source {
        Some(ds) => ds,
        None => return Vec::new(),
    };

    let topo_schema = HdInstancerTopologySchema::get_from_parent(ds);
    if !topo_schema.is_defined() {
        return Vec::new();
    }

    if let Some(proto_ds) = topo_schema.get_prototypes() {
        proto_ds.get_typed_value(0.0)
    } else {
        Vec::new()
    }
}

/// Build an instancer topology data source with just prototypes.
fn instancer_topology_ds(prototypes: &[Path]) -> Option<HdContainerDataSourceHandle> {
    let proto_ds = HdRetainedTypedSampledDataSource::new(prototypes.to_vec());
    let topo = HdInstancerTopologySchema::build_retained(Some(proto_ds), None, None, None);

    Some(HdRetainedContainerDataSource::from_entries(&[(
        tokens::INSTANCER_TOPOLOGY.clone(),
        topo as HdDataSourceBaseHandle,
    )]))
}

// ---------------------------------------------------------------------------
// _InstancerObserver
// ---------------------------------------------------------------------------

/// Recursive instancer observer.
///
/// Port of C++ `_InstancerObserver`. Each instance observes a (possibly
/// re-rooted) prototype hierarchy for point instancers. When an instancer
/// is found, it:
/// 1. Creates a PiPrototypeSceneIndex wrapping a rerooting of the input
/// 2. Creates a rerooting SI to place the prototype at a unique path
/// 3. Adds the result to the merging SI
/// 4. Updates the instancer's prototypes in the retained SI
/// 5. Creates sub-observers for nested instancers
struct InstancerObserver {
    context: Arc<RwLock<Context>>,
    /// Original prototype path (what instancer references)
    prototype: Path,
    /// Re-rooted prototype path (where it appears in output)
    propagated_prototype: Path,
    /// The PiPrototypeSceneIndex for this observer
    prototype_scene_index: Option<HdSceneIndexHandle>,
    /// The rerooting scene index wrapping prototype_scene_index
    rerooting_scene_index: Option<HdSceneIndexHandle>,
    /// Nested instancer observers: instancer_path -> (prototype_path -> observer)
    sub_instancer_observers: BTreeMap<Path, BTreeMap<Path, Box<InstancerObserver>>>,
}

impl InstancerObserver {
    /// Root-level constructor (for the top-level scene).
    /// Port of C++ `_InstancerObserver(_ContextSharedPtr)`.
    fn new_root(context: Arc<RwLock<Context>>) -> Self {
        let mut obs = Self::new_inner(
            context,
            &Path::empty(), // no instancer
            &Path::absolute_root(),
            &Path::absolute_root(),
        );
        obs.populate();
        obs
    }

    /// Prototype-level constructor.
    /// Port of C++ `_InstancerObserver(context, instancer, prototype, propagatedPrototype)`.
    fn new_for_prototype(
        context: Arc<RwLock<Context>>,
        instancer: &Path,
        prototype: &Path,
        propagated_prototype: &Path,
    ) -> Self {
        let mut obs = Self::new_inner(context, instancer, prototype, propagated_prototype);

        // Add the rerooting scene index to the merging scene index
        if let Some(ref rerooting_si) = obs.rerooting_scene_index {
            let ctx = obs.context.read();
            ctx.merging_scene
                .write()
                .add_input_scene(rerooting_si.clone(), propagated_prototype.clone());
        }

        obs.populate();
        obs
    }

    fn new_inner(
        context: Arc<RwLock<Context>>,
        instancer: &Path,
        prototype: &Path,
        propagated_prototype: &Path,
    ) -> Self {
        use crate::pi_prototype_scene_index::PiPrototypeSceneIndex;

        // Build prototype scene index chain:
        // input -> rerooting(proto,proto) -> PiPrototypeSceneIndex -> rerooting(proto, propagated)
        let input_scene = context.read().input_scene.clone();

        // Isolate the prototype subtree
        let isolated = make_rerooting_scene(input_scene, prototype, prototype);

        // Create PiPrototypeSceneIndex
        let proto_si = PiPrototypeSceneIndex::new(isolated, instancer.clone(), prototype.clone())
            as HdSceneIndexHandle;

        // Re-root from prototype to propagated_prototype
        let rerooting_si = make_rerooting_scene(proto_si.clone(), prototype, propagated_prototype);

        Self {
            context,
            prototype: prototype.clone(),
            propagated_prototype: propagated_prototype.clone(),
            prototype_scene_index: Some(proto_si),
            rerooting_scene_index: Some(rerooting_si),
            sub_instancer_observers: BTreeMap::new(),
        }
    }

    /// Compute the path in the output space for a given instancer path.
    fn rerooted_path(&self, instancer: &Path) -> Path {
        instancer
            .replace_prefix(&self.prototype, &self.propagated_prototype)
            .unwrap_or_else(|| instancer.clone())
    }

    /// Compute unique hash token for re-rooted prototype naming.
    /// Port of C++ `_InstancerHash`.
    fn instancer_hash(&self, instancer: &Path) -> Token {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        instancer.get_text().hash(&mut hasher);
        self.propagated_prototype.get_text().hash(&mut hasher);
        Token::new(&format!("ForInstancer{:x}", hasher.finish()))
    }

    /// Traverse the prototype scene index and find all instancers.
    /// Port of C++ `_Populate`.
    fn populate(&mut self) {
        let proto_si = match &self.prototype_scene_index {
            Some(si) => si.clone(),
            None => return,
        };

        // DFS traversal looking for instancer prims
        let mut stack = vec![self.prototype.clone()];
        let mut skip_set: std::collections::HashSet<Path> = std::collections::HashSet::new();

        while let Some(path) = stack.pop() {
            if skip_set.iter().any(|s| path.has_prefix(s)) {
                continue;
            }

            let prim = si_ref(&proto_si).get_prim(&path);

            if prim.prim_type == *tokens::INSTANCER {
                // Found an instancer - process its prototypes
                self.update_instancer_from_prim(&path, &prim);
                // Skip descendants: nested instancers inside prototypes
                // will be discovered by the sub-observer
                skip_set.insert(path.clone());
                continue;
            }

            // Get children and push onto stack (reversed for correct DFS order)
            let children = si_ref(&proto_si).get_child_prim_paths(&path);

            for child in children.into_iter().rev() {
                stack.push(child);
            }
        }
    }

    /// Convenience: update instancer from prim data.
    fn update_instancer_from_prim(&mut self, path: &Path, prim: &HdSceneIndexPrim) {
        let prototypes = get_prototypes(prim);
        let rerooted_instancer = self.rerooted_path(path);
        let instancer_hash = self.instancer_hash(path);
        let observers = self
            .sub_instancer_observers
            .entry(path.clone())
            .or_default();
        Self::update_instancer_prototypes_static(
            &self.context,
            observers,
            path,
            &prototypes,
            &rerooted_instancer,
            &instancer_hash,
        );
    }

    /// Convenience: update instancer by querying prototype scene index.
    fn update_instancer(&mut self, path: &Path) {
        let prim = match self.prototype_scene_index {
            Some(ref proto_si) => si_ref(proto_si).get_prim(path),
            None => return,
        };
        self.update_instancer_from_prim(path, &prim);
    }

    /// Process an instancer's prototypes - create/update sub-observers.
    /// Port of C++ `_UpdateInstancerPrototypes`.
    ///
    /// Static to avoid borrow issues with self.sub_instancer_observers.
    fn update_instancer_prototypes_static(
        context: &Arc<RwLock<Context>>,
        proto_to_observer: &mut BTreeMap<Path, Box<InstancerObserver>>,
        _instancer: &Path,
        prototypes: &[Path],
        rerooted_instancer: &Path,
        instancer_hash: &Token,
    ) {
        // Remove stale observers
        let prototype_set: std::collections::HashSet<&Path> = prototypes.iter().collect();
        let stale_keys: Vec<Path> = proto_to_observer
            .keys()
            .filter(|p| !prototype_set.contains(p))
            .cloned()
            .collect();
        for key in stale_keys {
            context
                .write()
                .usd_prim_info_scene
                .remove_propagated_prototype(&key, instancer_hash);
            proto_to_observer.remove(&key);
        }

        // Create new sub-observers and propagated paths
        let mut propagated_prototypes = Vec::with_capacity(prototypes.len());
        for prototype in prototypes {
            let propagated_prototype = prototype
                .append_child(instancer_hash.as_str())
                .unwrap_or_else(|| prototype.clone());
            propagated_prototypes.push(propagated_prototype.clone());

            if !proto_to_observer.contains_key(prototype) {
                let sub_obs = Box::new(InstancerObserver::new_for_prototype(
                    context.clone(),
                    rerooted_instancer,
                    prototype,
                    &propagated_prototype,
                ));
                proto_to_observer.insert(prototype.clone(), sub_obs);

                context
                    .write()
                    .usd_prim_info_scene
                    .add_propagated_prototype(
                        prototype,
                        instancer_hash.clone(),
                        propagated_prototype.clone(),
                    );
            }
        }

        // Update instancer topology in retained SI
        if let Some(topo_ds) = instancer_topology_ds(&propagated_prototypes) {
            let ctx = context.read();
            ctx.instancer_scene
                .write()
                .add_prims(&[RetainedAddedPrimEntry::new(
                    rerooted_instancer.clone(),
                    tokens::INSTANCER.clone(),
                    Some(topo_ds),
                )]);
        }
    }

    /// Handle PrimsAdded from prototype scene index.
    fn handle_prims_added(&mut self, entries: &[AddedPrimEntry]) {
        for entry in entries {
            if entry.prim_type == *tokens::INSTANCER {
                self.update_instancer(&entry.prim_path);
            } else {
                // Prim re-synced and is no longer an instancer - clean up
                if self
                    .sub_instancer_observers
                    .remove(&entry.prim_path)
                    .is_some()
                {
                    let ctx = self.context.read();
                    ctx.instancer_scene
                        .write()
                        .remove_prims(&vec![RemovedPrimEntry::new(
                            self.rerooted_path(&entry.prim_path),
                        )]);
                }
            }
        }
    }

    /// Handle PrimsDirtied from prototype scene index.
    fn handle_prims_dirtied(&mut self, entries: &[DirtiedPrimEntry]) {
        if self.sub_instancer_observers.is_empty() {
            return;
        }

        let prototypes_locator = HdDataSourceLocator::from_tokens_2(
            tokens::INSTANCER_TOPOLOGY.clone(),
            tokens::PROTOTYPES.clone(),
        );

        for entry in entries {
            if !entry.dirty_locators.contains(&prototypes_locator) {
                continue;
            }
            // Only process known sub-instancers
            if !self.sub_instancer_observers.contains_key(&entry.prim_path) {
                continue;
            }

            let prim = match self.prototype_scene_index {
                Some(ref proto_si) => si_ref(proto_si).get_prim(&entry.prim_path),
                None => continue,
            };

            let prototypes = get_prototypes(&prim);
            let rerooted = self.rerooted_path(&entry.prim_path);
            let hash = self.instancer_hash(&entry.prim_path);

            // Must temporarily remove the entry to avoid double borrow
            if let Some(mut observers) = self.sub_instancer_observers.remove(&entry.prim_path) {
                Self::update_instancer_prototypes_static(
                    &self.context,
                    &mut observers,
                    &entry.prim_path,
                    &prototypes,
                    &rerooted,
                    &hash,
                );
                self.sub_instancer_observers
                    .insert(entry.prim_path.clone(), observers);
            }
        }
    }

    /// Handle PrimsRemoved from prototype scene index.
    fn handle_prims_removed(&mut self, entries: &[RemovedPrimEntry]) {
        if self.sub_instancer_observers.is_empty() {
            return;
        }

        let mut removed_instancers = Vec::new();

        for entry in entries {
            let to_remove: Vec<Path> = self
                .sub_instancer_observers
                .keys()
                .filter(|k| k.has_prefix(&entry.prim_path))
                .cloned()
                .collect();

            for key in to_remove {
                removed_instancers.push(RemovedPrimEntry::new(self.rerooted_path(&key)));
                self.sub_instancer_observers.remove(&key);
            }
        }

        if !removed_instancers.is_empty() {
            let ctx = self.context.read();
            ctx.instancer_scene
                .write()
                .remove_prims(&removed_instancers);
        }
    }
}

impl Drop for InstancerObserver {
    /// RAII cleanup: remove scene indices and prims added by this observer.
    fn drop(&mut self) {
        // Remove sub-instancer entries from retained SI
        if !self.sub_instancer_observers.is_empty() {
            let removed: Vec<RemovedPrimEntry> = self
                .sub_instancer_observers
                .keys()
                .map(|k| RemovedPrimEntry::new(self.rerooted_path(k)))
                .collect();

            let ctx = self.context.read();
            ctx.instancer_scene.write().remove_prims(&removed);
            drop(ctx);
            self.sub_instancer_observers.clear();
        }

        // Remove rerooting scene index from merging SI (reverse order of addition)
        if let Some(ref rerooting_si) = self.rerooting_scene_index {
            let ctx = self.context.read();
            ctx.merging_scene.write().remove_input_scene(rerooting_si);
        }
    }
}

// ---------------------------------------------------------------------------
// MergingSceneIndexObserver
// ---------------------------------------------------------------------------

/// Observer on the merging scene index that forwards notifications to
/// the outer PiPrototypePropagatingSceneIndex's observers.
///
/// Port of C++ `_MergingSceneIndexObserver`.
struct MergingSceneIndexObserver {
    owner: Weak<RwLock<PiPrototypePropagatingSceneIndex>>,
}

impl MergingSceneIndexObserver {
    fn new(owner: Weak<RwLock<PiPrototypePropagatingSceneIndex>>) -> Self {
        Self { owner }
    }
}

impl HdSceneIndexObserver for MergingSceneIndexObserver {
    fn prims_added(&self, sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        if let Some(owner) = self.owner.upgrade() {
            let owner_lock = unsafe { &*owner.data_ptr() };
            owner_lock.notify_prims_added(sender, entries);
        }
    }

    fn prims_removed(&self, sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        if let Some(owner) = self.owner.upgrade() {
            let owner_lock = unsafe { &*owner.data_ptr() };
            owner_lock.notify_prims_removed(sender, entries);
        }
    }

    fn prims_dirtied(&self, sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        if let Some(owner) = self.owner.upgrade() {
            let owner_lock = unsafe { &*owner.data_ptr() };
            owner_lock.notify_prims_dirtied(sender, entries);
        }
    }

    fn prims_renamed(&self, sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        // Convert rename to remove+add, matching C++
        let (removed, added) =
            usd_hd::scene_index::observer::convert_prims_renamed_to_removed_and_added(
                sender, entries,
            );
        if !removed.is_empty() {
            self.prims_removed(sender, &removed);
        }
        if !added.is_empty() {
            self.prims_added(sender, &added);
        }
    }
}

// ---------------------------------------------------------------------------
// PiPrototypePropagatingSceneIndex
// ---------------------------------------------------------------------------

/// A scene index translating USD point instancers into Hydra instancers.
///
/// Port of C++ `UsdImagingPiPrototypePropagatingSceneIndex`.
///
/// This is the main entry point for point instancer translation. It:
/// - Creates a merging scene index combining input + retained overrides + re-rooted prototypes
/// - Discovers point instancers via recursive `_InstancerObserver`
/// - Creates PiPrototypeSceneIndex + rerooting for each prototype
/// - Updates instancer topology to reference re-rooted copies
/// - Forwards all queries and notifications through the merging scene index
pub struct PiPrototypePropagatingSceneIndex {
    /// Shared context with merging/retained scene indices
    context: Arc<RwLock<Context>>,
    /// Root instancer observer (scans entire scene for instancers)
    instancer_observer: Option<InstancerObserver>,
    /// Registered observers for forwarding notifications
    observers: std::sync::Mutex<Vec<HdSceneIndexObserverHandle>>,
}

impl std::fmt::Debug for PiPrototypePropagatingSceneIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PiPrototypePropagatingSceneIndex").finish()
    }
}

impl PiPrototypePropagatingSceneIndex {
    /// Creates a new prototype propagating scene index.
    ///
    /// Sets up the context (merging + retained SIs), adds the input scene
    /// to the merging SI, creates the root instancer observer, and registers
    /// the merging SI observer for notification forwarding.
    pub fn new(input: HdSceneIndexHandle) -> Arc<RwLock<Self>> {
        let context = Arc::new(RwLock::new(Context::new(input.clone())));

        // Add input scene to merging SI (weakest opinion, added last)
        {
            let ctx = context.read();
            ctx.merging_scene
                .write()
                .add_input_scene(input, Path::absolute_root());
        }

        // Create root instancer observer
        let instancer_observer = InstancerObserver::new_root(context.clone());

        let si = Arc::new(RwLock::new(Self {
            context,
            instancer_observer: Some(instancer_observer),
            observers: std::sync::Mutex::new(Vec::new()),
        }));

        // Register merging SI observer for notification forwarding
        let weak_si = Arc::downgrade(&si);
        let merging_obs =
            Arc::new(MergingSceneIndexObserver::new(weak_si)) as HdSceneIndexObserverHandle;

        {
            let si_lock = si.read();
            let ctx = si_lock.context.read();
            ctx.merging_scene.read().add_observer(merging_obs);
        }

        si
    }

    /// Get input scenes (for scene debugger).
    pub fn get_input_scenes(&self) -> Vec<HdSceneIndexHandle> {
        vec![self.context.read().input_scene.clone()]
    }

    /// Get encapsulated scenes (for scene debugger).
    pub fn get_encapsulated_scenes(&self) -> Vec<HdSceneIndexHandle> {
        vec![self.context.read().merging_scene.clone() as HdSceneIndexHandle]
    }

    /// Forward PrimsAdded to registered observers.
    fn notify_prims_added(&self, sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        let observers = self.observers.lock().expect("Lock poisoned");
        for obs in observers.iter() {
            obs.prims_added(sender, entries);
        }
    }

    /// Forward PrimsRemoved to registered observers.
    fn notify_prims_removed(&self, sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        let observers = self.observers.lock().expect("Lock poisoned");
        for obs in observers.iter() {
            obs.prims_removed(sender, entries);
        }
    }

    /// Forward PrimsDirtied to registered observers.
    fn notify_prims_dirtied(&self, sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        let observers = self.observers.lock().expect("Lock poisoned");
        for obs in observers.iter() {
            obs.prims_dirtied(sender, entries);
        }
    }
}

impl HdSceneIndexBase for PiPrototypePropagatingSceneIndex {
    /// Get prim from the merging scene index.
    fn get_prim(&self, prim_path: &Path) -> HdSceneIndexPrim {
        let ctx = self.context.read();
        let inner = unsafe { &*ctx.merging_scene.data_ptr() };
        inner.get_prim(prim_path)
    }

    /// Get child prim paths from the merging scene index.
    fn get_child_prim_paths(&self, prim_path: &Path) -> SdfPathVector {
        let ctx = self.context.read();
        let inner = unsafe { &*ctx.merging_scene.data_ptr() };
        inner.get_child_prim_paths(prim_path)
    }

    fn add_observer(&self, observer: HdSceneIndexObserverHandle) {
        let mut observers = self.observers.lock().expect("Lock poisoned");
        observers.push(observer);
    }

    fn remove_observer(&self, observer: &HdSceneIndexObserverHandle) {
        let target_ptr = Arc::as_ptr(observer) as *const ();
        let mut observers = self.observers.lock().expect("Lock poisoned");
        observers.retain(|obs| Arc::as_ptr(obs) as *const () != target_ptr);
    }

    fn _system_message(&self, _message_type: &Token, _args: Option<HdDataSourceBaseHandle>) {}

    fn get_display_name(&self) -> String {
        "PiPrototypePropagatingSceneIndex".to_string()
    }
}

impl usd_hd::scene_index::filtering::FilteringObserverTarget for PiPrototypePropagatingSceneIndex {
    fn on_prims_added(&self, sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        self.notify_prims_added(sender, entries);
    }
    fn on_prims_removed(&self, sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        self.notify_prims_removed(sender, entries);
    }
    fn on_prims_dirtied(&self, sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        self.notify_prims_dirtied(sender, entries);
    }
    fn on_prims_renamed(&self, sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        let _ = (sender, entries); // renamed handled by internal observer
    }
}

/// Handle type for PiPrototypePropagatingSceneIndex.
pub type PiPrototypePropagatingSceneIndexHandle = Arc<RwLock<PiPrototypePropagatingSceneIndex>>;

/// Creates a new prototype propagating scene index.
pub fn create_pi_prototype_propagating_scene_index(
    input: HdSceneIndexHandle,
) -> PiPrototypePropagatingSceneIndexHandle {
    PiPrototypePropagatingSceneIndex::new(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Minimal scene index for testing
    use usd_hd::scene_index::merging::MergingSender;

    fn test_input() -> HdSceneIndexHandle {
        Arc::new(RwLock::new(MergingSender)) as HdSceneIndexHandle
    }

    #[test]
    fn test_instancer_hash_deterministic() {
        let context = Arc::new(RwLock::new(Context::new(test_input())));
        let obs = InstancerObserver {
            context,
            prototype: Path::absolute_root(),
            propagated_prototype: Path::absolute_root(),
            prototype_scene_index: None,
            rerooting_scene_index: None,
            sub_instancer_observers: BTreeMap::new(),
        };
        let instancer = Path::from_string("/MyInstancer").unwrap();
        let hash1 = obs.instancer_hash(&instancer);
        let hash2 = obs.instancer_hash(&instancer);
        assert_eq!(hash1, hash2);
        assert!(hash1.as_str().starts_with("ForInstancer"));
    }

    #[test]
    fn test_rerooted_path() {
        let context = Arc::new(RwLock::new(Context::new(test_input())));
        let obs = InstancerObserver {
            context,
            prototype: Path::from_string("/Proto").unwrap(),
            propagated_prototype: Path::from_string("/Proto/ForInstancer123").unwrap(),
            prototype_scene_index: None,
            rerooting_scene_index: None,
            sub_instancer_observers: BTreeMap::new(),
        };
        let instancer = Path::from_string("/Proto/Instancer").unwrap();
        let rerooted = obs.rerooted_path(&instancer);
        assert_eq!(rerooted.get_text(), "/Proto/ForInstancer123/Instancer");
    }

    #[test]
    fn test_get_prototypes_empty() {
        let prim = HdSceneIndexPrim::default();
        assert!(get_prototypes(&prim).is_empty());
    }

    #[test]
    fn test_propagated_prototypes_source() {
        let mut src = PropagatedPrototypesSource::default();
        assert!(src.is_empty());

        src.add(
            Token::new("ForInstancer123"),
            Path::from_string("/Proto/ForInstancer123").unwrap(),
        );
        assert!(!src.is_empty());

        src.remove(&Token::new("ForInstancer123"));
        assert!(src.is_empty());
    }

    #[test]
    fn test_context_creation() {
        let ctx = Context::new(test_input());
        // Merging SI should have 2 inputs (instancer + usdPrimInfo)
        assert_eq!(ctx.merging_scene.read().get_input_scenes().len(), 2);
    }

    #[test]
    fn test_scene_index_creation() {
        let si = PiPrototypePropagatingSceneIndex::new(test_input());
        let si_lock = si.read();
        // Merging SI should have 3 inputs (instancer + usdPrimInfo + input)
        let ctx = si_lock.context.read();
        assert_eq!(ctx.merging_scene.read().get_input_scenes().len(), 3);
    }

    #[test]
    fn test_get_prim_delegates_to_merging() {
        let si = PiPrototypePropagatingSceneIndex::new(test_input());
        let si_lock = si.read();
        // Should not crash, returns empty prim
        let prim = si_lock.get_prim(&Path::from_string("/World").unwrap());
        assert!(!prim.is_defined());
    }

    #[test]
    fn test_get_child_prim_paths_delegates_to_merging() {
        let si = PiPrototypePropagatingSceneIndex::new(test_input());
        let si_lock = si.read();
        let children = si_lock.get_child_prim_paths(&Path::absolute_root());
        assert!(children.is_empty());
    }
}
