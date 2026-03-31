//! Platform-agnostic GL context abstraction.
//!
//! Provides a cross-platform interface for GL context management with
//! platform-specific implementations for Windows (WGL), Linux (GLX), and macOS (CGL).

use std::fmt;
use std::hash::{Hash, Hasher};

/// Platform-agnostic GL context state.
///
/// This is a trait object wrapper around platform-specific context handles.
/// Use this to store and restore GL contexts in a platform-independent way.
pub trait GarchGLPlatformContext: Send + Sync + fmt::Debug {
    /// Make this context current on the calling thread.
    fn make_current(&self);

    /// Check if this context state is valid.
    fn is_valid(&self) -> bool;

    /// Check if this context shares resources with another context.
    fn is_sharing(&self, other: &dyn GarchGLPlatformContext) -> bool;

    /// Get hash for this context state.
    fn get_hash(&self) -> u64;

    /// Clone as boxed trait object.
    fn clone_box(&self) -> Box<dyn GarchGLPlatformContext>;
}

/// Opaque platform-specific context state handle.
///
/// Stores the current GL context state in a platform-independent way.
/// Internally dispatches to Windows/GLX/macOS implementations.
#[derive(Debug)]
pub struct GarchGLPlatformContextState {
    inner: Box<dyn GarchGLPlatformContext>,
}

impl Clone for GarchGLPlatformContextState {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone_box(),
        }
    }
}

impl GarchGLPlatformContextState {
    /// Create new context state from platform-specific implementation.
    pub fn new(ctx: Box<dyn GarchGLPlatformContext>) -> Self {
        Self { inner: ctx }
    }

    /// Capture the current GL context state.
    pub fn current() -> Self {
        #[cfg(target_os = "windows")]
        {
            Self::new(Box::new(platform::windows::WGLContextState::current()))
        }
        #[cfg(target_os = "linux")]
        {
            Self::new(Box::new(platform::linux::GLXContextState::current()))
        }
        #[cfg(target_os = "macos")]
        {
            Self::new(Box::new(platform::macos::CGLContextState::current()))
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            Self::null()
        }
    }

    /// Create null/invalid context state.
    pub fn null() -> Self {
        Self::new(Box::new(NullContextState))
    }

    /// Make this context current.
    pub fn make_current(&self) {
        self.inner.make_current();
    }

    /// Check if context is valid.
    pub fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    /// Check if this context shares resources with another.
    pub fn is_sharing(&self, other: &Self) -> bool {
        self.inner.is_sharing(&*other.inner)
    }

    /// Make no context current on this thread.
    pub fn done_current() {
        #[cfg(target_os = "windows")]
        platform::windows::WGLContextState::done_current();

        #[cfg(target_os = "linux")]
        platform::linux::GLXContextState::done_current();

        #[cfg(target_os = "macos")]
        platform::macos::CGLContextState::done_current();
    }
}

impl Hash for GarchGLPlatformContextState {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.get_hash().hash(state);
    }
}

impl PartialEq for GarchGLPlatformContextState {
    fn eq(&self, other: &Self) -> bool {
        self.inner.get_hash() == other.inner.get_hash()
    }
}

impl Eq for GarchGLPlatformContextState {}

/// Null context implementation (always invalid).
#[derive(Clone, Debug)]
struct NullContextState;

impl GarchGLPlatformContext for NullContextState {
    fn make_current(&self) {
        // No-op
    }

    fn is_valid(&self) -> bool {
        false
    }

    fn is_sharing(&self, _other: &dyn GarchGLPlatformContext) -> bool {
        false
    }

    fn get_hash(&self) -> u64 {
        0
    }

    fn clone_box(&self) -> Box<dyn GarchGLPlatformContext> {
        Box::new(Self)
    }
}

/// Platform-specific context implementations.
mod platform {
    use super::*;

    #[cfg(target_os = "windows")]
    pub mod windows {
        use super::*;
        use std::hash::Hasher;
        use windows_sys::Win32::Graphics::Gdi::HDC;
        use windows_sys::Win32::Graphics::OpenGL::HGLRC;
        use windows_sys::Win32::Graphics::OpenGL::{
            wglGetCurrentContext, wglGetCurrentDC, wglMakeCurrent,
        };

        /// Windows WGL context state.
        ///
        /// Stores HDC (device context) and HGLRC (OpenGL render context) handles
        /// for capturing and restoring GL context state on Windows.
        #[derive(Clone, Debug)]
        pub struct WGLContextState {
            /// Device context handle
            hdc: HDC,
            /// OpenGL render context handle
            hglrc: HGLRC,
        }

        impl WGLContextState {
            /// Capture current WGL context.
            pub fn current() -> Self {
                // SAFETY: Win32 API calls to query current GL context
                #[allow(unsafe_code)]
                unsafe {
                    Self {
                        hdc: wglGetCurrentDC(),
                        hglrc: wglGetCurrentContext(),
                    }
                }
            }

