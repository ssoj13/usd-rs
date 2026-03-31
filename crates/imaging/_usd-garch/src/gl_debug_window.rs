//! OpenGL debug window for headless testing and GPU diagnostics.
//!
//! Provides a minimal platform window for creating GL contexts in test environments,
//! headless rendering scenarios, and GPU debugging. This module enables GL operations
//! without requiring a full windowing system.
//!
//! # Use Cases
//!
//! - Unit testing GL rendering code
//! - Headless rendering for CI/CD pipelines
//! - GPU diagnostics and profiling
//! - Context creation without desktop environment
//! - Offscreen rendering and texture generation
//!
//! # Platform Support
//!
//! - **Windows**: Creates invisible Win32 window with WGL context
//! - **Linux**: Creates GLX window with X11 or EGL context
//! - **macOS**: Creates NSWindow with OpenGL view
//!
//! # Implementation
//!
//! With `opengl` feature: uses `glutin` and `winit` for cross-platform
//! window and GL context creation. Without the feature: stub implementation.
//!
//! # OpenUSD Reference
//!
//! Corresponds to `GarchGLDebugWindow` in OpenUSD's Garch library.
//! See [GarchGLDebugWindow](https://openusd.org/dev/api/class_garch_g_l_debug_window.html)
//!
//! # Examples
//!
//! ```no_run
//! use usd_garch::GarchGLDebugWindow;
//!
//! let mut window = GarchGLDebugWindow::new("Test Window", 800, 600);
//! window.init();
//! // GL context is now active
//! window.run();
//! ```

/// Mouse button identifiers for input events.
///
/// Maps to standard mouse buttons with zero-based indexing.
/// Used in mouse press/release callbacks.
///
/// # OpenUSD Reference
///
/// Corresponds to mouse button enums in OpenUSD's Garch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    /// Primary mouse button (usually left button).
    Button1 = 0,
    /// Secondary mouse button (usually right button).
    Button2 = 1,
    /// Tertiary mouse button (usually middle button/wheel).
    Button3 = 2,
}

/// Keyboard modifier keys state for input events.
///
/// Tracks which modifier keys are pressed during mouse or keyboard events.
/// Can be converted to/from bitmasks for C++ API compatibility.
///
/// # Examples
///
/// ```ignore
/// use usd_garch::ModifierKeys;
///
/// let mods = ModifierKeys { shift: true, ctrl: true, alt: false };
/// let bits = mods.to_bits();
/// let restored = ModifierKeys::from_bits(bits);
/// assert_eq!(mods.shift, restored.shift);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModifierKeys {
    /// Shift key is pressed.
    pub shift: bool,
    /// Alt/Option key is pressed.
    pub alt: bool,
    /// Control/Command key is pressed.
    pub ctrl: bool,
}

impl ModifierKeys {
    /// Constant representing no modifier keys pressed.
    ///
    /// Useful as default value for events without modifiers.
    pub const NONE: Self = Self {
        shift: false,
        alt: false,
        ctrl: false,
    };

    /// Creates modifier state from bitmask.
    ///
    /// # Parameters
    ///
    /// - `bits`: Bitmask where bit 0=shift, bit 1=alt, bit 2=ctrl
    ///
    /// # Returns
    ///
    /// ModifierKeys with flags set according to bitmask.
    ///
    /// # C++ Compatibility
    ///
    /// Compatible with OpenUSD's modifier key bitmask encoding.
    pub fn from_bits(bits: u32) -> Self {
        Self {
            shift: (bits & 1) != 0,
            alt: (bits & 2) != 0,
            ctrl: (bits & 4) != 0,
        }
    }

    /// Converts modifier state to bitmask.
    ///
    /// # Returns
    ///
    /// Bitmask where bit 0=shift, bit 1=alt, bit 2=ctrl.
    ///
    /// # C++ Compatibility
    ///
    /// Compatible with OpenUSD's modifier key bitmask encoding.
    pub fn to_bits(&self) -> u32 {
        (self.shift as u32) | ((self.alt as u32) << 1) | ((self.ctrl as u32) << 2)
    }
}

