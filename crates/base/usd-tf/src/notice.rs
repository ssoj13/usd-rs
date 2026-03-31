//! Notice/Notification system for event-driven communication.
//!
//! This module provides a pub-sub notification system where objects can:
//! - Register interest in specific notice types
//! - Register globally or for specific senders
//! - Send notices to all registered listeners
//! - Revoke registration using keys
//! - Attach probes to introspect notice send/delivery flow
//!
//! # Examples
//!
//! ```
//! use usd_tf::notice::{Notice, NoticeRegistry, ListenerKey};
//! use std::sync::Arc;
//!
//! // Define a custom notice type
//! #[derive(Clone)]
//! struct MyNotice {
//!     message: String,
//! }
//!
//! impl Notice for MyNotice {
//!     fn notice_type_name() -> &'static str { "MyNotice" }
//! }
//!
//! // Create a registry
//! let registry = NoticeRegistry::new();
//!
//! // Register a global listener
//! let received = Arc::new(std::sync::atomic::AtomicBool::new(false));
//! let received_clone = received.clone();
//!
//! let key = registry.register_global::<MyNotice, _>(move |notice| {
//!     received_clone.store(true, std::sync::atomic::Ordering::SeqCst);
//! });
//!
//! // Send a notice
//! registry.send(&MyNotice { message: "Hello".to_string() });
//!
//! assert!(received.load(std::sync::atomic::Ordering::SeqCst));
//!
//! // Revoke when done
//! registry.revoke(key);
//! ```

use crate::spin_mutex::SpinMutexData;
use crate::type_info::TfType;
use std::any::{Any, TypeId};
use std::cell::Cell;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

// Per-thread block count matching C++ TfNotice thread-local semantics.
thread_local! {
    static BLOCK_COUNT: Cell<u64> = const { Cell::new(0) };
}

/// Trait for notice types.
///
/// All notice types must implement this trait to be usable with the
/// notification system.
pub trait Notice: Any + Clone + Send + Sync {
    /// Returns the type name for this notice (for debugging/logging).
    fn notice_type_name() -> &'static str
    where
        Self: Sized;

    /// Returns the TypeId for this notice type.
    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }
}

/// A key representing a listener registration.
///
/// This key can be used to revoke the registration later.
#[derive(Clone)]
pub struct ListenerKey {
    id: u64,
    notice_type: TypeId,
    active: Arc<AtomicBool>,
}

impl ListenerKey {
    /// Returns true if this key refers to an active registration.
    pub fn is_valid(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }
}

impl std::fmt::Debug for ListenerKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ListenerKey")
            .field("id", &self.id)
            .field("active", &self.is_valid())
            .finish()
    }
}

/// Sender identifier for per-sender registration.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SenderId(u64);

impl SenderId {
    /// Creates a new unique sender ID.
    pub fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    /// Creates a sender ID from a raw value (for testing).
    pub fn from_raw(id: u64) -> Self {
        Self(id)
    }
}

impl Default for SenderId {
    fn default() -> Self {
        Self::new()
    }
}

// ==================== Probe system ====================

/// Probe interface for introspecting notice send and delivery.
///
/// Corresponds to C++ TfNotice::Probe. Implement this trait and register
/// via `insert_probe` to observe notice flow.
pub trait NoticeProbe: Send + Sync {
    /// Called just before a notice is sent to any listeners.
    fn begin_send(&self, notice_type: TypeId, notice_type_name: &str, sender: Option<SenderId>);

    /// Called after the notice has been delivered to all listeners.
    fn end_send(&self);

    /// Called just before a notice is delivered to a specific listener.
    fn begin_delivery(&self, notice_type: TypeId, listener_id: u64);

    /// Called after the notice has been processed by the listener.
    fn end_delivery(&self);
}

// ==================== Internal types ====================

/// Arc-wrapped callback for safe snapshot-based invocation.
type ArcCallback = Arc<dyn Fn(&dyn Any) + Send + Sync>;

/// A registered listener entry.
struct ListenerEntry {
    id: u64,
    /// Arc-wrapped so snapshots can hold a strong ref during dispatch.
    callback: ArcCallback,
    active: Arc<AtomicBool>,
    /// Sender ID for per-sender listeners (None for global).
    #[allow(dead_code)]
    sender_id: Option<SenderId>,
}

