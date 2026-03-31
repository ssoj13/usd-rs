//! Main viewer application.
//!
//! AppController analogue — orchestrates data model, dock, and panels.

mod actions;
mod file_ops;
mod panels;
pub(crate) mod sync;
mod toolbar;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use egui_dock::DockState;

use usd_imaging::gl::{Engine, EngineParameters};
use usd_sdf::TimeCode;

use crate::camera::FreeCamera;
use crate::data_model::{ChangeNotice, DataModel, RootDataModel};
use crate::dock::{DockTab, DockTabViewer};
use crate::event_bus::{downcast_event, EventBus};
use crate::events::{LoadPhase, LoadProgress, StageLoadFailed, StageLoaded};
use crate::file_watcher::FileWatcher;
use crate::menus::MenuState;
use crate::panels::{
    attr_editor::AttrEditorState, attributes_enhanced::AttributesPanelState,
    composition::CompositionPanelState, debug_flags::DebugFlagsState,
    layer_stack_enhanced::LayerStackPanelState, preferences::PreferencesState,
    prim_tree_enhanced::PrimTreeState, spline_viewer::SplineViewerState,
    validation::ValidationPanelState, viewport,
};
use crate::persistence::{self, AppPersistState, PreferencesSettingsJson};
use crate::playback::PlaybackState;
use crate::recent_files::RecentFiles;
use crate::status_bar::{draw_status_bar, StatusBarInfo};

/// Viewer configuration (CLI and initial state).
#[derive(Debug, Clone, Default)]
pub struct ViewerConfig {
    /// Initial file to load.
    pub initial_file: Option<PathBuf>,
    /// Verbosity.
    pub verbose: u8,
    /// Initial prim path to select (e.g. --select /World).
    pub initial_select: Option<String>,
    /// Camera prim path for initial view (e.g. --camera /cam).
    pub camera_prim: Option<String>,
    /// Comma-separated prim paths for population mask (e.g. --mask /A,/B).
    pub population_mask: Option<String>,
    /// Do not load payloads (--unloaded).
    pub unloaded: bool,
    /// Override first frame (--ff).
    pub frame_first: Option<f64>,
    /// Override last frame (--lf).
    pub frame_last: Option<f64>,
    /// Override current frame (--cf).
    pub frame_current: Option<f64>,
    /// Display only hierarchy browser, no viewport (--norender).
    pub no_render: bool,
    /// Display unloaded prims with bounding boxes (--bboxStandin).
    pub bbox_standin: bool,
    /// Restore default settings, skip loading persisted (--clearsettings/--defaultsettings).
    pub clear_settings: bool,
    /// Initial mesh refinement complexity (--complexity).
    pub complexity: Option<f64>,
    /// Log file path (-l/--log). If flag given without value, uses "usdview.log".
    pub log_file: Option<String>,
    /// Screenshot output path (-s/--screenshot). Saves first frame as PNG/JPEG/EXR and exits.
    pub screenshot: Option<PathBuf>,
    /// Delay before screenshot capture (--delay). Allows camera/scene to settle.
    pub screenshot_delay: Option<std::time::Duration>,
    /// Enable performance profiling (--profile). Writes trace.json on exit.
    pub profile: bool,
    /// Renderer plugin name (--renderer). Stored for future multi-renderer support.
    /// Currently only "Storm" is supported.
    pub renderer: Option<String>,
    /// Enable memory profiling logs (--mem-profile).
    pub mem_profile: bool,
    /// Regex patterns for muting layers on stage open (--mute PATTERN).
    /// Ref: appController.py:1171 _applyStageOpenLayerMutes.
    pub mute_layers_re: Vec<String>,
    /// Start playback immediately after loading (--play).
    pub autoplay: bool,
}

/// Level of GPU invalidation when scene content changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InvalidateLevel {
    /// Soft resync: keep device + GPU resources (reload, visibility change)
    Reload,
    /// Scene switch: keep device, clear GPU resource caches (new file load)
    SceneSwitch,
    /// Full device death: destroy device + all resources (clear, shutdown)
    DeviceDeath,
}

