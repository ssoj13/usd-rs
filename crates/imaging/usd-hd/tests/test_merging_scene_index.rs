// Port of pxr/imaging/hd/testenv/testHdMergingSceneIndex.cpp

use parking_lot::{Mutex, RwLock};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

use usd_hd::scene_index::base::{rwlock_data_ref, scene_index_to_handle};
use usd_hd::scene_index::filtering::{
    FilteringObserverTarget, HdSingleInputFilteringSceneIndexBase,
};
use usd_hd::scene_index::observer::{DirtiedPrimEntry, RenamedPrimEntry};
use usd_hd::scene_index::{
    AddedPrimEntry, HdMergingSceneIndex, HdRetainedSceneIndex, HdSceneIndexBase,
    HdSceneIndexHandle, HdSceneIndexObserver, HdSceneIndexObserverHandle, HdSceneIndexPrim,
    RemovedPrimEntry, RetainedAddedPrimEntry, SdfPathVector,
};
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;

// ---------------------------------------------------------------------------
// LogEntry — mirrors C++ `using _LogEntry = std::tuple<std::string, SdfPath>`
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
struct LogEntry {
    what: String,
    path: SdfPath,
}

impl LogEntry {
    fn new(what: &str, path: &str) -> Self {
        Self {
            what: what.to_string(),
            path: SdfPath::from_string(path).expect("valid path"),
        }
    }
}

// ---------------------------------------------------------------------------
// Logger — mirrors C++ `_Logger : public HdSceneIndexObserver`
// ---------------------------------------------------------------------------

struct Logger {
    log: Mutex<Vec<LogEntry>>,
}

impl Logger {
    fn new() -> Self {
        Self {
            log: Mutex::new(Vec::new()),
        }
    }

    fn get_log(&self) -> Vec<LogEntry> {
        self.log.lock().clone()
    }
}

impl HdSceneIndexObserver for Logger {
    fn prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        let mut log = self.log.lock();
        for e in entries {
            log.push(LogEntry {
                what: "add".into(),
                path: e.prim_path.clone(),
            });
        }
    }

    fn prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        let mut log = self.log.lock();
        for e in entries {
            log.push(LogEntry {
                what: "remove".into(),
                path: e.prim_path.clone(),
            });
        }
    }

    fn prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        let mut log = self.log.lock();
        for e in entries {
            log.push(LogEntry {
                what: "dirty".into(),
                path: e.prim_path.clone(),
            });
        }
    }

    fn prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        let mut log = self.log.lock();
        for e in entries {
            log.push(LogEntry {
                what: "rename".into(),
                path: e.old_prim_path.clone(),
            });
        }
    }
}

// ---------------------------------------------------------------------------
// MySceneIndex — mirrors C++ `_MySceneIndex : HdSingleInputFilteringSceneIndexBase`
//
// A single-input filter that can be disabled. On disable it fires
// PrimsRemoved("/") so downstream observers see surviving prims from other
// merged inputs via PrimsAdded.
//
// Differences from C++:
//   - C++ uses inheritance; Rust composes HdSingleInputFilteringSceneIndexBase.
//   - The enabled flag is Arc<AtomicBool> shared with the caller so disable()
//     can flip it without holding a write lock. This is required to prevent a
//     deadlock: holding write on filter while the MergingObserver callback
//     re-acquires read on the same filter (to call get_prim inside
//     compose_prim_from_inputs). In C++ there are no per-object locks.
//   - Notification is fired via unsafe raw pointer after the write lock is
//     released, safe here because the test is single-threaded and Arc keeps
//     the allocation alive.
// ---------------------------------------------------------------------------

struct MySceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
    _input_observer: Option<HdSceneIndexObserverHandle>,
    enabled: Arc<AtomicBool>,
}

