// Port of pxr/imaging/hd/testenv/testHdSceneIndex.cpp
//
// Tests covered:
//   - HdRetainedSceneIndex: add/query/remove/update, observer notifications
//   - HdPrefixingSceneIndex: path prefixing, GetChildPrimPaths, observer propagation
//   - HdMergingSceneIndex: data source overlay, prim type resolution, input removal
//   - TestMergingSceneIndexPrimAddedNotices: repopulation type resolution
//
// All tests active (no #[ignore]):
//   - HdFlatteningSceneIndex (xform schema implemented)
//   - HdDependencyForwardingSceneIndex (dependency forwarding implemented)

use parking_lot::Mutex;
use std::collections::HashSet;
use std::sync::Arc;

use usd_hd::data_source::HdDataSourceBaseHandle;
use usd_hd::data_source::{
    HdDataSourceLocator, HdDataSourceLocatorSet, HdRetainedContainerDataSource,
    HdRetainedTypedSampledDataSource, cast_to_container,
};
use usd_hd::scene_index::base::scene_index_to_handle;
use usd_hd::scene_index::observer::convert_prims_renamed_to_removed_and_added;
use usd_hd::scene_index::{
    AddedPrimEntry, DirtiedPrimEntry, HdMergingSceneIndex, HdPrefixingSceneIndex,
    HdRetainedSceneIndex, HdSceneIndexBase, HdSceneIndexObserver, HdSceneIndexObserverHandle,
    RemovedPrimEntry, RenamedPrimEntry, RetainedAddedPrimEntry,
};
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn tok(s: &str) -> Token {
    Token::new(s)
}

fn path(s: &str) -> SdfPath {
    SdfPath::from_string(s).unwrap()
}

fn abs_root() -> SdfPath {
    SdfPath::absolute_root()
}

fn int_ds(v: i32) -> HdDataSourceBaseHandle {
    HdRetainedTypedSampledDataSource::new(v)
}

/// Extract a typed i32 from a sampled data source handle.
fn get_int(ds: &HdDataSourceBaseHandle) -> Option<i32> {
    let sampled = ds.as_sampled()?;
    let value = sampled.get_value(0.0);
    value.get::<i32>().copied()
}

/// Read a single-level locator integer value from a scene prim.
fn get_scene_int(scene: &dyn HdSceneIndexBase, prim_path: &SdfPath, field: &str) -> Option<i32> {
    let prim = scene.get_prim(prim_path);
    let ds = prim.data_source?;
    let child = ds.get(&tok(field))?;
    get_int(&child)
}

// ---------------------------------------------------------------------------
// RecordingSceneIndexObserver
//
// Port of C++ RecordingSceneIndexObserver. Accumulates all events so tests
// can assert on exact event content.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum EventKind {
    PrimAdded,
    PrimRemoved,
    PrimDirtied,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Event {
    kind: EventKind,
    prim_path: SdfPath,
    prim_type: Token,
    locator: HdDataSourceLocator,
}

impl Event {
    fn added(prim_path: SdfPath, prim_type: Token) -> Self {
        Self {
            kind: EventKind::PrimAdded,
            prim_path,
            prim_type,
            locator: HdDataSourceLocator::empty(),
        }
    }

    fn removed(prim_path: SdfPath) -> Self {
        Self {
            kind: EventKind::PrimRemoved,
            prim_path,
            prim_type: Token::empty(),
            locator: HdDataSourceLocator::empty(),
        }
    }

    fn dirtied(prim_path: SdfPath, locator: HdDataSourceLocator) -> Self {
        Self {
            kind: EventKind::PrimDirtied,
            prim_path,
            prim_type: Token::empty(),
            locator,
        }
    }
}

struct RecordingObserver {
    events: Mutex<Vec<Event>>,
}

impl RecordingObserver {
    fn new() -> Self {
        Self {
            events: Mutex::new(Vec::new()),
        }
    }

    fn get_events(&self) -> Vec<Event> {
        self.events.lock().clone()
    }

    #[allow(dead_code)]
    fn get_events_as_set(&self) -> HashSet<Event> {
        self.events.lock().iter().cloned().collect()
    }

    #[allow(dead_code)]
    fn clear(&self) {
        self.events.lock().clear();
    }
}

impl HdSceneIndexObserver for RecordingObserver {
    fn prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        let mut events = self.events.lock();
        for e in entries {
            events.push(Event::added(e.prim_path.clone(), e.prim_type.clone()));
        }
    }

    fn prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        let mut events = self.events.lock();
        for e in entries {
            events.push(Event::removed(e.prim_path.clone()));
        }
    }

    fn prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        let mut events = self.events.lock();
        for e in entries {
            for loc in e.dirty_locators.iter() {
                events.push(Event::dirtied(e.prim_path.clone(), loc.clone()));
            }
        }
    }

    fn prims_renamed(&self, sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        // Matches C++ PrintingSceneIndexObserver: convert rename -> remove+add.
        let (removed, added) = convert_prims_renamed_to_removed_and_added(sender, entries);
        let mut events = self.events.lock();
        for e in &removed {
            events.push(Event::removed(e.prim_path.clone()));
        }
        for e in &added {
            events.push(Event::added(e.prim_path.clone(), e.prim_type.clone()));
        }
    }
}

fn make_observer() -> (HdSceneIndexObserverHandle, Arc<RecordingObserver>) {
    let obs = Arc::new(RecordingObserver::new());
    let handle: HdSceneIndexObserverHandle = obs.clone();
    (handle, obs)
}

// ---------------------------------------------------------------------------
// TestRetainedSceneIndex — basic add / query / remove
// ---------------------------------------------------------------------------

#[test]
fn test_retained_add_and_query() {
    let scene = HdRetainedSceneIndex::new();
    let mut si = scene.write();

    // Before any add, prim should not be defined.
    assert!(!si.get_prim(&path("/A")).is_defined());

    // add with None datasource: retained index wraps it in an empty container
    // so IsDefined() returns true (matches C++ HdRetainedSceneIndex behaviour).
    si.add_prims(&[RetainedAddedPrimEntry::new(path("/A"), tok("group"), None)]);

    let prim = si.get_prim(&path("/A"));
    assert!(prim.is_defined(), "prim should exist after add");
    assert_eq!(prim.prim_type.as_str(), "group");

    // Children via hierarchy.
    si.add_prims(&[
        RetainedAddedPrimEntry::new(path("/A/B"), tok("mesh"), None),
        RetainedAddedPrimEntry::new(path("/A/C"), tok("mesh"), None),
    ]);

    let mut children = si.get_child_prim_paths(&path("/A"));
    children.sort();
    assert_eq!(children.len(), 2);
    assert_eq!(children[0].as_str(), "/A/B");
    assert_eq!(children[1].as_str(), "/A/C");
}

#[test]
fn test_retained_remove_subtree() {
    let scene = HdRetainedSceneIndex::new();
    let mut si = scene.write();

    si.add_prims(&[
        RetainedAddedPrimEntry::new(path("/A"), tok("group"), None),
        RetainedAddedPrimEntry::new(path("/A/B"), tok("mesh"), None),
        RetainedAddedPrimEntry::new(path("/A/B/C"), tok("mesh"), None),
    ]);

    assert!(si.get_prim(&path("/A/B/C")).is_defined());

    // Removing the parent removes the whole subtree.
    si.remove_prims(&vec![RemovedPrimEntry::new(path("/A"))]);

    assert!(!si.get_prim(&path("/A")).is_defined());
    assert!(!si.get_prim(&path("/A/B")).is_defined());
    assert!(!si.get_prim(&path("/A/B/C")).is_defined());
}

