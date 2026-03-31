//! Data model for USD viewer.
//!
//! Split into three sub-models matching C++ usdviewq architecture:
//! - RootDataModel: stage, time, frame range
//! - ViewSettingsDataModel: all display/render settings (persisted via serde)
//! - SelectionDataModel: prim/prop selection state

use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path as FsPath, PathBuf};
use std::sync::{Arc, Mutex};
use usd_core::{Prim, Stage};
use usd_geom::bbox_cache::BBoxCache;
use usd_geom::xform_cache::XformCache;
use usd_sdf::{Path, TimeCode};
use usd_tf::notice::ListenerKey;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Reference-style `Drange` from usdviewq/common.py.
fn drange(start: f64, stop: f64, step: f64) -> Vec<f64> {
    if !start.is_finite() || !stop.is_finite() || !step.is_finite() || step <= 0.0 {
        return Vec::new();
    }
    if start > stop {
        return Vec::new();
    }

    let mut values = vec![start];
    let mut n = 1u64;
    while start + (n as f64) * step <= stop {
        values.push(start + (n as f64) * step);
        n += 1;
    }
    values
}

/// Snaps `value` to the nearest time sample in `samples` using binary search.
/// Returns `value` unchanged if `samples` is empty.
pub fn snap_to_nearest(value: f64, samples: &[f64]) -> f64 {
    if samples.is_empty() {
        return value;
    }
    let idx = samples.partition_point(|&s| s < value);
    if idx == 0 {
        return samples[0];
    }
    if idx >= samples.len() {
        return samples[samples.len() - 1];
    }
    // Compare distance to left and right neighbors
    let left = samples[idx - 1];
    let right = samples[idx];
    if (value - left).abs() <= (right - value).abs() {
        left
    } else {
        right
    }
}

// ---------------------------------------------------------------------------
// Refinement Complexities (matches Python UsdAppUtils.complexityArgs)
// ---------------------------------------------------------------------------

/// Named mesh refinement complexity levels.
/// Mirrors Python `RefinementComplexities` from `pxr.UsdAppUtils.complexityArgs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RefinementComplexity {
    Low,
    Medium,
    High,
    VeryHigh,
}

impl RefinementComplexity {
    /// Numeric complexity value sent to Hydra.
    pub const fn value(self) -> f64 {
        match self {
            Self::Low => 1.0,
            Self::Medium => 1.1,
            Self::High => 1.2,
            Self::VeryHigh => 1.3,
        }
    }

    /// Display name shown in menus and HUD.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
            Self::VeryHigh => "Very High",
        }
    }

    /// CLI identifier (matches Python `fromId`).
    pub const fn id(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::VeryHigh => "veryhigh",
        }
    }

    /// All levels in ascending order.
    pub const ORDERED: &[RefinementComplexity] =
        &[Self::Low, Self::Medium, Self::High, Self::VeryHigh];

    /// Minimum complexity value (Low).
    pub const MIN: f64 = 1.0;
    /// Maximum complexity value (Very High).
    pub const MAX: f64 = 1.3;

    /// Look up by numeric value (nearest match within epsilon).
    pub fn from_value(v: f64) -> Self {
        Self::ORDERED
            .iter()
            .copied()
            .min_by(|a, b| {
                (a.value() - v)
                    .abs()
                    .partial_cmp(&(b.value() - v).abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap_or(Self::Low)
    }

    /// Look up by CLI identifier string ("low", "medium", "high", "veryhigh").
    pub fn from_id(s: &str) -> Option<Self> {
        Self::ORDERED.iter().copied().find(|c| c.id() == s)
    }

    /// Next higher level (saturates at VeryHigh).
    pub fn next(self) -> Self {
        match self {
            Self::Low => Self::Medium,
            Self::Medium => Self::High,
            Self::High => Self::VeryHigh,
            Self::VeryHigh => Self::VeryHigh,
        }
    }

    /// Previous lower level (saturates at Low).
    pub fn prev(self) -> Self {
        match self {
            Self::Low => Self::Low,
            Self::Medium => Self::Low,
            Self::High => Self::Medium,
            Self::VeryHigh => Self::High,
        }
    }
}

impl std::fmt::Display for RefinementComplexity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// Render/draw mode for viewport (matches UsdImagingGL + usdviewq extras).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DrawMode {
    Wireframe,
    /// Wireframe overlaid on shaded surface.
    WireframeOnSurface,
    #[default]
    ShadedSmooth,
    ShadedFlat,
    /// Geometry only (no lighting).
    GeometryOnly,
    Points,
    /// Smooth geometry (subdivision surface, no lighting).
    GeomSmooth,
    /// Flat geometry (subdivision surface, no lighting).
    GeomFlat,
    /// Hidden-surface wireframe.
    HiddenSurfaceWireframe,
    /// Bounding boxes.
    Bounds,
}

impl DrawMode {
    /// Display name for UI.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Wireframe => "Wireframe",
            Self::WireframeOnSurface => "WireframeOnSurface",
            Self::ShadedSmooth => "Smooth Shaded",
            Self::ShadedFlat => "Flat Shaded",
            Self::GeometryOnly => "Geom Only",
            Self::Points => "Points",
            Self::GeomSmooth => "Geom Smooth",
            Self::GeomFlat => "Geom Flat",
            Self::HiddenSurfaceWireframe => "Hidden Surface Wireframe",
            Self::Bounds => "Bounds",
        }
    }

    /// All modes for iteration in UI.
    pub const ALL: &'static [DrawMode] = &[
        Self::Wireframe,
        Self::WireframeOnSurface,
        Self::ShadedSmooth,
        Self::ShadedFlat,
        Self::GeometryOnly,
        Self::Points,
        Self::GeomSmooth,
        Self::GeomFlat,
        Self::HiddenSurfaceWireframe,
        Self::Bounds,
    ];
}

/// Color correction mode (reference: ColorCorrectionModes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ColorCorrectionMode {
    Disabled,
    #[default]
    SRGB,
    OpenColorIO,
}

impl ColorCorrectionMode {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Disabled => "Disabled",
            Self::SRGB => "sRGB",
            Self::OpenColorIO => "OpenColorIO",
        }
    }
}

/// Pick mode (reference: PickModes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PickMode {
    #[default]
    Prims,
    Models,
    Instances,
    Prototypes,
}

impl PickMode {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Prims => "Prims",
            Self::Models => "Models",
            Self::Instances => "Instances",
            Self::Prototypes => "Prototypes",
        }
    }
}

/// Clear/background color for viewport (reference: ClearColors).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ClearColor {
    Black,
    #[default]
    DarkGrey,
    LightGrey,
    White,
}

impl ClearColor {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Black => "Black",
            Self::DarkGrey => "Dark Grey",
            Self::LightGrey => "Light Grey",
            Self::White => "White",
        }
    }

    pub fn to_color32(&self) -> egui::Color32 {
        match self {
            Self::Black => egui::Color32::from_rgb(0, 0, 0),
            Self::DarkGrey => egui::Color32::from_rgb(18, 18, 18),
            Self::LightGrey => egui::Color32::from_rgb(116, 116, 116),
            Self::White => egui::Color32::from_rgb(255, 255, 255),
        }
    }

    /// Returns clear color as Vec4f (RGBA, 0-1) for UsdImagingGL RenderParams.
    pub fn to_vec4f(&self) -> usd_gf::Vec4f {
        match self {
            Self::Black => usd_gf::Vec4f::new(0.0, 0.0, 0.0, 1.0),
            // C++ linear values from viewSettingsDataModel.py _CLEAR_COLORS_DICT
            Self::DarkGrey => usd_gf::Vec4f::new(0.07074, 0.07074, 0.07074, 1.0),
            Self::LightGrey => usd_gf::Vec4f::new(0.45626, 0.45626, 0.45626, 1.0),
            Self::White => usd_gf::Vec4f::new(1.0, 1.0, 1.0, 1.0),
        }
    }
}