/// Main viewer application.
pub struct ViewerApp {
    /// Data model.
    pub data_model: DataModel,
    /// Free camera for viewport (orbit/pan/zoom).
    pub camera: FreeCamera,
    /// UsdImagingGL Engine for viewport rendering (opengl feature).
    pub(crate) engine: Engine,
    /// Persistent viewport state (cached texture, staging buffer).
    pub(crate) viewport_state: viewport::ViewportState,
    /// Dock state.
    pub(crate) dock_state: DockState<DockTab>,
    /// Saved dock state for viewer mode toggle restore.
    pub(crate) saved_dock_state: Option<DockState<DockTab>>,
    /// Config.
    pub config: ViewerConfig,
    /// Last window title (to avoid redundant updates).
    pub(crate) last_window_title: Option<String>,
    /// Last error message (e.g. from failed load) — shown in UI.
    pub last_error: Option<String>,
    /// Frame counter for periodic persistence save.
    pub(crate) frame_count: u64,
    /// Playback state (play/pause/loop/FPS).
    pub(crate) playback: PlaybackState,
    /// Menu bar state (render mode, toggles, HUD settings).
    pub(crate) menu_state: MenuState,
    /// Scene camera list cache dirty bit.
    pub(crate) scene_cameras_dirty: bool,
    /// Recent files list (populated by launcher from persisted state).
    pub recent_files: RecentFiles,
    /// Enhanced prim tree panel state.
    pub(crate) prim_tree_state: PrimTreeState,
    /// Enhanced attributes panel state.
    pub(crate) attrs_state: AttributesPanelState,
    /// Composition panel state.
    pub(crate) composition_state: CompositionPanelState,
    /// Enhanced layer stack panel state.
    pub(crate) layer_stack_state: LayerStackPanelState,
    /// Status bar runtime info (FPS, render time).
    pub(crate) status_info: StatusBarInfo,
    /// FPS tracking: timestamp of previous frame.
    pub(crate) last_frame_instant: std::time::Instant,
    /// Preferences dialog state (settings populated by launcher from persisted state).
    pub prefs_state: PreferencesState,
    /// Last successfully loaded file path (persisted as last_file).
    pub(crate) last_file: Option<PathBuf>,
    /// Pending auto-frame after first render (bbox computed during sync).
    pub(crate) auto_frame_pending: bool,
    /// Screenshot already saved (prevents re-saving every frame).
    pub(crate) screenshot_done: bool,
    /// Timestamp when first frame was rendered (for --delay countdown).
    pub(crate) screenshot_start: Option<std::time::Instant>,
    /// Last serialized persist snapshot to skip redundant disk writes.
    pub(crate) last_persist_snapshot: Option<String>,
    /// Adjust Free Camera dialog open state.
    pub(crate) free_camera_dialog_open: bool,
    /// Adjust Default Material dialog open state.
    pub(crate) default_material_dialog_open: bool,

    /// Free camera near clipping override.
    pub(crate) free_camera_override_near: Option<f64>,
    /// Free camera far clipping override.
    pub(crate) free_camera_override_far: Option<f64>,
    /// Last applied font size (to avoid re-setting style every frame).
    pub(crate) last_font_size: u32,
    /// TF_DEBUG flags dialog state.
    pub(crate) debug_flags_state: DebugFlagsState,
    /// File watcher for auto-reload on disk changes.
    pub(crate) file_watcher: Option<FileWatcher>,
    /// Startup instant for measuring time-to-first-frame.
    startup_instant: Option<std::time::Instant>,
    /// Enable memory profiling logs (--mem-profile).
    pub(crate) mem_profile: bool,
    /// Spline/animation curve viewer state.
    pub(crate) spline_state: SplineViewerState,
    /// Attribute value editor dialog state.
    pub(crate) attr_editor_state: AttrEditorState,
    /// Camera snapshot saved before the last Frame Selected / Frame All operation.
    /// Used by Toggle Framed View to swap back to the pre-frame viewpoint.
    pub(crate) pre_frame_camera: Option<FreeCamera>,
    /// USD Validation panel state.
    pub(crate) validation_state: ValidationPanelState,
    /// Named dock layouts: name -> RON-serialized DockState.
    pub(crate) layouts: HashMap<String, String>,
    /// Currently selected layout name.
    pub(crate) current_layout: Option<String>,
    /// Text input buffer for new layout name.
    pub(crate) layout_name_input: String,
    /// Delete button armed state (first click arms, second deletes).
    pub(crate) layout_delete_armed: bool,

    // --- EventBus + background loading ---
    /// Event bus for decoupled component communication.
    pub(crate) event_bus: EventBus,
    /// Loading state shown as viewport overlay during background stage load.
    pub(crate) loading_state: Option<LoadProgress>,
    /// Monotonic counter to ignore stale load results from superseded loads.
    pub(crate) load_generation: u64,
    /// Join handle for background loading thread.
    pub(crate) loading_handle: Option<std::thread::JoinHandle<()>>,
    /// Debug-only one-shot `StepForward` fired after the first presented frame.
    /// Enabled via `USDVIEW_DEBUG_STEP_AFTER_FIRST_PRESENT=1`.
    pub(crate) debug_step_after_first_present: bool,
    /// Tracks whether the debug post-present step has already been injected.
    pub(crate) debug_step_after_first_present_done: bool,
}