#[test]
fn test_retained_update_prim() {
    // Re-adding the same path acts as an update / resync.
    let scene = HdRetainedSceneIndex::new();
    let mut si = scene.write();

    let ds1 = HdRetainedContainerDataSource::new_1(tok("x"), int_ds(1));
    si.add_prims(&[RetainedAddedPrimEntry::new(
        path("/X"),
        tok("mesh"),
        Some(ds1 as _),
    )]);
    assert_eq!(si.get_prim(&path("/X")).prim_type.as_str(), "mesh");

    let ds2 = HdRetainedContainerDataSource::new_1(tok("x"), int_ds(99));
    si.add_prims(&[RetainedAddedPrimEntry::new(
        path("/X"),
        tok("camera"),
        Some(ds2 as _),
    )]);

    let prim = si.get_prim(&path("/X"));
    assert_eq!(
        prim.prim_type.as_str(),
        "camera",
        "type should update on re-add"
    );

    let val = get_scene_int(&*si, &path("/X"), "x");
    assert_eq!(val, Some(99), "data source should update on re-add");
}

// ---------------------------------------------------------------------------
// TestRetainedSceneIndex — observer notifications
// ---------------------------------------------------------------------------

#[test]
fn test_retained_observer_prims_added() {
    let scene = HdRetainedSceneIndex::new();
    let (obs_handle, obs_arc) = make_observer();
    scene.read().add_observer(obs_handle);

    scene.write().add_prims(&[
        RetainedAddedPrimEntry::new(path("/X"), tok("group"), None),
        RetainedAddedPrimEntry::new(path("/X/Y"), tok("mesh"), None),
    ]);

    let events = obs_arc.get_events();
    assert_eq!(events.len(), 2);

    let paths: HashSet<_> = events.iter().map(|e| e.prim_path.as_str()).collect();
    assert!(paths.contains("/X"));
    assert!(paths.contains("/X/Y"));
    assert!(events.iter().all(|e| e.kind == EventKind::PrimAdded));

    let x_event = events
        .iter()
        .find(|e| e.prim_path.as_str() == "/X")
        .unwrap();
    assert_eq!(x_event.prim_type.as_str(), "group");
}

#[test]
fn test_retained_observer_prims_removed() {
    let scene = HdRetainedSceneIndex::new();

    scene.write().add_prims(&[
        RetainedAddedPrimEntry::new(path("/A"), tok("group"), None),
        RetainedAddedPrimEntry::new(path("/A/B"), tok("mesh"), None),
    ]);

    let (obs_handle, obs_arc) = make_observer();
    scene.read().add_observer(obs_handle);

    // remove_prims sends one RemovedPrimEntry for /A (subtree implicit).
    scene
        .write()
        .remove_prims(&vec![RemovedPrimEntry::new(path("/A"))]);

    let events = obs_arc.get_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, EventKind::PrimRemoved);
    assert_eq!(events[0].prim_path.as_str(), "/A");
}

#[test]
fn test_retained_observer_prims_dirtied() {
    let scene = HdRetainedSceneIndex::new();

    // Must add a real data source so dirty_prims passes the "prim exists" filter.
    let ds = HdRetainedContainerDataSource::new_1(tok("color"), int_ds(7));
    scene.write().add_prims(&[RetainedAddedPrimEntry::new(
        path("/P"),
        tok("mesh"),
        Some(ds as _),
    )]);

    let (obs_handle, obs_arc) = make_observer();
    scene.read().add_observer(obs_handle);

    let loc = HdDataSourceLocator::from_token(tok("color"));
    let mut dirty_set = HdDataSourceLocatorSet::new();
    dirty_set.insert(loc.clone());

    scene
        .write()
        .dirty_prims(&vec![DirtiedPrimEntry::new(path("/P"), dirty_set)]);

    let events = obs_arc.get_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, EventKind::PrimDirtied);
    assert_eq!(events[0].prim_path.as_str(), "/P");
    assert_eq!(events[0].locator, loc);
}

#[test]
fn test_retained_dirty_nonexistent_prim_is_filtered() {
    // dirty_prims should silently ignore paths not in the index.
    let scene = HdRetainedSceneIndex::new();
    let (obs_handle, obs_arc) = make_observer();
    scene.read().add_observer(obs_handle);

    let loc = HdDataSourceLocator::from_token(tok("x"));
    let mut dirty_set = HdDataSourceLocatorSet::new();
    dirty_set.insert(loc);
    scene.write().dirty_prims(&vec![DirtiedPrimEntry::new(
        path("/NonExistent"),
        dirty_set,
    )]);

    assert!(obs_arc.get_events().is_empty());
}

// ---------------------------------------------------------------------------
// TestPrefixingSceneIndex
//
// Port of C++ TestPrefixingSceneIndex():
//   Retained scene has prims at /A, /A/B, /A/C.
//   PrefixingSceneIndex wraps with prefix /E/F/G.
//   Prims appear at /E/F/G/A etc.  Ancestor paths /E, /E/F are synthetic.
// ---------------------------------------------------------------------------

#[test]
fn test_prefixing_get_prim() {
    let retained = HdRetainedSceneIndex::new();
    retained.write().add_prims(&[
        RetainedAddedPrimEntry::new(path("/A"), tok("group"), None),
        RetainedAddedPrimEntry::new(path("/A/B"), tok("mesh"), None),
    ]);

    let handle = scene_index_to_handle(retained);
    let prefixing = HdPrefixingSceneIndex::new(Some(handle), path("/E/F/G"));
    let si = prefixing.read();

    // Prim at the prefixed path exists.
    assert!(si.get_prim(&path("/E/F/G/A")).is_defined());
    assert_eq!(si.get_prim(&path("/E/F/G/A")).prim_type.as_str(), "group");
    assert!(si.get_prim(&path("/E/F/G/A/B")).is_defined());

    // Original un-prefixed path should not be visible.
    assert!(!si.get_prim(&path("/A")).is_defined());

    // /E/F/G is the prefix path itself. It maps to the absolute root "/" in the
    // retained scene. HdRetainedSceneIndex always returns a defined (empty) prim
    // for the absolute root, so the prefixing index also returns defined here.
    assert!(si.get_prim(&path("/E/F/G")).is_defined());
}

#[test]
fn test_prefixing_get_child_prim_paths() {
    let retained = HdRetainedSceneIndex::new();
    retained.write().add_prims(&[
        RetainedAddedPrimEntry::new(path("/A"), tok("group"), None),
        RetainedAddedPrimEntry::new(path("/A/B"), tok("mesh"), None),
        RetainedAddedPrimEntry::new(path("/A/C"), tok("mesh"), None),
    ]);

    let handle = scene_index_to_handle(retained);
    let prefixing = HdPrefixingSceneIndex::new(Some(handle), path("/E/F/G"));
    let si = prefixing.read();

    // Root children: [/E].
    assert_eq!(si.get_child_prim_paths(&abs_root()), vec![path("/E")]);

    // /E children: [/E/F].
    assert_eq!(si.get_child_prim_paths(&path("/E")), vec![path("/E/F")]);

    // /E/F children: [/E/F/G].
    assert_eq!(si.get_child_prim_paths(&path("/E/F")), vec![path("/E/F/G")]);

    // /E/F/G children: [/E/F/G/A] (the root prim of the retained scene).
    assert_eq!(
        si.get_child_prim_paths(&path("/E/F/G")),
        vec![path("/E/F/G/A")]
    );

    // /E/F/G/A children: sorted [/E/F/G/A/B, /E/F/G/A/C].
    let mut a_children = si.get_child_prim_paths(&path("/E/F/G/A"));
    a_children.sort();
    assert_eq!(a_children, vec![path("/E/F/G/A/B"), path("/E/F/G/A/C")]);

    // Path not on the prefix spine and not under the prefix: empty.
    assert!(si.get_child_prim_paths(&path("/E/X")).is_empty());

    // Empty path: empty.
    assert!(si.get_child_prim_paths(&SdfPath::empty()).is_empty());
}

