//! Color correction support for viewport presentation and capture.
//!
//! Reference: hdx/colorCorrectionTask.cpp — fullscreen color correction pass.
//!
//! On wgpu builds the normal viewport path now stays on GPU:
//! `Engine::render() -> optional sRGB / OCIO compute pass -> native egui texture`.
//! The CPU path in this file remains for non-wgpu backends and for explicit frame
//! capture/export, where readback is still required.

use crate::data_model::{ColorCorrectionMode, OcioSettings};

use super::ViewportState;

// ---------------------------------------------------------------------------
// sRGB LUT
// ---------------------------------------------------------------------------

/// Precomputed sRGB OETF lookup table (linear u8 → sRGB u8).
/// Initialized once, reused every frame.
fn srgb_lut() -> &'static [u8; 256] {
    static LUT: std::sync::OnceLock<[u8; 256]> = std::sync::OnceLock::new();
    LUT.get_or_init(|| {
        let mut lut = [0u8; 256];
        for i in 0..256 {
            let linear = i as f32 / 255.0;
            let srgb = if linear <= 0.0031308 {
                linear * 12.92
            } else {
                1.055 * linear.powf(1.0 / 2.4) - 0.055
            };
            lut[i] = (srgb * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
        }
        lut
    })
}

/// Apply sRGB gamma correction to linear RGBA pixel buffer (in-place).
fn apply_srgb_correction(pixels: &mut [u8]) {
    let lut = srgb_lut();
    for chunk in pixels.chunks_exact_mut(4) {
        chunk[0] = lut[chunk[0] as usize];
        chunk[1] = lut[chunk[1] as usize];
        chunk[2] = lut[chunk[2] as usize];
    }
}

// ---------------------------------------------------------------------------
// OCIO CPU state — cached config + processor, rebuilt on settings change
// ---------------------------------------------------------------------------

/// Cached OCIO state: config + processor, rebuilt when settings change.
/// Reference: hdx/colorCorrectionTask.cpp _CreateOpenColorIOResources.
///
/// The CPU path here is retained as fallback for non-wgpu backends and for
/// explicit frame capture/export.
pub(crate) struct OcioCpuState {
    /// Loaded OCIO config (from $OCIO or builtin ACES 1.3).
    config: Option<vfx_ocio::Config>,
    /// CPU display processor (scene_linear → display/view).
    processor: Option<vfx_ocio::Processor>,
    /// Settings key for cache invalidation ("display/view/colorspace/looks").
    settings_key: String,
    /// Available (display_name, [view_names]) for UI enumeration.
    displays: Vec<(String, Vec<String>)>,
    /// Available colorspace names for UI enumeration.
    colorspaces: Vec<String>,
    /// Whether config has been loaded at least once.
    loaded: bool,
}

impl Default for OcioCpuState {
    fn default() -> Self {
        Self {
            config: None,
            processor: None,
            settings_key: String::new(),
            displays: Vec::new(),
            colorspaces: Vec::new(),
            loaded: false,
        }
    }
}

impl OcioCpuState {
    /// Load OCIO config: $OCIO env → Config::from_file, fallback → builtin ACES 1.3.
    /// Reference: hdx/colorCorrectionTask.cpp config loading + defaults.
    pub fn load_config(&mut self) {
        if self.loaded {
            return;
        }
        self.loaded = true;

        let config = if let Ok(ocio_path) = std::env::var("OCIO") {
            if !ocio_path.is_empty() {
                match vfx_ocio::Config::from_file(&ocio_path) {
                    Ok(cfg) => {
                        log::info!("[ocio] loaded config from $OCIO: {ocio_path}");
                        cfg
                    }
                    Err(e) => {
                        log::warn!(
                            "[ocio] failed to load $OCIO ({ocio_path}): {e}, using builtin ACES"
                        );
                        vfx_ocio::builtin::aces_1_3()
                    }
                }
            } else {
                log::info!("[ocio] $OCIO empty, using builtin ACES 1.3");
                vfx_ocio::builtin::aces_1_3()
            }
        } else {
            log::info!("[ocio] $OCIO not set, using builtin ACES 1.3");
            vfx_ocio::builtin::aces_1_3()
        };

        // Enumerate displays and views for UI.
        self.displays = config
            .displays()
            .displays()
            .iter()
            .map(|d| {
                let views: Vec<String> = d.views().iter().map(|v| v.name().to_string()).collect();
                (d.name().to_string(), views)
            })
            .collect();

        // Enumerate colorspace names for UI.
        self.colorspaces = config.colorspace_names().map(|s| s.to_string()).collect();

        self.config = Some(config);
    }

    /// Available displays: [(display_name, [view_names])].
    pub fn available_displays(&self) -> &[(String, Vec<String>)] {
        &self.displays
    }

    /// Available colorspace names.
    pub fn available_colorspaces(&self) -> &[String] {
        &self.colorspaces
    }

    /// Views for a specific display.
    // DEAD: never called, but useful utility for future OCIO view filtering UI. Keep.
    #[allow(dead_code)]
    pub fn views_for_display(&self, display: &str) -> &[String] {
        self.displays
            .iter()
            .find(|(d, _)| d == display)
            .map(|(_, v)| v.as_slice())
            .unwrap_or(&[])
    }

