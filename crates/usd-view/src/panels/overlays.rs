//! Viewport overlays — axis indicator, bounding boxes, camera mask, reticles.
//!
//! Drawn on top of the rendered image using egui painter primitives.
//! Reference: usdviewq stageView.py overlay drawing.
//!
//! # Convention
//!
//! Row-vector (Imath/OpenUSD): `clip = point * VP`, where `VP = View * Proj`.
//! When projecting points to screen, use `VP.column(i)` — NOT `VP.row(i)`.
//! See [`project_to_screen`] for the canonical projection implementation.

use egui::{Color32, Pos2, Rect, Stroke, Ui};
use usd_gf::matrix4::Matrix4d;
use usd_gf::vec3::Vec3d;

/// RGB colors for XYZ axes (standard: R=X, G=Y, B=Z).
const AXIS_X: Color32 = Color32::from_rgb(220, 50, 50);
const AXIS_Y: Color32 = Color32::from_rgb(50, 200, 50);
const AXIS_Z: Color32 = Color32::from_rgb(50, 100, 220);

/// Axis label colors (slightly brighter).
const AXIS_LABEL_X: Color32 = Color32::from_rgb(255, 80, 80);
const AXIS_LABEL_Y: Color32 = Color32::from_rgb(80, 255, 80);
const AXIS_LABEL_Z: Color32 = Color32::from_rgb(80, 130, 255);

// Note: mask/reticle colors now passed as parameters from data_model.

/// Overlay visibility toggles.
#[derive(Debug, Clone)]
pub struct OverlayState {
    /// Show RGB axis indicator in corner.
    pub show_axes: bool,
    /// Show selection bounding box wireframe.
    pub show_selection_bbox: bool,
    /// Show full scene bounding box wireframe.
    pub show_scene_bbox: bool,
    /// Show camera mask (letterbox) overlay.
    pub show_camera_mask: bool,
    /// Show camera reticles (crosshair / rule-of-thirds).
    pub show_reticles: bool,
    /// Camera aspect ratio for mask/reticle (0 = use viewport aspect).
    pub camera_aspect: f64,
}

impl Default for OverlayState {
    fn default() -> Self {
        Self {
            show_axes: true,
            show_selection_bbox: true,
            show_scene_bbox: false,
            show_camera_mask: false,
            show_reticles: false,
            camera_aspect: 0.0,
        }
    }
}

impl OverlayState {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Draw the axis orientation indicator in the bottom-left corner.
///
/// Projects world XYZ directions through the view matrix rotation
/// (ignoring translation) to get screen-space axis directions.
pub fn draw_axes(ui: &mut Ui, rect: Rect, view_matrix: &Matrix4d) {
    let painter = ui.painter();
    let axis_len = 30.0_f32;
    let origin = Pos2::new(rect.left() + 40.0, rect.bottom() - 40.0);

    // Extract rotation from view matrix (upper-left 3x3)
    let axes = [
        (Vec3d::new(1.0, 0.0, 0.0), AXIS_X, AXIS_LABEL_X, "X"),
        (Vec3d::new(0.0, 1.0, 0.0), AXIS_Y, AXIS_LABEL_Y, "Y"),
        (Vec3d::new(0.0, 0.0, 1.0), AXIS_Z, AXIS_LABEL_Z, "Z"),
    ];

    for (dir, color, label_color, label) in &axes {
        let rotated = transform_direction(view_matrix, dir);
        // Project to screen: x right, y up (egui y is down, so negate)
        let end = Pos2::new(
            origin.x + rotated.x as f32 * axis_len,
            origin.y - rotated.y as f32 * axis_len,
        );
        painter.line_segment([origin, end], Stroke::new(2.0, *color));

        // Label at axis tip
        let label_pos = Pos2::new(
            origin.x + rotated.x as f32 * (axis_len + 10.0),
            origin.y - rotated.y as f32 * (axis_len + 10.0),
        );
        painter.text(
            label_pos,
            egui::Align2::CENTER_CENTER,
            *label,
            egui::FontId::proportional(10.0),
            *label_color,
        );
    }
}

/// Draw bounding box wireframe overlay projected to screen space.
///
/// `bbox_min`/`bbox_max` are world-space AABB corners.
/// Uses view+proj matrices to project the 8 corners, then draws 12 edges.
pub fn draw_bbox(
    ui: &mut Ui,
    rect: Rect,
    bbox_min: Vec3d,
    bbox_max: Vec3d,
    view_matrix: &Matrix4d,
    proj_matrix: &Matrix4d,
    color: Color32,
) {
    let painter = ui.painter();
    let vp = *view_matrix * *proj_matrix;
    let w = rect.width();
    let h = rect.height();

    // 8 corners of the AABB
    let corners = [
        Vec3d::new(bbox_min.x, bbox_min.y, bbox_min.z),
        Vec3d::new(bbox_max.x, bbox_min.y, bbox_min.z),
        Vec3d::new(bbox_max.x, bbox_max.y, bbox_min.z),
        Vec3d::new(bbox_min.x, bbox_max.y, bbox_min.z),
        Vec3d::new(bbox_min.x, bbox_min.y, bbox_max.z),
        Vec3d::new(bbox_max.x, bbox_min.y, bbox_max.z),
        Vec3d::new(bbox_max.x, bbox_max.y, bbox_max.z),
        Vec3d::new(bbox_min.x, bbox_max.y, bbox_max.z),
    ];

    // Project corners to screen space
    let screen: Vec<Option<Pos2>> = corners
        .iter()
        .map(|c| project_to_screen(&vp, c, rect.min, w, h))
        .collect();

    // 12 edges of a box (index pairs)
    let edges = [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 0), // front face
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 4), // back face
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7), // connecting edges
    ];

    let stroke = Stroke::new(1.0, color);
    for (a, b) in &edges {
        if let (Some(pa), Some(pb)) = (screen[*a], screen[*b]) {
            painter.line_segment([pa, pb], stroke);
        }
    }
}