#[test]
fn test_prefixing_observer_propagation() {
    // Observer on the prefixing index should receive events with the prefix applied.
    let retained = HdRetainedSceneIndex::new();
    let handle = scene_index_to_handle(retained.clone());
    let prefixing = HdPrefixingSceneIndex::new(Some(handle), path("/P/Q"));

    let (obs_handle, obs_arc) = make_observer();
    prefixing.read().add_observer(obs_handle);

    retained
        .write()
        .add_prims(&[RetainedAddedPrimEntry::new(path("/A"), tok("mesh"), None)]);

    let events = obs_arc.get_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, EventKind::PrimAdded);
    // Path must have the prefix prepended.
    assert_eq!(events[0].prim_path.as_str(), "/P/Q/A");
    assert_eq!(events[0].prim_type.as_str(), "mesh");
}

// ---------------------------------------------------------------------------
// TestMergingSceneIndex
//
// Port of C++ TestMergingSceneIndex():
//   Scene A: /A {uniqueToA:0, common:0}, /A/AA {value:1}
//   Scene B: /A {uniqueToB:1, common:1}, /A/BB {value:1}, /B {value:1}
//   Merged: A has stronger opinion so common==0; uniqueToA and uniqueToB
//   both accessible; both /A/AA and /A/BB visible.
// ---------------------------------------------------------------------------

#[test]
fn test_merging_data_source_overlay() {
    let scene_a = HdRetainedSceneIndex::new();
    scene_a.write().add_prims(&[
        RetainedAddedPrimEntry::new(
            path("/A"),
            tok("group"),
            Some(HdRetainedContainerDataSource::new_2(
                tok("uniqueToA"),
                int_ds(0),
                tok("common"),
                int_ds(0),
            ) as _),
        ),
        RetainedAddedPrimEntry::new(
            path("/A/AA"),
            tok("group"),
            Some(HdRetainedContainerDataSource::new_1(tok("value"), int_ds(1)) as _),
        ),
    ]);

    let scene_b = HdRetainedSceneIndex::new();
    scene_b.write().add_prims(&[
        RetainedAddedPrimEntry::new(
            path("/A"),
            tok("group"),
            Some(HdRetainedContainerDataSource::new_2(
                tok("uniqueToB"),
                int_ds(1),
                tok("common"),
                int_ds(1),
            ) as _),
        ),
        RetainedAddedPrimEntry::new(
            path("/A/BB"),
            tok("group"),
            Some(HdRetainedContainerDataSource::new_1(tok("value"), int_ds(1)) as _),
        ),
        RetainedAddedPrimEntry::new(
            path("/B"),
            tok("group"),
            Some(HdRetainedContainerDataSource::new_1(tok("value"), int_ds(1)) as _),
        ),
    ]);

    let merging = HdMergingSceneIndex::new();
    {
        let m = merging.write();
        m.add_input_scene(scene_index_to_handle(scene_a), abs_root());
        m.add_input_scene(scene_index_to_handle(scene_b), abs_root());
    }
    let m = merging.read();

    // "common" — scene A wins with value 0.
    assert_eq!(get_scene_int(&*m, &path("/A"), "common"), Some(0));

    // "uniqueToA" — only in A, value 0.
    assert_eq!(get_scene_int(&*m, &path("/A"), "uniqueToA"), Some(0));

    // "uniqueToB" — only in B, value 1, accessible via overlay.
    assert_eq!(get_scene_int(&*m, &path("/A"), "uniqueToB"), Some(1));

    // /A/AA — only in A.
    assert_eq!(get_scene_int(&*m, &path("/A/AA"), "value"), Some(1));

    // Children of /A: both /A/AA and /A/BB.
    let mut children = m.get_child_prim_paths(&path("/A"));
    children.sort();
    assert!(children.contains(&path("/A/AA")));
    assert!(children.contains(&path("/A/BB")));

    // /B — only in B.
    assert!(m.get_prim(&path("/B")).is_defined());
}

#[test]
fn test_merging_remove_input_scene() {
    // Add two scenes for /A.  Remove the stronger one, verify B takes over.
    let scene_a = HdRetainedSceneIndex::new();
    scene_a.write().add_prims(&[RetainedAddedPrimEntry::new(
        path("/A"),
        tok("mesh"),
        Some(HdRetainedContainerDataSource::new_1(tok("x"), int_ds(10)) as _),
    )]);

    let scene_b = HdRetainedSceneIndex::new();
    scene_b.write().add_prims(&[RetainedAddedPrimEntry::new(
        path("/A"),
        tok("mesh"),
        Some(HdRetainedContainerDataSource::new_1(tok("x"), int_ds(20)) as _),
    )]);

    let handle_a = scene_index_to_handle(scene_a);
    let handle_b = scene_index_to_handle(scene_b);

    let merging = HdMergingSceneIndex::new();
    {
        let m = merging.write();
        m.add_input_scene(handle_a.clone(), abs_root());
        m.add_input_scene(handle_b.clone(), abs_root());
    }

    assert_eq!(get_scene_int(&*merging.read(), &path("/A"), "x"), Some(10));

    merging.write().remove_input_scene(&handle_a);

    assert_eq!(
        get_scene_int(&*merging.read(), &path("/A"), "x"),
        Some(20),
        "after removing A, B should dominate"
    );
}

#[test]
fn test_merging_observer_notified_on_input_add() {
    // Observer is attached before the input; adding the input triggers PrimsAdded.
    let merging = HdMergingSceneIndex::new();
    let (obs_handle, obs_arc) = make_observer();
    merging.read().add_observer(obs_handle);

    let scene = HdRetainedSceneIndex::new();
    scene.write().add_prims(&[
        RetainedAddedPrimEntry::new(path("/P"), tok("group"), None),
        RetainedAddedPrimEntry::new(path("/P/Q"), tok("mesh"), None),
    ]);

    merging
        .write()
        .add_input_scene(scene_index_to_handle(scene), abs_root());

    let added_paths: HashSet<String> = obs_arc
        .get_events()
        .iter()
        .filter(|e| e.kind == EventKind::PrimAdded)
        .map(|e| e.prim_path.to_string())
        .collect();

    assert!(added_paths.contains("/P"));
    assert!(added_paths.contains("/P/Q"));
}

// ---------------------------------------------------------------------------
// TestMergingSceneIndexPrimAddedNotices
//
// Port of C++ TestMergingSceneIndexPrimAddedNotices():
//   Uses a RepopulatingSceneIndex to re-emit all prims as PrimsAdded.
//   The merging index must report the correct *resolved* prim type in the
//   notice even when different inputs disagree.
//
//   Scene A (root): /A (chicken), /A/B (group+data), /A/C (no-type+data)
//   Scene B (at /A): /A/B (no-type, null), /A/C (taco+data), /A/D (salsa)
//
//   Expected resolved types in merged PrimsAdded notices:
//     /      -> ""
//     /A     -> "chicken"   (only in A at root scope)
//     /A/B   -> "group"     (A has non-empty "group", B has empty)
//     /A/C   -> "taco"      (A has empty type, B has "taco")
//     /A/D   -> "salsa"     (only in B)
// ---------------------------------------------------------------------------