            /// Make no context current on this thread.
            pub fn done_current() {
                // SAFETY: Win32 API call to release current context
                #[allow(unsafe_code)]
                unsafe {
                    wglMakeCurrent(std::ptr::null_mut(), std::ptr::null_mut());
                }
            }
        }

        impl GarchGLPlatformContext for WGLContextState {
            fn make_current(&self) {
                if self.hdc != std::ptr::null_mut() && self.hglrc != std::ptr::null_mut() {
                    // SAFETY: Win32 API call with valid HDC/HGLRC handles
                    #[allow(unsafe_code)]
                    unsafe {
                        wglMakeCurrent(self.hdc, self.hglrc);
                    }
                }
            }

            fn is_valid(&self) -> bool {
                self.hglrc != std::ptr::null_mut()
            }

            fn is_sharing(&self, other: &dyn GarchGLPlatformContext) -> bool {
                // Can only compare with same type - check hash equality as proxy
                // Real sharing check would use wglShareLists or compare share groups
                self.get_hash() == other.get_hash()
            }

            fn get_hash(&self) -> u64 {
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                (self.hdc as usize).hash(&mut hasher);
                (self.hglrc as usize).hash(&mut hasher);
                hasher.finish()
            }

            fn clone_box(&self) -> Box<dyn GarchGLPlatformContext> {
                Box::new(self.clone())
            }
        }

        // SAFETY: WGL handles (HDC, HGLRC) are opaque OS handles that can be
        // sent between threads. OpenGL context operations are serialized by the driver.
        #[allow(unsafe_code)]
        unsafe impl Send for WGLContextState {}
        #[allow(unsafe_code)]
        unsafe impl Sync for WGLContextState {}
    }

    #[cfg(target_os = "linux")]
    pub mod linux {
        use super::*;

        /// Linux GLX context state (stub).
        #[derive(Clone, Debug)]
        pub struct GLXContextState {
            // Future: Display*, GLXDrawable, GLXContext
            _display: u64,
            _drawable: u64,
            _context: u64,
        }

        impl GLXContextState {
            /// Capture current GLX context.
            pub fn current() -> Self {
                // Note: Requires GLX bindings. Returns invalid context without GL support.
                Self {
                    _display: 0,
                    _drawable: 0,
                    _context: 0,
                }
            }

            /// Make no context current.
            pub fn done_current() {
                // Note: Requires GLX bindings (glXMakeCurrent)
            }
        }

        impl GarchGLPlatformContext for GLXContextState {
            fn make_current(&self) {
                // Note: Requires GLX bindings (glXMakeCurrent)
            }

            fn is_valid(&self) -> bool {
                // Note: Returns false without GL bindings (no valid context)
                false
            }

            fn is_sharing(&self, _other: &dyn GarchGLPlatformContext) -> bool {
                // Note: Returns false without GL bindings (cannot compare)
                false
            }

            fn get_hash(&self) -> u64 {
                // Note: Returns 0 without GL bindings
                0
            }

            fn clone_box(&self) -> Box<dyn GarchGLPlatformContext> {
                Box::new(self.clone())
            }
        }
    }

    #[cfg(target_os = "macos")]
    pub mod macos {
        use super::*;

        /// macOS CGL/NSOpenGL context state (stub).
        #[derive(Clone, Debug)]
        pub struct CGLContextState {
            // Future: CGLContextObj or NSOpenGLContext*
            _context: u64,
        }

        impl CGLContextState {
            /// Capture current CGL context.
            pub fn current() -> Self {
                // Note: Requires CGL/NSOpenGL bindings. Returns invalid context without GL support.
                Self { _context: 0 }
            }

            /// Make no context current.
            pub fn done_current() {
                // Note: Requires CGL bindings (CGLSetCurrentContext)
            }
        }

        impl GarchGLPlatformContext for CGLContextState {
            fn make_current(&self) {
                // Note: Requires CGL bindings (CGLSetCurrentContext)
            }

            fn is_valid(&self) -> bool {
                // Note: Returns false without GL bindings (no valid context)
                false
            }

            fn is_sharing(&self, _other: &dyn GarchGLPlatformContext) -> bool {
                // Note: Returns false without GL bindings (cannot compare)
                false
            }

            fn get_hash(&self) -> u64 {
                // Note: Returns 0 without GL bindings
                0
            }

            fn clone_box(&self) -> Box<dyn GarchGLPlatformContext> {
                Box::new(self.clone())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_null_context() {
        let ctx = GarchGLPlatformContextState::null();
        assert!(!ctx.is_valid());
    }

    #[test]
    fn test_context_comparison() {
        let ctx1 = GarchGLPlatformContextState::null();
        let ctx2 = GarchGLPlatformContextState::null();
        assert_eq!(ctx1, ctx2);
    }
}