impl ViewerApp {
    /// Creates a new viewer with the given config and optional persisted dock layout.
    pub fn new(config: ViewerConfig, dock_state: DockState<DockTab>) -> Self {
        let engine = Engine::new(
            EngineParameters::default()
                .with_gpu_enabled(true)
                .with_display_unloaded_prims_with_bounds(config.bbox_standin),
        );
        let mem_profile = config.mem_profile;
        Self {
            data_model: DataModel::new(),
            camera: FreeCamera::new(),
            engine,
            viewport_state: viewport::ViewportState::new(),
            dock_state,
            saved_dock_state: None,
            config,
            last_window_title: None,
            last_error: None,
            frame_count: 0,
            playback: PlaybackState::new(),
            menu_state: MenuState::default(),
            scene_cameras_dirty: true,
            recent_files: RecentFiles::new(),
            prim_tree_state: PrimTreeState::new(),
            attrs_state: AttributesPanelState::default(),
            composition_state: CompositionPanelState::default(),
            layer_stack_state: LayerStackPanelState::default(),
            status_info: StatusBarInfo::default(),
            last_frame_instant: std::time::Instant::now(),
            prefs_state: PreferencesState::new(),
            last_file: None,
            auto_frame_pending: false,
            screenshot_done: false,
            screenshot_start: None,
            last_persist_snapshot: None,
            free_camera_dialog_open: false,
            default_material_dialog_open: false,
            free_camera_override_near: None,
            free_camera_override_far: None,
            last_font_size: 11,
            debug_flags_state: DebugFlagsState::new(),
            file_watcher: None,
            startup_instant: Some(std::time::Instant::now()),
            mem_profile,
            spline_state: SplineViewerState::new(),
            attr_editor_state: AttrEditorState::new(),
            pre_frame_camera: None,
            validation_state: ValidationPanelState::new(),
            layouts: HashMap::new(),
            current_layout: None,
            layout_name_input: String::new(),
            layout_delete_armed: false,
            event_bus: EventBus::new(),
            loading_state: None,
            load_generation: 0,
            loading_handle: None,
            debug_step_after_first_present: std::env::var("USDVIEW_DEBUG_STEP_AFTER_FIRST_PRESENT")
                .map(|v| v != "0" && !v.is_empty())
                .unwrap_or(false),
            debug_step_after_first_present_done: false,
        }
    }

    /// Injects a one-shot frame step after the first presented frame.
    ///
    /// This is a deterministic replacement for OS-level key injection when
    /// reproducing post-load hangs in release builds on heavy files.
    fn maybe_debug_step_after_first_present(&mut self, ctx: &egui::Context) {
        if !self.debug_step_after_first_present || self.debug_step_after_first_present_done {
            return;
        }
        if !self.viewport_state.has_presented_frame {
            return;
        }
        self.debug_step_after_first_present_done = true;
        log::info!(
            "[diag] injecting StepForward after first presented frame at t={}",
            self.data_model.root.current_time.value()
        );
        self.dispatch_action(&crate::keyboard::AppAction::StepForward, ctx);
        ctx.request_repaint();
    }

    #[cfg(feature = "wgpu")]
    pub fn configure_wgpu_render_state(&mut self, render_state: egui_wgpu::RenderState) {
        tracing::info!("egui wgpu target format: {:?}", render_state.target_format);
        self.status_info.display_format = Some(format!("{:?}", render_state.target_format));
        self.status_info.hdr_present = matches!(
            render_state.target_format,
            egui_wgpu::wgpu::TextureFormat::Rgba16Float
                | egui_wgpu::wgpu::TextureFormat::Rgb10a2Unorm
        );
        self.engine.set_shared_wgpu_context(
            render_state.adapter.clone(),
            render_state.device.clone(),
            render_state.queue.clone(),
        );
        self.viewport_state
            .configure_wgpu_render_state(render_state);
    }

    /// Unified scene invalidation — called whenever scene content changes.
    ///
    /// Three levels per C++ reference:
    /// * `SceneSwitch` — new file: clear GPU resource caches, keep device alive
    /// * `Reload`      — F5/file watcher: soft resync, keep device + GPU resources
    /// * `DeviceDeath` — clear(): destroy wgpu device + all GPU resources
    fn invalidate_scene(&mut self, level: InvalidateLevel) {
        self.data_model.clear_caches();
        self.scene_cameras_dirty = true;
        self.viewport_state.hud.invalidate_prim_stats();
        match level {
            InvalidateLevel::DeviceDeath => self.engine.invalidate_device(),
            InvalidateLevel::SceneSwitch => self.engine.invalidate_scene(),
            InvalidateLevel::Reload => self.engine.invalidate(),
        }
        self.prim_tree_state.invalidate();
        self.attrs_state.invalidate_cache();
    }