/// Snapshot of a callback for lock-free invocation.
struct CallbackSnapshot {
    id: u64,
    callback: ArcCallback,
    active: Arc<AtomicBool>,
}

/// Container for listeners of a specific notice type.
struct ListenerContainer {
    /// Global listeners (receive from any sender).
    global_listeners: Vec<ListenerEntry>,
    /// Per-sender listeners.
    per_sender: HashMap<SenderId, Vec<ListenerEntry>>,
}

impl ListenerContainer {
    fn new() -> Self {
        Self {
            global_listeners: Vec::new(),
            per_sender: HashMap::new(),
        }
    }
}

/// The notice registry manages listener registrations and notice delivery.
///
/// This is typically used as a singleton, but can be instantiated directly
/// for testing or isolation purposes.
pub struct NoticeRegistry {
    /// Map from notice TypeId to listener container.
    listeners: SpinMutexData<HashMap<TypeId, ListenerContainer>>,
    /// Next listener ID.
    next_id: AtomicU64,
    /// Registered probes for introspection.
    probes: SpinMutexData<Vec<Arc<dyn NoticeProbe>>>,
    /// Number of in-flight send operations (for revoke_and_wait barrier).
    in_flight: AtomicUsize,
}

impl Default for NoticeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl NoticeRegistry {
    /// Creates a new notice registry.
    pub fn new() -> Self {
        Self {
            listeners: SpinMutexData::new(HashMap::new()),
            next_id: AtomicU64::new(1),
            probes: SpinMutexData::new(Vec::new()),
            in_flight: AtomicUsize::new(0),
        }
    }

    /// Registers a global listener for notices of type N.
    ///
    /// The callback will be invoked for all notices of type N, regardless
    /// of sender.
    pub fn register_global<N, F>(&self, callback: F) -> ListenerKey
    where
        N: Notice,
        F: Fn(&N) + Send + Sync + 'static,
    {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let active = Arc::new(AtomicBool::new(true));
        let type_id = TypeId::of::<N>();

        // Wrap the typed callback in a type-erased Arc wrapper
        let arc_callback: ArcCallback = Arc::new(move |any: &dyn Any| {
            if let Some(notice) = any.downcast_ref::<N>() {
                callback(notice);
            }
        });

        let entry = ListenerEntry {
            id,
            callback: arc_callback,
            active: active.clone(),
            sender_id: None,
        };

        let mut listeners = self.listeners.lock();
        listeners
            .entry(type_id)
            .or_insert_with(ListenerContainer::new)
            .global_listeners
            .push(entry);

        ListenerKey {
            id,
            notice_type: type_id,
            active,
        }
    }

    /// Registers a per-sender listener for notices of type N.
    ///
    /// The callback will only be invoked for notices sent by the specified
    /// sender.
    pub fn register_for_sender<N, F>(&self, sender: SenderId, callback: F) -> ListenerKey
    where
        N: Notice,
        F: Fn(&N) + Send + Sync + 'static,
    {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let active = Arc::new(AtomicBool::new(true));
        let type_id = TypeId::of::<N>();

        let arc_callback: ArcCallback = Arc::new(move |any: &dyn Any| {
            if let Some(notice) = any.downcast_ref::<N>() {
                callback(notice);
            }
        });

        let entry = ListenerEntry {
            id,
            callback: arc_callback,
            active: active.clone(),
            sender_id: Some(sender),
        };

        let mut listeners = self.listeners.lock();
        let container = listeners
            .entry(type_id)
            .or_insert_with(ListenerContainer::new);

        container.per_sender.entry(sender).or_default().push(entry);

        ListenerKey {
            id,
            notice_type: type_id,
            active,
        }
    }

    /// Revokes a listener registration.
    ///
    /// After this call, the listener will no longer receive notices.
    /// Returns true if the key was valid and successfully revoked.
    pub fn revoke(&self, key: ListenerKey) -> bool {
        if !key.active.swap(false, Ordering::AcqRel) {
            return false;
        }

        let mut listeners = self.listeners.lock();
        if let Some(container) = listeners.get_mut(&key.notice_type) {
            container.global_listeners.retain(|e| e.id != key.id);

            for sender_list in container.per_sender.values_mut() {
                sender_list.retain(|e| e.id != key.id);
            }

            container.per_sender.retain(|_, v| !v.is_empty());
        }

        true
    }

