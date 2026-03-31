
//! Renderer creation arguments.
//!
//! Corresponds to pxr/imaging/hd/rendererCreateArgs.h.
//! Contains members indicating the resources available when creating a renderer plugin.

/// Arguments passed when creating a renderer plugin.
///
/// Corresponds to C++ `HdRendererCreateArgs`.
/// Indicates resources (e.g., GPU, HGI) available for the renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HdRendererCreateArgs {
    /// Whether the GPU is available or not.
    pub gpu_enabled: bool,

    /// Optional HGI instance for backend support checks.
    /// Forward-declared as opaque pointer in C++; use when checking
    /// renderer backend capabilities against an Hgi device.
    pub hgi: Option<*mut std::ffi::c_void>,
}

impl HdRendererCreateArgs {
    /// Create args with default values (GPU enabled, no HGI).
    pub fn new() -> Self {
        Self::default()
    }

    /// Create args with GPU disabled.
    pub fn gpu_disabled() -> Self {
        Self {
            gpu_enabled: false,
            hgi: None,
        }
    }

    /// Create args with HGI instance for backend checks.
    pub fn with_hgi(hgi: *mut std::ffi::c_void) -> Self {
        Self {
            gpu_enabled: true,
            hgi: if hgi.is_null() { None } else { Some(hgi) },
        }
    }

    /// Set GPU enabled flag.
    pub fn set_gpu_enabled(&mut self, enabled: bool) {
        self.gpu_enabled = enabled;
    }

    /// Set HGI instance.
    pub fn set_hgi(&mut self, hgi: Option<*mut std::ffi::c_void>) {
        self.hgi = hgi.and_then(|p| if p.is_null() { None } else { Some(p) });
    }
}
