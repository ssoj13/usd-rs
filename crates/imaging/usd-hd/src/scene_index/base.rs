
//! Base scene index trait and implementation.

use super::observer::{
    AddedPrimEntry, DirtiedPrimEntry, HdSceneIndexObserverHandle, RemovedPrimEntry,
    RenamedPrimEntry,
};
use crate::flo_debug::{flo_debug_enabled, summarize_dirtied_entries};
use super::prim::HdSceneIndexPrim;
use std::collections::HashSet;
use std::sync::{Arc, Weak};

// Scene index handles use parking_lot::RwLock instead of std::sync::RwLock.
// std::sync::RwLock (backed by Windows SRWLock) deadlocks when observer
// notifications cascade through the scene index chain on a single thread.
// parking_lot::RwLock guarantees reentrant reads and avoids this.
pub use parking_lot::RwLock as SceneRwLock;

/// Returns a shared reference to the contents of a scene-index lock without
/// taking a read guard.
///
/// # Why this exists
///
/// The scene-index observer chain is re-entrant by design. During a notice
/// cascade, callbacks can immediately re-enter upstream scene indices that are
/// already on the current stack. With `parking_lot::RwLock`, taking a normal
/// `.read()` in that situation can deadlock on Windows and in other callback
/// chains where the same lock is visited recursively.
///
/// Hydra's scene-index API is fundamentally `&self` + interior mutability,
/// mirroring OpenUSD's shared-pointer model rather than a Rust ownership model
/// where each query would acquire an external guard. This helper therefore
/// centralizes the one deliberate lock-free dereference needed to preserve the
/// reference runtime behavior without scattering raw `unsafe { &*data_ptr() }`
/// sites across the codebase.
///
/// # Safety contract
///
/// Callers must use this only for read-only access patterns that already obey
/// the scene-index threading contract: methods take `&self`, observers are
/// serialized externally, and mutation happens through the owning scene-index
/// implementation's interior synchronization.
#[inline]
#[allow(unsafe_code)]
pub fn rwlock_data_ref<T: ?Sized>(lock: &SceneRwLock<T>) -> &T {
    unsafe { &*lock.data_ptr() }
}

/// Access the inner scene index trait object without taking a read lock.
///
/// This is the trait-object convenience wrapper around [`rwlock_data_ref`], so
/// higher-level scene-index code can stay explicit about the re-entrant
/// lock-free access point.
pub fn si_ref(handle: &HdSceneIndexHandle) -> &dyn HdSceneIndexBase {
    rwlock_data_ref(handle.as_ref())
}
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;

// Forward declare for data source types

/// Handle to container data source providing structured prim data.
/// Maps to `HdContainerDataSourceHandle` in OpenUSD.
pub type HdContainerDataSourceHandle = super::prim::HdContainerDataSourceHandle;

/// Base handle for any data source type.
/// Re-export from data_source module.
pub use crate::data_source::HdDataSourceBaseHandle;

/// Hierarchical path into a container data source (e.g., "primvars/color").
/// See `HdDataSourceLocator` in OpenUSD.
pub type HdDataSourceLocator = crate::data_source::HdDataSourceLocator;

/// Type alias for scene index paths.
pub type SdfPathVector = Vec<SdfPath>;

/// Type alias for token vectors.
pub type TfTokenVector = Vec<TfToken>;

/// Base trait for all scene indices.
///
/// Provides the core interface for querying scene data and managing observers.
/// All scene indices must implement this trait.
///
/// # Thread Safety
///
/// - GetPrim() and GetChildPrimPaths() MUST be thread-safe
/// - Observer add/remove and notifications are NOT thread-safe
/// - Observers are called from a single thread
pub trait HdSceneIndexBase: Send + Sync {
    /// Returns a prim at the given path.
    ///
    /// A prim exists if and only if the returned data source is non-null.
    /// The prim type or container may be empty, but we still consider the
    /// prim to exist if the data source pointer is set.
    ///
    /// This function must be thread-safe.
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim;

    /// Returns the paths of all immediate children of a prim.
    ///
    /// This can be used to traverse the scene by recursing from
    /// SdfPath::AbsoluteRoot. The traversal should give exactly the set
    /// of paths where prims exist (as defined by GetPrim).
    ///
    /// This function must be thread-safe.
    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector;

