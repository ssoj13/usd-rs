//! OpenGL test context harness for unit tests.
//!
//! Provides best-practice utilities for running GL-using tests with a proper
//! context.
//!
//! # Usage
//!
//! ```ignore
//! #[test]
//! fn test_gl_thing() {
//!     crate::with_gl_context(|| {
//!         let buffer = HgiGLBuffer::new(&desc, None);
//!         assert!(buffer.byte_size() > 0);
//!     });
//! }
//! ```
//!
//! # Best Practices
//!
//! - **Single GL test**: All GL tests run in one `test_all_gl_functionality` test to avoid
//!   winit's one-EventLoop-per-process limit. See `imaging::gl_tests`.
//! - **Thread-local init**: GL context is created once per thread on first use.
//! - **Minimal overhead**: Uses small (256×256) hidden window; context created on first use.
//! - **Feature-gated**: When `opengl` is disabled, tests panic with a clear message.

/// GL test context stub. OpenGL backend has been removed (wgpu only).
pub fn with_gl_context<R>(_f: impl FnOnce() -> R) -> R {
    panic!("GL tests not available: OpenGL backend removed, use wgpu")
}