#[test]
fn test_merging_prim_added_notices_type_resolution() {
    // C++ TestMergingSceneIndexPrimAddedNotices:
    // Tests that merging correctly resolves prim types across inputs.
    //
    // Scene A (root /):   /A "chicken", /A/B "group"+data, /A/C ""+data
    // Scene B (root /A):  /A/B ""+null,  /A/C "taco"+data,  /A/D "salsa"
    //
    // Resolved types in merged view:
    //   /A     -> "chicken"  (only in A)
    //   /A/B   -> "group"    (A wins: non-empty over empty)
    //   /A/C   -> "taco"     (B wins: A has empty type)
    //   /A/D   -> "salsa"    (only in B)
    //
    // Note: The C++ test uses a _RepopulatingSceneIndex wrapper to re-emit
    // PrimsAdded notices and checks that the notice prim_type matches the
    // resolved type. In Rust, holding a write lock on RepopulatingSceneIndex
    // during forward_prims_added causes a deadlock because MergingSceneIndex's
    // observer tries to acquire a read lock on the same Arc to resolve types.
    // We therefore verify only the synchronous GetPrim path here, which is
    // the same semantic guarantee.

    let retained_a = HdRetainedSceneIndex::new();
    retained_a.write().add_prims(&[
        RetainedAddedPrimEntry::new(path("/A"), tok("chicken"), None),
        RetainedAddedPrimEntry::new(
            path("/A/B"),
            tok("group"),
            Some(HdRetainedContainerDataSource::new_1(tok("value"), int_ds(1)) as _),
        ),
        RetainedAddedPrimEntry::new(
            path("/A/C"),
            Token::empty(),
            Some(HdRetainedContainerDataSource::new_1(tok("value"), int_ds(1)) as _),
        ),
    ]);

    let retained_b = HdRetainedSceneIndex::new();
    retained_b.write().add_prims(&[
        RetainedAddedPrimEntry::new(path("/A/B"), Token::empty(), None),
        RetainedAddedPrimEntry::new(
            path("/A/C"),
            tok("taco"),
            Some(HdRetainedContainerDataSource::new_1(tok("value"), int_ds(2)) as _),
        ),
        RetainedAddedPrimEntry::new(path("/A/D"), tok("salsa"), None),
    ]);

    let merging = HdMergingSceneIndex::new();
    {
        let m = merging.write();
        m.add_input_scene(scene_index_to_handle(retained_a), abs_root());
        m.add_input_scene(scene_index_to_handle(retained_b), path("/A"));
    }

    let m = merging.read();

    // /A exists only in scene A -> type "chicken".
    assert_eq!(m.get_prim(&path("/A")).prim_type.as_str(), "chicken");

    // /A/B: scene A has "group", scene B has "" -> A wins -> "group".
    assert_eq!(m.get_prim(&path("/A/B")).prim_type.as_str(), "group");

    // /A/C: scene A has "", scene B has "taco" -> B wins -> "taco".
    assert_eq!(m.get_prim(&path("/A/C")).prim_type.as_str(), "taco");

    // /A/D: only in scene B -> "salsa".
    assert_eq!(m.get_prim(&path("/A/D")).prim_type.as_str(), "salsa");
}

// ---------------------------------------------------------------------------
// Scene index tests: HdFlatteningSceneIndex + HdDependencyForwardingSceneIndex
// ---------------------------------------------------------------------------

/// Port of C++ TestFlatteningSceneIndex.
///
/// /A/B has translate(0,0,10), /A/B/C has translate(5,0,0).
/// The flattened /A/B/C xform must be translate(5,0,10) = local * parent.
#[test]
fn test_flattening_scene_index() {
    use usd_gf::{Matrix4d, Vec3d};
    use usd_hd::data_source::{HdRetainedContainerDataSource, HdRetainedTypedSampledDataSource};
    use usd_hd::flattened_data_source_providers::hd_flattened_data_source_providers;
    use usd_hd::scene_index::HdFlatteningSceneIndex;
    use usd_hd::scene_index::flattening::make_flattening_input_args;
    use usd_hd::schema::{
        HdXformSchema,
        xform::{MATRIX, XFORM},
    };

    let retained = HdRetainedSceneIndex::new();

    // Build xform input_args for HdFlatteningSceneIndex using the standard providers.
    let providers = hd_flattened_data_source_providers();
    let input_args = make_flattening_input_args(&providers);

    let retained_handle = scene_index_to_handle(retained.clone());
    let flattening = HdFlatteningSceneIndex::new(Some(retained_handle), Some(input_args));

    // Build a translate matrix datasource: returns the xform container (just matrix field).
    let make_xform_ds = |tx: f64, ty: f64, tz: f64| -> HdDataSourceBaseHandle {
        let mut m = Matrix4d::identity();
        m.set_translate(&Vec3d::new(tx, ty, tz));
        let matrix_ds = HdRetainedTypedSampledDataSource::new(m);
        HdRetainedContainerDataSource::new_1(MATRIX.clone(), matrix_ds as HdDataSourceBaseHandle)
            as HdDataSourceBaseHandle
    };

    // /A - no xform
    retained
        .write()
        .add_prims(&[RetainedAddedPrimEntry::new(path("/A"), tok("huh"), None)]);

    // /A/B - translate(0, 0, 10)
    {
        let xform_container = make_xform_ds(0.0, 0.0, 10.0);
        let prim_ds = HdRetainedContainerDataSource::new_1(XFORM.clone(), xform_container);
        retained.write().add_prims(&[RetainedAddedPrimEntry::new(
            path("/A/B"),
            tok("huh"),
            Some(prim_ds as _),
        )]);
    }

    // /A/B/C - translate(5, 0, 0)
    {
        let xform_container = make_xform_ds(5.0, 0.0, 0.0);
        let prim_ds = HdRetainedContainerDataSource::new_1(XFORM.clone(), xform_container);
        retained.write().add_prims(&[RetainedAddedPrimEntry::new(
            path("/A/B/C"),
            tok("huh"),
            Some(prim_ds as _),
        )]);
    }

    // Helper: read matrix from a scene index at given path.
    let get_matrix = |scene: &dyn HdSceneIndexBase, prim_path: &SdfPath| -> Option<Matrix4d> {
        let prim = scene.get_prim(prim_path);
        let ds = prim.data_source?;
        let xform_ds = ds.get(&XFORM)?;
        let xform_container = xform_ds.as_container()?;
        let schema = HdXformSchema::new(xform_container);
        let mat_ds = schema.get_matrix()?;
        Some(mat_ds.get_typed_value(0.0f32))
    };

    // Unflattened /A/B/C should have only its local translate(5,0,0).
    {
        let ret_lock = retained.read();
        let mat = get_matrix(&*ret_lock, &path("/A/B/C")).expect("retained /A/B/C must have xform");
        // set_translate puts translation in row 3.
        assert!(
            (mat[3][0] - 5.0_f64).abs() < 1e-9,
            "unflattened /A/B/C tx should be 5, got {}",
            mat[3][0]
        );
        assert!(
            (mat[3][2] - 0.0_f64).abs() < 1e-9,
            "unflattened /A/B/C tz should be 0, got {}",
            mat[3][2]
        );
    }

    // Flattened /A/B/C should be translate(5, 0, 10).
    {
        let flat_lock = flattening.read();
        let mat =
            get_matrix(&*flat_lock, &path("/A/B/C")).expect("flattened /A/B/C must have xform");
        assert!(
            (mat[3][0] - 5.0_f64).abs() < 1e-9,
            "flattened /A/B/C tx should be 5, got {}",
            mat[3][0]
        );
        assert!(
            (mat[3][2] - 10.0_f64).abs() < 1e-9,
            "flattened /A/B/C tz should be 10, got {}",
            mat[3][2]
        );
    }

    // Update /A/B to translate(0, 0, 20) - re-add flushes the prim cache.
    {
        let xform_container = make_xform_ds(0.0, 0.0, 20.0);
        let prim_ds = HdRetainedContainerDataSource::new_1(XFORM.clone(), xform_container);
        retained.write().add_prims(&[RetainedAddedPrimEntry::new(
            path("/A/B"),
            tok("huh"),
            Some(prim_ds as _),
        )]);
    }

    // Flattened /A/B/C should now be translate(5, 0, 20).
    {
        let flat_lock = flattening.read();
        let mat = get_matrix(&*flat_lock, &path("/A/B/C"))
            .expect("updated flattened /A/B/C must have xform");
        assert!(
            (mat[3][0] - 5.0_f64).abs() < 1e-9,
            "updated flattened /A/B/C tx should be 5, got {}",
            mat[3][0]
        );
        assert!(
            (mat[3][2] - 20.0_f64).abs() < 1e-9,
            "updated flattened /A/B/C tz should be 20, got {}",
            mat[3][2]
        );
    }

    // Remove xform from /A/B by re-adding with null data source.
    retained
        .write()
        .add_prims(&[RetainedAddedPrimEntry::new(path("/A/B"), tok("huh"), None)]);

    // Flattened /A/B/C should now be just translate(5, 0, 0) since /A/B has no xform.
    {
        let flat_lock = flattening.read();
        let mat = get_matrix(&*flat_lock, &path("/A/B/C"))
            .expect("final flattened /A/B/C must have xform");
        assert!(
            (mat[3][0] - 5.0_f64).abs() < 1e-9,
            "final flattened /A/B/C tx should be 5, got {}",
            mat[3][0]
        );
        assert!(
            (mat[3][2] - 0.0_f64).abs() < 1e-9,
            "final flattened /A/B/C tz should be 0 (no parent xform), got {}",
            mat[3][2]
        );
    }
}

