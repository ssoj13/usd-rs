//! Package-relative path utilities.
//!
//! Utility functions for working with package assets. Assets within package
//! assets can be addressed via "package-relative" paths.
//!
//! # Package-Relative Paths
//!
//! A package-relative path consists of two parts:
//!
//! - The outer "package" path is the path to the containing package asset.
//! - The inner "packaged" path is the path to an asset contained within the
//!   package asset.
//!
//! Package-relative paths use square brackets to delimit the packaged path:
//!
//! - `Model.package[Geom.file]` - `Geom.file` inside `Model.package`
//! - `Model.package[a/b/Sub.package[c/d/Geom.file]]` - nested packages
//!
//! Delimiters (`[` and `]`) within paths are escaped with backslashes:
//! `\[` and `\]`. This matches C++ OpenUSD's `ArJoinPackageRelativePath`
//! and `ArSplitPackageRelativePath*` behavior.
//!
//! # Examples
//!
//! ```
//! use usd_ar::package_utils::*;
//!
//! assert!(is_package_relative_path("a.usdz[b.usd]"));
//! assert!(!is_package_relative_path("a.usd"));
//!
//! let (pkg, inner) = split_package_relative_path_outer("a.usdz[b.usdz[c.usd]]");
//! assert_eq!(pkg, "a.usdz");
//! assert_eq!(inner, "b.usdz[c.usd]");
//! ```

/// The opening delimiter character used in package-relative paths.
pub const PACKAGE_DELIMITER: char = '[';

/// The closing delimiter character used in package-relative paths.
pub const PACKAGE_DELIMITER_CLOSE: char = ']';

// ── Internal helpers (matching C++ anonymous namespace) ──────────────────

/// Find the matching opening `[` for a closing `]` at `close_pos`.
/// Returns `Some(pos)` of the `[`, or `None` if not found.
/// Handles escaped delimiters (`\[`, `\]`).
///
/// Matches C++ `_FindMatchingOpeningDelimiter`.
fn find_matching_opening_delimiter(path: &str, close_pos: usize) -> Option<usize> {
    let bytes = path.as_bytes();
    let mut num_open_needed: usize = 1;
    let mut i = close_pos;

    while i > 0 && num_open_needed > 0 {
        i -= 1;
        let ch = bytes[i];
        if ch == b'[' || ch == b']' {
            // Ignore this delimiter if it's been escaped
            if i > 0 && bytes[i - 1] == b'\\' {
                continue;
            }
            if ch == b'[' {
                num_open_needed -= 1;
            } else {
                num_open_needed += 1;
            }
        }
    }

    if num_open_needed == 0 { Some(i) } else { None }
}

/// Find the innermost closing `]` delimiter.
/// The innermost `]` is the first one (from the right) that is NOT preceded
/// by another `]` or preceded by `\` (escaped).
///
/// Matches C++ `_FindInnermostClosingDelimiter`.
fn find_innermost_closing_delimiter(path: &str) -> Option<usize> {
    let bytes = path.as_bytes();
    if bytes.is_empty() || *bytes.last().unwrap() != b']' {
        return None;
    }

    let mut i = bytes.len() - 1;
    loop {
        if i == 0 {
            return None;
        }
        i -= 1;
        if bytes[i] == b'\\' {
            // The `]` after this backslash was escaped, so the innermost
            // delimiter is really the one before that.
            return Some(i + 1);
        } else if bytes[i] != b']' {
            return Some(i + 1);
        }
        // else it's another ']', keep going
    }
}

/// Escape delimiters in `path` using backslash: `[` → `\[`, `]` → `\]`.
///
/// If `path` is itself a package-relative path, only the package portion
/// (before the outermost `[`) is escaped; the packaged portion is assumed
/// to already be escaped.
///
/// Matches C++ `_EscapeDelimiters`.
fn escape_delimiters_internal(path: &str) -> String {
    if path.is_empty() {
        return String::new();
    }

    let bytes = path.as_bytes();
    let escape_end = if *bytes.last().unwrap() == b']' {
        // Find matching opening delimiter for the outermost `]`
        find_matching_opening_delimiter(path, bytes.len() - 1).unwrap_or(bytes.len())
    } else {
        bytes.len()
    };

    let to_escape = &path[..escape_end];
    let rest = &path[escape_end..];
    let mut result = to_escape.replace('[', "\\[").replace(']', "\\]");
    result.push_str(rest);
    result
}

