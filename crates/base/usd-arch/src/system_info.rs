// SAFETY: This module provides FFI bindings to system APIs requiring unsafe
#![allow(unsafe_code)]

//! System information utilities.
//!
//! Provides functions for querying system properties like memory, paths, and hardware info.

use std::env;
use std::path::{Path, PathBuf};

/// Returns the current working directory.
///
/// # Examples
///
/// ```
/// use usd_arch::get_cwd;
///
/// if let Some(cwd) = get_cwd() {
///     println!("Current directory: {}", cwd.display());
/// }
/// ```
#[must_use]
pub fn get_cwd() -> Option<PathBuf> {
    env::current_dir().ok()
}

/// Sets the current working directory.
///
/// # Errors
///
/// Returns an error if the directory doesn't exist or is not accessible.
pub fn set_cwd(path: &Path) -> std::io::Result<()> {
    env::set_current_dir(path)
}

/// Returns the path to the current executable.
///
/// # Examples
///
/// ```
/// use usd_arch::get_executable_path;
///
/// if let Some(path) = get_executable_path() {
///     println!("Executable: {}", path.display());
/// }
/// ```
#[must_use]
pub fn get_executable_path() -> Option<PathBuf> {
    env::current_exe().ok()
}

/// Returns the directory containing the current executable.
#[must_use]
pub fn get_executable_dir() -> Option<PathBuf> {
    get_executable_path().and_then(|p| p.parent().map(|p| p.to_path_buf()))
}

/// Returns the system's temporary directory.
///
/// # Examples
///
/// ```
/// use usd_arch::get_temp_dir;
///
/// let tmp = get_temp_dir();
/// println!("Temp dir: {}", tmp.display());
/// ```
#[must_use]
pub fn get_temp_dir() -> PathBuf {
    env::temp_dir()
}

/// Returns the user's home directory.
///
/// # Platform Notes
///
/// - On Unix: Returns `$HOME`
/// - On Windows: Returns `%USERPROFILE%`
#[must_use]
pub fn get_home_dir() -> Option<PathBuf> {
    #[cfg(unix)]
    {
        env::var_os("HOME").map(PathBuf::from)
    }
    #[cfg(windows)]
    {
        env::var_os("USERPROFILE").map(PathBuf::from)
    }
    #[cfg(not(any(unix, windows)))]
    {
        None
    }
}

