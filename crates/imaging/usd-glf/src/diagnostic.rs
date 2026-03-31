//! GL diagnostic utilities: error checking, debug groups, query objects.
//!
//! Port of pxr/imaging/glf/diagnostic.h / diagnostic.cpp
//!
//! Provides:
//! - `glf_post_pending_gl_errors`   — drain and log all pending GL errors
//! - `glf_register_debug_callback`  — install the default KHR_debug callback
//! - `GlfDebugGroup`                — RAII push/pop of a GL debug group scope
//! - `GlfGLQueryObject`             — wrapper around a GL query object

// ---------------------------------------------------------------------------
// GL error draining
// ---------------------------------------------------------------------------

/// Drain all pending GL errors from the current context and log them.
///
/// Mirrors `GlfPostPendingGLErrors()`.  The `where_` string and line number
/// are included in each log message for context.  At most 256 errors are
/// consumed (watchdog prevents infinite loops on broken contexts).
#[cfg(feature = "opengl")]
pub fn glf_post_pending_gl_errors(where_: &str, line: u32) {
    let mut count = 0u32;
    loop {
        if count >= 256 {
            break;
        }
        let err = unsafe { gl::GetError() };
        if err == gl::NO_ERROR {
            break;
        }
        count += 1;
        let desc = gl_error_str(err);
        if where_.is_empty() {
            log::error!("GL error: {} (0x{:04X})", desc, err);
        } else {
            log::error!(
                "GL error: {} (0x{:04X}) — reported from {} at line {}",
                desc,
                err,
                where_,
                line
            );
        }
    }
}

/// No-op stub when the `opengl` feature is disabled.
#[cfg(not(feature = "opengl"))]
pub fn glf_post_pending_gl_errors(_where_: &str, _line: u32) {}

/// Convenience macro — captures the call-site location automatically.
///
/// ```ignore
/// glf_check_gl_errors!();
/// glf_check_gl_errors!("after draw");
/// ```
#[macro_export]
macro_rules! glf_check_gl_errors {
    () => {
        $crate::diagnostic::glf_post_pending_gl_errors("", 0)
    };
    ($where:expr) => {
        $crate::diagnostic::glf_post_pending_gl_errors($where, line!())
    };
}

/// Human-readable description for common GL error codes.
#[cfg(feature = "opengl")]
fn gl_error_str(err: gl::types::GLenum) -> &'static str {
    match err {
        gl::INVALID_ENUM => "GL_INVALID_ENUM",
        gl::INVALID_VALUE => "GL_INVALID_VALUE",
        gl::INVALID_OPERATION => "GL_INVALID_OPERATION",
        gl::STACK_OVERFLOW => "GL_STACK_OVERFLOW",
        gl::STACK_UNDERFLOW => "GL_STACK_UNDERFLOW",
        gl::OUT_OF_MEMORY => "GL_OUT_OF_MEMORY",
        gl::INVALID_FRAMEBUFFER_OPERATION => "GL_INVALID_FRAMEBUFFER_OPERATION",
        _ => "GL_UNKNOWN_ERROR",
    }
}

// ---------------------------------------------------------------------------
// KHR_debug callback
// ---------------------------------------------------------------------------

/// Register the default debug output message callback with the current context.
///
/// Mirrors `GlfRegisterDefaultDebugOutputMessageCallback()`.
/// After this call, the driver sends all debug messages to
/// `glf_debug_output_callback`.  Requires GL 4.3 / KHR_debug support.
#[cfg(feature = "opengl")]
pub fn glf_register_debug_callback() {
    unsafe {
        gl::Enable(gl::DEBUG_OUTPUT);
        gl::Enable(gl::DEBUG_OUTPUT_SYNCHRONOUS);
        gl::DebugMessageCallback(Some(glf_debug_output_callback), std::ptr::null());
        // Suppress push/pop-group messages — they are noisy and not errors
        gl::DebugMessageControl(
            gl::DONT_CARE,
            gl::DEBUG_TYPE_PUSH_GROUP,
            gl::DONT_CARE,
            0,
            std::ptr::null(),
            gl::FALSE,
        );
        gl::DebugMessageControl(
            gl::DONT_CARE,
            gl::DEBUG_TYPE_POP_GROUP,
            gl::DONT_CARE,
            0,
            std::ptr::null(),
            gl::FALSE,
        );
    }
    log::debug!("GlfDiagnostic: KHR_debug callback registered");
}

/// No-op stub when the `opengl` feature is disabled.
#[cfg(not(feature = "opengl"))]
pub fn glf_register_debug_callback() {}