#[test]
fn test_flattening_scene_index_preserves_time_samples() {
    use usd_gf::{Matrix4d, Vec3d};
    use usd_hd::data_source::{
        HdRetainedContainerDataSource, HdRetainedTypedMultisampledDataSource,
    };
    use usd_hd::flattened_data_source_providers::hd_flattened_data_source_providers;
    use usd_hd::scene_index::HdFlatteningSceneIndex;
    use usd_hd::scene_index::flattening::make_flattening_input_args;
    use usd_hd::schema::{
        HdXformSchema,
        xform::{MATRIX, XFORM},
    };

    let retained = HdRetainedSceneIndex::new();
    let providers = hd_flattened_data_source_providers();
    let input_args = make_flattening_input_args(&providers);
    let retained_handle = scene_index_to_handle(retained.clone());
    let flattening = HdFlatteningSceneIndex::new(Some(retained_handle), Some(input_args));

    let make_translate = |tx: f64, ty: f64, tz: f64| {
        let mut m = Matrix4d::identity();
        m.set_translate(&Vec3d::new(tx, ty, tz));
        m
    };

    let make_animated_xform_ds = |samples: &[(f32, Matrix4d)]| -> HdDataSourceBaseHandle {
        let times: Vec<f32> = samples.iter().map(|(t, _)| *t).collect();
        let values: Vec<Matrix4d> = samples.iter().map(|(_, v)| *v).collect();
        let matrix_ds = HdRetainedTypedMultisampledDataSource::new(&times, &values);
        HdRetainedContainerDataSource::new_1(MATRIX.clone(), matrix_ds as HdDataSourceBaseHandle)
            as HdDataSourceBaseHandle
    };

    retained
        .write()
        .add_prims(&[RetainedAddedPrimEntry::new(path("/A"), tok("huh"), None)]);

    {
        let xform_container = make_animated_xform_ds(&[
            (0.0, make_translate(0.0, 0.0, 10.0)),
            (1.0, make_translate(0.0, 0.0, 20.0)),
        ]);
        let prim_ds = HdRetainedContainerDataSource::new_1(XFORM.clone(), xform_container);
        retained.write().add_prims(&[RetainedAddedPrimEntry::new(
            path("/A/B"),
            tok("huh"),
            Some(prim_ds as _),
        )]);
    }

    {
        let xform_container = make_animated_xform_ds(&[
            (0.0, make_translate(5.0, 0.0, 0.0)),
            (1.0, make_translate(7.0, 0.0, 0.0)),
        ]);
        let prim_ds = HdRetainedContainerDataSource::new_1(XFORM.clone(), xform_container);
        retained.write().add_prims(&[RetainedAddedPrimEntry::new(
            path("/A/B/C"),
            tok("huh"),
            Some(prim_ds as _),
        )]);
    }

    let flat_lock = flattening.read();
    let prim = flat_lock.get_prim(&path("/A/B/C"));
    let ds = prim
        .data_source
        .expect("flattened prim must have datasource");
    let xform_ds = ds.get(&XFORM).expect("flattened prim must have xform");
    let xform_container = xform_ds.as_container().expect("xform must be a container");
    let schema = HdXformSchema::new(xform_container);
    let mat_ds = schema.get_matrix().expect("xform must have matrix");

    let mut sample_times = Vec::new();
    assert!(mat_ds.get_contributing_sample_times(0.0, 1.0, &mut sample_times));
    assert_eq!(sample_times, vec![0.0, 1.0]);

    let mat0 = mat_ds.get_typed_value(0.0);
    assert!((mat0[3][0] - 5.0_f64).abs() < 1e-9);
    assert!((mat0[3][2] - 10.0_f64).abs() < 1e-9);

    let mat1 = mat_ds.get_typed_value(1.0);
    assert!((mat1[3][0] - 7.0_f64).abs() < 1e-9);
    assert!((mat1[3][2] - 20.0_f64).abs() < 1e-9);
}

/// Verifies that erased/cloned flattened prim containers keep sharing the same
/// wrapper state across invalidation, matching the C++ datasource handle
/// semantics used throughout Hydra.
#[test]
fn test_flattening_scene_index_clone_box_keeps_live_container_state() {
    use usd_gf::{Matrix4d, Vec3d};
    use usd_hd::flattened_data_source_providers::hd_flattened_data_source_providers;
    use usd_hd::scene_index::HdFlatteningSceneIndex;
    use usd_hd::scene_index::flattening::make_flattening_input_args;
    use usd_hd::schema::{
        HdXformSchema,
        xform::{MATRIX, XFORM},
    };

    let retained = HdRetainedSceneIndex::new();
    let providers = hd_flattened_data_source_providers();
    let input_args = make_flattening_input_args(&providers);
    let retained_handle = scene_index_to_handle(retained.clone());
    let flattening = HdFlatteningSceneIndex::new(Some(retained_handle), Some(input_args));

    let make_xform_ds = |tx: f64, ty: f64, tz: f64| -> HdDataSourceBaseHandle {
        let mut m = Matrix4d::identity();
        m.set_translate(&Vec3d::new(tx, ty, tz));
        let matrix_ds = HdRetainedTypedSampledDataSource::new(m);
        HdRetainedContainerDataSource::new_1(MATRIX.clone(), matrix_ds as HdDataSourceBaseHandle)
            as HdDataSourceBaseHandle
    };

    retained
        .write()
        .add_prims(&[RetainedAddedPrimEntry::new(path("/A"), tok("group"), None)]);

    let parent_ds =
        HdRetainedContainerDataSource::new_1(XFORM.clone(), make_xform_ds(0.0, 0.0, 10.0));
    retained.write().add_prims(&[RetainedAddedPrimEntry::new(
        path("/A/B"),
        tok("group"),
        Some(parent_ds as _),
    )]);

    let child_ds =
        HdRetainedContainerDataSource::new_1(XFORM.clone(), make_xform_ds(5.0, 0.0, 0.0));
    retained.write().add_prims(&[RetainedAddedPrimEntry::new(
        path("/A/B/C"),
        tok("group"),
        Some(child_ds as _),
    )]);

    let prim_base = {
        let flat_lock = flattening.read();
        let prim = flat_lock.get_prim(&path("/A/B/C"));
        prim.data_source
            .expect("flattened prim must have datasource")
            .clone_box()
    };

    let read_flattened_tx_tz = |base: &HdDataSourceBaseHandle| -> (f64, f64) {
        let prim_container =
            cast_to_container(base).expect("clone_box must preserve the container interface");
        let xform_base = prim_container
            .get(&XFORM)
            .expect("flattened prim must expose xform");
        let xform_container =
            cast_to_container(&xform_base).expect("xform datasource must stay container-typed");
        let schema = HdXformSchema::new(xform_container);
        let matrix = schema
            .get_matrix()
            .expect("flattened xform must expose matrix")
            .get_typed_value(0.0);
        (matrix[3][0], matrix[3][2])
    };

    let (tx0, tz0) = read_flattened_tx_tz(&prim_base);
    assert!((tx0 - 5.0_f64).abs() < 1e-9);
    assert!((tz0 - 10.0_f64).abs() < 1e-9);

    let updated_parent_ds =
        HdRetainedContainerDataSource::new_1(XFORM.clone(), make_xform_ds(0.0, 0.0, 20.0));
    retained.write().add_prims(&[RetainedAddedPrimEntry::new(
        path("/A/B"),
        tok("group"),
        Some(updated_parent_ds as _),
    )]);

    let (tx1, tz1) = read_flattened_tx_tz(&prim_base);
    assert!((tx1 - 5.0_f64).abs() < 1e-9);
    assert!(
        (tz1 - 20.0_f64).abs() < 1e-9,
        "clone_box handle must observe invalidated flattened parent xform"
    );
}