    /// Revokes a listener and waits for any in-flight callbacks to complete.
    ///
    /// Matches C++ `TfNotice::RevokeAndWait(Key&)`. After return, no
    /// in-flight send operations reference this listener's callback.
    pub fn revoke_and_wait(&self, key: ListenerKey) -> bool {
        let result = self.revoke(key);
        if result {
            // Spin-wait until no send operations are in flight.
            // This ensures any callback that was captured in a snapshot
            // before the revoke has finished executing.
            while self.in_flight.load(Ordering::Acquire) > 0 {
                std::hint::spin_loop();
            }
        }
        result
    }

    /// Revokes multiple listener registrations.
    pub fn revoke_all(&self, keys: &mut Vec<ListenerKey>) {
        for key in keys.drain(..) {
            self.revoke(key);
        }
    }

    /// Revokes multiple listeners and waits for in-flight callbacks.
    ///
    /// Matches C++ `TfNotice::RevokeAndWait(Keys*)`.  
    pub fn revoke_all_and_wait(&self, keys: &mut Vec<ListenerKey>) {
        for key in keys.drain(..) {
            self.revoke_and_wait(key);
        }
    }

    /// Sends a notice globally (to all global listeners).
    ///
    /// Returns the number of listeners that received the notice.
    pub fn send<N: Notice>(&self, notice: &N) -> usize {
        self.send_impl(notice, None)
    }

    /// Sends a notice from a specific sender.
    ///
    /// Both global listeners and listeners registered for this sender
    /// will receive the notice.
    pub fn send_from<N: Notice>(&self, notice: &N, sender: SenderId) -> usize {
        self.send_impl(notice, Some(sender))
    }

    /// Internal send implementation.
    ///
    /// Snapshots active callbacks under the lock, then invokes them
    /// without holding the lock. This prevents deadlocks if a callback
    /// tries to register or revoke listeners.
    ///
    /// Mirrors C++ noticeRegistry.cpp delivery loop: after delivering to listeners
    /// registered for the exact notice type, walks up the ancestor chain so that
    /// base-type listeners also receive derived notices.
    fn send_impl<N: Notice>(&self, notice: &N, sender: Option<SenderId>) -> usize {
        // Check per-thread block count (matches C++ thread-local semantics)
        if BLOCK_COUNT.with(|c| c.get()) > 0 {
            return 0;
        }

        // Track in-flight sends for revoke_and_wait barrier
        self.in_flight.fetch_add(1, Ordering::AcqRel);

        let type_id = TypeId::of::<N>();

        // Snapshot probes
        let probes: Vec<Arc<dyn NoticeProbe>> = self.probes.lock().clone();

        // Notify probes: begin_send
        if !probes.is_empty() {
            let name = N::notice_type_name();
            for probe in &probes {
                probe.begin_send(type_id, name, sender);
            }
        }

        // Build the ordered list of type IDs to deliver to: the exact notice type
        // first, then each ancestor in C3 order (matching C++ hierarchy walk).
        // get_all_ancestor_types() includes self as index 0, so we use it directly.
        let ancestor_type_ids: Vec<TypeId> = {
            let tf = TfType::find_by_typeid(type_id);
            if tf.is_unknown() {
                // Type not registered in TfType — only deliver to exact type.
                vec![type_id]
            } else {
                tf.get_all_ancestor_types()
                    .into_iter()
                    .filter_map(|t| t.get_typeid())
                    .collect()
            }
        };

        // Snapshot active callbacks for all relevant types under a single lock
        // acquisition to get a consistent view.
        let snapshots: Vec<CallbackSnapshot> = {
            let listeners = self.listeners.lock();
            let mut snaps = Vec::new();

            for tid in &ancestor_type_ids {
                let Some(container) = listeners.get(tid) else {
                    continue;
                };

                // Global listeners for this type level
                for entry in &container.global_listeners {
                    if entry.active.load(Ordering::Acquire) {
                        snaps.push(CallbackSnapshot {
                            id: entry.id,
                            callback: entry.callback.clone(),
                            active: entry.active.clone(),
                        });
                    }
                }

                // Per-sender listeners for this type level
                if let Some(sid) = sender {
                    if let Some(sender_list) = container.per_sender.get(&sid) {
                        for entry in sender_list {
                            if entry.active.load(Ordering::Acquire) {
                                snaps.push(CallbackSnapshot {
                                    id: entry.id,
                                    callback: entry.callback.clone(),
                                    active: entry.active.clone(),
                                });
                            }
                        }
                    }
                }
            }

            snaps
        };
        // Lock released here

        if snapshots.is_empty() {
            for probe in &probes {
                probe.end_send();
            }
            self.in_flight.fetch_sub(1, Ordering::AcqRel);
            return 0;
        }

        // Invoke callbacks without holding the lock
        let mut count = 0;
        for snap in &snapshots {
            if snap.active.load(Ordering::Acquire) {
                for probe in &probes {
                    probe.begin_delivery(type_id, snap.id);
                }

                (snap.callback)(notice);
                count += 1;

                for probe in &probes {
                    probe.end_delivery();
                }
            }
        }

        for probe in &probes {
            probe.end_send();
        }

        self.in_flight.fetch_sub(1, Ordering::AcqRel);
        count
    }

