//! Enhanced prim tree panel with virtual scrolling.
//!
//! Architecture: DATA COLLECTION (when dirty) is separated from RENDERING (every frame).
//! - PrimViewItem: cached per-prim display data (queried from USD once, reused)
//! - FlatTreeRow: lightweight row in the flattened visible tree (path key + depth + expand state)
//! - PrimTreeState: owns expanded set, flat row cache, dirty flags
//! - show_rows(): only renders the ~20-50 rows visible in viewport, not all 10K+
//!
//! Features: type coloring, font styles, ancestor highlighting, visibility toggle,
//! draw mode editor, search with regex, show/hide filters, display names, tooltips,
//! and full context menu.

mod render;
pub mod state;
pub mod view_item;

pub use render::ui_prim_tree_enhanced;
pub use state::{PrimTreeAction, PrimTreeFilters, PrimTreeSearch, PrimTreeState};
pub use view_item::{PrimDrawMode, PrimViewItem, PrimVisibility};

/// Width of the VIS column (px).
pub(super) const VIS_COL_W: f32 = 18.0;
/// Width of the GUIDES column (px).
pub(super) const GUIDES_COL_W: f32 = 18.0;
/// Width of the DRAWMODE column (px).
pub(super) const DRAWMODE_COL_W: f32 = 68.0;

use egui::Color32;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Row height for virtual scrolling (px).
const ROW_HEIGHT: f32 = 18.0;
/// Indentation per depth level (px).
const INDENT_PX: f32 = 16.0;
/// Expand arrow width (px).
const ARROW_W: f32 = 18.0;

// ---------------------------------------------------------------------------
// Colors (matching UIPrimTreeColors from reference)
// ---------------------------------------------------------------------------

/// Instance prim color -- light blue.
const CLR_INSTANCE: Color32 = Color32::from_rgb(135, 206, 250);
/// Prim with composition arcs -- dark yellow.
const CLR_HAS_ARCS: Color32 = Color32::from_rgb(222, 158, 46);
/// Prototype prim -- purple.
const CLR_PROTOTYPE: Color32 = Color32::from_rgb(118, 136, 217);
/// Normal prim -- light gray.
const CLR_NORMAL: Color32 = Color32::from_rgb(227, 227, 227);
/// Selected prim text -- warm highlight (not too bright).
const CLR_SELECTED: Color32 = Color32::from_rgb(220, 190, 120);
/// Ancestor of selected prim -- lighter gold background.
const CLR_ANCESTOR_BG: Color32 = Color32::from_rgb(80, 70, 40);

// ---------------------------------------------------------------------------
// FlatTreeRow -- lightweight row in the flattened visible tree (crate-private)
// ---------------------------------------------------------------------------

/// One row in the flattened tree. Indexes into PrimTreeState.cache by path_key.
/// Only stores layout info; display data lives in PrimViewItem cache.
#[derive(Debug, Clone)]
struct FlatTreeRow {
    /// Key into cache HashMap (path.to_string()).
    path_key: String,
    /// Depth level for indentation.
    depth: usize,
    /// Whether this prim has children (show expand arrow).
    has_children: bool,
    /// Whether this prim is currently expanded.
    is_expanded: bool,
}
