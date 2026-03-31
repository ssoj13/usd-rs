// Ported to Rust by Claude Code (2026)
//

// SAFETY: This module provides FFI bindings to system APIs requiring unsafe
#![allow(unsafe_code)]

//! Debugger interaction utilities.
//!
//! Provides platform-specific functions for interacting with debuggers:
//! - Detecting if a debugger is attached
//! - Triggering debug breakpoints
//! - Attaching a debugger via ARCH_DEBUGGER env var
//! - Aborting with optional backtrace logging

use std::sync::atomic::{AtomicBool, Ordering};

/// Global atomic flag for controlling wait behavior in debug traps
static DEBUGGER_WAIT: AtomicBool = AtomicBool::new(false);

/// Returns true if ARCH_AVOID_JIT is set in the environment.
#[inline]
fn avoid_jit() -> bool {
    std::env::var_os("ARCH_AVOID_JIT").is_some()
}

/// Check if a debugger is currently attached to the process.
///
/// - **Windows**: `IsDebuggerPresent()`
/// - **Linux**: reads `TracerPid` from `/proc/self/status`
/// - **macOS**: `sysctl` KERN_PROC_PID + P_TRACED flag
#[inline(never)]
pub fn arch_debugger_is_attached() -> bool {
    #[cfg(target_os = "windows")]
    {
        // SAFETY: IsDebuggerPresent is always safe to call
        unsafe { windows_sys::Win32::System::Diagnostics::Debug::IsDebuggerPresent() != 0 }
    }

    #[cfg(target_os = "linux")]
    {
        read_tracer_pid().unwrap_or(0) > 0
    }

    #[cfg(target_os = "macos")]
    {
        is_being_debugged_darwin()
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        false
    }
}

/// Trigger a debug breakpoint.
///
/// If the wait flag (set by `arch_debugger_wait(true)`) is active, the
/// process will stop (SIGSTOP on POSIX / DebugBreak on Windows) and wait
/// for a debugger to attach. Otherwise issues a normal trap instruction.
#[inline(never)]
pub fn arch_debugger_trap() {
    // C++ parity: CAS wait flag; if it was true, stop and wait for debugger
    if DEBUGGER_WAIT
        .compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        #[cfg(unix)]
        {
            // SAFETY: raise(SIGSTOP) suspends until resumed by a debugger/signal
            unsafe { libc::raise(libc::SIGSTOP) };
        }
        #[cfg(windows)]
        {
            // SAFETY: DebugBreak triggers a breakpoint exception handled by the debugger
            unsafe { windows_sys::Win32::System::Diagnostics::Debug::DebugBreak() };
        }
        return;
    }

    // Normal synchronous trap
    #[cfg(target_os = "windows")]
    unsafe {
        core::arch::asm!("int3");
    }

    #[cfg(all(
        not(target_os = "windows"),
        any(target_arch = "x86", target_arch = "x86_64")
    ))]
    unsafe {
        core::arch::asm!("int 3");
    }

    #[cfg(all(
        not(target_os = "windows"),
        not(any(target_arch = "x86", target_arch = "x86_64")),
        unix
    ))]
    unsafe {
        libc::raise(libc::SIGTRAP);
    }

    #[cfg(not(any(
        target_os = "windows",
        any(target_arch = "x86", target_arch = "x86_64"),
        unix
    )))]
    {
        // No trap instruction available on this platform
    }
}

/// Configure whether debug traps should wait for debugger attachment.
///
/// When `wait` is `true`, the next `arch_debugger_trap()` call stops the
/// process (SIGSTOP / DebugBreak) until a debugger continues it. The flag
/// resets automatically after being consumed.
pub fn arch_debugger_wait(wait: bool) {
    DEBUGGER_WAIT.store(wait, Ordering::SeqCst);
}

/// Attempt to attach a debugger to the current process.
///
/// Reads the `ARCH_DEBUGGER` environment variable and launches it via the
/// system shell (POSIX: `/bin/sh -c`; Windows: `cmd /c`). Substitutions:
/// - `%p` — replaced with the current process ID
/// - `%e` — replaced with the path to the current executable
///
/// On POSIX the debugger is launched as an unrelated process (double-fork)
/// so it is re-parented to init and can actually attach. After launching,
/// the function sleeps 5 s to give the debugger time to connect.
///
/// Returns `true` if `ARCH_DEBUGGER` was set and the command was launched
/// successfully, `false` otherwise (including when `ARCH_AVOID_JIT` is set).
///
/// Parity: C++ `ArchDebuggerAttach()` in debugger.cpp.
#[inline(never)]
pub fn arch_debugger_attach() -> bool {
    // C++ parity: returns false immediately if ARCH_AVOID_JIT is set
    if avoid_jit() {
        return false;
    }
    // Already attached — nothing more to do
    if arch_debugger_is_attached() {
        return true;
    }
    attach_impl()
}