impl MySceneIndex {
    fn new(input_scene: HdSceneIndexHandle) -> (Arc<RwLock<Self>>, Arc<AtomicBool>) {
        let enabled = Arc::new(AtomicBool::new(true));
        let arc = Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(input_scene.clone())),
            _input_observer: None,
            enabled: enabled.clone(),
        }));

        // Register a forwarding observer on the input scene.
        let weak: Weak<RwLock<Self>> = Arc::downgrade(&arc);
        let obs: HdSceneIndexObserverHandle =
            Arc::new(MySceneIndexObserver { owner: weak });
        input_scene.read().add_observer(obs.clone());
        arc.write()._input_observer = Some(obs);

        (arc, enabled)
    }
}

impl FilteringObserverTarget for MySceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        if self.enabled.load(Ordering::SeqCst) {
            self.base.forward_prims_added(self, entries);
        }
    }
    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        if self.enabled.load(Ordering::SeqCst) {
            self.base.forward_prims_removed(self, entries);
        }
    }
    fn on_prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        if self.enabled.load(Ordering::SeqCst) {
            self.base.forward_prims_dirtied(self, entries);
        }
    }
    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        if self.enabled.load(Ordering::SeqCst) {
            self.base.forward_prims_renamed(self, entries);
        }
    }
}

impl HdSceneIndexBase for MySceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        if self.enabled.load(Ordering::SeqCst) {
            if let Some(input) = self.base.get_input_scene() {
                return input.read().get_prim(prim_path);
            }
        }
        HdSceneIndexPrim::empty()
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        if self.enabled.load(Ordering::SeqCst) {
            if let Some(input) = self.base.get_input_scene() {
                return input.read().get_child_prim_paths(prim_path);
            }
        }
        Vec::new()
    }

    fn add_observer(&self, observer: HdSceneIndexObserverHandle) {
        self.base.base().add_observer(observer);
    }
    fn remove_observer(&self, observer: &HdSceneIndexObserverHandle) {
        self.base.base().remove_observer(observer);
    }
}

/// Observer wired onto the input scene; routes all notices into MySceneIndex.
struct MySceneIndexObserver {
    owner: Weak<RwLock<MySceneIndex>>,
}

impl HdSceneIndexObserver for MySceneIndexObserver {
    fn prims_added(&self, sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        if let Some(arc) = self.owner.upgrade() {
            let owner = rwlock_data_ref(arc.as_ref());
            owner.on_prims_added(sender, entries);
        }
    }
    fn prims_removed(&self, sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        if let Some(arc) = self.owner.upgrade() {
            let owner = rwlock_data_ref(arc.as_ref());
            owner.on_prims_removed(sender, entries);
        }
    }
    fn prims_dirtied(&self, sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        if let Some(arc) = self.owner.upgrade() {
            let owner = rwlock_data_ref(arc.as_ref());
            owner.on_prims_dirtied(sender, entries);
        }
    }
    fn prims_renamed(&self, sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        if let Some(arc) = self.owner.upgrade() {
            let owner = rwlock_data_ref(arc.as_ref());
            owner.on_prims_renamed(sender, entries);
        }
    }
}

/// Minimal sender used by disable() for self-originated remove notices.
struct DisabledSender;
impl HdSceneIndexBase for DisabledSender {
    fn get_prim(&self, _: &SdfPath) -> HdSceneIndexPrim {
        HdSceneIndexPrim::empty()
    }
    fn get_child_prim_paths(&self, _: &SdfPath) -> SdfPathVector {
        Vec::new()
    }
    fn add_observer(&self, _: HdSceneIndexObserverHandle) {}
    fn remove_observer(&self, _: &HdSceneIndexObserverHandle) {}
}

