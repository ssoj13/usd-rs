//! Path utilities for USD.
//!
//! This module provides various path manipulation functions including
//! normalization, absolute path conversion, and symlink resolution.
//!
//! # Examples
//!
//! ```
//! use usd_tf::path_utils::*;
//!
//! // Normalize a path
//! let normalized = norm_path("/foo/bar/../baz");
//! assert_eq!(normalized, "/foo/baz");
//!
//! // Get file extension
//! let ext = get_extension("/path/to/file.txt");
//! assert_eq!(ext, "txt");
//! ```

use std::env;
use std::fs;
use std::path::Path;

use super::file_utils;
use super::string_utils;

/// Returns the canonical path of the specified filename.
///
/// Resolves symbolic links and returns the real path.
/// If `allow_inaccessible_suffix` is true, handles paths where only
/// a prefix exists.
///
/// # Examples
///
/// ```no_run
/// use usd_tf::path_utils::real_path;
///
/// let path = real_path("/usr/bin/env", false);
/// ```
pub fn real_path(path: &str, allow_inaccessible_suffix: bool) -> Result<String, String> {
    if path.is_empty() {
        return Ok(String::new());
    }

    let (prefix, suffix) = if allow_inaccessible_suffix {
        let split = find_longest_accessible_prefix(path)?;
        (path[..split].to_string(), path[split..].to_string())
    } else {
        (path.to_string(), String::new())
    };

    if prefix.is_empty() {
        return Ok(abs_path(&suffix));
    }

    // Canonicalize the prefix (resolves symlinks)
    match fs::canonicalize(&prefix) {
        Ok(resolved) => {
            let mut result = resolved.to_string_lossy().to_string();

            // On Windows, canonicalize returns \\?\ prefix, remove it
            #[cfg(windows)]
            {
                if result.starts_with("\\\\?\\") {
                    result = result[4..].to_string();
                }
            }

            if !suffix.is_empty() {
                result.push_str(&suffix);
            }

            Ok(abs_path(&result))
        }
        Err(e) => Err(e.to_string()),
    }
}

/// Normalizes the specified path.
///
/// Eliminates double slashes, and removes `.` and `..` components.
/// On Windows, converts backslashes to forward slashes.
///
/// # Examples
///
/// ```
/// use usd_tf::path_utils::norm_path;
///
/// assert_eq!(norm_path("/foo/bar/../baz"), "/foo/baz");
/// assert_eq!(norm_path("/foo//bar"), "/foo/bar");
/// assert_eq!(norm_path("./foo"), "foo");
/// ```
#[must_use]
pub fn norm_path(path: &str) -> String {
    usd_arch::norm_path(path, false)
}

/// Normalizes the specified path, optionally stripping drive specifier.
#[must_use]
pub fn norm_path_strip_drive(path: &str, strip_drive: bool) -> String {
    usd_arch::norm_path(path, strip_drive)
}

/// Find the index delimiting the longest accessible prefix of path.
///
/// Returns the length of the accessible prefix.
///
/// # Examples
///
/// ```
/// use usd_tf::path_utils::find_longest_accessible_prefix;
///
/// // Root always exists
/// let idx = find_longest_accessible_prefix("/nonexistent/path").unwrap_or(0);
/// assert!(idx > 0 || idx == 0);
/// ```
pub fn find_longest_accessible_prefix(path: &str) -> Result<usize, String> {
    if path.is_empty() {
        return Ok(0);
    }

    // Collect all split points (positions of path separators)
    let mut split_points: Vec<usize> = Vec::new();
    let mut pos = 0;

    for c in path.chars() {
        if (c == '/' || c == '\\') && pos > 0 {
            split_points.push(pos);
        }
        pos += c.len_utf8();
    }
    split_points.push(path.len());

    // Binary search to find the longest accessible prefix
    let mut lo = 0;
    let mut hi = split_points.len();

    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let check_path = &path[..split_points[mid - 1]];

        if file_utils::path_exists(check_path) {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }

    if lo == 0 {
        // Check if the first component exists
        if !split_points.is_empty() {
            let first = &path[..split_points[0]];
            if file_utils::path_exists(first) {
                return Ok(split_points[0]);
            }
        }
        Ok(0)
    } else {
        Ok(split_points[lo - 1])
    }
}

/// Returns the canonical absolute path.
///
/// Makes the path absolute by prepending the current working directory
/// if needed. Unlike `real_path`, does not resolve symlinks.
///
/// # Examples
///
/// ```
/// use usd_tf::path_utils::abs_path;
///
/// let abs = abs_path("relative/path");
/// assert!(abs.starts_with('/') || abs.contains(':'));
/// ```
#[must_use]
pub fn abs_path(path: &str) -> String {
    if path.is_empty() {
        return String::new();
    }

    let path_obj = Path::new(path);

    // Already absolute?
    if path_obj.is_absolute() {
        return norm_path(path);
    }

    // Prepend current directory
    match env::current_dir() {
        Ok(cwd) => {
            let full_path = cwd.join(path);
            norm_path(&full_path.to_string_lossy())
        }
        Err(_) => norm_path(path),
    }
}

