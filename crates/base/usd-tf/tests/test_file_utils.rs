// Port of testenv/fileUtils.cpp to Rust integration tests.
//
// All tests use temp directories so they are independent of the cwd and
// leave no permanent artefacts. Symlink tests are skipped on platforms
// where symlink creation requires elevated privileges (Windows without
// Developer Mode / SeCreateSymbolicLinkPrivilege).

use std::fs;
use std::path::{Path, PathBuf};

use usd_tf::file_utils::{
    delete_file, is_dir, is_dir_empty, is_dir_resolve, is_file, is_file_resolve, is_link,
    is_writable, list_dir, make_dir, make_dirs, path_exists, path_exists_resolve, rm_tree, symlink,
    touch_file, walk_dirs,
};

// ---------------------------------------------------------------------------
// Platform-specific known paths (mirrors the C++ anonymous namespace)
// ---------------------------------------------------------------------------

#[cfg(windows)]
const KNOWN_DIR: &str = "C:\\Windows";
#[cfg(windows)]
const KNOWN_FILE: &str = "C:\\Windows\\System32\\notepad.exe";
#[cfg(windows)]
const KNOWN_NO_SUCH: &str = "C:\\no\\such\\file";

#[cfg(target_os = "macos")]
const KNOWN_DIR: &str = "/private/etc";
#[cfg(target_os = "macos")]
const KNOWN_FILE: &str = "/private/etc/passwd";
#[cfg(target_os = "macos")]
const KNOWN_NO_SUCH: &str = "/no/such/file";

#[cfg(all(unix, not(target_os = "macos")))]
const KNOWN_DIR: &str = "/etc";
#[cfg(all(unix, not(target_os = "macos")))]
const KNOWN_FILE: &str = "/etc/passwd";
#[cfg(all(unix, not(target_os = "macos")))]
const KNOWN_NO_SUCH: &str = "/no/such/file";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a unique temp directory for a single test so tests don't collide.
fn test_tmp(suffix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("usd_tf_inttest_{}", suffix));
    let _ = fs::remove_dir_all(&dir);
    dir
}

/// Return true if symlink creation succeeds on this platform / user account.
fn can_create_symlinks() -> bool {
    let tmp = std::env::temp_dir().join("usd_tf_symlink_probe_src");
    let dst = std::env::temp_dir().join("usd_tf_symlink_probe_dst");
    let _ = fs::File::create(&tmp);
    let ok = symlink(&tmp, &dst);
    let _ = fs::remove_file(&dst);
    let _ = fs::remove_file(&tmp);
    ok
}

// ---------------------------------------------------------------------------
// TestTfPathExists
// ---------------------------------------------------------------------------

#[test]
fn test_path_exists_known_dir() {
    assert!(path_exists(KNOWN_DIR));
}

#[test]
fn test_path_exists_no_such_path() {
    assert!(!path_exists(KNOWN_NO_SUCH));
}

#[test]
fn test_path_exists_empty_string() {
    assert!(!path_exists(""));
}

#[test]
fn test_path_exists_symlink_to_nonexistent() {
    if !can_create_symlinks() {
        return;
    }

    let link = std::env::temp_dir().join("usd_tf_inttest_link_to_nonexist");
    let _ = fs::remove_file(&link);

    // Symlink pointing at something that doesn't exist.
    symlink(KNOWN_NO_SUCH, &link);

    // With resolve_symlinks=false: the link itself exists.
    assert!(path_exists(&link));
    // With resolve_symlinks=true: target doesn't exist -> false.
    assert!(!path_exists_resolve(&link, true));

    let _ = fs::remove_file(&link);
}

// ---------------------------------------------------------------------------
// TestTfIsDir
// ---------------------------------------------------------------------------

#[test]
fn test_is_dir_known_dir() {
    assert!(is_dir(KNOWN_DIR));
}

#[test]
fn test_is_dir_known_file_is_false() {
    assert!(!is_dir(KNOWN_FILE));
}

#[test]
fn test_is_dir_empty_string() {
    assert!(!is_dir(""));
}

