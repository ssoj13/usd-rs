// Port of testUsdStageNotification.cpp — stage notification subset
// Reference: _ref/OpenUSD/pxr/usd/usd/testenv/testUsdStageNotification.cpp

mod common;

use std::sync::Arc;
use usd_core::notice::{StageContentsChanged, StageNotice};
use usd_core::{InitialLoadSet, Stage};

// ============================================================================
// StageNotice with live stage
// ============================================================================

#[test]
fn stage_notice_with_live_stage() {
    common::setup();
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create");
    let notice = StageNotice::new(Arc::downgrade(&stage));

    assert!(notice.stage().is_some());
    assert!(Arc::ptr_eq(&notice.stage().unwrap(), &stage));
}

#[test]
fn stage_notice_with_dropped_stage() {
    common::setup();
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create");
    let weak = Arc::downgrade(&stage);
    drop(stage);

    let notice = StageNotice::new(weak);
    assert!(notice.stage().is_none());
}

// ============================================================================
// StageContentsChanged
// ============================================================================

#[test]
fn stage_contents_changed_notice() {
    common::setup();
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create");
    let notice = StageContentsChanged::new(Arc::downgrade(&stage));
    assert!(notice.stage().is_some());
}

// ============================================================================
// Multiple notices for same stage
// ============================================================================

#[test]
fn multiple_notices_same_stage() {
    common::setup();
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create");

    let n1 = StageNotice::new(Arc::downgrade(&stage));
    let n2 = StageNotice::new(Arc::downgrade(&stage));

    assert!(Arc::ptr_eq(&n1.stage().unwrap(), &n2.stage().unwrap()));
}

// ============================================================================
// Notice for different stages
// ============================================================================

#[test]
fn notices_for_different_stages() {
    common::setup();
    let stage1 = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("s1");
    let stage2 = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("s2");

    let n1 = StageNotice::new(Arc::downgrade(&stage1));
    let n2 = StageNotice::new(Arc::downgrade(&stage2));

    assert!(!Arc::ptr_eq(&n1.stage().unwrap(), &n2.stage().unwrap()));
}

// ============================================================================
// StageNotice weak ref
// ============================================================================

#[test]
fn stage_notice_weak_ref() {
    common::setup();
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create");
    let notice = StageNotice::new(Arc::downgrade(&stage));

    let weak = notice.stage_weak();
    assert!(weak.upgrade().is_some());
}