/// GL debug message callback.
///
/// Errors are logged at `error!` level; everything else at `warn!`.
/// Matches `GlfDefaultDebugOutputMessageCallback()`.
#[cfg(feature = "opengl")]
extern "system" fn glf_debug_output_callback(
    source: gl::types::GLenum,
    type_: gl::types::GLenum,
    id: gl::types::GLuint,
    severity: gl::types::GLenum,
    _length: gl::types::GLsizei,
    message: *const gl::types::GLchar,
    _user_param: *mut std::ffi::c_void,
) {
    let msg = unsafe {
        std::ffi::CStr::from_ptr(message)
            .to_string_lossy()
            .into_owned()
    };
    let src_s = debug_enum_str(source);
    let type_s = debug_enum_str(type_);
    let sev_s = debug_enum_str(severity);

    if type_ == gl::DEBUG_TYPE_ERROR {
        log::error!(
            "GL debug [src={} type={} id={} sev={}]: {}",
            src_s,
            type_s,
            id,
            sev_s,
            msg
        );
    } else {
        log::warn!(
            "GL debug [src={} type={} sev={}]: {}",
            src_s,
            type_s,
            sev_s,
            msg
        );
    }
}

/// Convert a GL debug enum to a short human-readable string.
///
/// Mirrors `GlfDebugEnumToString()`.
#[cfg(feature = "opengl")]
pub fn debug_enum_str(e: gl::types::GLenum) -> &'static str {
    match e {
        gl::DEBUG_SOURCE_API => "API",
        gl::DEBUG_SOURCE_WINDOW_SYSTEM => "WINDOW_SYSTEM",
        gl::DEBUG_SOURCE_SHADER_COMPILER => "SHADER_COMPILER",
        gl::DEBUG_SOURCE_THIRD_PARTY => "THIRD_PARTY",
        gl::DEBUG_SOURCE_APPLICATION => "APPLICATION",
        gl::DEBUG_SOURCE_OTHER => "OTHER",
        gl::DEBUG_TYPE_ERROR => "ERROR",
        gl::DEBUG_TYPE_DEPRECATED_BEHAVIOR => "DEPRECATED",
        gl::DEBUG_TYPE_UNDEFINED_BEHAVIOR => "UNDEFINED_BEHAVIOR",
        gl::DEBUG_TYPE_PORTABILITY => "PORTABILITY",
        gl::DEBUG_TYPE_PERFORMANCE => "PERFORMANCE",
        gl::DEBUG_TYPE_MARKER => "MARKER",
        gl::DEBUG_TYPE_PUSH_GROUP => "PUSH_GROUP",
        gl::DEBUG_TYPE_POP_GROUP => "POP_GROUP",
        gl::DEBUG_TYPE_OTHER => "OTHER",
        gl::DEBUG_SEVERITY_HIGH => "HIGH",
        gl::DEBUG_SEVERITY_MEDIUM => "MEDIUM",
        gl::DEBUG_SEVERITY_LOW => "LOW",
        gl::DEBUG_SEVERITY_NOTIFICATION => "NOTIFICATION",
        _ => "UNKNOWN",
    }
}

// ---------------------------------------------------------------------------
// GlfDebugGroup — RAII GL debug scope
// ---------------------------------------------------------------------------

/// RAII guard that pushes/pops a GL debug group for the duration of a scope.
///
/// Maps to `GlfDebugGroup` / `GLF_GROUP_FUNCTION()` / `GLF_GROUP_SCOPE()`.
///
/// When the `opengl` feature is enabled the group is pushed on creation and
/// popped on drop (requires GL 4.3 / KHR_debug).  Otherwise this is a zero-
/// cost no-op.
pub struct GlfDebugGroup {
    #[cfg(feature = "opengl")]
    _active: bool,
}

/// Returns true when the `GLF_ENABLE_DIAGNOSTIC_TRACE` environment variable
/// is set to a non-empty value. Matches C++ TfEnvSetting check.
#[cfg_attr(not(feature = "opengl"), allow(dead_code))]
fn diagnostic_trace_enabled() -> bool {
    std::env::var("GLF_ENABLE_DIAGNOSTIC_TRACE")
        .map(|v| !v.is_empty() && v != "0")
        .unwrap_or(false)
}

impl GlfDebugGroup {
    /// Push a named debug group onto the GL command stream.
    ///
    /// Only active when the `opengl` feature is enabled AND the env var
    /// `GLF_ENABLE_DIAGNOSTIC_TRACE` is set (matches C++ behaviour).
    pub fn new(message: &str) -> Self {
        #[cfg(feature = "opengl")]
        {
            if !diagnostic_trace_enabled() {
                return Self { _active: false };
            }
            use std::ffi::CString;
            if let Ok(cmsg) = CString::new(message) {
                unsafe {
                    gl::PushDebugGroup(gl::DEBUG_SOURCE_THIRD_PARTY, 0, -1, cmsg.as_ptr());
                }
                return Self { _active: true };
            }
            Self { _active: false }
        }
        #[cfg(not(feature = "opengl"))]
        {
            let _ = message;
            Self {}
        }
    }
}

