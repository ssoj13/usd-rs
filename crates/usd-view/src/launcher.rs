//! Launcher for usdview.
//!
//! CLI parsing and native run.

use std::path::PathBuf;

use egui_dock::DockState;
use tracing_subscriber::EnvFilter;

use crate::app::{ViewerApp, ViewerConfig, sync::nearest_highlight_color};
use crate::data_model::{ClearColor, DrawMode, ViewSettingsDataModel};
use crate::dock::{DockTab, default_dock_state, default_dock_state_no_render};
use crate::panels::preferences::{BgPreset, DefaultRenderMode};
use crate::persistence;
use crate::recent_files::RecentFiles;

/// Initialize logging based on ViewerConfig.
///
/// Verbosity levels: -v = INFO, -vv = DEBUG, -vvv = TRACE.
/// Optional --log file output (default: "usdview.log" if flag given without value).
pub fn init_logging(config: &ViewerConfig) {
    // Build filter: 0=warn, 1=info, 2=debug, 3+=trace
    let level = match config.verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };

    // Allow RUST_LOG env override, fall back to computed level
    // Suppress noisy wgpu/vulkan/naga logs unless explicitly requested
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| {
        format!("{level},wgpu_core=warn,wgpu_hal=warn,naga=warn,eframe=info,egui_wgpu=warn")
    });

    let subscriber = tracing_subscriber::fmt().with_env_filter(EnvFilter::new(&filter));

    if let Some(ref log_path) = config.log_file {
        let file = std::fs::File::create(log_path)
            .unwrap_or_else(|e| panic!("Cannot open log file {log_path}: {e}"));
        subscriber
            .with_writer(std::sync::Mutex::new(file))
            .with_ansi(false)
            .init();
        eprintln!("Logging to {log_path} (level: {filter})");
    } else {
        subscriber.init();
    }

    // Bridge log crate -> tracing (for crates using log::trace! etc.)
    let _ = tracing_log::LogTracer::init();

    // Mirror uncaught panics into the same tracing sink so GUI crashes still
    // leave actionable breadcrumbs in the log file.
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let backtrace = std::backtrace::Backtrace::force_capture();
        tracing::error!("panic: {panic_info}\nbacktrace:\n{backtrace}");
        previous_hook(panic_info);
    }));

    tracing::info!("usdview logging initialized (level: {filter})");
}