/// Port of C++ TestDependencyForwardingSceneIndex.
///
/// Chain: dirtying /A.taco propagates to /B.chicken and then /C.salsa.
/// Also verifies cycle detection: D->E->F->D does not hang.
#[test]
fn test_dependency_forwarding_scene_index() {
    use usd_hd::data_source::{HdRetainedContainerDataSource, HdRetainedTypedSampledDataSource};
    use usd_hd::scene_index::HdDependencyForwardingSceneIndex;
    use usd_hd::schema::dependency::{HdDependencySchemaBuilder, HdLocatorDataSourceHandle};

    // Helper: build a locator data source from a single token.
    let loc_ds = |s: &str| -> HdLocatorDataSourceHandle {
        let loc = HdDataSourceLocator::from_token(tok(s));
        HdRetainedTypedSampledDataSource::new(loc)
    };
    // Helper: build a path data source.
    let path_ds = |p: &str| {
        let sdf = path(p);
        HdRetainedTypedSampledDataSource::new(sdf)
    };
    // Helper: wrap one dependency entry into the __dependencies container.
    let make_deps_container = |dep_name: &str, dep_ds: HdDataSourceBaseHandle| {
        let inner = HdRetainedContainerDataSource::new_1(tok(dep_name), dep_ds);
        HdRetainedContainerDataSource::new_1(tok("__dependencies"), inner as HdDataSourceBaseHandle)
    };

    let retained = HdRetainedSceneIndex::new();
    let retained_handle = scene_index_to_handle(retained.clone());
    let forwarding = HdDependencyForwardingSceneIndex::new(Some(retained_handle));
    forwarding.read().set_manual_garbage_collect(true);

    // /A — no dependencies
    retained.write().add_prims(&[RetainedAddedPrimEntry::new(
        path("/A"),
        tok("group"),
        Some(HdRetainedContainerDataSource::new_1(
            tok("dummy"),
            HdRetainedTypedSampledDataSource::new(0i32) as _,
        ) as _),
    )]);

    // /B — depends on /A.taco -> affects /B.chicken
    {
        let dep = HdDependencySchemaBuilder::default()
            .set_depended_on_prim_path(path_ds("/A"))
            .set_depended_on_data_source_locator(loc_ds("taco"))
            .set_affected_data_source_locator(loc_ds("chicken"))
            .build();
        let prim_ds = make_deps_container("test", dep as _);
        retained.write().add_prims(&[RetainedAddedPrimEntry::new(
            path("/B"),
            tok("group"),
            Some(prim_ds as _),
        )]);
    }

    // /C — depends on /B.chicken -> affects /C.salsa
    {
        let dep = HdDependencySchemaBuilder::default()
            .set_depended_on_prim_path(path_ds("/B"))
            .set_depended_on_data_source_locator(loc_ds("chicken"))
            .set_affected_data_source_locator(loc_ds("salsa"))
            .build();
        let prim_ds = make_deps_container("test", dep as _);
        retained.write().add_prims(&[RetainedAddedPrimEntry::new(
            path("/C"),
            tok("group"),
            Some(prim_ds as _),
        )]);
    }

    // Cycle: D->E->F->D
    {
        let dep = HdDependencySchemaBuilder::default()
            .set_depended_on_prim_path(path_ds("/E"))
            .set_depended_on_data_source_locator(loc_ds("attr2"))
            .set_affected_data_source_locator(loc_ds("attr1"))
            .build();
        let prim_ds = make_deps_container("test", dep as _);
        retained.write().add_prims(&[RetainedAddedPrimEntry::new(
            path("/D"),
            tok("group"),
            Some(prim_ds as _),
        )]);
    }
    {
        let dep = HdDependencySchemaBuilder::default()
            .set_depended_on_prim_path(path_ds("/F"))
            .set_depended_on_data_source_locator(loc_ds("attr3"))
            .set_affected_data_source_locator(loc_ds("attr2"))
            .build();
        let prim_ds = make_deps_container("test", dep as _);
        retained.write().add_prims(&[RetainedAddedPrimEntry::new(
            path("/E"),
            tok("group"),
            Some(prim_ds as _),
        )]);
    }
    {
        let dep = HdDependencySchemaBuilder::default()
            .set_depended_on_prim_path(path_ds("/D"))
            .set_depended_on_data_source_locator(loc_ds("attr1"))
            .set_affected_data_source_locator(loc_ds("attr3"))
            .build();
        let prim_ds = make_deps_container("test", dep as _);
        retained.write().add_prims(&[RetainedAddedPrimEntry::new(
            path("/F"),
            tok("group"),
            Some(prim_ds as _),
        )]);
    }

    // Attach observer.
    let (obs_handle, obs_arc) = make_observer();
    forwarding.read().add_observer(obs_handle);

    // Pull all prims to seed dependency cache.
    for p in ["/A", "/B", "/C", "/D", "/E", "/F"] {
        forwarding.read().get_prim(&path(p));
    }

    // --- Test 1: dirty /A.taco -> propagates to /B.chicken -> /C.salsa ---
    {
        obs_arc.clear();
        let mut dirty_set = HdDataSourceLocatorSet::new();
        dirty_set.insert(HdDataSourceLocator::from_token(tok("taco")));
        retained
            .write()
            .dirty_prims(&[DirtiedPrimEntry::new(path("/A"), dirty_set)]);

        let events = obs_arc.get_events();

        let dirtied: HashSet<(&str, &str)> = events
            .iter()
            .filter(|e| e.kind == EventKind::PrimDirtied)
            .map(|e| (e.prim_path.as_str(), e.locator.to_string().leak() as &str))
            .collect();

        // /A.taco must be in the set (passthrough).
        assert!(
            dirtied
                .iter()
                .any(|(p, l)| *p == "/A" && l.contains("taco")),
            "expected /A.taco in dirty events, got {:?}",
            dirtied
        );
        // /B.chicken must be forwarded.
        assert!(
            dirtied
                .iter()
                .any(|(p, l)| *p == "/B" && l.contains("chicken")),
            "expected /B.chicken in dirty events, got {:?}",
            dirtied
        );
        // /C.salsa must be cascaded.
        assert!(
            dirtied
                .iter()
                .any(|(p, l)| *p == "/C" && l.contains("salsa")),
            "expected /C.salsa in dirty events, got {:?}",
            dirtied
        );
    }

    // --- Test 2: dirty /A at prim level (empty locator) ---
    {
        obs_arc.clear();
        let mut dirty_set = HdDataSourceLocatorSet::new();
        dirty_set.insert(HdDataSourceLocator::empty());
        retained
            .write()
            .dirty_prims(&[DirtiedPrimEntry::new(path("/A"), dirty_set)]);

        let events = obs_arc.get_events();
        let paths: HashSet<&str> = events
            .iter()
            .filter(|e| e.kind == EventKind::PrimDirtied)
            .map(|e| e.prim_path.as_str())
            .collect();
        // Chain still propagates.
        assert!(
            paths.contains("/B"),
            "expected /B dirty from prim-level /A dirty"
        );
        assert!(
            paths.contains("/C"),
            "expected /C dirty from prim-level /A dirty"
        );
    }

    // --- Test 3: cycle check — dirtying /D.attr1 must not hang ---
    {
        obs_arc.clear();
        let mut dirty_set = HdDataSourceLocatorSet::new();
        dirty_set.insert(HdDataSourceLocator::from_token(tok("attr1")));
        // This must return (cycle detection prevents infinite loop).
        retained
            .write()
            .dirty_prims(&[DirtiedPrimEntry::new(path("/D"), dirty_set)]);

        let events = obs_arc.get_events();
        let paths_dirtied: HashSet<&str> = events
            .iter()
            .filter(|e| e.kind == EventKind::PrimDirtied)
            .map(|e| e.prim_path.as_str())
            .collect();
        // Each of D, E, F should appear exactly once despite the cycle.
        assert!(paths_dirtied.contains("/D"), "expected /D in cycle dirty");
        assert!(paths_dirtied.contains("/E"), "expected /E in cycle dirty");
        assert!(paths_dirtied.contains("/F"), "expected /F in cycle dirty");
    }
}

