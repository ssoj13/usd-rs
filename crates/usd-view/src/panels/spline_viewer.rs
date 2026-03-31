//! Animation curve / spline viewer panel.
//!
//! Displays time-sampled attribute values as curves in a 2D plot.
//! Reference: splineViewer.py from usdviewq.
//!
//! Features:
//! - Queries attr.get_time_samples() for selected attribute
//! - Draws curves with keyframe dots
//! - Current time vertical indicator (playhead)
//! - Auto-scale axes
//! - Supports f32/f64/i32 scalars + Vec3 (3 curves: R/G/B)
//! - MMB drag to pan, scroll wheel to zoom (P2-6)
//! - Optional time range override start/end fields (P2-7)

use egui::{Color32, Pos2, Rect, Stroke, Ui, Vec2};
use usd_core::Prim;
use usd_sdf::TimeCode;
use usd_vt::Value;

use crate::data_model::DataModel;

// ---------------------------------------------------------------------------
// Constants (matching splineViewer.py)
// ---------------------------------------------------------------------------

const BACKGROUND: Color32 = Color32::from_rgb(30, 30, 30);
const GRID_COLOR: Color32 = Color32::from_rgb(80, 80, 80);
const AXIS_COLOR: Color32 = Color32::from_rgb(200, 200, 200);
const PLAYHEAD_COLOR: Color32 = Color32::from_rgb(255, 255, 0);
const KNOT_COLOR: Color32 = Color32::WHITE;
const KNOT_RADIUS: f32 = 3.0;
const NUM_TICKS: usize = 5;
const MARGIN: f32 = 40.0;
const MIN_DELTA: f64 = 1e-5;

/// Curve colors for multi-channel (Vec3) display.
const CURVE_COLORS: [Color32; 4] = [
    Color32::from_rgb(0, 200, 255),   // single / X
    Color32::from_rgb(100, 255, 100), // Y
    Color32::from_rgb(255, 100, 100), // Z
    Color32::from_rgb(255, 200, 100), // W (if needed)
];

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// Persistent state for the spline viewer panel.
#[derive(Debug)]
pub struct SplineViewerState {
    /// Currently displayed attribute name (cleared on prim change).
    pub attr_name: Option<String>,
    /// Cached prim path for dirty detection.
    cached_prim_path: Option<usd_sdf::Path>,
    /// Cached time samples (time, [channel_values]).
    cached_samples: Vec<(f64, Vec<f64>)>,
    /// Number of channels (1 for scalar, 3 for Vec3, etc.).
    channel_count: usize,
    /// Channel labels.
    channel_labels: Vec<&'static str>,
    /// Data range derived from samples: (min_time, max_time, min_val, max_val).
    data_range: (f64, f64, f64, f64),

    // --- P2-6: pan / zoom ---
    /// View translation in normalised data-space (0..1 per axis before scale).
    pub view_offset: Vec2,
    /// View scale multiplier (1.0 = fit all data).
    pub view_scale: Vec2,

    // --- P2-7: time range override ---
    /// Optional override for the visible start time.
    pub time_range_override_start: Option<f64>,
    /// Optional override for the visible end time.
    pub time_range_override_end: Option<f64>,
}

impl Default for SplineViewerState {
    fn default() -> Self {
        Self {
            attr_name: None,
            cached_prim_path: None,
            cached_samples: Vec::new(),
            channel_count: 0,
            channel_labels: Vec::new(),
            data_range: (0.0, 1.0, 0.0, 1.0),
            view_offset: Vec2::ZERO,
            view_scale: Vec2::new(1.0, 1.0),
            time_range_override_start: None,
            time_range_override_end: None,
        }
    }
}