/// Returns the system's page size in bytes.
///
/// This is useful for memory-mapped I/O and aligned allocations.
///
/// # Examples
///
/// ```
/// use usd_arch::get_page_size;
///
/// let page_size = get_page_size();
/// println!("Page size: {} bytes", page_size);
/// ```
#[must_use]
pub fn get_page_size() -> usize {
    #[cfg(unix)]
    {
        // SAFETY: sysconf is safe to call
        unsafe {
            let size = libc::sysconf(libc::_SC_PAGESIZE);
            if size > 0 {
                size as usize
            } else {
                4096 // Fallback
            }
        }
    }
    #[cfg(windows)]
    {
        use std::mem::MaybeUninit;
        // SAFETY: GetSystemInfo is safe to call with a valid pointer
        unsafe {
            let mut info = MaybeUninit::uninit();
            windows_sys::Win32::System::SystemInformation::GetSystemInfo(info.as_mut_ptr());
            info.assume_init().dwPageSize as usize
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        4096 // Reasonable default
    }
}

/// Returns the total physical memory in bytes.
///
/// # Examples
///
/// ```
/// use usd_arch::get_physical_memory;
///
/// if let Some(mem) = get_physical_memory() {
///     println!("Physical memory: {} GB", mem / (1024 * 1024 * 1024));
/// }
/// ```
#[must_use]
pub fn get_physical_memory() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        // Read from /proc/meminfo
        if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
            for line in content.lines() {
                if line.starts_with("MemTotal:") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        if let Ok(kb) = parts[1].parse::<u64>() {
                            return Some(kb * 1024);
                        }
                    }
                }
            }
        }
        None
    }
    #[cfg(target_os = "macos")]
    {
        // SAFETY: sysctl is safe to call
        unsafe {
            let mut mem: u64 = 0;
            let mut size = std::mem::size_of::<u64>();
            let mut mib = [libc::CTL_HW, libc::HW_MEMSIZE];
            if libc::sysctl(
                mib.as_mut_ptr(),
                2,
                &mut mem as *mut u64 as *mut libc::c_void,
                &mut size,
                std::ptr::null_mut(),
                0,
            ) == 0
            {
                Some(mem)
            } else {
                None
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        use std::mem::MaybeUninit;
        // SAFETY: GlobalMemoryStatusEx is safe to call with a valid pointer
        unsafe {
            let mut status = MaybeUninit::<
                windows_sys::Win32::System::SystemInformation::MEMORYSTATUSEX,
            >::uninit();
            (*status.as_mut_ptr()).dwLength = std::mem::size_of::<
                windows_sys::Win32::System::SystemInformation::MEMORYSTATUSEX,
            >() as u32;
            if windows_sys::Win32::System::SystemInformation::GlobalMemoryStatusEx(
                status.as_mut_ptr(),
            ) != 0
            {
                Some(status.assume_init().ullTotalPhys)
            } else {
                None
            }
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        None
    }
}

/// Returns the amount of available (free) physical memory in bytes.
#[must_use]
pub fn get_available_memory() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
            for line in content.lines() {
                if line.starts_with("MemAvailable:") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        if let Ok(kb) = parts[1].parse::<u64>() {
                            return Some(kb * 1024);
                        }
                    }
                }
            }
        }
        None
    }
    #[cfg(target_os = "macos")]
    {
        // Use sysctlbyname("vm.page_free_count") * page_size
        unsafe {
            let mut free_count: u64 = 0;
            let mut size = std::mem::size_of::<u64>();
            let name = b"vm.page_free_count\0";
            if libc::sysctlbyname(
                name.as_ptr() as *const libc::c_char,
                &mut free_count as *mut u64 as *mut libc::c_void,
                &mut size,
                std::ptr::null_mut(),
                0,
            ) == 0
            {
                Some(free_count * get_page_size() as u64)
            } else {
                None
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        use std::mem::MaybeUninit;
        unsafe {
            let mut status = MaybeUninit::<
                windows_sys::Win32::System::SystemInformation::MEMORYSTATUSEX,
            >::uninit();
            (*status.as_mut_ptr()).dwLength = std::mem::size_of::<
                windows_sys::Win32::System::SystemInformation::MEMORYSTATUSEX,
            >() as u32;
            if windows_sys::Win32::System::SystemInformation::GlobalMemoryStatusEx(
                status.as_mut_ptr(),
            ) != 0
            {
                Some(status.assume_init().ullAvailPhys)
            } else {
                None
            }
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        None
    }
}

/// Returns the hostname of the system.
#[must_use]
pub fn get_hostname() -> Option<String> {
    #[cfg(unix)]
    {
        let mut buf = [0u8; 256];
        // SAFETY: gethostname is safe to call with a valid buffer
        unsafe {
            if libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len()) == 0 {
                let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
                String::from_utf8(buf[..len].to_vec()).ok()
            } else {
                None
            }
        }
    }
    #[cfg(windows)]
    {
        env::var("COMPUTERNAME").ok()
    }
    #[cfg(not(any(unix, windows)))]
    {
        None
    }
}

/// Returns the username of the current user.
#[must_use]
pub fn get_username() -> Option<String> {
    #[cfg(unix)]
    {
        env::var("USER").ok()
    }
    #[cfg(windows)]
    {
        env::var("USERNAME").ok()
    }
    #[cfg(not(any(unix, windows)))]
    {
        None
    }
}

/// Returns the process ID of the current process.
#[must_use]
pub fn get_pid() -> u32 {
    std::process::id()
}

/// Returns the parent process ID.
#[cfg(unix)]
#[must_use]
pub fn get_ppid() -> u32 {
    // SAFETY: getppid is always safe to call
    unsafe { libc::getppid() as u32 }
}

/// Returns the parent process ID.
///
/// On Windows, uses `CreateToolhelp32Snapshot` + `Process32First/Next`
/// to walk the process list and find the parent of the current PID.
#[cfg(windows)]
#[must_use]
pub fn get_ppid() -> u32 {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32, Process32First, Process32Next, TH32CS_SNAPPROCESS,
    };

    // SAFETY: Win32 snapshot API is safe when called with valid parameters.
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
            return 0;
        }

        let mut entry: PROCESSENTRY32 = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32>() as u32;

        let my_pid = std::process::id();

        if Process32First(snap, &mut entry) != 0 {
            loop {
                if entry.th32ProcessID == my_pid {
                    let ppid = entry.th32ParentProcessID;
                    CloseHandle(snap);
                    return ppid;
                }
                if Process32Next(snap, &mut entry) == 0 {
                    break;
                }
            }
        }

        CloseHandle(snap);
        0
    }
}

