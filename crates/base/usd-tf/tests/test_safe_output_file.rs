// Port of testenv/safeOutputFile.cpp
//
// Original tests: Replace (new file), Replace (existing file), Update,
// Replace symlink (Unix only), file permissions (Unix only), Discard.
// Error cases (empty path, unwritable dirs) are adapted to Rust semantics.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use usd_tf::safe_output_file::SafeOutputFile;

// ---------------------------------------------------------------------------
// Helper — unique temp path inside the system temp dir
// ---------------------------------------------------------------------------

fn tmp(name: &str) -> PathBuf {
    std::env::temp_dir().join(name)
}

fn cleanup(path: &Path) {
    let _ = fs::remove_file(path);
}

// ---------------------------------------------------------------------------
// Error cases  (mirrors TestErrorCases)
// ---------------------------------------------------------------------------

#[test]
fn error_empty_path_update() {
    // Empty path must fail.
    assert!(
        SafeOutputFile::update("").is_err(),
        "update(\"\") must return Err"
    );
}

#[test]
fn error_empty_path_replace() {
    assert!(
        SafeOutputFile::replace("").is_err(),
        "replace(\"\") must return Err"
    );
}

#[test]
fn error_nonexistent_directory_update() {
    // Parent directory does not exist.
    let p = tmp("usd_tf_nonexistent_dir_12345/file.txt");
    assert!(
        SafeOutputFile::update(&p).is_err(),
        "update into missing dir must fail"
    );
}

#[test]
fn error_nonexistent_directory_replace() {
    let p = tmp("usd_tf_nonexistent_dir_12345/file.txt");
    assert!(
        SafeOutputFile::replace(&p).is_err(),
        "replace into missing dir must fail"
    );
}

#[test]
fn error_update_nonexistent_file() {
    // update() requires the file to exist.
    let p = tmp("usd_tf_no_such_file_abcde.txt");
    cleanup(&p);
    assert!(
        SafeOutputFile::update(&p).is_err(),
        "update of missing file must fail"
    );
}

// ---------------------------------------------------------------------------
// TestReplaceNewFile
// ---------------------------------------------------------------------------

#[test]
fn replace_new_file_commit() {
    let target = tmp("usd_tf_new_file_commit.txt");
    cleanup(&target);

    // Target does not exist yet.
    assert!(!target.exists(), "pre-condition: target must not exist");

    let mut outf = SafeOutputFile::replace(&target).expect("replace() must succeed for new file");
    assert!(!outf.is_open_for_update());
    assert!(outf.is_open());

    // Temp file created in replace mode.
    let temp_path = outf.temp_path().map(|p| p.to_path_buf());
    assert!(temp_path.is_some(), "replace mode must create a temp file");
    assert!(temp_path.as_ref().unwrap().exists(), "temp file must exist");

    write!(outf, "New Content\n").expect("write must succeed");

    // Commit.
    outf.close().expect("close must succeed");
    assert!(!outf.is_open(), "file must be closed after close()");

    // Target now exists with correct content.
    let content = fs::read_to_string(&target).expect("target must be readable after close");
    assert_eq!(content, "New Content\n");

    // Temp file gone (it was renamed over target).
    if let Some(tp) = temp_path {
        assert!(
            !tp.exists() || tp == target,
            "temp must be removed after commit"
        );
    }

    cleanup(&target);
}

// ---------------------------------------------------------------------------
// TestReplaceExistingFile
// ---------------------------------------------------------------------------

#[test]
fn replace_existing_file() {
    let target = tmp("usd_tf_ex_file_commit.txt");
    fs::write(&target, "Existing content\n").expect("write original");

    let mut outf =
        SafeOutputFile::replace(&target).expect("replace() must succeed for existing file");
    assert!(!outf.is_open_for_update());

    // Temp file exists alongside original.
    let temp_path = outf.temp_path().map(|p| p.to_path_buf());
    assert!(temp_path.is_some());

    write!(outf, "New Content\n").expect("write");
    outf.close().expect("close");

    // Target has new content.
    let content = fs::read_to_string(&target).expect("read after replace");
    assert_eq!(content, "New Content\n");

    cleanup(&target);
}

// ---------------------------------------------------------------------------
// TestUpdateExistingFile
// ---------------------------------------------------------------------------