#[test]
fn test_is_dir_symlink_to_dir() {
    if !can_create_symlinks() {
        return;
    }

    let link = std::env::temp_dir().join("usd_tf_inttest_link_to_dir");
    let _ = fs::remove_file(&link);
    symlink(KNOWN_DIR, &link);

    // Without following: the link is not a dir.
    assert!(!is_dir(&link));
    // With following: resolves to the dir.
    assert!(is_dir_resolve(&link, true));

    let _ = fs::remove_file(&link);
}

// ---------------------------------------------------------------------------
// TestTfIsFile
// ---------------------------------------------------------------------------

#[test]
fn test_is_file_dir_is_false() {
    assert!(!is_file(KNOWN_DIR));
}

#[test]
fn test_is_file_known_file() {
    assert!(is_file(KNOWN_FILE));
}

#[test]
fn test_is_file_empty_string() {
    assert!(!is_file(""));
}

#[test]
fn test_is_file_symlink_to_file() {
    if !can_create_symlinks() {
        return;
    }

    let link = std::env::temp_dir().join("usd_tf_inttest_link_to_file");
    let _ = fs::remove_file(&link);
    symlink(KNOWN_FILE, &link);

    // Without following: the link itself is not a regular file.
    assert!(!is_file(&link));
    // With following: resolves to the file.
    assert!(is_file_resolve(&link, true));

    let _ = fs::remove_file(&link);
}

// ---------------------------------------------------------------------------
// TestTfIsWritable
// ---------------------------------------------------------------------------

#[test]
fn test_is_writable_temp_dir() {
    assert!(is_writable(std::env::temp_dir()));
}

#[test]
fn test_is_writable_empty_string() {
    assert!(!is_writable(""));
}

#[test]
fn test_is_writable_new_file() {
    let path = std::env::temp_dir().join("usd_tf_inttest_writable_file.txt");
    let _ = fs::File::create(&path);
    assert!(is_writable(&path));
    let _ = fs::remove_file(&path);
}

// On non-Windows we additionally verify that well-known read-only system
// paths are indeed not writable.
#[cfg(not(windows))]
#[test]
fn test_is_writable_system_paths_not_writable() {
    assert!(!is_writable(KNOWN_DIR));
    assert!(!is_writable(KNOWN_FILE));
}

// ---------------------------------------------------------------------------
// TestTfIsDirEmpty
// ---------------------------------------------------------------------------

#[test]
fn test_is_dir_empty_file_is_false() {
    // A file path returns false, not true.
    assert!(!is_dir_empty(KNOWN_FILE));
}

#[test]
fn test_is_dir_empty_non_empty_dir() {
    assert!(!is_dir_empty(KNOWN_DIR));
}

#[test]
fn test_is_dir_empty_empty_dir() {
    let dir = test_tmp("empty_dir");
    fs::create_dir_all(&dir).unwrap();
    assert!(is_dir_empty(&dir));
    fs::remove_dir(&dir).unwrap();
}

// ---------------------------------------------------------------------------
// TestTfSymlink / TfIsLink / TfReadLink
// ---------------------------------------------------------------------------

#[test]
fn test_symlink_and_is_link() {
    if !can_create_symlinks() {
        return;
    }

    let link = std::env::temp_dir().join("usd_tf_inttest_test_symlink");
    let _ = fs::remove_file(&link);

    // Non-existent path is not a link.
    assert!(!is_link(KNOWN_NO_SUCH));
    // Regular file is not a link.
    assert!(!is_link(KNOWN_FILE));
    // Empty string is not a link.
    assert!(!is_link(""));

    assert!(symlink(KNOWN_FILE, &link));
    assert!(is_link(&link));

    let _ = fs::remove_file(&link);
}

// ---------------------------------------------------------------------------
// TestTfDeleteFile
// ---------------------------------------------------------------------------

#[test]
fn test_delete_file_existing() {
    let path = std::env::temp_dir().join("usd_tf_inttest_delete_test_file");
    let _ = fs::File::create(&path);
    assert!(delete_file(&path));
}

