//! Port of testUsdObjectsChangedNoticesSublayerOps.py from OpenUSD
//! Tests ObjectsChanged notices fired by sublayer operations.

mod common;

#[test]
#[ignore = "Needs Tf.Notice registration + sublayer operation notice tracking"]
fn objects_changed_notices_sublayer_ops() {
    common::setup();
    // C++ registers ObjectsChanged + StageContentsChanged notice listeners,
    // performs sublayer add/remove/reorder operations, verifies correct
    // resynced/changedInfoOnly paths in each notice.
}
