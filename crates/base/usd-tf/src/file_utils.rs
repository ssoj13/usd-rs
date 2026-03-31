#![allow(unsafe_code)]
//! File utilities for USD.
//!
//! This module provides various file system operations including
//! existence checks, directory operations, and file manipulation.
//!
//! # Examples
//!
//! ```
//! use usd_tf::file_utils::*;
//!
//! // Check if path exists
//! let exists = path_exists("/tmp");
//!
//! // Check if path is a directory
//! let is_dir = is_dir("/tmp");
//! ```

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

/// Returns true if the path exists.
///
/// If `resolve_symlinks` is false (default), uses lstat-like behavior.
/// If `resolve_symlinks` is true, follows symbolic links.
///
/// # Examples
///
/// ```
/// use usd_tf::file_utils::path_exists;
///
/// // Check if current directory exists
/// assert!(path_exists("."));
/// ```
#[must_use]
pub fn path_exists(path: impl AsRef<Path>) -> bool {
    path_exists_impl(path.as_ref(), false)
}

/// Returns true if the path exists, optionally resolving symlinks.
#[must_use]
pub fn path_exists_resolve(path: impl AsRef<Path>, resolve_symlinks: bool) -> bool {
    path_exists_impl(path.as_ref(), resolve_symlinks)
}

fn path_exists_impl(path: &Path, resolve_symlinks: bool) -> bool {
    if resolve_symlinks {
        // Follow symlinks - use metadata()
        fs::metadata(path).is_ok()
    } else {
        // Don't follow symlinks - use symlink_metadata()
        fs::symlink_metadata(path).is_ok()
    }
}

/// Returns true if the path exists and is a directory.
///
/// # Examples
///
/// ```
/// use usd_tf::file_utils::is_dir;
///
/// assert!(is_dir("."));
/// ```
#[must_use]
pub fn is_dir(path: impl AsRef<Path>) -> bool {
    is_dir_impl(path.as_ref(), false)
}

/// Returns true if the path is a directory, optionally resolving symlinks.
#[must_use]
pub fn is_dir_resolve(path: impl AsRef<Path>, resolve_symlinks: bool) -> bool {
    is_dir_impl(path.as_ref(), resolve_symlinks)
}

fn is_dir_impl(path: &Path, resolve_symlinks: bool) -> bool {
    let metadata = if resolve_symlinks {
        fs::metadata(path)
    } else {
        fs::symlink_metadata(path)
    };

    metadata.map(|m| m.is_dir()).unwrap_or(false)
}

/// Returns true if the path exists and is a file.
///
/// # Examples
///
/// ```no_run
/// use usd_tf::file_utils::is_file;
///
/// assert!(is_file("/etc/passwd"));
/// ```
#[must_use]
pub fn is_file(path: impl AsRef<Path>) -> bool {
    is_file_impl(path.as_ref(), false)
}

/// Returns true if the path is a file, optionally resolving symlinks.
#[must_use]
pub fn is_file_resolve(path: impl AsRef<Path>, resolve_symlinks: bool) -> bool {
    is_file_impl(path.as_ref(), resolve_symlinks)
}

fn is_file_impl(path: &Path, resolve_symlinks: bool) -> bool {
    let metadata = if resolve_symlinks {
        fs::metadata(path)
    } else {
        fs::symlink_metadata(path)
    };

    metadata.map(|m| m.is_file()).unwrap_or(false)
}

/// Returns true if the path exists and is a symbolic link.
///
/// # Examples
///
/// ```
/// use usd_tf::file_utils::is_link;
///
/// // Regular files are not links
/// assert!(!is_link("."));
/// ```
#[must_use]
pub fn is_link(path: impl AsRef<Path>) -> bool {
    fs::symlink_metadata(path.as_ref())
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
}