/// Unescape delimiters in `path`: `\[` → `[`, `\]` → `]`.
///
/// If `path` is itself a package-relative path, only the package portion
/// is unescaped.
///
/// Matches C++ `_UnescapeDelimiters`.
fn unescape_delimiters_internal(path: &str) -> String {
    if path.is_empty() {
        return String::new();
    }

    let bytes = path.as_bytes();
    let escape_end = if *bytes.last().unwrap() == b']' {
        find_matching_opening_delimiter(path, bytes.len() - 1).unwrap_or(bytes.len())
    } else {
        bytes.len()
    };

    let to_unescape = &path[..escape_end];
    let rest = &path[escape_end..];
    let mut result = to_unescape.replace("\\[", "[").replace("\\]", "]");
    result.push_str(rest);
    result
}

// ── Public API ───────────────────────────────────────────────────────────

/// Returns true if `path` is a package-relative path, false otherwise.
///
/// A package-relative path must end with `]` and have a matching `[`
/// that is not escaped. This matches C++ `ArIsPackageRelativePath`.
///
/// # Examples
///
/// ```
/// use usd_ar::package_utils::is_package_relative_path;
///
/// assert!(is_package_relative_path("Model.usdz[Geom.usd]"));
/// assert!(is_package_relative_path("a.pack[b.pack[c.file]]"));
/// assert!(!is_package_relative_path("Model.usd"));
/// assert!(!is_package_relative_path(""));
/// ```
#[inline]
#[must_use]
pub fn is_package_relative_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    if bytes.is_empty() || *bytes.last().unwrap() != b']' {
        return false;
    }
    find_matching_opening_delimiter(path, bytes.len() - 1).is_some()
}

/// Combines the given paths into a single package-relative path, nesting
/// paths as necessary. Empty paths are skipped.
///
/// Delimiters in inner paths are escaped with backslashes.
///
/// Matches C++ `ArJoinPackageRelativePath(vector)`.
///
/// # Examples
///
/// ```
/// use usd_ar::package_utils::join_package_relative_path;
///
/// assert_eq!(
///     join_package_relative_path(&["a.pack", "b.pack"]),
///     "a.pack[b.pack]"
/// );
/// assert_eq!(
///     join_package_relative_path(&["a.pack", "b.pack", "c.pack"]),
///     "a.pack[b.pack[c.pack]]"
/// );
/// ```
#[must_use]
pub fn join_package_relative_path(paths: &[&str]) -> String {
    join_impl(paths.iter().copied())
}

/// Combines two paths into a package-relative path.
///
/// Matches C++ `ArJoinPackageRelativePath(packagePath, packagedPath)`.
///
/// # Examples
///
/// ```
/// use usd_ar::package_utils::join_package_relative_path_pair;
///
/// assert_eq!(
///     join_package_relative_path_pair("a.pack", "b.file"),
///     "a.pack[b.file]"
/// );
/// assert_eq!(
///     join_package_relative_path_pair("a.pack[b.pack]", "c.file"),
///     "a.pack[b.pack[c.file]]"
/// );
/// ```
#[must_use]
pub fn join_package_relative_path_pair(package_path: &str, packaged_path: &str) -> String {
    join_impl([package_path, packaged_path].into_iter())
}

/// Internal join implementation matching C++ `_JoinPackageRelativePath`.
fn join_impl<'a>(mut iter: impl Iterator<Item = &'a str>) -> String {
    // Skip leading empty paths, find first non-empty
    let first = loop {
        match iter.next() {
            Some(p) if !p.is_empty() => break p.to_string(),
            Some(_) => continue,
            None => return String::new(),
        }
    };

    let mut result = first;

    // Determine insert position: just before the innermost `]` run.
    // If result ends with `]`, find how many trailing `]` there are
    // (not preceded by `\`) and insert before them.
    let mut insert_idx = result.len();
    if result.ends_with(']') {
        let bytes = result.as_bytes();
        let mut pos = bytes.len();
        while pos > 0 && bytes[pos - 1] == b']' {
            pos -= 1;
        }
        // `pos` now points just past the last non-`]` character
        insert_idx = pos;
    }

    for path in iter {
        if path.is_empty() {
            continue;
        }

        // Escape delimiters in the path being inserted
        let escaped = escape_delimiters_internal(path);
        let to_insert = format!("[{}]", escaped);
        result.insert_str(insert_idx, &to_insert);
        // Next insertion goes before the newly-added `]` (but after its content)
        insert_idx += to_insert.len() - 1;
    }

    result
}

