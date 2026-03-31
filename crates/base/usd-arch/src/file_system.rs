// SAFETY: This module provides FFI bindings to system APIs requiring unsafe
#![allow(unsafe_code)]

//! File system operations with cross-platform support.
//!
//! Provides platform-independent file operations including:
//! - File opening with flexible sharing modes
//! - Positional read/write (pread/pwrite)
//! - Memory-mapped file I/O
//! - File access hints for optimization
//! - File metadata queries
//!
//! # Platform Support
//!
//! - Windows: Uses Win32 API for proper file sharing and UTF-8 handling
//! - Linux/Unix: Uses POSIX APIs with nanosecond precision
//! - macOS: Darwin-specific optimizations
//!
//! # Examples
//!
//! ```no_run
//! use usd_arch::{open_file, get_file_len, map_file_ro};
//! use std::io::Write;
//!
//! // Open file with proper cross-platform handling
//! let mut file = open_file("test.txt", "w").unwrap();
//! file.write_all(b"Hello, USD!").unwrap();
//!
//! // Memory map for fast read access
//! let mapping = map_file_ro("test.txt", None).unwrap();
//! assert_eq!(&mapping[..], b"Hello, USD!");
//! ```

use std::fs::{File, OpenOptions};
#[cfg(unix)]
use std::io::ErrorKind;
use std::io::{Error, Result};
use std::path::Path;
use std::time::SystemTime;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[cfg(windows)]
use std::os::windows::fs::OpenOptionsExt;

use memmap2::{Mmap, MmapMut, MmapOptions};

/// Platform-specific path separator.
///
/// `\` on Windows, `/` on Unix.
#[cfg(windows)]
pub const PATH_SEP: &str = "\\";

/// Platform-specific path separator.
///
/// `/` on Unix, `\` on Windows.
#[cfg(not(windows))]
pub const PATH_SEP: &str = "/";

/// Platform-specific path list separator (for environment variables like PATH).
///
/// `;` on Windows, `:` on Unix.
#[cfg(windows)]
pub const PATH_LIST_SEP: &str = ";";

/// Platform-specific path list separator (for environment variables like PATH).
///
/// `:` on Unix, `;` on Windows.
#[cfg(not(windows))]
pub const PATH_LIST_SEP: &str = ":";

/// Platform-specific relative path identifier.
///
/// `.\` on Windows, `./` on Unix.
#[cfg(windows)]
pub const REL_PATH_IDENT: &str = ".\\";

/// Platform-specific relative path identifier.
///
/// `./` on Unix, `.\` on Windows.
#[cfg(not(windows))]
pub const REL_PATH_IDENT: &str = "./";

/// Maximum path length for the current platform.
///
/// Typically 260 on Windows, 4096 on Linux, 1024 on macOS.
#[cfg(windows)]
pub const PATH_MAX: usize = 260;

/// Maximum path length for the current platform.
#[cfg(target_os = "linux")]
pub const PATH_MAX: usize = 4096;

/// Maximum path length for the current platform.
#[cfg(target_os = "macos")]
pub const PATH_MAX: usize = 1024;

/// Maximum path length for the current platform (fallback).
#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
pub const PATH_MAX: usize = 1024;

/// File access advice hints for optimization.
///
/// These hints inform the OS about expected file access patterns,
/// allowing it to optimize prefetching, caching, and resource management.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileAdvice {
    /// Normal access pattern (default).
    Normal,
    /// Data will be accessed soon - OS may prefetch.
    WillNeed,
    /// Data won't be needed soon - OS may free resources.
    DontNeed,
    /// Random access pattern - sequential prefetch may not help.
    RandomAccess,
}

/// Read-only memory-mapped file.
///
/// Provides zero-copy access to file contents. The mapping is automatically
/// unmapped when dropped (RAII).
pub struct ConstFileMapping {
    mmap: Mmap,
}

impl ConstFileMapping {
    /// Get the length of the mapped region in bytes.
    #[inline]
    pub fn len(&self) -> usize {
        self.mmap.len()
    }

    /// Check if the mapping is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.mmap.is_empty()
    }

    /// Get a slice to the mapped data.
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        &self.mmap[..]
    }
}

impl std::ops::Deref for ConstFileMapping {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.mmap[..]
    }
}

impl AsRef<[u8]> for ConstFileMapping {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        &self.mmap[..]
    }
}

/// Mutable (copy-on-write) memory-mapped file.
///
/// Modifications create private copies of affected pages (MAP_PRIVATE).
/// Changes are not written back to the underlying file.
pub struct MutableFileMapping {
    mmap: MmapMut,
}

impl MutableFileMapping {
    /// Get the length of the mapped region in bytes.
    #[inline]
    pub fn len(&self) -> usize {
        self.mmap.len()
    }

    /// Check if the mapping is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.mmap.is_empty()
    }

    /// Get a slice to the mapped data.
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        &self.mmap[..]
    }

    /// Get a mutable slice to the mapped data.
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.mmap[..]
    }
}

impl std::ops::Deref for MutableFileMapping {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.mmap[..]
    }
}

impl std::ops::DerefMut for MutableFileMapping {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.mmap[..]
    }
}

impl AsRef<[u8]> for MutableFileMapping {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        &self.mmap[..]
    }
}

impl AsMut<[u8]> for MutableFileMapping {
    #[inline]
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.mmap[..]
    }
}

/// Open a file with cross-platform mode string.
///
/// Mode string follows C stdio conventions:
/// - "r" or "rb" - read
/// - "w" or "wb" - write (truncate)
/// - "a" or "ab" - append
/// - "r+" or "rb+" - read/write
/// - "w+" or "wb+" - read/write (truncate)
/// - "a+" or "ab+" - read/append
///
/// On Windows, this enables proper file sharing (other processes can read/write/delete),
/// matching Unix-like behavior that USD code expects.
///
/// # Errors
///
/// Returns error if the file cannot be opened with the specified mode.
pub fn open_file<P: AsRef<Path>>(path: P, mode: &str) -> Result<File> {
    let path = path.as_ref();
    let mode = mode.as_bytes();

    let read = mode.contains(&b'r') || mode.contains(&b'+');
    let write = mode.contains(&b'w') || mode.contains(&b'a') || mode.contains(&b'+');
    let append = mode.contains(&b'a');
    let truncate = mode.contains(&b'w');
    let create = write;

    let mut opts = OpenOptions::new();
    opts.read(read)
        .write(write)
        .append(append)
        .create(create)
        .truncate(truncate);

    #[cfg(windows)]
    {
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
        };
        // Enable full sharing to match Unix behavior
        opts.share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE);
    }

    opts.open(path)
}