impl Drop for GlfDebugGroup {
    fn drop(&mut self) {
        #[cfg(feature = "opengl")]
        if self._active {
            unsafe {
                gl::PopDebugGroup();
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Object labelling helpers
// ---------------------------------------------------------------------------

/// Attach a debug label to a GL buffer object (requires KHR_debug / GL 4.3).
#[cfg(feature = "opengl")]
pub fn debug_label_buffer(id: u32, label: &str) {
    use std::ffi::CString;
    if let Ok(clabel) = CString::new(label) {
        unsafe {
            gl::ObjectLabel(gl::BUFFER, id, -1, clabel.as_ptr());
        }
    }
}

/// No-op stub when the `opengl` feature is disabled.
#[cfg(not(feature = "opengl"))]
pub fn debug_label_buffer(_id: u32, _label: &str) {}

/// Attach a debug label to a GL shader object.
#[cfg(feature = "opengl")]
pub fn debug_label_shader(id: u32, label: &str) {
    use std::ffi::CString;
    if let Ok(clabel) = CString::new(label) {
        unsafe {
            gl::ObjectLabel(gl::SHADER, id, -1, clabel.as_ptr());
        }
    }
}

/// No-op stub when the `opengl` feature is disabled.
#[cfg(not(feature = "opengl"))]
pub fn debug_label_shader(_id: u32, _label: &str) {}

/// Attach a debug label to a GL program object.
#[cfg(feature = "opengl")]
pub fn debug_label_program(id: u32, label: &str) {
    use std::ffi::CString;
    if let Ok(clabel) = CString::new(label) {
        unsafe {
            gl::ObjectLabel(gl::PROGRAM, id, -1, clabel.as_ptr());
        }
    }
}

/// No-op stub when the `opengl` feature is disabled.
#[cfg(not(feature = "opengl"))]
pub fn debug_label_program(_id: u32, _label: &str) {}

// ---------------------------------------------------------------------------
// GlfGLQueryObject — GL timing / occlusion queries
// ---------------------------------------------------------------------------

/// Wrapper around a GL query object.
///
/// Mirrors `GlfGLQueryObject`.  Supports samples-passed, primitives-generated,
/// and time-elapsed queries.
pub struct GlfGLQueryObject {
    /// GL object ID, 0 when not allocated or feature disabled.
    #[allow(dead_code)]
    id: u32,
    /// Currently active target (0 when idle).
    #[allow(dead_code)]
    target: u32,
}

impl GlfGLQueryObject {
    /// Allocate a new query object in the current GL context.
    pub fn new() -> Self {
        #[cfg(feature = "opengl")]
        {
            let mut id: u32 = 0;
            unsafe {
                gl::GenQueries(1, &mut id);
            }
            Self { id, target: 0 }
        }
        #[cfg(not(feature = "opengl"))]
        Self { id: 0, target: 0 }
    }

    /// Begin a `GL_SAMPLES_PASSED` query.
    pub fn begin_samples_passed(&mut self) {
        self.begin(gl::SAMPLES_PASSED);
    }

    /// Begin a `GL_PRIMITIVES_GENERATED` query.
    pub fn begin_primitives_generated(&mut self) {
        self.begin(gl::PRIMITIVES_GENERATED);
    }

    /// Begin a `GL_TIME_ELAPSED` query (result in nanoseconds).
    pub fn begin_time_elapsed(&mut self) {
        self.begin(gl::TIME_ELAPSED);
    }

    /// Begin a query for the given `target`.
    pub fn begin(&mut self, target: u32) {
        #[cfg(feature = "opengl")]
        if self.id != 0 {
            self.target = target;
            unsafe {
                gl::BeginQuery(target, self.id);
            }
        }
        #[cfg(not(feature = "opengl"))]
        let _ = target;
    }

    /// End the active query.
    pub fn end(&mut self) {
        #[cfg(feature = "opengl")]
        if self.target != 0 {
            unsafe {
                gl::EndQuery(self.target);
            }
            self.target = 0;
        }
    }

    /// Return the query result, blocking until it is available.
    pub fn get_result(&self) -> i64 {
        #[cfg(feature = "opengl")]
        if self.id != 0 {
            let mut value: i64 = 0;
            unsafe {
                gl::GetQueryObjecti64v(self.id, gl::QUERY_RESULT, &mut value);
            }
            return value;
        }
        0
    }

    /// Return the query result without blocking; returns 0 if not yet ready.
    pub fn get_result_no_wait(&self) -> i64 {
        #[cfg(feature = "opengl")]
        if self.id != 0 {
            let mut available: i64 = 0;
            unsafe {
                gl::GetQueryObjecti64v(self.id, gl::QUERY_RESULT_AVAILABLE, &mut available);
            }
            if available != 0 {
                let mut value: i64 = 0;
                unsafe {
                    gl::GetQueryObjecti64v(self.id, gl::QUERY_RESULT, &mut value);
                }
                return value;
            }
        }
        0
    }
}

impl Default for GlfGLQueryObject {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for GlfGLQueryObject {
    fn drop(&mut self) {
        #[cfg(feature = "opengl")]
        if self.id != 0 {
            unsafe {
                gl::DeleteQueries(1, &self.id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_group_noop() {
        // Without a real GL context this is a no-op but must not panic.
        let _g = GlfDebugGroup::new("test scope");
    }

    #[test]
    fn test_query_object_default() {
        let q = GlfGLQueryObject::new();
        // No context: result should be 0
        assert_eq!(q.get_result(), 0);
    }

    #[test]
    fn test_post_pending_gl_errors_noop() {
        // Without a GL context this should be safe (no-op outside opengl feature)
        glf_post_pending_gl_errors("test", 0);
    }
}