/// Runs the viewer with the given config.
pub fn run(config: ViewerConfig) -> eframe::Result<()> {
    usd_trace::trace_scope!("eframe_run_native");
    let _app_t0 = std::time::Instant::now();
    // Pre-load state to get window size before eframe init
    let pre_saved = if config.clear_settings {
        persistence::AppPersistState::default()
    } else {
        persistence::load_state()
    };
    let win_size = pre_saved.window_size.unwrap_or([1280.0, 800.0]);
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("usdview — USD Scene Viewer")
        .with_inner_size(win_size)
        .with_min_inner_size([800.0, 500.0]);
    if let Some([x, y]) = pre_saved.window_pos {
        viewport = viewport.with_position([x, y]);
    }
    let mut options = eframe::NativeOptions {
        viewport,
        persist_window: false, // we manage window pos/size ourselves now
        #[cfg(feature = "wgpu")]
        renderer: eframe::Renderer::Wgpu,
        #[cfg(not(feature = "wgpu"))]
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    #[cfg(feature = "wgpu")]
    {
        options.wgpu_options.preferred_surface_formats = vec![
            egui_wgpu::wgpu::TextureFormat::Rgb10a2Unorm,
            egui_wgpu::wgpu::TextureFormat::Rgba16Float,
            egui_wgpu::wgpu::TextureFormat::Rgba8Unorm,
            egui_wgpu::wgpu::TextureFormat::Bgra8Unorm,
        ];
        options.wgpu_options.wgpu_setup =
            egui_wgpu::WgpuSetup::CreateNew(egui_wgpu::WgpuSetupCreateNew {
                device_descriptor: std::sync::Arc::new(|adapter| {
                    usd_hgi_wgpu::HgiWgpu::create_device_descriptor(adapter, "usd-view")
                }),
                ..Default::default()
            });
    }

    log::trace!("[PERF] launcher pre-init: {:?}", _app_t0.elapsed());
    eframe::run_native(
        "usdview",
        options,
        Box::new(move |cc| {
            // Default body font size 10pt to match C++ usdview (egui default is 14)
            let mut style = (*cc.egui_ctx.style()).clone();
            let scale = 10.0 / 14.0;
            for (_ts, font_id) in style.text_styles.iter_mut() {
                font_id.size = (font_id.size * scale).round();
            }
            cc.egui_ctx.set_style(style);

            let config = config.clone();

            // --clearsettings: wipe persisted JSON and start fresh
            if config.clear_settings {
                persistence::delete_state();
            }

            // Use pre-loaded state (avoid double disk read)
            let saved = pre_saved;

            // Restore dock layout (no_render ignores saved layout)
            let dock_state: DockState<DockTab> = if config.no_render {
                default_dock_state_no_render()
            } else {
                let restored: DockState<DockTab> = saved
                    .dock_layout
                    .as_deref()
                    .and_then(|s| ron::from_str(s).ok())
                    .unwrap_or_else(default_dock_state);
                // Safety: if persisted layout lost the Viewport tab, reset to default
                let has_viewport = restored
                    .iter_all_tabs()
                    .any(|(_, tab)| *tab == DockTab::Viewport);
                if has_viewport {
                    restored
                } else {
                    default_dock_state()
                }
            };

            let mut app = ViewerApp::new(config.clone(), dock_state);
            #[cfg(feature = "wgpu")]
            if let Some(render_state) = cc.wgpu_render_state.clone() {
                app.configure_wgpu_render_state(render_state);
            }

            // Restore view settings
            let view_settings_restored = if let Some(ref s) = saved.view_settings {
                if let Ok(settings) = ron::from_str::<ViewSettingsDataModel>(s) {
                    app.data_model.view = settings;
                    true
                } else {
                    false
                }
            } else {
                false
            };

            // CLI overrides win
            if config.bbox_standin {
                app.data_model.view.show_bboxes = true;
            }
            if let Some(c) = config.complexity {
                app.data_model.view.complexity = c;
            }

            // Restore preferences
            app.prefs_state.settings = saved.preferences.to_prefs();

            // Apply prefs that are "startup defaults" only — skipped when persisted
            // view settings were already restored (user's last session state wins).
            if !view_settings_restored {
                // Default render mode from Preferences > Viewport
                app.data_model.view.draw_mode = match app.prefs_state.settings.default_render_mode {
                    DefaultRenderMode::SmoothShaded => DrawMode::ShadedSmooth,
                    DefaultRenderMode::FlatShaded => DrawMode::ShadedFlat,
                    DefaultRenderMode::Wireframe => DrawMode::Wireframe,
                    DefaultRenderMode::WireframeOnSurface => DrawMode::WireframeOnSurface,
                    DefaultRenderMode::Points => DrawMode::Points,
                };

                // Background color from Preferences > Viewport
                app.data_model.view.clear_color = match app.prefs_state.settings.background {
                    BgPreset::Black => ClearColor::Black,
                    // Gradient maps to DarkGrey (no gradient shader yet)
                    BgPreset::DarkGrey | BgPreset::Gradient => ClearColor::DarkGrey,
                };

                // Selection highlight color from Preferences > Viewport
                app.data_model.view.highlight_color =
                    nearest_highlight_color(app.prefs_state.settings.selection_highlight_color);
            }

            // Restore recent files
            app.recent_files = RecentFiles::from_vec(saved.recent_files.clone());

            // Restore named layouts
            app.layouts = saved.layouts.clone();
            app.current_layout = saved.current_layout.clone();

            // Determine file to load: CLI arg wins, else last_file from state
            let file_to_load = config
                .initial_file
                .clone()
                .or_else(|| saved.last_file.as_ref().map(|p| strip_unc_prefix(p)));
            if let Some(ref path) = file_to_load {
                app.load_file(path);
            }

            if config.autoplay {
                app.playback.play();
            }

            // Restore HDRI environment map from persisted view settings
            #[cfg(feature = "wgpu")]
            if let Some(ref hdr) = app.data_model.view.hdr_path {
                let hdr = hdr.clone();
                app.engine.set_dome_light_texture_path(Some(hdr));
                if app.data_model.view.dome_light_enabled {
                    app.engine.set_dome_light_enabled(true);
                }
            }

            Ok(Box::new(app))
        }),
    )
}

/// Parses CLI arguments and returns config.
pub fn parse_args(
    args: impl Iterator<Item = String>,
) -> Result<ViewerConfig, Box<dyn std::error::Error>> {
    let mut config = ViewerConfig::default();
    let mut args = args.skip(1).peekable();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-v" => config.verbose = config.verbose.saturating_add(1),
            "-vv" => config.verbose = 2,
            "-vvv" => config.verbose = 3,
            "--verbose" => config.verbose = config.verbose.saturating_add(1),
            "-l" | "--log" => {
                // Always log to usdview.log (use --log-file for custom path)
                config.log_file = Some("usdview.log".to_string());
            }
            "--log-file" => {
                config.log_file = args.next().or(Some("usdview.log".to_string()));
            }
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            "--select" => {
                config.initial_select = args.next();
            }
            "--camera" => {
                config.camera_prim = args.next();
            }
            "--mask" => {
                config.population_mask = args.next();
            }
            "--unloaded" => config.unloaded = true,
            "-s" | "--screenshot" => {
                config.screenshot = args.next().map(PathBuf::from);
            }
            "--delay" => {
                if let Some(s) = args.next() {
                    config.screenshot_delay = parse_delay(&s);
                }
            }
            "--play" => config.autoplay = true,
            "--norender" => config.no_render = true,
            "--profile" => config.profile = true,
            "--mem-profile" => config.mem_profile = true,
            "--bboxStandin" => config.bbox_standin = true,
            "--clearsettings" | "--defaultsettings" => config.clear_settings = true,
            "--complexity" => {
                if let Some(s) = args.next() {
                    // Accept named IDs (low/medium/high/veryhigh) or numeric (1.0-1.3)
                    use crate::data_model::RefinementComplexity;
                    config.complexity = RefinementComplexity::from_id(&s)
                        .map(|c| c.value())
                        .or_else(|| s.parse().ok());
                }
            }
            "--ff" => {
                if let Some(s) = args.next() {
                    config.frame_first = s.parse().ok();
                }
            }
            "--lf" => {
                if let Some(s) = args.next() {
                    config.frame_last = s.parse().ok();
                }
            }
            "--cf" => {
                if let Some(s) = args.next() {
                    config.frame_current = s.parse().ok();
                }
            }
            "--renderer" => {
                config.renderer = args.next();
            }
            "--mute" => {
                // Regex pattern for layer muting (ref: appController.py:1171)
                if let Some(pat) = args.next() {
                    config.mute_layers_re.push(pat);
                }
            }
            _ if arg.starts_with('-') => {
                eprintln!("Unknown option: {}", arg);
            }
            _ => {
                // Resolve to absolute early — CWD may change after eframe window init.
                // Do NOT use canonicalize() — on Windows it returns UNC (\\?\) paths
                // that break SDF layer resolution.
                let p = PathBuf::from(&arg);
                let abs = if p.is_absolute() {
                    p
                } else {
                    std::env::current_dir().unwrap_or_default().join(p)
                };
                config.initial_file = Some(strip_unc_prefix(&abs));
                // Don't break — allow flags after the file path
            }
        }
    }

    Ok(config)
}

