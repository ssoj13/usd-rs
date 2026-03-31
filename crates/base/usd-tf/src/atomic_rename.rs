//! Atomic file rename utilities.
//!
//! Provides functions for safely renaming files atomically and creating
//! temporary sibling files for safe writing operations.
//!
//! # Examples
//!
//! ```no_run
//! use usd_tf::atomic_rename::{atomic_rename_file_over, create_sibling_temp_file};
//!
//! // Create a temp file next to target
//! let result = create_sibling_temp_file("/path/to/target.txt");
//! if let Ok((temp_path, real_path)) = result {
//!     // Write to temp_path...
//!     // Then atomically rename
//!     atomic_rename_file_over(&temp_path, &real_path).unwrap();
//! }
//! ```

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::path_utils::real_path as tf_real_path;

/// Environment setting for requiring filesystem write permission checks.
///
/// On Windows, this defaults to false because older networked filesystems
/// have reported incorrect file permissions. On Unix, defaults to true.
#[cfg(windows)]
const DEFAULT_REQUIRE_WRITE_PERMISSION: bool = false;
#[cfg(not(windows))]
const DEFAULT_REQUIRE_WRITE_PERMISSION: bool = true;

/// Atomically rename `src_file` over `dst_file`.
///
/// Both files must be on the same filesystem for this to work reliably.
/// On success, returns `Ok(())`. On failure, returns an error describing
/// what went wrong.
///
/// # Platform Behavior
///
/// - On Unix: Uses `std::fs::rename` after optionally setting permissions.
/// - On Windows: Uses `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING`.
///
/// # Examples
///
/// ```no_run
/// use usd_tf::atomic_rename::atomic_rename_file_over;
///
/// atomic_rename_file_over("/tmp/temp_file.txt", "/home/user/target.txt")
///     .expect("Failed to rename");
/// ```
pub fn atomic_rename_file_over<P: AsRef<Path>, Q: AsRef<Path>>(
    src_file: P,
    dst_file: Q,
) -> io::Result<()> {
    let src = src_file.as_ref();
    let dst = dst_file.as_ref();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        // On Unix, try to match permissions of existing file or use default
        let mode = if dst.exists() {
            let meta = fs::metadata(dst)?;
            meta.permissions().mode() & 0o666
        } else {
            // Use default file mode (0666 & ~umask)
            0o666
        };

        // Try to set permissions on source file (ignore failure)
        let perms = fs::Permissions::from_mode(mode);
        let _ = fs::set_permissions(src, perms);

        // Perform atomic rename
        fs::rename(src, dst)
    }

    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;

        #[allow(unsafe_code)]
        unsafe extern "system" {
            fn MoveFileExW(
                lpExistingFileName: *const u16,
                lpNewFileName: *const u16,
                dwFlags: u32,
            ) -> i32;
        }

        // MOVEFILE_REPLACE_EXISTING: replace dst if it exists (like Unix rename)
        // MOVEFILE_COPY_ALLOWED: allow cross-volume move via copy+delete
        const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
        const MOVEFILE_COPY_ALLOWED: u32 = 0x2;

        let src_wide: Vec<u16> = src.as_os_str().encode_wide().chain(Some(0)).collect();
        let dst_wide: Vec<u16> = dst.as_os_str().encode_wide().chain(Some(0)).collect();

        // SAFETY: FFI call to Win32 MoveFileExW with valid null-terminated wide strings
        #[allow(unsafe_code)]
        let result = unsafe {
            MoveFileExW(
                src_wide.as_ptr(),
                dst_wide.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_COPY_ALLOWED,
            )
        };

        if result != 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
}

/// Create a temporary sibling file for the given target file path.
///
/// Returns a tuple of (temp_file_path, real_target_path) on success.
/// The temp file is created in the same directory as the target to ensure
/// atomic rename is possible.
///
/// # Arguments
///
/// * `file_name` - Path to the target file (may not exist yet)
///
/// # Returns
///
/// * `Ok((temp_path, real_path))` - Temp file path and resolved real path of target
/// * `Err(error)` - If temp file creation fails
///
/// # Examples
///
/// ```no_run
/// use usd_tf::atomic_rename::create_sibling_temp_file;
///
/// let (temp_path, real_path) = create_sibling_temp_file("/home/user/file.txt")?;
/// // Write content to temp_path...
/// # Ok::<(), std::io::Error>(())
/// ```
pub fn create_sibling_temp_file<P: AsRef<Path>>(file_name: P) -> io::Result<(PathBuf, PathBuf)> {
    let file_name = file_name.as_ref();

    if file_name.as_os_str().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Empty file name",
        ));
    }

    // Get the real path (resolving symlinks)
    let file_name_str = file_name.to_str().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "Path contains invalid UTF-8")
    })?;

    let real_file_path = tf_real_path(file_name_str, true)
        .map(PathBuf::from)
        .map_err(|e| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "Unable to determine real path for '{}': {}",
                    file_name.display(),
                    e
                ),
            )
        })?;

    // Get parent directory
    let dir_path = real_file_path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "No parent directory"))?;

    // Check write permissions if required
    if should_check_write_permission() {
        check_directory_writable(dir_path)?;
        check_file_writable(&real_file_path)?;
    }

    // Create temp file with prefix from target file name
    let prefix = real_file_path
        .file_stem()
        .and_then(|s: &std::ffi::OsStr| s.to_str())
        .unwrap_or("tmp");

    let temp_path = create_temp_file(dir_path, prefix)?;

    Ok((temp_path, real_file_path))
}