/// Split package-relative path into a (package path, packaged path) pair.
///
/// If `path` contains nested package-relative paths, the package path will be
/// the outermost package path, and the packaged path will be the inner
/// package-relative path.
///
/// Returns `(path, "")` if the path is not a valid package-relative path.
/// This matches C++ `ArSplitPackageRelativePathOuter`.
///
/// # Examples
///
/// ```
/// use usd_ar::package_utils::split_package_relative_path_outer;
///
/// assert_eq!(
///     split_package_relative_path_outer("a.pack[b.pack]"),
///     ("a.pack".to_string(), "b.pack".to_string())
/// );
/// assert_eq!(
///     split_package_relative_path_outer("a.pack[b.pack[c.pack]]"),
///     ("a.pack".to_string(), "b.pack[c.pack]".to_string())
/// );
/// assert_eq!(
///     split_package_relative_path_outer("not_package_path"),
///     ("not_package_path".to_string(), String::new())
/// );
/// ```
#[must_use]
pub fn split_package_relative_path_outer(path: &str) -> (String, String) {
    let bytes = path.as_bytes();

    // Find outermost closing delimiter (must be at the very end)
    if bytes.is_empty() || *bytes.last().unwrap() != b']' {
        return (path.to_string(), String::new());
    }
    let outermost_close = bytes.len() - 1;

    // Find matching opening delimiter
    let Some(outermost_open) = find_matching_opening_delimiter(path, outermost_close) else {
        return (path.to_string(), String::new());
    };

    // Package path is everything before the outermost `[`
    let package_path = &path[..outermost_open];

    // Packaged path is between `[` and `]`, with unescaping
    let packaged_path = &path[outermost_open + 1..outermost_close];
    let packaged_path = unescape_delimiters_internal(packaged_path);

    (package_path.to_string(), packaged_path)
}

/// Split package-relative path into a (package path, packaged path) pair.
///
/// If `path` contains nested package-relative paths, the package path will be
/// the outermost package-relative path, and the packaged path will be the
/// innermost packaged path.
///
/// Returns `(path, "")` if the path is not a valid package-relative path.
/// This matches C++ `ArSplitPackageRelativePathInner`.
///
/// # Examples
///
/// ```
/// use usd_ar::package_utils::split_package_relative_path_inner;
///
/// assert_eq!(
///     split_package_relative_path_inner("a.pack[b.pack]"),
///     ("a.pack".to_string(), "b.pack".to_string())
/// );
/// assert_eq!(
///     split_package_relative_path_inner("a.pack[b.pack[c.pack]]"),
///     ("a.pack[b.pack]".to_string(), "c.pack".to_string())
/// );
/// assert_eq!(
///     split_package_relative_path_inner("not_package_path"),
///     ("not_package_path".to_string(), String::new())
/// );
/// ```
#[must_use]
pub fn split_package_relative_path_inner(path: &str) -> (String, String) {
    // Find innermost closing `]`
    let Some(innermost_close) = find_innermost_closing_delimiter(path) else {
        return (path.to_string(), String::new());
    };

    // Find matching opening `[`
    let Some(innermost_open) = find_matching_opening_delimiter(path, innermost_close) else {
        return (path.to_string(), String::new());
    };

    // Package path = original path with [innermost_open..=innermost_close] erased
    let mut package_path = path.to_string();
    package_path.replace_range(innermost_open..=innermost_close, "");

    // Packaged path = between `[` and `]`, with unescaping
    let packaged_path = &path[innermost_open + 1..innermost_close];
    let packaged_path = unescape_delimiters_internal(packaged_path);

    (package_path, packaged_path)
}