/// Returns true if the file or directory at path is writable.
///
/// # Examples
///
/// ```
/// use usd_tf::file_utils::is_writable;
///
/// // Current directory should be writable
/// assert!(is_writable("."));
/// ```
#[must_use]
pub fn is_writable(path: impl AsRef<Path>) -> bool {
    let path = path.as_ref();

    // Try to get metadata (follows symlinks)
    let metadata = match fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return false,
    };

    // On Unix, check write permission
    #[cfg(unix)]
    {
        let mode = metadata.mode();
        let uid = unsafe { libc::getuid() };
        let gid = unsafe { libc::getgid() };
        let file_uid = metadata.uid();
        let file_gid = metadata.gid();

        // Check user write permission
        if uid == file_uid && (mode & 0o200) != 0 {
            return true;
        }
        // Check group write permission
        if gid == file_gid && (mode & 0o020) != 0 {
            return true;
        }
        // Check other write permission
        if (mode & 0o002) != 0 {
            return true;
        }

        false
    }

    #[cfg(windows)]
    {
        // On Windows, check readonly attribute
        !metadata.permissions().readonly()
    }
}

/// Returns true if the path is an empty directory.
///
/// # Examples
///
/// ```no_run
/// use usd_tf::file_utils::is_dir_empty;
///
/// // Check if a directory is empty
/// let empty = is_dir_empty("/tmp/empty_dir");
/// ```
#[must_use]
pub fn is_dir_empty(path: impl AsRef<Path>) -> bool {
    match fs::read_dir(path.as_ref()) {
        Ok(mut entries) => entries.next().is_none(),
        Err(_) => false,
    }
}