/// Check if the file or directory at the given path is writable.
///
/// On Unix, checks owner/group/other write permissions against effective uid/gid.
/// On Windows, checks the write bit in file attributes.
///
/// # Errors
///
/// Returns error if metadata cannot be retrieved.
pub fn stat_is_writable<P: AsRef<Path>>(path: P) -> Result<bool> {
    let metadata = std::fs::metadata(path)?;

    #[cfg(unix)]
    {
        let mode = metadata.permissions().mode();
        let uid = unsafe { libc::geteuid() };
        let gid = unsafe { libc::getegid() };

        // Check other write
        if mode & 0o002 != 0 {
            return Ok(true);
        }

        // Check group write
        {
            use std::os::unix::fs::MetadataExt;
            if gid == metadata.gid() && (mode & 0o020 != 0) {
                return Ok(true);
            }
        }

        // Check owner write
        {
            use std::os::unix::fs::MetadataExt;
            if uid == metadata.uid() && (mode & 0o200 != 0) {
                return Ok(true);
            }
        }

        // Not writable
        Ok(false)
    }

    #[cfg(windows)]
    {
        Ok(!metadata.permissions().readonly())
    }
}

/// Read from file at specified offset without changing file position.
///
/// This is a positional read operation that doesn't modify the file's
/// current seek position, making it safe for concurrent reads.
///
/// # Platform Notes
///
/// - Unix: Uses pread(2) system call
/// - Windows: Uses ReadFile with OVERLAPPED structure
///
/// # Errors
///
/// Returns error if read fails. Returns actual bytes read (may be less than requested).
pub fn pread(file: &File, buf: &mut [u8], offset: i64) -> Result<usize> {
    if buf.is_empty() {
        return Ok(0);
    }

    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;
        let fd = file.as_raw_fd();
        let count = buf.len();

        let mut total = 0usize;
        let mut remaining = count;
        let mut current_offset = offset;

        loop {
            let result = unsafe {
                libc::pread(
                    fd,
                    buf[total..].as_mut_ptr() as *mut libc::c_void,
                    remaining,
                    current_offset,
                )
            };

            if result == -1 {
                let err = Error::last_os_error();
                if err.kind() == ErrorKind::Interrupted {
                    continue;
                }
                return Err(err);
            }

            if result == 0 {
                // EOF
                return Ok(total);
            }

            let nread = result as usize;
            total += nread;
            remaining -= nread;
            current_offset += nread as i64;

            if remaining == 0 || nread == 0 {
                return Ok(total);
            }
        }
    }

    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Foundation::HANDLE;
        use windows_sys::Win32::Storage::FileSystem::ReadFile;
        use windows_sys::Win32::System::IO::OVERLAPPED;

        let handle = file.as_raw_handle() as HANDLE;

        let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
        let uoff = offset as u64;
        overlapped.Anonymous.Anonymous.Offset = (uoff & 0xFFFFFFFF) as u32;
        overlapped.Anonymous.Anonymous.OffsetHigh = (uoff >> 32) as u32;

        let mut bytes_read: u32 = 0;
        let result = unsafe {
            ReadFile(
                handle,
                buf.as_mut_ptr() as *mut _,
                buf.len() as u32,
                &mut bytes_read,
                &mut overlapped,
            )
        };

        if result == 0 {
            return Err(Error::last_os_error());
        }

        Ok(bytes_read as usize)
    }
}

/// Write to file at specified offset without changing file position.
///
/// This is a positional write operation that doesn't modify the file's
/// current seek position, making it safe for concurrent writes to different offsets.
///
/// # Platform Notes
///
/// - Unix: Uses pwrite(2) system call
/// - Windows: Uses WriteFile with OVERLAPPED structure
///
/// # Errors
///
/// Returns error if write fails. Returns actual bytes written.
pub fn pwrite(file: &File, buf: &[u8], offset: i64) -> Result<usize> {
    if buf.is_empty() {
        return Ok(0);
    }

    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;
        let fd = file.as_raw_fd();
        let count = buf.len();

        let mut total = 0usize;
        let mut remaining = count;
        let mut current_offset = offset;

        loop {
            let result = unsafe {
                libc::pwrite(
                    fd,
                    buf[total..].as_ptr() as *const libc::c_void,
                    remaining,
                    current_offset,
                )
            };

            if result == -1 {
                let err = Error::last_os_error();
                if err.kind() == ErrorKind::Interrupted {
                    continue;
                }
                return Err(err);
            }

            let nwritten = result as usize;
            total += nwritten;
            remaining -= nwritten;
            current_offset += nwritten as i64;

            if remaining == 0 {
                return Ok(total);
            }
        }
    }

    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Foundation::HANDLE;
        use windows_sys::Win32::Storage::FileSystem::WriteFile;
        use windows_sys::Win32::System::IO::OVERLAPPED;

        let handle = file.as_raw_handle() as HANDLE;

        let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
        let uoff = offset as u64;
        overlapped.Anonymous.Anonymous.Offset = (uoff & 0xFFFFFFFF) as u32;
        overlapped.Anonymous.Anonymous.OffsetHigh = (uoff >> 32) as u32;

        let mut bytes_written: u32 = 0;
        let result = unsafe {
            WriteFile(
                handle,
                buf.as_ptr() as *const _,
                buf.len() as u32,
                &mut bytes_written,
                &mut overlapped,
            )
        };

        if result == 0 {
            return Err(Error::last_os_error());
        }

        Ok(bytes_written as usize)
    }
}

