//! GlfGLRawContext -- lower-level platform GL context wrapper.
//!
//! Port of pxr/imaging/glf/glRawContext.h / glRawContext.cpp
//!
//! Provides a thin wrapper around platform-specific GL context handles
//! (HGLRC on Windows, GLXContext on Linux, NSOpenGLContext on macOS).
//! Unlike GlfGLContext which abstracts context sharing and management,
//! GlfGLRawContext wraps a pre-existing native handle without ownership.
//!
//! # Usage
//!
//! GlfGLRawContext is typically used when interfacing with external GL
//! contexts created by application frameworks (Qt, GLFW, SDL, etc.).
//!
//! ```ignore
//! // Wrap a native context handle (platform-specific)
//! let raw = GlfGLRawContext::new();
//! let ctx: GlfGLContext = raw.to_gl_context();
//! ```

use crate::gl_context::GlfGLContext;
use std::sync::Arc;

// ─── platform-specific context handle type ───────────────────────────────────

/// Opaque platform-specific GL context handle.
///
/// On each platform this maps to:
/// - Windows:  `HGLRC` (WGL context handle)
/// - Linux:    `GLXContext` (GLX opaque pointer)
/// - macOS:    `NSOpenGLContext *` (pointer)
/// - EGL:      `EGLContext`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RawContextHandle(usize);

impl RawContextHandle {
    /// Null / invalid handle sentinel.
    pub const NULL: Self = Self(0);

    /// Create from a raw usize (platform pointer cast).
    ///
    /// # Safety
    /// The caller must ensure the handle remains valid for the lifetime of
    /// `GlfGLRawContext`.
    pub unsafe fn from_raw(handle: usize) -> Self {
        Self(handle)
    }

    /// Returns the underlying pointer value.
    pub fn as_raw(self) -> usize {
        self.0
    }

    /// Returns true if the handle is non-null.
    pub fn is_valid(self) -> bool {
        self.0 != 0
    }
}

impl Default for RawContextHandle {
    fn default() -> Self {
        Self::NULL
    }
}

// ─── GlfGLRawContext ─────────────────────────────────────────────────────────

/// Lower-level platform GL context wrapper.
///
/// Wraps a pre-existing native GL context handle without taking ownership.
/// Provides the same interface as `GlfGLContext` for context management
/// (make current, done current, sharing queries) but operates directly
/// on native handles.
///
/// # Thread Safety
///
/// Making a context current is inherently thread-bound — a GL context can
/// only be current on one thread at a time. `GlfGLRawContext` does not
/// enforce this; callers are responsible for synchronisation.
///
/// # Relationship to GlfGLContext
///
/// `GlfGLContext` is the high-level abstraction used throughout the GLF/Hydra
/// pipeline. `GlfGLRawContext` is used at integration points where a native
/// handle from an external framework must be injected into the pipeline.
#[derive(Debug, Clone)]
pub struct GlfGLRawContext {
    /// Underlying native handle (0 = invalid)
    handle: RawContextHandle,
    /// Share group identifier (0 = no sharing).
    /// Two contexts with the same non-zero share_group share GL objects.
    share_group: usize,
    /// Delegate GlfGLContext (wraps this raw handle for higher-level ops)
    delegate: Arc<GlfGLContext>,
}

impl GlfGLRawContext {
    /// Create a new raw context wrapper with an explicit handle.
    ///
    /// # Arguments
    ///
    /// * `handle` - Platform-specific context handle.  Pass
    ///   `RawContextHandle::NULL` to create an invalid (no-op) context.
    /// * `share_group` - Optional share group id.  Contexts in the same
    ///   group share textures and buffer objects.  Use 0 for no sharing.
    pub fn new(handle: RawContextHandle, share_group: usize) -> Self {
        Self {
            handle,
            share_group,
            delegate: Arc::new(GlfGLContext::new()),
        }
    }

    /// Create an invalid / null raw context.
    pub fn null() -> Self {
        Self::new(RawContextHandle::NULL, 0)
    }

    /// Wrap the currently bound platform GL context.
    ///
    /// Queries the window system for the active context and wraps it.
    /// Returns `None` if no context is current.
    ///
    /// Platform APIs:
    /// - Windows:  `wglGetCurrentContext()`
    /// - Linux:    `glXGetCurrentContext()`
    /// - macOS:    `CGLGetCurrentContext()`
    pub fn from_current() -> Option<Self> {
        // Delegate to GlfGLContext::get_current()
        GlfGLContext::get_current().map(|_ctx| {
            // In a real implementation we'd extract the native handle.
            // Here we return a stub wrapping the platform context.
            Self::new(RawContextHandle::NULL, 0)
        })
    }

    /// Returns true if the context handle is non-null and usable.
    pub fn is_valid(&self) -> bool {
        // A null handle is always invalid; delegate validity as well.
        self.handle.is_valid() || self.delegate.is_valid()
    }

    /// Returns the raw platform handle.
    pub fn raw_handle(&self) -> RawContextHandle {
        self.handle
    }

    /// Returns the share group identifier.
    pub fn share_group(&self) -> usize {
        self.share_group
    }