/// Creates a symbolic link from `src` to `dst`.
///
/// Returns true on success.
#[cfg(unix)]
pub fn symlink(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> bool {
    std::os::unix::fs::symlink(src.as_ref(), dst.as_ref()).is_ok()
}

/// Creates a symbolic link from `src` to `dst`.
///
/// Returns true on success.
#[cfg(windows)]
pub fn symlink(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> bool {
    let src = src.as_ref();
    let dst = dst.as_ref();

    // Determine if source is a directory
    if src.is_dir() {
        std::os::windows::fs::symlink_dir(src, dst).is_ok()
    } else {
        std::os::windows::fs::symlink_file(src, dst).is_ok()
    }
}

/// Deletes a file at path.
///
/// Returns true on success.
///
/// # Examples
///
/// ```no_run
/// use usd_tf::file_utils::delete_file;
///
/// delete_file("/tmp/test_file.txt");
/// ```
pub fn delete_file(path: impl AsRef<Path>) -> bool {
    fs::remove_file(path.as_ref()).is_ok()
}

/// Creates a directory.
///
/// Returns true on success. If the directory already exists, returns false.
///
/// # Examples
///
/// ```no_run
/// use usd_tf::file_utils::make_dir;
///
/// make_dir("/tmp/new_dir");
/// ```
pub fn make_dir(path: impl AsRef<Path>) -> bool {
    fs::create_dir(path.as_ref()).is_ok()
}

/// Creates a directory with the specified permissions (Unix only).
#[cfg(unix)]
pub fn make_dir_with_mode(path: impl AsRef<Path>, mode: u32) -> bool {
    use std::os::unix::fs::DirBuilderExt;

    fs::DirBuilder::new()
        .mode(mode)
        .create(path.as_ref())
        .is_ok()
}

/// Creates a directory hierarchy.
///
/// Creates all parent directories as needed.
/// Returns true on success. If `exist_ok` is true, returns true even if
/// the directory already exists.
///
/// # Examples
///
/// ```no_run
/// use usd_tf::file_utils::make_dirs;
///
/// make_dirs("/tmp/a/b/c", true);
/// ```
pub fn make_dirs(path: impl AsRef<Path>, exist_ok: bool) -> bool {
    let path = path.as_ref();

    match fs::create_dir_all(path) {
        Ok(_) => true,
        Err(e) => {
            if exist_ok && e.kind() == io::ErrorKind::AlreadyExists {
                path.is_dir()
            } else {
                false
            }
        }
    }
}

/// Recursively delete a directory tree rooted at path.
///
/// Returns true on success.
///
/// # Examples
///
/// ```no_run
/// use usd_tf::file_utils::rm_tree;
///
/// rm_tree("/tmp/dir_to_remove");
/// ```
pub fn rm_tree(path: impl AsRef<Path>) -> bool {
    fs::remove_dir_all(path.as_ref()).is_ok()
}

/// Return a list containing files and directories in path.
///
/// If `recursive` is true, includes all subdirectory contents.
/// Directories are returned with a trailing path separator.
///
/// # Examples
///
/// ```no_run
/// use usd_tf::file_utils::list_dir;
///
/// let entries = list_dir("/tmp", false);
/// ```
#[must_use]
pub fn list_dir(path: impl AsRef<Path>, recursive: bool) -> Vec<String> {
    let mut result = Vec::new();
    list_dir_impl(path.as_ref(), recursive, &mut result);
    result
}

fn list_dir_impl(path: &Path, recursive: bool, result: &mut Vec<String>) {
    let entries = match fs::read_dir(path) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let entry_path = entry.path();
        let mut name = entry_path.to_string_lossy().to_string();

        if entry_path.is_dir() {
            // Add trailing separator for directories
            if !name.ends_with(std::path::MAIN_SEPARATOR) {
                name.push(std::path::MAIN_SEPARATOR);
            }
            result.push(name);

            if recursive {
                list_dir_impl(&entry_path, recursive, result);
            }
        } else {
            result.push(name);
        }
    }
}

/// Read directory contents.
///
/// Reads the contents of `dir_path` and appends names to the provided vectors.
/// Returns Ok on success, Err with error message on failure.
///
/// # Examples
///
/// ```no_run
/// use usd_tf::file_utils::read_dir;
///
/// let mut dirs = Vec::new();
/// let mut files = Vec::new();
/// let mut links = Vec::new();
///
/// read_dir("/tmp", Some(&mut dirs), Some(&mut files), Some(&mut links)).ok();
/// ```
pub fn read_dir(
    dir_path: impl AsRef<Path>,
    mut dirnames: Option<&mut Vec<String>>,
    mut filenames: Option<&mut Vec<String>>,
    mut symlinknames: Option<&mut Vec<String>>,
) -> Result<(), String> {
    let dir_path = dir_path.as_ref();

    let entries = fs::read_dir(dir_path).map_err(|e| e.to_string())?;

    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        let file_type = entry.file_type().map_err(|e| e.to_string())?;

        if file_type.is_symlink() {
            if let Some(ref mut links) = symlinknames {
                links.push(name);
            }
        } else if file_type.is_dir() {
            if let Some(ref mut dirs) = dirnames {
                dirs.push(name);
            }
        } else if let Some(ref mut files) = filenames {
            files.push(name);
        }
    }

    Ok(())
}