/// Advise the OS about expected file access patterns.
///
/// This is a hint to the kernel about how the application intends to access
/// file data. The OS may use this to optimize I/O performance through prefetching,
/// caching adjustments, or resource management.
///
/// # Platform Support
///
/// - Linux: Uses posix_fadvise(2)
/// - macOS: Not implemented (no posix_fadvise)
/// - Windows: Not implemented (could use PrefetchVirtualMemory in future)
///
/// This is a performance hint only and does not affect correctness.
pub fn file_advise(file: &File, offset: i64, count: usize, advice: FileAdvice) {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::io::AsRawFd;
        let fd = file.as_raw_fd();

        let advice_flag = match advice {
            FileAdvice::Normal => libc::POSIX_FADV_NORMAL,
            FileAdvice::WillNeed => libc::POSIX_FADV_WILLNEED,
            FileAdvice::DontNeed => libc::POSIX_FADV_DONTNEED,
            FileAdvice::RandomAccess => libc::POSIX_FADV_RANDOM,
        };

        unsafe {
            libc::posix_fadvise(fd, offset, count as i64, advice_flag);
        }
    }

    #[cfg(not(target_os = "linux"))]
    {
        // No-op on platforms without support
        let _ = (file, offset, count, advice);
    }
}

/// Get the length of a file in bytes.
///
/// # Errors
///
/// Returns error if file metadata cannot be retrieved.
pub fn get_file_len(file: &File) -> Result<u64> {
    Ok(file.metadata()?.len())
}

/// Get the length of a file at the given path in bytes.
///
/// # Errors
///
/// Returns error if file metadata cannot be retrieved.
pub fn get_file_len_path<P: AsRef<Path>>(path: P) -> Result<u64> {
    Ok(std::fs::metadata(path)?.len())
}

/// Get the modification time of a file in seconds since Unix epoch.
///
/// Returns fractional seconds with maximum precision available on the platform
/// (typically nanoseconds on modern systems).
///
/// # Errors
///
/// Returns error if file metadata cannot be retrieved.
pub fn get_mtime<P: AsRef<Path>>(path: P) -> Result<f64> {
    let metadata = std::fs::metadata(path)?;
    let mtime = metadata.modified()?;
    let duration = mtime
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(Error::other)?;

    Ok(duration.as_secs() as f64 + duration.subsec_nanos() as f64 * 1e-9)
}

/// Memory map a file for read-only access.
///
/// Creates a private, read-only memory mapping of the entire file or a portion of it.
/// This provides zero-copy access to file contents.
///
/// # Arguments
///
/// * `path` - Path to the file to map
/// * `err_msg` - Optional error message output
///
/// # Errors
///
/// Returns `None` if mapping fails, and populates `err_msg` if provided.
///
/// # Safety
///
/// The file must not be modified by other processes while mapped, as this could
/// cause undefined behavior. The mapping uses MAP_PRIVATE/FILE_MAP_COPY to avoid
/// this issue, but the OS may still deliver SIGBUS on truncation.
pub fn map_file_ro<P: AsRef<Path>>(
    path: P,
    err_msg: Option<&mut String>,
) -> Option<ConstFileMapping> {
    match File::open(path.as_ref()) {
        Ok(file) => map_file_ro_from_file(&file, err_msg),
        Err(e) => {
            if let Some(msg) = err_msg {
                *msg = format!("Failed to open file: {}", e);
            }
            None
        }
    }
}

/// Memory map an open file for read-only access.
///
/// Creates a private, read-only memory mapping of the entire file.
///
/// # Errors
///
/// Returns `None` if mapping fails, and populates `err_msg` if provided.
pub fn map_file_ro_from_file(
    file: &File,
    err_msg: Option<&mut String>,
) -> Option<ConstFileMapping> {
    match get_file_len(file) {
        Ok(len) if len == 0 => {
            if let Some(msg) = err_msg {
                *msg = "Cannot map empty file".to_string();
            }
            None
        }
        Ok(_) => match unsafe { MmapOptions::new().map(file) } {
            Ok(mmap) => Some(ConstFileMapping { mmap }),
            Err(e) => {
                if let Some(msg) = err_msg {
                    *msg = format!("Failed to mmap: {}", e);
                }
                None
            }
        },
        Err(e) => {
            if let Some(msg) = err_msg {
                *msg = format!("Failed to get file length: {}", e);
            }
            None
        }
    }
}

/// Memory map a file for copy-on-write access.
///
/// Creates a private, copy-on-write memory mapping. Modifications create private
/// copies of affected pages and are not written back to the file.
///
/// # Arguments
///
/// * `path` - Path to the file to map
/// * `err_msg` - Optional error message output
///
/// # Errors
///
/// Returns `None` if mapping fails, and populates `err_msg` if provided.
pub fn map_file_rw<P: AsRef<Path>>(
    path: P,
    err_msg: Option<&mut String>,
) -> Option<MutableFileMapping> {
    match OpenOptions::new()
        .read(true)
        .write(true)
        .open(path.as_ref())
    {
        Ok(file) => map_file_rw_from_file(&file, err_msg),
        Err(e) => {
            if let Some(msg) = err_msg {
                *msg = format!("Failed to open file: {}", e);
            }
            None
        }
    }
}

/// Memory map an open file for copy-on-write access.
///
/// Creates a private, copy-on-write memory mapping of the entire file.
///
/// # Errors
///
/// Returns `None` if mapping fails, and populates `err_msg` if provided.
pub fn map_file_rw_from_file(
    file: &File,
    err_msg: Option<&mut String>,
) -> Option<MutableFileMapping> {
    match get_file_len(file) {
        Ok(len) if len == 0 => {
            if let Some(msg) = err_msg {
                *msg = "Cannot map empty file".to_string();
            }
            None
        }
        Ok(_) => match unsafe { MmapOptions::new().map_copy(file) } {
            Ok(mmap) => Some(MutableFileMapping { mmap }),
            Err(e) => {
                if let Some(msg) = err_msg {
                    *msg = format!("Failed to mmap: {}", e);
                }
                None
            }
        },
        Err(e) => {
            if let Some(msg) = err_msg {
                *msg = format!("Failed to get file length: {}", e);
            }
            None
        }
    }
}