/// Check if write permission checks are enabled.
fn should_check_write_permission() -> bool {
    std::env::var("TF_REQUIRE_FILESYSTEM_WRITE_PERMISSION")
        .ok()
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(DEFAULT_REQUIRE_WRITE_PERMISSION)
}

/// Check if directory is writable.
fn check_directory_writable(dir: &Path) -> io::Result<()> {
    let meta = fs::metadata(dir)?;
    if !meta.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotADirectory,
            format!("'{}' is not a directory", dir.display()),
        ));
    }

    // On Unix, check write permission bit
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = meta.permissions().mode();
        if mode & 0o200 == 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "Insufficient permissions to write to directory '{}'",
                    dir.display()
                ),
            ));
        }
    }

    Ok(())
}

/// Check if file is writable (if it exists).
fn check_file_writable(file: &Path) -> io::Result<()> {
    match fs::metadata(file) {
        Ok(meta) => {
            // On Unix, check write permission
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = meta.permissions().mode();
                if mode & 0o200 == 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        format!(
                            "Insufficient permissions to write to file '{}'",
                            file.display()
                        ),
                    ));
                }
            }

            // On Windows, check readonly attribute
            #[cfg(windows)]
            {
                if meta.permissions().readonly() {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        format!("File '{}' is read-only", file.display()),
                    ));
                }
            }

            Ok(())
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()), // File doesn't exist yet
        Err(e) => Err(e),
    }
}

/// Create a temporary file in the given directory with the given prefix.
fn create_temp_file(dir: &Path, prefix: &str) -> io::Result<PathBuf> {
    use std::time::{SystemTime, UNIX_EPOCH};

    // Generate unique suffix
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);

    let pid = std::process::id();

    for attempt in 0..100 {
        let name = format!("{}.{}.{}.{}.tmp", prefix, pid, timestamp, attempt);
        let path = dir.join(&name);

        // Try to create the file exclusively
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(_) => return Ok(path),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }

    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "Failed to create unique temporary file",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_atomic_rename() {
        let temp_dir = std::env::temp_dir();
        let src = temp_dir.join("atomic_rename_test_src.txt");
        let dst = temp_dir.join("atomic_rename_test_dst.txt");

        // Cleanup
        let _ = fs::remove_file(&src);
        let _ = fs::remove_file(&dst);

        // Create source file with content
        {
            let mut f = fs::File::create(&src).unwrap();
            f.write_all(b"test content").unwrap();
        }

        // Rename
        atomic_rename_file_over(&src, &dst).unwrap();

        // Verify
        assert!(!src.exists());
        assert!(dst.exists());
        let content = fs::read_to_string(&dst).unwrap();
        assert_eq!(content, "test content");

        // Cleanup
        let _ = fs::remove_file(&dst);
    }

    #[test]
    fn test_atomic_rename_replace_existing() {
        let temp_dir = std::env::temp_dir();
        let src = temp_dir.join("atomic_rename_replace_src.txt");
        let dst = temp_dir.join("atomic_rename_replace_dst.txt");

        // Cleanup
        let _ = fs::remove_file(&src);
        let _ = fs::remove_file(&dst);

        // Create destination with old content
        {
            let mut f = fs::File::create(&dst).unwrap();
            f.write_all(b"old content").unwrap();
        }

        // Create source with new content
        {
            let mut f = fs::File::create(&src).unwrap();
            f.write_all(b"new content").unwrap();
        }

        // Rename (should replace)
        atomic_rename_file_over(&src, &dst).unwrap();

        // Verify
        assert!(!src.exists());
        let content = fs::read_to_string(&dst).unwrap();
        assert_eq!(content, "new content");

        // Cleanup
        let _ = fs::remove_file(&dst);
    }

    #[test]
    fn test_create_sibling_temp_file() {
        let temp_dir = std::env::temp_dir();
        let target = temp_dir.join("sibling_test_target.txt");

        let result = create_sibling_temp_file(&target);
        assert!(result.is_ok());

        let (temp_path, real_path) = result.unwrap();

        // Temp file should exist and be in same directory
        assert!(temp_path.exists());
        assert_eq!(temp_path.parent(), real_path.parent());

        // Cleanup
        let _ = fs::remove_file(&temp_path);
    }

    #[test]
    fn test_create_sibling_temp_file_empty_name() {
        let result = create_sibling_temp_file("");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn test_create_temp_file() {
        let temp_dir = std::env::temp_dir();
        let result = create_temp_file(&temp_dir, "test_prefix");

        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.exists());
        assert!(
            path.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .contains("test_prefix")
        );

        // Cleanup
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_check_directory_writable() {
        let temp_dir = std::env::temp_dir();
        assert!(check_directory_writable(&temp_dir).is_ok());
    }

    #[test]
    fn test_check_file_writable_nonexistent() {
        let path = std::env::temp_dir().join("nonexistent_file_check.txt");
        // Non-existent file should return Ok (can be created)
        assert!(check_file_writable(&path).is_ok());
    }
}
