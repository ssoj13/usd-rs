//! Error number utilities.
//!
//! Provides cross-platform access to system error codes and their descriptions.

use std::io;

/// Returns the last OS error code.
///
/// This is equivalent to `errno` on Unix or `GetLastError()` on Windows.
///
/// # Examples
///
/// ```
/// use usd_arch::get_last_error;
///
/// let err = get_last_error();
/// println!("Last error: {}", err);
/// ```
#[must_use]
pub fn get_last_error() -> i32 {
    io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

/// Returns a human-readable description of an error code.
///
/// # Arguments
///
/// * `errnum` - The error number to describe
///
/// # Examples
///
/// ```
/// use usd_arch::strerror;
///
/// let desc = strerror(2); // ENOENT on Unix
/// println!("Error 2: {}", desc);
/// ```
#[must_use]
pub fn strerror(errnum: i32) -> String {
    io::Error::from_raw_os_error(errnum).to_string()
}

/// Returns a description of the last OS error.
///
/// # Examples
///
/// ```
/// use usd_arch::last_error_string;
///
/// let desc = last_error_string();
/// println!("Last error: {}", desc);
/// ```
#[must_use]
pub fn last_error_string() -> String {
    io::Error::last_os_error().to_string()
}

/// Common error codes for cross-platform use.
pub mod codes {
    /// No error.
    pub const SUCCESS: i32 = 0;

    /// Operation not permitted (EPERM).
    #[cfg(unix)]
    pub const PERM: i32 = libc::EPERM;
    /// Operation not permitted (EPERM).
    #[cfg(windows)]
    pub const PERM: i32 = 1;

    /// No such file or directory (ENOENT).
    #[cfg(unix)]
    pub const NOENT: i32 = libc::ENOENT;
    /// No such file or directory (ENOENT).
    #[cfg(windows)]
    pub const NOENT: i32 = 2;

    /// No such process (ESRCH).
    #[cfg(unix)]
    pub const SRCH: i32 = libc::ESRCH;
    /// No such process (ESRCH).
    #[cfg(windows)]
    pub const SRCH: i32 = 3;

    /// Interrupted system call (EINTR).
    #[cfg(unix)]
    pub const INTR: i32 = libc::EINTR;
    /// Interrupted system call (EINTR).
    #[cfg(windows)]
    pub const INTR: i32 = 4;

    /// I/O error (EIO).
    #[cfg(unix)]
    pub const IO: i32 = libc::EIO;
    /// I/O error (EIO).
    #[cfg(windows)]
    pub const IO: i32 = 5;

    /// No such device or address (ENXIO).
    #[cfg(unix)]
    pub const NXIO: i32 = libc::ENXIO;
    /// No such device or address (ENXIO).
    #[cfg(windows)]
    pub const NXIO: i32 = 6;

    /// Argument list too long (E2BIG).
    #[cfg(unix)]
    pub const TOOBIG: i32 = libc::E2BIG;
    /// Argument list too long (E2BIG).
    #[cfg(windows)]
    pub const TOOBIG: i32 = 7;

    /// Exec format error (ENOEXEC).
    #[cfg(unix)]
    pub const NOEXEC: i32 = libc::ENOEXEC;
    /// Exec format error (ENOEXEC).
    #[cfg(windows)]
    pub const NOEXEC: i32 = 8;

    /// Bad file number (EBADF).
    #[cfg(unix)]
    pub const BADF: i32 = libc::EBADF;
    /// Bad file number (EBADF).
    #[cfg(windows)]
    pub const BADF: i32 = 9;

    /// No child processes (ECHILD).
    #[cfg(unix)]
    pub const CHILD: i32 = libc::ECHILD;
    /// No child processes (ECHILD).
    #[cfg(windows)]
    pub const CHILD: i32 = 10;

    /// Try again (EAGAIN/EWOULDBLOCK).
    #[cfg(unix)]
    pub const AGAIN: i32 = libc::EAGAIN;
    /// Try again (EAGAIN/EWOULDBLOCK).
    #[cfg(windows)]
    pub const AGAIN: i32 = 11;

    /// Out of memory (ENOMEM).
    #[cfg(unix)]
    pub const NOMEM: i32 = libc::ENOMEM;
    /// Out of memory (ENOMEM).
    #[cfg(windows)]
    pub const NOMEM: i32 = 12;

    /// Permission denied (EACCES).
    #[cfg(unix)]
    pub const ACCES: i32 = libc::EACCES;
    /// Permission denied (EACCES).
    #[cfg(windows)]
    pub const ACCES: i32 = 13;

    /// Bad address (EFAULT).
    #[cfg(unix)]
    pub const FAULT: i32 = libc::EFAULT;
    /// Bad address (EFAULT).
    #[cfg(windows)]
    pub const FAULT: i32 = 14;

    /// Device or resource busy (EBUSY).
    #[cfg(unix)]
    pub const BUSY: i32 = libc::EBUSY;
    /// Device or resource busy (EBUSY).
    #[cfg(windows)]
    pub const BUSY: i32 = 16;

    /// File exists (EEXIST).
    #[cfg(unix)]
    pub const EXIST: i32 = libc::EEXIST;
    /// File exists (EEXIST).
    #[cfg(windows)]
    pub const EXIST: i32 = 17;

    /// Cross-device link (EXDEV).
    #[cfg(unix)]
    pub const XDEV: i32 = libc::EXDEV;
    /// Cross-device link (EXDEV).
    #[cfg(windows)]
    pub const XDEV: i32 = 18;

    /// Not a directory (ENOTDIR).
    #[cfg(unix)]
    pub const NOTDIR: i32 = libc::ENOTDIR;
    /// Not a directory (ENOTDIR).
    #[cfg(windows)]
    pub const NOTDIR: i32 = 20;

    /// Is a directory (EISDIR).
    #[cfg(unix)]
    pub const ISDIR: i32 = libc::EISDIR;
    /// Is a directory (EISDIR).
    #[cfg(windows)]
    pub const ISDIR: i32 = 21;

    /// Invalid argument (EINVAL).
    #[cfg(unix)]
    pub const INVAL: i32 = libc::EINVAL;
    /// Invalid argument (EINVAL).
    #[cfg(windows)]
    pub const INVAL: i32 = 22;

    /// File table overflow (ENFILE).
    #[cfg(unix)]
    pub const NFILE: i32 = libc::ENFILE;
    /// File table overflow (ENFILE).
    #[cfg(windows)]
    pub const NFILE: i32 = 23;

    /// Too many open files (EMFILE).
    #[cfg(unix)]
    pub const MFILE: i32 = libc::EMFILE;
    /// Too many open files (EMFILE).
    #[cfg(windows)]
    pub const MFILE: i32 = 24;

    /// Text file busy (ETXTBSY).
    #[cfg(unix)]
    pub const TXTBSY: i32 = libc::ETXTBSY;
    /// Text file busy (ETXTBSY).
    #[cfg(windows)]
    pub const TXTBSY: i32 = 26;

    /// File too large (EFBIG).
    #[cfg(unix)]
    pub const FBIG: i32 = libc::EFBIG;
    /// File too large (EFBIG).
    #[cfg(windows)]
    pub const FBIG: i32 = 27;

    /// No space left on device (ENOSPC).
    #[cfg(unix)]
    pub const NOSPC: i32 = libc::ENOSPC;
    /// No space left on device (ENOSPC).
    #[cfg(windows)]
    pub const NOSPC: i32 = 28;

    /// Illegal seek (ESPIPE).
    #[cfg(unix)]
    pub const SPIPE: i32 = libc::ESPIPE;
    /// Illegal seek (ESPIPE).
    #[cfg(windows)]
    pub const SPIPE: i32 = 29;

    /// Read-only file system (EROFS).
    #[cfg(unix)]
    pub const ROFS: i32 = libc::EROFS;
    /// Read-only file system (EROFS).
    #[cfg(windows)]
    pub const ROFS: i32 = 30;

    /// Too many links (EMLINK).
    #[cfg(unix)]
    pub const MLINK: i32 = libc::EMLINK;
    /// Too many links (EMLINK).
    #[cfg(windows)]
    pub const MLINK: i32 = 31;

    /// Broken pipe (EPIPE).
    #[cfg(unix)]
    pub const PIPE: i32 = libc::EPIPE;
    /// Broken pipe (EPIPE).
    #[cfg(windows)]
    pub const PIPE: i32 = 32;

    /// Math argument out of domain (EDOM).
    #[cfg(unix)]
    pub const DOM: i32 = libc::EDOM;
    /// Math argument out of domain (EDOM).
    #[cfg(windows)]
    pub const DOM: i32 = 33;

    /// Math result not representable (ERANGE).
    #[cfg(unix)]
    pub const RANGE: i32 = libc::ERANGE;
    /// Math result not representable (ERANGE).
    #[cfg(windows)]
    pub const RANGE: i32 = 34;
}

/// A wrapper around system error codes that provides additional context.
#[derive(Debug, Clone)]
pub struct SystemError {
    /// The raw error code
    pub code: i32,
    /// Human-readable description
    pub message: String,
    /// Optional context about where the error occurred
    pub context: Option<String>,
}

impl SystemError {
    /// Creates a new `SystemError` from an error code.
    #[must_use]
    pub fn from_code(code: i32) -> Self {
        Self {
            code,
            message: strerror(code),
            context: None,
        }
    }

    /// Creates a new `SystemError` from the last OS error.
    #[must_use]
    pub fn last() -> Self {
        Self::from_code(get_last_error())
    }

    /// Adds context to the error.
    #[must_use]
    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    /// Returns true if this represents no error (code 0).
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.code == 0
    }
}

impl std::fmt::Display for SystemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(ref ctx) = self.context {
            write!(f, "{}: {} (error {})", ctx, self.message, self.code)
        } else {
            write!(f, "{} (error {})", self.message, self.code)
        }
    }
}

impl std::error::Error for SystemError {}

impl From<io::Error> for SystemError {
    fn from(err: io::Error) -> Self {
        Self {
            code: err.raw_os_error().unwrap_or(-1),
            message: err.to_string(),
            context: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strerror() {
        let desc = strerror(codes::NOENT);
        assert!(!desc.is_empty());
    }

    #[test]
    fn test_system_error() {
        let err = SystemError::from_code(codes::NOENT);
        assert_eq!(err.code, codes::NOENT);
        assert!(!err.is_success());

        let success = SystemError::from_code(0);
        assert!(success.is_success());
    }

    #[test]
    fn test_system_error_with_context() {
        let err = SystemError::from_code(codes::NOENT).with_context("opening file '/tmp/test.txt'");
        let display = format!("{}", err);
        assert!(display.contains("opening file"));
    }
}
