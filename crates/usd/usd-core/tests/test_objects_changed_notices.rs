// Port of testUsdObjectsChangedNotices — detailed ObjectsChanged notice tests
// Reference: _ref/OpenUSD/pxr/usd/usd/testenv/testUsdNotices.py (ObjectsChanged section)

mod common;

use std::collections::HashMap;
use std::sync::Arc;
use usd_core::notice::{ChangeEntry, NamespaceEditsInfo, ObjectsChanged};
use usd_core::{InitialLoadSet, Stage};
use usd_sdf::Path;
use usd_tf::Token;

fn setup_stage() -> Arc<Stage> {
    common::setup();
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    stage.define_prim("/Foo", "Xform").expect("define /Foo");
    stage
        .define_prim("/Foo/Bar", "Mesh")
        .expect("define /Foo/Bar");
    stage.define_prim("/Baz", "Scope").expect("define /Baz");
    stage
}

// ============================================================================
// AffectedObject checks
// ============================================================================

#[test]
fn affected_object_resync() {
    // C++ ref: test_ObjectsChangedNotice — resync implies affected
    let stage = setup_stage();
    let path = Path::from_string("/Foo").expect("path");

    let mut resyncs = HashMap::new();
    resyncs.insert(path.clone(), vec![ChangeEntry::new()]);

    let notice = ObjectsChanged::new(
        Arc::downgrade(&stage),
        resyncs,
        HashMap::new(),
        HashMap::new(),
        NamespaceEditsInfo::default(),
    );

    // The resynced path should be affected
    assert!(notice.resynced_path(&path));
    // Non-resynced paths should not be affected
    let baz = Path::from_string("/Baz").expect("path");
    assert!(!notice.resynced_path(&baz));
}

#[test]
fn affected_object_info_change() {
    let stage = setup_stage();
    let path = Path::from_string("/Foo").expect("path");

    let mut info = HashMap::new();
    info.insert(
        path.clone(),
        vec![ChangeEntry::with_changed_fields(vec![Token::new(
            "comment",
        )])],
    );

    let notice = ObjectsChanged::new(
        Arc::downgrade(&stage),
        HashMap::new(),
        info,
        HashMap::new(),
        NamespaceEditsInfo::default(),
    );

    assert!(notice.changed_info_only_path(&path));
    assert!(!notice.resynced_path(&path));
}

// ============================================================================
// GetResyncedPaths / GetChangedInfoOnlyPaths
// ============================================================================

#[test]
fn get_resynced_paths() {
    let stage = setup_stage();

    let mut resyncs = HashMap::new();
    resyncs.insert(
        Path::from_string("/Foo").expect("p"),
        vec![ChangeEntry::new()],
    );
    resyncs.insert(
        Path::from_string("/Baz").expect("p"),
        vec![ChangeEntry::new()],
    );

    let notice = ObjectsChanged::new(
        Arc::downgrade(&stage),
        resyncs,
        HashMap::new(),
        HashMap::new(),
        NamespaceEditsInfo::default(),
    );

    let paths = notice.get_resynced_paths();
    assert_eq!(paths.len(), 2);
}

#[test]
fn get_changed_info_only_paths() {
    let stage = setup_stage();

    let mut info = HashMap::new();
    info.insert(
        Path::from_string("/Foo").expect("p"),
        vec![ChangeEntry::with_changed_fields(vec![Token::new(
            "typeName",
        )])],
    );

    let notice = ObjectsChanged::new(
        Arc::downgrade(&stage),
        HashMap::new(),
        info,
        HashMap::new(),
        NamespaceEditsInfo::default(),
    );

    let paths = notice.get_changed_info_only_paths();
    assert_eq!(paths.len(), 1);
}

// ============================================================================
// Resolved asset paths resynced
// ============================================================================

#[test]
fn resolved_asset_paths_resynced() {
    let stage = setup_stage();
    let path = Path::from_string("/Foo").expect("p");

    let mut asset = HashMap::new();
    asset.insert(path.clone(), vec![ChangeEntry::new()]);

    let notice = ObjectsChanged::new(
        Arc::downgrade(&stage),
        HashMap::new(),
        HashMap::new(),
        asset,
        NamespaceEditsInfo::default(),
    );

    assert!(notice.resolved_asset_paths_resynced_path(&path));
    let paths = notice.get_resolved_asset_paths_resynced_paths();
    assert_eq!(paths.len(), 1);
}

// ============================================================================
// Mixed resync + info changes
// ============================================================================

#[test]
fn mixed_resync_and_info() {
    let stage = setup_stage();

    let mut resyncs = HashMap::new();
    resyncs.insert(
        Path::from_string("/Foo").expect("p"),
        vec![ChangeEntry::new()],
    );

    let mut info = HashMap::new();
    info.insert(
        Path::from_string("/Baz").expect("p"),
        vec![ChangeEntry::with_changed_fields(vec![Token::new(
            "documentation",
        )])],
    );

    let notice = ObjectsChanged::new(
        Arc::downgrade(&stage),
        resyncs,
        info,
        HashMap::new(),
        NamespaceEditsInfo::default(),
    );

    let foo = Path::from_string("/Foo").expect("p");
    let baz = Path::from_string("/Baz").expect("p");

    assert!(notice.resynced_path(&foo));
    assert!(!notice.changed_info_only_path(&foo));
    assert!(!notice.resynced_path(&baz));
    assert!(notice.changed_info_only_path(&baz));
}

// ============================================================================
// Empty notice
// ============================================================================

#[test]
fn empty_notice_nothing_affected() {
    let stage = setup_stage();
    let notice = ObjectsChanged::new(
        Arc::downgrade(&stage),
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
        NamespaceEditsInfo::default(),
    );

    let path = Path::from_string("/Foo").expect("p");
    assert!(!notice.resynced_path(&path));
    assert!(!notice.changed_info_only_path(&path));
    assert!(!notice.resolved_asset_paths_resynced_path(&path));
    assert!(!notice.has_changed_fields_path(&path));
}
