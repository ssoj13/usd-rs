// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Error category, mirrors C++ `Far::ErrorType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ErrorType {
    /// No error.
    NoError = 0,
    /// Fatal error — program should terminate.
    FatalError = 1,
    /// Internal programming error (continue execution).
    InternalCodingError = 2,
    /// Generic programming error (continue execution).
    CodingError = 3,
    /// Generic runtime error (continue execution).
    RuntimeError = 4,
}

impl ErrorType {
    pub fn label(self) -> &'static str {
        match self {
            Self::NoError => "No Error",
            Self::FatalError => "Fatal Error",
            Self::InternalCodingError => "Coding Error (internal)",
            Self::CodingError => "Coding Error",
            Self::RuntimeError => "Error",
        }
    }
}

// ---------------------------------------------------------------------------
// Callback storage — thread-unsafe by design (matches C++ implementation)
// ---------------------------------------------------------------------------

/// Error callback function type, mirrors C++ `Far::ErrorCallbackFunc`.
pub type ErrorCallbackFunc = fn(ErrorType, &str);

/// Warning callback function type, mirrors C++ `Far::WarningCallbackFunc`.
pub type WarningCallbackFunc = fn(&str);

static ERROR_CALLBACK: std::sync::OnceLock<std::sync::Mutex<Option<ErrorCallbackFunc>>> =
    std::sync::OnceLock::new();
static WARNING_CALLBACK: std::sync::OnceLock<std::sync::Mutex<Option<WarningCallbackFunc>>> =
    std::sync::OnceLock::new();

fn error_mutex() -> &'static std::sync::Mutex<Option<ErrorCallbackFunc>> {
    ERROR_CALLBACK.get_or_init(|| std::sync::Mutex::new(None))
}
fn warning_mutex() -> &'static std::sync::Mutex<Option<WarningCallbackFunc>> {
    WARNING_CALLBACK.get_or_init(|| std::sync::Mutex::new(None))
}

/// Set the global error callback.
///
/// Mirrors C++ `Far::SetErrorCallback()`.
/// Note: C++ documents this as NOT thread-safe; we wrap in Mutex for Rust safety.
pub fn set_error_callback(func: Option<ErrorCallbackFunc>) {
    *error_mutex().lock().unwrap() = func;
}

/// Set the global warning callback.
///
/// Mirrors C++ `Far::SetWarningCallback()`.
pub fn set_warning_callback(func: Option<WarningCallbackFunc>) {
    *warning_mutex().lock().unwrap() = func;
}

// ---------------------------------------------------------------------------
// Sending errors and warnings
// ---------------------------------------------------------------------------

/// Send an error with formatted message.
///
/// Mirrors C++ `Far::Error(ErrorType, format, ...)`.
pub fn far_error(err: ErrorType, message: &str) {
    assert!(err != ErrorType::NoError, "far_error called with NoError");
    if let Some(cb) = *error_mutex().lock().unwrap() {
        cb(err, message);
    } else {
        // C++ uses printf (stdout)
        println!("{}: {}", err.label(), message);
    }
}

/// Send a warning message.
///
/// Mirrors C++ `Far::Warning(format, ...)` — output goes to stdout.
pub fn far_warning(message: &str) {
    if let Some(cb) = *warning_mutex().lock().unwrap() {
        cb(message);
    } else {
        // C++ uses fprintf(stdout, "Warning: %s\n", message)
        println!("Warning: {}", message);
    }
}
