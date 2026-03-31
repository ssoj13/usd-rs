#![allow(unsafe_code)]
//! Background process (daemon) creation.
//!
//! This module provides functionality to daemonize processes on Unix-like systems.
//! On Windows, daemon functionality is not supported directly (use Windows Services instead).
//!
//! # Example
//!
//! ```no_run
//! use usd_arch::{daemonize, DaemonOptions};
//!
//! let options = DaemonOptions::default()
//!     .close_stdin(true)
//!     .close_stdout(false)
//!     .close_stderr(false)
//!     .change_dir("/")
//!     .pid_file(Some("/var/run/mydaemon.pid"));
//!
//! daemonize(&options).expect("Failed to daemonize");
//! ```

use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::fs::File;
#[cfg(unix)]
use std::io::Write;

/// Options for daemonizing a process.
#[derive(Debug, Clone)]
pub struct DaemonOptions {
    /// Close stdin after forking (default: true)
    pub close_stdin: bool,
    /// Close stdout after forking (default: true)
    pub close_stdout: bool,
    /// Close stderr after forking (default: true)
    pub close_stderr: bool,
    /// Change working directory (default: "/")
    pub change_dir: PathBuf,
    /// Optional path to write daemon's PID
    pub pid_file: Option<PathBuf>,
    /// File creation mask (default: 0)
    pub umask: u32,
}

impl Default for DaemonOptions {
    fn default() -> Self {
        Self {
            close_stdin: true,
            close_stdout: true,
            close_stderr: true,
            change_dir: PathBuf::from("/"),
            pid_file: None,
            umask: 0,
        }
    }
}

impl DaemonOptions {
    /// Create new default options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set whether to close stdin.
    pub fn close_stdin(mut self, close: bool) -> Self {
        self.close_stdin = close;
        self
    }

    /// Set whether to close stdout.
    pub fn close_stdout(mut self, close: bool) -> Self {
        self.close_stdout = close;
        self
    }

    /// Set whether to close stderr.
    pub fn close_stderr(mut self, close: bool) -> Self {
        self.close_stderr = close;
        self
    }

    /// Set working directory.
    pub fn change_dir<P: AsRef<Path>>(mut self, dir: P) -> Self {
        self.change_dir = dir.as_ref().to_path_buf();
        self
    }

    /// Set PID file path.
    pub fn pid_file<P: AsRef<Path>>(mut self, path: Option<P>) -> Self {
        self.pid_file = path.map(|p| p.as_ref().to_path_buf());
        self
    }

    /// Set umask value.
    pub fn umask(mut self, mask: u32) -> Self {
        self.umask = mask;
        self
    }
}

/// Daemonize the current process.
///
/// This function forks the process into a background daemon. On Unix systems,
/// it performs the standard double-fork dance to ensure the process is properly
/// detached from the controlling terminal.
///
/// # Platform Support
///
/// - **Unix**: Full support via fork/setsid
/// - **Windows**: Returns error - use Windows Services API instead
///
/// # Errors
///
/// Returns error if:
/// - Platform doesn't support daemonization (Windows)
/// - Fork fails
/// - setsid fails
/// - Directory change fails
/// - PID file cannot be written
///
/// # Example
///
/// ```no_run
/// use usd_arch::{daemonize, DaemonOptions};
///
/// let opts = DaemonOptions::new()
///     .pid_file(Some("/var/run/app.pid"));
///
/// match daemonize(&opts) {
///     Ok(_) => println!("Daemonized successfully"),
///     Err(e) => eprintln!("Failed to daemonize: {}", e),
/// }
/// ```
pub fn daemonize(_options: &DaemonOptions) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        daemonize_unix(_options)
    }

    #[cfg(not(unix))]
    {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "Daemonization is not supported on this platform. Use Windows Services API on Windows.",
        ))
    }
}

#[cfg(unix)]
fn daemonize_unix(options: &DaemonOptions) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    // First fork - parent exits, child continues
    match unsafe { libc::fork() } {
        -1 => return Err(std::io::Error::last_os_error()),
        0 => {
            // Child process continues
        }
        _ => {
            // Parent exits
            std::process::exit(0);
        }
    }

    // Create new session and process group
    if unsafe { libc::setsid() } == -1 {
        return Err(std::io::Error::last_os_error());
    }

    // Second fork - ensures we can't acquire controlling terminal
    match unsafe { libc::fork() } {
        -1 => return Err(std::io::Error::last_os_error()),
        0 => {
            // Child process continues
        }
        _ => {
            // Parent exits
            std::process::exit(0);
        }
    }

    // Set umask
    unsafe {
        libc::umask(options.umask as libc::mode_t);
    }

    // Change working directory
    std::env::set_current_dir(&options.change_dir)?;

    // Close standard file descriptors if requested
    let mut except_fds = Vec::new();

    if !options.close_stdin {
        except_fds.push(0);
    }
    if !options.close_stdout {
        except_fds.push(1);
    }
    if !options.close_stderr {
        except_fds.push(2);
    }

    // Close all file descriptors except specified ones
    close_all_files(&except_fds)?;

    // Redirect closed standard streams to /dev/null
    if options.close_stdin || options.close_stdout || options.close_stderr {
        let dev_null = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/null")?;

        let null_fd = std::os::unix::io::AsRawFd::as_raw_fd(&dev_null);

        if options.close_stdin {
            unsafe { libc::dup2(null_fd, 0) };
        }
        if options.close_stdout {
            unsafe { libc::dup2(null_fd, 1) };
        }
        if options.close_stderr {
            unsafe { libc::dup2(null_fd, 2) };
        }
    }

    // Write PID file if specified
    if let Some(ref pid_path) = options.pid_file {
        let pid = unsafe { libc::getpid() };
        let mut file = File::create(pid_path)?;
        writeln!(file, "{}", pid)?;

        // Set permissions to 644
        let mut perms = file.metadata()?.permissions();
        perms.set_mode(0o644);
        std::fs::set_permissions(pid_path, perms)?;
    }

    Ok(())
}