/// Camera mask mode (reference: CameraMaskModes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CameraMaskMode {
    #[default]
    None,
    Partial,
    Full,
}

impl CameraMaskMode {
    pub fn name(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Partial => "Partial",
            Self::Full => "Full",
        }
    }
}

/// Selection highlight mode (reference: SelectionHighlightModes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SelectionHighlightMode {
    Never,
    #[default]
    OnlyWhenPaused,
    Always,
}

impl SelectionHighlightMode {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Never => "Never",
            Self::OnlyWhenPaused => "Only when paused",
            Self::Always => "Always",
        }
    }
}

/// Highlight color for selection (reference: HighlightColors).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum HighlightColor {
    White,
    #[default]
    Yellow,
    Cyan,
}

impl HighlightColor {
    pub fn name(&self) -> &'static str {
        match self {
            Self::White => "White",
            Self::Yellow => "Yellow",
            Self::Cyan => "Cyan",
        }
    }

    /// Returns highlight color as Vec4f (RGBA, 0-1) for UsdImagingGL selection.
    pub fn to_vec4f(&self) -> usd_gf::Vec4f {
        match self {
            Self::White => usd_gf::Vec4f::new(1.0, 1.0, 1.0, 0.5),
            Self::Yellow => usd_gf::Vec4f::new(1.0, 1.0, 0.0, 0.5),
            Self::Cyan => usd_gf::Vec4f::new(0.0, 1.0, 1.0, 0.5),
        }
    }
}

// ---------------------------------------------------------------------------
// RootDataModel — stage and playback state
// ---------------------------------------------------------------------------

/// Change notification classification (matches C++ rootDataModel.py ChangeNotice).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ChangeNotice {
    /// No change detected.
    None,
    /// Non-structural change (metadata, attribute values).
    InfoChanges,
    /// Structural change (prim added/removed/resynced).
    Resync,
}

/// Stage, time, and playback state (reference: RootDataModel).
/// Includes ObjectsChanged notice listener per C++ rootDataModel.py:76-102.
pub struct RootDataModel {
    /// Loaded stage (None if no file open).
    pub stage: Option<Arc<Stage>>,
    /// Path to the currently loaded file.
    pub file_path: Option<PathBuf>,
    /// Current time code for attribute evaluation.
    pub current_time: TimeCode,
    /// Whether playback is active.
    pub playing: bool,
    /// Whether the user is actively scrubbing the frame slider.
    pub scrubbing: bool,
    /// Computed frame range (start, end) from stage metadata.
    pub frame_range: (f64, f64),
    /// Optional frame range override from CLI (--ff, --lf).
    pub frame_range_override: Option<(f64, f64)>,
    /// User-editable playback sub-range begin (overrides stage startTimeCode for slider).
    pub range_begin: Option<f64>,
    /// User-editable playback sub-range end (overrides stage endTimeCode for slider).
    pub range_end: Option<f64>,
    /// ObjectsChanged listener key (revoked on stage close).
    notice_listener: Option<ListenerKey>,
    /// Shared dirty state set by notice callback (prim_change, prop_change).
    /// Uses Mutex since the callback runs on an arbitrary thread.
    pub change_state: Arc<Mutex<(ChangeNotice, ChangeNotice)>>,
    /// Union of all attribute time samples across the stage (sorted, unique).
    /// Used for slider snapping to nearest authored time sample.
    pub stage_time_samples: Vec<f64>,
}

impl Default for RootDataModel {
    fn default() -> Self {
        Self {
            stage: None,
            file_path: None,
            current_time: TimeCode::new(0.0),
            playing: false,
            scrubbing: false,
            frame_range: (0.0, 0.0),
            frame_range_override: None,
            range_begin: None,
            range_end: None,
            notice_listener: None,
            change_state: Arc::new(Mutex::new((ChangeNotice::None, ChangeNotice::None))),
            stage_time_samples: Vec::new(),
        }
    }
}

impl std::fmt::Debug for RootDataModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RootDataModel")
            .field("stage", &self.stage.is_some())
            .field("file_path", &self.file_path)
            .field("current_time", &self.current_time)
            .field("playing", &self.playing)
            .field("scrubbing", &self.scrubbing)
            .field("frame_range", &self.frame_range)
            .finish()
    }
}

impl Drop for RootDataModel {
    fn drop(&mut self) {
        // Revoke ObjectsChanged listener on destruction
        if let Some(key) = self.notice_listener.take() {
            usd_tf::notice::revoke(key);
        }
    }
}

impl RootDataModel {
    /// Frame range start for the playback slider.
    /// Priority: user range_begin > CLI override > stage startTimeCode > default.
    pub fn frame_range_start(&self) -> f64 {
        self.range_begin
            .or_else(|| self.frame_range_override.map(|(s, _)| s))
            .or_else(|| {
                self.stage
                    .as_ref()
                    .filter(|s| s.has_authored_time_code_range())
                    .map(|s| s.get_start_time_code())
            })
            .unwrap_or(0.0)
    }

    /// Frame range end for the playback slider.
    /// Priority: user range_end > CLI override > stage endTimeCode > default.
    pub fn frame_range_end(&self) -> f64 {
        self.range_end
            .or_else(|| self.frame_range_override.map(|(_, e)| e))
            .or_else(|| {
                self.stage
                    .as_ref()
                    .filter(|s| s.has_authored_time_code_range())
                    .map(|s| s.get_end_time_code())
            })
            .unwrap_or(0.0)
    }

    /// Full stage time range (ignores user range_begin/range_end overrides).
    pub fn stage_start(&self) -> f64 {
        self.frame_range_override
            .map(|(s, _)| s)
            .or_else(|| {
                self.stage
                    .as_ref()
                    .filter(|s| s.has_authored_time_code_range())
                    .map(|s| s.get_start_time_code())
            })
            .unwrap_or(0.0)
    }

    /// Full stage time range end (ignores user range_begin/range_end overrides).
    /// Returns 0.0 for static scenes (no authored range, no CLI override).
    pub fn stage_end(&self) -> f64 {
        self.frame_range_override
            .map(|(_, e)| e)
            .or_else(|| {
                self.stage
                    .as_ref()
                    .filter(|s| s.has_authored_time_code_range())
                    .map(|s| s.get_end_time_code())
            })
            .unwrap_or(0.0)
    }

    /// Whether the stage has an authored time range or CLI override.
    pub fn has_frame_range(&self) -> bool {
        self.frame_range_override.is_some()
            || self
                .stage
                .as_ref()
                .map(|s| s.has_authored_time_code_range())
                .unwrap_or(false)
    }

    /// Whether playback is available (C++ `len(timeSamples) > 1`).
    /// True only when there are at least 2 distinct time samples.
    pub fn playback_available(&self) -> bool {
        self.stage_time_samples.len() > 1
    }

    /// Loads a stage from file.
    pub fn load_stage(
        &mut self,
        path: &FsPath,
        unloaded: bool,
        population_mask_paths: Option<&[String]>,
    ) -> Result<(), String> {
        use usd_core::{InitialLoadSet, StagePopulationMask};

        // Resolve to absolute, then strip Windows UNC prefix (\\?\) which breaks SDF
        let abs_path: PathBuf = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .map_err(|e| format!("Cannot get CWD: {e}"))?
                .join(path)
        };
        if !abs_path.exists() {
            return Err(format!("File not found: {}", path.display()));
        }
        let mut path_str = abs_path.to_string_lossy().replace('\\', "/");
        // Strip UNC prefix that canonicalize/current_dir can produce on Windows
        if path_str.starts_with("//?/") {
            path_str = path_str[4..].to_string();
        }

        let load_set = if unloaded {
            InitialLoadSet::LoadNone
        } else {
            InitialLoadSet::LoadAll
        };