    fn sync_playback_to_root_time(&mut self) {
        // Match usdview's non-tracking scrub behavior: while the slider is held down,
        // UI frame state may diverge from the stage's current time until release.
        if self.playback.is_scrubbing() {
            return;
        }
        let frame = self
            .playback
            .set_frame(self.data_model.root.current_time.value());
        if self.data_model.root.current_time.value() != frame {
            self.data_model.root.current_time = TimeCode::new(frame);
        }
    }

    fn rebuild_timeline_from_view_settings(&mut self) {
        self.data_model
            .root
            .rebuild_stage_time_samples(self.data_model.view.step_size);
        self.data_model.root.clamp_current_time_to_samples();
        self.sync_playback_to_root_time();
    }

    fn handle_stage_notices(&mut self) {
        usd_trace::trace_scope!("viewer_handle_stage_notices");
        let (prim_change, prop_change) = self.data_model.root.drain_changes();
        if prim_change == ChangeNotice::None && prop_change == ChangeNotice::None {
            return;
        }

        self.rebuild_timeline_from_view_settings();
        self.invalidate_scene(InvalidateLevel::Reload);
    }

    /// Loads a stage from file in a background thread.
    ///
    /// Spawns a thread that opens the stage and collects time samples,
    /// then emits StageLoaded/StageLoadFailed via EventBus. UI stays
    /// responsive and shows a progress overlay during loading.
    pub fn load_file(&mut self, path: &Path) {
        usd_trace::trace_scope!("viewer_load_file");
        tracing::info!("[load] loading file: {}", path.display());
        self.last_error = None;
        let bbox_standin = self.config.bbox_standin;
        self.data_model.view.show_bboxes = bbox_standin;

        self.load_generation += 1;
        let generation = self.load_generation;

        self.loading_state = Some(LoadProgress {
            phase: LoadPhase::Opening,
            progress: 0.0,
            message: format!(
                "Opening {}",
                path.file_name().unwrap_or_default().to_string_lossy()
            ),
            generation,
        });

        let bus = self.event_bus.clone();
        let path_owned = path.to_path_buf();
        let unloaded = self.config.unloaded;
        let mask_paths = self.config.population_mask.as_ref().map(|s| {
            s.split([',', ' '])
                .filter(|p| !p.is_empty())
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
        });

        self.loading_handle = Some(std::thread::spawn(move || {
            bus.emit(LoadProgress {
                phase: LoadPhase::Opening,
                progress: 0.1,
                message: "Opening stage...".to_string(),
                generation,
            });

            let mask_slice = mask_paths.as_deref();
            match RootDataModel::open_stage_detached(&path_owned, unloaded, mask_slice) {
                Ok(stage) => {
                    bus.emit(LoadProgress {
                        phase: LoadPhase::TimeSamples,
                        progress: 0.6,
                        message: "Collecting time samples...".to_string(),
                        generation,
                    });

                    let time_samples = RootDataModel::collect_stage_time_samples(&stage);

                    bus.emit(LoadProgress {
                        phase: LoadPhase::Ready,
                        progress: 1.0,
                        message: "Ready".to_string(),
                        generation,
                    });

                    bus.emit(StageLoaded {
                        stage,
                        path: path_owned,
                        time_samples,
                        generation,
                    });
                }
                Err(error) => {
                    bus.emit(StageLoadFailed {
                        path: path_owned,
                        error,
                        generation,
                    });
                }
            }
        }));
    }

    /// Polls EventBus and handles stage load events.
    fn handle_bus_events(&mut self) {
        let events = self.event_bus.poll();
        for event in &events {
            if let Some(loaded) = downcast_event::<StageLoaded>(event) {
                if loaded.generation == self.load_generation {
                    self.on_stage_loaded(
                        loaded.stage.clone(),
                        loaded.path.clone(),
                        loaded.time_samples.clone(),
                    );
                }
            } else if let Some(failed) = downcast_event::<StageLoadFailed>(event) {
                if failed.generation == self.load_generation {
                    tracing::error!("{}", failed.error);
                    self.last_error = Some(failed.error.clone());
                    self.loading_state = None;
                    self.loading_handle = None;
                }
            } else if let Some(progress) = downcast_event::<LoadProgress>(event) {
                if progress.generation == self.load_generation {
                    self.loading_state = Some(progress.clone());
                }
            }
        }
    }

