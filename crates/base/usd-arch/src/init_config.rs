//! Architecture module initialization.
//!
//! This module provides initialization functions that are called when the
//! architecture module is loaded. This is the Rust equivalent of
//! `pxr/base/arch/initConfig.cpp`.
//!
//! # Example
//!
//! ```ignore
//! use usd_arch::arch_init_config;
//!
//! // Initialize architecture module (typically called automatically)
//! arch_init_config();
//! ```

use crate::arch_debugger_is_attached;
use crate::assumptions::arch_validate_assumptions;
use crate::system_info::{get_executable_path, get_temp_dir};
use std::sync::Once;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Global flag to track if initialization has been performed.
static INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Global initialization guard.
static INIT_GUARD: Once = Once::new();

/// Application launch time (Unix timestamp).
static APP_LAUNCH_TIME: std::sync::OnceLock<u64> = std::sync::OnceLock::new();

/// Program name for error reporting.
static PROGRAM_NAME: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// Sets the application launch time to the current time.
///
/// This function is called during initialization to record when the application
/// started. This is useful for timing and diagnostics.
///
/// # Example
///
/// ```ignore
/// use usd_arch::arch_set_app_launch_time;
///
/// arch_set_app_launch_time();
/// ```
pub fn arch_set_app_launch_time() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    APP_LAUNCH_TIME.set(now).ok();
}

/// Gets the application launch time.
///
/// Returns the Unix timestamp when the application was launched, or `None`
/// if it hasn't been set yet.
///
/// # Example
///
/// ```ignore
/// use usd_arch::arch_get_app_launch_time;
///
/// if let Some(launch_time) = arch_get_app_launch_time() {
///     println!("Application launched at: {}", launch_time);
/// }
/// ```
pub fn arch_get_app_launch_time() -> Option<u64> {
    APP_LAUNCH_TIME.get().copied()
}

/// Initializes the temporary directory.
///
/// This function ensures that the temporary directory is accessible and
/// can be used by other parts of the system. In Rust, this is typically
/// handled by `std::env::temp_dir()`, but we call it here to ensure it's
/// initialized early.
///
/// # Example
///
/// ```ignore
/// use usd_arch::arch_init_tmp_dir;
///
/// arch_init_tmp_dir();
/// ```
pub fn arch_init_tmp_dir() {
    // Just ensure temp_dir() is called to initialize it
    let _ = get_temp_dir();
}

/// Sets the program name for error reporting.
///
/// This function sets the program name that will be used in diagnostic
/// output and error messages.
///
/// # Arguments
///
/// * `prog_name` - The program name to use for error reporting
///
/// # Example
///
/// ```ignore
/// use usd_arch::arch_set_program_name_for_errors;
///
/// arch_set_program_name_for_errors("my_program");
/// ```
pub fn arch_set_program_name_for_errors(prog_name: &str) {
    // Use lock().ok() to avoid panic on poisoned mutex; silently skip on failure.
    if let Ok(mut name) = PROGRAM_NAME.lock() {
        *name = if prog_name.is_empty() {
            None
        } else {
            Some(prog_name.to_string())
        };
    }
}

/// Gets the program name for error reporting.
///
/// Returns the currently set program name, or "libArch" if none has been set.
///
/// # Example
///
/// ```ignore
/// use usd_arch::arch_get_program_name_for_errors;
///
/// let name = arch_get_program_name_for_errors();
/// println!("Program name: {}", name);
/// ```
pub fn arch_get_program_name_for_errors() -> String {
    // Gracefully fall back to "libArch" if the mutex is poisoned.
    PROGRAM_NAME
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().cloned())
        .unwrap_or_else(|| "libArch".to_string())
}

/// Initializes the debugger interface.
///
/// This function performs any necessary initialization for debugger interaction.
/// In Rust, most of this is handled automatically, but we call it here for
/// compatibility with the C++ API.
///
/// # Example
///
/// ```ignore
/// use usd_arch::arch_init_debugger_attach;
///
/// arch_init_debugger_attach();
/// ```
pub fn arch_init_debugger_attach() {
    // Just check if debugger is attached to initialize the interface
    let _ = arch_debugger_is_attached();
}