    /// Get a data source at a specific locator within a prim.
    ///
    /// This is a convenience function equivalent to calling GetPrim()
    /// and then extracting the data source at the locator.
    fn get_data_source(
        &self,
        prim_path: &SdfPath,
        locator: &HdDataSourceLocator,
    ) -> Option<HdDataSourceBaseHandle> {
        // Get the prim and its data source
        let prim = self.get_prim(prim_path);
        let data_source = prim.data_source?;

        // Traverse the locator path through the container
        crate::data_source::hd_container_get(data_source, locator)
    }

    /// Add an observer to receive change notifications.
    ///
    /// The observer will receive notices for prims added, removed, or
    /// dirtied after registration. It will NOT be sent notices for prims
    /// already in the scene; the caller is responsible for initial sync.
    ///
    /// This function is not thread-safe.
    fn add_observer(&self, observer: HdSceneIndexObserverHandle);

    /// Remove an observer.
    ///
    /// The observer will no longer receive notifications. It won't get
    /// any notices as a result of being detached.
    ///
    /// This function is not thread-safe.
    fn remove_observer(&self, observer: &HdSceneIndexObserverHandle);

    /// Send a system message to this and upstream scene indices.
    ///
    /// Port of C++ HdSceneIndexBase::SystemMessage: recurse through input
    /// scenes first (if filtering), then handle locally via _system_message.
    fn system_message(&self, message_type: &TfToken, args: Option<HdDataSourceBaseHandle>) {
        // Recurse into input scenes first (C++ dynamic_cast to HdFilteringSceneIndexBase)
        for input in self.get_input_scenes_for_system_message() {
            {
                let input_lock = input.read();
                input_lock.system_message(message_type, args.clone());
            }
        }
        self._system_message(message_type, args);
    }

    /// Return input scenes for SystemMessage recursion.
    ///
    /// Default: empty (non-filtering scene indices). Filtering scene indices
    /// override this to return their inputs, matching C++ dynamic_cast pattern.
    fn get_input_scenes_for_system_message(&self) -> Vec<HdSceneIndexHandle> {
        Vec::new()
    }

    /// Get the display name for this scene index.
    ///
    /// Used for debugging and UI purposes.
    fn get_display_name(&self) -> String {
        // Default: use type name
        std::any::type_name::<Self>().to_string()
    }

    /// Set the display name for this scene index.
    ///
    /// Default implementation is a no-op. Scene indices that need
    /// display name functionality should override this.
    fn set_display_name(&mut self, _name: String) {
        // Default: no-op
    }

    /// Add a tag to this scene index for categorization.
    ///
    /// Default implementation is a no-op. Scene indices that need
    /// tag functionality should override this.
    fn add_tag(&mut self, _tag: TfToken) {
        // Default: no-op
    }

    /// Remove a tag from this scene index.
    ///
    /// Default implementation is a no-op.
    fn remove_tag(&mut self, _tag: &TfToken) {
        // Default: no-op
    }

    /// Check if this scene index has a specific tag.
    ///
    /// Default implementation returns false.
    fn has_tag(&self, _tag: &TfToken) -> bool {
        false
    }

    /// Get all tags on this scene index.
    ///
    /// Default implementation returns empty vector.
    fn get_tags(&self) -> TfTokenVector {
        Vec::new()
    }

    /// Override to handle system messages.
    fn _system_message(&self, _message_type: &TfToken, _args: Option<HdDataSourceBaseHandle>) {
        // Default: no-op
    }
}

/// Strong reference to a scene index.
pub type HdSceneIndexHandle = Arc<SceneRwLock<dyn HdSceneIndexBase>>;

/// Weak reference to a scene index.
pub type HdSceneIndexWeakHandle = Weak<SceneRwLock<dyn HdSceneIndexBase>>;

/// Convert Arc<RwLock<SceneIndex>> to HdSceneIndexHandle.
///
/// Scene indices often return Arc<RwLock<Self>>; the pipeline needs HdSceneIndexHandle.
/// This uses a delegating wrapper to bridge the types.
pub fn scene_index_to_handle<T>(si: Arc<SceneRwLock<T>>) -> HdSceneIndexHandle
where
    T: HdSceneIndexBase + 'static,
{
    let wrapper = SceneIndexDelegate(si);
    Arc::new(SceneRwLock::new(wrapper))
}

/// Wrapper that delegates HdSceneIndexBase to an inner Arc<SceneRwLock<T>>.
pub struct SceneIndexDelegate<T>(pub Arc<SceneRwLock<T>>)
where
    T: HdSceneIndexBase;