    /// Registers a probe for introspecting notice flow.
    pub fn insert_probe(&self, probe: Arc<dyn NoticeProbe>) {
        self.probes.lock().push(probe);
    }

    /// Removes a previously registered probe (by Arc pointer equality).
    pub fn remove_probe(&self, probe: &Arc<dyn NoticeProbe>) {
        self.probes.lock().retain(|p| !Arc::ptr_eq(p, probe));
    }

    /// Blocks notice delivery on the current thread while the guard is held.
    ///
    /// Matches C++ TfNotice::Block which uses thread-local storage.
    pub fn block(&self) -> NoticeBlock<'_> {
        NoticeBlock::new(self)
    }

    /// Returns true if notice delivery is blocked on the current thread.
    pub fn is_blocked(&self) -> bool {
        BLOCK_COUNT.with(|c| c.get()) > 0
    }

    /// Returns the number of active listeners for a given notice type.
    pub fn listener_count<N: Notice>(&self) -> usize {
        let type_id = TypeId::of::<N>();
        let listeners = self.listeners.lock();
        let Some(container) = listeners.get(&type_id) else {
            return 0;
        };
        let global = container
            .global_listeners
            .iter()
            .filter(|e| e.active.load(Ordering::Acquire))
            .count();
        let per_sender: usize = container
            .per_sender
            .values()
            .flat_map(|v| v.iter())
            .filter(|e| e.active.load(Ordering::Acquire))
            .count();
        global + per_sender
    }
}

/// RAII guard that blocks notice delivery on the current thread while held.
pub struct NoticeBlock<'a> {
    /// Lifetime tie to the registry (blocking is thread-local, not registry-level).
    _registry: &'a NoticeRegistry,
}

impl<'a> NoticeBlock<'a> {
    fn new(registry: &'a NoticeRegistry) -> Self {
        BLOCK_COUNT.with(|c| c.set(c.get() + 1));
        Self {
            _registry: registry,
        }
    }
}

impl Drop for NoticeBlock<'_> {
    fn drop(&mut self) {
        BLOCK_COUNT.with(|c| c.set(c.get().saturating_sub(1)));
    }
}

// ==================== Global registry singleton ====================

/// Global notice registry singleton.
static GLOBAL_REGISTRY: std::sync::OnceLock<NoticeRegistry> = std::sync::OnceLock::new();

/// Returns the global notice registry.
pub fn global_registry() -> &'static NoticeRegistry {
    GLOBAL_REGISTRY.get_or_init(NoticeRegistry::new)
}

/// Registers a global listener on the global registry.
pub fn register_global<N, F>(callback: F) -> ListenerKey
where
    N: Notice,
    F: Fn(&N) + Send + Sync + 'static,
{
    global_registry().register_global(callback)
}

/// Registers a per-sender listener on the global registry.
pub fn register_for_sender<N, F>(sender: SenderId, callback: F) -> ListenerKey
where
    N: Notice,
    F: Fn(&N) + Send + Sync + 'static,
{
    global_registry().register_for_sender(sender, callback)
}