impl SplineViewerState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Clear cached data (e.g. on prim selection change).
    pub fn clear(&mut self) {
        self.attr_name = None;
        self.cached_prim_path = None;
        self.cached_samples.clear();
        self.channel_count = 0;
        self.channel_labels.clear();
        self.reset_view();
    }

    /// Reset pan/zoom to show all data.
    pub fn reset_view(&mut self) {
        self.view_offset = Vec2::ZERO;
        self.view_scale = Vec2::new(1.0, 1.0);
    }

    /// Set the attribute to display. Rebuilds cache if changed.
    pub fn set_attribute(&mut self, prim: &Prim, attr_name: &str) {
        let prim_path = prim.path().clone();
        if self.cached_prim_path.as_ref() == Some(&prim_path)
            && self.attr_name.as_deref() == Some(attr_name)
        {
            return; // already cached
        }

        self.cached_samples.clear();
        self.channel_count = 0;
        self.channel_labels.clear();

        let Some(attr) = prim.get_attribute(attr_name) else {
            self.attr_name = None;
            return;
        };

        let time_samples = attr.get_time_samples();
        if time_samples.is_empty() {
            self.attr_name = None;
            return;
        }

        // Sample the attribute at each authored time code
        let mut samples: Vec<(f64, Vec<f64>)> = Vec::with_capacity(time_samples.len());
        let mut ch_count = 0usize;
        let mut labels: Vec<&'static str> = Vec::new();

        for &t in &time_samples {
            let tc = TimeCode::new(t);
            let Some(val) = attr.get(tc) else { continue };
            let channels = extract_channels(&val);
            if channels.is_empty() {
                continue;
            }
            if ch_count == 0 {
                ch_count = channels.len();
                labels = match ch_count {
                    1 => vec!["Value"],
                    2 => vec!["X", "Y"],
                    3 => vec!["X", "Y", "Z"],
                    4 => vec!["X", "Y", "Z", "W"],
                    _ => (0..ch_count).map(|_| "?").collect(),
                };
            }
            if channels.len() == ch_count {
                samples.push((t, channels));
            }
        }

        if samples.is_empty() || ch_count == 0 {
            self.attr_name = None;
            return;
        }

        // Compute data range
        let min_t = samples.first().map(|s| s.0).unwrap_or(0.0);
        let max_t = samples.last().map(|s| s.0).unwrap_or(1.0);
        let mut min_v = f64::MAX;
        let mut max_v = f64::MIN;
        for (_, chs) in &samples {
            for &v in chs {
                if v < min_v {
                    min_v = v;
                }
                if v > max_v {
                    max_v = v;
                }
            }
        }
        // Prevent zero-height range
        if (max_v - min_v).abs() < MIN_DELTA {
            min_v -= 1.0;
            max_v += 1.0;
        }

        self.cached_samples = samples;
        self.channel_count = ch_count;
        self.channel_labels = labels;
        self.data_range = (min_t, max_t, min_v, max_v);
        self.attr_name = Some(attr_name.to_string());
        self.cached_prim_path = Some(prim_path);
    }
}

/// Extract scalar channel values from a VtValue.
fn extract_channels(val: &Value) -> Vec<f64> {
    // Scalar types
    if let Some(&v) = val.get::<f32>() {
        return vec![v as f64];
    }
    if let Some(&v) = val.get::<f64>() {
        return vec![v];
    }
    if let Some(&v) = val.get::<i32>() {
        return vec![v as f64];
    }
    if let Some(&v) = val.get::<i64>() {
        return vec![v as f64];
    }
    if let Some(&v) = val.get::<bool>() {
        return vec![if v { 1.0 } else { 0.0 }];
    }

    // Vector types
    if let Some(v) = val.get::<usd_gf::Vec2f>() {
        return vec![v.x as f64, v.y as f64];
    }
    if let Some(v) = val.get::<usd_gf::Vec3f>() {
        return vec![v.x as f64, v.y as f64, v.z as f64];
    }
    if let Some(v) = val.get::<usd_gf::Vec3d>() {
        return vec![v.x, v.y, v.z];
    }
    if let Some(v) = val.get::<usd_gf::Vec4f>() {
        return vec![v.x as f64, v.y as f64, v.z as f64, v.w as f64];
    }

    Vec::new()
}

// ---------------------------------------------------------------------------
// Main UI
// ---------------------------------------------------------------------------