/// Touch a file, updating access and modification time to 'now'.
///
/// If `create` is true and the file doesn't exist, creates an empty file.
/// Returns true on success.
///
/// # Examples
///
/// ```no_run
/// use usd_tf::file_utils::touch_file;
///
/// touch_file("/tmp/touched_file", true);
/// ```
/// Walk directory tree with a callback function.
///
/// This matches C++ `TfWalkDirs`. Calls the callback function for each directory
/// in the tree, allowing control over which subdirectories are visited.
///
/// # Arguments
///
/// * `top` - Root directory to start walking from
/// * `callback` - Function called for each directory: (dir_path, subdirs, files) -> bool
///   Return false to stop walking, true to continue
/// * `top_down` - If true, visit parent before children
/// * `on_error` - Optional error handler: (path, error_msg) -> ()
/// * `follow_links` - If true, follow symbolic links
///
/// # Examples
///
/// ```no_run
/// use usd_tf::file_utils::walk_dirs;
///
/// walk_dirs("/tmp", |dir, subdirs, files| {
///     println!("Directory: {}, subdirs: {}, files: {}", dir, subdirs.len(), files.len());
///     true // Continue walking
/// }, true, None, false);
/// ```
pub fn walk_dirs<F>(
    top: impl AsRef<Path>,
    mut callback: F,
    top_down: bool,
    on_error: Option<Box<dyn Fn(&str, &str)>>,
    follow_links: bool,
) where
    F: FnMut(&str, &mut Vec<String>, &[String]) -> bool,
{
    let top = top.as_ref();

    if !top.is_dir() {
        return;
    }

    fn walk_impl<F>(
        path: &Path,
        callback: &mut F,
        top_down: bool,
        on_error: &Option<Box<dyn Fn(&str, &str)>>,
        follow_links: bool,
        visited: &mut HashSet<PathBuf>,
    ) -> bool
    where
        F: FnMut(&str, &mut Vec<String>, &[String]) -> bool,
    {
        // Symlink loop detection: track canonical paths (Windows) or (dev,ino) encoded
        // as a PathBuf key (Unix uses a synthetic path from dev+ino to avoid canonicalize cost).
        #[cfg(unix)]
        let key: Option<PathBuf> = if follow_links {
            fs::metadata(path).ok().map(|m| {
                // Encode (dev, ino) as a synthetic path — cheap and unambiguous.
                PathBuf::from(format!("{}:{}", m.dev(), m.ino()))
            })
        } else {
            None
        };

        #[cfg(windows)]
        let key: Option<PathBuf> = if follow_links {
            fs::canonicalize(path).ok()
        } else {
            None
        };

        #[cfg(not(any(unix, windows)))]
        let key: Option<PathBuf> = None;

        if let Some(ref k) = key {
            if !visited.insert(k.clone()) {
                // Already visited — symlink cycle, skip.
                return true;
            }
        }

        let entries = match fs::read_dir(path) {
            Ok(e) => e,
            Err(e) => {
                if let Some(handler) = &on_error {
                    handler(path.to_string_lossy().as_ref(), &e.to_string());
                }
                return false;
            }
        };

        let mut subdirs = Vec::new();
        let mut files = Vec::new();

        for entry in entries.flatten() {
            let entry_path = entry.path();
            let name = entry_path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string());

            if let Some(name) = name {
                let metadata = if follow_links {
                    fs::metadata(&entry_path)
                } else {
                    fs::symlink_metadata(&entry_path)
                };

                match metadata {
                    Ok(md) => {
                        if md.is_dir() {
                            subdirs.push(name);
                        } else {
                            files.push(name);
                        }
                    }
                    Err(e) => {
                        if let Some(handler) = &on_error {
                            handler(entry_path.to_string_lossy().as_ref(), &e.to_string());
                        }
                    }
                }
            }
        }

        if top_down && !callback(path.to_string_lossy().as_ref(), &mut subdirs, &files) {
            return false;
        }

        // Visit subdirectories
        for subdir in &subdirs.clone() {
            let subdir_path = path.join(subdir);
            if !walk_impl(
                &subdir_path,
                callback,
                top_down,
                on_error,
                follow_links,
                visited,
            ) {
                return false;
            }
        }

        if !top_down && !callback(path.to_string_lossy().as_ref(), &mut subdirs, &files) {
            return false;
        }

        true
    }

    let mut visited = HashSet::new();
    walk_impl(
        top,
        &mut callback,
        top_down,
        &on_error,
        follow_links,
        &mut visited,
    );
}

/// Error handler that ignores all errors.
///
/// This matches C++ `TfWalkIgnoreErrorHandler`.
///
/// # Examples
///
/// ```no_run
/// use usd_tf::file_utils::{walk_dirs, walk_ignore_error_handler};
///
/// walk_dirs("/tmp", |dir, subdirs, files| true, true, Some(Box::new(walk_ignore_error_handler)), false);
/// ```
pub fn walk_ignore_error_handler(_path: &str, _msg: &str) {
    // Ignore all errors
}

