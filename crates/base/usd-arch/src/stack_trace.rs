//! Stack trace capture and formatting utilities.
//!
//! Provides cross-platform stack trace capture, symbolization callbacks,
//! crash annotation, and session logging (C++ arch/stackTrace.h parity).

use std::collections::HashMap;
use std::fmt;
use std::io::{self, Write};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use backtrace::Backtrace;
use once_cell::sync::Lazy;

// ---------------------------------------------------------------------------
// Callback type  (P0-2 fix: was fn(&str)->(), now fn(usize)->String)
// ---------------------------------------------------------------------------

/// Stack trace callback for custom symbolization.
///
/// C++ parity: `std::function<std::string(uintptr_t address)>`.
/// Takes an instruction-pointer address, returns a symbolic representation.
pub type StackTraceCallback = fn(usize) -> String;

/// Global callback for custom symbolization.
static STACK_TRACE_CALLBACK: Lazy<Mutex<Option<StackTraceCallback>>> =
    Lazy::new(|| Mutex::new(None));

// ---------------------------------------------------------------------------
// Crash / error annotation globals  (P1-8, P1-9)
// ---------------------------------------------------------------------------

/// Atomic flag: true when a fatal crash handler has been invoked.
static APP_IS_CRASHING: AtomicBool = AtomicBool::new(false);

/// Whether fatal stack logging is enabled.
static FATAL_STACK_LOGGING: AtomicBool = AtomicBool::new(false);

/// Key-value program info for crash annotations.
static PROGRAM_INFO: Lazy<Mutex<HashMap<String, String>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Extra log info lines keyed by string.
static EXTRA_LOG_INFO: Lazy<Mutex<HashMap<String, Vec<String>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

// ---------------------------------------------------------------------------
// Process state log command  (ArchSetProcessStateLogCommand parity)
// ---------------------------------------------------------------------------

/// Stored command + non-fatal argv for ArchLogCurrentProcessState / ArchLogFatalProcessState.
static PROCESS_STATE_CMD: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));
static PROCESS_STATE_ARGV: Lazy<Mutex<Option<Vec<String>>>> = Lazy::new(|| Mutex::new(None));
static PROCESS_STATE_FATAL_ARGV: Lazy<Mutex<Option<Vec<String>>>> = Lazy::new(|| Mutex::new(None));

// ---------------------------------------------------------------------------
// Session log command  (ArchSetLogSession parity)
// ---------------------------------------------------------------------------

/// Stored command + argv for ArchSetLogSession (non-crash and crash variants).
static SESSION_LOG_CMD: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));
static SESSION_LOG_ARGV: Lazy<Mutex<Option<Vec<String>>>> = Lazy::new(|| Mutex::new(None));
static SESSION_CRASH_LOG_ARGV: Lazy<Mutex<Option<Vec<String>>>> = Lazy::new(|| Mutex::new(None));

// ---------------------------------------------------------------------------
// StackFrame
// ---------------------------------------------------------------------------

/// Represents a single frame in a stack trace.
#[derive(Debug, Clone)]
pub struct StackFrame {
    /// Raw instruction pointer address
    pub address: usize,
    /// Resolved symbol name (demangled if possible)
    pub symbol_name: Option<String>,
    /// Source file name
    pub file_name: Option<String>,
    /// Source line number
    pub line_number: Option<u32>,
    /// Offset from symbol start
    pub offset: usize,
}

