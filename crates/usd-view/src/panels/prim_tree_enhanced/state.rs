//! Prim tree state: filters, search, actions, and the main PrimTreeState struct.

use std::collections::{HashMap, HashSet};

use usd_core::Prim;
use usd_sdf::{Path, TimeCode};

use super::FlatTreeRow;
use super::view_item::PrimViewItem;

// ---------------------------------------------------------------------------
// Filters
// ---------------------------------------------------------------------------

/// Show/hide filter flags for the prim tree.
#[derive(Debug, Clone, PartialEq)]
pub struct PrimTreeFilters {
    pub show_inactive: bool,
    pub show_prototypes: bool,
    pub show_undefined: bool,
    pub show_abstract: bool,
    pub use_display_names: bool,
}

impl Default for PrimTreeFilters {
    fn default() -> Self {
        Self {
            show_inactive: true,
            show_prototypes: false,
            show_undefined: false,
            show_abstract: false,
            use_display_names: true,
        }
    }
}

impl PrimTreeFilters {
    /// Whether this prim should be shown given current filters.
    pub fn accept(&self, item: &PrimViewItem) -> bool {
        if !self.show_inactive && !item.is_active {
            return false;
        }
        if !self.show_prototypes && item.is_prototype {
            return false;
        }
        if !self.show_undefined && !item.is_defined && !item.is_abstract {
            return false;
        }
        if !self.show_abstract && item.is_abstract {
            return false;
        }
        true
    }
}

// ---------------------------------------------------------------------------
// Search state
// ---------------------------------------------------------------------------

/// Prim tree search state.
#[derive(Debug, Clone, Default)]
pub struct PrimTreeSearch {
    pub query: String,
    pub use_regex: bool,
    /// Index of the last match found (for Find Next cycling).
    pub last_match_idx: usize,
}

impl PrimTreeSearch {
    /// Returns true if path/name matches the current query.
    pub fn matches(&self, name: &str, path_str: &str) -> bool {
        if self.query.is_empty() {
            return true;
        }
        if self.use_regex {
            if let Ok(re) = regex::Regex::new(&self.query) {
                return re.is_match(name) || re.is_match(path_str);
            }
        }
        let q = self.query.to_lowercase();
        name.to_lowercase().contains(&q) || path_str.to_lowercase().contains(&q)
    }
}

// ---------------------------------------------------------------------------
// Actions from prim tree UI
// ---------------------------------------------------------------------------

/// Actions the prim tree can request.
#[derive(Debug, Clone)]
pub enum PrimTreeAction {
    Select(Path),
    CopyPath(Path),
    CopyPrimName(String),
    CopyModelPath(Path),
    Frame(Path),
    MakeVisible(Path),
    MakeInvisible(Path),
    VisOnly(Path),
    RemoveSessionVis(Path),
    LoadPayload(Path),
    UnloadPayload(Path),
    Activate(Path),
    Deactivate(Path),
    SetAsActiveCamera(Path),
    SetAsActiveRenderSettings(Path),
    SetAsActiveRenderPass(Path),
    JumpToEnclosingModel(Path),
    SelectBoundPreviewMaterial(Path),
    SelectBoundFullMaterial(Path),
    /// Set a variant selection on a prim's variant set.
    SetVariantSelection {
        path: Path,
        vset_name: String,
        variant_name: String,
    },
    /// Toggle visibility on a prim (VIS column click, P1-7).
    ToggleVis(Path),
    /// Load payload and all descendants (P1-5).
    LoadPayloadWithDescendants(Path),
    /// Set draw mode on a model prim via session layer (P1-1).
    SetDrawMode(Path, String),
    /// Clear session-layer draw mode override (P1-1).
    ClearDrawMode(Path),
    /// Toggle guide visibility on a prim (P1-4).
    ToggleGuides(Path),
}

// ---------------------------------------------------------------------------
// Enhanced prim tree state
// ---------------------------------------------------------------------------