/// Disable a MySceneIndex from outside the RwLock.
///
/// Mirrors C++ `_MySceneIndex::Disable()`. The enabled flag is flipped via
/// AtomicBool so get_prim() returns empty without acquiring any lock. The
/// write lock is then acquired briefly to grab a raw pointer to the base,
/// then dropped before send_prims_removed is called. This avoids the deadlock
/// that would occur if the write lock were held during the merging observer
/// callback, which re-reads the filter's input via read().
///
/// SAFETY: safe in this single-threaded test because Arc keeps the allocation
/// alive and no concurrent mutation occurs during the notification window.
fn disable(filter_arc: &Arc<RwLock<MySceneIndex>>) {
    filter_arc
        .read()
        .enabled
        .store(false, Ordering::SeqCst);

    let removed = vec![RemovedPrimEntry::new(SdfPath::absolute_root())];
    let sender = DisabledSender;

    let base_ptr: *mut HdSingleInputFilteringSceneIndexBase = {
        let mut guard = filter_arc.write();
        &mut guard.base as *mut _
    };
    // Write lock released — safe to fire notification now.
    #[allow(unsafe_code)]
    unsafe {
        (*base_ptr).base_mut().send_prims_removed(&sender, &removed);
    }
}

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn retained_to_handle(si: Arc<RwLock<HdRetainedSceneIndex>>) -> HdSceneIndexHandle {
    scene_index_to_handle(si)
}

// ---------------------------------------------------------------------------
// test_notices_after_remove
//
// Port of C++ _TestNoticesAfterRemove.
//
// Creates two retained scenes (A and B) sharing the same hierarchy but with
// different prim types. Wraps each in a MySceneIndex filter, merges both (A
// stronger). Disables filter A and verifies the downstream observer receives:
//   remove(/)
//   add(/)
//   add(/Parent)
//   add(/Parent/Child)
// i.e. the merging index synthesises PrimsAdded for the B-branch that
// survives after A's output disappears.
// ---------------------------------------------------------------------------

#[test]
fn test_notices_after_remove() {
    let si_a = HdRetainedSceneIndex::new();
    {
        let mut lock = si_a.write();
        lock.add_prims(&[
            RetainedAddedPrimEntry::new(
                SdfPath::from_string("/Parent").expect("path"),
                TfToken::new("A"),
                None,
            ),
            RetainedAddedPrimEntry::new(
                SdfPath::from_string("/Parent/Child").expect("path"),
                TfToken::new("A"),
                None,
            ),
        ]);
    }

    let si_b = HdRetainedSceneIndex::new();
    {
        let mut lock = si_b.write();
        lock.add_prims(&[
            RetainedAddedPrimEntry::new(
                SdfPath::from_string("/Parent").expect("path"),
                TfToken::new("B"),
                None,
            ),
            RetainedAddedPrimEntry::new(
                SdfPath::from_string("/Parent/Child").expect("path"),
                TfToken::new("B"),
                None,
            ),
        ]);
    }

    let (filter_a, _enabled_a) = MySceneIndex::new(retained_to_handle(si_a));
    let (filter_b, _enabled_b) = MySceneIndex::new(retained_to_handle(si_b));

    let merging = HdMergingSceneIndex::new();
    {
        let m = merging.write();
        let root = SdfPath::absolute_root();
        m.add_input_scene(scene_index_to_handle(filter_a.clone()), root.clone());
        m.add_input_scene(scene_index_to_handle(filter_b.clone()), root);
    }

    let logger: Arc<Logger> = Arc::new(Logger::new());
    merging
        .read()
        .add_observer(logger.clone() as HdSceneIndexObserverHandle);

    disable(&filter_a);

    let got = logger.get_log();
    let expected = vec![
        LogEntry::new("remove", "/"),
        LogEntry::new("add", "/"),
        LogEntry::new("add", "/Parent"),
        LogEntry::new("add", "/Parent/Child"),
    ];

    assert_eq!(
        got, expected,
        "test_notices_after_remove: log mismatch.\n  got:      {:?}\n  expected: {:?}",
        got, expected,
    );
}

// ---------------------------------------------------------------------------
// test_remove_input_scenes
//
// Port of C++ _TestRemoveInputScenes.
//
// Creates three retained scenes with distinct path hierarchies, merges them
// at different roots, then removes two simultaneously. Verifies:
//   remove(/A/B/C/D)   — was unique to siB
//   remove(/A/B/C/D2)  — was unique to siA
//   add(/A/B)          — implicit ancestor in siC (via SdfPathTable parity)
//   add(/A/B/C)        — explicit prim in siC
//
// Ordering within the remove and add groups is non-deterministic (HashMap
// vs C++ SdfPathTable preorder), so each group is sorted independently.
// The structural invariant (all removes before all adds) is checked separately.
// ---------------------------------------------------------------------------