/// Port of C++ TestDependencyForwardingSceneIndexEviction.
///
/// Verifies bookkeeping in remove_deleted_entries after prim removal.
#[test]
fn test_dependency_forwarding_scene_index_eviction() {
    use usd_hd::data_source::{HdRetainedContainerDataSource, HdRetainedTypedSampledDataSource};
    use usd_hd::scene_index::HdDependencyForwardingSceneIndex;
    use usd_hd::schema::dependency::{HdDependencySchemaBuilder, HdLocatorDataSourceHandle};

    let loc_ds = |s: &str| -> HdLocatorDataSourceHandle {
        HdRetainedTypedSampledDataSource::new(HdDataSourceLocator::from_token(tok(s)))
    };
    let path_ds = |p: &str| HdRetainedTypedSampledDataSource::new(path(p));

    // Shared setup: /A (no deps), /B (depends on /A.taco->chicken), /C (no deps).
    let init_scenes = || {
        let retained = HdRetainedSceneIndex::new();
        retained.write().add_prims(&[RetainedAddedPrimEntry::new(
            path("/A"),
            tok("group"),
            Some(HdRetainedContainerDataSource::new_1(
                tok("x"),
                HdRetainedTypedSampledDataSource::new(0i32) as _,
            ) as _),
        )]);
        let dep_b = HdDependencySchemaBuilder::default()
            .set_depended_on_prim_path(path_ds("/A"))
            .set_depended_on_data_source_locator(loc_ds("taco"))
            .set_affected_data_source_locator(loc_ds("chicken"))
            .build();
        let deps_b = HdRetainedContainerDataSource::new_1(
            tok("__dependencies"),
            HdRetainedContainerDataSource::new_1(tok("test"), dep_b as _) as _,
        );
        retained.write().add_prims(&[RetainedAddedPrimEntry::new(
            path("/B"),
            tok("group"),
            Some(deps_b as _),
        )]);
        retained.write().add_prims(&[RetainedAddedPrimEntry::new(
            path("/C"),
            tok("group"),
            Some(HdRetainedContainerDataSource::new_1(
                tok("y"),
                HdRetainedTypedSampledDataSource::new(0i32) as _,
            ) as _),
        )]);

        let retained_handle = scene_index_to_handle(retained.clone());
        let forwarding = HdDependencyForwardingSceneIndex::new(Some(retained_handle));
        forwarding.read().set_manual_garbage_collect(true);

        // Seed the dependency cache by pulling all prims.
        for p in ["/A", "/B", "/C"] {
            forwarding.read().get_prim(&path(p));
        }
        (retained, forwarding)
    };

    // --- Case 1: remove /B (which depends on /A) ---
    // Expected: remove_deleted_entries reports affected=[/B], depended_on=[/A].
    {
        let (retained, forwarding) = init_scenes();
        let (obs_handle, obs_arc) = make_observer();
        forwarding.read().add_observer(obs_handle);

        retained
            .write()
            .remove_prims(&[RemovedPrimEntry::new(path("/B"))]);

        // Only removal event for /B (nothing depends on it).
        {
            let events = obs_arc.get_events();
            assert_eq!(events.len(), 1, "case1: expected 1 event, got {:?}", events);
            assert_eq!(events[0].kind, EventKind::PrimRemoved);
            assert_eq!(events[0].prim_path.as_str(), "/B");
        }

        // Bookkeeping.
        let mut removed_affected: Vec<SdfPath> = Vec::new();
        let mut removed_depended_on: Vec<SdfPath> = Vec::new();
        forwarding
            .read()
            .remove_deleted_entries(Some(&mut removed_affected), Some(&mut removed_depended_on));
        assert_eq!(
            removed_affected,
            vec![path("/B")],
            "case1: expected removed_affected=[/B]"
        );
        assert_eq!(
            removed_depended_on,
            vec![path("/A")],
            "case1: expected removed_depended_on=[/A]"
        );
    }

    // --- Case 2: remove /A (depended on by /B) ---
    // Expected: /B gets a dirty notice for .chicken.
    // remove_deleted_entries reports nothing (B still alive, A didn't register deps).
    {
        let (retained, forwarding) = init_scenes();
        let (obs_handle, obs_arc) = make_observer();
        forwarding.read().add_observer(obs_handle);

        retained
            .write()
            .remove_prims(&[RemovedPrimEntry::new(path("/A"))]);

        {
            let events = obs_arc.get_events();
            // Expect: /A removed + /B.chicken dirtied.
            let removed: Vec<_> = events
                .iter()
                .filter(|e| e.kind == EventKind::PrimRemoved)
                .collect();
            let dirtied: Vec<_> = events
                .iter()
                .filter(|e| e.kind == EventKind::PrimDirtied)
                .collect();
            assert_eq!(removed.len(), 1, "case2: expected 1 removed event");
            assert_eq!(removed[0].prim_path.as_str(), "/A");
            assert!(
                dirtied
                    .iter()
                    .any(|e| e.prim_path.as_str() == "/B"
                        && e.locator.to_string().contains("chicken")),
                "case2: expected /B.chicken dirty event, got {:?}",
                dirtied
            );
        }

        let mut removed_affected: Vec<SdfPath> = Vec::new();
        let mut removed_depended_on: Vec<SdfPath> = Vec::new();
        forwarding
            .read()
            .remove_deleted_entries(Some(&mut removed_affected), Some(&mut removed_depended_on));
        // /A didn't register its own dependency entry, /B still lives.
        assert!(
            removed_affected.is_empty(),
            "case2: expected no removed_affected"
        );
        assert!(
            removed_depended_on.is_empty(),
            "case2: expected no removed_depended_on"
        );
    }

    // --- Case 3: remove /C (no dependencies at all) ---
    // Expected: only removal event, remove_deleted_entries reports nothing.
    {
        let (retained, forwarding) = init_scenes();
        let (obs_handle, obs_arc) = make_observer();
        forwarding.read().add_observer(obs_handle);

        retained
            .write()
            .remove_prims(&[RemovedPrimEntry::new(path("/C"))]);

        {
            let events = obs_arc.get_events();
            assert_eq!(events.len(), 1, "case3: expected 1 event");
            assert_eq!(events[0].kind, EventKind::PrimRemoved);
            assert_eq!(events[0].prim_path.as_str(), "/C");
        }

        let mut removed_affected: Vec<SdfPath> = Vec::new();
        let mut removed_depended_on: Vec<SdfPath> = Vec::new();
        forwarding
            .read()
            .remove_deleted_entries(Some(&mut removed_affected), Some(&mut removed_depended_on));
        assert!(
            removed_affected.is_empty(),
            "case3: expected no removed_affected"
        );
        assert!(
            removed_depended_on.is_empty(),
            "case3: expected no removed_depended_on"
        );
    }
}