/// Persistent state for the enhanced prim tree panel.
///
/// Separates data collection (rebuild_flat_list when dirty) from rendering
/// (show_rows renders only visible rows each frame).
#[derive(Debug)]
pub struct PrimTreeState {
    /// Cached PrimViewItem per path string key.
    pub cache: HashMap<String, PrimViewItem>,
    /// Set of expanded path keys.
    pub expanded: HashSet<String>,
    /// Flattened visible tree rows (rebuilt only when dirty).
    pub(super) flat_rows: Vec<FlatTreeRow>,
    /// Filter settings.
    pub filters: PrimTreeFilters,
    /// Previous filters snapshot for dirty detection.
    prev_filters: PrimTreeFilters,
    /// Search state.
    pub search: PrimTreeSearch,
    /// Previous search query for dirty detection.
    prev_search_query: String,
    /// Previous search regex flag for dirty detection.
    prev_search_regex: bool,
    /// Collected actions during this frame.
    pub actions: Vec<PrimTreeAction>,
    /// Request keyboard focus for the search box on next UI pass.
    pub request_search_focus: bool,
    /// Whether flat list needs rebuild.
    pub tree_dirty: bool,
    /// Show type column in prim tree.
    pub show_type_column: bool,
    /// Show visibility column in prim tree.
    pub show_vis_column: bool,
    /// Show draw mode column in prim tree.
    pub show_draw_mode_column: bool,
    /// Show guides visibility column in prim tree (P1-4).
    pub show_guides_column: bool,
    /// Row index to scroll into view on the next frame (set by keyboard nav).
    pub scroll_to_row: Option<usize>,
    /// Path key of the prim whose draw mode is being edited (P1-1 popup).
    pub draw_mode_edit_path: Option<String>,
}

impl Default for PrimTreeState {
    fn default() -> Self {
        let filters = PrimTreeFilters::default();
        Self {
            cache: HashMap::new(),
            expanded: HashSet::new(),
            flat_rows: Vec::new(),
            prev_filters: filters.clone(),
            filters,
            search: PrimTreeSearch::default(),
            prev_search_query: String::new(),
            prev_search_regex: false,
            actions: Vec::new(),
            request_search_focus: false,
            tree_dirty: true,
            show_type_column: true,
            show_vis_column: true,
            show_draw_mode_column: true,
            show_guides_column: true,
            scroll_to_row: None,
            draw_mode_edit_path: None,
        }
    }
}