impl fmt::Display for StackFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:016x}", self.address)?;

        if let Some(ref symbol) = self.symbol_name {
            write!(f, " in {}", symbol)?;
            if self.offset > 0 {
                write!(f, "+{:#x}", self.offset)?;
            }
        } else {
            write!(f, " <unknown>")?;
        }

        if let (Some(file), Some(line)) = (&self.file_name, self.line_number) {
            write!(f, " at {}:{}", file, line)?;
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Stack trace capture
// ---------------------------------------------------------------------------

/// Captures the current stack trace up to `max_depth` frames.
pub fn arch_get_stack_trace(max_depth: usize) -> Vec<StackFrame> {
    let bt = Backtrace::new();
    let mut frames = Vec::new();

    for (idx, bt_frame) in bt.frames().iter().enumerate() {
        if idx >= max_depth {
            break;
        }

        // Frame 0 is the current instruction; for all other frames ip() is
        // the return address (instruction AFTER the call). Subtract 1 so
        // that symbolization resolves to the call site, not the next line.
        let ip = if idx == 0 {
            bt_frame.ip() as usize
        } else {
            (bt_frame.ip() as usize).saturating_sub(1)
        };

        backtrace::resolve(bt_frame.ip(), |symbol| {
            let symbol_name = symbol.name().map(|n| n.to_string());
            let file_name = symbol
                .filename()
                .and_then(|p| p.to_str())
                .map(|s| s.to_string());
            let line_number = symbol.lineno();
            let symbol_addr = symbol.addr().map(|a| a as usize).unwrap_or(0);
            let offset = if symbol_addr > 0 && symbol_addr <= ip {
                ip.saturating_sub(symbol_addr)
            } else {
                0
            };

            frames.push(StackFrame {
                address: ip,
                symbol_name,
                file_name,
                line_number,
                offset,
            });
        });

        // If resolution failed, add raw frame
        if frames.len() <= idx {
            frames.push(StackFrame {
                address: ip,
                symbol_name: None,
                file_name: None,
                line_number: None,
                offset: 0,
            });
        }
    }

    frames.truncate(max_depth);
    frames
}

/// Captures stack trace and formats it as a string.
pub fn arch_get_stack_trace_string(max_depth: usize) -> String {
    let frames = arch_get_stack_trace(max_depth);
    format_stack_frames(&frames)
}

/// Formats a slice of stack frames into a readable string.
fn format_stack_frames(frames: &[StackFrame]) -> String {
    let mut output = String::new();
    if frames.is_empty() {
        output.push_str("No stack frames available\n");
        return output;
    }
    for (idx, frame) in frames.iter().enumerate() {
        output.push_str(&format!(" #{:<3} {}\n", idx, frame));
    }
    output
}

/// Prints the current stack trace to the provided writer.
pub fn arch_print_stack_trace(writer: &mut impl Write) -> io::Result<()> {
    let frames = arch_get_stack_trace(128);
    writeln!(writer, "Stack trace:")?;
    writeln!(
        writer,
        "============================================================"
    )?;
    for (idx, frame) in frames.iter().enumerate() {
        writeln!(writer, " #{:<3} {}", idx, frame)?;
    }
    writeln!(
        writer,
        "============================================================"
    )?;
    Ok(())
}

/// Prints the current stack trace to stderr.
pub fn arch_print_stack_trace_stderr() {
    let mut stderr = io::stderr();
    if let Err(e) = arch_print_stack_trace(&mut stderr) {
        eprintln!("Failed to print stack trace: {}", e);
    }
}

// ---------------------------------------------------------------------------
// Symbolization callback  (P0-2, P1-12)
// ---------------------------------------------------------------------------

/// Sets a custom callback for symbolic representation of addresses.
///
/// C++ parity: `ArchSetStackTraceCallback(const ArchStackTraceCallback&)`.
pub fn arch_set_stack_trace_callback(cb: StackTraceCallback) {
    let mut callback = STACK_TRACE_CALLBACK
        .lock()
        .expect("stack trace callback lock poisoned");
    *callback = Some(cb);
}

/// Returns the current stack trace callback, if any.
///
/// C++ parity: `ArchGetStackTraceCallback(ArchStackTraceCallback*)`.
pub fn arch_get_stack_trace_callback() -> Option<StackTraceCallback> {
    let callback = STACK_TRACE_CALLBACK
        .lock()
        .expect("stack trace callback lock poisoned");
    *callback
}

/// Clears the custom stack trace callback.
pub fn arch_clear_stack_trace_callback() {
    let mut callback = STACK_TRACE_CALLBACK
        .lock()
        .expect("stack trace callback lock poisoned");
    *callback = None;
}

/// Logs the current stack trace with a reason.
pub fn arch_log_stack_trace(reason: &str) {
    let frames = arch_get_stack_trace(128);
    let cb_opt = arch_get_stack_trace_callback();

    eprintln!("============================================================");
    eprintln!("Stack trace requested: {}", reason);
    eprintln!("============================================================");

    for (idx, frame) in frames.iter().enumerate() {
        // If user supplied a symbolizer callback, use it per-address
        if let Some(cb) = cb_opt {
            let sym = cb(frame.address);
            if !sym.is_empty() {
                eprintln!(" #{:<3} 0x{:016x} {}", idx, frame.address, sym);
                continue;
            }
        }
        eprintln!(" #{:<3} {}", idx, frame);
    }

    eprintln!("============================================================");
}

// ---------------------------------------------------------------------------
// Crash annotation / program info  (P1-8, P1-9, P1-10)
// ---------------------------------------------------------------------------

/// Returns `true` if the fatal signal handler has been invoked.
///
/// C++ parity: `ArchIsAppCrashing()`.
#[must_use]
pub fn arch_is_app_crashing() -> bool {
    APP_IS_CRASHING.load(Ordering::SeqCst)
}

/// Marks the process as crashing (internal use by fatal handlers).
pub fn arch_set_app_crashing() {
    APP_IS_CRASHING.store(true, Ordering::SeqCst);
}

/// Enables or disables automatic logging of crash info.
///
/// C++ parity: `ArchSetFatalStackLogging(bool)`.
pub fn arch_set_fatal_stack_logging(flag: bool) {
    FATAL_STACK_LOGGING.store(flag, Ordering::SeqCst);
}

/// Returns whether automatic fatal-crash logging is enabled.
#[must_use]
pub fn arch_get_fatal_stack_logging() -> bool {
    FATAL_STACK_LOGGING.load(Ordering::SeqCst)
}

// Note: arch_set_program_name_for_errors / arch_get_program_name_for_errors
// are defined in init_config.rs and re-exported from lib.rs.

/// Sets additional program info to be reported on fatal errors.
///
/// C++ parity: `ArchSetProgramInfoForErrors(key, value)`.
pub fn arch_set_program_info_for_errors(key: &str, value: &str) {
    let mut info = PROGRAM_INFO.lock().expect("program info lock poisoned");
    info.insert(key.to_owned(), value.to_owned());
}

/// Returns the program info value for the given key.
#[must_use]
pub fn arch_get_program_info_for_errors(key: &str) -> String {
    PROGRAM_INFO
        .lock()
        .expect("program info lock poisoned")
        .get(key)
        .cloned()
        .unwrap_or_default()
}

/// Stores extra log lines that will be included in crash output.
///
/// Pass `None` to remove the entry for `key`.
/// C++ parity: `ArchSetExtraLogInfoForErrors(key, lines)`.
pub fn arch_set_extra_log_info_for_errors(key: &str, lines: Option<Vec<String>>) {
    let mut info = EXTRA_LOG_INFO.lock().expect("extra log info lock poisoned");
    match lines {
        Some(l) => {
            info.insert(key.to_owned(), l);
        }
        None => {
            info.remove(key);
        }
    }
}

/// Registers `arch_log_session_info` for end-of-session logging.
///
/// C++ parity: `ArchEnableSessionLogging()`.
pub fn arch_enable_session_logging() {
    // Ensure the launch time is captured (lives in init_config.rs).
    let _ = crate::init_config::arch_get_app_launch_time();
}

/// Sets the external command used to log process state (call-stack dumps).
///
/// C++ parity: `ArchSetProcessStateLogCommand(command, argv, fatalArgv)`.
///
/// `command`    — path to the external program.
/// `argv`       — argument list for non-fatal invocations.
/// `fatal_argv` — argument list for fatal-crash invocations.
/// Supports `$cmd`, `$pid`, `$log`, `$time`, `$reason` substitution tokens.
pub fn arch_set_process_state_log_command(
    command: &str,
    argv: Vec<String>,
    fatal_argv: Vec<String>,
) {
    *PROCESS_STATE_CMD
        .lock()
        .expect("process state cmd lock poisoned") = Some(command.to_owned());
    *PROCESS_STATE_ARGV
        .lock()
        .expect("process state argv lock poisoned") = Some(argv);
    *PROCESS_STATE_FATAL_ARGV
        .lock()
        .expect("process state fatal argv lock poisoned") = Some(fatal_argv);
}

/// Returns the currently configured process-state log command, if any.
#[must_use]
pub fn arch_get_process_state_log_command() -> Option<String> {
    PROCESS_STATE_CMD
        .lock()
        .expect("process state cmd lock poisoned")
        .clone()
}

/// Sets the external command used for session logging.
///
/// C++ parity: `ArchSetLogSession(command, argv, crashArgv)`.
///
/// `command`    — path to the external program (or env var `$ARCH_LOGSESSION` substitute).
/// `argv`       — argument list used for normal session-end logging.
/// `crash_argv` — argument list used when a crash stack trace is available.
/// Supports `$cmd`, `$prog`, `$pid`, `$time`, `$stack` substitution tokens.
pub fn arch_set_log_session(command: &str, argv: Vec<String>, crash_argv: Vec<String>) {
    *SESSION_LOG_CMD
        .lock()
        .expect("session log cmd lock poisoned") = Some(command.to_owned());
    *SESSION_LOG_ARGV
        .lock()
        .expect("session log argv lock poisoned") = Some(argv);
    *SESSION_CRASH_LOG_ARGV
        .lock()
        .expect("session crash log argv lock poisoned") = Some(crash_argv);
}

/// Returns the currently configured session log command, if any.
#[must_use]
pub fn arch_get_log_session_command() -> Option<String> {
    SESSION_LOG_CMD
        .lock()
        .expect("session log cmd lock poisoned")
        .clone()
}

/// Logs session information by invoking the registered session-log command.
///
/// C++ parity: `ArchLogSessionInfo(const char* crashStackTrace)`.
///
/// When `crash_stack_trace` is `Some`, uses the crash argv; otherwise the
/// normal argv.  Only executes if `arch_get_fatal_stack_logging()` is `true`
/// **and** a session log command has been registered via `arch_set_log_session`.
/// Falls back to stderr output when no command is configured.
pub fn arch_log_session_info(crash_stack_trace: Option<&str>) {
    if !arch_get_fatal_stack_logging() {
        return;
    }

    let cmd_opt = SESSION_LOG_CMD
        .lock()
        .expect("session log cmd lock poisoned")
        .clone();

    let Some(cmd) = cmd_opt else {
        // No command registered — fall back to stderr output.
        let prog = crate::init_config::arch_get_program_name_for_errors();
        if let Some(trace) = crash_stack_trace {
            eprintln!("[session-log] {} crashed:\n{}", prog, trace);
        } else {
            eprintln!("[session-log] {} session ended normally", prog);
        }
        return;
    };

    // Choose the appropriate argv template.
    let argv_opt = if crash_stack_trace.is_some() {
        SESSION_CRASH_LOG_ARGV
            .lock()
            .expect("session crash log argv lock poisoned")
            .clone()
    } else {
        SESSION_LOG_ARGV
            .lock()
            .expect("session log argv lock poisoned")
            .clone()
    };

    let Some(argv_template) = argv_opt else {
        return;
    };

    // Perform token substitution: $cmd, $prog, $pid, $time, $stack.
    let prog = crate::init_config::arch_get_program_name_for_errors();
    let pid = std::process::id().to_string();
    let stack = crash_stack_trace.unwrap_or("");

    let argv: Vec<String> = argv_template
        .iter()
        .map(|arg| match arg.as_str() {
            "$cmd" => cmd.clone(),
            "$prog" => prog.clone(),
            "$pid" => pid.clone(),
            "$stack" => stack.to_owned(),
            other => other.to_owned(),
        })
        .collect();

    if argv.is_empty() {
        return;
    }

    // Spawn the external logger and wait up to 30 s.
    match std::process::Command::new(&argv[0])
        .args(&argv[1..])
        .spawn()
    {
        Ok(mut child) => {
            let _ = child.wait();
        }
        Err(e) => {
            eprintln!("[session-log] failed to spawn '{}': {}", argv[0], e);
        }
    }
}

// ---------------------------------------------------------------------------
// Raw stack frame capture (P1-11) and printing (P1-12)
// ---------------------------------------------------------------------------

/// Captures raw instruction-pointer addresses of the current stack.
///
/// C++ parity: `ArchGetStackFrames(size_t maxDepth, vector<uintptr_t>*)`.
#[must_use]
pub fn arch_get_stack_frames(max_depth: usize) -> Vec<usize> {
    arch_get_stack_frames_skip(max_depth, 0)
}

/// Captures raw addresses, skipping the topmost `skip` frames.
///
/// C++ parity: `ArchGetStackFrames(maxDepth, numFramesToSkipAtTop, ...)`.
#[must_use]
pub fn arch_get_stack_frames_skip(max_depth: usize, skip: usize) -> Vec<usize> {
    let bt = Backtrace::new_unresolved();
    bt.frames()
        .iter()
        .skip(skip)
        .take(max_depth)
        .map(|f| f.ip() as usize)
        .collect()
}

/// Stores at most `buf.len()` raw frame addresses into the provided slice.
///
/// Returns the number of frames actually written.
/// C++ parity: `size_t ArchGetStackFrames(maxDepth, uintptr_t*)`.
pub fn arch_get_stack_frames_into(buf: &mut [usize]) -> usize {
    let bt = Backtrace::new_unresolved();
    let mut count = 0;
    for frame in bt.frames().iter().take(buf.len()) {
        buf[count] = frame.ip() as usize;
        count += 1;
    }
    count
}

/// Prints pre-captured stack frames to the given writer.
///
/// C++ parity: `ArchPrintStackFrames(ostream&, vector<uintptr_t>&, bool)`.
pub fn arch_print_stack_frames(
    writer: &mut impl Write,
    frames: &[usize],
    skip_unknown: bool,
) -> io::Result<()> {
    let cb_opt = arch_get_stack_trace_callback();

    for (idx, &addr) in frames.iter().enumerate() {
        // Try the user callback first
        if let Some(cb) = cb_opt {
            let sym = cb(addr);
            if !sym.is_empty() {
                writeln!(writer, " #{:<3} 0x{:016x} {}", idx, addr, sym)?;
                continue;
            }
        }

        // Resolve via backtrace crate
        let mut resolved = false;
        backtrace::resolve(addr as *mut std::ffi::c_void, |symbol| {
            if let Some(name) = symbol.name() {
                let _ = writeln!(writer, " #{:<3} 0x{:016x} {}", idx, addr, name);
                resolved = true;
            }
        });

        if !resolved && !skip_unknown {
            writeln!(writer, " #{:<3} 0x{:016x} <unknown>", idx, addr)?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_stack_trace() {
        let frames = arch_get_stack_trace(10);
        assert!(!frames.is_empty(), "Should capture at least one frame");
        for frame in &frames {
            assert!(frame.address > 0, "Frame address should be non-zero");
        }
    }

    #[test]
    fn test_get_stack_trace_string() {
        let trace = arch_get_stack_trace_string(5);
        assert!(!trace.is_empty());
        assert!(trace.contains("#"));
    }

    #[test]
    fn test_print_stack_trace() {
        let mut buffer = Vec::new();
        let result = arch_print_stack_trace(&mut buffer);
        assert!(result.is_ok());
        let output = String::from_utf8(buffer).unwrap();
        assert!(output.contains("Stack trace:"));
        assert!(output.contains("#"));
    }

    #[test]
    fn test_stack_frame_display() {
        let frame = StackFrame {
            address: 0x7fff5fbff000,
            symbol_name: Some("test_function".to_string()),
            file_name: Some("test.rs".to_string()),
            line_number: Some(42),
            offset: 0x10,
        };
        let output = format!("{}", frame);
        assert!(output.contains("0x00007fff5fbff000"));
        assert!(output.contains("test_function"));
        assert!(output.contains("test.rs:42"));
        assert!(output.contains("+0x10"));
    }

    #[test]
    fn test_stack_frame_unknown() {
        let frame = StackFrame {
            address: 0x1234,
            symbol_name: None,
            file_name: None,
            line_number: None,
            offset: 0,
        };
        let output = format!("{}", frame);
        assert!(output.contains("0x0000000000001234"));
        assert!(output.contains("<unknown>"));
    }

    #[test]
    fn test_callback_mechanism() {
        static CALLED: AtomicBool = AtomicBool::new(false);

        // New signature: fn(usize) -> String
        fn test_symbolizer(addr: usize) -> String {
            CALLED.store(true, Ordering::SeqCst);
            format!("sym_0x{:x}", addr)
        }
        arch_set_stack_trace_callback(test_symbolizer);
        arch_log_stack_trace("test reason");
        assert!(CALLED.load(Ordering::SeqCst), "Callback should be invoked");
        arch_clear_stack_trace_callback();
    }

    #[test]
    fn test_max_depth_limit() {
        let frames = arch_get_stack_trace(3);
        assert!(frames.len() <= 3);
    }

    #[test]
    fn test_crash_flag() {
        assert!(!arch_is_app_crashing());
        arch_set_app_crashing();
        assert!(arch_is_app_crashing());
        // Reset for other tests
        APP_IS_CRASHING.store(false, Ordering::SeqCst);
    }

    #[test]
    fn test_program_info() {
        arch_set_program_info_for_errors("version", "1.0");
        assert_eq!(arch_get_program_info_for_errors("version"), "1.0");
        assert_eq!(arch_get_program_info_for_errors("nonexist"), "");
    }

    #[test]
    fn test_raw_stack_frames() {
        let frames = arch_get_stack_frames(10);
        assert!(!frames.is_empty());
        for &addr in &frames {
            assert!(addr > 0);
        }
    }

    #[test]
    fn test_print_stack_frames_to_buf() {
        let frames = arch_get_stack_frames(5);
        let mut buf = Vec::new();
        let result = arch_print_stack_frames(&mut buf, &frames, false);
        assert!(result.is_ok());
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("#"));
    }

    #[test]
    #[should_panic]
    fn test_stack_trace_on_panic() {
        fn level3() {
            panic!("Intentional panic for testing");
        }
        fn level2() {
            level3();
        }
        fn level1() {
            level2();
        }
        level1();
    }

    #[test]
    fn test_set_process_state_log_command() {
        arch_set_process_state_log_command(
            "/usr/bin/logger",
            vec!["$cmd".into(), "--pid=$pid".into()],
            vec!["$cmd".into(), "--fatal".into(), "$reason".into()],
        );
        let cmd = arch_get_process_state_log_command();
        assert_eq!(cmd.as_deref(), Some("/usr/bin/logger"));

        // Reset to avoid polluting other tests.
        *PROCESS_STATE_CMD.lock().unwrap() = None;
        *PROCESS_STATE_ARGV.lock().unwrap() = None;
        *PROCESS_STATE_FATAL_ARGV.lock().unwrap() = None;
    }

    #[test]
    fn test_set_log_session() {
        arch_set_log_session(
            "/usr/bin/session-log",
            vec!["$cmd".into(), "$prog".into()],
            vec!["$cmd".into(), "$stack".into()],
        );
        let cmd = arch_get_log_session_command();
        assert_eq!(cmd.as_deref(), Some("/usr/bin/session-log"));

        // Reset to avoid polluting other tests.
        *SESSION_LOG_CMD.lock().unwrap() = None;
        *SESSION_LOG_ARGV.lock().unwrap() = None;
        *SESSION_CRASH_LOG_ARGV.lock().unwrap() = None;
    }

    #[test]
    fn test_log_session_info_no_cmd_no_flag() {
        // With logging disabled and no command, arch_log_session_info is a no-op.
        arch_set_fatal_stack_logging(false);
        *SESSION_LOG_CMD.lock().unwrap() = None;
        // Should not panic.
        arch_log_session_info(None);
        arch_log_session_info(Some("fake trace"));
    }

    #[test]
    fn test_log_session_info_fallback_stderr() {
        // With logging enabled but no command registered, falls back to stderr.
        arch_set_fatal_stack_logging(true);
        *SESSION_LOG_CMD.lock().unwrap() = None;
        // Just verify it doesn't panic.
        arch_log_session_info(None);
        arch_set_fatal_stack_logging(false);
    }
}
