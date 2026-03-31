// Port of testUsdNotices.py — notice construction and query subset
// Reference: _ref/OpenUSD/pxr/usd/usd/testenv/testUsdNotices.py

mod common;

use std::collections::HashMap;
use std::sync::Arc;
use usd_core::Stage;
use usd_core::common::InitialLoadSet;
use usd_core::notice::{
    ChangeEntry, NamespaceEditsInfo, ObjectsChanged, PrimResyncInfo, PrimResyncType,
    StageContentsChanged, StageNotice,
};
use usd_sdf::Path;
use usd_tf::Token;

fn setup_stage() -> Arc<Stage> {
    common::setup();
    Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage")
}

// ============================================================================
// StageNotice
// ============================================================================

#[test]
fn stage_notice_construction() {
    let stage = setup_stage();
    let notice = StageNotice::new(Arc::downgrade(&stage));
    assert!(notice.stage().is_some());
    assert!(Arc::ptr_eq(&notice.stage().unwrap(), &stage));
}

#[test]
fn stage_notice_expired_stage() {
    let stage = setup_stage();
    let weak = Arc::downgrade(&stage);
    drop(stage);
    let notice = StageNotice::new(weak);
    assert!(notice.stage().is_none());
}

// ============================================================================
// StageContentsChanged
// ============================================================================

#[test]
fn stage_contents_changed() {
    let stage = setup_stage();
    let notice = StageContentsChanged::new(Arc::downgrade(&stage));
    assert!(notice.stage().is_some());
}

// ============================================================================
// PrimResyncType
// ============================================================================

#[test]
fn prim_resync_type_rename() {
    assert!(PrimResyncType::RenameSource.is_rename());
    assert!(PrimResyncType::RenameDestination.is_rename());
    assert!(PrimResyncType::RenameAndReparentSource.is_rename());
    assert!(PrimResyncType::RenameAndReparentDestination.is_rename());
    assert!(!PrimResyncType::ReparentSource.is_rename());
    assert!(!PrimResyncType::Delete.is_rename());
}

#[test]
fn prim_resync_type_reparent() {
    assert!(PrimResyncType::ReparentSource.is_reparent());
    assert!(PrimResyncType::ReparentDestination.is_reparent());
    assert!(PrimResyncType::RenameAndReparentSource.is_reparent());
    assert!(PrimResyncType::RenameAndReparentDestination.is_reparent());
    assert!(!PrimResyncType::RenameSource.is_reparent());
    assert!(!PrimResyncType::Delete.is_reparent());
}

#[test]
fn prim_resync_type_source_dest() {
    assert!(PrimResyncType::RenameSource.is_source());
    assert!(PrimResyncType::ReparentSource.is_source());
    assert!(PrimResyncType::RenameAndReparentSource.is_source());
    assert!(!PrimResyncType::RenameSource.is_destination());

    assert!(PrimResyncType::RenameDestination.is_destination());
    assert!(PrimResyncType::ReparentDestination.is_destination());
    assert!(PrimResyncType::RenameAndReparentDestination.is_destination());
    assert!(!PrimResyncType::RenameDestination.is_source());
}

#[test]
fn prim_resync_type_delete() {
    assert!(!PrimResyncType::Delete.is_rename());
    assert!(!PrimResyncType::Delete.is_reparent());
    assert!(!PrimResyncType::Delete.is_source());
    assert!(!PrimResyncType::Delete.is_destination());
}

// ============================================================================
// ChangeEntry
// ============================================================================

#[test]
fn change_entry_new() {
    let entry = ChangeEntry::new();
    assert!(entry.changed_fields.is_empty());
}

#[test]
fn change_entry_with_fields() {
    let entry =
        ChangeEntry::with_changed_fields(vec![Token::new("typeName"), Token::new("documentation")]);
    assert_eq!(entry.changed_fields.len(), 2);
    assert_eq!(entry.changed_fields[0].as_str(), "typeName");
    assert_eq!(entry.changed_fields[1].as_str(), "documentation");
}

// ============================================================================
// ObjectsChanged
// ============================================================================

#[test]
fn objects_changed_empty() {
    let stage = setup_stage();
    let notice = ObjectsChanged::new(
        Arc::downgrade(&stage),
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
        NamespaceEditsInfo::default(),
    );

    assert!(notice.stage().is_some());
    assert!(notice.get_resynced_paths().is_empty());
    assert!(notice.get_changed_info_only_paths().is_empty());
}

#[test]
fn objects_changed_with_resyncs() {
    // C++ ref: test_ObjectsChangedNotice — resync paths
    let stage = setup_stage();
    stage.define_prim("/Foo", "Xform").expect("define");

    let path = Path::from_string("/Foo").expect("path");
    let mut resyncs = HashMap::new();
    resyncs.insert(path.clone(), vec![ChangeEntry::new()]);

    let notice = ObjectsChanged::new_with_resyncs(Arc::downgrade(&stage), resyncs);

    assert!(notice.resynced_path(&path));
    assert!(!notice.get_resynced_paths().is_empty());
}

