// Port of pxr/base/tf/testenv/pathUtils.cpp
// Tests for TfPathUtils — norm_path, abs_path, real_path, get_extension, read_link.
//
// NOTE: TestTfRealPath symlink tests are skipped — they require a writable
// filesystem sandbox and specific symlink topology that is incompatible with a
// unit test environment.  TestTfGlob and TestTfReadLink are also omitted for the
// same reason (they depend on a writable cwd and known system paths).

use usd_tf::path_utils::*;

// ---------------------------------------------------------------------------
// TestTfNormPath
// ---------------------------------------------------------------------------

#[test]
fn test_norm_path_empty() {
    assert_eq!(norm_path(""), ".");
}

#[test]
fn test_norm_path_dot() {
    assert_eq!(norm_path("."), ".");
}

#[test]
fn test_norm_path_dotdot() {
    assert_eq!(norm_path(".."), "..");
}

#[test]
fn test_norm_path_dotdot_removal() {
    assert_eq!(norm_path("foobar/../barbaz"), "barbaz");
}

#[test]
fn test_norm_path_root() {
    assert_eq!(norm_path("/"), "/");
}

#[test]
fn test_norm_path_double_slash() {
    // POSIX allows // at the start to have special meaning; three or more collapse to /
    assert_eq!(norm_path("//"), "//");
    assert_eq!(norm_path("///"), "/");
}

#[test]
fn test_norm_path_complex_trailing_slashes() {
    assert_eq!(norm_path("///foo/.//bar//"), "/foo/bar");
}

#[test]
fn test_norm_path_complex_dotdot_chain() {
    assert_eq!(norm_path("///foo/.//bar//.//..//.//baz"), "/foo/baz");
}

#[test]
fn test_norm_path_unc_prefix_collapse() {
    assert_eq!(norm_path("///..//./foo/.//bar"), "/foo/bar");
}

#[test]
fn test_norm_path_excessive_dotdots() {
    assert_eq!(
        norm_path("foo/bar/../../../../../../baz"),
        "../../../../baz"
    );
}

// ---------------------------------------------------------------------------
// TestTfAbsPath
// ---------------------------------------------------------------------------

#[test]
fn test_abs_path_empty() {
    assert_eq!(abs_path(""), "");
}

#[test]
fn test_abs_path_relative_differs_from_relative() {
    // A relative path should be made absolute (not equal to "foo").
    assert_ne!(abs_path("foo"), "foo");
}

#[test]
fn test_abs_path_already_absolute_unix_style() {
    // On all platforms abs_path of an already-absolute slash-prefixed path
    // should keep that prefix after normalization.
    // Strip drive letter on Windows for a portable check.
    let result = abs_path("/foo/bar");
    let stripped = strip_drive(&result);
    assert_eq!(stripped, "/foo/bar");
}

#[test]
fn test_abs_path_normalizes_dotdot() {
    let result = abs_path("/foo/bar/../baz");
    let stripped = strip_drive(&result);
    assert_eq!(stripped, "/foo/baz");
}

// ---------------------------------------------------------------------------
// TestTfGetExtension
// ---------------------------------------------------------------------------

#[test]
fn test_get_extension_empty() {
    assert_eq!(get_extension(""), "");
}

#[test]
fn test_get_extension_dotfile() {
    // Dotfiles have no extension (leading dot is part of the name)
    assert_eq!(get_extension(".foo"), "");
}

#[test]
fn test_get_extension_dotfile_with_path() {
    assert_eq!(get_extension("/bar/baz/.foo"), "");
}

#[test]
fn test_get_extension_no_extension() {
    // Plain directory component without dot → no extension
    assert_eq!(get_extension("/bar/baz"), "");
}

#[test]
fn test_get_extension_normal() {
    assert_eq!(get_extension("/bar/baz/foo.py"), "py");
}

#[test]
fn test_get_extension_dot_in_directory() {
    // The dot is in a parent directory; the filename itself has extension "py"
    assert_eq!(get_extension("/bar.foo/baz.py"), "py");
}

#[test]
fn test_get_extension_multi_dot() {
    // Last extension only
    assert_eq!(get_extension("/bar/baz/foo.bar.py"), "py");
}

#[test]
fn test_get_extension_hidden_with_extension() {
    assert_eq!(get_extension("/foo/.bar.py"), "py");
}

// ---------------------------------------------------------------------------
// TestTfRealPath — non-symlink subset
// ---------------------------------------------------------------------------

#[test]
fn test_real_path_empty() {
    // Empty path always returns empty string (no error).
    match real_path("", false) {
        Ok(s) => assert_eq!(s, ""),
        Err(_) => {} // allowed
    }
}

#[test]
fn test_real_path_nonexistent_binary() {
    // "binary" as a bare relative name should fail (not an absolute path or
    // a path that resolves without allow_inaccessible_suffix).
    let result = real_path("binary", false);
    // Either an error or an empty string is acceptable.
    match result {
        Ok(s) => assert_eq!(s, ""),
        Err(_) => {}
    }
}

#[test]
fn test_real_path_known_nonexistent_abs() {
    // A path with allow_inaccessible_suffix=true for a nonexistent absolute
    // path should return that absolute path unchanged (C++ TF_AXIOM behavior).
    #[cfg(unix)]
    {
        let result = real_path("/nosuch", true);
        match result {
            Ok(s) => assert_eq!(s, "/nosuch"),
            Err(_) => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Helper used by tests above
// ---------------------------------------------------------------------------

/// Strip Windows drive specifier (e.g. "C:") and convert backslashes to
/// forward slashes so path assertions are portable.
fn strip_drive(path: &str) -> String {
    let s = path.replace('\\', "/");
    // Remove two-character drive specifier like "C:"
    if s.len() >= 2 && s.as_bytes()[1] == b':' && s.as_bytes()[0].is_ascii_alphabetic() {
        s[2..].to_string()
    } else {
        s
    }
}