    /// Ensure processor matches current settings; rebuild if changed.
    /// Reference: hdx/colorCorrectionTask.cpp processor creation with defaults.
    pub fn ensure_processor(&mut self, settings: &OcioSettings) {
        let key = format!(
            "{}/{}/{}/{}",
            settings.display, settings.view, settings.color_space, settings.looks
        );
        if key == self.settings_key && self.processor.is_some() {
            return; // Cache hit
        }

        self.settings_key = key;
        self.processor = None;

        let config = match &self.config {
            Some(c) => c,
            None => return,
        };

        // Resolve display: user setting → config default.
        let display = if settings.display.is_empty() {
            config
                .default_display()
                .map(String::from)
                .unwrap_or_default()
        } else {
            settings.display.clone()
        };

        // Resolve view: user setting → config default for display.
        let view = if settings.view.is_empty() {
            config
                .default_view(&display)
                .map(String::from)
                .unwrap_or_default()
        } else {
            settings.view.clone()
        };

        // Resolve source colorspace: user setting → scene_linear role → "scene_linear".
        let src = if settings.color_space.is_empty() {
            config
                .colorspace("scene_linear")
                .map(|cs| cs.name().to_string())
                .unwrap_or_else(|| "scene_linear".to_string())
        } else {
            settings.color_space.clone()
        };

        if display.is_empty() || view.is_empty() {
            log::warn!("[ocio] no display/view available");
            return;
        }

        // Build processor (with looks if specified).
        let result = if settings.looks.is_empty() {
            config.display_processor(&src, &display, &view)
        } else {
            // Apply looks via combined processor: looks + display transform.
            match config.processor_with_looks(&src, &src, &settings.looks) {
                Ok(looks_proc) => match config.display_processor(&src, &display, &view) {
                    Ok(disp_proc) => vfx_ocio::Processor::combine(&looks_proc, &disp_proc),
                    Err(e) => Err(e),
                },
                Err(e) => Err(e),
            }
        };

        match result {
            Ok(proc) => {
                log::info!("[ocio] display processor: {src} → {display}/{view}");
                self.processor = Some(proc);
            }
            Err(e) => {
                log::warn!("[ocio] failed to create display processor: {e}");
            }
        }
    }

    /// Get cached processor (None if not built or failed).
    pub fn processor(&self) -> Option<&vfx_ocio::Processor> {
        self.processor.as_ref()
    }
}

// ---------------------------------------------------------------------------
// Color correction entry point
// ---------------------------------------------------------------------------

/// Apply OCIO display transform to RGBA u8 pixel buffer (in-place).
/// Converts u8→f32, applies processor, converts f32→u8.
fn apply_ocio_correction(pixels: &mut [u8], proc: &vfx_ocio::Processor) {
    let pixel_count = pixels.len() / 4;
    let mut rgba_buf: Vec<[f32; 4]> = Vec::with_capacity(pixel_count);
    for chunk in pixels.chunks_exact(4) {
        rgba_buf.push([
            chunk[0] as f32 / 255.0,
            chunk[1] as f32 / 255.0,
            chunk[2] as f32 / 255.0,
            chunk[3] as f32 / 255.0,
        ]);
    }

    proc.apply_rgba(&mut rgba_buf);

    for (chunk, rgba) in pixels.chunks_exact_mut(4).zip(rgba_buf.iter()) {
        chunk[0] = (rgba[0] * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
        chunk[1] = (rgba[1] * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
        chunk[2] = (rgba[2] * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
        chunk[3] = (rgba[3] * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
    }
}

/// Apply color correction to pixel buffer based on mode and OCIO settings.
/// Returns owned buffer (may be modified in-place copy or passthrough).
///
/// Reference: hdx/colorCorrectionTask.cpp Execute() — mode dispatch.
pub(super) fn color_correct(
    pixels: &[u8],
    mode: ColorCorrectionMode,
    ocio: &mut OcioCpuState,
    settings: &OcioSettings,
) -> Vec<u8> {
    usd_trace::trace_scope!("color_correct");
    match mode {
        ColorCorrectionMode::Disabled => pixels.to_vec(),
        ColorCorrectionMode::SRGB => {
            let mut buf = pixels.to_vec();
            apply_srgb_correction(&mut buf);
            buf
        }
        ColorCorrectionMode::OpenColorIO => {
            // Lazy-load config on first OCIO use.
            ocio.load_config();
            ocio.ensure_processor(settings);

            match ocio.processor() {
                Some(p) => {
                    let mut buf = pixels.to_vec();
                    apply_ocio_correction(&mut buf, p);
                    buf
                }
                None => {
                    // OCIO unavailable — graceful fallback to sRGB.
                    log::warn!("[viewport] OCIO processor unavailable, falling back to sRGB");
                    let mut buf = pixels.to_vec();
                    apply_srgb_correction(&mut buf);
                    buf
                }
            }
        }
    }
}

/// Update the persistent viewport texture from pixel data.
///
/// Reuses the existing texture handle when dimensions match, avoiding
/// per-frame GPU texture re-creation. Only creates a new handle on
/// first call or viewport resize.
pub(super) fn update_viewport_texture(
    ctx: &egui::Context,
    state: &mut ViewportState,
    pixels: &[u8],
    width: usize,
    height: usize,
) {
    let image = egui::ColorImage::from_rgba_unmultiplied([width, height], pixels);

    if let Some(ref mut handle) = state.texture_handle {
        if state.last_width == width && state.last_height == height {
            handle.set(image, egui::TextureOptions::NEAREST);
            return;
        }
    }

    state.texture_handle =
        Some(ctx.load_texture("usd_viewport", image, egui::TextureOptions::NEAREST));
    state.last_width = width;
    state.last_height = height;
}