/// Escapes package path delimiters in a string using backslashes.
///
/// Replaces `[` with `\[` and `]` with `\]` so that the string can be
/// safely embedded in package-relative paths.
///
/// If the path is already package-relative, only the package portion is
/// escaped (the packaged portion is assumed already escaped).
///
/// Matches C++ `_EscapeDelimiters` behavior.
///
/// # Examples
///
/// ```
/// use usd_ar::package_utils::escape_package_delimiter;
///
/// // Brackets not at the end are escaped
/// assert_eq!(escape_package_delimiter("a[b]c"), "a\\[b\\]c");
/// // Trailing package-relative path brackets are preserved
/// assert_eq!(escape_package_delimiter("a[b]"), "a[b]");
/// assert_eq!(escape_package_delimiter("no_delimiters"), "no_delimiters");
/// ```
#[must_use]
pub fn escape_package_delimiter(path: &str) -> String {
    escape_delimiters_internal(path)
}

/// Unescapes package path delimiters in a string.
///
/// Replaces `\[` with `[` and `\]` with `]`, restoring the original path.
///
/// Matches C++ `_UnescapeDelimiters` behavior.
///
/// # Examples
///
/// ```
/// use usd_ar::package_utils::unescape_package_delimiter;
///
/// assert_eq!(unescape_package_delimiter("a\\[b\\]"), "a[b]");
/// assert_eq!(unescape_package_delimiter("no_delimiters"), "no_delimiters");
/// ```
#[must_use]
pub fn unescape_package_delimiter(path: &str) -> String {
    unescape_delimiters_internal(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_package_relative_path() {
        assert!(is_package_relative_path("a.usdz[b.usd]"));
        assert!(is_package_relative_path("Model.package[Geom.file]"));
        assert!(is_package_relative_path(
            "/path/to/Model.package[a/b/Geom.file]"
        ));
        assert!(is_package_relative_path("a.pack[b.pack[c.pack]]"));

        assert!(!is_package_relative_path(""));
        assert!(!is_package_relative_path("Model.usd"));
        assert!(!is_package_relative_path("a.pack["));
        // In C++, "[b.usd]" IS package-relative (empty package path + packaged "b.usd")
        assert!(is_package_relative_path("[b.usd]"));

        // Escaped delimiter should NOT count as closing
        assert!(!is_package_relative_path("a.pack\\]"));
    }

    #[test]
    fn test_join_package_relative_path_empty() {
        assert_eq!(join_package_relative_path(&[]), "");
        assert_eq!(join_package_relative_path(&["a.pack"]), "a.pack");
    }

    #[test]
    fn test_join_package_relative_path_two() {
        assert_eq!(
            join_package_relative_path(&["a.pack", "b.pack"]),
            "a.pack[b.pack]"
        );
    }

    #[test]
    fn test_join_package_relative_path_three() {
        assert_eq!(
            join_package_relative_path(&["a.pack", "b.pack", "c.pack"]),
            "a.pack[b.pack[c.pack]]"
        );
    }

    #[test]
    fn test_join_package_relative_path_nested() {
        assert_eq!(
            join_package_relative_path(&["a.pack[b.pack]", "c.pack"]),
            "a.pack[b.pack[c.pack]]"
        );
    }

    #[test]
    fn test_join_package_relative_path_with_delimiters() {
        // A path containing brackets should get escaped when placed inside
        assert_eq!(
            join_package_relative_path(&["outer.pack", "inner[x].file"]),
            "outer.pack[inner\\[x\\].file]"
        );
    }

    #[test]
    fn test_join_package_relative_path_pair() {
        assert_eq!(
            join_package_relative_path_pair("a.pack", "b.file"),
            "a.pack[b.file]"
        );
        assert_eq!(join_package_relative_path_pair("", "b.file"), "b.file");
        assert_eq!(join_package_relative_path_pair("a.pack", ""), "a.pack");
    }

    #[test]
    fn test_split_package_relative_path_outer_simple() {
        let (pkg, inner) = split_package_relative_path_outer("a.pack[b.pack]");
        assert_eq!(pkg, "a.pack");
        assert_eq!(inner, "b.pack");
    }

    #[test]
    fn test_split_package_relative_path_outer_nested() {
        let (pkg, inner) = split_package_relative_path_outer("a.pack[b.pack[c.pack]]");
        assert_eq!(pkg, "a.pack");
        assert_eq!(inner, "b.pack[c.pack]");
    }

    #[test]
    fn test_split_package_relative_path_outer_not_package() {
        // C++ returns (path, "") for non-package paths
        let (pkg, inner) = split_package_relative_path_outer("not_package_path");
        assert_eq!(pkg, "not_package_path");
        assert_eq!(inner, "");
    }

    #[test]
    fn test_split_package_relative_path_inner_simple() {
        let (pkg, inner) = split_package_relative_path_inner("a.pack[b.pack]");
        assert_eq!(pkg, "a.pack");
        assert_eq!(inner, "b.pack");
    }

    #[test]
    fn test_split_package_relative_path_inner_nested() {
        let (pkg, inner) = split_package_relative_path_inner("a.pack[b.pack[c.pack]]");
        assert_eq!(pkg, "a.pack[b.pack]");
        assert_eq!(inner, "c.pack");
    }

    #[test]
    fn test_split_package_relative_path_inner_not_package() {
        let (pkg, inner) = split_package_relative_path_inner("not_package_path");
        assert_eq!(pkg, "not_package_path");
        assert_eq!(inner, "");
    }

    #[test]
    fn test_roundtrip() {
        // Split and join should roundtrip
        let original = "a.pack[b.pack[c.pack]]";
        let (pkg, inner) = split_package_relative_path_outer(original);
        let rejoined = join_package_relative_path_pair(&pkg, &inner);
        assert_eq!(rejoined, original);
    }

    #[test]
    fn test_deeply_nested() {
        let path = "a[b[c[d[e]]]]";
        assert!(is_package_relative_path(path));

        let (outer_pkg, outer_inner) = split_package_relative_path_outer(path);
        assert_eq!(outer_pkg, "a");
        assert_eq!(outer_inner, "b[c[d[e]]]");

        let (inner_pkg, inner_inner) = split_package_relative_path_inner(path);
        assert_eq!(inner_pkg, "a[b[c[d]]]");
        assert_eq!(inner_inner, "e");
    }

    #[test]
    fn test_escape_package_delimiter() {
        // "a[b]" is already package-relative, so only "a" (the package part)
        // is escaped — and "a" has no delimiters, so result is unchanged.
        assert_eq!(escape_package_delimiter("a[b]"), "a[b]");
        // Non-package-relative path: brackets get escaped
        assert_eq!(escape_package_delimiter("a[b"), "a\\[b");
        assert_eq!(escape_package_delimiter("no_brackets"), "no_brackets");
        assert_eq!(escape_package_delimiter(""), "");
    }

    #[test]
    fn test_unescape_package_delimiter() {
        assert_eq!(unescape_package_delimiter("a\\[b\\]"), "a[b]");
        assert_eq!(unescape_package_delimiter("no_brackets"), "no_brackets");
        assert_eq!(unescape_package_delimiter(""), "");
    }

    #[test]
    fn test_escape_unescape_roundtrip() {
        let original = "file[with]brackets";
        let escaped = escape_package_delimiter(original);
        assert_eq!(escaped, "file\\[with\\]brackets");
        let unescaped = unescape_package_delimiter(&escaped);
        assert_eq!(unescaped, original);
    }

    #[test]
    fn test_split_with_escaped_delimiters() {
        // Join a path that has brackets, then split it back
        let joined = join_package_relative_path(&["outer.pack", "inner[x].file"]);
        assert_eq!(joined, "outer.pack[inner\\[x\\].file]");

        let (pkg, inner) = split_package_relative_path_outer(&joined);
        assert_eq!(pkg, "outer.pack");
        assert_eq!(inner, "inner[x].file"); // Unescaped on split
    }

    #[test]
    fn test_join_skips_empty() {
        assert_eq!(
            join_package_relative_path(&["", "a.pack", "", "b.pack", ""]),
            "a.pack[b.pack]"
        );
    }
}