/// Draws the spline/animation curve viewer panel.
pub fn ui_spline_viewer(ui: &mut Ui, data_model: &DataModel, state: &mut SplineViewerState) {
    // Header: which attribute are we showing?
    ui.horizontal(|ui| {
        ui.label("Spline Viewer");
        if let Some(ref name) = state.attr_name {
            ui.label(
                egui::RichText::new(format!(" - {}", name)).color(Color32::from_rgb(180, 220, 255)),
            );
        }
    });

    // Attribute picker: list time-sampled attributes of selected prim
    let prim_opt = data_model.first_selected_prim();
    let Some(prim) = prim_opt else {
        ui.label("Select a prim with time-sampled attributes.");
        return;
    };

    // Collect time-sampled attribute names
    let ts_attrs: Vec<String> = prim
        .get_attribute_names()
        .iter()
        .filter_map(|n| {
            let name = n.to_string();
            prim.get_attribute(&name)
                .filter(|a| a.get_num_time_samples() > 0)
                .map(|_| name)
        })
        .collect();

    if ts_attrs.is_empty() {
        ui.label("No time-sampled attributes on this prim.");
        state.clear();
        return;
    }

    // Detect prim change -> clear
    let prim_path = prim.path().clone();
    if state.cached_prim_path.as_ref() != Some(&prim_path) {
        state.clear();
    }

    // Attribute selector combo box
    let current_name = state.attr_name.clone().unwrap_or_default();
    ui.horizontal(|ui| {
        ui.label("Attribute:");
        egui::ComboBox::from_id_salt("spline_attr_sel")
            .selected_text(if current_name.is_empty() {
                "(select)"
            } else {
                &current_name
            })
            .width(200.0)
            .show_ui(ui, |ui| {
                for name in &ts_attrs {
                    if ui.selectable_label(&current_name == name, name).clicked() {
                        state.set_attribute(&prim, name);
                    }
                }
            });

        // P2-6: Frame All button
        if ui.small_button("Frame All").clicked() {
            state.reset_view();
        }
    });

    // Auto-select first attribute if nothing selected
    if state.attr_name.is_none() && !ts_attrs.is_empty() {
        state.set_attribute(&prim, &ts_attrs[0]);
    }

    if state.cached_samples.is_empty() {
        ui.label("No plottable data.");
        return;
    }

    // P2-7: time range override toolbar - fall back to frame range from RootDataModel
    let stage_min_t = data_model.root.frame_range_start();
    let stage_max_t = data_model.root.frame_range_end();
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("T:").small());

        // Start override
        let mut start_val = state.time_range_override_start.unwrap_or(stage_min_t);
        let start_changed = ui
            .add(
                egui::DragValue::new(&mut start_val)
                    .speed(0.5)
                    .prefix("start: ")
                    .max_decimals(1),
            )
            .changed();
        if start_changed {
            state.time_range_override_start = Some(start_val);
        }

        ui.label(" ");

        // End override
        let mut end_val = state.time_range_override_end.unwrap_or(stage_max_t);
        let end_changed = ui
            .add(
                egui::DragValue::new(&mut end_val)
                    .speed(0.5)
                    .prefix("end: ")
                    .max_decimals(1),
            )
            .changed();
        if end_changed {
            state.time_range_override_end = Some(end_val);
        }

        // Clear button: reset overrides to stage range
        if ui
            .small_button("x")
            .on_hover_text("Reset to stage range")
            .clicked()
        {
            state.time_range_override_start = None;
            state.time_range_override_end = None;
        }
    });

    ui.separator();

    // Channel legend
    if state.channel_count > 1 {
        ui.horizontal(|ui| {
            for (i, label) in state.channel_labels.iter().enumerate() {
                let color = CURVE_COLORS[i % CURVE_COLORS.len()];
                let (r, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                ui.painter().rect_filled(r, 2.0, color);
                ui.label(egui::RichText::new(*label).color(color).small());
            }
        });
    }

    // Compute effective time range (override takes priority over stage range)
    let eff_min_t = state.time_range_override_start.unwrap_or(stage_min_t);
    let eff_max_t = state.time_range_override_end.unwrap_or(stage_max_t);

    // Draw the plot using egui::Painter.
    // Sense::click_and_drag allows MMB pan; scroll is read from response.ctx.
    let available = ui.available_size();
    let plot_size = Vec2::new(available.x.max(200.0), available.y.max(150.0));
    let (response, painter) = ui.allocate_painter(plot_size, egui::Sense::click_and_drag());
    let plot_rect = response.rect;

    // --- P2-6: handle MMB drag (pan) ---
    if response.dragged_by(egui::PointerButton::Middle) {
        let drag_delta = response.drag_delta();
        // Convert pixel delta to normalised data-space offset.
        // The inner plot area width/height (minus margins) maps to 1.0 data units.
        let inner_w = (plot_rect.width() - MARGIN - 10.0).max(1.0);
        let inner_h = (plot_rect.height() - 10.0 - MARGIN).max(1.0);
        // Negate: dragging right moves view left (positive data direction)
        state.view_offset.x -= drag_delta.x / inner_w;
        // Y is flipped (screen Y increases down, data Y increases up)
        state.view_offset.y += drag_delta.y / inner_h;
    }

    // --- P2-6: handle scroll wheel (zoom) ---
    let scroll_delta = ui.input(|i| i.smooth_scroll_delta);
    if response.hovered() && scroll_delta.y.abs() > 0.01 {
        let zoom_factor = 1.0 + scroll_delta.y * 0.01;
        state.view_scale.x = (state.view_scale.x * zoom_factor).clamp(0.05, 100.0);
        state.view_scale.y = (state.view_scale.y * zoom_factor).clamp(0.05, 100.0);
    }

    draw_plot(
        &painter,
        plot_rect,
        state,
        data_model.root.current_time.value(),
        eff_min_t,
        eff_max_t,
    );
}