        // Build population mask before opening, so it applies during composition (C++ OpenMasked)
        let mask = if let Some(paths) = population_mask_paths {
            if !paths.is_empty() {
                let sdf_paths: Vec<Path> =
                    paths.iter().filter_map(|s| Path::from_string(s)).collect();
                if sdf_paths.len() != paths.len() {
                    return Err("Invalid path in population mask".to_string());
                }
                Some(StagePopulationMask::from_paths(sdf_paths))
            } else {
                None
            }
        } else {
            None
        };

        let stage = if let Some(m) = mask {
            // Use Stage::open_masked to apply mask during composition (efficient)
            Stage::open_masked(path_str, m, load_set)
                .map_err(|e| format!("Failed to open {}: {}", path.display(), e))?
        } else {
            Stage::open(path_str, load_set)
                .map_err(|e| format!("Failed to open {}: {}", path.display(), e))?
        };

        // Per C++ appController.py _UpdateTimeSamples (lines 1399-1408):
        // - has time range → currentFrame = startTimeCode
        // - no time range  → currentFrame = 0.0
        // NEVER leave as Default (NaN): USDC files store xformOps as time
        // samples, NaN skips them → identity matrices → "exploded" rendering.
        if stage.has_authored_time_code_range() {
            self.current_time = TimeCode::new(stage.get_start_time_code());
        } else {
            self.current_time = TimeCode::new(0.0);
        }

        // Revoke any previous notice listener
        if let Some(key) = self.notice_listener.take() {
            usd_tf::notice::revoke(key);
        }

        // Register ObjectsChanged listener (per C++ rootDataModel.py:76-78)
        let change_state = self.change_state.clone();
        self.notice_listener = Some(usd_tf::notice::register_global::<
            usd_core::notice::ObjectsChanged,
            _,
        >(
            move |notice: &usd_core::notice::ObjectsChanged| {
                let mut prim_change = ChangeNotice::None;
                let mut prop_change = ChangeNotice::None;

                // Classify resynced paths
                for p in notice.get_resynced_paths().iter() {
                    if p.is_absolute_root_or_prim_path() {
                        prim_change = ChangeNotice::Resync;
                    }
                    if p.is_property_path() {
                        prop_change = ChangeNotice::Resync;
                    }
                }

                // Classify info-only changes (only upgrade from None)
                if prim_change == ChangeNotice::None || prop_change == ChangeNotice::None {
                    for p in notice.get_changed_info_only_paths().iter() {
                        if p.is_prim_path() && prim_change == ChangeNotice::None {
                            prim_change = ChangeNotice::InfoChanges;
                        }
                        if p.is_property_path() && prop_change == ChangeNotice::None {
                            prop_change = ChangeNotice::InfoChanges;
                        }
                    }
                }

                // Store in shared state
                if let Ok(mut state) = change_state.lock() {
                    // Upgrade severity (Resync > InfoChanges > None)
                    if prim_change as u8 > state.0 as u8 {
                        state.0 = prim_change;
                    }
                    if prop_change as u8 > state.1 as u8 {
                        state.1 = prop_change;
                    }
                }
            },
        ));

        // Build reference-style timeline samples from the stage time range metadata.
        self.stage_time_samples = Self::collect_stage_time_samples(&stage);

        self.stage = Some(stage);
        self.file_path = Some(abs_path);

        Ok(())
    }

    /// Opens a stage from file without touching self — safe for background threads.
    ///
    /// Returns the opened `Arc<Stage>`. Does NOT register notice listeners,
    /// set current_time, or collect time samples (caller does that).
    pub fn open_stage_detached(
        path: &FsPath,
        unloaded: bool,
        population_mask_paths: Option<&[String]>,
    ) -> Result<Arc<Stage>, String> {
        use usd_core::{InitialLoadSet, StagePopulationMask};

        let abs_path: PathBuf = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .map_err(|e| format!("Cannot get CWD: {e}"))?
                .join(path)
        };
        if !abs_path.exists() {
            return Err(format!("File not found: {}", path.display()));
        }
        let mut path_str = abs_path.to_string_lossy().replace('\\', "/");
        if path_str.starts_with("//?/") {
            path_str = path_str[4..].to_string();
        }

        let load_set = if unloaded {
            InitialLoadSet::LoadNone
        } else {
            InitialLoadSet::LoadAll
        };

        let mask = if let Some(paths) = population_mask_paths {
            if !paths.is_empty() {
                let sdf_paths: Vec<Path> =
                    paths.iter().filter_map(|s| Path::from_string(s)).collect();
                if sdf_paths.len() != paths.len() {
                    return Err("Invalid path in population mask".to_string());
                }
                Some(StagePopulationMask::from_paths(sdf_paths))
            } else {
                None
            }
        } else {
            None
        };

        let stage = if let Some(m) = mask {
            Stage::open_masked(path_str, m, load_set)
                .map_err(|e| format!("Failed to open {}: {}", path.display(), e))?
        } else {
            Stage::open(path_str, load_set)
                .map_err(|e| format!("Failed to open {}: {}", path.display(), e))?
        };

        Ok(stage)
    }

    /// Applies a pre-loaded stage to this model (UI thread only).
    ///
    /// Sets current_time, registers notice listener, stores stage and path.
    /// Called after background loading completes.
    pub fn apply_loaded_stage(&mut self, stage: Arc<Stage>, time_samples: Vec<f64>, path: PathBuf) {
        // Set initial time (same logic as load_stage)
        if stage.has_authored_time_code_range() {
            self.current_time = TimeCode::new(stage.get_start_time_code());
        } else {
            self.current_time = TimeCode::new(0.0);
        }

        // Revoke previous notice listener
        if let Some(key) = self.notice_listener.take() {
            usd_tf::notice::revoke(key);
        }

        // Register ObjectsChanged listener
        let change_state = self.change_state.clone();
        self.notice_listener = Some(usd_tf::notice::register_global::<
            usd_core::notice::ObjectsChanged,
            _,
        >(
            move |notice: &usd_core::notice::ObjectsChanged| {
                let mut prim_change = ChangeNotice::None;
                let mut prop_change = ChangeNotice::None;

                for path in notice.get_resynced_paths().iter() {
                    if path.is_absolute_root_or_prim_path() {
                        prim_change = ChangeNotice::Resync;
                    }
                    if path.is_property_path() {
                        prop_change = ChangeNotice::Resync;
                    }
                }

                if prim_change == ChangeNotice::None || prop_change == ChangeNotice::None {
                    for path in notice.get_changed_info_only_paths().iter() {
                        if path.is_prim_path() && prim_change == ChangeNotice::None {
                            prim_change = ChangeNotice::InfoChanges;
                        }
                        if path.is_property_path() && prop_change == ChangeNotice::None {
                            prop_change = ChangeNotice::InfoChanges;
                        }
                    }
                }

                if let Ok(mut state) = change_state.lock() {
                    if prim_change as u8 > state.0 as u8 {
                        state.0 = prim_change;
                    }
                    if prop_change as u8 > state.1 as u8 {
                        state.1 = prop_change;
                    }
                }
            },
        ));

        self.stage_time_samples = time_samples;
        self.stage = Some(stage);
        self.file_path = Some(path);
    }

    /// Rebuilds playback samples from the current effective frame range.
    pub fn rebuild_stage_time_samples(&mut self, step: f64) {
        if !self.has_frame_range() {
            self.stage_time_samples.clear();
            return;
        }

        let start = self.frame_range_start();
        let end = self.frame_range_end();
        self.stage_time_samples = drange(start, end, step.max(0.001));
    }

    /// Clamps current time into the active playback samples, or resets to 0.0.
    pub fn clamp_current_time_to_samples(&mut self) {
        match (
            self.stage_time_samples.first().copied(),
            self.stage_time_samples.last().copied(),
        ) {
            (Some(first), Some(last)) => {
                let clamped = self.current_time.value().clamp(first, last);
                self.current_time = TimeCode::new(clamped);
            }
            _ => {
                self.current_time = TimeCode::new(0.0);
            }
        }
    }

    /// Builds playback samples from stage metadata, matching usdviewq `_UpdateTimeSamples`.
    pub fn collect_stage_time_samples(stage: &Stage) -> Vec<f64> {
        if !stage.has_authored_time_code_range() {
            return Vec::new();
        }

        let start = stage.get_start_time_code();
        let end = stage.get_end_time_code();
        let fps = stage.get_frames_per_second();
        let fps = if fps < 1.0 { 24.0 } else { fps };
        let step = (stage.get_time_codes_per_second() / fps).max(0.001);
        drange(start, end, step)
    }

    /// Drains pending change notices (prim_change, prop_change), resetting to None.
    pub fn drain_changes(&self) -> (ChangeNotice, ChangeNotice) {
        if let Ok(mut state) = self.change_state.lock() {
            let result = *state;
            *state = (ChangeNotice::None, ChangeNotice::None);
            result
        } else {
            (ChangeNotice::None, ChangeNotice::None)
        }
    }

    /// Clears the stage.
    pub fn clear(&mut self) {
        // Revoke ObjectsChanged listener before dropping stage
        if let Some(key) = self.notice_listener.take() {
            usd_tf::notice::revoke(key);
        }
        self.stage = None;
        self.file_path = None;
        // Reset dirty state
        if let Ok(mut state) = self.change_state.lock() {
            *state = (ChangeNotice::None, ChangeNotice::None);
        }
        self.stage_time_samples.clear();
    }

    /// Returns the pseudo-root prim if stage is loaded.
    pub fn pseudo_root(&self) -> Option<Prim> {
        self.stage.as_ref().map(|s| s.get_pseudo_root())
    }

    /// Returns the prim at path if it exists.
    pub fn prim_at_path(&self, path: &Path) -> Option<Prim> {
        self.stage.as_ref().and_then(|s| s.get_prim_at_path(path))
    }
}