/// Revokes a listener on the global registry.
pub fn revoke(key: ListenerKey) -> bool {
    global_registry().revoke(key)
}

/// Revokes a listener and waits for in-flight callbacks on the global registry.
///
/// Matches C++ `TfNotice::RevokeAndWait(Key&)`.
pub fn revoke_and_wait(key: ListenerKey) -> bool {
    global_registry().revoke_and_wait(key)
}

/// Sends a notice globally on the global registry.
pub fn send<N: Notice>(notice: &N) -> usize {
    global_registry().send(notice)
}

/// Sends a notice from a sender on the global registry.
pub fn send_from<N: Notice>(notice: &N, sender: SenderId) -> usize {
    global_registry().send_from(notice, sender)
}

/// Registers a probe on the global registry.
pub fn insert_probe(probe: Arc<dyn NoticeProbe>) {
    global_registry().insert_probe(probe);
}

/// Removes a probe from the global registry.
pub fn remove_probe(probe: &Arc<dyn NoticeProbe>) {
    global_registry().remove_probe(probe);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[derive(Clone)]
    struct TestNotice {
        value: i32,
    }

    impl Notice for TestNotice {
        fn notice_type_name() -> &'static str {
            "TestNotice"
        }
    }

    #[derive(Clone)]
    #[allow(dead_code)]
    struct OtherNotice {
        message: String,
    }

    impl Notice for OtherNotice {
        fn notice_type_name() -> &'static str {
            "OtherNotice"
        }
    }

    #[test]
    fn test_global_registration() {
        let registry = NoticeRegistry::new();
        let received = Arc::new(AtomicUsize::new(0));
        let received_clone = received.clone();

        let key = registry.register_global::<TestNotice, _>(move |notice| {
            received_clone.fetch_add(notice.value as usize, Ordering::SeqCst);
        });

        assert!(key.is_valid());

        let count = registry.send(&TestNotice { value: 42 });
        assert_eq!(count, 1);
        assert_eq!(received.load(Ordering::SeqCst), 42);
    }

    #[test]
    fn test_multiple_listeners() {
        let registry = NoticeRegistry::new();
        let count = Arc::new(AtomicUsize::new(0));

        let count1 = count.clone();
        let _key1 = registry.register_global::<TestNotice, _>(move |_| {
            count1.fetch_add(1, Ordering::SeqCst);
        });

        let count2 = count.clone();
        let _key2 = registry.register_global::<TestNotice, _>(move |_| {
            count2.fetch_add(1, Ordering::SeqCst);
        });

        registry.send(&TestNotice { value: 1 });
        assert_eq!(count.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_revoke() {
        let registry = NoticeRegistry::new();
        let received = Arc::new(AtomicBool::new(false));
        let received_clone = received.clone();

        let key = registry.register_global::<TestNotice, _>(move |_| {
            received_clone.store(true, Ordering::SeqCst);
        });

        assert!(registry.revoke(key.clone()));
        assert!(!key.is_valid());

        registry.send(&TestNotice { value: 1 });
        assert!(!received.load(Ordering::SeqCst));
    }

    #[test]
    fn test_per_sender_registration() {
        let registry = NoticeRegistry::new();
        let sender1 = SenderId::new();
        let sender2 = SenderId::new();

        let received1 = Arc::new(AtomicUsize::new(0));
        let received1_clone = received1.clone();

        let _key = registry.register_for_sender::<TestNotice, _>(sender1, move |n| {
            received1_clone.fetch_add(n.value as usize, Ordering::SeqCst);
        });

        // Send from sender1 - should be received
        registry.send_from(&TestNotice { value: 10 }, sender1);
        assert_eq!(received1.load(Ordering::SeqCst), 10);

        // Send from sender2 - should NOT be received
        registry.send_from(&TestNotice { value: 20 }, sender2);
        assert_eq!(received1.load(Ordering::SeqCst), 10);

        // Send globally - should NOT be received by per-sender listener
        registry.send(&TestNotice { value: 30 });
        assert_eq!(received1.load(Ordering::SeqCst), 10);
    }

    #[test]
    fn test_global_and_per_sender() {
        let registry = NoticeRegistry::new();
        let sender = SenderId::new();

        let global_count = Arc::new(AtomicUsize::new(0));
        let sender_count = Arc::new(AtomicUsize::new(0));

        let gc = global_count.clone();
        let _global_key = registry.register_global::<TestNotice, _>(move |_| {
            gc.fetch_add(1, Ordering::SeqCst);
        });

        let sc = sender_count.clone();
        let _sender_key = registry.register_for_sender::<TestNotice, _>(sender, move |_| {
            sc.fetch_add(1, Ordering::SeqCst);
        });

        // Send from sender - both should receive
        registry.send_from(&TestNotice { value: 1 }, sender);
        assert_eq!(global_count.load(Ordering::SeqCst), 1);
        assert_eq!(sender_count.load(Ordering::SeqCst), 1);

        // Send globally - only global should receive
        registry.send(&TestNotice { value: 2 });
        assert_eq!(global_count.load(Ordering::SeqCst), 2);
        assert_eq!(sender_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_different_notice_types() {
        let registry = NoticeRegistry::new();

        let test_count = Arc::new(AtomicUsize::new(0));
        let other_count = Arc::new(AtomicUsize::new(0));

        let tc = test_count.clone();
        let _key1 = registry.register_global::<TestNotice, _>(move |_| {
            tc.fetch_add(1, Ordering::SeqCst);
        });

        let oc = other_count.clone();
        let _key2 = registry.register_global::<OtherNotice, _>(move |_| {
            oc.fetch_add(1, Ordering::SeqCst);
        });

        registry.send(&TestNotice { value: 1 });
        assert_eq!(test_count.load(Ordering::SeqCst), 1);
        assert_eq!(other_count.load(Ordering::SeqCst), 0);

        registry.send(&OtherNotice {
            message: "hi".into(),
        });
        assert_eq!(test_count.load(Ordering::SeqCst), 1);
        assert_eq!(other_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_block_notices() {
        let registry = NoticeRegistry::new();
        let received = Arc::new(AtomicBool::new(false));
        let received_clone = received.clone();

        let _key = registry.register_global::<TestNotice, _>(move |_| {
            received_clone.store(true, Ordering::SeqCst);
        });

        {
            let _block = registry.block();
            assert!(registry.is_blocked());
            registry.send(&TestNotice { value: 1 });
            assert!(!received.load(Ordering::SeqCst));
        }

        assert!(!registry.is_blocked());
        registry.send(&TestNotice { value: 1 });
        assert!(received.load(Ordering::SeqCst));
    }

    #[test]
    fn test_nested_blocks() {
        let registry = NoticeRegistry::new();
        let received = Arc::new(AtomicUsize::new(0));
        let received_clone = received.clone();

        let _key = registry.register_global::<TestNotice, _>(move |_| {
            received_clone.fetch_add(1, Ordering::SeqCst);
        });

        {
            let _block1 = registry.block();
            {
                let _block2 = registry.block();
                registry.send(&TestNotice { value: 1 });
            }
            // Still blocked
            registry.send(&TestNotice { value: 1 });
        }

        // Now unblocked
        registry.send(&TestNotice { value: 1 });
        assert_eq!(received.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_revoke_all() {
        let registry = NoticeRegistry::new();
        let count = Arc::new(AtomicUsize::new(0));

        let mut keys = Vec::new();
        for _ in 0..3 {
            let c = count.clone();
            keys.push(registry.register_global::<TestNotice, _>(move |_| {
                c.fetch_add(1, Ordering::SeqCst);
            }));
        }

        registry.send(&TestNotice { value: 1 });
        assert_eq!(count.load(Ordering::SeqCst), 3);

        registry.revoke_all(&mut keys);
        assert!(keys.is_empty());

        registry.send(&TestNotice { value: 1 });
        assert_eq!(count.load(Ordering::SeqCst), 3); // No change
    }

    #[test]
    fn test_sender_id() {
        let s1 = SenderId::new();
        let s2 = SenderId::new();
        assert_ne!(s1, s2);

        let s3 = SenderId::from_raw(42);
        let s4 = SenderId::from_raw(42);
        assert_eq!(s3, s4);
    }

    #[test]
    fn test_listener_key_debug() {
        let registry = NoticeRegistry::new();
        let key = registry.register_global::<TestNotice, _>(|_| {});
        let debug = format!("{:?}", key);
        assert!(debug.contains("ListenerKey"));
        assert!(debug.contains("active: true"));
    }

    // ==================== Probe tests ====================

    /// Test probe that records send/delivery events.
    struct TestProbe {
        send_count: AtomicUsize,
        delivery_count: AtomicUsize,
        end_send_count: AtomicUsize,
        end_delivery_count: AtomicUsize,
    }

    impl TestProbe {
        fn new() -> Self {
            Self {
                send_count: AtomicUsize::new(0),
                delivery_count: AtomicUsize::new(0),
                end_send_count: AtomicUsize::new(0),
                end_delivery_count: AtomicUsize::new(0),
            }
        }
    }

    impl NoticeProbe for TestProbe {
        fn begin_send(&self, _: TypeId, _: &str, _: Option<SenderId>) {
            self.send_count.fetch_add(1, Ordering::SeqCst);
        }

        fn end_send(&self) {
            self.end_send_count.fetch_add(1, Ordering::SeqCst);
        }

        fn begin_delivery(&self, _: TypeId, _: u64) {
            self.delivery_count.fetch_add(1, Ordering::SeqCst);
        }

        fn end_delivery(&self) {
            self.end_delivery_count.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn test_probe_send_events() {
        let registry = NoticeRegistry::new();
        let probe = Arc::new(TestProbe::new());
        registry.insert_probe(probe.clone());

        let _key = registry.register_global::<TestNotice, _>(|_| {});

        registry.send(&TestNotice { value: 1 });

        assert_eq!(probe.send_count.load(Ordering::SeqCst), 1);
        assert_eq!(probe.end_send_count.load(Ordering::SeqCst), 1);
        assert_eq!(probe.delivery_count.load(Ordering::SeqCst), 1);
        assert_eq!(probe.end_delivery_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_probe_multiple_deliveries() {
        let registry = NoticeRegistry::new();
        let probe = Arc::new(TestProbe::new());
        registry.insert_probe(probe.clone());

        let _k1 = registry.register_global::<TestNotice, _>(|_| {});
        let _k2 = registry.register_global::<TestNotice, _>(|_| {});
        let _k3 = registry.register_global::<TestNotice, _>(|_| {});

        registry.send(&TestNotice { value: 1 });

        assert_eq!(probe.send_count.load(Ordering::SeqCst), 1);
        assert_eq!(probe.end_send_count.load(Ordering::SeqCst), 1);
        assert_eq!(probe.delivery_count.load(Ordering::SeqCst), 3);
        assert_eq!(probe.end_delivery_count.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn test_probe_no_listeners() {
        let registry = NoticeRegistry::new();
        let probe = Arc::new(TestProbe::new());
        registry.insert_probe(probe.clone());

        registry.send(&TestNotice { value: 1 });

        assert_eq!(probe.send_count.load(Ordering::SeqCst), 1);
        assert_eq!(probe.end_send_count.load(Ordering::SeqCst), 1);
        assert_eq!(probe.delivery_count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn test_remove_probe() {
        let registry = NoticeRegistry::new();
        let probe = Arc::new(TestProbe::new());
        registry.insert_probe(probe.clone());

        let _key = registry.register_global::<TestNotice, _>(|_| {});
        registry.send(&TestNotice { value: 1 });
        assert_eq!(probe.send_count.load(Ordering::SeqCst), 1);

        let probe_dyn: Arc<dyn NoticeProbe> = probe.clone();
        registry.remove_probe(&probe_dyn);
        registry.send(&TestNotice { value: 2 });
        assert_eq!(probe.send_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_listener_count() {
        let registry = NoticeRegistry::new();
        assert_eq!(registry.listener_count::<TestNotice>(), 0);

        let k1 = registry.register_global::<TestNotice, _>(|_| {});
        assert_eq!(registry.listener_count::<TestNotice>(), 1);

        let sender = SenderId::new();
        let _k2 = registry.register_for_sender::<TestNotice, _>(sender, |_| {});
        assert_eq!(registry.listener_count::<TestNotice>(), 2);

        registry.revoke(k1);
        assert_eq!(registry.listener_count::<TestNotice>(), 1);
    }

    #[test]
    fn test_double_revoke() {
        let registry = NoticeRegistry::new();
        let key = registry.register_global::<TestNotice, _>(|_| {});
        assert!(key.is_valid());
        assert!(registry.revoke(key.clone()));
        assert!(!key.is_valid());
        assert!(!registry.revoke(key));
    }

    #[test]
    fn test_send_no_listeners() {
        let registry = NoticeRegistry::new();
        assert_eq!(registry.send(&TestNotice { value: 42 }), 0);
    }

    #[test]
    fn test_callback_revoke_during_send() {
        // A callback can revoke another listener without deadlock
        let registry = Arc::new(NoticeRegistry::new());
        let count = Arc::new(AtomicUsize::new(0));

        let c = count.clone();
        let _k1 = registry.register_global::<TestNotice, _>(move |_| {
            c.fetch_add(1, Ordering::SeqCst);
        });

        let reg2 = registry.clone();
        let key2_storage: Arc<SpinMutexData<Option<ListenerKey>>> =
            Arc::new(SpinMutexData::new(None));
        let key2_for_cb = key2_storage.clone();

        // Callback that revokes key2
        let _k_revoker = registry.register_global::<TestNotice, _>(move |_| {
            if let Some(k) = key2_for_cb.lock().take() {
                reg2.revoke(k);
            }
        });

        let c2 = count.clone();
        let key2 = registry.register_global::<TestNotice, _>(move |_| {
            c2.fetch_add(100, Ordering::SeqCst);
        });
        *key2_storage.lock() = Some(key2);

        // First send: all 3 fire (snapshot taken before revoke happens)
        registry.send(&TestNotice { value: 1 });
        // k1 adds 1, k_revoker revokes k2, k2 still in snapshot adds 100
        // But k2's active flag was set to false by revoke, so it may be skipped
        // depending on timing. The key point: no deadlock.
        let total = count.load(Ordering::SeqCst);
        assert!(total >= 1); // At least k1 fired

        // Second send: k2 is definitely revoked
        count.store(0, Ordering::SeqCst);
        registry.send(&TestNotice { value: 1 });
        assert_eq!(count.load(Ordering::SeqCst), 1); // Only k1
    }

    #[test]
    fn test_concurrent_send_recv() {
        use std::thread;

        let registry = Arc::new(NoticeRegistry::new());
        let total = Arc::new(AtomicUsize::new(0));

        let t = total.clone();
        let _key = registry.register_global::<TestNotice, _>(move |n| {
            t.fetch_add(n.value as usize, Ordering::SeqCst);
        });

        let mut handles = Vec::new();
        for _ in 0..4 {
            let reg = registry.clone();
            handles.push(thread::spawn(move || {
                for i in 0..100 {
                    reg.send(&TestNotice { value: i });
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        // 4 threads * sum(0..100) = 4 * 4950 = 19800
        assert_eq!(total.load(Ordering::SeqCst), 19800);
    }

    #[test]
    fn test_revoke_and_wait() {
        let registry = NoticeRegistry::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        let key = registry.register_global::<TestNotice, _>(move |_| {
            c.fetch_add(1, Ordering::SeqCst);
        });

        // Send once
        registry.send(&TestNotice { value: 1 });
        assert_eq!(count.load(Ordering::SeqCst), 1);

        // Revoke and wait -- after return, no in-flight sends reference the callback
        assert!(registry.revoke_and_wait(key));

        // Subsequent send should not fire
        registry.send(&TestNotice { value: 1 });
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_revoke_all_and_wait() {
        let registry = NoticeRegistry::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c1 = count.clone();
        let c2 = count.clone();
        let k1 = registry.register_global::<TestNotice, _>(move |_| {
            c1.fetch_add(1, Ordering::SeqCst);
        });
        let k2 = registry.register_global::<TestNotice, _>(move |_| {
            c2.fetch_add(10, Ordering::SeqCst);
        });

        registry.send(&TestNotice { value: 1 });
        assert_eq!(count.load(Ordering::SeqCst), 11);

        let mut keys = vec![k1, k2];
        registry.revoke_all_and_wait(&mut keys);

        count.store(0, Ordering::SeqCst);
        registry.send(&TestNotice { value: 1 });
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }
}