/// Returns the parent process ID (unsupported platform stub).
#[cfg(not(any(unix, windows)))]
#[must_use]
pub fn get_ppid() -> u32 {
    0
}

/// Information about the current system.
#[derive(Debug, Clone)]
pub struct SystemInfo {
    /// Operating system name
    pub os_name: &'static str,
    /// CPU architecture
    pub arch: &'static str,
    /// Number of logical CPUs
    pub cpu_count: usize,
    /// Page size in bytes
    pub page_size: usize,
    /// Total physical memory in bytes
    pub physical_memory: u64,
    /// Available physical memory in bytes
    pub available_memory: u64,
    /// Hostname
    pub hostname: String,
    /// Username
    pub username: String,
    /// Process ID
    pub pid: u32,
}

impl SystemInfo {
    /// Gathers current system information.
    #[must_use]
    pub fn collect() -> Self {
        Self {
            os_name: std::env::consts::OS,
            arch: std::env::consts::ARCH,
            cpu_count: super::threads::get_concurrency(),
            page_size: get_page_size(),
            physical_memory: get_physical_memory().unwrap_or(0),
            available_memory: get_available_memory().unwrap_or(0),
            hostname: get_hostname().unwrap_or_default(),
            username: get_username().unwrap_or_default(),
            pid: get_pid(),
        }
    }
}

impl std::fmt::Display for SystemInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "OS: {}", self.os_name)?;
        writeln!(f, "Architecture: {}", self.arch)?;
        writeln!(f, "CPU count: {}", self.cpu_count)?;
        writeln!(f, "Page size: {} bytes", self.page_size)?;
        writeln!(
            f,
            "Physical memory: {} MB",
            self.physical_memory / (1024 * 1024)
        )?;
        writeln!(
            f,
            "Available memory: {} MB",
            self.available_memory / (1024 * 1024)
        )?;
        writeln!(f, "Hostname: {}", self.hostname)?;
        writeln!(f, "Username: {}", self.username)?;
        writeln!(f, "PID: {}", self.pid)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_cwd() {
        let cwd = get_cwd();
        assert!(cwd.is_some());
        assert!(cwd.unwrap().exists());
    }

    #[test]
    fn test_get_executable_path() {
        let path = get_executable_path();
        assert!(path.is_some());
        assert!(path.unwrap().exists());
    }

    #[test]
    fn test_get_temp_dir() {
        let tmp = get_temp_dir();
        assert!(tmp.exists());
    }

    #[test]
    fn test_get_page_size() {
        let size = get_page_size();
        assert!(size > 0);
        assert!(size.is_power_of_two());
    }

    #[test]
    fn test_get_pid() {
        let pid = get_pid();
        assert!(pid > 0);
    }

    #[test]
    fn test_system_info() {
        let info = SystemInfo::collect();
        assert!(!info.os_name.is_empty());
        assert!(!info.arch.is_empty());
        assert!(info.cpu_count > 0);
        assert!(info.page_size > 0);
    }

    #[test]
    fn test_get_home_dir() {
        // This might be None in some CI environments, but should work locally
        let home = get_home_dir();
        if let Some(path) = home {
            assert!(path.exists());
        }
    }
}
