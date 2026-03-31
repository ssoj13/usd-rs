//! OpenGL error checking and diagnostic utilities

use std::fmt;

/// OpenGL error codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum GLError {
    /// No error has been recorded (GL_NO_ERROR = 0x0000)
    NoError = 0,
    /// An unacceptable value is specified for an enumerated argument (GL_INVALID_ENUM = 0x0500)
    InvalidEnum = 0x0500,
    /// A numeric argument is out of range (GL_INVALID_VALUE = 0x0501)
    InvalidValue = 0x0501,
    /// The specified operation is not allowed in the current state (GL_INVALID_OPERATION = 0x0502)
    InvalidOperation = 0x0502,
    /// An attempt has been made to perform an operation that would cause an internal stack to overflow (GL_STACK_OVERFLOW = 0x0503)
    StackOverflow = 0x0503,
    /// An attempt has been made to perform an operation that would cause an internal stack to underflow (GL_STACK_UNDERFLOW = 0x0504)
    StackUnderflow = 0x0504,
    /// There is not enough memory left to execute the command (GL_OUT_OF_MEMORY = 0x0505)
    OutOfMemory = 0x0505,
    /// The framebuffer object is not complete (GL_INVALID_FRAMEBUFFER_OPERATION = 0x0506)
    InvalidFramebufferOperation = 0x0506,
    /// The OpenGL context has been lost due to a graphics card reset (GL_CONTEXT_LOST = 0x0507)
    ContextLost = 0x0507,
}

impl fmt::Display for GLError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GLError::NoError => write!(f, "GL_NO_ERROR"),
            GLError::InvalidEnum => write!(f, "GL_INVALID_ENUM"),
            GLError::InvalidValue => write!(f, "GL_INVALID_VALUE"),
            GLError::InvalidOperation => write!(f, "GL_INVALID_OPERATION"),
            GLError::StackOverflow => write!(f, "GL_STACK_OVERFLOW"),
            GLError::StackUnderflow => write!(f, "GL_STACK_UNDERFLOW"),
            GLError::OutOfMemory => write!(f, "GL_OUT_OF_MEMORY"),
            GLError::InvalidFramebufferOperation => write!(f, "GL_INVALID_FRAMEBUFFER_OPERATION"),
            GLError::ContextLost => write!(f, "GL_CONTEXT_LOST"),
        }
    }
}

/// Check for OpenGL errors and log them
///
/// # Returns
///
/// Returns the first error encountered, or None if no errors
///
/// # Stub
///
/// Real implementation: glGetError() loop until GL_NO_ERROR.
pub fn check_gl_errors(_context: &str) -> Option<GLError> {
    // Note: Would call glGetError() here
    // let error = unsafe { gl::GetError() };
    // if error != gl::NO_ERROR {
    //     eprintln!("OpenGL error in {}: {}", context, gl_error_string(error));
    //     return Some(error_from_code(error));
    // }
    None
}

/// Clear all pending OpenGL errors
///
/// # Stub
///
/// Real implementation: glGetError() loop until GL_NO_ERROR.
pub fn clear_gl_errors() {
    // Note: Would call glGetError() in a loop until NO_ERROR
    // loop {
    //     let error = unsafe { gl::GetError() };
    //     if error == gl::NO_ERROR {
    //         break;
    //     }
    // }
}

/// Get OpenGL version string
///
/// # Stub
///
/// Real implementation: glGetString(GL_VERSION).
pub fn get_gl_version() -> String {
    // Note: Would call glGetString(GL_VERSION)
    // let version_ptr = unsafe { gl::GetString(gl::VERSION) };
    // let version_cstr = unsafe { std::ffi::CStr::from_ptr(version_ptr as *const i8) };
    // version_cstr.to_string_lossy().into_owned()
    "4.5.0 STUB".to_string()
}

/// Get GLSL version string
///
/// # Stub
///
/// Real implementation: glGetString(GL_SHADING_LANGUAGE_VERSION).
pub fn get_glsl_version() -> String {
    // Note: Would call glGetString(GL_SHADING_LANGUAGE_VERSION)
    // let version_ptr = unsafe { gl::GetString(gl::SHADING_LANGUAGE_VERSION) };
    // let version_cstr = unsafe { std::ffi::CStr::from_ptr(version_ptr as *const i8) };
    // version_cstr.to_string_lossy().into_owned()
    "450 STUB".to_string()
}

/// Get OpenGL renderer string
///
/// # Stub
///
/// Real implementation: glGetString(GL_RENDERER).
pub fn get_gl_renderer() -> String {
    // Note: Would call glGetString(GL_RENDERER)
    "Stub OpenGL Renderer".to_string()
}

/// Get OpenGL vendor string
///
/// # Stub
///
/// Real implementation: glGetString(GL_VENDOR).
pub fn get_gl_vendor() -> String {
    // Note: Would call glGetString(GL_VENDOR)
    "Stub Vendor".to_string()
}

/// Check if an OpenGL extension is supported
///
/// # Stub
///
/// Real implementation: glGetStringi(GL_EXTENSIONS, i) loop.
pub fn has_extension(_name: &str) -> bool {
    // Note: Would check extension support
    // let num_extensions = get_integerv(gl::NUM_EXTENSIONS);
    // for i in 0..num_extensions {
    //     let ext_ptr = unsafe { gl::GetStringi(gl::EXTENSIONS, i as u32) };
    //     let ext_cstr = unsafe { std::ffi::CStr::from_ptr(ext_ptr as *const i8) };
    //     if ext_cstr.to_str().unwrap() == name {
    //         return true;
    //     }
    // }
    false
}

/// Get an OpenGL integer parameter
///
/// # Stub
///
/// Real implementation: glGetIntegerv(pname, &value).
pub fn get_integerv(_pname: u32) -> i32 {
    // Note: Would call glGetIntegerv
    // let mut value = 0;
    // unsafe { gl::GetIntegerv(pname, &mut value) };
    // value
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gl_error_display() {
        assert_eq!(GLError::NoError.to_string(), "GL_NO_ERROR");
        assert_eq!(GLError::InvalidEnum.to_string(), "GL_INVALID_ENUM");
        assert_eq!(GLError::OutOfMemory.to_string(), "GL_OUT_OF_MEMORY");
    }

    #[test]
    fn test_stub_functions() {
        // These are stubs, just verify they don't crash
        clear_gl_errors();
        assert!(check_gl_errors("test").is_none());
        assert!(!get_gl_version().is_empty());
        assert!(!get_glsl_version().is_empty());
        assert!(!has_extension("GL_ARB_direct_state_access"));
    }
}