// ---------------------------------------------------------------------------
// Plot drawing
// ---------------------------------------------------------------------------

/// Draw the full plot: background, grid, axes, curves, keyframes, playhead.
///
/// `eff_min_t` / `eff_max_t` are the effective (possibly overridden) time
/// bounds that define what the full view (scale=1, offset=0) shows on X.
/// `state.view_offset` and `state.view_scale` apply pan/zoom on top.
fn draw_plot(
    painter: &egui::Painter,
    rect: Rect,
    state: &SplineViewerState,
    current_time: f64,
    eff_min_t: f64,
    eff_max_t: f64,
) {
    // Background
    painter.rect_filled(rect, 4.0, BACKGROUND);

    // Base data range for Y comes from sample data; X uses effective time bounds.
    let (_, _, base_min_v, base_max_v) = state.data_range;
    let base_dt = (eff_max_t - eff_min_t).abs().max(MIN_DELTA);
    let base_dv = (base_max_v - base_min_v).abs().max(MIN_DELTA);

    // Apply zoom: view_scale shrinks the visible span.
    // view_scale > 1 => zoomed in (smaller visible range).
    let vis_dt = base_dt / f64::from(state.view_scale.x.max(0.001));
    let vis_dv = base_dv / f64::from(state.view_scale.y.max(0.001));

    // Apply pan: view_offset shifts the window start (in data units).
    // view_offset.x = 0 => show from eff_min_t; positive => panned right.
    let min_t = eff_min_t + f64::from(state.view_offset.x) * base_dt;
    let max_t = min_t + vis_dt;
    let min_v = base_min_v + f64::from(state.view_offset.y) * base_dv;

    // Plot area (inside margins)
    let plot = Rect::from_min_max(
        Pos2::new(rect.min.x + MARGIN, rect.min.y + 10.0),
        Pos2::new(rect.max.x - 10.0, rect.max.y - MARGIN),
    );

    if plot.width() < 10.0 || plot.height() < 10.0 {
        return;
    }

    // Coordinate transform: data -> screen (Y flipped)
    let to_screen = |t: f64, v: f64| -> Pos2 {
        let nx = (t - min_t) / vis_dt;
        let ny = (v - min_v) / vis_dv;
        Pos2::new(
            plot.min.x + nx as f32 * plot.width(),
            plot.max.y - ny as f32 * plot.height(),
        )
    };

    // Grid
    let grid_stroke = Stroke::new(0.5, GRID_COLOR);
    for i in 0..=NUM_TICKS {
        let frac = i as f32 / NUM_TICKS as f32;
        let x = plot.min.x + frac * plot.width();
        painter.line_segment(
            [Pos2::new(x, plot.min.y), Pos2::new(x, plot.max.y)],
            grid_stroke,
        );
        let y = plot.min.y + frac * plot.height();
        painter.line_segment(
            [Pos2::new(plot.min.x, y), Pos2::new(plot.max.x, y)],
            grid_stroke,
        );
    }

    // Axes
    let axis_stroke = Stroke::new(1.0, AXIS_COLOR);
    painter.line_segment(
        [
            Pos2::new(plot.min.x, plot.max.y),
            Pos2::new(plot.max.x, plot.max.y),
        ],
        axis_stroke,
    );
    painter.line_segment(
        [
            Pos2::new(plot.min.x, plot.min.y),
            Pos2::new(plot.min.x, plot.max.y),
        ],
        axis_stroke,
    );

    // Axis labels
    let label_font = egui::FontId::proportional(9.0);
    for i in 0..=NUM_TICKS {
        let frac = i as f64 / NUM_TICKS as f64;
        let t_val = min_t + frac * vis_dt;
        let x = plot.min.x + frac as f32 * plot.width();
        painter.text(
            Pos2::new(x, plot.max.y + 4.0),
            egui::Align2::CENTER_TOP,
            format!("{:.1}", t_val),
            label_font.clone(),
            AXIS_COLOR,
        );
        let v_val = min_v + frac * vis_dv;
        let y = plot.max.y - frac as f32 * plot.height();
        painter.text(
            Pos2::new(plot.min.x - 4.0, y),
            egui::Align2::RIGHT_CENTER,
            format!("{:.2}", v_val),
            label_font.clone(),
            AXIS_COLOR,
        );
    }

    // Clip curves to the plot rectangle
    painter.with_clip_rect(plot);

    // Draw curves (one per channel)
    for ch in 0..state.channel_count {
        let color = CURVE_COLORS[ch % CURVE_COLORS.len()];
        let stroke = Stroke::new(1.5, color);

        let points: Vec<Pos2> = state
            .cached_samples
            .iter()
            .map(|(t, chs)| to_screen(*t, chs[ch]))
            .collect();

        if points.len() >= 2 {
            for pair in points.windows(2) {
                painter.line_segment([pair[0], pair[1]], stroke);
            }
        }

        for pt in &points {
            painter.circle_filled(*pt, KNOT_RADIUS, KNOT_COLOR);
        }
    }

    // Playhead (vertical line at current_time)
    if current_time >= min_t && current_time <= max_t {
        let playhead_x = to_screen(current_time, 0.0).x;
        let playhead_stroke = Stroke::new(1.5, PLAYHEAD_COLOR);
        painter.line_segment(
            [
                Pos2::new(playhead_x, plot.min.y),
                Pos2::new(playhead_x, plot.max.y),
            ],
            playhead_stroke,
        );

        for ch in 0..state.channel_count {
            if let Some(val) = interpolate_at(current_time, ch, &state.cached_samples) {
                let pt = to_screen(current_time, val);
                let color = CURVE_COLORS[ch % CURVE_COLORS.len()];
                painter.circle_filled(pt, KNOT_RADIUS + 2.0, color);
                painter.circle_stroke(pt, KNOT_RADIUS + 2.0, Stroke::new(1.0, KNOT_COLOR));
            }
        }
    }
}

/// Linear interpolation of channel value at given time.
fn interpolate_at(t: f64, ch: usize, samples: &[(f64, Vec<f64>)]) -> Option<f64> {
    if samples.is_empty() {
        return None;
    }
    if t <= samples[0].0 {
        return samples[0].1.get(ch).copied();
    }
    if t >= samples.last().unwrap().0 {
        return samples.last().unwrap().1.get(ch).copied();
    }
    // Binary search for bracketing interval
    let idx = samples.partition_point(|s| s.0 < t);
    if idx == 0 {
        return samples[0].1.get(ch).copied();
    }
    let (t0, v0) = (&samples[idx - 1].0, samples[idx - 1].1.get(ch)?);
    let (t1, v1) = (&samples[idx].0, samples[idx].1.get(ch)?);
    let dt = t1 - t0;
    if dt.abs() < MIN_DELTA {
        return Some(*v0);
    }
    let frac = (t - t0) / dt;
    Some(v0 + frac * (v1 - v0))
}