// ---------------------------------------------------------------------------
// ViewSettingsDataModel — all display/render settings (persisted)
// ---------------------------------------------------------------------------

/// All display and render settings (reference: ViewSettingsDataModel).
/// Implements Serialize/Deserialize for RON persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ViewSettingsDataModel {
    // -- Render --
    pub draw_mode: DrawMode,
    pub complexity: f64,
    pub color_correction_mode: ColorCorrectionMode,
    pub ocio_settings: OcioSettings,
    pub pick_mode: PickMode,

    // -- Selection --
    pub sel_highlight_mode: SelectionHighlightMode,
    pub highlight_color: HighlightColor,

    // -- Background --
    pub clear_color: ClearColor,

    // -- Lighting --
    pub ambient_light_only: bool,
    pub dome_light_enabled: bool,
    pub dome_light_textures_visible: bool,
    /// Last loaded HDRI environment map path (persisted across sessions).
    pub hdr_path: Option<String>,

    // -- Purpose visibility --
    pub display_guide: bool,
    pub display_proxy: bool,
    pub display_render: bool,

    // -- Bounding boxes --
    pub show_bboxes: bool,
    pub show_aa_bbox: bool,
    pub show_ob_box: bool,
    pub show_bbox_playback: bool,

    // -- Rendering flags --
    pub auto_compute_clipping_planes: bool,
    pub cull_backfaces: bool,
    pub enable_scene_materials: bool,
    pub enable_scene_lights: bool,

    // -- Camera mask --
    pub camera_mask_mode: CameraMaskMode,
    pub show_mask_outline: bool,
    pub show_reticles_inside: bool,
    pub show_reticles_outside: bool,
    pub display_camera_oracles: bool,
    pub camera_mask_color: [f32; 4],
    pub camera_reticles_color: [f32; 4],

    // -- Default material --
    pub default_material_ambient: f32,
    pub default_material_specular: f32,

    // -- Free camera --
    pub free_camera_fov: f64,
    pub free_camera_aspect: f64,
    pub lock_free_camera_aspect: bool,
    pub free_camera_override_near: Option<f64>,
    pub free_camera_override_far: Option<f64>,

    // -- Grid --
    pub show_grid: bool,

    // -- HUD --
    pub show_hud: bool,
    pub show_hud_info: bool,
    pub show_hud_complexity: bool,
    pub show_hud_performance: bool,
    pub show_hud_gpu_stats: bool,
    pub show_hud_vbo_info: bool,

    // -- Prim browser --
    pub show_inactive_prims: bool,
    pub show_all_prototype_prims: bool,
    pub show_undefined_prims: bool,
    pub show_abstract_prims: bool,
    pub show_prim_display_names: bool,
    /// When enabled, hovering over prims in viewport shows a tooltip with path/type/visibility.
    pub rollover_prim_info: bool,

    // -- Extents / interpolation --
    pub use_extents_hint: bool,
    /// true = Held, false = Linear
    pub interpolation_held: bool,

    // -- Playback --
    pub redraw_on_scrub: bool,
    pub step_size: f64,

    // -- UI --
    pub font_size: u32,

    // -- Camera path (stored as string for serde, convert via Path::from_string) --
    pub active_camera_path: Option<String>,

    // -- Camera sensitivity (from Preferences > Camera) --
    /// Orbit/tumble sensitivity multiplier (default 1.0 = original 0.25 deg/px).
    pub tumble_speed: f32,
    /// Scroll/dolly zoom sensitivity multiplier (default 1.0).
    pub zoom_speed: f32,
    /// Seed near clipping plane for manual free-camera overrides.
    ///
    /// The normal free-camera path uses adaptive clipping from
    /// `FreeCamera::projection_matrix()`. This value is only used to seed the
    /// Adjust Free Camera dialog when the user opts into an explicit override.
    pub default_near_clip: f64,
    /// Seed far clipping plane for manual free-camera overrides.
    ///
    /// Kept separate from the normal adaptive projection path for the same
    /// reason as `default_near_clip`: fixed far values are a user override,
    /// not the default runtime policy.
    pub default_far_clip: f64,

    // -- Overlays (from Preferences > Viewport) --
    /// Show XYZ axis indicator in the viewport corner.
    pub show_axes: bool,

    // -- Default material (from Preferences > Materials; mirrors default_material_ambient/specular) --
    /// Default material roughness (0.0 = smooth, 1.0 = rough).
    pub default_material_roughness: f32,
}

impl Default for ViewSettingsDataModel {
    fn default() -> Self {
        Self {
            draw_mode: DrawMode::default(),
            complexity: RefinementComplexity::Low.value(),
            color_correction_mode: ColorCorrectionMode::default(),
            ocio_settings: OcioSettings::default(),
            pick_mode: PickMode::default(),
            sel_highlight_mode: SelectionHighlightMode::default(),
            highlight_color: HighlightColor::default(),
            clear_color: ClearColor::default(),
            ambient_light_only: true,
            dome_light_enabled: false,
            dome_light_textures_visible: true,
            hdr_path: None,
            display_guide: false,
            display_proxy: true,
            display_render: false,
            show_bboxes: true,
            show_aa_bbox: true,
            show_ob_box: true,
            show_bbox_playback: false,
            auto_compute_clipping_planes: false,
            cull_backfaces: false,
            enable_scene_materials: true,
            enable_scene_lights: true,
            camera_mask_mode: CameraMaskMode::default(),
            show_mask_outline: false,
            show_reticles_inside: false,
            show_reticles_outside: false,
            display_camera_oracles: false,
            camera_mask_color: [0.1, 0.1, 0.1, 1.0],
            camera_reticles_color: [0.0, 0.7, 1.0, 1.0],
            default_material_ambient: 0.2,
            default_material_specular: 0.1,
            free_camera_fov: 60.0,
            free_camera_aspect: 1.0,
            lock_free_camera_aspect: false,
            free_camera_override_near: None,
            free_camera_override_far: None,
            show_grid: true,
            show_hud: true,
            show_hud_info: false,
            show_hud_complexity: true,
            show_hud_performance: true,
            show_hud_gpu_stats: false,
            show_hud_vbo_info: false,
            show_inactive_prims: true,
            show_all_prototype_prims: false,
            show_undefined_prims: false,
            show_abstract_prims: false,
            show_prim_display_names: true,
            rollover_prim_info: false,
            use_extents_hint: true,
            interpolation_held: false,
            redraw_on_scrub: true,
            step_size: 1.0,
            font_size: 10,
            active_camera_path: None,
            tumble_speed: 1.0,
            zoom_speed: 1.0,
            default_near_clip: 0.1,
            default_far_clip: 10000.0,
            show_axes: true,
            default_material_roughness: 0.5,
        }
    }
}