#[test]
fn objects_changed_info_only() {
    // C++ ref: test_ObjectsChangedNotice — info-only changes
    let stage = setup_stage();
    stage.define_prim("/Foo", "Xform").expect("define");

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
    assert!(!notice.get_changed_info_only_paths().is_empty());
}

#[test]
fn objects_changed_has_changed_fields() {
    let stage = setup_stage();
    let path = Path::from_string("/Foo").expect("path");

    let mut info = HashMap::new();
    info.insert(
        path.clone(),
        vec![ChangeEntry::with_changed_fields(vec![
            Token::new("typeName"),
            Token::new("documentation"),
        ])],
    );

    let notice = ObjectsChanged::new(
        Arc::downgrade(&stage),
        HashMap::new(),
        info,
        HashMap::new(),
        NamespaceEditsInfo::default(),
    );

    assert!(notice.has_changed_fields_path(&path));
    let fields = notice.get_changed_fields_path(&path);
    assert_eq!(fields.len(), 2);
}

#[test]
fn objects_changed_no_changed_fields() {
    let stage = setup_stage();
    let path = Path::from_string("/Bar").expect("path");

    let notice = ObjectsChanged::new(
        Arc::downgrade(&stage),
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
        NamespaceEditsInfo::default(),
    );

    assert!(!notice.has_changed_fields_path(&path));
    let fields = notice.get_changed_fields_path(&path);
    assert!(fields.is_empty());
}

// ============================================================================
// ObjectsChanged — prim resync type
// ============================================================================

#[test]
fn objects_changed_prim_resync_type_delete() {
    let stage = setup_stage();
    let path = Path::from_string("/Foo").expect("path");

    let mut ns_edits = NamespaceEditsInfo::default();
    ns_edits.prim_resyncs.insert(
        path.clone(),
        PrimResyncInfo {
            resync_type: PrimResyncType::Delete,
            associated_path: None,
        },
    );

    let mut resyncs = HashMap::new();
    resyncs.insert(path.clone(), vec![ChangeEntry::new()]);

    let notice = ObjectsChanged::new(
        Arc::downgrade(&stage),
        resyncs,
        HashMap::new(),
        HashMap::new(),
        ns_edits,
    );

    let mut assoc = None;
    let resync_type = notice.get_prim_resync_type(&path, &mut assoc);
    assert_eq!(resync_type, PrimResyncType::Delete);
}

#[test]
fn objects_changed_prim_resync_type_rename() {
    let stage = setup_stage();
    let source = Path::from_string("/OldName").expect("path");
    let dest = Path::from_string("/NewName").expect("path");

    let mut ns_edits = NamespaceEditsInfo::default();
    ns_edits.prim_resyncs.insert(
        source.clone(),
        PrimResyncInfo {
            resync_type: PrimResyncType::RenameSource,
            associated_path: Some(dest.clone()),
        },
    );
    ns_edits.prim_resyncs.insert(
        dest.clone(),
        PrimResyncInfo {
            resync_type: PrimResyncType::RenameDestination,
            associated_path: Some(source.clone()),
        },
    );

    let mut resyncs = HashMap::new();
    resyncs.insert(source.clone(), vec![ChangeEntry::new()]);
    resyncs.insert(dest.clone(), vec![ChangeEntry::new()]);

    let notice = ObjectsChanged::new(
        Arc::downgrade(&stage),
        resyncs,
        HashMap::new(),
        HashMap::new(),
        ns_edits,
    );

    let mut assoc_src = None;
    let mut assoc_dst = None;
    assert_eq!(
        notice.get_prim_resync_type(&source, &mut assoc_src),
        PrimResyncType::RenameSource
    );
    assert_eq!(
        notice.get_prim_resync_type(&dest, &mut assoc_dst),
        PrimResyncType::RenameDestination
    );
}

// ============================================================================
// ObjectsChanged — renamed properties
// ============================================================================

#[test]
fn objects_changed_renamed_properties() {
    let stage = setup_stage();

    let mut ns_edits = NamespaceEditsInfo::default();
    let prop_path = Path::from_string("/Foo.oldAttr").expect("path");
    ns_edits
        .renamed_properties
        .push((prop_path.clone(), Token::new("newAttr")));

    let notice = ObjectsChanged::new(
        Arc::downgrade(&stage),
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
        ns_edits,
    );

    let renamed = notice.get_renamed_properties();
    assert_eq!(renamed.len(), 1);
    assert_eq!(renamed[0].0.get_string(), "/Foo.oldAttr");
    assert_eq!(renamed[0].1.as_str(), "newAttr");
}