    /// Called when background stage load completes successfully.
    fn on_stage_loaded(
        &mut self,
        stage: std::sync::Arc<usd_core::Stage>,
        path: PathBuf,
        time_samples: Vec<f64>,
    ) {
        tracing::info!("[load] stage opened OK (background)");
        self.data_model
            .root
            .apply_loaded_stage(stage, time_samples, path.clone());
        self.data_model.selection.clear();
        self.invalidate_scene(InvalidateLevel::SceneSwitch);
        self.recent_files.add(path.clone());
        self.last_file = Some(path.clone());
        self.file_watcher = FileWatcher::new(&[path]);
        self.apply_post_load_config();
        self.loading_state = None;
        self.loading_handle = None;
    }

    /// Navigate to next/prev USD file in the same directory.
    pub(crate) fn navigate_sibling(&mut self, direction: i32) {
        if let Some(current) = self.last_file.clone() {
            if let Some(path) = find_sibling_usd(&current, direction) {
                self.load_file(&path);
            }
        }
    }

    /// Applies config overrides after stage load (select, camera, frame, mute).
    fn apply_post_load_config(&mut self) {
        // Layer muting (--mute regex) — ref: appController.py:1171-1199
        if !self.config.mute_layers_re.is_empty() {
            if let Some(ref stage) = self.data_model.root.stage {
                let combined = self.config.mute_layers_re.join("|");
                match regex::Regex::new(&combined) {
                    Ok(re) => {
                        let layers_to_mute: Vec<String> = stage
                            .get_used_layers(true)
                            .iter()
                            .filter(|layer| re.is_match(layer.identifier()))
                            .map(|layer| layer.identifier().to_string())
                            .collect();
                        if !layers_to_mute.is_empty() {
                            tracing::info!(
                                "Muting {} layers matching '{}'",
                                layers_to_mute.len(),
                                combined
                            );
                            stage.mute_and_unmute_layers(&layers_to_mute, &[]);
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Invalid --mute regex '{}': {}", combined, e);
                    }
                }
            }
        }

        // Initial selection and frame (--select flag)
        if let Some(ref sel) = self.config.initial_select {
            if let Some(path) = usd_sdf::Path::from_string(sel) {
                if self.data_model.root.prim_at_path(&path).is_some() {
                    self.data_model.selection.set_paths(vec![path.clone()]);
                    self.frame_prim_in_viewport(&path);
                } else {
                    tracing::warn!("--select: prim not found at path '{}'", sel);
                }
            } else {
                tracing::warn!("--select: invalid SdfPath '{}'", sel);
            }
        }

        // Frame overrides (--ff, --lf, --cf)
        // Per C++ appController.py:1322-1365:
        // - C++ always reads stageStartTimeCode/stageEndTimeCode (unconditionally)
        // - When CLI --ff/--lf given, the other bound comes from stage metadata
        // - When neither CLI given, HasAuthoredTimeCodeRange() gates whether
        //   playback is enabled at all (None = disabled)
        let stage_ref = self.data_model.root.stage.as_ref();
        let has_cli = self.config.frame_first.is_some() || self.config.frame_last.is_some();
        let (base_start, base_end) = if has_cli {
            // CLI given — always read stage times as fallback (C++ lines 1332-1333)
            (
                stage_ref.map(|s| s.get_start_time_code()),
                stage_ref.map(|s| s.get_end_time_code()),
            )
        } else {
            // No CLI — only use stage range if authored
            let has_range = stage_ref.map_or(false, |s| s.has_authored_time_code_range());
            if has_range {
                (
                    stage_ref.map(|s| s.get_start_time_code()),
                    stage_ref.map(|s| s.get_end_time_code()),
                )
            } else {
                (None, None)
            }
        };
        let start = self.config.frame_first.or(base_start);
        let end = self.config.frame_last.or(base_end);
        if let (Some(s), Some(e)) = (start, end) {
            if has_cli {
                self.data_model.root.frame_range_override = Some((s, e));
            }
        }
        if let Some(cf) = self.config.frame_current {
            // Per C++ appController.py:1370-1377: validate --cf against range.
            let rs = self.data_model.root.frame_range_start();
            let re = self.data_model.root.frame_range_end();
            if cf < rs || cf > re {
                tracing::warn!("--cf {cf} outside range [{rs}..{re}], ignoring");
            } else {
                self.data_model.root.current_time = TimeCode::new(cf);
            }
        }

        // Per C++ appController.py:1339-1348: compute FPS and step from stage.
        if let Some(ref stage) = self.data_model.root.stage {
            let fps = stage.get_frames_per_second();
            let fps = if fps < 1.0 { 24.0 } else { fps };
            self.playback.set_fps(fps);
            let tcps = stage.get_time_codes_per_second();
            let step = tcps / fps;
            self.playback.set_step_size(step);
            self.data_model.view.step_size = step;
        }

        self.rebuild_timeline_from_view_settings();

        // camera_prim stored in config; viewport will use it when camera is wired

        // Set Z-up flag on free camera from stage upAxis metadata.
        if let Some(ref stage) = self.data_model.root.stage {
            let up = usd_geom::metrics::get_stage_up_axis(stage);
            self.camera.set_is_z_up(up == "Z");
        }

        // Schedule auto-frame after first render (bbox computed during mesh sync).
        if self.config.initial_select.is_none() {
            self.auto_frame_pending = true;
        }
    }

    /// Opens file dialog and loads selected file.
    pub fn open_file_dialog(&mut self) {
        let start = file_ops::dialog_start_dir(
            self.data_model.root.file_path.as_deref(),
        );
        if let Some(path) = rfd::FileDialog::new()
            .set_directory(&start)
            .add_filter("USD", &["usd", "usda", "usdc", "usdz"])
            .add_filter("All files", &["*"])
            .pick_file()
        {
            self.load_file(&path);
        }
    }

    /// Clears the stage and all associated state.
    pub fn clear(&mut self) {
        self.data_model.clear();
        self.invalidate_scene(InvalidateLevel::DeviceDeath);
        self.file_watcher = None;
    }

    pub(crate) fn update_window_title(&mut self, ctx: &egui::Context) {
        let title = if let Some(path) = &self.data_model.root.file_path {
            // Per Python appController.py:2841 — just the filename
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        } else {
            "New Stage".to_string()
        };

        if self.last_window_title.as_deref() != Some(title.as_str()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_window_title = Some(title);
        }
    }
}

impl eframe::App for ViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        usd_trace::trace_scope!("viewer_frame_update");