impl ViewSettingsDataModel {
    /// Whether camera mask is shown (FULL or PARTIAL).
    /// Per viewSettingsDataModel.py:644.
    pub fn show_mask(&self) -> bool {
        matches!(
            self.camera_mask_mode,
            CameraMaskMode::Full | CameraMaskMode::Partial
        )
    }

    /// Whether camera mask is fully opaque.
    /// Per viewSettingsDataModel.py:648.
    pub fn show_mask_opaque(&self) -> bool {
        self.camera_mask_mode == CameraMaskMode::Full
    }

    /// Gets active camera path as SdfPath.
    pub fn active_camera(&self) -> Option<Path> {
        self.active_camera_path
            .as_deref()
            .and_then(Path::from_string)
    }

    /// Sets active camera path, validating the prim is a UsdGeomCamera.
    /// Matches C++ viewSettingsDataModel.py cameraPrim setter (line 855-863).
    /// Pass `stage` to validate; if None, sets path without validation.
    pub fn set_active_camera(&mut self, path: Option<&Path>, stage: Option<&Stage>) {
        match path {
            Some(p) => {
                if let Some(stg) = stage {
                    // Validate path points to a Camera prim
                    if let Some(prim) = stg.get_prim_at_path(p) {
                        let camera_token = usd_tf::Token::new("Camera");
                        if prim.is_a(&camera_token) {
                            self.active_camera_path = Some(p.to_string());
                        } else {
                            log::warn!("Camera path '{}' is not a UsdGeomCamera -- ignoring", p);
                        }
                    } else {
                        log::warn!("Camera path '{}' not found in stage -- ignoring", p);
                    }
                } else {
                    // No stage for validation, accept raw path
                    self.active_camera_path = Some(p.to_string());
                }
            }
            None => {
                self.active_camera_path = None;
            }
        }
    }

    /// Sets camera from a prim, validating it is a UsdGeom.Camera.
    /// Matches C++ viewSettingsDataModel.py cameraPrim setter (line 856).
    pub fn set_camera_prim(&mut self, prim: Option<&Prim>) {
        match prim {
            Some(p) => {
                let camera_token = usd_tf::Token::new("Camera");
                if p.is_a(&camera_token) {
                    self.active_camera_path = Some(p.get_path().to_string());
                } else {
                    log::warn!(
                        "Attempted to view scene using prim '{}', \
                         but it is not a UsdGeom.Camera",
                        p.name()
                    );
                }
            }
            None => {
                self.active_camera_path = None;
            }
        }
    }

    /// Default ambient value for material reset (matches C++ DEFAULT_AMBIENT = 0.2).
    pub const DEFAULT_AMBIENT: f32 = 0.2;
    /// Default specular value for material reset (matches C++ DEFAULT_SPECULAR = 0.1).
    pub const DEFAULT_SPECULAR: f32 = 0.1;

    /// Reset default material ambient/specular to factory values.
    /// Matches C++ viewSettingsDataModel.py resetDefaultMaterial().
    pub fn reset_default_material(&mut self) {
        self.default_material_ambient = Self::DEFAULT_AMBIENT;
        self.default_material_specular = Self::DEFAULT_SPECULAR;
    }
}

/// OCIO color management settings.
/// Reference: usdviewq viewSettingsDataModel.py OCIOSettings + hdx colorCorrectionTask params.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OcioSettings {
    /// Whether OCIO color management is enabled.
    pub enabled: bool,
    /// OCIO display name (e.g. "sRGB", "ACES"). Empty = config default.
    pub display: String,
    /// OCIO view name (e.g. "Film", "Raw"). Empty = config default for display.
    pub view: String,
    /// Input color space (e.g. "ACEScg"). Empty = scene_linear role.
    pub color_space: String,
    /// OCIO looks override (e.g. "show_lut"). Empty = none.
    pub looks: String,
    /// 3D LUT edge length for GPU path (default 65 per C++ HdxColorCorrectionTask).
    pub lut3d_size: i32,
}

impl Default for OcioSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            display: String::new(),
            view: String::new(),
            color_space: String::new(),
            looks: String::new(),
            lut3d_size: 65,
        }
    }
}

/// Backward-compatible alias for persisted settings.
/// ViewSettingsDataModel IS the persisted settings now.
pub type PersistedViewSettings = ViewSettingsDataModel;

// ---------------------------------------------------------------------------
// SelectionDataModel — prim/prop/instance/point selection state
// ---------------------------------------------------------------------------

/// Sentinel: all instances of a prim are selected (mirrors Python ALL_INSTANCES = -1).
pub const ALL_INSTANCES: i32 = -1;

/// Prim + property selection state (reference: SelectionDataModel).
/// Mirrors the Python SelectionDataModel architecture:
///   - Prim selection: ordered vec + O(1) hashset + ancestor set
///   - Instance selection: per-prim instance index sets (ALL_INSTANCES = all)
///   - Property selection: ordered (prim_path, prop_name) pairs with target sets
///   - Computed property selection: same shape as prop but for virtual props
///   - Point selection: 3D point in world space (for point instancers)
///   - Batch/diff: begin_batch/end_batch deferred recompute, get_diff() drain
#[derive(Debug)]
pub struct SelectionDataModel {
    // --- Prim selection ---
    /// Ordered selected prim paths (insertion order = focus order).
    pub prims: Vec<Path>,
    /// O(1) lookup mirror of `prims`.
    prim_set: HashSet<Path>,
    /// Pre-computed ancestor paths of all selected prims (for tree highlighting).
    pub ancestor_set: HashSet<Path>,
    /// LCD (least common denominator) paths: prims without ancestors also selected.
    pub lcd_paths: Vec<Path>,

    // --- Instance selection ---
    /// Per-prim instance indices. Value = None means ALL_INSTANCES.
    pub instances: HashMap<Path, Option<HashSet<i32>>>,

    // --- Property selection: (prim_path, prop_name) -> target paths ---
    pub props: Vec<(Path, String)>,
    prop_set: HashSet<(Path, String)>,
    pub prop_targets: HashMap<(Path, String), HashSet<Path>>,

    // --- Computed property selection ---
    pub computed_props: Vec<(Path, String)>,
    computed_prop_set: HashSet<(Path, String)>,

    // --- Point selection (for point instancers) ---
    pub point: [f32; 3],

    // --- Batching ---
    /// Nesting counter; recompute is deferred while > 0.
    batch_count: u32,
    /// True when a batched change happened that needs recomputation.
    batch_dirty: bool,

    // --- Diff tracking ---
    /// Prim paths added since last get_diff() call.
    pub diff_added: HashSet<Path>,
    /// Prim paths removed since last get_diff() call.
    pub diff_removed: HashSet<Path>,

}

impl Default for SelectionDataModel {
    fn default() -> Self {
        Self {
            prims: vec![Path::absolute_root()],
            prim_set: [Path::absolute_root()].into_iter().collect(),
            ancestor_set: HashSet::new(),
            lcd_paths: vec![Path::absolute_root()],
            instances: HashMap::new(),
            props: Vec::new(),
            prop_set: HashSet::new(),
            prop_targets: HashMap::new(),
            computed_props: Vec::new(),
            computed_prop_set: HashSet::new(),
            point: [0.0, 0.0, 0.0],
            batch_count: 0,
            batch_dirty: false,
            diff_added: HashSet::new(),
            diff_removed: HashSet::new(),
        }
    }
}