impl<T: HdSceneIndexBase> HdSceneIndexBase for SceneIndexDelegate<T> {
    fn get_prim(&self, prim_path: &SdfPath) -> super::prim::HdSceneIndexPrim {
        // Use data_ptr() to avoid recursive read-lock deadlock in parking_lot.
        // SceneIndexDelegate is already behind its own RwLock; the inner lock is redundant
        // for read-only access since HdSceneIndexBase methods take &self.
        let inner = rwlock_data_ref(self.0.as_ref());
        inner.get_prim(prim_path)
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        let inner = rwlock_data_ref(self.0.as_ref());
        inner.get_child_prim_paths(prim_path)
    }

    fn add_observer(&self, observer: HdSceneIndexObserverHandle) {
        let inner = rwlock_data_ref(self.0.as_ref());
        inner.add_observer(observer);
    }

    fn remove_observer(&self, observer: &HdSceneIndexObserverHandle) {
        let inner = rwlock_data_ref(self.0.as_ref());
        inner.remove_observer(observer);
    }

    fn _system_message(&self, message_type: &TfToken, args: Option<HdDataSourceBaseHandle>) {
        let inner = rwlock_data_ref(self.0.as_ref());
        inner._system_message(message_type, args);
    }

    fn get_input_scenes_for_system_message(&self) -> Vec<HdSceneIndexHandle> {
        let inner = rwlock_data_ref(self.0.as_ref());
        inner.get_input_scenes_for_system_message()
    }

    fn get_display_name(&self) -> String {
        let inner = rwlock_data_ref(self.0.as_ref());
        inner.get_display_name()
    }
}

/// Convert `Arc<T>` (no outer RwLock) to `HdSceneIndexHandle`.
///
/// Used for scene indices that use interior mutability and return `Arc<Self>`
/// directly (e.g. `StageSceneIndex`). Wraps the `Arc<T>` in a thin delegating
/// `RwLock` so the rest of the pipeline can treat it as `HdSceneIndexHandle`.
pub fn arc_scene_index_to_handle<T>(si: Arc<T>) -> HdSceneIndexHandle
where
    T: HdSceneIndexBase + 'static,
{
    let wrapper = ArcSceneIndexDelegate(si);
    Arc::new(SceneRwLock::new(wrapper))
}

/// Wrapper that delegates `HdSceneIndexBase` to an inner `Arc<T>` (lock-free).
struct ArcSceneIndexDelegate<T: HdSceneIndexBase>(Arc<T>);

impl<T: HdSceneIndexBase> HdSceneIndexBase for ArcSceneIndexDelegate<T> {
    fn get_prim(&self, prim_path: &SdfPath) -> super::prim::HdSceneIndexPrim {
        self.0.get_prim(prim_path)
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        self.0.get_child_prim_paths(prim_path)
    }

    fn add_observer(&self, observer: HdSceneIndexObserverHandle) {
        self.0.add_observer(observer);
    }

    fn remove_observer(&self, observer: &HdSceneIndexObserverHandle) {
        self.0.remove_observer(observer);
    }

    fn _system_message(&self, message_type: &TfToken, args: Option<HdDataSourceBaseHandle>) {
        self.0._system_message(message_type, args);
    }

    fn get_input_scenes_for_system_message(&self) -> Vec<HdSceneIndexHandle> {
        self.0.get_input_scenes_for_system_message()
    }

    fn get_display_name(&self) -> String {
        self.0.get_display_name()
    }
}

/// Base implementation for scene index common functionality.
///
/// This struct provides default implementations for observer management,
/// notifications, and metadata. Concrete scene index types can embed this
/// to avoid reimplementing common functionality.
pub struct HdSceneIndexBaseImpl {
    state: std::sync::Mutex<HdSceneIndexBaseState>,
}

struct HdSceneIndexBaseState {
    /// Registered observers
    observers: Vec<HdSceneIndexObserverHandle>,
    /// Display name for debugging
    display_name: String,
    /// Tags for categorization
    tags: HashSet<TfToken>,
}

impl HdSceneIndexBaseImpl {
    /// Create a new scene index base implementation.
    pub fn new() -> Self {
        Self {
            state: std::sync::Mutex::new(HdSceneIndexBaseState {
                observers: Vec::new(),
                display_name: String::new(),
                tags: HashSet::new(),
            }),
        }
    }

    /// Add an observer.
    pub fn add_observer(&self, observer: HdSceneIndexObserverHandle) {
        let mut state = self.state.lock().expect("Lock poisoned");
        state.observers.push(observer);
    }