        // Measure frame-to-frame interval to find where time is lost
        {
            static LAST_UPDATE: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);
            let now = std::time::Instant::now();
            if let Ok(mut last) = LAST_UPDATE.lock() {
                if let Some(prev) = *last {
                    let gap = now.duration_since(prev);
                    if gap.as_millis() > 100 {
                        log::info!("[PERF] update_gap: {:.1}ms (time between update() calls)", gap.as_secs_f64() * 1000.0);
                    }
                }
                *last = Some(now);
            }
        }

        // Log time-to-first-frame on first update call
        if let Some(t0) = self.startup_instant.take() {
            log::trace!("[PERF] time_to_first_frame: {:?}", t0.elapsed());
        }

        self.apply_theme_preference(ctx);
        // Apply font size from view settings (only when changed)
        if self.data_model.view.font_size != self.last_font_size {
            self.last_font_size = self.data_model.view.font_size;
            let fs = self.last_font_size as f32;
            if fs > 0.0 {
                let mut style = (*ctx.style()).clone();
                style
                    .text_styles
                    .insert(egui::TextStyle::Body, egui::FontId::proportional(fs));
                style
                    .text_styles
                    .insert(egui::TextStyle::Button, egui::FontId::proportional(fs));
                style.text_styles.insert(
                    egui::TextStyle::Small,
                    egui::FontId::proportional(fs * 0.85),
                );
                ctx.set_style(style);
            }
        }
        self.update_window_title(ctx);
        self.frame_count = self.frame_count.saturating_add(1);

        // Poll file watcher for auto-reload
        if let Some(ref watcher) = self.file_watcher {
            if watcher.poll_changed() {
                log::info!("[watcher] file changed on disk, reloading");
                if let Some(stage) = self.data_model.root.stage.as_ref() {
                    if let Err(e) = stage.reload() {
                        self.last_error = Some(format!("Auto-reload failed: {e}"));
                    } else {
                        self.invalidate_scene(InvalidateLevel::Reload);
                    }
                }
            }
        }

        // Poll EventBus for background load results
        self.handle_bus_events();
        self.handle_stage_notices();
        // Request repaint while loading so progress updates are visible
        if self.loading_state.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }

        // Drag & drop
        ctx.input(|i| {
            if let Some(file) = i.raw.dropped_files.first() {
                if let Some(path) = &file.path {
                    if path.extension().map_or(false, |e| {
                        USD_EXTENSIONS
                            .iter()
                            .any(|x| x.eq_ignore_ascii_case(&e.to_string_lossy().as_ref()))
                    }) {
                        self.load_file(path);
                    }
                }
            }
        });

        if self.dispatch_actions(ctx) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        // Check scrub-resume timer (500ms after slider release)
        self.playback.check_scrub_resume();

        // Tick playback (advance frame if playing)
        self.data_model.root.playing = self.playback.is_playing();
        self.data_model.root.scrubbing = self.playback.is_scrubbing();
        if self.playback.is_playing() {
            if let Some(new_frame) = self.playback.advance() {
                log::debug!("[playback] advanced to frame={new_frame}");
                self.data_model.root.current_time = TimeCode::new(new_frame);
            }
            // Immediate repaint — FPS gating is in PlaybackState::advance().
            ctx.request_repaint();
        }

        // Update persistent caches when time changes (per C++ currentFrame setter).
        self.data_model.update_cache_time();

        // Sync playback frame range from stage
        if self.data_model.root.stage.is_some() {
            let start = self.data_model.root.frame_range_start();
            let end = self.data_model.root.frame_range_end();
            self.playback.set_frame_range(start, end);
            self.sync_playback_to_root_time();
        }

        // Error banner
        if let Some(err) = &self.last_error {
            let err = err.clone();
            let mut dismiss = false;
            egui::TopBottomPanel::top("error_banner")
                .min_height(0.0)
                .show(ctx, |ui| {
                    ui.colored_label(egui::Color32::from_rgb(200, 60, 60), "Error:");
                    ui.label(&err);
                    dismiss = ui.button("Dismiss").clicked();
                });
            if dismiss {
                self.last_error = None;
            }
        }

        let panels_t0 = std::time::Instant::now();

        // Status bar (bottom, declared before dock so it claims space first)
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            draw_status_bar(ui, &self.data_model, &self.playback, &self.status_info);
        });

        // Toolbar
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            self.draw_toolbar(ui);
        });
        self.draw_theme_toggle(ctx);

        // Preferences modal (shown on top of everything)
        crate::panels::preferences::ui_preferences(ctx, &mut self.prefs_state);

        // TF_DEBUG flags dialog
        crate::panels::debug_flags::ui_debug_flags(ctx, &mut self.debug_flags_state);

        // Free Camera / Default Material dialogs
        self.draw_dialogs(ctx);

        // Attribute value editor dialog
        crate::panels::attr_editor::ui_attr_editor(
            ctx,
            &mut self.attr_editor_state,
            self.data_model.root.stage.as_ref(),
        );

        // USD Validation panel
        let sel_paths = self.data_model.selection.get_paths().to_vec();
        if let Some(nav_path) = crate::panels::validation::ui_validation(
            ctx,
            &mut self.validation_state,
            self.data_model.root.stage.as_ref(),
            &sel_paths,
        ) {
            // Navigate to the prim the user double-clicked in the results table.
            if let Some(path) = usd_sdf::Path::from_string(&nav_path) {
                self.data_model.selection.switch_to_path(path);
            }
        }

        let pre_dock_ms = panels_t0.elapsed().as_secs_f64() * 1000.0;
        if pre_dock_ms > 10.0 {
            log::info!("[TRACE] pre-dock panels: {:.1}ms", pre_dock_ms);
        }

        // Dock — swap out with a minimal placeholder to satisfy the borrow checker;
        // the real state is put back immediately after show().
        let mut dock_state = std::mem::replace(
            &mut self.dock_state,
            DockState::new(vec![]), // cheap placeholder, never rendered
        );
        let dock_t0 = std::time::Instant::now();
        egui_dock::DockArea::new(&mut dock_state)
            .style(egui_dock::Style::from_egui(ctx.style().as_ref()))
            .show(ctx, &mut DockTabViewer { app: self });
        let dock_ms = dock_t0.elapsed().as_secs_f64() * 1000.0;
        if dock_ms > 50.0 {
            log::info!("[TRACE] dock_show: {:.1}ms", dock_ms);
        }
        self.dock_state = dock_state;
        self.maybe_debug_step_after_first_present(ctx);

        // Screenshot: save frame after optional delay and exit
        if let Some(ref screenshot_path) = self.config.screenshot.clone() {
            if !self.screenshot_done {
                // Start delay timer on first rendered frame
                if self.viewport_state.has_presented_frame && self.screenshot_start.is_none() {
                    self.screenshot_start = Some(std::time::Instant::now());
                    ctx.request_repaint(); // keep rendering during delay
                }

                // Check if delay elapsed (or no delay)
                let delay = self.config.screenshot_delay.unwrap_or_default();
                let elapsed = self
                    .screenshot_start
                    .map(|t| t.elapsed() >= delay)
                    .unwrap_or(false);

                if elapsed {
                    // Re-capture fresh pixels (after delay, camera may have moved)
                    let save_result = match crate::screenshot::detect_format(screenshot_path) {
                        Ok(crate::screenshot::ScreenshotFormat::Exr) => {
                            if let Some((pixels, w, h)) =
                                crate::panels::viewport::capture_current_frame_linear(
                                    &mut self.engine,
                                )
                            {
                                crate::screenshot::save_exr(&pixels, w, h, screenshot_path)
                                    .map(|()| (w, h))
                            } else {
                                Err("No rendered frame available yet".to_string())
                            }
                        }
                        Ok(
                            crate::screenshot::ScreenshotFormat::Png
                            | crate::screenshot::ScreenshotFormat::Jpeg,
                        ) => {
                            if let Some((pixels, w, h)) =
                                crate::panels::viewport::capture_current_frame(
                                    &mut self.engine,
                                    &mut self.viewport_state,
                                    &self.data_model,
                                )
                            {
                                crate::screenshot::save_ldr(&pixels, w, h, screenshot_path)
                                    .map(|()| (w, h))
                            } else {
                                Err("No rendered frame available yet".to_string())
                            }
                        }
                        Err(e) => Err(e),
                    };

                    match save_result {
                        Ok((w, h)) => {
                            eprintln!(
                                "Screenshot saved: {} ({}x{})",
                                screenshot_path.display(),
                                w,
                                h
                            );
                        }
                        Err(e) => {
                            eprintln!("Screenshot error: {e}");
                        }
                    }
                    self.screenshot_done = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    return;
                } else if self.screenshot_start.is_some() {
                    ctx.request_repaint();
                }
            }
        }

        // FPS tracking
        let now = std::time::Instant::now();
        let dt = now.duration_since(self.last_frame_instant).as_secs_f64();
        self.last_frame_instant = now;
        if dt > 0.0 {
            self.status_info.fps = 1.0 / dt;
            self.status_info.render_ms = dt * 1000.0;
        }

        // Persist all state to JSON every 60 frames
        if self.frame_count % 60 == 0 {
            // Read window rect from egui viewport
            let (win_pos, win_size) = ctx.input(|i| {
                let rect = i.viewport().inner_rect;
                match rect {
                    Some(r) => (Some([r.left(), r.top()]), Some([r.width(), r.height()])),
                    None => (None, None),
                }
            });
            let state = AppPersistState {
                version: persistence::PERSIST_VERSION,
                last_file: self.last_file.clone(),
                recent_files: self.recent_files.to_vec(),
                preferences: PreferencesSettingsJson::from(&self.prefs_state.settings),
                dock_layout: ron::to_string(&self.dock_state).ok(),
                view_settings: ron::to_string(&self.data_model.view).ok(),
                window_pos: win_pos,
                window_size: win_size,
                layouts: self.layouts.clone(),
                current_layout: self.current_layout.clone(),
            };
            if let Ok(snapshot) = serde_json::to_string(&state) {
                if self.last_persist_snapshot.as_deref() != Some(snapshot.as_str()) {
                    persistence::save_state(&state);
                    self.last_persist_snapshot = Some(snapshot);
                }
            }
        }

        // Memory profiling: log cache sizes every 60 frames
        if self.frame_count % 60 == 0 && self.mem_profile {
            let (draw_items, meshes, _) = self.engine.render_stats();
            log::info!(
                "[MEM] frame={} draw_items={} meshes={}",
                self.frame_count,
                draw_items,
                meshes,
            );
        }

        // Diagnostic: log when update() finishes
        if self.playback.is_playing() || self.data_model.root.playing {
            log::info!("[TRACE] update() end frame={} fc={}", self.data_model.root.current_time, self.frame_count);
        }
    }
}

// --- File navigation helpers ---

const USD_EXTENSIONS: &[&str] = &["usd", "usda", "usdc", "usdz"];

/// Find next/prev USD file in the same directory (wrapping).
fn find_sibling_usd(current: &std::path::Path, direction: i32) -> Option<PathBuf> {
    let dir = current.parent()?;
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .map(|ext| {
                    let s = ext.to_string_lossy();
                    USD_EXTENSIONS.iter().any(|e| e.eq_ignore_ascii_case(&s))
                })
                .unwrap_or(false)
        })
        .collect();
    files.sort();
    if files.is_empty() {
        return None;
    }
    let idx = files.iter().position(|p| p == current)?;
    let new_idx = if direction > 0 {
        (idx + 1) % files.len()
    } else if idx == 0 {
        files.len() - 1
    } else {
        idx - 1
    };
    if new_idx == idx {
        None
    } else {
        Some(files[new_idx].clone())
    }
}