/// Normalizes a path by removing redundant separators and resolving `.` and `..`.
///
/// Normalizes a path following C++ ArchNormPath semantics:
/// - Converts backslashes to forward slashes (all platforms)
/// - Removes redundant separators (e.g., `a//b` -> `a/b`)
/// - Resolves `.` (current directory) and `..` (parent directory)
/// - Preserves `..` in relative paths when no parent exists
/// - POSIX leading slashes: 1->1, 2->2, 3+->1
/// - On Windows, extracts drive specifier before normalizing
///
/// # Examples
///
/// ```
/// use usd_arch::norm_path;
///
/// assert_eq!(norm_path("a//b/./c/../d", false), "a/b/d");
/// assert_eq!(norm_path("/a/b/..", false), "/a");
/// assert_eq!(norm_path("..", false), "..");
/// assert_eq!(norm_path("//", false), "//");
/// assert_eq!(norm_path("///", false), "/");
/// assert_eq!(norm_path("foo/../../bar", false), "../bar");
/// ```
pub fn norm_path(path: &str, strip_drive: bool) -> String {
    // Convert backslashes to forward slashes on all platforms.
    let path = path.replace('\\', "/");

    // Extract drive specifier (e.g. "C:" from "C:/foo").
    let (prefix, remainder) = extract_drive(&path, strip_drive);

    // Core normalization matching C++ _NormPath().
    let normalized = norm_path_impl(remainder);

    if prefix.is_empty() {
        normalized
    } else {
        format!("{prefix}{normalized}")
    }
}

/// Extract drive specifier from path (Windows: "C:" prefix).
/// Returns (prefix_to_keep, remainder).
fn extract_drive<'a>(path: &'a str, strip_drive: bool) -> (String, &'a str) {
    let bytes = path.as_bytes();
    if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        let prefix = if strip_drive {
            String::new()
        } else {
            path[..2].to_string()
        };
        (prefix, &path[2..])
    } else {
        (String::new(), path)
    }
}

/// Core path normalization matching C++ `_NormPath()`.
/// Single-pass: handles `.`, `..`, redundant slashes, POSIX leading slashes.
fn norm_path_impl(input: &str) -> String {
    if input.is_empty() {
        return ".".to_string();
    }

    let bytes = input.as_bytes();
    let len = bytes.len();

    // Count leading slashes -- POSIX: 1->1, 2->2, 3+->1.
    let leading = bytes.iter().take_while(|&&b| b == b'/').count();
    let num_leading = if leading >= 3 { 1 } else { leading };

    // Output buffer pre-filled with leading slashes.
    let mut out: Vec<u8> = vec![b'/'; num_leading];
    let first_write = num_leading;

    // Tokenize: walk through slash-delimited elements.
    let mut i = leading;
    while i < len {
        // Skip slashes between tokens.
        while i < len && bytes[i] == b'/' {
            i += 1;
        }
        if i >= len {
            break;
        }
        // Find end of token.
        let start = i;
        while i < len && bytes[i] != b'/' {
            i += 1;
        }
        let token = &bytes[start..i];

        match token {
            // "." -- skip entirely.
            b"." => {}
            // ".." -- pop last element or preserve if relative.
            b".." => {
                let cur_len = out.len();
                // Trim trailing slash (not a leading slash).
                let trim = if cur_len > first_write && out[cur_len - 1] == b'/' {
                    cur_len - 1
                } else {
                    cur_len
                };

                // Find start of last element.
                let last_slash = out[first_write..trim]
                    .iter()
                    .rposition(|&b| b == b'/')
                    .map(|p| first_write + p);

                let last_elem_start = match last_slash {
                    Some(pos) => pos + 1,
                    None => first_write,
                };

                let last_elem = &out[last_elem_start..trim];

                if last_elem.is_empty() && num_leading == 0 {
                    // Relative path, no elems left -- emit "..".
                    if out.len() > first_write {
                        out.push(b'/');
                    }
                    out.extend_from_slice(b"..");
                } else if last_elem == b".." {
                    // Previous elem is also ".." -- emit another.
                    out.push(b'/');
                    out.extend_from_slice(b"..");
                } else if !last_elem.is_empty() {
                    // Pop last element, including preceding slash if not at root.
                    let trunc_to = if last_elem_start > first_write {
                        last_elem_start - 1 // remove the '/' before the element too
                    } else {
                        last_elem_start
                    };
                    out.truncate(trunc_to);
                }
                // else: absolute path at root -- ignore (can't go above root).
            }
            // Normal element -- copy.
            _ => {
                if out.len() > first_write {
                    out.push(b'/');
                }
                out.extend_from_slice(token);
            }
        }
    }

    // Remove trailing slash (not if it's the only content).
    if out.len() > first_write && out.last() == Some(&b'/') {
        out.pop();
    }

    if out.is_empty() {
        return ".".to_string();
    }

    String::from_utf8(out).unwrap_or_else(|_| ".".to_string())
}

/// Returns the absolute path of the given path.
///
/// If the path is already absolute, returns it normalized.
/// If relative, prepends the current working directory.
///
/// # Examples
///
/// ```
/// use usd_arch::abs_path;
///
/// let path = abs_path("relative/path");
/// assert!(path.starts_with('/') || path.chars().nth(1) == Some(':'));
/// ```
pub fn abs_path(path: &str) -> String {
    let p = std::path::Path::new(path);

    if p.is_absolute() {
        return norm_path(path, false);
    }

    if let Ok(cwd) = std::env::current_dir() {
        let full = cwd.join(p);
        norm_path(&full.to_string_lossy(), false)
    } else {
        norm_path(path, false)
    }
}