/// Daemonize the current process using default options.
///
/// Direct equivalent of C++ `ArchDaemonizeProcess()`. Calls [`daemonize`] with
/// [`DaemonOptions::default`]: closes stdin/stdout/stderr, chdir to `/`, umask 0.
///
/// On Windows returns `ErrorKind::Unsupported`.
pub fn daemonize_process() -> std::io::Result<()> {
    daemonize(&DaemonOptions::default())
}

/// Close all file descriptors except the ones specified.
///
/// This function closes all open file descriptors in the current process,
/// except for those listed in `except_fds`. Invalid file descriptors in
/// the exception list are ignored.
///
/// # Safety
///
/// This function is intended to be used after a `fork()` call to close
/// all unwanted file descriptors in the child process. It does NOT:
/// - Flush stdio buffers
/// - Wait for processes opened with popen
/// - Shut down X11 display connections
/// - Perform any cleanup
///
/// It simply closes the file descriptors. Use with caution.
///
/// # Arguments
///
/// * `except_fds` - Slice of file descriptor numbers to keep open
///
/// # Errors
///
/// Returns error if closing a file descriptor fails (other than EBADF).
/// On error, continues closing remaining descriptors and returns the last error.
///
/// # Example
///
/// ```no_run
/// use usd_arch::close_all_files;
///
/// // Close all except stdin(0), stdout(1), stderr(2)
/// close_all_files(&[0, 1, 2]).expect("Failed to close files");
/// ```
pub fn close_all_files(_except_fds: &[i32]) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        close_all_files_unix(_except_fds)
    }

    #[cfg(not(unix))]
    {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "File descriptor closing is not supported on this platform",
        ))
    }
}

#[cfg(unix)]
fn close_all_files_unix(except_fds: &[i32]) -> std::io::Result<()> {
    use std::io::ErrorKind;

    // Get maximum number of file descriptors
    let mut limits = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };

    let status = unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut limits) };

    if status != 0 {
        return Err(std::io::Error::last_os_error());
    }

    let maxfd = if limits.rlim_cur == libc::RLIM_INFINITY {
        // Use NOFILE constant as fallback
        #[cfg(target_os = "linux")]
        const NOFILE: u64 = 1024;
        #[cfg(target_os = "macos")]
        const NOFILE: u64 = 256;
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        const NOFILE: u64 = 256;

        NOFILE as i32
    } else {
        limits.rlim_cur as i32
    };

    // Find max exception fd for optimization
    let max_except = except_fds.iter().max().copied().unwrap_or(-1);

    let mut last_error: Option<std::io::Error> = None;

    for fd in 0..maxfd {
        // Check if this fd should be skipped
        if fd <= max_except && except_fds.contains(&fd) {
            continue;
        }

        // Close the file descriptor, retry if interrupted
        loop {
            let result = unsafe { libc::close(fd) };

            if result == 0 {
                break; // Success
            }

            let err = std::io::Error::last_os_error();

            match err.kind() {
                ErrorKind::Interrupted => continue, // Retry on EINTR
                _ if err.raw_os_error() == Some(libc::EBADF) => break, // fd wasn't open
                _ => {
                    // Real error - save it but continue closing others
                    last_error = Some(err);
                    break;
                }
            }
        }
    }

    // C++ restores errno after the loop; in Rust we propagate errors via
    // Result instead of errno, so explicit restoration is unnecessary.
    match last_error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_daemon_options_builder() {
        let opts = DaemonOptions::new()
            .close_stdin(false)
            .close_stdout(true)
            .close_stderr(true)
            .change_dir("/tmp")
            .umask(0o022);

        assert!(!opts.close_stdin);
        assert!(opts.close_stdout);
        assert!(opts.close_stderr);
        assert_eq!(opts.change_dir, PathBuf::from("/tmp"));
        assert_eq!(opts.umask, 0o022);
    }

    #[test]
    fn test_daemon_options_default() {
        let opts = DaemonOptions::default();

        assert!(opts.close_stdin);
        assert!(opts.close_stdout);
        assert!(opts.close_stderr);
        assert_eq!(opts.change_dir, PathBuf::from("/"));
        assert_eq!(opts.umask, 0);
        assert!(opts.pid_file.is_none());
    }

    #[test]
    #[cfg(unix)]
    fn test_close_all_files_with_exceptions() {
        // We can't really test this without risking closing important fds
        // Just test that it doesn't crash with valid input
        let result = close_all_files(&[0, 1, 2]);
        // Should succeed or fail gracefully
        let _ = result;
    }

    #[test]
    #[cfg(not(unix))]
    fn test_daemonize_unsupported() {
        let opts = DaemonOptions::default();
        let result = daemonize(&opts);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::Unsupported);
    }

    #[test]
    #[cfg(not(unix))]
    fn test_close_all_files_unsupported() {
        let result = close_all_files(&[0, 1, 2]);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::Unsupported);
    }
}