/// Draw camera mask (letterbox bars) for a given target aspect ratio.
///
/// If `camera_aspect` is wider than viewport, draws top/bottom bars.
/// If narrower, draws left/right bars.
pub fn draw_camera_mask(ui: &mut Ui, rect: Rect, camera_aspect: f64, mask_color: Color32) {
    if camera_aspect <= 0.0 {
        return;
    }

    let painter = ui.painter();
    let vp_aspect = (rect.width() / rect.height().max(1.0)) as f64;

    if camera_aspect > vp_aspect {
        // Camera is wider — letterbox top/bottom
        let visible_h = rect.width() as f64 / camera_aspect;
        let bar_h = ((rect.height() as f64 - visible_h) * 0.5).max(0.0) as f32;
        painter.rect_filled(
            Rect::from_min_size(rect.left_top(), egui::vec2(rect.width(), bar_h)),
            0.0,
            mask_color,
        );
        painter.rect_filled(
            Rect::from_min_size(
                Pos2::new(rect.left(), rect.bottom() - bar_h),
                egui::vec2(rect.width(), bar_h),
            ),
            0.0,
            mask_color,
        );
    } else {
        // Camera is narrower — pillarbox left/right
        let visible_w = rect.height() as f64 * camera_aspect;
        let bar_w = ((rect.width() as f64 - visible_w) * 0.5).max(0.0) as f32;
        painter.rect_filled(
            Rect::from_min_size(rect.left_top(), egui::vec2(bar_w, rect.height())),
            0.0,
            mask_color,
        );
        painter.rect_filled(
            Rect::from_min_size(
                Pos2::new(rect.right() - bar_w, rect.top()),
                egui::vec2(bar_w, rect.height()),
            ),
            0.0,
            mask_color,
        );
    }
}

/// Draw camera mask outline (thin border around the masked region).
pub fn draw_camera_mask_outline(ui: &mut Ui, rect: Rect, camera_aspect: f64) {
    if camera_aspect <= 0.0 {
        return;
    }
    let painter = ui.painter();
    let vp_aspect = (rect.width() / rect.height().max(1.0)) as f64;
    let stroke = Stroke::new(1.0, Color32::from_rgb(200, 200, 200));

    if camera_aspect > vp_aspect {
        let visible_h = rect.width() as f64 / camera_aspect;
        let bar_h = ((rect.height() as f64 - visible_h) * 0.5).max(0.0) as f32;
        let inner = Rect::from_min_max(
            Pos2::new(rect.left(), rect.top() + bar_h),
            Pos2::new(rect.right(), rect.bottom() - bar_h),
        );
        painter.rect_stroke(inner, 0.0, stroke, egui::StrokeKind::Outside);
    } else {
        let visible_w = rect.height() as f64 * camera_aspect;
        let bar_w = ((rect.width() as f64 - visible_w) * 0.5).max(0.0) as f32;
        let inner = Rect::from_min_max(
            Pos2::new(rect.left() + bar_w, rect.top()),
            Pos2::new(rect.right() - bar_w, rect.bottom()),
        );
        painter.rect_stroke(inner, 0.0, stroke, egui::StrokeKind::Outside);
    }
}

