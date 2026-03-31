//! Thread limits and concurrency control.
//!
//! This module provides functions to query and control the number of threads
//! used for parallel execution.
//!
//! # Environment Variable
//!
//! The `PXR_WORK_THREAD_LIMIT` environment variable can be used to set the
//! thread limit at startup. Values:
//! - `0`: Use all available cores (default)
//! - Positive number: Use exactly that many threads
//! - Negative number: Use all cores minus that many (e.g., `-2` leaves 2 cores free)
//!
//! # Examples
//!
//! ```
//! use usd_work::{get_concurrency_limit, has_concurrency, get_physical_concurrency_limit};
//!
//! // Query current limits
//! let limit = get_concurrency_limit();
//! let physical = get_physical_concurrency_limit();
//! let concurrent = has_concurrency();
//!
//! println!("Current limit: {}", limit);
//! println!("Physical cores: {}", physical);
//! println!("Has concurrency: {}", concurrent);
//! ```

use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Global concurrency limit. 0 means "use rayon default".
static CONCURRENCY_LIMIT: AtomicUsize = AtomicUsize::new(0);

/// Cached physical concurrency limit (logical cores respecting affinity).
static PHYSICAL_LIMIT: OnceLock<usize> = OnceLock::new();

/// Cached physical core count (excluding hyperthreads).
static PHYSICAL_CORE_COUNT: OnceLock<usize> = OnceLock::new();

/// Initialize thread limits from environment variable at first use.
fn init_from_env() {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        if let Ok(val) = std::env::var("PXR_WORK_THREAD_LIMIT") {
            if let Ok(n) = val.parse::<i32>() {
                if n != 0 {
                    set_concurrency_limit_argument(n);
                }
            }
        }
    });
}

/// Returns the current concurrency limit, always >= 1.
///
/// This value is determined by the underlying concurrency subsystem (rayon).
/// It may have been set by the `PXR_WORK_THREAD_LIMIT` environment variable,
/// by a call to [`set_concurrency_limit`], or by rayon's default configuration.
///
/// # Examples
///
/// ```
/// use usd_work::get_concurrency_limit;
///
/// let limit = get_concurrency_limit();
/// assert!(limit >= 1);
/// ```
pub fn get_concurrency_limit() -> usize {
    init_from_env();
    let limit = CONCURRENCY_LIMIT.load(Ordering::Relaxed);
    if limit == 0 {
        // Use rayon's current thread pool size
        rayon::current_num_threads()
    } else {
        limit
    }
}

/// Returns the concurrency limit setting that was explicitly configured.
///
/// Returns 0 if no explicit limit was set (meaning rayon's default is used).
///
/// # Examples
///
/// ```
/// use usd_work::get_concurrency_limit_setting;
///
/// let setting = get_concurrency_limit_setting();
/// // 0 means no explicit limit, uses rayon default
/// ```
pub fn get_concurrency_limit_setting() -> usize {
    init_from_env();
    CONCURRENCY_LIMIT.load(Ordering::Relaxed)
}

/// Returns true if the current concurrency limit is greater than 1.
///
/// This is useful for deciding whether to use parallel algorithms or
/// fall back to serial execution.
///
/// # Examples
///
/// ```
/// use usd_work::has_concurrency;
///
/// if has_concurrency() {
///     println!("Parallel execution available");
/// } else {
///     println!("Running single-threaded");
/// }
/// ```
pub fn has_concurrency() -> bool {
    get_concurrency_limit() > 1
}

/// Returns the number of physical execution cores available.
///
/// This is either the number of physical cores on the machine or the number
/// of cores specified by the process's affinity mask, whichever is smaller.
///
/// # Examples
///
/// ```
/// use usd_work::get_physical_concurrency_limit;
///
/// let cores = get_physical_concurrency_limit();
/// assert!(cores >= 1);
/// ```
pub fn get_physical_concurrency_limit() -> usize {
    *PHYSICAL_LIMIT.get_or_init(|| {
        // std::thread::available_parallelism() respects the process affinity mask
        // on both Linux and Windows, matching C++ TBB's default_concurrency().
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
    })
}

/// Returns the number of physical CPU cores (excluding hyperthreads).
///
/// On Windows, uses `GetLogicalProcessorInformation` to count physical cores.
/// On other platforms, falls back to `available_parallelism() / 2` (heuristic)
/// with a minimum of 1.
///
/// This is useful for setting default thread pool sizes where hyperthreading
/// provides diminishing returns.
pub fn get_physical_core_count() -> usize {
    *PHYSICAL_CORE_COUNT.get_or_init(|| {
        let count = detect_physical_cores();
        count.max(1)
    })
}