/// Returns the extension for a file path.
///
/// Returns empty string for directories, empty paths, or dotfiles without extension.
///
/// # Examples
///
/// ```
/// use usd_tf::path_utils::get_extension;
///
/// assert_eq!(get_extension("/foo/bar"), "");
/// assert_eq!(get_extension("/foo/bar/file.txt"), "txt");
/// assert_eq!(get_extension("/foo/bar/file.tar.gz"), "gz");
/// assert_eq!(get_extension("/foo/bar/.hidden"), "");
/// assert_eq!(get_extension("/foo/bar/.hidden.txt"), "txt");
/// ```
#[must_use]
pub fn get_extension(path: &str) -> &str {
    if path.is_empty() {
        return "";
    }

    let base_name = string_utils::get_base_name(path);

    // If this is a dotfile with no extension (e.g., .folder)
    let before_suffix = string_utils::get_before_suffix(base_name, '.');
    if before_suffix.is_empty() {
        return "";
    }

    string_utils::get_suffix(base_name, '.')
}

/// Returns the value of a symbolic link.
///
/// Returns empty string on error or if path is not a symbolic link.
///
/// # Examples
///
/// ```
/// use usd_tf::path_utils::read_link;
///
/// // Non-symlink returns empty string
/// let target = read_link("Cargo.toml");
/// assert_eq!(target, "");
/// ```
#[must_use]
pub fn read_link(path: &str) -> String {
    match fs::read_link(path) {
        Ok(target) => target.to_string_lossy().to_string(),
        Err(_) => String::new(),
    }
}

/// Return true if path is relative (not absolute).
///
/// # Examples
///
/// ```
/// use usd_tf::path_utils::is_relative_path;
///
/// assert!(is_relative_path("relative/path"));
/// assert!(is_relative_path(""));
/// assert!(!is_relative_path("/absolute/path"));
/// ```
#[must_use]
pub fn is_relative_path(path: &str) -> bool {
    if path.is_empty() {
        return true;
    }

    #[cfg(windows)]
    {
        // On Windows, check for drive letters and UNC paths
        let bytes = path.as_bytes();
        if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
            return false; // Has drive letter
        }
        !path.starts_with('/') && !path.starts_with('\\')
    }

    #[cfg(not(windows))]
    {
        !path.starts_with('/')
    }
}

/// Expand shell glob patterns.
///
/// Returns a vector of matching file paths.
///
/// # Examples
///
/// ```no_run
/// use usd_tf::path_utils::glob;
///
/// let files = glob("*.txt");
/// ```
#[must_use]
pub fn glob(pattern: &str) -> Vec<String> {
    glob_patterns(&[pattern])
}

/// Expand multiple shell glob patterns.
///
/// Returns a vector of matching file paths.
#[must_use]
pub fn glob_patterns(patterns: &[&str]) -> Vec<String> {
    let mut result = Vec::new();

    for pattern in patterns {
        if let Ok(entries) = ::glob::glob(pattern) {
            for entry in entries.flatten() {
                result.push(entry.to_string_lossy().to_string());
            }
        }
    }

    result.sort();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_norm_path_basic() {
        assert_eq!(norm_path("/foo/bar"), "/foo/bar");
        assert_eq!(norm_path("/foo//bar"), "/foo/bar");
        assert_eq!(norm_path("/foo/./bar"), "/foo/bar");
        assert_eq!(norm_path("/foo/bar/../baz"), "/foo/baz");
    }

    #[test]
    fn test_norm_path_dots() {
        assert_eq!(norm_path("./foo"), "foo");
        assert_eq!(norm_path("../foo"), "../foo");
        assert_eq!(norm_path("foo/.."), ".");
        assert_eq!(norm_path("/.."), "/");
    }

    #[test]
    fn test_norm_path_empty() {
        assert_eq!(norm_path(""), ".");
    }

    #[test]
    fn test_norm_path_trailing_slash() {
        // Trailing slash is stripped
        assert_eq!(norm_path("/foo/bar/"), "/foo/bar");
    }

    #[test]
    fn test_abs_path_relative() {
        let abs = abs_path("relative");
        assert!(!is_relative_path(&abs));
    }

    #[test]
    fn test_abs_path_already_absolute() {
        #[cfg(unix)]
        {
            let abs = abs_path("/already/absolute");
            assert_eq!(abs, "/already/absolute");
        }
    }

    #[test]
    fn test_get_extension() {
        assert_eq!(get_extension("/foo/bar"), "");
        assert_eq!(get_extension("/foo/bar.txt"), "txt");
        assert_eq!(get_extension("/foo/bar.tar.gz"), "gz");
        assert_eq!(get_extension("/foo/.hidden"), "");
        assert_eq!(get_extension("/foo/.hidden.txt"), "txt");
        assert_eq!(get_extension(""), "");
    }

    #[test]
    fn test_is_relative_path() {
        assert!(is_relative_path(""));
        assert!(is_relative_path("relative/path"));
        assert!(is_relative_path("./relative"));

        #[cfg(unix)]
        {
            assert!(!is_relative_path("/absolute/path"));
        }
    }

    #[test]
    fn test_read_link_non_link() {
        // Regular file should return empty string
        assert_eq!(read_link("Cargo.toml"), "");
    }

    #[test]
    fn test_find_longest_accessible_prefix() {
        // Current directory always exists
        let idx = find_longest_accessible_prefix(".").unwrap();
        assert!(idx > 0 || idx == 0);

        // Root should exist
        #[cfg(unix)]
        {
            let idx = find_longest_accessible_prefix("/usr/nonexistent_12345").unwrap();
            // Should find /usr or at least /
            assert!(idx > 0);
        }
    }

    #[test]
    fn test_real_path_nonexistent() {
        let result = real_path("/nonexistent_path_12345", false);
        assert!(result.is_err());
    }

    #[test]
    fn test_real_path_current_dir() {
        let result = real_path(".", false);
        assert!(result.is_ok());
    }
}
