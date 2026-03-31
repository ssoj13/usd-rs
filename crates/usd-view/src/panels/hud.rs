//! HUD overlay for viewport — stats, performance, camera info.
//!
//! Draws semi-transparent text panels in 4 corners of the viewport rect.
//! Reference: usdviewq stageView.py HUD drawing.

use std::collections::HashMap;
use std::time::Duration;

use crate::data_model::RefinementComplexity;
use crate::formatting::fmt_int;
use egui::{Color32, FontId, Pos2, Rect, Ui};

use usd_core::Stage;

/// Gold color matching usdview HUD text.
const HUD_GOLD: Color32 = Color32::from_rgb(218, 165, 32);
/// Semi-transparent black background for HUD text.
const HUD_BG: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 160);
/// Padding around HUD text blocks.
const HUD_PAD: f32 = 6.0;
/// Font size for HUD text.
const HUD_FONT_SIZE: f32 = 12.0;

/// HUD visibility and cached stats.
pub struct HudState {
    /// Master toggle.
    pub visible: bool,
    /// Show prim info (top-left).
    pub show_info: bool,
    /// Show complexity + camera name (bottom-right).
    pub show_complexity: bool,
    /// Show FPS / render time (bottom-left).
    pub show_performance: bool,
    /// Show GPU stats (bottom-left, below perf).
    pub show_gpu_stats: bool,
    /// Show VBO/buffer info (bottom-left, below GPU stats).
    pub show_vbo_info: bool,

    // Cached stats (updated per-frame or on stage change)
    prim_counts: HashMap<String, usize>,
    total_prims: usize,
    fps: f64,
    render_time_ms: f64,
    /// Complexity label ("Low", "Medium", "High", "Very High").
    pub complexity_name: String,
    /// Active camera name ("Free" or prim name).
    pub camera_name: String,
    /// Active renderer backend.
    pub renderer_name: String,
    /// Whether the stage-derived prim counts need recomputing.
    ///
    /// This stays decoupled from camera/time interaction on purpose. Traversing
    /// the full stage is a scene-content operation, not a per-frame viewport
    /// operation, and running it on a timer can starve interactive repaints on
    /// heavy files even when Hydra rendering itself is cheap.
    prim_stats_dirty: bool,
    // -- GPU / render stats --
    /// Number of draw items submitted to Storm.
    pub draw_item_count: usize,
    /// Number of meshes (approximate triangle groups).
    pub mesh_count: usize,
    /// Frame time in ms (from engine render pass, not wall clock).
    pub gpu_frame_ms: f64,
    /// Readback + color correction time in ms.
    pub readback_ms: f64,
    /// Per-phase timing breakdown (from viewport).
    pub phase_render_ms: f64,
    pub phase_readback_ms: f64,
    pub phase_cc_ms: f64,
    pub phase_tex_ms: f64,
    /// GPU frame time from timestamp queries (if available).
    pub gpu_time_ms: Option<f64>,
    /// Current near/far clip plane values (for debug display).
    pub near_clip: f64,
    pub far_clip: f64,
    // -- VBO / buffer info (P1-14) --
    /// Total GPU buffer memory in bytes.
    pub vbo_total_bytes: u64,
    /// Number of GPU buffers allocated.
    pub vbo_buffer_count: u32,
    /// Triangle count in scene.
    pub triangle_count: u64,
    /// Point count in scene.
    pub point_count: u64,
    /// Reusable scratch buffer for iterative prim traversal (avoids per-call alloc).
    prim_stack_buf: Vec<usd_core::Prim>,
}

impl Default for HudState {
    fn default() -> Self {
        Self::new()
    }
}

impl HudState {
    pub fn new() -> Self {
        Self {
            visible: true,
            show_info: true,
            show_complexity: true,
            show_performance: true,
            show_gpu_stats: false,
            show_vbo_info: false,
            prim_counts: HashMap::with_capacity(32),
            total_prims: 0,
            fps: 0.0,
            render_time_ms: 0.0,
            complexity_name: "Low".to_string(),
            camera_name: "Free".to_string(),
            renderer_name: "Storm".to_string(),
            prim_stats_dirty: true,
            draw_item_count: 0,
            mesh_count: 0,
            gpu_frame_ms: 0.0,
            readback_ms: 0.0,
            phase_render_ms: 0.0,
            phase_readback_ms: 0.0,
            phase_cc_ms: 0.0,
            phase_tex_ms: 0.0,
            gpu_time_ms: None,
            near_clip: 0.0,
            far_clip: 0.0,
            vbo_total_bytes: 0,
            vbo_buffer_count: 0,
            triangle_count: 0,
            point_count: 0,
            prim_stack_buf: Vec::with_capacity(1024),
        }
    }