/// Platform-specific attach logic.
#[cfg(unix)]
fn attach_impl() -> bool {
    let template = match std::env::var("ARCH_DEBUGGER") {
        Ok(v) if !v.is_empty() => v,
        _ => return false,
    };

    let pid = std::process::id();
    let exe = std::env::current_exe()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();

    // Expand %p -> pid, %e -> exe path (matches C++ Arch_InitDebuggerAttach)
    let cmd = expand_debugger_template(&template, pid, &exe);

    // Launch via /bin/sh in a detached grandchild (double-fork) so the
    // debugger is not a child of this process — otherwise it cannot attach.
    let result = unsafe { double_fork_exec(&cmd) };
    if result {
        // Give the debugger time to attach (mirrors C++ sleep(5))
        std::thread::sleep(std::time::Duration::from_secs(5));
    }
    result
}

#[cfg(windows)]
fn attach_impl() -> bool {
    // On Windows the C++ code simply calls DebugBreak() when ARCH_DEBUGGER
    // is present, which triggers the JIT (Just-In-Time) debugger configured
    // in the registry (AeDebug).
    if std::env::var_os("ARCH_DEBUGGER").is_none() {
        return false;
    }
    // SAFETY: DebugBreak raises a breakpoint exception; the OS JIT debugger
    // (if configured) will catch it and attach.
    unsafe { windows_sys::Win32::System::Diagnostics::Debug::DebugBreak() };
    true
}

#[cfg(not(any(unix, windows)))]
fn attach_impl() -> bool {
    false
}