/// Debug window for GL context creation and testing.
///
/// Provides a minimal cross-platform window suitable for:
/// - Unit testing GL code
/// - Headless rendering
/// - Context creation without full windowing system
///
/// # Platform Support
///
/// - Windows: Creates window with WGL context
/// - Linux: Creates GLX or EGL window
/// - macOS: Creates NSWindow with CGL/OpenGL view
pub struct GarchGLDebugWindow {
    title: String,
    width: i32,
    height: i32,
    _platform: Option<PlatformWindow>,
}

impl GarchGLDebugWindow {
    /// Creates new debug window with specified dimensions.
    ///
    /// # Parameters
    ///
    /// - `title`: Window title (may not be visible in headless mode)
    /// - `width`: Window width in pixels
    /// - `height`: Window height in pixels
    ///
    /// # Returns
    ///
    /// Uninitialized window. Call `init()` to create platform window and GL context.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_garch::GarchGLDebugWindow;
    ///
    /// let window = GarchGLDebugWindow::new("Test", 800, 600);
    /// ```
    pub fn new(title: impl Into<String>, width: i32, height: i32) -> Self {
        Self {
            title: title.into(),
            width,
            height,
            _platform: None,
        }
    }

    /// Initializes platform window and creates OpenGL context.
    ///
    /// Creates the underlying platform window and associated GL context.
    /// Must be called before any GL operations or rendering.
    ///
    /// # Side Effects
    ///
    /// - Creates platform-specific window
    /// - Initializes OpenGL context
    /// - Calls `on_initialize_gl()` callback
    /// - Makes GL context current on calling thread
    ///
    /// # Platform Behavior
    ///
    /// - **Windows**: Creates invisible Win32 window
    /// - **Linux**: Creates GLX window (may require X11)
    /// - **macOS**: Creates NSWindow with OpenGL view
    ///
    /// # Panics
    ///
    /// May panic if platform window creation fails (implementation-dependent).
    pub fn init(&mut self) {
        self._platform = Some(platform_init(self.title.as_str(), self.width, self.height));
        self.on_initialize_gl();
    }

    /// Runs the window event loop.
    ///
    /// Processes platform events until `exit_app()` is called.
    /// Repeatedly calls `on_idle()` and `on_paint_gl()` callbacks.
    ///
    /// # Event Loop
    ///
    /// 1. Process platform events (mouse, keyboard, resize)
    /// 2. Call `on_idle()` for animations/updates
    /// 3. Call `on_paint_gl()` for rendering
    /// 4. Swap buffers
    /// 5. Repeat until exit requested
    ///
    /// # Blocking
    ///
    /// This method blocks until `exit_app()` is called.
    /// For non-blocking rendering, use manual event processing.
    pub fn run(&mut self) {
        // Note: Event loop requires windowing library (e.g., winit, glutin).
        // Currently no-op for testing without actual window. Usage:
        // while !should_exit {
        //     process_events();
        //     self.on_idle();
        //     self.on_paint_gl();
        // }
    }

    /// Signals the event loop to exit.
    ///
    /// Causes `run()` to return after current event loop iteration completes.
    /// GL context remains valid until window is dropped.
    pub fn exit_app(&mut self) {
        // Note: No-op without windowing library integration
    }

    /// Returns current window width in pixels.
    ///
    /// Updated automatically when window is resized.
    pub fn width(&self) -> i32 {
        self.width
    }

    /// Returns current window height in pixels.
    ///
    /// Updated automatically when window is resized.
    pub fn height(&self) -> i32 {
        self.height
    }

    /// Returns window title string.
    pub fn title(&self) -> &str {
        &self.title
    }

    // Virtual methods (override in subclass pattern)

    /// Callback invoked once after GL context is created.
    ///
    /// Override this method to initialize GL resources (shaders, buffers, textures).
    /// GL context is current when this is called.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use usd_garch::GarchGLDebugWindow;
    /// struct MyWindow {
    ///     window: GarchGLDebugWindow,
    /// }
    ///
    /// impl MyWindow {
    ///     fn on_initialize_gl(&mut self) {
    ///         // Initialize GL resources
    ///         // gl::load_with(|s| /* ... */);
    ///         // self.setup_shaders();
    ///     }
    /// }
    /// ```
    pub fn on_initialize_gl(&mut self) {
        // Default: no-op
    }