    /// Remove a specific observer.
    ///
    /// C++ parity: finds the observer by Arc pointer equality and removes it.
    /// In Rust, mid-notification removal can't happen (send methods hold
    /// &mut self), so we always remove immediately.
    pub fn remove_observer(&self, observer: &HdSceneIndexObserverHandle) {
        let target_ptr = Arc::as_ptr(observer) as *const ();
        let mut state = self.state.lock().expect("Lock poisoned");

        if let Some(pos) = state
            .observers
            .iter()
            .position(|arc| Arc::as_ptr(arc) as *const () == target_ptr)
        {
            state.observers.remove(pos);
        }
    }

    /// Check if there are any observers.
    pub fn is_observed(&self) -> bool {
        let state = self.state.lock().expect("Lock poisoned");
        !state.observers.is_empty()
    }

    /// Send prims added notification.
    pub fn send_prims_added(&self, sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        if entries.is_empty() || !self.is_observed() {
            return;
        }

        let observers = {
            let state = self.state.lock().expect("Lock poisoned");
            state.observers.clone()
        };

        for observer in &observers {
            observer.prims_added(sender, entries);
        }
    }

    /// Send prims removed notification.
    pub fn send_prims_removed(&self, sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        if entries.is_empty() || !self.is_observed() {
            return;
        }

        let observers = {
            let state = self.state.lock().expect("Lock poisoned");
            state.observers.clone()
        };

        for observer in &observers {
            observer.prims_removed(sender, entries);
        }
    }

    /// Send prims dirtied notification.
    pub fn send_prims_dirtied(&self, sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        if entries.is_empty() || !self.is_observed() {
            return;
        }

        if flo_debug_enabled() {
            let summary = summarize_dirtied_entries(entries);
            eprintln!(
                "[dirty-trace] stage=scene_index_base emitter={} observed=true total={} unique={} dup_paths={} dup_instances={} first={}",
                sender.get_display_name(),
                summary.total,
                summary.unique_paths,
                summary.duplicate_paths,
                summary.duplicate_instances,
                summary.first_path,
            );
        }

        let observers = {
            let state = self.state.lock().expect("Lock poisoned");
            state.observers.clone()
        };

        for observer in &observers {
            observer.prims_dirtied(sender, entries);
        }
    }

    /// Send prims renamed notification.
    pub fn send_prims_renamed(&self, sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        if entries.is_empty() || !self.is_observed() {
            return;
        }

        let observers = {
            let state = self.state.lock().expect("Lock poisoned");
            state.observers.clone()
        };

        for observer in &observers {
            observer.prims_renamed(sender, entries);
        }
    }

    /// Get display name.
    pub fn get_display_name(&self) -> String {
        let state = self.state.lock().expect("Lock poisoned");
        state.display_name.clone()
    }

    /// Set display name.
    pub fn set_display_name(&self, name: String) {
        let mut state = self.state.lock().expect("Lock poisoned");
        state.display_name = name;
    }

    /// Add a tag.
    pub fn add_tag(&self, tag: TfToken) {
        let mut state = self.state.lock().expect("Lock poisoned");
        state.tags.insert(tag);
    }

    /// Remove a tag.
    pub fn remove_tag(&self, tag: &TfToken) {
        let mut state = self.state.lock().expect("Lock poisoned");
        state.tags.remove(tag);
    }

    /// Check if has tag.
    pub fn has_tag(&self, tag: &TfToken) -> bool {
        let state = self.state.lock().expect("Lock poisoned");
        state.tags.contains(tag)
    }

    /// Get all tags.
    pub fn get_tags(&self) -> TfTokenVector {
        let state = self.state.lock().expect("Lock poisoned");
        state.tags.iter().cloned().collect()
    }
}

impl Default for HdSceneIndexBaseImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_impl_creation() {
        let base = HdSceneIndexBaseImpl::new();
        assert!(!base.is_observed());
        assert_eq!(base.get_display_name(), "");
        assert_eq!(base.get_tags().len(), 0);
    }

    #[test]
    fn test_base_impl_display_name() {
        let base = HdSceneIndexBaseImpl::new();
        base.set_display_name("TestScene".to_string());
        assert_eq!(base.get_display_name(), "TestScene");
    }

    #[test]
    fn test_base_impl_tags() {
        let base = HdSceneIndexBaseImpl::new();
        let tag1 = TfToken::new("tag1");
        let tag2 = TfToken::new("tag2");

        assert!(!base.has_tag(&tag1));

        base.add_tag(tag1.clone());
        assert!(base.has_tag(&tag1));
        assert!(!base.has_tag(&tag2));

        base.add_tag(tag2.clone());
        assert_eq!(base.get_tags().len(), 2);

        base.remove_tag(&tag1);
        assert!(!base.has_tag(&tag1));
        assert!(base.has_tag(&tag2));
    }
}