/// Returns the path to a system-determined temporary directory.
///
/// On Linux: Returns `/var/tmp` for legacy compatibility (persists across reboots)
/// On Darwin/macOS: Returns `/tmp` (cleaned on reboot)
/// On Windows: Returns the system temp directory
///
/// The returned path does not have a trailing slash.
///
/// # Examples
///
/// ```
/// use usd_arch::get_arch_tmp_dir;
///
/// let tmp = get_arch_tmp_dir();
/// assert!(!tmp.ends_with('/'));
/// assert!(!tmp.ends_with('\\'));
/// ```
#[must_use]
pub fn get_arch_tmp_dir() -> String {
    use std::sync::OnceLock;
    static DIR: OnceLock<String> = OnceLock::new();
    DIR.get_or_init(|| {
        // C++ checks $TMPDIR first, falls back to platform default
        if let Ok(val) = std::env::var("TMPDIR") {
            let trimmed = val.trim_end_matches('/').trim_end_matches('\\');
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }

        #[cfg(target_os = "linux")]
        {
            // USD uses /var/tmp on Linux for legacy reasons
            return "/var/tmp".to_string();
        }

        #[cfg(target_os = "macos")]
        {
            // macOS uses /tmp (cleaned on reboot)
            return "/tmp".to_string();
        }

        #[cfg(windows)]
        {
            return std::env::temp_dir()
                .to_string_lossy()
                .trim_end_matches('\\')
                .trim_end_matches('/')
                .to_string();
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
        {
            std::env::temp_dir()
                .to_string_lossy()
                .trim_end_matches('/')
                .to_string()
        }
    })
    .clone()
}

/// Creates a temporary file with a unique name.
///
/// Returns a file descriptor and optionally the full path to the created file.
/// The file is created in the system temporary directory.
///
/// Format: `tmpdir/prefix.XXXXXX` where XXXXXX is a unique suffix.
///
/// # Arguments
///
/// * `prefix` - Prefix for the temporary filename
/// * `pathname` - Optional output for the full path to the created file
///
/// # Errors
///
/// Returns -1 on failure (check errno).
///
/// # Safety
///
/// The returned file descriptor must be closed by the caller.
///
/// # Examples
///
/// ```ignore
/// use usd_arch::make_tmp_file;
///
/// let mut path = String::new();
/// let fd = make_tmp_file("myprefix", Some(&mut path));
/// if fd >= 0 {
///     println!("Created temp file: {}", path);
///     // Close the file descriptor when done
/// }
/// ```
pub fn make_tmp_file(prefix: &str, pathname: Option<&mut String>) -> i32 {
    let tmpdir = get_arch_tmp_dir();
    make_tmp_file_in(&tmpdir, prefix, pathname)
}

/// Creates a temporary file in a specific directory.
///
/// Like `make_tmp_file` but allows specifying the directory.
///
/// # Errors
///
/// Returns -1 on failure (check errno).
pub fn make_tmp_file_in(tmpdir: &str, prefix: &str, pathname: Option<&mut String>) -> i32 {
    #[cfg(unix)]
    use std::ffi::CString;

    #[cfg(unix)]
    {
        let template = format!("{}/{}XXXXXX", tmpdir, prefix);
        let c_template = match CString::new(template) {
            Ok(s) => s,
            Err(_) => return -1,
        };

        let mut buf = c_template.into_bytes_with_nul();
        let fd = unsafe { libc::mkstemp(buf.as_mut_ptr() as *mut i8) };

        if fd >= 0 {
            // Match C++ fchmod(fd, 0640) after mkstemp
            unsafe {
                libc::fchmod(fd, 0o640);
            }
            if let Some(path) = pathname {
                // Remove trailing null
                buf.pop();
                *path = String::from_utf8_lossy(&buf).into_owned();
            }
        }

        fd
    }

    #[cfg(windows)]
    {
        use std::fs::OpenOptions;
        use std::os::windows::fs::OpenOptionsExt;
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_TEMPORARY;

        // Generate unique filename
        let pid = std::process::id();
        let thread_id = std::thread::current().id();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);

        // Use hash of thread_id since as_u64() is unstable
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        thread_id.hash(&mut hasher);
        let thread_hash = hasher.finish();

        let filename = format!(
            "{}/{}.{:x}.{:016x}.{:x}.tmp",
            tmpdir, prefix, pid, thread_hash, timestamp
        );

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .attributes(FILE_ATTRIBUTE_TEMPORARY)
            .open(&filename);

        match file {
            Ok(f) => {
                if let Some(path) = pathname {
                    *path = filename;
                }
                // Convert handle to fd-like number (not a real fd on Windows)
                f.as_raw_handle() as i32
            }
            Err(_) => -1,
        }
    }
}

/// Creates a temporary subdirectory with a unique name.
///
/// Returns the full path to the created directory.
///
/// Format: `tmpdir/prefix.XXXXXX/` where XXXXXX is a unique suffix.
///
/// # Arguments
///
/// * `tmpdir` - Parent directory for the temp subdirectory
/// * `prefix` - Prefix for the temporary directory name
///
/// # Errors
///
/// Returns an empty string on failure (check errno).
///
/// # Examples
///
/// ```no_run
/// use usd_arch::make_tmp_subdir;
///
/// let tmpdir = "/tmp";
/// let subdir = make_tmp_subdir(tmpdir, "myprefix");
/// if !subdir.is_empty() {
///     println!("Created temp subdir: {}", subdir);
/// }
/// ```
pub fn make_tmp_subdir(tmpdir: &str, prefix: &str) -> String {
    #[cfg(unix)]
    use std::ffi::CString;

    #[cfg(unix)]
    {
        let template = format!("{}/{}XXXXXX", tmpdir, prefix);
        let c_template = match CString::new(template) {
            Ok(s) => s,
            Err(_) => return String::new(),
        };

        let mut buf = c_template.into_bytes_with_nul();
        let result = unsafe { libc::mkdtemp(buf.as_mut_ptr() as *mut i8) };

        if result.is_null() {
            return String::new();
        }

        // Match C++ chmod(path, 0750) after mkdtemp
        unsafe {
            libc::chmod(buf.as_ptr() as *const libc::c_char, 0o750);
        }

        // Remove trailing null
        buf.pop();
        String::from_utf8_lossy(&buf).into_owned()
    }

    #[cfg(windows)]
    {
        use std::fs;

        // Generate unique directory name
        let pid = std::process::id();
        let thread_id = std::thread::current().id();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);

        // Use hash of thread_id since as_u64() is unstable
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        thread_id.hash(&mut hasher);
        let thread_hash = hasher.finish();

        let dirname = format!(
            "{}/{}.{:x}.{:016x}.{:x}",
            tmpdir, prefix, pid, thread_hash, timestamp
        );

        match fs::create_dir(&dirname) {
            Ok(_) => dirname,
            Err(_) => String::new(),
        }
    }
}

/// Reads the target of a symbolic link.
///
/// Returns the path that the symbolic link points to.
///
/// # Errors
///
/// Returns an empty string if the path is not a symbolic link or cannot be read.
///
/// # Examples
///
/// ```no_run
/// use usd_arch::read_link;
///
/// let target = read_link("/tmp/mylink");
/// if !target.is_empty() {
///     println!("Link points to: {}", target);
/// }
/// ```
#[must_use]
pub fn read_link(path: &str) -> String {
    match std::fs::read_link(path) {
        Ok(target) => target.to_string_lossy().into_owned(),
        Err(_) => String::new(),
    }
}

