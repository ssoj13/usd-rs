//! Garch - GL Architecture
//!
//! OpenGL platform abstraction layer providing cross-platform GL context management
//! and capabilities querying. This is the FOUNDATION for all GL-based rendering in Hydra.
//!
//! # Architecture
//!
//! - `gl_api` - GL API version detection and capabilities
//! - `gl_platform_context` - Platform-agnostic context interface
//! - `gl_debug_window` - Minimal debug window for headless GL testing
//! - `tokens` - Garch-specific tokens
//!
//! # Platform Support
//!
//! - Windows (WGL)
//! - Linux (GLX)
//! - macOS (CGL/NSOpenGL)
//!
//! Current implementation provides STUB interfaces for future GL integration.

pub mod gl_api;
pub mod gl_debug_window;
pub mod gl_platform_context;
pub mod tokens;

// Re-exports
pub use gl_api::{GarchGLApiCapabilities, GarchGLApiVersion};
pub use gl_debug_window::{GarchGLDebugWindow, ModifierKeys, MouseButton};
pub use gl_platform_context::{GarchGLPlatformContext, GarchGLPlatformContextState};
pub use tokens::GarchTokens;