    /// Marks cached prim statistics dirty after scene content changes.
    ///
    /// Camera motion, scrubbing, and hover do not alter prim topology or prim
    /// type counts, so they must not trigger a full stage traversal. We only
    /// invalidate this cache on scene switch/reload paths.
    pub fn invalidate_prim_stats(&mut self) {
        self.prim_stats_dirty = true;
    }

    /// Recompute prim type counts by traversing the stage when invalidated.
    pub fn update_prim_stats(&mut self, stage: &Stage) {
        usd_trace::trace_scope!("hud_update_prim_stats");
        if !self.prim_stats_dirty {
            return;
        }

        self.prim_counts.clear();
        self.total_prims = 0;
        count_prims_iterative(
            &stage.get_pseudo_root(),
            &mut self.prim_counts,
            &mut self.total_prims,
            &mut self.prim_stack_buf,
        );
        self.prim_stats_dirty = false;
    }

    /// Update FPS / render time from last frame duration.
    pub fn update_performance(&mut self, frame_time: Duration) {
        let secs = frame_time.as_secs_f64().max(1e-9);
        self.fps = 1.0 / secs;
        self.render_time_ms = secs * 1000.0;
    }

    /// Set complexity label from numeric value (1.0 = Low, 1.1 = Medium, etc.).
    pub fn set_complexity(&mut self, complexity: f64) {
        self.complexity_name = RefinementComplexity::from_value(complexity)
            .name()
            .to_string();
    }