/// Makes a temporary file name (deprecated - prefer `make_tmp_file`).
///
/// Returns a path of the form `TMPDIR/prefix.PID[.N]suffix`.
///
/// **Warning**: This has a TOCTOU race between naming and creation.
/// Use `make_tmp_file` instead for secure temporary file creation.
///
/// # Arguments
///
/// * `prefix` - Filename prefix
/// * `suffix` - Optional filename suffix
///
/// # Examples
///
/// ```
/// use usd_arch::make_tmp_file_name;
///
/// let name = make_tmp_file_name("usd", ".tmp");
/// assert!(name.contains("usd"));
/// ```
pub fn make_tmp_file_name(prefix: &str, suffix: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let tmpdir = get_arch_tmp_dir();
    let pid = std::process::id();
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);

    if count == 0 {
        format!("{}{}{}.{}{}", tmpdir, PATH_SEP, prefix, pid, suffix)
    } else {
        format!(
            "{}{}{}.{}.{}{}",
            tmpdir, PATH_SEP, prefix, pid, count, suffix
        )
    }
}

/// Returns the permissions mode for the given path.
///
/// On success, returns `Ok(mode)` where `mode` is the file permission bits.
/// On failure, returns `Err`.
///
/// On Unix, this returns the `st_mode` field from `stat(2)`.
/// On Windows, this returns a simplified mode.
///
/// # Examples
///
/// ```no_run
/// use usd_arch::get_stat_mode;
///
/// if let Ok(mode) = get_stat_mode("/tmp/test") {
///     println!("mode: {:o}", mode);
/// }
/// ```
pub fn get_stat_mode<P: AsRef<Path>>(path: P) -> Result<u32> {
    let metadata = std::fs::metadata(path)?;

    #[cfg(unix)]
    {
        Ok(metadata.permissions().mode())
    }

    #[cfg(windows)]
    {
        // On Windows, approximate Unix-style mode bits
        let readonly = metadata.permissions().readonly();
        let is_dir = metadata.is_dir();
        let mut mode: u32 = if is_dir { 0o755 } else { 0o644 };
        if readonly {
            mode &= !0o222; // Remove write bits
        }
        Ok(mode)
    }

    #[cfg(not(any(unix, windows)))]
    {
        Ok(0o644)
    }
}

/// Memory access advice hints for memory-mapped regions.
///
/// These hints inform the OS about expected access patterns for
/// memory-mapped regions, similar to `madvise(2)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemAdvice {
    /// Normal access pattern (default).
    Normal,
    /// Data will be accessed soon - OS may prefetch.
    WillNeed,
    /// Data won't be needed soon - OS may free resources.
    DontNeed,
    /// Random access pattern - sequential prefetch may not help.
    RandomAccess,
}

/// Advise the OS regarding how the application intends to access a range
/// of memory.
///
/// This is primarily useful for memory-mapped file regions. This call
/// does not change program semantics - it is only an optimization hint
/// and may be a no-op on some platforms.
///
/// # Note
///
/// This is a safe no-op in the pure Rust port. The C++ version uses
/// `madvise(2)` on Linux/macOS, but since this is purely an optimization
/// hint with no semantic effect, a no-op is correct behavior.
/// The `memmap2` crate used for file mapping already provides its own
/// advisory methods if needed in the future.
///
/// # Arguments
///
/// * `_addr` - Start of the memory range
/// * `_len` - Length of the memory range in bytes
/// * `_advice` - The access advice hint
pub fn mem_advise(addr: *const u8, len: usize, advice: MemAdvice) {
    #[cfg(unix)]
    {
        // Page-align the address down before calling madvise (C++ parity).
        let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize };
        let aligned = (addr as usize) & !(page_size - 1);
        // Extend length to cover the original range from the aligned base.
        let extra = (addr as usize) - aligned;
        let advise_flag = match advice {
            MemAdvice::Normal => libc::MADV_NORMAL,
            MemAdvice::WillNeed => libc::MADV_WILLNEED,
            MemAdvice::DontNeed => libc::MADV_DONTNEED,
            MemAdvice::RandomAccess => libc::MADV_RANDOM,
        };
        unsafe {
            libc::madvise(aligned as *mut libc::c_void, len + extra, advise_flag);
        }
    }
    #[cfg(not(unix))]
    {
        // No-op on non-Unix: memory advice is an optimization hint.
        let _ = (addr, len, advice);
    }
}

/// Query whether memory-mapped pages are resident in RAM.
///
/// Returns `false` on all platforms in the pure Rust port.
///
/// The C++ version uses `mincore(2)` on Linux/macOS. Since this is only
/// used for diagnostic/profiling purposes and has no effect on correctness,
/// always returning `false` is safe.
///
/// # Arguments
///
/// * `_addr` - Start of the memory range (must be page-aligned)
/// * `_len` - Length of the memory range in bytes
/// * `_page_map` - Output buffer for residency information
#[allow(dead_code)]
pub fn query_mapped_memory_residency(_addr: *const u8, _len: usize, _page_map: &mut [u8]) -> bool {
    // No-op: returns false like the C++ Windows implementation.
    // This is purely diagnostic and has no effect on correctness.
    false
}

/// Returns the file path associated with an open file handle, if available.
///
/// There are many reasons why it may be impossible to obtain a filename
/// from a file handle. Whenever possible, avoid using this function and
/// instead store the filename for future use.
///
/// # Platform Notes
///
/// - Linux: reads `/proc/self/fd/<fd>` symlink via readlink(2)
/// - macOS: uses `fcntl(F_GETPATH)` syscall
/// - Windows: uses `GetFinalPathNameByHandleW` Win32 API
///
/// Returns `None` if the path cannot be determined.
pub fn get_file_name(file: &File) -> Option<std::path::PathBuf> {
    get_file_name_impl(file)
}

// ---- Linux implementation ------------------------------------------------

#[cfg(target_os = "linux")]
fn get_file_name_impl(file: &File) -> Option<std::path::PathBuf> {
    use std::os::unix::io::AsRawFd;
    let fd = file.as_raw_fd();
    // /proc/self/fd/<N> is a symlink to the actual file path
    let proc_path = format!("/proc/self/fd/{}", fd);
    std::fs::read_link(&proc_path).ok()
}

// ---- macOS implementation ------------------------------------------------

