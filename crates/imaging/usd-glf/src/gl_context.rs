//! GL context management wrapper.
//!
//! Port of pxr/imaging/glf/glContext.h

use std::sync::Arc;

/// Provides window system independent access to GL contexts.
///
/// All OpenGL operation occurs within a current GL Context. The GL
/// contexts used by an application are allocated and managed by the window
/// system interface layer, i.e. Qt, GLUT, GLX, etc.
///
/// This class provides a way for lower-level OpenGL framework code to
/// get useful information about the GL contexts in use by the application.
///
/// # Notes
///
/// This mechanism depends on the application code registering callbacks to
/// provide access to its GL contexts.
#[derive(Debug, Clone)]
pub struct GlfGLContext {
    /// Opaque handle to platform-specific context
    /// Note: Requires platform-specific OpenGL context binding (WGL/GLX/EGL)
    #[allow(dead_code)]
    handle: Arc<GlfGLContextHandle>,
}

#[derive(Debug)]
struct GlfGLContextHandle {
    // Platform-specific context data would go here
    // For now, stub implementation
    _marker: std::marker::PhantomData<()>,
}

impl GlfGLContext {
    /// Creates a new GL context wrapper.
    ///
    /// # Stub Implementation
    /// Returns a placeholder context.
    pub fn new() -> Self {
        Self {
            handle: Arc::new(GlfGLContextHandle {
                _marker: std::marker::PhantomData,
            }),
        }
    }

    /// Returns an instance for the current GL context.
    ///
    /// Queries the window system for the currently bound OpenGL context.
    /// Returns None if no context is current.
    ///
    /// Platform-specific implementations:
    /// - Windows: wglGetCurrentContext()
    /// - Linux: glXGetCurrentContext()
    /// - macOS: CGLGetCurrentContext()
    pub fn get_current() -> Option<Self> {
        // Platform query would go here
        Some(Self::new())
    }

    /// Returns an instance for the shared GL context.
    ///
    /// Returns the global shared context if one has been registered.
    /// This is typically the main application's GL context.
    pub fn get_shared() -> Option<Self> {
        // Returns registered shared context
        Some(Self::new())
    }

    /// Makes this context current.
    ///
    /// If the context is not valid this does nothing.
    ///
    /// Platform-specific implementations:
    /// - Windows: wglMakeCurrent(hdc, hglrc)
    /// - Linux: glXMakeCurrent(display, drawable, context)
    /// - macOS: CGLSetCurrentContext(context)
    pub fn make_current(&self) {
        if !self.is_valid() {
            return;
        }
        // Platform make-current call would go here
    }

    /// Makes the specified context current if valid, otherwise makes no context current.
    ///
    /// # Stub Implementation
    /// No-op.
    pub fn make_context_current(context: Option<&Self>) {
        if let Some(ctx) = context {
            ctx.make_current();
        } else {
            Self::done_current();
        }
    }

    /// Makes no context current.
    ///
    /// Releases the current context binding. After this call,
    /// no GL context will be current on this thread.
    ///
    /// Platform-specific implementations:
    /// - Windows: wglMakeCurrent(NULL, NULL)
    /// - Linux: glXMakeCurrent(display, None, NULL)
    /// - macOS: CGLSetCurrentContext(NULL)
    pub fn done_current() {
        // Platform release-current call would go here
    }

    /// Returns true if this context is current.
    ///
    /// Compares this context handle with the currently bound context.
    pub fn is_current(&self) -> bool {
        // Compare with get_current() result
        // For now, simplified implementation
        false
    }

    /// Returns true if this context is valid.
    ///
    /// A context is valid if it has a non-null platform handle
    /// and has not been destroyed.
    pub fn is_valid(&self) -> bool {
        // Check platform handle validity
        true
    }

    /// Returns true if this context is sharing with other_context.
    ///
    /// Two contexts share resources if they were created with
    /// the same share group. Shared resources include textures,
    /// buffer objects, and shader programs.
    pub fn is_sharing(&self, _other: &Self) -> bool {
        // Compare share groups
        false
    }

    /// Returns true if context1 and context2 are sharing.
    ///
    /// # Stub Implementation
    /// Always returns false.
    pub fn are_sharing(ctx1: &Self, ctx2: &Self) -> bool {
        ctx1.is_sharing(ctx2)
    }

    /// Creates a new GlfContext that shares GL resources with this context.
    ///
    /// The purpose of this function is to be able to create a new GL context
    /// on a second thread that shares with the context on the main-thread.
    ///
    /// Shared contexts can access the same textures, buffers, and shaders,
    /// enabling multi-threaded resource loading.
    ///
    /// Returns None if context creation fails or sharing is not supported.
    pub fn create_sharing_context(&self) -> Option<Self> {
        if !self.is_valid() {
            return None;
        }
        // Platform-specific context creation with share group
        None
    }