/// Platform-specific physical core detection.
#[cfg(target_os = "windows")]
fn detect_physical_cores() -> usize {
    // Use GetLogicalProcessorInformation to count physical cores on Windows.
    // This correctly handles hyperthreading by counting RelationProcessorCore entries.
    use std::mem;

    #[repr(C)]
    #[allow(non_snake_case)]
    struct SYSTEM_LOGICAL_PROCESSOR_INFORMATION {
        ProcessorMask: usize,
        Relationship: u32,
        _data: [u64; 2],
    }

    const RELATION_PROCESSOR_CORE: u32 = 0;

    #[allow(unsafe_code)] // SAFETY: Windows FFI for physical core detection
    unsafe extern "system" {
        fn GetLogicalProcessorInformation(
            buffer: *mut SYSTEM_LOGICAL_PROCESSOR_INFORMATION,
            return_length: *mut u32,
        ) -> i32;
    }

    let entry_size = mem::size_of::<SYSTEM_LOGICAL_PROCESSOR_INFORMATION>() as u32;
    let mut buf_len: u32 = 0;

    // First call to get required buffer size
    #[allow(unsafe_code)] // SAFETY: Win32 API call to query buffer size
    unsafe {
        GetLogicalProcessorInformation(std::ptr::null_mut(), &mut buf_len);
    }
    if buf_len == 0 {
        return fallback_core_count();
    }

    let count = (buf_len / entry_size) as usize;
    let mut buf = vec![0u8; buf_len as usize];
    let ptr = buf.as_mut_ptr() as *mut SYSTEM_LOGICAL_PROCESSOR_INFORMATION;

    #[allow(unsafe_code)] // SAFETY: Win32 API call with valid buffer
    let ok = unsafe { GetLogicalProcessorInformation(ptr, &mut buf_len) };
    if ok == 0 {
        return fallback_core_count();
    }

    #[allow(unsafe_code)] // SAFETY: ptr is valid, count calculated from buf_len
    let entries = unsafe { std::slice::from_raw_parts(ptr, count) };
    let physical = entries
        .iter()
        .filter(|e| e.Relationship == RELATION_PROCESSOR_CORE)
        .count();

    if physical > 0 {
        physical
    } else {
        fallback_core_count()
    }
}

#[cfg(target_os = "linux")]
fn detect_physical_cores() -> usize {
    // Parse /proc/cpuinfo to count unique physical cores.
    // Each "core id" + "physical id" pair represents one physical core.
    use std::collections::HashSet;
    use std::fs;

    let cpuinfo = match fs::read_to_string("/proc/cpuinfo") {
        Ok(s) => s,
        Err(_) => return fallback_core_count(),
    };

    let mut cores = HashSet::new();
    let mut physical_id: Option<String> = None;

    for line in cpuinfo.lines() {
        if let Some(val) = line.strip_prefix("physical id") {
            if let Some(v) = val.split(':').nth(1) {
                physical_id = Some(v.trim().to_string());
            }
        } else if let Some(val) = line.strip_prefix("core id") {
            if let Some(v) = val.split(':').nth(1) {
                let core_id = v.trim().to_string();
                let pid = physical_id.clone().unwrap_or_default();
                cores.insert((pid, core_id));
            }
        }
    }

    if cores.is_empty() {
        fallback_core_count()
    } else {
        cores.len()
    }
}