#[cfg(target_os = "macos")]
fn get_file_name_impl(file: &File) -> Option<std::path::PathBuf> {
    use std::os::unix::io::AsRawFd;

    let fd = file.as_raw_fd();
    // MAXPATHLEN on Darwin is 1024
    let mut buf = vec![0i8; libc::PATH_MAX as usize];

    // fcntl(F_GETPATH) fills buf with the resolved path for the fd
    let ret = unsafe { libc::fcntl(fd, libc::F_GETPATH, buf.as_mut_ptr()) };
    if ret == -1 {
        return None;
    }

    // Find NUL terminator and convert to OsString
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    let bytes: Vec<u8> = buf[..end].iter().map(|&c| c as u8).collect();
    use std::os::unix::ffi::OsStringExt;
    Some(std::path::PathBuf::from(std::ffi::OsString::from_vec(
        bytes,
    )))
}

// ---- Windows implementation ----------------------------------------------

#[cfg(windows)]
fn get_file_name_impl(file: &File) -> Option<std::path::PathBuf> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::Storage::FileSystem::{GetFinalPathNameByHandleW, VOLUME_NAME_DOS};

    let handle = file.as_raw_handle() as HANDLE;

    // First call: get required buffer size (returned value includes NUL)
    let needed =
        unsafe { GetFinalPathNameByHandleW(handle, std::ptr::null_mut(), 0, VOLUME_NAME_DOS) };
    if needed == 0 {
        return None;
    }

    let mut buf: Vec<u16> = vec![0u16; needed as usize];
    let written = unsafe {
        GetFinalPathNameByHandleW(handle, buf.as_mut_ptr(), buf.len() as u32, VOLUME_NAME_DOS)
    };
    if written == 0 {
        return None;
    }

    // Strip the extended-path prefix "\\?\" added by VOLUME_NAME_DOS
    let path_slice = &buf[..written as usize];
    let path = std::path::PathBuf::from(std::ffi::OsString::from(String::from_utf16_lossy(
        path_slice,
    )));

    // Remove \\?\ or \\?\UNC\ prefix so callers get a clean path
    let path_str = path.to_string_lossy();
    let clean = if let Some(rest) = path_str.strip_prefix(r"\\?\UNC\") {
        std::path::PathBuf::from(format!(r"\\{}", rest))
    } else if let Some(rest) = path_str.strip_prefix(r"\\?\") {
        std::path::PathBuf::from(rest)
    } else {
        path
    };
    Some(clean)
}