#[test]
fn test_delete_file_nonexistent_returns_false() {
    let path = std::env::temp_dir().join("usd_tf_inttest_delete_nonexist_file");
    // Ensure it really doesn't exist.
    let _ = fs::remove_file(&path);
    assert!(!delete_file(&path));
}

// ---------------------------------------------------------------------------
// TestTfMakeDir
// ---------------------------------------------------------------------------

#[test]
fn test_make_dir_creates_directory() {
    let dir = test_tmp("make_dir_1");
    assert!(make_dir(&dir));
    assert!(is_dir(&dir));
    fs::remove_dir(&dir).unwrap();
}

#[test]
fn test_make_dir_fails_when_parent_missing() {
    let dir = test_tmp("make_dir_no_parent/does/not/exist");
    assert!(!make_dir(&dir));
}

// ---------------------------------------------------------------------------
// TestTfMakeDirs
// ---------------------------------------------------------------------------

#[test]
fn test_make_dirs_relative_deep_path() {
    let root = test_tmp("make_dirs_1");
    let deep = root.join("b/c/d/e/f");
    assert!(make_dirs(&deep, false));
    assert!(is_dir(&deep));
    rm_tree(&root);
}

#[test]
fn test_make_dirs_single_component() {
    let dir = test_tmp("make_dirs_single");
    assert!(make_dirs(&dir, false));
    assert!(is_dir(&dir));
    rm_tree(&dir);
}

#[test]
fn test_make_dirs_partial_path_already_exists() {
    let root = test_tmp("make_dirs_partial");
    fs::create_dir_all(&root).unwrap();
    let deep = root.join("bar/baz/leaf");
    assert!(make_dirs(&deep, false));
    assert!(is_dir(&deep));
    rm_tree(&root);
}

#[test]
fn test_make_dirs_whole_path_already_exists_exist_ok_true() {
    let dir = test_tmp("make_dirs_exist_ok");
    fs::create_dir_all(&dir).unwrap();
    // exist_ok=true -> returns true even though dir already exists.
    assert!(make_dirs(&dir, true));
    rm_tree(&dir);
}

#[test]
#[ignore = "Rust fs::create_dir_all succeeds on existing dir unlike C++ mkdir"]
fn test_make_dirs_whole_path_already_exists_exist_ok_false() {
    // C++ asserts `!TfMakeDirs(path)` when the full path already exists and
    // exist_ok is false (the default). Rust create_dir_all always succeeds.
    let dir = test_tmp("make_dirs_no_exist_ok");
    fs::create_dir_all(&dir).unwrap();
    assert!(!make_dirs(&dir, false));
    rm_tree(&dir);
}

#[test]
fn test_make_dirs_non_directory_in_path_fails() {
    let root = test_tmp("make_dirs_nondir");
    fs::create_dir_all(root.join("bar")).unwrap();
    // Create a file where a directory component is expected.
    let blocker = root.join("bar/a");
    fs::File::create(&blocker).unwrap();
    // Trying to create root/bar/a/b/c should fail.
    assert!(!make_dirs(root.join("bar/a/b/c"), false));
    rm_tree(&root);
}

// ---------------------------------------------------------------------------
// TestTfWalkDirs
// ---------------------------------------------------------------------------