    /// Draw HUD overlay onto the viewport rect.
    /// Returns the topmost Y of the bottom-left HUD block (for positioning hover labels above it).
    pub fn draw(&self, ui: &mut Ui, rect: Rect) -> f32 {
        if !self.visible {
            return rect.bottom();
        }

        let painter = ui.painter();
        let font = FontId::monospace(HUD_FONT_SIZE);

        // Top-left: prim info
        if self.show_info {
            let meshes = self.prim_counts.get("Mesh").copied().unwrap_or(0);
            let cameras = self.prim_counts.get("Camera").copied().unwrap_or(0);
            let lights = self.prim_counts.get("Light").copied().unwrap_or(0)
                + self.prim_counts.get("DistantLight").copied().unwrap_or(0)
                + self.prim_counts.get("DomeLight").copied().unwrap_or(0)
                + self.prim_counts.get("SphereLight").copied().unwrap_or(0)
                + self.prim_counts.get("RectLight").copied().unwrap_or(0)
                + self.prim_counts.get("DiskLight").copied().unwrap_or(0)
                + self.prim_counts.get("CylinderLight").copied().unwrap_or(0);
            let text = format!(
                "Prims: {} | Meshes: {} | Cameras: {} | Lights: {}",
                self.total_prims, meshes, cameras, lights
            );
            draw_hud_text(
                painter,
                &text,
                &font,
                rect.left_top() + egui::vec2(HUD_PAD, HUD_PAD),
                HUD_GOLD,
            );
        }

        // Top-right: renderer + AOV
        {
            let text = format!("Hydra: {}  AOV: color", self.renderer_name);
            let galley = painter.layout_no_wrap(text.clone(), font.clone(), HUD_GOLD);
            let pos = Pos2::new(
                rect.right() - galley.size().x - HUD_PAD,
                rect.top() + HUD_PAD,
            );
            draw_hud_text(painter, &text, &font, pos, HUD_GOLD);
        }

        // Bottom-right: complexity + camera + clip planes
        if self.show_complexity {
            // Clip planes line
            let clip_text = format!("Near: {:.4}  Far: {:.1}", self.near_clip, self.far_clip);
            let clip_galley = painter.layout_no_wrap(clip_text.clone(), font.clone(), HUD_GOLD);
            let clip_pos = Pos2::new(
                rect.right() - clip_galley.size().x - HUD_PAD,
                rect.bottom() - clip_galley.size().y - HUD_PAD,
            );
            draw_hud_text(painter, &clip_text, &font, clip_pos, HUD_GOLD);

            // Complexity + camera line (above clip planes)
            let text = format!(
                "Complexity: {}  Camera: {}",
                self.complexity_name, self.camera_name
            );
            let galley = painter.layout_no_wrap(text.clone(), font.clone(), HUD_GOLD);
            let pos = Pos2::new(
                rect.right() - galley.size().x - HUD_PAD,
                clip_pos.y - galley.size().y - 2.0,
            );
            draw_hud_text(painter, &text, &font, pos, HUD_GOLD);
        }

        // Bottom-left: performance + GPU stats (stack upwards from bottom)
        let mut bl_y = rect.bottom(); // track topmost Y used in bottom-left

        if self.show_performance {
            let text = format!(
                "FPS: {:.0} | Render: {:.1}ms | Readback: {:.1}ms | OCIO: {:.1}ms | Tex: {:.1}ms",
                self.fps,
                self.phase_render_ms,
                self.phase_readback_ms,
                self.phase_cc_ms,
                self.phase_tex_ms,
            );
            bl_y = rect.bottom() - HUD_FONT_SIZE - HUD_PAD * 2.0;
            draw_hud_text(
                painter,
                &text,
                &font,
                Pos2::new(rect.left() + HUD_PAD, bl_y),
                HUD_GOLD,
            );

            // GPU time line (if available)
            if let Some(gpu_ms) = self.gpu_time_ms {
                bl_y -= HUD_FONT_SIZE + 2.0;
                let gpu_text = format!("GPU: {:.1}ms", gpu_ms);
                draw_hud_text(
                    painter,
                    &gpu_text,
                    &font,
                    Pos2::new(rect.left() + HUD_PAD, bl_y),
                    HUD_GOLD,
                );
            }
        }

        // GPU timing breakdown table (above perf lines)
        if self.show_gpu_stats {
            let total_gpu = self.phase_render_ms + self.phase_readback_ms;
            let lines: &[String] = &[
                "--- GPU Timing ---".to_string(),
                format!("  Render:     {:6.2} ms", self.phase_render_ms),
                format!("  Readback:   {:6.2} ms", self.phase_readback_ms),
                format!("  Total GPU:  {:6.2} ms", total_gpu),
                format!("  Draw Items: {:>6}", fmt_int(self.draw_item_count as i64)),
                format!("  Meshes:     {:>6}", fmt_int(self.mesh_count as i64)),
            ];
            let line_h = HUD_FONT_SIZE + 2.0;
            let block_h = line_h * lines.len() as f32 + HUD_PAD;
            bl_y -= block_h;
            for (i, line) in lines.iter().enumerate() {
                let y = bl_y + line_h * i as f32;
                draw_hud_text(
                    painter,
                    line,
                    &font,
                    Pos2::new(rect.left() + HUD_PAD, y),
                    HUD_GOLD,
                );
            }
        }

        // VBO / buffer info block (P1-14)
        if self.show_vbo_info {
            let vram_mb = self.vbo_total_bytes as f64 / (1024.0 * 1024.0);
            let lines: &[String] = &[
                "--- VBO Info ---".to_string(),
                format!("  Buffers:   {:>6}", fmt_int(self.vbo_buffer_count as i64)),
                format!("  VRAM:    {:6.1} MB", vram_mb),
                format!("  Triangles: {:>6}", fmt_int(self.triangle_count as i64)),
                format!("  Points:    {:>6}", fmt_int(self.point_count as i64)),
            ];
            let line_h = HUD_FONT_SIZE + 2.0;
            let block_h = line_h * lines.len() as f32 + HUD_PAD;
            bl_y -= block_h;
            for (i, line) in lines.iter().enumerate() {
                let y = bl_y + line_h * i as f32;
                draw_hud_text(
                    painter,
                    line,
                    &font,
                    Pos2::new(rect.left() + HUD_PAD, y),
                    HUD_GOLD,
                );
            }
        }

        bl_y
    }
}

/// Draw text with semi-transparent background rect.
fn draw_hud_text(painter: &egui::Painter, text: &str, font: &FontId, pos: Pos2, color: Color32) {
    let galley = painter.layout_no_wrap(text.to_string(), font.clone(), color);
    let bg_rect = Rect::from_min_size(
        pos - egui::vec2(2.0, 1.0),
        galley.size() + egui::vec2(4.0, 2.0),
    );
    painter.rect_filled(bg_rect, 2.0, HUD_BG);
    painter.galley(pos, galley, color);
}

/// Recursively count prims by type name.
/// Iterative prim traversal (stack-safe for deep hierarchies).
fn count_prims_iterative(
    root: &usd_core::Prim,
    counts: &mut HashMap<String, usize>,
    total: &mut usize,
    stack: &mut Vec<usd_core::Prim>,
) {
    usd_trace::trace_scope!("hud_count_prims_iterative");
    if !root.is_valid() {
        return;
    }
    stack.clear();
    stack.push(root.clone());
    while let Some(prim) = stack.pop() {
        let type_name = prim.get_type_name();
        let name_str = type_name.as_str();
        if !name_str.is_empty() {
            *counts.entry(name_str.to_string()).or_insert(0) += 1;
        }
        *total += 1;
        for child in prim.get_children() {
            stack.push(child);
        }
    }
}