// ---- Fallback for other platforms -----------------------------------------

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn get_file_name_impl(_file: &File) -> Option<std::path::PathBuf> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use tempfile::NamedTempFile;

    #[test]
    fn test_open_file_modes() {
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path();

        // Write mode
        {
            let mut file = open_file(path, "w").unwrap();
            file.write_all(b"test data").unwrap();
        }

        // Read mode
        {
            let mut file = open_file(path, "r").unwrap();
            let mut buf = String::new();
            file.read_to_string(&mut buf).unwrap();
            assert_eq!(buf, "test data");
        }

        // Append mode
        {
            let mut file = open_file(path, "a").unwrap();
            file.write_all(b" appended").unwrap();
        }

        // Read back
        {
            let mut file = open_file(path, "r").unwrap();
            let mut buf = String::new();
            file.read_to_string(&mut buf).unwrap();
            assert_eq!(buf, "test data appended");
        }
    }

    #[test]
    fn test_stat_is_writable() {
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path();

        // Temporary files should be writable
        let writable = stat_is_writable(path).unwrap();
        assert!(writable);
    }

    #[test]
    fn test_pread_pwrite() {
        let mut temp = NamedTempFile::new().unwrap();

        // Write initial data
        temp.write_all(b"0123456789").unwrap();
        temp.flush().unwrap();

        let path = temp.path().to_path_buf();
        let file = open_file(&path, "r+").unwrap();

        // Pwrite at offset 5
        let written = pwrite(&file, b"ABCDE", 5i64).unwrap();
        assert_eq!(written, 5);

        // Pread from offset 3
        let mut buf = [0u8; 7];
        let read = pread(&file, &mut buf, 3i64).unwrap();
        assert_eq!(read, 7);
        assert_eq!(&buf, b"34ABCDE");

        // Verify file position unchanged
        let mut content = Vec::new();
        let mut file_copy = open_file(&path, "r").unwrap();
        file_copy.read_to_end(&mut content).unwrap();
        assert_eq!(&content, b"01234ABCDE");
    }

    #[test]
    fn test_get_file_len() {
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(b"test content").unwrap();
        temp.flush().unwrap();

        let len = get_file_len(temp.as_file()).unwrap();
        assert_eq!(len, 12);

        let len_path = get_file_len_path(temp.path()).unwrap();
        assert_eq!(len_path, 12);
    }

    #[test]
    fn test_get_mtime() {
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path();

        let mtime = get_mtime(path).unwrap();
        assert!(mtime > 0.0);

        // Modification time should be recent (within last hour)
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64();
        assert!((now - mtime).abs() < 3600.0);
    }

    #[test]
    fn test_file_advise() {
        let temp = NamedTempFile::new().unwrap();
        let file = temp.as_file();

        // Should not crash (may be no-op on some platforms)
        file_advise(file, 0, 1024, FileAdvice::Normal);
        file_advise(file, 0, 1024, FileAdvice::WillNeed);
        file_advise(file, 0, 1024, FileAdvice::DontNeed);
        file_advise(file, 0, 1024, FileAdvice::RandomAccess);
    }

    #[test]
    fn test_map_file_ro() {
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(b"memory mapped content").unwrap();
        temp.flush().unwrap();
        let path = temp.path().to_path_buf();

        let mut err_msg = String::new();
        let mapping = map_file_ro(&path, Some(&mut err_msg)).unwrap();

        assert_eq!(mapping.len(), 21);
        assert_eq!(&mapping[..], b"memory mapped content");
        assert_eq!(mapping.as_slice(), b"memory mapped content");
    }

    #[test]
    fn test_map_file_rw() {
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(b"mutable mapped data").unwrap();
        temp.flush().unwrap();
        let path = temp.path().to_path_buf();

        let mut err_msg = String::new();
        let mut mapping = map_file_rw(&path, Some(&mut err_msg)).unwrap();

        assert_eq!(mapping.len(), 19);

        // Modify mapped data (copy-on-write)
        mapping[0] = b'M';
        assert_eq!(&mapping[..7], b"Mutable");

        // Original file should be unchanged (copy-on-write)
        let mut file = open_file(&path, "r").unwrap();
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).unwrap();
        assert_eq!(&buf, b"mutable mapped data");
    }

    #[test]
    fn test_map_empty_file() {
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path();

        let mut err_msg = String::new();
        let mapping = map_file_ro(path, Some(&mut err_msg));

        assert!(mapping.is_none());
        assert!(err_msg.contains("empty"));
    }

    #[test]
    fn test_const_mapping_traits() {
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(b"test").unwrap();
        temp.flush().unwrap();
        let path = temp.path().to_path_buf();

        let mapping = map_file_ro(&path, None).unwrap();

        // Test Deref
        assert_eq!(&*mapping, b"test");

        // Test AsRef
        let slice: &[u8] = mapping.as_ref();
        assert_eq!(slice, b"test");

        // Test indexing
        assert_eq!(mapping[0], b't');
    }

    #[test]
    fn test_mutable_mapping_traits() {
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(b"test").unwrap();
        temp.flush().unwrap();
        let path = temp.path().to_path_buf();

        let mut mapping = map_file_rw(&path, None).unwrap();

        // Test DerefMut
        mapping[0] = b'T';
        assert_eq!(&*mapping, b"Test");

        // Test AsMut
        let slice: &mut [u8] = mapping.as_mut();
        slice[1] = b'E';
        assert_eq!(&*mapping, b"TEst");
    }

    #[test]
    fn test_path_constants() {
        assert!(!PATH_SEP.is_empty());
        assert!(!PATH_LIST_SEP.is_empty());
        assert!(!REL_PATH_IDENT.is_empty());
        assert!(PATH_MAX > 0);
    }

    #[test]
    fn test_make_tmp_file_name() {
        let name1 = make_tmp_file_name("usd_test", ".tmp");
        assert!(name1.contains("usd_test"));
        assert!(name1.ends_with(".tmp"));

        // Second call should produce different name
        let name2 = make_tmp_file_name("usd_test", ".tmp");
        assert_ne!(name1, name2);
    }

    #[test]
    fn test_get_stat_mode() {
        let temp = NamedTempFile::new().unwrap();
        let mode = get_stat_mode(temp.path());
        assert!(mode.is_ok());
        let mode = mode.unwrap();
        // File should have at least read permission for owner
        assert!(mode & 0o400 != 0);
    }

    #[test]
    fn test_get_file_name() {
        let temp = NamedTempFile::new().unwrap();
        let file = temp.as_file();
        let name = get_file_name(file);
        // May or may not return a name depending on platform, but shouldn't panic
        // On most platforms this should return something non-empty for a named temp file
        let _ = name;
    }

    #[cfg(windows)]
    #[test]
    fn test_windows_utf_roundtrip() {
        let original = "Hello World - USD test";
        let wide = arch_windows_utf8_to_utf16(original);
        let back = arch_windows_utf16_to_utf8(&wide);
        assert_eq!(original, back);
    }

    // -- norm_path tests matching C++ TestArchNormPath --

    #[test]
    fn test_norm_path_empty() {
        assert_eq!(norm_path("", false), ".");
    }

    #[test]
    fn test_norm_path_dot() {
        assert_eq!(norm_path(".", false), ".");
    }

    #[test]
    fn test_norm_path_dotdot() {
        assert_eq!(norm_path("..", false), "..");
    }

    #[test]
    fn test_norm_path_dotdot_resolve() {
        assert_eq!(norm_path("foobar/../barbaz", false), "barbaz");
    }

    #[test]
    fn test_norm_path_single_slash() {
        assert_eq!(norm_path("/", false), "/");
    }

    #[test]
    fn test_norm_path_double_slash() {
        // POSIX: exactly 2 leading slashes preserved.
        assert_eq!(norm_path("//", false), "//");
    }

    #[test]
    fn test_norm_path_triple_slash() {
        // POSIX: 3+ leading slashes collapsed to 1.
        assert_eq!(norm_path("///", false), "/");
    }

    #[test]
    fn test_norm_path_complex_slashes() {
        assert_eq!(norm_path("///foo/.//bar//", false), "/foo/bar");
    }

    #[test]
    fn test_norm_path_dotdot_in_middle() {
        assert_eq!(norm_path("///foo/.//bar//.//..//.//baz", false), "/foo/baz");
    }

    #[test]
    fn test_norm_path_dotdot_at_root() {
        assert_eq!(norm_path("///..//./foo/.//bar", false), "/foo/bar");
    }

    #[test]
    fn test_norm_path_relative_dotdot_overflow() {
        // More ".." than path elements -- preserves remaining ".." for relative paths.
        assert_eq!(
            norm_path("foo/bar/../../../../../../baz", false),
            "../../../../baz"
        );
    }

    #[test]
    fn test_norm_path_backslash() {
        assert_eq!(norm_path("a\\b\\c", false), "a/b/c");
    }

    #[test]
    fn test_norm_path_windows_drive() {
        assert_eq!(norm_path("C:\\foo\\bar", false), "C:/foo/bar");
        assert_eq!(norm_path("C:foo\\bar", false), "C:foo/bar");
        assert_eq!(norm_path("c:\\foo\\bar", false), "c:/foo/bar");
        assert_eq!(norm_path("c:foo\\bar", false), "c:foo/bar");
    }

    #[test]
    fn test_norm_path_strip_drive() {
        assert_eq!(norm_path("C:\\foo\\bar", true), "/foo/bar");
        assert_eq!(norm_path("C:foo\\bar", true), "foo/bar");
    }

    #[test]
    fn test_norm_path_no_trailing_slash() {
        assert_eq!(norm_path("foo/bar/", false), "foo/bar");
        assert_eq!(norm_path("/foo/bar/", false), "/foo/bar");
    }
}

// ---------------------------------------------------------------------------
// Windows Unicode helpers (P1-13)
// ---------------------------------------------------------------------------

/// Converts a UTF-16 wide string to a UTF-8 `String`.
///
/// C++ parity: `ArchWindowsUtf16ToUtf8(const std::wstring&)` -- Windows only.
#[cfg(windows)]
pub fn arch_windows_utf16_to_utf8(wstr: &[u16]) -> String {
    String::from_utf16_lossy(wstr)
}

/// Converts a UTF-8 string to a UTF-16 `Vec<u16>`.
///
/// C++ parity: `ArchWindowsUtf8ToUtf16(const std::string&)` -- Windows only.
#[cfg(windows)]
pub fn arch_windows_utf8_to_utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}