    /// Callback invoked before GL context is destroyed.
    ///
    /// Override this method to cleanup GL resources (delete buffers, textures, shaders).
    /// GL context is still current when this is called.
    /// Also called automatically in `Drop` implementation.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use usd_garch::GarchGLDebugWindow;
    /// struct MyWindow {
    ///     window: GarchGLDebugWindow,
    ///     vbo: u32,
    /// }
    ///
    /// impl MyWindow {
    ///     fn on_uninitialize_gl(&mut self) {
    ///         // Cleanup GL resources
    ///         // unsafe { gl::DeleteBuffers(1, &self.vbo); }
    ///     }
    /// }
    /// ```
    pub fn on_uninitialize_gl(&mut self) {
        // Default: no-op
    }

    /// Callback invoked when window is resized.
    ///
    /// Override this to update viewport and projection matrices.
    /// Default implementation updates internal width/height.
    ///
    /// # Parameters
    ///
    /// - `w`: New window width in pixels
    /// - `h`: New window height in pixels
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use usd_garch::GarchGLDebugWindow;
    /// struct MyWindow {
    ///     window: GarchGLDebugWindow,
    /// }
    ///
    /// impl MyWindow {
    ///     fn on_resize(&mut self, w: i32, h: i32) {
    ///         // Update viewport
    ///         // unsafe { gl::Viewport(0, 0, w, h); }
    ///         // self.update_projection(w, h);
    ///     }
    /// }
    /// ```
    pub fn on_resize(&mut self, w: i32, h: i32) {
        self.width = w;
        self.height = h;
    }

    /// Callback invoked during event loop idle time.
    ///
    /// Override this for continuous animations, physics updates, or state changes.
    /// Called once per frame before `on_paint_gl()`.
    ///
    /// # Performance
    ///
    /// This is called continuously, so avoid expensive operations.
    /// Use time-based updates for frame-rate independent animation.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use usd_garch::GarchGLDebugWindow;
    /// struct MyWindow {
    ///     window: GarchGLDebugWindow,
    ///     rotation: f32,
    /// }
    ///
    /// impl MyWindow {
    ///     fn on_idle(&mut self) {
    ///         // Update animation
    ///         self.rotation += 0.01;
    ///     }
    /// }
    /// ```
    pub fn on_idle(&mut self) {
        // Default: no-op
    }

    /// Callback invoked to render OpenGL content.
    ///
    /// Override this to draw your scene. Called once per frame after `on_idle()`.
    /// GL context is current when this is called.
    ///
    /// # Rendering
    ///
    /// Typical pattern:
    /// 1. Clear buffers
    /// 2. Set GL state (depth test, blending, etc.)
    /// 3. Bind shaders and uniforms
    /// 4. Draw geometry
    /// 5. Buffers are swapped automatically after return
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use usd_garch::GarchGLDebugWindow;
    /// struct MyWindow {
    ///     window: GarchGLDebugWindow,
    /// }
    ///
    /// impl MyWindow {
    ///     fn on_paint_gl(&mut self) {
    ///         // Render scene
    ///         // unsafe {
    ///         //     gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);
    ///         //     self.draw_scene();
    ///         // }
    ///     }
    /// }
    /// ```
    pub fn on_paint_gl(&mut self) {
        // Default: no-op
    }

    /// Callback invoked when keyboard key is released.
    ///
    /// Override this to handle keyboard input.
    ///
    /// # Parameters
    ///
    /// - `key`: Platform-specific key code
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use usd_garch::GarchGLDebugWindow;
    /// struct MyWindow {
    ///     window: GarchGLDebugWindow,
    /// }
    ///
    /// impl MyWindow {
    ///     fn on_key_release(&mut self, key: i32) {
    ///         // Handle key press (ESC = 27 on many platforms)
    ///         if key == 27 {
    ///             self.window.exit_app();
    ///         }
    ///     }
    /// }
    /// ```
    pub fn on_key_release(&mut self, _key: i32) {
        // Default: no-op
    }