#[test]
fn test_remove_input_scenes() {
    let prim_type = TfToken::new("PrimType");

    // siA at /A/B: contributes /A/B/C/D/E/F and /A/B/C/D2
    let si_a = HdRetainedSceneIndex::new();
    {
        let mut lock = si_a.write();
        lock.add_prims(&[
            RetainedAddedPrimEntry::new(
                SdfPath::from_string("/A/B/C/D/E/F").expect("path"),
                prim_type.clone(),
                None,
            ),
            RetainedAddedPrimEntry::new(
                SdfPath::from_string("/A/B/C/D2").expect("path"),
                prim_type.clone(),
                None,
            ),
        ]);
    }

    // siB at /A/B/C: contributes /A/B/C/D
    let si_b = HdRetainedSceneIndex::new();
    {
        let mut lock = si_b.write();
        lock.add_prims(&[RetainedAddedPrimEntry::new(
            SdfPath::from_string("/A/B/C/D").expect("path"),
            prim_type.clone(),
            None,
        )]);
    }

    // siC at /: contributes /A/B/C (and implicit ancestors /A/B, /A)
    let si_c = HdRetainedSceneIndex::new();
    {
        let mut lock = si_c.write();
        lock.add_prims(&[RetainedAddedPrimEntry::new(
            SdfPath::from_string("/A/B/C").expect("path"),
            prim_type.clone(),
            None,
        )]);
    }

    let handle_a: HdSceneIndexHandle = retained_to_handle(si_a);
    let handle_b: HdSceneIndexHandle = retained_to_handle(si_b);
    let handle_c: HdSceneIndexHandle = retained_to_handle(si_c);

    let merging = HdMergingSceneIndex::new();
    {
        let m = merging.write();
        m.add_input_scene(
            handle_a.clone(),
            SdfPath::from_string("/A/B").expect("path"),
        );
        m.add_input_scene(
            handle_b.clone(),
            SdfPath::from_string("/A/B/C").expect("path"),
        );
        m.add_input_scene(handle_c.clone(), SdfPath::absolute_root());
    }

    let logger: Arc<Logger> = Arc::new(Logger::new());
    merging
        .read()
        .add_observer(logger.clone() as HdSceneIndexObserverHandle);

    merging
        .write()
        .remove_input_scenes(&[handle_a, handle_b]);

    let got = logger.get_log();

    // Partition into removes and adds, sort each group for determinism.
    let (mut removes, mut adds): (Vec<_>, Vec<_>) = got.iter().partition(|e| e.what == "remove");
    removes.sort_by(|a, b| a.path.as_str().cmp(b.path.as_str()));
    adds.sort_by(|a, b| a.path.as_str().cmp(b.path.as_str()));

    let removes: Vec<LogEntry> = removes.into_iter().cloned().collect();
    let adds: Vec<LogEntry> = adds.into_iter().cloned().collect();

    let expected_removes = [
        LogEntry::new("remove", "/A/B/C/D"),
        LogEntry::new("remove", "/A/B/C/D2"),
    ];
    let expected_adds = [LogEntry::new("add", "/A/B"), LogEntry::new("add", "/A/B/C")];

    assert_eq!(
        removes, &expected_removes,
        "removed entries mismatch.\n  got: {:?}\n  expected: {:?}",
        removes, &expected_removes,
    );
    assert_eq!(
        adds, &expected_adds,
        "added entries mismatch.\n  got: {:?}\n  expected: {:?}",
        adds, &expected_adds,
    );

    // All removes must precede all adds (C++ _SendPrimsRemoved then _SendPrimsAdded).
    let all_removes_before_adds = got
        .windows(2)
        .all(|w| !(w[0].what == "add" && w[1].what == "remove"));
    assert!(
        all_removes_before_adds,
        "removes must precede adds; got: {:?}",
        got
    );
}