#[cfg(target_os = "macos")]
fn detect_physical_cores() -> usize {
    // Use sysctl hw.physicalcpu on macOS.
    use std::process::Command;

    let output = Command::new("sysctl")
        .arg("-n")
        .arg("hw.physicalcpu")
        .output();

    match output {
        Ok(out) => {
            let s = String::from_utf8_lossy(&out.stdout);
            s.trim()
                .parse::<usize>()
                .unwrap_or_else(|_| fallback_core_count())
        }
        Err(_) => fallback_core_count(),
    }
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn detect_physical_cores() -> usize {
    fallback_core_count()
}

/// Fallback: assume 2 logical cores per physical core (hyperthreading).
fn fallback_core_count() -> usize {
    let logical = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    (logical / 2).max(1)
}

/// Sets the concurrency limit to `n`, if `n` is a non-zero value.
///
/// If `n` is zero, the current concurrency limit is not changed.
///
/// # Warning
///
/// Calling this with `n > get_physical_concurrency_limit()` may overtax
/// the machine. In general, very few places should call this function.
///
/// # Note
///
/// Due to rayon's design, changing the thread pool size after initialization
/// requires building a new global thread pool, which may have performance
/// implications. This function stores the limit for use by work functions
/// but may not immediately affect rayon's thread pool.
///
/// # Examples
///
/// ```
/// use usd_work::{set_concurrency_limit, get_concurrency_limit};
///
/// // Set to 4 threads
/// set_concurrency_limit(4);
/// ```
pub fn set_concurrency_limit(n: usize) {
    if n == 0 {
        return;
    }
    CONCURRENCY_LIMIT.store(n, Ordering::Relaxed);

    // Try to configure rayon's thread pool
    // Note: This only works if the thread pool hasn't been initialized yet
    let _ = rayon::ThreadPoolBuilder::new()
        .num_threads(n)
        .build_global();
}

/// Sanitize `n` and set the concurrency limit accordingly.
///
/// This function is useful for interpreting command line arguments:
///
/// - If `n` is zero, do not change the current concurrency limit.
/// - If `n` is positive, call `set_concurrency_limit(n)`.
/// - If `n` is negative, set the limit to all but `abs(n)` cores.
///   For example, `-2` means use all but 2 cores.
///   If `abs(n)` is greater than the physical core count, sets limit to 1.
///
/// # Examples
///
/// ```
/// use usd_work::set_concurrency_limit_argument;
///
/// // Use all but 2 cores
/// set_concurrency_limit_argument(-2);
///
/// // Use exactly 4 threads
/// set_concurrency_limit_argument(4);
/// ```
pub fn set_concurrency_limit_argument(n: i32) {
    if n == 0 {
        return;
    }

    if n > 0 {
        set_concurrency_limit(n as usize);
    } else {
        let physical = get_physical_concurrency_limit();
        let reserved = (-n) as usize;
        let limit = if reserved >= physical {
            1
        } else {
            physical - reserved
        };
        set_concurrency_limit(limit);
    }
}

/// Sets the concurrency limit to the maximum recommended for the hardware.
///
/// Equivalent to `set_concurrency_limit(get_physical_concurrency_limit())`.
///
/// # Examples
///
/// ```
/// use usd_work::set_maximum_concurrency_limit;
///
/// set_maximum_concurrency_limit();
/// ```
pub fn set_maximum_concurrency_limit() {
    set_concurrency_limit(get_physical_concurrency_limit());
}

/// Returns true if granular thread limits between 1 and physical concurrency
/// can be set and respected.
///
/// With rayon, this is always true as it supports arbitrary thread counts.
///
/// # Examples
///
/// ```
/// use usd_work::supports_granular_thread_limits;
///
/// assert!(supports_granular_thread_limits());
/// ```
pub fn supports_granular_thread_limits() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_concurrency_limit() {
        let limit = get_concurrency_limit();
        assert!(limit >= 1);
    }

    #[test]
    fn test_get_physical_concurrency_limit() {
        let physical = get_physical_concurrency_limit();
        assert!(physical >= 1);
    }

    #[test]
    fn test_has_concurrency() {
        // On most machines this should be true
        let _ = has_concurrency();
    }

    #[test]
    fn test_supports_granular_thread_limits() {
        assert!(supports_granular_thread_limits());
    }

    #[test]
    fn test_set_concurrency_limit_zero() {
        let before = get_concurrency_limit();
        set_concurrency_limit(0);
        // Zero should not change the limit
        let after = get_concurrency_limit();
        assert_eq!(before, after);
    }

    #[test]
    fn test_set_concurrency_limit_argument_negative() {
        let physical = get_physical_concurrency_limit();
        if physical > 2 {
            set_concurrency_limit_argument(-1);
            // Should be physical - 1
            let limit = CONCURRENCY_LIMIT.load(Ordering::Relaxed);
            assert!(limit > 0);
        }
    }

    #[test]
    fn test_get_physical_core_count() {
        let cores = get_physical_core_count();
        assert!(cores >= 1, "must have at least 1 physical core");
        // Physical cores should not exceed logical cores
        let logical = get_physical_concurrency_limit();
        assert!(
            cores <= logical,
            "physical cores ({}) must be <= logical ({})",
            cores,
            logical
        );
    }

    #[test]
    fn test_physical_vs_logical() {
        let physical = get_physical_core_count();
        let logical = get_physical_concurrency_limit();
        // On most modern CPUs with HT, physical <= logical
        // On non-HT systems, physical == logical
        assert!(physical >= 1);
        assert!(logical >= 1);
        assert!(physical <= logical);
    }
}