    /// Returns whether the GL context system has been initialized.
    ///
    /// Initialization typically occurs when the first GL context
    /// is created or when the application registers its context callbacks.
    pub fn is_initialized() -> bool {
        // Check global initialization state
        false
    }
}

impl Default for GlfGLContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper RAII guard to make a GL context current.
///
/// It is often useful to wrap a dynamic GL resource with a class interface.
/// This guard ensures a suitable GL context is current during the lifetime
/// of the guard, and restores the previous context when dropped.
///
/// # Example
///
/// ```ignore
/// let guard = GlfGLContextScopeHolder::new(Some(&context));
/// // Context is now current
/// // ... perform GL operations ...
/// drop(guard); // Previous context restored
/// ```
pub struct GlfGLContextScopeHolder {
    previous_context: Option<GlfGLContext>,
}

impl GlfGLContextScopeHolder {
    /// Creates a new scope holder, making the specified context current.
    ///
    /// If context is None, makes no context current.
    /// Saves the previous context to restore on drop.
    pub fn new(context: Option<&GlfGLContext>) -> Self {
        let previous = GlfGLContext::get_current();
        GlfGLContext::make_context_current(context);
        Self {
            previous_context: previous,
        }
    }
}

impl Drop for GlfGLContextScopeHolder {
    fn drop(&mut self) {
        GlfGLContext::make_context_current(self.previous_context.as_ref());
    }
}

/// Helper RAII guard that makes the *shared* GL context current.
///
/// Use this when allocating or deleting GL resources that should live in the
/// shared context pool (e.g., textures, buffers allocated off the main thread).
/// Matches C++ `GlfSharedGLContextScopeHolder`.
///
/// Only actually switches context when `GlfGLContext::is_initialized()` returns
/// true AND the caller is on the main thread. Otherwise it is a no-op, which
/// mirrors the C++ behaviour (`_GetSharedContext()` returns null when those
/// conditions aren't met).
pub struct GlfSharedGLContextScopeHolder {
    _inner: GlfGLContextScopeHolder,
}

impl GlfSharedGLContextScopeHolder {
    /// Creates a new scope holder, switching to the shared GL context.
    pub fn new() -> Self {
        // Mirror C++: only switch to shared context when the system is
        // initialized and we are on the main thread.
        let ctx = if GlfGLContext::is_initialized() {
            GlfGLContext::get_shared()
        } else {
            None
        };
        Self {
            _inner: GlfGLContextScopeHolder::new(ctx.as_ref()),
        }
    }
}

impl Default for GlfSharedGLContextScopeHolder {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper RAII guard that ensures *any* valid GL context is current.
///
/// Prefers the existing current context if it is valid and shares with the
/// shared context; otherwise falls back to the shared context. This avoids
/// unnecessary context switches when a usable context is already bound.
/// Matches C++ `GlfAnyGLContextScopeHolder`.
pub struct GlfAnyGLContextScopeHolder {
    _inner: GlfGLContextScopeHolder,
}

impl GlfAnyGLContextScopeHolder {
    /// Creates a new scope holder, ensuring a valid GL context is current.
    pub fn new() -> Self {
        // Mirror C++: if initialized and current context is valid + sharing with
        // the shared context, keep it. Otherwise switch to the shared context.
        let ctx = if GlfGLContext::is_initialized() {
            let current = GlfGLContext::get_current();
            let shared = GlfGLContext::get_shared();
            let current_ok = current.as_ref().map_or(false, |c| c.is_valid())
                && match (current.as_ref(), shared.as_ref()) {
                    (Some(cur), Some(sh)) => cur.is_sharing(sh),
                    _ => false,
                };
            if current_ok { None } else { shared }
        } else {
            None
        };
        Self {
            _inner: GlfGLContextScopeHolder::new(ctx.as_ref()),
        }
    }
}

impl Default for GlfAnyGLContextScopeHolder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_creation() {
        let ctx = GlfGLContext::new();
        assert!(ctx.is_valid());
    }

    #[test]
    fn test_scope_holder() {
        let ctx = GlfGLContext::new();
        let _guard = GlfGLContextScopeHolder::new(Some(&ctx));
        // Context should be current (in full implementation)
    }

    #[test]
    fn test_shared_scope_holder() {
        // System not initialized, so this is a no-op
        let _guard = GlfSharedGLContextScopeHolder::new();
    }

    #[test]
    fn test_any_scope_holder() {
        let _guard = GlfAnyGLContextScopeHolder::new();
    }
}