impl SelectionDataModel {
    // -----------------------------------------------------------------------
    // Prim selection
    // -----------------------------------------------------------------------

    /// Sets prim selection to the given paths (replaces previous selection).
    /// All paths must be absolute root or prim paths.
    pub fn set_paths(&mut self, paths: Vec<Path>) {
        debug_assert!(
            paths.iter().all(|p| p.is_absolute_root_or_prim_path()),
            "All paths must be prim paths"
        );
        let new_set: HashSet<Path> = paths.iter().cloned().collect();
        // Diff: removed = old - new
        let removed: Vec<Path> = self
            .prims
            .iter()
            .filter(|p| !new_set.contains(p))
            .cloned()
            .collect();
        // Diff: added = new - old
        let added: Vec<Path> = paths
            .iter()
            .filter(|p| !self.prim_set.contains(p))
            .cloned()
            .collect();
        for p in &removed {
            self.diff_remove(p);
        }
        for p in &added {
            self.diff_add(p);
        }
        self.prims = paths;
        self.instances.retain(|k, _| new_set.contains(k));
        self.rebuild_prim_sets();
    }

    /// Adds a prim path to the selection (no-op if already selected).
    /// Respects batch mode: defers ancestor/LCD recomputation until end_batch().
    /// Path must be absolute root or prim path (per Python _ensureValidPrimPath).
    pub fn add_path(&mut self, path: Path) {
        debug_assert!(
            path.is_absolute_root_or_prim_path(),
            "Path must be a prim path, got: {:?}",
            path
        );
        if self.prim_set.contains(&path) {
            return;
        }
        self.diff_add(&path);
        self.prim_set.insert(path.clone());
        self.prims.push(path);
        if self.is_batching() {
            self.batch_dirty = true;
        } else {
            self.rebuild_ancestors();
            self.rebuild_lcd();
        }
    }

    /// Removes a prim path from the selection.
    /// Respects batch mode: defers ancestor/LCD recomputation until end_batch().
    pub fn remove_path(&mut self, path: &Path) {
        if !self.prim_set.remove(path) {
            return;
        }
        self.diff_remove(path);
        self.prims.retain(|p| p != path);
        self.instances.remove(path);
        // Selection must never be empty
        if self.prims.is_empty() {
            self.prims.push(Path::absolute_root());
            self.prim_set.insert(Path::absolute_root());
        }
        if self.is_batching() {
            self.batch_dirty = true;
        } else {
            self.rebuild_ancestors();
            self.rebuild_lcd();
        }
    }

    /// Toggles a prim path in the selection.
    pub fn toggle_path(&mut self, path: Path) {
        if self.prim_set.contains(&path) {
            self.remove_path(&path);
        } else {
            self.add_path(path);
        }
    }

    /// O(1) check whether a path is selected.
    pub fn is_selected(&self, path: &Path) -> bool {
        self.prim_set.contains(path)
    }

    /// O(1) check whether a path is an ancestor of any selected prim.
    pub fn is_ancestor_of_selected(&self, path: &Path) -> bool {
        self.ancestor_set.contains(path)
    }

    /// Returns the focus (first / most recently set) selected path.
    /// Per Python _requireNotBatchingPrims: panics in debug if called during batch.
    pub fn focus_path(&self) -> Option<&Path> {
        debug_assert!(
            !self.is_batching(),
            "cannot read prim selection while batching"
        );
        self.prims.first()
    }

    /// Returns all selected prim paths as a slice (alias: `prims`).
    pub fn get_paths(&self) -> &[Path] {
        debug_assert!(
            !self.is_batching(),
            "cannot read prim selection while batching"
        );
        &self.prims
    }

    /// LCD paths: selected paths that have no selected ancestor.
    pub fn get_lcd_paths(&self) -> &[Path] {
        debug_assert!(
            !self.is_batching(),
            "cannot read prim selection while batching"
        );
        &self.lcd_paths
    }

    // -----------------------------------------------------------------------
    // Instance selection
    // -----------------------------------------------------------------------

    /// Sets the instance selection for a prim. Pass `ALL_INSTANCES` sentinel
    /// (`-1`) to select all instances, or a non-negative index for one instance.
    pub fn add_prim_instance(&mut self, path: Path, instance: i32) {
        if instance == ALL_INSTANCES {
            self.instances.insert(path, None); // None = all
        } else {
            self.instances
                .entry(path)
                .or_insert_with(|| Some(HashSet::new()))
                .get_or_insert_with(HashSet::new)
                .insert(instance);
        }
    }

    /// Removes one instance from a prim's instance selection.
    /// If the last instance is removed the prim entry is kept (all cleared).
    pub fn remove_prim_instance(&mut self, path: &Path, instance: i32) {
        if let Some(entry) = self.instances.get_mut(path) {
            if instance == ALL_INSTANCES {
                *entry = Some(HashSet::new());
            } else if let Some(set) = entry {
                set.remove(&instance);
            }
        }
    }

    /// Clears the instance selection for a prim (reverts to all-instances).
    pub fn clear_prim_instances(&mut self, path: &Path) {
        self.instances.remove(path);
    }

    /// Returns the selected instances for a prim.
    /// `None` means all instances; `Some(set)` means specific instances.
    pub fn get_prim_instances(&self, path: &Path) -> Option<Option<&HashSet<i32>>> {
        self.instances.get(path).map(|v| v.as_ref())
    }

    // -----------------------------------------------------------------------
    // Property selection
    // -----------------------------------------------------------------------

    /// Adds a property (by prim path + name) to the property selection.
    pub fn add_prop(&mut self, prim_path: Path, prop_name: String) {
        let key = (prim_path, prop_name);
        if !self.prop_set.contains(&key) {
            self.prop_set.insert(key.clone());
            self.props.push(key);
        }
    }

    /// Removes a property from the property selection.
    pub fn remove_prop(&mut self, prim_path: &Path, prop_name: &str) {
        let key = (prim_path.clone(), prop_name.to_string());
        if self.prop_set.remove(&key) {
            self.props.retain(|k| k != &key);
            self.prop_targets.remove(&key);
        }
    }

    /// Clears the property selection.
    pub fn clear_props(&mut self) {
        self.props.clear();
        self.prop_set.clear();
        self.prop_targets.clear();
    }

    /// Clear property selection then add a single property (per Python setPropPath).
    pub fn set_prop(&mut self, prim_path: Path, prop_name: String) {
        self.clear_props();
        self.add_prop(prim_path, prop_name);
    }

    /// Adds a target path to a property's target set.
    pub fn add_prop_target(&mut self, prim_path: Path, prop_name: String, target: Path) {
        let key = (prim_path.clone(), prop_name.clone());
        self.add_prop(prim_path, prop_name);
        self.prop_targets.entry(key).or_default().insert(target);
    }

    /// Returns the focus property (last in the ordered list), if any.
    pub fn focus_prop(&self) -> Option<&(Path, String)> {
        self.props.last()
    }

    // -----------------------------------------------------------------------
    // Computed property selection
    // -----------------------------------------------------------------------

    /// Adds a computed property (xformOp:translate, bbox, etc.) to the selection.
    pub fn add_computed_prop(&mut self, prim_path: Path, prop_name: String) {
        let key = (prim_path, prop_name);
        if !self.computed_prop_set.contains(&key) {
            self.computed_prop_set.insert(key.clone());
            self.computed_props.push(key);
        }
    }

    /// Removes a computed property from the selection.
    pub fn remove_computed_prop(&mut self, prim_path: &Path, prop_name: &str) {
        let key = (prim_path.clone(), prop_name.to_string());
        if self.computed_prop_set.remove(&key) {
            self.computed_props.retain(|k| k != &key);
        }
    }

    /// Clears the computed property selection.
    pub fn clear_computed_props(&mut self) {
        self.computed_props.clear();
        self.computed_prop_set.clear();
    }