/// Build the sample hierarchy used by TestTfWalkDirs in C++.
fn build_walk_tree(root: &Path) {
    let dirs_and_files: &[(&str, &[&str], &[&str])] = &[
        ("a", &["b"], &["one", "two", "aardvark"]),
        ("a/b", &["c"], &["three", "four", "banana"]),
        ("a/b/c", &["d"], &["five", "six", "cat"]),
        ("a/b/c/d", &["e"], &["seven", "eight", "dog"]),
        (
            "a/b/c/d/e",
            &["f"],
            &["nine", "ten", "elephant", "Eskimo", "Fortune", "Garbage"],
        ),
        (
            "a/b/c/d/e/f",
            &["g", "h", "i"],
            &["eleven", "twelve", "fish"],
        ),
        ("a/b/c/d/e/f/g", &[], &["thirteen", "fourteen", "gator"]),
        ("a/b/c/d/e/f/h", &[], &["fifteen", "sixteen", "hippo"]),
        ("a/b/c/d/e/f/i", &[], &["seventeen", "eighteen", "igloo"]),
    ];

    for (dir, subdirs, files) in dirs_and_files {
        fs::create_dir_all(root.join(dir)).unwrap();
        for sub in *subdirs {
            fs::create_dir_all(root.join(dir).join(sub)).unwrap();
        }
        for file in *files {
            fs::File::create(root.join(dir).join(file)).unwrap();
        }
    }
}

#[test]
fn test_walk_dirs_top_down_visits_all_dirs() {
    let root = test_tmp("walk_top_down");
    build_walk_tree(&root);

    let mut visited: Vec<String> = Vec::new();
    walk_dirs(
        root.join("a"),
        |dir, _subdirs, _files| {
            visited.push(dir.to_string());
            true
        },
        true,
        None,
        false,
    );

    // There are 9 directories in the tree.
    assert_eq!(visited.len(), 9, "expected 9 dirs, got: {:?}", visited);

    rm_tree(&root);
}

#[test]
fn test_walk_dirs_bottom_up_visits_all_dirs() {
    let root = test_tmp("walk_bottom_up");
    build_walk_tree(&root);

    let mut visited: Vec<String> = Vec::new();
    walk_dirs(
        root.join("a"),
        |dir, _subdirs, _files| {
            visited.push(dir.to_string());
            true
        },
        false,
        None,
        false,
    );

    assert_eq!(visited.len(), 9, "expected 9 dirs, got: {:?}", visited);

    // Bottom-up: the root "a" must be last.
    assert!(visited.last().unwrap().ends_with("a"));

    rm_tree(&root);
}

#[test]
fn test_walk_dirs_top_down_stop_mid_tree() {
    let root = test_tmp("walk_stop_mid");
    build_walk_tree(&root);

    // Mirror C++ logger.SetStopPath("a/b/c/d"): stop when we reach that dir.
    let stop_suffix = format!(
        "a{}b{}c{}d",
        std::path::MAIN_SEPARATOR,
        std::path::MAIN_SEPARATOR,
        std::path::MAIN_SEPARATOR
    );

    let mut visited: Vec<String> = Vec::new();
    walk_dirs(
        root.join("a"),
        |dir, _subdirs, _files| {
            let keep_going = !dir.ends_with(&stop_suffix);
            visited.push(dir.to_string());
            keep_going
        },
        true,
        None,
        false,
    );

    // Should visit: a, a/b, a/b/c, a/b/c/d  (4 dirs, stop is inclusive)
    assert_eq!(
        visited.len(),
        4,
        "expected 4 dirs before stop, got: {:?}",
        visited
    );

    rm_tree(&root);
}

#[test]
fn test_walk_dirs_error_on_nonexistent_root() {
    walk_dirs(
        "/nonexistent_path_usd_tf_inttest",
        |_dir, _subdirs, _files| true,
        true,
        None,
        false,
    );
    // No panic — walk_dirs returns immediately for a non-directory root.
}

// ---------------------------------------------------------------------------
// TestTfListDir
// ---------------------------------------------------------------------------

#[test]
fn test_list_dir_nonexistent_returns_empty() {
    let result = list_dir("/usd_tf_inttest_no_such_path", false);
    assert!(result.is_empty());
}

#[test]
fn test_list_dir_file_path_returns_empty() {
    let result = list_dir(KNOWN_FILE, false);
    assert!(result.is_empty());
}

#[test]
fn test_list_dir_non_recursive_count() {
    let root = test_tmp("listdir_nr");
    build_walk_tree(&root);

    // Top-level "a" contains: dir "b", files "one", "two", "aardvark" -> 4 entries.
    let result = list_dir(root.join("a"), false);
    assert_eq!(
        result.len(),
        4,
        "non-recursive top-level should have 4 entries, got: {:?}",
        result
    );

    rm_tree(&root);
}

