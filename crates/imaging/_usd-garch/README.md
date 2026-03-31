# Garch - GL Architecture

OpenGL platform abstraction layer for Hydra rendering framework.

## Overview

Garch provides cross-platform GL context management and API capability detection. This is the FOUNDATION for all GL-based rendering in Hydra.

## Modules

### `gl_api`
- `GarchGLApiVersion` - GL version enum (3.3, 4.0, 4.5, etc.)
- `GarchGLApiCapabilities` - Query GL version, extensions, vendor info
- `error` - GL error checking utilities

### `gl_platform_context`
- `GarchGLPlatformContext` trait - Platform-agnostic context interface
- `GarchGLPlatformContextState` - Opaque context state handle
- Platform-specific implementations:
  - Windows: WGL (wglMakeCurrent, wglGetCurrentContext)
  - Linux: GLX (glXMakeCurrent, glXGetCurrentContext)
  - macOS: CGL/NSOpenGL (CGLSetCurrentContext)

### `gl_debug_window`
- `GarchGLDebugWindow` - Minimal window for headless GL testing
- Event handling: mouse, keyboard, resize, paint
- Used for unit tests and headless rendering scenarios

### `tokens`
- `GarchTokens` - GL-related token identifiers
- Tokens: opengl, glsl, coreProfile, compatibilityProfile, debugContext

## Current Status: STUB Implementation

All modules currently provide STUB implementations. Future integration will use:

- `gl` or `glow` crate for GL bindings
- `winit` or platform-specific APIs for window management
- Actual GL context creation and querying

## Architecture Design

### Trait-based Context Abstraction

```rust
pub trait GarchGLPlatformContext: Send + Sync + Debug {
    fn make_current(&self);
    fn is_valid(&self) -> bool;
    fn is_sharing(&self, other: &dyn GarchGLPlatformContext) -> bool;
    fn clone_box(&self) -> Box<dyn GarchGLPlatformContext>;
}
```

### Platform-specific Dispatch

Uses `#[cfg(target_os = "...")]` to select implementation:

```rust
#[cfg(target_os = "windows")]
pub mod windows { /* WGL implementation */ }

#[cfg(target_os = "linux")]
pub mod linux { /* GLX implementation */ }

#[cfg(target_os = "macos")]
pub mod macos { /* CGL implementation */ }
```

## Usage Example (Future)

```rust
use usd::imaging::garch::{GarchGLPlatformContextState, GarchGLApiCapabilities};

// Capture current GL context
let ctx = GarchGLPlatformContextState::current();
assert!(ctx.is_valid());

// Query GL capabilities
let caps = GarchGLApiCapabilities::query();
println!("GL Version: {}", caps.version());
println!("Vendor: {}", caps.vendor());

if caps.has_extension("GL_ARB_direct_state_access") {
    // Use DSA
}

// Switch contexts
ctx.make_current();
```

## Reference

Based on OpenUSD `pxr/imaging/garch/`:
- `glApi.h` - GL API definitions
- `glPlatformContext.h` - Context abstraction
- `glDebugWindow.h` - Debug window
- Platform-specific: `glPlatformContextWindows.h`, `glPlatformContextGLX.h`, etc.

## Next Steps

1. Integrate `gl` or `glow` crate for GL bindings
2. Implement actual context querying (glGetString, glGetIntegerv)
3. Add window creation via `winit` or platform APIs
4. Implement context sharing detection
5. Add comprehensive GL error checking
6. Support headless EGL contexts for server-side rendering