    /// Returns the focus computed property (last added), if any.
    pub fn focus_computed_prop(&self) -> Option<&(Path, String)> {
        self.computed_props.last()
    }

    // -----------------------------------------------------------------------
    // Point selection
    // -----------------------------------------------------------------------

    /// Sets the 3-D world-space pick point (for point instancers).
    pub fn set_point(&mut self, point: [f32; 3]) {
        self.point = point;
    }

    /// Clears the pick point back to origin.
    pub fn clear_point(&mut self) {
        self.point = [0.0, 0.0, 0.0];
    }

    // -----------------------------------------------------------------------
    // Batching
    // -----------------------------------------------------------------------

    /// Begin a batch: defers ancestor/LCD recomputation.
    pub fn begin_batch(&mut self) {
        self.batch_count += 1;
    }

    /// End a batch. When the last nested batch closes, recomputes sets if dirty.
    pub fn end_batch(&mut self) {
        if self.batch_count > 0 {
            self.batch_count -= 1;
        }
        if self.batch_count == 0 && self.batch_dirty {
            self.batch_dirty = false;
            self.rebuild_prim_sets();
        }
    }

    /// Returns true while inside a begin_batch/end_batch pair.
    pub fn is_batching(&self) -> bool {
        self.batch_count > 0
    }

    // -----------------------------------------------------------------------
    // Diff tracking
    // -----------------------------------------------------------------------

    /// Returns and drains the added/removed sets accumulated since the last call.
    pub fn get_diff(&mut self) -> (HashSet<Path>, HashSet<Path>) {
        let added = std::mem::take(&mut self.diff_added);
        let removed = std::mem::take(&mut self.diff_removed);
        (added, removed)
    }

    // -----------------------------------------------------------------------
    // Clear all
    // -----------------------------------------------------------------------

    /// Clears all selection state (prims, instances, props, computed props, point).
    pub fn clear(&mut self) {
        self.prims.clear();
        self.prim_set.clear();
        self.ancestor_set.clear();
        self.lcd_paths.clear();
        self.instances.clear();
        self.props.clear();
        self.prop_set.clear();
        self.prop_targets.clear();
        self.computed_props.clear();
        self.computed_prop_set.clear();
        self.point = [0.0, 0.0, 0.0];
        self.diff_added.clear();
        self.diff_removed.clear();
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Rebuilds prim_set, ancestor_set, and lcd_paths from `prims`.
    fn rebuild_prim_sets(&mut self) {
        // Selection must never be empty (per Python _primSelectionChanged)
        if self.prims.is_empty() {
            self.prims.push(Path::absolute_root());
        }
        self.prim_set = self.prims.iter().cloned().collect();
        self.rebuild_ancestors();
        self.rebuild_lcd();
    }

    /// Recomputes ancestor_set from current `prims`.
    fn rebuild_ancestors(&mut self) {
        self.ancestor_set.clear();
        for path in &self.prims {
            let mut p = path.get_parent_path();
            while !p.is_empty() {
                if !self.ancestor_set.insert(p.clone()) {
                    break; // already visited
                }
                p = p.get_parent_path();
            }
        }
    }

    /// Recomputes lcd_paths: paths without a selected ancestor.
    fn rebuild_lcd(&mut self) {
        // Per Python _primSelectionChanged: if >1 path, strip absoluteRootPath
        let paths: Vec<&Path> = if self.prims.len() > 1 {
            self.prims
                .iter()
                .filter(|p| !p.is_absolute_root_path())
                .collect()
        } else {
            self.prims.iter().collect()
        };
        // RemoveDescendentPaths: keep only paths without a selected ancestor
        self.lcd_paths = paths
            .iter()
            .filter(|path| {
                let mut p = path.get_parent_path();
                while !p.is_empty() {
                    if self.prim_set.contains(&p) {
                        return false; // has a selected ancestor
                    }
                    p = p.get_parent_path();
                }
                true
            })
            .cloned()
            .cloned()
            .collect();
    }

    /// Records a path addition in the diff.
    fn diff_add(&mut self, path: &Path) {
        if self.diff_removed.contains(path) {
            // Was removed then re-added: net no change.
            self.diff_removed.remove(path);
        } else {
            self.diff_added.insert(path.clone());
        }
    }

    /// Records a path removal in the diff.
    fn diff_remove(&mut self, path: &Path) {
        if self.diff_added.contains(path) {
            // Was added then removed: net no change.
            self.diff_added.remove(path);
        } else {
            self.diff_removed.insert(path.clone());
        }
    }

    // -----------------------------------------------------------------------
    // Bulk removal (per selectionDataModel.py:740-778)
    // -----------------------------------------------------------------------

    /// Remove all selected paths matching a predicate.
    /// After removal, ensures selection is never empty (falls back to root).
    pub fn remove_matching_paths(&mut self, pred: impl Fn(&Path) -> bool) {
        let to_remove: Vec<Path> = self.prims.iter().filter(|p| pred(p)).cloned().collect();
        if to_remove.is_empty() {
            return;
        }
        for path in &to_remove {
            self.prim_set.remove(path);
            self.diff_remove(path);
            self.instances.remove(path);
        }
        self.prims.retain(|p| !to_remove.iter().any(|r| r == p));
        // Ensure non-empty invariant
        if self.prims.is_empty() {
            self.add_path(Path::absolute_root());
        }
        self.rebuild_ancestors();
        self.rebuild_lcd();
    }

    /// Remove prim paths no longer populated on stage.
    /// Per selectionDataModel.py:768-778.
    pub fn remove_unpopulated_prims(&mut self, stage: &usd_core::Stage) {
        self.remove_matching_paths(|path| stage.get_prim_at_path(path).is_none());
    }

    /// Remove prototype/in-prototype prims from selection.
    /// Per selectionDataModel.py:748-754.
    pub fn remove_prototype_prims(&mut self, stage: &usd_core::Stage) {
        self.remove_matching_paths(|path| {
            stage
                .get_prim_at_path(path)
                .map_or(false, |p| p.is_prototype() || p.is_in_prototype())
        });
    }

    /// Remove abstract prims from selection.
    /// Per selectionDataModel.py:756-760.
    pub fn remove_abstract_prims(&mut self, stage: &usd_core::Stage) {
        self.remove_matching_paths(|path| {
            stage
                .get_prim_at_path(path)
                .map_or(false, |p| p.is_abstract())
        });
    }

    /// Remove undefined prims from selection.
    /// Per selectionDataModel.py:762-766.
    pub fn remove_undefined_prims(&mut self, stage: &usd_core::Stage) {
        self.remove_matching_paths(|path| {
            stage
                .get_prim_at_path(path)
                .map_or(false, |p| !p.is_defined())
        });
    }

    // -----------------------------------------------------------------------
    // Property toggles (per selectionDataModel.py:645-690)
    // -----------------------------------------------------------------------

    /// Toggle a property in the selection.
    pub fn toggle_prop(&mut self, prim_path: Path, prop_name: String) {
        let key = (prim_path.clone(), prop_name.clone());
        if self.prop_set.contains(&key) {
            self.remove_prop(&prim_path, &prop_name);
        } else {
            self.add_prop(prim_path, prop_name);
        }
    }

    /// Toggle a computed property in the selection.
    pub fn toggle_computed_prop(&mut self, prim_path: Path, prop_name: String) {
        let key = (prim_path.clone(), prop_name.clone());
        if self.computed_prop_set.contains(&key) {
            self.remove_computed_prop(&prim_path, &prop_name);
        } else {
            self.add_computed_prop(prim_path, prop_name);
        }
    }

    // -----------------------------------------------------------------------
    // Switch props on prim change (per selectionDataModel.py:520-555)
    // -----------------------------------------------------------------------

    /// Switch property selection from one prim to another.
    /// Preserves property names, transferring them to the target prim.
    pub fn switch_props(&mut self, from_prim: &Path, to_prim: &Path) {
        // Only switch if ALL current props belong to from_prim
        for (prim_path, _) in &self.props {
            if prim_path != from_prim {
                return;
            }
        }
        for (prim_path, _) in &self.computed_props {
            if prim_path != from_prim {
                return;
            }
        }
        let old_props: Vec<(Path, String)> = self.props.clone();
        let old_targets: std::collections::HashMap<(Path, String), HashSet<Path>> =
            self.prop_targets.clone();
        let old_computed: Vec<(Path, String)> = self.computed_props.clone();
        self.clear_props();
        // Don't add non-computed props if target is root
        if !to_prim.is_absolute_root_path() {
            for (_, prop_name) in &old_props {
                let old_key = (from_prim.clone(), prop_name.clone());
                self.add_prop(to_prim.clone(), prop_name.clone());
                if let Some(targets) = old_targets.get(&old_key) {
                    for target in targets {
                        self.add_prop_target(to_prim.clone(), prop_name.clone(), target.clone());
                    }
                }
            }
        }
        self.clear_computed_props();
        for (_, prop_name) in &old_computed {
            self.add_computed_prop(to_prim.clone(), prop_name.clone());
        }
    }

    /// Switch to a new prim path, preserving property selection.
    /// Per appController.py:652-668 (switchToPrimPath).
    pub fn switch_to_path(&mut self, path: Path) {
        let old_prims = self.prims.clone();
        self.set_paths(vec![path.clone()]);
        if old_prims.len() == 1 {
            self.switch_props(&old_prims[0], &path);
        }
    }
}

// ---------------------------------------------------------------------------
// DataModel — top-level composite
// ---------------------------------------------------------------------------

/// Central data model for the viewer, owning all three sub-models.
pub struct DataModel {
    /// Stage and playback state.
    pub root: RootDataModel,
    /// Display/render settings (persisted).
    pub view: ViewSettingsDataModel,
    /// Prim selection state.
    pub selection: SelectionDataModel,
    /// Persistent BBox cache (C++ rootDataModel._bboxCache). RefCell for
    /// interior mutability — `compute_world_bound(&self)` needs mutable cache.
    bbox_cache: RefCell<Option<BBoxCache>>,
    /// Persistent XformCache (C++ rootDataModel._xformCache).
    xform_cache: RefCell<Option<XformCache>>,
}

impl Default for DataModel {
    fn default() -> Self {
        Self {
            root: RootDataModel::default(),
            view: ViewSettingsDataModel::default(),
            selection: SelectionDataModel::default(),
            bbox_cache: RefCell::new(None),
            xform_cache: RefCell::new(None),
        }
    }
}

impl std::fmt::Debug for DataModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataModel")
            .field("root", &self.root)
            .field("view", &self.view)
            .field("selection", &self.selection)
            .field("bbox_cache", &"<BBoxCache>")
            .field("xform_cache", &"<XformCache>")
            .finish()
    }
}