/// Draw camera reticles (rule-of-thirds grid + center crosshair).
pub fn draw_reticles(ui: &mut Ui, rect: Rect, reticle_color: Color32) {
    let painter = ui.painter();
    let stroke = Stroke::new(1.0, reticle_color);

    // Center crosshair
    let cx = rect.center().x;
    let cy = rect.center().y;
    let cross_size = 10.0;
    painter.line_segment(
        [
            Pos2::new(cx - cross_size, cy),
            Pos2::new(cx + cross_size, cy),
        ],
        stroke,
    );
    painter.line_segment(
        [
            Pos2::new(cx, cy - cross_size),
            Pos2::new(cx, cy + cross_size),
        ],
        stroke,
    );

    // Rule-of-thirds grid (2 horizontal + 2 vertical lines)
    let w = rect.width();
    let h = rect.height();
    for i in 1..=2 {
        let t = i as f32 / 3.0;
        // Vertical
        let x = rect.left() + w * t;
        painter.line_segment(
            [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
            stroke,
        );
        // Horizontal
        let y = rect.top() + h * t;
        painter.line_segment(
            [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
            stroke,
        );
    }
}

/// Draw XZ ground-plane grid overlay projected to screen space.
///
/// Grid covers +-`extent` units with `spacing` cell size and `major_every`
/// major-line interval. Axis lines (X=red at Z=0, Z=blue at X=0) are drawn
/// thicker for orientation. Lines fade near the grid edge to avoid hard cutoff.
pub fn draw_grid(
    ui: &mut Ui,
    rect: Rect,
    view_matrix: &Matrix4d,
    proj_matrix: &Matrix4d,
    extent: f64,
    spacing: f64,
    major_every: i32,
    is_z_up: bool,
) {
    let vp = *view_matrix * *proj_matrix;
    let w = rect.width();
    let h = rect.height();
    let painter = ui.painter();

    let steps = (extent / spacing).round() as i32;
    // Fade zone: lines in the outer 30% of extent fade to transparent
    let fade_start = extent * 0.7;

    for i in -steps..=steps {
        let t = i as f64 * spacing;
        let is_major = major_every > 0 && i % major_every == 0;
        let is_axis = i == 0;

        // Edge fade factor: 1.0 at center, 0.0 at extent boundary
        let abs_t = t.abs();
        let fade = if abs_t > fade_start {
            1.0 - ((abs_t - fade_start) / (extent - fade_start)).clamp(0.0, 1.0)
        } else {
            1.0
        };

        // Grid plane: Y-up → XZ floor (Y=0), Z-up → XY floor (Z=0).
        // First axis lines (parallel to X): at i=0 this IS the X axis (red).
        let (base_color_x, width_x) = if is_axis {
            ([200u8, 60, 60], 2.0_f32)
        } else if is_major {
            ([120, 120, 120], 1.2)
        } else {
            ([80, 80, 80], 0.7)
        };
        let alpha_x = if is_axis {
            220.0
        } else if is_major {
            140.0
        } else {
            100.0
        };
        let a = (alpha_x * fade) as u8;
        let color_x =
            Color32::from_rgba_unmultiplied(base_color_x[0], base_color_x[1], base_color_x[2], a);

        let (p0, p1) = if is_z_up {
            // Z-up: lines along X at Y=t, Z=0
            (Vec3d::new(-extent, t, 0.0), Vec3d::new(extent, t, 0.0))
        } else {
            // Y-up: lines along X at Z=t, Y=0
            (Vec3d::new(-extent, 0.0, t), Vec3d::new(extent, 0.0, t))
        };
        if let (Some(pa), Some(pb)) = (
            project_to_screen(&vp, &p0, rect.min, w, h),
            project_to_screen(&vp, &p1, rect.min, w, h),
        ) {
            painter.line_segment([pa, pb], egui::Stroke::new(width_x, color_x));
        }

        // Second axis lines: Y-up → parallel to Z (blue), Z-up → parallel to Y (green).
        let (base_color_z, width_z) = if is_axis {
            if is_z_up {
                ([60u8, 200, 60], 2.0_f32) // Y axis = green for Z-up
            } else {
                ([60u8, 100, 200], 2.0_f32) // Z axis = blue for Y-up
            }
        } else if is_major {
            ([120, 120, 120], 1.2)
        } else {
            ([80, 80, 80], 0.7)
        };
        let alpha_z = if is_axis {
            220.0
        } else if is_major {
            140.0
        } else {
            100.0
        };
        let az = (alpha_z * fade) as u8;
        let color_z =
            Color32::from_rgba_unmultiplied(base_color_z[0], base_color_z[1], base_color_z[2], az);

        let (q0, q1) = if is_z_up {
            // Z-up: lines along Y at X=t, Z=0
            (Vec3d::new(t, -extent, 0.0), Vec3d::new(t, extent, 0.0))
        } else {
            // Y-up: lines along Z at X=t, Y=0
            (Vec3d::new(t, 0.0, -extent), Vec3d::new(t, 0.0, extent))
        };
        if let (Some(pc), Some(pd)) = (
            project_to_screen(&vp, &q0, rect.min, w, h),
            project_to_screen(&vp, &q1, rect.min, w, h),
        ) {
            painter.line_segment([pc, pd], egui::Stroke::new(width_z, color_z));
        }
    }
}

/// Choose a bbox line color that contrasts with the given clear/background color.
pub fn contrasting_bbox_color(clear: Color32) -> Color32 {
    let luma = 0.299 * clear.r() as f64 + 0.587 * clear.g() as f64 + 0.114 * clear.b() as f64;
    if luma > 128.0 {
        Color32::from_rgb(40, 40, 40) // Dark lines on bright bg
    } else {
        Color32::from_rgb(200, 200, 50) // Yellow lines on dark bg
    }
}

/// Draw a camera frustum wireframe oracle for a scene camera.
///
/// Projects the 8 corners of the camera frustum (near + far planes) into
/// screen space and draws 12 edges as a wireframe box. Skipped when the
/// viewport is already looking through `cam_view`/`cam_proj` (i.e. the
/// oracle camera itself).
pub fn draw_camera_oracle(
    ui: &mut Ui,
    rect: Rect,
    cam_view: &Matrix4d,
    cam_proj: &Matrix4d,
    viewport_view: &Matrix4d,
    viewport_proj: &Matrix4d,
) {
    // Compute inverse VP of the scene camera to get frustum corners in world space
    let cam_vp = *cam_view * *cam_proj;
    let inv_vp = match cam_vp.inverse() {
        Some(m) => m,
        None => return,
    };

    // 8 NDC frustum corners: near (z=-1) and far (z=1) planes
    let ndc_corners: [(f64, f64, f64); 8] = [
        (-1.0, -1.0, -1.0),
        (1.0, -1.0, -1.0),
        (1.0, 1.0, -1.0),
        (-1.0, 1.0, -1.0),
        (-1.0, -1.0, 1.0),
        (1.0, -1.0, 1.0),
        (1.0, 1.0, 1.0),
        (-1.0, 1.0, 1.0),
    ];

    // Unproject NDC corners to world space
    let world_corners: Vec<Vec3d> = ndc_corners
        .iter()
        .map(|&(x, y, z)| inv_vp.transform_point(&Vec3d::new(x, y, z)))
        .collect();

    // Project world-space frustum corners through viewport's VP
    let vp = *viewport_view * *viewport_proj;
    let w = rect.width();
    let h = rect.height();
    let screen: Vec<Option<Pos2>> = world_corners
        .iter()
        .map(|c| project_to_screen(&vp, c, rect.min, w, h))
        .collect();

    // 12 edges of the frustum box
    let edges = [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 0), // near plane
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 4), // far plane
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7), // connecting edges
    ];

    let stroke = Stroke::new(1.5, Color32::from_rgb(255, 180, 50));
    let painter = ui.painter();
    for (a, b) in &edges {
        if let (Some(pa), Some(pb)) = (screen[*a], screen[*b]) {
            painter.line_segment([pa, pb], stroke);
        }
    }
}