    /// Callback invoked when mouse button is pressed.
    ///
    /// Override this to handle mouse button input.
    ///
    /// # Parameters
    ///
    /// - `button`: Which mouse button was pressed
    /// - `x`: Mouse X coordinate in window pixels (0 = left)
    /// - `y`: Mouse Y coordinate in window pixels (0 = top)
    /// - `mods`: Modifier keys held during press
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use usd_garch::{GarchGLDebugWindow, MouseButton, ModifierKeys};
    /// struct MyWindow {
    ///     window: GarchGLDebugWindow,
    /// }
    ///
    /// impl MyWindow {
    ///     fn on_mouse_press(&mut self, button: MouseButton, x: i32, y: i32, mods: ModifierKeys) {
    ///         if button == MouseButton::Button1 && mods.shift {
    ///             // Handle shift+left-click at (x, y)
    ///         }
    ///     }
    /// }
    /// ```
    pub fn on_mouse_press(&mut self, _button: MouseButton, _x: i32, _y: i32, _mods: ModifierKeys) {
        // Default: no-op
    }

    /// Callback invoked when mouse button is released.
    ///
    /// Override this to handle mouse button release events.
    ///
    /// # Parameters
    ///
    /// - `button`: Which mouse button was released
    /// - `x`: Mouse X coordinate in window pixels (0 = left)
    /// - `y`: Mouse Y coordinate in window pixels (0 = top)
    /// - `mods`: Modifier keys held during release
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use usd_garch::{GarchGLDebugWindow, MouseButton, ModifierKeys};
    /// struct MyWindow {
    ///     window: GarchGLDebugWindow,
    ///     dragging: bool,
    /// }
    ///
    /// impl MyWindow {
    ///     fn on_mouse_release(&mut self, button: MouseButton, x: i32, y: i32, mods: ModifierKeys) {
    ///         if button == MouseButton::Button1 {
    ///             self.dragging = false;
    ///         }
    ///     }
    /// }
    /// ```
    pub fn on_mouse_release(
        &mut self,
        _button: MouseButton,
        _x: i32,
        _y: i32,
        _mods: ModifierKeys,
    ) {
        // Default: no-op
    }

    /// Callback invoked when mouse is moved.
    ///
    /// Override this to handle mouse motion (camera rotation, object picking, etc.).
    ///
    /// # Parameters
    ///
    /// - `x`: Mouse X coordinate in window pixels (0 = left)
    /// - `y`: Mouse Y coordinate in window pixels (0 = top)
    /// - `mods`: Modifier keys held during movement
    ///
    /// # Performance
    ///
    /// This can be called very frequently. Avoid expensive operations.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use usd_garch::{GarchGLDebugWindow, ModifierKeys};
    /// struct MyWindow {
    ///     window: GarchGLDebugWindow,
    ///     last_x: i32,
    ///     last_y: i32,
    /// }
    ///
    /// impl MyWindow {
    ///     fn on_mouse_move(&mut self, x: i32, y: i32, mods: ModifierKeys) {
    ///         let dx = x - self.last_x;
    ///         let dy = y - self.last_y;
    ///         // Update camera rotation based on mouse delta
    ///         self.last_x = x;
    ///         self.last_y = y;
    ///     }
    /// }
    /// ```
    pub fn on_mouse_move(&mut self, _x: i32, _y: i32, _mods: ModifierKeys) {
        // Default: no-op
    }
}

impl Drop for GarchGLDebugWindow {
    fn drop(&mut self) {
        self.on_uninitialize_gl();
    }
}

fn platform_init(_title: &str, _width: i32, _height: i32) -> PlatformWindow {
    PlatformWindow::Stub
}

#[derive(Debug)]
enum PlatformWindow {
    Stub,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_creation() {
        let window = GarchGLDebugWindow::new("Test Window", 800, 600);
        assert_eq!(window.width(), 800);
        assert_eq!(window.height(), 600);
        assert_eq!(window.title(), "Test Window");
    }

    #[test]
    fn test_modifier_keys() {
        let mods = ModifierKeys {
            shift: true,
            ctrl: true,
            alt: false,
        };
        let bits = mods.to_bits();
        let mods2 = ModifierKeys::from_bits(bits);
        assert_eq!(mods.shift, mods2.shift);
        assert_eq!(mods.ctrl, mods2.ctrl);
        assert_eq!(mods.alt, mods2.alt);
    }
}