impl PrimTreeState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark everything as dirty (e.g. stage reloaded).
    pub fn invalidate(&mut self) {
        self.cache.clear();
        self.flat_rows.clear();
        self.expanded.clear();
        self.tree_dirty = true;
    }

    /// Toggle expand/collapse for a path.
    pub(super) fn toggle_expanded(&mut self, key: &str) {
        if self.expanded.contains(key) {
            self.expanded.remove(key);
        } else {
            self.expanded.insert(key.to_string());
        }
        self.tree_dirty = true;
    }

    /// Expand all descendants of a prim recursively.
    pub fn expand_recursive(&mut self, prim: &Prim) {
        let mut stack = vec![prim.clone()];
        while let Some(p) = stack.pop() {
            if !p.is_valid() {
                continue;
            }
            let children = p.get_all_children();
            if !children.is_empty() {
                self.expanded.insert(p.path().to_string());
                stack.extend(children);
            }
        }
        self.tree_dirty = true;
    }

    /// Collapse all descendants of a prim recursively.
    pub fn collapse_recursive(&mut self, prim: &Prim) {
        let root_key = prim.path().to_string();
        if root_key == "/" {
            // Pseudo-root: clear everything
            self.expanded.clear();
        } else {
            self.expanded.remove(&root_key);
            // Remove all expanded keys that start with this path
            let prefix = format!("{}/", root_key);
            self.expanded.retain(|k| !k.starts_with(&prefix));
        }
        self.tree_dirty = true;
    }

    /// Expand prims from root up to a given depth level.
    pub fn expand_to_depth(&mut self, prim: &Prim, max_depth: usize) {
        self.expand_to_depth_recursive(prim, 0, max_depth);
        self.tree_dirty = true;
    }

    fn expand_to_depth_recursive(&mut self, prim: &Prim, current_depth: usize, max_depth: usize) {
        // Use `>` not `>=`: pseudo-root consumes depth 0, so "Level N" must
        // expand through depth N to show children at flat-tree depth N.
        if current_depth > max_depth {
            return;
        }
        if !prim.is_valid() || !prim.has_children() {
            return;
        }
        let key = prim.path().to_string();
        self.expanded.insert(key);
        for child in prim.get_all_children() {
            self.expand_to_depth_recursive(&child, current_depth + 1, max_depth);
        }
    }

    /// Check if filters or search changed and set dirty accordingly.
    pub(super) fn check_dirty(&mut self) {
        if self.filters != self.prev_filters
            || self.search.query != self.prev_search_query
            || self.search.use_regex != self.prev_search_regex
        {
            self.tree_dirty = true;
            self.prev_filters = self.filters.clone();
            self.prev_search_query = self.search.query.clone();
            self.prev_search_regex = self.search.use_regex;
        }
    }

    /// Get or create a cached PrimViewItem for the given prim.
    fn get_or_insert(&mut self, prim: &Prim, tc: TimeCode, depth: usize) -> PrimViewItem {
        let key = prim.path().to_string();
        if let Some(item) = self.cache.get(&key) {
            return item.clone();
        }
        let item = PrimViewItem::from_prim(prim, tc, depth);
        self.cache.insert(key, item.clone());
        item
    }

    /// Rebuild the flat row list by walking expanded nodes.
    /// Only called when tree_dirty is true.
    pub(super) fn rebuild_flat_list(&mut self, root: &Prim, tc: TimeCode) {
        self.flat_rows.clear();
        let children = root.get_all_children();
        for child in &children {
            self.flatten_prim(child, tc, 0);
        }
        self.tree_dirty = false;
    }

    /// Recursively flatten a prim into flat_rows.
    fn flatten_prim(&mut self, prim: &Prim, tc: TimeCode, depth: usize) {
        if !prim.is_valid() || depth > 64 {
            return;
        }

        let item = self.get_or_insert(prim, tc, depth);

        // Apply filters
        if !self.filters.accept(&item) {
            return;
        }

        // Apply search filter
        let path_str = item.path.to_string();
        if !self.search.matches(&item.name, &path_str) {
            // Prim itself doesn't match. Still walk children to find
            // descendants that do match — always recurse regardless of
            // expanded state so search results are never hidden.
            if !item.has_children {
                return;
            }
            let row_count_before = self.flat_rows.len();
            let is_expanded = self.expanded.contains(&path_str);
            // Always recurse children when searching (ignore expanded state)
            // so that matches deep in the tree are surfaced.
            for child in prim.get_all_children() {
                self.flatten_prim(&child, tc, depth + 1);
            }
            // If any descendants were added, prepend ancestor row for context.
            // The ancestor row is shown collapsed (is_expanded=false) so the
            // user sees the match path, matching usdview behavior.
            if self.flat_rows.len() > row_count_before {
                let parent_row = FlatTreeRow {
                    path_key: path_str,
                    depth,
                    has_children: item.has_children,
                    is_expanded,
                };
                self.flat_rows.insert(row_count_before, parent_row);
            }
            return;
        }

        let is_expanded = self.expanded.contains(&path_str);

        self.flat_rows.push(FlatTreeRow {
            path_key: path_str,
            depth,
            has_children: item.has_children,
            is_expanded,
        });

        // Only recurse into children if expanded
        if is_expanded && item.has_children {
            for child in prim.get_all_children() {
                self.flatten_prim(&child, tc, depth + 1);
            }
        }
    }
}