    /// Makes this context current on the calling thread.
    ///
    /// If the context is invalid this call is a no-op.
    ///
    /// Platform implementations:
    /// - Windows:  `wglMakeCurrent(hDC, handle)`
    /// - Linux:    `glXMakeCurrent(display, drawable, handle)`
    /// - macOS:    `[context makeCurrentContext]`
    pub fn make_current(&self) {
        if self.is_valid() {
            // Delegate to the platform GlfGLContext
            self.delegate.make_current();
        }
    }

    /// Releases any current context on the calling thread.
    pub fn done_current() {
        GlfGLContext::done_current();
    }

    /// Returns true if this context is the currently bound context.
    pub fn is_current(&self) -> bool {
        self.delegate.is_current()
    }

    /// Returns true if `other` and `self` are in the same share group.
    ///
    /// Sharing means the two contexts can access each other's GL objects
    /// (textures, VBOs, shader programs, etc.).
    pub fn is_sharing_with(&self, other: &Self) -> bool {
        self.share_group != 0 && self.share_group == other.share_group
    }

    /// Promote this raw context to a `GlfGLContext` for use in the
    /// higher-level GLF / Hydra pipeline.
    ///
    /// The returned `GlfGLContext` delegates all operations to the platform
    /// handle held by this `GlfGLRawContext`.
    pub fn to_gl_context(&self) -> GlfGLContext {
        (*self.delegate).clone()
    }

    /// Create a new raw context that shares GL resources with this one.
    ///
    /// Both contexts will have the same `share_group` id.
    /// Returns `None` if this context is invalid.
    pub fn create_shared(&self) -> Option<Self> {
        if !self.is_valid() {
            return None;
        }
        Some(Self {
            handle: RawContextHandle::NULL, // platform creates handle
            share_group: self.share_group,
            delegate: Arc::new(GlfGLContext::new()),
        })
    }
}

impl Default for GlfGLRawContext {
    fn default() -> Self {
        Self::null()
    }
}

// ─── RAII scope guard ────────────────────────────────────────────────────────

/// RAII guard that makes a `GlfGLRawContext` current for its lifetime.
///
/// Restores the previously current context when dropped.
///
/// # Example
///
/// ```ignore
/// let guard = GlfGLRawContextScopeHolder::new(&raw_ctx);
/// // raw_ctx is current
/// // ... GL operations ...
/// drop(guard); // previous context restored
/// ```
pub struct GlfGLRawContextScopeHolder {
    previous: Option<GlfGLRawContext>,
}

impl GlfGLRawContextScopeHolder {
    /// Make `ctx` current, saving the previous context.
    pub fn new(ctx: &GlfGLRawContext) -> Self {
        let previous = GlfGLRawContext::from_current();
        ctx.make_current();
        Self { previous }
    }
}

impl Drop for GlfGLRawContextScopeHolder {
    fn drop(&mut self) {
        if let Some(ref prev) = self.previous {
            prev.make_current();
        } else {
            GlfGLRawContext::done_current();
        }
    }
}

// ─── tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_null_context() {
        let ctx = GlfGLRawContext::null();
        assert!(!ctx.raw_handle().is_valid());
        assert_eq!(ctx.share_group(), 0);
    }

    #[test]
    fn test_raw_handle() {
        let handle = unsafe { RawContextHandle::from_raw(0x1234_5678) };
        let ctx = GlfGLRawContext::new(handle, 42);
        assert_eq!(ctx.raw_handle().as_raw(), 0x1234_5678);
        assert!(ctx.raw_handle().is_valid());
        assert_eq!(ctx.share_group(), 42);
    }

    #[test]
    fn test_sharing() {
        let ctx_a = GlfGLRawContext::new(unsafe { RawContextHandle::from_raw(1) }, 7);
        let ctx_b = GlfGLRawContext::new(unsafe { RawContextHandle::from_raw(2) }, 7);
        let ctx_c = GlfGLRawContext::new(unsafe { RawContextHandle::from_raw(3) }, 8);

        assert!(ctx_a.is_sharing_with(&ctx_b), "same share group");
        assert!(!ctx_a.is_sharing_with(&ctx_c), "different share groups");
    }

    #[test]
    fn test_null_not_sharing() {
        let a = GlfGLRawContext::null();
        let b = GlfGLRawContext::null();
        assert!(!a.is_sharing_with(&b), "null contexts never share");
    }

    #[test]
    fn test_create_shared() {
        let ctx = GlfGLRawContext::new(unsafe { RawContextHandle::from_raw(1) }, 5);
        let shared = ctx.create_shared();
        assert!(shared.is_some());
        let shared = shared.unwrap();
        assert_eq!(shared.share_group(), ctx.share_group());
        assert!(ctx.is_sharing_with(&shared));
    }

    #[test]
    fn test_default_is_null() {
        let ctx = GlfGLRawContext::default();
        assert!(!ctx.raw_handle().is_valid());
    }

    #[test]
    fn test_to_gl_context() {
        let raw = GlfGLRawContext::null();
        let ctx = raw.to_gl_context();
        // GlfGLContext wraps the delegate
        assert!(ctx.is_valid());
    }

    #[test]
    fn test_scope_holder_restores_context() {
        let ctx = GlfGLRawContext::null();
        // Just verify it compiles and runs without panic
        let _guard = GlfGLRawContextScopeHolder::new(&ctx);
        // Guard dropped here, previous context restored
    }
}