/// Initializes the tick timer.
///
/// This function initializes the high-resolution timing system. In Rust,
/// this is handled automatically by the timing module, but we call it here
/// for compatibility.
///
/// # Example
///
/// ```ignore
/// use usd_arch::arch_init_tick_timer;
///
/// arch_init_tick_timer();
/// ```
pub fn arch_init_tick_timer() {
    // Initialize the tick timer by calling get_ticks()
    use crate::timing::get_ticks;
    let _ = get_ticks();
}

/// Performs architecture module initialization.
///
/// This function performs all necessary initialization for the architecture module:
/// - Sets application launch time
/// - Initializes temporary directory
/// - Sets program name for errors
/// - Validates platform assumptions
/// - Initializes debugger interface
///
/// This function is safe to call multiple times; it will only initialize once.
///
/// # Example
///
/// ```ignore
/// use usd_arch::arch_init_config;
///
/// // Initialize architecture module
/// arch_init_config();
/// ```
pub fn arch_init_config() {
    INIT_GUARD.call_once(|| {
        // Initialize the application start time. First so it's as close as
        // possible to the real start time.
        arch_set_app_launch_time();

        // Initialize the temp directory. Early so other initialization
        // functions can use it.
        arch_init_tmp_dir();

        // Initialize program name for errors. Early for initialization
        // error reporting.
        if let Some(exec_path) = get_executable_path() {
            if let Some(file_name) = exec_path.file_name() {
                if let Some(name_str) = file_name.to_str() {
                    arch_set_program_name_for_errors(name_str);
                }
            }
        }

        // Perform platform validations: these are very quick, lightweight
        // checks. The reason that we call this function here is that pretty
        // much any program that uses anything from lib/tf will end up here
        // at some point. It is not so important that *every* program
        // perform this check; what is important is that when we bring up a new
        // architecture/compiler/build, the validation gets performed at some
        // point, to alert us to any problems.
        arch_validate_assumptions();

        // Initialize the debugger interface.
        arch_init_debugger_attach();

        // Initialize the tick timer.
        arch_init_tick_timer();

        INITIALIZED.store(true, Ordering::Release);
    });
}

/// Returns whether the architecture module has been initialized.
///
/// # Example
///
/// ```ignore
/// use usd_arch::{arch_init_config, arch_is_initialized};
///
/// if !arch_is_initialized() {
///     arch_init_config();
/// }
/// ```
pub fn arch_is_initialized() -> bool {
    INITIALIZED.load(Ordering::Acquire)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arch_init_config() {
        // Reset initialization state for testing
        // Note: Once::call_once prevents re-initialization, so we can't
        // fully test re-initialization, but we can test that it doesn't crash
        arch_init_config();
        assert!(arch_is_initialized());
    }

    #[test]
    fn test_arch_set_app_launch_time() {
        arch_set_app_launch_time();
        assert!(arch_get_app_launch_time().is_some());
    }

    #[test]
    fn test_arch_program_name() {
        arch_set_program_name_for_errors("test_program");
        assert_eq!(arch_get_program_name_for_errors(), "test_program");
    }

    #[test]
    fn test_arch_program_name_default() {
        // Reset to default
        arch_set_program_name_for_errors("");
        let name = arch_get_program_name_for_errors();
        // Should either be empty or default to "libArch"
        assert!(!name.is_empty());
    }

    #[test]
    fn test_arch_init_tmp_dir() {
        arch_init_tmp_dir();
        // Should not crash
    }

    #[test]
    fn test_arch_init_debugger_attach() {
        arch_init_debugger_attach();
        // Should not crash
    }

    #[test]
    fn test_arch_init_tick_timer() {
        arch_init_tick_timer();
        // Should not crash
    }
}