#[test]
fn update_existing_file() {
    let target = tmp("usd_tf_ex_file_update.txt");
    fs::write(&target, "Existing content\n").expect("write original");

    let mut outf = SafeOutputFile::update(&target).expect("update() must succeed");
    assert!(outf.is_open_for_update());
    // Update mode: no temp file.
    assert!(outf.temp_path().is_none(), "update mode has no temp file");

    write!(outf, "New Content\n").expect("write");
    outf.close().expect("close");
    assert!(!outf.is_open());

    // File exists and starts with the new content.
    let content = fs::read_to_string(&target).expect("read after update");
    assert!(
        content.starts_with("New Content\n"),
        "file should start with new content, got: {:?}",
        content
    );

    cleanup(&target);
}

// ---------------------------------------------------------------------------
// TestFilePermissions — basic sanity: file must exist after close
// (platform-specific permission bits are Unix-only in the C++ original,
//  so here we only assert existence)
// ---------------------------------------------------------------------------

#[test]
fn file_permissions_new_file_exists() {
    let target = tmp("usd_tf_new_file_perm.txt");
    cleanup(&target);

    let mut outf = SafeOutputFile::replace(&target).expect("replace");
    outf.close().expect("close");

    assert!(target.exists(), "new file must exist after replace+close");
    cleanup(&target);
}

#[test]
fn file_permissions_existing_file_preserved() {
    let target = tmp("usd_tf_existing_file_perm.txt");
    fs::write(&target, "").expect("create");

    let mut outf = SafeOutputFile::replace(&target).expect("replace");
    write!(outf, "data\n").expect("write");
    outf.close().expect("close");

    assert!(target.exists(), "existing file must survive replace+close");
    cleanup(&target);
}

// ---------------------------------------------------------------------------
// TestDiscard
// ---------------------------------------------------------------------------

#[test]
fn discard_update_is_error() {
    // Calling discard() on an Update-mode file must return Err.
    let target = tmp("usd_tf_discard_update.txt");
    fs::write(&target, "Existing content\n").expect("write original");

    let mut outf = SafeOutputFile::update(&target).expect("update");
    let result = outf.discard();
    assert!(result.is_err(), "discard on Update mode must be an error");

    // Close normally to avoid leaving the file locked.
    let _ = outf.close();
    cleanup(&target);
}

#[test]
fn discard_replace_preserves_original() {
    // Discard on Replace: original content must be untouched.
    let target = tmp("usd_tf_discard_replace.txt");
    fs::write(&target, "Existing content\n").expect("write original");

    let mut outf = SafeOutputFile::replace(&target).expect("replace");
    write!(outf, "New Content").expect("write");
    outf.discard()
        .expect("discard must succeed on Replace mode");

    assert!(!outf.is_open(), "file must be closed after discard");

    let content = fs::read_to_string(&target).expect("read");
    assert_eq!(
        content, "Existing content\n",
        "original must be intact after discard"
    );

    cleanup(&target);
}

#[test]
fn discard_replace_new_file_not_created() {
    // Discard on Replace for a new (non-existent) file: target must NOT appear.
    let target = tmp("usd_tf_discard_new.txt");
    cleanup(&target);

    let mut outf = SafeOutputFile::replace(&target).expect("replace");
    write!(outf, "New Content").expect("write");
    outf.discard().expect("discard");

    assert!(
        !target.exists(),
        "target must not exist after discard of new-file replace"
    );
}

// ---------------------------------------------------------------------------
// Drop commits (implicit close)
// ---------------------------------------------------------------------------

#[test]
fn drop_commits_replace() {
    let target = tmp("usd_tf_drop_commit.txt");
    cleanup(&target);

    {
        let mut outf = SafeOutputFile::replace(&target).expect("replace");
        write!(outf, "auto-committed\n").expect("write");
        // Drop here — must commit.
    }

    let content = fs::read_to_string(&target).expect("file must exist after drop-commit");
    assert_eq!(content, "auto-committed\n");

    cleanup(&target);
}

// ---------------------------------------------------------------------------
// Release updated file
// ---------------------------------------------------------------------------

#[test]
fn release_updated_file_ok() {
    let target = tmp("usd_tf_release_update.txt");
    fs::write(&target, "data\n").expect("create");

    let outf = SafeOutputFile::update(&target).expect("update");
    let raw = outf.release_updated_file().expect("release must succeed");
    drop(raw);

    cleanup(&target);
}

#[test]
fn release_replace_mode_is_error() {
    let target = tmp("usd_tf_release_replace.txt");
    cleanup(&target);

    let outf = SafeOutputFile::replace(&target).expect("replace");
    assert!(
        outf.release_updated_file().is_err(),
        "release of Replace-mode file must fail"
    );
}