/// Replace `%p` with the decimal PID and `%e` with the exe path.
#[cfg(unix)]
fn expand_debugger_template(template: &str, pid: u32, exe: &str) -> String {
    let mut out = String::with_capacity(template.len() + 32);
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            match chars.peek() {
                Some(&'p') => {
                    chars.next();
                    out.push_str(&pid.to_string());
                }
                Some(&'e') => {
                    chars.next();
                    out.push_str(exe);
                }
                _ => out.push(c),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Fork twice so the grandchild is re-parented to init, then exec /bin/sh.
///
/// SAFETY: Called between fork() calls — only async-signal-safe operations
/// are performed (write, _exit, execve). No heap allocation after the first
/// fork.
#[cfg(unix)]
unsafe fn double_fork_exec(cmd: &str) -> bool {
    unsafe {
        // First fork: become parent of an intermediate child
        let child = libc::fork();
        if child < 0 {
            return false;
        }
        if child > 0 {
            // Parent: wait for the intermediate child to exit (it exits quickly)
            let mut status: libc::c_int = 0;
            libc::waitpid(child, &mut status, 0);
            // Success is signalled by the intermediate child exiting with 0
            return libc::WIFEXITED(status) && libc::WEXITSTATUS(status) == 0;
        }

        // --- Intermediate child ---
        // Become session leader so the grandchild has no controlling terminal
        libc::setsid();

        // Second fork: grandchild is not the session leader and is re-parented
        // to init when we exit here
        let grandchild = libc::fork();
        if grandchild < 0 {
            libc::_exit(1);
        }
        if grandchild > 0 {
            // Intermediate child exits — grandchild is now owned by init
            libc::_exit(0);
        }

        // --- Grandchild: exec the debugger command ---
        // Build argv: ["sh", "-c", cmd, NULL]
        // Use a fixed-size stack buffer — no heap allocation
        let sh = b"/bin/sh\0";
        let c_flag = b"-c\0";
        // Copy cmd into a stack buffer (max 4096 bytes)
        let mut buf = [0u8; 4096];
        let len = cmd.len().min(buf.len() - 1);
        buf[..len].copy_from_slice(&cmd.as_bytes()[..len]);

        let argv: [*const libc::c_char; 4] = [
            sh.as_ptr() as *const libc::c_char,
            c_flag.as_ptr() as *const libc::c_char,
            buf.as_ptr() as *const libc::c_char,
            std::ptr::null(),
        ];
        libc::execve(
            sh.as_ptr() as *const libc::c_char,
            argv.as_ptr(),
            std::ptr::null(),
        );
        // execve only returns on failure
        libc::_exit(1);
    }
}

/// Abort the process.
///
/// Mirrors C++ `ArchAbort(bool logging)`:
/// - If `ARCH_AVOID_JIT` is set **and** no debugger is attached, calls
///   `_exit(134)` to sidestep the JIT debugger dialog.
/// - Otherwise resets the SIGABRT handler (when `logging=false`) to skip
///   crash-reporting hooks, then calls `abort()`/`process::abort()`.
/// - When `logging=true`, captures and prints a backtrace before aborting.
pub fn arch_abort(logging: bool) -> ! {
    let debugger_attached = arch_debugger_is_attached();

    if avoid_jit() && !debugger_attached {
        // Avoid the JIT debugger popup — _exit(134) skips atexit handlers/destructors
        #[cfg(unix)]
        unsafe {
            libc::_exit(134)
        }
        #[cfg(windows)]
        unsafe {
            windows_sys::Win32::System::Threading::ExitProcess(134)
        }
        #[cfg(not(any(unix, windows)))]
        std::process::exit(134);
    }

    if logging {
        // Print a backtrace to stderr before dying
        let bt = std::backtrace::Backtrace::capture();
        // Only emit if capture succeeded (requires RUST_BACKTRACE=1 or =full)
        if bt.status() == std::backtrace::BacktraceStatus::Captured {
            eprintln!("[arch_abort] backtrace:\n{bt}");
        }
    } else {
        // Reset SIGABRT to SIG_DFL so crash-logging signal handlers are skipped
        #[cfg(unix)]
        {
            // SAFETY: resetting a signal to its default action is always safe
            unsafe { libc::signal(libc::SIGABRT, libc::SIG_DFL) };
        }
    }

    std::process::abort();
}

//
// Platform-specific helpers
//

#[cfg(target_os = "linux")]
fn read_tracer_pid() -> Option<i32> {
    use std::io::BufRead;

    let file = std::fs::File::open("/proc/self/status").ok()?;
    let reader = std::io::BufReader::new(file);

    for line in reader.lines() {
        let line = line.ok()?;
        if let Some(value) = line.strip_prefix("TracerPid:") {
            return value.trim().parse::<i32>().ok();
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn is_being_debugged_darwin() -> bool {
    use libc::{CTL_KERN, KERN_PROC, KERN_PROC_PID, c_int, c_void, getpid, sysctl};

    // From Apple Technical Q&A QA1361
    // P_TRACED flag from darwin-xnu/bsd/sys/proc.h
    const P_TRACED: i32 = 0x00000800;

    // kinfo_proc is large (~648 bytes on x86_64). We only need kp_proc.p_flag
    // at a known offset. Use raw byte buffer + sysctl to avoid depending on
    // libc::kinfo_proc which may be absent in some libc crate builds.
    const KINFO_PROC_SIZE: usize = 648;
    // Offset of kp_proc.p_flag within kinfo_proc (macOS x86_64 and arm64)
    const P_FLAG_OFFSET: usize = 16; // offsetof(kinfo_proc, kp_proc.p_flag)

    let mut buf = [0u8; KINFO_PROC_SIZE];
    let mut size = KINFO_PROC_SIZE;
    let mib = [CTL_KERN, KERN_PROC, KERN_PROC_PID, unsafe { getpid() }
        as c_int];

    let rc = unsafe {
        sysctl(
            mib.as_ptr() as *mut c_int,
            4,
            buf.as_mut_ptr() as *mut c_void,
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };

    if rc != 0 || size < P_FLAG_OFFSET + 4 {
        return false;
    }

    // Read p_flag as i32 at the known offset
    let p_flag = i32::from_ne_bytes([
        buf[P_FLAG_OFFSET],
        buf[P_FLAG_OFFSET + 1],
        buf[P_FLAG_OFFSET + 2],
        buf[P_FLAG_OFFSET + 3],
    ]);
    (p_flag & P_TRACED) != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_attached_no_crash() {
        // Must not panic regardless of environment
        let _ = arch_debugger_is_attached();
    }

    #[test]
    fn test_wait_flag_roundtrip() {
        arch_debugger_wait(true);
        assert!(DEBUGGER_WAIT.load(Ordering::SeqCst));
        arch_debugger_wait(false);
        assert!(!DEBUGGER_WAIT.load(Ordering::SeqCst));
    }

    #[test]
    fn test_attach_without_env_returns_false() {
        // Without ARCH_DEBUGGER set, attach should return false
        // (guards against accidentally spawning a process in CI)
        // SAFETY: single-threaded test, no concurrent env access
        unsafe {
            std::env::remove_var("ARCH_DEBUGGER");
            std::env::remove_var("ARCH_AVOID_JIT");
        }
        // attach returns false when env var is absent
        assert!(!attach_impl());
    }

    #[cfg(unix)]
    #[test]
    fn test_expand_template() {
        let pid = 12345u32;
        let exe = "/usr/bin/foo";
        let out = expand_debugger_template("gdb -p %p %e", pid, exe);
        assert_eq!(out, "gdb -p 12345 /usr/bin/foo");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_read_tracer_pid_present() {
        // TracerPid line exists in /proc/self/status on Linux
        let pid = read_tracer_pid();
        assert!(pid.is_some());
        assert!(pid.unwrap() >= 0);
    }

    // Note: arch_debugger_trap() and arch_abort() terminate the process;
    // they are exercised only in dedicated integration tests.
}