/// Creates or updates the modification time of a file.
///
/// If the file exists, updates its modification time to now.
/// If create is true and the file doesn't exist, creates an empty file.
/// Returns true on success, false on failure.
pub fn touch_file(path: impl AsRef<Path>, create: bool) -> bool {
    let path = path.as_ref();

    if !path.exists() {
        if create {
            // Create empty file
            return fs::File::create(path).is_ok();
        } else {
            return false;
        }
    }

    // Update modification time using libc::utime on Unix
    #[cfg(unix)]
    {
        use std::ffi::CString;
        use std::ptr;

        // Use utime with null times to set to current time
        if let Ok(c_path) = CString::new(path.to_string_lossy().as_bytes()) {
            unsafe { libc::utime(c_path.as_ptr(), ptr::null()) == 0 }
        } else {
            false
        }
    }

    #[cfg(windows)]
    {
        // SetFileTime equivalent: open and call set_modified (stable since Rust 1.75)
        match fs::OpenOptions::new().write(true).open(path) {
            Ok(file) => file.set_modified(std::time::SystemTime::now()).is_ok(),
            Err(_) => false,
        }
    }

    #[cfg(not(any(unix, windows)))]
    {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_path_exists() {
        assert!(path_exists("."));
        assert!(!path_exists("/nonexistent_path_12345"));
    }

    #[test]
    fn test_is_dir() {
        assert!(is_dir("."));
        assert!(!is_dir("/nonexistent_path_12345"));
    }

    #[test]
    fn test_is_file() {
        // Cargo.toml should exist and be a file
        assert!(is_file("Cargo.toml"));
        assert!(!is_file("."));
    }

    #[test]
    fn test_is_link() {
        // Regular files/dirs are not links
        assert!(!is_link("."));
        assert!(!is_link("Cargo.toml"));
    }

    #[test]
    fn test_is_writable() {
        // Current directory should be writable
        assert!(is_writable("."));
    }

    #[test]
    fn test_make_and_delete_dir() {
        let temp_dir = env::temp_dir().join("usd_test_make_dir");

        // Clean up if exists
        let _ = fs::remove_dir_all(&temp_dir);

        // Create directory
        assert!(make_dir(&temp_dir));
        assert!(is_dir(&temp_dir));
        assert!(is_dir_empty(&temp_dir));

        // Delete directory
        assert!(fs::remove_dir(&temp_dir).is_ok());
        assert!(!path_exists(&temp_dir));
    }

    #[test]
    fn test_make_dirs() {
        let temp_dir = env::temp_dir().join("usd_test_make_dirs/a/b/c");

        // Clean up if exists
        let _ = fs::remove_dir_all(env::temp_dir().join("usd_test_make_dirs"));

        // Create directory hierarchy
        assert!(make_dirs(&temp_dir, false));
        assert!(is_dir(&temp_dir));

        // exist_ok should work
        assert!(make_dirs(&temp_dir, true));

        // Clean up
        let _ = fs::remove_dir_all(env::temp_dir().join("usd_test_make_dirs"));
    }

    #[test]
    fn test_list_dir() {
        let entries = list_dir(".", false);
        // Should have at least Cargo.toml and src/
        assert!(!entries.is_empty());
    }

    #[test]
    fn test_read_dir() {
        let mut dirs = Vec::new();
        let mut files = Vec::new();
        let mut links = Vec::new();

        let result = read_dir(".", Some(&mut dirs), Some(&mut files), Some(&mut links));
        assert!(result.is_ok());

        // Should have src/ directory
        assert!(dirs.iter().any(|d| d == "src"));
        // Should have Cargo.toml file
        assert!(files.iter().any(|f| f == "Cargo.toml"));
    }
}