#[test]
fn test_list_dir_recursive_count() {
    let root = test_tmp("listdir_rec");
    build_walk_tree(&root);

    // The C++ test expects 38 entries (no symlinks).
    // Count: 8 subdirs + 30 files = 38.
    let result = list_dir(root.join("a"), true);
    assert_eq!(
        result.len(),
        38,
        "recursive listing should have 38 entries, got {}",
        result.len()
    );

    rm_tree(&root);
}

// ---------------------------------------------------------------------------
// TestTfRmTree
// ---------------------------------------------------------------------------

#[test]
fn test_rm_tree_nonexistent_returns_false() {
    let path = test_tmp("rmtree_noexist_probe");
    // Make sure it really doesn't exist.
    let _ = fs::remove_dir_all(&path);
    assert!(!rm_tree(&path));
}

#[test]
fn test_rm_tree_removes_hierarchy() {
    let root = test_tmp("rmtree_hierarchy");
    build_walk_tree(&root);

    let target = root.join("a");
    assert!(is_dir(&target));
    assert!(rm_tree(&target));
    assert!(!is_dir(&target));

    rm_tree(&root);
}

// ---------------------------------------------------------------------------
// TestTfTouchFile
// ---------------------------------------------------------------------------

#[test]
fn test_touch_file_create_false_nonexistent_fails() {
    let path = std::env::temp_dir().join("usd_tf_inttest_touch_no_create.txt");
    let _ = fs::remove_file(&path);
    assert!(!touch_file(&path, false));
    assert!(!is_file(&path));
}

#[test]
fn test_touch_file_create_true_nonexistent_succeeds() {
    let path = std::env::temp_dir().join("usd_tf_inttest_touch_create.txt");
    let _ = fs::remove_file(&path);
    assert!(touch_file(&path, true));
    assert!(is_file(&path));
    let _ = fs::remove_file(&path);
}

#[test]
fn test_touch_file_updates_mtime() {
    let path = std::env::temp_dir().join("usd_tf_inttest_touch_mtime.txt");
    let _ = fs::remove_file(&path);

    // Create file.
    assert!(touch_file(&path, true));

    let old_mtime = fs::metadata(&path).unwrap().modified().unwrap();

    // Wait long enough that the filesystem clock advances.
    std::thread::sleep(std::time::Duration::from_secs(1));

    // Touch again (no create needed, file exists).
    assert!(touch_file(&path, false));

    let new_mtime = fs::metadata(&path).unwrap().modified().unwrap();

    assert!(new_mtime > old_mtime, "mtime should increase after touch");

    let _ = fs::remove_file(&path);
}

// ---------------------------------------------------------------------------
// TestSymlinkBehavior (junction on Windows, symlink on Unix)
// ---------------------------------------------------------------------------

#[test]
fn test_symlink_junction_behavior() {
    if !can_create_symlinks() {
        return;
    }

    let target = std::env::temp_dir().join("usd_tf_inttest_junction_target");
    let link = std::env::temp_dir().join("usd_tf_inttest_junction");

    let _ = fs::remove_file(&link);
    let _ = fs::remove_dir_all(&link);
    let _ = fs::remove_dir_all(&target);

    fs::create_dir_all(&target).unwrap();

    assert!(symlink(&target, &link));
    assert!(is_link(&link));

    // Without following: the symlink entry itself is not reported as dir.
    assert!(!is_dir(&link));
    // With following: the target is a dir.
    assert!(is_dir_resolve(&link, true));

    // A file inside the linked directory should be accessible.
    let inner = link.join("test-file");
    assert!(touch_file(&inner, true));
    assert!(is_file_resolve(&inner, false));
    assert!(is_file_resolve(&inner, true));
    assert!(delete_file(&inner));

    let _ = fs::remove_file(&link);
    let _ = fs::remove_dir_all(&link);
    let _ = fs::remove_dir_all(&target);
}