// --- Internal helpers ---

/// Transform a direction vector by the upper-left 3x3 of a 4x4 matrix.
fn transform_direction(m: &Matrix4d, dir: &Vec3d) -> Vec3d {
    // Matrix4d stores data as [[T; 4]; 4], access via row()
    let r0 = m.row(0);
    let r1 = m.row(1);
    let r2 = m.row(2);
    Vec3d::new(
        r0.x * dir.x + r0.y * dir.y + r0.z * dir.z,
        r1.x * dir.x + r1.y * dir.y + r1.z * dir.z,
        r2.x * dir.x + r2.y * dir.y + r2.z * dir.z,
    )
}

/// Project a world-space point to screen-space pixel coordinates.
///
/// Returns `None` if the point is behind the camera (w <= 0).
fn project_to_screen(
    vp_matrix: &Matrix4d,
    point: &Vec3d,
    viewport_min: Pos2,
    viewport_w: f32,
    viewport_h: f32,
) -> Option<Pos2> {
    // USD row-vector convention: clip = point * VP, use columns not rows
    let c0 = vp_matrix.column(0);
    let c1 = vp_matrix.column(1);
    let c3 = vp_matrix.column(3);

    let clip_x = point.x * c0.x + point.y * c0.y + point.z * c0.z + c0.w;
    let clip_y = point.x * c1.x + point.y * c1.y + point.z * c1.z + c1.w;
    let clip_w = point.x * c3.x + point.y * c3.y + point.z * c3.z + c3.w;

    if clip_w <= 0.0 {
        return None; // Behind camera
    }

    let ndc_x = clip_x / clip_w;
    let ndc_y = clip_y / clip_w;

    // NDC [-1, 1] -> screen pixels (y flipped for egui top-down)
    let sx = viewport_min.x + (ndc_x as f32 + 1.0) * 0.5 * viewport_w;
    let sy = viewport_min.y + (1.0 - ndc_y as f32) * 0.5 * viewport_h;

    Some(Pos2::new(sx, sy))
}