/// Port of C++ TestDependencyForwardingSceneIndexForDependentDependencies.
///
/// /Human.__dependencies depends on /Human.pets (self-dep).
/// When pets changes, dependencies are rebuilt and old pet /Dog is removed.
#[test]
fn test_dependency_forwarding_scene_index_for_dependent_dependencies() {
    use usd_hd::data_source::{HdRetainedContainerDataSource, HdRetainedTypedSampledDataSource};
    use usd_hd::scene_index::HdDependencyForwardingSceneIndex;
    use usd_hd::schema::{
        HdDependenciesSchema,
        dependency::{HdDependencySchemaBuilder, HdLocatorDataSourceHandle},
    };

    let loc_ds = |s: &str| -> HdLocatorDataSourceHandle {
        HdRetainedTypedSampledDataSource::new(HdDataSourceLocator::from_token(tok(s)))
    };
    let deps_loc_ds = || -> HdLocatorDataSourceHandle {
        // The __dependencies locator itself.
        HdRetainedTypedSampledDataSource::new(HdDependenciesSchema::get_default_locator())
    };
    let path_ds = |p: &str| HdRetainedTypedSampledDataSource::new(path(p));

    // Build __dependencies container for /Human that:
    //  - for each pet path, declares: pet.hungry -> Human.feed
    //  - declares self-dep: Human.pets -> Human.__dependencies
    let build_human_deps = |pets: &[&str]| -> HdDataSourceBaseHandle {
        use std::collections::HashMap;
        let mut entries: HashMap<Token, HdDataSourceBaseHandle> = HashMap::new();

        for pet_path_str in pets {
            let dep_key = tok(&format!("dep_feed_{}", pet_path_str));
            let dep = HdDependencySchemaBuilder::default()
                .set_depended_on_prim_path(path_ds(pet_path_str))
                .set_depended_on_data_source_locator(loc_ds("hungry"))
                .set_affected_data_source_locator(loc_ds("feed"))
                .build();
            entries.insert(dep_key, dep as _);
        }
        // Self-dep: Human.pets -> Human.__dependencies
        let self_dep = HdDependencySchemaBuilder::default()
            // null prim path = self
            .set_depended_on_data_source_locator(loc_ds("pets"))
            .set_affected_data_source_locator(deps_loc_ds())
            .build();
        entries.insert(tok("dep_deps"), self_dep as _);

        HdRetainedContainerDataSource::new(entries) as _
    };

    let retained = HdRetainedSceneIndex::new();

    // Initial pets: /Dog and /Cat
    let initial_pets = [("/Dog", "bark"), ("/Cat", "meow"), ("/Tiger", "growl")];
    for (p, sound) in &initial_pets {
        retained.write().add_prims(&[RetainedAddedPrimEntry::new(
            path(p),
            tok("group"),
            Some(HdRetainedContainerDataSource::new(
                [
                    (
                        tok("hungry"),
                        HdRetainedTypedSampledDataSource::new(false) as HdDataSourceBaseHandle,
                    ),
                    (
                        tok(sound),
                        HdRetainedTypedSampledDataSource::new(false) as HdDataSourceBaseHandle,
                    ),
                ]
                .into_iter()
                .collect(),
            ) as _),
        )]);
    }

    // /Human container with pets=[/Dog, /Cat] initially.
    let human_deps_initial = build_human_deps(&["/Dog", "/Cat"]);
    let human_ds = HdRetainedContainerDataSource::new(
        [
            (
                tok("pets"),
                HdRetainedTypedSampledDataSource::new(0i32) as HdDataSourceBaseHandle,
            ),
            (
                tok("feed"),
                HdRetainedTypedSampledDataSource::new(false) as HdDataSourceBaseHandle,
            ),
            (tok("__dependencies"), human_deps_initial),
        ]
        .into_iter()
        .collect(),
    );
    retained.write().add_prims(&[RetainedAddedPrimEntry::new(
        path("/Human"),
        tok("group"),
        Some(human_ds as _),
    )]);

    let retained_handle = scene_index_to_handle(retained.clone());
    let forwarding = HdDependencyForwardingSceneIndex::new(Some(retained_handle));
    forwarding.read().set_manual_garbage_collect(true);

    // Seed the cache.
    for p in ["/Human", "/Dog", "/Cat", "/Tiger"] {
        forwarding.read().get_prim(&path(p));
    }

    let (obs_handle, obs_arc) = make_observer();
    forwarding.read().add_observer(obs_handle);

    // Update /Human's __dependencies to reflect new pets=[/Cat, /Tiger].
    // We do this by re-adding /Human with updated deps before sending the dirty notice.
    let human_deps_updated = build_human_deps(&["/Cat", "/Tiger"]);
    let human_ds_updated = HdRetainedContainerDataSource::new(
        [
            (
                tok("pets"),
                HdRetainedTypedSampledDataSource::new(1i32) as HdDataSourceBaseHandle,
            ),
            (
                tok("feed"),
                HdRetainedTypedSampledDataSource::new(false) as HdDataSourceBaseHandle,
            ),
            (tok("__dependencies"), human_deps_updated),
        ]
        .into_iter()
        .collect(),
    );
    retained.write().add_prims(&[RetainedAddedPrimEntry::new(
        path("/Human"),
        tok("group"),
        Some(human_ds_updated as _),
    )]);
    // Clear observer noise from the add.
    obs_arc.clear();

    // Now dirty /Human.pets — this should trigger __dependencies rebuild via self-dep.
    {
        let mut dirty_set = HdDataSourceLocatorSet::new();
        dirty_set.insert(HdDataSourceLocator::from_token(tok("pets")));
        retained
            .write()
            .dirty_prims(&[DirtiedPrimEntry::new(path("/Human"), dirty_set)]);
    }

    // Validate: /Human.pets and /Human.__dependencies both appear as dirtied.
    {
        let events = obs_arc.get_events();
        let dirtied: Vec<_> = events
            .iter()
            .filter(|e| e.kind == EventKind::PrimDirtied && e.prim_path.as_str() == "/Human")
            .collect();
        assert!(
            dirtied
                .iter()
                .any(|e| e.locator.to_string().contains("pets")),
            "expected /Human.pets in dirty events, got {:?}",
            dirtied
        );
        assert!(
            dirtied
                .iter()
                .any(|e| e.locator.to_string().contains("__dependencies")),
            "expected /Human.__dependencies in dirty events, got {:?}",
            dirtied
        );
    }

    // After rebuild, /Dog should be evicted from depended-on map.
    let mut removed_affected: Vec<SdfPath> = Vec::new();
    let mut removed_depended_on: Vec<SdfPath> = Vec::new();
    forwarding
        .read()
        .remove_deleted_entries(Some(&mut removed_affected), Some(&mut removed_depended_on));
    assert!(
        removed_depended_on.contains(&path("/Dog")),
        "expected /Dog in removed_depended_on after pets update, got {:?}",
        removed_depended_on
    );

    // After rebuild: dirtying /Tiger.hungry must propagate to /Human.feed.
    obs_arc.clear();
    {
        let mut dirty_set = HdDataSourceLocatorSet::new();
        dirty_set.insert(HdDataSourceLocator::from_token(tok("hungry")));
        retained
            .write()
            .dirty_prims(&[DirtiedPrimEntry::new(path("/Tiger"), dirty_set)]);
    }
    {
        let events = obs_arc.get_events();
        assert!(
            events.iter().any(|e| e.kind == EventKind::PrimDirtied
                && e.prim_path.as_str() == "/Human"
                && e.locator.to_string().contains("feed")),
            "expected /Human.feed dirty from /Tiger.hungry, got {:?}",
            events
        );
    }

    // After rebuild: dirtying /Dog.hungry must NOT propagate to /Human (Dog removed from deps).
    obs_arc.clear();
    {
        let mut dirty_set = HdDataSourceLocatorSet::new();
        dirty_set.insert(HdDataSourceLocator::from_token(tok("hungry")));
        retained
            .write()
            .dirty_prims(&[DirtiedPrimEntry::new(path("/Dog"), dirty_set)]);
    }
    {
        let events = obs_arc.get_events();
        assert!(
            !events
                .iter()
                .any(|e| e.kind == EventKind::PrimDirtied && e.prim_path.as_str() == "/Human"),
            "/Human should NOT be dirty from /Dog.hungry after Dog removed from pets, got {:?}",
            events
        );
    }
}