/// Strip Windows UNC prefix (\\?\) from paths.
///
/// `canonicalize()` on Windows returns `\\?\C:\...` which breaks SDF layer resolution.
fn strip_unc_prefix(path: &PathBuf) -> PathBuf {
    let s = path.to_string_lossy();
    if s.starts_with(r"\\?\") {
        PathBuf::from(&s[4..])
    } else {
        path.clone()
    }
}

/// Parse delay string: "3s" = 3 seconds, "500ms" = 500 millis, "2" = 2 seconds (default unit).
fn parse_delay(s: &str) -> Option<std::time::Duration> {
    let s = s.trim();
    if let Some(ms) = s.strip_suffix("ms") {
        ms.trim()
            .parse::<u64>()
            .ok()
            .map(std::time::Duration::from_millis)
    } else if let Some(sec) = s.strip_suffix('s') {
        sec.trim()
            .parse::<f64>()
            .ok()
            .map(std::time::Duration::from_secs_f64)
    } else {
        // Bare number = seconds
        s.parse::<f64>()
            .ok()
            .map(std::time::Duration::from_secs_f64)
    }
}

fn print_help() {
    eprintln!(
        r#"usdview — USD Scene Viewer

Usage: usdview [options] [file.usd]

Options:
  -v                INFO logging (-vv = DEBUG, -vvv = TRACE)
  --verbose         Increase verbosity (stackable)
  -l, --log [FILE]  Log to file (default: usdview.log)
  -h, --help        Show this help
  --select PATH     Initial prim to select (default: /)
  --camera PATH     Camera prim for initial view
  --mask PATH,...   Limit stage to these prim paths (comma/space separated)
  --unloaded        Do not load payloads
  -s, --screenshot FILE  Save first rendered frame to FILE (.png/.jpg/.exr) and exit
  --delay DURATION   Delay before screenshot (e.g. 3s, 500ms, 2). Default: immediate
  --norender        Display only hierarchy browser (no viewport)
  --profile         Enable performance profiling (writes trace.json on exit)
  --bboxStandin     Display unloaded prims with bounding boxes
  --clearsettings   Restore default settings
  --defaultsettings Same as --clearsettings
  --complexity N     Refinement complexity: low|medium|high|veryhigh or 1.0-1.3
  --ff FRAME        First frame
  --lf FRAME        Last frame
  --cf FRAME        Current frame
  --renderer NAME   Render backend (e.g. Storm, GL). Default: Storm
  --mem-profile     Log memory/cache stats every 60 frames
  --mute REGEX      Mute layers matching regex (repeatable)

Examples:
  usdview scene.usda
  usdview --select /World/cam --cf 24 bmw_x3.usda
  usdview --unloaded --mask /Geo /path/to/stage.usdc
  usdview --norender scene.usda
"#
    );
}