impl DataModel {
    /// Creates a new empty data model.
    pub fn new() -> Self {
        Self::default()
    }

    /// Clears the stage, selection, and camera path.
    pub fn clear(&mut self) {
        self.root.clear();
        self.selection.clear();
        self.view.active_camera_path = None;
    }

    /// Returns the first selected prim (if any).
    pub fn first_selected_prim(&self) -> Option<Prim> {
        self.selection
            .focus_path()
            .and_then(|p| self.root.prim_at_path(p))
    }

    /// Returns clear color as egui Color32 for viewport background.
    pub fn clear_color_color32(&self) -> egui::Color32 {
        self.view.clear_color.to_color32()
    }

    /// Compute world-space bounding box for a prim.
    /// Uses persistent BBoxCache per C++ rootDataModel._bboxCache.
    pub fn compute_world_bound(&self, prim: &Prim) -> usd_gf::BBox3d {
        let mut borrow = self.bbox_cache.borrow_mut();
        let cache = borrow.get_or_insert_with(|| {
            let purposes = crate::bounds::included_purposes_from_view(&self.view);
            BBoxCache::new(
                self.root.current_time,
                purposes,
                self.view.use_extents_hint,
                false,
            )
        });
        cache.compute_world_bound(prim)
    }

    /// Compute the composed scene bbox using the persistent root-data-model cache.
    ///
    /// Free-camera framing and auto-clipping must not instantiate a fresh
    /// `BBoxCache` on every interaction frame. That turns camera orbit/dolly
    /// into a full stage-bound recomputation loop and can collapse the viewer to
    /// roughly one frame per expensive bbox traversal. Reuse the same cache
    /// contract as other root-data-model world-bound queries and fall back from
    /// pseudo-root to `defaultPrim` only when the pseudo-root range is empty.
    pub fn compute_stage_bbox_for_view(&self) -> Option<(usd_gf::Vec3d, usd_gf::Vec3d)> {
        let stage = self.root.stage.as_ref()?;

        let pseudo_root = stage.get_pseudo_root();
        let pseudo_root_bbox = self.compute_world_bound(&pseudo_root);
        if let Some(bounds) = crate::bounds::aligned_range_to_bounds(pseudo_root_bbox) {
            return Some(bounds);
        }

        let default_prim = stage.get_default_prim();
        if !default_prim.is_valid() {
            return None;
        }
        crate::bounds::aligned_range_to_bounds(self.compute_world_bound(&default_prim))
    }

    /// Compute local-to-world transform matrix for a prim.
    /// Uses persistent XformCache per C++ rootDataModel._xformCache.
    pub fn get_local_to_world_transform(&self, prim: &Prim) -> usd_gf::Matrix4d {
        let mut borrow = self.xform_cache.borrow_mut();
        let cache = borrow.get_or_insert_with(|| XformCache::new(self.root.current_time));
        cache.get_local_to_world_transform(prim)
    }

    /// Update cache time when current frame changes.
    /// Per C++ rootDataModel.py currentFrame setter: _bboxCache.SetTime(), _xformCache.SetTime().
    pub fn update_cache_time(&self) {
        let time = self.root.current_time;
        if let Some(ref mut cache) = *self.bbox_cache.borrow_mut() {
            cache.set_time(time);
        }
        if let Some(ref mut cache) = *self.xform_cache.borrow_mut() {
            cache.set_time(time);
        }
    }

    /// Compute bound material for a prim.
    /// Per C++ rootDataModel.py computeBoundMaterial(prim, purpose).
    pub fn compute_bound_material(
        &self,
        prim: &Prim,
        material_purpose: &usd_tf::Token,
    ) -> Option<usd_shade::Material> {
        let api = usd_shade::MaterialBindingAPI::new(prim.clone());
        let mut binding_rel = None;
        let mat = api.compute_bound_material(material_purpose, &mut binding_rel, false);
        if mat.is_valid() {
            Some(mat)
        } else {
            None
        }
    }

    /// Clear all caches. Called on stage change/reload.
    /// Per C++ rootDataModel._clearCaches().
    pub fn clear_caches(&self) {
        *self.bbox_cache.borrow_mut() = None;
        *self.xform_cache.borrow_mut() = None;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drange_matches_reference_example() {
        assert_eq!(
            drange(1.0, 3.0, 0.3),
            vec![1.0, 1.3, 1.6, 1.9, 2.2, 2.5, 2.8]
        );
    }

    #[test]
    fn test_playback_available_requires_multiple_samples() {
        let mut root = RootDataModel::default();
        assert!(!root.playback_available());

        root.stage_time_samples = vec![1.0];
        assert!(!root.playback_available());

        root.stage_time_samples = vec![1.0, 2.0];
        assert!(root.playback_available());
    }

    #[test]
    fn test_rebuild_stage_time_samples_uses_effective_range() {
        let mut root = RootDataModel::default();
        root.frame_range_override = Some((10.0, 11.5));
        root.rebuild_stage_time_samples(0.5);
        assert_eq!(root.stage_time_samples, vec![10.0, 10.5, 11.0, 11.5]);
    }

    // ---------------------------------------------------------------------------
}
